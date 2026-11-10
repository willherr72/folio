use folio_engine::{DocumentInfo, ExportRequest, PagePlan, PdfEngine};
use lopdf::{Document, Object};
use std::path::{Path, PathBuf};
#[path = "support/embedded_forms.rs"]
mod fixture;
static REGISTRY: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
fn engine() -> PdfEngine {
    PdfEngine::start(root().join("resources/pdfium/pdfium.dll")).unwrap()
}
fn font() -> Vec<u8> {
    std::fs::read(root().join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()
}
fn plan(d: &DocumentInfo, i: usize, id: &str) -> PagePlan {
    PagePlan {
        crop: None,
        id: id.into(),
        source_id: d.id.clone(),
        page_index: i,
        width: d.pages[i].width,
        height: d.pages[i].height,
        rotation: 0,
        overlays: vec![],
    }
}

#[test]
fn explicit_font_fills_missing_glyph_and_survives_release_export_and_reedit() {
    let _lock = REGISTRY.lock().unwrap();
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let font_bytes = font();
    for cid in [false, true] {
        for rotation in [0, 90, 180, 270] {
            let input = temp.path().join("original.pdf");
            fixture::fixture(&input, cid, rotation, rotation % 180 == 0, "Café old");
            let original = std::fs::read(&input).unwrap();
            let source = e.open_document(&input).unwrap();
            let before = e.list_form_text_runs(&source.id, 0).unwrap().runs;
            assert_eq!(
                serde_json::to_value(&before[0]).unwrap()["canSubstitute"],
                true
            );
            assert!(e
                .replace_form_text(
                    &source.id,
                    0,
                    &before[0].object_path,
                    "Café old",
                    "Cafè edit"
                )
                .is_err());
            let chosen = e.fonts().register(font_bytes.clone()).unwrap();
            let changed = e
                .replace_form_text_with_font(
                    &source.id,
                    0,
                    &before[0].object_path,
                    "Café old",
                    "Cafè edit",
                    Some(&chosen.id),
                )
                .unwrap();
            let after = e.list_form_text_runs(&changed.id, 0).unwrap().runs;
            assert_eq!(after[0].text, "Cafè edit");
            assert_eq!(after[0].font_size, before[0].font_size);
            assert_eq!(after[0].font_display_name.as_deref(), Some("DejaVu Serif"));
            assert_eq!(after[1], before[1]);
            assert!(fixture::programs(&e.source_bytes(&changed.id).unwrap()).contains(&font_bytes));
            for program in fixture::programs(&original) {
                assert!(fixture::programs(&e.source_bytes(&changed.id).unwrap()).contains(&program));
            }
            // Same program can use its verified existing encoding; repeated choices
            // must not accumulate full font programs or alter pixels on same-text edits.
            let same = e
                .replace_form_text_with_font(
                    &changed.id,
                    0,
                    &after[0].object_path,
                    "Cafè edit",
                    "Cafè edit",
                    Some(&chosen.id),
                )
                .unwrap();
            assert!(
                e.render_page(&same.id, 0, 800).unwrap()
                    == e.render_page(&changed.id, 0, 800).unwrap()
            );
            assert!(
                e.source_bytes(&same.id).unwrap().len()
                    < e.source_bytes(&changed.id).unwrap().len() + 4096
            );
            e.fonts().remove(&chosen.id).unwrap();
            let output = temp.path().join("saved.pdf");
            e.export_pdf(
                ExportRequest {
                    pages: vec![
                        plan(&changed, 0, "changed"),
                        plan(&source, 1, "second"),
                        plan(&source, 0, "duplicate"),
                    ],
                    flatten: false,
                },
                &output,
            )
            .unwrap();
            let reopened = e.open_document(&output).unwrap();
            for (i, original_index) in [(1, 1), (2, 0)] {
                assert!(
                    e.render_page(&reopened.id, i, 800).unwrap()
                        == e.render_page(&source.id, original_index, 800).unwrap()
                );
            }
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
            assert_eq!(std::fs::read(&input).unwrap(), original);
            assert_eq!(
                e.source_bytes(&source.id).unwrap().as_ref(),
                original.as_slice()
            );
            if let Some(directory) = std::env::var_os("FOLIO_FORM_FONT_ARTIFACTS") {
                let directory = PathBuf::from(directory);
                std::fs::create_dir_all(&directory).unwrap();
                let prefix = format!("{}-{rotation}", if cid { "cid" } else { "simple" });
                std::fs::copy(&input, directory.join(format!("{prefix}-original.pdf"))).unwrap();
                std::fs::copy(&output, directory.join(format!("{prefix}-edited.pdf"))).unwrap();
            }
            for id in [&source.id, &changed.id, &same.id, &reopened.id, &next.id] {
                e.close_document(id).unwrap();
            }
        }
    }
}

fn with_neighbor(path: &Path, parent: bool) {
    let mut pdf = Document::load(path).unwrap();
    for object in pdf.objects.values_mut() {
        if let Ok(stream) = object.as_stream_mut() {
            if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {
                continue;
            }
            let bytes = stream.get_plain_content().unwrap();
            let has_text = bytes.windows(2).any(|v| v == b"BT");
            if has_text && !parent {
                let mut content = lopdf::content::Content::decode(&bytes).unwrap();
                let font = content
                    .operations
                    .iter()
                    .find(|o| o.operator == "Tf")
                    .unwrap()
                    .clone();
                content.operations.insert(1, font);
                let suffix =
                    lopdf::content::Content::decode(b"BT /F1 16 Tf 1 0 0 1 150 10 Tm (Old) Tj ET")
                        .unwrap();
                content.operations.extend(suffix.operations);
                stream.set_plain_content(content.encode().unwrap());
            } else if !has_text && parent {
                let mut text =
                    b"BT /F1 16 Tf 0.2 0.4 0.6 rg 1 0 0 1 100 30 Tm (Old) Tj ET ".to_vec();
                text.extend(bytes);
                stream.set_plain_content(text);
            }
        }
    }
    // A preexisting alias must not be overwritten by the substitute.
    for object in pdf.objects.values_mut() {
        let d = match object {
            Object::Dictionary(d) => Some(d),
            Object::Stream(s) => Some(&mut s.dict),
            _ => None,
        };
        if let Some(d) = d {
            if let Ok(resources) = d.get_mut(b"Resources").and_then(Object::as_dict_mut) {
                if let Ok(fonts) = resources.get_mut(b"Font").and_then(Object::as_dict_mut) {
                    if let Ok(font) = fonts.get(b"F1") {
                        fonts.set("FolioFormFont", font.clone());
                    }
                }
            }
        }
    }
    pdf.save(path).unwrap();
}

#[test]
fn substitution_preserves_neighbor_font_and_inherited_descendant_scopes() {
    let _lock = REGISTRY.lock().unwrap();
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let chosen = e.fonts().register(font()).unwrap();
    for parent in [false, true] {
        let input = temp.path().join("source.pdf");
        fixture::fixture(&input, false, 0, true, "Café old");
        with_neighbor(&input, parent);
        let source = e.open_document(&input).unwrap();
        let before = e.list_form_text_runs(&source.id, 0).unwrap().runs;
        assert_eq!(before.len(), 4);
        let changed = e
            .replace_form_text_with_font(
                &source.id,
                0,
                &before[0].object_path,
                &before[0].text,
                "Cafè edit",
                Some(&chosen.id),
            )
            .unwrap();
        let after = e.list_form_text_runs(&changed.id, 0).unwrap().runs;
        assert_eq!(after.len(), before.len());
        assert_eq!(&after[1..], &before[1..]);
        assert_eq!(after[0].text, "Cafè edit");
        e.close_document(&source.id).unwrap();
        e.close_document(&changed.id).unwrap();
    }
    e.fonts().remove(&chosen.id).unwrap();
}

#[test]
fn invalid_substitution_leaves_sources_and_font_registry_usable() {
    let _lock = REGISTRY.lock().unwrap();
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let chosen = e.fonts().register(font()).unwrap();
    let input = temp.path().join("source.pdf");
    fixture::fixture(&input, false, 0, false, "Café old");
    let original = std::fs::read(&input).unwrap();
    let source = e.open_document(&input).unwrap();
    let before = e.list_form_text_runs(&source.id, 0).unwrap().runs;
    for (expected, replacement, font_id) in [
        ("Café old", "Cafè edit", "missing"),
        ("Stale", "Cafè edit", chosen.id.as_str()),
        ("Café old", "  ", chosen.id.as_str()),
        ("Café old", "Café\nedit", chosen.id.as_str()),
        (
            "Café old",
            "Old Old Old Old Old Old Old Old Old Old Old Old Old Old Old Old",
            chosen.id.as_str(),
        ),
        ("Café old", "Cafè \u{205e}", chosen.id.as_str()),
    ] {
        assert!(e
            .replace_form_text_with_font(
                &source.id,
                0,
                &before[0].object_path,
                expected,
                replacement,
                Some(font_id)
            )
            .is_err());
        assert_eq!(
            e.source_bytes(&source.id).unwrap().as_ref(),
            original.as_slice()
        );
        assert_eq!(e.list_form_text_runs(&source.id, 0).unwrap().runs, before);
    }
    assert!(e.fonts().get(&chosen.id).is_ok());
    assert_eq!(std::fs::read(&input).unwrap(), original);
    e.close_document(&source.id).unwrap();
    e.fonts().remove(&chosen.id).unwrap();
}

#[test]
fn same_text_can_change_a_standard_form_font_without_changing_shared_copies() {
    let _lock = REGISTRY.lock().unwrap();
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let chosen = e.fonts().register(font()).unwrap();
    let input = temp.path().join("standard.pdf");
    fixture::fixture(&input, false, 0, false, "Café old");
    let mut pdf = Document::load(&input).unwrap();
    let standard = pdf
        .add_object(lopdf::dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    for object in pdf.objects.values_mut() {
        let dict = match object {
            Object::Dictionary(d) => Some(d),
            Object::Stream(s) => Some(&mut s.dict),
            _ => None,
        };
        if let Some(dict) = dict {
            if let Ok(resources) = dict.get_mut(b"Resources").and_then(Object::as_dict_mut) {
                if let Ok(fonts) = resources.get_mut(b"Font").and_then(Object::as_dict_mut) {
                    if fonts.has(b"F1") {
                        fonts.set("F1", standard);
                    }
                }
            }
        }
        if let Object::Stream(stream) = object {
            if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Form") {
                let mut content =
                    lopdf::content::Content::decode(&stream.get_plain_content().unwrap()).unwrap();
                for op in &mut content.operations {
                    if op.operator == "Tj" {
                        op.operands[0] = Object::string_literal("Cafe old");
                    }
                }
                stream.set_plain_content(content.encode().unwrap());
            }
        }
    }
    pdf.save(&input).unwrap();
    let source = e.open_document(&input).unwrap();
    let before = e.list_form_text_runs(&source.id, 0).unwrap().runs;
    assert!(!before[0].is_embedded);
    let changed = e
        .replace_form_text_with_font(
            &source.id,
            0,
            &before[0].object_path,
            "Cafe old",
            "Cafe old",
            Some(&chosen.id),
        )
        .unwrap();
    let after = e.list_form_text_runs(&changed.id, 0).unwrap().runs;
    assert!(after[0].is_embedded);
    assert_eq!(after[0].text, before[0].text);
    assert_eq!(after[1], before[1]);
    assert!(
        e.render_page(&source.id, 0, 800).unwrap() != e.render_page(&changed.id, 0, 800).unwrap()
    );
    e.close_document(&source.id).unwrap();
    e.close_document(&changed.id).unwrap();
    e.fonts().remove(&chosen.id).unwrap();
}
