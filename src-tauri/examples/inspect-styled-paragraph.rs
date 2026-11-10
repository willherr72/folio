//! Read-only staging probe. Usage: inspect-styled-paragraph input.pdf 0,1,2
//! Does not expose mixed editing or modify the source document.
use folio_engine::PdfEngine;
use std::path::PathBuf;
fn main()->Result<(),Box<dyn std::error::Error>>{
 let args:Vec<_>=std::env::args().collect();
 let path=args.get(1).ok_or("Pass input PDF")?;
 let indices:Vec<usize>=args.get(2).ok_or("Pass comma-separated text-object indices")?.split(',').map(str::parse).collect::<Result<_,_>>()?;
 let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))?;
 let source=engine.open_document(path)?;
 let selection=engine.inspect_styled_paragraph(&source.id,0,&indices)?;
 println!("{}",serde_json::to_string_pretty(&selection)?);
 engine.close_document(&source.id)?;Ok(())
}
