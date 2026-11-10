use super::*;
use crate::{text_export::{self as export,TextExportRequest,TextExportResult,TextExportControl,TextExportProgress},FormFieldValue};
pub(super) type Progress=Arc<dyn Fn(TextExportProgress)+Send+Sync>;
impl PdfEngine{
 pub fn export_text(&self,mut request:TextExportRequest,destination:impl AsRef<Path>,control:Arc<TextExportControl>,progress:Progress)->EngineResult<Option<TextExportResult>>{
  request.validate()?;if control.cancelled(){return Ok(None);}
  let resources=self.capture_export_resources(&request.pages)?;
  request.pages=self.export_in_raw(ExportRequest{pages:request.pages,flatten:true})?.pages;
  let(reply,receive)=mpsc::channel();self.send(WorkerRequest::ExportText{request,destination:destination.as_ref().to_path_buf(),control,progress,resources,reply})?;
  receive.recv().map_err(|_|EngineError::WorkerStopped)?
 }
}
impl WorkerRuntime{
 pub(super) fn export_text(&mut self,request:TextExportRequest,destination:PathBuf,control:Arc<TextExportControl>,progress:Progress,resources:page_image_render::Resources)->EngineResult<Option<TextExportResult>>{
  let previous=std::mem::replace(&mut self.fonts,resources.fonts.clone());
  let result=self.export_text_owned(request,destination,&control,&progress);self.fonts=previous;result
 }
 fn export_text_owned(&mut self,request:TextExportRequest,destination:PathBuf,control:&Arc<TextExportControl>,progress:&Progress)->EngineResult<Option<TextExportResult>>{
  request.validate()?;let total=request.pages.len();let report=|completed,stage|progress(TextExportProgress{completed,total,stage});
  report(0,"validating");if control.cancelled(){return Ok(None);}
  let destination=self.validated_export_destination(&destination,"txt")?;
  let batch=ExportRequest{pages:request.pages.clone(),flatten:true};self.validate_export(&batch)?;self.authorize_composition(&batch,&[],CompositionIntent::TextExport)?;
  let mut output=vec![0xef,0xbb,0xbf];let mut empty_pages=0;let mut skipped_fields=0;
  for(index,plan)in request.pages.into_iter().enumerate(){
   report(index,"extracting");if control.cancelled(){return Ok(None);}
   let cancel=control.clone();
   let view=self.prepare_read_page(plan,CompositionIntent::TextExport,Arc::new(move||cancel.cancelled()));
   if control.cancelled(){return Ok(None);}
   let view=view?;let bytes=view.bytes;let selected=view.page_index;
   if control.cancelled(){return Ok(None);}
   let document=self.pdfium.load_pdf_from_reader(Cursor::new(bytes.as_ref()),None)?;
   let page=document.pages().get(page_index_i32(selected)?)?;let geometry=page_geometry(&page)?;let size=geometry.displayed_size();
   #[cfg(feature="shaped-text")]
   let logical=self.read_ocr_composed_text(&bytes,&document,selected)?;
   #[cfg(not(feature="shaped-text"))]
   let logical:Option<PageText>=None;
   let text=match logical{Some(text)=>text,None=>self.read_document_page_text(&document,selected,Some(export::MAX_CHARACTERS))?};
   let mut text=export::page_text(&text,size.width,size.height)?;
   if request.include_form_values{
    let fields=crate::form_fill::NativeForm::open(self.bindings,&bytes)?.fields_filtered(selected,8*1024*1024,|field|{
     if !field.visible{return false;}
     let Some(raw)=&field.bounds else{return false};let anchor=geometry.pdf_to_displayed(raw.x,raw.y+raw.height);let opposite=geometry.pdf_to_displayed(raw.x+raw.width,raw.y);
     let(x,y,w,h)=if field.no_rotate{(anchor.x,anchor.y,raw.width,raw.height)}else{(anchor.x.min(opposite.x),anchor.y.min(opposite.y),(opposite.x-anchor.x).abs(),(opposite.y-anchor.y).abs())};
     if !export::intersects(x,y,w,h,size.width,size.height){return false;}
     if field.password||!field.supported{skipped_fields+=1;return false;}true
    })?;
    let mut lines=Vec::new();let mut form_bytes=0usize;
    for field in fields{
     if !field.visible{continue;}
     let Some(raw)=field.bounds else{continue};let anchor=geometry.pdf_to_displayed(raw.x,raw.y+raw.height);let opposite=geometry.pdf_to_displayed(raw.x+raw.width,raw.y);
     let (x,y,w,h)=if field.no_rotate{(anchor.x,anchor.y,raw.width,raw.height)}else{(anchor.x.min(opposite.x),anchor.y.min(opposite.y),(opposite.x-anchor.x).abs(),(opposite.y-anchor.y).abs())};
     if !export::intersects(x,y,w,h,size.width,size.height){continue;}
     if field.password||!matches!(field.kind.as_str(),"text"|"checkbox"|"radio"|"combo"|"list"){skipped_fields+=1;continue;}
     let value=match &field.value{
      FormFieldValue::Text{text}=>if field.editable_choice&&!field.selected_indices.is_empty(){field.selected_indices.iter().filter_map(|i|field.options.get(*i)).cloned().collect::<Vec<_>>().join(", ")}else{text.clone()},
      FormFieldValue::Toggle{checked}=>if field.kind=="radio"{if !checked{continue;}field.options.first().cloned().unwrap_or_else(||"Selected".into())}else{if *checked{"Checked"}else{"Unchecked"}.into()},
      FormFieldValue::Choice{indices}=>indices.iter().filter_map(|i|field.options.get(*i)).cloned().collect::<Vec<_>>().join(", "),
     };
     let label=if !field.label.is_empty(){&field.label}else if !field.name.is_empty(){&field.name}else{"Unnamed field"};
     let line=format!("{label}: {}",value.replace("\r\n","\n").replace('\r',"\n"));
     form_bytes=form_bytes.checked_add(line.len()+1).ok_or_else(||export::bad("Form values exceed the output limit."))?;
     if form_bytes>export::MAX_BYTES{return Err(export::bad("Form values exceed the output limit."));}lines.push(line);
    }
    if !lines.is_empty(){if !text.is_empty(){text.push_str("\n\n");}text.push_str("[Form fields]\n");text.push_str(&lines.join("\n"));}
   }
   if text.trim().is_empty(){empty_pages+=1;}
   if index>0{export::append(&mut output,"\n\u{c}\n")?;}
   export::append(&mut output,&text)?;report(index+1,"extracting");if control.cancelled(){return Ok(None);}
  }
  report(total,"writing");if !control.commit(){return Ok(None);}
  export::publish(&destination,&output)?;control.completed();report(total,"completed");
  Ok(Some(TextExportResult{path:destination,page_count:total,empty_pages,skipped_fields,bytes:output.len()}))
 }
}
