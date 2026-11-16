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

#[path = "embedded_cff.rs"]
mod cff;

#[cfg(test)]
#[path = "embedded_cff_tests.rs"]
mod cff_tests;

#[cfg(test)]
#[path = "cid_font_without_cmap_tests.rs"]
mod cid_font_without_cmap_tests;

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

/// Same unshaped scalar policy as imported fonts. Source fonts still need exact
/// Unicode/code/glyph/width proof; this predicate grants no font authority.
pub(super) fn unshaped(text: &str) -> bool {
    !text.trim().is_empty() && text.len() <= 4096
        && text.chars().all(crate::fonts::allowed_character)
}
/// Candidate syntax only. Each sequence still needs the actual physical glyph,
/// exact source code, width, outline and intrinsic-alias proof below.
fn source_sequence(text: &str) -> bool {
    if text.len()>64 {return false;}
    let count=text.chars().count();
    if !(2..=16).contains(&count) {return false;}
    if latin(text) && unshaped(text) && !text.chars().any(char::is_whitespace) {return true;}
    use unicode_normalization::char::{canonical_combining_class,is_public_assigned};
    let mut chars=text.chars();let first=chars.next().unwrap();
    crate::fonts::allowed_character(first) && first.is_alphabetic()
        && canonical_combining_class(first)==0
        && chars.all(|c|matches!(c as u32,0x300..=0x36f)
            && canonical_combining_class(c)!=0 && is_public_assigned(c))
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
    book: Option<Arc<super::source_glyphs::Codebook>>,
    code_len: usize,
    // CFF outlines already validated during bounded proof, in em units.
    source_ink: Option<Arc<BTreeMap<u16,[f32;4]>>>,
    // A partial ToUnicode repertoire is safe only with an exact source operand
    // proof. Native extraction may omit an excluded code or expand a ligature.
    pub(super) requires_source_operands: bool,
    // Historical Folio maps need our proved CID encoder; native regeneration
    // may choose a different reverse mapping for their old cross-byte range.
    pub(super) preserve_source_encoding: bool,
    pub(super) display_name: Option<String>,
}

impl VerifiedFont {
    /// Call only after binding the actual code/GID and source program identity.
    pub(super) fn source_glyph_ink(&self,gid:u16,size:f32)->EngineResult<Option<[f32;4]>> {
        let Some(ink)=&self.source_ink else{return Ok(None);};
        let value=ink.get(&gid).ok_or_else(||invalid("Missing proved CFF source glyph outline."))?.map(|v|v*size);
        if !size.is_finite() || size<=0. || value.iter().any(|v|!v.is_finite()||v.abs()>100_000.) || value[2]<value[0] || value[3]<value[1] {
            return Err(invalid("Proved CFF glyph ink exceeds paragraph geometry limits."));
        }
        Ok(Some(value))
    }
    // Exact physical widths for a source-bound cursor plan. This lookup grants
    // no resource identity; the caller still checks the actual Tf resource,
    // program hash, static CID TrueType format and publication proof.
    pub(super) fn cursor_glyphs(&self, bytes: &[u8]) -> EngineResult<Vec<(u16,u16,f32)>> {
        if self.code_len!=2 || bytes.is_empty() || bytes.len()>8192 || bytes.len()%2!=0 {
            return Err(invalid("Invalid cursor glyph encoding."));
        }
        if let Some(book)=&self.book {
            let text=book.bind(bytes).map_err(invalid)?;
            return Ok(text.encoded().atoms().iter().map(|a|(a.code,a.gid,a.width)).collect());
        }
        let codes=self.characters.values().map(|(code,gid,width)|(*code,(*gid,*width))).collect::<BTreeMap<_,_>>();
        bytes.chunks_exact(2).map(|b|{
            let code=u16::from_be_bytes([b[0],b[1]]);
            let (gid,width)=codes.get(&code).ok_or_else(||invalid("An exact cursor glyph is unavailable."))?;
            Ok((code,*gid,*width))
        }).collect()
    }
    pub(super) fn bind(&self, bytes: &[u8]) -> EngineResult<Option<Arc<super::source_glyphs::BoundText>>> {
        self.book.as_ref().map(|book|book.bind(bytes).map(Arc::new).map_err(invalid)).transpose()
    }
    pub(super) fn sample(&self)->Option<char> {self.characters.keys().copied().find(|c|!c.is_whitespace())}
    pub(super) fn glyph(&self,c:char)->Option<u16> {self.characters.get(&c).map(|v|v.1)}
    pub(super) fn advance(&self, text: &str) -> EngineResult<f32> {
        if let Some(book)=&self.book {return book.encode(text).map(|text|text.advance()).map_err(invalid);}
        self.supports(text)?;
        Ok(text.chars().map(|c| self.characters[&c].2 / 1000.0).sum())
    }
    pub(super) fn decode(&self, bytes: &[u8]) -> EngineResult<String> {
        if let Some(book)=&self.book {return book.bind(bytes).map(|bound|bound.encoded().text().to_owned()).map_err(invalid);}
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
        if let Some(book)=&self.book {return book.encode(text).map(|text|text.bytes().to_vec()).map_err(invalid);}
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
    /// Only an already-verified source codebook can admit non-scalar text.
    /// This never relaxes imported-font or complete form-font repertoire policy.
    pub(super) fn allows_source_text(&self,text:&str)->bool {
        unshaped(text) || (!text.trim().is_empty() && text.len()<=4096
            && self.book.as_ref().is_some_and(|book|book.encode(text).is_ok()))
    }
    pub(super) fn supports(&self, text: &str) -> EngineResult<()> {
        if let Some(book)=&self.book {return book.encode(text).map(|_|()).map_err(invalid);}
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
            if !matches!(subtype, b"TrueType" | b"Type0") && !(subtype==b"Type1" && cff::uses(&pdf,font)) {
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
            if let Ok(proof)=&verified {
                if let Some(ink)=&proof.source_ink {
                    // Conservative BTree node slack and shared-map header.
                    retained=retained.checked_add(128+ink.len()*128).filter(|n|*n<=128*1024*1024)
                        .ok_or_else(||invalid("The embedded font ink maps exceed their aggregate memory budget."))?;
                }
                if let Some(book)=&proof.book {
                    retained=retained.checked_add(book.retained_bytes()).filter(|n|*n<=128*1024*1024)
                        .ok_or_else(||invalid("The embedded font glyph maps exceed their aggregate memory budget."))?;
                }
            }
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
            if other.characters != first.characters || other.code_len != first.code_len || other.book != first.book
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
        if !matches!(child.get(b"Subtype").and_then(Object::as_name).ok(), Some(b"CIDFontType2"|b"CIDFontType0")) {
            return Err(invalid("Only embedded TrueType outlines can be edited."));
        }
        Ok(child)
    } else {
        Ok(font)
    }
}
pub(super) fn program(pdf: &Document, font: &Dictionary) -> EngineResult<Vec<u8>> {
    if cff::uses(pdf,font) {return cff::program(pdf,font);}
    let child = descendant(pdf, font)?;
    if child.get(b"Subtype").and_then(Object::as_name).ok()==Some(b"CIDFontType0") {
        return Err(invalid("A CFF descendant requires a FontFile3 program."));
    }
    let descriptor = dict(pdf, field(child, b"FontDescriptor")?)?;
    if descriptor.has(b"FontFile3") {
        if !matches!(child.get(b"Subtype").and_then(Object::as_name).ok(),Some(b"TrueType"|b"CIDFontType2"))
            || [b"FontFile".as_slice(),b"FontFile2",b"FontFile3"].iter().filter(|key|descriptor.has(key)).count()!=1 {
            return Err(invalid("The OpenType TrueType resource has conflicting font programs or types."));
        }
        let value=field(descriptor,b"FontFile3")?;
        let object=pdf.dereference(value).map_err(|_|invalid("The OpenType stream is malformed."))?.1.as_stream().map_err(|_|invalid("The OpenType stream is malformed."))?;
        if object.dict.get(b"Subtype").and_then(Object::as_name).ok()!=Some(b"OpenType") {
            return Err(invalid("A TrueType FontFile3 stream must declare OpenType."));
        }
        let bytes=stream(pdf,value,MAX_FONT)?;
        let face=ttf_parser::Face::parse(&bytes,0).map_err(|_|invalid("The OpenType TrueType program is malformed."))?;
        if face.tables().glyf.is_none() || face.tables().hmtx.is_none() || face.tables().cff.is_some()
            || face.raw_face().table(ttf_parser::Tag::from_bytes(b"CFF2")).is_some() || face.is_variable() {
            return Err(invalid("The OpenType TrueType stream has unsupported or conflicting outlines."));
        }
        return Ok(bytes);
    }
    stream(pdf, field(descriptor, b"FontFile2")?, MAX_FONT)
}

#[path = "embedded_cmap_parse.rs"]
mod cmap_parse;
use cmap_parse::*;

#[cfg(test)]
#[path = "embedded_cmap_tests.rs"]
mod cmap_tests;

pub(super) fn widths(pdf: &Document, font: &Dictionary, cid: bool) -> EngineResult<BTreeMap<u16, f32>> {
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
    if cff::uses(pdf,font) {return cff::verify(pdf,font,bytes,all_scalars);}
    let child=descendant(pdf,font)?;
    if child.get(b"FontDescriptor").ok().and_then(|value|dict(pdf,value).ok()).is_some_and(|descriptor|descriptor.has(b"FontFile3"))
        && program(pdf,font)?!=bytes {
        return Err(invalid("The native OpenType program differs from its source resource."));
    }
    if descendant(pdf,font)?.get(b"Subtype").and_then(Object::as_name).ok()==Some(b"CIDFontType0") {
        return Err(invalid("A CFF descendant requires a FontFile3 program."));
    }
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
    let maps = face.tables().cmap;
    // A CIDFontType2 can omit its internal Unicode cmap: Identity-H and the
    // PDF CIDToGIDMap select the glyph, while ToUnicode supplies text semantics.
    // Keep malformed-present tables, simple encodings and full form repertoire
    // proof strict. Newly admitted fonts use exact source operators, never a
    // guessed native Unicode-to-glyph reverse mapping.
    let pdf_mapped_cid = maps.is_none() && cid && !all_scalars
        && child.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"CIDFontType2")
        && face.raw_face().table_records.into_iter()
            .all(|record| record.tag != ttf_parser::Tag::from_bytes(b"cmap"));
    if maps.is_none() && !pdf_mapped_cid {
        return Err(invalid("The embedded font has no glyph character map."));
    }
    if maps.is_some_and(|maps| maps.subtables.len() > 32) {
        return Err(invalid(
            "The embedded font contains too many character maps for safe editing.",
        ));
    }
    if legacy
        && maps.is_none_or(|maps| maps.subtables.len() != 1
            || maps.subtables.get(0).is_none_or(|table| {
                table.platform_id != ttf_parser::PlatformId::Macintosh || table.encoding_id != 0
            }))
    {
        return Err(invalid(
            "The embedded simple font has an ambiguous implicit character encoding.",
        ));
    }
    let widths = widths(pdf, child, cid)?;
    let mut requires_source_operands = pdf_mapped_cid;
    let mut preserve_source_encoding = pdf_mapped_cid;
    let mut sequences=BTreeMap::new();
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
        } else if !all_scalars && cid && child.get(b"Subtype").and_then(Object::as_name).ok()==Some(b"CIDFontType2") {
            source_cmap(&unicode_bytes,2)?.into_iter().map(|(code,value)| {
                let character=match value {
                    SourceMapping::Scalar(c)=>Some(c),
                    SourceMapping::Sequence(text) if source_sequence(&text)=>{
                        sequences.insert(code,text);None
                    },
                    _=>None,
                };
                (code,character)
            }).collect()
        } else {
            cmap_with_policy(&unicode_bytes, if cid { 2 } else { 1 }, policy)?
        };
        requires_source_operands |= map.values().any(Option::is_none);
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
                    .filter(|c| all_scalars || crate::fonts::allowed_character(*c))
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
            character.filter(|character| (all_scalars || !cid) && !legacy && (all_scalars || crate::fonts::allowed_character(*character)))
        {
            let mut agreed: Option<Option<ttf_parser::GlyphId>> = None;
            for table in maps.into_iter().flat_map(|maps| maps.subtables).filter(|table| {
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
            maps.and_then(|maps| maps.subtables.get(0))
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
        let (actual,width)=glyph_width(&face,child,&widths,cid,code,gid)?;
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
        if !all_scalars && !crate::fonts::allowed_character(character) {
            continue;
        }
        if characters.insert(character, (code, gid, width)).is_some() {
            return Err(invalid("The font has ambiguous Unicode mappings: multiple character codes select the same character."));
        }
    }
    // Count declarations before excluding absent/malformed physical glyphs.
    // An invalid duplicate must not grant its surviving alias reverse authority.
    let mut sequence_counts=BTreeMap::new();
    for text in sequences.values() {*sequence_counts.entry(text.clone()).or_insert(0usize)+=1;}
    let mut sequence_symbols=Vec::new();
    for (code,text) in sequences {
        // The PDF CID map chooses the actual glyph. A multi-letter destination
        // has no single Unicode cmap lookup; never substitute its first letter.
        let gid=match &glyph_map {
            Some(bytes)=>bytes.get(code as usize*2..code as usize*2+2)
                .map(|b|u16::from_be_bytes([b[0],b[1]])).unwrap_or(0),
            None=>code,
        };
        if gid==0 {continue;}
        // This entry was unavailable under the older scalar policy. Failure to
        // prove it must keep it unavailable, not revoke unrelated scalar codes.
        // Exact source decoding still refuses any run that uses that code.
        if gid>=face.number_of_glyphs() {continue;}
        let Some(bounds)=face.glyph_bounding_box(ttf_parser::GlyphId(gid))else{continue;};
        if bounds.x_max<=bounds.x_min || bounds.y_max<=bounds.y_min {continue;}
        let Ok((actual,width))=glyph_width(&face,child,&widths,true,code,gid)else{continue;};
        if !actual.is_finite() || actual<=0. || !width.is_finite() || !(0.0..=10000.).contains(&width)
            || width<=0. || (actual-width).abs()>1.01 {continue;}
        sequence_symbols.push(super::source_glyphs::Symbol{code,text,gid,width});
    }
    // Exclude every code in an ambiguous sequence equivalence class. Never let
    // definition order choose a reverse mapping, and retain the scalar domain.
    sequence_symbols.retain(|symbol|sequence_counts[&symbol.text]==1);
    let intrinsic=sequence_aliases(&face,&sequence_symbols);
    let mut ordinal=0;
    sequence_symbols.retain(|_|{let keep=intrinsic[ordinal];ordinal+=1;keep});
    let book=if sequence_symbols.is_empty() {None}else{
        requires_source_operands=true;preserve_source_encoding=true;
        sequence_symbols.extend(characters.iter().map(|(ch,(code,gid,width))|super::source_glyphs::Symbol {
            code:*code,text:ch.to_string(),gid:*gid,width:*width,
        }));
        Some(Arc::new(super::source_glyphs::Codebook::new(sequence_symbols,2).map_err(invalid)?))
    };
    if characters.is_empty() && book.is_none() {
        return Err(invalid(
            if all_scalars { "The embedded font has no verified characters." } else { "The embedded font has no verified unshaped characters." },
        ));
    }
    Ok(VerifiedFont {
        characters,
        book,
        source_ink: None,
        code_len: if cid { 2 } else { 1 },
        requires_source_operands,
        preserve_source_encoding,
        display_name: crate::fonts::font_display_name(&face),
    })
}

fn glyph_width(face:&ttf_parser::Face<'_>,font:&Dictionary,widths:&BTreeMap<u16,f32>,cid:bool,code:u16,gid:u16)->EngineResult<(f32,f32)> {
    let actual=face.glyph_hor_advance(ttf_parser::GlyphId(gid))
        .ok_or_else(||invalid("The embedded glyph has no horizontal advance."))? as f32*1000./face.units_per_em()as f32;
    let width=if let Some(width)=widths.get(&code) {*width}else if cid {
        match font.get(b"DW") {
            Ok(value)=>value.as_float().map_err(|_|invalid("The embedded default glyph width is malformed."))?,
            Err(_)=>1000.,
        }
    }else{return Err(invalid("The embedded glyph has no PDF width."));};
    Ok((actual,width))
}

fn sequence_aliases(face:&ttf_parser::Face<'_>,symbols:&[super::source_glyphs::Symbol])->Vec<bool> {
    use unicode_normalization::UnicodeNormalization;
    if symbols.is_empty() {return Vec::new();}
    let Some(maps)=face.tables().cmap else {return vec![true;symbols.len()];};
    // No callback enumerates an unbounded or malformed cmap. Failure to prove
    // alias coverage excludes new sequences while retaining the scalar policy.
    if face.raw_face().table(ttf_parser::Tag::from_bytes(b"cmap"))
        .is_none_or(|bytes|crate::fonts::bounded_cmap_coverage(bytes).is_err()) {
        return vec![false;symbols.len()];
    }
    let wanted:Vec<_>=symbols.iter().map(|symbol|(symbol.gid,symbol.text.nfkd().collect::<String>())).collect();
    let keys:HashSet<_>=wanted.iter().cloned().collect();
    let targets:HashSet<_>=symbols.iter().map(|symbol|symbol.gid).collect();
    let mut seen=HashSet::new();let mut matched=HashSet::new();
    let mut contrary=HashSet::new();let mut intrinsic=HashMap::new();
    for table in maps.subtables.into_iter().filter(|table|table.is_unicode()
        && !matches!(table.format,ttf_parser::cmap::Format::UnicodeVariationSequences(_))) {
        table.codepoints(|code| {
            let Some(character)=char::from_u32(code)else{return;};
            if !unicode_normalization::char::is_public_assigned(character)||character.is_control(){return;}
            let Some(gid)=table.glyph_index(code).filter(|gid|targets.contains(&gid.0))else{return;};
            seen.insert(gid.0);
            if contrary.contains(&gid.0){return;}
            let mut utf8=[0;4];let text=character.encode_utf8(&mut utf8);let mut normalized=text.nfkd();
            let Some(first)=normalized.next()else{return;};
            // One positive alias must never conceal an ordinary or differently
            // normalized alias for the same physical glyph. Retain at most one
            // bounded comparison string per candidate GID, not every cmap entry.
            let Some(second)=normalized.next()else{contrary.insert(gid.0);return;};
            let mut text=first.to_string();text.push(second);text.extend(normalized);
            let key=(gid.0,text);
            if !keys.contains(&key) || intrinsic.get(&gid.0).is_some_and(|value|value!=&key.1) {
                contrary.insert(gid.0);return;
            }
            intrinsic.entry(gid.0).or_insert_with(||key.1.clone());
            // A positive alias cannot borrow a different Unicode subtable's
            // glyph when another actual mapping for that alias disagrees.
            let agrees=maps.subtables.into_iter().filter(|other|other.is_unicode()
                && !matches!(other.format,ttf_parser::cmap::Format::UnicodeVariationSequences(_)))
                .all(|other|other.glyph_index(code).is_none_or(|other_gid|other_gid==gid));
            if agrees {matched.insert(key);}else{contrary.insert(gid.0);}
        });
    }
    // The font may genuinely lack a Unicode alias for a CID-only glyph. Actual
    // CID/GID/width/outline and exact source proofs still apply. Known ordinary
    // 'l' cannot be relabeled 'fi'; known 'ﬁ' can supply the compatible alias.
    // Normalization is comparison evidence only: source bytes/text never change.
    wanted.into_iter().map(|key|!contrary.contains(&key.0) && (!seen.contains(&key.0)||matched.contains(&key))).collect()
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
