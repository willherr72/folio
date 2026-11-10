//! Conditional PDFium post-stretcher sample influence, not source authority.
use super::{Interval, RendererTransform};
use crate::{redaction_content::require, redaction_sampling::SampleRegions, EngineResult};

const BASE: f64 = 256.;
fn point(value: f64) -> Interval {
    Interval([value; 2])
}

// All coefficients and local coordinates at this stage are integer-valued
// binary32 numbers. Small integer operations are exact. Otherwise one full
// ULP at an outward magnitude bound includes each rounded operation's error.
fn integer_roundoff(value: Interval) -> f64 {
    let magnitude = value.0[0].abs().max(value.0[1].abs());
    if magnitude <= (1_u32 << 24) as f64 {
        0.
    } else {
        f64::from((magnitude as f32).next_up()) - magnitude
    }
}

impl RendererTransform {
    /// `self` maps local result pixels to the actual clipped stretched bitmap.
    /// The caller proves the post-stretcher frame and its valid pixel storage.
    /// These bounds do not include prior stretching or decoder influence.
    pub fn pdfium_affine_bounds(
        self,
        samples: &SampleRegions,
        result: [i32; 4],
    ) -> EngineResult<Vec<[i32; 4]>> {
        let (sw, sh) = samples.dimensions();
        let width = i64::from(result[2]) - i64::from(result[0]);
        let height = i64::from(result[3]) - i64::from(result[1]);
        require(
            sw <= 32768
                && sh <= 32768
                && u64::from(sw) * u64::from(sh) * 4 <= i32::MAX as u64
                && (0..=1 << 24).contains(&width)
                && (0..=1 << 24).contains(&height),
        )?;
        let mut quantized = [point(0.); 6];
        for (out, value) in quantized.iter_mut().zip(self.0) {
            let scaled = value.mul(point(BASE))?;
            require(scaled.0[0] >= f64::from(i32::MIN) && scaled.0[1] <= f64::from(i32::MAX))?;
            *out = Interval::enclosing(scaled.0[0].round(), scaled.0[1].round())?;
        }
        let mut linear = Self::default();
        for i in 0..4 {
            linear.0[i] = Interval::enclosing(quantized[i].0[0] / BASE, quantized[i].0[1] / BASE)?;
        }
        let inverse = linear.reader_inverse()?;
        let x = Interval([0., (width - 1).max(0) as f64]);
        let y = Interval([0., (height - 1).max(0) as f64]);
        let mut errors = [0.; 2];
        for axis in 0..2 {
            let px = quantized[axis].mul(x)?;
            let py = quantized[axis + 2].mul(y)?;
            let sum = px.add(py)?;
            let translation = sum.add(quantized[axis + 4])?;
            let value = translation.add(point(BASE / 2.))?;
            require(value.0[0] >= f64::from(i32::MIN) && value.0[1] <= f64::from(i32::MAX))?;
            errors[axis] = [px, py, sum, translation, value]
                .into_iter()
                .map(integer_roundoff)
                .sum();
        }
        if width == 0 || height == 0 {
            return Ok(Vec::new());
        }
        let [a, b, c, d, _, _] = inverse.0;
        let mut output = Vec::with_capacity(samples.cells().len());
        for &[l, t, r, bot] in samples.cells() {
            let mut targets = [point(0.); 2];
            for (axis, (left, right, size)) in [(l, r, sw), (t, bot, sh)].into_iter().enumerate() {
                let first = left.saturating_sub(1);
                let last = if right == size { size } else { right - 1 };
                // Truncation toward zero lets (-1,0) select index zero.
                // At the other edge, index == size is admitted then clamped.
                let low = if first == 0 {
                    -BASE + 1.
                } else {
                    f64::from(first) * BASE
                };
                let high = f64::from(last + 1) * BASE - 1.;
                targets[axis] = Interval::enclosing(
                    (low - quantized[axis + 4].0[1] - BASE / 2. - errors[axis]) / BASE,
                    (high - quantized[axis + 4].0[0] - BASE / 2. + errors[axis]) / BASE,
                )?;
            }
            let offsets = [
                a.mul(targets[0])?.add(c.mul(targets[1])?)?,
                b.mul(targets[0])?.add(d.mul(targets[1])?)?,
            ];
            let mut rect = [0_i32; 4];
            for axis in 0..2 {
                let extent = if axis == 0 { width } else { height } as f64;
                let low = offsets[axis].0[0].ceil().clamp(0., extent) as i64;
                let high = (offsets[axis].0[1].floor() + 1.).clamp(0., extent) as i64;
                rect[axis] = (i64::from(result[axis]) + low) as i32;
                rect[axis + 2] = (i64::from(result[axis]) + high) as i32;
            }
            if rect[0] < rect[2] && rect[1] < rect[3] {
                output.push(rect);
            }
        }
        Ok(output)
    }
}
#[cfg(test)]
#[path = "redaction_render_pdfium_affine_tests.rs"]
mod tests;
