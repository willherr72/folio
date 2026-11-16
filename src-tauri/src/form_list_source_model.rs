//! Optional native-before evidence; no missing evidence authorizes a repair.
use super::*;
use crate::{form_fill::NativeForm, FormField, FormFieldValue};
use lopdf::{ObjectId, Stream};
use sha2::{Digest, Sha256};

struct Captured {
    widget: ObjectId,
    dictionary: Dictionary,
    digest: [u8;32],
    model: ListModel,
    top: usize,
}
pub(crate) struct CapturedListSources {
    owner: usize,
    entries: HashMap<ObjectId,Captured>,
}

fn appearance(source:&Document,widget:ObjectId)->EngineResult<&Stream> {
    let widget=source.get_dictionary(widget).map_err(|_|bad("missing source list widget"))?;
    let ap=resolve(source,widget.get(b"AP").map_err(|_|bad("missing source list appearance"))?)?
        .as_dict().map_err(|_|bad("invalid source list appearances"))?;
    resolve(source,ap.get(b"N").map_err(|_|bad("missing source list normal appearance"))?)?
        .as_stream().map_err(|_|bad("invalid source list normal appearance"))
}
pub(super) fn bounds(source:&Document,stream:&Stream)->EngineResult<[f64;4]> {
    let array=resolve(source,stream.dict.get(b"BBox").map_err(|_|bad("missing source list bounds"))?)?
        .as_array().map_err(|_|bad("invalid source list bounds"))?;
    if array.len()!=4 {return Err(bad("invalid source list bounds"));}
    let mut out=[0.;4];
    for(i,value)in array.iter().enumerate(){
        out[i]=resolve(source,value)?.as_float().map_err(|_|bad("invalid source list bound"))? as f64;
    }
    if out.iter().any(|v|!v.is_finite())||out[0]>=out[2]||out[1]>=out[3]{return Err(bad("empty source list bounds"));}
    Ok(out)
}
fn top_index(source:&Document,widget:ObjectId)->EngineResult<usize> {
    let mut current=source.get_dictionary(widget).map_err(|_|bad("missing source list widget"))?;
    let mut visited=HashSet::new();
    for _ in 0..64 {
        if let Ok(value)=current.get(b"TI") {
            return usize::try_from(resolve(source,value)?.as_i64().map_err(|_|bad("invalid source list top index"))?)
                .map_err(|_|bad("negative source list top index"));
        }
        let Ok(parent)=current.get(b"Parent")else{return Ok(0)};
        if let Object::Reference(id)=parent {if !visited.insert(*id){return Err(bad("cyclic source list parent"));}}
        current=resolve(source,parent)?.as_dict().map_err(|_|bad("invalid source list parent"))?;
    }
    Err(bad("source list parent depth exceeds its limit"))
}
fn field_cost(field:&FormField)->Option<usize> {
    let mut size=std::mem::size_of::<FormField>();
    for value in [&field.name,&field.label,&field.kind] {size=size.checked_add(value.capacity())?;}
    size=size.checked_add(field.reason.as_ref().map_or(0,String::capacity))?;
    size=size.checked_add(field.options.capacity().checked_mul(std::mem::size_of::<String>())?)?;
    for option in &field.options {size=size.checked_add(option.capacity())?;}
    size=size.checked_add(field.selected_indices.capacity().checked_mul(std::mem::size_of::<usize>())?)?;
    size=size.checked_add(match &field.value {
        FormFieldValue::Text{text}=>text.capacity(),
        FormFieldValue::Choice{indices}=>indices.capacity().checked_mul(std::mem::size_of::<usize>())?,
        _=>0,
    })?;
    Some(size)
}
fn same_list(a:&FormField,b:&FormField)->bool {
    a.kind=="list"&&b.kind=="list"&&a.options==b.options&&a.selected_indices==b.selected_indices
        &&a.multi_select==b.multi_select&&a.value==b.value
}
impl CapturedListSources {
    pub(crate) fn capture<'a>(native:&NativeForm<'_>,source:&Document,
        widgets:impl Iterator<Item=(ObjectId,&'a [(usize,usize)])>,enabled:bool)->Self {
        Self::capture_with_limit(native,source,widgets,enabled,64*1024*1024)
    }
    pub(crate) fn capture_with_limit<'a>(native:&NativeForm<'_>,source:&Document,
        widgets:impl Iterator<Item=(ObjectId,&'a [(usize,usize)])>,enabled:bool,limit:usize)->Self {
        let limit=limit.min(64*1024*1024);
        let mut result=Self{owner:source as *const Document as usize,entries:HashMap::new()};
        if !enabled{return result;}
        let mut budget=crate::form_graph::CopyBudget::with_byte_limit(limit);
        // This verdict cache contains no font, row, resource or native authority.
        let mut verdicts=HashMap::<[u8;32],bool>::new();
        for(widget,placements)in widgets {
            let attempt=(||->EngineResult<Option<Captured>>{
                let old=appearance(source,widget)?;
                let remaining=limit.saturating_sub(budget.bytes_used());
                let bytes=old.get_plain_content_with_limit(MAX_BYTES.min(remaining))
                    .map_err(|_|bad("optional list source decoding exhausted its budget"))?;
                budget.reserve_bytes(bytes.len())?;
                let digest:[u8;32]=Sha256::digest(&bytes).into();
                let bbox=bounds(source,old)?;
                // BBox is part of the legacy verdict. The source dictionary must
                // match as well before a verdict can be reused.
                let mut key=Sha256::new();key.update(digest);
                for value in bbox {key.update(value.to_bits().to_le_bytes());}
                let key:[u8;32]=key.finalize().into();
                let supported=if let Some(value)=verdicts.get(&key){*value}else{
                    let supported=AnswerSlot::parse_bounds(&bytes,AnswerKind::List,Some(bbox)).is_ok();
                    budget.reserve_bytes(128)?;verdicts.insert(key,supported);supported
                };
                if supported{return Ok(None);}
                let &(page,index)=placements.first().ok_or_else(||bad("source list has no placement"))?;
                let field=native.field_bounded(page,index,limit.saturating_sub(budget.bytes_used()))?;
                budget.reserve_bytes(field_cost(&field).ok_or_else(||bad("source list model size overflow"))?)?;
                if field.kind!="list"||!matches!(&field.value,FormFieldValue::Choice{indices}if indices==&field.selected_indices){
                    return Err(bad("source widget is not a consistent native list"));
                }
                for &(page,index)in &placements[1..] {
                    let repeated=native.field_bounded(page,index,limit.saturating_sub(budget.bytes_used()))?;
                    budget.reserve_bytes(field_cost(&repeated).ok_or_else(||bad("source list model size overflow"))?)?;
                    if !same_list(&field,&repeated){return Err(bad("source list placements have different native state"));}
                }
                let top=top_index(source,widget)?;
                budget.dictionary(&old.dict)?;
                budget.reserve_bytes(2*std::mem::size_of::<Captured>())?;
                Ok(Some(Captured{widget,dictionary:old.dict.clone(),digest,
                    model:ListModel{options:field.options,selected:field.selected_indices,multiple:field.multi_select},top}))
            })();
            if let Ok(Some(value))=attempt {result.entries.insert(value.widget,value);}
        }
        result
    }
    pub(super) fn get(&self,source:&Document,widget:ObjectId,old:&Stream,bytes:&[u8])->EngineResult<(&ListModel,usize)> {
        if self.owner!=source as *const Document as usize{return Err(bad("source list evidence belongs to another document"));}
        let entry=self.entries.get(&widget).ok_or_else(||bad("source list has no captured native evidence"))?;
        if entry.dictionary!=old.dict||entry.digest!=<[u8;32]>::from(Sha256::digest(bytes)){
            return Err(bad("source list evidence belongs to another appearance"));
        }
        Ok((&entry.model,entry.top))
    }
}
