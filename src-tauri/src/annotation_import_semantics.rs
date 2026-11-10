//! Borrowed semantic admission tied to the exact immutable parsed source.
use super::*;
use lopdf::{Dictionary,Document,Object,ObjectId};
use crate::composition_structure::{StructureInfo,Parents};
use std::collections::{BTreeMap,BTreeSet};

pub(super) struct Proof<'d>{document:&'d Document,annotation:&'d Dictionary}
impl Proof<'_>{
    pub(super) fn permits(&self,document:&Document,annotation:&Dictionary,key:&[u8])->bool{
        std::ptr::eq(self.document,document) && std::ptr::eq(self.annotation,annotation) && matches!(key,b"StructParent"|b"Lang")
    }
}
pub(super) struct Inspection<'d>{document:&'d Document,allowed:BTreeSet<(ObjectId,ObjectId)>}
impl<'d> Inspection<'d>{
    pub(super) fn inspect(document:&'d Document)->Option<Self>{Self::bounded(document,16*1024*1024,1_000_000).ok()}
    fn bounded(document:&'d Document,bytes:usize,steps:usize)->EngineResult<Self>{
        let mut work=Work{bytes,steps};work.charge(document.objects.len(),128)?;
        let catalog=work.resolve(document,document.trailer.get(b"Root").map_err(bad)?)?.0.ok_or_else(||bad("catalog identity"))?;
        let pages=crate::composition_pages::inspect(document,catalog,&mut ||work.step(0))?.pages;
        let structure=crate::composition_structure::inspect(document,catalog,&pages,&mut ||work.step(0))?;
        let mut version=pdf_version(document.version.as_bytes()).ok_or_else(||bad("PDF version"))?;
        if let Some(v)=work.optional(document,document.get_dictionary(catalog).map_err(bad)?,b"Version")?{version=version.max(pdf_version(v.as_name().map_err(bad)?).ok_or_else(||bad("catalog version"))?);}
        let mut members=BTreeMap::new();
        for page in pages {
            work.step(0)?;if let Some(annots)=work.optional(document,document.get_dictionary(page).map_err(bad)?,b"Annots")?{
                for value in annots.as_array().map_err(bad)?{work.step(0)?;if let Some(id)=work.resolve(document,value)?.0{work.charge(1,96)?;match members.entry(id){std::collections::btree_map::Entry::Vacant(e)=>{e.insert(Some(page));},std::collections::btree_map::Entry::Occupied(mut e)=>{e.insert(None);}}}}
            }
        }
        let mut parents=BTreeMap::new();let mut allowed=BTreeSet::new();
        for (id,page) in members {
            work.step(0)?;let Some(page)=page else{continue};let dict=document.get_dictionary(id).map_err(bad)?;
            if !sensitive(dict){continue;}
            if dict.has(b"StructParents"){continue;}
            if let Some(p)=work.optional(document,dict,b"P")?{if work.resolve(document,dict.get(b"P").map_err(bad)?)?.0!=Some(page) || !matches!(p,Object::Dictionary(_)){continue;}}
            if let Some(lang)=work.optional(document,dict,b"Lang")?{if version<(2,0) || lang.as_str().ok().is_none_or(|v|crate::composition_language::normalized_language(v).is_err()){continue;}}
            if let Some(key)=work.optional(document,dict,b"StructParent")?{
                let Some(owner)=key.as_i64().ok().and_then(|k|structure.owners.get(&k)) else{continue};
                let Parents::Whole(parent)=owner.parents else{continue};if owner.object!=id{continue;}
                if !parents.contains_key(&parent){work.charge(1,128)?;parents.insert(parent,parent_objects(document,&structure,parent,&mut work)?);}
                if !parents[&parent].as_ref().is_some_and(|objects|objects.get(&id)==Some(&Some(page))){continue;}
            }
            work.charge(1,64)?;allowed.insert((page,id));
        }
        work.step(0)?;Ok(Self{document,allowed})
    }
    pub(super) fn proof(&self,page:ObjectId,id:Option<ObjectId>,annotation:&'d Dictionary)->Option<Proof<'d>>{
        let id=id?;if !self.allowed.contains(&(page,id)) || !std::ptr::eq(self.document.objects.get(&id)?.as_dict().ok()?,annotation){return None;}
        Some(Proof{document:self.document,annotation})
    }
}
pub(super) fn sensitive(annotation:&Dictionary)->bool{[b"StructParent".as_slice(),b"StructParents",b"Lang"].iter().any(|k|annotation.has(k))}
fn pdf_version(v:&[u8])->Option<(u8,u8)>{match v{[b'1',b'.',n @ b'0'..=b'7']=>Some((1,n-b'0')),b"2.0"=>Some((2,0)),_=>None}}
fn parent_objects(doc:&Document,structure:&StructureInfo,parent:ObjectId,work:&mut Work)->EngineResult<Option<BTreeMap<ObjectId,Option<ObjectId>>>>{
    let root=structure.root.ok_or_else(||bad("missing structure root"))?;let mut current=parent;let mut inherited=None;
    for depth in 0..=64 {
        work.step(depth)?;if current!=root && !structure.elements.contains(&current){return Err(bad("foreign structure ancestor"));}
        let dict=doc.get_dictionary(current).map_err(bad)?;
        for key in [b"Alt".as_slice(),b"ActualText",b"E",b"A",b"C"]{if work.optional(doc,dict,key)?.is_some(){return Ok(None);}}
        if current==root{break;}
        if inherited.is_none(){if let Some(value)=dict.get(b"Pg").ok(){if !matches!(work.resolve(doc,value)?.1,Object::Null){inherited=work.resolve(doc,value)?.0;}}}
        current=work.resolve(doc,dict.get(b"P").map_err(bad)?)?.0.ok_or_else(||bad("ancestor identity"))?;
    }
    if current!=root{return Err(bad("ancestor depth"));}
    let dict=doc.get_dictionary(parent).map_err(bad)?;let Some(kids)=work.optional(doc,dict,b"K")? else{return Ok(None)};
    let values=if let Object::Array(values)=kids{values.as_slice()}else{std::slice::from_ref(kids)};
    let mut objects=BTreeMap::new();
    for value in values {
        work.step(0)?;let value=work.resolve(doc,value)?.1;let Ok(dict)=value.as_dict() else{continue};
        if work.optional(doc,dict,b"Type")?.and_then(|v|v.as_name().ok())!=Some(b"OBJR"){continue;}
        let id=work.resolve(doc,dict.get(b"Obj").map_err(bad)?)?.0.ok_or_else(||bad("object owner identity"))?;
        let page=if work.optional(doc,dict,b"Pg")?.is_some(){work.resolve(doc,dict.get(b"Pg").map_err(bad)?)?.0}else{inherited};
        work.charge(1,96)?;match objects.entry(id){std::collections::btree_map::Entry::Vacant(e)=>{e.insert(page);},std::collections::btree_map::Entry::Occupied(mut e)=>{e.insert(None);}}
    }
    Ok(Some(objects))
}
fn bad(error:impl std::fmt::Display)->EngineError{EngineError::InvalidRequest(format!("Annotation import semantics: {error}"))}
struct Work{bytes:usize,steps:usize}
impl Work{
    fn step(&mut self,depth:usize)->EngineResult<()>{if depth>64{return Err(bad("nesting limit"));}self.steps=self.steps.checked_sub(1).ok_or_else(||bad("traversal limit"))?;Ok(())}
    fn charge(&mut self,n:usize,size:usize)->EngineResult<()>{self.bytes=self.bytes.checked_sub(n.checked_mul(size).ok_or_else(||bad("metadata overflow"))?).ok_or_else(||bad("metadata limit"))?;Ok(())}
    fn resolve<'a>(&mut self,doc:&'a Document,mut value:&'a Object)->EngineResult<(Option<ObjectId>,&'a Object)>{let mut id=None;for _ in 0..=64{self.step(0)?;match value{Object::Reference(next)=>{id=Some(*next);value=doc.objects.get(next).ok_or_else(||bad("missing object"))?;},_=>return Ok((id,value))}}Err(bad("reference cycle or depth"))}
    fn optional<'a>(&mut self,doc:&'a Document,dict:&'a Dictionary,key:&[u8])->EngineResult<Option<&'a Object>>{match dict.get(key){Err(_)=>Ok(None),Ok(value)=>{let value=self.resolve(doc,value)?.1;Ok((!matches!(value,Object::Null)).then_some(value))}}}
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;
    #[test]
    fn tagged_overlay_import_semantic_tokens_bind_document_dictionary_and_page(){
        let mut doc=Document::with_version("2.0");let pages=doc.new_object_id();let page=doc.new_object_id();let annotation=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","P"=>page,"Lang"=>Object::string_literal("en")});
        doc.objects.insert(page,dictionary!{"Type"=>"Page","Parent"=>pages,"Annots"=>vec![Object::Reference(annotation)]}.into());doc.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}.into());let cat=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});doc.trailer.set("Root",cat);
        let inspection=Inspection::inspect(&doc).unwrap();let dict=doc.get_dictionary(annotation).unwrap();let proof=inspection.proof(page,Some(annotation),dict).unwrap();
        assert!(proof.permits(&doc,dict,b"Lang"));assert!(!proof.permits(&doc,dict,b"AP"));assert!(!proof.permits(&doc,&dict.clone(),b"Lang"));assert!(!proof.permits(&doc.clone(),dict,b"Lang"));
        assert!(inspection.proof(pages,Some(annotation),dict).is_none());assert!(inspection.proof(page,None,dict).is_none());assert!(inspection.proof(page,Some(annotation),&dict.clone()).is_none());
        for (bytes,steps) in [(0,1_000_000),(256,1_000_000),(16*1024*1024,1)]{assert!(Inspection::bounded(&doc,bytes,steps).is_err());}
        let mut lang=doc.clone();lang.catalog_mut().unwrap().set("Lang",Object::string_literal(vec![b'x';1025]));assert!(crate::composition_language::source_default(&lang,cat).is_err());
    }
}
