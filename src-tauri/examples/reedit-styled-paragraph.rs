//! Authored-fixture acceptance probe: source.pdf request.json output-directory
//! [font.ttf ...]. Publishes, exports/reopens and performs three actual no-op edits.
use folio_engine::{PdfEngine,StyledParagraphLayoutRequest,ExportRequest,PagePlan};
use std::{path::PathBuf,time::Instant};
fn main()->Result<(),Box<dyn std::error::Error>>{
 let args=std::env::args().collect::<Vec<_>>();let input=args.get(1).ok_or("Pass source PDF")?;let mut request:StyledParagraphLayoutRequest=serde_json::from_slice(&std::fs::read(args.get(2).ok_or("Pass request JSON")?)?)?;let directory=PathBuf::from(args.get(3).ok_or("Pass output directory")?);std::fs::create_dir_all(&directory)?;
 let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))?;for font in args.iter().skip(4){engine.fonts().register(std::fs::read(font)?)?;}
 let original=std::fs::read(input)?;let mut source=engine.open_document(input)?;let logical=request.runs.iter().map(|r|r.text.as_str()).collect::<String>();let mut reference=None;let mut reports=Vec::new();
 for iteration in 0..4{
  let started=Instant::now();let layout=engine.layout_styled_paragraph(&source.id,0,&request)?;let count=layout.lines.iter().map(|l|l.runs.len()).sum::<usize>();
  let edited=engine.preview_styled_paragraph(&source.id,0,&request)?;let pixels=engine.render_page(&edited.id,0,1000)?;
  if let Some(expected)=&reference{assert_eq!(expected,&pixels,"No-op raster changed");}else{reference=Some(pixels);}
  let output=directory.join(format!("edit-{iteration}.pdf"));engine.export_pdf(ExportRequest{flatten:false,pages:vec![PagePlan{crop: None,id:"p".into(),source_id:edited.id.clone(),page_index:0,width:edited.pages[0].width,height:edited.pages[0].height,rotation:0,overlays:source.pages[0].overlays.clone()}]},&output)?;
  engine.close_document(&source.id)?;engine.close_document(&edited.id)?;source=engine.open_document(&output)?;
  let runs=engine.list_text_runs(&source.id,0)?.runs;let ids=runs[runs.len()-count..].iter().map(|r|r.object_index).collect::<Vec<_>>();let selection=engine.inspect_styled_paragraph(&source.id,0,&ids)?;assert_eq!(selection.text,logical);assert_eq!(selection.bounds,request.bounds);assert_eq!(selection.alignment,Some(request.alignment));assert_eq!(selection.line_spacing,Some(request.line_spacing));
  request=StyledParagraphLayoutRequest{baseline_anchor:selection.baseline_anchor,object_indices:ids,expected_text:selection.text,runs:selection.runs,style_changes:vec![],bounds:selection.bounds,alignment:selection.alignment.ok_or("Missing saved alignment")?,line_spacing:selection.line_spacing.ok_or("Missing saved spacing")?};
  reports.push(serde_json::json!({"iteration":iteration,"fragments":count,"textBytes":logical.len(),"milliseconds":started.elapsed().as_millis(),"output":output}));
  eprintln!("Completed iteration {iteration}: {count} fragments in {} ms",started.elapsed().as_millis());
 }
 let mut budget_refusal=false;
 if request.object_indices.len()==4096{
  let before=engine.source_bytes(&source.id)?;let other=engine.list_text_runs(&source.id,0)?.runs.into_iter().find(|r|!request.object_indices.contains(&r.object_index)).ok_or("Stress fixture needs a neighbor")?;
  let selection=engine.inspect_styled_paragraph(&source.id,0,&[other.object_index])?;
  let extra=StyledParagraphLayoutRequest{baseline_anchor:selection.baseline_anchor,object_indices:vec![other.object_index],expected_text:selection.text,runs:vec![folio_engine::StyledParagraphRun{text:"X".into(),style_id:0}],style_changes:vec![],bounds:selection.bounds,alignment:folio_engine::ParagraphAlignment::Left,line_spacing:1.2};
  let error=engine.preview_styled_paragraph(&source.id,0,&extra).unwrap_err().to_string();assert!(error.contains("fragment limit"),"{error}");assert_eq!(engine.source_bytes(&source.id)?,before);budget_refusal=true;
 }
 engine.close_document(&source.id)?;assert_eq!(std::fs::read(input)?,original);println!("{}",serde_json::to_string_pretty(&serde_json::json!({"passed":true,"iterations":reports,"budgetRefusal":budget_refusal}))?);Ok(())
}
