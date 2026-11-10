use folio_engine::{ExportRequest, PagePlan, PdfEngine};
use lopdf::{dictionary, Document, Object, Stream};
use std::{fs, path::PathBuf};

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn fixture() -> Vec<u8> {
    let mut d = Document::with_version("1.7");
    let pages = d.new_object_id();
    let page = d.new_object_id();
    let note = d.new_object_id();
    let ap=d.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),18.into(),18.into()]},b"0 0 1 rg 0 0 18 18 re f".to_vec()));
    let popup=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Popup","Parent"=>note,"Rect"=>vec![120.into(),200.into(),220.into(),300.into()],"Open"=>false});
    d.objects.insert(note,Object::Dictionary(dictionary!{"Type"=>"Annot","Subtype"=>"Text","P"=>page,"Rect"=>vec![70.into(),230.into(),88.into(),248.into()],"F"=>4,"Contents"=>Object::string_literal("External note"),"T"=>Object::string_literal("Original reviewer"),"Subj"=>Object::string_literal("Original subject"),"Name"=>"Help","NM"=>Object::string_literal("external-identity"),"M"=>Object::string_literal("D:20261023090000"),"CreationDate"=>Object::string_literal("D:20261022090000"),"AP"=>dictionary!{"N"=>ap},"Popup"=>popup,"C"=>vec![0.into(),0.into(),1.into()]}));
    d.objects.insert(page,Object::Dictionary(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary!{},"Annots"=>vec![Object::Reference(note),Object::Reference(popup)]}));
    d.objects.insert(
        pages,
        Object::Dictionary(
            dictionary! {"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1},
        ),
    );
    let root = d.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    d.trailer.set("Root", root);
    let mut bytes = vec![];
    d.save_to(&mut bytes).unwrap();
    bytes
}
fn plan(info: &folio_engine::DocumentInfo) -> PagePlan {
    serde_json::from_value(serde_json::json!({"id":"page","sourceId":info.id,"pageIndex":0,"width":info.pages[0].width,"height":info.pages[0].height,"rotation":0,"overlays":info.pages[0].overlays})).unwrap()
}
fn note(doc: &Document) -> &lopdf::Dictionary {
    let page = *doc.get_pages().values().next().unwrap();
    let values = doc.get_dictionary(page).unwrap().get(b"Annots").unwrap();
    doc.dereference(values)
        .unwrap()
        .1
        .as_array()
        .unwrap()
        .iter()
        .map(|v| doc.dereference(v).unwrap().1.as_dict().unwrap())
        .find(|d| d.get(b"Subtype").unwrap().as_name().unwrap() == b"Text")
        .unwrap()
}
#[test]
fn saving_an_untouched_standard_comment_preserves_author_icon_popup_and_appearance() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.pdf");
    let original = fixture();
    fs::write(&path, &original).unwrap();
    let source = e.open_document(&path).unwrap();
    let output = dir.path().join("copy.pdf");
    e.export_pdf(
        ExportRequest {
            pages: vec![plan(&source)],
            flatten: false,
        },
        &output,
    )
    .unwrap();
    let a = Document::load_mem(&original).unwrap();
    let b = Document::load(&output).unwrap();
    let before = note(&a);
    let after = note(&b);
    println!("Source comment: {before:?}\nSaved comment: {after:?}");
    for key in [
        b"T".as_slice(),
        b"Subj",
        b"Name",
        b"NM",
        b"M",
        b"CreationDate",
        b"Rect",
        b"F",
        b"C",
    ] {
        assert_eq!(
            before.get(key).ok(),
            after.get(key).ok(),
            "metadata field {:?}",
            String::from_utf8_lossy(key)
        );
    }
    let ap = |d: &Document, n: &lopdf::Dictionary| {
        let appearance = d
            .dereference(n.get(b"AP").unwrap())
            .unwrap()
            .1
            .as_dict()
            .unwrap();
        {
            let stream = d
                .dereference(appearance.get(b"N").unwrap())
                .unwrap()
                .1
                .as_stream()
                .unwrap();
            stream
                .decompressed_content()
                .unwrap_or_else(|_| stream.content.clone())
        }
    };
    assert_eq!(ap(&a, before), ap(&b, after));
    assert!(after.has(b"Popup"));
    assert!(source.pages[0].overlays.is_empty());
    assert_eq!(
        e.list_native_comments(&source.id).unwrap()[0].text,
        "External note"
    );
    assert_eq!(fs::read(path).unwrap(), original);
    e.close_document(&source.id).unwrap();
}
#[test]
fn saved_folio_comments_reopen_natively_and_remain_editable() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let source = e.create_blank_document(300.0, 400.0).unwrap();
    let mut page = plan(&source);
    page.overlays=serde_json::from_value(serde_json::json!([{"type":"comment","id":"owned-note","x":30,"y":40,"text":"Folio-created note","color":"#ffcc00"}])).unwrap();
    let output = dir.path().join("created.pdf");
    e.export_pdf(
        ExportRequest {
            pages: vec![page],
            flatten: false,
        },
        &output,
    )
    .unwrap();
    let reopened = e.open_document(&output).unwrap();
    assert!(reopened.pages[0].overlays.is_empty());
    let notes = e.list_native_comments(&reopened.id).unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].text, "Folio-created note");
    assert!(notes[0].reason.is_none());
    e.close_document(&reopened.id).unwrap();
    e.close_document(&source.id).unwrap();
}

fn workspace(info: &folio_engine::DocumentInfo) -> serde_json::Value {
    serde_json::json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","savedDigest":"","dirty":true,"zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"comments.pdf","pages":[plan(info)],"selectedPageId":"page","selectedOverlayId":null}}]})
}
fn checkpoint(root: &std::path::Path) -> (PathBuf, serde_json::Value) {
    let path = fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .max()
        .unwrap();
    let value = serde_json::from_slice(
        &folio_engine::private_storage::unseal(
            folio_engine::private_storage::Purpose::Workspace,
            &fs::read(&path).unwrap(),
            32 * 1024 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    (path, value)
}
#[test]
fn new_recovery_records_native_comment_ownership_per_source() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.pdf");
    fs::write(&path, fixture()).unwrap();
    let source = e.open_document(path).unwrap();
    let root = dir.path().join("recovery");
    let store = folio_engine::RecoveryStore::new(root.clone()).unwrap();
    store.save(&e, workspace(&source)).unwrap();
    let (_, manifest) = checkpoint(&root);
    assert_eq!(manifest["annotationImportVersion"], 4);
    assert_eq!(
        manifest["sources"][&source.id]["legacyCommentImport"],
        false
    );
    let restored = store.load(&e).unwrap().unwrap();
    let id = restored["tabs"][0]["document"]["pages"][0]["sourceId"]
        .as_str()
        .unwrap();
    assert!(restored["tabs"][0]["document"]["pages"][0]["overlays"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(e.list_native_comments(id).unwrap().len(), 1);
}
#[test]
fn legacy_deleted_comments_stay_deleted_across_mixed_recovery_checkpoints() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.pdf");
    fs::write(&path, fixture()).unwrap();
    let source = e.open_document(&path).unwrap();
    let root = dir.path().join("recovery");
    let store = folio_engine::RecoveryStore::new(root.clone()).unwrap();
    let mut old = workspace(&source);
    old["tabs"][0]["document"]["pages"][0]["overlays"] = serde_json::json!([]);
    store.save(&e, old).unwrap();
    let (manifest_path, mut manifest) = checkpoint(&root);
    manifest["annotationImportVersion"] = 2.into();
    for snapshot in manifest["sources"].as_object_mut().unwrap().values_mut() {
        snapshot
            .as_object_mut()
            .unwrap()
            .remove("legacyCommentImport");
    }
    fs::write(manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let mut restored = store.load(&e).unwrap().unwrap();
    let legacy_id = restored["tabs"][0]["document"]["pages"][0]["sourceId"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(e.list_native_comments(&legacy_id).unwrap().is_empty());
    let fresh = e.open_document(path).unwrap();
    let mut fresh_page = serde_json::to_value(plan(&fresh)).unwrap();
    fresh_page["id"] = "fresh-page".into();
    restored["tabs"][0]["document"]["pages"]
        .as_array_mut()
        .unwrap()
        .push(fresh_page);
    store.save(&e, restored).unwrap();
    let (_, manifest) = checkpoint(&root);
    assert_eq!(manifest["annotationImportVersion"], 4);
    assert_eq!(manifest["sources"][&legacy_id]["legacyCommentImport"], true);
    assert_eq!(manifest["sources"][&fresh.id]["legacyCommentImport"], false);
    assert!(manifest["sources"][&legacy_id]["annotationSources"].is_null());
    assert_eq!(manifest["sources"][&fresh.id]["annotationSources"]["bindings"], serde_json::json!([]));
    let again = store.load(&e).unwrap().unwrap();
    let pages = again["tabs"][0]["document"]["pages"].as_array().unwrap();
    assert!(e
        .list_native_comments(pages[0]["sourceId"].as_str().unwrap())
        .unwrap()
        .is_empty());
    assert_eq!(
        e.list_native_comments(pages[1]["sourceId"].as_str().unwrap())
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn moving_native_comments_preserves_custom_appearance_and_crop_rotation() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    for rotation in [0, 90, 180, 270] {
        let mut d = Document::load_mem(&fixture()).unwrap();
        let page = *d.get_pages().values().next().unwrap();
        d.get_dictionary_mut(page).unwrap().set(
            "CropBox",
            vec![40.into(), 50.into(), 240.into(), 350.into()],
        );
        d.get_dictionary_mut(page).unwrap().set("Rotate", rotation);
        let mut original = vec![];
        d.save_to(&mut original).unwrap();
        let path = dir.path().join(format!("move-{rotation}.pdf"));
        fs::write(&path, &original).unwrap();
        let source = e.open_document(&path).unwrap();
        let request=serde_json::from_value(serde_json::json!({"pageIndex":0,"annotationIndex":0,"expectedText":"External note","text":"Moved note","position":{"x":60,"y":70}})).unwrap();
        let moved = e.edit_native_comment(&source.id, request).unwrap();
        let notes = e.list_native_comments(&moved.id).unwrap();
        assert_eq!((notes[0].bounds.x, notes[0].bounds.y), (60., 70.));
        assert_eq!((notes[0].bounds.width, notes[0].bounds.height), (18., 18.));
        assert_eq!(notes[0].text, "Moved note");
        let changed = Document::load_mem(&e.source_bytes(&moved.id).unwrap()).unwrap();
        let mut a = note(&d).clone();
        let mut b = note(&changed).clone();
        for key in [b"Rect".as_slice(), b"Contents"] {
            a.remove(key);
            b.remove(key);
        }
        assert_eq!(a, b, "all unrelated comment metadata remains identical");
        assert_eq!(fs::read(path).unwrap(), original);
        for invalid in [
            serde_json::json!({"x":-1,"y":20}),
            serde_json::json!({"x":1999,"y":20}),
        ] {
            let request=serde_json::from_value(serde_json::json!({"pageIndex":0,"annotationIndex":0,"expectedText":"External note","text":"External note","position":invalid})).unwrap();
            assert!(e.edit_native_comment(&source.id, request).is_err());
        }
    }
}
#[test]
fn native_color_changes_require_an_unstyled_icon_and_keep_other_metadata() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    for appearance in [false, true] {
        let mut d = Document::load_mem(&fixture()).unwrap();
        let id = d
            .objects
            .iter()
            .find(|(_, o)| {
                o.as_dict().is_ok_and(|n| {
                    n.get(b"Subtype")
                        .and_then(Object::as_name)
                        .is_ok_and(|s| s == b"Text")
                })
            })
            .unwrap()
            .0
            .to_owned();
        if !appearance {
            d.get_dictionary_mut(id).unwrap().remove(b"AP");
        }
        let mut bytes = vec![];
        d.save_to(&mut bytes).unwrap();
        let path = dir.path().join(format!("color-{appearance}.pdf"));
        fs::write(&path, &bytes).unwrap();
        let source = e.open_document(&path).unwrap();
        let value = serde_json::to_value(e.list_native_comments(&source.id).unwrap()).unwrap();
        assert_eq!(value[0]["color"], "#0000ff");
        assert_eq!(value[0]["canChangeColor"], !appearance);
        let request=serde_json::from_value(serde_json::json!({"pageIndex":0,"annotationIndex":0,"expectedText":"External note","text":"External note","color":"#ff8000"})).unwrap();
        let result = e.edit_native_comment(&source.id, request);
        if appearance {
            assert!(result.is_err());
        } else {
            let changed = result.unwrap();
            let value = serde_json::to_value(e.list_native_comments(&changed.id).unwrap()).unwrap();
            assert_eq!(value[0]["color"], "#ff8000");
            let output = Document::load_mem(&e.source_bytes(&changed.id).unwrap()).unwrap();
            assert_eq!(
                note(&output).get(b"T").unwrap(),
                note(&d).get(b"T").unwrap()
            );
        }
    }
}

#[test]
fn deleting_a_native_comment_removes_its_popup_and_shared_placements_only() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let mut d = Document::load_mem(&fixture()).unwrap();
    let page = *d.get_pages().values().next().unwrap();
    let square=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Square","Rect"=>vec![10.into(),10.into(),30.into(),30.into()]});
    let annots = d
        .get_dictionary_mut(page)
        .unwrap()
        .get_mut(b"Annots")
        .unwrap()
        .as_array_mut()
        .unwrap();
    annots.push(annots[0].clone());
    annots.push(square.into());
    let mut bytes = vec![];
    d.save_to(&mut bytes).unwrap();
    let path = dir.path().join("delete.pdf");
    fs::write(&path, &bytes).unwrap();
    let source = e.open_document(&path).unwrap();
    let request = serde_json::from_value(
        serde_json::json!({"pageIndex":0,"annotationIndex":0,"expectedText":"External note"}),
    )
    .unwrap();
    let deleted = e.delete_native_comment(&source.id, request).unwrap();
    assert!(e.list_native_comments(&deleted.id).unwrap().is_empty());
    assert_eq!(e.list_native_comments(&source.id).unwrap().len(), 2);
    let output = Document::load_mem(&e.source_bytes(&deleted.id).unwrap()).unwrap();
    let annotations = output.get_page_annotations(page).unwrap();
    assert_eq!(annotations.len(), 1);
    assert_eq!(
        annotations[0].get(b"Subtype").unwrap().as_name().unwrap(),
        b"Square"
    );
    assert_eq!(fs::read(path).unwrap(), bytes);
    let stale = serde_json::from_value(
        serde_json::json!({"pageIndex":0,"annotationIndex":0,"expectedText":"stale"}),
    )
    .unwrap();
    assert!(e.delete_native_comment(&source.id, stale).is_err());
}
#[test]
fn parent_comments_with_replies_cannot_be_deleted() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let mut d = Document::load_mem(&fixture()).unwrap();
    let page = *d.get_pages().values().next().unwrap();
    let parent = d
        .get_dictionary(page)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .clone();
    let reply=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![20.into(),20.into(),38.into(),38.into()],"Contents"=>Object::string_literal("Reply"),"IRT"=>parent,"F"=>4});
    d.get_dictionary_mut(page)
        .unwrap()
        .get_mut(b"Annots")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .push(reply.into());
    let mut bytes = vec![];
    d.save_to(&mut bytes).unwrap();
    let path = dir.path().join("replies.pdf");
    fs::write(&path, &bytes).unwrap();
    let source = e.open_document(&path).unwrap();
    let notes = serde_json::to_value(e.list_native_comments(&source.id).unwrap()).unwrap();
    assert!(notes[0]["deleteReason"]
        .as_str()
        .unwrap()
        .contains("replies"));
    let request = serde_json::from_value(
        serde_json::json!({"pageIndex":0,"annotationIndex":0,"expectedText":"External note"}),
    )
    .unwrap();
    assert!(e.delete_native_comment(&source.id, request).is_err());
    assert_eq!(&*e.source_bytes(&source.id).unwrap(), bytes);
}

#[test]
fn legacy_ownership_survives_native_edit_delete_and_another_checkpoint() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let mut d = Document::load_mem(&fixture()).unwrap();
    let page = *d.get_pages().values().next().unwrap();
    let fixed=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![20.into(),20.into(),38.into(),38.into()],"Contents"=>Object::string_literal("Fixed note"),"F"=>28});
    d.get_dictionary_mut(page)
        .unwrap()
        .get_mut(b"Annots")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .push(fixed.into());
    let mut bytes = vec![];
    d.save_to(&mut bytes).unwrap();
    let path = dir.path().join("legacy.pdf");
    fs::write(&path, &bytes).unwrap();
    let source = e.open_document(&path).unwrap();
    let root = dir.path().join("recovery");
    let store = folio_engine::RecoveryStore::new(root.clone()).unwrap();
    store.save(&e, workspace(&source)).unwrap();
    let (path, mut manifest) = checkpoint(&root);
    manifest["annotationImportVersion"] = 2.into();
    manifest["sources"][&source.id]
        .as_object_mut()
        .unwrap()
        .remove("legacyCommentImport");
    manifest["workspace"]["tabs"][0]["document"]["pages"][0]["overlays"] = serde_json::json!([]);
    fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let restored = store.load(&e).unwrap().unwrap();
    let id = restored["tabs"][0]["document"]["pages"][0]["sourceId"]
        .as_str()
        .unwrap();
    assert_eq!(e.list_native_comments(id).unwrap().len(), 1);
    let edited=e.edit_native_comment(id,serde_json::from_value(serde_json::json!({"pageIndex":0,"annotationIndex":2,"expectedText":"Fixed note","text":"Edited fixed note"})).unwrap()).unwrap();
    assert_eq!(e.list_native_comments(&edited.id).unwrap().len(), 1);
    store.save(&e, workspace(&edited)).unwrap();
    let (_, manifest) = checkpoint(&root);
    assert_eq!(manifest["sources"][&edited.id]["legacyCommentImport"], true);
    let restored = store.load(&e).unwrap().unwrap();
    let id = restored["tabs"][0]["document"]["pages"][0]["sourceId"]
        .as_str()
        .unwrap();
    let deleted=e.delete_native_comment(id,serde_json::from_value(serde_json::json!({"pageIndex":0,"annotationIndex":2,"expectedText":"Edited fixed note"})).unwrap()).unwrap();
    assert!(e.list_native_comments(&deleted.id).unwrap().is_empty());
    store.save(&e, workspace(&deleted)).unwrap();
    let restored = store.load(&e).unwrap().unwrap();
    assert!(e
        .list_native_comments(
            restored["tabs"][0]["document"]["pages"][0]["sourceId"]
                .as_str()
                .unwrap()
        )
        .unwrap()
        .is_empty());
}
#[test]
fn new_recovery_without_explicit_comment_ownership_is_rejected() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let source = e.create_blank_document(300., 400.).unwrap();
    let root = dir.path().join("recovery");
    let store = folio_engine::RecoveryStore::new(root.clone()).unwrap();
    store.save(&e, workspace(&source)).unwrap();
    let (path, mut manifest) = checkpoint(&root);
    manifest["sources"][&source.id]
        .as_object_mut()
        .unwrap()
        .remove("legacyCommentImport");
    fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(store.load(&e).unwrap_err().contains("comment ownership"));
}

#[test]
fn future_recovery_is_never_replaced_by_a_stale_fallback_or_autosave() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let source = e.create_blank_document(300., 400.).unwrap();
    let root = dir.path().join("recovery");
    let store = folio_engine::RecoveryStore::new(root.clone()).unwrap();
    store.save(&e, workspace(&source)).unwrap();
    store.save(&e, workspace(&source)).unwrap();
    let (path, mut manifest) = checkpoint(&root);
    manifest["annotationImportVersion"] = 5.into();
    let bytes = folio_engine::private_storage::seal(
        folio_engine::private_storage::Purpose::Workspace,
        &serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(
        store.load(&e).is_err(),
        "unsupported newest checkpoint must block stale fallback"
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(
        store.save(&e, workspace(&source)).is_err(),
        "autosave must not replace an unsupported checkpoint"
    );
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn application_recovery_migrates_once_and_is_isolated_from_older_cleanup() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.pdf");
    fs::write(&path, fixture()).unwrap();
    let source = e.open_document(&path).unwrap();
    let root = dir.path().join("recovery");
    let old = folio_engine::RecoveryStore::new(root.clone()).unwrap();
    let mut draft = workspace(&source);
    draft["tabs"][0]["document"]["pages"][0]["overlays"] = serde_json::json!([]);
    old.save(&e, draft).unwrap();
    let (path, mut manifest) = checkpoint(&root);
    manifest["annotationImportVersion"] = 2.into();
    manifest["sources"][&source.id]
        .as_object_mut()
        .unwrap()
        .remove("legacyCommentImport");
    fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    drop(old);
    let current = folio_engine::RecoveryStore::for_application(root.clone(), &e).unwrap();
    assert!(
        folio_engine::RecoveryStore::new(root.clone()).is_err(),
        "new app retains the shared compatibility lock"
    );
    let restored = current.load(&e).unwrap().unwrap();
    let id = restored["tabs"][0]["document"]["pages"][0]["sourceId"]
        .as_str()
        .unwrap();
    assert!(
        e.list_native_comments(id).unwrap().is_empty(),
        "a legacy deleted note stays deleted after migration"
    );
    let current_root = root.join("comment-format-3");
    let (path, _) = checkpoint(&current_root);
    let bytes = fs::read(&path).unwrap();
    drop(current);
    let older = folio_engine::RecoveryStore::new(root.clone()).unwrap();
    assert!(older.load(&e).unwrap().is_none());
    older.clear().unwrap();
    older.save(&e, workspace(&source)).unwrap();
    drop(older);
    assert_eq!(
        fs::read(&path).unwrap(),
        bytes,
        "older-slot cleanup cannot alter newer checkpoints"
    );
    let current = folio_engine::RecoveryStore::for_application(root, &e).unwrap();
    let restored = current.load(&e).unwrap().unwrap();
    let id = restored["tabs"][0]["document"]["pages"][0]["sourceId"]
        .as_str()
        .unwrap();
    assert!(
        e.list_native_comments(id).unwrap().is_empty(),
        "later legacy sessions do not overwrite the newer draft"
    );
}

#[test]
fn page_targets_exclude_hidden_comment_flags() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    for flags in [0, 4, 28, 64, 128, 1, 2, 32, 256, 260] {
        let mut doc = Document::load_mem(&fixture()).unwrap();
        let id = doc
            .objects
            .iter()
            .find(|(_, o)| {
                o.as_dict().is_ok_and(|d| {
                    d.get(b"Subtype")
                        .is_ok_and(|s| s.as_name().is_ok_and(|n| n == b"Text"))
                })
            })
            .unwrap()
            .0
            .to_owned();
        doc.get_dictionary_mut(id).unwrap().set("F", flags);
        let path = dir.path().join(format!("flags-{flags}.pdf"));
        doc.save(&path).unwrap();
        let info = e.open_document(&path).unwrap();
        let comments = e.list_native_comments(&info.id).unwrap();
        assert_eq!(comments.len(), 1);
        let value = serde_json::to_value(&comments[0]).unwrap();
        assert_eq!(
            value.get("onPage").and_then(|v| v.as_bool()),
            Some(flags & (1 | 2 | 32 | 256) == 0),
            "flags {flags}"
        );
        e.close_document(&info.id).unwrap();
    }
}

#[test]
fn moving_comments_preserves_unchanged_contents_representation() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    for missing in [false, true] {
        let mut doc = Document::load_mem(&fixture()).unwrap();
        if missing {
            let id = doc
                .objects
                .iter()
                .find(|(_, o)| {
                    o.as_dict().is_ok_and(|d| {
                        d.get(b"Subtype")
                            .is_ok_and(|s| s.as_name().is_ok_and(|n| n == b"Text"))
                    })
                })
                .unwrap()
                .0
                .to_owned();
            doc.get_dictionary_mut(id).unwrap().remove(b"Contents");
        }
        let path = dir.path().join(format!("contents-{missing}.pdf"));
        doc.save(&path).unwrap();
        let before = Document::load(&path).unwrap();
        let info = e.open_document(&path).unwrap();
        let comment = e.list_native_comments(&info.id).unwrap().remove(0);
        let updated = e.edit_native_comment(&info.id, serde_json::from_value(serde_json::json!({"pageIndex":0,"annotationIndex":comment.annotation_index,"expectedText":comment.text,"text":comment.text,"position":{"x":100,"y":110}})).unwrap()).unwrap();
        let output = dir.path().join(format!("moved-{missing}.pdf"));
        e.export_pdf(
            ExportRequest {
                pages: vec![plan(&updated)],
                flatten: false,
            },
            &output,
        )
        .unwrap();
        let after = Document::load(output).unwrap();
        assert_eq!(
            note(&before).get(b"Contents").ok(),
            note(&after).get(b"Contents").ok(),
            "missing Contents: {missing}"
        );
        assert_ne!(
            note(&before).get(b"Rect").ok(),
            note(&after).get(b"Rect").ok()
        );
        e.close_document(&updated.id).unwrap();
        e.close_document(&info.id).unwrap();
    }
}

#[test]
fn rendering_standard_icons_does_not_change_saved_rectangles_or_appearance_ownership() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    for rotation in [0, 90, 180, 270] {
        let mut doc = Document::load_mem(&fixture()).unwrap();
        let id = *doc
            .objects
            .iter()
            .find(|(_, o)| {
                o.as_dict().is_ok_and(|d| {
                    d.get(b"Subtype")
                        .is_ok_and(|s| s.as_name().is_ok_and(|n| n == b"Text"))
                })
            })
            .unwrap()
            .0;
        let data = doc.get_dictionary_mut(id).unwrap();
        data.remove(b"AP");
        data.set("Rect", vec![70.into(), 230.into(), 94.into(), 254.into()]);
        let page_id = *doc.get_pages().values().next().unwrap();
        doc.get_dictionary_mut(page_id)
            .unwrap()
            .set("Rotate", rotation);
        let path = dir.path().join(format!("icon-{rotation}.pdf"));
        doc.save(&path).unwrap();
        let info = e.open_document(&path).unwrap();
        e.render_page(&info.id, 0, 600).unwrap();
        let output = dir.path().join(format!("copy-{rotation}.pdf"));
        e.export_pdf(
            ExportRequest {
                pages: vec![plan(&info)],
                flatten: false,
            },
            &output,
        )
        .unwrap();
        let saved = Document::load(output).unwrap();
        assert_eq!(
            note(&doc).get(b"Rect").unwrap(),
            note(&saved).get(b"Rect").unwrap()
        );
        assert!(!note(&saved).has(b"AP"));
        e.close_document(&info.id).unwrap();
    }
}

#[test]
fn page_hit_bounds_match_pdfium_pixels_for_no_rotate_icons() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    for rotation in [0, 90, 180, 270] {
        for flags in [4, 20, 28] {
            let mut doc = Document::load_mem(&fixture()).unwrap();
            let id = *doc
                .objects
                .iter()
                .find(|(_, o)| {
                    o.as_dict().is_ok_and(|d| {
                        d.get(b"Subtype")
                            .is_ok_and(|s| s.as_name().is_ok_and(|n| n == b"Text"))
                    })
                })
                .unwrap()
                .0;
            let ap=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),20.into(),30.into()]},b"0 0 1 rg 0 0 20 30 re f".to_vec()));
            let data = doc.get_dictionary_mut(id).unwrap();
            data.set("F", flags);
            data.set("AP", dictionary! {"N"=>ap});
            data.set("Rect", vec![70.into(), 230.into(), 90.into(), 260.into()]);
            let page_id = *doc.get_pages().values().next().unwrap();
            let page = doc.get_dictionary_mut(page_id).unwrap();
            page.set("Rotate", rotation);
            page.set(
                "CropBox",
                vec![40.into(), 50.into(), 240.into(), 350.into()],
            );
            let path = dir.path().join(format!("hit-{rotation}-{flags}.pdf"));
            doc.save(&path).unwrap();
            let info = e.open_document(&path).unwrap();
            let value =
                serde_json::to_value(&e.list_native_comments(&info.id).unwrap()[0]).unwrap();
            assert_eq!(value["noRotate"].as_bool(), Some(flags & 16 != 0));
            let bounds = &value["hitBounds"];
            assert!(bounds.is_object(), "missing rendered bounds");
            let png = e
                .render_page(&info.id, 0, (info.pages[0].width * 2.0) as u32)
                .unwrap();
            let bitmap = image::load_from_memory(&png).unwrap().to_rgb8();
            let pixels: Vec<_> = bitmap
                .enumerate_pixels()
                .filter(|(_, _, p)| p[2] > 240 && p[0] < 15 && p[1] < 15)
                .map(|(x, y, _)| (x, y))
                .collect();
            assert!(!pixels.is_empty());
            let x = pixels.iter().map(|p| p.0).min().unwrap() as f64;
            let y = pixels.iter().map(|p| p.1).min().unwrap() as f64;
            let right = pixels.iter().map(|p| p.0).max().unwrap() as f64 + 1.0;
            let bottom = pixels.iter().map(|p| p.1).max().unwrap() as f64 + 1.0;
            for (actual, expected) in [
                (x, bounds["x"].as_f64().unwrap() * 2.0),
                (y, bounds["y"].as_f64().unwrap() * 2.0),
                (
                    right,
                    (bounds["x"].as_f64().unwrap() + bounds["width"].as_f64().unwrap()) * 2.0,
                ),
                (
                    bottom,
                    (bounds["y"].as_f64().unwrap() + bounds["height"].as_f64().unwrap()) * 2.0,
                ),
            ] {
                assert!(
                    (actual - expected).abs() < 1.1,
                    "rotation {rotation} flags {flags}: actual {actual}, expected {expected}"
                );
            }
            e.close_document(&info.id).unwrap();
        }
    }
}

#[test]
fn rotated_single_page_preview_survives_comment_replacement_in_a_multipage_document() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let mut doc = Document::load_mem(&fixture()).unwrap();
    let page_id = *doc.get_pages().values().next().unwrap();
    let parent = doc
        .get_dictionary(page_id)
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    let blank = doc.add_object(dictionary!{"Type"=>"Page","Parent"=>parent,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary!{}});
    doc.get_dictionary_mut(parent).unwrap().set(
        "Kids",
        vec![Object::Reference(blank), Object::Reference(page_id)],
    );
    doc.get_dictionary_mut(parent).unwrap().set("Count", 2);
    doc.get_dictionary_mut(page_id).unwrap().set("Rotate", 90);
    let note_id = doc
        .get_dictionary(page_id)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .as_reference()
        .unwrap();
    doc.get_dictionary_mut(note_id).unwrap().set("F", 20);
    let path = dir.path().join("multipage.pdf");
    doc.save(&path).unwrap();
    let source = e.open_document(&path).unwrap();
    let make_plan = |info: &folio_engine::DocumentInfo| -> PagePlan {
        serde_json::from_value(serde_json::json!({"id":"page","sourceId":info.id,"pageIndex":1,"width":info.pages[1].width,"height":info.pages[1].height,"rotation":90,"overlays":[]})).unwrap()
    };
    e.render_page_plan(make_plan(&source), 600).unwrap();
    let comment = e
        .list_native_comments(&source.id)
        .unwrap()
        .into_iter()
        .find(|n| n.page_index == 1)
        .unwrap();
    let updated=e.edit_native_comment(&source.id,serde_json::from_value(serde_json::json!({"pageIndex":1,"annotationIndex":comment.annotation_index,"expectedText":comment.text,"text":comment.text,"position":{"x":100,"y":110}})).unwrap()).unwrap();
    let preview = e.render_page_plan(make_plan(&updated), 600).unwrap();
    let output = dir.path().join("single-page.pdf");
    e.export_pdf(
        ExportRequest {
            pages: vec![make_plan(&updated)],
            flatten: false,
        },
        &output,
    )
    .unwrap();
    let saved = Document::load(&output).unwrap();
    let owner = note(&saved);
    let popup = saved
        .dereference(owner.get(b"Popup").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    assert_eq!(
        saved
            .dereference(popup.get(b"Parent").unwrap())
            .unwrap()
            .1
            .as_dict()
            .unwrap(),
        owner
    );
    let reopened = e.open_document(&output).unwrap();
    let after = e.render_page(&reopened.id, 0, 600).unwrap();
    assert_eq!(
        image::load_from_memory(&preview).unwrap().to_rgb8(),
        image::load_from_memory(&after).unwrap().to_rgb8()
    );
    e.close_document(&reopened.id).unwrap();
    e.close_document(&updated.id).unwrap();
    e.close_document(&source.id).unwrap();
}

#[test]
fn rotated_preview_preserves_popup_links_after_moving_one_of_several_native_notes() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("comments.pdf");
    fs::write(
        &path,
        include_bytes!("../../tests/fixtures/native-comments-rotation.pdf"),
    )
    .unwrap();
    let source = e.open_document(&path).unwrap();
    let make_plan = |info: &folio_engine::DocumentInfo| -> PagePlan {
        serde_json::from_value(serde_json::json!({"id":"page","sourceId":info.id,"pageIndex":1,"width":info.pages[1].width,"height":info.pages[1].height,"rotation":90,"overlays":[]})).unwrap()
    };
    e.render_page(&source.id, 1, 600).unwrap();
    e.render_page_plan(make_plan(&source), 600).unwrap();
    let comment = e
        .list_native_comments(&source.id)
        .unwrap()
        .into_iter()
        .find(|n| n.text == "Review this paragraph")
        .unwrap();
    let updated=e.edit_native_comment(&source.id,serde_json::from_value(serde_json::json!({"pageIndex":1,"annotationIndex":comment.annotation_index,"expectedText":comment.text,"text":comment.text,"position":{"x":comment.bounds.x+30.0,"y":comment.bounds.y+40.0}})).unwrap()).unwrap();
    let preview = e.render_page_plan(make_plan(&updated), 600).unwrap();
    let output = dir.path().join("single-page.pdf");
    e.export_pdf(
        ExportRequest {
            pages: vec![make_plan(&updated)],
            flatten: false,
        },
        &output,
    )
    .unwrap();
    let saved = Document::load(&output).unwrap();
    let owner = note(&saved);
    let popup = saved
        .dereference(owner.get(b"Popup").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    assert_eq!(
        saved
            .dereference(popup.get(b"Parent").unwrap())
            .unwrap()
            .1
            .as_dict()
            .unwrap(),
        owner
    );
    let reopened = e.open_document(&output).unwrap();
    let after = e.render_page(&reopened.id, 0, 600).unwrap();
    assert_eq!(
        image::load_from_memory(&preview).unwrap().to_rgb8(),
        image::load_from_memory(&after).unwrap().to_rgb8()
    );
    e.close_document(&reopened.id).unwrap();
    e.close_document(&updated.id).unwrap();
    e.close_document(&source.id).unwrap();
}

fn resolved_note_appearance(doc: &Document) -> Option<Object> {
    fn canonical(doc: &Document, value: &Object, depth: usize) -> Object {
        assert!(depth < 32);
        let value = doc.dereference(value).unwrap().1;
        match value {
            // PDF writers may spell an integral real as an integer. Compare
            // their numeric meaning while retaining exact nonintegral values.
            Object::Real(number)
                if number.is_finite()
                    && number.fract() == 0.0
                    && f64::from(*number) >= f64::from(i32::MIN)
                    && f64::from(*number) <= f64::from(i32::MAX) =>
            {
                Object::Integer(*number as i64)
            }
            Object::Array(values) => Object::Array(
                values
                    .iter()
                    .map(|v| canonical(doc, v, depth + 1))
                    .collect(),
            ),
            Object::Dictionary(data) => {
                let mut result = lopdf::Dictionary::new();
                for (key, value) in data {
                    result.set(key.clone(), canonical(doc, value, depth + 1));
                }
                Object::Dictionary(result)
            }
            Object::Stream(stream) => {
                let mut data = stream.dict.clone();
                for key in [b"Length".as_slice(), b"Filter", b"DecodeParms"] {
                    data.remove(key);
                }
                let data = canonical(doc, &Object::Dictionary(data), depth + 1)
                    .as_dict()
                    .unwrap()
                    .clone();
                Object::Stream(Stream::new(
                    data,
                    stream
                        .decompressed_content()
                        .unwrap_or_else(|_| stream.content.clone()),
                ))
            }
            _ => value.clone(),
        }
    }
    note(doc)
        .get(b"AP")
        .ok()
        .map(|value| canonical(doc, value, 0))
}
fn assert_untouched_note(before: &Document, after: &Document) {
    for key in [
        b"Rect".as_slice(),
        b"PDFIUM_HasGeneratedAP",
        b"Contents",
        b"T",
        b"Subj",
        b"Name",
        b"NM",
        b"M",
        b"CreationDate",
        b"F",
        b"C",
    ] {
        assert_eq!(
            note(before).get(key).ok(),
            note(after).get(key).ok(),
            "native note field {:?}",
            String::from_utf8_lossy(key)
        );
    }
    assert_eq!(
        resolved_note_appearance(before),
        resolved_note_appearance(after)
    );
    let owner = note(after);
    let popup = after
        .dereference(owner.get(b"Popup").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    assert_eq!(
        after
            .dereference(popup.get(b"Parent").unwrap())
            .unwrap()
            .1
            .as_dict()
            .unwrap(),
        owner
    );
    let page = *after.get_pages().values().next().unwrap();
    let annots = after.get_dictionary(page).unwrap().get(b"Annots").unwrap();
    assert!(after
        .dereference(annots)
        .unwrap()
        .1
        .as_array()
        .unwrap()
        .iter()
        .any(|v| after.dereference(v).unwrap().1.as_dict().unwrap() == popup));
    let original_popup = before
        .dereference(note(before).get(b"Popup").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    assert_eq!(
        popup.get(b"Rect").unwrap(),
        original_popup.get(b"Rect").unwrap()
    );
}
fn text_edit_preserves_unedited_native_note(mode: &str) {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    for custom in [false, true] {
        for rotation in [0, 90, 180, 270] {
            let mut doc = Document::load_mem(&fixture()).unwrap();
            let page_id = *doc.get_pages().values().next().unwrap();
            let note_id = doc
                .get_dictionary(page_id)
                .unwrap()
                .get(b"Annots")
                .unwrap()
                .as_array()
                .unwrap()[0]
                .as_reference()
                .unwrap();
            if custom {
                let appearance = doc
                    .get_dictionary(note_id)
                    .unwrap()
                    .get(b"AP")
                    .unwrap()
                    .as_dict()
                    .unwrap()
                    .get(b"N")
                    .unwrap()
                    .as_reference()
                    .unwrap();
                let state = doc.add_object(dictionary! {"Type"=>"ExtGState","ca"=>1.0});
                let stream = doc
                    .get_object_mut(appearance)
                    .unwrap()
                    .as_stream_mut()
                    .unwrap();
                stream.dict.set(
                    "Resources",
                    dictionary! {"ExtGState"=>dictionary!{"NoteGS"=>state}},
                );
                stream.set_content(b"/NoteGS gs 0 0 1 rg 0 0 18 18 re f".to_vec());
            }
            let data = doc.get_dictionary_mut(note_id).unwrap();
            if !custom {
                data.remove(b"AP");
            }
            data.set("Rect", vec![70.into(), 230.into(), 94.into(), 254.into()]);
            let font = doc.add_object(
                dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"},
            );
            let resources = dictionary! {"Font"=>dictionary!{"F1"=>font}};
            let program = if matches!(mode, "group" | "group-noop") {
                b"BT /F1 12 Tf 40 330 Td (Old ) Tj (words) Tj ET".to_vec()
            } else {
                b"BT /F1 12 Tf 40 330 Td (Old words) Tj ET".to_vec()
            };
            let (content, page_resources) = if mode == "form" {
                let form=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>resources},program));
                (
                    b"/Form Do".to_vec(),
                    dictionary! {"XObject"=>dictionary!{"Form"=>form}},
                )
            } else {
                (program, resources)
            };
            let stream = doc.add_object(Stream::new(dictionary! {}, content));
            let data = doc.get_dictionary_mut(page_id).unwrap();
            data.set("Resources", page_resources);
            data.set("Contents", stream);
            data.set("Rotate", rotation);
            let path = dir.path().join(format!("{mode}-{rotation}-{custom}.pdf"));
            doc.save(&path).unwrap();
            let original = fs::read(&path).unwrap();
            let source = e.open_document(&path).unwrap();
            e.render_page(&source.id, 0, 600).unwrap();
            if mode == "form" {
                let runs = e.list_form_text_runs(&source.id, 0).unwrap();
                assert!(runs.runs.is_empty());
                assert!(runs
                    .reason
                    .unwrap()
                    .contains("with annotations is unsupported"));
                assert_eq!(fs::read(&path).unwrap(), original);
                e.close_document(&source.id).unwrap();
                continue;
            }
            let edited = match mode {
                "paragraph" => {
                    let selected = e.inspect_text_paragraph(&source.id, 0, &[0]).unwrap();
                    let mut bounds = selected.bounds.clone();
                    bounds.width = (bounds.width + 40.0).min(source.pages[0].width - bounds.x);
                    bounds.height = (bounds.height + 40.0).min(source.pages[0].height - bounds.y);
                    e.preview_text_paragraph(
                        &source.id,
                        0,
                        &[0],
                        &selected.text,
                        "New words",
                        &bounds,
                    )
                    .unwrap()
                }
                "single" => e
                    .replace_text(&source.id, 0, 0, "Old words", "New words")
                    .unwrap(),
                "group" | "group-noop" => {
                    let selected = e.inspect_text_group(&source.id, 0, &[0, 1]).unwrap();
                    let replacement = if mode == "group-noop" { &selected.text } else { "New words" };
                    e.replace_text_group(&source.id, 0, &[0, 1], &selected.text, replacement)
                        .unwrap()
                }
                _ => unreachable!(),
            };
            let candidate = Document::load_mem(&e.source_bytes(&edited.id).unwrap()).unwrap();
            assert_untouched_note(&doc, &candidate);
            assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(), original.as_slice());
            let original_text = if mode == "group-noop" { Some(e.extract_text(&source.id, 0).unwrap()) } else { None };
            if mode == "group-noop" {
                assert_eq!(e.render_page(&edited.id, 0, 600).unwrap(), e.render_page(&source.id, 0, 600).unwrap());
                e.close_document(&source.id).unwrap();
            }
            let output = dir
                .path()
                .join(format!("edited-{mode}-{rotation}-{custom}.pdf"));
            e.export_pdf(
                ExportRequest {
                    pages: vec![plan(&edited)],
                    flatten: false,
                },
                &output,
            )
            .unwrap();
            let saved = Document::load(&output).unwrap();
            assert_untouched_note(&doc, &saved);
            if let Some(original_text) = original_text {
                assert_eq!(e.extract_text(&edited.id, 0).unwrap(), original_text);
            } else {
                assert!(e.extract_text(&edited.id, 0).unwrap().contains("New words"));
            }
            assert_eq!(fs::read(&path).unwrap(), original);
            e.close_document(&edited.id).unwrap();
            if mode != "group-noop" { e.close_document(&source.id).unwrap(); }
        }
    }
}
#[test]
fn paragraph_rewrite_retains_native_note_appearance_ownership() {
    text_edit_preserves_unedited_native_note("paragraph");
}
#[test]
fn single_text_rewrite_retains_native_note_appearance_ownership() {
    text_edit_preserves_unedited_native_note("single");
}
#[test]
fn grouped_text_rewrite_retains_native_note_appearance_ownership() {
    text_edit_preserves_unedited_native_note("group");
}
#[test]
fn unchanged_group_retains_native_note_after_original_is_closed() {
    text_edit_preserves_unedited_native_note("group-noop");
}
#[test]
fn form_text_refuses_annotated_sources_without_rewriting_them() {
    text_edit_preserves_unedited_native_note("form");
}
