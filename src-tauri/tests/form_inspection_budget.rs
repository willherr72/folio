use folio_engine::{PdfEngine,FillFormFieldRequest,FormFieldValue};
use lopdf::{dictionary,Document,Object};
use std::{fs,path::PathBuf};

fn fixture(count:usize,options:usize,label_bytes:usize)->Vec<u8>{
    let mut d=Document::with_version("1.7");let pages=d.new_object_id();let page=d.new_object_id();let normal_page=d.new_object_id();
    let label=d.add_object(Object::string_literal("x".repeat(label_bytes)));
    let options=d.add_object(Object::Array(vec![Object::Reference(label);options]));let mut fields=vec![];let mut annots=vec![];
    for i in 0..count {
        let field=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Ch","T"=>Object::string_literal(format!("Choice{i}")),"P"=>page,"Rect"=>vec![20.into(),(20+i as i64*40).into(),280.into(),(50+i as i64*40).into()],"F"=>4,"Opt"=>options});
        fields.push(Object::Reference(field));annots.push(Object::Reference(field));
    }
    let font=d.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let normal=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>Object::string_literal("Normal field"),"P"=>normal_page,"Rect"=>vec![20.into(),300.into(),280.into(),340.into()],"F"=>4,"DA"=>Object::string_literal("/F1 12 Tf 0 g"),"V"=>Object::string_literal("Original")});
    fields.push(Object::Reference(normal));
    d.objects.insert(page,dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Annots"=>annots}.into());
    d.objects.insert(normal_page,dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Annots"=>vec![Object::Reference(normal)]}.into());
    d.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>2,"Kids"=>vec![Object::Reference(page),Object::Reference(normal_page)]}.into());
    let form=d.add_object(dictionary!{"Fields"=>fields,"DR"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"DA"=>Object::string_literal("/F1 12 Tf 0 g")});let root=d.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages,"AcroForm"=>form});d.trailer.set("Root",root);
    let mut bytes=vec![];d.save_to(&mut bytes).unwrap();bytes
}
fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}

#[test]
fn shared_options_are_bounded_and_another_page_remains_usable(){
    let e=engine();let temp=tempfile::tempdir().unwrap();let bytes=fixture(6,1000,2048);assert!(bytes.len()<100_000);
    let path=temp.path().join("shared-options.pdf");fs::write(&path,&bytes).unwrap();let source=e.open_document(&path).unwrap();
    match e.list_form_fields(&source.id,0) {Err(error)=>assert!(error.to_string().contains("memory budget"),"{error}"),Ok(_)=>panic!("a small shared input expanded past the form inspection memory budget")}
    let fields=e.list_form_fields(&source.id,1).unwrap();assert_eq!(fields.len(),1);assert_eq!(fields[0].name,"Normal field");
    let value=FormFieldValue::Text{text:"Still usable".into()};let filled=e.fill_form_field(&source.id,FillFormFieldRequest{page_index:1,annotation_index:fields[0].annotation_index,expected_value:fields[0].value.clone(),value:value.clone()}).unwrap();
    assert_eq!(e.list_form_fields(&filled.id,1).unwrap()[0].value,value);assert_eq!(fs::read(&path).unwrap(),bytes);
    if let Some(root)=std::env::var_os("FOLIO_FORM_BUDGET_OUTPUT"){let root=PathBuf::from(root);fs::create_dir_all(&root).unwrap();fs::write(root.join("Shared options.pdf"),&bytes).unwrap();}
    e.close_document(&filled.id).unwrap();e.close_document(&source.id).unwrap();
}

#[test]
fn ordinary_shared_options_keep_all_labels_and_values(){
    let e=engine();let temp=tempfile::tempdir().unwrap();let bytes=fixture(3,30,128);let path=temp.path().join("normal.pdf");fs::write(&path,&bytes).unwrap();let source=e.open_document(&path).unwrap();
    let fields=e.list_form_fields(&source.id,0).unwrap();assert_eq!(fields.len(),3);
    for field in fields {assert_eq!(field.options,vec!["x".repeat(128);30]);assert_eq!(field.value,FormFieldValue::Choice{indices:vec![]});}
    e.close_document(&source.id).unwrap();
}

#[test]
fn a_single_oversized_field_is_bounded_during_fill() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let bytes=fixture(1,1000,10000);
    let path=temp.path().join("single-oversized.pdf");fs::write(&path,&bytes).unwrap();let source=e.open_document(&path).unwrap();
    let result=e.fill_form_field(&source.id,FillFormFieldRequest{page_index:0,annotation_index:0,expected_value:FormFieldValue::Choice{indices:vec![]},value:FormFieldValue::Choice{indices:vec![0]}});
    match result { Err(error)=>assert!(error.to_string().contains("memory budget"),"{error}"),Ok(_)=>panic!("single-field fill inspection must be bounded") }
    assert_eq!(fs::read(&path).unwrap(),bytes);
    assert_eq!(e.list_form_fields(&source.id,1).unwrap()[0].value,FormFieldValue::Text{text:"Original".into()});
    e.close_document(&source.id).unwrap();
}
