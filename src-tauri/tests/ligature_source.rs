use folio_engine::{DocumentInfo, ExportRequest, PagePlan, PdfEngine};
use lopdf::{
    content::{Content, Operation},
    dictionary, Document, Object, Stream,
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn retain(path: &Path) {
    if let Some(folder) = std::env::var_os("FOLIO_LIGATURE_OUTPUTS") {
        let folder = PathBuf::from(folder);
        fs::create_dir_all(&folder).unwrap();
        fs::copy(path, folder.join(path.file_name().unwrap())).unwrap();
    }
}

fn altered(source: &Path, target: &Path, kind: &str) {
    let mut pdf = Document::load(source).unwrap();
    let font = pdf
        .objects
        .iter()
        .find_map(|(id, object)| {
            object
                .as_dict()
                .ok()
                .filter(|d| d.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Type0"))
                .map(|_| *id)
        })
        .unwrap();
    let parent = pdf.get_dictionary(font).unwrap();
    let child = parent.get(b"DescendantFonts").unwrap().as_array().unwrap()[0]
        .as_reference()
        .unwrap();
    let map = parent.get(b"ToUnicode").unwrap().as_reference().unwrap();
    let data = include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf");
    let face = ttf_parser::Face::parse(data, 0).unwrap();
    let ligature = face.glyph_index('\u{fb03}').unwrap().0;
    let width = |gid| {
        face.glyph_hor_advance(ttf_parser::GlyphId(gid)).unwrap() as f32 * 1000.
            / face.units_per_em() as f32
    };
    match kind {
        "bad-width" | "empty-glyph" | "missing-glyph" | "ambiguous" | "wrong-letter" => {
            let mut widths = pdf
                .get_dictionary(child)
                .unwrap()
                .get(b"W")
                .unwrap()
                .as_array()
                .unwrap()
                .clone();
            let entry = widths
                .chunks_exact_mut(2)
                .find(|pair| pair[0].as_i64().unwrap() == i64::from(ligature))
                .unwrap();
            if kind == "bad-width" {
                entry[1] = Object::Array(vec![Object::Real(width(ligature) + 7.)]);
            }
            if kind == "empty-glyph" {
                entry[1] =
                    Object::Array(vec![Object::Real(width(face.glyph_index(' ').unwrap().0))]);
            }
            if kind == "wrong-letter" {
                entry[1] =
                    Object::Array(vec![Object::Real(width(face.glyph_index('l').unwrap().0))]);
            }
            if kind != "bad-width" {
                let mut mapping: Vec<u8> = (0u16..=6000).flat_map(u16::to_be_bytes).collect();
                let selected = if kind == "wrong-letter" {
                    face.glyph_index('l').unwrap().0
                } else if kind == "empty-glyph" {
                    face.glyph_index(' ').unwrap().0
                } else if kind == "missing-glyph" {
                    u16::MAX
                } else {
                    ligature
                };
                mapping[ligature as usize * 2..ligature as usize * 2 + 2]
                    .copy_from_slice(&selected.to_be_bytes());
                if kind == "ambiguous" {
                    mapping[12000..12002].copy_from_slice(&ligature.to_be_bytes());
                    widths.extend([
                        6000.into(),
                        Object::Array(vec![Object::Real(width(ligature))]),
                    ]);
                    let map_text = String::from_utf8(
                        pdf.get_object(map)
                            .unwrap()
                            .as_stream()
                            .unwrap()
                            .get_plain_content()
                            .unwrap(),
                    )
                    .unwrap();
                    let map_text = map_text.replace(
                        "endbfchar",
                        "endbfchar 1 beginbfchar <1770> <006600660069> endbfchar",
                    );
                    pdf.get_object_mut(map)
                        .unwrap()
                        .as_stream_mut()
                        .unwrap()
                        .set_content(map_text.into_bytes());
                }
                let mapping = pdf.add_object(Stream::new(dictionary! {}, mapping));
                pdf.get_dictionary_mut(child)
                    .unwrap()
                    .set("CIDToGIDMap", mapping);
            }
            pdf.get_dictionary_mut(child).unwrap().set("W", widths);
        }
        "malformed-unicode" => {
            let map_text = String::from_utf8(
                pdf.get_object(map)
                    .unwrap()
                    .as_stream()
                    .unwrap()
                    .get_plain_content()
                    .unwrap(),
            )
            .unwrap();
            assert!(map_text.contains("006600660069"));
            pdf.get_object_mut(map)
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .set_content(map_text.replace("006600660069", "0066d800").into_bytes());
        }
        "sequence-only" => {
            pdf.get_object_mut(map).unwrap().as_stream_mut().unwrap().set_content(format!(
                "1 begincodespacerange <0000> <ffff> endcodespacerange 1 beginbfchar <{ligature:04x}> <006600660069> endbfchar").into_bytes());
            let page = *pdf.get_pages().values().next().unwrap();
            let mut content = Content::decode(
                &pdf.get_page_content_with_limit(page, 16 * 1024 * 1024)
                    .unwrap(),
            )
            .unwrap();
            let show = content
                .operations
                .iter_mut()
                .find(|op| op.operator == "Tj")
                .unwrap();
            show.operands = vec![Object::string_literal(ligature.to_be_bytes().to_vec())];
            let stream = pdf.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
            pdf.get_dictionary_mut(page)
                .unwrap()
                .set("Contents", stream);
        }
        "no-cmap" => {
            let descriptor = pdf
                .get_dictionary(child)
                .unwrap()
                .get(b"FontDescriptor")
                .unwrap()
                .as_reference()
                .unwrap();
            let program = pdf
                .get_dictionary(descriptor)
                .unwrap()
                .get(b"FontFile2")
                .unwrap()
                .as_reference()
                .unwrap();
            let bytes = include_bytes!(
                "../../tests/fixtures/embedded-text/DejaVuSans-ligature-no-cmap.ttf"
            );
            let stream = pdf
                .get_object_mut(program)
                .unwrap()
                .as_stream_mut()
                .unwrap();
            stream.set_content(bytes.to_vec());
            stream.dict.set("Length1", bytes.len() as i64);
        }
        _ => panic!("Unknown public control"),
    }
    pdf.save(target).unwrap();
    retain(target);
}

fn scalar_copy(source: &Path, target: &Path) {
    let mut pdf = Document::load(source).unwrap();
    let page = *pdf.get_pages().values().next().unwrap();
    let mut content = Content::decode(
        &pdf.get_page_content_with_limit(page, 16 * 1024 * 1024)
            .unwrap(),
    )
    .unwrap();
    let face = ttf_parser::Face::parse(
        include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf"),
        0,
    )
    .unwrap();
    let show = content
        .operations
        .iter_mut()
        .find(|op| op.operator == "Tj")
        .unwrap();
    show.operands = vec![Object::string_literal(
        "this"
            .chars()
            .flat_map(|c| face.glyph_index(c).unwrap().0.to_be_bytes())
            .collect::<Vec<_>>(),
    )];
    let stream = pdf.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    pdf.get_dictionary_mut(page)
        .unwrap()
        .set("Contents", stream);
    pdf.save(target).unwrap();
    retain(target);
}
fn fixture(path: &Path, sequence: &str, positioned: bool, dependent: bool) {
    let program = include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf");
    let face = ttf_parser::Face::parse(program, 0).unwrap();
    let gid = |ch| face.glyph_index(ch).unwrap().0;
    let mut symbols: BTreeMap<_, _> = "Office this safe"
        .chars()
        .map(|c| (gid(c), c.to_string()))
        .collect();
    let ligature = if sequence == "fi" {
        '\u{fb01}'
    } else {
        '\u{fb03}'
    };
    symbols.insert(gid(ligature), sequence.into());
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let data = pdf.add_object(Stream::new(
        dictionary! {"Length1"=>program.len()as i64},
        program.to_vec(),
    ));
    let descriptor = pdf.add_object(dictionary!{"Type"=>"FontDescriptor","FontName"=>"DejaVuSans",
        "Flags"=>32,"FontBBox"=>vec![(-1000).into(),(-1000).into(),2000.into(),2000.into()],
        "ItalicAngle"=>0,"Ascent"=>1000,"Descent"=>-300,"CapHeight"=>800,"StemV"=>80,"FontFile2"=>data});
    let widths: Vec<Object> = symbols
        .keys()
        .flat_map(|code| {
            [
                Object::Integer(i64::from(*code)),
                Object::Array(vec![Object::Real(
                    face.glyph_hor_advance(ttf_parser::GlyphId(*code)).unwrap() as f32 * 1000.
                        / face.units_per_em() as f32,
                )]),
            ]
        })
        .collect();
    let child = pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"CIDFontType2","BaseFont"=>"DejaVuSans",
        "CIDSystemInfo"=>dictionary!{"Registry"=>Object::string_literal("Adobe"),"Ordering"=>Object::string_literal("Identity"),"Supplement"=>0},
        "FontDescriptor"=>descriptor,"CIDToGIDMap"=>"Identity","DW"=>0,"W"=>widths});
    let map = format!(
        "1 begincodespacerange <0000> <ffff> endcodespacerange {} beginbfchar {} endbfchar",
        symbols.len(),
        symbols
            .iter()
            .map(|(code, text)| format!(
                "<{code:04x}> <{}>",
                text.encode_utf16()
                    .map(|u| format!("{u:04x}"))
                    .collect::<String>()
            ))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let map = pdf.add_object(Stream::new(dictionary! {}, map.into_bytes()));
    let font = pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type0","BaseFont"=>"DejaVuSans","Encoding"=>"Identity-H","DescendantFonts"=>vec![child.into()],"ToUnicode"=>map});
    let footer =
        pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let encode = |text: &str| {
        text.chars()
            .flat_map(|c| gid(c).to_be_bytes())
            .collect::<Vec<_>>()
    };
    let body: Vec<_> = if sequence == "fi" {
        vec!['O', 'f', ligature, 'c', 'e']
    } else {
        vec!['O', ligature, 'c', 'e']
    };
    let raw: Vec<_> = body.iter().flat_map(|c| gid(*c).to_be_bytes()).collect();
    let mut operations = vec![
        Operation::new("BT", vec![]),
        Operation::new("Tf", vec![Object::Name(b"F1".to_vec()), 24.into()]),
        Operation::new(
            "Tm",
            vec![
                1.into(),
                0.into(),
                0.into(),
                1.into(),
                72.into(),
                280.into(),
            ],
        ),
    ];
    if dependent {
        operations.push(Operation::new("Tc", vec![Object::Real(1.5)]));
    }
    if positioned {
        operations.push(Operation::new(
            "TJ",
            vec![Object::Array(vec![
                25.into(),
                Object::string_literal(raw[..2].to_vec()),
                (-20).into(),
                Object::string_literal(raw[2..raw.len() - 4].to_vec()),
                10.into(),
                Object::string_literal(raw[raw.len() - 4..].to_vec()),
                45.into(),
            ])],
        ));
    } else {
        operations.push(Operation::new("Tj", vec![Object::string_literal(raw)]));
    }
    if dependent {
        operations.push(Operation::new(
            "Tj",
            vec![Object::string_literal(encode(" safe"))],
        ));
    }
    operations.extend([
        Operation::new("ET", vec![]),
        Operation::new("BT", vec![]),
        Operation::new("Tf", vec![Object::Name(b"F2".to_vec()), 14.into()]),
        Operation::new("Td", vec![72.into(), 70.into()]),
        Operation::new("Tj", vec![Object::string_literal("Neighbor safe")]),
        Operation::new("ET", vec![]),
    ]);
    let stream = pdf.add_object(Stream::new(
        dictionary! {},
        Content { operations }.encode().unwrap(),
    ));
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),400.into()],
        "Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font,"F2"=>footer}},"Contents"=>stream});
    pdf.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into(),
    );
    let catalog = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    pdf.trailer.set("Root", catalog);
    pdf.save(path).unwrap();
    retain(path);
}
fn save(e: &PdfEngine, source: &DocumentInfo, path: &Path) {
    e.export_pdf(
        ExportRequest {
            flatten: false,
            pages: vec![PagePlan {
                crop: None,
                id: "page".into(),
                source_id: source.id.clone(),
                page_index: 0,
                width: source.pages[0].width,
                height: source.pages[0].height,
                rotation: 0,
                overlays: vec![],
            }],
        },
        path,
    )
    .unwrap();
    retain(path);
}

#[test]
fn ligature_source_original_font_noop_growth_delete_save_and_reedit() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for sequence in ["fi", "ffi"] {
        for positioned in [false, true] {
            let input = temp.path().join(format!("{sequence}-{positioned}.pdf"));
            fixture(&input, sequence, positioned, false);
            let original = fs::read(&input).unwrap();
            let source = e.open_document(&input).unwrap();
            let before = e.list_occurrence_text_runs(&source.id, 0).unwrap().runs;
            assert_eq!(before[0].text, "Office");
            assert!(
                before[0].supported && before[0].can_substitute,
                "{before:?}"
            );
            let noop = e
                .replace_text_occurrence(&source.id, &[0], 0, 0, "Office", "Office", None)
                .unwrap()
                .document;
            assert_eq!(
                e.render_page(&source.id, 0, 800).unwrap(),
                e.render_page(&noop.id, 0, 800).unwrap()
            );
            e.close_document(&noop.id).unwrap();
            for value in ["Office this", "Ofice"] {
                let edited = e
                    .replace_text_occurrence(&source.id, &[0], 0, 0, "Office", value, None)
                    .unwrap()
                    .document;
                let output = temp
                    .path()
                    .join(format!("{sequence}-{positioned}-{value}.pdf"));
                save(&e, &edited, &output);
                let reopened = e.open_document(&output).unwrap();
                let after = e.list_occurrence_text_runs(&reopened.id, 0).unwrap().runs;
                assert_eq!(after[0].text, value);
                assert!(after[0].supported);
                assert_eq!(after[0].font_name, before[0].font_name);
                assert_eq!(after[1].text, before[1].text);
                assert_eq!(after[1].bounds, before[1].bounds);
                let again = e
                    .replace_text_occurrence(
                        &reopened.id,
                        &[0],
                        0,
                        0,
                        value,
                        &format!("{value} this"),
                        None,
                    )
                    .unwrap()
                    .document;
                assert_eq!(
                    e.list_occurrence_text_runs(&again.id, 0).unwrap().runs[0].text,
                    format!("{value} this")
                );
                for id in [&edited.id, &reopened.id, &again.id] {
                    e.close_document(id).unwrap();
                }
            }
            assert_eq!(fs::read(&input).unwrap(), original);
            e.close_document(&source.id).unwrap();
        }
    }
}

#[test]
fn ligature_source_character_spacing_keeps_following_text_and_explicit_substitution() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let font = e
        .fonts()
        .register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec())
        .unwrap();
    for positioned in [false, true] {
        let input = temp.path().join(format!("dependent-{positioned}.pdf"));
        fixture(&input, "ffi", positioned, true);
        let source = e.open_document(&input).unwrap();
        let before = e.list_occurrence_text_runs(&source.id, 0).unwrap().runs;
        assert!(before[0].supported, "{before:?}");
        eprintln!(
            "Public Tc fixture positioned={positioned}: selected text={:?}",
            before[0].text
        );
        assert_eq!(
            before[0]
                .text
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>(),
            "Office"
        );
        for substitute in [None, Some(font.id.as_str())] {
            let edited = e
                .replace_text_occurrence(
                    &source.id,
                    &[0],
                    0,
                    0,
                    &before[0].text,
                    "Ofice",
                    substitute,
                )
                .unwrap()
                .document;
            let output = temp.path().join(format!(
                "dependent-{positioned}-{}.pdf",
                substitute.is_some()
            ));
            save(&e, &edited, &output);
            let reopened = e.open_document(&output).unwrap();
            let after = e.list_occurrence_text_runs(&reopened.id, 0).unwrap().runs;
            assert_eq!(
                after[0]
                    .text
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect::<String>(),
                "Ofice"
            );
            assert_eq!(after.len(), before.len());
            for (a, b) in after.iter().skip(1).zip(before.iter().skip(1)) {
                assert_eq!(a.text, b.text);
                assert_eq!(a.bounds, b.bounds);
            }
            for id in [&edited.id, &reopened.id] {
                e.close_document(id).unwrap();
            }
        }
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn ligature_source_requires_actual_glyph_width_outline_and_unambiguous_unicode() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for kind in [
        "bad-width",
        "missing-glyph",
        "malformed-unicode",
        "ambiguous",
        "empty-glyph",
        "wrong-letter",
    ] {
        let valid = temp.path().join(format!("control-{kind}-source.pdf"));
        fixture(&valid, "ffi", false, false);
        let input = temp.path().join(format!("control-{kind}-bad.pdf"));
        altered(&valid, &input, kind);
        let bytes = fs::read(&input).unwrap();
        let doc = e.open_document(&input).unwrap();
        let runs = e.list_occurrence_text_runs(&doc.id, 0).unwrap().runs;
        assert!(
            !runs[0].supported && !runs[0].can_substitute,
            "{kind}: {runs:?}"
        );
        assert!(e
            .replace_text_occurrence(&doc.id, &[0], 0, 0, &runs[0].text, "Office this", None)
            .is_err());
        assert_eq!(e.source_bytes(&doc.id).unwrap().as_ref(), bytes.as_slice());
        assert_eq!(fs::read(&input).unwrap(), bytes);
        assert!(runs.last().unwrap().supported);
        e.close_document(&doc.id).unwrap();
    }
}

#[test]
fn ligature_source_sequence_only_repertoire_keeps_noop_and_explicit_substitution() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let valid = temp.path().join("sequence-only-source.pdf");
    fixture(&valid, "ffi", false, false);
    let input = temp.path().join("sequence-only.pdf");
    altered(&valid, &input, "sequence-only");
    let doc = e.open_document(&input).unwrap();
    let runs = e.list_occurrence_text_runs(&doc.id, 0).unwrap().runs;
    assert_eq!(runs[0].text, "ffi");
    assert!(runs[0].supported && runs[0].can_substitute, "{runs:?}");
    let noop = e
        .replace_text_occurrence(&doc.id, &[0], 0, 0, "ffi", "ffi", None)
        .unwrap()
        .document;
    assert_eq!(
        e.render_page(&doc.id, 0, 800).unwrap(),
        e.render_page(&noop.id, 0, 800).unwrap()
    );
    e.close_document(&noop.id).unwrap();
    assert!(e
        .replace_text_occurrence(&doc.id, &[0], 0, 0, "ffi", "fi", None)
        .is_err());
    let font = e
        .fonts()
        .register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec())
        .unwrap();
    let edited = e
        .replace_text_occurrence(&doc.id, &[0], 0, 0, "ffi", "fi", Some(&font.id))
        .unwrap()
        .document;
    let output = temp.path().join("sequence-only-substitute.pdf");
    save(&e, &edited, &output);
    let reopened = e.open_document(&output).unwrap();
    assert_eq!(
        e.list_occurrence_text_runs(&reopened.id, 0).unwrap().runs[0].text,
        "fi"
    );
    for id in [&doc.id, &edited.id, &reopened.id] {
        e.close_document(id).unwrap();
    }
}

#[test]
fn ligature_source_unused_unavailable_sequences_keep_the_scalar_repertoire_editable() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for kind in [
        "bad-width",
        "missing-glyph",
        "malformed-unicode",
        "ambiguous",
        "empty-glyph",
        "wrong-letter",
    ] {
        let valid = temp.path().join(format!("unused-{kind}-source.pdf"));
        fixture(&valid, "ffi", false, false);
        let bad = temp.path().join(format!("unused-{kind}-unused.pdf"));
        altered(&valid, &bad, kind);
        let input = temp.path().join(format!("unused-{kind}-scalar.pdf"));
        scalar_copy(&bad, &input);
        let doc = e.open_document(&input).unwrap();
        let runs = e.list_occurrence_text_runs(&doc.id, 0).unwrap().runs;
        assert_eq!(runs[0].text, "this");
        assert!(
            runs[0].supported && runs[0].can_substitute,
            "{kind}: {runs:?}"
        );
        let edited = e
            .replace_text_occurrence(&doc.id, &[0], 0, 0, "this", "this this", None)
            .unwrap()
            .document;
        assert_eq!(
            e.list_occurrence_text_runs(&edited.id, 0).unwrap().runs[0].text,
            "this this"
        );
        e.close_document(&edited.id).unwrap();
        e.close_document(&doc.id).unwrap();
    }
}

#[test]
fn ligature_source_without_internal_cmap_keeps_exact_source_and_saved_reedit() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let valid = temp.path().join("no-cmap-source.pdf");
    fixture(&valid, "ffi", false, false);
    let input = temp.path().join("no-cmap.pdf");
    altered(&valid, &input, "no-cmap");
    let source = e.open_document(&input).unwrap();
    let runs = e.list_occurrence_text_runs(&source.id, 0).unwrap().runs;
    assert_eq!(runs[0].text, "Office");
    assert!(runs[0].supported, "{runs:?}");
    let noop = e
        .replace_text_occurrence(&source.id, &[0], 0, 0, "Office", "Office", None)
        .unwrap()
        .document;
    assert_eq!(
        e.render_page(&source.id, 0, 800).unwrap(),
        e.render_page(&noop.id, 0, 800).unwrap()
    );
    e.close_document(&noop.id).unwrap();
    let edit = e
        .replace_text_occurrence(&source.id, &[0], 0, 0, "Office", "Office this", None)
        .unwrap()
        .document;
    let output = temp.path().join("no-cmap-saved.pdf");
    save(&e, &edit, &output);
    let reopened = e.open_document(&output).unwrap();
    assert_eq!(
        e.list_occurrence_text_runs(&reopened.id, 0).unwrap().runs[0].text,
        "Office this"
    );
    let again = e
        .replace_text_occurrence(&reopened.id, &[0], 0, 0, "Office this", "Office", None)
        .unwrap()
        .document;
    assert_eq!(
        e.list_occurrence_text_runs(&again.id, 0).unwrap().runs[0].text,
        "Office"
    );
    for id in [&source.id, &edit.id, &reopened.id, &again.id] {
        e.close_document(id).unwrap();
    }
}

// Append to the existing public ligature_source integration target after the
// current source/input guards settle. Uses its public fixture/save helpers.
fn shared_form_fixture(source: &Path, target: &Path) {
    let mut pdf = Document::load(source).unwrap();
    let page = *pdf.get_pages().values().next().unwrap();
    let mut content = Content::decode(
        &pdf.get_page_content_with_limit(page, 16 * 1024 * 1024)
            .unwrap(),
    )
    .unwrap();
    let stop = content
        .operations
        .iter()
        .position(|op| op.operator == "ET")
        .unwrap()
        + 1;
    let body: Vec<_> = content.operations.drain(..stop).collect();
    let resources = pdf
        .get_dictionary(page)
        .unwrap()
        .get(b"Resources")
        .unwrap()
        .clone();
    let leaf = pdf.add_object(Stream::new(
        dictionary! {"Type"=>"XObject","Subtype"=>"Form","FormType"=>1,
        "BBox"=>vec![0.into(),0.into(),400.into(),400.into()],"Resources"=>resources.clone()},
        Content { operations: body }.encode().unwrap(),
    ));
    let mut resources = resources.as_dict().unwrap().clone();
    resources.set("XObject", dictionary! {"Leaf"=>leaf});
    pdf.get_dictionary_mut(page)
        .unwrap()
        .set("Resources", resources);
    let mut operations = vec![
        Operation::new("q", vec![]),
        Operation::new("Do", vec![Object::Name(b"Leaf".to_vec())]),
        Operation::new("Q", vec![]),
        Operation::new("q", vec![]),
        Operation::new(
            "cm",
            vec![
                1.into(),
                0.into(),
                0.into(),
                1.into(),
                0.into(),
                (-120).into(),
            ],
        ),
        Operation::new("Do", vec![Object::Name(b"Leaf".to_vec())]),
        Operation::new("Q", vec![]),
    ];
    operations.extend(content.operations);
    let stream = pdf.add_object(Stream::new(
        dictionary! {},
        Content { operations }.encode().unwrap(),
    ));
    pdf.get_dictionary_mut(page)
        .unwrap()
        .set("Contents", stream);
    pdf.save(target).unwrap();
    retain(target);
}

fn embedded_programs(bytes: &[u8]) -> Vec<Vec<u8>> {
    let pdf = Document::load_mem(bytes).unwrap();
    let mut programs = Vec::new();
    for descriptor in pdf.objects.values().filter_map(|o| o.as_dict().ok()) {
        if let Ok(reference) = descriptor.get(b"FontFile2") {
            let stream = pdf.dereference(reference).unwrap().1.as_stream().unwrap();
            programs.push(stream.get_plain_content().unwrap());
        }
    }
    programs.sort();
    programs.dedup();
    programs
}

#[test]
fn ligature_forms_noop_and_same_program_reuse_preserve_shared_source_glyphs() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let font = e
        .fonts()
        .register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec())
        .unwrap();
    for positioned in [false, true] {
        let source_path = temp.path().join(format!("form-noop-{positioned}-flat.pdf"));
        fixture(&source_path, "ffi", positioned, false);
        let input = temp.path().join(format!("form-noop-{positioned}.pdf"));
        shared_form_fixture(&source_path, &input);
        let source = e.open_document(&input).unwrap();
        let before = e.list_occurrence_form_text_runs(&source.id, 0).unwrap();
        assert_eq!(before.runs.len(), 2, "{before:?}");
        assert!(before.runs.iter().all(|r| r.supported), "{before:?}");
        assert!(before.runs.iter().all(|r| r.text == "Office"));
        // Exercise the core API too, rather than depending on a no-op shortcut
        // in an outer occurrence/automation wrapper.
        let noop = e
            .replace_form_text(
                &source.id,
                0,
                &before.runs[0].object_path,
                "Office",
                "Office",
            )
            .unwrap();
        assert!(
            e.render_page(&source.id, 0, 800).unwrap() == e.render_page(&noop.id, 0, 800).unwrap(),
            "A source-font form no-op changed pixels"
        );
        assert_eq!(
            embedded_programs(&e.source_bytes(&source.id).unwrap()),
            embedded_programs(&e.source_bytes(&noop.id).unwrap())
        );
        e.close_document(&noop.id).unwrap();
        let reuse = e
            .replace_form_text_with_font(
                &source.id,
                0,
                &before.runs[0].object_path,
                "Office",
                "Office",
                Some(&font.id),
            )
            .unwrap();
        assert!(
            e.render_page(&source.id, 0, 800).unwrap() == e.render_page(&reuse.id, 0, 800).unwrap(),
            "Same-program source reuse changed pixels"
        );
        e.close_document(&reuse.id).unwrap();
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn ligature_forms_growth_delete_save_reopen_reedit_isolate_the_chosen_invocation() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for positioned in [false, true] {
        let flat = temp.path().join(format!("form-edit-{positioned}-flat.pdf"));
        fixture(&flat, "ffi", positioned, false);
        let input = temp.path().join(format!("form-edit-{positioned}.pdf"));
        shared_form_fixture(&flat, &input);
        let source = e.open_document(&input).unwrap();
        let before = e.list_occurrence_form_text_runs(&source.id, 0).unwrap();
        assert_eq!(before.runs.len(), 2, "{before:?}");
        for text in ["Office this", "Ofice"] {
            let edit = e
                .replace_form_text_occurrence(
                    &source.id,
                    &[0],
                    0,
                    &before.runs[0].object_path,
                    "Office",
                    text,
                    None,
                )
                .unwrap()
                .document;
            let output = temp
                .path()
                .join(format!("form-edit-{positioned}-{text}.pdf"));
            save(&e, &edit, &output);
            let reopened = e.open_document(&output).unwrap();
            let after = e.list_occurrence_form_text_runs(&reopened.id, 0).unwrap();
            assert_eq!(after.runs.len(), 2, "{after:?}");
            assert_eq!(after.runs[0].text, text);
            assert_eq!(after.runs[1], before.runs[1]);
            assert_eq!(after.runs[0].font_name, before.runs[0].font_name);
            assert_eq!(
                embedded_programs(&e.source_bytes(&source.id).unwrap()),
                embedded_programs(&e.source_bytes(&reopened.id).unwrap())
            );
            let again = e
                .replace_form_text_occurrence(
                    &reopened.id,
                    &[0],
                    0,
                    &after.runs[0].object_path,
                    text,
                    &format!("{text} this"),
                    None,
                )
                .unwrap()
                .document;
            assert_eq!(
                e.list_occurrence_form_text_runs(&again.id, 0).unwrap().runs[0].text,
                format!("{text} this")
            );
            for id in [&edit.id, &reopened.id, &again.id] {
                e.close_document(id).unwrap();
            }
        }
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn ligature_forms_cursor_and_explicit_substitution_preserve_other_shows() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let font = e
        .fonts()
        .register(include_bytes!("../../tests/fixtures/corpus/fonts/DejaVuSerif.ttf").to_vec())
        .unwrap();
    for positioned in [false, true] {
        let flat = temp
            .path()
            .join(format!("form-cursor-{positioned}-flat.pdf"));
        fixture(&flat, "ffi", positioned, true);
        let input = temp.path().join(format!("form-cursor-{positioned}.pdf"));
        shared_form_fixture(&flat, &input);
        let source = e.open_document(&input).unwrap();
        let before = e.list_occurrence_form_text_runs(&source.id, 0).unwrap();
        assert_eq!(before.runs.len(), 4, "{before:?}");
        for substitute in [None, Some(font.id.as_str())] {
            let edit = e
                .replace_form_text_occurrence(
                    &source.id,
                    &[0],
                    0,
                    &before.runs[0].object_path,
                    "Office",
                    "Ofice",
                    substitute,
                )
                .unwrap()
                .document;
            let output = temp.path().join(format!(
                "form-cursor-{positioned}-{}.pdf",
                substitute.is_some()
            ));
            save(&e, &edit, &output);
            let reopened = e.open_document(&output).unwrap();
            let after = e.list_occurrence_form_text_runs(&reopened.id, 0).unwrap();
            assert_eq!(after.runs.len(), 4, "{after:?}");
            assert_eq!(after.runs[0].text, "Ofice");
            for (actual, original) in after.runs.iter().skip(1).zip(before.runs.iter().skip(1)) {
                assert_eq!(actual, original);
            }
            for id in [&edit.id, &reopened.id] {
                e.close_document(id).unwrap();
            }
        }
        e.close_document(&source.id).unwrap();
    }
}

fn grouped_fixture(flat: &Path, target: &Path, cuts: &[usize]) {
    let mut pdf = Document::load(flat).unwrap();
    let page = *pdf.get_pages().values().next().unwrap();
    let mut content = Content::decode(&pdf.get_page_content_with_limit(page, 16 * 1024 * 1024).unwrap()).unwrap();
    let index = content.operations.iter().position(|op| op.operator == "Tj").unwrap();
    let raw = content.operations[index].operands[0].as_str().unwrap().to_vec();
    let mut offset = 0;
    let mut shows = Vec::new();
    for end in cuts.iter().copied().chain(std::iter::once(raw.len())) {
        assert!(end > offset && end <= raw.len() && end % 2 == 0);
        shows.push(Operation::new("Tj", vec![Object::string_literal(raw[offset..end].to_vec())]));
        offset = end;
    }
    content.operations.splice(index..=index, shows);
    let stream = pdf.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    pdf.get_dictionary_mut(page).unwrap().set("Contents", stream);
    pdf.save(target).unwrap();
    retain(target);
}

#[test]
fn ligature_groups_preserve_exact_source_atoms_across_piece_boundaries() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for (case, cuts) in [("later", vec![2]), ("first", vec![4]), ("many", vec![2, 4, 6])] {
        let flat = temp.path().join(format!("group-{case}-flat.pdf"));
        fixture(&flat, "ffi", false, false);
        let input = temp.path().join(format!("group-{case}.pdf"));
        grouped_fixture(&flat, &input, &cuts);
        let original = fs::read(&input).unwrap();
        let source = e.open_document(&input).unwrap();
        let count = cuts.len() + 1;
        let indices: Vec<_> = (0..count).collect();
        let group = e.inspect_text_group(&source.id, 0, &indices).unwrap();
        assert_eq!(group.text, "Office");
        let noop = e.replace_text_group(&source.id, 0, &indices, &group.text, &group.text).unwrap();
        assert!(e.render_page(&source.id, 0, 800).unwrap() == e.render_page(&noop.id, 0, 800).unwrap(), "Grouped no-op changed pixels");
        let noop_output = temp.path().join(format!("group-{case}-noop.pdf"));
        save(&e, &noop, &noop_output);
        e.close_document(&noop.id).unwrap();
        for text in ["Office this", "Ofice"] {
            let edit = e.replace_text_group(&source.id, 0, &indices, &group.text, text).unwrap();
            let output = temp.path().join(format!("group-{case}-{text}.pdf"));
            save(&e, &edit, &output);
            let reopened = e.open_document(&output).unwrap();
            let after = e.list_text_runs(&reopened.id, 0).unwrap().runs;
            assert_eq!(after[0].text, text);
            assert!(after[0].supported, "{after:?}");
            assert_eq!(embedded_programs(&original), embedded_programs(&e.source_bytes(&reopened.id).unwrap()));
            let again = e.replace_text(&reopened.id, 0, after[0].object_index, &after[0].text, &format!("{text} this")).unwrap();
            assert_eq!(e.list_text_runs(&again.id, 0).unwrap().runs[0].text, format!("{text} this"));
            for id in [&edit.id, &reopened.id, &again.id] { e.close_document(id).unwrap(); }
        }
        assert_eq!(fs::read(&input).unwrap(), original);
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn ligature_groups_with_sequence_only_fonts_do_not_need_scalar_glyphs() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let flat = temp.path().join("group-sequence-flat.pdf");
    fixture(&flat, "ffi", false, false);
    let restricted = temp.path().join("group-sequence-restricted.pdf");
    altered(&flat, &restricted, "sequence-only");
    let mut pdf = Document::load(&restricted).unwrap();
    let page = *pdf.get_pages().values().next().unwrap();
    let mut content = Content::decode(&pdf.get_page_content_with_limit(page, 16 * 1024 * 1024).unwrap()).unwrap();
    let show = content.operations.iter_mut().find(|op| op.operator == "Tj").unwrap();
    let code = show.operands[0].as_str().unwrap().to_vec();
    show.operands[0] = Object::string_literal([code.as_slice(), code.as_slice()].concat());
    let stream = pdf.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    pdf.get_dictionary_mut(page).unwrap().set("Contents", stream);
    let doubled = temp.path().join("group-sequence-doubled.pdf");
    pdf.save(&doubled).unwrap();
    let input = temp.path().join("group-sequence.pdf");
    grouped_fixture(&doubled, &input, &[2]);
    let source = e.open_document(&input).unwrap();
    let selected = e.inspect_text_group(&source.id, 0, &[0, 1]).unwrap();
    assert_eq!(selected.text, "ffiffi");
    let noop = e.replace_text_group(&source.id, 0, &[0, 1], "ffiffi", "ffiffi").unwrap();
    assert!(e.render_page(&source.id, 0, 800).unwrap() == e.render_page(&noop.id, 0, 800).unwrap());
    let changed = e.replace_text_group(&source.id, 0, &[0, 1], "ffiffi", "ffi").unwrap();
    let output = temp.path().join("group-sequence-ffi.pdf");
    save(&e, &changed, &output);
    let reopened = e.open_document(&output).unwrap();
    let runs = e.list_text_runs(&reopened.id, 0).unwrap().runs;
    assert_eq!(runs[0].text, "ffi");
    assert!(runs[0].supported, "{runs:?}");
    assert!(e.replace_text_group(&source.id, 0, &[0, 1], "ffiffi", "fi").is_err());
    for id in [&source.id, &noop.id, &changed.id, &reopened.id] { e.close_document(id).unwrap(); }
}

fn paragraph_request(e: &PdfEngine, source: &DocumentInfo, text: Option<&str>) -> folio_engine::StyledParagraphLayoutRequest {
    let selected = e.inspect_styled_paragraph(&source.id, 0, &[0]).unwrap();
    folio_engine::StyledParagraphLayoutRequest {
        baseline_anchor: selected.baseline_anchor,
        object_indices: selected.object_indices,
        expected_text: selected.text,
        runs: text.map_or(selected.runs, |text| vec![folio_engine::StyledParagraphRun {text: text.into(), style_id: 0}]),
        style_changes: vec![], bounds: selected.bounds,
        alignment: selected.alignment.unwrap_or(folio_engine::ParagraphAlignment::Left),
        line_spacing: selected.line_spacing.unwrap_or(1.2),
    }
}

#[test]
fn ligature_paragraph_source_noop_keeps_the_original_physical_glyphs() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for sequence in ["fi", "ffi"] {
        let input = temp.path().join(format!("paragraph-noop-{sequence}.pdf"));
        fixture(&input, sequence, false, false);
        let original = fs::read(&input).unwrap();
        let source = e.open_document(&input).unwrap();
        let request = paragraph_request(&e, &source, None);
        let noop = e.preview_styled_paragraph(&source.id, 0, &request).unwrap();
        assert!(e.render_page(&source.id, 0, 800).unwrap() == e.render_page(&noop.id, 0, 800).unwrap(), "Paragraph no-op decomposed or moved a source glyph");
        let output = temp.path().join(format!("paragraph-noop-{sequence}-saved.pdf"));
        save(&e, &noop, &output);
        assert_eq!(embedded_programs(&original), embedded_programs(&fs::read(&output).unwrap()));
        assert_eq!(fs::read(&input).unwrap(), original);
        e.close_document(&noop.id).unwrap();
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn ligature_paragraph_sequence_only_font_uses_actual_samples_and_hard_breaks() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let flat = temp.path().join("paragraph-sequence-flat.pdf");
    fixture(&flat, "ffi", false, false);
    let input = temp.path().join("paragraph-sequence.pdf");
    altered(&flat, &input, "sequence-only");
    let source = e.open_document(&input).unwrap();
    let mut request = paragraph_request(&e, &source, Some("ffiffi\nffi"));
    request.bounds.width = 120.;
    request.bounds.height = 160.;
    let edited = e.preview_styled_paragraph(&source.id, 0, &request).unwrap();
    let output = temp.path().join("paragraph-sequence-saved.pdf");
    save(&e, &edited, &output);
    let reopened = e.open_document(&output).unwrap();
    let indices: Vec<_> = e.list_text_runs(&reopened.id, 0).unwrap().runs.iter()
        .filter(|r| r.text != "Neighbor safe").map(|r| r.object_index).collect();
    let replay = e.inspect_styled_paragraph(&reopened.id, 0, &indices).unwrap();
    assert_eq!(replay.text, "ffiffi\nffi");
    let next = folio_engine::StyledParagraphLayoutRequest {
        baseline_anchor: replay.baseline_anchor, object_indices: replay.object_indices,
        expected_text: replay.text, runs: replay.runs, style_changes: vec![], bounds: replay.bounds,
        alignment: replay.alignment.unwrap(), line_spacing: replay.line_spacing.unwrap(),
    };
    let noop = e.preview_styled_paragraph(&reopened.id, 0, &next).unwrap();
    assert!(e.render_page(&reopened.id, 0, 800).unwrap() == e.render_page(&noop.id, 0, 800).unwrap());
    for id in [&source.id, &edited.id, &reopened.id, &noop.id] { e.close_document(id).unwrap(); }
}

#[test]
fn ligature_paragraph_wrap_measures_each_encoded_occurrence_instead_of_letters() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let flat = temp.path().join("paragraph-wrap-flat.pdf");
    fixture(&flat, "ffi", false, false);
    let mut pdf = Document::load(&flat).unwrap();
    let page = *pdf.get_pages().values().next().unwrap();
    let mut content = Content::decode(&pdf.get_page_content_with_limit(page, 16 * 1024 * 1024).unwrap()).unwrap();
    let show = content.operations.iter_mut().find(|op| op.operator == "Tj").unwrap();
    let code = show.operands[0].as_str().unwrap().to_vec();
    let face = ttf_parser::Face::parse(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf"), 0).unwrap();
    let space = face.glyph_index(' ').unwrap().0.to_be_bytes();
    show.operands[0] = Object::string_literal([code.as_slice(), space.as_slice(), code.as_slice()].concat());
    let stream = pdf.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    pdf.get_dictionary_mut(page).unwrap().set("Contents", stream);
    let input = temp.path().join("paragraph-wrap.pdf");
    pdf.save(&input).unwrap(); retain(&input);
    let source = e.open_document(&input).unwrap();
    let mut request = paragraph_request(&e, &source, None);
    assert_eq!(request.expected_text, "Office Office");
    let scale = 24. / face.units_per_em() as f32;
    let width = |s: &str| s.chars().map(|c| f32::from(face.glyph_hor_advance(face.glyph_index(c).unwrap()).unwrap()) * scale).sum::<f32>();
    let actual = width("O\u{fb03}ce");
    let decomposed = width("Office");
    assert!(actual < decomposed);
    request.bounds.width = (actual + decomposed) * 0.5;
    request.bounds.height = 160.;
    let layout = e.layout_styled_paragraph(&source.id, 0, &request).unwrap();
    assert_eq!(layout.lines.len(), 2);
    let edited = e.preview_styled_paragraph(&source.id, 0, &request).unwrap();
    let output = temp.path().join("paragraph-wrap-saved.pdf");
    save(&e, &edited, &output);
    let reopened = e.open_document(&output).unwrap();
    let indices: Vec<_> = e.list_text_runs(&reopened.id, 0).unwrap().runs.iter()
        .filter(|r| r.text != "Neighbor safe").map(|r| r.object_index).collect();
    let replay = e.inspect_styled_paragraph(&reopened.id, 0, &indices).unwrap();
    assert_eq!(replay.text, "Office Office");
    assert_eq!(embedded_programs(&fs::read(&input).unwrap()), embedded_programs(&fs::read(&output).unwrap()));
    for id in [&source.id, &edited.id, &reopened.id] { e.close_document(id).unwrap(); }
}

#[test]
fn ligature_paragraph_style_boundary_uses_proved_scalar_decomposition() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("paragraph-style.pdf");
    fixture(&input, "ffi", false, false);
    let source = e.open_document(&input).unwrap();
    let mut request = paragraph_request(&e, &source, None);
    request.bounds.width = 140.; request.bounds.height = 100.;
    request.runs = vec![folio_engine::StyledParagraphRun {text:"Of".into(), style_id:0},
                       folio_engine::StyledParagraphRun {text:"fice".into(), style_id:1}];
    request.style_changes = vec![folio_engine::StyledParagraphStyleChange {
        style_id:1, source_style_id:0, font_size:None, color:Some("#0044ff".into()), font_id:None,
    }];
    let edited = e.preview_styled_paragraph(&source.id, 0, &request).unwrap();
    let output = temp.path().join("paragraph-style-saved.pdf");
    save(&e, &edited, &output);
    let reopened = e.open_document(&output).unwrap();
    let after = e.list_text_runs(&reopened.id, 0).unwrap().runs;
    assert_eq!(after.iter().filter(|r| r.text != "Neighbor safe").map(|r| r.text.as_str()).collect::<String>(), "Office");
    assert_eq!(after[0].text, "Of"); assert_eq!(after[1].text, "fice");
    let inspected = e.inspect_styled_paragraph(&reopened.id, 0, &[after[0].object_index, after[1].object_index]).unwrap();
    assert_eq!(inspected.styles.len(), 2);
    assert_ne!(inspected.styles[0].color, inspected.styles[1].color);
    assert_eq!(after[0].font_name, after[1].font_name);
    for id in [&source.id, &edited.id, &reopened.id] { e.close_document(id).unwrap(); }
}

#[test]
fn ligature_paragraph_mixed_occurrences_never_borrow_a_ligatures_width() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let flat = temp.path().join("paragraph-mixed-flat.pdf");
    fixture(&flat, "ffi", false, false);
    let mut pdf = Document::load(&flat).unwrap();
    let page = *pdf.get_pages().values().next().unwrap();
    let mut content = Content::decode(&pdf.get_page_content_with_limit(page, 16 * 1024 * 1024).unwrap()).unwrap();
    let show = content.operations.iter_mut().find(|op| op.operator == "Tj").unwrap();
    let mut raw = show.operands[0].as_str().unwrap().to_vec();
    let face = ttf_parser::Face::parse(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf"), 0).unwrap();
    raw.extend(" Office".chars().flat_map(|c| face.glyph_index(c).unwrap().0.to_be_bytes()));
    show.operands[0] = Object::string_literal(raw);
    let stream = pdf.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    pdf.get_dictionary_mut(page).unwrap().set("Contents", stream);
    let input = temp.path().join("paragraph-mixed.pdf");
    pdf.save(&input).unwrap(); retain(&input);
    let source = e.open_document(&input).unwrap();
    let original = e.source_bytes(&source.id).unwrap();
    let mut request = paragraph_request(&e, &source, None);
    assert_eq!(request.expected_text, "Office Office");
    let scale = 24. / face.units_per_em() as f32;
    let width = |s: &str| s.chars().map(|c| f32::from(face.glyph_hor_advance(face.glyph_index(c).unwrap()).unwrap()) * scale).sum::<f32>();
    request.bounds.width = (width("O\u{fb03}ce") + width("Office")) * 0.5;
    request.bounds.height = 160.;
    assert!(e.layout_styled_paragraph(&source.id, 0, &request).is_err(), "The wider scalar occurrence borrowed the ligature's narrower width");
    assert_eq!(e.source_bytes(&source.id).unwrap(), original);
    e.close_document(&source.id).unwrap();
}

#[test]
fn ligature_paragraph_formatting_keeps_later_scalar_words_and_exact_source_rgb() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for precise in [false,true] {
        let flat=temp.path().join(format!("paragraph-format-{precise}-flat.pdf"));
        fixture(&flat,"ffi",false,false);
        let mut pdf=Document::load(&flat).unwrap();let page=*pdf.get_pages().values().next().unwrap();
        let mut content=Content::decode(&pdf.get_page_content_with_limit(page,16*1024*1024).unwrap()).unwrap();
        let index=content.operations.iter().position(|op|op.operator=="Tj").unwrap();
        let first=content.operations[index].operands[0].as_str().unwrap().to_vec();
        let face=ttf_parser::Face::parse(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf"),0).unwrap();
        let scalar=|text:&str|text.chars().flat_map(|c|face.glyph_index(c).unwrap().0.to_be_bytes()).collect::<Vec<_>>();
        let first_space=[first.as_slice(),scalar(" ").as_slice()].concat();
        let second=scalar("Office");
        let mut ops=Vec::new();
        if precise {ops.push(Operation::new("rg",vec![Object::Real(0.1),Object::Real(0.2),Object::Real(0.3)]));}
        ops.push(Operation::new("Tj",vec![Object::string_literal(first_space.clone())]));
        if precise {ops.push(Operation::new("rg",vec![Object::Real(0.1001),Object::Real(0.2),Object::Real(0.3)]));}
        ops.push(Operation::new("Tj",vec![Object::string_literal(second.clone())]));
        content.operations.splice(index..=index,ops);
        let stream=pdf.add_object(Stream::new(dictionary!{},content.encode().unwrap()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);
        let input=temp.path().join(format!("paragraph-format-{precise}.pdf"));pdf.save(&input).unwrap();retain(&input);
        let source=e.open_document(&input).unwrap();let original=e.source_bytes(&source.id).unwrap();
        let selected=e.inspect_styled_paragraph(&source.id,0,&[0,1]).unwrap();
        assert_eq!(selected.text,"Office Office");
        assert_eq!(selected.styles.len(),if precise{2}else{1});
        let mut bounds=selected.bounds;bounds.width=250.;bounds.height=100.;
        let runs=if precise{selected.runs}else{vec![folio_engine::StyledParagraphRun{text:"Office ".into(),style_id:0},folio_engine::StyledParagraphRun{text:"Office".into(),style_id:1}]};
        let changes=if precise{vec![]}else{vec![folio_engine::StyledParagraphStyleChange{style_id:1,source_style_id:0,font_size:None,color:Some("#0044ff".into()),font_id:None}]};
        let request=folio_engine::StyledParagraphLayoutRequest{baseline_anchor:selected.baseline_anchor,object_indices:vec![0,1],expected_text:selected.text,runs,style_changes:changes,bounds,alignment:folio_engine::ParagraphAlignment::Left,line_spacing:1.2};
        let edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();
        let output=temp.path().join(format!("paragraph-format-{precise}-saved.pdf"));save(&e,&edited,&output);
        let saved=Document::load(&output).unwrap();let id=*saved.get_pages().values().next().unwrap();
        let content=Content::decode(&saved.get_page_content_with_limit(id,16*1024*1024).unwrap()).unwrap();
        let shows=content.operations.iter().filter(|op|op.operator=="Tj").take(2).map(|op|op.operands[0].as_str().unwrap().to_vec()).collect::<Vec<_>>();
        assert_eq!(shows,[first_space,second],"Formatting changed original per-occurrence glyph codes");
        if precise {assert!(e.render_page(&source.id,0,800).unwrap()==e.render_page(&edited.id,0,800).unwrap(),"Exact original RGB or glyph positions changed");}
        let reopened=e.open_document(&output).unwrap();let replay=e.inspect_styled_paragraph(&reopened.id,0,&[0,1]).unwrap();
        let next=folio_engine::StyledParagraphLayoutRequest{baseline_anchor:replay.baseline_anchor,object_indices:replay.object_indices,expected_text:replay.text,runs:replay.runs,style_changes:vec![],bounds:replay.bounds,alignment:replay.alignment.unwrap(),line_spacing:replay.line_spacing.unwrap()};
        let noop=e.preview_styled_paragraph(&reopened.id,0,&next).unwrap();
        assert!(e.render_page(&reopened.id,0,800).unwrap()==e.render_page(&noop.id,0,800).unwrap());
        assert_eq!(e.source_bytes(&source.id).unwrap(),original);
        for id in [&source.id,&edited.id,&reopened.id,&noop.id]{e.close_document(id).unwrap();}
    }
}


fn replay_paragraph_request(e:&PdfEngine,source:&DocumentInfo)->folio_engine::StyledParagraphLayoutRequest {
    let indices=e.list_text_runs(&source.id,0).unwrap().runs.iter().filter(|r|r.text!="Neighbor safe").map(|r|r.object_index).collect::<Vec<_>>();
    let p=e.inspect_styled_paragraph(&source.id,0,&indices).unwrap();
    folio_engine::StyledParagraphLayoutRequest{baseline_anchor:p.baseline_anchor,object_indices:p.object_indices,expected_text:p.text,runs:p.runs,style_changes:vec![],bounds:p.bounds,alignment:p.alignment.unwrap(),line_spacing:p.line_spacing.unwrap()}
}

#[test]
fn ligature_paragraph_growth_deletion_and_repeated_saved_edits_keep_original_font() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for sequence in ["fi","ffi"] {
        let input=temp.path().join(format!("paragraph-edit-{sequence}.pdf"));fixture(&input,sequence,false,false);
        let source=e.open_document(&input).unwrap();let original=e.source_bytes(&source.id).unwrap();
        for (case,text) in [("growth","Office this Office"),("deletion","Ofice"),("hardbreak","Office\nthis")] {
            let mut request=paragraph_request(&e,&source,Some(text));request.bounds.width=140.;request.bounds.height=160.;
            let edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();
            let output=temp.path().join(format!("paragraph-edit-{sequence}-{case}.pdf"));save(&e,&edited,&output);
            let reopened=e.open_document(&output).unwrap();let mut replay=replay_paragraph_request(&e,&reopened);
            assert_eq!(replay.expected_text,text);
            let noop=e.preview_styled_paragraph(&reopened.id,0,&replay).unwrap();
            assert!(e.render_page(&reopened.id,0,800).unwrap()==e.render_page(&noop.id,0,800).unwrap());
            assert_eq!(embedded_programs(&original),embedded_programs(&fs::read(&output).unwrap()));
            replay.runs=vec![folio_engine::StyledParagraphRun{text:format!("{text} this"),style_id:0}];
            let again=e.preview_styled_paragraph(&reopened.id,0,&replay).unwrap();
            let again_output=temp.path().join(format!("paragraph-edit-{sequence}-{case}-again.pdf"));save(&e,&again,&again_output);
            let last=e.open_document(&again_output).unwrap();assert_eq!(replay_paragraph_request(&e,&last).expected_text,format!("{text} this"));
            for id in [&edited.id,&reopened.id,&noop.id,&again.id,&last.id]{e.close_document(id).unwrap();}
        }
        assert_eq!(e.source_bytes(&source.id).unwrap(),original);e.close_document(&source.id).unwrap();
    }
}

#[test]
fn ligature_paragraph_without_internal_cmap_uses_proved_gids_through_saved_edits() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    let flat=temp.path().join("paragraph-mapfree-flat.pdf");fixture(&flat,"ffi",false,false);
    let input=temp.path().join("paragraph-mapfree.pdf");altered(&flat,&input,"no-cmap");
    let source=e.open_document(&input).unwrap();let original=e.source_bytes(&source.id).unwrap();
    let mut request=paragraph_request(&e,&source,Some("Office this Office"));request.bounds.width=140.;request.bounds.height=160.;
    let edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();
    let output=temp.path().join("paragraph-mapfree-saved.pdf");save(&e,&edited,&output);
    let reopened=e.open_document(&output).unwrap();let mut replay=replay_paragraph_request(&e,&reopened);
    assert_eq!(replay.expected_text,"Office this Office");
    let noop=e.preview_styled_paragraph(&reopened.id,0,&replay).unwrap();
    assert!(e.render_page(&reopened.id,0,800).unwrap()==e.render_page(&noop.id,0,800).unwrap());
    replay.runs[0].text.push_str(" this");let again=e.preview_styled_paragraph(&reopened.id,0,&replay).unwrap();
    assert_eq!(embedded_programs(&original),embedded_programs(&e.source_bytes(&again.id).unwrap()));
    for id in [&source.id,&edited.id,&reopened.id,&noop.id,&again.id]{e.close_document(id).unwrap();}
}

#[test]
fn ligature_paragraph_explicit_substitution_is_self_contained_and_original_refusal_is_immutable() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    let flat=temp.path().join("paragraph-substitute-flat.pdf");fixture(&flat,"ffi",false,false);
    let input=temp.path().join("paragraph-substitute.pdf");altered(&flat,&input,"sequence-only");
    let source=e.open_document(&input).unwrap();let original=e.source_bytes(&source.id).unwrap();
    let mut request=paragraph_request(&e,&source,Some("Office this"));request.bounds.width=180.;request.bounds.height=130.;
    assert!(e.preview_styled_paragraph(&source.id,0,&request).is_err());assert_eq!(e.source_bytes(&source.id).unwrap(),original);
    let font=e.fonts().register(include_bytes!("../../tests/fixtures/corpus/fonts/DejaVuSerif.ttf").to_vec()).unwrap();
    request.style_changes=vec![folio_engine::StyledParagraphStyleChange{style_id:0,source_style_id:0,font_size:None,color:None,font_id:Some(font.id.clone())}];
    let edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();
    let output=temp.path().join("paragraph-substitute-saved.pdf");save(&e,&edited,&output);
    e.close_document(&source.id).unwrap();e.close_document(&edited.id).unwrap();
    let reopened=e.open_document(&output).unwrap();let replay=replay_paragraph_request(&e,&reopened);
    assert_eq!(replay.expected_text,"Office this");
    assert!(e.list_text_runs(&reopened.id,0).unwrap().runs[0].font_name.contains(&font.id));
    let noop=e.preview_styled_paragraph(&reopened.id,0,&replay).unwrap();
    assert!(e.render_page(&reopened.id,0,800).unwrap()==e.render_page(&noop.id,0,800).unwrap());
    e.close_document(&noop.id).unwrap();e.close_document(&reopened.id).unwrap();
}

#[test]
fn ligature_paragraph_positioned_noop_keeps_original_glyph_origins() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for sequence in ["fi","ffi"] {
        let input=temp.path().join(format!("paragraph-positioned-{sequence}.pdf"));fixture(&input,sequence,true,false);retain(&input);
        let source=e.open_document(&input).unwrap();let original=e.source_bytes(&source.id).unwrap();
        let request=paragraph_request(&e,&source,None);
        let noop=e.preview_styled_paragraph(&source.id,0,&request).unwrap();
        assert!(e.render_page(&source.id,0,1000).unwrap()==e.render_page(&noop.id,0,1000).unwrap(),"Unchanged positioned paragraph moved its original glyphs");
        let output=temp.path().join(format!("paragraph-positioned-{sequence}-saved.pdf"));save(&e,&noop,&output);
        assert_eq!(e.source_bytes(&source.id).unwrap(),original);
        e.close_document(&noop.id).unwrap();e.close_document(&source.id).unwrap();
    }
}

#[test]
fn ligature_paragraph_highlight_wrap_growth_and_saved_reedit_keep_source_atoms() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for sequence in ["fi","ffi"] {
        let flat=temp.path().join(format!("paragraph-highlight-{sequence}-flat.pdf"));fixture(&flat,sequence,false,false);
        let base=e.open_document(&flat).unwrap();let bounds=e.list_text_runs(&base.id,0).unwrap().runs[0].bounds.clone();
        e.close_document(&base.id).unwrap();
        let mut pdf=Document::load(&flat).unwrap();let page=*pdf.get_pages().values().next().unwrap();
        let mut content=Content::decode(&pdf.get_page_content_with_limit(page,16*1024*1024).unwrap()).unwrap();
        let mark=vec![Operation::new("q",vec![]),Operation::new("rg",vec![1.into(),1.into(),0.into()]),Operation::new("re",vec![(bounds.x-2.).into(),(400.-bounds.y-bounds.height-2.).into(),(bounds.width+4.).into(),(bounds.height+4.).into()]),Operation::new("f",vec![]),Operation::new("Q",vec![])];
        content.operations.splice(0..0,mark);let stream=pdf.add_object(Stream::new(dictionary!{},content.encode().unwrap()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);
        let input=temp.path().join(format!("paragraph-highlight-{sequence}.pdf"));pdf.save(&input).unwrap();retain(&input);
        let source=e.open_document(&input).unwrap();let original=e.source_bytes(&source.id).unwrap();
        let inspected=e.inspect_styled_paragraph(&source.id,0,&[1]).unwrap();assert!(inspected.styles[0].background_color.is_some());
        let mut request=folio_engine::StyledParagraphLayoutRequest{baseline_anchor:inspected.baseline_anchor,object_indices:inspected.object_indices,expected_text:inspected.text,runs:vec![folio_engine::StyledParagraphRun{text:"Office Office\nthis".into(),style_id:0}],style_changes:vec![],bounds:inspected.bounds,alignment:folio_engine::ParagraphAlignment::Left,line_spacing:1.2};
        request.bounds.width=100.;request.bounds.height=160.;
        let edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();
        let output=temp.path().join(format!("paragraph-highlight-{sequence}-saved.pdf"));save(&e,&edited,&output);
        let reopened=e.open_document(&output).unwrap();let replay=replay_paragraph_request(&e,&reopened);
        assert_eq!(replay.expected_text,"Office Office\nthis");
        let noop=e.preview_styled_paragraph(&reopened.id,0,&replay).unwrap();
        assert!(e.render_page(&reopened.id,0,1000).unwrap()==e.render_page(&noop.id,0,1000).unwrap());
        assert_eq!(embedded_programs(&original),embedded_programs(&fs::read(&output).unwrap()));
        assert_eq!(e.source_bytes(&source.id).unwrap(),original);
        for id in [&source.id,&edited.id,&reopened.id,&noop.id]{e.close_document(id).unwrap();}
    }
}

#[test]
fn ligature_positioned_identity_requires_unchanged_frame_text_style_and_baseline() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("paragraph-identity-controls.pdf");fixture(&input,"ffi",true,false);
    let source=e.open_document(&input).unwrap();let original=e.source_bytes(&source.id).unwrap();
    let mut request=paragraph_request(&e,&source,None);
    request.expected_text.push('!');assert!(e.preview_styled_paragraph(&source.id,0,&request).is_err());
    request=paragraph_request(&e,&source,None);request.style_changes=vec![folio_engine::StyledParagraphStyleChange{style_id:0,source_style_id:0,font_size:None,color:None,font_id:Some("missing-font".into())}];
    assert!(e.preview_styled_paragraph(&source.id,0,&request).is_err());
    request=paragraph_request(&e,&source,None);request.runs=vec![folio_engine::StyledParagraphRun{text:"O".into(),style_id:0},folio_engine::StyledParagraphRun{text:"ffice".into(),style_id:0}];
    let segmented=e.preview_styled_paragraph(&source.id,0,&request).unwrap();assert!(e.render_page(&source.id,0,1000).unwrap()==e.render_page(&segmented.id,0,1000).unwrap());
    for case in 0..4 {
        let mut changed=paragraph_request(&e,&source,None);changed.bounds.width=160.;changed.bounds.height=100.;
        match case {
            0=>changed.runs[0].text.push_str(" this"),
            1=>changed.bounds.x+=8.,
            2=>changed.baseline_anchor.as_mut().unwrap().y+=8.,
            _=>changed.style_changes=vec![folio_engine::StyledParagraphStyleChange{style_id:0,source_style_id:0,font_size:None,color:Some("#0044FF".into()),font_id:None}],
        }
        let edited=e.preview_styled_paragraph(&source.id,0,&changed).unwrap();assert!(e.render_page(&source.id,0,1000).unwrap()!=e.render_page(&edited.id,0,1000).unwrap(),"Changed request was mistaken for source identity");
        e.close_document(&edited.id).unwrap();
    }
    assert_eq!(e.source_bytes(&source.id).unwrap(),original);e.close_document(&segmented.id).unwrap();e.close_document(&source.id).unwrap();
}

#[test]
fn ligature_highlight_mixed_size_color_and_later_scalar_occurrences_survive_reedit() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let flat=temp.path().join("paragraph-highlight-mixed-flat.pdf");fixture(&flat,"ffi",false,false);
    let base=e.open_document(&flat).unwrap();let bounds=e.list_text_runs(&base.id,0).unwrap().runs[0].bounds.clone();e.close_document(&base.id).unwrap();
    let mut pdf=Document::load(&flat).unwrap();let page=*pdf.get_pages().values().next().unwrap();let mut content=Content::decode(&pdf.get_page_content_with_limit(page,16*1024*1024).unwrap()).unwrap();
    content.operations.splice(0..0,vec![Operation::new("q",vec![]),Operation::new("rg",vec![1.into(),1.into(),0.into()]),Operation::new("re",vec![(bounds.x-2.).into(),(400.-bounds.y-bounds.height-2.).into(),(bounds.width+4.).into(),(bounds.height+4.).into()]),Operation::new("f",vec![]),Operation::new("Q",vec![])]);
    let stream=pdf.add_object(Stream::new(dictionary!{},content.encode().unwrap()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);
    let input=temp.path().join("paragraph-highlight-mixed.pdf");pdf.save(&input).unwrap();retain(&input);
    let source=e.open_document(&input).unwrap();let selected=e.inspect_styled_paragraph(&source.id,0,&[1]).unwrap();
    let request=folio_engine::StyledParagraphLayoutRequest{baseline_anchor:selected.baseline_anchor,object_indices:selected.object_indices,expected_text:selected.text,runs:vec![folio_engine::StyledParagraphRun{text:"Office ".into(),style_id:0},folio_engine::StyledParagraphRun{text:"Office\nthis".into(),style_id:1}],style_changes:vec![folio_engine::StyledParagraphStyleChange{style_id:1,source_style_id:0,font_size:Some(18.),color:Some("#0044FF".into()),font_id:None}],bounds:folio_engine::AnnotationRect{x:70.,y:90.,width:140.,height:160.},alignment:folio_engine::ParagraphAlignment::Left,line_spacing:1.3};
    let edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();let output=temp.path().join("paragraph-highlight-mixed-saved.pdf");save(&e,&edited,&output);
    let reopened=e.open_document(&output).unwrap();let replay=replay_paragraph_request(&e,&reopened);assert_eq!(replay.expected_text,"Office Office\nthis");
    let noop=e.preview_styled_paragraph(&reopened.id,0,&replay).unwrap();assert!(e.render_page(&reopened.id,0,1000).unwrap()==e.render_page(&noop.id,0,1000).unwrap());
    assert_eq!(replay.runs.iter().map(|r|r.style_id).collect::<Vec<_>>(),[0,1]);
    for id in [&source.id,&edited.id,&reopened.id,&noop.id]{e.close_document(id).unwrap();}
}

#[test]
fn ligature_paragraph_native_occurrences_keep_physical_rotation_scale_and_replay() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for rotation in [0,90,180,270] {for unit in [1.,1.7] {
        let flat=temp.path().join(format!("paragraph-physical-{rotation}-{unit}-flat.pdf"));fixture(&flat,"ffi",false,false);
        let mut pdf=Document::load(&flat).unwrap();let page=*pdf.get_pages().values().next().unwrap();pdf.get_dictionary_mut(page).unwrap().set("Rotate",rotation);pdf.get_dictionary_mut(page).unwrap().set("UserUnit",Object::Real(unit));
        let input=temp.path().join(format!("paragraph-physical-{rotation}-{unit}.pdf"));pdf.save(&input).unwrap();retain(&input);
        let source=e.open_document(&input).unwrap();let original=e.source_bytes(&source.id).unwrap();let request=paragraph_request(&e,&source,None);
        let noop=e.preview_styled_paragraph(&source.id,0,&request).unwrap();assert!(e.render_page(&source.id,0,1000).unwrap()==e.render_page(&noop.id,0,1000).unwrap(),"Unchanged rotated/scaled source glyph moved: {rotation}/{unit}");
        let mut request=paragraph_request(&e,&source,Some("Office this"));
        let [x,y,w,h]=match rotation{0=>[70.,90.,200.,130.],90=>[180.,70.,130.,200.],180=>[130.,180.,200.,130.],_=>[90.,130.,130.,200.]};
        request.bounds=folio_engine::AnnotationRect{x:x*unit,y:y*unit,width:w*unit,height:h*unit};
        let edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();let output=temp.path().join(format!("paragraph-physical-{rotation}-{unit}-saved.pdf"));save(&e,&edited,&output);
        let reopened=e.open_document(&output).unwrap();let replay=replay_paragraph_request(&e,&reopened);assert_eq!(replay.expected_text,"Office this");let again=e.preview_styled_paragraph(&reopened.id,0,&replay).unwrap();
        assert!(e.render_page(&reopened.id,0,1000).unwrap()==e.render_page(&again.id,0,1000).unwrap(),"Re-edit changed rotated/scaled glyphs: {rotation}/{unit}");
        assert_eq!(e.source_bytes(&source.id).unwrap(),original);
        for id in [&source.id,&noop.id,&edited.id,&reopened.id,&again.id]{e.close_document(id).unwrap();}
    }}
}

#[test]
fn ligature_sequence_only_highlight_default_area_fits_unchanged_source() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let flat=temp.path().join("paragraph-small-flat.pdf");fixture(&flat,"ffi",false,false);
    let restricted=temp.path().join("paragraph-small-restricted.pdf");altered(&flat,&restricted,"sequence-only");
    let base=e.open_document(&restricted).unwrap();let bounds=e.list_text_runs(&base.id,0).unwrap().runs[0].bounds.clone();e.close_document(&base.id).unwrap();
    let mut pdf=Document::load(&restricted).unwrap();let page=*pdf.get_pages().values().next().unwrap();let mut content=Content::decode(&pdf.get_page_content_with_limit(page,16*1024*1024).unwrap()).unwrap();
    content.operations.splice(0..0,vec![Operation::new("q",vec![]),Operation::new("rg",vec![1.into(),1.into(),0.into()]),Operation::new("re",vec![(bounds.x-2.).into(),(400.-bounds.y-bounds.height-2.).into(),(bounds.width+4.).into(),(bounds.height+4.).into()]),Operation::new("f",vec![]),Operation::new("Q",vec![])]);
    let stream=pdf.add_object(Stream::new(dictionary!{},content.encode().unwrap()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);let input=temp.path().join("paragraph-small-highlight.pdf");pdf.save(&input).unwrap();retain(&input);
    let source=e.open_document(&input).unwrap();let s=e.inspect_styled_paragraph(&source.id,0,&[1]).unwrap();assert_eq!(s.text,"ffi");assert!(s.styles[0].background_color.is_some());
    let request=folio_engine::StyledParagraphLayoutRequest{baseline_anchor:s.baseline_anchor,object_indices:s.object_indices,expected_text:s.text,runs:s.runs,style_changes:vec![],bounds:s.bounds,alignment:s.alignment.unwrap_or(folio_engine::ParagraphAlignment::Left),line_spacing:s.line_spacing.unwrap_or(1.2)};
    let layout=e.layout_styled_paragraph(&source.id,0,&request).unwrap();assert_eq!(layout.lines.len(),1);
    let noop=e.preview_styled_paragraph(&source.id,0,&request).unwrap();assert!(e.render_page(&source.id,0,1000).unwrap()==e.render_page(&noop.id,0,1000).unwrap());
    e.close_document(&noop.id).unwrap();e.close_document(&source.id).unwrap();
}

#[test]
fn ligature_reassigned_to_character_cell_highlight_reserves_one_physical_advance() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let flat=temp.path().join("paragraph-cell-flat.pdf");fixture(&flat,"ffi",false,false);
    let mut pdf=Document::load(&flat).unwrap();let page=*pdf.get_pages().values().next().unwrap();let mut content=Content::decode(&pdf.get_page_content_with_limit(page,16*1024*1024).unwrap()).unwrap();
    let face=ttf_parser::Face::parse(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf"),0).unwrap();
    let enc=|s:&str|s.chars().flat_map(|c|face.glyph_index(c).unwrap().0.to_be_bytes()).collect::<Vec<_>>();
    let index=content.operations.iter().position(|op|op.operator=="Tj").unwrap();let mut first=content.operations[index].operands[0].as_str().unwrap().to_vec();first.extend(enc(" "));
    let advance=|data:&[u8]|data.chunks_exact(2).map(|v|face.glyph_hor_advance(ttf_parser::GlyphId(u16::from_be_bytes([v[0],v[1]]))).unwrap()as f32*24./face.units_per_em()as f32).sum::<f32>();
    let x=72.+advance(&first);let w=advance(&enc("ff"));
    content.operations.splice(index..=index,vec![Operation::new("Tj",vec![Object::string_literal(first)]),Operation::new("Tj",vec![Object::string_literal(enc("ff"))])]);
    // f has a genuine right overhang; a complete-cell source mark is not an
    // enclosing-ink rectangle and must retain its independently proved policy.
    content.operations.splice(0..0,vec![Operation::new("q",vec![]),Operation::new("rg",vec![1.into(),1.into(),0.into()]),Operation::new("re",vec![x.into(),279.into(),w.into(),21.into()]),Operation::new("f",vec![]),Operation::new("Q",vec![])]);
    let stream=pdf.add_object(Stream::new(dictionary!{},content.encode().unwrap()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);let input=temp.path().join("paragraph-cell.pdf");pdf.save(&input).unwrap();retain(&input);
    let source=e.open_document(&input).unwrap();let s=e.inspect_styled_paragraph(&source.id,0,&[1,2]).unwrap();assert_eq!(s.text,"Office ff");assert_eq!(s.styles.len(),2);assert!(s.styles[1].background_color.is_some());
    let request=folio_engine::StyledParagraphLayoutRequest{baseline_anchor:s.baseline_anchor,object_indices:s.object_indices,expected_text:s.text,runs:vec![folio_engine::StyledParagraphRun{text:"O".into(),style_id:0},folio_engine::StyledParagraphRun{text:"ffi".into(),style_id:1},folio_engine::StyledParagraphRun{text:"ce ff".into(),style_id:0}],style_changes:vec![],bounds:folio_engine::AnnotationRect{x:70.,y:90.,width:180.,height:100.},alignment:folio_engine::ParagraphAlignment::Left,line_spacing:1.2};
    let edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();let output=temp.path().join("paragraph-cell-saved.pdf");save(&e,&edited,&output);
    let saved=Document::load(&output).unwrap();let page=*saved.get_pages().values().next().unwrap();let memory:serde_json::Value=serde_json::from_slice(saved.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();assert!(memory["paragraphs"][0]["backgrounds"][0]["cell"].is_object());
    let content=Content::decode(&saved.get_page_content_with_limit(page,16*1024*1024).unwrap()).unwrap();
    let painted=content.operations.windows(5).filter(|ops|ops.iter().zip(["q","rg","re","f","Q"]).all(|(op,name)|op.operator==name)).collect::<Vec<_>>();assert_eq!(painted.len(),1);
    let width=painted[0][2].operands[2].as_float().unwrap();let expected=face.glyph_hor_advance(face.glyph_index('\u{fb03}').unwrap()).unwrap()as f32*24./face.units_per_em()as f32;
    assert!((width-expected).abs()<0.02,"Character-cell mark used expanded letter widths instead of the physical ligature advance: {width} vs {expected}");
    let reopened=e.open_document(&output).unwrap();let replay=replay_paragraph_request(&e,&reopened);let noop=e.preview_styled_paragraph(&reopened.id,0,&replay).unwrap();assert!(e.render_page(&reopened.id,0,1000).unwrap()==e.render_page(&noop.id,0,1000).unwrap());
    for id in [&source.id,&edited.id,&reopened.id,&noop.id]{e.close_document(id).unwrap();}
}

#[test]
fn ligature_positioned_identity_does_not_hide_distinct_source_rgb_attribution() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let flat=temp.path().join("paragraph-exact-paint-flat.pdf");fixture(&flat,"ffi",false,false);
    let mut pdf=Document::load(&flat).unwrap();let page=*pdf.get_pages().values().next().unwrap();let mut content=Content::decode(&pdf.get_page_content_with_limit(page,16*1024*1024).unwrap()).unwrap();
    let face=ttf_parser::Face::parse(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf"),0).unwrap();let enc=|s:&str|s.chars().flat_map(|ch|face.glyph_index(ch).unwrap().0.to_be_bytes()).collect::<Vec<_>>();
    let index=content.operations.iter().position(|op|op.operator=="Tj").unwrap();let mut first=content.operations[index].operands[0].as_str().unwrap().to_vec();first.extend(enc(" "));
    content.operations.splice(index..=index,vec![Operation::new("rg",vec![Object::Real(0.1),Object::Real(0.2),Object::Real(0.3)]),Operation::new("TJ",vec![Object::Array(vec![Object::string_literal(first)])]),Operation::new("rg",vec![Object::Real(0.1001),Object::Real(0.2),Object::Real(0.3)]),Operation::new("Tj",vec![Object::string_literal(enc("Office"))])]);
    let stream=pdf.add_object(Stream::new(dictionary!{},content.encode().unwrap()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);let input=temp.path().join("paragraph-exact-paint.pdf");pdf.save(&input).unwrap();retain(&input);
    let source=e.open_document(&input).unwrap();let original=e.source_bytes(&source.id).unwrap();let s=e.inspect_styled_paragraph(&source.id,0,&[0,1]).unwrap();assert_eq!(s.styles.len(),2);assert_eq!(s.styles[0].color,s.styles[1].color);
    let request=folio_engine::StyledParagraphLayoutRequest{baseline_anchor:s.baseline_anchor,object_indices:s.object_indices,expected_text:s.text.clone(),runs:vec![folio_engine::StyledParagraphRun{text:s.text,style_id:1}],style_changes:vec![],bounds:s.bounds,alignment:s.alignment.unwrap_or(folio_engine::ParagraphAlignment::Left),line_spacing:s.line_spacing.unwrap_or(1.2)};
    let edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();let output=temp.path().join("paragraph-exact-paint-saved.pdf");save(&e,&edited,&output);
    let saved=Document::load(&output).unwrap();let page=*saved.get_pages().values().next().unwrap();let content=Content::decode(&saved.get_page_content_with_limit(page,16*1024*1024).unwrap()).unwrap();
    let mut rgb=[0.;3];let mut stack=vec![];let mut paints=vec![];
    for op in content.operations {match op.operator.as_str(){
        "q"=>stack.push(rgb),"Q"=>rgb=stack.pop().unwrap(),"rg"=>{for (i,v) in op.operands.iter().enumerate(){rgb[i]=v.as_float().unwrap();}},
        "Tj"=>{if op.operands[0].as_str().is_ok_and(|v|!v.is_empty()){paints.push(rgb);}},
        "TJ"=>{if op.operands[0].as_array().unwrap().iter().any(|v|v.as_str().is_ok_and(|s|!s.is_empty())){paints.push(rgb);}},_=>{}
    }}
    assert!(!paints.is_empty());assert!(paints.iter().all(|rgb|*rgb==[0.1001,0.2,0.3]),"Source identity incorrectly ignored a real RGB attribution change: {paints:?}");
    assert_eq!(e.source_bytes(&source.id).unwrap(),original);
    e.close_document(&edited.id).unwrap();e.close_document(&source.id).unwrap();
}

fn scalar_sequence_resource(path:&Path,positioned:bool,spacing:f32) {
    fixture(path,"ffi",false,false);
    let mut pdf=Document::load(path).unwrap();let page=*pdf.get_pages().values().next().unwrap();
    let font=pdf.get_dictionary(page).unwrap().get(b"Resources").unwrap().as_dict().unwrap().get(b"Font").unwrap().as_dict().unwrap().get(b"F1").unwrap().as_reference().unwrap();
    let cmap=pdf.get_dictionary(font).unwrap().get(b"ToUnicode").unwrap().as_reference().unwrap();
    let face=ttf_parser::Face::parse(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf"),0).unwrap();let gid=|c|face.glyph_index(c).unwrap().0;
    let mapping=format!("1 begincodespacerange <0000> <ffff> endcodespacerange 3 beginbfchar <{:04x}> <004f> <{:04x}> <0020> <{:04x}> <006600660069> endbfchar",gid('O'),gid(' '),gid('\u{fb03}'));
    pdf.get_object_mut(cmap).unwrap().as_stream_mut().unwrap().set_content(mapping.into_bytes());
    let mut c=Content::decode(&pdf.get_page_content_with_limit(page,16*1024*1024).unwrap()).unwrap();let first=c.operations.iter().position(|o|o.operator=="Tj").unwrap();let end=c.operations.iter().position(|o|o.operator=="ET").unwrap();
    let scalar=Object::string_literal(gid('O').to_be_bytes().to_vec());let show=if positioned{Operation::new("TJ",vec![Object::Array(vec![scalar])])}else{Operation::new("Tj",vec![scalar])};
    let tail=[gid(' ').to_be_bytes(),gid('O').to_be_bytes()].concat();
    c.operations.splice(first..end,vec![Operation::new("Tc",vec![Object::Real(spacing)]),show,Operation::new("TJ",vec![Object::Array(vec![(-4000).into()])]),Operation::new("Tj",vec![Object::string_literal(tail)])]);
    let stream=pdf.add_object(Stream::new(dictionary!{},c.encode().unwrap()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);pdf.save(path).unwrap();retain(path);
}
fn near_rect(a:&folio_engine::AnnotationRect,b:&folio_engine::AnnotationRect) {
    assert!([a.x-b.x,a.y-b.y,a.width-b.width,a.height-b.height].iter().all(|v|v.abs()<0.02),"Unselected placement changed: {a:?} {b:?}");
}
fn check_scalar_root_sequence(positioned:bool) {
    let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join(format!("review-scalar-root-{positioned}.pdf"));scalar_sequence_resource(&input,positioned,1.5);
    let source=e.open_document(&input).unwrap();let original=e.source_bytes(&source.id).unwrap();let before=e.list_occurrence_text_runs(&source.id,0).unwrap().runs;assert_eq!(before[0].text.trim(),"O");assert!(before[0].supported);
    let edited=e.replace_text_occurrence(&source.id,&[0],0,0,&before[0].text,"Offi",None).unwrap().document;
    let output=temp.path().join(format!("review-scalar-root-{positioned}-saved.pdf"));save(&e,&edited,&output);let reopened=e.open_document(&output).unwrap();let after=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
    assert_eq!(after[0].text.trim(),"Offi");assert_eq!(after.len(),before.len());
    for (a,b)in after.iter().skip(1).zip(before.iter().skip(1)){assert_eq!(a.text,b.text);near_rect(&a.bounds,&b.bounds);}
    assert_eq!(embedded_programs(&original),embedded_programs(&fs::read(&output).unwrap()));assert_eq!(e.source_bytes(&source.id).unwrap(),original);
    let noop=e.replace_text_occurrence(&reopened.id,&[0],0,0,&after[0].text,"Offi",None).unwrap().document;assert!(e.render_page(&reopened.id,0,1000).unwrap()==e.render_page(&noop.id,0,1000).unwrap());
    for id in [&source.id,&edited.id,&reopened.id,&noop.id]{e.close_document(id).unwrap();}
}
#[test]
fn review_i2_scalar_tj_uses_replacement_glyph_count(){check_scalar_root_sequence(false);}
#[test]
fn review_i2_scalar_positioned_uses_complete_sequence_boundaries(){check_scalar_root_sequence(true);}
#[test]
fn review_i2_shared_form_scalar_operand_can_add_sequence() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for positioned in [false,true] {
        let flat=temp.path().join(format!("review-scalar-form-{positioned}-flat.pdf"));scalar_sequence_resource(&flat,positioned,1.5);let input=temp.path().join(format!("review-scalar-form-{positioned}.pdf"));shared_form_fixture(&flat,&input);
        let source=e.open_document(&input).unwrap();let original=e.source_bytes(&source.id).unwrap();let before=e.list_occurrence_form_text_runs(&source.id,0).unwrap().runs;assert_eq!(before.len(),4);assert_eq!(before[0].text,"O");
        let edited=e.replace_form_text_occurrence(&source.id,&[0],0,&before[0].object_path,"O","Offi",None).unwrap().document;
        let output=temp.path().join(format!("review-scalar-form-{positioned}-saved.pdf"));save(&e,&edited,&output);let reopened=e.open_document(&output).unwrap();let after=e.list_occurrence_form_text_runs(&reopened.id,0).unwrap().runs;
        assert_eq!(after[0].text.trim(),"Offi");assert_eq!(after.len(),before.len());for (a,b)in after.iter().skip(1).zip(before.iter().skip(1)){assert_eq!(a.text,b.text);near_rect(&a.bounds,&b.bounds);}
        assert_eq!(e.source_bytes(&source.id).unwrap(),original);for id in [&source.id,&edited.id,&reopened.id]{e.close_document(id).unwrap();}
    }
}
#[test]
fn review_i2_scalar_paragraph_can_encode_complete_sequence() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("review-scalar-paragraph.pdf");scalar_sequence_resource(&input,false,0.);
    let source=e.open_document(&input).unwrap();let original=e.source_bytes(&source.id).unwrap();let mut request=paragraph_request(&e,&source,Some("Offi"));request.bounds=folio_engine::AnnotationRect{x:70.,y:90.,width:100.,height:100.};
    let edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();let output=temp.path().join("review-scalar-paragraph-saved.pdf");save(&e,&edited,&output);let reopened=e.open_document(&output).unwrap();
    let s=e.inspect_styled_paragraph(&reopened.id,0,&[0]).unwrap();assert_eq!(s.text,"Offi");let next=folio_engine::StyledParagraphLayoutRequest{baseline_anchor:s.baseline_anchor,object_indices:s.object_indices,expected_text:s.text,runs:s.runs,style_changes:vec![],bounds:s.bounds,alignment:s.alignment.unwrap(),line_spacing:s.line_spacing.unwrap()};
    let noop=e.preview_styled_paragraph(&reopened.id,0,&next).unwrap();assert!(e.render_page(&reopened.id,0,1000).unwrap()==e.render_page(&noop.id,0,1000).unwrap());assert_eq!(e.source_bytes(&source.id).unwrap(),original);
    for id in [&source.id,&edited.id,&reopened.id,&noop.id]{e.close_document(id).unwrap();}
}
#[test]
fn review_i2_scalar_paragraph_keeps_legacy_policy_without_sequence_occurrences() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("review-scalar-legacy.pdf");scalar_sequence_resource(&input,false,0.);
    let source=e.open_document(&input).unwrap();let mut request=paragraph_request(&e,&source,Some("OO"));request.bounds=folio_engine::AnnotationRect{x:70.,y:90.,width:100.,height:100.};let edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();let output=temp.path().join("review-scalar-legacy-saved.pdf");save(&e,&edited,&output);
    let pdf=Document::load(&output).unwrap();let page=*pdf.get_pages().values().next().unwrap();let record:serde_json::Value=serde_json::from_slice(pdf.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();assert!(record["version"].as_u64().unwrap()<9);assert!(record["paragraphs"][0].get("occurrence_metrics").is_none());
    let reopened=e.open_document(&output).unwrap();let s=e.inspect_styled_paragraph(&reopened.id,0,&[0]).unwrap();let next=folio_engine::StyledParagraphLayoutRequest{baseline_anchor:s.baseline_anchor,object_indices:s.object_indices,expected_text:s.text,runs:s.runs,style_changes:vec![],bounds:s.bounds,alignment:s.alignment.unwrap(),line_spacing:s.line_spacing.unwrap()};let noop=e.preview_styled_paragraph(&reopened.id,0,&next).unwrap();assert!(e.render_page(&reopened.id,0,1000).unwrap()==e.render_page(&noop.id,0,1000).unwrap());
    for id in [&source.id,&edited.id,&reopened.id,&noop.id]{e.close_document(id).unwrap();}
}
fn adjacent_string_fixture(path:&Path,zero_mode:usize) {
    fixture(path,"ffi",true,false);let mut pdf=Document::load(path).unwrap();let page=*pdf.get_pages().values().next().unwrap();let mut c=Content::decode(&pdf.get_page_content_with_limit(page,16*1024*1024).unwrap()).unwrap();let show=c.operations.iter_mut().find(|o|o.operator=="TJ").unwrap();let strings=show.operands[0].as_array().unwrap().iter().filter(|v|v.as_str().is_ok()).cloned().collect::<Vec<_>>();assert_eq!(strings.len(),3);let mut values=vec![];
    for (i,s)in strings.into_iter().enumerate(){if i>0{match zero_mode{1=>values.push(0.into()),2=>values.extend([7.into(),(-7).into()]),_=>{}}}values.push(s);}
    show.operands[0]=Object::Array(values);let stream=pdf.add_object(Stream::new(dictionary!{},c.encode().unwrap()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);pdf.save(path).unwrap();retain(path);
}
#[test]
fn review_i3_adjacent_tj_strings_allow_prefix_insertion_and_exact_noop() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for mode in 0..3 {
        let input=temp.path().join(format!("review-adjacent-root-{mode}.pdf"));adjacent_string_fixture(&input,mode);let source=e.open_document(&input).unwrap();let original=e.source_bytes(&source.id).unwrap();
        let noop=e.replace_text_occurrence(&source.id,&[0],0,0,"Office","Office",None).unwrap().document;assert!(e.render_page(&source.id,0,1000).unwrap()==e.render_page(&noop.id,0,1000).unwrap());
        let edited=e.replace_text_occurrence(&source.id,&[0],0,0,"Office","OOffice",None).unwrap().document;let output=temp.path().join(format!("review-adjacent-root-{mode}-saved.pdf"));save(&e,&edited,&output);let reopened=e.open_document(&output).unwrap();assert_eq!(e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs[0].text,"OOffice");assert_eq!(e.source_bytes(&source.id).unwrap(),original);
        for id in [&source.id,&noop.id,&edited.id,&reopened.id]{e.close_document(id).unwrap();}
    }
}
#[test]
fn review_i3_shared_form_adjacent_tj_strings_allow_prefix_insertion() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for mode in 0..3 {
        let flat=temp.path().join(format!("review-adjacent-form-{mode}-flat.pdf"));adjacent_string_fixture(&flat,mode);let input=temp.path().join(format!("review-adjacent-form-{mode}.pdf"));shared_form_fixture(&flat,&input);let source=e.open_document(&input).unwrap();let before=e.list_occurrence_form_text_runs(&source.id,0).unwrap().runs;
        let edited=e.replace_form_text_occurrence(&source.id,&[0],0,&before[0].object_path,"Office","OOffice",None).unwrap().document;let output=temp.path().join(format!("review-adjacent-form-{mode}-saved.pdf"));save(&e,&edited,&output);let reopened=e.open_document(&output).unwrap();let after=e.list_occurrence_form_text_runs(&reopened.id,0).unwrap().runs;assert_eq!(after[0].text,"OOffice");assert_eq!(after[1],before[1]);
        for id in [&source.id,&edited.id,&reopened.id]{e.close_document(id).unwrap();}
    }
}

#[test]
fn cursor_pdf_font_name_isolates_metrics_without_weakening_mapping_proof() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    let input=temp.path().join("cursor-resource-source.pdf");fixture(&input,"ffi",true,true);
    let source=e.open_document(&input).unwrap();
    let expected=e.list_occurrence_text_runs(&source.id,0).unwrap().runs[0].text.clone();
    let edited=e.replace_text_occurrence(&source.id,&[0],0,0,&expected,"Ofice",None).unwrap().document;
    let prior=temp.path().join("cursor-resource-prior.pdf");save(&e,&edited,&prior);
    let face=ttf_parser::Face::parse(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf"),0).unwrap();let code=face.glyph_index('i').unwrap().0;
    for unique in [false,true] {
        let mut pdf=Document::load(&prior).unwrap();let page=*pdf.get_pages().values().next().unwrap();
        let mut resources=pdf.dereference(pdf.get_dictionary(page).unwrap().get(b"Resources").unwrap()).unwrap().1.as_dict().unwrap().clone();
        let mut fonts=pdf.dereference(resources.get(b"Font").unwrap()).unwrap().1.as_dict().unwrap().clone();
        let mut font=pdf.dereference(fonts.get(b"F1").unwrap()).unwrap().1.as_dict().unwrap().clone();
        let mut child=pdf.dereference(&font.get(b"DescendantFonts").unwrap().as_array().unwrap()[0]).unwrap().1.as_dict().unwrap().clone();
        let widths=child.get_mut(b"W").unwrap().as_array_mut().unwrap();let mut found=false;
        for pair in widths.chunks_exact_mut(2) {if pair[0].as_i64().unwrap()==i64::from(code){
            let item=&mut pair[1].as_array_mut().unwrap()[0];*item=Object::Real(item.as_float().unwrap()-0.0859375);found=true;
        }}
        assert!(found);
        if unique {
            let name=Object::Name(b"FolioCursorResourceProbe".to_vec());font.set("BaseFont",name.clone());child.set("BaseFont",name.clone());
            let mut descriptor=pdf.dereference(child.get(b"FontDescriptor").unwrap()).unwrap().1.as_dict().unwrap().clone();descriptor.set("FontName",name);
            child.set("FontDescriptor",pdf.add_object(descriptor));
        }
        let child=pdf.add_object(child);font.set("DescendantFonts",vec![Object::Reference(child)]);
        let font=pdf.add_object(font);fonts.set("CursorProbe",font);resources.set("Font",fonts);pdf.get_dictionary_mut(page).unwrap().set("Resources",resources);
        let mut content=Content::decode(&pdf.get_page_content_with_limit(page,16*1024*1024).unwrap()).unwrap();
        let show=content.operations.iter().position(|o|o.operator=="TJ"&&o.operands[0].as_array().unwrap().iter().any(|v|v.as_str().is_ok())).unwrap();
        assert_eq!(content.operations[show+1].operator,"TJ");content.operations[show+1].operands=vec![Object::Array(vec![Object::Real(-229.5)])];
        content.operations.insert(show,Operation::new("Tf",vec![Object::Name(b"CursorProbe".to_vec()),24.into()]));
        content.operations.insert(show+3,Operation::new("Tf",vec![Object::Name(b"F1".to_vec()),24.into()]));
        let stream=pdf.add_object(Stream::new(dictionary!{},content.encode().unwrap()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);
        let target=temp.path().join(format!("cursor-resource-{unique}.pdf"));pdf.save(&target).unwrap();retain(&target);
        let d=e.open_document(&target).unwrap();let runs=e.list_occurrence_text_runs(&d.id,0).unwrap().runs;assert_eq!(runs[0].text.trim(),"Ofice");
        if unique {
            assert!(runs[0].supported,"Unique actual font resource refused: {runs:?}");assert!(runs[1].supported);
            let noop=e.replace_text_occurrence(&d.id,&[0],0,0,&runs[0].text,"Ofice",None).unwrap().document;
            assert!(e.render_page(&d.id,0,1000).unwrap()==e.render_page(&noop.id,0,1000).unwrap());e.close_document(&noop.id).unwrap();
        }else{assert!(!runs[0].supported,"Different width maps sharing one ambiguous identity became editable");}
        assert_eq!(embedded_programs(&fs::read(&input).unwrap()),embedded_programs(&fs::read(&target).unwrap()));e.close_document(&d.id).unwrap();
    }
    e.close_document(&edited.id).unwrap();e.close_document(&source.id).unwrap();
}

#[test]
fn cursor_production_resource_preserves_original_program_neighbors_and_saved_reedit() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("cursor-production-source.pdf");fixture(&input,"ffi",true,true);
    let source=e.open_document(&input).unwrap();let original=e.source_bytes(&source.id).unwrap();let before=e.list_occurrence_text_runs(&source.id,0).unwrap().runs;
    let edited=e.replace_text_occurrence(&source.id,&[0],0,0,&before[0].text,"Ofice",None).unwrap().document;
    let saved=temp.path().join("cursor-production-saved.pdf");save(&e,&edited,&saved);let reopened=e.open_document(&saved).unwrap();let after=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
    assert!(after[0].font_name.starts_with("FolioCursor"),"A separate proved width resource was not published: {:?}",after[0].font_name);
    assert!(after[0].supported);assert_eq!(after[0].font_display_name,before[0].font_display_name);assert_eq!(after[0].text.trim(),"Ofice");
    for (a,b)in after.iter().skip(1).zip(before.iter().skip(1)){assert_eq!(a.text,b.text);assert_eq!(a.font_name,b.font_name);near_rect(&a.bounds,&b.bounds);}
    assert_eq!(embedded_programs(&original),embedded_programs(&fs::read(&saved).unwrap()));
    let noop=e.replace_text_occurrence(&reopened.id,&[0],0,0,&after[0].text,"Ofice",None).unwrap().document;
    assert!(e.render_page(&reopened.id,0,1000).unwrap()==e.render_page(&noop.id,0,1000).unwrap());
    save(&e,&noop,&temp.path().join("cursor-production-noop.pdf"));
    let preserved=e.source_bytes(&reopened.id).unwrap();
    // A retained-code deletion may now use a separately named private metric
    // resource. Prove the saved result instead of demanding the old refusal.
    let deletion=e.replace_text_occurrence(&reopened.id,&[0],0,0,&after[0].text,"Ofce",None).unwrap().document;
    let deletion_path=temp.path().join("cursor-production-deletion.pdf");save(&e,&deletion,&deletion_path);
    let deletion_reopened=e.open_document(&deletion_path).unwrap();
    let deletion_runs=e.list_occurrence_text_runs(&deletion_reopened.id,0).unwrap().runs;
    assert_eq!(deletion_runs.len(),before.len());assert_eq!(deletion_runs[0].text.trim(),"Ofce");assert!(deletion_runs[0].supported);
    for(a,b)in deletion_runs.iter().skip(1).zip(before.iter().skip(1)){assert_eq!(a.text,b.text);assert_eq!(a.font_name,b.font_name);near_rect(&a.bounds,&b.bounds);}
    assert_eq!(embedded_programs(&original),embedded_programs(&fs::read(&deletion_path).unwrap()));
    assert_eq!(e.source_bytes(&reopened.id).unwrap(),preserved);
    let reedit=e.replace_text_occurrence(&reopened.id,&[0],0,0,&after[0].text,"Ofite",None).unwrap().document;
    let again=temp.path().join("cursor-production-reedit.pdf");save(&e,&reedit,&again);let final_doc=e.open_document(&again).unwrap();assert!(e.list_occurrence_text_runs(&final_doc.id,0).unwrap().runs[0].supported);
    assert_eq!(e.source_bytes(&source.id).unwrap(),original);
    for id in [&source.id,&edited.id,&reopened.id,&noop.id,&deletion.id,&deletion_reopened.id,&reedit.id,&final_doc.id]{e.close_document(id).unwrap();}
}

#[test]
fn cursor_production_shared_forms_preserve_other_invocation_and_saved_reedit() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let flat=temp.path().join("cursor-production-form-flat.pdf");fixture(&flat,"ffi",true,true);
    let input=temp.path().join("cursor-production-form-source.pdf");shared_form_fixture(&flat,&input);
    let asset=e.fonts().register(include_bytes!("../../tests/fixtures/corpus/fonts/DejaVuSerif.ttf").to_vec()).unwrap();
    let source=e.open_document(&input).unwrap();let original=e.source_bytes(&source.id).unwrap();let before=e.list_occurrence_form_text_runs(&source.id,0).unwrap().runs;
    for substitute in [None,Some(asset.id.as_str())] {
        let edited=e.replace_form_text_occurrence(&source.id,&[0],0,&before[0].object_path,"Office","Ofice",substitute).unwrap().document;
        let saved=temp.path().join(format!("cursor-production-form-{}.pdf",substitute.is_some()));save(&e,&edited,&saved);
        let reopened=e.open_document(&saved).unwrap();let after=e.list_occurrence_form_text_runs(&reopened.id,0).unwrap().runs;
        assert!(after[0].font_name.starts_with("FolioCursor"),"The form cursor did not receive its own proved resource");assert_eq!(after[0].text,"Ofice");assert!(after[0].supported);
        for(a,b)in after.iter().skip(1).zip(before.iter().skip(1)){assert_eq!(a,b);}
        if substitute.is_none(){assert_eq!(after[0].font_display_name,before[0].font_display_name);}
        let noop=e.replace_form_text_occurrence(&reopened.id,&[0],0,&after[0].object_path,"Ofice","Ofice",None).unwrap().document;
        assert!(e.render_page(&reopened.id,0,1000).unwrap()==e.render_page(&noop.id,0,1000).unwrap());
        save(&e,&noop,&temp.path().join(format!("cursor-production-form-noop-{}.pdf",substitute.is_some())));
        let reedit=e.replace_form_text_occurrence(&reopened.id,&[0],0,&after[0].object_path,"Ofice","Ofite",None).unwrap().document;
        let again=temp.path().join(format!("cursor-production-form-reedit-{}.pdf",substitute.is_some()));save(&e,&reedit,&again);
        let final_doc=e.open_document(&again).unwrap();assert!(e.list_occurrence_form_text_runs(&final_doc.id,0).unwrap().runs[0].supported);
        let programs=embedded_programs(&fs::read(&saved).unwrap());assert!(embedded_programs(&original).iter().all(|p|programs.contains(p)));
        for id in [&edited.id,&reopened.id,&noop.id,&reedit.id,&final_doc.id]{e.close_document(id).unwrap();}
    }
    assert_eq!(e.source_bytes(&source.id).unwrap(),original);e.close_document(&source.id).unwrap();
}
