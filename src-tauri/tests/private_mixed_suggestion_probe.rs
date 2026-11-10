//! Opt-in numeric-only suggestion/edit diagnostic; never exports a private PDF.
use folio_engine::{PdfEngine,ParagraphAlignment,StyledParagraphLayoutRequest};
use std::path::PathBuf;
#[test]
#[ignore="Requires explicit private input, no private words or PDF output"]
fn suggested_private_mixed_paragraph_can_preview_in_memory(){
 let path=PathBuf::from(std::env::var_os("FOLIO_PRIVATE_HIGHLIGHT_PDF").expect("Explicit input required"));let original=std::fs::read(&path).expect("Read input");
 let e=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).expect("Native engine");let source=e.open_document(&path).unwrap_or_else(|_|panic!("Open failed"));let runs=e.list_text_runs(&source.id,0).unwrap_or_else(|_|panic!("List failed")).runs;
 let anchor=runs.iter().find(|r|e.inspect_styled_paragraph(&source.id,0,&[r.object_index]).is_ok_and(|s|s.styles.iter().any(|s|s.background_color.is_some()))).expect("Marked anchor");
 let s=e.suggest_styled_paragraph(&source.id,0,anchor.object_index,&anchor.text).unwrap_or_else(|_|panic!("Suggest failed"));
 assert!(s.object_indices.len()==7,"Expected the verified three-row fixture paragraph");
 for id in &s.object_indices{let anchor=runs.iter().find(|r|r.object_index==*id).expect("Source member");let other=e.suggest_styled_paragraph(&source.id,0,*id,&anchor.text).unwrap_or_else(|_|panic!("Member suggestion failed"));assert!(other==s,"Paragraph suggestion changed with its anchor");}
 let geometry=runs.iter().filter(|r|s.object_indices.contains(&r.object_index)).map(|r|serde_json::json!({"index":r.object_index,"size":r.font_size,"bounds":r.bounds,"bytes":r.text.len()})).collect::<Vec<_>>();let mut outcomes=vec![];
 for mode in 0..3 {
  let mut request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:s.object_indices.clone(),expected_text:s.text.clone(),runs:s.runs.clone(),style_changes:vec![],bounds:s.bounds.clone(),alignment:s.alignment.unwrap_or(ParagraphAlignment::Left),line_spacing:s.line_spacing.unwrap_or(1.2)};
  if mode>0{let run=request.runs.iter_mut().find(|r|s.styles[r.style_id].background_color.is_some()).expect("Marked run");let offset=run.text.find(char::is_whitespace).unwrap_or(run.text.len());run.text.insert_str(offset," this");}
  if mode==2{request.bounds.width=source.pages[0].width-request.bounds.x;}
  match e.preview_styled_paragraph(&source.id,0,&request){Ok(preview)=>{let rendered=e.render_page(&preview.id,0,900).is_ok();e.close_document(&preview.id).expect("Close preview");outcomes.push(serde_json::json!({"mode":mode,"preview":true,"rendered":rendered}));},Err(error)=>{let error=error.to_string().to_lowercase();let category=if error.contains("font")||error.contains("glyph"){"font"}else if error.contains("fit")||error.contains("overflow"){"fit"}else if error.contains("neighboring text"){"neighbor"}else if error.contains("graphics"){"graphics"}else{"other"};outcomes.push(serde_json::json!({"mode":mode,"preview":false,"category":category}));}}
 }
 e.close_document(&source.id).expect("Close source");assert!(std::fs::read(&path).expect("Re-read input")==original,"Source changed");
 assert!(outcomes[..2].iter().all(|v|v["preview"]==true&&v["rendered"]==true),"Unchanged or inserted draft did not preview in the suggested area");
 println!("PRIVATE_SUGGESTED_EDIT_PROBE {}",serde_json::json!({"pieces":s.object_indices.len(),"styles":s.styles.len(),"geometry":geometry,"outcomes":outcomes,"sourceUnchanged":true,"exported":false}));
}
