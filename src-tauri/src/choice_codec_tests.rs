use super::{batches, verify_rows};
use crate::choice_font::ChoiceFontProof;
use lopdf::{
    content::{Content, Operation},
    dictionary, Document, Object,
};
use std::collections::BTreeMap;

fn proof() -> ChoiceFontProof {
    let d = Document::new();
    ChoiceFontProof::for_texts(&d,&dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica","Encoding"=>"WinAnsiEncoding"}.into(),&[]).unwrap()
}
fn names() -> BTreeMap<Vec<u8>, Vec<u8>> {
    [(b"F1".to_vec(), b"F1".to_vec())].into()
}
fn content(bytes: &[u8]) -> Content {
    Content::decode_strict(bytes).unwrap()
}

#[test]
fn decoded_rows_preserve_spaces_empty_labels_and_split_native_strings() {
    let c = content(b"BT /F1 12 Tf (Caf) Tj <e9> Tj ET BT ET BT /F1 12 Tf (  B  ) Tj ET");
    verify_rows(&c, &proof(), &names(), &["Café", "", "  B  "]).unwrap();
    verify_rows(&content(b""), &proof(), &names(), &[]).unwrap();
}

#[test]
fn lost_changed_extra_or_reordered_rows_are_not_equivalent() {
    for (c, expected) in [
        (b"BT /F1 12 Tf (A) Tj ET".as_slice(), vec!["Alpha"]),
        (b"BT /F1 12 Tf (B) Tj ET", vec!["A"]),
        (b"BT /F1 12 Tf (A) Tj ET BT ET", vec!["A"]),
        (b"BT ET", vec!["A"]),
        (b"", vec![""]),
        (
            b"BT /F1 12 Tf (B) Tj ET BT /F1 12 Tf (A) Tj ET",
            vec!["A", "B"],
        ),
    ] {
        assert!(verify_rows(&content(c), &proof(), &names(), &expected).is_err());
    }
}

#[test]
fn unknown_and_incomplete_cid_codes_are_refused() {
    let (d, font, _) = crate::choice_font_tests::cid_for(&['Ω']);
    let p = ChoiceFontProof::for_texts(&d, &font, &["Ω"]).unwrap();
    let codes = p.encode("Ω").unwrap();
    let make = |bytes: Vec<u8>| Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            Operation::new("Tj", vec![Object::string_literal(bytes)]),
            Operation::new("ET", vec![]),
        ],
    };
    verify_rows(&make(codes.clone()), &p, &names(), &["Ω"]).unwrap();
    for bytes in [codes[..1].to_vec(), vec![255, 255]] {
        assert!(verify_rows(&make(bytes), &p, &names(), &["Ω"]).is_err());
    }
    assert!(verify_rows(
        &content(b"BT /F1 12 Tf <00> Tj ET"),
        &proof(),
        &names(),
        &["A"]
    )
    .is_err());
}

#[test]
fn native_font_binding_and_text_object_boundaries_are_required() {
    for bytes in [
        b"BT /F2 12 Tf (A) Tj ET".as_slice(),
        b"BT (A) Tj ET",
        b"BT /F1 0 Tf (A) Tj ET",
        b"BT /F1 -1 Tf (A) Tj ET",
        b"BT /F1 Tf (A) Tj ET",
        b"BT /F1 12 Tf BT (A) Tj ET ET",
        b"BT /F1 12 Tf (A) Tj",
        b"/F1 12 Tf (A) Tj",
        b"BT /F1 12 Tf [(A)] TJ ET",
        b"BT /F1 12 Tf q (A) Tj Q ET",
    ] {
        assert!(
            verify_rows(&content(bytes), &proof(), &names(), &["A"]).is_err(),
            "{bytes:?}"
        );
    }
    let mut c = content(b"BT /F1 12 Tf (A) Tj ET");
    c.operations[1].operands[1] = Object::Real(f32::NAN);
    assert!(verify_rows(&c, &proof(), &names(), &["A"]).is_err());
}

#[test]
fn extra_text_and_operation_budgets_are_checked_before_decoding() {
    let c = Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            Operation::new("Tj", vec![Object::string_literal(vec![b'A'; 8193])]),
            Operation::new("ET", vec![]),
        ],
    };
    assert!(verify_rows(&c, &proof(), &names(), &["A"]).is_err());
    let c = Content {
        operations: vec![Operation::new("q", vec![]); 16385],
    };
    assert!(verify_rows(&c, &proof(), &names(), &[]).is_err());
    assert!(verify_rows(
        &content(b"BT /F1 12 Tf <e9e9> Tj ET"),
        &proof(),
        &names(),
        &["é"]
    )
    .is_err());
}

#[test]
fn batches_bound_each_native_field_without_losing_or_reordering_labels() {
    let p = proof();
    let full = "A".repeat(4096);
    for texts in [
        vec![full.as_str(); 32],
        vec![""; 4001],
        vec![" Alpha "; 2000],
        vec![],
    ] {
        let groups = batches(&p, &texts).unwrap();
        assert!(groups.len() <= 128);
        let mut seen = Vec::new();
        for group in groups {
            assert!(!group.is_empty() && group.len() <= 128);
            assert!(texts[group.clone()].iter().map(|s| s.len()).sum::<usize>() <= 4096);
            seen.extend(group);
        }
        assert_eq!(seen, (0..texts.len()).collect::<Vec<_>>());
    }
    assert!(batches(&p, &vec![""; 4002]).is_err());
    assert!(batches(&p, &vec![full.as_str(); 33]).is_err());
    assert!(batches(&p, &["A".repeat(4097).as_str()]).is_err());
}
