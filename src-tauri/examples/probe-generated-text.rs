//! Read native ownership/generation flags without modifying the input PDF.
use pdfium_render::prelude::*;
use std::path::PathBuf;
fn main()->Result<(),Box<dyn std::error::Error>>{
    let path=std::env::args().nth(1).ok_or("Pass a PDF path")?;
    let pdfium=Pdfium::new(Pdfium::bind_to_library(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))?);
    let doc=pdfium.load_pdf_from_file(&path,None)?;let page=doc.pages().get(0)?;let text=page.text()?;
    let object=page.objects().get(0)?;let object=object.as_text_object().ok_or("First object is not text")?;
    let chars=text.chars_for_object(object)?;let mut rows=vec![];
    for ch in chars.iter(){rows.push(serde_json::json!({"index":ch.index(),"text":ch.unicode_string(),"generated":ch.is_generated()?}));}
    let mut page_rows=vec![];
    for ch in text.chars().iter().take(8){page_rows.push(serde_json::json!({"index":ch.index(),"text":ch.unicode_string(),"generated":ch.is_generated()?,"hasOwner":ch.text_object().is_ok()}));}
    println!("{}",serde_json::to_string_pretty(&serde_json::json!({"objectText":text.for_object(object),"characters":rows,"pageCharacters":page_rows}))?);
    Ok(())
}
