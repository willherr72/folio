use super::*;
use lopdf::{dictionary, Stream};

fn fixture() -> (Document, ObjectId, ObjectId, ObjectId, ObjectId) {
    let mut pdf = Document::with_version("1.7");
    let tree = pdf.new_object_id();
    let page = pdf.new_object_id();
    let annotation = pdf.new_object_id();
    let font =
        pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let ap=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),50.into(),12.into()]},b"BT /F1 10 Tf (Appearance) Tj ET".to_vec()));
    let popup=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Popup","Parent"=>annotation,"P"=>page,"Rect"=>vec![0.into(),0.into(),10.into(),10.into()]});
    let first = dictionary! {"Type"=>"Annot","Subtype"=>"Highlight","P"=>page,"Popup"=>popup,"AP"=>dictionary!{"N"=>ap},
    "Rect"=>vec![20.into(),280.into(),70.into(),292.into()],"NM"=>Object::string_literal("same-nm"),"Contents"=>Object::string_literal("original")};
    let mut direct = first.clone();
    direct.remove(b"Popup");
    direct.set("Contents", Object::string_literal("second"));
    pdf.objects.insert(annotation, first.into());
    let content = pdf.add_object(Stream::new(
        dictionary! {},
        b"BT /F1 12 Tf (Old text) Tj ET".to_vec(),
    ));
    pdf.objects.insert(page,dictionary!{"Type"=>"Page","Parent"=>tree,"Contents"=>content,
        "Annots"=>vec![Object::Reference(annotation),Object::Reference(popup),Object::Dictionary(direct)]}.into());
    pdf.objects.insert(tree,dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1,
        "MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}}.into());
    let catalog = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree});
    pdf.trailer.set("Root", catalog);
    (pdf, page, annotation, popup, ap)
}
fn index(pdf: &Document, annotation: ObjectId, prefix: &str) -> TestIndex {
    let mut pdf = pdf.clone();
    let source = crate::source_storage::SourceStore::new().capture(crate::pdf_output::save(&mut pdf).unwrap().into()).unwrap();
    let mut index = TestIndex { index: Index::capturing(source.identity().clone()), source };
    let overlay = |id: &str| {
        Overlay::Highlight(HighlightOverlay {
            id: id.into(),
            rects: vec![AnnotationRect {
                x: 20.,
                y: 108.,
                width: 50.,
                height: 12.,
            }],
            color: "#FFFF00".into(),
            text: Some("original".into()),
            opacity: None,
        })
    };
    assert!(index.capture(
        0,
        0,
        Some(annotation),
        &[1],
        &overlay(&format!("{prefix}-first"))
    ));
    assert!(index.capture(0, 2, None, &[], &overlay(&format!("{prefix}-second"))));
    index
}

#[test]
fn imported_overlay_sources_transfer_retains_roots_direct_slots_popups_and_materialized_ap_context()
{
    let (pdf, page, annotation, _, _) = fixture();
    let old = index(&pdf, annotation, "old");
    let before = old.snapshot().unwrap();
    let mut edited = pdf.clone();
    crate::engine::page_content_replace::retain_appearance_contexts(&mut edited, page).unwrap();
    let new_content =
        edited.add_object(Stream::new(dictionary! {}, b"q 0 0 10 10 re f Q".to_vec()));
    edited
        .get_dictionary_mut(page)
        .unwrap()
        .set("Contents", new_content);
    edited
        .get_dictionary_mut(page)
        .unwrap()
        .set("Resources", dictionary! {});
    let next = index(&edited, annotation, "new");
    let saved = old.retained_snapshot(&next).unwrap();
    assert_eq!(saved.bindings, before.bindings);
    assert_ne!(saved.source_sha256, before.source_sha256);
    let (rebound, renames) = next.restore(saved).unwrap();
    assert_eq!(rebound.snapshot().unwrap().bindings, before.bindings);
    assert_eq!(renames.len(), 2);
    assert_eq!(old.snapshot().unwrap(), before);
}

#[test]
fn imported_overlay_sources_transfer_rejects_appearance_metadata_resource_and_popup_changes() {
    let (pdf, page, annotation, popup, ap) = fixture();
    let old = index(&pdf, annotation, "old");
    for case in [
        "paint", "font", "popup", "note", "direct", "identity", "missing",
    ] {
        let mut edited = pdf.clone();
        let mut next_annotation = annotation;
        match case {
            "paint" => {
                edited
                    .get_object_mut(ap)
                    .unwrap()
                    .as_stream_mut()
                    .unwrap()
                    .content = b"0 0 1 rg 0 0 50 12 re f".to_vec()
            }
            "font" => {
                let resources = crate::page_resources::inherited_page_resources(&edited, page)
                    .unwrap()
                    .unwrap()
                    .as_dict()
                    .unwrap();
                let font = resources
                    .get(b"Font")
                    .unwrap()
                    .as_dict()
                    .unwrap()
                    .get(b"F1")
                    .unwrap()
                    .as_reference()
                    .unwrap();
                edited
                    .get_dictionary_mut(font)
                    .unwrap()
                    .set("BaseFont", "Courier");
            }
            "popup" => {
                edited
                    .get_dictionary_mut(popup)
                    .unwrap()
                    .set("Parent", page);
            }
            "note" => {
                edited
                    .get_dictionary_mut(annotation)
                    .unwrap()
                    .set("Contents", Object::string_literal("changed"));
            }
            "direct" => {
                edited
                    .get_dictionary_mut(page)
                    .unwrap()
                    .get_mut(b"Annots")
                    .unwrap()
                    .as_array_mut()
                    .unwrap()[2]
                    .as_dict_mut()
                    .unwrap()
                    .set("Contents", Object::string_literal("changed"));
            }
            "identity" => {
                next_annotation =
                    edited.add_object(edited.get_dictionary(annotation).unwrap().clone());
                edited
                    .get_dictionary_mut(page)
                    .unwrap()
                    .get_mut(b"Annots")
                    .unwrap()
                    .as_array_mut()
                    .unwrap()[0] = next_annotation.into();
            }
            "missing" => {
                edited
                    .get_dictionary_mut(page)
                    .unwrap()
                    .get_mut(b"Annots")
                    .unwrap()
                    .as_array_mut()
                    .unwrap()
                    .pop();
            }
            _ => unreachable!(),
        }
        // Deliberately hold the imported semantic hashes constant: they cannot
        // by themselves authorize an altered native graph or appearance.
        let next = index(&edited, next_annotation, "new");
        assert!(old.retained_snapshot(&next).is_err(), "accepted {case}");
    }
}

#[test]
fn imported_overlay_sources_transfer_rejects_changed_physical_page_context() {
    let (pdf, page, annotation, _, _) = fixture();
    let old = index(&pdf, annotation, "old");
    for (key, value) in [
        ("UserUnit", Object::Real(1.7)),
        ("Rotate", Object::Integer(90)),
        (
            "MediaBox",
            vec![0.into(), 0.into(), 600.into(), 400.into()].into(),
        ),
        (
            "CropBox",
            vec![10.into(), 10.into(), 290.into(), 390.into()].into(),
        ),
    ] {
        let mut edited = pdf.clone();
        edited.get_dictionary_mut(page).unwrap().set(key, value);
        assert!(
            old.retained_snapshot(&index(&edited, annotation, "new"))
                .is_err(),
            "accepted {key}"
        );
    }
}

#[test]
fn imported_overlay_sources_transfer_does_not_promote_legacy_or_unproved_copies() {
    let (pdf, page, annotation, _, _) = fixture();
    let mut old = index(&pdf, annotation, "old");
    let next = index(&pdf, annotation, "new");
    old.trusted = false;
    assert!(old.retained_snapshot(&next).is_err());
    old.trusted = true;
    let mut copy = pdf.clone();
    let tree = copy
        .get_dictionary(page)
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    let new_page = copy.add_object(copy.get_dictionary(page).unwrap().clone());
    copy.get_dictionary_mut(tree).unwrap().set(
        "Kids",
        vec![Object::Reference(page), Object::Reference(new_page)],
    );
    copy.get_dictionary_mut(tree).unwrap().set("Count", 2);
    assert!(old
        .retained_snapshot(&index(&copy, annotation, "new"))
        .is_err());
}

#[test]
fn imported_overlay_sources_transfer_refuses_equal_but_redirected_metadata_references() {
    let (mut pdf, _, annotation, _, _) = fixture();
    let reference = pdf.add_object(Object::string_literal("opaque relationship"));
    pdf.get_dictionary_mut(annotation)
        .unwrap()
        .set("CustomRelationship", reference);
    let old = index(&pdf, annotation, "old");
    let mut changed = pdf.clone();
    let other = changed.add_object(changed.objects[&reference].clone());
    changed
        .get_dictionary_mut(annotation)
        .unwrap()
        .set("CustomRelationship", other);
    assert!(old
        .retained_snapshot(&index(&changed, annotation, "new"))
        .is_err());
}

#[test]
fn imported_overlay_sources_transfer_bounds_aliases_payload_metadata_and_cancellation() {
    let (mut pdf, _, annotation, _, _) = fixture();
    let old = index(&pdf, annotation, "old");
    let before = old.snapshot().unwrap();
    assert!(old
        .retained_snapshot_controlled(&index(&pdf, annotation, "new"), Arc::new(|| true))
        .is_err());
    assert_eq!(old.snapshot().unwrap(), before);
    let first = pdf.add_object(Object::Integer(1));
    let second = pdf.add_object(Object::Integer(2));
    let refs = Object::Array(vec![first.into(), second.into()]);
    let empty = Object::Dictionary(Dictionary::new());
    let pages = BTreeSet::new();
    for case in ["metadata", "work", "payload", "cancel", "depth"] {
        let mut budget = Budget {
            work: 0,
            bytes: 0,
            metadata: 0,
            cancelled: Arc::new(|| false),
        };
        match case {
            "metadata" => budget.metadata = MAX_METADATA - 256,
            "work" => budget.work = MAX_WORK - 1,
            "payload" => budget.bytes = 256 * 1024 * 1024 - 2,
            "cancel" => budget.cancelled = Arc::new(|| true),
            _ => {}
        }
        let mut proof = Proof {
            old: &pdf,
            new: &pdf,
            pages: &pages,
            budget: &mut budget,
            seen: BTreeSet::new(),
            old_resources: &empty,
            new_resources: &empty,
            copy: None,
        };
        let payload = Object::string_literal("too many bytes");
        let value = if case == "payload" { &payload } else { &refs };
        assert!(
            proof
                .value(
                    value,
                    value,
                    Mode::Exact,
                    if case == "depth" { 65 } else { 0 }
                )
                .is_err(),
            "accepted {case}"
        );
        if case == "metadata" {
            assert_eq!(
                proof.seen.len(),
                1,
                "reject before retaining the second reference pair"
            );
        }
    }
    let mut cycle = pdf.clone();
    cycle.objects.insert(first, Object::Reference(second));
    cycle.objects.insert(second, Object::Reference(first));
    cycle
        .get_dictionary_mut(annotation)
        .unwrap()
        .set("CustomRelationship", first);
    // Cyclic aliases are bounded even when reached as untrusted annotation metadata.
    let same = index(&cycle, annotation, "same");
    assert!(same.retained_snapshot(&same).is_err());
}

#[test]
fn imported_overlay_sources_transfer_refuses_ambiguous_or_incomplete_page_trees() {
    let (pdf, page, annotation, _, _) = fixture();
    for case in ["duplicate", "count", "missing"] {
        let mut changed = pdf.clone();
        let tree = changed
            .get_dictionary(page)
            .unwrap()
            .get(b"Parent")
            .unwrap()
            .as_reference()
            .unwrap();
        match case {
            "duplicate" => {
                changed.get_dictionary_mut(tree).unwrap().set(
                    "Kids",
                    vec![Object::Reference(page), Object::Reference(page)],
                );
                changed.get_dictionary_mut(tree).unwrap().set("Count", 2);
            }
            "count" => {
                changed
                    .get_dictionary_mut(tree)
                    .unwrap()
                    .set("Count", 500_000);
            }
            "missing" => {
                changed.get_dictionary_mut(tree).unwrap().set(
                    "Kids",
                    vec![Object::Reference(page), Object::Reference((9999, 0))],
                );
            }
            _ => unreachable!(),
        }
        let same = index(&changed, annotation, "same");
        assert!(same.retained_snapshot(&same).is_err(), "accepted {case}");
    }
}

fn widget_removal_indexes() -> (TestIndex, TestIndex, Document, ObjectId, ObjectId) {
    let (pdf, page, annotation, _, _) = fixture();
    let next = index(&pdf, annotation, "new");
    let mut before = pdf.clone();
    let widget = before.add_object(dictionary! {"Type"=>"Annot","Subtype"=>"Widget","P"=>page});
    before
        .get_dictionary_mut(page)
        .unwrap()
        .get_mut(b"Annots")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .insert(0, widget.into());
    let mut old = index(&before, annotation, "old");
    for binding in &mut old.bindings {
        binding.annotation_index += 1;
        for slot in &mut binding.popup_indices {
            *slot += 1;
        }
    }
    (old, next, pdf, page, annotation)
}

#[test]
fn imported_overlay_sources_widget_removal_requires_exact_bounded_receipt() {
    let (old, next, _, _, _) = widget_removal_indexes();
    let saved = old
        .retained_snapshot_transfer(&next, Some(&[(0, 0)]), Arc::new(|| false))
        .unwrap();
    assert_eq!(saved.bindings[0].overlay_id, "old-first");
    assert_eq!(saved.bindings[0].annotation_index, 0);
    assert_eq!(saved.bindings[0].popup_indices, vec![1]);
    assert_eq!(saved.bindings[1].annotation_index, 2);
    assert!(old.retained_snapshot(&next).is_err());
    for receipt in [
        vec![],
        vec![(0, 1)],
        vec![(0, 0), (0, 0)],
        vec![(0, 1), (0, 0)],
        vec![(1, 0)],
        vec![(0, 10_000)],
        vec![(usize::MAX, 0)],
        vec![(0, 0), (0, 3)],
    ] {
        assert!(
            old.retained_snapshot_transfer(&next, Some(&receipt), Arc::new(|| false))
                .is_err(),
            "accepted {receipt:?}"
        );
    }
    assert!(old
        .retained_snapshot_transfer(&next, Some(&[(0, 0)]), Arc::new(|| true))
        .is_err());
    let mut untrusted = old;
    untrusted.trusted = false;
    assert!(untrusted
        .retained_snapshot_transfer(&next, Some(&[(0, 0)]), Arc::new(|| false))
        .is_err());
}

#[test]
fn imported_overlay_sources_widget_receipt_does_not_authorize_owned_graph_changes() {
    let (_, _, pdf, page, annotation) = widget_removal_indexes();
    for case in ["reorder", "direct", "popup", "paint", "non-widget"] {
        let mut changed = pdf.clone();
        let (mut original, _, _, _, _) = widget_removal_indexes();
        match case {
            "reorder" => changed
                .get_dictionary_mut(page)
                .unwrap()
                .get_mut(b"Annots")
                .unwrap()
                .as_array_mut()
                .unwrap()
                .swap(0, 1),
            "direct" => changed
                .get_dictionary_mut(page)
                .unwrap()
                .get_mut(b"Annots")
                .unwrap()
                .as_array_mut()
                .unwrap()[2]
                .as_dict_mut()
                .unwrap()
                .set("Contents", Object::string_literal("forged")),
            "popup" => {
                let popup = changed
                    .get_dictionary(annotation)
                    .unwrap()
                    .get(b"Popup")
                    .unwrap()
                    .as_reference()
                    .unwrap();
                changed
                    .get_dictionary_mut(popup)
                    .unwrap()
                    .set("Parent", page);
            }
            "paint" => {
                let ap = changed
                    .get_dictionary(annotation)
                    .unwrap()
                    .get(b"AP")
                    .unwrap()
                    .as_dict()
                    .unwrap()
                    .get(b"N")
                    .unwrap()
                    .as_reference()
                    .unwrap();
                changed
                    .get_object_mut(ap)
                    .unwrap()
                    .as_stream_mut()
                    .unwrap()
                    .content = b"0 0 10 10 re f".to_vec();
            }
            "non-widget" => {
                let mut before = Document::load_mem(&original.source.lease().unwrap()).unwrap();
                let widget = before
                    .get_dictionary(page)
                    .unwrap()
                    .get(b"Annots")
                    .unwrap()
                    .as_array()
                    .unwrap()[0]
                    .as_reference()
                    .unwrap();
                before
                    .get_dictionary_mut(widget)
                    .unwrap()
                    .set("Subtype", "Highlight");
                original.replace_source(crate::pdf_output::save(&mut before).unwrap());
            }
            _ => unreachable!(),
        }
        assert!(
            original
                .retained_snapshot_transfer(
                    &index(&changed, annotation, "new"),
                    Some(&[(0, 0)]),
                    Arc::new(|| false)
                )
                .is_err(),
            "accepted {case}"
        );
    }
}

#[test]
fn imported_overlay_sources_native_comment_receipts_are_typed_and_cannot_remove_owned_popups() {
    for direct in [false, true] {
        for subtype in ["Text", "Popup", "Widget", "Highlight"] {
            let (mut old, next, _, page, _) = widget_removal_indexes();
            let mut before = Document::load_mem(&old.source.lease().unwrap()).unwrap();
            let removed = before
                .get_dictionary(page)
                .unwrap()
                .get(b"Annots")
                .unwrap()
                .as_array()
                .unwrap()[0]
                .as_reference()
                .unwrap();
            before
                .get_dictionary_mut(removed)
                .unwrap()
                .set("Subtype", subtype);
            if direct {
                let dict = before.get_dictionary(removed).unwrap().clone();
                before
                    .get_dictionary_mut(page)
                    .unwrap()
                    .get_mut(b"Annots")
                    .unwrap()
                    .as_array_mut()
                    .unwrap()[0] = dict.into();
            }
            old.replace_source(crate::pdf_output::save(&mut before).unwrap());
            let accepted = old.retained_snapshot_after_removal(
                &next,
                Some((RemovalKind::NativeComments, &[(0, 0)])),
                Arc::new(|| false),
            );
            assert_eq!(
                accepted.is_ok(),
                subtype == "Text" || (!direct && subtype == "Popup"),
                "{direct}/{subtype}"
            );
            if subtype == "Text" {
                assert!(old
                    .retained_snapshot_transfer(&next, Some(&[(0, 0)]), Arc::new(|| false))
                    .is_err());
                for receipt in [
                    vec![],
                    vec![(0, 0), (0, 0)],
                    vec![(1, 0)],
                    vec![(0, 1)],
                    vec![(0, 0), (0, 2)],
                ] {
                    assert!(old
                        .retained_snapshot_after_removal(
                            &next,
                            Some((RemovalKind::NativeComments, &receipt)),
                            Arc::new(|| false)
                        )
                        .is_err());
                }
            }
        }
    }
    let (mut old, _, mut after, page, annotation) = widget_removal_indexes();
    let mut before = Document::load_mem(&old.source.lease().unwrap()).unwrap();
    let note = before
        .get_dictionary(page)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .as_reference()
        .unwrap();
    before
        .get_dictionary_mut(note)
        .unwrap()
        .set("Subtype", "Text");
    old.replace_source(crate::pdf_output::save(&mut before).unwrap());
    after
        .get_dictionary_mut(page)
        .unwrap()
        .get_mut(b"Annots")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .remove(1);
    let mut next = index(&after, annotation, "new");
    next.bindings[0].popup_indices.clear();
    next.bindings[1].annotation_index = 1;
    assert!(
        old.retained_snapshot_after_removal(
            &next,
            Some((RemovalKind::NativeComments, &[(0, 0), (0, 2)])),
            Arc::new(|| false)
        )
        .is_err(),
        "a native-comment receipt cannot consume an owned highlight popup"
    );
}

#[test]
fn imported_overlay_sources_copy_edges_cannot_keep_original_owners() {
    let (pdf, page, annotation, _, _) = fixture();
    let mut new = pdf.clone();
    let copy = new.add_object(pdf.get_dictionary(annotation).unwrap().clone());
    let copied_page = new.add_object(pdf.get_dictionary(page).unwrap().clone());
    let pages = BTreeSet::from([page]);
    let empty = Object::Dictionary(Dictionary::new());
    let mut budget = Budget {
        work: 0,
        bytes: 0,
        metadata: 0,
        cancelled: Arc::new(|| false),
    };
    let mut proof = Proof {
        old: &pdf,
        new: &new,
        pages: &pages,
        budget: &mut budget,
        seen: BTreeSet::new(),
        old_resources: &empty,
        new_resources: &empty,
        copy: Some(CopyContext {
            old_page: page,
            new_page: copied_page,
            annotations: BTreeMap::from([(annotation, copy)]),
            target_roots: BTreeSet::from([copy]),
            resources: BTreeMap::new(),
            reverse_resources: BTreeMap::new(),
        }),
    };
    assert!(
        proof
            .value(
                &Object::Reference(annotation),
                &Object::Reference(annotation),
                Mode::Exact,
                0
            )
            .is_err(),
        "a copied ownership edge cannot refer back to the original annotation"
    );
}

// Build a copied graph independently of PDFium, including relocated font/AP
// resources and materialization of the original direct annotation.
fn copied_fixture() -> (
    Document,
    Document,
    ObjectId,
    ObjectId,
    ObjectId,
    ObjectId,
    ObjectId,
    ObjectId,
) {
    let (mut old, page, owner, popup, ap) = fixture();
    // Keep serialization from allocating an xref-stream object in the same ID
    // slot as this independently constructed copied page.
    old.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
    let mut new = old.clone();
    let copied_page = new.new_object_id();
    let copied_owner = new.new_object_id();
    let copied_popup = new.new_object_id();
    let font =
        new.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let mut appearance = old.get_object(ap).unwrap().as_stream().unwrap().clone();
    appearance
        .dict
        .set("Resources", dictionary! {"Font"=>dictionary!{"F1"=>font}});
    let copied_ap = new.add_object(appearance);
    let mut annotation = old.get_dictionary(owner).unwrap().clone();
    annotation.set("P", copied_page);
    annotation.set("Popup", copied_popup);
    annotation.set("AP", dictionary! {"N"=>copied_ap});
    new.objects.insert(copied_owner, annotation.into());
    let mut popup_dict = old.get_dictionary(popup).unwrap().clone();
    popup_dict.set("P", copied_page);
    popup_dict.set("Parent", copied_owner);
    new.objects.insert(copied_popup, popup_dict.into());
    let mut page_dict = old.get_dictionary(page).unwrap().clone();
    let mut direct = page_dict.get(b"Annots").unwrap().as_array().unwrap()[2]
        .as_dict()
        .unwrap()
        .clone();
    direct.set("P", copied_page);
    direct.set("AP", dictionary! {"N"=>copied_ap});
    let direct_id = new.add_object(direct);
    page_dict.set(
        "Annots",
        vec![
            Object::Reference(copied_owner),
            copied_popup.into(),
            direct_id.into(),
        ],
    );
    let tree = page_dict.get(b"Parent").unwrap().as_reference().unwrap();
    new.objects.insert(copied_page, page_dict.into());
    new.get_dictionary_mut(tree)
        .unwrap()
        .set("Kids", vec![Object::Reference(page), copied_page.into()]);
    new.get_dictionary_mut(tree).unwrap().set("Count", 2);
    (
        old,
        new,
        owner,
        copied_page,
        copied_owner,
        copied_popup,
        copied_ap,
        font,
    )
}
fn copied_index(pdf: &Document, owner: ObjectId) -> TestIndex {
    let mut next = index(pdf, owner, "next");
    let page = pdf.get_pages()[&2];
    let annots = pdf
        .get_dictionary(page)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap();
    for (slot, label, popups) in [(0, "copied-first", vec![1]), (2, "copied-second", vec![])] {
        let overlay = Overlay::Highlight(HighlightOverlay {
            id: label.into(),
            rects: vec![AnnotationRect {
                x: 20.,
                y: 108.,
                width: 50.,
                height: 12.,
            }],
            color: "#FFFF00".into(),
            text: Some("original".into()),
            opacity: None,
        });
        assert!(next.capture(
            1,
            slot,
            pdf.dereference(&annots[slot]).unwrap().0,
            &popups,
            &overlay
        ));
    }
    next
}
#[test]
fn imported_overlay_sources_copy_proves_fresh_roots_popups_materialized_direct_and_resource_closures(
) {
    let (old, new, owner, _, copied_owner, _, _, _) = copied_fixture();
    let old = index(&old, owner, "old");
    let next = copied_index(&new, owner);
    assert!(
        old.retained_snapshot(&next).is_err(),
        "generic replacement cannot authorize copying"
    );
    let saved = old
        .retained_snapshot_transaction(&next, None, Some(0), Arc::new(|| false))
        .unwrap();
    assert_eq!(saved.bindings.len(), 4);
    for i in 0..2 {
        assert_eq!(
            saved.bindings[i].overlay_id,
            saved.bindings[i + 2].overlay_id
        );
    }
    assert_eq!(saved.bindings[2].annotation_id, Some(copied_owner));
    let (restored, renames) = next.restore(saved.clone()).unwrap();
    assert_eq!(restored.snapshot().unwrap(), saved);
    assert_eq!(renames.len(), 4);
}
#[test]
fn imported_overlay_sources_copy_refuses_changed_or_ambiguous_graphs() {
    for case in [
        "popup-original",
        "popup-other",
        "paint",
        "font",
        "page",
        "semantics",
        "reordered",
        "aliased-root",
        "tagged",
        "missing-page",
        "original-changed",
        "wrong-origin",
        "budget",
        "cancel",
        "legacy",
    ] {
        let (old, mut new, owner, page, copied_owner, popup, ap, font) = copied_fixture();
        match case {
            "popup-original" => new.get_dictionary_mut(popup).unwrap().set("Parent", owner),
            "popup-other" => {
                let other = new.add_object(new.get_dictionary(copied_owner).unwrap().clone());
                new.get_dictionary_mut(popup).unwrap().set("Parent", other);
            }
            "paint" => {
                new.get_object_mut(ap)
                    .unwrap()
                    .as_stream_mut()
                    .unwrap()
                    .content = b"0 0 1 rg 0 0 50 12 re f".to_vec()
            }
            "font" => new
                .get_dictionary_mut(font)
                .unwrap()
                .set("BaseFont", "Courier"),
            "page" => new.get_dictionary_mut(page).unwrap().set("UserUnit", 2),
            "semantics" => new
                .get_dictionary_mut(copied_owner)
                .unwrap()
                .set("Contents", Object::string_literal("changed")),
            "reordered" => new
                .get_dictionary_mut(page)
                .unwrap()
                .get_mut(b"Annots")
                .unwrap()
                .as_array_mut()
                .unwrap()
                .swap(0, 2),
            "aliased-root" => {
                new.get_dictionary_mut(page)
                    .unwrap()
                    .get_mut(b"Annots")
                    .unwrap()
                    .as_array_mut()
                    .unwrap()[0] = owner.into()
            }
            "tagged" => new
                .get_dictionary_mut(copied_owner)
                .unwrap()
                .set("StructParent", 0),
            "original-changed" => new
                .get_dictionary_mut(owner)
                .unwrap()
                .set("Contents", Object::string_literal("changed original")),
            _ => {}
        }
        let mut next = copied_index(&new, owner);
        let mut original = index(&old, owner, "old");
        if case == "missing-page" {
            next.bindings.retain(|b| b.page_index == 0);
        }
        if case == "budget" {
            original.used = MAX_METADATA;
        }
        if case == "legacy" {
            original.trusted = false;
        }
        let origin = if case == "wrong-origin" { 1 } else { 0 };
        let cancel = case == "cancel";
        assert!(
            original
                .retained_snapshot_transaction(&next, None, Some(origin), Arc::new(move || cancel))
                .is_err(),
            "accepted {case}"
        );
    }
}

// The fixture owns its backing separately, exactly as OpenDocument does.
struct TestIndex { index: Index, source: crate::source_storage::SourceHandle }
impl std::ops::Deref for TestIndex { type Target=Index; fn deref(&self)->&Index { &self.index } }
impl std::ops::DerefMut for TestIndex { fn deref_mut(&mut self)->&mut Index { &mut self.index } }
impl TestIndex {
    fn replace_source(&mut self, bytes: Vec<u8>) {
        self.source=crate::source_storage::SourceStore::new().capture(bytes.into()).unwrap();
        self.index.source=self.source.identity().clone();
        self.index.source_hash=OnceLock::new();
    }
    fn retained_snapshot(&self, next:&Self)->EngineResult<Snapshot> {
        self.index.retained_snapshot(&next.index, (&self.source,&next.source))
    }
    fn retained_snapshot_controlled(&self,next:&Self,cancelled:Arc<dyn Fn()->bool+Send+Sync>)->EngineResult<Snapshot> {
        self.index.retained_snapshot_controlled(&next.index,(&self.source,&next.source),cancelled)
    }
    fn retained_snapshot_transfer(&self,next:&Self,removed:Option<&[(usize,usize)]>,cancelled:Arc<dyn Fn()->bool+Send+Sync>)->EngineResult<Snapshot> {
        self.index.retained_snapshot_transfer(&next.index,(&self.source,&next.source),removed,cancelled)
    }
    fn retained_snapshot_after_removal(&self,next:&Self,removed:Option<(RemovalKind,&[(usize,usize)])>,cancelled:Arc<dyn Fn()->bool+Send+Sync>)->EngineResult<Snapshot> {
        self.index.retained_snapshot_after_removal(&next.index,(&self.source,&next.source),removed,cancelled)
    }
    fn retained_snapshot_transaction(&self,next:&Self,removed:Option<(RemovalKind,&[(usize,usize)])>,copied:Option<usize>,cancelled:Arc<dyn Fn()->bool+Send+Sync>)->EngineResult<Snapshot> {
        self.index.retained_snapshot_transaction(&next.index,(&self.source,&next.source),removed,copied,cancelled)
    }
}

#[test]
fn transfer_rejects_equal_byte_foreign_backing_handle() {
    let (pdf,_,annotation,_,_)=fixture();let old=index(&pdf,annotation,"old");let next=index(&pdf,annotation,"new");
    let foreign=crate::source_storage::SourceStore::new().capture(old.source.lease().unwrap()).unwrap();
    assert_eq!(foreign.identity().sha256(),old.source.identity().sha256());
    assert!(old.index.retained_snapshot(&next.index,(&foreign,&next.source)).is_err());
    assert!(old.index.retained_snapshot(&next.index,(&old.source,&foreign)).is_err());
    assert!(old.retained_snapshot(&next).is_ok());
}
