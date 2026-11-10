//! Selected-widget text settings; native regeneration never owns sibling appearances.
use crate::{AnnotationRect, EngineError, EngineResult, FormFieldValue};
use lopdf::{
    content::{Content, Operation},
    dictionary, Dictionary, Document, Object, ObjectId, Stream,
};
use pdfium_render::prelude::PdfiumLibraryBindings;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextFieldAlignment {
    Left,
    Center,
    Right,
}
impl TextFieldAlignment {
    fn number(self) -> i64 {
        match self {
            Self::Left => 0,
            Self::Center => 1,
            Self::Right => 2,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormTextAppearance {
    pub style_key: String,
    pub font_label: String,
    pub font_size: f32,
    pub color: String,
    pub color_space: String,
    pub alignment: TextFieldAlignment,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextFieldAppearanceChanges {
    pub font_size: Option<f32>,
    pub color: Option<String>,
    pub alignment: Option<TextFieldAlignment>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetFormTextAppearanceRequest {
    pub annotation_index: usize,
    pub expected_bounds: AnnotationRect,
    pub expected_value: FormFieldValue,
    pub expected_style_key: String,
    pub changes: TextFieldAppearanceChanges,
}
pub(crate) fn bad(e: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("Text field appearance: {e}"))
}
fn rgb(s: &str) -> EngineResult<[u8; 3]> {
    if s.len() != 7 || !s.starts_with('#') || !s.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit) {
        return Err(bad("choose a color as #RRGGBB"));
    }
    Ok([
        u8::from_str_radix(&s[1..3], 16).map_err(bad)?,
        u8::from_str_radix(&s[3..5], 16).map_err(bad)?,
        u8::from_str_radix(&s[5..7], 16).map_err(bad)?,
    ])
}
impl SetFormTextAppearanceRequest {
    pub(crate) fn validate(&self) -> EngineResult<()> {
        if self.changes.font_size.is_none()
            && self.changes.color.is_none()
            && self.changes.alignment.is_none()
        {
            return Err(bad("change a text setting before previewing"));
        }
        if self
            .changes
            .font_size
            .is_some_and(|n| !n.is_finite() || (n != 0. && !(1. ..=512.).contains(&n)))
        {
            return Err(bad("font size must be Auto fit or 1–512 pt"));
        }
        if let Some(c) = &self.changes.color {
            rgb(c)?;
        }
        let r = &self.expected_bounds;
        if ![r.x, r.y, r.width, r.height].iter().all(|v| v.is_finite())
            || r.width <= 0.
            || r.height <= 0.
            || self.expected_style_key.len() > 128
        {
            return Err(bad("invalid expected field identity"));
        }
        Ok(())
    }
}
fn resolve<'a>(d: &'a Document, v: &'a Object) -> EngineResult<&'a Object> {
    Ok(d.dereference(v).map_err(bad)?.1)
}
fn dict<'a>(d: &'a Document, v: &'a Object) -> EngineResult<&'a Dictionary> {
    resolve(d, v)?.as_dict().map_err(bad)
}
fn inherited<'a>(
    d: &'a Document,
    mut id: ObjectId,
    key: &[u8],
) -> EngineResult<Option<&'a Object>> {
    let mut seen = HashSet::new();
    for _ in 0..64 {
        if !seen.insert(id) {
            return Err(bad("cyclic field settings"));
        }
        let node = d.get_dictionary(id).map_err(bad)?;
        if let Ok(v) = node.get(key) {
            return Ok(Some(resolve(d, v)?));
        }
        match node.get(b"Parent") {
            Ok(v) => id = v.as_reference().map_err(bad)?,
            Err(_) => return Ok(None),
        }
    }
    Err(bad("field settings exceed inheritance limit"))
}
fn font_in<'a>(
    d: &'a Document,
    resources: Option<&'a Object>,
    name: &[u8],
) -> EngineResult<Option<&'a Object>> {
    let Some(resources) = resources else {
        return Ok(None);
    };
    let resources = dict(d, resources)?;
    let Ok(fonts) = resources.get(b"Font") else {
        return Ok(None);
    };
    let fonts = dict(d, fonts)?;
    Ok(fonts.get(name).ok())
}
struct Context {
    doc: Document,
    id: ObjectId,
    style: FormTextAppearance,
    font: Object,
    name: Vec<u8>,
    content: Content,
    color: Operation,
    rect: [f32; 4],
}
fn numbers(op: &Operation, count: usize) -> EngineResult<Vec<f32>> {
    if op.operands.len() != count {
        return Err(bad("invalid saved appearance instructions"));
    }
    op.operands
        .iter()
        .map(|v| {
            let n = v.as_float().map_err(bad)?;
            if !n.is_finite() {
                return Err(bad("invalid saved appearance numbers"));
            }
            Ok(n)
        })
        .collect()
}
fn close_numbers(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(a, b)| (a - b).abs() <= 0.00001 * a.abs().max(b.abs()).max(1.))
}
fn color_values(op: &Operation) -> EngineResult<Vec<f32>> {
    let count = match op.operator.as_str() {
        "g" | "G" => 1,
        "rg" | "RG" => 3,
        "k" | "K" => 4,
        _ => return Err(bad("unsupported saved color")),
    };
    Ok(numbers(op, count)?
        .into_iter()
        .map(|n| n.clamp(0., 1.))
        .collect())
}
fn equal_color(a: &[f32], b: &[f32]) -> bool {
    match (a.len(), b.len()) {
        (1, 3) => close_numbers(&[a[0]; 3], b),
        (3, 1) => close_numbers(a, &[b[0]; 3]),
        _ => close_numbers(a, b),
    }
}
#[derive(Clone)]
struct SavedPaint {
    fill: Vec<f32>,
    stroke: Vec<f32>,
    width: f32,
    font_size: Option<f32>,
    clipped: bool,
}
// A deliberately small proof: ordinary text plus dictionary-backed solid
// rectangular borders/backgrounds. Unrecognized painting is not discarded.
pub(crate) fn compatible_saved_appearance(
    doc: &Document,
    w: &Dictionary,
    n: &Stream,
    ops: &Content,
    size: f32,
    color: &Operation,
) -> EngineResult<()> {
    let refuse = || {
        bad("custom saved artwork or text settings cannot be preserved by text appearance editing")
    };
    if n.dict.has(b"Group") {
        return Err(refuse());
    }
    let array = |v: &Object| -> EngineResult<Vec<f32>> {
        resolve(doc, v)?
            .as_array()
            .map_err(bad)?
            .iter()
            .map(|v| {
                let x = resolve(doc, v)?.as_float().map_err(bad)?;
                if !x.is_finite() {
                    return Err(refuse());
                }
                Ok(x)
            })
            .collect()
    };
    if let Ok(v) = n.dict.get(b"Matrix") {
        if !close_numbers(&array(v)?, &[1., 0., 0., 1., 0., 0.]) {
            return Err(refuse());
        }
    }
    let rect = array(w.get(b"Rect").map_err(bad)?)?;
    if rect.len() != 4 {
        return Err(refuse());
    }
    let width = rect[2] - rect[0];
    let height = rect[3] - rect[1];
    if !close_numbers(
        &array(n.dict.get(b"BBox").map_err(bad)?)?,
        &[0., 0., width, height],
    ) {
        return Err(refuse());
    }
    let mk = w.get(b"MK").ok().map(|v| dict(doc, v)).transpose()?;
    let expected_bg = mk.and_then(|d| d.get(b"BG").ok()).map(array).transpose()?;
    let expected_bc = mk.and_then(|d| d.get(b"BC").ok()).map(array).transpose()?;
    let bs = w.get(b"BS").ok().map(|v| dict(doc, v)).transpose()?;
    let border_width = bs
        .and_then(|d| d.get(b"W").ok())
        .map(|v| resolve(doc, v)?.as_float().map_err(bad))
        .transpose()?
        .unwrap_or(1.);
    if bs
        .and_then(|d| d.get(b"S").ok())
        .map(|v| resolve(doc, v)?.as_name().map_err(bad))
        .transpose()?
        .is_some_and(|v| v != b"S")
    {
        return Err(refuse());
    }
    let expected = color_values(color)?;
    let mut state = SavedPaint {
        fill: vec![0.],
        stroke: vec![0.],
        width: 1.,
        font_size: None,
        clipped: false,
    };
    let mut stack = Vec::new();
    let mut path: Vec<Vec<f32>> = Vec::new();
    let mut text = false;
    let mut painted_text = false;
    let mut painted_border = false;
    let mut painted_background = false;
    for op in &ops.operations {
        match op.operator.as_str() {
            "q" => {
                numbers(op, 0)?;
                if stack.len() >= 64 {
                    return Err(refuse());
                }
                stack.push(state.clone());
            }
            "Q" => {
                numbers(op, 0)?;
                state = stack.pop().ok_or_else(refuse)?;
            }
            "BT" => {
                numbers(op, 0)?;
                if text {
                    return Err(refuse());
                }
                text = true;
            }
            "ET" => {
                numbers(op, 0)?;
                if !text {
                    return Err(refuse());
                }
                text = false;
            }
            "Tf" => {
                if op.operands.len() != 2 {
                    return Err(refuse());
                }
                let v = op.operands[1].as_float().map_err(bad)?;
                if !v.is_finite() || v <= 0. {
                    return Err(refuse());
                }
                state.font_size = Some(v);
            }
            "g" | "rg" | "k" => state.fill = color_values(op)?,
            "G" | "RG" | "K" => state.stroke = color_values(op)?,
            "Tj" | "TJ" | "'" | "\"" => {
                painted_text = true;
                if !text
                    || !equal_color(&state.fill, &expected)
                    || state.font_size.is_none()
                    || size != 0. && !close_numbers(&[state.font_size.unwrap()], &[size])
                {
                    return Err(refuse());
                }
                if op.operator == "\""
                    && (op.operands.len() != 3
                        || op.operands[..2]
                            .iter()
                            .any(|v| v.as_float().ok() != Some(0.)))
                {
                    return Err(refuse());
                }
            }
            "Tm" => {
                let v = numbers(op, 6)?;
                if !close_numbers(&v[..4], &[1., 0., 0., 1.]) {
                    return Err(refuse());
                }
            }
            "cm" => {
                if !close_numbers(&numbers(op, 6)?, &[1., 0., 0., 1., 0., 0.]) {
                    return Err(refuse());
                }
            }
            "Tc" | "Tw" | "Ts" | "Tr" => {
                if numbers(op, 1)?[0] != 0. {
                    return Err(refuse());
                }
            }
            "Tz" => {
                if numbers(op, 1)?[0] != 100. {
                    return Err(refuse());
                }
            }
            "Td" | "TD" => {
                numbers(op, 2)?;
            }
            "TL" => {
                numbers(op, 1)?;
            }
            "T*" => {
                numbers(op, 0)?;
            }
            "w" => {
                state.width = numbers(op, 1)?[0];
                if state.width <= 0. {
                    return Err(refuse());
                }
            }
            "J" | "j" => {
                if numbers(op, 1)?[0] != 0. {
                    return Err(refuse());
                }
            }
            "M" => {
                if numbers(op, 1)?[0] < 1. {
                    return Err(refuse());
                }
            }
            "d" => {
                if op.operands.len() != 2
                    || !resolve(doc, &op.operands[0])?
                        .as_array()
                        .map_err(bad)?
                        .is_empty()
                    || op.operands[1].as_float().map_err(bad)? != 0.
                {
                    return Err(refuse());
                }
            }
            "re" => {
                if path.len() >= 2 {
                    return Err(refuse());
                }
                path.push(numbers(op, 4)?);
            }
            "W" | "W*" => {
                numbers(op, 0)?;
                if path.len() != 1 {
                    return Err(refuse());
                }
                let p = &path[0];
                let full = close_numbers(p, &[0., 0., width, height]);
                let inset = close_numbers(
                    p,
                    &[
                        border_width,
                        border_width,
                        width - 2. * border_width,
                        height - 2. * border_width,
                    ],
                );
                if !full && !inset {
                    return Err(refuse());
                }
                state.clipped |= !full;
            }
            "n" => {
                numbers(op, 0)?;
                path.clear();
            }
            "S" => {
                numbers(op, 0)?;
                if painted_text || painted_border || state.clipped || path.len() != 1 {
                    return Err(refuse());
                }
                let p = path.pop().unwrap();
                if !expected_bc
                    .as_ref()
                    .is_some_and(|v| equal_color(v, &state.stroke))
                    || !close_numbers(&[state.width], &[border_width])
                    || !close_numbers(
                        &p,
                        &[
                            border_width / 2.,
                            border_width / 2.,
                            width - border_width,
                            height - border_width,
                        ],
                    )
                {
                    return Err(refuse());
                }
                painted_border = true;
            }
            "f" | "F" | "f*" => {
                numbers(op, 0)?;
                if painted_text || state.clipped {
                    return Err(refuse());
                }
                let background = path.len() == 1
                    && !painted_background
                    && !painted_border
                    && expected_bg
                        .as_ref()
                        .is_some_and(|v| equal_color(v, &state.fill))
                    && close_numbers(&path[0], &[0., 0., width, height]);
                let border = op.operator == "f*"
                    && path.len() == 2
                    && !painted_border
                    && expected_bc
                        .as_ref()
                        .is_some_and(|v| equal_color(v, &state.fill))
                    && border_width > 0.
                    && close_numbers(&path[0], &[0., 0., width, height])
                    && close_numbers(
                        &path[1],
                        &[
                            border_width,
                            border_width,
                            width - 2. * border_width,
                            height - 2. * border_width,
                        ],
                    );
                if !background && !border {
                    return Err(refuse());
                }
                painted_background |= background;
                painted_border |= border;
                path.clear();
            }
            "BMC" => {
                if op.operands.len() != 1 || op.operands[0].as_name().map_err(bad)? != b"Tx" {
                    return Err(refuse());
                }
            }
            "EMC" => {
                numbers(op, 0)?;
            }
            _ => return Err(refuse()),
        }
    }
    if text || !stack.is_empty() || !path.is_empty() {
        return Err(refuse());
    }
    Ok(())
}
fn load(bytes: &[u8], page: usize, index: usize) -> EngineResult<Context> {
    let (doc, _) = crate::acroforms::authoring_document(bytes)?;
    let pages = doc.get_pages();
    let page_id = *pages
        .values()
        .nth(page)
        .ok_or_else(|| bad("page outside source"))?;
    let annots = resolve(
        &doc,
        doc.get_dictionary(page_id)
            .map_err(bad)?
            .get(b"Annots")
            .map_err(bad)?,
    )?
    .as_array()
    .map_err(bad)?;
    let id = annots
        .get(index)
        .ok_or_else(|| bad("field no longer exists"))?
        .as_reference()
        .map_err(bad)?;
    let w = doc.get_dictionary(id).map_err(bad)?;
    if resolve(&doc, w.get(b"Subtype").map_err(bad)?)?
        .as_name()
        .map_err(bad)?
        != b"Widget"
        || inherited(&doc, id, b"FT")?.and_then(|v| v.as_name().ok()) != Some(b"Tx")
    {
        return Err(bad("select a text field"));
    }
    let flags = w
        .get(b"F")
        .ok()
        .map(|v| resolve(&doc, v)?.as_i64().map_err(bad))
        .transpose()?
        .unwrap_or(0);
    let ff = inherited(&doc, id, b"Ff")?
        .map(|v| v.as_i64().map_err(bad))
        .transpose()?
        .unwrap_or(0);
    if flags & (1 | 2 | 8 | 16 | 32 | 128 | 512) != 0
        || ff & ((1 << 13) | (1 << 20) | (1 << 25)) != 0
    {
        return Err(bad(
            "hidden, locked, password or rich-text fields are not supported",
        ));
    }
    let mut placements = 0;
    for p in pages.values() {
        if let Ok(a) = doc.get_dictionary(*p).map_err(bad)?.get(b"Annots") {
            for a in resolve(&doc, a)?.as_array().map_err(bad)? {
                if a.as_reference().ok() == Some(id) {
                    placements += 1;
                }
            }
        }
    }
    if placements != 1 {
        return Err(bad("one widget is reused in several locations"));
    }
    if let Ok(p) = w.get(b"P") {
        if p.as_reference().map_err(bad)? != page_id {
            return Err(bad("field page ownership is inconsistent"));
        }
    }
    let form = dict(
        &doc,
        doc.catalog().map_err(bad)?.get(b"AcroForm").map_err(bad)?,
    )?;
    let da = inherited(&doc, id, b"DA")?
        .or(form.get(b"DA").ok().map(|v| resolve(&doc, v)).transpose()?)
        .ok_or_else(|| bad("missing default font settings"))?;
    let da = da.as_str().map_err(bad)?;
    if da.len() > 64 * 1024 {
        return Err(bad("default appearance exceeds 64 KiB"));
    }
    let content = Content::decode_strict(da).map_err(bad)?;
    let mut selected_font = None;
    let mut color = Operation::new("g", vec![0.into()]);
    for op in &content.operations {
        if op.operator == "Tf" {
            if selected_font.is_some() || op.operands.len() != 2 {
                return Err(bad("requires one explicit default font"));
            }
            selected_font = Some(op);
            continue;
        }
        let count = match op.operator.as_str() {
            "g" => 1,
            "rg" => 3,
            "k" => 4,
            _ => return Err(bad("unsupported field text settings")),
        };
        if op.operands.len() != count {
            return Err(bad("invalid text color"));
        }
        let mut operands = Vec::with_capacity(count);
        for v in &op.operands {
            let n = v.as_float().map_err(bad)?;
            if !n.is_finite() {
                return Err(bad("invalid text color"));
            }
            operands.push(Object::Real(n.clamp(0., 1.)));
        }
        color = Operation::new(&op.operator, operands);
    }
    let tf = selected_font.ok_or_else(|| bad("missing default font"))?;
    let name = tf.operands[0].as_name().map_err(bad)?;
    let size = tf.operands[1].as_float().map_err(bad)?;
    if name.is_empty() || !size.is_finite() || size < 0. {
        return Err(bad("invalid default font"));
    }
    let local = font_in(&doc, inherited(&doc, id, b"DR")?, name)?;
    let global = font_in(&doc, form.get(b"DR").ok(), name)?;
    let page_font = if local.is_none() && global.is_none() && w.has(b"P") {
        font_in(&doc, inherited(&doc, page_id, b"Resources")?, name)?
    } else {
        None
    };
    let font = local
        .or(global)
        .or(page_font)
        .ok_or_else(|| bad("the original field font cannot be resolved"))?;
    let fd = dict(&doc, font)?;
    if fd
        .get(b"Type")
        .ok()
        .map(|v| resolve(&doc, v))
        .transpose()?
        .and_then(|v| v.as_name().ok())
        != Some(b"Font")
        || !matches!(
            resolve(&doc, fd.get(b"Subtype").map_err(bad)?)?
                .as_name()
                .map_err(bad)?,
            b"Type0" | b"Type1" | b"TrueType"
        )
    {
        return Err(bad("unsupported original field font"));
    }
    if let Ok(ap) = w.get(b"AP") {
        let ap = dict(&doc, ap)?;
        if let Ok(n) = ap.get(b"N") {
            let n = resolve(&doc, n)?.as_stream().map_err(bad)?;
            let data = n.get_plain_content_with_limit(1024 * 1024).map_err(bad)?;
            let ops = Content::decode_strict(&data).map_err(bad)?;
            if ops.operations.len() > 8192 {
                return Err(bad("saved appearance has too many instructions"));
            }
            compatible_saved_appearance(&doc, w, n, &ops, size, &color)?;
            let resources = n.dict.get(b"Resources").ok();
            let mut bound = false;
            let mut paint = false;
            let mut proof = crate::form_appearance::Equivalence::default();
            for op in &ops.operations {
                match op.operator.as_str() {
                    "Do" => return Err(bad("custom nested field artwork cannot yet be restyled")),
                    "Tf" => {
                        if op.operands.len() != 2 {
                            return Err(bad("invalid saved font settings"));
                        }
                        let selected =
                            font_in(&doc, resources, op.operands[0].as_name().map_err(bad)?)?
                                .ok_or_else(|| bad("saved font is not bound"))?;
                        if !proof.check(&doc, font, &doc, selected)? {
                            return Err(bad("saved artwork and edit settings use different fonts"));
                        }
                        bound = true;
                    }
                    "Tj" | "TJ" | "'" | "\"" => {
                        if !bound {
                            return Err(bad("saved text has no verified font"));
                        }
                        paint = true;
                    }
                    _ => {}
                }
            }
            if !paint
                && inherited(&doc, id, b"V")?
                    .and_then(|v| v.as_str().ok())
                    .is_some_and(|v| !v.is_empty())
            {
                return Err(bad("saved artwork does not contain verifiable text"));
            }
        }
    }
    let q = inherited(&doc, id, b"Q")?
        .or(form.get(b"Q").ok().map(|v| resolve(&doc, v)).transpose()?)
        .map(|v| v.as_i64().map_err(bad))
        .transpose()?
        .unwrap_or(0);
    let alignment = match q {
        0 => TextFieldAlignment::Left,
        1 => TextFieldAlignment::Center,
        2 => TextFieldAlignment::Right,
        _ => return Err(bad("invalid text alignment")),
    };
    let c: Vec<_> = color
        .operands
        .iter()
        .map(|v| v.as_float().unwrap())
        .collect();
    let (space, channels) = match color.operator.as_str() {
        "g" => ("gray", [c[0]; 3]),
        "rg" => ("rgb", [c[0], c[1], c[2]]),
        _ => (
            "cmyk",
            [
                1. - (c[0] + c[3]).min(1.),
                1. - (c[1] + c[3]).min(1.),
                1. - (c[2] + c[3]).min(1.),
            ],
        ),
    };
    let color_text = format!(
        "#{:02x}{:02x}{:02x}",
        (channels[0] * 255.).round() as u8,
        (channels[1] * 255.).round() as u8,
        (channels[2] * 255.).round() as u8
    );
    let label=fd.get(b"BaseFont").ok().map(|v|resolve(&doc,v)).transpose()?.and_then(|v|v.as_name().ok()).filter(|v|v.len()<=1024).map(|v|String::from_utf8_lossy(v).to_string()).filter(|v|!v.chars().any(|c|c.is_control()||matches!(c as u32,0x061c|0x200b..=0x200f|0x202a..=0x202e|0x2060..=0x2069|0xfeff))).unwrap_or_else(||"Original PDF font".into());
    let label = if label.len() > 7
        && label.as_bytes()[6] == b'+'
        && label.as_bytes()[..6].iter().all(u8::is_ascii_uppercase)
    {
        label[7..].to_owned()
    } else {
        label
    };
    let r = resolve(&doc, w.get(b"Rect").map_err(bad)?)?
        .as_array()
        .map_err(bad)?;
    if r.len() != 4 {
        return Err(bad("invalid field rectangle"));
    }
    let mut rect = [0.; 4];
    for (i, v) in r.iter().enumerate() {
        rect[i] = v.as_float().map_err(bad)?;
    }
    if rect.iter().any(|v| !v.is_finite()) || rect[2] <= rect[0] || rect[3] <= rect[1] {
        return Err(bad("invalid field rectangle"));
    }
    let mut budget = crate::form_graph::CopyBudget::default();
    budget.array(std::slice::from_ref(font))?;
    let font = font.clone();
    let name = name.to_vec();
    let style = FormTextAppearance {
        style_key: format!(
            "{}-{page}-{index}",
            Sha256::digest(bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        ),
        font_label: label,
        font_size: size,
        color: color_text,
        color_space: space.into(),
        alignment,
    };
    Ok(Context {
        doc,
        id,
        style,
        font,
        name,
        content,
        color,
        rect,
    })
}
pub(crate) fn inspect(
    pdfium: &pdfium_render::prelude::Pdfium,
    bytes: &[u8],
    page: usize,
    index: usize,
) -> EngineResult<FormTextAppearance> {
    let ctx = load(bytes, page, index)?;
    let value = match inherited(&ctx.doc, ctx.id, b"V")? {
        None | Some(Object::Null) => String::new(),
        Some(v @ Object::String(raw, _)) => {
            if raw.len() > crate::form_appearance_value::TEXT_LIMIT {
                return Err(bad("field value exceeds the text verification limit"));
            }
            let value = if let Some(utf8) = raw.strip_prefix(b"\xef\xbb\xbf") {
                std::str::from_utf8(utf8).map_err(bad)?.to_owned()
            } else {
                if raw.starts_with(b"\xfe\xff") && raw.len() % 2 != 0 {
                    return Err(bad("field value has incomplete UTF-16 text"));
                }
                lopdf::decode_text_string(v).map_err(bad)?
            };
            if value.len() > crate::form_appearance_value::TEXT_LIMIT {
                return Err(bad("field value exceeds the text verification limit"));
            }
            value
        }
        _ => return Err(bad("field value is not a PDF text string")),
    };
    crate::form_appearance_value::verify(
        pdfium,
        &ctx.doc,
        ctx.doc.get_dictionary(ctx.id).map_err(bad)?,
        &value,
    )?;
    Ok(ctx.style)
}
pub(crate) fn change(
    bindings: &dyn PdfiumLibraryBindings,
    bytes: &[u8],
    page: usize,
    index: usize,
    value: &FormFieldValue,
    changes: &TextFieldAppearanceChanges,
) -> EngineResult<Vec<u8>> {
    let mut ctx = load(bytes, page, index)?;
    let w = ctx.doc.get_dictionary(ctx.id).map_err(bad)?;
    let mut budget = crate::form_graph::CopyBudget::default();
    budget.dictionary(w)?;
    let original_da = w
        .get(b"DA")
        .ok()
        .map(|v| resolve(&ctx.doc, v))
        .transpose()?
        .cloned();
    for op in &mut ctx.content.operations {
        if op.operator == "Tf" {
            if let Some(size) = changes.font_size {
                op.operands[1] = Object::Real(size);
            }
        }
    }
    if let Some(color) = &changes.color {
        let c = rgb(color)?;
        ctx.color = Operation::new(
            "rg",
            c.iter().map(|n| Object::Real(*n as f32 / 255.)).collect(),
        );
        ctx.content
            .operations
            .retain(|op| !matches!(op.operator.as_str(), "g" | "rg" | "k"));
        ctx.content.operations.push(ctx.color.clone());
    }
    let desired_da = if changes.font_size.is_some() || changes.color.is_some() {
        Some(Object::string_literal(ctx.content.encode().map_err(bad)?))
    } else {
        original_da
    };
    let tf = ctx
        .content
        .operations
        .iter()
        .find(|op| op.operator == "Tf")
        .unwrap()
        .clone();
    let probe_da = Object::string_literal(
        Content {
            operations: vec![tf, ctx.color],
        }
        .encode()
        .map_err(bad)?,
    );
    // A private font-bound placeholder lets the existing isolated regeneration
    // prove the native result without borrowing or modifying old shared AP streams.
    let stub=ctx.doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),(ctx.rect[2]-ctx.rect[0]).into(),(ctx.rect[3]-ctx.rect[1]).into()],"Resources"=>dictionary!{"Font"=>{let mut f=Dictionary::new();f.set(ctx.name,ctx.font);f}}},Vec::new()));
    let original_ap = ctx
        .doc
        .get_dictionary(ctx.id)
        .map_err(bad)?
        .get(b"AP")
        .ok()
        .map(|v| dict(&ctx.doc, v))
        .transpose()?;
    if let Some(ap) = original_ap {
        budget.dictionary(ap)?;
    }
    let mut ap = original_ap.cloned().unwrap_or_default();
    ap.set("N", stub);
    let w = ctx.doc.get_dictionary_mut(ctx.id).map_err(bad)?;
    w.set("AP", ap);
    w.set("DA", probe_da);
    if let Some(q) = changes.alignment {
        w.set("Q", q.number());
    }
    let probe = crate::pdf_output::save(&mut ctx.doc)?;
    let generated = crate::form_reflow::change(bindings, &probe, page, index, value)?;
    let (mut final_doc, _) = crate::acroforms::authoring_document(&generated)?;
    // IDs survive serialization, and no source dictionary is modified after publication.
    let w = final_doc.get_dictionary_mut(ctx.id).map_err(bad)?;
    match desired_da {
        Some(da) => {
            w.set("DA", da);
        }
        None => {
            w.remove(b"DA");
        }
    }
    crate::page_resources::prune_unreachable(&mut final_doc)?;
    crate::pdf_output::save(&mut final_doc)
}
