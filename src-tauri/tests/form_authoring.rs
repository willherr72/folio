use folio_engine::{AnnotationRect, CreateFormFieldRequest, FillFormFieldRequest, FormFieldValue, NewFormField, PdfEngine};
use std::path::PathBuf;
use lopdf::{dictionary,Document,Object,Stream};
#[path="support/protected_fixture.rs"] mod protected_fixture;

fn fixture(rotation:i32,unit:f32)->Vec<u8>{
 let mut d=Document::with_version("1.7");let pages=d.new_object_id();
 let font=d.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
 let content=d.add_object(Stream::new(dictionary!{},b"BT /F1 10 Tf 30 50 Td (Untouched original text) Tj ET".to_vec()));
 let page=d.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),320.into(),440.into()],"CropBox"=>vec![10.into(),20.into(),310.into(),420.into()],"Rotate"=>rotation,"UserUnit"=>unit,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
 d.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into());
 let root=d.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});d.trailer.set("Root",root);let mut bytes=vec![];d.save_to(&mut bytes).unwrap();bytes
}
fn export(e:&PdfEngine,info:&folio_engine::DocumentInfo,rotation:u16,path:&std::path::Path){
 let plans=info.pages.iter().enumerate().map(|(i,p)|serde_json::from_value(serde_json::json!({"id":format!("p{i}"),"sourceId":info.id,"pageIndex":i,"width":p.width,"height":p.height,"rotation":rotation,"overlays":p.overlays})).unwrap()).collect();
 e.export_pdf(folio_engine::ExportRequest{pages:plans,flatten:false},path).unwrap();
}

#[test]
fn rotated_scaled_fields_survive_filling_export_and_reopen(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for rotation in [0,90,180,270] {for unit in [1.,1.7,3.] {for workspace in [0,90,180,270] {
  let bytes=fixture(rotation,unit);let path=temp.path().join("source.pdf");std::fs::write(&path,&bytes).unwrap();let source=e.open_document(&path).unwrap();
  let mut r=request("answer",NewFormField::Text{multiline:false});r.rotation=workspace;
  if workspace==90||workspace==270 {r.bounds.width=40.;r.bounds.height=160.;}
  let created=e.create_form_field_occurrence(&source.id,&[0],0,r.clone()).unwrap();
  let field=e.list_form_fields(&created.document.id,0).unwrap().remove(0);
  let b=field.bounds.as_ref().unwrap();assert!((b.x-r.bounds.x).abs()<0.01&&(b.y-r.bounds.y).abs()<0.01&&(b.width-r.bounds.width).abs()<0.01&&(b.height-r.bounds.height).abs()<0.01);
  let filled=e.fill_form_field(&created.document.id,FillFormFieldRequest{page_index:0,annotation_index:field.annotation_index,expected_value:field.value,value:FormFieldValue::Text{text:"UPRIGHT text".into()}}).unwrap();
  let out=temp.path().join("saved.pdf");export(&e,&filled,workspace,&out);
  let saved=e.open_document(&out).unwrap();assert_eq!(e.list_form_fields(&saved.id,0).unwrap()[0].value,FormFieldValue::Text{text:"UPRIGHT text".into()});
  if let Some(root)=std::env::var_os("FOLIO_FORM_AUTHOR_OUTPUT") {let root=PathBuf::from(root);std::fs::create_dir_all(&root).unwrap();std::fs::copy(&out,root.join(format!("r{rotation}-u{unit}-w{workspace}.pdf"))).unwrap();}
  assert_eq!(std::fs::read(&path).unwrap(),bytes);
  for id in [&source.id,&created.document.id,&filled.id,&saved.id]{e.close_document(id).unwrap();}
 }}}
}

#[test]
fn creation_requires_edit_and_annotate_and_duplicate_assembly(){
 use lopdf::encryption::Permissions;
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for rights in [Permissions::FILLABLE,Permissions::MODIFIABLE,Permissions::ANNOTABLE,Permissions::MODIFIABLE|Permissions::ANNOTABLE] {
  let bytes=protected_fixture::encrypt(&fixture(0,1.),rights);let path=temp.path().join("protected.pdf");std::fs::write(&path,&bytes).unwrap();
  let source=e.open_document_with_password(&path,"reader").unwrap();
  let result=e.create_form_field_occurrence(&source.id,&[0],0,request("answer",NewFormField::Checkbox));
  if rights.contains(Permissions::MODIFIABLE|Permissions::ANNOTABLE) {let created=result.unwrap();assert_eq!(created.document.protection,source.protection);e.close_document(&created.document.id).unwrap();}else{assert!(result.is_err());}
  assert!(e.create_form_field_occurrence(&source.id,&[0,0],1,request("answer",NewFormField::Checkbox)).is_err());
  e.close_document(&source.id).unwrap();
  let owner=e.open_document_with_password(&path,"owner").unwrap();let created=e.create_form_field_occurrence(&owner.id,&[0,0],1,request("answer",NewFormField::Checkbox)).unwrap();
  assert_eq!(created.document.protection,owner.protection);e.close_document(&created.document.id).unwrap();e.close_document(&owner.id).unwrap();
  assert_eq!(std::fs::read(path).unwrap(),bytes);
 }
}

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()
}
fn request(name: &str, field: NewFormField) -> CreateFormFieldRequest {
    CreateFormFieldRequest {
        bounds: AnnotationRect { x: 40., y: 50., width: 160., height: 40. },
        rotation: 0, name: name.into(), label: "Accessible field label".into(),
        required: true, font_size: 12., field,
    }
}

#[test]
fn create_and_fill_each_common_field_without_changing_original() {
    let e = engine();
    for (field, kind, value) in [
        (NewFormField::Text { multiline: false }, "text", FormFieldValue::Text { text: "New text".into() }),
        (NewFormField::Text { multiline: true }, "text", FormFieldValue::Text { text: "Line one\nLine two".into() }),
        (NewFormField::Checkbox, "checkbox", FormFieldValue::Toggle { checked: true }),
        (NewFormField::Dropdown { options: vec!["One".into(), "Two".into()] }, "combo", FormFieldValue::Choice { indices: vec![1] }),
        (NewFormField::List { options: vec!["One".into(), "Two".into()], multi_select: true }, "list", FormFieldValue::Choice { indices: vec![0, 1] }),
    ] {
        let source = e.create_blank_document(300., 400.).unwrap();
        let added = e.create_form_field_occurrence(&source.id, &[0], 0, request("created", field)).unwrap();
        assert_eq!(added.page_indices, vec![0]);
        assert!(e.list_form_fields(&source.id, 0).unwrap().is_empty());
        let fields = e.list_form_fields(&added.document.id, 0).unwrap();
        assert_eq!(fields.len(), 1);
        let f = &fields[0];
        assert_eq!(f.name, "created"); assert_eq!(f.kind, kind); assert!(f.required);
        assert_eq!(f.label, "Accessible field label");
        let bounds = f.bounds.as_ref().unwrap();
        assert!((bounds.x - 40.).abs() < 0.01 && (bounds.y - 50.).abs() < 0.01);
        eprintln!("Filling newly authored {kind}");
        let filled = e.fill_form_field(&added.document.id, FillFormFieldRequest {
            page_index: 0, annotation_index: f.annotation_index, expected_value: f.value.clone(), value: value.clone(),
        }).unwrap();
        assert_eq!(e.list_form_fields(&filled.id, 0).unwrap()[0].value, value);
        for id in [&source.id, &added.document.id, &filled.id] { e.close_document(id).unwrap(); }
    }
}

#[test]
fn duplicate_occurrences_are_isolated_and_names_remain_unique() {
    let e = engine(); let source = e.create_blank_document(300.,400.).unwrap();
    for selected in 0..2 {
        let added = e.create_form_field_occurrence(&source.id, &[0,0], selected, request("answer", NewFormField::Checkbox)).unwrap();
        assert_eq!(added.page_indices, vec![0,1]);
        assert_eq!(e.list_form_fields(&added.document.id, selected).unwrap().len(), 1);
        assert!(e.list_form_fields(&added.document.id, 1-selected).unwrap().is_empty());
        assert!(e.create_form_field_occurrence(&added.document.id, &[0,1], 1-selected, request("answer", NewFormField::Checkbox)).is_err());
        e.close_document(&added.document.id).unwrap();
    }
    e.close_document(&source.id).unwrap();
}

#[test]
fn rejects_invalid_definitions_before_publication() {
    let e = engine(); let source = e.create_blank_document(300.,400.).unwrap();
    let valid = request("answer", NewFormField::Checkbox);
    let mut invalid = vec![];
    for name in ["", " ", "parent.child", "bad\nname"] { let mut r=valid.clone(); r.name=name.into(); invalid.push(r); }
    for size in [0., 301., f32::NAN] { let mut r=valid.clone(); r.font_size=size; invalid.push(r); }
    let mut r=valid.clone();r.bounds.x=290.;invalid.push(r);
    let mut r=valid.clone();r.rotation=45;invalid.push(r);
    invalid.push(request("choice",NewFormField::Dropdown{options:vec!["same".into(),"same".into()]}));
    invalid.push(request("choice",NewFormField::Dropdown{options:vec!["漢字".into()]}));
    for r in invalid { assert!(e.create_form_field_occurrence(&source.id,&[0],0,r).is_err()); }
    assert!(e.list_form_fields(&source.id,0).unwrap().is_empty());
    e.close_document(&source.id).unwrap();
}

#[test]
fn preserves_hierarchy_indirect_resources_and_existing_fields(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let mut pdf=Document::load_mem(&fixture(0,1.)).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
 let parent=pdf.new_object_id();
 let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
 let fonts=pdf.add_object(dictionary!{"FolioField0"=>font});let dr=pdf.add_object(dictionary!{"Font"=>fonts});
 let widget=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","Parent"=>parent,"T"=>Object::string_literal("name"),"P"=>page,"F"=>4,"Rect"=>vec![30.into(),180.into(),230.into(),215.into()],"DA"=>Object::string_literal("/FolioField0 14 Tf 0 g"),"V"=>Object::string_literal("")});
 pdf.objects.insert(parent,dictionary!{"T"=>Object::string_literal("person"),"Kids"=>vec![Object::Reference(widget)]}.into());
 let form=pdf.add_object(dictionary!{"Fields"=>vec![Object::Reference(parent)],"DR"=>dr});pdf.catalog_mut().unwrap().set("AcroForm",form);pdf.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(widget)]);
 let mut bytes=vec![];pdf.save_to(&mut bytes).unwrap();let path=temp.path().join("hierarchy.pdf");std::fs::write(&path,&bytes).unwrap();let source=e.open_document(&path).unwrap();
 assert!(e.create_form_field_occurrence(&source.id,&[0],0,request("person",NewFormField::Checkbox)).is_err());
 let created=e.create_form_field_occurrence(&source.id,&[0],0,request("新しい名前",NewFormField::Text{multiline:false})).unwrap();
 let fields=e.list_form_fields(&created.document.id,0).unwrap();assert_eq!(fields.len(),2);
 let old=fields.iter().find(|f|f.name=="person.name").unwrap();let filled=e.fill_form_field(&created.document.id,FillFormFieldRequest{page_index:0,annotation_index:old.annotation_index,expected_value:old.value.clone(),value:FormFieldValue::Text{text:"Courier preserved".into()}}).unwrap();
 let saved=temp.path().join("saved.pdf");export(&e,&filled,0,&saved);let output=Document::load(&saved).unwrap();
 let (_,form)=output.dereference(output.catalog().unwrap().get(b"AcroForm").unwrap()).unwrap();let (_,resources)=output.dereference(form.as_dict().unwrap().get(b"DR").unwrap()).unwrap();let (_,fontmap)=output.dereference(resources.as_dict().unwrap().get(b"Font").unwrap()).unwrap();
 assert!(fontmap.as_dict().unwrap().iter().any(|(_,v)|{let (_,font)=output.dereference(v).unwrap();font.as_dict().unwrap().get(b"BaseFont").unwrap().as_name().unwrap()==b"Courier"}));
 assert_eq!(std::fs::read(&path).unwrap(),bytes);
 for id in [&source.id,&created.document.id,&filled.id]{e.close_document(id).unwrap();}
}

#[test]
fn review_validates_only_the_new_widget_not_unrelated_choice_data(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let mut doc=Document::load_mem(&fixture(0,1.)).unwrap();let page=*doc.get_pages().get(&1).unwrap();
 let label=doc.add_object(Object::string_literal("shared option"));let options=doc.add_object(Object::Array(vec![Object::Reference(label);2001]));
 let old=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Ch","T"=>Object::string_literal("existing choice"),"Opt"=>options,"P"=>page,"Rect"=>vec![20.into(),20.into(),100.into(),100.into()],"F"=>4});
 let form=doc.add_object(dictionary!{"Fields"=>vec![Object::Reference(old)]});doc.catalog_mut().unwrap().set("AcroForm",form);doc.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(old)]);
 let path=temp.path().join("shared-options.pdf");doc.save(&path).unwrap();let source=e.open_document(&path).unwrap();
 let created=e.create_form_field_occurrence(&source.id,&[0],0,request("new checkbox",NewFormField::Checkbox)).expect("an unrelated old choice must not be deeply inspected");
 let bytes=e.source_bytes(&created.document.id).unwrap();let saved=Document::load_mem(&bytes).unwrap();
 let page=saved.get_dictionary(*saved.get_pages().get(&1).unwrap()).unwrap();assert_eq!(page.get(b"Annots").unwrap().as_array().unwrap().len(),2);
 e.close_document(&source.id).unwrap();e.close_document(&created.document.id).unwrap();
}

#[test]
fn review_unfilled_lists_keep_visible_rows_across_rotation_scale_and_save(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for rotation in [0,90,180,270] {for unit in [1.,1.7,3.] {for workspace in [0,90,180,270] {
  let path=temp.path().join("source.pdf");std::fs::write(&path,fixture(rotation,unit)).unwrap();let source=e.open_document(&path).unwrap();
  let mut r=request("unfilled list",NewFormField::List{options:vec!["Café €".into(),"Second row".into()],multi_select:true});r.rotation=workspace;r.bounds.height=70.;
  if workspace==90||workspace==270 {r.bounds.width=70.;r.bounds.height=160.;}
  let created=e.create_form_field_occurrence(&source.id,&[0],0,r).unwrap();
  let saved=temp.path().join("list.pdf");export(&e,&created.document,workspace,&saved);
  let reopened=e.open_document(&saved).unwrap();let fields=e.list_form_fields(&reopened.id,0).unwrap();
  assert_eq!(fields[0].options,vec!["Café €","Second row"]);assert_eq!(fields[0].value,FormFieldValue::Choice{indices:vec![]});
  if let Some(root)=std::env::var_os("FOLIO_FORM_AUTHOR_OUTPUT") {let root=PathBuf::from(root);std::fs::create_dir_all(&root).unwrap();std::fs::copy(&saved,root.join(format!("list-r{rotation}-u{unit}-w{workspace}.pdf"))).unwrap();}
  for id in [&source.id,&created.document.id,&reopened.id]{e.close_document(id).unwrap();}
 }}}
}
