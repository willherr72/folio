//! Source-bound JPEG influence for the pinned MuPDF draw device.
use super::{Coverage, CoverageBudget};
use crate::{
    redaction_content::require,
    redaction_render_transform::{GridFit, RendererTransform},
    EngineResult,
};
use std::collections::BTreeSet;
impl Coverage {
    /// Matrix uses native top-row image coordinates. Device/clip ownership is
    /// supplied by the caller; neither can expand the retained source mask.
    pub fn mupdf_influence(
        &self,
        matrix: RendererTransform,
        device: [u32; 2],
        clip: [i32; 4],
        mode: GridFit,
        budget: &mut CoverageBudget,
    ) -> EngineResult<Vec<[i32; 4]>> {
        require(
            device.into_iter().all(|n| (1..=1 << 24).contains(&n))
                && clip[0] >= 0
                && clip[1] >= 0
                && clip[0] <= clip[2]
                && clip[1] <= clip[3]
                && clip[2] <= device[0] as i32
                && clip[3] <= device[1] as i32,
        )?;
        CoverageBudget::charge(&mut budget.render_variants, 1, 4096)?;
        let available = 1_048_576_usize.saturating_sub(budget.render_work);
        let mut remaining = available;
        let result = (|| {
            let plans = matrix.mupdf_tiles_budgeted(
                [self.shape.width, self.shape.height],
                clip,
                mode,
                &mut remaining,
            )?;
            let mut out = BTreeSet::new();
            for tile in plans {
                CoverageBudget::charge(&mut budget.render_variants, 1, 4096)?;
                require(remaining > 0)?;
                remaining -= 1;
                let samples = self
                    .decoded_window_samples(tile.denominator, [1, 1], tile.window, budget)?
                    .box_reduce(tile.extra)?;
                for rect in tile.paint_bounds(&samples, clip, mode, &mut remaining)? {
                    if !out.contains(&rect) {
                        CoverageBudget::charge(&mut budget.render_rectangles, 1, 16384)?;
                        require(out.len() < 4096)?;
                        out.insert(rect);
                    }
                }
            }
            Ok(out.into_iter().collect())
        })();
        budget.render_work += available - remaining;
        result
    }
}
