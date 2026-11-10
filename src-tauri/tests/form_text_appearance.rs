use folio_engine::{
    FillFormFieldRequest, FormFieldValue, PdfEngine, SetFormTextAppearanceRequest,
    TextFieldAlignment, TextFieldAppearanceChanges,
};
use lopdf::{Document, Object, Stream};
use std::path::{Path, PathBuf};
#[path = "support/affine_forms.rs"]
mod authored;
#[path = "support/protected_fixture.rs"]
mod protected_fixture;

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn fixture(rotation: u16, unit: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../tests/fixtures/form-text-appearance/fields-r{rotation}-u{unit}.pdf"
    ))
}
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
fn widget(d: &Document, page: usize, index: usize) -> &lopdf::Dictionary {
    let p = d
        .get_dictionary(*d.get_pages().values().nth(page).unwrap())
        .unwrap();
    let a = d
        .dereference(p.get(b"Annots").unwrap())
        .unwrap()
        .1
        .as_array()
        .unwrap();
    d.dereference(&a[index]).unwrap().1.as_dict().unwrap()
}
fn normal(d: &Document, page: usize, index: usize) -> &Stream {
    let ap = d
        .dereference(widget(d, page, index).get(b"AP").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    d.dereference(ap.get(b"N").unwrap())
        .unwrap()
        .1
        .as_stream()
        .unwrap()
}
fn request(
    e: &PdfEngine,
    id: &str,
    page: usize,
    index: usize,
    changes: TextFieldAppearanceChanges,
) -> SetFormTextAppearanceRequest {
    let field = e
        .list_form_fields(id, page)
        .unwrap()
        .into_iter()
        .find(|f| f.annotation_index == index)
        .unwrap();
    let style = e.inspect_form_text_appearance(id, page, index).unwrap();
    SetFormTextAppearanceRequest {
        annotation_index: index,
        expected_bounds: field.bounds.unwrap(),
        expected_value: field.value,
        expected_style_key: style.style_key,
        changes,
    }
}
fn changes(
    size: Option<f32>,
    color: Option<&str>,
    alignment: Option<TextFieldAlignment>,
) -> TextFieldAppearanceChanges {
    TextFieldAppearanceChanges {
        font_size: size,
        color: color.map(str::to_owned),
        alignment,
    }
}

#[test]
fn physical_size_and_alignment_keep_original_cmyk_shared_value_and_sibling_artwork() {
    let e = engine();
    let path = fixture(0, "1");
    let input = std::fs::read(&path).unwrap();
    let original = Document::load_mem(&input).unwrap();
    let info = e.open_document(&path).unwrap();
    let style = e.inspect_form_text_appearance(&info.id, 0, 0).unwrap();
    assert_eq!(style.font_size, 12.);
    assert_eq!(style.color_space, "cmyk");
    assert!(style.font_label.contains("Helvetica"));
    let req = request(
        &e,
        &info.id,
        0,
        0,
        changes(Some(18.), None, Some(TextFieldAlignment::Center)),
    );
    let updated = e
        .set_form_text_appearance_occurrence(&info.id, &[0, 1, 2], 0, req)
        .unwrap();
    let now = e
        .inspect_form_text_appearance(&updated.document.id, 0, 0)
        .unwrap();
    assert_eq!(now.font_size, 18.);
    assert_eq!(now.color_space, "cmyk");
    assert_eq!(now.alignment, TextFieldAlignment::Center);
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("styled.pdf");
    export(&e, &updated.document, &output);
    let saved = Document::load(&output).unwrap();
    assert_eq!(
        normal(&original, 1, 0).get_plain_content().unwrap(),
        normal(&saved, 1, 0).get_plain_content().unwrap()
    );
    let ops = lopdf::content::Content::decode(&normal(&saved, 0, 0).get_plain_content().unwrap())
        .unwrap();
    assert!(ops.operations.iter().any(|op| op.operator == "k"));
    let p0 = widget(&saved, 0, 0)
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    assert_eq!(
        widget(&saved, 1, 0)
            .get(b"Parent")
            .unwrap()
            .as_reference()
            .unwrap(),
        p0
    );
    let parent = saved.get_dictionary(p0).unwrap();
    assert_eq!(parent.get(b"MaxLen").unwrap().as_i64().unwrap(), 80);
    assert!(parent.has(b"AA"));
    assert_eq!(
        parent.get(b"DV").unwrap().as_str().unwrap(),
        b"Default value"
    );
    let reopened = e.open_document(&output).unwrap();
    for page in [0, 1] {
        assert_eq!(
            e.list_form_fields(&reopened.id, page).unwrap()[0].value,
            FormFieldValue::Text {
                text: "Original value".into()
            }
        );
    }
    assert_eq!(std::fs::read(path).unwrap(), input);
    for id in [&info.id, &updated.document.id, &reopened.id] {
        e.close_document(id).unwrap();
    }
}

#[test]
fn empty_widget_can_gain_appearance_and_later_fill_keeps_explicit_settings() {
    let e = engine();
    let info = e.open_document(fixture(0, "1")).unwrap();
    let style = e.inspect_form_text_appearance(&info.id, 0, 1).unwrap();
    assert_eq!(style.font_size, 0.);
    let req = request(
        &e,
        &info.id,
        0,
        1,
        changes(Some(16.), Some("#007f00"), Some(TextFieldAlignment::Left)),
    );
    let updated = e
        .set_form_text_appearance_occurrence(&info.id, &[0, 1, 2], 0, req)
        .unwrap();
    let filled = e
        .fill_form_field(
            &updated.document.id,
            FillFormFieldRequest {
                page_index: 0,
                annotation_index: 1,
                expected_value: FormFieldValue::Text {
                    text: String::new(),
                },
                value: FormFieldValue::Text {
                    text: "New value".into(),
                },
            },
        )
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("filled.pdf");
    export(&e, &filled, &output);
    let reopened = e.open_document(&output).unwrap();
    let actual = e.inspect_form_text_appearance(&reopened.id, 0, 1).unwrap();
    assert_eq!(actual.font_size, 16.);
    assert_eq!(actual.color, "#007f00");
    let saved = Document::load(&output).unwrap();
    let content = normal(&saved, 0, 1).get_plain_content().unwrap();
    assert!(!content.is_empty());
    assert!(lopdf::content::Content::decode(&content)
        .unwrap()
        .operations
        .iter()
        .any(|op| op.operator == "Tj" || op.operator == "TJ"));
    for id in [&info.id, &updated.document.id, &filled.id, &reopened.id] {
        e.close_document(id).unwrap();
    }
}

#[test]
fn changed_only_payload_rejects_empty_invalid_and_stale_styles() {
    let e = engine();
    let info = e.open_document(fixture(0, "1")).unwrap();
    for change in [
        changes(None, None, None),
        changes(Some(-1.), None, None),
        changes(Some(513.), None, None),
        changes(None, Some("red"), None),
    ] {
        let req = request(&e, &info.id, 0, 0, change);
        assert!(e
            .set_form_text_appearance_occurrence(&info.id, &[0, 1, 2], 0, req)
            .is_err());
    }
    let mut req = request(&e, &info.id, 0, 0, changes(Some(18.), None, None));
    req.expected_style_key = "stale".into();
    assert!(e
        .set_form_text_appearance_occurrence(&info.id, &[0, 1, 2], 0, req)
        .is_err());
    e.close_document(&info.id).unwrap();
}

#[test]
fn physical_settings_survive_all_rotations_units_and_saved_reopen() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for unit in ["1", "1.7", "2"] {
        for rotation in [0, 90, 180, 270] {
            let info = e.open_document(fixture(rotation, unit)).unwrap();
            let original = e.inspect_form_text_appearance(&info.id, 0, 0).unwrap();
            assert!((original.font_size - 12. * unit.parse::<f32>().unwrap()).abs() < 0.0001);
            let req = request(
                &e,
                &info.id,
                0,
                0,
                changes(Some(18.), Some("#336699"), Some(TextFieldAlignment::Right)),
            );
            let result = e
                .set_form_text_appearance_occurrence(&info.id, &[0, 1, 2], 0, req)
                .unwrap();
            let output = temp.path().join(format!("r{rotation}-u{unit}.pdf"));
            export(&e, &result.document, &output);
            let reopened = e.open_document(&output).unwrap();
            let actual = e.inspect_form_text_appearance(&reopened.id, 0, 0).unwrap();
            assert!((actual.font_size - 18.).abs() < 0.0001);
            assert_eq!(actual.color, "#336699");
            assert_eq!(actual.alignment, TextFieldAlignment::Right);
            if let Some(root) = std::env::var_os("FOLIO_FORM_STYLE_OUTPUT") {
                let root = PathBuf::from(root);
                std::fs::create_dir_all(&root).unwrap();
                std::fs::copy(&output, root.join(format!("r{rotation}-u{unit}.pdf"))).unwrap();
            }
            for id in [&info.id, &result.document.id, &reopened.id] {
                e.close_document(id).unwrap();
            }
        }
    }
}

#[test]
fn repeated_page_selection_styles_only_selected_occurrence_and_keeps_shared_values() {
    let e = engine();
    for selected in [0, 1] {
        let info = e.open_document(fixture(0, "1")).unwrap();
        let req = request(&e, &info.id, 0, 0, changes(Some(20.), None, None));
        let result = e
            .set_form_text_appearance_occurrence(&info.id, &[0, 0, 1, 2], selected, req)
            .unwrap();
        let target = result.page_indices[selected];
        let other = result.page_indices[1 - selected];
        assert_ne!(target, other);
        assert_eq!(
            e.inspect_form_text_appearance(&result.document.id, target, 0)
                .unwrap()
                .font_size,
            20.
        );
        assert_eq!(
            e.inspect_form_text_appearance(&result.document.id, other, 0)
                .unwrap()
                .font_size,
            12.
        );
        let filled = e
            .fill_form_field(
                &result.document.id,
                FillFormFieldRequest {
                    page_index: target,
                    annotation_index: 0,
                    expected_value: FormFieldValue::Text {
                        text: "Original value".into(),
                    },
                    value: FormFieldValue::Text {
                        text: "Shared update".into(),
                    },
                },
            )
            .unwrap();
        for page in [target, other, result.page_indices[2]] {
            assert_eq!(
                e.list_form_fields(&filled.id, page).unwrap()[0].value,
                FormFieldValue::Text {
                    text: "Shared update".into()
                }
            );
        }
        for id in [&info.id, &result.document.id, &filled.id] {
            e.close_document(id).unwrap();
        }
    }
}

#[test]
fn conflicting_saved_font_is_rejected_without_changing_source() {
    let e = engine();
    let mut doc = Document::load(fixture(0, "1")).unwrap();
    let normal_id = doc
        .dereference(widget(&doc, 0, 0).get(b"AP").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap()
        .get(b"N")
        .unwrap()
        .as_reference()
        .unwrap();
    let courier = doc
        .add_object(lopdf::dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
    doc.get_object_mut(normal_id)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .dict
        .set(
            "Resources",
            lopdf::dictionary! {"Font"=>lopdf::dictionary!{"F1"=>courier}},
        );
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("conflict.pdf");
    doc.save(&path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let info = e.open_document(&path).unwrap();
    let error = e
        .inspect_form_text_appearance(&info.id, 0, 0)
        .unwrap_err()
        .to_string();
    assert!(error.contains("different fonts"), "{error}");
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    e.close_document(&info.id).unwrap();
}

#[test]
fn indirect_original_da_and_clipped_color_survive_alignment_only_regeneration() {
    let e = engine();
    let mut doc = Document::load(fixture(0, "1")).unwrap();
    let page = *doc.get_pages().values().next().unwrap();
    let id = doc
        .get_dictionary(page)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .as_reference()
        .unwrap();
    let original = b"/F1 12 Tf 2 -1 .5 rg";
    let da = doc.add_object(Object::string_literal(original.to_vec()));
    doc.get_dictionary_mut(id).unwrap().set("DA", da);
    // This test varies DA storage and clipping, not visible/default disagreement.
    let mut ap = normal(&doc, 0, 0).clone();
    ap.set_plain_content(b"BT /F1 12 Tf 1 0 .5 rg 3 10 Td (Original value) Tj ET".to_vec());
    let ap = doc.add_object(ap);
    doc.get_dictionary_mut(id)
        .unwrap()
        .set("AP", lopdf::dictionary! {"N"=>ap});
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("indirect.pdf");
    doc.save(&path).unwrap();
    let info = e.open_document(&path).unwrap();
    let req = request(
        &e,
        &info.id,
        0,
        0,
        changes(None, None, Some(TextFieldAlignment::Right)),
    );
    let result = e
        .set_form_text_appearance_occurrence(&info.id, &[0, 1, 2], 0, req)
        .unwrap();
    let output = temp.path().join("after.pdf");
    export(&e, &result.document, &output);
    let saved = Document::load(&output).unwrap();
    let da = saved
        .dereference(widget(&saved, 0, 0).get(b"DA").unwrap())
        .unwrap()
        .1
        .as_str()
        .unwrap();
    assert_eq!(da, original);
    let reopened = e.open_document(&output).unwrap();
    assert_eq!(
        e.inspect_form_text_appearance(&reopened.id, 0, 0)
            .unwrap()
            .color,
        "#ff0080"
    );
    for id in [&info.id, &result.document.id, &reopened.id] {
        e.close_document(id).unwrap();
    }
}

#[test]
fn repeated_style_changes_prune_obsolete_appearances_and_preserve_auto_fit() {
    let e = engine();
    let info = e.open_document(fixture(0, "1")).unwrap();
    let mut current = info;
    let temp = tempfile::tempdir().unwrap();
    let mut count = 0;
    for round in 0..12 {
        let req = request(
            &e,
            &current.id,
            0,
            0,
            changes(
                Some(if round == 11 { 0. } else { 12. + round as f32 }),
                None,
                None,
            ),
        );
        let result = e
            .set_form_text_appearance_occurrence(&current.id, &[0, 1, 2], 0, req)
            .unwrap();
        e.close_document(&current.id).unwrap();
        current = result.document;
        let output = temp.path().join(format!("round{round}.pdf"));
        export(&e, &current, &output);
        let doc = Document::load(&output).unwrap();
        if round == 0 {
            count = doc.objects.len();
        }
        assert!(
            doc.objects.len() <= count + 4,
            "unreachable appearance graphs retained"
        );
    }
    assert_eq!(
        e.inspect_form_text_appearance(&current.id, 0, 0)
            .unwrap()
            .font_size,
        0.
    );
    e.close_document(&current.id).unwrap();
}

#[test]
fn reader_permissions_block_styling_and_owner_copy_retains_passwords() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let input = std::fs::read(fixture(0, "1")).unwrap();
    let encrypted = protected_fixture::encrypt(&input, lopdf::encryption::Permissions::FILLABLE);
    let path = temp.path().join("protected.pdf");
    std::fs::write(&path, &encrypted).unwrap();
    let reader = e.open_document_with_password(&path, "reader").unwrap();
    assert!(e.inspect_form_text_appearance(&reader.id, 0, 0).is_err());
    let owner = e.open_document_with_password(&path, "owner").unwrap();
    let req = request(&e, &owner.id, 0, 0, changes(Some(18.), None, None));
    assert!(e
        .set_form_text_appearance_occurrence(&reader.id, &[0, 1, 2], 0, req)
        .is_err());
    let req = request(&e, &owner.id, 0, 0, changes(Some(18.), None, None));
    let result = e
        .set_form_text_appearance_occurrence(&owner.id, &[0, 1, 2], 0, req)
        .unwrap();
    let out = temp.path().join("saved.pdf");
    export(&e, &result.document, &out);
    assert!(e.open_document(&out).is_err());
    let reopened = e.open_document_with_password(&out, "owner").unwrap();
    assert_eq!(
        e.inspect_form_text_appearance(&reopened.id, 0, 0)
            .unwrap()
            .font_size,
        18.
    );
    let restricted = e.open_document_with_password(&out, "reader").unwrap();
    assert!(e
        .inspect_form_text_appearance(&restricted.id, 0, 0)
        .is_err());
    assert_eq!(std::fs::read(path).unwrap(), encrypted);
    for id in [
        &reader.id,
        &owner.id,
        &result.document.id,
        &reopened.id,
        &restricted.id,
    ] {
        e.close_document(id).unwrap();
    }
}

#[test]
fn inherited_and_page_font_contexts_keep_readonly_flags_and_alternate_artwork() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for location in ["widget", "parent", "page"] {
        let mut d = Document::load(fixture(0, "1")).unwrap();
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
        let parent = d
            .get_dictionary(wid)
            .unwrap()
            .get(b"Parent")
            .unwrap()
            .as_reference()
            .unwrap();
        let form_value = d
            .dereference(d.catalog().unwrap().get(b"AcroForm").unwrap())
            .unwrap()
            .1
            .clone();
        let form = d.add_object(form_value);
        let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
        d.get_dictionary_mut(root).unwrap().set("AcroForm", form);
        let resources = d.get_dictionary(form).unwrap().get(b"DR").unwrap().clone();
        d.get_dictionary_mut(form).unwrap().remove(b"DR");
        let target = match location {
            "widget" => wid,
            "parent" => parent,
            _ => page,
        };
        d.get_dictionary_mut(target).unwrap().set(
            if location == "page" {
                "Resources"
            } else {
                "DR"
            },
            resources,
        );
        d.get_dictionary_mut(parent).unwrap().set("Ff", 1);
        d.get_dictionary_mut(wid).unwrap().set("P", page);
        let alternate = d.add_object(Stream::new(
            lopdf::dictionary! {"BBox"=>vec![0.into(),0.into(),200.into(),40.into()]},
            b"0 1 0 rg 0 0 200 40 re f".to_vec(),
        ));
        let mut ap = d
            .get_dictionary(wid)
            .unwrap()
            .get(b"AP")
            .unwrap()
            .as_dict()
            .unwrap()
            .clone();
        ap.set("R", alternate);
        ap.set("D", alternate);
        d.get_dictionary_mut(wid).unwrap().set("AP", ap);
        let path = temp.path().join(format!("{location}.pdf"));
        d.save(&path).unwrap();
        let info = e.open_document(&path).unwrap();
        let req = request(&e, &info.id, 0, 0, changes(Some(17.), None, None));
        let result = e
            .set_form_text_appearance_occurrence(&info.id, &[0, 1, 2], 0, req)
            .unwrap();
        let out = temp.path().join(format!("{location}-after.pdf"));
        export(&e, &result.document, &out);
        let saved = Document::load(&out).unwrap();
        let w = widget(&saved, 0, 0);
        let ap = saved
            .dereference(w.get(b"AP").unwrap())
            .unwrap()
            .1
            .as_dict()
            .unwrap();
        for key in [b"R", b"D"] {
            let stream = saved
                .dereference(ap.get(key).unwrap())
                .unwrap()
                .1
                .as_stream()
                .unwrap();
            assert_eq!(
                stream.get_plain_content().unwrap(),
                b"0 1 0 rg 0 0 200 40 re f"
            );
        }
        assert!(e.list_form_fields(&result.document.id, 0).unwrap()[0].read_only);
        for id in [&info.id, &result.document.id] {
            e.close_document(id).unwrap();
        }
    }
}

#[test]
fn embedded_font_program_survives_styling_empty_widget_and_later_fill() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("embedded.pdf");
    authored::fixture(&path, "simple", "plain", 0, "Cafe old");
    let mut d = Document::load(&path).unwrap();
    let font = d
        .objects
        .iter()
        .find_map(|(id, o)| {
            o.as_dict()
                .ok()
                .filter(|v| v.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"TrueType"))
                .map(|_| *id)
        })
        .unwrap();
    let program = |d: &Document| -> Vec<u8> {
        let value = d
            .objects
            .values()
            .filter_map(|o| o.as_dict().ok())
            .find_map(|v| v.get(b"FontFile2").ok())
            .unwrap();
        d.dereference(value)
            .unwrap()
            .1
            .as_stream()
            .unwrap()
            .get_plain_content()
            .unwrap()
    };
    let original_program = program(&d);
    let page = *d.get_pages().values().next().unwrap();
    let w = d.add_object(lopdf::dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>Object::string_literal("embedded"),"V"=>Object::string_literal(""),"Rect"=>vec![30.into(),30.into(),300.into(),70.into()],"P"=>page,"F"=>4});
    let form = d.add_object(lopdf::dictionary!{"Fields"=>vec![Object::Reference(w)],"DA"=>Object::string_literal("/F1 12 Tf 0 g"),"DR"=>lopdf::dictionary!{"Font"=>lopdf::dictionary!{"F1"=>font}}});
    d.get_dictionary_mut(page)
        .unwrap()
        .set("Annots", vec![Object::Reference(w)]);
    let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
    d.get_dictionary_mut(root).unwrap().set("AcroForm", form);
    d.save(&path).unwrap();
    let source = e.open_document(&path).unwrap();
    let req = request(
        &e,
        &source.id,
        0,
        0,
        changes(Some(18.), Some("#004488"), None),
    );
    let styled = e
        .set_form_text_appearance_occurrence(&source.id, &[0], 0, req)
        .unwrap();
    let filled = e
        .fill_form_field(
            &styled.document.id,
            FillFormFieldRequest {
                page_index: 0,
                annotation_index: 0,
                expected_value: FormFieldValue::Text {
                    text: String::new(),
                },
                value: FormFieldValue::Text {
                    text: "Cafe new".into(),
                },
            },
        )
        .unwrap();
    let output = temp.path().join("filled.pdf");
    export(&e, &filled, &output);
    let saved = Document::load(&output).unwrap();
    assert_eq!(program(&saved), original_program);
    let ap = normal(&saved, 0, 0);
    let resources = saved
        .dereference(ap.dict.get(b"Resources").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    let fonts = saved
        .dereference(resources.get(b"Font").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    assert!(fonts.iter().any(|(_, v)| saved
        .dereference(v)
        .unwrap()
        .1
        .as_dict()
        .unwrap()
        .get(b"Subtype")
        .and_then(Object::as_name)
        .ok()
        == Some(b"TrueType")));
    for op in lopdf::content::Content::decode(&ap.get_plain_content().unwrap())
        .unwrap()
        .operations
        .iter()
        .filter(|op| op.operator == "Tf")
    {
        let used = saved
            .dereference(fonts.get(op.operands[0].as_name().unwrap()).unwrap())
            .unwrap()
            .1
            .as_dict()
            .unwrap();
        let descriptor = saved
            .dereference(used.get(b"FontDescriptor").unwrap())
            .unwrap()
            .1
            .as_dict()
            .unwrap();
        let actual = saved
            .dereference(descriptor.get(b"FontFile2").unwrap())
            .unwrap()
            .1
            .as_stream()
            .unwrap()
            .get_plain_content()
            .unwrap();
        assert_eq!(
            actual, original_program,
            "the AP must use the original embedded program"
        );
    }
    let reopened = e.open_document(&output).unwrap();
    assert_eq!(
        e.inspect_form_text_appearance(&reopened.id, 0, 0)
            .unwrap()
            .font_size,
        18.
    );
    for id in [&source.id, &styled.document.id, &filled.id, &reopened.id] {
        e.close_document(id).unwrap();
    }
}

#[test]
fn review_custom_paint_and_mismatched_visible_settings_are_not_discarded() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut admitted = Vec::new();
    for (name, paint, empty) in [
        (
            "background",
            "q 1 1 0 rg 0 0 220 30 re f Q BT /F1 12 Tf 0 1 1 0 k 3 10 Td (Original value) Tj ET",
            false,
        ),
        ("empty-art", "q 1 1 0 rg 0 0 220 30 re f Q", true),
        ("declared-background-after-text", "BT /F1 12 Tf 0 1 1 0 k 3 10 Td (Original value) Tj ET 1 1 0 rg 0 0 220 30 re f", false),
        ("declared-clipped-background", "q 1 1 218 28 re W n 1 1 0 rg 0 0 220 30 re f Q BT /F1 12 Tf 0 1 1 0 k 3 10 Td (Original value) Tj ET", false),
        (
            "color",
            "BT /F1 12 Tf 0 g 3 10 Td (Original value) Tj ET",
            false,
        ),
        (
            "size",
            "BT /F1 18 Tf 0 1 1 0 k 3 10 Td (Original value) Tj ET",
            false,
        ),
        (
            "scale",
            "BT /F1 12 Tf 0 1 1 0 k 80 Tz 3 10 Td (Original value) Tj ET",
            false,
        ),
    ] {
        let mut d = Document::load(fixture(0, "1")).unwrap();
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
        let ap = normal(&d, 0, 0).clone();
        let mut ap = ap;
        ap.set_plain_content(paint.as_bytes().to_vec());
        let ap = d.add_object(ap);
        d.get_dictionary_mut(wid)
            .unwrap()
            .set("AP", lopdf::dictionary! {"N"=>ap});
        if name.starts_with("declared-"){d.get_dictionary_mut(wid).unwrap().set("MK",lopdf::dictionary!{"BG"=>vec![1.into(),1.into(),0.into()]});}
        if empty {
            let parent = d
                .get_dictionary(wid)
                .unwrap()
                .get(b"Parent")
                .unwrap()
                .as_reference()
                .unwrap();
            d.get_dictionary_mut(parent)
                .unwrap()
                .set("V", Object::string_literal(""));
        }
        let path = temp.path().join(format!("{name}.pdf"));
        d.save(&path).unwrap();
        let input = std::fs::read(&path).unwrap();
        let info = e.open_document(&path).unwrap();
        let outcome = e.inspect_form_text_appearance(&info.id, 0, 0);
        if outcome.is_ok() {
            admitted.push(name);
        }
        assert_eq!(std::fs::read(path).unwrap(), input);
        e.close_document(&info.id).unwrap();
    }
    assert!(
        admitted.is_empty(),
        "custom or mismatched appearances admitted: {admitted:?}"
    );
}

#[test]
fn dictionary_border_and_multiline_text_remain_editable_after_reopen() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for multiline in [false, true] {
        let blank = e.create_blank_document(400., 500.).unwrap();
        let request=serde_json::from_value(serde_json::json!({"bounds":{"x":20,"y":30,"width":220,"height":80},"rotation":0,"name":"answer","label":"Answer","required":false,"fontSize":12,"field":{"kind":"text","multiline":multiline}})).unwrap();
        let created = e
            .create_form_field_occurrence(&blank.id, &[0], 0, request)
            .unwrap();
        let req = self::request(
            &e,
            &created.document.id,
            0,
            0,
            changes(Some(16.), None, None),
        );
        let styled = e
            .set_form_text_appearance_occurrence(&created.document.id, &[0], 0, req)
            .unwrap_or_else(|err| {
                let diagnostic = e
                    .fill_form_field(
                        &created.document.id,
                        FillFormFieldRequest {
                            page_index: 0,
                            annotation_index: 0,
                            expected_value: FormFieldValue::Text {
                                text: String::new(),
                            },
                            value: FormFieldValue::Text { text: "X".into() },
                        },
                    )
                    .unwrap();
                let pdf = Document::load_mem(&e.source_bytes(&diagnostic.id).unwrap()).unwrap();
                panic!(
                    "{err}; regenerated empty AP {:?}",
                    String::from_utf8_lossy(&normal(&pdf, 0, 0).get_plain_content().unwrap())
                );
            });
        let out = temp.path().join(format!("{multiline}.pdf"));
        export(&e, &styled.document, &out);
        let reopened = e.open_document(&out).unwrap();
        let filled = e
            .fill_form_field(
                &reopened.id,
                FillFormFieldRequest {
                    page_index: 0,
                    annotation_index: 0,
                    expected_value: FormFieldValue::Text {
                        text: String::new(),
                    },
                    value: FormFieldValue::Text {
                        text: if multiline {
                            "First line\nSecond line"
                        } else {
                            "Single line"
                        }
                        .into(),
                    },
                },
            )
            .unwrap();
        let req = self::request(&e, &filled.id, 0, 0, changes(None, Some("#008800"), None));
        let restyled = e
            .set_form_text_appearance_occurrence(&filled.id, &[0], 0, req)
            .unwrap();
        assert_eq!(
            e.inspect_form_text_appearance(&restyled.document.id, 0, 0)
                .unwrap()
                .font_size,
            16.
        );
        for id in [
            &blank.id,
            &created.document.id,
            &styled.document.id,
            &reopened.id,
            &filled.id,
            &restyled.document.id,
        ] {
            e.close_document(id).unwrap();
        }
    }
}

#[test]
fn omitted_fractional_size_and_cmyk_survive_reopen_then_auto_fit_and_later_fill() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let source = e.open_document(fixture(90, "1.7")).unwrap();
    let before = e.inspect_form_text_appearance(&source.id, 0, 0).unwrap();
    let r = request(
        &e,
        &source.id,
        0,
        0,
        changes(None, None, Some(TextFieldAlignment::Right)),
    );
    let changed = e
        .set_form_text_appearance_occurrence(&source.id, &[0, 1, 2], 0, r)
        .unwrap();
    let file = temp.path().join("fractional.pdf");
    export(&e, &changed.document, &file);
    let reopened = e.open_document(&file).unwrap();
    let style = e.inspect_form_text_appearance(&reopened.id, 0, 0).unwrap();
    assert_eq!(style.font_size, before.font_size);
    assert_eq!(style.color_space, "cmyk");
    let r = request(&e, &reopened.id, 0, 0, changes(Some(0.), None, None));
    let auto = e
        .set_form_text_appearance_occurrence(&reopened.id, &[0, 1, 2], 0, r)
        .unwrap();
    let file = temp.path().join("auto.pdf");
    export(&e, &auto.document, &file);
    let reopened_auto = e.open_document(&file).unwrap();
    let field = e.list_form_fields(&reopened_auto.id, 0).unwrap().remove(0);
    let filled = e
        .fill_form_field(
            &reopened_auto.id,
            FillFormFieldRequest {
                page_index: 0,
                annotation_index: 0,
                expected_value: field.value,
                value: FormFieldValue::Text {
                    text: "Longer value after reopening auto fit".into(),
                },
            },
        )
        .unwrap();
    let actual = e.inspect_form_text_appearance(&filled.id, 0, 0).unwrap();
    assert_eq!(actual.font_size, 0.);
    assert_eq!(actual.color_space, "cmyk");
    assert_eq!(actual.alignment, TextFieldAlignment::Right);
    for id in [
        &source.id,
        &changed.document.id,
        &reopened.id,
        &auto.document.id,
        &reopened_auto.id,
        &filled.id,
    ] {
        e.close_document(id).unwrap();
    }
}

#[test]
fn plain_comb_field_keeps_value_and_cell_count_when_styled() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut d = Document::load(fixture(0, "1")).unwrap();
    let parent = widget(&d, 0, 0)
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    d.get_dictionary_mut(parent).unwrap().set("Ff", 1 << 24);
    d.get_dictionary_mut(parent).unwrap().set("MaxLen", 20);
    let file = temp.path().join("comb.pdf");
    d.save(&file).unwrap();
    let source = e.open_document(&file).unwrap();
    let r = request(&e, &source.id, 0, 0, changes(Some(10.), None, None));
    let styled = e
        .set_form_text_appearance_occurrence(&source.id, &[0, 1, 2], 0, r)
        .unwrap();
    let file = temp.path().join("comb-after.pdf");
    export(&e, &styled.document, &file);
    let reopened = e.open_document(&file).unwrap();
    let actual = e.inspect_form_text_appearance(&reopened.id, 0, 0).unwrap();
    assert_eq!(actual.font_size, 10.);
    let saved = Document::load(&file).unwrap();
    let parent = widget(&saved, 0, 0)
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    assert_eq!(
        saved
            .get_dictionary(parent)
            .unwrap()
            .get(b"MaxLen")
            .unwrap()
            .as_i64()
            .unwrap(),
        20
    );
    assert_eq!(
        e.list_form_fields(&reopened.id, 0).unwrap()[0].value,
        FormFieldValue::Text {
            text: "Original value".into()
        }
    );
    for id in [&source.id, &styled.document.id, &reopened.id] {
        e.close_document(id).unwrap();
    }
}
