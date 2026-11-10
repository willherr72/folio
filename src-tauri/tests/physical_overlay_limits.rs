#![cfg(feature = "shaped-text")]
use folio_engine::{PdfEngine,ExportRequest};
use lopdf::{dictionary,Document,Stream};
use serde_json::{json,Value};
use std::path::{Path,PathBuf};
static REGISTRY:std::sync::Mutex<()>=std::sync::Mutex::new(());
fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn capture(path:&Path){if let Some(dir)=std::env::var_os("FOLIO_OVERLAY_UNITS_CAPTURE_DIR"){let dir=PathBuf::from(dir);std::fs::create_dir_all(&dir).unwrap();let target=dir.join(path.file_name().unwrap());assert!(!target.exists(),"Preserve prior evidence");std::fs::copy(path,target).unwrap();}}
fn fixture(path:&Path,unit:f32){let mut p=Document::with_version("1.7");let pages=p.new_object_id();let contents=p.add_object(Stream::new(dictionary!{},b"q Q".to_vec()));let page=p.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),(1000./unit).into(),(1600./unit).into()],"UserUnit"=>unit,"Resources"=>dictionary!{},"Contents"=>contents});p.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());let cat=p.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});p.trailer.set("Root",cat);p.save(path).unwrap();}
fn overlay(kind:&str,font:&str,over:bool)->Value{
 if kind=="ink"{return json!({"type":"ink","id":"ink","paths":[[{"x":100,"y":100},{"x":500,"y":500}]],"strokeWidth":if over{100.01}else{100.},"color":"#123456"});}
 let mut v=json!({"type":"text","id":"text","x":40,"y":40,"text":"A","fontSize":if over{512.01}else{512.},"fontName":"Helvetica","color":"#123456","rotation":0});
 if kind!="standard"{v["fontId"]=json!(font);v["shaping"]=json!({"version":1,"direction":"ltr","ligatures":true});}
 if kind=="area"{v["fontSize"]=json!(12);v["area"]=json!({"version":1,"width":if over{14400.1}else{14400.},"height":1000,"inset":4,"lineSpacing":1.25,"alignment":"left"});}
 v
}
fn request(d:&folio_engine::DocumentInfo,overlay:Value,flatten:bool)->ExportRequest{serde_json::from_value(json!({"flatten":flatten,"pages":[{"id":"p","sourceId":d.id,"pageIndex":0,"width":d.pages[0].width,"height":d.pages[0].height,"rotation":0,"overlays":[overlay]}]})).unwrap()}
#[test]
fn physical_overlay_endpoints_export_and_reopen_on_scaled_pages(){
 let _guard=REGISTRY.lock().unwrap_or_else(|e|e.into_inner());let e=engine();let dir=tempfile::tempdir().unwrap();let font=e.fonts().register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec()).unwrap();let mut failures=vec![];
 for unit in [0.5,2.,1.7,1.9]{let input=dir.path().join(format!("input-{unit}.pdf"));fixture(&input,unit);let source=e.open_document(&input).unwrap();
  for kind in ["standard","shaped","area","ink"]{let overlay=overlay(kind,&font.id,false);let output=dir.path().join(format!("{unit}-{kind}.pdf"));match e.export_pdf(request(&source,overlay.clone(),false),&output){Err(error)=>failures.push(format!("unit{unit} {kind}: {error}")),Ok(_)=>{let reopened=e.open_document(&output).unwrap();if reopened.pages[0].overlays.len()!=1{failures.push(format!("unit{unit} {kind}: saved overlay was not restored"));}else{let actual=serde_json::to_value(&reopened.pages[0].overlays[0]).unwrap();for field in ["x","y","fontSize","strokeWidth"]{if let Some(expected)=overlay.get(field).and_then(Value::as_f64){assert!((actual[field].as_f64().unwrap()-expected).abs()<0.0002,"{unit} {kind} {field}");}}}let again=dir.path().join(format!("{unit}-{kind}-again.pdf"));if let Some(saved)=reopened.pages[0].overlays.first(){e.export_pdf(request(&reopened,serde_json::to_value(saved).unwrap(),false),&again).unwrap_or_else(|error|panic!("Re-export unit{unit} {kind}: {error}"));}let flat=dir.path().join(format!("{unit}-{kind}-flat.pdf"));e.export_pdf(request(&source,overlay.clone(),true),&flat).unwrap();for path in [&output,&again,&flat]{capture(path);}e.close_document(&reopened.id).unwrap();}}
  }e.close_document(&source.id).unwrap();
 }e.fonts().remove(&font.id).unwrap();assert!(failures.is_empty(),"{}",failures.join("\n"));
}
#[test]
fn physical_overlay_overlimits_are_rejected_before_export(){
 let _guard=REGISTRY.lock().unwrap_or_else(|e|e.into_inner());let e=engine();let dir=tempfile::tempdir().unwrap();let font=e.fonts().register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec()).unwrap();let mut accepted=vec![];
 for unit in [0.5,2.,1.7,1.9]{let input=dir.path().join(format!("input-{unit}.pdf"));fixture(&input,unit);let source=e.open_document(&input).unwrap();for kind in ["standard","shaped","area","ink"]{let output=dir.path().join(format!("{unit}-{kind}.pdf"));if e.export_pdf(request(&source,overlay(kind,&font.id,true),false),&output).is_ok(){accepted.push(format!("{unit} {kind}"));}}e.close_document(&source.id).unwrap();}e.fonts().remove(&font.id).unwrap();assert!(accepted.is_empty(),"Overlimits accepted: {}",accepted.join(", "));
}

#[test]
fn older_raw_shaped_appearances_remain_editable_after_page_scaling(){
 let _guard=REGISTRY.lock().unwrap_or_else(|e|e.into_inner());let e=engine();let dir=tempfile::tempdir().unwrap();let font=e.fonts().register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec()).unwrap();
 let input=dir.path().join("input.pdf");fixture(&input,1.);let source=e.open_document(&input).unwrap();
 for kind in ["shaped","area"]{let mut value=overlay(kind,&font.id,false);value["fontSize"]=json!(12);if kind=="area"{value["area"]["width"]=json!(300);value["area"]["height"]=json!(400);}
  let output=dir.path().join(format!("old-{kind}.pdf"));e.export_pdf(request(&source,value,false),&output).unwrap();let mut pdf=Document::load(&output).unwrap();let page=*pdf.get_pages().get(&1).unwrap();pdf.get_dictionary_mut(page).unwrap().set("UserUnit",2.);pdf.save(&output).unwrap();
  let reopened=e.open_document(&output).unwrap();assert_eq!(reopened.pages[0].overlays.len(),1,"Legacy raw {kind} appearance should remain editable");let actual=serde_json::to_value(&reopened.pages[0].overlays[0]).unwrap();assert_eq!(actual["fontSize"],json!(24.));e.close_document(&reopened.id).unwrap();
 }e.close_document(&source.id).unwrap();e.fonts().remove(&font.id).unwrap();
}

#[test]
fn cropped_rotated_scaled_overlay_appearances_export_and_reexport(){
 let _guard=REGISTRY.lock().unwrap_or_else(|e|e.into_inner());let e=engine();let dir=tempfile::tempdir().unwrap();let font=e.fonts().register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec()).unwrap();
 for unit in [0.5,1.7]{for rotation in [0,90,180,270]{
  let input=dir.path().join(format!("rotated-{unit}-{rotation}.pdf"));fixture(&input,unit);let mut pdf=Document::load(&input).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let page=pdf.get_dictionary_mut(page).unwrap();page.set("CropBox",vec![(10./unit).into(),(20./unit).into(),(990./unit).into(),(1580./unit).into()]);page.set("Rotate",rotation);pdf.save(&input).unwrap();let source=e.open_document(&input).unwrap();
  for kind in ["standard","shaped","area","ink"]{
   let mut value=overlay(kind,&font.id,false);if kind!="ink"{value["fontSize"]=json!(18);value["x"]=json!(12.25);value["y"]=json!(89.5);}if kind=="area"{value["area"]["width"]=json!(300);value["area"]["height"]=json!(400);}
   let stem=format!("rot{rotation}-unit{unit}-{kind}");let output=dir.path().join(format!("{stem}.pdf"));e.export_pdf(request(&source,value.clone(),false),&output).unwrap();let opened=e.open_document(&output).unwrap();assert_eq!(opened.pages[0].overlays.len(),1);let again=dir.path().join(format!("{stem}-again.pdf"));e.export_pdf(request(&opened,serde_json::to_value(&opened.pages[0].overlays[0]).unwrap(),false),&again).unwrap();let flat=dir.path().join(format!("{stem}-flat.pdf"));e.export_pdf(request(&source,value,true),&flat).unwrap();for path in [&output,&again,&flat]{capture(path);}e.close_document(&opened.id).unwrap();
  }e.close_document(&source.id).unwrap();
 }}e.fonts().remove(&font.id).unwrap();
}
