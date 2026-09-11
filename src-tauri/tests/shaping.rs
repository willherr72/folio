use folio_engine::{shape_text, FontAsset, ShapedText, TextDirection};
use std::{fs, path::PathBuf};
fn font(name: &str) -> FontAsset {
    FontAsset::parse(
        fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../tests/fixtures")
                .join(name),
        )
        .unwrap(),
    )
    .unwrap()
}
fn glyph_count(text: &ShapedText) -> usize {
    text.runs.iter().map(|run| run.glyphs.len()).sum()
}

#[test]
fn shaping_exhaustion_returns_an_explicit_error() {
    let mut accepted = Vec::new();
    for (name, text) in [
        ("recursive", "A".into()),
        ("budget", "A".repeat(16)),
        ("expansion", "A".repeat(64)),
    ] {
        let bytes = fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../tests/fixtures/shaped-text/exhaustion/{name}.ttf"
        )))
        .unwrap();
        let font = FontAsset::parse_for_shaping(bytes).unwrap();
        let start = std::time::Instant::now();
        let result = shape_text(&font, &text, 18.0, TextDirection::Ltr, true);
        eprintln!(
            "{name}: {:?}, output={:?}",
            start.elapsed(),
            result.as_ref().map(|layout| layout
                .runs
                .iter()
                .flat_map(|run| run.glyphs.iter().map(|glyph| glyph.glyph_id))
                .collect::<Vec<_>>())
        );
        if result.is_ok() {
            accepted.push(name);
        } else {
            assert!(result.unwrap_err().to_string().contains("budget"));
        }
    }
    assert!(
        accepted.is_empty(),
        "resource exhaustion produced successful partial layouts: {accepted:?}"
    );
}

#[test]
fn repeated_composite_outline_is_rejected_before_bounds_traversal() {
    let asset = font("shaped-text/exhaustion/composite.ttf");
    let start = std::time::Instant::now();
    let result = shape_text(&asset, "A", 18.0, TextDirection::Ltr, true);
    eprintln!(
        "composite: {:?}, accepted={}",
        start.elapsed(),
        result.is_ok()
    );
    assert!(result.unwrap_err().to_string().contains("outline"));
}

#[test]
fn point_matched_components_are_refused_before_the_outline_parser() {
    let asset = font("shaped-text/exhaustion/composite.ttf");
    let mut bytes = asset.bytes.to_vec();
    let face = asset.face().unwrap();
    let glyf = face
        .raw_face()
        .table(ttf_parser::Tag::from_bytes(b"glyf"))
        .unwrap();
    let loca = ttf_parser::loca::Table::parse(
        face.tables().maxp.number_of_glyphs,
        face.tables().head.index_to_location_format,
        face.raw_face()
            .table(ttf_parser::Tag::from_bytes(b"loca"))
            .unwrap(),
    )
    .unwrap();
    let offset = glyf.as_ptr() as usize - asset.bytes.as_ptr() as usize
        + loca
            .glyph_range(face.glyph_index('A').unwrap())
            .unwrap()
            .start
        + 10;
    // One simple B component, but point-matched args. ttf-parser does not consume
    // those args, so a preflight must refuse them instead of advancing past them.
    bytes[offset..offset + 2].copy_from_slice(&0u16.to_be_bytes());
    bytes[offset + 2..offset + 4].copy_from_slice(&face.glyph_index('B').unwrap().0.to_be_bytes());
    let asset = FontAsset::parse_for_shaping(bytes).unwrap();
    let error = shape_text(&asset, "A", 18.0, TextDirection::Ltr, true).unwrap_err();
    assert!(error.to_string().contains("point-matched"), "{error}");
}

#[test]
fn malformed_glyph_locations_cannot_silently_remove_ink() {
    let original = font("shaped-text/exhaustion/expansion.ttf");
    let face = original.face().unwrap();
    let loca = face
        .raw_face()
        .table(ttf_parser::Tag::from_bytes(b"loca"))
        .unwrap();
    let start = loca.as_ptr() as usize - original.bytes.as_ptr() as usize;
    let glyph = face.glyph_index('A').unwrap().0 as usize;
    let mut bytes = original.bytes.to_vec();
    match face.tables().head.index_to_location_format {
        ttf_parser::head::IndexToLocationFormat::Short => {
            bytes[start + glyph * 2..start + glyph * 2 + 2].copy_from_slice(&u16::MAX.to_be_bytes())
        }
        ttf_parser::head::IndexToLocationFormat::Long => {
            bytes[start + glyph * 4..start + glyph * 4 + 4].copy_from_slice(&u32::MAX.to_be_bytes())
        }
    }
    let asset = FontAsset::parse_for_shaping(bytes).unwrap();
    let error = shape_text(&asset, "AB", 18.0, TextDirection::Ltr, false).unwrap_err();
    assert!(error.to_string().contains("outline"), "{error}");
}

#[test]
fn harfrust_limits_detect_transient_growth_and_defaults_preserve_normal_shaping() {
    let asset = font("shaped-text/exhaustion/expansion.ttf");
    let font_ref = harfrust::FontRef::new(&asset.bytes).unwrap();
    let data = harfrust::ShaperData::new(&font_ref);
    let shaper = data.shaper(&font_ref).build();
    let buffer = |reserve| {
        let mut buffer = harfrust::UnicodeBuffer::new();
        buffer.reserve(reserve);
        buffer.push_str("A");
        buffer.guess_segment_properties();
        buffer
    };
    let default = shaper.shape(buffer(0), harfrust::ShapeOptions::new());
    assert!(default.is_successful());
    assert_eq!(default.len(), 1, "expansion is followed by contraction");
    for reserve in [0, 1024] {
        let limited = shaper.shape(
            buffer(reserve),
            harfrust::ShapeOptions::new().max_glyphs(Some(32)),
        );
        assert!(
            !limited.is_successful(),
            "must enforce intermediate length even with reserved capacity"
        );
        assert!(limited.len() <= 32);
    }
    let no_ops = shaper.shape(
        buffer(0),
        harfrust::ShapeOptions::new().max_operations(Some(0)),
    );
    assert!(!no_ops.is_successful());
    let no_glyphs = shaper.shape(buffer(0), harfrust::ShapeOptions::new().max_glyphs(Some(0)));
    assert!(!no_glyphs.is_successful());
    for (name, text) in [("recursive", "A".into()), ("budget", "A".repeat(16))] {
        let font = font(&format!("shaped-text/exhaustion/{name}.ttf"));
        let face = harfrust::FontRef::new(&font.bytes).unwrap();
        let data = harfrust::ShaperData::new(&face);
        let shaper = data.shaper(&face).build();
        let mut buffer = harfrust::UnicodeBuffer::new();
        buffer.push_str(&text);
        buffer.guess_segment_properties();
        let output = shaper.shape(buffer, harfrust::ShapeOptions::new());
        assert!(
            !output.is_successful(),
            "upstream default exhaustion status must be visible: {name}"
        );
    }
    eprintln!("HarfRust glyph storage: info={}B positions={}B, 16384 slots={}B (excludes allocator capacity and other font caches)",std::mem::size_of::<harfrust::GlyphInfo>(),std::mem::size_of::<harfrust::GlyphPosition>(),16384*(std::mem::size_of::<harfrust::GlyphInfo>()+std::mem::size_of::<harfrust::GlyphPosition>()));
}

#[test]
fn latin_ligatures_are_explicit_and_logical_unicode_is_retained() {
    let font = font("corpus/fonts/DejaVuSerif.ttf");
    let on = shape_text(&font, "office", 18.0, TextDirection::Auto, true).unwrap();
    let off = shape_text(&font, "office", 18.0, TextDirection::Auto, false).unwrap();
    assert!(glyph_count(&on) < glyph_count(&off));
    assert_eq!(off.text, "office");
    assert_eq!(on.text, "office");
    assert!(on
        .runs
        .iter()
        .flat_map(|run| &run.glyphs)
        .any(|glyph| glyph.cluster.utf8_end - glyph.cluster.utf8_start > 1));
    assert_eq!(on.font_id, font.info.id);
    assert_eq!(
        serde_json::from_str::<ShapedText>(&serde_json::to_string(&on).unwrap()).unwrap(),
        on
    );
}

#[test]
fn composed_and_decomposed_accents_share_geometry_with_zero_advance_positioned_marks() {
    let font = font("corpus/fonts/DejaVuSerif.ttf");
    let composed = shape_text(&font, "ạ\u{301}", 24.0, TextDirection::Ltr, true).unwrap();
    let decomposed = shape_text(&font, "a\u{323}\u{301}", 24.0, TextDirection::Ltr, true).unwrap();
    let geometry = |layout: &ShapedText| {
        layout
            .runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .map(|g| (g.glyph_id, g.x, g.y, g.x_advance, g.y_advance))
            .collect::<Vec<_>>()
    };
    assert_eq!(geometry(&composed), geometry(&decomposed));
    assert!(
        decomposed.runs[0].glyphs.iter().any(|g| g.x_advance == 0.0),
        "{:?}",
        decomposed.runs[0].glyphs
    );
    assert!(decomposed.runs[0]
        .glyphs
        .iter()
        .all(|g| g.cluster.utf8_start == 0 && g.cluster.utf8_end == 5 && g.cluster.utf16_end == 3));
    assert_eq!(decomposed.grapheme_boundaries.len(), 2);
    assert!(
        font.validate_text(&decomposed.text).is_err(),
        "legacy validation must remain unshaped"
    );
    let mark_font = FontAsset::parse(
        fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../tests/fixtures/shaped-text/DejaVuSans.ttf"),
        )
        .unwrap(),
    )
    .unwrap();
    let stacked = shape_text(
        &mark_font,
        "q\u{307}\u{323}",
        24.0,
        TextDirection::Ltr,
        true,
    )
    .unwrap();
    assert!(
        stacked.runs[0]
            .glyphs
            .iter()
            .any(|g| g.x_advance == 0.0 && g.x_offset != 0.0 && g.y_offset != 0.0),
        "{:?}",
        stacked.runs[0].glyphs
    );
}

#[test]
fn supplementary_text_has_correct_utf8_and_utf16_cluster_ranges() {
    let font = font("corpus/fonts/DejaVuSerif.ttf");
    let layout = shape_text(&font, "A𝑁𝑂", 18.0, TextDirection::Auto, true).unwrap();
    let ranges: Vec<_> = layout
        .runs
        .iter()
        .flat_map(|run| &run.glyphs)
        .map(|g| {
            (
                g.cluster.utf8_start,
                g.cluster.utf8_end,
                g.cluster.utf16_start,
                g.cluster.utf16_end,
            )
        })
        .collect();
    assert_eq!(ranges, vec![(0, 1, 0, 1), (1, 5, 1, 3), (5, 9, 3, 5)]);
}

#[test]
fn arabic_joining_and_mixed_bidi_preserve_logical_text_and_visual_runs() {
    let arabic = font("shaped-text/NotoSansArabic-Regular.ttf");
    let layout = shape_text(&arabic, "سلام 123", 20.0, TextDirection::Auto, true).unwrap();
    assert_eq!(layout.text, "سلام 123");
    assert_eq!(layout.direction, TextDirection::Rtl);
    assert_eq!(layout.runs[0].direction, TextDirection::Ltr);
    assert_eq!(
        &layout.text[layout.runs[0].range.utf8_start..layout.runs[0].range.utf8_end],
        "123"
    );
    let arabic_run = layout
        .runs
        .iter()
        .find(|run| run.direction == TextDirection::Rtl)
        .unwrap();
    assert_eq!(arabic_run.script, "Arab");
    assert!(arabic_run
        .glyphs
        .windows(2)
        .any(|g| g[0].cluster.utf8_start > g[1].cluster.utf8_start));
    assert!(
        arabic_run.glyphs.iter().any(|g| !"سلام "
            .chars()
            .filter_map(|c| arabic.face().unwrap().glyph_index(c))
            .any(|id| id.0 == g.glyph_id)),
        "joining must select contextual glyphs"
    );
    let mixed = font("shaped-text/DejaVuSans.ttf");
    let layout = shape_text(&mixed, "ABC سلام 123 DEF", 20.0, TextDirection::Auto, true).unwrap();
    assert_eq!(layout.direction, TextDirection::Ltr);
    assert!(layout
        .runs
        .iter()
        .any(|run| run.direction == TextDirection::Rtl));
    assert!(layout.runs.iter().any(|run| run.script == "Latn"));
    assert!(layout
        .runs
        .iter()
        .flat_map(|run| &run.glyphs)
        .all(|g| g.glyph_id != 0));
}

#[test]
fn indic_reordering_and_joiner_input_are_shaped_without_losing_logical_clusters() {
    let font = font("shaped-text/NotoSansDevanagari-Regular.ttf");
    let layout = shape_text(&font, "किताब", 22.0, TextDirection::Auto, true).unwrap();
    assert_eq!(layout.runs[0].script, "Deva");
    assert_eq!(layout.runs[0].glyphs[0].cluster.utf8_start, 0);
    assert!(layout.runs[0].glyphs[0].cluster.utf8_end >= 6);
    let first = font.face().unwrap().glyph_index('क').unwrap().0;
    assert_ne!(
        layout.runs[0].glyphs[0].glyph_id, first,
        "pre-base vowel should reorder"
    );
    for text in ["क्\u{200d}ष", "क्\u{200c}ष"] {
        let layout = shape_text(&font, text, 22.0, TextDirection::Ltr, true).unwrap();
        assert_eq!(layout.text, text);
        assert!(glyph_count(&layout) > 0);
    }
}

#[test]
fn missing_glyph_controls_invalid_assets_and_resource_excess_are_explicit_errors() {
    let mut font = font("corpus/fonts/DejaVuSerif.ttf");
    for text in [
        "x\u{378}",
        "x\n",
        "x\u{202e}",
        "x\u{2066}",
        "x\u{200b}",
        "x\u{fe0f}",
        "",
    ] {
        assert!(
            shape_text(&font, text, 18.0, TextDirection::Auto, true).is_err(),
            "{text:?}"
        );
    }
    let error = shape_text(&font, "\u{378}", 18.0, TextDirection::Auto, true)
        .unwrap_err()
        .to_string();
    assert!(error.contains("U+0378"), "{error}");
    for size in [f32::NAN, f32::INFINITY, 0.0, 1001.0] {
        assert!(shape_text(&font, "A", size, TextDirection::Auto, true).is_err());
    }
    assert!(shape_text(&font, &"A".repeat(4097), 18.0, TextDirection::Auto, true).is_err());
    assert!(shape_text(
        &font,
        &format!("a{}", "\u{301}".repeat(64)),
        18.0,
        TextDirection::Auto,
        true
    )
    .is_err());
    font.info.id = "forged".into();
    assert!(shape_text(&font, "A", 18.0, TextDirection::Auto, true).is_err());
}

#[test]
fn excessive_script_runs_are_bounded_and_shaping_profile_does_not_widen_legacy_text() {
    let bytes = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/shaped-text/DejaVuSans.ttf"),
    )
    .unwrap();
    let font = FontAsset::parse_for_shaping(bytes).unwrap();
    assert!(font.validate_text("سلام").is_err());
    let error = shape_text(&font, &"Aسلام".repeat(256), 18.0, TextDirection::Auto, true)
        .unwrap_err()
        .to_string();
    assert!(error.contains("256 shaping runs"), "{error}");
    assert!(FontAsset::parse_for_shaping(vec![0u8; 16 * 1024 * 1024 + 1]).is_err());
    assert!(FontAsset::parse_for_shaping(b"invalid font".to_vec()).is_err());
}

#[test]
fn malformed_font_ink_extents_cannot_escape_the_output_geometry_limit() {
    let original = font("corpus/fonts/DejaVuSerif.ttf");
    let face = original.face().unwrap();
    let glyph = face.glyph_index('A').unwrap().0 as usize;
    let offset = |tag: &[u8; 4]| {
        face.raw_face()
            .table_records
            .into_iter()
            .find(|record| record.tag == ttf_parser::Tag::from_bytes(tag))
            .unwrap()
            .offset as usize
    };
    let head = offset(b"head");
    let glyf = offset(b"glyf");
    let loca = offset(b"loca");
    let short = i16::from_be_bytes([original.bytes[head + 50], original.bytes[head + 51]]) == 0;
    let start = if short {
        u16::from_be_bytes(
            original.bytes[loca + glyph * 2..loca + glyph * 2 + 2]
                .try_into()
                .unwrap(),
        ) as usize
            * 2
    } else {
        u32::from_be_bytes(
            original.bytes[loca + glyph * 4..loca + glyph * 4 + 4]
                .try_into()
                .unwrap(),
        ) as usize
    };
    let mut bytes = original.bytes.to_vec();
    bytes[head + 18..head + 20].copy_from_slice(&16u16.to_be_bytes());
    let glyph_start = glyf + start;
    let contours =
        u16::from_be_bytes(bytes[glyph_start..glyph_start + 2].try_into().unwrap()) as usize;
    let ends = glyph_start + 10 + 2 * contours;
    let points = u16::from_be_bytes(bytes[ends - 2..ends].try_into().unwrap()) as usize + 1;
    let instruction_len = u16::from_be_bytes(bytes[ends..ends + 2].try_into().unwrap()) as usize;
    let mut cursor = ends + 2 + instruction_len;
    let first_flag = bytes[cursor];
    let mut flags = 0;
    while flags < points {
        let flag = bytes[cursor];
        cursor += 1;
        flags += 1;
        if flag & 8 != 0 {
            flags += bytes[cursor] as usize;
            cursor += 1;
        }
    }
    assert_eq!(
        first_flag & 0x12,
        0,
        "fixture's first x delta must be a signed16-bit value"
    );
    // Shift every outline point via its first x delta; ttf-parser computes the
    // real outline bounds, so falsifying only the stored glyf header is insufficient.
    bytes[cursor..cursor + 2].copy_from_slice(&30000i16.to_be_bytes());
    let font = FontAsset::parse_for_shaping(bytes).unwrap();
    let result = shape_text(&font, "A", 1000.0, TextDirection::Auto, true);
    assert!(result.is_err(), "unbounded ink geometry: {result:?}");
}
