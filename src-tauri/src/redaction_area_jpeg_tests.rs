use super::*;
use crate::redaction_images::{ImageContext, Registry};
use lopdf::{content::Content, dictionary, Document, Object, Stream};
const JPEG: &[u8] = include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub2-progressive1.jpg");
fn image(pdf: &mut Document) -> lopdf::ObjectId {
    pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>64,"Height"=>48,"BitsPerComponent"=>8,"ColorSpace"=>"DeviceRGB","Filter"=>"DCTDecode"},JPEG.to_vec()))
}
fn fixture() -> Vec<u8> {
    let mut pdf = Document::with_version("1.7");
    let tree = pdf.new_object_id();
    let image = image(&mut pdf);
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier","Encoding"=>"WinAnsiEncoding"});
    let mut kids = vec![];
    for program in [
        b"q 128 0 0 96 20 24 cm /A Do Q q 128 0 0 96 200 24 cm /A Do Q BT /F1 12 Tf 1 0 0 1 20 160 Tm (Left SECRET Right) Tj ET q 0.9 0.2 0.1 rg 50 130 80 10 re f Q".as_slice(),
        b"q 128 0 0 96 20 24 cm /A Do Q BT /F1 12 Tf 1 0 0 1 20 160 Tm (Unchanged second page) Tj ET".as_slice(),
    ] {
        let stream=pdf.add_object(Stream::new(dictionary!{},program.to_vec()));
        let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),400.into(),200.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font},"XObject"=>dictionary!{"A"=>image}},"Contents"=>stream});kids.push(page.into());
    }
    pdf.objects.insert(
        tree,
        dictionary! {"Type"=>"Pages","Kids"=>kids,"Count"=>2}.into(),
    );
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree});
    pdf.trailer.set("Root", root);
    let mut out = vec![];
    pdf.save_to(&mut out).unwrap();
    out
}
fn mark(x: f64, y: f64, width: f64, height: f64) -> TextRedactionMark {
    TextRedactionMark {
        page_index: 0,
        rect: TextMarkRect {
            x,
            y,
            width,
            height,
        },
    }
}
fn save(group: &str, name: &str, bytes: &[u8]) {
    if let Some(root) = std::env::var_os("FOLIO_JPEG_TRANSACTION_OUTPUT") {
        let root = PathBuf::from(root).join(group);
        std::fs::create_dir_all(&root).unwrap();
        use std::io::Write;
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(root.join(name))
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }
}
#[test]
fn redaction_area_jpeg_native_transaction_combines_text_paths_aliases_and_saved_reedit() {
    let mut rt = WorkerRuntime::new(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
        Arc::new(FontRegistry::new()),
    )
    .unwrap();
    let original = fixture();
    let info = rt
        .register_generated_document(original.clone(), "JPEG source.pdf")
        .unwrap();
    let marks = [
        mark(55.25, 130.75, 1., 1.),
        mark(56.1, 25., 42.9, 20.),
        mark(60., 62., 10., 6.),
    ];
    assert!(
        rt.redact_content_areas(&info.id, &marks).is_err(),
        "ordinary JPEG admission must remain gated"
    );
    let first = rt
        .redact_content_areas_with_context(
            &info.id,
            &marks,
            &mut ImageContext::for_jpeg_validation(),
        )
        .unwrap();
    assert_eq!(first.output.removed_characters, 6);
    assert_eq!(first.output.paths.len(), 1);
    assert_eq!(first.output.images.len(), 1);
    assert_eq!(first.output.images[0].pixels, 256);
    assert!(first.output.images[0].jpeg.is_some());
    for scale in [1u32, 3] {
        let render = |bytes: &[u8]| {
            let doc = rt.pdfium.load_pdf_from_byte_slice(bytes, None).unwrap();
            let page = doc.pages().get(0).unwrap();
            let raster = page
                .render_with_config(&PdfRenderConfig::new().set_target_width((400 * scale) as i32))
                .unwrap()
                .as_image()
                .unwrap()
                .to_rgba8();
            image::imageops::crop_imm(&raster, 105 * scale, 25 * scale, 40 * scale, 20 * scale)
                .to_image()
                .into_raw()
        };
        assert_eq!(
            render(&original),
            render(&first.output.bytes),
            "retained Right glyphs changed at {scale}x"
        );
    }
    assert_eq!(&*rt.document(&info.id).unwrap().bytes().unwrap(), original);
    assert_eq!(rt.documents.len(), 1);
    let loaded = rt
        .register_generated_document(first.output.bytes.clone(), "Saved JPEG.pdf")
        .unwrap();
    let second = rt
        .redact_content_areas_with_context(
            &loaded.id,
            &[mark(87.25, 130.75, 1., 1.)],
            &mut ImageContext::for_jpeg_validation(),
        )
        .unwrap();
    assert_eq!(second.output.removed_characters, 0);
    assert!(second.output.paths.is_empty());
    assert_eq!(second.output.images.len(), 1);
    let parsed = Document::load_mem(&second.output.bytes).unwrap();
    let mut context = ImageContext::for_jpeg_validation();
    let prepared = crate::redaction_core::prepare_area_with_context(&parsed, &mut context).unwrap();
    assert_eq!(
        prepared.pages[0].images[0].jpeg_coverage().unwrap().mask(),
        &[0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0]
    );
    assert!(prepared.pages[0].images[1].jpeg_coverage().is_none());
    assert!(Arc::ptr_eq(
        &prepared.pages[0].images[1],
        &prepared.pages[1].images[0]
    ));
    assert_eq!(prepared.pages[0].program.blocks[0].text, "Left  Right");
    assert_eq!(
        prepared.pages[1].program.blocks[0].text,
        "Unchanged second page"
    );
    // Logical preparation/republish restores exactly one verified fill per use.
    assert_eq!(
        crate::redaction_core::publish(prepared.pages).unwrap(),
        second.output.bytes
    );
    save("mixed", "source.pdf", &original);
    save("mixed", "first.pdf", &first.output.bytes);
    save("mixed", "second.pdf", &second.output.bytes);
}
#[test]
fn redaction_area_jpeg_private_parser_editor_publisher_exports_transformed_saved_transactions() {
    for (name, matrix) in [
        ("axis", [128., 0., 0., 96., 20., 24.]),
        ("rotate", [0., 128., -96., 0., 116., 16.]),
        ("reflect", [-128., 0., 0., 96., 156., 24.]),
        ("shear", [100., 10., 25., 90., 20., 20.]),
        ("fractional", [123.5, 0., 0., 95.25, 21.125, 22.5]),
        ("small", [8., 0., 0., 6., 40., 40.]),
    ] {
        let mut pdf = Document::new();
        let value = Object::Reference(image(&mut pdf));
        let source = Registry::default().load(&pdf, &value).unwrap();
        let data = format!(
            "q {} {} {} {} {} {} cm /I0 Do Q",
            matrix[0], matrix[1], matrix[2], matrix[3], matrix[4], matrix[5]
        );
        let page = crate::redaction_core::Page {
            width: 192.into(),
            height: 160.into(),
            fonts: vec![],
            graphics: vec![],
            images: vec![source],
            program: crate::redaction_content::Program {
                operations: Content::decode_strict(data.as_bytes()).unwrap().operations,
                blocks: vec![],
            },
        };
        let mut bytes = crate::redaction_core::publish(vec![page]).unwrap();
        save("ownership", &format!("{name}-source.pdf"), &bytes);
        let transform = crate::redaction_area_paths::ImageTransform::default()
            .concat(matrix)
            .unwrap();
        for (label, x) in [("prior", 0.375), ("actual", 0.625)] {
            let rect = transform
                .unit_bounds(AreaRect {
                    left: x,
                    bottom: 0.5,
                    right: x + 0.001,
                    top: 0.501,
                })
                .unwrap();
            let parsed = Document::load_mem(&bytes).unwrap();
            let result = remove_planned_content_with_context(
                &parsed,
                &[],
                &[PageAreaMark {
                    page_index: 0,
                    rect,
                }],
                &mut ImageContext::for_jpeg_validation(),
            )
            .unwrap();
            assert_eq!(result.images.len(), 1);
            assert_eq!(result.images[0].pixels, 256);
            assert!(
                result.paths.is_empty(),
                "owned fill must not be treated as an unrelated path"
            );
            bytes = result.bytes;
            save("ownership", &format!("{name}-{label}.pdf"), &bytes);
        }
    }
}
