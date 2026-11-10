//! Connect scaled sample support to the frame actually passed to the painter.
use super::super::{mupdf_scale_bounds, GridFit, Interval, RendererTransform};
use super::{point, MuPdfTile, LIMIT};
use crate::{redaction_content::require, redaction_sampling::SampleRegions, EngineResult};
use std::collections::BTreeSet;

fn charge(remaining: &mut usize, cells: usize) -> EngineResult<()> {
    let amount = cells.max(1);
    if amount > *remaining {
        *remaining = 0;
        require(false)?;
    }
    *remaining -= amount;
    Ok(())
}

fn sub(a: Interval, b: Interval) -> EngineResult<Interval> {
    a.add(Interval([-b.0[1], -b.0[0]]))
}

fn integer_range(v: [f64; 2]) -> EngineResult<std::ops::RangeInclusive<i32>> {
    require(
        v[0] >= -f64::from(LIMIT) && v[1] <= f64::from(LIMIT) && v[0] <= v[1] && v[1] - v[0] <= 7.,
    )?;
    Ok(v[0] as i32..=v[1] as i32)
}

// Possible physical scaled-grid endpoints, before clipping. Positive and
// negative placements have different native rounding sequences.
fn grid_axis(origin: Interval, extent: Interval) -> EngineResult<Vec<[i32; 2]>> {
    let mut out = BTreeSet::new();
    for negative in [false, true] {
        if (negative && extent.0[0] >= 0.) || (!negative && extent.0[1] < 0.) {
            continue;
        }
        let size = if negative {
            Interval([(-extent.0[1].min(0.)).max(1.), (-extent.0[0]).max(1.)])
        } else {
            Interval([extent.0[0].max(1.), extent.0[1].max(1.)])
        };
        if negative {
            let left = sub(origin, size)?.0.map(f64::floor);
            for l in integer_range(left)? {
                for r in integer_range(origin.0.map(f64::ceil))? {
                    if l < r {
                        out.insert([l, r]);
                    }
                }
            }
        } else {
            let base = Interval(origin.0.map(f64::floor));
            let phase = sub(origin, base)?;
            let phase = Interval([phase.0[0].max(0.), phase.0[1].min(1.)]);
            let count = phase.add(size)?.0.map(f64::ceil);
            for l in integer_range(base.0)? {
                for n in integer_range(count)? {
                    require(n > 0 && i64::from(l) + i64::from(n) <= i64::from(LIMIT))?;
                    out.insert([l, l + n]);
                }
            }
        }
    }
    require(out.len() <= 128)?;
    Ok(out.into_iter().collect())
}

fn local_samples(bounds: &[[i32; 4]], grid: [i32; 4]) -> EngineResult<SampleRegions> {
    let [l, t, r, b] = grid;
    require(l < r && t < b && r - l <= 32768 && b - t <= 32768)?;
    let cells = bounds
        .iter()
        .filter_map(|v| {
            let a = v[0].max(l);
            let c = v[2].min(r);
            let y = v[1].max(t);
            let z = v[3].min(b);
            (a < c && y < z).then_some([
                (a - l) as u32,
                (y - t) as u32,
                (c - l) as u32,
                (z - t) as u32,
            ])
        })
        .collect();
    SampleRegions::new((r - l) as u32, (b - t) as u32, cells)
}

fn add(output: &mut BTreeSet<[i32; 4]>, rectangles: Vec<[i32; 4]>) -> EngineResult<()> {
    for rect in rectangles {
        output.insert(rect);
        require(output.len() <= 4096)?;
    }
    Ok(())
}

impl MuPdfTile {
    pub fn paint_bounds(
        &self,
        samples: &SampleRegions,
        clip: [i32; 4],
        mode: GridFit,
        remaining: &mut usize,
    ) -> EngineResult<Vec<[i32; 4]>> {
        require(
            matches!(self.denominator, 1 | 2 | 4 | 8)
                && self.extra.is_power_of_two()
                && self.extra <= 64 / self.denominator
                && self.window[0] < self.window[2]
                && self.window[1] < self.window[3]
                && self.window[2] <= 32768
                && self.window[3] <= 32768
                && self
                    .extents
                    .iter()
                    .all(|e| e[0] <= e[1] && e[1] <= LIMIT as u32),
        )?;
        let (w, h) = samples.dimensions();
        require(
            w == (self.window[2] - self.window[0])
                .div_ceil(self.denominator)
                .div_ceil(self.extra)
                && h == (self.window[3] - self.window[1])
                    .div_ceil(self.denominator)
                    .div_ceil(self.extra),
        )?;
        let mut out = BTreeSet::new();
        charge(remaining, samples.cells().len())?;
        // This also covers rendering hints that suppress the earlier scaler.
        add(&mut out, self.frame.mupdf_affine_bounds(samples, clip)?)?;
        if self.extents[0][0] >= w || self.extents[1][0] >= h {
            return Ok(out.into_iter().collect());
        }
        let zero = |v: Interval| v.0[0] <= 0. && v.0[1] >= 0.;
        let nonzero = |v: Interval| v.0 != [0., 0.];
        let [a, b, c, d, _, _] = self.frame.0;
        for quarter in [false, true] {
            let possible = if quarter {
                zero(a) && zero(d) && nonzero(b) && nonzero(c)
            } else {
                zero(b) && zero(c) && nonzero(a) && nonzero(d)
            };
            if !possible {
                continue;
            }
            let mut orth = self.frame;
            for i in if quarter { [0, 3] } else { [1, 2] } {
                orth.0[i] = point(0.);
            }
            // Alpha and type-3 choices can disable the second grid fit.
            for fit in [GridFit::Disabled, mode] {
                charge(remaining, 1)?;
                let fitted = orth.mupdf_sizing([w, h], fit)?.fitted;
                let (origin, extent, scissor) = if quarter {
                    (
                        [fitted.0[5], fitted.0[4]],
                        [fitted.0[1], fitted.0[2]],
                        [clip[1], clip[0], clip[3], clip[2]],
                    )
                } else {
                    ([fitted.0[4], fitted.0[5]], [fitted.0[0], fitted.0[3]], clip)
                };
                charge(remaining, samples.cells().len())?;
                let bounds = mupdf_scale_bounds(
                    samples,
                    origin.map(|v| v.0),
                    extent.map(|v| v.0),
                    Some(scissor),
                )?;
                let xs = grid_axis(origin[0], extent[0])?;
                let ys = grid_axis(origin[1], extent[1])?;
                require(xs.len() * ys.len() <= 256)?;
                for x in &xs {
                    for y in &ys {
                        charge(remaining, 1)?;
                        let grid = [
                            x[0].max(scissor[0]),
                            y[0].max(scissor[1]),
                            x[1].min(scissor[2]),
                            y[1].min(scissor[3]),
                        ];
                        if grid[0] >= grid[2] || grid[1] >= grid[3] {
                            continue;
                        }
                        let local = local_samples(&bounds, grid)?;
                        let [l, t, r, bot] = grid.map(f64::from);
                        let matrix = if quarter {
                            [0., r - l, bot - t, 0., t, l]
                        } else {
                            [r - l, 0., 0., bot - t, l, t]
                        };
                        charge(remaining, local.cells().len())?;
                        add(
                            &mut out,
                            RendererTransform::known(matrix.map(|v| v as f32))?
                                .mupdf_affine_bounds(&local, clip)?,
                        )?;
                    }
                }
            }
        }
        // General affine downscaling, and the caller's fallback when an
        // orthogonal clipped scaler returns no bitmap. JPEG tile origins are 0.
        let dx = self.extents[0].map(|v| v.max(1));
        let dy = self.extents[1].map(|v| v.max(1));
        require(dx[1] - dx[0] <= 7 && dy[1] - dy[0] <= 7)?;
        for width in dx[0]..=dx[1] {
            for height in dy[0]..=dy[1] {
                require(width <= 32768 && height <= 32768)?;
                charge(remaining, samples.cells().len())?;
                let bounds = mupdf_scale_bounds(
                    samples,
                    [[0.; 2]; 2],
                    [[f64::from(width); 2], [f64::from(height); 2]],
                    None,
                )?;
                let local = local_samples(&bounds, [0, 0, width as i32, height as i32])?;
                charge(remaining, local.cells().len())?;
                add(&mut out, self.frame.mupdf_affine_bounds(&local, clip)?)?;
            }
        }
        Ok(out.into_iter().collect())
    }
}
