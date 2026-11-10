//! Local producer round-trip diagnostic. Original inputs are never modified.
use folio_engine::{ExportRequest, PagePlan, PdfEngine};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = PathBuf::from(std::env::args().nth(1).ok_or("input directory")?);
    let output = PathBuf::from(std::env::args().nth(2).ok_or("new output directory")?);
    std::fs::create_dir(&output)?;
    let engine = PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))?;
    let mut files = std::fs::read_dir(input)?.collect::<Result<Vec<_>, _>>()?;
    files.sort_by_key(|entry| entry.path());
    let mut results = vec![];
    for entry in files {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("pdf") { continue; }
        let before = std::fs::read(&path)?;
        let source = engine.open_document(&path)?;
        let pages = source.pages.iter().enumerate().map(|(index, page)| PagePlan {
            id: format!("page-{index}"), source_id: source.id.clone(), page_index: index,
            width: page.width, height: page.height, rotation: 0, crop: None,
            overlays: page.overlays.clone(),
        }).collect();
        let destination = output.join(path.file_name().ok_or("filename")?);
        let result = engine.export_pdf(ExportRequest { pages, flatten: false }, &destination);
        results.push(serde_json::json!({ "source": path, "output": destination,
            "success": result.is_ok(), "error": result.err().map(|error| error.to_string()) }));
        engine.close_document(&source.id)?;
        assert_eq!(before, std::fs::read(&path)?);
    }
    std::fs::write(output.join("results.json"), serde_json::to_vec_pretty(&results)?)?;
    println!("Audited {} unchanged producer saves", results.len());
    Ok(())
}
