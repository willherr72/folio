use super::*;

#[test]
fn redaction_render_inverse_identity_scaling_and_reflection_are_exact() {
    for (matrix, expected) in [
        ([1., 0., 0., 1., 0., 0.], [1., 0., 0., 1., 0., 0.]),
        ([2., 0., 0., 4., 8., 12.], [0.5, 0., 0., 0.25, -4., -3.]),
        ([-2., 0., 0., 4., 8., 12.], [-0.5, 0., 0., 0.25, 4., -3.]),
        ([0., 2., -4., 0., 8., 12.], [0., -0.25, 0.5, 0., -6., 2.]),
    ] {
        assert_eq!(
            RendererTransform::known(matrix)
                .unwrap()
                .reader_inverse()
                .unwrap()
                .coefficients(),
            expected.map(|v| [v, v])
        );
    }
}

#[test]
fn redaction_render_inverse_affine_translation_contains_exact_values() {
    let result = RendererTransform::known([2., 1., 1., 3., 4., 5.])
        .unwrap()
        .reader_inverse()
        .unwrap();
    for (bound, exact) in result
        .coefficients()
        .into_iter()
        .zip([0.6, -0.2, -0.2, 0.4, -1.4, -1.2])
    {
        assert!(
            bound[0] <= exact && exact <= bound[1],
            "{bound:?} misses {exact}"
        );
    }
}

#[test]
fn redaction_render_inverse_refuses_singular_ambiguous_and_extreme_matrices() {
    for values in [
        [0.; 6],
        [1., 1., 1., 1., 0., 0.],
        [1., 0., 0., 1e-18, 0., 0.],
        [1e-6, 0., 0., 1e-6, 2_f32.powi(50), 0.],
    ] {
        assert!(RendererTransform::known(values)
            .unwrap()
            .reader_inverse()
            .is_err());
    }
    let mut matrix = RendererTransform::default();
    matrix.0[1] = Interval([0., 1.]);
    matrix.0[2] = Interval([0., 1.]);
    assert!(matrix.reader_inverse().is_err());
}

#[test]
fn redaction_render_inverse_exports_known_and_uncertain_cases() {
    use std::fmt::Write;
    let mut cases = Vec::new();
    for matrix in [
        [1., 0., 0., 1., 0., 0.],
        [2., 1., 1., 3., 4., 5.],
        [-2., 1., 1., 3., -4., 5.],
        [0., 2., -4., 0., 8., 12.],
        [0.; 6],
        [1., 1., 1., 1., 0., 0.],
        [1., 1., 1., 1.0000001, 1000000., -1000000.],
        [1000000.1, 1000000., 1.0000001, 1., 0.25, -0.75],
        [1e-6, 0., 0., 1e-6, 2_f32.powi(50), 0.],
    ] {
        cases.push(RendererTransform::known(matrix).unwrap());
    }
    for exponent in -60..=30 {
        for sign in [-1., 1.] {
            cases.push(
                RendererTransform::known([1., 0., 0., sign * 2_f32.powi(exponent), 0., 0.])
                    .unwrap(),
            );
        }
    }
    let mut seed = 0x49a713c9_u32;
    let mut random = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        seed
    };
    for index in 0..512 {
        let scale = 2_f32.powi((random() % 33) as i32 - 16);
        let values = std::array::from_fn(|_| ((random() % 20001) as f32 - 10000.) / 997. * scale);
        let matrix = RendererTransform::known(values).unwrap();
        cases.push(matrix);
        if index % 4 == 0 {
            cases.push(RendererTransform(values.map(|v| {
                Interval([f64::from(v.next_down()), f64::from(v.next_up())])
            })));
        }
    }
    let mut ambiguous = RendererTransform::default();
    ambiguous.0[1] = Interval([0., 1.]);
    ambiguous.0[2] = Interval([0., 1.]);
    cases.push(ambiguous);
    let mut tsv = String::from("case\taccepted");
    for name in ["input", "output"] {
        for coefficient in 0..6 {
            for side in ["low", "high"] {
                write!(&mut tsv, "\t{name}_{coefficient}_{side}").unwrap();
            }
        }
    }
    tsv.push('\n');
    let mut admitted = 0;
    for (index, matrix) in cases.iter().copied().enumerate() {
        let result = matrix.reader_inverse();
        write!(&mut tsv, "{index}\t{}", result.is_ok()).unwrap();
        for interval in matrix.coefficients() {
            for v in interval {
                write!(&mut tsv, "\t{v}").unwrap();
            }
        }
        if let Ok(inverse) = result {
            admitted += 1;
            for interval in inverse.coefficients() {
                assert!(interval[0].is_finite() && interval[0] <= interval[1]);
                for v in interval {
                    write!(&mut tsv, "\t{v}").unwrap();
                }
            }
        } else {
            for _ in 0..12 {
                tsv.push('\t');
            }
        }
        tsv.push('\n');
    }
    assert!(admitted > 600 && admitted < cases.len());
    if let Some(root) = std::env::var_os("FOLIO_RENDER_TRANSFORM_OUTPUT") {
        let root = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("inverse.tsv"), tsv).unwrap();
    }
}
