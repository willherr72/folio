//! Feasibility probe only: inspect OCR transport through Folio's actual reading path.
use folio_engine::PdfEngine;
use std::path::PathBuf;
fn main() {
    let root=PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let engine=PdfEngine::start(root.join("resources/pdfium/pdfium.dll")).unwrap();
    let mut results=Vec::new();
    for rotation in [0,90,180,270] {
        let source=engine.open_document(root.join(format!("../artifacts/ocr-probe/searchable-line-{rotation}.pdf"))).unwrap();
        let page=engine.page_text(&source.id,0).unwrap();
        let text:String=page.characters.iter().map(|c|c.text.as_str()).collect();
        let bounded=page.characters.iter().filter(|c|!c.text.trim().is_empty()).all(|c|c.x>=-0.1&&c.y>=-0.1&&c.x+c.width<=source.pages[0].width+0.1&&c.y+c.height<=source.pages[0].height+0.1);
        results.push(serde_json::json!({"rotation":rotation,"text":text,"boundsInsidePage":bounded}));
        engine.close_document(&source.id).unwrap();
    }
    println!("{}",serde_json::to_string_pretty(&results).unwrap());
}
