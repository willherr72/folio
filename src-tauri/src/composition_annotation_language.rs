//! Staged whole-annotation language ownership for private composition graphs.
#![allow(dead_code)]
use crate::{EngineError,EngineResult};
use crate::composition_structure::{StructureInfo,Parents,Owner};
use lopdf::{dictionary,Document,Object,ObjectId};
use std::collections::{BTreeMap,BTreeSet};

pub(crate) struct Prepared {
    pub structure:Option<StructureInfo>,
    pub language_owned_annotations:BTreeSet<ObjectId>,
}

/// A PDF2 output can activate entries that were only extensions in an older
/// source. Run after the final output version is known, even when its language
/// default did not change. This read-only admission never changes annotations.
pub(crate) fn validate_version_promotion(
    document:&Document,pages:&[ObjectId],check:&mut impl FnMut()->EngineResult<()>,
)->EngineResult<()> {
    let mut work=Work{check,steps:0,metadata:0,appearance_seen:BTreeSet::new()};
    let mut seen=BTreeSet::new();
    fn annotation(document:&Document,value:&Object,depth:usize,seen:&mut BTreeSet<ObjectId>,work:&mut Work<'_>)->EngineResult<()> {
        work.step(depth)?;
        let (id,value)=document.dereference(value).map_err(bad)?;
        if let Some(id)=id {if !seen.insert(id){return Ok(());}require(seen.len()<=100_000)?;}
        let dictionary=value.as_dict().map_err(bad)?;
        require(optional(document,dictionary,b"Lang")?.is_none())?;
        for key in [b"Popup".as_slice(),b"IRT",b"Parent"] {
            if key==b"Parent" && optional(document,dictionary,b"Subtype")?.and_then(|v|v.as_name().ok())!=Some(b"Popup") {continue;}
            if optional(document,dictionary,key)?.is_some(){annotation(document,dictionary.get(key).map_err(bad)?,depth+1,seen,work)?;}
        }
        Ok(())
    }
    require(pages.len()<=100_000)?;
    for page in pages {
        work.step(0)?;
        let dictionary=document.get_dictionary(*page).map_err(bad)?;
        if let Some(annots)=optional(document,dictionary,b"Annots")? {
            let annots=annots.as_array().map_err(bad)?;require(annots.len()<=100_000)?;
            for value in annots {annotation(document,value,0,&mut seen,&mut work)?;}
        }
    }
    work.step(0)
}

pub(crate) fn prepare(
    document:&mut Document,pages:&[ObjectId],original:&StructureInfo,
    language:&[u8],pdf2:bool,check:&mut impl FnMut()->EngineResult<()>,
)->EngineResult<Prepared> {
    let mut work=Work{check,steps:0,metadata:16*1024*1024,appearance_seen:BTreeSet::new()};
    work.step(0)?;require(language.len()<=1024 && pages.len()<=100_000)?;
    let mut occurrences:BTreeMap<ObjectId,Vec<ObjectId>>=BTreeMap::new();
    let mut occurrence_seen=BTreeSet::new();let mut order=Vec::new();
    for page in pages {
        work.step(0)?;
        let page_object=Object::Reference(*page);
        let (id,value)=document.dereference(&page_object).map_err(bad)?;
        let page=id.ok_or_else(invalid)?;
        let dictionary=value.as_dict().map_err(bad)?;
        let Some(annots)=optional(document,dictionary,b"Annots")? else {continue};
        let annots=annots.as_array().map_err(bad)?;require(annots.len()<=100_000)?;
        for annotation in annots {
            work.step(0)?;require(matches!(annotation,Object::Reference(_)))?;
            let (id,value)=document.dereference(annotation).map_err(bad)?;
            let id=id.ok_or_else(invalid)?;value.as_dict().map_err(bad)?;
            if occurrence_seen.insert((id,page)) {
                require(occurrence_seen.len()<=100_000)?;
                if !occurrences.contains_key(&id){order.push(id);}
                occurrences.entry(id).or_default().push(page);
            }
        }
    }
    let mut owned=BTreeSet::new();let mut additions=Vec::new();
    for id in order {
        work.step(0)?;
        let dictionary=document.get_dictionary(id).map_err(bad)?;
        let explicit=optional(document,dictionary,b"Lang")?;
        if let Some(language)=explicit {require(pdf2 && language.as_str().map_err(bad)?.len()<=1024)?;}
        require(optional(document,dictionary,b"StructParents")?.is_none())?;
        if let Some(key)=optional(document,dictionary,b"StructParent")? {
            let key=key.as_i64().map_err(bad)?;
            let owner=original.owners.get(&key).ok_or_else(invalid)?;
            require(owner.object==id && matches!(owner.parents,Parents::Whole(_)))?;
            owned.insert(id);continue;
        }
        if explicit.is_some(){owned.insert(id);continue;}
        let subtype=optional(document,dictionary,b"Subtype")?.ok_or_else(invalid)?.as_name().map_err(bad)?;
        require(subtype!=b"Widget")?;
        if let Some(appearance)=dictionary.get(b"AP").ok() {work.appearance(document,appearance,0)?;}
        let pages=occurrences.remove(&id).ok_or_else(invalid)?;
        let charge=pages.len().checked_mul(96).and_then(|n|n.checked_add(language.len()+512)).ok_or_else(invalid)?;
        work.metadata=work.metadata.checked_sub(charge).ok_or_else(invalid)?;
        additions.push((id,pages,subtype==b"Link"));owned.insert(id);
    }
    if additions.is_empty(){work.step(0)?;return Ok(Prepared{structure:None,language_owned_annotations:owned});}
    let base=document.max_id.max(document.objects.keys().next_back().map_or(0,|id|id.0));
    let end=base.checked_add(u32::try_from(additions.len()+2).map_err(bad)?).ok_or_else(invalid)?;
    let root=(base+1,0);let parent_tree=(base+2,0);
    let mut staged=Vec::new();let mut nums=Vec::new();let mut kids=Vec::new();let mut updates=Vec::new();
    let mut structure=StructureInfo{root:Some(root),..Default::default()};
    for (index,(annotation,pages,link)) in additions.iter().enumerate() {
        work.step(0)?;
        let element=(base+3+index as u32,0);let key=index as i64;
        let mut content=Vec::new();
        for page in pages {
            work.step(0)?;
            content.push(Object::Dictionary(dictionary!{"Type"=>"OBJR","Obj"=>*annotation,"Pg"=>*page}));
        }
        staged.push((element,Object::Dictionary(dictionary!{"Type"=>"StructElem","S"=>if *link {"Link"} else {"Annot"},"P"=>root,"Lang"=>Object::string_literal(language),"K"=>content})));
        structure.top.push(element);structure.elements.insert(element);
        structure.owners.insert(key,Owner{object:*annotation,parents:Parents::Whole(element)});
        kids.push(Object::Reference(element));nums.push(Object::Integer(key));nums.push(Object::Reference(element));updates.push((*annotation,key));
    }
    staged.push((parent_tree,Object::Dictionary(dictionary!{"Nums"=>nums})));
    staged.push((root,Object::Dictionary(dictionary!{"Type"=>"StructTreeRoot","K"=>kids,"ParentTree"=>parent_tree,"ParentTreeNextKey"=>additions.len() as i64})));
    work.step(0)?;
    // All lookups, allocations for staged values, limits and cancellation checks
    // precede this commit. Canonical annotation dictionaries cannot change while
    // this function holds the private document's exclusive borrow.
    for (id,key) in updates {
        document.objects.get_mut(&id).expect("validated canonical annotation")
            .as_dict_mut().expect("validated annotation dictionary").set("StructParent",key);
    }
    document.objects.extend(staged);document.max_id=end;
    Ok(Prepared{structure:Some(structure),language_owned_annotations:owned})
}

fn invalid()->EngineError {EngineError::InvalidRequest("PDF annotation language requires an unsupported or inconsistent ownership adapter".into())}
fn bad(error:impl std::fmt::Display)->EngineError {EngineError::InvalidRequest(format!("PDF annotation language: {error}"))}
fn require(ok:bool)->EngineResult<()> {if ok {Ok(())} else {Err(invalid())}}
fn optional<'a>(document:&'a Document,dictionary:&'a lopdf::Dictionary,key:&[u8])->EngineResult<Option<&'a Object>> {
    match dictionary.get(key) {
        Ok(value)=>{let (_,value)=document.dereference(value).map_err(bad)?;Ok(if matches!(value,Object::Null){None}else{Some(value)})},
        Err(_)=>Ok(None),
    }
}
struct Work<'a> {
    check:&'a mut dyn FnMut()->EngineResult<()>,steps:usize,metadata:usize,appearance_seen:BTreeSet<ObjectId>,
}
impl Work<'_> {
    fn step(&mut self,depth:usize)->EngineResult<()> {(self.check)()?;self.steps+=1;require(depth<=64 && self.steps<=1_000_000)}
    fn appearance(&mut self,document:&Document,value:&Object,depth:usize)->EngineResult<()> {
        self.step(depth)?;
        match value {
            Object::Reference(_)=>{
                let (id,value)=document.dereference(value).map_err(bad)?;let id=id.ok_or_else(invalid)?;
                if self.appearance_seen.insert(id) {require(self.appearance_seen.len()<=100_000)?;self.appearance(document,value,depth+1)?;}
            }
            Object::Array(values)=>for value in values {self.appearance(document,value,depth+1)?;},
            Object::Dictionary(dictionary)=>self.appearance_dictionary(document,dictionary,depth)?,
            Object::Stream(stream)=>self.appearance_dictionary(document,&stream.dict,depth)?,
            _=>{},
        }
        Ok(())
    }
    fn appearance_dictionary(&mut self,document:&Document,dictionary:&lopdf::Dictionary,depth:usize)->EngineResult<()> {
        require(optional(document,dictionary,b"StructParent")?.is_none() && optional(document,dictionary,b"StructParents")?.is_none())?;
        for (_,value) in dictionary.iter(){self.appearance(document,value,depth+1)?;}
        Ok(())
    }
}

#[cfg(test)]
#[path="composition_annotation_language_tests.rs"]
mod tests;
