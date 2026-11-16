use super::*;
fn s(bytes: &[u8]) -> Object {
    Object::String(bytes.to_vec(), StringFormat::Hexadecimal)
}

#[test]
fn source_clusters_same_unicode_substitution_maps_ligature_slots_to_letters() {
    let source = [0, 1, 0, 21, 0, 2, 0, 3];
    let changed = [0, 50, 0, 73, 0, 73, 0, 76, 0, 70, 0, 72];
    let values = vec![
        25.into(),
        s(&source[..2]),
        (-20).into(),
        s(&source[2..4]),
        10.into(),
        s(&source[4..]),
        45.into(),
    ];
    let result = rewrite_clusters(
        &values,
        &source,
        &changed,
        &[0, 2, 4, 6, 8],
        &[0, 2, 4, 6, 8, 10, 12],
        &[0, 1, 4, 5, 6],
        &[0, 1, 2, 3, 4, 5, 6],
        true,
    )
    .unwrap();
    assert_eq!(
        result,
        vec![
            25.into(),
            s(&changed[..2]),
            (-20).into(),
            s(&changed[2..8]),
            10.into(),
            s(&changed[8..]),
            45.into()
        ]
    );
}

#[test]
fn source_clusters_noop_and_growth_keep_exact_source_atoms_and_adjustments() {
    let source = [0, 1, 0, 21, 0, 2, 0, 3];
    let changed = [0, 1, 0, 21, 0, 2, 0, 3, 0, 4, 0, 5];
    let values = vec![s(&source[..4]), (-20).into(), s(&source[4..]), 45.into()];
    let before = [0, 2, 4, 6, 8];
    let logical = [0, 1, 4, 5, 6];
    assert_eq!(
        rewrite_clusters(&values, &source, &source, &before, &before, &logical, &logical, true)
            .unwrap(),
        values
    );
    assert_eq!(
        rewrite_clusters(
            &values,
            &source,
            &changed,
            &before,
            &[0, 2, 4, 6, 8, 10, 12],
            &logical,
            &[0, 1, 4, 5, 6, 7, 8],
            false
        )
        .unwrap(),
        vec![
            s(&changed[..4]),
            (-20).into(),
            s(&changed[4..8]),
            45.into(),
            s(&changed[8..])
        ]
    );
}

#[test]
fn source_clusters_shortening_clamps_slots_without_splitting_new_glyphs() {
    let source = [0, 1, 0, 2, 0, 3, 0, 4];
    let changed = [0, 9];
    let values = vec![
        s(&source[..2]),
        (-20).into(),
        s(&source[2..4]),
        10.into(),
        s(&source[4..]),
    ];
    assert_eq!(
        rewrite_clusters(
            &values,
            &source,
            &changed,
            &[0, 2, 4, 6, 8],
            &[0, 2],
            &[0, 1, 2, 3, 4],
            &[0, 1],
            false
        )
        .unwrap(),
        vec![s(&changed), s(&[]), s(&[])]
    );
    assert!(rewrite_clusters(
        &[s(&source[..2]), 10.into(), s(&source[2..])],
        &source,
        &changed,
        &[0, 2, 4, 6, 8],
        &[0, 2],
        &[0, 1, 2, 3, 4],
        &[0, 2],
        false
    )
    .is_err());
}

#[test]
fn source_clusters_validate_both_byte_and_logical_boundary_maps() {
    let source = [0, 1, 0, 2];
    let values = vec![s(&source)];
    for (before, after, logical_before, logical_after) in [
        (vec![0, 2, 4], vec![0, 2, 4], vec![0, 2], vec![0, 1, 2]),
        (vec![0, 2, 4], vec![0, 2, 4], vec![0, 1, 2], vec![0, 1]),
        (vec![0, 2, 4], vec![0, 2, 4], vec![1, 2, 3], vec![0, 1, 2]),
        (vec![0, 2, 4], vec![0, 2, 4], vec![0, 1, 1], vec![0, 1, 2]),
        (vec![0, 2, 4], vec![0, 4], vec![0, 1, 2], vec![0, 4097]),
        (vec![0, 2, 5], vec![0, 2, 4], vec![0, 1, 2], vec![0, 1, 2]),
    ] {
        assert!(rewrite_clusters(
            &values,
            &source,
            &source,
            &before,
            &after,
            &logical_before,
            &logical_after,
            true
        )
        .is_err());
    }
    assert!(rewrite_clusters(
        &[s(&source[..1]), 10.into(), s(&source[1..])],
        &source,
        &source,
        &[0, 2, 4],
        &[0, 2, 4],
        &[0, 1, 2],
        &[0, 1, 2],
        true
    )
    .is_err());
    assert!(rewrite_clusters(
        &[s(&source), Object::Real(f32::NAN)],
        &source,
        &source,
        &[0, 2, 4],
        &[0, 2, 4],
        &[0, 1, 2],
        &[0, 1, 2],
        true
    )
    .is_err());
}

#[test]
fn source_clusters_contiguous_strings_and_zero_gaps_do_not_create_cuts() {
    let source = [0, 1, 0, 21, 0, 2, 0, 3];
    let changed = [0, 1, 0, 1, 0, 21, 0, 2, 0, 3];
    for gaps in [vec![], vec![0.into()], vec![7.into(), (-7).into()]] {
        let mut values = vec![s(&source[..2])];
        values.extend(gaps.clone());
        values.push(s(&source[2..4]));
        values.extend(gaps);
        values.push(s(&source[4..]));
        assert_eq!(rewrite_clusters(&values, &source, &source,
            &[0,2,4,6,8], &[0,2,4,6,8], &[0,1,4,5,6], &[0,1,4,5,6], true).unwrap(), values);
        let result = rewrite_clusters(&values, &source, &changed,
            &[0,2,4,6,8], &[0,2,4,6,8,10], &[0,1,4,5,6], &[0,1,2,5,6,7], false).unwrap();
        assert_eq!(result.iter().flat_map(|v|v.as_str().unwrap().iter().copied()).collect::<Vec<_>>(), changed);
    }
    // A real gap at the old logical offset4 still cannot split the new ffi.
    assert!(rewrite_clusters(&[s(&source[..4]), 7.into(), s(&source[4..])], &source, &changed,
        &[0,2,4,6,8], &[0,2,4,6,8,10], &[0,1,4,5,6], &[0,1,2,5,6,7], false).is_err());
}

#[test]
fn source_clusters_terminal_string_does_not_require_a_replacement_cut() {
    // One old scalar replaced by one three-scalar glyph, with no actual gap.
    let source = [0, 1];
    let changed = [0, 21];
    assert_eq!(rewrite_clusters(&[s(&source)], &source, &changed,
        &[0,2], &[0,2], &[0,1], &[0,3], false).unwrap(), vec![s(&changed)]);
    // A trailing real adjustment still retains its original logical ownership.
    assert!(rewrite_clusters(&[s(&source), 7.into()], &source, &changed,
        &[0,2], &[0,2], &[0,1], &[0,3], false).is_err());
}
