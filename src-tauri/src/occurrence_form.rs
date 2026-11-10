//! Source-authorized reusable form edits, with complete-document publication.
use super::*;

pub(super) enum Request {
    List{source:String,page:usize,reply:mpsc::Sender<EngineResult<FormTextRuns>>},
    Replace{source:String,occurrences:Vec<usize>,selected:usize,path:Vec<usize>,expected:String,replacement:String,font:Option<String>,reply:mpsc::Sender<EngineResult<PageContentReplacement>>},
}

impl PdfEngine {
    pub fn list_occurrence_form_text_runs(&self,source_id:&str,page_index:usize)->EngineResult<FormTextRuns>{
        let (reply,receive)=mpsc::channel();
        self.send(WorkerRequest::OccurrenceForm(Request::List{source:source_id.into(),page:page_index,reply}))?;
        self.in_points(source_id,page_index,receive.recv().map_err(|_|EngineError::WorkerStopped)?)
    }
    pub fn replace_form_text_occurrence(&self,source_id:&str,occurrences:&[usize],selected:usize,path:&[usize],expected:&str,replacement:&str,font_id:Option<&str>)->EngineResult<PageContentReplacement>{
        if occurrences.is_empty()||occurrences.len()>100_000||selected>=occurrences.len(){return Err(EngineError::InvalidRequest("Invalid form page occurrence selection.".into()));}
        let (reply,receive)=mpsc::channel();
        self.send(WorkerRequest::OccurrenceForm(Request::Replace{source:source_id.into(),occurrences:occurrences.to_vec(),selected,path:path.to_vec(),expected:expected.into(),replacement:replacement.into(),font:font_id.map(str::to_owned),reply}))?;
        let mut result=receive.recv().map_err(|_|EngineError::WorkerStopped)??;
        result.document=self.document_in_points(Ok(result.document))?;Ok(result)
    }
}

impl WorkerRuntime {
    pub(super) fn handle_occurrence_form(&mut self,request:Request){
        match request {
            Request::List{source,page,reply}=>{
                let result=self.form_source_admission(&source,true).and_then(|()| {
                    if !self.text_page_has_form_objects(&source,page)? {
                        return Ok(FormTextRuns{runs:vec![],reason:None});
                    }
                    self.with_text_projection(&source,page,|rt,id|rt.list_transaction_form_text_runs(id,0,true))
                });
                let result=match result {Err(EngineError::InvalidRequest(reason))=>Ok(FormTextRuns{runs:vec![],reason:Some(reason)}),other=>other};
                let _=reply.send(result);
            }
            Request::Replace{source,occurrences,selected,path,expected,replacement,font,reply}=>{
                let result=self.form_source_admission(&source,true).and_then(|()|
                    self.publish_text_occurrence(&source,&occurrences,selected,|rt,page|
                        rt.with_text_projection(&source,page,|rt,id|rt.replace_transaction_form_text(id,0,&path,&expected,&replacement,font.as_deref(),true))));
                if let Err(mpsc::SendError(Ok(result)))=reply.send(result){self.discard_unpublished_source(&result.document.id);}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary,Document,Stream,Object};
    fn source(rt:&mut WorkerRuntime,guarded:bool)->DocumentInfo{
        let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
        let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
        let leaf=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),200.into(),40.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}},b"BT /F1 16 Tf 5 10 Td (Old) Tj ET".to_vec()));
        let content=pdf.add_object(Stream::new(dictionary!{},b"q 1 0 0 1 20 200 cm /F Do Q".to_vec()));
        let note=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![300.into(),300.into(),318.into(),318.into()],"Contents"=>Object::string_literal("Keep")});
        let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),400.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"F"=>leaf}},"Contents"=>content,"Annots"=>vec![Object::Reference(note)]});
        if guarded{pdf.get_dictionary_mut(page).unwrap().set("PieceInfo",dictionary!{});}
        pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into());
        let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);
        rt.register_generated_document(crate::pdf_output::save(&mut pdf).unwrap(),"Form.pdf").unwrap()
    }
    #[test]
    fn occurrence_form_releases_candidates_on_error_success_and_abandoned_reply(){
        let root=PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut rt=WorkerRuntime::new(&root.join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap();
        let source=source(&mut rt,false);let guarded=tests::source(&mut rt,true);let before=rt.documents.len();
        for case in ["stale","publication","font","abandoned","success","noop"]{
            let (reply,receive)=mpsc::channel();let receive=if case=="abandoned"{drop(receive);None}else{Some(receive)};
            rt.handle_occurrence_form(Request::Replace{source:if case=="publication"{guarded.id.clone()}else{source.id.clone()},occurrences:vec![0],selected:0,path:vec![0,0],expected:if case=="stale"{"stale"}else{"Old"}.into(),replacement:if case=="noop"{"Old"}else{"New"}.into(),font:(case=="font").then(||"missing".into()),reply});
            if let Some(receive)=receive{let result=receive.recv().unwrap();if matches!(case,"success"|"noop"){let result=result.unwrap();assert_eq!(rt.documents.len(),before+1);rt.discard_unpublished_source(&result.document.id);}else{assert!(result.is_err(),"{case}");}}
            assert_eq!(rt.documents.len(),before,"{case}");
        }
        let (reply,receive)=mpsc::channel();rt.handle_occurrence_form(Request::List{source:source.id,page:99,reply});
        let result=receive.recv().unwrap();assert!(result.is_err()||result.unwrap().reason.is_some());assert_eq!(rt.documents.len(),before);
    }
}
