//! Source-bound standard form-action targets. This module never executes actions.
#![allow(dead_code)]
use crate::{composition_form_assembly::Namespace,composition_form_budget::Budget,composition_forms::FormInfo,composition_pages::SourcePages,EngineError,EngineResult};
use lopdf::{Dictionary,Document,Object,ObjectId};
use std::collections::{BTreeMap,BTreeSet};
pub(crate) struct Scope<'a>{pub forms:&'a FormInfo,namespace:&'a Namespace,names:BTreeMap<&'a str,ObjectId>,annotations:BTreeMap<ObjectId,ObjectId>,terminals:BTreeSet<ObjectId>,foreign_roots:Vec<ObjectId>,retained:Option<&'a BTreeSet<ObjectId>>,pages:Option<&'a BTreeSet<ObjectId>>,field_ids:Option<&'a BTreeMap<ObjectId,ObjectId>>}
pub(crate) struct Changes {pub action:BTreeMap<Vec<u8>,Object>,pub fields:BTreeMap<(ObjectId,Vec<u8>),Object>}
pub(crate) fn select<'a>(mut scope:Scope<'a>,retained:&'a BTreeSet<ObjectId>,pages:&'a BTreeSet<ObjectId>)->Scope<'a>{scope.retained=Some(retained);scope.pages=Some(pages);scope}
pub(crate) fn rebind<'a>(mut scope:Scope<'a>,ids:&'a BTreeMap<ObjectId,ObjectId>)->Scope<'a>{scope.field_ids=Some(ids);scope}
impl Scope<'_>{fn output_field(&self,id:ObjectId)->ObjectId{self.field_ids.and_then(|ids|ids.get(&id)).copied().unwrap_or(id)}pub(crate) fn retains_field(&self,id:ObjectId)->bool{self.retained.map_or(true,|ids|ids.contains(&id))}}
pub(crate) fn bind<'a>(doc:&Document,pages:&SourcePages,forms:&'a FormInfo,namespace:&'a Namespace,budget:&mut Budget,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Scope<'a>>{
    let mut walk=Walk{doc,budget,check};let mut names=BTreeMap::new();let mut annotations=BTreeMap::new();let mut terminals=BTreeSet::new();
    let mut roots=BTreeSet::new();let mut foreign_roots=Vec::new();
    for &id in &forms.roots{walk.step(0)?;walk.budget.reserve(64)?;roots.insert(id);}
    for &id in &namespace.roots{walk.step(0)?;if !roots.contains(&id){walk.budget.reserve(32)?;foreign_roots.push(id);}}
    for (&id,node) in &forms.nodes {
        walk.step(0)?;require(namespace.names.contains_key(&id),"field has no staged name")?;
        if !node.field{continue;}
        if node.partial_name.is_some(){walk.budget.reserve(node.full_name.len()+64)?;require(names.insert(node.full_name.as_str(),id).is_none(),"ambiguous source field name")?;}
        let mut terminal=true;
        for child in &node.children{walk.step(0)?;terminal&=!forms.nodes.get(child).ok_or_else(||bad("missing child"))?.field;}
        if terminal{walk.budget.reserve(64)?;terminals.insert(id);}
    }
    for page in &pages.pages {
        walk.step(0)?;let dict=doc.get_dictionary(*page).map_err(bad)?;
        if let Some(value)=walk.optional(dict,b"Annots")? {
            for value in value.as_array().map_err(bad)? {
                walk.step(0)?;let (id,value)=walk.resolve(value)?;value.as_dict().map_err(bad)?;
                if let Some(id)=id{walk.budget.reserve(96)?;require(annotations.insert(id,*page).is_none(),"annotation belongs to multiple source pages")?;}
            }
        }
    }
    walk.step(0)?;Ok(Scope{forms,namespace,names,annotations,terminals,foreign_roots,retained:None,pages:None,field_ids:None})
}
/// Only call for a typed action dictionary reached through A/AA/Next roles.
pub(crate) fn rewrite(doc:&Document,scope:&Scope<'_>,action:&Dictionary,budget:&mut Budget,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Changes>{
    let mut walk=Walk{doc,budget,check};walk.step(0)?;
    let kind=walk.resolve(action.get(b"S").map_err(bad)?)?.1.as_name().map_err(bad)?;
    require([b"Hide".as_slice(),b"ResetForm",b"SubmitForm"].contains(&kind),"unsupported form action")?;
    if let Some(value)=walk.optional(action,b"Type")?{require(value.as_name().map_err(bad)?==b"Action","invalid action Type")?;}
    for (key,value) in action.iter(){
        walk.step(0)?;walk.budget.reserve(key.len())?;
        if matches!(walk.resolve(value)?.1,Object::Null){continue;}
        let allowed=match kind{b"Hide"=>[b"Type".as_slice(),b"S",b"Next",b"T",b"H"].contains(&key.as_slice()),b"ResetForm"=>[b"Type".as_slice(),b"S",b"Next",b"Fields",b"Flags"].contains(&key.as_slice()),_=>[b"Type".as_slice(),b"S",b"Next",b"Fields",b"Flags",b"F"].contains(&key.as_slice())};
        require(allowed,"unsupported form action entry")?;
    }
    let mut changes=Changes{action:BTreeMap::new(),fields:BTreeMap::new()};
    if kind==b"Hide" {
        if let Some(value)=walk.optional(action,b"H")?{value.as_bool().map_err(bad)?;}
        let raw=action.get(b"T").map_err(bad)?;let value=walk.resolve(raw)?.1;
        let target=if let Object::Array(values)=value {
            let mut out=Vec::new();for value in values{walk.budget.reserve(32)?;if let Some(value)=walk.hide(scope,value)?{out.push(value);}}Object::Array(out)
        }else{walk.hide(scope,raw)?.unwrap_or_else(||Object::Array(Vec::new()))};
        walk.action_patch(&mut changes,b"T",target)?;return Ok(changes);
    }
    let submit=kind==b"SubmitForm";
    let flags=walk.optional(action,b"Flags")?.map(Object::as_i64).transpose().map_err(bad)?.unwrap_or(0);
    require((0..=u32::MAX as i64).contains(&flags),"invalid action flags")?;
    require(flags & !(if submit{0x2fff}else{1})==0,"unknown action flag bits")?;
    let fdf=submit && flags&(4|32)==0;
    if submit {
        let file=walk.optional(action,b"F")?.ok_or_else(||bad("missing submission destination"))?;
        require(matches!(file,Object::String(..)|Object::Dictionary(_)),"invalid submission destination")?;
        // Whole-document, annotation and incremental-update payloads cannot be
        // confined by a Fields array. They need a separate document adapter.
        require(flags&256==0 && !(fdf && flags&(64|128|8192)!=0),"submission carries document data outside the source field domain")?;
        require(flags&4!=0 || flags&(8|16)==0,"HTML-only submission flags without HTML format")?;
    }
    let original=walk.optional(action,b"Fields")?;
    let mut output=Vec::new();let mut mentioned=BTreeSet::new();let mut selected=BTreeSet::new();
    if let Some(value)=original {
        for value in value.as_array().map_err(bad)? {
            walk.step(0)?;let (replacement,id)=walk.field_target(scope,value)?;
            if scope.retains_field(id){walk.budget.reserve(32)?;output.push(replacement);walk.descendants(scope,id,&mut mentioned,0)?;}
        }
        if flags&1!=0 {
            for &id in &scope.terminals {walk.step(0)?;if scope.retains_field(id) && !mentioned.contains(&id){walk.budget.reserve(64)?;selected.insert(id);}}
            // Preserve the original exclusion expression and exclude the other
            // source roots too, including their complete descendant subtrees.
            for &id in &scope.foreign_roots{walk.step(0)?;walk.budget.reserve(32)?;output.push(Object::Reference(scope.output_field(id)));}
        }else{selected=mentioned;}
    }else{
        for &id in &scope.terminals{walk.step(0)?;if scope.retains_field(id){walk.budget.reserve(64)?;selected.insert(id);}}
        if fdf {
            // PDF 1.7 p707: omitting Fields in FDF also omits pushbuttons.
            // Making Fields explicit must not accidentally start exporting AP.
            for &id in &scope.terminals {
                if scope.forms.nodes[&id].kind==Some(crate::composition_forms::FieldType::Button) && walk.field_flags(scope,id)?&65536!=0 {selected.remove(&id);}
            }
            for &id in &selected{walk.step(0)?;walk.budget.reserve(32)?;output.push(Object::Reference(scope.output_field(id)));}
        }else{
            for &id in &scope.forms.roots{walk.step(0)?;if scope.retains_field(id){walk.budget.reserve(32)?;output.push(Object::Reference(scope.output_field(id)));}}
        }
        walk.action_patch(&mut changes,b"Flags",Object::Integer(flags&!1))?;
    }
    walk.action_patch(&mut changes,b"Fields",Object::Array(output))?;
    if submit {
        for id in selected {
            walk.step(0)?;
            if walk.field_flags(scope,id)?&4!=0{continue;}
            let old=&scope.forms.nodes[&id].full_name;let new=&scope.namespace.names[&id];
            if old==new{continue;}
            let dict=doc.get_dictionary(id).map_err(bad)?;
            if let Some(value)=walk.optional(dict,b"TM")? {
                let bytes=value.as_str().map_err(bad)?;walk.budget.reserve(bytes.len())?;
                // Existing TM is retained verbatim, not decoded as a lookup
                // target. The shared metadata budget bounds this inspection.
            }else{
                // TM is not inherited. Preserve the original exported name on
                // each selected terminal whose qualified T name changes.
                require(!old.is_empty(),"cannot preserve an unnamed exported field")?;
                let value=walk.text(old)?;walk.budget.reserve(96)?;changes.fields.insert((id,b"TM".to_vec()),value);
            }
        }
    }
    walk.step(0)?;Ok(changes)
}
fn bad(message:impl std::fmt::Display)->EngineError{EngineError::InvalidRequest(format!("PDF form action targets: {message}"))}
fn require(value:bool,message:&str)->EngineResult<()>{if value{Ok(())}else{Err(bad(message))}}
struct Walk<'a,'b>{doc:&'a Document,budget:&'b mut Budget,check:&'b mut dyn FnMut()->EngineResult<()>}
impl<'a> Walk<'a,'_>{
    fn step(&mut self,depth:usize)->EngineResult<()>{self.budget.step(depth,self.check)}
    fn resolve<'v>(&mut self,mut value:&'v Object)->EngineResult<(Option<ObjectId>,&'v Object)> where 'a:'v {
        let mut id=None;
        for depth in 0..=64{self.step(depth)?;match value{Object::Reference(next)=>{id=Some(*next);value=self.doc.objects.get(next).ok_or_else(||bad("missing target object"))?;},_=>return Ok((id,value))}}Err(bad("cyclic or excessive aliases"))
    }
    fn optional<'v>(&mut self,dict:&'v Dictionary,key:&[u8])->EngineResult<Option<&'v Object>> where 'a:'v {
        dict.get(key).ok().map(|value|self.resolve(value).map(|(_,value)|if matches!(value,Object::Null){None}else{Some(value)})).transpose().map(Option::flatten)
    }
    fn copy(&mut self,value:&Object)->EngineResult<Object>{crate::composition_form_resources::copy_metadata(self.doc,value,self.budget,&mut self.check)}
    fn text(&mut self,value:&str)->EngineResult<Object>{self.budget.reserve(value.len().checked_mul(2).and_then(|n|n.checked_add(64)).ok_or_else(||bad("name overflow"))?)?;Ok(lopdf::text_string(value))}
    fn action_patch(&mut self,out:&mut Changes,key:&[u8],value:Object)->EngineResult<()>{self.step(0)?;self.budget.reserve(key.len()+96)?;out.action.insert(key.to_vec(),value);Ok(())}
    fn named(&mut self,scope:&Scope<'_>,value:&Object)->EngineResult<ObjectId>{
        let raw=self.resolve(value)?.1.as_str().map_err(bad)?;
        self.budget.reserve(raw.len().checked_mul(3).and_then(|n|n.checked_add(64)).ok_or_else(||bad("name size overflow"))?)?;
        let name=crate::form_properties::text(self.doc,value,32768)?;
        scope.names.get(name.as_str()).copied().ok_or_else(||bad("field name is outside source form"))
    }
    fn field_target(&mut self,scope:&Scope<'_>,value:&Object)->EngineResult<(Object,ObjectId)>{
        self.step(0)?;let (id,resolved)=self.resolve(value)?;
        match resolved {
            Object::String(..)=>{
                let id=self.named(scope,value)?;let old=&scope.forms.nodes[&id].full_name;let new=&scope.namespace.names[&id];
                Ok((if old==new{self.copy(value)?}else{self.text(new)?},id))
            }
            Object::Dictionary(_)=>{
                let id=id.ok_or_else(||bad("field target requires indirect identity"))?;
                require(scope.forms.nodes.get(&id).is_some_and(|node|node.field),"field target is outside source field hierarchy")?;
                Ok((if scope.output_field(id)==id{self.copy(value)?}else{Object::Reference(scope.output_field(id))},id))
            }
            _=>Err(bad("invalid field target")),
        }
    }
    fn hide(&mut self,scope:&Scope<'_>,value:&Object)->EngineResult<Option<Object>>{
        self.step(0)?;let (id,resolved)=self.resolve(value)?;
        match resolved {
            Object::String(..)=>self.field_target(scope,value).map(|(value,id)|scope.retains_field(id).then_some(value)),
            Object::Dictionary(_)=>{let page=id.and_then(|id|scope.annotations.get(&id)).ok_or_else(||bad("Hide target is outside source annotations"))?;if scope.pages.is_some_and(|pages|!pages.contains(page)){Ok(None)}else{self.copy(value).map(Some)}},
            _=>Err(bad("invalid Hide target")),
        }
    }
    fn descendants(&mut self,scope:&Scope<'_>,id:ObjectId,selected:&mut BTreeSet<ObjectId>,depth:usize)->EngineResult<()>{
        self.step(depth)?;if !scope.retains_field(id){return Ok(());}let node=scope.forms.nodes.get(&id).ok_or_else(||bad("missing field descendant"))?;
        if scope.terminals.contains(&id){if !selected.contains(&id){self.budget.reserve(64)?;selected.insert(id);}return Ok(());}
        for child in &node.children{self.descendants(scope,*child,selected,depth+1)?;}Ok(())
    }
    fn field_flags(&mut self,scope:&Scope<'_>,mut id:ObjectId)->EngineResult<i64>{
        for depth in 0..=64 {
            self.step(depth)?;let dict=self.doc.get_dictionary(id).map_err(bad)?;
            if let Ok(value)=dict.get(b"Ff"){
                let value=self.resolve(value)?.1;if matches!(value,Object::Null){return Ok(0);}
                let flags=value.as_i64().map_err(bad)?;require((0..=u32::MAX as i64).contains(&flags),"invalid inherited field flags")?;return Ok(flags);
            }
            match scope.forms.nodes.get(&id).ok_or_else(||bad("missing field flag owner"))?.parent{Some(parent)=>id=parent,None=>return Ok(0)}
        }Err(bad("field flags exceed inheritance limit"))
    }
}
#[cfg(test)]
#[path="composition_form_actions_tests.rs"]
mod tests;
