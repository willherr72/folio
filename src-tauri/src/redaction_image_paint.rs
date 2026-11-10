//! Recreate owned JPEG paint only inside a verified safe page context.
use super::{invalid, require, CoverageBudget, Image};
use crate::EngineResult;
use lopdf::content::Operation;
use std::sync::Arc;

pub(crate) fn strip_owned_paint(
    operations: Vec<Operation>,
    images: &[Arc<Image>],
    budget: &mut CoverageBudget,
) -> EngineResult<Vec<Operation>> {
    budget.charge_verified_operations(operations.len())?;
    if !images.iter().any(|image| image.jpeg_coverage().is_some()) {
        return Ok(operations);
    }
    let mut context = Context::default();
    let mut result = Vec::with_capacity(operations.len());
    let mut source = operations.into_iter();
    while let Some(op) = source.next() {
        context.observe(&op.operator)?;
        if let Some(image) = selected(&op, images)? {
            if image.jpeg_coverage().is_some() {
                context.require_owned()?;
                let count = image.verify_paint_prefix(source.as_slice(), budget)?;
                // Do not strip even one operation until the entire occurrence
                // has matched. Aliases must independently repeat this proof.
                for _ in 0..count {
                    source.next().ok_or_else(invalid)?;
                }
            }
        }
        result.push(op);
    }
    require(context.clips.is_empty() && !context.text)?;
    Ok(result)
}

fn selected<'a>(op: &Operation, images: &'a [Arc<Image>]) -> EngineResult<Option<&'a Image>> {
    if op.operator != "Do" {
        return Ok(None);
    }
    require(op.operands.len() == 1)?;
    let alias = op.operands[0].as_name().map_err(|_| invalid())?;
    let index = std::str::from_utf8(alias)
        .map_err(|_| invalid())?
        .strip_prefix('I')
        .ok_or_else(invalid)?
        .parse::<usize>()
        .map_err(|_| invalid())?;
    Ok(Some(images.get(index).ok_or_else(invalid)?.as_ref()))
}

#[derive(Default)]
struct Context {
    clips: Vec<bool>,
    clipped: bool,
    pending_clip: bool,
    path: bool,
    text: bool,
}
impl Context {
    fn observe(&mut self, operator: &str) -> EngineResult<()> {
        match operator {
            "BT" => {
                require(!self.text)?;
                self.text = true;
            }
            "ET" => {
                require(self.text)?;
                self.text = false;
            }
            "q" => {
                require(self.clips.len() < 64)?;
                self.clips.push(self.clipped);
            }
            "Q" => self.clipped = self.clips.pop().ok_or_else(invalid)?,
            "m" | "l" | "c" | "v" | "y" | "re" | "h" => self.path = true,
            "W" | "W*" => self.pending_clip = true,
            "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n" => {
                self.path = false;
                self.clipped |= self.pending_clip;
                self.pending_clip = false;
            }
            _ => {}
        }
        Ok(())
    }
    fn require_owned(&self) -> EngineResult<()> {
        // q/Q do not save a pending path or pending clipping operator.
        // The inserted fill must not consume an unrelated path or be clipped.
        require(
            self.clips.len() < 64
                && !self.path
                && !self.pending_clip
                && !self.clipped
                && !self.text,
        )
    }
}

pub(crate) fn publish_operations(
    operations: Vec<Operation>,
    images: &[Arc<Image>],
    budget: &mut CoverageBudget,
) -> EngineResult<Vec<Operation>> {
    budget.charge_source_operations(operations.len())?;
    if !images.iter().any(|image| image.jpeg_coverage().is_some()) {
        return Ok(operations);
    }
    let mut context = Context::default();
    let mut result = Vec::with_capacity(operations.len());
    for op in operations {
        context.observe(&op.operator)?;
        let paint = if let Some(image) = selected(&op, images)? {
            if image.jpeg_coverage().is_some() {
                context.require_owned()?;
                image.jpeg_paint(budget)?
            } else {
                None
            }
        } else {
            None
        };
        result.push(op);
        if let Some(paint) = paint {
            result.extend_from_slice(paint.operations());
        }
    }
    require(context.clips.is_empty() && !context.text)?;
    Ok(result)
}
