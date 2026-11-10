use super::*;
use crate::redaction_ranges::inspect_standard_glyphs;
use lopdf::{dictionary, Document, Object, Stream};
use sha2::{Digest, Sha256};

const SOURCE:&[u8]=b"q 0.9 0.2 0.1 rg 50 90 80 25 re f Q q 0.1 0.3 0.8 rg 160 90 40 25 re f Q BT /F1 12 Tf 1 0 0 1 20 160 Tm (Left SECRET Right) Tj ET BT /F1 12 Tf 1 0 0 1 20 40 Tm (Unchanged neighbor) Tj ET";
const EXPECTED:&[u8]=b"q 0.9 0.2 0.1 rg Q q 0.1 0.3 0.8 rg 160 90 40 25 re f Q BT /F1 12 Tf 1 0 0 1 20 160 Tm [(Left ) -3600 ( Right)] TJ ET BT /F1 12 Tf 1 0 0 1 20 40 Tm (Unchanged neighbor) Tj ET";
const SECOND:&[u8]=b"q 0.9 0.2 0.1 rg Q q 0.1 0.3 0.8 rg Q BT /F1 12 Tf 1 0 0 1 20 160 Tm [(Left ) -3600 ( Right)] TJ ET BT /F1 12 Tf 1 0 0 1 20 40 Tm (Unchanged neighbor) Tj ET";
fn fixture(program: &[u8]) -> Vec<u8> {
    let mut doc = Document::with_version("1.7");
    let tree = doc.new_object_id();
    let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier","Encoding"=>"WinAnsiEncoding"});
    let mut kids = vec![];
    for data in [program,b"0.2 0.7 0.3 rg 20 20 200 15 re f 0 g BT /F1 12 Tf 1 0 0 1 20 160 Tm (Second page stays editable) Tj ET".as_slice()]{
        let content=doc.add_object(Stream::new(dictionary!{},data.to_vec()));
        let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),200.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});kids.push(Object::Reference(page));
    }
    doc.objects.insert(
        tree,
        Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>kids,"Count"=>2}),
    );
    let root = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree});
    doc.trailer.set("Root", root);
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    bytes
}
fn runtime() -> WorkerRuntime {
    WorkerRuntime::new(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
        Arc::new(FontRegistry::new()),
    )
    .unwrap()
}
fn mark(page_index: usize, x: f64, y: f64, width: f64, height: f64) -> TextRedactionMark {
    TextRedactionMark {
        page_index,
        rect: TextMarkRect {
            x,
            y,
            width,
            height,
        },
    }
}
fn text(bytes: &[u8]) -> Vec<String> {
    inspect_standard_glyphs(&Document::load_mem(bytes).unwrap())
        .unwrap()
        .into_iter()
        .map(|b| b.source.text)
        .collect()
}
fn save(name: &str, bytes: &[u8]) {
    if let Some(root) = std::env::var_os("FOLIO_AREA_SELECTION_OUTPUT") {
        use std::io::Write;
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(std::path::Path::new(&root).join(name))
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }
}

#[test]
fn redaction_area_selection_derives_ranges_from_marks_and_preserves_ownership() {
    let mut rt = runtime();
    let source = fixture(SOURCE);
    let info = rt
        .register_generated_document(source.clone(), "Area source.pdf")
        .unwrap();
    let result = rt
        .redact_content_areas(
            &info.id,
            &[mark(0, 56.1, 25., 42.9, 20.), mark(0, 60., 95., 10., 10.)],
        )
        .unwrap();
    assert_eq!(result.source_id, info.id);
    assert_eq!(
        result.source_hash,
        <[u8; 32]>::from(Sha256::digest(&source))
    );
    assert_eq!(result.output.removed_characters, 6);
    assert_eq!(result.glyphs.len(), 6);
    assert!(result
        .glyphs
        .iter()
        .all(|g| g.page_index == 0 && g.bounds.valid()));
    assert_eq!(result.output.paths.len(), 1);
    assert_eq!(
        text(&result.output.bytes),
        vec![
            "Left  Right",
            "Unchanged neighbor",
            "Second page stays editable"
        ]
    );
    assert_eq!(rt.documents.len(), 1);
    assert_eq!(&*rt.document(&info.id).unwrap().bytes().unwrap(), source);
    let saved = rt
        .register_generated_document(result.output.bytes.clone(), "Saved.pdf")
        .unwrap();
    let second = rt
        .redact_content_areas(&saved.id, &[mark(0, 170., 95., 10., 10.)])
        .unwrap();
    assert_eq!(second.output.removed_characters, 0);
    assert_eq!(second.glyphs.len(), 0);
    assert_eq!(second.output.paths.len(), 1);
    assert_eq!(rt.documents.len(), 2);
    save("source.pdf", &source);
    save("actual.pdf", &result.output.bytes);
    save("expected.pdf", &fixture(EXPECTED));
    save("second.pdf", &second.output.bytes);
    save("expected-second.pdf", &fixture(SECOND));
}

#[test]
fn redaction_area_selection_space_only_marks_remove_actual_space_operands() {
    let mut rt = runtime();
    let source = fixture(b"BT /F1 12 Tf 1 0 0 1 20 160 Tm (A   B) Tj ET");
    let info = rt
        .register_generated_document(source.clone(), "Spaces.pdf")
        .unwrap();
    let result = rt
        .redact_content_areas(&info.id, &[mark(0, 28., 30., 20., 15.)])
        .unwrap();
    assert_eq!(result.output.removed_characters, 3);
    assert_eq!(result.glyphs.len(), 3);
    assert!(result.output.paths.is_empty());
    assert_eq!(
        text(&result.output.bytes),
        vec!["AB", "Second page stays editable"]
    );
    save("spaces-source.pdf", &source);
    save("spaces-actual.pdf", &result.output.bytes);
}

#[test]
fn redaction_area_selection_space_probe_covers_standard_fonts_and_transforms() {
    let mut rt = runtime();
    for face in [
        "Courier",
        "Courier-Bold",
        "Courier-Oblique",
        "Courier-BoldOblique",
        "Helvetica",
        "Helvetica-Bold",
        "Helvetica-Oblique",
        "Helvetica-BoldOblique",
        "Times-Roman",
        "Times-Bold",
        "Times-Italic",
        "Times-BoldItalic",
    ] {
        for matrix in [
            "1 0 0 1 60 100",
            "0 1 -1 0 100 60",
            "-1 0 0 1 150 100",
            "1 0.2 0.3 1 60 100",
        ] {
            let mut doc = Document::load_mem(&fixture(
                format!("BT /F1 12 Tf 2 Tc 3 Tw 80 Tz 4 Ts {matrix} Tm (A   B) Tj ET").as_bytes(),
            ))
            .unwrap();
            for object in doc.objects.values_mut() {
                if let Object::Dictionary(d) = object {
                    if d.get(b"Type").and_then(Object::as_name).ok() == Some(b"Font") {
                        d.set("BaseFont", Object::Name(face.as_bytes().to_vec()));
                    }
                }
            }
            let mut bytes = vec![];
            doc.save_to(&mut bytes).unwrap();
            let source = rt
                .register_generated_document(bytes.clone(), "Space transforms.pdf")
                .unwrap();
            let result = rt
                .redact_content_areas(&source.id, &[mark(0, 0., 0., 300., 200.)])
                .unwrap_or_else(|e| panic!("{face} / {matrix}: {e:?}"));
            assert_eq!(result.output.removed_characters, 5);
            assert_eq!(result.glyphs.len(), 5);
            assert_eq!(
                text(&result.output.bytes),
                vec!["Second page stays editable"]
            );
            assert_eq!(&*rt.document(&source.id).unwrap().bytes().unwrap(), bytes);
            rt.documents.remove(&source.id);
        }
    }
}

#[test]
fn redaction_area_selection_keeps_unselected_spaces_and_handles_all_space_runs() {
    let mut rt = runtime();
    let source = rt
        .register_generated_document(
            fixture(b"BT /F1 12 Tf 1 0 0 1 20 160 Tm (A   B) Tj ET"),
            "Separated spaces.pdf",
        )
        .unwrap();
    let result = rt
        .redact_content_areas(
            &source.id,
            &[mark(0, 28., 32., 1., 1.), mark(0, 43., 32., 1., 1.)],
        )
        .unwrap();
    assert_eq!(result.output.removed_characters, 2);
    assert_eq!(text(&result.output.bytes)[0], "A B");
    let only = rt
        .register_generated_document(
            fixture(b"BT /F1 12 Tf 1 0 0 1 20 160 Tm (   ) Tj ET"),
            "Only spaces.pdf",
        )
        .unwrap();
    let result = rt
        .redact_content_areas(&only.id, &[mark(0, 0., 0., 300., 200.)])
        .unwrap();
    assert_eq!(result.output.removed_characters, 3);
    assert_eq!(
        text(&result.output.bytes),
        vec!["Second page stays editable"]
    );
}

#[test]
fn redaction_area_selection_overlapping_marks_deduplicate_without_extra_spaces() {
    let mut rt = runtime();
    let info = rt
        .register_generated_document(fixture(SOURCE), "Overlap.pdf")
        .unwrap();
    let result = rt
        .redact_content_areas(
            &info.id,
            &[mark(0, 56.1, 25., 42.9, 20.), mark(0, 56.1, 25., 42.9, 20.)],
        )
        .unwrap();
    assert_eq!(result.output.removed_characters, 6);
    assert_eq!(result.glyphs.len(), 6);
    assert_eq!(text(&result.output.bytes)[0], "Left  Right");
}

#[test]
fn redaction_area_selection_white_text_and_generated_separators_do_not_escape() {
    let mut rt = runtime();
    for (program, area, removed, left) in [
        (
            b"1 g BT /F1 12 Tf 1 0 0 1 20 160 Tm (SECRET) Tj ET".as_slice(),
            mark(0, 19., 25., 45., 20.),
            6,
            "",
        ),
        (
            b"BT /F1 12 Tf 1 0 0 1 20 160 Tm (A) Tj 0 -20 Td (B) Tj ET",
            mark(0, 19., 50., 10., 20.),
            1,
            "A",
        ),
    ] {
        let info = rt
            .register_generated_document(fixture(program), "Measured.pdf")
            .unwrap();
        let result = rt.redact_content_areas(&info.id, &[area]).unwrap();
        assert_eq!(result.output.removed_characters, removed);
        let blocks =
            inspect_standard_glyphs(&Document::load_mem(&result.output.bytes).unwrap()).unwrap();
        assert_eq!(
            blocks
                .iter()
                .filter(|b| b.source.page_index == 0)
                .map(|b| b.source.text.as_str())
                .collect::<String>(),
            left
        );
    }
}

#[test]
fn redaction_area_selection_rotated_source_text_is_removed_by_native_boxes() {
    let mut rt = runtime();
    let info = rt
        .register_generated_document(
            fixture(b"BT /F1 12 Tf 0 1 -1 0 100 100 Tm (ABC) Tj ET"),
            "Rotated.pdf",
        )
        .unwrap();
    let result = rt
        .redact_content_areas(&info.id, &[mark(0, 85., 75., 20., 30.)])
        .unwrap();
    assert_eq!(result.output.removed_characters, 3);
    assert_eq!(result.glyphs.len(), 3);
    assert_eq!(
        text(&result.output.bytes),
        vec!["Second page stays editable"]
    );
}

#[test]
fn redaction_area_selection_invalid_blank_ambiguous_and_closed_sources_refuse() {
    let mut rt = runtime();
    let source = fixture(SOURCE);
    let info = rt
        .register_generated_document(source.clone(), "Invalid.pdf")
        .unwrap();
    for marks in [
        vec![],
        vec![mark(0, f64::NAN, 0., 1., 1.)],
        vec![mark(2, 0., 0., 1., 1.)],
        vec![mark(0, -1., 0., 1., 1.)],
        vec![mark(0, 299., 0., 2., 1.)],
        vec![mark(0, 250., 180., 1., 1.)],
        vec![mark(0, 0., 0., 0., 1.)],
        vec![mark(0, 56.1, 25., 42.9, 20.); 65],
    ] {
        assert!(rt.redact_content_areas(&info.id, &marks).is_err());
        assert_eq!(rt.documents.len(), 1);
    }
    rt.documents.get_mut(&info.id).unwrap().for_printing = true;
    assert!(rt
        .redact_content_areas(&info.id, &[mark(0, 56.1, 25., 42.9, 20.)])
        .is_err());
    rt.documents.get_mut(&info.id).unwrap().for_printing = false;
    assert_eq!(&*rt.document(&info.id).unwrap().bytes().unwrap(), source);
    rt.close_document(&info.id).unwrap();
    assert!(rt
        .redact_content_areas(&info.id, &[mark(0, 0., 0., 1., 1.)])
        .is_err());
    let duplicate = rt
        .register_generated_document(
            fixture(b"BT /F1 12 Tf 1 0 0 1 20 160 Tm (A) Tj 1 0 0 1 20 160 Tm (A) Tj ET"),
            "Ambiguous.pdf",
        )
        .unwrap();
    assert!(rt
        .redact_content_areas(&duplicate.id, &[mark(0, 19., 25., 10., 20.)])
        .is_err());
}
