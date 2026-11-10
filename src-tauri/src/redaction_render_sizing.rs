//! Conditional decoder sizing. Matrix lower bounds and device ownership are
//! caller proofs, not inferred from ideal page placement here.
use crate::{redaction_content::require, EngineResult};

pub(crate) fn pdfium_maximum(
    source: [u32; 2],
    mcu: [u32; 2],
    device: [u32; 2],
) -> EngineResult<u32> {
    require(
        source.into_iter().all(|n| (1..=32768).contains(&n))
            && mcu.into_iter().all(|n| matches!(n, 8 | 16))
            && device.into_iter().all(|n| (1..=1 << 24).contains(&n)),
    )?;
    if source[0] % mcu[0] != 0 || source[1] % mcu[1] != 0 {
        return Ok(1);
    }
    let ratio = (source[0] / device[0]).min(source[1] / device[1]).max(1);
    let mut factor = 1;
    while factor < 8 && factor * 2 <= ratio {
        factor *= 2;
    }
    Ok(factor)
}

pub(crate) fn mupdf_maximum(source: [u32; 2], extent_lower: [u32; 2]) -> EngineResult<u32> {
    require(
        source.into_iter().all(|n| (1..=32768).contains(&n))
            && extent_lower.into_iter().all(|n| n <= 1 << 24),
    )?;
    // Actual zero extent disables reduction. If only a zero lower bound is
    // known, positive actual extents remain possible and must be covered.
    let lower = [
        extent_lower[0].max(1).min(source[0]),
        extent_lower[1].max(1).min(source[1]),
    ];
    let mut factor = 1;
    while factor < 64
        && source[0] / (factor * 2) >= lower[0] + 2
        && source[1] / (factor * 2) >= lower[1] + 2
    {
        factor *= 2;
    }
    Ok(factor)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redaction_render_sizing_uses_pdfium_device_and_preserves_partial_edge_policy() {
        assert_eq!(pdfium_maximum([64, 48], [16, 16], [16, 12]).unwrap(), 4);
        assert_eq!(pdfium_maximum([64, 48], [16, 16], [64, 48]).unwrap(), 1);
        assert_eq!(pdfium_maximum([64, 48], [16, 16], [1, 1]).unwrap(), 8);
        assert_eq!(pdfium_maximum([65, 49], [16, 16], [1, 1]).unwrap(), 1);
        assert_eq!(pdfium_maximum([64, 49], [8, 8], [1, 1]).unwrap(), 1);
        assert_eq!(
            pdfium_maximum([64, 48], [8, 8], [1 << 24, 1 << 24]).unwrap(),
            1
        );
    }
    #[test]
    fn redaction_render_sizing_mupdf_lower_extents_include_zero_and_higher_resolution() {
        assert_eq!(mupdf_maximum([256, 192], [64, 48]).unwrap(), 2);
        assert_eq!(mupdf_maximum([256, 192], [62, 46]).unwrap(), 4);
        assert_eq!(mupdf_maximum([256, 192], [2, 1]).unwrap(), 64);
        assert_eq!(mupdf_maximum([256, 192], [0, 0]).unwrap(), 64);
        assert_eq!(mupdf_maximum([19, 17], [0, 0]).unwrap(), 4);
        assert_eq!(mupdf_maximum([256, 192], [1 << 24, 1 << 24]).unwrap(), 1);
    }
    #[test]
    fn redaction_render_sizing_invalid_geometry_refuses() {
        for source in [[0, 48], [64, 0], [32769, 48]] {
            assert!(pdfium_maximum(source, [8, 8], [1, 1]).is_err());
            assert!(mupdf_maximum(source, [1, 1]).is_err());
        }
        for device in [[0, 1], [1, 0], [u32::MAX, 1]] {
            assert!(pdfium_maximum([64, 48], [8, 8], device).is_err());
        }
        assert!(pdfium_maximum([64, 48], [4, 8], [1, 1]).is_err());
        assert!(mupdf_maximum([64, 48], [u32::MAX, 1]).is_err());
    }
}
