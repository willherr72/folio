use super::*;
fn rect(left: f64, bottom: f64, right: f64, top: f64) -> AreaRect {
    AreaRect {
        left,
        bottom,
        right,
        top,
    }
}
fn selected(matrix: [f64; 6], width: u32, height: u32, marks: &[AreaRect]) -> PixelPlan {
    plan_pixels(
        ImageTransform::default().concat(matrix).unwrap(),
        width,
        height,
        marks,
        &mut PixelBudget::default(),
    )
    .unwrap()
    .unwrap()
}
#[test]
fn redaction_image_pixels_top_row_orientation_and_disjoint_marks() {
    let p = selected(
        [80., 0., 0., 80., 20., 20.],
        8,
        8,
        &[rect(31., 81., 39., 89.), rect(81., 31., 89., 39.)],
    );
    assert_eq!(p.count, 2);
    assert!(p.selected[8 + 1]);
    assert!(p.selected[6 * 8 + 6]);
    assert_eq!(p.bounds, rect(30., 30., 90., 90.));
}
#[test]
fn redaction_image_pixels_boundary_contact_removes_adjacent_samples() {
    let p = selected(
        [80., 0., 0., 80., 20., 20.],
        8,
        8,
        &[rect(40., 70., 50., 80.)],
    );
    let wanted: Vec<_> = (0..8)
        .flat_map(|y| (0..8).map(move |x| (1..=3).contains(&x) && (1..=3).contains(&y)))
        .collect();
    assert_eq!(p.selected, wanted);
    assert_eq!(p.count, 9);
}
#[test]
fn redaction_image_pixels_reflection_rotation_shear_and_concat_are_conservative() {
    for (matrix, mark, index) in [
        ([-80., 0., 0., 80., 100., 20.], rect(81., 81., 89., 89.), 9),
        ([0., 80., -80., 0., 100., 20.], rect(31., 31., 39., 39.), 9),
        ([80., 0., 40., 80., 20., 20.], rect(61., 81., 69., 89.), 9),
    ] {
        let p = selected(matrix, 8, 8, &[mark]);
        assert!(p.selected[index]);
    }
    let t = ImageTransform::default()
        .concat([2., 0., 0., 3., 10., 20.])
        .unwrap()
        .concat([40., 0., 0., 20., 5., 0.])
        .unwrap();
    let a = plan_pixels(
        t,
        8,
        6,
        &[rect(31., 61., 39., 69.)],
        &mut PixelBudget::default(),
    )
    .unwrap()
    .unwrap();
    let b = selected(
        [80., 0., 0., 60., 20., 20.],
        8,
        6,
        &[rect(31., 61., 39., 69.)],
    );
    assert_eq!(a.selected, b.selected);
}
#[test]
fn redaction_image_pixels_limits_misses_and_unsafe_geometry_refuse() {
    let t = ImageTransform::default()
        .concat([80., 0., 0., 80., 20., 20.])
        .unwrap();
    assert!(plan_pixels(
        t,
        8,
        8,
        &[rect(200., 200., 210., 210.)],
        &mut PixelBudget::default()
    )
    .unwrap()
    .is_none());
    for (w, h) in [(0, 1), (1, 32769), (16001, 1000)] {
        assert!(plan_pixels(
            t,
            w,
            h,
            &[rect(20., 20., 100., 100.)],
            &mut PixelBudget::default()
        )
        .is_err());
    }
    for m in [
        [0.; 6],
        [1., 0., 0., 1e-20, 0., 0.],
        [f64::NAN, 0., 0., 1., 0., 0.],
    ] {
        assert!(ImageTransform::default().concat(m).is_err());
    }
    let mut exhausted = PixelBudget {
        comparisons: 8_000_000,
        ..PixelBudget::default()
    };
    assert!(plan_pixels(t, 8, 8, &[rect(20., 20., 100., 100.)], &mut exhausted).is_err());
}

#[test]
fn redaction_image_pixels_repeated_large_occurrences_have_a_shared_sample_work_limit() {
    let t = ImageTransform::default()
        .concat([80., 0., 0., 80., 20., 20.])
        .unwrap();
    let mut budget = PixelBudget::default();
    for index in 0..5 {
        let result = plan_pixels(t, 4000, 4000, &[rect(0., 0., 120., 120.)], &mut budget);
        assert_eq!(result.is_ok(), index < 4, "occurrence {index}");
    }
}
