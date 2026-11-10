//! Opt-in diagnostic: reads a private source, keeps previews in memory, emits no words.
use folio_engine::{PdfEngine,ParagraphAlignment,StyledParagraphLayoutRequest,StyledParagraphStyleChange};
use std::path::PathBuf;
#[test]
#[ignore="Requires explicit private source path; numeric diagnostics only, no PDF export"]
fn private_highlight_preview_without_export(){
 let path=PathBuf::from(std::env::var_os("FOLIO_PRIVATE_HIGHLIGHT_PDF").expect("Explicit private input required"));
 let original=std::fs::read(&path).expect("Read private input");let root=PathBuf::from(env!("CARGO_MANIFEST_DIR"));
 let e=PdfEngine::start(root.join("resources/pdfium/pdfium.dll")).expect("Start native engine");let source=e.open_document(&path).unwrap_or_else(|_|panic!("Open private input failed"));
 let runs=e.list_text_runs(&source.id,0).unwrap_or_else(|_|panic!("Inspect private page failed"));
 let mut selections=runs.runs.iter().filter_map(|run|e.inspect_styled_paragraph(&source.id,0,&[run.object_index]).ok()).filter(|s|s.styles.iter().any(|style|style.background_color.is_some())).collect::<Vec<_>>();
 assert_eq!(selections.len(),1,"Expected one fully owned highlighted source piece");
 let anchor=runs.runs.iter().find(|run|run.object_index==selections[0].object_indices[0]).expect("Find numeric source index");
 let row=runs.runs.iter().filter(|run|(run.bounds.y-anchor.bounds.y).abs()<2.).map(|run|run.object_index).collect::<Vec<_>>();
 if row.len()>1{if let Ok(s)=e.inspect_styled_paragraph(&source.id,0,&row){selections.push(s);}}
 let complete=selections.last().expect("Checked selection");
 let suggestions=complete.object_indices.iter().map(|id|{let run=runs.runs.iter().find(|run|run.object_index==*id).expect("Source anchor");let suggested=e.suggest_styled_paragraph(&source.id,0,*id,&run.text).unwrap_or_else(|_|panic!("Native suggestion failed"));serde_json::json!({"pieces":suggested.object_indices.len(),"matchesCheckedRow":suggested.object_indices==complete.object_indices,"highlightStyles":suggested.styles.iter().filter(|style|style.background_color.is_some()).count()})}).collect::<Vec<_>>();
 println!("PRIVATE_MIXED_SUGGESTION_PROBE {}",serde_json::json!({"anchors":suggestions,"exported":false}));
 let font=e.fonts().register(std::fs::read(root.join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).expect("Read authored substitute font")).expect("Register substitute font");
 let mut outcomes=vec![];
 for (selection,s) in selections.iter().enumerate(){for mode in 0..8 {
  let mut request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:s.object_indices.clone(),expected_text:s.text.clone(),runs:s.runs.clone(),style_changes:vec![],bounds:s.bounds.clone(),alignment:s.alignment.unwrap_or(ParagraphAlignment::Left),line_spacing:s.line_spacing.unwrap_or(1.2)};
  if mode>0{let run=request.runs.iter_mut().find(|run|s.styles[run.style_id].background_color.is_some()).expect("Find marked style");let text=&mut run.text;let offset=text.find(char::is_whitespace).unwrap_or(text.len());text.insert_str(offset," this");}
  if [2,3,5,6,7].contains(&mode){request.style_changes=s.styles.iter().map(|style|StyledParagraphStyleChange{style_id:style.style_id,source_style_id:style.style_id,font_id:Some(font.id.clone()),font_size:if mode==3||mode==6{Some(8.)}else{None},color:None}).collect();}
  if mode>=4{request.bounds.width=source.pages[0].width-request.bounds.x;}
  if mode==7{request.bounds.height=(request.bounds.height+5.).min(source.pages[0].height-request.bounds.y);}
  let outcome=match e.preview_styled_paragraph(&source.id,0,&request){
   Ok(preview)=>{let rendered=e.render_page(&preview.id,0,900).is_ok();e.close_document(&preview.id).expect("Close in-memory preview");serde_json::json!({"selection":selection,"pieces":s.object_indices.len(),"mode":mode,"preview":true,"rendered":rendered})},
   Err(error)=>{let error=error.to_string().to_lowercase();let category=if error.contains("font")||error.contains("glyph"){"font"}else if error.contains("fit")||error.contains("overflow"){"fit"}else if error.contains("neighboring text"){"neighbor"}else if error.contains("graphics"){"graphics"}else{"other"};serde_json::json!({"selection":selection,"pieces":s.object_indices.len(),"mode":mode,"preview":false,"category":category})}
  };outcomes.push(outcome);
 }}
 e.fonts().remove(&font.id).expect("Release substitute font");e.close_document(&source.id).expect("Close private source");
 assert!(std::fs::read(&path).expect("Re-read private input")==original,"Private source changed");
 println!("PRIVATE_HIGHLIGHT_PROBE {}",serde_json::json!({"selections":selections.len(),"outcomes":outcomes,"sourceUnchanged":true,"exported":false}));
}
