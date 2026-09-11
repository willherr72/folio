use folio_engine::{
    create_semantic_pdf, ExportRequest, FontAsset, PagePlan, PdfEngine, TextDirection,
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
fn contains_program(bytes: &[u8], expected: &[u8]) -> bool {
    let doc = lopdf::Document::load_mem(bytes).unwrap();
    doc.objects
        .values()
        .filter_map(|object| object.as_stream().ok())
        .any(|stream| {
            stream
                .decompressed_content()
                .unwrap_or_else(|_| stream.content.clone())
                == expected
        })
}

#[test]
fn semantic_unicode_and_visible_outlines_survive_quarter_turns_and_native_export() {
    let engine = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    for (index, (name, text)) in [
        ("corpus/fonts/DejaVuSerif.ttf", "office"),
        ("shaped-text/DejaVuSans.ttf", "q\u{307}\u{323}"),
        ("shaped-text/NotoSansArabic-Regular.ttf", "سلام"),
        ("shaped-text/NotoSansDevanagari-Regular.ttf", "किताब"),
        ("corpus/fonts/DejaVuSerif.ttf", "A𝑁B"),
    ]
    .into_iter()
    .enumerate()
    {
        let font = font(name);
        for rotation in [0, 90, 180, 270] {
            let output =
                create_semantic_pdf(&font, text, 24., TextDirection::Auto, true, rotation).unwrap();
            assert!(contains_program(&output.bytes, &font.bytes));
            let path = temp.path().join(format!("{index}-{rotation}.pdf"));
            fs::write(&path, &output.bytes).unwrap();
            let source = engine.open_document(&path).unwrap();
            let png = engine.render_page(&source.id, 0, 1000).unwrap();
            let image = image::load_from_memory(&png).unwrap().to_rgb8();
            assert!(
                image
                    .pixels()
                    .filter(|pixel| pixel.0.iter().any(|channel| *channel < 245))
                    .count()
                    > 100
            );
            assert_eq!(
                engine
                    .extract_text(&source.id, 0)
                    .unwrap()
                    .trim_end_matches(['\r', '\n']),
                text,
                "{name}, {rotation}"
            );
            let page_text = engine.page_text(&source.id, 0).unwrap();
            assert_eq!(
                page_text
                    .characters
                    .iter()
                    .map(|character| character.text.as_str())
                    .collect::<String>(),
                text,
                "page_text scalar fidelity: {name}, {rotation}"
            );
            if rotation == 0 {
                let chars = page_text.characters;
                let left = chars.iter().map(|c| c.x).fold(f32::INFINITY, f32::min);
                let right = chars
                    .iter()
                    .map(|c| c.x + c.width)
                    .fold(f32::NEG_INFINITY, f32::max);
                assert!(
                    right - left > output.layout.width * 0.8,
                    "Semantic geometry must span the line, not the first visible glyph"
                );
                if index == 1 {
                    assert_eq!(chars.len(), 3);
                    for character in &chars[1..] {
                        for (actual, expected) in
                            [character.x, character.y, character.width, character.height]
                                .into_iter()
                                .zip([chars[0].x, chars[0].y, chars[0].width, chars[0].height])
                        {
                            assert!(
                                (actual - expected).abs() < 0.01,
                                "Each mark scalar must select the full cluster box"
                            );
                        }
                    }
                }
            }
            let saved = temp.path().join(format!("{index}-{rotation}-saved.pdf"));
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
                            overlays: vec![],
                        }],
                    },
                    &saved,
                )
                .unwrap();
            let bytes = fs::read(&saved).unwrap();
            assert!(contains_program(&bytes, &font.bytes));
            let reopened = engine.open_document(&saved).unwrap();
            assert_eq!(
                engine
                    .extract_text(&reopened.id, 0)
                    .unwrap()
                    .trim_end_matches(['\r', '\n']),
                text
            );
            assert_eq!(engine.render_page(&reopened.id, 0, 1000).unwrap(), png);
            engine.close_document(&reopened.id).unwrap();
            engine.close_document(&source.id).unwrap();
        }
    }
}

#[test]
fn semantic_single_byte_codes_and_geometry_limits_are_explicit() {
    let font = font("corpus/fonts/DejaVuSerif.ttf");
    let output =
        create_semantic_pdf(&font, &"i".repeat(255), 12., TextDirection::Ltr, false, 0).unwrap();
    let pdf = lopdf::Document::load_mem(&output.bytes).unwrap();
    let type3: Vec<_> = pdf
        .objects
        .values()
        .filter_map(|object| object.as_dict().ok())
        .filter(|dict| {
            dict.get(b"Subtype")
                .ok()
                .and_then(|value| value.as_name().ok())
                == Some(b"Type3".as_slice())
        })
        .collect();
    assert_eq!(type3.len(), 1);
    assert!(type3
        .iter()
        .all(|font| font.get(b"LastChar").unwrap().as_i64().unwrap() <= 255));
    assert!(
        create_semantic_pdf(&font, &"i".repeat(256), 12., TextDirection::Ltr, false, 0).is_err()
    );
    assert!(create_semantic_pdf(&font, "A", 24., TextDirection::Ltr, true, 45).is_err());
    assert!(
        create_semantic_pdf(&font, &"W".repeat(100), 1000., TextDirection::Ltr, true, 0).is_err()
    );
}

#[test]
fn mixed_direction_and_cyclic_composite_outlines_are_rejected() {
    let mixed = font("shaped-text/DejaVuSans.ttf");
    let error = create_semantic_pdf(&mixed, "ABC سلام 123", 24., TextDirection::Auto, true, 0)
        .err()
        .unwrap();
    assert!(error.to_string().contains("Mixed directional"));
    let mut bytes = mixed.bytes.to_vec();
    let (start, id) = {
        let face = ttf_parser::Face::parse(&bytes, 0).unwrap();
        let id = face.glyph_index('A').unwrap();
        let raw = face.raw_face();
        let glyf = raw.table(ttf_parser::Tag::from_bytes(b"glyf")).unwrap();
        let loca = ttf_parser::loca::Table::parse(
            face.tables().maxp.number_of_glyphs,
            face.tables().head.index_to_location_format,
            raw.table(ttf_parser::Tag::from_bytes(b"loca")).unwrap(),
        )
        .unwrap();
        let range = loca.glyph_range(id).unwrap();
        assert!(range.len() >= 18);
        (
            glyf.as_ptr() as usize - bytes.as_ptr() as usize + range.start,
            id.0,
        )
    };
    bytes[start..start + 2].copy_from_slice(&(-1i16).to_be_bytes());
    bytes[start + 10..start + 12].copy_from_slice(&3u16.to_be_bytes());
    bytes[start + 12..start + 14].copy_from_slice(&id.to_be_bytes());
    bytes[start + 14..start + 18].fill(0);
    let cyclic = FontAsset::parse_for_shaping(bytes).unwrap();
    let error = create_semantic_pdf(&cyclic, "AB", 24., TextDirection::Ltr, true, 0)
        .err()
        .unwrap();
    assert!(
        error.to_string().contains("component work limit"),
        "{error}"
    );
}

#[test]
fn a_malformed_nonempty_outline_is_not_silently_omitted() {
    let source = font("shaped-text/DejaVuSans.ttf");
    let mut bytes = source.bytes.to_vec();
    let instruction_offset = {
        let face = ttf_parser::Face::parse(&bytes, 0).unwrap();
        let raw = face.raw_face();
        let glyf = raw.table(ttf_parser::Tag::from_bytes(b"glyf")).unwrap();
        let loca = ttf_parser::loca::Table::parse(
            face.tables().maxp.number_of_glyphs,
            face.tables().head.index_to_location_format,
            raw.table(ttf_parser::Tag::from_bytes(b"loca")).unwrap(),
        )
        .unwrap();
        let range = loca.glyph_range(face.glyph_index('A').unwrap()).unwrap();
        let count = i16::from_be_bytes([glyf[range.start], glyf[range.start + 1]]);
        assert!(count > 0);
        glyf.as_ptr() as usize - bytes.as_ptr() as usize + range.start + 10 + count as usize * 2
    };
    bytes[instruction_offset..instruction_offset + 2].copy_from_slice(&u16::MAX.to_be_bytes());
    let malformed = FontAsset::parse_for_shaping(bytes).unwrap();
    assert!(
        create_semantic_pdf(&malformed, "AB", 24., TextDirection::Ltr, true, 0).is_err(),
        "An unreadable A outline must not silently become a PDF that only paints B"
    );
}
