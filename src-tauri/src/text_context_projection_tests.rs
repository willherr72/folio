use super::*;
use lopdf::{content::Content, dictionary, Document, Object, Stream};

#[test]
fn text_page_parse_reuse_uses_one_source_parse_per_preparation() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut runtime = WorkerRuntime::new(
        &root.join("resources/pdfium/pdfium.dll"), Arc::new(FontRegistry::new()),
    ).unwrap();
    let bytes = source(dictionary! {"S"=>"Transparency","CS"=>"DeviceRGB","I"=>true});
    let info = runtime.register_generated_document(bytes, "Parse reuse.pdf").unwrap();
    for complete_source in [true, false] {
        for _ in 0..2 {
            let original = runtime.editing_document(&info.id).unwrap();
            let (copy, count) = crate::acroforms::count_source_loads(original.bytes().unwrap().as_ref(), || {
                runtime.text_transaction_page_copy(&info.id, 0, complete_source)
            });
            let copy = copy.unwrap();
            assert_eq!(copy.pages().get(0).unwrap().text().unwrap().all(), "Old content");
            assert_eq!(count, 1, "one source parse per preparation, including repeated requests");
        }
    }
}

pub(super) fn source(group: lopdf::Dictionary) -> Vec<u8> {
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let content = pdf.add_object(Stream::new(
        dictionary! {},
        b"BT /F1 18 Tf 30 200 Td (Old content) Tj ET".to_vec(),
    ));
    let font =
        pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let page=pdf.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content,"Group"=>group});
    pdf.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into(),
    );
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    pdf.trailer.set("Root", root);
    crate::pdf_output::save(&mut pdf).unwrap()
}

#[test]
fn text_context_projection_refuses_unproved_group_before_editing() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut runtime = WorkerRuntime::new(
        &root.join("resources/pdfium/pdfium.dll"),
        Arc::new(FontRegistry::new()),
    )
    .unwrap();
    for complete_source in [false, true] {
        let info = runtime
            .register_generated_document(
                source(dictionary! {"S"=>"Transparency","CS"=>"DeviceRGB","I"=>1}),
                "Group.pdf",
            )
            .unwrap();
        assert!(runtime
            .text_transaction_page_copy(&info.id, 0, complete_source)
            .is_err());
    }
}

#[test]
fn text_context_projection_keeps_supported_group_semantics() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut runtime = WorkerRuntime::new(
        &root.join("resources/pdfium/pdfium.dll"),
        Arc::new(FontRegistry::new()),
    )
    .unwrap();
    let bytes = source(
        dictionary! {"Type"=>"Group","S"=>"Transparency","CS"=>"DeviceRGB","I"=>true,"K"=>false},
    );
    let original = Document::load_mem(&bytes).unwrap();
    let expected =
        text_context_group::descriptor(&original, *original.get_pages().values().next().unwrap())
            .unwrap();
    let info = runtime
        .register_generated_document(bytes, "Group.pdf")
        .unwrap();
    for complete_source in [false, true] {
        let copy = runtime
            .text_transaction_page_copy(&info.id, 0, complete_source)
            .unwrap()
            .save_to_bytes()
            .unwrap();
        let copy = Document::load_mem(&copy).unwrap();
        assert_eq!(
            text_context_group::descriptor(&copy, *copy.get_pages().values().next().unwrap())
                .unwrap(),
            expected
        );
    }
}

#[test]
#[ignore = "requires the explicit locally generated Word producer corpus"]
fn original_word_context_metadata() {
    let root =
        std::env::var_os("FOLIO_WORD_CONTEXT_INPUTS").expect("explicit producer input directory");
    for name in ["word-False.pdf", "word-True.pdf"] {
        let pdf = Document::load(PathBuf::from(&root).join(name)).unwrap();
        let page = *pdf.get_pages().values().next().unwrap();
        let content = Content::decode_strict(
            &pdf.get_page_content_with_limit(page, 16 * 1024 * 1024)
                .unwrap(),
        )
        .unwrap();
        let context = text_context::inspect(&content.operations).unwrap();
        assert!(!context.shows.is_empty());
        text_context_tags::prove(&pdf, page, &[context.shows[0].1.clone()]).unwrap();
        assert!(context.shows[0].1.contains([90., 700., 270., 720.]));
        assert!(text_context_group::descriptor(&pdf, page)
            .unwrap()
            .is_some());
    }
}

fn parse_reuse_fixture(form: bool, repair: bool) -> Vec<u8> {
    let mut pdf = Document::load_mem(&source(
        dictionary! {"S"=>"Transparency","CS"=>"DeviceRGB","I"=>true},
    )).unwrap();
    let page = *pdf.get_pages().values().next().unwrap();
    if repair {
        let draw = pdf.add_object(Stream::new(
            dictionary! {"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary!{}},
            b"0.25 0.5 0.75 rg 20 20 25 25 re f".to_vec(),
        ));
        let resources = pdf.get_dictionary_mut(page).unwrap().get_mut(b"Resources").unwrap().as_dict_mut().unwrap();
        resources.set("XObject", dictionary! {"First"=>draw});
        let content = pdf.add_object(Stream::new(dictionary!{}, b"BT /F1 18 Tf 30 200 Td (Old content) Tj ET /First Do".to_vec()));
        pdf.get_dictionary_mut(page).unwrap().set("Contents", content);
    }
    if form {
        let font = pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
        let resources = dictionary! {"Font"=>dictionary!{"F1"=>font}};
        let appearance = pdf.add_object(Stream::new(
            dictionary! {"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),120.into(),20.into()],"Resources"=>resources.clone()},
            b"BT /F1 10 Tf 1 5 Td (Answer) Tj ET".to_vec(),
        ));
        let widget = pdf.add_object(dictionary! {"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>Object::string_literal("answer"),"V"=>Object::string_literal("Answer"),"P"=>page,"Rect"=>vec![30.into(),90.into(),150.into(),110.into()],"F"=>4,"AP"=>dictionary!{"N"=>appearance}});
        pdf.get_dictionary_mut(page).unwrap().set("Annots", vec![Object::Reference(widget)]);
        let form = pdf.add_object(dictionary! {"Fields"=>vec![Object::Reference(widget)],"DA"=>Object::string_literal("/F1 10 Tf 0 g"),"DR"=>resources});
        let root = pdf.trailer.get(b"Root").unwrap().as_reference().unwrap();
        pdf.get_dictionary_mut(root).unwrap().set("AcroForm", form);
    }
    crate::pdf_output::save(&mut pdf).unwrap()
}

#[test]
fn text_page_parse_reuse_preserves_forms_and_repaired_resources() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut runtime = WorkerRuntime::new(
        &root.join("resources/pdfium/pdfium.dll"), Arc::new(FontRegistry::new()),
    ).unwrap();
    for (form, repair) in [(false,true), (true,false), (true,true)] {
        let bytes = parse_reuse_fixture(form, repair);
        let info = runtime.register_generated_document(bytes.clone(), "Form resource reuse.pdf").unwrap();
        for complete_source in [true, false] {
            let original = runtime.editing_document(&info.id).unwrap();
            let (copy, count) = crate::acroforms::count_source_loads(original.bytes().unwrap().as_ref(), || {
                runtime.text_transaction_page_copy(&info.id, 0, complete_source)
            });
            let copy = copy.unwrap();
            assert_eq!(count, 1, "form={form}, repair={repair}, complete={complete_source}");
            let original_page = original.pdf().unwrap().pages().get(0).unwrap();
            let copied_page = copy.pages().get(0).unwrap();
            assert_eq!(original_page.text().unwrap().all(), copied_page.text().unwrap().all());
            assert_eq!(glyph_snapshot(&original_page).unwrap(), glyph_snapshot(&copied_page).unwrap());
            assert_eq!(raster(&original_page).unwrap(), raster(&copied_page).unwrap());
            assert_eq!(original.bytes().unwrap().as_ref(), bytes.as_slice());
            if form {
                let output = Document::load_mem(&copy.save_to_bytes().unwrap()).unwrap();
                let acro = output.catalog().unwrap().get(b"AcroForm").unwrap();
                let acro = output.dereference(acro).unwrap().1.as_dict().unwrap();
                let fields = acro.get(b"Fields").unwrap().as_array().unwrap();
                assert_eq!(fields.len(), 1);
                let field = output.dereference(&fields[0]).unwrap().1.as_dict().unwrap();
                assert_eq!(field.get(b"V").unwrap().as_str().unwrap(), b"Answer");
                let page = *output.get_pages().values().next().unwrap();
                let annotations = output.get_dictionary(page).unwrap().get(b"Annots").unwrap();
                let annotations = output.dereference(annotations).unwrap().1.as_array().unwrap();
                assert_eq!(annotations.len(), 1);
                let widget = output.dereference(&annotations[0]).unwrap().1.as_dict().unwrap();
                assert_eq!(widget.get(b"Parent").unwrap(), &fields[0]);
                let ap = output.dereference(widget.get(b"AP").unwrap()).unwrap().1.as_dict().unwrap().get(b"N").unwrap();
                assert_eq!(output.dereference(ap).unwrap().1.as_stream().unwrap().get_plain_content_with_limit(1024).unwrap(), b"BT /F1 10 Tf 1 5 Td (Answer) Tj ET");
            }
        }
    }
}
