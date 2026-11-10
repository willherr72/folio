// Tests share the process engine and explicitly remove registered fonts.
static REGISTRY:std::sync::Mutex<()>=std::sync::Mutex::new(());
use folio_engine::{PdfEngine,StyledParagraphLayoutRequest,StyledParagraphRun,StyledParagraphStyleChange,ParagraphAlignment};
use lopdf::{dictionary,Document,Stream};
use std::path::{Path,PathBuf};
fn root()->PathBuf{PathBuf::from(env!("CARGO_MANIFEST_DIR"))}
fn engine()->PdfEngine{PdfEngine::start(root().join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path,rotation:i32,highlight:bool,blocked:bool){
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
 let mark=if highlight{"q 1 1 0 rg 48 696 18 16 re f Q "}else{""};let obstacles=if blocked{" q 0 .5 .5 rg 66 697 8 20 re f 49 660 100 36 re f Q"}else{""};
 let content=pdf.add_object(Stream::new(dictionary!{},format!("{mark}BT /F1 12 Tf 50 700 Td (AA) Tj ET BT /F1 12 Tf 50 100 Td (Neighbor) Tj ET{obstacles}").into_bytes()));let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),600.into(),800.into()],"CropBox"=>vec![10.into(),20.into(),590.into(),780.into()],"Rotate"=>rotation,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",catalog);pdf.save(path).unwrap();
}
fn request(e:&PdfEngine,id:&str,index:usize)->StyledParagraphLayoutRequest{let s=e.inspect_styled_paragraph(id,0,&[index]).unwrap();StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:s.object_indices,expected_text:s.text,runs:s.runs,style_changes:vec![],bounds:s.bounds,alignment:ParagraphAlignment::Left,line_spacing:1.2}}
#[test]
fn longer_drafts_fit_at_all_rotations_without_changing_the_text_anchor_or_source(){
 let _registry=REGISTRY.lock().unwrap_or_else(|error|error.into_inner());
 let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("fit.pdf");
 for rotation in [0,90,180,270]{fixture(&path,rotation,false,false);let source=e.open_document(&path).unwrap();let bytes=e.source_bytes(&source.id).unwrap();let mut draft=request(&e,&source.id,0);let before=e.layout_styled_paragraph(&source.id,0,&draft).unwrap();assert_eq!(e.fit_styled_paragraph(&source.id,0,&draft).unwrap(),draft.bounds);
  draft.runs=vec![StyledParagraphRun{text:"AA BB CC DD EE FF".into(),style_id:0}];assert!(e.layout_styled_paragraph(&source.id,0,&draft).is_err());let fitted=e.fit_styled_paragraph(&source.id,0,&draft).unwrap();assert_ne!(fitted,draft.bounds);draft.bounds=fitted;
  let layout=e.layout_styled_paragraph(&source.id,0,&draft).unwrap();assert_eq!(layout.pdf_bounds[0],before.pdf_bounds[0]);assert_eq!(layout.pdf_bounds[3],before.pdf_bounds[3]);assert_eq!(layout.lines.len(),1);assert_eq!(e.fit_styled_paragraph(&source.id,0,&draft).unwrap(),draft.bounds);let preview=e.preview_styled_paragraph(&source.id,0,&draft).unwrap();e.close_document(&preview.id).unwrap();assert_eq!(e.source_bytes(&source.id).unwrap(),bytes);assert_eq!(std::fs::read(&path).unwrap(),bytes.as_ref());e.close_document(&source.id).unwrap();
 }
}
#[test]
fn fitting_current_substitution_size_and_highlight_uses_the_current_draft(){
 let _registry=REGISTRY.lock().unwrap_or_else(|error|error.into_inner());
 let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("marked.pdf");for rotation in [0,90,180,270]{fixture(&path,rotation,true,false);let source=e.open_document(&path).unwrap();let bytes=e.source_bytes(&source.id).unwrap();let font=e.fonts().register(std::fs::read(root().join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();let mut draft=request(&e,&source.id,1);
 draft.runs=vec![StyledParagraphRun{text:"Café marked words".into(),style_id:0}];draft.style_changes=vec![StyledParagraphStyleChange{style_id:0,source_style_id:0,font_size:Some(18.),color:Some("#993366".into()),font_id:Some(font.id.clone())}];draft.line_spacing=1.5;
 assert!(e.layout_styled_paragraph(&source.id,0,&draft).is_err());draft.bounds=e.fit_styled_paragraph(&source.id,0,&draft).unwrap();let preview=e.preview_styled_paragraph(&source.id,0,&draft).unwrap();let runs=e.list_text_runs(&preview.id,0).unwrap().runs;let marked=runs.iter().find(|r|r.text.contains("Café")).unwrap();let s=e.suggest_styled_paragraph(&preview.id,0,marked.object_index,&marked.text).unwrap();assert_eq!(s.text,"Café marked words");assert_eq!(s.styles[0].font_size,18.);assert_eq!(s.styles[0].color,"#993366");assert_eq!(s.styles[0].background_color.as_deref(),Some("#FFFF00"));assert_eq!(s.line_spacing,Some(1.5));assert_eq!(e.source_bytes(&source.id).unwrap(),bytes);e.close_document(&preview.id).unwrap();e.close_document(&source.id).unwrap();e.fonts().remove(&font.id).unwrap();}
}
#[test]
fn blocked_and_invalid_requests_do_not_produce_a_fitted_area(){
 let _registry=REGISTRY.lock().unwrap_or_else(|error|error.into_inner());
 let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("blocked.pdf");fixture(&path,0,false,true);let source=e.open_document(&path).unwrap();let bytes=e.source_bytes(&source.id).unwrap();let mut draft=request(&e,&source.id,0);draft.runs=vec![StyledParagraphRun{text:"AA BB CC DD EE FF".into(),style_id:0}];assert!(e.fit_styled_paragraph(&source.id,0,&draft).is_err());
 for case in 0..4{let mut bad=draft.clone();match case{0=>bad.expected_text="stale".into(),1=>bad.bounds.width=f32::NAN,2=>bad.runs[0].style_id=63,_=>bad.runs[0].text="Ω".into()};assert!(e.fit_styled_paragraph(&source.id,0,&bad).is_err());}
 assert_eq!(e.source_bytes(&source.id).unwrap(),bytes);assert_eq!(std::fs::read(&path).unwrap(),bytes.as_ref());e.close_document(&source.id).unwrap();
}

#[test]
fn legacy_highlight_memory_keeps_its_measurement_policy_and_exact_pixels(){
 let _registry=REGISTRY.lock().unwrap_or_else(|error|error.into_inner());
 let e=engine();let path=root().join("../tests/fixtures/paragraphs/legacy-highlight-v2.pdf");let original=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();let pixels=e.render_page(&source.id,0,900).unwrap();let run=e.list_text_runs(&source.id,0).unwrap().runs.into_iter().find(|r|r.text.contains("Café")).unwrap();let s=e.suggest_styled_paragraph(&source.id,0,run.object_index,&run.text).unwrap();assert_eq!(s.line_spacing,Some(1.2));
 let draft=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:s.object_indices,expected_text:s.text,runs:s.runs,style_changes:vec![],bounds:s.bounds,alignment:s.alignment.unwrap(),line_spacing:s.line_spacing.unwrap()};let preview=e.preview_styled_paragraph(&source.id,0,&draft).unwrap();assert!(e.render_page(&preview.id,0,900).unwrap()==pixels,"Legacy no-op changed pixels");
 let pdf=Document::load_mem(&e.source_bytes(&preview.id).unwrap()).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let metadata:serde_json::Value=serde_json::from_slice(pdf.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();assert_eq!(metadata["version"],2);assert!(metadata["paragraphs"][0].get("native_ink").is_none());assert_eq!(std::fs::read(&path).unwrap(),original);e.close_document(&preview.id).unwrap();e.close_document(&source.id).unwrap();
}

#[test]
fn native_ink_memory_replays_after_font_release_and_rejects_changed_measurement_flags(){
 let _registry=REGISTRY.lock().unwrap_or_else(|error|error.into_inner());
 let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("native-ink.pdf");fixture(&path,0,true,false);let source=e.open_document(&path).unwrap();let font=e.fonts().register(std::fs::read(root().join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();let mut draft=request(&e,&source.id,1);draft.runs[0].text="Café marked words".into();draft.style_changes=vec![StyledParagraphStyleChange{style_id:0,source_style_id:0,font_size:Some(18.),color:None,font_id:Some(font.id.clone())}];draft.line_spacing=1.5;draft.bounds=e.fit_styled_paragraph(&source.id,0,&draft).unwrap();let preview=e.preview_styled_paragraph(&source.id,0,&draft).unwrap();let bytes=e.source_bytes(&preview.id).unwrap();let pixels=e.render_page(&preview.id,0,900).unwrap();e.fonts().remove(&font.id).unwrap();
 let saved=dir.path().join("saved.pdf");std::fs::write(&saved,&bytes).unwrap();let reopened=e.open_document(&saved).unwrap();let run=e.list_text_runs(&reopened.id,0).unwrap().runs.into_iter().find(|r|r.text.contains("Café")).unwrap();let s=e.suggest_styled_paragraph(&reopened.id,0,run.object_index,&run.text).unwrap();assert_eq!(s.line_spacing,Some(1.5));let replay=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:s.object_indices,expected_text:s.text,runs:s.runs,style_changes:vec![],bounds:s.bounds,alignment:s.alignment.unwrap(),line_spacing:s.line_spacing.unwrap()};let repeated=e.preview_styled_paragraph(&reopened.id,0,&replay).unwrap();assert!(e.render_page(&repeated.id,0,900).unwrap()==pixels,"Native-ink replay changed pixels");
 let pdf=Document::load_mem(&bytes).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let metadata:serde_json::Value=serde_json::from_slice(pdf.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();assert_eq!(metadata["version"],5);assert_eq!(metadata["paragraphs"][0]["native_ink"],true);assert_eq!(metadata["paragraphs"][0]["text_alignment"],true);
 for case in 0..2{let mut pdf=pdf.clone();let mut value=metadata.clone();if case==0{value["paragraphs"][0]["native_ink"]=serde_json::json!(false);}else{value["version"]=serde_json::json!(2);}pdf.get_dictionary_mut(page).unwrap().set("FolioStyledParagraphs",lopdf::Object::string_literal(serde_json::to_vec(&value).unwrap()));let path=dir.path().join(format!("forged-{case}.pdf"));pdf.save(&path).unwrap();let bad=e.open_document(&path).unwrap();let r=e.list_text_runs(&bad.id,0).unwrap().runs.into_iter().find(|r|r.text.contains("Café")).unwrap();if let Ok(s)=e.suggest_styled_paragraph(&bad.id,0,r.object_index,&r.text){assert_eq!(s.line_spacing,None);}e.close_document(&bad.id).unwrap();}
 for id in [&source.id,&preview.id,&reopened.id,&repeated.id]{e.close_document(id).unwrap();}
}

#[test]
#[ignore = "Opt-in integration uses installed Node/Vite to build the real editor request"]
fn editor_normalized_legacy_runs_and_unused_palette_keep_exact_legacy_replay(){
 let _registry=REGISTRY.lock().unwrap_or_else(|error|error.into_inner());
 use std::io::Write;
 use std::process::{Command,Stdio};
 let e=engine();let dir=tempfile::tempdir().unwrap();let golden=std::fs::read(root().join("../tests/fixtures/paragraphs/legacy-highlight-v2.pdf")).unwrap();
 for case in 0..2{
  let mut pdf=Document::load_mem(&golden).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let mut m:serde_json::Value=serde_json::from_slice(pdf.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();
  let p=&mut m["paragraphs"][0];
  if case==0{let runs=p["runs"].as_array_mut().unwrap();let i=runs.iter().position(|r|r["text"].as_str().unwrap().contains("Café")).unwrap();let mut second=runs[i].clone();let text=runs[i]["text"].as_str().unwrap().to_string();let split=text.find(' ').unwrap()+1;runs[i]["text"]=text[..split].into();second["text"]=text[split..].into();runs.insert(i+1,second);}
  else{let styles=p["styles"].as_array_mut().unwrap();styles.insert(0,styles[0].clone());for run in p["runs"].as_array_mut().unwrap(){run["styleId"]=(run["styleId"].as_u64().unwrap()+1).into();}}
  pdf.get_dictionary_mut(page).unwrap().set("FolioStyledParagraphs",lopdf::Object::string_literal(serde_json::to_vec(&m).unwrap()));let path=dir.path().join(format!("normalized-{case}.pdf"));pdf.save(&path).unwrap();let source=e.open_document(&path).unwrap();let pixels=e.render_page(&source.id,0,900).unwrap();let run=e.list_text_runs(&source.id,0).unwrap().runs.into_iter().find(|r|r.text.contains("Café")).unwrap();let s=e.suggest_styled_paragraph(&source.id,0,run.object_index,&run.text).unwrap();assert_eq!(s.line_spacing,Some(1.2),"Equivalent legacy metadata must replay");
  let mut child=Command::new("node").arg("scripts/styled-request-test-helper.mjs").current_dir(root().join("..")).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();child.stdin.take().unwrap().write_all(&serde_json::to_vec(&s).unwrap()).unwrap();let output=child.wait_with_output().unwrap();assert!(output.status.success(),"Editor bridge failed: {}",String::from_utf8_lossy(&output.stderr));let draft:StyledParagraphLayoutRequest=serde_json::from_slice(&output.stdout).unwrap();
  if case==0{assert_ne!(draft.runs,s.runs);}else{assert!(!draft.style_changes.is_empty());}
  assert_eq!(e.fit_styled_paragraph(&source.id,0,&draft).unwrap(),draft.bounds);let preview=e.preview_styled_paragraph(&source.id,0,&draft).unwrap();let result=Document::load_mem(&e.source_bytes(&preview.id).unwrap()).unwrap();let page=*result.get_pages().get(&1).unwrap();let metadata:serde_json::Value=serde_json::from_slice(result.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();assert_eq!(metadata["version"],2,"Editor normalization must retain legacy measurements");assert!(e.render_page(&preview.id,0,900).unwrap()==pixels,"Normalized legacy no-op changed pixels");e.close_document(&preview.id).unwrap();e.close_document(&source.id).unwrap();
 }
}



#[test]
fn unmarked_full_font_ink_replays_after_registry_release_with_blank_lines(){
 let _registry=REGISTRY.lock().unwrap_or_else(|error|error.into_inner());
 let e=engine();let dir=tempfile::tempdir().unwrap();
 for rotation in [0,90,180,270]{for (text,spacing) in [("Café\n\ngyp\n",0.8),("Café\ngyp",1.)]{
  let path=dir.path().join("plain.pdf");fixture(&path,rotation,false,false);
  let mut pdf=Document::load(&path).unwrap();let page=*pdf.get_pages().get(&1).unwrap();pdf.get_dictionary_mut(page).unwrap().set("UserUnit",lopdf::Object::Real(1.7));pdf.save(&path).unwrap();
  let source=e.open_document(&path).unwrap();let original=e.source_bytes(&source.id).unwrap();
  let font=e.fonts().register(std::fs::read(root().join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
  let mut draft=request(&e,&source.id,0);draft.runs=vec![StyledParagraphRun{text:text.into(),style_id:0}];
  draft.style_changes=vec![StyledParagraphStyleChange{style_id:0,source_style_id:0,font_size:Some(18.),color:None,font_id:Some(font.id.clone())}];
  draft.bounds=folio_engine::AnnotationRect{x:150.,y:150.,width:250.,height:250.};draft.line_spacing=spacing;
  let layout=e.layout_styled_paragraph(&source.id,0,&draft).unwrap();if spacing==0.8{assert_eq!(layout.lines.len(),4);assert!(layout.lines[1].ascent>layout.lines[2].ascent);}
  else{assert_eq!(layout.lines.len(),2);assert!(((layout.lines[0].baseline-layout.lines[1].baseline)*1.7-18.).abs()<0.001,"full-font ink must preserve requested18physical pt leading");}
  let preview=e.preview_styled_paragraph(&source.id,0,&draft).unwrap();let saved=dir.path().join("saved.pdf");std::fs::write(&saved,e.source_bytes(&preview.id).unwrap()).unwrap();
  e.fonts().remove(&font.id).unwrap();let reopened=e.open_document(&saved).unwrap();
  let pdf=Document::load(&saved).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let meta:serde_json::Value=serde_json::from_slice(pdf.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();assert_eq!(meta["version"],5);assert_eq!(meta["paragraphs"][0]["ink_vertical"],true);assert_eq!(meta["paragraphs"][0]["native_ink"],true);assert!(meta["paragraphs"][0].get("backgrounds").is_none());
  let ids=e.list_text_runs(&reopened.id,0).unwrap().runs.into_iter().filter(|r|r.text.trim()!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();let s=e.inspect_styled_paragraph(&reopened.id,0,&ids).unwrap();assert_eq!(s.text,text);for (a,b) in [s.bounds.x,s.bounds.y,s.bounds.width,s.bounds.height].into_iter().zip([draft.bounds.x,draft.bounds.y,draft.bounds.width,draft.bounds.height]){assert!((a-b).abs()<0.001,"physical display round trip: {a} vs {b}");}
  let again=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:s.object_indices,expected_text:s.text,runs:s.runs,style_changes:vec![],bounds:s.bounds,alignment:s.alignment.unwrap(),line_spacing:s.line_spacing.unwrap()};
  assert_eq!(e.layout_styled_paragraph(&reopened.id,0,&again).unwrap(),layout);let repeated=e.preview_styled_paragraph(&reopened.id,0,&again).unwrap();
  assert_eq!(e.render_page(&preview.id,0,900).unwrap(),e.render_page(&repeated.id,0,900).unwrap());assert_eq!(e.source_bytes(&source.id).unwrap(),original);assert_eq!(std::fs::read(&path).unwrap(),original.as_ref());
  for id in [&source.id,&preview.id,&reopened.id,&repeated.id]{e.close_document(id).unwrap();}
 }}
}
