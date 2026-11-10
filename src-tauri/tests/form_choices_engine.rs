use folio_engine::{
    ChoiceDraftOption, ChoiceEdit, ChoiceEditSelection as Edit, ChoiceSelection, DocumentInfo,
    ExportRequest, FillFormFieldRequest, FormChoices, FormFieldValue, PagePlan, PdfEngine,
    RecoveryStore, SetFormChoicesRequest,
};
use lopdf::{dictionary, Document, Object};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
#[path = "support/protected_fixture.rs"]
mod protected_fixture;

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/choice-text/combo-source.pdf")
}
fn request(info: FormChoices) -> SetFormChoicesRequest {
    SetFormChoicesRequest {
        page_index: 0,
        annotation_index: 0,
        expected_key: info.expected_key,
        edit: ChoiceEdit {
            options: info
                .model
                .options
                .into_iter()
                .enumerate()
                .map(|(i, o)| ChoiceDraftOption {
                    source_index: Some(i),
                    display: o.display,
                    export_value: o.export_value,
                })
                .collect(),
            selection: Edit::Keep,
            defaults: Edit::Keep,
            manual_order: false,
        },
    }
}
fn plans(info: &DocumentInfo) -> Vec<PagePlan> {
    info.pages.iter().enumerate().map(|(i,p)|serde_json::from_value(json!({"id":format!("p{i}"),"sourceId":info.id,"pageIndex":i,"width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})).unwrap()).collect()
}
fn export(e: &PdfEngine, info: &DocumentInfo, path: &Path) {
    e.export_pdf(
        ExportRequest {
            pages: plans(info),
            flatten: false,
        },
        path,
    )
    .unwrap();
}

#[test]
fn choice_worker_publishes_complete_source_then_fills_saves_and_reopens() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let path = fixture();
    let original = std::fs::read(&path).unwrap();
    let source = e.open_document(&path).unwrap();
    let before = e.inspect_form_choices(&source.id, 0, 0).unwrap();
    let mut edit = request(before.clone());
    edit.edit.options[1].display = "Updated label".into();
    edit.edit.options[1].export_value = "updated".into();
    let candidate = e.set_form_choices(&source.id, edit.clone()).unwrap();
    assert_ne!(candidate.id, source.id);
    assert_eq!(candidate.pages, source.pages);
    assert!(e.source_bytes(&source.id).unwrap().as_ref() == original.as_slice());
    assert!(
        e.set_form_choices(&candidate.id, edit).is_err(),
        "stale source key reused"
    );
    for page in 0..2 {
        let after = e.inspect_form_choices(&candidate.id, page, 0).unwrap();
        assert_eq!(after.widgets.len(), 2);
        assert_eq!(after.model.options[1].display, "Updated label");
        assert_eq!(after.model.options[1].export_value, "updated");
        assert_eq!(after.model.selection, ChoiceSelection::Rows(vec![1]));
        assert_eq!(after.model.defaults, before.model.defaults);
        assert!(
            e.render_page(&candidate.id, page, 600).unwrap()
                != e.render_page(&source.id, page, 600).unwrap()
        );
    }
    let saved = temp.path().join("changed.pdf");
    export(&e, &candidate, &saved);
    let reopened = e.open_document(&saved).unwrap();
    assert_eq!(
        e.inspect_form_choices(&reopened.id, 0, 0).unwrap().model,
        e.inspect_form_choices(&candidate.id, 0, 0).unwrap().model
    );
    let value = e.list_form_fields(&reopened.id, 1).unwrap()[0]
        .value
        .clone();
    let filled = e
        .fill_form_field(
            &reopened.id,
            FillFormFieldRequest {
                page_index: 1,
                annotation_index: 0,
                expected_value: value,
                value: FormFieldValue::Choice { indices: vec![2] },
            },
        )
        .unwrap();
    for page in 0..2 {
        assert_eq!(
            e.inspect_form_choices(&filled.id, page, 0)
                .unwrap()
                .model
                .selection,
            ChoiceSelection::Rows(vec![2])
        );
    }
    let saved = temp.path().join("filled.pdf");
    export(&e, &filled, &saved);
    let final_source = e.open_document(saved).unwrap();
    assert_eq!(
        e.inspect_form_choices(&final_source.id, 0, 0)
            .unwrap()
            .model
            .options[1]
            .display,
        "Updated label"
    );
    assert_eq!(
        e.inspect_form_choices(&final_source.id, 1, 0)
            .unwrap()
            .model
            .selection,
        ChoiceSelection::Rows(vec![2])
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
    for id in [
        &source.id,
        &candidate.id,
        &reopened.id,
        &filled.id,
        &final_source.id,
    ] {
        e.close_document(id).unwrap();
    }
}

#[test]
fn choice_worker_keeps_bookmarks_actions_physical_dimensions_and_exact_noops() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut doc = Document::load(fixture()).unwrap();
    let pages: Vec<_> = doc.get_pages().values().copied().collect();
    let unit = doc.add_object(Object::Real(1.7));
    doc.get_dictionary_mut(pages[1])
        .unwrap()
        .set("UserUnit", unit);
    let outline = doc.new_object_id();
    let item=doc.add_object(dictionary! {"Title"=>lopdf::text_string("Linked field"),"Parent"=>outline,"Dest"=>vec![Object::Reference(pages[1]),Object::Name(b"Fit".to_vec())]});
    doc.objects.insert(
        outline,
        dictionary! {"Type"=>"Outlines","First"=>item,"Last"=>item,"Count"=>1}.into(),
    );
    doc.catalog_mut().unwrap().set("Outlines", outline);
    let widget = doc
        .get_dictionary(pages[0])
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .as_reference()
        .unwrap();
    let owner = doc
        .get_dictionary(widget)
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    let action =
        doc.add_object(dictionary! {"S"=>"ResetForm","Fields"=>vec![Object::Reference(owner)]});
    doc.get_dictionary_mut(widget).unwrap().set("A", action);
    let path = temp.path().join("metadata.pdf");
    doc.save(&path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let source = e.open_document(&path).unwrap();
    let same = e
        .set_form_choices(
            &source.id,
            request(e.inspect_form_choices(&source.id, 0, 0).unwrap()),
        )
        .unwrap();
    assert!(e.source_bytes(&same.id).unwrap().as_ref() == bytes.as_slice());
    let mut edit = request(e.inspect_form_choices(&source.id, 0, 0).unwrap());
    edit.edit.options[1].display = "Updated".into();
    let candidate = e.set_form_choices(&source.id, edit).unwrap();
    assert_eq!(candidate.pages, source.pages);
    assert_eq!(candidate.bookmarks, source.bookmarks);
    assert_eq!(candidate.bookmarks.len(), 1);
    assert_eq!(candidate.bookmark_error, source.bookmark_error);
    let saved = Document::load_mem(&e.source_bytes(&candidate.id).unwrap()).unwrap();
    assert_eq!(
        saved.get_object(action).unwrap(),
        doc.get_object(action).unwrap()
    );
    assert_eq!(
        saved.get_dictionary(widget).unwrap().get(b"A").unwrap(),
        &Object::Reference(action)
    );
    assert!(e.inspect_form_choices(&source.id, 999, 0).is_err());
    let mut bad = request(e.inspect_form_choices(&source.id, 0, 0).unwrap());
    bad.edit.options[1].display = "Missing 字".into();
    assert!(e.set_form_choices(&source.id, bad).is_err());
    assert!(e.source_bytes(&source.id).unwrap().as_ref() == bytes.as_slice());
    for id in [&source.id, &same.id, &candidate.id] {
        e.close_document(id).unwrap();
    }
}

#[test]
fn choice_worker_recovery_preserves_original_path_and_all_source_occurrences() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let source = e.open_document(fixture()).unwrap();
    let mut edit = request(e.inspect_form_choices(&source.id, 0, 0).unwrap());
    edit.edit.options[1].display = "Recovered label".into();
    let candidate = e.set_form_choices(&source.id, edit).unwrap();
    let expected = e.inspect_form_choices(&candidate.id, 0, 0).unwrap().model;
    let mut pages = plans(&candidate);
    let mut repeated = pages[0].clone();
    repeated.id = "repeated".into();
    pages.push(repeated);
    let workspace = json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":true,"savedDigest":"before-choice-edit","zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"Choices.pdf","selectedPageId":"p0","selectedOverlayId":null,"pages":pages}}]});
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    store.save(&e, workspace).unwrap();
    let snapshot = std::fs::read_dir(&root)
        .unwrap()
        .map(|v| v.unwrap().path())
        .find(|p| p.extension().is_some_and(|v| v == "json"))
        .unwrap();
    let manifest: Value = serde_json::from_slice(
        &folio_engine::private_storage::unseal(
            folio_engine::private_storage::Purpose::Workspace,
            &std::fs::read(snapshot).unwrap(),
            32 * 1024 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        PathBuf::from(
            manifest["sources"][&candidate.id]["originalPath"]
                .as_str()
                .unwrap()
        )
        .canonicalize()
        .unwrap(),
        fixture().canonicalize().unwrap()
    );
    for id in [&source.id, &candidate.id] {
        e.close_document(id).unwrap();
    }
    let restored = store.load(&e).unwrap().unwrap();
    let pages = restored["tabs"][0]["document"]["pages"].as_array().unwrap();
    assert_eq!(pages.len(), 3);
    let id = pages[0]["sourceId"].as_str().unwrap();
    assert_ne!(id, candidate.id);
    for p in pages {
        assert_eq!(p["sourceId"], id);
        assert_eq!(
            e.inspect_form_choices(id, p["pageIndex"].as_u64().unwrap() as usize, 0)
                .unwrap()
                .model,
            expected
        );
    }
    e.close_document(id).unwrap();
}

#[test]
fn choice_worker_requires_content_and_annotation_rights_and_preserves_owner_security() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let original = std::fs::read(fixture()).unwrap();
    for (index, permissions) in [
        lopdf::encryption::Permissions::FILLABLE,
        lopdf::encryption::Permissions::MODIFIABLE,
    ]
    .into_iter()
    .enumerate()
    {
        let encrypted = protected_fixture::encrypt(&original, permissions);
        let path = temp.path().join(format!("protected-{index}.pdf"));
        std::fs::write(&path, &encrypted).unwrap();
        let reader = e.open_document_with_password(&path, "reader").unwrap();
        let owner = e.open_document_with_password(&path, "owner").unwrap();
        assert!(e.inspect_form_choices(&reader.id, 0, 0).is_err());
        let mut edit = request(e.inspect_form_choices(&owner.id, 0, 0).unwrap());
        edit.edit.options[1].display = "Owner label".into();
        assert!(e.set_form_choices(&reader.id, edit.clone()).is_err());
        let candidate = e.set_form_choices(&owner.id, edit).unwrap();
        assert_eq!(candidate.protection, owner.protection);
        let saved = temp.path().join(format!("owner-{index}.pdf"));
        export(&e, &candidate, &saved);
        assert!(e.open_document(&saved).is_err());
        let reopened = e.open_document_with_password(&saved, "owner").unwrap();
        assert_eq!(
            e.inspect_form_choices(&reopened.id, 0, 0)
                .unwrap()
                .model
                .options[1]
                .display,
            "Owner label"
        );
        let restricted = e.open_document_with_password(&saved, "reader").unwrap();
        assert!(e.inspect_form_choices(&restricted.id, 0, 0).is_err());
        assert_eq!(std::fs::read(path).unwrap(), encrypted);
        for id in [
            &reader.id,
            &owner.id,
            &candidate.id,
            &reopened.id,
            &restricted.id,
        ] {
            e.close_document(id).unwrap();
        }
    }
}
