//! Experimental native layout gate. Legacy overlay validation is unchanged.
use crate::{EngineError, EngineResult, FontAsset};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ops::Range;
use unicode_bidi::{BidiInfo, Level};
use unicode_script::{Script, UnicodeScript};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextDirection {
    #[default]
    Auto,
    Ltr,
    Rtl,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRange {
    pub utf8_start: usize,
    pub utf8_end: usize,
    pub utf16_start: usize,
    pub utf16_end: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextBoundary {
    pub utf8: usize,
    pub utf16: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextBounds {
    pub x_min: f32,
    pub y_min: f32,
    pub x_max: f32,
    pub y_max: f32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionedGlyph {
    pub glyph_id: u16,
    pub x: f32,
    pub y: f32,
    pub x_advance: f32,
    pub y_advance: f32,
    pub x_offset: f32,
    pub y_offset: f32,
    pub cluster: TextRange,
    pub bounds: Option<TextBounds>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapedRun {
    pub direction: TextDirection,
    pub script: String,
    pub range: TextRange,
    pub glyphs: Vec<PositionedGlyph>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapedText {
    pub version: u32,
    pub font_id: String,
    pub text: String,
    pub font_size: f32,
    pub direction: TextDirection,
    pub ligatures: bool,
    pub width: f32,
    pub bounds: Option<TextBounds>,
    pub runs: Vec<ShapedRun>,
    pub grapheme_boundaries: Vec<TextBoundary>,
}

fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidRequest(message.into())
}
const MAX_TEXT_BYTES: usize = 16384;
const MAX_SCALARS: usize = 4096;
const MAX_GLYPHS: usize = 16384;
const MAX_RUNS: usize = 256;

fn checked_text(text: &str) -> EngineResult<()> {
    if text.trim().is_empty() || text.len() > MAX_TEXT_BYTES || text.chars().count() > MAX_SCALARS {
        return Err(invalid(
            "Shape one nonempty line within 4096 Unicode scalars and 16384 UTF-8 bytes.",
        ));
    }
    for c in text.chars() {
        // Joiners are deliberate shaping input and remain in logical text.
        // Explicit bidi controls, invisible separators, variation selectors,
        // line breaks and tags need separate UI/selection policy before use.
        if c.is_control()
            || matches!(c as u32,0xad|0x34f|0x61c|0x180b..=0x180f|0x200b|0x200e..=0x200f|0x2028..=0x202e|0x2060..=0x206f|0xfe00..=0xfe0f|0xfeff|0xfff9..=0xfffb|0xe0000..=0xe0fff)
        {
            return Err(invalid(format!("Control or formatting character U+{:04X} is not admitted by the single-line shaping policy.",c as u32)));
        }
    }
    for grapheme in text.graphemes(true) {
        if grapheme.chars().count() > 64 {
            return Err(invalid(
                "A shaping grapheme cannot exceed 64 Unicode scalars.",
            ));
        }
    }
    Ok(())
}

fn text_range(range: Range<usize>, utf16: &[usize]) -> TextRange {
    TextRange {
        utf8_start: range.start,
        utf8_end: range.end,
        utf16_start: utf16[range.start],
        utf16_end: utf16[range.end],
    }
}

fn script_runs(text: &str, range: Range<usize>) -> Vec<(Range<usize>, Script)> {
    // Itemize whole graphemes so combining/Indic clusters are never split.
    // Common/inherited graphemes adopt adjacent script within the bidi run.
    let mut items: Vec<_> = text[range.clone()]
        .grapheme_indices(true)
        .map(|(start, g)| {
            let script = g.chars().map(|c| c.script()).find(|script| {
                !matches!(script, Script::Common | Script::Inherited | Script::Unknown)
            });
            (range.start + start..range.start + start + g.len(), script)
        })
        .collect();
    let mut prior = None;
    for (_, script) in &mut items {
        if script.is_some() {
            prior = *script;
        } else {
            *script = prior;
        }
    }
    let mut next = Some(Script::Common);
    for (_, script) in items.iter_mut().rev() {
        if script.is_some() {
            next = *script;
        } else {
            *script = next;
        }
    }
    let mut runs: Vec<(Range<usize>, Script)> = Vec::new();
    for (range, script) in items {
        let script = script.unwrap_or(Script::Common);
        if let Some((last, previous)) = runs.last_mut() {
            if *previous == script {
                last.end = range.end;
                continue;
            }
        }
        runs.push((range, script));
    }
    runs
}

fn union(a: Option<TextBounds>, b: TextBounds) -> Option<TextBounds> {
    Some(if let Some(a) = a {
        TextBounds {
            x_min: a.x_min.min(b.x_min),
            y_min: a.y_min.min(b.y_min),
            x_max: a.x_max.max(b.x_max),
            y_max: a.y_max.max(b.y_max),
        }
    } else {
        b
    })
}

/// Shape exact validated bytes into visual runs. Coordinates are PDF points,
/// baseline zero, y-up. Glyph x/y already include their HarfRust offsets.
pub fn shape_text(
    font: &FontAsset,
    text: &str,
    font_size: f32,
    direction: TextDirection,
    ligatures: bool,
) -> EngineResult<ShapedText> {
    checked_text(text)?;
    if !font_size.is_finite() || !(1.0..=1000.0).contains(&font_size) {
        return Err(invalid(
            "Shaping font size must be finite and between 1 and 1000 points.",
        ));
    }
    let face = crate::fonts::checked_face(&font.bytes)?;
    let hash: String = Sha256::digest(&font.bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if hash != font.info.id {
        return Err(invalid(
            "The shaping font ID does not match its immutable bytes.",
        ));
    }
    let font_ref = harfrust::FontRef::new(&font.bytes)
        .map_err(|_| invalid("HarfRust could not read the validated font."))?;
    let data = harfrust::ShaperData::new(&font_ref);
    let shaper = data.shaper(&font_ref).build();
    let scale = font_size / face.units_per_em() as f32;
    let base_level = match direction {
        TextDirection::Auto => None,
        TextDirection::Ltr => Some(Level::ltr()),
        TextDirection::Rtl => Some(Level::rtl()),
    };
    let bidi = BidiInfo::new(text, base_level);
    if bidi.paragraphs.len() != 1 {
        return Err(invalid("Only one shaping paragraph is supported."));
    }
    let paragraph = &bidi.paragraphs[0];
    let resolved = if paragraph.level.is_rtl() {
        TextDirection::Rtl
    } else {
        TextDirection::Ltr
    };
    let (levels, visual) = bidi.visual_runs(paragraph, 0..text.len());
    let mut utf16 = vec![usize::MAX; text.len() + 1];
    let mut units = 0;
    for (index, c) in text.char_indices() {
        utf16[index] = units;
        units += c.len_utf16();
    }
    utf16[text.len()] = units;
    let grapheme_boundaries = text
        .grapheme_indices(true)
        .map(|(utf8, _)| TextBoundary {
            utf8,
            utf16: utf16[utf8],
        })
        .chain(std::iter::once(TextBoundary {
            utf8: text.len(),
            utf16: units,
        }))
        .collect::<Vec<_>>();
    let mut result = ShapedText {
        version: 1,
        font_id: hash,
        text: text.into(),
        font_size,
        direction: resolved,
        ligatures,
        width: 0.0,
        bounds: None,
        runs: Vec::new(),
        grapheme_boundaries,
    };
    let features: Vec<_> = if ligatures {
        Vec::new()
    } else {
        ["liga=0", "clig=0", "dlig=0", "hlig=0"]
            .iter()
            .map(|feature| feature.parse::<harfrust::Feature>().unwrap())
            .collect()
    };
    let mut glyph_count = 0;
    for visual_run in visual {
        let rtl = levels[visual_run.start].is_rtl();
        let mut scripts = script_runs(text, visual_run);
        if rtl {
            scripts.reverse();
        }
        for (range, script) in scripts {
            if result.runs.len() >= MAX_RUNS {
                return Err(invalid("The text exceeds 256 shaping runs."));
            }
            let mut buffer = harfrust::UnicodeBuffer::new();
            for (index, c) in text[range.clone()].char_indices() {
                buffer.add(c, (range.start + index) as u32);
            }
            buffer.set_direction(if rtl {
                harfrust::Direction::RightToLeft
            } else {
                harfrust::Direction::LeftToRight
            });
            buffer.set_script(
                script
                    .short_name()
                    .parse()
                    .map_err(|_| invalid("The script cannot be represented by HarfRust."))?,
            );
            buffer.set_cluster_level(harfrust::BufferClusterLevel::MonotoneGraphemes);
            buffer.set_pre_context(&text[..range.start]);
            buffer.set_post_context(&text[range.end..]);
            let shaped = shaper.shape(buffer, harfrust::ShapeOptions::new().features(&features));
            glyph_count += shaped.len();
            if shaped.is_empty() || glyph_count > MAX_GLYPHS {
                return Err(invalid(
                    "Shaping returned no glyphs or exceeded 16384 glyphs.",
                ));
            }
            if shaped.glyph_infos().len() != shaped.glyph_positions().len()
                || shaped.glyph_infos().iter().any(|info| {
                    info.cluster as usize >= range.end || (info.cluster as usize) < range.start
                })
            {
                return Err(invalid("HarfRust returned invalid glyph or cluster data."));
            }
            let mut clusters: Vec<_> = shaped
                .glyph_infos()
                .iter()
                .map(|info| info.cluster as usize)
                .collect();
            clusters.push(range.end);
            clusters.sort_unstable();
            clusters.dedup();
            if clusters.first() != Some(&range.start)
                || clusters.iter().any(|index| {
                    *index < range.start || *index > range.end || !text.is_char_boundary(*index)
                })
            {
                return Err(invalid(
                    "HarfRust returned invalid logical cluster boundaries.",
                ));
            }
            let mut run = ShapedRun {
                direction: if rtl {
                    TextDirection::Rtl
                } else {
                    TextDirection::Ltr
                },
                script: script.short_name().into(),
                range: text_range(range.clone(), &utf16),
                glyphs: Vec::new(),
            };
            let mut pen_y = 0.0;
            for (info, position) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
                let glyph_id = u16::try_from(info.glyph_id)
                    .map_err(|_| invalid("The shaped glyph exceeds the TrueType glyph range."))?;
                if glyph_id == 0 || glyph_id >= face.number_of_glyphs() {
                    let missing = text[info.cluster as usize..].chars().next().unwrap();
                    return Err(invalid(format!(
                        "{} has no shaped glyph for the cluster beginning U+{:04X} ({missing}).",
                        font.info.name, missing as u32
                    )));
                }
                let x_offset = position.x_offset as f32 * scale;
                let y_offset = position.y_offset as f32 * scale;
                let x = result.width + x_offset;
                let y = pen_y + y_offset;
                let x_advance = position.x_advance as f32 * scale;
                let y_advance = position.y_advance as f32 * scale;
                if [x, y, x_offset, y_offset, x_advance, y_advance]
                    .iter()
                    .any(|n| !n.is_finite() || n.abs() > 1_000_000.0)
                {
                    return Err(invalid("Shaped glyph positions exceed the geometry limit."));
                }
                let start = info.cluster as usize;
                let end = clusters[clusters.binary_search(&start).unwrap() + 1];
                let bounds = face
                    .glyph_bounding_box(ttf_parser::GlyphId(glyph_id))
                    .map(|b| TextBounds {
                        x_min: x + b.x_min as f32 * scale,
                        y_min: y + b.y_min as f32 * scale,
                        x_max: x + b.x_max as f32 * scale,
                        y_max: y + b.y_max as f32 * scale,
                    });
                if let Some(bounds) = bounds {
                    if [bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max]
                        .iter()
                        .any(|n| !n.is_finite() || n.abs() > 1_000_000.0)
                        || bounds.x_min > bounds.x_max
                        || bounds.y_min > bounds.y_max
                    {
                        return Err(invalid(
                            "Shaped glyph ink bounds exceed the geometry limit.",
                        ));
                    }
                    result.bounds = union(result.bounds, bounds);
                }
                run.glyphs.push(PositionedGlyph {
                    glyph_id,
                    x,
                    y,
                    x_advance,
                    y_advance,
                    x_offset,
                    y_offset,
                    cluster: text_range(start..end, &utf16),
                    bounds,
                });
                result.width += x_advance;
                pen_y += y_advance;
            }
            result.runs.push(run);
        }
    }
    if !result.width.is_finite() || result.width < 0.0 || result.width > 1_000_000.0 {
        return Err(invalid("The shaped line exceeds the geometry limit."));
    }
    Ok(result)
}
