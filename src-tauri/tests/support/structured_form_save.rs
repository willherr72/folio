//! Desired public behavior, explicitly exercised while its native adapter grows.
use super::*;
#[path="structured_form_controls.rs"]
mod controls;

pub(super) fn tagged_form(path:&Path)->Vec<u8> {
    fixture(path,true,"Form source");let mut doc=Document::load(path).unwrap();let page=doc.get_pages()[&1];
    let resources=doc.dereference(doc.get_dictionary(page).unwrap().get(b"Resources").unwrap()).unwrap().1.as_dict().unwrap();
    let font=resources.get(b"Font").unwrap().as_dict().unwrap().get(b"F1").unwrap().clone();
    let appearance=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),140.into(),25.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font.clone()}}},b"0 0 140 25 re S BT /F1 10 Tf 5 8 Td (Original answer) Tj ET".to_vec()));
    let widget=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>Object::string_literal("answer"),"V"=>Object::string_literal("Original answer"),"DV"=>Object::string_literal("Default answer"),"P"=>page,"Rect"=>vec![40.into(),100.into(),180.into(),125.into()],"F"=>4,"AP"=>dictionary!{"N"=>appearance},"StructParent"=>3});
    doc.get_dictionary_mut(page).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().push(widget.into());
    let root=doc.catalog().unwrap().get(b"StructTreeRoot").unwrap().as_reference().unwrap();
    let element=doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"Form","P"=>root,"Pg"=>page,"K"=>dictionary!{"Type"=>"OBJR","Obj"=>widget}});
    let parents=doc.get_dictionary(root).unwrap().get(b"ParentTree").unwrap().as_reference().unwrap();
    doc.get_dictionary_mut(parents).unwrap().get_mut(b"Nums").unwrap().as_array_mut().unwrap().extend([3.into(),element.into()]);
    doc.get_dictionary_mut(root).unwrap().get_mut(b"K").unwrap().as_array_mut().unwrap().push(element.into());
    doc.catalog_mut().unwrap().set("AcroForm",dictionary!{"Fields"=>vec![Object::Reference(widget)],"NeedAppearances"=>false,"DA"=>Object::string_literal("/F1 10 Tf 0 g"),"DR"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}});
    doc.save(path).unwrap();fs::read(path).unwrap()
}

#[test]
fn structured_forms_combine_preserves_tagged_widget_answer_and_appearance(){
    let engine=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("form.pdf");let b=dir.path().join("other.pdf");
    let original=tagged_form(&a);let other_bytes=fixture(&b,false,"Other");
    let first=engine.open_document(&a).unwrap();let second=engine.open_document(&b).unwrap();
    let fields=engine.list_form_fields(&first.id,0).unwrap();assert_eq!(fields.len(),1);assert_eq!(fields[0].name,"answer");
    let original_value=fields[0].value.clone();let image=engine.render_page(&first.id,0,500).unwrap();
    let output=dir.path().join("combined.pdf");fs::write(&output,b"existing destination").unwrap();
    let saved=engine.export_pdf(request(&[&first,&second]),&output);
    assert_eq!(fs::read(&a).unwrap(),original);assert_eq!(fs::read(&b).unwrap(),other_bytes);
    if saved.is_err(){assert_eq!(fs::read(&output).unwrap(),b"existing destination");}
    saved.expect("tagged form combination must preserve forms and structure");
    let reopened=engine.open_document(&output).unwrap();let fields=engine.list_form_fields(&reopened.id,0).unwrap();
    assert_eq!(fields.len(),1);assert_eq!(fields[0].name,"answer");assert_eq!(fields[0].value,original_value);
    assert_eq!(engine.render_page(&reopened.id,0,500).unwrap(),image);
    let doc=Document::load(&output).unwrap();check_structure(&doc,4);
}
#[test]
fn structured_forms_duplicate_save_reopen_and_refill_keeps_shared_answer_and_tags(){
    use folio_engine::{FillFormFieldRequest,FormFieldValue};
    let engine=engine();let dir=tempfile::tempdir().unwrap();let source=dir.path().join("source.pdf");let original=tagged_form(&source);let opened=engine.open_document(&source).unwrap();let original_image=engine.render_page(&opened.id,0,500).unwrap();let mut plan=request(&[&opened]);let mut copy=plan.pages[0].clone();copy.id="copied-form-page".into();plan.pages.insert(1,copy);
    let bookmark=folio_engine::DocumentBookmark{id:"copy".into(),title:"Copied form".into(),level:0,destination:Some(folio_engine::PdfValue::Array(vec![folio_engine::PdfValue::Page(plan.pages[1].id.clone()),folio_engine::PdfValue::Name(b"Fit".to_vec())])),action:None,bold:false,italic:false,closed:false,color:None};
    let output=dir.path().join("duplicated.pdf");engine.export_pdf_with_bookmarks(plan,Some(vec![bookmark]),&output).unwrap();let saved=engine.open_document(&output).unwrap();assert_eq!(saved.bookmarks.len(),1);assert_eq!(saved.bookmarks[0].title,"Copied form");assert_eq!(saved.bookmarks[0].destination,Some(folio_engine::PdfValue::Array(vec![folio_engine::PdfValue::Page("1".into()),folio_engine::PdfValue::Name(b"Fit".to_vec())])));check_structure(&Document::load(&output).unwrap(),7);
    for page in 0..2{let fields=engine.list_form_fields(&saved.id,page).unwrap();assert_eq!(fields.len(),1);assert_eq!(fields[0].name,"answer");assert_eq!(engine.render_page(&saved.id,page,500).unwrap(),original_image);}
    let field=engine.list_form_fields(&saved.id,1).unwrap().remove(0);let value=FormFieldValue::Text{text:"Shared changed answer".into()};let filled=engine.fill_form_field(&saved.id,FillFormFieldRequest{page_index:1,annotation_index:field.annotation_index,expected_value:field.value,value:value.clone()}).unwrap();
    for page in 0..2{assert_eq!(engine.list_form_fields(&filled.id,page).unwrap()[0].value,value);}
    let changed_image=engine.render_page(&filled.id,0,500).unwrap();assert_ne!(changed_image,original_image);
    let baseline_field=engine.list_form_fields(&opened.id,0).unwrap().remove(0);let baseline=engine.fill_form_field(&opened.id,FillFormFieldRequest{page_index:0,annotation_index:baseline_field.annotation_index,expected_value:baseline_field.value,value:value.clone()}).unwrap();assert_eq!(changed_image,engine.render_page(&baseline.id,0,500).unwrap());assert_eq!(engine.render_page(&filled.id,1,500).unwrap(),changed_image);
    let second=dir.path().join("filled-saved.pdf");engine.export_pdf(request(&[&filled]),&second).unwrap();let reopened=engine.open_document(&second).unwrap();for page in 0..2{assert_eq!(engine.list_form_fields(&reopened.id,page).unwrap()[0].value,value);assert_eq!(engine.render_page(&reopened.id,page,500).unwrap(),changed_image);}check_structure(&Document::load(&second).unwrap(),7);assert_eq!(fs::read(&source).unwrap(),original);assert_eq!(engine.render_page(&opened.id,0,500).unwrap(),original_image);
}
#[test]
fn structured_forms_combined_same_names_keep_independent_answers_and_later_fonts(){
    use folio_engine::{FillFormFieldRequest,FormFieldValue};
    let engine=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a.pdf");let b=dir.path().join("b.pdf");let original_a=tagged_form(&a);tagged_form(&b);let mut doc=Document::load(&b).unwrap();
    let form=doc.catalog().unwrap().get(b"AcroForm").unwrap().as_dict().unwrap();let font=form.get(b"DR").unwrap().as_dict().unwrap().get(b"Font").unwrap().as_dict().unwrap().get(b"F1").unwrap().as_reference().unwrap();doc.get_dictionary_mut(font).unwrap().set("BaseFont","Courier");doc.save(&b).unwrap();let original_b=fs::read(&b).unwrap();
    let first=engine.open_document(&a).unwrap();let second=engine.open_document(&b).unwrap();let image=engine.render_page(&first.id,0,500).unwrap();let second_image=engine.render_page(&second.id,0,500).unwrap();let output=dir.path().join("combined.pdf");engine.export_pdf(request(&[&first,&second]),&output).unwrap();let saved=engine.open_document(&output).unwrap();assert_eq!(engine.render_page(&saved.id,0,500).unwrap(),image);assert_eq!(engine.render_page(&saved.id,2,500).unwrap(),second_image);
    let field=engine.list_form_fields(&saved.id,2).unwrap().remove(0);assert_eq!(field.name,"answer_2");let value=FormFieldValue::Text{text:"Only second answer".into()};let filled=engine.fill_form_field(&saved.id,FillFormFieldRequest{page_index:2,annotation_index:field.annotation_index,expected_value:field.value,value:value.clone()}).unwrap();assert_eq!(engine.list_form_fields(&filled.id,2).unwrap()[0].value,value);assert_eq!(engine.list_form_fields(&filled.id,0).unwrap()[0].value,FormFieldValue::Text{text:"Original answer".into()});assert_eq!(engine.render_page(&filled.id,0,500).unwrap(),image);
    let baseline_field=engine.list_form_fields(&second.id,0).unwrap().remove(0);let baseline=engine.fill_form_field(&second.id,FillFormFieldRequest{page_index:0,annotation_index:baseline_field.annotation_index,expected_value:baseline_field.value,value:value.clone()}).unwrap();assert_eq!(engine.render_page(&filled.id,2,500).unwrap(),engine.render_page(&baseline.id,0,500).unwrap());
    let final_path=dir.path().join("filled.pdf");engine.export_pdf(request(&[&filled]),&final_path).unwrap();let final_doc=engine.open_document(&final_path).unwrap();assert_eq!(engine.list_form_fields(&final_doc.id,2).unwrap()[0].value,value);assert_eq!(engine.render_page(&final_doc.id,2,500).unwrap(),engine.render_page(&filled.id,2,500).unwrap());check_structure(&Document::load(&final_path).unwrap(),8);assert_eq!(fs::read(&a).unwrap(),original_a);assert_eq!(fs::read(&b).unwrap(),original_b);
}

#[test]
fn structured_forms_empty_catalog_is_deliberately_supported(){
 let engine=engine();let dir=tempfile::tempdir().unwrap();let a=dir.path().join("tagged.pdf");let b=dir.path().join("empty-form.pdf");fixture(&a,true,"Tagged");fixture(&b,false,"Empty form");let mut doc=Document::load(&b).unwrap();doc.catalog_mut().unwrap().set("AcroForm",dictionary!{});doc.save(&b).unwrap();let first=engine.open_document(&a).unwrap();let second=engine.open_document(&b).unwrap();let image=engine.render_page(&second.id,0,500).unwrap();let out=dir.path().join("out.pdf");engine.export_pdf(request(&[&first,&second]),&out).unwrap();let saved=engine.open_document(&out).unwrap();assert!(engine.list_form_fields(&saved.id,2).unwrap().is_empty());assert_eq!(engine.render_page(&saved.id,2,500).unwrap(),image);assert!(Document::load(&out).unwrap().catalog().unwrap().has(b"AcroForm"));
}
