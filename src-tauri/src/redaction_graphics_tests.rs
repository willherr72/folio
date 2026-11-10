use crate::redaction_ranges::{inspect_standard_glyphs, remove_standard_text_ranges, TextRange};
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};

fn state() -> Dictionary {
    dictionary! {"Type"=>"ExtGState", "OP"=>false, "op"=>false, "OPM"=>1, "SA"=>false, "SM"=>Object::Real(0.02)}
}
fn fixture(s: Dictionary, program: &[u8]) -> (Document, ObjectId, ObjectId) {
    let mut d = Document::with_version("1.7");
    let tree = d.new_object_id();
    let font = d.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica","Encoding"=>"WinAnsiEncoding"});
    let gs = d.add_object(s);
    let stream = d.add_object(Stream::new(dictionary! {}, program.to_vec()));
    let page = d.add_object(dictionary! {"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),500.into(),300.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font},"ExtGState"=>dictionary!{"Original"=>gs,"Alias"=>gs}},"Contents"=>stream});
    d.objects.insert(
        tree,
        Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}),
    );
    let root = d.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree});
    d.trailer.set("Root", root);
    (d, page, gs)
}
const PROGRAM: &[u8] = b"q /Original gs 0.8 0.9 1 rg 15 225 180 25 re f Q 0 g BT /F1 12 Tf 1 0 0 1 20 240 Tm (Left ) Tj /Alias gs (SECRET) Tj /Original gs ( Right) Tj ET 0.2 G 2 w 20 50 m 450 50 l S";
fn remove(d: &Document, start: usize, end: usize) -> Vec<u8> {
    let b = inspect_standard_glyphs(d).expect("opaque source must be admitted");
    remove_standard_text_ranges(
        d,
        &[TextRange {
            selection_id: b[0].source.selection_id.clone(),
            start,
            end,
        }],
    )
    .unwrap()
    .bytes
}

#[test]
fn redaction_graphics_retains_paint_state_order_and_every_retained_glyph() {
    let (d, _, _) = fixture(state(), PROGRAM);
    let before = inspect_standard_glyphs(&d).expect("producer opaque graphics state");
    let bytes = remove(&d, 5, 11);
    let out = Document::load_mem(&bytes).unwrap();
    let after = inspect_standard_glyphs(&out).unwrap();
    assert_eq!(after[0].source.text, "Left  Right");
    for (a, b) in before[0]
        .glyphs
        .iter()
        .filter(|g| g.offset < 5 || g.offset >= 11)
        .zip(&after[0].glyphs)
    {
        assert_eq!(a.code, b.code);
        for (u, v) in a
            .origin
            .iter()
            .chain(a.end.iter())
            .zip(b.origin.iter().chain(b.end.iter()))
        {
            assert!((u - v).abs() < 0.002);
        }
    }
    let page = *out.get_pages().values().next().unwrap();
    let resources = out
        .get_dictionary(page)
        .unwrap()
        .get(b"Resources")
        .unwrap()
        .as_dict()
        .unwrap();
    let gs = resources.get(b"ExtGState").unwrap().as_dict().unwrap();
    assert_eq!(gs.len(), 1, "aliases must share output resource");
    let published = out
        .get_dictionary(gs.get(b"G0").unwrap().as_reference().unwrap())
        .unwrap();
    assert_eq!(format!("{published:?}"), format!("{:?}", state()));
    let ops = lopdf::content::Content::decode(&out.get_page_content(page))
        .unwrap()
        .operations;
    assert_eq!(ops.iter().filter(|o| o.operator == "gs").count(), 3);
    assert!(ops
        .iter()
        .filter(|o| o.operator == "gs")
        .all(|o| o.operands[0].as_name().unwrap() == b"G0"));
    let twice = Document::load_mem(&remove(&out, 0, 4)).unwrap();
    assert_eq!(
        inspect_standard_glyphs(&twice).unwrap()[0].source.text,
        "  Right"
    );
    if let Some(root) = std::env::var_os("FOLIO_GRAPHICS_REMOVAL_OUT") {
        use std::io::Write;
        let root = std::path::PathBuf::from(root);
        let mut original = d.clone();
        let mut input = vec![];
        original.save_to(&mut input).unwrap();
        for (name, data) in [
            ("input.pdf", input),
            ("removed.pdf", bytes),
            ("reedited.pdf", remove(&out, 0, 4)),
        ] {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(root.join(name))
                .unwrap()
                .write_all(&data)
                .unwrap();
        }
    }
}

#[test]
fn redaction_graphics_state_changes_invalidate_selection() {
    let (mut d, _, gs) = fixture(state(), PROGRAM);
    let b = inspect_standard_glyphs(&d).unwrap();
    d.get_dictionary_mut(gs)
        .unwrap()
        .set("SM", Object::Real(0.5));
    assert!(remove_standard_text_ranges(
        &d,
        &[TextRange {
            selection_id: b[0].source.selection_id.clone(),
            start: 5,
            end: 11
        }]
    )
    .is_err());
    assert!(inspect_standard_glyphs(&d).is_ok());
}

#[test]
fn redaction_graphics_refuses_unknown_nonopaque_and_malformed_states() {
    for (key, value) in [
        ("ca", Object::Real(0.9)),
        ("CA", Object::Real(0.0)),
        ("BM", Object::Name(b"Multiply".to_vec())),
        ("SMask", Object::Dictionary(dictionary! {})),
        ("Font", Object::Null),
        ("TR", Object::Name(b"Identity".to_vec())),
        ("OPM", Object::Integer(2)),
        ("SA", Object::Integer(0)),
        ("SM", Object::Real(f32::NAN)),
        ("SM", Object::Real(-0.1)),
        ("SM", Object::Real(1.1)),
        ("Type", Object::Name(b"Other".to_vec())),
    ] {
        let mut s = state();
        s.set(key, value);
        let (d, _, _) = fixture(s, PROGRAM);
        assert!(inspect_standard_glyphs(&d).is_err(), "{key}");
    }
    for content in [
        b"/Missing gs".as_slice(),
        b"0 gs",
        b"/Original /Alias gs",
        b"gs",
    ] {
        let (d, _, _) = fixture(state(), content);
        assert!(inspect_standard_glyphs(&d).is_err());
    }
    let (mut d, _, gs) = fixture(state(), PROGRAM);
    d.objects.insert(gs, Object::Reference(gs));
    assert!(inspect_standard_glyphs(&d).is_err());
}

#[test]
fn redaction_graphics_indirect_scalars_partial_state_and_inherited_resources() {
    let (mut d, page, gs) = fixture(
        dictionary! {"OP"=>true,"ca"=>1,"CA"=>Object::Real(1.0),"BM"=>"Normal","SMask"=>"None"},
        PROGRAM,
    );
    let value = d.add_object(Object::Boolean(true));
    d.get_dictionary_mut(gs).unwrap().set("OP", value);
    let resources = d
        .get_dictionary_mut(page)
        .unwrap()
        .remove(b"Resources")
        .unwrap();
    let tree = d
        .get_dictionary(page)
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    d.get_dictionary_mut(tree)
        .unwrap()
        .set("Resources", resources);
    let out = Document::load_mem(&remove(&d, 5, 11)).unwrap();
    assert_eq!(
        inspect_standard_glyphs(&out).unwrap()[0].source.text,
        "Left  Right"
    );
    d.objects.insert(value, Object::Reference(value));
    assert!(inspect_standard_glyphs(&d).is_err());
}

fn many_states(pages: usize, per_page: usize, shared: bool) -> Document {
    let (mut d, page, _) = fixture(state(), PROGRAM);
    let template = d.get_dictionary(page).unwrap().clone();
    let tree = template.get(b"Parent").unwrap().as_reference().unwrap();
    let mut kids = vec![];
    for p in 0..pages {
        let mut resource = template
            .get(b"Resources")
            .unwrap()
            .as_dict()
            .unwrap()
            .clone();
        let mut graphics = Dictionary::new();
        let mut content = String::new();
        for index in 0..per_page {
            let n = if shared { 1 } else { p * per_page + index };
            graphics.set(
                format!("S{index}"),
                dictionary! {"SM"=>Object::Real(n as f32/1000.0)},
            );
            content.push_str(&format!("/S{index} gs\n"));
        }
        content.push_str("BT /F1 12 Tf 20 240 Td (Left SECRET Right) Tj ET");
        resource.set("ExtGState", graphics);
        let stream = d.add_object(Stream::new(dictionary! {}, content.into_bytes()));
        let mut next = template.clone();
        next.set("Resources", resource);
        next.set("Contents", stream);
        let id = d.add_object(next);
        kids.push(Object::Reference(id));
    }
    d.get_dictionary_mut(tree)
        .unwrap()
        .set("Count", pages as i64);
    d.get_dictionary_mut(tree).unwrap().set("Kids", kids);
    d
}

#[test]
fn redaction_graphics_bounds_unique_page_and_document_states_but_shares_aliases() {
    assert!(inspect_standard_glyphs(&many_states(1, 64, false)).is_ok());
    assert!(inspect_standard_glyphs(&many_states(1, 65, false)).is_err());
    assert!(inspect_standard_glyphs(&many_states(4, 64, false)).is_ok());
    assert!(inspect_standard_glyphs(&many_states(5, 64, false)).is_err());
    let d = many_states(5, 128, true);
    let p = crate::redaction_core::prepare(&d).unwrap();
    assert!(p.pages.iter().all(|page| page.graphics.len() == 1));
    assert!(p
        .pages
        .iter()
        .all(|page| std::sync::Arc::ptr_eq(&page.graphics[0], &p.pages[0].graphics[0])));
    let out = Document::load_mem(&remove(&d, 5, 11)).unwrap();
    let ids: std::collections::HashSet<_> = out
        .get_pages()
        .values()
        .map(|page| {
            out.get_dictionary(*page)
                .unwrap()
                .get(b"Resources")
                .unwrap()
                .as_dict()
                .unwrap()
                .get(b"ExtGState")
                .unwrap()
                .as_dict()
                .unwrap()
                .get(b"G0")
                .unwrap()
                .as_reference()
                .unwrap()
        })
        .collect();
    assert_eq!(ids.len(), 1);
}

#[test]
fn redaction_graphics_reference_depth_empty_states_and_key_order_are_bounded() {
    for depth in [64, 65] {
        let (mut d, _, gs) = fixture(dictionary! {}, PROGRAM);
        let mut value = Object::Boolean(false);
        for _ in 0..depth {
            value = Object::Reference(d.add_object(value));
        }
        d.get_dictionary_mut(gs).unwrap().set("OP", value);
        assert_eq!(inspect_standard_glyphs(&d).is_ok(), depth == 64);
    }
    let (d, _, _) = fixture(dictionary! {}, PROGRAM);
    assert!(inspect_standard_glyphs(&d).is_ok());
    let (mut d, _, gs) = fixture(state(), PROGRAM);
    let id = inspect_standard_glyphs(&d).unwrap()[0]
        .source
        .selection_id
        .clone();
    let mut reversed = Dictionary::new();
    let entries: Vec<_> = state()
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for (k, v) in entries.into_iter().rev() {
        reversed.set(k, v);
    }
    d.objects.insert(gs, Object::Dictionary(reversed));
    assert_eq!(
        inspect_standard_glyphs(&d).unwrap()[0].source.selection_id,
        id
    );
}

#[test]
#[ignore = "requires explicit private local PDF and report paths; compatibility probe, not acceptance"]
fn redaction_graphics_private_producer_probe() {
    use crate::redaction_content::TextFonts;
    use sha2::{Digest, Sha256};
    use std::io::Write;
    let path = std::path::PathBuf::from(
        std::env::var_os("FOLIO_PRIVATE_PROBE_INPUT").expect("explicit input"),
    );
    let output = std::path::PathBuf::from(
        std::env::var_os("FOLIO_PRIVATE_PROBE_REPORT").expect("explicit report"),
    );
    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.len() < 1024 * 1024);
    let d = Document::load_mem(&bytes).unwrap();
    let mut font_results = vec![];
    let mut registry = crate::redaction_fonts::Registry::default();
    for page in d.get_pages().values() {
        let resources = d.dereference(d.get_dictionary(*page).unwrap().get(b"Resources").unwrap()).unwrap().1.as_dict().unwrap();
        let (_, fonts) = d.dereference(resources.get(b"Font").unwrap()).unwrap();
        let mut resolver =
            crate::redaction_fonts::PageFonts::new(&d, Some(resources), &mut registry);
        for (name, _) in fonts.as_dict().unwrap() {
            font_results.push(resolver.select(name).is_ok());
        }
    }
    let inspection = inspect_standard_glyphs(&d);
    let (accepted, blocks, glyphs, removal) = match inspection {
        Ok(blocks) => {
            let glyphs = blocks.iter().map(|b| b.glyphs.len()).sum::<usize>();
            let first = blocks.iter().find(|b| !b.glyphs.is_empty()).unwrap();
            let result = remove_standard_text_ranges(
                &d,
                &[TextRange {
                    selection_id: first.source.selection_id.clone(),
                    start: 0,
                    end: 1,
                }],
            );
            (true, blocks.len(), glyphs, result.is_ok())
        }
        Err(_) => (false, 0, 0, false),
    };
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let report = serde_json::json!({"inputSha256":Sha256::digest(&bytes).iter().map(|b|format!("{b:02x}")).collect::<String>(),"bytes":bytes.len(),"pageCount":d.get_pages().len(),"fontProofAccepted":font_results,"inspectionAccepted":accepted,"blocks":blocks,"glyphs":glyphs,"inMemoryRemovalAccepted":removal,"originalUnchanged":true,"note":"No private text or edited PDF persisted; not desktop or independent raster acceptance."});
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output)
        .unwrap()
        .write_all(serde_json::to_string_pretty(&report).unwrap().as_bytes())
        .unwrap();
}
