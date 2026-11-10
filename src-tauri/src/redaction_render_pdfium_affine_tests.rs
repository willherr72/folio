use super::*;
#[test]
fn redaction_pdfium_affine_identity_and_reflection_keep_bilinear_neighbors() {
    let s = SampleRegions::new(8, 8, vec![[3, 3, 4, 4]]).unwrap();
    assert_eq!(
        RendererTransform::default()
            .pdfium_affine_bounds(&s, [0, 0, 8, 8])
            .unwrap(),
        vec![[2, 2, 4, 4]]
    );
    assert_eq!(
        RendererTransform::known([-1., 0., 0., 1., 8., 0.])
            .unwrap()
            .pdfium_affine_bounds(&s, [10, 20, 18, 28])
            .unwrap(),
        vec![[15, 22, 17, 24]]
    );
}
#[test]
fn redaction_pdfium_affine_negative_fractional_first_edge_is_included() {
    let s = SampleRegions::new(8, 8, vec![[0, 0, 1, 1]]).unwrap();
    assert_eq!(
        RendererTransform::known([1., 0., 0., 1., -0.75, -0.75])
            .unwrap()
            .pdfium_affine_bounds(&s, [0, 0, 8, 8])
            .unwrap(),
        vec![[0, 0, 2, 2]]
    );
}
#[test]
fn redaction_pdfium_affine_unsafe_coordinates_and_quantized_collapse_refuse() {
    let s = SampleRegions::new(8, 8, vec![]).unwrap();
    for matrix in [
        [0.; 6],
        [1e-4, 0., 0., 1e-4, 0., 0.],
        [1., 0., 0., 1., 1e9, 0.],
        [1e6, 0., 0., 1., 0., 0.],
    ] {
        assert!(RendererTransform::known(matrix)
            .unwrap()
            .pdfium_affine_bounds(&s, [0, 0, 100, 100])
            .is_err());
    }
    assert!(RendererTransform::default()
        .pdfium_affine_bounds(&s, [0, 0, -1, 10])
        .is_err());
}

#[test]
fn redaction_pdfium_affine_large_cancellation_and_long_scan_are_bounded() {
    let s = SampleRegions::new(8, 4, vec![[0, 0, 2, 4]]).unwrap();
    let bounds = RendererTransform::known([512., 0., 0.125, 1., -3584000., 0.])
        .unwrap()
        .pdfium_affine_bounds(&s, [0, 0, 8192, 4])
        .unwrap();
    assert!(bounds.iter().any(|r| r[0] <= 7000 && r[2] > 7000));
    let s = SampleRegions::new(32768, 1, vec![[32767, 0, 32768, 1]]).unwrap();
    let bounds = RendererTransform::known([1. / 256., 0., 0., 1., 0., 0.])
        .unwrap()
        .pdfium_affine_bounds(&s, [0, 0, 1 << 24, 1])
        .unwrap();
    assert!(bounds.iter().any(|r| r[0] <= 8388352 && r[2] > 8388352));
    let huge = SampleRegions::new(32768, 32768, vec![]).unwrap();
    assert!(RendererTransform::default()
        .pdfium_affine_bounds(&huge, [0, 0, 10, 10])
        .is_err());
}

#[test]
fn redaction_pdfium_affine_owned_cases_export() {
    use std::fmt::Write;
    let mut text = String::from("case\tsw\tsh\tresult\tcells\tbounds");
    for i in 0..6 {
        for side in ["low", "high"] {
            write!(&mut text, "\tm{i}_{side}").unwrap();
        }
    }
    text.push('\n');
    let mut count = 0;
    let mut record = |sw, sh, matrix: RendererTransform, result: [i32; 4], cells: Vec<[u32; 4]>| {
        let samples = SampleRegions::new(sw, sh, cells.clone()).unwrap();
        let bounds = matrix.pdfium_affine_bounds(&samples, result).unwrap();
        let bound_text = bounds
            .iter()
            .map(|r| format!("{},{},{},{}", r[0], r[1], r[2], r[3]))
            .collect::<Vec<_>>()
            .join(";");
        let cell_text = cells
            .iter()
            .map(|r| format!("{},{},{},{}", r[0], r[1], r[2], r[3]))
            .collect::<Vec<_>>()
            .join(";");
        write!(
            &mut text,
            "{count}\t{sw}\t{sh}\t{},{},{},{}\t{cell_text}\t{bound_text}",
            result[0], result[1], result[2], result[3]
        )
        .unwrap();
        for v in matrix.coefficients() {
            for n in v {
                write!(&mut text, "\t{n}").unwrap();
            }
        }
        text.push('\n');
        count += 1;
    };
    for (sw, sh) in [(1_u32, 1_u32), (8, 8), (19, 17), (65, 49)] {
        let w = sw as f32;
        let h = sh as f32;
        for (kind, values) in [
            [1., 0., 0., 1., 0., 0.],
            [-1., 0., 0., 1., w - 1., 0.],
            [0.75, 0.25, -0.25, 0.75, h * 0.25, 0.],
            [0.5, -0.25, 0.25, 0.5, 0., w * 0.25],
            [1.25, 0.75, 0.5, 1.25, -w * 0.25, -h * 0.25],
            [0.125, 0.25, -0.25, 0.125, w / 2., 0.],
            [0., 1., -1., 0., h - 1., 0.],
            [
                1. + 1. / 512.,
                -1. / 512.,
                1. / 512.,
                1. - 1. / 512.,
                -0.75,
                -0.75,
            ],
        ]
        .into_iter()
        .enumerate()
        {
            for uncertain in [false, true] {
                if uncertain && kind % 2 != 0 {
                    continue;
                }
                let matrix = if uncertain {
                    RendererTransform(
                        values
                            .map(|v| Interval([f64::from(v.next_down()), f64::from(v.next_up())])),
                    )
                } else {
                    RendererTransform::known(values).unwrap()
                };
                for (x, y) in [(0, 0), (-50, 29)] {
                    let result = [x, y, x + 2 * sw as i32, y + 2 * sh as i32];
                    for cells in [
                        vec![[0, 0, 1, 1]],
                        vec![[sw / 2, sh / 2, (sw / 2 + 2).min(sw), (sh / 2 + 2).min(sh)]],
                        vec![[sw - 1, sh - 1, sw, sh]],
                        vec![[0, 0, 1, sh], [sw - 1, 0, sw, sh]],
                    ] {
                        record(sw, sh, matrix, result, cells);
                    }
                }
            }
        }
    }
    for values in [
        [512., 0., 0.125, 1., -3584000., 0.],
        [512., 0., -0.125, 1., -3584000., -0.75],
    ] {
        record(
            8,
            4,
            RendererTransform::known(values).unwrap(),
            [0, 0, 8192, 4],
            vec![[0, 0, 2, 4]],
        );
    }
    assert_eq!(count, 386);
    if let Some(root) = std::env::var_os("FOLIO_RENDER_TRANSFORM_OUTPUT") {
        let root = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("pdfium-affine.tsv"), text).unwrap();
    }
}
