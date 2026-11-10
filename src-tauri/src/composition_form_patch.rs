//! Staged identity-preserving field defaults and local resource aliases.
#![allow(dead_code)]
use crate::{composition_form_budget::Budget,composition_form_defaults::{Inventory,Origin,ValueAt},composition_forms::FormInfo,EngineError,EngineResult};
use lopdf::{Dictionary,Document,Object,ObjectId};
use std::{collections::{BTreeMap,BTreeSet},sync::Arc};
pub(crate) struct Source<'a>{pub forms:&'a FormInfo,pub defaults:&'a Inventory<'a>,pub aliases:&'a BTreeMap<Vec<u8>,Vec<u8>>}
pub(crate) struct Patches {pub entries:BTreeMap<(ObjectId,Vec<u8>),Object>,pub global_da:Option<Object>}
/// Sources and reservations describe this same immutable private imported
/// graph. The caller must retain global_da when assembling the new AcroForm.
pub(crate) fn stage(doc:&Document,sources:&[Source<'_>],forbidden:&BTreeSet<Vec<u8>>,budget:&mut Budget,cancelled:Arc<dyn Fn()->bool+Send+Sync>,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Patches>{
    let mut walk=Walk{doc,budget,cancelled,check,patches:Patches{entries:BTreeMap::new(),global_da:None},required_da:None,resource_targets:BTreeMap::new()};
    let mut fields=BTreeSet::new();
    for (index,source) in sources.iter().enumerate() {
        walk.step()?;let mut output_names=BTreeSet::new();
        for (name,alias) in source.aliases {
            walk.step()?;walk.budget.reserve(alias.len().checked_add(64).ok_or_else(||bad("alias size"))?)?;
            require(!name.is_empty() && !alias.is_empty() && output_names.insert(alias.as_slice()),"ambiguous font map")?;
            require(name==alias || !forbidden.contains(alias),"font alias is not fresh in the source graph")?;
        }
        require(source.forms.nodes.keys().eq(source.defaults.nodes.keys()),"defaults do not match field inventory")?;
        for &id in source.forms.nodes.keys() {walk.step()?;walk.budget.reserve(64)?;require(fields.insert(id),"source field identities overlap")?;}
        let mut rewritten=BTreeMap::new();
        for (&origin,program) in &source.defaults.programs {
            walk.step()?;walk.budget.reserve(96)?;
            let bytes=crate::composition_form_da::rewrite(program.bytes,source.aliases,walk.budget,walk.cancelled.clone(),&mut walk.check)?;
            let value=match bytes {
                Some(bytes)=>{let format=match walk.resolve(program.raw)? {Object::String(_,format)=>*format,_=>return Err(bad("rewritten DA is not a string"))};Some(Object::String(bytes,format))},
                None=>None,
            };
            if let (Origin::Field(id),Some(value))=(origin,&value) {
                require(source.forms.nodes.contains_key(&id),"unbound DA owner")?;
                let value=walk.copy(value)?;walk.put(id,b"DA",value)?;
            }
            rewritten.insert(origin,value);
        }
        for &root in &source.forms.roots {
            walk.step()?;let defaults=source.defaults.nodes.get(&root).ok_or_else(||bad("missing root defaults"))?;
            if let Some(origin@Origin::Form(_))=defaults.control_da {
                require(!doc.get_dictionary(root).map_err(bad)?.has(b"DA"),"root DA provenance changed")?;
                let value=rewritten[&origin].as_ref().unwrap_or(source.defaults.programs[&origin].raw);
                let value=walk.copy(value)?;walk.put(root,b"DA",value)?;
            }
            if let Some(ValueAt{origin:Origin::Form(_),value})=defaults.alignment {
                require(!doc.get_dictionary(root).map_err(bad)?.has(b"Q"),"root alignment provenance changed")?;
                let value=walk.copy(value)?;walk.put(root,b"Q",value)?;
            }
        }
        for (&id,defaults) in &source.defaults.nodes {
            walk.step()?;
            // Materializing a nonempty form DA at the root gives every ordinary
            // inheriting field its original program. An explicit empty/null DA
            // still skips to the *global* form default when filled, however.
            // Keep that default, or refuse incompatible requirements; changing
            // the empty control into a nonempty one would alter its metadata.
            match defaults.fill_da {
                None=>walk.require_global(b"",None)?,
                Some(origin@Origin::Form(_))=>{
                    let program=&source.defaults.programs[&origin];
                    if defaults.control_da!=Some(origin) || program.bytes.is_empty() {
                        let value=rewritten[&origin].as_ref().unwrap_or(program.raw);
                        let bytes=match walk.resolve(value)? {Object::Null=>b"".as_slice(),Object::String(bytes,_)=>bytes.as_slice(),_=>return Err(bad("invalid fallback DA"))};
                        walk.require_global(bytes,Some(value))?;
                    }
                }
                Some(Origin::Field(_))=>{},
                _=>return Err(bad("unsupported fill origin")),
            }
            if !defaults.fonts.keys().any(|name|source.aliases.get(name).is_some_and(|alias|alias!=name)){continue;}
            if let Some(value)=defaults.field_resources {
                let Origin::Field(owner)=value.origin else{return Err(bad("invalid field resource owner"));};
                walk.local(index,owner,b"DR",value.value,source.aliases)?;
            }
            if let Some(value)=defaults.normal_resources {
                let Origin::Appearance(owner)=value.origin else{return Err(bad("invalid appearance resource owner"));};
                walk.local(index,owner,b"Resources",value.value,source.aliases)?;
            }
            if let Some(value)=defaults.page_resources {
                let page=source.forms.nodes[&id].widget_page.ok_or_else(||bad("page scope has no owning widget"))?;
                // A private page dictionary gets the inherited resources. Never
                // add aliases on its shared page-tree ancestor.
                walk.local(index,page,b"Resources",value.value,source.aliases)?;
            }
        }
    }
    walk.step()?;Ok(walk.patches)
}
/// Consume the same private graph used for staging. An error/cancellation drops
/// it, so no partially edited document is returned. Stream dictionaries change
/// in place; appearance bytes and object identities are retained.
pub(crate) fn apply(mut doc:Document,patches:Patches,budget:&mut Budget,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Document>{
    for ((id,key),_) in &patches.entries {
        budget.step(0,check)?;
        let object=doc.objects.get(id).ok_or_else(||bad("missing patch target"))?;
        match object {
            Object::Dictionary(_)=>require([b"DA".as_slice(),b"Q",b"DR",b"Resources"].contains(&key.as_slice()),"unsupported dictionary patch")?,
            Object::Stream(_)=>require(key==b"Resources","unsupported stream patch")?,
            _=>return Err(bad("patch target changed shape")),
        }
    }
    for ((id,key),value) in patches.entries {
        budget.step(0,check)?;
        match doc.objects.get_mut(&id).ok_or_else(||bad("missing patch target"))? {
            Object::Dictionary(dict)=>dict.set(key,value),Object::Stream(stream)=>stream.dict.set(key,value),
            _=>return Err(bad("patch target changed shape")),
        }
    }
    budget.step(0,check)?;Ok(doc)
}
fn bad(message:impl std::fmt::Display)->EngineError{EngineError::InvalidRequest(format!("PDF form default patches: {message}"))}
fn require(value:bool,message:&str)->EngineResult<()>{if value{Ok(())}else{Err(bad(message))}}
struct Walk<'a,'b>{doc:&'a Document,budget:&'b mut Budget,cancelled:Arc<dyn Fn()->bool+Send+Sync>,check:&'b mut dyn FnMut()->EngineResult<()>,patches:Patches,required_da:Option<Vec<u8>>,resource_targets:BTreeMap<(ObjectId,Vec<u8>),usize>}
impl<'a> Walk<'a,'_>{
    fn step(&mut self)->EngineResult<()>{if (self.cancelled)(){return Err(bad("cancelled"));}self.budget.step(0,self.check)}
    fn resolve<'v>(&mut self,mut value:&'v Object)->EngineResult<&'v Object> where 'a:'v {
        for _ in 0..=64 {self.step()?;match value {Object::Reference(id)=>value=self.doc.objects.get(id).ok_or_else(||bad("missing reference"))?,_=>return Ok(value)}}Err(bad("cyclic or excessive aliases"))
    }
    fn copy(&mut self,value:&Object)->EngineResult<Object>{
        let cancelled=self.cancelled.clone();let check=&mut *self.check;
        crate::composition_form_resources::copy_metadata(self.doc,value,self.budget,&mut ||{if cancelled(){return Err(bad("cancelled"));}check()})
    }
    fn put(&mut self,id:ObjectId,key:&[u8],value:Object)->EngineResult<()>{
        self.step()?;self.budget.reserve(key.len()+96)?;
        require(self.patches.entries.insert((id,key.to_vec()),value).is_none(),"conflicting patch targets")
    }
    fn require_global(&mut self,bytes:&[u8],value:Option<&Object>)->EngineResult<()>{
        self.step()?;self.budget.reserve(bytes.len())?;
        if let Some(required)=&self.required_da {return require(required==bytes,"incompatible global fallbacks for empty field appearances");}
        self.budget.reserve(bytes.len().checked_add(64).ok_or_else(||bad("fallback size"))?)?;
        self.required_da=Some(bytes.to_vec());
        if !bytes.is_empty(){self.patches.global_da=Some(self.copy(value.ok_or_else(||bad("missing fallback program"))?)?);}Ok(())
    }
    fn local(&mut self,index:usize,id:ObjectId,key:&[u8],value:&Object,aliases:&BTreeMap<Vec<u8>,Vec<u8>>)->EngineResult<()>{
        self.step()?;self.budget.reserve(key.len()+96)?;let target=(id,key.to_vec());
        if let Some(previous)=self.resource_targets.get(&target){return require(*previous==index,"local resource owner shared across source maps");}
        self.resource_targets.insert(target,index);
        let value=self.resolve(value)?;if matches!(value,Object::Null){return Ok(());}
        let resources=value.as_dict().map_err(bad)?;
        let Some(fonts)=resources.get(b"Font").ok() else{return Ok(());};
        let fonts=self.resolve(fonts)?;if matches!(fonts,Object::Null){return Ok(());}
        let fonts=fonts.as_dict().map_err(bad)?;let mut additions=Vec::new();
        for (old,new) in aliases {
            self.step()?;self.budget.reserve(old.len().checked_add(new.len()).ok_or_else(||bad("alias size"))?)?;
            if old==new{continue;}
            if let Ok(font)=fonts.get(old) {
                require(!fonts.has(new),"local alias would replace an existing binding")?;
                self.budget.reserve(64)?;additions.push((new,font));
            }
        }
        if additions.is_empty(){return Ok(());}
        // Copy just dictionary metadata and indirect edges. Font/appearance
        // payload streams remain in their original objects.
        let mut out=Dictionary::new();
        for (name,value) in resources.iter(){
            self.step()?;self.budget.reserve(name.len()+64)?;
            if name!=b"Font"{out.set(name.clone(),self.copy(value)?);}
        }
        let mut out_fonts=Dictionary::new();
        for (name,value) in fonts.iter(){self.step()?;self.budget.reserve(name.len()+64)?;out_fonts.set(name.clone(),self.copy(value)?);}
        for (name,value) in additions{self.step()?;self.budget.reserve(name.len()+64)?;out_fonts.set(name.clone(),self.copy(value)?);}
        out.set("Font",out_fonts);self.put(id,key,Object::Dictionary(out))
    }
}
#[cfg(test)]
#[path="composition_form_patch_tests.rs"]
mod tests;
