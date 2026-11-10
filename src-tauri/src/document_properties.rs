//! Explicit document-level metadata drafts, separate from source page identity.
use crate::{EngineError, EngineResult};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const FIELD_LIMIT: usize = 16 * 1024;
pub(crate) fn bad(message: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("Document properties: {message}"))
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "mode", rename_all = "camelCase", deny_unknown_fields)]
pub enum DocumentMetadataEdit {
    Update {
        #[serde(rename = "clearExisting", default)]
        clear_existing: bool,
        title: Option<String>,
        author: Option<String>,
        subject: Option<String>,
        keywords: Option<String>,
    },
    Remove {},
}
impl DocumentMetadataEdit {
    pub(crate) fn clears_existing(&self) -> bool {
        matches!(
            self,
            Self::Remove {}
                | Self::Update {
                    clear_existing: true,
                    ..
                }
        )
    }
    pub(crate) fn fields(&self) -> [Option<&str>; 4] {
        match self {
            Self::Update {
                title,
                author,
                subject,
                keywords,
                ..
            } => [
                title.as_deref(),
                author.as_deref(),
                subject.as_deref(),
                keywords.as_deref(),
            ],
            Self::Remove {} => [None; 4],
        }
    }
    pub(crate) fn validate(&self) -> EngineResult<()> {
        if matches!(self, Self::Remove {}) {
            return Ok(());
        }
        let fields = self.fields();
        if fields.iter().all(Option::is_none) {
            return Err(bad("change a field before applying"));
        }
        for value in fields.into_iter().flatten() {
            if value.len()>FIELD_LIMIT||value.chars().any(|c|!matches!(c,'\t'|'\n'|'\r'|'\u{20}'..='\u{d7ff}'|'\u{e000}'..='\u{fffd}'|'\u{10000}'..='\u{10ffff}')){return Err(bad("use XML-compatible text of at most16KiB per field"))}
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentProperties {
    pub title: String,
    pub author: String,
    pub subject: String,
    pub keywords: String,
    pub creator: String,
    pub producer: String,
    pub creation_date: String,
    pub modification_date: String,
    pub has_info: bool,
    pub has_xmp: bool,
    pub conflicts: Vec<String>,
    pub multiple_authors: bool,
    pub error: Option<String>,
}
const KEYS: [&[u8]; 8] = [
    b"Title",
    b"Author",
    b"Subject",
    b"Keywords",
    b"Creator",
    b"Producer",
    b"CreationDate",
    b"ModDate",
];
const NAMES: [&str; 4] = ["title", "author", "subject", "keywords"];
fn resolve<'a>(document: &'a Document, mut object: &'a Object) -> EngineResult<&'a Object> {
    for _ in 0..64 {
        match object {
            Object::Reference(id) => {
                object = document
                    .objects
                    .get(id)
                    .ok_or_else(|| bad("missing metadata reference"))?
            }
            _ => return Ok(object),
        }
    }
    Err(bad("metadata reference chain exceeds limit"))
}
fn info(document: &Document) -> EngineResult<Option<&Dictionary>> {
    let Some(value) = document.trailer.get(b"Info").ok() else {
        return Ok(None);
    };
    let value = resolve(document, value)?;
    if matches!(value, Object::Null) {
        Ok(None)
    } else {
        Ok(Some(value.as_dict().map_err(bad)?))
    }
}
fn packet(document: &Document) -> EngineResult<Option<(&Stream, Vec<u8>)>> {
    let Some(value) = document.catalog().map_err(bad)?.get(b"Metadata").ok() else {
        return Ok(None);
    };
    let value = resolve(document, value)?;
    if matches!(value, Object::Null) {
        return Ok(None);
    }
    let stream = value.as_stream().map_err(bad)?;
    for key in [b"F".as_slice(), b"FFilter", b"FDecodeParms"] {
        if let Ok(value) = stream.dict.get(key) {
            if !matches!(resolve(document, value)?, Object::Null) {
                return Err(bad("external metadata streams are unsupported"));
            }
        }
    }
    if let Ok(value) = stream.dict.get(b"Type") {
        if resolve(document, value)?.as_name().map_err(bad)? != b"Metadata" {
            return Err(bad("invalid metadata type"));
        }
    }
    if resolve(document, stream.dict.get(b"Subtype").map_err(bad)?)?
        .as_name()
        .map_err(bad)?
        != b"XML"
    {
        return Err(bad("invalid metadata subtype"));
    }
    if stream.content.len() > crate::document_metadata_xml::MAX_BYTES {
        return Err(bad("metadata stream exceeds limit"));
    }
    let bytes = stream
        .decompressed_content_strict_with_limit(crate::document_metadata_xml::MAX_BYTES)
        .map_err(bad)?;
    Ok(Some((stream, bytes)))
}
fn info_text(document: &Document, value: &Object) -> EngineResult<String> {
    let value = resolve(document, value)?;
    let bytes = value.as_str().map_err(bad)?;
    if bytes.len() > FIELD_LIMIT * 2 + 3 {
        return Err(bad("property text exceeds limit"));
    }
    let text = if let Some(bytes) = bytes.strip_prefix(b"\xef\xbb\xbf") {
        std::str::from_utf8(bytes).map_err(bad)?.to_owned()
    } else if let Some(bytes) = bytes.strip_prefix(b"\xfe\xff") {
        if bytes.len() % 2 != 0 {
            return Err(bad("incomplete UTF16 property"));
        }
        String::from_utf16(
            &bytes
                .chunks_exact(2)
                .map(|b| u16::from_be_bytes([b[0], b[1]]))
                .collect::<Vec<_>>(),
        )
        .map_err(bad)?
    } else {
        lopdf::decode_text_string(value).map_err(bad)?
    };
    if text.len() > FIELD_LIMIT {
        return Err(bad("decoded property text exceeds limit"));
    }
    Ok(text)
}
pub(crate) fn inspect(document: &Document) -> EngineResult<DocumentProperties> {
    let mut out = DocumentProperties::default();
    let mut values: [Option<String>; 8] = Default::default();
    let mut issues = Vec::new();
    out.has_info = document
        .trailer
        .get(b"Info")
        .is_ok_and(|v| !matches!(resolve(document, v), Ok(Object::Null)));
    out.has_xmp = document
        .catalog()
        .map_err(bad)?
        .get(b"Metadata")
        .is_ok_and(|v| !matches!(resolve(document, v), Ok(Object::Null)));
    match info(document) {
        Ok(Some(info)) => {
            for (index, key) in KEYS.iter().enumerate() {
                if let Ok(value) = info.get(key) {
                    match info_text(document,value){Ok(value)=>values[index]=Some(value),Err(_)=>issues.push("Some PDF information fields cannot be read within the supported limits.")}
                }
            }
        }
        Ok(None) => {}
        Err(_) => issues.push("The PDF information dictionary is unreadable."),
    }
    match packet(document).and_then(|value|value.map(|(_,bytes)|crate::document_properties_xml::inspect(&bytes)).transpose()){
        Ok(Some(xmp))=>{for index in 0..4{if values[index+4].is_none(){values[index+4]=xmp.details[index].clone();}}out.multiple_authors=xmp.multiple_authors;for index in 0..4{if let Some(value)=xmp.fields[index].as_ref(){if values[index].as_ref().is_some_and(|s|s!=value){out.conflicts.push(NAMES[index].into());}if values[index].is_none(){values[index]=Some(value.clone());}}}},
        Ok(None)=>{},Err(_)=>issues.push("XMP metadata is malformed or uses an unsupported property structure. It can still be removed explicitly."),
    }
    [
        out.title,
        out.author,
        out.subject,
        out.keywords,
        out.creator,
        out.producer,
        out.creation_date,
        out.modification_date,
    ] = values.map(Option::unwrap_or_default);
    if !issues.is_empty() {
        issues.dedup();
        out.error = Some(issues.join(" "));
    }
    Ok(out)
}
fn copy_dictionary(
    dict: &Dictionary,
    skip: &[&[u8]],
    remaining: &mut usize,
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<Dictionary> {
    fn charge(
        object: &Object,
        depth: usize,
        budget: &mut usize,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<()> {
        check()?;
        if depth > 64 {
            return Err(bad("metadata dictionary nesting exceeds limit"));
        }
        let own = match object {
            Object::Name(v) | Object::String(v, _) => v.len(),
            Object::Stream(s) => s.content.len(),
            _ => 0,
        };
        *budget = budget
            .checked_sub(64 + own)
            .ok_or_else(|| bad("metadata dictionary exceeds copy limit"))?;
        match object {
            Object::Array(items) => {
                for item in items {
                    charge(item, depth + 1, budget, check)?
                }
            }
            Object::Dictionary(d) => {
                for (k, v) in d.iter() {
                    *budget = budget
                        .checked_sub(k.len())
                        .ok_or_else(|| bad("metadata dictionary exceeds copy limit"))?;
                    charge(v, depth + 1, budget, check)?
                }
            }
            Object::Stream(s) => {
                for (k, v) in s.dict.iter() {
                    *budget = budget
                        .checked_sub(k.len())
                        .ok_or_else(|| bad("metadata dictionary exceeds copy limit"))?;
                    charge(v, depth + 1, budget, check)?
                }
            }
            _ => {}
        }
        Ok(())
    }
    let mut result = Dictionary::new();
    for (key, value) in dict.iter() {
        check()?;
        if skip.contains(&key.as_slice()) {
            continue;
        }
        *remaining = remaining
            .checked_sub(key.len() + 64)
            .ok_or_else(|| bad("metadata dictionary exceeds copy limit"))?;
        charge(value, 0, remaining, check)?;
        result.set(key.clone(), value.clone());
    }
    Ok(result)
}
/// Compute reachability against staged roots before any mutation. This avoids
/// lopdf's quadratic prune traversal and bounds direct nesting and work.
fn retained(
    document: &Document,
    trailer: &Dictionary,
    staged: &BTreeMap<ObjectId, Object>,
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<BTreeSet<ObjectId>> {
    let mut live = BTreeSet::new();
    let mut pending: Vec<_> = trailer.iter().map(|(_, v)| (v, 0usize)).collect();
    let mut steps = 0usize;
    while let Some((object, depth)) = pending.pop() {
        check()?;
        steps += 1;
        if steps > 2_000_000 || depth > 128 || pending.len() > 1_000_000 {
            return Err(bad("metadata publication graph exceeds traversal limits"));
        }
        match object {
            Object::Reference(id) => {
                if live.insert(*id) {
                    if live.len() > 250_000 {
                        return Err(bad("metadata publication graph exceeds object limit"));
                    }
                    pending.push((
                        staged
                            .get(id)
                            .or_else(|| document.objects.get(id))
                            .ok_or_else(|| bad("missing retained PDF object"))?,
                        0,
                    ));
                }
            }
            Object::Array(items) => {
                if items.len() + pending.len() > 1_000_000 {
                    return Err(bad("metadata publication graph exceeds traversal limits"));
                }
                pending.extend(items.iter().map(|v| (v, depth + 1)));
            }
            Object::Dictionary(d) => {
                if d.len() + pending.len() > 1_000_000 {
                    return Err(bad("metadata publication graph exceeds traversal limits"));
                }
                pending.extend(d.iter().map(|(_, v)| (v, depth + 1)));
            }
            Object::Stream(s) => {
                if s.dict.len() + pending.len() > 1_000_000 {
                    return Err(bad("metadata publication graph exceeds traversal limits"));
                }
                pending.extend(s.dict.iter().map(|(_, v)| (v, depth + 1)));
            }
            _ => {}
        }
    }
    Ok(live)
}
pub(crate) fn apply(
    document: &mut Document,
    edit: &DocumentMetadataEdit,
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<()> {
    edit.validate()?;
    check()?;
    let root = document
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .map_err(bad)?;
    let mut remaining = 8 * 1024 * 1024;
    let mut catalog = copy_dictionary(
        document.get_dictionary(root).map_err(bad)?,
        &[b"Metadata"],
        &mut remaining,
        check,
    )?;
    let mut trailer = copy_dictionary(&document.trailer, &[b"Info"], &mut remaining, check)?;
    let mut staged = BTreeMap::new();
    let mut max_id = crate::document_metadata::base(document);
    if matches!(edit, DocumentMetadataEdit::Update { .. }) {
        let fields = edit.fields();
        let mut skip = vec![b"GTS_PDFXVersion".as_slice(), b"GTS_PDFXConformance"];
        for i in 0..4 {
            if fields[i].is_some() {
                skip.push(KEYS[i]);
            }
        }
        let mut new_info = match if edit.clears_existing() {
            None
        } else {
            info(document)?
        } {
            Some(info) => copy_dictionary(info, &skip, &mut remaining, check)?,
            None => Dictionary::new(),
        };
        for (index, value) in fields.iter().enumerate() {
            if let Some(value) = value {
                new_info.set(KEYS[index].to_vec(), lopdf::text_string(value));
            }
        }
        let packet = if edit.clears_existing() {
            None
        } else {
            packet(document)?
        };
        check()?;
        let bytes = crate::document_properties_xml::update(
            packet.as_ref().map(|(_, b)| b.as_slice()),
            edit,
            check,
        )?;
        let mut dict = match packet {
            Some((stream, _)) => copy_dictionary(
                &stream.dict,
                &[b"Length", b"Filter", b"DecodeParms"],
                &mut remaining,
                check,
            )?,
            None => Dictionary::new(),
        };
        dict.set("Type", "Metadata");
        dict.set("Subtype", "XML");
        max_id = max_id
            .checked_add(1)
            .ok_or_else(|| bad("PDF object identifier overflow"))?;
        let info_id = (max_id, 0);
        staged.insert(info_id, Object::Dictionary(new_info));
        trailer.set("Info", info_id);
        max_id = max_id
            .checked_add(1)
            .ok_or_else(|| bad("PDF object identifier overflow"))?;
        let meta_id = (max_id, 0);
        staged.insert(meta_id, Object::Stream(Stream::new(dict, bytes)));
        catalog.set("Metadata", meta_id);
    }
    staged.insert(root, Object::Dictionary(catalog));
    let live = retained(document, &trailer, &staged, check)?;
    check()?;
    document.objects.extend(staged);
    document.trailer = trailer;
    document.max_id = max_id;
    document.objects.retain(|id, _| live.contains(id));
    Ok(())
}

#[cfg(test)]
#[path = "document_properties_tests.rs"]
mod tests;
