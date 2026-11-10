//! Eligibility for reading rotated pages in source axes on a temporary copy.
//! Unknown directions and layouts keep PDFium's original extraction path.
use pdfium_render::prelude::*;

struct Baseline {
    y: f32,
    first: f32,
    last: f32,
    forward: bool,
    singleton: bool,
    objects: usize,
}

fn forward_scalar(value: u32) -> bool {
    // Precomposed Latin, ordinary spacing, punctuation and currency. Excludes
    // combining marks, bidi controls, soft hyphens and scripts requiring layout.
    matches!(value,
        0x20..=0x7e | 0xa0..=0xac | 0xae..=0x2ff | 0x1e00..=0x1eff |
        0x2000..=0x200a | 0x2010..=0x2027 | 0x202f..=0x205e | 0x20a0..=0x20cf)
}

pub(super) fn simple_forward_text(page: &PdfPage<'_>, text: &PdfPageText<'_>) -> bool {
    // chars_for_object scans the text page; keep the existing work limits.
    if page.objects().len() > 64 || text.chars().len() > 4096 {
        return false;
    }
    let mut text_objects = 0;
    let mut baselines: Vec<Baseline> = Vec::new();
    for object in page.objects().iter() {
        match object.object_type() {
            PdfPageObjectType::Text => {
                let Some(object) = object.as_text_object() else {
                    return false;
                };
                let Ok(matrix) = object.matrix() else {
                    return false;
                };
                if matrix.a() <= 0.0
                    || matrix.d() <= 0.0
                    || matrix.b() != 0.0
                    || matrix.c() != 0.0
                    || ![matrix.a(), matrix.d(), matrix.e(), matrix.f()]
                        .iter()
                        .all(|v| v.is_finite())
                    || !object.unscaled_font_size().value.is_finite()
                    || object.unscaled_font_size().value <= 0.0
                {
                    return false;
                }
                let Ok(chars) = text.chars_for_object(object) else {
                    return false;
                };
                if chars.is_empty() {
                    // Word can retain separate trailing-space objects which
                    // PDFium omits from the reader. They carry no direction
                    // evidence. Ignore only filled objects with finite point
                    // bounds; unknown, stroked, clipping or nonzero ink still
                    // keeps the original extraction path.
                    if object.render_mode() == PdfPageTextRenderMode::FilledUnstroked {
                        if let Ok(bounds) = object.bounds() {
                            let rect = bounds.to_rect();
                            if [rect.left().value, rect.right().value,
                                rect.bottom().value, rect.top().value]
                                .iter().all(|value| value.is_finite())
                                && rect.left() == rect.right()
                                && rect.bottom() == rect.top()
                            {
                                continue;
                            }
                        }
                    }
                    return false;
                }
                let mut first = None;
                let mut previous_x = f32::NEG_INFINITY;
                for character in chars.iter() {
                    if !forward_scalar(character.unicode_value()) {
                        return false;
                    }
                    let Ok((x, y)) = character.origin() else {
                        return false;
                    };
                    if !x.value.is_finite() || !y.value.is_finite() || x.value < previous_x {
                        return false;
                    }
                    let (_, baseline) = *first.get_or_insert((x.value, y.value));
                    if y.value != baseline {
                        return false;
                    }
                    previous_x = x.value;
                }
                let Some((first_x, y)) = first else {
                    return false;
                };
                let singleton = chars.len() == 1;
                if !singleton && previous_x <= first_x {
                    return false;
                }
                if let Some(line) = baselines.iter_mut().find(|line| line.y == y) {
                    // Only singleton-bearing baselines use this additional proof.
                    // Object order must establish forward movement across pieces;
                    // coincident/overprinted or backward members are ambiguous.
                    line.forward &= first_x > line.last;
                    line.last = previous_x;
                    line.singleton |= singleton;
                    line.objects += 1;
                } else {
                    baselines.push(Baseline {
                        y,
                        first: first_x,
                        last: previous_x,
                        forward: true,
                        singleton,
                        objects: 1,
                    });
                }
                text_objects += 1;
            }
            PdfPageObjectType::Path | PdfPageObjectType::Image | PdfPageObjectType::Shading => (),
            _ => return false,
        }
    }
    text_objects >= 2
        && baselines.iter().all(|line| {
            !line.singleton || (line.objects >= 2 && line.forward && line.last > line.first)
        })
}
