use folio_engine::{
    AnnotationRect, CreateFormFieldRequest, DocumentInfo, FormFrameChanges, FrameColorChange,
    NewFormField, PdfEngine, SetFormFrameRequest,
};
use lopdf::{Document, Stream};
use std::path::{Path, PathBuf};
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
fn export(e: &PdfEngine, info: &DocumentInfo, path: &Path) {
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
fn record_probe(name: &str, bytes: &[u8]) {
    if let Some(root) = std::env::var_os("FOLIO_FRAME_ENGINE_PROBE") {
        let root = PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join(name))
            .unwrap();
        std::io::Write::write_all(&mut file, bytes).unwrap();
    }
}
fn request(e: &PdfEngine, id: &str, page: usize, index: usize) -> SetFormFrameRequest {
    let field = e
        .list_form_fields(id, page)
        .unwrap()
        .into_iter()
        .find(|f| f.annotation_index == index)
        .unwrap();
    let style = e.inspect_form_frame(id, page, index).unwrap();
    SetFormFrameRequest {
        annotation_index: index,
        expected_bounds: field.bounds.unwrap(),
        expected_value: field.value,
        expected_style_key: style.style_key,
        changes: FormFrameChanges {
            width: Some(2.25),
            border: Some(FrameColorChange::Rgb("#0033CC".into())),
            ..Default::default()
        },
    }
}
fn normal(d: &Document, page: usize, index: usize) -> &Stream {
    let p = d
        .get_dictionary(*d.get_pages().values().nth(page).unwrap())
        .unwrap();
    let annots = d
        .dereference(p.get(b"Annots").unwrap())
        .unwrap()
        .1
        .as_array()
        .unwrap();
    let w = d.dereference(&annots[index]).unwrap().1.as_dict().unwrap();
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
}
#[test]
fn five_field_kinds_publish_private_frames_without_changing_answers() {
    let e = engine();
    for field in [
        NewFormField::Text { multiline: false },
        NewFormField::Checkbox,
        NewFormField::Dropdown {
            options: vec!["One".into(), "Two".into()],
        },
        NewFormField::List {
            options: vec!["One".into(), "Two".into()],
            multi_select: false,
        },
        NewFormField::Radio {
            options: vec!["Yes".into(), "No".into()],
        },
    ] {
        let source = e.create_blank_document(320., 440.).unwrap();
        let created = e
            .create_form_field_occurrence(
                &source.id,
                &[0],
                0,
                CreateFormFieldRequest {
                    bounds: AnnotationRect {
                        x: 40.,
                        y: 50.,
                        width: 180.,
                        height: 60.,
                    },
                    rotation: 0,
                    name: "answer".into(),
                    label: "Answer".into(),
                    required: true,
                    font_size: 12.,
                    field,
                },
            )
            .unwrap();
        let before = e.list_form_fields(&created.document.id, 0).unwrap();
        let original = e.render_page(&created.document.id, 0, 640).unwrap();
        let req = request(&e, &created.document.id, 0, 0);
        let result = e
            .set_form_frame_occurrence(&created.document.id, &[0], 0, req)
            .unwrap();
        assert_ne!(result.document.id, created.document.id);
        assert_eq!(result.page_indices, vec![0]);
        let info = e.inspect_form_frame(&result.document.id, 0, 0).unwrap();
        assert!((info.width - 2.25).abs() < 0.0001);
        assert_eq!(info.border.preview.as_deref(), Some("#0033CC"));
        assert_eq!(
            serde_json::to_value(&before).unwrap(),
            serde_json::to_value(e.list_form_fields(&result.document.id, 0).unwrap()).unwrap()
        );
        assert_eq!(
            original,
            e.render_page(&created.document.id, 0, 640).unwrap()
        );
        assert_ne!(
            original,
            e.render_page(&result.document.id, 0, 640).unwrap()
        );
        for id in [&source.id, &created.document.id, &result.document.id] {
            e.close_document(id).unwrap();
        }
    }
}
#[test]
fn physical_width_and_rotations_preserve_shared_widget_answer_on_save() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for rotation in [0, 90, 180, 270] {
        for unit in ["1", "1.7"] {
            let path = fixture(rotation, unit);
            let source_bytes = std::fs::read(&path).unwrap();
            let before = Document::load_mem(&source_bytes).unwrap();
            let source = e.open_document(&path).unwrap();
            let fields = e.list_form_fields(&source.id, 0).unwrap();
            let result = e
                .set_form_frame_occurrence(&source.id, &[0, 1, 2], 0, request(&e, &source.id, 0, 0))
                .unwrap();
            assert!(
                (e.inspect_form_frame(&result.document.id, 0, 0)
                    .unwrap()
                    .width
                    - 2.25)
                    .abs()
                    < 0.0001
            );
            assert_eq!(
                serde_json::to_value(&fields).unwrap(),
                serde_json::to_value(e.list_form_fields(&result.document.id, 0).unwrap()).unwrap()
            );
            let output = temp.path().join(format!("r{rotation}-u{unit}.pdf"));
            export(&e, &result.document, &output);
            let saved = Document::load(&output).unwrap();
            assert_eq!(
                normal(&before, 1, 0).get_plain_content().unwrap(),
                normal(&saved, 1, 0).get_plain_content().unwrap()
            );
            let reopened = e.open_document(&output).unwrap();
            assert!(
                (e.inspect_form_frame(&reopened.id, 0, 0).unwrap().width - 2.25).abs() < 0.0001
            );
            let label = format!("physical-r{rotation}-u{unit}");
            record_probe(&format!("{label}-before.pdf"), &source_bytes);
            record_probe(
                &format!("{label}-after.pdf"),
                &std::fs::read(&output).unwrap(),
            );
            record_probe(
                &format!("{label}.json"),
                &serde_json::to_vec_pretty(&serde_json::json!({
                    "rotation":rotation,"unit":unit,"displayBounds":fields[0].bounds,
                    "pageWidth":source.pages[0].width,"pageHeight":source.pages[0].height,
                    "borderWidth":2.25
                }))
                .unwrap(),
            );
            assert_eq!(std::fs::read(path).unwrap(), source_bytes);
            for id in [&source.id, &result.document.id, &reopened.id] {
                e.close_document(id).unwrap();
            }
        }
    }
}
#[test]
fn duplicate_occurrences_change_only_selected_frame_and_keep_source() {
    let e = engine();
    let source = e.open_document(fixture(0, "1")).unwrap();
    let old = e.render_page(&source.id, 0, 640).unwrap();
    for selected in [0, 2] {
        let result = e
            .set_form_frame_occurrence(
                &source.id,
                &[0, 1, 0, 2],
                selected,
                request(&e, &source.id, 0, 0),
            )
            .unwrap();
        let target = result.page_indices[selected];
        let other = result.page_indices[if selected == 0 { 2 } else { 0 }];
        assert_ne!(target, other);
        assert_eq!(result.document.pages.len(), 4);
        assert!(
            (e.inspect_form_frame(&result.document.id, target, 0)
                .unwrap()
                .width
                - 2.25)
                .abs()
                < 0.0001
        );
        assert_eq!(old, e.render_page(&result.document.id, other, 640).unwrap());
        assert_eq!(old, e.render_page(&source.id, 0, 640).unwrap());
        assert_eq!(
            e.list_form_fields(&result.document.id, target).unwrap()[0].value,
            e.list_form_fields(&result.document.id, other).unwrap()[0].value
        );
        e.close_document(&result.document.id).unwrap();
    }
    e.close_document(&source.id).unwrap();
}
#[test]
fn stale_identity_invalid_width_and_selection_refuse_without_source_change() {
    let e = engine();
    let source = e.open_document(fixture(0, "1")).unwrap();
    let old = e.render_page(&source.id, 0, 640).unwrap();
    let good = request(&e, &source.id, 0, 0);
    for case in 0..6 {
        let mut req = good.clone();
        match case {
            0 => req.expected_bounds.x += 10.,
            1 => {
                req.expected_value = folio_engine::FormFieldValue::Text {
                    text: "stale".into(),
                }
            }
            2 => req.expected_style_key = "0".repeat(64),
            3 => req.changes.width = Some(f32::NAN),
            4 => req.changes.width = Some(32.1),
            _ => req.annotation_index = 100,
        }
        assert!(e
            .set_form_frame_occurrence(&source.id, &[0, 1, 2], 0, req)
            .is_err());
    }
    for (occurrences, selected) in [(vec![], 0), (vec![0], 1), (vec![0, 99], 0)] {
        assert!(e
            .set_form_frame_occurrence(&source.id, &occurrences, selected, good.clone())
            .is_err());
    }
    assert_eq!(old, e.render_page(&source.id, 0, 640).unwrap());
    e.close_document(&source.id).unwrap();
}
#[test]
fn border_edits_require_content_and_annotation_rights_and_copy_assembly() {
    use lopdf::encryption::Permissions;
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let bytes = std::fs::read(fixture(0, "1")).unwrap();
    for (i, rights) in [
        Permissions::FILLABLE,
        Permissions::MODIFIABLE,
        Permissions::ANNOTABLE,
        Permissions::MODIFIABLE | Permissions::ANNOTABLE,
    ]
    .into_iter()
    .enumerate()
    {
        let protected = protected_fixture::encrypt(&bytes, rights);
        let path = temp.path().join(format!("p{i}.pdf"));
        std::fs::write(&path, &protected).unwrap();
        let source = e.open_document_with_password(&path, "reader").unwrap();
        if rights.contains(Permissions::MODIFIABLE | Permissions::ANNOTABLE) {
            let req = request(&e, &source.id, 0, 0);
            let result = e
                .set_form_frame_occurrence(&source.id, &[0, 1, 2], 0, req.clone())
                .unwrap();
            assert_eq!(result.document.protection, source.protection);
            e.close_document(&result.document.id).unwrap();
            assert!(e
                .set_form_frame_occurrence(&source.id, &[0, 0], 1, req)
                .is_err());
        } else {
            assert!(e.inspect_form_frame(&source.id, 0, 0).is_err());
        }
        e.close_document(&source.id).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), protected);
    }
}

fn authored(e: &PdfEngine, field: NewFormField) -> DocumentInfo {
    let blank = e.create_blank_document(320., 440.).unwrap();
    let created = e
        .create_form_field_occurrence(
            &blank.id,
            &[0],
            0,
            CreateFormFieldRequest {
                bounds: AnnotationRect {
                    x: 40.,
                    y: 50.,
                    width: 180.,
                    height: 60.,
                },
                rotation: 0,
                name: "answer".into(),
                label: "Answer".into(),
                required: false,
                font_size: 12.,
                field,
            },
        )
        .unwrap();
    e.close_document(&blank.id).unwrap();
    created.document
}
fn fill(
    e: &PdfEngine,
    doc: &DocumentInfo,
    index: usize,
    value: folio_engine::FormFieldValue,
) -> DocumentInfo {
    let old = e
        .list_form_fields(&doc.id, 0)
        .unwrap()
        .into_iter()
        .find(|f| f.annotation_index == index)
        .unwrap();
    let context = format!("{} {:?} -> {:?}", old.kind, old.value, value);
    e.fill_form_field(
        &doc.id,
        folio_engine::FillFormFieldRequest {
            page_index: 0,
            annotation_index: index,
            expected_value: old.value,
            value,
        },
    )
    .unwrap_or_else(|error| {
        if let Some(root) = std::env::var_os("FOLIO_FRAME_ENGINE_PROBE") {
            let root = PathBuf::from(root);
            std::fs::create_dir_all(&root).unwrap();
            let path = root.join("failed-fill.pdf");
            assert!(!path.exists());
            export(e, doc, &path);
        }
        panic!("{context}: {error}")
    })
}
#[test]
fn edited_frames_survive_fill_clear_refill_and_saved_reopen_for_all_kinds() {
    use folio_engine::FormFieldValue::{Choice, Text, Toggle};
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for (case, field, answers) in [
        (
            "text",
            NewFormField::Text { multiline: false },
            vec![
                (
                    0,
                    Text {
                        text: "Saved answer".into(),
                    },
                ),
                (0, Text { text: "".into() }),
                (
                    0,
                    Text {
                        text: "Saved again".into(),
                    },
                ),
            ],
        ),
        (
            "check",
            NewFormField::Checkbox,
            vec![
                (0, Toggle { checked: true }),
                (0, Toggle { checked: false }),
                (0, Toggle { checked: true }),
            ],
        ),
        (
            "combo",
            NewFormField::Dropdown {
                options: vec!["One".into(), "Two".into()],
            },
            vec![
                (0, Choice { indices: vec![1] }),
                (0, Choice { indices: vec![0] }),
                (0, Choice { indices: vec![1] }),
            ],
        ),
        (
            "list",
            NewFormField::List {
                options: vec!["One".into(), "Two".into()],
                multi_select: true,
            },
            vec![
                (
                    0,
                    Choice {
                        indices: vec![0, 1],
                    },
                ),
                (0, Choice { indices: vec![] }),
                (0, Choice { indices: vec![1] }),
            ],
        ),
        (
            "radio",
            NewFormField::Radio {
                options: vec!["Yes".into(), "No".into()],
            },
            vec![
                (0, Toggle { checked: true }),
                (1, Toggle { checked: true }),
                (0, Toggle { checked: true }),
            ],
        ),
    ] {
        let source = authored(&e, field);
        let styled = e
            .set_form_frame_occurrence(&source.id, &[0], 0, request(&e, &source.id, 0, 0))
            .unwrap();
        let mut current = styled.document;
        for (index, value) in answers {
            let next = fill(&e, &current, index, value.clone());
            assert_eq!(
                e.list_form_fields(&next.id, 0)
                    .unwrap()
                    .into_iter()
                    .find(|f| f.annotation_index == index)
                    .unwrap()
                    .value,
                value,
                "{case}"
            );
            let frame = e.inspect_form_frame(&next.id, 0, 0).unwrap();
            assert!((frame.width - 2.25).abs() < 0.0001, "{case}");
            assert_eq!(frame.border.preview.as_deref(), Some("#0033CC"), "{case}");
            if next.id != current.id {
                e.close_document(&current.id).unwrap();
            }
            current = next;
        }
        let same = e.list_form_fields(&current.id, 0).unwrap()[0].value.clone();
        let pixels = e.render_page(&current.id, 0, 640).unwrap();
        let unchanged = fill(&e, &current, 0, same);
        assert_eq!(
            pixels,
            e.render_page(&unchanged.id, 0, 640).unwrap(),
            "same-value {case}"
        );
        let out = temp.path().join(format!("{case}.pdf"));
        export(&e, &current, &out);
        let reopened = e.open_document(&out).unwrap();
        assert!((e.inspect_form_frame(&reopened.id, 0, 0).unwrap().width - 2.25).abs() < 0.0001);
        assert_eq!(
            pixels,
            e.render_page(&reopened.id, 0, 640).unwrap(),
            "saved {case}"
        );
        if let Some(root) = std::env::var_os("FOLIO_FRAME_ENGINE_PROBE") {
            let root = PathBuf::from(root);
            std::fs::create_dir_all(&root).unwrap();
            let path = root.join(format!("{case}.pdf"));
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .unwrap();
            std::io::Write::write_all(&mut file, &std::fs::read(&out).unwrap()).unwrap();
        }
        for id in [&source.id, &current.id, &reopened.id] {
            e.close_document(id).unwrap();
        }
        if unchanged.id != current.id {
            e.close_document(&unchanged.id).unwrap();
        }
    }
}
#[test]
fn each_rectangular_style_and_transparency_survives_later_text_filling() {
    use folio_engine::{FormFieldValue, FrameStyle};
    let e = engine();
    for style in [
        FrameStyle::Solid,
        FrameStyle::Dashed,
        FrameStyle::Underline,
        FrameStyle::Beveled,
        FrameStyle::Inset,
    ] {
        for hidden in [false, true] {
            let source = authored(&e, NewFormField::Text { multiline: false });
            let mut req = request(&e, &source.id, 0, 0);
            req.changes.style = Some(style);
            req.changes.background = Some(FrameColorChange::Transparent);
            if hidden {
                req.changes.border = Some(FrameColorChange::Transparent);
            }
            let styled = e
                .set_form_frame_occurrence(&source.id, &[0], 0, req)
                .unwrap();
            let filled = fill(
                &e,
                &styled.document,
                0,
                FormFieldValue::Text {
                    text: "Changed answer".into(),
                },
            );
            let info = e.inspect_form_frame(&filled.id, 0, 0).unwrap();
            assert_eq!(info.style, Some(style));
            assert!((info.width - 2.25).abs() < 0.0001);
            assert_eq!(info.background.color_space, "transparent");
            if hidden {
                assert_eq!(info.border.color_space, "transparent");
            } else {
                assert_eq!(info.border.preview.as_deref(), Some("#0033CC"));
            }
            for id in [&source.id, &styled.document.id, &filled.id] {
                e.close_document(id).unwrap();
            }
        }
    }
}

#[test]
fn later_text_appearance_reflow_and_scaling_keep_fractional_frame() {
    use folio_engine::{
        FormFieldValue, PositionFormFieldRequest, SetFormTextAppearanceRequest, TextFieldAlignment,
        TextFieldAppearanceChanges,
    };
    let e = engine();
    let source = authored(&e, NewFormField::Text { multiline: false });
    let framed = e
        .set_form_frame_occurrence(&source.id, &[0], 0, request(&e, &source.id, 0, 0))
        .unwrap();
    let filled = fill(
        &e,
        &framed.document,
        0,
        FormFieldValue::Text {
            text: "Original answer".into(),
        },
    );
    let before = e.list_form_fields(&filled.id, 0).unwrap().remove(0);
    let style = e.inspect_form_text_appearance(&filled.id, 0, 0).unwrap();
    let styled = e
        .set_form_text_appearance_occurrence(
            &filled.id,
            &[0],
            0,
            SetFormTextAppearanceRequest {
                annotation_index: 0,
                expected_bounds: before.bounds.clone().unwrap(),
                expected_value: before.value.clone(),
                expected_style_key: style.style_key,
                changes: TextFieldAppearanceChanges {
                    font_size: Some(16.),
                    color: None,
                    alignment: Some(TextFieldAlignment::Center),
                },
            },
        )
        .unwrap();
    assert_eq!(
        e.inspect_form_frame(&styled.document.id, 0, 0)
            .unwrap()
            .border
            .preview
            .as_deref(),
        Some("#0033CC")
    );
    let before = e
        .list_form_fields(&styled.document.id, 0)
        .unwrap()
        .remove(0);
    let old = before.bounds.unwrap();
    let mut bounds = old.clone();
    bounds.width *= 1.1;
    bounds.height *= 1.1;
    let pixels = e.render_page(&styled.document.id, 0, 640).unwrap();
    let resized = e
        .position_form_field_occurrence(
            &styled.document.id,
            &[0],
            0,
            PositionFormFieldRequest {
                annotation_index: 0,
                expected_bounds: old.clone(),
                expected_value: before.value.clone(),
                bounds: bounds.clone(),
                reflow_text: true,
            },
        )
        .unwrap();
    assert!(
        (e.inspect_form_frame(&resized.document.id, 0, 0)
            .unwrap()
            .width
            - 2.25)
            .abs()
            < 0.0001
    );
    e.close_document(&resized.document.id).unwrap();
    assert_eq!(pixels, e.render_page(&styled.document.id, 0, 640).unwrap());
    let resized = e
        .position_form_field_occurrence(
            &styled.document.id,
            &[0],
            0,
            PositionFormFieldRequest {
                annotation_index: 0,
                expected_bounds: old,
                expected_value: before.value,
                bounds,
                reflow_text: false,
            },
        )
        .unwrap();
    let frame = e.inspect_form_frame(&resized.document.id, 0, 0).unwrap();
    assert!((frame.width - 2.475).abs() < 0.0001);
    assert_eq!(frame.border.preview.as_deref(), Some("#0033CC"));
    assert_eq!(
        e.list_form_fields(&resized.document.id, 0).unwrap()[0].value,
        FormFieldValue::Text {
            text: "Original answer".into()
        }
    );
    for id in [
        &source.id,
        &framed.document.id,
        &filled.id,
        &styled.document.id,
        &resized.document.id,
    ] {
        e.close_document(id).unwrap();
    }
}

#[test]
fn fractional_frame_does_not_allow_reflow_to_erase_custom_artwork() {
    reflow_preserves_or_refuses_artwork(b"\nq 1 0 0 rg 140 40 10 10 re f Q\n");
}
#[test]
fn reflow_does_not_erase_a_caption_outside_the_answer_marker() {
    reflow_preserves_or_refuses_artwork(b"\nq BT 0 g /F0 12 Tf 1 0 0 1 100 40 Tm (Approved) Tj ET Q\n");
}
#[test]
fn background_edit_preserves_dashed_radio_paths_in_every_appearance_state() {
    use lopdf::{content::Content, dictionary, Object};
    let e=engine();let temp=tempfile::tempdir().unwrap();
    let s=authored(&e,NewFormField::Radio{options:vec!["Yes".into(),"No".into()]});
    let path=temp.path().join("radio.pdf");export(&e,&s,&path);
    let mut d=Document::load(&path).unwrap();
    let pid=*d.get_pages().values().next().unwrap();
    let w_id=d.get_dictionary(pid).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap();
    let ap=d.get_dictionary(w_id).unwrap().get(b"AP").unwrap().as_dict().unwrap().clone();
    let states=ap.get(b"N").unwrap().as_dict().unwrap().clone();
    let circle=Content::decode_strict(b"[3 2] 1.5 d 3 15 m 3 17.761424 5.238576 20 8 20 c 10.761424 20 13 17.761424 13 15 c 13 12.238576 10.761424 10 8 10 c 5.238576 10 3 12.238576 3 15 c s").unwrap();
    let mut originals=std::collections::BTreeMap::new();
    for (name,value) in states.iter(){
        let stream=d.get_object_mut(value.as_reference().unwrap()).unwrap().as_stream_mut().unwrap();
        let mut c=Content::decode_strict(&stream.get_plain_content().unwrap()).unwrap();
        let start=c.operations.iter().position(|o|o.operator=="m").unwrap();
        let end=c.operations.iter().position(|o|o.operator=="S"||o.operator=="s").unwrap();
        c.operations.splice(start..=end,circle.operations.clone());
        let bytes=c.encode().unwrap();stream.set_plain_content(bytes.clone());originals.insert(name.clone(),bytes);
    }
    let w=d.get_object_mut(w_id).unwrap().as_dict_mut().unwrap();
    w.set("AP",dictionary!{"N"=>states.clone(),"R"=>states.clone(),"D"=>states});
    w.set("BS",dictionary!{"W"=>1,"S"=>"D","D"=>Object::Array(vec![3.into(),2.into()])});
    d.save(&path).unwrap();let input=std::fs::read(&path).unwrap();
    let opened=e.open_document(&path).unwrap();
    let mut r=request(&e,&opened.id,0,0);r.changes=FormFrameChanges{background:Some(FrameColorChange::Rgb("#FFEEDD".into())),..Default::default()};
    let result=e.set_form_frame_occurrence(&opened.id,&[0],0,r).unwrap();
    let output=temp.path().join("background.pdf");export(&e,&result.document,&output);
    let result_doc=Document::load(&output).unwrap();let p=*result_doc.get_pages().values().next().unwrap();
    let annots=result_doc.get_dictionary(p).unwrap().get(b"Annots").unwrap().as_array().unwrap();
    let w=result_doc.get_dictionary(annots[0].as_reference().unwrap()).unwrap();
    let ap=w.get(b"AP").unwrap().as_dict().unwrap();
    for mode in [b"N",b"R",b"D"] {
        for (name,ref_) in ap.get(mode).unwrap().as_dict().unwrap().iter(){
            let bytes=result_doc.get_object(ref_.as_reference().unwrap()).unwrap().as_stream().unwrap().get_plain_content().unwrap();
            assert!(bytes.ends_with(&originals[name]),"unchanged {mode:?}/{name:?} border/caption bytes changed");
        }
    }
    assert_eq!(std::fs::read(&path).unwrap(),input);
    record_probe("dashed-radio-before.pdf",&input);record_probe("dashed-radio-after.pdf",&std::fs::read(&output).unwrap());
    for id in [&s.id,&opened.id,&result.document.id]{e.close_document(id).unwrap();}
}
fn reflow_preserves_or_refuses_artwork(artwork: &[u8]) {
    use folio_engine::{FormFieldValue, PositionFormFieldRequest};
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let source = authored(&e, NewFormField::Text { multiline: false });
    let filled = fill(
        &e,
        &source,
        0,
        FormFieldValue::Text {
            text: "Keep answer".into(),
        },
    );
    let framed = e
        .set_form_frame_occurrence(&filled.id, &[0], 0, request(&e, &filled.id, 0, 0))
        .unwrap();
    let path = temp.path().join("base.pdf");
    export(&e, &framed.document, &path);
    let mut d = Document::load(&path).unwrap();
    let page = *d.get_pages().values().next().unwrap();
    let annots = d
        .dereference(d.get_dictionary(page).unwrap().get(b"Annots").unwrap())
        .unwrap()
        .1
        .as_array()
        .unwrap();
    let w = d.dereference(&annots[0]).unwrap().1.as_dict().unwrap();
    let ap = d
        .dereference(w.get(b"AP").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    let sid = ap.get(b"N").unwrap().as_reference().unwrap();
    let stream = d.get_object_mut(sid).unwrap().as_stream_mut().unwrap();
    let mut content = stream.get_plain_content().unwrap();
    content.extend_from_slice(artwork);
    stream.set_plain_content(content.clone());
    let path = temp.path().join("artwork.pdf");
    d.save(&path).unwrap();
    let input = std::fs::read(&path).unwrap();
    let opened = e.open_document(&path).unwrap();
    assert_eq!(e.inspect_form_frame(&opened.id, 0, 0).unwrap().width, 2.25);
    let f = e.list_form_fields(&opened.id, 0).unwrap().remove(0);
    let old = f.bounds.unwrap();
    let mut bounds = old.clone();
    bounds.width += 20.;
    let req = PositionFormFieldRequest {
        annotation_index: 0,
        expected_bounds: old,
        expected_value: f.value,
        bounds,
        reflow_text: true,
    };
    let result = e.position_form_field_occurrence(&opened.id, &[0], 0, req.clone());
    assert!(
        result.is_err(),
        "reflow must not erase saved artwork"
    );
    let mut req = req;
    req.reflow_text = false;
    let scaled = e
        .position_form_field_occurrence(&opened.id, &[0], 0, req)
        .unwrap();
    let output = temp.path().join("scaled.pdf");
    export(&e, &scaled.document, &output);
    assert_eq!(
        normal(&Document::load(&output).unwrap(), 0, 0)
            .get_plain_content()
            .unwrap(),
        content
    );
    assert_eq!(std::fs::read(&path).unwrap(), input);
    for id in [
        &source.id,
        &filled.id,
        &framed.document.id,
        &opened.id,
        &scaled.document.id,
    ] {
        e.close_document(id).unwrap();
    }
}
