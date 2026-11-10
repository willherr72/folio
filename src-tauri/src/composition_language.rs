//! Language adaptation for complete private source graphs, before catalog merge.
use crate::{EngineError,EngineResult,composition_pages::SourcePages,composition_structure::StructureInfo};
use lopdf::{dictionary,Dictionary,Document,Object,ObjectId,ResourceLimits,Stream};
use std::{collections::BTreeSet,sync::Arc};
#[path="composition_language_content.rs"] mod content;

pub(crate) fn adapt(
    document:&mut Document,trees:&[SourcePages],structures:&[StructureInfo],pdf2:&[bool],
    cancelled:Arc<dyn Fn()->bool+Send+Sync>,check:&mut impl FnMut()->EngineResult<()>,
)->EngineResult<Vec<(ObjectId,StructureInfo)>> {
    require(!trees.is_empty() && trees.len()==structures.len() && trees.len()==pdf2.len())?;
    let (_,leading)=language(document,trees[0].catalog)?;
    let mut roots=Vec::new();let mut parser=ResourceLimits::default();let mut frame_budget=16*1024*1024usize;let mut steps=0usize;
    let mut checkpoint=||{check()?;steps+=1;require(steps<=1_000_000)};
    for ((tree,structure),pdf2) in trees.iter().zip(structures).zip(pdf2) {
        checkpoint()?;
        if structure.root.is_some(){roots.push((tree.catalog,structure.clone()));}
        let (raw,current)=language(document,tree.catalog)?;
        if current==leading {continue;}
        let annotations=crate::composition_annotation_language::prepare(document,&tree.pages,structure,&raw,*pdf2,&mut checkpoint)?;
        let mut walker=Strings{doc:document,catalog:tree.catalog,pages:tree.pages.iter().copied().collect(),annotations:&annotations.language_owned_annotations,elements:&structure.elements,seen:BTreeSet::new(),check:&mut checkpoint};
        walker.value(&Object::Reference(tree.catalog),Context::Normal,0,None)?;
        let mut plans=Vec::new();
        {
            let mut inspector=content::Inspector::new(document,&mut checkpoint,cancelled.clone(),&mut parser);
            for page in &tree.pages {
                let (envelope,refs)=inspector.page(*page,&raw)?;
                let bytes=envelope.prefix.len().checked_add(envelope.suffix.len()).and_then(|n|n.checked_add(refs.len()*32+128)).ok_or_else(||bad("framing limit"))?;
                frame_budget=frame_budget.checked_sub(bytes).ok_or_else(||bad("framing limit"))?;
                plans.push((*page,envelope.prefix,envelope.suffix,refs));
            }
        }
        let additional=plans.len().checked_mul(2).and_then(|n|u32::try_from(n).ok()).ok_or_else(||bad("object limit"))?;
        require(document.max_id.checked_add(additional).is_some())?;
        for (page,prefix,suffix,refs) in plans {
            checkpoint()?;
            let first=document.add_object(Stream::new(dictionary!{},prefix));
            let last=document.add_object(Stream::new(dictionary!{},suffix.to_vec()));
            let mut contents=Vec::with_capacity(refs.len()+2);contents.push(Object::Reference(first));contents.extend(refs);contents.push(last.into());
            document.get_dictionary_mut(page).map_err(bad)?.set("Contents",contents);
        }
        if let Some(structure)=annotations.structure {roots.push((tree.catalog,structure));}
    }
    checkpoint()?;Ok(roots)
}

pub(crate) fn same_default(document:&Document,first:ObjectId,second:ObjectId)->EngineResult<bool>{
    Ok(language(document,first)?.1==language(document,second)?.1)
}
pub(crate) fn source_default(document:&Document,catalog:ObjectId)->EngineResult<Vec<u8>> {language(document,catalog).map(|(bytes,_)|bytes)}
fn language(document:&Document,catalog:ObjectId)->EngineResult<(Vec<u8>,String)> {
    let Some(value)=optional(document,document.get_dictionary(catalog).map_err(bad)?,b"Lang")? else {return Ok((vec![],String::new()))};
    let bytes=value.as_str().map_err(bad)?;let normalized=normalized_language(bytes)?;Ok((bytes.to_vec(),normalized))
}
pub(crate) fn normalized_language(bytes:&[u8])->EngineResult<String> {
    require(bytes.len()<=1024)?;
    let decoded=if bytes.starts_with(b"\xfe\xff") {
        require(bytes.len()%2==0)?;
        String::from_utf16(&bytes[2..].chunks_exact(2).map(|v|u16::from_be_bytes([v[0],v[1]])).collect::<Vec<_>>()).map_err(bad)?
    } else if bytes.starts_with(b"\xef\xbb\xbf") {std::str::from_utf8(&bytes[3..]).map_err(bad)?.to_owned()}
    else {std::str::from_utf8(bytes).map_err(bad)?.to_owned()};
    require(decoded.is_ascii() && (decoded.is_empty() || decoded.split('-').all(|part|!part.is_empty() && part.len()<=8 && part.bytes().all(|byte|byte.is_ascii_alphanumeric()))))?;
    Ok(decoded.to_ascii_lowercase())
}
fn bad(error:impl std::fmt::Display)->EngineError {EngineError::InvalidRequest(format!("PDF source language: {error}"))}
fn require(ok:bool)->EngineResult<()> {if ok {Ok(())}else{Err(bad("unsupported or inconsistent language context"))}}
fn optional<'a>(document:&'a Document,dictionary:&'a Dictionary,key:&[u8])->EngineResult<Option<&'a Object>> {
    match dictionary.get(key) {
        Ok(value)=>{let (_,value)=document.dereference(value).map_err(bad)?;Ok(if matches!(value,Object::Null){None}else{Some(value)})},
        Err(_)=>Ok(None),
    }
}

#[derive(Clone,Copy,PartialEq,Eq,PartialOrd,Ord)]
enum Context {Normal,Resources,Properties,PropertyList,String,IdTree,IdEntries,Identifiers,CidSystemInfo,
    NameDictionary,DestTree,DestEntries,LegacyDestinations,Destination,Outline,Action,PageActions,AnnotationActions,FileSpec}
struct Strings<'a,'b> {
    doc:&'a Document,annotations:&'b BTreeSet<ObjectId>,elements:&'b BTreeSet<ObjectId>,
    catalog:ObjectId,pages:BTreeSet<ObjectId>,
    seen:BTreeSet<(ObjectId,Context)>,check:&'b mut dyn FnMut()->EngineResult<()>,
}
impl Strings<'_,'_> {
    fn value(&mut self,value:&Object,context:Context,depth:usize,id:Option<ObjectId>)->EngineResult<()> {
        (self.check)()?;require(depth<=64)?;
        if matches!(value,Object::Reference(_)) {
            let (id,value)=self.doc.dereference(value).map_err(bad)?;let id=id.ok_or_else(||bad("indirect identity"))?;
            if self.seen.insert((id,context)){require(self.seen.len()<=100_000)?;self.value(value,context,depth+1,Some(id))?;}
            return Ok(());
        }
        if context==Context::String {return require(matches!(value,Object::String(..)|Object::Null));}
        if matches!(context,Context::Destination|Context::FileSpec) && matches!(value,Object::String(..)) {return Ok(());}
        match value {
            Object::String(..)=>return Err(bad("a retained string needs a typed language-preservation rule")),
            Object::Array(values)=>{
                if matches!(context,Context::IdEntries|Context::DestEntries) {require(values.len()%2==0)?;}
                for (index,value) in values.iter().enumerate() {
                    let child=match context {
                        Context::IdEntries|Context::DestEntries if index%2==0=>Context::String,
                        Context::DestEntries=>Context::Destination,
                        Context::Identifiers=>Context::String,Context::IdTree=>Context::IdTree,
                        Context::DestTree=>Context::DestTree,Context::Action=>Context::Action,
                        _=>Context::Normal};
                    self.value(value,child,depth+1,None)?;
                }
            }
            Object::Dictionary(dictionary)=>self.dictionary(dictionary,context,depth,id)?,
            Object::Stream(stream)=>self.dictionary(&stream.dict,context,depth,id)?,
            _=>{},
        }
        Ok(())
    }
    fn dictionary(&mut self,dictionary:&Dictionary,context:Context,depth:usize,id:Option<ObjectId>)->EngineResult<()> {
        let kind=optional(self.doc,dictionary,b"Type")?.and_then(|value|value.as_name().ok());
        let subtype=optional(self.doc,dictionary,b"Subtype")?.and_then(|value|value.as_name().ok());
        // Glyph-program text needs its own invocation/ownership interpreter.
        require(subtype!=Some(b"Type3".as_slice()))?;
        let annotation=id.is_some_and(|id|self.annotations.contains(&id));
        let element=id.is_some_and(|id|self.elements.contains(&id));
        let catalog=id==Some(self.catalog);
        let page=id.is_some_and(|id|self.pages.contains(&id));
        let action=if context==Context::Action {optional(self.doc,dictionary,b"S")?.and_then(|value|value.as_name().ok())}else{None};
        let uri=optional(self.doc,dictionary,b"S")?.and_then(|value|value.as_name().ok())==Some(b"URI");
        for (key,value) in dictionary.iter() {
            (self.check)()?;
            let key=key.as_slice();
            let child=if (kind==Some(b"Catalog") || element) && key==b"Lang" {Context::String}
            else if annotation && [b"Contents".as_slice(),b"T",b"Subj",b"RC",b"NM",b"M",b"DA",b"DS",b"Lang"].contains(&key) {Context::String}
            else if element && [b"Alt".as_slice(),b"ActualText",b"E",b"T",b"ID"].contains(&key) {Context::String}
            else if context==Context::Action && uri && key==b"URI" {Context::String}
            else if catalog && key==b"Names" {Context::NameDictionary}
            else if catalog && key==b"Dests" {Context::LegacyDestinations}
            else if catalog && key==b"Outlines" {Context::Outline}
            else if context==Context::NameDictionary && key==b"Dests" {Context::DestTree}
            else if context==Context::DestTree && key==b"Kids" {Context::DestTree}
            else if context==Context::DestTree && key==b"Names" {Context::DestEntries}
            else if context==Context::DestTree && key==b"Limits" {Context::Identifiers}
            else if context==Context::LegacyDestinations {Context::Destination}
            else if context==Context::Destination && key==b"D" {Context::Destination}
            else if context==Context::Outline && [b"First".as_slice(),b"Last",b"Next",b"Prev",b"Parent"].contains(&key) {Context::Outline}
            else if (context==Context::Outline || (annotation && subtype==Some(b"Link"))) && key==b"Dest" {Context::Destination}
            else if (context==Context::Outline || (annotation && subtype==Some(b"Link"))) && key==b"A" {Context::Action}
            else if page && key==b"AA" {Context::PageActions}
            else if annotation && key==b"AA" {Context::AnnotationActions}
            else if context==Context::PageActions && [b"O".as_slice(),b"C"].contains(&key) {Context::Action}
            else if context==Context::AnnotationActions && [b"E".as_slice(),b"X",b"D",b"U",b"PO",b"PC",b"PV",b"PI"].contains(&key) {Context::Action}
            else if context==Context::Action && key==b"Next" {Context::Action}
            else if matches!(action,Some(b"GoTo")|Some(b"GoToR")) && key==b"D" {Context::Destination}
            else if action==Some(b"GoToR") && key==b"F" {Context::FileSpec}
            else if context==Context::FileSpec && [b"F".as_slice(),b"UF",b"DOS",b"Mac",b"Unix"].contains(&key) {Context::String}
            else if context==Context::FileSpec && key==b"ID" {Context::Identifiers}
            else if (kind==Some(b"Page") || kind==Some(b"Pages") || subtype==Some(b"Form")) && key==b"Resources" {Context::Resources}
            else if context==Context::Resources && key==b"Properties" {Context::Properties}
            else if context==Context::Properties {Context::PropertyList}
            else if context==Context::PropertyList && [b"Lang".as_slice(),b"ActualText",b"Alt",b"E"].contains(&key) {Context::String}
            else if kind==Some(b"StructTreeRoot") && key==b"IDTree" {Context::IdTree}
            else if context==Context::IdTree && key==b"Kids" {Context::IdTree}
            else if context==Context::IdTree && key==b"Names" {Context::IdEntries}
            else if context==Context::IdTree && key==b"Limits" {Context::Identifiers}
            else if kind==Some(b"Font") && key==b"CIDSystemInfo" {Context::CidSystemInfo}
            else if context==Context::CidSystemInfo && [b"Registry".as_slice(),b"Ordering"].contains(&key) {Context::String}
            else {Context::Normal};
            self.value(value,child,depth+1,None)?;
        }
        Ok(())
    }
}
