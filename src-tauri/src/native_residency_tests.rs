use super::*;
use lopdf::{dictionary, Document, Object, Stream};

#[path = "native_residency_workflow_tests.rs"]
mod workflows;

#[path = "retained_source_tests.rs"]
mod retained_sources;

fn runtime() -> WorkerRuntime {
    WorkerRuntime::new(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"), Arc::new(FontRegistry::new())).unwrap()
}

fn request(rt: &mut WorkerRuntime, request: WorkerRequest) {
    let (send, receive) = mpsc::channel();
    send.send(request).unwrap();
    drop(send);
    rt.run(receive);
}

fn fixture() -> Vec<u8> {
    let mut pdf = Document::with_version("1.7");
    let tree = pdf.new_object_id();
    let second = pdf.new_object_id();
    let font = pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let first_content = pdf.add_object(Stream::new(dictionary!{},b"BT /F 14 Tf 20 200 Td (Resident alpha) Tj ET 1 0 0 rg 20 20 10 10 re f".to_vec()));
    let second_content = pdf.add_object(Stream::new(dictionary!{},b"BT /F 14 Tf 20 200 Td (Resident beta) Tj ET 0 0 1 rg 30 30 20 20 re f".to_vec()));
    let first = pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F"=>font}},"Contents"=>first_content});
    let highlight=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Highlight","Rect"=>vec![20.into(),280.into(),100.into(),300.into()],
        "QuadPoints"=>vec![20.into(),300.into(),100.into(),300.into(),20.into(),280.into(),100.into(),280.into()],
        "C"=>vec![1.into(),1.into(),0.into()],"F"=>4,"Contents"=>Object::string_literal("import me"),"P"=>second});
    let popup=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Popup","Parent"=>highlight,"P"=>second,"Rect"=>vec![0.into(),0.into(),10.into(),10.into()]});
    pdf.get_dictionary_mut(highlight).unwrap().set("Popup",popup);
    let note=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![200.into(),280.into(),220.into(),300.into()],"C"=>vec![1.into(),0.into(),0.into()],"F"=>4,"Contents"=>Object::string_literal("keep native"),"P"=>second});
    pdf.objects.insert(second,dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"UserUnit"=>1.7,
        "Resources"=>dictionary!{"Font"=>dictionary!{"F"=>font}},"Contents"=>second_content,"Annots"=>vec![highlight.into(),note.into(),popup.into()]}.into());
    pdf.objects.insert(tree,dictionary!{"Type"=>"Pages","Count"=>2,"Kids"=>vec![first.into(),second.into()]}.into());
    let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree});pdf.trailer.set("Root",catalog);
    crate::pdf_output::save(&mut pdf).unwrap()
}

fn opened(rt: &mut WorkerRuntime) -> DocumentInfo {
    let path=std::env::temp_dir().join("folio-resident-fixture.pdf");
    rt.open_bytes(fixture(),path.clone(),path,true,true,false,OpenSecurity::Authenticated(None)).unwrap()
}

fn evict(rt: &mut WorkerRuntime, id: &str) {
    let bytes=rt.document(id).unwrap().bytes().unwrap();
    let before=Arc::strong_count(&bytes);
    for _ in 0..5 {rt.create_blank_document(200.0,200.0).unwrap();}
    let (reply,receive)=mpsc::channel();request(rt,WorkerRequest::Status{reply});receive.recv().unwrap();
    assert_eq!(Arc::strong_count(&bytes)+1,before,"The source must really lose its native reader owner");
}

fn render(rt: &mut WorkerRuntime, id: &str, page: usize) -> EngineResult<Vec<u8>> {
    let (reply,receive)=mpsc::channel();
    request(rt,WorkerRequest::Render{source_id:id.into(),page_index:page,width:400,reply});
    receive.recv().unwrap()
}

#[test]
fn native_residency_reloads_imported_popups_without_duplicates_and_keeps_geometry_and_text() {
    let mut rt=runtime();let info=opened(&mut rt);
    assert_eq!(info.pages[1].overlays.len(),1);
    let source=rt.document(&info.id).unwrap();
    assert_eq!(source.imported_annotations[&1],HashSet::from([0,2]));
    let snapshot=source.imported_overlay_sources.snapshot().unwrap();
    let bytes=source.bytes().unwrap();
    let before=vec![render(&mut rt,&info.id,0).unwrap(),render(&mut rt,&info.id,1).unwrap()];
    evict(&mut rt,&info.id);
    let owners=Arc::strong_count(&bytes);
    let (reply,receive)=mpsc::channel();request(&mut rt,WorkerRequest::SourceUnits{source_id:info.id.clone(),reply});
    assert_eq!(receive.recv().unwrap().unwrap(),vec![1.0,1.7]);
    assert_eq!(Arc::strong_count(&bytes),owners,"Page metadata must not reload the native handle");
    for page in 0..2 {assert_eq!(render(&mut rt,&info.id,page).unwrap(),before[page]);}
    assert_eq!(rt.document(&info.id).unwrap().imported_overlay_sources.snapshot().unwrap(),snapshot);
    assert_eq!(rt.document(&info.id).unwrap().pdf().unwrap().pages().get(1).unwrap().annotations().len(),1);
    for (page,word) in [(0,"Resident alpha"),(1,"Resident beta")] {
        let (reply,receive)=mpsc::channel();request(&mut rt,WorkerRequest::ExtractText{source_id:info.id.clone(),page_index:page,reply});
        assert!(receive.recv().unwrap().unwrap().contains(word));
    }
    assert!(Arc::ptr_eq(&bytes,&rt.document(&info.id).unwrap().bytes().unwrap()));
    evict(&mut rt,&info.id);
    assert_eq!(render(&mut rt,&info.id,1).unwrap(),before[1]);
}

#[test]
fn native_residency_failed_annotation_replay_does_not_publish_partial_handle_and_can_retry() {
    let mut rt=runtime();let info=opened(&mut rt);let before=render(&mut rt,&info.id,1).unwrap();
    evict(&mut rt,&info.id);
    let bytes=rt.document(&info.id).unwrap().bytes().unwrap();let owners=Arc::strong_count(&bytes);
    rt.documents.get_mut(&info.id).unwrap().imported_annotations.get_mut(&1).unwrap().insert(999);
    for _ in 0..2 {
        assert!(render(&mut rt,&info.id,1).is_err());
        assert_eq!(Arc::strong_count(&bytes),owners,"Failed replay must drop its temporary native reader");
    }
    rt.documents.get_mut(&info.id).unwrap().imported_annotations.get_mut(&1).unwrap().remove(&999);
    rt.documents.get_mut(&info.id).unwrap().imported_annotations.insert(usize::MAX,HashSet::new());
    assert!(render(&mut rt,&info.id,1).is_err());
    assert_eq!(Arc::strong_count(&bytes),owners);
    rt.documents.get_mut(&info.id).unwrap().imported_annotations.remove(&usize::MAX);
    assert_eq!(render(&mut rt,&info.id,1).unwrap(),before);
}

#[test]
fn native_residency_reopening_keeps_authenticated_permissions_and_original_identity() {
    let mut rt=runtime();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("protected.pdf");
    let change: crate::protection::ProtectionChange=serde_json::from_value(serde_json::json!({"mode":"passwords","userPassword":"reader","ownerPassword":"owner","permissions":{"print":true,"printHighQuality":false,"copyContent":false,"editContent":false,"annotate":false,"fillForms":false,"assemble":false}})).unwrap();
    std::fs::write(&path,change.apply(&fixture()).unwrap()).unwrap();
    let info=rt.open_document(path.clone(),Some("reader"),None,true,true).unwrap();
    let identity=rt.document(&info.id).unwrap().original_path.clone();
    let policy=rt.document(&info.id).unwrap().policy.as_ref().unwrap().snapshot(rt.document(&info.id).unwrap().bytes().unwrap().as_ref()).unwrap();
    let before=render(&mut rt,&info.id,0).unwrap();evict(&mut rt,&info.id);
    assert_eq!(render(&mut rt,&info.id,0).unwrap(),before);
    assert!(matches!(rt.extract_text(&info.id,0),Err(EngineError::PermissionDenied(_))));
    assert!(matches!(rt.editing_document(&info.id),Err(EngineError::PermissionDenied(_))));
    let source=rt.document(&info.id).unwrap();assert_eq!(source.original_path,identity);
    let after=source.policy.as_ref().unwrap().snapshot(source.bytes().unwrap().as_ref()).unwrap();
    let restored_before=policy.restore(source.bytes().unwrap().as_ref()).unwrap().unwrap();
    let restored_after=after.restore(source.bytes().unwrap().as_ref()).unwrap().unwrap();
    assert!(restored_before.same_origin(&restored_after));
    assert_eq!(serde_json::to_value(restored_before.info()).unwrap(),serde_json::to_value(restored_after.info()).unwrap());
    // The sealed context is intentionally randomized. Compare its authenticated
    // meaning above, then every remaining provenance field without ciphertext.
    let mut before_state=serde_json::to_value(policy).unwrap();before_state.as_object_mut().unwrap().remove("context");
    let mut after_state=serde_json::to_value(after).unwrap();after_state.as_object_mut().unwrap().remove("context");
    assert_eq!(before_state,after_state);
    let printed=rt.prepare_print(ExportRequest{flatten:true,pages:vec![PagePlan{crop:None,id:"printed".into(),source_id:info.id.clone(),page_index:0,width:300.0,height:400.0,rotation:0,overlays:vec![]}]}).unwrap();
    let before=render(&mut rt,&printed.id,0).unwrap();evict(&mut rt,&printed.id);
    assert_eq!(render(&mut rt,&printed.id,0).unwrap(),before);
    let source=rt.document(&printed.id).unwrap();assert!(source.for_printing);assert_eq!(source.print_max_dpi,Some(150));
    assert!(rt.copying_document(&printed.id).is_err());
}

#[test]
fn native_residency_render_refreshes_recency_without_reopening_metadata_only_sources() {
    let mut rt=runtime();let mut held=vec![];
    for _ in 0..7 {
        let info=rt.create_blank_document(200.0,200.0).unwrap();
        let bytes=rt.document(&info.id).unwrap().bytes().unwrap();let owners=Arc::strong_count(&bytes);
        held.push((info.id,bytes,owners));
    }
    let (reply,receive)=mpsc::channel();request(&mut rt,WorkerRequest::Status{reply});receive.recv().unwrap();
    assert_eq!(Arc::strong_count(&held[0].1)+1,held[0].2);
    render(&mut rt,&held[0].0,0).unwrap();
    assert_eq!(Arc::strong_count(&held[0].1),held[0].2);
    for _ in 0..2 {rt.create_blank_document(200.0,200.0).unwrap();}
    let (reply,receive)=mpsc::channel();request(&mut rt,WorkerRequest::Status{reply});receive.recv().unwrap();
    assert_eq!(Arc::strong_count(&held[0].1),held[0].2,"Recently rendered old source must remain native");
    assert_eq!(Arc::strong_count(&held[4].1)+1,held[4].2,"An older unaccessed source must be evicted instead");
}

#[test]
fn native_residency_imported_images_and_fonts_survive_eviction_and_image_owner_closes_once() {
    let mut rt=runtime();let temp=tempfile::tempdir().unwrap();
    let mut image=Cursor::new(vec![]);
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(2,2,image::Rgba([210,40,80,128])))
        .write_to(&mut image,ImageFormat::Png).unwrap();
    let image=rt.images.register(image.into_inner()).unwrap();
    let font=rt.fonts.register(std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
    let blank=rt.create_blank_document(300.0,400.0).unwrap();let path=temp.path().join("assets.pdf");
    let export:ExportRequest=serde_json::from_value(serde_json::json!({"flatten":false,"pages":[{"id":"assets","sourceId":blank.id,"pageIndex":0,"width":300,"height":400,"rotation":0,"overlays":[
        {"id":"image","type":"image","assetId":image.image.id,"x":40,"y":40,"width":50,"height":50,"rotation":0},
        {"id":"text","type":"text","fontId":font.id,"text":"Retained font","x":40,"y":130,"fontSize":14,"color":"#000000"}]}]})).unwrap();
    rt.export_pdf_with_bookmarks(export,None,path.clone()).unwrap();rt.close_document(&blank.id).unwrap();
    rt.images.release(&image.token).unwrap();rt.fonts.remove(&font.id).unwrap();
    let source=rt.open_document(path,None,None,true,true).unwrap();
    assert_eq!(source.pages[0].overlays.len(),2);
    assert!(rt.images.get(&image.image.id).is_ok());assert!(rt.fonts.get(&font.id).is_ok());
    let snapshot=rt.document(&source.id).unwrap().imported_overlay_sources.snapshot().unwrap();
    let before=render(&mut rt,&source.id,0).unwrap();evict(&mut rt,&source.id);
    assert!(rt.images.get(&image.image.id).is_ok());assert!(rt.fonts.get(&font.id).is_ok());
    assert_eq!(render(&mut rt,&source.id,0).unwrap(),before);
    assert_eq!(rt.document(&source.id).unwrap().imported_overlay_sources.snapshot().unwrap(),snapshot);
    evict(&mut rt,&source.id);rt.close_document(&source.id).unwrap();
    assert!(rt.images.get(&image.image.id).is_err());assert!(rt.close_document(&source.id).is_err());
}

#[test]
fn native_residency_completed_requests_release_old_native_readers_but_keep_sources() {
    let mut rt = runtime();
    let mut held = Vec::new();
    for _ in 0..10 {
        let info = rt.create_blank_document(200.0, 200.0).unwrap();
        let bytes = rt.document(&info.id).unwrap().bytes().unwrap();
        let owners = Arc::strong_count(&bytes);
        held.push((info.id, bytes, owners));
    }
    let (reply, receive) = mpsc::channel();
    request(&mut rt, WorkerRequest::Status { reply });
    assert!(receive.recv().unwrap().contains("ready"));
    let released = held.iter().filter(|(_, bytes, before)| Arc::strong_count(bytes) + 1 == *before).count();
    assert_eq!(released, 6, "Six older PDFium reader owners must be released at the worker boundary; retain four native documents");
    for (id, bytes, _) in &held {
        let (reply, receive) = mpsc::channel();
        request(&mut rt, WorkerRequest::SourceBytes { source_id: id.clone(), reply });
        assert!(Arc::ptr_eq(&receive.recv().unwrap().unwrap(), bytes));
    }
    assert_eq!(held.iter().filter(|(_, bytes, before)| Arc::strong_count(bytes) + 1 == *before).count(),6,"Metadata reads must not reopen PDFium documents");
    for (id, _, _) in &held { rt.close_document(id).unwrap(); }
    for (_, bytes, _) in &held { assert_eq!(Arc::strong_count(bytes),1); }
}
