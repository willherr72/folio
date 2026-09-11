//! Experimental native layout gate. Legacy overlay validation is unchanged.
use crate::{EngineError, EngineResult, FontAsset};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, ops::Range};
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
const MAX_RUN_OPERATIONS: i32 = 262144;

/// ttf-parser's bbox and outline APIs traverse expanded composite trees. Bound
/// that work before either API, and charge each distinct top-level glyph once.
pub(crate) struct OutlineBudget<'a> {
    data: &'a [u8],
    loca: ttf_parser::loca::Table<'a>,
    glyphs: u16,
    remaining: usize,
    checked: BTreeMap<u16, bool>,
}
impl<'a> OutlineBudget<'a> {
    pub(crate) fn new(face: &ttf_parser::Face<'a>) -> EngineResult<Self> {
        let loca = face
            .raw_face()
            .table(ttf_parser::Tag::from_bytes(b"loca"))
            .ok_or_else(|| invalid("Missing outline glyph locations."))?;
        Ok(Self {
            data: face
                .raw_face()
                .table(ttf_parser::Tag::from_bytes(b"glyf"))
                .ok_or_else(|| invalid("Missing glyph outlines."))?,
            loca: ttf_parser::loca::Table::parse(
                face.tables().maxp.number_of_glyphs,
                face.tables().head.index_to_location_format,
                loca,
            )
            .ok_or_else(|| invalid("Malformed outline glyph locations."))?,
            glyphs: face.number_of_glyphs(),
            remaining: 100_000,
            checked: BTreeMap::new(),
        })
    }
    /// Returns whether the glyph declares nonempty contours. Such a glyph must
    /// produce an outline; None must not silently turn malformed ink into space.
    pub(crate) fn validate(&mut self, id: ttf_parser::GlyphId) -> EngineResult<bool> {
        if let Some(&nonempty) = self.checked.get(&id.0) {
            return Ok(nonempty);
        }
        let nonempty = self.visit(id, 0)?;
        self.checked.insert(id.0, nonempty);
        Ok(nonempty)
    }
    fn visit(&mut self, id: ttf_parser::GlyphId, depth: usize) -> EngineResult<bool> {
        if depth >= 16 || self.remaining == 0 {
            return Err(invalid("The outline exceeds the component work limit."));
        }
        self.remaining -= 1;
        if id.0 >= self.glyphs {
            return Err(invalid("Invalid outline component glyph."));
        }
        // glyph_range() conflates empty, descending and missing offsets. Only a
        // valid equal pair may stand for a blank glyph.
        let offset = |index| match self.loca {
            ttf_parser::loca::Table::Short(array) => array.get(index).map(|v| v as usize * 2),
            ttf_parser::loca::Table::Long(array) => array.get(index).map(|v| v as usize),
        };
        let start = offset(id.0).ok_or_else(|| invalid("Missing outline glyph location."))?;
        let end = offset(id.0 + 1).ok_or_else(|| invalid("Missing outline glyph location."))?;
        if end < start || end > self.data.len() {
            return Err(invalid("Malformed outline glyph location."));
        }
        if start == end {
            return Ok(false);
        }
        let glyph = self
            .data
            .get(start..end)
            .ok_or_else(|| invalid("Malformed outline glyph location."))?;
        let read = |offset: usize| -> EngineResult<u16> {
            let bytes = glyph
                .get(offset..offset + 2)
                .ok_or_else(|| invalid("Malformed outline program."))?;
            Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
        };
        let contours = read(0)? as i16;
        if glyph.len() < 10 {
            return Err(invalid("Malformed outline header."));
        }
        if contours > 0 {
            let contours = contours as usize;
            let mut previous = None;
            for i in 0..contours {
                let endpoint = read(10 + 2 * i)? as usize;
                if previous.is_some_and(|prev| endpoint <= prev) {
                    return Err(invalid("Malformed outline contours."));
                }
                previous = Some(endpoint);
            }
            self.remaining = self
                .remaining
                .checked_sub(previous.unwrap() + 1 + contours)
                .ok_or_else(|| invalid("The outline exceeds the point work limit."))?;
        } else if contours < 0 {
            let mut offset = 10;
            loop {
                let flags = read(offset)?;
                // This ttf-parser iterator does not consume point-matched args.
                // Refuse them so validation and actual traversal stay aligned.
                if flags & 2 == 0 {
                    return Err(invalid("Unsupported point-matched outline component."));
                }
                let child = ttf_parser::GlyphId(read(offset + 2)?);
                offset += 4 + if flags & 1 != 0 { 4 } else { 2 };
                offset += if flags & 0x80 != 0 {
                    8
                } else if flags & 0x40 != 0 {
                    4
                } else if flags & 8 != 0 {
                    2
                } else {
                    0
                };
                if offset > glyph.len() {
                    return Err(invalid("Malformed composite outline."));
                }
                self.visit(child, depth + 1)?;
                if flags & 0x20 == 0 {
                    break;
                }
            }
        }
        Ok(contours != 0)
    }
}

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
    let mut outline_budget = OutlineBudget::new(&face)?;
    let mut outline_bounds = BTreeMap::<u16, Option<ttf_parser::Rect>>::new();
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
            // Bound intermediate substitution growth, not merely final output.
            // Across <=4096 scalars and <=256 runs these per-run allowances sum
            // to <=6,291,456 instrumented operations (usually far fewer).
            let operations = ((text[range.clone()].chars().count() as i32) * 1024 + 8192)
                .min(MAX_RUN_OPERATIONS);
            let shaped = shaper.shape(
                buffer,
                harfrust::ShapeOptions::new()
                    .features(&features)
                    .max_glyphs(Some(MAX_GLYPHS - glyph_count))
                    .max_operations(Some(operations)),
            );
            if !shaped.is_successful() {
                return Err(invalid("Shaping exceeded its glyph, operation, or lookup-recursion budget. No partial layout was accepted."));
            }
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
                if let std::collections::btree_map::Entry::Vacant(entry) =
                    outline_bounds.entry(glyph_id)
                {
                    let nonempty = outline_budget.validate(ttf_parser::GlyphId(glyph_id))?;
                    let bounds = face.glyph_bounding_box(ttf_parser::GlyphId(glyph_id));
                    if nonempty && bounds.is_none() {
                        return Err(invalid("The nonempty glyph outline is malformed."));
                    }
                    entry.insert(bounds);
                }
                let bounds = outline_bounds[&glyph_id].map(|b| TextBounds {
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
