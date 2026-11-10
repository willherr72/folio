//! Bounded answer isolation; publication and native verification are separate.
#![allow(dead_code)] // Staged before preserving fill publication.
use crate::{EngineError, EngineResult};
use lopdf::{
    content::{Content, Operation},
    Object,
};
use std::ops::Range;

#[path = "form_answer_import.rs"]
mod import;
#[path = "form_answer_transplant.rs"]
mod transplant;
#[path = "form_answer_compound.rs"]
mod compound;
pub(crate) use import::NormalAppearanceAdmission;
pub(crate) use transplant::{
    retained_resources_are_local, transplant_answer_with_resources, FillAppearanceBudget,
    StagedAnswerAppearance,
};

const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_OPS: usize = 262144;
#[derive(Clone, Copy)]
pub(crate) enum AnswerKind {
    Text,
    Combo,
    List,
}
pub(crate) struct AnswerSlot {
    content: Content,
    region: Range<usize>,
    marked: bool,
    entry: Frame,
    answer_clip: Option<usize>,
    clips: Vec<ClipNode>,
}
fn bad(reason: &str) -> EngineError {
    EngineError::InvalidRequest(format!("Form answer appearance: {reason}"))
}
impl AnswerSlot {
    fn parse_empty_source(bytes: &[u8], kind: AnswerKind, bounds: [f64; 4]) -> EngineResult<Self> {
        let mut slot = Self::parse_empty_generated(bytes)?;
        if !matches!(kind, AnswerKind::Text) {
            choice_background(&slot.content.operations, &slot.region, bounds)?;
        }
        // With a verified empty value and no glyph/nested paint, the vectors
        // are static artwork. Give the first answer a persistent insertion slot.
        let start = slot.content.operations.len();
        slot.content.operations.extend([
            Operation::new("BMC", vec![Object::Name(b"Tx".to_vec())]),
            Operation::new("q", vec![]),
            Operation::new("Q", vec![]),
            Operation::new("EMC", vec![]),
        ]);
        slot.region = start..slot.content.operations.len();
        validate(&slot.content.operations, &slot.region, true, kind, true)?;
        (slot.entry, slot.answer_clip, slot.clips) =
            capture(&slot.content.operations, &slot.region)?;
        Ok(slot)
    }
    fn parse_empty_generated(bytes: &[u8]) -> EngineResult<Self> {
        preflight(bytes)?;
        let content = Content::decode_strict(bytes)
            .map_err(|_| bad("malformed empty generated appearance"))?;
        if content.operations.iter().any(|op| {
            matches!(
                op.operator.as_str(),
                "Tj" | "TJ" | "'" | "\"" | "Do" | "sh" | "BMC" | "BDC" | "EMC"
            )
        }) {
            return Err(bad(
                "empty generated answer still contains unidentified paint",
            ));
        }
        let region = content.operations.len()..content.operations.len();
        validate(&content.operations, &region, false, AnswerKind::Text, false)?;
        // A generated empty answer has no glyph/selection island. Preserve only
        // its non-painting setup; the original slot supplies the insertion layer.
        Ok(Self {
            content,
            region,
            marked: true,
            entry: Frame {
                matrix: IDENTITY,
                clip: None,
            },
            answer_clip: None,
            clips: Vec::new(),
        })
    }
    pub(crate) fn parse(bytes: &[u8], kind: AnswerKind) -> EngineResult<Self> {
        Self::parse_bounds(bytes, kind, None)
    }
    /// Whole-appearance regeneration may discard only the proved answer text.
    /// Unlike answer transplant, it cannot retain captions outside that slot.
    pub(crate) fn require_all_text_in_answer(&self) -> EngineResult<()> {
        if self.content.operations.iter().enumerate().any(|(i, op)| {
            !self.region.contains(&i) && matches!(op.operator.as_str(), "Tj" | "TJ" | "'" | "\"")
        }) {
            return Err(bad("saved caption text is outside the answer region"));
        }
        Ok(())
    }
    fn parse_bounds(
        bytes: &[u8],
        kind: AnswerKind,
        bounds: Option<[f64; 4]>,
    ) -> EngineResult<Self> {
        preflight(bytes)?;
        let mut content =
            Content::decode_strict(bytes).map_err(|_| bad("malformed appearance instructions"))?;
        let ops = &content.operations;
        if ops.len() > MAX_OPS {
            return Err(bad("too many appearance instructions"));
        }
        let mut marked_start = None;
        let mut marked_region = None;
        let mut text_start = None;
        let mut text_regions = Vec::new();
        for (i, op) in ops.iter().enumerate() {
            match op.operator.as_str() {
                "BMC" => {
                    if marked_start.is_some() || marked_region.is_some() || name(op)? != b"Tx" {
                        return Err(bad("ambiguous marked answer"));
                    }
                    marked_start = Some(i);
                }
                "EMC" => {
                    numbers(op, 0)?;
                    let start = marked_start
                        .take()
                        .ok_or_else(|| bad("unmatched answer marker"))?;
                    marked_region = Some(start..i + 1);
                }
                "BT" => {
                    if text_start.replace(i).is_some() {
                        return Err(bad("nested text object"));
                    }
                }
                "ET" => text_regions.push(
                    text_start
                        .take()
                        .ok_or_else(|| bad("unmatched text object"))?..i + 1,
                ),
                _ => {}
            }
        }
        if marked_start.is_some() || text_start.is_some() {
            return Err(bad("unclosed answer or text object"));
        }
        let marked = marked_region.is_some();
        let mut region = match marked_region {
            Some(region) => region,
            None if text_regions.len() == 1 => text_regions.remove(0),
            _ => return Err(bad("the answer cannot be distinguished from other text")),
        };
        validate(ops, &region, marked, kind, bounds.is_some())?;
        if marked {
            if let Some((normalized, answer)) = compound::normalize(ops, &region, kind, bounds)? {
                content = normalized;
                region = answer;
                validate(&content.operations, &region, true, kind, bounds.is_some())?;
            }
        }
        let ops = &content.operations;
        if !marked && !matches!(kind, AnswerKind::Text) {
            if let Some(bounds) = bounds {
                choice_background(ops, &region, bounds)?;
            }
        }
        let (entry, answer_clip, clips) = capture(ops, &region)?;
        Ok(Self {
            content,
            region,
            marked,
            entry,
            answer_clip,
            clips,
        })
    }
    fn splice(&self, invocation: &[Operation]) -> EngineResult<Vec<u8>> {
        encode_bounded(&self.splice_operations(invocation)?)
    }
    fn splice_operations(&self, invocation: &[Operation]) -> EngineResult<Vec<Operation>> {
        let mut retained = crate::form_graph::CopyBudget::with_byte_limit(64 * 1024 * 1024);
        for op in self.content.operations.iter().chain(invocation) {
            retained.array(&op.operands)?;
        }
        let mut output = Vec::new();
        output.extend_from_slice(&self.content.operations[..self.region.start]);
        output.push(Operation::new("BMC", vec![Object::Name(b"Tx".to_vec())]));
        output.push(Operation::new("q", vec![]));
        output.push(transform(inverse(self.entry.matrix)?));
        // The inherited clip already exists at invocation. Replay only the
        // additional clips used by the old answer, in appearance coordinates.
        let mut chain = Vec::new();
        let mut clip = self.answer_clip;
        while clip != self.entry.clip {
            if chain.len() >= 64 {
                return Err(bad("answer clip nesting exceeds 64 levels"));
            }
            let id = clip.ok_or_else(|| bad("answer escapes its inherited clip"))?;
            chain.push(id);
            clip = self.clips[id].parent;
        }
        for id in chain.into_iter().rev() {
            let clip = &self.clips[id];
            output.push(transform(clip.matrix));
            for op in &self.content.operations[clip.path.clone()] {
                if matches!(
                    op.operator.as_str(),
                    "m" | "l" | "c" | "v" | "y" | "h" | "re"
                ) {
                    output.push(op.clone());
                }
            }
            output.push(Operation::new(if clip.even { "W*" } else { "W" }, vec![]));
            output.push(Operation::new("n", vec![]));
            output.push(transform(inverse(clip.matrix)?));
        }
        output.extend_from_slice(invocation);
        output.push(Operation::new("Q", vec![]));
        output.push(Operation::new("EMC", vec![]));
        if !self.marked {
            // BT does not save the graphics state. Keep color/text-state changes
            // for later artwork, but remove glyph paint and text positioning.
            // Quote operators also change Tw/Tc; preserve those side effects.
            retain_text_state(&self.content.operations[self.region.clone()], &mut output)?;
        }
        output.extend_from_slice(&self.content.operations[self.region.end..]);
        if output.len() > MAX_OPS {
            return Err(bad("replacement exceeds the instruction limit"));
        }
        encoded_bound(&output)?;
        Ok(output)
    }
}

fn retain_text_state(ops: &[Operation], output: &mut Vec<Operation>) -> EngineResult<()> {
    for op in ops {
        match op.operator.as_str() {
            "Tj" | "TJ" | "'" | "Td" | "TD" | "Tm" | "T*" => {
                if op.operator == "TD" {
                    output.push(Operation::new("TL", vec![Object::Real(
                        -op.operands[1].as_float().map_err(|_| bad("invalid leading"))?,
                    )]));
                }
            }
            "\"" => {
                output.push(Operation::new("Tw", vec![op.operands[0].clone()]));
                output.push(Operation::new("Tc", vec![op.operands[1].clone()]));
            }
            _ => output.push(op.clone()),
        }
    }
    Ok(())
}

fn encoded_bound(ops: &[Operation]) -> EngineResult<usize> {
    encoded_bound_mode(ops, false)
}
fn encoded_bound_mode(ops: &[Operation], dictionaries: bool) -> EngineResult<usize> {
    fn object(value: &Object, depth: usize, dictionaries: bool) -> EngineResult<usize> {
        if depth > 64 {
            return Err(bad("output operand nesting exceeds 64 levels"));
        }
        let size = match value {
            Object::String(v, _) => v.len().saturating_mul(4).saturating_add(2),
            Object::Name(v) => v.len().saturating_mul(3).saturating_add(1),
            Object::Integer(_) => 24,
            Object::Real(v) if v.is_finite() => 64,
            Object::Array(values) => {
                let mut n = 2usize;
                for v in values {
                    n = n.saturating_add(object(v, depth + 1, dictionaries)? + 1);
                    if n > MAX_BYTES {
                        return Err(bad("output operands exceed 4 MiB"));
                    }
                }
                n
            }
            Object::Dictionary(values) if dictionaries => {
                let mut n = 4usize;
                for (key, value) in values.iter() {
                    n = n.saturating_add(key.len().saturating_mul(3).saturating_add(3));
                    n = n.saturating_add(object(value, depth + 1, dictionaries)?.saturating_add(1));
                    if n > MAX_BYTES {
                        return Err(bad("output property dictionary exceeds 4 MiB"));
                    }
                }
                n
            }
            Object::Boolean(_) | Object::Null if dictionaries => 5,
            _ => return Err(bad("unsupported output operand")),
        };
        if size > MAX_BYTES {
            return Err(bad("output operand exceeds 4 MiB"));
        }
        Ok(size)
    }
    if ops.len() > MAX_OPS {
        return Err(bad("output exceeds the instruction limit"));
    }
    let mut size = 0usize;
    for op in ops {
        size = size.saturating_add(op.operator.len() + 1);
        for value in &op.operands {
            size = size.saturating_add(object(value, 0, dictionaries)? + 1);
        }
        if size > MAX_BYTES {
            return Err(bad("encoded answer exceeds 4 MiB"));
        }
    }
    Ok(size)
}
fn encode_bounded(ops: &[Operation]) -> EngineResult<Vec<u8>> {
    let bound = encoded_bound(ops)?;
    encode_with_bound(ops, bound)
}
/// General retained content may contain bounded marked-content dictionaries.
/// Answer-isolation encoding keeps its stricter operand grammar unchanged.
pub(crate) fn encode_program(ops: &[Operation]) -> EngineResult<Vec<u8>> {
    encode_with_bound(ops, encoded_bound_mode(ops, true)?)
}
pub(crate) fn program_encoded_bound(ops: &[Operation]) -> EngineResult<usize> {
    encoded_bound_mode(ops, true)
}
fn encode_with_bound(ops: &[Operation], bound: usize) -> EngineResult<Vec<u8>> {
    let bytes = Content { operations: ops }
        .encode()
        .map_err(|_| bad("cannot encode answer appearance"))?;
    if bytes.len() > bound {
        return Err(bad("answer encoding exceeded its reserved bound"));
    }
    Ok(bytes)
}

const IDENTITY: [f64; 6] = [1., 0., 0., 1., 0., 0.];
#[derive(Clone, Copy)]
struct Frame {
    matrix: [f64; 6],
    clip: Option<usize>,
}
struct ClipNode {
    parent: Option<usize>,
    path: Range<usize>,
    matrix: [f64; 6],
    even: bool,
}
fn checked_matrix(m: [f64; 6]) -> EngineResult<[f64; 6]> {
    let det = m[0] * m[3] - m[1] * m[2];
    if m.iter().any(|n| !n.is_finite() || n.abs() > 1_000_000.)
        || !det.is_finite()
        || det.abs() < 1e-12
    {
        return Err(bad("singular or excessive appearance transform"));
    }
    Ok(m)
}
fn inverse(m: [f64; 6]) -> EngineResult<[f64; 6]> {
    let m = checked_matrix(m)?;
    let d = m[0] * m[3] - m[1] * m[2];
    checked_matrix([
        m[3] / d,
        -m[1] / d,
        -m[2] / d,
        m[0] / d,
        (m[2] * m[5] - m[3] * m[4]) / d,
        (m[1] * m[4] - m[0] * m[5]) / d,
    ])
}
fn compose(a: [f64; 6], b: [f64; 6]) -> EngineResult<[f64; 6]> {
    checked_matrix([
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ])
}
fn transform(m: [f64; 6]) -> Operation {
    Operation::new(
        "cm",
        m.into_iter().map(|n| Object::Real(n as f32)).collect(),
    )
}
fn capture(
    ops: &[Operation],
    region: &Range<usize>,
) -> EngineResult<(Frame, Option<usize>, Vec<ClipNode>)> {
    let mut state = Frame {
        matrix: IDENTITY,
        clip: None,
    };
    let mut stack = Vec::new();
    let mut clips: Vec<ClipNode> = Vec::new();
    let (mut entry, mut path, mut pending, mut painted, mut empty_clip) =
        (state, None, None, None, None);
    let mut empty_branches = false;
    for (i, op) in ops.iter().enumerate() {
        if i == region.start {
            entry = state;
            empty_clip = state.clip;
        }
        let inside = region.contains(&i);
        let paint = matches!(
            op.operator.as_str(),
            "Tj" | "TJ"
                | "'"
                | "\""
                | "Do"
                | "sh"
                | "S"
                | "s"
                | "f"
                | "F"
                | "f*"
                | "B"
                | "B*"
                | "b"
                | "b*"
        );
        if inside && paint {
            if painted.is_some_and(|old| old != state.clip) {
                return Err(bad("answer uses distinct clipping regions"));
            }
            painted = Some(state.clip);
        }
        match op.operator.as_str() {
            "q" => stack.push(state),
            "Q" => {
                state = stack
                    .pop()
                    .ok_or_else(|| bad("unmatched graphics restore"))?
            }
            "cm" => {
                let mut matrix = [0.; 6];
                for (n, v) in matrix.iter_mut().zip(&op.operands) {
                    *n = v.as_float().map_err(|_| bad("invalid transform"))? as f64;
                }
                state.matrix = compose(state.matrix, matrix)?;
            }
            "m" | "re" => {
                path.get_or_insert(i);
            }
            "W" | "W*" => pending = Some(op.operator == "W*"),
            "n" | "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                if let Some(even) = pending.take() {
                    if inside && empty_clip != state.clip {
                        // An empty slot may tighten one clip chain, but it has
                        // no answer paint proving which disconnected branch to
                        // choose after a graphics restore.
                        let mut ancestor = state.clip;
                        while let Some(id) = ancestor {
                            if Some(id) == empty_clip {
                                break;
                            }
                            ancestor = clips[id].parent;
                        }
                        if ancestor != empty_clip {
                            empty_branches = true;
                        }
                    }
                    let index = clips.len();
                    clips.push(ClipNode {
                        parent: state.clip,
                        path: path.ok_or_else(|| bad("missing clipping path"))?..i,
                        matrix: state.matrix,
                        even,
                    });
                    state.clip = Some(index);
                    if inside {
                        empty_clip = state.clip;
                    }
                }
                path = None;
            }
            _ => {}
        }
    }
    if painted.is_none() && empty_branches {
        return Err(bad("empty answer has ambiguous clipping branches"));
    }
    Ok((entry, painted.unwrap_or(empty_clip), clips))
}

fn delimiter(b: u8) -> bool {
    b == 0 || b.is_ascii_whitespace() || b"()<>[]{}/%".contains(&b)
}
/// Bound parser allocations before constructing Operation/Object vectors. Literal
/// strings and comments are skipped lexically, so text resembling operators is
/// not confused with executable appearance instructions.
/// Shared lexical allocation bound; does not authorize any appearance mutation.
pub(crate) fn preflight_program(bytes: &[u8]) -> EngineResult<usize> {
    preflight_cost_mode(bytes, true)
}
fn preflight(bytes: &[u8]) -> EngineResult<()> {
    preflight_cost(bytes).map(|_| ())
}
fn preflight_cost(bytes: &[u8]) -> EngineResult<usize> {
    preflight_cost_mode(bytes, false)
}
fn preflight_cost_mode(bytes: &[u8], dictionaries: bool) -> EngineResult<usize> {
    if bytes.len() > MAX_BYTES {
        return Err(bad("appearance exceeds 4 MiB"));
    }
    let (mut i, mut operations, mut operands, mut values, mut arrays) =
        (0, 0usize, 0usize, 0usize, 0usize);
    let mut dicts = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b == 0 || b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b == b'%' {
            while i < bytes.len() && !b"\r\n".contains(&bytes[i]) {
                i += 1;
            }
            continue;
        }
        values += 1;
        match b {
            b'(' => {
                i += 1;
                let mut depth = 1usize;
                while i < bytes.len() && depth > 0 {
                    match bytes[i] {
                        b'\\' => {
                            i += 1;
                            if i >= bytes.len() {
                                return Err(bad("unfinished string escape"));
                            }
                        }
                        b'(' => {
                            depth += 1;
                            if depth > 64 {
                                return Err(bad("string nesting exceeds 64 levels"));
                            }
                        }
                        b')' => depth -= 1,
                        _ => {}
                    }
                    i += 1;
                }
                if depth != 0 {
                    return Err(bad("unfinished string"));
                }
                operands += 1;
            }
            b'<' => {
                i += 1;
                if bytes.get(i) == Some(&b'<') {
                    if !dictionaries {
                        return Err(bad("inline property dictionaries are not supported"));
                    }
                    dicts += 1;
                    if arrays + dicts > 64 {
                        return Err(bad("operand nesting exceeds 64 levels"));
                    }
                    i += 1;
                } else {
                    while i < bytes.len() && bytes[i] != b'>' {
                        i += 1;
                    }
                    if i == bytes.len() {
                        return Err(bad("unfinished hexadecimal string"));
                    }
                    i += 1;
                }
                operands += 1;
            }
            b'>' if dictionaries && bytes.get(i + 1) == Some(&b'>') => {
                dicts = dicts
                    .checked_sub(1)
                    .ok_or_else(|| bad("unmatched dictionary"))?;
                i += 2;
            }
            b'[' => {
                arrays += 1;
                if arrays + dicts > 64 {
                    return Err(bad("operand nesting exceeds 64 levels"));
                }
                i += 1;
                operands += 1;
            }
            b']' => {
                arrays = arrays
                    .checked_sub(1)
                    .ok_or_else(|| bad("unmatched array"))?;
                i += 1;
            }
            b'/' => {
                i += 1;
                while i < bytes.len() && !delimiter(bytes[i]) {
                    i += 1;
                }
                operands += 1;
            }
            b')' | b'>' | b'{' | b'}' => return Err(bad("unexpected appearance delimiter")),
            _ => {
                let start = i;
                while i < bytes.len() && !delimiter(bytes[i]) {
                    i += 1;
                }
                let token = &bytes[start..i];
                if arrays == 0
                    && dicts == 0
                    && (b.is_ascii_alphabetic() || b"'\"".contains(&b))
                    && !matches!(token, b"true" | b"false" | b"null")
                {
                    if token == b"BI" {
                        return Err(bad("inline images are not answer text"));
                    }
                    operations += 1;
                    operands = 0;
                } else {
                    operands += 1;
                }
            }
        }
        if operations > MAX_OPS
            || operands > 100_000
            || values > 1_000_000
            || operations
                .saturating_mul(96)
                .saturating_add(values.saturating_mul(64))
                .saturating_add(bytes.len())
                > 64 * 1024 * 1024
        {
            return Err(bad("appearance parser working set exceeds its bound"));
        }
    }
    if arrays != 0 || dicts != 0 {
        return Err(bad("unfinished operand container"));
    }
    Ok(operations
        .saturating_mul(96)
        .saturating_add(values.saturating_mul(64))
        .saturating_add(bytes.len()))
}
fn numbers(op: &Operation, n: usize) -> EngineResult<()> {
    if op.operands.len() != n {
        return Err(bad("invalid instruction arguments"));
    }
    for v in &op.operands {
        if !v
            .as_float()
            .map_err(|_| bad("non-numeric instruction"))?
            .is_finite()
        {
            return Err(bad("non-finite instruction"));
        }
    }
    Ok(())
}
fn name(op: &Operation) -> EngineResult<&[u8]> {
    if op.operands.len() != 1 {
        return Err(bad("invalid named instruction"));
    }
    let name = op.operands[0]
        .as_name()
        .map_err(|_| bad("missing resource name"))?;
    if name.is_empty() {
        return Err(bad("empty resource name"));
    }
    Ok(name)
}
fn string(v: &Object) -> EngineResult<()> {
    v.as_str()
        .map(|_| ())
        .map_err(|_| bad("invalid text operand"))
}
#[derive(Clone, Copy, Default)]
struct TextState {
    font: bool,
    mode: i64,
}
fn validate(
    ops: &[Operation],
    region: &Range<usize>,
    marked: bool,
    kind: AnswerKind,
    has_bounds: bool,
) -> EngineResult<()> {
    let mut state = TextState::default();
    let mut stack = Vec::new();
    let (mut text, mut path, mut clip, mut base) = (false, false, false, None);
    for (i, op) in ops.iter().enumerate() {
        if i == region.start {
            if path || clip || text {
                return Err(bad("answer starts inside an unfinished drawing"));
            }
            base = Some(stack.len());
        }
        let inside = region.contains(&i);
        if marked
            && inside
            && !matches!(op.operator.as_str(), "BMC" | "EMC" | "q")
            && stack.len() <= base.unwrap()
        {
            return Err(bad("marked answer leaks graphics state"));
        }
        match op.operator.as_str() {
            "q" if !text && !path && !clip => {
                numbers(op, 0)?;
                if stack.len() >= 64 {
                    return Err(bad("graphics nesting exceeds 64 levels"));
                }
                stack.push(state);
            }
            "Q" if !text && !path && !clip => {
                numbers(op, 0)?;
                state = stack
                    .pop()
                    .ok_or_else(|| bad("unmatched graphics restore"))?;
            }
            "BMC" | "EMC" if !text && !path && !clip => {}
            "BT" if !text && !path && !clip => {
                numbers(op, 0)?;
                text = true;
            }
            "ET" if text => {
                numbers(op, 0)?;
                text = false;
            }
            "Tf" => {
                if op.operands.len() != 2
                    || op.operands[0].as_name().map_or(true, |v| v.is_empty())
                    || !op.operands[1]
                        .as_float()
                        .map_err(|_| bad("invalid font size"))?
                        .is_finite()
                {
                    return Err(bad("invalid font instruction"));
                }
                state.font = true;
            }
            "Tc" | "Tw" | "Tz" | "TL" | "Ts" => numbers(op, 1)?,
            "Tr" => {
                numbers(op, 1)?;
                let mode = op.operands[0]
                    .as_i64()
                    .map_err(|_| bad("invalid render mode"))?;
                if !(0..=3).contains(&mode) {
                    return Err(bad("text clipping cannot be replaced safely"));
                }
                state.mode = mode;
            }
            "Td" | "TD" if text => numbers(op, 2)?,
            "Tm" if text => numbers(op, 6)?,
            "T*" if text => numbers(op, 0)?,
            "Tj" | "'" if text && state.font => {
                if op.operands.len() != 1 {
                    return Err(bad("invalid text instruction"));
                }
                string(&op.operands[0])?;
            }
            "\"" if text && state.font => {
                if op.operands.len() != 3 {
                    return Err(bad("invalid quoted text"));
                }
                numbers(&Operation::new("", op.operands[..2].to_vec()), 2)?;
                string(&op.operands[2])?;
            }
            "TJ" if text && state.font => {
                if op.operands.len() != 1 {
                    return Err(bad("invalid positioned text"));
                }
                for v in op.operands[0]
                    .as_array()
                    .map_err(|_| bad("invalid text array"))?
                {
                    if matches!(v, Object::String(..)) {
                        string(v)?;
                    } else if !v
                        .as_float()
                        .map_err(|_| bad("invalid text adjustment"))?
                        .is_finite()
                    {
                        return Err(bad("non-finite text adjustment"));
                    }
                }
            }
            "g" | "G" => numbers(op, 1)?,
            "rg" | "RG" => numbers(op, 3)?,
            "k" | "K" => numbers(op, 4)?,
            "cm" if !text && !path && !clip => numbers(op, 6)?,
            "w" | "M" | "i" => numbers(op, 1)?,
            "J" | "j" => {
                numbers(op, 1)?;
                if !matches!(op.operands[0].as_i64(), Ok(0..=2)) {
                    return Err(bad("invalid line style"));
                }
            }
            "ri" => {
                name(op)?;
            }
            "d" => {
                if op.operands.len() != 2 {
                    return Err(bad("invalid dash instruction"));
                }
                for n in op.operands[0]
                    .as_array()
                    .map_err(|_| bad("invalid dash array"))?
                    .iter()
                    .chain(&op.operands[1..])
                {
                    let n = n.as_float().map_err(|_| bad("invalid dash number"))?;
                    if !n.is_finite() || n < 0. {
                        return Err(bad("invalid dash number"));
                    }
                }
            }
            "Do" | "gs" | "sh" if !text && !path && !clip => {
                name(op)?;
            }
            "m" | "re" if !text && !clip => {
                numbers(op, if op.operator == "m" { 2 } else { 4 })?;
                path = true;
            }
            "l" if !text && path && !clip => numbers(op, 2)?,
            "c" if !text && path && !clip => numbers(op, 6)?,
            "v" | "y" if !text && path && !clip => numbers(op, 4)?,
            "h" if !text && path && !clip => numbers(op, 0)?,
            "W" | "W*" if !text && path && !clip => {
                numbers(op, 0)?;
                clip = true;
            }
            "n" | "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" if !text => {
                numbers(op, 0)?;
                if !marked
                    && !has_bounds
                    && !matches!(kind, AnswerKind::Text)
                    && !matches!(op.operator.as_str(), "n" | "S" | "s")
                {
                    return Err(bad("unmarked choice selection paint is ambiguous"));
                }
                path = false;
                clip = false;
            }
            _ => return Err(bad("unsupported or misplaced appearance instruction")),
        }
        if !region.is_empty()
            && i + 1 == region.end
            && (text || path || clip || stack.len() != base.unwrap())
        {
            return Err(bad("answer leaves an unfinished graphics state"));
        }
    }
    if text || path || clip || !stack.is_empty() {
        return Err(bad("appearance ends in an unfinished graphics state"));
    }
    Ok(())
}

/// For an unmarked choice, a filled row can be old selection paint. Only an
/// unclipped full-BBox rectangle or even-odd inset ring before text is static.
/// Use geometry, not a color guess; all other unmarked fill remains ambiguous.
fn choice_background(
    ops: &[Operation],
    region: &Range<usize>,
    bounds: [f64; 4],
) -> EngineResult<()> {
    let mut state = (IDENTITY, false, false);
    let mut stack = Vec::new();
    let mut rectangle = None;
    let mut first_rectangle = None;
    let mut path_parts = 0;
    let mut clip = false;
    for (i, op) in ops.iter().enumerate() {
        match op.operator.as_str() {
            "q" => stack.push(state),
            "Q" => {
                state = stack
                    .pop()
                    .ok_or_else(|| bad("unmatched background graphics restore"))?
            }
            "cm" => {
                let mut m = [0.; 6];
                for (n, v) in m.iter_mut().zip(&op.operands) {
                    *n = v
                        .as_float()
                        .map_err(|_| bad("invalid background transform"))?
                        as f64;
                }
                state.0 = compose(state.0, m)?;
            }
            "gs" => state.2 = true,
            "m" | "l" | "c" | "v" | "y" | "h" => {
                path_parts += 1;
                rectangle = None;
            }
            "re" => {
                path_parts += 1;
                let mut r = [0.; 4];
                for (n, v) in r.iter_mut().zip(&op.operands) {
                    *n = v
                        .as_float()
                        .map_err(|_| bad("invalid background rectangle"))?
                        as f64;
                }
                rectangle = Some(r);
                if path_parts == 1 {
                    first_rectangle = Some(r);
                }
            }
            "W" | "W*" => clip = true,
            "n" | "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                if !matches!(op.operator.as_str(), "n" | "S" | "s") {
                    let ring = op.operator == "f*" && path_parts == 2;
                    if i >= region.start || state.1 || state.2 || (path_parts != 1 && !ring) {
                        return Err(bad("unmarked choice fill is not a full background"));
                    }
                    let last =
                        rectangle.ok_or_else(|| bad("unmarked choice fill is not rectangular"))?;
                    let r = if ring {
                        let outer = first_rectangle
                            .ok_or_else(|| bad("unmarked choice border is not rectangular"))?;
                        let inset = last[0] - outer[0];
                        let tolerance = 8.
                            * f32::EPSILON as f64
                            * outer.iter().copied().map(f64::abs).fold(1., f64::max);
                        if inset < 0.
                            || outer[2] <= 0.
                            || outer[3] <= 0.
                            || last[2] <= 0.
                            || last[3] <= 0.
                            || ((last[1] - outer[1]) - inset).abs() > tolerance
                            || (last[2] - (outer[2] - 2. * inset)).abs() > tolerance
                            || (last[3] - (outer[3] - 2. * inset)).abs() > tolerance
                        {
                            return Err(bad("unmarked choice fill is not a uniform inset border"));
                        }
                        outer
                    } else {
                        last
                    };
                    let tolerance = 1e-6;
                    let m = state.0;
                    let mut corners = std::collections::BTreeSet::new();
                    for x in [r[0], r[0] + r[2]] {
                        for y in [r[1], r[1] + r[3]] {
                            let px = m[0] * x + m[2] * y + m[4];
                            let py = m[1] * x + m[3] * y + m[5];
                            let ix = if (px - bounds[0]).abs() < tolerance {
                                0
                            } else if (px - bounds[2]).abs() < tolerance {
                                1
                            } else {
                                return Err(bad("unmarked choice fill does not cover its bounds"));
                            };
                            let iy = if (py - bounds[1]).abs() < tolerance {
                                0
                            } else if (py - bounds[3]).abs() < tolerance {
                                1
                            } else {
                                return Err(bad("unmarked choice fill does not cover its bounds"));
                            };
                            corners.insert((ix, iy));
                        }
                    }
                    if corners.len() != 4 {
                        return Err(bad("unmarked choice background is degenerate"));
                    }
                }
                if clip {
                    state.1 = true;
                }
                clip = false;
                path_parts = 0;
                rectangle = None;
                first_rectangle = None;
            }
            _ => {}
        }
    }
    Ok(())
}
#[cfg(test)]
#[path = "form_answer_appearance_tests.rs"]
mod tests;
