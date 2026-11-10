use super::*;

#[test]
fn redaction_render_transform_addition_retains_small_signed_terms() {
    let large = Interval::enclosing(2_f64.powi(50), 2_f64.powi(50)).unwrap();
    for small in [
        1.,
        -1.,
        f64::from(f32::MIN_POSITIVE),
        -f64::from(f32::MIN_POSITIVE),
    ] {
        let sum = large
            .add(Interval::enclosing(small, small).unwrap())
            .unwrap()
            .0;
        if small > 0. {
            assert!(sum[1] > large.0[1]);
        } else {
            assert!(sum[0] < large.0[0]);
        }
    }
    let negative =
        Interval::enclosing(-f64::from(f32::from_bits(1)), -f64::from(f32::from_bits(1))).unwrap();
    assert_eq!(negative.0[1], 0.);
}

#[test]
fn redaction_render_transform_owned_chains_export_forward_bounds() {
    use std::fmt::Write;
    let mut cases = vec![
        ("identity".to_owned(), vec![[1., 0., 0., 1., 0., 0.]]),
        (
            "reflection".to_owned(),
            vec![[-100., 0., 0., 75., 130., 20.], [0., 1., -1., 0., 1., 0.]],
        ),
        (
            "cancellation".to_owned(),
            vec![
                [1000000.1, 0., -1000000., 1., 0., 0.],
                [1.0000001, 1., 0., 1., 0., 0.],
            ],
        ),
    ];
    let mut seed = 173_u32;
    for index in 0..32 {
        let depth = [1, 4, 16, 64][index % 4];
        let mut chain = vec![[100., 0., 0., 75., 30., 20.]];
        for _ in 0..depth {
            let mut sample = || {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                ((seed >> 8) as f32 / 16777216. - 0.5) * 0.125
            };
            chain.push([
                1. + sample(),
                sample(),
                sample(),
                1. + sample(),
                sample(),
                sample(),
            ]);
        }
        cases.push((format!("chain-{index}-{depth}"), chain));
    }
    let mut operands = String::from("name\tstep\ta\tb\tc\td\te\tf\n");
    let mut bounds=String::from("name\tstep\ta0\ta1\tb0\tb1\tc0\tc1\td0\td1\te0\te1\tf0\tf1\tleft\tbottom\tright\ttop\tx0\ty0\tx1\ty1\n");
    for (name, chain) in cases {
        let mut matrix = RendererTransform::default();
        for (step, local) in chain.into_iter().enumerate() {
            writeln!(
                operands,
                "{name}\t{step}\t{}\t{}\t{}\t{}\t{}\t{}",
                local[0], local[1], local[2], local[3], local[4], local[5]
            )
            .unwrap();
            matrix = matrix
                .concat(RendererTransform::known(local).unwrap())
                .unwrap();
            for rect in [
                AreaRect {
                    left: 0.,
                    bottom: 0.,
                    right: 1.,
                    top: 1.,
                },
                AreaRect {
                    left: 1. / 7.,
                    bottom: 2. / 9.,
                    right: 5. / 7.,
                    top: 8. / 9.,
                },
            ] {
                let b = matrix.unit_bounds(rect).unwrap();
                write!(bounds, "{name}\t{step}").unwrap();
                for pair in matrix.coefficients() {
                    for value in pair {
                        write!(bounds, "\t{value}").unwrap();
                    }
                }
                writeln!(
                    bounds,
                    "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    rect.left, rect.bottom, rect.right, rect.top, b.left, b.bottom, b.right, b.top
                )
                .unwrap();
                assert!(b.left <= b.right && b.bottom <= b.top);
            }
        }
    }
    if let Some(root) = std::env::var_os("FOLIO_RENDER_TRANSFORM_OUTPUT") {
        let root = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("operands.tsv"), operands).unwrap();
        std::fs::write(root.join("bounds.tsv"), bounds).unwrap();
    }
}

#[test]
fn redaction_render_transform_identity_and_composition_keep_forward_bounds() {
    let identity = RendererTransform::default();
    assert_eq!(
        identity.concat(identity).unwrap().coefficients(),
        identity.coefficients()
    );
    let a = RendererTransform::known([100., 0., 0., 75., 30., 20.]).unwrap();
    let b = RendererTransform::known([1., 0.25, 0.5, 1., 2., 3.]).unwrap();
    let result = a.concat(b).unwrap();
    assert_eq!(
        result.coefficients(),
        [100., 18.75, 50., 75., 230., 245.].map(|x| [x, x])
    );
    let bounds = result
        .unit_bounds(AreaRect {
            left: 0.,
            bottom: 0.,
            right: 1.,
            top: 1.,
        })
        .unwrap();
    assert_eq!(
        (bounds.left, bounds.bottom, bounds.right, bounds.top),
        (230., 245., 380., 338.75)
    );
}

#[test]
fn redaction_render_transform_cancellation_encloses_fused_and_unfused_results() {
    let a = RendererTransform::known([1000000.1, 0., -1000000., 1., 0., 0.]).unwrap();
    let b = RendererTransform::known([1.0000001, 1., 0., 1., 0., 0.]).unwrap();
    let bound = a.concat(b).unwrap().coefficients()[0];
    let x = 1000000.1_f32;
    let y = 1.0000001_f32;
    for v in [
        f64::from(x * y - 1000000.),
        f64::from(x.mul_add(y, -1000000.)),
        f64::from(x) * f64::from(y) - 1000000.,
    ] {
        assert!(bound[0] <= v && v <= bound[1], "{bound:?} excludes {v}");
    }
}

#[test]
fn redaction_render_transform_subnormal_input_includes_flush_to_zero() {
    let tiny = f32::from_bits(1);
    let m = RendererTransform::known([tiny, 0., 0., 1., 0., 0.]).unwrap();
    assert_eq!(m.coefficients()[0], [0., f64::from(tiny)]);
    let r = m.unit_bounds(AreaRect {
        left: 1.,
        bottom: 0.,
        right: 2.,
        top: 1.,
    });
    assert!(r.is_err());
}

#[test]
fn redaction_render_transform_rejects_invalid_or_unbounded_arithmetic() {
    for v in [f32::NAN, f32::INFINITY, f32::MAX] {
        assert!(RendererTransform::known([v, 0., 0., 1., 0., 0.]).is_err());
    }
    let huge = RendererTransform::known([2_f32.powi(60), 0., 0., 1., 0., 0.]).unwrap();
    assert!(huge
        .concat(RendererTransform::known([2., 0., 0., 1., 0., 0.]).unwrap())
        .is_err());
    for r in [
        AreaRect {
            left: 0.,
            bottom: 0.,
            right: 0.,
            top: 1.,
        },
        AreaRect {
            left: f64::NAN,
            bottom: 0.,
            right: 1.,
            top: 1.,
        },
    ] {
        assert!(RendererTransform::default().unit_bounds(r).is_err());
    }
}
