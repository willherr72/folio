//! Source-block coverage. A mask alone never authorizes a page-paint exemption.
use crate::{
    redaction_area_paths::{AreaRect, ImageTransform},
    redaction_content::{invalid, require},
    redaction_jpeg::Header,
    EngineResult,
};
use lopdf::{content::Operation, Object};
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct CoverageBudget {
    pixels: usize,
    masks: usize,
    cells: usize,
    rectangles: usize,
    operations: usize,
    verified_operations: usize,
    render_variants: usize,
    render_work: usize,
    render_rectangles: usize,
    boundary_edges: usize,
}
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum PaintVersion {
    Rectangles = 1,
    Triangles = 2,
    Contours = 3,
}
impl CoverageBudget {
    pub fn charge_render_rectangles(&mut self, amount: usize) -> EngineResult<()> {
        Self::charge(&mut self.render_rectangles, amount, 16384)
    }
    pub fn charge_render_transforms(&mut self, amount: usize) -> EngineResult<()> {
        Self::charge(&mut self.render_work, amount, 1_048_576)
    }
    pub fn charge_verified_operations(&mut self, amount: usize) -> EngineResult<()> {
        Self::charge(&mut self.verified_operations, amount, 100_000)
    }
    pub fn charge_source_operations(&mut self, amount: usize) -> EngineResult<()> {
        Self::charge(&mut self.operations, amount, 100_000)
    }
    fn charge(counter: &mut usize, amount: usize, maximum: usize) -> EngineResult<()> {
        require(amount <= maximum.saturating_sub(*counter))?;
        *counter += amount;
        Ok(())
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Shape {
    width: u32,
    height: u32,
    mcu_width: u32,
    mcu_height: u32,
    columns: u32,
    rows: u32,
}
impl Shape {
    fn checked(h: &Header) -> EngineResult<Self> {
        require(
            (1..=32768).contains(&h.width)
                && (1..=32768).contains(&h.height)
                && u64::from(h.width) * u64::from(h.height) <= 16_000_000
                && matches!(h.mcu_width, 8 | 16)
                && matches!(h.mcu_height, 8 | 16),
        )?;
        require(
            h.columns == h.width.div_ceil(h.mcu_width) && h.rows == h.height.div_ceil(h.mcu_height),
        )?;
        Ok(Self {
            width: h.width,
            height: h.height,
            mcu_width: h.mcu_width,
            mcu_height: h.mcu_height,
            columns: h.columns,
            rows: h.rows,
        })
    }
    fn cells(self) -> usize {
        self.columns as usize * self.rows as usize
    }
}
pub(crate) struct Coverage {
    shape: Shape,
    mask: Vec<u8>,
}
pub(crate) struct Paint {
    operations: Vec<Operation>,
    regions: Vec<AreaRect>,
}
impl Paint {
    pub fn regions(&self) -> &[AreaRect] {
        &self.regions
    }
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }
    pub fn page_bounds(&self, matrix: ImageTransform) -> EngineResult<Vec<AreaRect>> {
        self.regions
            .iter()
            .map(|r| matrix.unit_bounds(*r).map_err(|_| invalid()))
            .collect()
    }
    /// Conditional geometric envelope, not a renderer policy or removal mask.
    /// No caller may treat arbitrary supplied margins as a fidelity proof.
    pub fn disclosure_bounds(
        &self,
        matrix: ImageTransform,
        unit_margin: [f64; 2],
        page_margin: [f64; 2],
    ) -> EngineResult<Vec<AreaRect>> {
        require(
            unit_margin
                .into_iter()
                .chain(page_margin)
                .all(|n| n.is_finite() && (0. ..=1_000_000.).contains(&n)),
        )?;
        self.regions
            .iter()
            .map(|r| {
                matrix
                    .disclosure_bounds(*r, unit_margin, page_margin)
                    .map_err(|_| invalid())
            })
            .collect()
    }
}
impl Coverage {
    /// Includes every supported lower-reduction cache choice and aligned crop.
    /// One decoded-cell halo covers admitted chroma filters. The caller must
    /// prove the maximum factor; no renderer-size or page-raster claim follows.
    pub fn source_envelope(
        &self,
        maximum_total: u32,
        budget: &mut CoverageBudget,
    ) -> EngineResult<crate::redaction_sampling::SampleRegions> {
        require(maximum_total.is_power_of_two() && maximum_total <= 64)?;
        let denominator = maximum_total.min(8);
        self.decoded_source_envelope(denominator, [1, 1], maximum_total / denominator, budget)
    }
    /// Conditional envelope over aligned crop/cache and extra-reduction paths.
    /// The caller still proves the candidate factors and downstream sampling.
    pub fn decoded_source_envelope(
        &self,
        denominator: u32,
        halo: [u32; 2],
        extra: u32,
        budget: &mut CoverageBudget,
    ) -> EngineResult<crate::redaction_sampling::SampleRegions> {
        require(
            matches!(denominator, 1 | 2 | 4 | 8)
                && extra.is_power_of_two()
                && extra <= 64 / denominator,
        )?;
        self.decoded_samples(denominator, halo, budget)?
            .source_envelope([self.shape.width, self.shape.height], denominator, extra)
    }
    /// An actual aligned source window, not an unadjusted requested crop.
    /// Decoder support is expanded on the full decoded grid before cropping.
    pub fn decoded_window_samples(
        &self,
        denominator: u32,
        halo: [u32; 2],
        [left, top, right, bottom]: [u32; 4],
        budget: &mut CoverageBudget,
    ) -> EngineResult<crate::redaction_sampling::SampleRegions> {
        require(matches!(denominator, 1 | 2 | 4 | 8))?;
        require(
            left < right
                && right <= self.shape.width
                && top < bottom
                && bottom <= self.shape.height
                && left % denominator == 0
                && top % denominator == 0
                && (right % denominator == 0 || right == self.shape.width)
                && (bottom % denominator == 0 || bottom == self.shape.height),
        )?;
        self.decoded_samples(denominator, halo, budget)?.crop([
            left / denominator,
            top / denominator,
            right.div_ceil(denominator),
            bottom.div_ceil(denominator),
        ])
    }
    /// Decoder sample cells in image-unit coordinates, not a removal mask or
    /// complete display envelope. The caller must separately include owned
    /// paint and downstream sampling/grid effects. No decoder policy is inferred.
    pub fn decoded_unit_regions(
        &self,
        denominator: u32,
        halo: [u32; 2],
        budget: &mut CoverageBudget,
    ) -> EngineResult<Vec<AreaRect>> {
        Ok(self
            .decoded_samples(denominator, halo, budget)?
            .into_unit_regions())
    }
    pub fn decoded_samples(
        &self,
        denominator: u32,
        halo: [u32; 2],
        budget: &mut CoverageBudget,
    ) -> EngineResult<crate::redaction_sampling::SampleRegions> {
        require(matches!(denominator, 1 | 2 | 4 | 8) && halo.into_iter().all(|n| n <= 1))?;
        let width = self.shape.width.div_ceil(denominator);
        let height = self.shape.height.div_ceil(denominator);
        let mut cells = self.rectangles(budget)?;
        for [left, top, right, bottom] in &mut cells {
            // MCU starts are multiples of every admitted denominator. Keep
            // cells integral until downstream sampling has finished.
            *left = (*left / denominator).saturating_sub(halo[0]);
            *top = (*top / denominator).saturating_sub(halo[1]);
            *right = (right.div_ceil(denominator) + halo[0]).min(width);
            *bottom = (bottom.div_ceil(denominator) + halo[1]).min(height);
        }
        crate::redaction_sampling::SampleRegions::new(width, height, cells)
    }
    pub fn binding(&self, source: &[u8; 32]) -> [u8; 32] {
        self.paint_binding(source, false)
    }
    pub fn paint_binding(&self, source: &[u8; 32], triangles: bool) -> [u8; 32] {
        self.version_binding(
            source,
            if triangles {
                PaintVersion::Triangles
            } else {
                PaintVersion::Rectangles
            },
        )
    }
    pub fn version_binding(&self, source: &[u8; 32], version: PaintVersion) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update(match version {
            PaintVersion::Rectangles => b"Folio verified JPEG coverage v1",
            PaintVersion::Triangles => b"Folio verified JPEG coverage v2",
            PaintVersion::Contours => b"Folio verified JPEG coverage v3",
        });
        hash.update(source);
        for value in [
            self.shape.width,
            self.shape.height,
            self.shape.mcu_width,
            self.shape.mcu_height,
            self.shape.columns,
            self.shape.rows,
        ] {
            hash.update(value.to_be_bytes());
        }
        hash.update((self.mask.len() as u64).to_be_bytes());
        hash.update(&self.mask);
        hash.finalize().into()
    }
    pub fn check_header(&self, header: &Header) -> EngineResult<()> {
        require(self.shape == Shape::checked(header)?)
    }
    pub fn from_pixels(
        header: &Header,
        pixels: &[bool],
        budget: &mut CoverageBudget,
    ) -> EngineResult<Self> {
        let shape = Shape::checked(header)?;
        require(pixels.len() == shape.width as usize * shape.height as usize)?;
        CoverageBudget::charge(&mut budget.pixels, pixels.len(), 64_000_000)?;
        CoverageBudget::charge(&mut budget.masks, shape.cells(), 4_000_000)?;
        let mut mask = vec![0; shape.cells()];
        // Read each original sample exactly once. Never feed expanded bounds
        // or preview fringe back into source selection.
        for (index, selected) in pixels.iter().enumerate() {
            if *selected {
                let x = index % shape.width as usize;
                let y = index / shape.width as usize;
                mask[y / shape.mcu_height as usize * shape.columns as usize
                    + x / shape.mcu_width as usize] = 1;
            }
        }
        Ok(Self { shape, mask })
    }
    pub fn from_mask(
        header: &Header,
        mask: &[u8],
        budget: &mut CoverageBudget,
    ) -> EngineResult<Self> {
        let shape = Shape::checked(header)?;
        require(mask.len() == shape.cells())?;
        CoverageBudget::charge(&mut budget.cells, mask.len(), 8_000_000)?;
        require(mask.iter().all(|v| *v <= 1))?;
        CoverageBudget::charge(&mut budget.masks, mask.len(), 4_000_000)?;
        Ok(Self {
            shape,
            mask: mask.to_vec(),
        })
    }
    pub fn mask(&self) -> &[u8] {
        &self.mask
    }
    pub fn retained_mask_bytes(&self) -> usize {
        self.mask.capacity()
    }
    pub fn union(&self, other: &Self, budget: &mut CoverageBudget) -> EngineResult<Self> {
        require(self.shape == other.shape)?;
        CoverageBudget::charge(&mut budget.cells, self.mask.len(), 8_000_000)?;
        CoverageBudget::charge(&mut budget.masks, self.mask.len(), 4_000_000)?;
        Ok(Self {
            shape: self.shape,
            mask: self
                .mask
                .iter()
                .zip(&other.mask)
                .map(|(a, b)| a | b)
                .collect(),
        })
    }
    pub fn rectangles(&self, budget: &mut CoverageBudget) -> EngineResult<Vec<[u32; 4]>> {
        CoverageBudget::charge(&mut budget.cells, self.mask.len(), 8_000_000)?;
        let s = self.shape;
        let mut result: Vec<[u32; 4]> = Vec::new();
        let mut previous: HashMap<(u32, u32), usize> = HashMap::new();
        for y in 0..s.rows {
            let mut active = HashMap::new();
            let mut x = 0;
            while x < s.columns {
                if self.mask[(y * s.columns + x) as usize] == 0 {
                    x += 1;
                    continue;
                }
                let start = x;
                while x < s.columns && self.mask[(y * s.columns + x) as usize] == 1 {
                    x += 1;
                }
                let end = x;
                let bottom = ((y + 1) * s.mcu_height).min(s.height);
                let index = if let Some(index) = previous.get(&(start, end)).copied() {
                    result[index][3] = bottom;
                    index
                } else {
                    CoverageBudget::charge(&mut budget.rectangles, 1, 4096)?;
                    let index = result.len();
                    result.push([
                        start * s.mcu_width,
                        y * s.mcu_height,
                        (end * s.mcu_width).min(s.width),
                        bottom,
                    ]);
                    index
                };
                active.insert((start, end), index);
            }
            previous = active;
        }
        Ok(result)
    }
    pub fn paint(&self, budget: &mut CoverageBudget) -> EngineResult<Paint> {
        self.contour_paint(budget)
    }
    pub fn version_paint(
        &self,
        budget: &mut CoverageBudget,
        version: PaintVersion,
    ) -> EngineResult<Paint> {
        match version {
            PaintVersion::Rectangles => self.legacy_paint(budget),
            PaintVersion::Triangles => self.paint_shape(budget, true),
            PaintVersion::Contours => self.paint(budget),
        }
    }
    // Exact old private publication grammar, accepted only after the source
    // image's coverage record and removed coefficients have been verified.
    pub fn legacy_paint(&self, budget: &mut CoverageBudget) -> EngineResult<Paint> {
        self.paint_shape(budget, false)
    }
    fn paint_shape(&self, budget: &mut CoverageBudget, triangles: bool) -> EngineResult<Paint> {
        let rectangles = self.rectangles(budget)?;
        if rectangles.is_empty() {
            return Ok(Paint {
                operations: Vec::new(),
                regions: Vec::new(),
            });
        }
        let count = 4 + if triangles { 8 } else { 5 } * rectangles.len();
        CoverageBudget::charge(&mut budget.operations, count, 100_000)?;
        let mut operations = Vec::with_capacity(count);
        let mut regions = Vec::with_capacity(rectangles.len());
        operations.push(Operation::new("q", vec![]));
        operations.push(Operation::new("g", vec![Object::Integer(0)]));
        for [x0, y0, x1, y1] in rectangles {
            let left = edge(x0, self.shape.width, false);
            let right = edge(x1, self.shape.width, true);
            let bottom = edge(self.shape.height - y1, self.shape.height, false);
            let top = edge(self.shape.height - y0, self.shape.height, true);
            regions.push(AreaRect {
                left: serialized_edge(left, false),
                bottom: serialized_edge(bottom, false),
                right: serialized_edge(right, true),
                top: serialized_edge(top, true),
            });
            let corners = [[left, bottom], [right, bottom], [right, top], [left, top]];
            let paths: &[&[usize]] = if triangles {
                &[&[0, 1, 2], &[0, 2, 3]]
            } else {
                &[&[0, 1, 2, 3]]
            };
            for path in paths {
                for (index, &corner) in path.iter().enumerate() {
                    let [x, y] = corners[corner];
                    operations.push(Operation::new(
                        if index == 0 { "m" } else { "l" },
                        vec![Object::Real(x), Object::Real(y)],
                    ));
                }
                operations.push(Operation::new("h", vec![]));
            }
        }
        // One nonzero fill unions equally wound subpaths. Touching blocks do
        // not acquire separate antialiased borders; unselected holes remain.
        // Two triangles share the exact reversed diagonal and bypass PDFium's
        // rectangle shortcut, which can trim partially covered boundary pixels.
        operations.push(Operation::new("f", vec![]));
        operations.push(Operation::new("Q", vec![]));
        Ok(Paint {
            operations,
            regions,
        })
    }
}

#[cfg(test)]
#[path = "redaction_jpeg_pdfium_tests.rs"]
mod pdfium_tests;

#[path = "redaction_jpeg_pdfium.rs"]
mod pdfium;

#[path = "redaction_jpeg_contours.rs"]
mod contours;
#[path = "redaction_jpeg_mupdf.rs"]
mod mupdf;
#[cfg(test)]
#[path = "redaction_jpeg_mupdf_tests.rs"]
mod mupdf_tests;

fn edge(n: u32, d: u32, upper: bool) -> f32 {
    if n == 0 {
        return 0.;
    }
    if n == d {
        return 1.;
    }
    let nearest = (f64::from(n) / f64::from(d)) as f32;
    // One extra ULP encloses both the exact source fraction and the shortest
    // decimal written by lopdf (which other readers may parse as f64).
    if upper {
        nearest.next_up()
    } else {
        nearest.next_down()
    }
}

fn serialized_edge(value: f32, upper: bool) -> f64 {
    if value == 0. || value == 1. {
        return f64::from(value);
    }
    // lopdf writes Real using Display. Enclose its decimal interpretation as
    // well as a reader's binary32 value, then account for binary64 parsing.
    let decimal = value
        .to_string()
        .parse::<f64>()
        .expect("finite generated coordinate");
    if upper {
        f64::from(value).max(decimal).next_up()
    } else {
        f64::from(value).min(decimal).next_down()
    }
}

#[cfg(test)]
#[path = "redaction_jpeg_coverage_tests.rs"]
mod tests;
