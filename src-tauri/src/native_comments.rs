//! Review native Text annotations without rebuilding their icons or popups.
use super::*;
use lopdf::{Dictionary, Document, Object, ObjectId, StringFormat};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeComment {
    pub page_index: usize,
    pub annotation_index: usize,
    pub text: String,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub bounds: AnnotationRect,
    pub reason: Option<String>,
    pub color: Option<String>,
    pub can_change_color: bool,
    pub delete_reason: Option<String>,
    pub on_page: bool,
    pub hit_bounds: AnnotationRect,
    pub no_rotate: bool,
    #[serde(skip)]
    default_appearance: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditNativeCommentRequest {
    pub page_index: usize,
    pub annotation_index: usize,
    pub expected_text: String,
    pub text: String,
    #[serde(default)]
    pub position: Option<Point>,
    #[serde(default)]
    pub color: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteNativeCommentRequest {
    pub page_index: usize,
    pub annotation_index: usize,
    pub expected_text: String,
}
fn bad(message: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("PDF comments: {message}"))
}
fn load(bytes: &[u8]) -> EngineResult<Document> {
    Document::load_mem_with_options(
        bytes,
        lopdf::LoadOptions {
            strict: true,
            max_decompressed_size: Some(64 * 1024 * 1024),
            ..Default::default()
        },
    )
    .map_err(bad)
}
fn resolve<'a>(doc: &'a Document, mut value: &'a Object) -> EngineResult<&'a Object> {
    let mut seen = HashSet::new();
    while let Object::Reference(id) = value {
        if seen.len() >= 64 || !seen.insert(*id) {
            return Err(bad("cyclic or deep reference"));
        }
        value = doc
            .objects
            .get(id)
            .ok_or_else(|| bad("missing referenced object"))?;
    }
    Ok(value)
}
fn text(doc: &Document, data: &Dictionary, key: &[u8]) -> EngineResult<Option<String>> {
    let Ok(value) = data.get(key) else {
        return Ok(None);
    };
    let value = resolve(doc, value)?;
    if matches!(value, Object::Null) {
        return Ok(None);
    }
    let bytes = value.as_str().map_err(bad)?;
    if bytes.len() > 2_000_002 {
        return Err(bad("comment metadata exceeds the text size limit"));
    }
    let result = if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        std::str::from_utf8(&bytes[3..]).map_err(bad)?.to_owned()
    } else {
        lopdf::decode_text_string(value).map_err(bad)?
    };
    validate_annotation_text(&result)?;
    Ok(Some(result))
}
fn annotations(doc: &Document, page: ObjectId) -> EngineResult<&Vec<Object>> {
    let data = doc.get_dictionary(page).map_err(bad)?;
    resolve(doc, data.get(b"Annots").map_err(bad)?)?
        .as_array()
        .map_err(bad)
}
fn color(doc: &Document, data: &Dictionary) -> Option<String> {
    let Ok(value) = data.get(b"C") else {
        return Some("#ffcc00".into());
    };
    let values = resolve(doc, value)
        .ok()?
        .as_array()
        .ok()?
        .iter()
        .map(|v| resolve(doc, v).ok()?.as_float().ok())
        .collect::<Option<Vec<_>>>()?;
    if values
        .iter()
        .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
    {
        return None;
    }
    let rgb = match values.as_slice() {
        [g] => [*g, *g, *g],
        [r, g, b] => [*r, *g, *b],
        [c, m, y, k] => [
            (1.0 - c) * (1.0 - k),
            (1.0 - m) * (1.0 - k),
            (1.0 - y) * (1.0 - k),
        ],
        _ => return None,
    };
    Some(format!(
        "#{:02x}{:02x}{:02x}",
        (rgb[0] * 255.0).round() as u8,
        (rgb[1] * 255.0).round() as u8,
        (rgb[2] * 255.0).round() as u8
    ))
}
fn reply_targets(doc: &Document) -> EngineResult<HashSet<ObjectId>> {
    let mut targets = HashSet::new();
    let mut count = 0usize;
    for page in doc.get_pages().values() {
        if !doc.get_dictionary(*page).map_err(bad)?.has(b"Annots") {
            continue;
        }
        let values = annotations(doc, *page)?;
        count = count.saturating_add(values.len());
        if count > 100_000 {
            return Err(bad("document exceeds 100000 annotations"));
        }
        for value in values {
            let data = resolve(doc, value)?.as_dict().map_err(bad)?;
            if let Ok(parent) = data.get(b"IRT") {
                if let Some(id) = terminal_reference(doc, parent)? {
                    targets.insert(id);
                }
            }
        }
    }
    Ok(targets)
}
fn signature_reason(doc: &Document) -> EngineResult<Option<String>> {
    // References are scanned once as entries in doc.objects; do not recursively
    // expand shared resources or cycles. Direct nested dictionaries still count.
    let mut stack: Vec<_> = doc.objects.values().map(|v| (v, 0usize)).collect();
    let mut visited = 0usize;
    let mut signed = doc.catalog().map_err(bad)?.has(b"Perms");
    while let Some((value, depth)) = stack.pop() {
        visited += 1;
        if visited > 1_000_000 || depth > 64 {
            return Err(bad("document metadata exceeds the inspection limit"));
        }
        let data = match value {
            Object::Dictionary(d) => Some(d),
            Object::Stream(s) => Some(&s.dict),
            Object::Array(a) => {
                stack.extend(a.iter().map(|v| (v, depth + 1)));
                None
            }
            _ => None,
        };
        if let Some(data) = data {
            let kind = data
                .get(b"Type")
                .ok()
                .map(|v| resolve(doc, v))
                .transpose()?;
            signed |= data.has(b"ByteRange")
                || kind.is_some_and(|v| v.as_name().is_ok_and(|n| n == b"Sig"));
            stack.extend(data.iter().map(|(_, v)| (v, depth + 1)));
        }
    }
    Ok(signed.then(|| {
        "Signed or certified PDFs cannot be rewritten without invalidating their signatures.".into()
    }))
}

fn read(
    doc: &Document,
    excluded: &HashMap<usize, HashSet<usize>>,
) -> EngineResult<Vec<NativeComment>> {
    if doc.objects.len() > 100_000 {
        return Err(bad("document exceeds 100000 objects"));
    }
    let global_reason = signature_reason(doc)?;
    let reply_targets = reply_targets(doc)?;
    let mut result = vec![];
    let mut count = 0usize;
    let mut bytes = 0usize;
    for (page_index, page_id) in doc.get_pages().values().enumerate() {
        if !doc.get_dictionary(*page_id).map_err(bad)?.has(b"Annots") {
            continue;
        }
        let annots = annotations(doc, *page_id)?;
        count = count.saturating_add(annots.len());
        if count > 100_000 {
            return Err(bad("document exceeds 100000 annotations"));
        }
        for (annotation_index, value) in annots.iter().enumerate() {
            if excluded
                .get(&page_index)
                .is_some_and(|indices| indices.contains(&annotation_index))
            {
                continue;
            }
            let data = resolve(doc, value)?.as_dict().map_err(bad)?;
            let subtype = data
                .get(b"Subtype")
                .ok()
                .map(|v| resolve(doc, v))
                .transpose()?;
            if !subtype.is_some_and(|v| v.as_name().is_ok_and(|s| s == b"Text")) {
                continue;
            }
            if result.len() >= 10_000 {
                return Err(bad("document exceeds 10000 native comments"));
            }
            let contents = text(doc, data, b"Contents")?.unwrap_or_default();
            let author = text(doc, data, b"T")?;
            let subject = text(doc, data, b"Subj")?;
            bytes = bytes.saturating_add(
                contents.len()
                    + author.as_ref().map_or(0, String::len)
                    + subject.as_ref().map_or(0, String::len),
            );
            if bytes > 16 * 1024 * 1024 {
                return Err(bad("comment text exceeds 16 MiB"));
            }
            let flags = data
                .get(b"F")
                .ok()
                .map(|v| resolve(doc, v)?.as_i64().map_err(bad))
                .transpose()?
                .unwrap_or(0);
            if flags < 0 || flags > u32::MAX as i64 {
                return Err(bad("invalid annotation flags"));
            }
            let reason = global_reason.clone().or_else(|| {
                if flags & (64 | 128 | 512) != 0 {
                    Some("This comment is locked or read-only.")
                } else if flags & (1 | 2 | 32 | 256) != 0 {
                    Some("This comment has visibility behavior that cannot be edited yet.")
                } else if data.has(b"RC") {
                    Some("Rich-text comments can be read here but cannot be edited yet.")
                } else if data.has(b"IRT") || data.has(b"State") || data.has(b"StateModel") {
                    Some("Comment replies and review states cannot be edited yet.")
                } else {
                    None
                }
                .map(str::to_owned)
            });
            let rect = resolve(doc, data.get(b"Rect").map_err(bad)?)?
                .as_array()
                .map_err(bad)?;
            if rect.len() != 4 {
                return Err(bad("invalid comment rectangle"));
            }
            let values = rect
                .iter()
                .map(|v| resolve(doc, v)?.as_float().map_err(bad))
                .collect::<EngineResult<Vec<_>>>()?;
            if values
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 10_000_000.0)
            {
                return Err(bad("invalid comment coordinates"));
            }
            let delete_reason = reason.clone().or_else(|| {
                terminal_reference(doc, value)
                    .ok()
                    .flatten()
                    .filter(|id| reply_targets.contains(id))
                    .map(|_| {
                        "This comment has replies. Removing comment threads is not supported yet."
                            .into()
                    })
            });
            let has_appearance = data
                .get(b"AP")
                .ok()
                .map(|v| resolve(doc, v))
                .transpose()?
                .is_some_and(|v| !matches!(v, Object::Null));
            result.push(NativeComment {
                page_index,
                annotation_index,
                text: contents,
                author,
                subject,
                reason,
                color: color(doc, data),
                can_change_color: !has_appearance,
                delete_reason,
                on_page: flags & (1 | 2 | 32 | 256) == 0,
                no_rotate: flags & 16 != 0,
                default_appearance: !has_appearance,
                hit_bounds: AnnotationRect {
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 0.0,
                },
                bounds: AnnotationRect {
                    x: values[0].min(values[2]),
                    y: values[1].min(values[3]),
                    width: (values[2] - values[0]).abs(),
                    height: (values[3] - values[1]).abs(),
                },
            });
        }
    }
    Ok(result)
}
// Mutate the terminal object while retaining every indirect reference and alias.
fn terminal_reference<'a>(
    doc: &'a Document,
    mut value: &'a Object,
) -> EngineResult<Option<ObjectId>> {
    let mut last = None;
    let mut seen = HashSet::new();
    while let Object::Reference(id) = value {
        if seen.len() >= 64 || !seen.insert(*id) {
            return Err(bad("cyclic or deep reference"));
        }
        last = Some(*id);
        value = doc
            .objects
            .get(id)
            .ok_or_else(|| bad("missing referenced object"))?;
    }
    Ok(last)
}
fn set_entries(doc: &mut Document, value: &mut Object, entries: Dictionary) -> EngineResult<()> {
    let data = if let Some(id) = terminal_reference(doc, value)? {
        doc.objects
            .get_mut(&id)
            .ok_or_else(|| bad("missing annotation"))?
            .as_dict_mut()
            .map_err(bad)?
    } else {
        value.as_dict_mut().map_err(bad)?
    };
    for (key, value) in entries {
        data.set(key, value);
    }
    Ok(())
}

pub(super) struct SourceComments<'a> {
    pub source: &'a crate::acroforms::SourceForm<'a>,
    pub excluded: &'a HashMap<usize, HashSet<usize>>,
}

/// Rendering a default Text icon mutates PDFium's Rect/AP in memory. Composition
/// restores native geometry and appearance ownership from immutable source bytes.
/// Retained annotation order, subtype and text are checked before changing fields.
pub(super) fn preserve_geometry(
    output: &mut Document,
    sources: &[SourceComments<'_>],
) -> EngineResult<bool> {
    let output_pages: Vec<_> = output.get_pages().values().copied().collect();
    let mut changed = false;
    for source in sources {
        let original = source.source.document()?;
        let source_pages: Vec<_> = original.get_pages().values().copied().collect();
        for &(source_index, output_index) in &source.source.pages {
            let source_page = *source_pages
                .get(source_index)
                .ok_or_else(|| bad("missing source comment page"))?;
            if !original
                .get_dictionary(source_page)
                .map_err(bad)?
                .has(b"Annots")
            {
                continue;
            }
            let kept: Vec<_> = annotations(&original, source_page)?
                .iter()
                .enumerate()
                .filter(|(i, _)| {
                    !source
                        .excluded
                        .get(&source_index)
                        .is_some_and(|set| set.contains(i))
                })
                .collect();
            if !kept.iter().any(|(_, v)| {
                resolve(&original, v)
                    .ok()
                    .and_then(|v| v.as_dict().ok())
                    .is_some_and(|d| {
                        d.get(b"Subtype")
                            .ok()
                            .and_then(|v| resolve(&original, v).ok())
                            .is_some_and(|v| v.as_name().is_ok_and(|n| n == b"Text"))
                    })
            }) {
                continue;
            }
            let output_page = *output_pages
                .get(output_index)
                .ok_or_else(|| bad("missing output comment page"))?;
            let mut destination = annotations(output, output_page)?.clone();
            if destination.len() < kept.len() {
                return Err(bad("copied annotations are missing"));
            }
            for (output_annotation, (_, original_value)) in kept.iter().enumerate() {
                let data = resolve(&original, original_value)?.as_dict().map_err(bad)?;
                if !data
                    .get(b"Subtype")
                    .ok()
                    .map(|v| resolve(&original, v))
                    .transpose()?
                    .is_some_and(|v| v.as_name().is_ok_and(|n| n == b"Text"))
                {
                    continue;
                }
                let target = resolve(output, &destination[output_annotation])?
                    .as_dict()
                    .map_err(bad)?;
                if !target
                    .get(b"Subtype")
                    .ok()
                    .map(|v| resolve(output, v))
                    .transpose()?
                    .is_some_and(|v| v.as_name().is_ok_and(|n| n == b"Text"))
                    || text(&original, data, b"Contents")? != text(output, target, b"Contents")?
                {
                    return Err(bad("copied comment identity changed"));
                }
                let rect = resolve(&original, data.get(b"Rect").map_err(bad)?)?
                    .as_array()
                    .map_err(bad)?;
                let rect = Object::Array(
                    rect.iter()
                        .map(|v| resolve(&original, v).cloned())
                        .collect::<EngineResult<Vec<_>>>()?,
                );
                let remove_ap = !data
                    .get(b"AP")
                    .ok()
                    .map(|v| resolve(&original, v))
                    .transpose()?
                    .is_some_and(|v| !matches!(v, Object::Null));
                let mut entries = Dictionary::new();
                entries.set("Rect", rect);
                set_entries(output, &mut destination[output_annotation], entries)?;
                if remove_ap {
                    let target = if let Some(id) =
                        terminal_reference(output, &destination[output_annotation])?
                    {
                        output.get_dictionary_mut(id).map_err(bad)?
                    } else {
                        destination[output_annotation].as_dict_mut().map_err(bad)?
                    };
                    if data.has(b"AP") {
                        target.set("AP", Object::Null);
                    } else {
                        target.remove(b"AP");
                    }
                    if !data.has(b"PDFIUM_HasGeneratedAP") {
                        target.remove(b"PDFIUM_HasGeneratedAP");
                    }
                }
                changed = true;
            }
            output
                .get_dictionary_mut(output_page)
                .map_err(bad)?
                .set("Annots", destination);
        }
    }
    Ok(changed)
}

/// PDFium's page copy can turn Popup.Parent into a self-reference. The unique
/// forward Text.Popup link identifies the owner without relying on object numbers
/// from a different document. Run before forms/overlay finalization and publication.
pub(super) fn repair_popup_parents(doc: &mut Document) -> EngineResult<bool> {
    let mut owners = HashMap::new();
    for page in doc.get_pages().values() {
        let data = doc.get_dictionary(*page).map_err(bad)?;
        let Ok(annots) = data.get(b"Annots") else {
            continue;
        };
        let annots = resolve(doc, annots)?;
        if matches!(annots, Object::Null) {
            continue;
        }
        for value in annots.as_array().map_err(bad)? {
            let data = resolve(doc, value)?.as_dict().map_err(bad)?;
            if !data.get(b"Subtype").is_ok_and(|v| {
                resolve(doc, v).is_ok_and(|v| v.as_name().is_ok_and(|n| n == b"Text"))
            }) {
                continue;
            }
            let Ok(popup) = data.get(b"Popup") else {
                continue;
            };
            if matches!(resolve(doc, popup)?, Object::Null) {
                continue;
            }
            let owner = terminal_reference(doc, value)?
                .ok_or_else(|| bad("popup owner has no indirect identity"))?;
            let popup_id = terminal_reference(doc, popup)?
                .ok_or_else(|| bad("popup has no indirect identity"))?;
            let popup_data = resolve(doc, popup)?.as_dict().map_err(bad)?;
            if !popup_data.get(b"Subtype").is_ok_and(|v| {
                resolve(doc, v).is_ok_and(|v| v.as_name().is_ok_and(|n| n == b"Popup"))
            }) {
                return Err(bad("comment popup does not identify a Popup annotation"));
            }
            if owners
                .insert(popup_id, owner)
                .is_some_and(|previous| previous != owner)
            {
                return Err(bad("one popup is linked to multiple comment owners"));
            }
            if owners.len() > 10_000 {
                return Err(bad("export exceeds 10000 native comment popups"));
            }
        }
    }
    let mut changed = false;
    for (popup, owner) in owners {
        let data = doc.get_dictionary(popup).map_err(bad)?;
        // PDFium can leave a dangling Parent when importing only one page from
        // an edited multipage source. The validated, unique Text.Popup forward
        // link above is authoritative even when this copied backlink is broken.
        let parent = data
            .get(b"Parent")
            .ok()
            .and_then(|v| terminal_reference(doc, v).ok())
            .flatten();
        if parent != Some(owner) {
            doc.get_dictionary_mut(popup)
                .map_err(bad)?
                .set("Parent", Object::Reference(owner));
            changed = true;
        }
    }
    Ok(changed)
}

fn edit(
    bytes: &[u8],
    excluded: &HashMap<usize, HashSet<usize>>,
    request: &EditNativeCommentRequest,
    geometry: PageGeometry,
) -> EngineResult<Vec<u8>> {
    validate_annotation_text(&request.text)?;
    let mut doc = load(bytes)?;
    let notes = read(&doc, excluded)?;
    let note = notes
        .iter()
        .find(|n| {
            n.page_index == request.page_index && n.annotation_index == request.annotation_index
        })
        .ok_or_else(|| bad("comment is no longer available"))?;
    if let Some(reason) = &note.reason {
        return Err(bad(reason));
    }
    if note.text != request.expected_text {
        return Err(bad("comment changed; reopen it before editing"));
    }
    // Reuse the full-source form preflight to reject malformed/signed field trees
    // or XFA before any rewrite. No form value or appearance is regenerated.
    crate::acroforms::filling_document(bytes)?;
    let page = *doc
        .get_pages()
        .values()
        .nth(request.page_index)
        .ok_or_else(|| bad("page is no longer available"))?;
    let annots_value = doc
        .get_dictionary(page)
        .map_err(bad)?
        .get(b"Annots")
        .map_err(bad)?
        .clone();
    let mut annots = annotations(&doc, page)?.clone();
    let mut entries = Dictionary::new();
    // Movement/color changes must retain the original string object (or its absence).
    if request.text != note.text {
        let encoded: Vec<u8> = [0xfe, 0xff]
            .into_iter()
            .chain(request.text.encode_utf16().flat_map(u16::to_be_bytes))
            .collect();
        entries.set(
            "Contents",
            Object::String(encoded, StringFormat::Hexadecimal),
        );
    }
    if let Some(position) = &request.position {
        let a = geometry.pdf_to_displayed(note.bounds.x, note.bounds.y);
        let b = geometry.pdf_to_displayed(
            note.bounds.x + note.bounds.width,
            note.bounds.y + note.bounds.height,
        );
        let (width, height) = ((a.x - b.x).abs(), (a.y - b.y).abs());
        let size = geometry.displayed_size();
        if !position.x.is_finite()
            || !position.y.is_finite()
            || position.x < 0.0
            || position.y < 0.0
            || position.x + width > size.width + 0.01
            || position.y + height > size.height + 0.01
        {
            return Err(bad("comment position must fit inside the page"));
        }
        let a = geometry.displayed_to_pdf(Point {
            x: position.x,
            y: position.y,
        });
        let b = geometry.displayed_to_pdf(Point {
            x: position.x + width,
            y: position.y + height,
        });
        entries.set(
            "Rect",
            vec![
                Object::Real(a.0.value.min(b.0.value)),
                Object::Real(a.1.value.min(b.1.value)),
                Object::Real(a.0.value.max(b.0.value)),
                Object::Real(a.1.value.max(b.1.value)),
            ],
        );
    }
    if let Some(color) = &request.color {
        if !note.can_change_color {
            return Err(bad("This comment uses a custom appearance. Its color cannot be changed without replacing the icon."));
        }
        let rgb = parse_color(color)?;
        entries.set(
            "C",
            vec![
                Object::Real(rgb.red() as f32 / 255.0),
                Object::Real(rgb.green() as f32 / 255.0),
                Object::Real(rgb.blue() as f32 / 255.0),
            ],
        );
    }
    set_entries(&mut doc, &mut annots[request.annotation_index], entries)?;
    // Only direct annotations need their array container updated. For indirect
    // annotations, the existing reference and all shared placements stay intact.
    if !matches!(annots[request.annotation_index], Object::Reference(_)) {
        if let Some(id) = terminal_reference(&doc, &annots_value)? {
            doc.objects.insert(id, Object::Array(annots));
        } else {
            doc.get_dictionary_mut(page)
                .map_err(bad)?
                .set("Annots", annots);
        }
    }
    let output = crate::pdf_output::save(&mut doc)?;
    let verified = read(&load(&output)?, excluded)?;
    if !verified.iter().any(|n| {
        n.page_index == request.page_index
            && n.annotation_index == request.annotation_index
            && n.text == request.text
            && request
                .color
                .as_ref()
                .is_none_or(|c| n.color.as_deref() == Some(c.to_ascii_lowercase().as_str()))
            && request.position.as_ref().is_none_or(|position| {
                let a = geometry.pdf_to_displayed(n.bounds.x, n.bounds.y);
                let b = geometry
                    .pdf_to_displayed(n.bounds.x + n.bounds.width, n.bounds.y + n.bounds.height);
                (a.x.min(b.x) - position.x).abs() < 0.01 && (a.y.min(b.y) - position.y).abs() < 0.01
            })
    }) {
        return Err(bad("serialized comment differs from the requested text"));
    }
    Ok(output)
}

struct DeletedComment {
    bytes: Vec<u8>,
    removed: Vec<(usize, usize)>,
}
fn delete(
    bytes: &[u8],
    excluded: &HashMap<usize, HashSet<usize>>,
    request: &DeleteNativeCommentRequest,
) -> EngineResult<DeletedComment> {
    let mut doc = load(bytes)?;
    let notes = read(&doc, excluded)?;
    let note = notes
        .iter()
        .find(|n| {
            n.page_index == request.page_index && n.annotation_index == request.annotation_index
        })
        .ok_or_else(|| bad("comment is no longer available"))?;
    if let Some(reason) = &note.delete_reason {
        return Err(bad(reason));
    }
    if note.text != request.expected_text {
        return Err(bad("comment changed; reopen it before deleting"));
    }
    crate::acroforms::filling_document(bytes)?;
    let pages: Vec<_> = doc.get_pages().values().copied().collect();
    let page = pages[request.page_index];
    let target = annotations(&doc, page)?[request.annotation_index].clone();
    let target_id = terminal_reference(&doc, &target)?;
    let data = resolve(&doc, &target)?.as_dict().map_err(bad)?;
    let mut popup_ids = HashSet::new();
    if let Ok(popup) = data.get(b"Popup") {
        if !matches!(resolve(&doc, popup)?, Object::Null) {
            let id = terminal_reference(&doc, popup)?
                .ok_or_else(|| bad("direct popup relationships cannot be deleted safely"))?;
            if resolve(&doc, popup)?
                .as_dict()
                .map_err(bad)?
                .get(b"Subtype")
                .and_then(Object::as_name)
                .map_err(bad)?
                != b"Popup"
            {
                return Err(bad("invalid linked popup"));
            }
            popup_ids.insert(id);
        }
    }
    // Discover reverse-only popup links, and refuse a popup owned by another note.
    for page in &pages {
        if !doc.get_dictionary(*page).map_err(bad)?.has(b"Annots") {
            continue;
        }
        for value in annotations(&doc, *page)? {
            let data = resolve(&doc, value)?.as_dict().map_err(bad)?;
            if target_id.is_some()
                && data
                    .get(b"Subtype")
                    .and_then(Object::as_name)
                    .is_ok_and(|s| s == b"Popup")
            {
                if let Ok(parent) = data.get(b"Parent") {
                    if terminal_reference(&doc, parent)? == target_id {
                        let id = terminal_reference(&doc, value)?.ok_or_else(|| {
                            bad("direct popup relationships cannot be deleted safely")
                        })?;
                        popup_ids.insert(id);
                    }
                }
            }
        }
    }
    let mut updates = Vec::new();
    let mut removed = Vec::new();
    for (page_index, page) in pages.iter().enumerate() {
        if !doc.get_dictionary(*page).map_err(bad)?.has(b"Annots") {
            continue;
        }
        let old = annotations(&doc, *page)?;
        let mut kept = Vec::with_capacity(old.len());
        for (index, value) in old.iter().enumerate() {
            let id = terminal_reference(&doc, value)?;
            let is_target = target_id.is_some() && id == target_id
                || page_index == request.page_index && index == request.annotation_index;
            let data = resolve(&doc, value)?.as_dict().map_err(bad)?;
            if !is_target {
                if let Ok(popup) = data.get(b"Popup") {
                    if terminal_reference(&doc, popup)?.is_some_and(|id| popup_ids.contains(&id)) {
                        return Err(bad(
                            "This comment shares its popup with another annotation.",
                        ));
                    }
                }
            }
            if !is_target && !id.is_some_and(|id| popup_ids.contains(&id)) {
                kept.push(value.clone());
            } else {
                if removed.len() >= 1_000_000
                    || removed.len().checked_add(1).and_then(|n| n.checked_mul(std::mem::size_of::<(usize, usize)>()))
                        .is_none_or(|n| n > 16 * 1024 * 1024) {
                    return Err(bad("deleted comment ownership exceeds its transfer budget"));
                }
                removed.push((page_index, index));
            }
        }
        if kept.len() != old.len() {
            updates.push((*page, kept));
        }
    }
    // Assign page-local arrays, so shared indirect arrays cannot cause index drift.
    for (page, kept) in updates {
        doc.get_dictionary_mut(page)
            .map_err(bad)?
            .set("Annots", kept);
    }
    let output = crate::pdf_output::save(&mut doc)?;
    let verified = load(&output)?;
    for page in verified.get_pages().values() {
        if !verified.get_dictionary(*page).map_err(bad)?.has(b"Annots") {
            continue;
        }
        for value in annotations(&verified, *page)? {
            let id = terminal_reference(&verified, value)?;
            if id.is_some() && (id == target_id || id.is_some_and(|id| popup_ids.contains(&id))) {
                return Err(bad("deleted comment remains on a page"));
            }
        }
    }
    Ok(DeletedComment { bytes: output, removed })
}

impl WorkerRuntime {
    pub(super) fn list_native_comments(&self, source_id: &str) -> EngineResult<Vec<NativeComment>> {
        let source = self.document(source_id)?;
        if source.for_printing
            || source.policy.as_ref().is_some_and(|p| {
                let r = p.protection.permissions();
                !r.annotate && !r.copy_content
            })
        {
            return Err(EngineError::PermissionDenied("reading comments"));
        }
        let mut notes = read(&load(source.bytes()?.as_ref())?, &source.imported_annotations)?;
        for note in &mut notes {
            let page = source
                .pdf()?
                .pages()
                .get(i32::try_from(note.page_index).map_err(bad)?)?;
            let geometry = page_geometry(&page)?;
            // PDFium's pinned Text appearance generator uses a 20pt square anchored
            // at Rect's lower-left. NoRotate keeps that appearance upright about
            // its transformed upper-left anchor, rather than rotating its bounds.
            let width = if note.default_appearance {
                20.0
            } else {
                note.bounds.width
            };
            let height = if note.default_appearance {
                20.0
            } else {
                note.bounds.height
            };
            let top_left = geometry.pdf_to_displayed(note.bounds.x, note.bounds.y + height);
            let bottom_right = geometry.pdf_to_displayed(note.bounds.x + width, note.bounds.y);
            note.hit_bounds = if note.no_rotate {
                AnnotationRect {
                    x: top_left.x,
                    y: top_left.y,
                    width,
                    height,
                }
            } else {
                AnnotationRect {
                    x: top_left.x.min(bottom_right.x),
                    y: top_left.y.min(bottom_right.y),
                    width: (bottom_right.x - top_left.x).abs(),
                    height: (bottom_right.y - top_left.y).abs(),
                }
            };
            let a = geometry.pdf_to_displayed(note.bounds.x, note.bounds.y);
            let b = geometry.pdf_to_displayed(
                note.bounds.x + note.bounds.width,
                note.bounds.y + note.bounds.height,
            );
            note.bounds = AnnotationRect {
                x: a.x.min(b.x),
                y: a.y.min(b.y),
                width: (b.x - a.x).abs(),
                height: (b.y - a.y).abs(),
            };
            if source
                .policy
                .as_ref()
                .is_some_and(|p| !p.protection.permissions().annotate)
            {
                note.reason = Some("This PDF does not permit changing annotations.".into());
                note.delete_reason = note.reason.clone();
            }
        }
        Ok(notes)
    }
    pub(super) fn edit_native_comment(
        &mut self,
        source_id: &str,
        request: &EditNativeCommentRequest,
    ) -> EngineResult<DocumentInfo> {
        let source = self.document(source_id)?;
        if source.for_printing
            || source
                .policy
                .as_ref()
                .is_some_and(|p| !p.protection.permissions().annotate)
        {
            return Err(EngineError::PermissionDenied("changing annotations"));
        }
        let page = source
            .pdf()?
            .pages()
            .get(i32::try_from(request.page_index).map_err(bad)?)?;
        let geometry = page_geometry(&page)?;
        drop(page);
        let bytes = edit(
            source.bytes()?.as_ref(),
            &source.imported_annotations,
            request,
            geometry,
        )?;
        self.register_comment_replacement(source_id, bytes, None)
    }
    pub(super) fn delete_native_comment(
        &mut self,
        source_id: &str,
        request: &DeleteNativeCommentRequest,
    ) -> EngineResult<DocumentInfo> {
        let source = self.document(source_id)?;
        if source.for_printing
            || source
                .policy
                .as_ref()
                .is_some_and(|p| !p.protection.permissions().annotate)
        {
            return Err(EngineError::PermissionDenied("deleting annotations"));
        }
        let deleted = delete(source.bytes()?.as_ref(), &source.imported_annotations, request)?;
        self.register_comment_replacement(source_id, deleted.bytes, Some(&deleted.removed))
    }
    fn register_comment_replacement(
        &mut self,
        source_id: &str,
        bytes: Vec<u8>,
        removed: Option<&[(usize, usize)]>,
    ) -> EngineResult<DocumentInfo> {
        let source = self.document(source_id)?;
        let policy = source.policy.clone();
        let original_path = source.original_path.clone();
        let legacy_comment_import = source.legacy_comment_import;
        let mut info = self.register_generated_document(bytes, "Edited comment.pdf")?;
        info.protection = policy.as_ref().map(SourcePolicy::info);
        let result = (|| {
            let candidate = self.documents.get_mut(&info.id).unwrap();
            candidate.policy = policy;
            candidate.original_path = original_path;
            candidate.legacy_comment_import = legacy_comment_import;
            candidate.imported_overlay_sources = imported_overlay_sources::Index::capturing(candidate.source_bytes.identity().clone());
            let mut metadata = None;
            let mut fonts = vec![];
            let mut images = vec![];
            for index in 0..candidate.document.page_count() {
                let mut page = candidate.document.get(&candidate.imported_annotations)?.pages().get(page_index_i32(index)?)?;
                let geometry = page_geometry(&page)?;
                info.pages[index as usize].overlays = import_annotations(
                    self.pdfium,
                    &mut page,
                    geometry,
                    index as usize,
                    candidate.bytes()?.as_ref(),
                    &mut metadata,
                    &mut fonts,
                    &mut images,
                    &mut candidate.imported_annotations,
                    &mut candidate.imported_overlay_sources,
                    candidate.legacy_comment_import,
                )?;
            }
            candidate.image_leases = Some(ImageSourceLeases {
                registry: self.images.clone(),
                registrations: self.images.register_source_assets(
                    &images,
                    candidate
                        .policy
                        .as_ref()
                        .and_then(SourcePolicy::image_restriction),
                )?,
            });
            Ok(())
        })();
        if let Err(error) = result {
            self.discard_unpublished_source(&info.id);
            return Err(error);
        }
        self.retain_session_annotation_sources(source_id, info, removed)
    }
}
