//! Native resource binding, measurement and exact emission plans for source atoms.
use super::*;
use styled_paragraph_layout::{GlyphMetric, MeasuredOccurrence, OccurrenceLayout, StyleMetrics};
use styled_paragraph_plan::ResolvedStyle;

#[derive(Clone)]
pub(super) struct FragmentPlan {
    pub text: String,
    pub bytes: Vec<u8>,
    pub glyphs: Vec<(String, [f32; 2], f32)>,
}
pub(super) type Fragments = HashMap<[u32; 2], FragmentPlan>;
pub(super) fn origin_key(origin: [f32; 2]) -> [u32; 2] {
    origin.map(|v| if v == 0. { 0 } else { v.to_bits() })
}
pub(super) struct Measurement {
    pub atoms: Vec<MeasuredOccurrence>,
    pub metrics: Vec<StyleMetrics>,
}

/// Recover exact RGB from current source operators before deciding whether two
/// saved formatting IDs are equivalent. Probe pages carry only the8-bit UI
/// colors, which cannot distinguish e.g.0.1 from0.1001. Candidate run offsets
/// must consume the independently decoded current fragments in order.
pub(super) fn restore_rgb(
    p: &Prepared, runs: &[StyledParagraphRun], styles: &mut [ResolvedStyle], bindings: &mut [BoundStyle],
) -> EngineResult<()> {
    let logical=runs.iter().map(|r|r.text.as_str()).collect::<String>();
    if logical.len()>4096||styles.len()!=bindings.len(){return Err(invalid("Invalid saved formatting attribution."));}
    let ids=styles.iter().map(|s|resource(p,&s.resource_name)).collect::<EngineResult<Vec<_>>>()?;
    let mut ranges=Vec::new();let mut offset=0;
    for run in runs {
        let end=offset+run.text.len();ranges.push((offset,end,run.style_id));offset=end;
    }
    let mut seen=vec![false;styles.len()];let mut cursor=0;
    for id in &p.selection.object_indices {
        while logical.as_bytes().get(cursor)==Some(&b'\n'){cursor+=1;}
        let value=p.source_text.get(id).ok_or_else(||invalid("Missing independently decoded saved fragment."))?;
        let end=cursor.checked_add(value.len()).filter(|n|*n<=logical.len()).ok_or_else(||invalid("Invalid saved text attribution."))?;
        if value.is_empty()||logical.get(cursor..end)!=Some(value.as_str()){return Err(invalid("Saved formatting text differs from the current operands."));}
        let actual=p.before.iter().find(|r|r.index==*id).ok_or_else(||invalid("Missing saved glyph owner."))?;
        let owner=p.styles.iter().find(|s|s.object_indices.contains(id)).ok_or_else(||invalid("Missing actual saved paint state."))?;
        let start=ranges.partition_point(|(_,end,_)|*end<=cursor);
        for &(_,_,style_id) in ranges[start..].iter().take_while(|(start,_,_)|*start<end) {
            let style=styles.get_mut(style_id).ok_or_else(||invalid("Unknown saved formatting style."))?;
            if ids[style_id]!=owner.resource_id||style.size!=actual.font_size||style.color!=actual.color
                ||style.source.font!=actual.font||style.source.program!=actual.program {
                return Err(invalid("Saved formatting differs from its actual font and paint state."));
            }
            if seen[style_id]&&bindings[style_id].original_rgb!=owner.original_rgb {
                return Err(invalid("One saved style has inconsistent source paint values."));
            }
            bindings[style_id].original_rgb=owner.original_rgb;style.original_rgb=owner.original_rgb;seen[style_id]=true;
        }
        cursor=end;
    }
    if !logical.get(cursor..).is_some_and(|s|s.bytes().all(|b|b==b'\n')){return Err(invalid("Saved formatting attribution is incomplete."));}
    Ok(())
}

fn resource(p: &Prepared, name: &[u8]) -> EngineResult<lopdf::ObjectId> {
    let fonts = p
        .pdf
        .dereference(
            p.resources
                .get(b"Font")
                .map_err(|_| invalid("Missing paragraph fonts."))?,
        )
        .and_then(|(_, v)| v.as_dict())
        .map_err(|_| invalid("Malformed paragraph fonts."))?;
    let value = fonts
        .get(name)
        .map_err(|_| invalid("Missing original paragraph font."))?;
    p.pdf
        .dereference(value)
        .map_err(|_| invalid("Malformed original paragraph font."))?
        .0
        .ok_or_else(|| invalid("Direct paragraph fonts cannot bind source glyphs."))
}

pub(super) fn canonical_runs(
    p: &Prepared,
    runs: &[StyledParagraphRun],
    styles: &[ResolvedStyle],
) -> EngineResult<Vec<StyledParagraphRun>> {
    let ids = styles
        .iter()
        .map(|s| resource(p, &s.resource_name))
        .collect::<EngineResult<Vec<_>>>()?;
    let mut canonical = Vec::with_capacity(styles.len());
    for (index, style) in styles.iter().enumerate() {
        let original = (0..index).find(|&other| {
            let b = &styles[other];
            (style.substitute.is_some() || ids[index] == ids[other])
                && style.size == b.size
                && style.rgb() == b.rgb()
                && style.background_id == b.background_id
                && style.substitute.as_ref().map(|a| &a.info.id)
                    == b.substitute.as_ref().map(|a| &a.info.id)
        });
        canonical.push(original.map_or(index, |i| canonical[i]));
    }
    runs.iter()
        .map(|r| {
            Ok(StyledParagraphRun {
                text: r.text.clone(),
                style_id: *canonical
                    .get(r.style_id)
                    .ok_or_else(|| invalid("Unknown paragraph style."))?,
            })
        })
        .collect()
}

fn sources(
    p: &Prepared,
    styles: &[ResolvedStyle],
) -> EngineResult<Vec<Option<Arc<source_glyphs::BoundText>>>> {
    struct Stream<'a> {
        proof: &'a VerifiedFont,
        program: &'a Option<String>,
        bytes: Vec<u8>,
    }
    let mut streams = HashMap::<lopdf::ObjectId, Stream<'_>>::new();
    let ids = styles
        .iter()
        .map(|s| resource(p, &s.resource_name))
        .collect::<EngineResult<Vec<_>>>()?;
    for (style, id) in styles.iter().zip(&ids) {
        if style.substitute.is_none() {
            if let Some(proof) = style
                .source
                .proof
                .as_ref()
                .filter(|p| p.bind(&[]).is_ok_and(|b| b.is_some()))
            {
                streams.entry(*id).or_insert(Stream {
                    proof,
                    program: &style.source.program,
                    bytes: vec![],
                });
            }
        }
    }
    let mut total = 0usize;
    for run in p
        .before
        .iter()
        .filter(|r| p.selection.object_indices.contains(&r.index))
    {
        let owner = p
            .styles
            .iter()
            .find(|s| s.object_indices.contains(&run.index))
            .ok_or_else(|| invalid("Missing paragraph font ownership."))?;
        if let Some(stream) = streams.get_mut(&owner.resource_id) {
            if run.proof.as_deref() != Some(stream.proof) || &run.program != stream.program {
                return Err(invalid(
                    "A paragraph resource has inconsistent glyph proof.",
                ));
            }
            let text = p
                .source_text
                .get(&run.index)
                .ok_or_else(|| invalid("Missing exact paragraph operand text."))?;
            let raw = positioned_text::encoded(run, text)?;
            total = total
                .checked_add(raw.len())
                .filter(|n| *n <= 8192)
                .ok_or_else(|| invalid("Paragraph source glyphs exceed their budget."))?;
            stream.bytes.extend(raw);
        }
    }
    let books = streams
        .into_iter()
        .map(|(id, stream)| {
            let book = stream
                .proof
                .bind(&stream.bytes)?
                .ok_or_else(|| invalid("Missing source glyph codebook."))?;
            Ok((id, book))
        })
        .collect::<EngineResult<HashMap<_, _>>>()?;
    Ok(styles
        .iter()
        .zip(ids)
        .map(|(s, id)| {
            if s.substitute.is_some() {
                None
            } else {
                books.get(&id).cloned()
            }
        })
        .collect())
}

pub(super) fn measure(
    p: &Prepared,
    probes: &PdfDocument<'_>,
    runs: &[StyledParagraphRun],
    styles: &[ResolvedStyle],
    force: bool,
) -> EngineResult<Option<Measurement>> {
    if !force
        && !p
            .before
            .iter()
            .any(|r| p.selection.object_indices.contains(&r.index) && r.source_encoding.is_some())
    {
        return Ok(None);
    }
    let runs = canonical_runs(p, runs, styles)?;
    let bindings = sources(p, styles)?;
    let scalar_runs = runs
        .iter()
        .filter(|r| bindings[r.style_id].is_none())
        .cloned()
        .collect::<Vec<_>>();
    let metrics = styled_paragraph_plan::measure(probes, &scalar_runs, styles)?;
    let request = runs
        .iter()
        .map(|r| paragraph_occurrences::Run {
            text: &r.text,
            style: r.style_id,
        })
        .collect::<Vec<_>>();
    let atoms = paragraph_occurrences::resolve(&request, &bindings, |id, ch| {
        let style = &styles[id];
        let bytes = style
            .encode(&ch.to_string())
            .map_err(|_| "Unsupported paragraph scalar encoding.")?;
        let width = style
            .advance(ch)
            .map_err(|_| "Unsupported paragraph scalar advance.")?
            / style.size
            * 1000.;
        let gid = if let Some(asset) = &style.substitute {
            Some(
                asset
                    .glyph_id(ch)
                    .map_err(|_| "Unsupported paragraph scalar glyph.")?,
            )
        } else {
            style.source.proof.as_ref().and_then(|p| p.glyph(ch))
        };
        Ok(paragraph_occurrences::Glyph { bytes, gid, width })
    })
    .map_err(invalid)?;

    // A sequence-capable resource alone does not change the saved scalar
    // paragraph policy. Switch metrics only for actual source/replacement
    // occurrences, or when replaying a record already using those metrics.
    let source_sequences = p.before.iter().any(|r| {
        p.selection.object_indices.contains(&r.index)
            && r.source_encoding.as_ref().is_some_and(|b| b.encoded().uses_sequences())
    });
    if !force && !source_sequences && !atoms.iter().any(|a| a.text.chars().count() > 1) {
        return Ok(None);
    }

    // One actual program copy per program identity, sharing the established
    // aggregate font budget instead of copying16MiB for each formatting style.
    let mut programs = HashMap::<String, Vec<u8>>::new();
    let mut program_bytes = 0usize;
    for (id, style) in styles
        .iter()
        .enumerate()
        .filter(|(id, _)| bindings[*id].is_some())
    {
        let key = style
            .source
            .program
            .as_ref()
            .ok_or_else(|| invalid("A source glyph has no program identity."))?;
        if programs.contains_key(key) {
            continue;
        }
        let page = probes.pages().get(id as i32)?;
        let object = page.objects().get(0)?;
        let text = object
            .as_text_object()
            .ok_or_else(|| invalid("Missing source glyph font probe."))?;
        let bytes = text.font().data()?;
        program_bytes = program_bytes
            .checked_add(bytes.len())
            .filter(|n| *n <= 128 * 1024 * 1024)
            .ok_or_else(|| invalid("Paragraph font programs exceed their budget."))?;
        if bytes.len() > 16 * 1024 * 1024 || embedded_font::key(&bytes) != *key {
            return Err(invalid(
                "The source glyph font program does not match its native proof.",
            ));
        }
        programs.insert(key.clone(), bytes);
    }
    let mut measured = Vec::with_capacity(atoms.len());
    for atom in atoms {
        let style = &styles[atom.style];
        let (encoded, gid, metric) = if let Some(glyph) = atom.glyph {
            let metric = if bindings[atom.style].is_some() {
                let gid=glyph.gid.ok_or_else(||invalid("Missing physical source glyph."))?;
                let proof=style.source.proof.as_ref().ok_or_else(||invalid("Missing physical source font proof."))?;
                let ink=if let Some(ink)=proof.source_glyph_ink(gid,style.size)? {
                    // Reuse the bounded CFF proof; do not walk outlines again.
                    if atom.text.chars().all(char::is_whitespace){[0.;4]}else{ink}
                }else{
                    let program=&programs[style.source.program.as_ref().unwrap()];
                    let face=ttf_parser::Face::parse(program,0).map_err(|_|invalid("Invalid proved paragraph font."))?;
                    if face.tables().glyf.is_none() || face.tables().hmtx.is_none() || face.tables().cff.is_some() {
                        return Err(invalid("Source sequences require proved physical glyph metrics."));
                    }
                    let scale=style.size/face.units_per_em()as f32;
                    if atom.text.chars().all(char::is_whitespace){[0.;4]}else{
                        face.glyph_bounding_box(ttf_parser::GlyphId(gid)).map(|b|[b.x_min as f32*scale,b.y_min as f32*scale,b.x_max as f32*scale,b.y_max as f32*scale])
                            .ok_or_else(||invalid("Missing proved source glyph outline."))?
                    }
                };
                GlyphMetric {
                    advance: glyph.width / 1000. * style.size,
                    ink,
                }
            } else {
                *metrics[atom.style]
                    .glyphs
                    .get(&atom.text.chars().next().unwrap())
                    .ok_or_else(|| invalid("Missing measured paragraph scalar."))?
            };
            (glyph.bytes, glyph.gid, metric)
        } else {
            (
                vec![],
                None,
                GlyphMetric {
                    advance: 0.,
                    ink: [0.; 4],
                },
            )
        };
        measured.push(MeasuredOccurrence {
            text: atom.text,
            style_id: atom.style,
            encoded,
            gid,
            metric,
            reservation: metric,
        });
    }
    Ok(Some(Measurement {
        atoms: measured,
        metrics,
    }))
}

pub(super) fn layout(
    atoms: &[MeasuredOccurrence],
    metrics: &[StyleMetrics],
    bounds: [f32; 4],
    alignment: ParagraphAlignment,
    spacing: f32,
    unit: f32,
    insets: [f32; 2],
    text_alignment: bool,
    ink_vertical: bool,
    baseline: Option<f32>,
) -> EngineResult<OccurrenceLayout> {
    let inner = paragraph_highlight_geometry::text_bounds(bounds, insets, text_alignment);
    let mut result = styled_paragraph_layout::layout_occurrences(
        atoms,
        metrics,
        inner,
        alignment,
        spacing,
        unit,
        text_alignment,
        ink_vertical,
        baseline,
    )?;
    result.layout.pdf_bounds = bounds;
    Ok(result)
}

pub(super) fn bind_fragments(
    styles: &mut [ResolvedStyle],
    atoms: &[MeasuredOccurrence],
    result: &OccurrenceLayout,
) -> EngineResult<()> {
    for style in styles.iter_mut() {
        style.fragments = Some(Arc::new(HashMap::new()));
    }
    let public = result
        .layout
        .lines
        .iter()
        .flat_map(|l| &l.runs)
        .collect::<Vec<_>>();
    if public.len() != result.fragments.len() {
        return Err(invalid("Occurrence placement mapping changed."));
    }
    for (fragment, placed) in public.into_iter().zip(&result.fragments) {
        let mut plan = FragmentPlan {
            text: String::new(),
            bytes: vec![],
            glyphs: vec![],
        };
        for item in placed {
            let atom = atoms
                .get(item.ordinal)
                .ok_or_else(|| invalid("Missing placed glyph occurrence."))?;
            if atom.style_id != fragment.style_id || atom.text == "\n" {
                return Err(invalid("Placed glyph attribution changed."));
            }
            plan.text.push_str(&atom.text);
            plan.bytes.extend(&atom.encoded);
            plan.glyphs.push((atom.text.clone(), item.origin, atom.metric.advance));
        }
        if plan.text != fragment.text {
            return Err(invalid("Placed glyph text changed."));
        }
        let map = Arc::make_mut(styles[fragment.style_id].fragments.as_mut().unwrap());
        if map.insert(origin_key(fragment.origin), plan).is_some() {
            return Err(invalid("Ambiguous occurrence fragment origin."));
        }
    }
    Ok(())
}
