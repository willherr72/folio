use folio_engine::{DocumentInfo,ExportRequest,PagePlan,PdfEngine,PdfSaveControl,PdfSaveProgress};
use lopdf::{dictionary,Document,Object,Stream};
use std::{fs,path::{Path,PathBuf},sync::{Arc,atomic::{AtomicBool,Ordering}}};
#[path="support/protected_fixture.rs"]
mod protected_fixture;
#[path="support/structured_navigation_save.rs"]
mod navigation_save;
#[path="support/structured_projection_save.rs"]
mod projection_save;
#[path="support/structured_form_save.rs"]
mod form_save;
#[path="support/structured_overlay_save.rs"]
mod overlay_save;

fn engine()->PdfEngine {PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path,tagged:bool,label:&str)->Vec<u8>{
    let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();let structure=pdf.new_object_id();
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let image=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>1,"Height"=>1,"BitsPerComponent"=>8,"ColorSpace"=>"DeviceRGB"},vec![20,80,160]));
    let resources=pdf.add_object(dictionary!{"Font"=>dictionary!{"F1"=>font},"XObject"=>dictionary!{"First"=>image,"Prev"=>image,"Parent"=>image}});
    let mut kids=vec![];let mut elements=vec![];let mut nums=vec![];
    for index in 0..2 {
        let content=pdf.add_object(Stream::new(dictionary!{},format!("/P <</MCID 0>> BDC BT /F1 14 Tf 40 300 Td ({label} page {index}) Tj ET EMC /Artifact BMC q 40 0 0 40 40 200 cm /First Do Q EMC").into_bytes()));
        let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"Contents"=>content,"Resources"=>resources,"UserUnit"=>1.7,"Group"=>dictionary!{"Type"=>"Group","S"=>"Transparency","CS"=>"DeviceRGB"}});
        if tagged {
            pdf.get_dictionary_mut(page).unwrap().set("StructParents",index);
            let element=pdf.add_object(dictionary!{"Type"=>"StructElem","S"=>"P","P"=>structure,"Pg"=>page,"K"=>0,"Alt"=>Object::string_literal(format!("{label} alternate {index}"))});
            elements.push(Object::Reference(element));nums.extend([Object::Integer(index),Object::Array(vec![element.into()])]);
        }
        kids.push(Object::Reference(page));
    }
    let link=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Link","Rect"=>vec![30.into(),40.into(),90.into(),60.into()],"Dest"=>vec![kids[1].clone(),Object::Name(b"Fit".to_vec())]});
    let note=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![20.into(),20.into(),40.into(),40.into()],"Contents"=>Object::string_literal(format!("{label} native note")),"NM"=>Object::string_literal("native-note")});
    pdf.get_dictionary_mut(kids[0].as_reference().unwrap()).unwrap().set("Annots",vec![Object::Reference(link),note.into()]);
    if tagged {
        pdf.get_dictionary_mut(link).unwrap().set("StructParent",2);
        let element=elements[0].as_reference().unwrap();
        pdf.get_dictionary_mut(element).unwrap().set("K",vec![0.into(),Object::Dictionary(dictionary!{"Type"=>"OBJR","Obj"=>link})]);
        nums.extend([2.into(),element.into()]);
        let parent=pdf.add_object(dictionary!{"Nums"=>nums});
        pdf.objects.insert(structure,dictionary!{"Type"=>"StructTreeRoot","K"=>elements,"ParentTree"=>parent}.into());
    }
    pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>kids,"Count"=>2,"MediaBox"=>vec![10.into(),20.into(),310.into(),420.into()],"Rotate"=>90}.into());
    let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages,"Lang"=>Object::string_literal("en-US")});
    if tagged {pdf.get_dictionary_mut(root).unwrap().set("StructTreeRoot",structure);pdf.get_dictionary_mut(root).unwrap().set("MarkInfo",dictionary!{"Marked"=>true});}
    pdf.trailer.set("Root",root);pdf.save(path).unwrap();fs::read(path).unwrap()
}
fn request(sources:&[&DocumentInfo])->ExportRequest{
    ExportRequest{flatten:false,pages:sources.iter().flat_map(|source|source.pages.iter().enumerate().map(|(index,page)|PagePlan{
        id:format!("{}-{index}",source.id),source_id:source.id.clone(),page_index:index,width:page.width,height:page.height,rotation:0,crop:None,overlays:page.overlays.clone(),
    })).collect()}
}
fn check_structure(document:&Document,owners:usize){
    let root=document.catalog().unwrap().get(b"StructTreeRoot").unwrap().as_reference().unwrap();
    let root=document.get_dictionary(root).unwrap();
    let parent=document.dereference(root.get(b"ParentTree").unwrap()).unwrap().1.as_dict().unwrap();
    let entries=parent.get(b"Nums").unwrap().as_array().unwrap();assert_eq!(entries.len(),owners*2);
    assert!(!document.catalog().unwrap().get(b"MarkInfo").unwrap().as_dict().unwrap().get(b"Marked").unwrap().as_bool().unwrap());
}

#[test]
fn structured_save_alias_source_is_refused_before_publication_by_current_renderer() {
    let engine=engine();let dir=tempfile::tempdir().unwrap();
    let first_path=dir.path().join("alias.pdf");fixture(&first_path,true,"Alias");
    let second_path=dir.path().join("plain.pdf");fixture(&second_path,false,"Plain");
    let mut doc=Document::load(&first_path).unwrap();
    let root=doc.catalog().unwrap().get(b"Pages").unwrap().as_reference().unwrap();
    let first=doc.get_dictionary(root).unwrap().get(b"Kids").unwrap().as_array().unwrap()[0].clone();
    let alias=doc.add_object(first);
    doc.get_dictionary_mut(root).unwrap().get_mut(b"Kids").unwrap().as_array_mut().unwrap()[0]=alias.into();
    doc.save(&first_path).unwrap();
    let before=fs::read(&first_path).unwrap();let output=dir.path().join("saved.pdf");fs::write(&output,b"keep").unwrap();
    assert!(engine.open_document(&first_path).is_err());
    assert_eq!(fs::read(&first_path).unwrap(),before);assert_eq!(fs::read(output).unwrap(),b"keep");
}

#[test]
fn structured_save_keeps_source_pixels_text_links_comments_and_resource_aliases(){
    let engine=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a.pdf");let b=dir.path().join("b.pdf");
    let before_a=fixture(&a,true,"First");let before_b=fixture(&b,true,"Second");
    let first=engine.open_document(&a).unwrap();let second=engine.open_document(&b).unwrap();
    let plan=request(&[&first,&second]);assert!(plan.pages.iter().all(|page|page.overlays.is_empty()));
    let output=dir.path().join("combined.pdf");engine.export_pdf(plan,&output).unwrap();
    let saved=Document::load(&output).unwrap();check_structure(&saved,6);
    let pages=saved.get_pages();assert_eq!(pages.len(),4);
    let reopened=engine.open_document(&output).unwrap();
    for (block,source) in [&first,&second].iter().enumerate(){
        for index in 0..2 {
            assert_eq!(engine.render_page(&source.id,index,400).unwrap(),engine.render_page(&reopened.id,block*2+index,400).unwrap());
            assert_eq!(engine.extract_text(&source.id,index).unwrap(),engine.extract_text(&reopened.id,block*2+index).unwrap());
        }
        let page=saved.get_dictionary(pages[&(block as u32*2+1)]).unwrap();
        let resources=saved.get_dictionary(page.get(b"Resources").unwrap().as_reference().unwrap()).unwrap();
        let images=resources.get(b"XObject").unwrap().as_dict().unwrap();
        assert_eq!(images.get(b"First").unwrap(),images.get(b"Prev").unwrap());assert_eq!(images.get(b"First").unwrap(),images.get(b"Parent").unwrap());
        let annotations=page.get(b"Annots").unwrap().as_array().unwrap();assert_eq!(annotations.len(),2);
        let link=saved.get_dictionary(annotations[0].as_reference().unwrap()).unwrap();
        assert_eq!(link.get(b"Dest").unwrap().as_array().unwrap()[0],Object::Reference(pages[&(block as u32*2+2)]));
        let note=saved.get_dictionary(annotations[1].as_reference().unwrap()).unwrap();assert_eq!(note.get(b"Subtype").unwrap().as_name().unwrap(),b"Text");
    }
    let again=dir.path().join("saved-again.pdf");engine.export_pdf(request(&[&reopened]),&again).unwrap();assert_eq!(fs::read(again).unwrap(),fs::read(output).unwrap());
    assert_eq!(fs::read(a).unwrap(),before_a);assert_eq!(fs::read(b).unwrap(),before_b);
}

#[test]
fn structured_save_applies_physical_crop_rotation_and_explicit_bookmarks(){
    let engine=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a.pdf");let b=dir.path().join("b.pdf");fixture(&a,true,"First");fixture(&b,false,"Second");
    let first=engine.open_document(&a).unwrap();let second=engine.open_document(&b).unwrap();
    let mut plan=request(&[&first,&second]);plan.pages[0].rotation=90;
    plan.pages[0].crop=Some(serde_json::from_value(serde_json::json!({"x":34,"y":51,"width":170,"height":255})).unwrap());
    let golden=dir.path().join("single-crop.pdf");let mut single=request(&[&first]);single.pages[0]=plan.pages[0].clone();engine.export_pdf(single,&golden).unwrap();
    let golden=engine.open_document(golden).unwrap();
    let bookmark=folio_engine::DocumentBookmark{id:"new".into(),title:"Second source".into(),level:0,destination:Some(folio_engine::PdfValue::Array(vec![folio_engine::PdfValue::Page(plan.pages[2].id.clone()),folio_engine::PdfValue::Name(b"Fit".to_vec())])),action:None,bold:true,italic:false,closed:false,color:Some([0.1,0.2,0.3])};
    let output=dir.path().join("changed.pdf");engine.export_pdf_with_bookmarks(plan,Some(vec![bookmark]),&output).unwrap();
    let saved=Document::load(&output).unwrap();check_structure(&saved,3);
    let reopened=engine.open_document(&output).unwrap();
    assert_eq!(engine.render_page(&golden.id,0,400).unwrap(),engine.render_page(&reopened.id,0,400).unwrap());
    assert_eq!(engine.render_page(&second.id,0,400).unwrap(),engine.render_page(&reopened.id,2,400).unwrap());
    assert!((reopened.pages[0].width-255.).abs()<0.001 && (reopened.pages[0].height-170.).abs()<0.001);
    assert_eq!(reopened.bookmarks[0].title,"Second source");assert!(reopened.bookmarks[0].bold);
    let outline=saved.get_dictionary(saved.catalog().unwrap().get(b"Outlines").unwrap().as_reference().unwrap()).unwrap();
    let first=saved.get_dictionary(outline.get(b"First").unwrap().as_reference().unwrap()).unwrap();
    assert_eq!(first.get(b"Dest").unwrap().as_array().unwrap()[0],Object::Reference(saved.get_pages()[&3]));
}

#[test]
fn structured_save_cancellation_preserves_destinations_and_inputs(){
    let engine=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a.pdf");let b=dir.path().join("b.pdf");let before=fixture(&a,true,"First");fixture(&b,false,"Second");
    let first=engine.open_document(&a).unwrap();let second=engine.open_document(&b).unwrap();
    for stage in ["checking","copying","preserving","applying","preparingFile"] {
        let output=dir.path().join(format!("cancel-{stage}.pdf"));fs::write(&output,b"keep destination").unwrap();
        let control=Arc::new(PdfSaveControl::new());let cancel=control.clone();let observed=Arc::new(AtomicBool::new(false));let seen=observed.clone();
        let report=Arc::new(move|event:PdfSaveProgress|{if event.stage==stage {seen.store(true,Ordering::SeqCst);cancel.cancel();}});
        assert!(!engine.export_pdf_controlled(request(&[&first,&second]),None,&output,control,report).unwrap());
        assert!(observed.load(Ordering::SeqCst),"missing stage {stage}");assert_eq!(fs::read(output).unwrap(),b"keep destination");
    }
    assert_eq!(fs::read(a).unwrap(),before);
}

#[test]
fn structured_save_refuses_unadapted_graphs_without_loss(){
    let engine=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a.pdf");let b=dir.path().join("b.pdf");fixture(&a,true,"First");fixture(&b,false,"Second");
    let first=engine.open_document(&a).unwrap();
    for key in ["AcroForm","OCProperties","OutputIntents"] {
        let input=dir.path().join(format!("{key}.pdf"));let mut document=Document::load(&b).unwrap();document.catalog_mut().unwrap().set(key,if key=="AcroForm"{dictionary!{"UnsupportedExtension"=>true}}else{dictionary!{}});document.save(&input).unwrap();
        let other=engine.open_document(input).unwrap();let output=dir.path().join(format!("{key}-refused.pdf"));fs::write(&output,b"keep").unwrap();
        assert!(engine.export_pdf_with_bookmarks(request(&[&first,&other]),Some(vec![]),&output).is_err(),"{key}");assert_eq!(fs::read(output).unwrap(),b"keep");
    }
}

#[test]
fn structured_save_enforces_assembly_rights_and_preserves_output_protection(){
    let engine=engine();let dir=tempfile::tempdir().unwrap();let plain=dir.path().join("plain.pdf");let bytes=fixture(&plain,true,"Protected");
    let input=dir.path().join("protected.pdf");let encrypted=protected_fixture::encrypt(&bytes,lopdf::encryption::Permissions::PRINTABLE);fs::write(&input,&encrypted).unwrap();
    let other=engine.open_document(&plain).unwrap();let reader=engine.open_document_with_password(&input,"reader").unwrap();
    let output=dir.path().join("combined.pdf");fs::write(&output,b"keep").unwrap();assert!(engine.export_pdf(request(&[&reader,&other]),&output).is_err());assert_eq!(fs::read(&output).unwrap(),b"keep");
    let owner=engine.open_document_with_password(&input,"owner").unwrap();engine.export_pdf(request(&[&owner,&other]),&output).unwrap();
    assert!(engine.open_document(&output).is_err());let reopened=engine.open_document_with_password(&output,"reader").unwrap();assert_eq!(reopened.pages.len(),4);
    assert!(!reopened.protection.as_ref().unwrap().permissions.assemble);
    assert_eq!(engine.render_page(&other.id,0,400).unwrap(),engine.render_page(&reopened.id,2,400).unwrap());
    assert_eq!(fs::read(input).unwrap(),encrypted);
}

#[test]
#[ignore="requires explicit authored inputs and new native Save output"]
fn structured_save_authored_word_pictures(){
    let engine=engine();let input=PathBuf::from(std::env::var_os("FOLIO_STRUCTURE_CORPUS").unwrap());let output=PathBuf::from(std::env::var_os("FOLIO_STRUCTURED_SAVE_OUTPUT").unwrap());assert!(!output.exists());
    let word=engine.open_document(input.join("Word.pdf")).unwrap();let pictures=engine.open_document(input.join("Pictures.pdf")).unwrap();
    engine.export_pdf_controlled(request(&[&word,&pictures]),None,&output,Arc::new(PdfSaveControl::new()),Arc::new(|_|{})).unwrap();
    let reopened=engine.open_document(&output).unwrap();assert_eq!(reopened.pages.len(),4);
    for (offset,source) in [(0,&word),(2,&pictures)] {for index in 0..2 {assert_eq!(engine.render_page(&source.id,index,900).unwrap(),engine.render_page(&reopened.id,offset+index,900).unwrap());assert_eq!(engine.extract_text(&source.id,index).unwrap(),engine.extract_text(&reopened.id,offset+index).unwrap());}}
}

#[test]
fn structured_save_version_promotion_refuses_old_annotation_language_without_publication(){
    let engine=engine();let dir=tempfile::tempdir().unwrap();
    let old_path=dir.path().join("old.pdf");fixture(&old_path,true,"Old");
    let new_path=dir.path().join("new.pdf");fixture(&new_path,true,"New");
    let mut old=Document::load(&old_path).unwrap();let page=old.get_pages()[&1];
    let note=old.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap()[1].as_reference().unwrap();
    old.get_dictionary_mut(note).unwrap().set("Lang",Object::string_literal("fr"));old.save(&old_path).unwrap();
    let mut new=Document::load(&new_path).unwrap();new.version="2.0".into();new.save(&new_path).unwrap();
    let before_old=fs::read(&old_path).unwrap();let before_new=fs::read(&new_path).unwrap();
    let a=engine.open_document(&old_path).unwrap();let b=engine.open_document(&new_path).unwrap();
    for (index,sources) in [[&a,&b],[&b,&a]].into_iter().enumerate(){
        let target=dir.path().join(format!("refused-{index}.pdf"));fs::write(&target,b"keep existing output").unwrap();
        assert!(engine.export_pdf(request(&sources),&target).unwrap_err().to_string().contains("annotation language"));
        assert_eq!(fs::read(&target).unwrap(),b"keep existing output");
        assert_eq!(engine.extract_text(&a.id,0).unwrap().contains("Old"),true);
    }
    assert_eq!(fs::read(old_path).unwrap(),before_old);assert_eq!(fs::read(new_path).unwrap(),before_new);
}
