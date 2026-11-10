//! Separate logical fields from merged widgets before repeated page ownership.
#![allow(dead_code)]
use crate::{composition_form_budget::Budget,composition_forms::FormInfo,composition_pages::SourcePages,composition_structure::StructureInfo,EngineError,EngineResult};
use lopdf::{Dictionary,Document,Object,ObjectId};
use std::collections::{BTreeMap,BTreeSet};
const FIELD_KEYS:&[&[u8]]=&[b"FT",b"T",b"TU",b"TM",b"Ff",b"V",b"DV",b"DA",b"Q",b"DS",b"RV",b"Opt",b"MaxLen",b"TI",b"I",b"Lock",b"SV",b"DR"];
const WIDGET_KEYS:&[&[u8]]=&[b"Type",b"Subtype",b"Rect",b"Contents",b"P",b"NM",b"M",b"F",b"AP",b"AS",b"Border",b"C",b"StructParent",b"OC",b"H",b"MK",b"A",b"BS",b"Lang"];
/// Inventories describe this same private graph. Original widgets, alias objects,
/// appearance streams and tag owners stay in place. Only logical field edges move.
/// No public Save/occurrence route is admitted by this helper alone.
pub(crate) fn normalize(mut doc:Document,pages:&SourcePages,structure:&StructureInfo,forms:&FormInfo,repeated:&BTreeSet<ObjectId>,budget:&mut Budget,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<(Document,BTreeMap<ObjectId,ObjectId>)>{
 budget.step(0,check)?;let mut page_ids=BTreeSet::new();
 for &page in &pages.pages{budget.step(0,check)?;budget.reserve(64)?;page_ids.insert(page);}
 require(repeated.is_subset(&page_ids),"repeated page outside inventory")?;
 let mut ids=BTreeMap::new();let mut max=doc.max_id.max(doc.objects.keys().next_back().map_or(0,|id|id.0));
 for (&id,node) in &forms.nodes{
  budget.step(0,check)?;
  if node.field && node.widget_page.is_some_and(|page|repeated.contains(&page)){
   require(node.children.is_empty(),"merged widget has children")?;max=max.checked_add(1).ok_or_else(||bad("object ID overflow"))?;
   budget.reserve(640)?;ids.insert(id,(max,0));
  }
 }
 if ids.is_empty(){budget.step(0,check)?;return Ok((doc,ids));}
 require(forms.nodes.len().checked_add(ids.len()).is_some_and(|n|n<=4000),"normalization exceeds 4000 field/widget nodes")?;
 let outlines=crate::composition_outlines::inspect(&doc,pages.catalog,structure,check)?;
 let mut outline_ids=BTreeSet::new();for &id in &outlines.items{budget.step(0,check)?;budget.reserve(64)?;outline_ids.insert(id);}
 let mut walk=Walk{doc:&doc,budget,check,ids:&ids,pages:&page_ids,catalog:pages.catalog,forms,structure,outlines:&outline_ids,annotations:BTreeSet::new(),seen:BTreeSet::new()};
 for &page in &pages.pages{
  let d=doc.get_dictionary(page).map_err(bad)?;
  if let Ok(value)=d.get(b"Annots") {let value=walk.resolve(value)?.1;if !matches!(value,Object::Null){for raw in value.as_array().map_err(bad)?{
   if let Some(id)=walk.resolve(raw)?.0{walk.budget.reserve(64)?;walk.annotations.insert(id);}
  }}}
 }
 for &id in ids.keys(){
  for (key,value) in doc.get_dictionary(id).map_err(bad)?.iter(){
   walk.step(0)?;
   require(FIELD_KEYS.contains(&key.as_slice()) || WIDGET_KEYS.contains(&key.as_slice()) || [b"Parent".as_slice(),b"AA"].contains(&key.as_slice()) || matches!(walk.resolve(value)?.1,Object::Null),"merged entry has an unknown field/annotation role")?;
  }
 }
 // Validate every incoming context, including an indirect container shared by
 // a typed consumer and opaque metadata. Caching only by object ID is unsound.
 for (_,value) in doc.trailer.iter(){walk.visit(value,Role::Unknown,0)?;}
 let mut arrays=BTreeMap::new();
 for (&id,node) in &forms.nodes{
  if !node.children.is_empty(){let raw=doc.get_dictionary(id).map_err(bad)?.get(b"Kids").map_err(bad)?;if let Some(value)=walk.array(raw)?{walk.budget.reserve(96)?;arrays.insert((id,b"Kids".to_vec()),value);}}
 }
 let form_raw=doc.get_dictionary(pages.catalog).map_err(bad)?.get(b"AcroForm").map_err(bad)?;
 let form=walk.resolve(form_raw)?.1.as_dict().map_err(bad)?;let mut form_patches=BTreeMap::new();
 for key in [b"Fields".as_slice(),b"CO"]{if let Ok(raw)=form.get(key){if let Some(value)=walk.array(raw)?{walk.budget.reserve(96)?;form_patches.insert(key.to_vec(),value);}}}
 let direct_form=if forms.form_id.is_none() && !form_patches.is_empty(){
  let Object::Dictionary(mut dict)=crate::composition_form_resources::copy_metadata(&doc,form_raw,walk.budget,&mut walk.check)? else{return Err(bad("direct form changed shape"));};
  for (key,value) in std::mem::take(&mut form_patches){dict.set(key,value);}Some(dict)
 }else{None};
 drop(walk);
 // Reserve IDs before the action walker allocates private action copies. Event
 // dictionaries are split *after* it commits, so no stale actions are restored.
 doc.max_id=max;
 crate::composition_navigation::rebind_form_fields(&mut doc,pages,&outlines,forms,&ids,budget,check)?;
 for (&old,&new) in &ids{
  budget.step(0,check)?;
  let events=split_events(&doc,doc.get_dictionary(old).map_err(bad)?,budget,check)?;
  let Object::Dictionary(mut widget)=doc.objects.remove(&old).ok_or_else(||bad("missing widget"))? else{return Err(bad("widget changed shape"));};
  let mut field=Dictionary::new();
  for key in FIELD_KEYS{budget.step(0,check)?;if let Some(value)=widget.remove(key){budget.reserve(key.len()+64)?;field.set(key.to_vec(),value);}}
  if let Some(parent)=widget.remove(b"Parent"){field.set("Parent",parent);}
  field.set("Kids",vec![Object::Reference(old)]);widget.set("Parent",new);widget.remove(b"AA");
  if let Some(events)=events.0{field.set("AA",events);}if let Some(events)=events.1{widget.set("AA",events);}
  require(!doc.objects.contains_key(&new),"new field ID collision")?;doc.objects.insert(old,widget.into());doc.objects.insert(new,field.into());
 }
 for ((id,key),value) in arrays{budget.step(0,check)?;doc.get_dictionary_mut(id).map_err(bad)?.set(key,value);}
 if let Some(form)=direct_form{doc.get_dictionary_mut(pages.catalog).map_err(bad)?.set("AcroForm",form);}
 else if let Some(id)=forms.form_id{for (key,value) in form_patches{budget.step(0,check)?;doc.get_dictionary_mut(id).map_err(bad)?.set(key,value);}}
 // This is a fresh output ownership phase; metadata/traversal remain cumulative.
 budget.nodes=0;let output=crate::composition_forms::inspect(&doc,pages,budget,check)?;
 require(output.nodes.len()==forms.nodes.len()+ids.len(),"output node count changed")?;
 let mapped=|id:&ObjectId|ids.get(id).copied().unwrap_or(*id);
 require(output.roots.iter().copied().eq(forms.roots.iter().map(mapped)) && output.calculation_order.iter().copied().eq(forms.calculation_order.iter().map(mapped)) && output.need_appearances==forms.need_appearances && output.form_id==forms.form_id,"form catalog ownership/order changed")?;
 for (&id,node) in &forms.nodes{
  budget.step(0,check)?;let out=output.nodes.get(&id).ok_or_else(||bad("lost original node"))?;
  if let Some(&new)=ids.get(&id){
   let parent=output.nodes.get(&new).ok_or_else(||bad("missing new field"))?;
   require(!out.field && out.parent==Some(new) && out.owner==new && out.widget_page==node.widget_page && parent.field && parent.parent==node.parent && parent.full_name==node.full_name && parent.kind==node.kind && parent.partial_name==node.partial_name && parent.children==[id] && out.full_name==node.full_name && out.kind==node.kind && out.children.is_empty(),"split ownership changed")?;
  }else{require(out.parent==node.parent && out.owner==node.owner && out.field==node.field && out.widget_page==node.widget_page && out.full_name==node.full_name && out.kind==node.kind && out.children.iter().copied().eq(node.children.iter().map(mapped)),"unrelated ownership changed")?;}
 }
 require(crate::composition_structure::inspect(&doc,pages.catalog,&pages.pages,check)?==*structure,"tag ownership changed")?;
 budget.step(0,check)?;Ok((doc,ids))
}
fn bad(message:impl std::fmt::Display)->EngineError{EngineError::InvalidRequest(format!("PDF merged form normalization: {message}"))}
fn require(ok:bool,message:&str)->EngineResult<()>{if ok{Ok(())}else{Err(bad(message))}}
#[derive(Clone,Copy,PartialEq,Eq,PartialOrd,Ord)]
enum Role{Unknown,Catalog,Form,Page,Field,Annotation,Outline,Structure,Kids,Objr,FieldArray,FieldTarget,Annotations,AnnotationTarget,HideTarget,Action,Sequence,Events}
struct Walk<'a,'b>{doc:&'a Document,budget:&'b mut Budget,check:&'b mut dyn FnMut()->EngineResult<()>,ids:&'a BTreeMap<ObjectId,ObjectId>,pages:&'a BTreeSet<ObjectId>,catalog:ObjectId,forms:&'a FormInfo,structure:&'a StructureInfo,outlines:&'a BTreeSet<ObjectId>,annotations:BTreeSet<ObjectId>,seen:BTreeSet<(ObjectId,Role)>}
impl<'a> Walk<'a,'_>{
 fn step(&mut self,depth:usize)->EngineResult<()>{self.budget.step(depth,self.check)}
 fn resolve<'v>(&mut self,mut value:&'v Object)->EngineResult<(Option<ObjectId>,&'v Object)> where 'a:'v{
  let mut id=None;for depth in 0..=64{self.step(depth)?;match value{Object::Reference(next)=>{id=Some(*next);value=self.doc.objects.get(next).ok_or_else(||bad("missing reference"))?;},_=>return Ok((id,value))}}Err(bad("cyclic or excessive aliases"))
 }
 fn array(&mut self,raw:&Object)->EngineResult<Option<Object>>{
  let value=self.resolve(raw)?.1;if matches!(value,Object::Null){return Ok(None);}let mut changed=false;let mut out=Vec::new();
  for value in value.as_array().map_err(bad)?{self.step(0)?;let id=self.resolve(value)?.0.ok_or_else(||bad("field array needs indirect identities"))?;self.budget.reserve(32)?;
   if let Some(&new)=self.ids.get(&id){out.push(Object::Reference(new));changed=true;}else{out.push(value.clone());}
  }Ok(changed.then_some(Object::Array(out)))
 }
 fn visit(&mut self,raw:&Object,mut role:Role,depth:usize)->EngineResult<()>{
  self.step(depth)?;let (id,value)=self.resolve(raw)?;
  if let Some(id)=id{
   if self.ids.contains_key(&id){require(matches!(role,Role::FieldTarget|Role::AnnotationTarget|Role::HideTarget),"opaque consumer of merged field/widget")?;}
   role=if id==self.catalog{Role::Catalog}else if self.pages.contains(&id){Role::Page}else if self.forms.nodes.contains_key(&id){Role::Field}else if self.annotations.contains(&id){Role::Annotation}else if self.structure.elements.contains(&id) || self.structure.root==Some(id){Role::Structure}else if self.outlines.contains(&id){Role::Outline}else{role};
   if self.seen.contains(&(id,role)){return Ok(());}self.budget.reserve(96)?;self.seen.insert((id,role));
  }
  match value{
   Object::Array(values)=>{let child=match role{Role::FieldArray=>Role::FieldTarget,Role::Annotations=>Role::AnnotationTarget,Role::HideTarget=>Role::HideTarget,Role::Sequence=>Role::Action,Role::Kids=>Role::Kids,_=>Role::Unknown};for value in values{self.visit(value,child,depth+1)?;}},
   Object::Dictionary(dict)=>{
    if matches!(role,Role::Sequence){role=Role::Action;}
    if matches!(role,Role::Kids){if let Ok(kind)=dict.get(b"Type"){if self.resolve(kind)?.1.as_name().ok()==Some(b"OBJR"){role=Role::Objr;}}}
    let action=if role==Role::Action{dict.get(b"S").ok().map(|v|self.resolve(v).map(|(_,v)|v.as_name().ok())).transpose()?.flatten()}else{None};
    for (key,value) in dict.iter(){
     let child=match (role,key.as_slice()){
      (Role::Catalog,b"AcroForm")=>Role::Form,
      (Role::Form,b"Fields"|b"CO")|(Role::Field,b"Kids")=>Role::FieldArray,
      (Role::Field,b"Parent")=>Role::FieldTarget,
      (Role::Page,b"Annots")=>Role::Annotations,
      (Role::Field|Role::Annotation|Role::Outline,b"A")=>Role::Action,
      (Role::Page|Role::Field|Role::Annotation,b"AA")=>Role::Events,
      (Role::Events,_)=>Role::Action,
      (Role::Action,b"Next")=>Role::Sequence,
      (Role::Action,b"Fields") if matches!(action,Some(b"ResetForm"|b"SubmitForm"))=>Role::FieldArray,
      (Role::Action,b"T") if action==Some(b"Hide")=>Role::HideTarget,
      (Role::Structure,b"K")=>Role::Kids,
      (Role::Objr,b"Obj")=>Role::AnnotationTarget,
      _=>Role::Unknown,
     };self.visit(value,child,depth+1)?;
    }
   },
   Object::Stream(stream)=>for (_,value) in stream.dict.iter(){self.visit(value,Role::Unknown,depth+1)?;},
   _=>{},
  }Ok(())
 }
}
fn split_events(doc:&Document,widget:&Dictionary,budget:&mut Budget,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<(Option<Dictionary>,Option<Dictionary>)>{
 let Some(mut value)=widget.get(b"AA").ok() else{return Ok((None,None));};
 for depth in 0..=64{budget.step(depth,check)?;match value{Object::Reference(id)=>{value=doc.objects.get(id).ok_or_else(||bad("missing event dictionary"))?;},_=>break,}}
 if matches!(value,Object::Null){return Ok((None,None));}let mut field=Dictionary::new();let mut annotation=Dictionary::new();
 for (key,value) in value.as_dict().map_err(bad)?.iter(){
  budget.step(0,check)?;budget.reserve(key.len()+64)?;
  let is_field=[b"K".as_slice(),b"F",b"V",b"C"].contains(&key.as_slice());
  let mut resolved=value;
  for depth in 0..=64{budget.step(depth,check)?;match resolved{Object::Reference(id)=>{resolved=doc.objects.get(id).ok_or_else(||bad("missing event value"))?;},_=>break,}}
  // Unknown optional null entries have no event semantics. Preserve their raw
  // edge on the existing annotation, consistently with navigation validation.
  require(is_field || [b"E".as_slice(),b"X",b"D",b"U",b"Fo",b"Bl",b"PO",b"PC",b"PV",b"PI"].contains(&key.as_slice()) || matches!(resolved,Object::Null),"unsupported merged event")?;
  let value=crate::composition_form_resources::copy_metadata(doc,value,budget,check)?;
  if is_field{field.set(key.clone(),value);}else{annotation.set(key.clone(),value);}
 }
 Ok(((!field.is_empty()).then_some(field),(!annotation.is_empty()).then_some(annotation)))
}
#[cfg(test)]
#[path="composition_form_normalize_tests.rs"]
mod tests;
