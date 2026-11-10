//! Existing-image facade and immutable full-source publication.
use super::*;
use crate::existing_image_model::{Model, Occurrence};
use crate::{ExistingImageEdit, ImageFitMode};
use sha2::{Digest, Sha256};

pub(super) enum Request {
    Edit {
        source: String,
        occurrences: Vec<usize>,
        selected: usize,
        path: Vec<usize>,
        selection_id: String,
        edit: ExistingImageEdit,
        reply: mpsc::Sender<EngineResult<PageContentReplacement>>,
    },
    List {
        source: String,
        page: usize,
        reply: mpsc::Sender<EngineResult<ExistingImages>>,
    },
    Preview {
        source: String,
        occurrences: Vec<usize>,
        selected: usize,
        path: Vec<usize>,
        selection_id: String,
        asset: String,
        mode: ImageFitMode,
        reply: mpsc::Sender<EngineResult<PageContentReplacement>>,
    },
}
enum ProjectedChange<'a> {
    Replace(&'a crate::PreparedPdfImage, ImageFitMode, f64),
    Edit(ExistingImageEdit),
}
fn bad(reason: &str) -> EngineError {
    EngineError::InvalidRequest(reason.into())
}
fn identity(source: &str, page: usize, image: &Occurrence) -> String {
    let mut hash = Sha256::new();
    hash.update(source.as_bytes());
    hash.update((page as u64).to_le_bytes());
    for index in &image.path {
        hash.update((*index as u64).to_le_bytes());
    }
    for value in image.matrix {
        hash.update(value.to_le_bytes());
    }
    hash.update(image.width.to_le_bytes());
    hash.update(image.height.to_le_bytes());
    if let Some(crop) = image.crop {
        hash.update([1]);
        for value in [crop.left, crop.top, crop.right, crop.bottom] {
            hash.update((if value == 0. { 0f64 } else { value }).to_le_bytes());
        }
    } else {
        hash.update([0]);
    }
    hash.finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn described(
    source: &str,
    page: usize,
    image: &Occurrence,
    geometry: PageGeometry,
) -> ExistingImage {
    let corners = image
        .corners
        .map(|p| geometry.pdf_to_displayed(p[0] as f32, p[1] as f32));
    let visible = image.crop.map_or(image.corners, |crop| crop.corners(image.matrix))
        .map(|p| geometry.pdf_to_displayed(p[0] as f32, p[1] as f32));
    let left = visible.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
    let top = visible.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
    let right = visible
        .iter()
        .map(|p| p.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let bottom = visible
        .iter()
        .map(|p| p.y)
        .fold(f32::NEG_INFINITY, f32::max);
    ExistingImage {
        object_path: image.path.clone(),
        selection_id: identity(source, page, image),
        corners,
        bounds: AnnotationRect {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        },
        pixel_width: image.width,
        pixel_height: image.height,
        crop: image.crop,
    }
}
impl Scale for ExistingImages {
    fn scale(&mut self, f: f32) -> EngineResult<()> {
        for image in &mut self.images {
            image.bounds.scale(f)?;
            for point in &mut image.corners {
                point.scale(f)?;
            }
        }
        Ok(())
    }
}
impl PdfEngine {
    pub fn preview_image_edit_occurrence(
        &self,
        source: &str,
        occurrences: &[usize],
        selected: usize,
        path: &[usize],
        selection_id: &str,
        mut edit: ExistingImageEdit,
    ) -> EngineResult<PageContentReplacement> {
        if occurrences.is_empty()
            || occurrences.len() > 100_000
            || selected >= occurrences.len()
            || path.is_empty()
            || path.len() > 13
            || path.iter().any(|i| *i >= 100_000)
            || selection_id.len() != 64
        {
            return Err(bad(
                "The image selection is invalid. Select the image again.",
            ));
        }
        if let ExistingImageEdit::Adjust { crop, .. } = &edit {
            crop.validate()?;
        }
        if let ExistingImageEdit::Transform { transform } | ExistingImageEdit::Adjust {transform, ..} = &mut edit {
            transform.validate()?;
            let unit = self.page_unit(source, occurrences[selected])? as f64;
            transform.delta_x /= unit;
            transform.delta_y /= unit;
        }
        let mut result = self.image_request(|reply| Request::Edit {
            source: source.into(),
            occurrences: occurrences.to_vec(),
            selected,
            path: path.to_vec(),
            selection_id: selection_id.into(),
            edit,
            reply,
        })?;
        let id = result.document.id.clone();
        result.document = match self.document_in_points(Ok(result.document)) {
            Ok(doc) => doc,
            Err(error) => {
                let _ = self.close_document(&id);
                return Err(error);
            }
        };
        Ok(result)
    }
    fn image_request<T>(
        &self,
        make: impl FnOnce(mpsc::Sender<EngineResult<T>>) -> Request,
    ) -> EngineResult<T> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::ExistingImage(make(reply)))?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }
    pub fn list_existing_images(&self, source: &str, page: usize) -> EngineResult<ExistingImages> {
        self.in_points(
            source,
            page,
            self.image_request(|reply| Request::List {
                source: source.into(),
                page,
                reply,
            }),
        )
    }
    pub fn preview_image_replacement_occurrence(
        &self,
        source: &str,
        occurrences: &[usize],
        selected: usize,
        path: &[usize],
        selection_id: &str,
        asset: &str,
        mode: ImageFitMode,
    ) -> EngineResult<PageContentReplacement> {
        if occurrences.is_empty()
            || occurrences.len() > 100_000
            || selected >= occurrences.len()
            || path.is_empty()
            || path.len() > 13
            || path.iter().any(|i| *i >= 100_000)
            || selection_id.len() != 64
            || asset.len() > 128
        {
            return Err(bad(
                "The image selection is invalid or too large. Select the image again.",
            ));
        }
        let mut result = self.image_request(|reply| Request::Preview {
            source: source.into(),
            occurrences: occurrences.to_vec(),
            selected,
            path: path.to_vec(),
            selection_id: selection_id.into(),
            asset: asset.into(),
            mode,
            reply,
        })?;
        let id = result.document.id.clone();
        result.document = match self.document_in_points(Ok(result.document)) {
            Ok(doc) => doc,
            Err(error) => {
                let _ = self.close_document(&id);
                return Err(error);
            }
        };
        Ok(result)
    }
}
impl WorkerRuntime {
    pub(super) fn handle_existing_image(&mut self, request: Request) {
        match request {
            Request::Edit {
                source,
                occurrences,
                selected,
                path,
                selection_id,
                edit,
                reply,
            } => {
                let result = self
                    .publish_text_occurrence(&source, &occurrences, selected, |rt, page| {
                        rt.image_source_preflight(&source, page)?;
                        rt.with_text_projection(&source, page, |rt, projection| {
                            rt.replace_projected_image(
                                &source,
                                page,
                                projection,
                                &path,
                                &selection_id,
                                ProjectedChange::Edit(edit),
                            )
                        })
                    })
                    .map(|mut result| {
                        result.document.name = "Edited image.pdf".into();
                        result
                    });
                if let Err(mpsc::SendError(Ok(result))) = reply.send(result) {
                    self.discard_unpublished_source(&result.document.id);
                }
            }
            Request::List {
                source,
                page,
                reply,
            } => {
                let result = self.list_existing_images_raw(&source, page);
                let _ = reply.send(result);
            }
            Request::Preview {
                source,
                occurrences,
                selected,
                path,
                selection_id,
                asset,
                mode,
                reply,
            } => {
                let result = self
                    .publish_text_occurrence(&source, &occurrences, selected, |rt, page| {
                        rt.image_source_preflight(&source, page)?;
                        let source_doc = rt.editing_document(&source)?;
                        rt.images.authorize(
                            &asset,
                            source_doc.policy.as_ref().map(SourcePolicy::image_origin),
                        )?;
                        let image = rt.images.get(&asset)?;
                        let ratio = image.info().width as f64 / image.info().height as f64;
                        let prepared = rt.images.prepared_pdf(&asset)?;
                        rt.with_text_projection(&source, page, |rt, projection| {
                            rt.replace_projected_image(
                                &source,
                                page,
                                projection,
                                &path,
                                &selection_id,
                                ProjectedChange::Replace(&prepared, mode, ratio),
                            )
                        })
                    })
                    .map(|mut result| {
                        result.document.name = "Edited image.pdf".into();
                        result
                    });
                if let Err(mpsc::SendError(Ok(result))) = reply.send(result) {
                    self.discard_unpublished_source(&result.document.id);
                }
            }
        }
    }
    fn image_source_preflight(&self, source: &str, page: usize) -> EngineResult<()> {
        let document = self.editing_document(source)?;
        Model::read(document.bytes()?.as_ref(), page)?;
        Ok(())
    }
    fn list_existing_images_raw(
        &mut self,
        source: &str,
        page: usize,
    ) -> EngineResult<ExistingImages> {
        self.image_source_preflight(source, page)?;
        self.with_text_projection(source, page, |rt, projection| {
            let document = rt.editing_document(projection)?;
            let model = Model::read(document.bytes()?.as_ref(), 0)?;
            let geometry = page_geometry(&document.pdf()?.pages().get(0)?)?;
            Ok(ExistingImages {
                images: model
                    .images()
                    .iter()
                    .map(|image| described(source, page, image, geometry))
                    .collect(),
            })
        })
    }
    fn register_image_candidate(
        &mut self,
        source: &str,
        bytes: Vec<u8>,
    ) -> EngineResult<DocumentInfo> {
        let original = self.editing_document(source)?;
        let policy = original.policy.clone();
        let original_path = original.original_path.clone();
        let legacy = original.legacy_comment_import;
        let mut result = self.register_generated_document(bytes, "Image preview.pdf")?;
        let doc = self.documents.get_mut(&result.id).unwrap();
        doc.policy = policy;
        doc.original_path = original_path;
        doc.legacy_comment_import = legacy;
        result.protection = doc.policy.as_ref().map(SourcePolicy::info);
        Ok(result)
    }
    fn replace_projected_image(
        &mut self,
        source: &str,
        page: usize,
        projection: &str,
        path: &[usize],
        expected: &str,
        change: ProjectedChange<'_>,
    ) -> EngineResult<DocumentInfo> {
        // Freeze inherited annotation appearance bindings before replacing any page
        // alias. Publication independently retains the original full-source context.
        let mut pdf = crate::pdf_output::load(self.editing_document(projection)?.bytes()?.as_ref())?;
        let page_id = *pdf
            .get_pages()
            .values()
            .next()
            .ok_or_else(|| bad("Image preview page is missing."))?;
        page_content_replace::retain_appearance_contexts(&mut pdf, page_id)?;
        let bytes = crate::pdf_output::save(&mut pdf)?;
        drop(pdf);
        let mut model = Model::read(&bytes, 0)?;
        let image = model
            .images()
            .iter()
            .find(|v| v.path == path)
            .ok_or_else(|| bad("The image selection changed. Select it again."))?
            .clone();
        if identity(source, page, &image) != expected {
            return Err(bad("The image selection changed. Select it again."));
        }
        let clone_bytes = model.rewrite(path, None)?;
        drop(model);
        let clone = self.register_image_candidate(projection, clone_bytes)?;
        let checked = self.verify_text_publication(&clone.id, 0, projection);
        self.discard_unpublished_source(&clone.id);
        checked?;
        let mut model = Model::read(&bytes, 0)?;
        let originals = model.images().to_vec();
        let mut expected_crop = image.crop;
        let mut restoring = false;
        let mut image_proof = None;
        let (output, target) = match change {
            ProjectedChange::Replace(prepared, mode, ratio) => (
                model.rewrite(path, Some((prepared, mode, ratio)))?,
                Some(image.matrix),
            ),
            ProjectedChange::Edit(ExistingImageEdit::Adjust {transform, crop}) => {
                restoring = crop == crate::ImageCrop::FULL;
                let target = transform.matrix(image.matrix, page_geometry(
                    &self.editing_document(projection)?.pdf()?.pages().get(0)?,
                )?.rotation)?;
                if image.crop != Some(crop) && !(image.crop.is_none() && crop == crate::ImageCrop::FULL) {
                    expected_crop = Some(crop.serialized()?);
                }
                image_proof = Some(model.image_fingerprint(path)?);
                (model.adjust(path,target,crop)?,Some(target))
            },
            ProjectedChange::Edit(edit) => {
                let target = match edit {
                    ExistingImageEdit::Remove => None,
                    ExistingImageEdit::Transform { transform } => Some(
                        transform.matrix(
                            image.matrix,
                            page_geometry(
                                &self.editing_document(projection)?.pdf()?.pages().get(0)?,
                            )?
                            .rotation,
                        )?,
                    ),
                    ExistingImageEdit::Adjust { .. } => unreachable!(),
                };
                (model.change(path, target)?, target)
            }
        };
        drop(model);
        // Ensure the output itself remains discoverable within the same graph limits.
        let after = Model::read(&output, 0)?;
        let selected = originals.iter().position(|v| v.path == path).unwrap();
        if after.images().len() != originals.len() - usize::from(target.is_none()) {
            return Err(bad("Image edit changed other image occurrences."));
        }
        let mut n = 0;
        let mut changed_frame = None;
        for (i, old) in originals.iter().enumerate() {
            if i == selected && target.is_none() {
                continue;
            }
            let new = &after.images()[n];
            n += 1;
            if i == selected {
                let target = target.unwrap();
                if new
                    .matrix
                    .iter()
                    .zip(target)
                    .any(|(a, b)| (a - b).abs() > 1e-5 * b.abs().max(1.))
                {
                    return Err(bad("Image edit could not preserve the requested frame."));
                }
                if new.crop != expected_crop && !(restoring && new.crop.is_none()) {
                    return Err(bad("Image edit could not preserve the requested crop."));
                }
                if let Some(proof) = image_proof {
                    if after.image_fingerprint(&new.path)? != proof {
                        return Err(bad("Cropping could not preserve the image pixels and color resources."));
                    }
                }
                changed_frame = Some(new.clone());
            } else if old.matrix != new.matrix || old.width != new.width || old.height != new.height || old.crop != new.crop
            {
                return Err(bad("Image edit changed an unselected image."));
            }
        }
        drop(after);
        let candidate = self.register_image_candidate(projection, output)?;
        if let Err(error) =
            self.verify_image_replacement(projection, &candidate.id, &image, changed_frame.as_ref())
        {
            self.discard_unpublished_source(&candidate.id);
            return Err(error);
        }
        Ok(candidate)
    }
    fn verify_image_replacement(
        &mut self,
        before_id: &str,
        after_id: &str,
        image: &Occurrence,
        changed: Option<&Occurrence>,
    ) -> EngineResult<()> {
        let original = self.editing_document(before_id)?.pdf()?.pages().get(0)?;
        let candidate = self.editing_document(after_id)?.pdf()?.pages().get(0)?;
        let a = page_geometry(&original)?;
        let b = page_geometry(&candidate)?;
        if [a.left, a.bottom, a.right, a.top, a.rotation as f32]
            != [b.left, b.bottom, b.right, b.top, b.rotation as f32]
            || text_edit::glyph_snapshot(&original)? != text_edit::glyph_snapshot(&candidate)?
        {
            return Err(bad(
                "Replacing the image changed page geometry or text. The original has been kept.",
            ));
        }
        let before = text_edit::raster(&original)?;
        let after = text_edit::raster(&candidate)?;
        if before.dimensions() != after.dimensions() {
            return Err(bad("Image preview changed page dimensions."));
        }
        let size = a.displayed_size();
        let sx = before.width() as f64 / size.width as f64;
        let sy = before.height() as f64 / size.height as f64;
        let frame = |image: &Occurrence| {
            image.corners.map(|p| {
                let p = a.pdf_to_displayed(p[0] as f32, p[1] as f32);
                [p.x as f64 * sx, p.y as f64 * sy]
            })
        };
        let quad = frame(image);
        let changed_quad = changed.map(frame);
        let crop_changed = changed.is_some_and(|new| new.crop != image.crop);
        let mut needs_ink_proof = crop_changed;
        for (x, y, pixel) in before.enumerate_pixels() {
            if pixel != after.get_pixel(x, y)
                && !within_quad([x as f64 + 0.5, y as f64 + 0.5], quad, 2.)
                && !changed_quad
                    .is_some_and(|quad| within_quad([x as f64 + 0.5, y as f64 + 0.5], quad, 2.))
            {
                needs_ink_proof = true;
                break;
            }
        }
        if let Some(new) = changed.filter(|new| crop_changed && new.matrix == image.matrix) {
            let crop_quad = |value: &Occurrence| {
                value.crop.unwrap_or(crate::ImageCrop::FULL).corners(value.matrix).map(|p| {
                    let p = a.pdf_to_displayed(p[0] as f32,p[1] as f32);
                    [p.x as f64*sx,p.y as f64*sy]
                })
            };
            // A zero count is inapplicable (tiny/disjoint/clipped intersection),
            // not a pixel-preservation proof. Adjust separately requires exact
            // selected image/mask/resource graph equality in every case.
            let _comparable_pixels = verify_crop_interior(&before,&after,crop_quad(image),crop_quad(new))?;
        }
        drop(original);
        drop(candidate);
        if needs_ink_proof {
            self.verify_image_removed_context(before_id, after_id, image, changed)?;
        }
        Ok(())
    }
    fn verify_image_removed_context(
        &mut self,
        before: &str,
        after: &str,
        original: &Occurrence,
        changed: Option<&Occurrence>,
    ) -> EngineResult<()> {
        // PDFium can paint a reflected/sheared image just beyond the geometric
        // two-pixel envelope. Do not expand that envelope indiscriminately.
        // Removing the exact selected invocation from both versions gives two
        // independently rewritten witnesses of ALL the remaining page content.
        // They must match exactly (complete glyphs, page geometry and raster).
        let without = |rt: &Self, source: &str, image: &Occurrence| -> EngineResult<Vec<u8>> {
            let mut pdf = crate::pdf_output::load(rt.editing_document(source)?.bytes()?.as_ref())?;
            let page = *pdf
                .get_pages()
                .values()
                .next()
                .ok_or_else(|| bad("Image proof page is missing."))?;
            page_content_replace::retain_appearance_contexts(&mut pdf, page)?;
            let bytes = crate::pdf_output::save(&mut pdf)?;
            drop(pdf);
            Model::read(&bytes, 0)?.change(&image.path, None)
        };
        let bytes = without(self, before, original)?;
        let baseline = self.register_image_candidate(before, bytes)?;
        let result = (|| {
            if let Some(changed) = changed {
                let bytes = without(self, after, changed)?;
                let candidate = self.register_image_candidate(after, bytes)?;
                let checked = self.verify_text_publication(&candidate.id, 0, &baseline.id);
                self.discard_unpublished_source(&candidate.id);
                checked
            } else {
                self.verify_text_publication(after, 0, &baseline.id)
            }
        })();
        self.discard_unpublished_source(&baseline.id);
        result.map_err(|_|bad("The image edit could not preserve surrounding page content. The original has been kept."))
    }
}
fn verify_crop_interior(before: &image::RgbaImage, after: &image::RgbaImage, old: [[f64;2];4], new: [[f64;2];4]) -> EngineResult<usize> {
    let mut samples = 0;
    for (x,y,pixel) in before.enumerate_pixels() {
        let p = [x as f64+0.5,y as f64+0.5];
        if inside_quad(p,old,2.) && inside_quad(p,new,2.) {
            samples += 1;
            if pixel != after.get_pixel(x,y) {
                return Err(bad("Cropping changed retained image colors. The original has been kept."));
            }
        }
    }
    Ok(samples)
}
fn inside_quad(p: [f64;2], quad: [[f64;2];4], inset: f64) -> bool {
    let mut sign = 0.;
    for i in 0..4 {
        let a = quad[i]; let b = quad[(i+1)%4];
        let dx = b[0]-a[0]; let dy=b[1]-a[1];
        let cross = dx*(p[1]-a[1])-dy*(p[0]-a[0]);
        if cross.abs() <= inset*dx.hypot(dy) || (sign != 0. && cross.signum() != sign) {return false;}
        sign = cross.signum();
    }
    true
}
fn within_quad(p: [f64; 2], quad: [[f64; 2]; 4], margin: f64) -> bool {
    let mut positive = false;
    let mut negative = false;
    let mut near = false;
    for i in 0..4 {
        let a = quad[i];
        let b = quad[(i + 1) % 4];
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let cross = dx * (p[1] - a[1]) - dy * (p[0] - a[0]);
        positive |= cross > 0.;
        negative |= cross < 0.;
        let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy)).clamp(0., 1.);
        near |= (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy) <= margin;
    }
    !(positive && negative) || near
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Document, Object, Stream};
    #[test]
    fn crop_interior_comparison_checks_all_channels_and_reports_empty_intersections() {
        let q = [[0.,0.],[10.,0.],[10.,10.],[0.,10.]];
        let before = image::RgbaImage::from_pixel(10,10,image::Rgba([255,0,0,255]));
        assert_eq!(verify_crop_interior(&before,&before,q,q).unwrap(),36);
        let mut after = before.clone();
        after.get_pixel_mut(4,4).0[0] = 254;
        assert!(verify_crop_interior(&before,&after,q,q).is_err());
        let narrow = [[0.,0.],[1.,0.],[1.,10.],[0.,10.]];
        assert_eq!(verify_crop_interior(&before,&after,q,narrow).unwrap(),0);
    }
    #[test]
    fn image_ink_witness_rejects_unrelated_paint_and_releases_every_private_source() {
        let mut pdf = Document::with_version("1.7");
        let pages = pdf.new_object_id();
        let image=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>2,"Height"=>1,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8},vec![255,0,0,0,255,0]));
        let content = pdf.add_object(Stream::new(
            dictionary! {},
            b"q -100 15 25 50 160 30 cm /I Do Q".to_vec(),
        ));
        let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),200.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"I"=>image}},"Contents"=>content});
        pdf.objects.insert(
            pages,
            dictionary! {"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}.into(),
        );
        let catalog = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
        pdf.trailer.set("Root", catalog);
        let bytes = crate::pdf_output::save(&mut pdf).unwrap();
        let mut model = Model::read(&bytes, 0).unwrap();
        let selected = model.images()[0].clone();
        let target = crate::ImageTransform {
            delta_x: 1.,
            delta_y: -0.5,
            scale_x: 0.99,
            scale_y: 1.01,
            rotation_degrees: 2.,
        }
        .matrix(selected.matrix, 0)
        .unwrap();
        let changed = model.change(&selected.path, Some(target)).unwrap();
        let changed_image = Model::read(&changed, 0).unwrap().images()[0].clone();
        let mut rt = WorkerRuntime::new(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
            Arc::new(FontRegistry::new()),
        )
        .unwrap();
        let before = rt.register_generated_document(bytes, "before.pdf").unwrap();
        let after = rt
            .register_generated_document(changed.clone(), "after.pdf")
            .unwrap();
        let count = rt.documents.len();
        rt.verify_image_removed_context(&before.id, &after.id, &selected, Some(&changed_image))
            .unwrap();
        assert_eq!(rt.documents.len(), count);
        // A crop must always prove the image-free context, even when an
        // unrelated paint change sits entirely inside the selected full frame.
        let cropped = model.adjust(&selected.path,selected.matrix,crate::ImageCrop {left:0.25,right:0.75,..crate::ImageCrop::FULL}).unwrap();
        let mut pdf = crate::pdf_output::load(&cropped).unwrap();
        let page = *pdf.get_pages().values().next().unwrap();
        let old = pdf.get_dictionary(page).unwrap().get(b"Contents").unwrap().clone();
        let paint = pdf.add_object(Stream::new(dictionary!{}, b"q 0 0 1 rg 100 60 10 10 re f Q".to_vec()));
        pdf.get_dictionary_mut(page).unwrap().set("Contents",vec![Object::Reference(paint),old]);
        let bad_crop = rt.register_generated_document(crate::pdf_output::save(&mut pdf).unwrap(),"bad-crop.pdf").unwrap();
        let bad_image = Model::read(rt.editing_document(&bad_crop.id).unwrap().bytes().unwrap().as_ref(),0).unwrap().images()[0].clone();
        let count = rt.documents.len();
        assert!(rt.verify_image_replacement(&before.id,&bad_crop.id,&selected,Some(&bad_image)).is_err());
        assert_eq!(rt.documents.len(),count);
        rt.discard_unpublished_source(&bad_crop.id);
        // Even altered paint hidden underneath the selected picture must fail
        // the witness; visible paint far from it must fail the full public guard.
        for (x, y) in [(350, 150), (100, 60)] {
            let mut bad_pdf = crate::pdf_output::load(&changed).unwrap();
            let page = *bad_pdf.get_pages().values().next().unwrap();
            let old = bad_pdf
                .get_dictionary(page)
                .unwrap()
                .get(b"Contents")
                .unwrap()
                .clone();
            let paint = bad_pdf.add_object(Stream::new(
                dictionary! {},
                format!("q 0 0 1 rg {x} {y} 10 10 re f Q").into_bytes(),
            ));
            bad_pdf
                .get_dictionary_mut(page)
                .unwrap()
                .set("Contents", vec![Object::Reference(paint), old]);
            let bad = rt
                .register_generated_document(
                    crate::pdf_output::save(&mut bad_pdf).unwrap(),
                    "bad.pdf",
                )
                .unwrap();
            let baseline_count = rt.documents.len();
            let bad_image = Model::read(rt.editing_document(&bad.id).unwrap().bytes().unwrap().as_ref(), 0)
                .unwrap()
                .images()[0]
                .clone();
            assert!(rt
                .verify_image_removed_context(&before.id, &bad.id, &selected, Some(&bad_image))
                .is_err());
            assert_eq!(rt.documents.len(), baseline_count);
            if x == 350 {
                assert!(rt
                    .verify_image_replacement(&before.id, &bad.id, &selected, Some(&bad_image))
                    .is_err());
                assert_eq!(rt.documents.len(), baseline_count);
            }
            let mut stale = bad_image.clone();
            stale.path = vec![100_000];
            assert!(rt
                .verify_image_removed_context(&before.id, &bad.id, &selected, Some(&stale))
                .is_err());
            assert_eq!(rt.documents.len(), baseline_count);
            rt.discard_unpublished_source(&bad.id);
        }
        let removed = model.change(&selected.path, None).unwrap();
        let removed = rt
            .register_generated_document(removed, "removed.pdf")
            .unwrap();
        let count = rt.documents.len();
        rt.verify_image_removed_context(&before.id, &removed.id, &selected, None)
            .unwrap();
        assert_eq!(rt.documents.len(), count);
    }
}
