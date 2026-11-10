//! Existing group proofs on a private page, followed by complete-source publication.
use super::*;

pub(super) enum Request {
    Inspect{source:String,page:usize,indices:Vec<usize>,reply:mpsc::Sender<EngineResult<TextGroupPreview>>},
    Replace{source:String,occurrences:Vec<usize>,selected:usize,indices:Vec<usize>,expected:String,replacement:String,reply:mpsc::Sender<EngineResult<PageContentReplacement>>},
}

impl PdfEngine {
    pub fn inspect_occurrence_text_group(&self,source_id:&str,page_index:usize,indices:&[usize])->EngineResult<TextGroupPreview>{
        let (reply,receive)=mpsc::channel();
        self.send(WorkerRequest::OccurrenceGroup(Request::Inspect{source:source_id.into(),page:page_index,indices:indices.to_vec(),reply}))?;
        self.in_points(source_id,page_index,receive.recv().map_err(|_|EngineError::WorkerStopped)?)
    }
    pub fn replace_text_group_occurrence(&self,source_id:&str,occurrences:&[usize],selected:usize,indices:&[usize],expected:&str,replacement:&str)->EngineResult<PageContentReplacement>{
        if occurrences.is_empty()||occurrences.len()>100_000||selected>=occurrences.len(){return Err(EngineError::InvalidRequest("Invalid group page occurrence selection.".into()));}
        let (reply,receive)=mpsc::channel();
        self.send(WorkerRequest::OccurrenceGroup(Request::Replace{source:source_id.into(),occurrences:occurrences.to_vec(),selected,indices:indices.to_vec(),expected:expected.into(),replacement:replacement.into(),reply}))?;
        let mut result=receive.recv().map_err(|_|EngineError::WorkerStopped)??;
        result.document=self.document_in_points(Ok(result.document))?;Ok(result)
    }
}

impl WorkerRuntime {
    pub(super) fn handle_occurrence_group(&mut self,request:Request){
        match request {
            Request::Inspect{source,page,indices,reply}=>{
                // Check the complete source first: projection may omit catalog tags.
                let result=self.group_source_admission(&source).and_then(|()|
                    self.with_text_projection(&source,page,|rt,id|rt.inspect_text_group(id,0,&indices)));
                let _=reply.send(result);
            }
            Request::Replace{source,occurrences,selected,indices,expected,replacement,reply}=>{
                let result=self.group_source_admission(&source).and_then(|()|
                    self.publish_text_occurrence(&source,&occurrences,selected,|rt,page|
                        rt.with_text_projection(&source,page,|rt,id|rt.replace_text_group(id,0,&indices,&expected,&replacement))));
                if let Err(mpsc::SendError(Ok(result)))=reply.send(result){self.discard_unpublished_source(&result.document.id);}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary,Document,Stream,Object};
    #[test]
    fn group_transaction_releases_private_documents_and_abandoned_results(){
        let root=PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut rt=WorkerRuntime::new(&root.join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap();
        let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
        let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
        let stream=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 12 Tf 50 240 Td (Old) Tj ( words) Tj ET".to_vec()));
        let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>stream});
        pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into());
        let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);
        let source=rt.register_generated_document(crate::pdf_output::save(&mut pdf).unwrap(),"Group.pdf").unwrap();
        pdf.get_dictionary_mut(page).unwrap().set("PieceInfo",dictionary!{});
        let guarded=rt.register_generated_document(crate::pdf_output::save(&mut pdf).unwrap(),"Guarded.pdf").unwrap();
        let before=rt.documents.len();
        for case in ["stale","publication","abandoned","success","noop"]{
            let (reply,receive)=mpsc::channel();let receive=if case=="abandoned"{drop(receive);None}else{Some(receive)};
            rt.handle_occurrence_group(Request::Replace{source:if case=="publication"{guarded.id.clone()}else{source.id.clone()},occurrences:vec![0,0],selected:1,indices:vec![0,1],expected:if case=="stale"{"stale"}else{"Old words"}.into(),replacement:if case=="noop"{"Old words"}else{"New text"}.into(),reply});
            if let Some(receive)=receive{
                let result=receive.recv().unwrap();
                if matches!(case,"success"|"noop"){let result=result.unwrap();assert_eq!(rt.documents.len(),before+1);rt.discard_unpublished_source(&result.document.id);}
                else{let error=result.unwrap_err();assert!(error.to_string().contains(if case=="stale"{"changed"}else{"document-linked"}),"{case}: {error}");}
            }
            assert_eq!(rt.documents.len(),before,"{case}");
        }
        let (reply,receive)=mpsc::channel();rt.handle_occurrence_group(Request::Inspect{source:source.id,page:0,indices:vec![99,100],reply});assert!(receive.recv().unwrap().is_err());assert_eq!(rt.documents.len(),before);
    }
}
