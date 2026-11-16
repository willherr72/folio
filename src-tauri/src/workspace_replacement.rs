//! Current-workspace replacement planning. Executable edits never cross IPC.
use super::*;
use serde::{Deserialize,Serialize};
use super::workspace_text_batch::{WorkspaceTextBatchEdit,WorkspaceSourceReplacement};
use super::physical_units::Scale;
use super::replacement_plan::{TextReplacementPagePlan,TextReplacementMatch,TextReplacementBlocker};
use sha2::{Digest,Sha256};
#[path="workspace_replacement_overlays.rs"]mod overlays;
pub(crate) const MAX_PENDING_BYTES:usize=512*1024*1024;
pub(crate) struct WorkspaceReplacementSnapshot {pub(crate) pages:Vec<PagePlan>,_resources:super::page_image_render::Resources}

#[derive(Debug,Clone,PartialEq,Eq,Hash,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct WorkspaceReplacementSelection {pub page_id:String,pub match_index:usize}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct WorkspaceReplacementIntent {
    pub query:String,pub replacement:String,
    #[serde(default)]pub options:PageSearchOptions,
    #[serde(default)]pub selected:Option<Vec<WorkspaceReplacementSelection>>,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct WorkspaceReplacementMatch {
    pub match_index:usize,pub overlay_id:Option<String>,pub rects:Vec<AnnotationRect>,pub selected:bool,pub blocker:Option<String>,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct WorkspaceReplacementPage {pub page_id:String,pub matches:Vec<WorkspaceReplacementMatch>}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct WorkspaceOverlayReplacement {pub page_id:String,pub overlay_id:String,pub expected_text:String,pub text:Option<String>}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct WorkspaceReplacementPlan {
    pub plan_id:String,pub query:String,pub replacement:String,pub options:PageSearchOptions,
    pub pages:Vec<WorkspaceReplacementPage>,pub selected_count:usize,pub blocked_count:usize,
    pub affected_page_ids:Vec<String>,pub batch_blocker:Option<String>,pub can_apply:bool,
    #[serde(skip)]pub(super) binding:String,
    #[serde(skip)]pub(super) native_edits:Vec<WorkspaceTextBatchEdit>,
    #[serde(skip)]pub(super) overlay_edits:Vec<WorkspaceOverlayReplacement>,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct WorkspaceReplacementResult {
    pub sources:Vec<WorkspaceSourceReplacement>,pub overlays:Vec<WorkspaceOverlayReplacement>,
    #[serde(skip)]pub(crate) retained_bytes:usize,
}

pub(super) enum Request {
    #[cfg(test)]
    Count{reply:mpsc::Sender<usize>},
    Capture{pages:Vec<PagePlan>,control:crate::ReplacementPlanningControl,reply:mpsc::Sender<EngineResult<()>>},
    CheckSources{originals:Vec<String>,candidates:Vec<String>,reply:mpsc::Sender<EngineResult<()>>},
    Plan{pages:Vec<PagePlan>,intent:WorkspaceReplacementIntent,control:crate::ReplacementPlanningControl,reply:mpsc::Sender<EngineResult<WorkspaceReplacementPlan>>},
    Preview{pages:Vec<PagePlan>,plan:WorkspaceReplacementPlan,control:crate::ReplacementPlanningControl,reply:mpsc::Sender<EngineResult<WorkspaceReplacementResult>>},
}
fn invalid()->EngineError{EngineError::InvalidRequest("Workspace replacement is invalid, stale or exceeds its limits; no partial result was returned.".into())}
fn bounded(value:&impl Serialize,max:usize)->EngineResult<Vec<u8>>{
    let mut writer=crate::pdf_output::BoundedWriter::new(max);serde_json::to_writer(&mut writer,value).map_err(|_|invalid())?;writer.finish().map_err(|_|invalid())
}
fn validate_intent(pages:&[PagePlan],intent:&WorkspaceReplacementIntent)->EngineResult<()> {
    if pages.is_empty()||pages.len()>10000||intent.selected.as_ref().is_some_and(|s|s.len()>10000){return Err(invalid());}
    super::document_replacement_plan::validate(&intent.query,&intent.replacement,None)?;
    if let Some(selected)=&intent.selected{let mut seen=HashSet::new();if selected.iter().any(|s|s.page_id.is_empty()||s.page_id.len()>256||s.match_index>=1000||!seen.insert(s)){return Err(invalid());}}
    Ok(())
}
fn visible(rects:&[AnnotationRect],crop:Option<&AnnotationRect>)->bool {
    crop.is_none_or(|crop|rects.iter().any(|r|r.x<crop.x+crop.width&&r.x+r.width>crop.x&&r.y<crop.y+crop.height&&r.y+r.height>crop.y))
}
fn blocker(value:TextReplacementBlocker)->String {
    use TextReplacementBlocker::*;
    match value{
        NeedsRemoval=>"Removing this entire native text object needs the text-removal tool.",
        NeedsParagraphLayout=>"This replacement needs paragraph layout.",
        CrossObject=>"This match spans separate PDF text objects.",
        Ineligible=>"The original text cannot be edited with its current font or permissions.",
        ReplacementTooLong|TooManyEdits|BatchBytes=>"The selected native text exceeds the replacement batch limits.",
        _=>"This match cannot be bound safely to the original PDF text.",
    }.into()
}
impl PdfEngine {
    #[cfg(test)]
    pub(crate) fn workspace_replacement_source_count(&self)->EngineResult<usize>{
        let(reply,receive)=mpsc::channel();self.send(WorkerRequest::WorkspaceReplacement(Request::Count{reply}))?;receive.recv().map_err(|_|EngineError::WorkerStopped)
    }
    pub(crate) fn capture_workspace_replacement(&self,pages:Vec<PagePlan>,control:crate::ReplacementPlanningControl)->EngineResult<WorkspaceReplacementSnapshot>{
        if pages.is_empty()||pages.len()>10000{return Err(invalid());}bounded(&pages,8*1024*1024)?;
        let resources=self.capture_export_resources(&pages)?;let(reply,receive)=mpsc::channel();
        self.send(WorkerRequest::WorkspaceReplacement(Request::Capture{pages:pages.clone(),control,reply}))?;
        receive.recv().map_err(|_|EngineError::WorkerStopped)??;Ok(WorkspaceReplacementSnapshot{pages,_resources:resources})
    }
    pub(crate) fn check_workspace_replacement_sources(&self,originals:Vec<String>,candidates:Vec<String>)->EngineResult<()>{
        if originals.len()>10000||candidates.len()>32{return Err(invalid());}let(reply,receive)=mpsc::channel();self.send(WorkerRequest::WorkspaceReplacement(Request::CheckSources{originals,candidates,reply}))?;
        receive.recv().map_err(|_|EngineError::WorkerStopped)?
    }
    pub fn plan_workspace_replacement(&self,pages:&[PagePlan],intent:&WorkspaceReplacementIntent,control:crate::ReplacementPlanningControl)->EngineResult<WorkspaceReplacementPlan>{
        validate_intent(pages,intent)?;bounded(&pages,8*1024*1024)?;control.check()?;
        let(reply,receive)=mpsc::channel();self.send(WorkerRequest::WorkspaceReplacement(Request::Plan{pages:pages.to_vec(),intent:intent.clone(),control,reply}))?;
        receive.recv().map_err(|_|EngineError::WorkerStopped)?
    }
    pub fn preview_workspace_replacement(&self,pages:&[PagePlan],plan:&WorkspaceReplacementPlan,control:crate::ReplacementPlanningControl)->EngineResult<WorkspaceReplacementResult>{
        bounded(&pages,8*1024*1024)?;control.check()?;
        let(reply,receive)=mpsc::channel();self.send(WorkerRequest::WorkspaceReplacement(Request::Preview{pages:pages.to_vec(),plan:plan.clone(),control,reply}))?;
        receive.recv().map_err(|_|EngineError::WorkerStopped)?
    }
}
impl WorkerRuntime {
    pub(super) fn handle_workspace_replacement(&mut self,request:Request){
        match request{
            #[cfg(test)]
            Request::Count{reply}=>{let _=reply.send(self.documents.len());}
            Request::Capture{pages,control,reply}=>{let result=(||{self.workspace_binding(&pages,&control)?;self.raw_workspace(&pages,&control)?;Ok(())})();let _=reply.send(result);}
            Request::CheckSources{originals,candidates,reply}=>{let result=(||{for id in originals{self.copying_document(&id)?;}for id in candidates{self.copying_document(&id)?;self.editing_document(&id)?;}Ok(())})();let _=reply.send(result);}
            Request::Plan{pages,intent,control,reply}=>{let result=self.plan_workspace_replacement(&pages,&intent,&control);let _=reply.send(result);}
            Request::Preview{pages,plan,control,reply}=>{
                let result=self.preview_workspace_replacement(&pages,&plan,&control);
                if let Err(mpsc::SendError(Ok(result)))=reply.send(result){for source in result.sources{self.discard_unpublished_source(&source.document.id);}}
            }
        }
    }
    fn workspace_binding(&self,pages:&[PagePlan],control:&crate::ReplacementPlanningControl)->EngineResult<String>{
        let mut sources=std::collections::BTreeMap::new();
        bounded(&pages,8*1024*1024)?;
        for page in pages{
            control.check()?;
            self.copying_document(&page.source_id)?;
            if !sources.contains_key(&page.source_id){let digest=Sha256::digest(self.document(&page.source_id)?.bytes()?.as_ref());sources.insert(page.source_id.clone(),digest.iter().map(|b|format!("{b:02x}")).collect::<String>());}
        }
        let bytes=bounded(&(pages,sources),8*1024*1024)?;Ok(Sha256::digest(bytes).iter().map(|b|format!("{b:02x}")).collect())
    }
    pub(super) fn validate_replacement_raw_workspace(&self,pages:&[PagePlan],control:&crate::ReplacementPlanningControl)->EngineResult<()>{
        if pages.is_empty()||pages.len()>10000{return Err(invalid());}let mut ids=HashSet::new();
        for page in pages{control.check()?;if !ids.insert(&page.id){return Err(invalid());}self.validate_export(&ExportRequest{pages:vec![page.clone()],flatten:false})?;}Ok(())
    }
    fn raw_workspace(&self,pages:&[PagePlan],control:&crate::ReplacementPlanningControl)->EngineResult<Vec<PagePlan>>{
        let mut raw=pages.to_vec();for page in &mut raw{control.check()?;page.scale(self.source_page_unit(&page.source_id,page.page_index)?.recip())?;}
        self.validate_replacement_raw_workspace(&raw,control)?;Ok(raw)
    }
    fn workspace_native_discovery(&mut self,page:&PagePlan,intent:&WorkspaceReplacementIntent,editable:bool,binding:&str)->EngineResult<TextReplacementPagePlan>{
        if editable{return self.plan_text_replacement_page_raw(&page.source_id,page.page_index,&intent.query,&intent.replacement,intent.options,Some(&[]));}
        let found=self.search_page(&page.source_id,page.page_index,&intent.query,intent.options)?;if found.truncated{return Err(invalid());}
        Ok(TextReplacementPagePlan{source_id:page.source_id.clone(),source_fingerprint:binding.into(),page_index:page.page_index,width:page.width,height:page.height,query:intent.query.clone(),replacement:intent.replacement.clone(),options:intent.options,
            matches:found.matches.into_iter().enumerate().map(|(i,rects)|TextReplacementMatch{match_index:i,character_start:0,character_end:0,rects,source_range:None,selected:false,blocker:Some(TextReplacementBlocker::Ineligible),source_reason:None}).collect(),edits:vec![],selected_count:0,blocked_count:0,batch_blocker:None,can_apply:false})
    }
    pub(super) fn plan_workspace_replacement(&mut self,pages:&[PagePlan],intent:&WorkspaceReplacementIntent,control:&crate::ReplacementPlanningControl)->EngineResult<WorkspaceReplacementPlan>{
        validate_intent(pages,intent)?;control.check()?;let binding=self.workspace_binding(pages,control)?;let raw=self.raw_workspace(pages,control)?;
        let selected=intent.selected.as_ref().map(|s|s.iter().map(|s|(s.page_id.as_str(),s.match_index)).collect::<HashSet<_>>());
        let chosen=|id:&str,index:usize|selected.as_ref().is_none_or(|s|s.contains(&(id,index)));
        let mut result=WorkspaceReplacementPlan{plan_id:Uuid::new_v4().to_string(),query:intent.query.clone(),replacement:intent.replacement.clone(),options:intent.options,pages:vec![],selected_count:0,blocked_count:0,affected_page_ids:vec![],batch_blocker:None,can_apply:false,binding,native_edits:vec![],overlay_edits:vec![]};
        let mut total=0;let mut bytes=0usize;control.checkpoint(0,pages.len())?;
        for (index,(page,raw_page))in pages.iter().zip(&raw).enumerate(){
            control.check()?;let editable=self.editing_document(&page.source_id).is_ok();let unit=self.source_page_unit(&page.source_id,page.page_index)?;
            let mut discovery=self.workspace_native_discovery(raw_page,intent,editable,&result.binding)?;discovery.scale(unit)?;
            let visible_indices=discovery.matches.iter().filter(|m|visible(&m.rects,page.crop.as_ref())).map(|m|m.match_index).collect::<Vec<_>>();
            let selected_indices=visible_indices.iter().enumerate().filter(|(ordinal,_)|chosen(&page.id,*ordinal)).map(|(_,source)|*source).collect::<Vec<_>>();
            let mut native=if editable{self.plan_text_replacement_page_raw(&page.source_id,page.page_index,&intent.query,&intent.replacement,intent.options,Some(&selected_indices))?}else{discovery};
            if editable{native.scale(unit)?;}
            if native.batch_blocker.is_some(){result.batch_blocker=Some("The selected native text exceeds a page batch limit.".into());}
            let mut rows=Vec::new();
            for native_index in visible_indices{
                let found=&native.matches[native_index];let ordinal=rows.len();rows.push(WorkspaceReplacementMatch{match_index:ordinal,overlay_id:None,rects:found.rects.clone(),selected:chosen(&page.id,ordinal),blocker:found.blocker.map(blocker)});
            }
            let native_changed=!native.edits.is_empty();
            result.native_edits.extend(native.edits.into_iter().map(|edit|WorkspaceTextBatchEdit{page_id:page.id.clone(),edit}));
            let before=result.overlay_edits.len();
            self.plan_workspace_overlays(page,intent,&chosen,&mut rows,&mut result.overlay_edits)?;
            if native_changed||result.overlay_edits.len()>before{result.affected_page_ids.push(page.id.clone());}
            if rows.len()>1000{return Err(invalid());}total+=rows.len();if total>10000{return Err(invalid());}
            result.selected_count+=rows.iter().filter(|m|m.selected).count();result.blocked_count+=rows.iter().filter(|m|m.selected&&m.blocker.is_some()).count();
            if !rows.is_empty(){result.pages.push(WorkspaceReplacementPage{page_id:page.id.clone(),matches:rows});}
            bounded(&result,4*1024*1024)?;control.checkpoint(index+1,pages.len())?;
        }
        for edit in &result.native_edits{bytes=bytes.checked_add(edit.edit.expected_text.len()+edit.edit.replacement.len()).ok_or_else(invalid)?;}
        for edit in &result.overlay_edits{bytes=bytes.checked_add(edit.expected_text.len()+edit.text.as_ref().map_or(0,String::len)).ok_or_else(invalid)?;}
        if selected.as_ref().is_some_and(|set|set.iter().any(|(id,index)|!result.pages.iter().any(|p|p.page_id==*id&&p.matches.get(*index).is_some()))){return Err(invalid());}
        if result.blocked_count>0{result.batch_blocker=Some("Some selected matches cannot be replaced. Review their reasons or choose an explicit subset.".into());}
        if result.native_edits.len()+result.overlay_edits.len()>64||result.affected_page_ids.len()>32||bytes>512*1024{result.batch_blocker=Some("Replace is limited to 64 changed text objects, 32 changed pages and 512 KiB of batch text.".into());}
        if result.batch_blocker.is_some(){result.native_edits.clear();result.overlay_edits.clear();result.affected_page_ids.clear();}
        result.can_apply=!result.native_edits.is_empty()||!result.overlay_edits.is_empty();bounded(&result,4*1024*1024)?;control.check()?;Ok(result)
    }
    pub(super) fn preview_workspace_replacement(&mut self,pages:&[PagePlan],plan:&WorkspaceReplacementPlan,control:&crate::ReplacementPlanningControl)->EngineResult<WorkspaceReplacementResult>{
        control.check()?;if !plan.can_apply||plan.batch_blocker.is_some()||self.workspace_binding(pages,control)?!=plan.binding{return Err(invalid());}
        let raw=self.raw_workspace(pages,control)?;
        let staged=if plan.native_edits.is_empty(){vec![]}else{self.replace_workspace_text_batch(&raw,&plan.native_edits,control)?.sources};
        let mut result=WorkspaceReplacementResult{sources:staged,overlays:plan.overlay_edits.clone(),retained_bytes:0};
        let operation=(||{
            result.retained_bytes=self.workspace_candidate_bytes(result.sources.iter().map(|source|source.document.id.as_str()))?;
            for source in &mut result.sources{for(index,page)in source.document.pages.iter_mut().enumerate(){page.scale(self.source_page_unit(&source.document.id,index)?)?;}}
            bounded(&result,4*1024*1024)?;control.check()?;Ok(())
        })();
        if let Err(error)=operation{for source in result.sources{self.discard_unpublished_source(&source.document.id);}return Err(error);}Ok(result)
    }
}
#[cfg(test)]
#[path="workspace_replacement_tests.rs"]
mod tests;
