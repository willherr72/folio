use super::*;

fn contains(matrix: RendererTransform, values: [f32; 6]) {
    for (bound, value) in matrix.coefficients().into_iter().zip(values) {
        assert!(
            bound[0] <= f64::from(value) && f64::from(value) <= bound[1],
            "{bound:?} does not contain {value}"
        );
    }
}

#[test]
fn redaction_render_placement_native_snapping_preserves_reflection_and_tiled_turns() {
    for (input, mode, expected) in [
        (
            [10.2, 0., 0., -8.3, -0.2, 9.4],
            GridFit::Outward,
            [11., 0., 0., -9., -1., 10.],
        ),
        (
            [10.2, 0., 0., -8.3, -0.2, 9.4],
            GridFit::Tiled,
            [10., 0., 0., -7., 0., 9.],
        ),
        (
            [0., 10.2, -8.3, 0., -0.2, 9.4],
            GridFit::Outward,
            [0., 11., -9., 0., 0., 9.],
        ),
        (
            [0., 10.2, -8.3, 0., -2.2, -9.4],
            GridFit::Tiled,
            [0., 9., -9., 0., -1., -8.],
        ),
    ] {
        let result = RendererTransform::known(input)
            .unwrap()
            .mupdf_sizing([256, 192], mode)
            .unwrap();
        contains(result.fitted, expected);
        assert!(result.extents.iter().all(|b| b[0] <= b[1] && b[1] < 32));
    }
}

#[test]
fn redaction_render_placement_exact_axes_connect_to_decoder_sizing() {
    let matrix = RendererTransform::known([64., 0., 0., 48., 0., 0.]).unwrap();
    for mode in [GridFit::Disabled, GridFit::Outward, GridFit::Tiled] {
        let result = matrix.mupdf_sizing([256, 192], mode).unwrap();
        assert_eq!(result.extents, [[64, 64], [48, 48]]);
        assert_eq!(result.maximum_reduction, 2);
    }
    let result = RendererTransform::known([3., 4., 0., 5., 0., 0.])
        .unwrap()
        .mupdf_sizing([256, 192], GridFit::Disabled)
        .unwrap();
    assert_eq!(result.extents, [[5, 5], [5, 5]]);
    let result = RendererTransform::known([0.; 6])
        .unwrap()
        .mupdf_sizing([256, 192], GridFit::Outward)
        .unwrap();
    assert_eq!(result.extents, [[0, 0], [0, 0]]);
}

#[test]
fn redaction_render_placement_uncertain_classification_contains_all_branches() {
    let mut matrix = RendererTransform::known([10.2, 0., 0., 8.3, -0.2, 9.4]).unwrap();
    matrix.0[1] = Interval([-f64::from(f32::EPSILON), f64::from(f32::EPSILON)]);
    let result = matrix.mupdf_sizing([256, 192], GridFit::Outward).unwrap();
    contains(result.fitted, [11., 0., 0., 9., -1., 9.]);
    contains(result.fitted, [10.2, f32::EPSILON, 0., 8.3, -0.2, 9.4]);
    contains(result.fitted, [10.2, -f32::EPSILON, 0., 8.3, -0.2, 9.4]);
}

#[test]
fn redaction_render_placement_rejects_unsafe_integer_casts_and_extent_ranges() {
    for values in [
        [10., 0., 0., 10., 2147483648., 0.],
        [16777218., 0., 0., 10., 0., 0.],
    ] {
        assert!(RendererTransform::known(values)
            .unwrap()
            .mupdf_sizing([256, 192], GridFit::Outward)
            .is_err());
    }
    assert!(RendererTransform::default()
        .mupdf_sizing([0, 192], GridFit::Disabled)
        .is_err());
}

#[test]
fn redaction_render_placement_owned_cases_export_native_contract() {
    use std::fmt::Write;
    let mut cases = vec![
        [0.; 6],
        [1., 0., 0., 1., 0., 0.],
        [64., 0., 0., 48., 0., 0.],
        [10.2, 0., 0., -8.3, -0.2, 9.4],
        [0., 10.2, -8.3, 0., -2.2, -9.4],
        [3., 4., -4., 3., 100., -100.],
    ]
    .into_iter()
    .map(|v| RendererTransform::known(v).unwrap())
    .collect::<Vec<_>>();
    for sign in [-1., 1.] {
        for epsilon in [
            0.,
            f32::EPSILON.next_down(),
            f32::EPSILON,
            f32::EPSILON.next_up(),
        ] {
            for origin in [-1.001_f32, -1., -0.001, 0., 0.001, 0.999, 1., 1.001] {
                for quarter in [false, true] {
                    let values = if quarter {
                        [epsilon, sign * 10.2, -sign * 8.3, epsilon, origin, -origin]
                    } else {
                        [sign * 10.2, epsilon, epsilon, -sign * 8.3, origin, -origin]
                    };
                    cases.push(RendererTransform::known(values).unwrap());
                }
            }
        }
    }
    for scale in [1_f32, 2., 16., 64., 128., 256., 32768., 1048576.] {
        for y in [0., 0.001, 0.01, 0.015, 0.02, 0.025, 0.1] {
            cases
                .push(RendererTransform::known([scale.next_down(), y, 0., scale, 0., 0.]).unwrap());
        }
    }
    let mut seed = 867_u32;
    for i in 0..192 {
        let mut value = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            ((seed >> 8) as f32 / 16777216. - 0.5) * 600.
        };
        let mut v = [value(), value(), value(), value(), value(), value()];
        if i % 3 == 0 {
            v[1] = 0.;
            v[2] = 0.;
        }
        if i % 3 == 1 {
            v[0] = 0.;
            v[3] = 0.;
        }
        cases.push(RendererTransform::known(v).unwrap());
    }
    for kind in 0..4 {
        let mut v = RendererTransform::known([10.2, 0., 0., 8.3, -0.2, 9.4]).unwrap();
        for i in 0..6 {
            if kind == 0 || i % 2 == kind % 2 {
                let p = v.0[i].0[0];
                let delta = if p == 0. {
                    f64::from(f32::EPSILON)
                } else {
                    0.25
                };
                v.0[i] = Interval::enclosing(p - delta, p + delta).unwrap();
            }
        }
        cases.push(v);
    }
    let mut output = String::from("case\tmode\tsw\tsh");
    for prefix in ["in", "out"] {
        for k in ['a', 'b', 'c', 'd', 'e', 'f'] {
            write!(output, "\t{prefix}_{k}0\t{prefix}_{k}1").unwrap();
        }
    }
    output.push_str("\tw0\tw1\th0\th1\tmaximum\n");
    for (index, matrix) in cases.into_iter().enumerate() {
        for (label, mode) in [
            ("disabled", GridFit::Disabled),
            ("outward", GridFit::Outward),
            ("tiled", GridFit::Tiled),
        ] {
            let result = matrix.mupdf_sizing([256, 192], mode).unwrap();
            write!(output, "{index}\t{label}\t256\t192").unwrap();
            for v in [matrix, result.fitted] {
                for pair in v.coefficients() {
                    for endpoint in pair {
                        write!(output, "\t{endpoint}").unwrap();
                    }
                }
            }
            writeln!(
                output,
                "\t{}\t{}\t{}\t{}\t{}",
                result.extents[0][0],
                result.extents[0][1],
                result.extents[1][0],
                result.extents[1][1],
                result.maximum_reduction
            )
            .unwrap();
        }
    }
    if let Some(path) = std::env::var_os("FOLIO_PLACEMENT_OUTPUT") {
        let path = std::path::PathBuf::from(path);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("sizing.tsv"), output).unwrap();
    }
}
