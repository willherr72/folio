//! Dependency conformance is deliberately separate from Folio's admitted-input
//! policy. These fixtures include controls/scripts the shaping API refuses.
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};
use unicode_bidi::{BidiClass, BidiInfo, Level};
use unicode_segmentation::UnicodeSegmentation;

fn fixture(name: &str) -> String {
    fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/shaped-text/unicode")
            .join(name),
    )
    .unwrap()
}

#[test]
fn unicode16_bidi_character_levels_and_visual_order_match() {
    assert_eq!(unicode_bidi::UNICODE_VERSION, (16, 0, 0));
    let full = std::env::var_os("FOLIO_FULL_BIDI_TEST");
    let corpus = if let Some(path) = &full {
        let bytes = fs::read(path).unwrap();
        let hash: String = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(
            hash, "d04a51a90052dcd71c4e91ee5b3a9d973ee35c12406b5a99875ac8163c8f2804",
            "The full dev corpus must match the pinned Unicode16 source."
        );
        String::from_utf8(bytes).unwrap()
    } else {
        fixture("BidiCharacterTest-16.0.0-sample.txt")
    };
    let mut count = 0;
    for (line_number, line) in corpus.lines().enumerate() {
        let line = line.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<_> = line.split(';').collect();
        assert_eq!(fields.len(), 5);
        let text: String = fields[0]
            .split_whitespace()
            .map(|hex| char::from_u32(u32::from_str_radix(hex, 16).unwrap()).unwrap())
            .collect();
        let base = match fields[1] {
            "0" => Some(Level::ltr()),
            "1" => Some(Level::rtl()),
            "2" => None,
            _ => panic!("invalid fixture direction"),
        };
        let bidi = BidiInfo::new(&text, base);
        assert_eq!(bidi.paragraphs.len(), 1);
        let paragraph = &bidi.paragraphs[0];
        assert_eq!(
            paragraph.level.number(),
            fields[2].parse::<u8>().unwrap(),
            "paragraph level at fixture line{}",
            line_number + 1
        );
        let expected_levels: Vec<_> = fields[3]
            .split_whitespace()
            .map(|n| {
                if n == "x" {
                    None
                } else {
                    Some(n.parse::<u8>().unwrap())
                }
            })
            .collect();
        let levels = bidi.reordered_levels_per_char(paragraph, 0..text.len());
        assert_eq!(levels.len(), expected_levels.len());
        let mut retained = Vec::new();
        for (index, (byte, _)) in text.char_indices().enumerate() {
            let removed = matches!(
                bidi.original_classes[byte],
                BidiClass::LRE
                    | BidiClass::RLE
                    | BidiClass::LRO
                    | BidiClass::RLO
                    | BidiClass::PDF
                    | BidiClass::BN
            );
            assert_eq!(
                removed,
                expected_levels[index].is_none(),
                "X9 at fixture line{}",
                line_number + 1
            );
            if let Some(expected) = expected_levels[index] {
                assert_eq!(
                    levels[index].number(),
                    expected,
                    "level at fixture line{}, scalar{index}",
                    line_number + 1
                );
                retained.push(index);
            }
        }
        let filtered: Vec<_> = retained.iter().map(|index| levels[*index]).collect();
        let visual: Vec<_> = BidiInfo::reorder_visual(&filtered)
            .iter()
            .map(|index| retained[*index])
            .collect();
        let expected_order: Vec<usize> = fields[4]
            .split_whitespace()
            .map(|n| n.parse().unwrap())
            .collect();
        assert_eq!(
            visual,
            expected_order,
            "visual order at fixture line{}",
            line_number + 1
        );
        count += 1;
    }
    assert_eq!(
        count,
        if full.is_some() { 91707 } else { 286 },
        "Keep the exact provenance-recorded corpus size visible."
    );
    eprintln!(
        "Unicode16 bidi corpus: {count} rows ({})",
        if full.is_some() {
            "complete BidiCharacterTest"
        } else {
            "sampled"
        }
    );
}

#[test]
fn unicode17_complete_grapheme_break_corpus_matches() {
    assert_eq!(unicode_segmentation::UNICODE_VERSION, (17, 0, 0));
    let corpus = fixture("GraphemeBreakTest-17.0.0.txt");
    let mut count = 0;
    for (line_number, line) in corpus.lines().enumerate() {
        let line = line.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        let mut text = String::new();
        let mut expected = Vec::new();
        for token in line.split_whitespace() {
            match token {
                "÷" => expected.push(text.len()),
                "×" => {}
                hex => text.push(char::from_u32(u32::from_str_radix(hex, 16).unwrap()).unwrap()),
            }
        }
        let actual: Vec<_> = text
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .chain(std::iter::once(text.len()))
            .collect();
        assert_eq!(
            actual,
            expected,
            "grapheme boundaries at fixture line{}",
            line_number + 1
        );
        count += 1;
    }
    assert!(
        count > 700,
        "Expected the complete versioned grapheme corpus."
    );
    eprintln!("Unicode17 grapheme corpus: {count} rows");
}
