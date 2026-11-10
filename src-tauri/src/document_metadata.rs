//! Private, bounded document-level metadata attachment.
use crate::{EngineError, EngineResult};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use std::collections::BTreeMap;

pub(crate) struct Prepared {
    pub metadata: Option<Object>,
    pub info: Option<Object>,
    objects: BTreeMap<ObjectId, Object>,
    max_id: u32,
}
impl Prepared {
    pub(crate) fn install(self, document: &mut Document) -> (Option<Object>, Option<Object>) {
        document.max_id = document.max_id.max(self.max_id);
        document.objects.extend(self.objects);
        (self.metadata, self.info)
    }
}
fn bad() -> EngineError {
    EngineError::InvalidRequest("Document metadata cannot be preserved within the supported PDF structure and size limits.".into())
}
fn require(ok: bool) -> EngineResult<()> { if ok { Ok(()) } else { Err(bad()) } }
pub(crate) fn base(document: &Document) -> u32 {
    document.max_id.max(document.objects.keys().next_back().map_or(0, |id| id.0))
}

/// Apply the leading source's metadata to a plain page-import Save. The visual
/// and deliberate-redaction routes must not invoke this preservation contract.
pub(crate) fn preserve_imported(document: &mut Document, source: &Document,
    check: &mut impl FnMut() -> EngineResult<()>) -> EngineResult<bool> {
    let root = document.trailer.get(b"Root").and_then(Object::as_reference).map_err(|_| bad())?;
    let catalog = document.get_dictionary(root).map_err(|_| bad())?;
    let had_metadata = catalog.has(b"Metadata") || document.trailer.has(b"Info");
    let original = source.trailer.get(b"Root").and_then(Object::as_reference).map_err(|_| bad())?;
    let prepared = prepare(source, original, source.trailer.get(b"Info").ok(), base(document), true, check)?;
    let changed = had_metadata || prepared.metadata.is_some() || prepared.info.is_some();
    check()?;
    let (metadata, info) = prepared.install(document);
    let catalog = document.get_dictionary_mut(root).map_err(|_| bad())?;
    if let Some(metadata) = metadata { catalog.set("Metadata", metadata); } else { catalog.remove(b"Metadata"); }
    if let Some(info) = info { document.trailer.set("Info", info); } else { document.trailer.remove(b"Info"); }
    Ok(changed)
}

struct Copy<'a, 'c> {
    source: &'a Document,
    import: bool,
    check: &'c mut dyn FnMut() -> EngineResult<()>,
    remaining: usize,
    steps: usize,
    ids: BTreeMap<ObjectId, ObjectId>,
    prepared: Prepared,
}
impl<'a> Copy<'a, '_> {
    fn step(&mut self, depth: usize) -> EngineResult<()> {
        (self.check)()?;
        self.steps += 1;
        require(depth <= 64 && self.steps <= 32_768)
    }
    fn charge(&mut self, bytes: usize) -> EngineResult<()> {
        self.remaining = self.remaining.checked_sub(bytes).ok_or_else(bad)?;
        Ok(())
    }
    fn id(&mut self) -> EngineResult<ObjectId> {
        self.charge(128)?;
        self.prepared.max_id = self.prepared.max_id.checked_add(1).ok_or_else(bad)?;
        Ok((self.prepared.max_id, 0))
    }
    fn add(&mut self, value: Object) -> EngineResult<Object> {
        let id = self.id()?;
        self.prepared.objects.insert(id, value);
        Ok(Object::Reference(id))
    }
    fn resolve<'v>(&mut self, mut value: &'v Object) -> EngineResult<&'v Object> where 'a: 'v {
        for depth in 0..=64 {
            self.step(depth)?;
            match value {
                Object::Reference(id) => value = self.source.objects.get(id).ok_or_else(bad)?,
                _ => return Ok(value),
            }
        }
        Err(bad())
    }
    fn dictionary(&mut self, dict: &Dictionary, skip: &[&[u8]], depth: usize) -> EngineResult<Dictionary> {
        self.step(depth)?;
        let mut result = Dictionary::new();
        for (key, value) in dict.iter() {
            self.step(depth)?;
            if skip.contains(&key.as_slice()) { continue; }
            self.charge(64 + key.len())?;
            result.set(key.clone(), self.value(value, depth + 1)?);
        }
        Ok(result)
    }
    fn value(&mut self, value: &Object, depth: usize) -> EngineResult<Object> {
        self.step(depth)?;
        self.charge(32)?;
        Ok(match value {
            Object::Reference(id) => {
                require(id.0 != 0 && self.source.objects.contains_key(id))?;
                if !self.import { return Ok(Object::Reference(*id)); }
                if let Some(new) = self.ids.get(id) { return Ok(Object::Reference(*new)); }
                let new = self.id()?;
                self.ids.insert(*id, new);
                let source = &self.source.objects[id];
                let copied = self.value(source, depth + 1)?;
                self.prepared.objects.insert(new, copied);
                Object::Reference(new)
            }
            Object::Name(bytes) => { self.charge(bytes.len())?; Object::Name(bytes.clone()) }
            Object::String(bytes, format) => { self.charge(bytes.len())?; Object::String(bytes.clone(), *format) }
            Object::Array(values) => {
                self.charge(values.len().checked_mul(32).ok_or_else(bad)?)?;
                let mut result = Vec::with_capacity(values.len());
                for value in values { result.push(self.value(value, depth + 1)?); }
                Object::Array(result)
            }
            Object::Dictionary(dict) => Object::Dictionary(self.dictionary(dict, &[], depth + 1)?),
            Object::Stream(stream) => {
                self.charge(stream.content.len())?;
                let dict = self.dictionary(&stream.dict, &[], depth + 1)?;
                Object::Stream(Stream { dict, content: stream.content.clone(),
                    allows_compression: stream.allows_compression, start_position: None })
            }
            Object::Real(value) => { require(value.is_finite())?; Object::Real(*value) }
            _ => value.clone(),
        })
    }
}

/// Stage everything before modifying the destination. `import=false` retains
/// existing graph identities; `import=true` relocates the small dependency graph
/// into a different document. Callers install immediately, using its current
/// `base()`; IDs may not be allocated between prepare and install.
pub(crate) fn prepare(source: &Document, catalog: ObjectId, info: Option<&Object>, base: u32,
    import: bool, check: &mut impl FnMut() -> EngineResult<()>) -> EngineResult<Prepared> {
    check()?;
    let mut copy = Copy { source, import, check, remaining: 8 * 1024 * 1024, steps: 0,
        ids: BTreeMap::new(), prepared: Prepared { metadata: None, info: None, objects: BTreeMap::new(), max_id: base } };
    let catalog = source.objects.get(&catalog).ok_or_else(bad)?.as_dict().map_err(|_| bad())?;
    if let Ok(value) = catalog.get(b"Metadata") {
        let resolved = copy.resolve(value)?;
        if !matches!(resolved, Object::Null) {
            require(matches!(value, Object::Reference(_)))?;
            let stream = resolved.as_stream().map_err(|_| bad())?;
            // External file descriptors change which bytes a PDF consumer reads.
            // Inline decoding cannot prove or preserve that metadata payload.
            for key in [b"F".as_slice(), b"FFilter", b"FDecodeParms"] {
                if let Ok(value) = stream.dict.get(key) { require(matches!(copy.resolve(value)?, Object::Null))?; }
            }
            if let Ok(kind) = stream.dict.get(b"Type") { require(copy.resolve(kind)?.as_name().map_err(|_| bad())? == b"Metadata")?; }
            require(copy.resolve(stream.dict.get(b"Subtype").map_err(|_| bad())?)?.as_name().map_err(|_| bad())? == b"XML")?;
            require(stream.content.len() <= crate::document_metadata_xml::MAX_BYTES)?;
            (copy.check)()?;
            let decoded = stream.decompressed_content_strict_with_limit(crate::document_metadata_xml::MAX_BYTES).map_err(|_| bad())?;
            let filtered = crate::document_metadata_xml::filter(&decoded, &mut || (copy.check)())?;
            drop(decoded);
            copy.prepared.metadata = Some(if let Some(bytes) = filtered {
                copy.charge(bytes.len())?;
                let dict = copy.dictionary(&stream.dict, &[b"Length", b"Filter", b"DecodeParms"], 0)?;
                // A private stream does not change other consumers of the source
                // metadata identity, including custom page/object references.
                copy.add(Object::Stream(Stream::new(dict, bytes)))?
            } else { copy.value(value, 0)? });
        }
    }
    if let Some(value) = info {
        let resolved = copy.resolve(value)?;
        if !matches!(resolved, Object::Null) {
            require(matches!(value, Object::Reference(_)))?;
            let dict = resolved.as_dict().map_err(|_| bad())?;
            copy.prepared.info = Some(if dict.has(b"GTS_PDFXVersion") || dict.has(b"GTS_PDFXConformance") {
                let dict = copy.dictionary(dict, &[b"GTS_PDFXVersion", b"GTS_PDFXConformance"], 0)?;
                copy.add(Object::Dictionary(dict))?
            } else { copy.value(value, 0)? });
        }
    }
    (copy.check)()?;
    Ok(copy.prepared)
}
#[cfg(test)]
#[path = "document_metadata_tests.rs"]
mod tests;
