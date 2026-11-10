//! Inspect native edit eligibility without modifying the input PDF.
//! Optional: PDF object-index replacement output.pdf exercises an edit on page 1.
use folio_engine::PdfEngine;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("Pass a PDF path")?;
    let engine = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )?;
    let source = engine.open_document(path)?;
    if let Some(index) = std::env::args().nth(2) {
        let index: usize = index.parse()?;
        let replacement = std::env::args().nth(3).ok_or("Pass replacement text")?;
        let output = PathBuf::from(std::env::args().nth(4).ok_or("Pass a new output path")?);
        let runs = engine.list_text_runs(&source.id, 0)?;
        let run = runs
            .runs
            .iter()
            .find(|r| r.object_index == index)
            .ok_or("Unknown text object")?;
        let edited = engine.replace_text(&source.id, 0, index, &run.text, &replacement)?;
        use std::io::Write;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output)?
            .write_all(&engine.source_bytes(&edited.id)?)?;
        let reopened = engine.open_document(&output)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&engine.list_text_runs(&reopened.id, 0)?)?
        );
        return Ok(());
    }
    for page in 0..source.pages.len() {
        let runs = engine.list_text_runs(&source.id, page)?;
        let forms = engine.list_form_text_runs(&source.id, page)?;
        println!(
            "{}",
            serde_json::to_string_pretty(
                &serde_json::json!({"page":page,"text":runs,"forms":forms})
            )?
        );
    }
    Ok(())
}
