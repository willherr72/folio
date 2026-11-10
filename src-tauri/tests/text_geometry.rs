use folio_engine::{EngineError, ExportRequest, PagePlan, PdfEngine};
use std::path::{Path, PathBuf};
#[path = "support/protected_fixture.rs"]
mod protected_fixture;

fn engine() -> PdfEngine {
    let library = std::env::var_os("FOLIO_PDFIUM_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")
        });
    PdfEngine::start(library).unwrap()
}

fn write_pdf(path: &Path, stream: &str, rotation: u16) {
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] /CropBox [40 50 240 350] /Rotate {rotation} /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"),
        format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
    ];
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let mut offsets = vec![0];
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(b"xref\n0 6\n0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    std::fs::write(path, pdf).unwrap();
}

#[test]
fn reading_order_is_stable_across_page_rotation_and_separate_text_operations() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for rotation in [0, 90, 180, 270] {
        let path = temp.path().join(format!("split-{rotation}.pdf"));
        write_pdf(
            &path,
            "BT /F1 20 Tf 60 300 Td (AB) Tj (CD) Tj (EF) Tj 0 -40 Td (GH) Tj (IJ) Tj ET",
            rotation,
        );
        let original = std::fs::read(&path).unwrap();
        let source = engine.open_document(&path).unwrap();
        let raw_before = engine.extract_text(&source.id, 0).unwrap();
        let before = engine.render_page(&source.id, 0, 600).unwrap();
        let text = engine.page_text(&source.id, 0).unwrap();
        let copied = text
            .characters
            .iter()
            .map(|c| c.text.as_str())
            .collect::<String>();
        assert_eq!(
            copied.replace("\r\n", "\n"),
            "ABCDEF\nGHIJ",
            "rotation {rotation}"
        );
        assert_eq!(text.intrinsic_rotation, rotation);
        let control_path = temp.path().join(format!("joined-{rotation}.pdf"));
        write_pdf(
            &control_path,
            "BT /F1 20 Tf 60 300 Td (ABCDEF) Tj 0 -40 Td (GHIJ) Tj ET",
            rotation,
        );
        let control = engine.open_document(&control_path).unwrap();
        let control_text = engine.page_text(&control.id, 0).unwrap();
        assert_eq!(text.characters.len(), control_text.characters.len());
        for (actual, expected) in text.characters.iter().zip(&control_text.characters) {
            assert_eq!(actual.text, expected.text);
            for (a, b) in [actual.x, actual.y, actual.width, actual.height]
                .into_iter()
                .zip([expected.x, expected.y, expected.width, expected.height])
            {
                assert!(
                    (a - b).abs() < 0.01,
                    "Each copied character must retain its original box"
                );
            }
        }
        engine.close_document(&control.id).unwrap();
        assert_eq!(engine.render_page(&source.id, 0, 600).unwrap(), before);
        assert_eq!(engine.extract_text(&source.id, 0).unwrap(), raw_before);
        let saved = temp.path().join(format!("saved-{rotation}.pdf"));
        engine
            .export_pdf(
                ExportRequest {
                    flatten: false,
                    pages: vec![PagePlan {
                        crop: None,
                        id: "page".into(),
                        source_id: source.id.clone(),
                        page_index: 0,
                        width: source.pages[0].width,
                        height: source.pages[0].height,
                        rotation: 0,
                        overlays: vec![],
                    }],
                },
                &saved,
            )
            .unwrap();
        let reopened = engine.open_document(&saved).unwrap();
        assert_eq!(engine.render_page(&reopened.id, 0, 600).unwrap(), before);
        assert_eq!(engine.extract_text(&reopened.id, 0).unwrap(), raw_before);
        assert_eq!(
            serde_json::to_value(engine.page_text(&reopened.id, 0).unwrap()).unwrap(),
            serde_json::to_value(&text).unwrap()
        );
        engine.close_document(&reopened.id).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original);
        engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn retained_minimal_pdfs_read_correctly_without_mutating_raw_pdfium_order() {
    let engine = engine();
    for name in ["single-180.pdf", "split-180.pdf", "counterrotated-180.pdf"] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/reader-order")
            .join(name);
        let source = engine.open_document(&path).unwrap();
        let raw = engine.extract_text(&source.id, 0).unwrap();
        assert_eq!(
            raw,
            if name == "split-180.pdf" {
                "EF CD AB"
            } else {
                "ABCDEF"
            }
        );
        let started = std::time::Instant::now();
        for _ in 0..20 {
            assert_eq!(
                engine
                    .page_text(&source.id, 0)
                    .unwrap()
                    .characters
                    .iter()
                    .map(|c| c.text.as_str())
                    .collect::<String>(),
                "ABCDEF"
            );
        }
        eprintln!(
            "{name}: 20 uncached reader requests in {:?}",
            started.elapsed()
        );
        assert_eq!(engine.extract_text(&source.id, 0).unwrap(), raw);
        engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn rotated_latin_splits_preserve_reading_search_boxes_and_original_content() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    let asset = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/embedded-text/reportlab-subset.pdf");
    let mut base = lopdf::Document::load(asset).unwrap();
    let page_id = *base.get_pages().values().next().unwrap();
    base.get_dictionary_mut(page_id).unwrap().set(
        "CropBox",
        vec![10.into(), 20.into(), 290.into(), 380.into()],
    );
    base.get_dictionary_mut(page_id).unwrap().set("Rotate", 0);
    let control_path = temp.path().join("control.pdf");
    base.save(&control_path).unwrap();
    let control = engine.open_document(&control_path).unwrap();
    let reference = engine.page_text(&control.id, 0).unwrap();
    let expected: String = reference
        .characters
        .iter()
        .map(|c| c.text.as_str())
        .collect();
    assert!(expected.contains("Caf\u{e9} old"));
    assert!(expected.contains("Neighbor"));
    let artifacts = std::env::var_os("FOLIO_READER_ARTIFACTS").map(PathBuf::from);
    if let Some(dir) = &artifacts {
        std::fs::create_dir_all(dir).unwrap();
    }
    let mut records = Vec::new();
    for rotation in [0, 90, 180, 270] {
        for cut in 1..8 {
            let mut pdf = base.clone();
            pdf.get_dictionary_mut(page_id)
                .unwrap()
                .set("Rotate", rotation);
            let mut content =
                lopdf::content::Content::decode(&pdf.get_page_content(page_id)).unwrap();
            let ordinal = content
                .operations
                .iter()
                .position(|o| o.operator == "Tj")
                .unwrap();
            let bytes = content.operations[ordinal].operands[0]
                .as_str()
                .unwrap()
                .to_vec();
            content.operations.splice(
                ordinal..=ordinal,
                [
                    lopdf::content::Operation::new(
                        "Tj",
                        vec![lopdf::Object::string_literal(bytes[..cut].to_vec())],
                    ),
                    lopdf::content::Operation::new(
                        "Tj",
                        vec![lopdf::Object::string_literal(bytes[cut..].to_vec())],
                    ),
                ],
            );
            let stream = pdf.add_object(lopdf::Stream::new(
                lopdf::dictionary! {},
                content.encode().unwrap(),
            ));
            pdf.get_dictionary_mut(page_id)
                .unwrap()
                .set("Contents", stream);
            let label = format!("r{rotation}-s{cut}");
            let path = temp.path().join(format!("{label}.pdf"));
            pdf.save(&path).unwrap();
            let original = std::fs::read(&path).unwrap();
            let source = engine.open_document(&path).unwrap();
            let raw = engine.extract_text(&source.id, 0).unwrap();
            let before = engine.render_page(&source.id, 0, 800).unwrap();
            let text = engine.page_text(&source.id, 0).unwrap();
            let actual: String = text.characters.iter().map(|c| c.text.as_str()).collect();
            assert_eq!(
                actual.replace("\r\n", "\n"),
                expected.replace("\r\n", "\n"),
                "{label}"
            );
            let search = engine.search_page(&source.id, 0, "Caf\u{e9} old").unwrap();
            assert_eq!(search.matches.len(), 1, "{label} search");
            let actual_glyphs: Vec<_> = text
                .characters
                .iter()
                .filter(|c| !c.text.trim().is_empty())
                .collect();
            let reference_glyphs: Vec<_> = reference
                .characters
                .iter()
                .filter(|c| !c.text.trim().is_empty())
                .collect();
            assert_eq!(actual_glyphs.len(), reference_glyphs.len());
            for (a, b) in actual_glyphs.iter().zip(reference_glyphs) {
                assert_eq!(a.text, b.text);
                let (w, h) = (control.pages[0].width, control.pages[0].height);
                let expected_box = match rotation {
                    0 => [b.x, b.y, b.width, b.height],
                    90 => [h - b.y - b.height, b.x, b.height, b.width],
                    180 => [w - b.x - b.width, h - b.y - b.height, b.width, b.height],
                    270 => [b.y, w - b.x - b.width, b.height, b.width],
                    _ => unreachable!(),
                };
                for (x, y) in [a.x, a.y, a.width, a.height].into_iter().zip(expected_box) {
                    assert!(
                        (x - y).abs() < 0.01,
                        "{label} character {}: {x} != {y}",
                        a.text
                    );
                }
            }
            assert_eq!(engine.render_page(&source.id, 0, 800).unwrap(), before);
            assert_eq!(engine.extract_text(&source.id, 0).unwrap(), raw);
            let saved = temp.path().join(format!("{label}-saved.pdf"));
            engine
                .export_pdf(
                    ExportRequest {
                        flatten: false,
                        pages: vec![PagePlan {
                            crop: None,
                            id: "page".into(),
                            source_id: source.id.clone(),
                            page_index: 0,
                            width: source.pages[0].width,
                            height: source.pages[0].height,
                            rotation: 0,
                            overlays: vec![],
                        }],
                    },
                    &saved,
                )
                .unwrap();
            let reopened = engine.open_document(&saved).unwrap();
            assert_eq!(engine.render_page(&reopened.id, 0, 800).unwrap(), before);
            assert_eq!(engine.extract_text(&reopened.id, 0).unwrap(), raw);
            assert_eq!(
                serde_json::to_value(engine.page_text(&reopened.id, 0).unwrap()).unwrap(),
                serde_json::to_value(&text).unwrap()
            );
            assert_eq!(
                engine.source_bytes(&source.id).unwrap().as_ref(),
                original.as_slice()
            );
            assert_eq!(std::fs::read(&path).unwrap(), original);
            if let Some(dir) = &artifacts {
                std::fs::copy(&path, dir.join(format!("{label}-source.pdf"))).unwrap();
                std::fs::copy(&saved, dir.join(format!("{label}-saved.pdf"))).unwrap();
                records.push(serde_json::json!({"label":label,"rotation":rotation,"cut":cut,"reader":actual,"raw":raw,"geometry":text,"search":search}));
            }
            engine.close_document(&reopened.id).unwrap();
            engine.close_document(&source.id).unwrap();
        }
    }
    if let Some(dir) = &artifacts {
        std::fs::write(
            dir.join("reading.json"),
            serde_json::to_vec_pretty(&records).unwrap(),
        )
        .unwrap();
    }
    engine.close_document(&control.id).unwrap();
}

#[test]
fn ambiguous_advances_and_single_character_sequences_keep_raw_order() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for (name, content) in [
        ("negative-spacing", "BT /F1 20 Tf -30 Tc 1 0 0 1 210 200 Tm (AB) Tj (CD) Tj (EF) Tj ET"),
        ("negative-singletons", "BT /F1 20 Tf -30 Tc 1 0 0 1 210 200 Tm (A) Tj (B) Tj (C) Tj ET"),
        ("stationary-singletons", "BT /F1 20 Tf 60 300 Td (A) Tj 1 0 0 1 60 300 Tm (B) Tj ET"),
        ("reversed-singletons", "BT /F1 20 Tf 1 0 0 1 100 300 Tm (A) Tj 1 0 0 1 60 300 Tm (B) Tj ET"),
        ("isolated-singleton", "BT /F1 20 Tf 60 300 Td (A) Tj 0 -50 Td (BC) Tj (DE) Tj ET"),
        ("empty-stroked", "BT /F1 20 Tf 60 300 Td (AB) Tj (CD) Tj ET BT /F1 20 Tf 1 Tr 60 200 Td ( ) Tj ET"),
        ("empty-clipping", "BT /F1 20 Tf 60 300 Td (AB) Tj (CD) Tj ET BT /F1 20 Tf 7 Tr 60 200 Td ( ) Tj ET"),
        ("empty-with-reversed-singletons", "BT /F1 20 Tf 100 300 Td (A) Tj 1 0 0 1 60 300 Tm (B) Tj ET BT /F1 20 Tf 60 200 Td ( ) Tj ET"),
        ("mixed-orientation", "BT /F1 20 Tf 60 300 Td (AB) Tj (CD) Tj ET BT /F1 20 Tf -1 0 0 -1 200 120 Tm (EF) Tj (GH) Tj ET"),
    ] {
        let path = temp.path().join(format!("{name}.pdf"));
        write_pdf(&path,content,180);
        let source = engine.open_document(&path).unwrap();
        let raw = engine.extract_text(&source.id,0).unwrap();
        let text = engine.page_text(&source.id,0).unwrap();
        assert_eq!(text.characters.iter().map(|c|c.text.as_str()).collect::<String>().replace("\r\n","\n"),raw.replace("\r\n","\n"),"{name}");
        engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn forward_single_character_objects_read_and_search_in_source_order() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for rotation in [0, 90, 180, 270] {
        let path = temp.path().join("singletons.pdf");
        write_pdf(
            &path,
            "BT /F1 20 Tf 60 300 Td (A) Tj (B) Tj (C) Tj (D) Tj 0 -40 Td (E) Tj (F) Tj ET",
            rotation,
        );
        let source = engine.open_document(&path).unwrap();
        let raw = engine.extract_text(&source.id, 0).unwrap();
        let text = engine.page_text(&source.id, 0).unwrap();
        let actual: String = text.characters.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(
            actual.replace("\r\n", "\n"),
            "ABCD\nEF",
            "rotation {rotation}"
        );
        assert_eq!(
            engine
                .search_page(&source.id, 0, "ABCD")
                .unwrap()
                .matches
                .len(),
            1
        );
        assert_eq!(engine.extract_text(&source.id, 0).unwrap(), raw);
        engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn suppressed_space_objects_do_not_reorder_visible_rotated_text() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    // Word emits separate trailing spaces with no reader characters or ink.
    // They must not force display-axis extraction for otherwise forward text.
    let content = "BT /F1 12 Tf 60 320 Td (Header) Tj ( ) Tj ET \
        BT /F1 12 Tf 60 280 Td (AB) Tj (CD) Tj 0 -20 Td (EF) Tj (GH) Tj ET \
        BT /F1 12 Tf 80 260 Td ( ) Tj ET \
        BT /F1 12 Tf 60 80 Td (Footer) Tj ( ) Tj ET \
        BT /F1 12 Tf 60 60 Td ( ) Tj ET";
    for rotation in [0, 90, 180, 270] {
        let path = temp.path().join(format!("spaces-{rotation}.pdf"));
        write_pdf(&path, content, rotation);
        let original = std::fs::read(&path).unwrap();
        let source = engine.open_document(&path).unwrap();
        let before = engine.render_page(&source.id, 0, 600).unwrap();
        let raw = engine.extract_text(&source.id, 0).unwrap();
        let text = engine.page_text(&source.id, 0).unwrap();
        let copied = text.characters.iter().map(|c| c.text.as_str()).collect::<String>();
        assert_eq!(copied.split_whitespace().collect::<Vec<_>>().join(" "), "Header ABCD EFGH Footer", "rotation {rotation}");
        assert_eq!(engine.search_page(&source.id, 0, "ABCD").unwrap().matches.len(), 1);
        assert_eq!(engine.render_page(&source.id, 0, 600).unwrap(), before);
        assert_eq!(engine.extract_text(&source.id, 0).unwrap(), raw);
        assert_eq!(engine.source_bytes(&source.id).unwrap().as_ref(), original.as_slice());
        engine.close_document(&source.id).unwrap();
    }
}

#[test]
#[ignore = "requires explicit locally authored mixed Word session corpus"]
fn original_word_rotated_session_keeps_reference_reading_order() {
    let root = PathBuf::from(std::env::var_os("FOLIO_MIXED_WORD_SESSION").expect("Explicit local corpus required"));
    let engine = engine();
    for name in ["A", "B"] {
        let reference = engine.open_document(&root.join(format!("edit-1/reference-{name}.pdf"))).unwrap();
        let source = engine.open_document(&root.join(format!("restore-1/recovered-{name}.pdf"))).unwrap();
        let expected = engine.page_text(&reference.id, 0).unwrap().characters.into_iter().map(|c| c.text).collect::<String>();
        let actual = engine.page_text(&source.id, 0).unwrap().characters.into_iter().map(|c| c.text).collect::<String>();
        assert_eq!(actual, expected, "original Word {name}");
        engine.close_document(&source.id).unwrap();
        engine.close_document(&reference.id).unwrap();
    }
}

#[test]
fn rotated_latin_search_keeps_copy_permissions_and_restricted_transport() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    for rotation in [90, 180, 270] {
        let asset = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/embedded-text/reportlab-subset.pdf");
        let mut pdf = lopdf::Document::load(asset).unwrap();
        let page = *pdf.get_pages().values().next().unwrap();
        pdf.get_dictionary_mut(page)
            .unwrap()
            .set("Rotate", rotation);
        let mut content = lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap();
        let ordinal = content
            .operations
            .iter()
            .position(|o| o.operator == "Tj")
            .unwrap();
        let text = content.operations[ordinal].operands[0]
            .as_str()
            .unwrap()
            .to_vec();
        content.operations.splice(
            ordinal..=ordinal,
            [
                lopdf::content::Operation::new(
                    "Tj",
                    vec![lopdf::Object::string_literal(text[..1].to_vec())],
                ),
                lopdf::content::Operation::new(
                    "Tj",
                    vec![lopdf::Object::string_literal(text[1..].to_vec())],
                ),
            ],
        );
        let stream = pdf.add_object(lopdf::Stream::new(
            lopdf::dictionary! {},
            content.encode().unwrap(),
        ));
        pdf.get_dictionary_mut(page)
            .unwrap()
            .set("Contents", stream);
        let mut plain = Vec::new();
        pdf.save_to(&mut plain).unwrap();
        let original =
            protected_fixture::encrypt(&plain, lopdf::encryption::Permissions::MODIFIABLE);
        let path = temp.path().join(format!("restricted-{rotation}.pdf"));
        std::fs::write(&path, &original).unwrap();
        let source = engine.open_document_with_password(&path, "reader").unwrap();
        assert!(matches!(
            engine.page_text(&source.id, 0),
            Err(EngineError::PermissionDenied(_))
        ));
        assert!(matches!(
            engine.extract_text(&source.id, 0),
            Err(EngineError::PermissionDenied(_))
        ));
        let result = engine.search_page(&source.id, 0, "Caf\u{e9} old").unwrap();
        assert_eq!(result.matches.len(), 1, "{rotation}");
        assert!(result.matches[0].iter().all(|r| r.width > 0.0
            && r.height > 0.0
            && r.x >= 0.0
            && r.y >= 0.0
            && r.x + r.width <= source.pages[0].width + 0.01
            && r.y + r.height <= source.pages[0].height + 0.01));
        let serialized = serde_json::to_string(&result).unwrap();
        assert!(
            !serialized.contains("Caf")
                && !serialized.contains("Neighbor")
                && !serialized.contains("characters")
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
        engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn non_latin_and_combining_actual_text_keep_the_native_reader_path() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    for (name, actual_text, scalar) in [
        ("greek", "FEFF03910392", '\u{391}'),
        ("combining", "FEFF00410301", '\u{301}'),
    ] {
        let path = temp.path().join(format!("{name}.pdf"));
        write_pdf(&path,&format!("BT /F1 20 Tf 60 300 Td /Span << /ActualText <{actual_text}> >> BDC (AB) Tj EMC (CD) Tj (EF) Tj ET"),180);
        let source = engine.open_document(&path).unwrap();
        let raw = engine.extract_text(&source.id, 0).unwrap();
        assert!(raw.contains(scalar), "{name}: {raw}");
        let text = engine.page_text(&source.id, 0).unwrap();
        assert_eq!(
            text.characters
                .iter()
                .map(|c| c.text.as_str())
                .collect::<String>()
                .replace("\r\n", "\n"),
            raw.replace("\r\n", "\n"),
            "{name}"
        );
        engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn counterrotated_text_retains_its_existing_correct_reading_order() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("counterrotated.pdf");
    write_pdf(
        &path,
        "BT /F1 20 Tf -1 0 0 -1 200 120 Tm (AB) Tj (CD) Tj (EF) Tj ET",
        180,
    );
    let engine = engine();
    let source = engine.open_document(&path).unwrap();
    assert_eq!(engine.extract_text(&source.id, 0).unwrap(), "ABCDEF");
    assert_eq!(
        engine
            .page_text(&source.id, 0)
            .unwrap()
            .characters
            .iter()
            .map(|c| c.text.as_str())
            .collect::<String>(),
        "ABCDEF"
    );
    engine.close_document(&source.id).unwrap();
}

#[test]
fn reading_copy_preserves_inherited_page_rotation_and_crop() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("inherited.pdf");
    write_pdf(&path, "BT /F1 20 Tf 60 300 Td (AB) Tj (CD) Tj ET", 180);
    let mut pdf = lopdf::Document::load(&path).unwrap();
    pdf.get_object_mut((3, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .remove(b"Rotate");
    pdf.get_object_mut((2, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Rotate", 180);
    pdf.save(&path).unwrap();
    let engine = engine();
    let source = engine.open_document(&path).unwrap();
    let before = engine.render_page(&source.id, 0, 600).unwrap();
    let text = engine.page_text(&source.id, 0).unwrap();
    assert_eq!(text.intrinsic_rotation, 180);
    assert_eq!(
        text.characters
            .iter()
            .map(|c| c.text.as_str())
            .collect::<String>(),
        "ABCD"
    );
    assert_eq!(engine.render_page(&source.id, 0, 600).unwrap(), before);
    engine.close_document(&source.id).unwrap();
}

#[test]
fn page_text_combines_pdfium_surrogate_pairs_into_selectable_scalars() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for rotation in [0, 90, 180, 270] {
        let path = temp.path().join(format!("supplementary-{rotation}.pdf"));
        write_pdf(&path,
            "/Span << /ActualText <FEFF0041D835DC340042> >> BDC BT /F1 20 Tf 60 300 Td (ABC) Tj ET EMC",
            rotation);
        let source = engine.open_document(&path).unwrap();
        assert_eq!(engine.extract_text(&source.id, 0).unwrap(), "A\u{1d434}B");
        let text = engine.page_text(&source.id, 0).unwrap();
        let copied: String = text
            .characters
            .iter()
            .map(|character| character.text.as_str())
            .collect();
        assert_eq!(copied, "A\u{1d434}B", "rotation {rotation}");
        assert_eq!(text.characters.len(), 3);
        let scalar = &text.characters[1];
        assert!(scalar.width > 0.0 && scalar.height > 0.0);
        let advance = if rotation % 180 == 0 {
            scalar.width
        } else {
            scalar.height
        };
        let first = &text.characters[0];
        let first_advance = if rotation % 180 == 0 {
            first.width
        } else {
            first.height
        };
        assert!(
            (advance - 2.0 * first_advance).abs() < 0.01,
            "The scalar must retain both UTF-16 entries' bounds."
        );
        engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn page_text_preserves_spaces_and_generated_line_breaks() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("words.pdf");
    write_pdf(
        &path,
        "BT /F1 20 Tf 60 300 Td (A B) Tj 0 -30 Td (C D) Tj ET",
        0,
    );
    let engine = engine();
    let source = engine.open_document(&path).unwrap();
    let text = engine.page_text(&source.id, 0).unwrap();
    let copied: String = text
        .characters
        .iter()
        .map(|character| character.text.as_str())
        .collect();
    assert_eq!(copied.replace("\r\n", "\n"), "A B\nC D");
    assert!(text
        .characters
        .iter()
        .filter(|character| character.text == " ")
        .all(|character| character.width > 0.0));
    assert!(text.characters.iter().all(|character| [
        character.x,
        character.y,
        character.width,
        character.height
    ]
    .iter()
    .all(|value| value.is_finite())));
    assert!(engine.extract_text(&source.id, 0).unwrap().contains("A B"));
    engine.close_document(&source.id).unwrap();
}

#[test]
fn character_bounds_match_rendered_ink_for_every_intrinsic_rotation_and_crop() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for rotation in [0, 90, 180, 270] {
        let path = temp.path().join(format!("rotation-{rotation}.pdf"));
        write_pdf(&path, "BT /F1 20 Tf 60 300 Td (A) Tj ET", rotation);
        let source = engine.open_document(&path).unwrap();
        let text = engine.page_text(&source.id, 0).unwrap();
        assert_eq!(
            serde_json::to_value(&text).unwrap()["intrinsicRotation"],
            rotation,
            "normalized bounds must retain the source orientation for browser carets"
        );
        assert_eq!(text.characters.len(), 1);
        let character = &text.characters[0];
        assert_eq!(character.text, "A");
        let png = engine
            .render_page(&source.id, 0, source.pages[0].width as u32)
            .unwrap();
        let image = image::load_from_memory(&png).unwrap().to_rgb8();
        let mut ink = (image.width(), image.height(), 0, 0);
        let mut count = 0;
        for (x, y, pixel) in image.enumerate_pixels() {
            if pixel.0.iter().all(|component| *component < 100) {
                ink = (
                    ink.0.min(x),
                    ink.1.min(y),
                    ink.2.max(x + 1),
                    ink.3.max(y + 1),
                );
                count += 1;
            }
        }
        assert!(count > 10);
        for (actual, expected) in [
            (character.x, ink.0 as f32),
            (character.y, ink.1 as f32),
            (character.x + character.width, ink.2 as f32),
            (character.y + character.height, ink.3 as f32),
        ] {
            assert!(
                (actual - expected).abs() < 2.0,
                "rotation {rotation}: geometry {actual}, rendered ink {expected}"
            );
        }
        engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn page_text_handles_blank_pages_and_rejects_invalid_or_closed_sources() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("blank.pdf");
    write_pdf(&path, "", 0);
    let engine = engine();
    assert!(matches!(
        engine.page_text("missing", 0),
        Err(EngineError::InvalidRequest(_))
    ));
    let source = engine.open_document(&path).unwrap();
    assert!(engine
        .page_text(&source.id, 0)
        .unwrap()
        .characters
        .is_empty());
    assert!(engine.page_text(&source.id, 1).is_err());
    assert!(matches!(
        engine.page_text(&source.id, usize::MAX),
        Err(EngineError::InvalidRequest(_))
    ));
    engine.close_document(&source.id).unwrap();
    assert!(matches!(
        engine.page_text(&source.id, 0),
        Err(EngineError::InvalidRequest(_))
    ));
}
