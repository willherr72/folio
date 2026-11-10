//! Conditional complete-pixel influence for native fractional pixmap scaling.
use super::Interval;
use crate::{redaction_content::require, redaction_sampling::SampleRegions, EngineResult};

pub(crate) fn mupdf_scale_bounds(
    samples: &SampleRegions,
    origin: [[f64; 2]; 2],
    extent: [[f64; 2]; 2],
    clip: Option<[i32; 4]>,
) -> EngineResult<Vec<[i32; 4]>> {
    let (width, height) = samples.dimensions();
    require(width <= 32768 && height <= 32768)?;
    let x = plans(origin[0], extent[0], width, true)?;
    let y = plans(origin[1], extent[1], height, false)?;
    if let Some([l, t, r, b]) = clip {
        require(l <= r && t <= b)?;
    }
    let mut result = Vec::with_capacity(samples.cells().len());
    for &[l, t, r, b] in samples.cells() {
        let Some([left, right]) = axis_bounds(&x, l, r)? else {
            continue;
        };
        let Some([top, bottom]) = axis_bounds(&y, t, b)? else {
            continue;
        };
        let mut rect = [left, top, right, bottom];
        if let Some([l, t, r, b]) = clip {
            rect = [left.max(l), top.max(t), right.min(r), bottom.min(b)];
        }
        if rect[0] < rect[2] && rect[1] < rect[3] {
            result.push(rect);
        }
    }
    Ok(result)
}

fn point(v: f64) -> Interval {
    Interval([v, v])
}
fn sub(a: Interval, b: Interval) -> EngineResult<Interval> {
    a.add(Interval([-b.0[1], -b.0[0]]))
}
fn input(v: [f64; 2]) -> EngineResult<Interval> {
    require(
        v[0] <= v[1]
            && v.into_iter()
                .all(|x| x.is_finite() && x.abs() <= 16777216. && f64::from(x as f32) == x),
    )?;
    Ok(Interval(v))
}

struct AxisPlan {
    source: u32,
    phase: Interval,
    size: Interval,
    window: Interval,
    base: Interval,
    count: u32,
    reverse_output: bool,
    reverse_source: bool,
}

fn plans(
    origin: [f64; 2],
    extent: [f64; 2],
    source: u32,
    horizontal: bool,
) -> EngineResult<Vec<AxisPlan>> {
    let origin = input(origin)?;
    let extent = input(extent)?;
    let mut result = Vec::with_capacity(2);
    for negative in [false, true] {
        if (negative && extent.0[0] >= 0.) || (!negative && extent.0[1] < 0.) {
            continue;
        }
        let size = if negative {
            Interval([(-extent.0[1].min(0.)).max(1.), (-extent.0[0]).max(1.)])
        } else {
            Interval([extent.0[0].max(1.), extent.0[1].max(1.)])
        };
        let rounded = if negative {
            Interval(origin.0.map(f64::ceil))
        } else {
            Interval(origin.0.map(f64::floor))
        };
        let phase = if negative {
            sub(rounded, origin)?
        } else {
            sub(origin, rounded)?
        };
        // For each actual binary32 origin, subtracting its adjacent integer
        // gives a phase in [0,1]. Intersect away interval dependency inflation.
        let phase = Interval([phase.0[0].max(0.), phase.0[1].min(1.)]);
        require(phase.0[0] <= phase.0[1])?;
        let (base, count) = if negative {
            let left = Interval(sub(origin, size)?.0.map(f64::floor));
            (
                if horizontal { rounded } else { left },
                rounded.0[1] - left.0[0],
            )
        } else {
            (rounded, phase.add(size)?.0[1].ceil())
        };
        require((0. ..=134217728.).contains(&count))?;
        let window = point(1.).div(size.div(point(f64::from(source)))?)?;
        let window = Interval([window.0[0].max(1.), window.0[1].max(1.)]);
        result.push(AxisPlan {
            source,
            phase,
            size,
            window,
            base,
            count: count as u32,
            reverse_output: negative && horizontal,
            reverse_source: negative && !horizontal,
        });
    }
    Ok(result)
}

impl AxisPlan {
    fn centre(&self, pixel: u32) -> EngineResult<Interval> {
        let pixel = Interval::enclosing(f64::from(pixel), f64::from(pixel))?;
        let shifted = sub(pixel, self.phase)?.add(point(0.5))?;
        sub(
            shifted.mul(point(f64::from(self.source)))?.div(self.size)?,
            point(0.5),
        )
    }

    fn first(&self, predicate: impl Fn(u32) -> EngineResult<bool>) -> EngineResult<u32> {
        let (mut low, mut high) = (0, self.count);
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

    fn bounds(&self, mut left: u32, mut right: u32) -> EngineResult<Option<[i32; 2]>> {
        if self.reverse_source {
            (left, right) = (self.source - right, self.source - left);
        }
        // The ordinary candidate interval includes every possible filter tap.
        // The fallback clamps floor(centre) to the source: edge cells must also
        // cover any out-of-source centres, even if all candidate weights vanish.
        let first = if left == 0 {
            0
        } else {
            self.first(|j| Ok(self.centre(j)?.add(self.window)?.0[1] >= f64::from(left)))?
        };
        let end = if right == self.source {
            self.count
        } else {
            self.first(|j| Ok(sub(self.centre(j)?, self.window)?.0[0] > f64::from(right - 1)))?
        };
        if first >= end {
            return Ok(None);
        }
        let [l, r] = if self.reverse_output {
            [
                self.base.0[0] - f64::from(end),
                self.base.0[1] - f64::from(first),
            ]
        } else {
            [
                self.base.0[0] + f64::from(first),
                self.base.0[1] + f64::from(end),
            ]
        };
        require(l >= f64::from(i32::MIN) && r <= f64::from(i32::MAX))?;
        Ok(Some([l as i32, r as i32]))
    }
}

fn axis_bounds(plans: &[AxisPlan], left: u32, right: u32) -> EngineResult<Option<[i32; 2]>> {
    let mut result: Option<[i32; 2]> = None;
    for plan in plans {
        if let Some([l, r]) = plan.bounds(left, right)? {
            result = Some(match result {
                None => [l, r],
                Some([a, b]) => [a.min(l), b.max(r)],
            });
        }
    }
    Ok(result)
}

#[cfg(test)]
#[path = "redaction_render_scale_tests.rs"]
mod tests;
