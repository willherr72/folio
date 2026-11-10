use folio_engine::{DocumentInfo, ExportRequest, PagePlan, PdfEngine};
use lopdf::{Document, Object};
use std::path::PathBuf;
#[path = "support/embedded_forms.rs"]
mod fixture;

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

// Resource identifiers are arbitrary names, including these three words that
// PDFium treats specially while importing dictionaries.
#[test]
fn reserved_resource_names_survive_export_reorder_and_duplicate() {
    let engine = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    for name in ["First", "Prev", "Parent"] {
        for scope in ["shared", "unique", "nested", "font"] {
            let input = temp.path().join("original.pdf");
            fixture::fixture(
                &input,
                false,
                90,
                false,
                if scope == "unique" {
                    "Café edit"
                } else {
                    "Café old"
                },
            );
            let mut pdf = Document::load(&input).unwrap();
            let rename = |resources: &mut lopdf::Dictionary| {
                if let Ok(xobjects) = resources.get_mut(b"XObject").and_then(Object::as_dict_mut) {
                    if let Some(value) = xobjects.remove(b"First") {
                        xobjects.set(
                            if matches!(scope, "shared" | "unique") {
                                name
                            } else {
                                "Safe"
                            },
                            value,
                        );
                    }
                    if scope == "nested" {
                        if let Some(value) = xobjects.remove(b"Leaf") {
                            xobjects.set(name, value);
                        }
                    }
                }
                if scope == "font" {
                    if let Ok(fonts) = resources.get_mut(b"Font").and_then(Object::as_dict_mut) {
                        if let Some(value) = fonts.remove(b"F1") {
                            fonts.set(name, value);
                        }
                    }
                }
            };
            for object in pdf.objects.values_mut() {
                if let Ok(dict) = object.as_dict_mut() {
                    if let Ok(resources) = dict.get_mut(b"Resources").and_then(Object::as_dict_mut)
                    {
                        rename(resources);
                    }
                }
                if let Ok(stream) = object.as_stream_mut() {
                    if let Ok(resources) = stream
                        .dict
                        .get_mut(b"Resources")
                        .and_then(Object::as_dict_mut)
                    {
                        rename(resources);
                    }
                    // Only these known synthetic content streams contain /Do or /Tf.
                    if stream
                        .content
                        .windows(3)
                        .any(|s| s == b" Do" || s == b" Tf")
                    {
                        let mut text = String::from_utf8(stream.content.clone()).unwrap().replace(
                            "/First",
                            &format!(
                                "/{}",
                                if matches!(scope, "shared" | "unique") {
                                    name
                                } else {
                                    "Safe"
                                }
                            ),
                        );
                        if scope == "nested" {
                            text = text.replace("/Leaf", &format!("/{name}"));
                        }
                        if scope == "font" {
                            text = text.replace("/F1", &format!("/{name}"));
                        }
                        stream.set_plain_content(text.into_bytes());
                    }
                }
            }
            pdf.save(&input).unwrap();
            let original = std::fs::read(&input).unwrap();
            let source = engine.open_document(&input).unwrap();
            let output = temp.path().join("exported.pdf");
            engine
                .export_pdf(
                    ExportRequest {
                        pages: vec![
                            plan(&source, 1, "second"),
                            plan(&source, 0, "first"),
                            plan(&source, 0, "duplicate"),
                        ],
                        flatten: false,
                    },
                    &output,
                )
                .unwrap();
            let reopened = engine.open_document(&output).unwrap();
            for (output_index, source_index) in [(0, 1), (1, 0), (2, 0)] {
                assert!(
                    engine.render_page(&source.id, source_index, 800).unwrap()
                        == engine.render_page(&reopened.id, output_index, 800).unwrap(),
                    "resource /{name}, {scope}, page {output_index}"
                );
                assert_eq!(
                    engine.extract_text(&source.id, source_index).unwrap(),
                    engine.extract_text(&reopened.id, output_index).unwrap()
                );
            }
            assert_eq!(
                fixture::programs(&std::fs::read(&output).unwrap()),
                fixture::programs(&original)
            );
            assert_eq!(std::fs::read(&input).unwrap(), original);
            engine.close_document(&source.id).unwrap();
            engine.close_document(&reopened.id).unwrap();
        }
    }
}

#[test]
fn repaired_resources_compose_with_flattened_text_and_editable_annotations() {
    use folio_engine::{Overlay, TextOverlay};
    let engine = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("original.pdf");
    fixture::fixture(&input, false, 90, false, "Café edit");
    let source = engine.open_document(&input).unwrap();
    let safe = temp.path().join("safe.pdf");
    let mut pdf = Document::load(&input).unwrap();
    for object in pdf.objects.values_mut() {
        if let Ok(d) = object.as_dict_mut() {
            if let Ok(resources) = d.get_mut(b"Resources").and_then(Object::as_dict_mut) {
                if let Ok(x) = resources.get_mut(b"XObject").and_then(Object::as_dict_mut) {
                    if let Some(value) = x.remove(b"First") {
                        x.set("Safe", value);
                    }
                }
            }
        }
        if let Ok(s) = object.as_stream_mut() {
            if s.content.windows(6).any(|v| v == b"/First") {
                s.set_plain_content(
                    String::from_utf8(s.content.clone())
                        .unwrap()
                        .replace("/First", "/Safe")
                        .into_bytes(),
                );
            }
        }
    }
    pdf.save(&safe).unwrap();
    let reference = engine.open_document(&safe).unwrap();
    for flatten in [false, true] {
        let mut reopened = vec![];
        for (source, name) in [(&source, "repaired"), (&reference, "reference")] {
            let mut first = plan(source, 0, "first");
            first.rotation = 90;
            first.overlays.push(Overlay::Text(TextOverlay {
                font_id: None,
                shaping: None,
                area: None,
                font_name: Default::default(),
                rotation: 0,
                id: "overlay".into(),
                x: 250.,
                y: 350.,
                text: "Overlay".into(),
                font_size: 12.,
                color: "#0044CC".into(),
            }));
            let path = temp.path().join(format!("{name}-{flatten}.pdf"));
            engine
                .export_pdf(
                    ExportRequest {
                        pages: vec![first, plan(source, 0, "duplicate")],
                        flatten,
                    },
                    &path,
                )
                .unwrap();
            reopened.push(engine.open_document(&path).unwrap());
        }
        for i in 0..2 {
            assert!(
                engine.render_page(&reopened[0].id, i, 800).unwrap()
                    == engine.render_page(&reopened[1].id, i, 800).unwrap()
            );
            assert_eq!(
                engine.extract_text(&reopened[0].id, i).unwrap(),
                engine.extract_text(&reopened[1].id, i).unwrap()
            );
        }
        if flatten {
            assert!(engine
                .extract_text(&reopened[0].id, 0)
                .unwrap()
                .contains("Overlay"));
            assert!(!engine
                .extract_text(&reopened[0].id, 1)
                .unwrap()
                .contains("Overlay"));
        }
        for document in reopened {
            engine.close_document(&document.id).unwrap();
        }
    }
    engine.close_document(&source.id).unwrap();
    engine.close_document(&reference.id).unwrap();
}
