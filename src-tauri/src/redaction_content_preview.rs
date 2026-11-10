//! Combined content candidates share the existing immutable session lifecycle.
use super::*;
use crate::redaction_area_content::RemovedPath;
use crate::redaction_area_images::RemovedImage;
use crate::redaction_area_paths::AreaRect;
use std::collections::BTreeMap;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContentRedactionPage {
    pub page_index: usize,
    pub width: f64,
    pub height: f64,
    pub removed_characters: usize,
    pub removed_paths: usize,
    pub affected_images: usize,
    pub removed_image_pixels: usize,
    pub extends_beyond_marks: bool,
    pub extent: Option<TextMarkRect>,
}
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContentRedactionPreview {
    pub preview_id: String,
    pub source_id: String,
    pub candidate: DocumentInfo,
    pub removed_characters: usize,
    pub removed_paths: usize,
    pub affected_images: usize,
    pub omitted_metadata: Vec<&'static str>,
    pub expires_in_seconds: u64,
    pub pages: Vec<ContentRedactionPage>,
}
pub(super) struct ContentPending {
    pub pages: Vec<ContentRedactionPage>,
    pub paths: Vec<RemovedPath>,
    pub images: Vec<RemovedImage>,
    pub reviewed_pages: Vec<bool>,
    geometry_bytes: usize,
}
pub(super) fn page_rect(
    rect: AreaRect,
    width: f64,
    height: f64,
) -> EngineResult<Option<TextMarkRect>> {
    require(
        [rect.left, rect.bottom, rect.right, rect.top, width, height]
            .into_iter()
            .all(f64::is_finite),
    )?;
    let left = rect.left.max(0.);
    let right = rect.right.min(width);
    let bottom = rect.bottom.max(0.);
    let top = rect.top.min(height);
    Ok((right > left && top > bottom).then_some(TextMarkRect {
        x: left,
        y: height - top,
        width: right - left,
        height: top - bottom,
    }))
}
fn storage<T>(values: &Vec<T>) -> EngineResult<usize> {
    values
        .capacity()
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(invalid)
}
fn add(sum: usize, value: usize) -> EngineResult<usize> {
    sum.checked_add(value).ok_or_else(invalid)
}

impl WorkerRuntime {
    pub(super) fn retained_redaction_geometry(&self) -> EngineResult<usize> {
        self.redaction_previews
            .pending
            .values()
            .try_fold(0, |used, p| {
                let glyphs = p.pages.iter().try_fold(0, |sum, (_, boxes)| {
                    add(sum, boxes.len().checked_mul(32).ok_or_else(invalid)?)
                })?;
                add(
                    add(used, glyphs)?,
                    p.content.as_ref().map_or(0, |p| p.geometry_bytes),
                )
            })
    }
    pub(super) fn preview_content_redaction(
        &mut self,
        owner: &str,
        source: &str,
        marks: &[TextRedactionMark],
    ) -> EngineResult<ContentRedactionPreview> {
        checked_id(owner)?;
        checked_id(source)?;
        checked_marks(marks)?;
        self.prune_redaction_previews();
        require(self.redaction_previews.owners.contains(owner))?;
        require(
            self.redaction_previews.pending.len() < 4
                && !self
                    .redaction_previews
                    .pending
                    .values()
                    .any(|p| p.candidate.id == source),
        )?;
        // Unlike the legacy text API, combined removal is only authorized on
        // the session's immutable Save-composed working copy.
        require(self.redaction_previews.prepared.contains_key(source))?;
        self.validate_prepared_redaction_source(owner, source)?;
        let original = self.redaction_source(source)?;
        let allocation = original.bytes()?;
        let source_hash: [u8; 32] = Sha256::digest(&allocation).into();
        let original_path = original.original_path.clone();
        let mut dimensions = BTreeMap::new();
        for mark in marks {
            if !dimensions.contains_key(&mark.page_index) {
                let page = original
                    .pdf()?
                    .pages()
                    .get(page_index_i32(mark.page_index)?)?;
                dimensions.insert(
                    mark.page_index,
                    (
                        f64::from(page.width().value),
                        f64::from(page.height().value),
                    ),
                );
            }
        }
        let mut context = crate::redaction_images::ImageContext::for_content_preview();
        let result = self.redact_content_areas_with_context(source, marks, &mut context)?;
        require(result.source_id == source && result.source_hash == source_hash)?;
        // The kernel's source/variant pool is temporary; pending previews retain
        // only the published PDF plus source-bound geometric descriptors.
        drop(context);
        let mut glyphs: BTreeMap<usize, Vec<TextMarkRect>> = BTreeMap::new();
        for glyph in result.glyphs {
            glyphs
                .entry(glyph.page_index)
                .or_default()
                .push(glyph.bounds);
        }
        let output = result.output;
        let mut summaries = vec![];
        let mut pages = vec![];
        for (page_index, (width, height)) in dimensions {
            let boxes = glyphs.remove(&page_index).unwrap_or_default();
            let paths: Vec<_> = output
                .paths
                .iter()
                .filter(|p| p.page_index == page_index)
                .collect();
            let images: Vec<_> = output
                .images
                .iter()
                .filter(|p| p.page_index == page_index)
                .collect();
            if boxes.is_empty() && paths.is_empty() && images.is_empty() {
                continue;
            }
            let mut bounds = boxes.clone();
            for rect in paths
                .iter()
                .map(|p| p.bounds)
                .chain(images.iter().map(|i| i.bounds))
            {
                if let Some(rect) = page_rect(rect, width, height)? {
                    bounds.push(rect);
                }
            }
            let extends = paths.iter().any(|p| p.extends_beyond_mark)
                || images.iter().any(|p| p.extends_beyond_mark)
                || boxes.iter().any(|b| {
                    !marks
                        .iter()
                        .filter(|m| m.page_index == page_index)
                        .any(|m| {
                            m.rect.x <= b.x
                                && m.rect.y <= b.y
                                && m.rect.x + m.rect.width >= b.x + b.width
                                && m.rect.y + m.rect.height >= b.y + b.height
                        })
                });
            summaries.push(ContentRedactionPage {
                page_index,
                width,
                height,
                removed_characters: boxes.len(),
                removed_paths: paths.len(),
                affected_images: images.len(),
                removed_image_pixels: images
                    .iter()
                    .try_fold(0, |sum, image| add(sum, image.pixels))?,
                extends_beyond_marks: extends,
                extent: if bounds.is_empty() {
                    None
                } else {
                    Some(extent(&bounds)?)
                },
            });
            if !boxes.is_empty() {
                pages.push((
                    TextRedactionPage {
                        page_index,
                        width,
                        height,
                        selected_glyph_count: boxes.len(),
                        extent: extent(&boxes)?,
                    },
                    boxes,
                ));
            }
        }
        require(glyphs.is_empty() && !summaries.is_empty())?;
        let reviewed_pages = vec![false; summaries.len()];
        let mut geometry_bytes = std::mem::size_of::<ContentPending>();
        for bytes in [
            storage(&summaries)?,
            storage(&output.paths)?,
            storage(&output.images)?,
            storage(&reviewed_pages)?,
            storage(&pages)?,
        ] {
            geometry_bytes = add(geometry_bytes, bytes)?;
        }
        for (_, boxes) in &pages {
            geometry_bytes = add(
                geometry_bytes,
                (boxes.capacity() - boxes.len())
                    .checked_mul(32)
                    .ok_or_else(invalid)?,
            )?;
        }
        for image in &output.images {
            if let Some(jpeg) = &image.jpeg {
                geometry_bytes = add(geometry_bytes, jpeg.retained_geometry_heap_bytes()?)?;
            }
        }
        let new_glyph_bytes = output
            .removed_characters
            .checked_mul(32)
            .ok_or_else(invalid)?;
        require(
            add(
                add(self.retained_redaction_geometry()?, geometry_bytes)?,
                new_glyph_bytes,
            )? <= self.redaction_previews.geometry_limit,
        )?;
        require(
            add(self.retained_redaction_bytes(), output.bytes.len())?
                <= self.redaction_previews.byte_limit,
        )?;
        let current = self.redaction_source(source)?;
        require(
            Arc::ptr_eq(&allocation, &current.bytes()?)
                && <[u8; 32]>::from(Sha256::digest(current.bytes()?.as_ref())) == source_hash,
        )?;
        let mut omitted_metadata = output.omitted_metadata;
        if let Some(prepared) = self.redaction_previews.prepared.get(source) {
            for item in &prepared.omitted_metadata {
                if !omitted_metadata.contains(item) {
                    omitted_metadata.push(item);
                }
            }
        }
        let candidate_hash = Sha256::digest(&output.bytes).into();
        let candidate =
            self.register_generated_document(output.bytes, "Content removal preview.pdf")?;
        let document = self
            .documents
            .get_mut(&candidate.id)
            .expect("new content candidate");
        document.original_path = original_path;
        let allocation = document.source_bytes.identity().clone();
        let preview_id = Uuid::new_v4().to_string();
        let info = ContentRedactionPreview {
            preview_id: preview_id.clone(),
            source_id: source.into(),
            candidate: candidate.clone(),
            removed_characters: output.removed_characters,
            removed_paths: output.paths.len(),
            affected_images: output.images.len(),
            omitted_metadata,
            expires_in_seconds: TTL.as_secs(),
            pages: summaries.clone(),
        };
        self.redaction_previews.pending.insert(
            preview_id,
            Pending {
                owner: owner.into(),
                source: source.into(),
                source_hash,
                candidate,
                candidate_hash,
                allocation,
                created: Instant::now(),
                pages,
                content: Some(ContentPending {
                    pages: summaries,
                    paths: output.paths,
                    images: output.images,
                    reviewed_pages,
                    geometry_bytes,
                }),
            },
        );
        Ok(info)
    }
}
