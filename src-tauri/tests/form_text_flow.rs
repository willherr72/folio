use folio_engine::{DocumentInfo, ExportRequest, PagePlan, PdfEngine};
use lopdf::{
    content::{Content, Operation},
    Document, Object, StringFormat,
};
use std::{
    fs,
    path::{Path, PathBuf},
};
#[path = "support/affine_forms.rs"]
mod authored;
#[path = "support/protected_fixture.rs"]
mod protected;
static FONT_REGISTRY: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn plan(d: &DocumentInfo, index: usize) -> PagePlan {
    PagePlan {
        crop: None,
        id: format!("{}-{index}", d.id),
        source_id: d.id.clone(),
        page_index: index,
        width: d.pages[index].width,
        height: d.pages[index].height,
        rotation: 0,
        overlays: vec![],
    }
}
fn encoded(text: &str, kind: &str) -> Vec<u8> {
    if kind == "cid" {
        text.encode_utf16().flat_map(u16::to_be_bytes).collect()
    } else {
        text.chars()
            .map(|c| {
                if c == 'é' {
                    1
                } else {
                    u8::try_from(c as u32).unwrap()
                }
            })
            .collect()
    }
}
fn string(text: &str, kind: &str) -> Object {
    Object::String(encoded(text, kind), StringFormat::Hexadecimal)
}
fn parsed(text: &str) -> Vec<Operation> {
    Content::decode(text.as_bytes()).unwrap().operations
}
fn fixture(path: &Path, kind: &str, mode: &str, change: Option<(usize, &str)>) {
    physical_fixture(path, kind, mode, change, "plain", 0, 1.);
}
fn physical_fixture(
    path: &Path,
    kind: &str,
    mode: &str,
    change: Option<(usize, &str)>,
    case: &str,
    rotation: i32,
    unit: f32,
) {
    let old = if kind == "standard" {
        "Cafe old"
    } else {
        "Café old"
    };
    authored::fixture(
        path,
        kind,
        case,
        rotation,
        if change.is_some() { "Cafe edit" } else { old },
    );
    let mut pdf = Document::load(path).unwrap();
    for id in pdf.get_pages().into_values() {
        pdf.get_dictionary_mut(id).unwrap().set("UserUnit", unit);
    }
    for object in pdf.objects.values_mut() {
        let Ok(stream) = object.as_stream_mut() else {
            continue;
        };
        if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {
            continue;
        }
        let existing = Content::decode(&stream.get_plain_content().unwrap()).unwrap();
        let Some(show) = existing.operations.iter().find(|op| op.operator == "Tj") else {
            continue;
        };
        let selected = show.operands[0].as_str().unwrap() == encoded("Cafe edit", kind);
        let lines: [&str; 2] = std::array::from_fn(|i| {
            if selected {
                change.filter(|c| c.0 == i).map_or(old, |c| c.1)
            } else {
                old
            }
        });
        let mut ops = parsed("BT /F1 16 Tf 40 TL ET BT 1 0 0 1 5 80 Tm");
        ops.push(Operation::new("Tj", vec![string(lines[0], kind)]));
        match mode {
            "setup" => ops.extend(parsed("T* ET BT 1 0 0 1 5 40 Tm")),
            "T*" => ops.extend(parsed("T*")),
            "Td" => ops.extend(parsed("0 -40 Td")),
            "TD" => ops.extend(parsed("0 -40 TD")),
            "Tm" => ops.extend(parsed("1 0 0 1 5 40 Tm")),
            "quote" | "double" => {}
            _ => panic!("unknown mode"),
        }
        ops.push(match mode {
            "quote" => Operation::new("'", vec![string(lines[1], kind)]),
            "double" => Operation::new("\"", vec![0.into(), 0.3.into(), string(lines[1], kind)]),
            _ => Operation::new("Tj", vec![string(lines[1], kind)]),
        });
        ops.extend(parsed("T* ET"));
        stream.set_plain_content(Content { operations: ops }.encode().unwrap());
    }
    pdf.save(path).unwrap();
}

#[test]
fn empty_setup_and_independent_lines_preserve_other_occurrences() {
    let temp = tempfile::tempdir().unwrap();
    let e = engine();
    for kind in ["standard", "simple", "cid"] {
        let old = if kind == "standard" {
            "Cafe old"
        } else {
            "Café old"
        };
        for mode in ["setup", "T*", "Td", "TD", "Tm", "quote", "double"] {
            let path = temp.path().join("original.pdf");
            fixture(&path, kind, mode, None);
            let bytes = fs::read(&path).unwrap();
            let source = e.open_document(&path).unwrap();
            let listed = e.list_form_text_runs(&source.id, 0).unwrap();
            assert_eq!(listed.runs.len(), 4, "{kind}/{mode}: {listed:?}");
            assert_eq!(
                listed
                    .runs
                    .iter()
                    .map(|r| r.object_path.clone())
                    .collect::<Vec<_>>(),
                vec![vec![1, 0, 0], vec![1, 0, 1], vec![2, 0, 0], vec![2, 0, 1]]
            );
            for line in [0, 1] {
                for value in [old, "Cafe", "Cafe older"] {
                    let changed = e
                        .replace_form_text(
                            &source.id,
                            0,
                            &listed.runs[line].object_path,
                            old,
                            value,
                        )
                        .unwrap_or_else(|err| panic!("{kind}/{mode}/{line}/{value}: {err}"));
                    let reference = temp.path().join("reference.pdf");
                    fixture(&reference, kind, mode, Some((line, value)));
                    let reference = e.open_document(&reference).unwrap();
                    assert!(
                        e.render_page(&changed.id, 0, 1160).unwrap()
                            == e.render_page(&reference.id, 0, 1160).unwrap(),
                        "{kind}/{mode}/{line}/{value}: pixels"
                    );
                    let after = e.list_form_text_runs(&changed.id, 0).unwrap();
                    assert_eq!(after.runs.len(), 4, "{after:?}");
                    for (i, run) in after.runs.iter().enumerate() {
                        assert_eq!(run.text, if i == line { value } else { old });
                    }
                    e.close_document(&reference.id).unwrap();
                    e.close_document(&changed.id).unwrap();
                }
            }
            assert_eq!(fs::read(&path).unwrap(), bytes);
            assert_eq!(
                e.source_bytes(&source.id).unwrap().as_ref(),
                bytes.as_slice()
            );
            e.close_document(&source.id).unwrap();
        }
    }
}

#[test]
fn substitute_font_changes_only_one_inherited_line() {
    let _lock = FONT_REGISTRY.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let e = engine();
    let font = e
        .fonts()
        .register(
            fs::read(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf"),
            )
            .unwrap(),
        )
        .unwrap();
    for kind in ["standard", "simple", "cid"] {
        let old = if kind == "standard" {
            "Cafe old"
        } else {
            "Café old"
        };
        for mode in ["setup", "TD", "quote", "double"] {
            let path = temp.path().join("original.pdf");
            fixture(&path, kind, mode, None);
            let source = e.open_document(&path).unwrap();
            let before = e.list_form_text_runs(&source.id, 0).unwrap();
            assert_eq!(before.runs.len(), 4, "{before:?}");
            for line in [0, 1] {
                let changed = e
                    .replace_form_text_with_font(
                        &source.id,
                        0,
                        &before.runs[line].object_path,
                        old,
                        "Cafè edit",
                        Some(&font.id),
                    )
                    .unwrap_or_else(|err| panic!("{kind}/{mode}/{line}: {err}"));
                let after = e.list_form_text_runs(&changed.id, 0).unwrap();
                assert_eq!(after.runs.len(), 4, "{after:?}");
                for (i, run) in after.runs.iter().enumerate() {
                    if i == line {
                        assert_eq!(run.text, "Cafè edit");
                        assert_eq!(run.font_display_name.as_deref(), Some("DejaVu Serif"));
                    } else {
                        assert_eq!(run, &before.runs[i], "{kind}/{mode}/{line}/{i}");
                    }
                }
                let repeated = e
                    .replace_form_text_with_font(
                        &changed.id,
                        0,
                        &after.runs[line].object_path,
                        "Cafè edit",
                        "Cafè edit",
                        Some(&font.id),
                    )
                    .unwrap();
                assert!(
                    e.render_page(&changed.id, 0, 1160).unwrap()
                        == e.render_page(&repeated.id, 0, 1160).unwrap()
                );
                let other = 1 - line;
                let again = e
                    .replace_form_text(&changed.id, 0, &after.runs[other].object_path, old, "Cafe")
                    .unwrap();
                assert_eq!(
                    e.list_form_text_runs(&again.id, 0).unwrap().runs[other].text,
                    "Cafe"
                );
                if let Some(root) = std::env::var_os("FOLIO_FORM_FLOW_ARTIFACTS") {
                    let root = PathBuf::from(root);
                    fs::create_dir_all(&root).unwrap();
                    let prefix = format!("{kind}-{mode}-line-{line}-substitute");
                    fs::copy(&path, root.join(format!("{prefix}-original.pdf"))).unwrap();
                    fs::write(
                        root.join(format!("{prefix}-edited.pdf")),
                        e.source_bytes(&changed.id).unwrap(),
                    )
                    .unwrap();
                }
                for id in [&changed.id, &repeated.id, &again.id] {
                    e.close_document(id).unwrap();
                }
            }
            e.close_document(&source.id).unwrap();
        }
    }
    e.fonts().remove(&font.id).unwrap();
}

#[test]
fn real_reportlab_forms_edit_and_reopen_against_authored_references() {
    let temp = tempfile::tempdir().unwrap();
    let e = engine();
    let fixtures =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/form-text-flow");
    for mode in ["draw-string", "text-lines", "explicit-lines"] {
        let path = fixtures.join(format!("{mode}-original.pdf"));
        let source = e.open_document(&path).unwrap();
        let listed = e.list_form_text_runs(&source.id, 0).unwrap();
        assert_eq!(listed.runs.len(), 4, "{mode}: {listed:?}");
        for line in [0, 1] {
            let replacement = ["First longer sentence", "Second longer sentence"][line];
            let run = &listed.runs[line];
            let changed = e
                .replace_form_text(&source.id, 0, &run.object_path, &run.text, replacement)
                .unwrap();
            let output = temp.path().join("saved.pdf");
            e.export_pdf(
                ExportRequest {
                    pages: vec![plan(&changed, 0), plan(&source, 1)],
                    flatten: false,
                },
                &output,
            )
            .unwrap();
            let reopened = e.open_document(&output).unwrap();
            let reference = fixtures.join(format!("{mode}-line-{line}-reference.pdf"));
            let expected = e.open_document(&reference).unwrap();
            for page in 0..2 {
                assert!(
                    e.render_page(&reopened.id, page, 1200).unwrap()
                        == e.render_page(&expected.id, page, 1200).unwrap(),
                    "{mode}/{line}/{page}"
                );
            }
            let after = e.list_form_text_runs(&reopened.id, 0).unwrap();
            assert_eq!(after.runs.len(), 4);
            assert_eq!(after.runs[line].text, replacement);
            let restored = e
                .replace_form_text(
                    &reopened.id,
                    0,
                    &after.runs[line].object_path,
                    replacement,
                    &run.text,
                )
                .unwrap();
            assert!(
                e.render_page(&restored.id, 0, 1200).unwrap()
                    == e.render_page(&source.id, 0, 1200).unwrap()
            );
            if let Some(root) = std::env::var_os("FOLIO_FORM_FLOW_ARTIFACTS") {
                let root = PathBuf::from(root);
                fs::create_dir_all(&root).unwrap();
                let prefix = format!("reportlab-{mode}-line-{line}");
                for (from, suffix) in [
                    (&path, "original"),
                    (&reference, "reference"),
                    (&output, "exported"),
                ] {
                    fs::copy(from, root.join(format!("{prefix}-{suffix}.pdf"))).unwrap();
                }
            }
            for id in [&changed.id, &reopened.id, &expected.id, &restored.id] {
                e.close_document(id).unwrap();
            }
        }
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn ancestor_font_selection_survives_shadowed_names_and_substitution() {
    let _lock = FONT_REGISTRY.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    fixture(&path, "standard", "T*", None);
    let mut pdf = Document::load(&path).unwrap();
    let shadow = pdf
        .add_object(lopdf::dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
    for object in pdf.objects.values_mut() {
        let Ok(stream) = object.as_stream_mut() else {
            continue;
        };
        if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {
            continue;
        }
        let mut content = Content::decode(&stream.get_plain_content().unwrap()).unwrap();
        if content.operations.iter().any(|op| op.operator == "Tj") {
            content.operations.retain(|op| op.operator != "Tf");
            stream.dict.set("Resources",lopdf::dictionary!{"Font"=>lopdf::dictionary!{"F1"=>shadow,"FolioFormFont"=>shadow,"FolioFormRestore"=>shadow}});
        } else {
            // The inner selection is restored before entering a leaf that shadows F1.
            content
                .operations
                .splice(0..0, parsed("/F1 16 Tf q /F1 9 Tf Q"));
        }
        stream.set_plain_content(content.encode().unwrap());
    }
    pdf.save(&path).unwrap();
    let original = fs::read(&path).unwrap();
    let e = engine();
    let source = e.open_document(&path).unwrap();
    let before = e.list_form_text_runs(&source.id, 0).unwrap();
    assert_eq!(before.runs.len(), 4, "{before:?}");
    assert!(before.runs.iter().all(|r| r.font_size == 16.));
    let font = e
        .fonts()
        .register(
            fs::read(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf"),
            )
            .unwrap(),
        )
        .unwrap();
    for line in [0, 1] {
        let changed = e
            .replace_form_text_with_font(
                &source.id,
                0,
                &before.runs[line].object_path,
                "Cafe old",
                "Cafè edit",
                Some(&font.id),
            )
            .unwrap();
        let after = e.list_form_text_runs(&changed.id, 0).unwrap();
        assert_eq!(after.runs.len(), 4, "{after:?}");
        for i in 0..4 {
            if i != line {
                assert_eq!(after.runs[i], before.runs[i]);
            }
        }
        let again = e
            .replace_form_text(
                &changed.id,
                0,
                &after.runs[1 - line].object_path,
                "Cafe old",
                "Cafe",
            )
            .unwrap();
        e.close_document(&again.id).unwrap();
        e.close_document(&changed.id).unwrap();
    }
    assert_eq!(fs::read(path).unwrap(), original);
    e.close_document(&source.id).unwrap();
    e.fonts().remove(&font.id).unwrap();
}

#[test]
fn physical_line_flow_preserves_saved_pages_and_restores_original_text() {
    let temp = tempfile::tempdir().unwrap();
    let e = engine();
    for (kind, mode, case, rotation, unit) in [
        ("standard", "TD", "plain", 0, 1.7),
        ("standard", "quote", "mirror", 180, 1.),
        ("simple", "T*", "form-shear", 90, 2.),
        ("cid", "double", "composed", 270, 0.5),
    ] {
        let old = if kind == "standard" {
            "Cafe old"
        } else {
            "Café old"
        };
        let path = temp.path().join("source.pdf");
        physical_fixture(&path, kind, mode, None, case, rotation, unit);
        let original = fs::read(&path).unwrap();
        let source = e.open_document(&path).unwrap();
        let before = e.list_form_text_runs(&source.id, 0).unwrap();
        assert_eq!(before.runs.len(), 4, "{kind}/{mode}/{case}: {before:?}");
        for (line, value) in [(0, "Cafe older"), (1, "Cafe")] {
            let changed = e
                .replace_form_text(&source.id, 0, &before.runs[line].object_path, old, value)
                .unwrap();
            let output = temp.path().join("saved.pdf");
            e.export_pdf(
                ExportRequest {
                    pages: vec![plan(&changed, 0), plan(&source, 1)],
                    flatten: false,
                },
                &output,
            )
            .unwrap();
            let reopened = e.open_document(&output).unwrap();
            let reference = temp.path().join("reference.pdf");
            physical_fixture(
                &reference,
                kind,
                mode,
                Some((line, value)),
                case,
                rotation,
                unit,
            );
            let expected = e.open_document(&reference).unwrap();
            for page in 0..2 {
                assert!(
                    e.render_page(&reopened.id, page, 1160).unwrap()
                        == e.render_page(&expected.id, page, 1160).unwrap(),
                    "{kind}/{case}/{line}/{page}"
                );
            }
            let after = e.list_form_text_runs(&reopened.id, 0).unwrap();
            assert_eq!(after.runs.len(), 4);
            let restored = e
                .replace_form_text(&reopened.id, 0, &after.runs[line].object_path, value, old)
                .unwrap();
            assert!(
                e.render_page(&restored.id, 0, 1160).unwrap()
                    == e.render_page(&source.id, 0, 1160).unwrap()
            );
            if let Some(root) = std::env::var_os("FOLIO_FORM_FLOW_ARTIFACTS") {
                let root = PathBuf::from(root);
                fs::create_dir_all(&root).unwrap();
                let prefix = format!("{kind}-{case}-{rotation}-line-{line}");
                for (from, suffix) in [
                    (&path, "original"),
                    (&reference, "reference"),
                    (&output, "exported"),
                ] {
                    fs::copy(from, root.join(format!("{prefix}-{suffix}.pdf"))).unwrap();
                }
            }
            for id in [&changed.id, &reopened.id, &expected.id, &restored.id] {
                e.close_document(id).unwrap();
            }
        }
        assert_eq!(fs::read(path).unwrap(), original);
        e.close_document(&source.id).unwrap();
    }
}

fn mutate_leaf(path: &Path, mut change: impl FnMut(&mut Content, &mut lopdf::Dictionary)) {
    let mut pdf = Document::load(path).unwrap();
    for o in pdf.objects.values_mut() {
        let Ok(s) = o.as_stream_mut() else { continue };
        if s.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {
            continue;
        }
        let mut content = Content::decode(&s.get_plain_content().unwrap()).unwrap();
        if !content.operations.iter().any(|op| op.operator == "Tj") {
            continue;
        }
        change(&mut content, &mut s.dict);
        s.set_plain_content(content.encode().unwrap());
    }
    pdf.save(path).unwrap();
}

#[test]
fn malformed_flow_is_refused_without_changing_source() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    let e = engine();
    for bad in [
        "1 T* (Next) Tj",
        "0 TD (Next) Tj",
        "() '",
        "0 (Next) \"",
        "0 0 0 \"",
        "1 ET",
        "BT (Next) Tj ET",
        "1 0 0 1 5 40 Tm /Missing 16 Tf (Next) Tj",
    ] {
        fixture(&path, "standard", "T*", None);
        mutate_leaf(&path, |content, _| {
            content.operations = parsed(&format!(
                "/F1 16 Tf BT 1 0 0 1 5 80 Tm (Cafe old) Tj {bad} ET"
            ));
        });
        let bytes = fs::read(&path).unwrap();
        let source = e.open_document(&path).unwrap();
        let listed = e.list_form_text_runs(&source.id, 0).unwrap();
        assert!(listed.runs.is_empty(), "{bad}: {listed:?}");
        assert!(e
            .replace_form_text(&source.id, 0, &[1, 0, 0], "Cafe old", "Cafe")
            .is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn double_quote_spacing_controls_substitution_and_survives_later_lines() {
    let _lock = FONT_REGISTRY.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    let e = engine();
    let font = e
        .fonts()
        .register(
            fs::read(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf"),
            )
            .unwrap(),
        )
        .unwrap();
    for kind in ["standard", "simple", "cid"] {
        fixture(&path, kind, "double", None);
        mutate_leaf(&path, |c, _| {
            let index = c
                .operations
                .iter()
                .position(|op| op.operator == "\"")
                .unwrap();
            c.operations[index].operands[0] = 2.into();
            let value = string("Cafe old", kind);
            c.operations.splice(
                index + 1..index + 1,
                [
                    Operation::new("T*", vec![]),
                    Operation::new("Tj", vec![value]),
                ],
            );
        });
        let source = e.open_document(&path).unwrap();
        let before = e.list_form_text_runs(&source.id, 0).unwrap();
        assert_eq!(before.runs.len(), 6, "{kind}: {before:?}");
        assert!(before.runs[0].can_substitute);
        assert_eq!(before.runs[1].can_substitute, kind == "cid");
        assert_eq!(before.runs[2].can_substitute, kind == "cid");
        let quoted = &before.runs[1];
        let result = e.replace_form_text_with_font(
            &source.id,
            0,
            &quoted.object_path,
            &quoted.text,
            "Cafè edit",
            Some(&font.id),
        );
        if kind == "cid" {
            let changed = result.unwrap();
            let after = e.list_form_text_runs(&changed.id, 0).unwrap();
            assert_eq!(after.runs[2], before.runs[2]);
            e.close_document(&changed.id).unwrap();
        } else {
            assert!(result.unwrap_err().to_string().contains("word spacing"));
        }
        let changed = e
            .replace_form_text(&source.id, 0, &quoted.object_path, &quoted.text, "Cafe")
            .unwrap();
        let after = e.list_form_text_runs(&changed.id, 0).unwrap();
        assert_eq!(after.runs[2], before.runs[2]);
        e.close_document(&changed.id).unwrap();
        e.close_document(&source.id).unwrap();
    }
    e.fonts().remove(&font.id).unwrap();
}

#[test]
fn line_growth_respects_ancestor_boxes_and_neighboring_shared_copy() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    let e = engine();
    for case in ["leaf", "ancestor", "collision"] {
        fixture(&path, "standard", "TD", None);
        let mut pdf = Document::load(&path).unwrap();
        if case == "collision" {
            let page = *pdf.get_pages().values().next().unwrap();
            let stream = pdf
                .get_dictionary(page)
                .unwrap()
                .get(b"Contents")
                .unwrap()
                .as_reference()
                .unwrap();
            pdf.get_object_mut(stream).unwrap().as_stream_mut().unwrap().set_plain_content(b"BT /N 12 Tf 1 0 0 1 30 60 Tm (Neighbor) Tj ET q 1 0 0 1 140 340 cm /First Do Q q 1 0 0 1 240 340 cm /Shared Do Q".to_vec());
        } else {
            for object in pdf.objects.values_mut() {
                let Ok(stream) = object.as_stream_mut() else {
                    continue;
                };
                if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {
                    continue;
                }
                let leaf = Content::decode(&stream.get_plain_content().unwrap())
                    .unwrap()
                    .operations
                    .iter()
                    .any(|op| op.operator == "Tj");
                if leaf == (case == "leaf") {
                    stream
                        .dict
                        .set("BBox", vec![0.into(), 0.into(), 90.into(), 120.into()]);
                }
            }
        }
        pdf.save(&path).unwrap();
        let source = e.open_document(&path).unwrap();
        let before = e.list_form_text_runs(&source.id, 0).unwrap();
        assert_eq!(before.runs.len(), 4, "{case}: {before:?}");
        let err = e
            .replace_form_text(
                &source.id,
                0,
                &before.runs[0].object_path,
                "Cafe old",
                "Cafe older and longer",
            )
            .unwrap_err()
            .to_string();
        assert!(
            err.contains(if case == "collision" {
                "overlap"
            } else {
                "clip"
            }),
            "{case}: {err}"
        );
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn line_flow_recovery_retains_state_without_original_source() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    fixture(&path, "cid", "double", None);
    let e = engine();
    let source = e.open_document(&path).unwrap();
    let run = e.list_form_text_runs(&source.id, 0).unwrap().runs.remove(0);
    let changed = e
        .replace_form_text(&source.id, 0, &run.object_path, &run.text, "Cafe older")
        .unwrap();
    let raster = e.render_page(&changed.id, 0, 1160).unwrap();
    let retained_bytes = e.source_bytes(&changed.id).unwrap();
    let store = folio_engine::RecoveryStore::new(temp.path().join("recovery")).unwrap();
    store.save(&e,serde_json::json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","savedDigest":"old","dirty":true,"zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"Line flow.pdf","selectedPageId":format!("{}-0",changed.id),"selectedOverlayId":null,"pages":[plan(&changed,0)]}}]})).unwrap();
    e.close_document(&source.id).unwrap();
    e.close_document(&changed.id).unwrap();
    fs::rename(&path, temp.path().join("unavailable.pdf")).unwrap();
    let restored = store.load(&e).unwrap().unwrap();
    let id = restored["tabs"][0]["document"]["pages"][0]["sourceId"]
        .as_str()
        .unwrap();
    assert!(e.render_page(id, 0, 1160).unwrap() == raster);
    assert_eq!(e.source_bytes(id).unwrap(), retained_bytes);
    let run = e.list_form_text_runs(id, 0).unwrap().runs.remove(0);
    let again = e
        .replace_form_text(id, 0, &run.object_path, "Cafe older", "Cafe old")
        .unwrap();
    e.close_document(&again.id).unwrap();
    e.close_document(id).unwrap();
}

#[test]
fn line_flow_edits_preserve_source_permissions_and_export_provenance() {
    use folio_engine::{protection::unlock_pdf, EngineError};
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    let e = engine();
    for allowed in [true, false] {
        fixture(&path, "standard", "TD", None);
        let bytes = protected::encrypt(
            &fs::read(&path).unwrap(),
            if allowed {
                lopdf::encryption::Permissions::MODIFIABLE
            } else {
                lopdf::encryption::Permissions::COPYABLE
            },
        );
        fs::write(&path, &bytes).unwrap();
        let source = e.open_document_with_password(&path, "reader").unwrap();
        if allowed {
            let run = e.list_form_text_runs(&source.id, 0).unwrap().runs.remove(0);
            let changed = e
                .replace_form_text(&source.id, 0, &run.object_path, &run.text, "Cafe older")
                .unwrap();
            assert_eq!(changed.protection, source.protection);
            assert!(matches!(
                e.extract_text(&changed.id, 0),
                Err(EngineError::PermissionDenied(_))
            ));
            let output = temp.path().join("saved.pdf");
            e.export_pdf(
                ExportRequest {
                    pages: vec![plan(&changed, 0), plan(&source, 1)],
                    flatten: false,
                },
                &output,
            )
            .unwrap();
            let saved = fs::read(&output).unwrap();
            for password in ["reader", "owner"] {
                let opened = unlock_pdf(&saved, Some(password)).unwrap();
                assert_eq!(
                    Document::load_mem(&opened.bytes).unwrap().get_pages().len(),
                    2
                );
                let copy = e.open_document_with_password(&output, password).unwrap();
                assert_eq!(
                    e.list_form_text_runs(&copy.id, 0).unwrap().runs[0].text,
                    "Cafe older"
                );
                assert_eq!(
                    e.list_form_text_runs(&copy.id, 1).unwrap().runs[0].text,
                    "Cafe old"
                );
                if password == "reader" {
                    let saved_policy = copy.protection.as_ref().unwrap();
                    let original_policy = source.protection.as_ref().unwrap();
                    assert_eq!(saved_policy.permissions, original_policy.permissions);
                    assert_eq!(saved_policy.owner, original_policy.owner);
                    assert_eq!(saved_policy.revision, original_policy.revision);
                }
                e.close_document(&copy.id).unwrap();
            }
            e.close_document(&changed.id).unwrap();
        } else {
            assert!(matches!(
                e.list_form_text_runs(&source.id, 0),
                Err(EngineError::PermissionDenied(_))
            ));
            assert!(matches!(
                e.replace_form_text(&source.id, 0, &[1, 0, 0], "Cafe old", "Cafe older"),
                Err(EngineError::PermissionDenied(_))
            ));
        }
        assert_eq!(fs::read(&path).unwrap(), bytes);
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn combined_font_and_leading_scopes_match_explicit_line_references() {
    fn make(path: &Path, reference: bool, inherited_leading: bool) {
        fixture(path, "standard", "TD", None);
        let mut pdf = Document::load(path).unwrap();
        let other = pdf.add_object(lopdf::dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
        for object in pdf.objects.values_mut() {
            let Ok(stream) = object.as_stream_mut() else { continue };
            if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") { continue }
            let old = Content::decode(&stream.get_plain_content().unwrap()).unwrap();
            if old.operations.iter().any(|op| op.operator == "Tj") {
                stream.dict.set("Resources", lopdf::dictionary! {"Font"=>lopdf::dictionary!{"F2"=>other}});
                // F1 was selected in the parent; the leaf has no F1 resource.
                let content = if reference {
                    "BT 1 0 0 1 5 67 Tm (Cafe old) Tj 1 0 0 1 5 37 Tm (Cafe old) Tj 1 0 0 1 5 7 Tm (Cafe old) Tj ET"
                } else {
                    "BT 1 0 0 1 5 80 Tm T* (Cafe old) Tj 0 -30 TD (Cafe old) Tj T* (Cafe old) Tj ET /F2 11 Tf 60 TL"
                };
                let prefix = if inherited_leading { "" } else { "13 TL " };
                stream.set_plain_content(format!("{prefix}{content}").into_bytes());
            } else {
                stream.dict.get_mut(b"Resources").unwrap().as_dict_mut().unwrap().get_mut(b"Font").unwrap().as_dict_mut().unwrap().set("F2", other);
                let prefix = if reference { "/F1 16 Tf 13 TL " } else { "/F1 16 Tf 13 TL q /F2 9 Tf 7 TL Q " };
                let mut content = parsed(prefix);
                content.extend(old.operations);
                content.extend(parsed("BT 1 0 0 1 200 150 Tm (Cafe) ' ET"));
                stream.set_plain_content(Content { operations: content }.encode().unwrap());
            }
        }
        pdf.save(path).unwrap();
    }
    let temp = tempfile::tempdir().unwrap(); let e = engine();
    let original = temp.path().join("source.pdf"); let reference = temp.path().join("reference.pdf");
    // The corrected renderer now agrees with explicit positions even when the
    // leaf relies entirely on the caller's leading. Preserve the same guards.
    let inherited = temp.path().join("inherited-leading.pdf");
    let inherited_reference = temp.path().join("inherited-reference.pdf");
    make(&inherited, false, true);
    make(&inherited_reference, true, true);
    let inherited_source = e.open_document(&inherited).unwrap();
    let inherited_expected = e.open_document(&inherited_reference).unwrap();
    let inherited_runs = e.list_form_text_runs(&inherited_source.id, 0).unwrap();
    assert_eq!(inherited_runs.runs.len(), 8, "{inherited_runs:?}");
    assert!(e.render_page(&inherited_source.id,0,1160).unwrap() == e.render_page(&inherited_expected.id,0,1160).unwrap());
    let inherited_changed = e.replace_form_text(&inherited_source.id,0,&[1,0,1],"Cafe old","Cafe older").unwrap();
    let inherited_after = e.list_form_text_runs(&inherited_changed.id,0).unwrap();
    assert_eq!(inherited_after.runs[1].text, "Cafe older");
    for (index, run) in inherited_after.runs.iter().enumerate() {
        if index != 1 { assert_eq!(run, &inherited_runs.runs[index]); }
    }
    for id in [&inherited_source.id, &inherited_expected.id, &inherited_changed.id] { e.close_document(id).unwrap(); }
    // Local leading still tests TD changing 13 to 30, later T*, q/Q font and
    // leading restoration, and child font/leading changes not leaking out.
    make(&original, false, false); make(&reference, true, false);
    let source = e.open_document(&original).unwrap(); let expected = e.open_document(&reference).unwrap();
    if let Some(root) = std::env::var_os("FOLIO_FORM_FLOW_DIAGNOSTICS") {
        let root=PathBuf::from(root); fs::create_dir_all(&root).unwrap();
        fs::copy(&original, root.join("source.pdf")).unwrap(); fs::copy(&reference, root.join("reference.pdf")).unwrap();
        fs::write(root.join("source-native.png"),e.render_page(&source.id,0,1160).unwrap()).unwrap();
        fs::write(root.join("reference-native.png"),e.render_page(&expected.id,0,1160).unwrap()).unwrap();
    }
    let before = e.list_form_text_runs(&source.id, 0).unwrap(); assert_eq!(before.runs.len(), 8, "{before:?}");
    assert!(before.runs.iter().all(|r| r.font_size == 16.));
    for page in 0..2 { assert!(e.render_page(&source.id, page, 1160).unwrap() == e.render_page(&expected.id, page, 1160).unwrap()); }
    let changed = e.replace_form_text(&source.id, 0, &[1,0,1], "Cafe old", "Cafe older").unwrap();
    let after = e.list_form_text_runs(&changed.id, 0).unwrap(); assert_eq!(after.runs.len(), 8);
    for (index, run) in after.runs.iter().enumerate() { if index != 1 { assert_eq!(run, &before.runs[index]); } }
    for id in [&source.id, &expected.id, &changed.id] { e.close_document(id).unwrap(); }
}

#[test]
fn mixed_cursor_tj_and_quote_lines_isolate_each_font_substitution() {
    let _lock = FONT_REGISTRY.lock().unwrap(); let temp = tempfile::tempdir().unwrap(); let e = engine();
    let path = temp.path().join("source.pdf"); fixture(&path, "standard", "TD", None);
    mutate_leaf(&path, |content, _| { content.operations = parsed("/F1 16 Tf BT 20 TL 1 0 0 1 5 80 Tm [18 -3] TJ [(Ca) -20 (fe old)] TJ [5 0] TJ 0 -20 TD 0 0.3 (Cafe old) \" ET"); });
    let source = e.open_document(&path).unwrap(); let before = e.list_form_text_runs(&source.id, 0).unwrap(); assert_eq!(before.runs.len(), 4, "{before:?}");
    assert_eq!(before.runs.iter().map(|r| r.object_path.clone()).collect::<Vec<_>>(), vec![vec![1,0,0],vec![1,0,1],vec![2,0,0],vec![2,0,1]]);
    let font = e.fonts().register(fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
    for line in [0, 1] {
        let changed = e.replace_form_text_with_font(&source.id, 0, &before.runs[line].object_path, "Cafe old", "Cafè edit", Some(&font.id)).unwrap();
        let after = e.list_form_text_runs(&changed.id, 0).unwrap(); assert_eq!(after.runs.len(), 4);
        for (index, run) in after.runs.iter().enumerate() { if index != line { assert_eq!(run, &before.runs[index]); } }
        let again = e.replace_form_text(&changed.id, 0, &after.runs[1-line].object_path, "Cafe old", "Cafe").unwrap();
        e.close_document(&again.id).unwrap(); e.close_document(&changed.id).unwrap();
    }
    e.close_document(&source.id).unwrap(); e.fonts().remove(&font.id).unwrap();
}
