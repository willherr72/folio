use folio_engine::{FillFormFieldRequest, FormFieldValue, PdfEngine};
use lopdf::{dictionary, Document, Object, Stream};
use std::{fs, path::PathBuf};
#[path="support/protected_fixture.rs"]
mod protected_fixture;

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()
}
fn bytes(doc: &mut Document) -> Vec<u8> {
    let mut bytes = Vec::new(); doc.save_to(&mut bytes).unwrap(); bytes
}
fn fixture(incoming: bool) -> Document {
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let font = doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>if incoming {"Courier"} else {"Helvetica"}});
    let resources = dictionary! {"Font"=>dictionary! {"F1"=>font}};
    let mut ids = Vec::new();
    for text in [if incoming {"New content"} else {"Old content"}, "Untouched page"] {
        let contents = doc.add_object(Stream::new(dictionary! {}, format!("BT /F1 18 Tf 40 240 Td ({text}) Tj ET").into_bytes()));
        ids.push(doc.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>resources.clone(),"Contents"=>contents}));
    }
    // Intentionally omit AP resources: they must keep the OLD /F1 binding.
    let ap = doc.add_object(Stream::new(dictionary! {"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),180.into(),26.into()]}, b"BT /F1 12 Tf 4 8 Td (Keep field) Tj ET".to_vec()));
    let widget = doc.add_object(dictionary! {"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>Object::string_literal("retained"),"V"=>Object::string_literal("Keep field"),"DA"=>Object::string_literal("/F1 12 Tf 0 g"),"Rect"=>vec![30.into(),45.into(),210.into(),71.into()],"F"=>4,"P"=>ids[0],"AP"=>dictionary! {"N"=>ap}});
    let note = doc.add_object(dictionary! {"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![240.into(),50.into(),258.into(),68.into()],"Contents"=>Object::string_literal("Keep review"),"F"=>4,"P"=>ids[0]});
    let link = doc.add_object(dictionary! {"Type"=>"Annot","Subtype"=>"Link","Rect"=>vec![30.into(),90.into(),180.into(),110.into()],"Border"=>vec![0.into(),0.into(),0.into()],"P"=>ids[0],"A"=>dictionary! {"S"=>"URI","URI"=>Object::string_literal("https://example.invalid/retained")}});
    doc.get_dictionary_mut(ids[0]).unwrap().set("Annots",vec![Object::Reference(widget),Object::Reference(note),Object::Reference(link)]);
    let form = doc.add_object(dictionary! {"Fields"=>vec![Object::Reference(widget)],"DA"=>Object::string_literal("/F1 12 Tf 0 g"),"DR"=>resources});
    let outlines = doc.new_object_id();
    let bookmark = doc.add_object(dictionary! {"Title"=>Object::string_literal("Keep target"),"Parent"=>outlines,"Dest"=>vec![Object::Reference(ids[0]),Object::Name(b"Fit".to_vec())]});
    doc.objects.insert(outlines,dictionary! {"Type"=>"Outlines","First"=>bookmark,"Last"=>bookmark,"Count"=>1}.into());
    doc.objects.insert(pages,dictionary! {"Type"=>"Pages","Kids"=>ids.iter().map(|p|Object::Reference(*p)).collect::<Vec<_>>(),"Count"=>2}.into());
    let root = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages,"AcroForm"=>form,"Outlines"=>outlines});
    doc.trailer.set("Root",root); doc
}

#[test]
fn replaces_content_retaining_interactions_and_editability() {
    let temp = tempfile::tempdir().unwrap(); let engine = engine();
    let a = bytes(&mut fixture(false)); let b = bytes(&mut fixture(true));
    let target_path = temp.path().join("target.pdf"); let incoming_path = temp.path().join("incoming.pdf");
    fs::write(&target_path,&a).unwrap(); fs::write(&incoming_path,&b).unwrap();
    let target = engine.open_document(&target_path).unwrap();
    let incoming = engine.open_document(&incoming_path).unwrap();
    let result = engine.replace_page_content(&target.id,0,&incoming.id,0).unwrap();
    assert_ne!(result.id,target.id); assert_eq!(result.pages,target.pages);
    let mut old_bookmarks=target.bookmarks.clone(); let mut new_bookmarks=result.bookmarks.clone();
    old_bookmarks.iter_mut().chain(new_bookmarks.iter_mut()).for_each(|b|b.id.clear());
    assert_eq!(old_bookmarks,new_bookmarks);
    let text = engine.extract_text(&result.id,0).unwrap();
    assert!(text.contains("New content")); assert!(!text.contains("Old content"));
    assert_eq!(engine.render_page(&result.id,1,600).unwrap(),engine.render_page(&target.id,1,600).unwrap());
    assert_eq!(serde_json::to_value(engine.list_form_fields(&result.id,0).unwrap()).unwrap(),serde_json::to_value(engine.list_form_fields(&target.id,0).unwrap()).unwrap());
    assert_eq!(serde_json::to_value(engine.list_native_comments(&result.id).unwrap()).unwrap(),serde_json::to_value(engine.list_native_comments(&target.id).unwrap()).unwrap());
    let runs = engine.list_text_runs(&result.id,0).unwrap();
    assert_eq!(runs.runs.len(),1); assert!(runs.runs[0].supported,"{:?}",runs);
    let edited=engine.replace_text(&result.id,0,runs.runs[0].object_index,"New content","New wording").unwrap();
    assert!(engine.extract_text(&edited.id,0).unwrap().contains("New wording"));
    let field=engine.list_form_fields(&result.id,0).unwrap().remove(0);
    let filled = engine.fill_form_field(&result.id,FillFormFieldRequest {page_index:0,annotation_index:field.annotation_index,expected_value:field.value,value:FormFieldValue::Text{text:"Still editable".into()}}).unwrap();
    assert_eq!(engine.list_form_fields(&filled.id,0).unwrap()[0].value,FormFieldValue::Text{text:"Still editable".into()});
    let output=engine.source_bytes(&result.id).unwrap(); let output=Document::load_mem(&output).unwrap();
    let original=Document::load_mem(&a).unwrap();
    assert_eq!(output.get_pages(),original.get_pages());
    let page=*output.get_pages().get(&1).unwrap();
    let annotations=output.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap();
    let original_annotations=original.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap();
    assert_eq!(annotations,original_annotations);
    let link=annotations[2].as_reference().unwrap(); assert_eq!(output.get_object(link).unwrap(),original.get_object(link).unwrap());
    assert_eq!(fs::read(target_path).unwrap(),a); assert_eq!(fs::read(incoming_path).unwrap(),b);
    for info in [&edited,&filled,&result,&incoming,&target] {engine.close_document(&info.id).unwrap();}
}

fn open(engine:&PdfEngine,temp:&tempfile::TempDir,name:&str,doc:&mut Document)->folio_engine::DocumentInfo {
    let path=temp.path().join(name);fs::write(&path,bytes(doc)).unwrap();engine.open_document(path).unwrap()
}
fn first(doc:&Document)->lopdf::ObjectId {*doc.get_pages().get(&1).unwrap()}
fn field_pixels(engine:&PdfEngine,info:&folio_engine::DocumentInfo)->Vec<u8> {
    let field=engine.list_form_fields(&info.id,0).unwrap().remove(0);
    let bounds=field.bounds.unwrap();let png=engine.render_page(&info.id,0,600).unwrap();
    let image=image::load_from_memory(&png).unwrap().to_rgba8();
    let ratio=image.width() as f32/info.pages[0].width;
    let (x,y)=((bounds.x*ratio).floor() as u32,(bounds.y*ratio).floor() as u32);
    let (right,bottom)=(((bounds.x+bounds.width)*ratio).ceil() as u32,((bounds.y+bounds.height)*ratio).ceil() as u32);
    image::imageops::crop_imm(&image,x,y,(right-x).min(image.width()-x),(bottom-y).min(image.height()-y)).to_image().into_raw()
}

#[test]
fn rotated_and_scaled_fits_keep_annotation_pixels_and_page_geometry() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for target_rotation in [0,90,180,270] {for incoming_rotation in [0,90,180,270] {for unit in [1.,1.7] {
        let label=format!("d{target_rotation}-s{incoming_rotation}-u{unit}");
        let mut target=fixture(false);let page=first(&target);
        target.get_dictionary_mut(page).unwrap().set("Rotate",target_rotation);
        target.get_dictionary_mut(page).unwrap().set("UserUnit",Object::Real(unit));
        target.get_dictionary_mut(page).unwrap().set("CropBox",vec![10.into(),20.into(),290.into(),380.into()]);
        let mut incoming=fixture(true);let page=first(&incoming);
        incoming.get_dictionary_mut(page).unwrap().set("Rotate",incoming_rotation);
        incoming.get_dictionary_mut(page).unwrap().set("CropBox",vec![10.into(),20.into(),290.into(),380.into()]);
        let target=open(&engine,&temp,&format!("{label}-target.pdf"),&mut target);
        let incoming=open(&engine,&temp,&format!("{label}-incoming.pdf"),&mut incoming);
        let old_field=field_pixels(&engine,&target);
        let result=engine.replace_page_content(&target.id,0,&incoming.id,0).unwrap();
        assert_eq!(result.pages,target.pages,"{label}");assert_eq!(field_pixels(&engine,&result),old_field,"{label}");
        assert!(engine.extract_text(&result.id,0).unwrap().contains("New content"),"{label}");
        assert_eq!(engine.render_page(&target.id,1,600).unwrap(),engine.render_page(&result.id,1,600).unwrap());
        if let Some(root)=std::env::var_os("FOLIO_PAGE_REPLACEMENT_OUTPUT") {
            let root=PathBuf::from(root);fs::create_dir_all(&root).unwrap();
            use std::io::Write;
            for (suffix,info) in [("target",&target),("incoming",&incoming),("result",&result)] {
                fs::OpenOptions::new().write(true).create_new(true).open(root.join(format!("{label}-{suffix}.pdf"))).unwrap().write_all(&engine.source_bytes(&info.id).unwrap()).unwrap();
            }
        }
        for info in [&result,&target,&incoming] {engine.close_document(&info.id).unwrap();}
    }}}
}

#[test]
fn repeated_replacement_prunes_old_imports_and_appearance_clones() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let mut target=open(&engine,&temp,"target.pdf",&mut fixture(false));
    let incoming=open(&engine,&temp,"incoming.pdf",&mut fixture(true));let mut count=None;
    for _ in 0..12 {
        let result=engine.replace_page_content(&target.id,0,&incoming.id,0).unwrap();
        let pdf=Document::load_mem(&engine.source_bytes(&result.id).unwrap()).unwrap();
        if let Some(count)=count {assert_eq!(pdf.objects.len(),count);} else {count=Some(pdf.objects.len());}
        engine.close_document(&target.id).unwrap();target=result;
    }
    engine.close_document(&target.id).unwrap();engine.close_document(&incoming.id).unwrap();
}

#[test]
fn shared_annotations_and_document_linked_content_are_rejected() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let incoming=open(&engine,&temp,"incoming.pdf",&mut fixture(true));
    for kind in ["shared","tagged","layered","group","missing-dr","missing-default-font"] {
        let mut target=fixture(false);let page=first(&target);
        match kind {
            "shared"=>{let annotations=target.get_dictionary(page).unwrap().get(b"Annots").unwrap().clone();let second=*target.get_pages().get(&2).unwrap();target.get_dictionary_mut(second).unwrap().set("Annots",annotations);},
            "tagged"=>target.catalog_mut().unwrap().set("StructTreeRoot",dictionary! {}),
            "layered"=>target.catalog_mut().unwrap().set("OCProperties",dictionary! {}),
            "group"=>target.get_dictionary_mut(page).unwrap().set("Group",dictionary! {"S"=>"Transparency"}),
            "missing-dr"=>{let form=target.catalog().unwrap().get(b"AcroForm").unwrap().as_reference().unwrap();target.get_dictionary_mut(form).unwrap().remove(b"DR");},
            "missing-default-font"=>{let form=target.catalog().unwrap().get(b"AcroForm").unwrap().as_reference().unwrap();target.get_dictionary_mut(form).unwrap().set("DR",dictionary! {"Font"=>dictionary! {}});},_=>unreachable!(),
        }
        let target=open(&engine,&temp,&format!("{kind}.pdf"),&mut target);
        assert!(engine.replace_page_content(&target.id,0,&incoming.id,0).is_err(),"{kind}");engine.close_document(&target.id).unwrap();
    }
    engine.close_document(&incoming.id).unwrap();
}

#[test]
fn malformed_unbalanced_incoming_stream_is_rejected() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let target=open(&engine,&temp,"target.pdf",&mut fixture(false));
    for content in ["Q BT /F1 18 Tf 40 240 Td (Bad) Tj ET","q BT /F1 18 Tf 40 240 Td (Bad) Tj ET","BT /F1 18 Tf 40 240 Td (Bad) Tj"] {
        let mut pdf=fixture(true);let page=first(&pdf);let stream=pdf.add_object(Stream::new(dictionary! {},content.as_bytes().to_vec()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);
        let incoming=open(&engine,&temp,"bad.pdf",&mut pdf);
        assert!(engine.replace_page_content(&target.id,0,&incoming.id,0).is_err(),"{content}");engine.close_document(&incoming.id).unwrap();
    }
    engine.close_document(&target.id).unwrap();
}

#[test]
fn replacement_enforces_destination_rights_and_rejects_protected_incoming() {
    use lopdf::encryption::Permissions;
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let incoming=open(&engine,&temp,"incoming.pdf",&mut fixture(true));
    let path=temp.path().join("protected.pdf");let original=protected_fixture::encrypt(&bytes(&mut fixture(false)),Permissions::COPYABLE);fs::write(&path,&original).unwrap();
    let target=engine.open_document_with_password(&path,"reader").unwrap();
    assert!(matches!(engine.replace_page_content(&target.id,0,&incoming.id,0),Err(folio_engine::EngineError::PermissionDenied(_))));
    let editable=engine.open_document_with_password(&path,"owner").unwrap();
    assert!(engine.replace_page_content(&incoming.id,0,&editable.id,0).is_err());
    let result=engine.replace_page_content(&editable.id,0,&incoming.id,0).unwrap();
    assert_eq!(result.protection,editable.protection);assert_eq!(fs::read(&path).unwrap(),original);
    for info in [&result,&editable,&target,&incoming] {engine.close_document(&info.id).unwrap();}
}

#[test]
fn embedded_fonts_remain_identical_and_text_can_be_edited_after_replacement() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let target=open(&engine,&temp,"target.pdf",&mut fixture(false));
    let incoming_path=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/embedded-text/reportlab-subset.pdf");
    let incoming=engine.open_document(&incoming_path).unwrap();
    let result=engine.replace_page_content(&target.id,0,&incoming.id,0).unwrap();
    let font_programs=|bytes:&[u8]| {
        let doc=Document::load_mem(bytes).unwrap();let mut programs=Vec::new();
        for object in doc.objects.values() {if let Ok(dict)=object.as_dict() {for key in [b"FontFile".as_slice(),b"FontFile2",b"FontFile3"] {if let Ok(value)=dict.get(key) {let (_,value)=doc.dereference(value).unwrap();programs.push(value.as_stream().unwrap().content.clone());}}}}
        programs.sort();programs
    };
    let before=font_programs(&engine.source_bytes(&incoming.id).unwrap());assert!(!before.is_empty());
    assert_eq!(font_programs(&engine.source_bytes(&result.id).unwrap()),before);
    let run=engine.list_text_runs(&result.id,0).unwrap().runs.into_iter().find(|r|r.text=="Café old").unwrap();
    assert!(run.supported,"{:?}",run);
    let edited=engine.replace_text(&result.id,0,run.object_index,&run.text,"Café edit").unwrap();
    assert!(engine.extract_text(&edited.id,0).unwrap().contains("Café edit"));
    for info in [&edited,&result,&incoming,&target] {engine.close_document(&info.id).unwrap();}
}

#[test]
fn reader_authorized_replacement_keeps_encryption_through_save_and_reopen() {
    use folio_engine::{ExportRequest,PagePlan};use lopdf::encryption::Permissions;
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let incoming=open(&engine,&temp,"incoming.pdf",&mut fixture(true));
    let path=temp.path().join("editable-protected.pdf");fs::write(&path,protected_fixture::encrypt(&bytes(&mut fixture(false)),Permissions::MODIFIABLE)).unwrap();
    let target=engine.open_document_with_password(&path,"reader").unwrap();
    let result=engine.replace_page_content(&target.id,0,&incoming.id,0).unwrap();
    assert_eq!(result.protection,target.protection);
    let plans:Vec<PagePlan>=result.pages.iter().enumerate().map(|(i,p)|serde_json::from_value(serde_json::json!({"id":format!("p{i}"),"sourceId":result.id,"pageIndex":i,"width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})).unwrap()).collect();
    let path=temp.path().join("output.pdf");engine.export_pdf(ExportRequest {pages:plans,flatten:false},&path).unwrap();
    for password in ["reader","owner"] {
        let copy=engine.open_document_with_password(&path,password).unwrap();
        if password=="owner" {assert!(copy.protection.as_ref().unwrap().owner);assert!(engine.extract_text(&copy.id,0).unwrap().contains("New content"));}
        else {
            assert_eq!(copy.protection.as_ref().unwrap().permissions,target.protection.as_ref().unwrap().permissions);
            assert!(!copy.protection.as_ref().unwrap().owner);
            assert!(matches!(engine.extract_text(&copy.id,0),Err(folio_engine::EngineError::PermissionDenied(_))));
        }
        engine.close_document(&copy.id).unwrap();
    }
    for info in [&result,&target,&incoming] {engine.close_document(&info.id).unwrap();}
}

#[test]
fn shared_appearances_keep_aliases_within_page_and_isolate_other_pages() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let mut pdf=fixture(false);
    let page=first(&pdf);let second=*pdf.get_pages().get(&2).unwrap();
    let mut annotations=pdf.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap().clone();
    let widget=annotations[0].as_reference().unwrap();let original=pdf.get_dictionary(widget).unwrap().clone();
    let original_ap=original.get(b"AP").unwrap().as_dict().unwrap().get(b"N").unwrap().as_reference().unwrap();
    let form=pdf.catalog().unwrap().get(b"AcroForm").unwrap().as_reference().unwrap();
    let mut fields=pdf.get_dictionary(form).unwrap().get(b"Fields").unwrap().as_array().unwrap().clone();
    for (name,page_id,y) in [("Second field",page,120),("Other page",second,45)] {
        let mut clone=original.clone();clone.set("T",Object::string_literal(name));clone.set("P",page_id);clone.set("Rect",vec![30.into(),y.into(),210.into(),(y+26).into()]);
        let id=pdf.add_object(clone);fields.push(id.into());
        if page_id==page {annotations.push(id.into());}else {pdf.get_dictionary_mut(second).unwrap().set("Annots",vec![Object::Reference(id)]);}
    }
    pdf.get_dictionary_mut(page).unwrap().set("Annots",annotations.clone());pdf.get_dictionary_mut(form).unwrap().set("Fields",fields);
    let other_font=pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Times-Roman"});
    pdf.get_dictionary_mut(second).unwrap().set("Resources",dictionary! {"Font"=>dictionary! {"F1"=>other_font}});
    let target=open(&engine,&temp,"shared-ap.pdf",&mut pdf);let incoming=open(&engine,&temp,"incoming.pdf",&mut fixture(true));
    let original_second=engine.render_page(&target.id,1,600).unwrap();let original_field=field_pixels(&engine,&target);
    let result=engine.replace_page_content(&target.id,0,&incoming.id,0).unwrap();
    assert_eq!(engine.render_page(&result.id,1,600).unwrap(),original_second);assert_eq!(field_pixels(&engine,&result),original_field);
    let output=Document::load_mem(&engine.source_bytes(&result.id).unwrap()).unwrap();
    let appearance=|annotation:&Object|output.get_dictionary(annotation.as_reference().unwrap()).unwrap().get(b"AP").unwrap().clone();
    let appearance_id=|annotation:&Object| {let ap=appearance(annotation);let (_,ap)=output.dereference(&ap).unwrap();ap.as_dict().unwrap().get(b"N").unwrap().as_reference().unwrap()};
    assert_eq!(appearance_id(&annotations[0]),appearance_id(annotations.last().unwrap()));
    assert_ne!(appearance_id(&annotations[0]),original_ap);
    assert!(!output.get_object(original_ap).unwrap().as_stream().unwrap().dict.has(b"Resources"));
    for info in [&result,&incoming,&target] {engine.close_document(&info.id).unwrap();}
}

fn independent_widget_fonts(kind: &str) -> Document {
    let mut pdf=fixture(false);let page=first(&pdf);
    let widget=pdf.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap();
    let form=pdf.catalog().unwrap().get(b"AcroForm").unwrap().as_reference().unwrap();
    pdf.get_dictionary_mut(form).unwrap().remove(b"DR");
    let ap=pdf.get_dictionary(widget).unwrap().get(b"AP").unwrap().as_dict().unwrap().get(b"N").unwrap().as_reference().unwrap();
    if kind=="parent" {
        let resources=pdf.get_dictionary(page).unwrap().get(b"Resources").unwrap().clone();
        let mut parent=dictionary!{"Kids"=>vec![Object::Reference(widget)],"DR"=>resources};
        for key in [b"FT".as_slice(),b"T",b"V",b"DA"] {
            let value=pdf.get_dictionary_mut(widget).unwrap().remove(key).unwrap();parent.set(key.to_vec(),value);
        }
        // This alias is absent from the page/AP, so freezing the AP's page
        // resources cannot change the original interactive-fill fallback.
        let field_font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Times-Roman"});
        parent.set("DA",Object::string_literal("/FieldFont 12 Tf 0 g"));
        parent.set("DR",dictionary!{"Font"=>dictionary!{"FieldFont"=>field_font}});
        let parent=pdf.add_object(parent);pdf.get_dictionary_mut(widget).unwrap().set("Parent",parent);
        pdf.get_dictionary_mut(form).unwrap().set("Fields",vec![Object::Reference(parent)]);
    } else {
        // The local /F1 intentionally differs from both old and incoming page /F1.
        let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Times-Roman"});
        let fonts=dictionary!{"F1"=>font};
        let resources=if kind=="indirect" {
            let fonts=pdf.add_object(fonts);let resources=pdf.add_object(dictionary!{"Font"=>fonts});Object::Reference(resources)
        }else {Object::Dictionary(dictionary!{"Font"=>fonts})};
        pdf.get_object_mut(ap).unwrap().as_stream_mut().unwrap().dict.set("Resources",resources);
        if kind=="indirect" {
            let ap=pdf.add_object(dictionary!{"N"=>ap});pdf.get_dictionary_mut(widget).unwrap().set("AP",ap);
            let da=pdf.add_object(Object::string_literal("/F1 12 Tf 0 g"));pdf.get_dictionary_mut(widget).unwrap().set("DA",da);
        }
    }
    pdf
}

#[test]
fn independent_widget_fonts_preserve_replacement_fill_and_saved_reedit() {
    use folio_engine::{ExportRequest,PagePlan};
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for kind in ["local","indirect","parent"] {
        let target=open(&engine,&temp,&format!("{kind}.pdf"),&mut independent_widget_fonts(kind));
        let original=engine.source_bytes(&target.id).unwrap();let old_pixels=field_pixels(&engine,&target);
        let incoming=open(&engine,&temp,"incoming.pdf",&mut fixture(true));
        let result=engine.replace_page_content(&target.id,0,&incoming.id,0).unwrap();
        assert_eq!(field_pixels(&engine,&result),old_pixels,"{kind}");
        assert_eq!(engine.render_page(&target.id,1,600).unwrap(),engine.render_page(&result.id,1,600).unwrap());
        let plans=|source:&folio_engine::DocumentInfo|->Vec<PagePlan>{source.pages.iter().enumerate().map(|(i,p)|serde_json::from_value(serde_json::json!({"id":format!("p{i}"),"sourceId":source.id,"pageIndex":i,"width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})).unwrap()).collect()};
        let saved_path=temp.path().join(format!("{kind}-saved.pdf"));
        engine.export_pdf(ExportRequest{pages:plans(&result),flatten:false},&saved_path).unwrap();
        let saved=engine.open_document(&saved_path).unwrap();assert_eq!(field_pixels(&engine,&saved),old_pixels,"saved {kind}");
        let run=engine.list_occurrence_text_runs(&saved.id,0).unwrap().runs.remove(0);
        let edited=engine.replace_text_occurrence(&saved.id,&[0,1],0,run.object_index,&run.text,"Next content",None).unwrap();
        assert_eq!(field_pixels(&engine,&edited.document),old_pixels,"reedit {kind}");
        let fill=|source:&folio_engine::DocumentInfo|{
            let field=engine.list_form_fields(&source.id,0).unwrap().remove(0);
            engine.fill_form_field(&source.id,FillFormFieldRequest{page_index:0,annotation_index:field.annotation_index,expected_value:field.value,value:FormFieldValue::Text{text:"Filled later".into()}}).unwrap()
        };
        let baseline_fill=fill(&target);let immediate_fill=fill(&result);let filled=fill(&edited.document);
        assert_eq!(field_pixels(&engine,&baseline_fill),field_pixels(&engine,&immediate_fill),"immediate fill {kind}");
        assert_eq!(field_pixels(&engine,&baseline_fill),field_pixels(&engine,&filled),"filled {kind}");
        assert_eq!(engine.list_form_fields(&filled.id,0).unwrap()[0].value,FormFieldValue::Text{text:"Filled later".into()});
        assert_eq!(engine.source_bytes(&target.id).unwrap(),original);
        if let Some(root)=std::env::var_os("FOLIO_WIDGET_FONT_OUTPUT") {
            let root=PathBuf::from(root);fs::create_dir_all(&root).unwrap();
            for (suffix,source) in [("source",&target),("replaced",&result),("reedited",&edited.document),("baseline-filled",&baseline_fill),("filled",&filled)] {
                let path=root.join(format!("{kind}-{suffix}.pdf"));assert!(!path.exists());fs::write(path,engine.source_bytes(&source.id).unwrap()).unwrap();
            }
        }
        for info in [&filled,&immediate_fill,&baseline_fill,&edited.document,&saved,&result,&incoming,&target]{engine.close_document(&info.id).unwrap();}
    }
}

#[test]
fn button_widget_without_text_defaults_keeps_its_appearance() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let mut pdf=fixture(false);let page=first(&pdf);
    let widget=pdf.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap();
    let field=pdf.get_dictionary_mut(widget).unwrap();field.set("FT","Btn");field.set("Ff",65536);field.remove(b"DA");
    let form=pdf.catalog().unwrap().get(b"AcroForm").unwrap().as_reference().unwrap();pdf.get_dictionary_mut(form).unwrap().remove(b"DR");pdf.get_dictionary_mut(form).unwrap().remove(b"DA");
    let source=open(&engine,&temp,"button.pdf",&mut pdf);let incoming=open(&engine,&temp,"incoming.pdf",&mut fixture(true));
    let before=field_pixels(&engine,&source);let result=engine.replace_page_content(&source.id,0,&incoming.id,0).unwrap();assert_eq!(field_pixels(&engine,&result),before);
    for info in [&result,&incoming,&source]{engine.close_document(&info.id).unwrap();}
}

#[test]
fn widget_font_context_rejects_unproven_or_malformed_bindings() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let incoming=open(&engine,&temp,"incoming.pdf",&mut fixture(true));
    for kind in ["bad-font","missing-font","cycle","oversized-da","indirect-page-only"] {
        let mut pdf=independent_widget_fonts("local");let page=first(&pdf);
        let widget=pdf.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap();
        let ap=pdf.get_dictionary(widget).unwrap().get(b"AP").unwrap().as_dict().unwrap().get(b"N").unwrap().as_reference().unwrap();
        match kind {
            "bad-font"=>{pdf.get_object_mut(ap).unwrap().as_stream_mut().unwrap().dict.set("Resources",dictionary!{"Font"=>dictionary!{"F1"=>42}});},
            "missing-font"=>{pdf.get_object_mut(ap).unwrap().as_stream_mut().unwrap().dict.set("Resources",dictionary!{"Font"=>dictionary!{}});},
            "cycle"=>{let id=pdf.new_object_id();pdf.objects.insert(id,Object::Reference(id));pdf.get_object_mut(ap).unwrap().as_stream_mut().unwrap().dict.set("Resources",id);},
            "oversized-da"=>{pdf.get_dictionary_mut(widget).unwrap().set("DA",Object::string_literal(format!("{} /F1 12 Tf"," ".repeat(65536))));},
            "indirect-page-only"=>{let subtype=pdf.add_object(Object::Name(b"Widget".to_vec()));pdf.get_dictionary_mut(widget).unwrap().set("Subtype",subtype);pdf.get_object_mut(ap).unwrap().as_stream_mut().unwrap().dict.remove(b"Resources");},
            _=>unreachable!(),
        }
        let source=open(&engine,&temp,&format!("{kind}.pdf"),&mut pdf);let before=engine.source_bytes(&source.id).unwrap();
        assert!(engine.replace_page_content(&source.id,0,&incoming.id,0).is_err(),"{kind}");
        assert_eq!(engine.source_bytes(&source.id).unwrap(),before);engine.close_document(&source.id).unwrap();
    }
    engine.close_document(&incoming.id).unwrap();
}

#[test]
fn widget_parent_resources_cannot_change_the_later_fill_font() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let mut pdf=independent_widget_fonts("parent");
    let page=first(&pdf);let courier=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
    let widget=pdf.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap();
    let parent=pdf.get_dictionary(widget).unwrap().get(b"Parent").unwrap().as_reference().unwrap();
    let field_font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Times-Roman"});
    pdf.get_dictionary_mut(parent).unwrap().set("DA",Object::string_literal("/F1 12 Tf 0 g"));
    pdf.get_dictionary_mut(parent).unwrap().set("DR",dictionary!{"Font"=>dictionary!{"F1"=>field_font}});
    pdf.get_dictionary_mut(page).unwrap().set("Resources",dictionary!{"Font"=>dictionary!{"F1"=>courier}});
    let source=open(&engine,&temp,"parent-different-page-font.pdf",&mut pdf);
    let incoming=open(&engine,&temp,"incoming.pdf",&mut fixture(true));
    let fill=|source:&folio_engine::DocumentInfo|{
        let field=engine.list_form_fields(&source.id,0).unwrap().remove(0);
        engine.fill_form_field(&source.id,FillFormFieldRequest{page_index:0,annotation_index:field.annotation_index,expected_value:field.value,value:FormFieldValue::Text{text:"Wide WWW narrow iii".into()}}).unwrap()
    };
    let original_fill=fill(&source);
    match engine.replace_page_content(&source.id,0,&incoming.id,0) {
        Ok(changed)=>{let changed_fill=fill(&changed);assert_eq!(field_pixels(&engine,&original_fill),field_pixels(&engine,&changed_fill),"replacement changed the future fill font");engine.close_document(&changed_fill.id).unwrap();engine.close_document(&changed.id).unwrap();},
        Err(error)=>assert!(error.to_string().contains("fill font"),"{error}"),
    }
    for info in [&original_fill,&incoming,&source]{engine.close_document(&info.id).unwrap();}
}

#[test]
fn widget_dictionary_shaped_invalid_fonts_are_not_independent_bindings() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let incoming=open(&engine,&temp,"incoming.pdf",&mut fixture(true));
    for kind in ["missing-descendants","invalid-descendant","wrong-subtype"] {
        let mut pdf=independent_widget_fonts("local");let page=first(&pdf);
        let widget=pdf.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap();
        let ap=pdf.get_dictionary(widget).unwrap().get(b"AP").unwrap().as_dict().unwrap().get(b"N").unwrap().as_reference().unwrap();
        let mut font=dictionary!{"Type"=>"Font","Subtype"=>"Type0","BaseFont"=>"Broken","Encoding"=>"Identity-H"};
        if kind=="invalid-descendant" {font.set("DescendantFonts",vec![Object::Reference((987654,0))]);}
        if kind=="wrong-subtype" {font.set("Subtype","Image");}
        pdf.get_object_mut(ap).unwrap().as_stream_mut().unwrap().dict.set("Resources",dictionary!{"Font"=>dictionary!{"F1"=>font}});
        let source=open(&engine,&temp,&format!("{kind}.pdf"),&mut pdf);
        assert!(engine.replace_page_content(&source.id,0,&incoming.id,0).is_err(),"{kind}");engine.close_document(&source.id).unwrap();
    }
    engine.close_document(&incoming.id).unwrap();
}

