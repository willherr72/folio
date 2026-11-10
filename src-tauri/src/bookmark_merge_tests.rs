use super::*;
fn item(id: &str) -> DocumentBookmark {
    DocumentBookmark {
        id: id.into(),
        title: format!("Title {id}"),
        level: 0,
        destination: Some(PdfValue::Array(vec![
            PdfValue::Page("0".into()),
            PdfValue::Name(b"Fit".to_vec()),
        ])),
        action: None,
        bold: false,
        italic: false,
        closed: false,
        color: None,
    }
}
fn map(value: &str) -> HashMap<String, String> {
    HashMap::from([("0".into(), value.into())])
}
fn check() -> EngineResult<()> {
    Ok(())
}
#[test]
fn bookmark_selected_merge_prunes_missing_targets_and_keeps_parent_of_surviving_child() {
    let mut items=vec![item("parent"),item("child"),item("drop")];
    items[1].level=1;items[1].destination=Some(PdfValue::Array(vec![PdfValue::Page("1".into()),PdfValue::Name(b"Fit".to_vec())]));
    let mapping=HashMap::from([("1".into(),"kept".into())]);
    let output=super::remap_selected([Ok((items.as_slice(),mapping.clone(),2))],&mut check).unwrap();
    assert_eq!(output,remap(&items,&mapping));assert_eq!(output.len(),2);
    assert!(output[0].destination.is_none());assert_eq!(primary_page(&output[1]),Some("kept"));
}

#[test]
fn bookmark_selected_merge_checks_aggregate_budget_original_targets_and_cancellation() {
    let items=vec![item("one")];
    assert!(super::remap_selected([Ok((items.as_slice(),HashMap::new(),1))],&mut check).unwrap().is_empty());
    assert!(super::remap_selected([Ok((items.as_slice(),map(&"x".repeat(MAX_BYTES)),1))],&mut check).is_err());
    assert!(super::remap_selected([Ok((items.as_slice(),map("kept"),0))],&mut check).is_err());
    let mut bad_items=items.clone();bad_items[0].destination=Some(PdfValue::Array(vec![PdfValue::Page("999".into()),PdfValue::Name(b"Fit".to_vec())]));
    assert!(super::remap_selected([Ok((bad_items.as_slice(),HashMap::new(),1))],&mut check).is_err());
    assert!(super::remap_selected([Ok((items.as_slice(),map("kept"),1)),Ok((items.as_slice(),map("other"),1))],&mut check).is_err());
    let mut calls=0;let error=super::remap_selected([Ok((items.as_slice(),map("kept"),1))],&mut ||{calls+=1;if calls>2{Err(bad("cancelled"))}else{Ok(())}}).unwrap_err();assert!(error.to_string().contains("cancelled"));
}
fn remap_complete(
    sources: &[(&[DocumentBookmark], HashMap<String, String>)],
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<Vec<DocumentBookmark>> {
    super::remap_complete(
        sources.iter().map(|(items, map)| Ok((*items, map.clone()))),
        check,
    )
}
#[test]
fn bookmark_complete_merge_preserves_source_order_styles_and_remapped_actions() {
    let mut a = vec![item("a"), item("child")];
    a[0].bold = true;
    a[0].closed = true;
    a[1].level = 1;
    a[1].color = Some([0., 0.5, 1.]);
    let mut b = vec![item("b")];
    b[0].destination = None;
    b[0].action = Some(PdfValue::Dictionary(vec![
        (b"S".to_vec(), PdfValue::Name(b"GoTo".to_vec())),
        (b"D".to_vec(), item("unused").destination.unwrap()),
        (b"Next".to_vec(), PdfValue::Null),
    ]));
    let output = remap_complete(&[(&a, map("first")), (&b, map("second"))], &mut check).unwrap();
    let expected = remap(&a, &map("first"))
        .into_iter()
        .chain(remap(&b, &map("second")))
        .collect::<Vec<_>>();
    assert_eq!(output, expected);
    validate(&output, &HashSet::from(["first".into(), "second".into()])).unwrap();
}
#[test]
fn bookmark_complete_merge_checks_aggregate_bytes_mapped_keys_and_counts_before_copying() {
    let large = |id: &str| {
        let mut b = item(id);
        b.action = Some(PdfValue::Dictionary(vec![
            (b"S".to_vec(), PdfValue::Name(b"JavaScript".to_vec())),
            (
                b"JS".to_vec(),
                PdfValue::String(vec![b'x'; 3 * 1024 * 1024]),
            ),
        ]));
        vec![b]
    };
    let a = large("a");
    let b = large("b");
    assert!(remap_complete(&[(&a, map("first"))], &mut check).is_ok());
    assert!(remap_complete(&[(&a, map("first")), (&b, map("second"))], &mut check).is_err());
    assert!(remap_complete(&[(&[item("a")], map(&"x".repeat(MAX_BYTES)))], &mut check).is_err());
    let a = (0..MAX_BOOKMARKS)
        .map(|n| item(&format!("b{n}")))
        .collect::<Vec<_>>();
    assert_eq!(
        remap_complete(&[(&a, map("first"))], &mut check)
            .unwrap()
            .len(),
        MAX_BOOKMARKS
    );
    assert!(remap_complete(
        &[(&a, map("first")), (&[item("extra")], map("second"))],
        &mut check
    )
    .is_err());
}
#[test]
fn bookmark_complete_merge_rejects_missing_targets_duplicate_ids_and_cancels() {
    let a = vec![item("a")];
    assert!(remap_complete(&[(&a, HashMap::new())], &mut check).is_err());
    assert!(remap_complete(&[(&a, map("first")), (&a, map("second"))], &mut check).is_err());
    let before = a.clone();
    let mut calls = 0;
    remap_complete(&[(&a, map("first"))], &mut || {
        calls += 1;
        Ok(())
    })
    .unwrap();
    for stop in [1, calls / 2, calls] {
        let mut n = 0;
        assert!(remap_complete(&[(&a, map("first"))], &mut || {
            n += 1;
            if n == stop {
                Err(bad("cancel"))
            } else {
                Ok(())
            }
        })
        .is_err());
    }
    assert_eq!(a, before);
}

#[test]
fn bookmark_complete_merge_stops_before_reading_later_sources_on_budget_refusal() {
    let mut a = item("a");
    a.action = Some(PdfValue::Dictionary(vec![(
        b"JS".to_vec(),
        PdfValue::String(vec![b'x'; 3 * 1024 * 1024]),
    )]));
    let mut b = a.clone();
    b.id = "b".into();
    let a = vec![a];
    let b = vec![b];
    let mut reads = 0;
    let sources = (0..3).map(|index| {
        reads += 1;
        assert!(
            index < 2,
            "later source must remain unread after budget refusal"
        );
        Ok((
            if index == 0 {
                a.as_slice()
            } else {
                b.as_slice()
            },
            map("page"),
        ))
    });
    assert!(super::remap_complete(sources, &mut check).is_err());
    assert_eq!(reads, 2);
}
