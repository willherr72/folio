//! Explicit groups are admitted only after a lossless merge on an unpublished copy.
use super::*;

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
        let expected = if let Some(proof) = &runs[ordinal].proof {
            proof.encode(text)?
        } else {
            text.as_bytes().to_vec()
        };
        if encoded != expected {
            return Err(invalid("A selected piece has ambiguous or omitted characters. Select pieces with exact text encoding."));
        }
    }
    Ok(())
}

impl WorkerRuntime {
    fn prepare_text_group(
        &self,
        source_id: &str,
        page_index: usize,
        indices: &[usize],
    ) -> EngineResult<(TextGroupPreview, Arc<[u8]>)> {
        if !(2..=8).contains(&indices.len())
            || indices
                .windows(2)
                .any(|p| p[0].checked_add(1) != Some(p[1]))
        {
            return Err(invalid(
                "Select 2-8 consecutive text pieces without duplicates.",
            ));
        }
        if let Some(reason) = self.source_text_edit_restriction(source_id)? {
            return Err(invalid(&reason));
        }
        let source = self.document(source_id)?;
        let raw = lopdf::Document::load_mem(&source.source_bytes)
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
        let original = source.document.pages().get(page_index_i32(page_index)?)?;
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
        let before = snapshot(&page, Some(&proofs))?;
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
        // Read logical characters in source axes, as page_text() does, without
        // touching the source. Raster proof remains in the original rotation.
        if rotation != PdfPageRenderRotation::None && !simple_forward_text(&page, &page.text()?) {
            return Err(invalid(
                "This rotated page has text directions that cannot be grouped safely.",
            ));
        }
        page.set_rotation(PdfPageRenderRotation::None);
        let before = snapshot(&page, Some(&proofs))?;
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
        let mut logical = String::new();
        for &index in indices {
            let run = before.iter().find(|r| r.index == index).ok_or_else(|| {
                invalid("Select consecutive top-level text pieces without intervening graphics.")
            })?;
            if let Some(reason) = &run.reason {
                return Err(invalid(reason));
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
        if !(if first.embedded {
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
            font_size: first.font_size,
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
        if original_raster != candidate_raster || !same_glyphs(&baseline_glyphs, &candidate_glyphs)
        {
            return Err(invalid("These pieces cannot be joined without changing character positions or appearance. Select contiguous pieces on the same line."));
        }
        drop(page);
        drop(candidate);
        Ok((preview, bytes))
    }

    pub(in crate::engine) fn inspect_text_group(
        &self,
        source_id: &str,
        page_index: usize,
        indices: &[usize],
    ) -> EngineResult<TextGroupPreview> {
        self.prepare_text_group(source_id, page_index, indices)
            .map(|(preview, _)| preview)
    }

    pub(in crate::engine) fn replace_text_group(
        &mut self,
        source_id: &str,
        page_index: usize,
        indices: &[usize],
        expected_text: &str,
        replacement: &str,
    ) -> EngineResult<DocumentInfo> {
        let (preview, bytes) = self.prepare_text_group(source_id, page_index, indices)?;
        if preview.text != expected_text {
            return Err(invalid(
                "The text changed since it was selected. Check the selection again.",
            ));
        }
        let source = self.document(source_id)?;
        let path = source.path.clone();
        let original_path = source.original_path.clone();
        let document = self
            .pdfium
            .load_pdf_from_reader(Cursor::new(bytes.clone()), None)?;
        let temporary = Uuid::new_v4().to_string();
        self.documents.insert(
            temporary.clone(),
            OpenDocument {
                path,
                original_path,
                document,
                source_bytes: bytes,
                for_printing: false,
                text_edit_restriction: OnceLock::new(),
            },
        );
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
