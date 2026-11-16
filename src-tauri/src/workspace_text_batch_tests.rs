use super::*;
use lopdf::{dictionary,Document,Object,Stream};

fn source(rt:&mut WorkerRuntime,name:&str,second:&str)->DocumentInfo {
    let mut pdf=Document::with_version("1.7");let parent=pdf.new_object_id();
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let mut kids=vec![];
    for text in ["Alpha",second]{
        let content=pdf.add_object(Stream::new(dictionary!{},format!("BT /F1 12 Tf 30 330 Td ({text}) Tj ET").into_bytes()));
        let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>parent,"MediaBox"=>vec![0.into(),0.into(),400.into(),500.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});kids.push(Object::Reference(page));
    }
    pdf.objects.insert(parent,dictionary!{"Type"=>"Pages","Count"=>2,"Kids"=>kids}.into());
    let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>parent});pdf.trailer.set("Root",root);
    let info=pdf.add_object(dictionary!{"Title"=>Object::string_literal("Preserve source metadata")});pdf.trailer.set("Info",info);
    rt.register_generated_document(crate::pdf_output::save(&mut pdf).unwrap(),name).unwrap()
}
fn runtime()->WorkerRuntime{WorkerRuntime::new(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap()}
fn page(source:&str,index:usize,id:&str)->PagePlan{PagePlan{id:id.into(),source_id:source.into(),page_index:index,width:400.,height:500.,rotation:90,crop:Some(AnnotationRect{x:10.,y:20.,width:350.,height:450.}),overlays:vec![]}}
fn edit(page:&PagePlan,expected:&str,replacement:&str)->WorkspaceTextBatchEdit{WorkspaceTextBatchEdit{page_id:page.id.clone(),edit:crate::TextBatchEdit{page_index:page.page_index,object_path:vec![0],expected_text:expected.into(),replacement:replacement.into(),font_id:None}}}
fn text(rt:&mut WorkerRuntime,source:&str,page:usize)->String{rt.list_transaction_text_runs(source,page,true).unwrap().runs.into_iter().map(|r|r.text).collect::<Vec<_>>().join("|")}

#[test]
fn workspace_text_batch_separates_duplicate_occurrences_and_preserves_imported_sources(){
    let mut rt=runtime();let first=source(&mut rt,"First.pdf","Beta");let second=source(&mut rt,"Second.pdf","Gamma");let before=rt.documents.len();
    let pages=vec![page(&first.id,0,"first"),page(&second.id,0,"import"),page(&first.id,1,"second"),page(&first.id,0,"duplicate")];
    let original=rt.document(&first.id).unwrap().bytes().unwrap().as_ref().to_vec();
    let edits=vec![edit(&pages[0],"Alpha","One"),edit(&pages[1],"Alpha","Two"),edit(&pages[3],"Alpha","Three")];
    let result=rt.replace_workspace_text_batch(&pages,&edits,&Default::default()).unwrap();assert_eq!(result.sources.len(),2);assert_eq!(rt.documents.len(),before+2);
    for candidate in &result.sources{
        let group=pages.iter().filter(|p|p.source_id==candidate.source_id).collect::<Vec<_>>();assert_eq!(group.len(),candidate.page_indices.len());
        for(p,index)in group.iter().zip(&candidate.page_indices){assert_eq!(text(&mut rt,&candidate.document.id,*index),match p.id.as_str(){"first"=>"One","import"=>"Two","duplicate"=>"Three",_=>"Beta"});}
        let bytes=rt.document(&candidate.document.id).unwrap().bytes().unwrap();let pdf=Document::load_mem(bytes.as_ref()).unwrap();let info=pdf.get_dictionary(pdf.trailer.get(b"Info").unwrap().as_reference().unwrap()).unwrap();assert_eq!(info.get(b"Title").unwrap().as_str().unwrap(),b"Preserve source metadata");
    }
    assert_eq!(rt.document(&first.id).unwrap().bytes().unwrap().as_ref(),original.as_slice());assert_eq!(text(&mut rt,&first.id,0),"Alpha");
    for candidate in result.sources{rt.discard_unpublished_source(&candidate.document.id);}assert_eq!(rt.documents.len(),before);
}
#[test]
fn workspace_text_batch_rejects_duplicate_stale_and_foreign_page_targets_before_mutation(){
    let mut rt=runtime();let info=source(&mut rt,"First.pdf","Beta");let pages=vec![page(&info.id,0,"one"),page(&info.id,1,"two")];let before=rt.documents.len();
    let valid=edit(&pages[0],"Alpha","New");
    for edits in [vec![valid.clone(),valid.clone()],vec![edit(&pages[0],"Stale","New")],vec![WorkspaceTextBatchEdit{page_id:"foreign".into(),..valid.clone()}],vec![WorkspaceTextBatchEdit{edit:crate::TextBatchEdit{page_index:1,..valid.edit.clone()},..valid.clone()}],vec![]]{
        assert!(rt.replace_workspace_text_batch(&pages,&edits,&Default::default()).is_err());assert_eq!(rt.documents.len(),before);
    }
}
#[test]
fn workspace_text_batch_late_failure_releases_previously_staged_sources(){
    let mut rt=runtime();let first=source(&mut rt,"First.pdf","Beta");let second=source(&mut rt,"Second.pdf","Gamma");
    let pages=vec![page(&first.id,0,"one"),page(&second.id,0,"two")];let before=rt.documents.len();
    let edits=vec![edit(&pages[0],"Alpha","New"),edit(&pages[1],"Alpha",&"Too wide ".repeat(100))];
    let(tx,rx)=mpsc::channel();let control=crate::ReplacementPlanningControl::new(Some(tx));
    assert!(rt.replace_workspace_text_batch(&pages,&edits,&control).is_err());assert_eq!(rt.documents.len(),before);
    assert!(rx.try_iter().any(|progress|progress.completed==1),"The late-failure oracle must reach a completed first stage");
    assert_eq!(text(&mut rt,&first.id,0),"Alpha");assert_eq!(text(&mut rt,&second.id,0),"Alpha");
}
#[test]
fn workspace_text_batch_cancelled_request_leaves_originals_owned(){
    let mut rt=runtime();let info=source(&mut rt,"First.pdf","Beta");let pages=vec![page(&info.id,0,"one")];let before=rt.documents.len();
    let control=crate::ReplacementPlanningControl::default();control.cancel();
    assert!(rt.replace_workspace_text_batch(&pages,&[edit(&pages[0],"Alpha","New")],&control).is_err());assert_eq!(rt.documents.len(),before);
}
#[test]
fn workspace_text_batch_abandoned_reply_releases_all_final_sources(){
    let mut rt=runtime();let first=source(&mut rt,"First.pdf","Beta");let second=source(&mut rt,"Second.pdf","Gamma");
    let pages=vec![page(&first.id,0,"one"),page(&second.id,0,"two")];let edits=vec![edit(&pages[0],"Alpha","One"),edit(&pages[1],"Alpha","Two")];let before=rt.documents.len();
    let(reply,receive)=mpsc::channel();drop(receive);let(tx,rx)=mpsc::channel();
    rt.handle_workspace_text_batch(Request{pages,edits,control:crate::ReplacementPlanningControl::new(Some(tx)),reply});
    assert_eq!(rt.documents.len(),before);assert!(rx.try_iter().any(|progress|progress.completed==2));
    assert_eq!(text(&mut rt,&first.id,0),"Alpha");assert_eq!(text(&mut rt,&second.id,0),"Alpha");
}
