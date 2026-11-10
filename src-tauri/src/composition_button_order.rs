//! Keep indexed button exports and appearance states bound to widget order.
#![allow(dead_code)]
use crate::{composition_form_budget::Budget,composition_forms::{FieldType,FormInfo},EngineError,EngineResult};
use lopdf::{Dictionary,Document,Object,ObjectId};
use std::collections::{BTreeMap,BTreeSet};
type Patches=BTreeMap<(ObjectId,Vec<u8>),Object>;
#[path="composition_button_repeat.rs"]
mod repeated;
pub(crate) use repeated::repeat;
/// Each pair identifies an original widget and its output occurrence. Field
/// ownership/copying remains the caller's job; this only changes positional
/// representation, never appearance streams or field flags.
pub(crate) fn remap(doc:&Document,forms:&FormInfo,field:ObjectId,output_field:ObjectId,controls:&[(ObjectId,ObjectId)],budget:&mut Budget,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Patches>{
 let mut walk=Walk{doc,budget,check,patches:BTreeMap::new()};walk.step(0)?;
 require(output_field.0!=0,"invalid output field")?;
 let node=forms.nodes.get(&field).ok_or_else(||bad("missing field"))?;require(node.field,"widget is not a field")?;
 if node.kind!=Some(FieldType::Button){return Ok(walk.patches);}
 let flags=match walk.attr(forms,field,b"Ff")?{Some(Object::Integer(value))=>*value,Some(Object::Null)|None=>0,_=>return Err(bad("invalid button flags"))};
 require((0..=u32::MAX as i64).contains(&flags),"invalid button flags")?;if flags&65536!=0{return Ok(walk.patches);}
 let originals=if node.widget_page.is_some(){std::slice::from_ref(&field)}else{node.children.as_slice()};
 let options=match walk.attr(forms,field,b"Opt")?{Some(Object::Null)|None=>{walk.named_selection(forms,field,originals,controls)?;return Ok(walk.patches);},Some(value)=>value.as_array().map_err(bad)?};
 require(!controls.is_empty() && controls.len()<=4000 && !originals.is_empty() && options.len()==originals.len(),"button export table does not match controls")?;
 let mut original_index=BTreeMap::new();let mut decoded=Vec::new();let mut checked=Vec::new();
 for (index,&widget) in originals.iter().enumerate(){
  walk.step(0)?;require(forms.nodes.get(&widget).is_some_and(|node|node.widget_page.is_some()),"button child is not a widget")?;
  walk.budget.reserve(128)?;require(original_index.insert(widget,index).is_none(),"repeated original widget")?;
  let raw=walk.resolve(&options[index])?.as_str().map_err(bad)?;
  walk.budget.reserve(raw.len().checked_mul(6).and_then(|n|n.checked_add(64)).ok_or_else(||bad("export text overflow"))?)?;
  decoded.push(crate::form_properties::text(doc,&options[index],raw.len().saturating_mul(3))?);
  checked.push(walk.checked(widget,index)?);
 }
 let mut seen=BTreeSet::new();let mut order=Vec::new();let mut new_options=Vec::new();
 for &(original,output) in controls{
  walk.step(0)?;walk.budget.reserve(96)?;require(output.0!=0 && seen.insert(output),"invalid or repeated output widget")?;
  let index=*original_index.get(&original).ok_or_else(||bad("output widget is not owned by this field"))?;
  order.push(index);new_options.push(walk.copy(&options[index])?);
 }
 let before=checked.iter().position(|value|*value).map(|index|&decoded[index]);
 let after=order.iter().find(|&&index|checked[index]).map(|&index|&decoded[index]);
 require(before==after,"selected answer has no equivalent surviving checked control")?;
 walk.put(output_field,b"Opt",Object::Array(new_options))?;
 for key in [b"V".as_slice(),b"DV"]{
  let Some(value)=walk.attr(forms,field,key)? else{continue;};if matches!(value,Object::Null){continue;}
  let name=value.as_name().map_err(bad)?;walk.budget.reserve(name.len())?;if name==b"Off"{continue;}
  let old=index_name(name,originals.len())?;
  // Prefer the same original control. An equivalent export value can retain
  // a removed control's logical default/value without changing live AP bytes.
  let new=order.iter().position(|index|*index==old).or_else(||order.iter().position(|index|decoded[*index]==decoded[old])).ok_or_else(||bad("answer or default has no equivalent surviving control"))?;
  if old!=new{walk.put(output_field,key,Object::Name(new.to_string().into_bytes()))?;}
 }
 for (new,(&(original,output),&old)) in controls.iter().zip(&order).enumerate(){
  walk.step(0)?;if old==new{continue;}
  if let Some(value)=walk.appearance(original,old,new)?{walk.put(output,b"AP",value)?;}
  if checked[old]{walk.put(output,b"AS",Object::Name(new.to_string().into_bytes()))?;}
 }
 walk.step(0)?;Ok(walk.patches)
}
fn bad(message:impl std::fmt::Display)->EngineError{EngineError::InvalidRequest(format!("PDF indexed button order: {message}"))}
fn require(value:bool,message:&str)->EngineResult<()>{if value{Ok(())}else{Err(bad(message))}}
fn index_name(name:&[u8],count:usize)->EngineResult<usize>{
 let text=std::str::from_utf8(name).map_err(bad)?;let index=text.parse::<usize>().map_err(bad)?;
 require(index<count && text==index.to_string(),"invalid indexed button state")?;Ok(index)
}
struct Walk<'a,'b>{doc:&'a Document,budget:&'b mut Budget,check:&'b mut dyn FnMut()->EngineResult<()>,patches:Patches}
impl<'a> Walk<'a,'_>{
 fn named_selection(&mut self,forms:&FormInfo,field:ObjectId,originals:&[ObjectId],controls:&[(ObjectId,ObjectId)])->EngineResult<()>{
  require(!originals.is_empty() && !controls.is_empty() && controls.len()<=4000,"invalid named button selection")?;
  let mut retained=BTreeSet::new();let mut outputs=BTreeSet::new();
  for &(old,new) in controls{self.step(0)?;self.budget.reserve(128)?;require(new.0!=0 && outputs.insert(new) && originals.contains(&old),"invalid named button control")?;retained.insert(old);}
  let mut required=BTreeSet::new();
  for key in [b"V".as_slice(),b"DV"]{if let Some(value)=self.attr(forms,field,key)?{if !matches!(value,Object::Null){let name=value.as_name().map_err(bad)?;self.budget.reserve(name.len()+64)?;if name!=b"Off"{required.insert(name.to_vec());}}}}
  let mut before=BTreeSet::new();let mut after=BTreeSet::new();let mut available=BTreeSet::new();
  for &widget in originals{
   self.step(0)?;require(forms.nodes.get(&widget).is_some_and(|node|node.widget_page.is_some()),"named button child is not a widget")?;
   let dict=self.doc.get_dictionary(widget).map_err(bad)?;let state=self.optional(dict,b"AS")?.map(Object::as_name).transpose().map_err(bad)?;
   if let Some(state)=state.filter(|state|*state!=b"Off"){
    self.budget.reserve(state.len().checked_mul(3).and_then(|n|n.checked_add(192)).ok_or_else(||bad("button state size overflow"))?)?;before.insert(state.to_vec());if retained.contains(&widget){after.insert(state.to_vec());}
   }
   if retained.contains(&widget) && (!required.is_empty() || state.is_some_and(|state|state!=b"Off")){
    let ap=self.optional(dict,b"AP")?.ok_or_else(||bad("surviving button lacks state appearances"))?.as_dict().map_err(bad)?;
    let normal=self.optional(ap,b"N")?.ok_or_else(||bad("surviving button lacks normal states"))?.as_dict().map_err(bad)?;
    require(normal.len()==2 && normal.has(b"Off"),"surviving button needs one on state and Off")?;
    for (name,value) in normal.iter(){self.step(0)?;self.budget.reserve(name.len()+64)?;require(matches!(self.resolve(value)?,Object::Stream(_)),"named button state is not a stream")?;if name!=b"Off"{available.insert(name.clone());}}
    require(state.map_or(true,|name|normal.has(name)),"saved named appearance state is missing")?;
   }
  }
  require(before==after,"selected answer has no equivalent surviving checked control")?;
  require(required.is_subset(&available),"answer or default has no equivalent surviving named control")?;
  self.step(0)
 }
 fn step(&mut self,depth:usize)->EngineResult<()>{self.budget.step(depth,self.check)}
 fn resolve<'v>(&mut self,mut value:&'v Object)->EngineResult<&'v Object> where 'a:'v{
  for depth in 0..=64{self.step(depth)?;match value{Object::Reference(id)=>value=self.doc.objects.get(id).ok_or_else(||bad("missing reference"))?,_=>return Ok(value)}}Err(bad("cyclic or excessive aliases"))
 }
 fn attr(&mut self,forms:&FormInfo,mut id:ObjectId,key:&[u8])->EngineResult<Option<&'a Object>>{
  // PDF null is absence, including an indirect null. Keep looking for the
  // effective Opt/Ff/V/DV instead of letting a local null hide an ancestor.
  for depth in 0..=64{self.step(depth)?;let dict=self.doc.get_dictionary(id).map_err(bad)?;if let Some(value)=self.optional(dict,key)?{return Ok(Some(value));}
   match forms.nodes.get(&id).ok_or_else(||bad("missing attribute owner"))?.parent{Some(parent)=>id=parent,None=>return Ok(None)}
  }Err(bad("attribute inheritance exceeds limit"))
 }
 fn optional(&mut self,dict:&'a Dictionary,key:&[u8])->EngineResult<Option<&'a Object>>{match dict.get(key){Ok(value)=>{let value=self.resolve(value)?;Ok((!matches!(value,Object::Null)).then_some(value))},Err(_)=>Ok(None)}}
 fn copy(&mut self,value:&Object)->EngineResult<Object>{crate::composition_form_resources::copy_metadata(self.doc,value,self.budget,&mut self.check)}
 fn put(&mut self,id:ObjectId,key:&[u8],value:Object)->EngineResult<()>{self.step(0)?;self.budget.reserve(128+key.len())?;require(self.patches.insert((id,key.to_vec()),value).is_none(),"conflicting button patches")}
 fn checked(&mut self,widget:ObjectId,index:usize)->EngineResult<bool>{
  let dict=self.doc.get_dictionary(widget).map_err(bad)?;let state=self.optional(dict,b"AS")?.map(Object::as_name).transpose().map_err(bad)?;
  if let Some(name)=state{self.budget.reserve(name.len())?;}
  let on=index.to_string();require(state.map_or(true,|name|name==b"Off" || name==on.as_bytes()),"appearance state does not match indexed control")?;
  if let Some(ap)=self.optional(dict,b"AP")?{
   let ap=ap.as_dict().map_err(bad)?;
   if let Some(normal)=self.optional(ap,b"N")?{let states=normal.as_dict().map_err(bad)?;self.states(states,index)?;require(states.has(b"Off") && states.has(on.as_bytes()),"normal button appearance lacks indexed on/off states")?;}
   else{require(state!=Some(on.as_bytes()),"checked control has no normal appearance")?;}
  }else{require(state!=Some(on.as_bytes()),"checked control has no appearance")?;}
  Ok(state==Some(on.as_bytes()))
 }
 fn states(&mut self,states:&Dictionary,index:usize)->EngineResult<()>{
  let on=index.to_string();for (key,value) in states.iter(){self.step(0)?;self.budget.reserve(key.len())?;require(key==b"Off" || key==on.as_bytes(),"unsupported indexed appearance name")?;require(matches!(self.resolve(value)?,Object::Stream(_)),"appearance state is not a stream")?;}Ok(())
 }
 fn appearance(&mut self,widget:ObjectId,old:usize,new:usize)->EngineResult<Option<Object>>{
  let dict=self.doc.get_dictionary(widget).map_err(bad)?;let Some(ap)=self.optional(dict,b"AP")? else{return Ok(None);};let ap=ap.as_dict().map_err(bad)?;let mut out=Dictionary::new();
  let old_name=old.to_string();let new=new.to_string();
  for (key,value) in ap.iter(){
   self.step(0)?;self.budget.reserve(key.len()+96)?;let resolved=self.resolve(value)?;
   if matches!(resolved,Object::Null){out.set(key.clone(),self.copy(value)?);continue;}
   require([b"N".as_slice(),b"R",b"D"].contains(&key.as_slice()),"unknown appearance role")?;
   if let Object::Dictionary(states)=resolved{
    self.states(states,old)?;let mut changed=Dictionary::new();
    for (name,value) in states.iter(){self.step(0)?;self.budget.reserve(name.len()+new.len()+96)?;changed.set(if name==old_name.as_bytes(){new.as_bytes().to_vec()}else{name.clone()},self.copy(value)?);}
    out.set(key.clone(),changed);
   }else{require(key!=b"N" && matches!(resolved,Object::Stream(_)),"invalid button appearance")?;out.set(key.clone(),self.copy(value)?);}
  }
  Ok(Some(out.into()))
 }
}
