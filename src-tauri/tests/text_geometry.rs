use folio_engine::{EngineError, PdfEngine};
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
