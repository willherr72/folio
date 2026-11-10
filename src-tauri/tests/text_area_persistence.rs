#![cfg(feature = "shaped-text")]
use folio_engine::{DocumentInfo, ExportRequest, FontAsset, PdfEngine, RecoveryStore};
use lopdf::{dictionary, Document, Object, Stream};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
static LOCK: Mutex<()> = Mutex::new(());
fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn font() -> FontAsset {
    FontAsset::parse(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec())
        .unwrap()
}
fn fixture(path: &Path) {
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let content = pdf.add_object(Stream::new(dictionary! {}, b"q Q".to_vec()));
    let page = pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),640.into(),860.into()],"CropBox"=>vec![20.into(),30.into(),620.into(),830.into()],"Rotate"=>90,"Resources"=>dictionary!{},"Contents"=>content});
    pdf.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}.into(),
    );
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    pdf.trailer.set("Root", root);
    pdf.save(path).unwrap();
}
const SOURCE: &str = "alpha  office café\r\n\r\nbravo\u{a0}charlie  \n";
fn area() -> Value {
    json!({"type":"text","id":"area","x":300,"y":300,"text":SOURCE,"fontSize":18,"fontId":font().info.id,
        "shaping":{"version":1,"direction":"ltr","ligatures":true},"color":"#123456","rotation":0,
        "area":{"version":1,"width":180,"height":240,"inset":5,"lineSpacing":1.25,"alignment":"center"}})
}
fn plan(source: &DocumentInfo, overlays: Value) -> Value {
    json!({"id":"page","sourceId":source.id,"pageIndex":0,"width":source.pages[0].width,"height":source.pages[0].height,"rotation":0,"overlays":overlays})
}
fn request(pages: Value, flatten: bool) -> ExportRequest {
    serde_json::from_value(json!({"pages":pages,"flatten":flatten})).unwrap()
}
fn annotations(pdf: &Document) -> Vec<lopdf::ObjectId> {
    pdf.get_pages()
        .values()
        .flat_map(|id| {
            pdf.get_dictionary(*id)
                .unwrap()
                .get(b"Annots")
                .ok()
                .map(|v| {
                    pdf.dereference(v)
                        .unwrap()
                        .1
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_reference().unwrap())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .collect()
}
#[test]
fn editable_areas_round_trip_source_fonts_and_appearance_across_rotations() {
    let _lock = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    let source_path = temp.path().join("source.pdf");
    fixture(&source_path);
    let source = engine.open_document(&source_path).unwrap();
    let font = font();
    engine.fonts().register(font.bytes.to_vec()).unwrap();
    let mut pages = vec![];
    for page_rotation in [0, 90, 180, 270] {
        for text_rotation in [0, 90, 180, 270] {
            let mut text = area();
            text["rotation"] = json!(text_rotation);
            let mut page = plan(
                &source,
                json!([text,
            {"type":"text","id":"later","x":300,"y":300,"text":"Later","fontSize":16,"color":"#AA0000"},
            {"type":"ink","id":"ink","paths":[[{"x":300,"y":305},{"x":400,"y":305}]],"strokeWidth":2,"color":"#00AA00"}]),
            );
            page["id"] = json!(format!("page-{page_rotation}-{text_rotation}"));
            page["rotation"] = json!(page_rotation);
            pages.push(page);
        }
    }
    let editable = temp.path().join("editable.pdf");
    let before = temp.path().join("before.pdf");
    engine
        .export_pdf(request(json!(pages), false), &editable)
        .unwrap();
    engine
        .export_pdf(request(json!(pages), true), &before)
        .unwrap();
    let pdf = Document::load(&editable).unwrap();
    for id in annotations(&pdf) {
        let metadata: Value = serde_json::from_slice(
            pdf.get_dictionary(id)
                .unwrap()
                .get(b"Folio")
                .unwrap()
                .as_str()
                .unwrap(),
        )
        .unwrap();
        if metadata["overlay"]["id"] == "area" {
            assert_eq!(metadata["version"], 4);
        }
    }
    engine.close_document(&source.id).unwrap();
    fs::remove_file(source_path).unwrap();
    engine.fonts().remove(&font.info.id).unwrap();
    let reopened = engine.open_document(&editable).unwrap();
    assert_eq!(
        engine.fonts().get(&font.info.id).unwrap().bytes.as_ref(),
        font.bytes.as_ref()
    );
    let restored:Vec<_>=reopened.pages.iter().enumerate().map(|(i,p)|{
        let values=serde_json::to_value(&p.overlays).unwrap();assert_eq!(values.as_array().unwrap().len(),3);
        assert_eq!(values[0]["text"],SOURCE);
        assert_eq!(serde_json::from_value::<folio_engine::TextAreaSettings>(values[0]["area"].clone()).unwrap(),serde_json::from_value::<folio_engine::TextAreaSettings>(area()["area"].clone()).unwrap());
        json!({"id":format!("restored-{i}"),"sourceId":reopened.id,"pageIndex":i,"width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})
    }).collect();
    let after = temp.path().join("after.pdf");
    engine
        .export_pdf(request(json!(restored), true), &after)
        .unwrap();
    let before = engine.open_document(&before).unwrap();
    let after = engine.open_document(&after).unwrap();
    for i in 0..16 {
        assert_eq!(
            engine.render_page(&before.id, i, 800).unwrap(),
            engine.render_page(&after.id, i, 800).unwrap(),
            "page {i}"
        );
        let text = engine.extract_text(&after.id, i).unwrap();
        assert!(
            text.contains("alpha")
                && text.contains("bravo")
                && text.contains("charlie")
                && text.contains("Later"),
            "{text:?}"
        );
    }
}
#[test]
fn altered_or_downgraded_area_records_remain_native_annotations() {
    let _lock = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    let path = temp.path().join("source.pdf");
    fixture(&path);
    let source = engine.open_document(path).unwrap();
    engine.fonts().register(font().bytes.to_vec()).unwrap();
    let saved = temp.path().join("editable.pdf");
    engine
        .export_pdf(
            request(json!([plan(&source, json!([area()]))]), false),
            &saved,
        )
        .unwrap();
    for case in ["width", "version", "unknown", "shaping", "font"] {
        let mut pdf = Document::load(&saved).unwrap();
        let id = annotations(&pdf)[0];
        let dict = pdf.get_dictionary_mut(id).unwrap();
        let mut metadata: Value =
            serde_json::from_slice(dict.get(b"Folio").unwrap().as_str().unwrap()).unwrap();
        match case {
            "width" => metadata["overlay"]["area"]["width"] = json!(190),
            "version" => metadata["version"] = json!(3),
            "unknown" => metadata["overlay"]["area"]["version"] = json!(99),
            "shaping" => metadata["overlay"]
                .as_object_mut()
                .unwrap()
                .remove("shaping")
                .map(|_| ())
                .unwrap(),
            _ => metadata["overlay"]
                .as_object_mut()
                .unwrap()
                .remove("fontId")
                .map(|_| ())
                .unwrap(),
        }
        dict.set(
            "Folio",
            Object::string_literal(serde_json::to_vec(&metadata).unwrap()),
        );
        let output = temp.path().join(format!("bad-{case}.pdf"));
        pdf.save(&output).unwrap();
        let reopened = engine.open_document(&output).unwrap();
        assert!(reopened.pages[0].overlays.is_empty(), "{case}");
        let flat = temp.path().join(format!("preserved-{case}.pdf"));
        engine
            .export_pdf(request(json!([plan(&reopened, json!([]))]), false), &flat)
            .unwrap();
        assert_eq!(
            annotations(&Document::load(flat).unwrap()).len(),
            1,
            "{case}"
        );
    }
}
#[test]
fn recovery_preserves_overflowing_drafts_but_rejects_invalid_area_structure() {
    let _lock = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    let path = temp.path().join("source.pdf");
    fixture(&path);
    let source = engine.open_document(&path).unwrap();
    let font = font();
    engine.fonts().register(font.bytes.to_vec()).unwrap();
    let mut draft = area();
    draft["area"]["height"] = json!(20);
    draft["text"] = json!(format!("{SOURCE}unsupported draft 日本"));
    let workspace = json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":true,"savedDigest":"","zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"Area.pdf","pages":[plan(&source,json!([draft]))],"selectedPageId":"page","selectedOverlayId":"area"}}]});
    let store = RecoveryStore::new(temp.path().join("recovery")).unwrap();
    store.save(&engine, workspace.clone()).unwrap();
    let mut bad = workspace.clone();
    bad["tabs"][0]["document"]["pages"][0]["overlays"][0]["area"]["width"] = json!(-1);
    assert!(store.save(&engine, bad).is_err());
    engine.close_document(&source.id).unwrap();
    fs::remove_file(path).unwrap();
    engine.fonts().remove(&font.info.id).unwrap();
    drop(store);
    let store = RecoveryStore::new(temp.path().join("recovery")).unwrap();
    let recovered = store.load(&engine).unwrap().unwrap();
    let page = &recovered["tabs"][0]["document"]["pages"][0];
    assert_eq!(page["overlays"][0], draft);
    assert_eq!(
        engine.fonts().get(&font.info.id).unwrap().bytes.as_ref(),
        font.bytes.as_ref()
    );
    assert!(engine
        .export_pdf(
            request(json!([page]), false),
            temp.path().join("overflow.pdf")
        )
        .is_err());
    assert!(!temp.path().join("overflow.pdf").exists());
}
#[test]
fn export_refuses_areas_without_the_required_font_and_shaping() {
    let _lock = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    let path = temp.path().join("source.pdf");
    fixture(&path);
    let source = engine.open_document(path).unwrap();
    engine.fonts().register(font().bytes.to_vec()).unwrap();
    for case in ["shaping", "fontId"] {
        let mut text = area();
        text["text"] = json!("Hello");
        text.as_object_mut().unwrap().remove(case);
        let output = temp.path().join(format!("{case}.pdf"));
        assert!(
            engine
                .export_pdf(
                    request(json!([plan(&source, json!([text]))]), false),
                    &output
                )
                .is_err(),
            "{case}"
        );
        assert!(!output.exists());
    }
}

#[test]
fn area_appearance_matches_independently_positioned_native_lines() {
    use folio_engine::{layout_text_area, TextAreaRequest};
    let _lock = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    let path = temp.path().join("source.pdf");
    fixture(&path);
    let source = engine.open_document(path).unwrap();
    let font = font();
    engine.fonts().register(font.bytes.to_vec()).unwrap();
    let text = area();
    let mut layout_request = text["area"].clone();
    for (key, value) in [
        ("text", text["text"].clone()),
        ("fontSize", text["fontSize"].clone()),
        ("direction", json!("ltr")),
        ("ligatures", json!(true)),
    ] {
        layout_request[key] = value;
    }
    let layout = layout_text_area(
        &font,
        &serde_json::from_value::<TextAreaRequest>(layout_request).unwrap(),
    )
    .unwrap();
    let lines: Vec<_> = layout
        .lines
        .iter()
        .filter(|line| line.shaped.is_some())
        .enumerate()
        .map(|(i, line)| {
            let mut single = text.clone();
            single.as_object_mut().unwrap().remove("area");
            single["id"] = json!(format!("line-{i}"));
            single["text"] = json!(&SOURCE[line.source.utf8_start..line.source.utf8_end]);
            single["x"] = json!(300. + line.baseline_x);
            single["y"] = json!(300. + line.baseline_y - 18.);
            single
        })
        .collect();
    let actual = temp.path().join("area.pdf");
    let expected = temp.path().join("lines.pdf");
    engine
        .export_pdf(
            request(json!([plan(&source, json!([text]))]), true),
            &actual,
        )
        .unwrap();
    engine
        .export_pdf(
            request(json!([plan(&source, json!(lines))]), true),
            &expected,
        )
        .unwrap();
    let actual = engine.open_document(actual).unwrap();
    let expected = engine.open_document(expected).unwrap();
    let actual = image::load_from_memory(&engine.render_page(&actual.id, 0, 1600).unwrap())
        .unwrap()
        .to_rgb8();
    let expected = image::load_from_memory(&engine.render_page(&expected.id, 0, 1600).unwrap())
        .unwrap()
        .to_rgb8();
    assert_eq!(actual.dimensions(), expected.dimensions());
    let changed = actual
        .pixels()
        .zip(expected.pixels())
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        changed < 100,
        "{changed} pixels differ from independently positioned lines"
    );
}
