use super::*;
use lopdf::{content::Content, Object};

fn parse(source: &str) -> PageContext {
    inspect(
        &Content::decode_strict(source.as_bytes())
            .unwrap()
            .operations,
    )
    .unwrap()
}
fn first(source: &str) -> ShowContext {
    parse(source).shows.remove(0).1
}

#[test]
fn opaque_unselected_marks_compare_exactly_without_becoming_edit_authority() {
    let source="/Artifact << /Type /Pagination /Subtype /Header >> BDC q 0 0 100 100 re W n BT (header) Tj ET Q EMC";
    let original=first(source); assert!(original.binding().is_err()); assert!(original.mcid().is_err());
    assert!(!original.contains([1.,1.,2.,2.])); assert!(original.clip_prefix().is_err());
    let preserved=original.preserved_binding().unwrap();
    assert_eq!(preserved,first(source).preserved_binding().unwrap());
    for changed in [source.replace("Header","Footer"),source.replace("100 100","99 100"),
        source.replace("/Pagination","/Layout"),source.replace("/Header","/Header /ActualText (different)")] {
        assert_ne!(preserved,first(&changed).preserved_binding().unwrap());
    }
    for unknown in [
        "0 0 m 10 0 l 0 10 l h W n BT (header) Tj ET",
        "0 0 0 0 re W n BT (header) Tj ET",
        "BT 7 Tr (header) Tj ET",
    ] { assert!(first(unknown).preserved_binding().is_err()); }
}

#[test]
fn nested_clip_intersection_and_graphics_restore() {
    let page = parse("q 0 0 100 100 re W n q 10 20 80 60 re W* n BT (inner) Tj ET Q BT (outer) Tj ET Q BT (plain) Tj ET");
    assert_eq!(page.shows.len(), 3);
    assert!(page.shows[0].1.contains([10., 20., 90., 80.]));
    assert!(!page.shows[0].1.contains([9.999999, 20., 90., 80.]));
    assert!(page.shows[1].1.contains([0., 0., 100., 100.]));
    assert!(!page.shows[1].1.contains([0., 0., 101., 100.]));
    assert!(page.shows[2].1.contains([-100., -100., 101., 101.]));
}

#[test]
fn pending_clip_and_path_survive_q_but_use_construction_transform() {
    let show = first("q 2 0 0 3 10 20 cm 0 0 40 20 re W Q n BT (text) Tj ET");
    assert!(show.contains([10., 20., 90., 80.]));
    assert!(!show.contains([0., 0., 40., 20.]));
    let show = first("0 0 40 20 re q 2 0 0 3 10 20 cm W n Q BT (text) Tj ET");
    assert!(show.contains([-1., -1., 41., 21.])); // Q discards the applied clip.
}

#[test]
fn transformed_rectangles_and_replayed_recipes_preserve_bounds() {
    for (source, bounds) in [
        (
            "q -2 0 0 3 100 20 cm 5 5 20 10 re W* n BT (text) Tj ET Q",
            [50., 35., 90., 65.],
        ),
        (
            "q 0 2 -3 0 100 20 cm 5 5 20 10 re W n BT (text) Tj ET Q",
            [55., 30., 85., 70.],
        ),
        (
            "q 1 0 0 1 0.00000912 0 cm 0 0 612 792 re W* n BT (text) Tj ET Q",
            [0.00001, 0., 612., 792.],
        ),
    ] {
        let show = first(source);
        assert!(show.contains(bounds), "{source}");
        assert!(!show.contains([bounds[0] - 1., bounds[1], bounds[2], bounds[3]]));
        let mut operations = show.clip_prefix().unwrap();
        operations.extend(
            Content::decode_strict(b"BT (generated) Tj ET Q")
                .unwrap()
                .operations,
        );
        let replay = inspect(&operations).unwrap();
        assert!(replay.shows[0].1.contains(bounds));
        assert_eq!(
            show.binding().unwrap(),
            replay.shows[0].1.binding().unwrap()
        );
    }
}

#[test]
fn generated_clips_consume_paths_under_their_construction_transform() {
    for source in [
        "q 1.25 0 0 1.25 12 -20 cm 45 540 100 90 re W n BT (text) Tj ET Q",
        "q 2 0 0 3 10 20 cm 0 0 40 40 re W n 1 0 0 1 5 5 cm 0 0 20 20 re W* n BT (text) Tj ET Q",
        "q -2 0 0 3 100 20 cm 5 5 20 10 re W* n BT (text) Tj ET Q",
        "q 0 2 -3 0 100 20 cm 5 5 20 10 re W n BT (text) Tj ET Q",
    ] {
        let show = first(source);
        let mut operations = show.clip_prefix().unwrap();
        operations.extend(Content::decode_strict(b"BT (generated) Tj ET Q").unwrap().operations);
        let mut ctm = IDENTITY;
        let mut stack = Vec::new();
        let mut construction = None;
        let mut pending = false;
        for op in &operations {
            match op.operator.as_str() {
                "q" => stack.push(ctm),
                "Q" => ctm = stack.pop().unwrap(),
                "cm" => ctm = product(ctm, numbers::<6>(op).unwrap()).unwrap(),
                "re" => construction = Some(ctm),
                "W" | "W*" => pending = true,
                "n" if pending => {
                    assert_eq!(construction.take().unwrap(), ctm,
                        "Applying a clip under a different CTM hides text in independent readers: {source}");
                    pending = false;
                }
                "Tj" => for (actual, expected) in ctm.into_iter().zip(IDENTITY) {
                    assert!((actual-expected).abs() < 0.0001, "generated text CTM drift: {ctm:?}");
                },
                _ => {}
            }
        }
        let replay = inspect(&operations).unwrap();
        assert_eq!(show.binding().unwrap(), replay.shows[0].1.binding().unwrap());
    }
}

#[test]
fn generated_clip_envelopes_are_stable_and_recognize_exact_legacy_ownership() {
    for source in [
        "q 1.25 0 0 1.25 12 -20 cm 45 540 100 90 re W n BT (text) Tj ET Q",
        "q 2 0 0 3 10 20 cm 0 0 40 40 re W n 1 0 0 1 5 5 cm 0 0 20 20 re W* n BT (text) Tj ET Q",
        // Two disjoint construction histories whose exact intersection can be
        // represented without rounding. The canonical fallback must re-edit.
        "q 2 0 0 2 0 0 cm 0 0 100 100 re Q W n q 3 0 0 3 0 0 cm 10 10 40 40 re Q W n BT (text) Tj ET",
    ] {
        let show = first(source);
        let prefix = show.clip_prefix().unwrap();
        let mut operations = prefix.clone();
        operations.extend(Content::decode_strict(b"BT (generated) Tj ET Q").unwrap().operations);
        // Saved content parses integral operands as Integer, even when an
        // emitter originally supplied Real. Ownership must survive real bytes.
        let operations = Content::decode_strict(&Content{operations:operations.as_slice()}.encode().unwrap()).unwrap().operations;
        assert_eq!(show.owned_clip_prefix_start(&operations,prefix.len()).unwrap(),0);
        let replay = inspect(&operations).unwrap();
        assert_eq!(show.binding().unwrap(), replay.shows[0].1.binding().unwrap());
        let again = replay.shows[0].1.clip_prefix().unwrap();
        assert_eq!(Content{operations:prefix.as_slice()}.encode().unwrap(), Content{operations:again.as_slice()}.encode().unwrap());
        for envelope in [prefix, show.legacy_clip_prefix().unwrap()] {
            let mut page = vec![Operation::new("n",vec![])];
            page.extend(envelope);
            assert_eq!(show.owned_clip_prefix_start(&page,page.len()).unwrap(),1);
            let rectangle = page.iter_mut().find(|op|op.operator=="re").unwrap();
            rectangle.operands[0] = Object::Real(999.);
            assert!(show.owned_clip_prefix_start(&page,page.len()).is_err(), "Changed clip is not owned");
        }
    }
}

#[test]
fn current_clip_ownership_does_not_expand_an_unneeded_legacy_history() {
    let source = format!("{}{}BT (text) Tj ET",
        "1 0 0 1 0 0 cm ".repeat(400), "0 0 100 100 re W n ".repeat(300));
    let show = first(&source);
    let prefix = show.clip_prefix().unwrap();
    assert!(prefix.len() < 2000);
    assert!(show.legacy_clip_prefix().is_err(), "legacy expansion exceeds its operation budget");
    assert_eq!(show.owned_clip_prefix_start(&prefix, prefix.len()).unwrap(), 0);
}

#[test]
fn unknown_empty_and_text_clips_never_authorize_text() {
    for prefix in [
        "0 0 m 20 0 l 20 20 l h W n",
        "0 0 20 20 re 30 30 20 20 re W n",
        "0 0 20 20 re W n 30 30 20 20 re W n",
        "0 0 0 20 re W n",
        "1 0.1 0 1 0 0 cm 0 0 20 20 re W n",
        "BT 7 Tr (clip) Tj ET 0 Tr",
    ] {
        let page = parse(&format!("{prefix} BT (target) Tj ET"));
        let show = &page.shows.last().unwrap().1;
        assert!(!show.contains([1., 1., 2., 2.]), "{prefix}");
        assert!(show.binding().is_err(), "{prefix}");
        assert!(show.clip_prefix().is_err(), "{prefix}");
    }
    let page = parse("q BT 7 Tr (clip) Tj ET Q BT (target) Tj ET");
    assert!(!page.shows[0].1.contains([1., 1., 2., 2.]));
    assert!(page.shows[1].1.contains([1., 1., 2., 2.]));
}

#[test]
fn marked_scope_identity_language_and_cursor_only_shows() {
    let page=parse("/Span << /MCID 0 /Lang (en-US) >> BDC BT [(a)] TJ [20 -5] TJ (b) Tj ET EMC /Span << /MCID 0 /Lang (en-US) >> BDC BT (c) Tj ET EMC");
    assert_eq!(page.shows.len(), 3);
    assert_eq!(page.marked_events.len(), 4);
    assert_eq!(
        page.shows[0].1.binding().unwrap(),
        page.shows[1].1.binding().unwrap()
    );
    assert_ne!(
        page.shows[0].1.binding().unwrap(),
        page.shows[2].1.binding().unwrap()
    );
    let other = first("/Span << /MCID 0 /Lang (fr-FR) >> BDC BT (a) Tj ET EMC");
    assert_ne!(page.shows[0].1.binding().unwrap(), other.binding().unwrap());
    for prefix in [
        "/Span << /MCID 0 /ActualText (other) >> BDC",
        "/Span /Named BDC",
        "/Span << /MCID -1 >> BDC",
    ] {
        let show = first(&format!("{prefix} BT (a) Tj ET EMC"));
        assert!(!show.contains([0., 0., 1., 1.]));
        assert!(show.binding().is_err());
    }
}

#[test]
fn rejects_malformed_state_and_resource_amplification() {
    for source in [
        "Q",
        "q",
        "EMC",
        "/Span BMC",
        "BT",
        "ET",
        "BT BT ET ET",
        "1 Tr BT (a) Tj",
        "0 0 re",
        "W n",
        "BT 8 Tr (a) Tj ET",
        "BT [ (a) /bad ] TJ ET",
    ] {
        let operations = Content::decode_strict(source.as_bytes())
            .unwrap()
            .operations;
        assert!(inspect(&operations).is_err(), "{source}");
    }
    assert!(inspect(&vec![Operation::new("q", vec![]); 65]).is_err());
    assert!(inspect(&vec![Operation::new("n", vec![]); 100_001]).is_err());
    let invalid = Operation::new(
        "cm",
        vec![
            Object::Real(f32::INFINITY),
            0.into(),
            0.into(),
            1.into(),
            0.into(),
            0.into(),
        ],
    );
    assert!(inspect(&[invalid]).is_err());
}
