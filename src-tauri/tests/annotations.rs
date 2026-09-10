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

fn fixture(path: &Path, rotation: u16, annotated: bool) {
    let annotations = if annotated {
        "/Annots [5 0 R 6 0 R 7 0 R]"
    } else {
        ""
    };
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] /CropBox [40 50 240 350] /Rotate {rotation} /Resources << >> /Contents 4 0 R {annotations} >>"),
        "<< /Length 0 >>\nstream\n\nendstream".to_owned(),
        "<< /Type /Annot /Subtype /Highlight /Rect [60 280 140 300] /QuadPoints [60 300 140 300 60 280 140 280] /C [1 1 0] /F 4 /Contents (Highlight note) >>".to_owned(),
        "<< /Type /Annot /Subtype /Text /Rect [70 230 88 248] /C [1 0.5 0] /F 4 /Contents (Review this paragraph) /Name /Comment >>".to_owned(),
        "<< /Type /Annot /Subtype /Square /Rect [160 100 200 140] /C [0 0 1] /F 4 /Contents (Unsupported square remains) >>".to_owned(),
    ];
    let mut bytes = b"%PDF-1.7\n".to_vec();
    let mut offsets = vec![0];
    for (index, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = bytes.len();
    bytes.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets.iter().skip(1) {
        bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    fs::write(path, bytes).unwrap();
}

fn page_plan(info: &Value, overlays: Value, rotation: u16) -> Value {
    json!({"id":"page","sourceId":info["id"],"pageIndex":0,
        "width":info["pages"][0]["width"],"height":info["pages"][0]["height"],"rotation":rotation,"overlays":overlays})
}

#[test]
fn imports_standard_annotations_with_crop_and_every_intrinsic_rotation() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for (rotation, rect, comment) in [
        (0, [20, 50, 80, 20], [30, 102]),
        (90, [230, 20, 20, 80], [180, 30]),
        (180, [100, 230, 80, 20], [152, 180]),
        (270, [50, 100, 20, 80], [102, 152]),
    ] {
        let path = temp.path().join(format!("source-{rotation}.pdf"));
        fixture(&path, rotation, true);
        let info = engine.open_document(&path).unwrap();
        let value = serde_json::to_value(&info).unwrap();
        let overlays = value["pages"][0]["overlays"]
            .as_array()
            .expect("editable imported annotations");
        assert_eq!(overlays.len(), 2);
        let highlight = overlays.iter().find(|o| o["type"] == "highlight").unwrap();
        for (key, expected) in ["x", "y", "width", "height"].into_iter().zip(rect) {
            assert_eq!(highlight["rects"][0][key].as_f64(), Some(expected as f64));
        }
        assert_eq!(highlight["text"], "Highlight note");
        assert_eq!(highlight["color"], "#ffff00");
        let note = overlays.iter().find(|o| o["type"] == "comment").unwrap();
        assert_eq!(
            (note["x"].as_f64().unwrap(), note["y"].as_f64().unwrap()),
            (comment[0] as f64, comment[1] as f64)
        );
        assert_eq!(note["text"], "Review this paragraph");
        let rendered = image::load_from_memory(&engine.render_page(&info.id, 0, 300).unwrap())
            .unwrap()
            .into_rgb8();
        assert!(
            !rendered
                .pixels()
                .any(|p| p[0] > 200 && p[1] > 180 && p[2] < 80),
            "imported highlight must be removed from source raster"
        );
        engine.close_document(&info.id).unwrap();
    }
}

#[test]
fn exports_editable_highlights_comments_and_deletion_without_duplicates() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    fixture(&path, 0, true);
    let engine = engine();
    let info = serde_json::to_value(engine.open_document(&path).unwrap()).unwrap();
    let overlays = json!([
        {"type":"highlight","id":"highlight","rects":[{"x":20,"y":50,"width":80,"height":20},{"x":20,"y":80,"width":45,"height":15}],"color":"#00ff00","text":"Preserved note"},
        {"type":"comment","id":"comment","x":30,"y":102,"text":"Unicode comment: café 漢字","color":"#ff8000"}
    ]);
    let output = temp.path().join("edited.pdf");
    engine
        .export_pdf(
            serde_json::from_value(json!({"pages":[page_plan(&info,overlays,0)]})).unwrap(),
            &output,
        )
        .unwrap();
    let reopened = serde_json::to_value(engine.open_document(&output).unwrap()).unwrap();
    let imported = reopened["pages"][0]["overlays"].as_array().unwrap();
    assert_eq!(imported.len(), 2, "export must replace imported originals");
    let highlight = imported.iter().find(|o| o["type"] == "highlight").unwrap();
    assert_eq!(highlight["rects"].as_array().unwrap().len(), 2);
    assert_eq!(highlight["color"], "#00ff00");
    assert_eq!(highlight["text"], "Preserved note");
    assert!(imported
        .iter()
        .any(|o| o["text"] == "Unicode comment: café 漢字"));
    let deleted = temp.path().join("deleted.pdf");
    engine
        .export_pdf(
            serde_json::from_value(json!({"pages":[page_plan(&reopened,json!([]),0)]})).unwrap(),
            &deleted,
        )
        .unwrap();
    let deleted_info = serde_json::to_value(engine.open_document(&deleted).unwrap()).unwrap();
    assert_eq!(deleted_info["pages"][0]["overlays"], json!([]));
    let bytes = fs::read(&deleted).unwrap();
    let bytes = String::from_utf8_lossy(&bytes);
    assert!(
        bytes.contains("/Square"),
        "unsupported source annotations must survive export"
    );
    if let Some(directory) = std::env::var_os("FOLIO_ANNOTATION_ARTIFACT_DIR") {
        let directory = PathBuf::from(directory);
        fs::create_dir_all(&directory).unwrap();
        fs::copy(&path, directory.join("source-standard-annotations.pdf")).unwrap();
        fs::copy(&output, directory.join("edited-highlight-comment.pdf")).unwrap();
        fs::copy(
            &deleted,
            directory.join("deleted-supported-annotations.pdf"),
        )
        .unwrap();
    }
}

#[test]
fn recovery_keeps_annotation_edits_and_deletions_after_original_disappears() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    fixture(&path, 0, true);
    let engine = engine();
    let info = serde_json::to_value(engine.open_document(&path).unwrap()).unwrap();
    let overlays = json!([{ "type":"comment","id":"comment","x":25,"y":35,"text":"Changed recovered comment","color":"#123456" }]);
    let page = page_plan(&info, overlays, 90);
    let workspace = json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","savedDigest":"","dirty":true,"zoom":90,"scrollPosition":{"top":0,"left":0},"document":{"name":"source.pdf","pages":[page],"selectedPageId":"page","selectedOverlayId":"comment"}}]});
    let store = RecoveryStore::new(temp.path().join("recovery")).unwrap();
    store.save(&engine, workspace).unwrap();
    engine.close_document(info["id"].as_str().unwrap()).unwrap();
    fs::remove_file(path).unwrap();
    let restored = store.load(&engine).unwrap().unwrap();
    let page = &restored["tabs"][0]["document"]["pages"][0];
    assert_eq!(page["overlays"].as_array().unwrap().len(), 1);
    assert_eq!(page["overlays"][0]["text"], "Changed recovered comment");
    let output = temp.path().join("recovered.pdf");
    let request: ExportRequest = serde_json::from_value(json!({"pages":[page]})).unwrap();
    engine.export_pdf(request, output.clone()).unwrap();
    let reopened = serde_json::to_value(engine.open_document(output).unwrap()).unwrap();
    assert_eq!(
        reopened["pages"][0]["overlays"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        reopened["pages"][0]["overlays"][0]["text"],
        "Changed recovered comment"
    );
}

#[test]
fn legacy_recovery_imports_source_annotations_once_before_next_checkpoint() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("legacy.pdf");
    fixture(&path, 0, true);
    let engine = engine();
    let info = serde_json::to_value(engine.open_document(&path).unwrap()).unwrap();
    let workspace = json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","savedDigest":"","dirty":false,"zoom":90,"scrollPosition":{"top":0,"left":0},"document":{"name":"legacy.pdf","pages":[page_plan(&info,json!([]),0)],"selectedPageId":"page","selectedOverlayId":null}}]});
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    store.save(&engine, workspace).unwrap();
    let manifest = fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "json"))
        .unwrap();
    let mut value: Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("annotationImportVersion");
    fs::write(manifest, serde_json::to_vec(&value).unwrap()).unwrap();
    engine.close_document(info["id"].as_str().unwrap()).unwrap();
    fs::remove_file(path).unwrap();
    let mut restored = store.load(&engine).unwrap().unwrap();
    assert_eq!(
        restored["tabs"][0]["document"]["pages"][0]["overlays"]
            .as_array()
            .unwrap()
            .len(),
        2,
        "pre-upgrade source annotations must remain visible and become editable"
    );
    restored["tabs"][0]["document"]["pages"][0]["overlays"] = json!([]);
    store.save(&engine, restored).unwrap();
    let after_delete = store.load(&engine).unwrap().unwrap();
    assert_eq!(
        after_delete["tabs"][0]["document"]["pages"][0]["overlays"],
        json!([]),
        "migration must not resurrect annotations deleted afterward"
    );
}

#[test]
fn exported_highlight_rectangles_follow_combined_intrinsic_and_editor_rotations() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    let expected = [
        [20, 50, 80, 20],
        [230, 20, 20, 80],
        [100, 230, 80, 20],
        [50, 100, 20, 80],
    ];
    for source_rotation in [0, 90, 180, 270] {
        let path = temp.path().join(format!("source-{source_rotation}.pdf"));
        fixture(&path, source_rotation, true);
        let info = serde_json::to_value(engine.open_document(&path).unwrap()).unwrap();
        for editor_rotation in [0, 90, 180, 270] {
            let output = temp
                .path()
                .join(format!("rotated-{source_rotation}-{editor_rotation}.pdf"));
            let request = serde_json::from_value(json!({"pages":[page_plan(&info,info["pages"][0]["overlays"].clone(),editor_rotation)]})).unwrap();
            engine.export_pdf(request, &output).unwrap();
            let reopened = serde_json::to_value(engine.open_document(output).unwrap()).unwrap();
            let highlight = reopened["pages"][0]["overlays"]
                .as_array()
                .unwrap()
                .iter()
                .find(|o| o["type"] == "highlight")
                .unwrap();
            for (key, value) in ["x", "y", "width", "height"]
                .into_iter()
                .zip(expected[((source_rotation + editor_rotation) % 360 / 90) as usize])
            {
                assert_eq!(
                    highlight["rects"][0][key].as_f64(),
                    Some(value as f64),
                    "source {source_rotation}, edit {editor_rotation}, {key}"
                );
            }
        }
    }
}

#[test]
fn malformed_annotation_plans_fail_before_creating_export_files() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    fixture(&path, 0, false);
    let engine = engine();
    let info = serde_json::to_value(engine.open_document(&path).unwrap()).unwrap();
    for overlays in [
        json!([{"type":"highlight","id":"bad","rects":[],"color":"#ffff00"}]),
        json!([{"type":"highlight","id":"bad","rects":[{"x":0,"y":0,"width":-5,"height":20}],"color":"#ffff00"}]),
        json!([{"type":"comment","id":"bad","x":0,"y":0,"text":"Bad\u{0000}comment","color":"#ffff00"}]),
        json!([{"type":"comment","id":"bad","x":0,"y":0,"text":"Bad color","color":"javascript:"}]),
    ] {
        let request =
            serde_json::from_value(json!({"pages":[page_plan(&info,overlays,0)]})).unwrap();
        let output = temp.path().join("must-not-exist.pdf");
        assert!(engine.export_pdf(request, &output).is_err());
        assert!(!output.exists());
    }
}

#[test]
fn exported_annotation_dictionaries_are_indirect_objects_for_other_readers() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    fixture(&path, 0, true);
    let engine = engine();
    let info = serde_json::to_value(engine.open_document(&path).unwrap()).unwrap();
    let output = temp.path().join("annotations.pdf");
    engine
        .export_pdf(
            serde_json::from_value(
                json!({"pages":[page_plan(&info,info["pages"][0]["overlays"].clone(),0)]}),
            )
            .unwrap(),
            &output,
        )
        .unwrap();
    let document = lopdf::Document::load(&output).unwrap();
    let page_id = document.get_pages()[&1];
    let annotations = document
        .get_dictionary(page_id)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(annotations.len(), 3);
    for annotation in annotations {
        let id = annotation
            .as_reference()
            .expect("standard annotation must be an indirect PDF object");
        assert!(document.get_dictionary(id).unwrap().get(b"Subtype").is_ok());
    }
}

#[test]
fn imports_declared_colors_with_appearance_details_and_removes_linked_popups() {
    use lopdf::{dictionary, Object, Stream};
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("appearances.pdf");
    fixture(&path, 0, true);
    let mut pdf = lopdf::Document::load(&path).unwrap();
    for (id, color, paint) in [
        ((5, 0), vec![0.0, 1.0, 0.0], "0 1 0"),
        ((6, 0), vec![1.0, 0.5, 0.0], "1 0.5 0"),
    ] {
        let appearance=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),80.into(),20.into()],"Resources"=>dictionary!{}},format!("0 0 0 rg 0 0 1 1 re f {paint} rg 2 2 70 15 re f").into_bytes()));
        let annotation = pdf.get_dictionary_mut(id).unwrap();
        annotation.set(
            "C",
            Object::Array(color.into_iter().map(Object::Real).collect()),
        );
        annotation.set("AP", dictionary! {"N"=>Object::Reference(appearance)});
    }
    let popup=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Popup","Parent"=>Object::Reference((6,0)),"Rect"=>vec![0.into(),0.into(),100.into(),100.into()]});
    pdf.get_dictionary_mut((6, 0))
        .unwrap()
        .set("Popup", Object::Reference(popup));
    pdf.get_dictionary_mut((3, 0))
        .unwrap()
        .get_mut(b"Annots")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .push(Object::Reference(popup));
    pdf.save(&path).unwrap();
    let engine = engine();
    let info = serde_json::to_value(engine.open_document(&path).unwrap()).unwrap();
    let overlays = info["pages"][0]["overlays"].as_array().unwrap();
    assert_eq!(
        overlays.iter().find(|o| o["type"] == "highlight").unwrap()["color"],
        "#00ff00"
    );
    assert_eq!(
        overlays.iter().find(|o| o["type"] == "comment").unwrap()["color"],
        "#ff8000"
    );
    let output = temp.path().join("deleted.pdf");
    engine
        .export_pdf(
            serde_json::from_value(json!({"pages":[page_plan(&info,json!([]),0)]})).unwrap(),
            &output,
        )
        .unwrap();
    let pdf = lopdf::Document::load(&output).unwrap();
    let annotations = pdf.get_page_annotations(pdf.get_pages()[&1]).unwrap();
    assert_eq!(
        annotations.len(),
        1,
        "a deleted comment must not leave its popup or parent contents behind"
    );
    assert_eq!(
        annotations[0].get(b"Subtype").unwrap().as_name().unwrap(),
        b"Square"
    );
    assert!(!String::from_utf8_lossy(&fs::read(output).unwrap()).contains("Review this paragraph"));
}

#[test]
fn source_and_new_highlight_opacity_survive_export_and_recovery_validation() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for opacity in [None, Some(0.2)] {
        let path = temp.path().join(format!("opacity-{opacity:?}.pdf"));
        fixture(&path, 0, true);
        if let Some(opacity) = opacity {
            let mut pdf = lopdf::Document::load(&path).unwrap();
            pdf.get_dictionary_mut((5, 0))
                .unwrap()
                .set("CA", lopdf::Object::Real(opacity));
            pdf.save(&path).unwrap();
        }
        let info = serde_json::to_value(engine.open_document(&path).unwrap()).unwrap();
        let overlay = info["pages"][0]["overlays"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["type"] == "highlight")
            .unwrap();
        assert!(
            (overlay["opacity"]
                .as_f64()
                .expect("imported source opacity")
                - opacity.unwrap_or(1.0) as f64)
                .abs()
                < 0.001
        );
        let output = temp.path().join(format!("export-{opacity:?}.pdf"));
        engine
            .export_pdf(
                serde_json::from_value(json!({"pages":[page_plan(&info,json!([overlay]),0)]}))
                    .unwrap(),
                &output,
            )
            .unwrap();
        let pdf = lopdf::Document::load(output).unwrap();
        let highlight = pdf
            .get_page_annotations(pdf.get_pages()[&1])
            .unwrap()
            .into_iter()
            .find(|d| d.get(b"Subtype").unwrap().as_name().unwrap() == b"Highlight")
            .unwrap();
        assert!(
            (highlight.get(b"CA").unwrap().as_float().unwrap() - opacity.unwrap_or(1.0)).abs()
                < 0.005
        );
        let mut invalid = overlay.clone();
        invalid["opacity"] = json!(2);
        let workspace = json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":true,"savedDigest":"","zoom":90,"scrollPosition":{"top":0,"left":0},"document":{"name":"source.pdf","pages":[page_plan(&info,json!([invalid]),0)],"selectedPageId":"page","selectedOverlayId":null}}]});
        let store = RecoveryStore::new(temp.path().join(format!("recovery-{opacity:?}"))).unwrap();
        assert!(store.save(&engine, workspace).is_err());
    }
}
