use super::*;
use lopdf::{dictionary,Document,Object,Stream};

fn setup()->(tempfile::TempDir,PdfEngine,WorkspaceReplacementHost,PagePlan){
    let dir=tempfile::tempdir().unwrap();let mut pdf=Document::with_version("1.7");let parent=pdf.new_object_id();
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let stream=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 12 Tf 30 330 Td (Alpha) Tj ET".to_vec()));
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>parent,"MediaBox"=>vec![0.into(),0.into(),400.into(),500.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>stream});
    pdf.objects.insert(parent,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into());let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>parent});pdf.trailer.set("Root",root);
    let path=dir.path().join("Authored.pdf");std::fs::write(&path,crate::pdf_output::save(&mut pdf).unwrap()).unwrap();
    let engine=PdfEngine::start(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();let info=engine.open_document(path).unwrap();let host=WorkspaceReplacementHost::new(engine.clone());
    let page=PagePlan{id:"one".into(),source_id:info.id,page_index:0,width:400.,height:500.,rotation:0,crop:None,overlays:vec![]};(dir,engine,host,page)
}
fn intent()->WorkspaceReplacementIntent{WorkspaceReplacementIntent{query:"Alpha".into(),replacement:"New".into(),options:Default::default(),selected:None}}
fn pending(window:&ReplacementWindow,page:&PagePlan)->(String,WorkspaceReplacementPreview){let id=window.begin(vec![page.clone()]).unwrap();let plan=window.plan(&id,&intent()).unwrap();let preview=window.preview(&id,&plan.plan_id).unwrap();(id,preview)}

#[test]
fn workspace_replacement_host_requires_registered_window_and_scopes_sessions(){
    let(_dir,engine,host,page)=setup();assert!(host.capture("main").is_err());host.register_window("main").unwrap();assert!(host.register_window("main").is_err());host.register_window("other").unwrap();
    let main=host.capture("main").unwrap();let other=host.capture("other").unwrap();let session=main.begin(vec![page.clone()]).unwrap();
    assert!(other.plan(&session,&intent()).is_err());assert!(main.plan("foreign",&intent()).is_err());main.release(&session).unwrap();assert!(engine.page_text(&page.source_id,0).is_ok());
}
#[test]
fn workspace_replacement_host_applies_exact_preview_and_release_retains_adopted_sources(){
    let(_dir,engine,host,page)=setup();host.register_window("main").unwrap();let window=host.capture("main").unwrap();let(session,preview)=pending(&window,&page);let candidate=preview.result.sources[0].document.id.clone();
    assert!(window.apply(&session,"foreign").is_err());let result=window.apply(&session,&preview.preview_id).unwrap();assert_eq!(result.sources[0].document.id,candidate);
    window.release(&session).unwrap();assert!(engine.page_text(&candidate,0).is_ok());assert!(engine.page_text(&page.source_id,0).is_ok());
    engine.close_document(&candidate).unwrap();host.forget_document(&candidate);host.detach("main").unwrap().cleanup();assert!(engine.page_text(&page.source_id,0).is_ok());
}
#[test]
fn workspace_replacement_host_new_plan_and_release_dispose_pending_candidates(){
    let(_dir,engine,host,page)=setup();host.register_window("main").unwrap();let window=host.capture("main").unwrap();let(session,preview)=pending(&window,&page);let old=preview.result.sources[0].document.id.clone();
    let plan=window.plan(&session,&intent()).unwrap();assert!(engine.page_text(&old,0).is_err());assert!(window.apply(&session,&preview.preview_id).is_err());
    let preview=window.preview(&session,&plan.plan_id).unwrap();let candidate=preview.result.sources[0].document.id.clone();window.release(&session).unwrap();assert!(engine.page_text(&candidate,0).is_err());assert!(engine.page_text(&page.source_id,0).is_ok());
}
#[test]
fn workspace_replacement_host_cancel_invalidates_existing_plan_and_preview(){
    let(_dir,engine,host,page)=setup();host.register_window("main").unwrap();let window=host.capture("main").unwrap();let session=window.begin(vec![page.clone()]).unwrap();let plan=window.plan(&session,&intent()).unwrap();
    window.cancel(&session).unwrap();assert!(window.preview(&session,&plan.plan_id).is_err());
    let plan=window.plan(&session,&intent()).unwrap();let preview=window.preview(&session,&plan.plan_id).unwrap();let candidate=preview.result.sources[0].document.id.clone();
    window.cancel(&session).unwrap();assert!(window.apply(&session,&preview.preview_id).is_err());window.discard_cancelled(&session).unwrap();assert!(engine.page_text(&candidate,0).is_err());window.release(&session).unwrap();
}
#[test]
fn workspace_replacement_host_destroyed_window_cleans_pending_and_adopted_results(){
    let(_dir,engine,host,page)=setup();host.register_window("main").unwrap();let window=host.capture("main").unwrap();let(a,pending_a)=pending(&window,&page);let adopted=pending_a.result.sources[0].document.id.clone();window.apply(&a,&pending_a.preview_id).unwrap();
    let(_b,pending_b)=pending(&window,&page);let candidate=pending_b.result.sources[0].document.id.clone();let cleanup=host.detach("main").unwrap();assert!(host.capture("main").is_err());assert!(window.plan(&a,&intent()).is_err());cleanup.cleanup();
    assert!(engine.page_text(&adopted,0).is_err());assert!(engine.page_text(&candidate,0).is_err());assert!(engine.page_text(&page.source_id,0).is_ok());
}
#[test]
fn workspace_replacement_host_rejects_cross_session_preview_ids_and_bounds_live_sessions(){
    let(_dir,_engine,host,page)=setup();host.register_window("main").unwrap();let window=host.capture("main").unwrap();let(a,preview)=pending(&window,&page);let b=window.begin(vec![page.clone()]).unwrap();
    assert!(window.apply(&b,&preview.preview_id).is_err());let c=window.begin(vec![page.clone()]).unwrap();let d=window.begin(vec![page.clone()]).unwrap();assert!(window.begin(vec![page.clone()]).is_err());window.release(&c).unwrap();let e=window.begin(vec![page]).unwrap();
    for id in [a,b,d,e]{window.release(&id).unwrap();}
}
#[test]
fn workspace_replacement_host_rejects_invalid_snapshot_without_closing_borrowed_sources(){
    let(_dir,engine,host,page)=setup();host.register_window("main").unwrap();let window=host.capture("main").unwrap();let mut invalid=page.clone();invalid.width=401.;
    assert!(window.begin(vec![invalid]).is_err());assert!(window.begin(vec![]).is_err());assert!(engine.page_text(&page.source_id,0).is_ok());
}
#[test]
fn workspace_replacement_host_cancel_does_not_wait_for_pdf_operation_lock(){
    let(_dir,_engine,host,page)=setup();host.register_window("main").unwrap();let window=host.capture("main").unwrap();let session=window.begin(vec![page]).unwrap();
    let lock=window.window.data.lock().unwrap();let owned=window.clone();let id=session.clone();let(tx,rx)=std::sync::mpsc::channel();
    let thread=std::thread::spawn(move||{tx.send(owned.cancel(&id)).unwrap();});let result=rx.recv_timeout(Duration::from_secs(1));drop(lock);thread.join().unwrap();
    assert!(result.unwrap().is_ok());window.release(&session).unwrap();
}
#[test]
fn workspace_replacement_host_expired_preview_can_be_checked_again_and_queued_cancel_preserves_new_plan(){
    let(_dir,engine,host,page)=setup();host.register_window("main").unwrap();let window=host.capture("main").unwrap();let(session,preview)=pending(&window,&page);let old=preview.result.sources[0].document.id.clone();
    {let mut data=window.window.data.lock().unwrap();let state=data.sessions.get_mut(&session).unwrap();state.touched=Instant::now()-TTL;state.preview.as_mut().unwrap().created=Instant::now()-TTL;}
    assert!(window.apply(&session,&preview.preview_id).unwrap_err().contains("expired"));assert!(engine.page_text(&old,0).is_err());
    window.cancel(&session).unwrap();let plan=window.plan(&session,&intent()).unwrap();window.discard_cancelled(&session).unwrap();
    let preview=window.preview(&session,&plan.plan_id).unwrap();assert!(engine.page_text(&preview.result.sources[0].document.id,0).is_ok());window.release(&session).unwrap();assert_eq!(host.0.bytes.load(Ordering::Acquire),0);
}
#[test]
fn workspace_replacement_host_budget_failure_releases_published_candidates_and_all_reservations(){
    let(_dir,engine,host,page)=setup();host.register_window("main").unwrap();let window=host.capture("main").unwrap();let session=window.begin(vec![page.clone()]).unwrap();let plan=window.plan(&session,&intent()).unwrap();
    let before=engine.workspace_replacement_source_count().unwrap();let held=reserve(&host.0.bytes,MAX_PENDING_BYTES,MAX_PENDING_BYTES).unwrap();
    assert!(window.preview(&session,&plan.plan_id).unwrap_err().contains("another replacement session"));assert_eq!(engine.workspace_replacement_source_count().unwrap(),before);drop(held);
    let preview=window.preview(&session,&plan.plan_id).unwrap();assert!(preview.result.retained_bytes>0);assert_eq!(engine.workspace_replacement_source_count().unwrap(),before+1);
    window.release(&session).unwrap();assert_eq!(engine.workspace_replacement_source_count().unwrap(),before);assert_eq!(host.0.bytes.load(Ordering::Acquire),0);assert_eq!(host.0.sessions.load(Ordering::Acquire),0);assert!(engine.page_text(&page.source_id,0).is_ok());
}
