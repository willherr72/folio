use super::*;
use crate::redaction_ranges::{inspect_standard_glyphs, remove_standard_text_ranges, TextRange};
use lopdf::{dictionary, Document, Object, Stream};

#[test]
fn redaction_ranges_native_following_text_keeps_exact_pixels() {
    let library = std::env::var_os("FOLIO_TEST_PDFIUM_LIBRARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")
        });
    let runtime = WorkerRuntime::new(&library, Arc::new(FontRegistry::new())).unwrap();
    let mut d = Document::with_version("1.7");
    let tree = d.new_object_id();
    let font=d.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier","Encoding"=>"WinAnsiEncoding"});
    let content=d.add_object(Stream::new(dictionary!{},b"BT /F1 12 Tf 16 TL 1 0 0 1 20 240 Tm (`Left' Confidential-84912 Right) Tj ( Following.) Tj T* (Next line.) Tj ET".to_vec()));
    let page=d.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),500.into(),300.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
    d.objects.insert(
        tree,
        Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}),
    );
    let root = d.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree});
    d.trailer.set("Root", root);
    let block = inspect_standard_glyphs(&d).unwrap().remove(0);
    let start = block.source.text.find("Confidential-84912").unwrap();
    let output = remove_standard_text_ranges(
        &d,
        &[TextRange {
            selection_id: block.source.selection_id,
            start,
            end: start + 18,
        }],
    )
    .unwrap();
    let mut original = vec![];
    d.save_to(&mut original).unwrap();
    let render = |bytes: &[u8]| {
        let doc = runtime
            .pdfium
            .load_pdf_from_byte_slice(bytes, None)
            .unwrap();
        let page = doc.pages().get(0).unwrap();
        let image = page
            .render_with_config(&PdfRenderConfig::new().set_target_width(1500))
            .unwrap()
            .as_image()
            .unwrap()
            .to_rgba8();
        image::imageops::crop_imm(&image, 735, 135, 255, 60)
            .to_image()
            .into_raw()
    };
    let before = render(&original);
    let after = render(&output.bytes);
    assert_eq!(
        before.iter().zip(&after).filter(|(a, b)| a != b).count(),
        0,
        "following text raster changed outside selected source text"
    );
}
