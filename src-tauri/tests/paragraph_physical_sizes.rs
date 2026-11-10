use folio_engine::{PdfEngine,StyledParagraphLayoutRequest,AnnotationRect,ExportRequest,PagePlan};
use lopdf::{dictionary,Document,Stream};
use serde_json::json;
use std::path::{Path,PathBuf};
static REGISTRY:std::sync::Mutex<()>=std::sync::Mutex::new(());
fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path,unit:f32){
 fixture_with_highlight(path,unit,false);
}
fn fixture_with_highlight(path:&Path,unit:f32,highlight:bool){
 let mut pdf=Document::with_version("1.7");let tree=pdf.new_object_id();let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
 let mark=if highlight{format!("q 1 1 0 rg {} {} {} {} re f Q ",98./unit,1796./unit,12./unit,16./unit)}else{String::new()};
 let content=pdf.add_object(Stream::new(dictionary!{},format!("{mark}BT /F1 {} Tf {} {} Td (A) Tj ET",12./unit,100./unit,1800./unit).into_bytes()));
 let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),(2000./unit).into(),(2000./unit).into()],"UserUnit"=>unit,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
 pdf.objects.insert(tree,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![page.into()]}.into());let cat=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree});pdf.trailer.set("Root",cat);pdf.save(path).unwrap();
}
fn area()->AnnotationRect{AnnotationRect{x:100.,y:100.,width:1500.,height:1600.}}
fn save(e:&PdfEngine,d:&folio_engine::DocumentInfo,path:&Path){e.export_pdf(ExportRequest{flatten:false,pages:vec![PagePlan{crop: None,id:"p".into(),source_id:d.id.clone(),page_index:0,width:d.pages[0].width,height:d.pages[0].height,rotation:0,overlays:vec![]}]},path).unwrap();}
#[test]
fn styled_physical_font_endpoints_survive_save_suggestion_and_replay(){
 let _guard=REGISTRY.lock().unwrap_or_else(|e|e.into_inner());let e=engine();let dir=tempfile::tempdir().unwrap();
 for unit in [2.,0.5,1.,1.3]{for size in [1.,300.]{
  let input=dir.path().join(format!("styled-{unit}-{size}.pdf"));fixture(&input,unit);let original=std::fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();let s=e.suggest_styled_paragraph(&source.id,0,0,"A").unwrap();
  let r:StyledParagraphLayoutRequest=serde_json::from_value(json!({"objectIndices":s.object_indices,"expectedText":s.text,"runs":[{"styleId":0,"text":"B"}],"styleChanges":[{"styleId":0,"sourceStyleId":0,"fontSize":size}],"bounds":area(),"alignment":"left","lineSpacing":1.5})).unwrap();
  let edited=e.preview_styled_paragraph(&source.id,0,&r).unwrap_or_else(|error|panic!("unit={unit}, physical size={size}: {error}"));
  let output=dir.path().join(format!("styled-{unit}-{size}-edited.pdf"));save(&e,&edited,&output);let opened=e.open_document(&output).unwrap();let run=e.list_text_runs(&opened.id,0).unwrap().runs.remove(0);assert!((run.font_size-size).abs()<0.0001);
  let s=e.suggest_styled_paragraph(&opened.id,0,run.object_index,"B").unwrap();assert_eq!(s.line_spacing,Some(1.5));
  let r=serde_json::from_value(json!({"objectIndices":s.object_indices,"expectedText":s.text,"runs":s.runs,"bounds":s.bounds,"alignment":s.alignment,"lineSpacing":s.line_spacing})).unwrap();let again=e.preview_styled_paragraph(&opened.id,0,&r).unwrap();
  assert!(e.render_page(&again.id,0,900).unwrap()==e.render_page(&edited.id,0,900).unwrap(),"No-op pixel drift: unit={unit}, size={size}");assert_eq!(std::fs::read(&input).unwrap(),original);
  for id in [&source.id,&edited.id,&opened.id,&again.id]{e.close_document(id).unwrap();}
 }}
}
#[test]
fn uniform_physical_font_endpoints_are_accepted_and_outside_sizes_rejected(){
 let _guard=REGISTRY.lock().unwrap_or_else(|e|e.into_inner());let e=engine();let dir=tempfile::tempdir().unwrap();
 for unit in [2.,0.5,1.,1.3]{
  let input=dir.path().join(format!("uniform-{unit}.pdf"));fixture(&input,unit);let source=e.open_document(&input).unwrap();
  for size in [1.,300.]{let edited=e.preview_text_paragraph_with_font_size(&source.id,0,&[0],"A","B",&area(),Some(size)).unwrap_or_else(|error|panic!("unit={unit}, physical size={size}: {error}"));let r=e.list_text_runs(&edited.id,0).unwrap().runs.remove(0);assert!((r.font_size-size).abs()<0.0001);let output=dir.path().join(format!("uniform-{unit}-{size}-edited.pdf"));save(&e,&edited,&output);let opened=e.open_document(&output).unwrap();let s=e.suggest_text_paragraph(&opened.id,0,r.object_index,"B").unwrap();assert_eq!(s.text,"B");let again=e.preview_text_paragraph(&opened.id,0,&s.object_indices,&s.text,&s.text,&s.bounds).unwrap();assert!(e.render_page(&edited.id,0,900).unwrap()==e.render_page(&again.id,0,900).unwrap(),"Uniform saved replay drift at unit={unit}, size={size}");for id in [&edited.id,&opened.id,&again.id]{e.close_document(id).unwrap();}}
  for size in [0.99,300.01]{assert!(e.preview_text_paragraph_with_font_size(&source.id,0,&[0],"A","B",&area(),Some(size)).is_err(),"Invalid physical size accepted: {size} at unit{unit}");}
  e.close_document(&source.id).unwrap();
 }
}

#[test]
fn styled_physical_policy_rejects_outside_sizes_for_layout_fit_and_preview(){
 let _guard=REGISTRY.lock().unwrap_or_else(|e|e.into_inner());let e=engine();let dir=tempfile::tempdir().unwrap();
 for unit in [2.,0.5,1.,1.3]{
  let input=dir.path().join(format!("limits-{unit}.pdf"));fixture(&input,unit);let source=e.open_document(&input).unwrap();let s=e.inspect_styled_paragraph(&source.id,0,&[0]).unwrap();
  for size in [0.99,300.01]{let r=serde_json::from_value(json!({"objectIndices":s.object_indices,"expectedText":s.text,"runs":s.runs,"styleChanges":[{"styleId":0,"sourceStyleId":0,"fontSize":size}],"bounds":area(),"alignment":"left","lineSpacing":1.5})).unwrap();assert!(e.layout_styled_paragraph(&source.id,0,&r).is_err());assert!(e.fit_styled_paragraph(&source.id,0,&r).is_err());assert!(e.preview_styled_paragraph(&source.id,0,&r).is_err());}
  e.close_document(&source.id).unwrap();
 }
}
#[test]
fn highlighted_substituted_physical_endpoints_replay_after_font_release(){
 let _guard=REGISTRY.lock().unwrap_or_else(|e|e.into_inner());let e=engine();let dir=tempfile::tempdir().unwrap();
 for unit in [2.,0.5,1.3]{for size in [1.,300.]{
  let input=dir.path().join(format!("highlight-{unit}-{size}.pdf"));fixture_with_highlight(&input,unit,true);let source=e.open_document(&input).unwrap();let s=e.inspect_styled_paragraph(&source.id,0,&[1]).unwrap();assert_eq!(s.styles[0].background_color.as_deref(),Some("#FFFF00"));
  let font=e.fonts().register(std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
  let r=serde_json::from_value(json!({"objectIndices":s.object_indices,"expectedText":s.text,"runs":[{"styleId":0,"text":"B"}],"styleChanges":[{"styleId":0,"sourceStyleId":0,"fontId":font.id,"fontSize":size}],"bounds":area(),"alignment":"left","lineSpacing":1.5})).unwrap();
  let edited=e.preview_styled_paragraph(&source.id,0,&r).unwrap_or_else(|error|panic!("Highlighted physical size={size}, unit={unit}: {error}"));e.fonts().remove(&font.id).unwrap();e.close_document(&source.id).unwrap();let output=dir.path().join(format!("highlight-{unit}-{size}-edited.pdf"));save(&e,&edited,&output);let opened=e.open_document(&output).unwrap();let run=e.list_text_runs(&opened.id,0).unwrap().runs.remove(0);let s=e.suggest_styled_paragraph(&opened.id,0,run.object_index,"B").unwrap();assert_eq!(s.styles[0].background_color.as_deref(),Some("#FFFF00"));assert!((s.styles[0].font_size-size).abs()<0.0001);assert_eq!(s.line_spacing,Some(1.5));
  let r=serde_json::from_value(json!({"objectIndices":s.object_indices,"expectedText":s.text,"runs":s.runs,"bounds":s.bounds,"alignment":s.alignment,"lineSpacing":s.line_spacing})).unwrap();let again=e.preview_styled_paragraph(&opened.id,0,&r).unwrap();assert!(e.render_page(&again.id,0,900).unwrap()==e.render_page(&edited.id,0,900).unwrap(),"Highlighted saved replay drift unit={unit}, size={size}");for id in [&edited.id,&opened.id,&again.id]{e.close_document(id).unwrap();}
 }}
}

fn memory(e:&PdfEngine,id:&str,key:&[u8])->serde_json::Value{
 let pdf=Document::load_mem(&e.source_bytes(id).unwrap()).unwrap();let page=*pdf.get_pages().get(&1).unwrap();serde_json::from_slice(pdf.get_dictionary(page).unwrap().get(key).unwrap().as_str().unwrap()).unwrap()
}
#[test]
fn decimal_page_units_preserve_exact_saved_geometry_on_unchanged_reedit(){
 let _guard=REGISTRY.lock().unwrap_or_else(|e|e.into_inner());let e=engine();let dir=tempfile::tempdir().unwrap();
 for unit in [1.1,1.7]{for styled in [false,true]{for explicit in [false,true]{
  let input=dir.path().join(format!("decimal-{unit}-{styled}-{explicit}.pdf"));fixture(&input,unit);let source=e.open_document(&input).unwrap();let mut bounds=area();bounds.x=100.1;
  let edited=if styled{let r=serde_json::from_value(json!({"objectIndices":[0],"expectedText":"A","runs":[{"styleId":0,"text":"B"}],"styleChanges":if explicit{json!([{"styleId":0,"sourceStyleId":0,"fontSize":100.1}])}else{json!([])},"bounds":bounds,"alignment":"left","lineSpacing":1.5})).unwrap();e.preview_styled_paragraph(&source.id,0,&r).unwrap()}else{e.preview_text_paragraph_with_font_size(&source.id,0,&[0],"A","B",&bounds,explicit.then_some(100.1)).unwrap()};
  let output=dir.path().join(format!("decimal-{unit}-{styled}-{explicit}-edited.pdf"));save(&e,&edited,&output);let opened=e.open_document(&output).unwrap();let key=if styled{b"FolioStyledParagraphs".as_slice()}else{b"FolioParagraphs".as_slice()};let before=memory(&e,&opened.id,key);
  let again=if styled{let s=e.suggest_styled_paragraph(&opened.id,0,0,"B").unwrap();let r=serde_json::from_value(json!({"objectIndices":s.object_indices,"expectedText":s.text,"runs":s.runs,"styleChanges":if explicit{json!([{"styleId":0,"sourceStyleId":0,"fontSize":s.styles[0].font_size}])}else{json!([])},"bounds":s.bounds,"alignment":s.alignment,"lineSpacing":s.line_spacing})).unwrap();e.preview_styled_paragraph(&opened.id,0,&r).unwrap()}else{let s=e.suggest_text_paragraph(&opened.id,0,0,"B").unwrap();e.preview_text_paragraph_with_font_size(&opened.id,0,&s.object_indices,&s.text,&s.text,&s.bounds,explicit.then_some(s.font_size)).unwrap()};
  assert_eq!(memory(&e,&again.id,key),before,"Exact saved geometry changed: unit={unit}, styled={styled}, explicit={explicit}");for id in [&source.id,&edited.id,&opened.id,&again.id]{e.close_document(id).unwrap();}
 }}}
}

#[test]
fn decimal_minimum_size_remains_a_valid_public_font_value(){
 let _guard=REGISTRY.lock().unwrap_or_else(|e|e.into_inner());let e=engine();let dir=tempfile::tempdir().unwrap();
 for styled in [false,true]{
  let input=dir.path().join(format!("minimum-{styled}.pdf"));fixture(&input,1.7);let source=e.open_document(&input).unwrap();
  let edited=if styled{let r=serde_json::from_value(json!({"objectIndices":[0],"expectedText":"A","runs":[{"styleId":0,"text":"B"}],"styleChanges":[{"styleId":0,"sourceStyleId":0,"fontSize":1}],"bounds":area(),"alignment":"left","lineSpacing":1.5})).unwrap();e.preview_styled_paragraph(&source.id,0,&r).unwrap()}else{e.preview_text_paragraph_with_font_size(&source.id,0,&[0],"A","B",&area(),Some(1.)).unwrap()};
  let again=if styled{let s=e.inspect_styled_paragraph(&edited.id,0,&[0]).unwrap();assert_eq!(s.styles[0].font_size,1.,"The physical minimum must remain valid when explicitly resubmitted");let r=serde_json::from_value(json!({"objectIndices":s.object_indices,"expectedText":s.text,"runs":s.runs,"styleChanges":[{"styleId":0,"sourceStyleId":0,"fontSize":s.styles[0].font_size}],"bounds":s.bounds,"alignment":s.alignment,"lineSpacing":s.line_spacing})).unwrap();e.preview_styled_paragraph(&edited.id,0,&r).unwrap()}else{let s=e.inspect_text_paragraph(&edited.id,0,&[0]).unwrap();assert_eq!(s.font_size,1.,"The physical minimum must remain valid when explicitly resubmitted");e.preview_text_paragraph_with_font_size(&edited.id,0,&s.object_indices,&s.text,&s.text,&s.bounds,Some(s.font_size)).unwrap()};
  let key=if styled{b"FolioStyledParagraphs".as_slice()}else{b"FolioParagraphs".as_slice()};assert_eq!(memory(&e,&edited.id,key),memory(&e,&again.id,key));for id in [&source.id,&edited.id,&again.id]{e.close_document(id).unwrap();}
 }
}
