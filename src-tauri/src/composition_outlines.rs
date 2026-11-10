//! Original outline graph ownership. Save admission follows navigation binding.
#![allow(dead_code)]
use crate::{EngineError,EngineResult,composition_structure::StructureInfo};
use lopdf::{dictionary,Dictionary,Document,Object,ObjectId};
use std::collections::BTreeSet;
const MAX_ITEMS:usize=4000;
const MAX_DEPTH:usize=32;

#[derive(Debug,Default,Clone,PartialEq,Eq)]
pub(crate) struct OutlineInfo {
    pub root:Option<ObjectId>,
    pub top:Vec<ObjectId>,
    pub items:Vec<ObjectId>,
    pub visible_count:i64,
}
/// A changed portable list has no origin-bound item identities. Rebuilding is
/// allowed only for plain projected outline fields with no outside consumers of
/// the old nodes. Actions and native-only item metadata require an adapter.
pub(crate) fn compatible_replacement(
    document:&Document,sources:&[crate::composition_pages::SourcePages],outlines:&[OutlineInfo],
    check:&mut impl FnMut()->EngineResult<()>,
)->EngineResult<()> {
    require(sources.len()==outlines.len())?;
    let mut work=Work{check,steps:0,bytes:16*1024*1024};
    let mut nodes=BTreeSet::new();let mut catalogs=BTreeSet::new();
    for (source,outline) in sources.iter().zip(outlines) {
        work.step()?;catalogs.insert(source.catalog);
        if let Some(root)=outline.root{nodes.insert(root);}
        for id in &outline.items {
            work.step()?;nodes.insert(*id);require(nodes.len()<=MAX_ITEMS+sources.len())?;
            let d=document.get_dictionary(*id).map_err(bad)?;
            for (key,value) in d.iter(){
                work.step()?;
                require([b"Title".as_slice(),b"Parent",b"Prev",b"Next",b"First",b"Last",b"Count",b"F",b"C",b"Dest"].contains(&key.as_slice()))?;
                if key==b"Dest" {
                    // Named/indirect operands can carry native destination metadata
                    // absent from the portable model; only direct explicit arrays.
                    let values=value.as_array().map_err(bad)?;
                    require(!values.is_empty() && source.pages.contains(&values[0].as_reference().map_err(bad)?))?;
                    require(values[1..].iter().all(|value|matches!(value,Object::Name(_)|Object::Integer(_)|Object::Real(_)|Object::Null)))?;
                }
            }
        }
    }
    fn no_reference(value:&Object,nodes:&BTreeSet<ObjectId>,work:&mut Work<'_>,depth:usize)->EngineResult<()> {
        work.step()?;require(depth<=64)?;
        match value {
            Object::Reference(id)=>require(!nodes.contains(id))?,
            Object::Array(values)=>for value in values{no_reference(value,nodes,work,depth+1)?;},
            Object::Dictionary(d)=>for (_,value) in d.iter(){no_reference(value,nodes,work,depth+1)?;},
            Object::Stream(s)=>for (_,value) in s.dict.iter(){no_reference(value,nodes,work,depth+1)?;},
            _=>{},
        }Ok(())
    }
    for (id,value) in &document.objects {
        work.step()?;
        if let Object::Dictionary(d)=value {
            for (key,value) in d.iter(){
                work.step()?;
                if (nodes.contains(id) && [b"Parent".as_slice(),b"Prev",b"Next",b"First",b"Last"].contains(&key.as_slice()))
                    || (catalogs.contains(id) && key==b"Outlines") {continue;}
                no_reference(value,&nodes,&mut work,0)?;
            }
        } else {no_reference(value,&nodes,&mut work,0)?;}
    }
    work.step()
}
pub(crate) fn inspect(document:&Document,catalog:ObjectId,structure:&StructureInfo,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<OutlineInfo> {
    let mut work=Work{check,steps:0,bytes:16*1024*1024};
    let (_,catalog)=work.at(document,catalog)?;let catalog=catalog.as_dict().map_err(bad)?;
    let Some(value)=work.optional(document,catalog,b"Outlines")? else{return Ok(OutlineInfo::default())};
    let (root,value)=work.resolve(document,value)?;let dictionary=value.as_dict().map_err(bad)?;
    for (key,_) in dictionary.iter(){work.step()?;require([b"Type".as_slice(),b"First",b"Last",b"Count"].contains(&key.as_slice()))?;}
    if let Some(kind)=work.optional(document,dictionary,b"Type")?{require(work.resolve(document,kind)?.1.as_name().map_err(bad)?==b"Outlines")?;}
    let first=work.reference(document,dictionary,b"First")?;let last=work.reference(document,dictionary,b"Last")?;
    require(first.is_some()==last.is_some())?;
    let mut reader=Reader{document,structure,work,seen:BTreeSet::new(),items:Vec::new()};
    let (top,visible_count)=match (first,last){
        (Some(first),Some(last))=>reader.level(root.ok_or_else(invalid)?,first,last,0)?,
        _=>(vec![],0),
    };
    let count=reader.work.integer(document,dictionary,b"Count")?;
    require(if top.is_empty(){count.is_none() || count==Some(0)}else{count==Some(visible_count)})?;
    reader.work.step()?;
    Ok(OutlineInfo{root,top,items:reader.items,visible_count})
}

struct Reader<'a,'b> {
    document:&'a Document,structure:&'a StructureInfo,work:Work<'b>,seen:BTreeSet<ObjectId>,items:Vec<ObjectId>,
}
impl Reader<'_,'_> {
    fn level(&mut self,parent:ObjectId,first:ObjectId,last:ObjectId,depth:usize)->EngineResult<(Vec<ObjectId>,i64)> {
        self.work.step()?;require(depth<MAX_DEPTH)?;
        let mut next=Some(first);let mut previous=None;let mut siblings=Vec::new();let mut visible=0i64;
        while let Some(id)=next {
            self.work.step()?;let (id,value)=self.work.at(self.document,id)?;
            require(self.seen.insert(id) && self.seen.len()<=MAX_ITEMS)?;
            let dictionary=value.as_dict().map_err(bad)?;
            require(self.work.reference(self.document,dictionary,b"Parent")?==Some(parent))?;
            require(self.work.reference(self.document,dictionary,b"Prev")?==previous)?;
            let title=dictionary.get(b"Title").map_err(bad)?;
            require(self.work.resolve(self.document,title)?.1.as_str().map_err(bad)?.len()<=8194)?;
            if let Some(flags)=self.work.integer(self.document,dictionary,b"F")?{require((0..=3).contains(&flags))?;}
            if let Some(color)=self.work.optional(self.document,dictionary,b"C")? {
                let values=self.work.resolve(self.document,color)?.1.as_array().map_err(bad)?;require(values.len()==3)?;
                for value in values {let number=self.work.resolve(self.document,value)?.1.as_float().map_err(bad)?;require(number.is_finite() && (0.0..=1.0).contains(&number))?;}
            }
            if let Some(element)=self.work.reference(self.document,dictionary,b"SE")?{require(self.structure.elements.contains(&element))?;}
            // Destination semantics belong to the source namespace adapter.
            require(!(self.work.optional(self.document,dictionary,b"Dest")?.is_some() && self.work.optional(self.document,dictionary,b"A")?.is_some()))?;
            self.items.push(id);siblings.push(id);
            let first_child=self.work.reference(self.document,dictionary,b"First")?;
            let last_child=self.work.reference(self.document,dictionary,b"Last")?;
            require(first_child.is_some()==last_child.is_some())?;
            let count=self.work.integer(self.document,dictionary,b"Count")?;
            let descendants=match (first_child,last_child){
                (Some(first),Some(last))=>{
                    let (_,visible)=self.level(id,first,last,depth+1)?;
                    let count=count.ok_or_else(invalid)?;require(count!=0 && count.unsigned_abs()==visible as u64)?;
                    if count>0{visible}else{0}
                },
                _=>{require(count.is_none() || count==Some(0))?;0},
            };
            visible=visible.checked_add(1+descendants).ok_or_else(invalid)?;
            previous=Some(id);next=self.work.reference(self.document,dictionary,b"Next")?;
        }
        require(previous==Some(last))?;Ok((siblings,visible))
    }
}

/// Only typed navigation fields may have changed since these infos were read.
/// No original dictionary changes until every update and cancellation check pass.
pub(crate) fn merge(document:&mut Document,sources:&[OutlineInfo],check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Option<ObjectId>> {
    let mut work=Work{check,steps:0,bytes:16*1024*1024};work.step()?;require(sources.len()<=100_000)?;
    let mut roots=BTreeSet::new();let mut items=BTreeSet::new();let mut nonempty=Vec::new();let mut visible=0i64;
    for source in sources {
        work.step()?;
        if let Some(root)=source.root{require(roots.insert(root))?;}
        for id in &source.items {work.step()?;require(items.insert(*id) && items.len()<=MAX_ITEMS)?;document.get_dictionary(*id).map_err(bad)?;}
        if source.top.is_empty(){require(source.items.is_empty() && source.visible_count==0)?;continue;}
        let root=source.root.ok_or_else(invalid)?;
        let dictionary=document.get_dictionary(root).map_err(bad)?;
        require(work.integer(document,dictionary,b"Count")?==Some(source.visible_count) && source.visible_count>0)?;
        require(work.reference(document,dictionary,b"First")?==source.top.first().copied() && work.reference(document,dictionary,b"Last")?==source.top.last().copied())?;
        let membership=source.items.iter().copied().collect::<BTreeSet<_>>();
        for (index,id) in source.top.iter().enumerate(){
            work.step()?;require(membership.contains(id))?;
            let node=document.get_dictionary(*id).map_err(bad)?;
            require(work.reference(document,node,b"Parent")?==Some(root))?;
            require(work.reference(document,node,b"Prev")?==index.checked_sub(1).map(|i|source.top[i]))?;
            require(work.reference(document,node,b"Next")?==source.top.get(index+1).copied())?;
        }
        visible=visible.checked_add(source.visible_count).ok_or_else(invalid)?;nonempty.push(source);
    }
    if nonempty.is_empty(){work.step()?;return Ok(None);}
    if nonempty.len()==1{work.step()?;return Ok(nonempty[0].root);}
    let base=document.max_id.max(document.objects.keys().next_back().map_or(0,|id|id.0));
    let root=(base.checked_add(1).ok_or_else(invalid)?,0);
    let mut changes=Vec::new();
    for (index,source) in nonempty.iter().enumerate(){
        for id in &source.top {work.charge(64)?;changes.push((*id,b"Parent".as_slice(),Object::Reference(root)));}
        if index>0 {work.charge(64)?;changes.push((source.top[0],b"Prev",Object::Reference(*nonempty[index-1].top.last().unwrap())));}
        if index+1<nonempty.len(){work.charge(64)?;changes.push((*source.top.last().unwrap(),b"Next",Object::Reference(nonempty[index+1].top[0])));}
    }
    work.charge(256)?;
    let dictionary=dictionary!{"Type"=>"Outlines","First"=>nonempty[0].top[0],"Last"=>*nonempty.last().unwrap().top.last().unwrap(),"Count"=>visible};
    for (id,_,_) in &changes{work.step()?;require(matches!(document.objects.get(id),Some(Object::Dictionary(_))))?;}
    work.step()?;
    for (id,key,value) in changes {document.objects.get_mut(&id).unwrap().as_dict_mut().unwrap().set(key,value);}
    document.objects.insert(root,dictionary.into());document.max_id=root.0;
    Ok(Some(root))
}

fn invalid()->EngineError {bad("inconsistent or unsupported outline graph")}
fn bad(error:impl std::fmt::Display)->EngineError {EngineError::InvalidRequest(format!("PDF outlines: {error}"))}
fn require(value:bool)->EngineResult<()> {if value{Ok(())}else{Err(invalid())}}
struct Work<'a>{check:&'a mut dyn FnMut()->EngineResult<()>,steps:usize,bytes:usize}
impl Work<'_> {
    fn step(&mut self)->EngineResult<()> {(self.check)()?;self.steps+=1;require(self.steps<=1_000_000)}
    fn charge(&mut self,bytes:usize)->EngineResult<()> {self.step()?;self.bytes=self.bytes.checked_sub(bytes).ok_or_else(invalid)?;Ok(())}
    fn at<'a>(&mut self,document:&'a Document,mut id:ObjectId)->EngineResult<(ObjectId,&'a Object)> {
        let mut seen=BTreeSet::new();
        loop {self.step()?;require(seen.len()<MAX_DEPTH && seen.insert(id))?;let value=document.objects.get(&id).ok_or_else(invalid)?;match value {Object::Reference(next)=>id=*next,_=>return Ok((id,value))}}
    }
    fn resolve<'a>(&mut self,document:&'a Document,value:&'a Object)->EngineResult<(Option<ObjectId>,&'a Object)> {
        self.step()?;if let Object::Reference(id)=value {let (id,value)=self.at(document,*id)?;Ok((Some(id),value))}else{Ok((None,value))}
    }
    fn optional<'a>(&mut self,document:&'a Document,dictionary:&'a Dictionary,key:&[u8])->EngineResult<Option<&'a Object>> {
        self.step()?;match dictionary.get(key){Ok(value)=>Ok((!matches!(self.resolve(document,value)?.1,Object::Null)).then_some(value)),Err(_)=>Ok(None)}
    }
    fn reference(&mut self,document:&Document,dictionary:&Dictionary,key:&[u8])->EngineResult<Option<ObjectId>> {
        self.optional(document,dictionary,key)?.map(|value|{require(matches!(value,Object::Reference(_)))?;self.resolve(document,value)?.0.ok_or_else(invalid)}).transpose()
    }
    fn integer(&mut self,document:&Document,dictionary:&Dictionary,key:&[u8])->EngineResult<Option<i64>> {
        self.optional(document,dictionary,key)?.map(|value|self.resolve(document,value)?.1.as_i64().map_err(bad)).transpose()
    }
}
#[cfg(test)]
#[path="composition_outline_tests.rs"]
mod tests;
