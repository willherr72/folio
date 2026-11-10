//! Mapping authority and structural limits, with hand-derived UTF-16 cases.
use super::*;

fn editing(body: &str) -> EngineResult<BTreeMap<u16, Option<char>>> {
    cmap_with_policy(body.as_bytes(), 2, CMapPolicy::LatinEditing)
}

const SPACE: &str = "1 begincodespacerange <0000> <FFFF> endcodespacerange ";

// Literal output from the historical writer, independent of the current writer.
const LEGACY_FOLIO_CMAP: &[u8] = b"/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /FolioUnicode def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n1 beginbfrange\n<0000> <FFFF> <0000>\nendbfrange\nendcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n";

#[test]
fn equivalent_font_resources_merge_operand_and_encoding_requirements_in_either_order() {
    use lopdf::{dictionary, Stream};
    let asset = crate::fonts::FontAsset::parse(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec()).unwrap();
    for paragraph in [false, true] { for reverse in [false, true] {
        let mut pdf = Document::with_version("1.7"); let mut name = None;
        let mut kinds = vec!["current", "legacy", "partial"];
        if reverse { kinds.reverse(); }
        for kind in kinds {
            let resource = if paragraph {
                super::super::persistence::install_paragraph_font_resource(&mut pdf, &asset)
            } else { super::super::persistence::install_font_resource(&mut pdf, &asset) }.unwrap();
            let id = resource.as_reference().unwrap();
            let font = pdf.get_dictionary(id).unwrap();
            name = Some(renderer_name(&pdf, font).unwrap());
            let mut unicode = if kind == "legacy" { LEGACY_FOLIO_CMAP.to_vec() } else {
                pdf.dereference(font.get(b"ToUnicode").unwrap()).unwrap().1.as_stream().unwrap().get_plain_content().unwrap()
            };
            if kind == "partial" {
                unicode = String::from_utf8(unicode).unwrap().replace("endcmap", "1 beginbfchar <D800> <D800> endbfchar\nendcmap").into_bytes();
            }
            let unicode = pdf.add_object(Stream::new(dictionary!{}, unicode));
            pdf.get_dictionary_mut(id).unwrap().set("ToUnicode", unicode);
        }
        let mut bytes = vec![]; pdf.save_to(&mut bytes).unwrap();
        let proofs = FontProofs::read(&bytes).unwrap();
        assert_eq!(proofs.programs.len(), 1);
        assert!(proofs.programs.values().next().unwrap().iter().all(Result::is_ok));
        let proof = proofs.find(name.as_ref().unwrap(), &asset.info.id).expect("Equivalent mappings must not be rejected for different routing requirements");
        assert!(proof.requires_source_operands && proof.preserve_source_encoding);
        assert_eq!(proof.decode(&proof.encode("Résumé café — €").unwrap()).unwrap(), "Résumé café — €");
    }}
}

#[test]
fn different_verified_character_codes_remain_ambiguous_in_either_order() {
    use lopdf::{dictionary, Stream};
    let asset = crate::fonts::FontAsset::parse(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec()).unwrap();
    for reverse in [false, true] {
        let mut pdf = Document::with_version("1.7"); let mut name = None;
        for shifted in if reverse {[true, false]} else {[false, true]} {
            let resource = super::super::persistence::install_font_resource(&mut pdf, &asset).unwrap();
            let id = resource.as_reference().unwrap(); let mut font = pdf.get_dictionary(id).unwrap().clone();
            name = Some(renderer_name(&pdf, &font).unwrap());
            let code = if shifted {1} else {65};
            let unicode = pdf.add_object(Stream::new(dictionary!{}, format!("{SPACE}1 beginbfchar <{code:04X}> <0041> endbfchar").into_bytes()));
            font.set("ToUnicode", unicode);
            let child_id = font.get(b"DescendantFonts").unwrap().as_array().unwrap()[0].as_reference().unwrap();
            let mut mapping = vec![0; 65536 * 2];
            let face = asset.face().unwrap(); let glyph = face.glyph_index('A').unwrap();
            mapping[code * 2..code * 2 + 2].copy_from_slice(&glyph.0.to_be_bytes());
            let mapping = pdf.add_object(Stream::new(dictionary!{}, mapping));
            let child = pdf.get_dictionary_mut(child_id).unwrap();
            child.set("CIDToGIDMap", mapping);
            child.set("W", vec![Object::Integer(code as i64), Object::Array(vec![Object::Real(face.glyph_hor_advance(glyph).unwrap() as f32 * 1000. / face.units_per_em() as f32)])]);
            assert!(verify(&pdf, &font, &asset.bytes).is_ok());
            pdf.objects.insert(id, font.into());
        }
        let mut bytes = vec![]; pdf.save_to(&mut bytes).unwrap();
        let proofs = FontProofs::read(&bytes).unwrap();
        assert_eq!(proofs.programs.len(), 1);
        assert!(proofs.programs.values().next().unwrap().iter().all(Result::is_ok));
        assert!(proofs.find(name.as_ref().unwrap(), &asset.info.id).unwrap_err().contains("ambiguous PDF font mappings"));
    }
}

#[test]
fn legacy_folio_encoding_requires_the_complete_historical_resource() {
    use lopdf::{dictionary, Stream};
    let asset = crate::fonts::FontAsset::parse(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec()).unwrap();
    for paragraph in [false, true] { for mutation in ["none", "name", "width", "map"] {
        let mut pdf = Document::with_version("1.7");
        let resource = if paragraph {
            super::super::persistence::install_paragraph_font_resource(&mut pdf, &asset)
        } else { super::super::persistence::install_font_resource(&mut pdf, &asset) }.unwrap();
        let mut font = pdf.dereference(&resource).unwrap().1.as_dict().unwrap().clone();
        let mut bytes = LEGACY_FOLIO_CMAP.to_vec();
        if mutation == "map" { bytes.push(b' '); }
        font.set("ToUnicode", Object::Stream(Stream::new(dictionary!{}, bytes)));
        if mutation == "name" { font.set("BaseFont", "FolioUnproved"); }
        if mutation == "width" {
            let child_id = font.get(b"DescendantFonts").unwrap().as_array().unwrap()[0].as_reference().unwrap();
            pdf.get_dictionary_mut(child_id).unwrap().set("DW", 1);
        }
        let proof = verify(&pdf, &font, &asset.bytes).unwrap();
        let encoded = proof.encode("Résumé café — €");
        if mutation == "none" {
            assert_eq!(proof.decode(&encoded.expect("Historical Folio encoding remains editable")).unwrap(), "Résumé café — €");
            assert!(!proof.requires_source_operands);
        } else { assert!(encoded.is_err(), "{mutation} must not authorize the historical encoding"); }
        assert!(choice_font_codes(&pdf, &font).is_err(), "Complete form proof remains strict");
    }}
}

#[test]
fn imported_font_writers_produce_complete_unicode_maps_for_latin_extensions() {
    let asset = crate::fonts::FontAsset::parse(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec()).unwrap();
    for paragraph in [false, true] {
        let mut pdf = Document::with_version("1.7");
        let resource = if paragraph {
            super::super::persistence::install_paragraph_font_resource(&mut pdf, &asset)
        } else {
            super::super::persistence::install_font_resource(&mut pdf, &asset)
        }.unwrap();
        let font = pdf.dereference(&resource).unwrap().1.as_dict().unwrap();
        let stream = pdf.dereference(font.get(b"ToUnicode").unwrap()).unwrap().1.as_stream().unwrap();
        let map = cmap(&stream.get_plain_content().unwrap(), 2).expect("Writer must emit a complete valid CMap");
        for code in [0x41, 0xe9, 0xff, 0x100, 0x2014, 0x20ac] {
            assert_eq!(map.get(&code), Some(&(code as u32)));
        }
        assert!(!map.keys().any(|c| (0xd800..=0xdfff).contains(c)));
        let proof = verify(&pdf, font, &asset.bytes).unwrap();
        assert!(!proof.requires_source_operands);
        let text = "Résumé café — €";
        assert_eq!(proof.decode(&proof.encode(text).unwrap()).unwrap(), text);
    }
}

#[test]
fn unused_unusable_strings_have_no_authority_but_keep_their_codes() {
    let body = format!("{SPACE}6 beginbfchar <0001> <0041> <0002> <00410042> <0003> <D800> <0004> <01F600> <0005> <> <0006> <D83DDE00> endbfchar");
    let actual = editing(&body).unwrap();
    assert_eq!(actual, BTreeMap::from([
        (1, Some('A')), (2, None), (3, None), (4, None), (5, None), (6, Some('\u{1f600}')),
    ]));
    assert!(cmap(body.as_bytes(), 2).is_err(), "Complete repertoire stays strict");
}

#[test]
fn ranges_increment_encoded_utf16_without_crossing_surrogate_gaps() {
    let body = format!("{SPACE}4 beginbfrange <0001> <0003> <D83DDFFF> <0010> <0012> <D7FF> <0020> <0021> <FFFF> <0030> <0032> <00410041> endbfrange");
    let actual = editing(&body).unwrap();
    assert_eq!(actual, BTreeMap::from([
        (1, Some('\u{1f7ff}')), (2, None), (3, None),
        (0x10, Some('\u{d7ff}')), (0x11, None), (0x12, None),
        (0x20, Some('\u{ffff}')), (0x21, None),
        (0x30, None), (0x31, None), (0x32, None),
    ]));
    assert!(cmap(body.as_bytes(), 2).is_err());
}

#[test]
fn complete_ranges_do_not_reinterpret_surrogate_overflow_as_scalars() {
    let body = format!("{SPACE}1 beginbfrange <0001> <0002> <D83DDFFF> endbfrange");
    assert!(cmap(body.as_bytes(), 2).is_err());
}

#[test]
fn arrays_are_only_range_destinations() {
    let bad = format!("{SPACE}1 beginbfchar <0001> [<0041>] endbfchar");
    assert!(editing(&bad).is_err());
    assert!(cmap(bad.as_bytes(), 2).is_err());
    let good = format!("{SPACE}1 beginbfrange <0001> <0003> [<0041> <D800> <0042>] endbfrange");
    assert_eq!(editing(&good).unwrap(), BTreeMap::from([(1, Some('A')), (2, None), (3, Some('B'))]));
}

#[test]
fn excluded_codes_still_conflict_in_every_definition_order() {
    for entries in [
        "<0001> <D800> <0001> <0041>",
        "<0001> <0041> <0001> <D800>",
        "<0001> <D800> <0001> <D800>",
        "<0001> <0041> <0001> <0041>",
    ] {
        assert!(editing(&format!("{SPACE}2 beginbfchar {entries} endbfchar")).is_err());
    }
    assert!(editing("1 begincodespacerange <0000> <0001> endcodespacerange 1 beginbfchar <0002> <D800> endbfchar").is_err());
}

#[test]
fn excluding_destinations_does_not_relax_structural_admission() {
    for body in [
        "2 beginbfchar <0001> <0041> endbfchar",
        "1 beginbfchar <0001> /Invalid endbfchar",
        "1 beginbfrange <0001> <0002> [<0041>] endbfrange",
        "1 beginbfrange <0001> <0002> [<0041> null] endbfrange",
        "1 beginbfrange <0002> <0001> <0041> endbfrange",
        "1 beginbfchar <01> <0041> endbfchar",
        "1 beginbfchar <0001> <0041> endbfrange",
        "1 beginbfchar <0001> <0041> pop endbfchar",
        "/Inherited usecmap 1 beginbfchar <0001> <0041> endbfchar",
        "65537 beginbfchar endbfchar",
        "1 beginbfchar <0001> <0041>",
    ] {
        assert!(editing(&format!("{SPACE}{body}")).is_err(), "{body}");
    }
}

#[test]
fn excluded_full_domain_is_bounded_and_retains_overlap_detection() {
    let body = format!("{SPACE}1 beginbfrange <0000> <FFFF> <> endbfrange");
    let map = editing(&body).unwrap();
    assert_eq!(map.len(), 65536);
    assert!(map.values().all(Option::is_none));
    assert!(editing(&format!("{body}1 beginbfchar <0000> <0041> endbfchar")).is_err());
}

#[test]
fn destination_increment_never_carries_into_the_previous_byte() {
    let body = format!("{SPACE}1 beginbfrange <0001> <0003> <00FE> endbfrange");
    assert_eq!(editing(&body).unwrap(), BTreeMap::from([(1, Some('\u{fe}')), (2, Some('\u{ff}')), (3, None)]));
    assert!(cmap(body.as_bytes(), 2).is_err());
}

fn composite(unusable: bool, identity: bool) -> (Document, Dictionary, &'static [u8]) {
    use lopdf::{dictionary, Stream};
    let bytes = include_bytes!("../../tests/fixtures/embedded-text/DejaVuSerif-subset.ttf");
    let face = ttf_parser::Face::parse(bytes, 0).unwrap();
    let mut pdf = Document::with_version("1.7");
    let file = pdf.add_object(Stream::new(dictionary!{}, bytes.to_vec()));
    let mut entries = String::new();
    let mut widths = vec![];
    for c in ['O', 'l', 'd'] {
        let gid = face.glyph_index(c).unwrap();
        entries.push_str(&format!("<{:04X}> <{:04X}> ", gid.0, c as u32));
        widths.extend([Object::Integer(gid.0 as i64), Object::Array(vec![Object::Real(
            face.glyph_hor_advance(gid).unwrap() as f32 * 1000. / face.units_per_em() as f32)])]);
    }
    if unusable { entries.push_str("<FFFE> <D800> "); }
    let map = pdf.add_object(Stream::new(dictionary!{}, format!("{SPACE}{} beginbfchar {entries}endbfchar", if unusable {4} else {3}).into_bytes()));
    let mut child = dictionary! {"Subtype"=>"CIDFontType2", "W"=>widths,
        "FontDescriptor"=>dictionary!{"FontFile2"=>file}};
    if identity { child.set("CIDToGIDMap", "Identity"); }
    let font = dictionary!{"Subtype"=>"Type0", "Encoding"=>"Identity-H", "ToUnicode"=>map,
        "DescendantFonts"=>vec![Object::Dictionary(child)]};
    (pdf, font, bytes)
}

#[test]
fn editing_verifies_usable_entries_without_granting_excluded_codes() {
    let (pdf, font, bytes) = composite(true, true);
    let proof = verify(&pdf, &font, bytes).unwrap();
    let encoded = proof.encode("Old").unwrap();
    assert_eq!(proof.decode(&encoded).unwrap(), "Old");
    assert!(proof.decode(&[0xff, 0xfe]).is_err());
    assert!(proof.encode("New").is_err());
    assert!(choice_font_codes(&pdf, &font).is_err());
}

#[test]
fn absent_cid_mapping_defaults_to_identity_only_for_text_proof() {
    let (pdf, font, bytes) = composite(false, false);
    let proof = verify(&pdf, &font, bytes).unwrap();
    let face = ttf_parser::Face::parse(bytes, 0).unwrap();
    let expected: Vec<u8> = "Old".chars().flat_map(|c| face.glyph_index(c).unwrap().0.to_be_bytes()).collect();
    assert_eq!(proof.encode("Old").unwrap(), expected);
    assert_eq!(proof.decode(&expected).unwrap(), "Old");
    assert!(choice_font_codes(&pdf, &font).is_err());
}

#[test]
fn present_invalid_cid_maps_are_never_defaulted_to_identity() {
    use lopdf::{dictionary, Stream};
    let (mut pdf, font, bytes) = composite(false, true);
    let odd = pdf.add_object(Stream::new(dictionary!{}, vec![0, 1, 2]));
    let short = pdf.add_object(Stream::new(dictionary!{}, vec![0, 1]));
    for value in [Object::Null, Object::Name(b"Other".to_vec()), Object::Integer(1), odd.into(), short.into()] {
        let mut bad = font.clone();
        bad.get_mut(b"DescendantFonts").unwrap().as_array_mut().unwrap()[0]
            .as_dict_mut().unwrap().set("CIDToGIDMap", value);
        assert!(verify(&pdf, &bad, bytes).is_err());
    }
    assert!(choice_font_codes(&pdf, &font).is_ok(), "Control is a valid complete font");
}

#[test]
fn an_unused_valid_mapping_still_requires_the_program_width() {
    let (pdf, mut font, bytes) = composite(true, true);
    let child = font.get_mut(b"DescendantFonts").unwrap().as_array_mut().unwrap()[0].as_dict_mut().unwrap();
    child.get_mut(b"W").unwrap().as_array_mut().unwrap()[5] = Object::Array(vec![Object::Integer(9999)]);
    let error = verify(&pdf, &font, bytes).unwrap_err().to_string();
    assert!(error.contains("width disagrees"), "{error}");
}
