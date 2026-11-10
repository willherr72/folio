//! Exact annotation identities across relocation and occurrence copying.
//! These receipts do not authorize edits to annotation fields or their consumers.
#![allow(dead_code)]
use crate::{EngineError,EngineResult};
use lopdf::{Document,Object,ObjectId};
use std::collections::{BTreeMap,BTreeSet};

#[path="composition_annotation_groups.rs"]
mod groups;
pub(crate) use groups::bind_source_groups;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) struct SourceSlot {pub page_index:usize,pub slot:usize,pub annotation_id:Option<ObjectId>}
pub(crate) struct SourceGroup<'a> {pub annotation:SourceSlot,pub popup_slots:&'a [usize]}
#[derive(Debug,PartialEq,Eq)]
pub(crate) struct GroupAnchors {pub annotation:Anchor,pub popups:Vec<Anchor>}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) struct Anchor {pub page:ObjectId,pub slot:usize,pub annotation:ObjectId}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) struct Receipt {pub origin:Anchor,pub output_page:ObjectId,pub output_slot:usize,pub annotation:ObjectId}
#[derive(Clone,Copy,Debug,PartialEq,Eq,PartialOrd,Ord)]
pub(crate) enum Container {Page(ObjectId),Array(ObjectId)}
pub(crate) type DirectSlot=(Container,usize);
pub(crate) struct SourceBindings {
    pub anchors:Vec<Anchor>,pub materialized:BTreeMap<DirectSlot,ObjectId>,pub metadata_remaining:usize,
}
fn bad(message:impl std::fmt::Display)->EngineError {EngineError::InvalidRequest(format!("PDF annotation correspondence: {message}"))}
fn require(ok:bool,message:&str)->EngineResult<()> {if ok{Ok(())}else{Err(bad(message))}}
struct Work {steps:usize,metadata:usize}
impl Default for Work {fn default()->Self{Self{steps:0,metadata:16*1024*1024}}}
impl Work {
    fn step(&mut self,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<()> {
        check()?;self.steps=self.steps.checked_add(1).filter(|v|*v<=1_000_000).ok_or_else(||bad("traversal limit"))?;Ok(())
    }
    fn reserve(&mut self,count:usize,size:usize)->EngineResult<()> {
        self.metadata=self.metadata.checked_sub(count.checked_mul(size).ok_or_else(||bad("metadata overflow"))?).ok_or_else(||bad("metadata limit"))?;Ok(())
    }
    fn resolve<'a>(&mut self,doc:&'a Document,mut value:&'a Object,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<(Option<ObjectId>,&'a Object)> {
        let mut id=None;
        for _ in 0..=64 {self.step(check)?;match value {
            Object::Reference(next)=>{id=Some(*next);value=doc.objects.get(next).ok_or_else(||bad("missing reference"))?;},
            _=>return Ok((id,value)),
        }}Err(bad("alias depth or cycle"))
    }
}
fn annotations<'a>(doc:&'a Document,page:ObjectId,work:&mut Work,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<&'a [Object]> {
    work.step(check)?;
    let page=doc.objects.get(&page).ok_or_else(||bad("missing canonical page"))?.as_dict().map_err(bad)?;
    match page.get(b"Annots") {
        Err(_)=>Ok(&[]),
        Ok(value)=>match work.resolve(doc,value,check)?.1 {Object::Null=>Ok(&[]),Object::Array(values)=>{require(values.len()<=100_000,"annotation count limit")?;Ok(values)},_=>Err(bad("invalid annotation list"))},
    }
}
fn page_owner(doc:&Document,value:&Object,page:ObjectId,work:&mut Work,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<()> {
    let dict=value.as_dict().map_err(bad)?;
    if let Ok(value)=dict.get(b"P") {
        let (id,value)=work.resolve(doc,value,check)?;
        require(matches!(value,Object::Null)||id==Some(page),"annotation page owner differs")?;
    }
    Ok(())
}
/// Source canonical IDs come from verified import lineage. The relocation map
/// must be the one created by importing those exact source bytes.
pub(crate) fn bind_source(doc:&Document,pages:&[ObjectId],ids:&BTreeMap<ObjectId,ObjectId>,slots:&[SourceSlot],normalized:&BTreeMap<DirectSlot,ObjectId>,metadata:usize,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<SourceBindings> {
    bind_source_inner(doc,pages,ids,slots,normalized,&mut Work{metadata,..Work::default()},check)
}
fn bind_source_inner(doc:&Document,pages:&[ObjectId],ids:&BTreeMap<ObjectId,ObjectId>,slots:&[SourceSlot],normalized:&BTreeMap<DirectSlot,ObjectId>,work:&mut Work,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<SourceBindings> {
    work.step(check)?;work.reserve(slots.len(),192)?;
    let mut seen=BTreeSet::new();let mut output=Vec::with_capacity(slots.len());
    let mut materialized=BTreeMap::new();
    let base=doc.max_id.max(doc.objects.keys().next_back().map_or(0,|id|id.0));
    for slot in slots {
        work.step(check)?;
        require(seen.insert((slot.page_index,slot.slot)),"duplicate source slot")?;
        let page=*pages.get(slot.page_index).ok_or_else(||bad("source page out of range"))?;
        let raw=doc.get_dictionary(page).map_err(bad)?.get(b"Annots").map_err(bad)?;
        let (array_id,array)=work.resolve(doc,raw,check)?;
        let list=array.as_array().map_err(bad)?;
        require(list.len()<=100_000,"annotation count limit")?;
        let container=array_id.map_or(Container::Page(page),Container::Array);
        let location=(container,slot.slot);
        let value=list.get(slot.slot).ok_or_else(||bad("source annotation slot out of range"))?;
        let (canonical,value)=work.resolve(doc,value,check)?;
        page_owner(doc,value,page,work,check)?;
        let annotation=if let Some(original)=slot.annotation_id {
            let annotation=*ids.get(&original).ok_or_else(||bad("unmapped source identity"))?;
            require(canonical==Some(annotation),"source slot and canonical identity differ")?;
            annotation
        }else if let Some(canonical)=canonical {
            require(normalized.get(&location)==Some(&canonical),"direct source slot changed identity")?;
            canonical
        }else {
            require(!normalized.contains_key(&location),"materialized slot reverted to a direct value")?;
            if let Some(id)=materialized.get(&location){*id}else{
                work.reserve(1,128)?;
                let offset=u32::try_from(materialized.len()).map_err(bad)?.checked_add(1).ok_or_else(||bad("identity overflow"))?;
                let id=(base.checked_add(offset).ok_or_else(||bad("identity overflow"))?,0);
                materialized.insert(location,id);id
            }
        };
        output.push(Anchor{page,slot:slot.slot,annotation});
    }
    work.step(check)?;Ok(SourceBindings{anchors:output,materialized,metadata_remaining:work.metadata})
}

/// Tracks identities, not exclusive ownership: an original annotation can be
/// shared by several pages. Mutation still needs a typed inbound-consumer proof.
pub(crate) struct Tracker {pages:BTreeMap<ObjectId,Vec<Anchor>>,pending:Vec<Receipt>,work:Work}
impl Tracker {
    pub(crate) fn new(doc:&Document,anchors:&[Anchor],check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Self> {
        let mut work=Work::default();work.step(check)?;work.reserve(anchors.len(),256)?;
        let catalog=doc.trailer.get(b"Root").and_then(Object::as_reference).map_err(bad)?;
        let page_ids=crate::composition_pages::inspect(doc,catalog,check)?.pages;
        work.reserve(page_ids.len(),64)?;
        let available=page_ids.into_iter().collect::<BTreeSet<_>>();
        let mut pages=BTreeMap::<ObjectId,Vec<Anchor>>::new();let mut seen=BTreeSet::new();
        for anchor in anchors {
            work.step(check)?;
            require(available.contains(&anchor.page),"annotation page is not a source occurrence")?;
            require(seen.insert((anchor.page,anchor.annotation)),"repeated annotation identity on one page")?;
            let list=annotations(doc,anchor.page,&mut work,check)?;
            let value=list.get(anchor.slot).ok_or_else(||bad("annotation slot out of range"))?;
            let (canonical,value)=work.resolve(doc,value,check)?;
            require(canonical==Some(anchor.annotation),"annotation anchor is stale")?;
            page_owner(doc,value,anchor.page,&mut work,check)?;
            pages.entry(anchor.page).or_default().push(*anchor);
        }
        work.step(check)?;Ok(Self{pages,pending:vec![],work})
    }
    pub(crate) fn record(&mut self,page:ObjectId,output_page:ObjectId,map:Option<&BTreeMap<ObjectId,ObjectId>>,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<()> {
        self.work.step(check)?;
        let count=self.pages.get(&page).map_or(0,Vec::len);self.work.reserve(count,160)?;
        for index in 0..count {
            self.work.step(check)?;let origin=self.pages[&page][index];
            let annotation=match map {None=>origin.annotation,Some(map)=>*map.get(&origin.annotation).ok_or_else(||bad("copy omitted a tracked annotation identity"))?};
            self.pending.push(Receipt{origin,output_page,output_slot:usize::MAX,annotation});
        }Ok(())
    }
    pub(crate) fn finish(mut self,doc:&Document,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Vec<Receipt>> {
        self.work.step(check)?;self.work.reserve(self.pending.len(),192)?;
        let mut by_page=BTreeMap::<ObjectId,BTreeMap<ObjectId,usize>>::new();
        for (index,receipt) in self.pending.iter().enumerate() {
            self.work.step(check)?;
            require(by_page.entry(receipt.output_page).or_default().insert(receipt.annotation,index).is_none(),"duplicate output identity")?;
        }
        for (page,wanted) in by_page {
            let list=annotations(doc,page,&mut self.work,check)?;let mut found=BTreeSet::new();
            for (slot,value) in list.iter().enumerate() {
                let (canonical,value)=self.work.resolve(doc,value,check)?;
                if let Some(id)=canonical {if let Some(index)=wanted.get(&id) {
                    require(found.insert(id),"tracked annotation occurs twice in its output list")?;
                    page_owner(doc,value,page,&mut self.work,check)?;
                    self.pending[*index].output_slot=slot;
                }}
            }
            require(found.len()==wanted.len(),"tracked annotation disappeared from output")?;
        }
        self.work.step(check)?;Ok(self.pending)
    }
}

#[cfg(test)]
#[path="composition_annotation_receipts_tests.rs"]
mod tests;
