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

// Match FPDFFont_GetBaseFontName: embedded simple fonts discard their subset
// prefix; composite fonts expose the descendant's name, including its prefix.
// A program hash alone cannot identify a PDF resource's character encoding.
pub(super) fn renderer_name(pdf: &Document, font: &Dictionary) -> EngineResult<String> {
    let cid = font.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Type0");
    let resource = if cid {
        let children = array(pdf, field(font, b"DescendantFonts")?)?;
        if children.len() != 1 { return Err(invalid("The composite font name is ambiguous.")); }
        dict(pdf, &children[0])?
    } else { font };
    let name = pdf.dereference(field(resource, b"BaseFont")?)
        .ok().and_then(|(_, v)| v.as_name().ok())
        .and_then(|v| std::str::from_utf8(v).ok())
        .ok_or_else(|| invalid("The embedded font name cannot be identified safely."))?;
    Ok(if cid { name } else { base_name(name) }.to_owned())
}

pub(super) fn matches_name(pdf: &Document, font: &Dictionary, native: &str, embedded: bool) -> bool {
    if embedded {
        renderer_name(pdf, font).is_ok_and(|name| name == native)
    } else {
        font.get(b"BaseFont").ok().and_then(|v| pdf.dereference(v).ok())
            .and_then(|(_, v)| v.as_name().ok()).and_then(|v| std::str::from_utf8(v).ok())
            .is_some_and(|name| base_name(name) == base_name(native))
    }
}

/// Reuse only an exact verified full-font program, mapping and integral advances.
pub(super) fn matches_asset_resource(pdf:&Document,font:&Dictionary,asset:&crate::fonts::FontAsset,text:&str)->EngineResult<bool>{
    let bytes=program(pdf,font)?;
    if key(&bytes)!=asset.info.id{return Ok(false);}
    let proof=verify(pdf,font,&bytes)?;
    for ch in text.chars(){
        let s=ch.to_string();let encoded=s.encode_utf16().flat_map(u16::to_be_bytes).collect::<Vec<_>>();
        if proof.encode(&s)?!=encoded||proof.glyph(ch)!=Some(asset.glyph_id(ch)?)||proof.advance(&s)?!=(asset.advance(ch)?*1000.).round()/1000.{return Ok(false);}
    }
    Ok(true)
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
    // A partial ToUnicode repertoire is safe only with an exact source operand
    // proof. Native extraction may omit an excluded code or expand a ligature.
    pub(super) requires_source_operands: bool,
    // Historical Folio maps need our proved CID encoder; native regeneration
    // may choose a different reverse mapping for their old cross-byte range.
    pub(super) preserve_source_encoding: bool,
    pub(super) display_name: Option<String>,
}

impl VerifiedFont {
    pub(super) fn sample(&self)->Option<char> {self.characters.keys().copied().find(|c|!c.is_whitespace())}
    pub(super) fn glyph(&self,c:char)->Option<u16> {self.characters.get(&c).map(|v|v.1)}
    pub(super) fn advance(&self, text: &str) -> EngineResult<f32> {
        self.supports(text)?;
        Ok(text.chars().map(|c| self.characters[&c].2 / 1000.0).sum())
    }
    pub(super) fn decode(&self, bytes: &[u8]) -> EngineResult<String> {
        if bytes.len() % self.code_len != 0 {
            return Err(invalid("A paragraph character code is incomplete."));
        }
        bytes
            .chunks(self.code_len)
            .map(|chunk| {
                let code = if self.code_len == 1 {
                    chunk[0] as u16
                } else {
                    u16::from_be_bytes([chunk[0], chunk[1]])
                };
                let mut matches = self.characters.iter().filter(|(_, value)| value.0 == code);
                let c = matches.next().map(|(c, _)| *c).ok_or_else(|| {
                    invalid("A paragraph character has no verified Unicode mapping.")
                })?;
                if matches.next().is_some() {
                    return Err(invalid(
                        "A paragraph character has ambiguous Unicode mappings.",
                    ));
                }
                Ok(c)
            })
            .collect()
    }
    pub(super) fn single_byte(&self) -> bool {
        self.code_len == 1
    }

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
            let name = renderer_name(&pdf, font)?;
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
                program == hash && candidate == name
            })
            .flat_map(|(_, entries)| entries.iter());
        let mut first = entries
            .next()
            .ok_or("The embedded TrueType program or its Unicode mapping could not be verified.")?
            .clone()?;
        let mut requires_source_operands = first.requires_source_operands;
        let mut preserve_source_encoding = first.preserve_source_encoding;
        for entry in entries {
            let other = entry.clone()?;
            if other.characters != first.characters || other.code_len != first.code_len
                || other.display_name != first.display_name
            {
                return Err("The same embedded program has ambiguous PDF font mappings; it cannot be edited safely.".into());
            }
            requires_source_operands |= other.requires_source_operands;
            preserve_source_encoding |= other.preserve_source_encoding;
        }
        // Resource representation can add safety requirements without changing
        // its proved encoding. Never let object order select the weaker route.
        if first.requires_source_operands != requires_source_operands
            || first.preserve_source_encoding != preserve_source_encoding
        {
            let merged = Arc::make_mut(&mut first);
            merged.requires_source_operands = requires_source_operands;
            merged.preserve_source_encoding = preserve_source_encoding;
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
#[cfg(test)]
#[path = "embedded_cmap_tests.rs"]
mod cmap_tests;

fn unicode(bytes: &[u8]) -> EngineResult<char> {
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum CMapPolicy {
    Complete,
    LatinEditing,
}

// PDF ToUnicode ranges increment only the final encoded byte. Do not invent
// scalar arithmetic, carry into an earlier byte, or repair invalid UTF-16.
fn destination(value: &Object, offset: u16, policy: CMapPolicy) -> EngineResult<Option<char>> {
    let bytes = value.as_str()
        .map_err(|_| invalid("The font Unicode mapping contains a non-string destination."))?;
    let character = if matches!(bytes.len(), 2 | 4) {
        let mut encoded = [0u8; 4];
        encoded[..bytes.len()].copy_from_slice(bytes);
        u8::try_from(offset).ok()
            .and_then(|offset| bytes[bytes.len() - 1].checked_add(offset))
            .and_then(|last| {
                encoded[bytes.len() - 1] = last;
                unicode(&encoded[..bytes.len()]).ok()
            })
    } else {
        None
    };
    if character.is_none() && policy == CMapPolicy::Complete {
        return Err(invalid("The font Unicode destination is unavailable: it is not one valid UTF-16 character or its range overflows."));
    }
    Ok(character)
}

// Parse the data sections ourselves: the general PDF CMap parser builds an
// unbounded reverse map and overwrites overlapping definitions. Both behaviors
// are unsuitable for positively proving an untrusted font's unique mapping.
#[cfg(test)]
fn cmap(bytes: &[u8], code_len: usize) -> EngineResult<BTreeMap<u16, u32>> {
    Ok(cmap_with_policy(bytes, code_len, CMapPolicy::Complete)?
        .into_iter().filter_map(|(code, character)| character.map(|c| (code, c as u32)))
        .collect())
}

fn cmap_with_policy(bytes: &[u8], code_len: usize, policy: CMapPolicy) -> EngineResult<BTreeMap<u16, Option<char>>> {
    if bytes.len() > MAX_CMAP {
        return Err(invalid("The embedded font ToUnicode map exceeds its size limit."));
    }
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
                    if name != "endbfrange" {
                        return Err(invalid("Only font Unicode ranges may have array destinations."));
                    }
                    if values.len() != usize::from(last - first) + 1 {
                        return Err(invalid("The font Unicode mapping array is incomplete."));
                    }
                }
                for code in first..=last {
                    let character = if let Object::Array(values) = target {
                        destination(&values[usize::from(code - first)], 0, policy)?
                    } else {
                        destination(target, code - first, policy)?
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

/// Exact represented source codes and PDF widths for structural removal.
pub(crate) fn redaction_font_codes(pdf:&Document,font:&Dictionary)->EngineResult<(usize,BTreeMap<u16,(char,f32)>)>{
    let bytes=program(pdf,font)?;
    let proof=verify(pdf,font,&bytes)?;
    let mut codes=BTreeMap::new();
    for (ch,(code,_,width)) in proof.characters {if codes.insert(code,(ch,width)).is_some(){return Err(invalid("The source character code is ambiguous."));}}
    Ok((proof.code_len,codes))
}

fn verify(pdf: &Document, font: &Dictionary, bytes: &[u8]) -> EngineResult<VerifiedFont> {
    verify_domain(pdf, font, bytes, false)
}

/// Separate from the Latin editing policy: prove the resource's represented
/// scalars, not merely the characters allowed by the paragraph editor.
pub(crate) fn choice_font_codes(
    pdf: &Document,
    font: &Dictionary,
) -> EngineResult<(usize, BTreeMap<char, u16>)> {
    let bytes = program(pdf, font)?;
    let proof = verify_domain(pdf, font, &bytes, true)?;
    Ok((proof.code_len, proof.characters.into_iter().map(|(c, v)| (c, v.0)).collect()))
}

fn verify_domain(pdf: &Document, font: &Dictionary, bytes: &[u8], all_scalars: bool) -> EngineResult<VerifiedFont> {
    let face = ttf_parser::Face::parse(bytes, 0)
        .map_err(|_| invalid("The embedded TrueType program is malformed."))?;
    if all_scalars {
        crate::fonts::bounded_cmap_coverage(
            face.raw_face().table(ttf_parser::Tag::from_bytes(b"cmap"))
                .ok_or_else(|| invalid("The embedded font has no glyph character map."))?,
        )?;
    }
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
        && encoding.and_then(|value| super::equivalent_encoding::name(pdf, value)) != Some(b"WinAnsiEncoding")
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
    let widths = widths(pdf, child, cid)?;
    let mut requires_source_operands = false;
    let mut preserve_source_encoding = false;
    let unicode: BTreeMap<u16, u32> = if let Ok(value) = font.get(b"ToUnicode") {
        let policy = if all_scalars { CMapPolicy::Complete } else { CMapPolicy::LatinEditing };
        // Excluded entries have already participated in all structural checks.
        // They never enter VerifiedFont, so exact source decoding and replacement
        // encoding cannot gain authority from native extraction alone.
        let unicode_bytes = stream(pdf, value, MAX_CMAP)?;
        let map = if !all_scalars && cid
            && super::persistence::legacy_encoding_matches(pdf, font, bytes, &unicode_bytes)
        {
            preserve_source_encoding = true;
            // Known historical Folio resources encode CIDs as Unicode. This
            // exception proves the whole writer resource, not just its name or
            // malformed range, and leaves complete form repertoire proof strict.
            (0..=u16::MAX).filter_map(|code| char::from_u32(code as u32).map(|c| (code, Some(c)))).collect()
        } else {
            cmap_with_policy(&unicode_bytes, if cid { 2 } else { 1 }, policy)?
        };
        requires_source_operands = map.values().any(Option::is_none);
        map.into_iter().filter_map(|(code, character)| character.map(|c| (code, c as u32)))
            .collect()
    } else if !cid && !legacy {
        // An explicit WinAnsi encoding supplies the character semantics even
        // when a printer omits ToUnicode. The embedded Unicode cmap and PDF
        // widths must still independently prove every represented glyph below.
        let descriptor = dict(pdf, field(child, b"FontDescriptor")?)?;
        let flags = field(descriptor, b"Flags")?.as_i64().unwrap_or(0);
        if flags & 32 == 0 || flags & 4 != 0 {
            return Err(invalid(
                "A font without ToUnicode must explicitly declare a nonsymbolic WinAnsi encoding.",
            ));
        }
        widths
            .keys()
            .filter_map(|&code| {
                win_ansi(code)
                    .filter(|c| all_scalars || latin_character(*c))
                    .map(|c| (code, c as u32))
            })
            .collect()
    } else {
        return Err(invalid(
            "The embedded font has no ToUnicode mapping; its characters cannot be verified safely.",
        ));
    };
    let glyph_map = if cid {
        match child.get(b"CIDToGIDMap") {
            // The Type2 CIDFont default is Identity. Keep complete form proof's
            // existing explicit-map requirement, and never default a bad value.
            Err(_) if !all_scalars => None,
            Err(_) => return Err(invalid("The embedded font is missing a required mapping or metric.")),
            Ok(value) if value.as_name().ok() == Some(b"Identity") => None,
            Ok(value) => {
                let bytes = stream(pdf, value, 65536 * 2)?;
                if bytes.len() % 2 != 0 {
                    return Err(invalid("The CID glyph mapping is incomplete."));
                }
                Some(bytes)
            }
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
        // The legacy editing policy checks WinAnsi's nominal Unicode map.
        // Choice repertoire proof also checks CID Unicode subtable agreement;
        // the explicit CIDToGIDMap must still agree with its selected glyph below.
        // The legacy subset path has exactly one verified Macintosh charmap.
        if let Some(character) =
            character.filter(|character| (all_scalars || !cid) && !legacy && (all_scalars || latin_character(*character)))
        {
            let mut agreed: Option<Option<ttf_parser::GlyphId>> = None;
            for table in maps.subtables.into_iter().filter(|table| {
                table.is_unicode()
                    && !matches!(
                        table.format,
                        ttf_parser::cmap::Format::UnicodeVariationSequences(_)
                    )
            }) {
                // Absence outside a subtable format's representable scalar
                // range is not contradictory evidence. Keep checking every
                // capable Unicode subtable, including missing glyphs within
                // that range. This does not change the old Latin policy.
                if all_scalars {
                    use ttf_parser::cmap::Format;
                    let maximum = match table.format {
                        Format::ByteEncodingTable(_) => 0xff,
                        Format::HighByteMappingThroughTable(_)
                        | Format::SegmentMappingToDeltaValues(_)
                        | Format::TrimmedTableMapping(_) => 0xffff,
                        _ => 0x10ffff,
                    };
                    if character as u32 > maximum { continue; }
                }
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
        let width = if let Some(width) = widths.get(&code) {
            *width
        } else if cid {
            match child.get(b"DW") {
                Ok(value) => value.as_float().map_err(|_| invalid("The embedded default glyph width is malformed."))?,
                Err(_) => 1000.0,
            }
        } else {
            return Err(invalid("The embedded glyph has no PDF width."));
        };
        // Some producers leave zero placeholders for unused WinAnsi slots. With
        // no explicit Unicode assertion, editing may use the remaining proven
        // repertoire. Never invent an advance or allow this code in source or
        // replacement text. Explicit maps and form repertoire proof stay strict.
        if !all_scalars && !cid && !legacy && !font.has(b"ToUnicode")
            && width == 0.0 && actual > 1.01
        {
            continue;
        }
        if !width.is_finite() || !(0.0..=10000.0).contains(&width) || (actual - width).abs() > 1.01 {
            return Err(invalid(
                "The PDF glyph width disagrees with the embedded TrueType program.",
            ));
        }
        if !all_scalars && !latin_character(character) {
            continue;
        }
        if characters.insert(character, (code, gid, width)).is_some() {
            return Err(invalid("The font has ambiguous Unicode mappings: multiple character codes select the same character."));
        }
    }
    if characters.is_empty() {
        return Err(invalid(
            if all_scalars { "The embedded font has no verified characters." } else { "The embedded font has no verified Latin characters." },
        ));
    }
    Ok(VerifiedFont {
        characters,
        code_len: if cid { 2 } else { 1 },
        requires_source_operands,
        preserve_source_encoding,
        display_name: crate::fonts::font_display_name(&face),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Stream};

    #[test]
    fn sparse_winansi_keeps_only_verified_editing_codes() {
        let bytes = include_bytes!("../../tests/fixtures/embedded-text/DejaVuSerif-subset.ttf");
        let face = ttf_parser::Face::parse(bytes, 0).unwrap();
        let width = face.glyph_hor_advance(face.glyph_index('O').unwrap()).unwrap() as f32
            * 1000.0 / face.units_per_em() as f32;
        assert!(face.glyph_hor_advance(face.glyph_index('l').unwrap()).unwrap() > 0);
        let mut widths = vec![Object::Integer(0); 108 - 79 + 1];
        widths[0] = Object::Real(width);
        let font = dictionary! {"Subtype"=>"TrueType", "Encoding"=>"WinAnsiEncoding",
            "FirstChar"=>79, "LastChar"=>108, "Widths"=>widths,
            "FontDescriptor"=>dictionary!{"Flags"=>32}};
        let mut pdf = Document::with_version("1.7");
        let proof = verify(&pdf, &font, bytes).unwrap();
        assert_eq!(proof.encode("O").unwrap(), b"O");
        assert_eq!(proof.decode(b"O").unwrap(), "O");
        assert!(proof.encode("l").is_err());
        assert!(proof.decode(b"l").is_err());
        // Form repertoire proof has a broader contract and must not inherit this
        // editing-only exclusion of unproven codes.
        assert!(verify_domain(&pdf, &font, bytes, true).is_err());
        let mut explicit = font.clone();
        let cmap = pdf.add_object(Stream::new(dictionary! {},
            b"1 begincodespacerange <00> <FF> endcodespacerange 2 beginbfchar <4F> <004F> <6C> <006C> endbfchar".to_vec()));
        explicit.set("ToUnicode", cmap);
        assert!(verify(&pdf, &explicit, bytes).unwrap_err().to_string().contains("width disagrees"));
        let mut nonzero = font.clone();
        nonzero.get_mut(b"Widths").unwrap().as_array_mut().unwrap()[108 - 79] = 1.into();
        assert!(verify(&pdf, &nonzero, bytes).unwrap_err().to_string().contains("width disagrees"));
    }

    #[test]
    fn winansi_without_unicode_requires_flags_and_width_evidence() {
        let bytes = include_bytes!("../../tests/fixtures/embedded-text/DejaVuSerif-subset.ttf");
        let face = ttf_parser::Face::parse(bytes, 0).unwrap();
        let width = face
            .glyph_hor_advance(face.glyph_index('O').unwrap())
            .unwrap() as f32
            * 1000.0
            / face.units_per_em() as f32;
        let mut pdf = Document::with_version("1.7");
        let font = dictionary! {"Subtype"=>"TrueType","Encoding"=>"WinAnsiEncoding","FirstChar"=>79,"LastChar"=>79,"Widths"=>vec![Object::Real(width)],"FontDescriptor"=>dictionary!{"Flags"=>32}};
        assert!(verify(&pdf, &font, bytes).unwrap().supports("O").is_ok());
        for flags in [
            None,
            Some(Object::Integer(0)),
            Some(Object::Integer(4)),
            Some(Object::Integer(36)),
            Some(Object::Real(32.0)),
            Some(Object::Name(b"invalid".to_vec())),
        ] {
            let mut bad = font.clone();
            let descriptor = bad
                .get_mut(b"FontDescriptor")
                .unwrap()
                .as_dict_mut()
                .unwrap();
            if let Some(flags) = flags {
                descriptor.set("Flags", flags);
            } else {
                descriptor.remove(b"Flags");
            }
            assert!(verify(&pdf, &bad, bytes).is_err());
        }
        let mut bad = font.clone();
        bad.set("Widths", vec![Object::Real(width + 10.0)]);
        assert!(verify(&pdf, &bad, bytes)
            .unwrap_err()
            .to_string()
            .contains("width disagrees"));
        let broken = pdf.add_object(Stream::new(dictionary! {}, b"not a cmap".to_vec()));
        let mut bad = font.clone();
        bad.set("ToUnicode", broken);
        assert!(verify(&pdf, &bad, bytes).is_err());
    }

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
        let error = cmap_with_policy(value.as_bytes(), 2, CMapPolicy::LatinEditing).unwrap_err().to_string();
        assert!(error.contains("codespaces overlap"), "{error}");
    }
}
