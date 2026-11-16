//! Shared canonical character extraction with native ownership indices retained.
use super::*;
pub(super) struct NativeReadingEntry {pub character:PdfTextCharacter,pub indices:std::ops::Range<usize>,pub generated:bool}
pub(super) fn read(text:&PdfPageText<'_>,geometry:PageGeometry,character_limit:Option<usize>)->EngineResult<Vec<NativeReadingEntry>>{
    if text.len()<0{return Err(EngineError::InvalidRequest("The page character count is invalid.".into()));}
    if character_limit.is_some_and(|limit|text.len() as usize>limit) {
        return Err(EngineError::InvalidRequest("This page exceeds the 250,000-character search limit.".into()));
    }
    let chars=text.chars();
    let mut characters: Vec<NativeReadingEntry> = Vec::with_capacity(chars.len() as usize);
    let mut pending_high_surrogate: Option<u32> = None;
    let mut last_position = Point { x: 0.0, y: 0.0 };
    for character in chars.iter() {
        let unicode = character.unicode_value();
        let value = char::from_u32(unicode).unwrap_or(char::REPLACEMENT_CHARACTER);
        // Generated whitespace can lack glyph bounds. Keep it in reading order
        // so browser range copying retains word and line separators.
        let bounds = if value.is_whitespace() {
            character
                .loose_bounds()
                .or_else(|_| character.tight_bounds())
        } else {
            character
                .tight_bounds()
                .or_else(|_| character.loose_bounds())
        };
        let (position, width, height) = match bounds {
            Ok(bounds) => {
                let a = geometry.pdf_to_displayed(bounds.left().value, bounds.bottom().value);
                let b = geometry.pdf_to_displayed(bounds.right().value, bounds.top().value);
                (
                    Point {
                        x: a.x.min(b.x),
                        y: a.y.min(b.y),
                    },
                    (a.x - b.x).abs(),
                    (a.y - b.y).abs(),
                )
            }
            Err(_) => {
                let position = character
                    .origin()
                    .map(|(x, y)| geometry.pdf_to_displayed(x.value, y.value))
                    .unwrap_or(last_position);
                (position, 0.0, 0.0)
            }
        };
        last_position = position;
        // PDFium can expose a ToUnicode/ActualText supplementary scalar as
        // two UTF-16 entries. Selection must carry the same Unicode as
        // extract_text(), with one box covering both entries. An isolated
        // surrogate retains the replacement character; never skip a unit.
        if let Some(high) = pending_high_surrogate.take() {
            if (0xdc00..=0xdfff).contains(&unicode) {
                let previous = characters
                    .last_mut()
                    .expect("a pending surrogate has an entry");
                previous.indices.end=character.index()+1;
                previous.generated|=character.is_generated().unwrap_or(true);
                let previous=&mut previous.character;
                previous.text =
                    char::from_u32(0x10000 + ((high - 0xd800) << 10) + unicode - 0xdc00)
                        .expect("a surrogate pair encodes a Unicode scalar")
                        .to_string();
                let right = (previous.x + previous.width).max(position.x + width);
                let bottom = (previous.y + previous.height).max(position.y + height);
                previous.x = previous.x.min(position.x);
                previous.y = previous.y.min(position.y);
                previous.width = right - previous.x;
                previous.height = bottom - previous.y;
                continue;
            }
        }
        if (0xd800..=0xdbff).contains(&unicode) {
            pending_high_surrogate = Some(unicode);
        }
        characters.push(NativeReadingEntry {indices:character.index()..character.index()+1,generated:character.is_generated().unwrap_or(true),character:PdfTextCharacter {
            text: value.to_string(),
            x: position.x,
            y: position.y,
            width,
            height,
        }});
    }
    Ok(characters)
}
