use super::*;
use crate::form_position::{bad,PositionFormFieldRequest};

impl PdfEngine {
    pub fn position_form_field_occurrence(&self,source_id:&str,occurrences:&[usize],selected:usize,request:PositionFormFieldRequest)->EngineResult<PageContentReplacement>{
        request.validate()?;
        if occurrences.is_empty()||occurrences.len()>100000||selected>=occurrences.len(){return Err(bad("invalid occurrence selection"));}
        let (reply,receive)=mpsc::channel();
        self.send(WorkerRequest::PositionFormField{source_id:source_id.into(),occurrences:occurrences.to_vec(),selected,request,reply})?;
        let mut result=receive.recv().map_err(|_|EngineError::WorkerStopped)??;
        result.document=self.document_in_points(Ok(result.document))?;Ok(result)
    }
}
fn same(a:&AnnotationRect,b:&AnnotationRect,tolerance:f32)->bool {[a.x-b.x,a.y-b.y,a.width-b.width,a.height-b.height].iter().all(|v|v.abs()<=tolerance)}
impl WorkerRuntime {
    pub(super) fn position_form_field_occurrence(&mut self,source_id:&str,occurrences:&[usize],selected:usize,request:&PositionFormFieldRequest)->EngineResult<PageContentReplacement>{
        request.validate()?;
        if occurrences.is_empty()||occurrences.len()>100000||selected>=occurrences.len(){return Err(bad("invalid occurrence selection"));}
        let source=self.editing_document(source_id)?;
        if source.policy.as_ref().is_some_and(|p|!p.protection.permissions().annotate){return Err(EngineError::PermissionDenied("positioning form fields"));}
        let count=source.document.page_count() as usize;
        if occurrences.iter().any(|p|*p>=count){return Err(bad("page outside source"));}
        let page_index=occurrences[selected];let unit=self.source_page_unit(source_id,page_index)?;
        // Bound metadata and reject original aliases/invalid APs before either
        // native inspection or duplication can normalize the original graph.
        crate::form_position::validate_source(source.bytes()?.as_ref(),page_index,request.annotation_index)?;
        let geometry=page_geometry(&source.pdf()?.pages().get(page_index_i32(page_index)?)?)?;
        let size=geometry.displayed_size();let r=&request.bounds;
        if r.x+r.width>size.width*unit+0.001||r.y+r.height>size.height*unit+0.001{return Err(bad("field must be inside the page"));}
        let original=crate::form_fill::NativeForm::open(self.bindings,source.bytes()?.as_ref())?.field_bounded(page_index,request.annotation_index,128*1024)?;
        if !original.visible||original.no_rotate||!matches!(original.kind.as_str(),"text"|"checkbox"|"radio"|"combo"|"list"){return Err(bad("this field does not support positioning"));}
        let raw=original.bounds.as_ref().ok_or_else(||bad("field has no rectangle"))?;
        let a=geometry.pdf_to_displayed(raw.x,raw.y+raw.height);let b=geometry.pdf_to_displayed(raw.x+raw.width,raw.y);
        let expected=AnnotationRect{x:a.x.min(b.x)*unit,y:a.y.min(b.y)*unit,width:(a.x-b.x).abs()*unit,height:(a.y-b.y).abs()*unit};
        if !same(&expected,&request.expected_bounds,0.01)||original.value!=crate::form_fill::normalized(&request.expected_value){return Err(bad("field changed; reopen positioning and try again"));}
        if request.reflow_text && ((request.bounds.width-request.expected_bounds.width).abs()>0.0001 || (request.bounds.height-request.expected_bounds.height).abs()>0.0001) {
            // Regeneration replaces the normal appearance. Prove its original
            // paint is ordinary field text/frame before changing the rectangle;
            // custom artwork remains available through Scale saved appearance.
            crate::form_reflow::validate_saved_paint(source.bytes()?.as_ref(),page_index,request.annotation_index)?;
        }
        let (x1,y1)=geometry.displayed_to_pdf(Point{x:r.x/unit,y:r.y/unit});let (x2,y2)=geometry.displayed_to_pdf(Point{x:(r.x+r.width)/unit,y:(r.y+r.height)/unit});
        let rect=AnnotationRect{x:x1.value.min(x2.value),y:y1.value.min(y2.value),width:(x2.value-x1.value).abs(),height:(y2.value-y1.value).abs()};
        let repeated=occurrences.iter().filter(|p|**p==page_index).count()>1;
        let policy=if repeated{source.policy.as_ref().map(|p|p.duplicated_page(page_index)).transpose()?}else{source.policy.clone()};
        let first=occurrences.iter().position(|p|*p==page_index).unwrap();
        let target=if repeated&&first!=selected{count}else{page_index};
        let page_indices=occurrences.iter().enumerate().map(|(i,p)|if repeated&&*p==page_index&&(i==selected)!=(first==selected){count}else{*p}).collect();
        let expanded;let original_bytes;let bytes:&[u8]=if repeated{expanded=self.append_page_copy(source,page_index)?;&expanded}else{original_bytes=source.bytes()?;&original_bytes};
        let before=crate::form_fill::NativeForm::open(self.bindings,bytes)?.field_bounded(target,request.annotation_index,128*1024)?;
        let bytes=crate::form_position::change(bytes,target,request.annotation_index,&rect)?;
        let bytes=if request.reflow_text && ((request.bounds.width-request.expected_bounds.width).abs()>0.0001 || (request.bounds.height-request.expected_bounds.height).abs()>0.0001) {
            crate::form_reflow::change(self.bindings,&bytes,target,request.annotation_index,&before.value)?
        } else {bytes};
        let mut after=crate::form_fill::NativeForm::open(self.bindings,&bytes)?.field_bounded(target,request.annotation_index,128*1024)?;
        if !after.bounds.as_ref().is_some_and(|b|same(b,&rect,0.01/unit)){return Err(bad("saved field geometry differs"));}
        after.bounds=before.bounds.clone();
        if serde_json::to_value(&after).map_err(bad)?!=serde_json::to_value(&before).map_err(bad)? {return Err(bad("positioning changed the field definition or value"));}
        let bookmarks=source.source_bookmarks()?.clone();
        let path=std::env::temp_dir().join(format!("folio-positioned-field-{}.pdf",Uuid::new_v4()));let original_path=source.original_path.clone();let legacy=source.legacy_comment_import;
        let mut document=self.open_bytes(bytes,path,original_path,true,true,legacy,OpenSecurity::Restored(policy))?;
        document = self.retain_annotation_sources_with_copy(source_id, document, repeated.then_some(page_index))?;
        document.name="Positioned form field.pdf".into();document.bookmarks=bookmarks.as_ref().cloned().unwrap_or_default();document.bookmark_error=bookmarks.as_ref().err().cloned();
        self.documents.get_mut(&document.id).unwrap().bookmarks=OnceLock::from(bookmarks);
        Ok(PageContentReplacement{document,page_indices})
    }
}
