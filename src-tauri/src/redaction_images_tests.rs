use super::*;
use lopdf::dictionary;

pub(super) fn jpeg_fixture(rgb: bool, params: Object) -> (Document, Object, Vec<u8>) {
    let data = if rgb {
        include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub2-progressive1.jpg").as_slice()
    } else {
        include_bytes!("../tests/fixtures/redaction-jpeg/gray-sub0-progressive1.jpg").as_slice()
    };
    let mut pdf = Document::new();
    let reference=Object::Reference(pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>64,"Height"=>48,
        "BitsPerComponent"=>8,"ColorSpace"=>if rgb{"DeviceRGB"}else{"DeviceGray"},"Filter"=>"DCTDecode","DecodeParms"=>params},data.to_vec())));
    (pdf, reference, data.to_vec())
}

#[test]
fn redaction_images_jpeg_canonical_owned_aliases_and_fresh_reload() {
    for rgb in [false, true] {
        let (mut pdf, r, source) = jpeg_fixture(rgb, Object::Null);
        let alias = Object::Reference(pdf.add_object(r.clone()));
        let mut registry = Registry::default();
        let image = registry.load(&pdf, &r).unwrap();
        assert!(Arc::ptr_eq(&image, &registry.load(&pdf, &alias).unwrap()));
        let stream = image.stream().unwrap();
        assert_eq!(
            stream.dict.get(b"Filter").unwrap().as_name().unwrap(),
            b"DCTDecode"
        );
        assert_eq!(
            pdf.get_object(r.as_reference().unwrap())
                .unwrap()
                .as_stream()
                .unwrap()
                .content,
            source
        );
        assert_eq!(registry.bytes, stream.content.len());
        if rgb {
            assert_eq!(
                stream
                    .dict
                    .get(b"DecodeParms")
                    .unwrap()
                    .as_dict()
                    .unwrap()
                    .get(b"ColorTransform")
                    .unwrap()
                    .as_i64()
                    .unwrap(),
                1
            );
        }
        let mut reopened = Document::new();
        let next = Object::Reference(reopened.add_object(stream.clone()));
        let loaded = Registry::default().load(&reopened, &next).unwrap();
        assert_eq!(loaded.digest, image.digest);
        assert_eq!(loaded.stream().unwrap().content, stream.content);
    }
}

#[test]
fn redaction_images_jpeg_transform_identity_and_exact_retained_budget() {
    let mut variants = vec![];
    for params in [
        Object::Null,
        dictionary! {}.into(),
        dictionary! {"ColorTransform"=>1}.into(),
        dictionary! {"ColorTransform"=>0}.into(),
    ] {
        let (pdf, r, _) = jpeg_fixture(true, params);
        let image = Registry::default().load(&pdf, &r).unwrap();
        variants.push(image);
    }
    assert_eq!(variants[0].digest, variants[1].digest);
    assert_eq!(variants[1].digest, variants[2].digest);
    assert_ne!(variants[2].digest, variants[3].digest);
    assert_eq!(
        variants[2].stream().unwrap().content,
        variants[3].stream().unwrap().content,
        "same coefficient bytes, distinct interpretation"
    );
    let length = variants[0].stream().unwrap().content.len();
    let (pdf, r, _) = jpeg_fixture(true, Object::Null);
    let mut exact = Registry {
        bytes: SAMPLE_BUDGET - length,
        ..Registry::default()
    };
    assert!(exact.load(&pdf, &r).is_ok());
    assert_eq!(exact.bytes, SAMPLE_BUDGET);
    let mut short = Registry {
        bytes: SAMPLE_BUDGET - length + 1,
        ..Registry::default()
    };
    assert!(short.load(&pdf, &r).is_err());
    assert!(short.images.is_empty());
    assert_eq!(short.bytes, SAMPLE_BUDGET - length + 1);
}

#[test]
fn redaction_images_jpeg_aggregate_input_and_work_budgets_precede_native_work() {
    let (mut pdf, r, source) = jpeg_fixture(true, Object::Null);
    let work = jpeg::inspect(&source, (64, 48, 3), Transform::Default)
        .unwrap()
        .coefficient_work;
    let maximum = jpeg::Limits::default().coefficient_work;
    let mut exact = Registry {
        input_bytes: SAMPLE_BUDGET - source.len(),
        jpeg_work: maximum - work,
        ..Registry::default()
    };
    let image = exact.load(&pdf, &r).unwrap();
    assert_eq!(exact.input_bytes, SAMPLE_BUDGET);
    assert_eq!(exact.jpeg_work, maximum);
    assert!(Arc::ptr_eq(&image, &exact.load(&pdf, &r).unwrap()));
    assert_eq!(exact.jpeg_work, maximum);
    let copy = pdf.get_object(r.as_reference().unwrap()).unwrap().clone();
    let other = Object::Reference(pdf.add_object(copy));
    assert!(exact.load(&pdf, &other).is_err());
    assert_eq!(exact.images.len(), 1);
    for mut limited in [
        Registry {
            input_bytes: SAMPLE_BUDGET - source.len() + 1,
            ..Registry::default()
        },
        Registry {
            jpeg_work: maximum - work + 1,
            ..Registry::default()
        },
    ] {
        assert!(limited.load(&pdf, &r).is_err());
        assert_eq!(limited.bytes, 0);
        assert!(limited.images.is_empty());
    }
    let mut exhausted = Registry {
        jpeg_work: maximum - work,
        ..Registry::default()
    };
    exhausted.load(&pdf, &r).unwrap();
    assert!(exhausted.load(&pdf, &other).is_err());
    assert_eq!(exhausted.jpeg_work, maximum);
    assert_eq!(exhausted.images.len(), 1);
}

#[test]
fn redaction_images_jpeg_failed_entropy_charges_attempt_without_retaining_or_poisoning() {
    let (mut pdf, r, _) = jpeg_fixture(true, Object::Null);
    let valid = include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub0-progressive0.jpg");
    let start = valid.windows(2).position(|v| v == [0xff, 0xda]).unwrap();
    let len = u16::from_be_bytes([valid[start + 2], valid[start + 3]]) as usize;
    let mut malformed = valid[..start + 2 + len].to_vec();
    malformed.extend_from_slice(&[0xff, 0xd9]);
    pdf.get_object_mut(r.as_reference().unwrap())
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .content = malformed;
    let mut registry = Registry::default();
    assert!(registry.load(&pdf, &r).is_err());
    assert!(registry.jpeg_work > 0);
    let charged = registry.jpeg_work;
    assert_eq!(registry.bytes, 0);
    assert!(registry.images.is_empty());
    pdf.get_object_mut(r.as_reference().unwrap())
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .content = valid.to_vec();
    assert!(registry.load(&pdf, &r).is_ok());
    assert!(registry.jpeg_work > charged);
    assert_eq!(registry.images.len(), 1);
}

#[test]
fn redaction_images_jpeg_parameters_types_and_page_admission_guard() {
    for params in [
        dictionary! {"ColorTransform"=>2}.into(),
        dictionary! {"ColorTransform"=>Object::Real(1.0)}.into(),
        dictionary! {"Predictor"=>1}.into(),
        Object::Array(vec![Object::Null]),
        Object::Reference((900, 0)),
    ] {
        let (pdf, r, _) = jpeg_fixture(true, params);
        assert!(Registry::default().load(&pdf, &r).is_err());
    }
    let (pdf, r, _) = jpeg_fixture(true, Object::Null);
    let resources = dictionary! {"XObject"=>dictionary!{"Im"=>r}};
    let mut registry = Registry::default();
    let mut page = PageImages::new(&pdf, Some(&resources), &mut registry, true);
    assert!(
        page.select(b"Im").is_err(),
        "JPEG page redaction needs MCU selection and opaque fill first"
    );
    assert!(page.images.is_empty());
}

#[test]
fn redaction_images_jpeg_variant_keys_and_payload_domains_keep_source_immutable() {
    let (pdf, r, _) = jpeg_fixture(true, Object::Null);
    let source = Registry::default().load(&pdf, &r).unwrap();
    let before = source.stream().unwrap().content;
    let h = source.jpeg_header().unwrap();
    let mut mask = vec![0; (h.columns * h.rows) as usize];
    let zero = source.jpeg_variant_key(&mask).unwrap();
    assert!(source.pixel_samples().is_err());
    assert!(source
        .sanitized(&vec![true; (h.width * h.height) as usize])
        .is_err());
    assert_eq!(
        source
            .sanitized_jpeg(&mask, jpeg::Limits::default())
            .unwrap()
            .digest,
        source.digest
    );
    mask[0] = 1;
    let first = source.jpeg_variant_key(&mask).unwrap();
    assert_ne!(zero, first);
    let edited = source
        .sanitized_jpeg(&mask, jpeg::Limits::default())
        .unwrap();
    assert_ne!(source.digest, edited.digest);
    assert_eq!(source.stream().unwrap().content, before);
    mask[1] = 1;
    assert_ne!(first, source.jpeg_variant_key(&mask).unwrap());
    mask[1] = 2;
    assert!(source.jpeg_variant_key(&mask).is_err());
    assert!(source.jpeg_variant_key(&[]).is_err());
    let (raw, r, _) = fixture(true, false);
    let pixels = Registry::default().load(&raw, &r).unwrap();
    assert!(pixels.jpeg_header().is_none());
    assert!(pixels.sanitized_jpeg(&[], jpeg::Limits::default()).is_err());
    // Stable digest from the pre-JPEG model's fixed RGB fixture.
    assert_eq!(
        pixels
            .digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        "e938c512349628bbca4031553bce159701670e3e2cbda3ab726eb1582ea303af"
    );
}

fn jpeg_model_pdf(stream: Stream) -> Vec<u8> {
    let mut pdf = Document::with_version("1.7");
    let tree = pdf.new_object_id();
    let image = pdf.add_object(stream);
    let content = pdf.add_object(Stream::new(
        dictionary! {},
        b"q 64 0 0 48 0 0 cm /Im Do Q".to_vec(),
    ));
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),64.into(),48.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"Im"=>image}},"Contents"=>content});
    pdf.objects.insert(
        tree,
        dictionary! {"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}.into(),
    );
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree});
    pdf.trailer.set("Root", root);
    let mut bytes = vec![];
    pdf.save_to(&mut bytes).unwrap();
    bytes
}

#[test]
fn redaction_images_jpeg_saved_model_corpus_preserves_coefficients_and_reloads() {
    for (name, data, rgb) in [
        (
            "gray-sub0-progressive0",
            include_bytes!("../tests/fixtures/redaction-jpeg/gray-sub0-progressive0.jpg")
                .as_slice(),
            false,
        ),
        (
            "gray-sub0-progressive1",
            include_bytes!("../tests/fixtures/redaction-jpeg/gray-sub0-progressive1.jpg")
                .as_slice(),
            false,
        ),
        (
            "rgb-sub0-progressive0",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub0-progressive0.jpg").as_slice(),
            true,
        ),
        (
            "rgb-sub0-progressive1",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub0-progressive1.jpg").as_slice(),
            true,
        ),
        (
            "rgb-sub1-progressive0",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub1-progressive0.jpg").as_slice(),
            true,
        ),
        (
            "rgb-sub1-progressive1",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub1-progressive1.jpg").as_slice(),
            true,
        ),
        (
            "rgb-sub2-progressive0",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub2-progressive0.jpg").as_slice(),
            true,
        ),
        (
            "rgb-sub2-progressive1",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub2-progressive1.jpg").as_slice(),
            true,
        ),
    ] {
        for direct in [false, true] {
            if direct && !rgb {
                continue;
            }
            let name = if direct {
                format!("{name}-direct")
            } else {
                name.to_string()
            };
            let (mut pdf, r, _) = jpeg_fixture(
                rgb,
                if direct {
                    dictionary! {"ColorTransform"=>0}.into()
                } else {
                    Object::Null
                },
            );
            pdf.get_object_mut(r.as_reference().unwrap())
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .content = data.to_vec();
            let source = Registry::default().load(&pdf, &r).unwrap();
            let h = source.jpeg_header().unwrap();
            let mut mask = vec![0; (h.columns * h.rows) as usize];
            for y in 16 / h.mcu_height..32 / h.mcu_height {
                for x in 16 / h.mcu_width..32 / h.mcu_width {
                    mask[(y * h.columns + x) as usize] = 1;
                }
            }
            let changed = source
                .sanitized_jpeg(&mask, jpeg::Limits::default())
                .unwrap();
            let actual = jpeg_model_pdf(changed.stream().unwrap());
            let zero = jpeg_model_pdf(source.stream().unwrap());
            let reopened = Document::load_mem(&actual).unwrap();
            let image = reopened
                .objects
                .iter()
                .find(|(_, o)| {
                    o.as_stream().ok().is_some_and(|s| {
                        s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image")
                    })
                })
                .unwrap()
                .0;
            let again = Registry::default()
                .load(&reopened, &Object::Reference(*image))
                .unwrap();
            assert_eq!(again.digest, changed.digest);
            if let Some(root) = std::env::var_os("FOLIO_IMAGE_MODEL_OUTPUT") {
                use std::io::Write;
                for (suffix, bytes) in [
                    (
                        "source",
                        jpeg_model_pdf(
                            pdf.get_object(r.as_reference().unwrap())
                                .unwrap()
                                .as_stream()
                                .unwrap()
                                .clone(),
                        ),
                    ),
                    ("zero", zero),
                    ("actual", actual),
                ] {
                    std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(std::path::PathBuf::from(&root).join(format!("{name}-{suffix}.pdf")))
                        .unwrap()
                        .write_all(&bytes)
                        .unwrap();
                }
            }
        }
    }
}

// Deliberately simple independent zlib writer: one stored DEFLATE block.
fn stored_zlib(bytes: &[u8]) -> Vec<u8> {
    assert!(bytes.len() <= 65535);
    let n = bytes.len() as u16;
    let mut out = vec![0x78, 0x01, 0x01];
    out.extend(n.to_le_bytes());
    out.extend((!n).to_le_bytes());
    out.extend(bytes);
    let (mut a, mut b) = (1u32, 0u32);
    for v in bytes {
        a = (a + u32::from(*v)) % 65521;
        b = (b + a) % 65521;
    }
    out.extend(((b << 16) | a).to_be_bytes());
    out
}
fn predicted(
    width: i64,
    height: i64,
    components: i64,
    predictor: i64,
    data: &[u8],
) -> (Document, Object) {
    let mut pdf = Document::new();
    let reference = Object::Reference(pdf.add_object(Stream::new(
        dictionary! {"Subtype"=>"Image", "Width"=>width,"Height"=>height,
            "ColorSpace"=>if components==1 {"DeviceGray"}else{"DeviceRGB"},"BitsPerComponent"=>8,
            "Filter"=>"FlateDecode", "DecodeParms"=>dictionary!{"Predictor"=>predictor,"Colors"=>components,"Columns"=>width,"BitsPerComponent"=>8}},
        stored_zlib(data))));
    (pdf, reference)
}
#[test]
fn redaction_images_predictors_decode_fixed_vectors_and_each_png_declaration() {
    // Rows: [250, 10, 30], [4, 240, 1], [12, 13, 250].
    let wanted = [250, 10, 30, 4, 240, 1, 12, 13, 250];
    let tiff = [250, 16, 20, 4, 236, 17, 12, 1, 237];
    let (pdf, reference) = predicted(3, 3, 1, 2, &tiff);
    assert_eq!(
        &*Registry::default()
            .load(&pdf, &reference)
            .unwrap()
            .pixel_samples()
            .unwrap(),
        wanted
    );
    // Sub, Up, Average. Average adds with at least nine-bit precision.
    let png = [1, 250, 16, 20, 2, 10, 230, 227, 3, 10, 143, 243];
    for predictor in 10..=15 {
        let (pdf, reference) = predicted(3, 3, 1, predictor, &png);
        assert_eq!(
            &*Registry::default()
                .load(&pdf, &reference)
                .unwrap()
                .pixel_samples()
                .unwrap(),
            wanted
        );
    }
    // RGB channels form separate chains; reset the left neighbor on each row.
    let (pdf, reference) = predicted(
        2,
        2,
        3,
        2,
        &[250, 1, 100, 16, 255, 200, 1, 2, 3, 255, 254, 253],
    );
    assert_eq!(
        &*Registry::default()
            .load(&pdf, &reference)
            .unwrap()
            .pixel_samples()
            .unwrap(),
        [250, 1, 100, 10, 0, 44, 1, 2, 3, 0, 0, 0]
    );
    // None followed by Paeth; ties must prefer left, then up, then upper-left.
    let (pdf, reference) = predicted(3, 2, 1, 14, &[0, 20, 40, 60, 4, 5, 246, 40]);
    assert_eq!(
        &*Registry::default()
            .load(&pdf, &reference)
            .unwrap()
            .pixel_samples()
            .unwrap(),
        [20, 40, 60, 25, 30, 100]
    );
}

#[test]
fn redaction_images_predictors_refuse_invalid_rows_parameters_and_zlib() {
    for data in [vec![0, 1, 2], vec![0, 1, 2, 3, 4], vec![5, 1, 2, 3]] {
        let (pdf, r) = predicted(3, 1, 1, 15, &data);
        assert!(Registry::default().load(&pdf, &r).is_err());
    }
    for (key, value) in [
        ("Predictor", 0.into()),
        ("Predictor", 16.into()),
        ("Colors", 3.into()),
        ("Columns", 2.into()),
        ("BitsPerComponent", 16.into()),
        ("EarlyChange", 1.into()),
        ("Columns", Object::Real(3.0)),
        ("Columns", i64::MAX.into()),
    ] {
        let (mut pdf, r) = predicted(3, 1, 1, 15, &[0, 1, 2, 3]);
        pdf.get_object_mut(r.as_reference().unwrap())
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .dict
            .get_mut(b"DecodeParms")
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set(key, value);
        let mut registry = Registry::default();
        assert!(registry.load(&pdf, &r).is_err(), "{key}");
        assert_eq!(registry.bytes, 0);
        assert!(registry.images.is_empty());
    }
    for mode in 0..4 {
        let (mut pdf, r) = predicted(3, 1, 1, 15, &[0, 1, 2, 3]);
        let s = pdf
            .get_object_mut(r.as_reference().unwrap())
            .unwrap()
            .as_stream_mut()
            .unwrap();
        match mode {
            0 => {
                s.content.pop();
            }
            1 => {
                let n = s.content.len();
                s.content[n - 1] ^= 1;
            }
            2 => s.content.push(0),
            _ => {
                s.dict.remove(b"Filter");
                s.content = vec![0, 1, 2, 3];
            }
        }
        assert!(Registry::default().load(&pdf, &r).is_err(), "mode {mode}");
    }
}

#[test]
fn redaction_images_predictor_defaults_width_one_and_failed_load_does_not_poison_cache() {
    for kind in [2, 10, 11, 12, 13, 14, 15] {
        let data = if kind == 2 {
            vec![250, 1, 240]
        } else {
            vec![4, 250, 2, 7, 3, 240]
        };
        let (mut pdf, r) = predicted(1, 3, 1, kind, &data);
        let s = pdf
            .get_object_mut(r.as_reference().unwrap())
            .unwrap()
            .as_stream_mut()
            .unwrap();
        s.dict.set("DecodeParms", dictionary! {"Predictor"=>kind});
        let mut registry = Registry::default();
        let expected = if kind == 2 {
            vec![250, 1, 240]
        } else {
            vec![250, 1, 240]
        };
        assert_eq!(
            &*registry.load(&pdf, &r).unwrap().pixel_samples().unwrap(),
            expected
        );
        assert_eq!(registry.bytes, 3);
    }
    let (mut pdf, r) = predicted(3, 1, 1, 15, &[5, 1, 2, 3]);
    let mut registry = Registry::default();
    assert!(registry.load(&pdf, &r).is_err());
    assert_eq!(registry.bytes, 0);
    pdf.get_object_mut(r.as_reference().unwrap())
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .content = stored_zlib(&[0, 1, 2, 3]);
    assert_eq!(
        &*registry.load(&pdf, &r).unwrap().pixel_samples().unwrap(),
        [1, 2, 3]
    );
    assert_eq!(registry.bytes, 3);
}

fn fixture(rgb: bool, compressed: bool) -> (Document, Object, Vec<u8>) {
    let pixels: Vec<u8> = (0usize..if rgb { 288 } else { 96 })
        .map(|v| ((v % 6) * 7) as u8)
        .collect();
    let mut stream = Stream::new(
        dictionary! {"Type"=>"XObject","Subtype"=>"Image",
        "Width"=>3,"Height"=>32,"BitsPerComponent"=>8,
        "ColorSpace"=>if rgb {"DeviceRGB"} else {"DeviceGray"}},
        pixels.clone(),
    );
    if compressed {
        stream.compress().unwrap();
        assert!(stream.dict.has(b"Filter"));
    }
    let mut doc = Document::new();
    let id = doc.add_object(stream);
    (doc, Object::Reference(id), pixels)
}
#[test]
fn redaction_images_decode_and_fresh_publish_preserve_exact_samples() {
    for rgb in [false, true] {
        for compressed in [false, true] {
            let (pdf, reference, pixels) = fixture(rgb, compressed);
            let image = Registry::default().load(&pdf, &reference).unwrap();
            assert_eq!(
                (image.width, image.height, image.components),
                (3, 32, if rgb { 3 } else { 1 })
            );
            assert_eq!(&*image.pixel_samples().unwrap(), pixels);
            let output = image.stream().unwrap();
            assert_eq!(
                output
                    .decompressed_content_strict_with_limit(pixels.len())
                    .unwrap(),
                pixels
            );
            assert!(output.dict.iter().all(|(k, _)| [
                b"Type".as_slice(),
                b"Subtype",
                b"Width",
                b"Height",
                b"BitsPerComponent",
                b"ColorSpace",
                b"Interpolate",
                b"Length",
                b"Filter"
            ]
            .contains(&k.as_slice())));
        }
    }
}
#[test]
fn redaction_images_shared_aliases_reuse_owned_allocation() {
    let (mut pdf, reference, _) = fixture(true, false);
    let alias = Object::Reference(pdf.add_object(reference.clone()));
    let mut registry = Registry::default();
    let first = registry.load(&pdf, &reference).unwrap();
    for _ in 0..300 {
        assert!(Arc::ptr_eq(&first, &registry.load(&pdf, &alias).unwrap()));
    }
}
#[test]
fn redaction_images_unknown_semantics_and_sample_lengths_refuse() {
    for (key, value) in [
        ("SMask", Object::Null),
        ("Mask", Object::Null),
        ("Interpolate", true.into()),
        ("ColorSpace", "DeviceCMYK".into()),
        ("BitsPerComponent", 16.into()),
        ("Width", 0.into()),
        ("Height", 32769.into()),
        (
            "Decode",
            vec![1.into(), 0.into(), 0.into(), 1.into(), 0.into(), 1.into()].into(),
        ),
        ("Filter", "DCTDecode".into()),
        ("DecodeParms", dictionary! {"Predictor"=>12}.into()),
        ("Metadata", Object::Null),
    ] {
        let (mut pdf, reference, _) = fixture(true, false);
        pdf.get_object_mut(reference.as_reference().unwrap())
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .dict
            .set(key, value);
        assert!(Registry::default().load(&pdf, &reference).is_err(), "{key}");
    }
    for delta in [-1, 1] {
        let (mut pdf, reference, _) = fixture(true, false);
        let stream = pdf
            .get_object_mut(reference.as_reference().unwrap())
            .unwrap()
            .as_stream_mut()
            .unwrap();
        if delta < 0 {
            stream.content.pop();
        } else {
            stream.content.push(7);
        }
        assert!(Registry::default().load(&pdf, &reference).is_err());
    }
}
#[test]
fn redaction_images_reference_cycles_and_unique_resource_limit_refuse() {
    let mut pdf = Document::new();
    let id = pdf.new_object_id();
    pdf.objects.insert(id, Object::Reference(id));
    assert!(Registry::default()
        .load(&pdf, &Object::Reference(id))
        .is_err());
    let mut registry = Registry::default();
    for n in 0..257 {
        let id = pdf.add_object(Stream::new(
            dictionary! {"Subtype"=>"Image","Width"=>1,"Height"=>1,
            "BitsPerComponent"=>8,"ColorSpace"=>"DeviceGray"},
            vec![(n % 256) as u8],
        ));
        assert_eq!(registry.load(&pdf, &Object::Reference(id)).is_ok(), n < 256);
    }
}

#[test]
fn redaction_images_aggregate_sample_budget_accepts_exact_limit_then_refuses() {
    let mut pdf = Document::new();
    let mut registry = Registry::default();
    for (index, (w, h)) in [
        (4000, 4000),
        (4000, 4000),
        (4000, 4000),
        (4000, 4000),
        (32768, 94),
        (28672, 1),
    ]
    .into_iter()
    .enumerate()
    {
        let mut stream = Stream::new(
            dictionary! {"Subtype"=>"Image","Width"=>w,"Height"=>h,
            "BitsPerComponent"=>8,"ColorSpace"=>"DeviceGray"},
            vec![index as u8; w as usize * h as usize],
        );
        stream.compress().unwrap();
        let reference = Object::Reference(pdf.add_object(stream));
        registry.load(&pdf, &reference).unwrap();
        assert!(Arc::ptr_eq(
            &registry.load(&pdf, &reference).unwrap(),
            registry
                .images
                .get(&reference.as_reference().unwrap())
                .unwrap()
        ));
    }
    assert_eq!(registry.bytes, SAMPLE_BUDGET);
    let extra = Object::Reference(pdf.add_object(Stream::new(
        dictionary! {"Subtype"=>"Image","Width"=>1,"Height"=>1,
        "BitsPerComponent"=>8,"ColorSpace"=>"DeviceGray"},
        vec![1],
    )));
    assert!(registry.load(&pdf, &extra).is_err());
    assert_eq!(registry.bytes, SAMPLE_BUDGET);
    assert_eq!(registry.images.len(), 6);
}

#[test]
fn redaction_images_identity_decode_parameters_preserve_raw_and_flate_samples() {
    for rgb in [false, true] {
        for compressed in [false, true] {
            for params in [
                Object::Null,
                Object::Dictionary(dictionary! {}),
                Object::Dictionary(dictionary! {"Predictor"=>1}),
            ] {
                let (mut pdf, reference, pixels) = fixture(rgb, compressed);
                pdf.get_object_mut(reference.as_reference().unwrap())
                    .unwrap()
                    .as_stream_mut()
                    .unwrap()
                    .dict
                    .set("DecodeParms", params.clone());
                let image = Registry::default()
                    .load(&pdf, &reference)
                    .unwrap_or_else(|e| {
                        panic!("rgb={rgb}, compressed={compressed}, parameters={params:?}: {e:?}")
                    });
                assert_eq!(&*image.pixel_samples().unwrap(), pixels);
                assert!(!image.stream().unwrap().dict.has(b"DecodeParms"));
            }
        }
    }
}
