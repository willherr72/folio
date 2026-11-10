//! OCR preparation and immutable source replacement stay on the PDF worker.
use super::*;
use crate::{
    ocr_pdf::{self, OcrMapping, OcrRecord},
    ocr_types::invalid,
    OcrBounds, OcrPage, OcrPreparedPage, OcrUpdate,
};
use std::cell::RefCell;

#[derive(Default)]
pub(super) struct ReadingCache {
    pages: RefCell<Vec<(String, usize, Option<PageText>)>>,
    plain: RefCell<Vec<String>>,
}
impl ReadingCache {
    pub(super) fn forget(&self, id: &str) {
        self.pages
            .borrow_mut()
            .retain(|(source, _, _)| source != id);
        self.plain.borrow_mut().retain(|source| source != id);
    }
}
const MAX_NATIVE_CHARS: usize = 250_000;

impl PdfEngine {
    pub fn prepare_ocr_page(
        &self,
        source_id: &str,
        page_index: usize,
        rotation: u16,
    ) -> EngineResult<OcrPreparedPage> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::PrepareOcr {
            source_id: source_id.into(),
            page_index,
            rotation,
            reply,
        })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }
    pub fn apply_ocr(
        &self,
        source_id: &str,
        updates: Vec<OcrUpdate>,
    ) -> EngineResult<DocumentInfo> {
        validate_batch(&updates)?;
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::ApplyOcr {
            source_id: source_id.into(),
            updates,
            reply,
        })?;
        self.document_in_points(receive.recv().map_err(|_| EngineError::WorkerStopped)?)
    }
}
fn rotation(value: u16) -> EngineResult<PdfPageRenderRotation> {
    match value {
        0 => Ok(PdfPageRenderRotation::None),
        90 => Ok(PdfPageRenderRotation::Degrees90),
        180 => Ok(PdfPageRenderRotation::Degrees180),
        270 => Ok(PdfPageRenderRotation::Degrees270),
        _ => Err(invalid("rotation must be 0, 90, 180 or 270")),
    }
}
fn validate_batch(updates: &[OcrUpdate]) -> EngineResult<()> {
    if updates.is_empty() || updates.len() > 100 {
        return Err(invalid("choose 1 to 100 source pages"));
    }
    let mut pages = HashSet::new();
    let mut words = 0usize;
    let mut scalars = 0usize;
    for update in updates {
        rotation(update.rotation)?;
        if !pages.insert(update.page_index) {
            return Err(invalid("a source page appears more than once"));
        }
        let (w, s) = update.page.validate()?;
        words += w;
        scalars += s;
        if words > 500_000 || scalars > 2_000_000 {
            return Err(invalid(
                "batch exceeds 500000 words or 2000000 Unicode scalars",
            ));
        }
    }
    Ok(())
}
struct Layout {
    geometry: PageGeometry,
    width: u32,
    height: u32,
    dpi: f32,
    mapping: OcrMapping,
}
impl Layout {
    fn with_unit(mut geometry: PageGeometry, extra: u16, unit:f32) -> EngineResult<Self> {
        rotation(extra)?;
        geometry.rotation = (geometry.rotation + extra) % 360;
        let size = geometry.displayed_size();
        let w = size.width as f64;
        let h = size.height as f64;
        if !w.is_finite() || !h.is_finite() || w <= 0. || h <= 0. {
            return Err(invalid("invalid page dimensions"));
        }
        let scale = (300. * unit as f64 / 72.0f64)
            .min(6000. / w)
            .min(6000. / h)
            .min((20_000_000. / (w * h)).sqrt());
        let width = (w * scale).floor().max(1.) as u32;
        let height = (h * scale).floor().max(1.) as u32;
        let point = |x: f32, y: f32| {
            let (x, y) = geometry.displayed_to_pdf(Point { x, y });
            [x.value, y.value]
        };
        let origin = point(0., 0.);
        let sx = size.width / width as f32;
        let sy = size.height / height as f32;
        let (right, down) = match geometry.rotation {
            0 => ([sx, 0.], [0., -sy]),
            90 => ([0., sx], [sy, 0.]),
            180 => ([-sx, 0.], [0., sy]),
            _ => ([0., -sx], [-sy, 0.]),
        };
        let mapping = OcrMapping {
            origin,
            right,
            down,
        };
        Ok(Self {
            geometry,
            width,
            height,
            dpi: (width as f64 / w).min(height as f64 / h) as f32 * 72. / unit,
            mapping,
        })
    }
    fn bounds(&self, left: f32, bottom: f32, right: f32, top: f32) -> OcrBounds {
        let a = self.geometry.pdf_to_displayed(left, bottom);
        let b = self.geometry.pdf_to_displayed(right, top);
        let size = self.geometry.displayed_size();
        let sx = self.width as f32 / size.width;
        let sy = self.height as f32 / size.height;
        OcrBounds {
            x: a.x.min(b.x) * sx,
            y: a.y.min(b.y) * sy,
            width: (a.x - b.x).abs() * sx,
            height: (a.y - b.y).abs() * sy,
        }
    }
    fn owned(&self, record: &OcrRecord) -> EngineResult<OcrPage> {
        if record.mapping == self.mapping
            && record.page.width == self.width
            && record.page.height == self.height
        {
            return Ok(record.page.clone());
        }
        let mut result = record.page.clone();
        result.width = self.width;
        result.height = self.height;
        for block in &mut result.blocks {
            for paragraph in &mut block.paragraphs {
                for line in &mut paragraph.lines {
                    for word in &mut line.words {
                        let b = &word.bounds;
                        let a = record.mapping.point(b.x, b.y);
                        let z = record.mapping.point(b.x + b.width, b.y + b.height);
                        word.bounds = self.bounds(
                            a[0].min(z[0]),
                            a[1].min(z[1]),
                            a[0].max(z[0]),
                            a[1].max(z[1]),
                        );
                        // Roundoff at a page edge is not a new out-of-page word.
                        let b = &mut word.bounds;
                        let right = (b.x + b.width).min(self.width as f32);
                        let bottom = (b.y + b.height).min(self.height as f32);
                        b.x = b.x.max(0.);
                        b.y = b.y.max(0.);
                        b.width = right - b.x;
                        b.height = bottom - b.y;
                    }
                }
            }
        }
        result.validate()?;
        Ok(result)
    }
}
fn remove_owned(
    document: &mut lopdf::Document,
    page: lopdf::ObjectId,
) -> EngineResult<Option<OcrRecord>> {
    let owned = ocr_pdf::read_owned(document, page)?.map(|o| o.record);
    if let Some(record) = &owned {
        let mut empty = record.page.clone();
        empty.blocks.clear();
        ocr_pdf::replace(document, page, record.mapping.clone(), &empty)?;
    }
    Ok(owned)
}
/// Corrections/removals retain the original raster axes. Reboxing a rotated word
/// and writing horizontally across that new box would turn its character cells.
fn reviewed_record(
    layout: &Layout,
    previous: &OcrRecord,
    edited: &OcrPage,
) -> EngineResult<OcrRecord> {
    let displayed = layout.owned(previous)?;
    let mut expected = displayed.lines().flat_map(|line| &line.words);
    let mut submitted = edited.lines().flat_map(|line| &line.words).peekable();
    let mut record = previous.clone();
    for block in &mut record.page.blocks {
        for paragraph in &mut block.paragraphs {
            for line in &mut paragraph.lines {
                line.words.retain_mut(|word| {
                    let bounds = &expected
                        .next()
                        .expect("owned display retains word order")
                        .bounds;
                    let Some(next) = submitted.peek() else {
                        return false;
                    };
                    let b = &next.bounds;
                    if (bounds.x - b.x).abs() > 0.01
                        || (bounds.y - b.y).abs() > 0.01
                        || (bounds.width - b.width).abs() > 0.01
                        || (bounds.height - b.height).abs() > 0.01
                    {
                        return false;
                    }
                    let next = submitted.next().expect("peeked submitted word");
                    word.text = next.text.clone();
                    word.confidence = next.confidence;
                    true
                });
            }
            paragraph.lines.retain(|line| !line.words.is_empty());
        }
        block
            .paragraphs
            .retain(|paragraph| !paragraph.lines.is_empty());
    }
    record
        .page
        .blocks
        .retain(|block| !block.paragraphs.is_empty());
    if submitted.next().is_some() {
        return Err(invalid(
            "owned review changed word geometry or order; recognize the page again",
        ));
    }
    record.page.validate()?;
    Ok(record)
}
fn exclusions(page: &PdfPage<'_>, layout: &Layout) -> EngineResult<Vec<OcrBounds>> {
    let text = page.text()?;
    let chars = text.chars();
    if chars.len() as usize > MAX_NATIVE_CHARS {
        return Err(invalid("page exceeds 250000 native characters"));
    }
    let mut result = vec![];
    for character in chars.iter() {
        if char::from_u32(character.unicode_value()).is_some_and(char::is_whitespace) {
            continue;
        }
        if let Ok(b) = character
            .tight_bounds()
            .or_else(|_| character.loose_bounds())
        {
            let b = layout.bounds(
                b.left().value,
                b.bottom().value,
                b.right().value,
                b.top().value,
            );
            if [b.x, b.y, b.width, b.height].iter().all(|n| n.is_finite())
                && b.width > 0.
                && b.height > 0.
            {
                result.push(b);
            }
        }
    }
    Ok(result)
}
fn exclude(page: &mut OcrPage, bounds: &[OcrBounds]) -> EngineResult<()> {
    // A uniform grid avoids words × characters work on dense mixed pages.
    const CELLS: usize = 64;
    let mut grid = vec![Vec::<usize>::new(); CELLS * CELLS];
    let mut broad = vec![];
    let cell = |x: f32, y: f32| {
        (
            (x / page.width as f32 * CELLS as f32)
                .floor()
                .clamp(0., 63.) as usize,
            (y / page.height as f32 * CELLS as f32)
                .floor()
                .clamp(0., 63.) as usize,
        )
    };
    for (i, b) in bounds.iter().enumerate() {
        let (x0, y0) = cell(b.x - 0.5, b.y - 0.5);
        let (x1, y1) = cell(b.x + b.width + 0.5, b.y + b.height + 0.5);
        if (x1 - x0 + 1) * (y1 - y0 + 1) > 64 {
            broad.push(i);
            continue;
        }
        for y in y0..=y1 {
            for x in x0..=x1 {
                grid[y * CELLS + x].push(i);
            }
        }
    }
    let mut comparisons = 0usize;
    for block in &mut page.blocks {
        for paragraph in &mut block.paragraphs {
            for line in &mut paragraph.lines {
                line.words.retain(|word| {
                    let w = &word.bounds;
                    let x = w.x + w.width / 2.;
                    let y = w.y + w.height / 2.;
                    let (x0, y0) = cell(w.x - 0.5, w.y - 0.5);
                    let (x1, y1) = cell(w.x + w.width + 0.5, w.y + w.height + 0.5);
                    comparisons += (x1 - x0 + 1) * (y1 - y0 + 1);
                    if comparisons > 5_000_000 {
                        return false;
                    }
                    let grid = &grid;
                    !(y0..=y1)
                        .flat_map(|cy| (x0..=x1).flat_map(move |cx| grid[cy * CELLS + cx].iter()))
                        .chain(&broad)
                        .any(|&i| {
                            comparisons += 1;
                            if comparisons > 5_000_000 {
                                return true;
                            }
                            let b = &bounds[i];
                            (x >= b.x - 0.5
                                && x <= b.x + b.width + 0.5
                                && y >= b.y - 0.5
                                && y <= b.y + b.height + 0.5)
                                || (b.x + b.width / 2. >= w.x - 0.5
                                    && b.x + b.width / 2. <= w.x + w.width + 0.5
                                    && b.y + b.height / 2. >= w.y - 0.5
                                    && b.y + b.height / 2. <= w.y + w.height + 0.5)
                        })
                });
                if comparisons > 5_000_000 {
                    return Err(invalid(
                        "native text overlap checks exceed the supported work limit",
                    ));
                }
            }
        }
    }
    Ok(())
}
fn raster(
    page: &PdfPage<'_>,
    extra: u16,
    width: u32,
    height: u32,
    annotations: bool,
) -> EngineResult<image::DynamicImage> {
    // Fixed dimensions allocate the bitmap but do not scale page content on
    // PDFium's matrix path (used when form rendering is disabled). Scale the
    // already rotated display dimensions explicitly to the admitted raster.
    let (display_width, display_height) = if extra == 90 || extra == 270 {
        (page.height().value, page.width().value)
    } else {
        (page.width().value, page.height().value)
    };
    Ok(page
        .render_with_config(
            &PdfRenderConfig::new()
                .set_fixed_size(width as i32, height as i32)
                .scale_page_width_by_factor(width as f32 / display_width)
                .scale_page_height_by_factor(height as f32 / display_height)
                .rotate(rotation(extra)?, false)
                .render_annotations(annotations)
                .render_form_data(annotations),
        )?
        .as_image()?)
}
impl WorkerRuntime {
    fn ocr_document(&self, id: &str) -> EngineResult<(&OpenDocument, lopdf::Document)> {
        let source = self.editing_document(id)?;
        self.copying_document(id)?;
        if !source.pdf()?.signatures().is_empty() {
            return Err(invalid(
                "signed PDFs cannot be changed without invalidating their signatures",
            ));
        }
        let (document, _) = crate::acroforms::filling_document(source.bytes()?.as_ref())?;
        if document.catalog().map_err(invalid)?.has(b"Perms") {
            return Err(invalid("certified PDFs are not supported for OCR"));
        }
        Ok((source, document))
    }
    pub(super) fn prepare_ocr_page(
        &self,
        id: &str,
        page_index: usize,
        extra: u16,
    ) -> EngineResult<OcrPreparedPage> {
        let (source, mut document) = self.ocr_document(id)?;
        let page = source.pdf()?.pages().get(page_index_i32(page_index)?)?;
        let unit=*source.units()?.get(page_index).ok_or_else(||invalid("page does not exist"))?;
        let layout = Layout::with_unit(page_geometry(&page)?, extra,unit)?;
        let page_id = *document
            .get_pages()
            .values()
            .nth(page_index)
            .ok_or_else(|| invalid("page does not exist"))?;
        let owned = remove_owned(&mut document, page_id)?;
        let baseline = self
            .pdfium
            .load_pdf_from_reader(Cursor::new(crate::pdf_output::save(&mut document)?), None)?;
        let mut recognition_page = baseline.pages().get(page_index_i32(page_index)?)?;
        let excluded = exclusions(&recognition_page, &layout)?;
        // Set rotation only on this private document. PDFium's display transform
        // handles intrinsic rotation and nonzero crop origins together; the
        // render-config extra-rotation matrix does not do so reliably.
        recognition_page.set_rotation(rotation(layout.geometry.rotation)?);
        let image = raster(&recognition_page, 0, layout.width, layout.height, false)?;
        let mut png = crate::pdf_output::BoundedWriter::new(32 * 1024 * 1024);
        // PNG encoding needs Seek; the image encoder itself can stream to a bounded writer.
        let rgba = image.to_rgba8();
        image::ImageEncoder::write_image(
            image::codecs::png::PngEncoder::new(&mut png),
            rgba.as_raw(),
            layout.width,
            layout.height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(invalid)?;
        Ok(OcrPreparedPage {
            png: png.finish()?,
            width: layout.width,
            height: layout.height,
            effective_dpi: layout.dpi,
            exclusions: excluded,
            owned: owned
                .as_ref()
                .map(|record| layout.owned(record))
                .transpose()?,
        })
    }
    pub(super) fn apply_ocr(
        &mut self,
        id: &str,
        updates: Vec<OcrUpdate>,
    ) -> EngineResult<DocumentInfo> {
        validate_batch(&updates)?;
        let (source, mut document) = self.ocr_document(id)?;
        let policy = source.policy.clone();
        let original = source.original_path.clone();
        let legacy_comment_import = source.legacy_comment_import;
        let pages: Vec<_> = document.get_pages().values().copied().collect();
        let mut layouts = vec![];
        let mut previous = vec![];
        let mut baseline = document.clone();
        for update in &updates {
            let page = source
                .pdf()?
                .pages()
                .get(page_index_i32(update.page_index)?)?;
            let unit=*source.units()?.get(update.page_index).ok_or_else(||invalid("page does not exist"))?;
            let layout = Layout::with_unit(page_geometry(&page)?, update.rotation,unit)?;
            if (layout.width, layout.height) != (update.page.width, update.page.height) {
                return Err(invalid(
                    "raster dimensions no longer match this source page",
                ));
            }
            let page_id = *pages
                .get(update.page_index)
                .ok_or_else(|| invalid("page does not exist"))?;
            previous.push(remove_owned(&mut baseline, page_id)?);
            layouts.push(layout);
        }
        let baseline = self
            .pdfium
            .load_pdf_from_reader(Cursor::new(crate::pdf_output::save(&mut baseline)?), None)?;
        let mut accepted = vec![];
        let mut changed = false;
        for ((mut update, layout), previous) in updates.into_iter().zip(&layouts).zip(previous) {
            let page = baseline.pages().get(page_index_i32(update.page_index)?)?;
            exclude(&mut update.page, &exclusions(&page, layout)?)?;
            let written = if update.preserve_owned_orientation {
                reviewed_record(
                    layout,
                    previous.as_ref().ok_or_else(|| {
                        invalid("owned review requires a verified existing OCR layer")
                    })?,
                    &update.page,
                )?
            } else {
                OcrRecord {
                    version: 1,
                    mapping: layout.mapping.clone(),
                    page: update.page.clone(),
                }
            };
            changed |= previous
                .as_ref()
                .map_or(written.page.validate()?.0 != 0, |p| p != &written);
            ocr_pdf::replace(
                &mut document,
                pages[update.page_index],
                written.mapping.clone(),
                &written.page,
            )?;
            accepted.push((update, written));
        }
        if !changed {
            return Err(invalid("No new searchable text to apply."));
        }
        let bytes = crate::pdf_output::save(&mut document)?;
        let candidate = self
            .pdfium
            .load_pdf_from_reader(Cursor::new(bytes.clone()), None)?;
        if candidate.pages().len() as usize != source.document.page_count() {
            return Err(invalid("page count changed during OCR"));
        }
        let mut proof = crate::pdf_output::load(&bytes)?;
        let proof_pages: Vec<_> = proof.get_pages().values().copied().collect();
        for ((update, written), layout) in accepted.iter().zip(&layouts) {
            // The open runtime has imported editable annotations removed. Compare
            // complete source appearances against the complete candidate instead.
            let before = baseline.pages().get(page_index_i32(update.page_index)?)?;
            let after = candidate.pages().get(page_index_i32(update.page_index)?)?;
            let g = page_geometry(&after)?;
            let original = page_geometry(&before)?;
            if [g.left, g.bottom, g.right, g.top]
                != [original.left, original.bottom, original.right, original.top]
                || g.rotation != original.rotation
            {
                return Err(invalid("page geometry changed during OCR"));
            }
            let scale = 1200. / layout.width.max(layout.height) as f32;
            let w = (layout.width as f32 * scale.min(1.)).round().max(1.) as u32;
            let h = (layout.height as f32 * scale.min(1.)).round().max(1.) as u32;
            if raster(&before, update.rotation, w, h, true)?.to_rgba8()
                != raster(&after, update.rotation, w, h, true)?.to_rgba8()
            {
                return Err(invalid("page appearance changed during OCR"));
            }
            let record = ocr_pdf::read_owned(&mut proof, proof_pages[update.page_index])?;
            if update.page.validate()?.0 > 0 && !record.is_some_and(|r| r.record == *written) {
                return Err(invalid(
                    "saved searchable text failed ownership verification",
                ));
            }
            // Test the native reader in the recognition orientation, independently
            // of our metadata-assisted workspace reading order.
            let mut copy = self.pdfium.create_new_pdf()?;
            copy.pages_mut().copy_page_from_document(
                &candidate,
                page_index_i32(update.page_index)?,
                0,
            )?;
            let mut page = copy.pages().get(0)?;
            let right = written.mapping.right;
            let reading_rotation = if right[0] > 0. {
                0
            } else if right[1] > 0. {
                90
            } else if right[0] < 0. {
                180
            } else {
                270
            };
            page.set_rotation(rotation(reading_rotation)?);
            // `all()` uses rotated width/height at (0,0), not raw PDF CropBox
            // coordinates, and can omit valid text on rotated/cropped pages.
            let extracted = page
                .text()?
                .inside_rect(page.boundaries().bounding()?.bounds)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            for word in update.page.lines().flat_map(|l| &l.words) {
                let expected = word.text.split_whitespace().collect::<Vec<_>>().join(" ");
                if !extracted.contains(&expected) {
                    return Err(invalid(
                        "saved searchable text could not be extracted correctly",
                    ));
                }
            }
        }
        drop(candidate);
        drop(baseline);
        let path = std::env::temp_dir().join(format!("Folio-OCR-{}.pdf", Uuid::new_v4()));
        let info = self.open_bytes(
            bytes,
            path,
            original,
            true,
            false,
            legacy_comment_import,
            OpenSecurity::Restored(policy),
        )?;
        self.retain_annotation_sources(id, info)
    }

    pub(super) fn read_ocr_composed_text(&self,bytes:&[u8],source_document:&PdfDocument<'_>,page_index:usize)->EngineResult<Option<PageText>> {
        let document=crate::pdf_output::load(bytes)?;
        if !document.objects.values().any(|o|o.as_stream().is_ok_and(|s|s.dict.has(b"FolioOCR"))){return Ok(None);}
        // Logical records are optional evidence. Failure to prove ownership must
        // never authorize their use or prevent bounded ordinary text extraction.
        Ok(self.read_ocr_parsed_text(document,source_document,page_index).unwrap_or(None))
    }
    fn read_ocr_parsed_text(&self,mut document:lopdf::Document,source_document:&PdfDocument<'_>,page_index:usize)->EngineResult<Option<PageText>> {
                let page = *document
                    .get_pages()
                    .values()
                    .nth(page_index)
                    .ok_or_else(|| invalid("page does not exist"))?;
                let Some(record) = remove_owned(&mut document, page)? else {
                    return Ok(None);
                };
                let baseline = self.pdfium.load_pdf_from_reader(
                    Cursor::new(crate::pdf_output::save(&mut document)?),
                    None,
                )?;
                let mut result =
                    self.read_document_page_text(&baseline, page_index, Some(MAX_NATIVE_CHARS))?;
                let geometry =
                    page_geometry(&source_document.pages().get(page_index_i32(page_index)?)?)?;
                let separator = |characters: &mut Vec<PdfTextCharacter>, value: &str| {
                    if !characters.is_empty() {
                        characters.push(PdfTextCharacter {
                            text: value.into(),
                            x: 0.,
                            y: 0.,
                            width: 0.,
                            height: 0.,
                        });
                    }
                };
                let count=record.page.lines().try_fold(result.characters.len(),|total,line|line.words.iter().try_fold(total+1,|n,word|n.checked_add(word.text.chars().count()+1))).ok_or_else(||invalid("OCR text count overflow"))?;
                if count>MAX_NATIVE_CHARS{return Err(invalid("page exceeds the text character limit"));}
                for line in record.page.lines().filter(|line| !line.words.is_empty()) {
                    separator(&mut result.characters, "\n");
                    for (i, word) in line.words.iter().enumerate() {
                        if i > 0 {
                            separator(&mut result.characters, " ");
                        }
                        let b = &word.bounds;
                        let width = b.width / word.text.chars().count() as f32;
                        for (j, c) in word.text.chars().enumerate() {
                            let a = record.mapping.point(b.x + j as f32 * width, b.y);
                            let z = record
                                .mapping
                                .point(b.x + (j + 1) as f32 * width, b.y + b.height);
                            let a = geometry.pdf_to_displayed(a[0], a[1]);
                            let z = geometry.pdf_to_displayed(z[0], z[1]);
                            result.characters.push(PdfTextCharacter {
                                text: c.to_string(),
                                x: a.x.min(z.x),
                                y: a.y.min(z.y),
                                width: (a.x - z.x).abs(),
                                height: (a.y - z.y).abs(),
                            });
                        }
                    }
                }
                Ok(Some(result))
    }

    pub(super) fn read_ocr_page_text(
        &self,
        id: &str,
        page_index: usize,
        limit: Option<usize>,
    ) -> EngineResult<Option<PageText>> {
        if self
            .ocr_reading
            .plain
            .borrow()
            .iter()
            .any(|source| source == id)
        {
            return Ok(None);
        }
        let cached = self
            .ocr_reading
            .pages
            .borrow()
            .iter()
            .find(|(source, index, _)| source == id && *index == page_index)
            .map(|(_, _, text)| text.clone());
        let result = if let Some(cached) = cached {
            cached
        } else {
            let source = self.document(id)?;
            // Backing I/O failures are retryable, not absent OCR proof.
            let source_bytes = source.bytes()?;
            // Ordinary or modified PDFs retain the existing reader. Only a complete
            // regenerated proof may replace PDFium's ordering of invisible text.
            let result = (|| -> EngineResult<Option<PageText>> {
                let document = crate::pdf_output::load(source_bytes.as_ref())?;
                // Scan parsed objects once, including compressed object streams.
                // Ordinary PDFs then retain their existing per-page read cost.
                if !document
                    .objects
                    .values()
                    .any(|object| object.as_stream().is_ok_and(|s| s.dict.has(b"FolioOCR")))
                {
                    let mut plain = self.ocr_reading.plain.borrow_mut();
                    if plain.len() >= 64 {
                        plain.remove(0);
                    }
                    plain.push(id.into());
                    return Ok(None);
                }
                self.read_ocr_parsed_text(document,source.pdf()?,page_index)
            })()
            .unwrap_or(None);
            let mut cache = self.ocr_reading.pages.borrow_mut();
            // Cache at most eight pages / 500000 characters across immutable sources.
            let count = result.as_ref().map_or(0, |p| p.characters.len());
            if count <= 500_000 {
                while !cache.is_empty()
                    && (cache.len() >= 8
                        || cache
                            .iter()
                            .map(|(_, _, p)| p.as_ref().map_or(0, |p| p.characters.len()))
                            .sum::<usize>()
                            + count
                            > 500_000)
                {
                    cache.remove(0);
                }
                cache.push((id.into(), page_index, result.clone()));
            }
            result
        };
        if result
            .as_ref()
            .is_some_and(|p| limit.is_some_and(|n| p.characters.len() > n))
        {
            return Err(invalid("page exceeds the search character limit"));
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cold_ocr_backing_failure_does_not_poison_reading_order_cache() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut runtime = WorkerRuntime::new(
            &root.join("resources/pdfium/pdfium.dll"),
            Arc::new(FontRegistry::new()),
        ).unwrap();
        runtime.source_store = crate::source_storage::SourceStore::with_test_limits(0, 1);
        let blank = runtime.create_blank_document(200., 200.).unwrap();
        let prep = runtime.prepare_ocr_page(&blank.id, 0, 0).unwrap();
        let page: OcrPage = serde_json::from_value(serde_json::json!({
            "width": prep.width, "height": prep.height,
            "blocks": [{"paragraphs": [{"lines": [{"words": [
                {"text": "first", "confidence": 90,
                 "bounds": {"x": 100, "y": 100, "width": 100, "height": 30}},
                {"text": "second", "confidence": 90,
                 "bounds": {"x": 300, "y": 100, "width": 120, "height": 30}}
            ]}]}]}]
        })).unwrap();
        let output = runtime.apply_ocr(&blank.id, vec![OcrUpdate {
            preserve_owned_orientation: false, page_index: 0, rotation: 0, page,
        }]).unwrap();
        let expected = runtime.read_ocr_page_text(&output.id, 0, None).unwrap().unwrap();
        assert_eq!(expected.characters.iter().map(|c| c.text.as_str()).collect::<String>(), "first second");
        runtime.ocr_reading.forget(&output.id);
        let backing = runtime.document(&output.id).unwrap().source_bytes.clone();
        for source in runtime.documents.values_mut() { source.document.evict(); }
        runtime.source_store.clear_test_raw_cache();
        let saved = backing.take_test_backing();
        for _ in 0..2 {
            assert!(matches!(runtime.page_text(&output.id, 0), Err(EngineError::Io(_))));
            assert!(!runtime.ocr_reading.pages.borrow().iter().any(|(id, _, _)| id == &output.id),
                    "Backing I/O failure must not cache absent OCR proof");
            assert!(!runtime.ocr_reading.plain.borrow().contains(&output.id));
        }
        backing.restore_test_backing(&saved);
        let restored = runtime.read_ocr_page_text(&output.id, 0, None).unwrap()
            .expect("Repaired backing must restore verified OCR reading order");
        assert_eq!(serde_json::to_value(&restored).unwrap(), serde_json::to_value(&expected).unwrap());
        assert_eq!(serde_json::to_value(runtime.page_text(&output.id, 0).unwrap()).unwrap(),
                   serde_json::to_value(expected).unwrap());
    }
    #[test]
    fn dropped_ocr_reply_reclaims_source_and_prepared_print_cannot_be_recognized() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut runtime = WorkerRuntime::new(
            &root.join("resources/pdfium/pdfium.dll"),
            Arc::new(FontRegistry::new()),
        )
        .unwrap();
        let mut document = runtime.pdfium.create_new_pdf().unwrap();
        document
            .pages_mut()
            .create_page_at_end(PdfPagePaperSize::a4())
            .unwrap();
        let info = runtime
            .register_generated_document(
                crate::pdf_output::from_pdfium(&document).unwrap(),
                "Blank.pdf",
            )
            .unwrap();
        let prep = runtime.prepare_ocr_page(&info.id, 0, 0).unwrap();
        let page:OcrPage=serde_json::from_value(serde_json::json!({"width":prep.width,"height":prep.height,"blocks":[{"paragraphs":[{"lines":[{"words":[
            {"text":"test","confidence":90,"bounds":{"x":100,"y":100,"width":100,"height":30}}
        ]}]}]}]})).unwrap();
        let before = runtime.fonts.ids().unwrap();
        let (reply, receive) = mpsc::channel();
        drop(receive);
        let (send, requests) = mpsc::channel();
        send.send(WorkerRequest::ApplyOcr {
            source_id: info.id.clone(),
            updates: vec![OcrUpdate {
                preserve_owned_orientation: false,
                page_index: 0,
                rotation: 0,
                page,
            }],
            reply,
        })
        .unwrap();
        drop(send);
        runtime.run(requests);
        assert_eq!(runtime.documents.len(), 1);
        assert!(runtime.documents.contains_key(&info.id));
        assert_eq!(runtime.fonts.ids().unwrap(), before);
        let request=serde_json::from_value(serde_json::json!({"pages":[{"id":"p","sourceId":info.id,"pageIndex":0,
            "width":info.pages[0].width,"height":info.pages[0].height,"rotation":0,"overlays":[]}]})).unwrap();
        let print = runtime.prepare_print(request).unwrap();
        assert!(matches!(
            runtime.prepare_ocr_page(&print.id, 0, 0),
            Err(EngineError::PermissionDenied(_))
        ));
    }
    #[test]
    fn excludes_native_words_across_letter_gaps_but_keeps_adjacent_scan_text() {
        let mut page:OcrPage=serde_json::from_value(serde_json::json!({"width":100,"height":100,"blocks":[{"paragraphs":[{"lines":[{"words":[
            {"text":"Native","confidence":90,"bounds":{"x":5,"y":5,"width":20,"height":10}},
            {"text":"scan","confidence":90,"bounds":{"x":30,"y":40,"width":25,"height":10}}
        ]}]}]}]})).unwrap();
        exclude(
            &mut page,
            &[
                OcrBounds {
                    x: 6.,
                    y: 6.,
                    width: 4.,
                    height: 8.,
                },
                OcrBounds {
                    x: 20.,
                    y: 6.,
                    width: 4.,
                    height: 8.,
                },
            ],
        )
        .unwrap();
        assert_eq!(page.blocks[0].paragraphs[0].lines[0].words.len(), 1);
        assert_eq!(page.blocks[0].paragraphs[0].lines[0].words[0].text, "scan");
    }
    #[test]
    fn exclusion_work_is_bounded_even_for_many_overlapping_large_characters() {
        let mut page:OcrPage=serde_json::from_value(serde_json::json!({"width":100,"height":100,"blocks":[{"paragraphs":[{"lines":[{"words":[
            {"text":"test","confidence":90,"bounds":{"x":80,"y":80,"width":10,"height":10}}
        ]}]}]}]})).unwrap();
        let word = page.blocks[0].paragraphs[0].lines[0].words[0].clone();
        page.blocks[0].paragraphs[0].lines[0].words = vec![word; 20_000];
        let bounds = vec![
            OcrBounds {
                x: 0.,
                y: 0.,
                width: 50.,
                height: 50.
            };
            250_000
        ];
        assert!(exclude(&mut page, &bounds).is_err());
    }
}
