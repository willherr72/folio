//! Literal replacement intent, stable plans and existing batch previews.
use super::*;
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use crate::{TextReplacementDocumentPlan,TextReplacementSelection};

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct ReplacementSelection{pub page:u32,pub match_index:usize}
#[derive(Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct ReplacementRequest{
    pub version:u32,pub query:String,pub replacement:String,
    #[serde(default)]pub options:crate::PageSearchOptions,
    #[serde(default,skip_serializing_if="Option::is_none",deserialize_with="super::paragraph_preview::non_null")]
    pub selected:Option<Vec<ReplacementSelection>>,
    #[serde(default,skip_serializing_if="Option::is_none",deserialize_with="super::paragraph_preview::non_null")]
    pub expected_plan_id:Option<String>,
}
impl ReplacementRequest{
    pub fn validate(&self,preview:bool)->EngineResult<()>{
        if self.version!=1||self.query.contains('\0'){return Err(invalid("Use a bounded version 1 replacement intent."));}
        crate::engine::document_replacement_plan::validate(&self.query,&self.replacement,None)?;
        match(preview,self.expected_plan_id.as_deref()){
            (false,None)=>(),(true,Some(id))if id.strip_prefix("replacement-plan-v1-").is_some_and(super::text_preview::valid_hash)=>(),
            _=>return Err(invalid("Preview requires the planId returned by planning; planning omits expectedPlanId.")),
        }
        if let Some(selected)=&self.selected{
            if selected.len()>10000{return Err(invalid("Choose at most 10000 original Find matches."));}
            let mut unique=std::collections::BTreeSet::new();
            if selected.iter().any(|s|s.page==0||s.page>10000||s.match_index>=1000||!unique.insert((s.page,s.match_index))){return Err(invalid("Replacement selections must be unique original page/match pairs."));}
        }
        super::inspection::bounded_json(self,512*1024)?;Ok(())
    }
    pub fn read(path:&Path,preview:bool)->EngineResult<Self>{
        let bytes=super::text_preview::read_bounded(path,512*1024)?;
        let request:Self=serde_json::from_slice(&bytes).map_err(|_|invalid("Replacement intent must match its declared schema."))?;
        request.validate(preview)?;Ok(request)
    }
}
pub(crate) fn check_response(value:&Value)->EngineResult<()>{
    super::inspection::bounded_json(value,2*1024*1024-1).map(|_|()).map_err(|_|invalid("Replacement response exceeds its 2 MiB limit; nothing was truncated."))
}
fn native_plan(engine:&PdfEngine,info:&DocumentInfo,request:&ReplacementRequest)->EngineResult<TextReplacementDocumentPlan>{
    let selected=request.selected.as_ref().map(|rows|rows.iter().map(|s|TextReplacementSelection{page_index:s.page as usize-1,match_index:s.match_index}).collect::<Vec<_>>());
    engine.plan_text_replacement_document(&info.id,&request.query,&request.replacement,request.options,selected.as_deref(),Default::default())
}
fn describe(plan:&TextReplacementDocumentPlan)->EngineResult<Value>{
    let mut value=serde_json::to_value(plan).map_err(|_|invalid("Cannot encode replacement plan."))?;
    value.as_object_mut().unwrap().remove("sourceId");
    for page in value["pages"].as_array_mut().unwrap(){page.as_object_mut().unwrap().remove("sourceId");}
    // Complete semantics include native edit identities; process-local IDs do not.
    let bytes=super::inspection::bounded_json(&value,4*1024*1024)?;
    let mut hash=Sha256::new();hash.update(b"folio-replacement-plan-v1\0");hash.update(bytes);
    let id=format!("replacement-plan-v1-{}",hash.finalize().iter().map(|b|format!("{b:02x}")).collect::<String>());
    value["planId"]=json!(id);value["editCount"]=json!(plan.edits.len());value.as_object_mut().unwrap().remove("edits");
    for page in value["pages"].as_array_mut().unwrap(){
        let object=page.as_object_mut().unwrap();object.remove("edits");
        let index=object.remove("pageIndex").and_then(|v|v.as_u64()).ok_or_else(||invalid("Invalid replacement page."))?;
        object.insert("page".into(),json!(index+1));
    }
    check_response(&value)?;Ok(value)
}
pub(super) fn plan(engine:&PdfEngine,info:&DocumentInfo,request:ReplacementRequest)->EngineResult<Value>{
    request.validate(false)?;describe(&native_plan(engine,info,&request)?)
}
fn batch_request(engine:&PdfEngine,info:&DocumentInfo,plan:&TextReplacementDocumentPlan)->EngineResult<super::text_batch_preview::BatchEdit>{
    let hash=Sha256::digest(engine.source_bytes(&info.id)?.as_ref());
    let fingerprint=hash.iter().map(|b|format!("{b:02x}")).collect::<String>();
    if plan.source_id!=info.id||plan.source_fingerprint!=fingerprint{return Err(invalid("Replacement source changed; plan again."));}
    let mut edits=Vec::with_capacity(plan.edits.len());
    for page_index in plan.edits.iter().map(|edit|edit.page_index).collect::<std::collections::BTreeSet<_>>(){
        let roots=engine.list_occurrence_text_runs(&info.id,page_index)?;
        let nested=engine.list_occurrence_form_text_runs(&info.id,page_index)?;
        for edit in plan.edits.iter().filter(|edit|edit.page_index==page_index){
            let page=page_index as u32+1;
            let selection=if edit.object_path.len()==1{
                let mut found=roots.runs.iter().filter(|run|run.object_index==edit.object_path[0]&&run.text==edit.expected_text&&run.supported);
                let run=found.next().ok_or_else(||invalid("Replacement selection is no longer available."))?;
                if found.next().is_some(){return Err(invalid("Replacement selection is ambiguous."));}
                super::inspection::selection(&hash,page,run)?.0
            }else{
                let mut found=nested.runs.iter().filter(|run|run.object_path==edit.object_path&&run.text==edit.expected_text&&run.supported);
                let run=found.next().ok_or_else(||invalid("Nested replacement selection is no longer available."))?;
                if found.next().is_some(){return Err(invalid("Nested replacement selection is ambiguous."));}
                super::nested_inspection::selection(&hash,page,run)?.0
            };
            edits.push(super::text_preview::TextEdit{page,selection_id:selection,replacement:edit.replacement.clone(),font_id:None});
        }
    }
    let result=super::text_batch_preview::BatchEdit{version:1,edits};result.validate()?;Ok(result)
}
pub(super) fn preview(engine:&PdfEngine,info:&DocumentInfo,request:ReplacementRequest,parent:&Path,password:Option<&str>,original:&super::original_snapshot::OriginalSnapshot,fonts:&super::font_inputs::FontInputs)->EngineResult<Value>{
    request.validate(true)?;let plan=native_plan(engine,info,&request)?;let report=describe(&plan)?;
    if report["planId"].as_str()!=request.expected_plan_id.as_deref(){return Err(invalid("Replacement plan is stale; plan the current source and intent again."));}
    if !plan.can_apply||plan.edits.is_empty(){return Err(invalid("Replacement plan has no applicable changes; inspect its blockers or select an eligible subset."));}
    super::text_batch_preview::preview(engine,info,batch_request(engine,info,&plan)?,parent,password,original,fonts)
}

#[cfg(test)]
#[path="replacement_tests.rs"]
mod tests;
