//! A bounded viewport and independently derived disclosure for an owned pair.
use super::*;
use image::{ImageEncoder, RgbaImage};
use std::io::Write;

const MAX_PNG_BYTES: usize = 16 * 1024 * 1024;
const MAX_MASK_WORK: usize = 64_000_000;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContentRedactionRaster {
    pub source_id: String,
    pub preview_id: Option<String>,
    pub candidate_id: Option<String>,
    pub page_index: usize,
    pub page_width: f64,
    pub page_height: f64,
    pub width: u32,
    pub height: u32,
    pub source_png: Vec<u8>,
    pub candidate_png: Option<Vec<u8>>,
    pub disclosure_png: Option<Vec<u8>>,
}

fn device(page: [f64; 2], requested: u32) -> EngineResult<[u32; 2]> {
    require(
        (64..=1200).contains(&requested)
            && page
                .into_iter()
                .all(|n| n.is_finite() && (1. ..=20000.).contains(&n)),
    )?;
    let scale = (f64::from(requested) / page[0]).min(2400. / page[1]);
    let size = page.map(|n| (n * scale).round().max(1.) as u32);
    require(
        size[0] <= 1200 && size[1] <= 2400 && u64::from(size[0]) * u64::from(size[1]) <= 2_880_000,
    )?;
    Ok(size)
}

struct PngOutput {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for PngOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(std::io::Error::other(
                "Content preview PNG budget exceeded.",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn png(image: &RgbaImage, remaining: &mut usize) -> EngineResult<Vec<u8>> {
    let mut output = PngOutput {
        bytes: vec![],
        limit: *remaining,
    };
    image::codecs::png::PngEncoder::new(&mut output)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| EngineError::Io(e.to_string()))?;
    *remaining -= output.bytes.len();
    Ok(output.bytes)
}

// Bit 1 is a conservative source-removal bound; bit 2 is renderer influence.
// This mask is derived without reading either raster's pixel differences.
struct Disclosure {
    size: [u32; 2],
    flags: Vec<u8>,
    work: usize,
}
impl Disclosure {
    fn new(size: [u32; 2]) -> Self {
        Self {
            size,
            flags: vec![0; size[0] as usize * size[1] as usize],
            work: 0,
        }
    }
    fn pixels(&mut self, rect: [i32; 4], flag: u8) -> EngineResult<()> {
        let [left, top, right, bottom] = [
            rect[0].clamp(0, self.size[0] as i32),
            rect[1].clamp(0, self.size[1] as i32),
            rect[2].clamp(0, self.size[0] as i32),
            rect[3].clamp(0, self.size[1] as i32),
        ];
        require(left <= right && top <= bottom)?;
        self.work = self
            .work
            .checked_add((right - left) as usize * (bottom - top) as usize)
            .ok_or_else(invalid)?;
        require(self.work <= MAX_MASK_WORK)?;
        for y in top..bottom {
            let row = y as usize * self.size[0] as usize;
            for v in &mut self.flags[row + left as usize..row + right as usize] {
                *v |= flag;
            }
        }
        Ok(())
    }
    fn source_rect(
        &mut self,
        rect: TextMarkRect,
        page: [f64; 2],
        influence: bool,
    ) -> EngineResult<()> {
        require(rect.valid())?;
        let sx = f64::from(self.size[0]) / page[0];
        let sy = f64::from(self.size[1]) / page[1];
        let pixels = [
            (rect.x * sx).floor() as i32,
            (rect.y * sy).floor() as i32,
            ((rect.x + rect.width) * sx).ceil() as i32,
            ((rect.y + rect.height) * sy).ceil() as i32,
        ];
        self.pixels(pixels, 1)?;
        if influence {
            self.pixels(
                [
                    pixels[0].saturating_sub(1),
                    pixels[1].saturating_sub(1),
                    pixels[2].saturating_add(1),
                    pixels[3].saturating_add(1),
                ],
                2,
            )?;
        }
        Ok(())
    }
    fn verify(&self, source: &RgbaImage, candidate: &RgbaImage) -> EngineResult<()> {
        require(
            source.dimensions() == (self.size[0], self.size[1])
                && source.dimensions() == candidate.dimensions(),
        )?;
        require(
            source
                .pixels()
                .zip(candidate.pixels())
                .zip(&self.flags)
                .all(|((before, after), flags)| before == after || flags & 2 != 0),
        )
    }
    fn image(&self) -> RgbaImage {
        RgbaImage::from_fn(self.size[0], self.size[1], |x, y| {
            let flags = self.flags[y as usize * self.size[0] as usize + x as usize];
            image::Rgba(if flags & 1 != 0 {
                [220, 38, 38, 100]
            } else if flags & 2 != 0 {
                [245, 158, 11, 100]
            } else {
                [0, 0, 0, 0]
            })
        })
    }
}

impl WorkerRuntime {
    fn content_page_dimensions(&self, id: &str, page: usize) -> EngineResult<[f64; 2]> {
        let document = self.redaction_source(id)?;
        let page = document.pdf()?.pages().get(page_index_i32(page)?)?;
        Ok([
            f64::from(page.width().value),
            f64::from(page.height().value),
        ])
    }
    fn content_page_rgba(&self, id: &str, page: usize, size: [u32; 2]) -> EngineResult<RgbaImage> {
        let document = self.redaction_source(id)?;
        let page = document.pdf()?.pages().get(page_index_i32(page)?)?;
        let bitmap = page.render_with_config(
            &PdfRenderConfig::new()
                .set_fixed_size(size[0] as i32, size[1] as i32)
                .render_form_data(true),
        )?;
        let image = bitmap.as_image()?.to_rgba8();
        require(image.dimensions() == (size[0], size[1]))?;
        Ok(image)
    }
    pub(super) fn render_content_redaction(
        &mut self,
        owner: &str,
        source: &str,
        preview: Option<&str>,
        page: usize,
        width: u32,
    ) -> EngineResult<ContentRedactionRaster> {
        checked_id(owner)?;
        checked_id(source)?;
        require(page < 1000 && (64..=1200).contains(&width))?;
        self.prune_redaction_previews();
        require(self.redaction_previews.owners.contains(owner))?;
        if let Some(preview) = preview {
            self.validate_redaction_preview(owner, preview)?;
            let pending = &self.redaction_previews.pending[preview];
            require(pending.source == source && pending.content.is_some())?;
        } else {
            require(self.redaction_previews.prepared.contains_key(source))?;
            self.validate_prepared_redaction_source(owner, source)?;
        }
        let dimensions = self.content_page_dimensions(source, page)?;
        let size = device(dimensions, width)?;
        let mut mask = Disclosure::new(size);
        let candidate_id = if let Some(preview) = preview {
            let pending = &self.redaction_previews.pending[preview];
            require(self.content_page_dimensions(&pending.candidate.id, page)? == dimensions)?;
            let content = pending.content.as_ref().ok_or_else(invalid)?;
            for (_, boxes) in pending.pages.iter().filter(|(p, _)| p.page_index == page) {
                for rect in boxes {
                    mask.source_rect(*rect, dimensions, true)?;
                }
            }
            for path in content.paths.iter().filter(|p| p.page_index == page) {
                if let Some(rect) = content::page_rect(path.bounds, dimensions[0], dimensions[1])? {
                    mask.source_rect(rect, dimensions, true)?;
                }
            }
            let mut budget = crate::redaction_jpeg_coverage::CoverageBudget::default();
            for image in content.images.iter().filter(|i| i.page_index == page) {
                if let Some(jpeg) = &image.jpeg {
                    for &bound in &jpeg.paint_bounds {
                        if let Some(rect) = content::page_rect(bound, dimensions[0], dimensions[1])?
                        {
                            mask.source_rect(rect, dimensions, false)?;
                        }
                    }
                    let influence = jpeg.page_influence(size, &mut budget).map_err(|error| {
                        EngineError::InvalidRequest(format!(
                            "The image influence could not be disclosed: {error}"
                        ))
                    })?;
                    for &rect in &influence[0] {
                        mask.pixels(rect, 2)?;
                    }
                } else if let Some(rect) =
                    content::page_rect(image.bounds, dimensions[0], dimensions[1])?
                {
                    mask.source_rect(rect, dimensions, true)?;
                }
            }
            Some(pending.candidate.id.clone())
        } else {
            None
        };
        let source_image = self.content_page_rgba(source, page, size)?;
        let mut remaining = MAX_PNG_BYTES;
        let source_png = png(&source_image, &mut remaining)?;
        let (candidate_png, disclosure_png) = if let Some(candidate) = &candidate_id {
            let candidate_image = self.content_page_rgba(candidate, page, size)?;
            #[cfg(test)]
            if let Some(root) = std::env::var_os("FOLIO_CONTENT_RASTER_OUTPUT") {
                let root = PathBuf::from(root);
                std::fs::create_dir_all(&root).unwrap();
                source_image.save(root.join("source.png")).unwrap();
                candidate_image.save(root.join("candidate.png")).unwrap();
                mask.image().save(root.join("disclosure.png")).unwrap();
            }
            mask.verify(&source_image, &candidate_image).map_err(|_| {
                EngineError::InvalidRequest(
                    "The preview changed pixels outside the disclosed content bounds.".into(),
                )
            })?;
            (
                Some(png(&candidate_image, &mut remaining)?),
                Some(png(&mask.image(), &mut remaining)?),
            )
        } else {
            (None, None)
        };
        if let Some(preview) = preview {
            let content = self
                .redaction_previews
                .pending
                .get_mut(preview)
                .ok_or_else(invalid)?
                .content
                .as_mut()
                .ok_or_else(invalid)?;
            if let Some(index) = content.pages.iter().position(|p| p.page_index == page) {
                content.reviewed_pages[index] = true;
            }
        }
        Ok(ContentRedactionRaster {
            source_id: source.into(),
            preview_id: preview.map(str::to_owned),
            candidate_id,
            page_index: page,
            page_width: dimensions[0],
            page_height: dimensions[1],
            width: size[0],
            height: size[1],
            source_png,
            candidate_png,
            disclosure_png,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redaction_content_raster_dimensions_and_budgets_are_bounded_before_work() {
        assert_eq!(device([1., 20000.], 1200).unwrap(), [1, 2400]);
        assert_eq!(device([20000., 1.], 1200).unwrap(), [1200, 1]);
        assert_eq!(device([1200., 2400.], 1200).unwrap(), [1200, 2400]);
        for page in [[0., 10.], [f64::NAN, 10.], [10., 20001.]] {
            assert!(device(page, 900).is_err());
        }
        for width in [0, 63, 1201, u32::MAX] {
            assert!(device([500., 300.], width).is_err());
        }
        let mut writer = PngOutput {
            bytes: vec![],
            limit: 10,
        };
        writer.write_all(&[1; 10]).unwrap();
        assert!(writer.write_all(&[1]).is_err());
        assert_eq!(writer.bytes.len(), 10);
        let mut mask = Disclosure::new([2, 2]);
        mask.work = MAX_MASK_WORK - 1;
        assert!(mask.pixels([0, 0, 2, 1], 2).is_err());
        assert_eq!(mask.flags, [0; 4]);
    }
    #[test]
    fn redaction_content_raster_never_expands_influence_to_fit_changed_pixels() {
        let before = RgbaImage::from_pixel(3, 3, image::Rgba([255; 4]));
        let mut after = before.clone();
        let mut mask = Disclosure::new([3, 3]);
        mask.pixels([0, 0, 1, 1], 3).unwrap();
        after.put_pixel(0, 0, image::Rgba([0, 0, 0, 255]));
        assert!(mask.verify(&before, &after).is_ok());
        after.put_pixel(2, 2, image::Rgba([254, 255, 255, 255]));
        let flags = mask.flags.clone();
        assert!(mask.verify(&before, &after).is_err());
        assert_eq!(mask.flags, flags);
    }
}
