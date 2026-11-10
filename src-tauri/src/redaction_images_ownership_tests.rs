use super::*;
use crate::redaction_jpeg_coverage::{Coverage, CoverageBudget};
use lopdf::content::{Content, Operation};

fn source(rgb: bool) -> (Document, Object, Arc<Image>) {
    let (pdf, value, _) = super::tests::jpeg_fixture(rgb, Object::Null);
    let image = Registry::default().load(&pdf, &value).unwrap();
    (pdf, value, image)
}
fn coverage(image: &Image, index: usize) -> Coverage {
    let header = image.jpeg_header().unwrap();
    let mut mask = vec![0; (header.columns * header.rows) as usize];
    mask[index] = 1;
    Coverage::from_mask(header, &mask, &mut CoverageBudget::default()).unwrap()
}
fn redact(image: &Image, index: usize) -> Image {
    image
        .redacted_jpeg(
            &coverage(image, index),
            jpeg::Limits::default(),
            &mut CoverageBudget::default(),
        )
        .unwrap()
}
fn reload(stream: Stream) -> EngineResult<Arc<Image>> {
    let mut pdf = Document::new();
    let value = Object::Reference(pdf.add_object(stream));
    Registry::default().load(&pdf, &value)
}
fn record(image: &Image, mask: &Coverage) -> Object {
    dictionary!{"V"=>1,"Mask"=>Object::String(mask.mask().to_vec(),lopdf::StringFormat::Hexadecimal),"Binding"=>Object::String(mask.binding(&image.digest).to_vec(),lopdf::StringFormat::Hexadecimal)}.into()
}

#[test]
fn redaction_images_triangle_union_replays_exactly_and_upgrades_only_when_selected() {
    let (_, _, original) = source(true);
    let head = original.jpeg_header().unwrap();
    let mask = Coverage::from_mask(
        head,
        &[1, 1, 1, 0, 1, 0, 1, 0, 1, 1, 1, 0],
        &mut CoverageBudget::default(),
    )
    .unwrap();
    let modern = original
        .redacted_jpeg(
            &mask,
            jpeg::Limits::default(),
            &mut CoverageBudget::default(),
        )
        .unwrap();
    let mut stream = modern.stream().unwrap();
    stream.dict.remove(b"FolioJpegRedaction");
    let base = reload(stream.clone()).unwrap();
    stream.dict.set("FolioJpegRedaction", dictionary!{"V"=>2,"Mask"=>Object::String(mask.mask().to_vec(),lopdf::StringFormat::Hexadecimal),"Binding"=>Object::String(mask.paint_binding(&base.digest,true).to_vec(),lopdf::StringFormat::Hexadecimal)});
    let old = reload(stream.clone()).unwrap();
    let expected = mask
        .version_paint(&mut CoverageBudget::default(), PaintVersion::Triangles)
        .unwrap();
    let actual_paint = old
        .jpeg_paint(&mut CoverageBudget::default())
        .unwrap()
        .unwrap();
    assert_eq!(
        Content {
            operations: actual_paint.operations().to_vec()
        }
        .encode()
        .unwrap(),
        Content {
            operations: expected.operations().to_vec()
        }
        .encode()
        .unwrap()
    );
    let republished = old.stream().unwrap();
    assert_eq!(republished.content, stream.content);
    assert_eq!(
        republished.dict.get(b"FolioJpegRedaction").unwrap(),
        stream.dict.get(b"FolioJpegRedaction").unwrap()
    );
    assert_ne!(old.digest, modern.digest);
    let mut context = ImageContext::for_jpeg_validation();
    let mut document = Document::new();
    let source_id = document.add_object(stream.clone());
    let loaded = context
        .begin_document()
        .load(&document, &Object::Reference(source_id))
        .unwrap();
    let upgraded = context.redact_jpeg(&loaded, &mask).unwrap();
    assert_eq!(upgraded.digest, modern.digest);
    assert_eq!(
        upgraded
            .jpeg_paint(&mut CoverageBudget::default())
            .unwrap()
            .unwrap()
            .operations()
            .len(),
        20
    );
    let unchanged = reload(stream).unwrap();
    assert_eq!(unchanged.digest, old.digest);
}

#[test]
fn redaction_images_paint_versions_keep_legacy_unedited_and_bind_cached_claims() {
    let (_, _, source) = source(true);
    let changed = redact(&source, 5);
    let modern = changed.stream().unwrap();
    assert_eq!(
        modern
            .dict
            .get(b"FolioJpegRedaction")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"V")
            .unwrap()
            .as_i64()
            .unwrap(),
        3
    );
    let mut bare = modern.clone();
    bare.dict.remove(b"FolioJpegRedaction");
    let base = reload(bare).unwrap();
    let mut legacy = modern.clone();
    legacy
        .dict
        .set("FolioJpegRedaction", record(&base, &coverage(&base, 5)));
    let old = reload(legacy.clone()).unwrap();
    let logical = operations(b"/I0 Do");
    let old_ops = publish_operations(
        logical.clone(),
        &[old.clone()],
        &mut CoverageBudget::default(),
    )
    .unwrap();
    assert_eq!(
        old_ops.len(),
        10,
        "unedited old paint must not silently change"
    );
    assert_ne!(old.digest, changed.digest);
    let mut context = ImageContext::for_jpeg_validation();
    let mut doc = Document::new();
    let id = doc.add_object(legacy);
    let loaded = context
        .begin_document()
        .load(&doc, &Object::Reference(id))
        .unwrap();
    let upgraded = context.redact_jpeg(&loaded, &coverage(&loaded, 5)).unwrap();
    assert_eq!(upgraded.digest, changed.digest);
    assert_eq!(
        publish_operations(logical, &[upgraded], &mut CoverageBudget::default())
            .unwrap()
            .len(),
        13
    );
    // A version-only mutation cannot reuse the already validated old entry.
    doc.get_object_mut(id)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .dict
        .get_mut(b"FolioJpegRedaction")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("V", 2);
    assert!(context
        .begin_document()
        .load(&doc, &Object::Reference(id))
        .is_err());
}

#[test]
fn redaction_images_owned_coverage_survives_reload_and_preserves_source() {
    for rgb in [false, true] {
        let (pdf, value, image) = source(rgb);
        let before = pdf
            .get_object(value.as_reference().unwrap())
            .unwrap()
            .as_stream()
            .unwrap()
            .content
            .clone();
        let changed = redact(&image, 5);
        let stream = changed.stream().unwrap();
        assert!(stream.dict.has(b"FolioJpegRedaction"));
        assert_eq!(
            changed.retained_bytes(),
            stream.content.len() + changed.jpeg_coverage().unwrap().mask().len()
        );
        let restored = reload(stream.clone()).unwrap();
        assert_eq!(restored.digest, changed.digest);
        assert_eq!(
            restored.jpeg_coverage().unwrap().mask(),
            changed.jpeg_coverage().unwrap().mask()
        );
        assert_eq!(restored.stream().unwrap().content, stream.content);
        assert_eq!(
            pdf.get_object(value.as_reference().unwrap())
                .unwrap()
                .as_stream()
                .unwrap()
                .content,
            before
        );
    }
}
#[test]
fn redaction_images_reedit_unions_coverage_and_refuses_dropping_ownership() {
    let (_, _, image) = source(true);
    let first = redact(&image, 5);
    let second = redact(&reload(first.stream().unwrap()).unwrap(), 6);
    assert_eq!(
        second.jpeg_coverage().unwrap().mask(),
        &[0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0]
    );
    let repeated = redact(&reload(second.stream().unwrap()).unwrap(), 5);
    assert_eq!(second.digest, repeated.digest);
    assert_eq!(
        second.stream().unwrap().content,
        repeated.stream().unwrap().content
    );
    assert!(second
        .sanitized_jpeg(&[0; 12], jpeg::Limits::default())
        .is_err());
    let empty = Coverage::from_mask(
        image.jpeg_header().unwrap(),
        &[0; 12],
        &mut CoverageBudget::default(),
    )
    .unwrap();
    assert!(image
        .redacted_jpeg(
            &empty,
            jpeg::Limits::default(),
            &mut CoverageBudget::default()
        )
        .is_err());
}
#[test]
fn redaction_images_coverage_identity_includes_paint_even_when_coefficients_match() {
    let (_, _, image) = source(true);
    let black = image
        .sanitized_jpeg(&[1; 12], jpeg::Limits::default())
        .unwrap();
    let a = redact(&black, 5);
    let b = redact(&black, 6);
    assert_eq!(a.stream().unwrap().content, b.stream().unwrap().content);
    assert_ne!(a.digest, b.digest);
    assert_ne!(a.digest, black.digest);
    assert_eq!(reload(a.stream().unwrap()).unwrap().digest, a.digest);
    assert_eq!(reload(b.stream().unwrap()).unwrap().digest, b.digest);
}
#[test]
fn redaction_images_coverage_record_requires_already_removed_coefficients() {
    let (_, _, image) = source(true);
    let mut forged = image.stream().unwrap();
    // A matching public checksum is not authority to claim unmodified blocks.
    forged
        .dict
        .set("FolioJpegRedaction", record(&image, &coverage(&image, 5)));
    assert!(reload(forged).is_err());
    let changed = redact(&image, 5);
    let valid = changed.stream().unwrap();
    for variant in 0..7 {
        let mut bad = valid.clone();
        let r = bad
            .dict
            .get_mut(b"FolioJpegRedaction")
            .unwrap()
            .as_dict_mut()
            .unwrap();
        match variant {
            0 => {
                r.set("V", 4);
            }
            1 => {
                r.set("Extra", true);
            }
            2 => {
                r.set(
                    "Mask",
                    Object::String(vec![0; 12], lopdf::StringFormat::Hexadecimal),
                );
            }
            3 => {
                r.set(
                    "Mask",
                    Object::String(vec![2; 12], lopdf::StringFormat::Hexadecimal),
                );
            }
            4 => {
                r.set(
                    "Mask",
                    Object::String(vec![1; 13], lopdf::StringFormat::Hexadecimal),
                );
            }
            5 => {
                r.set(
                    "Binding",
                    Object::String(vec![0; 32], lopdf::StringFormat::Hexadecimal),
                );
            }
            _ => {
                r.set(
                    "Binding",
                    Object::String(vec![0; 31], lopdf::StringFormat::Hexadecimal),
                );
            }
        }
        assert!(reload(bad).is_err(), "variant {variant}");
    }
    let mut wrong_color = valid.clone();
    wrong_color
        .dict
        .set("DecodeParms", dictionary! {"ColorTransform"=>0});
    assert!(reload(wrong_color).is_err());
    let mut null = valid;
    null.dict.set("FolioJpegRedaction", Object::Null);
    assert!(reload(null).is_err());
}
#[test]
fn redaction_images_owned_paint_is_verified_for_every_alias_occurrence() {
    let (_, _, image) = source(true);
    let changed = redact(&image, 5);
    let mut pdf = Document::new();
    let id = pdf.add_object(changed.stream().unwrap());
    let alias = pdf.add_object(Object::Reference(id));
    let mut registry = Registry::default();
    let a = registry.load(&pdf, &Object::Reference(id)).unwrap();
    let b = registry.load(&pdf, &Object::Reference(alias)).unwrap();
    assert!(Arc::ptr_eq(&a, &b));
    let paint = a
        .jpeg_coverage()
        .unwrap()
        .paint(&mut CoverageBudget::default())
        .unwrap();
    let serialized = Content {
        operations: paint.operations().to_vec(),
    }
    .encode()
    .unwrap();
    let mut following = Content::decode_strict(&serialized).unwrap().operations;
    let count = following.len();
    following.push(Operation::new("q", vec![]));
    assert_eq!(
        a.verify_paint_prefix(&following, &mut CoverageBudget::default())
            .unwrap(),
        count
    );
    // Exact previously published rectangle grammar remains readable; arbitrary
    // paths, changed colors or changed edges never acquire ownership.
    let legacy = Content::decode_strict(b"q 0 g 0.24999999 0.3333333 m 0.50000006 0.3333333 l 0.50000006 0.66666675 l 0.24999999 0.66666675 l h f Q").unwrap().operations;
    assert!(a
        .verify_paint_prefix(&legacy, &mut CoverageBudget::default())
        .is_err());
    assert!(count > legacy.len());
    let mut altered = legacy.clone();
    altered[4].operands[0] = Object::Real(0.75);
    assert!(a
        .verify_paint_prefix(&altered, &mut CoverageBudget::default())
        .is_err());
    for variant in 0..5 {
        let mut bad = following.clone();
        match variant {
            0 => {
                bad[1].operands[0] = Object::Integer(1);
            }
            1 => {
                bad[2].operands[0] = Object::Real(0.125);
            }
            2 => {
                bad.remove(0);
            }
            3 => {
                bad.truncate(count - 1);
            }
            _ => {
                bad.insert(0, Operation::new("n", vec![]));
            }
        }
        assert!(
            b.verify_paint_prefix(&bad, &mut CoverageBudget::default())
                .is_err(),
            "variant {variant}"
        );
    }
    assert_eq!(
        image
            .verify_paint_prefix(&[], &mut CoverageBudget::default())
            .unwrap(),
        0
    );
}

#[test]
fn redaction_images_coverage_proof_charges_second_decode_and_retained_mask() {
    let (_, _, image) = source(true);
    let changed = redact(&image, 5);
    let stream = changed.stream().unwrap();
    let size = changed.retained_bytes();
    let mut pdf = Document::new();
    let id = pdf.add_object(stream.clone());
    let value = Object::Reference(id);
    let work = image.jpeg_header().unwrap().coefficient_work;
    let maximum = jpeg::Limits::default().coefficient_work;
    let mut exact = Registry {
        jpeg_work: maximum - 2 * work,
        bytes: SAMPLE_BUDGET - size,
        ..Default::default()
    };
    assert_eq!(exact.load(&pdf, &value).unwrap().digest, changed.digest);
    assert_eq!(exact.jpeg_work, maximum);
    assert_eq!(exact.bytes, SAMPLE_BUDGET);
    let mut short = Registry {
        jpeg_work: maximum - 2 * work + 1,
        ..Default::default()
    };
    assert!(short.load(&pdf, &value).is_err());
    assert!(short.images.is_empty());
    assert_eq!(short.bytes, 0);
    assert_eq!(short.jpeg_work, maximum - work + 1);
    let mut short = Registry {
        bytes: SAMPLE_BUDGET - size + 1,
        ..Default::default()
    };
    assert!(short.load(&pdf, &value).is_err());
    assert!(short.images.is_empty());
    let input = stream.content.len() + 12 + 32;
    let mut exact = Registry {
        input_bytes: SAMPLE_BUDGET - input,
        ..Default::default()
    };
    assert!(exact.load(&pdf, &value).is_ok());
    assert_eq!(exact.input_bytes, SAMPLE_BUDGET);
    let mut short = Registry {
        input_bytes: SAMPLE_BUDGET - input + 1,
        ..Default::default()
    };
    assert!(short.load(&pdf, &value).is_err());
    assert_eq!(short.jpeg_work, 0);
}

#[test]
fn redaction_images_owned_output_cannot_bypass_maximum_by_subtracting_mask_bytes() {
    let (_, _, image) = source(true);
    let selected = coverage(&image, 5);
    assert!(image
        .redacted_jpeg(
            &selected,
            jpeg::Limits {
                output_bytes: jpeg::Limits::default().output_bytes + 1,
                ..jpeg::Limits::default()
            },
            &mut CoverageBudget::default()
        )
        .is_err());
    let changed = redact(&image, 5);
    let exact = changed.retained_bytes();
    let output = image
        .redacted_jpeg(
            &selected,
            jpeg::Limits {
                output_bytes: exact,
                ..jpeg::Limits::default()
            },
            &mut CoverageBudget::default(),
        )
        .unwrap();
    assert_eq!(output.retained_bytes(), exact);
    assert!(image
        .redacted_jpeg(
            &selected,
            jpeg::Limits {
                output_bytes: exact - 1,
                ..jpeg::Limits::default()
            },
            &mut CoverageBudget::default()
        )
        .is_err());
}

fn operations(data: &[u8]) -> Vec<Operation> {
    Content::decode_strict(data).unwrap().operations
}
#[test]
fn redaction_images_publisher_recreates_owned_paint_after_every_occurrence_only() {
    let (_, _, source) = source(true);
    let owned = Arc::new(redact(&source, 5));
    let images = vec![owned.clone(), source];
    let output = publish_operations(
        operations(b"q 64 0 0 48 0 0 cm /I0 Do Q q /I1 Do Q /I0 Do"),
        &images,
        &mut CoverageBudget::default(),
    )
    .unwrap();
    let mut found = 0;
    for (index, op) in output
        .iter()
        .enumerate()
        .filter(|(_, op)| op.operator == "Do")
    {
        let alias = op.operands[0].as_name().unwrap();
        if alias == b"I0" {
            assert_eq!(
                owned
                    .verify_paint_prefix(&output[index + 1..], &mut CoverageBudget::default())
                    .unwrap(),
                12
            );
            found += 1;
        } else {
            assert_eq!(output[index + 1].operator, "Q");
        }
    }
    assert_eq!(found, 2);
    assert_eq!(output.iter().filter(|op| op.operator == "f").count(), 2);
}
#[test]
fn redaction_images_owned_paint_refuses_pending_paths_clips_and_excessive_nesting() {
    let (_, _, source) = source(true);
    let images = vec![Arc::new(redact(&source, 5))];
    for data in [
        b"0 0 m /I0 Do".as_slice(),
        b"0 0 m q Q /I0 Do",
        b"0 0 1 1 re W /I0 Do",
        b"0 0 1 1 re W n /I0 Do",
        b"W /I0 Do",
        b"BT /I0 Do ET",
    ] {
        assert!(
            publish_operations(operations(data), &images, &mut CoverageBudget::default()).is_err()
        );
    }
    assert!(publish_operations(
        operations(b"q 0 0 1 1 re W n Q /I0 Do"),
        &images,
        &mut CoverageBudget::default()
    )
    .is_ok());
    for (depth, allowed) in [(63, true), (64, false)] {
        let data = format!("{} /I0 Do {}", "q ".repeat(depth), "Q ".repeat(depth));
        assert_eq!(
            publish_operations(
                operations(data.as_bytes()),
                &images,
                &mut CoverageBudget::default()
            )
            .is_ok(),
            allowed
        );
    }
}
#[test]
fn redaction_images_publication_counts_source_and_inserted_operations_together() {
    let (_, _, source) = source(true);
    let images = vec![Arc::new(redact(&source, 5))];
    let make = |n| {
        let mut ops = vec![Operation::new("n", vec![]); n];
        ops.push(Operation::new("Do", vec![Object::Name(b"I0".to_vec())]));
        ops
    };
    assert!(publish_operations(make(99_987), &images, &mut CoverageBudget::default()).is_ok());
    assert!(publish_operations(make(99_988), &images, &mut CoverageBudget::default()).is_err());
    let mut budget = CoverageBudget::default();
    assert!(publish_operations(make(49_987), &images, &mut budget).is_ok());
    assert!(publish_operations(make(49_987), &images, &mut budget).is_ok());
    assert!(publish_operations(make(0), &images, &mut budget).is_err());
}

#[test]
fn redaction_images_source_owned_fill_is_removed_from_logical_paths_only_after_proof() {
    let (_, _, source) = source(true);
    let owned = Arc::new(redact(&source, 5));
    let images = vec![owned];
    let logical = operations(b"q 64 0 0 48 0 0 cm /I0 Do Q /I0 Do 0 0 1 1 re f");
    let painted =
        publish_operations(logical.clone(), &images, &mut CoverageBudget::default()).unwrap();
    let bytes = Content {
        operations: painted,
    }
    .encode()
    .unwrap();
    let parsed = Content::decode_strict(&bytes).unwrap().operations;
    let restored =
        strip_owned_paint(parsed.clone(), &images, &mut CoverageBudget::default()).unwrap();
    assert_eq!(
        Content {
            operations: restored
        }
        .encode()
        .unwrap(),
        Content {
            operations: logical
        }
        .encode()
        .unwrap()
    );
    // The second occurrence must be checked despite sharing an image resource.
    let second = parsed
        .iter()
        .enumerate()
        .filter(|(_, op)| op.operator == "Do")
        .nth(1)
        .unwrap()
        .0;
    let mut bad = parsed.clone();
    bad[second + 2].operands[0] = Object::Integer(1);
    assert!(strip_owned_paint(bad, &images, &mut CoverageBudget::default()).is_err());
    let mut bad = parsed;
    bad.insert(0, Operation::new("m", vec![0.into(), 0.into()]));
    assert!(strip_owned_paint(bad, &images, &mut CoverageBudget::default()).is_err());
}
