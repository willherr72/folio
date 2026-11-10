use super::*;
fn contains(bounds: &[[i32; 4]], x: i32, y: i32) -> bool {
    bounds
        .iter()
        .any(|r| r[0] <= x && x < r[2] && r[1] <= y && y < r[3])
}
#[test]
fn redaction_pdfium_image_direct_and_quarter_orientation() {
    let s = SampleRegions::new(8, 8, vec![[1, 2, 2, 3]]).unwrap();
    for (matrix, point) in [
        ([8., 0., 0., -8., 10., 18.], [11, 12]),
        ([-8., 0., 0., 8., 18., 10.], [16, 15]),
        ([0., 8., 8., 0., 10., 10.], [15, 11]),
        ([0., -8., -8., 0., 18., 18.], [12, 16]),
    ] {
        let bounds = RendererTransform::known(matrix)
            .unwrap()
            .pdfium_image_bounds(&s, [0, 0, 32, 32])
            .unwrap();
        assert!(
            contains(&bounds, point[0], point[1]),
            "{matrix:?}: {bounds:?}"
        );
        assert!(!contains(&bounds, 0, 0));
    }
}
#[test]
fn redaction_pdfium_image_general_clip_and_empty_selection() {
    let s = SampleRegions::new(8, 8, vec![[0, 0, 8, 8]]).unwrap();
    let m = RendererTransform::known([16., 4., 3., -16., 20., 36.]).unwrap();
    let out = m.pdfium_image_bounds(&s, [25, 25, 32, 32]).unwrap();
    assert!(contains(&out, 28, 28));
    assert!(out
        .iter()
        .all(|r| r[0] >= 25 && r[1] >= 25 && r[2] <= 32 && r[3] <= 32));
    let empty = SampleRegions::new(8, 8, vec![]).unwrap();
    assert!(m
        .pdfium_image_bounds(&empty, [25, 25, 32, 32])
        .unwrap()
        .is_empty());
    assert!(m.pdfium_image_bounds(&s, [0, 0, 1, 1]).unwrap().is_empty());
}
#[test]
fn redaction_pdfium_image_unbounded_geometry_refuses_even_empty() {
    let s = SampleRegions::new(8, 8, vec![]).unwrap();
    let m = RendererTransform::known([8., 0., 0., -8., 1e9, 0.]).unwrap();
    assert!(m.pdfium_image_bounds(&s, [0, 0, 20, 20]).is_err());
    assert!(RendererTransform::default()
        .pdfium_image_bounds(&s, [2, 0, 1, 1])
        .is_err());
    let mut uncertain = RendererTransform::default();
    uncertain.0[4] = Interval([0., 1000.]);
    assert!(uncertain.pdfium_image_bounds(&s, [0, 0, 2000, 20]).is_err());
}

#[test]
fn redaction_pdfium_image_source_limits_apply_outside_clip() {
    let s = SampleRegions::new(32769, 1, vec![]).unwrap();
    assert!(RendererTransform::default()
        .pdfium_image_bounds(&s, [10, 10, 20, 20])
        .is_err());
}

#[test]
fn redaction_pdfium_image_branch_work_is_bounded() {
    let s = SampleRegions::new(8, 8, vec![[0, 0, 1, 1]; 4096]).unwrap();
    let mut m = RendererTransform::known([30.1, 10.2, 8.3, -24.4, 24.25, 60.75]).unwrap();
    m.0[4] = Interval([24., 26.]);
    assert!(m.pdfium_image_bounds(&s, [0, 0, 128, 128]).is_err());
}

#[test]
fn redaction_pdfium_image_uncertain_branches_include_endpoint_support() {
    let s = SampleRegions::new(19, 17, vec![[5, 4, 7, 6]]).unwrap();
    for (values, index, range) in [
        ([32., 0.5, 0.25, -24., 24., 60.], 1, [0.4999, 0.5001]),
        ([0.5, 32., 24., 0., 24., 24.], 0, [0.4999, 0.5001]),
    ] {
        let mut wide = RendererTransform::known(values).unwrap();
        wide.0[index] = Interval::enclosing(range[0], range[1]).unwrap();
        let bound = wide.pdfium_image_bounds(&s, [0, 0, 128, 128]).unwrap();
        for value in wide.0[index].0 {
            let mut actual = values;
            actual[index] = value as f32;
            for r in RendererTransform::known(actual)
                .unwrap()
                .pdfium_image_bounds(&s, [0, 0, 128, 128])
                .unwrap()
            {
                for y in r[1]..r[3] {
                    for x in r[0]..r[2] {
                        assert!(contains(&bound, x, y), "{actual:?}: {x},{y}");
                    }
                }
            }
        }
    }
}

#[test]
fn redaction_pdfium_image_owned_placement_cases_export() {
    use std::fmt::Write;
    let matrices: [(&str, [f32; 6]); 19] = [
        ("normal", [32., 0., 0., -24., 24., 60.]),
        ("fractional", [32., 0., 0., -24., 24.25, 60.75]),
        ("reflected", [-32., 0., 0., 24., 64.25, 20.75]),
        ("near-axis", [32., 0.49, -0.49, -24., 24.25, 60.75]),
        ("quarter", [0., 32., 24., 0., 24., 24.]),
        ("quarter-flip", [0., -32., -24., 0., 64.25, 64.75]),
        ("near-quarter", [0.49, 32., 24., -0.49, 24.25, 24.75]),
        ("general", [30., 10., 8., -24., 24.25, 60.75]),
        ("general-reflect", [30., 10., -8., 24., 24.25, 24.75]),
        ("rotation", [23., -17., 17., 23., 24.25, 48.75]),
        ("downscale", [2., 1., 1., -2., 40.25, 40.75]),
        ("upscale", [90., 16., 14., -86., 12.25, 100.75]),
        ("zero-width", [0., 0.03, 0.04, 20., 40., 40.]),
        ("axis-threshold", [32., 0.5, 0.25, -24., 24.25, 60.75]),
        ("quarter-threshold", [0.5, 32., 24., 0., 24.25, 24.75]),
        (
            "uncertain-axis-low",
            [32., 0.4999, 0.25, -24., 24.25, 60.75],
        ),
        (
            "uncertain-axis-high",
            [32., 0.5001, 0.25, -24., 24.25, 60.75],
        ),
        (
            "uncertain-quarter-low",
            [0.4999, 32., 24., 0., 24.25, 24.75],
        ),
        (
            "uncertain-quarter-high",
            [0.5001, 32., 24., 0., 24.25, 24.75],
        ),
    ];
    let mut text = String::from("case\tkind\tsw\tsh\tmatrixBits\tclip\tcells\tbounds\n");
    let mut count = 0;
    for (sw, sh) in [(8_u32, 8_u32), (19, 17), (65, 49)] {
        for (kind, matrix) in matrices {
            for clip in [[0, 0, 128, 128], [32, 32, 55, 55], [40, 39, 41, 65]] {
                for cells in [
                    vec![[0, 0, 1, 1]],
                    vec![[sw / 2, sh / 2, sw / 2 + 2, sh / 2 + 2]],
                    vec![[sw - 1, sh - 1, sw, sh]],
                    vec![[0, 0, 1, sh], [sw - 1, 0, sw, sh]],
                ] {
                    let samples = SampleRegions::new(sw, sh, cells).unwrap();
                    let mut proved = RendererTransform::known(matrix).unwrap();
                    if kind.starts_with("uncertain-") {
                        let index = if kind.starts_with("uncertain-axis-") {
                            1
                        } else {
                            0
                        };
                        proved.0[index] =
                            Interval::enclosing(f64::from(0.4999_f32), f64::from(0.5001_f32))
                                .unwrap();
                    }
                    let out = proved
                        .pdfium_image_bounds(&samples, clip)
                        .unwrap_or_else(|e| panic!("{kind} {sw}x{sh} {clip:?}: {e:?}"));
                    assert!(out.iter().all(|r| r[0] >= clip[0]
                        && r[1] >= clip[1]
                        && r[2] <= clip[2]
                        && r[3] <= clip[3]));
                    let nums = |v: &[i32]| {
                        v.iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(",")
                    };
                    let rects =
                        |v: &[[i32; 4]]| v.iter().map(|r| nums(r)).collect::<Vec<_>>().join(";");
                    let selected = samples
                        .cells()
                        .iter()
                        .map(|r| r.map(|v| v as i32))
                        .collect::<Vec<_>>();
                    writeln!(
                        &mut text,
                        "{count}\t{kind}\t{sw}\t{sh}\t{}\t{}\t{}\t{}",
                        matrix
                            .map(f32::to_bits)
                            .map(|v| format!("{v:08x}"))
                            .join(","),
                        nums(&clip),
                        rects(&selected),
                        rects(&out)
                    )
                    .unwrap();
                    count += 1;
                }
            }
        }
    }
    assert_eq!(count, 684);
    if let Some(root) = std::env::var_os("FOLIO_RENDER_TRANSFORM_OUTPUT") {
        let root = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("pdfium-image.tsv"), text).unwrap();
    }
}
