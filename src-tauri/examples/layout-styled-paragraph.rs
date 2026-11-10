//! Read-only diagnostic: input.pdf request.json. Prints source style inventory and
//! raw-PDF layout; does not publish an edited PDF or bypass publication checks.
use folio_engine::{PdfEngine,StyledParagraphLayoutRequest};
use std::path::PathBuf;
fn main()->Result<(),Box<dyn std::error::Error>>{
 let args:Vec<_>=std::env::args().collect();let input=args.get(1).ok_or("Pass input PDF")?;
 let request:StyledParagraphLayoutRequest=serde_json::from_slice(&std::fs::read(args.get(2).ok_or("Pass request JSON")?)?)?;
 let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))?;
 let source=engine.open_document(input)?;
 let selection=engine.inspect_styled_paragraph(&source.id,0,&request.object_indices)?;
 let layout=engine.layout_styled_paragraph(&source.id,0,&request)?;
 println!("{}",serde_json::to_string_pretty(&serde_json::json!({"selection":selection,"layout":layout}))?);
 engine.close_document(&source.id)?;Ok(())
}
