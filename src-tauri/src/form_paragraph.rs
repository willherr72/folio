//! Public physical-point facade; private projection IDs never cross the worker.
use super::*;
#[cfg(test)]
#[path="form_paragraph_tests.rs"]
mod tests;

pub(super) enum Request {
 Map{source:String,page:usize,path:Vec<usize>,request:crate::types::ParagraphDraftRequest,reply:mpsc::Sender<EngineResult<ParagraphDraftMapping>>},
 List{source:String,page:usize,path:Vec<usize>,expected:String,reply:mpsc::Sender<EngineResult<FormParagraphRuns>>},
 Inspect{source:String,page:usize,path:Vec<usize>,indices:Vec<usize>,reply:mpsc::Sender<EngineResult<StyledParagraphSelection>>},
 Suggest{source:String,page:usize,path:Vec<usize>,index:usize,expected:String,reply:mpsc::Sender<EngineResult<StyledParagraphSelection>>},
 Fit{source:String,page:usize,path:Vec<usize>,request:StyledParagraphLayoutRequest,reply:mpsc::Sender<EngineResult<AnnotationRect>>},
 Preview{source:String,occurrences:Vec<usize>,selected:usize,path:Vec<usize>,request:StyledParagraphLayoutRequest,reply:mpsc::Sender<EngineResult<PageContentReplacement>>},
}
fn bad(text:&str)->EngineError{EngineError::InvalidRequest(text.into())}
fn checked_path(path:&[usize])->EngineResult<Vec<usize>>{
 if !(2..=13).contains(&path.len()){return Err(bad("Select a bounded form paragraph path."));}Ok(path.to_vec())
}
fn checked_text(text:&str)->EngineResult<String>{if text.len()>4096{return Err(bad("The form paragraph anchor is too long."));}Ok(text.into())}
fn request_budget(request:&StyledParagraphLayoutRequest)->EngineResult<()>{
 if request.object_indices.is_empty()||request.object_indices.len()>4096||request.runs.is_empty()||request.runs.len()>4096{
  return Err(bad("Use 1 to 4096 paragraph pieces and formatting spans."));
 }
 if request.expected_text.len()>16*1024*1024{return Err(bad("The original paragraph text exceeds the source size limit."));}
 let mut bytes=0usize;
 for run in &request.runs{bytes=bytes.checked_add(run.text.len()).filter(|n|*n<=4096).ok_or_else(||bad("Use at most 4096 UTF-8 bytes for a paragraph."))?;}
 if request.style_changes.len()>64||request.style_changes.iter().any(|s|s.font_id.as_ref().is_some_and(|v|v.len()>512)||s.color.as_ref().is_some_and(|v|v.len()>64)){
  return Err(bad("The paragraph style request exceeds its input limits."));
 }
 Ok(())
}
fn check_scope(indices:&[usize],allowed:&[usize])->EngineResult<()>{
 if indices.is_empty()||indices.len()>4096||indices.windows(2).any(|w|w[0]>=w[1]){return Err(bad("Select 1 to 4096 paragraph pieces in order."));}
 let allowed=allowed.iter().copied().collect::<HashSet<_>>();
 if indices.iter().any(|i|!allowed.contains(i)){return Err(bad("The paragraph selection is outside the chosen form occurrence."));}Ok(())
}
impl PdfEngine {
 pub fn map_form_paragraph_draft(&self,source_id:&str,page_index:usize,path:&[usize],indices:&[usize],expected_paragraph:&str,anchor:usize,expected_source:&str)->EngineResult<ParagraphDraftMapping>{
  let path=checked_path(path)?;let request=crate::types::ParagraphDraftRequest::checked(indices,expected_paragraph,anchor,expected_source,true)?;
  self.form_paragraph_request(|reply|Request::Map{source:source_id.into(),page:page_index,path,request,reply})
 }
 fn form_paragraph_request<T>(&self,make:impl FnOnce(mpsc::Sender<EngineResult<T>>)->Request)->EngineResult<T>{
  let (reply,receive)=mpsc::channel();self.send(WorkerRequest::FormParagraph(make(reply)))?;receive.recv().map_err(|_|EngineError::WorkerStopped)?
 }
 pub fn list_form_paragraph_runs(&self,source_id:&str,page_index:usize,path:&[usize],expected:&str)->EngineResult<FormParagraphRuns>{
  let path=checked_path(path)?;let expected=checked_text(expected)?;
  self.in_points(source_id,page_index,self.form_paragraph_request(|reply|Request::List{source:source_id.into(),page:page_index,path,expected,reply}))
 }
 fn inspect_form_styled_raw(&self,source_id:&str,page_index:usize,path:&[usize],indices:&[usize])->EngineResult<StyledParagraphSelection>{
  let path=checked_path(path)?;if indices.len()>4096{return Err(bad("Select at most 4096 paragraph pieces."));}
  self.form_paragraph_request(|reply|Request::Inspect{source:source_id.into(),page:page_index,path,indices:indices.to_vec(),reply})
 }
 pub fn inspect_form_styled_paragraph(&self,source_id:&str,page_index:usize,path:&[usize],indices:&[usize])->EngineResult<StyledParagraphSelection>{
  self.in_points(source_id,page_index,self.inspect_form_styled_raw(source_id,page_index,path,indices))
 }
 pub fn suggest_form_styled_paragraph(&self,source_id:&str,page_index:usize,path:&[usize],index:usize,expected:&str)->EngineResult<StyledParagraphSelection>{
  let path=checked_path(path)?;let expected=checked_text(expected)?;
  self.in_points(source_id,page_index,self.form_paragraph_request(|reply|Request::Suggest{source:source_id.into(),page:page_index,path,index,expected,reply}))
 }
 fn form_styled_in_raw(&self,source_id:&str,page:usize,path:&[usize],request:&StyledParagraphLayoutRequest)->EngineResult<StyledParagraphLayoutRequest>{
  request_budget(request)?;
  if request.style_changes.iter().filter_map(|s|s.font_size).any(|size|!physical_units::paragraph_size(size,1.)){return Err(bad("Use a font size from 1 to 300 pt."));}
  let unit=self.page_unit(source_id,page)?;let mut raw=request.clone();raw.scale(unit.recip())?;
  if unit!=1. {
   let checked=self.inspect_form_styled_raw(source_id,page,path,&request.object_indices)?;
   physical_units::restore_rect(&mut raw.bounds,&request.bounds,&checked.bounds,unit);
   physical_units::restore_anchor(&mut raw.baseline_anchor,request.baseline_anchor,checked.baseline_anchor,unit);
   for (change,submitted) in raw.style_changes.iter_mut().zip(&request.style_changes){
    if let Some(source)=checked.styles.iter().find(|s|s.style_id==submitted.source_style_id){
     if let (Some(value),Some(physical))=(&mut change.font_size,submitted.font_size){physical_units::restore_value(value,physical,source.font_size,unit);}
    }
   }
  }
  Ok(raw)
 }
 pub fn fit_form_styled_paragraph(&self,source_id:&str,page:usize,path:&[usize],request:&StyledParagraphLayoutRequest)->EngineResult<AnnotationRect>{
  let path=checked_path(path)?;let request=self.form_styled_in_raw(source_id,page,&path,request)?;
  self.in_points(source_id,page,self.form_paragraph_request(|reply|Request::Fit{source:source_id.into(),page,path,request,reply}))
 }
 pub fn preview_form_styled_paragraph_occurrence(&self,source_id:&str,occurrences:&[usize],selected:usize,path:&[usize],request:&StyledParagraphLayoutRequest)->EngineResult<PageContentReplacement>{
  if occurrences.is_empty()||occurrences.len()>100_000||selected>=occurrences.len(){return Err(bad("Invalid form paragraph page occurrence selection."));}
  let path=checked_path(path)?;let request=self.form_styled_in_raw(source_id,occurrences[selected],&path,request)?;
  let mut result=self.form_paragraph_request(|reply|Request::Preview{source:source_id.into(),occurrences:occurrences.to_vec(),selected,path,request,reply})?;
  result.document=self.document_in_points(Ok(result.document))?;Ok(result)
 }
}
impl WorkerRuntime {
 pub(super) fn handle_form_paragraph(&mut self,request:Request){match request {
  Request::Map{source,page,path,request,reply}=>{
   let result=self.with_form_paragraph_projection(&source,page,&path,|rt,id,scope|{
    check_scope(&request.indices,&scope.object_indices)?;
    if scope.anchor_index!=request.anchor||scope.anchor_text!=request.source{return Err(bad("The selected form anchor changed."));}
    rt.map_paragraph_draft_inner(id,0,&request)
   }).map(|mut mapping|{mapping.source_id=source;mapping.page_index=page;mapping});let _=reply.send(result);
  }
  Request::List{source,page,path,expected,reply}=>{
   let result=self.with_form_paragraph_projection(&source,page,&path,|rt,id,scope|{
    if scope.anchor_text!=expected{return Err(bad("The selected form text changed. Select it again."));}
    let mut listing=rt.list_transaction_text_runs(id,0,true)?;listing.runs.retain(|run|scope.object_indices.contains(&run.object_index));
    if !listing.runs.iter().any(|run|run.object_index==scope.anchor_index){return Err(bad("The promoted paragraph anchor is unavailable."));}
    Ok(FormParagraphRuns{anchor_index:scope.anchor_index,runs:listing.runs})
   });let _=reply.send(result);
  }
  Request::Inspect{source,page,path,indices,reply}=>{
   let result=self.with_form_paragraph_projection(&source,page,&path,|rt,id,scope|{check_scope(&indices,&scope.object_indices)?;rt.inspect_styled_paragraph(id,0,&indices)});let _=reply.send(result);
  }
  Request::Suggest{source,page,path,index,expected,reply}=>{
   let result=self.with_form_paragraph_projection(&source,page,&path,|rt,id,scope|{
    check_scope(&[index],&scope.object_indices)?;let suggested=rt.suggest_styled_paragraph(id,0,index,&expected)?;
    let indices=suggested.object_indices.into_iter().filter(|i|scope.object_indices.contains(i)).collect::<Vec<_>>();
    check_scope(&indices,&scope.object_indices)?;rt.inspect_styled_paragraph(id,0,&indices)
   });let _=reply.send(result);
  }
  Request::Fit{source,page,path,request,reply}=>{
   let result=self.with_form_paragraph_projection(&source,page,&path,|rt,id,scope|{check_scope(&request.object_indices,&scope.object_indices)?;rt.fit_styled_paragraph(id,0,&request)});let _=reply.send(result);
  }
  Request::Preview{source,occurrences,selected,path,request,reply}=>{
   let result=self.publish_text_occurrence(&source,&occurrences,selected,|rt,page|rt.with_form_paragraph_projection(&source,page,&path,|rt,id,scope|{check_scope(&request.object_indices,&scope.object_indices)?;rt.preview_styled_paragraph(id,0,&request)}));
   if let Err(mpsc::SendError(Ok(result)))=reply.send(result){self.discard_unpublished_source(&result.document.id);}
  }
 }}
}
