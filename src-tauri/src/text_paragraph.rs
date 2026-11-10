//! Uniform paragraph reflow. Selected strings become cursor-only advances;
//! the original resource supplies the replacement glyphs. Publish only after
//! reopening and checking surrounding objects and pixels.
use super::*;
use lopdf::{
    content::{Content, Operation},
    dictionary, Document, Object, Stream, StringFormat,
};
#[path = "paragraph_font.rs"]
mod paragraph_font;
#[path = "paragraph_layout.rs"]
mod paragraph_layout;
#[path = "paragraph_source_alignment.rs"]
mod paragraph_source_alignment;
#[path = "paragraph_memory.rs"]
mod paragraph_memory;
#[path = "paragraph_spans.rs"]
mod paragraph_spans;
#[path = "paragraph_draft.rs"]
mod paragraph_draft;
// Only source-bound or independently replayed highlights may pass the graphics
// gate. Other artwork retains its existing collision refusal.
#[allow(dead_code)]
#[path = "paragraph_highlights.rs"]
mod paragraph_highlights;
#[allow(dead_code)]
#[path = "paragraph_highlight_geometry.rs"]
mod paragraph_highlight_geometry;
#[path = "paragraph_highlight_cells.rs"]
mod paragraph_highlight_cells;
#[path = "paragraph_highlight_publish.rs"]
mod paragraph_highlight_publish;
#[path = "styled_paragraph_layout.rs"]
mod styled_paragraph_layout;
#[path = "styled_paragraph_plan.rs"]
mod styled_paragraph_plan;
#[path = "styled_paragraph_native_metrics.rs"]
mod styled_paragraph_native_metrics;
#[path = "styled_paragraph_area.rs"]
mod styled_paragraph_area;
#[path = "styled_paragraph_publish.rs"]
mod styled_paragraph_publish;
#[path = "styled_paragraph_splice.rs"]
mod styled_paragraph_splice;
#[path = "styled_paragraph_memory.rs"]
mod styled_paragraph_memory;
use paragraph_layout::wrap;

const LIMIT: usize = 16 * 1024 * 1024;

fn advance(run: &RunSnapshot, text: &str) -> EngineResult<f32> {
    if let Some(proof) = &run.proof {
        return proof.advance(text);
    }
    if let Some(proof) = &run.standard { return proof.advance(text); }
    if run.embedded || !text.bytes().all(|c| (32..=126).contains(&c)) {
        return Err(invalid("The paragraph has no verified font metrics."));
    }
    let font: TextFont = serde_json::from_value(serde_json::Value::String(run.font.clone()))
        .map_err(|_| invalid("Unsupported paragraph font."))?;
    persistence::text_advance(text, font)
}

fn decode(run: &RunSnapshot, bytes: &[u8]) -> EngineResult<String> {
    if let Some(proof) = &run.proof {
        return proof.decode(bytes);
    }
    if let Some(proof) = &run.standard { return proof.decode(bytes); }
    if !run.embedded && bytes.iter().all(|c| (32..=126).contains(c)) {
        return Ok(String::from_utf8(bytes.to_vec()).unwrap());
    }
    Err(invalid("The paragraph has no verified character encoding."))
}

#[derive(Clone)]
struct TextState {
    font: Option<(Vec<u8>, f32)>,
    character: f32,
    word: f32,
    scale: f32,
    rise: f32,
    mode: f32,
}
impl Default for TextState {
    fn default() -> Self { Self {font:None,character:0.,word:0.,scale:100.,rise:0.,mode:0.} }
}

// Defer this one admission failure only until a complete paragraph hint has been
// independently validated. Foreign whitespace-only runs remain unsupported.
fn whitespace_run(run: &RunSnapshot) -> bool {
    !run.text.is_empty() && run.text.len()<=4096 && run.text.bytes().all(|c|c==b' ')
        && run.mode==PdfPageTextRenderMode::FilledUnstroked && run.color[3]==255
        && if run.embedded {run.proof.as_ref().is_some_and(|p|p.supports(&run.text).is_ok())}
            else {base_font(&run.font)}
}

#[derive(Clone)]
struct BoundStyle { resource_id:lopdf::ObjectId, resource_name:Vec<u8>, run:RunSnapshot, object_indices:Vec<usize>, background_id:Option<usize> }
#[derive(Clone)]
struct Prepared {
    context:Option<text_context::ShowContext>,
    unit:f32,
    pdf: Document,
    page_id: lopdf::ObjectId,
    content: Content<Vec<Operation>>,
    selection: TextParagraphSelection,
    before: Vec<RunSnapshot>,
    font_resource: Vec<u8>,
    resources: lopdf::Dictionary,
    removals: Vec<(usize, f32)>,
    geometry: PageGeometry,
    glyphs: HashMap<usize, Vec<Glyph>>,
    source_text: HashMap<usize,String>,
    source_ranges: Vec<paragraph_spans::SourceRange>,
    draft_ranges: Option<Vec<paragraph_spans::SourceRange>>,
    consecutive: bool,
    remembered: bool,
    first_baseline: Option<f32>,
    native_ink: bool,
    text_alignment: bool,
    ink_vertical: bool,
    text_insets: [f32;2],
    deferred_whitespace: bool,
    reproducible_text_state: bool,
    styles:Vec<BoundStyle>,
    runs:Vec<StyledParagraphRun>,
    backgrounds:Vec<paragraph_highlight_geometry::Background>,
    highlight_sources:Vec<paragraph_highlights::BoundHighlight>,
}

fn number(value: &Object) -> EngineResult<f32> {
    value
        .as_float()
        .ok()
        .filter(|v| v.is_finite() && v.abs() <= 1_000_000.0)
        .ok_or_else(|| invalid("A paragraph text state value is invalid."))
}

// Public paragraph rectangles use the same displayed source axes as text targets
// and previews. Layout remains in raw PDF axes so /Rotate never changes the font
// orientation or which dimension determines wrapping.
fn displayed_bounds(geometry: PageGeometry, raw: [f32; 4]) -> AnnotationRect {
    let a = geometry.pdf_to_displayed(raw[0], raw[3]);
    let b = geometry.pdf_to_displayed(raw[2], raw[1]);
    // Quarter turns preserve axis alignment, but can reverse either diagonal.
    AnnotationRect {
        x: a.x.min(b.x),
        y: a.y.min(b.y),
        width: (b.x - a.x).abs(),
        height: (b.y - a.y).abs(),
    }
}

fn pdf_bounds(geometry: PageGeometry, displayed: &AnnotationRect) -> [f32; 4] {
    let (ax, ay) = geometry.displayed_to_pdf(Point { x: displayed.x, y: displayed.y });
    let (bx, by) = geometry.displayed_to_pdf(Point {
        x: displayed.x + displayed.width,
        y: displayed.y + displayed.height,
    });
    [ax.value.min(bx.value), ay.value.min(by.value),
     ax.value.max(bx.value), ay.value.max(by.value)]
}

// PDF content has no paragraph-alignment property. Estimate only from distinct
// row widths sharing an edge or center; equal-width/single rows are ambiguous.
fn alignment_extents(run:&RunSnapshot,glyphs:&[Glyph],size:f32)->Option<[f32;2]> {
    let visible:Vec<_>=glyphs.iter().filter(|g|!g.text.trim().is_empty()).collect();
    if visible.is_empty(){return None;}
    let mut left=run.matrix[4];let mut right=f32::NEG_INFINITY;
    for glyph in visible {
        let end=glyph.position[0]+advance(run,&glyph.text).ok()?*size;
        if !end.is_finite()||glyph.position.iter().any(|v|!v.is_finite()){return None;}
        left=left.min(glyph.position[2]);right=right.max(glyph.position[4]).max(end);
    }
    Some([left,right])
}
fn inferred_alignment(selected:&[&RunSnapshot],size:f32,glyphs:&HashMap<usize,Vec<Glyph>>,source_text:&HashMap<usize,String>)->Option<ParagraphAlignment> {
    if paragraph_layout::inferred_justification(selected,size,glyphs,source_text){return Some(ParagraphAlignment::Justify);}
    let mut rows:Vec<[f32;3]>=Vec::new(); // baseline, left origin/ink, right ink/advance
    for run in selected {
        if run.text.trim().is_empty(){continue;}
        let [left,right]=alignment_extents(run,glyphs.get(&run.index)?,size)?;
        if let Some(row)=rows.last_mut().filter(|row|(row[0]-run.matrix[5]).abs()<=EPSILON) {
            row[1]=row[1].min(left);row[2]=row[2].max(right);
        }else{rows.push([run.matrix[5],left,right]);}
    }
    if rows.len()<2{return None;}
    let spread=|values:Vec<f32>|values.iter().copied().fold(f32::NEG_INFINITY,f32::max)-values.iter().copied().fold(f32::INFINITY,f32::min);
    let tolerance=(size*0.08).clamp(0.1,1.5);
    if spread(rows.iter().map(|r|r[2]-r[1]).collect())<=tolerance*4.0{return None;}
    let candidates=[(ParagraphAlignment::Left,spread(rows.iter().map(|r|r[1]).collect())),
        (ParagraphAlignment::Center,spread(rows.iter().map(|r|(r[1]+r[2])*0.5).collect())),
        (ParagraphAlignment::Right,spread(rows.iter().map(|r|r[2]).collect()))];
    let matching:Vec<_>=candidates.into_iter().filter(|(_,delta)|*delta<=tolerance).collect();
    if matching.len()==1{Some(matching[0].0)}else{None}
}

fn inferred_line_spacing(selected: &[&RunSnapshot], size: f32) -> Option<f32> {
    let mut baselines: Vec<f32> = Vec::new();
    for run in selected.iter().filter(|run| !run.text.trim().is_empty()) {
        let baseline = run.matrix[5];
        if baselines.last().is_none_or(|last| (last - baseline).abs() > EPSILON) {
            baselines.push(baseline);
        }
    }
    if baselines.len() < 2 { return None; }
    let gaps: Vec<_> = baselines.windows(2).map(|row| row[0] - row[1]).collect();
    let minimum = gaps.iter().copied().fold(f32::INFINITY, f32::min);
    let maximum = gaps.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let tolerance = (size * 0.02).clamp(0.02, 0.25);
    if gaps.iter().any(|gap| !gap.is_finite()) || maximum - minimum > tolerance {
        return None;
    }
    let ratio = gaps.iter().sum::<f32>() / gaps.len() as f32 / size;
    if !(0.6-EPSILON/size..=3.0+EPSILON/size).contains(&ratio) { return None; }
    Some(ratio.clamp(0.6, 3.0))
}

fn prepare(
    bytes: &[u8],
    before: Vec<RunSnapshot>,
    indices: &[usize],
    geometry: PageGeometry,
    mixed: bool,
) -> EngineResult<Prepared> {
    let pdf = Document::load_mem_with_options(
        bytes,
        lopdf::LoadOptions::with_max_decompressed_size(LIMIT),
    )
    .map_err(|_| invalid("The paragraph page cannot be decoded."))?;
    let pages = pdf.get_pages();
    if pages.len() != 1 {
        return Err(invalid("Paragraph editing requires an isolated page."));
    }
    let page_id = *pages.values().next().unwrap();
    prepare_page(pdf,page_id,before,indices,geometry,mixed)
}

// Internal scratch probes use distinct pages to prevent native duplicate-text
// suppression. Source editing still enters through the isolated-page wrapper.
fn prepare_page(
    pdf: Document,
    page_id: lopdf::ObjectId,
    mut before: Vec<RunSnapshot>,
    indices: &[usize],
    geometry: PageGeometry,
    mixed: bool,
) -> EngineResult<Prepared> {
    let _ = source_text::bind_standard_on_page(&pdf, page_id, &mut before);
    let unit=physical_units::page_unit(&pdf,page_id)?;
    if indices.is_empty() || indices.len() > if mixed {4096}else{64} || indices.windows(2).any(|w| w[0] >= w[1]) {
        return Err(invalid(if mixed {"Select 1 to 4096 text pieces in reading order."}else{"Select 1 to 64 text pieces in reading order."}));
    }
    let selected: Vec<_> = indices
        .iter()
        .map(|i| {
            before
                .iter()
                .find(|r| r.index == *i)
                .ok_or_else(|| invalid("A selected text piece is missing."))
        })
        .collect::<EngineResult<_>>()?;
    let first = selected[0];
    let size = first.font_size * first.matrix[0];
    let source_values = if selected.iter().any(|run| run.reason.is_some()
        || run.proof.as_ref().is_some_and(|proof| proof.requires_source_operands)) {
        source_text::values_on_page(&pdf, page_id, &before).unwrap_or_default()
    } else { vec![] };
    let source_values: HashMap<_, _> = before.iter().zip(&source_values)
        .filter_map(|(run, value)| value.as_deref().map(|value| (run.index, value))).collect();
    let mut deferred_whitespace = false;
    for run in &selected {
        // Keep raw snapshots for selection, neighbors and replay. Only eligibility
        // uses independently decoded source characters rather than reader spaces.
        let source_run = source_edit_run(run, source_values.get(&run.index).copied());
        if let Some(reason) = &source_run.reason {
            if whitespace_run(run)||(mixed&&run.text.is_empty()&&run.mode==PdfPageTextRenderMode::FilledUnstroked&&run.color[3]==255
                &&if run.embedded {run.proof.is_some()}else{base_font(&run.font)}) { deferred_whitespace = true; }
            else { return Err(invalid(reason)); }
        }
        if !mixed && (run.font != first.font
            || run.program != first.program
            || run.proof != first.proof
            || run.standard != first.standard
            || run.color != first.color
            || !close_values(&run.matrix[..4], &first.matrix[..4])
            || (run.font_size - first.font_size).abs() > EPSILON)
        {
            return Err(invalid("Select text with the same font, size, and color. Mixed-format paragraphs are unchanged."));
        }
        if run.matrix[1].abs() > 0.0001
            || run.matrix[2].abs() > 0.0001
            || run.matrix[0] <= 0.0
            || (run.matrix[0] - run.matrix[3]).abs() > 0.0001
            || !physical_units::paragraph_size(run.font_size*run.matrix[0],unit)
        {
            return Err(invalid(
                "Paragraph editing requires horizontal text with uniform scaling.",
            ));
        }
    }
    let data = pdf
        .get_page_content_with_limit(page_id, LIMIT)
        .map_err(|_| invalid("The page content exceeds the paragraph limit."))?;
    let content = Content::decode(&data).map_err(|_| invalid("Malformed paragraph content."))?;
    if content.operations.len() > 100_000 {
        return Err(invalid("The page has too many operations."));
    }
    let context=selected_context(&content.operations,&before,indices)?;
    if !mixed && context.as_ref().is_some_and(|c|c.has_marked()) {
        return Err(invalid("Use styled paragraph editing to preserve marked text ownership."));
    }
    let (resources, _) = pdf
        .get_page_resources(page_id)
        .map_err(|_| invalid("Missing paragraph resources."))?;
    // Isolated PDFium page copies have a direct Resources dictionary. Resolve
    // inherited resources explicitly when necessary, without flattening them.
    let resources = if let Some(resources) = resources {
        resources.clone()
    } else {
        let mut node = page_id;
        let mut found = None;
        for _ in 0..64 {
            let dict = pdf
                .get_dictionary(node)
                .map_err(|_| invalid("Malformed page resources."))?;
            if let Ok(value) = dict.get(b"Resources") {
                found = Some(
                    pdf.dereference(value)
                        .and_then(|(_, v)| v.as_dict())
                        .map_err(|_| invalid("Malformed page resources."))?
                        .clone(),
                );
                break;
            }
            node = dict
                .get(b"Parent")
                .and_then(Object::as_reference)
                .map_err(|_| invalid("Missing page resources."))?;
        }
        found.ok_or_else(|| invalid("Page resource nesting exceeds the limit."))?
    };
    let fonts = pdf
        .dereference(
            resources
                .get(b"Font")
                .map_err(|_| invalid("Missing paragraph fonts."))?,
        )
        .and_then(|(_, v)| v.as_dict())
        .map_err(|_| invalid("Malformed paragraph fonts."))?;
    let mut state = TextState::default();
    let mut stack = Vec::new();
    let mut in_text = false;
    let mut ordinal = 0;
    let mut removals = Vec::new();
    let mut members = Vec::new();
    let mut reproducible_text_state = true;
    let mut resource = None;
    let mut styles:Vec<BoundStyle>=Vec::new();
    for (op_index, op) in content.operations.iter().enumerate() {
        match op.operator.as_str() {
            "q" => {
                if stack.len() >= 64 {
                    return Err(invalid("Graphics nesting exceeds the limit."));
                }
                stack.push(state.clone());
            }
            "Q" => {
                state = stack
                    .pop()
                    .ok_or_else(|| invalid("Unbalanced graphics state."))?
            }
            "BT" => {
                if in_text {
                    return Err(invalid("Nested text objects."));
                }
                in_text = true;
            }
            "ET" => {
                if !in_text {
                    return Err(invalid("Unbalanced text objects."));
                }
                in_text = false;
            }
            "Tf" => {
                if op.operands.len() != 2 {
                    return Err(invalid("Malformed paragraph font selection."));
                }
                state.font = Some((
                    op.operands[0]
                        .as_name()
                        .map_err(|_| invalid("Malformed font name."))?
                        .to_vec(),
                    number(&op.operands[1])?,
                ));
            }
            "Tc" | "Tw" => {
                if op.operands.len() != 1 {
                    return Err(invalid("Malformed paragraph spacing."));
                }
                let value = number(&op.operands[0])?;
                if op.operator == "Tc" {
                    state.character = value;
                } else {
                    state.word = value;
                }
            }
            "Tz" | "Ts" | "Tr" => {
                if op.operands.len()!=1 { return Err(invalid("Malformed paragraph text state.")); }
                let value=number(&op.operands[0])?;
                match op.operator.as_str(){"Tz"=>state.scale=value,"Ts"=>state.rise=value,_=>state.mode=value}
            }
            "Tj" | "TJ" | "'" | "\"" => {
                if !in_text {
                    return Err(invalid("Text outside a text object."));
                }
                if positioned_text::cursor_only(op) {
                    continue;
                }
                let run = before
                    .get(ordinal)
                    .ok_or_else(|| invalid("Ambiguous paragraph object mapping."))?;
                ordinal += 1;
                if op.operator == "\"" {
                    if op.operands.len() != 3 {
                        return Err(invalid("Malformed text operator."));
                    }
                    state.word = number(&op.operands[0])?;
                    state.character = number(&op.operands[1])?;
                }
                if !indices.contains(&run.index) {
                    continue;
                }
                reproducible_text_state &= state.character==0. && state.word==0.
                    && state.scale==100. && state.rise==0. && state.mode==0.;
                if matches!(op.operator.as_str(), "'" | "\"") {
                    return Err(invalid(
                        "This paragraph uses unsupported implicit line positioning.",
                    ));
                }
                let (name, raw_size) = state
                    .font
                    .as_ref()
                    .ok_or_else(|| invalid("Missing paragraph font."))?;
                if *raw_size <= 0.0 || (*raw_size - run.font_size).abs() > EPSILON {
                    return Err(invalid("The paragraph font size could not be verified."));
                }
                let font_object = fonts
                    .get(name)
                    .map_err(|_| invalid("Missing font resource."))?;
                let (font_id, font) = pdf
                    .dereference(font_object)
                    .map_err(|_| invalid("Malformed font resource."))?;
                let font = font
                    .as_dict()
                    .map_err(|_| invalid("Malformed font resource."))?;
                let identity = font_id
                    .ok_or_else(|| invalid("Direct paragraph font resources are not supported."))?;
                if !mixed && resource.as_ref().is_some_and(|(id, _)| *id != identity) {
                    return Err(invalid("Select text with the same font, size, and color. Mixed-format paragraphs are unchanged."));
                }
                resource = Some((identity, name.clone()));
                if !embedded_font::matches_name(&pdf, font, &run.font, run.embedded) {
                    return Err(invalid(
                        "The paragraph font resource could not be verified.",
                    ));
                }
                // Standard fonts with explicit nonstandard widths are not covered
                // by the built-in metrics; embedded widths have already been proven.
                if !run.embedded && font.has(b"Widths") {
                    return Err(invalid(
                        "Explicit widths on standard paragraph fonts are not supported.",
                    ));
                }
                let values: Vec<&Object> = match (op.operator.as_str(), op.operands.as_slice()) {
                    ("Tj", [value]) => vec![value],
                    ("TJ", [Object::Array(values)]) => values.iter().collect(),
                    _ => return Err(invalid("Malformed paragraph text operator.")),
                };
                let mut logical = String::new();
                let mut distance = 0.0;
                for value in values {
                    if let Ok(bytes) = value.as_str() {
                        let text = decode(run, bytes)?;
                        distance += advance(run, &text)? * raw_size
                            + text.chars().count() as f32 * state.character;
                        if run.proof.as_ref().is_none_or(|p| p.single_byte()) {
                            distance +=
                                bytes.iter().filter(|c| **c == 32).count() as f32 * state.word;
                        }
                        logical.push_str(&text);
                    } else {
                        distance -= number(value)? * raw_size / 1000.0;
                    }
                }
                if paragraph_layout::extraction_text(&logical) != paragraph_layout::extraction_text(&run.text) {
                    return Err(invalid("The selected characters could not be mapped exactly to the paragraph stream."));
                }
                if mixed&&run.text.is_empty()&&(!logical.bytes().all(|b|b==b' ')||logical.is_empty()) {return Err(invalid("An empty extraction is not a verified space fragment."));}
                if !distance.is_finite() {
                    return Err(invalid("The paragraph advance exceeds its limit."));
                }
                removals.push((op_index, -distance * 1000.0 / raw_size));
                let style_id=styles.iter().position(|style|style.resource_id==identity
                    && style.run.font==run.font && style.run.program==run.program && style.run.proof==run.proof
                    && style.run.standard==run.standard
                    && style.run.color==run.color
                    && (style.run.font_size*style.run.matrix[0]).to_bits()==(run.font_size*run.matrix[0]).to_bits())
                    .unwrap_or_else(||{styles.push(BoundStyle{resource_id:identity,resource_name:name.clone(),run:run.clone(),object_indices:vec![],background_id:None});styles.len()-1});
                if styles.len()>64{return Err(invalid("Use at most 64 paragraph styles."));}
                styles[style_id].object_indices.push(run.index);
                members.push((run, logical,style_id));
            }
            _ => {}
        }
    }
    if ordinal != before.len() || in_text || !stack.is_empty() || members.len() != indices.len() {
        return Err(invalid(
            "Text operators cannot be mapped uniquely to the selected paragraph.",
        ));
    }
    let mut logical = String::new();
    let mut attributed=paragraph_spans::Builder::default();
    let mut previous_style=0;
    // A row's largest font may occur after its first piece. Complete each
    // contiguous row before judging spacing to either neighboring row.
    let mut row_sizes=vec![size;members.len()];
    if mixed {
        let mut start=0;
        while start<members.len() {
            let mut end=start+1;
            let mut maximum=members[start].0.font_size*members[start].0.matrix[0];
            while end<members.len() && (members[end-1].0.matrix[5]-members[end].0.matrix[5]).abs()<=EPSILON {
                maximum=maximum.max(members[end].0.font_size*members[end].0.matrix[0]);
                end+=1;
            }
            row_sizes[start..end].fill(maximum);
            start=end;
        }
    }
    let mut previous: Option<&RunSnapshot> = None;
    let mut consecutive = true;
    for (member_index,(run, text,style_id)) in members.iter().enumerate() {
        if let Some(prev) = previous {
            let dy = prev.matrix[5] - run.matrix[5];
            if dy.abs() <= EPSILON {
                if run.matrix[4] < prev.matrix[4] {
                    return Err(invalid("Select paragraph pieces in reading order."));
                }
                let gap_size=if mixed {(prev.font_size*prev.matrix[0]).max(run.font_size*run.matrix[0])} else {size};
                if run.bounds[0] - prev.bounds[2] > gap_size * 0.2
                    && !logical.ends_with(' ')
                    && !text.starts_with(' ')
                {
                    logical.push(' ');
                    attributed.push(" ",previous_style);
                }
            } else {
                // Saved f32 origins can round an exact boundary to either side.
                let spacing_size=if mixed {row_sizes[member_index-1].max(row_sizes[member_index])} else {size};
                if dy + EPSILON < spacing_size * 0.6 || dy - EPSILON > spacing_size * 3.0 {
                    consecutive = false;
                }
                while logical.ends_with(' ') {
                    logical.pop();
                }
                attributed.trim_end_spaces();
                logical.push(' ');
                attributed.push(" ",previous_style);
            }
        }
        logical.push_str(text);
        attributed.push_source(text,*style_id,run.index);
        previous_style=*style_id;
        previous = Some(run);
    }
    logical = logical.trim().to_string();
    let paragraph_spans::Assembly{runs,sources:source_ranges}=attributed.finish();
    if runs.iter().map(|r|r.text.as_str()).collect::<String>()!=logical {return Err(invalid("Paragraph formatting could not be mapped to its text."));}
    if logical.is_empty() || logical.len() > 4096 {
        return Err(invalid("Select a paragraph within 4096 UTF-8 bytes."));
    }
    let left = selected
        .iter()
        .map(|r| r.matrix[4].min(r.bounds[0]))
        .fold(f32::INFINITY, f32::min);
    let right = selected
        .iter()
        .map(|r| r.bounds[2])
        .fold(f32::NEG_INFINITY, f32::max);
    let top = selected
        .iter()
        .map(|r| r.matrix[5] + if mixed {r.font_size*r.matrix[0]} else {size})
        .fold(f32::NEG_INFINITY, f32::max);
    let bottom = selected
        .iter()
        .map(|r| r.matrix[5] - (if mixed {r.font_size*r.matrix[0]} else {size}) * 0.25)
        .fold(f32::INFINITY, f32::min);
    let selection = TextParagraphSelection {
        object_indices: indices.to_vec(),
        text: logical,
        font_name: first.font.clone(),
        font_display_name: first.proof.as_ref().and_then(|p|p.display_name.clone()),
        font_size: size,
        color: Some(format!("#{:02X}{:02X}{:02X}",first.color[0],first.color[1],first.color[2])),
        alignment: None, // Filled from glyph origins and verified advances below.
        line_spacing: if mixed{paragraph_source_alignment::line_spacing(&selected,unit)}else{inferred_line_spacing(&selected,size)},
        bounds: displayed_bounds(geometry, [
            left,
            top.min(geometry.top) - (top - bottom),
            left + (right - left + size * 0.1).max(size),
            top.min(geometry.top),
        ]),
    };
    // An omitted piece inside the selected span could be silently interleaved.
    for run in &before {
        if !indices.contains(&run.index)
            && !run.text.trim().is_empty()
            && run.index > indices[0]
            && run.index < *indices.last().unwrap()
        {
            return Err(invalid(
                "Include every text piece between the first and last selected pieces.",
            ));
        }
    }
    let source_text=members.iter().map(|(run,text,_)|(run.index,text.clone())).collect();
    Ok(Prepared {
        context,
        unit,
        pdf,
        page_id,
        content,
        selection,
        before,
        font_resource: resource.unwrap().1,
        resources,
        removals,
        geometry,
        glyphs: HashMap::new(),
        source_text,
        source_ranges,
        draft_ranges: None,
        consecutive,
        remembered: false,
        first_baseline: None,
        native_ink: false,
        text_alignment: true,
        ink_vertical: true,
        text_insets: [0.;2],
        deferred_whitespace,
        reproducible_text_state,
        styles,runs,
        backgrounds:vec![],highlight_sources:vec![],
    })
}

// Measure ink separately from advance: j and italic letters can extend left
// of their origin or right of their advance. Probes are unattached objects on
// the private copy; the encoded candidate is still independently reopened.
fn ink_metrics(
    document: &PdfDocument<'_>,
    page: &PdfPage<'_>,
    run: &RunSnapshot,
    text: &str,
    size: f32,
) -> EngineResult<HashMap<char, [f32; 3]>> {
    if text.len() > 4096 {
        return Err(invalid("Use at most 4096 UTF-8 bytes for a paragraph."));
    }
    let original = page.objects().get(run.index)?;
    let token = original
        .as_text_object()
        .ok_or_else(|| invalid("Missing paragraph font."))?
        .font()
        .token();
    let mut metrics = HashMap::new();
    for c in text.chars().filter(|c| *c != '\n') {
        if metrics.contains_key(&c) {
            continue;
        }
        let value = c.to_string();
        let width = advance(run, &value)? * size;
        let probe = PdfPageTextObject::new(document, &value, token, PdfPoints::new(size))?;
        let rect = probe.bounds()?;
        let metric = [width, rect.left().value, rect.right().value];
        if metric.iter().any(|v| !v.is_finite() || v.abs() > 100_000.0) {
            return Err(invalid("The paragraph glyph metrics exceed the limit."));
        }
        metrics.insert(c, metric);
    }
    Ok(metrics)
}

pub(super) fn object_glyphs(page: &PdfPage<'_>, index: usize) -> EngineResult<Vec<Glyph>> {
    let text_page=page.text()?;
    object_glyphs_with_text(page,index,&text_page)
}

fn object_glyphs_with_text(page:&PdfPage<'_>,index:usize,text_page:&PdfPageText<'_>)->EngineResult<Vec<Glyph>> {
    let object = page.objects().get(index)?;
    let text = object
        .as_text_object()
        .ok_or_else(|| invalid("A paragraph text object is missing."))?;
    let mut result = Vec::new();
    for c in text_page.chars_for_object(text)?.iter() {
        if c.is_generated()? {
            continue;
        }
        let rect = c.tight_bounds()?;
        let (x, y) = c.origin()?;
        result.push(Glyph {
            text: c.unicode_string().unwrap_or_default(),
            position: [
                x.value,
                y.value,
                rect.left().value,
                rect.bottom().value,
                rect.right().value,
                rect.top().value,
            ],
        });
    }
    Ok(result)
}

// chars_for_object() scans the whole text page for every requested object.
// Instead associate each character's native owner with a unique page-object
// key. A key collision is NOT guessed: use the exact native owner scan for all
// objects in that group. Font token, matrix and size are native owner properties,
// never inferred from character coordinates or saved paragraph metadata.
pub(super) fn page_object_glyphs(page:&PdfPage<'_>)->EngineResult<HashMap<usize,Vec<Glyph>>>{
    fn key(object:&PdfPageTextObject<'_>)->EngineResult<(PdfFontToken,[u32;6],u32)>{
        let m=object.matrix()?;Ok((object.font().token(),[m.a(),m.b(),m.c(),m.d(),m.e(),m.f()].map(f32::to_bits),object.unscaled_font_size().value.to_bits()))
    }
    let text_page=page.text()?;let mut groups=HashMap::<_,Vec<usize>>::new();let mut result=HashMap::new();
    // Nested form objects can share local font/matrix properties with root text.
    // Preserve exact native ownership scans for such pages rather than treating
    // local properties as a cross-container identity.
    if page.objects().iter().any(|o|o.as_x_object_form_object().is_some()){
        for (index,object) in page.objects().iter().enumerate(){if object.as_text_object().is_some(){result.insert(index,object_glyphs_with_text(page,index,&text_page)?);}}
        return Ok(result);
    }
    for (index,object) in page.objects().iter().enumerate(){if let Some(text)=object.as_text_object(){groups.entry(key(text)?).or_default().push(index);result.insert(index,Vec::new());}}
    for c in text_page.chars().iter(){
        if c.is_generated()?{continue;}let Ok(owner)=c.text_object()else{continue;};
        let Some(ids)=groups.get(&key(&owner)?)else{continue;};if ids.len()!=1{continue;}
        let rect=c.tight_bounds()?;let (x,y)=c.origin()?;
        result.get_mut(&ids[0]).unwrap().push(Glyph{text:c.unicode_string().unwrap_or_default(),position:[x.value,y.value,rect.left().value,rect.bottom().value,rect.right().value,rect.top().value]});
    }
    for ids in groups.values().filter(|ids|ids.len()>1){for id in ids{result.insert(*id,object_glyphs_with_text(page,*id,&text_page)?);}}
    Ok(result)
}

fn selected_context(operations:&[Operation],before:&[RunSnapshot],indices:&[usize])->EngineResult<Option<text_context::ShowContext>> {
    if !operations.iter().any(|op|matches!(op.operator.as_str(),"W"|"W*"|"BMC"|"BDC")) {return Ok(None);}
    let page=text_context::inspect(operations)?;
    if page.shows.len()!=before.len() {return Err(invalid("The paragraph context cannot be matched to native text."));}
    let contexts=before.iter().zip(page.shows).filter(|(run,_)|indices.contains(&run.index)).map(|(_,(_,ctx))|ctx).collect::<Vec<_>>();
    let first=contexts.first().ok_or_else(||invalid("Missing paragraph text context."))?;let binding=first.binding()?;
    for context in &contexts {if context.binding()?!=binding {return Err(invalid("Select text in one clipping and marked-content scope."));}}
    Ok(first.requires_binding().then(||first.clone()))
}
impl Prepared {
    fn remove_uniform_context_envelope(&mut self)->EngineResult<bool> {
        let Some(context)=self.context.as_ref().filter(|c|self.remembered&&c.has_clip())else{return Ok(false);};
        let bad=||invalid("The saved uniform paragraph clip envelope is not independently owned.");
        let first=self.removals.first().ok_or_else(bad)?.0;let last=self.removals.last().unwrap().0;
        let start=first.checked_sub(10).ok_or_else(bad)?;let end=last+3;
        let ops=&self.content.operations;
        let header=ops.get(start..first).ok_or_else(bad)?;
        if !header.iter().zip(["q","BT","Tf","Tc","Tw","Tz","Ts","Tr","rg","Tm"]).all(|(op,name)|op.operator==name)
            ||self.removals.iter().enumerate().any(|(i,(index,_))|*index!=first+i*2)
            ||!ops.get(last+1..end).is_some_and(|tail|tail[0].operator=="ET"&&tail[1].operator=="Q"&&tail.iter().all(|op|op.operands.is_empty())) {
            return Err(bad());
        }
        for pair in ops[first+1..last+1].chunks_exact(2) {if pair[0].operator!="Tm"||!matches!(pair[1].operator.as_str(),"Tj"|"TJ") {return Err(bad());}}
        let outer=context.owned_clip_prefix_start(ops,start)?;
        if !ops.get(end).is_some_and(|op|op.operator=="Q"&&op.operands.is_empty()) {return Err(bad());}
        self.content.operations.drain(outer..=end);Ok(true)
    }

    fn context_binding(&self)->EngineResult<Option<String>> {self.context.as_ref().map(|c|c.binding()).transpose()}
    fn check_context_area(&self,bounds:[f32;4])->EngineResult<()> {
        if self.context.as_ref().is_some_and(|c|!c.contains(bounds.map(f64::from))) {return Err(invalid("Keep the paragraph area inside its original clipping path."));}Ok(())
    }
}

impl WorkerRuntime {
    pub(in crate::engine) fn suggest_text_paragraph(
        &self, source_id: &str, page_index: usize, object_index: usize, expected_text: &str,
    ) -> EngineResult<TextParagraphSelection> {
        if let Some(reason)=self.source_text_edit_restriction(source_id)? {return Err(invalid(&reason));}
        let (indices, candidates)={
            let copy=self.editable_page_copy(source_id,page_index)?;
            let bytes=copy.save_to_bytes()?;
            let proofs=FontProofs::read(&bytes)?;
            let context=self.paragraph_contexts.get(source_id).map(|c|c.paragraph(&[object_index],false)).transpose()?;
            preflight_with_context(&bytes,&proofs,false,context.as_ref())?;
            let page=copy.pages().get(0)?;
            let before=bound_snapshot(&page,Some(&proofs),&bytes)?;
            let mut extents=HashMap::new();
            for run in before.iter().filter(|run|run.reason.is_none()&&!run.text.trim().is_empty()) {
                if let Some(bounds)=alignment_extents(run,&object_glyphs(&page,run.index)?,run.font_size*run.matrix[0]){extents.insert(run.index,bounds);}
            }
            let candidates = before.iter().find(|run|run.index==object_index)
                .filter(|run|run.text==expected_text)
                .map(|_|paragraph_memory::candidates(&bytes,&before,object_index)).unwrap_or_default();
            (paragraph_selection::suggest_mode(&before,object_index,expected_text,&extents,false,self.source_page_unit(source_id,page_index)?), candidates)
        };
        for candidate in candidates {
            if let Ok((prepared, _)) = self.paragraph_baseline(source_id,page_index,&candidate) {
                if prepared.remembered { return Ok(prepared.selection); }
            }
        }
        self.inspect_text_paragraph(source_id,page_index,&indices?)
    }

    fn paragraph_baseline(
        &self,
        source_id: &str,
        page_index: usize,
        indices: &[usize],
    ) -> EngineResult<(Prepared, image::RgbaImage)> {
        self.paragraph_baseline_mode(source_id,page_index,indices,false)
    }
    fn paragraph_baseline_mode(
        &self,source_id:&str,page_index:usize,indices:&[usize],mixed:bool,
    )->EngineResult<(Prepared,image::RgbaImage)> {
        self.paragraph_baseline_with_mapping(source_id,page_index,indices,mixed,false)
    }
    fn paragraph_baseline_with_mapping(
        &self,source_id:&str,page_index:usize,indices:&[usize],mixed:bool,capture:bool,
    )->EngineResult<(Prepared,image::RgbaImage)> {
        if let Some(reason) = self.source_text_edit_restriction(source_id)? {
            return Err(invalid(&reason));
        }
        let copy = self.editable_page_copy(source_id, page_index)?;
        let bytes = copy.save_to_bytes()?;
        let proofs = FontProofs::read(&bytes)?;
        let context=self.paragraph_contexts.get(source_id).map(|c|c.paragraph(indices,mixed)).transpose()?;
        preflight_with_context(&bytes, &proofs, false,context.as_ref())?;
        let page = copy.pages().get(0)?;
        let geometry = page_geometry(&page)?;
        let before = bound_snapshot(&page, Some(&proofs), &bytes)?;
        if let Some(proof)=&context {proof.baseline(&bytes,&before)?;}
        let mut prepared = prepare(&bytes, before, indices, geometry,mixed)?;
        if capture{prepared.draft_ranges=Some(vec![]);}
        if context.is_none() && prepared.content.operations.iter().any(|op|matches!(op.operator.as_str(),"BMC"|"BDC")) {
            return Err(invalid("Marked paragraphs require original document ownership."));
        }
        prepared.glyphs=page_object_glyphs(&page)?;
        let selected:Vec<_>=prepared.before.iter().filter(|run|indices.contains(&run.index)).collect();
        let fresh_alignment=if mixed{paragraph_source_alignment::infer(&selected,&prepared.glyphs,prepared.unit)}else{None};
        if let Some(proposal)=fresh_alignment{prepared.selection.alignment=Some(proposal.alignment);}
        if !mixed {prepared.selection.alignment=inferred_alignment(&selected,prepared.selection.font_size,&prepared.glyphs,&prepared.source_text);}
        // Re-estimate the horizontal edit area with the same metrics. Retained
        // trailing spaces must not enlarge it or move a right-aligned re-edit.
        let extents:Option<Vec<_>>=selected.iter().filter(|run|!run.text.trim().is_empty()).map(|run|alignment_extents(run,&prepared.glyphs[&run.index],if mixed {run.font_size*run.matrix[0]} else {prepared.selection.font_size})).collect();
        if let Some(extents)=extents.filter(|values|!values.is_empty()) {
            let left=extents.iter().map(|v|v[0]).fold(f32::INFINITY,f32::min);
            let right=extents.iter().map(|v|v[1]).fold(f32::NEG_INFINITY,f32::max);
            let mut bounds=pdf_bounds(geometry,&prepared.selection.bounds);
            bounds[0]=left;bounds[2]=left+(right-left+prepared.selection.font_size*0.1).max(prepared.selection.font_size);
            if let Some(proposal)=fresh_alignment{
                let width=bounds[2]-bounds[0];
                bounds[0]=match proposal.alignment{ParagraphAlignment::Center=>proposal.anchor-width*0.5,ParagraphAlignment::Right=>proposal.anchor-width,_=>proposal.anchor};
                bounds[2]=bounds[0]+width;
            }
            prepared.selection.bounds=displayed_bounds(geometry,bounds);
        }
        let mixed_remembered=mixed&&self.restore_styled_memory(&mut prepared,&page).unwrap_or(false);
        let remembered=if mixed {None} else if capture{paragraph_memory::restore_with_mapping(&prepared, &copy, &page,true)}
            else{paragraph_memory::restore(&prepared,&copy,&page).map(|selection|(selection,vec![]))};
        if let Some((selection,ranges)) = remembered {
            prepared.selection = selection;
            if capture{prepared.draft_ranges=Some(ranges);}
            prepared.remembered = true;
        } else if mixed_remembered {
            prepared.remembered=true;
        } else if mixed&&indices.len()>64 {
            return Err(invalid("Select 1 to 64 text pieces, or a verified complete saved paragraph."));
        } else if !prepared.consecutive {
            return Err(invalid("Select consecutive lines from one paragraph."));
        } else if prepared.deferred_whitespace {
            return Err(invalid("Whitespace-only text pieces require a verified complete paragraph."));
        }
        // Enrich only completely proven fresh selections. Unsupported graphics
        // retain the existing inspection palette and publication refusal. This
        // optional atomic step must not prevent font substitution or inspection
        // on a page whose other artwork is outside the highlight policy.
        if mixed&&!prepared.remembered&&page.objects().iter().any(|o|o.as_path_object().is_some()) {
            let _=paragraph_highlights::install_source_backgrounds(&mut prepared,&page);
        }
        let image = raster(&page)?;
        if image
            != raster(
                &self
                    .editing_document(source_id)?
                    .pdf()?
                    .pages()
                    .get(page_index_i32(page_index)?)?,
            )?
        {
            return Err(invalid("Copying this page would change its appearance."));
        }
        Ok((prepared, image))
    }

    pub(in crate::engine) fn inspect_text_paragraph(
        &self,
        source_id: &str,
        page_index: usize,
        indices: &[usize],
    ) -> EngineResult<TextParagraphSelection> {
        self.paragraph_baseline(source_id, page_index, indices)
            .map(|(p, _)| p.selection)
    }

    pub(in crate::engine) fn inspect_styled_paragraph(&self,source_id:&str,page_index:usize,indices:&[usize])->EngineResult<StyledParagraphSelection>{
        let (mut prepared,_)=self.paragraph_baseline_mode(source_id,page_index,indices,true)?;
        // A best-effort suggestion must not prevent inspection/font substitution
        // when existing source glyphs cannot yet be measured for reflow.
        if !prepared.remembered {
            if let Ok(Some(bounds))=self.initial_styled_area(source_id,page_index,&prepared){prepared.selection.bounds=bounds;}
        }
        Ok(Self::styled_selection(prepared))
    }

    pub(in crate::engine) fn suggest_styled_paragraph(&self,source_id:&str,page_index:usize,anchor:usize,expected_text:&str)->EngineResult<StyledParagraphSelection>{
        if let Some(reason)=self.source_text_edit_restriction(source_id)?{return Err(invalid(&reason));}
        let fresh={
            let copy=self.editable_page_copy(source_id,page_index)?;let bytes=copy.save_to_bytes()?;
            let proofs=FontProofs::read(&bytes)?;let page=copy.pages().get(0)?;let before=bound_snapshot(&page,Some(&proofs),&bytes)?;
            if !before.iter().any(|run|run.index==anchor&&run.text==expected_text){return Err(invalid("The selected text changed. Select it again."));}
            let candidate=styled_paragraph_memory::candidate(&bytes,&before,anchor);
            // A verified saved paragraph avoids fresh glyph/geometry work and
            // retains its authored membership even when it exceeds 64 pieces.
            if let Some(indices)=candidate{
                if let Ok((prepared,_))=self.paragraph_baseline_mode(source_id,page_index,&indices,true){
                    if prepared.remembered{return Ok(Self::styled_selection(prepared));}
                }
            }
            let fresh=if before.len()<=10_000 {
                let glyphs=page_object_glyphs(&page)?;
                let extents=before.iter().filter(|run|run.reason.is_none()&&!run.text.trim().is_empty()).filter_map(|run|
                    glyphs.get(&run.index).and_then(|g|alignment_extents(run,g,run.font_size*run.matrix[0])).map(|bounds|(run.index,bounds))).collect();
                paragraph_selection::suggest_mode(&before,anchor,expected_text,&extents,true,self.source_page_unit(source_id,page_index)?).ok()
            }else{None};
            fresh
        };
        // Geometry proposes membership only. Complete native inspection must
        // validate its reading order, resources, source ownership and appearance.
        if let Some(indices)=fresh.filter(|ids|ids.len()>1){
            if let Ok(selection)=self.inspect_styled_paragraph(source_id,page_index,&indices){return Ok(selection);}
        }
        // Ambiguous/unsupported groups retain the manual clicked-piece path.
        self.inspect_styled_paragraph(source_id,page_index,&[anchor])
    }

    fn styled_selection(prepared:Prepared)->StyledParagraphSelection{
        let baseline_anchor=prepared.baseline_anchor();
        let styles=prepared.styles.iter().enumerate().map(|(style_id,style)|StyledParagraphStyle{
            style_id,object_indices:style.object_indices.clone(),font_name:style.run.font.clone(),
            font_display_name:style.run.proof.as_ref().and_then(|p|p.display_name.clone()),
            font_size:style.run.font_size*style.run.matrix[0],
            color:format!("#{:02X}{:02X}{:02X}",style.run.color[0],style.run.color[1],style.run.color[2]),
            is_embedded:style.run.embedded,
            background_color:style.background_id.and_then(|id|prepared.backgrounds.get(id)).map(|background|format!("#{:02X}{:02X}{:02X}",background.color[0],background.color[1],background.color[2])),
        }).collect();
        StyledParagraphSelection{baseline_anchor,object_indices:prepared.selection.object_indices,text:prepared.selection.text,
            runs:prepared.runs,styles,bounds:prepared.selection.bounds,alignment:prepared.selection.alignment,line_spacing:prepared.selection.line_spacing}
    }

    pub(in crate::engine) fn preview_text_paragraph(
        &mut self,
        source_id: &str,
        page_index: usize,
        indices: &[usize],
        expected: &str,
        replacement: &str,
        bounds: &AnnotationRect,
        formatting: &ParagraphFormatting,
    ) -> EngineResult<DocumentInfo> {
        let unit=self.source_page_unit(source_id,page_index)?;
        if formatting.font_size.is_some_and(|size|!physical_units::paragraph_size(size,unit)) {
            return Err(invalid("Use a font size from 1 to 300 pt."));
        }
        if formatting.line_spacing.is_some_and(|spacing|!spacing.is_finite()||!(0.6..=3.0).contains(&spacing)) {
            return Err(invalid("Use line spacing from 0.6 to 3 times the font size."));
        }
        let requested_color = formatting.color.as_deref().map(parse_color).transpose()?
            .map(|rgb|[rgb.red(),rgb.green(),rgb.blue(),255]);
        let (mut prepared, original) = self.paragraph_baseline(source_id, page_index, indices)?;
        if expected != prepared.selection.text {
            return Err(invalid(
                "The text changed since it was selected. Check the selection again.",
            ));
        }
        let geometry = prepared.geometry;
        let page_size = geometry.displayed_size();
        if ![bounds.x, bounds.y, bounds.width, bounds.height]
            .iter()
            .all(|v| v.is_finite())
            || bounds.x < 0.0
            || bounds.y < 0.0
            || bounds.width <= 0.0
            || bounds.height <= 0.0
            || bounds.x + bounds.width > page_size.width
            || bounds.y + bounds.height > page_size.height
        {
            return Err(invalid("Keep the paragraph area inside the page."));
        }
        let first_owned = prepared
            .before
            .iter()
            .find(|r| r.index == indices[0])
            .unwrap().clone();
        let first=&first_owned;
        let size = formatting.font_size.unwrap_or(prepared.selection.font_size);
        let color = requested_color.unwrap_or(first.color);
        let alignment = formatting.alignment.or(prepared.selection.alignment).unwrap_or(ParagraphAlignment::Left);
        let line_spacing = formatting.line_spacing.or(prepared.selection.line_spacing).unwrap_or(1.2);
        let substitute=formatting.font_id.as_deref().map(|id|self.fonts.get(id)).transpose()?;
        let copy = self.editable_page_copy(source_id, page_index)?;
        let page = copy.pages().get(0)?;
        let metrics = if let Some(asset)=&substitute {paragraph_font::metrics(asset,replacement,size)?}
            else {ink_metrics(&copy, &page, first, replacement, size)?};
        let new_bounds = pdf_bounds(geometry, bounds);
        prepared.check_context_area(new_bounds)?;
        let lines = wrap(first, substitute.is_some(), replacement, new_bounds[2] - new_bounds[0], &metrics)?;
        if size * 1.25 + (lines.len() - 1) as f32 * size * line_spacing > new_bounds[3] - new_bounds[1] + EPSILON {
            return Err(invalid("The text does not fit in the paragraph area. Enlarge the area or shorten the text."));
        }
        let mut changed_area = new_bounds;
        for run in prepared
            .before
            .iter()
            .filter(|r| indices.contains(&r.index))
        {
            changed_area[0] = changed_area[0].min(run.bounds[0]);
            changed_area[1] = changed_area[1].min(run.bounds[1]);
            changed_area[2] = changed_area[2].max(run.bounds[2]);
            changed_area[3] = changed_area[3].max(run.bounds[3]);
        }
        for run in &prepared.before {
            if !indices.contains(&run.index)
                && !run.text.trim().is_empty()
                && adds_overlap(&[0.0; 4], &new_bounds, &run.bounds)
            {
                return Err(invalid(
                    "The paragraph area would overlap neighboring text.",
                ));
            }
        }
        for (index, object) in page.objects().iter().enumerate() {
            if object.as_text_object().is_some() {
                continue;
            }
            // This legacy emitter appends replacement shows. Unlike the styled
            // route, it cannot yet preserve reading order inside a callout;
            // retain whole-object refusal until source-position splicing exists.
            let rect = object.bounds()?;
            let other = [
                rect.left().value,
                rect.bottom().value,
                rect.right().value,
                rect.top().value,
            ];
            if !(index < indices[0] && solid_page_background(&object, geometry)?)
                && adds_overlap(&[0.0; 4], &changed_area, &other)
            {
                return Err(invalid("The paragraph overlaps graphics or highlighting. That selection is not supported yet."));
            }
        }
        drop(page);
        drop(copy);
        if let Some(asset)=&substitute {
            prepared.font_resource=paragraph_font::install(&mut prepared.pdf,prepared.page_id,&prepared.resources,asset)?;
        }
        let saved_context=prepared.context_binding()?;
        if !prepared.remove_uniform_context_envelope()? {
            for &(index, distance) in &prepared.removals {
                prepared.content.operations[index] =
                    Operation::new("TJ", vec![Object::Array(vec![Object::Real(distance)])]);
            }
        }
        // Isolate pre-existing root graphics state once. Our own output already
        // consists entirely of balanced q/Q scopes, so later edits need no extra
        // wrapper (which would otherwise hit the nesting limit after 64 edits).
        let mut depth = 0usize;
        let mut needs_scope = false;
        for op in &prepared.content.operations {
            match op.operator.as_str() {
                "q" => depth += 1,
                "Q" => depth -= 1, // balanced by prepare()
                _ => needs_scope |= depth == 0,
            }
        }
        let mut operations = Vec::new();
        if needs_scope {
            operations.push(Operation::new("q", vec![]));
        }
        operations.extend(prepared.content.operations);
        if needs_scope {
            operations.push(Operation::new("Q", vec![]));
        }
        let clip=prepared.context.as_ref().filter(|c|c.has_clip());
        if let Some(clip)=clip {operations.extend(clip.clip_prefix()?);}
        operations.extend([
            Operation::new("q", vec![]),
            Operation::new("BT", vec![]),
            Operation::new(
                "Tf",
                vec![Object::Name(prepared.font_resource), size.into()],
            ),
            Operation::new("Tc", vec![0.into()]),
            Operation::new("Tw", vec![0.into()]),
            Operation::new("Tz", vec![100.into()]),
            Operation::new("Ts", vec![0.into()]),
            Operation::new("Tr", vec![0.into()]),
            Operation::new(
                "rg",
                color[..3]
                    .iter()
                    .map(|c| Object::Real(*c as f32 / 255.0))
                    .collect(),
            ),
        ]);
        let mut expected_lines = Vec::new();
        for (line_index, wrapped) in lines.iter().enumerate() {
            let line=&wrapped.text;
            if line.is_empty() {
                continue;
            }
            let (origin, show) = paragraph_layout::positioned_line(first, substitute.is_some(), wrapped, line_index, new_bounds, size, alignment, line_spacing, &metrics)?;
            operations.push(Operation::new(
                "Tm",
                vec![
                    1.into(),
                    0.into(),
                    0.into(),
                    1.into(),
                    origin[0].into(),
                    origin[1].into(),
                ],
            ));
            operations.push(show);
            expected_lines.push((line,origin));
        }
        operations.extend([Operation::new("ET", vec![]), Operation::new("Q", vec![])]);
        if clip.is_some() {operations.push(Operation::new("Q",vec![]));}
        if operations.len()>100_000 {return Err(invalid("The edited page exceeds its operation limit."));}
        let data = Content { operations }
            .encode()
            .map_err(|_| invalid("The paragraph could not be encoded."))?;
        if data.len() > LIMIT {
            return Err(invalid("The edited content exceeds the size limit."));
        }
        let content_id = prepared.pdf.add_object(Stream::new(dictionary! {}, data));
        prepared
            .pdf
            .get_dictionary_mut(prepared.page_id)
            .map_err(|_| invalid("Missing paragraph page."))?
            .set("Contents", content_id);
        let mut bytes = Vec::new();
        prepared
            .pdf
            .save_to(&mut bytes)
            .map_err(|_| invalid("The paragraph could not be saved."))?;
        if bytes.len() as u64 > MAX_SOURCE_BYTES {
            return Err(invalid("The edited page exceeds the source size limit."));
        }
        let bytes: Arc<[u8]> = bytes.into();
        let bytes = self.preserve_text_edit_comments(source_id, page_index, bytes)?;
        let document = self
            .pdfium
            .load_pdf_from_reader(Cursor::new(bytes.clone()), None)?;
        let page = document.pages().get(0)?;
        let proofs = FontProofs::read(&bytes)?;
        let after = bound_snapshot(&page, Some(&proofs), &bytes)?;
        let after_glyphs: HashMap<_,_> = after.iter().map(|run| Ok((run.index,object_glyphs(&page,run.index)?))).collect::<EngineResult<_>>()?;
        let remaining: Vec<_> = prepared
            .before
            .iter()
            .filter(|r| !indices.contains(&r.index))
            .collect();
        if after.len() != remaining.len() + expected_lines.len() {
            return Err(invalid("The paragraph changed unexpected text objects."));
        }
        for (old, new) in remaining.iter().zip(&after) {
            let mut adjusted = (*old).clone();
            adjusted.index = new.index;
            let old_glyphs = &prepared.glyphs[&old.index];
            let new_glyphs = &after_glyphs[&new.index];
            if !same_style(&adjusted, new)
                || !close_values(&adjusted.bounds, &new.bounds)
                || old_glyphs.len() != new_glyphs.len()
                || !old_glyphs
                    .iter()
                    .zip(new_glyphs)
                    .all(|(a, b)| a.text == b.text && close_values(&a.position, &b.position))
            {
                return Err(invalid("The paragraph would change surrounding text."));
            }
        }
        for ((line,origin), run) in expected_lines.iter().zip(&after[remaining.len()..]) {
            let font_matches=if let Some(asset)=&substitute {
                (run.reason.is_none() || whitespace_run(run)) && run.program.as_deref()==Some(asset.info.id.as_str())
            } else {run.font==first.font && run.program==first.program};
            if paragraph_layout::extraction_text(&run.text) != paragraph_layout::extraction_text(line)
                || !font_matches
                || run.color != color
                || (run.font_size - size).abs() > EPSILON
                || !close_values(&run.matrix[..4], &[1.0, 0.0, 0.0, 1.0])
                || !close_values(&run.matrix[4..6],origin)
            {
                return Err(invalid(
                    "The paragraph could not preserve its replacement text and formatting.",
                ));
            }
            // PDFium's object box includes whitespace cursor positions. Keep
            // those spaces in the content, but require every visible glyph's
            // tight ink bounds to stay in the area, including negative bearings.
            let glyphs=&after_glyphs[&run.index];
            let visible:Vec<_>=glyphs.iter().filter(|g|!g.text.trim().is_empty()).collect();
            if (!line.trim().is_empty()&&visible.is_empty()) || visible.iter().any(|glyph|{
                let p=&glyph.position;
                p.iter().any(|v|!v.is_finite())||p[2]<new_bounds[0]-EPSILON||p[3]<new_bounds[1]-EPSILON||p[4]>new_bounds[2]+EPSILON||p[5]>new_bounds[3]+EPSILON
            }) {
                return Err(invalid("The text does not fit inside the paragraph area."));
            }
        }
        let changed = raster(&page)?;
        if original.dimensions() != changed.dimensions() {
            return Err(invalid("The paragraph changed the page dimensions."));
        }
        let sx = original.width() as f32 / page_size.width;
        let sy = original.height() as f32 / page_size.height;
        let displayed_area = displayed_bounds(geometry, changed_area);
        for (x, y, pixel) in original.enumerate_pixels() {
            if pixel != changed.get_pixel(x, y)
                && ((x as f32) < displayed_area.x * sx - 3.0
                    || (x as f32) > (displayed_area.x + displayed_area.width) * sx + 3.0
                    || (y as f32) < displayed_area.y * sy - 3.0
                    || (y as f32) > (displayed_area.y + displayed_area.height) * sy + 3.0)
            {
                return Err(invalid(
                    "The paragraph would change the appearance of surrounding content.",
                ));
            }
        }
        drop(page);
        let bytes: Arc<[u8]> = paragraph_memory::write(&bytes, &after, &after_glyphs, &lines, new_bounds, size, alignment, line_spacing,saved_context)?.into();
        drop(document);
        let document = self.pdfium.load_pdf_from_reader(Cursor::new(bytes.clone()), None)?;
        {
            let page = document.pages().get(0)?;
            let final_runs = bound_snapshot(&page, Some(&proofs), &bytes)?;
            if final_runs.len() != after.len() || !final_runs.iter().zip(&after).all(|(a,b)|same_run(a,b)) || raster(&page)? != changed {
                return Err(invalid("Saving paragraph settings changed the PDF appearance."));
            }
        }
        let source = self.editing_document(source_id)?;
        let path = source.path.clone();
        let original_path = source.original_path.clone();
        let policy=source.policy.as_ref().map(|p|p.page(page_index)).transpose()?;
        let protection=policy.as_ref().map(SourcePolicy::info);
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document.pdf")
            .to_string();
        let id = Uuid::new_v4().to_string();
        let retained = self.source_store.capture(bytes.clone())?;
        self.documents.insert(
            id.clone(),
            OpenDocument {
                physical_units:OnceLock::from(physical_units::read(&bytes,document.pages().len() as usize).map_err(|e|e.to_string())),
                legacy_comment_import:false,
                imported_annotations: Default::default(),
                imported_overlay_sources:imported_overlay_sources::Index::unbound(retained.identity().clone()),
                unpublished_fonts: vec![],
                image_leases: None,
                path,
                original_path,
                document: native_residency::ResidentDocument::new(document, self.pdfium, retained.clone()),
                source_bytes: retained,
                for_printing: false,
                print_max_dpi: None,
                policy,
                text_edit_restriction: OnceLock::new(),
                form_admission:form_admission::Admission::default(),
                bookmarks:OnceLock::new(),
            },
        );
        Ok(DocumentInfo {
            id,
            name,
            pages: vec![page_size],
            bookmarks:vec![],bookmark_error:None,protection,
        })
    }
}
