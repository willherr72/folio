use super::*;
use crate::{SplitPdfRequest,PageImageControl,PageImageExportResult,PageImageProgress};
use crate::page_image_export::{bad,io_error,BATCH_BYTES};
use crate::image_export_folder::OwnedImageFolder;
use super::page_image_render::{Resources,Progress};

impl PdfEngine{
 pub fn split_pdf(&self,mut request:SplitPdfRequest,parent:impl AsRef<Path>,control:Arc<PageImageControl>,progress:Progress)->EngineResult<Option<PageImageExportResult>>{
  request.validate()?;if control.cancelled(){return Ok(None);}
  let resources=self.capture_export_resources(&request.pages)?;
  request.pages=self.export_in_raw(ExportRequest{pages:request.pages,flatten:false})?.pages;
  let(reply,receive)=mpsc::channel();
  self.send(WorkerRequest::SplitPdf{request,parent:parent.as_ref().to_path_buf(),control,progress,resources,reply})?;
  receive.recv().map_err(|_|EngineError::WorkerStopped)?
 }
}
impl WorkerRuntime{
 pub(super) fn split_pdf(&mut self,request:SplitPdfRequest,parent:PathBuf,control:Arc<PageImageControl>,progress:Progress,resources:Resources)->EngineResult<Option<PageImageExportResult>>{
  let previous=std::mem::replace(&mut self.fonts,resources.fonts.clone());
  let result=self.split_pdf_owned(request,parent,&control,&progress);self.fonts=previous;result
 }
 fn split_pdf_owned(&mut self,request:SplitPdfRequest,parent:PathBuf,control:&Arc<PageImageControl>,progress:&Progress)->EngineResult<Option<PageImageExportResult>>{
  request.validate()?;let total=request.parts.len();let report=|completed,stage|progress(PageImageProgress{completed,total,stage});
  report(0,"validating");if control.cancelled(){return Ok(None);}
  let batch=ExportRequest{pages:request.pages,flatten:false};self.validate_export(&batch)?;
  if request.metadata.is_some(){self.authorize_metadata_edit(&batch)?;}
  for page in &batch.pages{
   let source=self.document(&page.source_id)?;
   if source.for_printing||source.policy.as_ref().is_some_and(|p|!p.protection.permissions().assemble){return Err(EngineError::PermissionDenied("splitting pages"));}
  }
  let bookmarks=match request.bookmarks{Some(items)=>items,None=>self.inherited_bookmarks(&batch)?};
  crate::bookmarks::validate(&bookmarks,&batch.pages.iter().map(|p|p.id.clone()).collect())?;
  // Check every part before owning any output, without retaining N copies of its metadata.
  for part in &request.parts{
   if control.cancelled(){return Ok(None);}
   let pages=batch.pages[part.start..part.end].to_vec();
   let map=pages.iter().map(|p|(p.id.clone(),p.id.clone())).collect();
   let items=crate::bookmarks::remap(&bookmarks,&map);
   self.authorize_composition(&ExportRequest{pages,flatten:false},&items,CompositionIntent::Save)?;
  }
  if control.cancelled(){return Ok(None);}
  let mut folder=OwnedImageFolder::create_split(&parent)?;
  let outcome=(||->EngineResult<Option<PageImageExportResult>>{
   let mut written=0usize;
   for(index,part)in request.parts.iter().enumerate(){
    report(index,"composing");if control.cancelled(){return Ok(None);}
    let pages=batch.pages[part.start..part.end].to_vec();
    let map=pages.iter().map(|p|(p.id.clone(),p.id.clone())).collect();
    let items=crate::bookmarks::remap(&bookmarks,&map);
    let monitor=crate::pdf_save::PdfSaveMonitor{control:control.clone(),total:pages.len(),report:Arc::new(|_|{})};
    let(bytes,_)=self.compose_pdf_with_route_options(ExportRequest{pages,flatten:false},Some(items),CompositionIntent::Save,Some(&monitor),None,true,request.metadata.as_ref())?;
    if bytes.len()>BATCH_BYTES.saturating_sub(written){return Err(bad("Split PDFs exceed the 1 GiB batch output limit. Export fewer pages at a time."));}
    report(index,"writing");if control.cancelled(){return Ok(None);}
    folder.write(&format!("part-{:04}.pdf",index+1),&bytes)?;written+=bytes.len();drop(bytes);
    report(index+1,"writing");if control.cancelled(){return Ok(None);}
   }
   if !control.commit(){return Ok(None);}
   let path=folder.finish()?;control.completed();report(total,"completed");Ok(Some(PageImageExportResult{folder:path,count:total}))
  })();
  let output=folder.path().to_path_buf();
  match outcome{
   Ok(Some(result))=>Ok(Some(result)),
   Ok(None)=>{folder.rollback().map_err(|e|io_error(format!("PDF split cancelled. Incomplete export remains at {}: {e}",output.display())))?;Ok(None)},
   Err(error)=>{if control.cancelled(){folder.rollback().map_err(|e|io_error(format!("PDF split cancelled. Incomplete export remains at {}: {e}",output.display())))?;Ok(None)}else{Err(folder.rollback_error(error))}}
  }
 }
}
