use super::*;
#[test]
fn redaction_render_affine_identity_reflection_and_clip_bound_complete_pixels() {
    let samples = SampleRegions::new(8, 8, vec![[3, 3, 4, 4]]).unwrap();
    for (matrix, clip, expected) in [
        (
            [8., 0., 0., 8., 0., 0.],
            [-10, -10, 20, 20],
            vec![[2, 2, 4, 4]],
        ),
        (
            [-8., 0., 0., 8., 8., 0.],
            [-10, -10, 20, 20],
            vec![[4, 2, 6, 4]],
        ),
        ([8., 0., 0., 8., 0., 0.], [3, 3, 8, 8], vec![[3, 3, 4, 4]]),
        ([8., 0., 0., 8., 0., 0.], [6, 6, 8, 8], vec![]),
    ] {
        assert_eq!(
            RendererTransform::known(matrix)
                .unwrap()
                .mupdf_affine_bounds(&samples, clip)
                .unwrap(),
            expected
        );
    }
}
#[test]
fn redaction_render_affine_shear_keeps_changed_source_center() {
    let samples = SampleRegions::new(8, 8, vec![[3, 3, 5, 5]]).unwrap();
    let result = RendererTransform::known([8., 4., 4., 8., 0., 0.])
        .unwrap()
        .mupdf_affine_bounds(&samples, [-10, -10, 20, 20])
        .unwrap();
    assert_eq!(result.len(), 1);
    let [l, t, r, b] = result[0];
    assert!(l <= 6 && t <= 6 && r > 6 && b > 6);
    assert!(
        r - l < 12 && b - t < 12,
        "must retain useful support: {result:?}"
    );
}
#[test]
fn redaction_render_affine_refuses_invalid_geometry_even_empty() {
    let samples = SampleRegions::new(8, 8, vec![]).unwrap();
    let m = RendererTransform::known([8., 0., 0., 8., 0., 0.]).unwrap();
    assert!(m.mupdf_affine_bounds(&samples, [0, 0, -1, 1]).is_err());
    assert!(RendererTransform::known([0.; 6])
        .unwrap()
        .mupdf_affine_bounds(&samples, [0, 0, 10, 10])
        .is_err());
    assert!(RendererTransform::known([1., 0., 0., 1., 1e9, 0.])
        .unwrap()
        .mupdf_affine_bounds(&samples, [0, 0, 10, 10])
        .is_err());
}

#[test]
fn redaction_render_affine_integer_limits_and_uncertain_steps_refuse() {
    assert!(check_steps([0, 0], [i64::MAX / 2; 2], [0, 0], 3, 1).is_err());
    assert!(check_steps([0, 0], [0, 0], [i64::MIN / 2; 2], 1, 3).is_err());
    assert!(check_steps([0, 0], [-ONE, ONE], [-ONE, ONE], 100000, 100000).is_ok());
    let single = SampleRegions::new(1, 1, vec![[0, 0, 1, 1]]).unwrap();
    // A valid extremely magnified image can have both native steps truncated
    // to zero. This inverse-based support proof must refuse that case.
    assert!(RendererTransform::known([200000., 0., 0., 200000., 0., 0.])
        .unwrap()
        .mupdf_affine_bounds(&single, [0, 0, 10, 10])
        .is_err());
    let samples = SampleRegions::new(8, 8, vec![[0, 0, 1, 1]]).unwrap();
    let mut matrix = RendererTransform::known([8., 0., 0., 8., 0., 0.]).unwrap();
    matrix.0[0] = Interval([-8., 8.]);
    assert!(matrix
        .mupdf_affine_bounds(&samples, [-10, -10, 20, 20])
        .is_err());
    assert!(RendererTransform::known([8., 7.9999, 8., 8., 0., 0.])
        .unwrap()
        .mupdf_affine_bounds(&samples, [0, 4, 16, 16])
        .is_err());
}

#[test]
fn redaction_render_affine_owned_stage_cases_export() {
    use std::fmt::Write;
    let mut output = String::from("case\tsw\tsh\tinterval\tclip\tcells\tbounds");
    for i in 0..6 {
        for side in ["low", "high"] {
            write!(&mut output, "\tm{i}_{side}").unwrap();
        }
    }
    output.push('\n');
    let mut count = 0;
    let rects = |values: &[[u32; 4]]| {
        values
            .iter()
            .map(|r| format!("{},{},{},{}", r[0], r[1], r[2], r[3]))
            .collect::<Vec<_>>()
            .join(";")
    };
    for (sw, sh) in [(1_u32, 1_u32), (8, 8), (19, 17), (65, 49)] {
        let w = sw as f32;
        let h = sh as f32;
        for (kind, [a, b, c, d]) in [
            [1.25 * w, 0.4 * w, 0.3 * h, 1.5 * h],
            [-1.25 * w, 0.4 * w, 0.3 * h, 1.5 * h],
            [1.25 * w, -0.4 * w, 0.3 * h, -1.5 * h],
            [0., 1.5 * w, -1.5 * h, 0.],
            [w, 0., 0., h],
            [w, 1e-8, -1e-8, h],
            [1e-8, 1.5 * w, -1.5 * h, -1e-8],
            [1.1 * w, 1.1 * w, -1.1 * h, 1.1 * h],
            [1.5 * w, 0.15 * w, 1.2 * h, 1.25 * h],
            [0.5 * w, 0.125 * w, 0.15 * h, 0.5 * h],
        ]
        .into_iter()
        .enumerate()
        {
            for (tx, ty) in [
                (0.25_f32, -0.75_f32),
                (-17.125, 21.375),
                (16384.25, -16384.75),
            ] {
                let values = [a, b, c, d, tx, ty];
                for uncertain in [false, true] {
                    // The complete interval corpus includes near-axis branch
                    // changes and uncertain integer scan origins.
                    if uncertain && (kind % 3 != 0 || sw == 1) {
                        continue;
                    }
                    let matrix =
                        if uncertain {
                            RendererTransform(values.map(|v| {
                                Interval([f64::from(v.next_down()), f64::from(v.next_up())])
                            }))
                        } else {
                            RendererTransform::known(values).unwrap()
                        };
                    let all = unit_rectangle(matrix).unwrap();
                    let full = [
                        all[0].0[0].floor() as i32 - 2,
                        all[1].0[0].floor() as i32 - 2,
                        all[2].0[1].ceil() as i32 + 2,
                        all[3].0[1].ceil() as i32 + 2,
                    ];
                    let middle = [
                        (full[0] + full[2]) / 2,
                        (full[1] + full[3]) / 2,
                        full[2],
                        full[3],
                    ];
                    for clip in [full, middle] {
                        for cells in [
                            vec![[0, 0, 1, 1]],
                            vec![[sw / 2, sh / 2, (sw / 2 + 2).min(sw), (sh / 2 + 2).min(sh)]],
                            vec![[sw - 1, sh - 1, sw, sh]],
                            vec![[0, 0, 1, sh], [sw - 1, 0, sw, sh]],
                        ] {
                            let samples = SampleRegions::new(sw, sh, cells.clone()).unwrap();
                            let bounds=matrix.mupdf_affine_bounds(&samples,clip).unwrap_or_else(|e|panic!("{values:?}, {sw}x{sh}, {clip:?}, interval={uncertain}: {e:?}"));
                            let text = bounds
                                .iter()
                                .map(|r| format!("{},{},{},{}", r[0], r[1], r[2], r[3]))
                                .collect::<Vec<_>>()
                                .join(";");
                            write!(
                                &mut output,
                                "{count}\t{sw}\t{sh}\t{uncertain}\t{},{},{},{}\t{}\t{text}",
                                clip[0],
                                clip[1],
                                clip[2],
                                clip[3],
                                rects(&cells)
                            )
                            .unwrap();
                            for pair in matrix.coefficients() {
                                for value in pair {
                                    write!(&mut output, "\t{value}").unwrap();
                                }
                            }
                            output.push('\n');
                            count += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(count > 1000);
    if let Some(root) = std::env::var_os("FOLIO_RENDER_TRANSFORM_OUTPUT") {
        let root = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("affine.tsv"), output).unwrap();
    }
}
