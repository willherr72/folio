use folio_engine::{DocumentInfo, ExportRequest, PagePlan, PdfEngine};
use std::path::{Path, PathBuf};
#[path = "support/affine_forms.rs"]
mod fixture;
static REGISTRY: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn plan(s: &DocumentInfo, i: usize) -> PagePlan {
    PagePlan {
        crop: None,
        id: format!("{}-{i}", s.id),
        source_id: s.id.clone(),
        page_index: i,
        width: s.pages[i].width,
        height: s.pages[i].height,
        rotation: 0,
        overlays: vec![],
    }
}
fn artifacts(prefix: &str, original: &Path, reference: &Path, output: &Path) {
    if let Some(root) = std::env::var_os("FOLIO_FORM_AFFINE_ARTIFACTS") {
        let root = PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        for (path, kind) in [
            (original, "original"),
            (reference, "reference"),
            (output, "exported"),
        ] {
            std::fs::copy(path, root.join(format!("{prefix}-{kind}.pdf"))).unwrap();
        }
    }
}
#[test]
fn affine_shared_occurrences_match_authored_reference_and_survive_reediting() {
    let _lock = REGISTRY.lock().unwrap_or_else(|error| error.into_inner());
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for case in fixture::CASES {
        for rotation in [0, 90, 180, 270] {
            let input = temp.path().join("original.pdf");
            fixture::fixture(&input, "standard", case, rotation, "Cafe old");
            let original = std::fs::read(&input).unwrap();
            let source = e.open_document(&input).unwrap();
            let listed = e.list_form_text_runs(&source.id, 0).unwrap();
            assert_eq!(listed.runs.len(), 2, "{case}/{rotation}: {listed:?}");
            let before = listed.runs;
            for (label, replacement) in [
                ("noop", "Cafe old"),
                ("longer", "New words"),
                ("shorter", "Hi"),
            ] {
                let changed = e
                    .replace_form_text(
                        &source.id,
                        0,
                        &before[0].object_path,
                        "Cafe old",
                        replacement,
                    )
                    .unwrap_or_else(|error| panic!("{case}/{rotation}/{label}: {error}"));
                let reference = temp.path().join("reference.pdf");
                fixture::fixture(&reference, "standard", case, rotation, replacement);
                let expected = e.open_document(&reference).unwrap();
                assert_eq!(
                    e.render_page(&changed.id, 0, 800).unwrap(),
                    e.render_page(&expected.id, 0, 800).unwrap(),
                    "{case}/{rotation}/{label} raster"
                );
                assert_eq!(
                    serde_json::to_value(e.page_text(&changed.id, 0).unwrap()).unwrap(),
                    serde_json::to_value(e.page_text(&expected.id, 0).unwrap()).unwrap(),
                    "{case}/{rotation}/{label} glyphs"
                );
                let after = e.list_form_text_runs(&changed.id, 0).unwrap().runs;
                assert_eq!(after[0].text, replacement);
                assert_eq!(after[1], before[1]);
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
                assert_eq!(
                    e.render_page(&reopened.id, 0, 800).unwrap(),
                    e.render_page(&expected.id, 0, 800).unwrap()
                );
                assert_eq!(
                    e.render_page(&reopened.id, 1, 800).unwrap(),
                    e.render_page(&source.id, 1, 800).unwrap()
                );
                let again = e
                    .replace_form_text(
                        &reopened.id,
                        0,
                        &after[0].object_path,
                        replacement,
                        "Cafe old",
                    )
                    .unwrap();
                assert_eq!(
                    e.render_page(&again.id, 0, 800).unwrap(),
                    e.render_page(&source.id, 0, 800).unwrap()
                );
                artifacts(
                    &format!("{case}-{rotation}-{label}"),
                    &input,
                    &reference,
                    &output,
                );
                for id in [&changed.id, &expected.id, &reopened.id, &again.id] {
                    e.close_document(id).unwrap();
                }
            }
            assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(), original);
            assert_eq!(std::fs::read(&input).unwrap(), original);
            e.close_document(&source.id).unwrap();
        }
    }
}
#[test]
fn embedded_affine_occurrences_reuse_verified_encodings_and_substitute_explicit_fonts() {
    let _lock = REGISTRY.lock().unwrap_or_else(|error| error.into_inner());
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let font = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf"),
    )
    .unwrap();
    for kind in ["simple", "cid"] {
        for case in ["form-rotation", "form-shear", "mirror", "composed"] {
            let input = temp.path().join("original.pdf");
            fixture::fixture(&input, kind, case, 90, "Café old");
            let source = e.open_document(&input).unwrap();
            let listed = e.list_form_text_runs(&source.id, 0).unwrap();
            assert_eq!(listed.runs.len(), 2, "{kind}/{case}: {listed:?}");
            let before = listed.runs;
            let changed = e
                .replace_form_text(
                    &source.id,
                    0,
                    &before[0].object_path,
                    "Café old",
                    "Café edit",
                )
                .unwrap();
            let reference = temp.path().join("reference.pdf");
            fixture::fixture(&reference, kind, case, 90, "Café edit");
            let expected = e.open_document(&reference).unwrap();
            assert_eq!(
                e.render_page(&changed.id, 0, 800).unwrap(),
                e.render_page(&expected.id, 0, 800).unwrap()
            );
            let output = temp.path().join("saved.pdf");
            e.export_pdf(
                ExportRequest {
                    pages: vec![plan(&changed, 0), plan(&source, 1)],
                    flatten: false,
                },
                &output,
            )
            .unwrap();
            artifacts(&format!("{kind}-{case}-90"), &input, &reference, &output);
            let chosen = e.fonts().register(font.clone()).unwrap();
            let substitute = e
                .replace_form_text_with_font(
                    &source.id,
                    0,
                    &before[0].object_path,
                    "Café old",
                    "Cafè edit",
                    Some(&chosen.id),
                )
                .unwrap();
            e.fonts().remove(&chosen.id).unwrap();
            let after = e.list_form_text_runs(&substitute.id, 0).unwrap().runs;
            assert_eq!(after[0].text, "Cafè edit");
            assert_eq!(after[1], before[1]);
            let portable = temp.path().join("portable.pdf");
            e.export_pdf(
                ExportRequest {
                    pages: vec![plan(&substitute, 0), plan(&source, 1)],
                    flatten: false,
                },
                &portable,
            )
            .unwrap();
            let reopened = e.open_document(&portable).unwrap();
            let next = e
                .replace_form_text(
                    &reopened.id,
                    0,
                    &after[0].object_path,
                    "Cafè edit",
                    "Cafè again",
                )
                .unwrap();
            assert_eq!(
                e.list_form_text_runs(&next.id, 0).unwrap().runs[0].text,
                "Cafè again"
            );
            if let Some(root) = std::env::var_os("FOLIO_FORM_AFFINE_ARTIFACTS") {
                let root = PathBuf::from(root);
                std::fs::copy(
                    &portable,
                    root.join(format!("{kind}-{case}-90-substitute.pdf")),
                )
                .unwrap();
            }
            for id in [
                &source.id,
                &changed.id,
                &expected.id,
                &substitute.id,
                &reopened.id,
                &next.id,
            ] {
                e.close_document(id).unwrap();
            }
        }
    }
}
#[test]
fn invalid_and_clipped_affine_forms_remain_unchanged() {
    let _lock = REGISTRY.lock().unwrap_or_else(|error| error.into_inner());
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for case in [
        "singular",
        "near-singular",
        "excessive",
        "composed-excessive",
        "clipped",
    ] {
        let input = temp.path().join("original.pdf");
        fixture::fixture(&input, "standard", "form-rotation", 0, "Cafe old");
        fixture::mutate(&input, case);
        let original = std::fs::read(&input).unwrap();
        let source = e.open_document(&input).unwrap();
        let listed = e.list_form_text_runs(&source.id, 0).unwrap();
        assert!(
            listed.runs.is_empty() && listed.reason.is_some(),
            "{case}: {listed:?}"
        );
        assert!(e
            .replace_form_text(&source.id, 0, &[1, 0, 0], "Cafe old", "New words")
            .is_err());
        assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(), original);
        e.close_document(&source.id).unwrap();
    }
}
#[test]
fn affine_growth_respects_every_ancestor_clip() {
    let _lock = REGISTRY.lock().unwrap_or_else(|error| error.into_inner());
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for case in ["ancestor-edge", "leaf-edge"] {
        let input = temp.path().join("original.pdf");
        fixture::fixture(&input, "standard", "form-rotation", 0, "Cafe old");
        fixture::mutate(&input, case);
        let source = e.open_document(&input).unwrap();
        let original = e.source_bytes(&source.id).unwrap();
        let listed = e.list_form_text_runs(&source.id, 0).unwrap();
        assert_eq!(listed.runs.len(), 2, "{case}: {listed:?}");
        let error = e
            .replace_form_text(
                &source.id,
                0,
                &listed.runs[0].object_path,
                "Cafe old",
                "More replacement words",
            )
            .unwrap_err();
        assert!(
            error.to_string().contains("clipping") || error.to_string().contains("bounding box"),
            "{case}: {error}"
        );
        assert_eq!(e.source_bytes(&source.id).unwrap(), original);
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn vertical_growth_cannot_overwrite_another_shared_occurrence() {
    let _lock = REGISTRY.lock().unwrap_or_else(|error| error.into_inner());
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("collision.pdf");
    fixture::fixture(&input, "standard", "page-quarter", 0, "Cafe old");
    fixture::mutate(&input, "collision");
    let source = e.open_document(&input).unwrap();
    let original = e.source_bytes(&source.id).unwrap();
    let listed = e.list_form_text_runs(&source.id, 0).unwrap();
    assert_eq!(listed.runs.len(), 2, "{listed:?}");
    let error = e
        .replace_form_text(
            &source.id,
            0,
            &listed.runs[0].object_path,
            "Cafe old",
            "More replacement words",
        )
        .unwrap_err();
    assert!(error.to_string().contains("overlap"), "{error}");
    assert_eq!(e.source_bytes(&source.id).unwrap(), original);
    e.close_document(&source.id).unwrap();
}
