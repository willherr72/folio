use folio_engine::{AnnotationRect,CreateFormFieldRequest,FillFormFieldRequest,FormFieldValue,PdfEngine,PositionFormFieldRequest};
use std::path::{Path,PathBuf};
#[path="support/protected_fixture.rs"] mod protected_fixture;
fn engine()->PdfEngine {PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn export(e:&PdfEngine,info:&folio_engine::DocumentInfo,path:&Path){
 let pages=info.pages.iter().enumerate().map(|(i,p)|serde_json::from_value(serde_json::json!({"id":format!("p{i}"),"sourceId":info.id,"pageIndex":i,"width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})).unwrap()).collect();
 e.export_pdf(folio_engine::ExportRequest{pages,flatten:false},path).unwrap();
}
fn text(e:&PdfEngine)->folio_engine::DocumentInfo {
 let blank=e.create_blank_document(300.,400.).unwrap();
 let request:CreateFormFieldRequest=serde_json::from_value(serde_json::json!({"bounds":{"x":20,"y":30,"width":100,"height":30},"rotation":0,"name":"name","label":"Name","required":true,"fontSize":12,"field":{"kind":"text","multiline":false}})).unwrap();
 let created=e.create_form_field_occurrence(&blank.id,&[0],0,request).unwrap().document;
 let field=e.list_form_fields(&created.id,0).unwrap().remove(0);
 let result=e.fill_form_field(&created.id,FillFormFieldRequest{page_index:0,annotation_index:field.annotation_index,expected_value:field.value,value:FormFieldValue::Text{text:"Keep this value".into()}}).unwrap();
 e.close_document(&blank.id).unwrap();e.close_document(&created.id).unwrap();result
}
#[test]
fn move_and_resize_keep_filled_value_and_saved_appearance(){
 let e=engine();let source=text(&e);let field=e.list_form_fields(&source.id,0).unwrap().remove(0);let temp=tempfile::tempdir().unwrap();
 let before=temp.path().join("before.pdf");export(&e,&source,&before);let bytes=std::fs::read(&before).unwrap();
 let bounds=AnnotationRect{x:130.,y:90.,width:150.,height:60.};
 let result=e.position_form_field_occurrence(&source.id,&[0],0,PositionFormFieldRequest{reflow_text:false,annotation_index:field.annotation_index,expected_bounds:field.bounds.clone().unwrap(),expected_value:field.value.clone(),bounds:bounds.clone()}).unwrap();
 let after=e.list_form_fields(&result.document.id,0).unwrap().remove(0);assert_eq!(after.value,field.value);assert_eq!(after.name,field.name);assert_eq!(after.required,field.required);
 assert!(same(after.bounds.as_ref().unwrap(),&bounds));assert!(same(e.list_form_fields(&source.id,0).unwrap()[0].bounds.as_ref().unwrap(),field.bounds.as_ref().unwrap()));
 let saved=temp.path().join("saved.pdf");export(&e,&result.document,&saved);let reopened=e.open_document(&saved).unwrap();let f=e.list_form_fields(&reopened.id,0).unwrap().remove(0);assert!(same(f.bounds.as_ref().unwrap(),&bounds));assert_eq!(f.value,field.value);
 assert_eq!(appearance(&before),appearance(&saved),"positioning must preserve the complete saved appearance graph");
 assert_eq!(std::fs::read(&before).unwrap(),bytes);
 let filled=e.fill_form_field(&reopened.id,FillFormFieldRequest{page_index:0,annotation_index:f.annotation_index,expected_value:f.value,value:FormFieldValue::Text{text:"Still editable".into()}}).unwrap();assert_eq!(e.list_form_fields(&filled.id,0).unwrap()[0].value,FormFieldValue::Text{text:"Still editable".into()});
 for id in [&source.id,&result.document.id,&reopened.id,&filled.id]{e.close_document(id).unwrap();}
}
fn same(a:&AnnotationRect,b:&AnnotationRect)->bool {[a.x-b.x,a.y-b.y,a.width-b.width,a.height-b.height].iter().all(|v|v.abs()<0.02)}

fn appearance(path:&Path)->String {
 fn resolved(d:&lopdf::Document,o:&lopdf::Object)->String {
  use lopdf::Object::*;let o=d.dereference(o).unwrap().1;
  match o {Dictionary(v)=>format!("{{{}}}",v.iter().map(|(k,v)|format!("{:?}:{}",k,resolved(d,v))).collect::<Vec<_>>().join(",")),Array(a)=>format!("[{}]",a.iter().map(|v|resolved(d,v)).collect::<Vec<_>>().join(",")),Stream(s)=>{let mut dict=s.dict.clone();dict.remove(b"Length");dict.remove(b"Filter");dict.remove(b"DecodeParms");format!("{}{:?}",resolved(d,&Dictionary(dict)),s.get_plain_content().unwrap())},_=>format!("{o:?}")}
 }
 let d=lopdf::Document::load(path).unwrap();let page=d.get_dictionary(*d.get_pages().get(&1).unwrap()).unwrap();let a=d.dereference(page.get(b"Annots").unwrap()).unwrap().1.as_array().unwrap();let widget=d.dereference(&a[0]).unwrap().1.as_dict().unwrap();resolved(&d,widget.get(b"AP").unwrap())
}

fn request(e:&PdfEngine,id:&str,page:usize,index:usize)->PositionFormFieldRequest {
 let f=e.list_form_fields(id,page).unwrap().into_iter().find(|f|f.annotation_index==index).unwrap();
 PositionFormFieldRequest{reflow_text:false,annotation_index:index,expected_bounds:f.bounds.clone().unwrap(),expected_value:f.value,bounds:AnnotationRect{x:130.,y:90.,width:150.,height:60.}}
}
#[test]
fn every_intrinsic_rotation_and_unit_uses_physical_display_points(){
 let e=engine();let source=text(&e);let temp=tempfile::tempdir().unwrap();let path=temp.path().join("base.pdf");export(&e,&source,&path);
 for rotation in [0,90,180,270] {for unit in [1.,1.7,3.] {
  let mut d=lopdf::Document::load(&path).unwrap();let page=*d.get_pages().get(&1).unwrap();let p=d.get_dictionary_mut(page).unwrap();
  p.set("Rotate",rotation);p.set("UserUnit",unit);p.set("MediaBox",vec![(-10).into(),(-20).into(),320.into(),440.into()]);p.set("CropBox",vec![10.into(),20.into(),300.into(),400.into()]);
  let input=temp.path().join("rotated.pdf");d.save(&input).unwrap();let opened=e.open_document(&input).unwrap();
  let r=request(&e,&opened.id,0,0);let expected=r.bounds.clone();let result=e.position_form_field_occurrence(&opened.id,&[0],0,r).unwrap();
  let f=e.list_form_fields(&result.document.id,0).unwrap().remove(0);assert!(same(f.bounds.as_ref().unwrap(),&expected),"r{rotation} u{unit}");
  let out=temp.path().join("saved.pdf");export(&e,&result.document,&out);let saved=e.open_document(&out).unwrap();assert!(same(e.list_form_fields(&saved.id,0).unwrap()[0].bounds.as_ref().unwrap(),&expected));
  if let Some(root)=std::env::var_os("FOLIO_FORM_POSITION_OUTPUT"){let root=PathBuf::from(root);std::fs::create_dir_all(&root).unwrap();std::fs::copy(&input,root.join(format!("before-r{rotation}-u{unit}.pdf"))).unwrap();std::fs::copy(&out,root.join(format!("after-r{rotation}-u{unit}.pdf"))).unwrap();}
  for id in [&opened.id,&result.document.id,&saved.id]{e.close_document(id).unwrap();}
 }}e.close_document(&source.id).unwrap();
}
#[test]
fn repeated_page_isolates_either_selected_occurrence(){
 let e=engine();let source=text(&e);let old=e.list_form_fields(&source.id,0).unwrap().remove(0);
 for selected in [0,1] {
  let r=request(&e,&source.id,0,0);let bounds=r.bounds.clone();let result=e.position_form_field_occurrence(&source.id,&[0,0],selected,r).unwrap();
  assert_eq!(result.page_indices,if selected==0{vec![0,1]}else{vec![0,1]});
  let changed=e.list_form_fields(&result.document.id,result.page_indices[selected]).unwrap().remove(0);
  let other=e.list_form_fields(&result.document.id,result.page_indices[1-selected]).unwrap().remove(0);
  assert!(same(changed.bounds.as_ref().unwrap(),&bounds));assert!(same(other.bounds.as_ref().unwrap(),old.bounds.as_ref().unwrap()));assert_eq!(other.value,old.value);
  e.close_document(&result.document.id).unwrap();
 }e.close_document(&source.id).unwrap();
}
#[test]
fn stale_invalid_locked_and_missing_appearance_requests_are_refused(){
 let e=engine();let source=text(&e);let temp=tempfile::tempdir().unwrap();let path=temp.path().join("base.pdf");export(&e,&source,&path);
 for case in 0..5 {
  let mut r=request(&e,&source.id,0,0);
  match case {0=>r.expected_bounds.x+=1.,1=>r.expected_value=FormFieldValue::Text{text:"stale".into()},2=>r.bounds.x=299.,3=>r.bounds.width=f32::NAN,_=>r.bounds.height=3.};
  assert!(e.position_form_field_occurrence(&source.id,&[0],0,r).is_err());
 }
 for flag in [8,16,128,512,0] {
  let mut d=lopdf::Document::load(&path).unwrap();let page=*d.get_pages().get(&1).unwrap();let annots=d.dereference(d.get_dictionary(page).unwrap().get(b"Annots").unwrap()).unwrap().1.as_array().unwrap();let widget=annots[0].as_reference().unwrap();
  if flag==0 {d.get_dictionary_mut(widget).unwrap().remove(b"AP");}else{d.get_dictionary_mut(widget).unwrap().set("F",4|flag);}
  let input=temp.path().join("refused.pdf");d.save(&input).unwrap();let opened=e.open_document(&input).unwrap();let r=request(&e,&opened.id,0,0);
  assert!(e.position_form_field_occurrence(&opened.id,&[0],0,r).is_err(),"flag {flag}");e.close_document(&opened.id).unwrap();
 }e.close_document(&source.id).unwrap();
}

#[test]
fn individual_radio_choice_keeps_siblings_and_group_selection(){
 let e=engine();let blank=e.create_blank_document(300.,400.).unwrap();let r:CreateFormFieldRequest=serde_json::from_value(serde_json::json!({"bounds":{"x":20,"y":30,"width":140,"height":90},"rotation":0,"name":"delivery","label":"Delivery","required":true,"fontSize":12,"field":{"kind":"radio","options":["Email","Paper","Café €"]}})).unwrap();
 let group=e.create_form_field_occurrence(&blank.id,&[0],0,r).unwrap().document;let before=e.list_form_fields(&group.id,0).unwrap();
 let result=e.position_form_field_occurrence(&group.id,&[0],0,request(&e,&group.id,0,1)).unwrap();let after=e.list_form_fields(&result.document.id,0).unwrap();
 for i in [0,2]{assert!(same(after[i].bounds.as_ref().unwrap(),before[i].bounds.as_ref().unwrap()));assert_eq!(after[i].options,before[i].options);}
 let filled=e.fill_form_field(&result.document.id,FillFormFieldRequest{page_index:0,annotation_index:1,expected_value:after[1].value.clone(),value:FormFieldValue::Toggle{checked:true}}).unwrap();
 for (i,f) in e.list_form_fields(&filled.id,0).unwrap().iter().enumerate(){assert_eq!(f.value,FormFieldValue::Toggle{checked:i==1});assert_eq!(f.name,"delivery");}
 for id in [&blank.id,&group.id,&result.document.id,&filled.id]{e.close_document(id).unwrap();}
}

#[test]
fn shared_text_widgets_keep_one_value_but_separate_geometry(){
 use lopdf::Object;
 let e=engine();let source=text(&e);let temp=tempfile::tempdir().unwrap();let path=temp.path().join("shared.pdf");export(&e,&source,&path);let mut d=lopdf::Document::load(&path).unwrap();
 let page=*d.get_pages().get(&1).unwrap();let a=d.dereference(d.get_dictionary(page).unwrap().get(b"Annots").unwrap()).unwrap().1.as_array().unwrap();let first=a[0].as_reference().unwrap();let mut node=d.get_dictionary(first).unwrap().clone();
 // Complete-source saving preserves an authored merged field/widget. Build the
 // shared terminal field explicitly instead of relying on export to split it.
 let parent=if let Ok(parent)=node.get(b"Parent") {parent.as_reference().unwrap()} else {
  assert_eq!(node.get(b"FT").unwrap().as_name().unwrap(),b"Tx");
  let mut definition=lopdf::Dictionary::new();
  for key in [b"FT".as_slice(),b"T",b"TU",b"TM",b"Ff",b"V",b"DV",b"DA",b"Q",b"MaxLen"] {
   if let Some(value)=node.remove(key){definition.set(key,value);}
  }
  d.add_object(definition)
 };
 let mut definition=d.get_dictionary(parent).unwrap().clone();assert_eq!(definition.get(b"FT").unwrap().as_name().unwrap(),b"Tx");
 node.set("Parent",parent);d.objects.insert(first,Object::Dictionary(node.clone()));node.set("Rect",vec![20.into(),280.into(),120.into(),310.into()]);let second=d.add_object(node);definition.set("Kids",vec![Object::Reference(first),Object::Reference(second)]);d.objects.insert(parent,Object::Dictionary(definition));
 let form=d.dereference(d.catalog().unwrap().get(b"AcroForm").unwrap()).unwrap().1.as_dict().unwrap().clone();let mut form=form;form.set("Fields",vec![Object::Reference(parent)]);d.catalog_mut().unwrap().set("AcroForm",form);d.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(first),Object::Reference(second)]);d.save(&path).unwrap();
 let opened=e.open_document(&path).unwrap();let old=e.list_form_fields(&opened.id,0).unwrap();assert_eq!(old.len(),2);assert!(old.iter().all(|f|f.kind=="text"&&f.name=="name"&&f.value==FormFieldValue::Text{text:"Keep this value".into()}));
 assert_eq!(e.inspect_form_properties(&opened.id,0,0).unwrap().linked_widgets,2);
 let moved=e.position_form_field_occurrence(&opened.id,&[0],0,request(&e,&opened.id,0,0)).unwrap();let fields=e.list_form_fields(&moved.document.id,0).unwrap();assert!(same(fields[1].bounds.as_ref().unwrap(),old[1].bounds.as_ref().unwrap()));
 let filled=e.fill_form_field(&moved.document.id,FillFormFieldRequest{page_index:0,annotation_index:0,expected_value:fields[0].value.clone(),value:FormFieldValue::Text{text:"Shared after moving".into()}}).unwrap();for f in e.list_form_fields(&filled.id,0).unwrap(){assert_eq!(f.value,FormFieldValue::Text{text:"Shared after moving".into()});}
 // A reused widget object is distinct from two ordinary sibling widgets.
 d.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(first),Object::Reference(first),Object::Reference(second)]);d.save(&path).unwrap();let alias=e.open_document(&path).unwrap();
 for (occurrences,selected) in [(vec![0],0),(vec![0,0],0),(vec![0,0],1)] {
  let result=e.position_form_field_occurrence(&alias.id,&occurrences,selected,request(&e,&alias.id,0,0));
  assert!(result.is_err(),"reused widget was published for occurrences {occurrences:?}, selected {selected}");
  assert!(result.unwrap_err().to_string().contains("reused in several locations"));
 }
 let parent_pages=d.get_dictionary(page).unwrap().get(b"Parent").unwrap().as_reference().unwrap();let mut other_page=d.get_dictionary(page).unwrap().clone();other_page.set("Annots",vec![Object::Reference(first)]);let other_page=d.add_object(other_page);
 d.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(first),Object::Reference(second)]);d.get_dictionary_mut(parent_pages).unwrap().set("Kids",vec![Object::Reference(page),Object::Reference(other_page)]);d.get_dictionary_mut(parent_pages).unwrap().set("Count",2);d.save(&path).unwrap();let cross=e.open_document(&path).unwrap();
 for (occurrences,selected) in [(vec![0,1],0),(vec![0,0,1],0),(vec![0,0,1],1)] {
  let result=e.position_form_field_occurrence(&cross.id,&occurrences,selected,request(&e,&cross.id,0,0));assert!(result.is_err());assert!(result.unwrap_err().to_string().contains("reused in several locations"));
 }
 for id in [&source.id,&opened.id,&moved.document.id,&filled.id,&alias.id,&cross.id]{e.close_document(id).unwrap();}
}
#[test]
fn protected_reader_is_refused_and_owner_output_keeps_passwords(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("protected.pdf");let original=text(&e);let plain=temp.path().join("plain.pdf");export(&e,&original,&plain);let bytes=protected_fixture::encrypt(&std::fs::read(&plain).unwrap(),lopdf::encryption::Permissions::FILLABLE);e.close_document(&original.id).unwrap();std::fs::write(&path,&bytes).unwrap();
 let reader=e.open_document_with_password(&path,"reader").unwrap();let r=request(&e,&reader.id,0,0);assert!(e.position_form_field_occurrence(&reader.id,&[0],0,r).is_err());
 let owner=e.open_document_with_password(&path,"owner").unwrap();let r=request(&e,&owner.id,0,0);let moved=e.position_form_field_occurrence(&owner.id,&[0],0,r).unwrap();let out=temp.path().join("saved.pdf");export(&e,&moved.document,&out);assert!(e.open_document(&out).is_err());let saved=e.open_document_with_password(&out,"reader").unwrap();assert_eq!(e.list_form_fields(&saved.id,0).unwrap().len(),1);assert_eq!(std::fs::read(&path).unwrap(),bytes);
 for id in [&reader.id,&owner.id,&moved.document.id,&saved.id]{e.close_document(id).unwrap();}
}

#[test]
fn review_malformed_normal_appearances_are_not_saved_artwork(){
 use lopdf::{dictionary,Object};
 let e=engine();let source=text(&e);let temp=tempfile::tempdir().unwrap();let path=temp.path().join("base.pdf");export(&e,&source,&path);
 for normal in [Object::Dictionary(dictionary!{"Off"=>Object::Null}),Object::Dictionary(dictionary!{"Off"=>123}),Object::Dictionary(dictionary!{})] {
  let mut d=lopdf::Document::load(&path).unwrap();let page=*d.get_pages().get(&1).unwrap();let a=d.dereference(d.get_dictionary(page).unwrap().get(b"Annots").unwrap()).unwrap().1.as_array().unwrap();let widget=a[0].as_reference().unwrap();d.get_dictionary_mut(widget).unwrap().set("AP",dictionary!{"N"=>normal});d.get_dictionary_mut(widget).unwrap().set("AS","Off");
  let bad=temp.path().join("bad.pdf");d.save(&bad).unwrap();let opened=e.open_document(&bad).unwrap();
  let result=e.position_form_field_occurrence(&opened.id,&[0],0,request(&e,&opened.id,0,0));assert!(result.is_err(),"a nonstream normal appearance was accepted");e.close_document(&opened.id).unwrap();
 }
 // A usable stream for a different state is insufficient for the current state.
 let mut d=lopdf::Document::load(&path).unwrap();let page=*d.get_pages().get(&1).unwrap();let a=d.dereference(d.get_dictionary(page).unwrap().get(b"Annots").unwrap()).unwrap().1.as_array().unwrap();let widget=a[0].as_reference().unwrap();let normal=d.dereference(d.get_dictionary(widget).unwrap().get(b"AP").unwrap()).unwrap().1.as_dict().unwrap().get(b"N").unwrap().clone();
 d.get_dictionary_mut(widget).unwrap().set("AP",dictionary!{"N"=>dictionary!{"Other"=>normal}});d.get_dictionary_mut(widget).unwrap().set("AS","Off");let bad=temp.path().join("missing.pdf");d.save(&bad).unwrap();let opened=e.open_document(&bad).unwrap();assert!(e.position_form_field_occurrence(&opened.id,&[0],0,request(&e,&opened.id,0,0)).is_err());
 for id in [&source.id,&opened.id]{e.close_document(id).unwrap();}
}
