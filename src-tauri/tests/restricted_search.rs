use folio_engine::{EngineError,ExportRequest,PagePlan,PdfEngine};
use std::{fs,path::PathBuf};
fn engine()->PdfEngine {PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}

#[test]
fn restricted_search_returns_rectangles_without_copying_page_text() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    let input=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/protection/aes_256.pdf");
    let original=fs::read(&input).unwrap();let source=e.open_document_with_password(&input,"reader").unwrap();
    assert!(matches!(e.page_text(&source.id,0),Err(EngineError::PermissionDenied(_))));
    let found=e.search_page(&source.id,0,"  FOLIO  ").unwrap();
    assert!(found.has_text&&!found.truncated);assert_eq!(found.matches.len(),1);
    assert!(found.matches[0].iter().all(|r|r.x>=30.0&&r.y>=35.0&&r.y<70.0&&r.width>0.0&&r.height>0.0));
    let serialized=serde_json::to_string(&found).unwrap();
    assert!(!serialized.contains("Protected")&&!serialized.contains("Folio")&&!serialized.contains("characters"));
    assert_eq!(e.search_page(&source.id,0,"Original \r\n paragraph.").unwrap().matches.len(),1);
    assert!(e.search_page(&source.id,0,"Not present").unwrap().matches.is_empty());
    assert!(e.search_page(&source.id,0,&"a".repeat(1025)).is_err());
    assert!(e.search_page(&source.id,100,"Folio").is_err());
    let owner=e.open_document_with_password(&input,"owner").unwrap();
    for rotation in [90,180,270] {
        let path=temp.path().join(format!("rotated-{rotation}.pdf"));
        let p=&owner.pages[0];e.export_pdf(ExportRequest {flatten:false,pages:vec![PagePlan {crop: None,id:"p".into(),source_id:owner.id.clone(),page_index:0,width:p.width,height:p.height,rotation,overlays:p.overlays.clone()}]},&path).unwrap();
        let rotated=e.open_document_with_password(&path,"reader").unwrap();
        let found=e.search_page(&rotated.id,0,"Protected Folio fixture").unwrap();
        assert_eq!(found.matches.len(),1);
        assert!(found.matches[0].iter().all(|r|r.x>=0.0&&r.y>=0.0&&r.x+r.width<=rotated.pages[0].width+0.1&&r.y+r.height<=rotated.pages[0].height+0.1));
        e.close_document(&rotated.id).unwrap();
    }
    assert_eq!(fs::read(input).unwrap(),original);e.close_document(&source.id).unwrap();e.close_document(&owner.id).unwrap();
}
