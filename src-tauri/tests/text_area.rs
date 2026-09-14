use folio_engine::*;
fn font() -> FontAsset {
    FontAsset::parse(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec())
        .unwrap()
}
fn request(text: &str) -> TextAreaRequest {
    TextAreaRequest {
        version: 1,
        text: text.into(),
        font_size: 18.,
        width: 200.,
        height: 300.,
        inset: 5.,
        line_spacing: 1.,
        alignment: TextAreaAlignment::Left,
        direction: TextDirection::Ltr,
        ligatures: true,
    }
}
fn check_source(layout: &TextAreaLayout) {
    let text = &layout.request.text;
    let mut cursor = 0;
    for line in &layout.lines {
        assert_eq!(line.source.utf8_start, cursor);
        assert_eq!(line.source.utf8_end, line.delimiter.utf8_start);
        for range in [line.source, line.delimiter] {
            assert_eq!(
                range.utf16_start,
                text[..range.utf8_start].encode_utf16().count()
            );
            assert_eq!(
                range.utf16_end,
                text[..range.utf8_end].encode_utf16().count()
            );
        }
        cursor = line.delimiter.utf8_end;
        if let Some(shaped) = &line.shaped {
            assert_eq!(
                shaped.text,
                text[line.source.utf8_start..line.source.utf8_end]
            );
            assert_eq!(
                *shaped,
                shape_text(
                    &font(),
                    &shaped.text,
                    layout.request.font_size,
                    TextDirection::Ltr,
                    layout.request.ligatures
                )
                .unwrap()
            );
        }
    }
    assert_eq!(cursor, text.len());
}
#[test]
fn wraps_at_space_runs_and_reshapes_final_lines_without_losing_source() {
    let mut r = request("office  café a\u{301}bc");
    r.width = 80.;
    let l = layout_text_area(&font(), &r).unwrap();
    assert_eq!(l.lines.len(), 3);
    assert!(l.can_export);
    assert_eq!(l.lines[0].shaped.as_ref().unwrap().text, "office");
    assert_eq!(
        &r.text[l.lines[0].delimiter.utf8_start..l.lines[0].delimiter.utf8_end],
        "  "
    );
    assert_eq!(l.lines[0].break_kind, TextAreaBreak::Soft);
    check_source(&l);
    let decoded = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    assert_eq!(l, layout_text_area(&font(), &decoded).unwrap());
}
#[test]
fn preserves_crlf_blank_rows_trailing_spaces_and_final_empty_row() {
    let l = layout_text_area(&font(), &request("A  \r\n\n B \n")).unwrap();
    assert_eq!(l.lines.len(), 4);
    assert!(l.lines[1].shaped.is_none());
    assert!(l.lines[3].shaped.is_none());
    assert_eq!(l.lines[0].source.utf8_end, 1);
    assert_eq!(l.lines[0].delimiter.utf8_end, 5);
    assert_eq!(l.lines[3].break_kind, TextAreaBreak::End);
    check_source(&l);
    for text in ["", "   ", "\n", " \n  "] {
        check_source(&layout_text_area(&font(), &request(text)).unwrap());
    }
}
#[test]
fn atomic_nbsp_and_long_words_overflow_without_truncation() {
    for text in ["hello\u{a0}world", "a\u{301}supercalifragilistic"] {
        let mut r = request(text);
        r.width = 20.;
        r.height = 12.;
        let l = layout_text_area(&font(), &r).unwrap();
        assert_eq!(l.lines.len(), 1);
        assert!(l.overflow.horizontal && l.overflow.vertical && !l.can_export);
        check_source(&l);
    }
}
#[test]
fn aligns_by_advance_and_uses_native_metrics_for_baseline_and_height() {
    let f = font();
    let mut r = request("Ag\nAg");
    let left = layout_text_area(&f, &r).unwrap();
    let face = f.face().unwrap();
    let scale = r.font_size / face.units_per_em() as f32;
    assert_eq!(
        left.lines[0].baseline_y,
        r.inset + face.ascender() as f32 * scale
    );
    r.alignment = TextAreaAlignment::Center;
    let center = layout_text_area(&f, &r).unwrap();
    r.alignment = TextAreaAlignment::Right;
    let right = layout_text_area(&f, &r).unwrap();
    assert!(
        (center.lines[0].baseline_x - (left.lines[0].baseline_x + right.lines[0].baseline_x) / 2.)
            .abs()
            < 0.001
    );
    r.height = left.lines[1].baseline_y;
    assert!(layout_text_area(&f, &r).unwrap().overflow.vertical);
    r.height = 300.;
    r.line_spacing = 2.;
    assert!(layout_text_area(&f, &r).unwrap().lines[1].baseline_y > right.lines[1].baseline_y);
}
#[test]
fn rejects_controls_unsupported_scripts_and_resource_exhaustion() {
    for text in [
        "a\rb",
        "a\tb",
        "a\u{202e}b",
        "שלום",
        "مرحبا",
        "देव",
        "a\u{200b}b",
    ] {
        assert!(
            layout_text_area(&font(), &request(text)).is_err(),
            "{text:?}"
        );
    }
    for text in [
        "a".repeat(4097),
        "\n".repeat(256),
        format!("a{}", "\u{301}".repeat(65)),
    ] {
        assert!(layout_text_area(&font(), &request(&text)).is_err());
    }
    let mut r = request(&"a ".repeat(2000));
    r.width = 14400.;
    assert!(layout_text_area(&font(), &r)
        .unwrap_err()
        .to_string()
        .contains("budget"));
}
#[test]
fn rejects_malformed_geometry_version_and_direction() {
    let f = font();
    for value in [f32::NAN, f32::INFINITY, -1., 0., 14401.] {
        let mut r = request("A");
        r.width = value;
        assert!(layout_text_area(&f, &r).is_err());
    }
    let mut r = request("A");
    r.inset = 100.;
    assert!(layout_text_area(&f, &r).is_err());
    r = request("A");
    r.version = 2;
    assert!(layout_text_area(&f, &r).is_err());
    r = request("A");
    r.direction = TextDirection::Auto;
    assert!(layout_text_area(&f, &r).is_err());
}

#[test]
fn whitespace_only_non_ascii_rows_are_explicitly_refused_without_mutating_draft() {
    for text in [" \u{a0} \n\u{a0}", "\u{2003}"] {
        let r = request(text);
        assert!(layout_text_area(&font(), &r)
            .unwrap_err()
            .to_string()
            .contains("whitespace-only"));
        assert_eq!(r.text, text);
    }
}

#[test]
fn combining_mark_on_space_is_not_discarded_or_split_at_wrap() {
    let r = TextAreaRequest {
        width: 35.,
        ..request("A \u{301}B C")
    };
    let l = layout_text_area(&font(), &r).unwrap();
    assert_eq!(l.lines[0].shaped.as_ref().unwrap().text, "A \u{301}B");
    assert!(l.overflow.horizontal);
    check_source(&l);
}

#[test]
fn line_limit_retains_exactly_256_rows_and_refuses_one_more() {
    let l = layout_text_area(&font(), &request(&"\n".repeat(255))).unwrap();
    assert_eq!(l.lines.len(), 256);
    assert!(l.overflow.vertical);
    check_source(&l);
    assert!(layout_text_area(&font(), &request(&"\n".repeat(256))).is_err());
    for text in ["A\u{2028}B", "A\u{2029}B"] {
        assert!(layout_text_area(&font(), &request(text)).is_err());
    }
}

#[test]
fn font_size_ligatures_and_actual_ink_control_layout() {
    let f = font();
    let mut r = request("office office");
    r.width = 115.;
    let small = layout_text_area(&f, &r).unwrap();
    assert_eq!(small.lines.len(), 1);
    r.font_size = 30.;
    let large = layout_text_area(&f, &r).unwrap();
    assert_eq!(large.lines.len(), 2);
    assert!(large.lines[0].baseline_y > small.lines[0].baseline_y);
    check_source(&large);
    r.ligatures = false;
    let unligated = layout_text_area(&f, &r).unwrap();
    check_source(&unligated);
    let count = |l: &TextAreaLayout| {
        l.lines[0]
            .shaped
            .as_ref()
            .unwrap()
            .runs
            .iter()
            .map(|run| run.glyphs.len())
            .sum::<usize>()
    };
    assert!(count(&unligated) > count(&large));
    let j = layout_text_area(&f, &request("j")).unwrap();
    assert!(j.lines[0].shaped.as_ref().unwrap().bounds.unwrap().x_min < 0.);
    assert!(j.overflow.horizontal && !j.can_export);
}

#[test]
fn inherits_non_ascii_whitespace_candidate_and_shaper_geometry_refusals() {
    let f = font();
    for (text, width) in [("\u{a0} A", 200.), ("A \u{a0} B", 30.)] {
        let mut r = request(text);
        r.width = width;
        assert!(layout_text_area(&f, &r).is_err());
        assert_eq!(r.text, text);
    }
    let mut r = request(&"W".repeat(4096));
    r.font_size = 512.;
    r.width = 14400.;
    assert!(layout_text_area(&f, &r).is_err());
    assert_eq!(r.text.chars().count(), 4096);
}
