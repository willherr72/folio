//! Bounded ToUnicode parsing; parsed Unicode is not a font/glyph proof.
use super::{invalid, EngineResult, MAX_CMAP};
use lopdf::Object;
use std::collections::BTreeMap;

pub(super) fn hex(value: &Object, length: usize) -> EngineResult<u16> {
    let bytes = value
        .as_str()
        .map_err(|_| invalid("The Unicode mapping contains a non-string code."))?;
    if bytes.len() != length || !matches!(length, 1 | 2) {
        return Err(invalid(
            "The font uses unsupported variable-length character codes.",
        ));
    }
    Ok(bytes
        .iter()
        .fold(0u16, |n, byte| n * 256 + u16::from(*byte)))
}

pub(super) fn unicode(bytes: &[u8]) -> EngineResult<char> {
    if !matches!(bytes.len(), 2 | 4) {
        return Err(invalid("The font maps one glyph to multiple characters or a ligature. Its text cannot be edited safely."));
    }
    let units: Vec<_> = bytes
        .chunks_exact(2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .collect();
    let text = String::from_utf16(&units)
        .map_err(|_| invalid("The font Unicode mapping is malformed."))?;
    let mut chars = text.chars();
    let character = chars
        .next()
        .ok_or_else(|| invalid("The font Unicode mapping is empty."))?;
    if chars.next().is_some() {
        return Err(invalid("The font maps one glyph to multiple characters or a ligature. Its text cannot be edited safely."));
    }
    Ok(character)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum CMapPolicy {
    Complete,
    LatinEditing,
}

// PDF ToUnicode ranges increment only the final encoded byte. Do not invent
// scalar arithmetic, carry into an earlier byte, or repair invalid UTF-16.
fn destination(value: &Object, offset: u16, policy: CMapPolicy) -> EngineResult<Option<char>> {
    let bytes = value
        .as_str()
        .map_err(|_| invalid("The font Unicode mapping contains a non-string destination."))?;
    let character = if matches!(bytes.len(), 2 | 4) {
        let mut encoded = [0u8; 4];
        encoded[..bytes.len()].copy_from_slice(bytes);
        u8::try_from(offset)
            .ok()
            .and_then(|offset| bytes[bytes.len() - 1].checked_add(offset))
            .and_then(|last| {
                encoded[bytes.len() - 1] = last;
                unicode(&encoded[..bytes.len()]).ok()
            })
    } else {
        None
    };
    if character.is_none() && policy == CMapPolicy::Complete {
        return Err(invalid("The font Unicode destination is unavailable: it is not one valid UTF-16 character or its range overflows."));
    }
    Ok(character)
}

// Parse the data sections ourselves: the general PDF CMap parser builds an
// unbounded reverse map and overwrites overlapping definitions. Both behaviors
// are unsuitable for positively proving an untrusted font's unique mapping.
#[cfg(test)]
pub(super) fn cmap(bytes: &[u8], code_len: usize) -> EngineResult<BTreeMap<u16, u32>> {
    Ok(cmap_with_policy(bytes, code_len, CMapPolicy::Complete)?
        .into_iter()
        .filter_map(|(code, character)| character.map(|c| (code, c as u32)))
        .collect())
}

fn parse<T>(
    bytes: &[u8],
    code_len: usize,
    mut destination: impl FnMut(&Object, u16) -> EngineResult<T>,
) -> EngineResult<BTreeMap<u16, T>> {
    if bytes.len() > MAX_CMAP {
        return Err(invalid(
            "The embedded font ToUnicode map exceeds its size limit.",
        ));
    }
    let content = lopdf::content::Content::decode(bytes)
        .map_err(|_| invalid("The embedded font ToUnicode map is malformed."))?;
    let mut result = BTreeMap::new();
    let mut codespaces = Vec::new();
    let mut section: Option<(&str, usize)> = None;
    for operation in &content.operations {
        let name = operation.operator.as_str();
        if name == "usecmap" {
            return Err(invalid("Inherited font CMaps cannot be verified safely."));
        }
        if matches!(name, "beginbfchar" | "beginbfrange" | "begincodespacerange") {
            if section.is_some() || operation.operands.len() != 1 {
                return Err(invalid("The embedded font CMap sections are ambiguous."));
            }
            let count = operation.operands[0]
                .as_i64()
                .map_err(|_| invalid("Invalid font CMap count."))?;
            if !(0..=65536).contains(&count) {
                return Err(invalid("The embedded font CMap exceeds its size limit."));
            }
            section = Some((name, count as usize));
        } else if matches!(name, "endbfchar" | "endbfrange" | "endcodespacerange") {
            let (start, count) = section
                .take()
                .ok_or_else(|| invalid("The embedded font CMap sections are incomplete."))?;
            let (expected, arity) = match name {
                "endbfchar" => ("beginbfchar", 2),
                "endbfrange" => ("beginbfrange", 3),
                _ => ("begincodespacerange", 2),
            };
            if start != expected || operation.operands.len() != count * arity {
                return Err(invalid(
                    "The embedded font CMap count does not match its entries.",
                ));
            }
            for item in operation.operands.chunks_exact(arity) {
                let first = hex(&item[0], code_len)?;
                if name == "endcodespacerange" {
                    let last = hex(&item[1], code_len)?;
                    if first > last {
                        return Err(invalid("Invalid font codespace range."));
                    }
                    if codespaces.len() >= 65536 {
                        return Err(invalid("The font codespaces exceed their size limit."));
                    }
                    codespaces.push((first, last));
                    continue;
                }
                let last = if name == "endbfchar" {
                    first
                } else {
                    hex(&item[1], code_len)?
                };
                if first > last || result.len() + usize::from(last - first) + 1 > 65536 {
                    return Err(invalid("The embedded font mapping exceeds its size limit."));
                }
                let target = &item[arity - 1];
                if let Object::Array(values) = target {
                    if name != "endbfrange" {
                        return Err(invalid(
                            "Only font Unicode ranges may have array destinations.",
                        ));
                    }
                    if values.len() != usize::from(last - first) + 1 {
                        return Err(invalid("The font Unicode mapping array is incomplete."));
                    }
                }
                for code in first..=last {
                    let character = if let Object::Array(values) = target {
                        destination(&values[usize::from(code - first)], 0)?
                    } else {
                        destination(target, code - first)?
                    };
                    if result.insert(code, character).is_some() {
                        return Err(invalid(
                            "The embedded font has ambiguous overlapping Unicode mappings.",
                        ));
                    }
                }
            }
        } else if section.is_some() {
            return Err(invalid(
                "The embedded font CMap contains unsupported instructions.",
            ));
        }
    }
    codespaces.sort_unstable();
    if codespaces.windows(2).any(|pair| pair[0].1 >= pair[1].0) {
        return Err(invalid("The embedded font codespaces overlap ambiguously."));
    }
    if section.is_some()
        || codespaces.is_empty()
        || result.is_empty()
        || result.keys().any(|code| {
            let index = codespaces.partition_point(|(first, _)| first <= code);
            index == 0 || codespaces[index - 1].1 < *code
        })
    {
        return Err(invalid(
            "The embedded font Unicode codespaces are incomplete.",
        ));
    }
    Ok(result)
}

pub(super) fn cmap_with_policy(
    bytes: &[u8],
    code_len: usize,
    policy: CMapPolicy,
) -> EngineResult<BTreeMap<u16, Option<char>>> {
    parse(bytes, code_len, |value, offset| {
        destination(value, offset, policy)
    })
}

#[derive(Debug, PartialEq)]
pub(super) enum SourceMapping {
    Scalar(char),
    Sequence(String),
    Unavailable,
}
pub(super) fn source_cmap(
    bytes: &[u8],
    code_len: usize,
) -> EngineResult<BTreeMap<u16, SourceMapping>> {
    let mut sequence_count = 0usize;
    let mut sequence_bytes = 0usize;
    let mut exceeded = false;
    let mut map = parse(bytes, code_len, |value, offset| {
        if let Some(character) = destination(value, offset, CMapPolicy::LatinEditing)? {
            return Ok(SourceMapping::Scalar(character));
        }
        // Parsing retains Unicode, not font authority. Language, shaping and
        // physical glyph/width evidence belong to the font verifier.
        let bytes = value
            .as_str()
            .map_err(|_| invalid("The font Unicode mapping contains a non-string destination."))?;
        if bytes.len() < 4 || bytes.len() > 64 || bytes.len() % 2 != 0 {
            return Ok(SourceMapping::Unavailable);
        }
        let mut encoded = [0u8; 64];
        encoded[..bytes.len()].copy_from_slice(bytes);
        let Some(last) = u8::try_from(offset)
            .ok()
            .and_then(|offset| bytes[bytes.len() - 1].checked_add(offset))
        else {
            return Ok(SourceMapping::Unavailable);
        };
        encoded[bytes.len() - 1] = last;
        let units: Vec<_> = encoded[..bytes.len()]
            .chunks_exact(2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]))
            .collect();
        let Ok(text) = String::from_utf16(&units) else {
            return Ok(SourceMapping::Unavailable);
        };
        if !(2..=16).contains(&text.chars().count()) || text.len() > 64 {
            return Ok(SourceMapping::Unavailable);
        }
        sequence_count += 1;
        sequence_bytes += text.len();
        exceeded |= sequence_count > 512 || sequence_bytes > 32768;
        Ok(if exceeded {
            SourceMapping::Unavailable
        } else {
            SourceMapping::Sequence(text)
        })
    })?;
    if exceeded {
        // Keep the existing scalar repertoire, and exclude ALL sequences rather
        // than letting CMap entry order choose which glyphs become eligible.
        for value in map.values_mut() {
            if matches!(value, SourceMapping::Sequence(_)) {
                *value = SourceMapping::Unavailable;
            }
        }
    }
    Ok(map)
}
#[cfg(test)]
#[path = "source_cmap_tests.rs"]
mod tests;
