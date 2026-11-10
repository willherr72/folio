use super::*;
use lopdf::{dictionary, Document, Object, Stream};

fn source() -> Arc<[u8]> {
    Arc::from(b"immutable source bytes".as_slice())
}
fn highlight(id: &str, x: f32) -> Overlay {
    Overlay::Highlight(HighlightOverlay {
        id: id.into(),
        rects: vec![AnnotationRect {
            x,
            y: 20.,
            width: 50.,
            height: 12.,
        }],
        color: "#FFFF00".into(),
        text: Some("Original note".into()),
        opacity: None,
    })
}
fn captured(a: &str, b: &str) -> Index {
    let mut index = Index::capturing(identity(source()));
    assert!(index.capture(0, 1, Some((7, 0)), &[2], &highlight(a, 10.)));
    assert!(index.capture(0, 3, None, &[], &highlight(b, 80.)));
    index
}

#[test]
fn imported_overlay_sources_capture_final_ids_slots_popups_and_semantics() {
    let first = captured("first", "second").snapshot().unwrap();
    assert_eq!(first.bindings.len(), 2);
    assert_eq!(
        (
            first.bindings[0].annotation_index,
            first.bindings[0].annotation_id
        ),
        (1, Some((7, 0)))
    );
    assert_eq!(first.bindings[0].popup_indices, vec![2]);
    assert_eq!(first.bindings[1].annotation_id, None);
    assert_eq!(
        first.source_sha256,
        Sha256::digest(source())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let second = captured("new-first", "new-second").snapshot().unwrap();
    assert_eq!(
        first.bindings[0].initial_semantics_sha256, second.bindings[0].initial_semantics_sha256,
        "workspace labels do not change annotation semantics"
    );
    assert_ne!(
        first.bindings[0].initial_semantics_sha256, first.bindings[1].initial_semantics_sha256,
        "placement is part of the baseline"
    );
}

#[test]
fn imported_overlay_sources_validated_rebind_preserves_original_workspace_ids() {
    let original = captured("saved-first", "saved-second").snapshot().unwrap();
    let mut encoded = serde_json::to_vec(&original).unwrap();
    let saved: Snapshot = serde_json::from_slice(&encoded).unwrap();
    encoded.clear();
    let current = captured("fresh-first", "fresh-second");
    let before = current.snapshot().unwrap();
    let (restored, renames) = current.restore(saved).unwrap();
    assert_eq!(restored.snapshot().unwrap(), original);
    assert_eq!(
        renames
            .iter()
            .map(|r| (r.page_index, r.from.as_str(), r.to.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (0, "fresh-first", "saved-first"),
            (0, "fresh-second", "saved-second")
        ]
    );
    assert_eq!(
        current.snapshot().unwrap(),
        before,
        "validation must not mutate the current capture"
    );
    let empty = Index::capturing(identity(source()));
    assert!(empty.snapshot().unwrap().bindings.is_empty());
    assert!(empty
        .restore(empty.snapshot().unwrap())
        .unwrap()
        .1
        .is_empty());
}

#[test]
fn imported_overlay_sources_stale_forged_and_ambiguous_snapshots_are_refused() {
    let current = captured("fresh-first", "fresh-second");
    let valid = captured("saved-first", "saved-second").snapshot().unwrap();
    assert_eq!(valid.bindings.len(), 2);
    let mut cases = vec![];
    let mut v = valid.clone();
    v.source_sha256 = "0".repeat(64);
    cases.push(v);
    let mut v = valid.clone();
    v.version = 2;
    cases.push(v);
    let mut v = valid.clone();
    v.bindings[0].page_index = 1;
    cases.push(v);
    let mut v = valid.clone();
    v.bindings[0].annotation_index = 9;
    cases.push(v);
    let mut v = valid.clone();
    v.bindings[0].annotation_id = Some((8, 0));
    cases.push(v);
    let mut v = valid.clone();
    v.bindings[0].popup_indices.clear();
    cases.push(v);
    let mut v = valid.clone();
    v.bindings[0].initial_semantics_sha256 = "f".repeat(64);
    cases.push(v);
    let mut v = valid.clone();
    v.bindings[1].overlay_id = v.bindings[0].overlay_id.clone();
    cases.push(v);
    let mut v = valid.clone();
    v.bindings[0].overlay_id = " ".into();
    cases.push(v);
    let mut v = valid.clone();
    v.bindings.pop();
    cases.push(v);
    let mut v = valid.clone();
    v.bindings.push(v.bindings[0].clone());
    cases.push(v);
    for (i, snapshot) in cases.into_iter().enumerate() {
        assert!(current.restore(snapshot).is_err(), "case {i}");
    }
    let mut other = Index::capturing(identity(Arc::from(b"different source bytes".as_slice())));
    assert!(other.capture(0, 1, Some((7, 0)), &[2], &highlight("fresh-first", 10.)));
    assert!(other.capture(0, 3, None, &[], &highlight("fresh-second", 80.)));
    assert!(other.restore(valid).is_err());
}

#[test]
fn imported_overlay_sources_capacity_keeps_the_uncaptured_annotation_native() {
    let mut index = Index::capturing(identity(source()));
    index.limit = 1024;
    assert!(index.capture(0, 0, None, &[], &highlight("one", 10.)));
    assert!(!index.capture(0, 1, None, &[], &highlight("two", 80.)));
    assert_eq!(index.snapshot().unwrap().bindings.len(), 1);
    assert!(!index.capture(0, 0, None, &[], &highlight("duplicate-slot", 10.)));
    let mut unbound = Index::unbound(identity(source()));
    assert!(unbound.capture(0, 0, None, &[], &highlight("fresh-but-unpublished", 10.)));
    assert!(
        unbound.snapshot().is_none(),
        "generated-source IDs require explicit transfer proof"
    );
}

#[test]
fn imported_overlay_sources_runtime_capture_and_unreadable_alias_preservation() {
    runtime_fixture(false);
}
#[test]
fn imported_overlay_sources_shared_popup_keeps_both_parents_native() {
    runtime_fixture(true);
}
fn runtime_fixture(shared_popup: bool) {
    let mut pdf = Document::with_version("1.7");
    let tree = pdf.new_object_id();
    let page = pdf.new_object_id();
    let first=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Highlight","Rect"=>vec![20.into(),280.into(),100.into(),300.into()],
        "QuadPoints"=>vec![20.into(),300.into(),100.into(),300.into(),20.into(),280.into(),100.into(),280.into()],
        "C"=>vec![1.into(),1.into(),0.into()],"F"=>4,"NM"=>Object::string_literal("same"),"Contents"=>Object::string_literal("first"),"P"=>page});
    let alias = pdf.add_object(Object::Reference(first));
    let mut second = pdf.get_dictionary(first).unwrap().clone();
    second.set("Rect", vec![120.into(), 280.into(), 200.into(), 300.into()]);
    second.set(
        "QuadPoints",
        vec![
            120.into(),
            300.into(),
            200.into(),
            300.into(),
            120.into(),
            280.into(),
            200.into(),
            280.into(),
        ],
    );
    second.set("Contents", Object::string_literal("second"));
    let popup=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Popup","Parent"=>first,"P"=>page,"Rect"=>vec![0.into(),0.into(),10.into(),10.into()]});
    pdf.get_dictionary_mut(first).unwrap().set("Popup", popup);
    let contents = pdf.add_object(Stream::new(dictionary! {}, vec![]));
    pdf.objects.insert(page,dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary!{},"Contents"=>contents,
        "Annots"=>vec![Object::Reference(alias),Object::Dictionary(second),Object::Reference(popup)]}.into());
    pdf.objects.insert(
        tree,
        dictionary! {"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into(),
    );
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree});
    pdf.trailer.set("Root", root);
    if shared_popup {
        pdf.get_dictionary_mut(page)
            .unwrap()
            .get_mut(b"Annots")
            .unwrap()
            .as_array_mut()
            .unwrap()[1]
            .as_dict_mut()
            .unwrap()
            .set("Popup", popup);
    }
    for aliased in [true, false] {
        pdf.get_dictionary_mut(page)
            .unwrap()
            .get_mut(b"Annots")
            .unwrap()
            .as_array_mut()
            .unwrap()[0] = Object::Reference(if aliased { alias } else { first });
        let bytes = crate::pdf_output::save(&mut pdf).unwrap();
        let original = bytes.clone();
        let metadata = parse_annotation_metadata(&bytes).unwrap();
        assert_eq!(metadata.pages[&0][0].id, Some(first));
        let mut rt = WorkerRuntime::new(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
            Arc::new(FontRegistry::new()),
        )
        .unwrap();
        let path = std::env::temp_dir().join("folio-lineage.pdf");
        let info = rt
            .open_bytes(
                bytes,
                path.clone(),
                path,
                true,
                true,
                false,
                OpenSecurity::Restored(None),
            )
            .unwrap();
        let source = rt.document(&info.id).unwrap();
        if aliased || shared_popup {
            // Shared popups must not be removed on behalf of either parent.
            // PDFium returns no handle for an alias-to-annotation reference. The
            // existing importer leaves the entire page native rather than guessing.
            assert!(info.pages[0].overlays.is_empty());
            assert!(source
                .imported_overlay_sources
                .snapshot()
                .unwrap()
                .bindings
                .is_empty());
            assert_eq!(
                source.pdf().unwrap().pages().get(0).unwrap().annotations().len(),
                3
            );
            assert_eq!(source.bytes().unwrap().as_ref(), original.as_slice());
            continue;
        }
        assert_eq!(info.pages[0].overlays.len(), 2);
        let snapshot = source.imported_overlay_sources.snapshot().unwrap();
        assert_eq!(snapshot.bindings.len(), 2);
        assert_eq!(snapshot.bindings[0].annotation_id, Some(first));
        assert_eq!(snapshot.bindings[1].annotation_id, None);
        assert_eq!(snapshot.bindings[0].popup_indices, vec![2]);
        assert!(snapshot.bindings[1].popup_indices.is_empty());
        for (binding, overlay) in snapshot.bindings.iter().zip(&info.pages[0].overlays) {
            let Overlay::Highlight(h) = overlay else {
                panic!("highlight")
            };
            assert_eq!(binding.overlay_id, h.id);
        }
        assert_ne!(
            snapshot.bindings[0].overlay_id,
            snapshot.bindings[1].overlay_id
        );
        assert_eq!(source.imported_annotations[&0], HashSet::from([0, 1, 2]));
        assert_eq!(
            source.pdf().unwrap().pages().get(0).unwrap().annotations().len(),
            0
        );
        assert_eq!(source.bytes().unwrap().as_ref(), original.as_slice());
    }
}

#[test]
fn imported_overlay_sources_overlap_refusal_and_page_local_ids() {
    let mut index = captured("first", "second");
    let before = index.snapshot().unwrap();
    assert!(!index.capture(0, 2, None, &[], &highlight("popup-as-parent", 20.)));
    assert!(!index.capture(0, 4, None, &[2], &highlight("shared-popup", 20.)));
    assert!(!index.capture(0, 4, None, &[], &highlight("first", 20.)));
    assert!(!index.capture(0, 4, None, &[6, 5], &highlight("unordered", 20.)));
    assert_eq!(index.snapshot().unwrap(), before);
    assert!(index.capture(1, 1, Some((7, 0)), &[2], &highlight("first", 10.)));
    let saved = index.snapshot().unwrap();
    assert!(index.restore(saved).is_ok());
}

#[test]
fn imported_overlay_sources_semantic_work_is_bounded_before_retention() {
    let mut index = Index::capturing(identity(source()));
    // Many empty paths are small in bytes but expensive in traversal count.
    let overlay = Overlay::Ink(InkOverlay {
        id: "many-paths".into(),
        paths: vec![vec![]; 1_000_001],
        color: "#000000".into(),
        stroke_width: 1.,
    });
    assert!(!index.capture(0, 0, None, &[], &overlay));
    assert!(index.snapshot().unwrap().bindings.is_empty());
    assert!(index.capture(0, 0, None, &[], &highlight("still-available", 10.)));
}

#[test]
fn imported_overlay_sources_semantics_cover_all_overlay_variants_without_ids() {
    use serde_json::json;
    let variants = [
        json!({"type":"text","id":"a","x":10,"y":20,"text":"Original","fontSize":12,"color":"#123456"}),
        json!({"type":"image","id":"a","assetId":"a".repeat(64),"x":10,"y":20,"width":30,"height":40}),
        json!({"type":"ink","id":"a","paths":[[{"x":10,"y":20},{"x":30,"y":40}]],"color":"#123456","strokeWidth":2}),
        json!({"type":"comment","id":"a","x":10,"y":20,"text":"Original","color":"#123456"}),
    ];
    for mut value in variants {
        let original: Overlay = serde_json::from_value(value.clone()).unwrap();
        let first = semantics(&original, MAX_METADATA).unwrap().0;
        value["id"] = json!("another workspace label");
        let renamed: Overlay = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(first, semantics(&renamed, MAX_METADATA).unwrap().0);
        match value["type"].as_str().unwrap() {
            "text" | "comment" => value["text"] = json!("Edited"),
            "image" => value["assetId"] = json!("b".repeat(64)),
            "ink" => value["paths"][0][0]["x"] = json!(11),
            _ => unreachable!(),
        }
        let changed: Overlay = serde_json::from_value(value).unwrap();
        assert_ne!(first, semantics(&changed, MAX_METADATA).unwrap().0);
    }
}

#[test]
fn imported_overlay_sources_cumulative_work_and_escaped_metadata_bounds() {
    let mut index = Index::capturing(identity(source()));
    let overlay = Overlay::Ink(InkOverlay {
        id: "paths".into(),
        paths: vec![vec![]; 150_000],
        color: "#000000".into(),
        stroke_width: 1.,
    });
    assert!(index.capture(0, 0, None, &[], &overlay));
    let before = index.snapshot().unwrap();
    assert!(!index.capture(1, 0, None, &[], &overlay));
    assert_eq!(index.snapshot().unwrap(), before);
    let mut escaped = Index::capturing(identity(source()));
    assert!(escaped.capture(
        0,
        0,
        None,
        &[],
        &highlight(&format!("id{}", "\u{0001}".repeat(1000)), 10.)
    ));
    assert!(
        serde_json::to_vec(&escaped.snapshot().unwrap())
            .unwrap()
            .len()
            <= escaped.used
    );
    let unbound = Index::unbound(identity(source()));
    assert!(unbound
        .restore(Index::capturing(identity(source())).snapshot().unwrap())
        .is_err());
}

#[test]
fn imported_overlay_sources_candidate_scans_share_the_work_budget() {
    let mut index = Index::capturing(identity(source()));
    assert!(index.reserve_scan(MAX_WORK - 100));
    assert!(!index.reserve_scan(101));
    assert!(index.capture(0, 0, None, &[], &highlight("first", 10.)));
    assert!(!index.capture(1, 0, None, &[], &highlight("second", 10.)));
    assert_eq!(index.snapshot().unwrap().bindings.len(), 1);
    assert!(!index.reserve_scan(usize::MAX));
}

fn physical_echo(overlay: &Overlay, unit: f32) -> Overlay {
    use super::super::physical_units::Scale;
    let mut page = PageInfo { width: 200., height: 300., overlays: vec![overlay.clone()] };
    page.scale(unit).unwrap();
    page.overlays.scale(unit.recip()).unwrap();
    page.overlays.pop().unwrap()
}

#[test]
fn imported_overlay_sources_physical_echo_matches_actual_public_conversion() {
    use serde_json::json;
    for unit in [1_f32, 1.7, 0.3, 2., 75_000.] {
        let variants = [
            json!({"type":"text","id":"a","x":29.411764,"y":37.64706,"text":"Original","fontSize":1.*unit.recip(),"color":"#123456"}),
            json!({"type":"text","id":"a","x":29.411764,"y":37.64706,"text":"Area","fontSize":512.*unit.recip(),"color":"#123456","area":{"version":1,"width":14400.*unit.recip(),"height":14400.*unit.recip(),"inset":1.1764705,"lineSpacing":1.2,"alignment":"center"}}),
            json!({"type":"image","id":"a","assetId":"asset","x":29.411764,"y":37.64706,"width":41.17647,"height":26.470589}),
            json!({"type":"ink","id":"a","paths":[[{"x":29.411764,"y":37.64706},{"x":41.17647,"y":26.470589}]],"color":"#123456","strokeWidth":100.*unit.recip()}),
            json!({"type":"highlight","id":"a","rects":[{"x":29.411764,"y":37.64706,"width":41.17647,"height":26.470589}],"text":"Original","color":"#123456","opacity":0.35}),
            json!({"type":"comment","id":"a","x":29.411764,"y":37.64706,"text":"Original","color":"#123456"}),
        ];
        for value in variants {
            let overlay: Overlay = serde_json::from_value(value).unwrap();
            let expected = semantics(&physical_echo(&overlay,unit),MAX_METADATA).unwrap();
            assert_eq!(unit_semantics::semantics(&overlay,unit,MAX_METADATA),Some(expected.clone()),"{unit}: {overlay:?}");
            assert!(unit_semantics::semantics(&overlay,unit,expected.1-1).is_none());
            assert_eq!(unit_semantics::semantics(&overlay,unit,expected.1),Some(expected));
        }
    }
}

#[test]
fn imported_overlay_sources_physical_echo_recovery_is_source_derived_and_exact() {
    use publication::OwnedChange;
    let original=highlight("saved",29.411764);
    let mut current=Index::capturing(identity(source()));
    assert!(current.capture_in_units(0,0,None,&[],&original,1.7));
    let echo=physical_echo(&original,1.7);
    let mut changed=echo.clone();
    let Overlay::Highlight(v)=&mut changed else {unreachable!()};
    v.rects[0].x=f32::from_bits(v.rects[0].x.to_bits()+1);
    let mut saved=current.snapshot().unwrap();
    let wire=serde_json::to_value(&saved).unwrap();
    assert!(wire["bindings"][0].get("publicEchoSha256").is_none());
    let decoded:Snapshot=serde_json::from_value(wire.clone()).unwrap();
    assert!(decoded.bindings[0].public_echo_sha256.is_none());
    let mut forged_wire=wire;
    forged_wire["bindings"][0]["publicEchoSha256"]=serde_json::json!(semantics(&changed,MAX_METADATA).unwrap().0);
    assert!(serde_json::from_value::<Snapshot>(forged_wire).is_err());
    saved.bindings[0].public_echo_sha256=Some(semantics(&changed,MAX_METADATA).unwrap().0);
    for snapshot in [decoded,saved] {
        let (restored,_)=current.restore(snapshot).unwrap();
        assert_eq!(restored.snapshot(),current.snapshot());
        for (draft,keep) in [(echo.clone(),true),(changed.clone(),false)] {
            let draft=[draft];let mut p=restored.publication(&restored.source,1,&mut ||Ok(())).unwrap();
            assert_eq!(matches!(p.page(0,&draft,&mut ||Ok(())).unwrap().owned[0],OwnedChange::Keep(_,_)),keep);
        }
    }
}

#[test]
fn imported_overlay_sources_physical_echo_refusal_does_not_consume_capacity() {
    let overlay=highlight("a",29.411764);
    let raw_bytes=semantics(&overlay,MAX_METADATA).unwrap().1;
    let echo_bytes=unit_semantics::semantics(&overlay,1.7,MAX_METADATA).unwrap().1;
    let work=semantic_work(&overlay,0,MAX_WORK).unwrap();
    for mode in 0..3 {
        let mut index=Index::capturing(identity(source()));
        match mode {
            0=>index.limit=reservation("a",0).unwrap()+127,
            1=>index.semantic_bytes=MAX_METADATA-raw_bytes-echo_bytes+1,
            _=>index.work=MAX_WORK-2*work+1,
        }
        let before=(index.snapshot(),index.used,index.semantic_bytes,index.work);
        assert!(!index.capture_in_units(0,0,None,&[],&overlay,1.7));
        assert_eq!((index.snapshot(),index.used,index.semantic_bytes,index.work),before);
        assert!(index.slots.is_empty() && index.labels.is_empty());
        match mode {0=>index.limit+=1,1=>index.semantic_bytes-=1,_=>index.work-=1}
        assert!(index.capture_in_units(0,0,None,&[],&overlay,1.7),"exact endpoint {mode}");
    }
    for unit in [0.,-1.,f32::NAN,f32::INFINITY,75_001.,f32::from_bits(1)] {
        assert!(unit_semantics::semantics(&overlay,unit,MAX_METADATA).is_none());
    }
}

fn identity(bytes: Arc<[u8]>) -> crate::source_storage::SourceIdentity { crate::source_storage::SourceStore::new().capture(bytes).unwrap().identity().clone() }
