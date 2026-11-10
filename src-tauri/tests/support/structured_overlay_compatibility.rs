use super::*;

fn workspace(plan:&ExportRequest)->serde_json::Value {
    serde_json::json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","savedDigest":"old","dirty":true,"zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"Drawings.pdf","selectedPageId":plan.pages[0].id,"selectedOverlayId":null,"pages":plan.pages}}]})
}

#[test]
fn structured_overlay_save_legacy_recovery_preserves_drafts_and_refuses_unproved_publication() {
    use folio_engine::{RecoveryStore,private_storage::{seal,unseal,Purpose}};
    let e=engine();let dir=tempfile::tempdir().unwrap();let original=dir.path().join("source.pdf");
    let original_bytes=fixture(&original,true,"Legacy source");let source=e.open_document(&original).unwrap();
    let mut plan=request(&[&source]);plan.pages[0].overlays=overlays(&drawing_asset(&e,dir.path()));
    let saved_path=dir.path().join("saved.pdf");e.export_pdf(plan,&saved_path).unwrap();
    let saved_bytes=fs::read(&saved_path).unwrap();let saved=e.open_document(&saved_path).unwrap();
    let mut plan=request(&[&saved]);
    let text=plan.pages[0].overlays.iter_mut().find_map(|o|if let Overlay::Text(v)=o{Some(v)}else{None}).unwrap();text.text="Unsaved legacy draft".into();
    plan.pages[0].overlays.retain(|o|!matches!(o,Overlay::Ink(_)));
    let mut copy=plan.pages[0].clone();copy.id="legacy-copy".into();plan.pages.push(copy);
    let before=workspace(&plan);let store_root=dir.path().join("recovery");let store=RecoveryStore::new(store_root.clone()).unwrap();
    store.save(&e,before.clone()).unwrap();
    let manifest_path=fs::read_dir(&store_root).unwrap().map(|p|p.unwrap().path()).find(|p|p.extension().is_some_and(|s|s=="json")).unwrap();
    let mut manifest:serde_json::Value=serde_json::from_slice(&unseal(Purpose::Workspace,&fs::read(&manifest_path).unwrap(),32*1024*1024).unwrap()).unwrap();
    manifest["annotationImportVersion"]=serde_json::json!(3);
    for source in manifest["sources"].as_object_mut().unwrap().values_mut(){source.as_object_mut().unwrap().remove("annotationSources");}
    fs::write(&manifest_path,seal(Purpose::Workspace,&serde_json::to_vec(&manifest).unwrap()).unwrap()).unwrap();
    e.close_document(&saved.id).unwrap();
    let restored=store.load(&e).unwrap().unwrap();let pages:Vec<PagePlan>=serde_json::from_value(restored["tabs"][0]["document"]["pages"].clone()).unwrap();
    for (actual,old) in pages.iter().zip(&plan.pages){assert_eq!(actual.overlays,old.overlays,"legacy recovery must keep changed/deleted/copied draft decisions");}
    let preview=e.render_page_plan(pages[0].clone(),500).unwrap();assert!(!preview.is_empty());
    for flatten in [false,true] {
        let output=dir.path().join(format!("legacy-refused-{flatten}.pdf"));fs::write(&output,b"keep destination").unwrap();
        let error=e.export_pdf(ExportRequest{flatten,pages:pages.clone()},&output).unwrap_err().to_string();
        assert!(error.contains("source ownership") && error.contains("could not be verified"),"{error}");
        assert_eq!(fs::read(&output).unwrap(),b"keep destination");
        same_pixels(&e.render_page_plan(pages[0].clone(),500).unwrap(),&preview,"legacy draft after refused Save");
    }
    store.save(&e,restored.clone()).unwrap();let again=store.load(&e).unwrap().unwrap();
    assert_eq!(again["tabs"][0]["document"]["pages"][0]["overlays"],restored["tabs"][0]["document"]["pages"][0]["overlays"]);
    assert_eq!(fs::read(&original).unwrap(),original_bytes);assert_eq!(fs::read(&saved_path).unwrap(),saved_bytes);
}

#[test]
fn structured_overlay_save_custom_fonts_crop_language_and_asset_independence() {
    for flatten in [false,true] {
        let e=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("english.pdf");let b=dir.path().join("french.pdf");
        let original_a=super::super::form_save::tagged_form(&a);fixture(&b,true,"French");
        let mut french=Document::load(&b).unwrap();french.catalog_mut().unwrap().set("Lang",Object::string_literal("fr-CA"));
        for page in french.get_pages().values(){french.get_dictionary_mut(*page).unwrap().set("UserUnit",2.75);french.get_dictionary_mut(*page).unwrap().set("Rotate",270);}
        french.save(&b).unwrap();let original_b=fs::read(&b).unwrap();
        let english=e.open_document(&a).unwrap();let french=e.open_document(&b).unwrap();
        let font_path=dir.path().join("source-font.ttf");fs::write(&font_path,include_bytes!("../../../tests/fixtures/shaped-text/DejaVuSans.ttf")).unwrap();
        let font=e.fonts().register(fs::read(&font_path).unwrap()).unwrap();
        let image_path=dir.path().join("drawings.png");
        image::RgbaImage::from_pixel(3,2,image::Rgba([10,70,160,171])).save(&image_path).unwrap();
        let image=e.import_image(&image_path).unwrap();let asset=image.image.id.clone();
        let base=request(&[&english,&french]);let mut plan=ExportRequest{flatten,pages:[0,2,0,3].into_iter().enumerate().map(|(index,source)|{let mut p=base.pages[source].clone();p.id=format!("mixed-{index}");p}).collect()};
        for page in &mut plan.pages[..3] {
            page.overlays=overlays(&asset);
            page.overlays[0]=serde_json::from_value(serde_json::json!({"type":"text","id":"text","x":35,"y":50,"text":"office café\nseconde ligne","fontSize":14,"fontId":font.id,"color":"#223344","rotation":0,"shaping":{"version":1,"direction":"ltr","ligatures":true},"area":{"version":1,"width":220,"height":120,"inset":3,"lineSpacing":1.2,"alignment":"center"}})).unwrap();
        }
        plan.pages[0].crop=Some(folio_engine::AnnotationRect{x:17.,y:34.,width:510.,height:340.});plan.pages[0].rotation=90;
        plan.pages[1].crop=Some(folio_engine::AnnotationRect{x:27.5,y:55.,width:825.,height:550.});plan.pages[1].rotation=180;
        let expected=plan.pages.iter().map(|p|e.render_page_plan(p.clone(),500).unwrap()).collect::<Vec<_>>();
        // Remove only fixture assets. Reopening must use the saved PDF.
        fs::remove_file(&font_path).unwrap();fs::remove_file(dir.path().join("drawings.png")).unwrap();
        let output=dir.path().join("mixed.pdf");e.export_pdf(plan,&output).unwrap();
        capture(&format!("compat-{flatten}-saved.pdf"),&output);
        let doc=Document::load(&output).unwrap();let pages=doc.get_pages();
        let font_files=doc.objects.values().filter_map(|v|v.as_dict().ok()).filter_map(|d|d.get(b"FontFile2").ok()).map(|o|o.as_reference().unwrap()).collect::<std::collections::BTreeSet<_>>();
        assert_eq!(font_files.len(),1,"one shared embedded font must serve the mixed physical pages");
        let mut language_counts=[0;4];
        for value in doc.objects.values() {
            let Ok(d)=value.as_dict() else {continue};
            if !d.get(b"Type").and_then(Object::as_name).is_ok_and(|v|v==b"StructElem") || !d.get(b"S").and_then(Object::as_name).is_ok_and(|v|v==b"Annot") {continue}
            let Ok(kid)=d.get(b"K").and_then(Object::as_dict) else {continue};
            if kid.get(b"Type").and_then(Object::as_name).is_ok_and(|v|v==b"OBJR") {
                let annotation=doc.get_dictionary(kid.get(b"Obj").unwrap().as_reference().unwrap()).unwrap();
                let name=annotation.get(b"NM").ok().and_then(|v|lopdf::decode_text_string(v).ok());
                if !name.as_deref().is_some_and(|s|["text","image","ink","highlight","comment"].contains(&s)){continue}
            } else {assert_eq!(kid.get(b"Type").unwrap().as_name().unwrap(),b"MCR");}
            let page=d.get(b"Pg").unwrap().as_reference().unwrap();let index=pages.values().position(|id|*id==page).unwrap();
            let language=d.get(b"Lang").unwrap().as_str().unwrap();assert_eq!(language,if index==1 {b"fr-CA".as_slice()}else{b"en-US".as_slice()});language_counts[index]+=1;
        }
        assert_eq!(language_counts,[5,5,5,0]);
        capture(&format!("compat-{flatten}-english.pdf"),&a);capture(&format!("compat-{flatten}-french.pdf"),&b);
        e.close_document(&english.id).unwrap();e.close_document(&french.id).unwrap();
        e.fonts().remove(&font.id).unwrap();e.images().release(&image.token).unwrap();
        let fresh=engine();assert!(fresh.fonts().get(&font.id).is_err());
        assert!(fresh.images().get(&asset).is_err());
        let opened=fresh.open_document(&output).unwrap();let reopened=request(&[&opened]);
        for (index,page) in reopened.pages.iter().enumerate() {
            same_pixels(&fresh.render_page_plan(page.clone(),500).unwrap(),&expected[index],&format!("custom font crop page {index} flatten {flatten}"));
            capture_bytes(&format!("compat-{flatten}-{index}-expected.png"),&expected[index]);
        }
        let images=fresh.export_page_images(folio_engine::PageImageExportRequest{
            pages:reopened.pages.clone(),page_numbers:vec![1,2,3,4],workspace_page_count:4,
            options:folio_engine::PageImageOptions{format:folio_engine::PageImageFormat::Png,dpi:72,jpeg_quality:90},
        },dir.path(),Arc::new(folio_engine::PageImageControl::new()),Arc::new(|_|{})).unwrap().unwrap();
        for (index,page) in reopened.pages.iter().enumerate() {
            same_pixels(&fs::read(images.folder.join(format!("page-{:04}.png",index+1))).unwrap(),
                &fresh.render_page_plan(page.clone(),page.width.round() as u32).unwrap(),"cropped custom-font PNG agrees with complete preview");
        }
        if !flatten {
            assert!(fresh.fonts().get(&font.id).is_ok());
            let mut changed=reopened.clone();let text=changed.pages[2].overlays.iter_mut().find_map(|o|if let Overlay::Text(t)=o{Some(t)}else{None}).unwrap();
            assert_eq!(text.text,"office café\nseconde ligne");assert_eq!(text.font_id.as_deref(),Some(font.id.as_str()));assert!(text.area.is_some());
            text.text="office café\nchanged copy".into();let reference=fresh.render_page_plan(changed.pages[2].clone(),500).unwrap();
            let changed_path=dir.path().join("changed.pdf");fresh.export_pdf(changed,&changed_path).unwrap();let next_doc=fresh.open_document(&changed_path).unwrap();let next=request(&[&next_doc]);
            same_pixels(&fresh.render_page_plan(next.pages[0].clone(),500).unwrap(),&expected[0],"custom-font unchanged copy");
            same_pixels(&fresh.render_page_plan(next.pages[2].clone(),500).unwrap(),&reference,"custom-font changed copy");
            capture("compat-false-changed.pdf",&changed_path);capture_bytes("compat-false-changed-2-expected.png",&reference);
            fresh.close_document(&next_doc.id).unwrap();
        }
        fresh.close_document(&opened.id).unwrap();
        assert_eq!(fs::read(&a).unwrap(),original_a);assert_eq!(fs::read(&b).unwrap(),original_b);
    }
}

#[test]
fn structured_overlay_save_unsupported_graph_remains_visible_without_lossy_save() {
    let e=engine();let dir=tempfile::tempdir().unwrap();let input=dir.path().join("tagged.pdf");fixture(&input,true,"Unsupported catalog");
    let source=e.open_document(&input).unwrap();let mut plan=request(&[&source]);plan.pages[0].overlays=overlays(&drawing_asset(&e,dir.path()));
    let path=dir.path().join("owned.pdf");e.export_pdf(plan,&path).unwrap();
    let mut pdf=Document::load(&path).unwrap();pdf.catalog_mut().unwrap().set("OCProperties",dictionary!{"Unsupported"=>true});pdf.save(&path).unwrap();
    let before=fs::read(&path).unwrap();let opened=e.open_document(&path).unwrap();let mut plan=request(&[&opened]);
    assert_eq!(plan.pages[0].overlays.len(),4);
    let original=e.render_page_plan(plan.pages[0].clone(),500).unwrap();
    let text=plan.pages[0].overlays.iter_mut().find_map(|o|if let Overlay::Text(v)=o{Some(v)}else{None}).unwrap();text.text="Visible unsupported draft".into();
    let edited=e.render_page_plan(plan.pages[0].clone(),500).unwrap();assert_ne!(original,edited,"unsupported source still renders draft edits");
    let output=dir.path().join("refused.pdf");fs::write(&output,b"keep destination").unwrap();
    assert!(e.export_pdf(plan,&output).is_err());assert_eq!(fs::read(output).unwrap(),b"keep destination");assert_eq!(fs::read(path).unwrap(),before);
}
