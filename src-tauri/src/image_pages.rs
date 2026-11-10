//! Bounded raster-to-PDF conversion; all pages succeed before a source is published.
use super::{EngineError, EngineResult};
use crate::ImagePageOptions;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use pdfium_render::prelude::*;
use std::{
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_IMAGE_PIXELS: u64 = 32_000_000;
const MAX_TOTAL_PIXELS: u64 = 80_000_000;
fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidRequest(message.into())
}

fn decode(path: &Path, total: &mut u64) -> EngineResult<DynamicImage> {
    let file = std::fs::File::open(path).map_err(|e| invalid(e.to_string()))?;
    let meta = file.metadata().map_err(|e| invalid(e.to_string()))?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return Err(invalid("Choose an image file no larger than 64 MiB."));
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| invalid(e.to_string()))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(invalid("Image exceeds 64 MiB."));
    }
    let format = image::guess_format(&bytes)
        .map_err(|_| invalid("This file is not a readable PNG or JPEG."))?;
    if !matches!(format, ImageFormat::Png | ImageFormat::Jpeg) {
        return Err(invalid("Only PNG and JPEG images are supported."));
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(32768);
    limits.max_image_height = Some(32768);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|e| invalid(e.to_string()))?;
    let (w, h) = decoder.dimensions();
    let pixels = u64::from(w) * u64::from(h);
    if pixels == 0 || pixels > MAX_IMAGE_PIXELS {
        return Err(invalid("Each image must contain 1–32 million pixels."));
    }
    if *total + pixels > MAX_TOTAL_PIXELS {
        return Err(invalid(
            "The images together exceed 80 million pixels. Import fewer at a time.",
        ));
    }
    *total += pixels;
    let orientation = decoder.orientation().map_err(|e| invalid(e.to_string()))?;
    let mut decoded = DynamicImage::from_decoder(decoder).map_err(|e| invalid(e.to_string()))?;
    decoded.apply_orientation(orientation);
    let rgba = decoded.into_rgba8();
    let rgb = image::RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let p = rgba.get_pixel(x, y);
        let a = u32::from(p[3]);
        image::Rgb([0, 1, 2].map(|i| ((u32::from(p[i]) * a + 255 * (255 - a) + 127) / 255) as u8))
    });
    Ok(DynamicImage::ImageRgb8(rgb))
}

pub(super) fn create(
    pdfium: &Pdfium,
    paths: &[PathBuf],
    options: &ImagePageOptions,
) -> EngineResult<Vec<u8>> {
    if paths.is_empty() || paths.len() > 32 {
        return Err(invalid("Choose between 1 and 32 images."));
    }
    if ![options.width, options.height]
        .iter()
        .all(|n| n.is_finite() && (144.0..=1440.0).contains(n))
        || !options.margin.is_finite()
        || options.margin < 0.0
        || options.margin * 2.0 >= options.width.min(options.height)
    {
        return Err(invalid(
            "Choose valid page dimensions (2–20 inches) and margins smaller than half the page.",
        ));
    }
    let mut document = pdfium.create_new_pdf()?;
    let mut total = 0;
    for path in paths {
        let image = decode(path, &mut total).map_err(|e| {
            invalid(format!(
                "{}: {e}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ))
        })?;
        let (mut w, mut h) = (options.width, options.height);
        if options.auto_orientation && (image.width() > image.height()) != (w > h) {
            std::mem::swap(&mut w, &mut h);
        }
        let scale = ((w - 2.0 * options.margin) / image.width() as f32)
            .min((h - 2.0 * options.margin) / image.height() as f32);
        let iw = image.width() as f32 * scale;
        let ih = image.height() as f32 * scale;
        let mut object = PdfPageImageObject::new_with_size(
            &document,
            &image,
            PdfPoints::new(iw),
            PdfPoints::new(ih),
        )?;
        object.translate(
            PdfPoints::new((w - iw) / 2.0),
            PdfPoints::new((h - ih) / 2.0),
        )?;
        let mut page = document
            .pages_mut()
            .create_page_at_end(PdfPagePaperSize::Custom(
                PdfPoints::new(w),
                PdfPoints::new(h),
            ))?;
        page.objects_mut().add_image_object(object)?;
    }
    Ok(document.save_to_bytes()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cumulative_budget_rejects_before_decoding_the_next_image() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("small.png");
        image::RgbImage::new(2, 2).save(&path).unwrap();
        let mut total = MAX_TOTAL_PIXELS - 3;
        assert!(decode(&path, &mut total)
            .unwrap_err()
            .to_string()
            .contains("80 million"));
        assert_eq!(total, MAX_TOTAL_PIXELS - 3);
    }
}
