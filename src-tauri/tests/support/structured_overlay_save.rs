use super::*;
use folio_engine::Overlay;
#[path="structured_overlay_compatibility.rs"]
mod compatibility;

fn same_pixels(a:&[u8],b:&[u8],context:&str) {
    let a=image::load_from_memory(a).unwrap().to_rgba8();let b=image::load_from_memory(b).unwrap().to_rgba8();
    assert_eq!(a.dimensions(),b.dimensions(),"{context}");
    if a!=b {if let Some(root)=std::env::var_os("FOLIO_STRUCTURED_OVERLAY_CAPTURE_DIR") {
        let root=PathBuf::from(root);fs::create_dir_all(&root).unwrap();let label=context.chars().map(|c|if c.is_ascii_alphanumeric(){c}else{'-'}).collect::<String>();
        a.save(root.join(format!("{label}-actual.png"))).unwrap();b.save(root.join(format!("{label}-expected.png"))).unwrap();
    }}
    assert_eq!(a.as_raw().iter().zip(b.as_raw()).filter(|(a,b)|a!=b).count(),0,"{context}: differing channels");
}
fn overlays(image:&str)->Vec<Overlay>{serde_json::from_value(serde_json::json!([
    {"type":"text","id":"text","x":30,"y":90,"text":"Added note","fontSize":14,"fontName":"Helvetica","color":"#223344","rotation":0},
    {"type":"ink","id":"ink","paths":[[{"x":30,"y":90},{"x":150,"y":120}]],"color":"#bb2244","strokeWidth":2},
    {"type":"highlight","id":"highlight","rects":[{"x":30,"y":80,"width":130,"height":25}],"color":"#ffff00","opacity":0.35,"text":"Marked words"},
    {"type":"image","id":"image","assetId":image,"x":50,"y":90,"width":70,"height":45,"rotation":0},
    {"type":"comment","id":"comment","x":180,"y":110,"text":"Added review note","color":"#ffcc00"}
])).unwrap()}
fn capture(name:&str,path:&Path){
    capture_bytes(name,&fs::read(path).unwrap());
}
fn capture_bytes(name:&str,bytes:&[u8]){
    if let Some(root)=std::env::var_os("FOLIO_STRUCTURED_OVERLAY_CAPTURE_DIR"){
        use std::io::Write;let root=PathBuf::from(root);fs::create_dir_all(&root).unwrap();
        fs::OpenOptions::new().write(true).create_new(true).open(root.join(name)).unwrap().write_all(bytes).unwrap();
    }
}
fn drawing_geometry(document:&Document,page:u32)->Vec<(String,Object,Option<Vec<u8>>)> {
    let page=document.get_dictionary(document.get_pages()[&page]).unwrap();
    page.get(b"Annots").unwrap().as_array().unwrap().iter().filter_map(|v|{
        let a=document.get_dictionary(v.as_reference().unwrap()).unwrap();
        let name=a.get(b"NM").ok().and_then(|v|lopdf::decode_text_string(v).ok())?;
        if !["text","ink","image","highlight"].contains(&name.as_str()){return None}
        let appearance=a.get(b"AP").ok().and_then(|v|v.as_dict().ok()).and_then(|v|v.get(b"N").ok()).and_then(|v|v.as_reference().ok()).map(|id|document.objects[&id].as_stream().unwrap().content.clone());
        Some((name,a.get(b"Rect").unwrap().clone(),appearance))
    }).collect()
}

#[test]
fn structured_overlay_save_added_drawings_reopen_and_edit_only_one_copy(){run(false);}
#[test]
fn structured_overlay_save_flattened_drawings_keep_original_tags_and_notes(){run(true);}
fn run(flatten:bool){
    let e=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("tagged.pdf");let b=dir.path().join("plain.pdf");
    let before_a=fixture(&a,true,"Tagged");let before_b=fixture(&b,false,"Plain");
    let first=e.open_document(&a).unwrap();let second=e.open_document(&b).unwrap();let base=request(&[&first,&second]);
    let asset_path=dir.path().join("image.png");image::RgbaImage::from_pixel(3,2,image::Rgba([20,80,150,170])).save(&asset_path).unwrap();let asset=e.import_image(&asset_path).unwrap();
    let mut plan=ExportRequest{flatten,pages:[0,0,1,2,3].into_iter().enumerate().map(|(i,old)|{let mut p=base.pages[old].clone();p.id=format!("occurrence-{i}");p}).collect()};
    for p in &mut plan.pages[..2]{p.overlays=overlays(&asset.image.id);}
    let expected=plan.pages.iter().map(|p|e.render_page_plan(p.clone(),500).unwrap()).collect::<Vec<_>>();
    let output=dir.path().join("combined.pdf");fs::write(&output,b"old destination").unwrap();
    e.export_pdf(plan.clone(),&output).expect("structured public Save must support source-bound drawings");
    let reopened=e.open_document(&output).unwrap();assert_eq!(reopened.pages.len(),5);
    let saved=Document::load(&output).unwrap();check_structure(&saved,15);
    capture(&format!("public-{flatten}-added.pdf"),&output);
    capture(&format!("public-{flatten}-source-tagged.pdf"),&a);capture(&format!("public-{flatten}-source-plain.pdf"),&b);
    let reopened_plan=request(&[&reopened]);
    for (i,expected) in expected.iter().enumerate(){
        // Editable opening removes owned drawings from the background and
        // returns them as drafts. Compare the complete canvas, not that layer.
        same_pixels(&e.render_page_plan(reopened_plan.pages[i].clone(),500).unwrap(),expected,&format!("saved page {i}, flatten={flatten}"));
        capture_bytes(&format!("public-{flatten}-added-{i}-expected.png"),expected);
    }
    let pages=saved.get_pages();let first_annotations=saved.get_dictionary(pages[&1]).unwrap().get(b"Annots").unwrap().as_array().unwrap();
    let second_annotations=saved.get_dictionary(pages[&2]).unwrap().get(b"Annots").unwrap().as_array().unwrap();
    assert!(first_annotations.iter().all(|id|!second_annotations.contains(id)),"copied pages must own separate annotations");
    assert_eq!(first_annotations.len(),if flatten{4}else{7});
    assert_eq!(reopened.pages[0].overlays.len(),if flatten{1}else{4});
    if !flatten{
        let mut edit=request(&[&reopened]);
        let original_copy=e.render_page_plan(edit.pages[0].clone(),500).unwrap();
        let text=edit.pages[1].overlays.iter_mut().find_map(|o|if let Overlay::Text(t)=o{Some(t)}else{None}).unwrap();text.text="Changed on this copy".into();
        edit.pages[1].overlays.retain(|o|!matches!(o,Overlay::Ink(_)));
        let expected=e.render_page_plan(edit.pages[1].clone(),500).unwrap();let changed=dir.path().join("changed.pdf");
        e.export_pdf(edit,&changed).unwrap();let again=e.open_document(&changed).unwrap();
        capture("public-false-changed.pdf",&changed);
        assert_eq!(drawing_geometry(&Document::load(&changed).unwrap(),1),drawing_geometry(&saved,1),"physical-unit round trip must not regenerate untouched source drawings");
        let again_plan=request(&[&again]);
        same_pixels(&e.render_page_plan(again_plan.pages[0].clone(),500).unwrap(),&original_copy,"untouched copy");
        same_pixels(&e.render_page_plan(again_plan.pages[1].clone(),500).unwrap(),&expected,"changed copy");
        capture_bytes("public-false-changed-0-expected.png",&original_copy);capture_bytes("public-false-changed-1-expected.png",&expected);
        assert_eq!(again.pages[0].overlays.len(),4);assert_eq!(again.pages[1].overlays.len(),3);
        assert!(again.pages[1].overlays.iter().any(|o|matches!(o,Overlay::Text(t) if t.text=="Changed on this copy")));
        let mut remove=request(&[&again]);remove.pages[1].overlays.clear();let expected=e.render_page_plan(remove.pages[1].clone(),500).unwrap();let removed=dir.path().join("removed.pdf");
        e.export_pdf(remove,&removed).unwrap();let last=e.open_document(&removed).unwrap();assert!(last.pages[1].overlays.is_empty());
        let last_plan=request(&[&last]);
        same_pixels(&e.render_page_plan(last_plan.pages[1].clone(),500).unwrap(),&expected,"deleted drawings");
        same_pixels(&e.render_page_plan(last_plan.pages[0].clone(),500).unwrap(),&original_copy,"untouched copy after deletion");
        capture_bytes("public-false-removed-0-expected.png",&original_copy);capture_bytes("public-false-removed-1-expected.png",&expected);
        capture("public-false-removed.pdf",&removed);
    }
    assert_eq!(fs::read(a).unwrap(),before_a);assert_eq!(fs::read(b).unwrap(),before_b);
}

fn drawing_asset(e:&PdfEngine,dir:&Path)->String {
    let path=dir.join("drawings.png");
    image::RgbaImage::from_pixel(3,2,image::Rgba([20,80,150,170])).save(&path).unwrap();
    e.import_image(path).unwrap().image.id
}

#[test]
fn structured_overlay_save_cancel_preserves_destination_with_added_and_imported_drawings() {
    let e=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("tagged.pdf");let b=dir.path().join("plain.pdf");
    let original_a=fixture(&a,true,"Tagged");let original_b=fixture(&b,false,"Plain");
    let first=e.open_document(&a).unwrap();let other=e.open_document(&b).unwrap();
    let mut added=request(&[&first,&other]);added.pages[0].overlays=overlays(&drawing_asset(&e,dir.path()));
    let baseline=dir.path().join("imported.pdf");e.export_pdf(added.clone(),&baseline).unwrap();
    let baseline_bytes=fs::read(&baseline).unwrap();let imported=e.open_document(&baseline).unwrap();
    let mut changed=request(&[&imported]);
    changed.pages[0].overlays.retain(|o|!matches!(o,Overlay::Ink(_)));
    let text=changed.pages[0].overlays.iter_mut().find_map(|o|if let Overlay::Text(v)=o{Some(v)}else{None}).unwrap();text.text="Changed before cancellation".into();
    for (kind,plan) in [("added",added),("imported",changed)] {for flatten in [false,true] {for stage in ["checking","copying","preserving","applying","preparingFile"] {
        let mut plan=plan.clone();plan.flatten=flatten;
        let output=dir.path().join(format!("cancel-{kind}-{flatten}-{stage}.pdf"));fs::write(&output,b"keep destination").unwrap();
        let control=Arc::new(PdfSaveControl::new());let cancel=control.clone();let observed=Arc::new(AtomicBool::new(false));let seen=observed.clone();
        let report=Arc::new(move|event:PdfSaveProgress|{if event.stage==stage {seen.store(true,Ordering::SeqCst);cancel.cancel();}});
        assert!(!e.export_pdf_controlled(plan,None,&output,control,report).unwrap());
        assert!(observed.load(Ordering::SeqCst),"missing stage {stage}");assert_eq!(fs::read(&output).unwrap(),b"keep destination");
    }}}
    assert_eq!(fs::read(a).unwrap(),original_a);assert_eq!(fs::read(b).unwrap(),original_b);assert_eq!(fs::read(baseline).unwrap(),baseline_bytes);
}

#[test]
fn structured_overlay_save_preserves_protection_and_rejects_reader_edits() {
    let e=engine();let dir=tempfile::tempdir().unwrap();let plain=dir.path().join("plain.pdf");let original=fixture(&plain,true,"Protected");
    let encrypted=protected_fixture::encrypt(&original,lopdf::encryption::Permissions::PRINTABLE);let input=dir.path().join("protected.pdf");fs::write(&input,&encrypted).unwrap();
    let other=e.open_document(&plain).unwrap();let reader=e.open_document_with_password(&input,"reader").unwrap();let owner=e.open_document_with_password(&input,"owner").unwrap();
    let drawings=overlays(&drawing_asset(&e,dir.path()));
    for flatten in [false,true] {
        let output=dir.path().join(format!("protected-{flatten}.pdf"));fs::write(&output,b"keep destination").unwrap();
        let mut plan=request(&[&reader,&other]);plan.flatten=flatten;plan.pages[0].overlays=drawings.clone();
        assert!(e.export_pdf(plan,&output).is_err());assert_eq!(fs::read(&output).unwrap(),b"keep destination");
        let mut plan=request(&[&owner,&other]);plan.flatten=flatten;plan.pages[0].overlays=drawings.clone();
        let expected=e.render_page_plan(plan.pages[0].clone(),500).unwrap();e.export_pdf(plan,&output).unwrap();
        assert!(e.open_document(&output).is_err());let saved=e.open_document_with_password(&output,"owner").unwrap();
        assert_eq!(saved.protection.as_ref().unwrap().permissions,owner.protection.as_ref().unwrap().permissions);
        same_pixels(&e.render_page_plan(request(&[&saved]).pages[0].clone(),500).unwrap(),&expected,"protected overlay page");
        let restricted=e.open_document_with_password(&output,"reader").unwrap();assert!(!restricted.protection.as_ref().unwrap().permissions.assemble);
    }
    assert_eq!(fs::read(input).unwrap(),encrypted);assert_eq!(fs::read(plain).unwrap(),original);
}

#[test]
fn structured_overlay_save_forms_navigation_recovery_and_png_share_visible_state() {
    use folio_engine::{RecoveryStore,PageImageExportRequest,PageImageOptions,PageImageFormat,PageImageControl,FillFormFieldRequest,FormFieldValue};
    let e=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("form.pdf");let b=dir.path().join("plain.pdf");
    let original_a=super::form_save::tagged_form(&a);let original_b=fixture(&b,false,"Plain");
    let form=e.open_document(&a).unwrap();let other=e.open_document(&b).unwrap();
    let mut plan=request(&[&form,&other]);let mut copy=plan.pages[0].clone();copy.id="copy".into();plan.pages.insert(1,copy);
    plan.pages[1].overlays=overlays(&drawing_asset(&e,dir.path()));
    let bookmark=folio_engine::DocumentBookmark{id:"copy-link".into(),title:"Copied form with drawings".into(),level:0,destination:Some(folio_engine::PdfValue::Array(vec![folio_engine::PdfValue::Page("copy".into()),folio_engine::PdfValue::Name(b"Fit".to_vec())])),action:None,bold:true,italic:false,closed:false,color:None};
    let saved_path=dir.path().join("saved.pdf");e.export_pdf_with_bookmarks(plan,Some(vec![bookmark]),&saved_path).unwrap();
    let saved=e.open_document(&saved_path).unwrap();let saved_bytes=fs::read(&saved_path).unwrap();
    let plan=request(&[&saved]);let expected=e.render_page_plan(plan.pages[1].clone(),680).unwrap();
    let result=e.export_page_images(PageImageExportRequest{pages:vec![plan.pages[1].clone()],page_numbers:vec![2],workspace_page_count:5,options:PageImageOptions{format:PageImageFormat::Png,dpi:72,jpeg_quality:90}},dir.path(),Arc::new(PageImageControl::new()),Arc::new(|_|{})).unwrap().unwrap();
    same_pixels(&fs::read(result.folder.join("page-0002.png")).unwrap(),&expected,"PNG export and full preview");
    let store=RecoveryStore::new(dir.path().join("recovery")).unwrap();
    // The workspace remaps source page numbers into its own stable page IDs.
    let mut bookmarks=saved.bookmarks.clone();
    bookmarks[0].destination=Some(folio_engine::PdfValue::Array(vec![folio_engine::PdfValue::Page(plan.pages[1].id.clone()),folio_engine::PdfValue::Name(b"Fit".to_vec())]));
    let workspace=serde_json::json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","savedDigest":"old","dirty":true,"zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"Saved.pdf","selectedPageId":plan.pages[1].id,"selectedOverlayId":null,"bookmarks":bookmarks,"pages":plan.pages}}]});
    store.save(&e,workspace).unwrap();e.close_document(&saved.id).unwrap();
    let restored=store.load(&e).unwrap().unwrap();let pages:Vec<PagePlan>=serde_json::from_value(restored["tabs"][0]["document"]["pages"].clone()).unwrap();
    same_pixels(&e.render_page_plan(pages[1].clone(),680).unwrap(),&expected,"recovered full preview");
    let recovered_path=dir.path().join("recovered.pdf");e.export_pdf(ExportRequest{flatten:false,pages},&recovered_path).unwrap();
    let recovered=e.open_document(&recovered_path).unwrap();let doc=Document::load(&recovered_path).unwrap();check_structure(&doc,12);
    assert_eq!(drawing_geometry(&doc,2),drawing_geometry(&Document::load_mem(&saved_bytes).unwrap(),2));
    assert_eq!(recovered.bookmarks[0].title,"Copied form with drawings");
    assert_eq!(recovered.bookmarks[0].destination,Some(folio_engine::PdfValue::Array(vec![folio_engine::PdfValue::Page("1".into()),folio_engine::PdfValue::Name(b"Fit".to_vec())])));
    for page in [0,1] {let fields=e.list_form_fields(&recovered.id,page).unwrap();assert_eq!(fields.len(),1);assert_eq!(fields[0].value,FormFieldValue::Text{text:"Original answer".into()});}
    let field=e.list_form_fields(&recovered.id,1).unwrap().remove(0);let value=FormFieldValue::Text{text:"Shared answer after recovery".into()};
    let filled=e.fill_form_field(&recovered.id,FillFormFieldRequest{page_index:1,annotation_index:field.annotation_index,expected_value:field.value,value:value.clone()}).unwrap();
    for page in [0,1] {assert_eq!(e.list_form_fields(&filled.id,page).unwrap()[0].value,value);}
    // Form fill returns a new background source. Like commitFormReplacement,
    // keep the workspace drafts instead of treating an empty reply as deletion.
    let mut filled_plan=request(&[&recovered]);
    for page in &mut filled_plan.pages {page.source_id=filled.id.clone();}
    let final_path=dir.path().join("filled.pdf");e.export_pdf(filled_plan,&final_path).unwrap();let final_doc=e.open_document(&final_path).unwrap();
    assert_eq!(final_doc.pages[1].overlays.len(),4);assert_eq!(e.list_form_fields(&final_doc.id,0).unwrap()[0].value,value);
    assert_eq!(fs::read(&a).unwrap(),original_a);assert_eq!(fs::read(&b).unwrap(),original_b);assert_eq!(fs::read(&saved_path).unwrap(),saved_bytes);
}

