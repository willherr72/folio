//! Isolated page-content placement of trusted, page-framed generated forms.
//! Annotation removal and semantic ownership are separate typed transactions.
#![allow(dead_code)]
use super::*;
use lopdf::{dictionary,Dictionary,Document,Object,ObjectId,Stream,content::Operation};
use crate::{composition_form_budget::Budget,composition_form_scope::ProgramKind};
use std::collections::BTreeSet;

pub(super) struct PageForms<'a>{pub page:ObjectId,pub forms:&'a [ObjectId]}
/// Forms must be the exact installed outer appearances produced by the isolated
/// renderer for these page frames. This is not arbitrary source-object editing.
pub(super) fn append(document:Document,pages:&[PageForms<'_>],check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Document>{
    append_controlled(document,pages,Arc::new(||false),check)
}
pub(super) fn append_controlled(document:Document,pages:&[PageForms<'_>],cancelled:Arc<dyn Fn()->bool+Send+Sync>,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Document>{
    append_bounded(document,pages,Budget::default(),cancelled,check)
}
fn bad(message:impl std::fmt::Display)->EngineError{EngineError::InvalidRequest(format!("Structured drawing flatten: {message}"))}
fn require(ok:bool,message:&str)->EngineResult<()>{if ok{Ok(())}else{Err(bad(message))}}
struct Work<'a,'b>{doc:&'a Document,budget:&'b mut Budget,check:&'b mut dyn FnMut()->EngineResult<()>}
impl<'a> Work<'a,'_>{
    fn step(&mut self,depth:usize)->EngineResult<()>{self.budget.step(depth,self.check)}
    fn reserve(&mut self,count:usize,size:usize)->EngineResult<()>{self.budget.reserve(count.checked_mul(size).ok_or_else(||bad("metadata overflow"))?)}
    fn resolve<'v>(&mut self,mut value:&'v Object)->EngineResult<(Option<ObjectId>,&'v Object)> where 'a:'v {
        let mut id=None;for depth in 0..=64{self.step(depth)?;match value{Object::Reference(next)=>{id=Some(*next);value=self.doc.objects.get(next).ok_or_else(||bad("missing object"))?;},_=>return Ok((id,value))}}Err(bad("reference depth"))
    }
    fn optional<'v>(&mut self,dict:&'v Dictionary,key:&[u8])->EngineResult<Option<&'v Object>> where 'a:'v {
        match dict.get(key){Err(_)=>Ok(None),Ok(value)=>{let value=self.resolve(value)?.1;Ok((!matches!(value,Object::Null)).then_some(value))}}
    }
    fn resources(&mut self,mut page:ObjectId)->EngineResult<Option<&'a Dictionary>>{
        for depth in 0..=64{self.step(depth)?;let dict=self.doc.objects.get(&page).ok_or_else(||bad("missing page ancestor"))?.as_dict().map_err(bad)?;
            if let Some(value)=self.optional(dict,b"Resources")?{return Ok(Some(value.as_dict().map_err(bad)?));}
            let Ok(parent)=dict.get(b"Parent") else{return Ok(None)};page=self.resolve(parent)?.0.ok_or_else(||bad("direct parent"))?;
        }Err(bad("resource inheritance depth"))
    }
    fn page_compositing(&mut self,page:ObjectId,resources:Option<&Dictionary>)->EngineResult<()> {
        self.step(0)?;
        let page=self.doc.get_dictionary(page).map_err(bad)?;
        if let Some(value)=self.optional(page,b"Group")? {
            let group=value.as_dict().map_err(bad)?;
            for (key,_) in group.iter(){self.step(0)?;require([b"Type".as_slice(),b"S",b"CS",b"I",b"K"].contains(&key.as_slice()),"page compositing extensions require a preservation adapter; keep drawings editable")?;}
            if let Some(value)=self.optional(group,b"Type")? {require(value.as_name().map_err(bad)?==b"Group","invalid page compositing group type")?;}
            require(self.optional(group,b"S")?.ok_or_else(||bad("missing page compositing group subtype"))?.as_name().map_err(bad)?==b"Transparency","unsupported page compositing group subtype")?;
            if let Some(value)=self.optional(group,b"I")? {value.as_bool().map_err(bad)?;}
            if let Some(value)=self.optional(group,b"K")? {require(!value.as_bool().map_err(bad)?,"knockout page compositing requires a preservation adapter; keep drawings editable")?;}
            // Annotation appearances sit above the completed page. Moving them
            // into a Gray/CMYK/ICC page group can change their colors in readers
            // even when PDFium produces identical pixels. Do not alter the
            // source group or silently convert the newly flattened artwork.
            if let Some(value)=self.optional(group,b"CS")? {require(matches!(value,Object::Name(name) if name==b"DeviceRGB"),"page blending color space requires a preservation adapter; keep drawings editable")?;}
        }
        if let Some(resources)=resources {
            if let Some(value)=self.optional(resources,b"ColorSpace")? {
                let spaces=value.as_dict().map_err(bad)?;
                require(self.optional(spaces,b"DefaultRGB")?.is_none(),"remapped RGB page compositing requires a preservation adapter; keep drawings editable")?;
            }
        }
        Ok(())
    }
    fn contents(&mut self,value:&Object,out:&mut Vec<Object>,depth:usize)->EngineResult<()>{
        self.step(depth)?;let (id,resolved)=self.resolve(value)?;match resolved{
            Object::Null=>{},Object::Array(values)=>for value in values{self.contents(value,out,depth+1)?;},
            Object::Stream(_) if id.is_some()=>{self.reserve(1,64)?;out.push(value.clone());},
            _=>return Err(bad("contents require indirect streams")),
        }Ok(())
    }
    fn copy(&mut self,value:&Object,depth:usize)->EngineResult<Object>{
        self.step(depth)?;self.reserve(1,32)?;Ok(match value{
            Object::Array(values)=>{self.reserve(values.len(),32)?;let mut out=Vec::with_capacity(values.len());for value in values{out.push(self.copy(value,depth+1)?);}Object::Array(out)},
            Object::Dictionary(values)=>{let mut out=Dictionary::new();for (key,value) in values.iter(){self.reserve(key.len()+32,1)?;out.set(key.clone(),self.copy(value,depth+1)?);}Object::Dictionary(out)},
            Object::String(bytes,_)|Object::Name(bytes)=>{self.reserve(bytes.len(),1)?;value.clone()},
            Object::Stream(_)=>return Err(bad("direct resource stream")),_=>value.clone(),
        })
    }
    fn dictionary(&mut self,dict:Option<&Dictionary>)->EngineResult<Dictionary>{
        let mut out=Dictionary::new();if let Some(dict)=dict{for (key,value) in dict.iter(){self.step(0)?;self.reserve(key.len()+32,1)?;out.set(key.clone(),self.copy(value,0)?);}}Ok(out)
    }
}
fn append_bounded(mut doc:Document,pages:&[PageForms<'_>],mut budget:Budget,cancelled:Arc<dyn Fn()->bool+Send+Sync>,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Document>{
    let mut checkpoint=||{if cancelled(){return Err(bad("cancelled"));}check()};checkpoint()?;
    require(pages.len()<=10_000,"page count limit")?;
    if pages.iter().all(|p|p.forms.is_empty()){return Ok(doc);}
    crate::composition_graph::validate_retained(&doc,&mut checkpoint)?;
    let mut work=Work{doc:&doc,budget:&mut budget,check:&mut checkpoint};work.reserve(pages.len(),256)?;
    let catalog=doc.trailer.get(b"Root").and_then(Object::as_reference).map_err(bad)?;
    let all=crate::composition_pages::inspect(&doc,catalog,&mut ||work.step(0))?.pages;
    work.reserve(all.len(),64)?;let all=all.into_iter().collect::<BTreeSet<_>>();let mut selected=BTreeSet::new();
    let mut plans=Vec::new();let mut roots=Vec::new();let mut dictionaries=Vec::new();let mut form_count=0usize;let mut reference_count=0usize;
    for page in pages.iter().filter(|p|!p.forms.is_empty()){
        work.step(0)?;require(all.contains(&page.page)&&selected.insert(page.page),"foreign or repeated page")?;require(page.forms.len()<=10_000,"form count per page")?;
        form_count=form_count.checked_add(page.forms.len()).filter(|n|*n<=100_000).ok_or_else(||bad("form count limit"))?;
        work.reserve(page.forms.len()+1,128)?;roots.push(page.page);
        for form in page.forms{
            work.step(0)?;let stream=doc.objects.get(form).ok_or_else(||bad("missing generated form"))?.as_stream().map_err(bad)?;
            require(stream.dict.get(b"Subtype").and_then(Object::as_name).map_err(bad)?==b"Form","appearance is not a form")?;
            let bounds=work.optional(&stream.dict,b"BBox")?.ok_or_else(||bad("missing appearance bounds"))?.as_array().map_err(bad)?;
            require(bounds.len()==4&&bounds.iter().all(number),"invalid appearance bounds")?;
            require(bounds[2].as_float().map_err(bad)?>bounds[0].as_float().map_err(bad)?&&bounds[3].as_float().map_err(bad)?>bounds[1].as_float().map_err(bad)?,"empty appearance bounds")?;
            if let Some(matrix)=work.optional(&stream.dict,b"Matrix")?{let matrix=matrix.as_array().map_err(bad)?;require(matrix.len()==6&&matrix.iter().zip([1.,0.,0.,1.,0.,0.]).all(|(v,n)|v.as_float().ok()==Some(n)),"outer appearance matrix is not page-framed")?;}
            roots.push(*form);
        }
        let resources=work.resources(page.page)?;
        work.page_compositing(page.page,resources)?;
        let xobjects=match resources{Some(r)=>work.optional(r,b"XObject")?.map(|v|v.as_dict().map_err(bad)).transpose()?,None=>None};
        if let Some(resources)=resources{dictionaries.push(resources);}if let Some(xobjects)=xobjects{dictionaries.push(xobjects);}
        let mut contents=vec![];if let Ok(value)=doc.get_dictionary(page.page).map_err(bad)?.get(b"Contents"){work.contents(value,&mut contents,0)?;}
        reference_count=reference_count.checked_add(contents.len()).ok_or_else(||bad("content count overflow"))?;
        plans.push((page,resources,xobjects,contents));
    }
    // Small authored streams/names/references plus conservative full copies of
    // inherited resource tables are reserved before dictionary duplication.
    let extra=plans.len().checked_mul(3).ok_or_else(||bad("object count overflow"))?;
    let values=plans.len().checked_mul(64).and_then(|n|n.checked_add(form_count*8)).and_then(|n|n.checked_add(reference_count.checked_mul(2)?)).ok_or_else(||bad("value count overflow"))?;
    let bytes=plans.len().checked_add(form_count).and_then(|n|n.checked_mul(256)).ok_or_else(||bad("byte count overflow"))?;
    crate::composition_graph::reserve_content_additions(&doc,&dictionaries,extra,values,bytes,&mut ||work.step(0))?;
    let base=doc.max_id.max(doc.objects.keys().next_back().map_or(0,|id|id.0));base.checked_add(u32::try_from(extra).map_err(bad)?).ok_or_else(||bad("object ID overflow"))?;
    let mut names=crate::composition_form_scope::reserve_names_checked(&doc,&roots,work.budget,cancelled.clone(),&mut ||(work.check)(),inspect_program)?;
    let mut next=0usize;let mut patches=Vec::with_capacity(plans.len());
    for (page,resources,xobjects,contents) in plans{
        work.step(0)?;let mut resources=work.dictionary(resources)?;let mut xobjects=work.dictionary(xobjects)?;let mut paint=Vec::new();
        work.reserve(page.forms.len(),256)?;
        for form in page.forms{
            let name=loop{work.step(0)?;let name=format!("FolioFlat{next}");next=next.checked_add(1).ok_or_else(||bad("resource name overflow"))?;if !names.contains(name.as_bytes()){work.reserve(name.len()+64,1)?;names.insert(name.as_bytes().to_vec());break name;}};
            xobjects.set(name.as_bytes(),*form);paint.extend_from_slice(format!("q /{name} Do Q\n").as_bytes());
        }
        resources.set("XObject",xobjects);patches.push((page.page,resources,contents,paint));
    }
    work.step(0)?;drop(work);doc.max_id=base;
    for (page,resources,old,paint) in patches{
        checkpoint()?;let first=doc.add_object(Stream::new(dictionary!{},b"q\n".to_vec()));
        // A path is not part of q/Q's saved graphics state. End it (including
        // any pending clipping operation) inside the saved source context.
        let last=doc.add_object(Stream::new(dictionary!{},b"\nn\nQ\n".to_vec()));let drawing=doc.add_object(Stream::new(dictionary!{},paint));
        let mut contents=Vec::with_capacity(old.len()+3);contents.push(first.into());contents.extend(old);contents.extend([last.into(),drawing.into()]);
        let dict=doc.get_dictionary_mut(page).map_err(bad)?;dict.set("Contents",contents);dict.set("Resources",resources);
    }
    crate::composition_graph::validate_retained(&doc,&mut checkpoint)?;checkpoint()?;Ok(doc)
}

fn number(value:&Object)->bool{match value{Object::Integer(_)=>true,Object::Real(v)=>v.is_finite(),_=>false}}
fn nums(values:&[Object],count:usize)->EngineResult<()>{require(values.len()==count&&values.iter().all(number),"invalid numeric operands")}
fn inspect_program(operations:&[Operation],kind:ProgramKind,budget:&mut Budget,check:&mut dyn FnMut()->EngineResult<()>)->EngineResult<()>{
    #[derive(PartialEq)]enum Scope{Text,Mark}
    let mut scopes=Vec::new();let(mut text,mut path,mut clip)=(false,false,false);let(mut graphics,mut compatibility)=(0usize,0usize);
    for op in operations{budget.step(0,check)?;let a=&op.operands;match op.operator.as_str(){
        "q"=>{require(a.is_empty()&&!text&&graphics<64,"graphics save scope")?;graphics+=1;},
        "Q"=>{require(a.is_empty()&&!text&&graphics>0,"graphics restore scope")?;graphics-=1;},
        "BT"=>{require(a.is_empty()&&!text&&!path&&scopes.len()<64,"text scope")?;scopes.push(Scope::Text);text=true;},
        "ET"=>{require(a.is_empty()&&text&&scopes.pop()==Some(Scope::Text),"text scope closure")?;text=false;},
        "BMC"|"BDC"|"MP"|"DP"=>{let properties=matches!(op.operator.as_str(),"BDC"|"DP");require(!path&&a.len()==if properties{2}else{1},"marked operands")?;require(matches!(a[0],Object::Name(_)),"marked tag")?;if properties{require(matches!(a[1],Object::Name(_)|Object::Dictionary(_)),"marked properties")?;}if matches!(op.operator.as_str(),"BMC"|"BDC"){require(scopes.len()<64,"marked depth")?;scopes.push(Scope::Mark);}},
        "EMC"=>{require(a.is_empty()&&!path&&scopes.pop()==Some(Scope::Mark),"marked closure")?;},
        "m"|"re"=>{require(!text,"path inside text")?;nums(a,if op.operator=="m"{2}else{4})?;path=true;},
        "l"|"c"|"v"|"y"|"h"=>{require(!text&&path,"path continuation")?;nums(a,match op.operator.as_str(){"l"=>2,"c"=>6,"h"=>0,_=>4})?;},
        "W"|"W*"=>{require(a.is_empty()&&!text&&path&&!clip,"clip scope")?;clip=true;},
        "n"|"S"|"s"|"f"|"F"|"f*"|"B"|"B*"|"b"|"b*"=>{require(a.is_empty()&&!text,"path ending")?;path=false;clip=false;},
        "Do"|"sh"=>{require(!text&&!path&&a.len()==1&&matches!(a[0],Object::Name(_)),"paint resource context")?;},
        "Tj"|"'"=>{require(text&&a.len()==1&&matches!(a[0],Object::String(..)),"text show")?;},
        "\""=>{require(text&&a.len()==3&&number(&a[0])&&number(&a[1])&&matches!(a[2],Object::String(..)),"text show spacing")?;},
        "TJ"=>{require(text&&a.len()==1,"text array")?;for value in a[0].as_array().map_err(bad)?{budget.step(0,check)?;require(number(value)||matches!(value,Object::String(..)),"text array value")?;}},
        "Tf"=>{require(a.len()==2&&matches!(a[0],Object::Name(_))&&number(&a[1]),"font operands")?;},
        "Td"|"TD"|"Tm"|"T*"=>{require(text,"text position outside text")?;nums(a,match op.operator.as_str(){"Tm"=>6,"T*"=>0,_=>2})?;},
        "w"|"J"|"j"|"M"|"i"|"Tc"|"Tw"|"Tz"|"TL"|"Tr"|"Ts"|"G"|"g"=>nums(a,1)?,
        "cm"=>nums(a,6)?,"RG"|"rg"=>nums(a,3)?,"K"|"k"=>nums(a,4)?,
        "d"=>{require(a.len()==2&&number(&a[1]),"dash operands")?;for value in a[0].as_array().map_err(bad)?{budget.step(0,check)?;require(number(value),"dash value")?;}},
        "ri"|"gs"|"CS"|"cs"=>require(a.len()==1&&matches!(a[0],Object::Name(_)),"named operand")?,
        "SC"|"sc"|"SCN"|"scn"=>{for (i,value) in a.iter().enumerate(){budget.step(0,check)?;require(number(value)||(matches!(op.operator.as_str(),"SCN"|"scn")&&i+1==a.len()&&matches!(value,Object::Name(_))),"color operand")?;}},
        "BX"=>{require(a.is_empty()&&compatibility<64,"compatibility depth")?;compatibility+=1;},
        "EX"=>{require(a.is_empty()&&compatibility>0,"compatibility closure")?;compatibility-=1;},
        "d0"|"d1"=>{require(kind==ProgramKind::Type3&&!text,"glyph metrics outside Type3")?;nums(a,if op.operator=="d0"{2}else{6})?;},
        _=>return Err(bad("unsupported source content operator")),
    }}
    require(scopes.is_empty()&&!text&&graphics==0&&compatibility==0,"unclosed source context")?;
    require(kind==ProgramKind::Page||(!path&&!clip),"unfinished path in isolated resource program")?;Ok(())
}

#[cfg(test)]
#[path="structured_overlay_flatten_tests.rs"]
mod tests;
