//! Portable editable text/ink annotations. Their PDF appearances are independent
//! of Folio, while bounded, versioned metadata restores exact editor geometry.
use super::{EngineError, EngineResult, PageGeometry, MAX_SOURCE_BYTES};
use crate::types::*;
use lopdf::{dictionary, Dictionary, Document, Object, Stream, StringFormat};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};

const MAX_METADATA_BYTES: usize = 32 * 1024 * 1024;
const MAX_AP_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoordinateFrame {
    left: f32,
    bottom: f32,
    right: f32,
    top: f32,
    rotation: u16,
}

impl From<PageGeometry> for CoordinateFrame {
    fn from(g: PageGeometry) -> Self {
        Self {
            left: g.left,
            bottom: g.bottom,
            right: g.right,
            top: g.top,
            rotation: g.rotation,
        }
    }
}
impl CoordinateFrame {
    fn geometry(&self) -> Option<PageGeometry> {
        if !matches!(self.rotation, 0 | 90 | 180 | 270)
            || [self.left, self.bottom, self.right, self.top]
                .iter()
                .any(|n| !n.is_finite() || n.abs() > 10_000_000.0)
            || self.right <= self.left
            || self.top <= self.bottom
        {
            return None;
        }
        Some(PageGeometry {
            left: self.left,
            bottom: self.bottom,
            right: self.right,
            top: self.top,
            rotation: self.rotation,
        })
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct PortableRecord {
    version: u32,
    frame: CoordinateFrame,
    overlay: Overlay,
    appearance_hash: String,
}

impl PortableRecord {
    pub(super) fn displayed(&self, current: PageGeometry) -> Option<Overlay> {
        let old = self.frame.geometry()?;
        let convert = |point: Point| {
            let (x, y) = old.displayed_to_pdf(point);
            current.pdf_to_displayed(x.value, y.value)
        };
        let mut overlay = self.overlay.clone();
        match &mut overlay {
            Overlay::Text(text) => {
                let point = convert(Point {
                    x: text.x,
                    y: text.y,
                });
                text.x = point.x;
                text.y = point.y;
                text.rotation = (text.rotation + current.rotation + 360 - old.rotation) % 360;
                super::validate_text(
                    text,
                    current.displayed_size().width,
                    current.displayed_size().height,
                )
                .ok()?;
            }
            Overlay::Ink(ink) => {
                for path in &mut ink.paths {
                    for point in path {
                        *point = convert(*point);
                    }
                }
                super::validate_ink(
                    ink,
                    current.displayed_size().width,
                    current.displayed_size().height,
                )
                .ok()?;
            }
            _ => return None,
        }
        Some(overlay)
    }
}

struct Appearance {
    content: Vec<u8>,
    bounds: [f32; 4],
    resources: Dictionary,
    fields: Dictionary,
}

fn font() -> Dictionary {
    dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica","Encoding"=>"WinAnsiEncoding"}
}
fn rgb(color: &str) -> EngineResult<[f32; 3]> {
    let c = super::parse_color(color)?;
    Ok([
        c.red() as f32 / 255.0,
        c.green() as f32 / 255.0,
        c.blue() as f32 / 255.0,
    ])
}
fn numbers(values: &[f32]) -> Object {
    Object::Array(values.iter().map(|n| Object::Real(*n)).collect())
}
fn pdf_string(value: &str) -> Object {
    let mut bytes = vec![0xfe, 0xff];
    for value in value.encode_utf16() {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    Object::String(bytes, StringFormat::Hexadecimal)
}
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join("")
}

pub(super) fn rotated_point(point: Point, rotation: u16) -> Point {
    match rotation {
        0 => point,
        90 => Point {
            x: -point.y,
            y: point.x,
        },
        180 => Point {
            x: -point.x,
            y: -point.y,
        },
        270 => Point {
            x: point.y,
            y: -point.x,
        },
        _ => point,
    }
}

fn win_ansi(value: &str) -> EngineResult<Vec<u8>> {
    let mut bytes = Vec::new();
    for character in value.chars() {
        let byte = match character {
            '\r' => continue,
            '\u{20ac}' => 0x80,
            '\u{201a}' => 0x82,
            '\u{0192}' => 0x83,
            '\u{201e}' => 0x84,
            '\u{2026}' => 0x85,
            '\u{2020}' => 0x86,
            '\u{2021}' => 0x87,
            '\u{02c6}' => 0x88,
            '\u{2030}' => 0x89,
            '\u{0160}' => 0x8a,
            '\u{2039}' => 0x8b,
            '\u{0152}' => 0x8c,
            '\u{017d}' => 0x8e,
            '\u{2018}' => 0x91,
            '\u{2019}' => 0x92,
            '\u{201c}' => 0x93,
            '\u{201d}' => 0x94,
            '\u{2022}' => 0x95,
            '\u{2013}' => 0x96,
            '\u{2014}' => 0x97,
            '\u{02dc}' => 0x98,
            '\u{2122}' => 0x99,
            '\u{0161}' => 0x9a,
            '\u{203a}' => 0x9b,
            '\u{0153}' => 0x9c,
            '\u{017e}' => 0x9e,
            '\u{0178}' => 0x9f,
            character if super::helvetica_supports(character) && character as u32 <= 255 => {
                character as u8
            }
            _ => {
                return Err(EngineError::InvalidRequest(
                    "unsupported text character in portable appearance".into(),
                ))
            }
        };
        bytes.push(byte);
    }
    Ok(bytes)
}

fn appearance(overlay: &Overlay, geometry: PageGeometry) -> EngineResult<Appearance> {
    use lopdf::content::{Content, Operation};
    let mut operations = vec![Operation::new("q", vec![])];
    let mut bounds = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    let mut include = |x: f32, y: f32| {
        bounds[0] = bounds[0].min(x);
        bounds[1] = bounds[1].min(y);
        bounds[2] = bounds[2].max(x);
        bounds[3] = bounds[3].max(y);
    };
    let mut resources = Dictionary::new();
    let mut fields = Dictionary::new();
    match overlay {
        Overlay::Text(text) => {
            super::validate_id("text id", &text.id)?;
            super::validate_text(
                text,
                geometry.displayed_size().width,
                geometry.displayed_size().height,
            )?;
            let color = rgb(&text.color)?;
            resources.set("Font", dictionary! {"FolioFont"=>font()});
            fields.set("Subtype", "FreeText");
            fields.set("Contents", pdf_string(&text.text));
            fields.set(
                "DA",
                Object::string_literal(format!(
                    "/Helv {} Tf {} {} {} rg",
                    text.font_size, color[0], color[1], color[2]
                )),
            );
            fields.set("C", numbers(&color));
            fields.set("Q", 0);
            let angle = (geometry.rotation + 360 - text.rotation) % 360;
            let (a, b, c, d) = match angle {
                0 => (1., 0., 0., 1.),
                90 => (0., 1., -1., 0.),
                180 => (-1., 0., 0., -1.),
                _ => (0., -1., 1., 0.),
            };
            let lines: Vec<_> = text.text.lines().collect();
            for (index, line) in lines.iter().enumerate() {
                let baseline = rotated_point(
                    Point {
                        x: 0.0,
                        y: text.font_size + index as f32 * text.font_size * 1.2,
                    },
                    text.rotation,
                );
                let (x, y) = geometry.displayed_to_pdf(Point {
                    x: text.x + baseline.x,
                    y: text.y + baseline.y,
                });
                operations.extend([
                    Operation::new("BT", vec![]),
                    Operation::new(
                        "Tf",
                        vec![
                            Object::Name(b"FolioFont".to_vec()),
                            Object::Real(text.font_size),
                        ],
                    ),
                    Operation::new("rg", color.iter().map(|n| Object::Real(*n)).collect()),
                    Operation::new(
                        "Tm",
                        [a, b, c, d, x.value, y.value]
                            .into_iter()
                            .map(Object::Real)
                            .collect(),
                    ),
                    Operation::new(
                        "Tj",
                        vec![Object::String(win_ansi(line)?, StringFormat::Literal)],
                    ),
                    Operation::new("ET", vec![]),
                ]);
            }
            // Conservative metric bounds prevent clipping descenders, accents and
            // side bearings. Text placement itself uses exact font-size baselines.
            let width = lines
                .iter()
                .map(|line| line.chars().count())
                .max()
                .unwrap_or(0) as f32
                * text.font_size
                * 1.2;
            let height = lines.len().max(1) as f32 * text.font_size * 1.2;
            for (x, y) in [
                (-text.font_size, -text.font_size),
                (width + text.font_size, -text.font_size),
                (-text.font_size, height + text.font_size),
                (width + text.font_size, height + text.font_size),
            ] {
                let p = rotated_point(Point { x, y }, text.rotation);
                let (x, y) = geometry.displayed_to_pdf(Point {
                    x: text.x + p.x,
                    y: text.y + p.y,
                });
                include(x.value, y.value);
            }
        }
        Overlay::Ink(ink) => {
            super::validate_id("ink id", &ink.id)?;
            super::validate_ink(
                ink,
                geometry.displayed_size().width,
                geometry.displayed_size().height,
            )?;
            let color = rgb(&ink.color)?;
            fields.set("Subtype", "Ink");
            fields.set("C", numbers(&color));
            fields.set(
                "BS",
                dictionary! {"W"=>Object::Real(ink.stroke_width),"S"=>"S"},
            );
            fields.set("Border", numbers(&[0.0, 0.0, ink.stroke_width]));
            operations.extend([
                Operation::new("RG", color.iter().map(|n| Object::Real(*n)).collect()),
                Operation::new("w", vec![Object::Real(ink.stroke_width)]),
                Operation::new("J", vec![1.into()]),
                Operation::new("j", vec![1.into()]),
            ]);
            let mut paths = Vec::new();
            for path in &ink.paths {
                let mut points = Vec::new();
                for (index, p) in path.iter().enumerate() {
                    let (x, y) = geometry.displayed_to_pdf(*p);
                    include(x.value, y.value);
                    points.extend([Object::Real(x.value), Object::Real(y.value)]);
                    operations.push(Operation::new(
                        if index == 0 { "m" } else { "l" },
                        vec![Object::Real(x.value), Object::Real(y.value)],
                    ));
                }
                operations.push(Operation::new("S", vec![]));
                paths.push(Object::Array(points));
            }
            fields.set("InkList", paths);
            let pad = ink.stroke_width / 2.0 + 1.0;
            bounds[0] -= pad;
            bounds[1] -= pad;
            bounds[2] += pad;
            bounds[3] += pad;
        }
        _ => {
            return Err(EngineError::InvalidRequest(
                "portable metadata requires text or ink".into(),
            ))
        }
    }
    if bounds.iter().any(|n| !n.is_finite()) || bounds[0] >= bounds[2] || bounds[1] >= bounds[3] {
        return Err(EngineError::InvalidRequest(
            "invalid portable appearance bounds".into(),
        ));
    }
    operations.push(Operation::new("Q", vec![]));
    let content = Content { operations }.encode().map_err(pdf_error)?;
    if content.len() > MAX_AP_BYTES {
        return Err(EngineError::InvalidRequest(
            "portable appearance exceeds 64 MiB".into(),
        ));
    }
    Ok(Appearance {
        content,
        bounds,
        resources,
        fields,
    })
}

pub(super) fn finalize(
    path: &Path,
    request: &ExportRequest,
    geometries: &[PageGeometry],
) -> EngineResult<()> {
    let file = std::fs::File::open(path).map_err(io_error)?;
    if file.metadata().map_err(io_error)?.len() > MAX_SOURCE_BYTES {
        return Err(EngineError::InvalidRequest(
            "annotated export exceeds the 512 MiB limit".into(),
        ));
    }
    let options = lopdf::LoadOptions {
        strict: true,
        max_decompressed_size: Some(MAX_AP_BYTES),
        ..Default::default()
    };
    let mut document = Document::load_from_with_options(file.take(MAX_SOURCE_BYTES + 1), options)
        .map_err(pdf_error)?;
    let page_ids: Vec<_> = document.get_pages().into_values().collect();
    if page_ids.len() != request.pages.len() || page_ids.len() != geometries.len() {
        return Err(EngineError::InvalidRequest(
            "portable page mapping mismatch".into(),
        ));
    }
    for (index, page_id) in page_ids.into_iter().enumerate() {
        let original = document
            .get_dictionary(page_id)
            .map_err(pdf_error)?
            .get(b"Annots")
            .ok()
            .cloned();
        let mut annotations = match original {
            Some(value) => document
                .dereference(&value)
                .map_err(pdf_error)?
                .1
                .as_array()
                .map_err(pdf_error)?
                .clone(),
            None => Vec::new(),
        };
        for annotation in &mut annotations {
            if let Object::Dictionary(dictionary) = annotation {
                let mut dictionary = dictionary.clone();
                dictionary.set("P", Object::Reference(page_id));
                *annotation = Object::Reference(document.add_object(dictionary));
            }
        }
        if !request.flatten {
            for overlay in &request.pages[index].overlays {
                if !matches!(overlay, Overlay::Text(_) | Overlay::Ink(_)) {
                    continue;
                }
                let rendered = appearance(overlay, geometries[index])?;
                let record = PortableRecord {
                    version: 1,
                    frame: geometries[index].into(),
                    overlay: overlay.clone(),
                    appearance_hash: hash(&rendered.content),
                };
                let metadata = serde_json::to_vec(&record)
                    .map_err(|error| EngineError::InvalidRequest(error.to_string()))?;
                if metadata.len() > MAX_METADATA_BYTES {
                    return Err(EngineError::InvalidRequest(
                        "portable metadata exceeds 32 MiB".into(),
                    ));
                }
                let mut resources = rendered.resources;
                if let Ok(fonts) = resources.get_mut(b"Font").and_then(Object::as_dict_mut) {
                    let font_id = document.add_object(font());
                    fonts.set("FolioFont", Object::Reference(font_id));
                }
                let ap_id=document.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>numbers(&rendered.bounds),"Resources"=>resources},rendered.content));
                let mut annotation = rendered.fields;
                annotation.set("Type", "Annot");
                annotation.set("Rect", numbers(&rendered.bounds));
                annotation.set("P", Object::Reference(page_id));
                annotation.set("F", 4);
                annotation.set("AP", dictionary! {"N"=>Object::Reference(ap_id)});
                annotation.set("Folio", Object::String(metadata, StringFormat::Hexadecimal));
                let id = match overlay {
                    Overlay::Text(text) => &text.id,
                    Overlay::Ink(ink) => &ink.id,
                    _ => unreachable!(),
                };
                annotation.set("NM", pdf_string(id));
                annotations.push(Object::Reference(document.add_object(annotation)));
            }
        }
        if !annotations.is_empty() {
            document
                .get_dictionary_mut(page_id)
                .map_err(pdf_error)?
                .set("Annots", annotations);
        }
    }
    document.save(path).map_err(io_error)?;
    Ok(())
}

fn canonical(document: &Document, object: &Object, depth: usize) -> Option<Object> {
    if depth > 16 {
        return None;
    }
    let object = document.dereference(object).ok()?.1;
    Some(match object {
        Object::Integer(value) => Object::Real(*value as f32),
        Object::String(bytes, _) => Object::String(bytes.clone(), StringFormat::Literal),
        Object::Array(values) => Object::Array(
            values
                .iter()
                .map(|value| canonical(document, value, depth + 1))
                .collect::<Option<Vec<_>>>()?,
        ),
        Object::Dictionary(values) => {
            let mut dictionary = Dictionary::new();
            for (key, value) in values.iter() {
                dictionary.set(key.clone(), canonical(document, value, depth + 1)?);
            }
            Object::Dictionary(dictionary)
        }
        Object::Stream(_) => return None,
        object => object.clone(),
    })
}

/// Import only metadata that still describes both the appearance and the public
/// annotation fields. An edited/missing AP or third-party change stays native.
pub(super) fn decode(document: &Document, annotation: &Dictionary) -> Option<PortableRecord> {
    let bytes = annotation.get(b"Folio").ok()?.as_str().ok()?;
    if bytes.len() > MAX_METADATA_BYTES {
        return None;
    }
    let record: PortableRecord = serde_json::from_slice(bytes).ok()?;
    if record.version != 1 {
        return None;
    }
    // Preserve third-party rich text, optional visibility, alternate appearances,
    // and other semantics that this editor cannot reproduce.
    let rendered = appearance(&record.overlay, record.frame.geometry()?).ok()?;
    let allowed = [
        "Type",
        "Rect",
        "P",
        "F",
        "AP",
        "Folio",
        "NM",
        "CA",
        "M",
        "CreationDate",
        "T",
        "Popup",
        "Parent",
    ];
    if annotation.iter().any(|(key, _)| {
        !rendered.fields.has(key) && !allowed.iter().any(|name| key.as_slice() == name.as_bytes())
    }) {
        return None;
    }
    if hash(&rendered.content) != record.appearance_hash {
        return None;
    }
    if annotation.get(b"F").ok()?.as_i64().ok() != Some(4) {
        return None;
    }
    if annotation
        .get(b"CA")
        .ok()
        .is_some_and(|value| value.as_float().ok() != Some(1.0))
    {
        return None;
    }
    for (key, value) in rendered.fields.iter() {
        if canonical(document, annotation.get(key).ok()?, 0)? != canonical(document, value, 0)? {
            return None;
        }
    }
    if canonical(document, annotation.get(b"Rect").ok()?, 0)?
        != canonical(document, &numbers(&rendered.bounds), 0)?
    {
        return None;
    }
    let ap = document
        .dereference(annotation.get(b"AP").ok()?)
        .ok()?
        .1
        .as_dict()
        .ok()?;
    if ap.len() != 1 {
        return None;
    }
    let stream = document
        .dereference(ap.get(b"N").ok()?)
        .ok()?
        .1
        .as_stream()
        .ok()?;
    if stream.dict.get(b"Type").ok()?.as_name().ok()? != b"XObject"
        || stream.dict.get(b"Subtype").ok()?.as_name().ok()? != b"Form"
    {
        return None;
    }
    let allowed = [
        "Type",
        "Subtype",
        "FormType",
        "BBox",
        "Matrix",
        "Resources",
        "Length",
        "Filter",
        "DecodeParms",
    ];
    if stream
        .dict
        .iter()
        .any(|(key, _)| !allowed.iter().any(|name| key.as_slice() == name.as_bytes()))
    {
        return None;
    }
    if stream
        .dict
        .get(b"FormType")
        .ok()
        .is_some_and(|value| value.as_i64().ok() != Some(1))
    {
        return None;
    }
    let contents = if stream.dict.has(b"Filter") {
        stream.decompressed_content_with_limit(MAX_AP_BYTES).ok()?
    } else {
        if stream.content.len() > MAX_AP_BYTES {
            return None;
        }
        stream.content.clone()
    };
    if hash(&contents) != record.appearance_hash {
        return None;
    }
    if canonical(document, stream.dict.get(b"BBox").ok()?, 0)?
        != canonical(document, &numbers(&rendered.bounds), 0)?
    {
        return None;
    }
    if stream.dict.get(b"Matrix").ok().is_some_and(|matrix| {
        canonical(document, matrix, 0)
            != canonical(document, &numbers(&[1., 0., 0., 1., 0., 0.]), 0)
    }) {
        return None;
    }
    if canonical(document, stream.dict.get(b"Resources").ok()?, 0)?
        != canonical(document, &Object::Dictionary(rendered.resources), 0)?
    {
        return None;
    }
    Some(record)
}

fn io_error(error: std::io::Error) -> EngineError {
    EngineError::Io(format!("portable PDF persistence: {error}"))
}
fn pdf_error(error: lopdf::Error) -> EngineError {
    EngineError::Io(format!("portable PDF persistence: {error}"))
}
