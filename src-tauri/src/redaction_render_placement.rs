//! Native grid fitting and requested extents for a caller-proved device matrix.
use super::{Interval, RendererTransform};
use crate::{redaction_content::require, EngineResult};

#[derive(Clone, Copy, Debug)]
pub(crate) enum GridFit {
    Disabled,
    Outward,
    Tiled,
}

pub(crate) struct ImageSizing {
    pub fitted: RendererTransform,
    pub extents: [[u32; 2]; 2],
    pub maximum_reduction: u32,
}

impl RendererTransform {
    /// `self` must enclose the device transform before native image grid fitting.
    /// Extents are requested full-image dimensions, not cropped tile dimensions.
    pub fn mupdf_sizing(self, source: [u32; 2], mode: GridFit) -> EngineResult<ImageSizing> {
        crate::redaction_render_sizing::mupdf_maximum(source, [0, 0])?;
        let fitted = self.grid_fit(mode)?;
        let [a, b, c, d, _, _] = fitted.0;
        let extents = [axis_extent(a, b, source[0])?, axis_extent(c, d, source[1])?];
        let maximum_reduction = if extents.iter().any(|v| v[1] == 0) {
            // MuPDF disables reduction when either requested extent is zero.
            1
        } else {
            crate::redaction_render_sizing::mupdf_maximum(source, [extents[0][0], extents[1][0]])?
        };
        Ok(ImageSizing {
            fitted,
            extents,
            maximum_reduction,
        })
    }

    fn grid_fit(self, mode: GridFit) -> EngineResult<Self> {
        if matches!(mode, GridFit::Disabled) {
            return Ok(self);
        }
        let [a, b, c, d, _, _] = self.0;
        let axis = [near_zero(b), near_zero(c)];
        let turn = [near_zero(a), near_zero(d)];
        let axis_possible = axis.iter().all(|v| v.0);
        let axis_certain = axis.iter().all(|v| v.1);
        let turn_possible = turn.iter().all(|v| v.0) && !axis_certain;
        let turn_certain = turn.iter().all(|v| v.1);
        let mut result: Option<Self> = None;
        let mut include = |value: Self| {
            result = Some(match result {
                None => value,
                Some(mut previous) => {
                    for (out, v) in previous.0.iter_mut().zip(value.0) {
                        *out = union(*out, v);
                    }
                    previous
                }
            });
        };
        if !axis_certain && !turn_certain {
            include(self);
        }
        if axis_possible {
            include(self.fit_pairs([(0, 4), (3, 5)], mode)?);
        }
        if turn_possible {
            // The native tiled quarter-turn path associates b with e and c
            // with f; the outward path associates b with f and c with e.
            let pairs = if matches!(mode, GridFit::Tiled) {
                [(1, 4), (2, 5)]
            } else {
                [(1, 5), (2, 4)]
            };
            include(self.fit_pairs(pairs, mode)?);
        }
        require(result.is_some())?;
        Ok(result.unwrap())
    }

    fn fit_pairs(mut self, pairs: [(usize, usize); 2], mode: GridFit) -> EngineResult<Self> {
        for (size, origin) in pairs {
            let (extent, position) = fit_axis(self.0[size], self.0[origin], mode)?;
            self.0[size] = extent;
            self.0[origin] = position;
        }
        Ok(self)
    }
}

fn point(value: f64) -> Interval {
    Interval([value, value])
}
fn union(a: Interval, b: Interval) -> Interval {
    Interval([a.0[0].min(b.0[0]), a.0[1].max(b.0[1])])
}
fn sub(a: Interval, b: Interval) -> EngineResult<Interval> {
    a.add(Interval([-b.0[1], -b.0[0]]))
}

fn near_zero(v: Interval) -> (bool, bool) {
    let epsilon = f64::from(f32::EPSILON);
    (
        v.0[0] < epsilon && v.0[1] > -epsilon,
        v.0[0] > -epsilon && v.0[1] < epsilon,
    )
}

fn trunc(v: Interval) -> EngineResult<Interval> {
    // Never model undefined native float-to-int conversions as a valid result.
    require(v.0[0] >= f64::from(i32::MIN) && v.0[1] < 2147483648.)?;
    Interval::enclosing(v.0[0].trunc(), v.0[1].trunc())
}

fn nearest(v: Interval) -> EngineResult<Interval> {
    trunc(v.add(point(0.5))?)
}

fn outward(v: Interval, lower: bool) -> EngineResult<Interval> {
    let base = trunc(v)?;
    let difference = if lower { sub(base, v)? } else { sub(v, base)? };
    let threshold = f64::from(0.001_f32);
    if difference.0[1] <= threshold {
        return Ok(base);
    }
    let adjusted = base.add(point(if lower { -1. } else { 1. }))?;
    Ok(if difference.0[0] > threshold {
        adjusted
    } else {
        union(base, adjusted)
    })
}

fn fit_axis(size: Interval, origin: Interval, mode: GridFit) -> EngineResult<(Interval, Interval)> {
    if matches!(mode, GridFit::Tiled) {
        let position = nearest(origin)?;
        return Ok((nearest(size.add(sub(origin, position)?)?)?, position));
    }
    let mut result = None;
    let mut include = |extent, position| {
        result = Some(match result {
            None => (extent, position),
            Some((s, p)) => (union(s, extent), union(p, position)),
        });
    };
    if size.0[0] <= 0. && size.0[1] >= 0. {
        include(point(0.), origin);
    }
    for positive in [true, false] {
        if (positive && size.0[1] <= 0.) || (!positive && size.0[0] >= 0.) {
            continue;
        }
        let restricted = if positive {
            Interval([size.0[0].max(0.), size.0[1]])
        } else {
            Interval([size.0[0], size.0[1].min(0.)])
        };
        let position = outward(origin, positive)?;
        let expanded = restricted.add(sub(origin, position)?)?;
        include(outward(expanded, !positive)?, position);
    }
    require(result.is_some())?;
    Ok(result.unwrap())
}

fn square(v: Interval) -> EngineResult<Interval> {
    let low = if v.0[0] <= 0. && v.0[1] >= 0. {
        0.
    } else {
        v.0[0].abs().min(v.0[1].abs())
    };
    let high = v.0[0].abs().max(v.0[1].abs());
    Interval::enclosing(low * low, high * high)
}

pub(super) fn axis_extent(a: Interval, b: Interval, source: u32) -> EngineResult<[u32; 2]> {
    let squared = square(a)?.add(square(b)?)?;
    // Endpoint squares are exact in binary64. Testing the candidate's square
    // avoids depending on the rounding direction of binary64 sqrt itself.
    let mut low = squared.0[0].sqrt() as f32;
    let mut high = squared.0[1].sqrt() as f32;
    if f64::from(low) * f64::from(low) > squared.0[0] {
        low = low.next_down();
    }
    if f64::from(high) * f64::from(high) < squared.0[1] {
        high = high.next_up();
    }
    require(low >= 0. && low.is_finite() && high.is_finite() && high <= 16777216.)?;
    Ok([(low as u32).min(source), (high as u32).min(source)])
}

#[cfg(test)]
#[path = "redaction_render_placement_tests.rs"]
mod tests;
