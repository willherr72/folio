//! Portable editable text/ink annotations. Their PDF appearances are independent
//! of Folio, while bounded, versioned metadata restores exact editor geometry.
use super::{EngineError, EngineResult, PageGeometry};
use crate::fonts::{FontAsset, FontRegistry};
use crate::image_persistence::{self, ImageImportCache, PreparedImageAsset};
use crate::types::*;
use lopdf::{dictionary, Dictionary, Document, Object, Stream, StringFormat};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Arc};

#[path = "custom_font_pdf.rs"]
mod custom_font_pdf;
pub(super) use custom_font_pdf::ImportCache;
pub(super) use custom_font_pdf::legacy_encoding_matches;
#[path = "overlay_render_budget.rs"]
mod render_budget;
pub(super) use render_budget::Limits as GeneratedLimits;

/// Reuse the audited full-font resource format for explicit source-text substitution.
pub(super) fn install_font_resource(
    document: &mut Document,
    asset: &FontAsset,
) -> EngineResult<Object> {
    Ok(custom_font_pdf::install(
        document,
        Object::Dictionary(custom_font_pdf::font(asset)?),
    ))
}

const MAX_METADATA_BYTES: usize = 32 * 1024 * 1024;

pub(super) fn install_paragraph_font_resource(document:&mut Document,asset:&FontAsset)->EngineResult<Object> {
    Ok(custom_font_pdf::install(document,Object::Dictionary(custom_font_pdf::paragraph_font(asset)?)))
}
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
    #[serde(skip)]
    pub(super) custom_font: Option<Arc<FontAsset>>,
    #[serde(skip)]
    pub(super) image: Option<Arc<PreparedImageAsset>>,
    /// Bound from the containing page, never from serialized annotation metadata.
    #[serde(skip)]
    unit:f32,
}

impl PortableRecord {
    pub(super) fn displayed(&self, current: PageGeometry) -> Option<Overlay> {
        let old = self.frame.geometry()?;
        let same_frame=old.left==current.left&&old.bottom==current.bottom&&old.right==current.right&&old.top==current.top&&old.rotation==current.rotation;
        let convert = |point: Point| {
            // The containing page already has this frame; a round trip through
            // PDF coordinates would add rounding to an unchanged annotation.
            if same_frame {return point;}
            let (x, y) = old.displayed_to_pdf(point);
            current.pdf_to_displayed(x.value, y.value)
        };
        let mut overlay = self.overlay.clone();
        match &mut overlay {
            Overlay::Image(image) => {
                let point=convert(Point{x:image.x,y:image.y});
                image.x=point.x; image.y=point.y;
                image.rotation=(image.rotation + current.rotation + 360 - old.rotation)%360;
                image.validate(current.displayed_size().width,current.displayed_size().height).ok()?;
            }
            Overlay::Text(text) => {
                let point = convert(Point {
                    x: text.x,
                    y: text.y,
                });
                text.x = point.x;
                text.y = point.y;
                text.rotation = (text.rotation + current.rotation + 360 - old.rotation) % 360;
                super::validate_text_in_units(
                    text,
                    current.displayed_size().width,
                    current.displayed_size().height,
                    self.custom_font.as_deref(),
                    self.unit,
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
                    self.unit,
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

fn font(name: TextFont) -> Dictionary {
    dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>name.pdf_name(),"Encoding"=>"WinAnsiEncoding"}
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

// Adapted from ReportLab 4.5.1; see THIRD_PARTY_NOTICES.md and docs/licenses/REPORTLAB.txt.
// Standard PDF font advances, in 1/1000 em, indexed by WinAnsi byte 32..255.
// Helvetica oblique faces share their upright advances; Courier is fixed at 600.
const HELVETICA_WIDTHS: [u16; 224] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
    350, 556, 350, 222, 556, 333, 1000, 556, 556, 333, 1000, 667, 333, 1000, 350, 611, 350, 350,
    222, 222, 333, 333, 350, 556, 1000, 333, 1000, 500, 333, 944, 350, 500, 667, 278, 333, 556,
    556, 556, 556, 260, 556, 333, 737, 370, 556, 584, 333, 737, 333, 400, 584, 333, 333, 333, 556,
    537, 278, 333, 333, 365, 556, 834, 834, 834, 611, 667, 667, 667, 667, 667, 667, 1000, 722, 667,
    667, 667, 667, 278, 278, 278, 278, 722, 722, 778, 778, 778, 778, 778, 584, 778, 722, 722, 722,
    722, 667, 667, 611, 556, 556, 556, 556, 556, 556, 889, 500, 556, 556, 556, 556, 278, 278, 278,
    278, 556, 556, 556, 556, 556, 556, 556, 584, 611, 556, 556, 556, 556, 500, 556, 500,
];
const HELVETICA_BOLD_WIDTHS: [u16; 224] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722, 722, 667,
    611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 333, 278, 333, 584, 556, 333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556,
    278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
    350, 556, 350, 278, 556, 500, 1000, 556, 556, 333, 1000, 667, 333, 1000, 350, 611, 350, 350,
    278, 278, 500, 500, 350, 556, 1000, 333, 1000, 556, 333, 944, 350, 500, 667, 278, 333, 556,
    556, 556, 556, 280, 556, 333, 737, 370, 556, 584, 333, 737, 333, 400, 584, 333, 333, 333, 611,
    556, 278, 333, 333, 365, 556, 834, 834, 834, 611, 722, 722, 722, 722, 722, 722, 1000, 722, 667,
    667, 667, 667, 278, 278, 278, 278, 722, 722, 778, 778, 778, 778, 778, 584, 778, 722, 722, 722,
    722, 667, 667, 611, 556, 556, 556, 556, 556, 556, 889, 556, 556, 556, 556, 556, 278, 278, 278,
    278, 611, 611, 611, 611, 611, 611, 611, 584, 611, 611, 611, 611, 611, 556, 611, 556,
];
const TIMES_ROMAN_WIDTHS: [u16; 224] = [
    250, 333, 408, 500, 500, 833, 778, 180, 333, 333, 500, 564, 250, 333, 250, 278, 500, 500, 500,
    500, 500, 500, 500, 500, 500, 500, 278, 278, 564, 564, 564, 444, 921, 722, 667, 667, 722, 611,
    556, 722, 722, 333, 389, 722, 611, 889, 722, 722, 556, 722, 667, 556, 611, 722, 722, 944, 722,
    722, 611, 333, 278, 333, 469, 500, 333, 444, 500, 444, 500, 444, 333, 500, 500, 278, 278, 500,
    278, 778, 500, 500, 500, 500, 333, 389, 278, 500, 500, 722, 500, 500, 444, 480, 200, 480, 541,
    350, 500, 350, 333, 500, 444, 1000, 500, 500, 333, 1000, 556, 333, 889, 350, 611, 350, 350,
    333, 333, 444, 444, 350, 500, 1000, 333, 980, 389, 333, 722, 350, 444, 722, 250, 333, 500, 500,
    500, 500, 200, 500, 333, 760, 276, 500, 564, 333, 760, 333, 400, 564, 300, 300, 333, 500, 453,
    250, 333, 300, 310, 500, 750, 750, 750, 444, 722, 722, 722, 722, 722, 722, 889, 667, 611, 611,
    611, 611, 333, 333, 333, 333, 722, 722, 722, 722, 722, 722, 722, 564, 722, 722, 722, 722, 722,
    722, 556, 500, 444, 444, 444, 444, 444, 444, 667, 444, 444, 444, 444, 444, 278, 278, 278, 278,
    500, 500, 500, 500, 500, 500, 500, 564, 500, 500, 500, 500, 500, 500, 500, 500,
];
const TIMES_BOLD_WIDTHS: [u16; 224] = [
    250, 333, 555, 500, 500, 1000, 833, 278, 333, 333, 500, 570, 250, 333, 250, 278, 500, 500, 500,
    500, 500, 500, 500, 500, 500, 500, 333, 333, 570, 570, 570, 500, 930, 722, 667, 722, 722, 667,
    611, 778, 778, 389, 500, 778, 667, 944, 722, 778, 611, 778, 722, 556, 667, 722, 722, 1000, 722,
    722, 667, 333, 278, 333, 581, 500, 333, 500, 556, 444, 556, 444, 333, 500, 556, 278, 333, 556,
    278, 833, 556, 500, 556, 556, 444, 389, 333, 556, 500, 722, 500, 500, 444, 394, 220, 394, 520,
    350, 500, 350, 333, 500, 500, 1000, 500, 500, 333, 1000, 556, 333, 1000, 350, 667, 350, 350,
    333, 333, 500, 500, 350, 500, 1000, 333, 1000, 389, 333, 722, 350, 444, 722, 250, 333, 500,
    500, 500, 500, 220, 500, 333, 747, 300, 500, 570, 333, 747, 333, 400, 570, 300, 300, 333, 556,
    540, 250, 333, 300, 330, 500, 750, 750, 750, 500, 722, 722, 722, 722, 722, 722, 1000, 722, 667,
    667, 667, 667, 389, 389, 389, 389, 722, 722, 778, 778, 778, 778, 778, 570, 778, 722, 722, 722,
    722, 722, 611, 556, 500, 500, 500, 500, 500, 500, 722, 444, 444, 444, 444, 444, 278, 278, 278,
    278, 500, 556, 500, 500, 500, 500, 500, 570, 500, 556, 556, 556, 556, 500, 556, 500,
];
const TIMES_ITALIC_WIDTHS: [u16; 224] = [
    250, 333, 420, 500, 500, 833, 778, 214, 333, 333, 500, 675, 250, 333, 250, 278, 500, 500, 500,
    500, 500, 500, 500, 500, 500, 500, 333, 333, 675, 675, 675, 500, 920, 611, 611, 667, 722, 611,
    611, 722, 722, 333, 444, 667, 556, 833, 667, 722, 611, 722, 611, 500, 556, 722, 611, 833, 611,
    556, 556, 389, 278, 389, 422, 500, 333, 500, 500, 444, 500, 444, 278, 500, 500, 278, 278, 444,
    278, 722, 500, 500, 500, 500, 389, 389, 278, 500, 444, 667, 444, 444, 389, 400, 275, 400, 541,
    350, 500, 350, 333, 500, 556, 889, 500, 500, 333, 1000, 500, 333, 944, 350, 556, 350, 350, 333,
    333, 556, 556, 350, 500, 889, 333, 980, 389, 333, 667, 350, 389, 556, 250, 389, 500, 500, 500,
    500, 275, 500, 333, 760, 276, 500, 675, 333, 760, 333, 400, 675, 300, 300, 333, 500, 523, 250,
    333, 300, 310, 500, 750, 750, 750, 500, 611, 611, 611, 611, 611, 611, 889, 667, 611, 611, 611,
    611, 333, 333, 333, 333, 722, 667, 722, 722, 722, 722, 722, 675, 722, 722, 722, 722, 722, 556,
    611, 500, 500, 500, 500, 500, 500, 500, 667, 444, 444, 444, 444, 444, 278, 278, 278, 278, 500,
    500, 500, 500, 500, 500, 500, 675, 500, 500, 500, 500, 500, 444, 500, 444,
];
const TIMES_BOLD_ITALIC_WIDTHS: [u16; 224] = [
    250, 389, 555, 500, 500, 833, 778, 278, 333, 333, 500, 570, 250, 333, 250, 278, 500, 500, 500,
    500, 500, 500, 500, 500, 500, 500, 333, 333, 570, 570, 570, 500, 832, 667, 667, 667, 722, 667,
    667, 722, 778, 389, 500, 667, 611, 889, 722, 722, 611, 722, 667, 556, 611, 722, 667, 889, 667,
    611, 611, 333, 278, 333, 570, 500, 333, 500, 500, 444, 500, 444, 333, 500, 556, 278, 278, 500,
    278, 778, 556, 500, 500, 500, 389, 389, 278, 556, 444, 667, 500, 444, 389, 348, 220, 348, 570,
    350, 500, 350, 333, 500, 500, 1000, 500, 500, 333, 1000, 556, 333, 944, 350, 611, 350, 350,
    333, 333, 500, 500, 350, 500, 1000, 333, 1000, 389, 333, 722, 350, 389, 611, 250, 389, 500,
    500, 500, 500, 220, 500, 333, 747, 266, 500, 606, 333, 747, 333, 400, 570, 300, 300, 333, 576,
    500, 250, 333, 300, 300, 500, 750, 750, 750, 500, 667, 667, 667, 667, 667, 667, 944, 667, 667,
    667, 667, 667, 389, 389, 389, 389, 722, 722, 722, 722, 722, 722, 722, 570, 722, 722, 722, 722,
    722, 611, 611, 500, 500, 500, 500, 500, 500, 500, 722, 444, 444, 444, 444, 444, 278, 278, 278,
    278, 500, 556, 500, 500, 500, 500, 500, 570, 500, 556, 556, 556, 556, 444, 500, 444,
];

pub(super) fn text_advance(line: &str, font: TextFont) -> EngineResult<f32> {
    let bytes = win_ansi(line)?;
    let widths = match font {
        TextFont::Helvetica | TextFont::HelveticaOblique => &HELVETICA_WIDTHS,
        TextFont::HelveticaBold | TextFont::HelveticaBoldOblique => &HELVETICA_BOLD_WIDTHS,
        TextFont::TimesRoman => &TIMES_ROMAN_WIDTHS,
        TextFont::TimesBold => &TIMES_BOLD_WIDTHS,
        TextFont::TimesItalic => &TIMES_ITALIC_WIDTHS,
        TextFont::TimesBoldItalic => &TIMES_BOLD_ITALIC_WIDTHS,
        TextFont::Courier
        | TextFont::CourierBold
        | TextFont::CourierOblique
        | TextFont::CourierBoldOblique => return Ok(bytes.len() as f32 * 0.6),
    };
    Ok(bytes
        .iter()
        .map(|byte| widths[usize::from(*byte - 32)] as f32 / 1000.0)
        .sum())
}

#[derive(Clone,Copy,Default)]
struct RenderResources<'a> {font:Option<&'a Object>,image:Option<lopdf::ObjectId>}

fn image_appearance(image: &ImageOverlay, geometry: PageGeometry, asset: &PreparedImageAsset,installed:Option<lopdf::ObjectId>) -> EngineResult<Appearance> {
    use lopdf::content::{Content, Operation};
    image.validate(geometry.displayed_size().width, geometry.displayed_size().height)?;
    if image.asset_id != asset.asset.info().id {
        return Err(EngineError::InvalidRequest("Image resource identity mismatch.".into()));
    }
    let points = image.corners().map(|p| { let (x,y)=geometry.displayed_to_pdf(p); (x.value,y.value) });
    let origin=points[3];
    let matrix=[points[2].0-origin.0,points[2].1-origin.1,points[0].0-origin.0,points[0].1-origin.1,origin.0,origin.1];
    let bounds=[points.iter().map(|p|p.0).fold(f32::INFINITY,f32::min),points.iter().map(|p|p.1).fold(f32::INFINITY,f32::min),
        points.iter().map(|p|p.0).fold(f32::NEG_INFINITY,f32::max),points.iter().map(|p|p.1).fold(f32::NEG_INFINITY,f32::max)];
    let content=Content {operations:vec![Operation::new("q",vec![]),Operation::new("cm",matrix.into_iter().map(Object::Real).collect()),
        Operation::new("Do",vec![Object::Name(b"FolioImage".to_vec())]),Operation::new("Q",vec![])]}.encode().map_err(pdf_error)?;
    Ok(Appearance {content,bounds,resources:dictionary! {"XObject"=>dictionary! {"FolioImage"=>installed.map(Object::Reference).unwrap_or_else(||Object::Stream(asset.prepared.form().clone()))}},
        fields:dictionary! {"Subtype"=>"Stamp","Name"=>"FolioImage","Contents"=>pdf_string("Image")}})
}

fn appearance(
    overlay: &Overlay,
    geometry: PageGeometry,
    legacy_text: bool,
    custom: Option<&FontAsset>,
    image: Option<&PreparedImageAsset>,
    unit:f32,
    installed:RenderResources<'_>,
) -> EngineResult<Appearance> {
    if let Overlay::Image(overlay) = overlay {
        return image_appearance(overlay, geometry, image.ok_or_else(|| EngineError::InvalidRequest("Image resource is unavailable.".into()))?,installed.image);
    }
    #[cfg(feature = "shaped-text")]
    if let Overlay::Text(text) = overlay {
        if text.shaping.is_some() {
            return shaped_appearance(text, geometry, custom,unit,installed.font);
        }
    }
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
            super::validate_text_in_units(
                text,
                geometry.displayed_size().width,
                geometry.displayed_size().height,
                custom,
                unit,
            )?;
            let color = rgb(&text.color)?;
            resources.set(
                "Font",
                dictionary! {"FolioFont"=>match installed.font {
                    Some(value)=>value.clone(),
                    None=>Object::Dictionary(match custom {Some(asset)=>custom_font_pdf::font(asset)?,None=>font(text.font_name)}),
                }},
            );
            fields.set("Subtype", "FreeText");
            fields.set("Contents", pdf_string(&text.text));
            fields.set(
                "DA",
                Object::string_literal(format!(
                    "/{} {} Tf {} {} {} rg",
                    if legacy_text { "Helv" } else { "FolioFont" },
                    text.font_size,
                    color[0],
                    color[1],
                    color[2]
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
                        vec![Object::String(
                            if custom.is_some() {
                                line.encode_utf16().flat_map(u16::to_be_bytes).collect()
                            } else {
                                win_ansi(line)?
                            },
                            if custom.is_some() {
                                StringFormat::Hexadecimal
                            } else {
                                StringFormat::Literal
                            },
                        )],
                    ),
                    Operation::new("ET", vec![]),
                ]);
            }
            // Conservative metric bounds prevent clipping descenders, accents and
            // side bearings. Text placement itself uses exact font-size baselines.
            let width = if legacy_text {
                lines
                    .iter()
                    .map(|line| line.chars().count())
                    .max()
                    .unwrap_or(0) as f32
                    * text.font_size
                    * 1.2
            } else {
                lines
                    .iter()
                    .map(|line| match custom {
                        Some(font) => line.chars().map(|c| font.advance(c)).sum(),
                        None => text_advance(line, text.font_name),
                    })
                    .collect::<EngineResult<Vec<_>>>()?
                    .into_iter()
                    .fold(0.0, f32::max)
                    * text.font_size
            };
            let height = lines.len().max(1) as f32 * text.font_size * 1.2;
            let pad = custom
                .map(|font| {
                    let face = font.face()?;
                    let bounds = face.global_bounding_box();
                    Ok::<f32, EngineError>(
                        [bounds.x_min, bounds.x_max, bounds.y_min, bounds.y_max]
                            .into_iter()
                            .map(|n| (n as f32).abs() / face.units_per_em() as f32)
                            .fold(1.0, f32::max)
                            * text.font_size,
                    )
                })
                .transpose()?
                .unwrap_or(text.font_size);
            for (x, y) in [
                (-pad, -pad),
                (width + pad, -pad),
                (-pad, height + pad),
                (width + pad, height + pad),
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
                unit,
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

pub(super) fn finalize_generated(
    document:&mut Document,request:&ExportRequest,geometries:&[PageGeometry],
    registry:&FontRegistry,image_registry:&crate::ImageRegistry,limits:GeneratedLimits,
    mut check:impl FnMut(usize,Option<usize>)->EngineResult<()>,
)->EngineResult<()> {
    if request.flatten{return Err(EngineError::InvalidRequest("generated overlay staging must retain editable appearances".into()));}
    let budget=render_budget::Budget::new(document,limits,&mut ||check(0,Some(0)))?;
    finalize_inner(document,request,geometries,registry,image_registry,true,Some(budget),check)
}

pub(super) fn finalize(
    document: &mut Document,
    request: &ExportRequest,
    geometries: &[PageGeometry],
    registry: &FontRegistry,
    image_registry: &crate::ImageRegistry,
    persist_images:bool,
    // None reports completed pages; Some checks cancellation within that page.
    check: impl FnMut(usize, Option<usize>) -> EngineResult<()>,
) -> EngineResult<()> {
    finalize_inner(document,request,geometries,registry,image_registry,persist_images,None,check)
}
fn finalize_inner(
    document:&mut Document,request:&ExportRequest,geometries:&[PageGeometry],
    registry:&FontRegistry,image_registry:&crate::ImageRegistry,persist_images:bool,
    mut budget:Option<render_budget::Budget>,
    mut check:impl FnMut(usize,Option<usize>)->EngineResult<()>,
)->EngineResult<()> {
    check(0,None)?;
    let page_ids: Vec<_> = document.get_pages().into_values().collect();
    if page_ids.len() != request.pages.len() || page_ids.len() != geometries.len() {
        return Err(EngineError::InvalidRequest(
            "portable page mapping mismatch".into(),
        ));
    }
    let mut embedded_fonts = HashMap::new();
    let mut prepared_images: HashMap<String, Arc<PreparedImageAsset>> = HashMap::new();
    let mut embedded_images = HashMap::new();
    let mut prepared_image_bytes = 0usize;
    for (index, page_id) in page_ids.into_iter().enumerate() {
        check(index,Some(0))?;
        let unit=super::physical_units::page_unit(document,page_id)?;
        let mut wrapped_contents = false;
        let flatten_portable = request.flatten
            && request.pages[index]
                .overlays
                .iter()
                .any(|overlay| matches!(overlay, Overlay::Text(_) | Overlay::Image(_)));
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
        let original_annotation_count = annotations.len();
        for annotation in &mut annotations {
            if let Object::Dictionary(dictionary) = annotation {
                let mut dictionary = dictionary.clone();
                dictionary.set("P", Object::Reference(page_id));
                *annotation = Object::Reference(document.add_object(dictionary));
            }
        }
        {
            for (overlay_index,overlay) in request.pages[index].overlays.iter().enumerate() {
                check(index,Some(overlay_index))?;
                if !matches!(overlay, Overlay::Text(_) | Overlay::Ink(_) | Overlay::Image(_)) {
                    continue;
                }
                let custom = match overlay {
                    Overlay::Text(text) => text
                        .font_id
                        .as_deref()
                        .map(|id| registry.get(id))
                        .transpose()?,
                    _ => None,
                };
                if request.flatten && !flatten_portable {
                    continue;
                }
                let image = if let Overlay::Image(overlay) = overlay {
                    if !prepared_images.contains_key(&overlay.asset_id) {
                        let asset = image_registry.get(&overlay.asset_id)?;
                        let prepared = image_registry.prepared_pdf(&overlay.asset_id)?;
                        check(index,Some(overlay_index))?;
                        prepared_image_bytes = prepared_image_bytes.checked_add(image_persistence::resource_bytes(prepared.form()))
                            .filter(|value| *value <= 512 * 1024 * 1024)
                            .ok_or_else(|| EngineError::InvalidRequest("Prepared image resources exceed 512 MiB.".into()))?;
                        prepared_images.insert(overlay.asset_id.clone(), Arc::new(PreparedImageAsset {asset,prepared}));
                    }
                    prepared_images.get(&overlay.asset_id).cloned()
                } else {None};
                // Admit shared assets before copying their payloads into the
                // output graph. Appearance construction receives references,
                // so later overlays do not rebuild/copy an already shared font
                // or an image form merely to replace it with that reference.
                if let Some(asset)=&custom {
                    if !embedded_fonts.contains_key(&asset.info.id){
                        if let Some(b)=&budget{b.font_inputs(asset)?;}
                        let value=Object::Dictionary(custom_font_pdf::font(asset)?);
                        if let Some(b)=&mut budget{b.font(&value,&mut ||check(index,Some(overlay_index)))?;}
                        let value=custom_font_pdf::install(document,value);
                        embedded_fonts.insert(asset.info.id.clone(),value);
                    }
                }
                if let Some(image)=&image {
                    if !embedded_images.contains_key(&image.asset.info().id){
                        let persist=!request.flatten&&persist_images;
                        if let Some(b)=&mut budget {
                            b.image(image.prepared.form(),&mut ||check(index,Some(overlay_index)))?;
                            if persist {b.attachment(&image_persistence::attachment_dictionary(&image.asset),image.asset.bytes(),&mut ||check(index,Some(overlay_index)))?;}
                        }
                        let form=image.prepared.install_copy(document);
                        let file=persist.then(||document.add_object(image_persistence::attachment(&image.asset)));
                        embedded_images.insert(image.asset.info().id.clone(),(form,file));
                    }
                }
                let image_resource=image.as_ref().and_then(|image|embedded_images.get(&image.asset.info().id)).copied();
                let installed=RenderResources{font:custom.as_ref().and_then(|asset|embedded_fonts.get(&asset.info.id)),image:image_resource.map(|v|v.0)};
                let rendered = appearance(overlay, geometries[index], false, custom.as_deref(), image.as_deref(),unit,installed)?;
                check(index,Some(overlay_index))?;
                let record = PortableRecord {
                    version: if image.is_some() { 5 } else if matches!(overlay,Overlay::Text(t) if t.area.is_some()) {
                        4
                    } else if matches!(overlay,Overlay::Text(t) if t.shaping.is_some()) {
                        3
                    } else if custom.is_some() {
                        2
                    } else {
                        1
                    },
                    unit,
                    custom_font: None,
                    image: None,
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
                    let original=fonts.remove(b"FolioFont").ok_or_else(||EngineError::InvalidRequest("missing generated font".into()))?;
                    let resource=if matches!(original,Object::Reference(_)){original}else{
                        if let Some(b)=&mut budget{b.font(&original,&mut ||check(index,Some(overlay_index)))?;}
                        Object::Reference(document.add_object(original))
                    };
                    fonts.set("FolioFont", resource);
                    let semantic:Vec<_>=fonts.iter().filter(|(key,_)|key.as_slice()!=b"FolioFont").map(|(key,_)|key.clone()).collect();
                    for key in semantic {
                        let value=fonts.remove(&key).expect("collected generated font");
                        if let Some(b)=&mut budget{b.font(&value,&mut ||check(index,Some(overlay_index)))?;}
                        fonts.set(key, custom_font_pdf::install(document, value));
                    }
                }
                let image_attachment=image_resource.and_then(|v|v.1);
                let ap=Object::Stream(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>numbers(&rendered.bounds),"Resources"=>resources},rendered.content));
                if let Some(b)=&mut budget{b.object(&ap,&mut ||check(index,Some(overlay_index)))?;}
                let ap_id=document.add_object(ap);
                if request.flatten {
                    append_appearance(document, page_id, ap_id, &mut wrapped_contents)?;
                    continue;
                }
                let mut annotation = rendered.fields;
                annotation.set("Type", "Annot");
                annotation.set("Rect", numbers(&rendered.bounds));
                annotation.set("P", Object::Reference(page_id));
                annotation.set("F", 4);
                annotation.set("AP", dictionary! {"N"=>Object::Reference(ap_id)});
                annotation.set("Folio", Object::String(metadata, StringFormat::Hexadecimal));
                if let Some(file)=image_attachment { annotation.set("FolioImageAsset",file); }
                let id = match overlay {
                    Overlay::Image(image) => &image.id,
                    Overlay::Text(text) => &text.id,
                    Overlay::Ink(ink) => &ink.id,
                    _ => unreachable!(),
                };
                annotation.set("NM", pdf_string(id));
                let annotation=Object::Dictionary(annotation);
                if let Some(b)=&mut budget{b.annotation(&annotation,&mut ||check(index,Some(overlay_index)))?;}
                annotations.push(Object::Reference(document.add_object(annotation)));
            }
        }
        // Placed images become page content when flattened, underneath PDF
        // annotations. Use the same layering in editable/preview/print output:
        // keep text/ink/images in their relative order beneath native annotations,
        // highlights and comments. Preserve the existing path on image-free pages.
        if !request.flatten && request.pages[index].overlays.iter().any(|o| matches!(o, Overlay::Image(_))) {
            let mut drawings = annotations.split_off(original_annotation_count);
            drawings.append(&mut annotations);
            annotations = drawings;
        }
        if !annotations.is_empty() {
            document
                .get_dictionary_mut(page_id)
                .map_err(pdf_error)?
                .set("Annots", annotations);
        }
        check(index+1,None)?;
    }
    Ok(())
}

#[cfg(test)]
mod save_cancellation_tests {
    use super::*;
    #[test]
    fn cancellation_between_overlays_skips_the_second_appearance() {
        let mut document=Document::with_version("1.7");let tree=document.new_object_id();
        let page=document.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary!{}});
        document.objects.insert(tree,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![page.into()]}.into());
        let root=document.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree});document.trailer.set("Root",root);
        let overlays:Vec<_>=["first","second"].iter().map(|id|serde_json::json!({"type":"text","id":id,"x":20,"y":40,"text":id,"fontSize":12,"fontName":"Helvetica","color":"#123456","rotation":0})).collect();
        let request:ExportRequest=serde_json::from_value(serde_json::json!({"pages":[{"id":"p","sourceId":"s","pageIndex":0,"width":300,"height":400,"rotation":0,"overlays":overlays}]})).unwrap();
        let geometry=PageGeometry{left:0.,bottom:0.,right:300.,top:400.,rotation:0};
        let control=crate::PdfSaveControl::new();let mut stopped=false;
        let result=finalize(&mut document,&request,&[geometry],&FontRegistry::default(),&crate::ImageRegistry::default(),true,|_,overlay|{
            if overlay==Some(1){stopped=true;control.cancel();}
            if control.cancelled(){Err(EngineError::InvalidRequest("cancelled".into()))}else{Ok(())}
        });
        assert!(stopped&&result.is_err());
        let appearances=document.objects.values().filter(|object|object.as_stream().is_ok_and(|s|s.dict.get(b"Type").and_then(Object::as_name).is_ok_and(|name|name==b"XObject"))).count();
        assert_eq!(appearances,1,"The first appearance was rendered; cancellation skipped the second");
    }
}

#[cfg(test)]
pub(super) fn legacy_flatten_reference(document:&mut Document,page_id:lopdf::ObjectId,forms:&[lopdf::ObjectId])->EngineResult<()> {
    // Test baseline through the established flatten primitive. Terminate a
    // pending source path explicitly; the new adapter proves when this is safe.
    document.add_page_contents(page_id,b"\nn\n".to_vec()).map_err(pdf_error)?;
    let mut wrapped=false;for form in forms{append_appearance(document,page_id,*form,&mut wrapped)?;}Ok(())
}

fn append_appearance(
    document: &mut Document,
    page_id: lopdf::ObjectId,
    ap_id: lopdf::ObjectId,
    wrapped: &mut bool,
) -> EngineResult<()> {
    let mut current = page_id;
    let mut resources = Dictionary::new();
    for _ in 0..64 {
        let page = document.get_dictionary(current).map_err(pdf_error)?;
        if let Ok(value) = page.get(b"Resources") {
            resources = document
                .dereference(value)
                .map_err(pdf_error)?
                .1
                .as_dict()
                .map_err(pdf_error)?
                .clone();
            break;
        }
        let Ok(parent) = page.get(b"Parent").and_then(Object::as_reference) else {
            break;
        };
        current = parent;
    }
    let mut xobjects = match resources.get(b"XObject") {
        Ok(value) => document
            .dereference(value)
            .map_err(pdf_error)?
            .1
            .as_dict()
            .map_err(pdf_error)?
            .clone(),
        Err(_) => Dictionary::new(),
    };
    let mut name = format!("FolioText{}", ap_id.0);
    while xobjects.has(name.as_bytes()) {
        name.push('_');
    }
    xobjects.set(name.as_bytes(), Object::Reference(ap_id));
    resources.set("XObject", xobjects);
    document
        .get_dictionary_mut(page_id)
        .map_err(pdf_error)?
        .set("Resources", resources);
    if !*wrapped {
        let mut contents = vec![Object::Reference(
            document.add_object(Stream::new(dictionary! {}, b"q\n".to_vec())),
        )];
        contents.extend(
            document
                .get_page_contents(page_id)
                .into_iter()
                .map(Object::Reference),
        );
        contents.push(Object::Reference(
            document.add_object(Stream::new(dictionary! {}, b"\nQ\n".to_vec())),
        ));
        document
            .get_dictionary_mut(page_id)
            .map_err(pdf_error)?
            .set("Contents", contents);
        *wrapped = true;
    }
    document
        .add_page_contents(page_id, format!("q /{name} Do Q\n").into_bytes())
        .map_err(pdf_error)
}

fn stream_content(stream: &Stream, limit: usize) -> Option<Vec<u8>> {
    if stream.dict.has(b"Filter") {
        stream.decompressed_content_with_limit(limit).ok()
    } else if stream.content.len() <= limit {
        Some(stream.content.clone())
    } else {
        None
    }
}

/// Compare only the expected resource shape. Reject extra keys/array elements
/// before descending, and never expand an attacker-controlled object graph into
/// a canonical tree. Stream decompression is bounded by the expected byte count.
pub(crate) fn same_object(document: &Document, actual: &Object, expected: &Object) -> bool {
    fn compare(
        document: &Document,
        actual: &Object,
        expected: &Object,
        depth: usize,
        remaining: &mut usize,
    ) -> Option<bool> {
        if depth > 16 || *remaining == 0 {
            return None;
        }
        *remaining -= 1;
        let actual = document.dereference(actual).ok()?.1;
        let expected = document.dereference(expected).ok()?.1;
        Some(match (actual, expected) {
            (Object::Integer(_) | Object::Real(_), Object::Integer(_) | Object::Real(_)) => {
                actual.as_float().ok()? == expected.as_float().ok()?
            }
            (Object::String(a, _), Object::String(b, _)) => a == b,
            (Object::Array(a), Object::Array(b)) => {
                if a.len() != b.len() {
                    return Some(false);
                }
                for (a, b) in a.iter().zip(b) {
                    if !compare(document, a, b, depth + 1, remaining)? {
                        return Some(false);
                    }
                }
                true
            }
            (Object::Dictionary(a), Object::Dictionary(b)) => {
                if a.len() != b.len() || a.iter().any(|(key, _)| !b.has(key)) {
                    return Some(false);
                }
                for (key, b) in b.iter() {
                    if !compare(document, a.get(key).ok()?, b, depth + 1, remaining)? {
                        return Some(false);
                    }
                }
                true
            }
            (Object::Stream(a), Object::Stream(b)) => {
                let transport = |key: &[u8]| matches!(key, b"Length" | b"Filter" | b"DecodeParms");
                let a_len = a.dict.iter().filter(|(key, _)| !transport(key)).count();
                let b_len = b.dict.iter().filter(|(key, _)| !transport(key)).count();
                if a_len != b_len
                    || a.dict
                        .iter()
                        .any(|(key, _)| !transport(key) && !b.dict.has(key))
                {
                    return Some(false);
                }
                for (key, b) in b.dict.iter().filter(|(key, _)| !transport(key)) {
                    if !compare(document, a.dict.get(key).ok()?, b, depth + 1, remaining)? {
                        return Some(false);
                    }
                }
                // Expected streams are generated locally and are uncompressed.
                stream_content(a, b.content.len())? == b.content
            }
            (Object::Name(a), Object::Name(b)) => a == b,
            (Object::Boolean(a), Object::Boolean(b)) => a == b,
            (Object::Null, Object::Null) => true,
            _ => false,
        })
    }
    compare(document, actual, expected, 0, &mut 100_000).unwrap_or(false)
}

/// Import only metadata that still describes both the appearance and the public
/// annotation fields. An edited/missing AP or third-party change stays native.
#[allow(dead_code)]
pub(super) fn decode(document:&Document,annotation:&Dictionary,fonts:&mut ImportCache,images:&mut ImageImportCache<'_>,unit:f32)->Option<PortableRecord>{
    decode_bound(document,annotation,fonts,images,unit,None)
}
pub(super) fn decode_bound(
    document: &Document,
    annotation: &Dictionary,
    fonts: &mut ImportCache,
    images: &mut ImageImportCache<'_>,
    unit:f32,
    semantic:Option<&super::annotation_import_semantics::Proof<'_>>,
) -> Option<PortableRecord> {
    let bytes = annotation.get(b"Folio").ok()?.as_str().ok()?;
    if bytes.len() > MAX_METADATA_BYTES {
        return None;
    }
    let metadata: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let legacy_text = metadata.get("overlay")?.get("fontName").is_none();
    let mut record: PortableRecord = serde_json::from_value(metadata).ok()?;
    record.unit=unit;
    let font_id = match &record.overlay {
        Overlay::Text(text) => text.font_id.as_deref(),
        _ => None,
    };
    if let Overlay::Image(image) = &record.overlay {
        if record.version != 5 { return None; }
        record.image = Some(images.read_asset(annotation.get(b"FolioImageAsset").ok()?, &image.asset_id)?);
    } else if let Some(id) = font_id {
        if record.version
            != if matches!(&record.overlay,Overlay::Text(t) if t.area.is_some()) {
                4
            } else if matches!(&record.overlay,Overlay::Text(t) if t.shaping.is_some()) {
                3
            } else {
                2
            }
        {
            return None;
        }
        record.custom_font = Some(custom_font_pdf::embedded_asset(
            document, annotation, id, fonts,
        )?);
    } else if record.version != 1 {
        return None;
    }
    // Preserve third-party rich text, optional visibility, alternate appearances,
    // and other semantics that this editor cannot reproduce.
    let geometry=record.frame.geometry()?;
    let render = |scale| appearance(
        &record.overlay,
        geometry,
        legacy_text,
        record.custom_font.as_deref(),
        record.image.as_deref(),
        scale,
        RenderResources::default(),
    );
    let rendered=render(unit).ok().filter(|value|hash(&value.content)==record.appearance_hash)
        .or_else(||{
            // Older writers shaped in raw coordinates. Their exact appearance
            // remains admissible only under all the same stream, resources,
            // public-field and bounds proofs below. No metadata scale is trusted.
            if unit==1.||!matches!(&record.overlay,Overlay::Text(text) if text.shaping.is_some()){return None;}
            render(1.).ok().filter(|value|hash(&value.content)==record.appearance_hash)
        })?;
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
        !rendered.fields.has(key) && !(record.image.is_some() && key == b"FolioImageAsset") && !allowed.iter().any(|name| key.as_slice() == name.as_bytes()) && !semantic.is_some_and(|proof|proof.permits(document,annotation,key))
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
        if !same_object(document, annotation.get(key).ok()?, value) {
            return None;
        }
    }
    if !same_object(
        document,
        annotation.get(b"Rect").ok()?,
        &numbers(&rendered.bounds),
    ) {
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
    if !same_object(
        document,
        stream.dict.get(b"BBox").ok()?,
        &numbers(&rendered.bounds),
    ) {
        return None;
    }
    if stream
        .dict
        .get(b"Matrix")
        .ok()
        .is_some_and(|matrix| !same_object(document, matrix, &numbers(&[1., 0., 0., 1., 0., 0.])))
    {
        return None;
    }
    let resources = stream.dict.get(b"Resources").ok()?;
    let matches = if record.image.is_some() {
        image_persistence::matches_resources(document, resources, &rendered.resources)
    } else {
        let mut expected = Object::Dictionary(rendered.resources);
        if same_object(document, resources, &expected) { true }
        else if record.custom_font.is_some() {
            // Only the locally regenerated FolioFont slot may use historical
            // bytes. All other resources, fields and appearance content retain
            // the exact comparison above and below this compatibility choice.
            let font = expected.as_dict_mut().ok()?.get_mut(b"Font").ok()?.as_dict_mut().ok()?
                .get_mut(b"FolioFont").ok()?.as_dict_mut().ok()?;
            custom_font_pdf::use_legacy_unicode(font);
            same_object(document, resources, &expected)
        } else { false }
    };
    if !matches { return None; }
    Some(record)
}


fn pdf_error(error: lopdf::Error) -> EngineError {
    EngineError::Io(format!("portable PDF persistence: {error}"))
}

#[cfg(feature = "shaped-text")]
fn shaped_appearance(
    text: &TextOverlay,
    geometry: PageGeometry,
    custom: Option<&FontAsset>,
    unit:f32,
    installed_font:Option<&Object>,
) -> EngineResult<Appearance> {
    super::validate_id("text id", &text.id)?;
    super::validate_point(
        text.x,
        text.y,
        geometry.displayed_size().width,
        geometry.displayed_size().height,
    )?;
    let font = custom
        .ok_or_else(|| EngineError::InvalidRequest("Shaped text requires a custom font.".into()))?;
    if unit!=1. {super::validate_raw_text_geometry(text,geometry.displayed_size().width,geometry.displayed_size().height)?;}
    let physical=super::physical_units::text_in_points(text,unit)?;
    let prepared = crate::prepare_text_overlay(font, &physical)?;
    let scale=unit.recip();
    let color = prepared.preview.color;
    let mut resources = prepared.resources;
    if !resources.has(b"Font") {
        resources.set("Font", Dictionary::new());
    }
    resources
        .get_mut(b"Font")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("FolioFont",match installed_font{Some(value)=>value.clone(),None=>Object::Dictionary(custom_font_pdf::font(font)?)});
    let point = |x, y| {
        let p = rotated_point(Point { x, y }, text.rotation);
        let (x, y) = geometry.displayed_to_pdf(Point {
            x: text.x + p.x,
            y: text.y + p.y,
        });
        (x.value, y.value)
    };
    let (e, f) = point(-prepared.origin[0]*scale, text.font_size + prepared.origin[1]*scale);
    let angle = (geometry.rotation + 360 - text.rotation) % 360;
    let (a, b, c, d) = match angle {
        0 => (1., 0., 0., 1.),
        90 => (0., 1., -1., 0.),
        180 => (-1., 0., 0., -1.),
        _ => (0., -1., 1., 0.),
    };
    let (a,b,c,d)=(a*scale,b*scale,c*scale,d*scale);
    let mut content = format!("q {a} {b} {c} {d} {e:.7} {f:.7} cm\n").into_bytes();
    content.extend_from_slice(&prepared.content);
    content.extend_from_slice(b"\nQ\n");
    let rect = prepared.bounds;
    let mut bounds = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for (x, y) in [
        (rect.x, rect.y),
        (rect.x + rect.width, rect.y),
        (rect.x, rect.y + rect.height),
        (rect.x + rect.width, rect.y + rect.height),
    ] {
        let (x, y) = point(x*scale, y*scale);
        bounds[0] = bounds[0].min(x);
        bounds[1] = bounds[1].min(y);
        bounds[2] = bounds[2].max(x);
        bounds[3] = bounds[3].max(y);
    }
    let fields = dictionary! {"Subtype"=>"FreeText","Contents"=>pdf_string(&text.text),"DA"=>Object::string_literal(format!("/FolioFont {} Tf {} {} {} rg",text.font_size,color[0],color[1],color[2])),"C"=>numbers(&color),"Q"=>0};
    Ok(Appearance {
        content,
        bounds,
        resources,
        fields,
    })
}
