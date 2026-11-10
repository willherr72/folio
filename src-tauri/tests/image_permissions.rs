use folio_engine::{EngineError,ExportRequest,ImageOverlay,Overlay,PagePlan,PdfEngine,RecoveryStore};
use image::{DynamicImage,ImageFormat,Rgba,RgbaImage};
use serde_json::json;
use std::{io::Cursor,path::PathBuf};
#[test]
fn protected_image_cannot_be_copied_through_a_blank_source_or_recovery(){
 let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();let temp=tempfile::tempdir().unwrap();
 let owner=engine.open_document_with_password(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/protection/aes_128.pdf"),"owner").unwrap();
 let mut encoded=Cursor::new(Vec::new());DynamicImage::ImageRgba8(RgbaImage::from_pixel(4,2,Rgba([217,40,80,128]))).write_to(&mut encoded,ImageFormat::Png).unwrap();let bytes=encoded.into_inner();let imported=engine.images().register(bytes.clone()).unwrap();
 let image=ImageOverlay{id:"image".into(),asset_id:imported.image.id.clone(),x:50.,y:50.,width:80.,height:40.,rotation:0};
 let page=PagePlan{crop: None,id:"page".into(),source_id:owner.id.clone(),page_index:0,width:owner.pages[0].width,height:owner.pages[0].height,rotation:0,overlays:vec![Overlay::Image(image.clone())]};
 let protected=temp.path().join("protected-image.pdf");engine.export_pdf(ExportRequest{pages:vec![page.clone()],flatten:false},&protected).unwrap();
 engine.close_document(&owner.id).unwrap();engine.images().release(&imported.token).unwrap();assert!(engine.images().get(&image.asset_id).is_err());
 let reader=engine.open_document_with_password(&protected,"reader").unwrap();assert!(!reader.protection.as_ref().unwrap().permissions.copy_content);
 let retained=engine.images().acquire(&image.asset_id).unwrap();let blank=engine.create_blank_document(page.width,page.height).unwrap();let copied=PagePlan{source_id:blank.id.clone(),..page.clone()};
 // Populate the prepared-image cache through an authorized preview first;
 // cached pixels must not make later cross-source reuse permissible.
 engine.render_page_plan(PagePlan{source_id:reader.id.clone(),..page.clone()},200).unwrap();
 let output=temp.path().join("copied.pdf");
 assert!(matches!(engine.export_pdf(ExportRequest{pages:vec![copied.clone()],flatten:true},&output),Err(EngineError::PermissionDenied(_))));assert!(!output.exists());
 assert!(matches!(engine.render_page_plan(copied.clone(),200),Err(EngineError::PermissionDenied(_))));
 let store=RecoveryStore::new(temp.path().join("recovery")).unwrap();let forged=json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":true,"savedDigest":"","zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"Copy.pdf","pages":[copied],"selectedPageId":"page","selectedOverlayId":"image"}}]});
 assert!(store.save(&engine,forged).is_err());assert!(store.load(&engine).unwrap().is_none());
 let original=PagePlan{source_id:reader.id.clone(),..page};engine.export_pdf(ExportRequest{pages:vec![original.clone()],flatten:false},temp.path().join("allowed-protected.pdf")).unwrap();
 let saved=json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":true,"savedDigest":"","zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"Original.pdf","pages":[original],"selectedPageId":"page","selectedOverlayId":"image"}}]});
 store.save(&engine,saved).unwrap();engine.close_document(&reader.id).unwrap();engine.images().release(&retained.token).unwrap();assert!(engine.images().get(&image.asset_id).is_err());
 let restored=store.load(&engine).unwrap().unwrap();let restored_page:PagePlan=serde_json::from_value(restored["tabs"][0]["document"]["pages"][0].clone()).unwrap();
 assert!(matches!(engine.export_pdf(ExportRequest{pages:vec![copied.clone()],flatten:true},&output),Err(EngineError::PermissionDenied(_))));
 engine.close_document(&restored_page.source_id).unwrap();assert!(engine.images().get(&image.asset_id).is_err());
 // An independently chosen original file grants legitimate reuse of identical bytes.
 let independent=engine.images().register(bytes).unwrap();engine.export_pdf(ExportRequest{pages:vec![copied],flatten:true},&output).unwrap();engine.images().release(&independent.token).unwrap();engine.close_document(&blank.id).unwrap();
}
