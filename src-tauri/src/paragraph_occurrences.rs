//! Resolve attributed draft text into exact ordered glyphs and layout controls.
//! Inputs are already verified font bindings; this module grants no font proof.
use super::source_glyphs::BoundText;
use std::sync::Arc;

type Result<T> = std::result::Result<T, &'static str>;

pub(super) struct Run<'a> {
    pub text: &'a str,
    pub style: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Glyph {
    pub bytes: Vec<u8>,
    // None retains the existing scalar/native metric path for non-TrueType fonts.
    pub gid: Option<u16>,
    pub width: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Atom {
    pub text: String,
    pub style: usize,
    // LF is a layout control, never an encoded font character.
    pub glyph: Option<Glyph>,
}

pub(super) fn resolve(
    runs: &[Run<'_>],
    // Derived formatting styles must share the SAME Arc when they refer to one
    // original verified font stream. Distinct streams are never merged by name,
    // program hash, glyph-book equality or equal Unicode text.
    sources: &[Option<Arc<BoundText>>],
    mut scalar: impl FnMut(usize, char) -> Result<Glyph>,
) -> Result<Vec<Atom>> {
    // Reject the complete request before copying text or invoking a font path.
    if runs.is_empty() || runs.len() > 4096 || sources.is_empty() || sources.len() > 64 {
        return Err("The paragraph occurrence palette or span count is invalid.");
    }
    let mut bytes = 0usize;
    let mut visible = false;
    for run in runs {
        bytes = bytes
            .checked_add(run.text.len())
            .filter(|n| *n <= 4096)
            .ok_or("The paragraph exceeds its text budget.")?;
        if run.style >= sources.len() || run.text.chars().any(|c| c.is_control() && c != '\n') {
            return Err("The paragraph has an invalid style or layout control.");
        }
        visible |= !run.text.trim().is_empty();
    }
    if !visible {
        return Err("The paragraph needs visible text.");
    }

    let mut groups: Vec<Option<Arc<BoundText>>> = Vec::new();
    let mut style_groups = Vec::with_capacity(sources.len());
    for source in sources {
        let existing = source.as_ref().and_then(|source| {
            groups.iter().position(|other| {
                other
                    .as_ref()
                    .is_some_and(|other| Arc::ptr_eq(source, other))
            })
        });
        let group = existing.unwrap_or_else(|| {
            groups.push(source.clone());
            groups.len() - 1
        });
        style_groups.push(group);
    }
    struct Slot {
        ch: char,
        style: usize,
        group: usize,
        offset: usize,
    }
    let mut texts = vec![String::new(); groups.len()];
    let mut boundaries = vec![Vec::new(); groups.len()];
    let mut slots = Vec::new();
    let mut previous = None;
    for run in runs {
        let group = style_groups[run.style];
        for ch in run.text.chars() {
            let offset = texts[group].len();
            slots.push(Slot {
                ch,
                style: run.style,
                group,
                offset,
            });
            if ch == '\n' {
                previous = None;
            } else {
                // Equal adjacent spans are transparent. A return to a style
                // after another style or LF cannot join their letters into one
                // physical glyph, including newly encoded replacement gaps.
                if previous != Some(run.style)
                    && offset > 0
                    && boundaries[group].last() != Some(&offset)
                {
                    boundaries[group].push(offset);
                }
                texts[group].push(ch);
                previous = Some(run.style);
            }
        }
    }
    let encoded = groups
        .iter()
        .enumerate()
        .map(|(style, source)| {
            source
                .as_ref()
                .filter(|_| !texts[style].is_empty())
                .map(|source| source.replace_separated(&texts[style], &boundaries[style]))
                .transpose()
        })
        .collect::<Result<Vec<_>>>()?;
    let mut cursors = vec![0; groups.len()];
    let mut result = Vec::new();
    let mut index = 0;
    let mut encoded_bytes = 0usize;
    while index < slots.len() {
        let slot = &slots[index];
        if slot.ch == '\n' {
            result.push(Atom {
                text: "\n".into(),
                style: slot.style,
                glyph: None,
            });
            index += 1;
            continue;
        }
        let (text, glyph, count) = if let Some(encoded) = &encoded[slot.group] {
            let ordinal = cursors[slot.group];
            let atom = encoded
                .atoms()
                .get(ordinal)
                .ok_or("Missing resolved source glyph.")?;
            if atom.unicode.start != slot.offset {
                return Err("The resolved glyph does not match its attributed offset.");
            }
            let text = &encoded.text()[atom.unicode.clone()];
            let count = text.chars().count();
            let part = slots
                .get(index..index + count)
                .ok_or("Incomplete attributed glyph.")?;
            let mut offset = slot.offset;
            for (actual, ch) in part.iter().zip(text.chars()) {
                if actual.style != slot.style
                    || actual.group != slot.group
                    || actual.ch != ch
                    || ch == '\n'
                    || actual.offset != offset
                {
                    return Err("A source glyph crosses a formatting or line boundary.");
                }
                offset += ch.len_utf8();
            }
            let code_len = encoded.bytes().len() / encoded.atoms().len();
            if !matches!(code_len, 1 | 2) {
                return Err("Invalid resolved glyph code length.");
            }
            let raw = encoded.bytes()[ordinal * code_len..(ordinal + 1) * code_len].to_vec();
            cursors[slot.group] += 1;
            (
                text.to_owned(),
                Glyph {
                    bytes: raw,
                    gid: Some(atom.gid),
                    width: atom.width,
                },
                count,
            )
        } else {
            (slot.ch.to_string(), scalar(slot.style, slot.ch)?, 1)
        };
        if !(1..=2).contains(&glyph.bytes.len())
            || glyph.gid == Some(0)
            || !glyph.width.is_finite()
            || !(0.0..=10000.).contains(&glyph.width)
        {
            return Err("The resolved glyph has invalid encoding or nominal metrics.");
        }
        encoded_bytes = encoded_bytes
            .checked_add(glyph.bytes.len())
            .filter(|n| *n <= 8192)
            .ok_or("The paragraph exceeds its encoded glyph budget.")?;
        result.push(Atom {
            text,
            style: slot.style,
            glyph: Some(glyph),
        });
        index += count;
    }
    if encoded
        .iter()
        .zip(cursors)
        .any(|(e, count)| e.as_ref().is_some_and(|e| e.atoms().len() != count))
    {
        return Err("The paragraph contains unattributed resolved glyphs.");
    }
    Ok(result)
}

#[cfg(test)]
#[path = "paragraph_occurrences_tests.rs"]
mod tests;
