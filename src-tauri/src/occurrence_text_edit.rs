//! Isolated text validation followed by complete-source publication.
use super::*;

fn invalid(message: &str) -> EngineError { EngineError::InvalidRequest(message.into()) }

impl PdfEngine {
    pub fn list_occurrence_text_runs(&self, source_id: &str, page_index: usize) -> EngineResult<TextRuns> {
        let (reply,receive)=mpsc::channel();
        self.send(WorkerRequest::ListOccurrenceTextRuns{source_id:source_id.into(),page_index,reply})?;
        self.in_points(source_id,page_index,receive.recv().map_err(|_|EngineError::WorkerStopped)?)
    }

    pub fn replace_text_occurrence(
        &self, source_id: &str, occurrences: &[usize], selected_occurrence: usize,
        object_index: usize, expected_text: &str, replacement: &str, font_id: Option<&str>,
    ) -> EngineResult<PageContentReplacement> {
        if occurrences.is_empty() || occurrences.len()>100_000 || selected_occurrence>=occurrences.len() {
            return Err(invalid("Invalid text page occurrence selection."));
        }
        let (reply,receive)=mpsc::channel();
        self.send(WorkerRequest::ReplaceTextOccurrence{
            source_id:source_id.into(),occurrences:occurrences.to_vec(),selected_occurrence,
            object_index,expected_text:expected_text.into(),replacement:replacement.into(),
            font_id:font_id.map(str::to_owned),reply,
        })?;
        let mut result=receive.recv().map_err(|_|EngineError::WorkerStopped)??;
        result.document=self.document_in_points(Ok(result.document))?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary,Document,Stream,Object};

    fn text_source(runtime:&mut WorkerRuntime, guarded:bool)->DocumentInfo {
        let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
        let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
        let content=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 18 Tf 30 200 Td (Old content) Tj ET".to_vec()));
        let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
        if guarded {pdf.get_dictionary_mut(page).unwrap().set("PieceInfo",dictionary!{});}
        pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into());
        let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);
        let bytes=crate::pdf_output::save(&mut pdf).unwrap();
        runtime.register_generated_document(bytes,"Text.pdf").unwrap()
    }

    #[test]
    fn exact_publication_proof_rejects_different_text() {
        let root=PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut runtime=WorkerRuntime::new(&root.join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap();
        let source=text_source(&mut runtime,false);
        let edited=runtime.replace_text_occurrence(&source.id,&[0],0,0,"Old content","New content",None).unwrap();
        runtime.verify_text_publication(&edited.document.id,0,&edited.document.id).unwrap();
        assert!(runtime.verify_text_publication(&edited.document.id,0,&source.id).is_err());
    }

    #[test]
    fn transaction_discards_private_candidates_on_publication_failure_and_abandoned_reply() {
        let root=PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut runtime=WorkerRuntime::new(&root.join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap();
        let source=text_source(&mut runtime,false);let guarded=text_source(&mut runtime,true);
        let before=runtime.documents.len();
        let error=runtime.replace_text_occurrence(&guarded.id,&[0],0,0,"Old content","New content",None).unwrap_err();
        assert!(error.to_string().contains("document-linked"),"{error}");
        assert_eq!(runtime.documents.len(),before);
        let result=runtime.replace_text_occurrence(&source.id,&[0],0,0,"Old content","New content",None).unwrap();
        assert_eq!(runtime.documents.len(),before+1);
        runtime.discard_unpublished_source(&result.document.id);
        let (reply,receive)=mpsc::channel();drop(receive);
        let (send,requests)=mpsc::channel();
        send.send(WorkerRequest::ReplaceTextOccurrence{source_id:source.id.clone(),occurrences:vec![0],selected_occurrence:0,object_index:0,expected_text:"Old content".into(),replacement:"New content".into(),font_id:None,reply}).unwrap();drop(send);
        runtime.run(requests);
        assert_eq!(runtime.documents.len(),before);
    }

    #[test]
    fn paragraph_projection_and_publication_cleanup_on_failure_and_abandoned_reply() {
        use occurrence_paragraph::Request;
        let root=PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut runtime=WorkerRuntime::new(&root.join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap();
        let source=text_source(&mut runtime,false);let guarded=text_source(&mut runtime,true);
        let before=runtime.documents.len();
        let area=AnnotationRect{x:30.,y:180.,width:200.,height:90.};
        for styled in [false,true] {for case in ["stale","publication","abandoned","success"] {
            let (reply,receive)=mpsc::channel();
            let receive=if case=="abandoned"{drop(receive);None}else{Some(receive)};
            let source=if case=="publication"{guarded.id.clone()}else{source.id.clone()};
            let expected=if case=="stale"{"stale"}else{"Old content"};
            let request=if styled {
                let request=serde_json::from_value(serde_json::json!({"objectIndices":[0],"expectedText":expected,"runs":[{"text":"New paragraph","styleId":0}],"bounds":area,"alignment":"left","lineSpacing":1.2})).unwrap();
                Request::PreviewStyled{source,occurrences:vec![0],selected:0,request,reply}
            }else{Request::PreviewText{source,occurrences:vec![0],selected:0,indices:vec![0],expected:expected.into(),replacement:"New paragraph".into(),bounds:area.clone(),formatting:ParagraphFormatting::default(),reply}};
            runtime.handle_occurrence_paragraph(request);
            if let Some(receive)=receive {
                let result=receive.recv().unwrap();
                if case=="success" {let result=result.unwrap();assert_eq!(runtime.documents.len(),before+1);runtime.discard_unpublished_source(&result.document.id);}
                else {let error=result.unwrap_err();assert!(error.to_string().contains(if case=="stale"{"changed"}else{"document-linked"}),"{error}");}
            }
            assert_eq!(runtime.documents.len(),before,"{styled} {case}");
        }}
        let (reply,receive)=mpsc::channel();
        runtime.handle_occurrence_paragraph(Request::InspectStyled{source:source.id, page:0,indices:vec![999],reply});
        assert!(receive.recv().unwrap().is_err());assert_eq!(runtime.documents.len(),before);
    }
}

impl WorkerRuntime {
    pub(super) fn replace_text_occurrence(
        &mut self, source_id: &str, occurrences: &[usize], selected_occurrence: usize,
        object_index: usize, expected_text: &str, replacement: &str, font_id: Option<&str>,
    ) -> EngineResult<PageContentReplacement> {
        if occurrences.is_empty() || occurrences.len()>100_000 || selected_occurrence>=occurrences.len() {
            return Err(invalid("Invalid text page occurrence selection."));
        }
        let _phase=crate::native_phase!("replacement.total");
        let _reuse=crate::request_parse::Scope::enter();
        let context=self.single_font_text_context(source_id,occurrences[selected_occurrence],object_index,expected_text,replacement,font_id)?;
        self.publish_context_text_occurrence(source_id,occurrences,selected_occurrence,context.as_ref(),|runtime,page| {
            runtime.replace_transaction_text(source_id,page,object_index,expected_text,replacement,font_id,true)
        })
    }

    pub(super) fn publish_text_occurrence(
        &mut self,source_id:&str,occurrences:&[usize],selected_occurrence:usize,
        create:impl FnOnce(&mut Self,usize)->EngineResult<DocumentInfo>,
    )->EngineResult<PageContentReplacement> {
        self.publish_context_text_occurrence(source_id,occurrences,selected_occurrence,None,create)
    }

    pub(super) fn publish_context_text_occurrence(
        &mut self,source_id:&str,occurrences:&[usize],selected_occurrence:usize,
        context:Option<&text_edit::PublicationProof>,
        create:impl FnOnce(&mut Self,usize)->EngineResult<DocumentInfo>,
    )->EngineResult<PageContentReplacement> {
        if occurrences.is_empty() || occurrences.len()>100_000 || selected_occurrence>=occurrences.len() {
            return Err(invalid("Invalid text page occurrence selection."));
        }
        let source=self.editing_document(source_id)?;
        let count=source.document.page_count() as usize;
        if occurrences.iter().any(|p|*p>=count) {return Err(invalid("Text page occurrence is outside its source."));}
        let selected_page=occurrences[selected_occurrence];
        let duplicate=occurrences.iter().filter(|p|**p==selected_page).count()>1;
        if duplicate && context.is_some_and(|proof|proof.tagged()) {
            return Err(invalid("Duplicating tagged text pages is not supported yet."));
        }
        if duplicate && count>=100_000 {return Err(invalid("Native page count exceeds text-edit limits."));}
        let policy=if duplicate {source.policy.as_ref().map(|p|p.duplicated_page(selected_page)).transpose()?}else{source.policy.clone()};
        let first=occurrences.iter().position(|p|*p==selected_page).unwrap();
        let replace_index=if duplicate && first!=selected_occurrence {count}else{selected_page};
        let page_indices=occurrences.iter().enumerate().map(|(i,p)| {
            if !duplicate || *p!=selected_page || (i==selected_occurrence)==(first==selected_occurrence) {*p}else{count}
        }).collect();
        let target_geometry=page_geometry(&source.pdf()?.pages().get(page_index_i32(selected_page)?)?)?;
        // All private mutation/font/collision/raster checks remain centralized in
        // the existing editor. No isolated result escapes this worker request.
        let candidate={let _phase=crate::native_phase!("replacement.isolated");create(self,selected_page)?};
        let result=(|| {
            let source=self.editing_document(source_id)?;
            let incoming=self.editing_document(&candidate.id)?;
            let source_geometry=page_geometry(&incoming.pdf()?.pages().get(0)?)?;
            if let Some(context)=context {
                let page=incoming.pdf()?.pages().get(0)?;
                context.verify_candidate(incoming.bytes()?.as_ref(),&page)?;
            }
            let expanded=if duplicate {self.append_page_copy(source,selected_page)?.into()}else{source.bytes()?};
            let publication={let _phase=crate::native_phase!("publication.graph");if let Some(context)=context {
                page_content_replace::replace_context_text(&expanded,replace_index,incoming.bytes()?.as_ref(),target_geometry,source_geometry,context)?
            } else {
                page_content_replace::replace_edited_text(&expanded,replace_index,incoming.bytes()?.as_ref(),target_geometry,source_geometry)?
            }};
            let bookmarks=source.source_bookmarks()?.clone();
            let original_path=source.original_path.clone();
            let legacy=source.legacy_comment_import;
            let path=std::env::temp_dir().join(format!("folio-text-occurrence-{}.pdf",Uuid::new_v4()));
            let mut document=self.open_text_publication(publication,path,original_path,legacy,OpenSecurity::Restored(policy))?;
            if let Err(error)=self.verify_text_publication(&document.id,replace_index,&candidate.id) {
                self.discard_unpublished_source(&document.id);
                return Err(error);
            }
            document={let _phase=crate::native_phase!("publication.annotation_retention");self.retain_annotation_sources_with_copy(source_id,document,duplicate.then_some(selected_page))?};
            document.name="Edited text.pdf".into();
            document.bookmarks=bookmarks.as_ref().cloned().unwrap_or_default();
            document.bookmark_error=bookmarks.as_ref().err().cloned();
            self.documents.get_mut(&document.id).unwrap().bookmarks=OnceLock::from(bookmarks);
            Ok(PageContentReplacement{document,page_indices})
        })();
        self.discard_unpublished_source(&candidate.id);
        result
    }
}
