use super::*;
use crate::redaction_ranges::{inspect_standard_glyphs, TextRange};
use lopdf::{dictionary, Document, Object, Stream};

const SOURCE: &[u8] = b"q 0.9 0.2 0.1 rg 50 90 80 25 re f Q q 0.1 0.3 0.8 rg 160 90 40 25 re f Q BT /F1 12 Tf 1 0 0 1 20 160 Tm (Left SECRET Right) Tj ET BT /F1 12 Tf 1 0 0 1 20 40 Tm (Unchanged neighbor) Tj ET";
const EXPECTED: &[u8] = b"q 0.9 0.2 0.1 rg Q q 0.1 0.3 0.8 rg 160 90 40 25 re f Q BT /F1 12 Tf 1 0 0 1 20 160 Tm [(Left ) -3600 ( Right)] TJ ET BT /F1 12 Tf 1 0 0 1 20 40 Tm (Unchanged neighbor) Tj ET";
const SECOND: &[u8] = b"q 0.9 0.2 0.1 rg Q q 0.1 0.3 0.8 rg Q BT /F1 12 Tf 1 0 0 1 20 160 Tm [(Left ) -3600 ( Right)] TJ ET BT /F1 12 Tf 1 0 0 1 20 40 Tm (Unchanged neighbor) Tj ET";

fn fixture(program: &[u8]) -> Document {
    let mut doc = Document::with_version("1.7");
    let tree = doc.new_object_id();
    let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier","Encoding"=>"WinAnsiEncoding"});
    let mut kids = vec![];
    for data in [program,b"0.2 0.7 0.3 rg 20 20 200 15 re f 0 g BT /F1 12 Tf 1 0 0 1 20 160 Tm (Second page stays editable) Tj ET".as_slice()] {
        let stream=doc.add_object(Stream::new(dictionary!{},data.to_vec()));
        let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),200.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>stream});
        kids.push(Object::Reference(page));
    }
    doc.objects.insert(
        tree,
        Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>kids,"Count"=>2}),
    );
    let root = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree});
    doc.trailer.set("Root", root);
    doc
}
fn bytes(doc: &Document) -> Vec<u8> {
    let mut b = vec![];
    doc.clone().save_to(&mut b).unwrap();
    b
}
fn area(page_index: usize, left: f64, bottom: f64, right: f64, top: f64) -> PageAreaMark {
    PageAreaMark {
        page_index,
        rect: AreaRect {
            left,
            bottom,
            right,
            top,
        },
    }
}
fn range(doc: &Document) -> TextRange {
    TextRange {
        selection_id: inspect_standard_glyphs(doc).unwrap()[0]
            .source
            .selection_id
            .clone(),
        start: 5,
        end: 11,
    }
}
fn text(doc: &Document) -> Vec<String> {
    inspect_standard_glyphs(doc)
        .unwrap()
        .into_iter()
        .map(|b| b.source.text)
        .collect()
}
fn ops(doc: &Document, page: usize) -> Vec<lopdf::content::Operation> {
    let id = *doc.get_pages().values().nth(page).unwrap();
    lopdf::content::Content::decode_strict(&doc.get_page_content(id))
        .unwrap()
        .operations
}
fn save(name: &str, data: &[u8]) {
    if let Some(root) = std::env::var_os("FOLIO_AREA_CONTENT_OUTPUT") {
        use std::io::Write;
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(std::path::Path::new(&root).join(name))
            .unwrap()
            .write_all(data)
            .unwrap();
    }
}

#[test]
fn combined_publication_removes_real_operands_preserves_neighbors_and_reedits() {
    let mut source = fixture(SOURCE);
    source.add_object(Stream::new(
        dictionary! {},
        b"UNREACHABLE_SECRET_913".to_vec(),
    ));
    let info = source.add_object(dictionary! {"Title"=>Object::string_literal("PRIVATE_INFO_913")});
    source.trailer.set("Info", info);
    let before = bytes(&source);
    let result =
        remove_planned_content(&source, &[range(&source)], &[area(0, 50., 90., 105., 175.)])
            .unwrap();
    assert_eq!(bytes(&source), before, "source mutated");
    assert_eq!(result.removed_characters, 6);
    assert_eq!(result.paths.len(), 1);
    assert_eq!(result.paths[0].page_index, 0);
    assert!(result.paths[0].painted && result.paths[0].extends_beyond_mark);
    assert!(result.paths[0].bounds.right >= 130.);
    assert!(result
        .omitted_metadata
        .contains(&"document information and identifiers"));
    for secret in [
        b"SECRET".as_slice(),
        b"UNREACHABLE_SECRET_913",
        b"PRIVATE_INFO_913",
    ] {
        assert!(!result.bytes.windows(secret.len()).any(|w| w == secret));
    }
    let actual = Document::load_mem(&result.bytes).unwrap();
    assert_eq!(
        text(&actual),
        vec![
            "Left  Right",
            "Unchanged neighbor",
            "Second page stays editable"
        ]
    );
    assert_eq!(
        ops(&actual, 0)
            .iter()
            .filter(|o| o.operator == "re")
            .count(),
        1
    );
    assert_eq!(
        ops(&actual, 1)
            .iter()
            .filter(|o| o.operator == "re")
            .count(),
        1
    );
    let again = remove_planned_content(&actual, &[], &[area(0, 170., 100., 180., 110.)]).unwrap();
    assert_eq!(again.paths.len(), 1);
    let reopened = Document::load_mem(&again.bytes).unwrap();
    assert_eq!(text(&reopened), text(&actual));
    assert_eq!(
        ops(&reopened, 0)
            .iter()
            .filter(|o| o.operator == "re")
            .count(),
        0
    );
    save("source.pdf", &before);
    save("actual.pdf", &result.bytes);
    save("expected.pdf", &bytes(&fixture(EXPECTED)));
    save("second.pdf", &again.bytes);
    save("expected-second.pdf", &bytes(&fixture(SECOND)));
}

#[test]
fn quote_expansion_and_split_text_arrays_do_not_reuse_old_path_indices() {
    for program in [
        b"BT /F1 12 Tf 18 TL 1 0 0 1 20 160 Tm (Left ) Tj (SECRET) ' ( Right) Tj ET 50 90 80 25 re f".as_slice(),
        b"BT /F1 12 Tf 18 TL 1 0 0 1 20 160 Tm (Left ) Tj 0 0 (SECRET) \" ( Right) Tj ET 50 90 80 25 re f",
        b"BT /F1 12 Tf 1 0 0 1 20 160 Tm [(Left ) 20 (SEC) -20 (RET Right)] TJ ET 50 90 80 25 re f",
    ] {
        let source=fixture(program);
        let result=remove_planned_content(&source,&[range(&source)],&[area(0,60.,100.,70.,110.)]).unwrap();
        let output=Document::load_mem(&result.bytes).unwrap();
        assert_eq!(text(&output)[0],"Left  Right");
        assert_eq!(ops(&output,0).iter().filter(|o|o.operator=="re").count(),0);
        assert_eq!(result.paths.len(),1);
    }
}

#[test]
fn path_only_and_nonpainted_geometry_are_removed_on_multiple_pages() {
    let source = fixture(b"q 0 -1 -1 0 200 180 cm 40 30 -20 -10 re f Q 70 80 m 90 90 l n");
    let result = remove_planned_content(
        &source,
        &[],
        &[
            area(0, 170., 155., 175., 165.),
            area(0, 80., 85., 85., 90.),
            area(1, 25., 25., 30., 30.),
        ],
    )
    .unwrap();
    assert_eq!(result.paths.len(), 3);
    assert_eq!(result.paths.iter().filter(|p| !p.painted).count(), 1);
    assert_eq!(result.removed_characters, 0);
    let output = Document::load_mem(&result.bytes).unwrap();
    assert!(ops(&output, 0)
        .iter()
        .all(|o| !matches!(o.operator.as_str(), "re" | "m" | "l" | "n")));
    assert_eq!(text(&output), vec!["Second page stays editable"]);
}

#[test]
fn stroke_extent_selects_path_when_mark_misses_centerline() {
    let source = fixture(b"8 w 0 j 4 M 2 J 50 50 m 90 50 l s");
    let result = remove_planned_content(&source, &[], &[area(0, 20., 49., 25., 51.)]).unwrap();
    assert_eq!(result.paths.len(), 1);
    assert!(result.paths[0].bounds.left <= 18.);
}

#[test]
fn stale_ranges_invalid_marks_and_no_change_are_refused_without_source_mutation() {
    let source = fixture(SOURCE);
    let original = bytes(&source);
    for marks in [
        vec![],
        vec![area(2, 0., 0., 1., 1.)],
        vec![area(0, -1., 0., 1., 1.)],
        vec![area(0, 0., 0., 301., 1.)],
        vec![area(0, 0., 0., 1., 201.)],
        vec![area(0, 0., 0., 0., 1.)],
        vec![area(0, f64::NAN, 0., 1., 1.)],
        vec![area(0, 250., 180., 260., 190.)],
    ] {
        assert!(remove_planned_content(&source, &[], &marks).is_err());
    }
    let mut bad = range(&source);
    bad.selection_id = "0".repeat(64);
    assert!(remove_planned_content(&source, &[bad], &[area(0, 50., 90., 105., 175.)]).is_err());
    assert_eq!(bytes(&source), original);
}

#[test]
fn unsupported_content_and_unmarked_page_clips_refuse_entire_candidate() {
    for program in [
        b"0 0 10 10 re W n".as_slice(),
        b"0 0 10 10 re W* n",
        b"/Unknown Do",
        b"0 w 50 50 m 90 50 l S",
        b"50 50 m q 90 50 l S Q",
    ] {
        let source = fixture(program);
        let original = bytes(&source);
        assert!(remove_planned_content(&source, &[], &[area(1, 25., 25., 30., 30.)]).is_err());
        assert_eq!(bytes(&source), original);
    }
    let mut source = fixture(b"/G1 gs 50 50 20 20 re f");
    let page = *source.get_pages().values().next().unwrap();
    source
        .get_dictionary_mut(page)
        .unwrap()
        .get_mut(b"Resources")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("ExtGState", dictionary! {"G1"=>dictionary!{"SA"=>true}});
    assert!(remove_planned_content(&source, &[], &[area(0, 55., 55., 60., 60.)]).is_err());
    let mut source = fixture(SOURCE);
    let page = *source.get_pages().values().next().unwrap();
    source.get_dictionary_mut(page).unwrap().set(
        "Annots",
        vec![Object::Dictionary(dictionary! {"Subtype"=>"Text"})],
    );
    assert!(remove_planned_content(
        &source,
        &[range(&fixture(SOURCE))],
        &[area(0, 50., 90., 105., 175.)]
    )
    .is_err());
}
