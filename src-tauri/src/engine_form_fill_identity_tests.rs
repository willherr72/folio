use super::*;
use crate::form_fill_identity::{
    tests::{fixture, request},
    FillIdentity,
};

#[test]
fn marked_native_fill_keeps_linked_field_correspondence() {
    // Share the engine tests' process-lifetime binding initializer. Do not bind
    // another DLL or create a competing PDFium initialization path.
    let library = std::env::var_os("FOLIO_TEST_PDFIUM_LIBRARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")
        });
    let runtime = WorkerRuntime::new(&library, Arc::new(FontRegistry::new())).unwrap();
    let f = fixture();
    let mut identity = FillIdentity::prepare(&f.bytes, &request()).unwrap();
    let probe = identity.probe_bytes().unwrap();
    let bytes = crate::form_fill::NativeForm::open(runtime.bindings, &probe)
        .unwrap()
        .fill(&request())
        .unwrap();
    let native = lopdf::Document::load_mem(&bytes).unwrap();
    let bound = identity.bind_native(&native).unwrap();
    assert_eq!(
        bound
            .nodes
            .iter()
            .filter(|v| v.selected && v.widget)
            .count(),
        2
    );
    assert!(bound.changes.iter().any(|v| v.original == f.owner
        && v.key == b"V"
        && v.value
            .as_ref()
            .is_some_and(|v| v.as_str().ok() == Some(b"new"))));
}
