//! Inherited/indirect page settings and physical page scaling during editing.
use folio_engine::{ExportRequest,PagePlan,PdfEngine,StyledParagraphLayoutRequest};
use lopdf::{dictionary,Document,Object,Stream};
use serde_json::json;
use std::path::{Path,PathBuf};
static REGISTRY:std::sync::Mutex<()>=std::sync::Mutex::new(());
fn root()->PathBuf{PathBuf::from(env!("CARGO_MANIFEST_DIR"))}
fn fixture(path:&Path,rotation:i32,unit:f32,local:bool){
 let mut p=Document::with_version("1.7");let pages=p.new_object_id();let middle=p.new_object_id();let near=p.new_object_id();
 let courier=p.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});let wrong=p.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
 let fonts=p.add_object(dictionary!{"F1"=>courier});let resources=p.add_object(dictionary!{"Font"=>fonts});
 let media=p.add_object(Object::Array(vec![(-20).into(),(-10).into(),500.into(),600.into()]));let crop=p.add_object(Object::Array(vec![10.into(),20.into(),490.into(),580.into()]));let rotate=p.add_object(Object::Integer(rotation.into()));
 let contents=p.add_object(Stream::new(dictionary!{},b"q 1 1 0 rg 48 496 39 16 re f Q BT /F1 12 Tf 50 500 Td (AA BB) Tj ET BT /F1 12 Tf 50 100 Td (Neighbor) Tj ET".to_vec()));
 let mut leaf=dictionary!{"Type"=>"Page","Parent"=>near,"Contents"=>contents,"UserUnit"=>unit};
 if local{leaf.set("Resources",resources);leaf.set("Rotate",rotate);}
 let page=p.add_object(leaf);
 let mut near_dict=dictionary!{"Type"=>"Pages","Parent"=>middle,"Count"=>1,"Kids"=>vec![page.into()],"CropBox"=>crop};
 if !local{near_dict.set("Resources",resources);near_dict.set("Rotate",rotate);}
 p.objects.insert(near,near_dict.into());
 p.objects.insert(middle,dictionary!{"Type"=>"Pages","Parent"=>pages,"Count"=>1,"Kids"=>vec![near.into()],"MediaBox"=>media,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>wrong}},"Rotate"=>(rotation+90)%360}.into());
 p.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![middle.into()],"MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()],"CropBox"=>vec![0.into(),0.into(),100.into(),100.into()],"Rotate"=>0}.into());
 let catalog=p.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});p.trailer.set("Root",catalog);p.save(path).unwrap();
}
fn capture(path:&Path){if let Some(dir)=std::env::var_os("FOLIO_INHERITANCE_ACCEPTANCE_DIR"){let dir=PathBuf::from(dir);std::fs::create_dir_all(&dir).unwrap();let target=dir.join(path.file_name().unwrap());assert!(!target.exists(),"Preserve output");std::fs::copy(path,target).unwrap();}}
fn save(e:&PdfEngine,d:&folio_engine::DocumentInfo,path:&Path){e.export_pdf(ExportRequest{flatten:false,pages:vec![PagePlan{crop: None,id:"p".into(),source_id:d.id.clone(),page_index:0,width:d.pages[0].width,height:d.pages[0].height,rotation:0,overlays:vec![]}]},path).unwrap();capture(path);}
fn workflow(unit:f32,local:bool){
 let _guard=REGISTRY.lock().unwrap_or_else(|error|error.into_inner());let e=PdfEngine::start(root().join("resources/pdfium/pdfium.dll")).unwrap();let temp=tempfile::tempdir().unwrap();let mut failures=vec![];
 for rotation in [0,90,180,270]{
  let stem=format!("unit{unit}-local{local}-r{rotation}");let input=temp.path().join(format!("{stem}-source.pdf"));fixture(&input,rotation,unit,local);capture(&input);let original=std::fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();
  let expected=if rotation%180==0{(480.*unit,560.*unit)}else{(560.*unit,480.*unit)};
  let actual=(source.pages[0].width,source.pages[0].height);if actual!=expected{failures.push(format!("Open {stem}: expected{expected:?} actual{actual:?}"));}
  let s=e.suggest_styled_paragraph(&source.id,0,1,"AA BB").unwrap();assert_eq!(s.text,"AA BB");
  let font=e.fonts().register(std::fs::read(root().join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
  let mut r:StyledParagraphLayoutRequest=serde_json::from_value(json!({"objectIndices":s.object_indices,"expectedText":s.text,"runs":[{"text":"Café ","styleId":0},{"text":"Café","styleId":1}],"styleChanges":[{"styleId":0,"sourceStyleId":0,"fontId":font.id,"fontSize":18,"color":"#993366"},{"styleId":1,"sourceStyleId":0,"fontId":font.id,"fontSize":18,"color":"#336699"}],"bounds":s.bounds,"alignment":"left","lineSpacing":1.5})).unwrap();
  r.bounds=e.fit_styled_paragraph(&source.id,0,&r).unwrap();let edited=e.preview_styled_paragraph(&source.id,0,&r).unwrap();e.fonts().remove(&font.id).unwrap();e.close_document(&source.id).unwrap();let output=temp.path().join(format!("{stem}-edited.pdf"));save(&e,&edited,&output);let reopened=e.open_document(&output).unwrap();let actual=(reopened.pages[0].width,reopened.pages[0].height);if actual!=expected{failures.push(format!("Reopen {stem}: expected{expected:?} actual{actual:?}"));}
  let runs=e.list_text_runs(&reopened.id,0).unwrap().runs;let anchor=runs.iter().find(|r|r.text.starts_with("Café")).unwrap();let s=e.suggest_styled_paragraph(&reopened.id,0,anchor.object_index,&anchor.text).unwrap();assert_eq!(s.text,"Café Café");
  let replay=serde_json::from_value(json!({"objectIndices":s.object_indices,"expectedText":s.text,"runs":s.runs,"bounds":s.bounds,"alignment":s.alignment,"lineSpacing":s.line_spacing})).unwrap();let again=e.preview_styled_paragraph(&reopened.id,0,&replay).unwrap();assert_eq!(e.render_page(&edited.id,0,900).unwrap(),e.render_page(&again.id,0,900).unwrap());save(&e,&again,&temp.path().join(format!("{stem}-replayed.pdf")));assert_eq!(std::fs::read(&input).unwrap(),original);
  for id in [&edited.id,&reopened.id,&again.id]{e.close_document(id).unwrap();}
 }
 assert!(failures.is_empty(),"{}",failures.join("; "));
}
#[test]fn inherited_indirect_geometry_and_nearest_fonts(){workflow(1.,false)}
#[test]fn local_overrides_win_over_conflicting_ancestors(){workflow(1.,true)}
#[test]fn doubled_user_space_preserves_physical_geometry(){workflow(2.,false)}
#[test]fn fractional_user_space_preserves_physical_geometry(){workflow(0.5,true)}
#[test]fn decimal_user_space_preserves_physical_geometry(){workflow(1.3,false)}

#[test]fn scaled_plain_exports_report_physical_dimensions(){
 let _guard=REGISTRY.lock().unwrap_or_else(|error|error.into_inner());let e=PdfEngine::start(root().join("resources/pdfium/pdfium.dll")).unwrap();let temp=tempfile::tempdir().unwrap();let mut failures=vec![];
 for unit in [0.5,2.] {for rotation in [0,90,180,270]{
  let stem=format!("plain-unit{unit}-r{rotation}");let input=temp.path().join(format!("{stem}-source.pdf"));fixture(&input,rotation,unit,false);capture(&input);let source=e.open_document(&input).unwrap();
  let output=temp.path().join(format!("{stem}-exported.pdf"));save(&e,&source,&output);let reopened=e.open_document(&output).unwrap();let expected=if rotation%180==0{(480.*unit,560.*unit)}else{(560.*unit,480.*unit)};
  let actual=(reopened.pages[0].width,reopened.pages[0].height);if actual!=expected{failures.push(format!("{stem}: expected{expected:?} actual{actual:?}"));}
  assert_eq!(e.render_page(&source.id,0,900).unwrap(),e.render_page(&reopened.id,0,900).unwrap(),"Plain export pixel change {stem}");e.close_document(&source.id).unwrap();e.close_document(&reopened.id).unwrap();
 }}
 assert!(failures.is_empty(),"{}",failures.join("; "));
}
