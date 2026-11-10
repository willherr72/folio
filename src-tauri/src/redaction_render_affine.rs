//! Conditional raster influence; never source-removal authority.
use super::{Interval, RendererTransform};
use crate::{redaction_content::require, redaction_sampling::SampleRegions, EngineResult};

const ONE: i64 = 1 << 14;
const HALF: i64 = ONE / 2;
const DEVICE_LIMIT: i32 = 1 << 24;

fn point(v: f64) -> Interval {
    Interval([v; 2])
}
fn union(a: Interval, b: Interval) -> Interval {
    Interval([a.0[0].min(b.0[0]), a.0[1].max(b.0[1])])
}

// Native rectangle transforms sometimes use two selected corners instead of
// extrema of all four. Include each possible sign/classification branch.
fn unit_rectangle(matrix: RendererTransform) -> EngineResult<[Interval; 4]> {
    native_rectangle(matrix, [point(0.), point(0.), point(1.), point(1.)])
}

pub(super) fn native_rectangle(
    matrix: RendererTransform,
    rectangle: [Interval; 4],
) -> EngineResult<[Interval; 4]> {
    let [a, b, c, d, e, f] = matrix.0;
    let mut corners = [[point(0.); 2]; 4];
    for y in 0..2 {
        for x in 0..2 {
            corners[x + 2 * y] = [
                a.mul(rectangle[2 * x])?
                    .add(c.mul(rectangle[1 + 2 * y])?)?
                    .add(e)?,
                b.mul(rectangle[2 * x])?
                    .add(d.mul(rectangle[1 + 2 * y])?)?
                    .add(f)?,
            ];
        }
    }
    let mut result = [point(0.); 4];
    for axis in 0..2 {
        result[axis] = Interval([
            corners
                .iter()
                .map(|p| p[axis].0[0])
                .fold(f64::INFINITY, f64::min),
            corners
                .iter()
                .map(|p| p[axis].0[1])
                .fold(f64::INFINITY, f64::min),
        ]);
        result[axis + 2] = Interval([
            corners
                .iter()
                .map(|p| p[axis].0[0])
                .fold(f64::NEG_INFINITY, f64::max),
            corners
                .iter()
                .map(|p| p[axis].0[1])
                .fold(f64::NEG_INFINITY, f64::max),
        ]);
    }
    let near = |v: Interval| v.0[0] < f64::from(f32::EPSILON) && v.0[1] > -f64::from(f32::EPSILON);
    let possible = |v: Interval, negative: usize| {
        if negative == 1 {
            v.0[0] < 0.
        } else {
            v.0[1] >= 0.
        }
    };
    for (active, sx, sy) in [(near(b) && near(c), a, d), (near(a) && near(d), b, c)] {
        if !active {
            continue;
        }
        for y in 0..2 {
            for x in 0..2 {
                if possible(sx, x) && possible(sy, y) {
                    for axis in 0..2 {
                        result[axis] = union(result[axis], corners[x + 2 * y][axis]);
                        result[axis + 2] =
                            union(result[axis + 2], corners[(1 - x) + 2 * (1 - y)][axis]);
                    }
                }
            }
        }
    }
    require(
        result
            .iter()
            .all(|v| v.0[0] >= -f64::from(DEVICE_LIMIT) && v.0[1] <= f64::from(DEVICE_LIMIT)),
    )?;
    Ok(result)
}

fn truncated(v: Interval) -> [i64; 2] {
    v.0.map(|n| n.trunc() as i64)
}

// Native loops also increment after their last sampled column and final row.
// Validate those values and all interpolation additions with wide arithmetic.
fn check_steps(
    initial: [i64; 2],
    horizontal: [i64; 2],
    vertical: [i64; 2],
    width: i64,
    height: i64,
) -> EngineResult<()> {
    let mut lo = i128::from(initial[0]);
    let mut hi = i128::from(initial[1]);
    for (step, count) in [(horizontal, width), (vertical, height)] {
        let a = i128::from(step[0]) * i128::from(count);
        let b = i128::from(step[1]) * i128::from(count);
        lo += a.min(0);
        hi += b.max(0);
    }
    require(
        lo - i128::from(ONE) >= i128::from(i64::MIN)
            && hi + i128::from(ONE) <= i128::from(i64::MAX),
    )
}

impl RendererTransform {
    /// `self` is the actual post-scale pixmap frame. `scissor` must include
    /// destination, shape and group-alpha rectangular limits. This encloses
    /// both nearest and bilinear sampling, not prior decoder/filter changes.
    pub fn mupdf_affine_bounds(
        self,
        samples: &SampleRegions,
        scissor: [i32; 4],
    ) -> EngineResult<Vec<[i32; 4]>> {
        let (sw, sh) = samples.dimensions();
        require(
            sw <= 32768
                && sh <= 32768
                && scissor[0] <= scissor[2]
                && scissor[1] <= scissor[3]
                && scissor
                    .iter()
                    .all(|&n| (-DEVICE_LIMIT..=DEVICE_LIMIT).contains(&n)),
        )?;
        let rectangle = unit_rectangle(self)?;
        let mut scale = Self::default();
        scale.0[0] = point(1.).div(point(f64::from(sw)))?;
        scale.0[3] = point(1.).div(point(f64::from(sh)))?;
        let inverse = self.concat(scale)?.reader_inverse()?;
        let mut scaled = [point(0.); 6];
        for (out, value) in scaled.iter_mut().zip(inverse.0) {
            *out = value.mul(point(ONE as f64))?;
        }
        let fixed = scaled.map(truncated);
        let mut linear = Self::default();
        for i in 0..4 {
            linear.0[i] = Interval::enclosing(
                fixed[i][0] as f64 / ONE as f64,
                fixed[i][1] as f64 / ONE as f64,
            )?;
        }
        let forward = linear.reader_inverse()?;
        let mut domain = [0_i32; 4];
        let mut origin = [[0_i64; 2]; 2];
        for axis in 0..2 {
            domain[axis] = (rectangle[axis].0[0].floor() as i32).max(scissor[axis]);
            domain[axis + 2] = (rectangle[axis + 2].0[1].ceil() as i32).min(scissor[axis + 2]);
            origin[axis] = [
                i64::from(domain[axis]),
                i64::from((rectangle[axis].0[1].floor() as i32).max(scissor[axis])),
            ];
        }
        if domain[0] >= domain[2] || domain[1] >= domain[3] {
            return Ok(Vec::new());
        }
        for axis in 0..2 {
            origin[axis][1] = origin[axis][1].min(i64::from(domain[axis + 2]) - 1);
        }
        let x = Interval::enclosing(origin[0][0] as f64, origin[0][1] as f64)?;
        let y = Interval::enclosing(origin[1][0] as f64, origin[1][1] as f64)?;
        let mut initial = [[0; 2]; 2];
        for axis in 0..2 {
            let value = scaled[axis]
                .mul(x)?
                .add(scaled[axis + 2].mul(y)?)?
                .add(scaled[axis + 4])?
                .add(scaled[axis].add(scaled[axis + 2])?.mul(point(0.5))?)?;
            require(value.0[0] >= f64::from(i32::MIN) && value.0[1] <= f64::from(i32::MAX))?;
            initial[axis] = truncated(value);
            check_steps(
                initial[axis],
                fixed[axis],
                fixed[axis + 2],
                i64::from(domain[2]) - origin[0][0],
                i64::from(domain[3]) - origin[1][0],
            )?;
        }
        let [a, b, c, d, _, _] = forward.0;
        let mut result = Vec::with_capacity(samples.cells().len());
        for &[l, t, r, bot] in samples.cells() {
            let mut targets = [point(0.); 2];
            for (axis, (low, high, size)) in [(l, r, sw), (t, bot, sh)].into_iter().enumerate() {
                let low = (i64::from(low) * ONE - HALF).max(0);
                let high = (i64::from(high) * ONE + HALF).min(i64::from(size) * ONE) - 1;
                targets[axis] = Interval::enclosing(
                    (low - initial[axis][1]) as f64 / ONE as f64,
                    (high - initial[axis][0]) as f64 / ONE as f64,
                )?;
            }
            let offsets = [
                a.mul(targets[0])?.add(c.mul(targets[1])?)?,
                b.mul(targets[0])?.add(d.mul(targets[1])?)?,
            ];
            let mut bounds = [0_i32; 4];
            for axis in 0..2 {
                let lower = offsets[axis].0[0].ceil() + origin[axis][0] as f64;
                let upper = offsets[axis].0[1].floor() + origin[axis][1] as f64 + 1.;
                bounds[axis] = lower
                    .max(f64::from(domain[axis]))
                    .min(f64::from(domain[axis + 2])) as i32;
                bounds[axis + 2] = upper
                    .max(f64::from(domain[axis]))
                    .min(f64::from(domain[axis + 2])) as i32;
            }
            if bounds[0] < bounds[2] && bounds[1] < bounds[3] {
                result.push(bounds);
            }
        }
        Ok(result)
    }
}
#[cfg(test)]
#[path = "redaction_render_affine_tests.rs"]
mod tests;
