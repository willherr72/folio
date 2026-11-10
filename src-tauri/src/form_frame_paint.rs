//! Source frame admission. Not connected to publication until its proof is complete.
#![allow(dead_code)]
use crate::{EngineError, EngineResult};
use lopdf::{
    content::{Content, Operation},
    Object,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FrameShape {
    Rectangle([f64; 4]),
    Circle([f64; 3]),
}
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PaintColor {
    Gray(f64),
    Rgb([f64; 3]),
    Cmyk([f64; 4]),
}
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PaintStyle {
    Solid,
    Dashed(Vec<f64>, f64),
    Underline,
    Compound,
    Invisible,
}
pub(crate) struct FramePaint {
    pub(crate) content: Content,
    pub(crate) shape: FrameShape,
    pub(crate) border: usize,
    pub(crate) background: Option<usize>,
    pub(crate) width: f64,
    pub(crate) color: PaintColor,
    pub(crate) background_color: Option<PaintColor>,
    pub(crate) style: PaintStyle,
    pub(crate) stroke_style: PaintStyle,
    pub(crate) edge_paints: Vec<usize>,
    pub(crate) edge_colors: Option<[PaintColor; 2]>,
}
impl FramePaint {
    pub(crate) fn inspect(bytes: &[u8], bounds: [f64; 4], radio: bool) -> EngineResult<Self> {
        Self::inspect_with_absence(bytes, bounds, radio, false)
    }
    pub(crate) fn inspect_with_absence(
        bytes: &[u8],
        bounds: [f64; 4],
        radio: bool,
        allow_absent: bool,
    ) -> EngineResult<Self> {
        crate::form_answer_appearance::preflight_program(bytes)?;
        if bounds.iter().any(|v| !finite(*v)) || bounds[0] >= bounds[2] || bounds[1] >= bounds[3] {
            return Err(bad("invalid appearance bounds"));
        }
        let rect = [
            bounds[0],
            bounds[1],
            bounds[2] - bounds[0],
            bounds[3] - bounds[1],
        ];
        let content =
            Content::decode_strict(bytes).map_err(|_| bad("malformed appearance program"))?;
        if content.operations.len() > 262144 {
            return Err(bad("too many appearance operations"));
        }
        let mut state = State::default();
        let mut stack = Vec::new();
        let mut path = Path::default();
        let mut pending_clip = false;
        let mut text = false;
        let mut marks = 0usize;
        let mut foreground = false;
        let mut found: Option<Border> = None;
        let mut background: Option<Background> = None;
        let mut edges: Vec<Edge> = Vec::new();
        let mut absence_safe = true;
        let mut border_stroke_style = PaintStyle::Solid;
        for (i, op) in content.operations.iter().enumerate() {
            match op.operator.as_str() {
                "q" => {
                    nums(op, 0)?;
                    if stack.len() >= 64 {
                        return Err(bad("graphics nesting exceeds 64"));
                    }
                    stack.push(state.clone());
                }
                "Q" => {
                    nums(op, 0)?;
                    state = stack
                        .pop()
                        .ok_or_else(|| bad("unmatched graphics restore"))?;
                }
                "cm" => {
                    let n = nums(op, 6)?;
                    let a = state.matrix;
                    state.matrix = [
                        a[0] * n[0] + a[2] * n[1],
                        a[1] * n[0] + a[3] * n[1],
                        a[0] * n[2] + a[2] * n[3],
                        a[1] * n[2] + a[3] * n[3],
                        a[0] * n[4] + a[2] * n[5] + a[4],
                        a[1] * n[4] + a[3] * n[5] + a[5],
                    ];
                    if state.matrix.iter().any(|v| !finite(*v)) {
                        return Err(bad("excessive appearance transform"));
                    }
                }
                "w" => {
                    state.width = nums(op, 1)?[0];
                    if state.width < 0. {
                        return Err(bad("negative line width"));
                    }
                }
                "J" | "j" => {
                    let n = nums(op, 1)?[0];
                    if ![0., 1., 2.].contains(&n) {
                        return Err(bad("invalid line cap/join"));
                    }
                    if op.operator == "J" {
                        state.cap = n as u8;
                    } else {
                        state.join = n as u8;
                    }
                }
                "M" => {
                    state.miter = nums(op, 1)?[0];
                    if state.miter < 1. {
                        return Err(bad("invalid miter limit"));
                    }
                }
                "d" => {
                    state.dash = dash(op)?;
                }
                "g" | "G" | "rg" | "RG" | "k" | "K" => {
                    let stroke = matches!(op.operator.as_str(), "G" | "RG" | "K");
                    let n = nums(
                        op,
                        match op.operator.as_str() {
                            "g" | "G" => 1,
                            "rg" | "RG" => 3,
                            _ => 4,
                        },
                    )?;
                    if n.iter().any(|v| !(0. ..=1.).contains(v)) {
                        return Err(bad("out-of-range device color"));
                    }
                    let c = match n.len() {
                        1 => PaintColor::Gray(n[0]),
                        3 => PaintColor::Rgb([n[0], n[1], n[2]]),
                        _ => PaintColor::Cmyk([n[0], n[1], n[2], n[3]]),
                    };
                    if stroke {
                        state.stroke = Some(c);
                    } else {
                        state.fill = Some(c);
                    }
                }
                "CS" | "SC" | "SCN" => {
                    state.stroke = None;
                }
                "cs" | "sc" | "scn" => {
                    state.fill = None;
                }
                "gs" => {
                    one_name(op)?;
                    state.external = true;
                }
                "m" | "l" | "c" | "v" | "y" | "h" | "re" => {
                    if text {
                        return Err(bad("path construction inside text"));
                    }
                    let count = match op.operator.as_str() {
                        "m" | "l" => 2,
                        "c" => 6,
                        "v" | "y" | "re" => 4,
                        _ => 0,
                    };
                    let n = nums(op, count)?;
                    path.add(&op.operator, &n, state.matrix == IDENTITY)?;
                }
                "W" | "W*" => {
                    nums(op, 0)?;
                    if pending_clip || text {
                        return Err(bad("ambiguous clipping path"));
                    }
                    pending_clip = true;
                }
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n" => {
                    nums(op, 0)?;
                    if text {
                        return Err(bad("path paint inside text"));
                    }
                    let candidate=(if op.operator=="n"&&pending_clip{None}else{classify(&path,op.operator.as_str(),&state,rect,radio)}).filter(|c| {
                        // An inner On-state disc is retained foreground, not a
                        // second field background. An equal outer disc remains
                        // ambiguous and is rejected by the ordering checks.
                        !matches!((c,found.as_ref()),(Candidate::Background(Background{shape:FrameShape::Circle(a),..}),Some(Border{shape:FrameShape::Circle(b),width,..}))
                            if (a[0]-b[0]).hypot(a[1]-b[1])+a[2] < b[2]-width+0.0001)
                    });
                    if let Some(mut candidate) = candidate {
                        let stroke = matches!(op.operator.as_str(), "S" | "s");
                        if pending_clip
                            || !path.identity
                            || state.matrix != IDENTITY
                            || !state.clip
                            || state.external
                            || marks != 0
                        {
                            return Err(bad(
                                "frame uses unproved transform, clipping or graphics state",
                            ));
                        }
                        if (stroke && (state.stroke.is_none() || state.cap != 0))
                            || (!stroke && state.fill.is_none())
                        {
                            return Err(bad("frame uses unproved color or stroke caps"));
                        }
                        match &mut candidate {
                            Candidate::Border(b) => {
                                if b.style == PaintStyle::Underline
                                    && state.dash != PaintStyle::Solid
                                {
                                    return Err(bad("dashed underline is not supported"));
                                }
                                if found.is_some() || foreground {
                                    return Err(bad(
                                        "frame paint is ambiguous or follows foreground content",
                                    ));
                                }
                                if !edges.is_empty() {
                                    if edges.len() != 2
                                        || b.style != PaintStyle::Solid
                                        || edges.iter().any(|e| !close(e.width, b.width))
                                    {
                                        return Err(bad(
                                            "compound edges do not match the outer border",
                                        ));
                                    }
                                    b.style = PaintStyle::Compound;
                                }
                                if let Some(bg) = &background {
                                    if !same_shape(&bg.shape, &b.shape) {
                                        return Err(bad("background and border geometries differ"));
                                    }
                                }
                                b.index = i;
                                border_stroke_style = state.dash.clone();
                                found = Some(b.clone());
                            }
                            Candidate::Background(bg) => {
                                if background.is_some()
                                    || found.is_some()
                                    || foreground
                                    || !edges.is_empty()
                                {
                                    return Err(bad(
                                        "background paint is ambiguous or follows content",
                                    ));
                                }
                                bg.index = i;
                                background = Some(bg.clone());
                            }
                            Candidate::Edge(e) => {
                                if found.is_some()
                                    || foreground
                                    || edges.len() != e.side
                                    || edges.len() >= 2
                                {
                                    return Err(bad("ambiguous compound border edge"));
                                }
                                if edges
                                    .first()
                                    .is_some_and(|first| !close(first.width, e.width))
                                {
                                    return Err(bad("compound border edge widths differ"));
                                }
                                e.index = i;
                                edges.push(e.clone());
                            }
                        }
                    } else if op.operator != "n" {
                        foreground = true;
                        absence_safe = false;
                    }
                    if pending_clip {
                        state.clip &= path.identity
                            && state.matrix == IDENTITY
                            && path.rect().is_some_and(|r| same(&r, &rect));
                        pending_clip = false;
                    }
                    path = Path::default();
                }
                "BT" => {
                    nums(op, 0)?;
                    if text {
                        return Err(bad("nested text object"));
                    }
                    text = true;
                }
                "ET" => {
                    nums(op, 0)?;
                    if !text {
                        return Err(bad("unmatched text end"));
                    }
                    text = false;
                    if state.text_clip {
                        state.clip = false;
                    }
                }
                "Tr" => {
                    let n = nums(op, 1)?[0];
                    if !(0. ..=7.).contains(&n) || n.fract() != 0. {
                        return Err(bad("invalid text rendering mode"));
                    }
                    state.text_clip = n >= 4.;
                }
                "Tj" | "TJ" | "'" | "\"" => {
                    if !text {
                        return Err(bad("text paint outside text object"));
                    }
                    foreground = true;
                }
                "Tf" | "Tc" | "Tw" | "Tz" | "TL" | "Ts" | "Td" | "TD" | "Tm" | "T*" => {}
                "Do" | "sh" => {
                    one_name(op)?;
                    foreground = true;
                    absence_safe = false;
                }
                "BMC" | "BDC" => {
                    marks += 1;
                    if marks > 64 {
                        return Err(bad("marked content exceeds 64 levels"));
                    }
                }
                "EMC" => {
                    nums(op, 0)?;
                    marks = marks
                        .checked_sub(1)
                        .ok_or_else(|| bad("unmatched content marker"))?;
                }
                "MP" | "DP" => {}
                // Rendering intent can change output color; flatness can change
                // the shape of a circle. Do not replace those affected frames.
                "ri" | "i" => {
                    state.external = true;
                }
                _ => return Err(bad("unsupported appearance instruction")),
            }
        }
        if !stack.is_empty() || text || marks != 0 || pending_clip || !path.empty() {
            return Err(bad("unfinished appearance state or path"));
        }
        let b = match found {
            Some(b) => b,
            None if allow_absent && !radio && absence_safe && edges.is_empty() => Border {
                index: usize::MAX,
                shape: FrameShape::Rectangle(rect),
                width: 0.,
                color: PaintColor::Gray(0.),
                style: PaintStyle::Invisible,
            },
            None => return Err(bad("a unique editable frame could not be proved")),
        };
        Ok(Self {
            content,
            shape: b.shape,
            border: b.index,
            background: background.as_ref().map(|b| b.index),
            width: b.width,
            color: b.color,
            background_color: background.map(|b| b.color),
            style: b.style,
            stroke_style: border_stroke_style,
            edge_paints: edges.iter().map(|e| e.index).collect(),
            edge_colors: (edges.len() == 2)
                .then(|| [edges[0].color.clone(), edges[1].color.clone()]),
        })
    }
}

fn bad(reason: &str) -> EngineError {
    EngineError::InvalidRequest(format!("Form frame: {reason}"))
}
fn finite(n: f64) -> bool {
    n.is_finite() && n.abs() <= 1_000_000.
}
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 0.0001
}
fn same(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| close(*a, *b))
}
fn nums(op: &Operation, n: usize) -> EngineResult<Vec<f64>> {
    if op.operands.len() != n {
        return Err(bad("invalid numeric operand count"));
    }
    op.operands
        .iter()
        .map(|v| {
            let n = match v {
                Object::Integer(n) => *n as f64,
                Object::Real(n) => f64::from(*n),
                _ => return Err(bad("expected numeric operand")),
            };
            if !finite(n) {
                return Err(bad("nonfinite or excessive operand"));
            }
            Ok(n)
        })
        .collect()
}
fn one_name(op: &Operation) -> EngineResult<()> {
    if op.operands.len() != 1 || op.operands[0].as_name().is_err() {
        Err(bad("expected resource name"))
    } else {
        Ok(())
    }
}
fn dash(op: &Operation) -> EngineResult<PaintStyle> {
    if op.operands.len() != 2 {
        return Err(bad("invalid dash operands"));
    }
    let a = op.operands[0]
        .as_array()
        .map_err(|_| bad("invalid dash array"))?;
    if a.len() > 64 {
        return Err(bad("dash array exceeds 64 values"));
    }
    let n = nums(&Operation::new("", a.clone()), a.len())?;
    let phase = nums(&Operation::new("", vec![op.operands[1].clone()]), 1)?[0];
    if phase < 0. || n.iter().any(|n| *n < 0.) || (!n.is_empty() && n.iter().all(|n| *n == 0.)) {
        return Err(bad("invalid dash values"));
    }
    Ok(if n.is_empty() {
        PaintStyle::Solid
    } else {
        PaintStyle::Dashed(n, phase)
    })
}
const IDENTITY: [f64; 6] = [1., 0., 0., 1., 0., 0.];
#[derive(Clone)]
struct State {
    matrix: [f64; 6],
    width: f64,
    dash: PaintStyle,
    fill: Option<PaintColor>,
    stroke: Option<PaintColor>,
    clip: bool,
    external: bool,
    text_clip: bool,
    cap: u8,
    join: u8,
    miter: f64,
}
impl Default for State {
    fn default() -> Self {
        Self {
            matrix: IDENTITY,
            width: 1.,
            dash: PaintStyle::Solid,
            fill: Some(PaintColor::Gray(0.)),
            stroke: Some(PaintColor::Gray(0.)),
            clip: true,
            external: false,
            text_clip: false,
            cap: 0,
            join: 0,
            miter: 10.,
        }
    }
}
#[derive(Clone)]
struct Segment {
    kind: u8,
    values: [f64; 6],
}
struct Path {
    parts: Vec<Segment>,
    complex: bool,
    identity: bool,
    current: bool,
}
impl Default for Path {
    fn default() -> Self {
        Self {
            parts: Vec::new(),
            complex: false,
            identity: true,
            current: false,
        }
    }
}
impl Path {
    fn empty(&self) -> bool {
        self.parts.is_empty() && !self.complex
    }
    fn add(&mut self, kind: &str, n: &[f64], identity: bool) -> EngineResult<()> {
        if kind != "m" && kind != "re" && !self.current {
            return Err(bad("path has no current point"));
        }
        self.current = true;
        self.identity &= identity;
        if self.parts.len() >= 16 {
            self.complex = true;
            return Ok(());
        }
        let mut values = [0.; 6];
        values[..n.len()].copy_from_slice(n);
        self.parts.push(Segment {
            kind: kind.as_bytes()[0],
            values,
        });
        Ok(())
    }
    fn rect(&self) -> Option<[f64; 4]> {
        if self.complex
            || self.parts.is_empty()
            || self.parts.len() > 2
            || self.parts[0].kind != b'r'
            || (self.parts.len() == 2 && self.parts[1].kind != b'h')
        {
            return None;
        }
        let v = &self.parts[0].values;
        (v[2] > 0. && v[3] > 0.).then_some([v[0], v[1], v[2], v[3]])
    }
    fn ring(&self, rect: [f64; 4]) -> Option<f64> {
        if self.complex || self.parts.len() != 2 || self.parts.iter().any(|p| p.kind != b'r') {
            return None;
        }
        if !same(&self.parts[0].values[..4], &rect) {
            return None;
        }
        let a = &self.parts[1].values;
        let w = a[0] - rect[0];
        (w > 0.
            && a[2] > 0.
            && a[3] > 0.
            && same(
                &[a[1] - rect[1], (rect[2] - a[2]) / 2., (rect[3] - a[3]) / 2.],
                &[w, w, w],
            ))
        .then_some(w)
    }
    fn edge(&self, rect: [f64; 4]) -> Option<(usize, f64)> {
        if self.complex
            || !(self.parts.len() == 6 || (self.parts.len() == 7 && self.parts[6].kind == b'h'))
            || self.parts[0].kind != b'm'
            || self.parts[1..6].iter().any(|p| p.kind != b'l')
        {
            return None;
        }
        let [x, y, w, h] = rect;
        let p = &self.parts[0].values;
        for side in 0..2 {
            let d = if side == 0 { p[0] - x } else { x + w - p[0] };
            if d <= 0. || d * 4. >= w.min(h) {
                continue;
            }
            let points = if side == 0 {
                [
                    [x + d, y + d],
                    [x + d, y + h - d],
                    [x + w - d, y + h - d],
                    [x + w - 2. * d, y + h - 2. * d],
                    [x + 2. * d, y + h - 2. * d],
                    [x + 2. * d, y + 2. * d],
                ]
            } else {
                [
                    [x + w - d, y + h - d],
                    [x + w - d, y + d],
                    [x + d, y + d],
                    [x + 2. * d, y + 2. * d],
                    [x + w - 2. * d, y + 2. * d],
                    [x + w - 2. * d, y + h - 2. * d],
                ]
            };
            if self.parts[..6]
                .iter()
                .zip(points)
                .all(|(p, v)| same(&p.values[..2], &v))
            {
                return Some((side, d));
            }
        }
        None
    }
    fn underline(&self, rect: [f64; 4], width: f64) -> bool {
        if self.complex
            || self.parts.len() != 2
            || self.parts[0].kind != b'm'
            || self.parts[1].kind != b'l'
            || width >= rect[3]
        {
            return false;
        }
        let a = &self.parts[0].values[..2];
        let b = &self.parts[1].values[..2];
        let left = [rect[0], rect[1] + width / 2.];
        let right = [rect[0] + rect[2], rect[1] + width / 2.];
        (same(a, &left) && same(b, &right)) || (same(a, &right) && same(b, &left))
    }
    fn circle(&self) -> Option<[f64; 3]> {
        if self.complex
            || !(self.parts.len() == 5 || (self.parts.len() == 6 && self.parts[5].kind == b'h'))
            || self.parts[0].kind != b'm'
            || self.parts[1..5].iter().any(|p| p.kind != b'c')
        {
            return None;
        }
        let mut points = [[0.; 2]; 5];
        points[0].copy_from_slice(&self.parts[0].values[..2]);
        for i in 0..4 {
            points[i + 1].copy_from_slice(&self.parts[i + 1].values[4..6]);
        }
        if !same(&points[0], &points[4]) {
            return None;
        }
        let cx = points[..4].iter().map(|p| p[0]).sum::<f64>() / 4.;
        let cy = points[..4].iter().map(|p| p[1]).sum::<f64>() / 4.;
        let r = (points[0][0] - cx).hypot(points[0][1] - cy);
        if r <= 0. || !finite(r) {
            return None;
        }
        for i in 0..4 {
            let a = [points[i][0] - cx, points[i][1] - cy];
            let b = [points[i + 1][0] - cx, points[i + 1][1] - cy];
            if !close(a[0].hypot(a[1]), r)
                || !(close(a[0], 0.) || close(a[1], 0.))
                || !close(a[0] * b[0] + a[1] * b[1], 0.)
            {
                return None;
            }
            let k = 0.5522848;
            if !same(
                &self.parts[i + 1].values[..4],
                &[
                    points[i][0] + k * b[0],
                    points[i][1] + k * b[1],
                    points[i + 1][0] + k * a[0],
                    points[i + 1][1] + k * a[1],
                ],
            ) {
                return None;
            }
        }
        Some([cx, cy, r])
    }
}
#[derive(Clone)]
struct Border {
    index: usize,
    shape: FrameShape,
    width: f64,
    color: PaintColor,
    style: PaintStyle,
}
#[derive(Clone)]
struct Background {
    index: usize,
    shape: FrameShape,
    color: PaintColor,
}
#[derive(Clone)]
struct Edge {
    index: usize,
    side: usize,
    width: f64,
    color: PaintColor,
}
enum Candidate {
    Border(Border),
    Background(Background),
    Edge(Edge),
}
fn same_shape(a: &FrameShape, b: &FrameShape) -> bool {
    match (a, b) {
        (FrameShape::Rectangle(a), FrameShape::Rectangle(b)) => same(a, b),
        (FrameShape::Circle(a), FrameShape::Circle(b)) => same(a, b),
        _ => false,
    }
}
fn classify(
    path: &Path,
    paint: &str,
    state: &State,
    rect: [f64; 4],
    radio: bool,
) -> Option<Candidate> {
    // Non-painting geometry keeps a removed border reversible. It is still
    // checked as a canonical, bounded path with known state and unique order;
    // no private marker can authorize geometry or delete visible artwork.
    if paint == "n" && state.width == 0. {
        let shape = if radio {
            path.circle().and_then(|c| {
                (c[0] - c[2] >= rect[0]
                    && c[1] - c[2] >= rect[1]
                    && c[0] + c[2] <= rect[0] + rect[2]
                    && c[1] + c[2] <= rect[1] + rect[3])
                    .then_some(FrameShape::Circle(c))
            })
        } else {
            path.rect()
                .filter(|r| same(r, &rect))
                .map(FrameShape::Rectangle)
        };
        if let Some(shape) = shape {
            return Some(Candidate::Border(Border {
                index: 0,
                shape,
                width: 0.,
                color: state.fill.clone().unwrap_or(PaintColor::Gray(0.)),
                style: PaintStyle::Invisible,
            }));
        }
    }
    if matches!(paint, "f" | "F" | "f*") {
        if !radio {
            if let Some((side, width)) = path.edge(rect) {
                return Some(Candidate::Edge(Edge {
                    index: 0,
                    side,
                    width,
                    color: state.fill.clone().unwrap_or(PaintColor::Gray(0.)),
                }));
            }
        }
        if !radio && paint == "f*" {
            if let Some(width) = path.ring(rect) {
                return Some(Candidate::Border(Border {
                    index: 0,
                    shape: FrameShape::Rectangle(rect),
                    width,
                    color: state.fill.clone().unwrap_or(PaintColor::Gray(0.)),
                    style: PaintStyle::Solid,
                }));
            }
        }
        let shape = if radio {
            path.circle().map(FrameShape::Circle)
        } else {
            path.rect()
                .filter(|r| same(r, &rect))
                .map(FrameShape::Rectangle)
        };
        if let Some(shape) = shape {
            // A radio's inner selected dot is answer paint, never its background.
            // It is classified later against the outer border geometry/order.
            return Some(Candidate::Background(Background {
                index: 0,
                shape,
                color: state.fill.clone().unwrap_or(PaintColor::Gray(0.)),
            }));
        }
    }
    if matches!(paint, "S" | "s") && state.width > 0. {
        if !radio && path.underline(rect, state.width) {
            return Some(Candidate::Border(Border {
                index: 0,
                shape: FrameShape::Rectangle(rect),
                width: state.width,
                color: state.stroke.clone().unwrap_or(PaintColor::Gray(0.)),
                style: PaintStyle::Underline,
            }));
        }
        let shape = if radio {
            path.circle().and_then(|c| {
                let r = c[2] + state.width / 2.;
                (c[0] - r >= rect[0] - 0.0001
                    && c[1] - r >= rect[1] - 0.0001
                    && c[0] + r <= rect[0] + rect[2] + 0.0001
                    && c[1] + r <= rect[1] + rect[3] + 0.0001)
                    .then_some(FrameShape::Circle([c[0], c[1], r]))
            })
        } else if state.join == 0 && state.miter >= 2. {
            path.rect().and_then(|r| {
                let outer = [
                    r[0] - state.width / 2.,
                    r[1] - state.width / 2.,
                    r[2] + state.width,
                    r[3] + state.width,
                ];
                same(&outer, &rect).then_some(FrameShape::Rectangle(rect))
            })
        } else {
            None
        };
        if let Some(shape) = shape {
            return Some(Candidate::Border(Border {
                index: 0,
                shape,
                width: state.width,
                color: state.stroke.clone().unwrap_or(PaintColor::Gray(0.)),
                style: state.dash.clone(),
            }));
        }
    }
    None
}

#[cfg(test)]
#[path = "form_frame_paint_tests.rs"]
mod tests;
