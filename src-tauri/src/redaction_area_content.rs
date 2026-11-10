//! Private combined publication; caller must derive complete geometric text selections.
use crate::{
    redaction_area_paths::{plan_paths, AreaRect, PaintKind, PathCommand},
    redaction_content::{invalid, number, require},
    redaction_core,
    redaction_ranges::{
        inspect_area_glyphs_with_context, remove_area_text_ranges_with_context, TextRange,
    },
    EngineResult,
};
use lopdf::{content::Operation, Document, Object};
use std::collections::BTreeSet;

pub(crate) struct PageAreaMark {
    pub page_index: usize,
    pub rect: AreaRect,
}
pub(crate) struct RemovedPath {
    pub page_index: usize,
    pub bounds: AreaRect,
    pub painted: bool,
    pub extends_beyond_mark: bool,
}
pub(crate) struct RemovedContentPdf {
    pub bytes: Vec<u8>,
    pub removed_characters: usize,
    pub paths: Vec<RemovedPath>,
    pub images: Vec<crate::redaction_area_images::RemovedImage>,
    pub omitted_metadata: Vec<&'static str>,
}
pub(crate) fn remove_planned_content(
    doc: &Document,
    ranges: &[TextRange],
    marks: &[PageAreaMark],
) -> EngineResult<RemovedContentPdf> {
    remove_planned_content_with_context(doc, ranges, marks, &mut Default::default())
}
pub(crate) fn remove_planned_content_with_context(
    doc: &Document,
    ranges: &[TextRange],
    marks: &[PageAreaMark],
    image_context: &mut crate::redaction_images::ImageContext,
) -> EngineResult<RemovedContentPdf> {
    require((1..=64).contains(&marks.len()) && ranges.len() <= 4096)?;
    let mut prepared = redaction_core::prepare_area_with_context(doc, image_context)?;
    for mark in marks {
        let page = prepared.pages.get(mark.page_index).ok_or_else(invalid)?;
        // Validate endpoints before page containment, including NaN and zero area.
        plan_paths(&[], &[mark.rect]).map_err(|_| invalid())?;
        require(
            mark.rect.left >= 0.
                && mark.rect.bottom >= 0.
                && mark.rect.right <= number(&page.width)?
                && mark.rect.top <= number(&page.height)?,
        )?;
    }
    let mut omitted: BTreeSet<&'static str> = prepared.omitted.iter().copied().collect();
    let removed_characters;
    let intermediate;
    if ranges.is_empty() {
        removed_characters = 0;
        intermediate = None;
    } else {
        // Do not hold two prepared font/content graphs during text rewriting.
        drop(prepared);
        let text = remove_area_text_ranges_with_context(doc, ranges, image_context)?;
        removed_characters = text.removed_characters;
        omitted.extend(text.omitted_metadata);
        intermediate = Some(parse(&text.bytes)?);
        prepared = redaction_core::prepare_area_with_context(
            intermediate.as_ref().unwrap(),
            image_context,
        )?;
    }
    let before =
        inspect_area_glyphs_with_context(intermediate.as_ref().unwrap_or(doc), image_context)?;
    let mut paths = Vec::new();
    let mut images = Vec::new();
    let mut image_work = crate::redaction_area_images::ImageWork::default();
    for (page_index, page) in prepared.pages.iter_mut().enumerate() {
        let commands = page
            .program
            .operations
            .iter()
            .map(command)
            .collect::<EngineResult<Vec<_>>>()?;
        let selected: Vec<_> = marks
            .iter()
            .filter(|m| m.page_index == page_index)
            .map(|m| m.rect)
            .collect();
        let validation_mark = AreaRect {
            left: 0.,
            bottom: 0.,
            right: number(&page.width)?,
            top: number(&page.height)?,
        };
        let decisions = plan_paths(
            &commands,
            if selected.is_empty() {
                std::slice::from_ref(&validation_mark)
            } else {
                &selected
            },
        )
        .map_err(|_| invalid())?;
        // Even unmarked pages must pass strict content/state admission. Their
        // validation mark has no removal authority.
        let mut keep = vec![true; page.program.operations.len()];
        for decision in decisions
            .into_iter()
            .filter(|d| !selected.is_empty() && d.remove)
        {
            for retained in &mut keep[decision.first..=decision.last] {
                *retained = false;
            }
            paths.push(RemovedPath {
                page_index,
                bounds: decision.bounds,
                painted: decision.painted,
                extends_beyond_mark: decision.extends_beyond_marks,
            });
        }
        page.program.operations = std::mem::take(&mut page.program.operations)
            .into_iter()
            .zip(keep)
            .filter_map(|(op, keep)| keep.then_some(op))
            .collect();
        images.extend(image_work.page(page, page_index, &selected, image_context)?);
    }
    require(removed_characters > 0 || !paths.is_empty() || !images.is_empty())?;
    omitted.extend(prepared.omitted);
    let bytes = redaction_core::publish(prepared.pages)?;
    let reopened = parse(&bytes)?;
    let after = inspect_area_glyphs_with_context(&reopened, image_context)?;
    require(before.len() == after.len())?;
    for (old, new) in before.iter().zip(&after) {
        require(
            old.source.page_index == new.source.page_index
                && old.source.block_index == new.source.block_index
                && old.source.text == new.source.text
                && old.glyphs.len() == new.glyphs.len(),
        )?;
        for (a, b) in old.glyphs.iter().zip(&new.glyphs) {
            // Deleting a path cannot alter text state. Require exact replay here,
            // in addition to the existing range remover's own advance proof.
            require(
                a.code == b.code
                    && a.unicode == b.unicode
                    && a.origin == b.origin
                    && a.end == b.end,
            )?;
        }
    }
    Ok(RemovedContentPdf {
        bytes,
        removed_characters,
        paths,
        images,
        omitted_metadata: omitted.into_iter().collect(),
    })
}

fn parse(bytes: &[u8]) -> EngineResult<Document> {
    require(bytes.len() <= 64 * 1024 * 1024)?;
    crate::redaction_numbers::load_pdf(bytes)
}

// Only called on redaction_core's strictly canonicalized operations. Keep one
// command per operation: its index is the exact operand-range deletion index.
fn command(op: &Operation) -> EngineResult<PathCommand> {
    use PathCommand::*;
    fn values<const N: usize>(args: &[Object]) -> EngineResult<[f64; N]> {
        require(args.len() == N)?;
        let mut out = [0.; N];
        for (n, arg) in out.iter_mut().zip(args) {
            *n = number(arg)?;
        }
        Ok(out)
    }
    let a = &op.operands;
    let scalar = || -> EngineResult<f64> { Ok(values::<1>(a)?[0]) };
    Ok(match op.operator.as_str() {
        "q" => {
            values::<0>(a)?;
            Save
        }
        "Q" => {
            values::<0>(a)?;
            Restore
        }
        "cm" => Concat(values(a)?),
        "w" => LineWidth(scalar()?),
        "M" => MiterLimit(scalar()?),
        "J" | "j" => {
            require(a.len() == 1)?;
            let value = a[0].as_i64().map_err(|_| invalid())?;
            require((0..=2).contains(&value))?;
            if op.operator == "J" {
                LineCap(value as u8)
            } else {
                LineJoin(value as u8)
            }
        }
        "m" => Move(values(a)?),
        "l" => Line(values(a)?),
        "c" => Curve(values(a)?),
        "v" => CurveV(values(a)?),
        "y" => CurveY(values(a)?),
        "re" => Rect(values(a)?),
        "h" => {
            values::<0>(a)?;
            Close
        }
        "n" => {
            values::<0>(a)?;
            End
        }
        "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
            values::<0>(a)?;
            Paint(match op.operator.as_str() {
                "S" => PaintKind::Stroke,
                "s" => PaintKind::CloseStroke,
                "B" | "B*" => PaintKind::FillStroke,
                "b" | "b*" => PaintKind::CloseFillStroke,
                _ => PaintKind::Fill,
            })
        }
        "W" | "W*" => Clip,
        // These are fully validated by canonicalize. Dash changes only remove
        // portions of the conservative solid stroke envelope.
        "BT" | "ET" | "Tf" | "Tm" | "Td" | "TD" | "T*" | "Tc" | "Tw" | "TL" | "Tz" | "Ts"
        | "Tr" | "Tj" | "TJ" | "g" | "G" | "rg" | "RG" | "k" | "K" | "d" | "Do" => Unchanged,
        // In particular gs must not silently discard stroke-adjustment state.
        _ => Unsupported,
    })
}

#[cfg(test)]
#[path = "redaction_area_content_tests.rs"]
mod tests;
