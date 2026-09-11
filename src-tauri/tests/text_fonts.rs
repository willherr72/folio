use folio_engine::{ExportRequest, Overlay, PdfEngine, RecoveryStore};
use lopdf::{dictionary, Document, Object, Stream};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

const FONTS: [&str; 12] = [
    "Helvetica",
    "Helvetica-Bold",
    "Helvetica-Oblique",
    "Helvetica-BoldOblique",
    "Times-Roman",
    "Times-Bold",
    "Times-Italic",
    "Times-BoldItalic",
    "Courier",
    "Courier-Bold",
    "Courier-Oblique",
    "Courier-BoldOblique",
];

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

fn fixture(path: &Path) {
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let font =
        pdf.add_object(dictionary! {"Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"Helvetica"});
    let content = pdf.add_object(Stream::new(
        dictionary! {},
        b"BT /F1 14 Tf 20 370 Td (BASE CONTENT) Tj ET".to_vec(),
    ));
    let page = pdf.add_object(dictionary! {"Type"=>"Page", "Parent"=>pages, "MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()], "Resources"=>dictionary! {"Font"=>dictionary! {"F1"=>font}}, "Contents"=>content});
    pdf.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages", "Kids"=>vec![Object::Reference(page)], "Count"=>1}.into(),
    );
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog", "Pages"=>pages});
    pdf.trailer.set("Root", root);
    pdf.save(path).unwrap();
}

fn overlay(font: &str) -> Value {
    json!({"type":"text", "id":"text", "x":30,"y":70,"text":"iiiiiiii café", "fontSize":20,"fontName":font,"color":"#123456","rotation":0})
}
fn page(source: &str, overlay: Value) -> Value {
    json!({"id":"page", "sourceId":source,"pageIndex":0,"width":300,"height":400,"rotation":0,"overlays":[overlay]})
}
fn annotation(pdf: &Document) -> &lopdf::Dictionary {
    let page = pdf.get_dictionary(pdf.get_pages()[&1]).unwrap();
    let array = pdf
        .dereference(page.get(b"Annots").unwrap())
        .unwrap()
        .1
        .as_array()
        .unwrap();
    pdf.dereference(&array[0]).unwrap().1.as_dict().unwrap()
}
fn appearance_font(pdf: &Document) -> String {
    let ap = annotation(pdf).get(b"AP").unwrap().as_dict().unwrap();
    let stream = pdf
        .dereference(ap.get(b"N").unwrap())
        .unwrap()
        .1
        .as_stream()
        .unwrap();
    let fonts = stream
        .dict
        .get(b"Resources")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Font")
        .unwrap()
        .as_dict()
        .unwrap();
    let font = pdf
        .dereference(fonts.get(b"FolioFont").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    String::from_utf8(font.get(b"BaseFont").unwrap().as_name().unwrap().to_vec()).unwrap()
}

#[test]
fn missing_font_defaults_to_helvetica_and_unknown_faces_are_rejected() {
    let mut old = overlay("Helvetica");
    old.as_object_mut().unwrap().remove("fontName");
    let parsed: Overlay = serde_json::from_value(old).unwrap();
    assert_eq!(
        serde_json::to_value(parsed).unwrap()["fontName"],
        "Helvetica"
    );
    for invalid in [
        json!("ComicSans"),
        json!("Helvetica /Bad"),
        json!(null),
        json!(42),
    ] {
        let mut value = overlay("Helvetica");
        value["fontName"] = invalid;
        assert!(serde_json::from_value::<Overlay>(value).is_err());
    }
}

#[test]
fn every_font_survives_editable_export_reopen_recovery_and_flattening() {
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.pdf");
    fixture(&source_path);
    let original = fs::read(&source_path).unwrap();
    let engine = engine();
    let source = engine.open_document(&source_path).unwrap();
    for font in FONTS {
        let output = temp.path().join(format!("{font}.pdf"));
        let request: ExportRequest =
            serde_json::from_value(json!({"pages":[page(&source.id,overlay(font))]})).unwrap();
        engine.export_pdf(request, &output).unwrap();
        let pdf = Document::load(&output).unwrap();
        assert_eq!(appearance_font(&pdf), font);
        let da =
            std::str::from_utf8(annotation(&pdf).get(b"DA").unwrap().as_str().unwrap()).unwrap();
        assert!(
            da.contains("/FolioFont 20 Tf"),
            "default appearance must use the selected appearance font"
        );
        let reopened = engine.open_document(&output).unwrap();
        let imported = serde_json::to_value(&reopened.pages[0].overlays).unwrap();
        assert_eq!(imported[0]["fontName"], font);
        assert!(!engine
            .extract_text(&reopened.id, 0)
            .unwrap()
            .contains("iiii"));
        let plan = page(&reopened.id, imported[0].clone());
        let store = RecoveryStore::new(temp.path().join(format!("recovery-{font}"))).unwrap();
        store.save(&engine, json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":true,"savedDigest":"","zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"font.pdf","pages":[plan],"selectedPageId":"page","selectedOverlayId":"text"}}]})).unwrap();
        let restored = store.load(&engine).unwrap().unwrap();
        let restored_plan = restored["tabs"][0]["document"]["pages"][0].clone();
        assert_eq!(restored_plan["overlays"][0]["fontName"], font);
        let flat_path = temp.path().join(format!("flat-{font}.pdf"));
        engine
            .export_pdf(
                serde_json::from_value(json!({"flatten":true,"pages":[restored_plan]})).unwrap(),
                &flat_path,
            )
            .unwrap();
        let flat_pdf = Document::load(&flat_path).unwrap();
        assert!(
            flat_pdf
                .objects
                .values()
                .filter_map(|o| o.as_dict().ok())
                .any(|d| d.get(b"BaseFont").ok().and_then(|o| o.as_name().ok())
                    == Some(font.as_bytes())),
            "flattened PDF must contain {font}"
        );
        let flat = engine.open_document(&flat_path).unwrap();
        assert!(flat.pages[0].overlays.is_empty());
        assert!(engine
            .extract_text(&flat.id, 0)
            .unwrap()
            .contains("iiiiiiii"));
        assert!(engine
            .extract_text(&source.id, 0)
            .unwrap()
            .contains("BASE CONTENT"));
        assert_eq!(fs::read(&source_path).unwrap(), original);
    }
}

#[test]
fn annotation_extents_follow_font_metrics() {
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.pdf");
    fixture(&source_path);
    let engine = engine();
    let source = engine.open_document(&source_path).unwrap();
    let mut widths = Vec::new();
    for font in ["Helvetica", "Helvetica-Bold", "Courier"] {
        let mut text = overlay(font);
        text["text"] = json!("iiiiiiii");
        let output = temp.path().join(format!("{font}.pdf"));
        engine
            .export_pdf(
                serde_json::from_value(json!({"pages":[page(&source.id,text)]})).unwrap(),
                &output,
            )
            .unwrap();
        let pdf = Document::load(&output).unwrap();
        let rect = annotation(&pdf).get(b"Rect").unwrap().as_array().unwrap();
        widths.push(rect[2].as_float().unwrap() - rect[0].as_float().unwrap());
    }
    assert!(
        widths[1] > widths[0],
        "bold narrow letters must account for their wider advances: {widths:?}"
    );
    assert!(
        widths[2] > widths[1],
        "Courier must use its wider monospace advances: {widths:?}"
    );
}

#[test]
fn legacy_helvetica_annotations_without_font_metadata_remain_editable() {
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.pdf");
    fixture(&source_path);
    let engine = engine();
    let source = engine.open_document(&source_path).unwrap();
    let output = temp.path().join("legacy.pdf");
    let mut text = overlay("Helvetica");
    text["text"] = json!("iiiiiiii");
    engine
        .export_pdf(
            serde_json::from_value(json!({"pages":[page(&source.id,text)]})).unwrap(),
            &output,
        )
        .unwrap();
    let mut pdf = Document::load(&output).unwrap();
    let page_id = pdf.get_pages()[&1];
    let annotation_id = pdf
        .get_dictionary(page_id)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .as_reference()
        .unwrap();
    let annotation = pdf.get_dictionary_mut(annotation_id).unwrap();
    let mut metadata: Value =
        serde_json::from_slice(annotation.get(b"Folio").unwrap().as_str().unwrap()).unwrap();
    metadata["overlay"]
        .as_object_mut()
        .unwrap()
        .remove("fontName");
    annotation.set(
        "Folio",
        Object::string_literal(serde_json::to_vec(&metadata).unwrap()),
    );
    let da = String::from_utf8(annotation.get(b"DA").unwrap().as_str().unwrap().to_vec()).unwrap();
    annotation.set(
        "DA",
        Object::string_literal(da.replace("/FolioFont", "/Helv")),
    );
    // Prior releases used character count * font size * 1.2, with 1 em padding.
    let legacy_bounds = vec![10.0.into(), 286.0.into(), 242.0.into(), 350.0.into()];
    annotation.set("Rect", legacy_bounds.clone());
    let ap_id = annotation
        .get(b"AP")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"N")
        .unwrap()
        .as_reference()
        .unwrap();
    pdf.get_object_mut(ap_id)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .dict
        .set("BBox", legacy_bounds);
    pdf.save(&output).unwrap();
    let reopened = engine.open_document(&output).unwrap();
    let imported = serde_json::to_value(&reopened.pages[0].overlays).unwrap();
    assert_eq!(imported.as_array().unwrap().len(), 1);
    assert_eq!(imported[0]["fontName"], "Helvetica");
    assert_eq!(imported[0]["text"], "iiiiiiii");
}

#[test]
fn tampered_portable_font_stays_native_instead_of_being_imported() {
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.pdf");
    fixture(&source_path);
    let engine = engine();
    let source = engine.open_document(&source_path).unwrap();
    for replacement in ["Unknown-Font", "Courier"] {
        let output = temp.path().join(format!("tampered-{replacement}.pdf"));
        engine
            .export_pdf(
                serde_json::from_value(json!({"pages":[page(&source.id,overlay("Helvetica"))]}))
                    .unwrap(),
                &output,
            )
            .unwrap();
        let mut pdf = Document::load(&output).unwrap();
        let page_id = pdf.get_pages()[&1];
        let annotation_id = pdf
            .get_dictionary(page_id)
            .unwrap()
            .get(b"Annots")
            .unwrap()
            .as_array()
            .unwrap()[0]
            .as_reference()
            .unwrap();
        let annotation = pdf.get_dictionary_mut(annotation_id).unwrap();
        let mut metadata: Value =
            serde_json::from_slice(annotation.get(b"Folio").unwrap().as_str().unwrap()).unwrap();
        metadata["overlay"]["fontName"] = json!(replacement);
        annotation.set(
            "Folio",
            Object::string_literal(serde_json::to_vec(&metadata).unwrap()),
        );
        pdf.save(&output).unwrap();
        let reopened = engine.open_document(&output).unwrap();
        assert!(reopened.pages[0].overlays.is_empty());
    }
}
