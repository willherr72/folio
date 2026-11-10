//! Preserve a nominal Tj's native float accumulation when one mixed show can
//! represent the removal exactly. Other layouts keep the established fallback.
use crate::EngineResult;
pub(crate) fn nominal_mixed_preserves(
    size: f32,
    count: usize,
    mut glyph: impl FnMut(usize) -> EngineResult<(f32, bool)>,
) -> EngineResult<bool> {
    // Keep this candidate within one output TJ array. Leading/trailing removal
    // would move the object origin or use the parser's separate final kerning.
    if count == 0 || count > 4094 || !size.is_finite() || size <= 0. {
        return Ok(false);
    }
    let mut original = 0f32;
    let mut candidate = 0f32;
    let mut pending = 0f32;
    for index in 0..count {
        let (width, removed) = glyph(index)?;
        if !width.is_finite() || width < 0. || (removed && (index == 0 || index + 1 == count)) {
            return Ok(false);
        }
        let advance = (width * size) / 1000.;
        if removed {
            pending -= width;
        } else {
            // PDFium accumulates adjacent TJ numbers before applying one
            // kerning. Use precisely that f32 operation order, not f64 totals
            // or an epsilon which can cross a rasterizer's phase threshold.
            candidate -= (pending * size) / 1000.;
            pending = 0.;
            if candidate != original {
                return Ok(false);
            }
            candidate += advance;
        }
        original += advance;
        if !original.is_finite() || !candidate.is_finite() || !pending.is_finite() {
            return Ok(false);
        }
    }
    Ok(candidate == original)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn courier(count: usize, start: usize, end: usize) -> bool {
        nominal_mixed_preserves(12., count, |index| {
            Ok((600., (start..end).contains(&index)))
        })
        .unwrap()
    }
    #[test]
    fn redaction_text_advances_keeps_one_show_when_middle_removal_matches_source_floats() {
        assert!(
            courier(17, 5, 11),
            "Left SECRET Right can keep its original object origin and float positions"
        );
    }
    #[test]
    fn redaction_text_advances_keeps_legacy_fallback_when_mixed_accumulation_changes() {
        assert!(
            !courier(31, 7, 25),
            "legacy Courier case needs per-glyph moves"
        );
        assert!(!courier(17, 0, 6));
        assert!(!courier(17, 11, 17));
    }
    #[test]
    fn redaction_text_advances_limits_lookahead_and_refuses_nonfinite_arithmetic() {
        assert!(
            !nominal_mixed_preserves(12., 4095, |_| panic!("must bound before traversal")).unwrap()
        );
        assert!(!nominal_mixed_preserves(f32::NAN, 3, |_| panic!("invalid size")).unwrap());
        assert!(!nominal_mixed_preserves(12., 3, |_| Ok((f32::INFINITY, false))).unwrap());
    }
}
