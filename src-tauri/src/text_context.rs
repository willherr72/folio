//! Source-derived clipping and marked-content context. This module does not
//! authorize edits: callers must also prove native object mapping and tag ownership.
use super::{invalid, positioned_text, EngineResult};
use lopdf::{
    content::{Content, Operation},
    Object,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;

const MAX_OPS: usize = 100_000;
const MAX_BYTES: usize = 16 * 1024 * 1024;
type Matrix = [f64; 6];
type Bounds = [f64; 4];
const IDENTITY: Matrix = [1., 0., 0., 1., 0., 0.];

// Flat arenas share histories without recursive Arc destruction or copying a
// growing transform/clip history into every native text object.
struct Transform {
    parent: Option<usize>,
    operation: Operation,
}
struct Recipe {
    parent: Option<usize>,
    transform: Option<usize>,
    rectangle: Operation,
    even_odd: bool,
}
struct Mark {
    parent: Option<usize>,
    opening: Operation,
    admitted: bool,
}
#[derive(Default)]
struct Arena {
    transforms: Vec<Transform>,
    clips: Vec<Recipe>,
    marks: Vec<Mark>,
}

#[derive(Clone, Copy)]
enum Clip {
    Unbounded,
    Rectangle(Bounds, usize),
    Empty,
    Unknown,
}
#[derive(Clone, Copy)]
struct State {
    ctm: Matrix,
    transform: Option<usize>,
    clip: Clip,
    mode: u8,
}
impl Default for State {
    fn default() -> Self {
        Self {
            ctm: IDENTITY,
            transform: None,
            clip: Clip::Unbounded,
            mode: 0,
        }
    }
}
enum Path {
    None,
    Rectangle {
        bounds: Bounds,
        transform: Option<usize>,
        operation: Operation,
    },
    Unknown,
}
#[derive(Clone)]
pub(super) struct ShowContext {
    clip: Clip,
    mark: Option<usize>,
    clipping_text: bool,
    arena: Arc<Arena>,
}
pub(super) struct PageContext {
    pub shows: Vec<(usize, ShowContext)>,
    pub marked_events: Vec<Operation>,
}

fn malformed() -> super::EngineError {
    invalid("The text clipping or marked-content context is malformed or exceeds its limits.")
}
fn number(value: &Object) -> EngineResult<f64> {
    let n = match value {
        Object::Integer(n) => *n as f64,
        Object::Real(n) => *n as f64,
        _ => return Err(malformed()),
    };
    if !n.is_finite() || n.abs() > 1_000_000. {
        return Err(malformed());
    }
    Ok(n)
}
fn numbers<const N: usize>(op: &Operation) -> EngineResult<[f64; N]> {
    if op.operands.len() != N {
        return Err(malformed());
    }
    let mut result = [0.; N];
    for (out, value) in result.iter_mut().zip(&op.operands) {
        *out = number(value)?;
    }
    Ok(result)
}
fn no_operands(op: &Operation) -> EngineResult<()> {
    if op.operands.is_empty() {
        Ok(())
    } else {
        Err(malformed())
    }
}
fn product(a: Matrix, b: Matrix) -> EngineResult<Matrix> {
    let m = [
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ];
    if m.iter().any(|n| !n.is_finite() || n.abs() > 1_000_000_000.) {
        return Err(malformed());
    }
    Ok(m)
}

// Validate before cloning/encoding any operand. The conservative serialized
// upper bound includes escaping and prevents a small op count hiding huge data.
fn budget(operations: &[Operation]) -> EngineResult<()> {
    if operations.len() > MAX_OPS {
        return Err(malformed());
    }
    let mut used = 0usize;
    let mut values = 0usize;
    for op in operations {
        used = used
            .checked_add(op.operator.len() + 1)
            .ok_or_else(malformed)?;
        if op.operands.len() > MAX_OPS {
            return Err(malformed());
        }
        let mut pending: Vec<_> = op.operands.iter().map(|v| (v, 0usize)).collect();
        while let Some((value, depth)) = pending.pop() {
            values += 1;
            if depth > 64 || values > 1_000_000 {
                return Err(malformed());
            }
            let extra = match value {
                Object::String(bytes, _) | Object::Name(bytes) => {
                    bytes.len().checked_mul(4).ok_or_else(malformed)?
                }
                Object::Real(n) if !n.is_finite() => return Err(malformed()),
                Object::Array(items) => {
                    if items.len() > MAX_OPS {
                        return Err(malformed());
                    }
                    pending.extend(items.iter().map(|v| (v, depth + 1)));
                    0
                }
                Object::Dictionary(dict) => {
                    if dict.len() > MAX_OPS {
                        return Err(malformed());
                    }
                    let mut keys = 0usize;
                    for (key, value) in dict.iter() {
                        keys = keys
                            .checked_add(key.len().checked_mul(4).ok_or_else(malformed)?)
                            .ok_or_else(malformed)?;
                        pending.push((value, depth + 1));
                    }
                    keys
                }
                Object::Stream(_) => return Err(malformed()),
                _ => 0,
            };
            used = used
                .checked_add(32)
                .and_then(|n| n.checked_add(extra))
                .ok_or_else(malformed)?;
            if used > MAX_BYTES || pending.len() > MAX_OPS {
                return Err(malformed());
            }
        }
        if used > MAX_BYTES {
            return Err(malformed());
        }
    }
    Ok(())
}

fn rectangle(state: State, op: &Operation) -> EngineResult<Path> {
    let [x, y, w, h] = numbers::<4>(op)?;
    let m = state.ctm;
    if !((m[1] == 0. && m[2] == 0.) || (m[0] == 0. && m[3] == 0.)) {
        return Ok(Path::Unknown);
    }
    let a = [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]];
    let b = [
        m[0] * (x + w) + m[2] * (y + h) + m[4],
        m[1] * (x + w) + m[3] * (y + h) + m[5],
    ];
    let bounds = [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[0].max(b[0]),
        a[1].max(b[1]),
    ];
    if bounds
        .iter()
        .any(|n| !n.is_finite() || n.abs() > 1_000_000_000.)
    {
        return Err(malformed());
    }
    Ok(Path::Rectangle {
        bounds,
        transform: state.transform,
        operation: op.clone(),
    })
}
fn apply_clip(state: &mut State, path: &Path, even_odd: bool, arena: &mut Arena) {
    let Path::Rectangle {
        bounds,
        transform,
        operation,
    } = path
    else {
        state.clip = Clip::Unknown;
        return;
    };
    let (mut bounds, parent) = (
        *bounds,
        match state.clip {
            Clip::Rectangle(_, head) => Some(head),
            _ => None,
        },
    );
    match state.clip {
        Clip::Unknown | Clip::Empty => return,
        Clip::Rectangle(old, _) => {
            bounds = [
                bounds[0].max(old[0]),
                bounds[1].max(old[1]),
                bounds[2].min(old[2]),
                bounds[3].min(old[3]),
            ];
        }
        Clip::Unbounded => {}
    }
    if bounds[0] >= bounds[2] || bounds[1] >= bounds[3] {
        state.clip = Clip::Empty;
        return;
    }
    let head = arena.clips.len();
    arena.clips.push(Recipe {
        parent,
        transform: *transform,
        rectangle: operation.clone(),
        even_odd,
    });
    state.clip = Clip::Rectangle(bounds, head);
}
fn admitted_mark(op: &Operation) -> bool {
    let Some(Object::Name(tag)) = op.operands.first() else {
        return false;
    };
    if tag.is_empty() || tag.len() > 128 || tag == b"OC" {
        return false;
    }
    if op.operator == "BMC" {
        return op.operands.len() == 1;
    }
    let [_, Object::Dictionary(dict)] = op.operands.as_slice() else {
        return false;
    };
    dict.get(b"MCID")
        .and_then(Object::as_i64)
        .is_ok_and(|id| (0..=65_535).contains(&id))
        && dict.iter().all(|(key, value)| match key.as_slice() {
            b"MCID" => true,
            b"Lang" => value
                .as_str()
                .is_ok_and(|s| !s.is_empty() && s.len() <= 256),
            _ => false,
        })
}
fn show(op: &Operation) -> EngineResult<bool> {
    match (op.operator.as_str(), op.operands.as_slice()) {
        ("Tj" | "'", [Object::String(..)]) => {}
        ("\"", [word, character, Object::String(..)]) => {
            number(word)?;
            number(character)?;
        }
        ("TJ", [Object::Array(values)]) => {
            for value in values {
                if !matches!(value, Object::String(..)) {
                    number(value)?;
                }
            }
        }
        _ => return Err(malformed()),
    }
    Ok(!positioned_text::cursor_only(op))
}

pub(super) fn inspect(operations: &[Operation]) -> EngineResult<PageContext> {
    budget(operations)?;
    let mut arena = Arena::default();
    let mut state = State::default();
    let mut graphics = Vec::new();
    let mut path = Path::None;
    let mut pending_clip = None;
    let mut mark = None;
    let mut mark_depth = 0;
    let mut in_text = false;
    let mut pending_text_clip = false;
    let mut shows = Vec::new();
    let mut marked_events = Vec::new();
    for (index, op) in operations.iter().enumerate() {
        match op.operator.as_str() {
            "q" => {
                no_operands(op)?;
                if graphics.len() >= 64 {
                    return Err(malformed());
                }
                graphics.push(state);
            }
            "Q" => {
                no_operands(op)?;
                state = graphics.pop().ok_or_else(malformed)?;
            }
            "cm" => {
                state.ctm = product(state.ctm, numbers::<6>(op)?)?;
                let head = arena.transforms.len();
                arena.transforms.push(Transform {
                    parent: state.transform,
                    operation: op.clone(),
                });
                state.transform = Some(head);
            }
            "re" => {
                if pending_clip.is_some() {
                    return Err(malformed());
                }
                let next = rectangle(state, op)?;
                path = if matches!(path, Path::None) {
                    next
                } else {
                    Path::Unknown
                };
            }
            "m" | "l" | "c" | "v" | "y" | "h" => {
                if pending_clip.is_some() {
                    return Err(malformed());
                }
                match op.operator.as_str() {
                    "m" | "l" => {
                        numbers::<2>(op)?;
                    }
                    "c" => {
                        numbers::<6>(op)?;
                    }
                    "v" | "y" => {
                        numbers::<4>(op)?;
                    }
                    _ => no_operands(op)?,
                }
                path = Path::Unknown;
            }
            "W" | "W*" => {
                no_operands(op)?;
                if pending_clip.is_some() || matches!(path, Path::None) {
                    return Err(malformed());
                }
                pending_clip = Some(op.operator == "W*");
            }
            "n" | "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                no_operands(op)?;
                if let Some(rule) = pending_clip.take() {
                    apply_clip(&mut state, &path, rule, &mut arena);
                }
                path = Path::None;
            }
            "BT" => {
                no_operands(op)?;
                if in_text {
                    return Err(malformed());
                }
                in_text = true;
                pending_text_clip = false;
            }
            "ET" => {
                no_operands(op)?;
                if !in_text {
                    return Err(malformed());
                }
                in_text = false;
                if pending_text_clip {
                    state.clip = Clip::Unknown;
                }
                pending_text_clip = false;
            }
            "Tr" => {
                let [value] = numbers::<1>(op)?;
                if !(0. ..=7.).contains(&value) || value.fract() != 0. {
                    return Err(malformed());
                }
                state.mode = value as u8;
            }
            "Tj" | "TJ" | "'" | "\"" => {
                if !in_text {
                    return Err(malformed());
                }
                if show(op)? {
                    shows.push((index, state.clip, mark, state.mode >= 4));
                    pending_text_clip |= state.mode >= 4;
                }
            }
            "BMC" | "BDC" => {
                if mark_depth >= 64 || op.operands.len() != if op.operator == "BMC" { 1 } else { 2 }
                {
                    return Err(malformed());
                }
                let head = arena.marks.len();
                arena.marks.push(Mark {
                    parent: mark,
                    opening: op.clone(),
                    admitted: admitted_mark(op),
                });
                mark = Some(head);
                mark_depth += 1;
                marked_events.push(op.clone());
            }
            "EMC" => {
                no_operands(op)?;
                let head = mark.ok_or_else(malformed)?;
                mark = arena.marks[head].parent;
                mark_depth -= 1;
                marked_events.push(op.clone());
            }
            // These operators cannot modify the current path, clip, CTM or mark
            // stack. Existing resource/font/pattern preflight remains mandatory.
            "w" | "J" | "j" | "M" | "d" | "ri" | "i" | "gs" | "CS" | "cs" | "SC" | "SCN" | "sc"
            | "scn" | "G" | "g" | "RG" | "rg" | "K" | "k" | "sh" | "Do" | "Tc" | "Tw" | "Tz"
            | "TL" | "Tf" | "Ts" | "Td" | "TD" | "Tm" | "T*" | "MP" | "DP" | "BX" | "EX" | "d0"
            | "d1" => {}
            _ => return Err(malformed()),
        }
    }
    if !graphics.is_empty() || mark.is_some() || in_text || pending_clip.is_some() {
        return Err(malformed());
    }
    let arena = Arc::new(arena);
    Ok(PageContext {
        shows: shows
            .into_iter()
            .map(|(i, clip, mark, clipping_text)| {
                (
                    i,
                    ShowContext {
                        clip,
                        mark,
                        clipping_text,
                        arena: arena.clone(),
                    },
                )
            })
            .collect(),
        marked_events,
    })
}
impl ShowContext {
    pub(super) fn requires_binding(&self)->bool {self.mark.is_some() || !matches!(self.clip,Clip::Unbounded) || self.clipping_text}
    pub(super) fn has_marked(&self)->bool {self.mark.is_some()}
    pub(super) fn has_clip(&self)->bool {!matches!(self.clip,Clip::Unbounded)}

    pub(super) fn mcid(&self) -> EngineResult<Option<i64>> {
        if !self.admitted() {
            return Err(invalid(
                "This text has unsupported clipping or marked-content semantics.",
            ));
        }
        let mut result = None;
        let mut mark = self.mark;
        while let Some(head) = mark {
            let node = &self.arena.marks[head];
            if let [_, Object::Dictionary(dict)] = node.opening.operands.as_slice() {
                let id = dict
                    .get(b"MCID")
                    .and_then(Object::as_i64)
                    .map_err(|_| malformed())?;
                if result.replace(id).is_some() {
                    return Err(invalid(
                        "Nested semantic text owners cannot be edited safely.",
                    ));
                }
            }
            mark = node.parent;
        }
        Ok(result)
    }
    fn admitted(&self) -> bool {
        if self.clipping_text || matches!(self.clip, Clip::Empty | Clip::Unknown) {
            return false;
        }
        let mut mark = self.mark;
        while let Some(head) = mark {
            let node = &self.arena.marks[head];
            if !node.admitted {
                return false;
            }
            mark = node.parent;
        }
        true
    }
    pub(super) fn contains(&self, bounds: Bounds) -> bool {
        if !self.admitted()
            || bounds.iter().any(|n| !n.is_finite())
            || bounds[0] > bounds[2]
            || bounds[1] > bounds[3]
        {
            return false;
        }
        match self.clip {
            Clip::Unbounded => true,
            Clip::Rectangle(c, _) => {
                bounds[0] >= c[0] && bounds[1] >= c[1] && bounds[2] <= c[2] && bounds[3] <= c[3]
            }
            _ => false,
        }
    }
    pub(super) fn binding(&self) -> EngineResult<String> {
        if !self.admitted() {
            return Err(invalid(
                "This text has unsupported clipping or marked-content semantics.",
            ));
        }
        self.preserved_binding()
    }
    // Equality evidence for an UNSELECTED show only. Opaque marks may be kept
    // byte-for-byte without granting authority to edit their semantics. Selected
    // shows still go through binding(), contains(), and original tag ownership.
    pub(super) fn preserved_binding(&self) -> EngineResult<String> {
        if self.clipping_text || matches!(self.clip, Clip::Empty | Clip::Unknown) {
            return Err(invalid("The unchanged text clipping context cannot be compared safely."));
        }
        let mut hash = Sha256::new();
        hash.update(b"Folio text context v1\0");
        match self.clip {
            Clip::Unbounded => hash.update([0]),
            Clip::Rectangle(bounds, _) => {
                hash.update([1]);
                for n in bounds {
                    hash.update((if n == 0. { 0f64 } else { n }).to_le_bytes());
                }
            }
            _ => unreachable!(),
        }
        let mut mark = self.mark;
        while let Some(head) = mark {
            let node = &self.arena.marks[head];
            hash.update((head as u64).to_le_bytes());
            let bytes = Content {
                operations: std::slice::from_ref(&node.opening),
            }
            .encode()
            .map_err(|_| malformed())?;
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
            mark = node.parent;
        }
        Ok(hash
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }
    pub(super) fn clip_prefix(&self) -> EngineResult<Vec<Operation>> {
        if !self.admitted() {
            return Err(invalid("This text clip cannot be reconstructed."));
        }
        let Clip::Rectangle(bounds, head) = self.clip else {
            return Ok(vec![Operation::new("q", vec![])]);
        };
        let mut nodes = Vec::new();
        let mut next = Some(head);
        while let Some(index) = next {
            nodes.push(index);
            next = self.arena.clips[index].parent;
        }
        // Preserve the established envelope for ordinary untransformed clips.
        if nodes.iter().all(|&index| self.arena.clips[index].transform.is_none()) {
            return self.legacy_clip_prefix();
        }
        let mut output = vec![Operation::new("q", vec![])];
        let mut applied = None;
        let mut ctm = IDENTITY;
        for index in nodes.into_iter().rev() {
            let recipe = &self.arena.clips[index];
            let mut transforms = Vec::new();
            let mut next = recipe.transform;
            while next != applied {
                let Some(index) = next else {
                    // Paths surviving graphics restoration can have disjoint
                    // histories. Flatten only if the exact clip is representable.
                    return self.exact_rectangle_clip_prefix(bounds);
                };
                if output.len() + transforms.len() + 4 > MAX_OPS { return Err(malformed()); }
                transforms.push(index);
                next = self.arena.transforms[index].parent;
            }
            for index in transforms.into_iter().rev() {
                let op = &self.arena.transforms[index].operation;
                ctm = product(ctm, numbers::<6>(op)?)?;
                output.push(op.clone());
            }
            applied = recipe.transform;
            output.push(recipe.rectangle.clone());
            // Apply the path while its construction CTM is still active.
            // Restoring CTM first loses this clip in independent PDF readers.
            output.push(Operation::new(if recipe.even_odd { "W*" } else { "W" }, vec![]));
            output.push(Operation::new("n", vec![]));
        }
        if ctm != IDENTITY {
            let determinant = ctm[0]*ctm[3]-ctm[1]*ctm[2];
            if determinant == 0. || !determinant.is_finite() { return Err(malformed()); }
            let inverse = [ctm[3]/determinant, -ctm[1]/determinant,
                -ctm[2]/determinant, ctm[0]/determinant,
                (ctm[2]*ctm[5]-ctm[3]*ctm[4])/determinant,
                (ctm[1]*ctm[4]-ctm[0]*ctm[5])/determinant];
            let inverse = Operation::new("cm", inverse.into_iter().map(|v| {
                let value = v as f32;
                // The content serializer writes integral reals without a decimal
                // point. Use the same type the parser restores, retaining exact
                // envelope ownership after save/reopen and preview self-replay.
                if value.is_finite() && value.fract() == 0. && value.abs() <= 1_000_000. {
                    Object::Integer(value as i64)
                } else { Object::Real(value) }
            }).collect());
            let restored = product(ctm, numbers::<6>(&inverse)?)?;
            // Only the replacement coordinate system is approximate. Every
            // clip above retains the exact source transform and binding. Bound
            // inverse rounding across the clip; publication also checks ink.
            for x in [bounds[0], bounds[2]] { for y in [bounds[1], bounds[3]] {
                let dx = (restored[0]-1.)*x + restored[2]*y + restored[4];
                let dy = restored[1]*x + (restored[3]-1.)*y + restored[5];
                if dx.abs() > 0.001 || dy.abs() > 0.001 {
                    return Err(invalid("The paragraph clip transform cannot retain precise placement."));
                }
            }}
            output.push(inverse);
        }
        budget(&output)?;
        Ok(output)
    }

    fn exact_rectangle_clip_prefix(&self, bounds: Bounds) -> EngineResult<Vec<Operation>> {
        let operands = [bounds[0], bounds[1], bounds[2]-bounds[0], bounds[3]-bounds[1]];
        let op = Operation::new("re", operands.into_iter().map(|n| {
            let value = n as f32;
            if value.is_finite() && value.fract() == 0. && value.abs() <= 1_000_000. {
                Object::Integer(value as i64)
            } else { Object::Real(value) }
        }).collect());
        if !matches!(rectangle(State::default(), &op)?, Path::Rectangle{bounds: actual, ..} if actual == bounds) {
            return Err(invalid("The restored paragraph clip cannot be represented exactly."));
        }
        // Match the established untransformed envelope, so its next re-edit is
        // recognized identically after the exact rectangle is parsed again.
        Ok(vec![Operation::new("q", vec![]), Operation::new("q", vec![]), op,
            Operation::new("Q", vec![]), Operation::new("W", vec![]), Operation::new("n", vec![])])
    }

    // Recognition only for valid envelopes emitted by earlier versions. New
    // transformed output must use clip_prefix(), never this historical recipe.
    fn legacy_clip_prefix(&self) -> EngineResult<Vec<Operation>> {
        if !self.admitted() {
            return Err(invalid("This text clip cannot be reconstructed."));
        }
        let mut nodes = Vec::new();
        let mut head = match self.clip {
            Clip::Rectangle(_, head) => Some(head),
            _ => None,
        };
        while let Some(index) = head {
            nodes.push(index);
            head = self.arena.clips[index].parent;
        }
        let mut output = vec![Operation::new("q", vec![])];
        for index in nodes.into_iter().rev() {
            let recipe = &self.arena.clips[index];
            let mut transforms = Vec::new();
            let mut head = recipe.transform;
            while let Some(index) = head {
                if output.len() + transforms.len() + 6 > MAX_OPS {
                    return Err(malformed());
                }
                transforms.push(index);
                head = self.arena.transforms[index].parent;
            }
            if output.len() + transforms.len() + 6 > MAX_OPS {
                return Err(malformed());
            }
            output.push(Operation::new("q", vec![]));
            for index in transforms.into_iter().rev() {
                output.push(self.arena.transforms[index].operation.clone());
            }
            output.push(recipe.rectangle.clone());
            output.push(Operation::new("Q", vec![]));
            output.push(Operation::new(
                if recipe.even_odd { "W*" } else { "W" },
                vec![],
            ));
            output.push(Operation::new("n", vec![]));
        }
        budget(&output)?;
        Ok(output)
    }

    pub(super) fn owned_clip_prefix_start(&self, operations: &[Operation], end: usize) -> EngineResult<usize> {
        for legacy in [false, true] {
            let prefix = if legacy { self.legacy_clip_prefix()? } else { self.clip_prefix()? };
            if let Some(start) = end.checked_sub(prefix.len()) {
                if operations.get(start..end).is_some_and(|ops| ops.iter().zip(&prefix)
                    .all(|(a,b)| a.operator == b.operator && a.operands == b.operands)) {
                    return Ok(start);
                }
            }
        }
        Err(invalid("The saved paragraph clip envelope is not independently owned."))
    }
}

#[cfg(test)]
#[path = "text_context_tests.rs"]
mod tests;
