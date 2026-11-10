//! Read-only content admission for a changed source language default.
use super::{bad,require,optional};
use crate::{EngineResult,composition_language_scope};
use lopdf::{Document,Object,ObjectId,Dictionary,ResourceLimits,content::Operation};
use std::{collections::{BTreeMap,BTreeSet},sync::Arc};

pub(super) struct Inspector<'a,'b> {
    pub doc:&'a Document,
    pub check:&'b mut dyn FnMut()->EngineResult<()>,
    pub cancelled:Arc<dyn Fn()->bool+Send+Sync>,
    pub remaining:&'b mut ResourceLimits,
    cache:BTreeMap<ObjectId,Arc<Vec<Operation>>>,
    active:BTreeSet<ObjectId>,
}
impl<'a,'b> Inspector<'a,'b> {
    pub fn new(doc:&'a Document,check:&'b mut dyn FnMut()->EngineResult<()>,cancelled:Arc<dyn Fn()->bool+Send+Sync>,remaining:&'b mut ResourceLimits)->Self {
        Self{doc,check,cancelled,remaining,cache:BTreeMap::new(),active:BTreeSet::new()}
    }
    fn step(&mut self,depth:usize)->EngineResult<()> {(self.check)()?;require(depth<=64)}
    pub fn page(&mut self,page:ObjectId,language:&[u8])->EngineResult<(composition_language_scope::Envelope,Vec<Object>)> {
        self.step(0)?;
        let dictionary=self.doc.get_dictionary(page).map_err(bad)?;
        let mut refs=Vec::new();
        if let Ok(value)=dictionary.get(b"Contents"){self.contents(value,&mut refs,0)?;}
        let mut bytes=Vec::new();let limit=self.remaining.input_bytes.min(16*1024*1024);
        for value in &refs {
            self.step(0)?;
            let stream=self.doc.dereference(value).map_err(bad)?.1.as_stream().map_err(bad)?;
            let part=self.stream(stream,limit.checked_sub(bytes.len()).ok_or_else(||bad("content limit"))?)?;
            bytes.extend_from_slice(&part);
        }
        let (envelope,left)=composition_language_scope::inspect(&bytes,language,*self.remaining,self.cancelled.clone())?;
        *self.remaining=left;
        let resources=self.page_resources(page)?;
        self.operations(&envelope.operations,resources,0,false,0)?;
        Ok((envelope,refs))
    }
    fn contents(&mut self,value:&Object,out:&mut Vec<Object>,depth:usize)->EngineResult<()> {
        self.step(depth)?;
        match value {
            Object::Null=>{},
            Object::Array(values)=>for value in values {self.contents(value,out,depth+1)?;},
            Object::Reference(_)=>{
                let (_,resolved)=self.doc.dereference(value).map_err(bad)?;
                if matches!(resolved,Object::Stream(_)) {require(out.len()<100_000)?;out.push(value.clone());}
                else {self.contents(resolved,out,depth+1)?;}
            }
            _=>return Err(bad("Contents needs indirect streams")),
        }
        Ok(())
    }
    fn stream(&mut self,stream:&lopdf::Stream,limit:usize)->EngineResult<Vec<u8>> {
        self.step(0)?;
        // Ordinary reading can recover partial Flate output or retry raw
        // deflate. Such recovery cannot establish the original language scope.
        // Reuse structural rewriting's bounded, complete-stream decoder.
        let bytes=stream.decompressed_content_strict_with_limit(limit).map_err(bad)?;
        self.step(0)?;Ok(bytes)
    }
    fn page_resources(&mut self,mut page:ObjectId)->EngineResult<Option<&'a Dictionary>> {
        let mut seen=BTreeSet::new();
        for depth in 0..=64 {
            self.step(depth)?;require(seen.insert(page))?;
            let dictionary=self.doc.get_dictionary(page).map_err(bad)?;
            if let Some(resources)=optional(self.doc,dictionary,b"Resources")? {return Ok(Some(resources.as_dict().map_err(bad)?));}
            let Some(parent)=optional(self.doc,dictionary,b"Parent")? else {return Ok(None)};
            // Resolve through the original reference to retain a canonical ID.
            parent.as_dict().map_err(bad)?;
            page=self.doc.dereference(dictionary.get(b"Parent").map_err(bad)?).map_err(bad)?.0.ok_or_else(||bad("direct page parent"))?;
        }
        Err(bad("resource inheritance depth"))
    }
    fn operations(&mut self,operations:&[Operation],resources:Option<&'a Dictionary>,initial_mode:i64,form:bool,depth:usize)->EngineResult<()> {
        self.step(depth)?;let mut mode=initial_mode;let mut saved=Vec::new();
        for op in operations {
            self.step(depth)?;
            match op.operator.as_str() {
                "q"=>{require(saved.len()<64)?;saved.push(mode);}
                "Q"=>mode=saved.pop().ok_or_else(||bad("graphics restore"))?,
                "Tr"=>{require(op.operands.len()==1)?;mode=op.operands[0].as_i64().map_err(bad)?;require((0..=7).contains(&mode))?;}
                "Tj"|"TJ"|"'"|"\""=>require(!form || mode!=7)?,
                "Do"=>{
                    require(op.operands.len()==1)?;let name=op.operands[0].as_name().map_err(bad)?;
                    let resources=resources.ok_or_else(||bad("missing XObject resources"))?;
                    let xobjects=optional(self.doc,resources,b"XObject")?.ok_or_else(||bad("missing XObject resources"))?.as_dict().map_err(bad)?;
                    let value=xobjects.get(name).map_err(bad)?;
                    let (id,value)=self.doc.dereference(value).map_err(bad)?;
                    let stream=value.as_stream().map_err(bad)?;
                    match optional(self.doc,&stream.dict,b"Subtype")?.ok_or_else(||bad("missing XObject subtype"))?.as_name().map_err(bad)? {
                        b"Image"=>{},
                        b"Form"=>{
                            let id=id.ok_or_else(||bad("direct Form stream"))?;
                            require(self.active.insert(id))?;
                            let operations=if let Some(cached)=self.cache.get(&id){cached.clone()}else{
                                let bytes=self.stream(stream,self.remaining.input_bytes.min(16*1024*1024))?;
                                let (envelope,left)=composition_language_scope::inspect(&bytes,b"",*self.remaining,self.cancelled.clone())?;
                                *self.remaining=left;
                                let operations=Arc::new(envelope.operations);self.cache.insert(id,operations.clone());operations
                            };
                            let form_resources=match optional(self.doc,&stream.dict,b"Resources")? {Some(value)=>Some(value.as_dict().map_err(bad)?),None=>Some(resources)};
                            self.operations(&operations,form_resources,mode,true,depth+1)?;
                            self.active.remove(&id);
                        }
                        _=>return Err(bad("unsupported XObject language context")),
                    }
                }
                _=>{},
            }
        }
        require(saved.is_empty())
    }
}
