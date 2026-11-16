use super::*;
fn glyph(code: u16, width: f32, adjustable: bool) -> Glyph {
    Glyph {
        code,
        width,
        nominal: width,
        adjustable,
    }
}
fn residue(width: f32) -> f64 {
    f64::from(width) - f64::from((width + 0.5).floor())
}
fn check(before: &[f32], after: &[Glyph], p: &Plan) {
    let old_r = before.iter().map(|w| residue(*w)).sum::<f64>();
    let mut new_r = 0.;
    let mut integer = 0.;
    for g in after {
        let w = p
            .changes
            .iter()
            .find(|(c, _)| *c == g.code)
            .map_or(g.width, |(_, w)| *w);
        if !g.adjustable {
            assert_eq!(w, g.width);
        }
        assert!(w.is_finite() && (0.0..=10000.).contains(&w) && (w - g.nominal).abs() <= 1.01);
        new_r += residue(w);
        integer += f64::from((w + 0.5).floor());
    }
    assert_eq!(old_r, new_r, "Different reader residues remain");
    integer -= before
        .iter()
        .map(|w| f64::from((*w + 0.5).floor()))
        .sum::<f64>();
    assert_eq!(p.width_delta, integer);
}
#[test]
fn unchanged_glyphs_keep_exact_widths_without_new_resource() {
    let before = [787.109375, 966.796875, 549.8046875, 615.234375];
    let after = before
        .iter()
        .enumerate()
        .map(|(i, w)| glyph(i as u16, *w, false))
        .collect::<Vec<_>>();
    let p = plan(&before, &after).unwrap();
    assert!(p.changes.is_empty());
    assert_eq!(p.width_delta, 0.);
    check(&before, &after, &p);
}
#[test]
fn office_ligature_decomposition_changes_only_new_codes() {
    let before = [787.109375, 966.796875, 549.8046875, 615.234375];
    let after = [
        glyph(50, 787.109375, false),
        glyph(73, 352.05078125, true),
        glyph(76, 277.83203125, true),
        glyph(70, 549.8046875, false),
        glyph(72, 615.234375, false),
    ];
    let snapshot = after.iter().map(|g| g.width).collect::<Vec<_>>();
    let p = plan(&before, &after).unwrap();
    assert!(!p.changes.is_empty());
    assert!(p.changes.iter().all(|(c, _)| [73, 76].contains(c)));
    check(&before, &after, &p);
    assert_eq!(snapshot, after.iter().map(|g| g.width).collect::<Vec<_>>());
}
#[test]
fn repeated_new_codes_use_one_consistent_weighted_width() {
    let before = [500.0625];
    let after = vec![glyph(7, 500., true); 2];
    let p = plan(&before, &after).unwrap();
    assert_eq!(p.changes.len(), 1);
    check(&before, &after, &p);
}
#[test]
fn multiple_new_codes_can_absorb_more_than_one_code_capacity() {
    let before = [500.25; 3];
    let after = [glyph(1, 500., true), glyph(2, 500., true)];
    let p = plan(&before, &after).unwrap();
    assert_eq!(p.changes.len(), 2);
    check(&before, &after, &p);
}
#[test]
fn weighted_representability_repair_uses_two_codes() {
    let before = [500f32.next_up()];
    let after = [
        glyph(7, 500., true),
        glyph(7, 500., true),
        glyph(8, 500., true),
        glyph(8, 500., true),
        glyph(8, 500., true),
    ];
    let p = plan(&before, &after).unwrap();
    assert_eq!(p.changes.len(), 2);
    check(&before, &after, &p);
}

#[test]
fn retries_share_the_remaining_repair_budget_without_resetting_it() {
    let before = [500f32.next_up()];
    let after = [glyph(7,500.,true),glyph(7,500.,true),
        glyph(8,500.,true),glyph(8,500.,true),glyph(8,500.,true)];
    let mut budget = 1;
    let repaired = plan_with_budget(&before,&after,&mut budget).unwrap();
    check(&before,&after,&repaired);
    assert_eq!(budget,0);
    assert!(plan_with_budget(&before,&after,&mut budget).is_err());
    assert_eq!(budget,0);
    // An already exact request needs no search and may still succeed.
    assert!(plan_with_budget(&[500.],&[glyph(1,500.,false)],&mut budget).is_ok());
    assert_eq!(budget,0);
}

#[test]
fn a_caller_cannot_raise_the_fixed_repair_budget() {
    let mut budget = REPAIR_BUDGET+1;
    assert!(plan_with_budget(&[500.],&[glyph(1,500.,false)],&mut budget).is_err());
    assert_eq!(budget,REPAIR_BUDGET+1);
}
#[test]
fn newly_introduced_glyph_can_cross_an_integer_width_bin() {
    let before = [0.5];
    let after = [glyph(3, 0., true)];
    let p = plan(&before, &after).unwrap();
    check(&before, &after, &p);
}
#[test]
fn retained_glyphs_and_insufficient_capacity_refuse_without_mutation() {
    assert!(plan(&[500.25], &[glyph(1, 500., false)]).is_err());
    assert!(plan(&[500.25; 5], &[glyph(1, 500., true)]).is_err());
    let after = [glyph(3, 500., false), glyph(3, 500., false)];
    let before = after.iter().map(|g| g.width).collect::<Vec<_>>();
    assert!(plan(&[500.25], &after).is_err());
    assert_eq!(before, after.iter().map(|g| g.width).collect::<Vec<_>>());
}
#[test]
fn invalid_and_inconsistent_maps_are_rejected_before_planning() {
    for width in [f32::NAN, f32::INFINITY, -1., 10001.] {
        assert!(plan(&[width], &[glyph(1, 500., true)]).is_err());
        assert!(plan(&[500.], &[glyph(1, width, true)]).is_err());
    }
    assert!(plan(&[500.], &[glyph(1, 500., true), glyph(1, 501., true)]).is_err());
    assert!(plan(&[500.], &[glyph(1, 500., true), glyph(1, 500., false)]).is_err());
    assert!(plan(
        &[500.],
        &[Glyph {
            code: 1,
            width: 500.,
            nominal: 490.,
            adjustable: true
        }]
    )
    .is_err());
    assert!(plan(&[], &[glyph(1, 500., true)]).is_err());
    assert!(plan(&[500.], &[]).is_err());
    assert!(plan(&vec![500.; 4097], &[glyph(1, 500., true)]).is_err());
    assert!(plan(&[500.], &vec![glyph(1, 500., true); 4097]).is_err());
}
#[test]
fn plan_is_deterministic_when_code_occurrences_are_reordered() {
    let before = [500.25; 3];
    let a = [glyph(8, 500., true), glyph(7, 500., true)];
    let mut b = a.clone();
    b.reverse();
    assert_eq!(
        plan(&before, &a).unwrap().changes,
        plan(&before, &b).unwrap().changes
    );
}
#[test]
fn small_weighted_width_cases_match_an_independent_integer_oracle() {
    let step = 1. / 32768.;
    for a in 1..=3 {
        for b in 1..=3 {
            for units in -4i32..=4 {
                let before = if units == 0 {
                    vec![500.]
                } else {
                    vec![500. + 0.25 * units.signum() as f32; units.unsigned_abs() as usize]
                };
                let mut after = vec![glyph(1, 500., true); a];
                after.extend(vec![glyph(2, 500., true); b]);
                let required = i64::from(units) * 8192;
                let feasible = (-16384i64..16384).any(|x| {
                    let remainder = required - a as i64 * x;
                    remainder % b as i64 == 0 && (-16384..16384).contains(&(remainder / b as i64))
                });
                let result = plan(&before, &after);
                assert_eq!(
                    result.is_ok(),
                    feasible,
                    "a={a},b={b},target={units}quarters,step={step}"
                );
                if let Ok(p) = result {
                    check(&before, &after, &p);
                }
            }
        }
    }
}
