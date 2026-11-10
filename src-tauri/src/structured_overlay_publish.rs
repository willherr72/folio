//! Generated overlay graph transfer. Typed source/tag publication follows this
//! private preparation layer; it does not enable the public Save route by itself.
#![allow(dead_code)]
use super::*;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use std::collections::{BTreeMap, BTreeSet};
use super::structured_overlay_render::{PreparedOverlays, GeneratedPage};

pub(super) struct TargetPage {
    pub page_id: ObjectId,
    pub geometry: PageGeometry,
    pub unit: f32,
}
pub(super) struct AttachedOverlays {
    pub document: Document,
    pub pages: Vec<GeneratedPage>,
}
pub(super) fn attach_generated(
    document: Document, targets: &[TargetPage], prepared: PreparedOverlays,
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<AttachedOverlays> {
    attach_generated_mode(document,targets,prepared,false,check)
}
pub(super) fn attach_read_view(
    document: Document, targets: &[TargetPage], prepared: PreparedOverlays,
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<AttachedOverlays> {
    attach_generated_mode(document,targets,prepared,true,check)
}
fn attach_generated_mode(
    mut document: Document, targets: &[TargetPage], prepared: PreparedOverlays, selected:bool,
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<AttachedOverlays> {
    let mut work=Work{check,steps:0,metadata:16*1024*1024};work.step(0)?;
    let PreparedOverlays{document:mut stage,mut pages}=prepared;
    require(targets.len()==pages.len() && !targets.is_empty() && targets.len()<=100_000,"page mapping differs")?;
    require(!stage.trailer.has(b"Encrypt"),"generated document is encrypted")?;
    work.storage(targets.len(),192)?;
    let output_pages=crate::composition_pages::inspect(&document,document.trailer.get(b"Root").and_then(Object::as_reference).map_err(bad)?,&mut ||(work.check)())?.pages;
    let stage_pages=crate::composition_pages::inspect(&stage,stage.trailer.get(b"Root").and_then(Object::as_reference).map_err(bad)?,&mut ||(work.check)())?.pages;
    let matches=if selected {
        // Read views target exactly one uniquely occurring original leaf. Every
        // other page/catalog object stays in the private full-source document.
        targets.len()==1 && output_pages.iter().filter(|id|**id==targets[0].page_id).count()==1
    }else{output_pages.iter().copied().eq(targets.iter().map(|p|p.page_id))};
    require(matches && stage_pages.iter().copied().eq(pages.iter().map(|p|p.page_id)),"page occurrence order differs")?;
    let mut roots=BTreeMap::new();let mut paint=vec![];let mut existing=vec![];
    for (index,(page,target)) in pages.iter().zip(targets).enumerate() {
        work.step(0)?;
        require(same_frame(page,target),"generated page frame differs")?;
        let mut labels=BTreeSet::new();let mut expected=BTreeSet::new();
        require(page.overlays.len()<=10_000,"too many generated overlays")?;
        for binding in &page.overlays {
            work.step(0)?;work.storage(1,256)?;work.storage(binding.overlay_id.len(),1)?;
            require(labels.insert(binding.overlay_id.as_str()) && expected.insert(binding.annotation_id) && roots.insert(binding.annotation_id,index).is_none(),"ambiguous generated binding")?;
            let annotation=stage.objects.get(&binding.annotation_id).ok_or_else(||bad("missing generated root"))?.as_dict().map_err(bad)?;
            require(annotation.get(b"P").and_then(Object::as_reference).map_err(bad)?==page.page_id,"generated annotation belongs to another page")?;
            let nm=annotation.get(b"NM").map_err(bad)?;work.storage(nm.as_str().map_err(bad)?.len(),1)?;
            require(lopdf::decode_text_string(nm).map_err(bad)?==binding.overlay_id,"generated identity differs")?;
            require(matches!(annotation.get(b"Subtype").and_then(Object::as_name).map_err(bad)?,b"FreeText"|b"Ink"|b"Stamp"|b"Highlight"|b"Text"),"unexpected generated annotation type")?;
        }
        let list=annotation_list(&stage,page.page_id,&mut work)?;
        require(list.len()==page.overlays.len(),"generated annotation count differs")?;
        work.storage(list.len(),32)?;
        let mut order=vec![];
        for item in list {work.step(0)?;let id=item.as_reference().map_err(bad)?;require(expected.remove(&id),"unexpected generated annotation")?;order.push(id);}
        require(expected.is_empty(),"missing generated annotation")?;paint.push(order);
        existing.push(annotation_list(&document,target.page_id,&mut work)?);
    }
    // Root P is the only permitted link into the staging page graph. Resource
    // closures cannot refer to a page, catalog, structure owner or annotation.
    let mut ids=roots.keys().copied().collect::<BTreeSet<_>>();let mut pending=vec![];
    for root in roots.keys() {
        let dictionary=stage.objects[root].as_dict().map_err(bad)?;
        discover_dictionary(&stage,dictionary,true,&roots,&mut ids,&mut pending,&mut work,0)?;
    }
    while let Some(id)=pending.pop() {
        work.step(0)?;
        discover(&stage,stage.objects.get(&id).ok_or_else(||bad("missing dependency"))?,&roots,&mut ids,&mut pending,&mut work,0)?;
    }
    // Catalog Version can override the file header. Raising the effective
    // version must not activate dormant semantics on untouched annotations.
    let version=if roots.is_empty(){document.version.clone()}else{
        let original=effective_version(&document,&mut work)?;
        let generated=effective_version(&stage,&mut work)?;
        let version=promoted_version(&original,&generated)?;
        if original!="2.0" && version=="2.0" {
            crate::composition_annotation_language::validate_version_promotion(&document,&output_pages,&mut ||(work.check)())?;
        }
        version
    };
    let lists=existing.iter().zip(&paint).filter(|(_,new)|!new.is_empty()).map(|(old,new)|(*old,new.len())).collect::<Vec<_>>();
    crate::composition_graph::validate_overlay_additions(&document,&stage,&ids,&lists,&mut ||(work.check)())?;
    for (old,new) in &lists {
        work.storage(*new,2*std::mem::size_of::<Object>())?;
        for value in *old {reserve_copy(value,&mut work,0)?;}
    }
    work.storage(ids.len(),96)?;
    let base=document.max_id.max(document.objects.keys().next_back().map_or(0,|id|id.0));
    let end=base.checked_add(u32::try_from(ids.len()).map_err(bad)?).ok_or_else(||bad("object identity overflow"))?;
    let map=ids.iter().enumerate().map(|(i,id)|(*id,(base+i as u32+1,0))).collect::<BTreeMap<_,_>>();
    // Every payload/list reservation precedes its first copy. Source page lists
    // are copied only when needed; indirect lists can be shared by other pages.
    let mut arrays=vec![];
    for (index,order) in paint.iter().enumerate() {
        if order.is_empty(){arrays.push(None);continue;}
        let mut values=Vec::with_capacity(existing[index].len()+order.len());
        for value in existing[index] {values.push(copy_existing(value,&mut work,0)?);}
        for id in order {work.step(0)?;values.push(Object::Reference(map[id]));}
        arrays.push(Some(values));
    }
    let mut additions=BTreeMap::new();
    for id in ids {
        work.step(0)?;
        let mut value=stage.objects.remove(&id).ok_or_else(||bad("missing admitted generated object"))?;
        if roots.contains_key(&id){value.as_dict_mut().map_err(bad)?.remove(b"P");}
        relocate(&mut value,&map,&mut work,0)?;
        if let Some(index)=roots.get(&id){value.as_dict_mut().map_err(bad)?.set("P",targets[*index].page_id);}
        additions.insert(map[&id],value);
    }
    for (page,target) in pages.iter_mut().zip(targets) {
        work.step(0)?;page.page_id=target.page_id;
        for binding in &mut page.overlays {work.step(0)?;binding.annotation_id=map[&binding.annotation_id];}
    }
    work.step(0)?;
    document.objects.append(&mut additions);document.max_id=end;
    if !roots.is_empty(){document.version=version;}
    for (target,values) in targets.iter().zip(arrays) {
        if let Some(values)=values {document.objects.get_mut(&target.page_id).expect("validated output page").as_dict_mut().expect("validated page dictionary").set("Annots",values);}
    }
    crate::composition_graph::validate_retained(&document,&mut ||(work.check)())?;
    work.step(0)?;
    Ok(AttachedOverlays{document,pages})
}

fn bad(message:impl std::fmt::Display)->EngineError {EngineError::InvalidRequest(format!("Structured overlay attachment: {message}"))}
fn require(ok:bool,message:&str)->EngineResult<()> {if ok {Ok(())} else {Err(bad(message))}}
struct Work<'a>{check:&'a mut dyn FnMut()->EngineResult<()>,steps:usize,metadata:usize}
impl Work<'_> {
    fn step(&mut self,depth:usize)->EngineResult<()> {(self.check)()?;self.steps+=1;require(depth<=64 && self.steps<=1_000_000,"traversal limit")}
    fn storage(&mut self,count:usize,size:usize)->EngineResult<()> {self.metadata=self.metadata.checked_sub(count.checked_mul(size).ok_or_else(||bad("metadata overflow"))?).ok_or_else(||bad("metadata limit"))?;Ok(())}
    fn resolve<'a>(&mut self,doc:&'a Document,mut value:&'a Object)->EngineResult<&'a Object> {
        for depth in 0..=64 {self.step(depth)?;match value {Object::Reference(id)=>value=doc.objects.get(id).ok_or_else(||bad("missing reference"))?,_=>return Ok(value)}}
        Err(bad("reference depth limit"))
    }
}
fn annotation_list<'a>(doc:&'a Document,page:ObjectId,work:&mut Work<'_>)->EngineResult<&'a [Object]> {
    work.step(0)?;let page=doc.objects.get(&page).ok_or_else(||bad("missing page"))?.as_dict().map_err(bad)?;
    match page.get(b"Annots") {Err(_)=>Ok(&[]),Ok(value)=>match work.resolve(doc,value)? {Object::Null=>Ok(&[]),Object::Array(values)=>Ok(values),_=>Err(bad("invalid annotation array"))}}
}
fn same_frame(page:&GeneratedPage,target:&TargetPage)->bool {
    let g=page.geometry;let t=target.geometry;
    let edges=|g:PageGeometry|[g.left,g.bottom,g.right,g.top];
    edges(g)==edges(t) && edges(g).iter().all(|n|n.is_finite()) && g.right>g.left && g.top>g.bottom && g.rotation==t.rotation && [0,90,180,270].contains(&g.rotation)
        && page.unit==target.unit && page.unit.is_finite() && page.unit>0. && page.unit<=75_000. && page.unit.recip().is_finite()
}
fn promoted_version(old:&str,new:&str)->EngineResult<String> {
    let versions=["1.0","1.1","1.2","1.3","1.4","1.5","1.6","1.7","2.0"];
    let a=versions.iter().position(|v|*v==old).ok_or_else(||bad("unsupported source version"))?;
    let b=versions.iter().position(|v|*v==new).ok_or_else(||bad("unsupported generated version"))?;
    Ok(versions[a.max(b)].into())
}
fn effective_version(document:&Document,work:&mut Work<'_>)->EngineResult<String> {
    let catalog=work.resolve(document,document.trailer.get(b"Root").map_err(bad)?)?.as_dict().map_err(bad)?;
    match catalog.get(b"Version") {
        Err(_)=>promoted_version(&document.version,&document.version),
        Ok(value)=>match work.resolve(document,value)? {
            Object::Null=>promoted_version(&document.version,&document.version),
            value=>promoted_version(&document.version,std::str::from_utf8(value.as_name().map_err(bad)?).map_err(bad)?),
        },
    }
}
fn discover(doc:&Document,value:&Object,roots:&BTreeMap<ObjectId,usize>,ids:&mut BTreeSet<ObjectId>,pending:&mut Vec<ObjectId>,work:&mut Work<'_>,depth:usize)->EngineResult<()> {
    work.step(depth)?;
    match value {
        Object::Reference(id)=>{
            require(!roots.contains_key(id),"resource links to an annotation root")?;
            require(id.0!=0 && doc.objects.contains_key(id),"missing generated resource")?;
            if !ids.contains(id){work.storage(1,144)?;ids.insert(*id);pending.push(*id);}
        },
        Object::Array(values)=>for value in values {discover(doc,value,roots,ids,pending,work,depth+1)?;},
        Object::Dictionary(dict)=>discover_dictionary(doc,dict,false,roots,ids,pending,work,depth)?,
        Object::Stream(stream)=>discover_dictionary(doc,&stream.dict,false,roots,ids,pending,work,depth)?,
        Object::Real(value)=>require(value.is_finite(),"nonfinite generated value")?,
        _=>(),
    }
    Ok(())
}
fn discover_dictionary(doc:&Document,dict:&Dictionary,root:bool,roots:&BTreeMap<ObjectId,usize>,ids:&mut BTreeSet<ObjectId>,pending:&mut Vec<ObjectId>,work:&mut Work<'_>,depth:usize)->EngineResult<()> {
    work.step(depth)?;
    if let Ok(kind)=dict.get(b"Type") {
        let kind=work.resolve(doc,kind)?.as_name().map_err(bad)?;
        require(!matches!(kind,b"Page"|b"Pages"|b"Catalog"|b"StructTreeRoot"|b"StructElem") && (root || kind!=b"Annot"),"resource escapes generated drawing graph")?;
    }
    require(!dict.has(b"StructParent") && !dict.has(b"StructParents") && !dict.has(b"OC"),"unexpected generated semantic owner")?;
    if root {require(![b"Parent".as_slice(),b"Popup",b"IRT",b"A",b"AA",b"Dest"].iter().any(|key|dict.has(key)),"unexpected generated annotation relationship")?;}
    for (key,value) in dict.iter() {if root && key==b"P" {continue;}discover(doc,value,roots,ids,pending,work,depth+1)?;}
    Ok(())
}
fn reserve_copy(value:&Object,work:&mut Work<'_>,depth:usize)->EngineResult<()> {
    work.step(depth)?;work.storage(1,2*std::mem::size_of::<Object>())?;
    match value {
        Object::Name(v)|Object::String(v,_)=>work.storage(v.len(),1)?,
        Object::Array(v)=>for item in v {reserve_copy(item,work,depth+1)?;},
        Object::Dictionary(v)=>reserve_dictionary(v,work,depth+1)?,
        Object::Stream(v)=>{work.storage(v.content.len(),1)?;reserve_dictionary(&v.dict,work,depth+1)?;},
        _=>(),
    }Ok(())
}
fn reserve_dictionary(dict:&Dictionary,work:&mut Work<'_>,depth:usize)->EngineResult<()> {
    for (key,value) in dict.iter(){work.step(depth)?;work.storage(key.len()+64,1)?;reserve_copy(value,work,depth+1)?;}Ok(())
}
fn copy_existing(value:&Object,work:&mut Work<'_>,depth:usize)->EngineResult<Object> {
    work.step(depth)?;
    Ok(match value {
        Object::Array(v)=>Object::Array(v.iter().map(|v|copy_existing(v,work,depth+1)).collect::<EngineResult<_>>()?),
        Object::Dictionary(v)=>Object::Dictionary(copy_dictionary(v,work,depth+1)?),
        Object::Stream(v)=>Object::Stream(Stream{dict:copy_dictionary(&v.dict,work,depth+1)?,content:v.content.clone(),allows_compression:v.allows_compression,start_position:None}),
        _=>value.clone(),
    })
}
fn copy_dictionary(dict:&Dictionary,work:&mut Work<'_>,depth:usize)->EngineResult<Dictionary> {
    let mut out=Dictionary::new();for (key,value) in dict.iter(){work.step(depth)?;out.set(key.clone(),copy_existing(value,work,depth+1)?);}Ok(out)
}
fn relocate(value:&mut Object,map:&BTreeMap<ObjectId,ObjectId>,work:&mut Work<'_>,depth:usize)->EngineResult<()> {
    work.step(depth)?;
    match value {
        Object::Reference(id)=>*id=*map.get(id).ok_or_else(||bad("generated reference leaves admitted closure"))?,
        Object::Array(values)=>for value in values {relocate(value,map,work,depth+1)?;},
        Object::Dictionary(dict)=>for (_,value) in dict.iter_mut(){relocate(value,map,work,depth+1)?;},
        Object::Stream(stream)=>{stream.start_position=None;for (_,value) in stream.dict.iter_mut(){relocate(value,map,work,depth+1)?;}},
        _=>(),
    }Ok(())
}

#[cfg(test)]
#[path = "structured_overlay_publish_tests.rs"]
mod tests;
