//! Encode complete characters and retain explicit TJ adjustments at source slots.
use super::{invalid, EngineResult};
use lopdf::{content::Operation, Object, StringFormat};

fn adjustment(value: &Object) -> EngineResult<()> {
    if !value
        .as_float()
        .is_ok_and(|n| n.is_finite() && n.abs() <= 1_000_000.)
    {
        return Err(invalid("Unsupported positioned text adjustment."));
    }
    Ok(())
}

pub(super) fn boundaries(
    text: &str,
    mut encode: impl FnMut(&str) -> EngineResult<Vec<u8>>,
) -> EngineResult<Vec<usize>> {
    let mut result: Vec<usize> = vec![0];
    for character in text.chars() {
        let encoded = encode(character.encode_utf8(&mut [0; 4]))?;
        if encoded.is_empty() {
            return Err(invalid("A character has no encoded bytes."));
        }
        result.push(
            result
                .last()
                .unwrap()
                .checked_add(encoded.len())
                .ok_or_else(|| invalid("Encoded character boundaries exceed safe limits."))?,
        );
    }
    Ok(result)
}

fn validate_boundaries(boundaries: &[usize], length: usize) -> EngineResult<()> {
    if boundaries.first() != Some(&0)
        || boundaries.last() != Some(&length)
        || boundaries.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(invalid("Invalid encoded character boundaries."));
    }
    Ok(())
}

pub(super) fn validate_segments(
    values: &[Object],
    expected: &[u8],
    boundaries: &[usize],
) -> EngineResult<()> {
    validate_boundaries(boundaries, expected.len())?;
    let mut offset = 0usize;
    for value in values {
        if let Ok(bytes) = value.as_str() {
            let end = offset
                .checked_add(bytes.len())
                .ok_or_else(|| invalid("Encoded text exceeds safe limits."))?;
            if expected.get(offset..end) != Some(bytes) {
                return Err(invalid("Ambiguous positioned text mapping."));
            }
            if boundaries.binary_search(&end).is_err() {
                return Err(invalid("A TJ segment splits an encoded character."));
            }
            offset = end;
        } else {
            adjustment(value)?;
        }
    }
    if offset != expected.len() {
        return Err(invalid("Ambiguous positioned text mapping."));
    }
    Ok(())
}

pub(super) fn rewrite_array(
    values: &[Object],
    expected: &[u8],
    changed: &[u8],
    source_boundaries: &[usize],
    replacement_boundaries: &[usize],
    same_text: bool,
) -> EngineResult<Vec<Object>> {
    validate_segments(values, expected, source_boundaries)?;
    validate_boundaries(replacement_boundaries, changed.len())?;
    if expected.is_empty()
        || changed.is_empty()
        || (same_text && source_boundaries.len() != replacement_boundaries.len())
    {
        return Err(invalid("Invalid positioned text replacement."));
    }
    if same_text && expected == changed && source_boundaries == replacement_boundaries {
        return Ok(values.to_vec());
    }
    let mut old_offset = 0;
    let mut new_offset = 0;
    let mut output = Vec::with_capacity(values.len() + 1);
    for value in values {
        if let Ok(text) = value.as_str() {
            old_offset += text.len(); // validate_segments proved the bounded sum.
            let slot = source_boundaries
                .binary_search(&old_offset)
                .map_err(|_| invalid("A TJ segment splits an encoded character."))?;
            let end = replacement_boundaries[slot.min(replacement_boundaries.len() - 1)];
            output.push(Object::String(
                changed[new_offset..end].to_vec(),
                StringFormat::Hexadecimal,
            ));
            new_offset = end;
        } else if new_offset < changed.len() || same_text {
            output.push(value.clone());
        }
    }
    if new_offset < changed.len() {
        output.push(Object::String(
            changed[new_offset..].to_vec(),
            StringFormat::Hexadecimal,
        ));
    }
    Ok(output)
}

/// Form admission bounds, separate from the existing top-level page policy.
pub(super) fn encoded_bytes(operation: &Operation) -> EngineResult<Vec<u8>> {
    let mut result = Vec::new();
    match (operation.operator.as_str(), operation.operands.as_slice()) {
        ("Tj", [Object::String(bytes, _)]) => {
            if bytes.len() > 8192 {
                return Err(invalid("Form text exceeds the encoded character limit."));
            }
            result.extend_from_slice(bytes);
        }
        ("TJ", [Object::Array(values)]) if values.len() <= 16_384 => {
            for value in values {
                if let Ok(bytes) = value.as_str() {
                    if bytes.len() > 8192 - result.len() {
                        return Err(invalid("Form text exceeds the encoded character limit."));
                    }
                    result.extend_from_slice(bytes);
                } else {
                    adjustment(value)?;
                }
            }
        }
        _ => return Err(invalid("Malformed or excessive form text-show operands.")),
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn s(bytes: &[u8]) -> Object {
        Object::String(bytes.to_vec(), StringFormat::Hexadecimal)
    }
    fn map(bytes: &[u8]) -> Vec<usize> {
        (0..=bytes.len()).collect()
    }
    #[test]
    fn text_show_keeps_slots_leading_consecutive_and_trailing_adjustments() {
        let values = vec![
            25.into(),
            s(b"AB"),
            (-20).into(),
            10.into(),
            s(b"CD"),
            45.into(),
        ];
        for (changed, expected) in [
            (
                b"XYZWuv".as_slice(),
                vec![
                    25.into(),
                    s(b"XY"),
                    (-20).into(),
                    10.into(),
                    s(b"ZW"),
                    45.into(),
                    s(b"uv"),
                ],
            ),
            (b"XY".as_slice(), vec![25.into(), s(b"XY"), s(b"")]),
            (b"X".as_slice(), vec![25.into(), s(b"X"), s(b"")]),
            (
                b"XYZW".as_slice(),
                vec![25.into(), s(b"XY"), (-20).into(), 10.into(), s(b"ZW")],
            ),
        ] {
            assert_eq!(
                rewrite_array(
                    &values,
                    b"ABCD",
                    changed,
                    &map(b"ABCD"),
                    &map(changed),
                    false
                )
                .unwrap(),
                expected
            );
        }
        assert_eq!(
            rewrite_array(
                &values,
                b"ABCD",
                b"ABCD",
                &map(b"ABCD"),
                &map(b"ABCD"),
                true
            )
            .unwrap(),
            values
        );
    }
    #[test]
    fn text_show_reencodes_logical_boundaries_across_code_widths() {
        let old = vec![s(b"\0A"), Object::Real(12.5), s(b"\0B\0C"), (-40).into()];
        assert_eq!(
            rewrite_array(
                &old,
                b"\0A\0B\0C",
                b"XYZW",
                &[0, 2, 4, 6],
                &[0, 1, 2, 3, 4],
                false
            )
            .unwrap(),
            vec![s(b"X"), Object::Real(12.5), s(b"YZ"), (-40).into(), s(b"W")]
        );
        let single = vec![s(b"A"), 0.into(), s(b"BC"), 40.into()];
        assert_eq!(
            rewrite_array(
                &single,
                b"ABC",
                b"\0A\0B\0C",
                &[0, 1, 2, 3],
                &[0, 2, 4, 6],
                true
            )
            .unwrap(),
            vec![s(b"\0A"), 0.into(), s(b"\0B\0C"), 40.into()]
        );
        assert_eq!(
            boundaries("Aé", |c| Ok(c
                .encode_utf16()
                .flat_map(u16::to_be_bytes)
                .collect()))
            .unwrap(),
            [0, 2, 4]
        );
    }
    #[test]
    fn text_show_noop_retains_empty_segments_and_original_string_objects() {
        let values = vec![
            s(b""),
            7.into(),
            Object::string_literal("AB"),
            s(b""),
            Object::Real(-2.25),
            s(b"C"),
            9.into(),
        ];
        assert_eq!(
            rewrite_array(&values, b"ABC", b"ABC", &map(b"ABC"), &map(b"ABC"), true).unwrap(),
            values
        );
    }
    #[test]
    fn text_show_rejects_invalid_maps_split_codes_and_source_mismatch() {
        for invalid_map in [
            vec![],
            vec![1, 2],
            vec![0, 1],
            vec![0, 2, 1, 2],
            vec![0, 0, 2],
            vec![0, usize::MAX, 2],
        ] {
            assert!(
                rewrite_array(&[s(b"AB")], b"AB", b"CD", &invalid_map, &[0, 1, 2], false).is_err()
            );
            assert!(
                rewrite_array(&[s(b"AB")], b"AB", b"CD", &[0, 1, 2], &invalid_map, false).is_err()
            );
        }
        assert!(validate_segments(&[s(b"\0"), 12.into(), s(b"A")], b"\0A", &[0, 2]).is_err());
        assert!(validate_segments(&[s(b"AX")], b"AB", &[0, 1, 2]).is_err());
        assert!(validate_segments(&[s(b"A")], b"AB", &[0, 1, 2]).is_err());
        assert!(boundaries("A", |_| Ok(vec![])).is_err());
    }
    #[test]
    fn text_show_rejects_bad_adjustments_and_bounds_form_operands() {
        for value in [
            Object::Real(f32::NAN),
            Object::Real(f32::INFINITY),
            Object::Real(-f32::INFINITY),
            1_000_001.into(),
            (-1_000_001).into(),
            Object::Null,
            Object::Array(vec![]),
            Object::Name(b"Bad".to_vec()),
        ] {
            assert!(validate_segments(&[s(b"A"), value.clone()], b"A", &[0, 1]).is_err());
            assert!(encoded_bytes(&Operation::new(
                "TJ",
                vec![Object::Array(vec![s(b"A"), value])]
            ))
            .is_err());
        }
        for values in [vec![0.into(); 16_385], vec![s(&vec![b'A'; 8193])]] {
            assert!(encoded_bytes(&Operation::new("TJ", vec![Object::Array(values)])).is_err());
        }
        assert!(encoded_bytes(&Operation::new("TJ", vec![s(b"A")])).is_err());
        assert!(encoded_bytes(&Operation::new("Tj", vec![s(b"A"), s(b"B")])).is_err());
        assert_eq!(
            encoded_bytes(&Operation::new(
                "TJ",
                vec![Object::Array(vec![1.into(), (-1).into()])]
            ))
            .unwrap(),
            Vec::<u8>::new()
        );
    }
}
