use folio_engine::{DocumentInfo, ExportRequest, PagePlan, PdfEngine};
use lopdf::{content::{Content, Operation}, dictionary, Document, Object, Stream};
use std::{fs, path::{Path, PathBuf}};

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()
}

fn fixture(path: &Path, unusable: bool, omitted: bool, source_excluded: bool, rotation: u16, unit: f32) {
    let bytes = include_bytes!("../../tests/fixtures/embedded-text/DejaVuSerif-subset.ttf");
    let face = ttf_parser::Face::parse(bytes, 0).unwrap();
    let mut chars: Vec<char> = "Old Edit café Neighbor safe ".chars().collect();
    chars.sort(); chars.dedup();
    let mut widths = vec![];
    let mut mappings = String::new();
    for c in &chars {
        let gid = face.glyph_index(*c).unwrap();
        mappings.push_str(&format!("<{:04X}> <{:04X}>\n", gid.0, *c as u32));
        widths.extend([Object::Integer(gid.0 as i64), Object::Array(vec![Object::Real(
            face.glyph_hor_advance(gid).unwrap() as f32 * 1000. / face.units_per_em() as f32)])]);
    }
    // A source code with no glyph and no valid scalar must never disappear from
    // exact source proof merely because native text extraction omits it.
    if unusable { mappings.push_str("<FFFE> <D800>\n"); }
    let cmap = format!("1 begincodespacerange <0000> <FFFF> endcodespacerange\n{} beginbfchar\n{mappings}endbfchar", chars.len() + usize::from(unusable));
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let program = pdf.add_object(Stream::new(dictionary!{"Length1"=>bytes.len() as i64}, bytes.to_vec()));
    let descriptor = pdf.add_object(dictionary!{"Type"=>"FontDescriptor", "FontName"=>"ABCDEF+DejaVuSerif", "Flags"=>32,
        "FontBBox"=>vec![(-1000).into(), (-1000).into(), 2000.into(), 2000.into()],
        "ItalicAngle"=>0, "Ascent"=>1000, "Descent"=>-300, "CapHeight"=>800, "StemV"=>80, "FontFile2"=>program});
    let mut child = dictionary!{"Type"=>"Font", "Subtype"=>"CIDFontType2", "BaseFont"=>"ABCDEF+DejaVuSerif",
        "CIDSystemInfo"=>dictionary!{"Registry"=>Object::string_literal("Adobe"), "Ordering"=>Object::string_literal("Identity"), "Supplement"=>0},
        "FontDescriptor"=>descriptor, "DW"=>0, "W"=>widths};
    if !omitted { child.set("CIDToGIDMap", "Identity"); }
    let child = pdf.add_object(child);
    let cmap = pdf.add_object(Stream::new(dictionary!{}, cmap.into_bytes()));
    let font = pdf.add_object(dictionary!{"Type"=>"Font", "Subtype"=>"Type0", "BaseFont"=>"ABCDEF+DejaVuSerif",
        "Encoding"=>"Identity-H", "DescendantFonts"=>vec![child.into()], "ToUnicode"=>cmap});
    let encoded = |s: &str| s.chars().flat_map(|c| face.glyph_index(c).unwrap().0.to_be_bytes()).collect::<Vec<_>>();
    let mut selected = encoded("Old");
    if source_excluded { selected.splice(2..2, [0xff, 0xfe]); }
    let mut operations = vec![];
    for (text, y) in [(selected, 300), (encoded("Neighbor safe"), 150)] {
        operations.extend([
            Operation::new("BT", vec![]), Operation::new("Tf", vec![Object::Name(b"F1".to_vec()), 14.into()]),
            Operation::new("Td", vec![40.into(), y.into()]),
            Operation::new("Tj", vec![Object::string_literal(text)]), Operation::new("ET", vec![]),
        ]);
    }
    let content = pdf.add_object(Stream::new(dictionary!{}, Content {operations}.encode().unwrap()));
    let page = pdf.add_object(dictionary!{"Type"=>"Page", "Parent"=>pages, "MediaBox"=>vec![0.into(), 0.into(), 400.into(), 400.into()],
        "Rotate"=>rotation as i64, "UserUnit"=>unit, "Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}, "Contents"=>content});
    pdf.objects.insert(pages, dictionary!{"Type"=>"Pages", "Kids"=>vec![page.into()], "Count"=>1}.into());
    let catalog = pdf.add_object(dictionary!{"Type"=>"Catalog", "Pages"=>pages});
    pdf.trailer.set("Root", catalog);
    pdf.save(path).unwrap();
}

fn font_streams(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let pdf = Document::load_mem(bytes).unwrap();
    let mut result = vec![];
    for d in pdf.objects.values().filter_map(|v| v.as_dict().ok()) {
        for name in [b"ToUnicode".as_slice(), b"FontFile2"] {
            if let Ok(value) = d.get(name) {
                let stream = pdf.dereference(value).unwrap().1.as_stream().unwrap();
                result.push((String::from_utf8(name.to_vec()).unwrap(), stream.get_plain_content().unwrap()));
            }
        }
    }
    result.sort(); result.dedup(); result
}

fn export(engine: &PdfEngine, source: &DocumentInfo, path: &Path) {
    engine.export_pdf(ExportRequest { flatten: false, pages: source.pages.iter().enumerate().map(|(index, page)| PagePlan {
        id: format!("page-{index}"), source_id: source.id.clone(), page_index: index,
        width: page.width, height: page.height, rotation: 0, crop: None, overlays: vec![],
    }).collect() }, path).unwrap();
}

fn saved_edit(unusable: bool, omitted: bool) {
    let e = engine(); let temp = tempfile::tempdir().unwrap();
    for rotation in [0, 90, 180, 270] { for unit in [1.0, 1.7] {
        let label = format!("{unusable}-{omitted}-{rotation}-{unit}");
        let input = temp.path().join(format!("{label}-original.pdf"));
        fixture(&input, unusable, omitted, false, rotation, unit);
        let original = fs::read(&input).unwrap(); let source = e.open_document(&input).unwrap();
        let before = e.list_occurrence_text_runs(&source.id, 0).unwrap().runs;
        assert_eq!(before[0].text, "Old"); assert!(before[0].supported, "{label}: {:?}", before[0]);
        let noop = e.replace_text_occurrence(&source.id, &[0], 0, before[0].object_index, "Old", "Old", None).unwrap();
        assert_eq!(e.render_page(&source.id, 0, 800).unwrap(), e.render_page(&noop.document.id, 0, 800).unwrap());
        e.close_document(&noop.document.id).unwrap();
        let edited = e.replace_text_occurrence(&source.id, &[0], 0, before[0].object_index, "Old", "Old café", None).unwrap();
        let saved = temp.path().join(format!("{label}-saved.pdf")); export(&e, &edited.document, &saved);
        let reopened = e.open_document(&saved).unwrap();
        let after = e.list_occurrence_text_runs(&reopened.id, 0).unwrap().runs;
        assert_eq!(after[0].text, "Old café"); assert!(after[0].supported);
        assert_eq!(after[0].font_name, before[0].font_name);
        assert_eq!(after[1].text, before[1].text); assert_eq!(after[1].bounds, before[1].bounds);
        assert_eq!(font_streams(&fs::read(&saved).unwrap()), font_streams(&original));
        let again = e.replace_text_occurrence(&reopened.id, &[0], 0, after[0].object_index, "Old café", "Edit café", None).unwrap();
        let saved2 = temp.path().join(format!("{label}-saved2.pdf")); export(&e, &again.document, &saved2);
        let final_doc = e.open_document(&saved2).unwrap();
        assert_eq!(e.list_occurrence_text_runs(&final_doc.id, 0).unwrap().runs[0].text, "Edit café");
        assert_eq!(font_streams(&fs::read(&saved2).unwrap()), font_streams(&original));
        assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(), original.as_slice()); assert_eq!(fs::read(&input).unwrap(), original);
        if let Some(root) = std::env::var_os("FOLIO_CMAP_OUTPUTS") {
            let root = PathBuf::from(root); fs::create_dir_all(&root).unwrap();
            for p in [&input, &saved, &saved2] { fs::copy(p, root.join(p.file_name().unwrap())).unwrap(); }
        }
        for id in [&source.id, &edited.document.id, &reopened.id, &again.document.id, &final_doc.id] { e.close_document(id).unwrap(); }
    }}
}

#[test]
fn unused_unusable_mapping_retains_original_font_across_saved_edits() { saved_edit(true, false); }
#[test]
fn absent_identity_mapping_retains_original_font_across_saved_edits() { saved_edit(false, true); }
#[test]
fn combined_sparse_mapping_and_identity_default_reedit() { saved_edit(true, true); }

fn mixed_age_fixture(e: &PdfEngine, path: &Path, font: &str, paragraph: bool, old_first: bool) {
    if paragraph {
        let base = path.with_extension("base.pdf"); fixture(&base, true, true, false, 0, 1.0);
        let source = e.open_document(&base).unwrap();
        let candidate = e.replace_text_occurrence(&source.id, &[0], 0, 0, "Old", "Résumé café — €", Some(font)).unwrap();
        export(e, &candidate.document, path);
        e.close_document(&candidate.document.id).unwrap(); e.close_document(&source.id).unwrap();
    } else {
        let source = e.create_blank_document(400., 400.).unwrap();
        let request = serde_json::from_value(serde_json::json!({"pages":[{
            "id":"page", "sourceId":source.id,"pageIndex":0,"width":400,"height":400,"rotation":0,
            "overlays":[{"type":"text","id":"text","x":40,"y":100,"text":"Résumé café — €","fontSize":14,"fontId":font,"color":"#000000","rotation":0}]
        }]})).unwrap();
        e.export_pdf(request, path).unwrap(); e.close_document(&source.id).unwrap();
    }
    let mut pdf = Document::load(path).unwrap();
    let first_id = *pdf.objects.iter().find(|(_, v)| v.as_dict().is_ok_and(|d|
        d.has(b"ToUnicode") && d.get(b"BaseFont").ok().and_then(|v| v.as_name().ok()).is_some_and(|n| n.starts_with(b"Folio")))).unwrap().0;
    let current = pdf.get_dictionary(first_id).unwrap().clone();
    let mut legacy = current.clone();
    let map = pdf.add_object(Stream::new(dictionary!{}, include_bytes!("../../tests/fixtures/embedded-text/folio-legacy-identity.cmap").to_vec()));
    legacy.set("ToUnicode", map);
    let (old, new) = if old_first {
        pdf.objects.insert(first_id, legacy.into()); (first_id, pdf.add_object(current))
    } else { (pdf.add_object(legacy), first_id) };
    let mut operations = vec![];
    for (alias, text, y) in [("OldFolio", "Résumé café — €", 300), ("CurrentFolio", "Résumé café — €", 220), ("CurrentFolio", "Neighbor safe", 100)] {
        let encoded: Vec<_> = text.encode_utf16().flat_map(u16::to_be_bytes).collect();
        operations.extend([
            Operation::new("BT", vec![]), Operation::new("Tf", vec![Object::Name(alias.as_bytes().to_vec()), 14.into()]),
            Operation::new("Td", vec![40.into(), y.into()]), Operation::new("Tj", vec![Object::string_literal(encoded)]), Operation::new("ET", vec![]),
        ]);
    }
    let content = pdf.add_object(Stream::new(dictionary!{}, Content {operations}.encode().unwrap()));
    let page = pdf.get_pages()[&1]; let page = pdf.get_dictionary_mut(page).unwrap();
    page.remove(b"Annots"); page.set("Contents", content);
    page.set("Resources", dictionary!{"Font"=>dictionary!{"OldFolio"=>old,"CurrentFolio"=>new}});
    pdf.prune_objects(); pdf.save(path).unwrap();
}

#[test]
fn mixed_age_folio_fonts_preserve_each_selected_run_across_saved_edits() {
    let e = engine(); let temp = tempfile::tempdir().unwrap();
    let font = e.fonts().register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec()).unwrap();
    for paragraph in [false, true] { for old_first in [false, true] {
        let folder = temp.path().join(format!("{paragraph}-{old_first}")); fs::create_dir(&folder).unwrap();
        let input = folder.join("original.pdf"); mixed_age_fixture(&e, &input, &font.id, paragraph, old_first);
        let original = fs::read(&input).unwrap(); let source = e.open_document(&input).unwrap();
        let before = e.list_occurrence_text_runs(&source.id, 0).unwrap().runs;
        assert_eq!(before.len(), 3); assert!(before.iter().all(|r| r.supported), "{before:?}");
        for selected in [0, 1] {
            let run = &before[selected];
            let noop = e.replace_text_occurrence(&source.id, &[0], 0, run.object_index, &run.text, &run.text, None).unwrap();
            assert_eq!(e.render_page(&source.id, 0, 800).unwrap(), e.render_page(&noop.document.id, 0, 800).unwrap());
            let edited = e.replace_text_occurrence(&source.id, &[0], 0, run.object_index, &run.text, "Résumé café — € 2", None).unwrap();
            let saved = folder.join("saved.pdf"); export(&e, &edited.document, &saved);
            let reopened = e.open_document(&saved).unwrap(); let after = e.list_occurrence_text_runs(&reopened.id, 0).unwrap().runs;
            assert_eq!(after[selected].text, "Résumé café — € 2"); assert!(after.iter().all(|r| r.supported));
            for i in 0..3 { if i != selected { assert_eq!(after[i].text, before[i].text); assert_eq!(after[i].bounds, before[i].bounds); } }
            let again = e.replace_text_occurrence(&reopened.id, &[0], 0, after[selected].object_index, &after[selected].text, "Résumé café — € 3", None).unwrap();
            let saved2 = folder.join("saved2.pdf"); export(&e, &again.document, &saved2);
            let final_doc = e.open_document(&saved2).unwrap();
            assert_eq!(e.list_occurrence_text_runs(&final_doc.id, 0).unwrap().runs[selected].text, "Résumé café — € 3");
            assert_eq!(font_streams(&original), font_streams(&fs::read(&saved).unwrap()));
            assert_eq!(font_streams(&original), font_streams(&fs::read(&saved2).unwrap()));
            assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(), original.as_slice()); assert_eq!(fs::read(&input).unwrap(), original);
            dump_consumer(&format!("mixed-{paragraph}-{old_first}-{selected}"), &[&input, &saved, &saved2]);
            for id in [&noop.document.id, &edited.document.id, &reopened.id, &again.document.id, &final_doc.id] { e.close_document(id).unwrap(); }
        }
        e.close_document(&source.id).unwrap();
    }}
}

#[test]
fn partial_source_accepts_explicit_imported_font_and_saved_currency_reedit() {
    let e = engine(); let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("original.pdf"); fixture(&input, true, true, false, 0, 1.0);
    let original = fs::read(&input).unwrap(); let source = e.open_document(&input).unwrap();
    let font = e.fonts().register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec()).unwrap();
    let before = e.list_occurrence_text_runs(&source.id, 0).unwrap().runs;
    let text = "Résumé café — €";
    let edited = e.replace_text_occurrence(&source.id, &[0], 0, before[0].object_index, "Old", text, Some(&font.id)).unwrap();
    let saved = temp.path().join("saved.pdf"); export(&e, &edited.document, &saved);
    let reopened = e.open_document(&saved).unwrap();
    let after = e.list_occurrence_text_runs(&reopened.id, 0).unwrap().runs;
    assert_eq!(after[0].text, text); assert!(after[0].supported);
    assert_eq!(after[1].text, before[1].text); assert_eq!(after[1].bounds, before[1].bounds);
    let again = e.replace_text_occurrence(&reopened.id, &[0], 0, after[0].object_index, text, "Résumé café — € 2", None).unwrap();
    let saved2 = temp.path().join("saved2.pdf"); export(&e, &again.document, &saved2);
    let final_doc = e.open_document(&saved2).unwrap();
    assert_eq!(e.list_occurrence_text_runs(&final_doc.id, 0).unwrap().runs[0].text, "Résumé café — € 2");
    // Reproduce an older Folio native-text save using the historical writer's
    // literal CMap. Its complete font resource must still prove saved re-edits.
    let mut legacy_pdf = Document::load(&saved).unwrap();
    let font_dictionary = legacy_pdf.objects.values().filter_map(|v| v.as_dict().ok())
        .find(|d| d.has(b"ToUnicode") && d.get(b"BaseFont").ok().and_then(|v| v.as_name().ok()).is_some_and(|n| n.starts_with(b"Folio"))).unwrap();
    let map_id = font_dictionary.get(b"ToUnicode").unwrap().as_reference().unwrap();
    legacy_pdf.objects.insert(map_id, Stream::new(dictionary!{}, include_bytes!("../../tests/fixtures/embedded-text/folio-legacy-identity.cmap").to_vec()).into());
    let legacy_path = temp.path().join("legacy.pdf"); legacy_pdf.save(&legacy_path).unwrap();
    let legacy = e.open_document(&legacy_path).unwrap();
    let runs = e.list_occurrence_text_runs(&legacy.id, 0).unwrap().runs;
    assert_eq!(runs[0].text, text); assert!(runs[0].supported);
    let noop = e.replace_text_occurrence(&legacy.id, &[0], 0, runs[0].object_index, text, text, None).unwrap();
    assert_eq!(e.render_page(&legacy.id, 0, 800).unwrap(), e.render_page(&noop.document.id, 0, 800).unwrap());
    let legacy_edit = e.replace_text_occurrence(&legacy.id, &[0], 0, runs[0].object_index, text, "Résumé café — € 3", None).unwrap();
    let legacy_saved = temp.path().join("legacy-saved.pdf"); export(&e, &legacy_edit.document, &legacy_saved);
    let legacy_reopened = e.open_document(&legacy_saved).unwrap();
    assert_eq!(e.list_occurrence_text_runs(&legacy_reopened.id, 0).unwrap().runs[0].text, "Résumé café — € 3");
    assert_eq!(font_streams(&fs::read(&legacy_path).unwrap()), font_streams(&fs::read(&legacy_saved).unwrap()));
    dump_consumer("legacy-substitution", &[&legacy_path, &legacy_saved]);
    for id in [&legacy.id, &noop.document.id, &legacy_edit.document.id, &legacy_reopened.id] { e.close_document(id).unwrap(); }
    assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(), original.as_slice());
    assert_eq!(fs::read(&input).unwrap(), original);
    dump_consumer("substitution", &[&input, &saved, &saved2]);
    for id in [&source.id, &edited.document.id, &reopened.id, &again.document.id, &final_doc.id] { e.close_document(id).unwrap(); }
}

#[test]
fn excluded_source_code_cannot_be_authorized_by_native_extraction() {
    let e = engine(); let temp = tempfile::tempdir().unwrap(); let input = temp.path().join("excluded.pdf");
    fixture(&input, true, true, true, 0, 1.0);
    let original = fs::read(&input).unwrap(); let source = e.open_document(&input).unwrap();
    let before = e.list_occurrence_text_runs(&source.id, 0).unwrap().runs;
    let run = &before[0];
    assert!(!run.supported && !run.can_substitute, "{run:?}");
    assert!(before[1].supported, "An excluded code must not disable a valid neighbor");
    assert!(e.replace_text_occurrence(&source.id, &[0], 0, run.object_index, &run.text, "Edit", None).is_err());
    assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(), original.as_slice());
    assert_eq!(fs::read(&input).unwrap(), original); e.close_document(&source.id).unwrap();
}

fn dump_consumer(label: &str, files: &[&Path]) {
    if let Some(root) = std::env::var_os("FOLIO_CMAP_OUTPUTS") {
        let root = PathBuf::from(root).join("consumers").join(label); fs::create_dir_all(&root).unwrap();
        for path in files { fs::copy(path, root.join(path.file_name().unwrap())).unwrap(); }
    }
}

#[test]
fn partial_mapping_paragraphs_reflow_save_and_replay_with_original_fonts() {
    use folio_engine::{ParagraphAlignment, StyledParagraphLayoutRequest, StyledParagraphRun};
    let e = engine(); let temp = tempfile::tempdir().unwrap();
    for styled in [false, true] {
        let input = temp.path().join("original.pdf"); fixture(&input, true, true, false, 0, 1.0);
        let original = fs::read(&input).unwrap(); let source = e.open_document(&input).unwrap();
        let before = e.list_occurrence_text_runs(&source.id, 0).unwrap().runs;
        let text = "Old café Edit café Old café";
        let edited = if styled {
            let selected = e.inspect_styled_paragraph(&source.id, 0, &[0]).unwrap();
            let mut bounds = selected.bounds; bounds.width = 110.; bounds.height = 80.;
            e.preview_styled_paragraph_occurrence(&source.id, &[0], 0, &StyledParagraphLayoutRequest {
                baseline_anchor:None,object_indices: vec![0], expected_text: selected.text,
                runs: vec![StyledParagraphRun {text: text.into(), style_id: 0}], style_changes: vec![],
                bounds, alignment: ParagraphAlignment::Left, line_spacing: 1.2,
            }).unwrap().document
        } else {
            let selected = e.inspect_text_paragraph(&source.id, 0, &[0]).unwrap();
            let mut bounds = selected.bounds; bounds.width = 110.; bounds.height = 80.;
            e.preview_text_paragraph_with_font_size(&source.id, 0, &[0], &selected.text, text, &bounds, None).unwrap()
        };
        let saved = temp.path().join("saved.pdf"); export(&e, &edited, &saved);
        let reopened = e.open_document(&saved).unwrap();
        let after = e.list_occurrence_text_runs(&reopened.id, 0).unwrap().runs;
        let neighbor = after.iter().find(|r| r.text == "Neighbor safe").unwrap(); assert_eq!(neighbor.bounds, before[1].bounds);
        let indices: Vec<_> = after.iter().filter(|r| r.text != "Neighbor safe").map(|r| r.object_index).collect();
        assert!(indices.len() >= 2, "Exercise real wrapping");
        let replay = if styled {
            let selected = e.inspect_styled_paragraph(&reopened.id, 0, &indices).unwrap();
            assert_eq!(selected.text, text);
            e.preview_styled_paragraph_occurrence(&reopened.id, &[0], 0, &StyledParagraphLayoutRequest {
                baseline_anchor:None,object_indices: selected.object_indices, expected_text: selected.text, runs: selected.runs,
                style_changes: vec![], bounds: selected.bounds, alignment: selected.alignment.unwrap(),
                line_spacing: selected.line_spacing.unwrap(),
            }).unwrap().document
        } else {
            let selected = e.inspect_text_paragraph(&reopened.id, 0, &indices).unwrap();
            assert_eq!(selected.text, text);
            e.preview_text_paragraph_with_font_size(&reopened.id, 0, &indices, &selected.text,
                &selected.text, &selected.bounds, None).unwrap()
        };
        assert_eq!(e.render_page(&reopened.id, 0, 800).unwrap(), e.render_page(&replay.id, 0, 800).unwrap());
        assert_eq!(font_streams(&fs::read(&saved).unwrap()), font_streams(&original));
        assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(), original.as_slice());
        assert_eq!(fs::read(&input).unwrap(), original);
        dump_consumer(if styled {"styled"} else {"uniform"}, &[&input, &saved]);
        for id in [&source.id, &edited.id, &reopened.id, &replay.id] { e.close_document(id).unwrap(); }
    }
}

#[test]
fn partial_mapping_group_preserves_exact_member_codes() {
    let e = engine(); let temp = tempfile::tempdir().unwrap(); let input = temp.path().join("original.pdf");
    fixture(&input, true, true, false, 0, 1.0);
    let mut pdf = Document::load(&input).unwrap(); let page = pdf.get_pages()[&1];
    let mut content = Content::decode(&pdf.get_page_content(page)).unwrap();
    let encoded = content.operations[3].operands[0].as_str().unwrap().to_vec();
    content.operations[3].operands[0] = Object::string_literal(encoded[..2].to_vec());
    let face = ttf_parser::Face::parse(include_bytes!("../../tests/fixtures/embedded-text/DejaVuSerif-subset.ttf"), 0).unwrap();
    let advance = face.glyph_hor_advance(face.glyph_index('O').unwrap()).unwrap() as f32 * 14. / face.units_per_em() as f32;
    content.operations.splice(4..4, [Operation::new("Td", vec![advance.into(), 0.into()]), Operation::new("Tj", vec![Object::string_literal(encoded[2..].to_vec())])]);
    let stream = pdf.add_object(Stream::new(dictionary!{}, content.encode().unwrap())); pdf.get_dictionary_mut(page).unwrap().set("Contents", stream);
    pdf.save(&input).unwrap(); let original = fs::read(&input).unwrap(); let source = e.open_document(&input).unwrap();
    let before = e.list_occurrence_text_runs(&source.id, 0).unwrap().runs;
    let selected = e.inspect_occurrence_text_group(&source.id, 0, &[0, 1]).unwrap(); assert_eq!(selected.text, "Old");
    let edited = e.replace_text_group_occurrence(&source.id, &[0], 0, &[0, 1], "Old", "Old café").unwrap();
    let saved = temp.path().join("saved.pdf"); export(&e, &edited.document, &saved); let reopened = e.open_document(&saved).unwrap();
    let after = e.list_occurrence_text_runs(&reopened.id, 0).unwrap().runs; assert_eq!(after[0].text, "Old café");
    assert_eq!(after.iter().find(|r| r.text == "Neighbor safe").unwrap().bounds, before[2].bounds);
    assert_eq!(font_streams(&fs::read(&saved).unwrap()), font_streams(&original));
    assert_eq!(fs::read(&input).unwrap(), original); dump_consumer("group", &[&input, &saved]);
    for id in [&source.id, &edited.document.id, &reopened.id] { e.close_document(id).unwrap(); }
}

#[test]
fn partial_mapping_reusable_form_text_preserves_original_mapping() {
    let e = engine(); let temp = tempfile::tempdir().unwrap(); let input = temp.path().join("original.pdf");
    fixture(&input, true, true, false, 0, 1.0);
    let mut pdf = Document::load(&input).unwrap(); let page = pdf.get_pages()[&1];
    let resources = pdf.get_dictionary(page).unwrap().get(b"Resources").unwrap().clone();
    let content = pdf.get_page_content(page);
    let form = pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject", "Subtype"=>"Form", "FormType"=>1,
        "BBox"=>vec![0.into(), 0.into(), 400.into(), 400.into()], "Resources"=>resources}, content));
    let content = pdf.add_object(Stream::new(dictionary!{}, b"q /Fm Do Q".to_vec()));
    let page = pdf.get_dictionary_mut(page).unwrap(); page.set("Contents", content);
    page.set("Resources", dictionary!{"XObject"=>dictionary!{"Fm"=>form}}); pdf.save(&input).unwrap();
    let original = fs::read(&input).unwrap(); let source = e.open_document(&input).unwrap();
    let runs = e.list_form_text_runs(&source.id, 0).unwrap(); assert!(runs.reason.is_none(), "{:?}", runs.reason);
    let run = runs.runs.iter().find(|r| r.text == "Old").unwrap(); assert!(run.supported);
    let edited = e.replace_form_text(&source.id, 0, &run.object_path, "Old", "Old café").unwrap();
    let saved = temp.path().join("saved.pdf"); export(&e, &edited, &saved); let reopened = e.open_document(&saved).unwrap();
    let runs = e.list_form_text_runs(&reopened.id, 0).unwrap();
    let run = runs.runs.iter().find(|r| r.text == "Old café").unwrap(); assert!(run.supported);
    let again = e.replace_form_text(&reopened.id, 0, &run.object_path, "Old café", "Edit café").unwrap();
    let saved2 = temp.path().join("saved2.pdf"); export(&e, &again, &saved2);
    assert_eq!(font_streams(&fs::read(&saved).unwrap()), font_streams(&original));
    assert_eq!(font_streams(&fs::read(&saved2).unwrap()), font_streams(&original));
    assert_eq!(fs::read(&input).unwrap(), original); dump_consumer("form", &[&input, &saved, &saved2]);
    for id in [&source.id, &edited.id, &reopened.id, &again.id] { e.close_document(id).unwrap(); }
}
