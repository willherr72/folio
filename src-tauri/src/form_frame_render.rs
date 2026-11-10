//! Paint-only frame rewriting. Graph publication and metadata are separate.
#![allow(dead_code)]
use crate::form_frame::FrameStyle;
use crate::form_frame_paint::{FramePaint, FrameShape, PaintColor, PaintStyle};
use crate::{EngineError, EngineResult};
#[cfg(test)]
use lopdf::content::Content;
use lopdf::content::Operation;
use lopdf::Object;
use std::ops::Range;

#[derive(Debug)]
pub(crate) struct FramePaintTarget {
    pub(crate) width: f64,
    pub(crate) style: FrameStyle,
    pub(crate) dash: Vec<f64>,
    pub(crate) dash_phase: f64,
    pub(crate) border: Option<PaintColor>,
    pub(crate) background: Option<PaintColor>,
    pub(crate) shading: Option<[PaintColor; 2]>,
}
fn bad(reason: &str) -> EngineError {
    EngineError::InvalidRequest(format!("Form frame: {reason}"))
}
fn op(name: &str, values: &[f64]) -> Operation {
    Operation::new(
        name,
        values.iter().map(|v| Object::Real(*v as f32)).collect(),
    )
}
fn color(c: &PaintColor, stroke: bool) -> EngineResult<Operation> {
    let (operator, values) = match c {
        PaintColor::Gray(n) => (if stroke { "G" } else { "g" }, std::slice::from_ref(n)),
        PaintColor::Rgb(n) => (if stroke { "RG" } else { "rg" }, n.as_slice()),
        PaintColor::Cmyk(n) => (if stroke { "K" } else { "k" }, n.as_slice()),
    };
    if values
        .iter()
        .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
    {
        return Err(bad("invalid frame paint color"));
    }
    Ok(op(operator, values))
}
fn shape_path(shape: &FrameShape, inset: f64) -> Vec<Operation> {
    match shape {
        FrameShape::Rectangle([x, y, w, h]) => vec![op(
            "re",
            &[x + inset, y + inset, w - 2. * inset, h - 2. * inset],
        )],
        FrameShape::Circle([x, y, r]) => {
            let r = r - inset;
            let k = r * 0.5522848;
            vec![
                op("m", &[x + r, *y]),
                op("c", &[x + r, y + k, x + k, y + r, *x, y + r]),
                op("c", &[x - k, y + r, x - r, y + k, x - r, *y]),
                op("c", &[x - r, y - k, x - k, y - r, *x, y - r]),
                op("c", &[x + k, y - r, x + r, y - k, x + r, *y]),
            ]
        }
    }
}
fn validate(shape: &FrameShape, t: &FramePaintTarget) -> EngineResult<()> {
    if !t.width.is_finite() || t.width < 0. || t.width > 1_000_000. {
        return Err(bad("invalid raw frame width"));
    }
    let values = match shape {
        FrameShape::Rectangle(v) => v.as_slice(),
        FrameShape::Circle(v) => v.as_slice(),
    };
    if values
        .iter()
        .any(|v| !v.is_finite() || v.abs() > 1_000_000.)
    {
        return Err(bad("invalid frame shape"));
    }
    let fits = match shape {
        FrameShape::Rectangle([_, _, w, h]) => {
            *w > 0.
                && *h > 0.
                && if t.style == FrameStyle::Underline {
                    t.width < *h
                } else if matches!(t.style, FrameStyle::Beveled | FrameStyle::Inset) {
                    t.width * 4. < w.min(*h)
                } else {
                    t.width * 2. < w.min(*h)
                }
        }
        FrameShape::Circle([_, _, r]) => {
            *r > 0. && t.width < *r && matches!(t.style, FrameStyle::Solid | FrameStyle::Dashed)
        }
    };
    if !fits {
        return Err(bad("border does not fit the field shape"));
    }
    if !t.dash_phase.is_finite()
        || t.dash_phase < 0.
        || t.dash_phase > 1_000_000.
        || t.dash.len() > 64
        || t.dash
            .iter()
            .any(|v| !v.is_finite() || *v < 0. || *v > 1_000_000.)
        || (!t.dash.is_empty() && t.dash.iter().all(|v| *v == 0.))
    {
        return Err(bad("invalid border dash pattern"));
    }
    if let Some(c) = &t.border {
        color(c, true)?;
    }
    if let Some(c) = &t.background {
        color(c, false)?;
    }
    if let Some(colors) = &t.shading {
        for c in colors {
            color(c, false)?;
        }
    }
    Ok(())
}
fn background(shape: &FrameShape, t: &FramePaintTarget) -> EngineResult<Vec<Operation>> {
    let Some(c) = &t.background else {
        return Ok(vec![]);
    };
    let mut out = vec![op("q", &[]), color(c, false)?];
    out.extend(shape_path(shape, 0.));
    out.extend([op("f", &[]), op("Q", &[])]);
    Ok(out)
}
// A dashed circle's visible pattern depends on its source start, traversal and
// closure, not just its center/radius. Scale that proved path about its center.
fn border_path(source: &FramePaint, inset: f64) -> EngineResult<Vec<Operation>> {
    let FrameShape::Circle([cx, cy, radius]) = source.shape else {
        return Ok(shape_path(&source.shape, inset));
    };
    let mut path = Vec::new();
    for item in &source.content.operations[..source.border] {
        match item.operator.as_str() {
            "m" | "l" | "c" | "v" | "y" | "h" | "re" => {
                // Earlier unrelated paths may be complex. Keep only a bounded
                // borrowed prefix until a paint clears it; the proved path is <=6.
                if path.len() < 7 { path.push(item); }
            }
            "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n" => path.clear(),
            _ => {}
        }
    }
    let first = path.first().ok_or_else(|| bad("missing proved circle path"))?;
    if first.operator != "m" || first.operands.len() != 2 || path.len() > 6 {
        return Err(bad("invalid proved circle path"));
    }
    let number = |v: &Object| v.as_float().map(f64::from).map_err(|_| bad("invalid circle coordinate"));
    let old_radius = (number(&first.operands[0])? - cx).hypot(number(&first.operands[1])? - cy);
    let ratio = (radius - inset) / old_radius;
    if !ratio.is_finite() || ratio <= 0. { return Err(bad("invalid circle scaling")); }
    let mut path = path.into_iter().cloned().collect::<Vec<_>>();
    if ratio != 1. {
        for item in &mut path {
            for pair in item.operands.chunks_mut(2) {
                if pair.len() != 2 { return Err(bad("invalid circle coordinates")); }
                pair[0] = Object::Real((cx + (number(&pair[0])? - cx) * ratio) as f32);
                pair[1] = Object::Real((cy + (number(&pair[1])? - cy) * ratio) as f32);
            }
        }
    }
    if source.content.operations[source.border].operator == "s" && path.last().is_some_and(|p| p.operator != "h") {
        path.push(op("h", &[]));
    }
    Ok(path)
}
fn border(source: &FramePaint, t: &FramePaintTarget) -> EngineResult<Vec<Operation>> {
    let shape = &source.shape;
    if t.width == 0. || t.border.is_none() {
        let mut out = vec![
            op("q", &[]),
            op("w", &[0.]),
            color(t.border.as_ref().unwrap_or(&PaintColor::Gray(0.)), false)?,
        ];
        if t.style == FrameStyle::Dashed {
            out.push(Operation::new(
                "d",
                vec![
                    Object::Array(t.dash.iter().map(|v| Object::Real(*v as f32)).collect()),
                    Object::Real(t.dash_phase as f32),
                ],
            ));
        }
        out.extend(border_path(source, 0.)?);
        out.extend([op("n", &[]), op("Q", &[])]);
        return Ok(out);
    }
    let c = t.border.as_ref().unwrap();
    let mut out = vec![op("q", &[])];
    if matches!(t.style, FrameStyle::Beveled | FrameStyle::Inset) {
        let FrameShape::Rectangle([x, y, w, h]) = shape else {
            return Err(bad("compound style requires rectangular frame"));
        };
        let d = t.width;
        let colors = t.shading.clone().unwrap_or_else(|| {
            if t.style == FrameStyle::Inset {
                [PaintColor::Gray(0.5), PaintColor::Gray(0.75)]
            } else {
                [PaintColor::Gray(1.), half_background(t.background.as_ref())]
            }
        });
        let edges = [
            [
                [x + d, y + d],
                [x + d, y + h - d],
                [x + w - d, y + h - d],
                [x + w - 2. * d, y + h - 2. * d],
                [x + 2. * d, y + h - 2. * d],
                [x + 2. * d, y + 2. * d],
            ],
            [
                [x + w - d, y + h - d],
                [x + w - d, y + d],
                [x + d, y + d],
                [x + 2. * d, y + 2. * d],
                [x + w - 2. * d, y + 2. * d],
                [x + w - 2. * d, y + h - 2. * d],
            ],
        ];
        for (points, c) in edges.iter().zip(&colors) {
            out.push(color(c, false)?);
            for (i, p) in points.iter().enumerate() {
                out.push(op(if i == 0 { "m" } else { "l" }, p));
            }
            out.push(op("f", &[]));
        }
    }
    if matches!(shape, FrameShape::Rectangle(_))
        && matches!(
            t.style,
            FrameStyle::Solid | FrameStyle::Beveled | FrameStyle::Inset
        )
    {
        out.push(color(c, false)?);
        out.extend(shape_path(shape, 0.));
        out.extend(shape_path(shape, t.width));
        out.push(op("f*", &[]));
    } else {
        out.extend([
            color(c, true)?,
            op("w", &[t.width]),
            op("J", &[0.]),
            op("j", &[0.]),
            op("M", &[10.]),
        ]);
        let dash = if t.style == FrameStyle::Dashed {
            t.dash.as_slice()
        } else {
            &[]
        };
        out.push(Operation::new(
            "d",
            vec![
                Object::Array(dash.iter().map(|v| Object::Real(*v as f32)).collect()),
                Object::Real(if t.style == FrameStyle::Dashed {
                    t.dash_phase as f32
                } else {
                    0.
                }),
            ],
        ));
        if t.style == FrameStyle::Underline {
            let FrameShape::Rectangle([x, y, w, _]) = shape else {
                return Err(bad("underline requires rectangular frame"));
            };
            out.extend([
                op("m", &[*x, y + t.width / 2.]),
                op("l", &[x + w, y + t.width / 2.]),
            ]);
        } else {
            out.extend(border_path(source, t.width / 2.)?);
        }
        out.push(op("S", &[]));
    }
    out.push(op("Q", &[]));
    Ok(out)
}
fn half_background(c: Option<&PaintColor>) -> PaintColor {
    match c {
        None => PaintColor::Gray(0.5),
        Some(PaintColor::Gray(v)) => PaintColor::Gray(v / 2.),
        Some(PaintColor::Rgb(v)) => PaintColor::Rgb(v.map(|v| v / 2.)),
        // Preserve an explicit CMYK shading supplied by the source. When
        // creating new shading, convert to RGB before darkening subtractive ink.
        Some(PaintColor::Cmyk([c, m, y, k])) => {
            PaintColor::Rgb([c, m, y].map(|v| (1. - (v + k).min(1.)) / 2.))
        }
    }
}
/// Reuse only a structurally self-contained q...paint Q block. No marker or
/// producer name grants authority. If its entry path is live, q/Q cannot
/// isolate it, since PDF saves graphics state but not the current path.
fn isolated_block(ops: &[Operation], paints: &[usize]) -> Option<Range<usize>> {
    let paint = *paints.iter().max()?;
    if ops.get(paint + 1)?.operator != "Q" {
        return None;
    }
    let mut start = *paints.iter().min()?;
    while start > 0 {
        start -= 1;
        match ops[start].operator.as_str() {
            "q" => break,
            "g" | "G" | "rg" | "RG" | "k" | "K" | "w" | "J" | "j" | "M" | "d" | "m" | "l" | "c"
            | "h" | "re" => {}
            _ => return None,
        }
    }
    if ops[start].operator != "q" {
        return None;
    }
    for (i, op) in ops.iter().enumerate().take(paint + 1).skip(start + 1) {
        if paints.contains(&i) {
            continue;
        }
        if !matches!(
            op.operator.as_str(),
            "g" | "G"
                | "rg"
                | "RG"
                | "k"
                | "K"
                | "w"
                | "J"
                | "j"
                | "M"
                | "d"
                | "m"
                | "l"
                | "c"
                | "h"
                | "re"
        ) {
            return None;
        }
    }
    let mut live_path = false;
    for op in &ops[..start] {
        match op.operator.as_str() {
            "m" | "l" | "c" | "v" | "y" | "h" | "re" => live_path = true,
            "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n" => live_path = false,
            _ => {}
        }
    }
    (!live_path).then_some(start..paint + 2)
}
fn replacement(
    ops: &[Operation],
    index: usize,
    mut paint: Vec<Operation>,
) -> EngineResult<(Range<usize>, Vec<Operation>)> {
    if let Some(block) = isolated_block(ops, &[index]) {
        Ok((block, paint))
    } else {
        // Use a neutral width while discarding the original path so a former
        // background under w=0 cannot masquerade as an invisible border. Do not
        // introduce q/Q into that live path: later answer admission deliberately
        // rejects that otherwise legal PDF construction. Restore its exact width.
        let mut width = 1.;
        let mut stack = Vec::new();
        for prior in &ops[..index] {
            match prior.operator.as_str() {
                "q" => stack.push(width),
                "Q" => width = stack.pop().ok_or_else(|| bad("unmatched frame state"))?,
                "w" => {
                    width = prior
                        .operands
                        .first()
                        .ok_or_else(|| bad("missing line width"))?
                        .as_float()
                        .map_err(|_| bad("invalid line width"))? as f64
                }
                _ => {}
            }
        }
        if width == 0. {
            paint.splice(0..0, [op("w", &[1.]), op("n", &[]), op("w", &[width])]);
        } else {
            paint.insert(0, op("n", &[]));
        }
        Ok((index..index + 1, paint))
    }
}
pub(crate) fn rewrite(source: &FramePaint, target: &FramePaintTarget) -> EngineResult<Vec<u8>> {
    validate(&source.shape, target)?;
    let ops = &source.content.operations;
    let paints = source
        .edge_paints
        .iter()
        .copied()
        .chain(std::iter::once(source.border))
        .collect::<Vec<_>>();
    let same_style = match &source.style {
        PaintStyle::Solid => target.style == FrameStyle::Solid,
        PaintStyle::Dashed(d, phase) => target.style == FrameStyle::Dashed && *d == target.dash && *phase == target.dash_phase,
        PaintStyle::Underline => target.style == FrameStyle::Underline,
        PaintStyle::Compound => matches!(target.style, FrameStyle::Beveled | FrameStyle::Inset),
        PaintStyle::Invisible => false,
    };
    let keep_border = same_style && source.width == target.width
        && target.border.as_ref() == Some(&source.color) && source.edge_colors == target.shading;
    let new_border = if keep_border { vec![] } else { border(source, target)? };
    let mut edits = if keep_border {
        vec![]
    } else if source.border == usize::MAX {
        let at = source
            .background
            .map(|i| isolated_block(ops, &[i]).map(|r| r.end).unwrap_or(i + 1))
            .unwrap_or(0);
        vec![(at..at, new_border)]
    } else if let Some(block) = isolated_block(ops, &paints) {
        vec![(block, new_border)]
    } else {
        let mut edits = source
            .edge_paints
            .iter()
            .map(|i| replacement(ops, *i, vec![]))
            .collect::<EngineResult<Vec<_>>>()?;
        edits.push(replacement(ops, source.border, new_border)?);
        edits
    };
    if source.background_color != target.background {
        let bg = background(&source.shape, target)?;
        if let Some(index) = source.background {
            edits.push(replacement(ops, index, bg)?);
        } else if !bg.is_empty() {
            edits.insert(0, (0..0, bg));
        }
    }
    edits.sort_by_key(|(r, _)| (r.start, r.end));
    let mut cursor = 0;
    let mut count = ops.len();
    for (range, paint) in &edits {
        if range.start < cursor || range.end > ops.len() {
            return Err(bad("overlapping frame replacements"));
        }
        count = count - range.len() + paint.len();
        cursor = range.end;
    }
    if count > 262144 {
        return Err(bad("frame output exceeds operation limit"));
    }
    // Charge all cloned operands and operator storage before allocating the
    // output. Publication additionally shares a budget across every AP state.
    let mut budget = crate::form_graph::CopyBudget::with_byte_limit(64 * 1024 * 1024);
    for op in ops.iter().chain(edits.iter().flat_map(|(_, v)| v.iter())) {
        budget.reserve_bytes(op.operator.len() + 96)?;
        budget.array(&op.operands)?;
    }
    let mut output = Vec::with_capacity(count);
    cursor = 0;
    for (range, paint) in edits {
        output.extend_from_slice(&ops[cursor..range.start]);
        output.extend(paint);
        cursor = range.end;
    }
    output.extend_from_slice(&ops[cursor..]);
    crate::form_answer_appearance::encode_program(&output)
}

#[cfg(test)]
mod tests {
    use super::*;
    const BOX: [f64; 4] = [0., 0., 100., 30.];
    const INPUT:&str="q 0.9 g 0 0 100 30 re f 0 g 0 0 100 30 re 1 1 98 28 re f* 1 0 0 rg 8 8 3 3 re f BT /F0 12 Tf 1 0 0 1 4 12 Tm (Answer) Tj ET Q";
    fn target() -> FramePaintTarget {
        FramePaintTarget {
            width: 2.25,
            style: FrameStyle::Solid,
            dash: vec![3., 2.],
            dash_phase: 0.,
            border: Some(PaintColor::Rgb([0., 0., 1.])),
            background: Some(PaintColor::Gray(0.75)),
            shading: None,
        }
    }
    fn foreground(bytes: &[u8]) -> Vec<u8> {
        let content = Content::decode_strict(bytes).unwrap();
        let start = content
            .operations
            .iter()
            .position(|o| o.operator == "rg" && o.operands == vec![1.into(), 0.into(), 0.into()])
            .unwrap();
        Content {
            operations: &content.operations[start..],
        }
        .encode()
        .unwrap()
    }
    #[test]
    fn replacement_keeps_foreground_and_state_effects() {
        let source = FramePaint::inspect(INPUT.as_bytes(), BOX, false).unwrap();
        let saved = rewrite(&source, &target()).unwrap();
        let p = FramePaint::inspect(&saved, BOX, false).unwrap();
        assert_eq!(p.width, 2.25);
        assert_eq!(p.color, PaintColor::Rgb([0., 0., 1.]));
        assert_eq!(p.background_color, Some(PaintColor::Gray(0.75)));
        assert_eq!(foreground(INPUT.as_bytes()), foreground(&saved));
    }
    #[test]
    fn solid_dashed_and_underline_can_be_reinspected() {
        let source = FramePaint::inspect(INPUT.as_bytes(), BOX, false).unwrap();
        for style in [FrameStyle::Solid, FrameStyle::Dashed, FrameStyle::Underline] {
            let mut t = target();
            t.style = style;
            let saved = rewrite(&source, &t).unwrap();
            let p = FramePaint::inspect(&saved, BOX, false).unwrap();
            assert_eq!(p.width, 2.25);
            assert_eq!(foreground(INPUT.as_bytes()), foreground(&saved));
            match style {
                FrameStyle::Dashed => assert!(
                    matches!(p.style,crate::form_frame_paint::PaintStyle::Dashed(ref d,0.) if d==&vec![3.,2.])
                ),
                FrameStyle::Underline => {
                    assert_eq!(p.style, crate::form_frame_paint::PaintStyle::Underline)
                }
                _ => assert_eq!(p.style, crate::form_frame_paint::PaintStyle::Solid),
            }
        }
    }
    #[test]
    fn repeated_edits_have_bounded_instruction_count() {
        let source = FramePaint::inspect(INPUT.as_bytes(), BOX, false).unwrap();
        let mut saved = rewrite(&source, &target()).unwrap();
        let first = Content::decode_strict(&saved).unwrap().operations.len();
        for i in 0..100 {
            let p = FramePaint::inspect(&saved, BOX, false).unwrap();
            let mut t = target();
            t.width = 1. + f64::from(i % 5) / 4.;
            saved = rewrite(&p, &t).unwrap();
            assert_eq!(
                Content::decode_strict(&saved).unwrap().operations.len(),
                first,
                "growth at edit {i}"
            );
        }
        assert_eq!(foreground(INPUT.as_bytes()), foreground(&saved));
    }
    #[test]
    fn zero_or_transparent_border_is_not_a_hairline() {
        let source = FramePaint::inspect(b"q 0 0 100 30 re 1 1 98 28 re f* Q", BOX, false).unwrap();
        for transparent in [false, true] {
            let mut t = target();
            t.background = None;
            if transparent {
                t.border = None;
            } else {
                t.width = 0.;
            }
            let saved = rewrite(&source, &t).unwrap();
            let ops = Content::decode_strict(&saved).unwrap().operations;
            assert!(!ops
                .iter()
                .any(|o| matches!(o.operator.as_str(), "S" | "s" | "f" | "f*" | "B" | "B*")));
        }
    }
    #[test]
    fn adding_background_stays_behind_existing_answer() {
        let input = b"q 0 0 100 30 re 1 1 98 28 re f* Q BT /F0 12 Tf (Answer) Tj ET";
        let source = FramePaint::inspect(input, BOX, false).unwrap();
        let saved = rewrite(&source, &target()).unwrap();
        let p = FramePaint::inspect(&saved, BOX, false).unwrap();
        assert!(p.background.unwrap() < p.border);
    }
    #[test]
    fn background_only_keeps_noncanonical_dashed_circle_paint_exact() {
        let input=b"q 0 G 1 w [3 2] 1.5 d 3 15 m 3 17.761424 5.238576 20 8 20 c 10.761424 20 13 17.761424 13 15 c 13 12.238576 10.761424 10 8 10 c 5.238576 10 3 12.238576 3 15 c S Q";
        let source=FramePaint::inspect(input,BOX,true).unwrap();
        let t=FramePaintTarget{width:1.,style:FrameStyle::Dashed,dash:vec![3.,2.],dash_phase:1.5,border:Some(PaintColor::Gray(0.)),background:Some(PaintColor::Rgb([1.,0.8,0.7])),shading:None};
        let saved=rewrite(&source,&t).unwrap();
        assert!(saved.ends_with(&source.content.encode().unwrap()),"Original dashed border must retain its source path and state");
        let proof=FramePaint::inspect(&saved,BOX,true).unwrap();
        assert_eq!(proof.style,source.style);
    }
    #[test]
    fn resized_noncanonical_dashed_circle_retains_start_direction_and_closure() {
        let input=b"q 0 G 1 w [3 2] 1.5 d 3 15 m 3 17.761424 5.238576 20 8 20 c 10.761424 20 13 17.761424 13 15 c 13 12.238576 10.761424 10 8 10 c 5.238576 10 3 12.238576 3 15 c s Q";
        let source=FramePaint::inspect(input,BOX,true).unwrap();
        let t=FramePaintTarget{width:1.5,style:FrameStyle::Dashed,dash:vec![3.,2.],dash_phase:1.5,border:Some(PaintColor::Rgb([0.8,0.,0.])),background:None,shading:None};
        let saved=rewrite(&source,&t).unwrap();
        let ops=Content::decode_strict(&saved).unwrap().operations;
        let start=ops.iter().find(|o|o.operator=="m").unwrap();
        assert!((start.operands[0].as_float().unwrap()-3.25).abs()<0.0001,"dashes must retain their left anchor");
        let curve=ops.iter().find(|o|o.operator=="c").unwrap();
        assert!((curve.operands[0].as_float().unwrap()-3.25).abs()<0.0001);
        assert!(curve.operands[1].as_float().unwrap()>15.);
        assert!(ops.windows(2).any(|p|p[0].operator=="h"&&p[1].operator=="S"),"implicit source closure must survive");
    }
    #[test]
    fn target_geometry_and_colors_are_bounded() {
        let source = FramePaint::inspect(INPUT.as_bytes(), BOX, false).unwrap();
        for width in [-1., f64::NAN, f64::INFINITY, 15., 1e20] {
            let mut t = target();
            t.width = width;
            assert!(rewrite(&source, &t).is_err());
        }
        let mut t = target();
        t.border = Some(PaintColor::Rgb([f64::NAN, 0., 0.]));
        assert!(rewrite(&source, &t).is_err());
        let mut t = target();
        t.style = FrameStyle::Dashed;
        t.dash = vec![0., 0.];
        assert!(rewrite(&source, &t).is_err());
    }
    #[test]
    fn circular_frames_keep_actual_position_and_label() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/form-frame-programs.json"))
                .unwrap();
        for c in cases
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["radio"] == true)
        {
            let b = c["bounds"].as_array().unwrap();
            let bounds = std::array::from_fn(|i| b[i].as_f64().unwrap());
            let input = c["program"].as_str().unwrap();
            let source = FramePaint::inspect(input.as_bytes(), bounds, true).unwrap();
            let mut t = target();
            t.width = 0.75;
            for style in [FrameStyle::Solid, FrameStyle::Dashed] {
                t.style = style;
                let saved = rewrite(&source, &t).unwrap();
                let proof = FramePaint::inspect(&saved, bounds, true).unwrap();
                match (&proof.shape, &source.shape) {
                    (FrameShape::Circle(a), FrameShape::Circle(b)) => {
                        assert!(a.iter().zip(b).all(|(a, b)| (a - b).abs() < 0.0001))
                    }
                    _ => panic!("circle changed shape"),
                }
                let signature = |bytes: &[u8]| {
                    let c = Content::decode_strict(bytes).unwrap();
                    Content {
                        operations: c
                            .operations
                            .iter()
                            .filter(|o| {
                                matches!(o.operator.as_str(), "BT" | "ET" | "Tf" | "Tm" | "Tj")
                            })
                            .cloned()
                            .collect::<Vec<_>>(),
                    }
                    .encode()
                    .unwrap()
                };
                assert_eq!(signature(input.as_bytes()), signature(&saved));
            }
            t.style = FrameStyle::Underline;
            assert!(rewrite(&source, &t).is_err());
        }
    }

    #[test]
    fn compound_styles_preserve_foreground_and_reedit_without_growth() {
        let source = FramePaint::inspect(INPUT.as_bytes(), BOX, false).unwrap();
        for style in [FrameStyle::Beveled, FrameStyle::Inset] {
            let mut t = target();
            t.style = style;
            let mut saved = rewrite(&source, &t).unwrap();
            let count = Content::decode_strict(&saved).unwrap().operations.len();
            for _ in 0..12 {
                let proof = FramePaint::inspect(&saved, BOX, false).unwrap();
                assert_eq!(format!("{:?}", proof.style), "Compound");
                saved = rewrite(&proof, &t).unwrap();
                assert_eq!(
                    Content::decode_strict(&saved).unwrap().operations.len(),
                    count
                );
                assert_eq!(foreground(INPUT.as_bytes()), foreground(&saved));
            }
        }
    }
    #[test]
    fn invisible_frames_can_be_restored_for_rectangles_and_labeled_radios() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/form-frame-programs.json"))
                .unwrap();
        for c in cases.as_array().unwrap() {
            let bounds = std::array::from_fn(|i| c["bounds"][i].as_f64().unwrap());
            let radio = c["radio"].as_bool().unwrap();
            let original =
                FramePaint::inspect(c["program"].as_str().unwrap().as_bytes(), bounds, radio)
                    .unwrap();
            let mut t = target();
            t.width = 0.;
            t.background = None;
            let mut saved = rewrite(&original, &t).unwrap();
            for _ in 0..5 {
                let invisible = FramePaint::inspect(&saved, bounds, radio).unwrap();
                assert_eq!(format!("{:?}", invisible.style), "Invisible");
                t.width = 0.5;
                t.border = Some(PaintColor::Gray(0.));
                let restored = rewrite(&invisible, &t).unwrap();
                let restored = FramePaint::inspect(&restored, bounds, radio).unwrap();
                assert!((restored.width - 0.5).abs() < 0.0001);
                t.border = None;
                saved = rewrite(&restored, &t).unwrap();
            }
        }
    }
    #[test]
    fn removing_background_does_not_leave_a_false_invisible_border() {
        let input = format!("0 w {INPUT}");
        let source = FramePaint::inspect(input.as_bytes(), BOX, false).unwrap();
        let mut t = target();
        t.background = None;
        let saved = rewrite(&source, &t).unwrap();
        let proof = FramePaint::inspect(&saved, BOX, false).unwrap();
        assert_eq!(proof.width, t.width);
        assert!(proof.background.is_none());
    }

    #[test]
    fn tagged_foreground_property_dictionary_is_preserved() {
        let input=b"q 0 0 100 30 re 1 1 98 28 re f* Q /Artifact << /Subtype /Layout /Attached [/Top /Bottom] /Flag true >> BDC BT /F0 12 Tf (Answer) Tj ET EMC";
        let source = FramePaint::inspect(input, BOX, false).unwrap();
        let saved = rewrite(&source, &target()).unwrap();
        let tail = |b: &[u8]| {
            let c = Content::decode_strict(b).unwrap();
            let i = c
                .operations
                .iter()
                .position(|o| o.operator == "BDC")
                .unwrap();
            Content {
                operations: &c.operations[i..],
            }
            .encode()
            .unwrap()
        };
        assert_eq!(tail(input), tail(&saved));
    }

    #[test]
    fn native_compound_samples_keep_shading_and_emit_independent_raster_cases() {
        use lopdf::{dictionary, Document, Stream};
        fn pdf(program: Vec<u8>) -> Vec<u8> {
            let mut doc = Document::with_version("1.7");
            let pages = doc.new_object_id();
            let font = doc.add_object(
                dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"},
            );
            let mut content = b"q 1 0 0 1 20 120 cm ".to_vec();
            content.extend(program);
            content.extend_from_slice(b" Q");
            let content = doc.add_object(Stream::new(dictionary! {}, content));
            let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),320.into(),240.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
            doc.objects.insert(
                pages,
                dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into(),
            );
            let catalog = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
            doc.trailer.set("Root", catalog);
            crate::pdf_output::save(&mut doc).unwrap()
        }
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/form-frame-compound-programs.json"
        ))
        .unwrap();
        for (i, c) in cases.as_array().unwrap().iter().enumerate() {
            let bounds = std::array::from_fn(|i| c["bounds"][i].as_f64().unwrap());
            let input = c["program"].as_str().unwrap().as_bytes();
            let original = FramePaint::inspect(input, bounds, false).unwrap();
            let mut t = target();
            t.style = if c["style"] == "B" {
                FrameStyle::Beveled
            } else {
                FrameStyle::Inset
            };
            t.background = original.background_color.clone();
            t.shading = original.edge_colors.clone();
            let saved = rewrite(&original, &t).unwrap();
            let proof = FramePaint::inspect(&saved, bounds, false).unwrap();
            assert_eq!(proof.edge_colors, original.edge_colors);
            assert_eq!(proof.background_color, original.background_color);
            if let Ok(dir) = std::env::var("FOLIO_FRAME_PAINT_PROBE") {
                let dir = std::path::Path::new(&dir);
                std::fs::create_dir_all(dir).unwrap();
                for (suffix, bytes) in [("before", pdf(input.to_vec())), ("after", pdf(saved))] {
                    use std::io::Write;
                    let mut file = std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(dir.join(format!("{i}-{suffix}.pdf")))
                        .unwrap();
                    file.write_all(&bytes).unwrap();
                }
            }
        }
    }
}
