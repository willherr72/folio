use super::*;
fn selected(image: &Image, cell: usize) -> Coverage {
    let header = image.jpeg_header().unwrap();
    let mut mask = vec![0; (header.columns * header.rows) as usize];
    mask[cell] = 1;
    Coverage::from_mask(header, &mask, &mut CoverageBudget::default()).unwrap()
}
#[test]
fn redaction_image_cache_jpeg_variants_reuse_work_preserve_old_coverage_and_seed_reopen() {
    let (pdf, value) = fixture(true, Object::Null);
    let mut context = ImageContext::default();
    let source = context.begin_document().load(&pdf, &value).unwrap();
    let mark = selected(&source, 0);
    let first = context.redact_jpeg(&source, &mark).unwrap();
    let work = context.registry.jpeg_work;
    let bytes = context.variant_bytes;
    assert!(Arc::ptr_eq(
        &first,
        &context.redact_jpeg(&source, &mark).unwrap()
    ));
    assert!(Arc::ptr_eq(
        &first,
        &context.redact_jpeg(&first, &mark).unwrap()
    ));
    assert_eq!(context.registry.jpeg_work, work);
    assert_eq!(context.variant_bytes, bytes);
    let second = context.redact_jpeg(&first, &selected(&first, 1)).unwrap();
    assert_eq!(&second.jpeg_coverage().unwrap().mask()[..2], &[1, 1]);
    let (pdf, key) = serialized(second.stream().unwrap());
    context.registry.jpeg_work = jpeg::Limits::default().coefficient_work;
    context.registry.input_bytes = SAMPLE_BUDGET;
    context.variant_bytes = SAMPLE_BUDGET;
    assert!(Arc::ptr_eq(
        &second,
        &context.begin_document().load(&pdf, &key).unwrap()
    ));
    assert!(Arc::ptr_eq(
        &first,
        &context.redact_jpeg(&source, &mark).unwrap()
    ));
    assert!(context.redact_jpeg(&source, &selected(&source, 2)).is_err());
}
#[test]
fn redaction_image_cache_variant_sources_must_belong_to_this_context() {
    let (pdf, value) = fixture(true, Object::Null);
    let mut a = ImageContext::default();
    let mut b = ImageContext::default();
    let one = a.begin_document().load(&pdf, &value).unwrap();
    let two = b.begin_document().load(&pdf, &value).unwrap();
    assert_eq!(one.digest, two.digest);
    assert!(!Arc::ptr_eq(&one, &two));
    assert!(a.redact_jpeg(&two, &selected(&two, 0)).is_err());
    assert!(a.redact_jpeg(&one, &selected(&one, 0)).is_ok());
}
#[test]
fn redaction_image_cache_variant_failure_is_charged_but_does_not_install_or_poison() {
    let (pdf, value) = fixture(true, Object::Null);
    let mut context = ImageContext::default();
    let source = context.begin_document().load(&pdf, &value).unwrap();
    let mark = selected(&source, 0);
    let mask = mark.mask().len();
    let before = context.registry.jpeg_work;
    context.variant_bytes = SAMPLE_BUDGET - mask - 1;
    assert!(context.redact_jpeg(&source, &mark).is_err());
    assert!(context.registry.jpeg_work > before);
    assert!(context.variants.is_empty());
    assert_eq!(context.registry.reuse.as_ref().unwrap().owned.len(), 1);
    assert_eq!(context.variant_bytes, SAMPLE_BUDGET - mask - 1);
    context.variant_bytes = 0;
    assert!(context.redact_jpeg(&source, &mark).is_ok());
    assert_eq!(context.variants.len(), 1);
}
#[test]
fn redaction_image_cache_pixel_variants_have_exact_limits_and_reuse_serialized_outputs() {
    let (pdf, value) = serialized(Stream::new(
        dictionary! {"Subtype"=>"Image","Width"=>2,"Height"=>1,"ColorSpace"=>"DeviceGray","BitsPerComponent"=>8},
        vec![11, 22],
    ));
    let mut context = ImageContext::default();
    let source = context.begin_document().load(&pdf, &value).unwrap();
    context.variant_bytes = SAMPLE_BUDGET - 1;
    assert!(context.redact_pixels(&source, &[true, false]).is_err());
    context.variant_bytes = SAMPLE_BUDGET - 2;
    let result = context.redact_pixels(&source, &[true, false]).unwrap();
    assert_eq!(result.pixel_samples().unwrap(), &[0, 22]);
    assert_eq!(context.variant_bytes, SAMPLE_BUDGET);
    assert!(Arc::ptr_eq(
        &result,
        &context.redact_pixels(&source, &[true, false]).unwrap()
    ));
    assert!(Arc::ptr_eq(
        &result,
        &context.redact_pixels(&result, &[true, false]).unwrap()
    ));
    assert!(context.redact_pixels(&source, &[false, true]).is_err());
    let (pdf, key) = serialized(result.stream().unwrap());
    context.registry.input_bytes = SAMPLE_BUDGET;
    assert!(Arc::ptr_eq(
        &result,
        &context.begin_document().load(&pdf, &key).unwrap()
    ));
}
fn fixture(rgb: bool, params: Object) -> (Document, Object) {
    let (pdf, value, _) = super::super::tests::jpeg_fixture(rgb, params);
    (pdf, value)
}
fn serialized(stream: Stream) -> (Document, Object) {
    let mut pdf = Document::new();
    let value = Object::Reference(pdf.add_object(stream));
    (pdf, value)
}
#[test]
fn redaction_image_cache_reuses_verified_source_and_canonical_reopen_without_codec_work() {
    let (pdf, value) = fixture(true, Object::Null);
    let mut context = ImageContext::default();
    let first = context.begin_document().load(&pdf, &value).unwrap();
    let work = context.registry.jpeg_work;
    let retained = context.registry.bytes;
    let again = context.begin_document().load(&pdf, &value).unwrap();
    assert!(Arc::ptr_eq(&first, &again));
    assert_eq!(context.registry.jpeg_work, work);
    assert_eq!(context.registry.bytes, retained);
    let (reopened, key) = serialized(first.stream().unwrap());
    context.registry.jpeg_work = jpeg::Limits::default().coefficient_work;
    context.registry.input_bytes = SAMPLE_BUDGET;
    let loaded = context.begin_document().load(&reopened, &key).unwrap();
    assert!(Arc::ptr_eq(&first, &loaded));
}
#[test]
fn redaction_image_cache_never_reuses_object_ids_or_incompatible_color_interpretations() {
    let (rgb, r) = fixture(true, Object::Null);
    let (gray, g) = fixture(false, Object::Null);
    assert_eq!(r.as_reference().unwrap(), g.as_reference().unwrap());
    let mut context = ImageContext::default();
    let a = context.begin_document().load(&rgb, &r).unwrap();
    let b = context.begin_document().load(&gray, &g).unwrap();
    assert_eq!(b.components, 1);
    assert_ne!(a.digest, b.digest);
    let (direct, d) = fixture(true, dictionary! {"ColorTransform"=>0}.into());
    let c = context.begin_document().load(&direct, &d).unwrap();
    assert_ne!(a.digest, c.digest);
    assert_eq!(a.stream().unwrap().content, c.stream().unwrap().content);
    assert!(Arc::ptr_eq(
        &a,
        &context.begin_document().load(&rgb, &r).unwrap()
    ));
}
#[test]
fn redaction_image_cache_predictors_and_invalid_dictionary_fields_cannot_hide_behind_hits() {
    let mut stream = Stream::new(
        dictionary! {"Subtype"=>"Image","Width"=>2,"Height"=>1,"ColorSpace"=>"DeviceGray","BitsPerComponent"=>8,"Filter"=>"FlateDecode"},
        vec![0x78, 1, 1, 2, 0, 253, 255, 1, 2, 0, 6, 0, 4],
    );
    let (plain, p) = serialized(stream.clone());
    stream.dict.set(
        "DecodeParms",
        dictionary! {"Predictor"=>2,"Colors"=>1,"Columns"=>2},
    );
    let (predicted, q) = serialized(stream.clone());
    let mut context = ImageContext::default();
    let a = context.begin_document().load(&plain, &p).unwrap();
    let b = context.begin_document().load(&predicted, &q).unwrap();
    assert_eq!(a.pixel_samples().unwrap(), &[1, 2]);
    assert_eq!(b.pixel_samples().unwrap(), &[1, 3]);
    assert!(Arc::ptr_eq(
        &a,
        &context.begin_document().load(&plain, &p).unwrap()
    ));
    stream.dict.set("Interpolate", true);
    let (bad, r) = serialized(stream);
    assert!(context.begin_document().load(&bad, &r).is_err());
    let mut invalid = plain.clone();
    invalid
        .get_object_mut(p.as_reference().unwrap())
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .dict
        .set("DecodeParms", dictionary! {"Predictor"=>1,"Unknown"=>true});
    assert!(context.begin_document().load(&invalid, &p).is_err());
}

#[test]
fn redaction_image_cache_hash_limit_applies_even_to_hits_and_canonical_seeding() {
    let (pdf, value) = fixture(true, Object::Null);
    let size = pdf
        .get_object(value.as_reference().unwrap())
        .unwrap()
        .as_stream()
        .unwrap()
        .content
        .len();
    let mut context = ImageContext::default();
    let image = context.begin_document().load(&pdf, &value).unwrap();
    context.registry.reuse.as_mut().unwrap().hashed_bytes = HASH_BYTES - size - 128;
    assert!(Arc::ptr_eq(
        &image,
        &context.begin_document().load(&pdf, &value).unwrap()
    ));
    assert_eq!(
        context.registry.reuse.as_ref().unwrap().hashed_bytes,
        HASH_BYTES
    );
    let work = context.registry.jpeg_work;
    assert!(context.begin_document().load(&pdf, &value).is_err());
    assert_eq!(context.registry.jpeg_work, work);
    let mut fresh = ImageContext::default();
    fresh.registry.reuse.as_mut().unwrap().hashed_bytes = HASH_BYTES - size - 128;
    assert!(fresh.begin_document().load(&pdf, &value).is_err());
    assert_eq!(fresh.registry.bytes, 0);
    assert!(fresh.registry.reuse.as_ref().unwrap().inputs.is_empty());
}

#[test]
fn redaction_image_cache_failed_entropy_never_installs_a_proof_or_refunds_attempted_work() {
    let (mut pdf, value) = fixture(true, Object::Null);
    let bytes = include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub0-progressive0.jpg");
    let start = bytes.windows(2).position(|v| v == [0xff, 0xda]).unwrap();
    let length = u16::from_be_bytes([bytes[start + 2], bytes[start + 3]]) as usize;
    let mut bad = bytes[..start + 2 + length].to_vec();
    bad.extend_from_slice(&[0xff, 0xd9]);
    let id = value.as_reference().unwrap();
    pdf.get_object_mut(id)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .content = bad;
    let mut context = ImageContext::default();
    assert!(context.begin_document().load(&pdf, &value).is_err());
    let work = context.registry.jpeg_work;
    assert!(work > 0);
    assert!(context.begin_document().load(&pdf, &value).is_err());
    assert_eq!(context.registry.jpeg_work, 2 * work);
    assert_eq!(context.registry.bytes, 0);
    assert!(context.registry.reuse.as_ref().unwrap().inputs.is_empty());
    pdf.get_object_mut(id)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .content = bytes.to_vec();
    assert!(context.begin_document().load(&pdf, &value).is_ok());
}

#[test]
fn redaction_image_cache_source_attempt_limit_cannot_grow_via_stripped_metadata() {
    let (mut pdf, value) = fixture(true, Object::Null);
    let id = value.as_reference().unwrap();
    let original = pdf
        .get_object(id)
        .unwrap()
        .as_stream()
        .unwrap()
        .content
        .clone();
    let mut context = ImageContext::default();
    let mut first = None;
    for tag in 0..=256u16 {
        let mut bytes = original[..2].to_vec();
        bytes.extend_from_slice(&[0xff, 0xfe, 0, 4]);
        bytes.extend_from_slice(&tag.to_be_bytes());
        bytes.extend_from_slice(&original[2..]);
        pdf.get_object_mut(id)
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .content = bytes;
        let loaded = context.begin_document().load(&pdf, &value);
        if tag == 256 {
            assert!(loaded.is_err());
        } else {
            let image = loaded.unwrap();
            if let Some(first) = &first {
                assert!(Arc::ptr_eq(first, &image));
            } else {
                first = Some(image);
            }
        }
    }
    assert_eq!(context.registry.reuse.as_ref().unwrap().source_misses, 256);
    assert_eq!(context.registry.reuse.as_ref().unwrap().owned.len(), 1);
    assert_eq!(context.registry.bytes, first.unwrap().retained_bytes());
}
