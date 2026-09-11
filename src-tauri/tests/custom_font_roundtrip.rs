use folio_engine::{ExportRequest, PdfEngine};
use lopdf::{dictionary, Document, Object, Stream};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

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

fn plan(source: &str, font: &str) -> Value {
    json!({"id":"page","sourceId":source,"pageIndex":0,"width":300,"height":400,"rotation":0,"overlays":[{"type":"text","id":"text","x":30,"y":70,"text":"Hello","fontSize":20,"fontId":font,"color":"#123456","rotation":0}]})
}

#[test]
fn unknown_custom_font_is_rejected_instead_of_silently_exporting_helvetica() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.pdf");
    fixture(&source_path);
    let engine = engine();
    let source = engine.open_document(&source_path).unwrap();
    let output = temp.path().join("unknown.pdf");
    let request: ExportRequest =
        serde_json::from_value(json!({"pages":[plan(&source.id,"missing-font")]})).unwrap();
    assert!(engine.export_pdf(request, &output).is_err());
    assert!(!output.exists());
}

fn font_bytes() -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf"),
    )
    .unwrap()
}

fn annotations(pdf: &Document) -> Vec<lopdf::ObjectId> {
    pdf.get_pages()
        .values()
        .flat_map(|id| {
            let page = pdf.get_dictionary(*id).unwrap();
            page.get(b"Annots")
                .ok()
                .map(|value| {
                    pdf.dereference(value)
                        .unwrap()
                        .1
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| value.as_reference().unwrap())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .collect()
}

fn keep_appearances_native(path: &Path) {
    let mut pdf = Document::load(path).unwrap();
    for id in annotations(&pdf) {
        pdf.get_dictionary_mut(id).unwrap().remove(b"Folio");
    }
    pdf.save(path).unwrap();
}

#[test]
fn custom_font_roundtrips_without_original_file_and_embeds_once_per_export() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.pdf");
    fixture(&source_path);
    let original = fs::read(&source_path).unwrap();
    let engine = engine();
    let source = engine.open_document(&source_path).unwrap();
    let font_path = temp.path().join("imported.ttf");
    fs::write(&font_path, font_bytes()).unwrap();
    let font = engine
        .fonts()
        .register(fs::read(&font_path).unwrap())
        .unwrap();
    fs::remove_file(&font_path).unwrap();
    let mut plans = Vec::new();
    for rotation in [0, 90, 180, 270] {
        let mut value = plan(&source.id, &font.id);
        value["id"] = json!(format!("page-{rotation}"));
        value["rotation"] = json!(rotation);
        value["overlays"][0]["text"] = json!("café Ω Ж\nSecond line");
        let mut second = value["overlays"][0].clone();
        second["id"] = json!("second");
        second["y"] = json!(150);
        value["overlays"].as_array_mut().unwrap().push(second);
        plans.push(value);
    }
    let editable = temp.path().join("editable.pdf");
    engine
        .export_pdf(
            serde_json::from_value(json!({"pages":plans})).unwrap(),
            &editable,
        )
        .unwrap();
    let pdf = Document::load(&editable).unwrap();
    assert_eq!(
        pdf.objects
            .values()
            .filter_map(|value| value.as_dict().ok())
            .filter(|dict| dict.has(b"FontFile2"))
            .count(),
        1
    );
    assert_eq!(annotations(&pdf).len(), 8);
    engine.fonts().remove(&font.id).unwrap();
    assert!(engine.fonts().get(&font.id).is_err());
    let reopened = engine.open_document(&editable).unwrap();
    assert_eq!(
        engine.fonts().get(&font.id).unwrap().bytes.as_ref(),
        font_bytes()
    );
    for page in &reopened.pages {
        let imported = serde_json::to_value(&page.overlays).unwrap();
        assert_eq!(imported.as_array().unwrap().len(), 2);
        assert_eq!(imported[0]["fontId"], font.id);
        assert_eq!(imported[0]["text"], "café Ω Ж\nSecond line");
    }
    let restored_plans: Vec<_> = reopened.pages.iter().enumerate().map(|(index,p)| json!({"id":format!("restored-{index}"),"sourceId":reopened.id,"pageIndex":index,"width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})).collect();
    let flattened = temp.path().join("flat.pdf");
    engine
        .export_pdf(
            serde_json::from_value(json!({"flatten":true,"pages":restored_plans})).unwrap(),
            &flattened,
        )
        .unwrap();
    let flat_pdf = Document::load(&flattened).unwrap();
    assert!(annotations(&flat_pdf).is_empty());
    assert_eq!(
        flat_pdf
            .objects
            .values()
            .filter_map(|value| value.as_dict().ok())
            .filter(|dict| dict.has(b"FontFile2"))
            .count(),
        1
    );
    let flat = engine.open_document(&flattened).unwrap();
    for index in 0..4 {
        assert!(flat.pages[index].overlays.is_empty());
        let extracted = engine.extract_text(&flat.id, index).unwrap();
        assert!(extracted.contains("café Ω Ж"), "{extracted:?}");
        assert!(extracted.contains("BASE CONTENT"));
    }
    assert_eq!(fs::read(&source_path).unwrap(), original);
    assert!(!engine.extract_text(&source.id, 0).unwrap().contains("café"));
}

#[test]
fn custom_editable_and_flattened_appearances_match_at_all_text_rotations() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.pdf");
    fixture(&source_path);
    let engine = engine();
    let source = engine.open_document(&source_path).unwrap();
    let font = engine.fonts().register(font_bytes()).unwrap();
    for rotation in [0, 90, 180, 270] {
        let mut value = plan(&source.id, &font.id);
        value["overlays"][0]["rotation"] = json!(rotation);
        value["overlays"][0]["x"] = json!(150);
        value["overlays"][0]["y"] = json!(200);
        value["overlays"][0]["text"] = json!("café Ω Ж");
        let editable = temp.path().join(format!("editable-{rotation}.pdf"));
        let flat = temp.path().join(format!("flat-{rotation}.pdf"));
        engine
            .export_pdf(
                serde_json::from_value(json!({"pages":[value]})).unwrap(),
                &editable,
            )
            .unwrap();
        engine
            .export_pdf(
                serde_json::from_value(json!({"flatten":true,"pages":[value]})).unwrap(),
                &flat,
            )
            .unwrap();
        keep_appearances_native(&editable);
        let editable = engine.open_document(&editable).unwrap();
        let flat = engine.open_document(&flat).unwrap();
        let a = image::load_from_memory(&engine.render_page(&editable.id, 0, 600).unwrap())
            .unwrap()
            .to_rgb8();
        let b = image::load_from_memory(&engine.render_page(&flat.id, 0, 600).unwrap())
            .unwrap()
            .to_rgb8();
        let different = a
            .pixels()
            .zip(b.pixels())
            .filter(|(a, b)| a.0.iter().zip(b.0).any(|(a, b)| a.abs_diff(b) > 8))
            .count();
        assert!(
            different < 100,
            "rotation {rotation}: {different} pixels differ between the same appearance paths"
        );
        assert!(
            a.pixels()
                .filter(|pixel| pixel[0] < 100 && pixel[2] > pixel[0] + 10)
                .count()
                > 100,
            "custom-colored glyphs must actually render"
        );
    }
}

#[test]
fn custom_text_missing_glyphs_and_shaping_are_rejected_before_writing() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.pdf");
    fixture(&source_path);
    let engine = engine();
    let source = engine.open_document(&source_path).unwrap();
    let font = engine.fonts().register(font_bytes()).unwrap();
    for unsupported in ["漢", "مرحبا", "😀"] {
        let mut value = plan(&source.id, &font.id);
        value["overlays"][0]["text"] = json!(unsupported);
        let output = temp.path().join("rejected.pdf");
        fs::write(&output, b"EXISTING DESTINATION").unwrap();
        assert!(engine
            .export_pdf(
                serde_json::from_value(json!({"pages":[value]})).unwrap(),
                &output
            )
            .is_err());
        assert_eq!(fs::read(&output).unwrap(), b"EXISTING DESTINATION");
    }
}

#[test]
fn tampered_font_resources_keep_annotations_native_without_registering_font() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.pdf");
    fixture(&source_path);
    let engine = engine();
    let source = engine.open_document(&source_path).unwrap();
    for mutation in ["widths", "unicode", "glyphs", "fontBytes", "fontId"] {
        let font = engine.fonts().register(font_bytes()).unwrap();
        let output = temp.path().join(format!("{mutation}.pdf"));
        engine
            .export_pdf(
                serde_json::from_value(json!({"pages":[plan(&source.id,&font.id)]})).unwrap(),
                &output,
            )
            .unwrap();
        let mut pdf = Document::load(&output).unwrap();
        if mutation == "fontId" {
            let id = annotations(&pdf)[0];
            let dictionary = pdf.get_dictionary_mut(id).unwrap();
            let mut metadata: Value =
                serde_json::from_slice(dictionary.get(b"Folio").unwrap().as_str().unwrap())
                    .unwrap();
            metadata["overlay"]["fontId"] = json!("0".repeat(64));
            dictionary.set(
                "Folio",
                Object::string_literal(serde_json::to_vec(&metadata).unwrap()),
            );
        } else if mutation == "widths" {
            let dictionary = pdf
                .objects
                .values_mut()
                .filter_map(|value| value.as_dict_mut().ok())
                .find(|dict| dict.has(b"W"))
                .unwrap();
            dictionary.set("W", vec![0.into(), Object::Array(vec![1000.into()])]);
        } else {
            let stream_id = if mutation == "fontBytes" {
                pdf.objects
                    .values()
                    .filter_map(|value| value.as_dict().ok())
                    .find(|dict| dict.has(b"FontFile2"))
                    .unwrap()
                    .get(b"FontFile2")
                    .unwrap()
                    .as_reference()
                    .unwrap()
            } else {
                let key = if mutation == "unicode" {
                    b"ToUnicode".as_slice()
                } else {
                    b"CIDToGIDMap".as_slice()
                };
                pdf.objects
                    .values()
                    .filter_map(|value| value.as_dict().ok())
                    .find(|dict| dict.has(key))
                    .unwrap()
                    .get(key)
                    .unwrap()
                    .as_reference()
                    .unwrap()
            };
            let stream = pdf
                .get_object_mut(stream_id)
                .unwrap()
                .as_stream_mut()
                .unwrap();
            let index = if mutation == "fontBytes" {
                stream.content.len() - 1
            } else {
                0
            };
            stream.content[index] ^= 1;
        }
        pdf.save(&output).unwrap();
        engine.fonts().remove(&font.id).unwrap();
        let opened = engine.open_document(&output).unwrap();
        assert!(
            opened.pages[0].overlays.is_empty(),
            "tampered {mutation} should remain native"
        );
        assert!(
            engine.fonts().get(&font.id).is_err(),
            "tampered {mutation} leaked a font registration"
        );
        let preserved = temp.path().join(format!("preserved-{mutation}.pdf"));
        let value = json!({"id":"page","sourceId":opened.id,"pageIndex":0,"width":300,"height":400,"rotation":0,"overlays":[]});
        engine
            .export_pdf(
                serde_json::from_value(json!({"pages":[value]})).unwrap(),
                &preserved,
            )
            .unwrap();
        assert_eq!(
            annotations(&Document::load(&preserved).unwrap()).len(),
            1,
            "tampered annotation must not disappear"
        );
    }
}

#[test]
fn failed_open_at_font_capacity_leaves_registry_unchanged() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.pdf");
    fixture(&source_path);
    let engine = engine();
    let source = engine.open_document(&source_path).unwrap();
    let font = engine.fonts().register(font_bytes()).unwrap();
    let output = temp.path().join("editable.pdf");
    engine
        .export_pdf(
            serde_json::from_value(json!({"pages":[plan(&source.id,&font.id)]})).unwrap(),
            &output,
        )
        .unwrap();
    engine.fonts().remove(&font.id).unwrap();
    let initial = engine.fonts().ids().unwrap();
    for index in 0..(64 - initial.len()) {
        let mut bytes = font_bytes();
        // Legal SFNT trailing bytes create distinct immutable resource identities.
        bytes.extend_from_slice(format!("capacity-{index}").as_bytes());
        engine.fonts().register(bytes).unwrap();
    }
    let full = engine.fonts().ids().unwrap();
    assert_eq!(full.len(), 64);
    assert!(engine.open_document(&output).is_err());
    assert_eq!(engine.fonts().ids().unwrap(), full);
    assert!(engine.fonts().get(&font.id).is_err());
    for id in full.difference(&initial) {
        engine.fonts().remove(id).unwrap();
    }
    let reopened = engine.open_document(&output).unwrap();
    assert_eq!(reopened.pages[0].overlays.len(), 1);
}

#[cfg(windows)]
#[test]
fn actual_regular_bold_italic_and_bold_italic_programs_survive_reopen() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.pdf");
    fixture(&source_path);
    let engine = engine();
    let source = engine.open_document(&source_path).unwrap();
    let mut overlays = Vec::new();
    let mut fonts = Vec::new();
    let windows = PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:/Windows".into()));
    for (index, (file, bold, italic)) in [
        ("arial.ttf", false, false),
        ("arialbd.ttf", true, false),
        ("ariali.ttf", false, true),
        ("arialbi.ttf", true, true),
    ]
    .into_iter()
    .enumerate()
    {
        let info = engine
            .fonts()
            .register(fs::read(windows.join("Fonts").join(file)).unwrap())
            .unwrap();
        assert_eq!(info.weight >= 700, bold);
        assert_eq!(info.italic, italic);
        let mut overlay = plan(&source.id, &info.id)["overlays"][0].clone();
        overlay["id"] = json!(format!("style-{index}"));
        overlay["y"] = json!(70 + index * 60);
        overlay["text"] = json!("Style café Ω Ж");
        overlays.push(overlay);
        fonts.push(info);
    }
    let mut value = plan(&source.id, &fonts[0].id);
    value["overlays"] = json!(overlays);
    let output = temp.path().join("styles.pdf");
    engine
        .export_pdf(
            serde_json::from_value(json!({"pages":[value]})).unwrap(),
            &output,
        )
        .unwrap();
    let pdf = Document::load(&output).unwrap();
    assert_eq!(
        pdf.objects
            .values()
            .filter_map(|value| value.as_dict().ok())
            .filter(|dict| dict.has(b"FontFile2"))
            .count(),
        4
    );
    for font in &fonts {
        engine.fonts().remove(&font.id).unwrap();
    }
    let reopened = engine.open_document(&output).unwrap();
    assert_eq!(reopened.pages[0].overlays.len(), 4);
    for font in fonts {
        assert_eq!(engine.fonts().get(&font.id).unwrap().info, font);
    }
}

#[test]
fn mixed_standard_custom_and_ink_keep_their_overlay_order_when_flattened() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.pdf");
    fixture(&source_path);
    let engine = engine();
    let source = engine.open_document(&source_path).unwrap();
    let font = engine.fonts().register(font_bytes()).unwrap();
    let mut value = plan(&source.id, &font.id);
    value["overlays"][0]["text"] = json!("MMMM");
    value["overlays"][0]["fontSize"] = json!(40);
    value["overlays"][0]["color"] = json!("#ff0000");
    let mut standard = value["overlays"][0].clone();
    standard.as_object_mut().unwrap().remove("fontId");
    standard["id"] = json!("standard");
    standard["color"] = json!("#0000ff");
    value["overlays"].as_array_mut().unwrap().push(standard);
    value["overlays"].as_array_mut().unwrap().push(json!({"type":"ink","id":"ink","color":"#00ff00","strokeWidth":8,"paths":[[{"x":30,"y":90},{"x":150,"y":90}]]}));
    let editable = temp.path().join("mixed-editable.pdf");
    let flat = temp.path().join("mixed-flat.pdf");
    engine
        .export_pdf(
            serde_json::from_value(json!({"pages":[value]})).unwrap(),
            &editable,
        )
        .unwrap();
    engine
        .export_pdf(
            serde_json::from_value(json!({"flatten":true,"pages":[value]})).unwrap(),
            &flat,
        )
        .unwrap();
    keep_appearances_native(&editable);
    let editable = engine.open_document(&editable).unwrap();
    let flat = engine.open_document(&flat).unwrap();
    let a = image::load_from_memory(&engine.render_page(&editable.id, 0, 600).unwrap())
        .unwrap()
        .to_rgb8();
    let b = image::load_from_memory(&engine.render_page(&flat.id, 0, 600).unwrap())
        .unwrap()
        .to_rgb8();
    let different = a
        .pixels()
        .zip(b.pixels())
        .filter(|(a, b)| a.0.iter().zip(b.0).any(|(a, b)| a.abs_diff(b) > 8))
        .count();
    assert!(
        different < 100,
        "mixed overlay order changed: {different} different pixels"
    );
}

#[test]
fn unused_shared_resource_graph_is_rejected_without_expanding_its_streams() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.pdf");
    fixture(&source_path);
    let engine = engine();
    let source = engine.open_document(&source_path).unwrap();
    let font = engine.fonts().register(font_bytes()).unwrap();
    let output = temp.path().join("shared-resource-graph.pdf");
    engine
        .export_pdf(
            serde_json::from_value(json!({"pages":[plan(&source.id,&font.id)]})).unwrap(),
            &output,
        )
        .unwrap();
    let mut pdf = Document::load(&output).unwrap();
    let mut stream = Stream::new(dictionary! {}, vec![0; 16 * 1024 * 1024]);
    stream.compress().unwrap();
    let mut shared = pdf.add_object(stream);
    // Expanding this compact DAG as a tree would materialize 4 GiB. It is an
    // unexpected resource key and must be rejected before any graph traversal.
    for _ in 0..8 {
        shared = pdf.add_object(dictionary! {"Left"=>shared,"Right"=>shared});
    }
    let annotation = pdf.get_dictionary(annotations(&pdf)[0]).unwrap();
    let ap = annotation
        .get(b"AP")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"N")
        .unwrap()
        .as_reference()
        .unwrap();
    pdf.get_object_mut(ap)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .dict
        .get_mut(b"Resources")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Unused", shared);
    pdf.save(&output).unwrap();
    engine.fonts().remove(&font.id).unwrap();
    let reopened = engine.open_document(&output).unwrap();
    assert!(reopened.pages[0].overlays.is_empty());
    assert!(engine.fonts().get(&font.id).is_err());
    let preserved = temp.path().join("preserved.pdf");
    let value = json!({"id":"page","sourceId":reopened.id,"pageIndex":0,"width":300,"height":400,"rotation":0,"overlays":[]});
    engine
        .export_pdf(
            serde_json::from_value(json!({"pages":[value]})).unwrap(),
            &preserved,
        )
        .unwrap();
    assert_eq!(annotations(&Document::load(&preserved).unwrap()).len(), 1);
}
