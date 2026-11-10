//! Private source-owned area removal; no window command or candidate adoption.
use super::redaction_selection::TextMarkRect;
use super::*;
use crate::redaction_area_content::RemovedContentPdf;
use crate::redaction_area_content::{remove_planned_content_with_context, PageAreaMark};
use crate::redaction_area_paths::AreaRect;
use crate::redaction_content::{invalid, require};
use sha2::{Digest, Sha256};

pub(super) struct AreaGlyph {
    pub page_index: usize,
    pub bounds: TextMarkRect,
}
pub(super) struct AreaContentResult {
    pub source_id: String,
    pub source_hash: [u8; 32],
    pub output: RemovedContentPdf,
    pub glyphs: Vec<AreaGlyph>,
}
impl WorkerRuntime {
    pub(super) fn redact_content_areas(
        &self,
        source: &str,
        marks: &[TextRedactionMark],
    ) -> EngineResult<AreaContentResult> {
        self.redact_content_areas_with_context(source, marks, &mut Default::default())
    }
    pub(super) fn redact_content_areas_with_context(
        &self,
        source: &str,
        marks: &[TextRedactionMark],
        image_context: &mut crate::redaction_images::ImageContext,
    ) -> EngineResult<AreaContentResult> {
        require(
            (1..=64).contains(&marks.len())
                && marks.iter().all(|m| m.page_index < 1000 && m.rect.valid()),
        )?;
        let allocation = self.redaction_source(source)?.bytes()?;
        let source_hash: [u8; 32] = Sha256::digest(&allocation).into();
        let mut groups = std::collections::BTreeMap::<usize, Vec<TextMarkRect>>::new();
        for mark in marks {
            groups.entry(mark.page_index).or_default().push(mark.rect);
        }
        let mut ranges = vec![];
        let mut glyphs = vec![];
        let mut page_marks = vec![];
        for (page_index, rects) in groups {
            let selected = self
                .inspect_redaction_area_page_with_context(source, page_index, image_context)?
                .area_ranges(source, source_hash, &rects)?;
            require(
                ranges
                    .len()
                    .checked_add(selected.ranges.len())
                    .is_some_and(|n| n <= 4096)
                    && glyphs
                        .len()
                        .checked_add(selected.bounds.len())
                        .is_some_and(|n| n <= 250_000),
            )?;
            ranges.extend(selected.ranges);
            glyphs.extend(
                selected
                    .bounds
                    .into_iter()
                    .map(|bounds| AreaGlyph { page_index, bounds }),
            );
            for rect in rects {
                page_marks.push(PageAreaMark {
                    page_index,
                    rect: AreaRect {
                        left: rect.x,
                        bottom: selected.page_height - (rect.y + rect.height),
                        right: rect.x + rect.width,
                        top: selected.page_height - rect.y,
                    },
                });
            }
        }
        let parsed = crate::redaction_numbers::load_pdf(&allocation)?;
        let output =
            remove_planned_content_with_context(&parsed, &ranges, &page_marks, image_context)?;
        require(output.removed_characters == glyphs.len())?;
        let current = self.redaction_source(source)?;
        require(
            Arc::ptr_eq(&allocation, &current.bytes()?)
                && <[u8; 32]>::from(Sha256::digest(current.bytes()?.as_ref())) == source_hash,
        )?;
        Ok(AreaContentResult {
            source_id: source.into(),
            source_hash,
            output,
            glyphs,
        })
    }
}
#[cfg(test)]
#[path = "redaction_area_image_tests.rs"]
mod image_tests;
#[cfg(test)]
#[path = "redaction_area_jpeg_tests.rs"]
mod jpeg_tests;
#[cfg(test)]
#[path = "redaction_area_jpeg_combined_tests.rs"]
mod jpeg_combined_tests;
#[cfg(test)]
#[path = "redaction_area_jpeg_coverage_tests.rs"]
mod jpeg_coverage_tests;
#[cfg(test)]
#[path = "redaction_area_selection_tests.rs"]
mod tests;
