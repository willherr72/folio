use super::*;
use crate::{
    redaction_images::ImageContext,
    redaction_jpeg::{inspect, Transform},
    redaction_jpeg_coverage::CoverageBudget,
};
use lopdf::{dictionary, Document, Object, Stream};

fn save(name: &str, bytes: &[u8]) {
    if let Some(root) = std::env::var_os("FOLIO_JPEG_TRANSACTION_OUTPUT") {
        use std::io::Write;
        let root = PathBuf::from(root).join("coverage-render");
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

fn original_pdf(
    input: &[u8],
    shape: (u32, u32, u32),
    color: Option<i64>,
    steps: &[[f32; 6]],
) -> Vec<u8> {
    let mut doc = Document::new();
    let mut image = Stream::new(
        dictionary! {"Type"=>"XObject","Subtype"=>"Image","Width"=>shape.0 as i64,"Height"=>shape.1 as i64,"BitsPerComponent"=>8,"ColorSpace"=>if shape.2==1 {"DeviceGray"} else {"DeviceRGB"},"Filter"=>"DCTDecode"},
        input.to_vec(),
    );
    if let Some(value) = color {
        image
            .dict
            .set("DecodeParms", dictionary! {"ColorTransform"=>value});
    }
    let image = doc.add_object(image);
    let mut program = String::from("q ");
    for step in steps {
        program.push_str(
            &step
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" "),
        );
        program.push_str(" cm ");
    }
    program.push_str("/Original Do Q");
    let content = doc.add_object(Stream::new(lopdf::Dictionary::new(), program.into_bytes()));
    let pages = doc.new_object_id();
    let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![Object::Integer(0),Object::Integer(0),Object::Real(192.125),Object::Real(160.25)],"Resources"=>dictionary!{"XObject"=>dictionary!{"Original"=>image}},"Contents"=>content});
    doc.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into(),
    );
    let root = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    doc.trailer.set("Root", root);
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    bytes
}

#[test]
fn redaction_area_jpeg_color_edge_and_hole_transactions_keep_original_source_masks() {
    let fixtures: &[(&str, &[u8], (u32, u32, u32), Option<i64>)] = &[
        (
            "rgb-ycbcr",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub2-progressive1.jpg"),
            (64, 48, 3),
            Some(1),
        ),
        (
            "rgb-direct",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub2-progressive1.jpg"),
            (64, 48, 3),
            Some(0),
        ),
        (
            "partial-ycbcr",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-rgb-sub2-progressive1.jpg"
            ),
            (65, 49, 3),
            Some(1),
        ),
        (
            "partial-direct",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-rgb-sub2-progressive1.jpg"
            ),
            (65, 49, 3),
            Some(0),
        ),
        (
            "gray-edge",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-19x17-gray-sub0-progressive1.jpg"
            ),
            (19, 17, 1),
            None,
        ),
        (
            "large",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-large-owned.jpg"),
            (256, 192, 3),
            Some(1),
        ),
    ];
    let layouts = [
        ("axis", [128., 0., 0., 96., 20., 24.]),
        ("fractional", [123.5, 0., 0., 95.25, 21.125, 22.5]),
        ("rotate", [0., 128., -96., 0., 116., 16.]),
        ("reflect", [-128., 0., 0., 96., 156., 24.]),
        ("shear", [100., 10., 25., 90., 20., 20.]),
        ("nested", [64., 0., 0., 48., 0., 0.]),
        ("small", [8., 0., 0., 6., 40., 40.]),
    ];
    let mut records = vec![];
    for &(fixture, input, shape, color) in fixtures {
        let transform = match color {
            Some(0) => Transform::Rgb,
            Some(1) => Transform::YCbCr,
            _ => Transform::Default,
        };
        let header = inspect(input, shape, transform).unwrap();
        for (layout, matrix) in layouts {
            let mut steps = vec![];
            if layout == "nested" {
                steps.extend([[1.1, 0.2, 0.3, 0.9, 35., 25.], [0.7, 0.1, 0.2, 1.3, 4., 5.]]);
            }
            steps.push(matrix);
            let mut placement = crate::redaction_area_paths::ImageTransform::default();
            for step in &steps {
                placement = placement.concat(step.map(f64::from)).unwrap();
            }
            let columns = header.columns;
            let rows = header.rows;
            let last = columns * rows - 1;
            for pattern in ["edge", "disjoint", "ring"] {
                let name = format!("{fixture}-{layout}-{pattern}");
                let choices: [Vec<u32>; 2] = match pattern {
                    "edge" => [vec![last], vec![0]],
                    "disjoint" => [vec![0, last], vec![columns - 1, columns * (rows - 1)]],
                    _ => [
                        (0..3)
                            .flat_map(|y| {
                                (0..3)
                                    .filter(move |&x| x != 1 || y != 1)
                                    .map(move |x| y * columns + x)
                            })
                            .collect(),
                        vec![columns + 1],
                    ],
                };
                let mut bytes = original_pdf(input, shape, color, &steps);
                let original = bytes.clone();
                let mut before = format!("{name}-source.pdf");
                save(&before, &bytes);
                let mut cumulative = vec![0_u8; (columns * rows) as usize];
                for (stage, indices) in ["prior", "actual"].into_iter().zip(choices) {
                    let mut mask = vec![0_u8; cumulative.len()];
                    let mut marks = vec![];
                    for index in indices {
                        mask[index as usize] = 1;
                        let x = (index % columns) * header.mcu_width;
                        let y = (index / columns) * header.mcu_height;
                        let u = f64::from(x + (x + header.mcu_width).min(shape.0))
                            / 2.
                            / f64::from(shape.0);
                        let v = 1.
                            - f64::from(y + (y + header.mcu_height).min(shape.1))
                                / 2.
                                / f64::from(shape.1);
                        marks.push(PageAreaMark {
                            page_index: 0,
                            rect: placement
                                .unit_bounds(AreaRect {
                                    left: u,
                                    bottom: v,
                                    right: u + 0.0001,
                                    top: v + 0.0001,
                                })
                                .unwrap(),
                        });
                    }
                    let doc = crate::redaction_numbers::load_pdf(&bytes).unwrap();
                    let result = remove_planned_content_with_context(
                        &doc,
                        &[],
                        &marks,
                        &mut ImageContext::for_jpeg_validation(),
                    )
                    .unwrap_or_else(|e| panic!("{name} {stage}: {e:?}"));
                    assert_eq!(result.images.len(), 1);
                    assert!(result.paths.is_empty());
                    assert_eq!(result.removed_characters, 0);
                    let selected = result.images[0].jpeg.as_ref().unwrap();
                    // Source selection conservatively intersects each transformed
                    // pixel's page-aligned bounds. A one-pixel partial MCU under
                    // shear can therefore include a neighboring block. Check an
                    // exhaustive oracle instead of assuming the requested cells
                    // are the entire result of the recursive pixel planner.
                    let requested = mask;
                    let mut mask = vec![0_u8; cumulative.len()];
                    for y in 0..shape.1 {
                        for x in 0..shape.0 {
                            let cell = placement
                                .pixel_bounds([x, y, x + 1, y + 1], shape.0, shape.1)
                                .unwrap();
                            if marks.iter().any(|m| m.rect.touches(cell)) {
                                mask[((y / header.mcu_height) * columns + x / header.mcu_width)
                                    as usize] = 1;
                            }
                        }
                    }
                    assert!(requested.iter().zip(&mask).all(|(a, b)| *a == 0 || *b == 1));
                    assert_eq!(selected.coverage.mask(), mask, "{name} {stage}");
                    assert_eq!(selected.header.transformed, header.transformed);
                    for (a, b) in cumulative.iter_mut().zip(&mask) {
                        *a |= *b;
                    }
                    let after = format!("{name}-{stage}.pdf");
                    save(&after, &result.bytes);
                    for device in [[64, 53], [192, 160], [576, 480]] {
                        let bounds = selected
                            .page_influence(device, &mut CoverageBudget::default())
                            .unwrap_or_else(|e| panic!("{name} {stage} {device:?}: {e:?}"));
                        let mark_rects: Vec<_> = marks
                            .iter()
                            .map(|m| [m.rect.left, m.rect.bottom, m.rect.right, m.rect.top])
                            .collect();
                        records.push(serde_json::json!({"name":name,"fixture":fixture,"pattern":pattern,"stage":stage,"source":before,"actual":after,"original":format!("{name}-source.pdf"),"page":[192.125,160.25],"shape":[shape.0,shape.1,shape.2],"mcu":[header.mcu_width,header.mcu_height],"grid":[columns,rows],"transformed":header.transformed,"sourceColorTransform":color,"steps":steps,"marks":mark_rects,"requested":requested,"selected":mask,"cumulative":cumulative,"device":device,"pdfium":bounds[0],"mupdf":bounds[1]}));
                    }
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
                    assert_eq!(
                        crate::redaction_core::publish(prepared.pages).unwrap(),
                        result.bytes
                    );
                    bytes = result.bytes;
                    before = after;
                }
                assert_eq!(original, original_pdf(input, shape, color, &steps));
            }
        }
    }
    assert_eq!(records.len(), 756);
    save("cases.json", &serde_json::to_vec_pretty(&records).unwrap());
}
