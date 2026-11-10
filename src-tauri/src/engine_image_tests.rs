use super::*;
use image::{DynamicImage, Rgba, RgbaImage};

fn image_file(path: &Path, value: u8) -> Vec<u8> {
    let mut out=Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(2,2,Rgba([value,40,80,128])))
        .write_to(&mut out,ImageFormat::Png).unwrap();
    let bytes=out.into_inner();std::fs::write(path,&bytes).unwrap();bytes
}
fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()
}
#[test]
fn internal_image_composition_omits_original_attachments_but_save_preserves_them() {
    let root=PathBuf::from(env!("CARGO_MANIFEST_DIR"));let mut runtime=WorkerRuntime::new(&root.join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap();
    let temp=tempfile::tempdir().unwrap();let bytes=image_file(&temp.path().join("image.png"),218);
    let image=runtime.images.register(bytes).unwrap();let source=runtime.create_blank_document(200.,300.).unwrap();
    let request=ExportRequest{flatten:false,pages:vec![PagePlan{crop: None,id:"page".into(),source_id:source.id.clone(),page_index:0,width:200.,height:300.,rotation:0,overlays:vec![Overlay::Image(ImageOverlay{id:"image".into(),asset_id:image.image.id.clone(),x:60.,y:30.,width:80.,height:40.,rotation:90})]}]};
    for intent in [CompositionIntent::Save,CompositionIntent::Preview,CompositionIntent::Print] {
        let bytes=runtime.compose_pdf(request.clone(),Some(vec![]),intent).unwrap();let pdf=lopdf::Document::load_mem(&bytes).unwrap();
        let attachments=pdf.objects.values().filter(|object|object.as_stream().is_ok_and(|stream|stream.dict.has(b"FolioImageId"))).count();
        assert_eq!(attachments,usize::from(matches!(intent,CompositionIntent::Save)));
        assert!(pdf.objects.values().any(|object|object.as_stream().is_ok_and(|stream|stream.dict.get(b"Subtype").is_ok_and(|value|value.as_name().is_ok_and(|name|name==b"Image")))));
    }
}

#[test]
fn image_import_validates_appearance_before_acquisition_and_keeps_independent_owners() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("image.png");
    let original=image_file(&path,219);
    let first=engine.import_image(&path).unwrap();let second=engine.import_image(&path).unwrap();
    assert_eq!(first.image,second.image);assert_ne!(first.token,second.token);
    engine.images().release(&first.token).unwrap();
    assert_eq!(engine.images().get(&second.image.id).unwrap().bytes(),original);
    let payload=b"profile\0\0bad";
    let mut chunk=(payload.len() as u32).to_be_bytes().to_vec();chunk.extend_from_slice(b"iCCP");chunk.extend(payload);
    chunk.extend(crc32fast::hash(&chunk[4..]).to_be_bytes());
    let mut bad=original.clone();bad.splice(33..33,chunk);let bad_path=temp.path().join("bad-profile.png");std::fs::write(&bad_path,&bad).unwrap();
    let bad_id=crate::ImageAsset::parse(bad.clone()).unwrap().info().id.clone();
    assert!(engine.import_image(&bad_path).is_err());assert!(engine.images().get(&bad_id).is_err());
    assert_eq!(std::fs::read(path).unwrap(),original);assert_eq!(std::fs::read(bad_path).unwrap(),bad);
    engine.images().release(&second.token).unwrap();assert!(engine.images().get(&second.image.id).is_err());
}

#[test]
fn abandoned_file_and_snapshot_opens_release_their_image_leases() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("image.png");
    image_file(&path,220);let registration=engine.import_image(&path).unwrap();
    let source=engine.create_blank_document(200.,300.).unwrap();let saved=temp.path().join("image.pdf");
    let page=PagePlan{crop: None,id:"page".into(),source_id:source.id.clone(),page_index:0,width:200.,height:300.,rotation:0,
        overlays:vec![Overlay::Image(ImageOverlay{id:"image".into(),asset_id:registration.image.id.clone(),x:50.,y:50.,width:40.,height:20.,rotation:0})]};
    engine.export_pdf(ExportRequest{pages:vec![page],flatten:false},&saved).unwrap();
    engine.images().release(&registration.token).unwrap();
    for snapshot in [false,true] {
        let (reply,receive)=mpsc::channel();drop(receive);
        let request=if snapshot {WorkerRequest::OpenBytes{annotation_sources:None,legacy_comment_import:false,bytes:std::fs::read(&saved).unwrap(),path:saved.clone(),original_path:saved.clone(),register_fonts:true,policy:Some(RecoveryPolicy::Unprotected),images:vec![],font_imports:crate::fonts::FontImportScope::new(engine.fonts()),reply}}
            else {WorkerRequest::Open{path:saved.clone(),password:None,original_path:None,editable_annotations:true,register_fonts:true,reply}};
        engine.send(request).unwrap();engine.status().unwrap(); // FIFO barrier after the abandoned open.
        assert!(engine.images().get(&registration.image.id).is_err(),"snapshot={snapshot}: abandoned open retained image");
    }
    engine.close_document(&source.id).unwrap();
}

#[test]
fn abandoned_recovery_opens_release_repeated_font_imports_without_revoking_external_claims() {
    let root=PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut runtime=WorkerRuntime::new(&root.join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap();
    let bytes=std::fs::read(root.join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap();
    let font=runtime.fonts.register(bytes.clone()).unwrap();
    let source=runtime.create_blank_document(200.,200.).unwrap();
    let temp=tempfile::tempdir().unwrap();let path=temp.path().join("font.pdf");
    let request=serde_json::from_value(serde_json::json!({"pages":[{"id":"p","sourceId":source.id,
        "pageIndex":0,"width":200,"height":200,"rotation":0,"overlays":[{"id":"t","type":"text",
        "fontId":font.id,"text":"Draft","x":30,"y":40,"fontSize":12,"color":"#000000"}]}],"flatten":false})).unwrap();
    runtime.export_pdf_with_bookmarks(request,None,path.clone()).unwrap();
    runtime.close_document(&source.id).unwrap();runtime.fonts.remove(&font.id).unwrap();
    let pdf=std::fs::read(&path).unwrap();
    for claim in ["none","before","after"] {
        if claim=="before" {runtime.fonts.register(bytes.clone()).unwrap();}
        let scope=crate::fonts::FontImportScope::new(runtime.fonts.clone());
        let mut opened=Vec::new();
        for _ in 0..2 {
            opened.push(runtime.open_bytes_scoped(pdf.clone(),path.clone(),path.clone(),true,true,false,
                OpenSecurity::Authenticated(None),Some(&scope)).unwrap());
        }
        assert!(runtime.fonts.get(&font.id).is_ok());
        if claim=="after" {runtime.fonts.register(bytes.clone()).unwrap();}
        for document in opened {
            let (reply,receive)=mpsc::channel();drop(receive);
            runtime.reply_document(reply,Ok(document));
        }
        assert!(runtime.documents.is_empty());
        drop(scope);
        assert_eq!(runtime.fonts.get(&font.id).is_ok(),claim!="none","external claim: {claim}");
        runtime.fonts.remove(&font.id).unwrap();
    }
}
