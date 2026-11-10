//! Local acceptance probe. Never overwrites the input or an existing output.
//! Usage: inspect-paragraph input.pdf 22,23,24 [output.pdf] [replacement]
use folio_engine::PdfEngine;
use std::{io::Write, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let engine = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )?;
    let source = engine.open_document(args.get(1).ok_or("Pass input PDF")?)?;
    let indices: Vec<usize> = args
        .get(2)
        .ok_or("Pass comma-separated object indices")?
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    let selection = engine.inspect_text_paragraph(&source.id, 0, &indices)?;
    println!("{}", serde_json::to_string_pretty(&selection)?);
    if let Some(output) = args.get(3) {
        let replacement = args
            .get(4)
            .cloned()
            .unwrap_or_else(|| selection.text.replacen("How ", "How this ", 1));
        let edited = engine.preview_text_paragraph(
            &source.id,
            0,
            &indices,
            &selection.text,
            &replacement,
            &selection.bounds,
        )?;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?
            .write_all(&engine.source_bytes(&edited.id)?)?;
        let reopened = engine.open_document(output)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&engine.list_text_runs(&reopened.id, 0)?)?
        );
        std::fs::write(
            format!("{output}.png"),
            engine.render_page(&reopened.id, 0, 1100)?,
        )?;
        engine.close_document(&reopened.id)?;
        engine.close_document(&edited.id)?;
    }
    engine.close_document(&source.id)?;
    Ok(())
}
