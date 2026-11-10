//! Structural text isolation only. Callers must also prove geometry, fonts and pixels.
// This private layer is staged ahead of the native choice appearance generator.
#![allow(dead_code)]
use crate::{EngineError, EngineResult};
use lopdf::{
    content::{Content, Operation},
    Object,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
};

const MAX_BYTES: usize = 256 * 1024;
const MAX_OPS: usize = 16384;
const MAX_DEPTH: usize = 32;

fn bad() -> EngineError {
    EngineError::InvalidRequest(
        "Choice appearance: text and surrounding artwork cannot be isolated safely".into(),
    )
}
fn number(v: &Object) -> EngineResult<f32> {
    let n = v.as_float().map_err(|_| bad())?;
    if !n.is_finite() {
        return Err(bad());
    }
    Ok(n)
}
fn numbers(op: &Operation, n: usize) -> EngineResult<()> {
    if op.operands.len() != n {
        return Err(bad());
    }
    for v in &op.operands {
        number(v)?;
    }
    Ok(())
}
fn name(v: &Object) -> EngineResult<&[u8]> {
    let n = v.as_name().map_err(|_| bad())?;
    if n.is_empty() {
        return Err(bad());
    }
    Ok(n)
}
fn one_name(op: &Operation) -> EngineResult<&[u8]> {
    if op.operands.len() != 1 {
        return Err(bad());
    }
    name(&op.operands[0])
}
fn text(v: &Object) -> EngineResult<()> {
    v.as_str().map_err(|_| bad())?;
    Ok(())
}
fn enum_value(op: &Operation, max: i64) -> EngineResult<()> {
    if op.operands.len() != 1 {
        return Err(bad());
    }
    let n = op.operands[0].as_i64().map_err(|_| bad())?;
    if !(0..=max).contains(&n) {
        return Err(bad());
    }
    Ok(())
}

/// Each segment starts and ends in the same graphics state, outside a text object,
/// with no current path. q/Q alone does not establish those latter two properties.
fn island(ops: &[Operation], tx: bool) -> EngineResult<BTreeSet<Vec<u8>>> {
    let mut stack = Vec::new();
    let (mut in_text, mut path, mut pending_clip, mut font) = (false, false, false, false);
    let mut fonts = BTreeSet::new();
    for op in ops {
        if op.operator == "q" {
            numbers(op, 0)?;
            if in_text || path || pending_clip || stack.len() >= MAX_DEPTH {
                return Err(bad());
            }
            stack.push(font);
            continue;
        }
        if stack.is_empty() {
            return Err(bad());
        }
        match op.operator.as_str() {
            "Q" => {
                numbers(op, 0)?;
                if in_text || path || pending_clip {
                    return Err(bad());
                }
                font = stack.pop().ok_or_else(bad)?;
            }
            "BT" => {
                numbers(op, 0)?;
                if in_text || path || pending_clip {
                    return Err(bad());
                }
                in_text = true;
            }
            "ET" => {
                numbers(op, 0)?;
                if !in_text {
                    return Err(bad());
                }
                in_text = false;
            }
            "Tf" if in_text => {
                if op.operands.len() != 2 {
                    return Err(bad());
                }
                let n = name(&op.operands[0])?;
                number(&op.operands[1])?;
                fonts.insert(n.to_vec());
                font = true;
            }
            "Tc" | "Tw" | "Tz" | "TL" | "Ts" if in_text => numbers(op, 1)?,
            "Tr" if in_text => enum_value(op, 7)?,
            "Td" | "TD" if in_text => numbers(op, 2)?,
            "Tm" if in_text => numbers(op, 6)?,
            "T*" if in_text => numbers(op, 0)?,
            "Tj" | "'" if in_text && font => {
                if op.operands.len() != 1 {
                    return Err(bad());
                }
                text(&op.operands[0])?;
            }
            "\"" if in_text && font => {
                if op.operands.len() != 3 {
                    return Err(bad());
                }
                number(&op.operands[0])?;
                number(&op.operands[1])?;
                text(&op.operands[2])?;
            }
            "TJ" if in_text && font => {
                if op.operands.len() != 1 {
                    return Err(bad());
                }
                for v in op.operands[0].as_array().map_err(|_| bad())? {
                    if matches!(v, Object::String(..)) {
                        text(v)?;
                    } else {
                        number(v)?;
                    }
                }
            }
            "g" | "G" => numbers(op, 1)?,
            "rg" | "RG" => numbers(op, 3)?,
            "k" | "K" => numbers(op, 4)?,
            "cm" if !in_text => numbers(op, 6)?,
            "w" | "M" | "i" if !in_text => numbers(op, 1)?,
            "J" | "j" if !in_text => enum_value(op, 2)?,
            "ri" if !in_text => {
                one_name(op)?;
            }
            "d" if !in_text => {
                if op.operands.len() != 2 {
                    return Err(bad());
                }
                for v in op.operands[0].as_array().map_err(|_| bad())? {
                    if number(v)? < 0. {
                        return Err(bad());
                    }
                }
                number(&op.operands[1])?;
            }
            "Do" | "gs" | "sh" if !in_text && !tx => {
                one_name(op)?;
            }
            "m" if !in_text => {
                numbers(op, 2)?;
                path = true;
            }
            "re" if !in_text => {
                numbers(op, 4)?;
                path = true;
            }
            "l" if !in_text && path => numbers(op, 2)?,
            "c" if !in_text && path => numbers(op, 6)?,
            "v" | "y" if !in_text && path => numbers(op, 4)?,
            "h" if !in_text && path => numbers(op, 0)?,
            "W" | "W*" if !in_text && path && !pending_clip => {
                numbers(op, 0)?;
                pending_clip = true;
            }
            "n" | "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" if !in_text => {
                numbers(op, 0)?;
                path = false;
                pending_clip = false;
            }
            _ => return Err(bad()),
        }
    }
    if !stack.is_empty() || in_text || path || pending_clip {
        return Err(bad());
    }
    Ok(fonts)
}

// Conservative bound on serialization, including escaped strings/names. Operand
// validation admits only flat arrays, so recursively malformed objects never enter.
fn object_size(v: &Object) -> EngineResult<usize> {
    Ok(match v {
        Object::Name(n) => n
            .len()
            .checked_mul(3)
            .and_then(|n| n.checked_add(2))
            .ok_or_else(bad)?,
        Object::String(s, _) => s
            .len()
            .checked_mul(4)
            .and_then(|n| n.checked_add(2))
            .ok_or_else(bad)?,
        Object::Integer(_) => 24,
        Object::Real(n) if n.is_finite() => 64,
        Object::Array(a) => {
            let mut size = 2usize;
            for v in a {
                size = size
                    .checked_add(object_size(v)? + 1)
                    .filter(|n| *n <= MAX_BYTES)
                    .ok_or_else(bad)?;
            }
            size
        }
        _ => return Err(bad()),
    })
}
fn charge(op: &Operation, mapped: Option<&[u8]>, total: &mut usize) -> EngineResult<()> {
    let mut size = op.operator.len() + 1;
    for (i, v) in op.operands.iter().enumerate() {
        let n = if i == 0 {
            mapped
                .map(|n| {
                    n.len()
                        .checked_mul(3)
                        .and_then(|n| n.checked_add(2))
                        .ok_or_else(bad)
                })
                .transpose()?
                .unwrap_or(object_size(v)?)
        } else {
            object_size(v)?
        };
        size = size
            .checked_add(n + 1)
            .filter(|n| *n <= MAX_BYTES)
            .ok_or_else(bad)?;
    }
    *total = total
        .checked_add(size)
        .filter(|n| *n <= MAX_BYTES)
        .ok_or_else(bad)?;
    Ok(())
}

pub(crate) struct ChoiceTextBlock {
    content: Content,
    region: Range<usize>,
    fonts: BTreeSet<Vec<u8>>,
}
impl ChoiceTextBlock {
    /// Caller must separately prove an empty value. This admits only independent
    /// vector drawing, never hidden text or resource-dependent state/content.
    pub(crate) fn parse_empty_background(bytes: &[u8]) -> EngineResult<Self> {
        if bytes.len() > MAX_BYTES {
            return Err(bad());
        }
        let content = Content::decode_strict(bytes).map_err(|_| bad())?;
        if content.operations.len() > MAX_OPS
            || content.operations.iter().any(|op| {
                op.operator.starts_with('T')
                    || matches!(
                        op.operator.as_str(),
                        "'" | "\"" | "BMC" | "BDC" | "EMC" | "Do" | "gs" | "sh"
                    )
            })
        {
            return Err(bad());
        }
        island(&content.operations, false)?;
        let end = content.operations.len();
        Ok(Self {
            content,
            region: end..end,
            fonts: BTreeSet::new(),
        })
    }
    pub(crate) fn parse(bytes: &[u8]) -> EngineResult<Self> {
        if bytes.len() > MAX_BYTES {
            return Err(bad());
        }
        let content = Content::decode_strict(bytes).map_err(|_| bad())?;
        let ops = &content.operations;
        if ops.len() > MAX_OPS {
            return Err(bad());
        }
        let (mut start, mut end) = (None, None);
        for (i, op) in ops.iter().enumerate() {
            match op.operator.as_str() {
                "BMC" if start.is_none() && one_name(op)? == b"Tx" => start = Some(i),
                "EMC" if start.is_some() && end.is_none() => {
                    numbers(op, 0)?;
                    end = Some(i + 1);
                }
                "BMC" | "BDC" | "EMC" => return Err(bad()),
                _ => {}
            }
        }
        let region = start.ok_or_else(bad)?..end.ok_or_else(bad)?;
        island(&ops[..region.start], false)?;
        let fonts = island(&ops[region.start + 1..region.end - 1], true)?;
        island(&ops[region.end..], false)?;
        Ok(Self {
            content,
            region,
            fonts,
        })
    }
    pub(crate) fn font_names(&self) -> &BTreeSet<Vec<u8>> {
        &self.fonts
    }
    pub(crate) fn transplant(
        &self,
        generated: &Self,
        names: &BTreeMap<Vec<u8>, Vec<u8>>,
    ) -> EngineResult<Vec<u8>> {
        self.transplant_mapped(generated, names, [1., 0., 0., 1., 0., 0.])
    }
    /// Maps generated BBox coordinates only. Rotation/skew belong to the saved
    /// outer Form Matrix, which the caller retains unchanged.
    pub(crate) fn transplant_mapped(
        &self,
        generated: &Self,
        names: &BTreeMap<Vec<u8>, Vec<u8>>,
        mapping: [f32; 6],
    ) -> EngineResult<Vec<u8>> {
        self.transplant_mapped_bounded(generated, names, mapping, MAX_BYTES)
    }
    pub(crate) fn transplant_mapped_bounded(
        &self,
        generated: &Self,
        names: &BTreeMap<Vec<u8>, Vec<u8>>,
        mapping: [f32; 6],
        byte_limit: usize,
    ) -> EngineResult<Vec<u8>> {
        if mapping
            .iter()
            .any(|n| !n.is_finite() || n.abs() > 1_000_000.)
            || !(0.000001..=1_000_000.).contains(&mapping[0])
            || !(0.000001..=1_000_000.).contains(&mapping[3])
            || mapping[1] != 0.
            || mapping[2] != 0.
        {
            return Err(bad());
        }
        for font in &generated.fonts {
            if names
                .get(font)
                .is_none_or(|n| n.is_empty() || n.len() > MAX_BYTES)
            {
                return Err(bad());
            }
        }
        let before = &self.content.operations[..self.region.start];
        // Keep an emptied source slot at its original layer. Removing the mark
        // would move future text above drawing that originally followed it.
        let empty_slot;
        let text = if generated.region.is_empty() && !self.region.is_empty() {
            empty_slot = vec![
                Operation::new("BMC", vec![Object::Name(b"Tx".to_vec())]),
                Operation::new("q", vec![]),
                Operation::new("Q", vec![]),
                Operation::new("EMC", vec![]),
            ];
            empty_slot.as_slice()
        } else {
            &generated.content.operations[generated.region.clone()]
        };
        let after = &self.content.operations[self.region.end..];
        let wrap = !generated.region.is_empty() && mapping != [1., 0., 0., 1., 0., 0.];
        let transform = Operation::new("cm", mapping.into_iter().map(Object::Real).collect());
        if wrap {
            let mut depth = 1usize;
            for op in text {
                match op.operator.as_str() {
                    "q" => {
                        depth += 1;
                        if depth > MAX_DEPTH {
                            return Err(bad());
                        }
                    }
                    "Q" => depth -= 1,
                    _ => {}
                }
            }
        }
        let count = before.len() + text.len() + after.len() + if wrap { 3 } else { 0 };
        if count > MAX_OPS {
            return Err(bad());
        }
        let mapped = |op: &Operation| -> EngineResult<Option<&[u8]>> {
            if op.operator == "Tf" {
                Ok(Some(
                    names
                        .get(name(&op.operands[0])?)
                        .ok_or_else(bad)?
                        .as_slice(),
                ))
            } else {
                Ok(None)
            }
        };
        let mut size = 0;
        for op in before.iter().chain(after) {
            charge(op, None, &mut size)?;
        }
        for op in text {
            charge(op, mapped(op)?, &mut size)?;
        }
        if wrap {
            charge(&transform, None, &mut size)?;
            charge(&Operation::new("q", vec![]), None, &mut size)?;
            charge(&Operation::new("Q", vec![]), None, &mut size)?;
        }
        if size > byte_limit {
            return Err(EngineError::InvalidRequest(
                "Choice appearance: retained text exceeds the byte budget".into(),
            ));
        }
        // All output allocations are admitted before any operation is cloned.
        let mut output = Vec::with_capacity(count);
        output.extend_from_slice(before);
        for (index, op) in text.iter().enumerate() {
            if wrap && index + 1 == text.len() {
                output.push(Operation::new("Q", vec![]));
            }
            let mut next = op.clone();
            if let Some(name) = mapped(op)? {
                next.operands[0] = Object::Name(name.to_vec());
            }
            output.push(next);
            if wrap && index == 0 {
                output.push(Operation::new("q", vec![]));
                output.push(transform.clone());
            }
        }
        output.extend_from_slice(after);
        let encoded = Content { operations: output }.encode().map_err(|_| bad())?;
        if encoded.len() > size {
            return Err(bad());
        }
        Ok(encoded)
    }
}
