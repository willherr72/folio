//! Typed ownership for newly attached renderer annotations in a private graph.
#![allow(dead_code)]
use crate::{EngineError,EngineResult};
use lopdf::{dictionary,Dictionary,Document,Object,ObjectId};
use crate::composition_structure::Parents;
use std::collections::{BTreeMap,BTreeSet};

#[path="composition_annotation_flatten.rs"]
mod flatten;
pub(crate) use flatten::{mark_flattened,FlattenOwner,MarkedDrawings};

#[derive(Default)]
pub(crate) struct SourceLanguage(Vec<u8>);
impl SourceLanguage {
    /// Capture from the exact admitted source catalog before catalog merge.
    pub(crate) fn capture(document:&Document,catalog:ObjectId)->EngineResult<Self>{
        crate::composition_language::source_default(document,catalog).map(Self)
    }
    /// Capture under the caller's cumulative traversal/cancellation budget,
    /// including catalog and language aliases, before allocating raw bytes.
    pub(crate) fn capture_controlled(document:&Document,catalog:ObjectId,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Self>{
        let mut work=Work{bytes:1024,steps:1_000_000,check};let root=Object::Reference(catalog);
        let dict=work.resolve(document,&root)?.1.as_dict().map_err(bad)?;
        let Some(value)=work.optional(document,dict,b"Lang")? else{return Ok(Self(vec![]));};
        let bytes=value.as_str().map_err(bad)?;work.charge(bytes.len(),1)?;
        crate::composition_language::normalized_language(bytes)?;work.step(0)?;
        Ok(Self(bytes.to_vec()))
    }
}
pub(crate) struct NewOwners<'a> {
    pub page:ObjectId,
    pub annotations:&'a [ObjectId],
    pub language:&'a SourceLanguage,
}
/// Roots must be newly attached outputs of the trusted renderer transaction.
pub(crate) fn append(document:Document,additions:&[NewOwners<'_>],check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Document>{
    append_bounded(document,additions,16*1024*1024,1_000_000,check)
}
fn bad(message:impl std::fmt::Display)->EngineError {EngineError::InvalidRequest(format!("New annotation structure: {message}"))}
fn require(ok:bool,message:&str)->EngineResult<()> {if ok{Ok(())}else{Err(bad(message))}}
struct Work<'a>{bytes:usize,steps:usize,check:&'a mut dyn FnMut()->EngineResult<()>}
impl Work<'_> {
    fn step(&mut self,depth:usize)->EngineResult<()>{(self.check)()?;require(depth<=64,"nesting limit")?;self.steps=self.steps.checked_sub(1).ok_or_else(||bad("traversal limit"))?;Ok(())}
    fn charge(&mut self,n:usize,size:usize)->EngineResult<()>{self.bytes=self.bytes.checked_sub(n.checked_mul(size).ok_or_else(||bad("metadata overflow"))?).ok_or_else(||bad("metadata limit"))?;Ok(())}
    fn resolve<'d>(&mut self,doc:&'d Document,mut value:&'d Object)->EngineResult<(Option<ObjectId>,&'d Object)>{
        let mut id=None;for _ in 0..=64 {self.step(0)?;match value{Object::Reference(next)=>{id=Some(*next);value=doc.objects.get(next).ok_or_else(||bad("missing reference"))?;},_=>return Ok((id,value))}}Err(bad("reference cycle or depth"))
    }
    fn optional<'d>(&mut self,doc:&'d Document,dict:&'d Dictionary,key:&[u8])->EngineResult<Option<&'d Object>>{
        match dict.get(key){Err(_)=>Ok(None),Ok(value)=>{let value=self.resolve(doc,value)?.1;Ok((!matches!(value,Object::Null)).then_some(value))}}
    }
    fn copy(&mut self,value:&Object,depth:usize)->EngineResult<Object>{
        self.step(depth)?;self.charge(1,32)?;
        Ok(match value {
            Object::Array(values)=>{self.charge(values.len(),32)?;let mut copied=Vec::with_capacity(values.len());for value in values{copied.push(self.copy(value,depth+1)?);}Object::Array(copied)},
            Object::Dictionary(dict)=>{let mut copied=Dictionary::new();for (k,v) in dict.iter(){self.charge(k.len()+32,1)?;copied.set(k.clone(),self.copy(v,depth+1)?);}Object::Dictionary(copied)},
            Object::Name(bytes)|Object::String(bytes,_)=>{self.charge(bytes.len(),1)?;value.clone()},
            Object::Stream(_)=>return Err(bad("unexpected direct stream in structure metadata")),
            _=>value.clone(),
        })
    }
    fn plain_tree(&mut self,doc:&Document,value:&Object,depth:usize)->EngineResult<()>{
        self.step(depth)?;let dict=self.resolve(doc,value)?.1.as_dict().map_err(bad)?;
        for (key,_) in dict.iter(){self.step(depth)?;require([b"Nums".as_slice(),b"Kids",b"Limits"].contains(&key.as_slice()),"parent-tree extension metadata requires a preservation adapter")?;}
        if let Some(kids)=self.optional(doc,dict,b"Kids")? {for kid in kids.as_array().map_err(bad)?{self.plain_tree(doc,kid,depth+1)?;}}Ok(())
    }
}
fn append_bounded(mut doc:Document,additions:&[NewOwners<'_>],bytes:usize,steps:usize,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Document>{
    let mut work=Work{bytes,steps,check};work.step(0)?;
    require(additions.len()<=100_000,"page plan limit")?;
    let mut count=0usize;for addition in additions{work.step(0)?;count=count.checked_add(addition.annotations.len()).ok_or_else(||bad("annotation count overflow"))?;require(count<=100_000,"annotation count limit")?;}
    if count==0 {work.step(0)?;return Ok(doc);}
    work.charge(doc.objects.len(),128)?;work.charge(count,1024)?;work.charge(additions.len(),96)?;
    let catalog=work.resolve(&doc,doc.trailer.get(b"Root").map_err(bad)?)?.0.ok_or_else(||bad("missing catalog identity"))?;
    let pages=crate::composition_pages::inspect(&doc,catalog,&mut ||work.step(0))?.pages;
    let before=crate::composition_structure::inspect(&doc,catalog,&pages,&mut ||work.step(0))?;
    require(before.elements.len().checked_add(count).is_some_and(|n|n<=100_000) && before.owners.len().checked_add(count).is_some_and(|n|n<=100_000),"structure capacity")?;
    work.charge(pages.len(),64)?;let page_set=pages.iter().copied().collect::<BTreeSet<_>>();
    let mut desired=BTreeMap::new();let mut ordered=Vec::with_capacity(count);
    for addition in additions {
        work.step(0)?;require(page_set.contains(&addition.page),"unknown output page")?;
        for id in addition.annotations {
            work.step(0)?;work.charge(addition.language.0.len(),1)?;
            require(desired.insert(*id,addition.page).is_none(),"new root repeats")?;
            let dict=doc.objects.get(id).ok_or_else(||bad("missing canonical annotation"))?.as_dict().map_err(bad)?;
            require(work.optional(&doc,dict,b"Type")?.and_then(|v|v.as_name().ok())==Some(b"Annot"),"not an annotation")?;
            require(matches!(work.optional(&doc,dict,b"Subtype")?.and_then(|v|v.as_name().ok()),Some(b"FreeText"|b"Ink"|b"Stamp"|b"Highlight"|b"Text")),"unsupported generated subtype")?;
            for key in [b"StructParent".as_slice(),b"StructParents",b"Lang"]{require(work.optional(&doc,dict,key)?.is_none(),"new annotation already has semantic ownership")?;}
            require(work.resolve(&doc,dict.get(b"P").map_err(bad)?)?.0==Some(addition.page),"annotation page differs")?;
            ordered.push((*id,addition.page,addition.language));
        }
    }
    let mut found=BTreeSet::new();
    for page in &pages {
        work.step(0)?;let dict=doc.get_dictionary(*page).map_err(bad)?;
        if let Some(list)=work.optional(&doc,dict,b"Annots")? {
            for value in list.as_array().map_err(bad)? {
                work.step(0)?;if let Some(id)=work.resolve(&doc,value)?.0 {if let Some(expected)=desired.get(&id){require(expected==page && found.insert(id),"new annotation has repeated or foreign page membership")?;}}
            }
        }
    }
    require(found.len()==count,"new annotation is detached")?;
    let base=doc.max_id.max(doc.objects.keys().next_back().map_or(0,|id|id.0));
    let extra=count.checked_add(if before.root.is_some(){1}else{2}).ok_or_else(||bad("object count overflow"))?;
    let end=base.checked_add(u32::try_from(extra).map_err(bad)?).ok_or_else(||bad("object identity overflow"))?;
    let root=before.root.unwrap_or((base+1,0));let tree=(base+if before.root.is_some(){1}else{2},0);
    let mut children=Vec::new();let mut next=before.owners.keys().next_back().map_or(Ok(0),|k|k.checked_add(1).ok_or_else(||bad("parent key overflow")))?;
    if let Some(root)=before.root {
        let dict=doc.get_dictionary(root).map_err(bad)?;
        if let Some(roles)=work.optional(&doc,dict,b"RoleMap")?{require(!roles.as_dict().map_err(bad)?.has(b"Annot"),"standard annotation role is remapped")?;}
        if let Some(value)=work.optional(&doc,dict,b"K")? {
            if let Object::Array(values)=value {work.charge(values.len(),32)?;for value in values{children.push(work.copy(value,0)?);}}
            else {children.push(work.copy(dict.get(b"K").map_err(bad)?,0)?);}
        }
        if let Some(value)=work.optional(&doc,dict,b"ParentTreeNextKey")? {next=value.as_i64().map_err(bad)?;}
        if let Some(value)=work.optional(&doc,dict,b"ParentTree")?{work.plain_tree(&doc,value,0)?;}
    }
    let final_key=next.checked_add(i64::try_from(count).map_err(bad)?).ok_or_else(||bad("parent key overflow"))?;
    let mut nums=Vec::new();
    for (key,owner) in &before.owners {
        work.step(0)?;work.charge(1,96)?;
        let value=match &owner.parents{Parents::Whole(parent)=>Object::Reference(*parent),Parents::Marked(slots)=>{work.charge(slots.len(),32)?;let mut values=Vec::with_capacity(slots.len());for parent in slots{work.step(0)?;values.push(parent.map_or(Object::Null,Object::Reference));}Object::Array(values)}};
        nums.extend([Object::Integer(*key),value]);
    }
    let mut staged=Vec::with_capacity(count);let mut patches=Vec::with_capacity(count);
    for (i,(annotation,page,language)) in ordered.iter().enumerate(){
        work.step(0)?;let element=(tree.0+1+i as u32,0);let key=next+i as i64;
        staged.push((element,Object::Dictionary(dictionary!{"Type"=>"StructElem","S"=>"Annot","P"=>root,"Pg"=>*page,"Lang"=>Object::string_literal(language.0.as_slice()),"K"=>dictionary!{"Type"=>"OBJR","Obj"=>*annotation,"Pg"=>*page}})));
        children.push(Object::Reference(element));nums.extend([Object::Integer(key),Object::Reference(element)]);patches.push((*annotation,key));
    }
    let mark=if before.root.is_none(){
        let mut mark=match work.optional(&doc,doc.get_dictionary(catalog).map_err(bad)?,b"MarkInfo")?{Some(v)=>match work.copy(v,0)?{Object::Dictionary(dict)=>dict,_=>return Err(bad("invalid MarkInfo"))},None=>Dictionary::new()};
        mark.set("Marked",false);Some(mark)
    }else{None};
    work.step(0)?;
    // Owned document: failed post-validation never exposes a partially updated graph.
    for (id,key) in patches{doc.get_dictionary_mut(id).map_err(bad)?.set("StructParent",key);}
    doc.objects.extend(staged);doc.objects.insert(tree,dictionary!{"Nums"=>nums}.into());
    if before.root.is_none(){doc.objects.insert(root,dictionary!{"Type"=>"StructTreeRoot"}.into());}
    let dict=doc.get_dictionary_mut(root).map_err(bad)?;dict.set("K",children);dict.set("ParentTree",tree);dict.set("ParentTreeNextKey",final_key);
    let cat=doc.get_dictionary_mut(catalog).map_err(bad)?;cat.set("StructTreeRoot",root);if let Some(mark)=mark{cat.set("MarkInfo",mark);}
    doc.max_id=end;
    let after=crate::composition_structure::inspect(&doc,catalog,&pages,&mut ||work.step(0))?;
    require(after.top.starts_with(&before.top) && after.top.len()==before.top.len()+count && after.owners.len()==before.owners.len()+count,"resulting ownership differs")?;
    for (key,owner) in &before.owners{work.step(0)?;require(after.owners.get(key)==Some(owner),"original ownership changed")?;}
    work.step(0)?;Ok(doc)
}
#[cfg(test)]
#[path="composition_annotation_additions_tests.rs"]
mod tests;
