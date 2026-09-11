//! Existing content editing always happens on a private page copy. The copy is
//! published only after saving and reopening validates text, placement and style.
use super::*;

#[path = "embedded_font.rs"]
mod embedded_font;
use embedded_font::{FontProofs, VerifiedFont};

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
    embedded: bool,
    program: Option<String>,
    proof: Option<Arc<VerifiedFont>>,
}

fn snapshot(page: &PdfPage<'_>, proofs: Option<&FontProofs>) -> EngineResult<Vec<RunSnapshot>> {
    if page.objects().len() > 5000 {
        return Err(invalid(
            "This page has too many objects for safe text editing.",
        ));
    }
    let page_text = page.text()?;
    let mut result = Vec::new();
    let mut programs: HashMap<PdfFontToken, Option<String>> = HashMap::new();
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
        let embedded = font.is_embedded()?;
        let program = if embedded {
            let token = font.token();
            if let Some(hash) = programs.get(&token) {
                hash.clone()
            } else {
                let hash = font.data().ok().map(|bytes| embedded_font::key(&bytes));
                programs.insert(token, hash.clone());
                hash
            }
        } else {
            None
        };
        let proof = if embedded {
            if let Some(proofs) = proofs {
                match proofs.find(&font_name, program.as_deref().unwrap_or_default()) {
                    Ok(proof) => Some(proof),
                    Err(error) => {
                        reason = Some(error);
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };
        if !embedded && !base_font(&font_name) {
            reason = Some("This non-embedded custom font cannot be edited safely.".into());
        } else if !(if embedded {
            embedded_font::latin(&value)
        } else {
            printable(&value)
        }) {
            reason = Some(
                if embedded {
                    "Only bounded single-line Latin text runs can be edited."
                } else {
                    "Only single-line printable ASCII text runs can be edited."
                }
                .into(),
            );
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
        if reason.is_none() {
            if let Some(proof) = &proof {
                if let Err(error) = proof.supports(&value) {
                    reason = Some(error.to_string());
                }
            }
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
            embedded,
            program,
            proof,
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
    a.font == b.font && a.program == b.program && same_layout_style(a, b)
}

fn same_layout_style(a: &RunSnapshot, b: &RunSnapshot) -> bool {
    a.index == b.index
        && (a.font_size - b.font_size).abs() <= EPSILON
        && close_values(&a.matrix, &b.matrix)
        && a.color == b.color
        && a.mode == b.mode
}

fn same_run(a: &RunSnapshot, b: &RunSnapshot) -> bool {
    same_style(a, b) && a.text == b.text && close_values(&a.bounds, &b.bounds)
}

// Existing overlap is permitted only within the original intersection. Comparing
// intersection edges also catches growth when two objects already overlap.
fn adds_overlap(before: &[f32; 4], after: &[f32; 4], other: &[f32; 4]) -> bool {
    let intersection = |bounds: &[f32; 4]| {
        [
            bounds[0].max(other[0]),
            bounds[1].max(other[1]),
            bounds[2].min(other[2]),
            bounds[3].min(other[3]),
        ]
    };
    let old = intersection(before);
    let new = intersection(after);
    new[2] - new[0] > EPSILON
        && new[3] - new[1] > EPSILON
        && (old[2] - old[0] <= EPSILON
            || old[3] - old[1] <= EPSILON
            || new[0] < old[0] - EPSILON
            || new[1] < old[1] - EPSILON
            || new[2] > old[2] + EPSILON
            || new[3] > old[3] + EPSILON)
}

fn solid_page_background(object: &PdfPageObject<'_>, geometry: PageGeometry) -> EngineResult<bool> {
    let Some(path) = object.as_path_object() else {
        // Image and form bounds cannot prove that their content is uniform.
        return Ok(false);
    };
    if path.is_stroked()?
        || path.fill_mode()? == PdfPathFillMode::None
        || path.fill_color()?.alpha() != 255
    {
        return Ok(false);
    }
    let segments = path.segments().transform(path.matrix()?);
    // A rectangle has a move and three sides, optionally an explicit final
    // line back to its start. Reject compound paths, curves, and open paths.
    if !matches!(segments.len(), 4 | 5) {
        return Ok(false);
    }
    let mut points = Vec::new();
    for index in 0..segments.len() {
        let segment = segments.get(index)?;
        let expected_type = if index == 0 {
            PdfPathSegmentType::MoveTo
        } else {
            PdfPathSegmentType::LineTo
        };
        if segment.segment_type() != expected_type
            || segment.is_close() != (index == segments.len() - 1)
        {
            return Ok(false);
        }
        let (x, y) = segment.point();
        if !x.value.is_finite() || !y.value.is_finite() {
            return Ok(false);
        }
        points.push([x.value, y.value]);
    }
    if points.len() == 5 && !close_values(&points[0], &points[4]) {
        return Ok(false);
    }
    // Every side must be nonzero, axis aligned, and alternate direction.
    let mut horizontal = [false; 4];
    for index in 0..4 {
        let a = points[index];
        let b = points[(index + 1) % 4];
        let same_x = (a[0] - b[0]).abs() <= EPSILON;
        let same_y = (a[1] - b[1]).abs() <= EPSILON;
        if same_x == same_y {
            return Ok(false);
        }
        horizontal[index] = same_y;
    }
    if (0..4).any(|index| horizontal[index] == horizontal[(index + 1) % 4]) {
        return Ok(false);
    }
    let left = points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
    let bottom = points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
    let right = points
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let top = points
        .iter()
        .map(|p| p[1])
        .fold(f32::NEG_INFINITY, f32::max);
    Ok(left <= geometry.left + EPSILON
        && bottom <= geometry.bottom + EPSILON
        && right >= geometry.right - EPSILON
        && top >= geometry.top - EPSILON)
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

fn substitute_in_bytes(
    bytes: &[u8],
    selected: &RunSnapshot,
    ordinal: usize,
    run_count: usize,
    replacement: &str,
    asset: &FontAsset,
) -> EngineResult<Vec<u8>> {
    use lopdf::{
        content::{Content, Operation},
        dictionary, Document, Object, Stream, StringFormat,
    };
    let mut pdf = Document::load_mem_with_options(
        bytes,
        lopdf::LoadOptions::with_max_decompressed_size(16 * 1024 * 1024),
    )
    .map_err(|_| invalid("The copied page could not be inspected for font substitution."))?;
    let page_id = *pdf
        .get_pages()
        .values()
        .next()
        .ok_or_else(|| invalid("The copied page is missing."))?;
    let content = pdf
        .get_page_content_with_limit(page_id, 16 * 1024 * 1024)
        .map_err(|_| invalid("The page content is too complex for font substitution."))?;
    let mut content = Content::decode(&content)
        .map_err(|_| invalid("The page content could not be decoded for font substitution."))?;
    let mut current = page_id;
    let mut resources = None;
    for _ in 0..64 {
        let page = pdf
            .get_dictionary(current)
            .map_err(|_| invalid("The page resources are malformed."))?;
        if let Ok(value) = page.get(b"Resources") {
            resources = Some(
                pdf.dereference(value)
                    .and_then(|(_, value)| value.as_dict())
                    .map_err(|_| invalid("The page resources are malformed."))?
                    .clone(),
            );
            break;
        }
        current = page
            .get(b"Parent")
            .and_then(Object::as_reference)
            .map_err(|_| invalid("The page font resources are missing."))?;
    }
    let mut resources =
        resources.ok_or_else(|| invalid("The page resource nesting exceeds the limit."))?;
    let mut fonts = pdf
        .dereference(
            resources
                .get(b"Font")
                .map_err(|_| invalid("The page has no fonts."))?,
        )
        .and_then(|(_, value)| value.as_dict())
        .map_err(|_| invalid("The font resources are malformed."))?
        .clone();
    let mut active_font = None;
    let mut font_stack = Vec::new();
    let mut target = None;
    let mut seen = 0;
    for (index, operation) in content.operations.iter().enumerate() {
        match operation.operator.as_str() {
            "q" => font_stack.push(active_font.clone()),
            "Q" => {
                active_font = font_stack
                    .pop()
                    .ok_or_else(|| invalid("The page graphics state is unbalanced."))?;
            }
            "Tf" => {
                if operation.operands.len() != 2
                    || operation.operands[0].as_name().is_err()
                    || operation.operands[1].as_float().is_err()
                {
                    return Err(invalid("The text font selection is malformed."));
                }
                active_font = Some(operation.operands.clone());
            }
            "Tj" | "TJ" | "'" | "\"" => {
                if seen == ordinal {
                    let encoded =
                        match (operation.operator.as_str(), operation.operands.as_slice()) {
                            ("Tj", [value]) => value.as_str().ok(),
                            ("TJ", [Object::Array(values)]) if values.len() == 1 => {
                                values[0].as_str().ok()
                            }
                            _ => None,
                        }
                        .ok_or_else(|| {
                            invalid(
                                "The selected text could not be isolated for font substitution.",
                            )
                        })?;
                    let expected = if let Some(proof) = &selected.proof {
                        proof.encode(&selected.text)?
                    } else {
                        selected.text.as_bytes().to_vec()
                    };
                    if encoded != expected.as_slice() {
                        return Err(invalid(
                            "The selected text has ambiguous content-stream positioning.",
                        ));
                    }
                    let active = active_font
                        .clone()
                        .ok_or_else(|| invalid("The selected text has no active font."))?;
                    let original = pdf
                        .dereference(
                            fonts
                                .get(active[0].as_name().unwrap())
                                .map_err(|_| invalid("The selected font resource is missing."))?,
                        )
                        .and_then(|(_, value)| value.as_dict())
                        .map_err(|_| invalid("The selected font resource is malformed."))?;
                    let resource_name = original
                        .get(b"BaseFont")
                        .and_then(Object::as_name)
                        .ok()
                        .and_then(|name| std::str::from_utf8(name).ok())
                        .unwrap_or_default();
                    if embedded_font::base_name(resource_name)
                        != embedded_font::base_name(&selected.font)
                        || (active[1].as_float().unwrap() - selected.font_size).abs() > EPSILON
                    {
                        return Err(invalid(
                            "The selected text font resource does not match its rendered run.",
                        ));
                    }
                    target = Some((index, active));
                }
                seen += 1;
            }
            _ => {}
        }
    }
    if seen != run_count || !font_stack.is_empty() {
        return Err(invalid(
            "The text content cannot be mapped uniquely to page objects.",
        ));
    }
    let (target, original_font) =
        target.ok_or_else(|| invalid("The selected text operator is missing."))?;
    let mut name = String::from("FolioReplacement");
    while fonts.has(name.as_bytes()) {
        name.push('_');
    }
    let resource = persistence::install_font_resource(&mut pdf, asset)?;
    fonts.set(name.as_bytes(), resource);
    resources.set("Font", fonts);
    pdf.get_dictionary_mut(page_id)
        .map_err(|_| invalid("The page resources are malformed."))?
        .set("Resources", resources);
    let replacement = vec![
        Operation::new(
            "Tf",
            vec![Object::Name(name.into_bytes()), original_font[1].clone()],
        ),
        Operation::new(
            "Tj",
            vec![Object::String(
                replacement
                    .encode_utf16()
                    .flat_map(u16::to_be_bytes)
                    .collect(),
                StringFormat::Hexadecimal,
            )],
        ),
        Operation::new("Tf", original_font),
    ];
    content.operations.splice(target..=target, replacement);
    let content = content
        .encode()
        .map_err(|_| invalid("The edited text could not be encoded."))?;
    let content_id = pdf.add_object(Stream::new(dictionary! {}, content));
    pdf.get_dictionary_mut(page_id)
        .map_err(|_| invalid("The copied page is malformed."))?
        .set("Contents", content_id);
    let mut output = Vec::new();
    pdf.save_to(&mut output)
        .map_err(|_| invalid("The edited page could not be saved."))?;
    Ok(output)
}

// Conservative page-level exclusions: these features can hide glyphs, change
// their meaning, or affect other graphics when PDFium regenerates the stream.
fn preflight(bytes: &[u8], proofs: &FontProofs, substitution: bool) -> EngineResult<()> {
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
        if substitution && matches!(operation.operator.as_str(), "BMC" | "BDC") {
            return Err(invalid(
                "Font substitution on marked-content or tagged text is not supported.",
            ));
        }
        // PDFium's fill-color accessor returns RGB/alpha even for a pattern.
        // A named scn/SCN operand selects a pattern, so its rectangle cannot be
        // positively identified as a solid background by inspecting that color.
        if matches!(operation.operator.as_str(), "scn" | "SCN")
            && operation.operands.iter().any(|o| o.as_name().is_ok())
        {
            return Err(invalid("Pages with pattern fills cannot be edited safely."));
        }
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
    for (id, object) in &pdf.objects {
        let Ok(dict) = object.as_dict() else {
            continue;
        };
        if dict.has(b"BaseFont") {
            if let Ok(encoding) = dict.get(b"Encoding") {
                let (_, encoding) = pdf
                    .dereference(encoding)
                    .map_err(|_| invalid("This page has an unsupported font encoding."))?;
                if !proofs.allows_resource(*id)
                    && !encoding.as_name().is_ok_and(|name| {
                        matches!(
                            name,
                            b"StandardEncoding" | b"WinAnsiEncoding" | b"MacRomanEncoding"
                        )
                    })
                {
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
        let after = snapshot(&page, None)?;
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
        let baseline = copy.save_to_bytes()?;
        let proofs = FontProofs::read(&baseline)?;
        let before = snapshot(&page, Some(&proofs))?;
        let can_substitute_page = preflight(&baseline, &proofs, true).is_ok();
        let page_reason = preflight(&baseline, &proofs, false).err().map(|e| match e {
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
                is_embedded: run.embedded,
                can_substitute: reason.is_none() && can_substitute_page,
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
        font_id: Option<&str>,
    ) -> EngineResult<DocumentInfo> {
        if let Some(reason) = self.source_text_edit_restriction(source_id)? {
            return Err(invalid(&reason));
        }
        let copy = self.editable_page_copy(source_id, page_index)?;
        let baseline = copy.save_to_bytes()?;
        let proofs = FontProofs::read(&baseline)?;
        let before = snapshot(&copy.pages().get(0)?, Some(&proofs))?;
        let selected = before
            .iter()
            .find(|r| r.index == object_index)
            .ok_or_else(|| invalid("The selected text run no longer exists."))?;
        if selected.text != expected_text {
            return Err(invalid(
                "The text changed since it was selected. Select the run again.",
            ));
        }
        if !(if selected.embedded || font_id.is_some() {
            embedded_font::latin(replacement)
        } else {
            printable(replacement)
        }) {
            return Err(invalid("Enter nonempty printable Latin text on one line, within 4096 UTF-8 bytes. Combining text, ligatures, and other scripts are not supported."));
        }
        let substitute = font_id.map(|id| self.fonts.get(id)).transpose()?;
        if let Some(font) = &substitute {
            font.validate_text(replacement)?;
        } else if let Some(proof) = &selected.proof {
            proof.supports(replacement)?;
        }
        preflight(&baseline, &proofs, substitute.is_some())?;
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
        let bytes: Arc<[u8]> = if let Some(font) = &substitute {
            // Normalize the selected stream through the already-proven no-op.
            // New PDFium text objects save into a new trailing stream and can
            // reorder content; retain the original operator slot instead.
            replace_in_page(&copy, object_index, &selected.text)?;
            let ordinal = before
                .iter()
                .position(|run| run.index == object_index)
                .unwrap();
            substitute_in_bytes(
                &copy.save_to_bytes()?,
                selected,
                ordinal,
                before.len(),
                replacement,
                font,
            )?
            .into()
        } else {
            replace_in_page(&copy, object_index, replacement)?;
            copy.save_to_bytes()?.into()
        };
        let document = self
            .pdfium
            .load_pdf_from_reader(Cursor::new(bytes.clone()), None)?;
        let page = document.pages().get(0)?;
        let after_proofs = FontProofs::read(&bytes)?;
        let after = snapshot(&page, Some(&after_proofs))?;
        let changed = after
            .iter()
            .find(|r| r.index == object_index)
            .ok_or_else(|| invalid("The edited text could not be reopened."))?;
        let style_preserved = if let Some(font) = &substitute {
            same_layout_style(selected, changed)
                && changed.program.as_deref() == Some(font.info.id.as_str())
        } else {
            same_style(selected, changed)
        };
        if let Some(reason) = &changed.reason {
            return Err(invalid(reason));
        }
        if changed.text != replacement
            || !style_preserved
            || changed.reason.is_some()
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
        if substitute.is_some() {
            self.prove_editable(&bytes, &after, object_index)?;
        }
        // The style check retains the font size and baseline. Width can grow
        // wherever the visible page has room without covering nearby content.
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
            if adds_overlap(&selected.bounds, &changed.bounds, &other.bounds) {
                return Err(invalid("The replacement would overlap neighboring text."));
            }
        }
        for (index, object) in page.objects().iter().enumerate() {
            if object.as_text_object().is_some() {
                continue;
            }
            let rect = object.bounds()?;
            let bounds = [
                rect.left().value,
                rect.bottom().value,
                rect.right().value,
                rect.top().value,
            ];
            // Only a proven solid rectangle behind the selected run is exempt.
            // Page-covering bounds alone can hide interior artwork or holes.
            let background = index < object_index && solid_page_background(&object, geometry)?;
            if !background && adds_overlap(&selected.bounds, &changed.bounds, &bounds) {
                return Err(invalid(
                    "The replacement would overlap neighboring graphics.",
                ));
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
