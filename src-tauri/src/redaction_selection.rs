//! Private source-owned native text marks; not all-content area redaction.
use super::*;
use crate::redaction_content::{invalid, require};
use crate::redaction_ranges::{
    inspect_standard_glyphs, remove_standard_text_ranges, RedactedTextRangesPdf, TextRange,
};
use sha2::{Digest, Sha256};

const MAX_BYTES: usize = 64 * 1024 * 1024;
const MAX_GLYPHS: usize = 250_000;
const CELL: f64 = 0.004;
const TOLERANCE: f64 = 0.002;

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TextMarkRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Debug)]
pub(super) struct MeasuredGlyph {
    pub block_index: usize,
    pub offset: usize,
    pub unicode: u32,
    pub bounds: TextMarkRect,
}
pub(super) struct PageTextSelection {
    source_id: String,
    source_hash: [u8; 32],
    page_index: usize,
    width: f64,
    height: f64,
    blocks: HashMap<usize, SelectionBlock>,
    glyphs: Vec<MeasuredGlyph>,
    complete_area: bool,
}
pub(super) struct AreaTextRanges {
    pub ranges: Vec<TextRange>,
    pub bounds: Vec<TextMarkRect>,
    pub page_height: f64,
}
struct SelectionBlock {
    id: String,
    text: Vec<char>,
}
pub(super) struct TextRedactionPlan {
    source_id: String,
    source_hash: [u8; 32],
    page_index: usize,
    width: f64,
    height: f64,
    ranges: Vec<TextRange>,
    bounds: Vec<TextMarkRect>,
}
impl TextMarkRect {
    pub(super) fn valid(&self) -> bool {
        [
            self.x,
            self.y,
            self.width,
            self.height,
            self.x + self.width,
            self.y + self.height,
        ]
        .iter()
        .all(|x| x.is_finite() && x.abs() <= 1_000_000.)
            && self.width > 0.
            && self.height > 0.
    }
    fn intersects(&self, other: &Self) -> bool {
        self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }
}
impl PageTextSelection {
    pub(super) fn area_ranges(
        &self,
        source_id: &str,
        source_hash: [u8; 32],
        rects: &[TextMarkRect],
    ) -> EngineResult<AreaTextRanges> {
        require(
            self.complete_area
                && self.source_id == source_id
                && self.source_hash == source_hash
                && (1..=64).contains(&rects.len()),
        )?;
        for rect in rects {
            require(
                rect.valid()
                    && rect.x >= 0.
                    && rect.y >= 0.
                    && rect.x + rect.width > rect.x
                    && rect.y + rect.height > rect.y
                    && rect.x + rect.width <= self.width
                    && rect.y + rect.height <= self.height,
            )?;
        }
        let mut ranges: Vec<TextRange> = vec![];
        let mut bounds = vec![];
        let mut previous_block = None;
        for glyph in &self.glyphs {
            let selected = rects.iter().any(|r| {
                r.x <= glyph.bounds.x + glyph.bounds.width
                    && glyph.bounds.x <= r.x + r.width
                    && r.y <= glyph.bounds.y + glyph.bounds.height
                    && glyph.bounds.y <= r.y + r.height
            });
            if !selected {
                continue;
            }
            let block = self.blocks.get(&glyph.block_index).ok_or_else(invalid)?;
            if previous_block == Some(glyph.block_index)
                && ranges.last().is_some_and(|r| r.end == glyph.offset)
            {
                ranges.last_mut().unwrap().end = glyph.offset + 1;
            } else {
                require(ranges.len() < 4096)?;
                ranges.push(TextRange {
                    selection_id: block.id.clone(),
                    start: glyph.offset,
                    end: glyph.offset + 1,
                });
            }
            previous_block = Some(glyph.block_index);
            bounds.push(glyph.bounds);
        }
        // A valid mark can target only drawings; publication decides whether
        // the entire request actually removes admitted content.
        Ok(AreaTextRanges {
            ranges,
            bounds,
            page_height: self.height,
        })
    }
    pub(super) fn glyphs(&self) -> &[MeasuredGlyph] {
        &self.glyphs
    }
    pub(super) fn mark(&self, rect: TextMarkRect) -> EngineResult<TextRedactionPlan> {
        self.mark_many(&[rect])
    }
    pub(super) fn mark_many(&self, rects: &[TextMarkRect]) -> EngineResult<TextRedactionPlan> {
        require((1..=64).contains(&rects.len()))?;
        for rect in rects {
            require(
                rect.valid()
                    && rect.x >= 0.
                    && rect.y >= 0.
                    && rect.x + rect.width <= self.width
                    && rect.y + rect.height <= self.height,
            )?;
        }
        let mut hit = vec![false; rects.len()];
        let mut ranges: Vec<TextRange> = vec![];
        let mut bounds = vec![];
        let mut previous_block = None;
        for glyph in &self.glyphs {
            let mut selected = false;
            for (rect, hit) in rects.iter().zip(&mut hit) {
                if rect.intersects(&glyph.bounds) {
                    *hit = true;
                    selected = true;
                }
            }
            if !selected {
                continue;
            }
            let block = self.blocks.get(&glyph.block_index).ok_or_else(invalid)?;
            let merge = previous_block == Some(glyph.block_index)
                && ranges.last().is_some_and(|r| {
                    r.end <= glyph.offset
                        && block.text[r.end..glyph.offset].iter().all(|b| *b == ' ')
                });
            if merge {
                ranges.last_mut().unwrap().end = glyph.offset + 1;
            } else {
                require(ranges.len() < 4096)?;
                ranges.push(TextRange {
                    selection_id: block.id.clone(),
                    start: glyph.offset,
                    end: glyph.offset + 1,
                });
            }
            previous_block = Some(glyph.block_index);
            bounds.push(glyph.bounds);
        }
        require(!ranges.is_empty() && hit.iter().all(|h| *h))?;
        Ok(TextRedactionPlan {
            source_id: self.source_id.clone(),
            source_hash: self.source_hash,
            page_index: self.page_index,
            width: self.width,
            height: self.height,
            ranges,
            bounds,
        })
    }
}
impl TextRedactionPlan {
    pub(super) fn into_affected_glyphs(self) -> Vec<TextMarkRect> {
        self.bounds
    }
    pub(super) fn page_geometry(&self) -> (usize, f64, f64) {
        (self.page_index, self.width, self.height)
    }
    pub(super) fn range_count(&self) -> usize {
        self.ranges.len()
    }
    pub(super) fn affected_glyphs(&self) -> &[TextMarkRect] {
        &self.bounds
    }
}
#[derive(Clone)]
struct ExpectedGlyph {
    block: usize,
    offset: usize,
    unicode: u32,
    origin: [f64; 2],
}
struct NativeGlyph {
    unicode: u32,
    origin: [f64; 2],
    bounds: TextMarkRect,
}
fn cell(origin: [f64; 2]) -> EngineResult<(i64, i64)> {
    require(
        origin
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 1_000_000.),
    )?;
    Ok((
        (origin[0] / CELL).floor() as i64,
        (origin[1] / CELL).floor() as i64,
    ))
}
fn bind_glyphs(
    expected: Vec<ExpectedGlyph>,
    native: Vec<NativeGlyph>,
) -> EngineResult<Vec<MeasuredGlyph>> {
    require(expected.len() <= MAX_GLYPHS && native.len() == expected.len())?;
    let mut cells: HashMap<(u32, i64, i64), Vec<usize>> = HashMap::new();
    for (i, g) in expected.iter().enumerate() {
        let (x, y) = cell(g.origin)?;
        let bucket = cells.entry((g.unicode, x, y)).or_default();
        require(bucket.len() < 32)?;
        bucket.push(i);
    }
    let mut bounds = vec![None; expected.len()];
    for g in native {
        require(g.bounds.valid())?;
        let (x, y) = cell(g.origin)?;
        let mut found = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(bucket) = cells.get(&(g.unicode, x + dx, y + dy)) {
                    for &i in bucket {
                        let other = &expected[i];
                        if other
                            .origin
                            .iter()
                            .zip(g.origin)
                            .all(|(a, b)| (a - b).abs() <= TOLERANCE)
                        {
                            // Test all candidates, even already consumed ones: order
                            // cannot authorize an otherwise ambiguous association.
                            require(found.is_none())?;
                            found = Some(i);
                        }
                    }
                }
            }
        }
        let i = found.ok_or_else(invalid)?;
        require(bounds[i].is_none())?;
        bounds[i] = Some(g.bounds);
    }
    expected
        .into_iter()
        .zip(bounds)
        .map(|(g, b)| {
            Ok(MeasuredGlyph {
                block_index: g.block,
                offset: g.offset,
                unicode: g.unicode,
                bounds: b.ok_or_else(invalid)?,
            })
        })
        .collect()
}
fn parse(bytes: &[u8]) -> EngineResult<lopdf::Document> {
    require(bytes.len() <= MAX_BYTES)?;
    crate::redaction_numbers::load_pdf(bytes)
}
#[path = "redaction_space_probe.rs"]
mod space_probe;

fn native_glyphs(
    page: &PdfPage,
    height: f64,
    include_spaces: bool,
    mapped_spaces: bool,
) -> EngineResult<Vec<NativeGlyph>> {
    let text = page.text()?;
    let chars = text.chars();
    require(chars.len() as usize <= 500_000)?;
    let mut native = vec![];
    for c in chars.iter() {
        if c.is_generated()? {
            continue;
        }
        let unicode = match c.unicode_value() {
            0xE000 if mapped_spaces => 32,
            value => value,
        };
        if unicode == 32 && !include_spaces {
            continue;
        }
        require(native.len() < MAX_GLYPHS)?;
        let (x, y) = c.origin()?;
        let b = if unicode == 32 {
            c.loose_bounds()?
        } else {
            c.tight_bounds()?
        };
        native.push(NativeGlyph {
            unicode,
            origin: [x.value as f64, y.value as f64],
            bounds: TextMarkRect {
                x: b.left().value as f64,
                y: height - b.top().value as f64,
                width: b.right().value as f64 - b.left().value as f64,
                height: b.top().value as f64 - b.bottom().value as f64,
            },
        });
    }
    Ok(native)
}
impl WorkerRuntime {
    pub(super) fn redaction_source(&self, source_id: &str) -> EngineResult<&OpenDocument> {
        let source = self.editing_document(source_id)?;
        self.copying_document(source_id)?;
        if source.policy.is_some() {
            return Err(EngineError::InvalidRequest(
                "Protected PDF text redaction is not supported yet.".into(),
            ));
        }
        require(source.source_bytes.identity().len() <= MAX_BYTES)?;
        Ok(source)
    }
    pub(super) fn inspect_redaction_text_page(
        &self,
        source_id: &str,
        page_index: usize,
    ) -> EngineResult<PageTextSelection> {
        self.inspect_redaction_page(source_id, page_index, false, &mut Default::default())
    }
    pub(super) fn inspect_redaction_area_page(
        &self,
        source_id: &str,
        page_index: usize,
    ) -> EngineResult<PageTextSelection> {
        self.inspect_redaction_area_page_with_context(
            source_id,
            page_index,
            &mut Default::default(),
        )
    }
    pub(super) fn inspect_redaction_area_page_with_context(
        &self,
        source_id: &str,
        page_index: usize,
        images: &mut crate::redaction_images::ImageContext,
    ) -> EngineResult<PageTextSelection> {
        self.inspect_redaction_page(source_id, page_index, true, images)
    }
    fn inspect_redaction_page(
        &self,
        source_id: &str,
        page_index: usize,
        complete_area: bool,
        images: &mut crate::redaction_images::ImageContext,
    ) -> EngineResult<PageTextSelection> {
        let source = self.redaction_source(source_id)?;
        let parsed = parse(source.bytes()?.as_ref())?;
        let inspected = if complete_area {
            crate::redaction_ranges::inspect_area_glyphs_with_context(&parsed, images)?
        } else {
            inspect_standard_glyphs(&parsed)?
        };
        let count = parsed.get_pages().len();
        require(page_index < count)?;
        let source_hash = Sha256::digest(source.bytes()?.as_ref()).into();
        let mut expected = vec![];
        let mut blocks = HashMap::new();
        let mut source_count = 0usize;
        for block in inspected
            .into_iter()
            .filter(|b| b.source.page_index == page_index)
        {
            source_count = source_count
                .checked_add(block.glyphs.len())
                .ok_or_else(invalid)?;
            require(source_count <= MAX_GLYPHS)?;
            let index = block.source.block_index;
            for g in block
                .glyphs
                .into_iter()
                .filter(|g| complete_area || g.unicode != 32)
            {
                expected.push(ExpectedGlyph {
                    block: index,
                    offset: g.offset,
                    unicode: g.unicode,
                    origin: g.origin,
                });
            }
            blocks.insert(
                index,
                SelectionBlock {
                    id: block.source.selection_id,
                    text: block.source.text.chars().collect(),
                },
            );
        }
        // Measure the same immutable bytes, not the possibly annotation-adjusted
        // display document. This temporary native document is never registered.
        let document = self
            .pdfium
            .load_pdf_from_reader(Cursor::new(source.bytes()?), None)?;
        require(document.pages().len() as usize == count)?;
        let page = document.pages().get(page_index_i32(page_index)?)?;
        let width = page.width().value as f64;
        let height = page.height().value as f64;
        require(
            width.is_finite()
                && height.is_finite()
                && (1.0..=20_000.).contains(&width)
                && (1.0..=20_000.).contains(&height),
        )?;
        let glyphs = if complete_area && expected.iter().any(|g| g.unicode == 32) {
            // PDFium normalizes consecutive spaces even when each exists in
            // source operands. Bind visible source glyphs against the original,
            // then measure spaces in a temporary standard-font-only copy whose
            // ToUnicode mapping prevents that extraction normalization.
            let original = bind_glyphs(
                expected
                    .iter()
                    .filter(|g| g.unicode != 32)
                    .cloned()
                    .collect(),
                native_glyphs(&page, height, false, false)?,
            )?;
            let probe_bytes = space_probe::build_with_context(&parsed, page_index, images)?;
            let probe = self
                .pdfium
                .load_pdf_from_reader(Cursor::new(probe_bytes), None)?;
            require(probe.pages().len() == 1)?;
            let probe_page = probe.pages().get(0)?;
            require(
                probe_page.width().value as f64 == width
                    && probe_page.height().value as f64 == height,
            )?;
            let measured = bind_glyphs(expected, native_glyphs(&probe_page, height, true, true)?)?;
            let mut retained = measured.iter().filter(|g| g.unicode != 32);
            for before in original {
                let after = retained.next().ok_or_else(invalid)?;
                require(
                    before.block_index == after.block_index
                        && before.offset == after.offset
                        && before.unicode == after.unicode
                        && before.bounds.x == after.bounds.x
                        && before.bounds.y == after.bounds.y
                        && before.bounds.width == after.bounds.width
                        && before.bounds.height == after.bounds.height,
                )?;
            }
            require(retained.next().is_none())?;
            measured
        } else {
            bind_glyphs(
                expected,
                native_glyphs(&page, height, complete_area, false)?,
            )?
        };
        Ok(PageTextSelection {
            source_id: source_id.into(),
            source_hash,
            page_index,
            width,
            height,
            blocks,
            glyphs,
            complete_area,
        })
    }
    pub(super) fn redact_text_mark(
        &self,
        mark: &TextRedactionPlan,
    ) -> EngineResult<RedactedTextRangesPdf> {
        self.redact_text_marks(std::slice::from_ref(mark))
    }
    pub(super) fn redact_text_marks(
        &self,
        marks: &[TextRedactionPlan],
    ) -> EngineResult<RedactedTextRangesPdf> {
        require((1..=64).contains(&marks.len()))?;
        let first = &marks[0];
        let mut pages = HashSet::new();
        let mut count = 0usize;
        for mark in marks {
            require(
                mark.source_id == first.source_id
                    && mark.source_hash == first.source_hash
                    && pages.insert(mark.page_index),
            )?;
            count = count.checked_add(mark.ranges.len()).ok_or_else(invalid)?;
            require(count <= 4096)?;
        }
        let source = self.redaction_source(&first.source_id)?;
        let hash: [u8; 32] = Sha256::digest(source.bytes()?.as_ref()).into();
        require(hash == first.source_hash)?;
        let ranges = marks
            .iter()
            .flat_map(|m| {
                m.ranges.iter().map(|r| TextRange {
                    selection_id: r.selection_id.clone(),
                    start: r.start,
                    end: r.end,
                })
            })
            .collect::<Vec<_>>();
        remove_standard_text_ranges(&parse(source.bytes()?.as_ref())?, &ranges)
    }
}
#[cfg(test)]
#[path = "redaction_selection_tests.rs"]
mod tests;
