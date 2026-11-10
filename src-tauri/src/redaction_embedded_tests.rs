use super::*;
use crate::redaction_ranges::{inspect_standard_glyphs, remove_standard_text_ranges, TextRange};
use lopdf::{
    content::{Content, Operation},
    dictionary, Dictionary, Document, Object, Stream, StringFormat,
};

fn fixture(cid: bool) -> (Document, lopdf::ObjectId) {
    let asset = FontAsset::parse(
        include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec(),
    )
    .unwrap();
    let mut pdf = Document::with_version("1.7");
    let tree = pdf.new_object_id();
    let resource = persistence::install_paragraph_font_resource(&mut pdf, &asset).unwrap();
    let id = resource.as_reference().unwrap();
    if !cid {
        let root = pdf.get_dictionary(id).unwrap().clone();
        let child = pdf
            .dereference(&root.get(b"DescendantFonts").unwrap().as_array().unwrap()[0])
            .unwrap()
            .1
            .as_dict()
            .unwrap()
            .clone();
        let mut widths = vec![Object::Integer(0); 256];
        let mut cmap = String::from("1 begincodespacerange <00> <FF> endcodespacerange\n");
        let chars: std::collections::BTreeSet<char> = "Résumé SECRET suite".chars().collect();
        cmap.push_str(&format!("{} beginbfchar\n", chars.len()));
        for ch in chars {
            widths[ch as usize] = Object::Real((asset.advance(ch).unwrap() * 1000.).round());
            cmap.push_str(&format!("<{:02X}> <{:04X}>\n", ch as u32, ch as u32));
        }
        cmap.push_str("endbfchar\n");
        let map = pdf.add_object(Stream::new(dictionary! {}, cmap.into_bytes()));
        pdf.objects.insert(id,Object::Dictionary(dictionary!{"Type"=>"Font","Subtype"=>"TrueType","BaseFont"=>child.get(b"BaseFont").unwrap().clone(),"Encoding"=>"WinAnsiEncoding","FirstChar"=>0,"LastChar"=>255,"Widths"=>widths,"FontDescriptor"=>child.get(b"FontDescriptor").unwrap().clone(),"ToUnicode"=>map}));
    }
    let bytes = if cid {
        "Résumé SECRET suite"
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect()
    } else {
        "Résumé SECRET suite".chars().map(|ch| ch as u8).collect()
    };
    let ops = vec![
        Operation::new("BT", vec![]),
        Operation::new("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
        Operation::new(
            "Tm",
            vec![
                1.into(),
                0.into(),
                0.into(),
                1.into(),
                20.into(),
                240.into(),
            ],
        ),
        Operation::new("Tj", vec![Object::String(bytes, StringFormat::Hexadecimal)]),
        Operation::new("ET", vec![]),
    ];
    let stream = pdf.add_object(Stream::new(
        dictionary! {},
        Content { operations: ops }.encode().unwrap(),
    ));
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),500.into(),300.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>id}},"Contents"=>stream});
    pdf.objects.insert(
        tree,
        Object::Dictionary(
            dictionary! {"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1},
        ),
    );
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree});
    pdf.trailer.set("Root", root);
    (pdf, id)
}

#[test]
fn redaction_embedded_removes_by_scalar_and_retains_original_codes_and_geometry() {
    for cid in [false, true] {
        let (pdf, _) = fixture(cid);
        let before =
            inspect_standard_glyphs(&pdf).expect("verified embedded source must be inspectable");
        assert_eq!(before[0].source.text, "Résumé SECRET suite");
        let out = remove_standard_text_ranges(
            &pdf,
            &[TextRange {
                selection_id: before[0].source.selection_id.clone(),
                start: 7,
                end: 13,
            }],
        )
        .unwrap();
        assert_eq!(out.removed_characters, 6);
        let saved = Document::load_mem(&out.bytes).unwrap();
        let after = inspect_standard_glyphs(&saved).unwrap();
        assert_eq!(after[0].source.text, "Résumé  suite");
        let kept: Vec<_> = before[0]
            .glyphs
            .iter()
            .enumerate()
            .filter_map(|(i, g)| (!(7..13).contains(&i)).then_some(g))
            .collect();
        for (a, b) in kept.iter().zip(&after[0].glyphs) {
            assert_eq!(a.unicode, b.unicode);
            assert_eq!(a.code, b.code);
            for (x, y) in a.origin.iter().zip(b.origin).chain(a.end.iter().zip(b.end)) {
                assert!((x - y).abs() <= 0.002);
            }
        }
        let second = remove_standard_text_ranges(
            &saved,
            &[TextRange {
                selection_id: after[0].source.selection_id.clone(),
                start: 0,
                end: 6,
            }],
        )
        .unwrap();
        assert_eq!(
            inspect_standard_glyphs(&Document::load_mem(&second.bytes).unwrap()).unwrap()[0]
                .source
                .text,
            "  suite"
        );
    }
}

fn content(pdf: &Document) -> (lopdf::ObjectId, Vec<Operation>) {
    let page = *pdf.get_pages().values().next().unwrap();
    let id = pdf
        .get_dictionary(page)
        .unwrap()
        .get(b"Contents")
        .unwrap()
        .as_reference()
        .unwrap();
    (
        id,
        Content::decode(
            &pdf.get_object(id)
                .unwrap()
                .as_stream()
                .unwrap()
                .get_plain_content()
                .unwrap(),
        )
        .unwrap()
        .operations,
    )
}

#[test]
fn redaction_embedded_distinct_resources_share_one_published_program() {
    let (mut pdf, font) = fixture(true);
    let mut alternate = pdf.get_dictionary(font).unwrap().clone();
    alternate.set("BaseFont", "ProducerAlternative");
    let alternate = pdf.add_object(alternate);
    let first = *pdf.get_pages().values().next().unwrap();
    let mut page = pdf.get_dictionary(first).unwrap().clone();
    let tree = page.get(b"Parent").unwrap().as_reference().unwrap();
    page.set(
        "Resources",
        dictionary! {"Font"=>dictionary!{"F1"=>alternate}},
    );
    let second = pdf.add_object(page);
    pdf.get_dictionary_mut(tree).unwrap().set(
        "Kids",
        vec![Object::Reference(first), Object::Reference(second)],
    );
    pdf.get_dictionary_mut(tree).unwrap().set("Count", 2);
    let b = inspect_standard_glyphs(&pdf).unwrap();
    let out = remove_standard_text_ranges(
        &pdf,
        &[TextRange {
            selection_id: b[0].source.selection_id.clone(),
            start: 7,
            end: 13,
        }],
    )
    .unwrap();
    let saved = Document::load_mem(&out.bytes).unwrap();
    let mut programs = std::collections::HashSet::new();
    let mut roots = 0;
    for object in saved.objects.values() {
        if let Ok(d) = object.as_dict() {
            if d.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Type0") {
                roots += 1;
            }
            if let Ok(p) = d.get(b"FontFile2") {
                programs.insert(p.as_reference().unwrap());
            }
        }
    }
    assert_eq!(
        roots, 2,
        "distinct resource identities must remain distinct"
    );
    assert_eq!(programs.len(), 1, "shared program must publish only once");
}
fn set_content(pdf: &mut Document, id: lopdf::ObjectId, ops: Vec<Operation>) {
    pdf.objects.insert(
        id,
        Object::Stream(Stream::new(
            dictionary! {},
            Content { operations: ops }.encode().unwrap(),
        )),
    );
}
fn descendant_id(pdf: &Document, font: lopdf::ObjectId) -> lopdf::ObjectId {
    pdf.get_dictionary(font)
        .unwrap()
        .get(b"DescendantFonts")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .as_reference()
        .unwrap()
}

#[test]
fn redaction_embedded_shared_aliases_pages_publish_one_program_and_bind_font_identity() {
    let (mut pdf, font) = fixture(true);
    let first = *pdf.get_pages().values().next().unwrap();
    let page = pdf.get_dictionary(first).unwrap().clone();
    let tree = page.get(b"Parent").unwrap().as_reference().unwrap();
    let second = pdf.add_object(page);
    pdf.get_dictionary_mut(tree).unwrap().set(
        "Kids",
        vec![Object::Reference(first), Object::Reference(second)],
    );
    pdf.get_dictionary_mut(tree).unwrap().set("Count", 2);
    {
        use crate::redaction_content::TextFonts;
        let resources = dictionary! {"Font"=>dictionary!{"F1"=>font,"Alias"=>font}};
        let mut registry = crate::redaction_fonts::Registry::default();
        let mut aliases =
            crate::redaction_fonts::PageFonts::new(&pdf, Some(&resources), &mut registry);
        assert_eq!(aliases.select(b"F1").unwrap(), 0);
        assert_eq!(aliases.select(b"Alias").unwrap(), 0);
        assert_eq!(aliases.fonts.len(), 1);
    }
    let prepared = crate::redaction_core::prepare(&pdf).unwrap();
    assert!(Arc::ptr_eq(
        &prepared.pages[0].fonts[0],
        &prepared.pages[1].fonts[0]
    ));
    let blocks = inspect_standard_glyphs(&pdf).unwrap();
    let range = TextRange {
        selection_id: blocks[0].source.selection_id.clone(),
        start: 7,
        end: 13,
    };
    let out = remove_standard_text_ranges(&pdf, &[range]).unwrap();
    let saved = Document::load_mem(&out.bytes).unwrap();
    assert_eq!(
        saved
            .objects
            .values()
            .filter(|v| v.as_dict().is_ok_and(|d| d.has(b"FontFile2")))
            .count(),
        1
    );
    assert_eq!(
        inspect_standard_glyphs(&saved)
            .unwrap()
            .iter()
            .map(|b| b.source.text.as_str())
            .collect::<Vec<_>>(),
        vec!["Résumé  suite", "Résumé SECRET suite"]
    );
    pdf.get_dictionary_mut(font)
        .unwrap()
        .set("BaseFont", "ChangedResourceName");
    assert!(remove_standard_text_ranges(
        &pdf,
        &[TextRange {
            selection_id: blocks[0].source.selection_id.clone(),
            start: 7,
            end: 13
        }]
    )
    .is_err());
}

#[test]
fn redaction_embedded_cid_space_and_code32_do_not_receive_word_spacing() {
    let (mut pdf, font) = fixture(true);
    let child = descendant_id(&pdf, font);
    let asset = FontAsset::parse(
        include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec(),
    )
    .unwrap();
    let codes = [(32u16, 'S'), (67, 'é'), (65, ' '), (66, 'R')];
    let mut mapping = vec![0u8; 136];
    let mut widths = vec![];
    let mut unicode =
        String::from("1 begincodespacerange <0000> <FFFF> endcodespacerange\n4 beginbfchar\n");
    for (code, ch) in codes {
        mapping[code as usize * 2..code as usize * 2 + 2]
            .copy_from_slice(&asset.glyph_id(ch).unwrap().to_be_bytes());
        widths.push(Object::Integer(code as i64));
        widths.push(Object::Array(vec![Object::Real(
            (asset.advance(ch).unwrap() * 1000.).round(),
        )]));
        unicode.push_str(&format!("<{code:04X}> <{:04X}>\n", ch as u32));
    }
    unicode.push_str("endbfchar\n");
    let mapping = pdf.add_object(Stream::new(dictionary! {}, mapping));
    let unicode = pdf.add_object(Stream::new(dictionary! {}, unicode.into_bytes()));
    pdf.get_dictionary_mut(child)
        .unwrap()
        .set("CIDToGIDMap", mapping);
    pdf.get_dictionary_mut(child).unwrap().set("W", widths);
    pdf.get_dictionary_mut(font)
        .unwrap()
        .set("ToUnicode", unicode);
    let (id, mut ops) = content(&pdf);
    ops[3].operands = vec![Object::String(
        codes
            .iter()
            .flat_map(|(code, _)| code.to_be_bytes())
            .collect(),
        StringFormat::Hexadecimal,
    )];
    ops.insert(3, Operation::new("Tw", vec![10.into()]));
    set_content(&mut pdf, id, ops);
    let b = inspect_standard_glyphs(&pdf).unwrap();
    assert_eq!(b[0].source.text, "Sé R");
    let expected = (asset.advance('S').unwrap() * 1000.).round() as f64 / 1000. * 12.;
    assert!((b[0].glyphs[1].origin[0] - 20. - expected).abs() < 0.002);
    let out = remove_standard_text_ranges(
        &pdf,
        &[TextRange {
            selection_id: b[0].source.selection_id.clone(),
            start: 0,
            end: 1,
        }],
    )
    .unwrap();
    assert_eq!(
        inspect_standard_glyphs(&Document::load_mem(&out.bytes).unwrap()).unwrap()[0]
            .source
            .text,
        "é R"
    );
}

#[test]
fn redaction_embedded_mixed_fonts_restore_across_q_and_keep_tj_spacing() {
    for cid in [false, true] {
        let (mut pdf, _) = fixture(cid);
        let page = *pdf.get_pages().values().next().unwrap();
        let other=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier","Encoding"=>"WinAnsiEncoding"});
        pdf.get_dictionary_mut(page)
            .unwrap()
            .get_mut(b"Resources")
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .get_mut(b"Font")
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("Other", other);
        let (id, mut ops) = content(&pdf);
        let text = ops[3].operands[0].clone();
        ops.splice(
            3..4,
            [
                Operation::new("Tc", vec![Object::Real(0.25)]),
                Operation::new("Tw", vec![3.into()]),
                Operation::new("Tz", vec![80.into()]),
                Operation::new("TJ", vec![Object::Array(vec![text, Object::Integer(-125)])]),
                Operation::new("q", vec![]),
                Operation::new("Tf", vec![Object::Name(b"Other".to_vec()), 12.into()]),
                Operation::new("Tj", vec![Object::string_literal(" Following")]),
                Operation::new("Q", vec![]),
                Operation::new("Td", vec![0.into(), (-20).into()]),
                Operation::new(
                    "Tj",
                    vec![Object::String(
                        if cid {
                            vec![0, 83, 0, 69, 0, 67, 0, 82, 0, 69, 0, 84]
                        } else {
                            b"SECRET".to_vec()
                        },
                        StringFormat::Hexadecimal,
                    )],
                ),
            ],
        );
        set_content(&mut pdf, id, ops);
        let b = inspect_standard_glyphs(&pdf).unwrap();
        assert_eq!(b[0].source.text, "Résumé SECRET suite FollowingSECRET");
        let out = remove_standard_text_ranges(
            &pdf,
            &[TextRange {
                selection_id: b[0].source.selection_id.clone(),
                start: 7,
                end: 13,
            }],
        )
        .unwrap();
        assert_eq!(
            inspect_standard_glyphs(&Document::load_mem(&out.bytes).unwrap()).unwrap()[0]
                .source
                .text,
            "Résumé  suite FollowingSECRET"
        );
    }
}

#[test]
fn redaction_embedded_rejects_incomplete_codes_unproved_maps_and_unrelated_graphs() {
    let (base, font) = fixture(true);
    for mode in 0..5 {
        let mut pdf = base.clone();
        match mode {
            0 => {
                let (id, mut ops) = content(&pdf);
                ops[3].operands = vec![Object::String(vec![0], StringFormat::Hexadecimal)];
                set_content(&mut pdf, id, ops);
            }
            1 => {
                let (id, mut ops) = content(&pdf);
                ops[3].operands = vec![Object::String(vec![0xd8, 0], StringFormat::Hexadecimal)];
                set_content(&mut pdf, id, ops);
            }
            2 => {
                pdf.get_dictionary_mut(font).unwrap().set(
                    "Unrelated",
                    dictionary! {"Secret"=>Object::string_literal("must never copy")},
                );
            }
            3 => {
                pdf.get_dictionary_mut(font)
                    .unwrap()
                    .set("DescendantFonts", vec![Object::Reference(font)]);
            }
            _ => {
                let map=pdf.add_object(Stream::new(dictionary!{},b"1 begincodespacerange <0000> <FFFF> endcodespacerange 1 beginbfchar <0053> <0053> endbfchar".to_vec()));
                pdf.get_dictionary_mut(font).unwrap().set("ToUnicode", map);
            }
        }
        assert!(inspect_standard_glyphs(&pdf).is_err(), "mode {mode}");
    }
}

#[test]
fn redaction_embedded_native_preview_apply_keeps_original_and_retained_pixels() {
    for (cid, spaced) in [(false, false), (true, false), (false, true), (true, true)] {
        let (mut pdf, _) = fixture(cid);
        if spaced {
            let (id, mut ops) = content(&pdf);
            ops.splice(
                3..3,
                [
                    Operation::new("Tc", vec![Object::Real(0.25)]),
                    Operation::new("Tw", vec![3.into()]),
                    Operation::new("Tz", vec![80.into()]),
                ],
            );
            set_content(&mut pdf, id, ops);
        }
        let mut bytes = vec![];
        pdf.save_to(&mut bytes).unwrap();
        let mut rt = WorkerRuntime::new(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
            Arc::new(FontRegistry::new()),
        )
        .unwrap();
        let source = rt
            .register_generated_document(bytes.clone(), "Embedded removal.pdf")
            .unwrap();
        let owner = rt.begin_redaction_session().unwrap();
        let selected = rt.inspect_redaction_text_page(&source.id, 0).unwrap();
        let boxes: Vec<_> = selected
            .glyphs()
            .iter()
            .filter(|g| (7..13).contains(&g.offset))
            .map(|g| g.bounds)
            .collect();
        assert_eq!(boxes.len(), 6);
        let x = boxes.iter().map(|b| b.x).fold(f64::INFINITY, f64::min);
        let y = boxes.iter().map(|b| b.y).fold(f64::INFINITY, f64::min);
        let right = boxes.iter().map(|b| b.x + b.width).fold(0., f64::max);
        let bottom = boxes.iter().map(|b| b.y + b.height).fold(0., f64::max);
        let preview = rt
            .preview_redaction(
                &owner,
                &source.id,
                &[TextRedactionMark {
                    page_index: 0,
                    rect: TextRedactionRect {
                        x,
                        y,
                        width: right - x,
                        height: bottom - y,
                    },
                }],
            )
            .unwrap();
        assert_eq!(preview.removed_characters, 6);
        let applied = rt.apply_redaction(&owner, &preview.preview_id).unwrap();
        rt.release_redaction_owner(&owner);
        let output = rt.document(&applied.id).unwrap().bytes().unwrap();
        assert_eq!(&*rt.document(&source.id).unwrap().bytes().unwrap(), &bytes);
        assert_eq!(
            inspect_standard_glyphs(&Document::load_mem(&output).unwrap()).unwrap()[0]
                .source
                .text,
            "Résumé  suite"
        );
        let render = |data: &[u8]| {
            let doc = rt.pdfium.load_pdf_from_byte_slice(data, None).unwrap();
            doc.pages()
                .get(0)
                .unwrap()
                .render_with_config(&PdfRenderConfig::new().set_target_width(1500))
                .unwrap()
                .as_image()
                .unwrap()
                .to_rgba8()
        };
        let a = render(&bytes);
        let b = render(&output);
        let mut changed = 0;
        for (px, py, v) in a.enumerate_pixels() {
            if v != b.get_pixel(px, py) {
                changed += 1;
                assert!(
                    boxes.iter().any(|r| (px as f64) >= r.x * 3. - 2.
                        && (px as f64) <= (r.x + r.width) * 3. + 2.
                        && (py as f64) >= r.y * 3. - 2.
                        && (py as f64) <= (r.y + r.height) * 3. + 2.),
                    "changed outside source glyphs {px},{py}"
                );
            }
        }
        assert!(changed > 0);
        rt.inspect_redaction_text_page(&applied.id, 0).unwrap();
        if let Some(dir) = std::env::var_os("FOLIO_EMBEDDED_REMOVAL_OUT") {
            use std::io::Write;
            let dir = PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            let stem = format!(
                "{}{}",
                if cid { "cid" } else { "simple" },
                if spaced { "-spaced" } else { "" }
            );
            for (name, data) in [
                (format!("{stem}-input.pdf"), bytes),
                (format!("{stem}-removed.pdf"), output.to_vec()),
                (
                    format!("{stem}-bounds.json"),
                    serde_json::to_vec_pretty(&boxes).unwrap(),
                ),
            ] {
                let mut f = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(dir.join(name))
                    .unwrap();
                f.write_all(&data).unwrap();
            }
        }
    }
}

#[test]
fn redaction_embedded_extension_keeps_standard_source_ascii_text() {
    let (mut pdf, font) = fixture(false);
    pdf.objects.insert(
        font,
        Object::Dictionary(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"}),
    );
    let (id, mut ops) = content(&pdf);
    ops[3].operands = vec![Object::string_literal("`Quoted'")];
    set_content(&mut pdf, id, ops);
    assert_eq!(
        crate::redaction_core::inspect_standard_text(&pdf).unwrap()[0].text,
        "`Quoted'"
    );
}

#[test]
fn redaction_embedded_resource_count_is_bounded_without_recopying_the_shared_program() {
    use crate::redaction_content::TextFonts;
    let (mut pdf, font) = fixture(false);
    let original = pdf.get_dictionary(font).unwrap().clone();
    let mut names = Dictionary::new();
    for i in 0..65 {
        let mut d = original.clone();
        d.set(
            "BaseFont",
            Object::Name(format!("Producer{i}").into_bytes()),
        );
        names.set(format!("F{i}"), pdf.add_object(d));
    }
    let resources = dictionary! {"Font"=>names};
    let mut registry = crate::redaction_fonts::Registry::default();
    let mut resolver =
        crate::redaction_fonts::PageFonts::new(&pdf, Some(&resources), &mut registry);
    for i in 0..64 {
        assert_eq!(resolver.select(format!("F{i}").as_bytes()).unwrap(), i);
    }
    assert!(resolver.select(b"F64").is_err());
    assert_eq!(resolver.fonts.len(), 64);
}
