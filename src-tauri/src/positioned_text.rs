//! Preserve a uniquely mapped text-show operator and its inherited PDF state.
//! Explicit TJ gaps stay attached to their original character boundaries. New
//! characters extend the final segment; gaps beyond a shortened string vanish.
use super::*;
use lopdf::{content::Content, dictionary, Document, Object, Stream, StringFormat};

const LIMIT: usize = 16 * 1024 * 1024;

// Numeric-only TJ moves the text cursor but creates no PDFium text object.
// Keep this classification shared by all top-level operator/object mappings.
pub(super) fn cursor_only(op: &lopdf::content::Operation) -> bool {
    matches!((op.operator.as_str(), op.operands.as_slice()), ("TJ", [Object::Array(values)])
        if values.iter().all(|v| v.as_float().is_ok_and(f32::is_finite)))
}

pub(super) fn encoded(run: &RunSnapshot, value: &str) -> EngineResult<Vec<u8>> {
    if let Some(proof) = &run.proof {
        proof.encode(value)
    } else if let Some(proof) = &run.standard {
        proof.encode(value)
    } else if !run.embedded && value.len() <= 4096 && value.bytes().all(|c| (32..=126).contains(&c))
    {
        Ok(value.as_bytes().to_vec())
    } else {
        Err(invalid(
            "The positioned text has no verified character encoding.",
        ))
    }
}

pub(super) fn replace(
    bytes: &[u8],
    selected: &RunSnapshot,
    runs: &[RunSnapshot],
    replacement: &str,
) -> EngineResult<Vec<u8>> {
    let mut pdf = Document::load_mem_with_options(
        bytes,
        lopdf::LoadOptions::with_max_decompressed_size(LIMIT),
    )
    .map_err(|_| invalid("The positioned page could not be decoded."))?;
    let pages = pdf.get_pages();
    if pages.len() != 1 {
        return Err(invalid("Positioned editing requires an isolated page."));
    }
    let page_id = *pages.values().next().unwrap();
    let data = pdf
        .get_page_content_with_limit(page_id, LIMIT)
        .map_err(|_| invalid("The positioned page exceeds the content limit."))?;
    let mut content =
        Content::decode(&data).map_err(|_| invalid("The positioned text stream is malformed."))?;
    if content.operations.len() > 100_000 {
        return Err(invalid("The positioned page has too many operations."));
    }
    let mut current = page_id;
    let mut resources = None;
    for _ in 0..64 {
        let node = pdf
            .get_dictionary(current)
            .map_err(|_| invalid("Malformed page resources."))?;
        if let Ok(value) = node.get(b"Resources") {
            resources = Some(
                pdf.dereference(value)
                    .and_then(|(_, v)| v.as_dict())
                    .map_err(|_| invalid("Malformed page resources."))?,
            );
            break;
        }
        current = node
            .get(b"Parent")
            .and_then(Object::as_reference)
            .map_err(|_| invalid("Missing page resources."))?;
    }
    let fonts = pdf
        .dereference(
            resources
                .ok_or_else(|| invalid("Page resource nesting exceeds the limit."))?
                .get(b"Font")
                .map_err(|_| invalid("Missing page fonts."))?,
        )
        .and_then(|(_, v)| v.as_dict())
        .map_err(|_| invalid("Malformed page fonts."))?;
    let ordinal = runs
        .iter()
        .position(|r| r.index == selected.index)
        .ok_or_else(|| invalid("The selected positioned run is missing."))?;
    let mut active_font: Option<(Vec<u8>, f32)> = None;
    let mut stack = Vec::new();
    let mut seen = 0;
    let mut target = None;
    let mut in_text = false;
    for (index, op) in content.operations.iter().enumerate() {
        match op.operator.as_str() {
            "q" => {
                if stack.len() >= 64 {
                    return Err(invalid("Graphics state nesting exceeds the limit."));
                }
                stack.push(active_font.clone());
            }
            "Q" => {
                active_font = stack
                    .pop()
                    .ok_or_else(|| invalid("Unbalanced graphics state."))?
            }
            "BT" => {
                if in_text {
                    return Err(invalid("Nested text objects are not supported."));
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
                    return Err(invalid("Malformed text font selection."));
                }
                let name = op.operands[0]
                    .as_name()
                    .map_err(|_| invalid("Malformed font name."))?;
                let size = op.operands[1]
                    .as_float()
                    .map_err(|_| invalid("Malformed font size."))?;
                if !size.is_finite() || size <= 0.0 || size > 10_000.0 {
                    return Err(invalid("Font size exceeds positioned editing bounds."));
                }
                active_font = Some((name.to_vec(), size));
            }
            "Tj" | "TJ" | "'" | "\"" => {
                if !in_text {
                    return Err(invalid("Text outside a text object is not supported."));
                }
                if cursor_only(op) {
                    continue;
                }
                if seen == ordinal {
                    let (name, size) = active_font
                        .as_ref()
                        .ok_or_else(|| invalid("Missing text font selection."))?;
                    let font = pdf
                        .dereference(
                            fonts
                                .get(name)
                                .map_err(|_| invalid("Missing font resource."))?,
                        )
                        .and_then(|(_, v)| v.as_dict())
                        .map_err(|_| invalid("Malformed font resource."))?;
                    if !embedded_font::matches_name(&pdf, font, &selected.font, selected.embedded)
                        || (size - selected.font_size).abs() > EPSILON
                    {
                        return Err(invalid(
                            "The positioned font does not match the selected run.",
                        ));
                    }
                    target = Some(index);
                }
                seen += 1;
            }
            _ => {}
        }
    }
    if seen != runs.len() || !stack.is_empty() || in_text {
        return Err(invalid(
            "Text operators cannot be mapped uniquely to page objects.",
        ));
    }
    let op = &mut content.operations[target.ok_or_else(|| invalid("Missing text operator."))?];
    let expected = encoded(selected, &selected.text)?;
    let changed = encoded(selected, replacement)?;
    match (op.operator.as_str(), op.operands.as_mut_slice()) {
        ("Tj" | "'", [value]) | ("\"", [_, _, value]) => {
            if value.as_str().ok() != Some(expected.as_slice()) {
                return Err(invalid("Ambiguous positioned text mapping."));
            }
            *value = Object::String(changed, StringFormat::Hexadecimal);
        }
        ("TJ", [Object::Array(values)]) => {
            let source_boundaries = text_show::boundaries(&selected.text, |c| encoded(selected, c))?;
            let replacement_boundaries = text_show::boundaries(replacement, |c| encoded(selected, c))?;
            *values = text_show::rewrite_array(values, &expected, &changed,
                &source_boundaries, &replacement_boundaries, replacement == selected.text)?;
        }
        _ => return Err(invalid("Unsupported positioned text operator.")),
    }
    let data = content
        .encode()
        .map_err(|_| invalid("The positioned text could not be encoded."))?;
    if data.len() > LIMIT {
        return Err(invalid("Edited content exceeds the limit."));
    }
    let id = pdf.add_object(Stream::new(dictionary! {}, data));
    pdf.get_dictionary_mut(page_id)
        .map_err(|_| invalid("Missing copied page."))?
        .set("Contents", id);
    let mut result = Vec::new();
    pdf.save_to(&mut result)
        .map_err(|_| invalid("The positioned page could not be saved."))?;
    Ok(result)
}
