use super::*;
use lopdf::{dictionary, Document, Object};
fn runtime() -> WorkerRuntime {
    WorkerRuntime::new(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
        Arc::new(FontRegistry::new()),
    )
    .unwrap()
}
fn bytes(title: Option<&str>, count: usize) -> Vec<u8> {
    let mut doc = Document::with_version("1.7");
    let tree = doc.new_object_id();
    let mut pages = vec![];
    for _ in 0..count {
        pages.push(doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),200.into(),300.into()],"Resources"=>dictionary!{}}));
    }
    doc.objects.insert(tree,dictionary!{"Type"=>"Pages","Kids"=>pages.iter().map(|p|Object::Reference(*p)).collect::<Vec<_>>(),"Count"=>count as i64}.into());
    let mut catalog = dictionary! {"Type"=>"Catalog","Pages"=>tree};
    if let Some(title) = title {
        let root = doc.new_object_id();
        let item=doc.add_object(dictionary!{"Title"=>Object::string_literal(title),"Parent"=>root,"Dest"=>vec![Object::Reference(pages[count-1]),"Fit".into()],"Private"=>Object::string_literal("preserve raw outline")});
        doc.objects.insert(
            root,
            dictionary! {"Type"=>"Outlines","First"=>item,"Last"=>item,"Count"=>1}.into(),
        );
        catalog.set("Outlines", root);
    }
    let root = doc.add_object(catalog);
    doc.trailer.set("Root", root);
    let mut output = vec![];
    doc.save_to(&mut output).unwrap();
    output
}
fn policy(password: &str) -> SourcePolicy {
    let encrypted = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/protection/edit_only_aes_256.pdf"),
    )
    .unwrap();
    let protection = crate::protection::unlock_pdf(&encrypted, Some(password))
        .unwrap()
        .protection
        .unwrap();
    let original = crate::bookmarks::read(&bytes(Some("Historical"), 2)).unwrap();
    SourcePolicy::new(
        protection,
        &vec![
            PageInfo {
                width: 200.,
                height: 300.,
                overlays: vec![]
            };
            2
        ],
        &Ok(original),
    )
}
fn request(info: &DocumentInfo, prefix: &str) -> ExportRequest {
    ExportRequest {
        flatten: false,
        pages: info
            .pages
            .iter()
            .enumerate()
            .map(|(index, page)| PagePlan {
                id: format!("{prefix}-{index}"),
                source_id: info.id.clone(),
                page_index: index,
                width: page.width,
                height: page.height,
                rotation: 0,
                crop: None,
                overlays: vec![],
            })
            .collect(),
    }
}
#[test]
fn bookmark_complete_inheritance_uses_current_protected_source_and_preserves_raw_graph() {
    let mut rt = runtime();
    for title in [Some("Current"), None] {
        let original = bytes(title, 2);
        let current = rt
            .register_generated_document(original.clone(), "Current.pdf")
            .unwrap();
        rt.documents.get_mut(&current.id).unwrap().policy = Some(policy("owner"));
        let plan = request(&current, "current");
        let inherited = rt.inherited_bookmarks(&plan).unwrap();
        assert_eq!(inherited.first().map(|b| b.title.as_str()), title);
        let (saved, retained) = rt
            .compose_pdf_with_route(plan, None, CompositionIntent::Save, None, None)
            .unwrap();
        assert!(retained);
        let unlocked = crate::protection::unlock_pdf(&saved, Some("owner")).unwrap();
        let current_pdf = Document::load_mem(&original).unwrap();
        let output = Document::load_mem(&unlocked.bytes).unwrap();
        assert_eq!(
            crate::bookmarks::read(&unlocked.bytes)
                .unwrap()
                .first()
                .map(|b| b.title.clone()),
            title.map(str::to_owned)
        );
        if title.is_some() {
            let root = current_pdf
                .catalog()
                .unwrap()
                .get(b"Outlines")
                .unwrap()
                .as_reference()
                .unwrap();
            let item = current_pdf
                .get_dictionary(root)
                .unwrap()
                .get(b"First")
                .unwrap()
                .as_reference()
                .unwrap();
            assert_eq!(
                output
                    .get_dictionary(item)
                    .unwrap()
                    .get(b"Private")
                    .unwrap(),
                current_pdf
                    .get_dictionary(item)
                    .unwrap()
                    .get(b"Private")
                    .unwrap()
            );
        }
        assert_eq!(
            &*rt.document(&current.id).unwrap().bytes().unwrap(),
            original.as_slice()
        );
    }
}
#[test]
fn bookmark_complete_inheritance_does_not_grant_reader_bookmark_rights() {
    let mut rt = runtime();
    let current = rt
        .register_generated_document(bytes(Some("Current"), 2), "Current.pdf")
        .unwrap();
    rt.documents.get_mut(&current.id).unwrap().policy = Some(policy("reader"));
    assert!(matches!(
        rt.compose_pdf_with_route(
            request(&current, "current"),
            None,
            CompositionIntent::Save,
            None,
            None
        ),
        Err(EngineError::PermissionDenied(_))
    ));
}
#[test]
fn bookmark_complete_inheritance_combines_current_blocks_but_keeps_partial_origin_behavior() {
    let mut rt = runtime();
    let a = rt
        .register_generated_document(bytes(Some("Current A"), 2), "A.pdf")
        .unwrap();
    let b = rt
        .register_generated_document(bytes(Some("Current B"), 2), "B.pdf")
        .unwrap();
    rt.documents.get_mut(&a.id).unwrap().policy = Some(policy("owner"));
    let mut plan = request(&a, "a");
    plan.pages.extend(request(&b, "b").pages);
    let items = rt.inherited_bookmarks(&plan).unwrap();
    assert_eq!(
        items.iter().map(|b| b.title.as_str()).collect::<Vec<_>>(),
        vec!["Current A", "Current B"]
    );
    assert_eq!(
        items
            .iter()
            .map(crate::bookmarks::primary_page)
            .collect::<Vec<_>>(),
        vec![Some("a-1"), Some("b-1")]
    );
    let partial = rt
        .register_generated_document(bytes(Some("Page projection"), 1), "Page.pdf")
        .unwrap();
    rt.documents.get_mut(&partial.id).unwrap().policy = Some(policy("owner").page(1).unwrap());
    let items = rt.inherited_bookmarks(&request(&partial, "part")).unwrap();
    assert_eq!(items[0].title, "Historical");
    assert_eq!(crate::bookmarks::primary_page(&items[0]), Some("part-0"));
}
#[test]
fn bookmark_selected_inheritance_uses_current_full_source_after_reorder_and_selection() {
    let mut rt=runtime();
    let current=rt.register_generated_document(bytes(Some("Current selected"),2),"Current.pdf").unwrap();
    rt.documents.get_mut(&current.id).unwrap().policy=Some(policy("owner"));
    let mut plan=request(&current,"selected");plan.pages.reverse();
    for _ in 0..2 {
        let items=rt.inherited_bookmarks(&plan).unwrap();
        assert_eq!(items[0].title,"Current selected");
        assert_eq!(crate::bookmarks::primary_page(&items[0]),Some("selected-1"));
        plan.pages.pop();
    }
}
#[test]
fn bookmark_complete_inheritance_never_hides_unreadable_current_outlines() {
    let mut rt = runtime();
    let mut doc = Document::load_mem(&bytes(Some("Current"), 2)).unwrap();
    doc.catalog_mut().unwrap().set("Outlines", 42);
    let mut raw = vec![];
    doc.save_to(&mut raw).unwrap();
    let current = rt
        .register_generated_document(raw, "Unreadable.pdf")
        .unwrap();
    rt.documents.get_mut(&current.id).unwrap().policy = Some(policy("owner"));
    assert!(rt
        .inherited_bookmarks(&request(&current, "current"))
        .is_err());
}

#[test]
fn bookmark_complete_inheritance_recovery_distinguishes_single_page_projection_from_cleared_document(
) {
    let mut rt = runtime();
    let raw = bytes(None, 1);
    let original = crate::bookmarks::read(&bytes(Some("Historical"), 1)).unwrap();
    let full = SourcePolicy::new(
        (*policy("owner").protection).clone(),
        &[PageInfo {
            width: 200.,
            height: 300.,
            overlays: vec![],
        }],
        &Ok(original),
    );
    for projection in [false, true] {
        let source = if projection {
            full.page(0).unwrap()
        } else {
            full.clone()
        };
        let snapshot = source.snapshot(&raw).unwrap();
        let encoded = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(encoded["current_bookmarks"], !projection);
        let decoded: RecoveryPolicy = serde_json::from_value(encoded.clone()).unwrap();
        let restored = decoded.restore(&raw).unwrap().unwrap();
        let info = rt
            .register_generated_document(raw.clone(), "Recovered.pdf")
            .unwrap();
        rt.documents.get_mut(&info.id).unwrap().policy = Some(restored);
        let inherited = rt
            .inherited_bookmarks(&request(&info, "recovered"))
            .unwrap();
        assert_eq!(
            inherited.first().map(|b| b.title.as_str()),
            if projection { Some("Historical") } else { None }
        );
        // Older recovery records have no evidence that a source retained its
        // whole outline graph. Preserve their legacy origin-based behavior.
        let mut legacy = encoded;
        legacy.as_object_mut().unwrap().remove("current_bookmarks");
        let legacy: RecoveryPolicy = serde_json::from_value(legacy).unwrap();
        rt.documents.get_mut(&info.id).unwrap().policy = legacy.restore(&raw).unwrap();
        assert_eq!(
            rt.inherited_bookmarks(&request(&info, "legacy")).unwrap()[0].title,
            "Historical"
        );
    }
}

#[test]
fn bookmark_complete_inheritance_current_block_cancellation_preserves_source() {
    let mut rt = runtime();
    let raw = bytes(Some("Current"), 2);
    let current = rt
        .register_generated_document(raw.clone(), "Current.pdf")
        .unwrap();
    rt.documents.get_mut(&current.id).unwrap().policy = Some(policy("owner"));
    let request = request(&current, "current");
    let mut calls = 0;
    rt.inherited_bookmarks_controlled(&request, &mut || {
        calls += 1;
        Ok(())
    })
    .unwrap();
    for stop in [1, calls / 2, calls] {
        let mut n = 0;
        assert!(rt
            .inherited_bookmarks_controlled(&request, &mut || {
                n += 1;
                if n == stop {
                    Err(EngineError::InvalidRequest(
                        "cancel inherited bookmarks".into(),
                    ))
                } else {
                    Ok(())
                }
            })
            .is_err());
    }
    assert_eq!(
        &*rt.document(&current.id).unwrap().bytes().unwrap(),
        raw.as_slice()
    );
    assert_eq!(
        rt.inherited_bookmarks(&request).unwrap()[0].title,
        "Current"
    );
}
