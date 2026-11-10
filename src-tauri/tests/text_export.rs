use folio_engine::{PdfEngine,PagePlan,TextExportRequest,TextExportControl};
use lopdf::{dictionary,Document,Stream};
use std::{fs,path::{Path,PathBuf},sync::Arc};
fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path){
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
 let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
 let mut kids=vec![];
 for text in ["First page","Second page"]{
  let stream=pdf.add_object(Stream::new(dictionary!{},format!("BT /F1 12 Tf 30 240 Td ({text}) Tj 0 -180 Td (Outside crop) Tj ET").into_bytes()));
  let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),300.into(),300.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>stream});kids.push(page.into());
 }
 pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>2,"Kids"=>kids}.into());let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);pdf.save(path).unwrap();
}
fn plans(e:&PdfEngine,path:&Path)->Vec<PagePlan>{let info=e.open_document(path).unwrap();info.pages.iter().enumerate().map(|(i,p)|serde_json::from_value(serde_json::json!({"id":format!("p{i}"),"sourceId":info.id,"pageIndex":i,"width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})).unwrap()).collect()}
fn request(pages:Vec<PagePlan>)->TextExportRequest{TextExportRequest{pages,include_form_values:true}}
#[test]
fn edited_ordered_cropped_pages_export_without_mutating_sources(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("source.pdf");fixture(&source);let original=fs::read(&source).unwrap();let mut pages=plans(&e,&source);
 pages[0].crop=Some(folio_engine::AnnotationRect{x:0.,y:0.,width:300.,height:140.});
 pages[0].overlays=serde_json::from_value(serde_json::json!([{"type":"text","id":"added","x":30,"y":90,"text":"Unsaved note","fontSize":12,"color":"#000000"}])).unwrap();
 pages[0].rotation=180;let ordered=vec![pages[1].clone(),pages[0].clone()];let path=temp.path().join("edited.txt");
 let result=e.export_text(request(ordered),&path,Arc::new(TextExportControl::new()),Arc::new(|_|{})).unwrap().unwrap();
 let text=fs::read_to_string(&path).unwrap();let parts:Vec<_>=text.trim_start_matches('\u{feff}').split('\u{c}').collect();assert_eq!(parts.len(),2);assert!(parts[0].contains("Second page"));assert!(parts[1].contains("First page"));assert!(parts[1].contains("Unsaved note"));assert!(!parts[1].contains("Outside crop"));assert_eq!(result.page_count,2);assert_eq!(fs::read(&source).unwrap(),original);
}
#[test]
fn cancel_preserves_existing_output_and_source_alias_is_rejected(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("source.pdf");fixture(&source);let pages=plans(&e,&source);let output=temp.path().join("existing.txt");fs::write(&output,b"Keep me").unwrap();
 let control=Arc::new(TextExportControl::new());let cancel=control.clone();
 assert!(e.export_text(request(pages.clone()),&output,control,Arc::new(move|p|{if p.completed==1{cancel.cancel();}})).unwrap().is_none());assert_eq!(fs::read(&output).unwrap(),b"Keep me");
 let alias=temp.path().join("source.txt");fs::hard_link(&source,&alias).unwrap();assert!(e.export_text(request(pages),&alias,Arc::new(TextExportControl::new()),Arc::new(|_|{})).is_err());assert!(fs::read(&source).unwrap().starts_with(b"%PDF"));
 assert!(!fs::read_dir(temp.path()).unwrap().any(|p|p.unwrap().file_name().to_string_lossy().starts_with(".folio-text-")));
}
#[path="support/protected_fixture.rs"]mod protected_fixture;
#[test]
fn copy_rights_are_native_and_independent_of_edit_or_assembly(){
 use lopdf::encryption::Permissions;
 let e=engine();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("source.pdf");fixture(&source);let bytes=fs::read(&source).unwrap();
 for (name,rights,allowed) in [("copy",Permissions::COPYABLE,true),("print",Permissions::PRINTABLE,false)]{
  let path=temp.path().join(format!("{name}.pdf"));fs::write(&path,protected_fixture::encrypt(&bytes,rights)).unwrap();let info=e.open_document_with_password(&path,"reader").unwrap();let p=&info.pages[0];
  let plan=serde_json::from_value(serde_json::json!({"id":name,"sourceId":info.id,"pageIndex":0,"width":p.width,"height":p.height,"rotation":90,"overlays":p.overlays,"protection":{"owner":true}})).unwrap();
  let output=temp.path().join(format!("{name}.txt"));let result=e.export_text(request(vec![plan]),&output,Arc::new(TextExportControl::new()),Arc::new(|_|{}));
  assert_eq!(result.is_ok(),allowed,"{name}: {result:?}");assert_eq!(output.exists(),allowed);
 }
}

#[test]
fn form_values_are_labeled_and_password_hidden_and_cropped_fields_are_omitted(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("forms.pdf");fixture(&source);let mut pdf=Document::load(&source).unwrap();let page=*pdf.get_pages().values().next().unwrap();let mut fields=vec![];
 for(name,value,flags,annotation_flags,y)in[("Name","Alice",0,4,180),("Secret","Never export this password",8192,4,150),("Hidden","Hidden value",0,2,120),("Below crop","Outside value",0,4,20)]{
  let field=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>lopdf::Object::string_literal(name),"V"=>lopdf::Object::string_literal(value),"Ff"=>flags,"F"=>annotation_flags,"Rect"=>vec![30.into(),y.into(),200.into(),(y+20).into()],"P"=>page});fields.push(field.into());
 }
 pdf.get_dictionary_mut(page).unwrap().set("Annots",fields.clone());let root=pdf.trailer.get(b"Root").unwrap().as_reference().unwrap();pdf.get_dictionary_mut(root).unwrap().set("AcroForm",dictionary!{"Fields"=>fields});pdf.save(&source).unwrap();
 let mut p=plans(&e,&source).remove(0);p.crop=Some(folio_engine::AnnotationRect{x:0.,y:0.,width:300.,height:200.});let path=temp.path().join("forms.txt");
 let result=e.export_text(request(vec![p.clone()]),&path,Arc::new(TextExportControl::new()),Arc::new(|_|{})).unwrap().unwrap();let text=fs::read_to_string(&path).unwrap();assert!(text.contains("Name: Alice"));for excluded in["Never export","Hidden value","Outside value"]{assert!(!text.contains(excluded),"{text}");}assert_eq!(result.skipped_fields,1);
 let no_fields=temp.path().join("no-fields.txt");e.export_text(TextExportRequest{pages:vec![p],include_form_values:false},&no_fields,Arc::new(TextExportControl::new()),Arc::new(|_|{})).unwrap();assert!(!fs::read_to_string(no_fields).unwrap().contains("Alice"));
}

#[cfg(feature="shaped-text")]
#[test]
fn unicode_owned_ocr_retains_logical_lines_at_all_workspace_rotations(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("ocr.pdf");fixture(&source);let pages=plans(&e,&source);let prep=e.prepare_ocr_page(&pages[0].source_id,0,0).unwrap();
 let ocr=serde_json::from_value(serde_json::json!({"width":prep.width,"height":prep.height,"blocks":[{"paragraphs":[{"lines":[
  {"words":[{"text":"café niño","confidence":95,"bounds":{"x":50,"y":300,"width":350,"height":35}}]},
  {"words":[{"text":"𐐀 final","confidence":95,"bounds":{"x":50,"y":380,"width":350,"height":35}}]}
 ]}]}]})).unwrap();
 let info=e.apply_ocr(&pages[0].source_id,vec![folio_engine::OcrUpdate{page_index:0,rotation:0,preserve_owned_orientation:false,page:ocr}]).unwrap();
 for rotation in [0,90,180,270]{
  let p=&info.pages[0];let plan=serde_json::from_value(serde_json::json!({"id":"ocr","sourceId":info.id,"pageIndex":0,"width":p.width,"height":p.height,"rotation":rotation,"overlays":p.overlays})).unwrap();let path=temp.path().join(format!("ocr-{rotation}.txt"));
  e.export_text(request(vec![plan]),&path,Arc::new(TextExportControl::new()),Arc::new(|_|{})).unwrap();let text=fs::read_to_string(path).unwrap();assert!(text.contains("café niño\n𐐀 final"),"{rotation}: {text}");assert!(text.contains("First page"));
 }
}

#[test]
fn empty_pages_and_physical_units_are_exported_without_false_text(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("units.pdf");fixture(&source);let mut pdf=Document::load(&source).unwrap();let ids:Vec<_>=pdf.get_pages().values().copied().collect();
 pdf.get_dictionary_mut(ids[0]).unwrap().set("UserUnit",1.7f32);pdf.get_dictionary_mut(ids[1]).unwrap().remove(b"Contents");pdf.save(&source).unwrap();
 let mut pages=plans(&e,&source);pages[0].crop=Some(folio_engine::AnnotationRect{x:0.,y:0.,width:510.,height:200.});let path=temp.path().join("units.txt");
 let result=e.export_text(request(pages),&path,Arc::new(TextExportControl::new()),Arc::new(|_|{})).unwrap().unwrap();let text=fs::read_to_string(path).unwrap();assert!(text.contains("First page"));assert!(!text.contains("Outside crop"));assert_eq!(result.empty_pages,1);assert_eq!(text.matches('\u{c}').count(),1);
}

#[test]
fn oversized_page_fails_without_replacing_destination(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("large.pdf");fixture(&source);let mut pdf=Document::load(&source).unwrap();let page=*pdf.get_pages().values().next().unwrap();
 let line="a".repeat(100);let mut content=String::from("BT /F1 0.1 Tf ");for i in 0..3000{content.push_str(&format!("1 0 0 1 10 {} Tm ({line}) Tj ",299.-i as f32*0.095));}content.push_str("ET");
 let stream=pdf.add_object(Stream::new(dictionary!{},content.into_bytes()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);pdf.save(&source).unwrap();
 let pages=plans(&e,&source);let path=temp.path().join("large.txt");fs::write(&path,b"Original destination").unwrap();let result=e.export_text(request(vec![pages[0].clone()]),&path,Arc::new(TextExportControl::new()),Arc::new(|_|{}));
 assert!(result.is_err());assert_eq!(fs::read(path).unwrap(),b"Original destination");
}

#[test]
fn excluded_form_values_cannot_exhaust_the_export_budget(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("excluded.pdf");fixture(&source);let mut pdf=Document::load(&source).unwrap();let page=*pdf.get_pages().values().next().unwrap();let mut fields=vec![];
 for i in 0..330{
  let(flags,f,y)=if i<110{(0,2,180)}else if i<220{(8192,4,180)}else{(0,4,10)};
  let field=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>lopdf::Object::string_literal(format!("excluded-{i}")),"V"=>lopdf::Object::string_literal("x".repeat(32768)),"Ff"=>flags,"F"=>f,"Rect"=>vec![30.into(),y.into(),200.into(),(y+20).into()],"P"=>page});fields.push(field.into());
 }
 // An oversized password fails the ordinary field reader's per-string bound.
 let long=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>lopdf::Object::string_literal("long-password"),"V"=>lopdf::Object::string_literal("private".repeat(6000)),"Ff"=>8192,"F"=>4,"Rect"=>vec![30.into(),150.into(),200.into(),170.into()],"P"=>page});fields.push(long.into());
 pdf.get_dictionary_mut(page).unwrap().set("Annots",fields.clone());let root=pdf.trailer.get(b"Root").unwrap().as_reference().unwrap();pdf.get_dictionary_mut(root).unwrap().set("AcroForm",dictionary!{"Fields"=>fields});pdf.save(&source).unwrap();
 let mut p=plans(&e,&source).remove(0);p.crop=Some(folio_engine::AnnotationRect{x:0.,y:0.,width:300.,height:200.});let path=temp.path().join("excluded.txt");
 let result=e.export_text(request(vec![p]),&path,Arc::new(TextExportControl::new()),Arc::new(|_|{})).unwrap().unwrap();assert_eq!(result.skipped_fields,111);assert_eq!(fs::read_to_string(path).unwrap(),"\u{feff}First page");
}

#[cfg(feature="shaped-text")]
#[test]
fn invalid_owned_ocr_proof_falls_back_to_native_text(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("ocr-invalid.pdf");fixture(&source);let plans=plans(&e,&source);let prep=e.prepare_ocr_page(&plans[0].source_id,0,0).unwrap();
 let page=serde_json::from_value(serde_json::json!({"width":prep.width,"height":prep.height,"blocks":[{"paragraphs":[{"lines":[{"words":[{"text":"Fallback OCR","confidence":95,"bounds":{"x":50,"y":300,"width":350,"height":35}}]}]}]}]})).unwrap();
 let info=e.apply_ocr(&plans[0].source_id,vec![folio_engine::OcrUpdate{page_index:0,rotation:0,preserve_owned_orientation:false,page}]).unwrap();
 let page=&info.pages[0];let p:PagePlan=serde_json::from_value(serde_json::json!({"id":"owned","sourceId":info.id,"pageIndex":0,"width":page.width,"height":page.height,"rotation":0,"overlays":page.overlays})).unwrap();let saved=temp.path().join("saved.pdf");e.export_pdf(folio_engine::ExportRequest{pages:vec![p],flatten:false},&saved).unwrap();
 let mut pdf=Document::load(&saved).unwrap();let page=*pdf.get_pages().values().next().unwrap();pdf.add_page_contents(page,b"q 1 0 0 1 0 0 cm /FolioOCR Do Q".to_vec()).unwrap();let modified=temp.path().join("modified.pdf");pdf.save(&modified).unwrap();
 let info=e.open_document(&modified).unwrap();let page=&info.pages[0];let p=serde_json::from_value(serde_json::json!({"id":"modified","sourceId":info.id,"pageIndex":0,"width":page.width,"height":page.height,"rotation":0,"overlays":page.overlays})).unwrap();let path=temp.path().join("fallback.txt");e.export_text(request(vec![p]),&path,Arc::new(TextExportControl::new()),Arc::new(|_|{})).unwrap();let text=fs::read_to_string(path).unwrap();assert!(text.contains("First page"));assert!(text.contains("Fallback OCR"));
}
