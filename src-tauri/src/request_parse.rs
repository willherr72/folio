//! Request-local parse reuse; caller validation and mutation remain independent.
use lopdf::{Document,LoadOptions};
use std::{cell::{Cell,RefCell},collections::VecDeque,marker::PhantomData,rc::Rc};
use std::{ops::Deref,sync::Arc};

const MIN_INPUT:usize=64*1024;
const MAX_INPUT:usize=8*1024*1024;
const MAX_MODEL:usize=16*1024*1024;
const MAX_VALUES:usize=100_000;

pub(crate) enum ReadDocument {Owned(Document),Shared(Arc<Document>)}
impl Deref for ReadDocument {type Target=Document;fn deref(&self)->&Document {match self {Self::Owned(doc)=>doc,Self::Shared(doc)=>doc}}}
impl ReadDocument {pub(crate) fn into_owned(self)->Document {match self {Self::Owned(doc)=>doc,Self::Shared(doc)=>Arc::try_unwrap(doc).unwrap_or_else(|doc|(*doc).clone())}}}
pub(crate) fn read(bytes:&[u8],policy:Policy)->lopdf::Result<ReadDocument> {read_with(bytes,policy,|bytes,policy|Document::load_mem_with_options(bytes,policy.options()))}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum Policy {Strict64,Compatible64,Compatible16}
impl Policy {
    fn options(self)->LoadOptions {LoadOptions {strict:self==Self::Strict64,max_decompressed_size:Some(if self==Self::Compatible16 {16*1024*1024}else{64*1024*1024}),..Default::default()}}
    fn bit(self)->u8 {match self {Self::Strict64=>1,Self::Compatible64=>2,Self::Compatible16=>4}}
}
struct Entry {bytes:Box<[u8]>,policies:u8,document:Arc<Document>,cost:Cost,pinned:bool}
#[derive(Clone,Copy,Default)]
struct Cost {model:usize,values:usize}
impl Cost {
    fn charge(&mut self,bytes:usize,values:usize)->Option<()> {
        self.model=self.model.checked_add(bytes)?;self.values=self.values.checked_add(values)?;
        (self.model<=MAX_MODEL && self.values<=MAX_VALUES).then_some(())
    }
    fn dictionary(&mut self,value:&lopdf::Dictionary,depth:usize)->Option<()> {
        if depth>64{return None;}
        self.charge(64,0)?;
        for (key,value) in value.iter(){self.charge(key.len().checked_add(64)?,0)?;self.object(value,depth+1)?;}
        Some(())
    }
    fn object(&mut self,value:&lopdf::Object,depth:usize)->Option<()> {
        if depth>64{return None;}
        self.charge(64,1)?;
        match value {
            lopdf::Object::Name(data)|lopdf::Object::String(data,_)=>self.charge(data.len(),0)?,
            lopdf::Object::Array(values)=>for value in values{self.object(value,depth+1)?;},
            lopdf::Object::Dictionary(dict)=>self.dictionary(dict,depth+1)?,
            lopdf::Object::Stream(stream)=>{self.charge(stream.content.len(),0)?;self.dictionary(&stream.dict,depth+1)?;},
            lopdf::Object::Null|lopdf::Object::Boolean(_)|lopdf::Object::Integer(_)|lopdf::Object::Real(_)|lopdf::Object::Reference(_)=>{},
        }
        Some(())
    }
    fn document(document:&Document)->Option<Self> {
        // A new owned Document field requires revisiting both admission and model equality.
        let Document {version:_,binary_mark:_,trailer:_,reference_table:_,objects:_,max_id:_,max_bookmark_id:_,bookmarks:_,bookmark_table:_,xref_start:_,encryption_state:_}=document;
        if document.encryption_state.is_some() || !document.bookmarks.is_empty() || !document.bookmark_table.is_empty(){return None;}
        let mut cost=Self::default();
        let entries=document.reference_table.entries.len();
        if entries>MAX_VALUES || document.objects.len()>MAX_VALUES{return None;}
        // Metadata storage is byte charged; values counts actual Object nodes once.
        cost.charge(entries.checked_mul(64)?,0)?;
        cost.charge(document.version.len().checked_add(document.binary_mark.len())?,0)?;
        cost.charge(0,1)?; // Trailer dictionary is also one PDF value.
        cost.dictionary(&document.trailer,0)?;
        for value in document.objects.values(){cost.object(value,0)?;}
        Some(cost)
    }
}
fn same_dictionary(a:&lopdf::Dictionary,b:&lopdf::Dictionary,depth:usize)->bool {
    depth<=64 && a.len()==b.len() && a.iter().zip(b.iter()).all(|((ak,av),(bk,bv))|ak==bk && same_object(av,bv,depth+1))
}
fn same_object(a:&lopdf::Object,b:&lopdf::Object,depth:usize)->bool {
    use lopdf::Object::*;
    if depth>64{return false;}
    match (a,b) {
        (Null,Null)=>true,(Boolean(a),Boolean(b))=>a==b,(Integer(a),Integer(b))=>a==b,
        (Real(a),Real(b))=>a.to_bits()==b.to_bits(),(Name(a),Name(b))=>a==b,
        (String(a,af),String(b,bf))=>a==b && std::mem::discriminant(af)==std::mem::discriminant(bf),
        (Reference(a),Reference(b))=>a==b,
        (Array(a),Array(b))=>a.len()==b.len() && a.iter().zip(b).all(|(a,b)|same_object(a,b,depth+1)),
        (Dictionary(a),Dictionary(b))=>same_dictionary(a,b,depth+1),
        (Stream(a),Stream(b))=>a.content==b.content && a.allows_compression==b.allows_compression && a.start_position==b.start_position && same_dictionary(&a.dict,&b.dict,depth+1),
        _=>false,
    }
}
fn same_model(a:&Document,b:&Document)->bool {
    use lopdf::xref::XrefEntry;
    a.version==b.version && a.binary_mark==b.binary_mark && a.max_id==b.max_id && a.max_bookmark_id==b.max_bookmark_id
        && a.xref_start==b.xref_start && a.encryption_state.is_none() && b.encryption_state.is_none()
        && a.bookmarks.is_empty() && b.bookmarks.is_empty() && a.bookmark_table.is_empty() && b.bookmark_table.is_empty()
        && a.reference_table.size==b.reference_table.size
        && std::mem::discriminant(&a.reference_table.cross_reference_type)==std::mem::discriminant(&b.reference_table.cross_reference_type)
        && a.reference_table.entries.len()==b.reference_table.entries.len()
        && a.reference_table.entries.iter().zip(&b.reference_table.entries).all(|((ak,av),(bk,bv))|ak==bk && match (av,bv){
            (XrefEntry::Free,XrefEntry::Free)|(XrefEntry::UnusableFree,XrefEntry::UnusableFree)=>true,
            (XrefEntry::Normal{offset:a,generation:ag},XrefEntry::Normal{offset:b,generation:bg})=>a==b && ag==bg,
            (XrefEntry::Compressed{container:a,index:ai},XrefEntry::Compressed{container:b,index:bi})=>a==b && ai==bi,
            _=>false,
        })
        && same_dictionary(&a.trailer,&b.trailer,0)
        && a.objects.len()==b.objects.len() && a.objects.iter().zip(&b.objects).all(|((ak,av),(bk,bv))|ak==bk && same_object(av,bv,0))
}
struct Cache {generation:u64,scopes:usize,entries:VecDeque<Entry>,raw:usize,cost:Cost}
std::thread_local! {
    static CACHE:RefCell<Option<Cache>>=const {RefCell::new(None)};
    static GENERATION:Cell<u64>=const {Cell::new(1)};
}
pub(crate) struct Scope {generation:u64,_local:PhantomData<Rc<()>>}
impl Scope {
    pub(crate) fn enter()->Self {
        let generation=CACHE.with(|cell| {
            let mut slot=cell.borrow_mut();
            if let Some(cache)=slot.as_mut(){cache.scopes+=1;return cache.generation;}
            let generation=GENERATION.with(|counter|{let value=counter.get();counter.set(value.wrapping_add(1).max(1));value});
            *slot=Some(Cache {generation,scopes:1,entries:VecDeque::new(),raw:0,cost:Cost::default()});generation
        });
        Self {generation,_local:PhantomData}
    }
}
impl Drop for Scope {
    fn drop(&mut self){CACHE.with(|cell| {
        let mut slot=cell.borrow_mut();
        if let Some(cache)=slot.as_mut().filter(|cache|cache.generation==self.generation){
            cache.scopes-=1;if cache.scopes==0{*slot=None;}
        }
    });}
}
pub(crate) fn load(bytes:&[u8],policy:Policy)->lopdf::Result<Document> {
    read(bytes,policy).map(ReadDocument::into_owned)
}
fn load_with(bytes:&[u8],policy:Policy,parse:impl FnOnce(&[u8],Policy)->lopdf::Result<Document>)->lopdf::Result<Document> {
    read_with(bytes,policy,parse).map(ReadDocument::into_owned)
}
fn read_with(bytes:&[u8],policy:Policy,parse:impl FnOnce(&[u8],Policy)->lopdf::Result<Document>)->lopdf::Result<ReadDocument> {
    if !(MIN_INPUT..=MAX_INPUT).contains(&bytes.len()){return parse(bytes,policy).map(ReadDocument::Owned);}
    let (generation,hit)=CACHE.with(|cell|{
        let mut slot=cell.borrow_mut();let Some(cache)=slot.as_mut() else {return (None,None);};
        if cache.entries.iter().any(|entry|entry.pinned && (entry.cost.model>MAX_MODEL/2 || entry.cost.values>MAX_VALUES/2) && entry.bytes.as_ref()!=bytes){return (None,None);}
        let hit=cache.entries.iter().position(|entry|entry.policies&policy.bit()!=0 && entry.bytes.as_ref()==bytes).map(|position| {
            let entry=cache.entries.remove(position).expect("present cache entry");
            let document=Arc::clone(&entry.document);cache.entries.push_back(entry);document
        });
        (Some(cache.generation),hit)
    });
    if let Some(document)=hit{return Ok(ReadDocument::Shared(document));}
    let mut document=Some(parse(bytes,policy)?);
    if let Some(generation)=generation {
        let pooled=CACHE.with(|cell| {
            let mut slot=cell.borrow_mut();let Some(cache)=slot.as_mut().filter(|cache|cache.generation==generation) else {return None;};
            // This policy has just parsed successfully. Pool only exactly matching
            // models, including ordering/float bits/stream and xref metadata.
            if let Some(position)=cache.entries.iter().position(|entry|entry.bytes.as_ref()==bytes && same_model(&entry.document,document.as_ref().unwrap())) {
                let mut entry=cache.entries.remove(position).expect("present cache entry");
                entry.policies|=policy.bit();let result=Arc::clone(&entry.document);cache.entries.push_back(entry);return Some(result);
            }
            None
        });
        if let Some(pooled)=pooled{return Ok(ReadDocument::Shared(pooled));}
        if let Some(cost)=Cost::document(document.as_ref().unwrap()) {
          let retained=CACHE.with(|cell| {
            let mut slot=cell.borrow_mut();let Some(cache)=slot.as_mut().filter(|cache|cache.generation==generation) else {return None;};
            while cache.entries.len()>=2 || cache.raw+bytes.len()>MAX_INPUT || cache.cost.model+cost.model>MAX_MODEL || cache.cost.values+cost.values>MAX_VALUES {
                let Some(position)=cache.entries.iter().position(|entry|!entry.pinned) else {return None;};
                let old=cache.entries.remove(position).expect("present unpinned entry");
                cache.raw-=old.bytes.len();cache.cost.model-=old.cost.model;cache.cost.values-=old.cost.values;
            }
            // Read-only callers share the bounded immutable model. Writable
            // callers explicitly obtain an independent owned Document.
            let result=Arc::new(document.take().unwrap());
            let entry=Entry {bytes:bytes.into(),policies:policy.bit(),document:Arc::clone(&result),cost,pinned:cache.entries.is_empty()};
            cache.raw+=bytes.len();cache.cost.model+=cost.model;cache.cost.values+=cost.values;cache.entries.push_back(entry);
            Some(result)
          });
          if let Some(retained)=retained{return Ok(ReadDocument::Shared(retained));}
        }
    }
    Ok(ReadDocument::Owned(document.unwrap()))
}
#[cfg(test)]
fn stats()->(usize,usize,usize,usize){CACHE.with(|cell|cell.borrow().as_ref().map_or((0,0,0,0),|cache|(cache.entries.len(),cache.raw,cache.cost.model,cache.cost.values)))}
#[cfg(test)]
#[path="request_parse_tests.rs"]
mod tests;
