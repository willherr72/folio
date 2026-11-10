use folio_engine::{PdfEngine, RemoveFormFieldRequest};
use lopdf::{dictionary, Document, Object, ObjectId};
use std::path::{Path, PathBuf};
#[path = "support/protected_fixture.rs"]
mod protected_fixture;

fn export(e: &PdfEngine, info: &folio_engine::DocumentInfo, path: &Path) {
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

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}

#[test]
fn review_selected_alias_cannot_leave_a_canonical_widget_action_target() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut d = plain();
    let canonical = annotations(&d, 0)[1].as_reference().unwrap();
    assert!(!d.get_dictionary(canonical).unwrap().has(b"Parent"));
    let alias = d.add_object(Object::Reference(canonical));
    let mut f = form(&d).clone();
    let roots = f
        .get(b"Fields")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            if v.as_reference().unwrap() == canonical {
                Object::Reference(alias)
            } else {
                v.clone()
            }
        })
        .collect::<Vec<_>>();
    f.set("Fields", roots);
    let fid = d.add_object(f);
    let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
    d.get_dictionary_mut(root).unwrap().set("AcroForm", fid);
    let action = d.add_object(dictionary! {"S"=>"Hide","T"=>canonical});
    let link=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Link","Rect"=>vec![10.into(),10.into(),20.into(),20.into()],"A"=>action});
    let page = *d.get_pages().values().next().unwrap();
    let mut a = annotations(&d, 0);
    a[1] = alias.into();
    a.push(link.into());
    d.get_dictionary_mut(page).unwrap().set("Annots", a);
    let path = temp.path().join("selected-alias.pdf");
    save(&mut d, &path);
    let source = e.open_document(&path).unwrap();
    let info = e.inspect_form_removal(&source.id, 0, 1).unwrap();
    assert!(
        info.removal_reason.is_some(),
        "selected alias bypassed canonical action-target refusal"
    );
    assert!(e
        .remove_form_field(
            &source.id,
            RemoveFormFieldRequest {
                page_index: 0,
                annotation_index: 1,
                expected_key: info.expected_key
            }
        )
        .is_err());
    e.close_document(&source.id).unwrap();
}

#[test]
fn review_empty_ancestor_names_use_the_inspected_action_target_identity() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for intermediate in [false, true] {
        let mut d = plain();
        let target = owner(&d);
        let mut f = form(&d).clone();
        let roots = f.get(b"Fields").unwrap().as_array().unwrap().clone();
        let empty =
            d.add_object(dictionary! {"T"=>Object::string_literal(""),"Kids"=>roots.clone()});
        for child in &roots {
            d.get_dictionary_mut(child.as_reference().unwrap())
                .unwrap()
                .set("Parent", empty);
        }
        let root_field = if intermediate {
            let top=d.add_object(dictionary!{"T"=>Object::string_literal("section"),"Kids"=>vec![Object::Reference(empty)]});
            d.get_dictionary_mut(empty).unwrap().set("Parent", top);
            top
        } else {
            empty
        };
        f.set("Fields", vec![Object::Reference(root_field)]);
        let fid = d.add_object(f);
        let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
        d.get_dictionary_mut(root).unwrap().set("AcroForm", fid);
        let own = lopdf::decode_text_string(d.get_dictionary(target).unwrap().get(b"T").unwrap())
            .unwrap();
        let expected = if intermediate {
            format!("section..{own}")
        } else {
            own
        };
        let name = d.add_object(lopdf::text_string(&expected));
        let action =
            d.add_object(dictionary! {"S"=>"ResetForm","Fields"=>vec![Object::Reference(name)]});
        d.get_dictionary_mut(root)
            .unwrap()
            .set("FolioAction", action);
        let path = temp.path().join(format!("empty-name-{intermediate}.pdf"));
        save(&mut d, &path);
        let source = e.open_document(&path).unwrap();
        let info = e.inspect_form_removal(&source.id, 0, 0).unwrap();
        assert_eq!(info.full_name, expected);
        assert!(
            info.removal_reason.is_some(),
            "name guard used a different identity from inspection"
        );
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn review_direct_and_indirect_null_optional_arrays_do_not_block_removal() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for indirect in [false, true] {
        for co in [false, true] {
            let mut d = plain();
            let null = if indirect {
                Object::Reference(d.add_object(Object::Null))
            } else {
                Object::Null
            };
            if co {
                let mut f = form(&d).clone();
                f.set("CO", null);
                let fid = d.add_object(f);
                let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
                d.get_dictionary_mut(root).unwrap().set("AcroForm", fid);
            } else {
                let empty = *d.get_pages().values().nth(2).unwrap();
                d.get_dictionary_mut(empty).unwrap().set("Annots", null);
            }
            let path = temp.path().join(format!("null-{indirect}-{co}.pdf"));
            save(&mut d, &path);
            let source = e.open_document(&path).unwrap();
            let changed = remove(&e, &source.id);
            assert_eq!(e.list_form_fields(&changed.id, 0).unwrap().len(), 1);
            assert!(e.list_form_fields(&changed.id, 1).unwrap().is_empty());
            assert!(e.list_form_fields(&changed.id, 2).unwrap().is_empty());
            for id in [&source.id, &changed.id] {
                e.close_document(id).unwrap();
            }
        }
    }
}

#[test]
fn shared_arrays_are_copied_privately_or_refused_when_a_surviving_owner_needs_the_field() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for kind in ["Fields", "Kids", "Annots", "CO", "CO-safe"] {
        let mut d = plain();
        let target = owner(&d);
        let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let page = *d.get_pages().values().next().unwrap();
        let mut f = form(&d).clone();
        let fields = f.get(b"Fields").unwrap().as_array().unwrap().clone();
        let values = match kind {
            "Fields" => fields.clone(),
            "Kids" => d
                .get_dictionary(target)
                .unwrap()
                .get(b"Kids")
                .unwrap()
                .as_array()
                .unwrap()
                .clone(),
            "Annots" => annotations(&d, 0),
            "CO-safe" => fields
                .into_iter()
                .filter(|v| v.as_reference().unwrap() != target)
                .collect(),
            _ => vec![Object::Reference(target)],
        };
        let shared = d.add_object(values.clone());
        d.get_dictionary_mut(root)
            .unwrap()
            .set("PrivateArrayOwner", shared);
        match kind {
            "Fields" => f.set("Fields", shared),
            "Kids" => d.get_dictionary_mut(target).unwrap().set("Kids", shared),
            "Annots" => d.get_dictionary_mut(page).unwrap().set("Annots", shared),
            _ => f.set("CO", shared),
        }
        let fid = d.add_object(f);
        d.get_dictionary_mut(root).unwrap().set("AcroForm", fid);
        let path = temp.path().join(format!("shared-{kind}.pdf"));
        save(&mut d, &path);
        let source = e.open_document(&path).unwrap();
        let bytes = e.source_bytes(&source.id).unwrap();
        if kind == "CO-safe" {
            let changed = remove(&e, &source.id);
            let out = Document::load_mem(&e.source_bytes(&changed.id).unwrap()).unwrap();
            assert_eq!(out.get_object(shared).unwrap().as_array().unwrap(), &values);
            assert_eq!(form(&out).get(b"CO").unwrap().as_array().unwrap(), &values);
            e.close_document(&changed.id).unwrap();
        } else {
            let info = e.inspect_form_removal(&source.id, 0, 0).unwrap();
            assert!(
                info.removal_reason.is_some(),
                "retained {kind} reference was silently changed"
            );
            assert!(e
                .remove_form_field(
                    &source.id,
                    RemoveFormFieldRequest {
                        page_index: 0,
                        annotation_index: 0,
                        expected_key: info.expected_key
                    }
                )
                .is_err());
        }
        assert_eq!(e.source_bytes(&source.id).unwrap(), bytes);
        e.close_document(&source.id).unwrap();
    }
}
fn plain() -> Document {
    let mut d = Document::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/form-text-appearance/fields-r0-u1.pdf"),
    )
    .unwrap();
    for obj in d.objects.values_mut() {
        if let Ok(dict) = obj.as_dict_mut() {
            dict.remove(b"AA");
        }
    }
    d.prune_objects();
    d
}
fn annotations(d: &Document, page: usize) -> Vec<Object> {
    let p = d
        .get_dictionary(*d.get_pages().values().nth(page).unwrap())
        .unwrap();
    d.dereference(p.get(b"Annots").unwrap())
        .unwrap()
        .1
        .as_array()
        .unwrap()
        .clone()
}
fn owner(d: &Document) -> ObjectId {
    d.get_dictionary(annotations(d, 0)[0].as_reference().unwrap())
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap()
}
fn form(d: &Document) -> &lopdf::Dictionary {
    d.dereference(d.catalog().unwrap().get(b"AcroForm").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap()
}
fn save(d: &mut Document, path: &Path) {
    d.save(path).unwrap();
}
fn remove(e: &PdfEngine, id: &str) -> folio_engine::DocumentInfo {
    let info = e.inspect_form_removal(id, 0, 0).unwrap();
    assert_eq!(info.removal_reason, None);
    e.remove_form_field(
        id,
        RemoveFormFieldRequest {
            page_index: 0,
            annotation_index: 0,
            expected_key: info.expected_key,
        },
    )
    .unwrap()
}
#[test]
fn removes_linked_widgets_preserves_other_fields_and_original_then_reopens() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    let mut d = plain();
    save(&mut d, &path);
    let original = std::fs::read(&path).unwrap();
    let source = e.open_document(&path).unwrap();
    let info = e.inspect_form_removal(&source.id, 0, 0).unwrap();
    assert_eq!(info.linked_widgets, 2);
    assert_eq!(info.page_indices, vec![0, 1]);
    let before: Vec<_> = (0..2)
        .map(|p| e.list_form_fields(&source.id, p).unwrap())
        .collect();
    let changed = remove(&e, &source.id);
    assert_ne!(changed.id, source.id);
    let bytes = e.source_bytes(&changed.id).unwrap();
    let after = Document::load_mem(&bytes).unwrap();
    assert!(!after.objects.contains_key(&owner(&d)));
    for p in 0..2 {
        let fields = e.list_form_fields(&changed.id, p).unwrap();
        assert_eq!(fields.len(), before[p].len() - 1);
        for (old, new) in before[p].iter().skip(1).zip(&fields) {
            assert_eq!(old.name, new.name);
            assert_eq!(old.value, new.value);
            assert_eq!(old.bounds, new.bounds);
        }
        assert_eq!(annotations(&after, p), annotations(&d, p)[1..]);
        for reference in annotations(&after, p) {
            let id = reference.as_reference().unwrap();
            assert_eq!(after.get_object(id).unwrap(), d.get_object(id).unwrap());
        }
        let page = *d.get_pages().values().nth(p).unwrap();
        assert_eq!(d.get_page_content(page), after.get_page_content(page));
        assert_eq!(
            e.list_form_fields(&source.id, p).unwrap().len(),
            before[p].len()
        );
    }
    let out = temp.path().join("removed.pdf");
    export(&e, &changed, &out);
    let reopened = e.open_document(&out).unwrap();
    assert_eq!(
        e.list_form_fields(&reopened.id, 0).unwrap().len(),
        before[0].len() - 1
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
    if let Some(directory) = std::env::var_os("FOLIO_FORM_REMOVAL_OUTPUT_DIR") {
        let directory = PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("source.pdf"), &original).unwrap();
        std::fs::copy(&out, directory.join("removed.pdf")).unwrap();
    }
    for id in [&source.id, &changed.id, &reopened.id] {
        e.close_document(id).unwrap();
    }
}

#[test]
fn common_fields_including_entire_radio_groups_remove_and_export_empty_forms() {
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
        let old = e.list_form_fields(&made.document.id, 0).unwrap();
        let info = e.inspect_form_removal(&made.document.id, 0, 0).unwrap();
        assert_eq!(info.linked_widgets, old.len());
        assert_eq!(info.page_indices, vec![0]);
        let changed = remove(&e, &made.document.id);
        assert!(e.list_form_fields(&changed.id, 0).unwrap().is_empty());
        let path = temp.path().join(format!("empty-{i}.pdf"));
        export(&e, &changed, &path);
        let reopened = e.open_document(&path).unwrap();
        assert!(e.list_form_fields(&reopened.id, 0).unwrap().is_empty());
        assert_eq!(
            e.list_form_fields(&made.document.id, 0).unwrap().len(),
            old.len()
        );
        for id in [&blank.id, &made.document.id, &changed.id, &reopened.id] {
            e.close_document(id).unwrap();
        }
    }
}

#[test]
fn fill_only_permission_refuses_removal_and_owner_output_preserves_passwords() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut d = plain();
    let mut plain_bytes = Vec::new();
    d.save_to(&mut plain_bytes).unwrap();
    let encrypted =
        protected_fixture::encrypt(&plain_bytes, lopdf::encryption::Permissions::FILLABLE);
    let path = temp.path().join("protected.pdf");
    std::fs::write(&path, &encrypted).unwrap();
    let reader = e.open_document_with_password(&path, "reader").unwrap();
    assert!(e.inspect_form_removal(&reader.id, 0, 0).is_err());
    let owner = e.open_document_with_password(&path, "owner").unwrap();
    let info = e.inspect_form_removal(&owner.id, 0, 0).unwrap();
    assert!(e
        .remove_form_field(
            &reader.id,
            RemoveFormFieldRequest {
                page_index: 0,
                annotation_index: 0,
                expected_key: info.expected_key
            }
        )
        .is_err());
    let changed = remove(&e, &owner.id);
    let out = temp.path().join("protected-removed.pdf");
    export(&e, &changed, &out);
    assert!(e.open_document(&out).is_err());
    let reopened = e.open_document_with_password(&out, "owner").unwrap();
    let restricted = e.open_document_with_password(&out, "reader").unwrap();
    assert_eq!(
        e.list_form_fields(&owner.id, 0).unwrap().len() - 1,
        e.list_form_fields(&reopened.id, 0).unwrap().len()
    );
    assert!(e.inspect_form_removal(&restricted.id, 0, 0).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), encrypted);
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

#[test]
fn references_through_aliases_and_name_arrays_refuse_without_mutating_the_source() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for named in [false, true] {
        let mut d = plain();
        let target = owner(&d);
        let value = if named {
            d.get_dictionary(target).unwrap().get(b"T").unwrap().clone()
        } else {
            Object::Reference(target)
        };
        let alias = d.add_object(value);
        let targets = d.add_object(vec![Object::Reference(alias)]);
        let action=d.add_object(dictionary!{"S"=>"SubmitForm","Fields"=>targets,"F"=>Object::string_literal("https://example.test/submit")});
        let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
        d.get_dictionary_mut(root)
            .unwrap()
            .set("FolioAction", action);
        let path = temp.path().join(format!("alias-{named}.pdf"));
        save(&mut d, &path);
        let source = e.open_document(&path).unwrap();
        let bytes = e.source_bytes(&source.id).unwrap();
        let info = e.inspect_form_removal(&source.id, 0, 0).unwrap();
        assert!(info.removal_reason.is_some());
        assert!(e
            .remove_form_field(
                &source.id,
                RemoveFormFieldRequest {
                    page_index: 0,
                    annotation_index: 0,
                    expected_key: info.expected_key
                }
            )
            .is_err());
        assert_eq!(e.source_bytes(&source.id).unwrap(), bytes);
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn retained_parent_comments_and_global_form_actions_keep_their_identity() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut d = plain();
    let target = owner(&d);
    let mut f = form(&d).clone();
    let roots = f.get(b"Fields").unwrap().as_array().unwrap().clone();
    let parent =
        d.add_object(dictionary! {"T"=>Object::string_literal("section"),"Kids"=>roots.clone()});
    for child in &roots {
        d.get_dictionary_mut(child.as_reference().unwrap())
            .unwrap()
            .set("Parent", parent);
    }
    f.set("Fields", vec![Object::Reference(parent)]);
    let form_id = d.add_object(f);
    let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
    d.get_dictionary_mut(root).unwrap().set("AcroForm", form_id);
    let action = d.add_object(dictionary! {"S"=>"ResetForm"});
    d.get_dictionary_mut(root)
        .unwrap()
        .set("FolioAction", action);
    let comment=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![20.into(),20.into(),40.into(),40.into()],"Contents"=>Object::string_literal("Keep this comment")});
    let page = *d.get_pages().values().next().unwrap();
    let mut a = annotations(&d, 0);
    a.push(comment.into());
    d.get_dictionary_mut(page).unwrap().set("Annots", a);
    let path = temp.path().join("retained.pdf");
    save(&mut d, &path);
    let source = e.open_document(&path).unwrap();
    let changed = remove(&e, &source.id);
    let out = Document::load_mem(&e.source_bytes(&changed.id).unwrap()).unwrap();
    assert_eq!(
        out.get_dictionary(parent)
            .unwrap()
            .get(b"Kids")
            .unwrap()
            .as_array()
            .unwrap(),
        &roots
            .into_iter()
            .filter(|v| v.as_reference().unwrap() != target)
            .collect::<Vec<_>>()
    );
    for id in [comment, action] {
        assert_eq!(out.get_object(id).unwrap(), d.get_object(id).unwrap());
    }
    e.close_document(&source.id).unwrap();
    e.close_document(&changed.id).unwrap();
}
#[test]
fn nested_empty_parent_and_calculation_order_are_pruned_without_touching_siblings() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut d = plain();
    let target = owner(&d);
    let parent=d.add_object(dictionary!{"T"=>Object::string_literal("section"),"Kids"=>vec![Object::Reference(target)]});
    d.get_dictionary_mut(target).unwrap().set("Parent", parent);
    let mut f = form(&d).clone();
    let roots = f.get(b"Fields").unwrap().as_array().unwrap().clone();
    let sibling = roots
        .iter()
        .find(|r| r.as_reference().unwrap() != target)
        .unwrap()
        .clone();
    let new_roots: Vec<_> = roots
        .into_iter()
        .map(|r| {
            if r.as_reference().unwrap() == target {
                Object::Reference(parent)
            } else {
                r
            }
        })
        .collect();
    let roots_id = d.add_object(new_roots);
    f.set("Fields", roots_id);
    let co = d.add_object(vec![Object::Reference(target), sibling.clone()]);
    f.set("CO", co);
    let form_id = d.add_object(f);
    let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
    d.get_dictionary_mut(root).unwrap().set("AcroForm", form_id);
    for page in d.get_pages().values().copied().collect::<Vec<_>>() {
        if let Ok(annots) = d.get_dictionary(page).unwrap().get(b"Annots").cloned() {
            let id = d.add_object(annots);
            d.get_dictionary_mut(page).unwrap().set("Annots", id);
        }
    }
    let path = temp.path().join("nested.pdf");
    save(&mut d, &path);
    let source = e.open_document(&path).unwrap();
    let changed = remove(&e, &source.id);
    let out = Document::load_mem(&e.source_bytes(&changed.id).unwrap()).unwrap();
    assert!(!out.objects.contains_key(&parent));
    assert!(!out.objects.contains_key(&target));
    assert_eq!(
        form(&out).get(b"CO").unwrap().as_array().unwrap(),
        &vec![sibling]
    );
    e.close_document(&source.id).unwrap();
    e.close_document(&changed.id).unwrap();
}
#[test]
fn stale_keys_and_actions_referencing_removed_fields_are_refused() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for mode in 0..5 {
        let mut d = plain();
        let target = owner(&d);
        let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let name = d.get_dictionary(target).unwrap().get(b"T").unwrap().clone();
        let action = match mode {
            0 => dictionary! {"S"=>"ResetForm","Fields"=>vec![name]},
            1 => dictionary! {"S"=>"Hide","T"=>target},
            2 => dictionary! {"S"=>"JavaScript","JS"=>Object::string_literal("app.alert('test')")},
            3 => dictionary! {"PrivateReference"=>target},
            _ => dictionary! {"S"=>"URI","URI"=>Object::string_literal("https://example.test")},
        };
        let a = d.add_object(action);
        d.get_dictionary_mut(root).unwrap().set("FolioAction", a);
        let path = temp.path().join(format!("mode-{mode}.pdf"));
        save(&mut d, &path);
        let source = e.open_document(&path).unwrap();
        let info = e.inspect_form_removal(&source.id, 0, 0).unwrap();
        if mode < 4 {
            assert!(info.removal_reason.is_some(), "mode {mode}");
            assert!(e
                .remove_form_field(
                    &source.id,
                    RemoveFormFieldRequest {
                        page_index: 0,
                        annotation_index: 0,
                        expected_key: info.expected_key
                    }
                )
                .is_err());
        } else {
            assert_eq!(info.removal_reason, None);
            assert!(e
                .remove_form_field(
                    &source.id,
                    RemoveFormFieldRequest {
                        page_index: 0,
                        annotation_index: 0,
                        expected_key: "stale".into()
                    }
                )
                .is_err());
            let out = remove(&e, &source.id);
            let result = Document::load_mem(&e.source_bytes(&out.id).unwrap()).unwrap();
            assert_eq!(result.get_object(a).unwrap(), d.get_object(a).unwrap());
            e.close_document(&out.id).unwrap();
        }
        e.close_document(&source.id).unwrap();
    }
}
