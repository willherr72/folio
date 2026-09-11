//! Positive evidence for the font paths that existing-text editing can preserve.
//! No font is accepted solely because it has a familiar name or a ToUnicode map.
use super::{invalid, EngineError, EngineResult};
use lopdf::{Dictionary, Document, Object, ObjectId};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};

const MAX_FONT: usize = 16 * 1024 * 1024;
const MAX_CMAP: usize = 2 * 1024 * 1024;

pub(super) fn key(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(super) fn base_name(name: &str) -> &str {
    if name.as_bytes().get(6) == Some(&b'+')
        && name.as_bytes()[..6].iter().all(u8::is_ascii_uppercase)
    {
        &name[7..]
    } else {
        name
    }
}

pub(super) fn latin(text: &str) -> bool {
    !text.trim().is_empty() && text.len() <= 4096 && text.chars().all(latin_character)
}
fn latin_character(c: char) -> bool {
    matches!(c as u32,
        0x20..=0x7e | 0xa0..=0xac | 0xae..=0x2ff | 0x1e00..=0x1eff |
        0x2000..=0x200a | 0x2010..=0x2027 | 0x202f..=0x205e | 0x20a0..=0x20cf)
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct VerifiedFont {
    // Unicode -> (PDF character code, actual TrueType glyph, advance in 1000 em).
    characters: BTreeMap<char, (u16, u16, f32)>,
    code_len: usize,
}

impl VerifiedFont {
    pub(super) fn encode(&self, text: &str) -> EngineResult<Vec<u8>> {
        self.supports(text)?;
        Ok(text
            .chars()
            .flat_map(|c| {
                let code = self.characters[&c].0.to_be_bytes();
                if self.code_len == 1 {
                    vec![code[1]]
                } else {
                    code.to_vec()
                }
            })
            .collect())
    }
    pub(super) fn supports(&self, text: &str) -> EngineResult<()> {
        for c in text.chars() {
            if !self.characters.contains_key(&c) {
                return Err(invalid(&format!("The embedded font has no verified glyph for U+{:04X} ({c}). Choose an explicit substitute font to use this character.", c as u32)));
            }
        }
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct FontProofs {
    programs: HashMap<(String, String), Vec<Result<Arc<VerifiedFont>, String>>>,
    allowed: HashSet<ObjectId>,
}

impl FontProofs {
    pub(super) fn read(bytes: &[u8]) -> EngineResult<Self> {
        let pdf = Document::load_mem_with_options(
            bytes,
            lopdf::LoadOptions::with_max_decompressed_size(MAX_FONT),
        )
        .map_err(|_| invalid("The embedded font resources could not be inspected safely."))?;
        let mut result = Self::default();
        let mut retained = 0usize;
        let mut program_streams = HashSet::new();
        let mut program_count = 0usize;
        let mut total_program_bytes = 0usize;
        // PDFium's font-data accessor allocates its reported byte length. Bound
        // every embedded program before calling it, including unsupported types.
        let mut stack: Vec<_> = pdf
            .objects
            .values()
            .map(|object| (object, 0usize))
            .collect();
        let mut visited = 0usize;
        while let Some((object, depth)) = stack.pop() {
            visited += 1;
            if visited > 250000 || depth > 32 {
                return Err(invalid(
                    "The embedded font resource graph exceeds its size limit.",
                ));
            }
            let descriptor = match object {
                Object::Dictionary(dict) => dict,
                Object::Stream(stream) => &stream.dict,
                Object::Array(values) => {
                    stack.extend(values.iter().map(|value| (value, depth + 1)));
                    continue;
                }
                _ => continue,
            };
            // Indirect references are visited once in pdf.objects. Only descend
            // into direct containers, so aliases cannot expand this traversal.
            stack.extend(descriptor.iter().map(|(_, value)| (value, depth + 1)));
            for name in [
                b"FontFile".as_slice(),
                b"FontFile2".as_slice(),
                b"FontFile3".as_slice(),
            ] {
                if let Ok(value) = descriptor.get(name) {
                    if let Ok(id) = value.as_reference() {
                        if !program_streams.insert(id) {
                            continue;
                        }
                    }
                    program_count += 1;
                    if program_count > 64 {
                        return Err(invalid("Too many embedded font programs for safe editing."));
                    }
                    total_program_bytes += stream(&pdf, value, MAX_FONT)?.len();
                    if total_program_bytes > 128 * 1024 * 1024 {
                        return Err(invalid(
                            "The embedded font programs exceed the total size limit.",
                        ));
                    }
                }
            }
        }
        let mut resource_count = 0usize;
        for (id, object) in &pdf.objects {
            let Ok(font) = object.as_dict() else {
                continue;
            };
            let subtype = font
                .get(b"Subtype")
                .and_then(Object::as_name)
                .unwrap_or_default();
            if !matches!(subtype, b"TrueType" | b"Type0") {
                continue;
            }
            resource_count += 1;
            if resource_count > 64 {
                return Err(invalid(
                    "Too many embedded font resources for safe editing.",
                ));
            }
            let Ok(program) = program(&pdf, font) else {
                continue;
            };
            retained = retained
                .checked_add(program.len())
                .ok_or_else(|| invalid("Too many embedded font resources."))?;
            if result.programs.len() >= 64 || retained > 128 * 1024 * 1024 {
                return Err(invalid(
                    "Too many embedded font resources for safe editing.",
                ));
            }
            let hash = key(&program);
            let verified =
                verify(&pdf, font, &program)
                    .map(Arc::new)
                    .map_err(|error| match error {
                        EngineError::InvalidRequest(reason) => reason,
                        _ => "The embedded font mapping could not be verified.".into(),
                    });
            if verified.is_ok() {
                result.allowed.insert(*id);
            }
            let name = font
                .get(b"BaseFont")
                .and_then(Object::as_name)
                .map_err(|_| invalid("The embedded font name is missing."))?;
            let name = String::from_utf8(name.to_vec())
                .map_err(|_| invalid("The embedded font name cannot be identified safely."))?;
            result
                .programs
                .entry((name, hash))
                .or_default()
                .push(verified);
        }
        Ok(result)
    }

    pub(super) fn allows_resource(&self, id: ObjectId) -> bool {
        self.allowed.contains(&id)
    }

    pub(super) fn find(&self, name: &str, hash: &str) -> Result<Arc<VerifiedFont>, String> {
        let mut entries = self
            .programs
            .iter()
            .filter(|((candidate, program), _)| {
                program == hash && base_name(candidate) == base_name(name)
            })
            .flat_map(|(_, entries)| entries.iter());
        let first = entries
            .next()
            .ok_or("The embedded TrueType program or its Unicode mapping could not be verified.")?
            .clone()?;
        for entry in entries {
            let other = entry.clone()?;
            if other != first {
                return Err("The same embedded program has ambiguous PDF font mappings; it cannot be edited safely.".into());
            }
        }
        Ok(first)
    }
}

fn dict<'a>(pdf: &'a Document, value: &'a Object) -> EngineResult<&'a Dictionary> {
    pdf.dereference(value)
        .and_then(|(_, value)| value.as_dict())
        .map_err(|_| invalid("The embedded font dictionary is malformed."))
}
fn array<'a>(pdf: &'a Document, value: &'a Object) -> EngineResult<&'a Vec<Object>> {
    pdf.dereference(value)
        .and_then(|(_, value)| value.as_array())
        .map_err(|_| invalid("The embedded font array is malformed."))
}
fn field<'a>(value: &'a Dictionary, name: &[u8]) -> EngineResult<&'a Object> {
    value
        .get(name)
        .map_err(|_| invalid("The embedded font is missing a required mapping or metric."))
}
fn stream(pdf: &Document, value: &Object, limit: usize) -> EngineResult<Vec<u8>> {
    let stream = pdf
        .dereference(value)
        .and_then(|(_, value)| value.as_stream())
        .map_err(|_| invalid("The embedded font stream is missing or malformed."))?;
    if stream.dict.has(b"F") {
        return Err(invalid("External font streams cannot be edited."));
    }
    if stream.dict.has(b"Filter") {
        stream.decompressed_content_with_limit(limit).map_err(|_| {
            invalid("The embedded font stream exceeds the size limit or cannot be decoded.")
        })
    } else if stream.content.len() <= limit {
        Ok(stream.content.clone())
    } else {
        Err(invalid("The embedded font stream exceeds the size limit."))
    }
}
fn descendant<'a>(pdf: &'a Document, font: &'a Dictionary) -> EngineResult<&'a Dictionary> {
    if font.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Type0") {
        let children = array(pdf, field(font, b"DescendantFonts")?)?;
        if children.len() != 1 {
            return Err(invalid(
                "The composite font must have one TrueType descendant.",
            ));
        }
        let child = dict(pdf, &children[0])?;
        if child.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"CIDFontType2") {
            return Err(invalid("Only embedded TrueType outlines can be edited."));
        }
        Ok(child)
    } else {
        Ok(font)
    }
}
fn program(pdf: &Document, font: &Dictionary) -> EngineResult<Vec<u8>> {
    let child = descendant(pdf, font)?;
    let descriptor = dict(pdf, field(child, b"FontDescriptor")?)?;
    stream(pdf, field(descriptor, b"FontFile2")?, MAX_FONT)
}

fn hex(value: &Object, length: usize) -> EngineResult<u16> {
    let bytes = value
        .as_str()
        .map_err(|_| invalid("The Unicode mapping contains a non-string code."))?;
    if bytes.len() != length || !matches!(length, 1 | 2) {
        return Err(invalid(
            "The font uses unsupported variable-length character codes.",
        ));
    }
    Ok(bytes
        .iter()
        .fold(0u16, |n, byte| n * 256 + u16::from(*byte)))
}
fn unicode(value: &Object) -> EngineResult<char> {
    let bytes = value
        .as_str()
        .map_err(|_| invalid("The font Unicode mapping is malformed."))?;
    if !matches!(bytes.len(), 2 | 4) {
        return Err(invalid("The font maps one glyph to multiple characters or a ligature. Its text cannot be edited safely."));
    }
    let units: Vec<_> = bytes
        .chunks_exact(2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .collect();
    let text = String::from_utf16(&units)
        .map_err(|_| invalid("The font Unicode mapping is malformed."))?;
    let mut chars = text.chars();
    let character = chars
        .next()
        .ok_or_else(|| invalid("The font Unicode mapping is empty."))?;
    if chars.next().is_some() {
        return Err(invalid("The font maps one glyph to multiple characters or a ligature. Its text cannot be edited safely."));
    }
    Ok(character)
}

// Parse the data sections ourselves: the general PDF CMap parser builds an
// unbounded reverse map and overwrites overlapping definitions. Both behaviors
// are unsuitable for positively proving an untrusted font's unique mapping.
fn cmap(bytes: &[u8], code_len: usize) -> EngineResult<BTreeMap<u16, u32>> {
    let content = lopdf::content::Content::decode(bytes)
        .map_err(|_| invalid("The embedded font ToUnicode map is malformed."))?;
    let mut result = BTreeMap::new();
    let mut codespaces = Vec::new();
    let mut section: Option<(&str, usize)> = None;
    for operation in &content.operations {
        let name = operation.operator.as_str();
        if name == "usecmap" {
            return Err(invalid("Inherited font CMaps cannot be verified safely."));
        }
        if matches!(name, "beginbfchar" | "beginbfrange" | "begincodespacerange") {
            if section.is_some() || operation.operands.len() != 1 {
                return Err(invalid("The embedded font CMap sections are ambiguous."));
            }
            let count = operation.operands[0]
                .as_i64()
                .map_err(|_| invalid("Invalid font CMap count."))?;
            if !(0..=65536).contains(&count) {
                return Err(invalid("The embedded font CMap exceeds its size limit."));
            }
            section = Some((name, count as usize));
        } else if matches!(name, "endbfchar" | "endbfrange" | "endcodespacerange") {
            let (start, count) = section
                .take()
                .ok_or_else(|| invalid("The embedded font CMap sections are incomplete."))?;
            let (expected, arity) = match name {
                "endbfchar" => ("beginbfchar", 2),
                "endbfrange" => ("beginbfrange", 3),
                _ => ("begincodespacerange", 2),
            };
            if start != expected || operation.operands.len() != count * arity {
                return Err(invalid(
                    "The embedded font CMap count does not match its entries.",
                ));
            }
            for item in operation.operands.chunks_exact(arity) {
                let first = hex(&item[0], code_len)?;
                if name == "endcodespacerange" {
                    let last = hex(&item[1], code_len)?;
                    if first > last {
                        return Err(invalid("Invalid font codespace range."));
                    }
                    if codespaces.len() >= 65536 {
                        return Err(invalid("The font codespaces exceed their size limit."));
                    }
                    codespaces.push((first, last));
                    continue;
                }
                let last = if name == "endbfchar" {
                    first
                } else {
                    hex(&item[1], code_len)?
                };
                if first > last || result.len() + usize::from(last - first) + 1 > 65536 {
                    return Err(invalid("The embedded font mapping exceeds its size limit."));
                }
                let target = &item[arity - 1];
                if let Object::Array(values) = target {
                    if values.len() != usize::from(last - first) + 1 {
                        return Err(invalid("The font Unicode mapping array is incomplete."));
                    }
                }
                for code in first..=last {
                    let character = if let Object::Array(values) = target {
                        unicode(&values[usize::from(code - first)])? as u32
                    } else {
                        let base = unicode(target)? as u32;
                        base.checked_add(u32::from(code - first))
                            .filter(|value| *value <= 0x10ffff)
                            .ok_or_else(|| invalid("The font Unicode range is invalid."))?
                    };
                    if result.insert(code, character).is_some() {
                        return Err(invalid(
                            "The embedded font has ambiguous overlapping Unicode mappings.",
                        ));
                    }
                }
            }
        } else if section.is_some() {
            return Err(invalid(
                "The embedded font CMap contains unsupported instructions.",
            ));
        }
    }
    codespaces.sort_unstable();
    if codespaces.windows(2).any(|pair| pair[0].1 >= pair[1].0) {
        return Err(invalid("The embedded font codespaces overlap ambiguously."));
    }
    if section.is_some()
        || codespaces.is_empty()
        || result.is_empty()
        || result.keys().any(|code| {
            let index = codespaces.partition_point(|(first, _)| first <= code);
            index == 0 || codespaces[index - 1].1 < *code
        })
    {
        return Err(invalid(
            "The embedded font Unicode codespaces are incomplete.",
        ));
    }
    Ok(result)
}

fn widths(pdf: &Document, font: &Dictionary, cid: bool) -> EngineResult<BTreeMap<u16, f32>> {
    let mut result = BTreeMap::new();
    let mut put = |code: u16, value: &Object| -> EngineResult<()> {
        let n = value
            .as_float()
            .map_err(|_| invalid("The embedded glyph width is malformed."))?;
        if !n.is_finite() || !(0.0..=10000.0).contains(&n) || result.insert(code, n).is_some() {
            return Err(invalid(
                "The embedded glyph widths are invalid or ambiguous.",
            ));
        }
        Ok(())
    };
    if !cid {
        let first = field(font, b"FirstChar")?
            .as_i64()
            .map_err(|_| invalid("The simple font character range is malformed."))?;
        let last = field(font, b"LastChar")?
            .as_i64()
            .map_err(|_| invalid("The simple font character range is malformed."))?;
        let values = array(pdf, field(font, b"Widths")?)?;
        if first < 0 || last > 255 || first > last || values.len() != (last - first + 1) as usize {
            return Err(invalid("The simple font width range is incomplete."));
        }
        for (index, value) in values.iter().enumerate() {
            put(first as u16 + index as u16, value)?;
        }
    } else {
        let values = array(pdf, field(font, b"W")?)?;
        let mut index = 0;
        while index < values.len() {
            let first = values[index]
                .as_i64()
                .map_err(|_| invalid("Invalid CID width range."))?;
            if !(0..=65535).contains(&first) {
                return Err(invalid("Invalid CID width range."));
            }
            let next = values
                .get(index + 1)
                .ok_or_else(|| invalid("Incomplete CID widths."))?;
            if let Object::Array(widths) = next {
                if first as usize + widths.len() > 65536 {
                    return Err(invalid("CID widths exceed the character range."));
                }
                for (offset, width) in widths.iter().enumerate() {
                    put(first as u16 + offset as u16, width)?;
                }
                index += 2;
            } else {
                let last = next
                    .as_i64()
                    .map_err(|_| invalid("Invalid CID width range."))?;
                if last < first || last > 65535 {
                    return Err(invalid("Invalid CID width range."));
                }
                let value = values
                    .get(index + 2)
                    .ok_or_else(|| invalid("Incomplete CID widths."))?;
                for code in first..=last {
                    put(code as u16, value)?;
                }
                index += 3;
            }
        }
    }
    Ok(result)
}

fn win_ansi(code: u16) -> Option<char> {
    const EXT: [u16; 32] = [
        0x20ac, 0, 0x201a, 0x192, 0x201e, 0x2026, 0x2020, 0x2021, 0x2c6, 0x2030, 0x160, 0x2039,
        0x152, 0, 0x17d, 0, 0, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022, 0x2013, 0x2014, 0x2dc,
        0x2122, 0x161, 0x203a, 0x153, 0, 0x17e, 0x178,
    ];
    let value = if (128..160).contains(&code) {
        EXT[(code - 128) as usize]
    } else {
        code
    };
    (value != 0).then(|| char::from_u32(value as u32)).flatten()
}

fn verify(pdf: &Document, font: &Dictionary, bytes: &[u8]) -> EngineResult<VerifiedFont> {
    let face = ttf_parser::Face::parse(bytes, 0)
        .map_err(|_| invalid("The embedded TrueType program is malformed."))?;
    if face.tables().glyf.is_none() || face.tables().hmtx.is_none() || face.is_variable() {
        return Err(invalid(
            "Only static embedded TrueType outlines and horizontal metrics can be edited.",
        ));
    }
    let cid = font.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Type0");
    let child = descendant(pdf, font)?;
    let encoding = font
        .get(b"Encoding")
        .ok()
        .map(|value| pdf.dereference(value).map(|(_, value)| value))
        .transpose()
        .map_err(|_| invalid("The embedded font encoding is malformed."))?;
    if cid && encoding.and_then(|value| value.as_name().ok()) != Some(b"Identity-H") {
        return Err(invalid(
            "Only horizontal Identity-H composite font encodings can be edited.",
        ));
    }
    let legacy = !cid && encoding.is_none();
    if !cid
        && !legacy
        && encoding.and_then(|value| value.as_name().ok()) != Some(b"WinAnsiEncoding")
    {
        return Err(invalid(
            "This embedded simple font encoding cannot be verified safely.",
        ));
    }
    let maps = face
        .tables()
        .cmap
        .ok_or_else(|| invalid("The embedded font has no glyph character map."))?;
    if maps.subtables.len() > 32 {
        return Err(invalid(
            "The embedded font contains too many character maps for safe editing.",
        ));
    }
    if legacy
        && (maps.subtables.len() != 1
            || maps.subtables.get(0).is_none_or(|table| {
                table.platform_id != ttf_parser::PlatformId::Macintosh || table.encoding_id != 0
            }))
    {
        return Err(invalid(
            "The embedded simple font has an ambiguous implicit character encoding.",
        ));
    }
    let unicode=stream(pdf,font.get(b"ToUnicode").map_err(|_|invalid("The embedded font has no ToUnicode mapping; its characters cannot be verified safely."))?,MAX_CMAP)?;
    let unicode = cmap(&unicode, if cid { 2 } else { 1 })?;
    let widths = widths(pdf, child, cid)?;
    let glyph_map = if cid {
        let value = field(child, b"CIDToGIDMap")?;
        if value.as_name().ok() == Some(b"Identity") {
            None
        } else {
            let bytes = stream(pdf, value, 65536 * 2)?;
            if bytes.len() % 2 != 0 {
                return Err(invalid("The CID glyph mapping is incomplete."));
            }
            Some(bytes)
        }
    } else {
        None
    };
    let mut characters = BTreeMap::new();
    for (code, character) in unicode {
        let character = char::from_u32(character);
        // PDFium and ttf-parser do not necessarily select the same Unicode
        // subtable. All Unicode subtables must agree for a represented scalar;
        // otherwise a newly introduced character could render a different glyph
        // while extraction and the original-text no-op still appear correct.
        // CID fonts choose the glyph through their explicit CIDToGIDMap, and
        // the legacy subset path has exactly one verified Macintosh charmap.
        // Only WinAnsi relies on the engine's choice of nominal Unicode map.
        if let Some(character) =
            character.filter(|character| !cid && !legacy && latin_character(*character))
        {
            let mut agreed: Option<Option<ttf_parser::GlyphId>> = None;
            for table in maps.subtables.into_iter().filter(|table| {
                table.is_unicode()
                    && !matches!(
                        table.format,
                        ttf_parser::cmap::Format::UnicodeVariationSequences(_)
                    )
            }) {
                let gid = table.glyph_index(character as u32);
                if agreed.is_some_and(|previous| previous != gid) {
                    return Err(invalid(
                        "The embedded font has conflicting Unicode glyph maps.",
                    ));
                }
                agreed = Some(gid);
            }
        }
        let gid = if cid {
            match &glyph_map {
                Some(bytes) => bytes
                    .get(code as usize * 2..code as usize * 2 + 2)
                    .map(|b| u16::from_be_bytes([b[0], b[1]]))
                    .unwrap_or(0),
                None => code,
            }
        } else if legacy {
            maps.subtables
                .get(0)
                .and_then(|table| table.glyph_index(code as u32))
                .map(|gid| gid.0)
                .unwrap_or(0)
        } else {
            if win_ansi(code) != character {
                return Err(invalid(
                    "The font encoding disagrees with its Unicode mapping.",
                ));
            }
            character
                .and_then(|character| face.glyph_index(character))
                .map(|gid| gid.0)
                .unwrap_or(0)
        };
        if gid == 0 {
            continue;
        }
        let character = character.ok_or_else(|| {
            invalid("The font maps a visible glyph to an invalid Unicode scalar.")
        })?;
        if gid >= face.number_of_glyphs() {
            return Err(invalid("The embedded font references a missing glyph."));
        }
        if cid
            && face
                .glyph_index(character)
                .is_some_and(|expected| expected.0 != gid)
        {
            return Err(invalid(
                "The font Unicode mapping disagrees with its embedded glyph mapping.",
            ));
        }
        let actual = face
            .glyph_hor_advance(ttf_parser::GlyphId(gid))
            .ok_or_else(|| invalid("The embedded glyph has no horizontal advance."))?
            as f32
            * 1000.0
            / face.units_per_em() as f32;
        let width = widths
            .get(&code)
            .copied()
            .or_else(|| {
                cid.then(|| {
                    child
                        .get(b"DW")
                        .ok()
                        .and_then(|value| value.as_float().ok())
                        .unwrap_or(1000.0)
                })
            })
            .ok_or_else(|| invalid("The embedded glyph has no PDF width."))?;
        if (actual - width).abs() > 1.01 {
            return Err(invalid(
                "The PDF glyph width disagrees with the embedded TrueType program.",
            ));
        }
        if !latin_character(character) {
            continue;
        }
        if characters.insert(character, (code, gid, width)).is_some() {
            return Err(invalid("The font has ambiguous Unicode mappings: multiple character codes select the same character."));
        }
    }
    if characters.is_empty() {
        return Err(invalid(
            "The embedded font has no verified Latin characters.",
        ));
    }
    Ok(VerifiedFont {
        characters,
        code_len: if cid { 2 } else { 1 },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Stream};

    fn bytes(pdf: &mut Document) -> Vec<u8> {
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn shared_program_references_and_inline_descriptors_have_aggregate_limits() {
        let mut pdf = Document::with_version("1.7");
        let mut data = Stream::new(dictionary! {}, vec![1u8; 1024 * 1024]);
        data.compress().unwrap();
        let shared = pdf.add_object(data);
        for _ in 0..1024 {
            pdf.add_object(dictionary! {"FontDescriptor"=>dictionary!{"FontFile3"=>shared}});
        }
        assert!(FontProofs::read(&bytes(&mut pdf)).is_ok());
        for _ in 0..64 {
            let data = pdf.add_object(Stream::new(dictionary! {}, vec![1u8]));
            pdf.add_object(dictionary! {"FontDescriptor"=>dictionary!{"FontFile3"=>data}});
        }
        assert!(FontProofs::read(&bytes(&mut pdf))
            .err()
            .unwrap()
            .to_string()
            .contains("Too many embedded font programs"));

        let mut pdf = Document::with_version("1.7");
        let mut data = Stream::new(dictionary! {}, vec![1u8; MAX_FONT + 1]);
        data.compress().unwrap();
        let data = pdf.add_object(data);
        pdf.add_object(
            dictionary! {"Subtype"=>"Type1","FontDescriptor"=>dictionary!{"FontFile"=>data}},
        );
        assert!(FontProofs::read(&bytes(&mut pdf)).is_err());
    }

    #[test]
    fn repeated_codespaces_cannot_trigger_quadratic_unicode_validation() {
        let mut value = String::from("65536 begincodespacerange\n");
        for _ in 0..65535 {
            value.push_str("<0000> <0000>\n");
        }
        value.push_str(
            "<0000> <FFFF>\nendcodespacerange\n1 beginbfrange <0000> <FFFF> <0000> endbfrange",
        );
        let error = cmap(value.as_bytes(), 2).unwrap_err().to_string();
        assert!(error.contains("codespaces overlap"), "{error}");
    }
}
