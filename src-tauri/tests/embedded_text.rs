use folio_engine::{DocumentInfo, ExportRequest, PagePlan, PdfEngine};
use lopdf::{dictionary, Document, Object, Stream};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
static TEST_LOCK: Mutex<()> = Mutex::new(());

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/embedded-text")
        .join(name)
}
fn request(source: &DocumentInfo) -> ExportRequest {
    ExportRequest {
        flatten: false,
        pages: vec![PagePlan {
            id: "page".into(),
            source_id: source.id.clone(),
            page_index: 0,
            width: source.pages[0].width,
            height: source.pages[0].height,
            rotation: 0,
            overlays: vec![],
        }],
    }
}
fn cid_fixture(path: &Path, rotation: u16) {
    let bytes = fs::read(fixture_path("DejaVuSerif-subset.ttf")).unwrap();
    let face = ttf_parser::Face::parse(&bytes, 0).unwrap();
    let mut chars: Vec<char> = "Old Cafe Edit café Neighbor safe ".chars().collect();
    chars.sort();
    chars.dedup();
    let mut map = vec![0u8; 512];
    let mut widths = Vec::new();
    let mut unicode=String::from("/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def /CMapName /Fixture def /CMapType 2 def 1 begincodespacerange <0000> <FFFF> endcodespacerange\n");
    unicode.push_str(&format!("{} beginbfchar\n", chars.len()));
    for character in chars {
        let gid = face.glyph_index(character).unwrap();
        let code = character as usize;
        map[code * 2..code * 2 + 2].copy_from_slice(&gid.0.to_be_bytes());
        widths.extend([
            Object::Integer(code as i64),
            Object::Array(vec![Object::Real(
                face.glyph_hor_advance(gid).unwrap() as f32 * 1000.0 / face.units_per_em() as f32,
            )]),
        ]);
        unicode.push_str(&format!("<{code:04X}> <{code:04X}>\n"));
    }
    unicode.push_str("endbfchar endcmap CMapName currentdict /CMap defineresource pop end end");
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let font_file = pdf.add_object(Stream::new(
        dictionary! {"Length1"=>bytes.len() as i64},
        bytes,
    ));
    let descriptor=pdf.add_object(dictionary!{"Type"=>"FontDescriptor","FontName"=>"ABCDEF+DejaVuSerif","Flags"=>32,"FontBBox"=>vec![(-1000).into(),(-1000).into(),2000.into(),2000.into()],"ItalicAngle"=>0,"Ascent"=>1000,"Descent"=>-300,"CapHeight"=>800,"StemV"=>80,"FontFile2"=>font_file});
    let map = pdf.add_object(Stream::new(dictionary! {}, map));
    let unicode = pdf.add_object(Stream::new(dictionary! {}, unicode.into_bytes()));
    let descendant=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"CIDFontType2","BaseFont"=>"ABCDEF+DejaVuSerif","CIDSystemInfo"=>dictionary!{"Registry"=>Object::string_literal("Adobe"),"Ordering"=>Object::string_literal("Identity"),"Supplement"=>0},"FontDescriptor"=>descriptor,"DW"=>0,"W"=>widths,"CIDToGIDMap"=>map});
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type0","BaseFont"=>"ABCDEF+DejaVuSerif","Encoding"=>"Identity-H","DescendantFonts"=>vec![descendant.into()],"ToUnicode"=>unicode});
    let neighbor =
        pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let content=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 16 Tf 0.2 0.4 0.6 rg 1 0 0 1 30 300 Tm <004F006C0064> Tj ET BT /F2 16 Tf 0 g 1 0 0 1 30 240 Tm (Neighbor) Tj ET".to_vec()));
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Rotate"=>rotation as i64,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font,"F2"=>neighbor}},"Contents"=>content});
    pdf.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into(),
    );
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    pdf.trailer.set("Root", root);
    pdf.save(path).unwrap();
}

#[test]
fn verified_cid_subset_reuses_original_font_through_edit_export_and_undo_sources() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for rotation in [0, 90, 180, 270] {
        let path = temp.path().join(format!("cid-{rotation}.pdf"));
        cid_fixture(&path, rotation);
        let original = fs::read(&path).unwrap();
        let source = engine.open_document(&path).unwrap();
        let runs = engine.list_text_runs(&source.id, 0).unwrap();
        assert!(runs.runs[0].supported, "{:?}", runs.runs[0].reason);
        let edited = engine
            .replace_text(&source.id, 0, 0, "Old", "Edit café")
            .unwrap();
        let after = engine.list_text_runs(&edited.id, 0).unwrap();
        assert_eq!(after.runs[0].text, "Edit café");
        assert_eq!(after.runs[0].font_name, runs.runs[0].font_name);
        assert_eq!(after.runs[0].font_size, 16.0);
        assert_eq!(after.runs[1].text, "Neighbor");
        let output = temp.path().join(format!("edited-{rotation}.pdf"));
        engine.export_pdf(request(&edited), &output).unwrap();
        let reopened = engine.open_document(&output).unwrap();
        assert!(engine
            .extract_text(&reopened.id, 0)
            .unwrap()
            .contains("Edit café"));
        assert_eq!(fs::read(&path).unwrap(), original);
        assert!(engine.extract_text(&source.id, 0).unwrap().contains("Old"));
        assert!(engine
            .extract_text(&edited.id, 0)
            .unwrap()
            .contains("Edit café"));
        for id in [source.id, edited.id, reopened.id] {
            engine.close_document(&id).unwrap();
        }
    }
}

#[test]
fn real_reportlab_simple_subset_preserves_accented_text_and_missing_subset_glyph_is_precise() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let engine = engine();
    let source = engine
        .open_document(fixture_path("reportlab-subset.pdf"))
        .unwrap();
    let runs = engine.list_text_runs(&source.id, 0).unwrap();
    assert!(runs.runs[0].supported, "{:?}", runs.runs[0].reason);
    let edited = engine
        .replace_text(&source.id, 0, 0, "Café old", "Café edit")
        .unwrap();
    assert!(engine
        .extract_text(&edited.id, 0)
        .unwrap()
        .contains("Café edit"));
    let error = engine
        .replace_text(&source.id, 0, 0, "Café old", "Cafè old")
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("U+00E8") && error.contains("substitute"),
        "{error}"
    );
    for id in [source.id, edited.id] {
        engine.close_document(&id).unwrap();
    }
}

#[test]
fn explicit_substitute_preserves_style_order_and_embeds_independent_source_bytes() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let engine = engine();
    let source = engine
        .open_document(fixture_path("reportlab-subset.pdf"))
        .unwrap();
    let original = engine.source_bytes(&source.id).unwrap();
    let before = engine.list_text_runs(&source.id, 0).unwrap();
    let font_bytes = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf"),
    )
    .unwrap();
    let font = engine.fonts().register(font_bytes.clone()).unwrap();
    let edited = engine
        .replace_text_with_font(&source.id, 0, 0, "Café old", "Cafè edit", Some(&font.id))
        .unwrap();
    engine.fonts().remove(&font.id).unwrap();
    let after = engine.list_text_runs(&edited.id, 0).unwrap();
    assert_eq!(after.runs[0].text, "Cafè edit");
    assert_eq!(after.runs[0].font_size, before.runs[0].font_size);
    assert!(
        after.runs[0].is_embedded && after.runs[0].supported,
        "{:?}",
        after.runs[0].reason
    );
    assert_eq!(after.runs[1].text, before.runs[1].text);
    assert_eq!(after.runs[1].bounds, before.runs[1].bounds);
    let bytes = engine.source_bytes(&edited.id).unwrap();
    let pdf = Document::load_mem(&bytes).unwrap();
    assert!(pdf
        .objects
        .values()
        .filter_map(|value| value.as_dict().ok())
        .filter_map(|dict| dict.get(b"FontFile2").ok())
        .any(|value| {
            let stream = pdf.dereference(value).unwrap().1.as_stream().unwrap();
            let data = if stream.dict.has(b"Filter") {
                stream
                    .decompressed_content_with_limit(16 * 1024 * 1024)
                    .unwrap()
            } else {
                stream.content.clone()
            };
            data == font_bytes
        }));
    let next = engine
        .replace_text(&edited.id, 0, 0, "Cafè edit", "Cafè safe")
        .unwrap();
    assert!(engine
        .extract_text(&next.id, 0)
        .unwrap()
        .contains("Cafè safe"));
    assert_eq!(engine.source_bytes(&source.id).unwrap(), original);
    assert!(engine
        .extract_text(&source.id, 0)
        .unwrap()
        .contains("Café old"));
    assert!(engine
        .extract_text(&edited.id, 0)
        .unwrap()
        .contains("Cafè edit"));
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("substituted.pdf");
    engine.export_pdf(request(&edited), &output).unwrap();
    let reopened = engine.open_document(&output).unwrap();
    assert!(engine
        .extract_text(&reopened.id, 0)
        .unwrap()
        .contains("Cafè edit"));
    for id in [source.id, edited.id, next.id, reopened.id] {
        engine.close_document(&id).unwrap();
    }
}

#[test]
fn unverifiable_embedded_mappings_are_refused_without_source_mutation() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for kind in [
        "missing-unicode",
        "duplicate-unicode",
        "ligature",
        "wrong-glyph",
        "wrong-width",
        "huge-range",
        "odd-glyph-map",
        "overlapping-codespaces",
    ] {
        let path = temp.path().join(format!("{kind}.pdf"));
        cid_fixture(&path, 0);
        let mut pdf = Document::load(&path).unwrap();
        let font = *pdf
            .objects
            .iter()
            .find(|(_, value)| {
                value.as_dict().is_ok_and(|dict| {
                    dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Type0")
                })
            })
            .unwrap()
            .0;
        let unicode = pdf
            .get_dictionary(font)
            .unwrap()
            .get(b"ToUnicode")
            .unwrap()
            .as_reference()
            .unwrap();
        match kind {
            "missing-unicode" => {
                pdf.get_dictionary_mut(font).unwrap().remove(b"ToUnicode");
            }
            "duplicate-unicode" | "ligature" | "huge-range" | "overlapping-codespaces" => {
                let stream = pdf
                    .get_object_mut(unicode)
                    .unwrap()
                    .as_stream_mut()
                    .unwrap();
                let mut content = String::from_utf8(stream.content.clone()).unwrap();
                content = match kind {
                    "duplicate-unicode" => content.replace("<006C> <006C>", "<006C> <004F>"),
                    "ligature" => content.replace("<006C> <006C>", "<006C> <00660069>"),
                    "overlapping-codespaces" => content.replace(
                        "1 begincodespacerange <0000> <FFFF>",
                        "2 begincodespacerange <0000> <FFFF> <0000> <FFFF>",
                    ),
                    _ => content
                        .replace("beginbfchar", "beginbfrange")
                        .replace("<0020> <0020>", "<00000000> <FFFFFFFF> <0000>"),
                };
                stream.set_content(content.into_bytes());
            }
            "wrong-width" => {
                let child = pdf
                    .objects
                    .values_mut()
                    .filter_map(|value| value.as_dict_mut().ok())
                    .find(|dict| {
                        dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"CIDFontType2")
                    })
                    .unwrap();
                child.get_mut(b"W").unwrap().as_array_mut().unwrap()[1] =
                    Object::Array(vec![9000.into()]);
            }
            "wrong-glyph" | "odd-glyph-map" => {
                let map = pdf
                    .objects
                    .values()
                    .filter_map(|value| value.as_dict().ok())
                    .find_map(|dict| dict.get(b"CIDToGIDMap").ok())
                    .unwrap()
                    .as_reference()
                    .unwrap();
                let stream = pdf.get_object_mut(map).unwrap().as_stream_mut().unwrap();
                if kind == "odd-glyph-map" {
                    stream.content.pop();
                } else {
                    stream.content[0x4f * 2..0x4f * 2 + 2].copy_from_slice(&1u16.to_be_bytes());
                }
                stream.dict.set("Length", stream.content.len() as i64);
            }
            _ => unreachable!(),
        }
        pdf.save(&path).unwrap();
        let source = engine.open_document(&path).unwrap();
        let bytes = engine.source_bytes(&source.id).unwrap();
        let runs = engine.list_text_runs(&source.id, 0).unwrap();
        assert!(!runs.runs[0].supported, "{kind}: {:?}", runs.runs[0]);
        assert!(!runs.runs[0].can_substitute, "{kind}");
        assert!(
            engine
                .replace_text(&source.id, 0, 0, &runs.runs[0].text, "Edit")
                .is_err(),
            "{kind}"
        );
        assert_eq!(engine.source_bytes(&source.id).unwrap(), bytes, "{kind}");
        engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn substitution_preserves_rotation_pixels_and_rejects_invalid_font_references_and_overflow() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let font = engine
        .fonts()
        .register(
            fs::read(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf"),
            )
            .unwrap(),
        )
        .unwrap();
    for rotation in [0, 90, 180, 270] {
        let path = temp.path().join(format!("substitution-{rotation}.pdf"));
        cid_fixture(&path, rotation);
        let source = engine.open_document(&path).unwrap();
        let before_runs = engine.list_text_runs(&source.id, 0).unwrap();
        assert!(before_runs.runs[0].is_embedded && before_runs.runs[0].can_substitute);
        for (replacement, font_id) in [
            ("Edit", "missing-font"),
            ("Edit 漢", font.id.as_str()),
            (
                "MMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMM",
                font.id.as_str(),
            ),
        ] {
            assert!(engine
                .replace_text_with_font(&source.id, 0, 0, "Old", replacement, Some(font_id))
                .is_err());
        }
        let changed = engine
            .replace_text_with_font(&source.id, 0, 0, "Old", "Cafè", Some(&font.id))
            .unwrap();
        let after_runs = engine.list_text_runs(&changed.id, 0).unwrap();
        let before = image::load_from_memory(&engine.render_page(&source.id, 0, 600).unwrap())
            .unwrap()
            .to_rgb8();
        let after = image::load_from_memory(&engine.render_page(&changed.id, 0, 600).unwrap())
            .unwrap()
            .to_rgb8();
        let a = &before_runs.runs[0].bounds;
        let b = &after_runs.runs[0].bounds;
        let bounds = [
            a.x.min(b.x),
            a.y.min(b.y),
            (a.x + a.width).max(b.x + b.width),
            (a.y + a.height).max(b.y + b.height),
        ];
        let scale = 600.0 / source.pages[0].width;
        let mut differences = 0;
        for (x, y, pixel) in before.enumerate_pixels() {
            if pixel != after.get_pixel(x, y) {
                differences += 1;
                assert!(
                    x as f32 >= bounds[0] * scale - 3.0
                        && x as f32 <= bounds[2] * scale + 3.0
                        && y as f32 >= bounds[1] * scale - 3.0
                        && y as f32 <= bounds[3] * scale + 3.0,
                    "pixel outside selected bounds {rotation}: {x},{y}"
                );
            }
        }
        assert!(differences > 20);
        engine.close_document(&source.id).unwrap();
        engine.close_document(&changed.id).unwrap();
    }
}

#[test]
fn winansi_unicode_subtables_must_agree_even_for_a_new_replacement_character() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for kind in ["subset", "conflicting-cmap", "missing-cmap"] {
        let conflicting = kind != "subset";
        let path = temp.path().join(format!("winansi-{kind}.pdf"));
        cid_fixture(&path, 0);
        let mut pdf = Document::load(&path).unwrap();
        let font = *pdf
            .objects
            .iter()
            .find(|(_, value)| {
                value.as_dict().is_ok_and(|dict| {
                    dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Type0")
                })
            })
            .unwrap()
            .0;
        let child = pdf
            .objects
            .values()
            .filter_map(|value| value.as_dict().ok())
            .find(|dict| {
                dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"CIDFontType2")
            })
            .unwrap()
            .clone();
        let program = fs::read(fixture_path(&format!("DejaVuSerif-{kind}.ttf"))).unwrap();
        let face = ttf_parser::Face::parse(&program, 0).unwrap();
        let widths: Vec<Object> = (0..256)
            .map(|code| {
                face.glyph_index(char::from_u32(code).unwrap())
                    .and_then(|gid| face.glyph_hor_advance(gid))
                    .map(|width| Object::Real(width as f32 * 1000.0 / face.units_per_em() as f32))
                    .unwrap_or(0.into())
            })
            .collect();
        let chars: Vec<_> = (32..256)
            .filter(|code| face.glyph_index(char::from_u32(*code).unwrap()).is_some())
            .collect();
        let mut cmap = format!(
            "1 begincodespacerange <00> <FF> endcodespacerange {} beginbfchar\n",
            chars.len()
        );
        for code in chars {
            cmap.push_str(&format!("<{code:02X}> <{code:04X}>\n"));
        }
        cmap.push_str("endbfchar");
        let unicode = pdf.add_object(Stream::new(dictionary! {}, cmap.into_bytes()));
        let file = pdf
            .dereference(child.get(b"FontDescriptor").unwrap())
            .unwrap()
            .1
            .as_dict()
            .unwrap()
            .get(b"FontFile2")
            .unwrap()
            .as_reference()
            .unwrap();
        pdf.get_object_mut(file)
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .set_content(program);
        pdf.objects.insert(font,dictionary!{"Type"=>"Font","Subtype"=>"TrueType","BaseFont"=>"ABCDEF+DejaVuSerif","Encoding"=>"WinAnsiEncoding","FontDescriptor"=>child.get(b"FontDescriptor").unwrap().clone(),"FirstChar"=>0,"LastChar"=>255,"Widths"=>widths,"ToUnicode"=>unicode}.into());
        let page = *pdf.get_pages().values().next().unwrap();
        let content = pdf
            .get_dictionary(page)
            .unwrap()
            .get(b"Contents")
            .unwrap()
            .as_reference()
            .unwrap();
        let stream = pdf
            .get_object_mut(content)
            .unwrap()
            .as_stream_mut()
            .unwrap();
        stream.set_content(
            String::from_utf8(stream.content.clone())
                .unwrap()
                .replace("<004F006C0064>", "(Old)")
                .into_bytes(),
        );
        pdf.save(&path).unwrap();
        let source = engine.open_document(&path).unwrap();
        let runs = engine.list_text_runs(&source.id, 0).unwrap();
        assert_eq!(
            runs.runs[0].supported, !conflicting,
            "{:?}",
            runs.runs[0].reason
        );
        if conflicting {
            assert!(runs.runs[0]
                .reason
                .as_ref()
                .unwrap()
                .contains("conflicting Unicode"));
            assert!(engine
                .replace_text(&source.id, 0, 0, "Old", "Edit")
                .is_err());
        } else {
            let edited = engine
                .replace_text(&source.id, 0, 0, "Old", "Edit")
                .unwrap();
            engine.close_document(&edited.id).unwrap();
        }
        engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn standard_text_can_substitute_and_marked_content_retains_a_clear_refusal() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    let font = engine
        .fonts()
        .register(
            fs::read(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf"),
            )
            .unwrap(),
        )
        .unwrap();
    for marked in [false, true] {
        let path = temp.path().join(format!("standard-{marked}.pdf"));
        cid_fixture(&path, 0);
        let mut pdf = Document::load(&path).unwrap();
        let page = *pdf.get_pages().values().next().unwrap();
        let content = pdf
            .get_dictionary(page)
            .unwrap()
            .get(b"Contents")
            .unwrap()
            .as_reference()
            .unwrap();
        let stream = pdf
            .get_object_mut(content)
            .unwrap()
            .as_stream_mut()
            .unwrap();
        let text = "BT /F2 16 Tf 0.2 0.4 0.6 rg 1 0 0 1 30 300 Tm (Old) Tj ET";
        stream.set_content(
            if marked {
                format!("/Span BMC {text} EMC")
            } else {
                text.into()
            }
            .into_bytes(),
        );
        pdf.save(&path).unwrap();
        let source = engine.open_document(&path).unwrap();
        let runs = engine.list_text_runs(&source.id, 0).unwrap();
        assert!(runs.runs[0].supported);
        assert!(!runs.runs[0].is_embedded);
        assert_eq!(runs.runs[0].can_substitute, !marked);
        let edited = engine.replace_text_with_font(&source.id, 0, 0, "Old", "Cafè", Some(&font.id));
        if marked {
            assert!(edited.unwrap_err().to_string().contains("marked-content"));
        } else {
            let edited = edited.unwrap();
            assert!(engine.extract_text(&edited.id, 0).unwrap().contains("Cafè"));
            engine.close_document(&edited.id).unwrap();
        }
        engine.close_document(&source.id).unwrap();
    }
    engine.fonts().remove(&font.id).unwrap();
}

#[test]
fn type3_font_without_extractable_program_does_not_hide_standard_text_candidates() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("type3.pdf");
    cid_fixture(&path, 0);
    let mut pdf = Document::load(&path).unwrap();
    let glyph = pdf.add_object(Stream::new(
        dictionary! {},
        b"600 0 0 0 500 700 d1 0 0 500 700 re f".to_vec(),
    ));
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type3","FontBBox"=>vec![0.into(),0.into(),500.into(),700.into()],"FontMatrix"=>vec![0.001.into(),0.into(),0.into(),0.001.into(),0.into(),0.into()],"CharProcs"=>dictionary!{"O"=>glyph,"l"=>glyph,"d"=>glyph},"Encoding"=>dictionary!{"Type"=>"Encoding","Differences"=>vec![79.into(),Object::Name(b"O".to_vec()),100.into(),Object::Name(b"d".to_vec()),108.into(),Object::Name(b"l".to_vec())]},"FirstChar"=>79,"LastChar"=>108,"Widths"=>vec![Object::Integer(600);30],"Resources"=>dictionary!{}});
    let page = *pdf.get_pages().values().next().unwrap();
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
        .set("F3", font);
    let content = pdf
        .get_dictionary(page)
        .unwrap()
        .get(b"Contents")
        .unwrap()
        .as_reference()
        .unwrap();
    let stream = pdf
        .get_object_mut(content)
        .unwrap()
        .as_stream_mut()
        .unwrap();
    stream.set_content(b"BT /F3 16 Tf 1 0 0 1 30 300 Tm (Old) Tj ET BT /F2 16 Tf 1 0 0 1 30 240 Tm (Neighbor) Tj ET".to_vec());
    pdf.save(&path).unwrap();
    let engine = engine();
    let source = engine.open_document(&path).unwrap();
    let runs = engine.list_text_runs(&source.id, 0).unwrap();
    assert!(runs.runs.iter().any(|run| !run.supported));
    // The page's custom Type3 encoding keeps the existing conservative page
    // gate, but unsupported font-data access must not abort run enumeration.
    assert!(runs.runs.iter().any(|run| run.text == "Neighbor"));
    engine.close_document(&source.id).unwrap();
}
