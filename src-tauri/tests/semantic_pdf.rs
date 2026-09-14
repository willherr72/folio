use folio_engine::{
    create_semantic_font_banks_probe, create_semantic_pdf, ExportRequest, FontAsset, PagePlan,
    PdfEngine, TextDirection,
};
use std::{fs, path::PathBuf};

#[test]
fn prepared_preview_retains_exact_font_layout_and_pdf_geometry() {
    use folio_engine::prepare_semantic_text;
    for (name, text) in [
        ("corpus/fonts/DejaVuSerif.ttf", "office"),
        ("shaped-text/DejaVuSans.ttf", "q\u{307}\u{323}"),
        ("shaped-text/NotoSansArabic-Regular.ttf", "سلام"),
        ("shaped-text/NotoSansDevanagari-Regular.ttf", "किताब"),
        ("corpus/fonts/DejaVuSerif.ttf", "A\u{1d434}B"),
    ] {
        let font = font(name);
        for rotation in [0, 90, 180, 270] {
            let prepared =
                prepare_semantic_text(&font, text, 24., TextDirection::Auto, true, rotation)
                    .unwrap();
            let preview = prepared.preview();
            assert_eq!(preview.version, 1);
            assert_eq!(preview.text, text);
            assert_eq!(preview.font_id, font.info.id);
            assert_eq!(preview.rotation, rotation);
            assert!(!preview.outlines.is_empty());
            assert!(!preview.glyphs.is_empty());
            let ids: std::collections::BTreeSet<_> =
                preview.outlines.iter().map(|o| o.glyph_id).collect();
            assert_eq!(ids.len(), preview.outlines.len());
            assert!(preview.glyphs.iter().all(|g| ids.contains(&g.glyph_id)));
            assert!(preview
                .outlines
                .iter()
                .all(|o| !o.path.is_empty() && o.path.is_ascii()));
            assert_eq!(prepared.pdf().layout.text, text);
            let pdf = lopdf::Document::load_mem(&prepared.pdf().bytes).unwrap();
            let page = pdf
                .get_dictionary(*pdf.get_pages().get(&1).unwrap())
                .unwrap();
            let media = page.get(b"MediaBox").unwrap().as_array().unwrap();
            assert_eq!(media[2].as_float().unwrap(), preview.width);
            assert_eq!(media[3].as_float().unwrap(), preview.height);
            assert!(contains_program(&prepared.pdf().bytes, &font.bytes));
            assert!(serde_json::to_vec(preview).unwrap().len() <= 8 * 1024 * 1024);
            let bytes = prepared.pdf().bytes.clone();
            assert_eq!(prepared.into_pdf().bytes, bytes);
        }
    }
}

#[test]
fn prepared_preview_keeps_writer_refusals_and_reuses_repeated_outlines() {
    let arabic = font("shaped-text/NotoSansArabic-Regular.ttf");
    let font = font("corpus/fonts/DejaVuSerif.ttf");
    let prepared = folio_engine::prepare_semantic_text(
        &font,
        &"i".repeat(100),
        12.,
        TextDirection::Ltr,
        false,
        0,
    )
    .unwrap();
    assert_eq!(prepared.preview().outlines.len(), 1);
    assert_eq!(prepared.preview().glyphs.len(), 100);
    for (text, rotation) in [("A", 45), ("\u{10ffff}", 0)] {
        assert!(folio_engine::prepare_semantic_text(
            &font,
            text,
            24.,
            TextDirection::Auto,
            true,
            rotation
        )
        .is_err());
    }
    assert!(folio_engine::prepare_semantic_text(
        &arabic,
        "سلام عالم",
        24.,
        TextDirection::Auto,
        true,
        0
    )
    .is_err());
}

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
        create_semantic_pdf(&font, &"i".repeat(4097), 4., TextDirection::Ltr, false, 0).is_err()
    );
    assert!(create_semantic_pdf(&font, "A", 24., TextDirection::Ltr, true, 45).is_err());
    assert!(
        create_semantic_pdf(&font, &"W".repeat(100), 1000., TextDirection::Ltr, true, 0).is_err()
    );
}

#[test]
fn repeated_semantic_text_reuses_codes_without_truncating_copy_or_selection() {
    let font = font("corpus/fonts/DejaVuSerif.ttf");
    let text = "i".repeat(4096);
    let engine = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    for rotation in [0, 90, 180, 270] {
        let output = create_semantic_pdf(&font, &text, 4., TextDirection::Ltr, false, rotation)
            .expect("Repeated text must not consume one PDF code per occurrence");
        let pdf = lopdf::Document::load_mem(&output.bytes).unwrap();
        let semantic = pdf
            .objects
            .values()
            .filter_map(|obj| obj.as_dict().ok())
            .find(|dict| {
                dict.get(b"Subtype").ok().and_then(|obj| obj.as_name().ok()) == Some(b"Type3")
            })
            .unwrap();
        assert!(semantic.get(b"LastChar").unwrap().as_i64().unwrap() <= 255);
        let path = temp.path().join(format!("long-{rotation}.pdf"));
        fs::write(&path, &output.bytes).unwrap();
        let source = engine.open_document(&path).unwrap();
        assert_eq!(engine.extract_text(&source.id, 0).unwrap(), text);
        let geometry = engine.page_text(&source.id, 0).unwrap();
        assert_eq!(geometry.characters.len(), 4096);
        assert_eq!(
            geometry
                .characters
                .iter()
                .map(|c| c.text.as_str())
                .collect::<String>(),
            text
        );
        // Reusing a code must retain each occurrence's distinct page position.
        let first = &geometry.characters[0];
        let last = &geometry.characters[4095];
        let span = if rotation % 180 == 0 {
            (last.x - first.x).abs()
        } else {
            (last.y - first.y).abs()
        };
        assert!(span > output.layout.width * 0.99);
        engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn font_bank_negative_controls_keep_unsafe_reader_order_out_of_the_writer() {
    let font = font("corpus/fonts/DejaVuSerif.ttf");
    let face = font.face().unwrap();
    let alphabet: String = (0x41..=0x52f)
        .filter_map(char::from_u32)
        .filter(|c| c.is_alphabetic() && face.glyph_index(*c).is_some())
        .take(511)
        .collect();
    assert_eq!(alphabet.chars().count(), 511);
    let engine = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    for count in [255usize, 256, 511] {
        let prefix: String = alphabet.chars().take(count).collect();
        // Return to earlier definitions after crossing one or two font banks.
        let text = format!("{prefix}AB{}CD", prefix.chars().last().unwrap());
        for rotation in [0, 90, 180, 270] {
            let output = create_semantic_font_banks_probe(
                &font,
                &text,
                4.,
                TextDirection::Ltr,
                false,
                rotation,
                false,
            )
            .unwrap();
            let pdf = lopdf::Document::load_mem(&output.bytes).unwrap();
            let banks: Vec<_> = pdf
                .objects
                .values()
                .filter_map(|o| o.as_dict().ok())
                .filter(|d| d.get(b"Subtype").ok().and_then(|o| o.as_name().ok()) == Some(b"Type3"))
                .collect();
            assert!(banks.len() >= count.div_ceil(255));
            for bank in banks {
                assert!((1..=255).contains(&bank.get(b"LastChar").unwrap().as_i64().unwrap()));
            }
            let path = temp.path().join(format!("banks-{count}-{rotation}.pdf"));
            fs::write(&path, &output.bytes).unwrap();
            let source = engine.open_document(&path).unwrap();
            let copied = engine.extract_text(&source.id, 0).unwrap();
            let geometry = engine.page_text(&source.id, 0).unwrap();
            if rotation == 180 {
                assert_ne!(
                    copied, text,
                    "Negative fixture must expose the reader-order failure"
                );
                engine.close_document(&source.id).unwrap();
                continue;
            }
            assert_eq!(copied, text);
            assert_eq!(
                geometry
                    .characters
                    .iter()
                    .map(|c| c.text.as_str())
                    .collect::<String>(),
                text
            );
            let first = &geometry.characters[0];
            let last = geometry.characters.last().unwrap();
            let span = if rotation % 180 == 0 {
                (last.x - first.x).abs()
            } else {
                (last.y - first.y).abs()
            };
            assert!(span > output.layout.width * 0.95);
            engine.close_document(&source.id).unwrap();
        }
    }
}

#[test]
fn rtl_word_boundaries_are_refused_before_reader_order_can_corrupt_selection() {
    let font = font("shaped-text/NotoSansArabic-Regular.ttf");
    let engine = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    for text in ["سلام عالم", "سلام\u{a0}عالم", "سلام،عالم", "سلام\u{200c}عالم"]
    {
        for rotation in [0, 90, 180, 270] {
            match create_semantic_pdf(&font, text, 24., TextDirection::Auto, true, rotation) {
                Err(_) => (),
                Ok(output) => {
                    let path = temp.path().join("rtl-words.pdf");
                    fs::write(&path, output.bytes).unwrap();
                    let source = engine.open_document(&path).unwrap();
                    let copied = engine.extract_text(&source.id, 0).unwrap();
                    let geometry = engine.page_text(&source.id, 0).unwrap();
                    panic!("Unsupported RTL word boundaries must be refused: source={text:?}, copied={copied:?}, rotation={rotation}, first={:?}, last={:?}", geometry.characters.first(), geometry.characters.last());
                }
            }
        }
    }
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

#[test]
fn wide_semantic_codes_preserve_unicode_geometry_and_native_save_at_all_rotations() {
    let font = font("corpus/fonts/DejaVuSerif.ttf");
    let face = font.face().unwrap();
    let alphabet: String = (0x41..=0x52f)
        .filter_map(char::from_u32)
        .filter(|c| c.is_alphabetic() && face.glyph_index(*c).is_some())
        .take(511)
        .collect();
    assert_eq!(alphabet.chars().count(), 511);
    let engine = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    for count in [256, 511] {
        let prefix: String = alphabet.chars().take(count).collect();
        let text = format!("{prefix}AB{}CD", prefix.chars().last().unwrap());
        for rotation in [0, 90, 180, 270] {
            let output = create_semantic_pdf(&font, &text, 4., TextDirection::Ltr, false, rotation)
                .expect("One wide semantic font must preserve definitions beyond 255");
            assert!(contains_program(&output.bytes, &font.bytes));
            let document = lopdf::Document::load_mem(&output.bytes).unwrap();
            assert!(!document
                .objects
                .values()
                .filter_map(|o| o.as_dict().ok())
                .any(|d| d.get(b"Subtype").ok().and_then(|o| o.as_name().ok()) == Some(b"Type3")));
            let path = temp.path().join(format!("wide-{count}-{rotation}.pdf"));
            fs::write(&path, &output.bytes).unwrap();
            let source = engine.open_document(&path).unwrap();
            assert_eq!(engine.extract_text(&source.id, 0).unwrap(), text);
            let geometry = engine.page_text(&source.id, 0).unwrap();
            assert_eq!(
                geometry
                    .characters
                    .iter()
                    .map(|c| c.text.as_str())
                    .collect::<String>(),
                text
            );
            let first = &geometry.characters[0];
            let last = geometry.characters.last().unwrap();
            let span = if rotation % 180 == 0 {
                (last.x - first.x).abs()
            } else {
                (last.y - first.y).abs()
            };
            assert!(
                span > output.layout.width * 0.95,
                "Wide copy must retain the full line geometry"
            );
            let before = engine.render_page(&source.id, 0, 1200).unwrap();
            let saved = temp.path().join(format!("saved-{count}-{rotation}.pdf"));
            let page = &source.pages[0];
            engine
                .export_pdf(
                    ExportRequest {
                        flatten: false,
                        pages: vec![PagePlan {
                            id: "wide".into(),
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
            let reopened = engine.open_document(&saved).unwrap();
            assert_eq!(engine.extract_text(&reopened.id, 0).unwrap(), text);
            assert_eq!(engine.render_page(&reopened.id, 0, 1200).unwrap(), before);
            engine.close_document(&reopened.id).unwrap();
            engine.close_document(&source.id).unwrap();
        }
    }
}

#[test]
fn whole_line_actualtext_repairs_copy_but_collapses_banked_selection() {
    let font = font("corpus/fonts/DejaVuSerif.ttf");
    let face = font.face().unwrap();
    let prefix: String = (0x41..=0x52f)
        .filter_map(char::from_u32)
        .filter(|c| c.is_alphabetic() && face.glyph_index(*c).is_some())
        .take(511)
        .collect();
    assert_eq!(prefix.chars().count(), 511);
    let text = format!("{prefix}AB{}CD", prefix.chars().last().unwrap());
    let engine = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    for rotation in [0, 90, 180, 270] {
        let output = create_semantic_font_banks_probe(
            &font,
            &text,
            4.,
            TextDirection::Ltr,
            false,
            rotation,
            true,
        )
        .unwrap();
        let path = temp.path().join(format!("actual-{rotation}.pdf"));
        fs::write(&path, &output.bytes).unwrap();
        let source = engine.open_document(&path).unwrap();
        assert_eq!(engine.extract_text(&source.id, 0).unwrap(), text);
        let geometry = engine.page_text(&source.id, 0).unwrap();
        let first = &geometry.characters[0];
        let last = geometry.characters.last().unwrap();
        let span = if rotation % 180 == 0 {
            (last.x - first.x).abs()
        } else {
            (last.y - first.y).abs()
        };
        assert!(
            span < output.layout.width * 0.75,
            "Negative control must expose collapsed caret positions"
        );
        engine.close_document(&source.id).unwrap();
    }
}
#[test]
fn editor_shaping_schema_preserves_options_and_rejects_unknown_version() {
    let wire = serde_json::json!({"id":"shape","x":20,"y":30,"text":"office","fontSize":24,"fontId":"exact","color":"#cc4422","shaping":{"version":1,"direction":"auto","ligatures":true}});
    let overlay: folio_engine::TextOverlay = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(overlay).unwrap()["shaping"],
        wire["shaping"]
    );
    let mut unknown = wire;
    unknown["shaping"]["version"] = 2.into();
    assert!(serde_json::from_value::<folio_engine::TextOverlay>(unknown).is_err());
}
#[test]
fn editor_preparation_has_requested_color_logical_cluster_bounds_and_refusals() {
    let asset = font("corpus/fonts/DejaVuSerif.ttf");
    let make = |text: &str| {
        serde_json::from_value::<folio_engine::TextOverlay>(serde_json::json!({"id":"shape","x":20,"y":30,"text":text,"fontSize":24,"fontId":asset.info.id,"color":"#cc4422","shaping":{"version":1,"direction":"auto","ligatures":true}})).unwrap()
    };
    let prepared = folio_engine::prepare_text_overlay(&asset, &make("office")).unwrap();
    assert_eq!(prepared.preview.color, [0.8, 68. / 255., 34. / 255.]);
    assert_eq!(
        prepared
            .characters
            .iter()
            .map(|c| c.text.as_str())
            .collect::<String>(),
        "office"
    );
    assert_eq!(prepared.characters[1].x, prepared.characters[2].x);
    assert_eq!(prepared.characters[1].width, prepared.characters[2].width);
    assert!(prepared.bounds.width > 20.);
    assert!(prepared
        .characters
        .iter()
        .all(|c| c.y >= prepared.bounds.y - 0.001));
    assert!(folio_engine::prepare_text_overlay(&asset, &make("a\nb")).is_err());
    let mut missing = make("office");
    missing.font_id = None;
    assert!(folio_engine::prepare_text_overlay(&asset, &missing).is_err());
    let blank = folio_engine::prepare_text_overlay(&asset, &make("")).unwrap();
    assert!(blank.preview.glyphs.is_empty());
    assert!(blank.characters.is_empty());
    assert_eq!(blank.bounds.height, 24.);
}
#[test]
fn editor_shaped_export_reopens_flattens_and_rejects_tampered_metadata() {
    use folio_engine::Overlay;
    let engine = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let mut pdf = lopdf::Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let page = pdf.add_object(lopdf::dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),600.into(),600.into()],"Resources"=>lopdf::dictionary!{}});
    pdf.objects.insert(
        pages,
        lopdf::Object::Dictionary(
            lopdf::dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1},
        ),
    );
    let catalog = pdf.add_object(lopdf::dictionary! {"Type"=>"Catalog","Pages"=>pages});
    pdf.trailer.set("Root", catalog);
    let source_path = temp.path().join("blank.pdf");
    pdf.save(&source_path).unwrap();
    let source = engine.open_document(&source_path).unwrap();
    for (name, text) in [
        ("corpus/fonts/DejaVuSerif.ttf", "office"),
        ("shaped-text/DejaVuSans.ttf", "q\u{307}\u{323}"),
        ("shaped-text/NotoSansArabic-Regular.ttf", "سلام"),
        ("shaped-text/NotoSansDevanagari-Regular.ttf", "किताब"),
        ("corpus/fonts/DejaVuSerif.ttf", ""),
    ] {
        let asset = font(name);
        engine.fonts().register(asset.bytes.to_vec()).unwrap();
        for rotation in [0, 90, 180, 270] {
            let overlay:Overlay=serde_json::from_value(serde_json::json!({"type":"text","id":"shape","x":250,"y":250,"text":text,"fontSize":24,"fontId":asset.info.id,"color":"#cc4422","rotation":rotation,"shaping":{"version":1,"direction":"auto","ligatures":true}})).unwrap();
            let plan = PagePlan {
                id: "page".into(),
                source_id: source.id.clone(),
                page_index: 0,
                width: 600.,
                height: 600.,
                rotation: 0,
                overlays: vec![overlay.clone()],
            };
            let saved = temp.path().join("saved.pdf");
            engine
                .export_pdf(
                    ExportRequest {
                        pages: vec![plan.clone()],
                        flatten: false,
                    },
                    &saved,
                )
                .unwrap();
            assert!(contains_program(&fs::read(&saved).unwrap(), &asset.bytes));
            let opened = engine.open_document(&saved).unwrap();
            assert_eq!(opened.pages[0].overlays, vec![overlay.clone()]);
            let flat = temp.path().join("flat.pdf");
            engine
                .export_pdf(
                    ExportRequest {
                        pages: vec![plan],
                        flatten: true,
                    },
                    &flat,
                )
                .unwrap();
            let flattened = engine.open_document(&flat).unwrap();
            assert!(flattened.pages[0].overlays.is_empty());
            let Overlay::Text(text_overlay) = &overlay else {
                unreachable!()
            };
            let prepared = engine.prepare_text_overlay(text_overlay).unwrap();
            let actual = engine.page_text(&flattened.id, 0).unwrap().characters;
            assert_eq!(actual.len(), prepared.characters.len());
            for (actual, local) in actual.iter().zip(&prepared.characters) {
                let rotate = |x: f32, y: f32| match rotation {
                    0 => (x, y),
                    90 => (-y, x),
                    180 => (-x, -y),
                    _ => (y, -x),
                };
                let points = [
                    (local.x, local.y),
                    (local.x + local.width, local.y),
                    (local.x, local.y + local.height),
                    (local.x + local.width, local.y + local.height),
                ]
                .map(|(x, y)| rotate(x, y));
                let left = points.iter().map(|p| p.0).fold(f32::INFINITY, f32::min) + 250.;
                let top = points.iter().map(|p| p.1).fold(f32::INFINITY, f32::min) + 250.;
                let right = points.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max) + 250.;
                let bottom = points.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max) + 250.;
                assert_eq!(actual.text, local.text);
                for (a, b) in [actual.x, actual.y, actual.width, actual.height]
                    .into_iter()
                    .zip([left, top, right - left, bottom - top])
                {
                    assert!(
                        (a - b).abs() < 0.05,
                        "cluster geometry {name} {rotation}: {a} vs {b}"
                    );
                }
            }
            if !text.is_empty() {
                let png = engine.render_page(&flattened.id, 0, 1000).unwrap();
                let image = image::load_from_memory(&png).unwrap().to_rgb8();
                assert!(
                    image.pixels().any(|p| p.0 == [204, 68, 34]),
                    "requested color must paint visible ink"
                );
            }

            assert_eq!(
                engine
                    .extract_text(&flattened.id, 0)
                    .unwrap()
                    .trim_end_matches(['\r', '\n']),
                text,
                "{name} {rotation}"
            );
            if text == "office" {
                let mut resources = lopdf::Document::load(&saved).unwrap();
                let mut changed = false;
                for object in resources.objects.values_mut() {
                    if let Ok(stream) = object.as_stream_mut() {
                        let content = stream
                            .decompressed_content()
                            .unwrap_or_else(|_| stream.content.clone());
                        if String::from_utf8_lossy(&content).contains("/FolioSemantic") {
                            stream.set_plain_content(
                                content
                                    .iter()
                                    .copied()
                                    .chain(b"% changed resource\n".iter().copied())
                                    .collect(),
                            );
                            changed = true;
                        }
                    }
                }
                assert!(changed);
                let path = temp.path().join("resource-tampered.pdf");
                resources.save(&path).unwrap();
                assert!(engine.open_document(&path).unwrap().pages[0]
                    .overlays
                    .is_empty());
            }
            let mut tampered = lopdf::Document::load(&saved).unwrap();
            for object in tampered.objects.values_mut() {
                if let Ok(dict) = object.as_dict_mut() {
                    if let Ok(raw) = dict.get(b"Folio").and_then(lopdf::Object::as_str) {
                        let mut metadata: serde_json::Value = serde_json::from_slice(raw).unwrap();
                        assert_eq!(metadata["version"], 3);
                        metadata["overlay"]["shaping"]["ligatures"] = false.into();
                        dict.set(
                            "Folio",
                            lopdf::Object::string_literal(serde_json::to_vec(&metadata).unwrap()),
                        );
                    }
                }
            }
            if text == "office" {
                let path = temp.path().join("tampered.pdf");
                tampered.save(&path).unwrap();
                assert!(engine.open_document(&path).unwrap().pages[0]
                    .overlays
                    .is_empty());
            }
            engine.close_document(&opened.id).unwrap();
            engine.close_document(&flattened.id).unwrap();
        }
    }
}

#[test]
fn editor_font_info_exposes_shaped_coverage_without_widening_legacy() {
    let asset = font("shaped-text/NotoSansArabic-Regular.ttf");
    let wire = serde_json::to_value(&asset.info).unwrap();
    assert!(wire["shapedCoverage"]
        .as_array()
        .is_some_and(|a| a
            .iter()
            .any(|r| r[0].as_u64().unwrap() <= 0x633 && r[1].as_u64().unwrap() >= 0x633)));
    assert!(asset.validate_text("سلام").is_err());
}

#[test]
fn editor_shaped_crop_and_intrinsic_rotation_matrix_preserves_geometry_and_ink() {
    use folio_engine::{AnnotationRect, Overlay, TextOverlay};
    let engine = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let asset = font("corpus/fonts/DejaVuSerif.ttf");
    engine.fonts().register(asset.bytes.to_vec()).unwrap();
    let overlay: TextOverlay = serde_json::from_value(serde_json::json!({
        "id":"matrix","x":180,"y":170,"text":"office","fontSize":24,
        "fontId":asset.info.id,"color":"#cc4422","rotation":0,
        "shaping":{"version":1,"direction":"auto","ligatures":true}
    }))
    .unwrap();
    let prepared = engine.prepare_text_overlay(&overlay).unwrap();
    let ink = folio_engine::shape_text(
        &asset,
        &overlay.text,
        overlay.font_size,
        TextDirection::Auto,
        true,
    )
    .unwrap()
    .bounds
    .unwrap();
    let ink = AnnotationRect {
        x: ink.x_min,
        y: overlay.font_size - ink.y_max,
        width: ink.x_max - ink.x_min,
        height: ink.y_max - ink.y_min,
    };
    for intrinsic in [0u16, 90, 180, 270] {
        let mut pdf = lopdf::Document::with_version("1.7");
        let pages = pdf.new_object_id();
        // The non-square crop has a nonzero origin in both PDF axes.
        let page = pdf.add_object(lopdf::dictionary!{
            "Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),700.into(),700.into()],
            "CropBox"=>vec![37.into(),53.into(),537.into(),453.into()],"Rotate"=>i64::from(intrinsic),"Resources"=>lopdf::dictionary!{}
        });
        pdf.objects.insert(
            pages,
            lopdf::Object::Dictionary(
                lopdf::dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1},
            ),
        );
        let catalog = pdf.add_object(lopdf::dictionary! {"Type"=>"Catalog","Pages"=>pages});
        pdf.trailer.set("Root", catalog);
        let source_path = temp.path().join(format!("source-{intrinsic}.pdf"));
        pdf.save(&source_path).unwrap();
        let source = engine.open_document(&source_path).unwrap();
        let source_page = &source.pages[0];
        assert_eq!(
            (source_page.width, source_page.height),
            if intrinsic % 180 == 0 {
                (500., 400.)
            } else {
                (400., 500.)
            }
        );
        for rotation in [0u16, 90, 180, 270] {
            for plan_rotation in [0u16, 90] {
                let label =
                    format!("intrinsic {intrinsic}, overlay {rotation}, plan {plan_rotation}");
                let mut overlay = overlay.clone();
                overlay.rotation = rotation;
                // Local text coordinates are already in the source's displayed
                // frame. Apply overlay rotation there, then the added page turn.
                let point = |x: f32, y: f32| {
                    let (x, y) = match rotation {
                        0 => (x, y),
                        90 => (-y, x),
                        180 => (-x, -y),
                        _ => (y, -x),
                    };
                    let (x, y) = (overlay.x + x, overlay.y + y);
                    if plan_rotation == 90 {
                        (source_page.height - y, x)
                    } else {
                        (x, y)
                    }
                };
                let rect = |r: &AnnotationRect| {
                    let corners = [
                        point(r.x, r.y),
                        point(r.x + r.width, r.y),
                        point(r.x, r.y + r.height),
                        point(r.x + r.width, r.y + r.height),
                    ];
                    let x = corners.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
                    let y = corners.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
                    AnnotationRect {
                        x,
                        y,
                        width: corners
                            .iter()
                            .map(|p| p.0)
                            .fold(f32::NEG_INFINITY, f32::max)
                            - x,
                        height: corners
                            .iter()
                            .map(|p| p.1)
                            .fold(f32::NEG_INFINITY, f32::max)
                            - y,
                    }
                };
                let plan = PagePlan {
                    id: "page".into(),
                    source_id: source.id.clone(),
                    page_index: 0,
                    width: source_page.width,
                    height: source_page.height,
                    rotation: plan_rotation,
                    overlays: vec![Overlay::Text(overlay.clone())],
                };
                let saved = temp.path().join("matrix-saved.pdf");
                engine
                    .export_pdf(
                        ExportRequest {
                            pages: vec![plan.clone()],
                            flatten: false,
                        },
                        &saved,
                    )
                    .unwrap();
                let reopened = engine.open_document(&saved).unwrap();
                let mut expected = overlay.clone();
                (expected.x, expected.y) = point(0., 0.);
                expected.rotation = (rotation + plan_rotation) % 360;
                assert_eq!(
                    reopened.pages[0].overlays,
                    vec![Overlay::Text(expected)],
                    "{label}"
                );
                let expected_size = if plan_rotation == 90 {
                    (source_page.height, source_page.width)
                } else {
                    (source_page.width, source_page.height)
                };
                assert_eq!(
                    (reopened.pages[0].width, reopened.pages[0].height),
                    expected_size,
                    "{label}"
                );
                engine.close_document(&reopened.id).unwrap();
                let flat = temp.path().join("matrix-flat.pdf");
                engine
                    .export_pdf(
                        ExportRequest {
                            pages: vec![plan],
                            flatten: true,
                        },
                        &flat,
                    )
                    .unwrap();
                let flattened = engine.open_document(&flat).unwrap();
                assert!(flattened.pages[0].overlays.is_empty(), "{label}");
                let characters = engine.page_text(&flattened.id, 0).unwrap().characters;
                assert_eq!(characters.len(), prepared.characters.len(), "{label}");
                for (actual, local) in characters.iter().zip(&prepared.characters) {
                    let expected = rect(&AnnotationRect {
                        x: local.x,
                        y: local.y,
                        width: local.width,
                        height: local.height,
                    });
                    assert_eq!(actual.text, local.text, "{label}");
                    for (a, b) in [actual.x, actual.y, actual.width, actual.height]
                        .into_iter()
                        .zip([expected.x, expected.y, expected.width, expected.height])
                    {
                        assert!(
                            (a - b).abs() < 0.05,
                            "{label}: character geometry {a} vs {b}"
                        );
                    }
                }
                let png = engine.render_page(&flattened.id, 0, 1000).unwrap();
                let image = image::load_from_memory(&png).unwrap().to_rgb8();
                let mut pixels = [u32::MAX, u32::MAX, 0, 0];
                let mut count = 0;
                for (x, y, pixel) in image.enumerate_pixels() {
                    if pixel.0.iter().any(|c| *c < 245) {
                        count += 1;
                        pixels[0] = pixels[0].min(x);
                        pixels[1] = pixels[1].min(y);
                        pixels[2] = pixels[2].max(x + 1);
                        pixels[3] = pixels[3].max(y + 1);
                    }
                }
                assert!(count > 100, "{label}: visible ink missing");
                assert!(
                    image.pixels().any(|p| p.0 == [204, 68, 34]),
                    "{label}: requested ink color missing"
                );
                let expected = rect(&ink);
                let sx = image.width() as f32 / expected_size.0;
                let sy = image.height() as f32 / expected_size.1;
                for (actual, expected) in pixels.into_iter().map(|p| p as f32).zip([
                    expected.x * sx,
                    expected.y * sy,
                    (expected.x + expected.width) * sx,
                    (expected.y + expected.height) * sy,
                ]) {
                    assert!(
                        (actual - expected).abs() < 2.,
                        "{label}: ink bounds {actual} vs {expected}"
                    );
                }
                engine.close_document(&flattened.id).unwrap();
            }
        }
        engine.close_document(&source.id).unwrap();
    }
}
