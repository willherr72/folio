use super::*;
use crate::redaction_render_transform::RendererTransform;
fn selected() -> Coverage {
    let input = include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub2-progressive1.jpg");
    let h = crate::redaction_jpeg::inspect(
        input,
        (64, 48, 3),
        crate::redaction_jpeg::Transform::Default,
    )
    .unwrap();
    let mut mask = vec![0; (h.columns * h.rows) as usize];
    mask[5] = 1;
    Coverage::from_mask(&h, &mask, &mut CoverageBudget::default()).unwrap()
}
#[test]
fn redaction_jpeg_pdfium_device_influence_keeps_selection_and_contains_decoder_choices() {
    let coverage = selected();
    let original = coverage.mask().to_vec();
    let binding = coverage.binding(&[7; 32]);
    let matrix = RendererTransform::known([8., 0., 0., -6., 0., 6.]).unwrap();
    let out = coverage
        .pdfium_influence(matrix, [8, 6], [0, 0, 8, 6], &mut CoverageBudget::default())
        .unwrap();
    for d in [1, 2, 4, 8] {
        let samples = coverage
            .decoded_samples(d, [1, 1], &mut CoverageBudget::default())
            .unwrap();
        for rect in matrix.pdfium_image_bounds(&samples, [0, 0, 8, 6]).unwrap() {
            for y in rect[1]..rect[3] {
                for x in rect[0]..rect[2] {
                    assert!(out
                        .iter()
                        .any(|r| r[0] <= x && x < r[2] && r[1] <= y && y < r[3]));
                }
            }
        }
    }
    assert_eq!(coverage.mask(), original);
    assert_eq!(coverage.binding(&[7; 32]), binding);
}
#[test]
fn redaction_jpeg_pdfium_invalid_device_and_clip_refuse() {
    let c = selected();
    let m = RendererTransform::default();
    for (size, clip) in [
        ([0, 6], [0, 0, 1, 1]),
        ([8, 6], [-1, 0, 2, 2]),
        ([8, 6], [0, 0, 9, 6]),
        ([8, 6], [2, 2, 1, 3]),
    ] {
        assert!(c
            .pdfium_influence(m, size, clip, &mut CoverageBudget::default())
            .is_err());
    }
}

#[test]
fn redaction_jpeg_pdfium_failed_work_remains_charged_and_shared() {
    let c = selected();
    let m = RendererTransform::known([8., 0., 0., -6., 0., 6.]).unwrap();
    let mut budget = CoverageBudget::default();
    budget.render_work = 1_048_575;
    assert!(c
        .pdfium_influence(m, [8, 6], [0, 0, 8, 6], &mut budget)
        .is_err());
    assert_eq!(budget.render_work, 1_048_576);
    assert_eq!(budget.render_variants, 1);
    assert!(c
        .pdfium_influence(m, [8, 6], [0, 0, 8, 6], &mut budget)
        .is_err());
    assert_eq!(budget.render_work, 1_048_576);
    let mut budget = CoverageBudget::default();
    budget.render_variants = 4096;
    assert!(c
        .pdfium_influence(m, [8, 6], [0, 0, 8, 6], &mut budget)
        .is_err());
    assert_eq!(budget.render_work, 0);
}

#[test]
fn redaction_jpeg_pdfium_partial_mcu_edges_do_not_authorize_reduction() {
    let input =
        include_bytes!("../tests/fixtures/redaction-jpeg/partial-65x49-rgb-sub2-progressive1.jpg");
    let h = crate::redaction_jpeg::inspect(
        input,
        (65, 49, 3),
        crate::redaction_jpeg::Transform::Default,
    )
    .unwrap();
    let mut mask = vec![0; (h.columns * h.rows) as usize];
    mask[6] = 1;
    let c = Coverage::from_mask(&h, &mask, &mut CoverageBudget::default()).unwrap();
    let m = RendererTransform::known([8., 0., 0., -6., 0., 6.]).unwrap();
    let mut budget = CoverageBudget::default();
    let out = c
        .pdfium_influence(m, [8, 6], [0, 0, 8, 6], &mut budget)
        .unwrap();
    let expected = m
        .pdfium_image_bounds(
            &c.decoded_samples(1, [1, 1], &mut CoverageBudget::default())
                .unwrap(),
            [0, 0, 8, 6],
        )
        .unwrap();
    assert_eq!(out, expected);
    assert_eq!(budget.render_variants, 1);
    assert!(budget.render_work > 0);
    budget.render_rectangles = 16384;
    assert!(c
        .pdfium_influence(m, [8, 6], [0, 0, 8, 6], &mut budget)
        .is_err());
}

#[test]
fn redaction_jpeg_pdfium_rewritten_corpus_exports_source_bound_influence() {
    use crate::redaction_jpeg::{inspect, rewrite, Limits, Transform};
    use std::fmt::Write;
    let fixtures: &[(&str, &[u8], (u32, u32, u32))] = &[
        (
            "gray-sub0-progressive0",
            include_bytes!("../tests/fixtures/redaction-jpeg/gray-sub0-progressive0.jpg"),
            (64, 48, 1),
        ),
        (
            "gray-sub0-progressive1",
            include_bytes!("../tests/fixtures/redaction-jpeg/gray-sub0-progressive1.jpg"),
            (64, 48, 1),
        ),
        (
            "partial-19x17-gray-sub0-progressive0",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-19x17-gray-sub0-progressive0.jpg"
            ),
            (19, 17, 1),
        ),
        (
            "partial-19x17-gray-sub0-progressive1",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-19x17-gray-sub0-progressive1.jpg"
            ),
            (19, 17, 1),
        ),
        (
            "partial-19x17-rgb-sub0-progressive0",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-19x17-rgb-sub0-progressive0.jpg"
            ),
            (19, 17, 3),
        ),
        (
            "partial-19x17-rgb-sub0-progressive1",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-19x17-rgb-sub0-progressive1.jpg"
            ),
            (19, 17, 3),
        ),
        (
            "partial-19x17-rgb-sub1-progressive0",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-19x17-rgb-sub1-progressive0.jpg"
            ),
            (19, 17, 3),
        ),
        (
            "partial-19x17-rgb-sub1-progressive1",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-19x17-rgb-sub1-progressive1.jpg"
            ),
            (19, 17, 3),
        ),
        (
            "partial-19x17-rgb-sub2-progressive0",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-19x17-rgb-sub2-progressive0.jpg"
            ),
            (19, 17, 3),
        ),
        (
            "partial-19x17-rgb-sub2-progressive1",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-19x17-rgb-sub2-progressive1.jpg"
            ),
            (19, 17, 3),
        ),
        (
            "partial-19x17-rgb-subv-progressive0",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-19x17-rgb-subv-progressive0.jpg"
            ),
            (19, 17, 3),
        ),
        (
            "partial-19x17-rgb-subv-progressive1",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-19x17-rgb-subv-progressive1.jpg"
            ),
            (19, 17, 3),
        ),
        (
            "partial-65x49-gray-sub0-progressive0",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-gray-sub0-progressive0.jpg"
            ),
            (65, 49, 1),
        ),
        (
            "partial-65x49-gray-sub0-progressive1",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-gray-sub0-progressive1.jpg"
            ),
            (65, 49, 1),
        ),
        (
            "partial-65x49-rgb-sub0-progressive0",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-rgb-sub0-progressive0.jpg"
            ),
            (65, 49, 3),
        ),
        (
            "partial-65x49-rgb-sub0-progressive1",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-rgb-sub0-progressive1.jpg"
            ),
            (65, 49, 3),
        ),
        (
            "partial-65x49-rgb-sub1-progressive0",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-rgb-sub1-progressive0.jpg"
            ),
            (65, 49, 3),
        ),
        (
            "partial-65x49-rgb-sub1-progressive1",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-rgb-sub1-progressive1.jpg"
            ),
            (65, 49, 3),
        ),
        (
            "partial-65x49-rgb-sub2-progressive0",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-rgb-sub2-progressive0.jpg"
            ),
            (65, 49, 3),
        ),
        (
            "partial-65x49-rgb-sub2-progressive1",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-rgb-sub2-progressive1.jpg"
            ),
            (65, 49, 3),
        ),
        (
            "partial-65x49-rgb-subv-progressive0",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-rgb-subv-progressive0.jpg"
            ),
            (65, 49, 3),
        ),
        (
            "partial-65x49-rgb-subv-progressive1",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-rgb-subv-progressive1.jpg"
            ),
            (65, 49, 3),
        ),
        (
            "rgb-large-owned",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-large-owned.jpg"),
            (256, 192, 3),
        ),
        (
            "rgb-sub0-progressive0",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub0-progressive0.jpg"),
            (64, 48, 3),
        ),
        (
            "rgb-sub0-progressive1",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub0-progressive1.jpg"),
            (64, 48, 3),
        ),
        (
            "rgb-sub1-progressive0",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub1-progressive0.jpg"),
            (64, 48, 3),
        ),
        (
            "rgb-sub1-progressive1",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub1-progressive1.jpg"),
            (64, 48, 3),
        ),
        (
            "rgb-sub2-progressive0",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub2-progressive0.jpg"),
            (64, 48, 3),
        ),
        (
            "rgb-sub2-progressive1",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub2-progressive1.jpg"),
            (64, 48, 3),
        ),
    ];
    assert_eq!(fixtures.len(), 29);
    let destination = std::env::var_os("FOLIO_JPEG_DECODER_OUTPUT")
        .map(|p| std::path::PathBuf::from(p).join("pdfium-influence"));
    if let Some(root) = &destination {
        std::fs::create_dir_all(root).unwrap();
    }
    let mut table = String::from(
        "case\tname\tkind\tsw\tsh\tcomponents\tdw\tdh\tmaximum\tmatrixBits\tclip\tcells\tbounds\n",
    );
    let mut count = 0;
    for &(name, input, shape) in fixtures {
        let h = inspect(input, shape, Transform::Default).unwrap();
        let mut mask = vec![0; (h.columns * h.rows) as usize];
        let first = if h.columns > 2 && h.rows > 2 {
            (h.columns + 1) as usize
        } else {
            0
        };
        mask[first] = 1;
        *mask.last_mut().unwrap() = 1;
        let c = Coverage::from_mask(&h, &mask, &mut CoverageBudget::default()).unwrap();
        let binding = c.binding(&[9; 32]);
        let rewritten =
            rewrite(input, shape, Transform::Default, &mask, Limits::default()).unwrap();
        if let Some(root) = &destination {
            std::fs::write(root.join(format!("{name}-source.jpg")), input).unwrap();
            std::fs::write(root.join(format!("{name}-actual.jpg")), &rewritten.bytes).unwrap();
        }
        let cells = c.rectangles(&mut CoverageBudget::default()).unwrap();
        for device in [[8_u32, 6_u32], [16, 12], [65, 49], [129, 97]] {
            let [dw, dh] = device;
            let w = dw as f32;
            let y = dh as f32;
            let maximum = crate::redaction_render_sizing::pdfium_maximum(
                [h.width, h.height],
                [h.mcu_width, h.mcu_height],
                device,
            )
            .unwrap();
            for (kind, matrix) in [
                ("normal", [w, 0., 0., -y, 0., y]),
                (
                    "fractional",
                    [w * 0.75, 0.25, -0.25, -y * 0.75, w * 0.125, y * 0.875],
                ),
                (
                    "quarter",
                    [0., y * 0.75, w * 0.75, 0., w * 0.125, y * 0.125],
                ),
                (
                    "general",
                    [w * 0.75, y * 0.25, w * 0.25, -y * 0.75, 0., y * 0.75],
                ),
                (
                    "reflected",
                    [-w * 0.75, y * 0.25, w * 0.25, y * 0.75, w * 0.75, 0.],
                ),
                ("small", [w * 0.125, 0., 0., -y * 0.125, w * 0.5, y * 0.75]),
            ] {
                for clip in [
                    [0, 0, dw as i32, dh as i32],
                    [
                        (dw / 3) as i32,
                        (dh / 3) as i32,
                        (dw - dw / 4) as i32,
                        (dh - dh / 4) as i32,
                    ],
                ] {
                    let bound = c
                        .pdfium_influence(
                            RendererTransform::known(matrix).unwrap(),
                            device,
                            clip,
                            &mut CoverageBudget::default(),
                        )
                        .unwrap_or_else(|e| panic!("{name} {kind} {device:?} {clip:?}: {e:?}"));
                    let rects = |v: &[[i32; 4]]| {
                        v.iter()
                            .map(|r| {
                                r.iter()
                                    .map(ToString::to_string)
                                    .collect::<Vec<_>>()
                                    .join(",")
                            })
                            .collect::<Vec<_>>()
                            .join(";")
                    };
                    let selected = cells
                        .iter()
                        .map(|r| r.map(|v| v as i32))
                        .collect::<Vec<_>>();
                    writeln!(&mut table,"{count}\t{name}\t{kind}\t{}\t{}\t{}\t{dw}\t{dh}\t{maximum}\t{}\t{}\t{}\t{}",shape.0,shape.1,shape.2,matrix.map(f32::to_bits).map(|v|format!("{v:08x}")).join(","),clip.iter().map(ToString::to_string).collect::<Vec<_>>().join(","),rects(&selected),rects(&bound)).unwrap();
                    count += 1;
                }
            }
        }
        assert_eq!(c.mask(), mask);
        assert_eq!(c.binding(&[9; 32]), binding);
    }
    assert_eq!(count, 1392);
    if let Some(root) = &destination {
        std::fs::write(root.join("cases.tsv"), table).unwrap();
    }
}
