use super::*;
fn styles() -> Vec<StyleMetrics> {
    vec![
        StyleMetrics {
            em: 10.,
            ascent: 8.,
            descent: 2.,
            glyphs: HashMap::new(),
        },
        StyleMetrics {
            em: 20.,
            ascent: 16.,
            descent: 4.,
            glyphs: HashMap::new(),
        },
    ]
}
fn glyph(text: &str, style_id: usize, width: f32, code: u8) -> MeasuredOccurrence {
    let metric = GlyphMetric {
        advance: width,
        ink: if text == " " {
            [0.; 4]
        } else {
            [0., -2., width, 8.]
        },
    };
    MeasuredOccurrence {
        text: text.into(),
        style_id,
        encoded: vec![code],
        gid: Some(u16::from(code) + 1),
        metric,
        reservation: metric,
    }
}
fn lf(style_id: usize) -> MeasuredOccurrence {
    let zero = GlyphMetric {
        advance: 0.,
        ink: [0.; 4],
    };
    MeasuredOccurrence {
        text: "\n".into(),
        style_id,
        encoded: vec![],
        gid: None,
        metric: zero,
        reservation: zero,
    }
}
fn layout(atoms: &[MeasuredOccurrence], width: f32) -> EngineResult<OccurrenceLayout> {
    layout_occurrences(
        atoms,
        &styles(),
        [0., 0., width, 100.],
        ParagraphAlignment::Left,
        1.2,
        1.,
        true,
        false,
        None,
    )
}
fn text(line: &StyledParagraphLayoutLine) -> String {
    line.runs.iter().map(|r| r.text.as_str()).collect()
}

#[test]
fn physical_occurrence_widths_do_not_become_global_letter_metrics() {
    let mut atoms = vec![
        glyph("ffi", 0, 8., 1),
        glyph(" ", 0, 1., 2),
        glyph("ffi", 0, 8., 3),
    ];
    let result = layout(&atoms, 9.).unwrap();
    assert_eq!(
        result.layout.lines.iter().map(text).collect::<Vec<_>>(),
        ["ffi ", "ffi"]
    );
    assert_eq!(
        result
            .fragments
            .iter()
            .map(|f| f.iter().map(|a| a.ordinal).collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        [vec![0, 1], vec![2]]
    );
    assert_eq!(result.fragments[0][1].origin[0], 8.);
    assert_eq!(result.fragments[1][0].origin[0], 0.);
    atoms[2].metric.advance = 12.;
    atoms[2].metric.ink[2] = 12.;
    atoms[2].reservation = atoms[2].metric;
    assert!(
        layout(&atoms, 9.).is_err(),
        "A wider occurrence borrowed the first ligature's advance"
    );
}

#[test]
fn physical_occurrences_keep_cross_style_words_whole_and_retain_blank_line_styles() {
    let word = [glyph("f", 0, 4., 1), glyph("fi", 1, 6., 2)];
    assert!(layout(&word, 9.).is_err());
    let result = layout(&word, 10.).unwrap();
    assert_eq!(text(&result.layout.lines[0]), "ffi");
    assert_eq!(result.layout.lines[0].runs[1].origin[0], 4.);
    let atoms = [
        glyph("ffi", 0, 8., 1),
        lf(0),
        lf(1),
        glyph("ffi", 0, 8., 2),
        lf(1),
    ];
    let result = layout(&atoms, 20.).unwrap();
    assert_eq!(
        result.layout.lines.iter().map(text).collect::<Vec<_>>(),
        ["ffi", "", "ffi", ""]
    );
    assert_eq!(
        result
            .layout
            .lines
            .iter()
            .map(|l| l.baseline)
            .collect::<Vec<_>>(),
        [92., 68., 44., 20.]
    );
    assert_eq!(
        result
            .fragments
            .iter()
            .flatten()
            .map(|p| p.ordinal)
            .collect::<Vec<_>>(),
        [0, 3]
    );
}

#[test]
fn physical_occurrence_reservation_keeps_the_text_anchor_and_baseline() {
    let mut atom = glyph("ffi", 0, 8., 1);
    atom.reservation.ink = [-4., -6., 11., 13.];
    let result = layout_occurrences(
        &[atom],
        &styles(),
        [0., 0., 20., 50.],
        ParagraphAlignment::Right,
        1.2,
        1.,
        true,
        true,
        Some(30.),
    )
    .unwrap();
    assert_eq!(result.layout.lines[0].runs[0].origin, [12., 30.]);
    assert_eq!(result.layout.lines[0].ascent, 13.);
    assert_eq!(result.layout.lines[0].descent, 6.);
    assert_eq!(
        result.fragments[0][0],
        PlacedOccurrence {
            ordinal: 0,
            origin: [12., 30.]
        }
    );
}

#[test]
fn physical_occurrence_first_line_measurement_uses_the_same_wrap_and_checks_later_atoms() {
    let mut atoms = [
        glyph("ffi", 0, 8., 1),
        glyph(" ", 0, 1., 2),
        glyph("ffi", 1, 12., 3),
    ];
    assert_eq!(
        first_line_ascent_occurrences(&atoms, &styles(), 10., 1., false).unwrap(),
        8.
    );
    assert_eq!(
        first_line_ascent_occurrences(&atoms, &styles(), 30., 1., false).unwrap(),
        16.
    );
    assert_eq!(
        first_line_ascent_occurrences(&atoms, &styles(), 1., 1., false).unwrap(),
        8.
    );
    assert!(layout(&atoms, 10.).is_err());
    atoms[2].metric.ink[3] = f32::NAN;
    assert!(first_line_ascent_occurrences(&atoms, &styles(), 10., 1., false).is_err());
}

#[test]
fn physical_occurrence_validation_rejects_split_controls_and_invalid_font_metrics() {
    let good = glyph("ffi", 0, 8., 1);
    assert!(layout(&[good], 30.).is_ok());
    let mut bad = glyph("f\ni", 0, 8., 1);
    assert!(layout(&[bad], 30.).is_err());
    bad = glyph("ffi ", 0, 8., 1);
    assert!(layout(&[bad], 30.).is_err());
    bad = glyph("ffi", 0, 8., 1);
    bad.gid = None;
    assert!(layout(&[bad], 30.).is_err());
    bad = glyph("ffi", 0, 8., 1);
    bad.gid = Some(0);
    assert!(layout(&[bad], 30.).is_err());
    bad = glyph("ffi", 0, 8., 1);
    bad.encoded.clear();
    assert!(layout(&[bad], 30.).is_err());
    bad = glyph("ffi", 0, 8., 1);
    bad.encoded = vec![1, 2, 3];
    assert!(layout(&[bad], 30.).is_err());
    bad = glyph("ffi", 0, 8., 1);
    bad.reservation.advance = 9.;
    assert!(layout(&[bad], 30.).is_err());
    bad = lf(0);
    bad.encoded = vec![1];
    assert!(layout(&[bad, glyph("ffi", 0, 8., 1)], 30.).is_err());
    assert!(layout(&[lf(0)], 30.).is_err());
    assert!(layout(&[glyph("ffi", 3, 8., 1)], 30.).is_err());
}
