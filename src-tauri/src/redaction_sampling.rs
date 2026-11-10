//! Conditional sampling support, never source-removal or renderer-choice authority.
use crate::{redaction_area_paths::AreaRect, redaction_content::require, EngineResult};

pub(crate) struct SampleRegions {
    width: u32,
    height: u32,
    cells: Vec<[u32; 4]>,
}
impl SampleRegions {
    /// Conditional full-grid stretch of an original-source-coordinate bound.
    /// The caller proves total reduction and destination source-pixel pitches.
    /// Fractional/grid-fit/matrix/affine effects are separate terms.
    pub fn into_stretch_envelope(
        self,
        total: u32,
        pitch_upper: [f64; 2],
    ) -> EngineResult<Vec<AreaRect>> {
        require(
            total.is_power_of_two() && total <= 64 && self.width <= 32768 && self.height <= 32768,
        )?;
        let dimensions = [f64::from(self.width), f64::from(self.height)];
        let mut margins = [0.; 2];
        for axis in 0..2 {
            let p = pitch_upper[axis];
            require(p.is_normal() && p > 0. && p <= dimensions[axis])?;
            let t = f64::from(total);
            let half = ((t + p).next_up() * 0.5).next_up();
            let margin = (t.max(p) + half).next_up();
            margins[axis] = (margin / dimensions[axis]).next_up().min(1.);
        }
        self.into_unit_regions()
            .into_iter()
            .map(|r| {
                crate::redaction_area_paths::ImageTransform::default()
                    .disclosure_bounds(r, margins, [0., 0.])
                    .map_err(|_| crate::redaction_content::invalid())
            })
            .collect()
    }
    pub fn new(width: u32, height: u32, cells: Vec<[u32; 4]>) -> EngineResult<Self> {
        require(
            (1..=1 << 24).contains(&width)
                && (1..=1 << 24).contains(&height)
                && cells.len() <= 4096,
        )?;
        require(
            cells
                .iter()
                .all(|&[l, t, r, b]| l < r && r <= width && t < b && b <= height),
        )?;
        Ok(Self {
            width,
            height,
            cells,
        })
    }
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    pub fn cells(&self) -> &[[u32; 4]] {
        &self.cells
    }
    /// Covers any total-factor-aligned source crop (including full-image cache
    /// hits) followed by fixed or overlapping extra box reduction. Incoming
    /// cells must already include decoder taps. Not a final page-space bound.
    pub fn source_envelope(
        mut self,
        [width, height]: [u32; 2],
        denominator: u32,
        extra: u32,
    ) -> EngineResult<Self> {
        require(
            matches!(denominator, 1 | 2 | 4 | 8)
                && extra.is_power_of_two()
                && extra <= 64 / denominator,
        )?;
        require(
            (1..=32768).contains(&width)
                && (1..=32768).contains(&height)
                && self.width == width.div_ceil(denominator)
                && self.height == height.div_ceil(denominator),
        )?;
        let total = denominator * extra;
        for [l, t, r, b] in &mut self.cells {
            *l = (*l / extra * total).saturating_sub(total - 1);
            *t = (*t / extra * total).saturating_sub(total - 1);
            *r = ((*r + extra - 1).div_ceil(extra) * total).min(width);
            *b = ((*b + extra - 1).div_ceil(extra) * total).min(height);
        }
        self.width = width;
        self.height = height;
        Ok(self)
    }
    /// Crop already-expanded support. Touching an excluded edge contributes no
    /// samples. The caller separately owns the tile's original-image frame.
    pub fn crop(mut self, [left, top, right, bottom]: [u32; 4]) -> EngineResult<Self> {
        require(left < right && right <= self.width && top < bottom && bottom <= self.height)?;
        self.cells.retain_mut(|[l, t, r, b]| {
            *l = (*l).max(left);
            *t = (*t).max(top);
            *r = (*r).min(right);
            *b = (*b).min(bottom);
            if *l >= *r || *t >= *b {
                return false;
            }
            *l -= left;
            *r -= left;
            *t -= top;
            *b -= top;
            true
        });
        self.width = right - left;
        self.height = bottom - top;
        Ok(self)
    }
    /// Ideal source-frame coordinates for a caller-proved actual tile window.
    /// Does not establish crop/cache policy or renderer matrix roundoff.
    pub fn into_source_regions(
        self,
        [source_width, source_height]: [u32; 2],
        [left, top, right, bottom]: [u32; 4],
    ) -> EngineResult<Vec<AreaRect>> {
        require((1..=1 << 24).contains(&source_width) && (1..=1 << 24).contains(&source_height))?;
        require(left < right && right <= source_width && top < bottom && bottom <= source_height)?;
        let (w, h) = (u64::from(self.width), u64::from(self.height));
        let (dx, dy) = (u64::from(source_width) * w, u64::from(source_height) * h);
        let x = |n| u64::from(left) * w + u64::from(n) * u64::from(right - left);
        let y = |n| dy - (u64::from(top) * h + u64::from(n) * u64::from(bottom - top));
        fn ratio(n: u64, d: u64, upper: bool) -> f64 {
            // Every integer is at most 2^48 and exactly representable in f64.
            let value = n as f64 / d as f64;
            if n == 0 || n == d || d.is_power_of_two() {
                value
            } else if upper {
                value.next_up()
            } else {
                value.next_down()
            }
        }
        Ok(self
            .cells
            .into_iter()
            .map(|[l, t, r, b]| AreaRect {
                left: ratio(x(l), dx, false),
                bottom: ratio(y(b), dy, false),
                right: ratio(x(r), dx, true),
                top: ratio(y(t), dy, true),
            })
            .collect())
    }
    /// Includes fixed groups and the pinned partial-edge overlapping groups.
    pub fn box_reduce(mut self, factor: u32) -> EngineResult<Self> {
        require(factor.is_power_of_two() && factor <= 64)?;
        let width = self.width.div_ceil(factor);
        let height = self.height.div_ceil(factor);
        let overlap_x = width * factor - self.width;
        let overlap_y = height * factor - self.height;
        for [l, t, r, b] in &mut self.cells {
            *l /= factor;
            *t /= factor;
            *r = (*r + overlap_x).div_ceil(factor).min(width);
            *b = (*b + overlap_y).div_ceil(factor).min(height);
        }
        self.width = width;
        self.height = height;
        Ok(self)
    }
    /// Full zero-origin integer-grid scaling. Does not model crop, grid fit,
    /// affine sampling, or fractional placement of the resulting bitmap.
    pub fn stretch(mut self, width: u32, height: u32) -> EngineResult<Self> {
        require(
            self.width <= 32768
                && self.height <= 32768
                && (1..=1 << 24).contains(&width)
                && (1..=1 << 24).contains(&height),
        )?;
        fn axis(left: u32, right: u32, source: u32, destination: u32) -> (u32, u32) {
            let (l, r, s, d) = (
                i64::from(left),
                i64::from(right),
                i64::from(source),
                i64::from(destination),
            );
            let support = d.max(s) + d;
            let low = (2 * l + 1) * d - 2 * support - s;
            let high = (2 * r - 1) * d + 2 * support - s;
            let first = -(-low).div_euclid(2 * s);
            let end = high.div_euclid(2 * s) + 1;
            (first.clamp(0, d) as u32, end.clamp(0, d) as u32)
        }
        for [l, t, r, b] in &mut self.cells {
            (*l, *r) = axis(*l, *r, self.width, width);
            (*t, *b) = axis(*t, *b, self.height, height);
        }
        self.width = width;
        self.height = height;
        Ok(self)
    }
    pub fn into_unit_regions(self) -> Vec<AreaRect> {
        fn ratio(n: u32, d: u32, upper: bool) -> f64 {
            let value = f64::from(n) / f64::from(d);
            if n == 0 || n == d || d.is_power_of_two() {
                value
            } else if upper {
                value.next_up()
            } else {
                value.next_down()
            }
        }
        self.cells
            .into_iter()
            .map(|[l, t, r, b]| AreaRect {
                left: ratio(l, self.width, false),
                bottom: ratio(self.height - b, self.height, false),
                right: ratio(r, self.width, true),
                top: ratio(self.height - t, self.height, true),
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "redaction_sampling_tests.rs"]
mod tests;
