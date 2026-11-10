//! Actual fresh-graph publisher coverage. Page admission is still separately gated.
use crate::{
    redaction_content::Program,
    redaction_core::{publish, Page},
    redaction_images::{Image, Registry},
    redaction_jpeg::Limits,
    redaction_jpeg_coverage::{Coverage, CoverageBudget},
};
use lopdf::{content::Content, dictionary, Document, Object, Stream};
use std::sync::Arc;

fn source() -> Arc<Image> {
    let mut pdf = Document::new();
    let value=Object::Reference(pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>64,"Height"=>48,"BitsPerComponent"=>8,"ColorSpace"=>"DeviceRGB","Filter"=>"DCTDecode"},include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub2-progressive1.jpg").to_vec())));
    Registry::default().load(&pdf, &value).unwrap()
}
fn redacted(source: &Image, index: usize) -> Arc<Image> {
    let mut mask = vec![0; 12];
    mask[index] = 1;
    let coverage = Coverage::from_mask(
        source.jpeg_header().unwrap(),
        &mask,
        &mut CoverageBudget::default(),
    )
    .unwrap();
    Arc::new(
        source
            .redacted_jpeg(&coverage, Limits::default(), &mut CoverageBudget::default())
            .unwrap(),
    )
}
fn page(image: Arc<Image>, matrix: [f32; 6]) -> Page {
    let data = format!(
        "q {} {} {} {} {} {} cm /I0 Do Q",
        matrix[0], matrix[1], matrix[2], matrix[3], matrix[4], matrix[5]
    );
    Page {
        width: 192.into(),
        height: 160.into(),
        fonts: vec![],
        graphics: vec![],
        images: vec![image],
        program: Program {
            operations: Content::decode_strict(data.as_bytes()).unwrap().operations,
            blocks: vec![],
        },
    }
}
fn reopen(data: &[u8]) -> (Arc<Image>, Vec<lopdf::content::Operation>) {
    let pdf = Document::load_mem(data).unwrap();
    let page = pdf.get_pages()[&1];
    let dict = pdf.get_object(page).unwrap().as_dict().unwrap();
    let value = dict
        .get(b"Resources")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"XObject")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"I0")
        .unwrap();
    let image = Registry::default().load(&pdf, value).unwrap();
    let ops = Content::decode_strict(&pdf.get_page_content(page))
        .unwrap()
        .operations;
    (image, ops)
}
#[test]
fn redaction_jpeg_publication_saves_reopens_and_repeats_owned_fill_without_drift() {
    let source = source();
    let first = redacted(&source, 5);
    let first_bytes = publish(vec![page(first.clone(), [128., 0., 0., 96., 20., 24.])]).unwrap();
    let (loaded, ops) = reopen(&first_bytes);
    let index = ops.iter().position(|op| op.operator == "Do").unwrap();
    assert_eq!(
        loaded
            .verify_paint_prefix(&ops[index + 1..], &mut CoverageBudget::default())
            .unwrap(),
        12
    );
    let logical = crate::redaction_images::strip_owned_paint(
        ops,
        &[loaded.clone()],
        &mut CoverageBudget::default(),
    )
    .unwrap();
    let mut next = page(loaded.clone(), [128., 0., 0., 96., 20., 24.]);
    next.program.operations = logical;
    assert_eq!(publish(vec![next]).unwrap(), first_bytes);
    let second = redacted(&loaded, 6);
    assert_eq!(
        second.jpeg_coverage().unwrap().mask(),
        &[0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0]
    );
    for (name, matrix) in [
        ("axis", [128., 0., 0., 96., 20., 24.]),
        ("rotate", [0., 128., -96., 0., 116., 16.]),
        ("reflect", [-128., 0., 0., 96., 156., 24.]),
        ("shear", [100., 10., 25., 90., 20., 20.]),
        ("fractional", [123.5, 0., 0., 95.25, 21.125, 22.5]),
        ("small", [8., 0., 0., 6., 40., 40.]),
    ] {
        for (label, image) in [
            ("source", source.clone()),
            ("prior", first.clone()),
            ("actual", second.clone()),
        ] {
            let bytes = publish(vec![page(image.clone(), matrix)]).unwrap();
            let (loaded, ops) = reopen(&bytes);
            assert_eq!(loaded.digest, image.digest);
            let index = ops.iter().position(|op| op.operator == "Do").unwrap();
            assert_eq!(
                loaded
                    .verify_paint_prefix(&ops[index + 1..], &mut CoverageBudget::default())
                    .unwrap(),
                if label == "source" { 0 } else { 12 }
            );
            if let Some(root) = std::env::var_os("FOLIO_JPEG_OWNERSHIP_OUTPUT") {
                let root = std::path::PathBuf::from(root);
                std::fs::create_dir_all(&root).unwrap();
                std::fs::write(root.join(format!("{name}-{label}.pdf")), bytes).unwrap();
            }
        }
    }
}
