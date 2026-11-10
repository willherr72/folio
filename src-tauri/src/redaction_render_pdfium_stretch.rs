//! Conditional signed/clipped stretching support; never source-removal authority.
use super::Interval;
use crate::{redaction_content::require, redaction_sampling::SampleRegions, EngineResult};

fn point(value: f64) -> Interval {
    Interval([value; 2])
}

struct Axis {
    length: u32,
    negative: bool,
    clip: [u32; 2],
    scale: Interval,
    base: Interval,
    first_source: [u32; 2],
    last_source: [u32; 2],
}
impl Axis {
    fn new(source: u32, destination: i32, clip: [u32; 2]) -> EngineResult<Self> {
        let length = destination.unsigned_abs();
        require(
            source <= 32768
                && (1..=1 << 24).contains(&length)
                && clip[0] < clip[1]
                && clip[1] <= length,
        )?;
        let mut scale = point(f64::from(source)).div(point(f64::from(length)))?;
        let negative = destination < 0;
        if negative {
            scale = Interval([-scale.0[1], -scale.0[0]]);
        }
        // Source clipping uses a binary32 quotient, converted to double for
        // the products. The same interval also covers the weight table's
        // binary64 quotient. Keep its actual negative-scale evaluation order.
        let clip_base = if negative { f64::from(destination) } else { 0. };
        let p = scale.mul(point(f64::from(clip[0]) + clip_base))?;
        let q = scale.mul(point(f64::from(clip[1]) + clip_base))?;
        let clamp = |v: f64| v.clamp(0., f64::from(source)) as u32;
        let first_source = [
            clamp(p.0[0].min(q.0[0]).floor()),
            clamp(p.0[1].min(q.0[1]).floor()),
        ];
        let end_source = [
            clamp(p.0[0].max(q.0[0]).ceil()),
            clamp(p.0[1].max(q.0[1]).ceil()),
        ];
        // Every possible source window must have real stored pixels. This is
        // conservative when float clipping lies on an uncertain tiny boundary.
        require(first_source[1] < end_source[0])?;
        Ok(Self {
            length,
            negative,
            clip,
            scale,
            base: point(if negative { f64::from(source) } else { 0. }),
            first_source,
            last_source: [end_source[0] - 1, end_source[1] - 1],
        })
    }
    fn window(&self, ordered: u32) -> EngineResult<[i64; 2]> {
        let physical = if self.negative {
            self.length - 1 - ordered
        } else {
            ordered
        };
        let product = self.scale.mul(point(f64::from(physical)))?;
        let start = product.add(self.base)?;
        let end = start.add(self.scale)?;
        let center = product.add(self.scale.mul(point(0.5))?)?.add(self.base)?;
        let bilinear_low = center.add(point(-0.5))?;
        let bilinear_high = center.add(point(0.5))?;
        Ok([
            start.0[0].min(end.0[0]).min(bilinear_low.0[0]).floor() as i64,
            start.0[1].max(end.0[1]).max(bilinear_high.0[1]).floor() as i64,
        ])
    }
    fn affected(&self, low: u32, high: u32) -> EngineResult<Option<[u32; 2]>> {
        let [begin, end] = if self.negative {
            [self.length - self.clip[1], self.length - self.clip[0]]
        } else {
            self.clip
        };
        let overlaps = |range: [u32; 2]| low <= range[1] && range[0] < high;
        // Clamps and fallback can reuse a source-window boundary even when
        // the unbounded filter window lies beyond it. Preserve that influence.
        let first = if overlaps(self.first_source) {
            begin
        } else {
            first_true(begin, end, |p| Ok(self.window(p)?[1] >= i64::from(low)))?
        };
        let last = if overlaps(self.last_source) {
            end
        } else {
            first_true(begin, end, |p| Ok(self.window(p)?[0] >= i64::from(high)))?
        };
        if first >= last {
            return Ok(None);
        }
        let physical = if self.negative {
            [self.length - last, self.length - first]
        } else {
            [first, last]
        };
        Ok(Some([
            physical[0] - self.clip[0],
            physical[1] - self.clip[0],
        ]))
    }
}

fn first_true(
    mut low: u32,
    mut high: u32,
    mut predicate: impl FnMut(u32) -> EngineResult<bool>,
) -> EngineResult<u32> {
    while low < high {
        let middle = low + (high - low) / 2;
        if predicate(middle)? {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    Ok(low)
}

/// Exact signed stretch sizes and clip are caller-proved native stage inputs.
/// Output coordinates are local to the clipped stretched bitmap. Includes all
/// admitted smoothing choices, without changing source-removal selections.
pub(crate) fn pdfium_stretch_bounds(
    samples: &SampleRegions,
    destination: [i32; 2],
    clip: [u32; 4],
) -> EngineResult<SampleRegions> {
    let (sw, sh) = samples.dimensions();
    let x = Axis::new(sw, destination[0], [clip[0], clip[2]])?;
    let y = Axis::new(sh, destination[1], [clip[1], clip[3]])?;
    // Validate candidate arithmetic even when no changed samples remain.
    for axis in [&x, &y] {
        axis.window(0)?;
        axis.window(axis.length - 1)?;
    }
    let mut cells = Vec::with_capacity(samples.cells().len());
    for &[left, top, right, bottom] in samples.cells() {
        if let (Some(x), Some(y)) = (x.affected(left, right)?, y.affected(top, bottom)?) {
            cells.push([x[0], y[0], x[1], y[1]]);
        }
    }
    SampleRegions::new(clip[2] - clip[0], clip[3] - clip[1], cells)
}
#[cfg(test)]
#[path = "redaction_render_pdfium_stretch_tests.rs"]
mod tests;
