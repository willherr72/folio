//! Possible native decode/cache frames, never source-removal authority.
use super::{
    affine::native_rectangle, placement::axis_extent, GridFit, Interval, RendererTransform,
};
use crate::{redaction_content::require, EngineResult};
use std::collections::BTreeSet;

const LIMIT: i32 = 1 << 24;

fn point(n: f64) -> Interval {
    Interval([n; 2])
}

// Independent interval enumeration of the default reader's requested crop.
// Refuse excessive uncertainty rather than selecting one rounded endpoint.
fn requested(
    frame: RendererTransform,
    source: [u32; 2],
    clip: [i32; 4],
) -> EngineResult<Vec<[u32; 4]>> {
    let mut inverse = frame.reader_inverse()?;
    for i in 0..6 {
        inverse.0[i] = inverse.0[i].mul(point(f64::from(source[i % 2])))?;
    }
    let mut expansion = [1_f64; 2];
    for v in &inverse.0[..4] {
        let low = if v.0[0] <= 0. && v.0[1] >= 0. {
            0.
        } else {
            v.0[0].abs().min(v.0[1].abs())
        };
        expansion[0] = expansion[0].max(low);
        expansion[1] = expansion[1].max(v.0[0].abs().max(v.0[1].abs()));
    }
    let padding = Interval(expansion).mul(point(4.))?;
    let rect = native_rectangle(inverse, clip.map(|n| point(f64::from(n))))?;
    let mut ranges = [[0_u32; 2]; 4];
    let mut count = 1_u32;
    for i in 0..4 {
        let offset = if i < 2 {
            Interval([-padding.0[1], -padding.0[0]])
        } else {
            padding
        };
        let extended = rect[i].add(offset)?;
        for j in 0..2 {
            let n = extended.0[j].clamp(0., f64::from(source[i % 2]));
            ranges[i][j] = if i < 2 {
                n.floor() as u32
            } else {
                n.ceil() as u32
            };
        }
        let width = ranges[i][1] - ranges[i][0] + 1;
        require(width <= 8)?;
        count *= width;
        require(count <= 256)?;
    }
    let mut result = Vec::new();
    for l in ranges[0][0]..=ranges[0][1] {
        for t in ranges[1][0]..=ranges[1][1] {
            for r in ranges[2][0]..=ranges[2][1] {
                for b in ranges[3][0]..=ranges[3][1] {
                    if l < r && t < b {
                        result.push([l, t, r, b]);
                    }
                }
            }
        }
    }
    Ok(result)
}

fn tuned(mut r: [u32; 4], source: [u32; 2], factor: u32) -> [u32; 4] {
    let [w, h] = source;
    // Admitted dimensions keep the native signed integer products in range.
    if (r[2] - r[0]) * (r[3] - r[1]) >= (w * h / 10) * 9 {
        return [0, 0, w, h];
    }
    for axis in 0..2 {
        let size = source[axis];
        if r[axis] <= size / 100 {
            r[axis] = 0;
        }
        if r[axis + 2] >= size * 99 / 100 {
            r[axis + 2] = size;
        }
        r[axis] = r[axis] / factor * factor;
        r[axis + 2] = (r[axis + 2].div_ceil(factor) * factor).min(size);
    }
    r
}
pub(crate) struct MuPdfTile {
    pub window: [u32; 4],
    pub denominator: u32,
    pub extra: u32,
    pub frame: RendererTransform,
    pub extents: [[u32; 2]; 2],
}
#[path = "redaction_render_mupdf_paint.rs"]
mod paint;
impl RendererTransform {
    pub fn mupdf_tiles(
        self,
        source: [u32; 2],
        clip: [i32; 4],
        mode: GridFit,
    ) -> EngineResult<Vec<MuPdfTile>> {
        self.mupdf_tiles_budgeted(source, clip, mode, &mut 65536)
    }
    pub fn mupdf_tiles_budgeted(
        self,
        source: [u32; 2],
        clip: [i32; 4],
        mode: GridFit,
        remaining: &mut usize,
    ) -> EngineResult<Vec<MuPdfTile>> {
        // Bound interval/crop preflight even when it refuses before decoding.
        if *remaining < 256 {
            *remaining = 0;
            require(false)?;
        }
        *remaining -= 256;
        let sizing = self.mupdf_sizing(source, mode)?;
        require(
            clip[0] <= clip[2]
                && clip[1] <= clip[3]
                && clip.iter().all(|n| (-LIMIT..=LIMIT).contains(n)),
        )?;
        if clip[0] == clip[2] || clip[1] == clip[3] {
            return Ok(Vec::new());
        }
        let areas = requested(sizing.fitted, source, clip)?;
        let mut choices = BTreeSet::new();
        let mut work = 0;
        for area in areas {
            let mut ideal = 1;
            while ideal <= sizing.maximum_reduction {
                // Full-image cache lookup follows the requested-window lookup.
                for window in [tuned(area, source, ideal), [0, 0, source[0], source[1]]] {
                    let mut actual = 1;
                    while actual <= ideal {
                        require(*remaining > 0)?;
                        *remaining -= 1;
                        work += 1;
                        require(work <= 8192)?;
                        choices.insert((window, actual));
                        require(choices.len() <= 1024)?;
                        actual *= 2;
                    }
                }
                ideal *= 2;
            }
        }
        let mut result = Vec::with_capacity(choices.len());
        for (window, total) in choices {
            let frame = sizing.fitted.mupdf_tile_frame(source, window)?;
            let [a, b, c, d, _, _] = frame.0;
            result.push(MuPdfTile {
                window,
                denominator: total.min(8),
                extra: total / total.min(8),
                frame,
                // Returned dw/dh precede the decoder's source-size clamp.
                extents: [
                    axis_extent(a, b, LIMIT as u32)?,
                    axis_extent(c, d, LIMIT as u32)?,
                ],
            });
        }
        Ok(result)
    }
}
#[cfg(test)]
#[path = "redaction_render_mupdf_tiles_tests.rs"]
mod tests;
