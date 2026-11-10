use folio_engine::{DocumentInfo, ExportRequest, PagePlan, PdfEngine};
use lopdf::{content::Content, Document, Object, StringFormat};
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

fn mutate_leaf(path: &Path, mut change: impl FnMut(&mut Content)) {
    let mut pdf = Document::load(path).unwrap();
    for object in pdf.objects.values_mut() {
        let Ok(stream) = object.as_stream_mut() else {
            continue;
        };
        if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {
            continue;
        }
        let mut content = Content::decode(&stream.get_plain_content().unwrap()).unwrap();
        if !content.operations.iter().any(|op| op.operator == "TJ") {
            continue;
        }
        change(&mut content);
        stream.set_plain_content(content.encode().unwrap());
    }
    pdf.save(path).unwrap();
}
fn parsed(data: &[u8]) -> Vec<lopdf::content::Operation> {
    Content::decode(data).unwrap().operations
}
fn text_arrays(bytes: &[u8]) -> Vec<Vec<Object>> {
    let pdf = Document::load_mem(bytes).unwrap();
    pdf.objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .filter(|s| s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Form"))
        .flat_map(|s| {
            Content::decode(&s.get_plain_content().unwrap())
                .unwrap()
                .operations
        })
        .filter_map(|op| {
            if op.operator == "TJ" {
                op.operands.into_iter().next().and_then(|v| {
                    if let Object::Array(values) = v {
                        Some(values)
                    } else {
                        None
                    }
                })
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn cursor_arrays_keep_paths_and_text_state_after_the_selected_show() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("cursor.pdf");
    let e = engine();
    fixture(&path, "standard", "Cafe old");
    mutate_leaf(&path, |content| {
        let index = content
            .operations
            .iter()
            .position(|op| op.operator == "TJ")
            .unwrap();
        content
            .operations
            .splice(index..index, parsed(b"[18 -3 0] TJ [] TJ"));
        let index = content
            .operations
            .iter()
            .rposition(|op| op.operator == "TJ")
            .unwrap();
        content
            .operations
            .splice(index + 1..index + 1, parsed(b"[5 0] TJ 2 Tw"));
        content
            .operations
            .extend(parsed(b"BT /F1 16 Tf 1 0 0 1 5 70 Tm (Cafe) Tj ET"));
    });
    let source = e.open_document(&path).unwrap();
    let listed = e.list_form_text_runs(&source.id, 0).unwrap();
    assert_eq!(listed.runs.len(), 4, "{listed:?}");
    assert_eq!(
        listed
            .runs
            .iter()
            .map(|r| r.object_path.clone())
            .collect::<Vec<_>>(),
        vec![vec![1, 0, 0], vec![1, 0, 1], vec![2, 0, 0], vec![2, 0, 1]]
    );
    assert_eq!(
        listed
            .runs
            .iter()
            .map(|r| r.can_substitute)
            .collect::<Vec<_>>(),
        [true, false, true, false]
    );
    let same = e
        .replace_form_text(
            &source.id,
            0,
            &listed.runs[0].object_path,
            "Cafe old",
            "Cafe old",
        )
        .unwrap();
    assert!(
        e.render_page(&same.id, 0, 1160).unwrap() == e.render_page(&source.id, 0, 1160).unwrap()
    );
    let changed = e
        .replace_form_text(
            &source.id,
            0,
            &listed.runs[0].object_path,
            "Cafe old",
            "Cafe edit",
        )
        .unwrap();
    let after = e.list_form_text_runs(&changed.id, 0).unwrap().runs;
    assert_eq!(after[0].text, "Cafe edit");
    assert_eq!(after[1..], listed.runs[1..]);
    let arrays = text_arrays(&e.source_bytes(&changed.id).unwrap());
    assert!(arrays
        .iter()
        .any(|a| a == &vec![18.into(), (-3).into(), 0.into()]));
    assert!(arrays.iter().any(Vec::is_empty));
    assert!(arrays.iter().any(|a| a == &vec![5.into(), 0.into()]));
    for id in [&source.id, &same.id, &changed.id] {
        e.close_document(id).unwrap();
    }
}

#[test]
fn positioned_font_substitution_reencodes_each_logical_slot() {
    let _lock = FONT_REGISTRY.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    let e = engine();
    let bytes = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf"),
    )
    .unwrap();
    let font = e.fonts().register(bytes).unwrap();
    for kind in ["standard", "simple", "cid"] {
        let old = if kind == "standard" {
            "Cafe old"
        } else {
            "Café old"
        };
        if kind == "cid" {
            physical_fixture(&path, kind, "form-shear", 90, 1.7, old);
        } else {
            fixture(&path, kind, old);
        }
        let source = e.open_document(&path).unwrap();
        let before = e.list_form_text_runs(&source.id, 0).unwrap().runs;
        let changed = e
            .replace_form_text_with_font(
                &source.id,
                0,
                &before[0].object_path,
                old,
                "Cafè edit",
                Some(&font.id),
            )
            .unwrap();
        let after = e.list_form_text_runs(&changed.id, 0).unwrap().runs;
        assert_eq!(after[0].text, "Cafè edit");
        assert_eq!(after[1], before[1]);
        let encode = |text: &str| {
            string(
                &text
                    .encode_utf16()
                    .flat_map(u16::to_be_bytes)
                    .collect::<Vec<_>>(),
            )
        };
        let expected = vec![
            25.into(),
            encode("Ca"),
            (-20).into(),
            10.into(),
            encode("fè edi"),
            45.into(),
            encode("t"),
        ];
        assert!(
            text_arrays(&e.source_bytes(&changed.id).unwrap()).contains(&expected),
            "{kind}: wrong logical slots"
        );
        let same = e
            .replace_form_text_with_font(
                &changed.id,
                0,
                &after[0].object_path,
                "Cafè edit",
                "Cafè edit",
                Some(&font.id),
            )
            .unwrap();
        assert!(
            e.render_page(&same.id, 0, 1160).unwrap()
                == e.render_page(&changed.id, 0, 1160).unwrap()
        );
        if let Some(root) = std::env::var_os("FOLIO_FORM_POSITIONED_ARTIFACTS") {
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
    e.fonts().remove(&font.id).unwrap();
}

#[test]
fn positioned_arrays_keep_the_word_spacing_font_guard() {
    let _lock = FONT_REGISTRY.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    let e = engine();
    let bytes = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf"),
    )
    .unwrap();
    let font = e.fonts().register(bytes).unwrap();
    for kind in ["standard", "simple", "cid"] {
        let old = if kind == "standard" {
            "Cafe old"
        } else {
            "Café old"
        };
        fixture(&path, kind, old);
        mutate_leaf(&path, |content| {
            content.operations.splice(0..0, parsed(b"2 Tw"));
        });
        let source = e.open_document(&path).unwrap();
        let listed = e.list_form_text_runs(&source.id, 0).unwrap();
        assert_eq!(listed.runs.len(), 2, "{listed:?}");
        let run = &listed.runs[0];
        assert_eq!(run.can_substitute, kind == "cid");
        let result = e.replace_form_text_with_font(
            &source.id,
            0,
            &run.object_path,
            old,
            "Cafe edit",
            Some(&font.id),
        );
        if kind == "cid" {
            e.close_document(&result.unwrap().id).unwrap();
        } else {
            assert!(result.unwrap_err().to_string().contains("word spacing"));
        }
        e.close_document(&source.id).unwrap();
    }
    e.fonts().remove(&font.id).unwrap();
}

#[test]
fn malformed_arrays_and_split_character_codes_leave_sources_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("bad.pdf");
    let e = engine();
    for case in [
        "nested",
        "name",
        "huge",
        "count",
        "empty-strings",
        "outside",
        "split-code",
    ] {
        fixture(
            &path,
            if case == "split-code" {
                "cid"
            } else {
                "standard"
            },
            if case == "split-code" {
                "Café old"
            } else {
                "Cafe old"
            },
        );
        mutate_leaf(&path, |content| {
            let index = content
                .operations
                .iter()
                .position(|op| op.operator == "TJ")
                .unwrap();
            let op = &mut content.operations[index];
            match case {
                "nested" => op.operands = vec![Object::Array(vec![Object::Array(vec![])])],
                "name" => op.operands = vec![Object::Array(vec![Object::Name(b"Bad".to_vec())])],
                "huge" => {
                    op.operands = vec![Object::Array(vec![string(b"Cafe old"), 1_000_001.into()])]
                }
                "count" => op.operands = vec![Object::Array(vec![0.into(); 16_385])],
                "empty-strings" => {
                    op.operands = vec![Object::Array(vec![string(b""), 0.into(), string(b"")])]
                }
                "outside" => {
                    content.operations.splice(0..0, parsed(b"[0] TJ"));
                }
                "split-code" => {
                    op.operands = vec![Object::Array(vec![
                        string(b"\0"),
                        0.into(),
                        string(b"C\0a\0f\0\xe9\0 \0o\0l\0d"),
                    ])]
                }
                _ => unreachable!(),
            }
        });
        let original = fs::read(&path).unwrap();
        let source = e.open_document(&path).unwrap();
        let listed = e.list_form_text_runs(&source.id, 0).unwrap();
        assert!(
            listed.runs.is_empty() && listed.reason.is_some(),
            "{case}: {listed:?}"
        );
        assert!(e
            .replace_form_text(&source.id, 0, &[1, 0, 0], "Cafe old", "Cafe edit")
            .is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        e.close_document(&source.id).unwrap();
    }
}
fn string(bytes: &[u8]) -> Object {
    Object::String(bytes.to_vec(), StringFormat::Hexadecimal)
}

// Authored reference uses two explicit slots (2 and 8 source characters).
// It does not invoke the production rewriting or encoding helpers.
fn fixture(path: &Path, kind: &str, first: &str) {
    authored::fixture(path, kind, "plain", 0, first);
    make_arrays(path, kind);
}
fn make_arrays(path: &Path, kind: &str) {
    let mut pdf = Document::load(path).unwrap();
    let width = if kind == "cid" { 2 } else { 1 };
    let old = if kind == "standard" {
        b"Cafe old".to_vec()
    } else if kind == "simple" {
        b"Caf\x01 old".to_vec()
    } else {
        "Café old"
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect()
    };
    for object in pdf.objects.values_mut() {
        let Ok(stream) = object.as_stream_mut() else {
            continue;
        };
        if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {
            continue;
        }
        let mut content = Content::decode(&stream.get_plain_content().unwrap()).unwrap();
        for op in &mut content.operations {
            if op.operator != "Tj" {
                continue;
            }
            let bytes = op.operands[0].as_str().unwrap();
            let a = (2 * width).min(bytes.len());
            let b = (8 * width).min(bytes.len());
            let mut values = vec![25.into(), string(&bytes[..a])];
            if bytes.len() > 2 * width || bytes == old {
                values.extend([(-20).into(), 10.into()]);
            }
            values.push(string(&bytes[a..b]));
            if bytes.len() > 8 * width || bytes == old {
                values.push(45.into());
            }
            if bytes.len() > 8 * width {
                values.push(string(&bytes[b..]));
            }
            op.operator = "TJ".into();
            op.operands = vec![Object::Array(values)];
        }
        stream.set_plain_content(content.encode().unwrap());
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
fn physical_fixture(path: &Path, kind: &str, case: &str, rotation: i32, unit: f32, first: &str) {
    authored::fixture(path, kind, case, rotation, first);
    make_arrays(path, kind);
    mutate_leaf(path, |content| {
        content
            .operations
            .splice(0..0, parsed(b"0.3 Tc 90 Tz 2 Ts 14 TL"));
    });
    let mut pdf = Document::load(path).unwrap();
    for page in pdf.get_pages().into_values() {
        pdf.get_dictionary_mut(page)
            .unwrap()
            .set("UserUnit", Object::Real(unit));
    }
    pdf.save(path).unwrap();
}

#[test]
fn positioned_affine_physical_edits_survive_save_reopen_and_restore() {
    let temp = tempfile::tempdir().unwrap();
    let e = engine();
    for (kind, case, rotation, unit) in [
        ("standard", "plain", 0, 1.7),
        ("standard", "mirror", 180, 1.),
        ("simple", "form-shear", 90, 2.),
        ("cid", "composed", 270, 0.5),
    ] {
        let path = temp.path().join("original.pdf");
        let old = if kind == "standard" {
            "Cafe old"
        } else {
            "Café old"
        };
        physical_fixture(&path, kind, case, rotation, unit, old);
        let original = fs::read(&path).unwrap();
        let source = e.open_document(&path).unwrap();
        let listed = e.list_form_text_runs(&source.id, 0).unwrap();
        assert_eq!(listed.runs.len(), 2, "{kind}/{case}/{rotation}: {listed:?}");
        for (label, replacement) in [("noop", old), ("shorter", "Ca"), ("longer", "Cafe older")] {
            let changed = e
                .replace_form_text(&source.id, 0, &listed.runs[0].object_path, old, replacement)
                .unwrap_or_else(|err| panic!("{kind}/{case}/{rotation}/{label}: {err}"));
            let reference = temp.path().join("reference.pdf");
            physical_fixture(&reference, kind, case, rotation, unit, replacement);
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
            for page in 0..2 {
                assert!(
                    e.render_page(&reopened.id, page, 1160).unwrap()
                        == e.render_page(&expected.id, page, 1160).unwrap(),
                    "{kind}/{case}/{rotation}/{label}/{page}: pixels"
                );
            }
            let after = e.list_form_text_runs(&reopened.id, 0).unwrap().runs;
            assert_eq!(after[0].text, replacement);
            assert_eq!(after[1], listed.runs[1]);
            let restored = e
                .replace_form_text(&reopened.id, 0, &after[0].object_path, replacement, old)
                .unwrap();
            // A shortened edit intentionally discards gaps after its new end. A
            // later extension cannot reconstruct deleted source positioning.
            if label != "shorter" {
                assert!(
                    e.render_page(&restored.id, 0, 1160).unwrap()
                        == e.render_page(&source.id, 0, 1160).unwrap()
                );
            }
            assert_eq!(fs::read(&path).unwrap(), original);
            if let Some(root) = std::env::var_os("FOLIO_FORM_POSITIONED_ARTIFACTS") {
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

#[test]
fn positioned_array_growth_respects_neighbors_and_ancestor_boxes() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    let e = engine();
    for case in ["ancestor-edge", "leaf-edge", "collision"] {
        fixture(&path, "standard", "Cafe old");
        authored::mutate(&path, case);
        let original = fs::read(&path).unwrap();
        let source = e.open_document(&path).unwrap();
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
        assert_eq!(fs::read(&path).unwrap(), original);
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn positioned_recovery_retains_arrays_without_original_source() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    fixture(&path, "cid", "Café old");
    let e = engine();
    let source = e.open_document(&path).unwrap();
    let run = e.list_form_text_runs(&source.id, 0).unwrap().runs.remove(0);
    let changed = e
        .replace_form_text(&source.id, 0, &run.object_path, &run.text, "Cafe older")
        .unwrap();
    let raster = e.render_page(&changed.id, 0, 1160).unwrap();
    let arrays = text_arrays(&e.source_bytes(&changed.id).unwrap());
    let store = folio_engine::RecoveryStore::new(temp.path().join("recovery")).unwrap();
    store.save(&e,serde_json::json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","savedDigest":"old","dirty":true,"zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"Positioned.pdf","selectedPageId":format!("{}-0",changed.id),"selectedOverlayId":null,"pages":[page_plan(&changed,0)]}}]})).unwrap();
    e.close_document(&source.id).unwrap();
    e.close_document(&changed.id).unwrap();
    fs::rename(&path, temp.path().join("unavailable.pdf")).unwrap();
    let restored = store.load(&e).unwrap().unwrap();
    let id = restored["tabs"][0]["document"]["pages"][0]["sourceId"]
        .as_str()
        .unwrap();
    assert!(e.render_page(id, 0, 1160).unwrap() == raster);
    assert_eq!(text_arrays(&e.source_bytes(id).unwrap()), arrays);
    let run = e.list_form_text_runs(id, 0).unwrap().runs.remove(0);
    let again = e
        .replace_form_text(id, 0, &run.object_path, "Cafe older", "Cafe old")
        .unwrap();
    e.close_document(&again.id).unwrap();
    e.close_document(id).unwrap();
}

#[test]
fn positioned_edits_preserve_source_permissions_and_export_provenance() {
    use folio_engine::{protection::unlock_pdf, EngineError};
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    let e = engine();
    for allowed in [true, false] {
        fixture(&path, "standard", "Cafe old");
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
                    pages: vec![page_plan(&changed, 0), page_plan(&source, 1)],
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
fn nested_positioned_arrays_preserve_original_font_edits_and_shared_copies() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    let e = engine();
    for kind in ["standard", "simple", "cid"] {
        let old = if kind == "standard" {
            "Cafe old"
        } else {
            "Café old"
        };
        fixture(&path, kind, old);
        let original = fs::read(&path).unwrap();
        let source = e.open_document(&path).unwrap();
        let listed = e.list_form_text_runs(&source.id, 0).unwrap();
        assert_eq!(listed.runs.len(), 2, "{kind}: {listed:?}");
        for replacement in [old, "Ca", "Cafe", "Cafe older"] {
            let changed = e
                .replace_form_text(&source.id, 0, &listed.runs[0].object_path, old, replacement)
                .unwrap();
            let reference = temp.path().join("reference.pdf");
            fixture(&reference, kind, replacement);
            let expected = e.open_document(&reference).unwrap();
            assert!(
                e.render_page(&changed.id, 0, 1160).unwrap()
                    == e.render_page(&expected.id, 0, 1160).unwrap(),
                "{kind}: {replacement}"
            );
            let after = e.list_form_text_runs(&changed.id, 0).unwrap().runs;
            assert_eq!(after[0].text, replacement);
            assert_eq!(after[1], listed.runs[1]);
            assert_eq!(fs::read(&path).unwrap(), original);
            e.close_document(&changed.id).unwrap();
            e.close_document(&expected.id).unwrap();
        }
        e.close_document(&source.id).unwrap();
    }
}
