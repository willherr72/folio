//! Global resource staging; local scope aliases and DA publication follow later.
#![allow(dead_code)]
use crate::{composition_form_budget::Budget,composition_form_defaults::Inventory,EngineError,EngineResult};
use lopdf::{Dictionary,Document,Object,ObjectId};
use std::collections::{BTreeMap,BTreeSet};
pub(crate) struct Source<'a> {pub catalog:ObjectId,pub defaults:&'a Inventory<'a>}
#[derive(Debug)]
pub(crate) struct Resources {
    pub dictionary:Dictionary,
    pub font_names:Vec<BTreeMap<Vec<u8>,Vec<u8>>>,
}
/// Copy bounded metadata while retaining indirect edges; never duplicate a
/// direct stream payload. Shared by global staging and local resource patches.
pub(crate) fn copy_metadata(doc:&Document,value:&Object,budget:&mut Budget,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Object> {
    Walk{doc,budget,check}.copy(value,0)
}
pub(crate) fn stage(doc:&Document,sources:&[Source<'_>],budget:&mut Budget,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Resources> {
    stage_reserved(doc,sources,&BTreeSet::new(),budget,check)
}
/// The caller supplies local resource/content reservations from its bounded
/// scope inspection. An empty set is useful for isolated global staging, but
/// does not establish freshness for applying aliases to local font tables.
pub(crate) fn stage_reserved(doc:&Document,sources:&[Source<'_>],forbidden:&BTreeSet<Vec<u8>>,budget:&mut Budget,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Resources> {
    let mut walk=Walk{doc,budget,check};walk.step(0)?;
    require(sources.len()<=100_000,"too many sources")?;
    let mut dictionaries=Vec::new();let mut source_names=Vec::new();
    let mut reserved=BTreeSet::new();let mut unbound=BTreeSet::new();let mut catalogs=BTreeSet::new();
    for source in sources {
        walk.step(0)?;walk.budget.reserve(256)?;
        let object=doc.objects.get(&source.catalog).ok_or_else(||bad("missing catalog"))?;
        let (id,catalog)=walk.resolve(object)?;
        require(catalogs.insert(id.unwrap_or(source.catalog)),"repeated source catalog")?;
        let form=walk.optional_dict(catalog.as_dict().map_err(bad)?,b"AcroForm")?;
        if form.is_none(){require(source.defaults.programs.is_empty() && source.defaults.nodes.is_empty(),"defaults have no source form")?;}
        let resources=form.map(|form|walk.optional_dict(form,b"DR")).transpose()?.flatten();
        let fonts=resources.map(|dict|walk.optional_dict(dict,b"Font")).transpose()?.flatten();
        let mut names=BTreeSet::new();
        for name in fonts.into_iter().flat_map(|fonts|fonts.iter().map(|(name,_)|name.as_slice()))
            .chain(source.defaults.programs.values().flat_map(|program|program.font_names.iter().map(Vec::as_slice))) {
            walk.step(0)?;walk.budget.reserve(name.len())?;
            if !names.contains(name){walk.budget.reserve(size(name.len(),96)?)?;names.insert(name);}
            if !reserved.contains(name){walk.budget.reserve(size(name.len(),64)?)?;reserved.insert(name);}
            if walk.font(fonts,name)?.is_none() && !unbound.contains(name) {
                walk.budget.reserve(size(name.len(),64)?)?;unbound.insert(name);
            }
        }
        dictionaries.push((resources,fonts));source_names.push(names);
    }
    let mut result=Resources{dictionary:Dictionary::new(),font_names:Vec::new()};
    let mut claimed=BTreeSet::new();let mut merged_fonts=Dictionary::new();let mut has_fonts=false;
    let mut procsets=Vec::new();let mut known_procsets=BTreeSet::new();let mut has_procsets=false;
    for (index,((resources,fonts),names)) in dictionaries.into_iter().zip(source_names).enumerate() {
        walk.step(0)?;walk.budget.reserve(128)?;
        let mut aliases=BTreeMap::new();let mut counter=1usize;
        for name in names {
            walk.step(0)?;walk.budget.reserve(size(name.len(),96)?)?;
            let binding=walk.font(fonts,name)?;
            // Missing global bindings take precedence over source order. Keep
            // those original names available to unchanged field/page/AP scopes;
            // only sources providing a global font need a fresh alias.
            let alias=if binding.is_some() && (unbound.contains(name) || claimed.contains(name)) {
                loop {
                    walk.step(0)?;walk.budget.reserve(96)?;
                    let candidate=format!("Folio{}_F{}",index+1,counter).into_bytes();
                    counter=counter.checked_add(1).ok_or_else(||bad("alias overflow"))?;
                    if !reserved.contains(candidate.as_slice()) && !claimed.contains(candidate.as_slice()) && !forbidden.contains(&candidate){break candidate;}
                }
            }else{name.to_vec()};
            walk.budget.reserve(size(alias.len(),64)?)?;claimed.insert(alias.clone());
            if let Some(font)=binding {
                walk.budget.reserve(size(alias.len(),64)?)?;
                merged_fonts.set(alias.clone(),walk.copy(font,0)?);
            }
            walk.budget.reserve(size(name.len(),64)?)?;aliases.insert(name.to_vec(),alias);
        }
        has_fonts|=fonts.is_some();result.font_names.push(aliases);
        let Some(resources)=resources else{continue;};
        for (category,value) in resources.iter() {
            walk.step(0)?;
            if category==b"Font"{continue;}
            require([b"ExtGState".as_slice(),b"ColorSpace",b"Pattern",b"Shading",b"XObject",b"Properties",b"ProcSet"].contains(&category.as_slice()),"unsupported default resource category")?;
            let value=walk.resolve(value)?.1;
            if matches!(value,Object::Null){continue;}
            if category==b"ProcSet" {
                has_procsets=true;
                for entry in value.as_array().map_err(bad)? {
                    walk.step(0)?;let name=walk.resolve(entry)?.1.as_name().map_err(bad)?;
                    walk.budget.reserve(name.len())?;
                    if !known_procsets.contains(name) {
                        walk.budget.reserve(size(name.len(),96)?)?;known_procsets.insert(name.to_vec());
                        procsets.push(walk.copy(entry,0)?);
                    }
                }
                continue;
            }
            let entries=value.as_dict().map_err(bad)?;
            if !result.dictionary.has(category) {
                walk.budget.reserve(size(category.len(),96)?)?;result.dictionary.set(category.clone(),Dictionary::new());
            }
            let merged=result.dictionary.get_mut(category).map_err(bad)?.as_dict_mut().map_err(bad)?;
            for (name,value) in entries.iter() {
                // A shared dictionary can repeat a large key many times without
                // copying it again. Charge before the lookup/comparison too.
                walk.step(0)?;walk.budget.reserve(name.len())?;
                if let Ok(previous)=merged.get(name) {
                    require(walk.equivalent(previous,value,0)?,"conflicting non-font default resources need a typed rename adapter")?;
                } else {
                    walk.budget.reserve(size(name.len(),64)?)?;merged.set(name.clone(),walk.copy(value,0)?);
                }
            }
        }
    }
    if has_fonts{walk.budget.reserve(96)?;result.dictionary.set("Font",merged_fonts);}
    if has_procsets{walk.budget.reserve(96)?;result.dictionary.set("ProcSet",procsets);}
    walk.step(0)?;Ok(result)
}

fn bad(message:impl std::fmt::Display)->EngineError {EngineError::InvalidRequest(format!("PDF form resource staging: {message}"))}
fn require(condition:bool,message:&str)->EngineResult<()> {if condition{Ok(())}else{Err(bad(message))}}
fn size(bytes:usize,overhead:usize)->EngineResult<usize>{bytes.checked_add(overhead).ok_or_else(||bad("metadata size overflow"))}
struct Walk<'a,'b>{doc:&'a Document,budget:&'b mut Budget,check:&'b mut dyn FnMut()->EngineResult<()>}
impl<'a> Walk<'a,'_> {
    fn step(&mut self,depth:usize)->EngineResult<()> {self.budget.step(depth,self.check)}
    fn resolve<'c>(&mut self,mut value:&'c Object)->EngineResult<(Option<ObjectId>,&'c Object)> where 'a:'c {
        let mut id=None;
        for depth in 0..=64 {
            self.step(depth)?;
            match value {Object::Reference(next)=>{id=Some(*next);value=self.doc.objects.get(next).ok_or_else(||bad("missing resource object"))?;},_=>return Ok((id,value))}
        }
        Err(bad("cyclic or excessive resource aliases"))
    }
    fn optional_dict(&mut self,dict:&'a Dictionary,key:&[u8])->EngineResult<Option<&'a Dictionary>> {
        let Some(value)=dict.get(key).ok() else{return Ok(None);};
        match self.resolve(value)?.1 {Object::Null=>Ok(None),value=>Ok(Some(value.as_dict().map_err(bad)?))}
    }
    fn font<'c>(&mut self,fonts:Option<&'c Dictionary>,name:&[u8])->EngineResult<Option<&'c Object>> where 'a:'c {
        let Some(font)=fonts.and_then(|fonts|fonts.get(name).ok()) else{return Ok(None);};
        match self.resolve(font)?.1 {
            Object::Null=>Ok(None),
            Object::Dictionary(_)=>Ok(Some(font)),
            _=>Err(bad("font resource is not a dictionary")),
        }
    }
    fn copy(&mut self,value:&Object,depth:usize)->EngineResult<Object> {
        self.step(depth)?;self.budget.reserve(64)?;
        Ok(match value {
            Object::Name(bytes)=>{self.budget.reserve(bytes.len())?;Object::Name(bytes.clone())},
            Object::String(bytes,format)=>{self.budget.reserve(bytes.len())?;Object::String(bytes.clone(),*format)},
            Object::Array(values)=>{let mut copied=Vec::new();for value in values{copied.push(self.copy(value,depth+1)?);}Object::Array(copied)},
            Object::Dictionary(dict)=>{let mut copied=Dictionary::new();for (key,value) in dict.iter(){self.budget.reserve(size(key.len(),64)?)?;copied.set(key.clone(),self.copy(value,depth+1)?);}Object::Dictionary(copied)},
            Object::Stream(_)=>return Err(bad("direct resource stream requires an indirect identity")),
            Object::Real(value) if !value.is_finite()=>return Err(bad("non-finite resource value")),
            Object::Reference(_)=>{self.resolve(value)?;value.clone()},
            _=>value.clone(),
        })
    }
    fn equivalent(&mut self,a:&Object,b:&Object,depth:usize)->EngineResult<bool> {
        self.step(depth)?;
        // Distinct indirect identities are not coalesced, even if their stream
        // or dictionary payloads happen to look alike.
        if matches!(a,Object::Reference(_)) && matches!(b,Object::Reference(_)) {
            return Ok(self.resolve(a)?.0==self.resolve(b)?.0);
        }
        let a=self.resolve(a)?.1;let b=self.resolve(b)?.1;
        Ok(match (a,b) {
            (Object::Array(a),Object::Array(b))=>{if a.len()!=b.len(){false}else{let mut equal=true;for (a,b) in a.iter().zip(b){if !self.equivalent(a,b,depth+1)?{equal=false;break;}}equal}},
            (Object::Dictionary(a),Object::Dictionary(b))=>{
                if a.len()!=b.len(){false}else{let mut equal=true;for (key,a) in a.iter(){self.budget.reserve(size(key.len(),64)?)?;let Ok(b)=b.get(key) else{equal=false;break;};if !self.equivalent(a,b,depth+1)?{equal=false;break;}}equal}
            },
            (Object::Name(a),Object::Name(b))=>{self.budget.reserve(size(a.len(),b.len())?)?;a==b},
            (Object::String(a,af),Object::String(b,bf))=>{self.budget.reserve(size(a.len(),b.len())?)?;af==bf && a==b},
            (Object::Null,Object::Null)=>true,
            (Object::Boolean(a),Object::Boolean(b))=>a==b,
            (Object::Integer(a),Object::Integer(b))=>a==b,
            (Object::Real(a),Object::Real(b))=>a.is_finite() && b.is_finite() && a==b,
            _=>false,
        })
    }
}
#[cfg(test)]
#[path="composition_form_resources_tests.rs"]
mod tests;
