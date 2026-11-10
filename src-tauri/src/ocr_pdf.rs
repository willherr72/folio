//! Searchable layers are private Form XObjects, adopted only after full regeneration proof.
use crate::{ocr_types::invalid, EngineResult, OcrPage};
use lopdf::{
    content::Content, dictionary, Dictionary, Document, Object, ObjectId, Stream, StringFormat,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OcrMapping {
    pub origin: [f32; 2],
    pub right: [f32; 2],
    pub down: [f32; 2],
}
impl OcrMapping {
    pub(crate) fn point(&self, x: f32, y: f32) -> [f32; 2] {
        [
            self.origin[0] + self.right[0] * x + self.down[0] * y,
            self.origin[1] + self.right[1] * x + self.down[1] * y,
        ]
    }
    fn bbox(&self, page: &OcrPage) -> EngineResult<[f32; 4]> {
        if !self
            .origin
            .iter()
            .chain(self.right.iter())
            .chain(self.down.iter())
            .all(|n| n.is_finite() && n.abs() <= 1_000_000.)
            || (self.right[0] == 0.) == (self.right[1] == 0.)
            || (self.down[0] == 0.) == (self.down[1] == 0.)
            || self.right[0] * self.down[0] + self.right[1] * self.down[1] != 0.
            || self.right[0] * self.down[1] - self.right[1] * self.down[0] >= 0.
        {
            return Err(invalid("invalid raster-to-PDF mapping"));
        }
        let points = [
            self.origin,
            self.point(page.width as f32, 0.),
            self.point(0., page.height as f32),
            self.point(page.width as f32, page.height as f32),
        ];
        if points
            .iter()
            .flatten()
            .any(|n| !n.is_finite() || n.abs() > 1_000_000.)
        {
            return Err(invalid("mapped page exceeds PDF coordinate bounds"));
        }
        let bbox = [
            points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min),
            points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min),
            points
                .iter()
                .map(|p| p[0])
                .fold(f32::NEG_INFINITY, f32::max),
            points
                .iter()
                .map(|p| p[1])
                .fold(f32::NEG_INFINITY, f32::max),
        ];
        if bbox[2] <= bbox[0] || bbox[3] <= bbox[1] {
            return Err(invalid("mapped page has no area"));
        }
        Ok(bbox)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OcrRecord {
    pub version: u8,
    pub mapping: OcrMapping,
    pub page: OcrPage,
}
pub(crate) struct OwnedOcr {
    pub form_id: ObjectId,
    pub record: OcrRecord,
}
struct Slot {
    name: Vec<u8>,
    form_id: ObjectId,
    record: Option<OcrRecord>,
}
fn numbers(values: &[f32]) -> Object {
    Object::Array(values.iter().copied().map(Object::Real).collect())
}
fn error(e: impl std::fmt::Display) -> crate::EngineError {
    invalid(e)
}

fn resources(document: &Document, page: ObjectId) -> EngineResult<Dictionary> {
    let mut current = page;
    let mut seen = HashSet::new();
    for _ in 0..64 {
        if !seen.insert(current) {
            return Err(invalid("cyclic page resources"));
        }
        let dictionary = document.get_dictionary(current).map_err(error)?;
        if let Ok(value) = dictionary.get(b"Resources") {
            let found = document
                .dereference(value)
                .map_err(error)?
                .1
                .as_dict()
                .map_err(error)?;
            crate::form_graph::CopyBudget::default().dictionary(found)?;
            return Ok(found.clone());
        }
        match dictionary.get(b"Parent") {
            Ok(value) => current = value.as_reference().map_err(error)?,
            Err(_) => return Ok(Dictionary::new()),
        }
    }
    Err(invalid("page resource inheritance exceeds 64 levels"))
}
fn xobjects(document: &Document, resources: &Dictionary) -> EngineResult<Dictionary> {
    match resources.get(b"XObject") {
        Ok(value) => {
            let result = document
                .dereference(value)
                .map_err(error)?
                .1
                .as_dict()
                .map_err(error)?;
            crate::form_graph::CopyBudget::default().dictionary(result)?;
            Ok(result.clone())
        }
        Err(_) => Ok(Dictionary::new()),
    }
}
fn form(document: &mut Document, record: &OcrRecord) -> EngineResult<ObjectId> {
    record.page.validate()?;
    let bbox = record.mapping.bbox(&record.page)?;
    let (font, content) = crate::ocr_font::build(document, &record.page, &record.mapping)?;
    let metadata = serde_json::to_vec(record).map_err(error)?;
    if metadata.len() > 8 * 1024 * 1024 {
        return Err(invalid("OCR record exceeds 8 MiB"));
    }
    Ok(document.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>numbers(&bbox),
        "Resources"=>dictionary!{"Font"=>dictionary!{"F0"=>font}},"FolioOCR"=>Object::String(metadata,StringFormat::Hexadecimal)},content)))
}
fn empty_form(bbox: Object) -> Object {
    Object::Stream(Stream::new(
        dictionary! {"Type"=>"XObject","Subtype"=>"Form","BBox"=>bbox,
        "Resources"=>dictionary!{},"FolioOCREmpty"=>true},
        b"q\nQ\n".to_vec(),
    ))
}

#[derive(Clone, Copy)]
struct Graphics {
    matrix: [f32; 6],
    clipped: bool,
}
const IDENTITY: [f32; 6] = [1., 0., 0., 1., 0., 0.];
fn multiply(a: [f32; 6], b: [f32; 6]) -> [f32; 6] {
    [
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ]
}
/// Unlike lopdf's display-oriented helpers, admission must never omit a bad entry
/// or treat undecodable compressed bytes as an empty original content stream.
fn page_streams(document: &Document, page: ObjectId) -> EngineResult<Vec<ObjectId>> {
    let dictionary = document.get_dictionary(page).map_err(error)?;
    let Ok(contents) = dictionary.get(b"Contents") else {
        return Ok(vec![]);
    };
    let (id, value) = document.dereference(contents).map_err(error)?;
    match value {
        Object::Null => Ok(vec![]),
        Object::Stream(_) => {
            Ok(vec![id.ok_or_else(|| {
                invalid("page content stream must be indirect")
            })?])
        }
        Object::Array(values) => {
            if values.len() > 100_000 {
                return Err(invalid("too many page content streams"));
            }
            values
                .iter()
                .map(|value| {
                    if !matches!(value, Object::Reference(_)) {
                        return Err(invalid("page content array must contain stream references"));
                    }
                    let (id, value) = document.dereference(value).map_err(error)?;
                    value.as_stream().map_err(error)?;
                    id.ok_or_else(|| invalid("page content stream must be indirect"))
                })
                .collect()
        }
        _ => Err(invalid("invalid page content object")),
    }
}
fn page_content(document: &Document, page: ObjectId) -> EngineResult<Vec<u8>> {
    const LIMIT: usize = 16 * 1024 * 1024;
    let mut bytes = Vec::new();
    for id in page_streams(document, page)? {
        let stream = document
            .get_object(id)
            .map_err(error)?
            .as_stream()
            .map_err(error)?;
        let remaining = LIMIT
            .checked_sub(bytes.len() + 1)
            .ok_or_else(|| invalid("page content exceeds 16 MiB"))?;
        if stream.dict.has(b"Filter") {
            bytes.extend(
                stream
                    .decompressed_content_with_limit(remaining)
                    .map_err(error)?,
            );
        } else {
            if stream.content.len() > remaining {
                return Err(invalid("page content exceeds 16 MiB"));
            }
            bytes.extend_from_slice(&stream.content);
        }
        bytes.push(b'\n');
    }
    Ok(bytes)
}
/// Reading all source operations also proves that wrapping restores the initial graphics state.
fn invocations(
    document: &Document,
    page: ObjectId,
) -> EngineResult<BTreeMap<Vec<u8>, (usize, bool)>> {
    let bytes = page_content(document, page)?;
    let content = Content::decode(&bytes).map_err(error)?;
    if content.operations.len() > 1_000_000 {
        return Err(invalid("page content exceeds one million operations"));
    }
    let mut state = Graphics {
        matrix: IDENTITY,
        clipped: false,
    };
    let mut stack = vec![];
    let mut text = false;
    let mut found = BTreeMap::new();
    for operation in content.operations {
        match operation.operator.as_str() {
            "q" => {
                if stack.len() >= 256 {
                    return Err(invalid("graphics state depth exceeds 256"));
                }
                stack.push(state);
            }
            "Q" => {
                state = stack
                    .pop()
                    .ok_or_else(|| invalid("unbalanced graphics state"))?
            }
            "BT" => {
                if text {
                    return Err(invalid("nested text objects"));
                }
                text = true;
            }
            "ET" => {
                if !text {
                    return Err(invalid("unbalanced text object"));
                }
                text = false;
            }
            "W" | "W*" => state.clipped = true,
            "cm" => {
                if operation.operands.len() != 6 {
                    return Err(invalid("invalid graphics transform"));
                }
                let mut matrix = [0.; 6];
                for (out, value) in matrix.iter_mut().zip(&operation.operands) {
                    *out = value.as_float().map_err(error)?;
                }
                state.matrix = multiply(state.matrix, matrix);
                if state
                    .matrix
                    .iter()
                    .any(|n| !n.is_finite() || n.abs() > 1_000_000.)
                {
                    return Err(invalid("graphics transform exceeds bounds"));
                }
            }
            "Do" => {
                if operation.operands.len() != 1 {
                    return Err(invalid("invalid Form invocation"));
                }
                let name = operation.operands[0].as_name().map_err(error)?.to_vec();
                let untransformed = !text
                    && !state.clipped
                    && state
                        .matrix
                        .iter()
                        .zip(IDENTITY)
                        .all(|(a, b)| (*a - b).abs() < 0.00001);
                let entry = found.entry(name).or_insert((0usize, true));
                entry.0 += 1;
                entry.1 &= untransformed;
            }
            _ => {}
        }
    }
    if !stack.is_empty() || text {
        return Err(invalid("unbalanced original page content"));
    }
    Ok(found)
}

fn slot(document: &mut Document, page: ObjectId) -> EngineResult<Option<Slot>> {
    let names = xobjects(document, &resources(document, page)?)?;
    let invocations = invocations(document, page)?;
    let mut found = None;
    let mut candidates = 0;
    for (name, value) in names.iter() {
        let (Some(form_id), Object::Stream(stream)) = document.dereference(value).map_err(error)?
        else {
            continue;
        };
        let marker = stream.dict.get(b"FolioOCR").ok();
        let empty = stream
            .dict
            .get(b"FolioOCREmpty")
            .ok()
            .and_then(|v| v.as_bool().ok())
            == Some(true);
        if marker.is_none() && !empty {
            continue;
        }
        candidates += 1;
        if candidates > 8 {
            return Err(invalid("too many OCR ownership candidates on one page"));
        }
        let record = if let Some(marker) = marker {
            let Ok(bytes) = marker.as_str() else { continue };
            if bytes.len() > 8 * 1024 * 1024 {
                continue;
            }
            let Ok(record) = serde_json::from_slice::<OcrRecord>(bytes) else {
                continue;
            };
            if record.version != 1
                || record.page.validate().is_err()
                || record.mapping.bbox(&record.page).is_err()
            {
                continue;
            }
            Some(record)
        } else {
            None
        };
        let actual = Object::Reference(form_id);
        let expected_empty = if record.is_none() {
            let Ok(bbox) = stream.dict.get(b"BBox") else {
                continue;
            };
            let Ok(values) = bbox.as_array() else {
                continue;
            };
            if values.len() != 4
                || values.iter().any(|v| {
                    v.as_float()
                        .ok()
                        .is_none_or(|n| !n.is_finite() || n.abs() > 1_000_000.)
                })
            {
                continue;
            }
            Some(empty_form(bbox.clone()))
        } else {
            None
        };
        let previous_max = document.max_id;
        let expected = match &record {
            Some(record) => form(document, record).map(Object::Reference),
            None => Ok(expected_empty.unwrap()),
        };
        let matches =
            expected.is_ok_and(|expected| crate::engine::same_object(document, &actual, &expected));
        document.objects.retain(|id, _| id.0 <= previous_max);
        document.max_id = previous_max;
        if !matches {
            continue;
        }
        match invocations.get(name) {
            Some((1, true)) => {}
            Some(_) => {
                return Err(invalid(
                    "owned OCR has repeated, clipped or transformed invocations",
                ))
            }
            None => continue,
        }
        if found.is_some() {
            return Err(invalid("multiple owned OCR layers on one page"));
        }
        found = Some(Slot {
            name: name.clone(),
            form_id,
            record,
        });
    }
    Ok(found)
}
pub(crate) fn read_owned(
    document: &mut Document,
    page: ObjectId,
) -> EngineResult<Option<OwnedOcr>> {
    Ok(slot(document, page)?.and_then(|slot| {
        slot.record.map(|record| OwnedOcr {
            form_id: slot.form_id,
            record,
        })
    }))
}
pub(crate) fn replace(
    document: &mut Document,
    page: ObjectId,
    mapping: OcrMapping,
    result: &OcrPage,
) -> EngineResult<()> {
    let (words, _) = result.validate()?;
    let bbox = mapping.bbox(result)?;
    let previous = slot(document, page)?;
    if previous.is_none() && words == 0 {
        return Ok(());
    }
    let mut resources = resources(document, page)?;
    let mut xobjects = xobjects(document, &resources)?;
    let new_form = if words == 0 {
        document.add_object(empty_form(numbers(&bbox)))
    } else {
        form(
            document,
            &OcrRecord {
                version: 1,
                mapping,
                page: result.clone(),
            },
        )?
    };
    let name = if let Some(slot) = previous {
        slot.name
    } else {
        let mut name = b"FolioOCR".to_vec();
        while xobjects.has(&name) {
            name.push(b'_');
        }
        // Existing resources/streams remain intact. Restore initial clip/CTM before Do.
        let prefix = document.add_object(Stream::new(dictionary! {}, b"q\n".to_vec()));
        let suffix = document.add_object(Stream::new(
            dictionary! {},
            format!("\nQ\nq /{} Do Q\n", String::from_utf8_lossy(&name)).into_bytes(),
        ));
        let mut contents = vec![Object::Reference(prefix)];
        contents.extend(
            page_streams(document, page)?
                .into_iter()
                .map(Object::Reference),
        );
        contents.push(Object::Reference(suffix));
        document
            .get_dictionary_mut(page)
            .map_err(error)?
            .set("Contents", contents);
        name
    };
    xobjects.set(name, Object::Reference(new_form));
    resources.set("XObject", xobjects);
    document
        .get_dictionary_mut(page)
        .map_err(error)?
        .set("Resources", resources);
    document.prune_objects();
    Ok(())
}

#[cfg(test)]
#[path = "ocr_pdf_tests.rs"]
mod tests;
