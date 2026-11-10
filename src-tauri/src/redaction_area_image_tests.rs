use super::*;
use lopdf::{content::Content, dictionary, Document, Object, Stream};

#[test]
fn redaction_area_image_context_survives_text_rewrite_publication_and_reopen() {
    let source = Document::load_mem(&fixture(pixels(), None, false)).unwrap();
    let mut context = crate::redaction_images::ImageContext::default();
    let first = crate::redaction_core::prepare_area_with_context(&source, &mut context).unwrap();
    let original = first.pages[0].images[0].clone();
    let repeated = crate::redaction_core::prepare_area_with_context(&source, &mut context).unwrap();
    assert!(Arc::ptr_eq(&original, &repeated.pages[0].images[0]));
    let expected = context.redact_pixels(&original, &[true; 64]).unwrap();
    let blocks =
        crate::redaction_ranges::inspect_area_glyphs_with_context(&source, &mut context).unwrap();
    let ranges = [crate::redaction_ranges::TextRange {
        selection_id: blocks[0].source.selection_id.clone(),
        start: 5,
        end: 11,
    }];
    let marks = [PageAreaMark {
        page_index: 0,
        rect: AreaRect {
            left: 20.,
            bottom: 20.,
            right: 100.,
            top: 100.,
        },
    }];
    let output =
        remove_planned_content_with_context(&source, &ranges, &marks, &mut context).unwrap();
    assert_eq!(output.removed_characters, 6);
    assert_eq!(output.images.len(), 1);
    let saved = Document::load_mem(&output.bytes).unwrap();
    let reopened = crate::redaction_core::prepare_area_with_context(&saved, &mut context).unwrap();
    assert!(Arc::ptr_eq(&expected, &reopened.pages[0].images[0]));
    assert!(Arc::ptr_eq(&original, &reopened.pages[0].images[1]));
    assert!(Arc::ptr_eq(
        &first.pages[1].images[0],
        &reopened.pages[1].images[0]
    ));
    assert_eq!(original.pixel_samples().unwrap(), pixels());
}

fn pixels() -> Vec<u8> {
    (0..8)
        .flat_map(|y| (0..8).flat_map(move |x| [20 + x * 20, 20 + y * 20, 30 + x * 5 + y * 5]))
        .collect()
}
fn image(pdf: &mut Document, samples: Vec<u8>, rgb: bool) -> lopdf::ObjectId {
    let mut stream = Stream::new(
        dictionary! {"Type"=>"XObject","Subtype"=>"Image","Width"=>8,"Height"=>8,
        "BitsPerComponent"=>8,"ColorSpace"=>if rgb{"DeviceRGB"}else{"DeviceGray"}},
        samples,
    );
    stream.compress().unwrap();
    pdf.add_object(stream)
}
fn fixture(first: Vec<u8>, second: Option<Vec<u8>>, removed_text: bool) -> Vec<u8> {
    let mut pdf = Document::with_version("1.7");
    let tree = pdf.new_object_id();
    let a = image(&mut pdf, first, true);
    let b = second.map(|s| image(&mut pdf, s, true)).unwrap_or(a);
    let gray = image(&mut pdf, (0..64).map(|n| n * 3).collect(), false);
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier","Encoding"=>"WinAnsiEncoding"});
    let tail = if removed_text {
        "BT /F1 12 Tf 1 0 0 1 20 160 Tm [(Left ) -3600 ( Right)] TJ ET"
    } else {
        "BT /F1 12 Tf 1 0 0 1 20 160 Tm (Left SECRET Right) Tj ET q 0.9 0.2 0.1 rg 50 110 80 10 re f Q"
    };
    let first_program =
        format!("q 80 0 0 80 20 20 cm /A Do Q q 80 0 0 80 140 20 cm /B Do Q {tail}");
    let second_program=b"q 80 0 0 80 20 20 cm /Gray Do Q BT /F1 12 Tf 1 0 0 1 20 160 Tm (Unchanged second page) Tj ET";
    let mut kids = vec![];
    for (program, xobjects) in [
        (first_program.as_bytes(), dictionary! {"A"=>a,"B"=>b}),
        (second_program.as_slice(), dictionary! {"Gray"=>gray}),
    ] {
        let content = pdf.add_object(Stream::new(dictionary! {}, program.to_vec()));
        let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),200.into()],
            "Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font},"XObject"=>xobjects},"Contents"=>content});
        kids.push(Object::Reference(page));
    }
    pdf.objects.insert(
        tree,
        dictionary! {"Type"=>"Pages","Kids"=>kids,"Count"=>2}.into(),
    );
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree});
    pdf.trailer.set("Root", root);
    // Unreachable old image samples must not survive fresh publication.
    image(&mut pdf, pixels(), true);
    let mut bytes = vec![];
    pdf.save_to(&mut bytes).unwrap();
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
fn samples(bytes: &[u8], page_index: usize) -> Vec<Vec<u8>> {
    let pdf = Document::load_mem(bytes).unwrap();
    let id = *pdf.get_pages().values().nth(page_index).unwrap();
    let page = pdf.get_object(id).unwrap().as_dict().unwrap();
    let resources = pdf
        .dereference(page.get(b"Resources").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    let images = pdf
        .dereference(resources.get(b"XObject").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    Content::decode(&pdf.get_page_content(id))
        .unwrap()
        .operations
        .iter()
        .filter(|o| o.operator == "Do")
        .map(|op| {
            let alias = op.operands[0].as_name().unwrap();
            let stream = pdf
                .dereference(images.get(alias).unwrap())
                .unwrap()
                .1
                .as_stream()
                .unwrap();
            stream.decompressed_content_strict_with_limit(1024).unwrap()
        })
        .collect()
}
fn save(name: &str, bytes: &[u8]) {
    if let Some(root) = std::env::var_os("FOLIO_AREA_IMAGE_OUTPUT") {
        use std::io::Write;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(PathBuf::from(root).join(name))
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }
}
#[test]
fn redaction_area_images_partial_samples_isolate_occurrences_and_survive_reedit() {
    let mut rt = runtime();
    let original = fixture(pixels(), None, false);
    let info = rt
        .register_generated_document(original.clone(), "Shared images.pdf")
        .unwrap();
    assert!(
        rt.inspect_redaction_text_page(&info.id, 0).is_err(),
        "existing text-only image admission stays unchanged"
    );
    let first = rt
        .redact_content_areas(
            &info.id,
            &[
                mark(0, 31., 111., 8., 8.),
                mark(0, 56.1, 25., 42.9, 20.),
                mark(0, 60., 82., 10., 6.),
            ],
        )
        .unwrap();
    let mut wanted = pixels();
    wanted[9 * 3..10 * 3].fill(0);
    assert_eq!(
        samples(&first.output.bytes, 0),
        vec![wanted.clone(), pixels()]
    );
    assert_eq!(samples(&first.output.bytes, 1), samples(&original, 1));
    assert_eq!(first.output.removed_characters, 6);
    assert_eq!(first.output.paths.len(), 1);
    assert_eq!(first.output.images.len(), 1);
    let affected = &first.output.images[0];
    assert_eq!(
        (
            affected.page_index,
            affected.occurrence_index,
            affected.pixels
        ),
        (0, 0, 1)
    );
    assert_eq!(
        affected.bounds,
        crate::redaction_area_paths::AreaRect {
            left: 30.,
            bottom: 80.,
            right: 40.,
            top: 90.
        }
    );
    assert!(affected.extends_beyond_mark);
    assert_eq!(rt.documents.len(), 1);
    assert_eq!(&*rt.document(&info.id).unwrap().bytes().unwrap(), original);
    let saved = rt
        .register_generated_document(first.output.bytes.clone(), "Saved.pdf")
        .unwrap();
    let second = rt
        .redact_content_areas(&saved.id, &[mark(0, 151., 111., 8., 8.)])
        .unwrap();
    assert_eq!(
        samples(&second.output.bytes, 0),
        vec![wanted.clone(), wanted.clone()]
    );
    assert_eq!(samples(&second.output.bytes, 1), samples(&original, 1));
    let reopened = Document::load_mem(&second.output.bytes).unwrap();
    for object in reopened.objects.values() {
        if let Object::Stream(s) = object {
            if s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image") {
                assert_ne!(
                    s.decompressed_content_strict_with_limit(1024).unwrap(),
                    pixels()
                );
            }
        }
    }
    save("image-source.pdf", &original);
    save("image-actual.pdf", &first.output.bytes);
    save("image-second.pdf", &second.output.bytes);
    save(
        "image-expected.pdf",
        &fixture(wanted.clone(), Some(pixels()), true),
    );
    save(
        "image-expected-second.pdf",
        &fixture(wanted.clone(), Some(wanted), true),
    );
}
#[test]
fn redaction_area_images_unknown_image_semantics_refuse_without_candidate() {
    let mut rt = runtime();
    let mut pdf = Document::load_mem(&fixture(pixels(), None, false)).unwrap();
    for obj in pdf.objects.values_mut() {
        if let Object::Stream(s) = obj {
            if s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image") {
                s.dict.set("Interpolate", true);
            }
        }
    }
    let mut bytes = vec![];
    pdf.save_to(&mut bytes).unwrap();
    let source = rt
        .register_generated_document(bytes.clone(), "Unsupported.pdf")
        .unwrap();
    assert!(rt
        .redact_content_areas(&source.id, &[mark(0, 31., 111., 8., 8.)])
        .is_err());
    assert_eq!(rt.documents.len(), 1);
    assert_eq!(&*rt.document(&source.id).unwrap().bytes().unwrap(), bytes);
}

#[test]
fn redaction_area_images_predicted_sources_preserve_samples_and_three_saved_edits() {
    let fixtures: [(u8, &[u8]); 7] = [
        (
            2,
            include_bytes!("../tests/fixtures/redaction-predictors/predictor-2.pdf"),
        ),
        (
            10,
            include_bytes!("../tests/fixtures/redaction-predictors/predictor-10.pdf"),
        ),
        (
            11,
            include_bytes!("../tests/fixtures/redaction-predictors/predictor-11.pdf"),
        ),
        (
            12,
            include_bytes!("../tests/fixtures/redaction-predictors/predictor-12.pdf"),
        ),
        (
            13,
            include_bytes!("../tests/fixtures/redaction-predictors/predictor-13.pdf"),
        ),
        (
            14,
            include_bytes!("../tests/fixtures/redaction-predictors/predictor-14.pdf"),
        ),
        (
            15,
            include_bytes!("../tests/fixtures/redaction-predictors/predictor-15.pdf"),
        ),
    ];
    let mut rt = runtime();
    let mut wanted = pixels();
    wanted[27..30].fill(0);
    let gray: Vec<u8> = (0..64).map(|n| n * 3).collect();
    let mut gray_wanted = gray.clone();
    gray_wanted[9] = 0;
    for (predictor, source) in fixtures {
        let before = rt.documents.len();
        let original = rt
            .register_generated_document(source.to_vec(), "Predicted source.pdf")
            .unwrap();
        let first = rt
            .redact_content_areas(
                &original.id,
                &[
                    mark(0, 31., 111., 8., 8.),
                    mark(0, 56.1, 25., 42.9, 20.),
                    mark(0, 60., 82., 10., 6.),
                ],
            )
            .unwrap();
        assert_eq!(
            samples(&first.output.bytes, 0),
            vec![wanted.clone(), pixels()]
        );
        assert_eq!(samples(&first.output.bytes, 1), vec![gray.clone()]);
        assert_eq!(
            (
                first.output.removed_characters,
                first.output.paths.len(),
                first.output.images.len()
            ),
            (6, 1, 1)
        );
        assert_eq!(&*rt.document(&original.id).unwrap().bytes().unwrap(), source);
        assert_eq!(rt.documents.len(), before + 1);
        let saved = rt
            .register_generated_document(first.output.bytes.clone(), "First saved.pdf")
            .unwrap();
        let second = rt
            .redact_content_areas(&saved.id, &[mark(0, 151., 111., 8., 8.)])
            .unwrap();
        assert_eq!(
            samples(&second.output.bytes, 0),
            vec![wanted.clone(), wanted.clone()]
        );
        let saved_again = rt
            .register_generated_document(second.output.bytes.clone(), "Second saved.pdf")
            .unwrap();
        let third = rt
            .redact_content_areas(&saved_again.id, &[mark(1, 31., 111., 8., 8.)])
            .unwrap();
        assert_eq!(
            samples(&third.output.bytes, 0),
            vec![wanted.clone(), wanted.clone()]
        );
        assert_eq!(samples(&third.output.bytes, 1), vec![gray_wanted.clone()]);
        assert_eq!(third.output.images.len(), 1);
        assert_eq!(
            (
                third.output.images[0].page_index,
                third.output.images[0].pixels
            ),
            (1, 1)
        );
        if let Some(root) = std::env::var_os("FOLIO_AREA_IMAGE_OUTPUT") {
            use std::io::Write;
            let folder = PathBuf::from(root).join(format!("predictor-{predictor}"));
            std::fs::create_dir(&folder).unwrap();
            for (name, bytes) in [
                ("image-source.pdf", source),
                ("image-actual.pdf", &first.output.bytes),
                ("image-second.pdf", &second.output.bytes),
                ("image-gray.pdf", &third.output.bytes),
            ] {
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(folder.join(name))
                    .unwrap()
                    .write_all(bytes)
                    .unwrap();
            }
        }
    }
}
