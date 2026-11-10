//! Same-font probe ownership and multiple imported highlighted styles.
use folio_engine::{ExportRequest,PagePlan,PdfEngine,StyledParagraphLayoutRequest,StyledParagraphSelection};
use lopdf::{dictionary,Document,Stream};
use serde_json::json;
use std::path::{Path,PathBuf};
static REGISTRY:std::sync::Mutex<()>=std::sync::Mutex::new(());
fn root()->PathBuf{PathBuf::from(env!("CARGO_MANIFEST_DIR"))}
fn engine()->PdfEngine{PdfEngine::start(root().join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path,rotation:i32,multi:bool){
 let mut p=Document::with_version("1.7");let pages=p.new_object_id();let f=p.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
 let (marks,text)=if multi{("q 1 1 0 rg 48 496 18 16 re f Q q 0 1 0 rg 70 496 18 16 re f Q","AA BB CC")}else{("q 1 1 0 rg 48 496 39 16 re f Q","AA BB")};
 let content=p.add_object(Stream::new(dictionary!{},format!("{marks} BT /F1 12 Tf 50 500 Td ({text}) Tj ET BT /F1 12 Tf 50 100 Td (Neighbor) Tj ET").into_bytes()));let page=p.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),500.into(),600.into()],"CropBox"=>vec![10.into(),20.into(),490.into(),580.into()],"Rotate"=>rotation,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>f}},"Contents"=>content});p.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());let catalog=p.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});p.trailer.set("Root",catalog);p.save(path).unwrap();
}
fn save(e:&PdfEngine,d:&folio_engine::DocumentInfo,path:&Path){e.export_pdf(ExportRequest{flatten:false,pages:vec![PagePlan{crop: None,id:"p".into(),source_id:d.id.clone(),page_index:0,width:d.pages[0].width,height:d.pages[0].height,rotation:0,overlays:vec![]}]},path).unwrap();}
fn capture(path:&Path){if let Some(dir)=std::env::var_os("FOLIO_MULTIFONT_ACCEPTANCE_DIR"){let dir=PathBuf::from(dir);std::fs::create_dir_all(&dir).unwrap();let target=dir.join(path.file_name().unwrap());assert!(!target.exists(),"Preserve prior outputs");std::fs::copy(path,target).unwrap();}}
fn replay(s:&StyledParagraphSelection)->StyledParagraphLayoutRequest{serde_json::from_value(json!({"objectIndices":s.object_indices,"expectedText":s.text,"runs":s.runs,"bounds":s.bounds,"alignment":s.alignment.unwrap(),"lineSpacing":s.line_spacing.unwrap()})).unwrap()}
fn selected(e:&PdfEngine,id:&str)->StyledParagraphSelection{let runs=e.list_text_runs(id,0).unwrap().runs;let anchor=runs.iter().find(|r|r.text!="Neighbor"&&!r.text.trim().is_empty()).unwrap();e.suggest_styled_paragraph(id,0,anchor.object_index,&anchor.text).unwrap()}

#[test]
fn identical_imported_probe_glyphs_in_distinct_styles_preserve_color_and_saved_identity(){
 let _guard=REGISTRY.lock().unwrap();
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for rotation in [0,90,180,270]{
  let font=e.fonts().register(std::fs::read(root().join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();let input=temp.path().join(format!("same-source-{rotation}.pdf"));fixture(&input,rotation,false);capture(&input);let original=std::fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();let s=e.inspect_styled_paragraph(&source.id,0,&[1]).unwrap();
  let mut r:StyledParagraphLayoutRequest=serde_json::from_value(json!({"objectIndices":s.object_indices,"expectedText":s.text,"runs":[{"text":"Café ","styleId":0},{"text":"Café","styleId":1}],"styleChanges":[{"styleId":0,"sourceStyleId":0,"fontId":font.id,"fontSize":18,"color":"#993366"},{"styleId":1,"sourceStyleId":0,"fontId":font.id,"fontSize":18,"color":"#336699"}],"bounds":s.bounds,"alignment":"left","lineSpacing":1.5})).unwrap();
  r.bounds=e.fit_styled_paragraph(&source.id,0,&r).unwrap();let edited=e.preview_styled_paragraph(&source.id,0,&r).unwrap();e.fonts().remove(&font.id).unwrap();e.close_document(&source.id).unwrap();let output=temp.path().join(format!("same-edited-{rotation}.pdf"));save(&e,&edited,&output);capture(&output);let opened=e.open_document(&output).unwrap();let s=selected(&e,&opened.id);assert_eq!(s.text,"Café Café");assert_eq!(s.line_spacing,Some(1.5));for color in ["#993366","#336699"]{assert!(s.styles.iter().any(|s|s.color==color&&s.font_size==18.&&s.background_color.as_deref()==Some("#FFFF00")));}
  let again=e.preview_styled_paragraph(&opened.id,0,&replay(&s)).unwrap();assert!(e.render_page(&again.id,0,900).unwrap()==e.render_page(&edited.id,0,900).unwrap(),"Same-owner saved re-edit changed pixels");let repeated=temp.path().join(format!("same-replayed-{rotation}.pdf"));save(&e,&again,&repeated);capture(&repeated);assert_eq!(std::fs::read(&input).unwrap(),original);for id in [&edited.id,&opened.id,&again.id]{e.close_document(id).unwrap();}
 }
}

#[test]
fn several_fonts_marks_and_wrapped_lines_replay_after_registry_and_source_release(){
 let _guard=REGISTRY.lock().unwrap();
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for (rotation,b) in [(0,[40.,60.,110.,200.]),(90,[300.,40.,200.,110.]),(180,[330.,300.,110.,200.]),(270,[60.,330.,200.,110.])]{
  let serif=e.fonts().register(std::fs::read(root().join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();let sans=e.fonts().register(std::fs::read(root().join("../tests/fixtures/shaped-text/DejaVuSans.ttf")).unwrap()).unwrap();let input=temp.path().join(format!("multi-source-{rotation}.pdf"));fixture(&input,rotation,true);capture(&input);let original=std::fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();let s=e.inspect_styled_paragraph(&source.id,0,&[2]).unwrap();
  let yellow=s.styles.iter().position(|s|s.background_color.as_deref()==Some("#FFFF00")).unwrap();let green=s.styles.iter().position(|s|s.background_color.as_deref()==Some("#00FF00")).unwrap();let plain=s.styles.iter().position(|s|s.background_color.is_none()).unwrap();let extra=s.styles.len();
  let r:StyledParagraphLayoutRequest=serde_json::from_value(json!({"objectIndices":s.object_indices,"expectedText":s.text,"runs":[{"text":"Café Café ","styleId":yellow},{"text":"words words ","styleId":green},{"text":"plain\n\n","styleId":plain},{"text":"Café ","styleId":extra},{"text":"words","styleId":green}],"styleChanges":[{"styleId":yellow,"sourceStyleId":yellow,"fontId":serif.id,"fontSize":18,"color":"#993366"},{"styleId":green,"sourceStyleId":green,"fontId":sans.id,"fontSize":14,"color":"#336699"},{"styleId":extra,"sourceStyleId":yellow,"fontId":serif.id,"fontSize":18,"color":"#663399"}],"bounds":{"x":b[0],"y":b[1],"width":b[2],"height":b[3]},"alignment":"left","lineSpacing":1.5})).unwrap();
  let layout=e.layout_styled_paragraph(&source.id,0,&r).unwrap();assert!(layout.lines.len()>=4);let edited=e.preview_styled_paragraph(&source.id,0,&r).unwrap();e.fonts().remove(&serif.id).unwrap();e.fonts().remove(&sans.id).unwrap();e.close_document(&source.id).unwrap();let output=temp.path().join(format!("multi-edited-{rotation}.pdf"));save(&e,&edited,&output);capture(&output);let opened=e.open_document(&output).unwrap();let s=selected(&e,&opened.id);assert_eq!(s.text,"Café Café words words plain\n\nCafé words");assert_eq!(s.line_spacing,Some(1.5));assert!(s.styles.iter().any(|s|s.font_size==14.&&s.background_color.as_deref()==Some("#00FF00")));let again=e.preview_styled_paragraph(&opened.id,0,&replay(&s)).unwrap();assert!(e.render_page(&again.id,0,900).unwrap()==e.render_page(&edited.id,0,900).unwrap(),"Multi-font saved re-edit changed pixels");let repeated=temp.path().join(format!("multi-replayed-{rotation}.pdf"));save(&e,&again,&repeated);capture(&repeated);assert_eq!(std::fs::read(&input).unwrap(),original);for id in [&edited.id,&opened.id,&again.id]{e.close_document(id).unwrap();}
 }
}

#[test]
fn maximum_palette_of_coincident_probe_styles_remains_reeditable(){
 let _guard=REGISTRY.lock().unwrap();let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("capacity-source.pdf");fixture(&path,0,false);capture(&path);let source=e.open_document(&path).unwrap();let s=e.inspect_styled_paragraph(&source.id,0,&[1]).unwrap();let font=e.fonts().register(std::fs::read(root().join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
 let changes=(0..64).map(|id|json!({"styleId":id,"sourceStyleId":0,"fontId":font.id,"fontSize":12,"color":format!("#{:02x}3366",id*3)})).collect::<Vec<_>>();let runs=(0..64).map(|id|json!({"styleId":id,"text":if id==63{"AA"}else{"AA "}})).collect::<Vec<_>>();
 let mut r:StyledParagraphLayoutRequest=serde_json::from_value(json!({"objectIndices":s.object_indices,"expectedText":s.text,"runs":runs,"styleChanges":changes,"bounds":s.bounds,"alignment":"left","lineSpacing":1.5})).unwrap();let start=std::time::Instant::now();r.bounds=e.fit_styled_paragraph(&source.id,0,&r).unwrap();let edited=e.preview_styled_paragraph(&source.id,0,&r).unwrap();e.fonts().remove(&font.id).unwrap();e.close_document(&source.id).unwrap();let output=temp.path().join("capacity-edited.pdf");save(&e,&edited,&output);capture(&output);let reopened=e.open_document(&output).unwrap();let s=selected(&e,&reopened.id);assert_eq!(s.styles.len(),64);assert_eq!(s.line_spacing,Some(1.5));let again=e.preview_styled_paragraph(&reopened.id,0,&replay(&s)).unwrap();assert!(e.render_page(&again.id,0,900).unwrap()==e.render_page(&edited.id,0,900).unwrap(),"Maximum palette replay changed pixels");let repeated=temp.path().join("capacity-replayed.pdf");save(&e,&again,&repeated);capture(&repeated);eprintln!("MULTIFONT_CAPACITY_MS {}",start.elapsed().as_millis());for id in [&edited.id,&reopened.id,&again.id]{e.close_document(id).unwrap();}
}

#[test]
fn existing_version_three_native_ink_keeps_exact_pixels(){
 let _guard=REGISTRY.lock().unwrap();let e=engine();let temp=tempfile::tempdir().unwrap();let path=root().join("../tests/fixtures/paragraphs/native-ink-v3.pdf");let original=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();let s=selected(&e,&source.id);assert_eq!(s.line_spacing,Some(1.5));assert_eq!(s.text,"AA Café marked words CC");let again=e.preview_styled_paragraph(&source.id,0,&replay(&s)).unwrap();assert!(e.render_page(&again.id,0,900).unwrap()==e.render_page(&source.id,0,900).unwrap(),"Existing v3 replay changed pixels");let output=temp.path().join("legacy-v3-replayed.pdf");save(&e,&again,&output);capture(&output);assert_eq!(std::fs::read(path).unwrap(),original);e.close_document(&source.id).unwrap();e.close_document(&again.id).unwrap();
}
