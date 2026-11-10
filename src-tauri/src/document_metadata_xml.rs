//! Bounded selective XMP preservation for recomposed document metadata.
use crate::{EngineError, EngineResult};
use quick_xml::{events::Event, Reader};
use std::{borrow::Cow, ops::Range};

pub(crate) const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_EVENTS: u32 = 131_072;
const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";

fn bad() -> EngineError {
    EngineError::InvalidRequest("Document metadata is malformed, uses an unsupported encoding, or exceeds preservation limits.".into())
}
fn require(ok: bool) -> EngineResult<()> { if ok { Ok(()) } else { Err(bad()) } }

#[derive(Clone, Copy, PartialEq, Eq)]
enum Encoding { Utf8, Utf16Le, Utf16Be, Utf32Le, Utf32Be }
impl Encoding {
    fn matches(self, value: &str) -> bool {
        match self {
            Self::Utf8 => value.eq_ignore_ascii_case("UTF-8"),
            Self::Utf16Le => value.eq_ignore_ascii_case("UTF-16") || value.eq_ignore_ascii_case("UTF-16LE"),
            Self::Utf16Be => value.eq_ignore_ascii_case("UTF-16") || value.eq_ignore_ascii_case("UTF-16BE"),
            Self::Utf32Le => value.eq_ignore_ascii_case("UTF-32") || value.eq_ignore_ascii_case("UTF-32LE"),
            Self::Utf32Be => value.eq_ignore_ascii_case("UTF-32") || value.eq_ignore_ascii_case("UTF-32BE"),
        }
    }
}

fn decode<'a>(bytes: &'a [u8], check: &mut impl FnMut() -> EngineResult<()>) -> EngineResult<(Cow<'a, str>, Encoding)> {
    check()?;
    require(bytes.len() <= MAX_BYTES)?;
    let (encoding, offset) = if bytes.starts_with(&[0xff, 0xfe, 0, 0]) { (Encoding::Utf32Le, 4) }
        else if bytes.starts_with(&[0, 0, 0xfe, 0xff]) { (Encoding::Utf32Be, 4) }
        else if bytes.starts_with(&[0xff, 0xfe]) { (Encoding::Utf16Le, 2) }
        else if bytes.starts_with(&[0xfe, 0xff]) { (Encoding::Utf16Be, 2) }
        else if bytes.starts_with(&[0, 0, 0, b'<']) { (Encoding::Utf32Be, 0) }
        else if bytes.starts_with(&[b'<', 0, 0, 0]) { (Encoding::Utf32Le, 0) }
        else if bytes.starts_with(&[0, b'<']) { (Encoding::Utf16Be, 0) }
        else if bytes.starts_with(&[b'<', 0]) { (Encoding::Utf16Le, 0) }
        else { (Encoding::Utf8, 0) };
    if encoding == Encoding::Utf8 {
        return Ok((Cow::Borrowed(std::str::from_utf8(bytes).map_err(|_| bad())?), encoding));
    }
    let bytes = &bytes[offset..];
    let mut text = String::new();
    let mut append = |index: usize, character: char| -> EngineResult<()> {
        if index % 1024 == 0 { check()?; }
        require(text.len() + character.len_utf8() <= MAX_BYTES)?;
        text.push(character);
        Ok(())
    };
    if matches!(encoding, Encoding::Utf16Le | Encoding::Utf16Be) {
        require(bytes.len() % 2 == 0)?;
        let words = bytes.chunks_exact(2).map(|pair| if encoding == Encoding::Utf16Le {
            u16::from_le_bytes([pair[0], pair[1]])
        } else { u16::from_be_bytes([pair[0], pair[1]]) });
        for (index, character) in char::decode_utf16(words).enumerate() { append(index, character.map_err(|_| bad())?)?; }
    } else {
        require(bytes.len() % 4 == 0)?;
        for (index, word) in bytes.chunks_exact(4).enumerate() {
            let word = [word[0], word[1], word[2], word[3]];
            let value = if encoding == Encoding::Utf32Le { u32::from_le_bytes(word) } else { u32::from_be_bytes(word) };
            append(index, char::from_u32(value).ok_or_else(bad)?)?;
        }
    }
    Ok((Cow::Owned(text), encoding))
}

/// No DOM or namespace expansion until the borrowed token stream has bounded
/// depth, names, attributes and namespace declarations. Duplicate-attribute
/// checks in the DOM parser are quadratic per element, hence the local cap too.
fn preflight(text: &str, encoding: Encoding, check: &mut impl FnMut() -> EngineResult<()>) -> EngineResult<Option<Range<usize>>> {
    check()?;
    let mut reader = Reader::from_str(text);
    reader.config_mut().check_comments = true;
    let (mut depth, mut events, mut attributes, mut namespaces) = (0usize, 0u32, 0usize, 0usize);
    let mut declaration = None;
    loop {
        check()?;
        events += 1;
        require(events <= MAX_EVENTS)?;
        let start = reader.buffer_position() as usize;
        let event = reader.read_event().map_err(|_| bad())?;
        match &event {
            Event::Start(element) | Event::Empty(element) => {
                require(depth < 64 && element.name().0.len() <= 1024)?;
                let mut local = 0usize;
                // Strict duplicate expanded-name validation follows in roxmltree.
                for attribute in element.attributes().with_checks(false) {
                    check()?;
                    let attribute = attribute.map_err(|_| bad())?;
                    local += 1;
                    attributes += 1;
                    require(local <= 256 && attributes <= 32_768 && attribute.key.0.len() <= 1024)?;
                    if attribute.key.0 == "xmlns" || attribute.key.0.starts_with("xmlns:") {
                        namespaces += 1;
                        require(namespaces <= 256)?;
                    }
                }
                if matches!(event, Event::Start(_)) { depth += 1; }
            }
            Event::End(_) => { depth = depth.checked_sub(1).ok_or_else(bad)?; }
            Event::Decl(value) => {
                require(declaration.is_none() && value.version().map_err(|_| bad())? == "1.0")?;
                if let Some(value) = value.encoding() { require(encoding.matches(&value.map_err(|_| bad())?))?; }
                if let Some(value) = value.standalone() { require(matches!(value.map_err(|_| bad())?.as_ref(), "yes" | "no"))?; }
                declaration = Some(start..reader.buffer_position() as usize);
            }
            Event::DocType(_) => return Err(bad()),
            Event::Eof => { require(depth == 0)?; break; }
            _ => {},
        }
    }
    Ok(declaration)
}

fn claim(namespace: Option<&str>, name: &str) -> bool {
    matches!(namespace, Some("http://www.aiim.org/pdfa/ns/id/"
        | "http://www.aiim.org/pdfua/ns/id/" | "http://www.npes.org/pdfx/ns/id/"
        | "http://www.npes.org/pdfvt/ns/id/"))
        || (namespace == Some("http://ns.adobe.com/pdfx/1.3/") && matches!(name, "GTS_PDFXVersion" | "GTS_PDFXConformance"))
}
fn parse(text: &str) -> EngineResult<roxmltree::Document<'_>> {
    let document = roxmltree::Document::parse_with_options(text, roxmltree::ParsingOptions {
        allow_dtd: false, nodes_limit: MAX_EVENTS, entity_resolver: None,
    }).map_err(|_| bad())?;
    let root = document.root_element().tag_name();
    require((root.namespace() == Some("adobe:ns:meta/") && matches!(root.name(), "xmpmeta" | "xapmeta"))
        || (root.namespace() == Some(RDF) && root.name() == "RDF"))?;
    Ok(document)
}

/// Validated Unicode text for explicit property editing. Wide declarations are
/// removed before UTF-8 publication; unchanged preservation still uses filter.
pub(crate) fn normalized(bytes:&[u8],check:&mut impl FnMut()->EngineResult<()>)->EngineResult<String>{
    let (text,encoding)=decode(bytes,check)?;
    let declaration=preflight(&text,encoding,check)?;
    parse(&text)?;check()?;
    let mut result=text.into_owned();
    if encoding!=Encoding::Utf8 {if let Some(range)=declaration{result.replace_range(range,"");}}
    Ok(result)
}

/// None means retain the original packet (including byte encoding and stream
/// dictionary). Some is a filtered UTF-8 packet. Ordinary/custom properties are
/// never reserialized through a whitelist or a different RDF representation.
pub(crate) fn filter(bytes: &[u8], check: &mut impl FnMut() -> EngineResult<()>) -> EngineResult<Option<Vec<u8>>> {
    let (text, encoding) = decode(bytes, check)?;
    let declaration = preflight(&text, encoding, check)?;
    check()?;
    let document = parse(&text)?;
    let mut removed = Vec::new();
    for node in document.descendants().filter(|node| node.is_element()) {
        check()?;
        let tag = node.tag_name();
        if claim(tag.namespace(), tag.name()) { removed.push(node.range()); }
        for attribute in node.attributes() {
            check()?;
            if claim(attribute.namespace(), attribute.name()) { removed.push(attribute.range()); }
        }
    }
    if removed.is_empty() { check()?; return Ok(None); }
    // Removing the whole declaration avoids leaving a UTF-16/32 assertion in
    // UTF-8 output; it does not alter the surrounding packet or RDF properties.
    if encoding != Encoding::Utf8 { if let Some(range) = declaration { removed.push(range); } }
    removed.sort_unstable_by_key(|range| (range.start, range.end));
    let mut output = String::with_capacity(text.len());
    let mut end = 0;
    for range in removed {
        check()?;
        require(range.start <= range.end && range.end <= text.len()
            && text.is_char_boundary(range.start) && text.is_char_boundary(range.end))?;
        if range.start > end { output.push_str(&text[end..range.start]); }
        end = end.max(range.end);
    }
    output.push_str(&text[end..]);
    drop(document);
    preflight(&output, Encoding::Utf8, check)?;
    check()?;
    let validated = parse(&output)?;
    for node in validated.descendants().filter(|node| node.is_element()) {
        check()?;
        require(!claim(node.tag_name().namespace(), node.tag_name().name()))?;
        for attribute in node.attributes() { require(!claim(attribute.namespace(), attribute.name()))?; }
    }
    check()?;
    Ok(Some(output.into_bytes()))
}

#[cfg(test)]
#[path = "document_metadata_xml_tests.rs"]
mod tests;
