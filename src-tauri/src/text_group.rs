//! Groups retain original bytes for no-op edits; private merges prove glyph and
//! surrounding-content preservation before changed text can be published.
use super::*;

struct PreparedGroup {
    preview: TextGroupPreview,
    original: Arc<[u8]>,
    merged: Arc<[u8]>,
}

fn isolated_raster_change(
    original: &PdfPage<'_>,
    before: &[RunSnapshot],
    indices: &[usize],
    selected: &RunSnapshot,
    source: &image::RgbaImage,
    merged: &image::RgbaImage,
) -> EngineResult<bool> {
    if source.dimensions() != merged.dimensions() || original.annotations().len() != 0 {
        return Ok(false);
    }
    let mut bounds = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for run in before.iter().filter(|r| indices.contains(&r.index)) {
        bounds[0] = bounds[0].min(run.bounds[0]);
        bounds[1] = bounds[1].min(run.bounds[1]);
        bounds[2] = bounds[2].max(run.bounds[2]);
        bounds[3] = bounds[3].max(run.bounds[3]);
    }
    if !close_values(&bounds, &selected.bounds) {
        return Ok(false);
    }
    let geometry = page_geometry(original)?;
    let size = geometry.displayed_size();
    let scale_x = source.width() as f32 / size.width;
    let scale_y = source.height() as f32 / size.height;
    // Reserve the three-pixel change fringe plus the neighboring object's own
    // three-pixel raster fringe. Inclusive contact is excluded, matching the
    // inclusive pixel mask below. The minimum scale covers quarter-turn axes.
    let padding = 6.0 / scale_x.min(scale_y);
    let touches = |other: &[f32; 4]| {
        other[0] <= bounds[2] + padding
            && other[2] >= bounds[0] - padding
            && other[1] <= bounds[3] + padding
            && other[3] >= bounds[1] - padding
    };
    if before
        .iter()
        .any(|r| !indices.contains(&r.index) && touches(&r.bounds))
    {
        return Ok(false);
    }
    for object in original.objects().iter() {
        if object.as_text_object().is_none() {
            if occupied_regions(&object, geometry)?.iter().any(touches) {
                return Ok(false);
            }
        }
    }
    let p1 = geometry.pdf_to_displayed(bounds[0], bounds[1]);
    let p2 = geometry.pdf_to_displayed(bounds[2], bounds[3]);
    Ok(source.enumerate_pixels().all(|(x, y, pixel)| {
        pixel == merged.get_pixel(x, y)
            || ((x as f32) >= p1.x.min(p2.x) * scale_x - 3.0
                && (x as f32) <= p1.x.max(p2.x) * scale_x + 3.0
                && (y as f32) >= p1.y.min(p2.y) * scale_y - 3.0
                && (y as f32) <= p1.y.max(p2.y) * scale_y + 3.0)
    }))
}

fn same_glyphs(a: &[Glyph], b: &[Glyph]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(a, b)| a.text == b.text && close_values(&a.position, &b.position))
}

// Extraction can omit repeated/space-only characters. Check attributed Unicode
// against the actual encoded operands too; raster equality cannot detect a lost
// trailing space. The ordinal is valid only for a unique top-level show mapping.
fn prove_encoded_members(
    bytes: &[u8],
    runs: &[RunSnapshot],
    members: &[(usize, String)],
) -> EngineResult<()> {
    let pdf = lopdf::Document::load_mem(bytes)
        .map_err(|_| invalid("The group content could not be inspected."))?;
    let page = *pdf
        .get_pages()
        .values()
        .next()
        .ok_or_else(|| invalid("The page is missing."))?;
    let bytes = pdf
        .get_page_content_with_limit(page, 16 * 1024 * 1024)
        .map_err(|_| invalid("The group content is too complex."))?;
    let content = lopdf::content::Content::decode(&bytes)
        .map_err(|_| invalid("The group content is malformed."))?;
    let shows: Vec<_> = content
        .operations
        .iter()
        .filter(|op| matches!(op.operator.as_str(), "Tj" | "TJ" | "'" | "\""))
        .filter(|op| !positioned_text::cursor_only(op))
        .collect();
    if shows.len() != runs.len() {
        return Err(invalid(
            "The selected text cannot be mapped uniquely to page objects.",
        ));
    }
    for (index, text) in members {
        let ordinal = runs
            .iter()
            .position(|r| r.index == *index)
            .ok_or_else(|| invalid("The selected piece is missing."))?;
        let op = shows[ordinal];
        let encoded = match (op.operator.as_str(), op.operands.as_slice()) {
            ("Tj", [value]) => value.as_str().ok(),
            ("TJ", [lopdf::Object::Array(values)]) if values.len() == 1 => values[0].as_str().ok(),
            _ => None,
        }
        .ok_or_else(|| invalid("The selected pieces use unsupported text positioning."))?;
        let expected = positioned_text::encoded(&runs[ordinal], text)?;
        if encoded != expected {
            return Err(invalid("A selected piece has ambiguous or omitted characters. Select pieces with exact text encoding."));
        }
    }
    Ok(())
}

impl WorkerRuntime {
    pub(in crate::engine) fn group_source_admission(&self,source_id:&str)->EngineResult<()> {
        if let Some(reason) = self.source_text_edit_restriction(source_id)? {
            return Err(invalid(&reason));
        }
        let source = self.editing_document(source_id)?;
        let raw = lopdf::Document::load_mem(source.bytes()?.as_ref())
            .map_err(|_| invalid("The source cannot be inspected safely."))?;
        if raw
            .objects
            .values()
            .filter_map(|o| o.as_dict().ok())
            .any(|d| d.has(b"StructTreeRoot") || d.has(b"StructParents") || d.has(b"StructParent"))
        {
            return Err(invalid(
                "Grouped editing of tagged documents is not supported.",
            ));
        }
        Ok(())
    }

    fn prepare_text_group(
        &self,
        source_id: &str,
        page_index: usize,
        indices: &[usize],
    ) -> EngineResult<PreparedGroup> {
        if !(2..=8).contains(&indices.len())
            || indices
                .windows(2)
                .any(|p| p[0].checked_add(1) != Some(p[1]))
        {
            return Err(invalid(
                "Select 2-8 consecutive text pieces without duplicates.",
            ));
        }
        self.group_source_admission(source_id)?;
        let source = self.editing_document(source_id)?;
        let original = source.pdf()?.pages().get(page_index_i32(page_index)?)?;
        let copy = self.editable_page_copy(source_id, page_index)?;
        let baseline = copy.save_to_bytes()?;
        let proofs = FontProofs::read(&baseline)?;
        preflight(&baseline, &proofs, true).map_err(|e| match e {
            EngineError::InvalidRequest(reason) => {
                invalid(&reason.replace("Font substitution", "Grouped editing"))
            }
            other => other,
        })?;
        let mut page = copy.pages().get(0)?;
        let rotation = page.rotation()?;
        let before = bound_snapshot(&page, Some(&proofs), &baseline)?;
        let original_runs = snapshot(&original, None)?;
        let original_glyphs = glyph_snapshot(&original)?;
        let baseline_glyphs = glyph_snapshot(&page)?;
        let original_raster = raster(&original)?;
        if original_raster != raster(&page)?
            || !same_glyphs(&original_glyphs, &baseline_glyphs)
            || original_runs.len() != before.len()
            || !original_runs
                .iter()
                .zip(&before)
                .all(|(a, b)| same_run(a, b))
        {
            return Err(invalid(
                "Copying this page changes its text or appearance; it cannot be grouped safely.",
            ));
        }
        // PDFium orders separate show operators differently under /Rotate.
        // Read logical characters in source axes without touching the source.
        // Bound lookup cost, then prove the selected direction below; unrelated
        // text need not satisfy the page-wide reader's ASCII-only workaround.
        let rotated = rotation != PdfPageRenderRotation::None;
        if rotated && (page.objects().len() > 64 || page.text()?.chars().len() > 4096) {
            return Err(invalid(
                "This rotated page has text directions that cannot be grouped safely.",
            ));
        }
        page.set_rotation(PdfPageRenderRotation::None);
        let before = bound_snapshot(&page, Some(&proofs), &baseline)?;
        let baseline_glyphs = glyph_snapshot(&page)?;
        let first = before
            .iter()
            .find(|r| r.index == indices[0])
            .ok_or_else(|| invalid("The selected text piece no longer exists."))?;
        let first_object = page.objects().get(
            indices[0]
                .try_into()
                .map_err(|_| invalid("Invalid object index."))?,
        )?;
        let first_token = first_object
            .as_text_object()
            .ok_or_else(|| invalid("Select only top-level text pieces."))?
            .font()
            .token();
        let text_page = page.text()?;
        let mut members = Vec::new();
        let mut last_character = None;
        let mut first_origin = None;
        let mut previous_x = f32::NEG_INFINITY;
        let mut logical = String::new();
        for &index in indices {
            let run = before.iter().find(|r| r.index == index).ok_or_else(|| {
                invalid("Select consecutive top-level text pieces without intervening graphics.")
            })?;
            if let Some(reason) = &run.reason {
                return Err(invalid(reason));
            }
            if rotated
                && (run.matrix[0] <= 0.0
                    || run.matrix[3] <= 0.0
                    || run.matrix[1] != 0.0
                    || run.matrix[2] != 0.0
                    || !run.matrix.iter().all(|v| v.is_finite()))
            {
                return Err(invalid("This rotated selection has text directions that cannot be grouped safely."));
            }
            let object = page.objects().get(
                index
                    .try_into()
                    .map_err(|_| invalid("Invalid object index."))?,
            )?;
            let text = object
                .as_text_object()
                .ok_or_else(|| invalid("Select only text pieces."))?;
            if text.font().token() != first_token
                || run.font_size != first.font_size
                || run.color != first.color
                || run.mode != first.mode
                || run.matrix[..4] != first.matrix[..4]
            {
                return Err(invalid("Select pieces with the same PDF font resource, size, color, and text transform."));
            }
            let mut member = String::new();
            for character in text_page.chars_for_object(text)?.iter() {
                if rotated {
                    let (x, y) = character.origin()?;
                    let (_, baseline) = *first_origin.get_or_insert((x.value, y.value));
                    if !x.value.is_finite()
                        || !y.value.is_finite()
                        || x.value < previous_x
                        || y.value != baseline
                    {
                        return Err(invalid("This rotated selection has text directions that cannot be grouped safely."));
                    }
                    previous_x = x.value;
                }
                if character.is_generated()?
                    || last_character.is_some_and(|previous| previous + 1 != character.index())
                {
                    return Err(invalid(
                        "The selected pieces contain generated spacing or ambiguous reading order.",
                    ));
                }
                let value = character
                    .unicode_string()
                    .filter(|v| !v.is_empty())
                    .ok_or_else(|| {
                        invalid("A selected character has no unambiguous Unicode text.")
                    })?;
                member.push_str(&value);
                last_character = Some(character.index());
            }
            if member.is_empty() {
                return Err(invalid(
                    "Each selected piece must contain attributable text characters.",
                ));
            }
            logical.push_str(&member);
            members.push((index, member));
        }
        if rotated && !first_origin.is_some_and(|(x, _)| previous_x > x) {
            return Err(invalid("This rotated selection has text directions that cannot be grouped safely."));
        }
        if !(if first.embedded || first.standard.is_some() {
            embedded_font::latin(&logical)
        } else {
            printable(&logical)
        }) {
            return Err(invalid("Select bounded single-line Latin text pieces."));
        }
        prove_encoded_members(&baseline, &before, &members)?;
        let preview = TextGroupPreview {
            object_indices: indices.to_vec(),
            text: logical.clone(),
            font_name: first.font.clone(),
            font_display_name: first.proof.as_ref().and_then(|p| p.display_name.clone()),
            font_size: first.font_size,
            display_font_size: first.display_font_size(),
        };
        drop(text_page);
        drop(first_object);
        drop(page);
        {
            let mut page = copy.pages().get(0)?;
            let mut object = page.objects().get(indices[0])?;
            object
                .as_text_object_mut()
                .ok_or_else(|| invalid("The first piece is not text."))?
                .set_text(&logical)?;
            drop(object);
            for &index in indices[1..].iter().rev() {
                page.objects_mut().remove_object_at_index(index)?;
            }
            page.regenerate_content()?;
            page.set_rotation(rotation);
        }
        let bytes: Arc<[u8]> = copy.save_to_bytes()?.into();
        let bytes = self.preserve_text_edit_comments(source_id, page_index, bytes)?;
        let candidate = self.pdfium.load_pdf_from_byte_slice(&bytes, None)?;
        let mut page = candidate.pages().get(0)?;
        let candidate_raster = raster(&page)?;
        page.set_rotation(PdfPageRenderRotation::None);
        let after = snapshot(&page, None)?;
        let selected = after
            .iter()
            .find(|r| r.index == indices[0])
            .ok_or_else(|| invalid("The merged text could not be reopened."))?;
        if selected.text != logical
            || !same_style(first, selected)
            || before.len() != after.len() + indices.len() - 1
        {
            return Err(invalid(
                "The merged pieces could not preserve their text and font.",
            ));
        }
        for run in before.iter().filter(|r| !indices.contains(&r.index)) {
            let mut adjusted = run.clone();
            if adjusted.index > *indices.last().unwrap() {
                adjusted.index -= indices.len() - 1;
            }
            if !after.iter().any(|r| same_run(&adjusted, r)) {
                return Err(invalid("Grouping would change an unselected text piece."));
            }
        }
        let candidate_glyphs = glyph_snapshot(&page)?;
        if !same_glyphs(&baseline_glyphs, &candidate_glyphs)
            || (original_raster != candidate_raster
                && !isolated_raster_change(
                    &original,
                    &before,
                    indices,
                    selected,
                    &original_raster,
                    &candidate_raster,
                )?)
        {
            return Err(invalid("These pieces cannot be joined without changing character positions or appearance. Select contiguous pieces on the same line."));
        }
        drop(page);
        drop(candidate);
        Ok(PreparedGroup {
            preview,
            original: baseline.into(),
            merged: bytes,
        })
    }

    pub(in crate::engine) fn inspect_text_group(
        &self,
        source_id: &str,
        page_index: usize,
        indices: &[usize],
    ) -> EngineResult<TextGroupPreview> {
        self.prepare_text_group(source_id, page_index, indices)
            .map(|prepared| prepared.preview)
    }

    pub(in crate::engine) fn replace_text_group(
        &mut self,
        source_id: &str,
        page_index: usize,
        indices: &[usize],
        expected_text: &str,
        replacement: &str,
    ) -> EngineResult<DocumentInfo> {
        let prepared = self.prepare_text_group(source_id, page_index, indices)?;
        let preview = prepared.preview;
        if preview.text != expected_text {
            return Err(invalid(
                "The text changed since it was selected. Check the selection again.",
            ));
        }
        let unchanged = replacement == preview.text;
        let bytes = if unchanged {
            self.preserve_text_edit_comments(source_id, page_index, prepared.original)?
        } else {
            prepared.merged
        };
        if bytes.len() as u64 > MAX_SOURCE_BYTES {
            return Err(invalid("The edited page exceeds the source size limit."));
        }
        let source = self.editing_document(source_id)?;
        let path = source.path.clone();
        let original_path = source.original_path.clone();
        let policy = source
            .policy
            .as_ref()
            .map(|p| p.page(page_index))
            .transpose()?;
        let document = self
            .pdfium
            .load_pdf_from_reader(Cursor::new(bytes.clone()), None)?;
        let pages = if unchanged {
            let page = document.pages().get(0)?;
            let original = source.pdf()?.pages().get(page_index_i32(page_index)?)?;
            if raster(&page)? != raster(&original)?
                || !same_glyphs(&glyph_snapshot(&page)?, &glyph_snapshot(&original)?)
            {
                return Err(invalid(
                    "The unchanged group could not preserve its original appearance.",
                ));
            }
            vec![page_geometry(&page)?.displayed_size()]
        } else {
            vec![]
        };
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document.pdf")
            .to_string();
        let protection = policy.as_ref().map(SourcePolicy::info);
        let temporary = Uuid::new_v4().to_string();
        let retained = self.source_store.capture(bytes.clone())?;
        self.documents.insert(
            temporary.clone(),
            OpenDocument {
                physical_units:OnceLock::from(physical_units::read(&bytes,document.pages().len() as usize).map_err(|e|e.to_string())),
                legacy_comment_import: false,
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
                bookmarks: OnceLock::new(),
            },
        );
        if unchanged {
            return Ok(DocumentInfo {
                id: temporary,
                name,
                pages,
                bookmarks: vec![],
                bookmark_error: None,
                protection,
            });
        }
        // No `?` between insertion and removal: even validation errors release
        // the private source. Only replace_text's final immutable source escapes.
        let result = self.replace_text(&temporary, 0, indices[0], &preview.text, replacement, None);
        self.documents.remove(&temporary);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raster_only_admission_requires_clear_selected_region_and_unchanged_surroundings() {
        use lopdf::{dictionary, Document, Stream};
        let runtime = WorkerRuntime::new(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
            Arc::new(FontRegistry::new()),
        )
        .unwrap();
        for (name, extra) in [
            ("clear", ""),
            ("artwork", "0.5 g 50 95 60 20 re f"),
            ("neighbor", "BT /F 16 Tf 1 0 0 1 50 98 Tm (Other) Tj ET"),
        ] {
            let mut pdf = Document::with_version("1.7");
            let pages = pdf.new_object_id();
            let font = pdf.add_object(
                dictionary! {"Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"Courier"},
            );
            let stream = pdf.add_object(Stream::new(
                dictionary! {},
                format!("BT /F 16 Tf 1 0 0 1 50 100 Tm (Old) Tj ( words) Tj ET {extra}")
                    .into_bytes(),
            ));
            let page = pdf.add_object(dictionary! {"Type"=>"Page", "Parent"=>pages, "MediaBox"=>vec![0.into(),0.into(),300.into(),200.into()], "Contents"=>stream, "Resources"=>dictionary! {"Font"=>dictionary! {"F"=>font}}});
            pdf.objects.insert(
                pages,
                dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into(),
            );
            let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
            pdf.trailer.set("Root", root);
            let mut bytes = vec![];
            pdf.save_to(&mut bytes).unwrap();
            let document = runtime
                .pdfium
                .load_pdf_from_byte_slice(&bytes, None)
                .unwrap();
            let mut page = document.pages().get(0).unwrap();
            let before = snapshot(&page, None).unwrap();
            let mut selected = before[0].clone();
            selected.bounds[0] = before[0].bounds[0].min(before[1].bounds[0]);
            selected.bounds[1] = before[0].bounds[1].min(before[1].bounds[1]);
            selected.bounds[2] = before[0].bounds[2].max(before[1].bounds[2]);
            selected.bounds[3] = before[0].bounds[3].max(before[1].bounds[3]);
            let source = raster(&page).unwrap();
            let mut merged = source.clone();
            let geometry = page_geometry(&page).unwrap();
            let point = geometry.pdf_to_displayed(
                (selected.bounds[0] + selected.bounds[2]) / 2.0,
                (selected.bounds[1] + selected.bounds[3]) / 2.0,
            );
            let size = geometry.displayed_size();
            let pixel = merged.get_pixel_mut(
                (point.x * source.width() as f32 / size.width) as u32,
                (point.y * source.height() as f32 / size.height) as u32,
            );
            pixel.0[0] ^= 255;
            assert_eq!(
                isolated_raster_change(&page, &before, &[0, 1], &selected, &source, &merged)
                    .unwrap(),
                name == "clear",
                "{name}"
            );
            merged.get_pixel_mut(0, 0).0[0] ^= 255;
            assert!(
                !isolated_raster_change(&page, &before, &[0, 1], &selected, &source, &merged)
                    .unwrap()
            );
            if name == "clear" {
                // Model the native bound of an unselected neighbor at the
                // permitted mask edge. Its antialias pixels can precede that
                // bound. Exercise the predicate without relying on one font's
                // rasterizer to put a fractional edge at the desired coordinate.
                for rotation in [
                    PdfPageRenderRotation::None,
                    PdfPageRenderRotation::Degrees90,
                    PdfPageRenderRotation::Degrees180,
                    PdfPageRenderRotation::Degrees270,
                ] {
                    page.set_rotation(rotation);
                    let source = raster(&page).unwrap();
                    let geometry = page_geometry(&page).unwrap();
                    let size = geometry.displayed_size();
                    let sx = source.width() as f32 / size.width;
                    let sy = source.height() as f32 / size.height;
                    let scale = sx.min(sy);
                    let p1 = geometry.pdf_to_displayed(selected.bounds[0], selected.bounds[1]);
                    let p2 = geometry.pdf_to_displayed(selected.bounds[2], selected.bounds[3]);
                    let point = geometry.pdf_to_displayed(
                        selected.bounds[2] + 3.0 / scale,
                        (selected.bounds[1] + selected.bounds[3]) / 2.0,
                    );
                    let x = (point.x * sx).floor().clamp(
                        (p1.x.min(p2.x) * sx - 3.0).ceil(),
                        (p1.x.max(p2.x) * sx + 3.0).floor(),
                    ) as u32;
                    let y = (point.y * sy).floor().clamp(
                        (p1.y.min(p2.y) * sy - 3.0).ceil(),
                        (p1.y.max(p2.y) * sy + 3.0).floor(),
                    ) as u32;
                    let mut changed = source.clone();
                    changed.get_pixel_mut(x, y).0[0] ^= 255;
                    for gap_pixels in [3.0, 3.25, 7.0] {
                        let mut neighbor = before[0].clone();
                        neighbor.index = 2;
                        neighbor.bounds = [
                            selected.bounds[2] + gap_pixels / scale,
                            selected.bounds[1],
                            selected.bounds[2] + gap_pixels / scale + 5.0,
                            selected.bounds[3],
                        ];
                        let mut with_neighbor = before.clone();
                        with_neighbor.push(neighbor);
                        assert_eq!(
                            isolated_raster_change(
                                &page,
                                &with_neighbor,
                                &[0, 1],
                                &selected,
                                &source,
                                &changed
                            )
                            .unwrap(),
                            gap_pixels > 6.0,
                            "neighbor gap {gap_pixels}px, rotation {rotation:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn group_private_sources_are_released_on_inspect_success_and_every_error() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut runtime = WorkerRuntime::new(
            &path.join("resources/pdfium/pdfium.dll"),
            Arc::new(FontRegistry::new()),
        )
        .unwrap();
        let source = runtime
            .open_document(
                path.join("../examples/Edit text together.pdf"),
                None,
                None,
                true,
                false,
            )
            .unwrap();
        let runs = runtime.list_text_runs(&source.id, 0).unwrap();
        let start = runs.runs.iter().position(|r| r.text == "In").unwrap();
        let indices: Vec<_> = runs.runs[start..start + 3]
            .iter()
            .map(|r| r.object_index)
            .collect();
        for _ in 0..10 {
            assert_eq!(
                runtime
                    .inspect_text_group(&source.id, 0, &indices)
                    .unwrap()
                    .text,
                "Invoice total"
            );
            assert_eq!(runtime.documents.len(), 1);
        }
        for (expected, replacement) in [("stale", "New"), ("Invoice total", ""), ("Invoice total", "\u{2603}"), ("Invoice total", "This replacement is far too long to fit within the visible page without causing a safety validation error")] {
            assert!(runtime.replace_text_group(&source.id, 0, &indices, expected, replacement).is_err());
            assert_eq!(runtime.documents.len(), 1);
        }
        let edited = runtime
            .replace_text_group(
                &source.id,
                0,
                &indices,
                "Invoice total",
                "Invoice grand total",
            )
            .unwrap();
        assert_eq!(runtime.documents.len(), 2);
        assert!(runtime.documents.contains_key(&source.id));
        assert!(runtime.documents.contains_key(&edited.id));
        runtime.documents.remove(&edited.id);
        runtime.documents.remove(&source.id);
        assert!(runtime.documents.is_empty());
    }
}
