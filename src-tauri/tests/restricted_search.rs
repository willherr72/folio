use folio_engine::{EngineError,ExportRequest,PagePlan,PdfEngine,PageSearchOptions};
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

#[test]
fn protected_options_filter_case_and_whole_words_without_exposing_text() {
    let e=engine();let input=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/protection/aes_256.pdf");
    let original=fs::read(&input).unwrap();let source=e.open_document_with_password(&input,"reader").unwrap();
    let exact=PageSearchOptions {match_case:true,whole_words:true};
    let whole=PageSearchOptions {match_case:false,whole_words:true};
    assert_eq!(e.search_page(&source.id,0,"Foli").unwrap().matches.len(),1);
    assert!(e.search_page_with_options(&source.id,0,"Foli",whole).unwrap().matches.is_empty());
    assert!(e.search_page_with_options(&source.id,0,"FOLIO",exact).unwrap().matches.is_empty());
    let found=e.search_page_with_options(&source.id,0,"Folio",exact).unwrap();assert_eq!(found.matches.len(),1);
    assert_eq!(found.matches,e.search_page(&source.id,0,"folio").unwrap().matches);
    let json=serde_json::to_value(&found).unwrap();assert_eq!(json.as_object().unwrap().len(),3);
    assert!(!serde_json::to_string(&found).unwrap().contains("Folio"));
    assert!(matches!(e.page_text(&source.id,0),Err(EngineError::PermissionDenied(_))));
    assert!(e.search_page_with_options(&source.id,0,&"İ".repeat(512),whole).unwrap().matches.is_empty());
    assert!(e.search_page_with_options(&source.id,0,&"İ".repeat(513),whole).is_err());
    assert_eq!(fs::read(input).unwrap(),original);e.close_document(&source.id).unwrap();
}


#[test]
fn compact_reading_text_matches_exact_rotated_unicode_character_sequence() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    let fixtures=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/source-accent-clusters/v1");
    let authority:serde_json::Value=serde_json::from_slice(&fs::read(fixtures.join("authority.json")).unwrap()).unwrap();
    for row in authority["cases"].as_array().unwrap().iter().take(2) {
        let input=fixtures.join(row["path"].as_str().unwrap());let bytes=fs::read(&input).unwrap();
        let source=e.open_document(&input).unwrap();
        for rotation in [0,90,180,270] {
            let path=temp.path().join(format!("rotation-{rotation}.pdf"));let p=&source.pages[0];
            e.export_pdf(ExportRequest{flatten:false,pages:vec![PagePlan{crop:None,id:"p".into(),source_id:source.id.clone(),page_index:0,width:p.width,height:p.height,rotation,overlays:vec![]}]},&path).unwrap();
            let rotated=e.open_document(&path).unwrap();
            let expected:String=e.page_text(&rotated.id,0).unwrap().characters.into_iter().map(|c|c.text).collect();
            assert!(expected.contains("Cafe\u{301}"),"{expected:?}");
            assert_eq!(e.page_reading_text(&rotated.id,0).unwrap(),expected);
            e.close_document(&rotated.id).unwrap();
        }
        assert_eq!(fs::read(input).unwrap(),bytes);e.close_document(&source.id).unwrap();
    }
}

#[test]
fn compact_reading_text_enforces_permissions_page_bounds_and_source_lifetime() {
    let e=engine();let input=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/protection/aes_256.pdf");
    let bytes=fs::read(&input).unwrap();let reader=e.open_document_with_password(&input,"reader").unwrap();
    assert!(matches!(e.page_reading_text(&reader.id,0),Err(EngineError::PermissionDenied(_))));
    let owner=e.open_document_with_password(&input,"owner").unwrap();
    let expected:String=e.page_text(&owner.id,0).unwrap().characters.into_iter().map(|c|c.text).collect();
    assert!(!expected.is_empty());assert_eq!(e.page_reading_text(&owner.id,0).unwrap(),expected);
    assert!(e.page_reading_text(&owner.id,usize::MAX).is_err());
    e.close_document(&owner.id).unwrap();assert!(e.page_reading_text(&owner.id,0).is_err());
    assert!(e.page_reading_text("missing-compact-source",0).is_err());
    e.close_document(&reader.id).unwrap();assert_eq!(fs::read(input).unwrap(),bytes);
}
