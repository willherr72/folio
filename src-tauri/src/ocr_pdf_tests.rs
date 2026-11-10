use super::*;
use crate::{ExportRequest, OcrPage, PdfEngine};
use lopdf::{dictionary, Document, Object, Stream};
use std::path::PathBuf;

fn source() -> (Document, lopdf::ObjectId, lopdf::ObjectId) {
    let mut d = Document::with_version("1.7");
    let pages = d.new_object_id();
    let font =
        d.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let image=d.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>20,"Height"=>10,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8},
        (0..200).flat_map(|n|[n as u8,80,190]).collect()));
    let content = d.add_object(Stream::new(
        dictionary! {},
        b"q 180 0 0 60 10 10 cm /Im0 Do Q BT /FH 10 Tf 1 0 0 1 10 88 Tm (Native header) Tj ET\n"
            .to_vec(),
    ));
    let resources = d.add_object(
        dictionary! {"Font"=>dictionary!{"FH"=>font},"XObject"=>dictionary!{"Im0"=>image}},
    );
    let page=d.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),200.into(),100.into()],"Contents"=>content});
    let other = d.add_object(d.get_object(page).unwrap().clone());
    d.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(page),Object::Reference(other)],"Count"=>2,"Resources"=>resources}.into());
    let catalog = d.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    d.trailer.set("Root", catalog);
    (d, page, other)
}
fn recognized(text: &str) -> OcrPage {
    serde_json::from_value(
        serde_json::json!({"width":100,"height":50,"blocks":[{"paragraphs":[{"lines":[{"words":[
            {"text":text,"confidence":93,"bounds":{"x":10,"y":15,"width":45,"height":10}}
        ]}]}]}]}),
    )
    .unwrap()
}
fn mapping() -> OcrMapping {
    OcrMapping {
        origin: [0., 100.],
        right: [2., 0.],
        down: [0., -2.],
    }
}
fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn write(d: &mut Document, name: &str, dir: &std::path::Path) -> PathBuf {
    let path = dir.join(name);
    d.save(&path).unwrap();
    path
}
fn text(e: &PdfEngine, id: &str) -> String {
    e.page_text(id, 0)
        .unwrap()
        .characters
        .iter()
        .map(|c| c.text.as_str())
        .collect()
}

#[test]
fn ocr_form_preserves_pixels_and_unicode_and_proves_its_record() {
    let (mut d, page, _) = source();
    let tmp = tempfile::tempdir().unwrap();
    let e = engine();
    let before = e
        .open_document(write(&mut d, "before.pdf", tmp.path()))
        .unwrap();
    let original = d.get_page_content(page);
    replace(&mut d, page, mapping(), &recognized("café 𐐀")).unwrap();
    let owned = read_owned(&mut d, page).unwrap().unwrap();
    assert_eq!(owned.record.page, recognized("café 𐐀"));
    let contents = d.get_page_content(page);
    assert!(contents.windows(original.len()).any(|w| w == original));
    let after = e
        .open_document(write(&mut d, "after.pdf", tmp.path()))
        .unwrap();
    assert_eq!(
        e.render_page(&before.id, 0, 800).unwrap(),
        e.render_page(&after.id, 0, 800).unwrap()
    );
    let extracted = text(&e, &after.id);
    assert!(extracted.contains("Native header"), "{extracted:?}");
    assert!(extracted.contains("café 𐐀"), "{extracted:?}");
    e.close_document(&before.id).unwrap();
    e.close_document(&after.id).unwrap();
}
#[test]
fn repeated_corrections_and_removal_do_not_accumulate_text_or_resources() {
    let (mut d, page, _) = source();
    replace(&mut d, page, mapping(), &recognized("first")).unwrap();
    let count = d.objects.len();
    for value in ["second", "third", "final café"] {
        replace(&mut d, page, mapping(), &recognized(value)).unwrap();
        assert_eq!(
            read_owned(&mut d, page).unwrap().unwrap().record.page,
            recognized(value)
        );
        assert!(
            d.objects.len() <= count + 2,
            "{} > {count}",
            d.objects.len()
        );
    }
    let tmp = tempfile::tempdir().unwrap();
    let e = engine();
    let info = e
        .open_document(write(&mut d, "corrected.pdf", tmp.path()))
        .unwrap();
    let result = text(&e, &info.id);
    assert!(result.contains("final café"));
    assert!(!result.contains("first"));
    e.close_document(&info.id).unwrap();
    let mut empty = recognized("unused");
    empty.blocks.clear();
    replace(&mut d, page, mapping(), &empty).unwrap();
    assert!(read_owned(&mut d, page).unwrap().is_none());
    let info = e
        .open_document(write(&mut d, "removed.pdf", tmp.path()))
        .unwrap();
    assert!(!text(&e, &info.id).contains("final café"));
    e.close_document(&info.id).unwrap();
}
#[test]
fn marker_cannot_authorize_removing_modified_visible_content() {
    let (mut d, page, _) = source();
    replace(&mut d, page, mapping(), &recognized("old")).unwrap();
    let form = read_owned(&mut d, page).unwrap().unwrap().form_id;
    d.get_object_mut(form)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .content
        .extend_from_slice(b"\n0 0 20 20 re f\n");
    assert!(read_owned(&mut d, page).unwrap().is_none());
    let content = d
        .get_object(form)
        .unwrap()
        .as_stream()
        .unwrap()
        .content
        .clone();
    replace(&mut d, page, mapping(), &recognized("new")).unwrap();
    assert_eq!(
        d.get_object(form).unwrap().as_stream().unwrap().content,
        content
    );
}
#[test]
fn tampered_font_mapping_is_not_an_owned_layer() {
    let (mut d, page, _) = source();
    replace(&mut d, page, mapping(), &recognized("old")).unwrap();
    let owned = read_owned(&mut d, page).unwrap().unwrap();
    let form = d.get_object(owned.form_id).unwrap().as_stream().unwrap();
    let resources = d
        .dereference(form.dict.get(b"Resources").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    let fonts = d
        .dereference(resources.get(b"Font").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    let font = d
        .dereference(fonts.get(b"F0").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    let unicode = font.get(b"ToUnicode").unwrap().as_reference().unwrap();
    d.get_object_mut(unicode)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .content
        .push(b' ');
    assert!(read_owned(&mut d, page).unwrap().is_none());
}
#[test]
fn inherited_resources_and_unselected_page_remain_unchanged() {
    let (mut d, page, other) = source();
    let before = d.get_dictionary(other).unwrap().clone();
    replace(&mut d, page, mapping(), &recognized("new")).unwrap();
    assert_eq!(d.get_dictionary(other).unwrap(), &before);
    assert!(read_owned(&mut d, other).unwrap().is_none());
}
#[test]
fn unbalanced_original_graphics_and_text_are_rejected() {
    for content in [b"q".to_vec(), b"Q".to_vec(), b"BT".to_vec(), b"ET".to_vec()] {
        let (mut d, page, _) = source();
        let id = d.add_object(Stream::new(dictionary! {}, content));
        d.get_dictionary_mut(page).unwrap().set("Contents", id);
        assert!(replace(&mut d, page, mapping(), &recognized("new")).is_err());
    }
}
#[test]
fn malformed_or_undecodable_original_contents_are_never_silently_dropped() {
    for case in 0..5 {
        let (mut d, page, _) = source();
        let contents = match case {
            0 => Object::Integer(7),
            1 => Object::Array(vec![
                d.get_dictionary(page)
                    .unwrap()
                    .get(b"Contents")
                    .unwrap()
                    .clone(),
                Object::Integer(3),
            ]),
            2 => Object::Reference((999_999, 0)),
            3 => {
                let id = d.new_object_id();
                d.objects.insert(id, Object::Reference(id));
                Object::Reference(id)
            }
            _ => Object::Reference(d.add_object(Stream::new(
                dictionary! {"Filter"=>"UnsupportedFilter"},
                b"q Q".to_vec(),
            ))),
        };
        d.get_dictionary_mut(page)
            .unwrap()
            .set("Contents", contents);
        assert!(
            replace(&mut d, page, mapping(), &recognized("new")).is_err(),
            "case {case}"
        );
    }
}
#[test]
fn owned_form_survives_real_export_and_rotation() {
    let (mut d, page, _) = source();
    replace(&mut d, page, mapping(), &recognized("café")).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let e = engine();
    let info = e
        .open_document(write(&mut d, "source.pdf", tmp.path()))
        .unwrap();
    let request:ExportRequest=serde_json::from_value(serde_json::json!({"pages":[{"id":"p","sourceId":info.id,"pageIndex":0,"width":200,"height":100,"rotation":90,"overlays":[]}]})).unwrap();
    let path = tmp.path().join("export.pdf");
    e.export_pdf(request, path.clone()).unwrap();
    e.close_document(&info.id).unwrap();
    let mut reopened = Document::load(path).unwrap();
    let page = *reopened.get_pages().values().next().unwrap();
    assert_eq!(
        read_owned(&mut reopened, page)
            .unwrap()
            .unwrap()
            .record
            .page,
        recognized("café")
    );
}

#[test]
fn repeated_or_transformed_owned_invocations_are_rejected() {
    for invocation in ["q /FolioOCR Do Q", "q 2 0 0 2 0 0 cm /FolioOCR Do Q"] {
        let (mut d, page, _) = source();
        replace(&mut d, page, mapping(), &recognized("old")).unwrap();
        if invocation.contains("cm") {
            let contents = d.get_page_contents(page);
            let last = *contents.last().unwrap();
            d.get_object_mut(last)
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .content = format!("Q\n{invocation}\n").into_bytes();
        } else {
            d.add_page_contents(page, invocation.as_bytes().to_vec())
                .unwrap();
        }
        assert!(read_owned(&mut d, page).is_err());
    }
}

#[test]
#[ignore = "writes independent-reader OCR fixtures when FOLIO_OCR_ARTIFACTS is explicitly set"]
fn independent_reader_artifacts() {
    let directory = PathBuf::from(std::env::var("FOLIO_OCR_ARTIFACTS").unwrap());
    assert!(directory.is_absolute());
    std::fs::create_dir_all(&directory).unwrap();
    let e = engine();
    let mut records = vec![];
    for intrinsic in [0u16, 90, 180, 270] {
        for extra in [0u16, 90, 180, 270] {
            let total = (intrinsic + extra) % 360;
            let (mut d, page, other) = source();
            for id in [page, other] {
                let dictionary = d.get_dictionary_mut(id).unwrap();
                dictionary.set("MediaBox", numbers(&[20., 30., 220., 130.]));
                dictionary.set("CropBox", numbers(&[20., 30., 220., 130.]));
                dictionary.set("Rotate", i64::from(intrinsic));
            }
            let original = d.get_page_content(page);
            let mut shifted = b"q 1 0 0 1 20 30 cm\n".to_vec();
            shifted.extend(original);
            shifted.extend(b"\nQ\n");
            let content = d.add_object(Stream::new(dictionary! {}, shifted));
            for id in [page, other] {
                d.get_dictionary_mut(id).unwrap().set("Contents", content);
            }
            let (map, width, height) = match total {
                0 => (
                    OcrMapping {
                        origin: [20., 130.],
                        right: [2., 0.],
                        down: [0., -2.],
                    },
                    100,
                    50,
                ),
                90 => (
                    OcrMapping {
                        origin: [20., 30.],
                        right: [0., 2.],
                        down: [2., 0.],
                    },
                    50,
                    100,
                ),
                180 => (
                    OcrMapping {
                        origin: [220., 30.],
                        right: [-2., 0.],
                        down: [0., 2.],
                    },
                    100,
                    50,
                ),
                _ => (
                    OcrMapping {
                        origin: [220., 130.],
                        right: [0., -2.],
                        down: [-2., 0.],
                    },
                    50,
                    100,
                ),
            };
            let ocr:OcrPage=serde_json::from_value(serde_json::json!({"width":width,"height":height,"blocks":[{"paragraphs":[{"lines":[
            {"words":[{"text":"café","confidence":94,"bounds":{"x":5,"y":15,"width":18,"height":8}},
                {"text":"niño","confidence":94,"bounds":{"x":27,"y":15,"width":17,"height":8}}]},
            {"words":[{"text":"𐐀 final","confidence":91,"bounds":{"x":5,"y":30,"width":38,"height":8}}]}
        ]}]}]})).unwrap();
            let before = e
                .open_document(write(
                    &mut d,
                    &format!("source-{intrinsic}-{extra}.pdf"),
                    &directory,
                ))
                .unwrap();
            replace(&mut d, page, map, &ocr).unwrap();
            let after = e
                .open_document(write(
                    &mut d,
                    &format!("layer-{intrinsic}-{extra}.pdf"),
                    &directory,
                ))
                .unwrap();
            let extracted = text(&e, &after.id).replace("Native header", "");
            assert_eq!(
                extracted.split_whitespace().collect::<Vec<_>>().join(" "),
                "café niño 𐐀 final",
                "workspace {intrinsic}/{extra}"
            );
            let export = |info: &crate::DocumentInfo, name: &str| {
                let request:ExportRequest=serde_json::from_value(serde_json::json!({"pages":[{"id":"p","sourceId":info.id,"pageIndex":0,
                "width":info.pages[0].width,"height":info.pages[0].height,"rotation":extra,"overlays":[]}]})).unwrap();
                let path = directory.join(name);
                e.export_pdf(request, path.clone()).unwrap();
                path
            };
            let source = export(&before, &format!("before-{intrinsic}-{extra}.pdf"));
            let output = export(&after, &format!("after-{intrinsic}-{extra}.pdf"));
            let reopened = e.open_document(&output).unwrap();
            let extracted = text(&e, &reopened.id).replace("Native header", "");
            assert_eq!(
                extracted.split_whitespace().collect::<Vec<_>>().join(" "),
                "café niño 𐐀 final",
                "export {intrinsic}/{extra}"
            );
            e.close_document(&reopened.id).unwrap();
            e.close_document(&before.id).unwrap();
            e.close_document(&after.id).unwrap();
            let mut saved = Document::load(&output).unwrap();
            let id = *saved.get_pages().values().next().unwrap();
            assert_eq!(
                read_owned(&mut saved, id).unwrap().unwrap().record.page,
                ocr
            );
            records.push(serde_json::json!({"intrinsic":intrinsic,"extra":extra,"before":source,"after":output,"ocr":ocr}));
        }
    }
    std::fs::write(
        directory.join("fixtures.json"),
        serde_json::to_vec_pretty(&records).unwrap(),
    )
    .unwrap();
}
