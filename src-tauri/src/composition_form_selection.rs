//! Original-ownership field selection; action/opaque-consumer checks stay separate.
#![allow(dead_code)]
use crate::{composition_form_budget::Budget,composition_forms::FormInfo,composition_pages::SourcePages,EngineError,EngineResult};
use lopdf::{Document,Object,ObjectId};
use std::collections::{BTreeMap,BTreeSet};
pub(crate) struct Selection {pub retained:BTreeSet<ObjectId>,pub removed:BTreeSet<ObjectId>,pub patches:BTreeMap<(ObjectId,Vec<u8>),Object>}
pub(crate) fn prepare(doc:&Document,pages:&SourcePages,forms:&FormInfo,keep:&BTreeSet<ObjectId>,budget:&mut Budget,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Selection>{
 let mut walk=Walk{doc,budget,check,selection:Selection{retained:BTreeSet::new(),removed:BTreeSet::new(),patches:BTreeMap::new()}};
 let mut available=BTreeSet::new();for &page in &pages.pages{walk.step(0)?;walk.budget.reserve(64)?;available.insert(page);}
 require(!keep.is_empty() && keep.is_subset(&available),"invalid selected pages")?;
 for &id in &forms.roots{walk.select(forms,id,keep,0)?;}
 for (&id,node) in &forms.nodes{
  walk.step(0)?;
  if node.kind==Some(crate::composition_forms::FieldType::Button) && node.field && walk.selection.retained.contains(&id)
   && !node.children.is_empty() && node.children.iter().all(|child|!forms.nodes[child].field)
   && node.children.iter().any(|child|!walk.selection.retained.contains(child)){
    walk.budget.reserve(node.children.len().checked_mul(32).ok_or_else(||bad("control order overflow"))?)?;
    let controls=node.children.iter().filter(|id|walk.selection.retained.contains(id)).map(|&id|(id,id)).collect::<Vec<_>>();
    for ((target,key),value) in crate::composition_button_order::remap(doc,forms,id,id,&controls,walk.budget,&mut walk.check)?{walk.put(target,&key,value)?;}
  }
 }
 require(walk.selection.retained.len()+walk.selection.removed.len()==forms.nodes.len(),"incomplete original inventory")?;
 if forms.present{
  let raw=doc.get_dictionary(pages.catalog).map_err(bad)?.get(b"AcroForm").map_err(bad)?;
  let dict=walk.resolve(raw)?.1.as_dict().map_err(bad)?;let mut changes=BTreeMap::new();
  for key in [b"Fields".as_slice(),b"CO"]{if let Ok(value)=dict.get(key){if let Some(value)=walk.filtered(value)?{walk.budget.reserve(64+key.len())?;changes.insert(key.to_vec(),value);}}}
  if !changes.is_empty(){
   if let Some(id)=forms.form_id{for (key,value) in changes{walk.put(id,&key,value)?;}}
   else{
    let Object::Dictionary(mut dictionary)=crate::composition_form_resources::copy_metadata(doc,raw,walk.budget,&mut walk.check)? else{return Err(bad("direct form changed shape"));};
    for (key,value) in changes{dictionary.set(key,value);}walk.put(pages.catalog,b"AcroForm",dictionary.into())?;
   }
  }
 }
 walk.step(0)?;Ok(walk.selection)
}
/// Only mutates the caller's private graph. It deliberately leaves removed
/// objects present until typed consumers and reachability have been checked.
pub(crate) fn apply(mut doc:Document,selection:Selection,budget:&mut Budget,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Document>{
 for ((id,key),_) in &selection.patches{budget.step(0,check)?;require([b"Kids".as_slice(),b"Fields",b"CO",b"AcroForm",b"Opt",b"AP",b"AS",b"V",b"DV"].contains(&key.as_slice()),"invalid patch role")?;doc.get_dictionary(*id).map_err(bad)?;}
 for ((id,key),value) in selection.patches{budget.step(0,check)?;doc.get_dictionary_mut(id).map_err(bad)?.set(key,value);}
 budget.step(0,check)?;Ok(doc)
}
fn bad(message:impl std::fmt::Display)->EngineError{EngineError::InvalidRequest(format!("PDF form selection: {message}"))}
fn require(value:bool,message:&str)->EngineResult<()>{if value{Ok(())}else{Err(bad(message))}}
struct Walk<'a,'b>{doc:&'a Document,budget:&'b mut Budget,check:&'b mut dyn FnMut()->EngineResult<()>,selection:Selection}
impl<'a> Walk<'a,'_>{
 fn step(&mut self,depth:usize)->EngineResult<()>{self.budget.step(depth,self.check)}
 fn resolve<'v>(&mut self,mut value:&'v Object)->EngineResult<(Option<ObjectId>,&'v Object)> where 'a:'v{
  let mut id=None;for depth in 0..=64{self.step(depth)?;match value{Object::Reference(next)=>{id=Some(*next);value=self.doc.objects.get(next).ok_or_else(||bad("missing reference"))?;},_=>return Ok((id,value))}}Err(bad("cyclic or excessive aliases"))
 }
 fn put(&mut self,id:ObjectId,key:&[u8],value:Object)->EngineResult<()>{self.step(0)?;self.budget.reserve(key.len()+96)?;require(self.selection.patches.insert((id,key.to_vec()),value).is_none(),"conflicting patches")}
 fn select(&mut self,forms:&FormInfo,id:ObjectId,keep:&BTreeSet<ObjectId>,depth:usize)->EngineResult<bool>{
  self.step(depth)?;let node=forms.nodes.get(&id).ok_or_else(||bad("missing original node"))?;let mut child_kept=false;
  for &child in &node.children{child_kept|=self.select(forms,child,keep,depth+1)?;}
  let retained=match node.widget_page{Some(page)=>keep.contains(&page),None=>node.children.is_empty() || child_kept};
  self.budget.reserve(64)?;
  if retained{
   require(self.selection.retained.insert(id),"repeated node")?;
   if !node.children.is_empty(){let raw=self.doc.get_dictionary(id).map_err(bad)?.get(b"Kids").map_err(bad)?;if let Some(value)=self.filtered(raw)?{self.put(id,b"Kids",value)?;}}
  }else{require(self.selection.removed.insert(id),"repeated node")?;}
  Ok(retained)
 }
 fn filtered(&mut self,raw:&Object)->EngineResult<Option<Object>>{
  let value=self.resolve(raw)?.1;if matches!(value,Object::Null){return Ok(None);}let values=value.as_array().map_err(bad)?;let mut output=Vec::new();
  for value in values{
   self.step(0)?;require(matches!(value,Object::Reference(_)),"field edge needs indirect identity")?;
   let id=self.resolve(value)?.0.ok_or_else(||bad("missing field identity"))?;
   if self.selection.retained.contains(&id){self.budget.reserve(32)?;output.push(value.clone());}
   else{require(self.selection.removed.contains(&id),"field edge outside inventory")?;}
  }
  Ok((output.len()!=values.len()).then_some(Object::Array(output)))
 }
}
#[cfg(test)]
#[path="composition_form_selection_tests.rs"]
mod tests;
