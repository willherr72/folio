//! Candidate renderer acceptance. The override is test-only; the product never
//! reads this environment variable or changes its library selection.
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

fn parsed(text: &str) -> Vec<Operation> {
    Content::decode(text.as_bytes()).unwrap().operations
}

fn text(text: &str, kind: &str) -> Object {
    let bytes = if kind == "cid" {
        text.encode_utf16().flat_map(u16::to_be_bytes).collect()
    } else {
        text.as_bytes().to_vec()
    };
    Object::String(bytes, StringFormat::Hexadecimal)
}

fn fixture(
    path: &Path,
    kind: &str,
    state: &str,
    case: &str,
    rotation: i32,
    local_reference: bool,
    first: &str,
) {
    authored::fixture(path, kind, case, rotation, first);
    let mut pdf = Document::load(path).unwrap();
    let full_state = format!("0 TL 0 Ts 100 Tz {state} ");
    for object in pdf.objects.values_mut() {
        let Ok(stream) = object.as_stream_mut() else {
            continue;
        };
        if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {
            continue;
        }
        let old = Content::decode(&stream.get_plain_content().unwrap()).unwrap();
        let leaf = old.operations.iter().any(|op| op.operator == "Tj");
        let mut operations = vec![];
        if leaf {
            if local_reference {
                operations.extend(parsed(&full_state));
            }
            for operation in old.operations {
                if operation.operator == "Tj" {
                    operations.extend(parsed("T*"));
                }
                operations.push(operation);
            }
            if !local_reference {
                // A child may change its own text state; the caller's tail must
                // still use its original values when the form returns.
                operations.extend(parsed("BT 70 TL 12 Ts 40 Tz ET"));
            }
        } else {
            operations.extend(parsed(&full_state));
            operations.extend(parsed("q 3 TL 1 Ts 90 Tz Q"));
            // No q/Q around Do: this tests the form call's own state isolation.
            operations.extend(
                old.operations
                    .into_iter()
                    .filter(|op| !matches!(op.operator.as_str(), "q" | "Q")),
            );
            operations.extend(parsed("BT /F1 12 Tf 1 0 0 1 170 80 Tm T*"));
            // Stay within the CID fixture's deliberately limited ToUnicode map.
            operations.push(Operation::new("Tj", vec![text("Cafe", kind)]));
            operations.extend(parsed("ET"));
        }
        stream.set_plain_content(Content { operations }.encode().unwrap());
    }
    pdf.save(path).unwrap();
}

fn page(source: &DocumentInfo, index: usize) -> PagePlan {
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

#[test]
fn inherited_scalars_preserve_real_edits_and_caller_state() {
    let temp = tempfile::tempdir().unwrap();
    let e = engine();
    for kind in ["standard", "simple", "cid"] {
        for (state_name, state) in [
            ("leading", "17 TL"),
            ("rise", "9 Ts"),
            ("scale", "65 Tz"),
            ("combined", "17 TL 9 Ts 65 Tz"),
        ] {
            let old = if kind == "standard" {
                "Cafe old"
            } else {
                "Café old"
            };
            let path = temp.path().join("source.pdf");
            let reference_path = temp.path().join("reference.pdf");
            fixture(&path, kind, state, "plain", 0, false, old);
            fixture(&reference_path, kind, state, "plain", 0, true, old);
            let bytes = fs::read(&path).unwrap();
            let source = e.open_document(&path).unwrap();
            let reference = e.open_document(&reference_path).unwrap();
            let listed = e.list_form_text_runs(&source.id, 0).unwrap();
            assert_eq!(listed.runs.len(), 4, "{kind}/{state_name}: {listed:?}");
            assert!(
                e.render_page(&source.id, 0, 1160).unwrap()
                    == e.render_page(&reference.id, 0, 1160).unwrap(),
                "{kind}/{state_name}: inherited source differs from explicit reference"
            );
            e.close_document(&reference.id).unwrap();
            let run = &listed.runs[0];
            assert_eq!(run.text, old);
            let changed = e
                .replace_form_text(&source.id, 0, &run.object_path, old, "Cafe older")
                .unwrap();
            let after = e.list_form_text_runs(&changed.id, 0).unwrap();
            assert_eq!(after.runs.len(), 4);
            for index in 1..4 {
                assert_eq!(after.runs[index], listed.runs[index]);
            }
            fixture(&reference_path, kind, state, "plain", 0, true, "Cafe older");
            let reference = e.open_document(&reference_path).unwrap();
            assert!(
                e.render_page(&changed.id, 0, 1160).unwrap()
                    == e.render_page(&reference.id, 0, 1160).unwrap(),
                "{kind}/{state_name}: edited pixels differ from authored reference"
            );
            let saved_path = temp.path().join(format!("{kind}-{state_name}-saved.pdf"));
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
                e.list_form_text_runs(&saved.id, 0).unwrap().runs[0].text,
                "Cafe older"
            );
            assert!(
                e.render_page(&saved.id, 0, 1160).unwrap()
                    == e.render_page(&reference.id, 0, 1160).unwrap()
            );
            assert!(
                e.render_page(&saved.id, 1, 1160).unwrap()
                    == e.render_page(&source.id, 1, 1160).unwrap()
            );
            let restored = e
                .replace_form_text(&saved.id, 0, &after.runs[0].object_path, "Cafe older", old)
                .unwrap();
            assert!(
                e.render_page(&restored.id, 0, 1160).unwrap()
                    == e.render_page(&source.id, 0, 1160).unwrap()
            );
            assert_eq!(
                e.source_bytes(&source.id).unwrap().as_ref(),
                bytes.as_slice()
            );
            assert_eq!(fs::read(&path).unwrap(), bytes);
            if let Some(output) = std::env::var_os("FOLIO_INHERITANCE_ARTIFACTS") {
                let output = PathBuf::from(output);
                fs::create_dir_all(&output).unwrap();
                for (suffix, input) in [
                    ("source", &path),
                    ("reference", &reference_path),
                    ("saved", &saved_path),
                ] {
                    fs::copy(
                        input,
                        output.join(format!("{kind}-{state_name}-{suffix}.pdf")),
                    )
                    .unwrap();
                }
            }
            for id in [
                &source.id,
                &changed.id,
                &reference.id,
                &saved.id,
                &restored.id,
            ] {
                e.close_document(id).unwrap();
            }
        }
    }
}

#[test]
fn transformed_inherited_state_matches_local_references() {
    let temp = tempfile::tempdir().unwrap();
    let e = engine();
    for (kind, case, rotation) in [
        ("standard", "form-shear", 90),
        ("simple", "composed", 0),
        ("cid", "mirror", 270),
    ] {
        let old = if kind == "standard" {
            "Cafe old"
        } else {
            "Café old"
        };
        let path = temp.path().join("source.pdf");
        let reference_path = temp.path().join("reference.pdf");
        fixture(&path, kind, "17 TL 9 Ts 65 Tz", case, rotation, false, old);
        fixture(
            &reference_path,
            kind,
            "17 TL 9 Ts 65 Tz",
            case,
            rotation,
            true,
            "Cafe older",
        );
        let source = e.open_document(&path).unwrap();
        let reference = e.open_document(&reference_path).unwrap();
        let listed = e.list_form_text_runs(&source.id, 0).unwrap();
        assert_eq!(listed.runs.len(), 4, "{kind}/{case}: {listed:?}");
        let changed = e
            .replace_form_text(
                &source.id,
                0,
                &listed.runs[0].object_path,
                old,
                "Cafe older",
            )
            .unwrap();
        assert!(
            e.render_page(&changed.id, 0, 1160).unwrap()
                == e.render_page(&reference.id, 0, 1160).unwrap(),
            "{kind}/{case}: edited transform differs from authored reference"
        );
        let saved_path = temp.path().join(format!("{kind}-{case}-saved.pdf"));
        e.export_pdf(
            ExportRequest {
                pages: vec![page(&changed, 0), page(&source, 1)],
                flatten: false,
            },
            &saved_path,
        )
        .unwrap();
        let saved = e.open_document(&saved_path).unwrap();
        assert!(
            e.render_page(&saved.id, 0, 1160).unwrap()
                == e.render_page(&reference.id, 0, 1160).unwrap()
        );
        assert!(
            e.render_page(&saved.id, 1, 1160).unwrap()
                == e.render_page(&source.id, 1, 1160).unwrap()
        );
        if let Some(output) = std::env::var_os("FOLIO_INHERITANCE_ARTIFACTS") {
            let output = PathBuf::from(output);
            fs::create_dir_all(&output).unwrap();
            for (suffix, input) in [
                ("source", &path),
                ("reference", &reference_path),
                ("saved", &saved_path),
            ] {
                fs::copy(input, output.join(format!("{kind}-{case}-{suffix}.pdf"))).unwrap();
            }
        }
        for id in [&source.id, &reference.id, &changed.id, &saved.id] {
            e.close_document(id).unwrap();
        }
    }
}
