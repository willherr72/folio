use folio_engine::{
    create_shaped_pdf, ExportRequest, FontAsset, PagePlan, PdfEngine, ShapedText, TextDirection,
};
use std::{fs, path::PathBuf};
fn font(name: &str) -> FontAsset {
    FontAsset::parse_for_shaping(
        fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../tests/fixtures")
                .join(name),
        )
        .unwrap(),
    )
    .unwrap()
}
fn check_ink(layout: &ShapedText, rotation: u16, png: &[u8]) {
    let image = image::load_from_memory(png).unwrap().to_rgb8();
    let mut actual = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    let mut count = 0;
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel.0.iter().any(|channel| *channel < 245) {
            actual[0] = actual[0].min(x as f32);
            actual[1] = actual[1].min(y as f32);
            actual[2] = actual[2].max(x as f32 + 1.0);
            actual[3] = actual[3].max(y as f32 + 1.0);
            count += 1;
        }
    }
    assert!(
        count > 100,
        "The shaped page must contain actual visible glyph ink."
    );
    let b = layout.bounds.unwrap();
    let origin = (48.0 - b.x_min.min(0.0), 48.0 - b.y_min.min(0.0));
    let width = (b.x_max.max(layout.width) - b.x_min.min(0.0) + 96.0).max(300.0);
    let height = (b.y_max.max(layout.font_size) - b.y_min.min(0.0) + 96.0).max(160.0);
    let display_width = if rotation % 180 == 0 { width } else { height };
    let display_height = if rotation % 180 == 0 { height } else { width };
    let mut expected = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for x in [origin.0 + b.x_min, origin.0 + b.x_max] {
        for y in [origin.1 + b.y_min, origin.1 + b.y_max] {
            let (sx, sy) = match rotation {
                0 => (x, height - y),
                90 => (y, x),
                180 => (width - x, y),
                270 => (height - y, width - x),
                _ => unreachable!(),
            };
            let px = sx * image.width() as f32 / display_width;
            let py = sy * image.height() as f32 / display_height;
            expected[0] = expected[0].min(px);
            expected[1] = expected[1].min(py);
            expected[2] = expected[2].max(px);
            expected[3] = expected[3].max(py);
        }
    }
    for (actual, expected) in actual.iter().zip(expected) {
        assert!(
            (actual - expected).abs() < 3.0,
            "rotation {rotation}, text {:?}: raster bound {actual} != native ink {expected}",
            layout.text
        );
    }
}
#[test]
fn native_glyph_ink_and_logical_text_survive_all_rotations_and_engine_export() {
    let engine = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let cases = [
        ("corpus/fonts/DejaVuSerif.ttf", "office"),
        ("corpus/fonts/DejaVuSerif.ttf", "cafe\u{301}"),
        ("shaped-text/DejaVuSans.ttf", "q\u{307}\u{323}"),
        ("shaped-text/NotoSansArabic-Regular.ttf", "سلام"),
        ("shaped-text/DejaVuSans.ttf", "ABC سلام 123 DEF"),
        ("shaped-text/NotoSansDevanagari-Regular.ttf", "किताब"),
        ("corpus/fonts/DejaVuSerif.ttf", "A \u{1d434} B"),
    ];
    for (index, (fixture, text)) in cases.iter().enumerate() {
        let font = font(fixture);
        for rotation in [0, 90, 180, 270] {
            let output =
                create_shaped_pdf(&font, text, 24.0, TextDirection::Auto, true, rotation).unwrap();
            let path = temp.path().join(format!("shaped-{index}-{rotation}.pdf"));
            fs::write(&path, &output.bytes).unwrap();
            let source = engine.open_document(&path).unwrap();
            let original_png = engine.render_page(&source.id, 0, 1000).unwrap();
            check_ink(&output.layout, rotation, &original_png);
            assert_eq!(
                engine
                    .extract_text(&source.id, 0)
                    .unwrap()
                    .trim_end_matches(['\r', '\n']),
                *text
            );
            if index == 0 && rotation == 0 {
                // Negative gate: exact ActualText still gives incorrect selection bounds.
                let characters = engine.page_text(&source.id, 0).unwrap().characters;
                let left = characters.iter().map(|c| c.x).fold(f32::INFINITY, f32::min);
                let right = characters
                    .iter()
                    .map(|c| c.x + c.width)
                    .fold(f32::NEG_INFINITY, f32::max);
                assert!(
                    right - left < output.layout.width / 2.0,
                    "Reader geometry behavior changed; reassess portability gate."
                );
            }
            let saved = temp.path().join(format!("saved-{index}-{rotation}.pdf"));
            let page = &source.pages[0];
            engine
                .export_pdf(
                    ExportRequest {
                        flatten: false,
                        pages: vec![PagePlan {
                            id: "page".into(),
                            source_id: source.id.clone(),
                            page_index: 0,
                            width: page.width,
                            height: page.height,
                            rotation: 0,
                            overlays: Vec::new(),
                        }],
                    },
                    &saved,
                )
                .unwrap();
            let reopened = engine.open_document(&saved).unwrap();
            assert_eq!(
                engine
                    .extract_text(&reopened.id, 0)
                    .unwrap()
                    .trim_end_matches(['\r', '\n']),
                *text
            );
            assert_eq!(
                engine.render_page(&reopened.id, 0, 1000).unwrap(),
                original_png,
                "Export changed glyphs for {text:?}, rotation {rotation}"
            );
            engine.close_document(&reopened.id).unwrap();
            engine.close_document(&source.id).unwrap();
        }
    }
}
#[test]
fn serializer_rejects_invalid_rotation_and_excessive_page_geometry() {
    let font = font("corpus/fonts/DejaVuSerif.ttf");
    assert!(create_shaped_pdf(&font, "A", 24.0, TextDirection::Auto, true, 45).is_err());
    assert!(create_shaped_pdf(
        &font,
        &"W".repeat(100),
        1000.0,
        TextDirection::Auto,
        true,
        0
    )
    .is_err());
}
