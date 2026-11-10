//! Geometry-only widget editing. Appearance and field graphs remain intact.
use crate::{AnnotationRect,EngineError,EngineResult,FormFieldValue};
use lopdf::{Document,Object};
use serde::Deserialize;

#[derive(Debug,Clone,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct PositionFormFieldRequest {
    pub annotation_index:usize,
    pub expected_bounds:AnnotationRect,
    pub expected_value:FormFieldValue,
    pub bounds:AnnotationRect,
    #[serde(default)]
    pub reflow_text:bool,
}
pub(crate) fn bad(reason:impl std::fmt::Display)->EngineError {EngineError::InvalidRequest(format!("Position form field: {reason}"))}
impl PositionFormFieldRequest {
    pub(crate) fn validate(&self)->EngineResult<()> {
        for r in [&self.expected_bounds,&self.bounds] {
            if ![r.x,r.y,r.width,r.height,r.x+r.width,r.y+r.height].iter().all(|v|v.is_finite()) || r.width<=0. || r.height<=0. {return Err(bad("invalid field geometry"));}
        }
        if self.bounds.x<0. || self.bounds.y<0. || self.bounds.width<4. || self.bounds.height<4. {return Err(bad("place the entire field inside the page, at least 4 pt wide and tall"));}
        Ok(())
    }
}
fn resolve<'a>(d:&'a Document,value:&'a Object)->EngineResult<&'a Object> {d.dereference(value).map(|(_,v)|v).map_err(bad)}
fn saved_stream(doc:&Document,value:&Object)->EngineResult<()> {
    let stream=resolve(doc,value)?.as_stream().map_err(|_|bad("saved normal appearance must be a stream"))?;
    let bbox=resolve(doc,stream.dict.get(b"BBox").map_err(|_|bad("saved appearance has no bounding box"))?)?.as_array().map_err(bad)?;
    if bbox.len()!=4{return Err(bad("invalid saved appearance bounding box"));}
    let mut v=[0.;4];for (i,value) in bbox.iter().enumerate(){v[i]=resolve(doc,value)?.as_float().map_err(bad)?;}
    if v.iter().any(|v|!v.is_finite())||v[0]>=v[2]||v[1]>=v[3]{return Err(bad("invalid saved appearance bounding box"));}
    Ok(())
}
fn checked_document(bytes:&[u8],page_index:usize,index:usize)->EngineResult<(Document,lopdf::ObjectId)> {
    let (doc,_)=crate::acroforms::authoring_document(bytes)?;
    let pages=doc.get_pages();let page=*pages.values().nth(page_index).ok_or_else(||bad("page outside source"))?;
    let annots=resolve(&doc,doc.get_dictionary(page).map_err(bad)?.get(b"Annots").map_err(bad)?)?.as_array().map_err(bad)?;
    let id=annots.get(index).ok_or_else(||bad("field no longer exists"))?.as_reference().map_err(bad)?;
    let widget=doc.get_dictionary(id).map_err(bad)?;
    if resolve(&doc,widget.get(b"Subtype").map_err(bad)?)?.as_name().map_err(bad)?!=b"Widget" {return Err(bad("selected annotation is not a form field"));}
    let flags=widget.get(b"F").ok().map(|v|resolve(&doc,v)?.as_i64().map_err(bad)).transpose()?.unwrap_or(0);
    if flags&(1|2|8|16|32|128|512)!=0 {return Err(bad("hidden, fixed-view or locked fields cannot be positioned"));}
    let appearance=resolve(&doc,widget.get(b"AP").map_err(|_|bad("this field has no saved appearance to preserve"))?)?.as_dict().map_err(bad)?;
    let normal=resolve(&doc,appearance.get(b"N").map_err(|_|bad("this field has no saved normal appearance"))?)?;
    match normal {
        Object::Stream(_)=>saved_stream(&doc,normal)?,
        Object::Dictionary(states) if !states.is_empty()=>{
            let state=resolve(&doc,widget.get(b"AS").map_err(|_|bad("saved appearance state is missing"))?)?.as_name().map_err(bad)?;
            if !states.has(state){return Err(bad("the current field state has no saved appearance"));}
            for (_,value) in states.iter(){saved_stream(&doc,value)?;}
        },
        _=>return Err(bad("unsupported saved field appearance")),
    }
    let mut placements=0;
    for page in pages.values() {
        if let Ok(annots)=doc.get_dictionary(*page).map_err(bad)?.get(b"Annots") {
            for value in resolve(&doc,annots)?.as_array().map_err(bad)? {
                if value.as_reference().ok()==Some(id) {placements+=1;}
            }
        }
    }
    if placements!=1 {return Err(bad("one widget is reused in several locations; its placement cannot yet be isolated"));}
    Ok((doc,id))
}
/// Check the immutable original before a page copier can split aliases or repair APs.
pub(crate) fn validate_source(bytes:&[u8],page_index:usize,index:usize)->EngineResult<()> {
    checked_document(bytes,page_index,index).map(|_|())
}
pub(crate) fn change(bytes:&[u8],page_index:usize,index:usize,rect:&AnnotationRect)->EngineResult<Vec<u8>> {
    let (mut doc,id)=checked_document(bytes,page_index,index)?;
    doc.get_dictionary_mut(id).map_err(bad)?.set("Rect",vec![Object::Real(rect.x),Object::Real(rect.y),Object::Real(rect.x+rect.width),Object::Real(rect.y+rect.height)]);
    crate::pdf_output::save(&mut doc)
}
