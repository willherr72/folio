//! Bounded repertoire and code mapping for one original choice-widget font.
//! This is not a proof of native shaping, layout, or appearance fidelity.
#![allow(dead_code)] // Private foundation; publication integration follows.
use crate::engine::{choice_font_codes, EngineError, EngineResult};
use lopdf::{dictionary, Dictionary, Document, Object};
use std::collections::BTreeMap;

const MAX_TEXT: usize = 4096;
const MAX_TOTAL: usize = 128 * 1024;
fn invalid(message: &str) -> EngineError {
    EngineError::InvalidRequest(message.into())
}

pub(crate) struct ChoiceFontProof {
    code_len: usize,
    encode: BTreeMap<char, u16>,
    decode: BTreeMap<u16, char>,
}

fn text_budget(texts: &[&str]) -> EngineResult<()> {
    if texts.len() > 4001 {
        return Err(invalid("Choice font text count exceeds its limit."));
    }
    let mut total = 0usize;
    for text in texts {
        if text.len() > MAX_TEXT {
            return Err(invalid("Choice font text exceeds its size limit."));
        }
        total += text.len(); // Count and individual limits bound this sum.
        if total > MAX_TOTAL {
            return Err(invalid("Choice font texts exceed their total size limit."));
        }
    }
    Ok(())
}

impl ChoiceFontProof {
    pub(crate) fn for_texts(pdf: &Document, font: &Object, texts: &[&str]) -> EngineResult<Self> {
        text_budget(texts)?;
        let font = pdf
            .dereference(font)
            .and_then(|(_, o)| o.as_dict())
            .map_err(|_| invalid("The choice font dictionary is malformed."))?;
        let proof = match font.get(b"Subtype").and_then(Object::as_name).ok() {
            Some(b"TrueType" | b"Type0") => {
                let (code_len, encode) = choice_font_codes(pdf, font)?;
                let mut decode = BTreeMap::new();
                for (&c, &code) in &encode {
                    if decode.insert(code, c).is_some() {
                        return Err(invalid("The choice font has ambiguous character codes."));
                    }
                }
                Self {
                    code_len,
                    encode,
                    decode,
                }
            }
            Some(b"Type1") => Self::standard(pdf, font)?,
            _ => return Err(invalid("This choice font cannot be verified safely.")),
        };
        proof.supports(texts)?;
        Ok(proof)
    }

    fn standard(pdf: &Document, font: &Dictionary) -> EngineResult<Self> {
        let name = font.get(b"BaseFont").and_then(Object::as_name).ok();
        if !matches!(
            name,
            Some(
                b"Helvetica"
                    | b"Helvetica-Bold"
                    | b"Helvetica-Oblique"
                    | b"Helvetica-BoldOblique"
                    | b"Times-Roman"
                    | b"Times-Bold"
                    | b"Times-Italic"
                    | b"Times-BoldItalic"
                    | b"Courier"
                    | b"Courier-Bold"
                    | b"Courier-Oblique"
                    | b"Courier-BoldOblique"
            )
        ) || font.has(b"ToUnicode")
        {
            return Err(invalid(
                "This choice font is not an unmodified standard Latin font.",
            ));
        }
        if let Ok(descriptor) = font.get(b"FontDescriptor") {
            let descriptor = pdf
                .dereference(descriptor)
                .and_then(|(_, o)| o.as_dict())
                .map_err(|_| invalid("The choice font descriptor is malformed."))?;
            let flags = descriptor
                .get(b"Flags")
                .and_then(Object::as_i64)
                .map_err(|_| invalid("The choice font flags are malformed."))?;
            if flags & 4 != 0
                || flags & 32 == 0
                || [b"FontFile".as_slice(), b"FontFile2", b"FontFile3"]
                    .iter()
                    .any(|k| descriptor.has(k))
            {
                return Err(invalid(
                    "An embedded or symbolic font cannot use the standard choice font proof.",
                ));
            }
        }
        let encoding = match font.get(b"Encoding") {
            Ok(value) => pdf
                .dereference(value)
                .and_then(|(_, o)| o.as_name())
                .map_err(|_| invalid("Custom choice font encodings cannot be verified safely."))?,
            Err(_) => b"StandardEncoding",
        };
        if !matches!(encoding, b"WinAnsiEncoding" | b"StandardEncoding") {
            return Err(invalid(
                "This standard choice font encoding is unsupported.",
            ));
        }
        // Never give the lenient library an untrusted encoding: construct only
        // the whitelisted built-in case, then retain explicit scalar/code maps.
        let normalized =
            dictionary! {"Type" => "Font", "Encoding" => Object::Name(encoding.to_vec())};
        let mapping = normalized
            .get_font_encoding(pdf)
            .map_err(|_| invalid("The standard choice font encoding is unavailable."))?;
        let mut encode = BTreeMap::new();
        let mut decode = BTreeMap::new();
        for code in 0..=255u16 {
            let text = mapping
                .bytes_to_string(&[code as u8])
                .map_err(|_| invalid("The standard choice font encoding is malformed."))?;
            let mut chars = text.chars();
            if let Some(c) = chars.next().filter(|c| !c.is_control() && *c != '\u{fffd}') {
                if chars.next().is_some() {
                    return Err(invalid("The standard choice font mapping is ambiguous."));
                }
                encode.entry(c).or_insert(code);
                decode.insert(code, c);
            }
        }
        Ok(Self {
            code_len: 1,
            encode,
            decode,
        })
    }

    pub(crate) fn supports(&self, texts: &[&str]) -> EngineResult<()> {
        text_budget(texts)?;
        if texts
            .iter()
            .any(|text| text.chars().any(|c| !self.encode.contains_key(&c)))
        {
            return Err(invalid(
                "The original choice font cannot represent every required character.",
            ));
        }
        Ok(())
    }

    pub(crate) fn encode(&self, text: &str) -> EngineResult<Vec<u8>> {
        self.supports(&[text])?;
        let mut output = Vec::with_capacity(text.len() * self.code_len);
        for c in text.chars() {
            let bytes = self.encode[&c].to_be_bytes();
            output.extend_from_slice(&bytes[2 - self.code_len..]);
        }
        Ok(output)
    }

    pub(crate) fn decode(&self, bytes: &[u8]) -> EngineResult<String> {
        if bytes.len() > MAX_TOTAL || bytes.len() % self.code_len != 0 {
            return Err(invalid(
                "Choice character codes exceed their size limit or are incomplete.",
            ));
        }
        let mut output = String::new();
        for chunk in bytes.chunks_exact(self.code_len) {
            let code = if self.code_len == 1 {
                chunk[0] as u16
            } else {
                u16::from_be_bytes([chunk[0], chunk[1]])
            };
            let c = *self.decode.get(&code).ok_or_else(|| {
                invalid("A choice character code has no verified Unicode mapping.")
            })?;
            if output.len() + c.len_utf8() > MAX_TOTAL {
                return Err(invalid("Decoded choice text exceeds its size limit."));
            }
            output.push(c);
        }
        Ok(output)
    }
}
