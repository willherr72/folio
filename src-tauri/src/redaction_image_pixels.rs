//! Conservative source-sample selection for image occurrences.
pub(crate) use crate::redaction_area_paths::ImageTransform;
use crate::redaction_area_paths::{AreaRect, PathError};
#[derive(Default)]
pub(crate) struct PixelBudget {
    comparisons: usize,
    sample_work: usize,
}
pub(crate) struct PixelPlan {
    pub selected: Vec<bool>,
    pub count: usize,
    pub bounds: AreaRect,
}
pub(crate) fn plan_pixels(
    matrix: ImageTransform,
    width: u32,
    height: u32,
    marks: &[AreaRect],
    budget: &mut PixelBudget,
) -> Result<Option<PixelPlan>, PathError> {
    if width == 0
        || height == 0
        || width > 32768
        || height > 32768
        || u64::from(width) * u64::from(height) > 16_000_000
    {
        return Err(PathError::Limit);
    }
    if !(1..=64).contains(&marks.len()) || marks.iter().any(|r| !r.valid_mark()) {
        return Err(PathError::InvalidMark);
    }
    let mut stack = vec![[0, 0, width, height]];
    let mut selected = Vec::new();
    let mut count = 0;
    let mut affected: Option<AreaRect> = None;
    while let Some([x0, y0, x1, y1]) = stack.pop() {
        if marks.len() > 8_000_000usize.saturating_sub(budget.comparisons) {
            return Err(PathError::Limit);
        }
        budget.comparisons += marks.len();
        let bounds = matrix.pixel_bounds([x0, y0, x1, y1], width, height)?;
        let mut touches = false;
        let mut contained = false;
        for mark in marks {
            touches |= mark.touches(bounds);
            contained |= mark.contains(bounds);
        }
        if !touches {
            continue;
        }
        if contained || (x1 - x0 == 1 && y1 - y0 == 1) {
            if selected.is_empty() {
                let pixels = width as usize * height as usize;
                if pixels > 64_000_000usize.saturating_sub(budget.sample_work) {
                    return Err(PathError::Limit);
                }
                budget.sample_work += pixels;
                selected.resize(width as usize * height as usize, false);
            }
            for row in y0..y1 {
                let start = row as usize * width as usize + x0 as usize;
                selected[start..start + (x1 - x0) as usize].fill(true);
            }
            count += (x1 - x0) as usize * (y1 - y0) as usize;
            if let Some(previous) = affected.as_mut() {
                previous.include(bounds);
            } else {
                affected = Some(bounds);
            }
        } else if x1 - x0 >= y1 - y0 && x1 - x0 > 1 {
            let middle = x0 + (x1 - x0) / 2;
            stack.push([middle, y0, x1, y1]);
            stack.push([x0, y0, middle, y1]);
        } else {
            let middle = y0 + (y1 - y0) / 2;
            stack.push([x0, middle, x1, y1]);
            stack.push([x0, y0, x1, middle]);
        }
    }
    Ok(affected.map(|bounds| PixelPlan {
        selected,
        count,
        bounds,
    }))
}

#[cfg(test)]
#[path = "redaction_image_pixels_tests.rs"]
mod tests;
