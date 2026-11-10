//! Private occurrence-bound annotation/popup/tag removal.
#![allow(dead_code)]
use crate::{composition_annotation_receipts::Receipt, EngineError, EngineResult};
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::collections::{BTreeMap, BTreeSet};

#[path="composition_annotation_replacement.rs"]
mod replacement;
pub(crate) use replacement::{replace_owned,Replacement};

pub(crate) struct Removal<'a> {
    pub annotation: Receipt,
    pub popups: &'a [Receipt],
}
pub(crate) fn remove_owned(
    document: Document, requests: &[Removal<'_>],
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<Document> {
    remove_with_budget(document, requests, 16*1024*1024, 1_000_000, check)
}

fn bad(message: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("PDF annotation removal: {message}"))
}
fn require(ok: bool, message: &str) -> EngineResult<()> {
    if ok { Ok(()) } else { Err(bad(message)) }
}

/// Receipts must come from the trusted source/copy correspondence. They identify
/// the occurrence; this separate proof classifies its live incoming consumers.
/// The private document is consumed, so refusal never exposes partial mutation.
fn remove_with_budget(
    mut document: Document, requests: &[Removal<'_>], metadata: usize, steps: usize,
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<Document> {
    check()?;
    if requests.is_empty() { return Ok(document); }
    let catalog=document.trailer.get(b"Root").and_then(Object::as_reference).map_err(bad)?;
    let pages=crate::composition_pages::inspect(&document,catalog,check)?.pages;
    let structure=crate::composition_structure::inspect(&document,catalog,&pages,check)?;
    let mut work=Walk{doc:&document,structure:&structure,catalog,check,metadata,steps,pages:BTreeSet::new(),targets:BTreeMap::new(),tag_parents:BTreeMap::new(),memberships:BTreeSet::new(),seen:BTreeSet::new()};
    work.reserve(pages.len(),64)?;
    work.pages.extend(pages.iter().copied());
    let (removed,retained_targets)=work.prepare(requests,&pages)?;
    // Cache by incoming role as well as canonical identity. A list reached by
    // Page.Annots AND opaque metadata must not be accepted only in its first role.
    for (_,value) in document.trailer.iter() {work.visit(value,Role::Unknown,0)?;}

    let tag_selection=if work.tag_parents.is_empty(){None}else{
        work.reserve(work.tag_parents.len(),64)?;
        let excluded=work.tag_parents.keys().copied().collect();
        Some(crate::composition_projection::structure_without_annotations(&document,&structure,&pages,&excluded,&mut work.metadata,&mut work.steps,&mut ||(work.check)())?)
    };
    let mut forbidden=BTreeSet::new();
    work.reserve(work.targets.len(),64)?;
    forbidden.extend(work.targets.keys().filter(|id|!retained_targets.contains(id)).copied());
    if let Some((selection,records))=&tag_selection {
        work.reserve(structure.elements.len()+records.len(),64)?;
        forbidden.extend(structure.elements.difference(&selection.elements).copied());forbidden.extend(records);
    }

    work.reserve(removed.len(),128)?;
    let mut lists=Vec::with_capacity(removed.len());
    for page in removed.keys() {lists.push((work.annotations(*page)?,0));}
    // Page-local arrays may duplicate direct metadata from a shared old array.
    // Reserve total retained storage before making even that bounded copy.
    crate::composition_graph::validate_overlay_additions(&document,&document,&BTreeSet::new(),&lists,&mut ||(work.check)())?;
    let mut patches=Vec::with_capacity(removed.len());
    for (page,slots) in removed {
        let old=work.annotations(page)?;
        work.reserve(old.len(),std::mem::size_of::<Object>())?;
        let mut kept=Vec::with_capacity(old.len()-slots.len());
        for (slot,value) in old.iter().enumerate() {
            work.step(0)?;
            if !slots.contains(&slot) {kept.push(work.copy(value,0)?);}
        }
        patches.push((page,kept));
    }
    work.step(0)?;
    let mut metadata=work.metadata;let mut steps=work.steps;
    drop(work);
    for (page,values) in patches {
        check()?;
        document.get_dictionary_mut(page).map_err(bad)?.set("Annots",values);
    }
    if let Some((selection,_))=tag_selection {
        for ((id,key),value) in selection.fields {
            check()?;
            let dict=document.get_dictionary_mut(id).map_err(bad)?;
            match value {Some(value)=>dict.set(key,value),None=>{dict.remove(&key);}}
        }
    }
    crate::composition_projection::validate_removed_owners(&document,&forbidden,&mut metadata,&mut steps,check)?;
    let actual=crate::composition_structure::inspect(&document,catalog,&pages,check)?;
    require(actual.owners.len()==structure.owners.values().filter(|owner|!forbidden.contains(&owner.object)).count()
        && structure.owners.iter().filter(|(_,owner)|!forbidden.contains(&owner.object)).all(|(key,owner)|actual.owners.get(key)==Some(owner)),"surviving structure owners changed")?;
    // Original dictionaries/resources remain until final reachability pruning.
    // This is an editing operation, not secure erasure or redaction.
    crate::composition_graph::validate_retained(&document,check)?;
    check()?;
    Ok(document)
}

#[derive(Clone,Copy,PartialEq,Eq,PartialOrd,Ord)]
enum Role {
    Unknown,Catalog,Page(ObjectId),Annotations(ObjectId),Slot(ObjectId),Owned(ObjectId),Popup(ObjectId),Parent(ObjectId),
    StructureRoot,Element(ObjectId,Option<ObjectId>),Kids(ObjectId,Option<ObjectId>),Objr(ObjectId,Option<ObjectId>),
    WholeOwner(ObjectId,Option<ObjectId>),StructureParent,NumberTree,ParentValues,
}
struct Walk<'a,'b> {
    doc:&'a Document,check:&'b mut dyn FnMut()->EngineResult<()>,metadata:usize,steps:usize,
    pages:BTreeSet<ObjectId>,targets:BTreeMap<ObjectId,Option<ObjectId>>,
    structure:&'a crate::composition_structure::StructureInfo,catalog:ObjectId,
    tag_parents:BTreeMap<ObjectId,ObjectId>,memberships:BTreeSet<(ObjectId,ObjectId)>,
    seen:BTreeSet<(ObjectId,Role)>,
}
impl<'a> Walk<'a,'_> {
    fn prepare(&mut self,requests:&[Removal<'_>],pages:&[ObjectId])->EngineResult<(BTreeMap<ObjectId,BTreeSet<usize>>,BTreeSet<ObjectId>)> {
    self.reserve(requests.len(),192)?;
    let mut removed=BTreeMap::<ObjectId,BTreeSet<usize>>::new();
    for request in requests {
        self.step(0)?;
        let root=request.annotation;
        let dict=self.binding(root,false)?;
        self.target(root.annotation,None)?;
        self.reserve(1,128)?;
        require(removed.entry(root.output_page).or_default().insert(root.output_slot),"duplicate removal occurrence")?;
        let forward=self.optional_ref(dict,b"Popup")?;
        let mut popup_ids=BTreeSet::new();
        self.reserve(request.popups.len(),192)?;
        for popup in request.popups {
            self.step(0)?;
            require(popup.output_page==root.output_page && popup.origin.page==root.origin.page,"popup belongs to another occurrence")?;
            require(popup_ids.insert(popup.annotation),"repeated popup identity")?;
            let dict=self.binding(*popup,true)?;
            let parent=self.optional_ref(dict,b"Parent")?;
            require(parent==Some(root.annotation) || (parent.is_none() && forward==Some(popup.annotation)),"popup ownership differs")?;
            self.target(popup.annotation,Some(root.annotation))?;
            require(removed.entry(root.output_page).or_default().insert(popup.output_slot),"overlapping popup occurrence")?;
        }
        require(forward.is_none_or(|id|popup_ids.contains(&id)),"owned popup was omitted")?;
    }
    // Check every live membership, including other pages sharing a source root.
    // A repeated identity on one page has no unambiguous slot-edit contract yet.
    let mut retained_targets=BTreeSet::new();
    for page in pages {
        let mut seen=BTreeSet::new();
        for (slot,value) in self.annotations(*page)?.iter().enumerate() {
            let (id,value)=self.resolve(value)?;
            if let Some(id)=id.filter(|id|self.targets.contains_key(id)) {
                self.reserve(1,192)?;
                require(seen.insert(id),"repeated owned identity on one page")?;
                self.page_owner(value.as_dict().map_err(bad)?,*page)?;
                self.memberships.insert((id,*page));
                if !removed.get(page).is_some_and(|slots|slots.contains(&slot)) {retained_targets.insert(id);}
            }
        }
    }
    for owner in self.structure.owners.values() {
        self.step(0)?;
        if self.targets.contains_key(&owner.object) {
            let crate::composition_structure::Parents::Whole(parent)=owner.parents else{return Err(bad("annotation has a marked-content owner"))};
            require(!retained_targets.contains(&owner.object),"shared tagged source annotation requires occurrence-specific tag isolation")?;
            self.reserve(1,128)?;self.tag_parents.insert(owner.object,parent);
        }
    }
        Ok((removed,retained_targets))
    }
    fn step(&mut self,depth:usize)->EngineResult<()> {
        (self.check)()?;
        self.steps=self.steps.checked_sub(1).ok_or_else(||bad("traversal limit"))?;
        require(depth<=64,"nesting depth limit")
    }
    fn reserve(&mut self,count:usize,size:usize)->EngineResult<()> {
        self.metadata=self.metadata.checked_sub(count.checked_mul(size).ok_or_else(||bad("metadata overflow"))?).ok_or_else(||bad("metadata limit"))?;
        Ok(())
    }
    fn resolve<'v>(&mut self,mut value:&'v Object)->EngineResult<(Option<ObjectId>,&'v Object)> where 'a:'v {
        let mut id=None;
        for _ in 0..=64 {
            self.step(0)?;
            match value {
                Object::Reference(next)=>{id=Some(*next);value=self.doc.objects.get(next).ok_or_else(||bad("missing reference"))?;},
                _=>return Ok((id,value)),
            }
        }
        Err(bad("alias depth or cycle"))
    }
    fn annotations(&mut self,page:ObjectId)->EngineResult<&'a [Object]> {
        self.step(0)?;
        let dict=self.doc.get_dictionary(page).map_err(bad)?;
        match dict.get(b"Annots") {
            Err(_)=>Ok(&[]),
            Ok(value)=>match self.resolve(value)?.1 {
                Object::Null=>Ok(&[]),
                Object::Array(values)=>{require(values.len()<=100_000,"annotation count limit")?;Ok(values)},
                _=>Err(bad("invalid annotation list")),
            },
        }
    }
    fn optional_ref(&mut self,dict:&Dictionary,key:&[u8])->EngineResult<Option<ObjectId>> {
        let Ok(value)=dict.get(key) else{return Ok(None)};
        let (id,value)=self.resolve(value)?;
        if matches!(value,Object::Null){return Ok(None)}
        id.map(Some).ok_or_else(||bad("relationship has no canonical identity"))
    }
    fn page_owner(&mut self,dict:&Dictionary,page:ObjectId)->EngineResult<()> {
        require(self.optional_ref(dict,b"P")?.is_none_or(|id|id==page),"annotation page owner differs")
    }
    fn binding(&mut self,receipt:Receipt,popup:bool)->EngineResult<&'a Dictionary> {
        self.step(0)?;
        require(self.pages.contains(&receipt.output_page),"receipt page is not live")?;
        let value=self.annotations(receipt.output_page)?.get(receipt.output_slot).ok_or_else(||bad("stale annotation slot"))?;
        let (id,value)=self.resolve(value)?;
        require(id==Some(receipt.annotation),"stale annotation identity")?;
        let dict=value.as_dict().map_err(bad)?;
        let kind=self.resolve(dict.get(b"Subtype").map_err(bad)?)?.1.as_name().map_err(bad)?;
        require(if popup {kind==b"Popup"}else{matches!(kind,b"FreeText"|b"Ink"|b"Stamp"|b"Highlight"|b"Text")},"unsupported owned annotation type")?;
        self.page_owner(dict,receipt.output_page)?;
        if let Ok(value)=dict.get(b"StructParents"){require(matches!(self.resolve(value)?.1,Object::Null),"annotation has marked-content ownership")?;}
        Ok(dict)
    }
    fn target(&mut self,id:ObjectId,parent:Option<ObjectId>)->EngineResult<()> {
        if let Some(old)=self.targets.get(&id){return require(*old==parent,"conflicting annotation roles")}
        self.reserve(1,128)?;self.targets.insert(id,parent);Ok(())
    }
    fn visit(&mut self,raw:&Object,mut role:Role,depth:usize)->EngineResult<()> {
        self.step(depth)?;
        let (id,value)=self.resolve(raw)?;
        // These are validated reverse links, not alternate paths into K. An
        // opaque reference to the same element is still walked as Unknown.
        if matches!(role,Role::StructureParent|Role::ParentValues) && id.is_some_and(|id|self.structure.elements.contains(&id)||self.structure.root==Some(id)) {return Ok(())}
        if let Role::Kids(parent,inherited)=role {
            if let Object::Dictionary(dict)=value {
                let page=self.optional_ref(dict,b"Pg")?.or(inherited);
                if id.is_some_and(|id|self.structure.elements.contains(&id)){role=Role::Element(id.unwrap(),page);}
                else if dict.get(b"Type").ok().map(|v|self.resolve(v).map(|(_,v)|v.as_name().ok()==Some(b"OBJR"))).transpose()?.unwrap_or(false){role=Role::Objr(parent,page);}
            }
        }
        if let Some(id)=id {
            if let Some(parent)=self.targets.get(&id) {
                let allowed=match role {
                    Role::Slot(page)=>self.pages.contains(&page),
                    Role::Popup(root)=>*parent==Some(root),
                    Role::Parent(popup)=>parent.is_none() && self.targets.get(&popup)==Some(&Some(id)),
                    Role::WholeOwner(parent,Some(page))=>self.tag_parents.get(&id)==Some(&parent) && self.memberships.contains(&(id,page)),
                    _=>false,
                };
                require(allowed,"unhandled consumer of removed annotation")?;
                role=Role::Owned(id);
            } else if self.pages.contains(&id) {role=Role::Page(id);}
            else if id==self.catalog {role=Role::Catalog;}
            if self.seen.contains(&(id,role)){return Ok(())}
            self.reserve(1,96)?;self.seen.insert((id,role));
        }
        match value {
            Object::Array(values)=>{
                let child=match role {Role::Annotations(page)=>Role::Slot(page),Role::Kids(..)|Role::NumberTree|Role::ParentValues=>role,_=>Role::Unknown};
                for value in values {self.visit(value,child,depth+1)?;}
            },
            Object::Dictionary(dict)=>{
                // Scripts can address annotations by page/name without an
                // indirect edge. Until script semantics have a typed adapter,
                // any live non-null JS payload prevents this mutation. Do not
                // decode, copy or execute it; null optional entries are inert.
                if let Ok(script)=dict.get(b"JS") {
                    require(matches!(self.resolve(script)?.1,Object::Null),"symbolic script consumers require a mutation adapter")?;
                }
                for (key,value) in dict.iter() {
                    let child=match (role,key.as_slice()) {
                        (Role::Catalog,b"StructTreeRoot")=>Role::StructureRoot,
                        (Role::StructureRoot,b"K")=>Role::Kids(self.structure.root.ok_or_else(||bad("missing structure root"))?,None),
                        (Role::StructureRoot,b"ParentTree")=>Role::NumberTree,
                        (Role::Element(id,page),b"K")=>Role::Kids(id,page),
                        (Role::Element(..),b"P")=>Role::StructureParent,
                        (Role::Objr(parent,page),b"Obj")=>Role::WholeOwner(parent,page),
                        (Role::NumberTree,b"Kids")=>Role::NumberTree,
                        (Role::NumberTree,b"Nums")=>Role::ParentValues,
                        (Role::Page(page),b"Annots")=>Role::Annotations(page),
                        (Role::Owned(root),b"Popup") if self.targets[&root].is_none()=>Role::Popup(root),
                        (Role::Owned(popup),b"Parent") if self.targets[&popup].is_some()=>Role::Parent(popup),
                        _=>Role::Unknown,
                    };
                    self.visit(value,child,depth+1)?;
                }
            },
            Object::Stream(stream)=>for (_,value) in stream.dict.iter(){self.visit(value,Role::Unknown,depth+1)?;},
            _=>{},
        }
        Ok(())
    }
    fn copy(&mut self,value:&Object,depth:usize)->EngineResult<Object> {
        self.step(depth)?;self.reserve(1,std::mem::size_of::<Object>())?;
        match value {
            Object::String(bytes,format)=>{self.reserve(bytes.len(),1)?;Ok(Object::String(bytes.clone(),*format))},
            Object::Name(bytes)=>{self.reserve(bytes.len(),1)?;Ok(Object::Name(bytes.clone()))},
            Object::Array(values)=>{
                self.reserve(values.len(),std::mem::size_of::<Object>())?;
                let mut copied=Vec::with_capacity(values.len());for value in values {copied.push(self.copy(value,depth+1)?);}Ok(Object::Array(copied))
            },
            Object::Dictionary(dict)=>{
                let mut copied=Dictionary::new();
                for (key,value) in dict.iter(){self.reserve(key.len()+96,1)?;copied.set(key.clone(),self.copy(value,depth+1)?);}
                Ok(Object::Dictionary(copied))
            },
            Object::Stream(_)=>Err(bad("direct stream metadata cannot be copied")),
            _=>Ok(value.clone()),
        }
    }
}

#[cfg(test)]
#[path="composition_annotation_removal_tests.rs"]
mod tests;
