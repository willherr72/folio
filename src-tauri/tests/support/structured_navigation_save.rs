use super::*;

fn outlined(path:&Path,label:&str,native:bool)->Vec<u8>{
    fixture(path,true,label);
    let mut d=Document::load(path).unwrap();let pages=d.get_pages();
    let root=d.new_object_id();let one=d.new_object_id();let two=d.new_object_id();
    let title=Object::String([vec![0xfe,0xff],format!("{label} first").encode_utf16().flat_map(u16::to_be_bytes).collect()].concat(),lopdf::StringFormat::Hexadecimal);
    let mut first=dictionary!{"Title"=>title,"Parent"=>root,"Next"=>two,"F"=>3,"C"=>vec![0.2.into(),0.4.into(),0.6.into()]};
    let mut second=dictionary!{"Title"=>Object::string_literal(format!("{label} second")),"Parent"=>root,"Prev"=>one};
    if native {
        let structure=d.get_dictionary(d.catalog().unwrap().get(b"StructTreeRoot").unwrap().as_reference().unwrap()).unwrap();
        let element=structure.get(b"K").unwrap().as_array().unwrap()[0].clone();
        first.set("SE",element);first.set("Private",dictionary!{"Text"=>Object::string_literal("retained metadata")});
        first.set("Dest",Object::Name(b"chapter".to_vec()));
        let action=d.add_object(dictionary!{"S"=>"GoTo","D"=>Object::string_literal("chapter")});
        second.set("A",action);
        let annotation=d.get_dictionary(pages[&1]).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap();
        let link=d.get_dictionary_mut(annotation).unwrap();link.remove(b"Dest");link.set("A",action);
        d.catalog_mut().unwrap().set("Dests",dictionary!{"chapter"=>vec![Object::Reference(pages[&1]),Object::Name(b"Fit".to_vec())]});
        let tree=d.add_object(dictionary!{"Names"=>vec![Object::string_literal("chapter"),Object::Array(vec![Object::Reference(pages[&2]),Object::Name(b"XYZ".to_vec()),Object::Null,12.into(),Object::Null])]});
        d.catalog_mut().unwrap().set("Names",dictionary!{"Dests"=>tree});
    } else {
        first.set("Dest",vec![Object::Reference(pages[&1]),Object::Name(b"Fit".to_vec())]);
        second.set("Dest",vec![Object::Reference(pages[&2]),Object::Name(b"Fit".to_vec())]);
    }
    d.objects.insert(one,first.into());d.objects.insert(two,second.into());
    d.objects.insert(root,dictionary!{"Type"=>"Outlines","First"=>one,"Last"=>two,"Count"=>2}.into());
    d.catalog_mut().unwrap().set("Outlines",root);d.save(path).unwrap();fs::read(path).unwrap()
}
fn explicit(source:&DocumentInfo,plan:&ExportRequest)->Vec<folio_engine::DocumentBookmark>{
    let map=source.pages.iter().enumerate().map(|(i,_)|(i.to_string(),plan.pages[i].id.clone())).collect();
    folio_engine::bookmarks::remap(&source.bookmarks,&map)
}

#[test]
fn structured_navigation_selected_native_outlines_keep_raw_metadata_and_requested_intent(){
    let e=engine();let dir=tempfile::tempdir().unwrap();let input=dir.path().join("native.pdf");
    let original=outlined(&input,"Native",true);let source=e.open_document(&input).unwrap();
    let old=Document::load_mem(&original).unwrap();let original_items=outline_items(&old);
    for index in 0..2 {
        let all=request(&[&source]);let plan=ExportRequest{flatten:false,pages:vec![all.pages[index].clone()]};
        let mapping=std::collections::HashMap::from([(index.to_string(),plan.pages[0].id.clone())]);
        let projected=folio_engine::bookmarks::remap(&source.bookmarks,&mapping);
        for explicit in [false,true] {
            let output=dir.path().join(format!("selected-{index}-{explicit}.pdf"));
            e.export_pdf_with_bookmarks(plan.clone(),explicit.then(||projected.clone()),&output).unwrap();
            let saved=Document::load(&output).unwrap();let items=outline_items(&saved);assert_eq!(items.len(),1);
            let kept=saved.get_dictionary(items[0]).unwrap();let prior=old.get_dictionary(original_items[index]).unwrap();
            assert_eq!(kept.get(b"Title").unwrap(),prior.get(b"Title").unwrap());
            if index==0 {assert_eq!(kept.get(b"Private").unwrap(),prior.get(b"Private").unwrap());assert!(kept.has(b"SE"));assert!(saved.catalog().unwrap().has(b"Dests"));assert!(!saved.catalog().unwrap().has(b"Names"));}
            else {assert!(kept.has(b"A"));assert!(!saved.catalog().unwrap().has(b"Dests"));assert!(saved.catalog().unwrap().has(b"Names"));}
            let reopened=e.open_document(&output).unwrap();assert_eq!(reopened.bookmarks.len(),1);
            assert_eq!(e.render_page(&source.id,index,400).unwrap(),e.render_page(&reopened.id,0,400).unwrap());
            assert_eq!(e.extract_text(&source.id,index).unwrap(),e.extract_text(&reopened.id,0).unwrap());
        }
        let mut changed=projected;changed[0].title="An unrelated edit".into();let output=dir.path().join("refused.pdf");fs::write(&output,b"keep").unwrap();
        assert!(e.export_pdf_with_bookmarks(plan,Some(changed),&output).is_err());assert_eq!(fs::read(&output).unwrap(),b"keep");
    }
    assert_eq!(fs::read(input).unwrap(),original);
}
fn outline_items(d:&Document)->Vec<lopdf::ObjectId>{
    let root=d.get_dictionary(d.catalog().unwrap().get(b"Outlines").unwrap().as_reference().unwrap()).unwrap();
    let mut next=Some(root.get(b"First").unwrap().as_reference().unwrap());let mut ids=vec![];
    while let Some(id)=next {assert!(ids.len()<10);ids.push(id);next=d.get_dictionary(id).unwrap().get(b"Next").ok().map(|o|o.as_reference().unwrap());}ids
}
fn destination(d:&Document,v:&Object)->Vec<Object>{
    let v=d.dereference(v).unwrap().1;
    let resolved=match v {
        Object::Name(key)=>d.catalog().unwrap().get(b"Dests").unwrap().as_dict().unwrap().get(key).unwrap(),
        Object::String(key,_)=>{
            let tree=d.dereference(d.catalog().unwrap().get(b"Names").unwrap().as_dict().unwrap().get(b"Dests").unwrap()).unwrap().1.as_dict().unwrap();
            let entries=tree.get(b"Names").unwrap().as_array().unwrap();&entries.chunks_exact(2).find(|pair|pair[0].as_str().unwrap()==key).unwrap()[1]
        },
        _=>v,
    };d.dereference(resolved).unwrap().1.as_array().unwrap().clone()
}

#[test]
fn structured_navigation_preserves_raw_outlines_and_distinct_local_names_on_save_reopen(){
    let e=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a.pdf");let b=dir.path().join("b.pdf");
    let before_a=outlined(&a,"First",true);let before_b=outlined(&b,"Second",true);
    let first=e.open_document(&a).unwrap();let second=e.open_document(&b).unwrap();assert_eq!(first.bookmarks.len(),2);assert_eq!(second.bookmarks.len(),2);
    let output=dir.path().join("combined.pdf");e.export_pdf(request(&[&first,&second]),&output).unwrap();
    let d=Document::load(&output).unwrap();check_structure(&d,6);let pages=d.get_pages();let items=outline_items(&d);assert_eq!(items.len(),4);
    let reopened=e.open_document(&output).unwrap();assert_eq!(reopened.bookmarks.iter().map(|item|item.title.as_str()).collect::<Vec<_>>(),vec!["First first","First second","Second first","Second second"]);
    for (index,source) in [&first,&second].iter().enumerate(){
        let one=d.get_dictionary(items[index*2]).unwrap();let two=d.get_dictionary(items[index*2+1]).unwrap();
        let original=Document::load_mem(if index==0{&before_a}else{&before_b}).unwrap();let original_items=outline_items(&original);
        for (actual,old) in [(one,original_items[0]),(two,original_items[1])] {assert_eq!(actual.get(b"Title").unwrap().as_str().unwrap(),original.get_dictionary(old).unwrap().get(b"Title").unwrap().as_str().unwrap());}
        assert_eq!(one.get(b"C").unwrap(),original.get_dictionary(original_items[0]).unwrap().get(b"C").unwrap());
        assert_eq!(one.get(b"Private").unwrap().as_dict().unwrap().get(b"Text").unwrap().as_str().unwrap(),b"retained metadata");
        assert_eq!(one.get(b"F").unwrap().as_i64().unwrap(),3);assert!(one.get(b"Title").unwrap().as_str().unwrap().starts_with(b"\xfe\xff"));
        assert_eq!(d.get_dictionary(one.get(b"SE").unwrap().as_reference().unwrap()).unwrap().get(b"Pg").unwrap(),&Object::Reference(pages[&(index as u32*2+1)]));
        assert_eq!(destination(&d,one.get(b"Dest").unwrap())[0],Object::Reference(pages[&(index as u32*2+1)]));
        let action=d.dereference(two.get(b"A").unwrap()).unwrap().1.as_dict().unwrap();
        assert_eq!(destination(&d,action.get(b"D").unwrap()),vec![Object::Reference(pages[&(index as u32*2+2)]),Object::Name(b"XYZ".to_vec()),Object::Null,12.into(),Object::Null]);
        let link=d.get_dictionary(d.get_dictionary(pages[&(index as u32*2+1)]).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap()).unwrap();
        assert_eq!(link.get(b"A").unwrap(),two.get(b"A").unwrap());
        for p in 0..2 {assert_eq!(e.render_page(&source.id,p,400).unwrap(),e.render_page(&reopened.id,index*2+p,400).unwrap());assert_eq!(e.extract_text(&source.id,p).unwrap(),e.extract_text(&reopened.id,index*2+p).unwrap());}
    }
    let again=dir.path().join("again.pdf");e.export_pdf(request(&[&reopened]),&again).unwrap();assert_eq!(fs::read(again).unwrap(),fs::read(output).unwrap());
    let plan=request(&[&first,&second]);let mut projected=explicit(&first,&plan);
    let mapping=(0..2).map(|index|(index.to_string(),plan.pages[index+2].id.clone())).collect();
    projected.extend(folio_engine::bookmarks::remap(&second.bookmarks,&mapping));
    for (index,item) in projected.iter_mut().enumerate(){item.id=format!("transient-ui-{index}");}
    let ui=dir.path().join("ui-save.pdf");e.export_pdf_with_bookmarks(plan,Some(projected),&ui).unwrap();
    let ui=Document::load(ui).unwrap();let items=outline_items(&ui);assert_eq!(items.len(),4);
    assert!(ui.get_dictionary(items[0]).unwrap().has(b"SE"));assert!(ui.get_dictionary(items[2]).unwrap().has(b"Private"));
    assert_eq!(fs::read(a).unwrap(),before_a);assert_eq!(fs::read(b).unwrap(),before_b);
}

#[test]
fn structured_navigation_deliberate_clear_accepts_unreadable_outlines_but_keeps_links(){
    let e=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a.pdf");let b=dir.path().join("b.pdf");
    outlined(&a,"First",true);outlined(&b,"Second",true);
    let mut bad=Document::load(&b).unwrap();bad.catalog_mut().unwrap().set("Outlines",42);bad.save(&b).unwrap();
    let first=e.open_document(&a).unwrap();let second=e.open_document(&b).unwrap();let output=dir.path().join("cleared.pdf");
    fs::write(&output,b"keep").unwrap();assert!(e.export_pdf(request(&[&first,&second]),&output).is_err());assert_eq!(fs::read(&output).unwrap(),b"keep");
    e.export_pdf_with_bookmarks(request(&[&first,&second]),Some(vec![]),&output).unwrap();
    let saved=Document::load(output).unwrap();assert!(!saved.catalog().unwrap().has(b"Outlines"));check_structure(&saved,6);
    for (index,id) in saved.get_pages().values().step_by(2).enumerate(){
        let link=saved.get_dictionary(saved.get_dictionary(*id).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap()).unwrap();
        let action=saved.dereference(link.get(b"A").unwrap()).unwrap().1.as_dict().unwrap();
        assert_eq!(destination(&saved,action.get(b"D").unwrap())[0],Object::Reference(saved.get_pages()[&(index as u32*2+2)]));
    }
}

#[test]
fn structured_navigation_changes_only_compatible_outline_projections(){
    let e=engine();let dir=tempfile::tempdir().unwrap();let b=dir.path().join("plain.pdf");fixture(&b,true,"Plain");let plain=e.open_document(&b).unwrap();
    for native in [false,true]{
        let a=dir.path().join(format!("outline-{native}.pdf"));outlined(&a,"First",native);let first=e.open_document(&a).unwrap();let plan=request(&[&first,&plain]);
        let mut bookmarks=explicit(&first,&plan);bookmarks[0].title="Changed title".into();let output=dir.path().join(format!("changed-{native}.pdf"));fs::write(&output,b"keep").unwrap();
        let result=e.export_pdf_with_bookmarks(plan,Some(bookmarks),&output);
        if native{assert!(result.is_err());assert_eq!(fs::read(output).unwrap(),b"keep");}else{result.unwrap();let reopened=e.open_document(output).unwrap();assert_eq!(reopened.bookmarks[0].title,"Changed title");assert!(reopened.bookmarks[0].bold && reopened.bookmarks[0].italic);}
    }
}

#[test]
fn structured_navigation_language_identifiers_do_not_authorize_titles_or_unknown_strings(){
    let e=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a.pdf");let b=dir.path().join("b.pdf");
    outlined(&a,"First",true);outlined(&b,"Second",true);
    let mut d=Document::load(&b).unwrap();d.catalog_mut().unwrap().set("Lang",Object::string_literal("fr-FR"));
    let page=d.get_pages()[&1];let link=d.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap();
    let action=d.get_dictionary(link).unwrap().get(b"A").unwrap().as_reference().unwrap();
    let remote=d.add_object(dictionary!{"S"=>"GoToR","D"=>Object::string_literal("chapter"),"F"=>dictionary!{"Type"=>"Filespec","F"=>Object::string_literal("other.pdf"),"UF"=>Object::string_literal("other.pdf")}});
    d.get_dictionary_mut(action).unwrap().set("Next",remote);
    d.get_dictionary_mut(page).unwrap().set("AA",dictionary!{"O"=>action});d.save(&b).unwrap();
    let first=e.open_document(&a).unwrap();let second=e.open_document(&b).unwrap();let output=dir.path().join("mixed.pdf");
    fs::write(&output,b"keep").unwrap();assert!(e.export_pdf(request(&[&first,&second]),&output).is_err(),"page language cannot preserve differently-defaulted outline titles");assert_eq!(fs::read(&output).unwrap(),b"keep");
    e.export_pdf_with_bookmarks(request(&[&first,&second]),Some(vec![]),&output).unwrap();
    let saved=Document::load(&output).unwrap();assert!(!saved.catalog().unwrap().has(b"Outlines"));
    let third=saved.get_dictionary(saved.get_pages()[&3]).unwrap();
    let action=saved.dereference(third.get(b"AA").unwrap().as_dict().unwrap().get(b"O").unwrap()).unwrap().1.as_dict().unwrap();
    assert_eq!(destination(&saved,action.get(b"D").unwrap())[0],Object::Reference(saved.get_pages()[&4]));
    let remote=saved.dereference(action.get(b"Next").unwrap()).unwrap().1.as_dict().unwrap();assert_eq!(remote.get(b"D").unwrap().as_str().unwrap(),b"chapter");
    for field in ["D","Names","URI"]{
        let mut poisoned=d.clone();poisoned.get_dictionary_mut(page).unwrap().set("UnknownMetadata",dictionary!{field=>Object::string_literal("unclassified human text")});
        let path=dir.path().join(format!("unknown-{field}.pdf"));poisoned.save(&path).unwrap();let source=e.open_document(path).unwrap();fs::write(&output,b"keep").unwrap();
        assert!(e.export_pdf_with_bookmarks(request(&[&first,&source]),Some(vec![]),&output).is_err());assert_eq!(fs::read(&output).unwrap(),b"keep");
    }
}

#[test]
fn structured_navigation_explicit_clear_of_empty_tree_is_distinct_from_inheritance(){
    let e=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a.pdf");let b=dir.path().join("b.pdf");fixture(&a,true,"First");fixture(&b,true,"Second");
    let mut d=Document::load(&b).unwrap();let root=d.add_object(dictionary!{"Type"=>"Outlines","Count"=>0,"Private"=>Object::string_literal("producer root metadata")});d.catalog_mut().unwrap().set("Outlines",root);d.save(&b).unwrap();
    let first=e.open_document(a).unwrap();let second=e.open_document(b).unwrap();assert!(second.bookmarks.is_empty());
    let output=dir.path().join("clear.pdf");fs::write(&output,b"keep").unwrap();
    assert!(e.export_pdf(request(&[&first,&second]),&output).is_err());assert_eq!(fs::read(&output).unwrap(),b"keep");
    e.export_pdf_with_bookmarks(request(&[&first,&second]),Some(vec![]),&output).unwrap();assert!(!Document::load(output).unwrap().catalog().unwrap().has(b"Outlines"));
}

#[test]
fn structured_navigation_replacement_refuses_external_outline_node_consumers(){
    let e=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a.pdf");let b=dir.path().join("b.pdf");outlined(&a,"First",false);fixture(&b,true,"Second");
    let mut d=Document::load(&a).unwrap();let first=outline_items(&d)[0];let page=d.get_pages()[&1];let alias=d.add_object(Object::Reference(first));d.get_dictionary_mut(page).unwrap().set("PrivateBookmarkReference",alias);d.save(&a).unwrap();
    let first=e.open_document(&a).unwrap();let second=e.open_document(b).unwrap();let plan=request(&[&first,&second]);let mut bookmarks=explicit(&first,&plan);bookmarks[0].title="Changed".into();
    let output=dir.path().join("changed.pdf");fs::write(&output,b"keep").unwrap();assert!(e.export_pdf_with_bookmarks(plan,Some(bookmarks),&output).is_err());assert_eq!(fs::read(&output).unwrap(),b"keep");
    e.export_pdf(request(&[&first,&second]),output).unwrap();
}

#[test]
fn structured_navigation_preserves_permissions_and_atomic_cancellation(){
    let e=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a.pdf");let b=dir.path().join("b.pdf");let bytes=outlined(&a,"First",true);let before_b=outlined(&b,"Second",true);
    let encrypted=protected_fixture::encrypt(&bytes,lopdf::encryption::Permissions::PRINTABLE);fs::write(&a,&encrypted).unwrap();
    let reader=e.open_document_with_password(&a,"reader").unwrap();let second=e.open_document(&b).unwrap();let output=dir.path().join("output.pdf");fs::write(&output,b"keep").unwrap();
    assert!(e.export_pdf(request(&[&reader,&second]),&output).is_err());assert_eq!(fs::read(&output).unwrap(),b"keep");
    let owner=e.open_document_with_password(&a,"owner").unwrap();
    for stage in ["copying","preserving","applying","preparingFile"] {
        let control=Arc::new(PdfSaveControl::new());let cancel=control.clone();let seen=Arc::new(AtomicBool::new(false));let observed=seen.clone();
        let report=Arc::new(move|event:PdfSaveProgress|{if event.stage==stage{observed.store(true,Ordering::SeqCst);cancel.cancel();}});
        assert!(!e.export_pdf_controlled(request(&[&owner,&second]),None,&output,control,report).unwrap());assert!(seen.load(Ordering::SeqCst));assert_eq!(fs::read(&output).unwrap(),b"keep");
    }
    e.export_pdf(request(&[&owner,&second]),&output).unwrap();assert!(e.open_document(&output).is_err());let reopened=e.open_document_with_password(output,"reader").unwrap();
    assert_eq!(reopened.bookmarks.len(),4);assert!(!reopened.protection.unwrap().permissions.assemble);assert_eq!(fs::read(a).unwrap(),encrypted);assert_eq!(fs::read(b).unwrap(),before_b);
}

#[test]
fn structured_navigation_unsupported_name_catalogs_and_invalid_replacement_destinations_refuse(){
    let e=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a.pdf");let b=dir.path().join("b.pdf");outlined(&a,"First",false);fixture(&b,true,"Second");let plain=e.open_document(&b).unwrap();let template=e.open_document(&a).unwrap();
    for case in ["names","destination"]{
        let mut d=Document::load(&a).unwrap();
        if case=="names"{d.catalog_mut().unwrap().set("Names",dictionary!{"EmbeddedFiles"=>dictionary!{}});}else{let id=outline_items(&d)[0];let page=d.get_pages()[&1];d.get_dictionary_mut(id).unwrap().set("Dest",vec![Object::Reference(page),Object::Name(b"InvalidView".to_vec())]);}
        let input=dir.path().join(format!("{case}.pdf"));d.save(&input).unwrap();let source=e.open_document(input).unwrap();let plan=request(&[&source,&plain]);let mut bookmarks=explicit(&template,&plan);assert_eq!(bookmarks.len(),2);bookmarks[0].title="Changed".into();
        let output=dir.path().join(format!("{case}-output.pdf"));fs::write(&output,b"keep").unwrap();assert!(e.export_pdf_with_bookmarks(plan,Some(bookmarks),&output).is_err());assert_eq!(fs::read(output).unwrap(),b"keep");
    }
}

#[test]
fn structured_navigation_unknown_action_triggers_cannot_authorize_unclassified_strings(){
    let e=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("first.pdf");fixture(&a,true,"First");let first=e.open_document(a).unwrap();
    for (host,trigger) in [("page","X"),("page","Human"),("annotation","O"),("annotation","Fo")] {
        let path=dir.path().join(format!("{host}-{trigger}.pdf"));fixture(&path,true,"Second");let mut d=Document::load(&path).unwrap();
        d.catalog_mut().unwrap().set("Lang",Object::string_literal("fr-FR"));let page=d.get_pages()[&1];
        let host_id=if host=="page"{page}else{d.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap()[1].as_reference().unwrap()};
        d.get_dictionary_mut(host_id).unwrap().set("AA",dictionary!{trigger=>dictionary!{"S"=>"GoTo","D"=>Object::string_literal("unclassified human text")}});d.save(&path).unwrap();
        let second=e.open_document(path).unwrap();let output=dir.path().join(format!("{host}-{trigger}-out.pdf"));fs::write(&output,b"keep").unwrap();
        assert!(e.export_pdf_with_bookmarks(request(&[&first,&second]),Some(vec![]),&output).is_err(),"unknown {host} trigger {trigger} is not a typed action site");
        assert_eq!(fs::read(output).unwrap(),b"keep");
    }
}

#[test]
fn structured_navigation_replacement_cannot_change_another_sources_title_language(){
    let e=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a.pdf");let b=dir.path().join("b.pdf");outlined(&a,"First",false);outlined(&b,"French",false);
    let mut d=Document::load(&b).unwrap();d.catalog_mut().unwrap().set("Lang",Object::string_literal("fr-FR"));d.save(&b).unwrap();
    let first=e.open_document(&a).unwrap();let second=e.open_document(&b).unwrap();let plan=request(&[&first,&second]);let mut bookmarks=explicit(&first,&plan);
    let mapping=(0..2).map(|index|(index.to_string(),plan.pages[index+2].id.clone())).collect();bookmarks.extend(folio_engine::bookmarks::remap(&second.bookmarks,&mapping));
    bookmarks[0].title="Only the English title changed".into();let output=dir.path().join("changed.pdf");fs::write(&output,b"keep").unwrap();
    assert!(e.export_pdf_with_bookmarks(plan,Some(bookmarks),&output).is_err(),"unchanged French titles must not acquire the leading English default");assert_eq!(fs::read(&output).unwrap(),b"keep");
    e.export_pdf_with_bookmarks(request(&[&first,&second]),Some(vec![]),&output).unwrap();assert!(!Document::load(output).unwrap().catalog().unwrap().has(b"Outlines"));
}

#[test]
fn structured_navigation_replacement_respects_final_retained_object_budget(){
    let e=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("dense.pdf");let b=dir.path().join("b.pdf");outlined(&a,"First",false);fixture(&b,false,"Second");
    let mut d=Document::load(&a).unwrap();while d.objects.len()<99_900{d.add_object(Object::Integer(7));}d.save(&a).unwrap();
    let before=fs::read(&a).unwrap();let first=e.open_document(&a).unwrap();let second=e.open_document(&b).unwrap();let plan=request(&[&first,&second]);
    let baseline=dir.path().join("baseline.pdf");e.export_pdf(plan.clone(),&baseline).unwrap();let base_objects=Document::load(&baseline).unwrap().objects.len();assert!((99_900..99_990).contains(&base_objects));
    let count=100_000-base_objects-2;
    let original=explicit(&first,&plan);
    let list=|count:usize|(0..count).map(|index|{let mut item=original[0].clone();item.id=format!("requested-{index}");item.title=format!("New title {index}");item}).collect::<Vec<_>>();
    let accepted=dir.path().join("accepted.pdf");e.export_pdf_with_bookmarks(plan.clone(),Some(list(count)),&accepted).unwrap();assert_eq!(Document::load(&accepted).unwrap().objects.len(),99_999);
    let output=dir.path().join("refused.pdf");fs::write(&output,b"keep").unwrap();
    let error=e.export_pdf_with_bookmarks(plan,Some(list(count+2)),&output).expect_err("replacement must not publish 100001 retained objects");assert!(error.to_string().contains("preservation limits"),"{error}");
    assert_eq!(fs::read(output).unwrap(),b"keep");assert_eq!(fs::read(a).unwrap(),before);
}

#[test]
fn structured_occurrences_preserve_first_bookmark_targets_and_distinguish_explicit_copy_intent() {
    let e=engine();let dir=tempfile::tempdir().unwrap();
    for native in [false,true] {
        let input=dir.path().join(format!("source-{native}.pdf"));let before=outlined(&input,"Copies",native);let source=e.open_document(&input).unwrap();
        let all=request(&[&source]);let mut copy=all.pages[0].clone();copy.id="page-copy".into();
        let plan=ExportRequest{flatten:false,pages:vec![all.pages[0].clone(),copy,all.pages[1].clone()]};
        let mapping=std::collections::HashMap::from([("0".to_string(),plan.pages[0].id.clone()),("1".to_string(),plan.pages[2].id.clone())]);
        let unchanged=folio_engine::bookmarks::remap(&source.bookmarks,&mapping);
        for explicit in [false,true] {
            let output=dir.path().join(format!("unchanged-{native}-{explicit}.pdf"));e.export_pdf_with_bookmarks(plan.clone(),explicit.then(||unchanged.clone()),&output).unwrap();
            let d=Document::load(&output).unwrap();let pages=d.get_pages();let items=outline_items(&d);assert_eq!(items.len(),2);
            let first=d.get_dictionary(items[0]).unwrap();assert_eq!(destination(&d,first.get(b"Dest").unwrap())[0],Object::Reference(pages[&1]));
            if native{assert!(first.has(b"Private"));assert!(first.has(b"SE"));}
            let reopened=e.open_document(&output).unwrap();assert_eq!(reopened.bookmarks.len(),2);
        }
        let mut changed=unchanged;changed[0].destination=Some(folio_engine::PdfValue::Array(vec![folio_engine::PdfValue::Page(plan.pages[1].id.clone()),folio_engine::PdfValue::Name(b"Fit".to_vec())]));
        let output=dir.path().join(format!("retargeted-{native}.pdf"));fs::write(&output,b"keep").unwrap();
        let result=e.export_pdf_with_bookmarks(plan,Some(changed),&output);
        if native {assert!(result.is_err(),"native-only outline metadata must not be silently discarded for a copy target");assert_eq!(fs::read(output).unwrap(),b"keep");}
        else {result.unwrap();let d=Document::load(output).unwrap();let items=outline_items(&d);assert_eq!(destination(&d,d.get_dictionary(items[0]).unwrap().get(b"Dest").unwrap())[0],Object::Reference(d.get_pages()[&2]));}
        assert_eq!(fs::read(input).unwrap(),before);
    }
}
