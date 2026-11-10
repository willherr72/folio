use crate::form_choice_text::ChoiceTextBlock;
use lopdf::content::Content;
use std::collections::BTreeMap;

const OLD: &[u8] = b"q 1 0 0 rg 0 0 20 10 re f Q /Tx BMC q BT /Old 12 Tf 2 3 Td (Old) Tj ET Q EMC q 0 1 0 rg 80 0 20 10 re f Q";
const NEW: &[u8] = b"q 0 g 0 0 100 10 re f Q /Tx BMC q BT /New 12 Tf 2 3 Td (New) Tj ET Q EMC q 0 0 1 rg 80 0 20 10 re f Q";
fn map() -> BTreeMap<Vec<u8>, Vec<u8>> {
    BTreeMap::from([(b"New".to_vec(), b"Old".to_vec())])
}
fn encoded(bytes: &[u8]) -> Vec<u8> {
    Content::decode_strict(bytes).unwrap().encode().unwrap()
}

#[test]
fn replacement_retains_source_artwork_and_maps_only_generated_text_fonts() {
    let source = ChoiceTextBlock::parse(OLD).unwrap();
    let generated = ChoiceTextBlock::parse(NEW).unwrap();
    assert_eq!(
        generated.font_names().iter().cloned().collect::<Vec<_>>(),
        vec![b"New".to_vec()]
    );
    let output = source.transplant(&generated, &map()).unwrap();
    let expected = String::from_utf8(OLD.to_vec())
        .unwrap()
        .replace("(Old)", "(New)");
    assert_eq!(output, encoded(expected.as_bytes()));
}

#[test]
fn no_op_text_and_empty_appearances_are_supported() {
    let source = ChoiceTextBlock::parse(OLD).unwrap();
    let same = BTreeMap::from([(b"Old".to_vec(), b"Old".to_vec())]);
    assert_eq!(source.transplant(&source, &same).unwrap(), encoded(OLD));
    let empty = ChoiceTextBlock::parse(b"/Tx BMC q Q EMC").unwrap();
    assert!(empty.font_names().is_empty());
    assert_eq!(
        empty.transplant(&empty, &BTreeMap::new()).unwrap(),
        encoded(b"/Tx BMC q Q EMC")
    );
    assert!(empty.transplant(&source, &same).is_ok());
}

#[test]
fn list_selection_paint_and_text_are_local_to_the_replacement() {
    let bytes=b"/Tx BMC q 1 1 80 20 re W n 0 0 1 rg 1 1 80 12 re f BT /F 12 Tf 1 g (Selected) Tj ET 0 g BT /F 12 Tf 1 15 Td (Other) Tj ET Q EMC";
    assert!(ChoiceTextBlock::parse(bytes).is_ok());
}

#[test]
fn marker_words_inside_strings_are_not_marked_content() {
    let bytes = b"/Tx BMC q BT /F 12 Tf (EMC /Tx BMC \\(text\\)) Tj ET Q EMC";
    assert!(ChoiceTextBlock::parse(bytes).is_ok());
    for bytes in [
        b"q Q".as_slice(),
        b"/Tx BMC q Q EMC /Tx BMC q Q EMC",
        b"/Tx BMC /Span BMC q Q EMC EMC",
        b"/Tx <<>> BDC q Q EMC",
        b"/Tx BMC q Q",
    ] {
        assert!(
            ChoiceTextBlock::parse(bytes).is_err(),
            "{:?}",
            String::from_utf8_lossy(bytes)
        );
    }
}

#[test]
fn transforms_text_and_graphics_state_cannot_cross_text_boundaries() {
    for bytes in [
        b"q 2 0 0 2 0 0 cm /Tx BMC q Q EMC Q".as_slice(),
        b"1 0 0 rg /Tx BMC q Q EMC",
        b"/Tx BMC q BT /F 12 Tf (x) Tj Q EMC",
        b"/Tx BMC q BT BT ET ET Q EMC",
        b"/Tx BMC q Q Q EMC",
        b"/Tx BMC q Q EMC 1 0 0 rg",
        b"/Tx BMC q /State gs Q EMC",
        b"/Tx BMC q /Other Do Q EMC",
    ] {
        assert!(
            ChoiceTextBlock::parse(bytes).is_err(),
            "{:?}",
            String::from_utf8_lossy(bytes)
        );
    }
    assert!(ChoiceTextBlock::parse(b"q 2 0 0 2 0 0 cm /Artwork Do Q /Tx BMC q Q EMC").is_ok());
}

#[test]
fn paths_and_pending_clips_are_not_saved_by_q_and_q_restore() {
    for bytes in [
        b"q 0 0 10 10 re Q /Tx BMC q Q EMC".as_slice(),
        b"q 0 0 10 10 re W Q /Tx BMC q Q EMC",
        b"/Tx BMC q 0 0 10 10 re W Q EMC",
        b"/Tx BMC q 0 0 m q Q Q EMC",
        b"/Tx BMC q W n Q EMC",
        b"/Tx BMC q BT 0 0 10 10 re f ET Q EMC",
    ] {
        assert!(
            ChoiceTextBlock::parse(bytes).is_err(),
            "{:?}",
            String::from_utf8_lossy(bytes)
        );
    }
    assert!(ChoiceTextBlock::parse(b"q 0 0 10 10 re W n Q /Tx BMC q Q EMC").is_ok());
}

#[test]
fn malformed_unknown_and_unbound_text_instructions_are_refused() {
    for body in [
        "1 2 rg",
        "true g",
        "1e999 g",
        "/F (12) Tf",
        "BT (x) Tj ET",
        "BT /F 12 Tf [true] TJ ET",
        "BT /F 12 Tf 8 Tr ET",
        "BT /F 12 Tf (x) 1 2 \" ET",
        "1 Surprise",
        "BX 1 Surprise EX",
        "[1 (x)] 0 d",
        "Q Q q",
    ] {
        let raw = format!("/Tx BMC q {body} Q EMC");
        assert!(ChoiceTextBlock::parse(raw.as_bytes()).is_err(), "{body}");
    }
    // Two sequential, balanced islands are valid; the first-pass oracle was wrong.
    assert!(ChoiceTextBlock::parse(b"/Tx BMC q Q q Q EMC").is_ok());
}

#[test]
fn all_generated_fonts_need_explicit_nonempty_mappings() {
    let source = ChoiceTextBlock::parse(OLD).unwrap();
    let generated = ChoiceTextBlock::parse(NEW).unwrap();
    assert!(source.transplant(&generated, &BTreeMap::new()).is_err());
    assert!(source
        .transplant(&generated, &BTreeMap::from([(b"New".to_vec(), vec![])]))
        .is_err());
    let names = BTreeMap::from([(b"New".to_vec(), b"A name/with#escapes".to_vec())]);
    let output = source.transplant(&generated, &names).unwrap();
    assert!(ChoiceTextBlock::parse(&output)
        .unwrap()
        .font_names()
        .contains(b"A name/with#escapes".as_slice()));
}

#[test]
fn input_operation_depth_and_remapped_output_budgets_are_enforced() {
    assert!(ChoiceTextBlock::parse(&vec![b' '; 256 * 1024 + 1]).is_err());
    let many = format!("{} /Tx BMC q Q EMC", "q Q ".repeat(8193));
    assert!(ChoiceTextBlock::parse(many.as_bytes()).is_err());
    let deep = format!("/Tx BMC {} {} EMC", "q ".repeat(33), "Q ".repeat(33));
    assert!(ChoiceTextBlock::parse(deep.as_bytes()).is_err());
    let body = format!("/Tx BMC q BT {} ET Q EMC", "/New 12 Tf (x) Tj ".repeat(200));
    let generated = ChoiceTextBlock::parse(body.as_bytes()).unwrap();
    let source = ChoiceTextBlock::parse(OLD).unwrap();
    let big = BTreeMap::from([(b"New".to_vec(), vec![b'#'; 1024])]);
    assert!(source.transplant(&generated, &big).is_err());
}

#[test]
fn empty_vector_background_can_receive_text_without_importing_generated_artwork() {
    let background = b"q 1 0 0 rg 0 0 20 10 re f Q q 0 1 0 rg 80 0 20 10 re f Q";
    let old = ChoiceTextBlock::parse_empty_background(background).unwrap();
    let new = ChoiceTextBlock::parse(NEW).unwrap();
    let output = old.transplant(&new, &map()).unwrap();
    let expected = [
        background.as_slice(),
        b" /Tx BMC q BT /Old 12 Tf 2 3 Td (New) Tj ET Q EMC",
    ]
    .concat();
    assert_eq!(output, encoded(&expected));
    assert_eq!(
        old.transplant(&old, &BTreeMap::new()).unwrap(),
        encoded(background)
    );
}

#[test]
fn clearing_retains_the_original_text_layer_for_later_restore() {
    let source = ChoiceTextBlock::parse(OLD).unwrap();
    let empty = ChoiceTextBlock::parse_empty_background(b"q 0 g 0 0 100 20 re f Q").unwrap();
    let cleared = source.transplant(&empty, &BTreeMap::new()).unwrap();
    let expected = b"q 1 0 0 rg 0 0 20 10 re f Q /Tx BMC q Q EMC q 0 1 0 rg 80 0 20 10 re f Q";
    assert_eq!(cleared, encoded(expected));
    let slot = ChoiceTextBlock::parse(&cleared).unwrap();
    let names = BTreeMap::from([(b"Old".to_vec(), b"Old".to_vec())]);
    assert_eq!(slot.transplant(&source, &names).unwrap(), encoded(OLD));
}

#[test]
fn purported_empty_background_cannot_hide_text_resources_or_unfinished_state() {
    for raw in [
        b"/Tx BMC q Q EMC".as_slice(),
        b"/Tx BMC q Q",
        b"q BT /F 12 Tf (hidden) Tj ET Q",
        b"q /Image Do Q",
        b"q /State gs Q",
        b"q /Paint sh Q",
        b"q 0 0 10 10 re Q",
        b"q 0 0 10 10 re W Q",
        b"q 0 g",
        b"0 g",
        b"q BX EX Q",
    ] {
        assert!(
            ChoiceTextBlock::parse_empty_background(raw).is_err(),
            "{:?}",
            String::from_utf8_lossy(raw)
        );
    }
    assert!(ChoiceTextBlock::parse_empty_background(b"").is_ok());
    assert!(ChoiceTextBlock::parse_empty_background(&vec![b' '; 256 * 1024 + 1]).is_err());
}

fn widget(doc: &lopdf::Document, page: usize) -> lopdf::ObjectId {
    let page = doc
        .get_dictionary(*doc.get_pages().values().nth(page).unwrap())
        .unwrap();
    let annots = doc
        .dereference(page.get(b"Annots").unwrap())
        .unwrap()
        .1
        .as_array()
        .unwrap();
    annots[0].as_reference().unwrap()
}
fn appearance(doc: &lopdf::Document, page: usize) -> lopdf::Stream {
    let w = doc.get_dictionary(widget(doc, page)).unwrap();
    let ap = doc
        .dereference(w.get(b"AP").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    doc.dereference(ap.get(b"N").unwrap())
        .unwrap()
        .1
        .as_stream()
        .unwrap()
        .clone()
}
fn render(e: &crate::PdfEngine, bytes: &[u8], dir: &std::path::Path, name: &str) -> Vec<Vec<u8>> {
    use std::io::Write;
    let path = dir.join(format!("{name}.pdf"));
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap()
        .write_all(bytes)
        .unwrap();
    let source = e.open_document(&path).unwrap();
    let result = (0..source.pages.len())
        .map(|i| e.render_page(&source.id, i, 600).unwrap())
        .collect();
    e.close_document(&source.id).unwrap();
    result
}
#[test]
fn native_generated_choices_preserve_no_op_pixels_and_custom_artwork_structure() {
    use lopdf::Document;
    use std::path::PathBuf;
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/choice-text");
    let engine = crate::PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let dir = std::env::var_os("FOLIO_CHOICE_TEXT_EVIDENCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| temp.path().to_path_buf());
    std::fs::create_dir_all(&dir).unwrap();
    for case in ["combo", "list", "empty", "free", "artwork"] {
        let raw = std::fs::read(fixtures.join(format!("{case}-source.pdf"))).unwrap();
        let generated = Document::load(fixtures.join(format!("{case}-generated.pdf"))).unwrap();
        let original = render(&engine, &raw, &dir, &format!("{case}-original"));
        for changed in [false, true] {
            let mut source = Document::load_mem(&raw).unwrap();
            for page in 0..source.get_pages().len() {
                let saved = appearance(&source, page);
                let old = ChoiceTextBlock::parse(&saved.get_plain_content().unwrap()).unwrap();
                let new = if changed {
                    appearance(&generated, page)
                } else {
                    saved.clone()
                };
                let text = ChoiceTextBlock::parse(&new.get_plain_content().unwrap()).unwrap();
                let resources = |s: &lopdf::Stream, d: &Document| -> lopdf::Object {
                    let r = d
                        .dereference(s.dict.get(b"Resources").unwrap())
                        .unwrap()
                        .1
                        .as_dict()
                        .unwrap();
                    r.get(b"Font").unwrap().clone()
                };
                let gdoc = if changed { &generated } else { &source };
                // Fixture-specific graph proof. Matching names alone is insufficient.
                assert!(crate::form_appearance::Equivalence::default()
                    .check(
                        &source,
                        &resources(&saved, &source),
                        gdoc,
                        &resources(&new, gdoc)
                    )
                    .unwrap());
                let names = text
                    .font_names()
                    .iter()
                    .map(|n| (n.clone(), n.clone()))
                    .collect();
                let bytes = old.transplant(&text, &names).unwrap();
                let mut replacement = saved.clone();
                replacement.set_plain_content(bytes);
                let w = widget(&source, page);
                let ap = source
                    .dereference(source.get_dictionary(w).unwrap().get(b"AP").unwrap())
                    .unwrap()
                    .1
                    .as_dict()
                    .unwrap()
                    .clone();
                let mut ap = ap;
                ap.set("N", source.add_object(replacement));
                source.get_dictionary_mut(w).unwrap().set("AP", ap);
            }
            let bytes = crate::pdf_output::save(&mut source).unwrap();
            let pixels = render(&engine, &bytes, &dir, &format!("{case}-changed{changed}"));
            if changed && matches!(case, "combo" | "list" | "artwork") {
                assert!(pixels != original, "expected visible label change: {case}");
            } else {
                assert!(pixels == original, "expected unchanged pixels: {case}");
            }
        }
    }
}


#[test]
fn choice_mapping_is_inside_the_text_mark_and_preserves_both_artwork_layers() {
    let source = ChoiceTextBlock::parse(OLD).unwrap();
    let generated = ChoiceTextBlock::parse(NEW).unwrap();
    let result = source
        .transplant_mapped(&generated, &map(), [2., 0., 0., 3., 7., 11.])
        .unwrap();
    let expected = b"q 1 0 0 rg 0 0 20 10 re f Q /Tx BMC q 2 0 0 3 7 11 cm q BT /Old 12 Tf 2 3 Td (New) Tj ET Q Q EMC q 0 1 0 rg 80 0 20 10 re f Q";
    assert_eq!(result, encoded(expected));
    assert!(ChoiceTextBlock::parse(&result).is_ok());
    assert_eq!(source.transplant(&generated, &map()).unwrap(),
        source.transplant_mapped(&generated, &map(), [1., 0., 0., 1., 0., 0.]).unwrap());
}

#[test]
fn choice_mapping_refuses_invalid_or_non_axis_transforms() {
    let source = ChoiceTextBlock::parse(OLD).unwrap();
    let generated = ChoiceTextBlock::parse(NEW).unwrap();
    for transform in [
        [f32::NAN, 0., 0., 1., 0., 0.],
        [1., 0., 0., 1., f32::INFINITY, 0.],
        [0., 0., 0., 1., 0., 0.],
        [-1., 0., 0., 1., 0., 0.],
        [1., 1., 0., 1., 0., 0.],
        [1., 0., 1., 1., 0., 0.],
        [1e20, 0., 0., 1., 0., 0.],
    ] {
        assert!(source.transplant_mapped(&generated, &map(), transform).is_err(), "{transform:?}");
    }
}

#[test]
fn choice_mapping_charges_its_added_graphics_depth_and_operations() {
    let source = ChoiceTextBlock::parse(OLD).unwrap();
    let deepest = format!("/Tx BMC {} BT /New 12 Tf (x) Tj ET {} EMC", "q ".repeat(32), "Q ".repeat(32));
    let generated = ChoiceTextBlock::parse(deepest.as_bytes()).unwrap();
    assert!(source.transplant(&generated, &map()).is_ok());
    assert!(source.transplant_mapped(&generated, &map(), [2., 0., 0., 2., 0., 0.]).is_err());
    let almost_full = format!("/Tx BMC {} q Q EMC", "q Q ".repeat(8190));
    let generated = ChoiceTextBlock::parse(almost_full.as_bytes()).unwrap();
    let empty = ChoiceTextBlock::parse_empty_background(b"").unwrap();
    assert!(empty.transplant(&generated, &BTreeMap::new()).is_ok());
    assert!(empty.transplant_mapped(&generated, &BTreeMap::new(), [2., 0., 0., 2., 0., 0.]).is_err());
}

#[test]
fn choice_mapping_clear_and_restore_keep_the_original_text_slot() {
    let source = ChoiceTextBlock::parse(OLD).unwrap();
    let empty = ChoiceTextBlock::parse_empty_background(b"").unwrap();
    let cleared = source.transplant_mapped(&empty, &BTreeMap::new(), [2., 0., 0., 2., 7., 11.]).unwrap();
    let slot = ChoiceTextBlock::parse(&cleared).unwrap();
    let new = ChoiceTextBlock::parse(NEW).unwrap();
    assert_eq!(slot.transplant_mapped(&new, &map(), [2., 0., 0., 2., 7., 11.]).unwrap(),
        source.transplant_mapped(&new, &map(), [2., 0., 0., 2., 7., 11.]).unwrap());
}
