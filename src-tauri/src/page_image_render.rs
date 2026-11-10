use super::*;
use crate::page_image_export::{self as export,PageImageExportRequest,PageImageExportResult,PageImageControl,PageImageProgress};
use crate::image_export_folder::OwnedImageFolder;
pub(super) struct Resources{pub(super) fonts:Arc<FontRegistry>,_images:ImageSourceLeases}
pub(super) type Progress=Arc<dyn Fn(PageImageProgress)+Send+Sync>;
impl PdfEngine{
 pub(super) fn capture_export_resources(&self,pages:&[PagePlan])->EngineResult<Resources>{
  let fonts=Arc::new(FontRegistry::new());let mut assets=Vec::new();let mut font_ids=HashSet::new();let mut image_ids=HashSet::new();
  let mut leases=ImageSourceLeases{registry:self.inner.images.clone(),registrations:vec![]};
  for page in pages{for overlay in &page.overlays{match overlay{
   Overlay::Text(text)=>{if let Some(id)=&text.font_id{if font_ids.insert(id.clone()){assets.push(self.inner.fonts.get(id)?);}}},
   Overlay::Image(image)=>{if image_ids.insert(image.asset_id.clone()){leases.registrations.push(self.inner.images.acquire(&image.asset_id)?);}},_=>{}
  }}}
  fonts.register_assets(&assets)?;
  Ok(Resources{fonts,_images:leases})
 }
 pub fn export_page_images(&self,mut request:PageImageExportRequest,parent:impl AsRef<Path>,control:Arc<PageImageControl>,progress:Progress)->EngineResult<Option<PageImageExportResult>>{
  request.validate()?;if control.cancelled(){return Ok(None);}
  let resources=self.capture_export_resources(&request.pages)?;
  request.pages=self.export_in_raw(ExportRequest{pages:request.pages,flatten:false})?.pages;
  let (reply,receive)=mpsc::channel();self.send(WorkerRequest::ExportPageImages{request,parent:parent.as_ref().to_path_buf(),control,progress,resources,reply})?;
  receive.recv().map_err(|_|EngineError::WorkerStopped)?
 }
}
impl WorkerRuntime{
 pub(super) fn export_page_images(&mut self,request:PageImageExportRequest,parent:PathBuf,control:Arc<PageImageControl>,progress:Progress,resources:Resources)->EngineResult<Option<PageImageExportResult>>{
  // This job is one serial request. Composition consumes the snapshot even if UI font leases disappear.
  let previous=std::mem::replace(&mut self.fonts,resources.fonts.clone());
  let result=self.export_page_images_owned(request,parent,&control,&progress);self.fonts=previous;result
 }
 fn export_page_images_owned(&mut self,request:PageImageExportRequest,parent:PathBuf,control:&Arc<PageImageControl>,progress:&Progress)->EngineResult<Option<PageImageExportResult>>{
  request.validate()?;let total=request.pages.len();let report=|completed,stage|progress(PageImageProgress{completed,total,stage});
  report(0,"validating");if control.cancelled(){return Ok(None);}
  let batch=ExportRequest{pages:request.pages.clone(),flatten:false};self.validate_export(&batch)?;self.authorize_composition(&batch,&[],CompositionIntent::ImageExport)?;
  let mut physical=Vec::with_capacity(total);
  for plan in &request.pages{
   if control.cancelled(){return Ok(None);}
   let source=self.document(&plan.source_id)?;let page=source.pdf()?.pages().get(page_index_i32(plan.page_index)?)?;
   let size=page_geometry(&page)?.displayed_size();let unit=source.units()?[plan.page_index];
   let (mut w,mut h)=plan.crop.as_ref().map(|c|((c.x+c.width).min(size.width)-c.x,(c.y+c.height).min(size.height)-c.y)).unwrap_or((size.width,size.height));
   if plan.rotation%180!=0{std::mem::swap(&mut w,&mut h);}
   let dims=(f64::from(w)*f64::from(unit),f64::from(h)*f64::from(unit));request.options.dimensions(dims.0,dims.1)?;physical.push(dims);
  }
  if control.cancelled(){return Ok(None);}
  let mut folder=OwnedImageFolder::create(&parent)?;
  let outcome=(||->EngineResult<Option<PageImageExportResult>>{
   let mut encoded_bytes=0usize;
   for (index,plan) in request.pages.into_iter().enumerate(){
    report(index,"rendering");if control.cancelled(){return Ok(None);}
    let rgb={
     let cancel=control.clone();
     let view=self.prepare_read_page(plan,CompositionIntent::ImageExport,Arc::new(move||cancel.cancelled()));
     if control.cancelled(){return Ok(None);}
     let view=view?;let unit=view.unit;let document=self.pdfium.load_pdf_from_reader(Cursor::new(view.bytes),None)?;let page=document.pages().get(page_index_i32(view.page_index)?)?;
     let w=f64::from(page.width().value)*f64::from(unit);let h=f64::from(page.height().value)*f64::from(unit);
     if (w-physical[index].0).abs()>0.001||(h-physical[index].1).abs()>0.001{return Err(export::bad("Composed page geometry changed during image export."));}
     let (width,height)=request.options.dimensions(w,h)?;
     let bitmap=page.render_with_config(&PdfRenderConfig::new().set_target_size(width as i32,height as i32).set_clear_color(PdfColor::WHITE).render_annotations(true).render_form_data(true).use_print_quality(false))?;
     if bitmap.width()!=width as i32||bitmap.height()!=height as i32{return Err(export::bad("Renderer did not produce the requested image dimensions."));}
     bitmap.as_image()?.into_rgb8()
    };
    report(index,"encoding");if control.cancelled(){return Ok(None);}
    let encoded=export::encode(&rgb,request.options,export::BATCH_BYTES.saturating_sub(encoded_bytes),control)?;drop(rgb);
    report(index,"writing");if control.cancelled(){return Ok(None);}
    let name=format!("page-{:04}.{}",request.page_numbers[index],request.options.format.extension());folder.write(&name,&encoded)?;encoded_bytes+=encoded.len();drop(encoded);
    report(index+1,"writing");if control.cancelled(){return Ok(None);}
   }
   if !control.commit(){return Ok(None);}
   let path=folder.finish()?;control.completed();report(total,"completed");Ok(Some(PageImageExportResult{folder:path,count:total}))
  })();
  let output_path=folder.path().to_path_buf();
  match outcome{
   Ok(Some(result))=>Ok(Some(result)),
   Ok(None)=>{folder.rollback().map_err(|error|export::io_error(format!("Image export cancelled. Incomplete export remains at {}: {error}",output_path.display())))?;Ok(None)},
   Err(error)=>{if control.cancelled(){folder.rollback().map_err(|cleanup|export::io_error(format!("Image export cancelled. Incomplete export remains at {}: {cleanup}",output_path.display())))?;Ok(None)}else{Err(folder.rollback_error(error))}}
  }
 }
}
