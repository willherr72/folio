#![cfg(windows)]
use folio_engine::{private_storage::{seal,unseal,Purpose},ExportRequest,PdfEngine,RecoveryStore};
use image::{DynamicImage,ImageFormat,Rgba,RgbaImage};
use serde_json::{json,Value};
use std::{fs,io::Cursor,path::{Path,PathBuf}};
static LOCK:std::sync::Mutex<()>=std::sync::Mutex::new(());
fn engine()->PdfEngine {PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn bytes(red:u8)->Vec<u8>{let mut out=Cursor::new(Vec::new());DynamicImage::ImageRgba8(RgbaImage::from_pixel(4,2,Rgba([red,20,40,128]))).write_to(&mut out,ImageFormat::Png).unwrap();out.into_inner()}
fn workspace(source:&str,id:&str)->Value {
 json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":true,"savedDigest":"","zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"Images.pdf","selectedPageId":"page","selectedOverlayId":"photo","pages":[{"id":"page","sourceId":source,"pageIndex":0,"width":200,"height":300,"rotation":90,"overlays":[{"type":"image","id":"photo","assetId":id,"x":80,"y":90,"width":80,"height":40,"rotation":90}]}]}}]})
}
fn checkpoint(root:&Path)->PathBuf {let mut paths:Vec<_>=fs::read_dir(root).unwrap().map(|e|e.unwrap().path()).filter(|p|p.extension().is_some_and(|e|e=="json")).collect();paths.sort();paths.pop().unwrap()}
fn read_manifest(root:&Path)->Value {serde_json::from_slice(&unseal(Purpose::Workspace,&fs::read(checkpoint(root)).unwrap(),32*1024*1024).unwrap()).unwrap()}
fn write_manifest(root:&Path,value:&Value){fs::write(checkpoint(root),seal(Purpose::Workspace,&serde_json::to_vec(value).unwrap()).unwrap()).unwrap();}

#[test]
fn encrypted_images_restore_after_originals_disappear_and_source_owners_are_independent(){
 let _lock=LOCK.lock().unwrap_or_else(|e|e.into_inner());let temp=tempfile::tempdir().unwrap();let engine=engine();
 let original=temp.path().join("photo.png");let original_bytes=bytes(211);fs::write(&original,&original_bytes).unwrap();
 let image=engine.import_image(&original).unwrap();let source=engine.create_blank_document(200.,300.).unwrap();
 let root=temp.path().join("recovery");let store=RecoveryStore::new(root.clone()).unwrap();
 store.save(&engine,workspace(&source.id,&image.image.id)).unwrap();
 let manifest=read_manifest(&root);assert_eq!(manifest["images"].as_object().unwrap().len(),1);
 let payload=root.join("images").join(manifest["images"][&image.image.id]["file"].as_str().unwrap());
 let encrypted=fs::read(&payload).unwrap();assert!(encrypted.starts_with(b"FOLIO-SEALED-"));
 assert!(!encrypted.windows(original_bytes.len()).any(|w|w==original_bytes));
 engine.close_document(&source.id).unwrap();engine.images().release(&image.token).unwrap();fs::remove_file(original).unwrap();
 let recovered=store.load(&engine).unwrap().unwrap();let again=store.load(&engine).unwrap().unwrap();
 assert_eq!(engine.images().get(&image.image.id).unwrap().bytes(),original_bytes);
 let page=recovered["tabs"][0]["document"]["pages"][0].clone();let second=again["tabs"][0]["document"]["pages"][0]["sourceId"].as_str().unwrap();
 let request:ExportRequest=serde_json::from_value(json!({"pages":[page.clone()],"flatten":false})).unwrap();
 engine.export_pdf(request,temp.path().join("recovered.pdf")).unwrap();
 engine.close_document(page["sourceId"].as_str().unwrap()).unwrap();assert!(engine.images().get(&image.image.id).is_ok());
 let workspace_owner=engine.images().acquire(&image.image.id).unwrap();engine.close_document(second).unwrap();assert!(engine.images().get(&image.image.id).is_ok());
 engine.images().release(&workspace_owner.token).unwrap();assert!(engine.images().get(&image.image.id).is_err());
 store.clear().unwrap();assert_eq!(fs::read_dir(root.join("images")).unwrap().count(),0);
}

#[test]
fn invalid_payload_mapping_and_late_restore_failure_preserve_existing_owners(){
 let _lock=LOCK.lock().unwrap_or_else(|e|e.into_inner());let temp=tempfile::tempdir().unwrap();let engine=engine();
 let image=engine.images().register(bytes(212)).unwrap();let source=engine.create_blank_document(200.,300.).unwrap();
 let root=temp.path().join("recovery");let store=RecoveryStore::new(root.clone()).unwrap();store.save(&engine,workspace(&source.id,&image.image.id)).unwrap();
 let original=read_manifest(&root);let payload=root.join("images").join(original["images"][&image.image.id]["file"].as_str().unwrap());let encrypted=fs::read(&payload).unwrap();
 for change in 0..4 {
  let mut manifest=original.clone();
  match change {0=>manifest["images"][&image.image.id]["file"]=json!("../outside.image"),1=>manifest["images"]=json!({}),2=>manifest["images"][&image.image.id]["length"]=json!(u64::MAX),_=>manifest["workspace"]["tabs"][0]["document"]["pages"][0]["pageIndex"]=json!(99)};
  write_manifest(&root,&manifest);assert!(store.load(&engine).is_err(),"case {change}");assert!(engine.images().get(&image.image.id).is_ok());
 }
 write_manifest(&root,&original);let mut bad=encrypted.clone();let last=bad.len()-1;bad[last]^=1;fs::write(&payload,bad).unwrap();
 assert!(store.load(&engine).is_err());assert!(engine.images().get(&image.image.id).is_ok());
 fs::remove_file(&payload).unwrap();assert!(store.load(&engine).is_err());fs::write(&payload,encrypted).unwrap();
 // All failed restores must have released their own tokens, even when the asset preexisted.
 engine.images().release(&image.token).unwrap();assert!(engine.images().get(&image.image.id).is_err());
 engine.close_document(&source.id).unwrap();
}

#[test]
fn image_payloads_are_deduplicated_and_collected_only_after_both_generations_release_them(){
 let _lock=LOCK.lock().unwrap_or_else(|e|e.into_inner());let temp=tempfile::tempdir().unwrap();let engine=engine();
 let image=engine.images().register(bytes(213)).unwrap();let source=engine.create_blank_document(200.,300.).unwrap();let root=temp.path().join("recovery");let store=RecoveryStore::new(root.clone()).unwrap();
 let with=workspace(&source.id,&image.image.id);store.save(&engine,with.clone()).unwrap();let manifest=read_manifest(&root);let path=root.join("images").join(manifest["images"][&image.image.id]["file"].as_str().unwrap());let encrypted=fs::read(&path).unwrap();
 store.save(&engine,with.clone()).unwrap();assert_eq!(fs::read(&path).unwrap(),encrypted);assert_eq!(fs::read_dir(root.join("images")).unwrap().count(),1);
 let mut without=with;without["tabs"][0]["document"]["pages"][0]["overlays"]=json!([]);without["tabs"][0]["document"]["selectedOverlayId"]=Value::Null;
 store.save(&engine,without.clone()).unwrap();assert!(path.exists());fs::write(root.join("images/keep.txt"),b"unrelated").unwrap();
 store.save(&engine,without).unwrap();assert!(!path.exists());assert!(root.join("images/keep.txt").exists());
 engine.images().release(&image.token).unwrap();engine.close_document(&source.id).unwrap();
}

#[test]
fn restore_capacity_failure_is_atomic_and_retry_can_succeed(){
 let _lock=LOCK.lock().unwrap_or_else(|e|e.into_inner());let temp=tempfile::tempdir().unwrap();let engine=engine();
 let image=engine.images().register(bytes(214)).unwrap();let source=engine.create_blank_document(200.,300.).unwrap();let root=temp.path().join("recovery");let store=RecoveryStore::new(root).unwrap();
 store.save(&engine,workspace(&source.id,&image.image.id)).unwrap();engine.images().release(&image.token).unwrap();engine.close_document(&source.id).unwrap();
 let owners:Vec<_>=(0..64).map(|value|engine.images().register(bytes(value)).unwrap()).collect();
 assert!(store.load(&engine).is_err());assert_eq!(engine.images().ids().unwrap().len(),64);assert!(engine.images().get(&image.image.id).is_err());
 for owner in &owners{assert!(engine.images().get(&owner.image.id).is_ok());}
 engine.images().release(&owners[0].token).unwrap();let restored=store.load(&engine).unwrap().unwrap();
 engine.close_document(restored["tabs"][0]["document"]["pages"][0]["sourceId"].as_str().unwrap()).unwrap();
 assert!(engine.images().get(&image.image.id).is_err());assert_eq!(engine.images().ids().unwrap().len(),63);
 engine.images().release_all(&owners).unwrap();assert!(engine.images().ids().unwrap().is_empty());
}

#[test]
fn protected_source_images_recover_without_original_file_and_keep_credentials(){
 let _lock=LOCK.lock().unwrap_or_else(|e|e.into_inner());let temp=tempfile::tempdir().unwrap();let engine=engine();
 let path=temp.path().join("protected.pdf");fs::copy(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/protection/aes_128.pdf"),&path).unwrap();
 let source=engine.open_document_with_password(&path,"owner").unwrap();let image=engine.images().register(bytes(215)).unwrap();
 let mut saved=workspace(&source.id,&image.image.id);saved["tabs"][0]["document"]["pages"][0]["width"]=json!(source.pages[0].width);saved["tabs"][0]["document"]["pages"][0]["height"]=json!(source.pages[0].height);
 let store=RecoveryStore::new(temp.path().join("recovery")).unwrap();store.save(&engine,saved).unwrap();engine.close_document(&source.id).unwrap();engine.images().release(&image.token).unwrap();fs::remove_file(path).unwrap();
 let restored=store.load(&engine).unwrap().unwrap();let page=restored["tabs"][0]["document"]["pages"][0].clone();assert_eq!(page["protection"]["owner"],true);
 let output=temp.path().join("restored.pdf");engine.export_pdf(serde_json::from_value(json!({"pages":[page.clone()],"flatten":false})).unwrap(),&output).unwrap();
 for password in ["reader","owner"]{let opened=engine.open_document_with_password(&output,password).unwrap();assert!(opened.protection.is_some());assert_eq!(opened.pages[0].overlays.len(),1);engine.close_document(&opened.id).unwrap();}
 engine.close_document(page["sourceId"].as_str().unwrap()).unwrap();assert!(engine.images().get(&image.image.id).is_err());
}
