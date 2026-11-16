//! Atomic staging across immutable sources and independently selected page occurrences.
use super::*;
use serde::{Deserialize,Serialize};

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct WorkspaceTextBatchEdit {pub page_id:String,pub edit:crate::TextBatchEdit}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct WorkspaceSourceReplacement {
    pub source_id:String,pub document:DocumentInfo,pub page_indices:Vec<usize>,
}
#[derive(Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct WorkspaceTextBatchResult {pub sources:Vec<WorkspaceSourceReplacement>}

pub(super) struct Request {
    pub pages:Vec<PagePlan>,pub edits:Vec<WorkspaceTextBatchEdit>,
    pub control:crate::ReplacementPlanningControl,
    pub reply:mpsc::Sender<EngineResult<WorkspaceTextBatchResult>>,
}
fn invalid()->EngineError{EngineError::InvalidRequest("Workspace text batch has invalid, stale or overlapping page selections.".into())}
fn bounded(result:&WorkspaceTextBatchResult)->EngineResult<()>{
    let mut writer=crate::pdf_output::BoundedWriter::new(4*1024*1024);
    serde_json::to_writer(&mut writer,result).map_err(|_|invalid())?;
    writer.finish().map(|_|()).map_err(|_|invalid())
}

impl PdfEngine {
    /// Low-level staging for a trusted workspace planner; this accepts no output
    /// paths and does not adopt sources into a desktop document or write a PDF.
    pub fn replace_workspace_text_batch(&self,pages:&[PagePlan],edits:&[WorkspaceTextBatchEdit],control:crate::ReplacementPlanningControl)->EngineResult<WorkspaceTextBatchResult>{
        control.check()?;
        let pages=self.export_in_raw(ExportRequest{pages:pages.to_vec(),flatten:false})?.pages;
        let(reply,receive)=mpsc::channel();
        self.send(WorkerRequest::WorkspaceTextBatch(Request{pages,edits:edits.to_vec(),control,reply}))?;
        let mut result=receive.recv().map_err(|_|EngineError::WorkerStopped)??;
        let converted=(||{
            for source in &mut result.sources{source.document=self.document_in_points(Ok(source.document.clone()))?;}
            bounded(&result)
        })();
        if let Err(error)=converted{
            for source in result.sources{let _=self.close_document(&source.document.id);}return Err(error);
        }
        Ok(result)
    }
}

impl WorkerRuntime {
    pub(super) fn handle_workspace_text_batch(&mut self,request:Request){
        let result=self.replace_workspace_text_batch(&request.pages,&request.edits,&request.control);
        if let Err(mpsc::SendError(Ok(result)))=request.reply.send(result){
            for source in result.sources{self.discard_unpublished_source(&source.document.id);}
        }
    }
    pub(super) fn replace_workspace_text_batch(&mut self,pages:&[PagePlan],edits:&[WorkspaceTextBatchEdit],control:&crate::ReplacementPlanningControl)->EngineResult<WorkspaceTextBatchResult>{
        control.check()?;
        self.validate_replacement_raw_workspace(pages,control)?;
        let locations=pages.iter().enumerate().map(|(i,page)|(page.id.as_str(),i)).collect::<HashMap<_,_>>();
        let mut shape=Vec::with_capacity(edits.len());
        let mut prepared=Vec::with_capacity(edits.len());
        for item in edits{
            let index=*locations.get(item.page_id.as_str()).ok_or_else(invalid)?;
            let page=&pages[index];if item.edit.page_index!=page.page_index{return Err(invalid());}
            self.copying_document(&page.source_id)?;self.editing_document(&page.source_id)?;
            let mut edit=item.edit.clone();edit.page_index=index;shape.push(edit);
        }
        super::text_batch::validate_shape(&shape)?;
        for item in edits{
            let index=locations[item.page_id.as_str()];
            prepared.push((index,&item.edit,self.text_batch_target(&pages[index].source_id,&item.edit)?));
        }
        prepared.sort_by(|(a,ae,_),(b,be,_)|a.cmp(b).then_with(||be.object_path.cmp(&ae.object_path)));
        let original_ids=self.documents.keys().cloned().collect::<HashSet<_>>();
        let mut staged=HashMap::<String,PageContentReplacement>::new();
        let operation=(||{
            control.checkpoint(0,edits.len())?;
            for (stage,(index,edit,original)) in prepared.into_iter().enumerate(){
                control.check()?;let page=&pages[index];
                let group=pages.iter().filter(|p|p.source_id==page.source_id).collect::<Vec<_>>();
                let selected=group.iter().position(|p|p.id==page.id).ok_or_else(invalid)?;
                let current=staged.get(&page.source_id);
                let source=current.map_or(page.source_id.as_str(),|c|c.document.id.as_str());
                let order=current.map_or_else(||group.iter().map(|p|p.page_index).collect::<Vec<_>>(),|c|c.page_indices.clone());
                let mut rebound=edit.clone();rebound.page_index=order[selected];
                if self.text_batch_target(source,&rebound)?!=original{return Err(invalid());}
                let count=self.document(source)?.document.page_count() as usize;
                let duplicate=order.iter().filter(|p|**p==rebound.page_index).count()>1;
                let first=order.iter().position(|p|*p==rebound.page_index).ok_or_else(invalid)?;
                let expected=order.iter().enumerate().map(|(i,p)|if !duplicate||*p!=rebound.page_index||(i==selected)==(first==selected){*p}else{count}).collect::<Vec<_>>();
                let next=if edit.object_path.len()==1{
                    self.replace_text_occurrence(source,&order,selected,edit.object_path[0],&edit.expected_text,&edit.replacement,edit.font_id.as_deref())?
                }else{
                    self.replace_complete_form_text_occurrence(source,&order,selected,&edit.object_path,&edit.expected_text,&edit.replacement,edit.font_id.as_deref())?
                };
                if original_ids.contains(next.document.id.as_str())||staged.values().any(|c|c.document.id==next.document.id){return Err(invalid());}
                if next.page_indices!=expected||next.document.pages.len()!=count+usize::from(duplicate){
                    self.discard_unpublished_source(&next.document.id);return Err(invalid());
                }
                if let Some(previous)=staged.insert(page.source_id.clone(),next){self.discard_unpublished_source(&previous.document.id);}
                self.workspace_candidate_bytes(staged.values().map(|candidate|candidate.document.id.as_str()))?;
                control.checkpoint(stage+1,edits.len())?;
            }
            let mut seen=HashSet::new();
            let sources=pages.iter().filter(|page|seen.insert(page.source_id.as_str())).filter_map(|page|staged.get(&page.source_id).map(|candidate|WorkspaceSourceReplacement{
                source_id:page.source_id.clone(),document:candidate.document.clone(),page_indices:candidate.page_indices.clone(),
            })).collect::<Vec<_>>();
            let result=WorkspaceTextBatchResult{sources};bounded(&result)?;control.check()?;Ok(result)
        })();
        if operation.is_err(){for candidate in staged.into_values(){self.discard_unpublished_source(&candidate.document.id);}}
        operation
    }
    pub(super) fn workspace_candidate_bytes<'a>(&self,ids:impl IntoIterator<Item=&'a str>)->EngineResult<usize>{
        let mut total=0usize;
        for id in ids{total=total.checked_add(self.document(id)?.bytes()?.as_ref().len()).filter(|n|*n<=super::workspace_replacement::MAX_PENDING_BYTES).ok_or_else(||EngineError::InvalidRequest("Replacement previews exceed 512 MiB. Choose fewer changed sources.".into()))?;}
        Ok(total)
    }
}

#[cfg(test)]
#[path="workspace_text_batch_tests.rs"]
mod tests;
