//! Validated whole-source page trees. No projection or occurrence cloning.
#![allow(dead_code)]
use crate::{EngineError, EngineResult};
use lopdf::{Document, Object, ObjectId};
use std::collections::BTreeSet;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SourcePages {
    pub catalog: ObjectId,
    pub root: ObjectId,
    pub pages: Vec<ObjectId>,
}

pub(crate) fn inspect(
    document: &Document,
    catalog: ObjectId,
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<SourcePages> {
    let mut walk=Walk {document,check,steps:0,seen:BTreeSet::new(),pages:vec![]};
    let (catalog,value)=walk.reference(&Object::Reference(catalog))?;
    let dictionary=value.as_dict().map_err(bad)?;
    require(walk.value(dictionary.get(b"Type").map_err(bad)?)?.as_name().map_err(bad)? == b"Catalog")?;
    let (root,_)=walk.reference(dictionary.get(b"Pages").map_err(bad)?)?;
    walk.node(root,None,0)?;
    walk.step(0)?;
    Ok(SourcePages {catalog,root,pages:walk.pages})
}

fn bad(message:impl std::fmt::Display)->EngineError {
    EngineError::InvalidRequest(format!("PDF source page tree: {message}"))
}
fn require(condition:bool)->EngineResult<()> {
    if condition {Ok(())} else {Err(bad("inconsistent or unsupported page tree"))}
}
struct Walk<'a,'b> {
    document:&'a Document,
    check:&'b mut dyn FnMut()->EngineResult<()>,
    steps:usize,
    seen:BTreeSet<ObjectId>,
    pages:Vec<ObjectId>,
}
impl<'a> Walk<'a,'_> {
    fn step(&mut self,depth:usize)->EngineResult<()> {
        (self.check)()?;
        self.steps+=1;
        require(depth<=64 && self.steps<=1_000_000)
    }
    fn reference(&mut self,value:&Object)->EngineResult<(ObjectId,&'a Object)> {
        let mut id=value.as_reference().map_err(bad)?;
        let mut aliases=BTreeSet::new();
        loop {
            self.step(aliases.len())?;
            require(aliases.insert(id))?;
            let value=self.document.objects.get(&id).ok_or_else(||bad("missing object"))?;
            match value {
                Object::Reference(next)=>id=*next,
                _=>return Ok((id,value)),
            }
        }
    }
    fn value<'c>(&mut self,value:&'c Object)->EngineResult<&'c Object> where 'a:'c {
        self.step(0)?;
        if matches!(value,Object::Reference(_)){Ok(self.reference(value)?.1)} else {Ok(value)}
    }
    fn node(&mut self,id:ObjectId,parent:Option<ObjectId>,depth:usize)->EngineResult<usize> {
        self.step(depth)?;
        require(self.seen.len()<100_000 && self.seen.insert(id))?;
        let value=self.document.objects.get(&id).ok_or_else(||bad("missing page node"))?;
        let dictionary=value.as_dict().map_err(bad)?;
        let parent_value=dictionary.get(b"Parent").ok();
        if let Some(parent)=parent {
            require(self.reference(parent_value.ok_or_else(||bad("missing Parent"))?)?.0==parent)?;
        } else if let Some(value)=parent_value {
            require(matches!(self.value(value)?,Object::Null))?;
        }
        match self.value(dictionary.get(b"Type").map_err(bad)?)?.as_name().map_err(bad)? {
            b"Page"=>{
                require(parent.is_some() && !dictionary.has(b"Kids") && !dictionary.has(b"Count"))?;
                self.pages.push(id);
                Ok(1)
            }
            b"Pages"=>{
                let count=self.value(dictionary.get(b"Count").map_err(bad)?)?.as_i64().map_err(bad)?;
                require((0..=100_000).contains(&count))?;
                let kids=self.value(dictionary.get(b"Kids").map_err(bad)?)?.as_array().map_err(bad)?;
                require(kids.len()<=100_000)?;
                let mut total=0;
                for kid in kids {
                    let (kid,_)=self.reference(kid)?;
                    total+=self.node(kid,Some(id),depth+1)?;
                }
                require(total==count as usize)?;
                Ok(total)
            }
            _=>Err(bad("invalid page node type")),
        }
    }
}

#[cfg(test)]
#[path = "composition_pages_tests.rs"]
mod tests;
