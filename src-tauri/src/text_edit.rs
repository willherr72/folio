//! Existing content editing always happens on a private page copy. The copy is
//! published only after saving and reopening validates text, placement and style.
use super::*;

const EPSILON: f32 = 0.02;

fn invalid(message: &str) -> EngineError {
    EngineError::InvalidRequest(message.into())
}

fn printable(text: &str) -> bool {
    !text.trim().is_empty() && text.len() <= 4096 && text.bytes().all(|c| (32..=126).contains(&c))
}

fn base_font(name: &str) -> bool {
    matches!(
        name,
        "Helvetica"
            | "Helvetica-Bold"
            | "Helvetica-Oblique"
            | "Helvetica-BoldOblique"
            | "Times-Roman"
            | "Times-Bold"
            | "Times-Italic"
            | "Times-BoldItalic"
            | "Courier"
            | "Courier-Bold"
            | "Courier-Oblique"
            | "Courier-BoldOblique"
    )
}

#[derive(Debug, Clone, PartialEq)]
struct Glyph {
    text: String,
    position: [f32; 6],
}

#[derive(Debug, Clone, PartialEq)]
struct RunSnapshot {
    index: usize,
    text: String,
    font: String,
    font_size: f32,
    matrix: [f32; 6],
    color: [u8; 4],
    mode: PdfPageTextRenderMode,
    bounds: [f32; 4],
    reason: Option<String>,
}

fn snapshot(page: &PdfPage<'_>) -> EngineResult<Vec<RunSnapshot>> {
    if page.objects().len() > 5000 {
        return Err(invalid(
            "This page has too many objects for safe text editing.",
        ));
    }
    let page_text = page.text()?;
    let mut result = Vec::new();
    for (index, object) in page.objects().iter().enumerate() {
        let Some(text) = object.as_text_object() else {
            continue;
        };
        let font = text.font();
        let font_name = font.name();
        let value = page_text.for_object(text);
        let matrix = text.matrix()?;
        let matrix = [
            matrix.a(),
            matrix.b(),
            matrix.c(),
            matrix.d(),
            matrix.e(),
            matrix.f(),
        ];
        let rect = object.bounds()?;
        let bounds = [
            rect.left().value,
            rect.bottom().value,
            rect.right().value,
            rect.top().value,
        ];
        let mode = text.render_mode();
        let color = text.fill_color()?;
        let mut reason = None;
        if !base_font(&font_name) || font.is_embedded()? {
            reason = Some("This font is embedded, subset, or custom. Only standard Helvetica, Times, and Courier fonts can be edited.".into());
        } else if !printable(&value) {
            reason = Some("Only single-line printable ASCII text runs can be edited.".into());
        } else if mode != PdfPageTextRenderMode::FilledUnstroked || color.alpha() != 255 {
            reason =
                Some("Invisible, clipped, stroked, or transparent text is not supported.".into());
        } else if (matrix[0] - 1.0).abs() > EPSILON
            || matrix[1].abs() > EPSILON
            || matrix[2].abs() > EPSILON
            || (matrix[3] - 1.0).abs() > EPSILON
        {
            reason = Some("Rotated, scaled, or skewed text objects are not supported. Page rotation is supported.".into());
        }
        result.push(RunSnapshot {
            index,
            text: value,
            font: font_name,
            font_size: text.unscaled_font_size().value,
            matrix,
            color: [color.red(), color.green(), color.blue(), color.alpha()],
            mode,
            bounds,
            reason,
        });
    }
    Ok(result)
}

fn glyph_snapshot(page: &PdfPage<'_>) -> EngineResult<Vec<Glyph>> {
    let text = page.text()?;
    text.chars()
        .iter()
        .map(|character| {
            let rect = character.tight_bounds()?;
            let (x, y) = character.origin()?;
            Ok(Glyph {
                text: character.unicode_string().unwrap_or_default(),
                position: [
                    x.value,
                    y.value,
                    rect.left().value,
                    rect.bottom().value,
                    rect.right().value,
                    rect.top().value,
                ],
            })
        })
        .collect()
}

fn raster(page: &PdfPage<'_>) -> EngineResult<image::RgbaImage> {
    Ok(page
        .render_with_config(
            &PdfRenderConfig::new()
                .set_target_width(1000)
                .set_maximum_height(1600),
        )?
        .as_image()?
        .to_rgba8())
}

fn close_values(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(a, b)| a.is_finite() && b.is_finite() && (a - b).abs() <= EPSILON)
}

fn same_style(a: &RunSnapshot, b: &RunSnapshot) -> bool {
    a.index == b.index
        && a.font == b.font
        && (a.font_size - b.font_size).abs() <= EPSILON
        && close_values(&a.matrix, &b.matrix)
        && a.color == b.color
        && a.mode == b.mode
}

fn same_run(a: &RunSnapshot, b: &RunSnapshot) -> bool {
    same_style(a, b) && a.text == b.text && close_values(&a.bounds, &b.bounds)
}

fn replace_in_page(
    document: &PdfDocument<'_>,
    index: usize,
    replacement: &str,
) -> EngineResult<()> {
    let mut page = document.pages().get(0)?;
    {
        let mut object = page.objects().get(
            index
                .try_into()
                .map_err(|_| invalid("Invalid text object index."))?,
        )?;
        object
            .as_text_object_mut()
            .ok_or_else(|| invalid("The selected object is not text."))?
            .set_text(replacement)?;
    }
    page.regenerate_content()?;
    Ok(())
}

// Conservative page-level exclusions: these features can hide glyphs, change
// their meaning, or affect other graphics when PDFium regenerates the stream.
fn preflight(bytes: &[u8]) -> EngineResult<()> {
    let pdf = lopdf::Document::load_mem(bytes)
        .map_err(|_| invalid("The page content could not be inspected safely."))?;
    let page = *pdf
        .get_pages()
        .values()
        .next()
        .ok_or_else(|| invalid("The page is missing."))?;
    let content = pdf
        .get_page_content_with_limit(page, 16 * 1024 * 1024)
        .map_err(|_| invalid("The page content is too complex for safe text editing."))?;
    let content = lopdf::content::Content::decode(&content)
        .map_err(|_| invalid("The page content could not be inspected safely."))?;
    for operation in content.operations {
        if matches!(operation.operator.as_str(), "W" | "W*") {
            return Err(invalid(
                "Pages with clipping paths cannot be edited safely.",
            ));
        }
        if operation.operator == "TJ"
            && operation
                .operands
                .iter()
                .filter_map(|o| o.as_array().ok())
                .flatten()
                .any(|o| o.as_float().is_ok_and(|n| n.abs() > EPSILON))
        {
            return Err(invalid("This page uses custom character positioning (kerning). Its text cannot be edited safely."));
        }
    }
    for object in pdf.objects.values() {
        let Ok(dict) = object.as_dict() else {
            continue;
        };
        if dict.has(b"BaseFont") {
            if let Ok(encoding) = dict.get(b"Encoding") {
                let (_, encoding) = pdf
                    .dereference(encoding)
                    .map_err(|_| invalid("This page has an unsupported font encoding."))?;
                if !encoding.as_name().is_ok_and(|name| {
                    matches!(
                        name,
                        b"StandardEncoding" | b"WinAnsiEncoding" | b"MacRomanEncoding"
                    )
                }) {
                    return Err(invalid(
                        "This page uses a custom font encoding that cannot be preserved safely.",
                    ));
                }
            }
        }
        if dict
            .get(b"Type")
            .and_then(lopdf::Object::as_name)
            .is_ok_and(|name| name == b"ExtGState")
        {
            for (key, value) in dict.iter() {
                let supported = match key.as_slice() {
                    b"Type" => true,
                    b"ca" | b"CA" => value.as_float().is_ok_and(|a| (a - 1.0).abs() < EPSILON),
                    b"BM" => value.as_name().is_ok_and(|n| n == b"Normal"),
                    b"SMask" => value.as_name().is_ok_and(|n| n == b"None"),
                    _ => false,
                };
                if !supported {
                    return Err(invalid("This page uses transparency or advanced graphics state that cannot be edited safely."));
                }
            }
        }
    }
    Ok(())
}

impl WorkerRuntime {
    fn source_text_edit_restriction(&self, source_id: &str) -> EngineResult<Option<String>> {
        let source = self.document(source_id)?;
        Ok(source
            .text_edit_restriction
            .get_or_init(|| {
                let Ok(pdf) = lopdf::Document::load_mem(&source.source_bytes) else {
                    return Some("The source document could not be inspected safely.".into());
                };
                let optional = pdf
                    .objects
                    .values()
                    .filter_map(|o| o.as_dict().ok())
                    .any(|dict| {
                        dict.has(b"OCProperties")
                            || dict
                                .get(b"Type")
                                .and_then(lopdf::Object::as_name)
                                .is_ok_and(|name| matches!(name, b"OCG" | b"OCMD"))
                    });
                optional.then(|| {
                    "Documents with optional content (PDF layers) cannot be edited safely.".into()
                })
            })
            .clone())
    }

    fn editable_page_copy(
        &self,
        source_id: &str,
        page_index: usize,
    ) -> EngineResult<PdfDocument<'static>> {
        let source = self.document(source_id)?;
        let mut copy = self.pdfium.create_new_pdf()?;
        copy.pages_mut().copy_page_from_document(
            &source.document,
            page_index_i32(page_index)?,
            0,
        )?;
        Ok(copy)
    }

    /// Prove that FPDFText_SetText does not discard TJ positioning or text state.
    /// Testing a no-op through save/reopen also catches encodings that cannot roundtrip.
    fn prove_editable(
        &self,
        baseline: &[u8],
        before: &[RunSnapshot],
        index: usize,
    ) -> EngineResult<()> {
        let run = before
            .iter()
            .find(|r| r.index == index)
            .ok_or_else(|| invalid("The selected text run no longer exists."))?;
        if let Some(reason) = &run.reason {
            return Err(invalid(reason));
        }
        let copy = self.pdfium.load_pdf_from_byte_slice(baseline, None)?;
        let original_glyphs = glyph_snapshot(&copy.pages().get(0)?)?;
        let original_raster = raster(&copy.pages().get(0)?)?;
        replace_in_page(&copy, index, &run.text)?;
        let bytes = copy.save_to_bytes()?;
        let reopened = self.pdfium.load_pdf_from_byte_slice(&bytes, None)?;
        let page = reopened.pages().get(0)?;
        let after = snapshot(&page)?;
        let glyphs = glyph_snapshot(&page)?;
        if before.len() != after.len() || !before.iter().zip(&after).all(|(a, b)| same_run(a, b)) {
            return Err(invalid(
                "This run uses character positioning or encoding that cannot be preserved safely.",
            ));
        }
        if original_glyphs.len() != glyphs.len()
            || !original_glyphs
                .iter()
                .zip(&glyphs)
                .all(|(a, b)| a.text == b.text && close_values(&a.position, &b.position))
            || original_raster != raster(&page)?
        {
            return Err(invalid("This run uses character positioning or appearance that cannot be preserved safely."));
        }
        Ok(())
    }

    pub(super) fn list_text_runs(
        &self,
        source_id: &str,
        page_index: usize,
    ) -> EngineResult<TextRuns> {
        if let Some(reason) = self.source_text_edit_restriction(source_id)? {
            // Still validate the requested page even when the document is unsupported.
            self.document(source_id)?
                .document
                .pages()
                .get(page_index_i32(page_index)?)?;
            return Ok(TextRuns {
                runs: vec![],
                reason: Some(reason),
            });
        }
        let copy = self.editable_page_copy(source_id, page_index)?;
        let page = copy.pages().get(0)?;
        let geometry = page_geometry(&page)?;
        let before = snapshot(&page)?;
        let baseline = copy.save_to_bytes()?;
        let page_reason = preflight(&baseline).err().map(|e| match e {
            EngineError::InvalidRequest(s) => s,
            _ => "This page could not be validated for editing.".into(),
        });
        let mut runs = Vec::new();
        for run in &before {
            let reason = run.reason.clone().or_else(|| page_reason.clone());
            let p1 = geometry.pdf_to_displayed(run.bounds[0], run.bounds[1]);
            let p2 = geometry.pdf_to_displayed(run.bounds[2], run.bounds[3]);
            runs.push(EditableTextRun {
                object_index: run.index,
                text: run.text.clone(),
                font_name: run.font.clone(),
                font_size: run.font_size,
                bounds: AnnotationRect {
                    x: p1.x.min(p2.x),
                    y: p1.y.min(p2.y),
                    width: (p1.x - p2.x).abs(),
                    height: (p1.y - p2.y).abs(),
                },
                supported: reason.is_none(),
                reason,
            });
        }
        let reason = if runs.is_empty() {
            Some("No top-level text runs were found. Scanned pages and text inside forms cannot be edited.".into())
        } else {
            None
        };
        Ok(TextRuns { runs, reason })
    }

    pub(super) fn replace_text(
        &mut self,
        source_id: &str,
        page_index: usize,
        object_index: usize,
        expected_text: &str,
        replacement: &str,
    ) -> EngineResult<DocumentInfo> {
        if let Some(reason) = self.source_text_edit_restriction(source_id)? {
            return Err(invalid(&reason));
        }
        if !printable(replacement) {
            return Err(invalid("Enter 1–4096 printable ASCII characters on one line. Empty text and unsupported characters cannot be saved."));
        }
        let copy = self.editable_page_copy(source_id, page_index)?;
        let before = snapshot(&copy.pages().get(0)?)?;
        let selected = before
            .iter()
            .find(|r| r.index == object_index)
            .ok_or_else(|| invalid("The selected text run no longer exists."))?;
        if selected.text != expected_text {
            return Err(invalid(
                "The text changed since it was selected. Select the run again.",
            ));
        }
        let baseline = copy.save_to_bytes()?;
        preflight(&baseline)?;
        self.prove_editable(&baseline, &before, object_index)?;
        let original_raster = raster(
            &self
                .document(source_id)?
                .document
                .pages()
                .get(page_index_i32(page_index)?)?,
        )?;
        if original_raster != raster(&copy.pages().get(0)?)? {
            return Err(invalid(
                "Copying this page would change its appearance. Its text cannot be edited safely.",
            ));
        }
        replace_in_page(&copy, object_index, replacement)?;
        let bytes: Arc<[u8]> = copy.save_to_bytes()?.into();
        let document = self
            .pdfium
            .load_pdf_from_reader(Cursor::new(bytes.clone()), None)?;
        let page = document.pages().get(0)?;
        let after = snapshot(&page)?;
        let changed = after
            .iter()
            .find(|r| r.index == object_index)
            .ok_or_else(|| invalid("The edited text could not be reopened."))?;
        if changed.text != replacement
            || !same_style(selected, changed)
            || before.len() != after.len()
            || !before
                .iter()
                .zip(&after)
                .all(|(a, b)| a.index == object_index || same_run(a, b))
        {
            return Err(invalid(
                "The replacement could not preserve the font, placement, or surrounding text.",
            ));
        }
        // Retain the original baseline and maximum right edge; glyphs may have
        // different ascenders/descenders, but never expand into the next run.
        if changed.bounds[2] > selected.bounds[2] + EPSILON {
            return Err(invalid("The replacement is too wide for this text run. Use shorter text; font size and neighboring content are preserved."));
        }
        let geometry = page_geometry(&page)?;
        if changed.bounds[0] < geometry.left
            || changed.bounds[2] > geometry.right
            || changed.bounds[1] < geometry.bottom
            || changed.bounds[3] > geometry.top
        {
            return Err(invalid(
                "The replacement would extend outside the visible page.",
            ));
        }
        for other in &before {
            if other.index == object_index {
                continue;
            }
            let overlap = |a: &[f32; 4], b: &[f32; 4]| {
                a[0] < b[2] - EPSILON
                    && a[2] > b[0] + EPSILON
                    && a[1] < b[3] - EPSILON
                    && a[3] > b[1] + EPSILON
            };
            if overlap(&changed.bounds, &other.bounds) && !overlap(&selected.bounds, &other.bounds)
            {
                return Err(invalid("The replacement would overlap neighboring text."));
            }
        }
        let changed_raster = raster(&page)?;
        if original_raster.dimensions() != changed_raster.dimensions() {
            return Err(invalid("The replacement changed the page dimensions."));
        }
        let size = geometry.displayed_size();
        let scale_x = original_raster.width() as f32 / size.width;
        let scale_y = original_raster.height() as f32 / size.height;
        let p1 = geometry.pdf_to_displayed(
            selected.bounds[0].min(changed.bounds[0]),
            selected.bounds[1].min(changed.bounds[1]),
        );
        let p2 = geometry.pdf_to_displayed(
            selected.bounds[2].max(changed.bounds[2]),
            selected.bounds[3].max(changed.bounds[3]),
        );
        for (x, y, pixel) in original_raster.enumerate_pixels() {
            if pixel != changed_raster.get_pixel(x, y)
                && ((x as f32) < p1.x.min(p2.x) * scale_x - 3.0
                    || (x as f32) > p1.x.max(p2.x) * scale_x + 3.0
                    || (y as f32) < p1.y.min(p2.y) * scale_y - 3.0
                    || (y as f32) > p1.y.max(p2.y) * scale_y + 3.0)
            {
                return Err(invalid(
                    "The replacement would change the appearance of surrounding page content.",
                ));
            }
        }
        if bytes.len() as u64 > MAX_SOURCE_BYTES {
            return Err(invalid("The edited page exceeds the source size limit."));
        }
        let pages = vec![geometry.displayed_size()];
        drop(page);
        let source = self.document(source_id)?;
        let path = source.path.clone();
        let original_path = source.original_path.clone();
        let id = Uuid::new_v4().to_string();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document.pdf")
            .to_string();
        self.documents.insert(
            id.clone(),
            OpenDocument {
                path,
                original_path,
                document,
                source_bytes: bytes,
                for_printing: false,
                text_edit_restriction: OnceLock::new(),
            },
        );
        Ok(DocumentInfo { id, name, pages })
    }
}
