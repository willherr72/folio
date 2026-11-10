use folio_engine::{DocumentInfo, ExportRequest, PagePlan, PdfEngine, PdfSaveControl, PdfSaveProgress};
use lopdf::{dictionary, Document, Object, Stream};
use serde_json::json;
use std::{fs, path::{Path, PathBuf}, sync::{Arc, atomic::{AtomicBool, Ordering}}};
#[path = "support/protected_fixture.rs"]
mod protected_fixture;

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()
}
fn fixture(path: &Path, tagged: bool, highlight: bool) -> Vec<u8> {
    let mut pdf = Document::with_version("1.7");
    let tree = pdf.new_object_id();
    let structure = pdf.new_object_id();
    let font = pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let resources = pdf.add_object(dictionary! {"Font"=>dictionary! {"F1"=>font}});
    let mut pages = vec![];
    let mut children = vec![];
    let mut numbers = vec![];
    for i in 0..2 {
        let content = pdf.add_object(Stream::new(dictionary! {}, format!("/P <</MCID 0>> BDC BT /F1 14 Tf 40 300 Td (Original page {i}) Tj ET EMC").into_bytes()));
        let page = pdf.add_object(dictionary! {"Type"=>"Page","Parent"=>tree,"Resources"=>resources,"Contents"=>content,
            "Group"=>dictionary! {"Type"=>"Group","S"=>"Transparency","CS"=>"DeviceRGB"}});
        if tagged {
            pdf.get_dictionary_mut(page).unwrap().set("StructParents", i);
            let element = pdf.add_object(dictionary! {"Type"=>"StructElem","S"=>"P","P"=>structure,"Pg"=>page,"K"=>0});
            children.push(Object::Reference(element));
            numbers.extend([Object::Integer(i), Object::Array(vec![element.into()])]);
        }
        if highlight && i == 0 {
            let annotation = pdf.add_object(dictionary! {"Type"=>"Annot","Subtype"=>"Highlight",
                "Rect"=>vec![40.into(),290.into(),150.into(),315.into()],
                "QuadPoints"=>vec![40.into(),315.into(),150.into(),315.into(),40.into(),290.into(),150.into(),290.into()],
                "C"=>vec![1.into(),1.into(),0.into()],"F"=>4,"Contents"=>Object::string_literal("Imported highlight")});
            pdf.get_dictionary_mut(page).unwrap().set("Annots", vec![annotation.into()]);
        }
        pages.push(Object::Reference(page));
    }
    pdf.objects.insert(tree, dictionary! {"Type"=>"Pages","Count"=>2,"Kids"=>pages.clone(),
        "MediaBox"=>vec![10.into(),20.into(),310.into(),420.into()],"Rotate"=>90,"UserUnit"=>1.7}.into());
    // UserUnit is deliberately on each page; Rotate and MediaBox are inherited.
    for page in &pages { pdf.get_dictionary_mut(page.as_reference().unwrap()).unwrap().set("UserUnit", 1.7); }
    let metadata = pdf.add_object(Stream::new(dictionary! {"Type"=>"Metadata","Subtype"=>"XML"}, b"<x:xmpmeta xmlns:x='adobe:ns:meta/'/>".to_vec()));
    let outlines = pdf.new_object_id();
    let outline = pdf.add_object(dictionary! {"Title"=>Object::string_literal("Original bookmark"),"Parent"=>outlines,
        "Dest"=>vec![pages[1].clone(),Object::Name(b"Fit".to_vec())],"FolioUnknownOutlineData"=>Object::string_literal("retain")});
    pdf.objects.insert(outlines, dictionary! {"Type"=>"Outlines","First"=>outline,"Last"=>outline,"Count"=>1}.into());
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree,"Lang"=>Object::string_literal("en-US"),
        "Metadata"=>metadata,"ViewerPreferences"=>dictionary! {"DisplayDocTitle"=>true},"Outlines"=>outlines});
    if tagged {
        let parent_tree = pdf.add_object(dictionary! {"Nums"=>numbers});
        pdf.objects.insert(structure, dictionary! {"Type"=>"StructTreeRoot","K"=>children,"ParentTree"=>parent_tree,"ParentTreeNextKey"=>2}.into());
        pdf.get_dictionary_mut(root).unwrap().set("StructTreeRoot", structure);
        pdf.get_dictionary_mut(root).unwrap().set("MarkInfo", dictionary! {"Marked"=>true});
    }
    pdf.trailer.set("Root", root);
    let info = pdf.add_object(dictionary! {"Title"=>Object::string_literal("Original document title")});
    pdf.trailer.set("Info", info);
    pdf.save(path).unwrap(); fs::read(path).unwrap()
}
fn request(source: &DocumentInfo) -> ExportRequest {
    ExportRequest { flatten: false, pages: source.pages.iter().enumerate().map(|(i,p)| PagePlan {
        id: format!("page-{i}"), source_id: source.id.clone(), page_index: i,
        width: p.width, height: p.height, rotation: 0, crop: None, overlays: p.overlays.clone(),
    }).collect() }
}
fn same_object(a: &Object, b: &Object) { assert_eq!(format!("{a:?}"), format!("{b:?}")); }

#[test]
fn complete_save_keeps_exact_source_bytes_and_unmodeled_outline_properties() {
    let engine = engine(); let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("source.pdf"); let before = fixture(&input,true,false);
    let source = engine.open_document(&input).unwrap(); let plan = request(&source);
    for explicit in [false,true] {
        let output = temp.path().join(format!("saved-{explicit}.pdf"));
        let bookmarks = explicit.then(|| {
            let mut items = source.bookmarks.clone();
            for item in &mut items {
                item.id = format!("ui-{}",item.id);
                item.destination = Some(folio_engine::PdfValue::Array(vec![folio_engine::PdfValue::Page("page-1".into()),folio_engine::PdfValue::Name(b"Fit".to_vec())]));
            } items
        });
        engine.export_pdf_with_bookmarks(plan.clone(),bookmarks,&output).unwrap();
        assert!(fs::read(&output).unwrap()==before,"complete source save changed original graph");
        let reopened=engine.open_document(output).unwrap();
        assert_eq!(engine.render_page(&source.id,0,400).unwrap(),engine.render_page(&reopened.id,0,400).unwrap());
        engine.close_document(&reopened.id).unwrap();
    }
    assert_eq!(fs::read(input).unwrap(),before);
}

#[test]
fn crop_rotation_and_bookmark_removal_retain_all_other_source_objects() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let input=temp.path().join("source.pdf");let before=fixture(&input,true,false);
    let original=Document::load_mem(&before).unwrap();let pages=original.get_pages();
    let source=engine.open_document(&input).unwrap();let mut plan=request(&source);
    plan.pages[0].rotation=90;
    plan.pages[0].crop=Some(serde_json::from_value(json!({"x":34,"y":51,"width":170,"height":255})).unwrap());
    let output=temp.path().join("edited.pdf");
    engine.export_pdf_with_bookmarks(plan,Some(vec![]),&output).unwrap();
    let saved=Document::load(&output).unwrap();assert_eq!(saved.get_pages(),pages);
    assert!(!saved.catalog().unwrap().has(b"Outlines"));
    let root=original.trailer.get(b"Root").unwrap().as_reference().unwrap();
    for (id,object) in &original.objects {
        if *id==root || *id==pages[&1] { continue; }
        // A fresh file has a fresh xref stream; this is serialization bookkeeping,
        // not a reachable document object that the edit should preserve.
        if object.as_stream().is_ok_and(|s|s.dict.get(b"Type").and_then(Object::as_name).is_ok_and(|name|name==b"XRef")) {continue;}
        same_object(object,saved.objects.get(id).unwrap_or_else(||panic!("source object {id:?} disappeared: {object:?}")));
    }
    let mut expected=original.catalog().unwrap().clone();expected.remove(b"Outlines");
    same_object(&expected.into(),&saved.catalog().unwrap().clone().into());
    let changed=saved.get_dictionary(pages[&1]).unwrap();
    assert_eq!(changed.get(b"Rotate").unwrap().as_i64().unwrap(),180);
    assert_eq!(changed.get(b"CropBox").unwrap().as_array().unwrap().iter().map(|v|v.as_float().unwrap()).collect::<Vec<_>>(),vec![40.,40.,190.,140.]);
    let reopened=engine.open_document(output).unwrap();
    assert!((reopened.pages[0].width-255.).abs()<0.001);assert!((reopened.pages[0].height-170.).abs()<0.001);
    assert_eq!(fs::read(input).unwrap(),before);
}

#[test]
fn deleting_imported_annotations_never_reintroduces_original_highlight() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let input=temp.path().join("source.pdf");fixture(&input,false,true);
    let source=engine.open_document(input).unwrap();let mut plan=request(&source);
    assert_eq!(plan.pages[0].overlays.len(),1);plan.pages[0].overlays.clear();
    let output=temp.path().join("deleted.pdf");engine.export_pdf(plan, &output).unwrap();
    let reopened=engine.open_document(output).unwrap();assert!(reopened.pages[0].overlays.is_empty());
}

#[test]
fn structured_composition_refuses_loss_and_leaves_existing_destination_intact() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let input=temp.path().join("source.pdf");fixture(&input,true,false);
    let source=engine.open_document(input).unwrap();let base=request(&source);
    for case in ["subset","reorder","duplicate"] {
        let mut plan=base.clone();match case {
            "subset"=>{plan.pages.pop();},
            "reorder"=>plan.pages.reverse(),
            "duplicate"=>{plan.pages[1].page_index=0;},
            _=>unreachable!(),
        }
        let output=temp.path().join(format!("{case}.pdf"));fs::write(&output,b"keep destination").unwrap();
        let error=engine.export_pdf(plan,&output).unwrap_err().to_string();
        // Native bookmark deletion is supported, but this old Pages branch
        // carries non-inherited UserUnit metadata that must not be discarded.
        assert!(error.contains("page branch metadata"),"{case}: {error}");
        assert_eq!(fs::read(output).unwrap(),b"keep destination");
    }
}

#[test]
fn structured_added_text_preserves_complete_source_geometry_and_ordinary_info() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let input=temp.path().join("source.pdf");let before=fixture(&input,true,false);
    let original=Document::load_mem(&before).unwrap();let source=engine.open_document(&input).unwrap();
    let mut plan=request(&source);
    plan.pages[0].overlays=serde_json::from_value(json!([{"type":"text","id":"new","x":30,"y":30,"text":"Added","fontSize":14,"fontName":"Helvetica","color":"#000000","rotation":0}])).unwrap();
    let expected=engine.render_page_plan(plan.pages[0].clone(),400).unwrap();
    let output=temp.path().join("added.pdf");fs::write(&output,b"keep destination until success").unwrap();
    engine.export_pdf(plan,&output).expect("new owned text on this complete tagged source is supported");
    let saved=Document::load(&output).unwrap();let old_pages=original.get_pages();let pages=saved.get_pages();
    assert_eq!(pages.len(),2);
    for number in 1..=2 {
        assert_eq!(saved.get_page_content(pages[&number]),original.get_page_content(old_pages[&number]),"original page program {number}");
        for key in [b"UserUnit".as_slice(),b"Group"] {
            same_object(original.get_dictionary(old_pages[&number]).unwrap().get(key).unwrap(),saved.get_dictionary(pages[&number]).unwrap().get(key).unwrap());
        }
    }
    let old_catalog=original.catalog().unwrap();let catalog=saved.catalog().unwrap();
    let old_root=original.dereference(old_catalog.get(b"Pages").unwrap()).unwrap().1.as_dict().unwrap();
    let root=saved.dereference(catalog.get(b"Pages").unwrap()).unwrap().1.as_dict().unwrap();
    for key in [b"MediaBox".as_slice(),b"Rotate",b"UserUnit"] {same_object(old_root.get(key).unwrap(),root.get(key).unwrap());}
    for key in [b"Lang".as_slice(),b"ViewerPreferences"] {same_object(old_catalog.get(key).unwrap(),catalog.get(key).unwrap());}
    // Ordinary XMP stays byte-identical; separate metadata regressions exercise
    // selective conformance-property removal without discarding the packet.
    let old_metadata=original.dereference(old_catalog.get(b"Metadata").unwrap()).unwrap().1.as_stream().unwrap();
    let metadata=saved.dereference(catalog.get(b"Metadata").unwrap()).unwrap().1.as_stream().unwrap();
    assert_eq!(metadata.content,old_metadata.content);
    for key in [b"Type".as_slice(),b"Subtype"] {same_object(old_metadata.dict.get(key).unwrap(),metadata.dict.get(key).unwrap());}
    assert!(!catalog.get(b"MarkInfo").unwrap().as_dict().unwrap().get(b"Marked").unwrap().as_bool().unwrap());
    same_object(original.dereference(original.trailer.get(b"Info").unwrap()).unwrap().1,saved.dereference(saved.trailer.get(b"Info").unwrap()).unwrap().1);
    let outlines=saved.dereference(catalog.get(b"Outlines").unwrap()).unwrap().1.as_dict().unwrap();
    let bookmark=saved.dereference(outlines.get(b"First").unwrap()).unwrap().1.as_dict().unwrap();
    assert_eq!(lopdf::decode_text_string(bookmark.get(b"Title").unwrap()).unwrap(),"Original bookmark");
    assert_eq!(lopdf::decode_text_string(bookmark.get(b"FolioUnknownOutlineData").unwrap()).unwrap(),"retain");
    let destination=bookmark.get(b"Dest").unwrap().as_array().unwrap();
    assert_eq!(destination[0].as_reference().unwrap(),pages[&2]);assert_eq!(destination[1].as_name().unwrap(),b"Fit");

    let structure_id=catalog.get(b"StructTreeRoot").unwrap().as_reference().unwrap();
    let structure=saved.get_dictionary(structure_id).unwrap();
    let children=saved.dereference(structure.get(b"K").unwrap()).unwrap().1.as_array().unwrap();assert_eq!(children.len(),3);
    let parents=saved.dereference(structure.get(b"ParentTree").unwrap()).unwrap().1.as_dict().unwrap();
    let numbers=parents.get(b"Nums").unwrap().as_array().unwrap();assert_eq!(numbers.len(),6);
    for index in 0..2 {
        let element=saved.dereference(&children[index]).unwrap().1.as_dict().unwrap();
        assert_eq!(element.get(b"S").unwrap().as_name().unwrap(),b"P");assert_eq!(element.get(b"K").unwrap().as_i64().unwrap(),0);
        assert_eq!(element.get(b"Pg").unwrap().as_reference().unwrap(),pages[&(index as u32+1)]);
        let parent_key=saved.get_dictionary(pages[&(index as u32+1)]).unwrap().get(b"StructParents").unwrap().as_i64().unwrap();
        let pair=numbers.chunks_exact(2).find(|pair|pair[0].as_i64().unwrap()==parent_key).unwrap();
        assert_eq!(saved.dereference(&pair[1]).unwrap().1.as_array().unwrap()[0],children[index]);
    }
    let annotations=saved.get_dictionary(pages[&1]).unwrap().get(b"Annots").unwrap().as_array().unwrap();assert_eq!(annotations.len(),1);
    let annotation=annotations[0].as_reference().unwrap();let drawing=saved.get_dictionary(annotation).unwrap();
    assert_eq!(drawing.get(b"Subtype").unwrap().as_name().unwrap(),b"FreeText");
    assert_eq!(drawing.get(b"P").unwrap().as_reference().unwrap(),pages[&1]);
    let key=drawing.get(b"StructParent").unwrap().as_i64().unwrap();
    let pair=numbers.chunks_exact(2).find(|pair|pair[0].as_i64().unwrap()==key).unwrap();assert_eq!(pair[1],children[2]);
    let owner=saved.dereference(&children[2]).unwrap().1.as_dict().unwrap();
    assert_eq!(owner.get(b"P").unwrap().as_reference().unwrap(),structure_id);assert_eq!(owner.get(b"S").unwrap().as_name().unwrap(),b"Annot");
    assert_eq!(owner.get(b"Pg").unwrap().as_reference().unwrap(),pages[&1]);
    let language=owner.get(b"Lang").unwrap_or_else(|_|catalog.get(b"Lang").unwrap());assert_eq!(lopdf::decode_text_string(language).unwrap(),"en-US");
    let kid=saved.dereference(owner.get(b"K").unwrap()).unwrap().1;
    let kid=if let Object::Array(items)=kid {assert_eq!(items.len(),1);saved.dereference(&items[0]).unwrap().1}else{kid};
    let objr=kid.as_dict().unwrap();assert_eq!(objr.get(b"Type").unwrap().as_name().unwrap(),b"OBJR");assert_eq!(objr.get(b"Obj").unwrap().as_reference().unwrap(),annotation);

    let reopened=engine.open_document(&output).unwrap();let reopened_plan=request(&reopened);
    assert_eq!(reopened.pages[0].overlays.len(),1);assert!(reopened.pages[1].overlays.is_empty());
    assert!(matches!(&reopened.pages[0].overlays[0],folio_engine::Overlay::Text(text) if text.text=="Added"));
    for index in 0..2 {
        assert_eq!((reopened.pages[index].width,reopened.pages[index].height),(source.pages[index].width,source.pages[index].height));
        assert_eq!(engine.render_page(&reopened.id,index,400).unwrap(),engine.render_page(&source.id,index,400).unwrap(),"original background {index}");
        assert_eq!(engine.extract_text(&reopened.id,index).unwrap(),engine.extract_text(&source.id,index).unwrap());
    }
    assert_eq!(engine.render_page_plan(reopened_plan.pages[0].clone(),400).unwrap(),expected);
    assert_eq!(fs::read(&input).unwrap(),before);
    if let Some(directory)=std::env::var_os("FOLIO_STRUCTURED_OVERLAY_CAPTURE_DIR") {
        use std::io::Write;
        let directory=PathBuf::from(directory);fs::create_dir_all(&directory).unwrap();
        for (name,bytes) in [("complete-source-original.pdf",before),("complete-source-added.pdf",fs::read(&output).unwrap()),("complete-source-expected.png",expected)] {
            fs::OpenOptions::new().write(true).create_new(true).open(directory.join(name)).unwrap().write_all(&bytes).unwrap();
        }
    }
    engine.close_document(&reopened.id).unwrap();engine.close_document(&source.id).unwrap();
}

#[test]
fn cancelling_source_preservation_leaves_original_and_destination_untouched() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("source.pdf");
    let before=fixture(&input,true,false);let source=engine.open_document(&input).unwrap();
    let output=temp.path().join("saved.pdf");fs::write(&output,b"keep destination").unwrap();
    let control=Arc::new(PdfSaveControl::new());let cancel=control.clone();
    let observed=Arc::new(AtomicBool::new(false));let seen=observed.clone();
    let report=Arc::new(move |event:PdfSaveProgress| {if event.stage=="preserving" {seen.store(true,Ordering::Relaxed);cancel.cancel();}});
    assert!(!engine.export_pdf_controlled(request(&source),None,&output,control,report).unwrap());
    assert!(observed.load(Ordering::Relaxed));assert_eq!(fs::read(output).unwrap(),b"keep destination");assert_eq!(fs::read(input).unwrap(),before);
}

#[test]
fn source_graph_save_still_protects_output_and_enforces_reader_rights() {
    use lopdf::encryption::Permissions;
    let engine=engine();let temp=tempfile::tempdir().unwrap();let plain=temp.path().join("plain.pdf");
    let bytes=fixture(&plain,true,false);
    let encrypted=protected_fixture::encrypt(&bytes,Permissions::PRINTABLE | Permissions::FILLABLE);
    let input=temp.path().join("protected.pdf");fs::write(&input,&encrypted).unwrap();
    let reader=engine.open_document_with_password(&input,"reader").unwrap();
    let output=temp.path().join("saved.pdf");engine.export_pdf(request(&reader),&output).unwrap();
    assert!(engine.open_document(&output).is_err());
    let reopened=engine.open_document_with_password(&output,"reader").unwrap();
    assert!(!reopened.protection.as_ref().unwrap().permissions.edit_content);
    let unlocked=folio_engine::protection::unlock_pdf(&fs::read(&output).unwrap(),Some("reader")).unwrap();
    let saved=Document::load_mem(&unlocked.bytes).unwrap();
    let original=Document::load_mem(&folio_engine::protection::unlock_pdf(&encrypted,Some("reader")).unwrap().bytes).unwrap();
    for key in [b"StructTreeRoot".as_slice(),b"MarkInfo",b"Lang",b"Metadata",b"ViewerPreferences",b"Outlines"] {
        same_object(saved.catalog().unwrap().get(key).unwrap(),original.catalog().unwrap().get(key).unwrap());
    }
    for (id,object) in &original.objects {
        // Encryption dictionaries and cross-reference objects are codec-owned.
        if object.as_dict().is_ok_and(|d|d.has(b"Filter")&&d.has(b"O")) {continue;}
        if let Some(actual)=saved.objects.get(id) {same_object(object,actual);}
    }
    let denied=temp.path().join("denied.pdf");fs::write(&denied,b"keep destination").unwrap();
    let mut changed=request(&reader);changed.pages[0].rotation=90;
    assert!(engine.export_pdf(changed,&denied).is_err());
    assert!(engine.export_pdf_with_security(request(&reader),None,&denied,folio_engine::ProtectionChange::Remove{},Arc::new(PdfSaveControl::new()),Arc::new(|_|{})).is_err());
    assert_eq!(fs::read(&denied).unwrap(),b"keep destination");
    let owner=engine.open_document_with_password(&input,"owner").unwrap();
    assert!(engine.export_pdf_with_security(request(&owner),None,&denied,folio_engine::ProtectionChange::Remove{},Arc::new(PdfSaveControl::new()),Arc::new(|_|{})).unwrap());
    assert!(engine.open_document(&denied).unwrap().protection.is_none());
    assert!(Document::load(&denied).unwrap().catalog().unwrap().has(b"StructTreeRoot"));
    assert_eq!(fs::read(input).unwrap(),encrypted);
}

#[test]
fn ordinary_composition_still_applies_overlays_and_page_changes() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("source.pdf");
    fixture(&input,false,false);let source=engine.open_document(&input).unwrap();
    for case in ["subset","reorder","duplicate","mixed","overlay"] {
        let mut plan=request(&source);match case {
            "subset"=>{plan.pages.pop();},
            "reorder"=>plan.pages.reverse(),
            "duplicate"=>plan.pages[1].page_index=0,
            "mixed"=>{let second=engine.open_document(&input).unwrap();plan.pages[1].source_id=second.id;},
            _=>plan.pages[0].overlays=serde_json::from_value(json!([{"type":"text","id":"new","x":30,"y":30,"text":"Added","fontSize":14,"fontName":"Helvetica","color":"#000000","rotation":0}])).unwrap(),
        }
        let count=plan.pages.len();let output=temp.path().join(format!("{case}.pdf"));engine.export_pdf(plan,&output).unwrap();
        let reopened=engine.open_document(&output).unwrap();assert_eq!(reopened.pages.len(),count);
        if case=="overlay" {assert_eq!(reopened.pages[0].overlays.len(),1);}
        else {
            let expected=if case=="reorder" {1}else{0};
            assert_eq!(engine.render_page(&source.id,expected,400).unwrap(),engine.render_page(&reopened.id,0,400).unwrap());
        }
    }
}

#[test]
fn all_guarded_catalog_relationships_survive_whole_saves_and_refuse_partial_import() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for key in ["StructTreeRoot","MarkInfo","OCProperties","OutputIntents"] {
        let input=temp.path().join(format!("{key}.pdf"));fixture(&input,false,false);
        let mut pdf=Document::load(&input).unwrap();
        pdf.catalog_mut().unwrap().set(key,dictionary!{});pdf.save(&input).unwrap();
        let before=fs::read(&input).unwrap();let source=engine.open_document(&input).unwrap();
        let output=temp.path().join(format!("{key}-whole.pdf"));engine.export_pdf(request(&source),&output).unwrap();
        assert!(fs::read(output).unwrap()==before,"{key}");
        let mut partial=request(&source);partial.pages.pop();
        let output=temp.path().join(format!("{key}-partial.pdf"));assert!(engine.export_pdf(partial,&output).is_err());assert!(!output.exists());
    }
}

#[test]
fn shared_source_page_dictionary_cannot_apply_one_rotation_to_both_occurrences() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("aliased.pdf");
    fixture(&input,true,false);let mut pdf=Document::load(&input).unwrap();
    let pages=pdf.get_pages();let tree=pdf.catalog().unwrap().get(b"Pages").unwrap().as_reference().unwrap();
    pdf.get_dictionary_mut(tree).unwrap().set("Kids",vec![Object::Reference(pages[&1]),Object::Reference(pages[&1])]);
    pdf.catalog_mut().unwrap().remove(b"Outlines");
    pdf.save(&input).unwrap();let source=engine.open_document(&input).unwrap();
    assert_eq!(source.pages.len(),2);
    let mut plan=request(&source);plan.pages[0].rotation=90;
    let output=temp.path().join("saved.pdf");
    assert!(engine.export_pdf(plan,&output).is_err(),"A transform must not mutate another page occurrence");
    assert!(!output.exists());
}
