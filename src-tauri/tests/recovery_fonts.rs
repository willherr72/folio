use folio_engine::{ExportRequest, PdfEngine, RecoveryStore};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
static TEST_LOCK: Mutex<()> = Mutex::new(());
fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn fixture(temp: &Path) -> PathBuf {
    let path = temp.join("source.pdf");
    fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples/Welcome to Folio.pdf"),
        &path,
    )
    .unwrap();
    path
}
fn font_bytes() -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf"),
    )
    .unwrap()
}
fn workspace(source: &folio_engine::DocumentInfo, font: Option<&str>) -> Value {
    let page = &source.pages[0];
    let mut text = json!({"type":"text","id":"added","text":"Custom café Ω","x":30,"y":70,"fontSize":18,"color":"#123456","rotation":90});
    if let Some(id) = font {
        text["fontId"] = json!(id);
    }
    json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":true,"savedDigest":"","zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"Font.pdf","pages":[{"id":"page","sourceId":source.id,"pageIndex":0,"width":page.width,"height":page.height,"rotation":90,"overlays":[text]}],"selectedPageId":"page","selectedOverlayId":"added"}}]})
}
fn manifests(root: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    paths
}
#[test]
fn recovery_keeps_exact_font_bytes_after_font_and_pdf_sources_disappear() {
    let _lock = TEST_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    let source_path = fixture(temp.path());
    let source = engine.open_document(&source_path).unwrap();
    let imported_path = temp.path().join("Imported.ttf");
    let bytes = font_bytes();
    fs::write(&imported_path, &bytes).unwrap();
    let font = engine
        .fonts()
        .register(fs::read(&imported_path).unwrap())
        .unwrap();
    fs::remove_file(&imported_path).unwrap();
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    store
        .save(&engine, workspace(&source, Some(&font.id)))
        .unwrap();
    let manifest: Value = serde_json::from_slice(&fs::read(&manifests(&root)[0]).unwrap()).unwrap();
    assert_eq!(
        manifest["fonts"].as_object().unwrap().len(),
        1,
        "font bytes need their own checkpoint resource"
    );
    assert!(!fs::read_to_string(&manifests(&root)[0])
        .unwrap()
        .contains("base64"));
    engine.close_document(&source.id).unwrap();
    engine.fonts().remove(&font.id).unwrap();
    fs::remove_file(source_path).unwrap();
    drop(store);
    let store = RecoveryStore::new(root.clone()).unwrap();
    let recovered = store.load(&engine).unwrap().unwrap();
    assert_eq!(
        &*engine.fonts().get(&font.id).unwrap().bytes,
        bytes.as_slice()
    );
    let page = recovered["tabs"][0]["document"]["pages"][0].clone();
    assert_eq!(page["overlays"][0]["fontId"], font.id);
    let output = temp.path().join("recovered.pdf");
    let request: ExportRequest =
        serde_json::from_value(json!({"pages":[page],"flatten":true})).unwrap();
    engine.export_pdf(request, &output).unwrap();
    let reopened = engine.open_document(&output).unwrap();
    assert!(engine
        .extract_text(&reopened.id, 0)
        .unwrap()
        .contains("Custom café Ω"));
    store.clear().unwrap();
    assert_eq!(fs::read_dir(root.join("fonts")).unwrap().count(), 0);
}
#[test]
fn recovery_reuses_font_files_and_collects_only_unreferenced_generations() {
    let _lock = TEST_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    let source = engine.open_document(fixture(temp.path())).unwrap();
    let font = engine.fonts().register(font_bytes()).unwrap();
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    let with = workspace(&source, Some(&font.id));
    store.save(&engine, with.clone()).unwrap();
    let file = fs::read_dir(root.join("fonts"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let modified = fs::metadata(&file).unwrap().modified().unwrap();
    store.save(&engine, with).unwrap();
    assert_eq!(fs::metadata(&file).unwrap().modified().unwrap(), modified);
    let mut without = workspace(&source, None);
    without["tabs"][0]["document"]["pages"][0]["overlays"] = json!([]);
    without["tabs"][0]["document"]["selectedOverlayId"] = Value::Null;
    store.save(&engine, without.clone()).unwrap();
    assert!(file.exists());
    fs::write(root.join("fonts/user-note.txt"), "Keep this unrelated file").unwrap();
    store.save(&engine, without).unwrap();
    assert!(!file.exists());
    assert!(root.join("fonts/user-note.txt").exists());
}
#[test]
fn invalid_font_snapshot_fails_without_registering_partial_assets() {
    let _lock = TEST_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    let source = engine.open_document(fixture(temp.path())).unwrap();
    let font = engine.fonts().register(font_bytes()).unwrap();
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    store
        .save(&engine, workspace(&source, Some(&font.id)))
        .unwrap();
    engine.fonts().remove(&font.id).unwrap();
    let file = fs::read_dir(root.join("fonts"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut bytes = fs::read(&file).unwrap();
    bytes[0] ^= 0xff;
    fs::write(&file, bytes).unwrap();
    assert!(store.load(&engine).is_err());
    assert!(engine.fonts().get(&font.id).is_err());
    let path = manifests(&root).pop().unwrap();
    let mut manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["fonts"][&font.id]["file"] = json!("../outside.ttf");
    fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(store.load(&engine).is_err());
    assert!(engine.fonts().get(&font.id).is_err());
}

#[test]
fn current_recovery_does_not_register_obsolete_source_fonts_at_capacity() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for id in engine.fonts().ids().unwrap() {
        engine.fonts().remove(&id).unwrap();
    }
    let original = engine.open_document(fixture(temp.path())).unwrap();
    let bytes = font_bytes();
    let old = engine.fonts().register(bytes.clone()).unwrap();
    let old_plan = workspace(&original, Some(&old.id))["tabs"][0]["document"]["pages"][0].clone();
    let source_path = temp.path().join("embedded-old.pdf");
    engine
        .export_pdf(
            serde_json::from_value(json!({"pages":[old_plan]})).unwrap(),
            &source_path,
        )
        .unwrap();
    let source = engine.open_document(&source_path).unwrap();
    engine.fonts().remove(&old.id).unwrap();
    let mut checkpoint = workspace(&source, None);
    let mut overlays = Vec::new();
    for index in 0..64u8 {
        let mut unique = bytes.clone();
        unique.extend_from_slice(&[0xf0, index]);
        let font = engine.fonts().register(unique).unwrap();
        let mut text = checkpoint["tabs"][0]["document"]["pages"][0]["overlays"][0].clone();
        text["id"] = json!(format!("font-{index}"));
        text["fontId"] = json!(font.id);
        overlays.push(text);
    }
    checkpoint["tabs"][0]["document"]["pages"][0]["overlays"] = json!(overlays);
    checkpoint["tabs"][0]["document"]["selectedOverlayId"] = Value::Null;
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    store.save(&engine, checkpoint).unwrap();
    drop(store);
    for id in engine.fonts().ids().unwrap() {
        engine.fonts().remove(&id).unwrap();
    }
    let store = RecoveryStore::new(root).unwrap();
    let restored = store.load(&engine).unwrap().unwrap();
    assert_eq!(engine.fonts().ids().unwrap().len(), 64);
    assert!(engine.fonts().get(&old.id).is_err());
    assert_eq!(
        restored["tabs"][0]["document"]["pages"][0]["overlays"]
            .as_array()
            .unwrap()
            .len(),
        64
    );
    for id in engine.fonts().ids().unwrap() {
        engine.fonts().remove(&id).unwrap();
    }
}

#[test]
fn substituted_source_recovers_without_original_font_file_or_registry_asset() {
    let _lock = TEST_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    let original_path = temp.path().join("subset.pdf");
    fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/embedded-text/reportlab-subset.pdf"),
        &original_path,
    )
    .unwrap();
    let original = engine.open_document(&original_path).unwrap();
    let imported_path = temp.path().join("font.ttf");
    fs::write(&imported_path, font_bytes()).unwrap();
    let font = engine
        .fonts()
        .register(fs::read(&imported_path).unwrap())
        .unwrap();
    let edited = engine
        .replace_text_with_font(&original.id, 0, 0, "Café old", "Cafè edit", Some(&font.id))
        .unwrap();
    engine.fonts().remove(&font.id).unwrap();
    fs::remove_file(&imported_path).unwrap();
    fs::remove_file(&original_path).unwrap();
    let mut plan = workspace(&edited, None);
    plan["tabs"][0]["document"]["pages"][0]["overlays"] = json!([]);
    plan["tabs"][0]["document"]["selectedOverlayId"] = Value::Null;
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    store.save(&engine, plan).unwrap();
    let manifest: Value = serde_json::from_slice(&fs::read(&manifests(&root)[0]).unwrap()).unwrap();
    assert!(
        manifest["fonts"].as_object().unwrap().is_empty(),
        "source fonts travel in source bytes, not overlay sidecars"
    );
    engine.close_document(&original.id).unwrap();
    engine.close_document(&edited.id).unwrap();
    drop(store);
    let store = RecoveryStore::new(root).unwrap();
    let recovered = store.load(&engine).unwrap().unwrap();
    let page = recovered["tabs"][0]["document"]["pages"][0].clone();
    let source_id = page["sourceId"].as_str().unwrap();
    assert!(engine
        .extract_text(source_id, 0)
        .unwrap()
        .contains("Cafè edit"));
    assert!(engine.fonts().get(&font.id).is_err());
    let changed = engine
        .replace_text(source_id, 0, 0, "Cafè edit", "Cafè safe")
        .unwrap();
    assert!(engine
        .extract_text(&changed.id, 0)
        .unwrap()
        .contains("Cafè safe"));
    let output = temp.path().join("recovered-substitution.pdf");
    engine
        .export_pdf(
            serde_json::from_value(json!({"pages":[page],"flatten":false})).unwrap(),
            &output,
        )
        .unwrap();
    let reopened = engine.open_document(&output).unwrap();
    assert!(engine
        .extract_text(&reopened.id, 0)
        .unwrap()
        .contains("Cafè edit"));
    for id in [source_id, changed.id.as_str(), reopened.id.as_str()] {
        engine.close_document(id).unwrap();
    }
    store.clear().unwrap();
}
