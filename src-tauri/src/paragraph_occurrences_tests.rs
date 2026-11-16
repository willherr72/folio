use super::super::source_glyphs::{Codebook, Symbol};
use super::*;

fn source(scalars: bool, codes: &[u16]) -> Arc<BoundText> {
    let mut symbols = vec![(21, "ffi", 910.), (20, "fi", 610.), (4, " ", 250.)];
    if scalars {
        symbols.extend([(8, "f", 350.), (9, "i", 280.)]);
    }
    let book = Arc::new(
        Codebook::new(
            symbols
                .into_iter()
                .map(|(code, text, width)| Symbol {
                    code,
                    text: text.into(),
                    gid: code + 1,
                    width,
                })
                .collect(),
            2,
        )
        .unwrap(),
    );
    Arc::new(
        book.bind(
            &codes
                .iter()
                .flat_map(|c| c.to_be_bytes())
                .collect::<Vec<_>>(),
        )
        .unwrap(),
    )
}
fn run(text: &str, style: usize) -> Run<'_> {
    Run { text, style }
}
fn codes(atoms: &[Atom]) -> Vec<u16> {
    atoms
        .iter()
        .filter_map(|a| a.glyph.as_ref())
        .map(|g| {
            assert_eq!(g.bytes.len(), 2);
            u16::from_be_bytes([g.bytes[0], g.bytes[1]])
        })
        .collect()
}
fn never(_: usize, _: char) -> Result<Glyph> {
    panic!("A bound font used the scalar fallback");
}

#[test]
fn paragraph_occurrences_keep_different_encodings_of_identical_letters() {
    let old = source(true, &[21, 8, 8, 9]);
    let atoms = resolve(&[run("ffiffi", 0)], &[Some(old)], never).unwrap();
    assert_eq!(codes(&atoms), [21, 8, 8, 9]);
    assert_eq!(
        atoms.iter().map(|a| a.text.as_str()).collect::<String>(),
        "ffiffi"
    );
    assert_eq!(
        atoms
            .iter()
            .map(|a| a.glyph.as_ref().unwrap().width)
            .sum::<f32>(),
        1890.
    );
    assert_eq!(atoms[0].glyph.as_ref().unwrap().gid, Some(22));
}

#[test]
fn paragraph_occurrences_formatting_changes_preserve_each_shared_font_occurrence() {
    // Two derived formatting styles share the actual original font stream.
    // Coloring the later scalar letters must not borrow the earlier ffi glyph.
    let old = source(true, &[21, 8, 8, 9]);
    let atoms = resolve(
        &[run("ffi", 0), run("ffi", 1)],
        &[Some(old.clone()), Some(old)],
        never,
    )
    .unwrap();
    assert_eq!(codes(&atoms), [21, 8, 8, 9]);
    assert_eq!(
        atoms.iter().map(|a| a.style).collect::<Vec<_>>(),
        [0, 1, 1, 1]
    );
}

#[test]
fn paragraph_occurrences_equal_separate_font_streams_do_not_acquire_shared_ownership() {
    let a = source(true, &[21, 8, 8, 9]);
    let b = Arc::new((*a).clone());
    assert_eq!(a, b);
    assert!(!Arc::ptr_eq(&a, &b));
    let atoms = resolve(&[run("ffi", 0), run("ffi", 1)], &[Some(a), Some(b)], never).unwrap();
    // These are independent source streams. Each edit may retain its own first
    // ligature; value equality does not grant cross-resource source ownership.
    assert_eq!(codes(&atoms), [21, 21]);
}

#[test]
fn paragraph_occurrences_adjacent_equal_styles_do_not_split_a_source_glyph() {
    let atoms = resolve(
        &[run("f", 0), run("fi", 0)],
        &[Some(source(false, &[21]))],
        never,
    )
    .unwrap();
    assert_eq!(codes(&atoms), [21]);
    assert_eq!(atoms[0].text, "ffi");
}

#[test]
fn paragraph_occurrences_style_boundaries_require_actual_decomposition() {
    let old = source(true, &[21]);
    let atoms = resolve(
        &[run("f", 0), run("fi", 1)],
        &[Some(old.clone()), Some(old)],
        never,
    )
    .unwrap();
    assert_eq!(codes(&atoms), [8, 8, 9]);
    assert_eq!(atoms.iter().map(|a| a.style).collect::<Vec<_>>(), [0, 1, 1]);
    let missing = source(false, &[21]);
    assert!(resolve(
        &[run("f", 0), run("fi", 1)],
        &[Some(missing.clone()), Some(missing)],
        never
    )
    .is_err());
}

#[test]
fn paragraph_occurrences_never_join_across_intervening_styles_or_hard_breaks() {
    let old = source(true, &[21]);
    let atoms = resolve(
        &[run("f", 0), run(" ", 1), run("fi", 0)],
        &[Some(old.clone()), Some(old.clone())],
        never,
    )
    .unwrap();
    assert_eq!(codes(&atoms), [8, 4, 8, 9]);
    let atoms = resolve(&[run("f\nfi", 0)], &[Some(old)], never).unwrap();
    assert_eq!(codes(&atoms), [8, 8, 9]);
    assert_eq!(
        atoms[1],
        Atom {
            text: "\n".into(),
            style: 0,
            glyph: None
        }
    );
    let missing = source(false, &[21]);
    assert!(resolve(&[run("f\nfi", 0)], &[Some(missing)], never).is_err());
    let atoms = resolve(
        &[run("ffi\n\nffi\n", 0)],
        &[Some(source(false, &[21, 21]))],
        never,
    )
    .unwrap();
    assert_eq!(codes(&atoms), [21, 21]);
    assert_eq!(atoms.iter().filter(|a| a.glyph.is_none()).count(), 3);
}

#[test]
fn paragraph_occurrences_scalar_fonts_keep_utf8_attribution_and_controls_outside_encoding() {
    let mut calls = Vec::new();
    let atoms = resolve(
        &[run("ffi", 0), run("x\né", 1)],
        &[Some(source(false, &[21])), None],
        |style, ch| {
            calls.push((style, ch));
            Ok(Glyph {
                bytes: (ch as u16).to_be_bytes().to_vec(),
                gid: None,
                width: 500.,
            })
        },
    )
    .unwrap();
    assert_eq!(calls, [(1, 'x'), (1, 'é')]);
    assert_eq!(
        atoms.iter().map(|a| a.text.as_str()).collect::<String>(),
        "ffix\né"
    );
    assert_eq!(atoms[2].glyph, None);
    assert_eq!(atoms[3].glyph.as_ref().unwrap().bytes, [0, 233]);
}

#[test]
fn paragraph_occurrences_limits_precede_encoding_and_bad_scalar_metrics_refuse() {
    let text = "a".repeat(4096);
    let atoms = resolve(&[run(&text, 0)], &[None], |_, _| {
        Ok(Glyph {
            bytes: vec![0, 65],
            gid: Some(1),
            width: 500.,
        })
    })
    .unwrap();
    assert_eq!(atoms.len(), 4096);
    assert_eq!(
        atoms
            .iter()
            .map(|a| a.glyph.as_ref().unwrap().bytes.len())
            .sum::<usize>(),
        8192
    );
    for (text, style, count) in [
        ("a".repeat(4097), 0, 1),
        ("a".into(), 1, 1),
        ("a".into(), 0, 0),
        ("a".into(), 0, 65),
        (" \n ".into(), 0, 1),
        ("a\rb".into(), 0, 1),
    ] {
        let mut calls = 0;
        let sources = vec![None; count];
        assert!(resolve(&[run(&text, style)], &sources, |_, _| {
            calls += 1;
            panic!("Invalid draft reached a font encoder");
        })
        .is_err());
        assert_eq!(calls, 0);
    }
    for glyph in [
        Glyph {
            bytes: vec![],
            gid: None,
            width: 100.,
        },
        Glyph {
            bytes: vec![1, 2, 3],
            gid: None,
            width: 100.,
        },
        Glyph {
            bytes: vec![1],
            gid: Some(0),
            width: 100.,
        },
        Glyph {
            bytes: vec![1],
            gid: None,
            width: f32::NAN,
        },
        Glyph {
            bytes: vec![1],
            gid: None,
            width: f32::INFINITY,
        },
        Glyph {
            bytes: vec![1],
            gid: None,
            width: -1.,
        },
        Glyph {
            bytes: vec![1],
            gid: None,
            width: 10001.,
        },
    ] {
        let mut calls = 0;
        assert!(resolve(&[run("a", 0)], &[None], |_, _| {
            calls += 1;
            Ok(glyph.clone())
        })
        .is_err());
        assert_eq!(calls, 1);
    }
}

#[test]
fn paragraph_occurrences_all_small_style_and_line_partitions_keep_exact_attribution() {
    let mut cases = 0;
    for styles in 0..64 {
        for breaks in 0..32 {
            let mut draft = Vec::new();
            let mut expected = Vec::new();
            for (i, ch) in "ffiffi".chars().enumerate() {
                let style = (styles >> i) & 1;
                let text = if i < 5 && breaks & (1 << i) != 0 {
                    format!("{ch}\n")
                } else {
                    ch.to_string()
                };
                expected.extend(text.chars().map(|c| (c, style)));
                draft.push((text, style));
            }
            let runs: Vec<_> = draft.iter().map(|(s, style)| run(s, *style)).collect();
            let old = source(true, &[21, 21]);
            let atoms = resolve(&runs, &[Some(old.clone()), Some(old)], never).unwrap();
            let actual: Vec<_> = atoms
                .iter()
                .flat_map(|a| a.text.chars().map(move |c| (c, a.style)))
                .collect();
            assert_eq!(actual, expected);
            for atom in &atoms {
                if let Some(glyph) = &atom.glyph {
                    assert!(!atom.text.contains('\n'));
                    let decoded = source(
                        true,
                        &[u16::from_be_bytes([glyph.bytes[0], glyph.bytes[1]])],
                    );
                    assert_eq!(decoded.encoded().text(), atom.text);
                    assert_eq!(decoded.encoded().atoms()[0].gid, glyph.gid.unwrap());
                    assert_eq!(decoded.encoded().atoms()[0].width, glyph.width);
                } else {
                    assert_eq!(atom.text, "\n");
                }
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 2048);
}
