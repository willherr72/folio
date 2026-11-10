//! Existing content editing always happens on a private page copy. The copy is
//! published only after saving and reopening validates text, placement and style.
use super::*;

#[path = "text_graphics.rs"]
mod text_graphics;
use text_graphics::occupied_regions;

#[path = "embedded_font.rs"]
mod embedded_font;
use embedded_font::{FontProofs, VerifiedFont};
pub(crate) use embedded_font::{choice_font_codes,redaction_font_codes};

#[path = "equivalent_encoding.rs"]
mod equivalent_encoding;

#[path = "standard_font.rs"]
mod standard_font;

/// Fixed nominal metrics for already admitted simple-font redaction programs.
/// Decode each source code in its actual encoding before consulting the shared
/// width tables (notably apostrophe/backtick in StandardEncoding).
pub(crate) fn nominal_standard_ascii_metrics(base: &[u8], win_ansi: bool) -> EngineResult<([u16; 95], [u32; 95])> {
    let mut resource = lopdf::Dictionary::new();
    resource.set("Type", "Font"); resource.set("Subtype", "Type1");
    resource.set("BaseFont", lopdf::Object::Name(base.to_vec()));
    if win_ansi { resource.set("Encoding", "WinAnsiEncoding"); }
    let pdf = lopdf::Document::new();
    let font = standard_font::StandardFont::read(&pdf, &resource)?;
    let mut widths = [0; 95];
    let mut scalars = [0; 95];
    for (index, code) in (32u8..=126).enumerate() {
        let text = font.decode(&[code])?;
        let mut chars = text.chars();
        scalars[index] = chars.next().ok_or_else(crate::redaction_content::invalid)? as u32;
        if chars.next().is_some() { return Err(crate::redaction_content::invalid()); }
        let width = font.advance(&text)? * 1000.0;
        if !width.is_finite() || width <= 0.0 || width > 2000.0 || (width - width.round()).abs() > 0.001 {
            return Err(crate::redaction_content::invalid());
        }
        widths[index] = width.round() as u16;
    }
    Ok((widths, scalars))
}

#[path = "positioned_text.rs"]
mod positioned_text;
#[path = "text_substitution.rs"]
mod text_substitution;
#[path = "text_show.rs"]
mod text_show;
#[path = "source_text.rs"]
mod source_text;
#[path = "text_context.rs"]
mod text_context;
#[path = "text_context_tags.rs"]
mod text_context_tags;
#[path = "text_context_edit.rs"]
mod text_context_edit;
pub(super) use text_context_edit::{PublicationProof,SourcePage as ParagraphSourcePage};
#[cfg(test)]
#[path = "text_context_projection_tests.rs"]
mod text_context_projection_tests;

#[path = "text_group.rs"]
mod text_group;
#[path = "text_paragraph.rs"]
mod text_paragraph;
#[path = "paragraph_selection.rs"]
mod paragraph_selection;

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
pub(super) struct Glyph {
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
    standard: Option<Arc<standard_font::StandardFont>>,
}

impl RunSnapshot {
    fn display_font_size(&self) -> Option<f32> {
        // The Tf operand can be 1 while Tm/CTM scales the em to 12 points.
        // Use the full vertical vector so quarter turns and reflection retain
        // their size. UserUnit is applied once by the public DTO adapter.
        let size = (self.font_size as f64).abs()
            * (self.matrix[2] as f64).hypot(self.matrix[3] as f64);
        let size = size as f32;
        (size.is_finite() && size > 0.).then_some(size)
    }
}

// Property validation is identical for native snapshots and a local editing
// view of verified source operands. Font-proof lookup failures stay with the
// snapshot; a source view cannot be constructed without that proof.
fn run_content_reason(run: &RunSnapshot) -> Option<String> {
    if !run.embedded && !base_font(&run.font) {
        Some("This non-embedded custom font cannot be edited safely.".into())
    } else if !(if run.embedded || run.standard.is_some() { embedded_font::latin(&run.text) } else { printable(&run.text) }) {
        Some(if run.embedded || run.standard.is_some() {
            "Only bounded single-line Latin text runs can be edited."
        } else {
            "Only single-line printable ASCII text runs can be edited."
        }.into())
    } else if run.mode != PdfPageTextRenderMode::FilledUnstroked || run.color[3] != 255 {
        Some("Invisible, clipped, stroked, or transparent text is not supported.".into())
    } else if run.matrix.iter().any(|v| !v.is_finite() || v.abs() > 1_000_000.0)
        || (run.matrix[0] * run.matrix[3] - run.matrix[1] * run.matrix[2]).abs() < 0.0001
        || run.matrix[..4].iter().any(|v| v.abs() > 100.0) {
        Some("This text transform is singular or exceeds safe editing bounds.".into())
    } else {
        run.proof.as_ref().and_then(|proof| proof.supports(&run.text).err())
            .or_else(|| run.standard.as_ref().and_then(|proof| proof.encode(&run.text).err()))
            .map(|error| error.to_string())
    }
}

/// `value` must come from the independently verified source operand mapper.
/// Raw snapshots still own selection, paragraph, neighbor and raster evidence.
fn source_edit_run(run: &RunSnapshot, value: Option<&str>) -> RunSnapshot {
    let mut source = run.clone();
    if let Some(value) = value.filter(|_| !run.embedded || run.proof.is_some()) {
        source.text = value.to_owned();
        source.reason = run_content_reason(&source);
    } else if run.proof.as_ref().is_some_and(|proof| proof.requires_source_operands) {
        source.reason = Some("This source text contains unavailable character mappings or cannot be matched to exact font codes.".into());
    }
    source
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
        let mut run = RunSnapshot {
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
            standard: None,
        };
        run.reason = run_content_reason(&run).or(run.reason);
        result.push(run);
    }
    Ok(result)
}

fn bound_snapshot(page: &PdfPage<'_>, proofs: Option<&FontProofs>, bytes: &[u8]) -> EngineResult<Vec<RunSnapshot>> {
    let mut runs = snapshot(page, proofs)?;
    // Failure adds no new authority; existing ASCII/embedded admission remains
    // available under its original checks. Binding is atomic on full traversal.
    let _ = source_text::bind_standard(bytes, &mut runs);
    Ok(runs)
}

pub(super) fn glyph_snapshot(page: &PdfPage<'_>) -> EngineResult<Vec<Glyph>> {
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

pub(super) fn raster(page: &PdfPage<'_>) -> EngineResult<image::RgbaImage> {
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
    let Some([left,bottom,right,top])=solid_rectangle_bounds(object)? else{return Ok(false);};
    Ok(left <= geometry.left + EPSILON
        && bottom <= geometry.bottom + EPSILON
        && right >= geometry.right - EPSILON
        && top >= geometry.top - EPSILON)
}

// Return actual transformed path geometry, never an object's broad bounding box.
// Callers still decide whether this rectangle is a page fill or a text mark.
fn solid_rectangle_bounds(object: &PdfPageObject<'_>) -> EngineResult<Option<[f32;4]>> {
    let Some(path) = object.as_path_object() else {
        // Image and form bounds cannot prove that their content is uniform.
        return Ok(None);
    };
    if path.is_stroked()?
        || path.fill_mode()? == PdfPathFillMode::None
        || path.fill_color()?.alpha() != 255
    {
        return Ok(None);
    }
    let segments = path.segments().transform(path.matrix()?);
    // A rectangle has a move and three sides, optionally an explicit final
    // line back to its start. Reject compound paths, curves, and open paths.
    if !matches!(segments.len(), 4 | 5) {
        return Ok(None);
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
            return Ok(None);
        }
        let (x, y) = segment.point();
        if !x.value.is_finite() || !y.value.is_finite() {
            return Ok(None);
        }
        points.push([x.value, y.value]);
    }
    if points.len() == 5 && !close_values(&points[0], &points[4]) {
        return Ok(None);
    }
    // Every side must be nonzero, axis aligned, and alternate direction.
    let mut horizontal = [false; 4];
    for index in 0..4 {
        let a = points[index];
        let b = points[(index + 1) % 4];
        let same_x = (a[0] - b[0]).abs() <= EPSILON;
        let same_y = (a[1] - b[1]).abs() <= EPSILON;
        if same_x == same_y {
            return Ok(None);
        }
        horizontal[index] = same_y;
    }
    if (0..4).any(|index| horizontal[index] == horizontal[(index + 1) % 4]) {
        return Ok(None);
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
    Ok(Some([left,bottom,right,top]))
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
                if positioned_text::cursor_only(operation) {continue;}
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
                    if !embedded_font::matches_name(&pdf, original, &selected.font, selected.embedded)
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
fn preflight(bytes: &[u8], proofs: &FontProofs, substitution: bool) -> EngineResult<bool> {
    preflight_with_context(bytes,proofs,substitution,None)
}

fn preflight_with_context(bytes: &[u8], proofs: &FontProofs, substitution: bool, context: Option<&PublicationProof>) -> EngineResult<bool> {
    if substitution && context.is_some() {
        return Err(invalid("Font substitution within preserved clipping or tagged text is not supported yet."));
    }
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
    let mut preserve_state = context.is_some();
    for operation in content.operations {
        let explicit_state = match operation.operator.as_str() {
            "Tc" | "Tw" => {
                !matches!(operation.operands.as_slice(), [value] if value.as_float().is_ok_and(|n| n == 0.0))
            }
            "\"" => true,
            "TJ" if !positioned_text::cursor_only(&operation) => operation
                .operands
                .iter()
                .filter_map(|v| v.as_array().ok())
                .flatten()
                .any(|v| v.as_float().is_ok_and(|n| n != 0.0)),
            _ => false,
        };
        preserve_state |= explicit_state;
        if substitution && explicit_state {
            return Err(invalid("Font substitution with explicit character positioning or word spacing is not supported."));
        }
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
        if context.is_none() && matches!(operation.operator.as_str(), "W" | "W*") {
            return Err(invalid(
                "Pages with clipping paths cannot be edited safely.",
            ));
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
                    && equivalent_encoding::name(&pdf, encoding).is_none()
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
                    // Printer-generated defaults can be retained verbatim by
                    // the stream route. Never regenerate or substitute them.
                    b"OP" | b"op" | b"SA" => value.as_bool().is_ok_and(|v| !v),
                    b"OPM" => value.as_i64().is_ok_and(|n| n == 0 || n == 1),
                    b"SM" => value
                        .as_float()
                        .is_ok_and(|n| n.is_finite() && (0.0..=1.0).contains(&n)),
                    _ => false,
                };
                if !supported {
                    return Err(invalid("This page uses transparency or advanced graphics state that cannot be edited safely."));
                }
                if matches!(key.as_slice(), b"OP" | b"op" | b"SA" | b"OPM" | b"SM") {
                    if substitution {
                        return Err(invalid(
                            "Font substitution with printer graphics state is not supported.",
                        ));
                    }
                    preserve_state = true;
                }
            }
        }
    }
    Ok(preserve_state)
}

impl WorkerRuntime {
    fn source_text_edit_restriction(&self, source_id: &str) -> EngineResult<Option<String>> {
        let _phase=crate::native_phase!("source.restriction");
        let source = self.editing_document(source_id)?;
        let bytes = if source.text_edit_restriction.get().is_none() { Some(source.bytes()?) } else { None };
        Ok(source
            .text_edit_restriction
            .get_or_init(|| {
                let Ok(pdf) = lopdf::Document::load_mem(bytes.as_ref().expect("uncached restriction lease")) else {
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

    /// Called only after the raw form-graph guard. Every nested form has an
    /// immediate page-level form ancestor; annotation appearances are not page
    /// content. Empty discovery need not copy, rasterize or validate text runs.
    pub(super) fn text_page_has_form_objects(&self, source_id: &str, page_index: usize) -> EngineResult<bool> {
        if let Some(reason) = self.source_text_edit_restriction(source_id)? {
            return Err(invalid(&reason));
        }
        let page = self.editing_document(source_id)?.pdf()?.pages().get(page_index_i32(page_index)?)?;
        Ok(page.objects().iter().any(|object| object.as_x_object_form_object().is_some()))
    }

    // Rendering a text-edit copy can mutate PDFium's native note Rect/AP.
    // Restore from the original immutable source before validating/publishing bytes.
    fn preserve_text_edit_comments(
        &self,
        source_id: &str,
        page_index: usize,
        bytes: Arc<[u8]>,
    ) -> EngineResult<Arc<[u8]>> {
        let source = self.editing_document(source_id)?;
        let mut output = crate::pdf_output::load(&bytes)?;
        let source_bytes = source.bytes()?;
        let comment_source = crate::acroforms::SourceForm::new(&source_bytes, vec![(page_index, 0)]);
        let mut changed = native_comments::preserve_geometry(&mut output, &[
            native_comments::SourceComments {
                source: &comment_source,
                excluded: &source.imported_annotations,
            },
        ])?;
        changed |= native_comments::repair_popup_parents(&mut output)?;
        if changed {
            Ok(crate::pdf_output::save(&mut output)?.into())
        } else {
            Ok(bytes)
        }
    }

    pub(super) fn verify_text_publication(
        &self, result_id: &str, page_index: usize, candidate_id: &str,
    ) -> EngineResult<()> {
        let _phase=crate::native_phase!("publication.exact_verify");
        let result=self.editing_document(result_id)?.pdf()?.pages().get(page_index_i32(page_index)?)?;
        let expected=self.editing_document(candidate_id)?.pdf()?.pages().get(0)?;
        let before=snapshot(&expected,None)?;
        let after=snapshot(&result,None)?;
        if before.len()!=after.len() || !before.iter().zip(&after).all(|(a,b)|same_run(a,b))
            || glyph_snapshot(&expected)?!=glyph_snapshot(&result)? || raster(&expected)?!=raster(&result)? {
            return Err(invalid("Publishing the edit changed its text, placement, or appearance. The original document has been kept."));
        }
        Ok(())
    }

    fn editable_page_copy(
        &self,
        source_id: &str,
        page_index: usize,
    ) -> EngineResult<PdfDocument<'static>> {
        self.text_transaction_page_copy(source_id, page_index, false)
    }

    /// Private, source-authorized text-edit baseline. No projected source ID is
    /// returned to a client; the enclosing worker request owns its full lifetime.
    pub(super) fn with_text_projection<T>(
        &mut self,source_id:&str,page_index:usize,
        use_page:impl FnOnce(&mut Self,&str)->EngineResult<T>,
    )->EngineResult<T> {
        if let Some(reason)=self.source_text_edit_restriction(source_id)? {return Err(invalid(&reason));}
        let source=self.editing_document(source_id)?;
        let policy=source.policy.as_ref().map(|p|p.page(page_index)).transpose()?;
        let original_path=source.original_path.clone();
        let legacy=source.legacy_comment_import;
        let copy=self.text_transaction_page_copy(source_id,page_index,true)?;
        let bytes=self.preserve_text_edit_comments(source_id,page_index,copy.save_to_bytes()?.into())?;
        drop(copy);
        let projection=self.register_generated_document(bytes.to_vec(),"Private text page.pdf")?;
        let projected=self.documents.get_mut(&projection.id).unwrap();
        projected.policy=policy;projected.original_path=original_path;projected.legacy_comment_import=legacy;
        let result=(|| {
            self.verify_text_publication(source_id,page_index,&projection.id)?;
            use_page(self,&projection.id)
        })();
        self.discard_unpublished_source(&projection.id);
        result
    }

    fn text_transaction_page_copy(
        &self, source_id: &str, page_index: usize, complete_source: bool,
    ) -> EngineResult<PdfDocument<'static>> {
        let _phase=crate::native_phase!("isolation.total");
        let source = self.editing_document(source_id)?;
        let group = {let _phase=crate::native_phase!("isolation.group");text_context_group::read(source.bytes()?.as_ref(), page_index)?};
        let mut copy = self.pdfium.create_new_pdf()?;
        copy.pages_mut().copy_page_from_document(
            source.pdf()?,
            page_index_i32(page_index)?,
            0,
        )?;
        let form_bytes = {
            let _phase=crate::native_phase!("isolation.resources");
            let source_bytes = source.bytes()?;
            let mut request = crate::acroforms::SourceForm::new(&source_bytes, vec![(page_index, 0)]);
            request.enable_parse_reuse();
            let resource_sources = std::slice::from_ref(&request);
            if crate::page_resources::needed(resource_sources)? {
                if let Some(bytes) = crate::page_resources::preserve_bytes(&copy.save_to_bytes()?, resource_sources)? {
                    copy = self.pdfium.load_pdf_from_reader(Cursor::new(bytes), None)?;
                }
            }
            let form_copy = if complete_source {
                crate::acroforms::project_page_for_text_transaction
            } else {
                crate::acroforms::preserve_page_copy
            };
            form_copy(&request, &copy.save_to_bytes()?)?
        };
        // The request-local parsed graph is dropped before loading the final
        // working copy and checking its transparency group.
        if let Some(bytes)=form_bytes {
            copy = self.pdfium.load_pdf_from_reader(Cursor::new(bytes),None)?;
        }
        if text_context_group::read(&copy.save_to_bytes()?, 0)? != group {
            return Err(invalid("The text page copy changed its transparency group."));
        }
        Ok(copy)
    }

    /// Prove that FPDFText_SetText does not discard TJ positioning or text state.
    /// Testing a no-op through save/reopen also catches encodings that cannot roundtrip.
    fn prove_editable(
        &self,
        baseline: &[u8],
        before: &[RunSnapshot],
        index: usize,
        preserve_stream: bool,
    ) -> EngineResult<()> {
        let run = before
            .iter()
            .find(|r| r.index == index)
            .ok_or_else(|| invalid("The selected text run no longer exists."))?;
        let source_value = source_text::selected(baseline, before, index);
        let source_run = source_edit_run(run, source_value.as_deref());
        if let Some(reason) = &source_run.reason {
            return Err(invalid(reason));
        }
        let copy = self.pdfium.load_pdf_from_byte_slice(baseline, None)?;
        let original_glyphs = glyph_snapshot(&copy.pages().get(0)?)?;
        let original_raster = raster(&copy.pages().get(0)?)?;
        let bytes = if preserve_stream {
            positioned_text::replace(baseline, &source_run, before, &source_run.text)?
        } else {
            replace_in_page(&copy, index, &source_run.text)?;
            copy.save_to_bytes()?
        };
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
        self.list_transaction_text_runs(source_id, page_index, false)
    }

    pub(super) fn list_transaction_text_runs(
        &self, source_id: &str, page_index: usize, complete_source: bool,
    ) -> EngineResult<TextRuns> {
        let _phase=crate::native_phase!("discovery.total");
        let _reuse=complete_source.then(crate::request_parse::Scope::enter);
        if let Some(reason) = self.source_text_edit_restriction(source_id)? {
            // Still validate the requested page even when the document is unsupported.
            self.editing_document(source_id)?
                .pdf()?
                .pages()
                .get(page_index_i32(page_index)?)?;
            return Ok(TextRuns {
                runs: vec![],
                reason: Some(reason),
            });
        }
        let copy = self.text_transaction_page_copy(source_id, page_index, complete_source)?;
        let page = copy.pages().get(0)?;
        let geometry = page_geometry(&page)?;
        let baseline = {let _phase=crate::native_phase!("discovery.serialize");copy.save_to_bytes()?};
        let proofs = {let _phase=crate::native_phase!("discovery.font_proofs");FontProofs::read(&baseline)?};
        let before = {let _phase=crate::native_phase!("discovery.snapshot");bound_snapshot(&page, Some(&proofs), &baseline)?};
        let source_values = {let _phase=crate::native_phase!("discovery.source_mapping");source_text::values(&baseline, &before).unwrap_or_default()};
        let substitution_admission={let _phase=crate::native_phase!("discovery.substitution_admission");if complete_source {text_substitution::admit_all(&baseline,&before,&source_values).unwrap_or_default()}else{vec![]}};
        let contexts = if complete_source {
            let source=self.editing_document(source_id)?;
            text_context_edit::SourcePage::read(source.bytes()?.as_ref(),page_index,&source.pdf()?.pages().get(page_index_i32(page_index)?)?)
        } else {Ok(None)};
        let page_proof=contexts.as_ref().ok().and_then(|p|p.as_ref()).and_then(|p|before.iter().find_map(|r|p.proof(r.index).ok()));
        let copy_context_reason=page_proof.as_ref().and_then(|p|p.baseline(&baseline,&before).err()).map(|e|e.to_string());
        let can_substitute_page = page_proof.is_none() && preflight(&baseline, &proofs, true).is_ok();
        let page_reason = preflight_with_context(&baseline, &proofs, false, page_proof.as_ref()).err().map(|e| match e {
            EngineError::InvalidRequest(s) => s,
            _ => "This page could not be validated for editing.".into(),
        }).or(copy_context_reason);
        let mut runs = Vec::new();
        for (ordinal, run) in before.iter().enumerate() {
            let value = source_values.get(ordinal).and_then(|v|v.as_deref());
            let context_reason = match &contexts {
                Ok(Some(contexts))=>contexts.proof(run.index).err().map(|e|e.to_string()),
                Err(error)=>Some(error.to_string()),
                Ok(None)=>None,
            };
            let reason = source_edit_run(run, value).reason.or(context_reason).or_else(|| page_reason.clone());
            let p1 = geometry.pdf_to_displayed(run.bounds[0], run.bounds[1]);
            let p2 = geometry.pdf_to_displayed(run.bounds[2], run.bounds[3]);
            runs.push(EditableTextRun {
                object_index: run.index,
                text: run.text.clone(),
                source_text: source_values.get(ordinal).and_then(|v|v.clone()).filter(|value|value != &run.text),
                font_name: run.font.clone(),
                font_display_name: run.proof.as_ref().and_then(|p|p.display_name.clone()),
                font_size: run.font_size,
                display_font_size: run.display_font_size(),
                bounds: AnnotationRect {
                    x: p1.x.min(p2.x),
                    y: p1.y.min(p2.y),
                    width: (p1.x - p2.x).abs(),
                    height: (p1.y - p2.y).abs(),
                },
                supported: reason.is_none(),
                is_embedded: run.embedded,
                can_substitute: reason.is_none() && if complete_source {
                    substitution_admission.get(ordinal).copied().unwrap_or(false)
                } else {can_substitute_page},
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
        self.replace_transaction_text(source_id, page_index, object_index, expected_text, replacement, font_id, false)
    }

    /// Only occurrence_text_edit may use complete_source=true. Its isolated
    /// result must be discarded after exact publication into the original graph.
    pub(super) fn replace_transaction_text(
        &mut self, source_id: &str, page_index: usize, object_index: usize,
        expected_text: &str, replacement: &str, font_id: Option<&str>, complete_source: bool,
    ) -> EngineResult<DocumentInfo> {
        let _phase=crate::native_phase!("isolated_edit.total");
        if let Some(reason) = self.source_text_edit_restriction(source_id)? {
            return Err(invalid(&reason));
        }
        let copy = self.text_transaction_page_copy(source_id, page_index, complete_source)?;
        let baseline = copy.save_to_bytes()?;
        let proofs = FontProofs::read(&baseline)?;
        let before = bound_snapshot(&copy.pages().get(0)?, Some(&proofs), &baseline)?;
        let selected = before
            .iter()
            .find(|r| r.index == object_index)
            .ok_or_else(|| invalid("The selected text run no longer exists."))?;
        if selected.text != expected_text {
            return Err(invalid(
                "The text changed since it was selected. Select the run again.",
            ));
        }
        if !(if selected.embedded || selected.standard.is_some() || font_id.is_some() {
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
        } else if let Some(proof) = &selected.standard {
            proof.encode(replacement)?;
        }
        let mut context=if complete_source {self.single_text_context(source_id,page_index,object_index)?}else{None};
        if let Some(context)=&context {context.baseline(&baseline,&before)?;}
        let mut planned=if complete_source {substitute.as_ref().map(|font| {
            let source_value=source_text::selected(&baseline,&before,object_index);
            let source_run=source_edit_run(selected,source_value.as_deref());
            text_substitution::Plan::prepare(&baseline,&source_run,&before,replacement,font)
        }).transpose()?}else{None};
        let planned_substitution=planned.is_some();
        if let (Some(context),Some(plan))=(&mut context,&planned){context.bind_substitution(plan)?;}
        let preserve_state = preflight_with_context(&baseline, &proofs, substitute.is_some()&&planned.is_none(), context.as_ref())?;
        let preserve_encoding = (selected.standard.is_some()
            && (!selected.text.is_ascii() || !replacement.is_ascii()))
            || selected.proof.as_ref().is_some_and(|proof| proof.preserve_source_encoding);
        // Native regeneration changes every Tj to TJ and renames font resources,
        // invalidating independently verified paragraph bindings elsewhere. In a
        // complete-source transaction keep those operators and resource names.
        let preserve_memory = if complete_source {
            let pdf=crate::pdf_output::load(&baseline)?;
            let page=*pdf.get_pages().values().next().ok_or_else(||invalid("Missing text page."))?;
            let page=pdf.get_dictionary(page).map_err(|_|invalid("Missing text page."))?;
            [(b"FolioParagraphs".as_slice(),512*1024),(b"FolioStyledParagraphs".as_slice(),4*1024*1024)].iter()
                .any(|(key,limit)|page.get(key).ok().and_then(|v|v.as_str().ok()).is_some_and(|bytes|!bytes.is_empty()&&bytes.len()<=*limit))
        } else {false};
        if complete_source && substitute.is_none() && (preserve_state || preserve_memory || preserve_encoding) {
            let source_value = source_text::selected(&baseline, &before, object_index);
            let source_run = source_edit_run(selected, source_value.as_deref());
            let plan = text_substitution::Plan::prepare_original(&baseline, &source_run, &before, replacement)?;
            if let Some(context) = &mut context { context.bind_substitution(&plan)?; }
            planned = Some(plan);
        }
        // A no-op cannot expose dormant Tc/Tw or a trailing TJ gap. Retain
        // those operators even when their current string shows no displacement.
        // On ordinary pages keep native editing, which preserves the absolute
        // positions of following relative text objects and proven backgrounds.
        let positioned = if (preserve_state || preserve_memory || preserve_encoding) && substitute.is_none() && planned.is_none() {
            let source_value = source_text::selected(&baseline, &before, object_index);
            let source_run = source_edit_run(selected, source_value.as_deref());
            let bytes = positioned_text::replace(&baseline, &source_run, &before, replacement)?;
            self.prove_editable(&baseline, &before, object_index, true)?;
            Some(bytes)
        } else {
            self.prove_editable(&baseline, &before, object_index, preserve_memory||planned.is_some())?;
            None
        };
        let original_raster = raster(
            &self
                .editing_document(source_id)?
                .pdf()?
                .pages()
                .get(page_index_i32(page_index)?)?,
        )?;
        if original_raster != raster(&copy.pages().get(0)?)? {
            return Err(invalid(
                "Copying this page would change its appearance. Its text cannot be edited safely.",
            ));
        }
        let bytes: Arc<[u8]> = if let Some(plan)=planned {plan.into_bytes().into()} else if let Some(font) = &substitute {
            // Normalize the selected stream through the already-proven no-op.
            // New PDFium text objects save into a new trailing stream and can
            // reorder content; retain the original operator slot instead.
            let source_value = source_text::selected(&baseline, &before, object_index);
            let source_run = source_edit_run(selected, source_value.as_deref());
            let substitution_source=if preserve_memory {baseline.clone()}else{
                replace_in_page(&copy, object_index, &source_run.text)?;
                copy.save_to_bytes()?
            };
            let ordinal = before
                .iter()
                .position(|run| run.index == object_index)
                .unwrap();
            substitute_in_bytes(
                &substitution_source,
                &source_run,
                ordinal,
                before.len(),
                replacement,
                font,
            )?
            .into()
        } else if let Some(bytes) = positioned {
            bytes.into()
        } else {
            replace_in_page(&copy, object_index, replacement)?;
            copy.save_to_bytes()?.into()
        };
        let bytes = self.preserve_text_edit_comments(source_id, page_index, bytes)?;
        let document = self
            .pdfium
            .load_pdf_from_reader(Cursor::new(bytes.clone()), None)?;
        let page = document.pages().get(0)?;
        let after_proofs = FontProofs::read(&bytes)?;
        let after = bound_snapshot(&page, Some(&after_proofs), &bytes)?;
        if let Some(context)=&context {context.candidate(&bytes,&page,&after)?;}
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
        let changed_source = source_text::selected(&bytes, &after, object_index);
        let changed_reason = source_edit_run(changed, changed_source.as_deref()).reason;
        if let Some(reason) = &changed_reason {
            return Err(invalid(reason));
        }
        if !style_preserved
            || changed_reason.is_some()
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
            self.prove_editable(&bytes, &after, object_index, planned_substitution)?;
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
                return Err(invalid("The replacement would overlap neighboring text. Shorten it to fit the available space. Editing existing text does not reflow the paragraph."));
            }
        }
        for (index, object) in page.objects().iter().enumerate() {
            if object.as_text_object().is_some() {
                continue;
            }
            // Only a proven solid rectangle behind the selected run is exempt.
            // Page-covering bounds alone can hide interior artwork or holes.
            let background = index < object_index && solid_page_background(&object, geometry)?;
            if !background && occupied_regions(&object, geometry)?.iter()
                .any(|bounds| adds_overlap(&selected.bounds, &changed.bounds, bounds)) {
                return Err(invalid(
                    "The replacement would overlap neighboring graphics.",
                ));
            }
        }
        // Overlapping glyphs can change PDFium's extracted object text even
        // though the encoded replacement is intact. Report the concrete fit
        // failure first, but still require exact text before publishing an edit.
        if changed_source.as_deref().unwrap_or(&changed.text) != replacement {
            return Err(invalid("The edited PDF did not retain the exact replacement text. The original page has been kept."));
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
        let source = self.editing_document(source_id)?;
        let path = source.path.clone();
        let original_path = source.original_path.clone();
        let policy=source.policy.as_ref().map(|p|p.page(page_index)).transpose()?;
        let protection=policy.as_ref().map(SourcePolicy::info);
        let id = Uuid::new_v4().to_string();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document.pdf")
            .to_string();
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
        Ok(DocumentInfo { id, name, pages,bookmarks:vec![],bookmark_error:None,protection })
    }
}

#[path = "form_text.rs"]
mod form_text;
