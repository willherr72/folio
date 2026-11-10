//! Validated field frame settings. Publication is integrated separately.
#![allow(dead_code)]
use crate::form_frame_paint::PaintColor;
use crate::{AnnotationRect, EngineError, EngineResult, FormFieldValue};
use lopdf::{Dictionary, Document, Object};
use serde::{Deserialize, Serialize};

#[path = "form_frame_document.rs"]
mod document;
pub(crate) use document::{change, inspect};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormFrameColor {
    pub color_space: String,
    pub components: Vec<f32>,
    pub preview: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormFrameAppearance {
    pub style_key: String,
    pub width: f32,
    pub style: Option<FrameStyle>,
    pub border: FormFrameColor,
    pub background: FormFrameColor,
    pub supported_styles: Vec<FrameStyle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FrameStyle {
    Solid,
    Dashed,
    Underline,
    Beveled,
    Inset,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "mode",
    content = "color",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum FrameColorChange {
    Transparent,
    Rgb(String),
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FormFrameChanges {
    pub width: Option<f32>,
    pub style: Option<FrameStyle>,
    pub border: Option<FrameColorChange>,
    pub background: Option<FrameColorChange>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetFormFrameRequest {
    pub annotation_index: usize,
    pub expected_bounds: AnnotationRect,
    pub expected_value: FormFieldValue,
    pub expected_style_key: String,
    pub changes: FormFrameChanges,
}
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FramePalette {
    Missing,
    Transparent,
    Color(PaintColor),
}
#[derive(Debug, Clone)]
pub(crate) struct FrameMetadata {
    pub(crate) width: f64,
    pub(crate) style: FrameStyle,
    pub(crate) dash: Vec<f64>,
    pub(crate) border: FramePalette,
    pub(crate) background: FramePalette,
}
fn bad(reason: &str) -> EngineError {
    EngineError::InvalidRequest(format!("Form frame: {reason}"))
}
impl SetFormFrameRequest {
    pub(crate) fn validate(&self) -> EngineResult<()> {
        let c = &self.changes;
        if c.width.is_none() && c.style.is_none() && c.border.is_none() && c.background.is_none() {
            return Err(bad(
                "change a border or background setting before previewing",
            ));
        }
        if c.width
            .is_some_and(|w| !w.is_finite() || !(0. ..=32.).contains(&w))
        {
            return Err(bad("border width must be 0–32 pt"));
        }
        for color in [&c.border, &c.background].into_iter().flatten() {
            if let FrameColorChange::Rgb(c) = color {
                rgb(c)?;
            }
        }
        let r = &self.expected_bounds;
        if ![r.x, r.y, r.width, r.height, r.x + r.width, r.y + r.height]
            .iter()
            .all(|n| n.is_finite())
            || r.width <= 0.
            || r.height <= 0.
            || self.expected_style_key.len() != 64
            || !self
                .expected_style_key
                .as_bytes()
                .iter()
                .all(u8::is_ascii_hexdigit)
        {
            return Err(bad("invalid expected field identity"));
        }
        Ok(())
    }
}
pub(crate) fn rgb(s: &str) -> EngineResult<PaintColor> {
    if s.len() != 7 || !s.starts_with('#') || !s.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit) {
        return Err(bad("choose a color as #RRGGBB"));
    }
    let mut out = [0.; 3];
    for (i, n) in out.iter_mut().enumerate() {
        *n = f64::from(
            u8::from_str_radix(&s[1 + i * 2..3 + i * 2], 16)
                .map_err(|_| bad("invalid RGB color"))?,
        ) / 255.;
    }
    Ok(PaintColor::Rgb(out))
}
fn resolve<'a>(doc: &'a Document, mut value: &'a Object) -> EngineResult<&'a Object> {
    for _ in 0..64 {
        match value {
            Object::Reference(id) => {
                value = doc
                    .objects
                    .get(id)
                    .ok_or_else(|| bad("missing frame settings reference"))?
            }
            _ => return Ok(value),
        }
    }
    Err(bad("frame settings references exceed 64 levels"))
}
fn optional<'a>(
    doc: &'a Document,
    d: &'a Dictionary,
    key: &[u8],
) -> EngineResult<Option<&'a Object>> {
    match d.get(key) {
        Ok(v) => match resolve(doc, v)? {
            Object::Null => Ok(None),
            v => Ok(Some(v)),
        },
        Err(_) => Ok(None),
    }
}
fn dictionary<'a>(
    doc: &'a Document,
    d: &'a Dictionary,
    key: &[u8],
) -> EngineResult<Option<&'a Dictionary>> {
    optional(doc, d, key)?
        .map(|v| {
            v.as_dict()
                .map_err(|_| bad("invalid frame settings dictionary"))
        })
        .transpose()
}
fn number(doc: &Document, v: &Object) -> EngineResult<f64> {
    let n = match resolve(doc, v)? {
        Object::Integer(n) => *n as f64,
        Object::Real(n) => f64::from(*n),
        _ => return Err(bad("invalid numeric frame setting")),
    };
    if !n.is_finite() || n.abs() > 1_000_000. {
        return Err(bad("nonfinite or excessive frame setting"));
    }
    Ok(n)
}
fn width(doc: &Document, v: &Object) -> EngineResult<f64> {
    let n = number(doc, v)?;
    if n < 0. {
        Err(bad("negative border width"))
    } else {
        Ok(n)
    }
}
fn dash(doc: &Document, v: &Object) -> EngineResult<Vec<f64>> {
    let a = resolve(doc, v)?
        .as_array()
        .map_err(|_| bad("invalid border dash array"))?;
    if a.len() > 64 {
        return Err(bad("border dash array exceeds 64 values"));
    }
    let n = a
        .iter()
        .map(|v| width(doc, v))
        .collect::<EngineResult<Vec<_>>>()?;
    if !n.is_empty() && n.iter().all(|n| *n == 0.) {
        return Err(bad("border dash array is entirely zero"));
    }
    Ok(n)
}
fn palette(doc: &Document, mk: Option<&Dictionary>, key: &[u8]) -> EngineResult<FramePalette> {
    let Some(mk) = mk else {
        return Ok(FramePalette::Missing);
    };
    let Some(value) = optional(doc, mk, key)? else {
        return Ok(FramePalette::Missing);
    };
    let a = value
        .as_array()
        .map_err(|_| bad("invalid frame color array"))?;
    if ![0, 1, 3, 4].contains(&a.len()) {
        return Err(bad("frame color must be transparent, gray, RGB or CMYK"));
    }
    let n = a
        .iter()
        .map(|v| number(doc, v))
        .collect::<EngineResult<Vec<_>>>()?;
    if n.iter().any(|v| !(0. ..=1.).contains(v)) {
        return Err(bad("frame color components must be 0–1"));
    }
    Ok(match n.len() {
        0 => FramePalette::Transparent,
        1 => FramePalette::Color(PaintColor::Gray(n[0])),
        3 => FramePalette::Color(PaintColor::Rgb([n[0], n[1], n[2]])),
        _ => FramePalette::Color(PaintColor::Cmyk([n[0], n[1], n[2], n[3]])),
    })
}
pub(crate) fn metadata(doc: &Document, widget: &Dictionary) -> EngineResult<FrameMetadata> {
    // Validate even inactive legacy entries. Some renderer fill paths consult
    // them first, and publication must not propagate a malformed fallback.
    let legacy = optional(doc, widget, b"Border")?
        .map(|v| -> EngineResult<_> {
            let a = v.as_array().map_err(|_| bad("invalid legacy border"))?;
            if !(3..=4).contains(&a.len()) {
                return Err(bad("legacy border requires 3 or 4 values"));
            }
            if number(doc, &a[0])? != 0. || number(doc, &a[1])? != 0. {
                return Err(bad("rounded legacy borders cannot yet be edited"));
            }
            Ok((
                width(doc, &a[2])?,
                if a.len() == 4 {
                    dash(doc, &a[3])?
                } else {
                    vec![]
                },
            ))
        })
        .transpose()?;
    let bs = dictionary(doc, widget, b"BS")?;
    let (width, style, dash) = if let Some(bs) = bs {
        let width = optional(doc, bs, b"W")?
            .map(|v| width(doc, v))
            .transpose()?
            .unwrap_or(1.);
        let style = match optional(doc, bs, b"S")?
            .map(|v| v.as_name().map_err(|_| bad("invalid border style")))
            .transpose()?
            .unwrap_or(b"S")
        {
            b"S" => FrameStyle::Solid,
            b"D" => FrameStyle::Dashed,
            b"U" => FrameStyle::Underline,
            b"B" => FrameStyle::Beveled,
            b"I" => FrameStyle::Inset,
            _ => return Err(bad("unsupported border style")),
        };
        let dash = optional(doc, bs, b"D")?
            .map(|v| dash(doc, v))
            .transpose()?
            .unwrap_or_else(|| {
                if style == FrameStyle::Dashed {
                    vec![3.]
                } else {
                    vec![]
                }
            });
        (width, style, dash)
    } else if let Some((width, dash)) = legacy {
        (
            width,
            if dash.is_empty() {
                FrameStyle::Solid
            } else {
                FrameStyle::Dashed
            },
            dash,
        )
    } else {
        (1., FrameStyle::Solid, vec![])
    };
    let mk = dictionary(doc, widget, b"MK")?;
    Ok(FrameMetadata {
        width,
        style,
        dash,
        border: palette(doc, mk, b"BC")?,
        background: palette(doc, mk, b"BG")?,
    })
}

#[cfg(test)]
#[path = "form_frame_tests.rs"]
mod tests;
