use super::*;

#[test]
fn structured_occurrences_duplicate_tagged_pages_with_independent_ownership() {
    let e=engine();let dir=tempfile::tempdir().unwrap();let input=dir.path().join("source.pdf");
    let before=fixture(&input,true,"Duplicated");let source=e.open_document(&input).unwrap();let base=request(&[&source]);
    for order in [vec![0,0,1],vec![0,1,0,1]] {
        let pages=order.iter().enumerate().map(|(i,old)|{let mut page=base.pages[*old].clone();page.id=format!("occurrence-{i}");page}).collect();
        let plan=ExportRequest{flatten:false,pages};let output=dir.path().join(format!("copy-{}.pdf",order.len()));
        e.export_pdf(plan.clone(),&output).expect("duplicate tagged pages should save faithfully");
        let saved=Document::load(&output).unwrap();let ids=saved.get_pages().values().copied().collect::<Vec<_>>();
        assert_eq!(ids.iter().collect::<std::collections::BTreeSet<_>>().len(),order.len());check_structure(&saved,if order.len()==3 {5}else{6});
        let first=saved.get_dictionary(ids[0]).unwrap();let copy=saved.get_dictionary(ids[if order.len()==3{1}else{2}]).unwrap();
        assert_ne!(first.get(b"StructParents").unwrap(),copy.get(b"StructParents").unwrap());
        assert_eq!(first.get(b"Contents").unwrap(),copy.get(b"Contents").unwrap(),"immutable page content can remain shared");
        let a=first.get(b"Annots").unwrap().as_array().unwrap();let b=copy.get(b"Annots").unwrap().as_array().unwrap();assert_eq!(a.len(),b.len());
        for (x,y) in a.iter().zip(b) {assert_ne!(x,y,"each copy owns its annotations");}
        let reopened=e.open_document(&output).unwrap();
        for (i,old) in order.iter().enumerate() {
            assert_eq!(e.render_page(&source.id,*old,400).unwrap(),e.render_page(&reopened.id,i,400).unwrap());
            assert_eq!(e.extract_text(&source.id,*old).unwrap(),e.extract_text(&reopened.id,i).unwrap());
        }
    }
    assert_eq!(fs::read(input).unwrap(),before);
}

#[test]
fn structured_projection_saves_unique_reordered_and_interleaved_pages() {
    let engine=engine();let dir=tempfile::tempdir().unwrap();
    let a=dir.path().join("first.pdf");let b=dir.path().join("second.pdf");
    let before_a=fixture(&a,true,"First");let before_b=fixture(&b,false,"Second");
    let first=engine.open_document(&a).unwrap();let second=engine.open_document(&b).unwrap();
    let base=request(&[&first,&second]);
    for order in [vec![1,0,3,2],vec![1,2,0,3]] {
        let plan=ExportRequest{flatten:false,pages:order.iter().map(|i|base.pages[*i].clone()).collect()};
        let output=dir.path().join(format!("ordered-{}.pdf",order[1]));
        engine.export_pdf(plan.clone(),&output).unwrap();let reopened=engine.open_document(&output).unwrap();
        assert_eq!(reopened.pages.len(),4);
        for (index,page) in plan.pages.iter().enumerate() {
            assert_eq!(engine.render_page(&page.source_id,page.page_index,400).unwrap(),engine.render_page(&reopened.id,index,400).unwrap());
            assert_eq!(engine.extract_text(&page.source_id,page.page_index).unwrap(),engine.extract_text(&reopened.id,index).unwrap());
        }
        let saved=Document::load(&output).unwrap();check_structure(&saved,3);
        let pages=saved.get_pages().values().copied().collect::<Vec<_>>();
        let first_page=pages[if order[1]==0 {1}else{2}];
        let link=saved.get_dictionary(first_page).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap();
        assert_eq!(saved.get_dictionary(link).unwrap().get(b"Dest").unwrap().as_array().unwrap()[0].as_reference().unwrap(),pages[0]);
    }
    assert_eq!(fs::read(a).unwrap(),before_a);assert_eq!(fs::read(b).unwrap(),before_b);
}

#[test]
fn structured_projection_selects_tagged_page_without_dangling_navigation() {
    let engine=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("source.pdf");
    let original=fixture(&a,true,"Selected");let source=engine.open_document(&a).unwrap();
    let mut plan=request(&[&source]);plan.pages.remove(0);
    let output=dir.path().join("selected.pdf");engine.export_pdf(plan,&output).unwrap();
    let reopened=engine.open_document(&output).unwrap();assert_eq!(reopened.pages.len(),1);
    assert_eq!(engine.render_page(&source.id,1,400).unwrap(),engine.render_page(&reopened.id,0,400).unwrap());
    assert_eq!(engine.extract_text(&source.id,1).unwrap(),engine.extract_text(&reopened.id,0).unwrap());
    let saved=Document::load(&output).unwrap();check_structure(&saved,1);
    assert_eq!(saved.objects.values().filter(|v|v.as_dict().ok().and_then(|d|d.get(b"Type").ok()).and_then(|v|v.as_name().ok())==Some(b"Page".as_slice())).count(),1);
    assert_eq!(fs::read(a).unwrap(),original);
}

#[test]
fn structured_projection_disables_live_link_to_removed_page_without_changing_appearance() {
    let engine=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("source.pdf");fixture(&a,true,"Keep");
    let original=fs::read(&a).unwrap();let source=engine.open_document(&a).unwrap();let mut plan=request(&[&source]);plan.pages.pop();
    let output=dir.path().join("selected.pdf");engine.export_pdf(plan,&output).unwrap();
    let reopened=engine.open_document(&output).unwrap();assert_eq!(reopened.pages.len(),1);
    assert_eq!(engine.render_page(&source.id,0,400).unwrap(),engine.render_page(&reopened.id,0,400).unwrap());
    assert_eq!(engine.extract_text(&source.id,0).unwrap(),engine.extract_text(&reopened.id,0).unwrap());
    let saved=Document::load(&output).unwrap();check_structure(&saved,2);
    let page=*saved.get_pages().values().next().unwrap();
    let link=saved.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap();
    assert!(!saved.get_dictionary(link).unwrap().has(b"Dest"));assert_eq!(fs::read(a).unwrap(),original);
}

#[test]
#[ignore="requires explicit authored corpus and a new output directory"]
fn structured_projection_authored_word_and_pictures() {
    let engine=engine();let base=PathBuf::from(std::env::var_os("FOLIO_PROJECTION_BASE").unwrap());
    let navigation=PathBuf::from(std::env::var_os("FOLIO_PROJECTION_NAVIGATION").unwrap());
    let output=PathBuf::from(std::env::var_os("FOLIO_PROJECTION_OUTPUT").unwrap());assert!(!output.exists());fs::create_dir(&output).unwrap();
    let mut records=vec![];
    for (name,input,order) in [
        ("word-selected",&base,vec![1]),
        ("combined-selected",&base,vec![1,3]),
        ("navigation-reordered",&navigation,vec![1,2,0,3]),
    ] {
        let word=engine.open_document(input.join("Word.pdf")).unwrap();let pictures=engine.open_document(input.join("Pictures.pdf")).unwrap();
        let all=request(&[&word,&pictures]);let pages=order.iter().map(|i|all.pages[*i].clone()).collect::<Vec<_>>();
        let plan=ExportRequest{flatten:false,pages};let path=output.join(format!("{name}.pdf"));
        engine.export_pdf(plan.clone(),&path).unwrap();let reopened=engine.open_document(&path).unwrap();
        assert_eq!(reopened.pages.len(),order.len());
        for (i,page) in plan.pages.iter().enumerate(){
            assert_eq!(engine.render_page(&page.source_id,page.page_index,900).unwrap(),engine.render_page(&reopened.id,i,900).unwrap());
            assert_eq!(engine.extract_text(&page.source_id,page.page_index).unwrap(),engine.extract_text(&reopened.id,i).unwrap());
        }
        records.push(serde_json::json!({"name":name,"input":input,"output":path,"order":order,"bookmarks":reopened.bookmarks}));
    }
    fs::write(output.join("native-result.json"),serde_json::to_vec_pretty(&records).unwrap()).unwrap();
}

#[test]
#[ignore="requires explicit authored navigation corpus and a new output directory"]
fn structured_projection_authored_deleted_navigation() {
    let engine=engine();let input=PathBuf::from(std::env::var_os("FOLIO_DELETED_NAVIGATION_INPUT").unwrap());
    let output=PathBuf::from(std::env::var_os("FOLIO_DELETED_NAVIGATION_OUTPUT").unwrap());assert!(!output.exists());fs::create_dir(&output).unwrap();
    let before=[fs::read(input.join("Word.pdf")).unwrap(),fs::read(input.join("Pictures.pdf")).unwrap()];
    let mut records=vec![];
    for (name,order) in [("word-first",vec![0]),("word-last",vec![1]),("combined-first",vec![0,2]),("combined-mixed",vec![1,2])] {
        let word=engine.open_document(input.join("Word.pdf")).unwrap();let pictures=engine.open_document(input.join("Pictures.pdf")).unwrap();
        let all=request(&[&word,&pictures]);let plan=ExportRequest{flatten:false,pages:order.iter().map(|i|all.pages[*i].clone()).collect()};
        let path=output.join(format!("{name}.pdf"));engine.export_pdf(plan.clone(),&path).unwrap();let reopened=engine.open_document(&path).unwrap();
        assert_eq!(reopened.pages.len(),order.len());assert_eq!(reopened.bookmarks.len(),1);
        for (i,page) in plan.pages.iter().enumerate(){
            assert_eq!(engine.render_page(&page.source_id,page.page_index,900).unwrap(),engine.render_page(&reopened.id,i,900).unwrap());
            assert_eq!(engine.extract_text(&page.source_id,page.page_index).unwrap(),engine.extract_text(&reopened.id,i).unwrap());
        }
        let again=output.join(format!("{name}-again.pdf"));engine.export_pdf(request(&[&reopened]),&again).unwrap();assert_eq!(fs::read(&again).unwrap(),fs::read(&path).unwrap());
        records.push(serde_json::json!({"name":name,"input":input,"output":path,"order":order,"bookmarks":reopened.bookmarks,"unchangedResave":again}));
    }
    assert_eq!(fs::read(input.join("Word.pdf")).unwrap(),before[0]);assert_eq!(fs::read(input.join("Pictures.pdf")).unwrap(),before[1]);
    fs::write(output.join("native-result.json"),serde_json::to_vec_pretty(&records).unwrap()).unwrap();
}

#[test]
fn structured_occurrences_copy_only_crop_rotation_and_cancel_preserve_the_original() {
    let e=engine();let dir=tempfile::tempdir().unwrap();let input=dir.path().join("source.pdf");let before=fixture(&input,true,"Independent");let source=e.open_document(&input).unwrap();
    let mut golden=request(&[&source]);golden.pages[0].rotation=90;golden.pages[0].crop=Some(serde_json::from_value(serde_json::json!({"x":34,"y":51,"width":170,"height":255})).unwrap());
    let golden_path=dir.path().join("golden.pdf");e.export_pdf(golden.clone(),&golden_path).unwrap();let golden_source=e.open_document(golden_path).unwrap();
    let base=request(&[&source]);let mut copy=golden.pages[0].clone();copy.id="copy".into();let plan=ExportRequest{flatten:false,pages:vec![base.pages[0].clone(),copy,base.pages[1].clone()]};
    for stage in ["preserving","applying","preparingFile"] {
        let output=dir.path().join(format!("cancel-{stage}.pdf"));fs::write(&output,b"keep destination").unwrap();
        let control=Arc::new(PdfSaveControl::new());let cancel=control.clone();let observed=Arc::new(AtomicBool::new(false));let seen=observed.clone();
        let report=Arc::new(move|event:PdfSaveProgress|{if event.stage==stage {seen.store(true,Ordering::SeqCst);cancel.cancel();}});
        assert!(!e.export_pdf_controlled(plan.clone(),None,&output,control,report).unwrap());assert!(observed.load(Ordering::SeqCst));assert_eq!(fs::read(output).unwrap(),b"keep destination");
    }
    let output=dir.path().join("saved.pdf");e.export_pdf(plan,&output).unwrap();let reopened=e.open_document(&output).unwrap();
    assert_eq!(e.render_page(&source.id,0,400).unwrap(),e.render_page(&reopened.id,0,400).unwrap());
    assert_eq!(e.render_page(&golden_source.id,0,400).unwrap(),e.render_page(&reopened.id,1,400).unwrap());
    assert_eq!(e.render_page(&source.id,1,400).unwrap(),e.render_page(&reopened.id,2,400).unwrap());
    let again=dir.path().join("again.pdf");e.export_pdf(request(&[&reopened]),&again).unwrap();assert_eq!(fs::read(again).unwrap(),fs::read(output).unwrap());assert_eq!(fs::read(input).unwrap(),before);
}

#[test]
#[ignore="requires explicit authored navigation inputs and a new occurrence output directory"]
fn structured_occurrences_authored_navigation_and_copy_rotation() {
    let e=engine();let input=PathBuf::from(std::env::var_os("FOLIO_DUPLICATE_INPUT").unwrap());let output=PathBuf::from(std::env::var_os("FOLIO_DUPLICATE_OUTPUT").unwrap());assert!(!output.exists());fs::create_dir(&output).unwrap();
    let before=[fs::read(input.join("Word.pdf")).unwrap(),fs::read(input.join("Pictures.pdf")).unwrap()];let mut records=vec![];
    for (name,order,rotated) in [("word-first",vec![0,0,1],false),("word-last",vec![0,1,1],false),("combined-interleaved",vec![0,2,0,3,1,2],false),("word-copy-rotated",vec![0,0,1],true)] {
        let word=e.open_document(input.join("Word.pdf")).unwrap();let pictures=e.open_document(input.join("Pictures.pdf")).unwrap();let all=request(&[&word,&pictures]);
        let mut pages=order.iter().enumerate().map(|(i,old)|{let mut page=all.pages[*old].clone();page.id=format!("occurrence-{i}");page}).collect::<Vec<_>>();
        if rotated{pages[1].rotation=90;}
        let plan=ExportRequest{flatten:false,pages};let path=output.join(format!("{name}.pdf"));e.export_pdf(plan.clone(),&path).unwrap();let reopened=e.open_document(&path).unwrap();assert_eq!(reopened.pages.len(),order.len());
        let golden=if rotated {let mut golden=request(&[&word]);golden.pages[0].rotation=90;let golden_path=output.join("rotated-golden.pdf");e.export_pdf(golden,&golden_path).unwrap();Some(e.open_document(golden_path).unwrap())}else{None};
        for (i,page) in plan.pages.iter().enumerate() {
            let (id,index)=if rotated && i==1{(&golden.as_ref().unwrap().id,0)}else{(&page.source_id,page.page_index)};
            assert_eq!(e.render_page(id,index,900).unwrap(),e.render_page(&reopened.id,i,900).unwrap());assert_eq!(e.extract_text(id,index).unwrap(),e.extract_text(&reopened.id,i).unwrap());
        }
        let again=output.join(format!("{name}-again.pdf"));e.export_pdf(request(&[&reopened]),&again).unwrap();assert_eq!(fs::read(&again).unwrap(),fs::read(&path).unwrap());
        records.push(serde_json::json!({"name":name,"input":input,"output":path,"order":order,"rotations":plan.pages.iter().map(|p|p.rotation).collect::<Vec<_>>(),"bookmarks":reopened.bookmarks,"unchangedResave":again}));
        for id in [&word.id,&pictures.id,&reopened.id]{e.close_document(id).unwrap();}if let Some(golden)=golden{e.close_document(&golden.id).unwrap();}
    }
    assert_eq!(fs::read(input.join("Word.pdf")).unwrap(),before[0]);assert_eq!(fs::read(input.join("Pictures.pdf")).unwrap(),before[1]);fs::write(output.join("native-result.json"),serde_json::to_vec_pretty(&records).unwrap()).unwrap();
}
