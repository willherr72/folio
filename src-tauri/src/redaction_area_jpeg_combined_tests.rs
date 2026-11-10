use super::*;
use crate::{
    redaction_images::{ImageContext, Registry},
    redaction_jpeg::{inspect, Transform},
    redaction_jpeg_coverage::CoverageBudget,
};
use lopdf::{content::Content, dictionary, Document, Object, Stream};

fn save(name: &str, bytes: &[u8]) {
    if let Some(root) = std::env::var_os("FOLIO_JPEG_TRANSACTION_OUTPUT") {
        use std::io::Write;
        let root = PathBuf::from(root).join("combined-render");
        std::fs::create_dir_all(&root).unwrap();
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
fn redaction_area_jpeg_combined_reader_bounds_export_actual_saved_transactions() {
    let fixtures: &[(&str, &[u8], (u32, u32, u32))] = &[
        (
            "rgb",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub2-progressive1.jpg"),
            (64, 48, 3),
        ),
        (
            "gray",
            include_bytes!("../tests/fixtures/redaction-jpeg/gray-sub0-progressive0.jpg"),
            (64, 48, 1),
        ),
        (
            "partial",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-rgb-sub2-progressive1.jpg"
            ),
            (65, 49, 3),
        ),
        (
            "large",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-large-owned.jpg"),
            (256, 192, 3),
        ),
    ];
    let layouts: [(&str, [f32; 6]); 7] = [
        ("axis", [128., 0., 0., 96., 20., 24.]),
        ("rotate", [0., 128., -96., 0., 116., 16.]),
        ("reflect", [-128., 0., 0., 96., 156., 24.]),
        ("shear", [100., 10., 25., 90., 20., 20.]),
        ("fractional", [123.5, 0., 0., 95.25, 21.125, 22.5]),
        ("small", [8., 0., 0., 6., 40., 40.]),
        ("nested", [64., 0., 0., 48., 0., 0.]),
    ];
    let mut records = Vec::new();
    for &(fixture, input, shape) in fixtures {
        let header = inspect(input, shape, Transform::Default).unwrap();
        for (layout, matrix) in layouts {
            let name = format!("{fixture}-{layout}");
            let mut steps = vec![];
            if layout == "nested" {
                steps.extend([[1.1, 0.2, 0.3, 0.9, 35., 25.], [0.7, 0.1, 0.2, 1.3, 4., 5.]]);
            }
            steps.push(matrix);
            let mut doc = Document::new();
            let id=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>shape.0 as i64,"Height"=>shape.1 as i64,"BitsPerComponent"=>8,"ColorSpace"=>if shape.2==1 {"DeviceGray"} else {"DeviceRGB"},"Filter"=>"DCTDecode"},input.to_vec()));
            let image = Registry::default()
                .load(&doc, &Object::Reference(id))
                .unwrap();
            let mut program = String::from("q ");
            let mut transform = crate::redaction_area_paths::ImageTransform::default();
            for step in &steps {
                program.push_str(
                    &step
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(" "),
                );
                program.push_str(" cm ");
                transform = transform.concat(step.map(f64::from)).unwrap();
            }
            program.push_str("/I0 Do Q");
            let page = crate::redaction_core::Page {
                width: Object::Real(192.125),
                height: Object::Real(160.25),
                fonts: vec![],
                graphics: vec![],
                images: vec![image],
                program: crate::redaction_content::Program {
                    operations: Content::decode_strict(program.as_bytes())
                        .unwrap()
                        .operations,
                    blocks: vec![],
                },
            };
            let mut bytes = crate::redaction_core::publish(vec![page]).unwrap();
            let mut before_name = format!("{name}-source.pdf");
            save(&before_name, &bytes);
            let mut cumulative = vec![0_u8; (header.columns * header.rows) as usize];
            for (label, column) in [("prior", 1_u32), ("actual", 2)] {
                let u = (f64::from(column) + 0.5) * f64::from(header.mcu_width)
                    / f64::from(header.width);
                let v = 1. - 1.5 * f64::from(header.mcu_height) / f64::from(header.height);
                let rect = transform
                    .unit_bounds(AreaRect {
                        left: u,
                        bottom: v,
                        right: u + 0.0001,
                        top: v + 0.0001,
                    })
                    .unwrap();
                let parsed = crate::redaction_numbers::load_pdf(&bytes).unwrap();
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
                assert!(result.paths.is_empty());
                assert_eq!(result.removed_characters, 0);
                let selected = result.images[0].jpeg.as_ref().unwrap();
                let mask = selected.coverage.mask().to_vec();
                assert_eq!(mask.iter().filter(|&&v| v == 1).count(), 1);
                for (a, b) in cumulative.iter_mut().zip(&mask) {
                    *a |= *b;
                }
                let after_name = format!("{name}-{label}.pdf");
                save(&after_name, &result.bytes);
                for device in [[64, 53], [96, 80], [192, 160], [240, 200], [576, 480]] {
                    let bound = selected
                        .page_influence(device, &mut CoverageBudget::default())
                        .unwrap_or_else(|e| panic!("{name} {label} {device:?}: {e:?}"));
                    records.push(serde_json::json!({"name":name,"stage":label,"source":before_name,"actual":after_name,"page":[192.125,160.25],"shape":[shape.0,shape.1,shape.2],"mcu":[header.mcu_width,header.mcu_height],"grid":[header.columns,header.rows],"steps":steps,"selected":mask,"cumulative":cumulative,"device":device,"pdfium":bound[0],"mupdf":bound[1]}));
                }
                assert_eq!(selected.coverage.mask(), mask);
                let reopened = crate::redaction_numbers::load_pdf(&result.bytes).unwrap();
                let prepared = crate::redaction_core::prepare_area_with_context(
                    &reopened,
                    &mut ImageContext::for_jpeg_validation(),
                )
                .unwrap();
                assert_eq!(
                    prepared.pages[0].images[0].jpeg_coverage().unwrap().mask(),
                    cumulative
                );
                // Reopen the exact previous private rectangle grammar and
                // upgrade it to the same canonical triangle publication.
                let cover = prepared.pages[0].images[0].jpeg_coverage().unwrap();
                let current = cover.paint(&mut CoverageBudget::default()).unwrap();
                let legacy = cover.legacy_paint(&mut CoverageBudget::default()).unwrap();
                let mut old_doc = crate::redaction_numbers::load_pdf(&result.bytes).unwrap();
                let mut old_image = prepared.pages[0].images[0].stream().unwrap();
                let mut bare = old_image.clone();
                bare.dict.remove(b"FolioJpegRedaction");
                let mut bare_doc = Document::new();
                let bare_id = bare_doc.add_object(bare);
                let base_image = Registry::default()
                    .load(&bare_doc, &Object::Reference(bare_id))
                    .unwrap();
                let record = old_image
                    .dict
                    .get_mut(b"FolioJpegRedaction")
                    .unwrap()
                    .as_dict_mut()
                    .unwrap();
                record.set("V", 1);
                record.set(
                    "Binding",
                    Object::String(
                        cover.binding(&base_image.digest).to_vec(),
                        lopdf::StringFormat::Hexadecimal,
                    ),
                );
                let image_id = *old_doc
                    .objects
                    .iter()
                    .find(|(_, value)| {
                        value.as_stream().ok().is_some_and(|s| {
                            s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image")
                        })
                    })
                    .unwrap()
                    .0;
                old_doc.objects.insert(image_id, Object::Stream(old_image));
                let page_id = old_doc.get_pages()[&1];
                let mut old_ops = Content::decode_strict(&old_doc.get_page_content(page_id))
                    .unwrap()
                    .operations;
                let start = old_ops.iter().position(|op| op.operator == "Do").unwrap() + 1;
                old_ops.splice(
                    start..start + current.operations().len(),
                    legacy.operations().iter().cloned(),
                );
                let stream = old_doc.add_object(Stream::new(
                    lopdf::Dictionary::new(),
                    Content {
                        operations: old_ops,
                    }
                    .encode()
                    .unwrap(),
                ));
                old_doc
                    .get_object_mut(page_id)
                    .unwrap()
                    .as_dict_mut()
                    .unwrap()
                    .set("Contents", stream);
                let mut legacy_bytes = vec![];
                old_doc.save_to(&mut legacy_bytes).unwrap();
                save(&format!("{name}-{label}-legacy.pdf"), &legacy_bytes);
                let old_prepared = crate::redaction_core::prepare_area_with_context(
                    &crate::redaction_numbers::load_pdf(&legacy_bytes).unwrap(),
                    &mut ImageContext::for_jpeg_validation(),
                )
                .unwrap();
                assert!(old_prepared.pages[0].images[0].jpeg_needs_paint_upgrade());
                let preserved = crate::redaction_core::publish(old_prepared.pages).unwrap();
                let unchanged = crate::redaction_numbers::load_pdf(&preserved).unwrap();
                assert_eq!(
                    unchanged.get_page_content(unchanged.get_pages()[&1]),
                    old_doc.get_page_content(page_id)
                );
                let upgraded = remove_planned_content_with_context(
                    &unchanged,
                    &[],
                    &[PageAreaMark {
                        page_index: 0,
                        rect,
                    }],
                    &mut ImageContext::for_jpeg_validation(),
                )
                .unwrap();
                assert_eq!(upgraded.bytes, result.bytes);
                assert_eq!(
                    crate::redaction_core::publish(prepared.pages).unwrap(),
                    result.bytes
                );
                bytes = result.bytes;
                before_name = after_name;
            }
        }
    }
    assert_eq!(records.len(), 280);
    save("cases.json", &serde_json::to_vec_pretty(&records).unwrap());
}
