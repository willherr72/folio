use super::{attachment, ImageImportCache, ImportLimits, IMPORT_LIMITS};
use crate::{prepare_pdf_image, ImageAsset};
use image::{DynamicImage, ImageEncoder, ImageFormat, Rgba, RgbaImage};
use lopdf::{dictionary, Document, Object, ObjectId, Stream};
use std::{io::Cursor, sync::Arc};

fn png_asset(value: u8) -> ImageAsset {
    let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(16, 8, Rgba([value, 20, 30, 128])));
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, ImageFormat::Png).unwrap();
    ImageAsset::parse(bytes.into_inner()).unwrap()
}
fn jpeg_asset() -> ImageAsset {
    ImageAsset::parse(
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/image-assets/cmyk-odd.jpg"
        ))
        .unwrap(),
    )
    .unwrap()
}
fn fixture(asset: &ImageAsset) -> (Document, ObjectId, ObjectId) {
    let mut document = Document::with_version("1.7");
    let file = document.add_object(attachment(asset));
    let form = prepare_pdf_image(asset).unwrap().install(&mut document);
    let catalog = document.add_object(dictionary! {"Type"=>"Catalog", "Asset"=>file, "Form"=>form});
    document.trailer.set("Root", catalog);
    (document, file, form)
}
fn pixel_id(document: &Document, form: ObjectId) -> ObjectId {
    document
        .get_object(form)
        .unwrap()
        .as_stream()
        .unwrap()
        .dict
        .get(b"Resources")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"XObject")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Im0")
        .unwrap()
        .as_reference()
        .unwrap()
}
fn verify(document: &Document, file: ObjectId, form: ObjectId, asset: &ImageAsset) -> bool {
    ImageImportCache::new(document)
        .verify(&file.into(), &asset.info().id, &form.into())
        .is_some()
}

#[test]
fn image_attachment_roundtrip_preserves_original_and_prepared_resource_identity() {
    for asset in [png_asset(200), jpeg_asset()] {
        let (mut doc, file, form) = fixture(&asset);
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        let loaded = Document::load_mem(&bytes).unwrap();
        let mut cache = ImageImportCache::new(&loaded);
        let first = cache
            .verify(&file.into(), &asset.info().id, &form.into())
            .unwrap();
        let second = cache
            .verify(&file.into(), &asset.info().id, &form.into())
            .unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(first.asset.bytes(), asset.bytes());
        assert_eq!(
            first.prepared.form().content,
            prepare_pdf_image(&asset).unwrap().form().content
        );
        assert_eq!(cache.assets.len(), 1);
        assert_eq!(cache.streams.len(), 1);
    }
}

#[test]
fn image_attachment_rejects_changed_identity_bytes_version_mime_and_lengths() {
    let asset = png_asset(200);
    for alteration in 0..7 {
        let (mut doc, file, form) = fixture(&asset);
        let stream = doc.get_object_mut(file).unwrap().as_stream_mut().unwrap();
        match alteration {
            0 => stream.content[20] ^= 1,
            1 => stream
                .dict
                .set("FolioImageId", Object::string_literal("0".repeat(64))),
            2 => stream.dict.set("FolioVersion", 2),
            3 => stream.dict.set("Subtype", "image/jpeg"),
            4 => stream
                .dict
                .get_mut(b"Params")
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("Size", 1),
            5 => stream
                .dict
                .get_mut(b"Params")
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("Size", 64 * 1024 * 1024 + 1),
            _ => stream.dict.set("Extra", true),
        }
        assert!(!verify(&doc, file, form, &asset), "alteration {alteration}");
    }
}

#[test]
fn image_proof_binds_pixels_mask_depth_color_and_resource_shape() {
    let asset = png_asset(200);
    for alteration in 0..8 {
        let (mut doc, file, form) = fixture(&asset);
        let pixel = pixel_id(&doc, form);
        let mask = doc
            .get_object(pixel)
            .unwrap()
            .as_stream()
            .unwrap()
            .dict
            .get(b"SMask")
            .unwrap()
            .as_reference()
            .unwrap();
        match alteration {
            0 => {
                doc.get_object_mut(pixel)
                    .unwrap()
                    .as_stream_mut()
                    .unwrap()
                    .content[0] ^= 1
            }
            1 => {
                doc.get_object_mut(mask)
                    .unwrap()
                    .as_stream_mut()
                    .unwrap()
                    .content[0] ^= 1
            }
            2 => doc
                .get_object_mut(mask)
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .dict
                .set("BitsPerComponent", 16),
            3 => doc
                .get_object_mut(pixel)
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .dict
                .set("ColorSpace", "DeviceCMYK"),
            4 => doc
                .get_object_mut(form)
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .dict
                .set(
                    "Matrix",
                    vec![1.into(), 0.into(), 0.into(), 1.into(), 1.into(), 0.into()],
                ),
            5 => doc
                .get_object_mut(form)
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .dict
                .get_mut(b"Resources")
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("Hidden", true),
            6 => doc
                .objects
                .insert(mask, Object::Reference(mask))
                .map(|_| ())
                .unwrap(),
            _ => doc
                .get_object_mut(pixel)
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .dict
                .set("DecodeParms", dictionary! {"Predictor"=>12}),
        }
        assert!(!verify(&doc, file, form, &asset), "alteration {alteration}");
    }
}

#[test]
fn jpeg_proof_binds_dct_filter_decode_transform_and_polarity() {
    let asset = jpeg_asset();
    for alteration in 0..4 {
        let (mut doc, file, form) = fixture(&asset);
        let pixel = pixel_id(&doc, form);
        let stream = doc.get_object_mut(pixel).unwrap().as_stream_mut().unwrap();
        match alteration {
            0 => stream.dict.set("Filter", "FlateDecode"),
            1 => stream
                .dict
                .get_mut(b"DecodeParms")
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("ColorTransform", 1),
            2 => stream.dict.remove(b"Decode").map(|_| ()).unwrap(),
            _ => stream.content[20] ^= 1,
        }
        assert!(!verify(&doc, file, form, &asset), "alteration {alteration}");
    }
}

#[test]
fn repeated_asset_streams_share_preparation_but_each_form_is_verified() {
    let asset = png_asset(200);
    let (mut doc, file, form) = fixture(&asset);
    let another_file = doc.add_object(attachment(&asset));
    let bad_form = doc.add_object(Stream::new(dictionary! {}, vec![]));
    let mut cache = ImageImportCache::new(&doc);
    let first = cache
        .verify(&file.into(), &asset.info().id, &form.into())
        .unwrap();
    let second = cache
        .verify(&another_file.into(), &asset.info().id, &form.into())
        .unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(cache.assets.len(), 1);
    assert_eq!(cache.streams.len(), 2);
    assert!(cache
        .verify(&file.into(), &asset.info().id, &bad_form.into())
        .is_none());
}

#[test]
fn image_cache_admission_failure_preserves_existing_entries() {
    let first = png_asset(200);
    let second = png_asset(201);
    let (mut doc, file, form) = fixture(&first);
    let second_file = doc.add_object(attachment(&second));
    let second_form = prepare_pdf_image(&second).unwrap().install(&mut doc);
    for limits in [
        ImportLimits {
            assets: 1,
            ..IMPORT_LIMITS
        },
        ImportLimits {
            encoded: first.bytes().len(),
            ..IMPORT_LIMITS
        },
        ImportLimits {
            prepared: 1,
            ..IMPORT_LIMITS
        },
        ImportLimits {
            streams: 1,
            ..IMPORT_LIMITS
        },
    ] {
        let mut cache = ImageImportCache::with_limits(&doc, limits);
        let original = cache.verify(&file.into(), &first.info().id, &form.into());
        if limits.prepared == 1 {
            assert!(original.is_none());
            assert!(cache.assets.is_empty());
            assert!(cache.streams.is_empty());
        } else {
            let original = original.unwrap();
            assert!(cache
                .verify(&second_file.into(), &second.info().id, &second_form.into())
                .is_none());
            let retained = cache
                .verify(&file.into(), &first.info().id, &form.into())
                .unwrap();
            assert!(Arc::ptr_eq(&original, &retained));
            assert_eq!(cache.assets.len(), 1);
            assert_eq!(cache.streams.len(), 1);
        }
    }
}

#[test]
fn image_proof_binds_embedded_icc_profile_bytes() {
    let mut bytes = Vec::new();
    let mut encoder = image::codecs::png::PngEncoder::new(&mut bytes);
    encoder
        .set_icc_profile(moxcms::ColorProfile::new_display_p3().encode().unwrap())
        .unwrap();
    encoder
        .write_image(&[200, 40, 60], 1, 1, image::ExtendedColorType::Rgb8)
        .unwrap();
    let asset = ImageAsset::parse(bytes).unwrap();
    let (mut doc, file, form) = fixture(&asset);
    assert!(verify(&doc, file, form, &asset));
    let pixel = pixel_id(&doc, form);
    let profile = doc
        .get_object(pixel)
        .unwrap()
        .as_stream()
        .unwrap()
        .dict
        .get(b"ColorSpace")
        .unwrap()
        .as_array()
        .unwrap()[1]
        .as_reference()
        .unwrap();
    doc.get_object_mut(profile)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .content[0] ^= 1;
    assert!(!verify(&doc, file, form, &asset));
}

#[test]
fn attachment_transport_is_lossless_bounded_and_cannot_supply_predictors() {
    let mut bytes = png_asset(200).bytes().to_vec();
    let payload = [b"Folio\0".as_slice(), vec![b'a'; 4096].as_slice()].concat();
    let mut chunk = (payload.len() as u32).to_be_bytes().to_vec();
    chunk.extend_from_slice(b"tEXt");
    chunk.extend(payload);
    chunk.extend(crc32fast::hash(&chunk[4..]).to_be_bytes());
    bytes.splice(33..33, chunk);
    let asset = ImageAsset::parse(bytes).unwrap();
    let (mut doc, file, form) = fixture(&asset);
    // A compressible ancillary chunk makes the entire original PNG compressible.
    // Stream compression is optional; a valid unfiltered original must work too.
    doc.get_object_mut(file)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .compress()
        .unwrap();
    assert!(doc
        .get_object(file)
        .unwrap()
        .as_stream()
        .unwrap()
        .dict
        .has(b"Filter"));
    assert!(verify(&doc, file, form, &asset));
    doc.get_object_mut(file)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .dict
        .set("DecodeParms", dictionary! {"Predictor"=>12});
    assert!(!verify(&doc, file, form, &asset));
    let mut stream = attachment(&asset);
    stream.content = vec![0; 2 * 1024 * 1024];
    stream.compress().unwrap();
    assert_eq!(
        stream.dict.get(b"Filter").unwrap().as_name().unwrap(),
        b"FlateDecode"
    );
    // The declared original length remains tiny; expansion must stop at that bound.
    doc.objects.insert(file, Object::Stream(stream));
    let mut cache = ImageImportCache::new(&doc);
    assert!(cache
        .verify(&file.into(), &asset.info().id, &form.into())
        .is_none());
    assert!(cache.assets.is_empty() && cache.streams.is_empty());
}

#[test]
fn attachment_aliases_and_drawing_reference_cycles_fail_without_publication() {
    let asset = png_asset(200);
    let (mut doc, file, form) = fixture(&asset);
    doc.objects.insert(file, Object::Reference(file));
    let mut cache = ImageImportCache::new(&doc);
    assert!(cache
        .verify(&file.into(), &asset.info().id, &form.into())
        .is_none());
    assert!(cache.assets.is_empty());
}

#[test]
fn image_print_preparation_retains_native_pixels_without_registry_ownership() {
    use crate::{ExportRequest, ImageOverlay, Overlay, PagePlan, PdfEngine};
    let engine = PdfEngine::start(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let source = engine.create_blank_document(200., 300.).unwrap();
    let acquisition = engine
        .images()
        .register(png_asset(210).bytes().to_vec())
        .unwrap();
    let page = PagePlan {
        crop: None,
        id: "page".into(),
        source_id: source.id.clone(),
        page_index: 0,
        width: 200.,
        height: 300.,
        rotation: 0,
        overlays: vec![Overlay::Image(ImageOverlay {
            id: "image".into(),
            asset_id: acquisition.image.id.clone(),
            x: 40.,
            y: 50.,
            width: 80.,
            height: 40.,
            rotation: 0,
        })],
    };
    let preview = engine.render_page_plan(page.clone(), 200).unwrap();
    let printed = engine
        .prepare_print(ExportRequest {
            pages: vec![page],
            flatten: false,
        })
        .unwrap();
    assert!(printed.pages[0].overlays.is_empty());
    engine.images().release(&acquisition.token).unwrap();
    assert!(engine.images().get(&acquisition.image.id).is_err());
    assert_eq!(
        image::load_from_memory(&preview).unwrap().into_rgb8(),
        image::load_from_memory(&engine.render_page(&printed.id, 0, 200).unwrap())
            .unwrap()
            .into_rgb8()
    );
    engine.close_document(&printed.id).unwrap();
    engine.close_document(&source.id).unwrap();
}
