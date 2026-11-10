use super::*;
use crate::form_author::{bad, CreateFormFieldRequest, NewFormField};

impl PdfEngine {
    pub fn create_form_field_occurrence(&self, source_id:&str, occurrences:&[usize], selected:usize, request:CreateFormFieldRequest)->EngineResult<PageContentReplacement> {
        request.validate()?;
        if occurrences.is_empty() || occurrences.len()>100000 || selected>=occurrences.len() {return Err(bad("invalid occurrence selection"));}
        let (reply,receive)=mpsc::channel();
        self.send(WorkerRequest::CreateFormField{source_id:source_id.into(),occurrences:occurrences.to_vec(),selected,request,reply})?;
        let mut result=receive.recv().map_err(|_|EngineError::WorkerStopped)??;
        result.document=self.document_in_points(Ok(result.document))?;
        Ok(result)
    }
}
impl WorkerRuntime {
    pub(super) fn create_form_field_occurrence(&mut self,source_id:&str,occurrences:&[usize],selected:usize,request:&CreateFormFieldRequest)->EngineResult<PageContentReplacement>{
        request.validate()?;
        if occurrences.is_empty() || occurrences.len()>100000 || selected>=occurrences.len() {return Err(bad("invalid occurrence selection"));}
        let source=self.editing_document(source_id)?;
        if source.policy.as_ref().is_some_and(|p|!p.protection.permissions().annotate) {return Err(EngineError::PermissionDenied("creating form fields"));}
        let count=source.document.page_count() as usize;
        if occurrences.iter().any(|i|*i>=count) {return Err(bad("page outside source"));}
        let page_index=occurrences[selected];
        let unit=self.source_page_unit(source_id,page_index)?;
        let geometry=page_geometry(&source.pdf()?.pages().get(page_index_i32(page_index)?)?)?;
        let size=geometry.displayed_size();let r=&request.bounds;
        if r.x+r.width>size.width*unit+0.001 || r.y+r.height>size.height*unit+0.001 {return Err(bad("field must be inside the page"));}
        if let NewFormField::Radio{options}=&request.field {
            let width=if request.rotation==90||request.rotation==270 {r.height}else{r.width};
            for label in options {
                if persistence::text_advance(label,TextFont::Helvetica)?*request.font_size+20.>width+0.001 {return Err(bad("make the radio group wider or reduce the font size so every label fits"));}
            }
        }
        let (x1,y1)=geometry.displayed_to_pdf(Point{x:r.x/unit,y:r.y/unit});
        let (x2,y2)=geometry.displayed_to_pdf(Point{x:(r.x+r.width)/unit,y:(r.y+r.height)/unit});
        let rect=AnnotationRect{x:x1.value.min(x2.value),y:y1.value.min(y2.value),width:(x2.value-x1.value).abs(),height:(y2.value-y1.value).abs()};
        let repeated=occurrences.iter().filter(|i|**i==page_index).count()>1;
        let policy=if repeated {source.policy.as_ref().map(|p|p.duplicated_page(page_index)).transpose()?}else{source.policy.clone()};
        let first=occurrences.iter().position(|i|*i==page_index).unwrap();
        let target_page=if repeated && first!=selected {count}else{page_index};
        let page_indices=occurrences.iter().enumerate().map(|(i,p)|if repeated && *p==page_index && (i==selected)!=(first==selected) {count}else{*p}).collect();
        let expanded;let original_bytes;
        let bytes:&[u8]=if repeated {expanded=self.append_page_copy(source,page_index)?;&expanded}else{original_bytes=source.bytes()?;&original_bytes};
        let created=crate::form_author::create(bytes,target_page,request,&rect,unit,(geometry.rotation+request.rotation)%360)?;
        let bytes=created.bytes;
        let native=crate::form_fill::NativeForm::open(self.bindings,&bytes)?;
        let empty=match &request.field {NewFormField::Text{..}=>crate::FormFieldValue::Text{text:String::new()},NewFormField::Checkbox|NewFormField::Radio{..}=>crate::FormFieldValue::Toggle{checked:false},_=>crate::FormFieldValue::Choice{indices:vec![]}};
        for widget in &created.widgets {
        let f=native.field_bounded(target_page,widget.annotation_index,128*1024)?;
        let rect=&widget.bounds;
        let bounds_ok=f.bounds.as_ref().is_some_and(|b|[b.x-rect.x,b.y-rect.y,b.width-rect.width,b.height-rect.height].iter().all(|n|n.abs()*unit<0.01));
        if f.name!=request.name || f.kind!=request.kind() || f.required!=request.required || f.value!=empty || !bounds_ok || f.read_only || f.reason.is_some()
            || f.multiline!=matches!(request.field,NewFormField::Text{multiline:true})
            || f.multi_select!=matches!(request.field,NewFormField::List{multi_select:true,..}) {
            return Err(bad("created field failed native validation"));
        }
        if let NewFormField::Dropdown{options}|NewFormField::List{options,..}=&request.field {
            if &f.options!=options {return Err(bad("choice labels changed during creation"));}
        }
        if let NewFormField::Radio{options}=&request.field {
            let option=widget.option_index.and_then(|i|options.get(i)).ok_or_else(||bad("missing radio option identity"))?;
            if f.options!=vec![option.clone()] {return Err(bad("radio export label changed during creation"));}
        }
        }
        drop(native);
        let bookmarks=source.source_bookmarks()?.clone();
        let path=std::env::temp_dir().join(format!("folio-created-field-{}.pdf",Uuid::new_v4()));
        let original_path=source.original_path.clone();let legacy=source.legacy_comment_import;
        let mut document=self.open_bytes(bytes,path,original_path,true,true,legacy,OpenSecurity::Restored(policy))?;
        document = self.retain_annotation_sources_with_copy(source_id, document, repeated.then_some(page_index))?;
        document.name="Created form field.pdf".into();
        document.bookmarks=bookmarks.as_ref().cloned().unwrap_or_default();document.bookmark_error=bookmarks.as_ref().err().cloned();
        self.documents.get_mut(&document.id).unwrap().bookmarks=OnceLock::from(bookmarks);
        Ok(PageContentReplacement{document,page_indices})
    }
}
