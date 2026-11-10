//! Independent page, annotation and tag ownership for repeated occurrences.
#![allow(dead_code)] // Save admission follows validated occurrence graph assembly.
use crate::{composition_copy,composition_projection::{self,StructureSelection,SelectionBudget},composition_structure::{StructureInfo,Parents,Owner},EngineError,EngineResult};
use lopdf::{dictionary,Dictionary,Document,Object,ObjectId};
use std::collections::{BTreeMap,BTreeSet};

fn bad(message:impl std::fmt::Display)->EngineError{EngineError::InvalidRequest(format!("PDF page occurrence: {message}"))}
fn require(ok:bool,message:&str)->EngineResult<()>{if ok{Ok(())}else{Err(bad(message))}}
struct Work<'a>{check:&'a mut dyn FnMut()->EngineResult<()>,steps:usize,bytes:usize}
impl Work<'_>{
    fn step(&mut self,depth:usize)->EngineResult<()>{(self.check)()?;self.steps+=1;require(depth<=64 && self.steps<=1_000_000,"traversal limit")}
    fn charge(&mut self,n:usize)->EngineResult<()>{self.step(0)?;self.bytes=self.bytes.checked_sub(n).ok_or_else(||bad("metadata limit"))?;Ok(())}
}
fn resolve<'a>(doc:&'a Document,mut value:&'a Object,work:&mut Work<'_>)->EngineResult<(Option<ObjectId>,&'a Object)>{
    let mut seen=BTreeSet::new();let mut id=None;
    while let Object::Reference(next)=value {work.step(seen.len())?;require(seen.insert(*next),"cyclic alias")?;id=Some(*next);value=doc.objects.get(next).ok_or_else(||bad("missing reference"))?;}
    Ok((id,value))
}
fn optional<'a>(doc:&'a Document,dict:&'a Dictionary,key:&[u8],work:&mut Work<'_>)->EngineResult<Option<&'a Object>>{
    match dict.get(key){Ok(value) if !matches!(resolve(doc,value,work)?.1,Object::Null)=>Ok(Some(value)),_=>Ok(None)}
}
struct Plan {selection:StructureSelection,seeds:BTreeMap<ObjectId,bool>,fields:composition_copy::Fields,annotations:BTreeSet<ObjectId>}
struct Copy {page:ObjectId,map:BTreeMap<ObjectId,ObjectId>}

pub(crate) fn project_forms_tracked(document:Document,pages:&[ObjectId],anchors:&[crate::composition_annotation_receipts::Anchor],check:&mut impl FnMut()->EngineResult<()>)->EngineResult<(Document,Vec<ObjectId>,Vec<crate::composition_annotation_receipts::Receipt>)>{
    let mut tracker=crate::composition_annotation_receipts::Tracker::new(&document,anchors,check)?;
    let (document,pages)=project_inner(document,pages,true,Some(&mut tracker),check)?;
    let receipts=tracker.finish(&document,check)?;
    Ok((document,pages,receipts))
}

pub(crate) fn project_forms(document:Document,pages:&[ObjectId],check:&mut impl FnMut()->EngineResult<()>)->EngineResult<(Document,Vec<ObjectId>)>{project_inner(document,pages,true,None,check)}

pub(crate) fn project(document:Document,pages:&[ObjectId],check:&mut impl FnMut()->EngineResult<()>)->EngineResult<(Document,Vec<ObjectId>)>{project_inner(document,pages,false,None,check)}
fn project_inner(document:Document,pages:&[ObjectId],allow_forms:bool,mut tracker:Option<&mut crate::composition_annotation_receipts::Tracker>,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<(Document,Vec<ObjectId>)>{
    check()?;require(!pages.is_empty() && pages.len()<=100_000,"empty or oversized occurrence list")?;
    let mut seen=BTreeSet::new();let unique=pages.iter().copied().filter(|id|seen.insert(*id)).collect::<Vec<_>>();
    if unique.len()==pages.len(){
        if let Some(tracker)=tracker.as_deref_mut(){for page in pages {tracker.record(*page,*page,None,check)?;}}
        return Ok((if allow_forms{composition_projection::project_forms(document,pages,check)?}else{composition_projection::project(document,pages,check)?},pages.to_vec()));
    }
    let mut form_budget=crate::composition_form_budget::Budget::default();
    let mut doc=if allow_forms{composition_projection::prepare_occurrences_forms(document,&unique,&mut form_budget,check)?}else{composition_projection::prepare_occurrences(document,&unique,check)?};
    let catalog=doc.trailer.get(b"Root").and_then(Object::as_reference).map_err(bad)?;
    let base=crate::composition_pages::inspect(&doc,catalog,check)?;
    let structure=crate::composition_structure::inspect(&doc,catalog,&base.pages,check)?;
    let forms=if allow_forms{
        form_budget.nodes=0;let mut forms=crate::composition_forms::inspect(&doc,&base,&mut form_budget,check)?;
        if forms.present{
            let mut counts=BTreeMap::new();for &page in pages{form_budget.step(0,check)?;if !counts.contains_key(&page){form_budget.reserve(64)?;}*counts.entry(page).or_insert(0usize)+=1;}
            form_budget.reserve(counts.len().checked_mul(64).ok_or_else(||bad("repeated-page metadata overflow"))?)?;
            let repeated=counts.iter().filter_map(|(&page,&count)|(count>1).then_some(page)).collect::<BTreeSet<_>>();
            let (normalized,ids)=crate::composition_form_normalize::normalize(doc,&base,&structure,&forms,&repeated,&mut form_budget,check)?;doc=normalized;
            if !ids.is_empty(){form_budget.nodes=0;forms=crate::composition_forms::inspect(&doc,&base,&mut form_budget,check)?;}
            let mut total=forms.nodes.len();for node in forms.nodes.values(){form_budget.step(0,check)?;if let Some(page)=node.widget_page{total=total.checked_add(counts[&page]-1).ok_or_else(||bad("widget count overflow"))?;}}
            require(total<=4000,"repeated widgets exceed 4000 field/widget nodes")?;
            Some(forms)
        }else{None}
    }else{None};
    let page_root=doc.get_dictionary(catalog).and_then(|d|d.get(b"Pages")).and_then(Object::as_reference).map_err(bad)?;
    let mut boundaries=structure.elements.clone();boundaries.extend(base.pages.iter().copied());boundaries.extend([catalog,page_root]);boundaries.extend(structure.root);
    if let Some(forms)=&forms{for &id in forms.nodes.keys(){form_budget.step(0,check)?;form_budget.reserve(64)?;boundaries.insert(id);}}
    let mut selection_budget=SelectionBudget::default();let mut budget=composition_copy::Budget::new(&doc,check)?;
    let mut work=Work{check,steps:0,bytes:16*1024*1024};
    let mut plans=BTreeMap::new();let mut copies=Vec::new();let mut output=Vec::new();seen.clear();
    for page in pages {
        work.step(0)?;
        if seen.insert(*page){if let Some(tracker)=tracker.as_deref_mut(){tracker.record(*page,*page,None,&mut ||(work.check)())?;}output.push(*page);continue;}
        if !plans.contains_key(page){
            let selection=composition_projection::structure_for_copy(&doc,&structure,*page,&mut selection_budget,&mut ||(work.check)())?;
            let plan=plan(&doc,&structure,*page,selection,forms.as_ref(),&mut work)?;plans.insert(*page,plan);
        }
        let plan=&plans[page];
        let map=composition_copy::copy(&mut doc,&plan.seeds,&boundaries,&plan.fields,&mut budget,&mut ||(work.check)())?;
        if let Some(tracker)=tracker.as_deref_mut(){tracker.record(*page,map[page],Some(&map),&mut ||(work.check)())?;}
        work.charge(map.len().checked_mul(48).ok_or_else(||bad("map size overflow"))?+64)?;
        output.push(map[page]);copies.push(Copy{page:*page,map});
    }
    // Nothing outside the copier mutates the graph until its cumulative budget
    // has reserved every copy. Subsequent steps operate on this owned scratch.
    let mut sources=Vec::new();if structure.root.is_some(){sources.push(structure.clone());}
    for copy in &copies {
        let plan=&plans[&copy.page];let new_page=copy.map[&copy.page];
        for (old,new) in &copy.map {
            let mut value=doc.objects.remove(new).ok_or_else(||bad("missing copied object"))?;
            patch_typed(&doc,&mut value,copy.page,new_page,plan.selection.elements.contains(old),plan.annotations.contains(old),&mut work,0)?;
            doc.objects.insert(*new,value);
        }
        if structure.root.is_some(){sources.push(copied_structure(&structure,&plan.selection,&copy.map,copy.page,&mut work)?);}
    }
    if let Some(forms)=&forms{append_widgets(&mut doc,forms,&plans,&copies,&mut form_budget,&mut work)?;}
    if sources.len()>1 {
        let root=crate::composition_structure_merge::merge(&mut doc,&sources,&mut ||(work.check)())?;
        doc.get_dictionary_mut(catalog).map_err(bad)?.set("StructTreeRoot",root);
    }
    for page in &output {work.step(0)?;doc.get_dictionary_mut(*page).map_err(bad)?.set("Parent",page_root);}
    let root=doc.get_dictionary_mut(page_root).map_err(bad)?;
    root.set("Kids",output.iter().copied().map(Object::Reference).collect::<Vec<_>>());root.set("Count",output.len() as i64);
    let actual=crate::composition_pages::inspect(&doc,catalog,&mut ||(work.check)())?;require(actual.pages==output,"occurrence order changed")?;
    let final_structure=crate::composition_structure::inspect(&doc,catalog,&output,&mut ||(work.check)())?;
    let targets=copies.iter().map(|copy|(copy.map[&copy.page],copy.page)).collect::<BTreeMap<_,_>>();
    let final_forms=if forms.is_some(){form_budget.nodes=0;Some(crate::composition_forms::inspect(&doc,&actual,&mut form_budget,&mut ||(work.check)())?)}else{None};
    if let Some(final_forms)=&final_forms{crate::composition_navigation::retarget_copies_forms(&mut doc,&actual,&targets,final_forms,&mut form_budget,&mut ||(work.check)())?;}
    else{crate::composition_navigation::retarget_copies(&mut doc,&actual,&targets,&mut ||(work.check)())?;}
    let outlines=crate::composition_outlines::inspect(&doc,catalog,&final_structure,&mut ||(work.check)())?;
    if let Some(final_forms)=final_forms{
        let namespace=crate::composition_form_assembly::namespace(&[&final_forms],&mut form_budget,&mut ||(work.check)())?;
        let values=crate::composition_navigation::reconcile_forms(&mut doc,&[actual],&[outlines],&[final_forms],&namespace,&mut form_budget,&mut ||(work.check)())?;
        let catalog=doc.get_dictionary_mut(catalog).map_err(bad)?;for (key,value) in [(b"Dests".as_slice(),values.dests),(b"Names".as_slice(),values.names)]{match value{Some(value)=>catalog.set(key,value),None=>{catalog.remove(key);}}}
    }else{crate::composition_navigation::reconcile(&mut doc,&[actual],&[outlines],&mut ||(work.check)())?;}
    crate::composition_graph::validate_retained(&doc,&mut ||(work.check)())?;
    Ok((doc,output))
}
/// Append occurrence-owned controls while retaining every logical field and
/// original child position. Stage indexed button metadata before any mutation.
fn append_widgets(doc:&mut Document,forms:&crate::composition_forms::FormInfo,plans:&BTreeMap<ObjectId,Plan>,copies:&[Copy],budget:&mut crate::composition_form_budget::Budget,work:&mut Work<'_>)->EngineResult<()> {
    let mut orders=BTreeMap::<ObjectId,Vec<(ObjectId,ObjectId)>>::new();let mut added=BTreeSet::new();
    for copy in copies{for &old in &plans[&copy.page].annotations{
        budget.step(0,&mut ||(work.check)())?;let Some(node)=forms.nodes.get(&old) else{continue};
        require(!node.field && node.widget_page==Some(copy.page),"copied widget has no independent owner")?;
        let new=*copy.map.get(&old).ok_or_else(||bad("missing copied widget"))?;
        budget.reserve(64)?;require(!forms.nodes.contains_key(&new) && added.insert(new),"reused copied widget identity")?;
        let field=node.owner;let parent=&forms.nodes[&field];require(parent.field && parent.widget_page.is_none(),"widget parent is still merged")?;
        if !orders.contains_key(&field){
            budget.reserve(parent.children.len().checked_mul(32).and_then(|n|n.checked_add(96)).ok_or_else(||bad("control metadata overflow"))?)?;
            require(parent.children.iter().all(|id|!forms.nodes[id].field),"field children are not pure controls")?;
            orders.insert(field,parent.children.iter().map(|&id|(id,id)).collect());
        }
        budget.reserve(32)?;orders.get_mut(&field).unwrap().push((old,new));
    }}
    let mut patches=BTreeMap::new();
    for (&field,controls) in &orders{
        budget.step(0,&mut ||(work.check)())?;let raw=doc.get_dictionary(field).map_err(bad)?.get(b"Kids").map_err(bad)?;let original=resolve(doc,raw,work)?.1.as_array().map_err(bad)?;
        let mut kids=Vec::new();for value in original{kids.push(crate::composition_form_resources::copy_metadata(doc,value,budget,&mut ||(work.check)())?);}
        for &(_,new) in &controls[original.len()..]{budget.step(0,&mut ||(work.check)())?;budget.reserve(32)?;kids.push(Object::Reference(new));}
        budget.reserve(96)?;patches.insert((field,b"Kids".to_vec()),Object::Array(kids));
        for (key,value) in crate::composition_button_order::repeat(doc,forms,field,controls,budget,&mut ||(work.check)())?{require(patches.insert(key,value).is_none(),"conflicting occurrence field patches")?;}
    }
    for ((id,key),value) in patches{budget.step(0,&mut ||(work.check)())?;doc.get_dictionary_mut(id).map_err(bad)?.set(key,value);}
    budget.step(0,&mut ||(work.check)())?;Ok(())
}
fn plan(doc:&Document,structure:&StructureInfo,page:ObjectId,selection:StructureSelection,forms:Option<&crate::composition_forms::FormInfo>,work:&mut Work<'_>)->EngineResult<Plan>{
    let mut seeds=BTreeMap::from([(page,false)]);let mut fields=composition_copy::Fields::new();
    for id in selection.elements.iter().chain(selection.records.iter()).chain(selection.whole.iter()){work.charge(48)?;seeds.insert(*id,true);}
    for (owner,_) in &selection.marked {if *owner!=page{work.charge(48)?;seeds.insert(*owner,true);}}
    if let Some(root)=structure.root {seeds.insert(root,true);}
    for ((id,key),value) in &selection.fields {fields.entry(*id).or_default().insert(key.clone(),value.clone());}
    // K arrays containing only direct MCRs or integer MCIDs also need their own
    // identity: their page-specific records may have no remapped reference.
    for id in selection.elements.iter().chain(structure.root.iter()) {
        let value=match selection.fields.get(&(*id,b"K".to_vec())) {
            Some(value)=>value.as_ref(),
            None=>optional(doc,doc.get_dictionary(*id).map_err(bad)?,b"K",work)?,
        };
        if let Some(value)=value {
            kid_containers(doc,value,&selection.elements,&mut seeds,work,0)?;
        }
    }
    // String Header IDs are rewritten by the merger, so reference dependency
    // propagation alone cannot establish attribute ownership. Seed the exact
    // indirect attributes/arrays it mutates; unrelated payloads remain shared.
    let mut attributes=BTreeSet::new();
    for id in &selection.elements {
        if let Some(value)=optional(doc,doc.get_dictionary(*id).map_err(bad)?,b"A",work)? {
            attribute_owners(doc,value,&mut seeds,&mut attributes,work,0)?;
        }
    }
    if let Some(root)=structure.root {
        if let Some(value)=optional(doc,doc.get_dictionary(root).map_err(bad)?,b"ClassMap",work)? {
            let dict=resolve(doc,value,work)?.1.as_dict().map_err(bad)?;
            for (_,value) in dict.iter(){attribute_owners(doc,value,&mut seeds,&mut attributes,work,0)?;}
        }
    }
    if let Some(root)=structure.root {
        let mut ids=BTreeMap::new();
        for id in &selection.elements {
            if let Some(value)=optional(doc,doc.get_dictionary(*id).map_err(bad)?,b"ID",work)?{
                let bytes=resolve(doc,value,work)?.1.as_str().map_err(bad)?;work.charge(bytes.len()+64)?;
                require(ids.insert(bytes.to_vec(),*id).is_none(),"duplicate structure ID")?;
            }
        }
        let mut names=Vec::new();for (name,id) in ids {names.push(Object::String(name,lopdf::StringFormat::Hexadecimal));names.push(id.into());}
        fields.entry(root).or_default().insert(b"IDTree".to_vec(),(!names.is_empty()).then(||dictionary!{"Names"=>names}.into()));
    }
    let page_dict=doc.get_dictionary(page).map_err(bad)?;

    let mut annotations=BTreeSet::new();
    if let Some(value)=optional(doc,page_dict,b"Annots",work)? {
        let (id,value)=resolve(doc,value,work)?;if let Some(id)=id{seeds.insert(id,true);}
        let values=value.as_array().map_err(bad)?;let mut primary=BTreeSet::new();
        for value in values {if let Some(id)=resolve(doc,value,work)?.0{work.charge(32)?;primary.insert(id);}}
        for value in values {annotation(doc,value,page,&selection,&primary,&mut annotations,&mut seeds,forms,work,0)?;}
    }
    Ok(Plan{selection,seeds,fields,annotations})
}
fn kid_containers(doc:&Document,value:&Object,elements:&BTreeSet<ObjectId>,seeds:&mut BTreeMap<ObjectId,bool>,work:&mut Work<'_>,depth:usize)->EngineResult<()>{
    work.step(depth)?;let (id,value)=resolve(doc,value,work)?;
    if let Some(id)=id {if elements.contains(&id){return Ok(());}}
    if let Object::Array(values)=value {
        if let Some(id)=id{work.charge(48)?;seeds.insert(id,true);}
        for value in values {kid_containers(doc,value,elements,seeds,work,depth+1)?;}
    }Ok(())
}
fn attribute_owners(doc:&Document,value:&Object,seeds:&mut BTreeMap<ObjectId,bool>,seen:&mut BTreeSet<ObjectId>,work:&mut Work<'_>,depth:usize)->EngineResult<()>{
    work.step(depth)?;let (id,value)=resolve(doc,value,work)?;
    if let Some(id)=id{if !seen.insert(id){return Ok(());}work.charge(32)?;}
    let dictionary=match value {
        Object::Array(values)=>{
            if let Some(id)=id{work.charge(48)?;seeds.insert(id,true);}
            for value in values {attribute_owners(doc,value,seeds,seen,work,depth+1)?;}
            return Ok(());
        },
        Object::Dictionary(dict)=>dict,Object::Stream(stream)=>&stream.dict,
        // Revision integers and malformed values remain subject to the existing
        // merger's grammar checks before publication.
        _=>return Ok(()),
    };
    let owner=resolve(doc,dictionary.get(b"O").map_err(bad)?,work)?.1.as_name().map_err(bad)?;
    if owner==b"Table" && optional(doc,dictionary,b"Headers",work)?.is_some() {
        if let Some(id)=id{work.charge(48)?;seeds.insert(id,true);}
    }
    Ok(())
}
fn annotation(doc:&Document,value:&Object,page:ObjectId,selection:&StructureSelection,primary:&BTreeSet<ObjectId>,annotations:&mut BTreeSet<ObjectId>,seeds:&mut BTreeMap<ObjectId,bool>,forms:Option<&crate::composition_forms::FormInfo>,work:&mut Work<'_>,depth:usize)->EngineResult<()>{
    work.step(depth)?;let (id,value)=resolve(doc,value,work)?;
    if matches!(value,Object::Null){return Ok(());}
    if let Some(id)=id {if !annotations.insert(id){return Ok(());}work.charge(80)?;seeds.insert(id,true);}
    let dict=value.as_dict().map_err(bad)?;let subtype=resolve(doc,dict.get(b"Subtype").map_err(bad)?,work)?.1.as_name().map_err(bad)?;
    require(subtype!=b"Widget" || forms.is_some_and(|forms|id.is_some_and(|id|forms.nodes.get(&id).is_some_and(|node|!node.field && node.widget_page==Some(page)))),"duplicated form widgets need independent field ownership")?;
    if depth>0 && !id.is_some_and(|id|primary.contains(&id)) {
        let parent=optional(doc,dict,b"Parent",work)?.map(|v|resolve(doc,v,work).map(|(id,_)|id)).transpose()?.flatten();
        require(subtype==b"Popup" && parent.is_some_and(|id|primary.contains(&id)),"related annotation ownership is outside the copied page")?;
    }
    if let Some(p)=optional(doc,dict,b"P",work)?{require(resolve(doc,p,work)?.0==Some(page),"annotation belongs to another page")?;}
    if optional(doc,dict,b"StructParent",work)?.is_some(){require(id.is_some_and(|id|selection.whole.contains(&id)),"annotation tag belongs outside the copied page")?;}
    if depth>0 && !id.is_some_and(|id|primary.contains(&id)) {require(optional(doc,dict,b"A",work)?.is_none() && optional(doc,dict,b"AA",work)?.is_none() && optional(doc,dict,b"Dest",work)?.is_none(),"related annotation actions require an additional navigation adapter")?;}
    for key in [b"Popup".as_slice(),b"IRT",b"Parent"] {
        if key==b"Parent" && subtype!=b"Popup"{continue;}
        if let Some(value)=optional(doc,dict,key,work)?{annotation(doc,value,page,selection,primary,annotations,seeds,forms,work,depth+1)?;}
    }Ok(())
}
fn patch_typed(doc:&Document,value:&mut Object,old:ObjectId,new:ObjectId,element:bool,annotation:bool,work:&mut Work<'_>,depth:usize)->EngineResult<()>{
    work.step(depth)?;
    match value {
        Object::Array(values)=>for value in values {patch_typed(doc,value,old,new,false,false,work,depth+1)?;},
        Object::Dictionary(dict)=>{
            let kind=optional(doc,dict,b"Type",work)?.map(|v|resolve(doc,v,work).and_then(|(_,v)|v.as_name().map_err(bad))).transpose()?;
            let record=matches!(kind,Some(b"MCR"|b"OBJR"));
            let annotation=annotation || kind==Some(b"Annot") || (dict.has(b"Subtype") && dict.has(b"Rect"));
            for key in [b"Pg".as_slice(),b"StmOwn",b"P"] {
                let applicable=if key==b"P"{annotation}else{element || record};
                if applicable {if let Some(value)=optional(doc,dict,key,work)? {if resolve(doc,value,work)?.0==Some(old){dict.set(key,new);}}}
            }
            for (_,value) in dict.iter_mut(){patch_typed(doc,value,old,new,false,false,work,depth+1)?;}
        },
        Object::Stream(_)=>{}, // Stream ownership uses StructParents, not a page edge.
        _=>{},
    }Ok(())
}
fn copied_structure(original:&StructureInfo,selection:&StructureSelection,map:&BTreeMap<ObjectId,ObjectId>,page:ObjectId,work:&mut Work<'_>)->EngineResult<StructureInfo>{
    let mapped=|id:&ObjectId|map.get(id).copied().ok_or_else(||bad("missing copied ownership identity"));
    let root=original.root.map(|id|mapped(&id)).transpose()?;
    let top=original.top.iter().filter(|id|selection.elements.contains(id)).map(mapped).collect::<EngineResult<_>>()?;
    let elements=selection.elements.iter().map(mapped).collect::<EngineResult<_>>()?;let mut owners=BTreeMap::new();
    for (key,owner) in &original.owners {
        work.step(0)?;
        let parents=match &owner.parents {
            Parents::Whole(parent)=>if selection.whole.contains(&owner.object){Some(Parents::Whole(mapped(parent)?))}else{None},
            Parents::Marked(slots)=>{
                let any=selection.marked.range((owner.object,i64::MIN)..=(owner.object,i64::MAX)).next().is_some();
                if !any && owner.object!=page{None}else{
                    work.charge(slots.len().checked_mul(32).ok_or_else(||bad("slot budget overflow"))?)?;
                    Some(Parents::Marked(slots.iter().enumerate().map(|(i,parent)|{
                        if selection.marked.contains(&(owner.object,i as i64)){parent.as_ref().map(mapped).transpose()}else{Ok(None)}
                    }).collect::<EngineResult<_>>()?))
                }
            },
        };
        if let Some(parents)=parents{work.charge(64)?;owners.insert(*key,Owner{object:mapped(&owner.object)?,parents});}
    }
    Ok(StructureInfo{root,top,elements,owners})
}
#[cfg(test)]
#[path="composition_occurrence_forms_tests.rs"]
mod form_tests;
