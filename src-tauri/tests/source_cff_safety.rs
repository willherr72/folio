//! CFF structure invariants required before a font can authorize text edits.
use ttf_parser::{cff::Table,GlyphId};

#[test]
fn public_cid_fixture_preserves_nonidentity_character_ids() {
    let table=Table::parse(include_bytes!("../../tests/fixtures/source-cff/cid.cff")).unwrap();
    assert_eq!(table.glyph_cid(GlyphId(0)),Some(0));
    assert_eq!(table.glyph_cid(GlyphId(2)),Some(17));
    assert_ne!(table.glyph_cid(GlyphId(2)),Some(2));
}

#[test]
fn malformed_fdselect_cannot_omit_the_first_glyph() {
    assert!(Table::parse(include_bytes!("../../tests/fixtures/source-cff/invalid-fdselect-start.cff")).is_none(),
        "A CID program whose FDSelect begins at GID1 cannot prove GID0 or its dictionary ownership");
}

#[test]
fn original_sid_width_api_keeps_its_integral_contract() {
    let table=Table::parse(include_bytes!("../../tests/fixtures/source-cff/fractional.cff")).unwrap();
    assert_eq!(table.glyph_width(GlyphId(2)),Some(500));
}
#[test]
fn exact_predefined_charset_resolves_standard_sids() {
    // Literal CFF: omitted charset selects ISOAdobe; default encoding is
    // StandardEncoding. Glyph34 is A, independent of Unicode or glyph names.
    let mut raw=vec![1,0,4,4,0,1,1,1,4,b'I',b'S',b'O',0,1,1,1,7,29,0,0,0,27,17,0,0,0,0,0,229,1];
    raw.extend(1..=230u8);raw.extend(vec![14;229]);
    let table=ttf_parser::cff::Table::parse(&raw).unwrap();
    assert_eq!(table.glyph_index_exact(32),Some(ttf_parser::GlyphId(1)));
    assert_eq!(table.glyph_index_exact(65),Some(ttf_parser::GlyphId(34)));
}

