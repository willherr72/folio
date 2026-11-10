//! Convert proved whole-annotation ownership to a unique marked appearance wrapper.
//! This private prerequisite does not detach annotations or publish page content.
use super::*;
use lopdf::Stream;

pub(crate) struct FlattenOwner {
    pub page:ObjectId,
    pub annotation:ObjectId,
    pub appearance:ObjectId,
}
pub(crate) struct MarkedDrawings {
    pub document:Document,
    /// Unique wrappers in request order, for the isolated page-content writer.
    pub forms:Vec<ObjectId>,
}
pub(crate) fn mark_flattened(document:Document,requests:&[FlattenOwner],check:&mut impl FnMut()->EngineResult<()>)->EngineResult<MarkedDrawings>{
    convert(document,requests,16*1024*1024,1_000_000,check)
}

/// Callers supply source/copy-proved roots and trusted page-framed renderer
/// appearances. This does not authorize detaching a root: the typed removal
/// transaction must still prove all annotation/popup consumers afterward.
fn convert(mut doc:Document,requests:&[FlattenOwner],bytes:usize,steps:usize,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<MarkedDrawings>{
    let mut work=Work{bytes,steps,check};work.step(0)?;
    if requests.is_empty(){return Ok(MarkedDrawings{document:doc,forms:vec![]});}
    require(requests.len()<=100_000,"flatten owner count limit")?;
    work.charge(doc.objects.len(),128)?;work.charge(requests.len(),1024)?;
    crate::composition_graph::validate_retained(&doc,&mut ||work.step(0))?;
    let catalog=work.resolve(&doc,doc.trailer.get(b"Root").map_err(bad)?)?.0.ok_or_else(||bad("missing catalog identity"))?;
    let pages=crate::composition_pages::inspect(&doc,catalog,&mut ||work.step(0))?.pages;
    let before=crate::composition_structure::inspect(&doc,catalog,&pages,&mut ||work.step(0))?;
    let root=before.root.ok_or_else(||bad("drawing requires an explicit semantic owner"))?;
    let root_dict=doc.get_dictionary(root).map_err(bad)?;
    if let Some(value)=work.optional(&doc,root_dict,b"ParentTree")?{work.plain_tree(&doc,value,0)?;}
    work.charge(pages.len(),64)?;let page_set=pages.iter().copied().collect::<BTreeSet<_>>();
    let base=doc.max_id.max(doc.objects.keys().next_back().map_or(0,|id|id.0));
    let end=base.checked_add(u32::try_from(requests.len()+1).map_err(bad)?).ok_or_else(||bad("object identity overflow"))?;
    let tree=(end,0);
    let mut selected=BTreeMap::new();let mut forms=Vec::with_capacity(requests.len());
    for (index,request) in requests.iter().enumerate(){
        work.step(0)?;require(page_set.contains(&request.page),"unknown flattened page")?;
        require(selected.insert(request.annotation,(request.page,(base+1+index as u32,0))).is_none(),"repeated flattened annotation")?;
        let dict=doc.objects.get(&request.annotation).ok_or_else(||bad("missing canonical annotation"))?.as_dict().map_err(bad)?;
        require(matches!(work.optional(&doc,dict,b"Subtype")?.and_then(|v|v.as_name().ok()),Some(b"FreeText"|b"Ink"|b"Stamp")),"annotation is not a flattenable drawing")?;
        // Annotation-level PDF2 language needs a separate explicit override
        // adapter. Never silently replace it with its structure ancestor's Lang.
        require(work.optional(&doc,dict,b"Lang")?.is_none(),"annotation language override requires a flatten adapter")?;
        // `optional` resolves aliases, so preserve the reference for page identity.
        if let Ok(value)=dict.get(b"P"){let (id,v)=work.resolve(&doc,value)?;require(matches!(v,Object::Null)||id==Some(request.page),"flattened annotation page differs")?;}
        forms.push((base+1+index as u32,0));
    }
    let mut found=BTreeSet::new();
    for page in &pages{
        if let Some(list)=work.optional(&doc,doc.get_dictionary(*page).map_err(bad)?,b"Annots")?{
            for value in list.as_array().map_err(bad)?{work.step(0)?;if let Some(id)=work.resolve(&doc,value)?.0{if let Some((expected,_))=selected.get(&id){require(expected==page&&found.insert(id),"flattened annotation has shared or foreign membership")?;}}}
        }
    }
    require(found.len()==requests.len(),"flattened annotation is detached")?;
    let mut bindings=BTreeMap::new();let mut parents=BTreeSet::new();let mut tree_values=0usize;
    for (key,owner) in &before.owners{
        work.step(0)?;
        tree_values=tree_values.checked_add(3+match &owner.parents{Parents::Marked(slots)=>slots.len(),Parents::Whole(_)=>1}).ok_or_else(||bad("parent tree capacity overflow"))?;
        if selected.contains_key(&owner.object){
            let Parents::Whole(parent)=owner.parents else{return Err(bad("drawing does not have a whole-object owner"))};
            work.charge(1,128)?;bindings.insert(owner.object,(*key,parent));parents.insert(parent);
        }
    }
    require(bindings.len()==requests.len(),"drawing requires an explicit semantic owner")?;
    let dictionaries=parents.iter().map(|id|doc.get_dictionary(*id).map_err(bad)).collect::<EngineResult<Vec<_>>>()?;
    // Reserve conservative combined retained storage before copying K values or
    // rebuilding the standard ParentTree. Original indirect metadata stays live
    // until the final transaction prunes it.
    let new_values=tree_values.checked_add(requests.len()*64).ok_or_else(||bad("value capacity overflow"))?;
    crate::composition_graph::reserve_content_additions(&doc,&dictionaries,requests.len()+1,new_values,requests.len()*1024,&mut ||work.step(0))?;
    work.charge(tree_values,32)?;
    let mut nums=Vec::new();
    for (key,owner) in &before.owners{
        work.step(0)?;
        let value=if let Some((_,parent))=bindings.get(&owner.object){Object::Array(vec![Object::Reference(*parent)])}else{match &owner.parents{
            Parents::Whole(parent)=>Object::Reference(*parent),
            Parents::Marked(slots)=>{let mut values=Vec::with_capacity(slots.len());for parent in slots{work.step(0)?;values.push(parent.map_or(Object::Null,Object::Reference));}Object::Array(values)},
        }};nums.extend([Object::Integer(*key),value]);
    }
    let mut replacements=Vec::with_capacity(parents.len());let mut changed=BTreeSet::new();
    for parent in parents{
        work.step(0)?;let kids=doc.get_dictionary(parent).map_err(bad)?.get(b"K").map_err(bad)?;
        let replacement=rewrite_kids(&doc,kids,parent,&selected,&bindings,&mut changed,&mut work,0)?;replacements.push((parent,replacement));
    }
    require(changed.len()==requests.len(),"flattened owner child differs")?;
    let mut wrappers=Vec::with_capacity(requests.len());
    for (index,request) in requests.iter().enumerate(){
        work.step(0)?;
        let appearance=doc.objects.get(&request.appearance).ok_or_else(||bad("missing canonical appearance"))?.as_stream().map_err(bad)?;
        require(work.optional(&doc,&appearance.dict,b"Subtype")?.and_then(|v|v.as_name().ok())==Some(b"Form"),"appearance is not a Form")?;
        for key in [b"StructParent".as_slice(),b"StructParents"]{require(work.optional(&doc,&appearance.dict,key)?.is_none(),"appearance already owns marked content")?;}
        if let Some(matrix)=work.optional(&doc,&appearance.dict,b"Matrix")?{
            let values=numbers(&doc,matrix,6,&mut work)?;require(values==[1.,0.,0.,1.,0.,0.],"appearance is not in the page frame")?;
        }
        let bbox=work.optional(&doc,&appearance.dict,b"BBox")?.ok_or_else(||bad("appearance has no box"))?;
        let values=numbers(&doc,bbox,4,&mut work)?;require(values[2]>values[0]&&values[3]>values[1],"empty appearance box")?;
        let bbox=work.copy(bbox,0)?;
        let key=bindings[&request.annotation].0;
        let stream=Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>bbox,"Resources"=>dictionary!{"XObject"=>dictionary!{"Drawing"=>request.appearance}},"StructParents"=>key},b"/Span <</MCID 0>> BDC /Drawing Do EMC\n".to_vec());
        wrappers.push((forms[index],Object::Stream(stream)));
    }
    work.step(0)?;
    for request in requests{work.step(0)?;doc.get_dictionary_mut(request.annotation).map_err(bad)?.remove(b"StructParent");}
    for (parent,kids) in replacements{work.step(0)?;doc.get_dictionary_mut(parent).map_err(bad)?.set("K",kids);}
    doc.objects.extend(wrappers);doc.objects.insert(tree,dictionary!{"Nums"=>nums}.into());doc.get_dictionary_mut(root).map_err(bad)?.set("ParentTree",tree);doc.max_id=end;
    let after=crate::composition_structure::inspect(&doc,catalog,&pages,&mut ||work.step(0))?;
    require(after.root==before.root&&after.top==before.top&&after.elements==before.elements&&after.owners.len()==before.owners.len(),"flattening changed structure order")?;
    for (key,owner) in &before.owners{
        work.step(0)?;let actual=after.owners.get(key).ok_or_else(||bad("flattening removed an owner"))?;
        if let Some((_,form))=selected.get(&owner.object){require(actual.object==*form&&actual.parents==Parents::Marked(vec![Some(bindings[&owner.object].1)]),"flattened owner differs")?;}
        else{require(actual==owner,"unselected owner changed")?;}
    }
    crate::composition_graph::validate_retained(&doc,&mut ||work.step(0))?;work.step(0)?;
    Ok(MarkedDrawings{document:doc,forms})
}
fn numbers(doc:&Document,value:&Object,count:usize,work:&mut Work<'_>)->EngineResult<Vec<f64>>{
    let values=value.as_array().map_err(bad)?;require(values.len()==count,"appearance coordinate count")?;work.charge(count,8)?;
    let mut out=Vec::with_capacity(count);for value in values{let value=work.resolve(doc,value)?.1;let n=match value{Object::Integer(n)=>*n as f64,Object::Real(n)=>*n as f64,_=>return Err(bad("invalid appearance coordinate"))};require(n.is_finite(),"nonfinite appearance coordinate")?;out.push(n);}Ok(out)
}
fn rewrite_kids(doc:&Document,raw:&Object,parent:ObjectId,selected:&BTreeMap<ObjectId,(ObjectId,ObjectId)>,bindings:&BTreeMap<ObjectId,(i64,ObjectId)>,changed:&mut BTreeSet<ObjectId>,work:&mut Work<'_>,depth:usize)->EngineResult<Object>{
    work.step(depth)?;let (_,value)=work.resolve(doc,raw)?;
    if let Object::Array(values)=value{work.charge(values.len(),32)?;let mut out=Vec::with_capacity(values.len());for value in values{out.push(rewrite_kids(doc,value,parent,selected,bindings,changed,work,depth+1)?);}return Ok(Object::Array(out));}
    if let Object::Dictionary(dict)=value{
        if work.optional(doc,dict,b"Type")?.and_then(|v|v.as_name().ok())==Some(b"OBJR"){
            let annotation=work.resolve(doc,dict.get(b"Obj").map_err(bad)?)?.0.ok_or_else(||bad("direct annotation owner"))?;
            if let Some((page,form))=selected.get(&annotation){
                require(bindings[&annotation].1==parent&&changed.insert(annotation),"flattened child is repeated or misplaced")?;
                for (key,_) in dict.iter(){work.step(depth)?;require(matches!(key.as_slice(),b"Type"|b"Obj"|b"Pg"),"OBJR extension requires a flatten adapter")?;}
                if let Ok(value)=dict.get(b"Pg"){let (id,value)=work.resolve(doc,value)?;require(matches!(value,Object::Null)||id==Some(*page),"flattened child page differs")?;}
                work.charge(1,256)?;
                return Ok(dictionary!{"Type"=>"MCR","Pg"=>*page,"Stm"=>*form,"MCID"=>0}.into());
            }
        }
    }
    work.copy(raw,depth)
}

#[cfg(test)]
#[path="composition_annotation_flatten_tests.rs"]
mod tests;
