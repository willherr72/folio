use folio_engine::{DocumentInfo, ExportRequest, PagePlan, PdfEngine};
use lopdf::{content::Content, Document, Object};
use std::{
    fs,
    path::{Path, PathBuf},
};
static FONT_REGISTRY: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[path = "support/affine_forms.rs"]
mod authored;
fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn fixture(path: &Path, kind: &str, state: &str, first: &str) {
    authored::fixture(path, kind, "plain", 0, first);
    set_state(path, state);
}
fn set_state(path: &Path, state: &str) {
    let mut pdf = Document::load(path).unwrap();
    for object in pdf.objects.values_mut() {
        let Ok(stream) = object.as_stream_mut() else {
            continue;
        };
        if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {
            continue;
        }
        let data = stream.get_plain_content().unwrap();
        if !Content::decode(&data)
            .unwrap()
            .operations
            .iter()
            .any(|op| op.operator == "Tj")
        {
            continue;
        }
        stream.set_plain_content([state.as_bytes(), b"\n", &data].concat());
    }
    pdf.save(path).unwrap();
}

fn physical_fixture(
    path: &Path,
    kind: &str,
    case: &str,
    rotation: i32,
    unit: f32,
    state: &str,
    first: &str,
) {
    authored::fixture(path, kind, case, rotation, first);
    set_state(path, state);
    let mut pdf = Document::load(path).unwrap();
    for id in pdf.get_pages().into_values() {
        pdf.get_dictionary_mut(id)
            .unwrap()
            .set("UserUnit", Object::Real(unit));
    }
    if state.contains("-85 Tz") {
        for object in pdf.objects.values_mut() {
            let Ok(stream) = object.as_stream_mut() else {
                continue;
            };
            if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {
                continue;
            }
            let mut content = Content::decode(&stream.get_plain_content().unwrap()).unwrap();
            for op in &mut content.operations {
                if op.operator == "Tm" {
                    op.operands[4] = 140.into();
                }
            }
            stream.set_plain_content(content.encode().unwrap());
        }
    }
    pdf.save(path).unwrap();
}
fn page_plan(source: &DocumentInfo, index: usize) -> PagePlan {
    PagePlan {
        crop: None,
        id: format!("{}-{index}", source.id),
        source_id: source.id.clone(),
        page_index: index,
        width: source.pages[index].width,
        height: source.pages[index].height,
        rotation: 0,
        overlays: vec![],
    }
}

// The Folio PDFium patch preserves inherited TL/Ts/Tz at form boundaries. These
// inputs previously failed the source/flat appearance comparison. Keep edits
// covered without changing any preservation guard.
#[test]
fn inherited_renderer_state_supports_edits_without_source_changes() {
    let temp = tempfile::tempdir().unwrap();
    let e = engine();
    for state in ["9 Ts", "65 Tz", "9 Ts 65 Tz"] {
        let path = temp.path().join("inherited.pdf");
        authored::fixture(&path, "standard", "plain", 0, "Cafe old");
        set_wrapper_state(&path, state);
        let bytes = fs::read(&path).unwrap();
        let source = e.open_document(&path).unwrap();
        let before = e.render_page(&source.id, 0, 1160).unwrap();
        let listed = e.list_form_text_runs(&source.id, 0).unwrap();
        assert_eq!(listed.runs.len(), 2, "{state}: {listed:?}");
        let selected = &listed.runs[0];
        let changed = e
            .replace_form_text(&source.id, 0, &selected.object_path, &selected.text, "Cafe")
            .unwrap();
        let after = e.list_form_text_runs(&changed.id, 0).unwrap();
        assert_eq!(after.runs[0].text, "Cafe");
        assert_eq!(after.runs[1], listed.runs[1]);
        assert_eq!(
            e.source_bytes(&source.id).unwrap().as_ref(),
            bytes.as_slice()
        );
        assert_eq!(e.render_page(&source.id, 0, 1160).unwrap(), before);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        e.close_document(&changed.id).unwrap();
        e.close_document(&source.id).unwrap();
    }
}

fn set_wrapper_state(path: &Path, state: &str) {
    let mut pdf = Document::load(path).unwrap();
    for object in pdf.objects.values_mut() {
        let Ok(stream) = object.as_stream_mut() else {
            continue;
        };
        if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {
            continue;
        }
        let data = stream.get_plain_content().unwrap();
        if Content::decode(&data)
            .unwrap()
            .operations
            .iter()
            .any(|op| op.operator == "Do")
        {
            stream.set_plain_content([state.as_bytes(), b"\n", &data].concat());
        }
    }
    pdf.save(path).unwrap();
}

fn set_leaf_fill_color(path: &Path, color: Option<[f32; 3]>) {
    let mut pdf = Document::load(path).unwrap();
    for object in pdf.objects.values_mut() {
        let Ok(stream) = object.as_stream_mut() else {
            continue;
        };
        if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {
            continue;
        }
        let mut content = Content::decode(&stream.get_plain_content().unwrap()).unwrap();
        if !content.operations.iter().any(|op| op.operator == "Tj") {
            continue;
        }
        content.operations.retain(|op| op.operator != "rg");
        if let Some(color) = color {
            content.operations.insert(
                0,
                lopdf::content::Operation::new("rg", color.into_iter().map(Object::Real).collect()),
            );
        }
        stream.set_plain_content(content.encode().unwrap());
    }
    pdf.save(path).unwrap();
}

#[test]
fn inherited_controls_and_local_overrides_remain_editable() {
    let temp = tempfile::tempdir().unwrap();
    let e = engine();
    for (case, (outer, local)) in [
        ("2 Tc", ""),
        ("5 Tw", ""),
        ("0.1 0.5 0.8 rg", ""),
        ("9 Ts 65 Tz", "0 Ts 100 Tz"),
        ("9 Ts 65 Tz", "2 Ts 90 Tz"),
    ]
    .into_iter()
    .enumerate()
    {
        let path = temp.path().join("supported.pdf");
        fixture(&path, "standard", local, "Cafe old");
        set_wrapper_state(&path, outer);
        if outer.ends_with(" rg") {
            // The shared fixture paints its own leaf color. Remove that
            // override only here so this case really exercises inheritance.
            set_leaf_fill_color(&path, None);
        }
        let bytes = fs::read(&path).unwrap();
        let source = e.open_document(&path).unwrap();
        if outer.ends_with(" rg") {
            let reference_path = temp.path().join("explicit-color.pdf");
            fs::copy(&path, &reference_path).unwrap();
            set_leaf_fill_color(&reference_path, Some([0.1, 0.5, 0.8]));
            let reference = e.open_document(&reference_path).unwrap();
            assert!(
                e.render_page(&source.id, 0, 1160).unwrap()
                    == e.render_page(&reference.id, 0, 1160).unwrap(),
                "Inherited fill color must match its explicit local reference",
            );
            e.close_document(&reference.id).unwrap();
        }
        let before = e.list_form_text_runs(&source.id, 0).unwrap();
        assert_eq!(before.runs.len(), 2, "{outer}/{local}: {before:?}");
        let run = &before.runs[0];
        let changed = e
            .replace_form_text(&source.id, 0, &run.object_path, &run.text, "Cafe")
            .unwrap();
        let after = e.list_form_text_runs(&changed.id, 0).unwrap();
        assert_eq!(after.runs.len(), 2);
        assert_eq!(after.runs[0].text, "Cafe");
        assert_eq!(after.runs[1], before.runs[1]);
        // A source edit returns the edited page as its own source. Recompose
        // the document through the public export API to check page isolation.
        let output = temp.path().join(format!("saved-{case}.pdf"));
        e.export_pdf(
            ExportRequest {
                pages: vec![page_plan(&changed, 0), page_plan(&source, 1)],
                flatten: false,
            },
            &output,
        )
        .unwrap();
        let saved = e.open_document(&output).unwrap();
        assert_eq!(
            e.render_page(&source.id, 1, 1160).unwrap(),
            e.render_page(&saved.id, 1, 1160).unwrap()
        );
        assert_eq!(
            e.render_page(&changed.id, 0, 1160).unwrap(),
            e.render_page(&saved.id, 0, 1160).unwrap()
        );
        assert_eq!(
            e.source_bytes(&source.id).unwrap().as_ref(),
            bytes.as_slice()
        );
        assert_eq!(fs::read(&path).unwrap(), bytes);
        e.close_document(&saved.id).unwrap();
        e.close_document(&changed.id).unwrap();
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn physical_affine_state_edits_survive_save_reopen_and_restore() {
    let temp = tempfile::tempdir().unwrap();
    let e = engine();
    for (kind, case, rotation, unit, state) in [
        (
            "standard",
            "plain",
            0,
            1.7,
            "0.3 Tc 1.5 Tw 90 Tz 2 Ts 14 TL 0 Tr",
        ),
        (
            "simple",
            "form-shear",
            90,
            2.,
            "0.3 Tc 1.5 Tw 90 Tz 2 Ts 14 TL",
        ),
        ("cid", "composed", 270, 0.5, "0.3 Tc 2 Tw 90 Tz 2 Ts 14 TL"),
        ("standard", "plain", 180, 1., "0.3 Tc 1.5 Tw -85 Tz 2 Ts"),
        ("cid", "mirror", 0, 1.7, "0.3 Tc 2 Tw -85 Tz 2 Ts"),
    ] {
        let path = temp.path().join("original.pdf");
        let old = if kind == "standard" {
            "Cafe old"
        } else {
            "Café old"
        };
        physical_fixture(&path, kind, case, rotation, unit, state, old);
        let original = fs::read(&path).unwrap();
        let source = e.open_document(&path).unwrap();
        let listed = e.list_form_text_runs(&source.id, 0).unwrap();
        assert_eq!(listed.runs.len(), 2, "{kind}/{case}/{rotation}: {listed:?}");
        for (label, replacement) in [("noop", old), ("shorter", "Cafe"), ("longer", "Cafe older")] {
            let changed = e
                .replace_form_text(&source.id, 0, &listed.runs[0].object_path, old, replacement)
                .unwrap_or_else(|err| panic!("{kind}/{case}/{rotation}/{label}: {err}"));
            let reference = temp.path().join("reference.pdf");
            physical_fixture(&reference, kind, case, rotation, unit, state, replacement);
            let expected = e.open_document(&reference).unwrap();
            let output = temp.path().join("saved.pdf");
            e.export_pdf(
                ExportRequest {
                    pages: vec![page_plan(&changed, 0), page_plan(&source, 1)],
                    flatten: false,
                },
                &output,
            )
            .unwrap();
            let reopened = e.open_document(&output).unwrap();
            for index in 0..2 {
                assert!(
                    e.render_page(&reopened.id, index, 1160).unwrap()
                        == e.render_page(&expected.id, index, 1160).unwrap(),
                    "{kind}/{case}/{rotation}/{label}/{index}: raster mismatch"
                );
            }
            let after = e.list_form_text_runs(&reopened.id, 0).unwrap().runs;
            assert_eq!(after[1], listed.runs[1]);
            assert_eq!(after[0].text, replacement);
            let restored = e
                .replace_form_text(&reopened.id, 0, &after[0].object_path, replacement, old)
                .unwrap();
            assert!(
                e.render_page(&restored.id, 0, 1160).unwrap()
                    == e.render_page(&source.id, 0, 1160).unwrap()
            );
            assert_eq!(fs::read(&path).unwrap(), original);
            if let Some(root) = std::env::var_os("FOLIO_FORM_STATE_ARTIFACTS") {
                let root = PathBuf::from(root);
                fs::create_dir_all(&root).unwrap();
                let prefix = format!("{kind}-{case}-{rotation}-{label}");
                for (from, suffix) in [
                    (&path, "original"),
                    (&reference, "reference"),
                    (&output, "exported"),
                ] {
                    fs::copy(from, root.join(format!("{prefix}-{suffix}.pdf"))).unwrap();
                }
            }
            for id in [&changed.id, &expected.id, &reopened.id, &restored.id] {
                e.close_document(id).unwrap();
            }
        }
        e.close_document(&source.id).unwrap();
    }
}
fn verify_original_font_edits(kind: &str, state: &str) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("original.pdf");
    let old = if kind == "standard" {
        "Cafe old"
    } else {
        "Café old"
    };
    fixture(&path, kind, state, old);
    let original = fs::read(&path).unwrap();
    let e = engine();
    let source = e.open_document(&path).unwrap();
    let listed = e.list_form_text_runs(&source.id, 0).unwrap();
    assert_eq!(listed.runs.len(), 2, "{kind} {state}: {listed:?}");
    assert!(listed.runs.iter().all(|run| run.supported));
    for replacement in [old, "Cafe", "Cafe older"] {
        let changed = e
            .replace_form_text(&source.id, 0, &listed.runs[0].object_path, old, replacement)
            .unwrap();
        let reference = temp.path().join("reference.pdf");
        fixture(&reference, kind, state, replacement);
        let expected = e.open_document(&reference).unwrap();
        assert!(
            e.render_page(&changed.id, 0, 1160).unwrap()
                == e.render_page(&expected.id, 0, 1160).unwrap(),
            "Raster mismatch: {kind} {state} {replacement}"
        );
        let after = e.list_form_text_runs(&changed.id, 0).unwrap();
        assert_eq!(after.runs[0].text, replacement);
        assert_eq!(after.runs[1].text, old);
        assert!(
            e.render_page(&source.id, 1, 1160).unwrap()
                == e.render_page(&expected.id, 1, 1160).unwrap(),
            "Other page changed"
        );
        assert_eq!(fs::read(&path).unwrap(), original);
        e.close_document(&changed.id).unwrap();
        e.close_document(&expected.id).unwrap();
    }
    e.close_document(&source.id).unwrap();
}

#[test]
fn malformed_text_state_is_not_admitted_for_editing() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("invalid.pdf");
    let e = engine();
    for state in [
        "Tc",
        "1 2 Tc",
        "/Bad Tw",
        "1000001 Ts",
        "-1000001 TL",
        "0 Tz",
        "0.001 Tz",
        "10001 Tz",
        "1 Tr",
        "3 Tr",
        "7 Tr",
        "0.0 Tr",
    ] {
        fixture(&path, "standard", state, "Cafe old");
        let source = e.open_document(&path).unwrap();
        let listed = e.list_form_text_runs(&source.id, 0).unwrap();
        assert!(
            listed.runs.is_empty() && listed.reason.is_some(),
            "Invalid state admitted: {state}: {listed:?}"
        );
        e.close_document(&source.id).unwrap();
    }
}

fn substitute_font(e: &PdfEngine) -> String {
    let bytes = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf"),
    )
    .unwrap();
    e.fonts().register(bytes).unwrap().id
}

#[test]
fn word_spacing_substitution_capability_follows_source_encoding() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    let e = engine();
    for kind in ["standard", "simple", "cid"] {
        fixture(
            &path,
            kind,
            "2 Tw",
            if kind == "standard" {
                "Cafe old"
            } else {
                "Café old"
            },
        );
        let source = e.open_document(&path).unwrap();
        let listed = e.list_form_text_runs(&source.id, 0).unwrap();
        assert_eq!(listed.runs.len(), 2, "{listed:?}");
        for run in &listed.runs {
            assert_eq!(run.can_substitute, kind == "cid", "{kind}");
            let json = serde_json::to_value(run).unwrap();
            assert_eq!(json.get("fontSubstitutionReason").is_some(), kind != "cid");
        }
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn direct_font_installation_cannot_drop_single_byte_word_spacing() {
    let _guard = FONT_REGISTRY.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    let e = engine();
    let font = substitute_font(&e);
    for kind in ["standard", "simple"] {
        fixture(
            &path,
            kind,
            "2 Tw",
            if kind == "standard" {
                "Cafe old"
            } else {
                "Café old"
            },
        );
        let source = e.open_document(&path).unwrap();
        let before = e.render_page(&source.id, 0, 800).unwrap();
        let run = e.list_form_text_runs(&source.id, 0).unwrap().runs.remove(0);
        let result = e.replace_form_text_with_font(
            &source.id,
            0,
            &run.object_path,
            &run.text,
            "Cafe edit",
            Some(&font),
        );
        assert!(
            result.is_err(),
            "{kind}: font change silently dropped source word spacing"
        );
        let error = result.unwrap_err().to_string();
    assert!(error.contains("word spacing"), "Unexpected font replacement refusal: {error}");
        assert!(e.render_page(&source.id, 0, 800).unwrap() == before);
        e.close_document(&source.id).unwrap();
    }
    e.fonts().remove(&font).unwrap();
}

fn scoped_fixture(path: &Path, scope: &str) {
    fixture(path, "standard", "", "Cafe old");
    let mut pdf = Document::load(path).unwrap();
    for object in pdf.objects.values_mut() {
        let Ok(stream) = object.as_stream_mut() else {
            continue;
        };
        if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {
            continue;
        }
        let data = stream.get_plain_content().unwrap();
        let leaf = Content::decode(&data)
            .unwrap()
            .operations
            .iter()
            .any(|op| op.operator == "Tj");
        let (prefix, suffix) = match (scope, leaf) {
            ("leaf", true) => ("2 Tw ", ""),
            ("reset", true) => ("2 Tw 0 Tw ", ""),
            ("saved", true) => ("q 2 Tw Q ", ""),
            ("override", true) => ("0 Tw ", ""),
            ("after-text", true) => ("", " 2 Tw"),
            ("wrapper", false) => ("2 Tw ", ""),
            ("after-child", false) => ("", " 2 Tw"),
            _ => ("", ""),
        };
        stream.set_plain_content([prefix.as_bytes(), &data, suffix.as_bytes()].concat());
    }
    if matches!(scope, "page" | "override" | "first-only") {
        let page = *pdf.get_pages().values().next().unwrap();
        let id = pdf
            .get_dictionary(page)
            .unwrap()
            .get(b"Contents")
            .unwrap()
            .as_reference()
            .unwrap();
        let stream = pdf.get_object_mut(id).unwrap().as_stream_mut().unwrap();
        let data = String::from_utf8(stream.get_plain_content().unwrap()).unwrap();
        stream.set_plain_content(if scope == "first-only" {
            data.replacen("/First Do", "2 Tw /First Do", 1).into_bytes()
        } else {
            format!("2 Tw {data}").into_bytes()
        });
    }
    pdf.save(path).unwrap();
}

#[test]
fn word_spacing_capability_is_local_to_the_selected_occurrence() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("scope.pdf");
    let e = engine();
    for (scope, expected) in [
        ("leaf", [false, false]),
        ("wrapper", [false, false]),
        ("page", [false, false]),
        ("reset", [true, true]),
        ("saved", [true, true]),
        ("override", [true, true]),
        ("after-text", [true, true]),
        ("after-child", [true, true]),
        ("first-only", [false, true]),
    ] {
        scoped_fixture(&path, scope);
        let source = e.open_document(&path).unwrap();
        let listed = e.list_form_text_runs(&source.id, 0).unwrap();
        assert_eq!(listed.runs.len(), 2, "{scope}: {listed:?}");
        assert_eq!(
            listed
                .runs
                .iter()
                .map(|r| r.can_substitute)
                .collect::<Vec<_>>(),
            expected,
            "{scope}"
        );
        let run = &listed.runs[0];
        let changed = e
            .replace_form_text(&source.id, 0, &run.object_path, &run.text, "Cafe edit")
            .unwrap();
        let after = e.list_form_text_runs(&changed.id, 0).unwrap();
        assert_eq!(after.runs[1], listed.runs[1], "{scope}");
        assert_eq!(after.runs[0].can_substitute, expected[0]);
        e.close_document(&changed.id).unwrap();
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn substitution_retains_spacing_scale_and_rise_when_encoding_allows_it() {
    let _guard = FONT_REGISTRY.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    let e = engine();
    let font = substitute_font(&e);
    for (kind, state) in [
        ("standard", "0.3 Tc 85 Tz 3 Ts 14 TL"),
        ("simple", "0.3 Tc 85 Tz 3 Ts 14 TL"),
        ("cid", "0.3 Tc 2 Tw 85 Tz 3 Ts 14 TL"),
    ] {
        fixture(
            &path,
            kind,
            state,
            if kind == "standard" {
                "Cafe old"
            } else {
                "Café old"
            },
        );
        let source = e.open_document(&path).unwrap();
        let before = e.list_form_text_runs(&source.id, 0).unwrap().runs;
        assert!(before[0].can_substitute);
        let changed = e
            .replace_form_text_with_font(
                &source.id,
                0,
                &before[0].object_path,
                &before[0].text,
                "Cafè edit",
                Some(&font),
            )
            .unwrap();
        let after = e.list_form_text_runs(&changed.id, 0).unwrap().runs;
        assert_eq!(after[0].text, "Cafè edit");
        assert_eq!(after[1], before[1]);
        let same = e
            .replace_form_text_with_font(
                &changed.id,
                0,
                &after[0].object_path,
                &after[0].text,
                &after[0].text,
                Some(&font),
            )
            .unwrap();
        assert!(
            e.render_page(&same.id, 0, 1160).unwrap()
                == e.render_page(&changed.id, 0, 1160).unwrap()
        );
        if let Some(root) = std::env::var_os("FOLIO_FORM_STATE_ARTIFACTS") {
            let root = PathBuf::from(root);
            fs::create_dir_all(&root).unwrap();
            fs::copy(&path, root.join(format!("{kind}-substitute-original.pdf"))).unwrap();
            fs::write(
                root.join(format!("{kind}-substitute-edited.pdf")),
                e.source_bytes(&changed.id).unwrap(),
            )
            .unwrap();
        }
        for id in [&source.id, &changed.id, &same.id] {
            e.close_document(id).unwrap();
        }
    }
    e.fonts().remove(&font).unwrap();
}

#[test]
fn exact_simple_font_reuse_keeps_its_word_spacing_encoding() {
    let _guard = FONT_REGISTRY.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    fixture(&path, "simple", "2 Tw", "Café old");
    let pdf = Document::load(&path).unwrap();
    let font_id = pdf
        .objects
        .values()
        .filter_map(|o| o.as_dict().ok())
        .find_map(|d| d.get(b"FontFile2").ok())
        .unwrap()
        .as_reference()
        .unwrap();
    let bytes = pdf
        .get_object(font_id)
        .unwrap()
        .as_stream()
        .unwrap()
        .get_plain_content()
        .unwrap();
    let e = engine();
    let chosen = e.fonts().register(bytes).unwrap();
    let source = e.open_document(&path).unwrap();
    let run = e.list_form_text_runs(&source.id, 0).unwrap().runs.remove(0);
    assert!(!run.can_substitute);
    let changed = e
        .replace_form_text_with_font(
            &source.id,
            0,
            &run.object_path,
            &run.text,
            "Cafe edit",
            Some(&chosen.id),
        )
        .unwrap();
    let reference = temp.path().join("reference.pdf");
    fixture(&reference, "simple", "2 Tw", "Cafe edit");
    let expected = e.open_document(&reference).unwrap();
    assert!(
        e.render_page(&changed.id, 0, 1160).unwrap()
            == e.render_page(&expected.id, 0, 1160).unwrap()
    );
    assert!(!e.list_form_text_runs(&changed.id, 0).unwrap().runs[0].can_substitute);
    for id in [&source.id, &changed.id, &expected.id] {
        e.close_document(id).unwrap();
    }
    e.fonts().remove(&chosen.id).unwrap();
}

#[test]
fn spacing_edits_keep_collision_and_every_ancestor_clip_checks() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    let e = engine();
    for case in ["leaf-edge", "ancestor-edge", "collision"] {
        fixture(&path, "standard", "0.3 Tc 90 Tz 2 Ts", "Cafe old");
        authored::mutate(&path, case);
        if case == "collision" {
            // Use a baseline accepted by the exact original/flat raster proof.
            let mut pdf = Document::load(&path).unwrap();
            let page = *pdf.get_pages().values().next().unwrap();
            let content = pdf
                .get_dictionary(page)
                .unwrap()
                .get(b"Contents")
                .unwrap()
                .as_reference()
                .unwrap();
            pdf.get_object_mut(content).unwrap().as_stream_mut().unwrap().set_plain_content(
                b"BT /N 12 Tf 1 0 0 1 30 60 Tm (Neighbor) Tj ET q 1 0 0 1 140 340 cm /First Do Q q 1 0 0 1 230 340 cm /Shared Do Q".to_vec());
            pdf.save(&path).unwrap();
        }
        let original = fs::read(&path).unwrap();
        let source = e.open_document(&path).unwrap();
        let before = e.render_page(&source.id, 0, 1160).unwrap();
        let listed = e.list_form_text_runs(&source.id, 0).unwrap();
        assert_eq!(listed.runs.len(), 2, "{case}: {listed:?}");
        let run = &listed.runs[0];
        let error = e
            .replace_form_text(
                &source.id,
                0,
                &run.object_path,
                &run.text,
                "Cafe older and longer",
            )
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(if case == "collision" {
                "overlap"
            } else {
                "clip"
            }),
            "{case}: {error}"
        );
        assert!(e.render_page(&source.id, 0, 1160).unwrap() == before);
        assert_eq!(fs::read(&path).unwrap(), original);
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn recovery_restores_spacing_edit_without_the_original_file() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    physical_fixture(
        &path,
        "standard",
        "plain",
        90,
        1.7,
        "0.3 Tc 1.5 Tw 90 Tz 2 Ts",
        "Cafe old",
    );
    let e = engine();
    let source = e.open_document(&path).unwrap();
    let run = e.list_form_text_runs(&source.id, 0).unwrap().runs.remove(0);
    let changed = e
        .replace_form_text(&source.id, 0, &run.object_path, &run.text, "Cafe edit")
        .unwrap();
    let expected = e.render_page(&changed.id, 0, 1160).unwrap();
    let store = folio_engine::RecoveryStore::new(temp.path().join("recovery")).unwrap();
    store.save(&e,serde_json::json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","savedDigest":"old","dirty":true,"zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"Spacing.pdf","selectedPageId":format!("{}-0",changed.id),"selectedOverlayId":null,"pages":[page_plan(&changed,0)]}}]})).unwrap();
    e.close_document(&source.id).unwrap();
    e.close_document(&changed.id).unwrap();
    fs::rename(&path, temp.path().join("unavailable.pdf")).unwrap();
    let restored = store.load(&e).unwrap().unwrap();
    let id = restored["tabs"][0]["document"]["pages"][0]["sourceId"]
        .as_str()
        .unwrap();
    assert!(e.render_page(id, 0, 1160).unwrap() == expected);
    let listed = e.list_form_text_runs(id, 0).unwrap();
    assert_eq!(listed.runs[0].text, "Cafe edit");
    assert!(!listed.runs[0].can_substitute);
    let again = e
        .replace_form_text(id, 0, &listed.runs[0].object_path, "Cafe edit", "Cafe old")
        .unwrap();
    e.close_document(&again.id).unwrap();
    e.close_document(id).unwrap();
}
#[test]
fn default_text_state_operators_allow_existing_form_edits() {
    verify_original_font_edits("standard", "0 Tc 0 Tw 100 Tz 0 Ts 0 TL 0 Tr");
}
#[test]
fn nondefault_spacing_scale_rise_and_leading_preserve_original_font_edits() {
    for kind in ["standard", "simple", "cid"] {
        for state in [
            "0.4 Tc",
            "2 Tw",
            "85 Tz",
            "3 Ts",
            "14 TL",
            "0.3 Tc 1.5 Tw 90 Tz 2 Ts 14 TL 0 Tr",
        ] {
            verify_original_font_edits(kind, state);
        }
    }
}
