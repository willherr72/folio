//! Development-only bounded Latin LTR area layout. Glyph ranges are line-local;
//! source and delimiter ranges address the original request in UTF-8 and UTF-16.
//! ASCII-space-only rows are empty; other whitespace-only rows are refused by
//! the existing shaper. Trailing ASCII spaces join the hard/end delimiter.
use crate::{EngineError, EngineResult, FontAsset, ShapedText, TextDirection, TextRange};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_script::{Script, UnicodeScript};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextAreaAlignment {
    Left,
    Center,
    Right,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextAreaBreak {
    Soft,
    Hard,
    End,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextAreaRequest {
    pub version: u32,
    pub text: String,
    pub font_size: f32,
    pub width: f32,
    pub height: f32,
    pub inset: f32,
    pub line_spacing: f32,
    pub alignment: TextAreaAlignment,
    pub direction: TextDirection,
    pub ligatures: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextAreaLine {
    pub source: TextRange,
    pub delimiter: TextRange,
    pub break_kind: TextAreaBreak,
    pub baseline_x: f32,
    pub baseline_y: f32,
    pub advance_width: f32,
    pub shaped: Option<ShapedText>,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextAreaOverflow {
    pub horizontal: bool,
    pub vertical: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextAreaLayout {
    pub version: u32,
    pub font_id: String,
    pub request: TextAreaRequest,
    pub lines: Vec<TextAreaLine>,
    pub overflow: TextAreaOverflow,
    pub can_export: bool,
}
fn invalid(message: &str) -> EngineError {
    EngineError::InvalidRequest(message.into())
}

// Each call can emit at most shaping's 16384 glyphs. Reserve that worst case
// before calling, including discarded candidates, rather than checking only
// after expensive shaping. Scalar work separately bounds growing prefixes.
struct ShapeBudget {
    scalars: usize,
    glyph_reservations: usize,
    emitted: usize,
}
impl ShapeBudget {
    fn shape(
        &mut self,
        font: &FontAsset,
        request: &TextAreaRequest,
        text: &str,
    ) -> EngineResult<ShapedText> {
        self.scalars = self
            .scalars
            .checked_sub(text.chars().count())
            .ok_or_else(|| invalid("Area shape work budget exhausted."))?;
        self.glyph_reservations = self
            .glyph_reservations
            .checked_sub(16384)
            .ok_or_else(|| invalid("Area glyph work budget exhausted."))?;
        crate::shape_text(
            font,
            text,
            request.font_size,
            TextDirection::Ltr,
            request.ligatures,
        )
    }
}
fn validate(r: &TextAreaRequest) -> EngineResult<()> {
    if r.version != 1 || r.direction != TextDirection::Ltr {
        return Err(invalid(
            "Area layout requires version 1 and explicit LTR direction.",
        ));
    }
    if ![r.width, r.height]
        .iter()
        .all(|v| v.is_finite() && *v > 0. && *v <= 14400.)
        || !r.font_size.is_finite()
        || !(1. ..=512.).contains(&r.font_size)
        || !r.inset.is_finite()
        || r.inset < 0.
        || r.inset * 2. >= r.width.min(r.height)
        || !r.line_spacing.is_finite()
        || !(1. ..=3.).contains(&r.line_spacing)
    {
        return Err(invalid(
            "Invalid area dimensions, inset, font size or line spacing.",
        ));
    }
    if r.text.len() > 16384 || r.text.chars().count() > 4096 {
        return Err(invalid("Area text exceeds 4096 scalars or 16384 bytes."));
    }
    let mut chars = r.text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\n' {
            continue;
        }
        if c == '\r' && chars.peek() == Some(&'\n') {
            continue;
        }
        if c.is_control()
            || matches!(c as u32,0xad|0x34f|0x61c|0x180b..=0x180f|0x200b|0x200e..=0x200f|0x2028..=0x202e|0x2060..=0x206f|0xfe00..=0xfe0f|0xfeff|0xfff9..=0xfffb|0xe0000..=0xe0fff)
            || !matches!(
                c.script(),
                Script::Latin | Script::Common | Script::Inherited
            )
            || matches!(
                unicode_bidi::bidi_class(c),
                unicode_bidi::BidiClass::R
                    | unicode_bidi::BidiClass::AL
                    | unicode_bidi::BidiClass::AN
            )
        {
            return Err(invalid("Unsupported area control, direction or script."));
        }
    }
    if r.text.graphemes(true).any(|g| g.chars().count() > 64) {
        return Err(invalid("Area grapheme exceeds 64 scalars."));
    }
    Ok(())
}
/// Greedy wrapping at ASCII-space grapheme runs. Trailing separators and hard
/// breaks are retained in unpainted delimiter spans; leading spaces are painted
/// with their first word. NBSP and all other nonseparators remain atomic.
pub fn layout_text_area(
    font: &FontAsset,
    request: &TextAreaRequest,
) -> EngineResult<TextAreaLayout> {
    validate(request)?;
    let face = font.face()?;
    let font_id: String = Sha256::digest(&font.bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if font_id != font.info.id {
        return Err(invalid("Area font identity does not match its bytes."));
    }
    let mut budget = ShapeBudget {
        scalars: 65536,
        glyph_reservations: 8388608,
        emitted: 16384,
    };
    let inner_width = request.width - 2. * request.inset;
    let mut utf16 = vec![0; request.text.len() + 1];
    let mut units = 0;
    for (index, c) in request.text.char_indices() {
        utf16[index] = units;
        units += c.len_utf16();
    }
    utf16[request.text.len()] = units;
    let range = |start: usize, end: usize| TextRange {
        utf8_start: start,
        utf8_end: end,
        utf16_start: utf16[start],
        utf16_end: utf16[end],
    };
    let mut lines = Vec::new();
    let mut paragraph_start = 0;
    loop {
        let newline = request.text[paragraph_start..]
            .find('\n')
            .map(|i| paragraph_start + i);
        let delimiter_end = newline.map_or(request.text.len(), |i| i + 1);
        let paragraph_end = newline.map_or(request.text.len(), |i| {
            if i > paragraph_start && request.text.as_bytes()[i - 1] == b'\r' {
                i - 1
            } else {
                i
            }
        });
        let paragraph = &request.text[paragraph_start..paragraph_end];
        // A space with a combining mark is not a separator: never cut a grapheme.
        let mut words = Vec::new();
        let mut word_start = None;
        for (i, g) in paragraph.grapheme_indices(true) {
            if g == " " {
                if let Some(start) = word_start.take() {
                    words.push((start, paragraph_start + i));
                }
            } else if word_start.is_none() {
                word_start = Some(paragraph_start + i);
            }
        }
        if let Some(start) = word_start {
            words.push((start, paragraph_end));
        }
        if !words.is_empty() && paragraph.trim().is_empty() {
            return Err(invalid(
                "Non-ASCII whitespace-only area rows are unsupported by the shaping checkpoint.",
            ));
        }
        let mut word = 0;
        let mut start = paragraph_start;
        loop {
            if lines.len() == 256 {
                return Err(invalid("Area exceeds 256 lines."));
            }
            let mut end = start;
            let mut shaped = None;
            if word < words.len() {
                end = words[word].1;
                shaped = Some(budget.shape(font, request, &request.text[start..end])?);
                word += 1;
                while word < words.len() {
                    let candidate =
                        budget.shape(font, request, &request.text[start..words[word].1])?;
                    let ink_fits = candidate.bounds.is_none_or(|b| {
                        b.x_max.max(candidate.width) - b.x_min.min(0.) <= inner_width
                    });
                    if candidate.width > inner_width || !ink_fits {
                        break;
                    }
                    end = words[word].1;
                    shaped = Some(candidate);
                    word += 1;
                }
            }
            let (next, break_kind) = if word < words.len() {
                (words[word].0, TextAreaBreak::Soft)
            } else {
                (
                    delimiter_end,
                    if newline.is_some() {
                        TextAreaBreak::Hard
                    } else {
                        TextAreaBreak::End
                    },
                )
            };
            if let Some(s) = &shaped {
                budget.emitted = budget
                    .emitted
                    .checked_sub(s.runs.iter().map(|r| r.glyphs.len()).sum())
                    .ok_or_else(|| invalid("Area emitted glyph budget exhausted."))?;
            }
            lines.push(TextAreaLine {
                source: range(start, end),
                delimiter: range(end, next),
                break_kind,
                baseline_x: 0.,
                baseline_y: 0.,
                advance_width: shaped.as_ref().map_or(0., |s| s.width),
                shaped,
            });
            if word == words.len() {
                break;
            }
            start = next;
        }
        if newline.is_none() {
            break;
        }
        paragraph_start = delimiter_end;
    }
    let scale = request.font_size / face.units_per_em() as f32;
    let mut ascent = (face.ascender() as f32 * scale).max(0.);
    let mut descent = (-face.descender() as f32 * scale).max(0.);
    for line in &lines {
        if let Some(bounds) = line.shaped.as_ref().and_then(|s| s.bounds) {
            ascent = ascent.max(bounds.y_max);
            descent = descent.max(-bounds.y_min);
        }
    }
    let pitch = (ascent + descent + (face.line_gap() as f32 * scale).max(0.))
        .max(request.font_size)
        * request.line_spacing;
    let mut overflow = TextAreaOverflow::default();
    for (index, line) in lines.iter_mut().enumerate() {
        line.baseline_x = request.inset
            + (inner_width - line.advance_width)
                * match request.alignment {
                    TextAreaAlignment::Left => 0.,
                    TextAreaAlignment::Center => 0.5,
                    TextAreaAlignment::Right => 1.,
                };
        line.baseline_y = request.inset + ascent + index as f32 * pitch;
        overflow.horizontal |= line.advance_width > inner_width;
        overflow.vertical |= line.baseline_y + descent > request.height - request.inset;
        if let Some(b) = line.shaped.as_ref().and_then(|s| s.bounds) {
            overflow.horizontal |= line.baseline_x + b.x_min < request.inset
                || line.baseline_x + b.x_max > request.width - request.inset;
            overflow.vertical |= line.baseline_y - b.y_max < request.inset
                || line.baseline_y - b.y_min > request.height - request.inset;
        }
    }
    Ok(TextAreaLayout {
        version: 1,
        font_id,
        request: request.clone(),
        lines,
        overflow,
        can_export: !overflow.horizontal && !overflow.vertical,
    })
}
