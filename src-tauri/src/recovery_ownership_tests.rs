use super::*;
use crate::fonts::FontRegistry;
use std::sync::Arc;
use sha2::{Digest,Sha256};

static SERIAL: Mutex<()> = Mutex::new(());

fn font_bytes() -> Vec<u8> {
    let mut bytes = fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap();
    // Distinct valid programs avoid claiming fonts used by other library tests.
    bytes.extend_from_slice(Uuid::new_v4().as_bytes());
    bytes
}

fn checkpoint(font: Option<&str>) -> (tempfile::TempDir, PdfEngine, RecoveryStore) {
    let temp = tempfile::tempdir().unwrap();
    let engine = PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources/pdfium/pdfium.dll")).unwrap();
    let source = engine.create_blank_document(200.0, 200.0).unwrap();
    let overlays = font.map(|id| json!([{"type":"text","id":"text","text":"Draft",
        "x":20,"y":40,"fontSize":12,"fontId":id,"color":"#000000","rotation":0}]))
        .unwrap_or_else(||json!([]));
    let workspace = json!({"version":1,"activeId":"tab","tabs":[{"id":"tab",
        "dirty":true,"savedDigest":"","zoom":100,"scrollPosition":{"top":0,"left":0},
        "document":{"name":"Draft.pdf","pages":[{"id":"page","sourceId":source.id,
            "pageIndex":0,"width":200,"height":200,"rotation":0,"overlays":overlays}],
            "selectedPageId":"page","selectedOverlayId":null}}]});
    let store = RecoveryStore::new(temp.path().join("recovery")).unwrap();
    store.save(&engine,workspace).unwrap();
    engine.close_document(&source.id).unwrap();
    (temp,engine,store)
}

fn invalidate_dimensions(store: &RecoveryStore) {
    let (_,path) = store.generations().unwrap().pop().unwrap();
    let mut manifest = store.read_manifest(&path).unwrap();
    manifest.workspace["tabs"][0]["document"]["pages"][0]["width"] = json!(201);
    private_write(&path,Purpose::Workspace,&serde_json::to_vec(&manifest).unwrap()).unwrap();
}

fn observe_registration(registry: Arc<FontRegistry>, bytes: Vec<u8>) -> String {
    let id = Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect();
    RESTORE_OBSERVER.with(|slot| {
        assert!(slot.borrow().is_none());
        *slot.borrow_mut() = Some(Box::new(move || {
            registry.register(bytes).unwrap();
        }));
    });
    id
}

fn close_workspace(engine: &PdfEngine, workspace: &Value) {
    for id in validate_workspace(workspace).unwrap() { engine.close_document(&id).unwrap(); }
}

#[test]
fn unrelated_registration_survives_successful_recovery() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (_temp,engine,store) = checkpoint(None);
    let id = observe_registration(engine.fonts(),font_bytes());
    let restored = store.load(&engine).unwrap().unwrap();
    close_workspace(&engine,&restored);
    let present = engine.fonts().get(&id).is_ok();
    engine.fonts().remove(&id).unwrap();
    assert!(present,"Recovery removed an unrelated registration made during restore");
}

#[test]
fn unrelated_registration_survives_failed_recovery() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (_temp,engine,store) = checkpoint(None);
    invalidate_dimensions(&store);
    let id = observe_registration(engine.fonts(),font_bytes());
    let error = store.load(&engine).unwrap_err();
    assert!(error.contains("dimensions"),"{error}");
    let present = engine.fonts().get(&id).is_ok();
    engine.fonts().remove(&id).unwrap();
    assert!(present,"Failed recovery removed another operation's registration");
}

#[test]
fn external_adoption_of_recovery_font_survives_failure() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let engine = PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources/pdfium/pdfium.dll")).unwrap();
    let bytes = font_bytes();
    let font = engine.fonts().register(bytes.clone()).unwrap();
    let (_temp,engine,store) = checkpoint(Some(&font.id));
    engine.fonts().remove(&font.id).unwrap();
    invalidate_dimensions(&store);
    observe_registration(engine.fonts(),bytes);
    let error = store.load(&engine).unwrap_err();
    assert!(error.contains("dimensions"),"{error}");
    let present = engine.fonts().get(&font.id).is_ok();
    engine.fonts().remove(&font.id).unwrap();
    assert!(present,"Rollback ignored an external claim of the recovered font");
}

#[test]
fn unrelated_registration_survives_legacy_migration() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (temp,engine,store) = checkpoint(None);
    drop(store);
    let id = observe_registration(engine.fonts(),font_bytes());
    let current = RecoveryStore::for_application(temp.path().join("recovery"),&engine).unwrap();
    let restored = current.load(&engine).unwrap().unwrap();
    close_workspace(&engine,&restored);
    let present = engine.fonts().get(&id).is_ok();
    engine.fonts().remove(&id).unwrap();
    assert!(present,"Legacy migration removed another operation's font");
}

#[test]
fn preexisting_registration_survives_recovery_failure() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (_temp,engine,store) = checkpoint(None);
    let font = engine.fonts().register(font_bytes()).unwrap();
    invalidate_dimensions(&store);
    assert!(store.load(&engine).unwrap_err().contains("dimensions"));
    let present = engine.fonts().get(&font.id).is_ok();
    engine.fonts().remove(&font.id).unwrap();
    assert!(present,"An existing font must survive recovery failure");
}
