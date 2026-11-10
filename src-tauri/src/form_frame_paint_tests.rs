use super::*;
const BOX: [f64; 4] = [0., 0., 100., 30.];
const RING: &str = "0 0 100 30 re 1 1 98 28 re f*";
const TEXT: &str = " BT /F0 12 Tf 1 0 0 1 4 12 Tm (Answer) Tj ET";
fn inspect(s: &str) -> EngineResult<FramePaint> {
    FramePaint::inspect(s.as_bytes(), BOX, false)
}
fn circle(cx: f64, cy: f64, r: f64) -> String {
    let k = r * 0.5522848;
    format!("{} {cy} m {} {} {} {} {cx} {} c {} {} {} {} {} {cy} c {} {} {} {} {cx} {} c {} {} {} {} {} {cy} c",
        cx+r,cx+r,cy+k,cx+k,cy+r,cy+r,cx-k,cy+r,cx-r,cy+k,cx-r,cx-r,cy-k,cx-k,cy-r,cy-r,cx+k,cy-r,cx+r,cy-k,cx+r)
}
#[test]
fn rectangle_ring_preserves_answer_and_art_operations() {
    let s = format!("q 0.9 g 0 0 100 30 re f 0 G 0 g {RING} 1 0 0 rg 8 8 3 3 re f {TEXT} Q");
    let p = inspect(&s).unwrap();
    assert_eq!(p.shape, FrameShape::Rectangle(BOX));
    assert_eq!(p.width, 1.);
    assert_eq!(p.style, PaintStyle::Solid);
    assert_eq!(p.color, PaintColor::Gray(0.));
    assert!(matches!(p.background_color,Some(PaintColor::Gray(v)) if (v-0.9).abs()<1e-6));
    assert_eq!(p.content.operations[p.border].operator, "f*");
    assert_eq!(p.content.operations[p.background.unwrap()].operator, "f");
    assert_eq!(
        p.content.encode().unwrap(),
        Content::decode_strict(s.as_bytes())
            .unwrap()
            .encode()
            .unwrap()
    );
}
#[test]
fn rectangle_stroke_and_dash_state_are_observed() {
    let p = inspect("q 0.1 0.2 0.3 RG 0.5 w [3 2] 1 d 0.25 0.25 99.5 29.5 re S Q").unwrap();
    assert_eq!(p.width, 0.5);
    assert_eq!(p.style, PaintStyle::Dashed(vec![3., 2.], 1.));
    assert!(matches!(p.color, PaintColor::Rgb(_)));
    assert!(p.background.is_none());
}
#[test]
fn nonzero_bbox_and_cmyk_are_retained() {
    let p = FramePaint::inspect(
        b"q 0 1 0 0 k 10 20 100 30 re 11 21 98 28 re f* Q",
        [10., 20., 110., 50.],
        false,
    )
    .unwrap();
    assert_eq!(p.shape, FrameShape::Rectangle([10., 20., 100., 30.]));
    assert_eq!(p.color, PaintColor::Cmyk([0., 1., 0., 0.]));
}
#[test]
fn labeled_radio_uses_small_circle_and_exact_bbox_clip() {
    let s = format!(
        "q 0 0 100 30 re W n 0 G 1 w {} S {} f {TEXT} Q",
        circle(8., 15., 5.),
        circle(8., 15., 2.5)
    );
    let p = FramePaint::inspect(s.as_bytes(), BOX, true).unwrap();
    assert_eq!(p.shape, FrameShape::Circle([8., 15., 5.5]));
    assert_eq!(p.width, 1.);
    assert!(p.background.is_none());
}
#[test]
fn radio_background_is_inner_disc_not_entire_caption_box() {
    let s = format!(
        "q 0.9 g {} f 0 G 1 w {} S Q",
        circle(8., 15., 5.5),
        circle(8., 15., 5.)
    );
    let p = FramePaint::inspect(s.as_bytes(), BOX, true).unwrap();
    assert!(p.background.is_some());
}
#[test]
fn pending_clip_on_border_cannot_be_replaced() {
    assert!(inspect("0 0 100 30 re 1 1 98 28 re W* f*").is_err());
}
#[test]
fn ambiguous_or_unproved_frames_refuse() {
    for s in [
        format!("{RING} {RING}"),
        "2 2 10 10 re S".into(),
        format!("2 0 0 2 0 0 cm {RING}"),
        format!("/G0 gs {RING}"),
        format!("/Pattern cs /P0 scn {RING}"),
        format!("0 0 90 30 re W n {RING}"),
        format!("BT /F0 12 Tf (label) Tj ET {RING}"),
    ] {
        assert!(inspect(&s).is_err(), "accepted {s}");
    }
}
#[test]
fn q_and_q_do_not_restore_path() {
    // The rectangle added in q survives Q; this is NOT a two-rectangle ring.
    assert!(inspect("0 0 100 30 re q 8 8 3 3 re Q 1 1 98 28 re f*").is_err());
    // Conversely a path begun in q survives Q and can form a valid ring.
    let p = inspect("q 0 0 100 30 re Q 1 1 98 28 re f*").unwrap();
    assert_eq!(p.width, 1.);
}
#[test]
fn restored_graphics_state_does_not_leak_into_frame() {
    let p = inspect(&format!("q 7 w 1 0 0 rg Q {RING}")).unwrap();
    assert_eq!(p.color, PaintColor::Gray(0.));
    assert_eq!(p.width, 1.);
}
#[test]
fn malformed_and_excessive_programs_refuse() {
    for s in [
        format!("q {RING}"),
        format!("Q {RING}"),
        format!("-1 w {RING}"),
        format!("[0 0] 0 d {RING}"),
        format!("[1 -2] 0 d {RING}"),
        format!("1 2 w {RING}"),
        format!("{RING} BT"),
        format!("{} {RING}", "q ".repeat(65)),
    ] {
        assert!(inspect(&s).is_err(), "accepted {s}");
    }
    assert!(FramePaint::inspect(RING.as_bytes(), [0., 0., f64::INFINITY, 30.], false).is_err());
    assert!(inspect(&" ".repeat(4 * 1024 * 1024 + 1)).is_err());
    assert!(inspect(&"n ".repeat(262145)).is_err());
    assert!(inspect(&format!("[{}] TJ {RING}", "0 ".repeat(100001))).is_err());
}

#[test]
fn frame_inside_answer_mark_is_not_independently_editable() {
    assert!(inspect(&format!("/Tx BMC {RING} EMC")).is_err());
}
#[test]
fn later_unknown_palette_cannot_hide_a_second_frame() {
    assert!(inspect(&format!("{RING} /Pattern cs /P scn {RING}")).is_err());
}
#[test]
fn underline_retains_physical_inset_and_butt_caps() {
    let p = inspect("q 2 w 0 G 0 1 m 100 1 l S Q").unwrap();
    assert_eq!(p.width, 2.);
    assert_eq!(p.style, PaintStyle::Underline);
    assert_eq!(p.shape, FrameShape::Rectangle(BOX));
    assert!(inspect("q 2 w 1 J 0 1 m 100 1 l S Q").is_err());
}
#[test]
fn dashed_round_caps_need_a_distinct_rendering_contract() {
    assert!(inspect("1 J 2 w [2 2] 0 d 1 1 98 28 re S").is_err());
}
#[test]
fn unproved_frame_never_becomes_unique_through_state_changes() {
    for s in [
        format!("{RING} q 0 0 90 30 re W n {RING} Q"),
        format!("{RING} q /G0 gs {RING} Q"),
        format!("{RING} q 2 0 0 2 0 0 cm {RING} Q"),
        format!("q 2 0 0 2 0 0 cm 0 0 100 30 re Q 1 1 98 28 re f*"),
        format!("0 0 90 30 re W n 0 0 100 30 re W n {RING}"),
    ] {
        assert!(inspect(&s).is_err(), "accepted {s}");
    }
}
#[test]
fn noncanonical_circle_and_duplicate_circles_refuse() {
    let c = circle(8., 15., 5.);
    assert!(FramePaint::inspect(format!("{c} S {c} S").as_bytes(), BOX, true).is_err());
    assert!(FramePaint::inspect(
        b"13 15 m 13 20 8 20 8 20 c 3 20 3 15 3 15 c 3 10 8 10 8 10 c 13 10 13 15 13 15 c S",
        BOX,
        true
    )
    .is_err());
}
#[test]
fn supported_synthetic_native_programs_all_classify() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("../tests/fixtures/form-frame-programs.json")).unwrap();
    let cases = cases.as_array().unwrap();
    assert_eq!(cases.len(), 19);
    for c in cases {
        let b = c["bounds"].as_array().unwrap();
        let bounds = std::array::from_fn(|i| b[i].as_f64().unwrap());
        let p = FramePaint::inspect(
            c["program"].as_str().unwrap().as_bytes(),
            bounds,
            c["radio"].as_bool().unwrap(),
        );
        assert!(p.is_ok(), "{}: {:?}", c["case"], p.err());
    }
}

#[test]
fn native_compound_frames_are_one_verified_border() {
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "../tests/fixtures/form-frame-compound-programs.json"
    ))
    .unwrap();
    assert_eq!(cases.as_array().unwrap().len(), 14);
    for c in cases.as_array().unwrap() {
        let bounds = std::array::from_fn(|i| c["bounds"][i].as_f64().unwrap());
        let p = FramePaint::inspect(c["program"].as_str().unwrap().as_bytes(), bounds, false);
        assert!(p.is_ok(), "{}: {:?}", c["case"], p.err());
        let p = p.unwrap();
        assert!((p.width - 1.25).abs() < 0.0001);
        assert_eq!(format!("{:?}", p.style), "Compound");
    }
}
#[test]
fn compound_edge_pair_must_match_outer_border_geometry() {
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "../tests/fixtures/form-frame-compound-programs.json"
    ))
    .unwrap();
    let s = cases[0]["program"].as_str().unwrap();
    for changed in [
        s.replacen("1.25 1.25 m", "1.5 1.25 m", 1),
        s.replacen(
            "0 g\n0 0 280 50 re\n1.25 1.25 277.5 47.5 re\nf*",
            "0 g\n0 0 280 50 re\n1 1 278 48 re\nf*",
            1,
        ),
        s.replacen("0.425 g", "/G0 gs 0.425 g", 1),
    ] {
        assert!(FramePaint::inspect(changed.as_bytes(), [0., 0., 280., 50.], false).is_err());
    }
}

#[test]
fn zero_width_clip_path_is_not_an_invisible_border() {
    let p = inspect(&format!("q 0 w 0 0 100 30 re W n {RING} Q")).unwrap();
    assert_eq!(p.width, 1.);
    assert_eq!(p.style, PaintStyle::Solid);
}

#[test]
fn invisible_geometry_is_unique_and_still_requires_known_state() {
    for s in [
        "q 0 w 0 0 100 30 re n Q q 0 w 0 0 100 30 re n Q",
        "q /G0 gs 0 w 0 0 100 30 re n Q",
        "q 2 0 0 2 0 0 cm 0 w 0 0 100 30 re n Q",
        "/Tx BMC q 0 w 0 0 100 30 re n Q EMC",
    ] {
        assert!(inspect(s).is_err());
    }
}
