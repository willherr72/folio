//! PDFium bitmap-device placement; not source-removal authority.
use super::{pdfium_stretch_bounds, Interval, RendererTransform};
use crate::{redaction_content::require, redaction_sampling::SampleRegions, EngineResult};
use std::collections::BTreeSet;

const COORD: f64 = (1_u32 << 24) as f64;
fn point(v: f64) -> Interval {
    Interval([v; 2])
}
fn neg(v: Interval) -> Interval {
    Interval([-v.0[1], -v.0[0]])
}
fn sub(a: Interval, b: Interval) -> EngineResult<Interval> {
    a.add(neg(b))
}
fn absolute(v: Interval) -> Interval {
    Interval([
        if v.0[0] <= 0. && v.0[1] >= 0. {
            0.
        } else {
            v.0[0].abs().min(v.0[1].abs())
        },
        v.0[0].abs().max(v.0[1].abs()),
    ])
}
// (possible, certain) for each strict comparison. Correlation is deliberately
// not used to discard any branch when arithmetic has widened an operand.
fn less(a: Interval, b: Interval) -> (bool, bool) {
    (a.0[0] < b.0[1], a.0[1] < b.0[0])
}
fn quarter(m: RendererTransform) -> EngineResult<(bool, bool)> {
    let [a, b, c, d, _, _] = m.0.map(absolute);
    let tests = [
        less(a, b.div(point(20.))?),
        less(d, c.div(point(20.))?),
        less(a, point(0.5)),
        less(d, point(0.5)),
    ];
    Ok((tests.iter().all(|t| t.0), tests.iter().all(|t| t.1)))
}
fn signed_choices(v: Interval, positive: bool) -> Vec<bool> {
    if positive {
        [(false, v.0[0] <= 0.), (true, v.0[1] > 0.)]
            .into_iter()
            .filter_map(|(v, p)| p.then_some(v))
            .collect()
    } else {
        [(false, v.0[1] >= 0.), (true, v.0[0] < 0.)]
            .into_iter()
            .filter_map(|(v, p)| p.then_some(v))
            .collect()
    }
}
fn intersection(a: [i32; 4], b: [i32; 4]) -> Option<[i32; 4]> {
    let r = [
        a[0].max(b[0]),
        a[1].max(b[1]),
        a[2].min(b[2]),
        a[3].min(b[3]),
    ];
    (r[0] < r[2] && r[1] < r[3]).then_some(r)
}

fn bbox(m: RendererTransform, r: [f64; 4]) -> EngineResult<[Interval; 4]> {
    let [a, b, c, d, e, f] = m.0;
    let mut out = [point(f64::INFINITY); 4];
    for (i, (x, y)) in [(r[0], r[1]), (r[0], r[3]), (r[2], r[1]), (r[2], r[3])]
        .into_iter()
        .enumerate()
    {
        for (axis, v) in [
            a.mul(point(x))?.add(c.mul(point(y))?)?.add(e)?,
            b.mul(point(x))?.add(d.mul(point(y))?)?.add(f)?,
        ]
        .into_iter()
        .enumerate()
        {
            require(v.0[0] >= -COORD && v.0[1] <= COORD)?;
            for k in 0..2 {
                out[axis].0[k] = if i == 0 {
                    v.0[k]
                } else {
                    out[axis].0[k].min(v.0[k])
                };
                out[axis + 2].0[k] = if i == 0 {
                    v.0[k]
                } else {
                    out[axis + 2].0[k].max(v.0[k])
                };
            }
        }
    }
    Ok(out)
}

fn integer_range(low: f64, high: f64) -> EngineResult<std::ops::RangeInclusive<i32>> {
    require(low >= -COORD && high <= COORD && low <= high && high - low <= 8.)?;
    Ok(low as i32..=high as i32)
}
fn outer(bounds: [Interval; 4]) -> EngineResult<Vec<[i32; 4]>> {
    let mut ranges = [(0, 0); 4];
    for (i, v) in bounds.into_iter().enumerate() {
        let range = if i < 2 {
            integer_range(v.0[0].floor(), v.0[1].floor())?
        } else {
            integer_range(v.0[0].ceil(), v.0[1].ceil())?
        };
        ranges[i] = (*range.start(), *range.end());
    }
    let count = ranges
        .iter()
        .map(|(a, b)| (b - a + 1) as usize)
        .product::<usize>();
    require(count <= 256)?;
    let mut out = Vec::new();
    for l in ranges[0].0..=ranges[0].1 {
        for t in ranges[1].0..=ranges[1].1 {
            for r in ranges[2].0..=ranges[2].1 {
                for b in ranges[3].0..=ranges[3].1 {
                    if l < r && t < b {
                        out.push([l, t, r, b]);
                    }
                }
            }
        }
    }
    Ok(out)
}
fn closest(bounds: [Interval; 4]) -> EngineResult<Vec<[i32; 4]>> {
    let mut axes = [Vec::new(), Vec::new()];
    for i in 0..2 {
        let starts = integer_range(bounds[i].0[0].floor(), bounds[i].0[1].ceil())?;
        let length = sub(bounds[i + 2], bounds[i])?;
        let lengths = integer_range(length.0[0].ceil().max(0.), length.0[1].ceil().max(0.))?;
        // MatchFloatRange chooses floor/ceil start using competing errors.
        // Include both choices. Integer sums in this domain are exact in f32.
        for start in starts {
            for len in lengths.clone() {
                let end = i64::from(start) + i64::from(len);
                require(end.abs() <= COORD as i64)?;
                if len > 0 {
                    axes[i].push([start, end as i32]);
                }
            }
        }
    }
    require(axes[0].len() * axes[1].len() <= 256)?;
    Ok(axes[0]
        .iter()
        .flat_map(|x| axes[1].iter().map(move |y| [x[0], y[0], x[1], y[1]]))
        .collect())
}
fn stretch_sizes(a: Interval, b: Interval) -> EngineResult<Vec<i32>> {
    let a = absolute(a);
    let b = absolute(b);
    let squares = a.mul(a)?.add(b.mul(b)?)?;
    let lo = squares.0[0].max(0.).sqrt() as f32;
    let hi = squares.0[1].sqrt() as f32;
    // One extra representable neighbor also encloses the native hypotf's
    // rounded library result. The squared-sum route is never used as a
    // nominal dimension: all enclosed integer sizes are considered.
    let low = f64::from(lo.next_down()).max(0.).ceil();
    let high = f64::from(hi.next_up()).ceil();
    Ok(integer_range(low, high)?.filter(|v| *v > 0).collect())
}
struct Work {
    remaining: usize,
    cells: usize,
    out: BTreeSet<[i32; 4]>,
}
impl Work {
    fn charge(&mut self) -> EngineResult<()> {
        let cost = self.cells.max(1);
        require(cost <= self.remaining)?;
        self.remaining -= cost;
        Ok(())
    }
    fn add(&mut self, r: [i32; 4]) -> EngineResult<()> {
        if r[0] < r[2] && r[1] < r[3] {
            self.out.insert(r);
        }
        require(self.out.len() <= 4096)
    }
    fn direct(
        &mut self,
        samples: &SampleRegions,
        image: [i32; 4],
        clip: [i32; 4],
        flipx: bool,
        flipy: bool,
        swap: bool,
    ) -> EngineResult<()> {
        self.charge()?;
        let w = image[2] - image[0];
        let h = image[3] - image[1];
        require(w > 0 && h > 0 && w <= 1 << 24 && h <= 1 << 24)?;
        let c = [
            clip[0] - image[0],
            clip[1] - image[1],
            clip[2] - image[0],
            clip[3] - image[1],
        ];
        let (size, sc) = if swap {
            (
                [h, w],
                [
                    if flipy { h - c[3] } else { c[1] },
                    if flipx { w - c[2] } else { c[0] },
                    if flipy { h - c[1] } else { c[3] },
                    if flipx { w - c[0] } else { c[2] },
                ],
            )
        } else {
            ([if flipx { -w } else { w }, if flipy { -h } else { h }], c)
        };
        let stretched = pdfium_stretch_bounds(samples, size, sc.map(|v| v as u32))?;
        let cw = clip[2] - clip[0];
        let ch = clip[3] - clip[1];
        for r in stretched.cells() {
            let r = r.map(|v| v as i32);
            let local = if swap {
                [
                    if flipx { cw - r[3] } else { r[1] },
                    if flipy { ch - r[2] } else { r[0] },
                    if flipx { cw - r[1] } else { r[3] },
                    if flipy { ch - r[0] } else { r[2] },
                ]
            } else {
                r
            };
            self.add([
                local[0] + clip[0],
                local[1] + clip[1],
                local[2] + clip[0],
                local[3] + clip[1],
            ])?;
        }
        Ok(())
    }
    fn general(
        &mut self,
        m: RendererTransform,
        samples: &SampleRegions,
        result: [i32; 4],
    ) -> EngineResult<()> {
        let [a, b, c, d, e, f] = m.0;
        for w in stretch_sizes(a, b)? {
            for h in stretch_sizes(c, d)? {
                self.charge()?;
                let normalized = RendererTransform([
                    a.div(point(w as f64))?,
                    b.div(point(w as f64))?,
                    c.div(point(h as f64))?,
                    d.div(point(h as f64))?,
                    e,
                    f,
                ]);
                // Do not replace H*(c/H)+e with c+e: native rounding differs.
                let to_device = normalized
                    .concat(RendererTransform::known([1., 0., 0., -1., 0., h as f32])?)?;
                let inverse = to_device.reader_inverse()?;
                let candidate = bbox(inverse, result.map(f64::from))?;
                // Clamp interval endpoints before enumeration. Native Intersect
                // follows rounding and yields the same nonempty integer clips.
                let clipped = std::array::from_fn(|i| {
                    Interval(
                        candidate[i]
                            .0
                            .map(|v| v.clamp(0., if i % 2 == 0 { w as f64 } else { h as f64 })),
                    )
                });
                for sc in outer(clipped)? {
                    self.charge()?;
                    let stretched = pdfium_stretch_bounds(samples, [w, h], sc.map(|v| v as u32))?;
                    let mut local = inverse.concat(RendererTransform::known([
                        1.,
                        0.,
                        0.,
                        1.,
                        result[0] as f32,
                        result[1] as f32,
                    ])?)?;
                    local.0[4] = local.0[4].add(point(-f64::from(sc[0])))?;
                    local.0[5] = local.0[5].add(point(-f64::from(sc[1])))?;
                    for r in local.pdfium_affine_bounds(&stretched, result)? {
                        self.add(r)?;
                    }
                }
            }
        }
        Ok(())
    }
}

impl RendererTransform {
    /// `self` encloses the AGG image-unit-to-device matrix and `clip` is its
    /// effective integer clip box. `samples` describes the actual decoded bitmap.
    /// This does not infer decoder dimensions or the page/device transform.
    pub fn pdfium_image_bounds(
        self,
        samples: &SampleRegions,
        clip: [i32; 4],
    ) -> EngineResult<Vec<[i32; 4]>> {
        self.pdfium_image_bounds_budgeted(samples, clip, &mut 65536)
    }

    /// Shares consumed cell-stage work with the enclosing edit, including
    /// failure paths. The original per-call cap remains in force.
    pub fn pdfium_image_bounds_budgeted(
        self,
        samples: &SampleRegions,
        clip: [i32; 4],
        remaining: &mut usize,
    ) -> EngineResult<Vec<[i32; 4]>> {
        let (sw, sh) = samples.dimensions();
        require(
            sw <= 32768
                && sh <= 32768
                && clip[0] <= clip[2]
                && clip[1] <= clip[3]
                && clip.iter().all(|v| f64::from(*v).abs() <= COORD),
        )?;
        let unit = bbox(self, [0., 0., 1., 1.])?;
        let images = outer(unit)?;
        let [a, b, c, d, _, _] = self.0;
        let nearb = less(absolute(b), point(0.5));
        let nearc = less(absolute(c), point(0.5));
        let nonzero = |v: Interval| (v.0[0] != 0. || v.0[1] != 0., v.0[0] > 0. || v.0[1] < 0.);
        let za = nonzero(a);
        let zd = nonzero(d);
        let normal_possible = nearb.0 && nearc.0 && za.0 && zd.0;
        let normal_certain = nearb.1 && nearc.1 && za.1 && zd.1;
        let turn = quarter(self)?;
        let allowance = (*remaining).min(65536);
        let mut work = Work {
            remaining: allowance,
            cells: samples.cells().len(),
            out: BTreeSet::new(),
        };
        // Compute even for empty selection/clip so invalid geometry cannot be
        // admitted by changing the selection after inspection.
        let closest_rects = if !normal_certain && !turn.1 {
            closest(unit)?
        } else {
            Vec::new()
        };
        let execution = (|| -> EngineResult<()> {
            for image in images {
                work.charge()?;
                let Some(active) = intersection(image, clip) else {
                    continue;
                };
                if normal_possible {
                    for flipx in signed_choices(a, false) {
                        for flipy in signed_choices(d, true) {
                            work.direct(samples, image, active, flipx, flipy, false)?;
                        }
                    }
                }
                if !normal_certain && turn.0 {
                    for flipx in signed_choices(c, true) {
                        for flipy in signed_choices(b, false) {
                            work.direct(samples, image, active, flipx, flipy, true)?;
                        }
                    }
                }
                if !normal_certain && !turn.1 {
                    let smallb = less(absolute(b), point(f64::from(0.05_f32)));
                    let smallc = less(absolute(c), point(f64::from(0.05_f32)));
                    // The transformer-normal route requires an exactly zero a/d
                    // to reach it from AGG. Its zero stretch size paints nothing.
                    if !(smallb.1 && smallc.1) {
                        for closest in &closest_rects {
                            if let Some(result) = intersection(*closest, active) {
                                work.general(self, samples, result)?;
                            }
                        }
                    }
                }
            }
            Ok(())
        })();
        *remaining -= allowance - work.remaining;
        execution?;
        Ok(work.out.into_iter().collect())
    }
}
#[cfg(test)]
#[path = "redaction_render_pdfium_image_tests.rs"]
mod tests;
