use super::*;
use lopdf::{dictionary, Document, Object, Stream};

fn runtime() -> WorkerRuntime {
    WorkerRuntime::new(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"), Arc::new(FontRegistry::new())).unwrap()
}

fn fixture(annotations: bool, tagged: bool, cyclic: bool) -> Vec<u8> {
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let form = pdf.new_object_id();
    let resources = if cyclic { dictionary!{"XObject"=>dictionary!{"Self"=>form}} } else { dictionary!{} };
    pdf.objects.insert(form, Stream::new(dictionary!{"Type"=>"XObject", "Subtype"=>"Form", "BBox"=>vec![0.into(),0.into(),20.into(),20.into()], "Resources"=>resources}, if cyclic { b"/Self Do".to_vec() } else { b"0 0 10 10 re f".to_vec() }).into());
    // An unused cyclic form must not be traversed by PDFium while opening.
    // Form-graph preflight checks invoked graphs, so the cycle is used only in
    // the pure validator test below, without opening in the native renderer.
    let content = pdf.add_object(Stream::new(dictionary!{}, b"/F Do".to_vec()));
    let mut kids = Vec::new();
    for _ in 0..2 {
        let mut page = dictionary!{"Type"=>"Page", "Parent"=>pages, "MediaBox"=>vec![0.into(),0.into(),200.into(),200.into()], "Resources"=>dictionary!{"XObject"=>dictionary!{"F"=>form}}, "Contents"=>content};
        if annotations { page.set("Annots", Object::Array(vec![])); }
        kids.push(Object::Reference(pdf.add_object(page)));
    }
    pdf.objects.insert(pages, dictionary!{"Type"=>"Pages", "Count"=>2, "Kids"=>kids}.into());
    let mut catalog = dictionary!{"Type"=>"Catalog", "Pages"=>pages};
    if tagged { catalog.set("MarkInfo", dictionary!{"Marked"=>true}); }
    let root = pdf.add_object(catalog); pdf.trailer.set("Root", root);
    crate::pdf_output::save(&mut pdf).unwrap()
}

fn is_cached(rt: &WorkerRuntime, id: &str, mode: bool) -> bool {
    // A failing callback never populates an empty slot. This observes whether
    // real worker admission succeeded, without a production test-only API.
    rt.document(id).unwrap().form_admission.check(mode, || Err(())).is_ok()
}

#[test]
fn form_admission_modes_and_refusals_remain_separate() {
    let mut rt = runtime();
    let source = rt.register_generated_document(fixture(true,false,false), "Annotated.pdf").unwrap();
    assert!(!is_cached(&rt, &source.id, true));
    rt.form_source_admission(&source.id, true).unwrap();
    assert!(is_cached(&rt, &source.id, true));
    for _ in 0..2 {
        let error = rt.form_source_admission(&source.id, false).unwrap_err();
        assert!(error.to_string().contains("with annotations"), "{error}");
        assert!(!is_cached(&rt, &source.id, false));
        rt.form_source_admission(&source.id, true).unwrap();
    }
}

#[test]
fn form_admission_rechecks_print_and_protected_policy_after_success() {
    let mut rt = runtime();
    let source = rt.register_generated_document(fixture(false,false,false), "Policy.pdf").unwrap();
    rt.form_source_admission(&source.id, true).unwrap();
    rt.documents.get_mut(&source.id).unwrap().for_printing = true;
    assert!(matches!(rt.form_source_admission(&source.id,true), Err(EngineError::PermissionDenied(_))));
    rt.documents.get_mut(&source.id).unwrap().for_printing = false;
    let bytes = rt.document(&source.id).unwrap().bytes().unwrap();
    let change: crate::protection::ProtectionChange = serde_json::from_value(serde_json::json!({"mode":"passwords","userPassword":"test-reader","ownerPassword":"test-owner","permissions":{"print":true,"printHighQuality":true,"copyContent":true,"editContent":false,"annotate":false,"fillForms":false,"assemble":true}})).unwrap();
    let encrypted = change.apply(&bytes).unwrap();
    let unlocked = crate::protection::unlock_pdf(&encrypted, Some("test-reader")).unwrap();
    rt.documents.get_mut(&source.id).unwrap().policy = Some(SourcePolicy::new(unlocked.protection.unwrap(), &source.pages, &Ok(vec![])));
    assert!(matches!(rt.form_source_admission(&source.id,true), Err(EngineError::PermissionDenied(_))));
    rt.documents.get_mut(&source.id).unwrap().policy = None;
    rt.form_source_admission(&source.id,true).unwrap();
}

#[test]
fn form_admission_opened_and_generated_sources_never_inherit_success() {
    let mut rt = runtime();
    let bytes = fixture(false,false,false);
    let source = rt.register_generated_document(bytes.clone(), "Original.pdf").unwrap();
    rt.form_source_admission(&source.id,true).unwrap();
    let path = std::env::temp_dir().join("folio-admission-no-disk-input.pdf");
    let reopened = rt.open_bytes(bytes, path.clone(), path, true,false,false, OpenSecurity::Authenticated(None)).unwrap();
    assert!(!is_cached(&rt, &reopened.id, true));
    rt.form_source_admission(&reopened.id,true).unwrap();
    let changed = rt.register_generated_document(fixture(false,true,false), "Changed.pdf").unwrap();
    for _ in 0..2 {
        let error = rt.form_source_admission(&changed.id,true).unwrap_err();
        assert!(error.to_string().contains("Tagged"), "{error}");
        assert!(!is_cached(&rt,&changed.id,true));
    }
    rt.discard_unpublished_source(&source.id);
    assert!(rt.form_source_admission(&source.id,true).is_err());
    rt.form_source_admission(&reopened.id,true).unwrap();
}

#[test]
fn form_admission_does_not_bypass_page_checks_on_occurrence_discovery() {
    let mut rt = runtime();
    let source = rt.register_generated_document(fixture(false,false,false), "Pages.pdf").unwrap();
    for page in [0,1] {
        let (reply, receive) = mpsc::channel();
        rt.handle_occurrence_form(occurrence_form::Request::List { source: source.id.clone(), page, reply });
        let result = receive.recv().unwrap().unwrap();
        assert!(result.runs.is_empty());
        assert!(is_cached(&rt,&source.id,true));
    }
    let (reply, receive) = mpsc::channel();
    rt.handle_occurrence_form(occurrence_form::Request::List { source: source.id.clone(), page: 99, reply });
    let result = receive.recv().unwrap();
    assert!(result.is_err() || result.unwrap().reason.is_some());
    assert_eq!(rt.documents.len(),1,"discovery must not leak a projection");
}

#[test]
fn form_admission_graph_refusals_are_not_cached() {
    let cache = form_admission::Admission::default();
    let bytes = fixture(false,false,true);
    for _ in 0..2 {
        let error = cache.check(true, || crate::form_graph::preflight(&bytes)).unwrap_err();
        assert!(error.to_string().contains("cyclic form"), "{error}");
        assert_eq!(cache.check(true, || Err("uncached")), Err("uncached"));
    }
}
