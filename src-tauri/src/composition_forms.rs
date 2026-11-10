//! Read-only canonical form ownership for graph-native composition.
#![allow(dead_code)] // Inventory prerequisite; public admission remains guarded.
use crate::{composition_pages::SourcePages, EngineError, EngineResult};
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::collections::{BTreeMap,BTreeSet};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum FieldType { Text, Button, Choice, Signature }
#[derive(Debug,PartialEq,Eq)]
pub(crate) struct Node {
    pub parent:Option<ObjectId>,
    pub children:Vec<ObjectId>,
    pub field:bool,
    pub owner:ObjectId,
    pub widget_page:Option<ObjectId>,
    pub partial_name:Option<String>,
    pub full_name:String,
    pub kind:Option<FieldType>,
}
#[derive(Debug,Default,PartialEq,Eq)]
pub(crate) struct FormInfo {
    pub present:bool,
    pub form_id:Option<ObjectId>,
    pub roots:Vec<ObjectId>,
    pub nodes:BTreeMap<ObjectId,Node>,
    pub calculation_order:Vec<ObjectId>,
    pub need_appearances:bool,
}
pub(crate) use crate::composition_form_budget::Budget;

pub(crate) fn inspect(document:&Document,pages:&SourcePages,budget:&mut Budget,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<FormInfo> {
    let mut walk=Walk{document,budget,check,info:FormInfo::default(),widgets:BTreeMap::new(),seen:BTreeSet::new(),names:BTreeSet::new()};
    walk.step(0)?;
    // SourcePages is supplied by the canonical page-tree inspector. Only page
    // annotation membership is traversed here; AP/resource payloads stay borrowed.
    for page in &pages.pages {
        walk.step(0)?;
        let dictionary=document.get_dictionary(*page).map_err(bad)?;
        if let Some(annots)=walk.optional(dictionary,b"Annots")? {
            for value in annots.as_array().map_err(bad)? {
                walk.step(0)?;
                let annotation=walk.value(value)?.as_dict().map_err(bad)?;
                if !walk.named(annotation,b"Subtype",b"Widget")? {continue;}
                let (id,_)=walk.reference(value)?;
                require(walk.widgets.len()<4000,"too many page widgets")?;
                walk.reserve(128)?;
                require(walk.widgets.insert(id,*page).is_none(),"widget occurs more than once in page annotations")?;
                if let Some(value)=walk.optional(annotation,b"P")? {
                    // optional() resolves values; identity needs the original edge.
                    value.as_dict().map_err(bad)?;
                    require(walk.reference(annotation.get(b"P").map_err(bad)?)?.0==*page,"widget P disagrees with owning page")?;
                }
            }
        }
    }
    let catalog=document.get_dictionary(pages.catalog).map_err(bad)?;
    let Some(form)=walk.optional(catalog,b"AcroForm")? else {
        require(walk.widgets.is_empty(),"page widgets have no AcroForm")?;walk.step(0)?;return Ok(walk.info);
    };
    let form=form.as_dict().map_err(bad)?;
    walk.info.present=true;
    if matches!(catalog.get(b"AcroForm").map_err(bad)?,Object::Reference(_)) {
        walk.info.form_id=Some(walk.reference(catalog.get(b"AcroForm").map_err(bad)?)?.0);
    }
    require(walk.optional(form,b"XFA")?.is_none(),"dynamic XFA form needs a separate adapter")?;
    walk.info.need_appearances=walk.optional(form,b"NeedAppearances")?.map(Object::as_bool).transpose().map_err(bad)?.unwrap_or(false);
    if let Some(fields)=walk.optional(form,b"Fields")? {
        for value in fields.as_array().map_err(bad)? {
            walk.step(0)?;let (id,_)=walk.reference(value)?;
            walk.node(id,None,None,"",None,None,0)?;
            walk.reserve(32)?;walk.info.roots.push(id);
        }
    }
    for widget in walk.widgets.keys() {
        require(walk.info.nodes.get(widget).is_some_and(|node|node.widget_page.is_some()),"page widget missing from field hierarchy")?;
    }
    if let Some(order)=walk.optional(form,b"CO")? {
        for value in order.as_array().map_err(bad)? {
            walk.step(0)?;let (id,_)=walk.reference(value)?;
            let node=walk.info.nodes.get(&id).ok_or_else(||bad("calculation target missing from field hierarchy"))?;
            require(node.field && node.kind.is_some() && node.children.iter().all(|child|!walk.info.nodes[child].field),"calculation target must be a terminal field")?;
            walk.reserve(32)?;walk.info.calculation_order.push(id);
        }
    }
    walk.step(0)?;Ok(walk.info)
}

fn bad(message:impl std::fmt::Display)->EngineError {EngineError::InvalidRequest(format!("PDF form composition: {message}"))}
fn require(condition:bool,message:&str)->EngineResult<()> {if condition{Ok(())}else{Err(bad(message))}}
struct Walk<'a,'b> {
    document:&'a Document,
    budget:&'b mut Budget,
    check:&'b mut dyn FnMut()->EngineResult<()>,
    info:FormInfo,
    widgets:BTreeMap<ObjectId,ObjectId>,
    seen:BTreeSet<ObjectId>,
    names:BTreeSet<String>,
}
impl<'a> Walk<'a,'_> {
    fn step(&mut self,depth:usize)->EngineResult<()> {
        self.budget.step(depth,self.check)
    }
    fn reserve(&mut self,bytes:usize)->EngineResult<()> {
        self.budget.reserve(bytes)
    }
    fn reference(&mut self,value:&Object)->EngineResult<(ObjectId,&'a Object)> {
        let mut id=value.as_reference().map_err(bad)?;let mut seen=BTreeSet::new();
        loop {
            self.step(seen.len())?;require(seen.insert(id),"cyclic indirect alias")?;
            let value=self.document.objects.get(&id).ok_or_else(||bad("missing object"))?;
            if let Object::Reference(next)=value{id=*next;}else{return Ok((id,value));}
        }
    }
    fn value(&mut self,value:&'a Object)->EngineResult<&'a Object> {
        self.step(0)?;
        if matches!(value,Object::Reference(_)){Ok(self.reference(value)?.1)}else{Ok(value)}
    }
    fn optional(&mut self,dictionary:&'a Dictionary,key:&[u8])->EngineResult<Option<&'a Object>> {
        dictionary.get(key).ok().map(|value|self.value(value)).transpose().map(|value|value.filter(|value|!matches!(value,Object::Null)))
    }
    fn named(&mut self,dictionary:&'a Dictionary,key:&[u8],expected:&[u8])->EngineResult<bool> {
        Ok(self.optional(dictionary,key)?.map(Object::as_name).transpose().map_err(bad)?==Some(expected))
    }
    fn node(&mut self,id:ObjectId,parent:Option<ObjectId>,owner:Option<ObjectId>,prefix:&str,inherited:Option<FieldType>,inherited_value:Option<&'a Object>,depth:usize)->EngineResult<()> {
        self.step(depth)?;
        require(self.budget.nodes<4000 && self.seen.insert(id),"cyclic, repeated or excessive field hierarchy")?;
        self.budget.nodes+=1;self.reserve(512)?;
        let dictionary=self.document.get_dictionary(id).map_err(bad)?;
        let actual_parent=if self.optional(dictionary,b"Parent")?.is_some(){Some(self.reference(dictionary.get(b"Parent").map_err(bad)?)?.0)}else{None};
        require(actual_parent==parent,"field Parent disagrees with hierarchy")?;
        let widget=self.named(dictionary,b"Subtype",b"Widget")?;
        let field=!widget || parent.is_none() || dictionary.has(b"T") || dictionary.has(b"FT");
        let partial=self.optional(dictionary,b"T")?;
        let raw_len=partial.map(Object::as_str).transpose().map_err(bad)?.map_or(0,<[u8]>::len);
        // Reserve decoded text, temporary decoding storage, full names and the
        // collision-set copy before any of those allocations. UTF-8 expansion
        // is bounded by three bytes per PDFDoc/UTF16 input byte.
        let charge=raw_len.checked_mul(10).and_then(|n|prefix.len().checked_mul(3).and_then(|p|n.checked_add(p))).and_then(|n|n.checked_add(32)).ok_or_else(||bad("name size overflow"))?;
        self.reserve(charge)?;
        let name=partial.map(|value|crate::form_properties::text(self.document,value,raw_len.saturating_mul(3))).transpose()?;
        let full=name.as_ref().map_or_else(||prefix.to_owned(),|name|crate::form_properties::qualified_name(prefix,name));
        if field && name.is_some(){require(self.names.insert(full.clone()),"duplicate canonical field name")?;}
        let kind=match dictionary.get(b"FT").ok() {
            Some(value)=>match self.value(value)? {
                Object::Null=>None,
                value=>Some(match value.as_name().map_err(bad)? {b"Tx"=>FieldType::Text,b"Btn"=>FieldType::Button,b"Ch"=>FieldType::Choice,b"Sig"=>FieldType::Signature,_=>return Err(bad("unsupported field type"))}),
            },
            None=>inherited,
        };
        // A nonterminal need not declare FT to supply the descendant's V.
        // Borrow effective values through the bounded walk; null (also through
        // aliases) is absent, not a barrier that can hide a signed ancestor.
        let value=self.optional(dictionary,b"V")?.or(inherited_value);
        if kind==Some(FieldType::Signature){require(value.is_none(),"signed form cannot be recomposed")?;}
        let logical=if field{id}else{owner.ok_or_else(||bad("widget has no logical field"))?};
        let widget_page=if widget{Some(*self.widgets.get(&id).ok_or_else(||bad("field widget missing from page annotations"))?)}else{None};
        let mut children=Vec::new();
        if let Some(kids)=self.optional(dictionary,b"Kids")? {
            require(!widget,"widget cannot contain children")?;
            for child in kids.as_array().map_err(bad)? {
                self.step(depth)?;let (child,_)=self.reference(child)?;
                self.node(child,Some(id),Some(logical),&full,kind,value,depth+1)?;
                self.reserve(32)?;children.push(child);
            }
        }
        let has_fields=children.iter().any(|child|self.info.nodes[child].field);
        let has_widgets=children.iter().any(|child|!self.info.nodes[child].field);
        require(!(has_fields && has_widgets),"mixed field and widget children")?;
        if !has_fields {require(kind.is_some(),"terminal field type is missing")?;}
        self.info.nodes.insert(id,Node{parent,children,field,owner:logical,widget_page,partial_name:name,full_name:full,kind});
        Ok(())
    }
}

#[cfg(test)]
#[path="composition_forms_tests.rs"]
mod tests;
