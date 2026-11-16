//! Complete document planning over one worker-owned source snapshot.
use super::*;
use super::replacement_plan::TextReplacementPagePlan;
use serde::{Deserialize,Serialize};
use std::sync::atomic::{AtomicBool,Ordering};

#[derive(Debug,Clone,Copy,PartialEq,Eq,Hash,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct TextReplacementSelection {pub page_index:usize,pub match_index:usize}
#[derive(Debug,Clone,Copy,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct ReplacementPlanningProgress {pub completed:usize,pub total:usize}
#[derive(Clone,Default)]
pub struct ReplacementPlanningControl {
    cancelled:Arc<AtomicBool>,
    progress:Option<mpsc::Sender<ReplacementPlanningProgress>>,
}
impl ReplacementPlanningControl {
    pub fn new(progress:Option<mpsc::Sender<ReplacementPlanningProgress>>)->Self{Self{progress,..Default::default()}}
    pub fn cancel(&self){self.cancelled.store(true,Ordering::Relaxed);}
    pub(crate) fn check(&self)->EngineResult<()>{
        if self.cancelled.load(Ordering::Relaxed){Err(EngineError::InvalidRequest("Document replacement planning was cancelled; no partial plan was returned.".into()))}else{Ok(())}
    }
    pub(super) fn checkpoint(&self,completed:usize,total:usize)->EngineResult<()>{
        self.check()?;
        if let Some(progress)=&self.progress{let _=progress.send(ReplacementPlanningProgress{completed,total});}
        self.check()
    }
}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub enum DocumentReplacementBlocker {SelectedMatchesBlocked,PageBatchBlocked,TooManyEdits,TooManyPages,BatchBytes}
#[derive(Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct TextReplacementDocumentPlan {
    pub source_id:String,pub source_fingerprint:String,pub query:String,pub replacement:String,pub options:PageSearchOptions,
    pub total_pages:usize,pub scanned_pages:usize,pub match_count:usize,pub selected_count:usize,pub blocked_count:usize,
    pub pages:Vec<TextReplacementPagePlan>,pub edits:Vec<crate::TextBatchEdit>,
    pub batch_blocker:Option<DocumentReplacementBlocker>,pub can_apply:bool,
}
pub(super) struct Request {
    pub source:String,pub query:String,pub replacement:String,pub options:PageSearchOptions,
    pub selected:Option<Vec<TextReplacementSelection>>,pub control:ReplacementPlanningControl,
    pub reply:mpsc::Sender<EngineResult<TextReplacementDocumentPlan>>,
}
fn invalid()->EngineError{EngineError::InvalidRequest("Document replacement planning is invalid or exceeds its limits; no partial plan was returned.".into())}
pub(crate) fn validate(query:&str,replacement:&str,selected:Option<&[TextReplacementSelection]>)->EngineResult<()>{
    if query.len()>1024||query.chars().all(super::search_matching::whitespace)||replacement.len()>4096||replacement.contains('\0')||selected.is_some_and(|s|s.len()>10000){return Err(invalid());}
    if let Some(selected)=selected{
        let mut unique=HashSet::with_capacity(selected.len());
        if selected.iter().any(|s|s.page_index>=10000||s.match_index>=1000||!unique.insert(*s)){return Err(invalid());}
    }
    Ok(())
}
fn serialized_size(value:&impl Serialize)->EngineResult<usize>{
    let mut writer=crate::pdf_output::BoundedWriter::new(4*1024*1024);
    serde_json::to_writer(&mut writer,value).map_err(|_|invalid())?;
    writer.finish().map(|bytes|bytes.len()).map_err(|_|invalid())
}
impl PdfEngine {
    pub fn plan_text_replacement_document(&self,source_id:&str,query:&str,replacement:&str,options:PageSearchOptions,selected:Option<&[TextReplacementSelection]>,control:ReplacementPlanningControl)->EngineResult<TextReplacementDocumentPlan>{
        validate(query,replacement,selected)?;control.check()?;
        let(reply,receive)=mpsc::channel();
        self.send(WorkerRequest::DocumentReplacementPlan(Request{source:source_id.into(),query:query.into(),replacement:replacement.into(),options,selected:selected.map(<[TextReplacementSelection]>::to_vec),control,reply}))?;
        receive.recv().map_err(|_|EngineError::WorkerStopped)?
    }
}
impl WorkerRuntime {
    pub(super) fn handle_document_replacement_plan(&mut self,request:Request){
        let result=self.plan_text_replacement_document_raw(&request.source,&request.query,&request.replacement,request.options,request.selected.as_deref(),&request.control);
        let _=request.reply.send(result);
    }
    pub(super) fn plan_text_replacement_document_raw(&mut self,source:&str,query:&str,replacement:&str,options:PageSearchOptions,selected:Option<&[TextReplacementSelection]>,control:&ReplacementPlanningControl)->EngineResult<TextReplacementDocumentPlan>{
        use super::physical_units::Scale;
        use sha2::{Digest,Sha256};
        self.copying_document(source)?;
        let total=self.editing_document(source)?.document.page_count()as usize;
        validate(query,replacement,selected)?;
        if total==0||total>10000||selected.is_some_and(|s|s.iter().any(|s|s.page_index>=total)){return Err(invalid());}
        control.checkpoint(0,total)?;
        let fingerprint=Sha256::digest(self.document(source)?.bytes()?.as_ref()).iter().map(|b|format!("{b:02x}")).collect::<String>();
        let mut by_page=std::collections::BTreeMap::<usize,Vec<usize>>::new();
        if let Some(selected)=selected{for selection in selected{by_page.entry(selection.page_index).or_default().push(selection.match_index);}}
        let mut result=Accumulator::new(source,&fingerprint,total,query,replacement,options);
        for index in 0..total{
            control.check()?;
            let chosen=selected.map(|_|by_page.get(&index).map_or(&[][..],Vec::as_slice));
            let mut page=self.plan_text_replacement_page_raw(source,index,query,replacement,options,chosen)?;
            if page.source_fingerprint!=fingerprint||page.source_id!=source||page.page_index!=index{return Err(invalid());}
            page.scale(self.source_page_unit(source,index)?)?;
            page.check_size()?;
            result.push(page)?;
            control.checkpoint(index+1,total)?;
        }
        let result=result.finish()?;control.check()?;Ok(result)
    }
}
struct Accumulator {plan:TextReplacementDocumentPlan,page_bytes:usize}
impl Accumulator {
    fn new(source:&str,fingerprint:&str,total:usize,query:&str,replacement:&str,options:PageSearchOptions)->Self {
        Self{plan:TextReplacementDocumentPlan{source_id:source.into(),source_fingerprint:fingerprint.into(),query:query.into(),replacement:replacement.into(),options,total_pages:total,scanned_pages:0,match_count:0,selected_count:0,blocked_count:0,pages:vec![],edits:vec![],batch_blocker:None,can_apply:false},page_bytes:0}
    }
    fn push(&mut self,page:TextReplacementPagePlan)->EngineResult<()>{
        if page.page_index!=self.plan.scanned_pages||self.plan.scanned_pages>=self.plan.total_pages||page.source_id!=self.plan.source_id||page.source_fingerprint!=self.plan.source_fingerprint{return Err(invalid());}
        let count=self.plan.match_count.checked_add(page.matches.len()).filter(|n|*n<=10000).ok_or_else(invalid)?;
        if !page.matches.is_empty(){
            let bytes=serialized_size(&page)?;
            self.page_bytes=self.page_bytes.checked_add(bytes).and_then(|n|n.checked_add(1)).filter(|n|*n<=4*1024*1024).ok_or_else(invalid)?;
        }
        self.plan.scanned_pages+=1;self.plan.match_count=count;
        self.plan.selected_count+=page.selected_count;self.plan.blocked_count+=page.blocked_count;
        if !page.matches.is_empty(){self.plan.pages.push(page);}
        Ok(())
    }
    fn finish(mut self)->EngineResult<TextReplacementDocumentPlan>{
        if self.plan.scanned_pages!=self.plan.total_pages{return Err(invalid());}
        let edits=self.plan.pages.iter().flat_map(|page|page.edits.iter().cloned()).collect::<Vec<_>>();
        let changed_pages=edits.iter().map(|edit|edit.page_index).collect::<HashSet<_>>().len();
        self.plan.batch_blocker=if self.plan.blocked_count>0{Some(DocumentReplacementBlocker::SelectedMatchesBlocked)}
            else if self.plan.pages.iter().any(|page|page.batch_blocker.is_some()){Some(DocumentReplacementBlocker::PageBatchBlocked)}
            else if edits.len()>64{Some(DocumentReplacementBlocker::TooManyEdits)}
            else if changed_pages>32{Some(DocumentReplacementBlocker::TooManyPages)}
            else if !super::text_batch::batch_strings_fit(&edits){Some(DocumentReplacementBlocker::BatchBytes)}else{None};
        if self.plan.batch_blocker.is_some(){
            for page in &mut self.plan.pages{page.edits.clear();page.can_apply=false;}
        }else{self.plan.edits=edits;self.plan.can_apply=!self.plan.edits.is_empty();}
        serialized_size(&self.plan)?;Ok(self.plan)
    }
}
#[cfg(test)]
#[path="document_replacement_plan_tests.rs"]
mod tests;
