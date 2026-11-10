use folio_engine::{ExportRequest,PdfEngine,PdfSaveControl,PdfSaveProgress};
use lopdf::{dictionary,Document,Object,Stream};
use serde_json::json;
use std::{fs,path::PathBuf,sync::{Arc,Mutex}};

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()
}
fn fixture() -> Vec<u8> {
    let mut doc=Document::with_version("1.7");let tree=doc.new_object_id();
    let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let mut pages=vec![];
    for i in 0..40 {
        let content=doc.add_object(Stream::new(dictionary!{},format!("BT /F1 12 Tf 40 200 Td (Page {i}) Tj ET").into_bytes()));
        pages.push(Object::Reference(doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"UserUnit"=>1.7,"Rotate"=>90,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content})));
    }
    doc.objects.insert(tree,Object::Dictionary(dictionary!{"Type"=>"Pages","Count"=>40,"Kids"=>pages}));
    let root=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree});doc.trailer.set("Root",root);
    let mut bytes=vec![];doc.save_to(&mut bytes).unwrap();bytes
}
fn setup(engine:&PdfEngine,dir:&std::path::Path)->(String,ExportRequest,Vec<u8>){
    let bytes=fixture();let input=dir.join("source.pdf");fs::write(&input,&bytes).unwrap();
    let source=engine.open_document(input).unwrap();
    let pages:Vec<_>=source.pages.iter().enumerate().rev().map(|(i,p)|json!({"id":format!("p{i}"),"sourceId":source.id,"pageIndex":i,"width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})).collect();
    (source.id,serde_json::from_value(json!({"pages":pages,"flatten":false})).unwrap(),bytes)
}

#[test]
fn cancelled_save_preserves_existing_destination_and_never_creates_new_output(){
    let engine=engine();let dir=tempfile::tempdir().unwrap();let(_,request,original)=setup(&engine,dir.path());
    for exists in [true,false] {
        let path=dir.path().join(if exists{"existing.pdf"}else{"new.pdf"});
        if exists{fs::write(&path,b"original destination").unwrap();}
        let control=Arc::new(PdfSaveControl::new());assert!(control.cancel());
        assert!(!engine.export_pdf_controlled(request.clone(),None,&path,control,Arc::new(|_|{})).unwrap());
        if exists{assert_eq!(fs::read(path).unwrap(),b"original destination");}else{assert!(!path.exists());}
    }
    assert_eq!(fs::read(dir.path().join("source.pdf")).unwrap(),original);
}

#[test]
fn cancelling_each_composition_stage_leaves_destination_and_source_unchanged(){
    let engine=engine();let dir=tempfile::tempdir().unwrap();let(_,request,original)=setup(&engine,dir.path());
    for (stage,completed) in [("preparing",0),("checking",0),("copying",32),("applying",32),("preserving",0),("preparingFile",0),("writing",0)] {
        let path=dir.path().join(format!("cancel-{stage}.pdf"));fs::write(&path,b"keep destination").unwrap();
        let control=Arc::new(PdfSaveControl::new());let cancel=control.clone();
        let observed=Arc::new(Mutex::new(false));let seen=observed.clone();
        let progress=Arc::new(move |event:PdfSaveProgress|{if event.stage==stage&&event.completed==completed{*seen.lock().unwrap()=true;assert!(cancel.cancel());}});
        assert!(!engine.export_pdf_controlled(request.clone(),None,&path,control,progress).unwrap(),"{stage}");
        assert!(*observed.lock().unwrap(),"{stage} was not reported");
        assert_eq!(fs::read(path).unwrap(),b"keep destination");
        assert_eq!(fs::read(dir.path().join("source.pdf")).unwrap(),original);
        assert!(fs::read_dir(dir.path()).unwrap().all(|e|!e.unwrap().file_name().to_string_lossy().starts_with(".folio-export-")));
    }
}

#[test]
fn successful_controlled_save_retains_physical_pages_and_refuses_late_cancel(){
    let engine=engine();let dir=tempfile::tempdir().unwrap();let(source,request,original)=setup(&engine,dir.path());
    let path=dir.path().join("saved.pdf");let control=Arc::new(PdfSaveControl::new());let late=control.clone();
    let events=Arc::new(Mutex::new(vec![]));let observed=events.clone();
    let progress=Arc::new(move |event:PdfSaveProgress|{if event.stage=="finishing"{assert!(!late.cancel());}observed.lock().unwrap().push(event);});
    assert!(engine.export_pdf_controlled(request,None,&path,control.clone(),progress).unwrap());
    assert!(!control.cancel());
    let events=events.lock().unwrap();
    for stage in ["preparing","copying","applying"] {
        let counts:Vec<_>=events.iter().filter(|e|e.stage==stage).map(|e|e.completed).collect();
        assert_eq!(counts.first(),Some(&0));assert_eq!(counts.last(),Some(&40));assert!(counts.windows(2).all(|v|v[0]<=v[1]));
    }
    assert_eq!(events.last().unwrap().stage,"finishing");
    let output=engine.open_document(&path).unwrap();assert_eq!(output.pages.len(),40);
    assert_eq!(engine.render_page(&output.id,0,300).unwrap(),engine.render_page(&source,39,300).unwrap());
    assert_eq!(engine.page_text(&output.id,0).unwrap().characters.iter().map(|c|c.text.as_str()).collect::<String>(),"Page 39");
    assert_eq!(fs::read(dir.path().join("source.pdf")).unwrap(),original);
}

#[test]
fn controlled_protected_save_retains_credentials_permissions_and_pixels(){
    let engine=engine();let dir=tempfile::tempdir().unwrap();
    let original=fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/protection/aes_256.pdf")).unwrap();
    let input=dir.path().join("protected.pdf");fs::write(&input,&original).unwrap();
    let source=engine.open_document_with_password(&input,"reader").unwrap();
    let pages:Vec<_>=source.pages.iter().enumerate().map(|(i,p)|json!({"id":format!("p{i}"),"sourceId":source.id,"pageIndex":i,"width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})).collect();
    let request:ExportRequest=serde_json::from_value(json!({"pages":pages})).unwrap();
    let output=dir.path().join("saved.pdf");
    assert!(engine.export_pdf_controlled(request.clone(),None,&output,Arc::new(PdfSaveControl::new()),Arc::new(|_|{})).unwrap());
    assert!(engine.open_document(&output).is_err());
    let reader=engine.open_document_with_password(&output,"reader").unwrap();
    let owner=engine.open_document_with_password(&output,"owner").unwrap();
    let actual=serde_json::to_value(&reader.protection).unwrap();let expected=serde_json::to_value(&source.protection).unwrap();
    for key in ["owner","permissions","revision"] { assert_eq!(actual[key],expected[key],"{key}"); }
    assert!(owner.protection.as_ref().unwrap().owner);
    assert_eq!(engine.render_page(&source.id,0,300).unwrap(),engine.render_page(&reader.id,0,300).unwrap());
    let mut denied=request.clone();denied.pages[0].rotation=90;
    let target=dir.path().join("denied.pdf");fs::write(&target,b"keep").unwrap();
    assert!(engine.export_pdf_controlled(denied,None,&target,Arc::new(PdfSaveControl::new()),Arc::new(|_|{})).is_err());
    assert_eq!(fs::read(target).unwrap(),b"keep");
    assert!(engine.export_pdf_controlled(request,None,&input,Arc::new(PdfSaveControl::new()),Arc::new(|_|{})).is_err());
    assert_eq!(fs::read(input).unwrap(),original);
}

#[cfg(windows)]
#[test]
fn locked_controlled_destination_reports_failure_and_preserves_bytes(){
    use std::os::windows::fs::OpenOptionsExt;
    let engine=engine();let dir=tempfile::tempdir().unwrap();let(_,request,_)=setup(&engine,dir.path());
    let path=dir.path().join("locked.pdf");fs::write(&path,b"original destination").unwrap();
    let lock=fs::OpenOptions::new().read(true).share_mode(0).open(&path).unwrap();
    assert!(engine.export_pdf_controlled(request,None,&path,Arc::new(PdfSaveControl::new()),Arc::new(|_|{})).is_err());
    drop(lock);assert_eq!(fs::read(path).unwrap(),b"original destination");
    assert!(fs::read_dir(dir.path()).unwrap().all(|e|!e.unwrap().file_name().to_string_lossy().starts_with(".folio-export-")));
}

#[test]
fn cancellation_stops_actual_portable_overlay_finalization(){
    let engine=engine();let dir=tempfile::tempdir().unwrap();let(_,mut request,original)=setup(&engine,dir.path());
    for page in &mut request.pages {
        page.overlays=serde_json::from_value(json!([
            {"type":"text","id":"first","x":20,"y":30,"text":"First overlay","fontSize":12,"fontName":"Helvetica","color":"#123456","rotation":0},
            {"type":"text","id":"second","x":20,"y":60,"text":"Second overlay","fontSize":12,"fontName":"Helvetica","color":"#123456","rotation":0}
        ])).unwrap();
    }
    for flatten in [false,true] {
        request.flatten=flatten;let path=dir.path().join(format!("overlays-{flatten}.pdf"));fs::write(&path,b"keep destination").unwrap();
        let control=Arc::new(PdfSaveControl::new());let cancel=control.clone();
        let seen=Arc::new(Mutex::new(Vec::new()));let observed=seen.clone();
        let report=Arc::new(move|event:PdfSaveProgress|{if event.stage=="renderingEdits"{observed.lock().unwrap().push(event.completed);if event.completed==32{assert!(cancel.cancel());}}});
        assert!(!engine.export_pdf_controlled(request.clone(),None,&path,control,report).unwrap(),"{flatten}");
        assert_eq!(*seen.lock().unwrap(),vec![0,32]);
        assert_eq!(fs::read(path).unwrap(),b"keep destination");assert_eq!(fs::read(dir.path().join("source.pdf")).unwrap(),original);
    }
}
