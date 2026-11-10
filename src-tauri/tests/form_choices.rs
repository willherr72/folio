use folio_engine::{inspect_choice_pdf, ChoiceSelection, PdfEngine};
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId};
use std::path::PathBuf;

struct Fixture {
    doc: Document,
    parent: ObjectId,
    owner: ObjectId,
    widgets: Vec<ObjectId>,
}
fn text(s: &str) -> Object {
    lopdf::text_string(s)
}
fn fixture() -> Fixture {
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let parent = doc.new_object_id();
    let owner = doc.new_object_id();
    let font =
        doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let mut page_ids = Vec::new();
    let mut widgets = Vec::new();
    for _ in 0..2 {
        let page = doc.new_object_id();
        let widget = doc.add_object(
            dictionary! {"Type"=>"Annot","Subtype"=>"Widget","Parent"=>owner,"P"=>page,"F"=>4,
            "Rect"=>vec![20.into(),30.into(),180.into(),130.into()]},
        );
        doc.objects.insert(page,dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),240.into(),180.into()],
            "Resources"=>Dictionary::new(),"Annots"=>vec![widget.into()]}.into());
        page_ids.push(page.into());
        widgets.push(widget);
    }
    doc.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Kids"=>page_ids,"Count"=>2}.into(),
    );
    let opts = vec![
        Object::Array(vec![text("a"), text("Same")]),
        Object::Array(vec![text("b"), text("Same")]),
        text("Third"),
    ];
    doc.objects.insert(parent,dictionary!{"T"=>text("Group"),"FT"=>"Ch","Opt"=>opts,"Ff"=>0,"Kids"=>vec![owner.into()],"DV"=>text("a")}.into());
    doc.objects.insert(owner,dictionary!{"T"=>text("choice"),"Parent"=>parent,"Kids"=>widgets.iter().copied().map(Object::Reference).collect::<Vec<_>>(),"V"=>text("b"),"I"=>vec![1.into()]}.into());
    let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages,"AcroForm"=>dictionary!{"Fields"=>vec![parent.into()],
        "DA"=>text("/F1 12 Tf 0 g"),"DR"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"NeedAppearances"=>false}});
    doc.trailer.set("Root", catalog);
    Fixture {
        doc,
        parent,
        owner,
        widgets,
    }
}
fn bytes(f: &mut Fixture) -> Vec<u8> {
    let mut bytes = Vec::new();
    f.doc.save_to(&mut bytes).unwrap();
    bytes
}
#[test]
fn inherited_pairs_and_shared_widgets_bind_to_source_positions() {
    let mut f = fixture();
    let original = bytes(&mut f);
    let a = inspect_choice_pdf(&original, 0, 0).unwrap();
    assert_eq!(a.name, "Group.choice");
    assert_eq!(a.widgets.len(), 2);
    assert_eq!((a.widgets[0].page_index, a.widgets[1].page_index), (0, 1));
    assert_eq!(a.model.options[0].export_value, "a");
    assert_eq!(a.model.options[0].display, "Same");
    assert_eq!(a.model.options[2].export_value, "Third");
    assert_eq!(a.model.selection, ChoiceSelection::Rows(vec![1]));
    assert_eq!(a.model.defaults, Some(vec!["a".into()]));
    let b = inspect_choice_pdf(&original, 1, 0).unwrap();
    assert_eq!(a.model, b.model);
    assert_ne!(a.expected_key, b.expected_key);
    f.doc
        .get_dictionary_mut(f.owner)
        .unwrap()
        .set("V", text("a"));
    f.doc
        .get_dictionary_mut(f.owner)
        .unwrap()
        .set("I", vec![0.into()]);
    assert_ne!(
        a.expected_key,
        inspect_choice_pdf(&bytes(&mut f), 0, 0)
            .unwrap()
            .expected_key
    );
    assert_eq!(
        inspect_choice_pdf(&original, 0, 0).unwrap().model.selection,
        ChoiceSelection::Rows(vec![1])
    );
}
#[test]
fn duplicate_exports_need_consistent_explicit_indices() {
    let mut f = fixture();
    f.doc.get_dictionary_mut(f.owner).unwrap().set(
        "Opt",
        vec![
            Object::Array(vec![text("b"), text("First")]),
            Object::Array(vec![text("b"), text("Second")]),
        ],
    );
    f.doc
        .get_dictionary_mut(f.owner)
        .unwrap()
        .set("DV", Object::Null);
    let a = inspect_choice_pdf(&bytes(&mut f), 0, 0).unwrap();
    assert_eq!(a.model.selection, ChoiceSelection::Rows(vec![1]));
    f.doc.get_dictionary_mut(f.owner).unwrap().remove(b"I");
    assert!(inspect_choice_pdf(&bytes(&mut f), 0, 0).is_err());
}
#[test]
fn mismatched_duplicate_unsorted_and_out_of_range_indices_are_refused() {
    for indices in [vec![0], vec![1, 1], vec![2, 1], vec![99]] {
        let mut f = fixture();
        f.doc.get_dictionary_mut(f.owner).unwrap().set(
            "I",
            indices.into_iter().map(Object::Integer).collect::<Vec<_>>(),
        );
        assert!(inspect_choice_pdf(&bytes(&mut f), 0, 0).is_err());
    }
}
#[test]
fn missing_value_uses_indices_and_multiselect_values_match_as_a_multiset() {
    let mut f = fixture();
    f.doc.get_dictionary_mut(f.owner).unwrap().remove(b"V");
    assert_eq!(
        inspect_choice_pdf(&bytes(&mut f), 0, 0)
            .unwrap()
            .model
            .selection,
        ChoiceSelection::Rows(vec![1])
    );
    let owner = f.doc.get_dictionary_mut(f.owner).unwrap();
    owner.set("Ff", 1 << 21);
    owner.set("V", vec![text("Third"), text("a")]);
    owner.set("I", vec![0.into(), 2.into()]);
    assert_eq!(
        inspect_choice_pdf(&bytes(&mut f), 0, 0)
            .unwrap()
            .model
            .selection,
        ChoiceSelection::Rows(vec![0, 2])
    );
}
#[test]
fn custom_and_blank_values_preserve_their_distinct_meaning() {
    let mut f = fixture();
    let owner = f.doc.get_dictionary_mut(f.owner).unwrap();
    owner.set("Ff", (1 << 17) | (1 << 18));
    owner.set("V", text("Custom"));
    owner.set("I", Vec::<Object>::new());
    assert_eq!(
        inspect_choice_pdf(&bytes(&mut f), 0, 0)
            .unwrap()
            .model
            .selection,
        ChoiceSelection::Text("Custom".into())
    );
    let owner = f.doc.get_dictionary_mut(f.owner).unwrap();
    owner.set("Ff", 0);
    owner.set("V", text(""));
    owner.set("DV", text(""));
    let a = inspect_choice_pdf(&bytes(&mut f), 0, 0).unwrap();
    assert_eq!(a.model.selection, ChoiceSelection::None);
    assert_eq!(a.model.defaults, None);
}
#[test]
fn null_default_override_does_not_resurrect_the_ancestor_value() {
    let mut f = fixture();
    f.doc
        .get_dictionary_mut(f.owner)
        .unwrap()
        .set("DV", Object::Null);
    assert_eq!(
        inspect_choice_pdf(&bytes(&mut f), 0, 0)
            .unwrap()
            .model
            .defaults,
        None
    );
}
#[test]
fn unicode_and_indirect_option_values_are_decoded_without_losing_pairs() {
    let mut f = fixture();
    let display = f.doc.add_object(text("Café 🐈"));
    let export = f
        .doc
        .add_object(Object::string_literal(b"\xef\xbb\xbfvalue".to_vec()));
    let pair = f.doc.add_object(vec![export.into(), display.into()]);
    let opts = f.doc.add_object(vec![pair.into()]);
    let owner = f.doc.get_dictionary_mut(f.owner).unwrap();
    owner.set("Opt", opts);
    owner.set("V", text("value"));
    owner.set("I", vec![0.into()]);
    owner.set("DV", Object::Null);
    let a = inspect_choice_pdf(&bytes(&mut f), 0, 0).unwrap();
    assert_eq!(a.model.options[0].display, "Café 🐈");
    assert_eq!(a.model.options[0].export_value, "value");
}
#[test]
fn malformed_utf16_utf8_shapes_and_large_operands_are_rejected() {
    for value in [
        Object::string_literal(vec![0xfe, 0xff, 0]),
        Object::string_literal(vec![0xfe, 0xff, 0xd8, 0]),
        Object::string_literal(vec![0xef, 0xbb, 0xbf, 0xff]),
        Object::string_literal(vec![b'x'; 8195]),
        Object::Array(vec![text("a")]),
    ] {
        let mut f = fixture();
        f.doc
            .get_dictionary_mut(f.owner)
            .unwrap()
            .set("Opt", vec![value]);
        assert!(inspect_choice_pdf(&bytes(&mut f), 0, 0).is_err());
    }
    let mut f = fixture();
    f.doc
        .get_dictionary_mut(f.owner)
        .unwrap()
        .set("Opt", vec![text(""); 2001]);
    assert!(inspect_choice_pdf(&bytes(&mut f), 0, 0).is_err());
}
#[test]
fn wrong_parent_and_locked_linked_widgets_are_not_admitted() {
    let mut f = fixture();
    f.doc
        .get_dictionary_mut(f.widgets[1])
        .unwrap()
        .set("F", 128);
    assert!(inspect_choice_pdf(&bytes(&mut f), 0, 0).is_err());
    let mut f = fixture();
    f.doc
        .get_dictionary_mut(f.widgets[1])
        .unwrap()
        .set("Parent", f.parent);
    assert!(inspect_choice_pdf(&bytes(&mut f), 0, 0).is_err());
}
#[test]
fn admitted_selected_rows_and_labels_agree_with_native_pdfium() {
    let e = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    for case in 0..8 {
        let mut f = fixture();
        if case == 1 {
            let owner = f.doc.get_dictionary_mut(f.owner).unwrap();
            owner.set("Opt", vec![text(""), text("b")]);
            owner.set("DV", Object::Null);
            owner.set("V", text(""));
            owner.set("I", vec![0.into()]);
        }
        if case == 2 {
            let owner = f.doc.get_dictionary_mut(f.owner).unwrap();
            owner.set("Ff", 1 << 21);
            owner.set("V", vec![text("a"), text("Third")]);
            owner.set("I", vec![0.into(), 2.into()]);
        }
        if case == 3 {
            let owner = f.doc.get_dictionary_mut(f.owner).unwrap();
            owner.set("V", text(""));
            owner.set("I", Vec::<Object>::new());
        }
        if case == 4 {
            let owner = f.doc.get_dictionary_mut(f.owner).unwrap();
            owner.set(
                "Opt",
                vec![Object::Array(vec![
                    Object::string_literal(b"\xef\xbb\xbfvalue".to_vec()),
                    text("Café 🐈"),
                ])],
            );
            owner.set("V", text("value"));
            owner.set("I", vec![0.into()]);
            owner.set("DV", Object::Null);
        }
        if case == 5 {
            f.doc.get_dictionary_mut(f.owner).unwrap().set("I", 1);
        }
        if case == 6 {
            let owner = f.doc.get_dictionary_mut(f.owner).unwrap();
            owner.set(
                "Opt",
                vec![
                    Object::Array(vec![text("b"), text("First")]),
                    Object::Array(vec![text("b"), text("Second")]),
                ],
            );
            owner.set("DV", Object::Null);
        }
        if case == 7 {
            f.doc.get_dictionary_mut(f.owner).unwrap().remove(b"V");
        }
        let raw = bytes(&mut f);
        let expected = inspect_choice_pdf(&raw, 0, 0).unwrap();
        let path = dir.path().join(format!("case-{case}.pdf"));
        std::fs::write(&path, &raw).unwrap();
        let source = e.open_document(&path).unwrap();
        for page in 0..2 {
            let fields = e.list_form_fields(&source.id, page).unwrap();
            let field = &fields[0];
            assert_eq!(
                field.options,
                expected
                    .model
                    .options
                    .iter()
                    .map(|o| o.display.clone())
                    .collect::<Vec<_>>()
            );
            let rows = match &expected.model.selection {
                ChoiceSelection::Rows(v) => v.clone(),
                ChoiceSelection::None => vec![],
                other => panic!("Unexpected {other:?}"),
            };
            assert_eq!(field.selected_indices, rows, "native case{case} page{page}");
        }
        assert_eq!(std::fs::read(path).unwrap(), raw);
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn explicit_null_current_value_must_not_activate_stale_indices() {
    let mut f = fixture();
    f.doc
        .get_dictionary_mut(f.owner)
        .unwrap()
        .set("V", Object::Null);
    let raw = bytes(&mut f);
    let e = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("null-current.pdf");
    std::fs::write(&path, &raw).unwrap();
    let source = e.open_document(&path).unwrap();
    assert!(e.list_form_fields(&source.id, 0).unwrap()[0]
        .selected_indices
        .is_empty());
    e.close_document(&source.id).unwrap();
    assert!(
        inspect_choice_pdf(&raw, 0, 0).is_err(),
        "null V cannot be treated as absent V with valid I"
    );
    f.doc
        .get_dictionary_mut(f.owner)
        .unwrap()
        .set("I", Vec::<Object>::new());
    assert_eq!(
        inspect_choice_pdf(&bytes(&mut f), 0, 0)
            .unwrap()
            .model
            .selection,
        ChoiceSelection::None
    );
}

#[test]
fn missing_parent_cannot_borrow_a_field_tree_ancestors_identity() {
    let mut f = fixture();
    let options = f
        .doc
        .get_dictionary(f.parent)
        .unwrap()
        .get(b"Opt")
        .unwrap()
        .clone();
    let owner = f.doc.get_dictionary_mut(f.owner).unwrap();
    owner.remove(b"Parent");
    owner.set("FT", "Ch");
    owner.set("Opt", options);
    owner.set("DV", Object::Null);
    assert!(inspect_choice_pdf(&bytes(&mut f), 0, 0).is_err());
}

#[test]
fn native_ignores_list_only_settings_on_a_dropdown() {
    let mut f = fixture();
    let owner = f.doc.get_dictionary_mut(f.owner).unwrap();
    owner.set("Ff", (1 << 17) | (1 << 21));
    owner.set("TI", 99);
    let raw = bytes(&mut f);
    let e = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("combo-old-list-settings.pdf");
    std::fs::write(&path, &raw).unwrap();
    let source = e.open_document(&path).unwrap();
    assert_eq!(
        e.list_form_fields(&source.id, 0).unwrap()[0].selected_indices,
        vec![1]
    );
    e.close_document(&source.id).unwrap();
    let inspected = inspect_choice_pdf(&raw, 0, 0).unwrap();
    assert!(inspected.model.combo);
    assert!(!inspected.model.multiple);
    assert_eq!(inspected.model.top_index, 0);
}

#[test]
fn repeated_widget_placements_are_enumerated_even_with_one_widget_object() {
    let mut f = fixture();
    let page = *f.doc.get_pages().values().nth(1).unwrap();
    f.doc
        .get_dictionary_mut(page)
        .unwrap()
        .set("Annots", vec![f.widgets[0].into()]);
    f.doc
        .get_dictionary_mut(f.owner)
        .unwrap()
        .set("Kids", vec![f.widgets[0].into()]);
    let a = inspect_choice_pdf(&bytes(&mut f), 0, 0).unwrap();
    assert_eq!(a.widgets.len(), 2);
    assert_eq!(a.widgets[1].page_index, 1);
}
