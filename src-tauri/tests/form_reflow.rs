use folio_engine::{
    AnnotationRect, CreateFormFieldRequest, FillFormFieldRequest, FormFieldValue, PdfEngine,
    PositionFormFieldRequest,
};
use std::path::{Path, PathBuf};
fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
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
fn source(e: &PdfEngine) -> folio_engine::DocumentInfo {
    let blank = e.create_blank_document(400., 500.).unwrap();
    let r:CreateFormFieldRequest=serde_json::from_value(serde_json::json!({"bounds":{"x":20,"y":30,"width":300,"height":80},"rotation":0,"name":"paragraph","label":"Paragraph","required":false,"fontSize":12,"field":{"kind":"text","multiline":true}})).unwrap();
    let created = e
        .create_form_field_occurrence(&blank.id, &[0], 0, r)
        .unwrap()
        .document;
    let f = e.list_form_fields(&created.id, 0).unwrap().remove(0);
    let value=FormFieldValue::Text{text:"Alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega".into()};
    let filled = e
        .fill_form_field(
            &created.id,
            FillFormFieldRequest {
                page_index: 0,
                annotation_index: f.annotation_index,
                expected_value: f.value,
                value,
            },
        )
        .unwrap();
    for id in [&blank.id, &created.id] {
        e.close_document(id).unwrap();
    }
    filled
}
fn normal(path: &Path) -> lopdf::Stream {
    let d = lopdf::Document::load(path).unwrap();
    let p = d.get_dictionary(*d.get_pages().get(&1).unwrap()).unwrap();
    let a = d
        .dereference(p.get(b"Annots").unwrap())
        .unwrap()
        .1
        .as_array()
        .unwrap();
    let w = d.dereference(&a[0]).unwrap().1.as_dict().unwrap();
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
        .clone()
}
#[test]
fn probe_missing_appearance_regenerates_a_resized_multiline_field() {
    let e = engine();
    let s = source(&e);
    let f = e.list_form_fields(&s.id, 0).unwrap().remove(0);
    let r = PositionFormFieldRequest {
        reflow_text: false,
        annotation_index: 0,
        expected_bounds: f.bounds.clone().unwrap(),
        expected_value: f.value.clone(),
        bounds: AnnotationRect {
            x: 20.,
            y: 30.,
            width: 120.,
            height: 180.,
        },
    };
    let resized = e
        .position_form_field_occurrence(&s.id, &[0], 0, r)
        .unwrap()
        .document;
    let temp = tempfile::tempdir().unwrap();
    let before = temp.path().join("before.pdf");
    let after = temp.path().join("after.pdf");
    export(&e, &resized, &before);
    let mut d = lopdf::Document::load(&before).unwrap();
    let page = d.get_dictionary(*d.get_pages().get(&1).unwrap()).unwrap();
    let a = d
        .dereference(page.get(b"Annots").unwrap())
        .unwrap()
        .1
        .as_array()
        .unwrap();
    let widget = a[0].as_reference().unwrap();
    d.get_dictionary_mut(widget).unwrap().remove(b"AP");
    let missing = temp.path().join("missing.pdf");
    d.save(&missing).unwrap();
    let probe = e.open_document(&missing).unwrap();
    let unchanged = e
        .fill_form_field(
            &probe.id,
            FillFormFieldRequest {
                page_index: 0,
                annotation_index: 0,
                expected_value: f.value.clone(),
                value: f.value.clone(),
            },
        )
        .unwrap();
    assert_eq!(
        e.source_bytes(&unchanged.id).unwrap(),
        e.source_bytes(&probe.id).unwrap(),
        "unchanged fill must preserve a missing appearance too"
    );
    let cleared = e
        .fill_form_field(
            &probe.id,
            FillFormFieldRequest {
                page_index: 0,
                annotation_index: 0,
                expected_value: f.value.clone(),
                value: FormFieldValue::Text {
                    text: String::new(),
                },
            },
        )
        .unwrap();
    let filled = e
        .fill_form_field(
            &cleared.id,
            FillFormFieldRequest {
                page_index: 0,
                annotation_index: 0,
                expected_value: FormFieldValue::Text {
                    text: String::new(),
                },
                value: f.value.clone(),
            },
        )
        .unwrap();
    export(&e, &filled, &after);
    let old = normal(&before);
    let new = normal(&after);
    let bbox = new.dict.get(b"BBox").unwrap().as_array().unwrap();
    assert_eq!(bbox[2].as_float().unwrap(), 120.);
    assert_eq!(bbox[3].as_float().unwrap(), 180.);
    assert_ne!(
        old.get_plain_content().unwrap(),
        new.get_plain_content().unwrap()
    );
    let operations = lopdf::content::Content::decode(&new.get_plain_content().unwrap()).unwrap();
    assert!(
        operations.operations.iter().any(|op| op.operator == "Tf"),
        "font-size oracle must observe an actual font instruction"
    );
    assert!(operations
        .operations
        .iter()
        .filter(|op| op.operator == "Tf")
        .all(|op| (op.operands[1].as_float().unwrap() - 12.).abs() < 0.001));
    if let Some(root) = std::env::var_os("FOLIO_FORM_REFLOW_OUTPUT") {
        let root = PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::copy(before, root.join("stretched.pdf")).unwrap();
        std::fs::copy(after, root.join("refilled.pdf")).unwrap();
    }
    for id in [
        &s.id,
        &resized.id,
        &probe.id,
        &unchanged.id,
        &cleared.id,
        &filled.id,
    ] {
        e.close_document(id).unwrap();
    }
}

#[test]
fn public_resize_reflows_text_with_original_font_size() {
    let e = engine();
    let s = source(&e);
    let f = e.list_form_fields(&s.id, 0).unwrap().remove(0);
    let r:PositionFormFieldRequest=serde_json::from_value(serde_json::json!({"annotationIndex":0,"expectedBounds":f.bounds,"expectedValue":f.value,"bounds":{"x":20,"y":30,"width":120,"height":180},"reflowText":true})).unwrap();
    let reflow = e
        .position_form_field_occurrence(&s.id, &[0], 0, r)
        .unwrap()
        .document;
    assert_eq!(e.list_form_fields(&reflow.id, 0).unwrap()[0].value, f.value);
    let temp = tempfile::tempdir().unwrap();
    let out = temp.path().join("reflow.pdf");
    export(&e, &reflow, &out);
    let ap = normal(&out);
    let bbox = ap.dict.get(b"BBox").unwrap().as_array().unwrap();
    assert_eq!(bbox[2].as_float().unwrap(), 120.);
    assert_eq!(bbox[3].as_float().unwrap(), 180.);
    let ops = lopdf::content::Content::decode(&ap.get_plain_content().unwrap()).unwrap();
    let sizes: Vec<_> = ops
        .operations
        .iter()
        .filter(|op| op.operator == "Tf")
        .map(|op| op.operands[1].as_float().unwrap())
        .collect();
    assert!(!sizes.is_empty());
    assert!(sizes.iter().all(|v| (*v - 12.).abs() < 0.001));
    if let Some(root) = std::env::var_os("FOLIO_FORM_REFLOW_OUTPUT") {
        let root = PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::copy(&out, root.join("public-reflow.pdf")).unwrap();
    }
    for id in [&s.id, &reflow.id] {
        e.close_document(id).unwrap();
    }
}

fn widget(d: &lopdf::Document, page: usize) -> lopdf::ObjectId {
    let p = d
        .get_dictionary(*d.get_pages().values().nth(page).unwrap())
        .unwrap();
    d.dereference(p.get(b"Annots").unwrap())
        .unwrap()
        .1
        .as_array()
        .unwrap()[0]
        .as_reference()
        .unwrap()
}
fn field_parent(d: &lopdf::Document, w: lopdf::ObjectId) -> lopdf::ObjectId {
    d.get_dictionary(w)
        .unwrap()
        .get(b"Parent")
        .ok()
        .and_then(|v| v.as_reference().ok())
        .unwrap_or(w)
}
fn reflow_request(e: &PdfEngine, id: &str, page: usize) -> PositionFormFieldRequest {
    let f = e.list_form_fields(id, page).unwrap().remove(0);
    PositionFormFieldRequest {
        annotation_index: 0,
        expected_bounds: f.bounds.unwrap(),
        expected_value: f.value,
        bounds: AnnotationRect {
            x: 20.,
            y: 30.,
            width: 120.,
            height: 180.,
        },
        reflow_text: true,
    }
}
fn ap_at(d: &lopdf::Document, page: usize) -> Vec<u8> {
    let w = d.get_dictionary(widget(d, page)).unwrap();
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
}
#[test]
fn duplicate_reflow_changes_only_selected_appearance_and_move_keeps_it_exact() {
    let e = engine();
    let s = source(&e);
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().join("base.pdf");
    export(&e, &s, &base);
    let original = normal(&base).get_plain_content().unwrap();
    for selected in [0, 1] {
        let changed = e
            .position_form_field_occurrence(&s.id, &[0, 0], selected, reflow_request(&e, &s.id, 0))
            .unwrap();
        let out = temp.path().join("copy.pdf");
        export(&e, &changed.document, &out);
        let d = lopdf::Document::load(&out).unwrap();
        assert_eq!(ap_at(&d, 1 - selected), original);
        assert_ne!(ap_at(&d, selected), original);
        let new = ap_at(&d, selected);
        let mut r = reflow_request(&e, &changed.document.id, selected);
        r.bounds = r.expected_bounds.clone();
        r.bounds.x += 5.;
        let moved = e
            .position_form_field_occurrence(&changed.document.id, &[0, 1], selected, r)
            .unwrap();
        export(&e, &moved.document, &out);
        let d = lopdf::Document::load(&out).unwrap();
        assert_eq!(ap_at(&d, selected), new);
        assert_eq!(ap_at(&d, 1 - selected), original);
        for id in [&changed.document.id, &moved.document.id] {
            e.close_document(id).unwrap();
        }
    }
    e.close_document(&s.id).unwrap();
}
#[test]
fn empty_readonly_autosize_and_singleline_fields_keep_settings() {
    let e = engine();
    let s = source(&e);
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().join("base.pdf");
    export(&e, &s, &base);
    for case in ["empty", "readonly", "auto", "single", "right", "password"] {
        let mut d = lopdf::Document::load(&base).unwrap();
        let w = widget(&d, 0);
        let parent = field_parent(&d, w);
        match case {
            "empty" => {
                d.get_dictionary_mut(parent)
                    .unwrap()
                    .set("V", lopdf::Object::string_literal(""));
            }
            "readonly" => {
                d.get_dictionary_mut(parent).unwrap().set("Ff", 4097);
            }
            "auto" => {
                d.get_dictionary_mut(w)
                    .unwrap()
                    .set("DA", lopdf::Object::string_literal("/FolioField0 0 Tf 0 g"));
            }
            "single" => {
                d.get_dictionary_mut(parent).unwrap().set("Ff", 0);
            }
            "right" => {
                d.get_dictionary_mut(parent).unwrap().set("Q", 2);
            }
            _ => {
                d.get_dictionary_mut(parent).unwrap().set("Ff", 8192);
            }
        }
        let input = temp.path().join(format!("{case}.pdf"));
        d.save(&input).unwrap();
        let opened = e.open_document(&input).unwrap();
        let before = e.list_form_fields(&opened.id, 0).unwrap().remove(0);
        let result = e
            .position_form_field_occurrence(&opened.id, &[0], 0, reflow_request(&e, &opened.id, 0))
            .unwrap_or_else(|err| panic!("{case}: {err}"));
        let after = e
            .list_form_fields(&result.document.id, 0)
            .unwrap()
            .remove(0);
        assert_eq!(before.value, after.value);
        assert_eq!(before.read_only, after.read_only);
        assert_eq!(before.multiline, after.multiline);
        assert_eq!(before.password, after.password);
        let out = temp.path().join(format!("{case}-after.pdf"));
        export(&e, &result.document, &out);
        let saved = e.open_document(&out).unwrap();
        assert_eq!(
            e.list_form_fields(&saved.id, 0).unwrap()[0].value,
            before.value
        );
        if let Some(root) = std::env::var_os("FOLIO_FORM_REFLOW_OUTPUT") {
            std::fs::copy(&out, PathBuf::from(root).join(format!("{case}-after.pdf"))).unwrap();
        }
        for id in [&opened.id, &result.document.id, &saved.id] {
            e.close_document(id).unwrap();
        }
    }
    e.close_document(&s.id).unwrap();
}
#[test]
fn local_appearance_font_wins_over_conflicting_global_default() {
    let e = engine();
    let s = source(&e);
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("font.pdf");
    export(&e, &s, &path);
    let mut d = lopdf::Document::load(&path).unwrap();
    let w = widget(&d, 0);
    let ap = d
        .dereference(d.get_dictionary(w).unwrap().get(b"AP").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    let normal = ap.get(b"N").unwrap().as_reference().unwrap();
    let courier=d.add_object(lopdf::dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier","Encoding"=>"WinAnsiEncoding"});
    d.get_object_mut(normal)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .dict
        .set(
            "Resources",
            lopdf::dictionary! {"Font"=>lopdf::dictionary!{"FolioField0"=>courier}},
        );
    d.save(&path).unwrap();
    let opened = e.open_document(&path).unwrap();
    let changed = e
        .position_form_field_occurrence(&opened.id, &[0], 0, reflow_request(&e, &opened.id, 0))
        .unwrap();
    let out = temp.path().join("after.pdf");
    export(&e, &changed.document, &out);
    let d = lopdf::Document::load(&out).unwrap();
    let ap = normal_from(&d, 0);
    let resources = d
        .dereference(ap.dict.get(b"Resources").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    let fonts = d
        .dereference(resources.get(b"Font").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    assert!(fonts.iter().all(|(_, v)| d
        .dereference(v)
        .unwrap()
        .1
        .as_dict()
        .unwrap()
        .get(b"BaseFont")
        .unwrap()
        .as_name()
        .unwrap()
        == b"Courier"));
    for id in [&s.id, &opened.id, &changed.document.id] {
        e.close_document(id).unwrap();
    }
}
fn normal_from(d: &lopdf::Document, page: usize) -> &lopdf::Stream {
    let w = d.get_dictionary(widget(d, page)).unwrap();
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
fn repeated_reflow_prunes_replaced_appearance_graphs() {
    let e = engine();
    let mut current = source(&e);
    let temp = tempfile::tempdir().unwrap();
    let out = temp.path().join("output.pdf");
    let mut counts = vec![];
    for i in 0..12 {
        let mut r = reflow_request(&e, &current.id, 0);
        r.bounds.width += if i % 2 == 0 { 0. } else { 20. };
        let next = e
            .position_form_field_occurrence(&current.id, &[0], 0, r)
            .unwrap()
            .document;
        e.close_document(&current.id).unwrap();
        current = next;
        export(&e, &current, &out);
        counts.push(lopdf::Document::load(&out).unwrap().objects.len());
    }
    assert!(
        counts.iter().max().unwrap() - counts.iter().min().unwrap() <= 2,
        "retained graphs: {counts:?}"
    );
    e.close_document(&current.id).unwrap();
}
#[test]
fn rotated_pages_and_physical_units_keep_original_raw_font_size() {
    let e = engine();
    let s = source(&e);
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().join("base.pdf");
    export(&e, &s, &base);
    for rotation in [0, 90, 180, 270] {
        for unit in [1., 1.7, 3.] {
            let mut d = lopdf::Document::load(&base).unwrap();
            let page = *d.get_pages().get(&1).unwrap();
            let p = d.get_dictionary_mut(page).unwrap();
            p.set("Rotate", rotation);
            p.set("UserUnit", unit);
            p.set(
                "CropBox",
                vec![10.into(), 10.into(), 390.into(), 490.into()],
            );
            let input = temp.path().join("rotated.pdf");
            d.save(&input).unwrap();
            let opened = e.open_document(&input).unwrap();
            let r = reflow_request(&e, &opened.id, 0);
            let changed = e
                .position_form_field_occurrence(&opened.id, &[0], 0, r)
                .unwrap();
            let out = temp.path().join("out.pdf");
            export(&e, &changed.document, &out);
            let ap = normal(&out);
            let ops = lopdf::content::Content::decode(&ap.get_plain_content().unwrap()).unwrap();
            assert!(ops
                .operations
                .iter()
                .filter(|op| op.operator == "Tf")
                .all(|op| (op.operands[1].as_float().unwrap() - 12.).abs() < 0.001));
            if let Some(root) = std::env::var_os("FOLIO_FORM_REFLOW_OUTPUT") {
                let root = PathBuf::from(root);
                std::fs::copy(&input, root.join(format!("before-r{rotation}-u{unit}.pdf")))
                    .unwrap();
                std::fs::copy(&out, root.join(format!("after-r{rotation}-u{unit}.pdf"))).unwrap();
            }
            for id in [&opened.id, &changed.document.id] {
                e.close_document(id).unwrap();
            }
        }
    }
    e.close_document(&s.id).unwrap();
}

#[test]
fn review_final_default_color_is_preserved_in_reflow_artwork() {
    let e = engine();
    let s = source(&e);
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().join("base.pdf");
    export(&e, &s, &base);
    for colors in ["0 g 1 0 0 rg", "0 1 0 rg 1 0 0 rg"] {
        let mut d = lopdf::Document::load(&base).unwrap();
        let w = widget(&d, 0);
        d.get_dictionary_mut(w).unwrap().set(
            "DA",
            lopdf::Object::string_literal(format!("/FolioField0 12 Tf {colors}")),
        );
        let ap = d
            .dereference(d.get_dictionary(w).unwrap().get(b"AP").unwrap())
            .unwrap()
            .1
            .as_dict()
            .unwrap();
        let normal = ap.get(b"N").unwrap().as_reference().unwrap();
        let old = d
            .get_object(normal)
            .unwrap()
            .as_stream()
            .unwrap()
            .get_plain_content()
            .unwrap();
        let text = String::from_utf8(old).unwrap().replace("0 g", "1 0 0 rg");
        d.get_object_mut(normal)
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .set_plain_content(text.into_bytes());
        let input = temp.path().join("red.pdf");
        d.save(&input).unwrap();
        let opened = e.open_document(&input).unwrap();
        let changed = e
            .position_form_field_occurrence(&opened.id, &[0], 0, reflow_request(&e, &opened.id, 0))
            .unwrap();
        let out = temp.path().join("red-after.pdf");
        export(&e, &changed.document, &out);
        let d = lopdf::Document::load(&out).unwrap();
        let ops = lopdf::content::Content::decode(&normal_from(&d, 0).get_plain_content().unwrap())
            .unwrap();
        let mut color = vec![0., 0., 0.];
        let mut text_colors = vec![];
        for op in ops.operations {
            match op.operator.as_str() {
                "g" => color = vec![op.operands[0].as_float().unwrap(); 3],
                "rg" => color = op.operands.iter().map(|v| v.as_float().unwrap()).collect(),
                "Tj" | "TJ" => text_colors.push(color.clone()),
                _ => {}
            }
        }
        assert!(!text_colors.is_empty());
        assert!(
            text_colors.iter().all(|c| c == &vec![1., 0., 0.]),
            "{colors}: {text_colors:?}"
        );
        assert_eq!(
            d.get_dictionary(widget(&d, 0))
                .unwrap()
                .get(b"DA")
                .unwrap()
                .as_str()
                .unwrap(),
            format!("/FolioField0 12 Tf {colors}").as_bytes()
        );
        if let Some(root) = std::env::var_os("FOLIO_FORM_REFLOW_OUTPUT") {
            std::fs::copy(
                &out,
                PathBuf::from(root).join(if colors.starts_with("0 g") {
                    "red-mixed-after.pdf"
                } else {
                    "red-repeated-after.pdf"
                }),
            )
            .unwrap();
        }
        for id in [&opened.id, &changed.document.id] {
            e.close_document(id).unwrap();
        }
    }
    e.close_document(&s.id).unwrap();
}
#[test]
fn fractional_solid_borders_preserve_width_while_dashed_reflow_refuses() {
    let e = engine();
    let s = source(&e);
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().join("base.pdf");
    export(&e, &s, &base);
    let mut accepted = vec![];
    for (name, bs) in [
        ("fractional", lopdf::dictionary! {"W"=>0.5,"S"=>"S"}),
        (
            "dash",
            lopdf::dictionary! {"W"=>2,"S"=>"D","D"=>vec![1.into(),7.into()]},
        ),
    ] {
        let mut d = lopdf::Document::load(&base).unwrap();
        let w = widget(&d, 0);
        d.get_dictionary_mut(w).unwrap().set("BS", bs);
        d.get_dictionary_mut(w).unwrap().remove(b"Border");
        let ap = d
            .dereference(d.get_dictionary(w).unwrap().get(b"AP").unwrap())
            .unwrap()
            .1
            .as_dict()
            .unwrap();
        let ap_id = ap.get(b"N").unwrap().as_reference().unwrap();
        let old = String::from_utf8(
            d.get_object(ap_id)
                .unwrap()
                .as_stream()
                .unwrap()
                .get_plain_content()
                .unwrap(),
        )
        .unwrap();
        let text = &old[old.find("/Tx BMC").unwrap()..];
        let border = if name == "fractional" {
            "q 0 G 0.5 w 0.25 0.25 299.5 79.5 re S Q\n"
        } else {
            "q 0 G 2 w [1 7] 0 d 1 1 298 78 re S Q\n"
        };
        d.get_object_mut(ap_id)
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .set_plain_content(format!("{border}{text}").into_bytes());
        let input = temp.path().join(format!("{name}.pdf"));
        d.save(&input).unwrap();
        let saved = std::fs::read(&input).unwrap();
        let opened = e.open_document(&input).unwrap();
        let r = reflow_request(&e, &opened.id, 0);
        let reflow = e.position_form_field_occurrence(&opened.id, &[0], 0, r);
        if name == "fractional" {
            let result = reflow.unwrap();
            assert!(
                (e.inspect_form_frame(&result.document.id, 0, 0)
                    .unwrap()
                    .width
                    - 0.5)
                    .abs()
                    < 0.0001
            );
            e.close_document(&result.document.id).unwrap();
        } else if let Ok(result) = reflow {
            accepted.push(name);
            e.close_document(&result.document.id).unwrap();
        }
        assert_eq!(std::fs::read(&input).unwrap(), saved);
        let mut r = reflow_request(&e, &opened.id, 0);
        r.reflow_text = false;
        let scaled = e
            .position_form_field_occurrence(&opened.id, &[0], 0, r)
            .unwrap();
        for id in [&opened.id, &scaled.document.id] {
            e.close_document(id).unwrap();
        }
    }
    e.close_document(&s.id).unwrap();
    assert!(
        accepted.is_empty(),
        "unsupported borders silently changed: {accepted:?}"
    );
}
#[test]
fn review_untyped_ownership_edges_in_font_resources_refuse_public_reflow() {
    let e = engine();
    let s = source(&e);
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("input.pdf");
    export(&e, &s, &path);
    let mut d = lopdf::Document::load(&path).unwrap();
    let ap = normal_from(&d, 0);
    let resources = d
        .dereference(ap.dict.get(b"Resources").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    let fonts = d
        .dereference(resources.get(b"Font").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    let font = fonts.get(b"FolioField0").unwrap().as_reference().unwrap();
    let action=d.add_object(lopdf::dictionary!{"S"=>"JavaScript","JS"=>lopdf::Object::string_literal("/* unrelated marker */")});
    let owner=d.add_object(lopdf::dictionary!{"FT"=>"Tx","T"=>lopdf::Object::string_literal("OtherField"),"V"=>lopdf::Object::string_literal("Other value"),"AA"=>lopdf::dictionary!{"K"=>action}});
    d.get_dictionary_mut(font).unwrap().set("FolioOwner", owner);
    d.save(&path).unwrap();
    let saved = std::fs::read(&path).unwrap();
    let opened = e.open_document(&path).unwrap();
    let result =
        e.position_form_field_occurrence(&opened.id, &[0], 0, reflow_request(&e, &opened.id, 0));
    assert!(result.is_err(), "unrelated field imported through font");
    assert_eq!(std::fs::read(&path).unwrap(), saved);
    for id in [&s.id, &opened.id] {
        e.close_document(id).unwrap();
    }
}
