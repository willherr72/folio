use folio_engine::{ExportRequest, PdfEngine, RecoveryStore};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn engine() -> PdfEngine {
    PdfEngine::start(
        std::env::var_os("FOLIO_PDFIUM_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")
            }),
    )
    .unwrap()
}

fn fixture(path: &Path, rotation: u16) {
    let text = "BT /F1 14 Tf 55 325 Td (BASE CONTENT) Tj ET";
    let objects=["<< /Type /Catalog /Pages 2 0 R >>".to_owned(),"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [-20 -30 300 400] /CropBox [40 50 240 350] /Rotate {rotation} /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"),
        format!("<< /Length {} >>\nstream\n{text}\nendstream",text.len()),"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned()];
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(b"xref\n0 6\n0000000000 65535 f \n");
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    fs::write(path, pdf).unwrap();
}

fn overlays(rotation: u16) -> Value {
    json!([
        {"type":"text","id":"portable-text","x":40,"y":70,"text":"Editable café\nSecond line","fontSize":16,"color":"#ab1234","rotation":rotation},
        {"type":"ink","id":"portable-ink","paths":[[{"x":30,"y":140},{"x":50,"y":120},{"x":80,"y":150}],[{"x":90,"y":140},{"x":110,"y":130}]],"strokeWidth":3.5,"color":"#1256ab"}
    ])
}

fn plan(info: &Value, overlays: Value, rotation: u16) -> Value {
    json!({"id":"page","sourceId":info["id"],"pageIndex":0,"width":info["pages"][0]["width"],"height":info["pages"][0]["height"],"rotation":rotation,"overlays":overlays})
}

fn annotations(pdf: &lopdf::Document) -> Vec<(lopdf::ObjectId, &lopdf::Dictionary)> {
    pdf.get_dictionary(pdf.get_pages()[&1])
        .unwrap()
        .get(b"Annots")
        .ok()
        .map(|o| {
            pdf.dereference(o)
                .unwrap()
                .1
                .as_array()
                .unwrap()
                .iter()
                .map(|o| {
                    let id = o.as_reference().unwrap();
                    (id, pdf.get_dictionary(id).unwrap())
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn default_save_is_portable_editable_and_flattening_is_explicit() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.pdf");
    fixture(&source, 0);
    let engine = engine();
    let info = serde_json::to_value(engine.open_document(&source).unwrap()).unwrap();
    let output = temp.path().join("editable.pdf");
    let request: ExportRequest =
        serde_json::from_value(json!({"pages":[plan(&info,overlays(0),0)]})).unwrap();
    engine.export_pdf(request, &output).unwrap();
    let pdf = lopdf::Document::load(&output).unwrap();
    let annotations = annotations(&pdf);
    assert_eq!(
        annotations.len(),
        2,
        "default save requires editable annotations, not painted source content"
    );
    for (_, annotation) in &annotations {
        assert!(annotation.get(b"Folio").is_ok());
        assert!(annotation.get(b"AP").is_ok());
    }
    assert!(annotations
        .iter()
        .any(|(_, a)| a.get(b"Subtype").unwrap().as_name().unwrap() == b"FreeText"));
    assert!(annotations
        .iter()
        .any(|(_, a)| a.get(b"Subtype").unwrap().as_name().unwrap() == b"Ink"));
    fs::remove_file(source).unwrap();
    engine.close_document(info["id"].as_str().unwrap()).unwrap();
    let reopened = serde_json::to_value(engine.open_document(&output).unwrap()).unwrap();
    assert_eq!(
        reopened["pages"][0]["overlays"].as_array().unwrap().len(),
        2
    );
    let text = engine
        .extract_text(reopened["id"].as_str().unwrap(), 0)
        .unwrap();
    assert!(text.contains("BASE CONTENT"));
    assert!(
        !text.contains("Editable"),
        "editable source raster must not contain the imported overlay"
    );
    let flattened = temp.path().join("flattened.pdf");
    engine.export_pdf(serde_json::from_value(json!({"flatten":true,"pages":[plan(&reopened,reopened["pages"][0]["overlays"].clone(),0)]})).unwrap(),&flattened).unwrap();
    let flat = serde_json::to_value(engine.open_document(&flattened).unwrap()).unwrap();
    assert_eq!(flat["pages"][0]["overlays"], json!([]));
    assert!(engine
        .extract_text(flat["id"].as_str().unwrap(), 0)
        .unwrap()
        .contains("Editable"));
    if let Some(root) = std::env::var_os("FOLIO_PERSISTENCE_ARTIFACT_DIR") {
        let root = PathBuf::from(root);
        fs::create_dir_all(&root).unwrap();
        fs::copy(&output, root.join("editable-text-ink.pdf")).unwrap();
        fs::copy(&flattened, root.join("flattened-text-ink.pdf")).unwrap();
    }
}

#[test]
fn text_anchor_rotation_and_ink_points_survive_all_page_rotation_combinations() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for source_rotation in [0, 90, 180, 270] {
        let source = temp.path().join(format!("source-{source_rotation}.pdf"));
        fixture(&source, source_rotation);
        let info = serde_json::to_value(engine.open_document(source).unwrap()).unwrap();
        let w = info["pages"][0]["width"].as_f64().unwrap();
        let h = info["pages"][0]["height"].as_f64().unwrap();
        for page_rotation in [0, 90, 180, 270] {
            for text_rotation in [0, 90, 180, 270] {
                let output = temp.path().join(format!(
                    "saved-{source_rotation}-{page_rotation}-{text_rotation}.pdf"
                ));
                engine
                    .export_pdf(
                        serde_json::from_value(
                            json!({"pages":[plan(&info,overlays(text_rotation),page_rotation)]}),
                        )
                        .unwrap(),
                        &output,
                    )
                    .unwrap();
                if let Some(root) = std::env::var_os("FOLIO_PERSISTENCE_ARTIFACT_DIR") {
                    let root = PathBuf::from(root);
                    fs::copy(&output, root.join(output.file_name().unwrap())).unwrap();
                    let flat = root.join(format!(
                        "flat-{source_rotation}-{page_rotation}-{text_rotation}.pdf"
                    ));
                    engine.export_pdf(serde_json::from_value(json!({"flatten":true,"pages":[plan(&info,overlays(text_rotation),page_rotation)]})).unwrap(),flat).unwrap();
                }
                let reopened = serde_json::to_value(engine.open_document(output).unwrap()).unwrap();
                let imported = reopened["pages"][0]["overlays"].as_array().unwrap();
                assert_eq!(imported.len(), 2);
                let text = imported.iter().find(|o| o["type"] == "text").unwrap();
                let (x, y) = match page_rotation {
                    0 => (40.0, 70.0),
                    90 => (h - 70.0, 40.0),
                    180 => (w - 40.0, h - 70.0),
                    _ => (70.0, w - 40.0),
                };
                assert!((text["x"].as_f64().unwrap() - x).abs() < 0.001);
                assert!((text["y"].as_f64().unwrap() - y).abs() < 0.001);
                assert_eq!(
                    text["rotation"].as_u64(),
                    Some(((text_rotation + page_rotation) % 360) as u64)
                );
                assert_eq!(text["text"], "Editable café\nSecond line");
                let ink = imported.iter().find(|o| o["type"] == "ink").unwrap();
                let (x, y) = match page_rotation {
                    0 => (30.0, 140.0),
                    90 => (h - 140.0, 30.0),
                    180 => (w - 30.0, h - 140.0),
                    _ => (140.0, w - 30.0),
                };
                assert!((ink["paths"][0][0]["x"].as_f64().unwrap() - x).abs() < 0.001);
                assert!((ink["paths"][0][0]["y"].as_f64().unwrap() - y).abs() < 0.001);
            }
        }
    }
}

#[test]
fn malformed_or_stale_metadata_preserves_native_appearance_instead_of_importing() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.pdf");
    fixture(&source, 0);
    let engine = engine();
    let info = serde_json::to_value(engine.open_document(source).unwrap()).unwrap();
    let saved = temp.path().join("saved.pdf");
    engine
        .export_pdf(
            serde_json::from_value(json!({"pages":[plan(&info,overlays(0),0)]})).unwrap(),
            &saved,
        )
        .unwrap();
    for mode in [
        "metadata",
        "version",
        "missing-ap",
        "contents",
        "bounds",
        "appearance",
        "font",
        "optional-content",
    ] {
        let mut pdf = lopdf::Document::load(&saved).unwrap();
        let id = annotations(&pdf)
            .into_iter()
            .find(|(_, a)| a.get(b"Subtype").unwrap().as_name().unwrap() == b"FreeText")
            .unwrap()
            .0;
        if mode == "appearance" || mode == "font" {
            let ap = pdf
                .get_dictionary(id)
                .unwrap()
                .get(b"AP")
                .unwrap()
                .as_dict()
                .unwrap()
                .get(b"N")
                .unwrap()
                .as_reference()
                .unwrap();
            if mode == "appearance" {
                pdf.get_object_mut(ap)
                    .unwrap()
                    .as_stream_mut()
                    .unwrap()
                    .content
                    .extend_from_slice(b"\n0 0 50 50 re f");
            } else {
                let font = pdf
                    .get_object(ap)
                    .unwrap()
                    .as_stream()
                    .unwrap()
                    .dict
                    .get(b"Resources")
                    .unwrap()
                    .as_dict()
                    .unwrap()
                    .get(b"Font")
                    .unwrap()
                    .as_dict()
                    .unwrap()
                    .get(b"FolioFont")
                    .unwrap()
                    .as_reference()
                    .unwrap();
                pdf.get_dictionary_mut(font)
                    .unwrap()
                    .set("BaseFont", "Courier");
            }
        }
        let annotation = pdf.get_dictionary_mut(id).unwrap();
        match mode {
            "metadata" => {
                annotation.set("Folio", lopdf::Object::string_literal("invalid metadata"));
            }
            "version" => {
                let mut record: Value =
                    serde_json::from_slice(annotation.get(b"Folio").unwrap().as_str().unwrap())
                        .unwrap();
                record["version"] = json!(99);
                annotation.set(
                    "Folio",
                    lopdf::Object::String(
                        serde_json::to_vec(&record).unwrap(),
                        lopdf::StringFormat::Hexadecimal,
                    ),
                );
            }
            "appearance" | "font" => {}
            "optional-content" => {
                annotation.set("OC",lopdf::dictionary!{"Type"=>"OCG","Name"=>lopdf::Object::string_literal("changed visibility")});
            }
            "missing-ap" => {
                annotation.remove(b"AP");
            }
            "contents" => {
                annotation.set(
                    "Contents",
                    lopdf::Object::string_literal("Changed in another reader"),
                );
            }
            _ => {
                annotation.set("Rect", vec![0.into(), 0.into(), 50.into(), 50.into()]);
            }
        }
        let path = temp.path().join(format!("changed-{mode}.pdf"));
        pdf.save(&path).unwrap();
        let reopened = serde_json::to_value(engine.open_document(&path).unwrap()).unwrap();
        let imported = reopened["pages"][0]["overlays"].as_array().unwrap();
        assert!(
            !imported.iter().any(|o| o["type"] == "text"),
            "{mode} metadata must not hide changed native appearance"
        );
        let output = temp.path().join(format!("preserved-{mode}.pdf"));
        engine
            .export_pdf(
                serde_json::from_value(json!({"pages":[plan(&reopened,json!([]),0)]})).unwrap(),
                &output,
            )
            .unwrap();
        let pdf = lopdf::Document::load(output).unwrap();
        assert!(annotations(&pdf).iter().any(|(_, a)| a
            .get(b"Subtype")
            .unwrap()
            .as_name()
            .unwrap()
            == b"FreeText"));
    }
}

#[test]
fn legacy_recovery_imports_new_owned_annotations_once_and_honors_later_deletion() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.pdf");
    fixture(&source, 0);
    let engine = engine();
    let info = serde_json::to_value(engine.open_document(source).unwrap()).unwrap();
    let editable = temp.path().join("editable.pdf");
    engine
        .export_pdf(
            serde_json::from_value(json!({"pages":[plan(&info,overlays(0),0)]})).unwrap(),
            &editable,
        )
        .unwrap();
    let info = serde_json::to_value(engine.open_document(&editable).unwrap()).unwrap();
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    let workspace = json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":false,"savedDigest":"","zoom":90,"scrollPosition":{"top":0,"left":0},"document":{"name":"editable.pdf","pages":[plan(&info,json!([]),0)],"selectedPageId":"page","selectedOverlayId":null}}]});
    store.save(&engine, workspace).unwrap();
    let path = fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "json"))
        .unwrap();
    let mut manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["annotationImportVersion"] = json!(1);
    fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    fs::remove_file(editable).unwrap();
    let mut restored = store.load(&engine).unwrap().unwrap();
    assert_eq!(
        restored["tabs"][0]["document"]["pages"][0]["overlays"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    restored["tabs"][0]["document"]["pages"][0]["overlays"] = json!([]);
    store.save(&engine, restored).unwrap();
    assert_eq!(
        store.load(&engine).unwrap().unwrap()["tabs"][0]["document"]["pages"][0]["overlays"],
        json!([])
    );
}

#[test]
fn repeated_edits_duplicate_annotations_and_blank_lines_remain_portable() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.pdf");
    fixture(&source, 270);
    let engine = engine();
    let mut info = serde_json::to_value(engine.open_document(source).unwrap()).unwrap();
    let mut expected = overlays(180);
    for round in 0..4 {
        expected[0]["text"] = json!(format!("\r\n\nEdited {round} café\n\n"));
        expected[0]["x"] = json!(60 + round * 5);
        expected[1]["strokeWidth"] = json!(2.0 + round as f32);
        let path = temp.path().join(format!("round-{round}.pdf"));
        engine
            .export_pdf(
                serde_json::from_value(json!({"pages":[plan(&info,expected.clone(),0)]})).unwrap(),
                &path,
            )
            .unwrap();
        info = serde_json::to_value(engine.open_document(&path).unwrap()).unwrap();
        assert_eq!(
            serde_json::from_value::<Vec<folio_engine::Overlay>>(
                info["pages"][0]["overlays"].clone()
            )
            .unwrap(),
            serde_json::from_value::<Vec<folio_engine::Overlay>>(expected.clone()).unwrap()
        );
    }
    let path = temp.path().join("round-3.pdf");
    let mut pdf = lopdf::Document::load(&path).unwrap();
    let page_id = pdf.get_pages()[&1];
    let text = annotations(&pdf)
        .into_iter()
        .find(|(_, a)| a.get(b"Subtype").unwrap().as_name().unwrap() == b"FreeText")
        .unwrap()
        .1
        .clone();
    let duplicate = pdf.add_object(text);
    pdf.get_dictionary_mut(page_id)
        .unwrap()
        .get_mut(b"Annots")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .push(lopdf::Object::Reference(duplicate));
    let path = temp.path().join("duplicate.pdf");
    pdf.save(&path).unwrap();
    let info = serde_json::to_value(engine.open_document(path).unwrap()).unwrap();
    let overlays = info["pages"][0]["overlays"].as_array().unwrap();
    assert_eq!(overlays.len(), 3);
    assert_ne!(overlays[0]["id"], overlays[2]["id"]);
    let output = temp.path().join("duplicate-saved.pdf");
    engine
        .export_pdf(
            serde_json::from_value(json!({"pages":[plan(&info,json!(overlays),0)]})).unwrap(),
            &output,
        )
        .unwrap();
    assert_eq!(
        engine.open_document(output).unwrap().pages[0]
            .overlays
            .len(),
        3
    );
    let output = temp.path().join("deleted.pdf");
    engine
        .export_pdf(
            serde_json::from_value(json!({"pages":[plan(&info,json!([]),0)]})).unwrap(),
            &output,
        )
        .unwrap();
    assert!(engine.open_document(output).unwrap().pages[0]
        .overlays
        .is_empty());
}

#[test]
fn ink_note_added_by_another_reader_is_preserved_native() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.pdf");
    fixture(&source, 0);
    let engine = engine();
    let info = serde_json::to_value(engine.open_document(source).unwrap()).unwrap();
    let saved = temp.path().join("saved.pdf");
    engine
        .export_pdf(
            serde_json::from_value(json!({"pages":[plan(&info,overlays(0),0)]})).unwrap(),
            &saved,
        )
        .unwrap();
    let mut pdf = lopdf::Document::load(saved).unwrap();
    let id = annotations(&pdf)
        .into_iter()
        .find(|(_, a)| a.get(b"Subtype").unwrap().as_name().unwrap() == b"Ink")
        .unwrap()
        .0;
    pdf.get_dictionary_mut(id).unwrap().set(
        "Contents",
        lopdf::Object::string_literal("Keep the attached ink note"),
    );
    let changed = temp.path().join("changed.pdf");
    pdf.save(&changed).unwrap();
    let info = serde_json::to_value(engine.open_document(changed).unwrap()).unwrap();
    assert!(!info["pages"][0]["overlays"]
        .as_array()
        .unwrap()
        .iter()
        .any(|o| o["type"] == "ink"));
    let saved = temp.path().join("preserved.pdf");
    engine
        .export_pdf(
            serde_json::from_value(json!({"pages":[plan(&info,json!([]),0)]})).unwrap(),
            &saved,
        )
        .unwrap();
    let pdf = lopdf::Document::load(saved).unwrap();
    assert!(annotations(&pdf).iter().any(|(_, a)| a
        .get(b"Contents")
        .ok()
        .and_then(|o| o.as_str().ok())
        == Some(b"Keep the attached ink note".as_slice())));
}
