//! Bounded selected-widget appearance ownership and publication.
use super::*;
use crate::form_frame_paint::{FramePaint, PaintStyle};
use crate::form_frame_render::{self, FramePaintTarget};
use crate::form_graph::CopyBudget;
use lopdf::{ObjectId, Stream};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Arc};

const BUDGET: usize = 64 * 1024 * 1024;
struct Original {
    stream: Stream,
    paint: FramePaint,
    scale: f64,
    key: usize,
}
struct State {
    mode: Vec<u8>,
    name: Option<Vec<u8>>,
    original: Arc<Original>,
}
struct Context {
    doc: Document,
    id: ObjectId,
    states: Vec<State>,
    main: usize,
    meta: FrameMetadata,
    radio: bool,
    button: bool,
    key: String,
    budget: CopyBudget,
    default_colors: bool,
}
fn err<T>(_: T) -> EngineError {
    bad("invalid field appearance structure")
}
fn reference<'a>(doc: &'a Document, mut value: &'a Object) -> EngineResult<ObjectId> {
    for _ in 0..64 {
        let id = value.as_reference().map_err(err)?;
        value = doc
            .objects
            .get(&id)
            .ok_or_else(|| bad("missing field reference"))?;
        if !matches!(value, Object::Reference(_)) {
            return Ok(id);
        }
    }
    Err(bad("cyclic field reference"))
}
fn inherited<'a>(
    doc: &'a Document,
    mut id: ObjectId,
    key: &[u8],
) -> EngineResult<Option<&'a Object>> {
    for _ in 0..64 {
        let node = doc.get_dictionary(id).map_err(err)?;
        if let Some(value) = optional(doc, node, key)? {
            return Ok(Some(value));
        }
        let Some(parent) = optional(doc, node, b"Parent")? else {
            return Ok(None);
        };
        id = reference(doc, node.get(b"Parent").map_err(err)?)?;
        if parent.as_dict().is_err() {
            return Err(bad("invalid field parent"));
        }
    }
    Err(bad("field inheritance exceeds 64 levels"))
}
fn array<const N: usize>(doc: &Document, value: &Object) -> EngineResult<[f64; N]> {
    let values = resolve(doc, value)?.as_array().map_err(err)?;
    if values.len() != N {
        return Err(bad("invalid appearance geometry"));
    }
    let mut out = [0.; N];
    for (n, v) in out.iter_mut().zip(values) {
        *n = number(doc, v)?;
    }
    Ok(out)
}
fn geometry(doc: &Document, stream: &Stream, rect: [f64; 4]) -> EngineResult<([f64; 4], f64)> {
    for (key, expected) in [
        (b"Type".as_slice(), b"XObject".as_slice()),
        (b"Subtype", b"Form"),
    ] {
        if let Some(value) = optional(doc, &stream.dict, key)? {
            if value.as_name().map_err(err)? != expected {
                return Err(bad("field appearance must be a Form XObject"));
            }
        }
    }
    let bbox = array::<4>(doc, stream.dict.get(b"BBox").map_err(err)?)?;
    if bbox[0] >= bbox[2] || bbox[1] >= bbox[3] {
        return Err(bad("invalid appearance bounding box"));
    }
    let m = optional(doc, &stream.dict, b"Matrix")?
        .map(|v| array::<6>(doc, v))
        .transpose()?
        .unwrap_or([1., 0., 0., 1., 0., 0.]);
    let diagonal = m[1] == 0. && m[2] == 0. && m[0] != 0. && m[3] != 0.;
    let quarter = m[0] == 0. && m[3] == 0. && m[1] != 0. && m[2] != 0.;
    if !diagonal && !quarter {
        return Err(bad("skewed frame appearances are not supported"));
    }
    let w = bbox[2] - bbox[0];
    let h = bbox[3] - bbox[1];
    let (sx, sy) = if diagonal {
        ((rect[2] - rect[0]) / w, (rect[3] - rect[1]) / h)
    } else {
        ((rect[3] - rect[1]) / w, (rect[2] - rect[0]) / h)
    };
    if !sx.is_finite()
        || !sy.is_finite()
        || sx <= 0.
        || sx > 1_000_000.
        || (sx - sy).abs() > sx.abs().max(sy.abs()) * 1e-6
    {
        return Err(bad("frame appearance uses nonuniform or excessive scaling"));
    }
    Ok((bbox, sx))
}
fn default_colors(doc: &Document, resources: Option<&Object>) -> EngineResult<bool> {
    let Some(r) = resources else {
        return Ok(false);
    };
    let r = resolve(doc, r)?.as_dict().map_err(err)?;
    let Some(c) = dictionary(doc, r, b"ColorSpace")? else {
        return Ok(false);
    };
    Ok([b"DefaultRGB".as_slice(), b"DefaultGray", b"DefaultCMYK"]
        .iter()
        .any(|k| c.has(k)))
}
fn load(bytes: &[u8], page_index: usize, index: usize) -> EngineResult<Context> {
    let (doc, _) = crate::acroforms::authoring_document(bytes)?;
    let pages = doc.get_pages();
    let page = *pages
        .values()
        .nth(page_index)
        .ok_or_else(|| bad("page outside source"))?;
    let annots = resolve(
        &doc,
        doc.get_dictionary(page)
            .map_err(err)?
            .get(b"Annots")
            .map_err(err)?,
    )?
    .as_array()
    .map_err(err)?;
    let id = reference(
        &doc,
        annots
            .get(index)
            .ok_or_else(|| bad("field no longer exists"))?,
    )?;
    let widget = doc.get_dictionary(id).map_err(err)?;
    if optional(&doc, widget, b"Subtype")?.and_then(|v| v.as_name().ok()) != Some(b"Widget") {
        return Err(bad("select a form field"));
    }
    let kind = inherited(&doc, id, b"FT")?
        .and_then(|v| v.as_name().ok())
        .ok_or_else(|| bad("missing field type"))?;
    if !matches!(kind, b"Tx" | b"Ch" | b"Btn") {
        return Err(bad("unsupported field type"));
    }
    let button = kind == b"Btn";
    let flags = optional(&doc, widget, b"F")?
        .map(|v| v.as_i64().map_err(err))
        .transpose()?
        .unwrap_or(0);
    let field_flags = inherited(&doc, id, b"Ff")?
        .map(|v| v.as_i64().map_err(err))
        .transpose()?
        .unwrap_or(0);
    if flags & (1 | 2 | 8 | 16 | 32 | 128 | 512) != 0 || (button && field_flags & (1 << 16) != 0) {
        return Err(bad(
            "hidden, fixed-view, locked or pushbutton fields cannot be restyled",
        ));
    }
    let radio = button && field_flags & (1 << 15) != 0;
    if let Ok(p) = widget.get(b"P") {
        if reference(&doc, p)? != page {
            return Err(bad("field page ownership is inconsistent"));
        }
    }
    let mut placements = 0usize;
    for page in pages.values() {
        if let Some(a) = optional(&doc, doc.get_dictionary(*page).map_err(err)?, b"Annots")? {
            for value in a.as_array().map_err(err)? {
                if reference(&doc, value)? == id {
                    placements += 1;
                }
            }
        }
    }
    if placements != 1 {
        return Err(bad("one widget is reused in multiple locations"));
    }
    let form = dictionary(&doc, doc.catalog().map_err(err)?, b"AcroForm")?
        .ok_or_else(|| bad("missing form dictionary"))?;
    if optional(&doc, form, b"NeedAppearances")?
        .map(|v| v.as_bool().map_err(err))
        .transpose()?
        .unwrap_or(false)
    {
        return Err(bad(
            "save generated form appearances before changing this frame",
        ));
    }
    let meta = metadata(&doc, widget)?;
    let rect = array::<4>(&doc, widget.get(b"Rect").map_err(err)?)?;
    if rect[0] >= rect[2] || rect[1] >= rect[3] {
        return Err(bad("invalid field rectangle"));
    }
    let ap = dictionary(&doc, widget, b"AP")?
        .ok_or_else(|| bad("this field has no saved appearance to preserve"))?;
    if !ap.has(b"N") {
        return Err(bad("missing normal field appearance"));
    }
    let mut budget = CopyBudget::with_byte_limit(BUDGET);
    let mut admission = crate::form_answer_appearance::NormalAppearanceAdmission::new(&doc);
    let mut memo: HashMap<usize, Arc<Original>> = HashMap::new();
    let mut states = Vec::new();
    let mut defaults = default_colors(&doc, inherited(&doc, page, b"Resources")?)?;
    for mode in [b"N".as_slice(), b"R", b"D"] {
        let Some(value) = optional(&doc, ap, mode)? else {
            continue;
        };
        admission.admit(value)?;
        let rows: Vec<(Option<&Vec<u8>>, &Object)> = match value {
            Object::Stream(_) => vec![(None, value)],
            Object::Dictionary(s) if !s.is_empty() && s.len() <= 4000 => {
                s.iter().map(|(name, v)| (Some(name), v)).collect()
            }
            _ => return Err(bad("invalid appearance states")),
        };
        for (name, value) in rows {
            if states.len() >= 4000 {
                return Err(bad("too many appearance states"));
            }
            budget.reserve_bytes(256 + name.map_or(0, |n| n.len()))?;
            let s = resolve(&doc, value)?.as_stream().map_err(err)?;
            let key = s as *const Stream as usize;
            let original = if let Some(original) = memo.get(&key) {
                Arc::clone(original)
            } else {
                let (bbox, scale) = geometry(&doc, s, rect)?;
                defaults |= default_colors(&doc, s.dict.get(b"Resources").ok())?;
                let bytes = s
                    .get_plain_content_with_limit(
                        (4 * 1024 * 1024).min(BUDGET - budget.bytes_used()),
                    )
                    .map_err(|_| bad("appearance exceeds decoded budget"))?;
                budget.reserve_bytes(crate::form_answer_appearance::preflight_program(&bytes)?)?;
                budget.stream(s)?;
                let allow_absent = meta.width == 0.
                    || matches!(
                        meta.border,
                        FramePalette::Missing | FramePalette::Transparent
                    );
                let paint = FramePaint::inspect_with_absence(&bytes, bbox, radio, allow_absent)?;
                let original = Arc::new(Original {
                    stream: s.clone(),
                    paint,
                    scale,
                    key,
                });
                memo.insert(key, Arc::clone(&original));
                original
            };
            states.push(State {
                mode: mode.to_vec(),
                name: name.cloned(),
                original,
            });
        }
    }
    let selected = optional(&doc, widget, b"AS")?
        .map(|v| v.as_name().map_err(err))
        .transpose()?;
    let main = states
        .iter()
        .position(|s| s.mode == b"N" && (s.name.is_none() || s.name.as_deref() == selected))
        .ok_or_else(|| bad("current normal appearance state is missing"))?;
    let mut hash = Sha256::new();
    hash.update(bytes);
    hash.update(page_index.to_le_bytes());
    hash.update(index.to_le_bytes());
    let key = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    Ok(Context {
        doc,
        id,
        states,
        main,
        meta,
        radio,
        button,
        key,
        budget,
        default_colors: defaults,
    })
}
fn style(p: &FramePaint, m: &FrameMetadata) -> Option<FrameStyle> {
    match p.style {
        PaintStyle::Solid => Some(FrameStyle::Solid),
        PaintStyle::Dashed(..) => Some(FrameStyle::Dashed),
        PaintStyle::Underline => Some(FrameStyle::Underline),
        PaintStyle::Invisible => Some(m.style),
        PaintStyle::Compound => {
            matches!(m.style, FrameStyle::Beveled | FrameStyle::Inset).then_some(m.style)
        }
    }
}
fn effective_border(p: &FramePaint, m: &FrameMetadata) -> Option<PaintColor> {
    if p.style == PaintStyle::Invisible {
        match &m.border {
            FramePalette::Color(c) => Some(c.clone()),
            _ => None,
        }
    } else {
        Some(p.color.clone())
    }
}
fn raw_width(s: &State, m: &FrameMetadata) -> f64 {
    if s.original.paint.style == PaintStyle::Invisible {
        m.width
    } else {
        s.original.paint.width * s.original.scale
    }
}
fn rgb_values(c: &PaintColor) -> [f64; 3] {
    match c {
        PaintColor::Gray(v) => [*v; 3],
        PaintColor::Rgb(v) => *v,
        PaintColor::Cmyk([c, m, y, k]) => [c, m, y].map(|v| 1. - (v + k).min(1.)),
    }
}
fn info_color(color: Option<&PaintColor>) -> FormFrameColor {
    let Some(c) = color else {
        return FormFrameColor {
            color_space: "transparent".into(),
            components: vec![],
            preview: None,
        };
    };
    let (space, components) = match c {
        PaintColor::Gray(n) => ("gray", vec![*n as f32]),
        PaintColor::Rgb(n) => ("rgb", n.iter().map(|n| *n as f32).collect()),
        PaintColor::Cmyk(n) => ("cmyk", n.iter().map(|n| *n as f32).collect()),
    };
    let rgb = rgb_values(c).map(|v| (v.clamp(0., 1.) * 255.).round() as u8);
    FormFrameColor {
        color_space: space.into(),
        components,
        preview: Some(format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])),
    }
}
pub(crate) fn inspect(
    bytes: &[u8],
    page: usize,
    index: usize,
) -> EngineResult<FormFrameAppearance> {
    let c = load(bytes, page, index)?;
    let s = &c.states[c.main];
    let p = &s.original.paint;
    Ok(FormFrameAppearance {
        style_key: c.key,
        width: raw_width(s, &c.meta) as f32,
        style: style(p, &c.meta),
        border: info_color(effective_border(p, &c.meta).as_ref()),
        background: info_color(p.background_color.as_ref()),
        supported_styles: if c.radio {
            vec![FrameStyle::Solid, FrameStyle::Dashed]
        } else {
            vec![
                FrameStyle::Solid,
                FrameStyle::Dashed,
                FrameStyle::Underline,
                FrameStyle::Beveled,
                FrameStyle::Inset,
            ]
        },
    })
}
fn requested_color(
    change: &Option<FrameColorChange>,
    original: Option<PaintColor>,
) -> EngineResult<Option<PaintColor>> {
    match change {
        None => Ok(original),
        Some(FrameColorChange::Transparent) => Ok(None),
        Some(FrameColorChange::Rgb(s)) => Ok(Some(rgb(s)?)),
    }
}
fn same_color(a: Option<&PaintColor>, b: Option<&PaintColor>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => rgb_values(a)
            .iter()
            .zip(rgb_values(b))
            .all(|(a, b)| (a - b).abs() < 0.0001),
        _ => false,
    }
}
fn darken(c: Option<&PaintColor>) -> PaintColor {
    let rgb = c.map(rgb_values).unwrap_or([1.; 3]);
    PaintColor::Rgb(rgb.map(|n| (n - 0.25).max(0.)))
}
fn standard_shading(
    style: FrameStyle,
    pressed: bool,
    background: Option<&PaintColor>,
) -> [PaintColor; 2] {
    if style == FrameStyle::Inset {
        if pressed {
            [PaintColor::Gray(0.), PaintColor::Gray(1.)]
        } else {
            [PaintColor::Gray(0.5), PaintColor::Gray(0.75)]
        }
    } else {
        let dark = PaintColor::Rgb(
            background
                .map(rgb_values)
                .unwrap_or([1.; 3])
                .map(|n| n / 2.),
        );
        if pressed {
            [dark, PaintColor::Gray(1.)]
        } else {
            [PaintColor::Gray(1.), dark]
        }
    }
}
fn targets(c: &Context, changes: &FormFrameChanges) -> EngineResult<Vec<FramePaintTarget>> {
    if changes.width.is_some_and(|n| !n.is_finite() || n < 0.) {
        return Err(bad("invalid raw field width"));
    }
    if c.default_colors
        && (changes.border.is_some() || changes.background.is_some() || changes.style.is_some())
    {
        return Err(bad(
            "calibrated default colors require a separate color proof",
        ));
    }
    let baseline = &c.states[c.main].original.paint;
    let wanted_bg = requested_color(&changes.background, baseline.background_color.clone())?;
    let mut targets = Vec::new();
    for s in &c.states {
        let p = &s.original.paint;
        let old_style = style(p, &c.meta);
        let wanted_style = changes.style.or(old_style).unwrap_or(FrameStyle::Beveled);
        let mut border = requested_color(&changes.border, effective_border(p, &c.meta))?;
        let mut background = requested_color(&changes.background, p.background_color.clone())?;
        if matches!(changes.border, Some(FrameColorChange::Rgb(_)))
            && !same_color(
                effective_border(p, &c.meta).as_ref(),
                effective_border(baseline, &c.meta).as_ref(),
            )
        {
            return Err(bad("state border palettes cannot be reconciled"));
        }
        if matches!(changes.background, Some(FrameColorChange::Rgb(_)))
            && !same_color(
                p.background_color.as_ref(),
                baseline.background_color.as_ref(),
            )
        {
            if s.mode == b"D"
                && c.button
                && same_color(
                    p.background_color.as_ref(),
                    Some(&darken(baseline.background_color.as_ref())),
                )
            {
                background = Some(darken(wanted_bg.as_ref()));
            } else {
                return Err(bad("state background palettes cannot be reconciled"));
            }
        }
        if p.style == PaintStyle::Invisible
            && matches!(
                c.meta.border,
                FramePalette::Transparent | FramePalette::Missing
            )
            && changes.border.is_none()
        {
            border = None;
        }
        let shading = if matches!(wanted_style, FrameStyle::Beveled | FrameStyle::Inset) {
            if (changes.style.is_none() || old_style == Some(wanted_style))
                && p.edge_colors.is_some()
            {
                let original = p.edge_colors.as_ref().unwrap();
                let canonical = standard_shading(
                    wanted_style,
                    s.mode == b"D" && c.button,
                    baseline.background_color.as_ref(),
                );
                if changes.background.is_some()
                    && old_style == Some(wanted_style)
                    && original
                        .iter()
                        .zip(canonical.iter())
                        .all(|(a, b)| same_color(Some(a), Some(b)))
                {
                    Some(standard_shading(
                        wanted_style,
                        s.mode == b"D" && c.button,
                        wanted_bg.as_ref(),
                    ))
                } else {
                    Some(original.clone())
                }
            } else {
                Some(standard_shading(
                    wanted_style,
                    s.mode == b"D" && c.button,
                    wanted_bg.as_ref(),
                ))
            }
        } else {
            None
        };
        let (dash, dash_phase) = match &p.style {
            PaintStyle::Dashed(d, phase) => (d.clone(), *phase),
            PaintStyle::Invisible if wanted_style == FrameStyle::Dashed => match &p.stroke_style {
                PaintStyle::Dashed(d, phase) => (d.clone(), *phase),
                _ => (
                    c.meta.dash.iter().map(|n| n / s.original.scale).collect(),
                    0.,
                ),
            },
            _ => (vec![3., 2.], 0.),
        };
        targets.push(FramePaintTarget {
            width: changes
                .width
                .map(f64::from)
                .unwrap_or(raw_width(s, &c.meta))
                / s.original.scale,
            style: wanted_style,
            dash,
            dash_phase,
            border,
            background,
            shading,
        });
    }
    Ok(targets)
}
// Explicitly re-entered visible settings leave state-specific paints and even
// conflicting defaults untouched. Only actual changes canonicalize metadata.
fn effective_changes(c: &Context, changes: &FormFrameChanges) -> EngineResult<FormFrameChanges> {
    if changes
        .width
        .is_some_and(|w| !w.is_finite() || !(0. ..=1_000_000.).contains(&w))
    {
        return Err(bad("invalid raw field width"));
    }
    let s = &c.states[c.main];
    let p = &s.original.paint;
    let color_change = |request: &Option<FrameColorChange>,
                        original: Option<PaintColor>|
     -> EngineResult<Option<FrameColorChange>> {
        let requested = requested_color(request, original.clone())?;
        if c.default_colors && matches!(request, Some(FrameColorChange::Rgb(_))) {
            // Device component equality does not prove displayed RGB
            // equality when the source substitutes a calibrated space.
            return Ok(request.clone());
        }
        let equal = match (&requested, &original) {
            (None, None) => true,
            (Some(a), Some(b)) => rgb_values(a)
                .iter()
                .zip(rgb_values(b))
                .all(|(a, b)| (a - b).abs() < 1e-7),
            _ => false,
        };
        Ok(if equal { None } else { request.clone() })
    };
    Ok(FormFrameChanges {
        width: changes.width.filter(|w| *w != raw_width(s, &c.meta) as f32),
        style: changes.style.filter(|s| Some(*s) != style(p, &c.meta)),
        border: color_change(&changes.border, effective_border(p, &c.meta))?,
        background: color_change(&changes.background, p.background_color.clone())?,
    })
}
fn color_object(c: Option<&PaintColor>) -> Object {
    let values = match c {
        None => vec![],
        Some(PaintColor::Gray(n)) => vec![*n],
        Some(PaintColor::Rgb(n)) => n.to_vec(),
        Some(PaintColor::Cmyk(n)) => n.to_vec(),
    };
    Object::Array(values.into_iter().map(|n| Object::Real(n as f32)).collect())
}
pub(crate) fn change(
    bytes: &[u8],
    page: usize,
    index: usize,
    changes: &FormFrameChanges,
) -> EngineResult<Vec<u8>> {
    let mut c = load(bytes, page, index)?;
    let changes = effective_changes(&c, changes)?;
    if changes.width.is_none()
        && changes.style.is_none()
        && changes.border.is_none()
        && changes.background.is_none()
    {
        return Ok(bytes.to_vec());
    }
    let changes = &changes;
    let targets = targets(&c, changes)?;
    let main = &targets[c.main];
    let raw_main_width = main.width * c.states[c.main].original.scale;
    let main_style = changes
        .style
        .or(style(&c.states[c.main].original.paint, &c.meta))
        .unwrap_or(c.meta.style);
    let widget = c.doc.get_dictionary(c.id).map_err(err)?;
    let old_ap = dictionary(&c.doc, widget, b"AP")?.ok_or_else(|| bad("missing appearance"))?;
    let mut ap = Dictionary::new();
    for (key, value) in old_ap.iter() {
        if !matches!(key.as_slice(), b"N" | b"R" | b"D") {
            c.budget.entry(key, value)?;
            ap.set(key.clone(), value.clone());
        }
    }
    let border_change =
        changes.width.is_some() || changes.style.is_some() || changes.border.is_some();
    let mut mk = if let Some(mk) = dictionary(&c.doc, widget, b"MK")? {
        c.budget.dictionary(mk)?;
        mk.clone()
    } else {
        Dictionary::new()
    };
    if border_change {
        mk.set("BC", color_object(main.border.as_ref()));
    }
    if changes.background.is_some() {
        mk.set("BG", color_object(main.background.as_ref()));
    }
    let mut bs = if let Some(bs) = dictionary(&c.doc, widget, b"BS")? {
        c.budget.dictionary(bs)?;
        bs.clone()
    } else {
        Dictionary::new()
    };
    if border_change {
        bs.set("W", raw_main_width as f32);
        bs.set(
            "S",
            match main_style {
                FrameStyle::Solid => "S",
                FrameStyle::Dashed => "D",
                FrameStyle::Underline => "U",
                FrameStyle::Beveled => "B",
                FrameStyle::Inset => "I",
            },
        );
        if main_style == FrameStyle::Dashed {
            bs.set(
                "D",
                main.dash
                    .iter()
                    .map(|n| Object::Real((n * c.states[c.main].original.scale) as f32))
                    .collect::<Vec<_>>(),
            );
        } else {
            bs.remove(b"D");
        }
    }
    let mut cache: HashMap<(usize, String), ObjectId> = HashMap::new();
    let mut modes: HashMap<Vec<u8>, Dictionary> = HashMap::new();
    for (s, target) in c.states.iter().zip(&targets) {
        c.budget.reserve_bytes(1024 + target.dash.len() * 64)?;
        let cache_key = (s.original.key, format!("{target:?}"));
        let id = if let Some(id) = cache.get(&cache_key) {
            *id
        } else {
            let proof = &s.original.paint;
            for op in &proof.content.operations {
                c.budget.reserve_bytes(op.operator.len() + 96)?;
                c.budget.array(&op.operands)?;
            }
            c.budget.reserve_bytes(
                crate::form_answer_appearance::program_encoded_bound(&proof.content.operations)?
                    .saturating_add(64 * 1024),
            )?;
            c.budget.dictionary(&s.original.stream.dict)?;
            let program = form_frame_render::rewrite(proof, target)?;
            let mut dict = s.original.stream.dict.clone();
            dict.remove(b"Length");
            dict.remove(b"Filter");
            dict.remove(b"DecodeParms");
            let id = c.doc.add_object(Stream::new(dict, program));
            cache.insert(cache_key, id);
            id
        };
        if let Some(name) = &s.name {
            modes
                .entry(s.mode.clone())
                .or_insert_with(Dictionary::new)
                .set(name.clone(), id);
        } else {
            ap.set(s.mode.clone(), id);
        }
    }
    for (mode, states) in modes {
        ap.set(mode, states);
    }
    let widget = c.doc.get_dictionary_mut(c.id).map_err(err)?;
    widget.set("AP", ap);
    if border_change || changes.background.is_some() {
        widget.set("MK", mk);
    }
    if border_change {
        widget.set("BS", bs);
        widget.remove(b"Border");
    }
    crate::page_resources::prune_unreachable(&mut c.doc)?;
    crate::pdf_output::save(&mut c.doc)
}

#[cfg(test)]
#[path = "form_frame_document_tests.rs"]
mod tests;
