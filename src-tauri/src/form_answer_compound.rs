//! Separate a geometrically proved static frame from marked variable text.
use super::*;

fn paints_outside_text(op: &Operation) -> bool {
    matches!(op.operator.as_str(), "Do" | "sh" | "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*")
}

pub(super) fn normalize(
    ops: &[Operation],
    marked: &Range<usize>,
    kind: AnswerKind,
    bounds: Option<[f64; 4]>,
) -> EngineResult<Option<(Content, Range<usize>)>> {
    if !matches!(kind, AnswerKind::Text) {
        return Ok(None);
    }
    let Some(bounds) = bounds else { return Ok(None) };
    let interior = &ops[marked.clone()];
    if !interior.iter().any(paints_outside_text) {
        return Ok(None);
    }
    let Some(first) = interior.iter().position(|op| op.operator == "BT") else {
        // Existing marked vector-only/empty slots keep their original proof.
        return Ok(None);
    };
    let last = interior.iter().rposition(|op| op.operator == "ET")
        .ok_or_else(|| bad("compound answer has no text end"))?;
    let text = marked.start + first..marked.start + last + 1;
    if ops[text.clone()].iter().any(paints_outside_text) {
        return Err(bad("compound answer interleaves text and artwork"));
    }
    // A balanced final depth alone is insufficient: Q followed by q could
    // escape the insertion state and re-enter a different graphics frame.
    let mut depth = 0usize;
    for op in &ops[text.clone()] {
        match op.operator.as_str() {
            "q" => depth += 1,
            "Q" => depth = depth.checked_sub(1)
                .ok_or_else(|| bad("compound text escapes its graphics frame"))?,
            _ => {}
        }
    }
    if depth != 0 {
        return Err(bad("compound text leaves an open graphics frame"));
    }
    validate(ops, &text, false, kind, true)?;
    // Preserve the existing one-answer-clip rule, applied to text rather than
    // the static frame. No union of disjoint source clipping regions is allowed.
    capture(ops, &text)?;

    encoded_bound(ops)?;
    let mut budget = crate::form_graph::CopyBudget::with_byte_limit(64 * 1024 * 1024);
    // Bound the unmarked copy, frame parser, canonical output and state replay
    // before any of those operation/operand copies are allocated.
    for op in ops.iter().chain(ops).chain(ops).chain(&ops[text.clone()]) {
        budget.reserve_bytes(std::mem::size_of::<Operation>() + op.operator.len())?;
        budget.array(&op.operands)?;
    }
    let unmarked: Vec<Operation> = ops.iter().enumerate()
        .filter(|(i, _)| *i != marked.start && *i + 1 != marked.end)
        .map(|(_, op)| op.clone()).collect();
    let frame_bytes = encode_bounded(&unmarked)?;
    let frame = crate::form_frame_paint::FramePaint::inspect_with_absence(
        &frame_bytes, bounds, false, true,
    ).map_err(|_| bad("compound answer frame cannot be proved"))?;
    let mut allowed = frame.edge_paints.clone();
    if frame.border != usize::MAX { allowed.push(frame.border); }
    if let Some(index) = frame.background { allowed.push(index); }
    for (index, op) in ops.iter().enumerate().take(marked.end).skip(marked.start) {
        if paints_outside_text(op) && !allowed.contains(&(index - 1)) {
            return Err(bad("compound marker contains unproved artwork"));
        }
    }
    drop(frame);

    // Removing the opening marker shifts both text endpoints by one. The
    // original closing marker is after the text and has no graphics effect.
    let text = text.start - 1..text.end - 1;
    let mut output = Vec::new();
    output.extend_from_slice(&unmarked[..text.start]);
    let start = output.len();
    output.push(Operation::new("BMC", vec![Object::Name(b"Tx".to_vec())]));
    output.push(Operation::new("q", vec![]));
    output.extend_from_slice(&unmarked[text.clone()]);
    output.push(Operation::new("Q", vec![]));
    output.push(Operation::new("EMC", vec![]));
    let region = start..output.len();
    retain_text_state(&unmarked[text.clone()], &mut output)?;
    output.extend_from_slice(&unmarked[text.end..]);
    encoded_bound(&output)?;
    Ok(Some((Content { operations: output }, region)))
}
