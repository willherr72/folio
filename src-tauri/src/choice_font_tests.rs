use crate::choice_font::ChoiceFontProof;
use lopdf::{dictionary, Dictionary, Document, Object, Stream};
use std::path::PathBuf;

fn base() -> (Document, Object) {
    (Document::new(),dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica","Encoding"=>"WinAnsiEncoding"}.into())
}
fn embedded() -> (Document, Object) {
    let d = Document::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/choice-probe/embedded-source.pdf"),
    )
    .unwrap();
    let font = d
        .objects
        .iter()
        .find(|(_, o)| {
            o.as_dict().is_ok_and(|v| {
                v.get(b"Subtype")
                    .is_ok_and(|v| v.as_name().ok() == Some(b"TrueType"))
            })
        })
        .map(|(id, _)| Object::Reference(*id))
        .unwrap();
    (d, font)
}
fn cid() -> (Document, Object, u16) {
    let (doc, font, glyphs) = cid_for(&['A', 'Ω', 'é']);
    (doc, font, glyphs[1])
}
pub(crate) fn cid_for(characters: &[char]) -> (Document, Object, Vec<u16>) {
    let bytes = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf"),
    )
    .unwrap();
    let face = ttf_parser::Face::parse(&bytes, 0).unwrap();
    let scale = 1000. / face.units_per_em() as f32;
    let glyphs: Vec<_> = characters.iter().map(|&c| (c, face.glyph_index(c).unwrap().0)).collect();
    let mut widths = Vec::new();
    let mut cmap=format!("/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CMapName /Proof def /CMapType 2 def 1 begincodespacerange <0000> <FFFF> endcodespacerange {} beginbfchar\n", glyphs.len());
    for &(c, gid) in &glyphs {
        widths.push(Object::Integer(gid as i64));
        widths.push(Object::Array(vec![Object::Real(
            face.glyph_hor_advance(ttf_parser::GlyphId(gid)).unwrap() as f32 * scale,
        )]));
        let mut units = [0u16; 2];
        let unicode: String = c.encode_utf16(&mut units).iter().map(|u| format!("{u:04X}")).collect();
        cmap.push_str(&format!("<{gid:04X}> <{unicode}>\n"));
    }
    cmap.push_str("endbfchar endcmap CMapName currentdict /CMap defineresource pop end end");
    let bounds = face.global_bounding_box();
    let mut descriptor=dictionary!{"Type"=>"FontDescriptor","FontName"=>"DejaVuSerif","Flags"=>32,
        "FontBBox"=>vec![(bounds.x_min as f32*scale).into(),(bounds.y_min as f32*scale).into(),(bounds.x_max as f32*scale).into(),(bounds.y_max as f32*scale).into()],
        "Ascent"=>face.ascender() as f32*scale,"Descent"=>face.descender() as f32*scale,
        "CapHeight"=>face.capital_height().unwrap_or(face.ascender()) as f32*scale,"ItalicAngle"=>0,"StemV"=>80};
    let mut d = Document::new();
    let program = d.add_object(Stream::new(Dictionary::new(), bytes));
    descriptor.set("FontFile2",program);
    let descriptor=d.add_object(descriptor);
    let child=d.add_object(dictionary!{"Type"=>"Font","Subtype"=>"CIDFontType2","BaseFont"=>"DejaVuSerif","CIDToGIDMap"=>"Identity","FontDescriptor"=>descriptor,"W"=>widths,
        "CIDSystemInfo"=>dictionary!{"Registry"=>Object::string_literal("Adobe"),"Ordering"=>Object::string_literal("Identity"),"Supplement"=>0}});
    let unicode = d.add_object(Stream::new(Dictionary::new(), cmap.into_bytes()));
    let font=d.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type0","BaseFont"=>"DejaVuSerif","Encoding"=>"Identity-H","DescendantFonts"=>vec![child.into()],"ToUnicode"=>unicode});
    (d, font.into(), glyphs.iter().map(|v|v.1).collect())
}
#[test]
fn supplementary_mapping_is_not_conflicted_by_bmp_only_subtables() {
    let (d,f,glyphs)=cid_for(&['A','𝔸']);
    let proof=ChoiceFontProof::for_texts(&d,&f,&["A𝔸"]).unwrap();
    assert_eq!(proof.encode("𝔸").unwrap(),glyphs[1].to_be_bytes());
    assert_eq!(proof.decode(&proof.encode("A𝔸").unwrap()).unwrap(),"A𝔸");
}
#[test]
fn standard_encoding_proves_accents_and_refuses_unrepresented_labels() {
    let (d, f) = base();
    let p = ChoiceFontProof::for_texts(&d, &f, &["Alpha", "Café"]).unwrap();
    let encoded = p.encode("Café").unwrap();
    assert_eq!(encoded, b"Caf\xe9");
    assert_eq!(p.decode(&encoded).unwrap(), "Café");
    assert!(p.supports(&["Alpha", "字"]).is_err());
    assert!(p.encode("字").is_err());
}
#[test]
fn familiar_names_do_not_authorize_custom_or_unknown_encodings() {
    for variant in 0..4 {
        let (mut d, mut f) = base();
        let font = f.as_dict_mut().unwrap();
        match variant {
            0=>font.set("Encoding","UnknownEncoding"),
            1=>font.set("Encoding",dictionary!{"BaseEncoding"=>"WinAnsiEncoding","Differences"=>vec![65.into(),Object::Name(b"B".to_vec())]}),
            2=>{let s=d.add_object(Stream::new(Dictionary::new(),b"garbage".to_vec()));font.set("ToUnicode",s);},
            _=>font.set("FontDescriptor",dictionary!{"Flags"=>4}),
        }
        assert!(
            ChoiceFontProof::for_texts(&d, &f, &["A"]).is_err(),
            "variant{variant}"
        );
    }
}
#[test]
fn full_embedded_winansi_keeps_its_real_encoding_and_glyphs() {
    let (d, f) = embedded();
    let p = ChoiceFontProof::for_texts(&d, &f, &["Café", "Beta"]).unwrap();
    assert_eq!(p.decode(&p.encode("Café").unwrap()).unwrap(), "Café");
    assert!(
        p.supports(&["Ω"]).is_err(),
        "program repertoire must not override the PDF encoding"
    );
}
#[test]
fn identity_h_can_prove_greek_without_changing_existing_latin_editing_policy() {
    let (d, f, gid) = cid();
    let p = ChoiceFontProof::for_texts(&d, &f, &["AΩé"]).unwrap();
    assert_eq!(p.encode("Ω").unwrap(), gid.to_be_bytes());
    assert_eq!(p.decode(&p.encode("AΩé").unwrap()).unwrap(), "AΩé");
    assert!(p.decode(&[0]).is_err());
    assert!(p.decode(&[0xff, 0xff]).is_err());
}
#[test]
fn missing_glyphs_and_inconsistent_widths_cannot_be_proved() {
    for width in [false, true] {
        let (mut d, f, gid) = cid();
        let root = d.dereference(&f).unwrap().1.as_dict().unwrap();
        let child = root.get(b"DescendantFonts").unwrap().as_array().unwrap()[0]
            .as_reference()
            .unwrap();
        if width {
            let widths = d
                .get_dictionary_mut(child)
                .unwrap()
                .get_mut(b"W")
                .unwrap()
                .as_array_mut()
                .unwrap();
            widths[3] = Object::Array(vec![1234.into()]);
        } else {
            let mut map = vec![0; 2 * (gid as usize + 1)];
            let (clean, _, _) = cid();
            let fd = clean.get_dictionary(child).unwrap();
            let w = fd.get(b"W").unwrap().as_array().unwrap();
            for code in w.iter().step_by(2) {
                let code = code.as_i64().unwrap() as usize;
                if code < map.len() / 2 && code != gid as usize {
                    map[code * 2..code * 2 + 2].copy_from_slice(&(code as u16).to_be_bytes());
                }
            }
            let stream = d.add_object(Stream::new(Dictionary::new(), map));
            d.get_dictionary_mut(child)
                .unwrap()
                .set("CIDToGIDMap", stream);
        }
        assert!(ChoiceFontProof::for_texts(&d, &f, &["Ω"]).is_err());
    }
}
#[test]
fn text_budgets_apply_before_font_work_and_codecs_are_bounded() {
    let d = Document::new();
    let huge = "a".repeat(4097);
    let e = ChoiceFontProof::for_texts(&d, &Object::Null, &[huge.as_str()])
        .err()
        .unwrap()
        .to_string();
    assert!(e.contains("limit"));
    let (d, f) = base();
    assert!(ChoiceFontProof::for_texts(&d, &f, &vec![""; 4002]).is_err());
    let text = "a".repeat(4096);
    assert!(ChoiceFontProof::for_texts(&d, &f, &vec![text.as_str(); 33]).is_err());
    let p = ChoiceFontProof::for_texts(&d, &f, &[""]).unwrap();
    assert_eq!(p.encode("").unwrap(), Vec::<u8>::new());
    assert!(p.decode(&vec![b'a'; 128 * 1024 + 1]).is_err());
}

#[test]
fn nonfinite_default_width_is_not_positive_glyph_evidence() {
    let (mut d, f, _) = cid();
    let child = d
        .dereference(&f)
        .unwrap()
        .1
        .as_dict()
        .unwrap()
        .get(b"DescendantFonts")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .as_reference()
        .unwrap();
    let font = d.get_dictionary_mut(child).unwrap();
    font.set("W", Vec::<Object>::new());
    font.set("DW", Object::Real(f32::NAN));
    assert!(
        ChoiceFontProof::for_texts(&d, &f, &["Ω"]).is_err(),
        "NaN cannot prove a width match"
    );
}

#[test]
fn standard_default_and_indirect_encoding_are_explicit_and_decode_is_strict() {
    let (mut d, mut f) = base();
    f.as_dict_mut().unwrap().remove(b"Encoding");
    let p = ChoiceFontProof::for_texts(&d, &f, &["Alpha"]).unwrap();
    assert_eq!(p.decode(&p.encode("Alpha").unwrap()).unwrap(), "Alpha");
    assert!(p.decode(&[0]).is_err());
    let encoding = d.add_object(Object::Name(b"WinAnsiEncoding".to_vec()));
    f.as_dict_mut().unwrap().set("Encoding", encoding);
    let p = ChoiceFontProof::for_texts(&d, &f, &["Café"]).unwrap();
    assert_eq!(p.decode(&[0xe9]).unwrap(), "é");
    assert!(
        p.decode(&vec![0xe9; 65537]).is_err(),
        "decoded UTF-8 must be charged too"
    );
    f.as_dict_mut().unwrap().set(
        "FontDescriptor",
        dictionary! {"Flags"=>32,"FontFile"=>Object::Null},
    );
    assert!(ChoiceFontProof::for_texts(&d, &f, &["Alpha"]).is_err());
}

#[test]
fn conflicting_unicode_maps_cannot_authorize_non_latin_choices() {
    let (mut d, f, _) = cid();
    let program = d
        .objects
        .values()
        .filter_map(|o| o.as_dict().ok())
        .find_map(|v| v.get(b"FontFile2").ok().and_then(|v| v.as_reference().ok()))
        .unwrap();
    let bytes = &mut d
        .get_object_mut(program)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .content;
    let u16_at = |b: &[u8], i| u16::from_be_bytes(b[i..i + 2].try_into().unwrap()) as usize;
    let u32_at = |b: &[u8], i| u32::from_be_bytes(b[i..i + 4].try_into().unwrap()) as usize;
    let record = (0..u16_at(bytes, 4))
        .map(|i| 12 + i * 16)
        .find(|&i| &bytes[i..i + 4] == b"cmap")
        .unwrap();
    let cmap = u32_at(bytes, record + 8);
    let subtable = (0..u16_at(bytes, cmap + 2))
        .map(|i| cmap + 4 + i * 8)
        .map(|i| cmap + u32_at(bytes, i + 4))
        .find(|&i| u16_at(bytes, i) == 4)
        .unwrap();
    let count = u16_at(bytes, subtable + 6) / 2;
    let segment = (0..count)
        .find(|&i| {
            u16_at(bytes, subtable + 14 + i * 2) >= 'Ω' as usize
                && u16_at(bytes, subtable + 16 + count * 2 + i * 2) <= 'Ω' as usize
        })
        .unwrap();
    let delta = subtable + 16 + count * 4 + segment * 2;
    let changed = (u16_at(bytes, delta) as u16).wrapping_add(1);
    bytes[delta..delta + 2].copy_from_slice(&changed.to_be_bytes());
    // Confirm the fixture actually has competing nominal glyphs; don't merely
    // accept an unrelated parser failure as evidence of the conflict guard.
    let face = ttf_parser::Face::parse(bytes, 0).unwrap();
    let glyphs: std::collections::BTreeSet<_> = face
        .tables()
        .cmap
        .unwrap()
        .subtables
        .into_iter()
        .filter(|t| t.is_unicode())
        .map(|t| t.glyph_index('Ω' as u32).map(|g| g.0))
        .collect();
    assert!(glyphs.len() > 1);
    let error = ChoiceFontProof::for_texts(&d, &f, &["Ω"])
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("conflicting Unicode"), "{error}");
}
