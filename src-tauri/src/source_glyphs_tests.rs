use super::*;

#[test]
fn source_glyphs_logical_slots_and_memory_charges_include_owned_storage() {
    let book = book(true);
    let bound = book.bind(&bytes(&[1, 21, 2, 3])).unwrap();
    assert_eq!(bound.encoded().unicode_boundaries(), vec![0, 1, 4, 5, 6]);
    let actual = std::mem::size_of::<BoundText>()
        + bound.encoded.text.capacity()
        + bound.encoded.bytes.capacity()
        + bound.encoded.atoms.capacity() * std::mem::size_of::<GlyphAtom>();
    assert!(bound.retained_bytes() >= actual);
    assert!(book.retained_bytes() >= book.symbols.len() * std::mem::size_of::<Symbol>());
    assert!(book.retained_bytes() < 128 * 1024);
}

fn symbol(code: u16, text: &str, width: f32) -> Symbol {
    Symbol {
        code,
        text: text.into(),
        gid: code + 1,
        width,
    }
}
fn book(scalars: bool) -> Arc<Codebook> {
    let mut symbols = vec![
        symbol(1, "O", 750.),
        symbol(2, "c", 500.),
        symbol(3, "e", 500.),
        symbol(20, "fi", 610.),
        symbol(21, "ffi", 910.),
        symbol(4, " ", 250.),
        symbol(5, "t", 350.),
        symbol(6, "h", 550.),
        symbol(7, "s", 450.),
    ];
    if scalars {
        symbols.extend([symbol(8, "f", 350.), symbol(9, "i", 280.)]);
    }
    Arc::new(Codebook::new(symbols, 2).unwrap())
}
fn bytes(codes: &[u16]) -> Vec<u8> {
    codes.iter().flat_map(|code| code.to_be_bytes()).collect()
}
fn codes(text: &EncodedText) -> Vec<u16> {
    text.atoms().iter().map(|a| a.code).collect()
}

#[test]
fn source_glyphs_keep_distinct_encodings_of_the_same_unicode() {
    let b = book(true);
    for (source, count, advance) in [
        (vec![1, 8, 8, 9, 2, 3], 6, 2.73),
        (vec![1, 8, 20, 2, 3], 5, 2.71),
        (vec![1, 21, 2, 3], 4, 2.66),
    ] {
        let bound = b.bind(&bytes(&source)).unwrap();
        let same = bound.replace("Office").unwrap();
        assert_eq!(same, *bound.encoded());
        assert_eq!(same.text(), "Office");
        assert_eq!(same.bytes(), bytes(&source));
        assert_eq!(same.atoms().len(), count);
        assert!((same.advance() - advance).abs() < 0.00001);
        assert_eq!(
            same.byte_boundaries(),
            (0..=count).map(|i| i * 2).collect::<Vec<_>>()
        );
    }
}

#[test]
fn source_glyphs_growth_retains_ligature_width_count_and_unicode_boundaries() {
    let b = book(true);
    let bound = b.bind(&bytes(&[1, 21, 2, 3])).unwrap();
    let changed = bound.replace("Office this").unwrap();
    assert_eq!(codes(&changed), vec![1, 21, 2, 3, 4, 5, 6, 9, 7]);
    assert_eq!(changed.atoms()[1].unicode, 1..4);
    assert_eq!(changed.atoms().len(), 9); // Unicode scalar count is eleven.
    assert_eq!(changed.byte_boundaries().last(), Some(&18));
    assert_eq!(bound.encoded().bytes(), bytes(&[1, 21, 2, 3]));
    let mut end = 0;
    for atom in changed.atoms() {
        assert_eq!(atom.unicode.start, end);
        end = atom.unicode.end;
        assert_eq!(
            &changed.text()[atom.unicode.clone()],
            if atom.code == 21 {
                "ffi"
            } else {
                match atom.code {
                    1 => "O",
                    2 => "c",
                    3 => "e",
                    4 => " ",
                    5 => "t",
                    6 => "h",
                    9 => "i",
                    7 => "s",
                    _ => panic!(),
                }
            }
        );
    }
    assert_eq!(end, changed.text().len());
}

#[test]
fn source_glyphs_separated_edits_preserve_middle_and_suffix_atoms() {
    let b = book(true);
    let bound = b.bind(&bytes(&[1, 21, 2, 3, 4, 1, 8, 20, 2, 3])).unwrap();
    let changed = bound.replace("this Office this Office this").unwrap();
    assert_eq!(
        codes(&changed),
        vec![5, 6, 9, 7, 4, 1, 21, 2, 3, 4, 5, 6, 9, 7, 4, 1, 8, 20, 2, 3, 4, 5, 6, 9, 7]
    );
}

#[test]
fn source_glyphs_edit_inside_an_atom_needs_actual_replacement_glyphs() {
    let b = book(true);
    let bound = b.bind(&bytes(&[1, 21, 2, 3])).unwrap();
    let changed = bound.replace("Ofice").unwrap();
    assert_eq!(codes(&changed), vec![1, 8, 9, 2, 3]);
    let b = book(false);
    let bound = b.bind(&bytes(&[1, 21, 2, 3])).unwrap();
    assert_eq!(codes(&bound.replace("Ofice").unwrap()), vec![1, 20, 2, 3]); // fi is independently proved.
    assert!(bound.replace("Oice").is_err()); // scalar i is not present.
    assert_eq!(bound.replace("Office").unwrap(), *bound.encoded());
    assert!(bound.replace("Office this").is_err()); // missing scalar i, no invented glyph.
}

#[test]
fn source_glyphs_alignment_does_not_greedily_consume_a_short_prefix_atom() {
    let b = Arc::new(Codebook::new(vec![symbol(1, "a", 100.), symbol(2, "ab", 130.)], 1).unwrap());
    let bound = b.bind(&[1, 2]).unwrap(); // a + ab; Unicode aab.
    assert_eq!(bound.reuse("ab").unwrap(), vec![(1, 0)]); // retain two letters, not one.
    assert_eq!(codes(&bound.replace("ab").unwrap()), vec![2]);
    let bound = b.bind(&[1]).unwrap();
    assert!(bound.reuse("ab").unwrap().is_empty()); // retaining a strands unavailable b.
    assert_eq!(codes(&bound.replace("ab").unwrap()), vec![2]);
}

// Deliberately separate oracle: enumerate all source-atom subsequences and all
// placements, then recursively encode gaps. No production DP or encoder calls.
fn oracle_encodable(book: &Codebook, text: &str) -> bool {
    text.is_empty()
        || book.symbols.values().any(|s| {
            text.strip_prefix(&s.text)
                .is_some_and(|tail| oracle_encodable(book, tail))
        })
}
fn oracle_score(book: &Codebook, source: &[&str], text: &str) -> Option<usize> {
    let mut best = oracle_encodable(book, text).then_some(0);
    for (index, atom) in source.iter().enumerate() {
        for at in 0..=text.len() {
            if !text.is_char_boundary(at)
                || !text[at..].starts_with(atom)
                || !oracle_encodable(book, &text[..at])
            {
                continue;
            }
            if let Some(tail) = oracle_score(book, &source[index + 1..], &text[at + atom.len()..]) {
                let value = tail + atom.chars().count();
                best = Some(best.unwrap_or(0).max(value));
            }
        }
    }
    best
}
fn words(alphabet: &[char], limit: usize) -> Vec<String> {
    let mut words = vec![String::new()];
    let mut level = vec![String::new()];
    for _ in 0..limit {
        level = level
            .iter()
            .flat_map(|word| alphabet.iter().map(move |c| format!("{word}{c}")))
            .collect();
        words.extend(level.clone());
    }
    words
}
#[test]
fn source_glyphs_alignment_matches_exhaustive_independent_small_oracle() {
    for (texts, alphabet) in [
        (
            vec!["a", "b", "ab", "ba", "aba"],
            vec!['1', '2', '3', '4', '5'],
        ),
        (vec!["a", "ab", "ba"], vec!['1', '2', '3']),
    ] {
        let b = Arc::new(
            Codebook::new(
                texts
                    .iter()
                    .enumerate()
                    .map(|(i, text)| symbol(i as u16 + 1, text, 100. + i as f32 * 17.))
                    .collect(),
                1,
            )
            .unwrap(),
        );
        for source in words(&alphabet, 3) {
            let raw: Vec<_> = source.bytes().map(|b| b - b'0').collect();
            let bound = b.bind(&raw).unwrap();
            let source_text: Vec<_> = bound
                .encoded()
                .atoms()
                .iter()
                .map(|atom| &bound.encoded().text()[atom.unicode.clone()])
                .collect();
            for changed in words(&['a', 'b', 'x'], 4) {
                let expected = oracle_score(&b, &source_text, &changed);
                let actual = bound.reuse(&changed);
                match (expected,actual) {
                    (None,Err(_))=>assert!(bound.replace(&changed).is_err()),
                    (Some(expected),Ok(anchors))=>{
                        let actual:usize=anchors.iter().map(|(i,_)|source_text[*i].chars().count()).sum();
                        assert_eq!(actual,expected,"source={source_text:?}, new={changed:?}");
                        let encoded=bound.replace(&changed).unwrap();
                        assert_eq!(encoded.text(),changed);
                        assert_eq!(b.bind(encoded.bytes()).unwrap().encoded(),&encoded);
                        for (source,at) in anchors {
                            assert!(encoded.atoms().iter().any(|a|a.unicode.start==at
                                && a.code==bound.encoded().atoms()[source].code));
                        }
                    },
                    (expected,actual)=>panic!("oracle {expected:?}, actual {actual:?}, source={source_text:?}, new={changed:?}"),
                }
            }
        }
    }
}

#[test]
fn source_glyphs_full_text_budget_uses_a_bounded_alignment_table() {
    let b = Arc::new(Codebook::new(vec![symbol(1, "a", 100.), symbol(2, "b", 200.)], 1).unwrap());
    let mut source = vec![1; 2048];
    source.extend(vec![2; 2048]);
    let bound = b.bind(&source).unwrap();
    let text = "b".repeat(2048) + &"a".repeat(2048);
    let anchors = bound.reuse(&text).unwrap();
    assert_eq!(anchors.len(), 2048);
    let changed = bound.replace(&text).unwrap();
    assert_eq!(changed.text(), text);
    assert_eq!(changed.atoms().len(), 4096);
    assert_eq!(MAX_CELLS * std::mem::size_of::<u16>(), 33_570_818);
}

#[test]
fn source_glyphs_sequence_count_is_bounded_before_retention() {
    let symbols = (1..=513)
        .map(|code| {
            symbol(
                code,
                &format!("a{}", char::from_u32(0x400 + u32::from(code)).unwrap()),
                100.,
            )
        })
        .collect();
    assert!(Codebook::new(symbols, 2).is_err());
    let symbols = (1..=512)
        .map(|code| {
            symbol(
                code,
                &format!("a{}", char::from_u32(0x400 + u32::from(code)).unwrap()),
                100.,
            )
        })
        .collect();
    assert!(Codebook::new(symbols, 2).is_ok());
}

#[test]
fn source_glyphs_encoding_uses_a_complete_segmentation_not_a_greedy_dead_end() {
    let b = Codebook::new(
        vec![
            symbol(1, "a", 10.),
            symbol(2, "ab", 20.),
            symbol(3, "bc", 30.),
            symbol(4, "x", 10.),
            symbol(5, "xy", 20.),
            symbol(6, "xyz", 30.),
            symbol(7, "zq", 20.),
        ],
        1,
    )
    .unwrap();
    assert_eq!(codes(&b.encode("abc").unwrap()), vec![1, 3]);
    assert_eq!(codes(&b.encode("xyzq").unwrap()), vec![5, 7]);
    assert!(b.encode("abcd").is_err());
}

#[test]
fn source_glyphs_prefer_scalars_for_new_text_but_never_decompose_source_noop() {
    let b = book(true);
    assert_eq!(codes(&b.encode("ffi").unwrap()), vec![8, 8, 9]);
    assert_eq!(
        codes(&b.bind(&bytes(&[21])).unwrap().replace("ffi").unwrap()),
        vec![21]
    );
    assert_eq!(codes(&book(false).encode("ffi").unwrap()), vec![21]);
}

#[test]
fn source_glyphs_invalid_books_and_source_bytes_are_refused() {
    for (symbols, length) in [
        (vec![], 2),
        (vec![symbol(1, "a", 10.)], 0),
        (vec![symbol(256, "a", 10.)], 1),
        (vec![symbol(1, "a", 10.), symbol(1, "b", 10.)], 2),
        (vec![symbol(1, "a", 10.), symbol(2, "a", 10.)], 2),
        (vec![symbol(1, "", 10.)], 2),
        (vec![symbol(1, "a\nb", 10.)], 2),
        (vec![symbol(1, "a b", 10.)], 2),
        (vec![symbol(1, "a", f32::NAN)], 2),
        (vec![symbol(1, "a", f32::INFINITY)], 2),
        (vec![symbol(1, "a", -1.)], 2),
        (vec![symbol(1, "a", 10_001.)], 2),
        (vec![symbol(1, &"a".repeat(17), 10.)], 2),
        (
            vec![Symbol {
                gid: 0,
                ..symbol(1, "a", 10.)
            }],
            2,
        ),
    ] {
        assert!(Codebook::new(symbols, length).is_err());
    }
    let b = book(true);
    assert!(b.bind(&[0]).is_err());
    assert!(b.bind(&bytes(&[65535])).is_err());
    assert!(b.bind(&vec![0; 8194]).is_err());
    assert!(b.encode(&"O".repeat(4097)).is_err());
    assert!(b.bind(&bytes(&vec![21; 1366])).is_err()); // expansion exceeds UTF-8 limit.
    assert!(b.bind(&bytes(&vec![1; 4096])).is_ok());
}

#[test]
fn source_glyphs_empty_and_supplementary_unicode_keep_byte_ranges() {
    let b = Arc::new(
        Codebook::new(
            vec![
                symbol(1, "é", 300.),
                symbol(2, "🦀", 900.),
                symbol(3, "é🦀", 1000.),
            ],
            2,
        )
        .unwrap(),
    );
    let bound = b.bind(&bytes(&[3])).unwrap();
    assert_eq!(bound.encoded().atoms()[0].unicode, 0..6);
    let changed = bound.replace("🦀é🦀é").unwrap();
    assert_eq!(codes(&changed), vec![2, 3, 1]);
    assert_eq!(
        changed
            .atoms()
            .iter()
            .map(|a| a.unicode.clone())
            .collect::<Vec<_>>(),
        vec![0..4, 4..10, 10..12]
    );
    let empty = bound.replace("").unwrap();
    assert!(empty.atoms().is_empty());
    assert_eq!(empty.byte_boundaries(), vec![0]);
    assert_eq!(empty.advance(), 0.);
    assert_eq!(
        b.bind(&[]).unwrap().replace("é").unwrap().bytes(),
        bytes(&[1])
    );
}

#[test]
fn source_glyphs_required_boundaries_decompose_only_through_proved_codes() {
    let b = book(true);
    let bound = b.bind(&bytes(&[1, 21, 2, 3])).unwrap();
    let original = bound.encoded().clone();
    let changed = bound.replace_separated("Office", &[2]).unwrap();
    assert_eq!(changed.text(), "Office");
    assert!(changed
        .atoms()
        .iter()
        .all(|a| !(a.unicode.start < 2 && a.unicode.end > 2)));
    assert_eq!(codes(&changed), vec![1, 8, 8, 9, 2, 3]);
    assert_eq!(bound.encoded(), &original);
    assert_eq!(
        bound.replace_separated("Office", &[1, 4]).unwrap(),
        original
    );
    let b = book(false);
    let restricted = b.bind(&bytes(&[1, 21, 2, 3])).unwrap();
    assert!(restricted.replace_separated("Office", &[2]).is_err());
    assert!(restricted.replace_separated("Office", &[3]).is_err());
    assert_eq!(
        restricted.replace_separated("Office", &[4]).unwrap(),
        *restricted.encoded()
    );
}

#[test]
fn source_glyphs_required_boundaries_constrain_new_gaps_and_repeated_source_atoms() {
    let b = book(false);
    let bound = b.bind(&bytes(&[1])).unwrap();
    assert!(bound.replace("O ffi").is_ok());
    assert!(bound.replace_separated("O ffi", &[3]).is_err());
    let b = book(true);
    let repeated = b.bind(&bytes(&[21, 21])).unwrap();
    let changed = repeated.replace_separated("ffiffi", &[1]).unwrap();
    assert_eq!(changed.text(), "ffiffi");
    assert_eq!(codes(&changed), vec![8, 8, 9, 21]);
    assert_eq!(changed.atoms().iter().filter(|a| a.code == 21).count(), 1);
    let anchors = repeated.reuse_separated("ffiffi", &[1]).unwrap();
    assert_eq!(
        anchors
            .iter()
            .map(|(i, _)| repeated.encoded().atoms()[*i].unicode.len())
            .sum::<usize>(),
        3
    );
}

#[test]
fn source_glyphs_required_boundaries_use_utf8_and_reject_malformed_limits() {
    let b = Arc::new(
        Codebook::new(
            vec![
                symbol(1, "éfi", 800.),
                symbol(2, "é", 400.),
                symbol(3, "fi", 610.),
            ],
            1,
        )
        .unwrap(),
    );
    let bound = b.bind(&[1]).unwrap();
    assert_eq!(
        codes(&bound.replace_separated("éfi", &[2]).unwrap()),
        vec![2, 3]
    );
    for boundaries in [
        vec![0],
        vec![1],
        vec![4],
        vec![5],
        vec![2, 2],
        vec![3, 2],
        vec![2; 4096],
    ] {
        assert!(
            bound.replace_separated("éfi", &boundaries).is_err(),
            "{boundaries:?}"
        );
    }
    assert_eq!(
        bound.replace_separated("éfi", &[]).unwrap(),
        *bound.encoded()
    );
}

// Independent recursive reference enumerates placements and encodes gaps.
// Its predicate scans every required byte boundary, rather than sharing the
// production path/alignment helpers or their binary-search logic.
fn separated_oracle_encodable(
    book: &Codebook,
    text: &str,
    boundaries: &[usize],
    base: usize,
) -> bool {
    text.is_empty()
        || book.symbols.values().any(|symbol| {
            let end = base + symbol.text.len();
            !boundaries.iter().any(|b| *b > base && *b < end)
                && text
                    .strip_prefix(&symbol.text)
                    .is_some_and(|tail| separated_oracle_encodable(book, tail, boundaries, end))
        })
}
fn separated_oracle_score(
    book: &Codebook,
    source: &[&str],
    text: &str,
    boundaries: &[usize],
    base: usize,
) -> Option<usize> {
    let mut best = separated_oracle_encodable(book, text, boundaries, base).then_some(0);
    for (index, atom) in source.iter().enumerate() {
        for at in 0..=text.len() {
            let end = base + at + atom.len();
            if !text.is_char_boundary(at)
                || !text[at..].starts_with(atom)
                || boundaries.iter().any(|b| *b > base + at && *b < end)
                || !separated_oracle_encodable(book, &text[..at], boundaries, base)
            {
                continue;
            }
            if let Some(tail) = separated_oracle_score(
                book,
                &source[index + 1..],
                &text[at + atom.len()..],
                boundaries,
                end,
            ) {
                best = Some(best.unwrap_or(0).max(tail + atom.chars().count()));
            }
        }
    }
    best
}

#[test]
fn source_glyphs_required_boundaries_match_independent_small_oracle() {
    let mut cases = 0;
    for symbols in [vec!["a", "b", "ab", "ba"], vec!["a", "ab", "ba"]] {
        let b = Arc::new(
            Codebook::new(
                symbols
                    .iter()
                    .enumerate()
                    .map(|(i, s)| symbol(i as u16 + 1, s, 100.))
                    .collect(),
                1,
            )
            .unwrap(),
        );
        let alphabet: Vec<_> = (0..symbols.len())
            .map(|i| char::from(b'1' + i as u8))
            .collect();
        for source in words(&alphabet, 2) {
            let bound = b
                .bind(&source.bytes().map(|c| c - b'0').collect::<Vec<_>>())
                .unwrap();
            let old: Vec<_> = bound
                .encoded()
                .atoms()
                .iter()
                .map(|a| &bound.encoded().text()[a.unicode.clone()])
                .collect();
            for text in words(&['a', 'b', 'x'], 3) {
                for mask in 0..1usize << text.len().saturating_sub(1) {
                    let boundaries: Vec<_> = (1..text.len())
                        .filter(|i| mask & (1 << (i - 1)) != 0)
                        .collect();
                    let expected = separated_oracle_score(&b, &old, &text, &boundaries, 0);
                    let actual = bound.reuse_separated(&text, &boundaries);
                    match (expected, actual) {
                        (None, Err(_)) => assert!(bound.replace_separated(&text, &boundaries).is_err()),
                        (Some(score), Ok(anchors)) => {
                            assert_eq!(anchors.iter().map(|(i, _)| old[*i].chars().count()).sum::<usize>(), score,
                                "old={old:?}, text={text:?}, boundaries={boundaries:?}");
                            let changed = bound.replace_separated(&text, &boundaries).unwrap();
                            assert_eq!(changed.text(), text);
                            assert!(changed.atoms().iter().all(|a| !boundaries.iter().any(|b| *b > a.unicode.start && *b < a.unicode.end)));
                            assert_eq!(b.bind(changed.bytes()).unwrap().encoded(), &changed);
                        },
                        (expected, actual) => panic!("old={old:?}, text={text:?}, boundaries={boundaries:?}, expected={expected:?}, actual={actual:?}"),
                    }
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 4420);
}
