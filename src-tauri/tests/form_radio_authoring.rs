use folio_engine::{CreateFormFieldRequest,FillFormFieldRequest,FormFieldValue,PdfEngine};
use std::path::PathBuf;
use lopdf::{dictionary,Document,Object,Stream};
#[path="support/protected_fixture.rs"] mod protected_fixture;
fn engine()->PdfEngine {PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn request()->CreateFormFieldRequest {
 serde_json::from_value(serde_json::json!({"bounds":{"x":40,"y":50,"width":180,"height":90},"rotation":0,"name":"answer","label":"Answer","required":true,"fontSize":12,"field":{"kind":"radio","options":["Yes","Café €","No"]}})).unwrap()
}
fn fixture(rotation:i32,unit:f32)->Vec<u8>{
 let mut d=Document::with_version("1.7");let pages=d.new_object_id();
 let font=d.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
 let content=d.add_object(Stream::new(dictionary!{},b"BT /F1 10 Tf 30 50 Td (Untouched original text) Tj ET".to_vec()));
 let page=d.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),320.into(),440.into()],"CropBox"=>vec![10.into(),20.into(),310.into(),420.into()],"Rotate"=>rotation,"UserUnit"=>unit,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
 d.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into());
 let root=d.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});d.trailer.set("Root",root);let mut bytes=vec![];d.save_to(&mut bytes).unwrap();bytes
}
fn view(r:&folio_engine::AnnotationRect,w:f32,h:f32,rotation:u16)->[f32;4]{
 match rotation {90=>[h-r.y-r.height,r.x,r.height,r.width],180=>[w-r.x-r.width,h-r.y-r.height,r.width,r.height],270=>[r.y,w-r.x-r.width,r.height,r.width],_=>[r.x,r.y,r.width,r.height]}
}
#[test]
fn radio_rows_follow_view_order_at_every_rotation_and_physical_scale(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for rotation in [0,90,180,270] {for unit in [1.,1.7,3.] {for workspace in [0,90,180,270] {
  let bytes=fixture(rotation,unit);let path=temp.path().join("source.pdf");std::fs::write(&path,&bytes).unwrap();let source=e.open_document(&path).unwrap();
  let mut r=request();r.rotation=workspace;if workspace==90||workspace==270 {r.bounds.width=90.;r.bounds.height=180.;}
  let frame=view(&r.bounds,source.pages[0].width,source.pages[0].height,workspace);
  let created=e.create_form_field_occurrence(&source.id,&[0],0,r).unwrap();let fields=e.list_form_fields(&created.document.id,0).unwrap();assert_eq!(fields.len(),3);
  for (i,f) in fields.iter().enumerate(){let b=view(f.bounds.as_ref().unwrap(),source.pages[0].width,source.pages[0].height,workspace);let expected=[frame[0],frame[1]+frame[3]*i as f32/3.,frame[2],frame[3]/3.];assert!(b.iter().zip(expected).all(|(a,b)|(*a-b).abs()<0.02),"r{rotation} u{unit} w{workspace} option{i}: {b:?} vs {expected:?}");}
  let filled=e.fill_form_field(&created.document.id,FillFormFieldRequest{page_index:0,annotation_index:fields[1].annotation_index,expected_value:fields[1].value.clone(),value:FormFieldValue::Toggle{checked:true}}).unwrap();
  let output=temp.path().join("saved.pdf");export(&e,&filled,workspace,&output);let saved=e.open_document(&output).unwrap();
  for (i,f) in e.list_form_fields(&saved.id,0).unwrap().iter().enumerate(){assert_eq!(f.value,FormFieldValue::Toggle{checked:i==1});assert_eq!(f.options,vec![["Yes","Café €","No"][i]]);}
  if let Some(root)=std::env::var_os("FOLIO_RADIO_OUTPUT"){let root=PathBuf::from(root);std::fs::create_dir_all(&root).unwrap();std::fs::copy(&output,root.join(format!("r{rotation}-u{unit}-w{workspace}.pdf"))).unwrap();}
  assert_eq!(std::fs::read(&path).unwrap(),bytes);for id in [&source.id,&created.document.id,&filled.id,&saved.id]{e.close_document(id).unwrap();}
 }}}
}
#[test]
fn radio_duplicate_isolation_and_existing_field_identity_survive(){
 let e=engine();let original=e.create_blank_document(300.,400.).unwrap();
 let mut old=request();old.name="original field".into();old.field=folio_engine::NewFormField::Text{multiline:false};
 let existing=e.create_form_field_occurrence(&original.id,&[0],0,old).unwrap();
 for selected in [0,1] {
  let created=e.create_form_field_occurrence(&existing.document.id,&[0,0],selected,request()).unwrap();assert_eq!(created.page_indices,vec![0,1]);
  assert_eq!(e.list_form_fields(&created.document.id,selected).unwrap().len(),4);let untouched=e.list_form_fields(&created.document.id,1-selected).unwrap();assert_eq!(untouched.len(),1);assert_eq!(untouched[0].name,"original field");
  assert!(e.create_form_field_occurrence(&created.document.id,&[0,1],selected,request()).is_err());
  e.close_document(&created.document.id).unwrap();
 }
 for id in [&original.id,&existing.document.id]{e.close_document(id).unwrap();}
}
#[test]
fn radio_invalid_choices_and_overflow_are_refused_without_partial_fields(){
 let e=engine();let source=e.create_blank_document(300.,400.).unwrap();
 let mut invalid=vec![];
 for options in [vec!["One".into()],vec!["One".into();2],vec!["Yes".into(),"漢字".into()],(0..33).map(|n|n.to_string()).collect(),vec!["W".repeat(100),"No".into()]] {let mut r=request();r.field=folio_engine::NewFormField::Radio{options};invalid.push(r);}
 let mut short=request();short.bounds.height=30.;invalid.push(short);
 let mut large=request();large.font_size=50.;invalid.push(large);
 for r in invalid {assert!(e.create_form_field_occurrence(&source.id,&[0],0,r).is_err());assert!(e.list_form_fields(&source.id,0).unwrap().is_empty());}
 e.close_document(&source.id).unwrap();
}
#[test]
fn radio_requires_creation_permissions_and_preserves_passwords(){
 use lopdf::encryption::Permissions;
 let e=engine();let temp=tempfile::tempdir().unwrap();let bytes=protected_fixture::encrypt(&fixture(0,1.),Permissions::FILLABLE);let path=temp.path().join("protected.pdf");std::fs::write(&path,&bytes).unwrap();
 let reader=e.open_document_with_password(&path,"reader").unwrap();assert!(e.create_form_field_occurrence(&reader.id,&[0],0,request()).is_err());
 let owner=e.open_document_with_password(&path,"owner").unwrap();let created=e.create_form_field_occurrence(&owner.id,&[0],0,request()).unwrap();let out=temp.path().join("saved.pdf");export(&e,&created.document,0,&out);
 assert!(e.open_document(&out).is_err());let saved=e.open_document_with_password(&out,"reader").unwrap();assert_eq!(e.list_form_fields(&saved.id,0).unwrap().len(),3);assert_eq!(std::fs::read(&path).unwrap(),bytes);
 for id in [&reader.id,&owner.id,&created.document.id,&saved.id]{e.close_document(id).unwrap();}
}
#[test]
fn radio_accepts_32_options_and_selects_the_last(){
 let e=engine();let source=e.create_blank_document(300.,1200.).unwrap();let mut r=request();r.bounds.height=960.;r.field=folio_engine::NewFormField::Radio{options:(0..32).map(|i|format!("Option {i}")).collect()};
 let created=e.create_form_field_occurrence(&source.id,&[0],0,r).unwrap();let fields=e.list_form_fields(&created.document.id,0).unwrap();assert_eq!(fields.len(),32);
 let filled=e.fill_form_field(&created.document.id,FillFormFieldRequest{page_index:0,annotation_index:fields[31].annotation_index,expected_value:fields[31].value.clone(),value:FormFieldValue::Toggle{checked:true}}).unwrap();
 for (i,f) in e.list_form_fields(&filled.id,0).unwrap().iter().enumerate(){assert_eq!(f.value,FormFieldValue::Toggle{checked:i==31});assert_eq!(f.options,vec![format!("Option {i}")]);}
 for id in [&source.id,&created.document.id,&filled.id]{e.close_document(id).unwrap();}
}
#[test]
fn radio_validation_does_not_read_unrelated_oversized_choice_labels(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let mut d=Document::load_mem(&fixture(0,1.)).unwrap();let page=*d.get_pages().get(&1).unwrap();
 let ap=d.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),40.into(),20.into()]},b"q Q".to_vec()));
 let old=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Ch","T"=>Object::string_literal("old"),"P"=>page,"F"=>4,"Rect"=>vec![20.into(),20.into(),60.into(),40.into()],"Opt"=>vec![Object::string_literal("x".repeat(40000))],"AP"=>dictionary!{"N"=>ap}});
 d.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(old)]);let form=d.add_object(dictionary!{"Fields"=>vec![Object::Reference(old)]});d.catalog_mut().unwrap().set("AcroForm",form);
 let mut bytes=vec![];d.save_to(&mut bytes).unwrap();let path=temp.path().join("oversized.pdf");std::fs::write(&path,&bytes).unwrap();let source=e.open_document(&path).unwrap();
 let created=e.create_form_field_occurrence(&source.id,&[0],0,request()).expect("validate only the three newly created widgets");
 let out=temp.path().join("saved.pdf");export(&e,&created.document,0,&out);let saved=Document::load(&out).unwrap();let root=saved.catalog().unwrap().get(b"AcroForm").unwrap();let form=saved.dereference(root).unwrap().1.as_dict().unwrap();assert_eq!(form.get(b"Fields").unwrap().as_array().unwrap().len(),2);
 assert_eq!(std::fs::read(&path).unwrap(),bytes);for id in [&source.id,&created.document.id]{e.close_document(id).unwrap();}
}
#[test]
fn review_large_accents_fit_inside_both_radio_appearance_states(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for (rotation,unit,workspace) in [(0,1.,0),(90,1.7,270),(180,3.,90),(270,1.7,180)] {
  let bytes=fixture(rotation,unit);let path=temp.path().join("source.pdf");std::fs::write(&path,&bytes).unwrap();let source=e.open_document(&path).unwrap();
  let mut r=request();r.name="accent-check".into();r.font_size=100.;r.bounds.height=248.;r.field=folio_engine::NewFormField::Radio{options:vec!["Å".into(),"Á".into()]};r.rotation=workspace;
  if workspace==90||workspace==270 {r.bounds.width=248.;r.bounds.height=180.;}
  let created=e.create_form_field_occurrence(&source.id,&[0],0,r).unwrap();let output=temp.path().join("large.pdf");export(&e,&created.document,workspace,&output);
  let d=Document::load(&output).unwrap();let page=d.get_dictionary(*d.get_pages().get(&1).unwrap()).unwrap();let annots=d.dereference(page.get(b"Annots").unwrap()).unwrap().1.as_array().unwrap();
  for (i,annot) in annots.iter().enumerate(){
   let widget=d.dereference(annot).unwrap().1.as_dict().unwrap();let ap=d.dereference(widget.get(b"AP").unwrap()).unwrap().1.as_dict().unwrap();let states=d.dereference(ap.get(b"N").unwrap()).unwrap().1.as_dict().unwrap();
   for (_,reference) in states.iter(){
    let stream=d.dereference(reference).unwrap().1.as_stream().unwrap();let bbox=stream.dict.get(b"BBox").unwrap().as_array().unwrap();let height=bbox[3].as_float().unwrap();
    let content=lopdf::content::Content::decode(&stream.get_plain_content().unwrap()).unwrap();
    let font_size=content.operations.iter().find(|op|op.operator=="Tf").unwrap().operands[1].as_float().unwrap();
    let baseline=content.operations.iter().find(|op|op.operator=="Tm").unwrap().operands[5].as_float().unwrap();
    // Adobe Helvetica AFM actual ink tops, not the requested line-height heuristic.
    let ascent=if i==0 {0.931}else{0.929};
    assert!(baseline+font_size*ascent<=height+0.001,"clipped accent: baseline {baseline}, glyph top {}, row {height}, r{rotation} u{unit} w{workspace}",baseline+font_size*ascent);
    assert!(baseline-font_size*0.225>=-0.001,"the complete Helvetica descender extent must also fit");
   }
  }
  if let Some(root)=std::env::var_os("FOLIO_RADIO_OUTPUT") {let root=PathBuf::from(root);std::fs::create_dir_all(&root).unwrap();std::fs::copy(&output,root.join(format!("accent-r{rotation}-u{unit}-w{workspace}.pdf"))).unwrap();}
  for id in [&source.id,&created.document.id]{e.close_document(id).unwrap();}
 }
}
fn export(e:&PdfEngine,info:&folio_engine::DocumentInfo,rotation:u16,path:&std::path::Path){
 let plans=info.pages.iter().enumerate().map(|(i,p)|serde_json::from_value(serde_json::json!({"id":format!("p{i}"),"sourceId":info.id,"pageIndex":i,"width":p.width,"height":p.height,"rotation":rotation,"overlays":p.overlays})).unwrap()).collect();
 e.export_pdf(folio_engine::ExportRequest{pages:plans,flatten:false},path).unwrap();
}
#[test]
fn radio_group_creates_labels_switches_exclusively_and_reopens(){
 let e=engine();let original=e.create_blank_document(300.,400.).unwrap();let created=e.create_form_field_occurrence(&original.id,&[0],0,request()).unwrap();
 assert!(e.list_form_fields(&original.id,0).unwrap().is_empty());
 let fields=e.list_form_fields(&created.document.id,0).unwrap();assert_eq!(fields.len(),3);
 for (i,f) in fields.iter().enumerate(){
  assert_eq!(f.kind,"radio");assert_eq!(f.name,"answer");assert_eq!(f.label,"Answer");assert!(f.required);
  assert_eq!(f.options,vec![["Yes","Café €","No"][i]]);assert_eq!(f.value,FormFieldValue::Toggle{checked:false});
  let b=f.bounds.as_ref().unwrap();assert!((b.x-40.).abs()<0.01&&(b.y-(50.+30.*i as f32)).abs()<0.01&&(b.width-180.).abs()<0.01&&(b.height-30.).abs()<0.01);
 }
 let mut source=created.document;
 for selected in [0,2,1] {
  let fields=e.list_form_fields(&source.id,0).unwrap();
  let next=e.fill_form_field(&source.id,FillFormFieldRequest{page_index:0,annotation_index:fields[selected].annotation_index,expected_value:fields[selected].value.clone(),value:FormFieldValue::Toggle{checked:true}}).unwrap();
  for (i,f) in e.list_form_fields(&next.id,0).unwrap().iter().enumerate(){assert_eq!(f.value,FormFieldValue::Toggle{checked:i==selected});}
  e.close_document(&source.id).unwrap();source=next;
 }
 let temp=tempfile::tempdir().unwrap();let path=temp.path().join("radio.pdf");export(&e,&source,0,&path);let saved=e.open_document(&path).unwrap();
 for (i,f) in e.list_form_fields(&saved.id,0).unwrap().iter().enumerate(){assert_eq!(f.value,FormFieldValue::Toggle{checked:i==1});assert_eq!(f.options,vec![["Yes","Café €","No"][i]]);}
 if let Some(root)=std::env::var_os("FOLIO_RADIO_OUTPUT"){let root=PathBuf::from(root);std::fs::create_dir_all(&root).unwrap();std::fs::copy(&path,root.join("Radio group.pdf")).unwrap();}
 for id in [&original.id,&source.id,&saved.id]{e.close_document(id).unwrap();}
}
