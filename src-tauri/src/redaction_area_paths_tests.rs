use super::*;
use PathCommand::*;

fn mark(left: f64, bottom: f64, right: f64, top: f64) -> AreaRect {
    AreaRect {
        left,
        bottom,
        right,
        top,
    }
}

#[test]
fn redaction_disclosure_clips_source_support_before_transforming_but_keeps_page_fringe() {
    let matrix = ImageTransform::default()
        .concat([-100., 20., 30., 80., 200., 40.])
        .unwrap();
    // Expand to [0, 0.125, 0.375, 1] in image units. Its transformed
    // corner hull is [166.25, 50, 230, 127.5], before page margins.
    let bounds = matrix
        .disclosure_bounds(mark(0., 0.25, 0.25, 0.875), [0.125, 0.125], [2., 3.])
        .unwrap();
    assert_eq!(bounds, mark(164.25, 47., 232., 130.5));
    // Applying the source margin after the transform, or clipping the final
    // result back to the image, would lose the right-hand display fringe.
    assert!(bounds.right > matrix.unit_bounds(mark(0., 0., 1., 1.)).unwrap().right);
}

#[test]
fn redaction_disclosure_zero_support_is_identical_and_nonzero_support_is_outward() {
    for values in [
        [123.5, 0., 0., 95.25, 21.125, 22.5],
        [0., 128., -96., 0., 116., 16.],
        [100., 10., 25., 90., 20., 20.],
    ] {
        let matrix = ImageTransform::default().concat(values).unwrap();
        let rect = mark(0.1, 0.2, 0.7, 0.9);
        assert_eq!(
            matrix.disclosure_bounds(rect, [0.; 2], [0.; 2]).unwrap(),
            matrix.unit_bounds(rect).unwrap()
        );
        let bounds = matrix
            .disclosure_bounds(rect, [0.03125, 0.015625], [0.2, 0.3])
            .unwrap();
        let source = matrix
            .unit_bounds(mark(0.06875, 0.184375, 0.73125, 0.915625))
            .unwrap();
        assert!(bounds.left < source.left && bounds.right > source.right);
        assert!(bounds.bottom < source.bottom && bounds.top > source.top);
    }
}

#[test]
fn redaction_disclosure_refuses_invalid_support_and_out_of_range_results() {
    let matrix = ImageTransform::default();
    let rect = mark(0., 0., 1., 1.);
    for bad in [-0.1, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1_000_001.] {
        for axis in 0..2 {
            let mut margin = [0.; 2];
            margin[axis] = bad;
            assert!(matrix.disclosure_bounds(rect, margin, [0.; 2]).is_err());
            assert!(matrix.disclosure_bounds(rect, [0.; 2], margin).is_err());
        }
    }
    assert!(matrix
        .disclosure_bounds(rect, [0.; 2], [1_000_000.; 2])
        .is_err());
    assert!(matrix
        .disclosure_bounds(mark(-0.1, 0., 0.5, 0.5), [1.; 2], [0.; 2])
        .is_err());
    assert_eq!(
        matrix
            .disclosure_bounds(mark(0.25, 0.25, 0.5, 0.5), [20.; 2], [0.; 2])
            .unwrap(),
        rect
    );
}
fn run(commands: &[PathCommand], area: AreaRect) -> Vec<PathDecision> {
    plan_paths(commands, &[area]).unwrap()
}

#[test]
fn selected_path_is_wholly_removed_and_unrelated_path_is_retained() {
    let decisions = run(
        &[
            Unchanged,
            Rect([10., 20., 30., 40.]),
            Paint(PaintKind::Fill),
            Rect([100., 100., 20., 20.]),
            Paint(PaintKind::Fill),
        ],
        mark(20., 30., 25., 35.),
    );
    assert_eq!(decisions.len(), 2);
    assert_eq!(
        decisions[0],
        PathDecision {
            first: 1,
            last: 2,
            bounds: mark(10., 20., 40., 60.),
            painted: true,
            remove: true,
            extends_beyond_marks: true,
        }
    );
    assert!(!decisions[1].remove);
    assert!(!decisions[1].extends_beyond_marks);
    assert_eq!((decisions[1].first, decisions[1].last), (3, 4));
}

#[test]
fn full_coverage_has_no_extra_extent_but_compound_paths_disclose_all_subpaths() {
    let full = run(
        &[Rect([10., 20., 30., 40.]), Paint(PaintKind::Fill)],
        mark(10., 20., 40., 60.),
    );
    assert!(full[0].remove);
    assert!(!full[0].extends_beyond_marks);
    let compound = run(
        &[
            Rect([10., 20., 30., 40.]),
            Rect([100., 200., 30., 40.]),
            Paint(PaintKind::Fill),
        ],
        mark(10., 20., 40., 60.),
    );
    assert_eq!(compound.len(), 1);
    assert_eq!(compound[0].bounds, mark(10., 20., 130., 240.));
    assert!(compound[0].extends_beyond_marks);
}

#[test]
fn boundary_contact_and_nonpainted_operands_are_selected() {
    let d = run(
        &[Move([10., 10.]), Line([20., 20.]), End],
        mark(20., 20., 30., 30.),
    );
    assert!(d[0].remove);
    assert!(!d[0].painted);
    assert_eq!(d[0].bounds, mark(10., 10., 20., 20.));
    assert!(plan_paths(&[End], &[mark(0., 0., 1., 1.)])
        .unwrap()
        .is_empty());
}

#[test]
fn every_curve_control_point_contributes_and_close_keeps_subpath_origin() {
    let d = run(
        &[
            Move([10., 10.]),
            Curve([20., 80., 70., -20., 90., 10.]),
            CurveV([110., 120., 130., 10.]),
            CurveY([150., -40., 170., 10.]),
            Close,
            Line([15., 25.]),
            Paint(PaintKind::Fill),
        ],
        mark(20., 70., 25., 75.),
    );
    assert_eq!(d[0].bounds, mark(10., -40., 170., 120.));
    assert!(d[0].remove);
}

#[test]
fn matrices_concatenate_in_pdf_order_and_restore_per_object() {
    let d = run(
        &[
            Concat([2., 0., 0., 3., 10., 20.]),
            Save,
            Concat([1., 0., 0., 1., 5., 7.]),
            Rect([0., 0., 10., 10.]),
            Paint(PaintKind::Fill),
            Restore,
            Rect([0., 0., 10., 10.]),
            Paint(PaintKind::Fill),
        ],
        mark(20., 60., 21., 61.),
    );
    assert_eq!(d[0].bounds, mark(20., 41., 40., 71.));
    assert_eq!(d[1].bounds, mark(10., 20., 30., 50.));
    assert!(d[0].remove);
    assert!(!d[1].remove);
}

#[test]
fn reflected_negative_rectangles_and_axis_swaps_have_ordered_bounds() {
    let d = run(
        &[
            Concat([0., -2., -3., 0., 100., 200.]),
            Rect([10., 20., -5., -10.]),
            Paint(PaintKind::Fill),
        ],
        mark(40., 180., 70., 190.),
    );
    assert_eq!(d[0].bounds, mark(40., 180., 70., 190.));
    assert!(d[0].remove);
    assert!(!d[0].extends_beyond_marks);
}

#[test]
fn thick_miter_stroke_is_selected_even_when_its_centerline_misses() {
    let d = run(
        &[
            LineWidth(10.),
            MiterLimit(4.),
            LineJoin(0),
            LineCap(2),
            Move([50., 50.]),
            Line([80., 50.]),
            Paint(PaintKind::Stroke),
        ],
        mark(14., 48., 16., 52.),
    );
    assert!(d[0].remove);
    assert!(d[0].bounds.left <= 10. && d[0].bounds.right >= 120.);
    assert!(d[0].bounds.bottom <= 10. && d[0].bounds.top >= 90.);
}

#[test]
fn sheared_square_caps_and_saved_stroke_styles_are_conservatively_bounded() {
    let d = run(
        &[
            LineJoin(2),
            LineWidth(2.),
            Save,
            LineWidth(20.),
            Concat([1., 0., 3., 2., 0., 0.]),
            Move([10., 10.]),
            Line([20., 10.]),
            Paint(PaintKind::CloseFillStroke),
            Restore,
            Move([10., 10.]),
            Line([20., 10.]),
            Paint(PaintKind::CloseStroke),
        ],
        mark(83., 19., 84., 21.),
    );
    assert!(
        d[0].remove,
        "square-cap diagonal under shear extends beyond centerline"
    );
    assert!(d[0].bounds.right >= 94.7213595499958);
    assert!(!d[1].remove);
    assert!(d[1].bounds.left > 8. && d[1].bounds.right < 22.);
}

#[test]
fn fill_stroke_variants_and_degenerate_path_bounds_do_not_escape_selection() {
    for paint in [
        PaintKind::Stroke,
        PaintKind::FillStroke,
        PaintKind::CloseStroke,
        PaintKind::CloseFillStroke,
    ] {
        let d = run(
            &[
                LineWidth(2.),
                LineJoin(1),
                Move([10., 10.]),
                Line([10., 10.]),
                Paint(paint),
            ],
            mark(9., 9., 11., 11.),
        );
        assert!(d[0].remove);
        assert!(d[0].bounds.left < 10. && d[0].bounds.top > 10.);
    }
}

#[test]
fn multiple_marks_select_paths_and_union_coverage_can_conservatively_disclose_extra() {
    let d = plan_paths(
        &[Rect([0., 0., 20., 10.]), Paint(PaintKind::Fill)],
        &[mark(0., 0., 10., 10.), mark(10., 0., 20., 10.)],
    )
    .unwrap();
    assert!(d[0].remove && d[0].extends_beyond_marks);
}

#[test]
fn clipping_unknown_commands_and_state_changes_with_pending_paths_refuse_atomically() {
    for bad in [
        Clip,
        Unsupported,
        Save,
        Restore,
        Concat([1., 0., 0., 1., 0., 0.]),
        LineWidth(2.),
        LineJoin(1),
        LineCap(1),
        MiterLimit(5.),
        Unchanged,
    ] {
        let cmds = [
            Rect([0., 0., 1., 1.]),
            Paint(PaintKind::Fill),
            Move([10., 10.]),
            bad,
            End,
        ];
        assert!(
            plan_paths(&cmds, &[mark(0., 0., 5., 5.)]).is_err(),
            "{bad:?}"
        );
    }
    for bad in [Clip, Unsupported] {
        assert_eq!(
            plan_paths(&[bad], &[mark(0., 0., 5., 5.)]),
            Err(PathError::Unsupported)
        );
    }
}

#[test]
fn malformed_path_state_and_unbalanced_stack_refuse() {
    let cases = vec![
        vec![Restore],
        vec![Save],
        vec![Move([0., 0.])],
        vec![Close],
        vec![Line([0., 0.]), End],
        vec![Curve([0.; 6]), End],
        vec![CurveV([0.; 4]), End],
        vec![CurveY([0.; 4]), End],
        vec![Paint(PaintKind::Fill)],
        vec![LineJoin(3)],
        vec![LineCap(3)],
        vec![MiterLimit(0.5)],
        vec![LineWidth(-1.)],
    ];
    for commands in cases {
        assert!(
            plan_paths(&commands, &[mark(0., 0., 1., 1.)]).is_err(),
            "{commands:?}"
        );
    }
}

#[test]
fn hairlines_refuse_only_when_stroked() {
    let commands = [
        LineWidth(0.),
        Rect([0., 0., 10., 10.]),
        Paint(PaintKind::Stroke),
    ];
    assert_eq!(
        plan_paths(&commands, &[mark(0., 0., 1., 1.)]),
        Err(PathError::Unsupported)
    );
    assert!(plan_paths(
        &[
            LineWidth(0.),
            Rect([0., 0., 10., 10.]),
            Paint(PaintKind::Fill)
        ],
        &[mark(0., 0., 1., 1.)]
    )
    .is_ok());
}

#[test]
fn numeric_overflow_singular_and_near_singular_matrices_refuse() {
    for matrix in [
        [0.; 6],
        [1., 2., 2., 4., 0., 0.],
        [1., 0., 0., 1e-13, 0., 0.],
        [f64::NAN, 0., 0., 1., 0., 0.],
        [1e7, 0., 0., 1., 0., 0.],
    ] {
        assert!(plan_paths(&[Concat(matrix)], &[mark(0., 0., 1., 1.)]).is_err());
    }
    for commands in [
        vec![
            Concat([1e6, 0., 0., 1e6, 0., 0.]),
            Concat([1e6, 0., 0., 1e6, 0., 0.]),
        ],
        vec![
            Concat([1e6, 0., 0., 1e6, 0., 0.]),
            Rect([2., 2., 1., 1.]),
            Paint(PaintKind::Fill),
        ],
        vec![Rect([999999., 0., 2., 1.]), Paint(PaintKind::Fill)],
        vec![
            LineWidth(1e6),
            MiterLimit(1e6),
            Move([0., 0.]),
            Line([1., 1.]),
            Paint(PaintKind::Stroke),
        ],
        vec![Move([f64::INFINITY, 0.]), End],
        vec![LineWidth(f64::NAN)],
        vec![MiterLimit(f64::INFINITY)],
    ] {
        assert!(plan_paths(&commands, &[mark(0., 0., 1., 1.)]).is_err());
    }
}

#[test]
fn collapsed_transformed_geometry_refuses_instead_of_accepting_lost_extent() {
    for commands in [
        vec![
            Concat([1e-200, 0., 0., 1e-200, 1., 1.]),
            Rect([0., 0., 1., 1.]),
            Paint(PaintKind::Fill),
        ],
        vec![
            Concat([1e-200, 0., 0., 1e-200, 0., 0.]),
            Concat([1e-200, 0., 0., 1e-200, 0., 0.]),
        ],
    ] {
        assert!(plan_paths(&commands, &[mark(0., 0., 2., 2.)]).is_err());
    }
}

#[test]
fn invalid_mark_and_resource_limits_refuse_without_partial_results() {
    for area in [
        mark(0., 0., 0., 1.),
        mark(0., 0., 1., 0.),
        mark(2., 0., 1., 1.),
        mark(f64::NAN, 0., 1., 1.),
        mark(0., 0., 1e7, 1.),
    ] {
        assert_eq!(plan_paths(&[], &[area]), Err(PathError::InvalidMark));
    }
    let area = mark(0., 0., 1., 1.);
    assert!(plan_paths(&[], &[]).is_err());
    assert!(plan_paths(&[], &vec![area; 65]).is_err());
    assert!(plan_paths(&[], &vec![area; 64]).is_ok());
    assert!(plan_paths(&vec![Unchanged; 100000], &[area]).is_ok());
    assert_eq!(
        plan_paths(&vec![Unchanged; 100001], &[area]),
        Err(PathError::Limit)
    );
    let mut commands = vec![Save; 64];
    commands.extend(vec![Restore; 64]);
    assert!(plan_paths(&commands, &[area]).is_ok());
    commands.insert(0, Save);
    assert_eq!(plan_paths(&commands, &[area]), Err(PathError::Limit));
}

#[test]
fn fractional_transforms_keep_rounding_uncertainty_in_the_selected_extent() {
    let d = run(
        &[
            Concat([0.1, 0., 0., 0.1, 1., 1.]),
            Rect([1., 1., 1., 1.]),
            Paint(PaintKind::Fill),
        ],
        mark(1., 1., 2., 2.),
    );
    // The exact binary input 0.1 + 1 is below the rounded f64 literal 1.1.
    // An ordinary rounded lower bound would omit that real interval.
    assert!(d[0].bounds.left < 1.1);
    assert!(d[0].bounds.bottom < 1.1);
    assert!(d[0].bounds.right >= 1.2);
    assert!(d[0].bounds.top >= 1.2);
}

#[test]
fn sampled_curves_and_square_strokes_stay_inside_transformed_bounds() {
    let controls = [[10., 20.], [25., 80.], [65., -30.], [90., 15.]];
    for [a, b, c, d, e, f] in [
        [1., 0., 0., 1., 0., 0.],
        [1.25, 0.5, 3., 2., -10., 30.],
        [-2., 0.75, 1.5, 0.5, 200., 100.],
        [0., -2., -3., 0., 100., 200.],
    ] {
        let decisions = run(
            &[
                Concat([a, b, c, d, e, f]),
                LineJoin(2),
                LineCap(2),
                LineWidth(8.),
                Move(controls[0]),
                Curve([25., 80., 65., -30., 90., 15.]),
                Paint(PaintKind::Stroke),
            ],
            mark(-1000., -1000., 1000., 1000.),
        );
        let bounds = decisions[0].bounds;
        for step in 0..=128 {
            let t = step as f64 / 128.;
            let u = 1. - t;
            // Bernstein evaluation is independent of the planner's hull union.
            let weights = [u * u * u, 3. * u * u * t, 3. * u * t * t, t * t * t];
            let x: f64 = controls.iter().zip(weights).map(|(p, w)| p[0] * w).sum();
            let y: f64 = controls.iter().zip(weights).map(|(p, w)| p[1] * w).sum();
            for dx in [-4., 0., 4.] {
                for dy in [-4., 0., 4.] {
                    let px = a * (x + dx) + c * (y + dy) + e;
                    let py = b * (x + dx) + d * (y + dy) + f;
                    assert!(px >= bounds.left && px <= bounds.right);
                    assert!(py >= bounds.bottom && py <= bounds.top);
                }
            }
        }
    }
}

#[test]
fn half_subnormal_product_residual_at_threshold_cannot_hide_a_touched_path() {
    // M*N = 2^105 - 1, so a*b = 2^-970 - 2^-1075. Both the product
    // and its FMA residual round to the apparent exact result (residual -0).
    // After translation the exact first point is (-2^-1074, 2), on the mark.
    let a = 6093374271049919. * 2f64.powi(-52);
    let b = 6657201314554049. * f64::from_bits(1u64 << 51);
    let threshold = f64::MIN_POSITIVE / f64::EPSILON;
    assert_residual_enclosure(a, b, threshold);
}

#[test]
fn half_subnormal_residual_above_threshold_also_requires_outward_bounds() {
    // This product rounds above 2^-970, so changing < to <= at that
    // threshold alone is insufficient. Exact product is rounded - 2^-1075.
    let a = 7436763404172921. * 2f64.powi(-52);
    let b = 5649369489077303. * f64::from_bits(1u64 << 51);
    let rounded = f64::from_bits(0x0350923b393a164f);
    assert_residual_enclosure(a, b, rounded);
}

fn assert_residual_enclosure(a: f64, b: f64, rounded: f64) {
    let smallest = f64::from_bits(1);
    for sign in [-1., 1.] {
        let commands = [
            Concat([sign * a, 0., 0., 1., -2. * sign * rounded, 0.]),
            Concat([1., 0., b, 1., 0., 0.]),
            Move([0., 2.]),
            Line([0., 3.]),
            End,
        ];
        let area = if sign > 0. {
            mark(-2. * smallest, 1., -smallest, 2.)
        } else {
            mark(smallest, 1., 2. * smallest, 2.)
        };
        let d = run(&commands, area);
        assert!(
            d[0].remove,
            "a rounded-to-zero residual lost the touched endpoint"
        );
        assert!(d[0].bounds.left <= -sign * smallest && d[0].bounds.right >= -sign * smallest);
        assert!(
            !run(&commands, mark(1., 1., 2., 3.))[0].remove,
            "unrelated path must remain"
        );
    }
}
