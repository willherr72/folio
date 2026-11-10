use folio_engine::{FormPropertyChanges, PdfEngine, SetFormPropertiesRequest};
use lopdf::{Document, Object};
use std::path::PathBuf;
#[path = "support/protected_fixture.rs"]
mod protected_fixture;
fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/form-text-appearance/fields-r0-u1.pdf")
}
fn widget(d: &Document, page: usize) -> &lopdf::Dictionary {
    let page = d
        .get_dictionary(*d.get_pages().values().nth(page).unwrap())
        .unwrap();
    d.dereference(&page.get(b"Annots").unwrap().as_array().unwrap()[0])
        .unwrap()
        .1
        .as_dict()
        .unwrap()
}
#[test]
fn shared_field_properties_preserve_both_widgets_values_and_saved_appearances() {
    let e = engine();
    let path = fixture();
    let before = std::fs::read(&path).unwrap();
    let original = Document::load_mem(&before).unwrap();
    let source = e.open_document(&path).unwrap();
    let old = e.inspect_form_properties(&source.id, 0, 0).unwrap();
    assert_eq!(old.linked_widgets, 2);
    assert!(old.rename_reason.is_some());
    let result = e
        .set_form_properties(
            &source.id,
            SetFormPropertiesRequest {
                page_index: 0,
                annotation_index: 0,
                expected_key: old.expected_key,
                changes: FormPropertyChanges {
                    name: None,
                    label: Some("Shared answer tooltip".into()),
                    read_only: Some(true),
                    required: Some(true),
                },
            },
        )
        .unwrap();
    let after = Document::load_mem(&e.source_bytes(&result.id).unwrap()).unwrap();
    for page in [0, 1] {
        let p = e.inspect_form_properties(&result.id, page, 0).unwrap();
        assert_eq!(p.label, "Shared answer tooltip");
        assert!(p.read_only && p.required);
        assert_eq!(p.linked_widgets, 2);
        let a = widget(&original, page);
        let b = widget(&after, page);
        for key in [b"Rect".as_slice(), b"AP".as_slice(), b"Parent".as_slice()] {
            assert_eq!(a.get(key).unwrap(), b.get(key).unwrap());
        }
        let ap = |d: &Document, w: &lopdf::Dictionary| {
            let ap = d
                .dereference(w.get(b"AP").unwrap())
                .unwrap()
                .1
                .as_dict()
                .unwrap();
            d.dereference(ap.get(b"N").unwrap())
                .unwrap()
                .1
                .as_stream()
                .unwrap()
                .get_plain_content()
                .unwrap()
        };
        assert_eq!(ap(&original, a), ap(&after, b));
        assert_eq!(
            e.list_form_fields(&result.id, page).unwrap()[0].value,
            e.list_form_fields(&source.id, page).unwrap()[0].value
        );
    }
    assert_eq!(std::fs::read(path).unwrap(), before);
    e.close_document(&source.id).unwrap();
    e.close_document(&result.id).unwrap();
}
fn changes() -> FormPropertyChanges {
    FormPropertyChanges {
        name: None,
        label: None,
        read_only: None,
        required: None,
    }
}
#[test]
fn review_empty_tooltips_allow_flags_unlock_rename_and_clearing() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for mode in 0..3 {
        let mut d = plain();
        let owner = parent(&d);
        let field = d.get_dictionary_mut(owner).unwrap();
        if mode == 0 {
            field.remove(b"TU");
        }
        if mode == 1 {
            field.set("TU", Object::string_literal(""));
        }
        field.set("Ff", 1);
        let path = temp.path().join(format!("empty-tooltip-{mode}.pdf"));
        d.save(&path).unwrap();
        let source = e.open_document(&path).unwrap();
        let updated = apply(
            &e,
            &source.id,
            FormPropertyChanges {
                read_only: Some(false),
                required: Some(true),
                label: (mode == 2).then(String::new),
                ..changes()
            },
        );
        let p = e.inspect_form_properties(&updated.id, 0, 0).unwrap();
        assert_eq!(p.label, "");
        assert!(!p.read_only && p.required);
        let renamed = apply(
            &e,
            &updated.id,
            FormPropertyChanges {
                name: Some("answer".into()),
                ..changes()
            },
        );
        assert_eq!(
            e.inspect_form_properties(&renamed.id, 0, 0).unwrap().label,
            ""
        );
        for id in [&source.id, &updated.id, &renamed.id] {
            e.close_document(id).unwrap();
        }
    }
}
fn nest(d: &mut Document, owner: lopdf::ObjectId, name: Object) -> lopdf::ObjectId {
    let top = d.add_object(lopdf::dictionary! {"T"=>name,"Kids"=>vec![Object::Reference(owner)]});
    d.get_dictionary_mut(owner).unwrap().set("Parent", top);
    let form = d
        .dereference(d.catalog().unwrap().get(b"AcroForm").unwrap())
        .unwrap()
        .1
        .clone();
    let form = d.add_object(form);
    let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
    d.get_dictionary_mut(root).unwrap().set("AcroForm", form);
    let roots = d
        .get_dictionary_mut(form)
        .unwrap()
        .get_mut(b"Fields")
        .unwrap()
        .as_array_mut()
        .unwrap();
    *roots
        .iter_mut()
        .find(|v| v.as_reference().ok() == Some(owner))
        .unwrap() = Object::Reference(top);
    top
}
#[test]
fn review_unnamed_nested_field_refuses_rename_but_preserves_ancestor_actions_on_flag_edits() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut d = plain();
    let owner = parent(&d);
    d.get_dictionary_mut(owner).unwrap().remove(b"T");
    nest(&mut d, owner, Object::string_literal("section"));
    let action = d.add_object(
        lopdf::dictionary! {"S"=>"ResetForm", "Fields"=>vec![Object::string_literal("section")]},
    );
    let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
    d.get_dictionary_mut(root)
        .unwrap()
        .set("FolioAction", action);
    let path = temp.path().join("unnamed.pdf");
    d.save(&path).unwrap();
    let source = e.open_document(path).unwrap();
    let p = e.inspect_form_properties(&source.id, 0, 0).unwrap();
    assert!(
        p.rename_reason.is_some(),
        "unnamed terminal allowed an ambiguous rename"
    );
    assert!(e
        .set_form_properties(
            &source.id,
            SetFormPropertiesRequest {
                page_index: 0,
                annotation_index: 0,
                expected_key: p.expected_key,
                changes: FormPropertyChanges {
                    name: Some("answer".into()),
                    ..changes()
                }
            }
        )
        .is_err());
    let changed = apply(
        &e,
        &source.id,
        FormPropertyChanges {
            required: Some(true),
            ..changes()
        },
    );
    let out = Document::load_mem(&e.source_bytes(&changed.id).unwrap()).unwrap();
    assert_eq!(
        out.get_dictionary(action).unwrap(),
        d.get_dictionary(action).unwrap()
    );
    assert!(!out.get_dictionary(owner).unwrap().has(b"T"));
    e.close_document(&source.id).unwrap();
    e.close_document(&changed.id).unwrap();
}
#[test]
fn review_explicit_empty_partial_name_keeps_ancestor_scope_when_renamed() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut d = plain();
    let owner = parent(&d);
    d.get_dictionary_mut(owner)
        .unwrap()
        .set("T", Object::string_literal(""));
    nest(&mut d, owner, Object::string_literal("section"));
    let action = d.add_object(lopdf::dictionary! {"S"=>"ResetForm", "Fields"=>vec![Object::string_literal("section"), Object::string_literal("section.")]});
    let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
    d.get_dictionary_mut(root)
        .unwrap()
        .set("FolioAction", action);
    let path = temp.path().join("empty-partial.pdf");
    d.save(&path).unwrap();
    let source = e.open_document(path).unwrap();
    let changed = apply(
        &e,
        &source.id,
        FormPropertyChanges {
            name: Some("answer".into()),
            ..changes()
        },
    );
    assert_eq!(
        e.inspect_form_properties(&changed.id, 0, 0)
            .unwrap()
            .full_name,
        "section.answer"
    );
    let out = Document::load_mem(&e.source_bytes(&changed.id).unwrap()).unwrap();
    assert_eq!(
        out.get_dictionary(action)
            .unwrap()
            .get(b"Fields")
            .unwrap()
            .as_array()
            .unwrap(),
        &vec![
            Object::string_literal("section"),
            Object::string_literal("section.answer")
        ]
    );
    e.close_document(&source.id).unwrap();
    e.close_document(&changed.id).unwrap();
}
#[test]
fn review_canonical_names_reject_bom_collisions_before_and_after_renaming() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for nested in [false, true] {
        for utf16 in [false, true] {
            let mut d = plain();
            let owner = parent(&d);
            let page = *d.get_pages().values().next().unwrap();
            let other = d
                .get_dictionary(page)
                .unwrap()
                .get(b"Annots")
                .unwrap()
                .as_array()
                .unwrap()[1]
                .as_reference()
                .unwrap();
            let other_name = if utf16 {
                Object::string_literal(b"\xfe\xff\0a\0n\0s\0w\0e\0r".to_vec())
            } else {
                Object::string_literal(b"\xef\xbb\xbfanswer".to_vec())
            };
            d.get_dictionary_mut(other).unwrap().set("T", other_name);
            if nested {
                // Both siblings under a canonically decoded UTF-8 ancestor.
                let top = nest(
                    &mut d,
                    owner,
                    Object::string_literal(b"\xef\xbb\xbfsection".to_vec()),
                );
                d.get_dictionary_mut(top).unwrap().set(
                    "Kids",
                    vec![Object::Reference(owner), Object::Reference(other)],
                );
                d.get_dictionary_mut(other).unwrap().set("Parent", top);
                let form = d
                    .catalog()
                    .unwrap()
                    .get(b"AcroForm")
                    .unwrap()
                    .as_reference()
                    .unwrap();
                d.get_dictionary_mut(form)
                    .unwrap()
                    .get_mut(b"Fields")
                    .unwrap()
                    .as_array_mut()
                    .unwrap()
                    .retain(|v| v.as_reference().ok() != Some(other));
            }
            let path = temp.path().join(format!("canonical-{nested}-{utf16}.pdf"));
            d.save(&path).unwrap();
            let source = e.open_document(&path).unwrap();
            let p = e.inspect_form_properties(&source.id, 0, 0).unwrap();
            let result = e.set_form_properties(
                &source.id,
                SetFormPropertiesRequest {
                    page_index: 0,
                    annotation_index: 0,
                    expected_key: p.expected_key,
                    changes: FormPropertyChanges {
                        name: Some("answer".into()),
                        ..changes()
                    },
                },
            );
            assert!(
                result.is_err(),
                "canonical duplicate accepted: nested={nested}, utf16={utf16}"
            );
            e.close_document(&source.id).unwrap();
            d.get_dictionary_mut(owner)
                .unwrap()
                .set("T", Object::string_literal("answer"));
            let path = temp.path().join(format!("duplicate-{nested}-{utf16}.pdf"));
            d.save(&path).unwrap();
            let source = e.open_document(path).unwrap();
            assert!(
                e.inspect_form_properties(&source.id, 0, 0).is_err(),
                "preexisting canonical duplicate accepted"
            );
            e.close_document(&source.id).unwrap();
        }
    }
}
fn plain() -> Document {
    let mut d = Document::load(fixture()).unwrap();
    for obj in d.objects.values_mut() {
        if let Ok(dict) = obj.as_dict_mut() {
            dict.remove(b"AA");
        }
    }
    d.prune_objects();
    d
}
fn parent(d: &Document) -> lopdf::ObjectId {
    widget(d, 0).get(b"Parent").unwrap().as_reference().unwrap()
}
fn apply(e: &PdfEngine, id: &str, c: FormPropertyChanges) -> folio_engine::DocumentInfo {
    let p = e.inspect_form_properties(id, 0, 0).unwrap();
    e.set_form_properties(
        id,
        SetFormPropertiesRequest {
            page_index: 0,
            annotation_index: 0,
            expected_key: p.expected_key,
            changes: c,
        },
    )
    .unwrap()
}
#[test]
fn rename_updates_standard_actions_without_rewriting_shared_strings_or_widget_references() {
    use lopdf::dictionary;
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut d = plain();
    let owner = parent(&d);
    let old =
        lopdf::decode_text_string(d.get_dictionary(owner).unwrap().get(b"T").unwrap()).unwrap();
    let shared_name = d.add_object(lopdf::text_string(&old));
    let shared_targets = d.add_object(vec![
        Object::Reference(shared_name),
        Object::Reference(owner),
        Object::string_literal("Unrelated"),
    ]);
    let reset = d.add_object(dictionary! {"S"=>"ResetForm","Fields"=>shared_targets});
    let submit=d.add_object(dictionary!{"S"=>"SubmitForm","Fields"=>shared_targets,"F"=>Object::string_literal("https://example.invalid/form")});
    let hide = d.add_object(dictionary! {"S"=>"Hide","T"=>shared_name});
    let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
    d.get_dictionary_mut(root).unwrap().set(
        "FolioActions",
        vec![
            Object::Reference(reset),
            Object::Reference(submit),
            Object::Reference(hide),
        ],
    );
    d.get_dictionary_mut(root)
        .unwrap()
        .set("FolioUnrelated", shared_name);
    let path = temp.path().join("actions.pdf");
    d.save(&path).unwrap();
    let source = e.open_document(path).unwrap();
    let p = e.inspect_form_properties(&source.id, 0, 0).unwrap();
    assert_eq!(p.rename_reason, None);
    let result = apply(
        &e,
        &source.id,
        FormPropertyChanges {
            name: Some("Renamed café".into()),
            ..changes()
        },
    );
    let out = Document::load_mem(&e.source_bytes(&result.id).unwrap()).unwrap();
    assert_eq!(
        lopdf::decode_text_string(out.get_object(shared_name).unwrap()).unwrap(),
        old
    );
    assert_eq!(
        out.get_dictionary(root)
            .unwrap()
            .get(b"FolioUnrelated")
            .unwrap()
            .as_reference()
            .unwrap(),
        shared_name
    );
    for action in [reset, submit] {
        let targets = out
            .get_dictionary(action)
            .unwrap()
            .get(b"Fields")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(
            lopdf::decode_text_string(&targets[0]).unwrap(),
            "Renamed café"
        );
        assert_eq!(targets[1].as_reference().unwrap(), owner);
        assert_eq!(lopdf::decode_text_string(&targets[2]).unwrap(), "Unrelated");
    }
    assert_eq!(
        lopdf::decode_text_string(out.get_dictionary(hide).unwrap().get(b"T").unwrap()).unwrap(),
        "Renamed café"
    );
    for page in [0, 1] {
        assert_eq!(
            e.list_form_fields(&result.id, page).unwrap()[0].name,
            "Renamed café"
        );
    }
    e.close_document(&source.id).unwrap();
    e.close_document(&result.id).unwrap();
}
#[test]
fn desktop_saved_renamed_widget_action_allows_later_filling_and_remains_intact() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut d = plain();
    let owner = parent(&d);
    let old =
        lopdf::decode_text_string(d.get_dictionary(owner).unwrap().get(b"T").unwrap()).unwrap();
    let action = d.add_object(
        lopdf::dictionary! {"S"=>"ResetForm","Fields"=>vec![Object::string_literal(old)]},
    );
    let page = *d.get_pages().values().next().unwrap();
    let widget = d
        .get_dictionary(page)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .as_reference()
        .unwrap();
    d.get_dictionary_mut(widget).unwrap().set("A", action);
    // Real composition can leave an obsolete, unreferenced action object.
    d.add_object(
        lopdf::dictionary! {"S"=>"ResetForm","Fields"=>vec![Object::string_literal("obsolete")]},
    );
    let path = temp.path().join("action-source.pdf");
    d.save(&path).unwrap();
    let original = std::fs::read(&path).unwrap();
    let source = e.open_document(&path).unwrap();
    let changed = apply(
        &e,
        &source.id,
        FormPropertyChanges {
            name: Some("renamed_answer".into()),
            ..changes()
        },
    );
    let saved = temp.path().join("action-saved.pdf");
    export(&e, &changed, &saved);
    let reopened = e.open_document(&saved).unwrap();
    let value = e.list_form_fields(&reopened.id, 1).unwrap()[0]
        .value
        .clone();
    let filled = e
        .fill_form_field(
            &reopened.id,
            folio_engine::FillFormFieldRequest {
                page_index: 1,
                annotation_index: 0,
                expected_value: value,
                value: folio_engine::FormFieldValue::Text {
                    text: "After unlocking".into(),
                },
            },
        )
        .unwrap();
    let final_path = temp.path().join("action-filled.pdf");
    export(&e, &filled, &final_path);
    let out = Document::load(final_path).unwrap();
    let page = *out.get_pages().values().next().unwrap();
    let widget = out
        .dereference(
            &out.get_dictionary(page)
                .unwrap()
                .get(b"Annots")
                .unwrap()
                .as_array()
                .unwrap()[0],
        )
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    let action = out
        .dereference(widget.get(b"A").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    assert_eq!(action.get(b"S").unwrap().as_name().unwrap(), b"ResetForm");
    assert_eq!(
        lopdf::decode_text_string(&action.get(b"Fields").unwrap().as_array().unwrap()[0]).unwrap(),
        "renamed_answer"
    );
    for page in [0, 1] {
        assert_eq!(
            e.list_form_fields(&filled.id, page).unwrap()[0].value,
            folio_engine::FormFieldValue::Text {
                text: "After unlocking".into()
            }
        );
    }
    assert_eq!(std::fs::read(path).unwrap(), original);
    for id in [&source.id, &changed.id, &reopened.id, &filled.id] {
        e.close_document(id).unwrap();
    }
}
#[test]
fn nested_names_and_inherited_flags_keep_unrelated_bits_and_allow_unlocking() {
    use lopdf::dictionary;
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut d = plain();
    let owner = parent(&d);
    d.get_dictionary_mut(owner).unwrap().remove(b"Ff");
    let top=d.add_object(dictionary!{"T"=>Object::string_literal("section"),"Ff"=>5,"Kids"=>vec![Object::Reference(owner)]});
    d.get_dictionary_mut(owner).unwrap().set("Parent", top);
    let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let form_object = d
        .dereference(d.get_dictionary(root).unwrap().get(b"AcroForm").unwrap())
        .unwrap()
        .1
        .clone();
    let form = d.add_object(form_object);
    d.get_dictionary_mut(root).unwrap().set("AcroForm", form);
    let roots = d
        .get_dictionary_mut(form)
        .unwrap()
        .get_mut(b"Fields")
        .unwrap()
        .as_array_mut()
        .unwrap();
    let first = roots
        .iter_mut()
        .find(|v| v.as_reference().ok() == Some(owner))
        .unwrap();
    *first = Object::Reference(top);
    let path = temp.path().join("nested.pdf");
    d.save(&path).unwrap();
    let source = e.open_document(path).unwrap();
    let old = e.inspect_form_properties(&source.id, 0, 0).unwrap();
    assert!(old.read_only && !old.required);
    assert!(old.full_name.starts_with("section."));
    let result = apply(
        &e,
        &source.id,
        FormPropertyChanges {
            name: Some("answer".into()),
            label: Some("Café answer".into()),
            read_only: Some(false),
            required: Some(true),
        },
    );
    let p = e.inspect_form_properties(&result.id, 0, 0).unwrap();
    assert_eq!(p.full_name, "section.answer");
    assert!(!p.read_only && p.required);
    let out = Document::load_mem(&e.source_bytes(&result.id).unwrap()).unwrap();
    assert_eq!(
        out.get_dictionary(owner)
            .unwrap()
            .get(b"Ff")
            .unwrap()
            .as_i64()
            .unwrap(),
        6
    );
    assert_eq!(
        out.get_dictionary(top)
            .unwrap()
            .get(b"Ff")
            .unwrap()
            .as_i64()
            .unwrap(),
        5
    );
    e.close_document(&source.id).unwrap();
    e.close_document(&result.id).unwrap();
}
#[test]
fn stale_duplicate_invalid_names_and_script_renames_are_rejected() {
    let e = engine();
    let source = e.open_document(fixture()).unwrap();
    let p = e.inspect_form_properties(&source.id, 0, 0).unwrap();
    for (key, c) in [
        (
            p.expected_key.clone(),
            FormPropertyChanges {
                name: Some("safe".into()),
                ..changes()
            },
        ),
        (
            "stale".into(),
            FormPropertyChanges {
                required: Some(true),
                ..changes()
            },
        ),
        (
            p.expected_key.clone(),
            FormPropertyChanges {
                name: Some("bad.name".into()),
                ..changes()
            },
        ),
        (p.expected_key.clone(), changes()),
    ] {
        assert!(e
            .set_form_properties(
                &source.id,
                SetFormPropertiesRequest {
                    page_index: 0,
                    annotation_index: 0,
                    expected_key: key,
                    changes: c
                }
            )
            .is_err());
    }
    let temp = tempfile::tempdir().unwrap();
    let mut d = plain();
    let path = temp.path().join("plain.pdf");
    d.save(&path).unwrap();
    let other = e.open_document(path).unwrap();
    let all = e.list_form_fields(&other.id, 0).unwrap();
    let p = e.inspect_form_properties(&other.id, 0, 0).unwrap();
    assert!(e
        .set_form_properties(
            &other.id,
            SetFormPropertiesRequest {
                page_index: 0,
                annotation_index: 0,
                expected_key: p.expected_key,
                changes: FormPropertyChanges {
                    name: Some(all[1].name.clone()),
                    ..changes()
                }
            }
        )
        .is_err());
    e.close_document(&source.id).unwrap();
    e.close_document(&other.id).unwrap();
}
fn export(e: &PdfEngine, info: &folio_engine::DocumentInfo, path: &std::path::Path) {
    let pages=info.pages.iter().enumerate().map(|(i,p)|serde_json::from_value(serde_json::json!({"id":format!("p{i}"),"sourceId":info.id,"pageIndex":i,"width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})).unwrap()).collect();
    e.export_pdf(
        folio_engine::ExportRequest {
            pages,
            flatten: false,
        },
        path,
    )
    .unwrap();
}
#[test]
fn merged_text_checkbox_radio_and_choice_definitions_survive_save_and_reopen() {
    use serde_json::json;
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for (i, field) in [
        json!({"kind":"text","multiline":true}),
        json!({"kind":"checkbox"}),
        json!({"kind":"radio","options":["One","Two"]}),
        json!({"kind":"dropdown","options":["One","Two"]}),
        json!({"kind":"list","options":["One","Two"],"multiSelect":true}),
    ]
    .into_iter()
    .enumerate()
    {
        let blank = e.create_blank_document(400., 500.).unwrap();
        let made=e.create_form_field_occurrence(&blank.id,&[0],0,serde_json::from_value(json!({"bounds":{"x":30,"y":40,"width":180,"height":90},"rotation":0,"name":"field","label":"Original","required":false,"fontSize":12,"field":field})).unwrap()).unwrap();
        let before = e.list_form_fields(&made.document.id, 0).unwrap();
        let result = apply(
            &e,
            &made.document.id,
            FormPropertyChanges {
                name: Some("answer".into()),
                label: Some("Changed tooltip".into()),
                read_only: Some(true),
                required: Some(true),
            },
        );
        let path = temp.path().join(format!("kind-{i}.pdf"));
        export(&e, &result, &path);
        let reopened = e.open_document(&path).unwrap();
        let after = e.list_form_fields(&reopened.id, 0).unwrap();
        assert_eq!(before.len(), after.len());
        for (old, new) in before.iter().zip(&after) {
            assert_eq!(new.name, "answer");
            assert_eq!(new.label, "Changed tooltip");
            assert!(new.read_only && new.required);
            assert_eq!(old.value, new.value);
            assert_eq!(old.options, new.options);
            assert_eq!(old.bounds, new.bounds);
        }
        let unlocked = apply(
            &e,
            &reopened.id,
            FormPropertyChanges {
                read_only: Some(false),
                ..changes()
            },
        );
        assert!(e
            .list_form_fields(&unlocked.id, 0)
            .unwrap()
            .iter()
            .all(|f| !f.read_only && f.required));
        if i == 0 {
            let filled = e
                .fill_form_field(
                    &unlocked.id,
                    folio_engine::FillFormFieldRequest {
                        page_index: 0,
                        annotation_index: 0,
                        expected_value: after[0].value.clone(),
                        value: folio_engine::FormFieldValue::Text {
                            text: "Filled after unlocking".into(),
                        },
                    },
                )
                .unwrap();
            assert_eq!(
                e.inspect_form_properties(&filled.id, 0, 0).unwrap().name,
                "answer"
            );
            e.close_document(&filled.id).unwrap();
        }
        for id in [
            &blank.id,
            &made.document.id,
            &result.id,
            &reopened.id,
            &unlocked.id,
        ] {
            e.close_document(id).unwrap();
        }
    }
}
#[test]
fn protected_reader_cannot_change_definitions_and_owner_copy_keeps_credentials() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let encrypted = protected_fixture::encrypt(
        &std::fs::read(fixture()).unwrap(),
        lopdf::encryption::Permissions::FILLABLE,
    );
    let path = temp.path().join("protected.pdf");
    std::fs::write(&path, &encrypted).unwrap();
    let reader = e.open_document_with_password(&path, "reader").unwrap();
    assert!(e.inspect_form_properties(&reader.id, 0, 0).is_err());
    let owner = e.open_document_with_password(&path, "owner").unwrap();
    let old = e.inspect_form_properties(&owner.id, 0, 0).unwrap();
    let request = SetFormPropertiesRequest {
        page_index: 0,
        annotation_index: 0,
        expected_key: old.expected_key,
        changes: FormPropertyChanges {
            required: Some(true),
            ..changes()
        },
    };
    assert!(e.set_form_properties(&reader.id, request.clone()).is_err());
    let changed = e.set_form_properties(&owner.id, request).unwrap();
    let out = temp.path().join("changed.pdf");
    export(&e, &changed, &out);
    assert!(e.open_document(&out).is_err());
    let reopened = e.open_document_with_password(&out, "owner").unwrap();
    assert!(
        e.inspect_form_properties(&reopened.id, 0, 0)
            .unwrap()
            .required
    );
    let restricted = e.open_document_with_password(&out, "reader").unwrap();
    assert!(e.inspect_form_properties(&restricted.id, 0, 0).is_err());
    assert_eq!(std::fs::read(path).unwrap(), encrypted);
    for id in [
        &reader.id,
        &owner.id,
        &changed.id,
        &reopened.id,
        &restricted.id,
    ] {
        e.close_document(id).unwrap();
    }
}
