//! Sparse dependency copying for independent page occurrences.
#![allow(dead_code)] // Wired into occurrence projection after its ownership planner.
use crate::{EngineError,EngineResult};
use lopdf::{Dictionary,Document,Object,ObjectId,Stream};
use std::collections::{BTreeMap,BTreeSet};
pub(crate) type FieldMap=BTreeMap<Vec<u8>,Option<Object>>;
pub(crate) type Fields=BTreeMap<ObjectId,FieldMap>;

#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
struct Usage{objects:usize,values:usize,bytes:usize}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) struct Budget{usage:Usage,max_objects:usize,max_values:usize,max_bytes:usize,steps:usize,metadata:usize}
impl Budget {
    pub(crate) fn new(doc:&Document,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Self>{
        let mut budget=Self{usage:Usage::default(),max_objects:100_000,max_values:1_000_000,max_bytes:256*1024*1024,steps:0,metadata:16*1024*1024};
        take(&mut budget.usage.objects,doc.objects.len(),budget.max_objects)?;
        take(&mut budget.usage.bytes,doc.version.len(),budget.max_bytes)?;
        let mut work=Work{check,steps:0,metadata:16*1024*1024};
        effective(&doc.trailer,None,&mut |key,value|{
            take(&mut budget.usage.bytes,key.len(),budget.max_bytes)?;
            budget.measure(doc,value,None,&mut work,0,false)
        })?;
        for value in doc.objects.values(){budget.measure(doc,value,None,&mut work,0,false)?;}
        Ok(budget)
    }
    fn measure(&mut self,doc:&Document,value:&Object,fields:Option<&FieldMap>,work:&mut Work<'_>,depth:usize,staged:bool)->EngineResult<()> {
        work.step(depth)?;take(&mut self.usage.values,1,self.max_values)?;
        if staged {work.charge(32)?;}
        match value {
            Object::Reference(id)=>require(id.0!=0 && doc.objects.contains_key(id),"unresolved source reference")?,
            Object::Name(bytes)|Object::String(bytes,_)=>{take(&mut self.usage.bytes,bytes.len(),self.max_bytes)?;if staged{work.charge(bytes.len())?;}},
            Object::Real(value)=>require(value.is_finite(),"nonfinite value")?,
            Object::Array(values)=>for value in values {self.measure(doc,value,None,work,depth+1,staged)?;},
            Object::Dictionary(dictionary)=>self.measure_dictionary(doc,dictionary,fields,work,depth,staged)?,
            Object::Stream(stream)=>{
                require(stream.content.len()<=64*1024*1024,"stored stream exceeds preservation limit")?;
                take(&mut self.usage.bytes,stream.content.len(),self.max_bytes)?;
                self.measure_dictionary(doc,&stream.dict,fields,work,depth,staged)?;
            },
            _=>{},
        }
        if fields.is_some_and(|f|!f.is_empty()) {require(matches!(value,Object::Dictionary(_)|Object::Stream(_)),"field overrides require a dictionary")?;}
        Ok(())
    }
    fn measure_dictionary(&mut self,doc:&Document,dictionary:&Dictionary,fields:Option<&FieldMap>,work:&mut Work<'_>,depth:usize,staged:bool)->EngineResult<()> {
        effective(dictionary,fields,&mut |key,value|{
            take(&mut self.usage.bytes,key.len(),self.max_bytes)?;if staged {work.charge(key.len()+32)?;}
            self.measure(doc,value,None,work,depth+1,staged)
        })
    }
}
fn bad(message:&str)->EngineError{EngineError::InvalidRequest(format!("PDF occurrence copy: {message}"))}
fn require(ok:bool,message:&str)->EngineResult<()>{if ok{Ok(())}else{Err(bad(message))}}
fn take(used:&mut usize,amount:usize,max:usize)->EngineResult<()>{*used=used.checked_add(amount).filter(|n|*n<=max).ok_or_else(||bad("copy preservation budget exceeded"))?;Ok(())}
struct Work<'a>{check:&'a mut dyn FnMut()->EngineResult<()>,steps:usize,metadata:usize}
impl Work<'_>{
    fn step(&mut self,depth:usize)->EngineResult<()>{(self.check)()?;require(depth<=64,"direct nesting limit")?;take(&mut self.steps,1,1_000_000)}
    fn charge(&mut self,bytes:usize)->EngineResult<()>{self.metadata=self.metadata.checked_sub(bytes).ok_or_else(||bad("staged metadata limit"))?;Ok(())}
}
fn effective(dictionary:&Dictionary,fields:Option<&FieldMap>,visit:&mut impl FnMut(&[u8],&Object)->EngineResult<()>)->EngineResult<()> {
    for (key,original) in dictionary.iter(){
        match fields.and_then(|fields|fields.get(key)) {Some(Some(value))=>visit(key,value)?,Some(None)=>{},None=>visit(key,original)?,}
    }
    if let Some(fields)=fields {for (key,value) in fields {if !dictionary.has(key) {if let Some(value)=value {visit(key,value)?;}}}}
    Ok(())
}
fn references(value:&Object,fields:Option<&FieldMap>,output:&mut BTreeSet<ObjectId>,work:&mut Work<'_>,depth:usize)->EngineResult<()> {
    work.step(depth)?;
    match value {
        Object::Reference(id)=>{if output.insert(*id){work.charge(32)?;}},
        Object::Array(values)=>for value in values{references(value,None,output,work,depth+1)?;},
        Object::Dictionary(dict)=>effective(dict,fields,&mut |_,value|references(value,None,output,work,depth+1))?,
        Object::Stream(stream)=>effective(&stream.dict,fields,&mut |_,value|references(value,None,output,work,depth+1))?,
        _=>{},
    }Ok(())
}
/// False seeds are copied but references to them remain original. A Page uses
/// this mode: typed P/Pg and navigation adapters decide which edges target a copy.
/// Callers must list typed document/structure boundaries; this is a dependency
/// copier, not an ownership validator. Overrides must belong to explicit seeds.
/// The document and cumulative budget commit only after every check succeeds.
pub(crate) fn copy(
    doc:&mut Document,seeds:&BTreeMap<ObjectId,bool>,boundaries:&BTreeSet<ObjectId>,
    fields:&Fields,budget:&mut Budget,check:&mut impl FnMut()->EngineResult<()>,
)->EngineResult<BTreeMap<ObjectId,ObjectId>> {
    let mut work=Work{check,steps:budget.steps,metadata:budget.metadata};work.step(0)?;
    require(!seeds.is_empty() && seeds.len()<=100_000,"empty or oversized seed set")?;
    for id in fields.keys(){work.step(0)?;require(seeds.contains_key(id),"an override is not owned by a seed")?;}
    let mut pending=seeds.keys().copied().collect::<Vec<_>>();let mut visited=BTreeSet::new();let mut reverse=BTreeMap::<ObjectId,BTreeSet<ObjectId>>::new();
    while let Some(id)=pending.pop(){
        work.step(0)?;if !visited.insert(id){continue;}work.charge(96)?;
        let value=doc.objects.get(&id).ok_or_else(||bad("missing dependency"))?;
        let mut refs=BTreeSet::new();references(value,fields.get(&id),&mut refs,&mut work,0)?;
        for child in refs {
            work.step(0)?;require(doc.objects.contains_key(&child),"missing dependency")?;
            if boundaries.contains(&child) && !seeds.contains_key(&child){continue;}
            if reverse.entry(child).or_default().insert(id){work.charge(64)?;}
            if !visited.contains(&child){work.charge(16)?;pending.push(child);}
        }
    }
    let mut copied=seeds.keys().copied().collect::<BTreeSet<_>>();
    let mut pending=seeds.iter().filter_map(|(id,remap)|remap.then_some(*id)).collect::<Vec<_>>();
    while let Some(id)=pending.pop(){
        work.step(0)?;
        if let Some(parents)=reverse.get(&id){for parent in parents {
            work.step(0)?;
            if copied.insert(*parent){work.charge(64)?;pending.push(*parent);}
        }}
    }
    // Reserve ALL effective values before cloning the first payload. Resource
    // cycles have already been reduced to a finite ID map, not recursive copies.
    let mut next=*budget;take(&mut next.usage.objects,copied.len(),next.max_objects)?;
    for id in &copied {next.measure(doc,&doc.objects[id],fields.get(id),&mut work,0,true)?;}
    let base=doc.max_id.max(doc.objects.keys().next_back().map_or(0,|id|id.0));
    let count=u32::try_from(copied.len()).map_err(|_|bad("object ID overflow"))?;
    let end=base.checked_add(count).ok_or_else(||bad("object ID overflow"))?;
    let map=copied.iter().enumerate().map(|(index,id)|(*id,(base+index as u32+1,0))).collect::<BTreeMap<_,_>>();
    let remap=map.iter().filter(|(id,_)|seeds.get(id).copied()!=Some(false)).map(|(a,b)|(*a,*b)).collect::<BTreeMap<_,_>>();
    let mut additions=BTreeMap::new();
    for id in &copied {
        let value=copy_value(&doc.objects[id],fields.get(id),&remap,&mut work,0)?;
        additions.insert(map[id],value);
    }
    work.step(0)?;next.steps=work.steps;next.metadata=work.metadata;
    doc.objects.extend(additions);doc.max_id=end;*budget=next;
    Ok(map)
}
fn copy_value(value:&Object,fields:Option<&FieldMap>,map:&BTreeMap<ObjectId,ObjectId>,work:&mut Work<'_>,depth:usize)->EngineResult<Object> {
    work.step(depth)?;
    Ok(match value {
        Object::Reference(id)=>Object::Reference(map.get(id).copied().unwrap_or(*id)),
        Object::Array(values)=>Object::Array(values.iter().map(|value|copy_value(value,None,map,work,depth+1)).collect::<EngineResult<_>>()?),
        Object::Dictionary(dictionary)=>Object::Dictionary(copy_dictionary(dictionary,fields,map,work,depth)?),
        Object::Stream(stream)=>Object::Stream(Stream::new(copy_dictionary(&stream.dict,fields,map,work,depth)?,stream.content.clone()).with_compression(stream.allows_compression)),
        _=>value.clone(),
    })
}
fn copy_dictionary(dictionary:&Dictionary,fields:Option<&FieldMap>,map:&BTreeMap<ObjectId,ObjectId>,work:&mut Work<'_>,depth:usize)->EngineResult<Dictionary> {
    let mut output=Dictionary::new();
    effective(dictionary,fields,&mut |key,value|{output.set(key.to_vec(),copy_value(value,None,map,work,depth+1)?);Ok(())})?;
    Ok(output)
}
#[cfg(test)]
#[path="composition_copy_tests.rs"]
mod tests;
