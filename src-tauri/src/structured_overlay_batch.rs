//! Private editable/flattened transactions; public route acceptance is separate.
#![allow(dead_code)]
use super::*;
use super::imported_overlay_sources::publication::{PageChanges,OwnedChange,AnnotationSource};
use super::structured_overlay_render::{GeneratedPage,GeneratedOverlay,PreparedOverlays};
use super::structured_overlay_publish::TargetPage;
use crate::composition_annotation_receipts::{GroupAnchors,Receipt};
use lopdf::{Document,Object,ObjectId};
use std::collections::{BTreeMap,BTreeSet};

#[path="structured_overlay_batch_flatten.rs"]
mod flatten;
#[path="structured_overlay_layering.rs"]
mod layering;
#[path="structured_overlay_read.rs"]
mod read_view;

pub(super) struct PagePlan<'a,'s,'d> {
    pub source_page:ObjectId,
    pub source_page_index:usize,
    pub source_language:&'a crate::composition_annotation_additions::SourceLanguage,
    pub changes:&'a PageChanges<'s,'d>,
    pub groups:&'a [GroupAnchors],
}
pub(super) struct Published {
    pub document:Document,
    pub pages:Vec<GeneratedPage>,
    pub added:Vec<Vec<ObjectId>>,
}
pub(super) fn publish(document:Document,targets:&[TargetPage],plans:&[PagePlan<'_,'_,'_>],receipts:&[Receipt],prepared:PreparedOverlays,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Published> {
    publish_with_budget(document,targets,plans,receipts,prepared,16*1024*1024,1_000_000,check)
}
pub(super) fn publish_flattened(document:Document,targets:&[TargetPage],plans:&[PagePlan<'_,'_,'_>],receipts:&[Receipt],prepared:PreparedOverlays,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Published>{
    publish_mode(document,targets,plans,receipts,prepared,true,false,16*1024*1024,1_000_000,check)
}
pub(super) fn publish_read_view(document:Document,targets:&[TargetPage],plans:&[PagePlan<'_,'_,'_>],receipts:&[Receipt],prepared:PreparedOverlays,flatten:bool,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Published>{
    publish_mode(document,targets,plans,receipts,prepared,flatten,true,16*1024*1024,1_000_000,check)
}

#[derive(Clone,Copy,PartialEq,Eq)]
enum Kind {Keep,Replace,Delete,Add}
struct Entry<'a> {label:&'a str,kind:Kind,root:Option<Receipt>,popups:Vec<Receipt>,id:Option<ObjectId>,flatten:bool,appearance:Option<ObjectId>}
type Key=(ObjectId,ObjectId,usize,ObjectId);
fn key(page:ObjectId,origin:crate::composition_annotation_receipts::Anchor)->Key {(page,origin.page,origin.slot,origin.annotation)}
fn source<'a,'s>(change:&'a OwnedChange<'s,'_>)->&'a AnnotationSource<'s> {match change{OwnedChange::Keep(s,_)|OwnedChange::Replace(s,_)|OwnedChange::Delete(s)=>s}}
fn label(overlay:&Overlay)->&str {match overlay{Overlay::Text(v)=>&v.id,Overlay::Ink(v)=>&v.id,Overlay::Image(v)=>&v.id,Overlay::Highlight(v)=>&v.id,Overlay::Comment(v)=>&v.id}}
fn bad(message:impl std::fmt::Display)->EngineError {EngineError::InvalidRequest(format!("Structured overlay transaction: {message}"))}
fn require(ok:bool,message:&str)->EngineResult<()> {if ok{Ok(())}else{Err(bad(message))}}
struct Work<'a>{metadata:usize,steps:usize,check:&'a mut dyn FnMut()->EngineResult<()>}
impl Work<'_> {
    fn step(&mut self)->EngineResult<()> {(self.check)()?;self.steps=self.steps.checked_sub(1).ok_or_else(||bad("traversal limit"))?;Ok(())}
    fn reserve(&mut self,count:usize,size:usize)->EngineResult<()> {self.metadata=self.metadata.checked_sub(count.checked_mul(size).ok_or_else(||bad("metadata overflow"))?).ok_or_else(||bad("metadata limit"))?;Ok(())}
    fn resolve<'d>(&mut self,doc:&'d Document,mut value:&'d Object)->EngineResult<(Option<ObjectId>,&'d Object)> {
        let mut id=None;
        for _ in 0..=64 {self.step()?;match value{Object::Reference(next)=>{id=Some(*next);value=doc.objects.get(next).ok_or_else(||bad("missing reference"))?;},_=>return Ok((id,value))}}
        Err(bad("reference depth or cycle"))
    }
    fn annotations<'d>(&mut self,doc:&'d Document,page:ObjectId)->EngineResult<&'d [Object]> {
        self.step()?;let dict=doc.objects.get(&page).ok_or_else(||bad("missing canonical page"))?.as_dict().map_err(bad)?;
        match dict.get(b"Annots"){Err(_)=>Ok(&[]),Ok(value)=>match self.resolve(doc,value)?.1{Object::Null=>Ok(&[]),Object::Array(values)=>{require(values.len()<=100_000,"annotation count limit")?;Ok(values)},_=>Err(bad("invalid annotation list"))}}
    }
    fn receipt(&mut self,doc:&Document,receipt:Receipt)->EngineResult<()> {
        let value=self.annotations(doc,receipt.output_page)?.get(receipt.output_slot).ok_or_else(||bad("stale occurrence slot"))?;
        require(self.resolve(doc,value)?.0==Some(receipt.annotation),"stale occurrence identity")
    }
}

/// Groups and receipts must be the outputs of the exact source binding and copy
/// transaction. Prepared drawings must be rendered from this same change plan.
/// All mutation consumes a private document; no partial result can escape.
fn publish_with_budget(document:Document,targets:&[TargetPage],plans:&[PagePlan<'_,'_,'_>],receipts:&[Receipt],prepared:PreparedOverlays,metadata:usize,steps:usize,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Published> {
    publish_mode(document,targets,plans,receipts,prepared,false,false,metadata,steps,check)
}
fn publish_mode(document:Document,targets:&[TargetPage],plans:&[PagePlan<'_,'_,'_>],receipts:&[Receipt],prepared:PreparedOverlays,flatten:bool,selected:bool,metadata:usize,steps:usize,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Published> {
    let mut work=Work{metadata,steps,check};work.step()?;
    require(!targets.is_empty() && targets.len()<=10_000 && targets.len()==plans.len() && targets.len()==prepared.pages.len(),"page counts differ")?;
    work.reserve(targets.len(),256)?;work.reserve(receipts.len(),192)?;
    let mut by_origin=BTreeMap::new();
    for receipt in receipts {work.step()?;work.receipt(&document,*receipt)?;require(by_origin.insert(key(receipt.output_page,receipt.origin),*receipt).is_none(),"duplicate occurrence receipt")?;}
    let mut entries=Vec::with_capacity(plans.len());
    for (index,(plan,target)) in plans.iter().zip(targets).enumerate() {
        work.step()?;require(plan.changes.owned.len()==plan.groups.len(),"source group count differs")?;
        let count=plan.changes.owned.len().checked_add(plan.changes.added.len()).ok_or_else(||bad("entry count overflow"))?;
        work.reserve(count,768)?;
        let mut page=Vec::with_capacity(count);let mut names=BTreeSet::new();
        for (change,group) in plan.changes.owned.iter().zip(plan.groups) {
            work.step()?;let source=source(change);
            require(source.page_index==plan.source_page_index && group.annotation.page==plan.source_page && group.annotation.slot==source.annotation_index,"source group context differs")?;
            require(source.popup_indices.len()==group.popups.len(),"popup count differs")?;
            let root=by_origin.remove(&key(target.page_id,group.annotation)).ok_or_else(||bad("missing source occurrence"))?;
            work.reserve(group.popups.len(),128)?;let mut popups=Vec::with_capacity(group.popups.len());
            for (slot,popup) in source.popup_indices.iter().zip(&group.popups) {
                work.step()?;require(popup.page==plan.source_page && popup.slot==*slot,"popup group context differs")?;
                popups.push(by_origin.remove(&key(target.page_id,*popup)).ok_or_else(||bad("missing popup occurrence"))?);
            }
            let kind=match change{OwnedChange::Keep(_,_)=>Kind::Keep,OwnedChange::Replace(_,overlay)=>{require(label(overlay)==source.overlay_id,"replacement workspace identity differs")?;Kind::Replace},OwnedChange::Delete(_)=>Kind::Delete};
            work.reserve(source.overlay_id.len(),1)?;
            require(names.insert(source.overlay_id),"duplicate workspace identity")?;
            let drawing=match change{OwnedChange::Keep(_,overlay)|OwnedChange::Replace(_,overlay)=>matches!(overlay,Overlay::Text(_)|Overlay::Ink(_)|Overlay::Image(_)),_=>false};
            page.push(Entry{label:source.overlay_id,kind,root:Some(root),popups,id:Some(root.annotation),flatten:flatten&&drawing,appearance:None});
        }
        for overlay in &plan.changes.added {
            work.step()?;let id=label(overlay);work.reserve(id.len(),1)?;
            require(!id.trim().is_empty() && id.len()<=1024 && names.insert(id),"invalid added workspace identity")?;
            page.push(Entry{label:id,kind:Kind::Add,root:None,popups:vec![],id:None,flatten:flatten&&matches!(overlay,Overlay::Text(_)|Overlay::Ink(_)|Overlay::Image(_)),appearance:None});
        }
        require(page.iter().filter(|e|e.kind!=Kind::Delete).count()<=10_000,"too many surviving overlays")?;
        let mut needed=page.iter().filter(|e|e.flatten||matches!(e.kind,Kind::Replace|Kind::Add)).map(|e|e.label).collect::<BTreeSet<_>>();
        for generated in &prepared.pages[index].overlays {work.step()?;require(needed.remove(generated.overlay_id.as_str()),"unexpected generated overlay")?;}
        require(needed.is_empty(),"missing generated overlay")?;
        entries.push(page);
    }
    require(by_origin.is_empty(),"unused occurrence receipt")?;
    // Nested adapters retain their own bounds, but their checkpoints also
    // consume this transaction's allowance rather than resetting its work cap.
    let attached=if selected{super::structured_overlay_publish::attach_read_view(document,targets,prepared,&mut ||work.step())?}else{super::structured_overlay_publish::attach_generated(document,targets,prepared,&mut ||work.step())?};
    let super::structured_overlay_publish::AttachedOverlays{mut document,mut pages}=attached;
    let mut paint_order=Vec::with_capacity(pages.len());let mut temporary=Vec::new();
    for (entries,page) in entries.iter_mut().zip(&pages) {
        work.reserve(page.overlays.len(),96)?;
        let positions=entries.iter().enumerate().map(|(i,e)|(e.label,i)).collect::<BTreeMap<_,_>>();let mut order=Vec::new();
        for generated in &page.overlays{
            work.step()?;let index=*positions.get(generated.overlay_id.as_str()).ok_or_else(||bad("attached drawing binding missing"))?;let entry=&mut entries[index];
            if entry.flatten{
                let dict=document.get_dictionary(generated.annotation_id).map_err(bad)?;let ap=work.resolve(&document,dict.get(b"AP").map_err(bad)?)?.1.as_dict().map_err(bad)?;
                let (id,value)=work.resolve(&document,ap.get(b"N").map_err(bad)?)?;require(matches!(value,Object::Stream(_)),"generated drawing has no stream appearance")?;
                entry.appearance=Some(id.ok_or_else(||bad("generated appearance has no identity"))?);
                if entry.kind==Kind::Keep{temporary.push((page.page_id,generated.annotation_id));}
            }
            order.push(index);
            if matches!(entry.kind,Kind::Replace|Kind::Add){entry.id=Some(generated.annotation_id);}
        }
        paint_order.push(order);
    }
    if selected {
        return read_view::finish(document,pages,plans,&entries,&paint_order,&mut work);
    }
    let replacement_count=entries.iter().flatten().filter(|e|e.kind==Kind::Replace).count();
    work.reserve(replacement_count,std::mem::size_of::<crate::composition_annotation_removal::Replacement<'_>>())?;
    let mut replacements=Vec::with_capacity(replacement_count);
    for entry in entries.iter().flatten(){work.step()?;if entry.kind==Kind::Replace{replacements.push(crate::composition_annotation_removal::Replacement{original:entry.root.expect("owned replacement"),popups:&entry.popups,generated:entry.id.expect("attached replacement")});}}
    // Only appended generated entries disappear here. Original source slots
    // therefore remain valid for the subsequent, single deletion transaction.
    document=crate::composition_annotation_removal::replace_owned(document,&replacements,&mut ||work.step())?;
    drop(replacements);
    // Keep appearances are temporary attachments, never replacements of the
    // original annotation dictionary. Remove only appended roots at this point;
    // original occurrence slots remain valid for source deletions below.
    document=flatten::discard_temporary(document,&temporary,&mut work)?;
    let removal_count=entries.iter().flatten().filter(|e|e.kind==Kind::Delete).count();
    work.reserve(removal_count,std::mem::size_of::<crate::composition_annotation_removal::Removal<'_>>())?;
    let mut removals=Vec::with_capacity(removal_count);
    for entry in entries.iter().flatten(){work.step()?;if entry.kind==Kind::Delete{removals.push(crate::composition_annotation_removal::Removal{annotation:entry.root.expect("owned deletion"),popups:&entry.popups});}}
    document=crate::composition_annotation_removal::remove_owned(document,&removals,&mut ||work.step())?;
    drop(removals);
    let mut added=Vec::with_capacity(pages.len());
    for (page,entries) in pages.iter_mut().zip(&entries) {
        // Output bindings, added IDs and three canonical-membership sets. The
        // label bytes were reserved during planning, before any string copy.
        work.reserve(entries.len(),384)?;
        work.step()?;let mut bindings=Vec::with_capacity(entries.len());let mut new_ids=vec![];let mut needed=BTreeSet::new();
        for entry in entries {
            work.step()?;if entry.kind==Kind::Delete{continue}
            let id=if entry.kind==Kind::Replace{entry.root.expect("owned replacement").annotation}else{entry.id.expect("surviving binding")};
            require(needed.insert(id),"multiple overlays share one output annotation")?;
            if entry.kind==Kind::Add{new_ids.push(id);}
            bindings.push(GeneratedOverlay{overlay_id:entry.label.to_owned(),annotation_id:id});
        }
        let expected=needed.clone();let mut found=BTreeSet::new();
        for value in work.annotations(&document,page.page_id)? {
            let (id,_)=work.resolve(&document,value)?;
            if let Some(id)=id.filter(|id|expected.contains(id)){require(found.insert(id),"surviving annotation is repeated")?;needed.remove(&id);}
        }
        require(needed.is_empty(),"surviving overlay is detached")?;
        page.overlays=bindings;added.push(new_ids);
    }
    // Only Add entries receive owners. Replacements retain their existing
    // semantic identities and Keep/Delete-only transactions remain exact no-ops
    // here; append does not manufacture an empty structure tree.
    work.reserve(pages.len(),std::mem::size_of::<crate::composition_annotation_additions::NewOwners<'_>>())?;
    let mut owners=Vec::with_capacity(pages.len());
    for ((page,ids),plan) in pages.iter().zip(&added).zip(plans){
        work.step()?;owners.push(crate::composition_annotation_additions::NewOwners{page:page.page_id,annotations:ids,language:plan.source_language});
    }
    if flatten{document=flatten::finish(document,&mut pages,&mut added,plans,&entries,&paint_order,&mut work)?;}
    else{
        document=crate::composition_annotation_additions::append(document,&owners,&mut ||work.step())?;
    }
    // Flatten removes only drawing annotations. Surviving highlights/comments
    // still need the changed draft's paint order after all slot consumers finish.
    document=layering::finish(document,&pages,plans,&entries,flatten,&mut work)?;
    crate::composition_graph::validate_retained(&document,&mut ||work.step())?;work.step()?;
    Ok(Published{document,pages,added})
}

#[cfg(test)]
#[path="structured_overlay_batch_tests.rs"]
mod tests;
