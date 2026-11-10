use folio_engine::{PdfEngine, SetFormTextAppearanceRequest, TextFieldAppearanceChanges};
use lopdf::{Document, Object};
use std::path::PathBuf;

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn sample(value: Object, painted: Option<&[u8]>) -> Document {
    let mut d = Document::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/form-text-appearance/fields-r0-u1.pdf"),
    )
    .unwrap();
    let page = *d.get_pages().values().next().unwrap();
    let wid = d
        .get_dictionary(page)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .as_reference()
        .unwrap();
    let ap = d
        .get_dictionary(wid)
        .unwrap()
        .get(b"AP")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"N")
        .unwrap()
        .as_reference()
        .unwrap();
    let font = d
        .get_object(ap)
        .unwrap()
        .as_stream()
        .unwrap()
        .dict
        .get(b"Resources")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Font")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"F1")
        .unwrap()
        .as_reference()
        .unwrap();
    d.get_dictionary_mut(font)
        .unwrap()
        .set("Encoding", "WinAnsiEncoding");
    let parent = d
        .get_dictionary(wid)
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    d.get_dictionary_mut(parent).unwrap().set("V", value);
    if let Some(painted) = painted {
        let mut content =
            lopdf::content::Content::decode(b"BT /F1 12 Tf 0 1 1 0 k 3 10 Td").unwrap();
        content.operations.push(lopdf::content::Operation::new(
            "Tj",
            vec![Object::string_literal(painted)],
        ));
        content
            .operations
            .push(lopdf::content::Operation::new("ET", vec![]));
        d.get_object_mut(ap)
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .set_plain_content(content.encode().unwrap());
    } else {
        d.get_dictionary_mut(wid).unwrap().remove(b"AP");
    }
    d
}
#[test]
fn exact_saved_answers_spaces_empty_and_missing_appearances_are_distinguished() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for (name, value, painted, accepted) in [
        ("same", "Answer", Some(b"Answer".as_slice()), true),
        (
            "extra",
            "Answer",
            Some(b"Answer decoration".as_slice()),
            false,
        ),
        ("stale", "", Some(b"Stale".as_slice()), false),
        ("space-missing", "A B", Some(b"AB".as_slice()), false),
        ("space-extra", "AB", Some(b"A B".as_slice()), false),
        ("empty-missing", "", None, true),
        ("nonempty-missing", "Answer", None, false),
        ("accent", "Café", Some(b"Caf\xe9".as_slice()), true),
    ] {
        let mut doc = sample(lopdf::text_string(value), painted);
        let path = temp.path().join(format!("{name}.pdf"));
        doc.save(&path).unwrap();
        let before = std::fs::read(&path).unwrap();
        let source = e.open_document(&path).unwrap();
        let result = e.inspect_form_text_appearance(&source.id, 0, 0);
        assert_eq!(result.is_ok(), accepted, "{name}: {result:?}");
        if !accepted {
            assert!(
                result.unwrap_err().to_string().contains("saved field text"),
                "{name}"
            );
        }
        assert_eq!(
            e.source_bytes(&source.id).unwrap().as_ref(),
            before.as_slice()
        );
        assert_eq!(std::fs::read(path).unwrap(), before);
        e.close_document(&source.id).unwrap();
    }
}
#[test]
fn utf8_bom_values_are_compared_as_text_without_a_visible_bom() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut doc = sample(Object::string_literal("\u{feff}Café"), Some(b"Caf\xe9"));
    let path = temp.path().join("utf8.pdf");
    doc.save(&path).unwrap();
    let source = e.open_document(path).unwrap();
    e.inspect_form_text_appearance(&source.id, 0, 0).unwrap();
    e.close_document(&source.id).unwrap();
}
#[test]
fn excessive_value_is_rejected_before_text_decoding() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut doc = sample(
        Object::string_literal(vec![b'x'; 128 * 1024 + 1]),
        Some(b"x"),
    );
    let path = temp.path().join("large.pdf");
    doc.save(&path).unwrap();
    let source = e.open_document(path).unwrap();
    let error = e
        .inspect_form_text_appearance(&source.id, 0, 0)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("value exceeds the text verification limit"),
        "{error}"
    );
    e.close_document(&source.id).unwrap();
}
#[test]
fn appearance_only_edit_does_not_replace_a_different_visible_value() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut d = Document::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/form-text-appearance/fields-r0-u1.pdf"),
    )
    .unwrap();
    let page = *d.get_pages().values().next().unwrap();
    let wid = d
        .get_dictionary(page)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .as_reference()
        .unwrap();
    let ap = d
        .get_dictionary(wid)
        .unwrap()
        .get(b"AP")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"N")
        .unwrap()
        .as_reference()
        .unwrap();
    d.get_object_mut(ap)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .set_plain_content(b"BT /F1 12 Tf 0 1 1 0 k 3 10 Td (Visible answer) Tj ET".to_vec());
    let path = temp.path().join("different.pdf");
    d.save(&path).unwrap();
    let before = std::fs::read(&path).unwrap();
    let source = e.open_document(&path).unwrap();
    let field = e.list_form_fields(&source.id, 0).unwrap().remove(0);
    let style = e.inspect_form_text_appearance(&source.id, 0, 0);
    if let Ok(style) = style {
        let result = e.set_form_text_appearance_occurrence(
            &source.id,
            &[0, 1, 2],
            0,
            SetFormTextAppearanceRequest {
                annotation_index: 0,
                expected_bounds: field.bounds.unwrap(),
                expected_value: field.value,
                expected_style_key: style.style_key,
                changes: TextFieldAppearanceChanges {
                    font_size: Some(18.),
                    color: None,
                    alignment: None,
                },
            },
        );
        if let Ok(result) = result {
            let output = Document::load_mem(&e.source_bytes(&result.document.id).unwrap()).unwrap();
            let page = *output.get_pages().values().next().unwrap();
            let w = output
                .dereference(
                    &output
                        .get_dictionary(page)
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
            let ap = output
                .dereference(w.get(b"AP").unwrap())
                .unwrap()
                .1
                .as_dict()
                .unwrap();
            let ap = output
                .dereference(ap.get(b"N").unwrap())
                .unwrap()
                .1
                .as_stream()
                .unwrap();
            let ops = lopdf::content::Content::decode(&ap.get_plain_content().unwrap()).unwrap();
            let painted: Vec<_> = ops
                .operations
                .iter()
                .filter(|op| op.operator == "Tj")
                .flat_map(|op| {
                    op.operands.iter().filter_map(|o| match o {
                        Object::String(s, _) => Some(s.clone()),
                        _ => None,
                    })
                })
                .flatten()
                .collect();
            e.close_document(&result.document.id).unwrap();
            assert_eq!(
                painted, b"Visible answer",
                "a size-only edit replaced the visible answer with the stored value"
            );
        }
    }
    assert_eq!(std::fs::read(path).unwrap(), before);
    e.close_document(&source.id).unwrap();
}

#[test]
fn naturally_wrapped_values_remain_editable_and_keep_the_exact_stored_answer() {
    use folio_engine::{FillFormFieldRequest, FormFieldValue};
    let e = engine();
    for answer in [
        "A long answer with several words that naturally wraps",
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz",
    ] {
        let blank = e.create_blank_document(400., 500.).unwrap();
        let created = e.create_form_field_occurrence(&blank.id, &[0], 0,
            serde_json::from_value(serde_json::json!({"bounds":{"x":20,"y":30,"width":95,"height":140},"rotation":0,"name":"answer","label":"Answer","required":false,"fontSize":12,"field":{"kind":"text","multiline":true}})).unwrap()).unwrap();
        let filled = e
            .fill_form_field(
                &created.document.id,
                FillFormFieldRequest {
                    page_index: 0,
                    annotation_index: 0,
                    expected_value: FormFieldValue::Text {
                        text: String::new(),
                    },
                    value: FormFieldValue::Text {
                        text: answer.into(),
                    },
                },
            )
            .unwrap();
        let field = e.list_form_fields(&filled.id, 0).unwrap().remove(0);
        if let Some(directory)=std::env::var_os("FOLIO_FILL_APPEARANCE_CAPTURE_DIR") {
            let directory=std::path::PathBuf::from(directory);std::fs::create_dir_all(&directory).unwrap();
            for (name,id) in [("created",&created.document.id),("filled",&filled.id)] {
                let path=directory.join(format!("{}-{name}.pdf",answer.len()));assert!(!path.exists());std::fs::write(path,e.source_bytes(id).unwrap()).unwrap();
            }
        }
        let style = e.inspect_form_text_appearance(&filled.id, 0, 0).unwrap();
        let styled = e
            .set_form_text_appearance_occurrence(
                &filled.id,
                &[0],
                0,
                SetFormTextAppearanceRequest {
                    annotation_index: 0,
                    expected_bounds: field.bounds.unwrap(),
                    expected_value: field.value,
                    expected_style_key: style.style_key,
                    changes: TextFieldAppearanceChanges {
                        font_size: Some(10.),
                        color: None,
                        alignment: None,
                    },
                },
            )
            .unwrap();
        assert_eq!(
            e.list_form_fields(&styled.document.id, 0).unwrap()[0].value,
            FormFieldValue::Text {
                text: answer.into()
            }
        );
        e.inspect_form_text_appearance(&styled.document.id, 0, 0)
            .unwrap();
        for id in [
            &blank.id,
            &created.document.id,
            &filled.id,
            &styled.document.id,
        ] {
            e.close_document(id).unwrap();
        }
    }
}

#[test]
fn review_indirect_appearance_bbox_keeps_ordinary_fields_editable() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for mode in 0..4 {
        let mut doc = sample(lopdf::text_string("Answer"), Some(b"Answer"));
        let page = *doc.get_pages().values().next().unwrap();
        let wid = doc
            .get_dictionary(page)
            .unwrap()
            .get(b"Annots")
            .unwrap()
            .as_array()
            .unwrap()[0]
            .as_reference()
            .unwrap();
        let ap = doc
            .get_dictionary(wid)
            .unwrap()
            .get(b"AP")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"N")
            .unwrap()
            .as_reference()
            .unwrap();
        let mut bounds = doc
            .get_object(ap)
            .unwrap()
            .as_stream()
            .unwrap()
            .dict
            .get(b"BBox")
            .unwrap()
            .clone();
        if mode & 2 != 0 {
            for value in bounds.as_array_mut().unwrap() {
                *value = doc.add_object(value.clone()).into();
            }
        }
        if mode & 1 != 0 {
            bounds = doc.add_object(bounds).into();
        }
        doc.get_object_mut(ap)
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .dict
            .set("BBox", bounds);
        let path = temp.path().join(format!("indirect-bbox-{mode}.pdf"));
        doc.save(&path).unwrap();
        let before = std::fs::read(&path).unwrap();
        let source = e.open_document(&path).unwrap();
        let style = e.inspect_form_text_appearance(&source.id, 0, 0).unwrap();
        let field = e.list_form_fields(&source.id, 0).unwrap().remove(0);
        let styled = e
            .set_form_text_appearance_occurrence(
                &source.id,
                &[0, 1, 2],
                0,
                SetFormTextAppearanceRequest {
                    annotation_index: 0,
                    expected_bounds: field.bounds.unwrap(),
                    expected_value: field.value,
                    expected_style_key: style.style_key,
                    changes: TextFieldAppearanceChanges {
                        font_size: Some(18.),
                        color: None,
                        alignment: None,
                    },
                },
            )
            .unwrap();
        e.inspect_form_text_appearance(&styled.document.id, 0, 0)
            .unwrap();
        assert_eq!(std::fs::read(path).unwrap(), before);
        e.close_document(&source.id).unwrap();
        e.close_document(&styled.document.id).unwrap();
    }
}
