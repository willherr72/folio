use super::*;
use lopdf::{dictionary, Document, Object, Stream};

#[path = "recovery_native_transfer_tests.rs"]
mod native_transactions;

fn fixture(path: &Path) {
    let mut pdf = Document::with_version("1.7");
    let tree = pdf.new_object_id();
    let page = pdf.new_object_id();
    let mut annots = vec![];
    for x in [20, 120] {
        annots.push(Object::Reference(pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Highlight",
            "Rect"=>vec![x.into(),280.into(),(x+60).into(),300.into()],
            "QuadPoints"=>vec![x.into(),300.into(),(x+60).into(),300.into(),x.into(),280.into(),(x+60).into(),280.into()],
            "C"=>vec![1.into(),1.into(),0.into()],"F"=>4,"NM"=>Object::string_literal("duplicate-nm"),
            "Contents"=>Object::string_literal(format!("Original {x}")),"P"=>page})));
    }
    let contents = pdf.add_object(Stream::new(dictionary! {}, vec![]));
    pdf.objects.insert(page,dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],
        "UserUnit"=>1.7,"Resources"=>dictionary!{},"Contents"=>contents,"Annots"=>annots}.into());
    pdf.objects.insert(
        tree,
        dictionary! {"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into(),
    );
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree});
    pdf.trailer.set("Root", root);
    fs::write(path, crate::pdf_output::save(&mut pdf).unwrap()).unwrap();
}
fn setup() -> (tempfile::TempDir, PdfEngine, RecoveryStore, String, Value) {
    let temp = tempfile::tempdir().unwrap();
    let original = temp.path().join("original.pdf");
    fixture(&original);
    let engine = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let source = engine.open_document(&original).unwrap();
    assert_eq!(source.pages[0].overlays.len(), 2);
    let mut page = json!({"id":"page","sourceId":source.id,"pageIndex":0,"width":source.pages[0].width,
        "height":source.pages[0].height,"rotation":0,"overlays":source.pages[0].overlays});
    let mut copy = page.clone();
    copy["id"] = json!("copy");
    page["overlays"].as_array_mut().unwrap().pop();
    page["overlays"][0]["text"] = json!("Saved draft changed this note");
    page["overlays"][0]["color"] = json!("#00ff00");
    let workspace = json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":true,"savedDigest":"",
        "zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"Recovered.pdf",
        "pages":[page,copy],"selectedPageId":"page","selectedOverlayId":null}}]});
    let store = RecoveryStore::new(temp.path().join("recovery")).unwrap();
    (temp, engine, store, source.id, workspace)
}
fn source_id(workspace: &Value) -> &str {
    workspace["tabs"][0]["document"]["pages"][0]["sourceId"]
        .as_str()
        .unwrap()
}
fn latest(store: &RecoveryStore) -> (PathBuf, Value) {
    let (_, path) = store.generations().unwrap().pop().unwrap();
    let value = serde_json::from_slice(
        &read_private(&path, Purpose::Workspace, MAX_MANIFEST_BYTES).unwrap(),
    )
    .unwrap();
    (path, value)
}

fn text_fixture(path: &Path) {
    fixture(path);
    let mut pdf = Document::load(path).unwrap();
    let page = pdf.get_pages()[&1];
    let font =
        pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let text = pdf.add_object(Stream::new(
        dictionary! {},
        b"BT /F1 12 Tf 30 200 Td (Contact team) Tj ET".to_vec(),
    ));
    let dict = pdf.get_dictionary_mut(page).unwrap();
    dict.set("Resources", dictionary! {"Font"=>dictionary! {"F1"=>font}});
    dict.set("Contents", text);
    fs::write(path, crate::pdf_output::save(&mut pdf).unwrap()).unwrap();
}

#[test]
fn recovery_annotation_sources_native_text_edit_retains_original_ownership() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("text-and-highlights.pdf");
    text_fixture(&path);
    let original_bytes = fs::read(&path).unwrap();
    let engine = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let original = engine.open_document(&path).unwrap();
    let ownership = serde_json::to_value(
        engine
            .source_imported_overlay_snapshot(&original.id)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(ownership["bindings"].as_array().unwrap().len(), 2);
    let run = engine
        .list_occurrence_text_runs(&original.id, 0)
        .unwrap()
        .runs
        .into_iter()
        .find(|r| r.text == "Contact team")
        .unwrap();
    let edited = engine
        .replace_text_occurrence(
            &original.id,
            &[0],
            0,
            run.object_index,
            &run.text,
            "Contact this team",
            None,
        )
        .unwrap()
        .document;
    let next =
        serde_json::to_value(engine.source_imported_overlay_snapshot(&edited.id).unwrap()).unwrap();
    assert_eq!(
        next["bindings"], ownership["bindings"],
        "native edits must transfer proven ownership, not fresh random highlight IDs"
    );
    assert_ne!(next["sourceSha256"], ownership["sourceSha256"]);
    assert_eq!(
        serde_json::to_value(&edited.pages[0].overlays).unwrap(),
        serde_json::to_value(&original.pages[0].overlays).unwrap()
    );
    let mut drafts = serde_json::to_value(&original.pages[0].overlays).unwrap();
    drafts.as_array_mut().unwrap().pop();
    drafts[0]["text"] = json!("Keep this unsaved note");
    let workspace = json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":true,"savedDigest":"",
        "zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"Edited.pdf",
        "pages":[{"id":"page","sourceId":edited.id,"pageIndex":0,"width":edited.pages[0].width,"height":edited.pages[0].height,"rotation":0,"overlays":drafts}],
        "selectedPageId":"page","selectedOverlayId":null}}]});
    let store = RecoveryStore::new(temp.path().join("recovery")).unwrap();
    store.save(&engine, workspace.clone()).unwrap();
    engine.close_document(&edited.id).unwrap();
    let recovered = store.load(&engine).unwrap().unwrap();
    assert_eq!(
        recovered["tabs"][0]["document"]["pages"][0]["overlays"],
        workspace["tabs"][0]["document"]["pages"][0]["overlays"]
    );
    assert_eq!(
        serde_json::to_value(
            engine
                .source_imported_overlay_snapshot(source_id(&recovered))
                .unwrap()
        )
        .unwrap(),
        next
    );
    assert!(engine
        .list_occurrence_text_runs(source_id(&recovered), 0)
        .unwrap()
        .runs
        .iter()
        .any(|r| r.text == "Contact this team"));
    assert_eq!(fs::read(path).unwrap(), original_bytes);
    engine.close_document(source_id(&recovered)).unwrap();
    engine.close_document(&original.id).unwrap();
}

#[test]
fn recovery_annotation_sources_native_text_edit_preserves_proved_copies_and_legacy_drafts() {
    for legacy in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("source.pdf");
        text_fixture(&path);
        let bytes = fs::read(&path).unwrap();
        let engine = PdfEngine::start(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
        )
        .unwrap();
        let original = engine.open_document(&path).unwrap();
        let store = RecoveryStore::new(temp.path().join("recovery")).unwrap();
        let mut workspace = json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":true,"savedDigest":"",
            "zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"Draft.pdf",
            "pages":[{"id":"page","sourceId":original.id,"pageIndex":0,"width":original.pages[0].width,"height":original.pages[0].height,"rotation":0,"overlays":original.pages[0].overlays}],
            "selectedPageId":"page","selectedOverlayId":null}}]});
        if legacy {
            store.save(&engine, workspace.clone()).unwrap();
            let (manifest_path, mut manifest) = latest(&store);
            manifest["annotationImportVersion"] = json!(3);
            manifest["sources"][&original.id]
                .as_object_mut()
                .unwrap()
                .remove("annotationSources");
            private_write(
                &manifest_path,
                Purpose::Workspace,
                &serde_json::to_vec(&manifest).unwrap(),
            )
            .unwrap();
            engine.close_document(&original.id).unwrap();
            workspace = store.load(&engine).unwrap().unwrap();
        }
        let input = source_id(&workspace).to_owned();
        let input_ownership = engine.source_imported_overlay_snapshot(&input).unwrap();
        let run = engine
            .list_occurrence_text_runs(&input, 0)
            .unwrap()
            .runs
            .into_iter()
            .find(|r| r.text == "Contact team")
            .unwrap();
        let changed = engine
            .replace_text_occurrence(
                &input,
                if legacy { &[0] } else { &[0, 0] },
                if legacy { 0 } else { 1 },
                run.object_index,
                &run.text,
                "Contact this team",
                None,
            )
            .unwrap();
        let changed_ownership = engine.source_imported_overlay_snapshot(&changed.document.id).unwrap();
        assert_eq!(changed_ownership.is_none(), legacy,
            "proved copies retain ownership; legacy drafts never gain invented authority");
        assert_eq!(
            engine.source_imported_overlay_snapshot(&input).unwrap(),
            input_ownership
        );
        let selected = *changed.page_indices.last().unwrap();
        let old_drafts = workspace["tabs"][0]["document"]["pages"][0]["overlays"].clone();
        workspace["tabs"][0]["document"]["pages"][0]["sourceId"] = json!(changed.document.id);
        workspace["tabs"][0]["document"]["pages"][0]["pageIndex"] = json!(selected);
        store.save(&engine, workspace).unwrap();
        engine.close_document(&changed.document.id).unwrap();
        let recovered = store.load(&engine).unwrap().unwrap();
        assert_eq!(engine.source_imported_overlay_snapshot(source_id(&recovered)).unwrap(), changed_ownership);
        assert_eq!(
            recovered["tabs"][0]["document"]["pages"][0]["overlays"],
            old_drafts
        );
        assert!(engine
            .list_occurrence_text_runs(source_id(&recovered), selected)
            .unwrap()
            .runs
            .iter()
            .any(|r| r.text == "Contact this team"));
        assert_eq!(fs::read(path).unwrap(), bytes);
        engine.close_document(&input).unwrap();
        engine.close_document(source_id(&recovered)).unwrap();
    }
}
fn assert_drafts(expected: &Value, actual: &Value) {
    let expected = &expected["tabs"][0]["document"]["pages"];
    let actual = &actual["tabs"][0]["document"]["pages"];
    for i in 0..2 {
        assert_eq!(actual[i]["overlays"], expected[i]["overlays"]);
        assert_eq!(actual[i]["width"], expected[i]["width"]);
        assert_eq!(actual[i]["height"], expected[i]["height"]);
    }
}

#[test]
fn recovery_annotation_sources_restore_ids_changed_deleted_and_copied_drafts() {
    let (temp, engine, store, id, workspace) = setup();
    let original = engine.source_imported_overlay_snapshot(&id).unwrap();
    assert!(
        original.is_some(),
        "fresh imported sources need source-bound ownership"
    );
    store.save(&engine, workspace.clone()).unwrap();
    let (_, manifest) = latest(&store);
    assert_eq!(manifest["annotationImportVersion"], 4);
    assert_eq!(
        manifest["sources"][&id]["annotationSources"],
        serde_json::to_value(&original).unwrap()
    );
    assert_eq!(
        manifest["sources"][&id]["annotationSources"]["bindings"]
            .as_array()
            .unwrap()
            .len(),
        2,
        "deleted draft still needs original ownership"
    );
    let bytes = engine.source_bytes(&id).unwrap();
    engine.close_document(&id).unwrap();
    fs::remove_file(temp.path().join("original.pdf")).unwrap();
    let restored = store.load(&engine).unwrap().unwrap();
    assert_drafts(&workspace, &restored);
    assert_ne!(source_id(&restored), id);
    assert_eq!(
        engine
            .source_imported_overlay_snapshot(source_id(&restored))
            .unwrap(),
        original
    );
    assert_eq!(
        engine.source_bytes(source_id(&restored)).unwrap().as_ref(),
        bytes.as_ref()
    );
    store.save(&engine, restored.clone()).unwrap();
    engine.close_document(source_id(&restored)).unwrap();
    let again = store.load(&engine).unwrap().unwrap();
    assert_drafts(&workspace, &again);
    assert_eq!(
        engine
            .source_imported_overlay_snapshot(source_id(&again))
            .unwrap(),
        original
    );
    engine.close_document(source_id(&again)).unwrap();
}

#[test]
fn recovery_annotation_sources_legacy_keeps_drafts_without_inventing_ownership() {
    let (_temp, engine, store, id, workspace) = setup();
    store.save(&engine, workspace.clone()).unwrap();
    let (path, mut manifest) = latest(&store);
    manifest["annotationImportVersion"] = json!(3);
    manifest["sources"][&id]
        .as_object_mut()
        .unwrap()
        .remove("annotationSources");
    private_write(
        &path,
        Purpose::Workspace,
        &serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    engine.close_document(&id).unwrap();
    let restored = store.load(&engine).unwrap().unwrap();
    assert_drafts(&workspace, &restored);
    assert!(engine
        .source_imported_overlay_snapshot(source_id(&restored))
        .unwrap()
        .is_none());
    store.save(&engine, restored.clone()).unwrap();
    let (_, manifest) = latest(&store);
    assert!(manifest["sources"][source_id(&restored)]["annotationSources"].is_null());
    engine.close_document(source_id(&restored)).unwrap();
}

#[test]
fn recovery_annotation_sources_tampering_is_rejected_without_discarding_checkpoint() {
    for mutation in [
        "source",
        "missing-binding",
        "slot",
        "missing-record",
        "future",
        "future-ownership",
    ] {
        let (_temp, engine, store, id, workspace) = setup();
        store.save(&engine, workspace).unwrap();
        let (path, mut manifest) = latest(&store);
        assert!(
            manifest["sources"][&id]["annotationSources"].is_object(),
            "ownership record must be persisted"
        );
        match mutation {
            "source" => {
                manifest["sources"][&id]["annotationSources"]["sourceSha256"] =
                    json!("0".repeat(64))
            }
            "missing-binding" => {
                manifest["sources"][&id]["annotationSources"]["bindings"]
                    .as_array_mut()
                    .unwrap()
                    .pop();
            }
            "slot" => {
                manifest["sources"][&id]["annotationSources"]["bindings"][0]["annotationIndex"] =
                    json!(99)
            }
            "missing-record" => {
                manifest["sources"][&id]
                    .as_object_mut()
                    .unwrap()
                    .remove("annotationSources");
            }
            "future" => manifest["annotationImportVersion"] = json!(999),
            "future-ownership" => {
                manifest["sources"][&id]["annotationSources"]["version"] = json!(2)
            }
            _ => unreachable!(),
        }
        private_write(
            &path,
            Purpose::Workspace,
            &serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let before = fs::read(&path).unwrap();
        engine.close_document(&id).unwrap();
        let error = store.load(&engine).unwrap_err();
        assert!(
            error.contains("ownership") || error.contains("newer version"),
            "{mutation}: {error}"
        );
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}

#[test]
fn recovery_annotation_sources_cache_does_not_retain_closed_runtime_duplicates() {
    let (_temp, engine, store, id, workspace) = setup();
    store.save(&engine, workspace).unwrap();
    engine.close_document(&id).unwrap();
    for _ in 0..4 {
        let restored = store.load(&engine).unwrap().unwrap();
        assert_eq!(
            store.state.lock().unwrap().len(),
            1,
            "only the current restored source needs a cache entry"
        );
        engine.close_document(source_id(&restored)).unwrap();
    }
}
#[test]
fn recovery_annotation_sources_snapshot_reserves_before_cloning() {
    let (_temp, engine, _store, id, _workspace) = setup();
    assert!(engine
        .source_imported_overlay_snapshot_with_budget(&id, 1)
        .is_err());
    assert!(engine
        .source_imported_overlay_snapshot(&id)
        .unwrap()
        .is_some());
    engine.close_document(&id).unwrap();
}

#[test]
fn recovery_annotation_sources_rejects_large_metadata_before_typed_allocation() {
    let (_temp, engine, store, id, workspace) = setup();
    store.save(&engine, workspace).unwrap();
    let (path, mut manifest) = latest(&store);
    let mut popups = vec![json!(0); 140_000];
    // The budget must win before serde allocates/visits the typed popup vector.
    *popups.last_mut().unwrap() = json!("not-a-slot");
    manifest["sources"][&id]["annotationSources"]["bindings"][0]["popupIndices"] =
        Value::Array(popups);
    private_write(
        &path,
        Purpose::Workspace,
        &serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let before = fs::read(&path).unwrap();
    engine.close_document(&id).unwrap();
    let error = store.load(&engine).unwrap_err();
    assert!(
        error.contains("ownership") && error.contains("budget"),
        "{error}"
    );
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn recovery_annotation_sources_empty_binding_set_stays_explicit_after_restore() {
    let (temp, engine, store, old_id, mut workspace) = setup();
    let bytes = engine.source_bytes(&old_id).unwrap();
    let mut pdf = Document::load_mem(&bytes).unwrap();
    let page = *pdf.get_pages().values().next().unwrap();
    pdf.get_dictionary_mut(page).unwrap().remove(b"Annots");
    let path = temp.path().join("without-annotations.pdf");
    fs::write(&path, crate::pdf_output::save(&mut pdf).unwrap()).unwrap();
    engine.close_document(&old_id).unwrap();
    let source = engine.open_document(&path).unwrap();
    for page in workspace["tabs"][0]["document"]["pages"]
        .as_array_mut()
        .unwrap()
    {
        page["sourceId"] = json!(source.id);
        page["overlays"] = json!([]);
    }
    let binding = engine.source_imported_overlay_snapshot(&source.id).unwrap();
    assert!(binding.is_some());
    assert_eq!(
        serde_json::to_value(&binding).unwrap()["bindings"],
        json!([])
    );
    store.save(&engine, workspace).unwrap();
    engine.close_document(&source.id).unwrap();
    let restored = store.load(&engine).unwrap().unwrap();
    assert_eq!(
        engine
            .source_imported_overlay_snapshot(source_id(&restored))
            .unwrap(),
        binding
    );
    engine.close_document(source_id(&restored)).unwrap();
}
#[test]
fn recovery_annotation_sources_failed_checkpoint_does_not_publish_new_cache_entries() {
    let (_temp, engine, store, id, workspace) = setup();
    store.save(&engine, workspace.clone()).unwrap();
    let (path, _) = latest(&store);
    let before = fs::read(&path).unwrap();
    let extra = engine.create_blank_document(200., 200.).unwrap();
    let mut invalid = workspace;
    let page = &mut invalid["tabs"][0]["document"]["pages"][1];
    page["sourceId"] = json!(extra.id);
    page["width"] = json!(200);
    page["height"] = json!(200);
    page["overlays"] = json!([{"type":"text","id":"missing-font","text":"Draft","x":10,"y":20,
        "fontSize":12,"fontId":"f".repeat(64),"color":"#000000"}]);
    assert!(store.save(&engine, invalid).is_err());
    assert_eq!(
        store
            .state
            .lock()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec![id.clone()]
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    engine.close_document(&id).unwrap();
    engine.close_document(&extra.id).unwrap();
}
