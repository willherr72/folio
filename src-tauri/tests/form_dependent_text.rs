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

fn engine() -> PdfEngine {
    let library = std::env::var_os("FOLIO_TEST_PDFIUM_LIBRARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")
        });
    PdfEngine::start(library).unwrap()
}
fn encoded(text: &str, kind: &str) -> Object {
    Object::String(
        if kind == "cid" {
            text.encode_utf16().flat_map(u16::to_be_bytes).collect()
        } else {
            text.as_bytes().to_vec()
        },
        StringFormat::Hexadecimal,
    )
}
fn fixture(path: &Path, kind: &str, state: &str, first: &str, case: &str, rotation: i32) {
    authored::fixture(path, kind, case, rotation, "Cafe old");
    let mut pdf = Document::load(path).unwrap();
    for object in pdf.objects.values_mut() {
        let Ok(stream) = object.as_stream_mut() else {
            continue;
        };
        if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {
            continue;
        }
        let content = Content::decode(&stream.get_plain_content().unwrap()).unwrap();
        if !content.operations.iter().any(|op| op.operator == "Tj") {
            continue;
        }
        let mut operations =
            Content::decode(format!("BT /F1 16 Tf 30 TL {state} 1 0 0 1 5 80 Tm").as_bytes())
                .unwrap()
                .operations;
        operations.push(Operation::new("Tj", vec![encoded(first, kind)]));
        operations.push(Operation::new(
            "TJ",
            vec![Object::Array(vec![(-1800).into()])],
        ));
        operations.push(Operation::new("Tj", vec![encoded("Cafe old", kind)]));
        operations.push(Operation::new("T*", vec![]));
        operations.push(Operation::new("Tj", vec![encoded("Cafe old", kind)]));
        operations.push(Operation::new("ET", vec![]));
        stream.set_plain_content(Content { operations }.encode().unwrap());
    }
    pdf.save(path).unwrap();
}
fn page(source: &DocumentInfo, index: usize) -> PagePlan {
    PagePlan {
        id: format!("{}-{index}", source.id),
        source_id: source.id.clone(),
        page_index: index,
        width: source.pages[index].width,
        height: source.pages[index].height,
        rotation: 0,
        crop: None,
        overlays: vec![],
    }
}

fn mutate_leaf(path: &Path, edit: impl Fn(&mut Vec<Operation>)) {
    let mut pdf = Document::load(path).unwrap();
    for object in pdf.objects.values_mut() {
        let Ok(stream) = object.as_stream_mut() else {
            continue;
        };
        if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {
            continue;
        }
        let mut content = Content::decode(&stream.get_plain_content().unwrap()).unwrap();
        if content.operations.iter().any(|op| op.operator == "Tj") {
            edit(&mut content.operations);
            stream.set_plain_content(content.encode().unwrap());
        }
    }
    pdf.save(path).unwrap();
}

fn same_run(a: &folio_engine::FormTextRun, b: &folio_engine::FormTextRun) {
    let mut normalized = b.clone();
    normalized.bounds = a.bounds.clone();
    assert_eq!(a, &normalized);
    for (x, y) in [
        (a.bounds.x, b.bounds.x),
        (a.bounds.y, b.bounds.y),
        (a.bounds.width, b.bounds.width),
        (a.bounds.height, b.bounds.height),
    ] {
        assert!((x - y).abs() < 0.001, "{a:?} versus {b:?}");
    }
}

#[test]
fn nonblank_pieces_retain_ordinary_surrounding_spaces() {
    let temp = tempfile::tempdir().unwrap();
    let e = engine();
    for kind in ["standard", "simple", "cid"] {
        let path = temp.path().join("source.pdf");
        fixture(&path, kind, "0.3 Tc 1.25 Tw", " Cafe old ", "plain", 0);
        let source = e.open_document(&path).unwrap();
        let before = e.list_form_text_runs(&source.id, 0).unwrap();
        assert_eq!(before.runs.len(), 6, "{kind}: {before:?}");
        assert_eq!(before.runs[0].text, " Cafe old ");
        let changed = e
            .replace_form_text(
                &source.id,
                0,
                &before.runs[0].object_path,
                " Cafe old ",
                "  Cafe ",
            )
            .unwrap_or_else(|error| panic!("{kind}: {error}"));
        let after = e.list_form_text_runs(&changed.id, 0).unwrap();
        assert_eq!(after.runs[0].text, "  Cafe ");
        for (a, b) in before.runs[1..].iter().zip(&after.runs[1..]) {
            same_run(a, b);
        }
        assert!(e
            .replace_form_text(
                &changed.id,
                0,
                &before.runs[0].object_path,
                "  Cafe ",
                "   "
            )
            .is_err());
        let restored = e
            .replace_form_text(
                &changed.id,
                0,
                &before.runs[0].object_path,
                "  Cafe ",
                " Cafe old ",
            )
            .unwrap();
        assert_eq!(
            e.render_page(&restored.id, 0, 1160).unwrap(),
            e.render_page(&source.id, 0, 1160).unwrap()
        );
        for id in [&restored.id, &changed.id, &source.id] {
            e.close_document(id).unwrap();
        }
    }
}

#[test]
fn positioned_arrays_and_spacing_changes_preserve_dependent_neighbors() {
    let temp = tempfile::tempdir().unwrap();
    let e = engine();
    for kind in ["standard", "simple", "cid"] {
        for (case, rotation) in [("plain", 0), ("form-shear", 90)] {
            let path = temp.path().join("source.pdf");
            fixture(
                &path,
                kind,
                "0.3 Tc 1.25 Tw 75 Tz 2 Ts",
                "Cafe old",
                case,
                rotation,
            );
            mutate_leaf(&path, |ops| {
                let index = ops.iter().position(|op| op.operator == "Tj").unwrap();
                ops[index] = Operation::new(
                    "TJ",
                    vec![Object::Array(vec![
                        encoded("Ca", kind),
                        (-20).into(),
                        encoded("fe old", kind),
                    ])],
                );
                // The compensation must precede changes to font size/Tc/Tw/Tz.
                let changes = Content::decode(b"/F1 12 Tf 0.1 Tc 0.5 Tw 90 Tz")
                    .unwrap()
                    .operations;
                ops.splice(index + 2..index + 2, changes);
            });
            let source = e.open_document(&path).unwrap();
            let before = e.list_form_text_runs(&source.id, 0).unwrap();
            assert_eq!(before.runs.len(), 6, "{kind}/{case}: {before:?}");
            let changed = e
                .replace_form_text(
                    &source.id,
                    0,
                    &before.runs[0].object_path,
                    "Cafe old",
                    "Cafe",
                )
                .unwrap_or_else(|error| panic!("{kind}/{case}: {error}"));
            let after = e.list_form_text_runs(&changed.id, 0).unwrap();
            for (a, b) in before.runs[1..].iter().zip(&after.runs[1..]) {
                same_run(a, b);
            }
            let output = temp.path().join("saved.pdf");
            e.export_pdf(
                ExportRequest {
                    pages: vec![page(&changed, 0), page(&source, 1)],
                    flatten: false,
                },
                &output,
            )
            .unwrap();
            if let Some(root) = std::env::var_os("FOLIO_DEPENDENT_ARTIFACTS") {
                let root = PathBuf::from(root);
                fs::create_dir_all(&root).unwrap();
                fs::copy(
                    &path,
                    root.join(format!("{kind}-{case}-spacing-source.pdf")),
                )
                .unwrap();
                fs::copy(
                    &output,
                    root.join(format!("{kind}-{case}-spacing-saved.pdf")),
                )
                .unwrap();
            }
            for id in [&changed.id, &source.id] {
                e.close_document(id).unwrap();
            }
        }
    }
}

#[test]
fn consecutive_pieces_preserve_following_text_shared_instances_and_saved_pages() {
    let temp = tempfile::tempdir().unwrap();
    let e = engine();
    for kind in ["standard", "simple", "cid"] {
        let path = temp.path().join("source.pdf");
        fixture(&path, kind, "", "Cafe old", "plain", 0);
        let original = fs::read(&path).unwrap();
        let source = e.open_document(&path).unwrap();
        let before = e.list_form_text_runs(&source.id, 0).unwrap();
        assert_eq!(before.runs.len(), 6, "{kind}: {before:?}");
        for selected in 0..3 {
            let replacement = if selected == 0 { "Cafe" } else { "Cafe older" };
            let changed = e
                .replace_form_text(
                    &source.id,
                    0,
                    &before.runs[selected].object_path,
                    "Cafe old",
                    replacement,
                )
                .unwrap_or_else(|error| panic!("{kind}/{selected}: {error}"));
            let after = e.list_form_text_runs(&changed.id, 0).unwrap();
            assert_eq!(after.runs.len(), 6);
            for (i, (a, b)) in before.runs.iter().zip(&after.runs).enumerate() {
                assert_eq!(a.object_path, b.object_path);
                if i == selected {
                    assert_eq!(b.text, replacement);
                    continue;
                }
                assert_eq!(a.text, b.text);
                assert_eq!(a.font_name, b.font_name);
                assert_eq!(a.font_size, b.font_size);
                for (x, y) in [
                    (a.bounds.x, b.bounds.x),
                    (a.bounds.y, b.bounds.y),
                    (a.bounds.width, b.bounds.width),
                    (a.bounds.height, b.bounds.height),
                ] {
                    assert!(
                        (x - y).abs() < 0.001,
                        "{kind}/{selected}/{i}: {a:?} vs {b:?}"
                    );
                }
            }
            let saved_path = temp.path().join("saved.pdf");
            e.export_pdf(
                ExportRequest {
                    pages: vec![page(&changed, 0), page(&source, 1)],
                    flatten: false,
                },
                &saved_path,
            )
            .unwrap();
            let saved = e.open_document(&saved_path).unwrap();
            assert_eq!(
                e.list_form_text_runs(&saved.id, 0).unwrap().runs[selected].text,
                replacement
            );
            assert_eq!(
                e.render_page(&saved.id, 1, 1160).unwrap(),
                e.render_page(&source.id, 1, 1160).unwrap()
            );
            let restored = e
                .replace_form_text(
                    &saved.id,
                    0,
                    &after.runs[selected].object_path,
                    replacement,
                    "Cafe old",
                )
                .unwrap();
            assert_eq!(
                e.render_page(&restored.id, 0, 1160).unwrap(),
                e.render_page(&source.id, 0, 1160).unwrap()
            );
            if let Some(output) = std::env::var_os("FOLIO_DEPENDENT_ARTIFACTS") {
                let output = PathBuf::from(output);
                fs::create_dir_all(&output).unwrap();
                fs::write(
                    output.join(format!("{kind}-{selected}-source.pdf")),
                    &original,
                )
                .unwrap();
                fs::copy(
                    &saved_path,
                    output.join(format!("{kind}-{selected}-saved.pdf")),
                )
                .unwrap();
            }
            e.close_document(&restored.id).unwrap();
            e.close_document(&saved.id).unwrap();
            e.close_document(&changed.id).unwrap();
        }
        assert_eq!(fs::read(path).unwrap(), original);
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn repeated_edits_coalesce_cursor_operations_and_restore_original_pixels() {
    let temp = tempfile::tempdir().unwrap();
    let e = engine();
    for kind in ["standard", "simple", "cid"] {
        let path = temp.path().join("source.pdf");
        fixture(&path, kind, "", "Cafe old", "plain", 0);
        let source = e.open_document(&path).unwrap();
        let before = e.list_form_text_runs(&source.id, 0).unwrap();
        let mut current = source.clone();
        let mut old = "Cafe old";
        for iteration in 0..10 {
            let replacement = if iteration % 2 == 0 {
                "Cafe"
            } else {
                "Cafe old"
            };
            let changed = e
                .replace_form_text(
                    &current.id,
                    0,
                    &before.runs[0].object_path,
                    old,
                    replacement,
                )
                .unwrap();
            let after = e.list_form_text_runs(&changed.id, 0).unwrap();
            for (a, b) in before.runs[1..].iter().zip(&after.runs[1..]) {
                same_run(a, b);
            }
            if current.id != source.id {
                e.close_document(&current.id).unwrap();
            }
            current = changed;
            old = replacement;
        }
        assert_eq!(
            e.render_page(&current.id, 0, 1160).unwrap(),
            e.render_page(&source.id, 0, 1160).unwrap()
        );
        let output = temp.path().join("saved.pdf");
        e.export_pdf(
            ExportRequest {
                pages: vec![page(&current, 0)],
                flatten: false,
            },
            &output,
        )
        .unwrap();
        let pdf = Document::load(&output).unwrap();
        let mut leaves = 0;
        for object in pdf.objects.values() {
            let Ok(stream) = object.as_stream() else {
                continue;
            };
            if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {
                continue;
            }
            let content = Content::decode(&stream.get_plain_content().unwrap()).unwrap();
            if !content.operations.iter().any(|op| op.operator == "Tj") {
                continue;
            }
            leaves += 1;
            assert_eq!(
                content
                    .operations
                    .iter()
                    .filter(|op| op.operator == "TJ")
                    .count(),
                1,
                "{kind}: repeated corrections accumulated"
            );
        }
        assert!(leaves > 0);
        for id in [&current.id, &source.id] {
            e.close_document(id).unwrap();
        }
    }
}

#[test]
fn explicit_font_substitution_restores_following_source_font_and_cursor() {
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
        let path = temp.path().join("source.pdf");
        fixture(&path, kind, "0.2 Tc 80 Tz", "Cafe old", "plain", 0);
        let source = e.open_document(&path).unwrap();
        let before = e.list_form_text_runs(&source.id, 0).unwrap();
        let changed = e
            .replace_form_text_with_font(
                &source.id,
                0,
                &before.runs[0].object_path,
                "Cafe old",
                "Cafè",
                Some(&font.id),
            )
            .unwrap_or_else(|error| panic!("{kind}: {error}"));
        let after = e.list_form_text_runs(&changed.id, 0).unwrap();
        assert_eq!(after.runs[0].text, "Cafè");
        assert!(after.runs[0].is_embedded);
        for (a, b) in before.runs[1..].iter().zip(&after.runs[1..]) {
            same_run(a, b);
        }
        let saved_path = temp.path().join("saved.pdf");
        e.export_pdf(
            ExportRequest {
                pages: vec![page(&changed, 0)],
                flatten: false,
            },
            &saved_path,
        )
        .unwrap();
        let saved = e.open_document(&saved_path).unwrap();
        let again = e
            .replace_form_text(&saved.id, 0, &before.runs[0].object_path, "Cafè", "Cafe")
            .unwrap();
        let second = e.list_form_text_runs(&again.id, 0).unwrap();
        for (a, b) in before.runs[1..].iter().zip(&second.runs[1..]) {
            same_run(a, b);
        }
        for id in [&again.id, &saved.id, &changed.id, &source.id] {
            e.close_document(id).unwrap();
        }
    }
    e.fonts().remove(&font.id).unwrap();
}

#[test]
fn alternating_fonts_and_saved_reedits_keep_correction_and_resources_bounded() {
    let temp = tempfile::tempdir().unwrap();
    let e = engine();
    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures");
    let fonts = ["corpus/fonts/DejaVuSerif.ttf", "shaped-text/DejaVuSans.ttf"].map(|path| {
        e.fonts()
            .register(fs::read(fixture_root.join(path)).unwrap())
            .unwrap()
    });
    let path = temp.path().join("source.pdf");
    fixture(&path, "standard", "", "Cafe old", "plain", 0);
    let source = e.open_document(&path).unwrap();
    let before = e.list_form_text_runs(&source.id, 0).unwrap();
    let mut current = source.clone();
    let mut old = "Cafe old";
    for iteration in 0..6 {
        let replacement = if iteration % 2 == 0 {
            "Cafe"
        } else {
            "Cafe old"
        };
        let changed = e
            .replace_form_text_with_font(
                &current.id,
                0,
                &before.runs[0].object_path,
                old,
                replacement,
                Some(&fonts[iteration % 2].id),
            )
            .unwrap();
        let after = e.list_form_text_runs(&changed.id, 0).unwrap();
        for (a, b) in before.runs[1..].iter().zip(&after.runs[1..]) {
            same_run(a, b);
        }
        let output = temp.path().join(format!("saved-{iteration}.pdf"));
        e.export_pdf(
            ExportRequest {
                pages: vec![page(&changed, 0)],
                flatten: false,
            },
            &output,
        )
        .unwrap();
        let saved = e.open_document(&output).unwrap();
        if current.id != source.id {
            e.close_document(&current.id).unwrap();
        }
        e.close_document(&changed.id).unwrap();
        current = saved;
        old = replacement;
    }
    let pdf = Document::load(temp.path().join("saved-5.pdf")).unwrap();
    for object in pdf.objects.values() {
        let Ok(stream) = object.as_stream() else {
            continue;
        };
        if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {
            continue;
        }
        let content = Content::decode(&stream.get_plain_content().unwrap()).unwrap();
        if !content.operations.iter().any(|op| op.operator == "Tj") {
            continue;
        }
        assert!(
            content
                .operations
                .iter()
                .filter(|op| op.operator == "TJ")
                .count()
                <= 1,
            "font substitutions accumulated cursor operations"
        );
        assert!(
            content
                .operations
                .iter()
                .filter(|op| op.operator == "Tf")
                .count()
                <= 3,
            "font substitutions accumulated selection operations"
        );
        if let Ok(resources) = stream.dict.get(b"Resources") {
            let (_, resources) = pdf.dereference(resources).unwrap();
            let fonts = resources.as_dict().unwrap().get(b"Font").unwrap();
            let (_, fonts) = pdf.dereference(fonts).unwrap();
            assert!(
                fonts.as_dict().unwrap().len() <= 3,
                "obsolete font resource aliases were retained"
            );
        }
    }
    for id in [&current.id, &source.id] {
        e.close_document(id).unwrap();
    }
    for font in fonts {
        e.fonts().remove(&font.id).unwrap();
    }
}
