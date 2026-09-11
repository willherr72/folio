use folio_engine::{EngineError, ExportRequest, PagePlan, PdfEngine};
use std::path::{Path, PathBuf};

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
fn ambiguous_advances_and_single_character_objects_keep_raw_order() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for (name, content) in [
        ("negative-spacing", "BT /F1 20 Tf -30 Tc 1 0 0 1 210 200 Tm (AB) Tj (CD) Tj (EF) Tj ET"),
        ("singletons", "BT /F1 20 Tf 60 300 Td (A) Tj (B) Tj (C) Tj (D) Tj ET"),
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
