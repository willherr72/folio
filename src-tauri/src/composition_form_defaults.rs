//! Borrowed default/resource provenance before any form namespace changes.
#![allow(dead_code)] // Private assembly prerequisite; not an admission proof.
use crate::{composition_forms::FormInfo,composition_pages::SourcePages,EngineError,EngineResult};
use lopdf::{content::Content,Dictionary,Document,Object,ObjectId};
use std::{collections::BTreeMap,sync::Arc};

#[derive(Clone,Copy,Debug,PartialEq,Eq,PartialOrd,Ord)]
pub(crate) enum Origin {Field(ObjectId),Form(ObjectId),Page(ObjectId),Appearance(ObjectId)}
#[derive(Clone,Copy,Debug)]
pub(crate) struct ValueAt<'a> {pub origin:Origin,pub value:&'a Object}
#[derive(Debug)]
pub(crate) struct Program<'a> {pub raw:&'a Object,pub bytes:&'a [u8],pub font_names:Vec<Vec<u8>>}
#[derive(Debug,Default)]
pub(crate) struct FontCandidates<'a> {
    pub field:Option<&'a Object>,pub form:Option<&'a Object>,pub page:Option<&'a Object>,pub normal:Option<&'a Object>,
}
#[derive(Debug)]
pub(crate) struct Defaults<'a> {
    pub control_da:Option<Origin>,pub fill_da:Option<Origin>,pub alignment:Option<ValueAt<'a>>,
    pub field_resources:Option<ValueAt<'a>>,pub form_resources:Option<ValueAt<'a>>,
    pub page_resources:Option<ValueAt<'a>>,pub control_page_resources:Option<ValueAt<'a>>,pub normal_resources:Option<ValueAt<'a>>,
    pub fonts:BTreeMap<Vec<u8>,FontCandidates<'a>>,
}
#[derive(Debug,Default)]
pub(crate) struct Inventory<'a> {pub programs:BTreeMap<Origin,Program<'a>>,pub nodes:BTreeMap<ObjectId,Defaults<'a>>}
pub(crate) use crate::composition_form_budget::Budget;

pub(crate) fn inspect<'a>(doc:&'a Document,pages:&SourcePages,forms:&FormInfo,budget:&mut Budget,cancelled:Arc<dyn Fn()->bool+Send+Sync>,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Inventory<'a>> {
    let mut walk=Walk{doc,budget,cancelled,check};
    walk.step(0)?;
    let mut result=Inventory::default();
    if !forms.present {return Ok(result);}
    let catalog=doc.get_dictionary(pages.catalog).map_err(bad)?;
    let form=walk.value(catalog.get(b"AcroForm").map_err(bad)?)?.as_dict().map_err(bad)?;
    let form_origin=Origin::Form(pages.catalog);
    let form_da=at(form,form_origin,b"DA");
    let form_resources=at(form,form_origin,b"DR");
    // Empty forms still carry authoring defaults. Their DA names must join the
    // source map even though no existing field currently inherits the program.
    if forms.roots.is_empty() {if let Some(value)=form_da{walk.program(&mut result,value)?;}}
    for (&id,node) in &forms.nodes {
        walk.step(0)?;walk.reserve(512)?;
        let inherited_da=walk.inherited(forms,id,b"DA")?;
        let control=inherited_da.or(form_da);
        let control_da=control.map(|value|value.origin);
        if let Some(value)=control {walk.program(&mut result,value)?;}
        // PDFium's control metadata and native filling use different empty-DA
        // fallbacks. Preserve both origins, rather than inventing one font.
        let fill=match inherited_da {
            Some(value) if !result.programs[&value.origin].bytes.is_empty()=>Some(value),
            _=>form_da,
        };
        let fill_da=fill.map(|value|value.origin);
        if let Some(value)=fill {walk.program(&mut result,value)?;}
        let alignment=walk.inherited(forms,id,b"Q")?.or(at(form,form_origin,b"Q"));
        if let Some(value)=alignment {
            match walk.value(value.value)? {
                Object::Null|Object::Integer(0..=2)=>{},
                _=>return Err(bad("invalid default alignment")),
            }
        }
        let field_resources=walk.inherited(forms,id,b"DR")?;
        let page_resources=node.widget_page.map(|page|walk.page_resources(page)).transpose()?.flatten();
        // AP rendering has a physical owning page even without widget P.
        // PDFium's control font fallback consults P explicitly. The canonical
        // ownership inventory has already proved any non-null P is that page.
        let control_page_resources=match doc.get_dictionary(id).map_err(bad)?.get(b"P").ok() {
            Some(value) if !matches!(walk.value(value)?,Object::Null)=>page_resources,
            _=>None,
        };
        let normal_resources=if node.widget_page.is_some(){walk.normal_resources(id)?}else{None};
        let mut fonts=BTreeMap::new();
        for origin in [control_da,fill_da].into_iter().flatten() {
            for name in &result.programs[&origin].font_names {
                walk.step(0)?;
                if fonts.contains_key(name) {continue;}
                walk.reserve(name.len().checked_add(160).ok_or_else(||bad("font name size overflow"))?)?;
                fonts.insert(name.clone(),FontCandidates{
                    field:walk.font(field_resources,name)?,form:walk.font(form_resources,name)?,
                    page:walk.font(control_page_resources,name)?,normal:walk.font(normal_resources,name)?,
                });
            }
        }
        result.nodes.insert(id,Defaults{control_da,fill_da,alignment,field_resources,form_resources,page_resources,control_page_resources,normal_resources,fonts});
    }
    walk.step(0)?;Ok(result)
}

fn bad(message:impl std::fmt::Display)->EngineError {EngineError::InvalidRequest(format!("PDF form default provenance: {message}"))}
fn at<'a>(dict:&'a Dictionary,origin:Origin,key:&[u8])->Option<ValueAt<'a>> {dict.get(key).ok().map(|value|ValueAt{origin,value})}
struct Walk<'a,'b> {
    doc:&'a Document,budget:&'b mut Budget,cancelled:Arc<dyn Fn()->bool+Send+Sync>,
    check:&'b mut dyn FnMut()->EngineResult<()>,
}
impl<'a> Walk<'a,'_> {
    fn step(&mut self,depth:usize)->EngineResult<()> {
        if (self.cancelled)(){return Err(bad("cancelled"));}
        self.budget.step(depth,self.check)
    }
    fn reserve(&mut self,bytes:usize)->EngineResult<()> {
        self.budget.reserve(bytes)
    }
    fn resolve(&mut self,mut value:&'a Object)->EngineResult<(Option<ObjectId>,&'a Object)> {
        let mut canonical=None;
        for depth in 0..=64 {
            self.step(depth)?;
            match value {
                Object::Reference(id)=>{canonical=Some(*id);value=self.doc.objects.get(id).ok_or_else(||bad("missing object"))?;},
                _=>return Ok((canonical,value)),
            }
        }
        Err(bad("cyclic or excessive indirect aliases"))
    }
    fn value(&mut self,value:&'a Object)->EngineResult<&'a Object> {Ok(self.resolve(value)?.1)}
    fn inherited(&mut self,forms:&FormInfo,mut id:ObjectId,key:&[u8])->EngineResult<Option<ValueAt<'a>>> {
        for depth in 0..=64 {
            self.step(depth)?;
            let dict=self.doc.get_dictionary(id).map_err(bad)?;
            if let Some(value)=at(dict,Origin::Field(id),key){return Ok(Some(value));}
            let node=forms.nodes.get(&id).ok_or_else(||bad("unbound field parent"))?;
            match node.parent {Some(parent)=>id=parent,None=>return Ok(None)}
        }
        Err(bad("excessive field inheritance"))
    }
    fn page_resources(&mut self,mut id:ObjectId)->EngineResult<Option<ValueAt<'a>>> {
        for depth in 0..=64 {
            self.step(depth)?;
            let dict=self.doc.get_dictionary(id).map_err(bad)?;
            if let Some(value)=at(dict,Origin::Page(id),b"Resources"){return Ok(Some(value));}
            let Some(parent)=dict.get(b"Parent").ok() else{return Ok(None);};
            let (canonical,value)=self.resolve(parent)?;
            if matches!(value,Object::Null){return Ok(None);}
            id=canonical.ok_or_else(||bad("direct page parent"))?;
        }
        Err(bad("excessive page inheritance"))
    }
    fn normal_resources(&mut self,id:ObjectId)->EngineResult<Option<ValueAt<'a>>> {
        let dict=self.doc.get_dictionary(id).map_err(bad)?;
        let Some(ap)=dict.get(b"AP").ok() else{return Ok(None);};
        let ap=self.value(ap)?;
        if matches!(ap,Object::Null){return Ok(None);}
        let Some(normal)=ap.as_dict().map_err(bad)?.get(b"N").ok() else{return Ok(None);};
        let (id,normal)=self.resolve(normal)?;
        match normal {
            Object::Stream(stream)=>Ok(at(&stream.dict,Origin::Appearance(id.ok_or_else(||bad("direct appearance stream"))?),b"Resources")),
            Object::Null|Object::Dictionary(_)=>Ok(None), // State dictionaries have no single normal font scope.
            _=>Err(bad("invalid normal appearance")),
        }
    }
    fn font(&mut self,resources:Option<ValueAt<'a>>,name:&[u8])->EngineResult<Option<&'a Object>> {
        let Some(resources)=resources else{return Ok(None);};
        let resources=self.value(resources.value)?;
        if matches!(resources,Object::Null){return Ok(None);}
        let Some(fonts)=resources.as_dict().map_err(bad)?.get(b"Font").ok() else{return Ok(None);};
        let fonts=self.value(fonts)?;
        if matches!(fonts,Object::Null){return Ok(None);}
        let font=fonts.as_dict().map_err(bad)?.get(name).ok();
        if let Some(value)=font {if matches!(self.value(value)?,Object::Null){return Ok(None);}}
        // Keep the source edge. This inventory is not proof that a font is usable.
        Ok(font)
    }
    fn program(&mut self,inventory:&mut Inventory<'a>,source:ValueAt<'a>)->EngineResult<()> {
        self.step(0)?;
        if inventory.programs.contains_key(&source.origin){return Ok(());}
        let bytes=match self.value(source.value)? {Object::Null=>b"".as_slice(),Object::String(bytes,_)=>bytes.as_slice(),_=>return Err(bad("default appearance is not a string"))};
        if bytes.len()>64*1024{return Err(bad("default appearance exceeds 64 KiB"));}
        self.reserve(128)?;
        let (content,remaining)=Content::decode_strict_with_resource_control(bytes,self.budget.parser,self.cancelled.clone()).map_err(bad)?;
        self.budget.parser=remaining;
        self.step(0)?;
        let mut font_names=Vec::new();
        for operation in content.operations {
            self.step(0)?;
            if operation.operator!="Tf"{continue;}
            let [Object::Name(name),size]=operation.operands.as_slice() else{return Err(bad("invalid Tf operands"));};
            let size=match size {Object::Integer(n)=>*n as f64,Object::Real(n)=>f64::from(*n),_=>return Err(bad("invalid Tf size"))};
            if name.is_empty() || !size.is_finite() || size<0.0{return Err(bad("invalid Tf name or size"));}
            self.reserve(name.len().checked_add(64).ok_or_else(||bad("font name size overflow"))?)?;
            font_names.push(name.clone());
        }
        inventory.programs.insert(source.origin,Program{raw:source.value,bytes,font_names});Ok(())
    }
}
#[cfg(test)]
#[path="composition_form_defaults_tests.rs"]
mod tests;
