//! Occurrence-specific sample replacement and output allocation accounting.
use crate::{
    redaction_area_paths::AreaRect,
    redaction_content::{invalid, number, require},
    redaction_core::Page,
    redaction_image_pixels::{plan_pixels, ImageTransform, PixelBudget},
    redaction_images::{Image, ImageContext, SAMPLE_BUDGET},
    EngineResult,
};
use lopdf::Object;
use std::{collections::HashMap, sync::Arc};

pub(crate) struct RemovedImage {
    pub page_index: usize,
    pub occurrence_index: usize,
    pub pixels: usize,
    pub bounds: AreaRect,
    pub extends_beyond_mark: bool,
    // When present, bounds cover MCU paint only. Reader resampling disclosure
    // must be derived before this experimental result can enter a public UI.
    pub jpeg: Option<JpegSelection>,
}
pub(crate) struct JpegSelection {
    pub header: crate::redaction_jpeg::Header,
    pub coverage: crate::redaction_jpeg_coverage::Coverage,
    pub matrix: ImageTransform,
    // Native content concatenation is not the nominal source-selection matrix.
    renderer_matrix: crate::redaction_render_transform::RendererTransform,
    // MuPDF incorporates the initial page transform before each content cm.
    // Retain the bounded source sequence to preserve that rounding order.
    renderer_steps: Vec<[f32; 6]>,
    page_dimensions: [f32; 2],
    // Replacing a coalesced path can change native edge rounding anywhere on
    // its old/new footprint. This is paint support, never source removal.
    repaint_regions: Vec<AreaRect>,
    pub paint_bounds: Vec<AreaRect>,
}
impl JpegSelection {
    /// Heap storage only: the owning RemovedImage accounts for inline fields.
    pub fn retained_geometry_heap_bytes(&self) -> EngineResult<usize> {
        [
            (self.renderer_steps.capacity(), std::mem::size_of::<[f32; 6]>()),
            (self.repaint_regions.capacity(), std::mem::size_of::<AreaRect>()),
            (self.paint_bounds.capacity(), std::mem::size_of::<AreaRect>()),
        ].into_iter().try_fold(self.coverage.retained_mask_bytes(), |sum, (count,size)| {
            sum.checked_add(count.checked_mul(size).ok_or_else(invalid)?).ok_or_else(invalid)
        })
    }
    pub fn page_influence(
        &self,
        device: [u32; 2],
        budget: &mut crate::redaction_jpeg_coverage::CoverageBudget,
    ) -> EngineResult<[Vec<[i32; 4]>; 2]> {
        use crate::redaction_render_transform::{GridFit, PageDevice, RendererTransform};
        use std::collections::BTreeSet;
        let view = PageDevice::new(self.page_dimensions, device)?;
        let clip = [0, 0, device[0] as i32, device[1] as i32];
        let pdfium = view.pdfium()?;
        let mupdf = view.mupdf()?;
        let mut output = [
            self.pdfium_influence(pdfium, device, clip, budget)?
                .into_iter()
                .collect::<BTreeSet<_>>(),
            self.mupdf_influence(mupdf, device, clip, GridFit::Outward, budget)?
                .into_iter()
                .collect::<BTreeSet<_>>(),
        ];
        budget
            .charge_render_transforms(self.renderer_steps.len() + self.repaint_regions.len() * 3)?;
        let mut native = mupdf;
        for step in &self.renderer_steps {
            native = native.concat(RendererTransform::known(*step)?)?;
        }
        for &region in &self.repaint_regions {
            // PDFium stores transformed path vertices before applying the
            // viewport. MuPDF transforms at flattening with the active CTM.
            let boxes = [
                pdfium.rectangle_bounds(self.renderer_matrix.unit_bounds(region)?)?,
                native.unit_bounds(region)?,
            ];
            for (index, bounds) in boxes.into_iter().enumerate() {
                if let Some(rect) = view.fill_pixels(bounds, index == 0)? {
                    if !output[index].contains(&rect) {
                        budget.charge_render_rectangles(1)?;
                        require(output[index].len() < 4096)?;
                        output[index].insert(rect);
                    }
                }
            }
        }
        Ok(output.map(|v| v.into_iter().collect()))
    }
    pub fn mupdf_influence(
        &self,
        initial_page: crate::redaction_render_transform::RendererTransform,
        device: [u32; 2],
        clip: [i32; 4],
        mode: crate::redaction_render_transform::GridFit,
        budget: &mut crate::redaction_jpeg_coverage::CoverageBudget,
    ) -> EngineResult<Vec<[i32; 4]>> {
        use crate::redaction_render_transform::RendererTransform;
        self.coverage.check_header(&self.header)?;
        budget.charge_render_transforms(self.renderer_steps.len() + 2)?;
        // Caller proves the PDF interpreter's initial page-to-device matrix;
        // the draw device itself must have the identity transform.
        let mut frame = initial_page;
        for step in &self.renderer_steps {
            frame = frame.concat(RendererTransform::known(*step)?)?;
        }
        frame = frame.concat(RendererTransform::known([1., 0., 0., -1., 0., 1.])?)?;
        self.coverage
            .mupdf_influence(frame, device, clip, mode, budget)
    }
    pub fn pdfium_influence(
        &self,
        page_to_device: crate::redaction_render_transform::RendererTransform,
        device: [u32; 2],
        clip: [i32; 4],
        budget: &mut crate::redaction_jpeg_coverage::CoverageBudget,
    ) -> EngineResult<Vec<[i32; 4]>> {
        self.coverage.check_header(&self.header)?;
        self.coverage.pdfium_influence(
            page_to_device.concat(self.renderer_matrix)?,
            device,
            clip,
            budget,
        )
    }
}
#[derive(Default)]
pub(crate) struct ImageWork {
    geometry: PixelBudget,
    occurrences: usize,
    bytes: usize,
    outputs: HashMap<[u8; 32], Arc<Image>>,
    retained_transforms: usize,
}
impl ImageWork {
    fn retain(&mut self, image: Arc<Image>) -> EngineResult<Arc<Image>> {
        let digest = image.digest;
        if let Some(image) = self.outputs.get(&digest) {
            return Ok(image.clone());
        }
        // Context accounts allocation before construction; this bounds the
        // distinct images used by this output, including unchanged sources.
        require(image.retained_bytes() <= SAMPLE_BUDGET.saturating_sub(self.bytes))?;
        self.bytes += image.retained_bytes();
        self.outputs.insert(digest, image.clone());
        Ok(image)
    }
    pub(crate) fn page(
        &mut self,
        page: &mut Page,
        page_index: usize,
        marks: &[AreaRect],
        context: &mut ImageContext,
    ) -> EngineResult<Vec<RemovedImage>> {
        let originals = std::mem::take(&mut page.images);
        let has_jpeg = originals.iter().any(|image| image.jpeg_header().is_some());
        let mut aliases = HashMap::new();
        let mut result = vec![];
        let mut matrix = ImageTransform::default();
        let mut renderer_matrix = crate::redaction_render_transform::RendererTransform::default();
        let mut renderer_steps = Vec::new();
        let mut stack = vec![];
        let mut occurrence = 0;
        for op in &mut page.program.operations {
            match op.operator.as_str() {
                "q" => {
                    require(stack.len() < 64)?;
                    stack.push((matrix, renderer_matrix, renderer_steps.len()));
                }
                "Q" => {
                    let (saved, rendered, length) = stack.pop().ok_or_else(invalid)?;
                    matrix = saved;
                    renderer_matrix = rendered;
                    renderer_steps.truncate(length);
                }
                "cm" => {
                    require(op.operands.len() == 6)?;
                    let mut values = [0.; 6];
                    for (value, arg) in values.iter_mut().zip(&op.operands) {
                        *value = number(arg)?;
                    }
                    matrix = matrix.concat(values).map_err(|_| invalid())?;
                    if has_jpeg {
                        require(renderer_steps.len() < 4096)?;
                        renderer_steps.push(values.map(|v| v as f32));
                        // Program admission proves source and canonical numeric
                        // tokens agree with native binary32 operands. Preserve
                        // every concatenation instead of casting the final f64
                        // source-selection matrix.
                        renderer_matrix = renderer_matrix.concat(
                            crate::redaction_render_transform::RendererTransform::known(
                                values.map(|v| v as f32),
                            )?,
                        )?;
                    }
                }
                "Do" => {
                    require(self.occurrences < 4096 && op.operands.len() == 1)?;
                    self.occurrences += 1;
                    let index =
                        std::str::from_utf8(op.operands[0].as_name().map_err(|_| invalid())?)
                            .map_err(|_| invalid())?
                            .strip_prefix('I')
                            .ok_or_else(invalid)?
                            .parse::<usize>()
                            .map_err(|_| invalid())?;
                    let original = originals.get(index).ok_or_else(invalid)?;
                    let plan = if marks.is_empty() {
                        None
                    } else {
                        plan_pixels(
                            matrix,
                            original.width,
                            original.height,
                            marks,
                            &mut self.geometry,
                        )
                        .map_err(|_| invalid())?
                    };
                    let mut jpeg = None;
                    let mut pixels = plan.as_ref().map_or(0, |p| p.count);
                    let mut bounds = plan.as_ref().map(|p| p.bounds);
                    let output = if let Some(plan) = &plan {
                        if let Some(header) = original.jpeg_header() {
                            let coverage = context.jpeg_selection(original, &plan.selected)?;
                            let (count, paint_bounds) =
                                context.jpeg_paint_geometry(&coverage, matrix)?;
                            let mut combined = *paint_bounds.first().ok_or_else(invalid)?;
                            for region in &paint_bounds[1..] {
                                combined.include(*region);
                            }
                            pixels = count;
                            bounds = Some(combined);
                            let output = context.redact_jpeg(original, &coverage)?;
                            let mut repaint_regions = Vec::new();
                            for image in [original, &output] {
                                if let Some(paint_coverage) = image.jpeg_coverage() {
                                    let (_, regions) = context.jpeg_paint_geometry(
                                        paint_coverage,
                                        ImageTransform::default(),
                                    )?;
                                    repaint_regions.extend(regions);
                                }
                            }
                            require(
                                renderer_steps.len()
                                    <= 65536_usize.saturating_sub(self.retained_transforms),
                            )?;
                            self.retained_transforms += renderer_steps.len();
                            jpeg = Some(JpegSelection {
                                header: header.clone(),
                                coverage,
                                matrix,
                                renderer_matrix,
                                renderer_steps: renderer_steps.clone(),
                                page_dimensions: [
                                    number(&page.width)? as f32,
                                    number(&page.height)? as f32,
                                ],
                                repaint_regions,
                                paint_bounds,
                            });
                            output
                        } else {
                            context.redact_pixels(original, &plan.selected)?
                        }
                    } else {
                        original.clone()
                    };
                    let output = self.retain(output)?;
                    let target = if let Some(index) = aliases.get(&output.digest) {
                        *index
                    } else {
                        let index = page.images.len();
                        aliases.insert(output.digest, index);
                        page.images.push(output);
                        index
                    };
                    op.operands[0] = Object::Name(format!("I{target}").into_bytes());
                    if let Some(bounds) = bounds {
                        result.push(RemovedImage {
                            page_index,
                            occurrence_index: occurrence,
                            pixels,
                            bounds,
                            extends_beyond_mark: !marks.iter().any(|mark| mark.contains(bounds)),
                            jpeg,
                        });
                    }
                    occurrence += 1;
                }
                _ => {}
            }
        }
        require(stack.is_empty())?;
        Ok(result)
    }
}
#[cfg(test)]
#[path = "redaction_jpeg_occurrence_tests.rs"]
mod jpeg_tests;
