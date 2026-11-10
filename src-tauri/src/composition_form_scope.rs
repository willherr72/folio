//! Read-only reservations for aliases added to existing form font scopes.
#![allow(dead_code)]
use crate::{composition_form_budget::Budget,EngineError,EngineResult};
use lopdf::{content::{Content,Operation},Dictionary,Document,Object,ObjectId,Stream};
use std::{collections::BTreeSet,sync::Arc};
#[derive(Clone,Copy,PartialEq,Eq,PartialOrd,Ord)]
pub(crate) enum ProgramKind {Page,Isolated,Type3}
pub(crate) type ProgramInspector=fn(&[Operation],ProgramKind,&mut Budget,&mut dyn FnMut()->EngineResult<()>)->EngineResult<()>;
/// Reserve a conservative superset: all reachable dictionary keys/name values,
/// and names in every reachable page, Form, tiling pattern, Type3 CharProc and
/// annotation appearance program. Binary image/font payloads are not programs.
/// This proves name freshness only, not resource validity or font usability.
pub(crate) fn reserve_names(doc:&Document,roots:&[ObjectId],budget:&mut Budget,cancelled:Arc<dyn Fn()->bool+Send+Sync>,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<BTreeSet<Vec<u8>>> {
    reserve(doc,roots,budget,cancelled,check,None)
}
/// Additional read-only program admission, sharing the name scanner's aggregate
/// parser, metadata, traversal and cancellation allowances.
pub(crate) fn reserve_names_checked(doc:&Document,roots:&[ObjectId],budget:&mut Budget,cancelled:Arc<dyn Fn()->bool+Send+Sync>,check:&mut impl FnMut()->EngineResult<()>,inspector:ProgramInspector)->EngineResult<BTreeSet<Vec<u8>>> {
    reserve(doc,roots,budget,cancelled,check,Some(inspector))
}
fn reserve(doc:&Document,roots:&[ObjectId],budget:&mut Budget,cancelled:Arc<dyn Fn()->bool+Send+Sync>,check:&mut impl FnMut()->EngineResult<()>,inspector:Option<ProgramInspector>)->EngineResult<BTreeSet<Vec<u8>>> {
    let mut walk=Walk{doc,budget,cancelled,check,inspector,names:BTreeSet::new(),visited:BTreeSet::new(),programs:BTreeSet::new()};
    for root in roots {walk.graph(&Object::Reference(*root),0)?;}
    walk.step(0)?;Ok(walk.names)
}
fn bad(message:impl std::fmt::Display)->EngineError {EngineError::InvalidRequest(format!("PDF form font scopes: {message}"))}
struct Walk<'a,'b> {
    doc:&'a Document,budget:&'b mut Budget,cancelled:Arc<dyn Fn()->bool+Send+Sync>,check:&'b mut dyn FnMut()->EngineResult<()>,
    inspector:Option<ProgramInspector>,names:BTreeSet<Vec<u8>>,visited:BTreeSet<ObjectId>,programs:BTreeSet<(ObjectId,ProgramKind)>,
}
impl<'a> Walk<'a,'_> {
    fn step(&mut self,depth:usize)->EngineResult<()> {
        if (self.cancelled)(){return Err(bad("cancelled"));}self.budget.step(depth,self.check)
    }
    fn resolve<'v>(&mut self,mut value:&'v Object)->EngineResult<(Option<ObjectId>,&'v Object)> where 'a:'v {
        let mut id=None;
        for depth in 0..=64 {
            self.step(depth)?;
            match value {Object::Reference(next)=>{id=Some(*next);value=self.doc.objects.get(next).ok_or_else(||bad("missing object"))?;},_=>return Ok((id,value))}
        }
        Err(bad("cyclic or excessive aliases"))
    }
    fn name(&mut self,name:&[u8])->EngineResult<()> {
        // Charge lookup bytes even for repeated names: shared dictionaries must
        // not turn a small retained set into unbounded comparison work.
        self.step(0)?;self.budget.reserve(name.len())?;
        if !self.names.contains(name) {
            self.budget.reserve(name.len().checked_add(64).ok_or_else(||bad("name overflow"))?)?;
            self.names.insert(name.to_vec());
        }Ok(())
    }
    fn is_name(&mut self,dict:&Dictionary,key:&[u8],name:&[u8])->EngineResult<bool> {
        let Some(value)=dict.get(key).ok() else{return Ok(false);};
        Ok(matches!(self.resolve(value)?.1,Object::Name(value) if value==name))
    }
    fn graph(&mut self,value:&Object,depth:usize)->EngineResult<()> {
        self.step(depth)?;let (id,value)=self.resolve(value)?;
        if let Some(id)=id {
            if self.visited.contains(&id){return Ok(());}
            self.budget.reserve(64)?;self.visited.insert(id);
        }
        match value {
            Object::Name(name)=>self.name(name)?,
            Object::Array(values)=>for value in values{self.graph(value,depth+1)?;},
            Object::Dictionary(dict)=>self.dictionary(dict,depth+1)?,
            Object::Stream(stream)=>{
                let tiling=match stream.dict.get(b"PatternType").ok(){Some(value)=>matches!(self.resolve(value)?.1,Object::Integer(1)),None=>false};
                if self.is_name(&stream.dict,b"Subtype",b"Form")? || tiling {self.program(id,stream,ProgramKind::Isolated)?;}
                self.dictionary(&stream.dict,depth+1)?;
            }
            _=>{},
        }Ok(())
    }
    fn dictionary(&mut self,dict:&Dictionary,depth:usize)->EngineResult<()> {
        self.step(depth)?;
        if self.is_name(dict,b"Type",b"Page")? {
            if let Ok(value)=dict.get(b"Contents") {
                // A token may span streams; parse their exact concatenation.
                let mut bytes=Vec::new();self.contents(value,&mut bytes,depth+1)?;self.parse(&bytes,ProgramKind::Page)?;
            }
            if let Ok(value)=dict.get(b"Annots") {
                let value=self.resolve(value)?.1;
                if !matches!(value,Object::Null) {
                    for value in value.as_array().map_err(bad)? {
                        self.step(depth+1)?;
                        let dict=self.resolve(value)?.1.as_dict().map_err(bad)?;
                        self.annotation(dict,depth+1)?;
                    }
                }
            }
        }
        let subtype=dict.get(b"Subtype").ok().map(|v|self.resolve(v)).transpose()?.map(|(_,v)|v);
        let known_annotation=matches!(subtype,Some(Object::Name(name)) if [
            b"Text".as_slice(),b"Link",b"FreeText",b"Line",b"Square",b"Circle",b"Polygon",b"PolyLine",
            b"Highlight",b"Underline",b"Squiggly",b"StrikeOut",b"Stamp",b"Caret",b"Ink",b"Popup",
            b"FileAttachment",b"Sound",b"Movie",b"Widget",b"Screen",b"PrinterMark",b"TrapNet",
            b"Watermark",b"3D",b"Redact",b"RichMedia",b"Projection"].contains(&name.as_slice()));
        if known_annotation || self.is_name(dict,b"Type",b"Annot")? {self.annotation(dict,depth+1)?;}
        if self.is_name(dict,b"Subtype",b"Type3")? {
            let value=dict.get(b"CharProcs").map_err(bad)?;
            let procs=self.resolve(value)?.1.as_dict().map_err(bad)?;
            for (_,value) in procs.iter() {
                self.step(depth)?;let (id,value)=self.resolve(value)?;
                self.program(id,value.as_stream().map_err(bad)?,ProgramKind::Type3)?;
            }
        }
        for (key,value) in dict.iter(){self.name(key)?;self.graph(value,depth+1)?;}Ok(())
    }
    fn annotation(&mut self,dict:&Dictionary,depth:usize)->EngineResult<()> {
        self.step(depth)?;
        // Inspect all states, including currently unselected and rollover/down
        // appearances. A stream visited earlier through another edge still
        // needs this typed program proof (separate from graph visitation).
        if let Ok(value)=dict.get(b"AP") {
            let value=self.resolve(value)?.1;
            if !matches!(value,Object::Null) {
                let appearances=value.as_dict().map_err(bad)?;
                for key in [b"N".as_slice(),b"R",b"D"] {if let Ok(value)=appearances.get(key){self.appearance(value,depth+1)?;}}
            }
        }
        Ok(())
    }
    fn appearance(&mut self,value:&Object,depth:usize)->EngineResult<()> {
        self.step(depth)?;let (id,value)=self.resolve(value)?;
        match value {
            Object::Null=>Ok(()),
            Object::Stream(stream)=>self.program(id,stream,ProgramKind::Isolated),
            Object::Dictionary(states)=>{
                for (_,value) in states.iter(){
                    self.step(depth+1)?;let (id,value)=self.resolve(value)?;
                    self.program(id,value.as_stream().map_err(bad)?,ProgramKind::Isolated)?;
                }Ok(())
            }
            _=>Err(bad("unsupported appearance shape")),
        }
    }
    fn decode(&mut self,stream:&Stream,limit:usize)->EngineResult<Vec<u8>> {
        self.step(0)?;
        // Allocation is capped before decoding, even for compressed content.
        let bytes=stream.decompressed_content_strict_with_limit(limit.min(self.budget.parser.input_bytes)).map_err(bad)?;
        self.step(0)?;Ok(bytes)
    }
    fn contents(&mut self,value:&Object,bytes:&mut Vec<u8>,depth:usize)->EngineResult<()> {
        self.step(depth)?;let (id,value)=self.resolve(value)?;
        match value {
            Object::Null=>{},
            Object::Array(values)=>for value in values{self.contents(value,bytes,depth+1)?;},
            Object::Stream(stream) if id.is_some()=>{
                let limit=self.budget.parser.input_bytes.checked_sub(bytes.len()).ok_or_else(||bad("page content limit"))?;
                let part=self.decode(stream,limit)?;
                self.budget.reserve(part.len())?;bytes.extend_from_slice(&part);
            }
            _=>return Err(bad("page contents need indirect streams")),
        }Ok(())
    }
    fn program(&mut self,id:Option<ObjectId>,stream:&Stream,kind:ProgramKind)->EngineResult<()> {
        self.step(0)?;let id=id.ok_or_else(||bad("content stream needs indirect identity"))?;
        // Existing name-only callers retain their one-parse-per-stream cache.
        // Scope admission additionally distinguishes Type3 metric operators.
        let cache_kind=if self.inspector.is_some(){kind}else{ProgramKind::Isolated};
        if self.programs.contains(&(id,cache_kind)){return Ok(());}
        let bytes=self.decode(stream,self.budget.parser.input_bytes)?;self.parse(&bytes,kind)?;
        self.budget.reserve(64)?;self.programs.insert((id,cache_kind));Ok(())
    }
    fn parse(&mut self,bytes:&[u8],kind:ProgramKind)->EngineResult<()> {
        self.step(0)?;
        let (content,remaining)=Content::decode_strict_with_resource_control(bytes,self.budget.parser,self.cancelled.clone()).map_err(bad)?;
        self.budget.parser=remaining;
        if let Some(inspector)=self.inspector{inspector(&content.operations,kind,self.budget,self.check)?;}
        for operation in content.operations {
            self.step(0)?;for value in &operation.operands{self.operand(value,0)?;}
        }Ok(())
    }
    fn operand(&mut self,value:&Object,depth:usize)->EngineResult<()> {
        self.step(depth)?;
        match value {
            Object::Name(name)=>self.name(name)?,
            Object::Array(values)=>for value in values{self.operand(value,depth+1)?;},
            Object::Dictionary(values)=>for (key,value) in values.iter(){self.name(key)?;self.operand(value,depth+1)?;},
            _=>{},
        }Ok(())
    }
}
#[cfg(test)]
#[path="composition_form_scope_tests.rs"]
mod tests;
