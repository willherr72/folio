//! One complete-source candidate for a group of inspected text edits.
use super::*;
use serde::{Deserialize,Serialize};
#[cfg(test)]
thread_local!{static COMPLETED_STAGES:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct TextBatchEdit {
    pub page_index:usize,
    pub object_path:Vec<usize>,
    pub expected_text:String,
    pub replacement:String,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub font_id:Option<String>,
}

pub(super) struct Request {
    pub source:String,
    pub edits:Vec<TextBatchEdit>,
    pub reply:mpsc::Sender<EngineResult<PageContentReplacement>>,
}

fn invalid()->EngineError{EngineError::InvalidRequest("The text batch has invalid, stale, overlapping or unsupported selections.".into())}
/// Shared by mutation and planners after logical text is converted to the exact
/// native identity. Generated separators and optional font IDs count as bytes.
pub(super) fn batch_strings_fit(edits:&[TextBatchEdit])->bool {
    let mut bytes=0usize;
    for edit in edits {
        if edit.expected_text.is_empty()||edit.expected_text.len()>16*1024||edit.replacement.is_empty()||edit.replacement.len()>4096{return false;}
        let Some(next)=bytes.checked_add(edit.expected_text.len()).and_then(|n|n.checked_add(edit.replacement.len())).and_then(|n|n.checked_add(edit.font_id.as_ref().map_or(0,String::len)))else{return false};
        if next>512*1024{return false;}bytes=next;
    }
    true
}
pub(super) fn validate_shape(edits:&[TextBatchEdit])->EngineResult<()>{
    if edits.is_empty()||edits.len()>64{return Err(invalid());}
    let mut pages=HashSet::new();
    for (i,edit) in edits.iter().enumerate(){
        if edit.object_path.is_empty()||edit.object_path.len()>13||edit.expected_text.is_empty()||edit.expected_text.len()>16*1024
            ||edit.replacement.is_empty()||edit.replacement.len()>4096
            ||edit.font_id.as_ref().is_some_and(|id|id.len()!=64||!id.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))){return Err(invalid());}
        pages.insert(edit.page_index);
        if pages.len()>32{return Err(invalid());}
        if edits[..i].iter().any(|other|other.page_index==edit.page_index&&
            (other.object_path.starts_with(&edit.object_path)||edit.object_path.starts_with(&other.object_path))){return Err(invalid());}
    }
    if !batch_strings_fit(edits){return Err(invalid());}
    Ok(())
}

#[derive(Clone,PartialEq)]
pub(super) enum Target{Page(EditableTextRun),Nested(FormTextRun)}

impl PdfEngine {
    pub fn replace_text_batch(&self,source_id:&str,edits:&[TextBatchEdit])->EngineResult<PageContentReplacement>{
        validate_shape(edits)?;
        let(reply,receive)=mpsc::channel();
        self.send(WorkerRequest::TextBatch(Request{source:source_id.into(),edits:edits.to_vec(),reply}))?;
        let mut result=receive.recv().map_err(|_|EngineError::WorkerStopped)??;
        result.document=self.document_in_points(Ok(result.document))?;Ok(result)
    }
}

impl WorkerRuntime {
    pub(super) fn handle_text_batch(&mut self,request:Request){
        let result=self.replace_text_batch(&request.source,&request.edits);
        if let Err(mpsc::SendError(Ok(result)))=request.reply.send(result){self.discard_unpublished_source(&result.document.id);}
    }
    pub(super) fn text_batch_target(&mut self,source:&str,edit:&TextBatchEdit)->EngineResult<Target>{
        if edit.object_path.len()==1{
            let listed=self.list_transaction_text_runs(source,edit.page_index,true)?;
            let mut matches=listed.runs.into_iter().filter(|run|run.object_index==edit.object_path[0]);
            let run=matches.next().ok_or_else(invalid)?;
            if matches.next().is_some()||run.text!=edit.expected_text||(!run.supported&&!(edit.font_id.is_some()&&run.can_substitute))
                ||(edit.font_id.is_some()&&!run.can_substitute){return Err(invalid());}
            Ok(Target::Page(run))
        }else{
            let listed=self.list_complete_form_text_runs(source,edit.page_index)?;
            let mut matches=listed.runs.into_iter().filter(|run|run.object_path==edit.object_path);
            let run=matches.next().ok_or_else(invalid)?;
            if matches.next().is_some()||run.text!=edit.expected_text||!run.supported||(edit.font_id.is_some()&&!run.can_substitute){return Err(invalid());}
            Ok(Target::Nested(run))
        }
    }
    fn replace_text_batch(&mut self,source:&str,edits:&[TextBatchEdit])->EngineResult<PageContentReplacement>{
        #[cfg(test)]COMPLETED_STAGES.with(|steps|steps.set(0));
        validate_shape(edits)?;
        let count=self.editing_document(source)?.document.page_count() as usize;
        if edits.iter().any(|edit|edit.page_index>=count){return Err(invalid());}
        let order=(0..count).collect::<Vec<_>>();
        let mut prepared=edits.iter().map(|edit|self.text_batch_target(source,edit).map(|target|(edit,target))).collect::<EngineResult<Vec<_>>>()?;
        prepared.sort_by(|(a,_),(b,_)|a.page_index.cmp(&b.page_index).then_with(||b.object_path.cmp(&a.object_path)));
        let mut staged:Option<PageContentReplacement>=None;
        let operation=(||{
            for (edit,original) in prepared{
                let current=staged.as_ref().map_or(source,|candidate|candidate.document.id.as_str());
                if self.text_batch_target(current,edit)?!=original{return Err(invalid());}
                let next=if edit.object_path.len()==1{
                    self.replace_text_occurrence(current,&order,edit.page_index,edit.object_path[0],&edit.expected_text,&edit.replacement,edit.font_id.as_deref())?
                }else{
                    self.replace_complete_form_text_occurrence(current,&order,edit.page_index,&edit.object_path,&edit.expected_text,&edit.replacement,edit.font_id.as_deref())?
                };
                if next.document.id==source||staged.as_ref().is_some_and(|previous|next.document.id==previous.document.id){return Err(invalid());}
                if next.page_indices!=order||next.document.pages.len()!=count{
                    self.discard_unpublished_source(&next.document.id);return Err(invalid());
                }
                if let Some(previous)=staged.replace(next){self.discard_unpublished_source(&previous.document.id);}
                #[cfg(test)]COMPLETED_STAGES.with(|steps|steps.set(steps.get()+1));
            }
            staged.take().ok_or_else(invalid)
        })();
        if operation.is_err(){if let Some(candidate)=staged.take(){self.discard_unpublished_source(&candidate.document.id);}}
        operation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary,Document,Object,Stream};

    fn fixture(rotation:i64,unit:f32)->Vec<u8>{
        let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
        let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
        let leaf=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),200.into(),30.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}},b"BT /F1 12 Tf 1 0 0 1 5 10 Tm (Shared) Tj ET".to_vec()));
        let mut kids=vec![];
        for index in 0..2{
            let stream=if index==0 {b"BT /F1 12 Tf 1 0 0 1 30 330 Tm (Alpha) Tj ET BT /F1 12 Tf 1 0 0 1 30 280 Tm (Beta) Tj ET q 1 0 0 1 30 150 cm /Shared Do Q q 1 0 0 1 30 100 cm /Shared Do Q".as_slice()}
            else{b"BT /F1 12 Tf 1 0 0 1 30 330 Tm (Gamma) Tj ET q 1 0 0 1 30 150 cm /Shared Do Q".as_slice()};
            let content=pdf.add_object(Stream::new(dictionary!{},stream.to_vec()));
            let note=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![350.into(),20.into(),370.into(),40.into()],"Contents"=>Object::string_literal("Keep note")});
            let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),500.into()],"Rotate"=>rotation,"UserUnit"=>Object::Real(unit),"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font},"XObject"=>dictionary!{"Shared"=>leaf}},"Contents"=>content,"Annots"=>vec![Object::Reference(note)]});
            kids.push(Object::Reference(page));
        }
        pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>2,"Kids"=>kids}.into());
        let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",catalog);
        crate::pdf_output::save(&mut pdf).unwrap()
    }
    fn runtime()->WorkerRuntime{
        WorkerRuntime::new(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap()
    }
    fn source(rt:&mut WorkerRuntime)->DocumentInfo{rt.register_generated_document(fixture(0,1.),"Batch.pdf").unwrap()}
    fn root_edit(rt:&mut WorkerRuntime,id:&str,page:usize,text:&str,replacement:&str)->TextBatchEdit{
        let runs=rt.list_transaction_text_runs(id,page,true).unwrap();
        let run=runs.runs.into_iter().find(|run|run.text==text).unwrap();assert!(run.supported,"{run:?}");
        TextBatchEdit{page_index:page,object_path:vec![run.object_index],expected_text:run.text,replacement:replacement.into(),font_id:None}
    }
    fn form_runs(rt:&mut WorkerRuntime,id:&str,page:usize)->Vec<FormTextRun>{
        let(reply,receive)=mpsc::channel();
        rt.handle_occurrence_form(occurrence_form::Request::List{source:id.into(),page,reply});
        let listed=receive.recv().unwrap().unwrap();assert!(listed.reason.is_none(),"{:?}",listed.reason);listed.runs
    }
    #[test]
    fn atomic_text_batch_root_order_and_pages_preserve_original(){
        let mut rt=runtime();let info=source(&mut rt);let original=rt.editing_document(&info.id).unwrap().bytes().unwrap();let count=rt.documents.len();
        let edits=vec![root_edit(&mut rt,&info.id,0,"Alpha","First"),root_edit(&mut rt,&info.id,0,"Beta","Second"),root_edit(&mut rt,&info.id,1,"Gamma","Third")];
        let result=rt.replace_text_batch(&info.id,&edits).unwrap();
        assert_eq!(result.page_indices,vec![0,1]);assert_eq!(result.document.pages.len(),2);assert_eq!(rt.documents.len(),count+1);
        let first=rt.extract_text(&result.document.id,0).unwrap();assert!(first.contains("First")&&first.contains("Second"),"{first}");assert!(!first.contains("Alpha")&&!first.contains("Beta"));
        assert!(rt.extract_text(&result.document.id,1).unwrap().contains("Third"));
        assert_eq!(rt.editing_document(&info.id).unwrap().bytes().unwrap().as_ref(),original.as_ref());
        let bytes=rt.editing_document(&result.document.id).unwrap().bytes().unwrap();let saved=Document::load_mem(bytes.as_ref()).unwrap();
        for page in saved.get_pages().values(){assert_eq!(saved.get_dictionary(*page).unwrap().get(b"Annots").unwrap().as_array().unwrap().len(),1);}
        rt.discard_unpublished_source(&result.document.id);assert_eq!(rt.documents.len(),count);
    }
    #[test]
    fn atomic_text_batch_nested_shared_occurrence_isolated(){
        let mut rt=runtime();let info=source(&mut rt);let count=rt.documents.len();
        let nested=form_runs(&mut rt,&info.id,0).remove(0);assert!(nested.supported);
        let edits=vec![root_edit(&mut rt,&info.id,0,"Alpha","First"),TextBatchEdit{page_index:0,object_path:nested.object_path,expected_text:nested.text,replacement:"Only here".into(),font_id:None}];
        let result=rt.replace_text_batch(&info.id,&edits).unwrap();let page=rt.extract_text(&result.document.id,0).unwrap();
        assert_eq!(page.matches("Only here").count(),1);assert_eq!(page.matches("Shared").count(),1);assert!(page.contains("First"));
        assert_eq!(rt.extract_text(&result.document.id,1).unwrap().matches("Shared").count(),1);
        assert_eq!(rt.extract_text(&info.id,0).unwrap().matches("Shared").count(),2);
        rt.discard_unpublished_source(&result.document.id);assert_eq!(rt.documents.len(),count);
    }
    #[test]
    fn atomic_text_batch_late_fit_failure_releases_stages(){
        let mut rt=runtime();let info=source(&mut rt);let count=rt.documents.len();let original=rt.editing_document(&info.id).unwrap().bytes().unwrap();
        let too_wide="W".repeat(300);
        for bad_font in [false,true]{
            let mut failing=root_edit(&mut rt,&info.id,0,"Alpha",if bad_font{"First"}else{&too_wide});
            if bad_font{failing.font_id=Some("a".repeat(64));}
            let edits=vec![failing,root_edit(&mut rt,&info.id,0,"Beta","Second")];
            let error=rt.replace_text_batch(&info.id,&edits).unwrap_err().to_string().to_lowercase();
            assert!(if bad_font{error.contains("font")}else{error.contains("page")||error.contains("fit")||error.contains("overlap")},"{error}");
            assert_eq!(COMPLETED_STAGES.with(|steps|steps.get()),1,"the failure must occur after a real successful edit");
            assert_eq!(rt.documents.len(),count);assert_eq!(rt.editing_document(&info.id).unwrap().bytes().unwrap().as_ref(),original.as_ref());
        }
    }
    #[test]
    fn atomic_text_batch_malformed_comment_is_not_bypassed(){
        let mut rt=runtime();let mut pdf=Document::load_mem(&fixture(0,1.)).unwrap();
        let page=*pdf.get_pages().values().next().unwrap();let note=pdf.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap();
        pdf.get_dictionary_mut(note).unwrap().set("Contents",Object::Name(b"Malformed name".to_vec()));
        let info=rt.register_generated_document(crate::pdf_output::save(&mut pdf).unwrap(),"Malformed note.pdf").unwrap();let count=rt.documents.len();
        let edit=root_edit(&mut rt,&info.id,0,"Alpha","First");
        let error=rt.replace_text_batch(&info.id,&[edit]).unwrap_err().to_string();assert!(error.contains("comments"),"{error}");
        assert_eq!(rt.documents.len(),count);
    }
    #[test]
    fn atomic_text_batch_stale_duplicate_and_budget_reject_without_sources(){
        let mut rt=runtime();let info=source(&mut rt);let count=rt.documents.len();let edit=root_edit(&mut rt,&info.id,0,"Alpha","First");
        let mut stale=edit.clone();stale.expected_text="stale".into();let mut huge=edit.clone();huge.replacement="x".repeat(4097);
        for edits in [vec![],vec![edit.clone(),edit.clone()],vec![stale],vec![huge],vec![edit;65]]{
            assert!(rt.replace_text_batch(&info.id,&edits).is_err());assert_eq!(rt.documents.len(),count);
        }
    }
    #[test]
    fn atomic_text_batch_abandoned_success_and_noop_release_candidates(){
        let mut rt=runtime();let info=source(&mut rt);let count=rt.documents.len();let edit=root_edit(&mut rt,&info.id,0,"Alpha","Alpha");
        let result=rt.replace_text_batch(&info.id,&[edit.clone()]).unwrap();assert_eq!(rt.extract_text(&result.document.id,0).unwrap(),rt.extract_text(&info.id,0).unwrap());rt.discard_unpublished_source(&result.document.id);
        let(reply,receive)=mpsc::channel();drop(receive);rt.handle_text_batch(Request{source:info.id,edits:vec![edit],reply});assert_eq!(rt.documents.len(),count);
    }
    #[test]
    fn atomic_text_batch_public_dimensions_keep_rotation_and_userunit(){
        let temp=tempfile::tempdir().unwrap();let path=temp.path().join("Batch physical.pdf");std::fs::write(&path,fixture(90,1.7)).unwrap();
        let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();let source=engine.open_document(&path).unwrap();
        let run=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs.into_iter().find(|run|run.text=="Alpha").unwrap();
        let result=engine.replace_text_batch(&source.id,&[TextBatchEdit{page_index:0,object_path:vec![run.object_index],expected_text:run.text,replacement:"First".into(),font_id:None}]).unwrap();
        assert!((result.document.pages[0].width-source.pages[0].width).abs()<0.001);assert!((result.document.pages[0].height-source.pages[0].height).abs()<0.001);
        engine.close_document(&result.document.id).unwrap();engine.close_document(&source.id).unwrap();
    }
    #[test]
    fn atomic_text_batch_fractional_userunit_keeps_second_target_binding(){
        let mut rt=runtime();let mut pdf=Document::load_mem(&fixture(90,1.7)).unwrap();
        let mut changed_streams=0;
        for object in pdf.objects.values_mut(){
            if let Object::Stream(stream)=object{
                let content=if stream.dict.has(b"Filter"){stream.decompressed_content().unwrap()}else{stream.content.clone()};
                let Ok(text)=String::from_utf8(content)else{continue};
                let changed=text.replace("/F1 12 Tf","/F1 7.058823529 Tf");
                if changed!=text{stream.set_plain_content(changed.into_bytes());changed_streams+=1;}
            }
        }
        assert_eq!(changed_streams,3,"both page text streams and the shared form must be changed");
        let info=rt.register_generated_document(crate::pdf_output::save(&mut pdf).unwrap(),"Fractional batch.pdf").unwrap();
        let edits=vec![root_edit(&mut rt,&info.id,0,"Alpha","First"),root_edit(&mut rt,&info.id,0,"Beta","Second")];
        let result=rt.replace_text_batch(&info.id,&edits).unwrap();assert_eq!(COMPLETED_STAGES.with(|steps|steps.get()),2);
        let text=rt.extract_text(&result.document.id,0).unwrap();assert!(text.contains("First")&&text.contains("Second"),"{text}");
        rt.discard_unpublished_source(&result.document.id);
    }
}
