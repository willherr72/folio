//! Exact, bounded native choice text auditing.
use crate::{choice_font::ChoiceFontProof, EngineError, EngineResult};
use lopdf::content::Content;
use std::{collections::BTreeMap, ops::Range};

fn bad() -> EngineError {
    EngineError::InvalidRequest(
        "The native choice text does not match the required labels and values.".into(),
    )
}

pub(super) fn batches(proof: &ChoiceFontProof, texts: &[&str]) -> EngineResult<Vec<Range<usize>>> {
    proof.supports(texts)?;
    let mut result = Vec::new();
    let mut start = 0;
    let mut bytes = 0;
    for (index, text) in texts.iter().enumerate() {
        if index - start == 128 || bytes + text.len() > 4096 {
            result.push(start..index);
            start = index;
            bytes = 0;
        }
        bytes += text.len();
    }
    if start < texts.len() {
        result.push(start..texts.len());
    }
    if result.len() > 128 {
        return Err(bad());
    }
    Ok(result)
}

// SDK lists emit one BT/ET pair per option (including empty labels). A combo
// emits one pair for its nonempty displayed value. This is deliberately stricter
// than general PDF text extraction and proves code order, not placement or ink.
pub(super) fn verify_rows(
    content: &Content,
    proof: &ChoiceFontProof,
    names: &BTreeMap<Vec<u8>, Vec<u8>>,
    expected: &[&str],
) -> EngineResult<()> {
    if content.operations.len() > 16384 {
        return Err(bad());
    }
    proof.supports(expected)?;
    let mut row = 0;
    let mut inside = false;
    let mut bound = false;
    let mut text = String::new();
    for op in &content.operations {
        match op.operator.as_str() {
            "BT" => {
                if inside || row >= expected.len() || !op.operands.is_empty() {
                    return Err(bad());
                }
                inside = true;
                bound = false;
                text.clear();
            }
            "ET" => {
                if !inside || !op.operands.is_empty() || text != expected[row] {
                    return Err(bad());
                }
                inside = false;
                row += 1;
            }
            "Tf" => {
                if !inside || op.operands.len() != 2 {
                    return Err(bad());
                }
                let name = op.operands[0].as_name().map_err(|_| bad())?;
                let size = op.operands[1].as_float().map_err(|_| bad())?;
                if !names.contains_key(name) || !size.is_finite() || size <= 0. {
                    return Err(bad());
                }
                bound = true;
            }
            "Tj" => {
                if !inside || !bound || op.operands.len() != 1 {
                    return Err(bad());
                }
                let bytes = op.operands[0].as_str().map_err(|_| bad())?;
                let remaining = expected[row].len() - text.len();
                // Verified codes use at most two bytes per scalar. Bound the
                // temporary decode before allocation, then charge UTF-8 output.
                if bytes.len() > remaining * 2 {
                    return Err(bad());
                }
                let decoded = proof.decode(bytes).map_err(|_| bad())?;
                if decoded.len() > remaining {
                    return Err(bad());
                }
                text.push_str(&decoded);
            }
            "TJ" | "'" | "\"" => return Err(bad()),
            "q" | "Q" if inside => return Err(bad()),
            _ => {}
        }
    }
    if inside || row != expected.len() {
        return Err(bad());
    }
    Ok(())
}
#[cfg(test)]
#[path = "choice_codec_tests.rs"]
mod tests;
