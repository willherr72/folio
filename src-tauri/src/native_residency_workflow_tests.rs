//! Replacement/recovery routes must reconstruct their own final source state.
use super::*;

fn assert_replay(rt: &mut WorkerRuntime, info: &DocumentInfo, removed: &[usize], remaining: usize) {
    let source=rt.document(&info.id).unwrap();
    assert_eq!(source.imported_annotations[&1],removed.iter().copied().collect());
    let snapshot=source.imported_overlay_sources.snapshot().unwrap();
    let bytes=source.bytes().unwrap();
    let text=(0..2).map(|p|serde_json::to_value(rt.page_text(&info.id,p).unwrap()).unwrap()).collect::<Vec<_>>();
    let pixels=(0..2).map(|p|render(rt,&info.id,p).unwrap()).collect::<Vec<_>>();
    evict(rt,&info.id);
    for page in 0..2 {
        assert_eq!(render(rt,&info.id,page).unwrap(),pixels[page]);
        assert_eq!(serde_json::to_value(rt.page_text(&info.id,page).unwrap()).unwrap(),text[page]);
    }
    let source=rt.document(&info.id).unwrap();
    assert!(Arc::ptr_eq(&source.bytes().unwrap(),&bytes));
    assert_eq!(source.imported_overlay_sources.snapshot().unwrap(),snapshot);
    assert_eq!(source.pdf().unwrap().pages().get(1).unwrap().annotations().len() as usize,remaining);
}

#[test]
fn native_residency_comment_edit_replacement_keeps_imports_and_answer_after_eviction() {
    let mut rt=runtime();let original=opened(&mut rt);
    let (reply,receive)=mpsc::channel();
    request(&mut rt,WorkerRequest::EditNativeComment{source_id:original.id.clone(),request:native_comments::EditNativeCommentRequest{
        page_index:1,annotation_index:1,expected_text:"keep native".into(),text:"Edited resident comment".into(),position:None,color:None},reply});
    let edited=receive.recv().unwrap().unwrap();
    assert_ne!(edited.id,original.id);
    assert_replay(&mut rt,&edited,&[0,2],1);
    let notes=rt.list_native_comments(&edited.id).unwrap();assert_eq!(notes.len(),1);assert_eq!(notes[0].text,"Edited resident comment");
    assert_eq!(rt.list_native_comments(&original.id).unwrap()[0].text,"keep native");
}

#[test]
fn native_residency_comment_deletion_replays_shifted_popup_index() {
    let mut rt=runtime();let original=opened(&mut rt);
    let (reply,receive)=mpsc::channel();
    request(&mut rt,WorkerRequest::DeleteNativeComment{source_id:original.id.clone(),request:native_comments::DeleteNativeCommentRequest{
        page_index:1,annotation_index:1,expected_text:"keep native".into()},reply});
    let edited=receive.recv().unwrap().unwrap();
    // Deleting slot 1 moves the owned popup from slot 2 to slot 1.
    assert_replay(&mut rt,&edited,&[0,1],0);
    assert!(rt.list_native_comments(&edited.id).unwrap().is_empty());
    assert_eq!(rt.list_native_comments(&original.id).unwrap().len(),1);
}

fn form_fixture() -> Vec<u8> {
    let mut pdf=Document::load_mem(&fixture()).unwrap();
    let second=*pdf.get_pages().get(&2).unwrap();
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let appearance=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),180.into(),30.into()],
        "Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}},b"0 0 180 30 re S BT /F1 12 Tf 5 10 Td (Old answer) Tj ET".to_vec()));
    let widget=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>Object::string_literal("answer"),"V"=>Object::string_literal("Old answer"),
        "P"=>second,"Rect"=>vec![40.into(),100.into(),220.into(),130.into()],"F"=>4,"AP"=>dictionary!{"N"=>appearance}});
    pdf.get_dictionary_mut(second).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().insert(0,widget.into());
    pdf.catalog_mut().unwrap().set("AcroForm",dictionary!{"Fields"=>vec![Object::Reference(widget)],"DA"=>Object::string_literal("/F1 12 Tf 0 g"),"DR"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}});
    crate::pdf_output::save(&mut pdf).unwrap()
}

#[test]
fn native_residency_form_fill_replacement_keeps_value_and_imported_popup() {
    let mut rt=runtime();let path=std::env::temp_dir().join("resident-fill.pdf");
    let original=rt.open_bytes(form_fixture(),path.clone(),path,true,true,false,OpenSecurity::Authenticated(None)).unwrap();
    let wanted=crate::FormFieldValue::Text{text:"New answer".into()};
    let (reply,receive)=mpsc::channel();
    request(&mut rt,WorkerRequest::FillFormField{source_id:original.id.clone(),request:crate::FillFormFieldRequest{page_index:1,annotation_index:0,
        expected_value:crate::FormFieldValue::Text{text:"Old answer".into()},value:wanted.clone()},reply});
    let filled=receive.recv().unwrap().unwrap();
    assert_replay(&mut rt,&filled,&[1,3],2);
    let (reply,receive)=mpsc::channel();request(&mut rt,WorkerRequest::ListFormFields{source_id:filled.id.clone(),page_index:1,reply});
    let fields=receive.recv().unwrap().unwrap();assert_eq!(fields.len(),1);assert_eq!(fields[0].value,wanted);
    let (reply,receive)=mpsc::channel();request(&mut rt,WorkerRequest::ListFormFields{source_id:original.id,page_index:1,reply});
    assert_eq!(receive.recv().unwrap().unwrap()[0].value,crate::FormFieldValue::Text{text:"Old answer".into()});
}

#[test]
fn native_residency_recovery_restores_protected_mappings_and_legacy_comment_ownership() {
    for legacy in [false,true] {
        let mut rt=runtime();let path=std::env::temp_dir().join("resident-recovered.pdf");
        let original=rt.open_bytes(fixture(),path.clone(),path.clone(),true,true,legacy,OpenSecurity::Authenticated(None)).unwrap();
        let change:crate::protection::ProtectionChange=serde_json::from_value(serde_json::json!({"mode":"passwords","userPassword":"reader","ownerPassword":"owner","permissions":{"print":true,"printHighQuality":true,"copyContent":false,"editContent":false,"annotate":false,"fillForms":false,"assemble":false}})).unwrap();
        let bytes=rt.document(&original.id).unwrap().bytes().unwrap();
        let unlocked=crate::protection::unlock_pdf(&change.apply(&bytes).unwrap(),Some("reader")).unwrap();
        rt.documents.get_mut(&original.id).unwrap().policy=Some(SourcePolicy::new(unlocked.protection.unwrap(),&original.pages,&Ok(vec![])));
        let source=rt.document(&original.id).unwrap();
        let policy=source.policy.as_ref().unwrap().snapshot(&bytes).unwrap();
        let mapping=source.imported_overlay_sources.snapshot().unwrap();
        let original_info=source.policy.as_ref().unwrap().info();
        let pixels=(0..2).map(|p|render(&mut rt,&original.id,p).unwrap()).collect::<Vec<_>>();
        rt.close_document(&original.id).unwrap();
        let scope=crate::fonts::FontImportScope::new(rt.fonts.clone());
        let (reply,receive)=mpsc::channel();
        request(&mut rt,WorkerRequest::OpenBytes{bytes:bytes.to_vec(),path:path.clone(),original_path:path,register_fonts:true,legacy_comment_import:legacy,
            policy:Some(policy),annotation_sources:Some(mapping.clone()),images:vec![],font_imports:scope.clone(),reply});
        let restored=receive.recv().unwrap().unwrap();
        assert_eq!(restored.pages,original.pages,"Recovered overlay IDs and placement must match");
        evict(&mut rt,&restored.id);
        for page in 0..2 {assert_eq!(render(&mut rt,&restored.id,page).unwrap(),pixels[page]);}
        let source=rt.document(&restored.id).unwrap();
        assert_eq!(source.imported_overlay_sources.snapshot().unwrap(),mapping);
        assert_eq!(source.legacy_comment_import,legacy);
        assert_eq!(source.pdf().unwrap().pages().get(1).unwrap().annotations().len(),if legacy {0}else{1});
        assert_eq!(serde_json::to_value(source.policy.as_ref().unwrap().info()).unwrap(),serde_json::to_value(original_info).unwrap());
        assert!(matches!(rt.extract_text(&restored.id,0),Err(EngineError::PermissionDenied(_))));
        assert!(matches!(rt.editing_document(&restored.id),Err(EngineError::PermissionDenied(_))));
        rt.discard_unpublished_source(&restored.id);drop(scope);
    }
}

#[test]
fn native_residency_recovered_asset_rollback_after_eviction_preserves_independent_claims() {
    let mut rt=runtime();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("recovery-assets.pdf");
    let mut image_bytes=Cursor::new(vec![]);
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(2,2,image::Rgba([95,140,210,180])))
        .write_to(&mut image_bytes,ImageFormat::Png).unwrap();
    let image_bytes=image_bytes.into_inner();let image=rt.images.register(image_bytes.clone()).unwrap();
    let font_bytes=std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap();
    let font=rt.fonts.register(font_bytes.clone()).unwrap();
    let blank=rt.create_blank_document(300.0,400.0).unwrap();
    let export:ExportRequest=serde_json::from_value(serde_json::json!({"flatten":false,"pages":[{"id":"recovery","sourceId":blank.id,"pageIndex":0,"width":300,"height":400,"rotation":0,"overlays":[
        {"id":"image","type":"image","assetId":image.image.id,"x":40,"y":40,"width":50,"height":50,"rotation":0},
        {"id":"text","type":"text","fontId":font.id,"text":"Recover this font","x":40,"y":130,"fontSize":14,"color":"#000000"}]}]})).unwrap();
    rt.export_pdf_with_bookmarks(export,None,path.clone()).unwrap();rt.close_document(&blank.id).unwrap();
    rt.images.release(&image.token).unwrap();rt.fonts.remove(&font.id).unwrap();
    let original=rt.open_document(path.clone(),None,None,true,true).unwrap();
    let source=rt.document(&original.id).unwrap();let bytes=source.bytes().unwrap();
    let mapping=source.imported_overlay_sources.snapshot().unwrap();
    let pixels=render(&mut rt,&original.id,0).unwrap();
    rt.close_document(&original.id).unwrap();rt.fonts.remove(&font.id).unwrap();
    assert!(rt.images.get(&image.image.id).is_err());
    for claim in ["none","before","after"] {
        let mut external_image=None;
        if claim=="before" {
            rt.fonts.register(font_bytes.clone()).unwrap();
            external_image=Some(rt.images.register(image_bytes.clone()).unwrap());
        }
        let scope=crate::fonts::FontImportScope::new(rt.fonts.clone());let mut sources=vec![];
        for _ in 0..2 {
            let (reply,receive)=mpsc::channel();
            request(&mut rt,WorkerRequest::OpenBytes{bytes:bytes.to_vec(),path:path.clone(),original_path:path.clone(),register_fonts:true,legacy_comment_import:false,
                policy:Some(RecoveryPolicy::Unprotected),annotation_sources:Some(mapping.clone()),
                images:vec![Arc::new(crate::ImageAsset::parse(image_bytes.clone()).unwrap())],font_imports:scope.clone(),reply});
            let restored=receive.recv().unwrap().unwrap();
            assert_eq!(restored.pages,original.pages);
            evict(&mut rt,&restored.id);
            assert_eq!(render(&mut rt,&restored.id,0).unwrap(),pixels);
            assert_eq!(rt.images.get(&image.image.id).unwrap().bytes(),image_bytes);
            assert!(rt.fonts.get(&font.id).is_ok());
            evict(&mut rt,&restored.id);sources.push(restored);
        }
        if claim=="after" {
            rt.fonts.register(font_bytes.clone()).unwrap();
            external_image=Some(rt.images.register(image_bytes.clone()).unwrap());
        }
        for source in sources {
            let id=source.id.clone();let (reply,receive)=mpsc::channel();drop(receive);
            rt.reply_document(reply,Ok(source));
            assert!(rt.document(&id).is_err());
        }
        drop(scope);
        assert_eq!(rt.fonts.get(&font.id).is_ok(),claim!="none","font claim {claim}");
        assert_eq!(rt.images.get(&image.image.id).is_ok(),claim!="none","image claim {claim}");
        if let Some(external)=external_image {rt.images.release(&external.token).unwrap();}
        assert!(rt.images.get(&image.image.id).is_err());
        rt.fonts.remove(&font.id).unwrap();
    }
}
