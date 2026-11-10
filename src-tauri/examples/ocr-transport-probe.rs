//! Feasibility only: verify owned OCR Form metadata through Folio's real export.
use folio_engine::{PdfEngine,ExportRequest};
use std::path::PathBuf;
fn main(){
    let root=PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let e=PdfEngine::start(root.join("resources/pdfium/pdfium.dll")).unwrap();
    let source=e.open_document(root.join("../artifacts/ocr-probe/transport-source.pdf")).unwrap();
    let request:ExportRequest=serde_json::from_value(serde_json::json!({"pages":[{
        "id":"p","sourceId":source.id,"pageIndex":0,"width":600,"height":400,"rotation":0,"overlays":[]
    }]})).unwrap();
    let path=root.join("../artifacts/ocr-probe/transport-export.pdf");
    e.export_pdf(request,path.clone()).unwrap();
    e.close_document(&source.id).unwrap();
    let document=lopdf::Document::load(path).unwrap();
    let markers:Vec<_>=document.objects.iter().filter_map(|(id,value)|value.as_stream().ok()
        .and_then(|s|s.dict.get(b"FolioOCR").ok()).map(|m|format!("{id:?}: {m:?}"))).collect();
    println!("{}",serde_json::to_string_pretty(&markers).unwrap());
    assert_eq!(markers.len(),1,"OCR metadata must survive the real export graph");
}
