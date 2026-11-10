//! Source-bound decoded sample influence for the installed PDFium AGG device.
use super::{Coverage, CoverageBudget};
use crate::{
    redaction_content::require, redaction_render_sizing::pdfium_maximum,
    redaction_render_transform::RendererTransform, EngineResult,
};
use std::collections::BTreeSet;

const MAX_VARIANTS: usize = 4096;
const MAX_WORK: usize = 1_048_576;
const MAX_RECTANGLES: usize = 16384;

impl Coverage {
    /// `matrix` must enclose the actual image-unit-to-device operands. Device
    /// dimensions govern decoder reduction; image extents are not substitutes.
    /// The result is display influence only and never changes this MCU mask.
    pub fn pdfium_influence(
        &self,
        matrix: RendererTransform,
        device: [u32; 2],
        clip: [i32; 4],
        budget: &mut CoverageBudget,
    ) -> EngineResult<Vec<[i32; 4]>> {
        let s = self.shape;
        let maximum = pdfium_maximum([s.width, s.height], [s.mcu_width, s.mcu_height], device)?;
        require(
            clip[0] >= 0
                && clip[1] >= 0
                && clip[0] <= clip[2]
                && clip[1] <= clip[3]
                && clip[2] <= device[0] as i32
                && clip[3] <= device[1] as i32,
        )?;
        let mut output = BTreeSet::new();
        for denominator in [1, 2, 4, 8].into_iter().filter(|d| *d <= maximum) {
            CoverageBudget::charge(&mut budget.render_variants, 1, MAX_VARIANTS)?;
            let available = MAX_WORK.saturating_sub(budget.render_work);
            require(available > 0)?;
            // Includes cold decoding plus every retained higher-resolution
            // cache choice. The halo covers the admitted chroma filters.
            let samples = self.decoded_samples(denominator, [1, 1], budget)?;
            let mut remaining = available;
            let result = matrix.pdfium_image_bounds_budgeted(&samples, clip, &mut remaining);
            budget.render_work += available - remaining;
            // Failed calls retain consumed work. No partial result is returned.
            for rect in result? {
                if !output.contains(&rect) {
                    CoverageBudget::charge(&mut budget.render_rectangles, 1, MAX_RECTANGLES)?;
                    require(output.len() < 4096)?;
                    output.insert(rect);
                }
            }
        }
        Ok(output.into_iter().collect())
    }
}
