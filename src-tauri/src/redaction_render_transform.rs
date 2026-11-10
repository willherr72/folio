//! Forward arithmetic for caller-proved operands, not source-token admission.
//! Each intermediate contains its exact result and binary32 rounding, including
//! fused evaluation that omits intermediate rounding. Endpoints are binary32
//! values stored in binary64. Subnormal ranges include flushing to zero.
//! Inverse transforms and raster coverage require separate proofs.
use crate::{redaction_area_paths::AreaRect, redaction_content::require, EngineResult};

const LIMIT: f64 = (1_u64 << 60) as f64;

#[derive(Clone, Copy)]
struct Interval([f64; 2]);

impl Interval {
    fn div(self, denominator: Self) -> EngineResult<Self> {
        require(denominator.0[0] >= f64::from(f32::MIN_POSITIVE))?;
        let mut low = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        for n in self.0 {
            for d in denominator.0 {
                let nearest = (n / d) as f32;
                require(nearest.is_finite())?;
                // Exact product of two binary32 endpoints tests the direction
                // of quotient rounding, including signed numerators.
                let product = f64::from(nearest) * d;
                low = low.min(f64::from(if product > n {
                    nearest.next_down()
                } else {
                    nearest
                }));
                high = high.max(f64::from(if product < n {
                    nearest.next_up()
                } else {
                    nearest
                }));
            }
        }
        Self::enclosing(low, high)
    }
    fn enclosing(mut low: f64, mut high: f64) -> EngineResult<Self> {
        require(
            low.is_finite() && high.is_finite() && low <= high && low >= -LIMIT && high <= LIMIT,
        )?;
        let a = low as f32;
        let b = high as f32;
        low = f64::from(if f64::from(a) > low { a.next_down() } else { a });
        high = f64::from(if f64::from(b) < high { b.next_up() } else { b });
        let normal = f64::from(f32::MIN_POSITIVE);
        if low > 0. && low < normal {
            low = 0.;
        }
        if high < 0. && high > -normal {
            high = 0.;
        }
        require(low >= -LIMIT && high <= LIMIT)?;
        Ok(Self([low, high]))
    }

    fn add(self, other: Self) -> EngineResult<Self> {
        // TwoSum preserves the direction of binary64 rounding even for widely
        // separated exponents. These bounded binary32 operands and their sums
        // are far from binary64 overflow or underflow.
        fn directed(a: f64, b: f64, lower: bool) -> f64 {
            let sum = a + b;
            let bv = sum - a;
            let residual = (a - (sum - bv)) + (b - bv);
            if lower && residual < 0. {
                sum.next_down()
            } else if !lower && residual > 0. {
                sum.next_up()
            } else {
                sum
            }
        }
        Self::enclosing(
            directed(self.0[0], other.0[0], true),
            directed(self.0[1], other.0[1], false),
        )
    }

    fn mul(self, other: Self) -> EngineResult<Self> {
        // Products of binary32 endpoints are exact in binary64.
        let products = [
            self.0[0] * other.0[0],
            self.0[0] * other.0[1],
            self.0[1] * other.0[0],
            self.0[1] * other.0[1],
        ];
        Self::enclosing(
            products.into_iter().fold(f64::INFINITY, f64::min),
            products.into_iter().fold(f64::NEG_INFINITY, f64::max),
        )
    }
}

#[derive(Clone, Copy)]
pub(crate) struct RendererTransform([Interval; 6]);
impl Default for RendererTransform {
    fn default() -> Self {
        Self([1., 0., 0., 1., 0., 0.].map(|n| Interval([n, n])))
    }
}
impl RendererTransform {
    /// Precompose an actual returned tile window in native top-row coordinates.
    /// A requested crop is not proof of this frame: cache hits can return a
    /// complete image. The page-to-device caller owns initial image orientation.
    pub fn mupdf_tile_frame(
        self,
        [width, height]: [u32; 2],
        window: [u32; 4],
    ) -> EngineResult<Self> {
        let [left, top, right, bottom] = window;
        require(
            (1..=32768).contains(&width)
                && (1..=32768).contains(&height)
                && left < right
                && right <= width
                && top < bottom
                && bottom <= height,
        )?;
        if window == [0, 0, width, height] {
            return Ok(self);
        }
        let ratio = |n: u32, d: u32| Interval([f64::from(n); 2]).div(Interval([f64::from(d); 2]));
        self.concat(Self([
            ratio(right - left, width)?,
            Interval([0.; 2]),
            Interval([0.; 2]),
            ratio(bottom - top, height)?,
            ratio(left, width)?,
            ratio(top, height)?,
        ]))
    }
    /// Caller must prove the renderer operands. Parsing numeric tokens through
    /// a different PDF library does not establish this correspondence.
    pub fn known(values: [f32; 6]) -> EngineResult<Self> {
        let mut result = Self::default();
        for (out, value) in result.0.iter_mut().zip(values) {
            *out = Interval::enclosing(f64::from(value), f64::from(value))?;
        }
        Ok(result)
    }
    pub fn coefficients(self) -> [[f64; 2]; 6] {
        self.0.map(|v| v.0)
    }
    pub fn concat(self, local: Self) -> EngineResult<Self> {
        let [a, b, c, d, e, f] = self.0;
        let [g, h, i, j, k, l] = local.0;
        Ok(Self([
            a.mul(g)?.add(c.mul(h)?)?,
            b.mul(g)?.add(d.mul(h)?)?,
            a.mul(i)?.add(c.mul(j)?)?,
            b.mul(i)?.add(d.mul(j)?)?,
            a.mul(k)?.add(c.mul(l)?)?.add(e)?,
            b.mul(k)?.add(d.mul(l)?)?.add(f)?,
        ]))
    }
    pub fn unit_bounds(self, rect: AreaRect) -> EngineResult<AreaRect> {
        require(
            [rect.left, rect.bottom, rect.right, rect.top]
                .into_iter()
                .all(f64::is_finite)
                && 0. <= rect.left
                && rect.left < rect.right
                && rect.right <= 1.
                && 0. <= rect.bottom
                && rect.bottom < rect.top
                && rect.top <= 1.,
        )?;
        self.rectangle_bounds(rect)
    }
    pub fn rectangle_bounds(self, rect: AreaRect) -> EngineResult<AreaRect> {
        require(
            [rect.left, rect.bottom, rect.right, rect.top]
                .into_iter()
                .all(|n| n.is_finite() && n.abs() <= LIMIT)
                && rect.left < rect.right
                && rect.bottom < rect.top,
        )?;
        let [a, b, c, d, e, f] = self.0;
        let mut out = AreaRect {
            left: f64::INFINITY,
            bottom: f64::INFINITY,
            right: f64::NEG_INFINITY,
            top: f64::NEG_INFINITY,
        };
        for x in [rect.left, rect.right] {
            for y in [rect.bottom, rect.top] {
                let x = Interval::enclosing(x, x)?;
                let y = Interval::enclosing(y, y)?;
                let px = a.mul(x)?.add(c.mul(y)?)?.add(e)?.0;
                let py = b.mul(x)?.add(d.mul(y)?)?.add(f)?.0;
                out.left = out.left.min(px[0]);
                out.right = out.right.max(px[1]);
                out.bottom = out.bottom.min(py[0]);
                out.top = out.top.max(py[1]);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
#[path = "redaction_render_transform_tests.rs"]
mod tests;

#[path = "redaction_render_placement.rs"]
mod placement;
pub(crate) use placement::{GridFit, ImageSizing};

#[path = "redaction_render_scale.rs"]
mod scale;
pub(crate) use scale::mupdf_scale_bounds;

#[cfg(test)]
#[path = "redaction_render_tile_tests.rs"]
mod tile_tests;
#[cfg(test)]
pub(crate) use tile_tests::{record_tile_case, tile_case_header};

#[path = "redaction_render_inverse.rs"]
mod inverse;

#[path = "redaction_render_affine.rs"]
mod affine;

#[path = "redaction_render_pdfium_affine.rs"]
mod pdfium_affine;

#[path = "redaction_render_pdfium_stretch.rs"]
mod pdfium_stretch;
pub(crate) use pdfium_stretch::pdfium_stretch_bounds;

#[path = "redaction_render_pdfium_image.rs"]
mod pdfium_image;

#[path = "redaction_render_mupdf_tiles.rs"]
mod mupdf_tiles;

#[path = "redaction_render_page.rs"]
mod page;
pub(crate) use page::PageDevice;
