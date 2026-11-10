use super::*;
use lopdf::{content::Content, Object};

fn header(width: u32, height: u32, mcu_width: u32, mcu_height: u32) -> Header {
    Header {
        width,
        height,
        components: 3,
        mcu_width,
        mcu_height,
        columns: width.div_ceil(mcu_width),
        rows: height.div_ceil(mcu_height),
        scans: 1,
        transformed: true,
        coefficient_work: 1,
    }
}
fn coverage(h: &Header, mask: &[u8]) -> Coverage {
    Coverage::from_mask(h, mask, &mut CoverageBudget::default()).unwrap()
}

#[test]
fn redaction_jpeg_ring_paint_merges_collinear_outer_and_hole_edges() {
    let c = coverage(&header(48, 48, 16, 16), &[1, 1, 1, 1, 0, 1, 1, 1, 1]);
    let p = c.paint(&mut CoverageBudget::default()).unwrap();
    assert_eq!(
        p.operations().len(),
        20,
        "two closed rectangular contours, with opposite winding, split into triangles"
    );
    assert_eq!(c.mask(), &[1, 1, 1, 1, 0, 1, 1, 1, 1]);
}

#[test]
fn redaction_jpeg_contours_keep_holes_and_diagonal_contacts_for_every_small_mask() {
    for bits in 0..512_u32 {
        let mask: Vec<_> = (0..9).map(|i| ((bits >> i) & 1) as u8).collect();
        let c = coverage(&header(19, 17, 8, 8), &mask);
        let p = c.paint(&mut CoverageBudget::default()).unwrap();
        let mut paths = Vec::<Vec<[f64; 2]>>::new();
        for op in p.operations() {
            if op.operator == "m" {
                paths.push(vec![]);
            }
            if op.operator == "m" || op.operator == "l" {
                paths.last_mut().unwrap().push(
                    op.operands
                        .iter()
                        .map(|v| crate::redaction_content::number(v).unwrap())
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap(),
                );
            }
        }
        for y in 0..17 {
            for x in 0..19 {
                let point = [(x as f64 + 0.5) / 19., 1. - (y as f64 + 0.5) / 17.];
                let mut winding = 0;
                for path in &paths {
                    for (a, b) in path
                        .iter()
                        .zip(path.iter().cycle().skip(1))
                        .take(path.len())
                    {
                        let cross =
                            (b[0] - a[0]) * (point[1] - a[1]) - (point[0] - a[0]) * (b[1] - a[1]);
                        if a[1] <= point[1] && b[1] > point[1] && cross > 0. {
                            winding += 1;
                        }
                        if b[1] <= point[1] && a[1] > point[1] && cross < 0. {
                            winding -= 1;
                        }
                    }
                }
                assert_eq!(
                    winding != 0,
                    mask[(y / 8) * 3 + x / 8] != 0,
                    "mask {bits} pixel {x},{y}"
                );
            }
        }
        assert_eq!(c.mask(), mask);
    }
    let c = coverage(&header(16, 16, 16, 16), &[1]);
    let mut budget = CoverageBudget {
        boundary_edges: 99_997,
        ..Default::default()
    };
    assert!(c.paint(&mut budget).is_err());
    assert_eq!(budget.boundary_edges, 100_000);
}

#[test]
fn redaction_jpeg_maximum_source_envelope_covers_higher_resolution_cache_choices() {
    for (w, h) in [(19, 17), (65, 49), (256, 192), (32768, 33)] {
        for (mw, mh) in [(8, 8), (8, 16), (16, 8), (16, 16)] {
            let head = header(w, h, mw, mh);
            let mut mask = vec![0; (head.columns * head.rows) as usize];
            let first = mask.len() / 3;
            mask[first] = 1;
            *mask.last_mut().unwrap() = 1;
            let c = coverage(&head, &mask);
            let binding = c.binding(&[7; 32]);
            for maximum in [1, 2, 4, 8, 16, 32, 64] {
                let broad = c
                    .source_envelope(maximum, &mut CoverageBudget::default())
                    .unwrap();
                for d in [1, 2, 4, 8] {
                    for f in [1, 2, 4, 8, 16, 32, 64]
                        .into_iter()
                        .filter(|f| d * f <= maximum)
                    {
                        let narrow = c
                            .decoded_source_envelope(d, [1, 1], f, &mut CoverageBudget::default())
                            .unwrap();
                        assert_eq!(broad.cells().len(), narrow.cells().len());
                        for (a, b) in broad.cells().iter().zip(narrow.cells()) {
                            assert!(a[0] <= b[0] && a[1] <= b[1] && a[2] >= b[2] && a[3] >= b[3]);
                        }
                    }
                }
            }
            assert_eq!(c.binding(&[7; 32]), binding);
            assert_eq!(c.mask(), mask);
        }
    }
    let c = coverage(&header(16, 16, 16, 16), &[1]);
    let mut budget = CoverageBudget::default();
    for maximum in [0, 3, 128, u32::MAX] {
        assert!(c.source_envelope(maximum, &mut budget).is_err());
    }
    assert_eq!((budget.cells, budget.rectangles), (0, 0));
}

#[test]
fn redaction_jpeg_crop_keeps_decoder_halo_before_aligned_window_intersection() {
    let mut mask = vec![0; 20];
    mask[6] = 1;
    mask[19] = 1;
    let c = coverage(&header(65, 49, 16, 16), &mask);
    let binding = c.binding(&[4; 32]);
    let grid = c
        .decoded_window_samples(2, [1, 1], [32, 16, 65, 49], &mut CoverageBudget::default())
        .unwrap();
    assert_eq!(grid.dimensions(), (17, 17));
    assert_eq!(grid.cells(), &[[0, 0, 1, 9], [15, 15, 17, 17]]);
    assert_eq!(c.binding(&[4; 32]), binding);
    assert_eq!(c.mask(), mask);
}

#[test]
fn redaction_jpeg_crop_invalid_alignment_refuses_before_spending_budget() {
    let c = coverage(&header(65, 49, 16, 16), &[0; 20]);
    let mut budget = CoverageBudget::default();
    for window in [
        [1, 8, 65, 49],
        [8, 1, 65, 49],
        [8, 8, 63, 49],
        [8, 8, 65, 47],
        [0, 0, 66, 49],
        [8, 8, 8, 16],
    ] {
        assert!(c
            .decoded_window_samples(4, [0, 0], window, &mut budget)
            .is_err());
    }
    assert_eq!(
        (budget.cells, budget.rectangles, budget.operations),
        (0, 0, 0)
    );
}

#[test]
fn redaction_jpeg_decoded_regions_use_actual_partial_grid_dimensions() {
    let mut mask = vec![0; 20];
    mask[19] = 1;
    let c = coverage(&header(65, 49, 16, 16), &mask);
    let binding = c.binding(&[3; 32]);
    let regions = c
        .decoded_unit_regions(8, [0, 0], &mut CoverageBudget::default())
        .unwrap();
    assert_eq!(regions.len(), 1);
    let r = regions[0];
    assert!(r.left <= 8. / 9. && r.left >= (8.0_f64 / 9.).next_down());
    assert_eq!((r.bottom, r.right), (0., 1.));
    assert!(r.top >= 1. / 7. && r.top <= (1.0_f64 / 7.).next_up());
    // Merely expanding the original MCU by zero at 1/8 decode would start at
    // 64/65 and miss most of the changed last decoded pixel's unit footprint.
    assert!(r.left < 64. / 65.);
    assert_eq!(c.binding(&[3; 32]), binding);
    assert_eq!(c.mask(), mask);
}

#[test]
fn redaction_jpeg_decoded_regions_expand_taps_before_unit_normalization() {
    let c = coverage(&header(19, 17, 16, 16), &[0, 0, 0, 1]);
    let regions = c
        .decoded_unit_regions(4, [1, 1], &mut CoverageBudget::default())
        .unwrap();
    let r = regions[0];
    // 5x5 decoded grid: the last MCU is cell [4,4,5,5]. One tap of support
    // expands this to [3,3,5,5], then image y is converted to bottom origin.
    assert!(r.left <= 0.6 && r.left >= 0.6_f64.next_down());
    assert_eq!((r.bottom, r.right), (0., 1.));
    assert!(r.top >= 0.4 && r.top <= 0.4_f64.next_up());
}

#[test]
fn redaction_jpeg_decoded_regions_refuse_unsupported_parameters_without_spending_budget() {
    let c = coverage(&header(16, 16, 16, 16), &[1]);
    let mut budget = CoverageBudget::default();
    for denominator in [0, 3, 5, 16, u32::MAX] {
        assert!(c
            .decoded_unit_regions(denominator, [0, 0], &mut budget)
            .is_err());
    }
    for halo in [[2, 0], [0, 2], [u32::MAX, 1]] {
        assert!(c.decoded_unit_regions(1, halo, &mut budget).is_err());
    }
    assert_eq!(
        (budget.cells, budget.rectangles, budget.operations),
        (0, 0, 0)
    );
    let empty = coverage(&header(16, 16, 16, 16), &[0]);
    assert!(empty.decoded_unit_regions(0, [0, 0], &mut budget).is_err());
    assert!(empty
        .decoded_unit_regions(1, [0, 0], &mut budget)
        .unwrap()
        .is_empty());
}

#[test]
fn redaction_jpeg_disclosure_preserves_disjoint_partial_coverage_and_paint() {
    let c = coverage(
        &header(81, 49, 16, 16),
        &[
            1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
        ],
    );
    let binding = c.binding(&[17; 32]);
    let mask = c.mask().to_vec();
    let paint = c.paint(&mut CoverageBudget::default()).unwrap();
    let encoded = Content {
        operations: paint.operations().to_vec(),
    }
    .encode()
    .unwrap();
    let matrix = ImageTransform::default()
        .concat([162., 0., 0., 98., 20., 30.])
        .unwrap();
    let original = paint.page_bounds(matrix).unwrap();
    let disclosed = paint
        .disclosure_bounds(matrix, [0.01, 0.02], [2., 3.])
        .unwrap();
    assert_eq!(disclosed.len(), 2);
    for (before, after) in original.iter().zip(&disclosed) {
        assert!(after.contains(*before));
        assert!(after.left < before.left && after.right > before.right);
        assert!(after.bottom < before.bottom && after.top > before.top);
    }
    assert!(!disclosed[0].touches(disclosed[1]));
    assert_eq!(
        paint.disclosure_bounds(matrix, [0.; 2], [0.; 2]).unwrap(),
        original
    );
    assert_eq!(c.mask(), mask);
    assert_eq!(c.binding(&[17; 32]), binding);
    assert_eq!(
        Content {
            operations: paint.operations().to_vec()
        }
        .encode()
        .unwrap(),
        encoded
    );
}

#[test]
fn redaction_jpeg_disclosure_validates_even_empty_coverage() {
    let empty = coverage(&header(16, 16, 16, 16), &[0])
        .paint(&mut CoverageBudget::default())
        .unwrap();
    let matrix = ImageTransform::default();
    assert!(empty
        .disclosure_bounds(matrix, [0.; 2], [0.; 2])
        .unwrap()
        .is_empty());
    for bad in [-1., f64::NAN, f64::INFINITY, 1_000_001.] {
        assert!(empty.disclosure_bounds(matrix, [bad, 0.], [0.; 2]).is_err());
        assert!(empty.disclosure_bounds(matrix, [0.; 2], [0., bad]).is_err());
    }
}
#[test]
fn redaction_jpeg_coverage_expands_only_original_pixels_and_clips_final_blocks() {
    let h = header(19, 17, 16, 8);
    let mut pixels = vec![false; 19 * 17];
    pixels[1 * 19 + 1] = true;
    pixels[16 * 19 + 18] = true;
    let c = Coverage::from_pixels(&h, &pixels, &mut CoverageBudget::default()).unwrap();
    assert_eq!(c.mask(), &[1, 0, 0, 0, 0, 1]);
    assert_eq!(
        c.rectangles(&mut CoverageBudget::default()).unwrap(),
        vec![[0, 0, 16, 8], [16, 16, 19, 17]]
    );
    assert_eq!(pixels.iter().filter(|v| **v).count(), 2);
}
#[test]
fn redaction_jpeg_coverage_reedit_keeps_prior_disjoint_coverage() {
    let h = header(48, 32, 16, 16);
    let old = coverage(&h, &[1, 0, 0, 0, 0, 1]);
    let later = coverage(&h, &[0, 1, 0, 0, 0, 0]);
    let both = old.union(&later, &mut CoverageBudget::default()).unwrap();
    assert_eq!(both.mask(), &[1, 1, 0, 0, 0, 1]);
    assert_eq!(old.mask(), &[1, 0, 0, 0, 0, 1]);
    assert_eq!(
        both.rectangles(&mut CoverageBudget::default()).unwrap(),
        vec![[0, 0, 32, 16], [32, 16, 48, 32]]
    );
    assert!(old
        .union(
            &coverage(&header(48, 31, 16, 16), &[1, 0, 0, 0, 0, 0]),
            &mut CoverageBudget::default()
        )
        .is_err());
}
#[test]
fn redaction_jpeg_coverage_coalesces_full_runs_without_filling_holes() {
    let h = header(48, 48, 16, 16);
    let full = coverage(&h, &[1; 9]);
    assert_eq!(
        full.rectangles(&mut CoverageBudget::default()).unwrap(),
        vec![[0, 0, 48, 48]]
    );
    let ring = coverage(&h, &[1, 1, 1, 1, 0, 1, 1, 1, 1]);
    let rects = ring.rectangles(&mut CoverageBudget::default()).unwrap();
    assert_eq!(
        rects,
        vec![
            [0, 0, 48, 16],
            [0, 16, 16, 32],
            [32, 16, 48, 32],
            [0, 32, 48, 48]
        ]
    );
    let paint = ring
        .paint(&mut CoverageBudget::default())
        .unwrap()
        .operations;
    assert_eq!(paint.iter().filter(|op| op.operator == "f").count(), 1);
    assert_eq!(paint.first().unwrap().operator, "q");
    assert_eq!(paint[1].operator, "g");
    assert_eq!(paint[1].operands, vec![Object::Integer(0)]);
    assert_eq!(paint.last().unwrap().operator, "Q");
    assert!(coverage(&h, &[0; 9])
        .paint(&mut CoverageBudget::default())
        .unwrap()
        .operations
        .is_empty());
}
#[test]
fn redaction_jpeg_coverage_serialized_fill_contains_nonbinary_pixel_edges() {
    // Upper left MCU: [0,16] x [0,8] in a 19x17 image. The PDF has a bottom origin.
    let c = coverage(&header(19, 17, 16, 8), &[1, 0, 0, 0, 0, 0]);
    let ops = c.paint(&mut CoverageBudget::default()).unwrap().operations;
    let bytes = Content { operations: ops }.encode().unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    let mut vertices = vec![];
    for line in text.lines() {
        let parts: Vec<_> = line.split_whitespace().collect();
        if parts.last().is_some_and(|v| *v == "m" || *v == "l") {
            vertices.push((
                parts[0].parse::<f64>().unwrap(),
                parts[1].parse::<f64>().unwrap(),
            ));
        }
    }
    assert_eq!(vertices.len(), 6);
    assert_eq!(vertices[0], vertices[3]);
    assert_eq!(vertices[2], vertices[4]);
    let (left, bottom) = vertices[0];
    let (right, top) = vertices[2];
    assert_eq!(left, 0.0);
    assert_eq!(top, 1.0);
    assert!(bottom <= 9.0 / 17.0 && bottom > 9.0 / 17.0 - 0.000001);
    assert!(right >= 16.0 / 19.0 && right < 16.0 / 19.0 + 0.000001);
    // Check serialized coordinates, not merely the in-memory float values.
    assert!(Content::decode(&bytes).is_ok());
}
#[test]
fn redaction_jpeg_coverage_rejects_bad_shapes_masks_and_mismatched_pixel_counts() {
    let h = header(19, 17, 16, 8);
    for mask in [vec![0; 5], vec![0; 7], vec![2; 6]] {
        assert!(Coverage::from_mask(&h, &mask, &mut CoverageBudget::default()).is_err());
    }
    assert!(Coverage::from_pixels(&h, &[true; 19], &mut CoverageBudget::default()).is_err());
    for bad in [
        Header {
            columns: 3,
            ..h.clone()
        },
        Header {
            width: 0,
            ..h.clone()
        },
        Header {
            mcu_height: 7,
            ..h.clone()
        },
        Header {
            height: 32769,
            ..h.clone()
        },
    ] {
        assert!(Coverage::from_mask(&bad, &[1; 6], &mut CoverageBudget::default()).is_err());
    }
}

#[test]
fn redaction_jpeg_coverage_limits_are_shared_across_occurrences_before_allocation() {
    let h = header(16, 16, 16, 16);
    let c = coverage(&h, &[1]);
    let mut budget = CoverageBudget {
        pixels: 64_000_000 - 256,
        ..Default::default()
    };
    assert!(Coverage::from_pixels(&h, &[true; 256], &mut budget).is_ok());
    assert!(Coverage::from_pixels(&h, &[true; 256], &mut budget).is_err());
    let mut budget = CoverageBudget {
        masks: 3_999_999,
        ..Default::default()
    };
    assert!(Coverage::from_mask(&h, &[1], &mut budget).is_ok());
    assert!(c.union(&c, &mut budget).is_err());
    let mut budget = CoverageBudget {
        rectangles: 4095,
        ..Default::default()
    };
    assert!(c.paint(&mut budget).is_ok());
    assert!(c.paint(&mut budget).is_err());
    let mut budget = CoverageBudget {
        operations: 99_988,
        ..Default::default()
    };
    assert!(c.paint(&mut budget).is_ok());
    assert!(c.paint(&mut budget).is_err());
    let mut budget = CoverageBudget {
        cells: 8_000_000,
        ..Default::default()
    };
    assert!(Coverage::from_mask(&h, &[1], &mut budget).is_err());
    assert!(c.rectangles(&mut budget).is_err());
    // A checkerboard produces >4096 rectangles despite a small encoded image.
    let h = header(1032, 1032, 8, 8);
    let mask: Vec<_> = (0..129 * 129)
        .map(|i| ((i / 129 + i % 129) % 2) as u8)
        .collect();
    assert!(coverage(&h, &mask)
        .paint(&mut CoverageBudget::default())
        .is_err());
}

#[test]
fn redaction_jpeg_coverage_decimal_and_float_edges_are_outward_at_dimension_limits() {
    for d in 1u32..=32768 {
        for n in [0, 1, d / 3, d / 2, d.saturating_sub(1), d] {
            for upper in [false, true] {
                let value = edge(n, d, upper);
                let decimal = value.to_string().parse::<f64>().unwrap();
                let exact = f64::from(n) / f64::from(d);
                for actual in [f64::from(value), decimal] {
                    assert!((0.0..=1.0).contains(&actual));
                    assert!(
                        if upper {
                            actual >= exact
                        } else {
                            actual <= exact
                        },
                        "{n}/{d}, {upper}: {actual}"
                    );
                    assert!((actual - exact).abs() < 0.000001);
                }
            }
        }
    }
}

#[test]
fn redaction_jpeg_coverage_paint_disclosure_encloses_serialized_affine_geometry() {
    let h = header(64, 48, 16, 16);
    let c = coverage(&h, &[0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    let paint = c.paint(&mut CoverageBudget::default()).unwrap();
    for (matrix, want) in [
        ([64., 0., 0., 48., 10., 20.], [26., 52., 42., 68.]),
        ([0., 64., -48., 0., 100., 10.], [52., 26., 68., 42.]),
        ([64., 0., 24., 48., 10., 20.], [42., 52., 66., 68.]),
        ([-64., 0., 0., 48., 100., 20.], [68., 52., 84., 68.]),
    ] {
        let bounds = paint
            .page_bounds(ImageTransform::default().concat(matrix).unwrap())
            .unwrap();
        assert_eq!(bounds.len(), 1);
        let actual = [
            bounds[0].left,
            bounds[0].bottom,
            bounds[0].right,
            bounds[0].top,
        ];
        for i in 0..4 {
            assert!(
                if i < 2 {
                    actual[i] <= want[i]
                } else {
                    actual[i] >= want[i]
                },
                "{actual:?}"
            );
            assert!((actual[i] - want[i]).abs() < 0.0001);
        }
    }
    assert!(coverage(&h, &[0; 12])
        .paint(&mut CoverageBudget::default())
        .unwrap()
        .page_bounds(ImageTransform::default())
        .unwrap()
        .is_empty());
}

fn saved_coverage_pdf(
    image: lopdf::Stream,
    matrix: [f32; 6],
    paint: &Paint,
    first: bool,
) -> Vec<u8> {
    use lopdf::{dictionary, Document, Stream};
    let mut pdf = Document::with_version("1.7");
    let tree = pdf.new_object_id();
    let image = pdf.add_object(image);
    let mut operations = vec![
        Operation::new("q", vec![]),
        Operation::new("cm", matrix.map(Object::Real).to_vec()),
    ];
    if first {
        operations.extend_from_slice(paint.operations());
    }
    operations.push(Operation::new("Do", vec![Object::Name(b"Im".to_vec())]));
    if !first {
        operations.extend_from_slice(paint.operations());
    }
    operations.push(Operation::new("Q", vec![]));
    let content = pdf.add_object(Stream::new(
        dictionary! {},
        Content { operations }.encode().unwrap(),
    ));
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),192.into(),160.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"Im"=>image}},"Contents"=>content});
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
fn redaction_jpeg_coverage_actual_paint_corpus_keeps_earlier_coverage_on_second_edit() {
    use crate::{redaction_images::Registry, redaction_jpeg::Limits};
    use lopdf::{dictionary, Document, Stream};
    let source = include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub2-progressive1.jpg");
    let mut pdf = Document::with_version("1.7");
    let image=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>64,"Height"=>48,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8,"Filter"=>"DCTDecode"},source.to_vec()));
    let source_image = Registry::default()
        .load(&pdf, &Object::Reference(image))
        .unwrap();
    let h = source_image.jpeg_header().unwrap();
    let mut pixels = vec![false; 64 * 48];
    pixels[20 * 64 + 20] = true;
    let old = Coverage::from_pixels(h, &pixels, &mut CoverageBudget::default()).unwrap();
    pixels.fill(false);
    pixels[20 * 64 + 36] = true;
    let new = Coverage::from_pixels(h, &pixels, &mut CoverageBudget::default()).unwrap();
    let union = old.union(&new, &mut CoverageBudget::default()).unwrap();
    let prior = source_image
        .sanitized_jpeg(old.mask(), Limits::default())
        .unwrap();
    let actual = prior
        .sanitized_jpeg(union.mask(), Limits::default())
        .unwrap();
    let one_pass = source_image
        .sanitized_jpeg(union.mask(), Limits::default())
        .unwrap();
    assert_eq!(actual.digest, one_pass.digest);
    assert_eq!(
        actual.stream().unwrap().content,
        one_pass.stream().unwrap().content
    );
    assert_eq!(
        pdf.get_object(image).unwrap().as_stream().unwrap().content,
        source
    );
    let empty = coverage(h, &[0; 12])
        .paint(&mut CoverageBudget::default())
        .unwrap();
    let old_paint = old.paint(&mut CoverageBudget::default()).unwrap();
    let new_paint = new.paint(&mut CoverageBudget::default()).unwrap();
    let union_paint = union.paint(&mut CoverageBudget::default()).unwrap();
    for (name, matrix) in [
        ("axis", [128., 0., 0., 96., 20., 24.]),
        ("rotate", [0., 128., -96., 0., 116., 16.]),
        ("reflect", [-128., 0., 0., 96., 156., 24.]),
        ("shear", [100., 10., 25., 90., 20., 20.]),
        ("fractional", [123.5, 0., 0., 95.25, 21.125, 22.5]),
        ("small", [8., 0., 0., 6., 40., 40.]),
    ] {
        for (label, payload, paint, first) in [
            ("source", source_image.as_ref(), &empty, false),
            ("prior", &prior, &old_paint, false),
            ("actual", &actual, &union_paint, false),
            ("lost-old-fill", &actual, &new_paint, false),
            ("wrong-order", &actual, &union_paint, true),
            ("cover-only", source_image.as_ref(), &union_paint, false),
        ] {
            let saved = saved_coverage_pdf(payload.stream().unwrap(), matrix, paint, first);
            assert!(Document::load_mem(&saved).is_ok());
            if let Some(root) = std::env::var_os("FOLIO_JPEG_COVERAGE_OUTPUT") {
                let root = std::path::PathBuf::from(root);
                std::fs::create_dir_all(&root).unwrap();
                std::fs::write(root.join(format!("{name}-{label}.pdf")), saved).unwrap();
            }
        }
    }
}
