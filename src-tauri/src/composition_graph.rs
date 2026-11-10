//! Source-object relocation for the forthcoming graph-native composition route.
//! Uses opt-in bounded parsing; not a catalog merger or publishing path.
#![allow(dead_code)] // Route integration follows catalog/lineage validation.

use crate::{EngineError, EngineResult};
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Usage {
    objects: usize,
    values: usize,
    bytes: usize,
}

#[derive(Clone, Copy)]
struct Limits {
    objects: usize,
    values: usize,
    bytes: usize,
    stream_bytes: usize,
    depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            objects: 100_000,
            values: 1_000_000,
            bytes: 256 * 1024 * 1024,
            stream_bytes: 64 * 1024 * 1024,
            depth: 64,
        }
    }
}

struct ImportedSource {
    ids: BTreeMap<ObjectId, ObjectId>,
    trailer: Dictionary,
    version: String,
}

pub(crate) struct CompositionGraph {
    document: Document,
    sources: BTreeMap<usize, ImportedSource>,
    usage: Usage,
    limits: Limits,
    parser_remaining: lopdf::ResourceLimits,
    normalized_annotations:BTreeMap<crate::composition_annotation_receipts::DirectSlot,ObjectId>,
    annotation_metadata:usize,
}

impl CompositionGraph {
    /// Capture each original catalog once, before assembly adapts its language
    /// and merges catalogs. Page occurrences borrow these bounded typed values;
    /// they must never recover a default from the assembled leading catalog.
    pub(crate) fn source_languages(&self,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<BTreeMap<usize,crate::composition_annotation_additions::SourceLanguage>> {
        self.source_languages_with_budget(16*1024*1024,1_000_000,check)
    }
    fn source_languages_with_budget(&self,bytes:usize,steps:usize,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<BTreeMap<usize,crate::composition_annotation_additions::SourceLanguage>> {
        let mut remaining=steps;
        let mut checkpoint=||{check()?;remaining=remaining.checked_sub(1).ok_or_else(||bad("source language traversal limit"))?;Ok(())};
        checkpoint()?;
        // SourceLanguage admits at most 1024 raw bytes. Reserve that maximum,
        // including map metadata, before any capture or map allocation.
        if self.sources.len().checked_mul(1024+128).is_none_or(|n|n>bytes){return Err(bad("source language metadata limit"));}
        let mut languages=BTreeMap::new();
        for (key,source) in &self.sources {
            checkpoint()?;
            let catalog=source.trailer.get(b"Root").and_then(Object::as_reference).map_err(|error|bad(&error.to_string()))?;
            languages.insert(*key,crate::composition_annotation_additions::SourceLanguage::capture_controlled(&self.document,catalog,&mut checkpoint)?);
        }
        checkpoint()?;Ok(languages)
    }
    pub(crate) fn source_annotation_groups(&mut self,key:usize,groups:&[crate::composition_annotation_receipts::SourceGroup<'_>],check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Vec<crate::composition_annotation_receipts::GroupAnchors>> {
        let pages=self.source_pages(key,check)?;
        let source=self.sources.get(&key).ok_or_else(||bad("unknown assembly source"))?;
        let (prepared,groups)=crate::composition_annotation_receipts::bind_source_groups(&self.document,&pages,&source.ids,groups,&self.normalized_annotations,self.annotation_metadata,check)?;
        self.install_source_annotations(prepared,check)?;
        Ok(groups)
    }
    pub(crate) fn source_annotations(&mut self,key:usize,slots:&[crate::composition_annotation_receipts::SourceSlot],check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Vec<crate::composition_annotation_receipts::Anchor>> {
        let pages=self.source_pages(key,check)?;
        let source=self.sources.get(&key).ok_or_else(||bad("unknown assembly source"))?;
        let prepared=crate::composition_annotation_receipts::bind_source(&self.document,&pages,&source.ids,slots,&self.normalized_annotations,self.annotation_metadata,check)?;
        self.install_source_annotations(prepared,check)
    }
    fn install_source_annotations(&mut self,prepared:crate::composition_annotation_receipts::SourceBindings,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Vec<crate::composition_annotation_receipts::Anchor>> {
        let mut usage=self.usage;
        take(&mut usage.objects,prepared.materialized.len(),self.limits.objects)?;
        // Moving a direct dictionary to an object preserves its existing values
        // and payload bytes; only the replacement reference adds a value.
        take(&mut usage.values,prepared.materialized.len(),self.limits.values)?;
        check()?;
        // All identities, slots and budgets are validated. There are no fallible
        // operations after this boundary. Move values; never clone AP payloads.
        for (&(container,slot),&id) in &prepared.materialized {
            use crate::composition_annotation_receipts::Container;
            let array=match container {
                Container::Page(page)=>self.document.objects.get_mut(&page).unwrap().as_dict_mut().unwrap().get_mut(b"Annots").unwrap(),
                Container::Array(array)=>self.document.objects.get_mut(&array).unwrap(),
            }.as_array_mut().unwrap();
            let value=std::mem::replace(&mut array[slot],Object::Reference(id));
            self.document.objects.insert(id,value);self.document.max_id=self.document.max_id.max(id.0);
        }
        self.normalized_annotations.extend(prepared.materialized);
        self.annotation_metadata=prepared.metadata_remaining;self.usage=usage;
        Ok(prepared.anchors)
    }
    pub(crate) fn source_pages(
        &self,
        source_key: usize,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<Vec<ObjectId>> {
        check()?;
        let source=self.sources.get(&source_key).ok_or_else(||bad("unknown assembly source"))?;
        let catalog=source.trailer.get(b"Root").and_then(Object::as_reference).map_err(|error|bad(&error.to_string()))?;
        Ok(crate::composition_pages::inspect(&self.document,catalog,check)?.pages)
    }

    // Consumes the private accumulator. A refused assembly cannot expose a
    // partially rewritten document or invalidate a reusable source graph.
    pub(crate) fn assemble(
        self,
        order: &[usize],
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<Document> {
        self.assemble_controlled(order,Arc::new(||false),check)
    }

    pub(crate) fn assemble_controlled(
        self,
        order: &[usize],
        cancelled:Arc<dyn Fn()->bool+Send+Sync>,
        check:&mut impl FnMut()->EngineResult<()>,
    )->EngineResult<Document> {
        self.assemble_navigation_controlled(order,crate::composition_catalog::OutlineIntent::Guarded,cancelled,check)
    }

    pub(crate) fn assemble_navigation_controlled(
        self,
        order:&[usize],
        intent:crate::composition_catalog::OutlineIntent,
        cancelled:Arc<dyn Fn()->bool+Send+Sync>,
        check:&mut impl FnMut()->EngineResult<()>,
    )->EngineResult<Document> {
        self.assemble_navigation_metadata_controlled(order,intent,true,cancelled,check)
    }
    pub(crate) fn assemble_navigation_metadata_controlled(
        self, order:&[usize], intent:crate::composition_catalog::OutlineIntent,
        preserve_metadata:bool, cancelled:Arc<dyn Fn()->bool+Send+Sync>,
        check:&mut impl FnMut()->EngineResult<()>,
    )->EngineResult<Document> {
        check()?;
        if order.len()!=self.sources.len() || order.is_empty() {
            return Err(bad("assembly requires every complete source exactly once"));
        }
        let mut seen=std::collections::BTreeSet::new();
        let mut sources=Vec::new();
        for key in order {
            check()?;
            if !seen.insert(*key) {return Err(bad("duplicate assembly source"));}
            let source=self.sources.get(key).ok_or_else(||bad("unknown assembly source"))?;
            if source.trailer.has(b"Encrypt") {return Err(bad("encrypted source requires authorized normalization"));}
            let catalog=source.trailer.get(b"Root").and_then(Object::as_reference).map_err(|error|bad(&error.to_string()))?;
            let info=match source.trailer.get(b"Info") {
                Ok(Object::Reference(id))=>Some(*id),
                Ok(Object::Null)|Err(_)=>None,
                _=>return Err(bad("Info must be an indirect dictionary")),
            };
            sources.push(crate::composition_catalog::Source {catalog,version:source.version.clone(),info});
        }
        let document=crate::composition_catalog::assemble_forms_metadata(self.document,sources,intent,preserve_metadata,check,cancelled)?;
        validate_retained_with_limits(&document,self.limits,check)?;
        Ok(document)
    }

    /// Retain a single complete catalog for a private read view. Unlike assembly,
    /// this does not merge/project pages or interpret form editing semantics.
    pub(crate) fn into_single_source(mut self, check: &mut impl FnMut()->EngineResult<()>) -> EngineResult<Document> {
        check()?;
        if self.sources.len()!=1 {return Err(bad("read view requires one complete source"));}
        let (_,source)=self.sources.pop_first().unwrap();
        if source.trailer.has(b"Encrypt") {return Err(bad("encrypted source requires authorized normalization"));}
        self.document.trailer=source.trailer;
        self.document.version=source.version;
        validate_retained_with_limits(&self.document,self.limits,check)?;
        Ok(self.document)
    }

    pub(crate) fn new() -> Self {
        Self {
            document: Document::with_version("1.7"),
            sources: BTreeMap::new(),
            usage: Usage::default(),
            limits: Limits::default(),
            // Sources are already admitted up to this encoded size on open.
            // Keep that allowance aggregate across imports, independently of
            // the unchanged decoded-data and parsed-object allocation budgets.
            parser_remaining: lopdf::ResourceLimits {
                input_bytes: crate::engine::MAX_SOURCE_BYTES as usize,
                ..Default::default()
            },
            normalized_annotations:BTreeMap::new(),
            annotation_metadata:16*1024*1024,
        }
    }

    pub(crate) fn import_bytes(
        &mut self,
        source_key: usize,
        bytes: &[u8],
        cancelled: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> EngineResult<()> {
        if cancelled() { return Err(bad("cancelled")); }
        if self.sources.contains_key(&source_key) { return Err(bad("source is already registered")); }
        let (source, remaining) = Document::load_mem_with_resource_control(
            bytes, self.parser_remaining, cancelled.clone(),
        ).map_err(|error| bad(&error.to_string()))?;
        self.import(source_key, source, &mut || {
            if cancelled() { Err(bad("cancelled")) } else { Ok(()) }
        })?;
        // Relocation has committed. No subsequent fallible checkpoint may split
        // the graph commit from its associated parser allowance update.
        self.parser_remaining = remaining;
        Ok(())
    }

    fn import(
        &mut self,
        source_key: usize,
        mut source: Document,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<()> {
        check()?;
        if self.sources.contains_key(&source_key) {
            return Err(bad("source is already registered"));
        }

        // These are already-parsed values. The counters bound retained stored
        // payload and graph complexity, not parsing/decompression or total RSS.
        // Work on local counters so every refusal leaves the accumulator intact.
        let mut usage = self.usage;
        take(&mut usage.objects, source.objects.len(), self.limits.objects)?;
        take(&mut usage.bytes, source.version.len(), self.limits.bytes)?;
        let mut scan = Scan { source: &source, limits: self.limits, usage: &mut usage, check };
        scan.node(0)?;
        scan.dictionary(&source.trailer, 0)?;
        for (id, value) in &source.objects {
            if id.0 == 0 { return Err(bad("object zero cannot own a value")); }
            scan.value(value, 0)?;
        }

        // Allocate all identities before visiting references. Never dereference:
        // cycles, aliases and forward references describe the original graph.
        let base = self.document.max_id.max(
            self.document.objects.keys().next_back().map_or(0, |id| id.0)
        );
        let count = u32::try_from(source.objects.len()).map_err(|_| bad("object ID limit"))?;
        let end = base.checked_add(count).ok_or_else(|| bad("object ID limit"))?;
        let mut ids = BTreeMap::new();
        for (index, id) in source.objects.keys().enumerate() {
            check()?;
            ids.insert(*id, (base + index as u32 + 1, 0));
        }

        relocate_dictionary(&mut source.trailer, &ids, check)?;
        for value in source.objects.values_mut() {
            relocate(value, &ids, check)?;
        }
        let mut objects = BTreeMap::new();
        for (id, value) in source.objects {
            check()?;
            objects.insert(ids[&id], value);
        }
        let imported = ImportedSource { ids, trailer: source.trailer, version: source.version };

        // No fallible operation or cancellation point after this commit boundary.
        // Encoded stream Vecs have only moved; no second payload copy is made.
        check()?;
        self.document.objects.append(&mut objects);
        self.document.max_id = end;
        self.sources.insert(source_key, imported);
        self.usage = usage;
        Ok(())
    }
}

/// Recheck after any caller-owned crop, rotation or bookmark mutation, before
/// serialization. Encoded file size alone does not bound retained complexity.
pub(crate) fn validate_retained(document:&Document,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<()> {
    validate_retained_with_limits(document,Limits::default(),check)
}

/// Conservative reservation before copying page-local resource dictionaries.
/// The caller supplies checked upper bounds for newly authored small objects,
/// names and references. Source dictionaries are scanned without cloning.
pub(crate) fn reserve_content_additions(document:&Document,dictionaries:&[&Dictionary],objects:usize,values:usize,bytes:usize,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<()> {
    let limits=Limits::default();let mut usage=Usage::default();
    take(&mut usage.objects,document.objects.len(),limits.objects)?;take(&mut usage.objects,objects,limits.objects)?;
    take(&mut usage.values,values,limits.values)?;take(&mut usage.bytes,bytes,limits.bytes)?;take(&mut usage.bytes,document.version.len(),limits.bytes)?;
    let mut scan=Scan{source:document,limits,usage:&mut usage,check};scan.node(0)?;scan.dictionary(&document.trailer,0)?;
    for value in document.objects.values(){scan.value(value,0)?;}
    for dictionary in dictionaries{scan.node(0)?;scan.dictionary(dictionary,0)?;}
    check()
}

/// Reserve the combined retained graph before an overlay adapter copies page
/// lists or moves generated payloads. Source references are checked in their
/// own namespaces; the caller separately proves and relocates the exact closure.
/// Counting old lists twice is conservative for replacement and also covers
/// shared indirect arrays that must remain intact for another page.
pub(crate) fn validate_overlay_additions(
    document:&Document,generated:&Document,ids:&BTreeSet<ObjectId>,
    lists:&[(&[Object],usize)],check:&mut impl FnMut()->EngineResult<()>,
)->EngineResult<()> {
    validate_overlay_additions_with_limits(document,generated,ids,lists,Limits::default(),check)
}
fn validate_overlay_additions_with_limits(
    document:&Document,generated:&Document,ids:&BTreeSet<ObjectId>,
    lists:&[(&[Object],usize)],limits:Limits,check:&mut impl FnMut()->EngineResult<()>,
)->EngineResult<()> {
    let mut usage=Usage::default();
    take(&mut usage.objects,document.objects.len(),limits.objects)?;
    take(&mut usage.objects,ids.len(),limits.objects)?;
    take(&mut usage.bytes,document.version.len(),limits.bytes)?;
    {
        let mut scan=Scan{source:document,limits,usage:&mut usage,check};
        scan.node(0)?;scan.dictionary(&document.trailer,0)?;
        for value in document.objects.values(){scan.value(value,0)?;}
        for (old,added) in lists {
            scan.node(0)?;
            for value in *old {scan.value(value,1)?;}
            take(&mut scan.usage.values,*added,limits.values)?;
        }
    }
    {
        let mut scan=Scan{source:generated,limits,usage:&mut usage,check};
        for id in ids {
            let value=generated.objects.get(id).ok_or_else(||bad("missing generated object"))?;
            scan.value(value,0)?;
        }
    }
    check()
}
fn validate_retained_with_limits(document:&Document,limits:Limits,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<()> {
    check()?;
    let mut usage=Usage::default();
    take(&mut usage.objects,document.objects.len(),limits.objects)?;
    take(&mut usage.bytes,document.version.len(),limits.bytes)?;
    let mut scan=Scan{source:document,limits,usage:&mut usage,check};
    scan.node(0)?;
    scan.dictionary(&document.trailer,0)?;
    for value in document.objects.values(){scan.value(value,0)?;}
    check()
}

fn bad(message: &str) -> EngineError {
    EngineError::InvalidRequest(format!("PDF composition graph: {message}"))
}

fn take(used: &mut usize, amount: usize, limit: usize) -> EngineResult<()> {
    *used = used.checked_add(amount).filter(|value| *value <= limit)
        .ok_or_else(|| bad("graph exceeds preservation limits"))?;
    Ok(())
}

struct Scan<'a, F> {
    source: &'a Document,
    limits: Limits,
    usage: &'a mut Usage,
    check: &'a mut F,
}

impl<F: FnMut() -> EngineResult<()>> Scan<'_, F> {
    fn node(&mut self, depth: usize) -> EngineResult<()> {
        (self.check)()?;
        if depth > self.limits.depth { return Err(bad("direct object nesting limit")); }
        take(&mut self.usage.values, 1, self.limits.values)
    }

    fn dictionary(&mut self, dictionary: &Dictionary, depth: usize) -> EngineResult<()> {
        for (key, value) in dictionary.iter() {
            take(&mut self.usage.bytes, key.len(), self.limits.bytes)?;
            self.value(value, depth + 1)?;
        }
        Ok(())
    }

    fn value(&mut self, value: &Object, depth: usize) -> EngineResult<()> {
        self.node(depth)?;
        match value {
            Object::Reference(id) => {
                if id.0 == 0 || !self.source.objects.contains_key(id) {
                    return Err(bad("unresolved source reference"));
                }
            }
            Object::Real(value) if !value.is_finite() => return Err(bad("nonfinite value")),
            Object::Name(bytes) | Object::String(bytes, _) => {
                take(&mut self.usage.bytes, bytes.len(), self.limits.bytes)?;
            }
            Object::Array(values) => {
                for value in values { self.value(value, depth + 1)?; }
            }
            Object::Dictionary(dictionary) => self.dictionary(dictionary, depth)?,
            Object::Stream(stream) => {
                if stream.content.len() > self.limits.stream_bytes {
                    return Err(bad("stored stream exceeds preservation limit"));
                }
                take(&mut self.usage.bytes, stream.content.len(), self.limits.bytes)?;
                self.dictionary(&stream.dict, depth)?;
            }
            _ => {}
        }
        Ok(())
    }
}

fn relocate_dictionary(
    dictionary: &mut Dictionary,
    ids: &BTreeMap<ObjectId, ObjectId>,
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<()> {
    for (_, value) in dictionary.iter_mut() { relocate(value, ids, check)?; }
    Ok(())
}

// Only receives the exact objects admitted by Scan, so direct recursion is
// bounded. Indirect references are rewritten without following them.
fn relocate(
    value: &mut Object,
    ids: &BTreeMap<ObjectId, ObjectId>,
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<()> {
    check()?;
    match value {
        Object::Reference(id) => *id = *ids.get(id).ok_or_else(|| bad("unresolved source reference"))?,
        Object::Array(values) => {
            for value in values { relocate(value, ids, check)?; }
        }
        Object::Dictionary(dictionary) => relocate_dictionary(dictionary, ids, check)?,
        Object::Stream(stream) => {
            relocate_dictionary(&mut stream.dict, ids, check)?;
            stream.start_position = None;
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
#[path = "composition_graph_tests.rs"]
mod tests;
