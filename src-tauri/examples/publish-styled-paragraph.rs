//! Native acceptance probe: input.pdf request.json output.pdf. Calls the real
//! preview/export route; the engine protects the original input path. Optional
//! trailing arguments register local font files; requests use their SHA-256 IDs.
use folio_engine::{PdfEngine,StyledParagraphLayoutRequest,ExportRequest,PagePlan};
use std::path::PathBuf;
fn main()->Result<(),Box<dyn std::error::Error>>{
 let args:Vec<_>=std::env::args().collect();let input=args.get(1).ok_or("Pass input PDF")?;
 let request:StyledParagraphLayoutRequest=serde_json::from_slice(&std::fs::read(args.get(2).ok_or("Pass request JSON")?)?)?;
 let output=args.get(3).ok_or("Pass output PDF")?;
 let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))?;
 for path in args.iter().skip(4){engine.fonts().register(std::fs::read(path)?)?;}
 let source=engine.open_document(input)?;
 let selection=engine.inspect_styled_paragraph(&source.id,0,&request.object_indices)?;
 let layout=engine.layout_styled_paragraph(&source.id,0,&request)?;
 let edited=engine.preview_styled_paragraph(&source.id,0,&request)?;
 engine.export_pdf(ExportRequest{flatten:false,pages:vec![PagePlan{crop: None,id:"acceptance".into(),source_id:edited.id.clone(),page_index:0,width:edited.pages[0].width,height:edited.pages[0].height,rotation:0,overlays:source.pages[0].overlays.clone()}]},output)?;
 println!("{}",serde_json::to_string_pretty(&serde_json::json!({"selection":selection,"layout":layout}))?);
 engine.close_document(&edited.id)?;engine.close_document(&source.id)?;Ok(())
}
