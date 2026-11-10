use folio_engine::{bookmarks, DocumentBookmark, ExportRequest, PagePlan, PdfEngine};
use lopdf::{dictionary, Document, Object, StringFormat};
use std::{fs, path::PathBuf};

fn fixture() -> Vec<u8> {
    let mut doc = Document::with_version("1.7");
    let root = doc.new_object_id();
    let pages_id = doc.new_object_id();
    let mut pages = vec![];
    for _ in 0..3 {
        pages.push(doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages_id,"MediaBox"=>vec![0.into(),0.into(),200.into(),300.into()],"Resources"=>dictionary!{}}));
    }
    doc.objects.insert(pages_id,Object::Dictionary(dictionary!{"Type"=>"Pages","Count"=>3,"Kids"=>pages.iter().map(|p|Object::Reference(*p)).collect::<Vec<_>>()}));
    let outlines = doc.new_object_id();
    let first = doc.new_object_id();
    let child = doc.new_object_id();
    let last = doc.new_object_id();
    doc.objects.insert(
        outlines,
        Object::Dictionary(dictionary! {"Type"=>"Outlines","First"=>first,"Last"=>last,"Count"=>2}),
    );
    doc.objects.insert(first,Object::Dictionary(dictionary!{"Title"=>lopdf::text_string("Chapitre Ω"),"Parent"=>outlines,"First"=>child,"Last"=>child,"Next"=>last,"Count"=>-1,"F"=>3,"C"=>vec![1.0.into(),0.0.into(),0.0.into()],"Dest"=>vec![Object::Reference(pages[1]),Object::Name(b"XYZ".to_vec()),Object::Null,50.into(),Object::Real(1.25)]}));
    doc.objects.insert(child,Object::Dictionary(dictionary!{"Title"=>lopdf::text_string("Named child"),"Parent"=>first,"A"=>dictionary!{"S"=>"GoTo","D"=>Object::String(b"last-page".to_vec(),StringFormat::Literal)}}));
    doc.objects.insert(last,Object::Dictionary(dictionary!{"Title"=>lopdf::text_string("Website"),"Parent"=>outlines,"Prev"=>first,"A"=>dictionary!{"S"=>"URI","URI"=>Object::String(b"https://example.com/".to_vec(),StringFormat::Literal)}}));
    doc.objects.insert(root,Object::Dictionary(dictionary!{"Type"=>"Catalog","Pages"=>pages_id,"Outlines"=>outlines,"Names"=>dictionary!{"Dests"=>dictionary!{"Names"=>vec![Object::String(b"last-page".to_vec(),StringFormat::Literal),Object::Array(vec![Object::Reference(pages[2]),Object::Name(b"Fit".to_vec())])]}}}));
    doc.trailer.set("Root", root);
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    bytes
}
fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}

#[test]
fn malformed_outline_hidden_by_pdfium_requires_explicit_discard() {
    let mut doc = Document::load_mem(&fixture()).unwrap();
    let root = doc
        .catalog()
        .unwrap()
        .get(b"Outlines")
        .unwrap()
        .as_reference()
        .unwrap();
    let first = doc
        .get_dictionary(root)
        .unwrap()
        .get(b"First")
        .unwrap()
        .as_reference()
        .unwrap();
    doc.objects.insert(first, Object::Integer(42));
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("malformed.pdf");
    fs::write(&source, bytes).unwrap();
    let engine = engine();
    let info = engine.open_document(&source).unwrap();
    assert_eq!(info.pages.len(), 3);
    assert!(
        info.bookmark_error.is_some(),
        "malformed outlines must not appear absent"
    );
    let output = temp.path().join("out.pdf");
    assert!(engine
        .export_pdf(
            ExportRequest {
                pages: plans(&info.id),
                flatten: false
            },
            &output
        )
        .is_err());
    assert!(!output.exists());
    engine
        .export_pdf_with_bookmarks(
            ExportRequest {
                pages: plans(&info.id),
                flatten: false,
            },
            Some(vec![]),
            &output,
        )
        .unwrap();
    assert!(bookmarks::read(&fs::read(output).unwrap())
        .unwrap()
        .is_empty());
}

#[test]
fn reference_and_container_depths_share_the_32_level_limit() {
    for field in ["Title", "A", "Dest"] {
        for depth in [32, 33] {
            let mut doc = Document::load_mem(&fixture()).unwrap();
            let root = doc
                .catalog()
                .unwrap()
                .get(b"Outlines")
                .unwrap()
                .as_reference()
                .unwrap();
            let first = doc
                .get_dictionary(root)
                .unwrap()
                .get(b"First")
                .unwrap()
                .as_reference()
                .unwrap();
            let mut value = match field {
                "Title" => Object::string_literal("Deep title"),
                "A" => Object::Dictionary(dictionary! {"S"=>"Named", "N"=>"NextPage"}),
                _ => doc
                    .get_dictionary(first)
                    .unwrap()
                    .get(b"Dest")
                    .unwrap()
                    .clone(),
            };
            // A and Dest have a container edge in addition to references.
            let refs = depth - usize::from(field != "Title");
            for _ in 0..refs {
                value = Object::Reference(doc.add_object(value));
            }
            doc.get_dictionary_mut(first).unwrap().set(field, value);
            let mut bytes = vec![];
            doc.save_to(&mut bytes).unwrap();
            let result = bookmarks::read(&bytes);
            assert_eq!(
                result.is_ok(),
                depth == 32,
                "field {field}, depth {depth}: {result:?}"
            );
        }
    }
}
fn plans(id: &str) -> Vec<PagePlan> {
    (0..3)
        .map(|i| PagePlan {
            crop: None,
            id: format!("p{i}"),
            source_id: id.into(),
            page_index: i,
            width: 200.,
            height: 300.,
            rotation: 0,
            overlays: vec![],
        })
        .collect()
}

#[test]
fn imports_nested_styles_unicode_and_named_local_destinations() {
    let items = bookmarks::read(&fixture()).unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!((items[0].title.as_str(), items[0].level), ("Chapitre Ω", 0));
    assert!(items[0].bold && items[0].italic && items[0].closed);
    assert_eq!(items[0].color, Some([1., 0., 0.]));
    assert_eq!(bookmarks::primary_page(&items[0]), Some("1"));
    assert_eq!(bookmarks::primary_page(&items[1]), Some("2"));
    assert_eq!(items[1].level, 1);
    assert!(items[2].action.is_some());
    assert_eq!(bookmarks::primary_page(&items[2]), None);
}

#[test]
fn destination_names_keep_legacy_name_and_modern_string_namespaces_separate() {
    let mut doc = Document::load_mem(&fixture()).unwrap();
    let pages = doc.get_pages().values().copied().collect::<Vec<_>>();
    let outline = doc.catalog().unwrap().get(b"Outlines").unwrap().as_reference().unwrap();
    let first = doc.get_dictionary(outline).unwrap().get(b"First").unwrap().as_reference().unwrap();
    let child = doc.get_dictionary(first).unwrap().get(b"First").unwrap().as_reference().unwrap();
    let key = b"last-page".to_vec();
    let legacy = doc.add_object(dictionary!{"D"=>vec![Object::Reference(pages[0]),Object::Name(b"XYZ".to_vec()),Object::Null,50.into(),Object::Real(1.25)],"Private"=>"retained"});
    let catalog = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
    doc.get_dictionary_mut(catalog).unwrap().set("Dests", dictionary!{key.clone()=>legacy});
    let alias = doc.add_object(Object::Name(key.clone()));
    doc.get_dictionary_mut(first).unwrap().set("Dest", alias);
    let before = doc.objects.clone();
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    let items = bookmarks::read(&bytes).unwrap();
    assert_eq!(bookmarks::primary_page(&items[0]), Some("0"));
    assert_eq!(bookmarks::primary_page(&items[1]), Some("2"));
    assert_eq!(items[0].destination, Some(bookmarks::PdfValue::Array(vec![
        bookmarks::PdfValue::Page("0".into()), bookmarks::PdfValue::Name(b"XYZ".to_vec()),
        bookmarks::PdfValue::Null, bookmarks::PdfValue::Integer(50), bookmarks::PdfValue::Real(1.25),
    ])));
    assert_eq!(doc.objects, before);

    // With only one namespace present, the other operand type must not fall back.
    for missing in ["Dests", "Names"] {
        let mut invalid = doc.clone();
        invalid.get_dictionary_mut(catalog).unwrap().remove(missing.as_bytes());
        let mut encoded = vec![];
        invalid.save_to(&mut encoded).unwrap();
        assert!(bookmarks::read(&encoded).is_err(), "missing {missing}");
    }

    // Direct Dest and GoTo/D both dispatch by the resolved operand type.
    doc.get_dictionary_mut(first).unwrap().set("Dest", Object::String(key.clone(), StringFormat::Hexadecimal));
    doc.get_dictionary_mut(child).unwrap().set("A", dictionary!{"S"=>"GoTo","D"=>Object::Name(key)});
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    let items = bookmarks::read(&bytes).unwrap();
    assert_eq!(bookmarks::primary_page(&items[0]), Some("2"));
    assert_eq!(bookmarks::primary_page(&items[1]), Some("0"));
}

#[test]
fn destination_name_tree_duplicates_remain_invalid_with_legacy_namespace() {
    let mut doc = Document::load_mem(&fixture()).unwrap();
    let catalog = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let page = *doc.get_pages().values().next().unwrap();
    let destination = Object::Array(vec![Object::Reference(page),Object::Name(b"Fit".to_vec())]);
    doc.get_dictionary_mut(catalog).unwrap().set("Dests", dictionary!{"last-page"=>destination.clone()});
    doc.get_dictionary_mut(catalog).unwrap().set("Names", dictionary!{"Dests"=>dictionary!{"Names"=>vec![
        Object::string_literal("last-page"),destination.clone(),Object::string_literal("last-page"),destination,
    ]}});
    let mut bytes=vec![];
    doc.save_to(&mut bytes).unwrap();
    assert!(bookmarks::read(&bytes).unwrap_err().to_string().contains("duplicate named destination"));
}

#[test]
fn export_remaps_inherited_bookmarks_after_reorder_extract_and_explicit_clear() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.pdf");
    fs::write(&source, fixture()).unwrap();
    let engine = engine();
    let info = engine.open_document(&source).unwrap();
    assert_eq!(info.bookmarks.len(), 3);
    assert!(info.bookmark_error.is_none());
    let pages = plans(&info.id);
    let output = temp.path().join("reordered.pdf");
    engine
        .export_pdf(
            ExportRequest {
                pages: vec![pages[2].clone(), pages[0].clone(), pages[1].clone()],
                flatten: false,
            },
            &output,
        )
        .unwrap();
    let reread = bookmarks::read(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(bookmarks::primary_page(&reread[0]), Some("2"));
    assert_eq!(bookmarks::primary_page(&reread[1]), Some("0"));
    assert!(reread[2].action.is_some());
    let subset = temp.path().join("subset.pdf");
    engine
        .export_pdf(
            ExportRequest {
                pages: vec![pages[2].clone()],
                flatten: false,
            },
            &subset,
        )
        .unwrap();
    let items = bookmarks::read(&fs::read(&subset).unwrap()).unwrap();
    assert_eq!(items[0].title, "Chapitre Ω");
    assert_eq!(bookmarks::primary_page(&items[0]), None);
    assert_eq!(bookmarks::primary_page(&items[1]), Some("0"));
    let cleared = temp.path().join("cleared.pdf");
    engine
        .export_pdf_with_bookmarks(
            ExportRequest {
                pages,
                flatten: false,
            },
            Some(Vec::<DocumentBookmark>::new()),
            &cleared,
        )
        .unwrap();
    assert!(bookmarks::read(&fs::read(cleared).unwrap())
        .unwrap()
        .is_empty());
    assert_eq!(fs::read(source).unwrap(), fixture());
}

#[test]
fn cyclic_outline_is_reported_without_blocking_page_reading_or_silently_losing_it() {
    let mut doc = Document::load_mem(&fixture()).unwrap();
    let first = doc
        .get_dictionary(
            doc.catalog()
                .unwrap()
                .get(b"Outlines")
                .unwrap()
                .as_reference()
                .unwrap(),
        )
        .unwrap()
        .get(b"First")
        .unwrap()
        .as_reference()
        .unwrap();
    doc.get_dictionary_mut(first).unwrap().set("Next", first);
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    assert!(bookmarks::read(&bytes).is_err());
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("cycle.pdf");
    fs::write(&source, bytes).unwrap();
    let engine = engine();
    let info = engine.open_document(&source).unwrap();
    assert_eq!(info.pages.len(), 3);
    assert!(info.bookmark_error.is_some());
    let out = temp.path().join("out.pdf");
    assert!(engine
        .export_pdf(
            ExportRequest {
                pages: plans(&info.id),
                flatten: false
            },
            &out
        )
        .is_err());
    assert!(!out.exists());
}

#[test]
fn remapping_action_chains_removes_missing_page_actions_without_null_entries() {
    let item:DocumentBookmark=serde_json::from_value(serde_json::json!({"id":"a","title":"External","level":0,"action":{"kind":"dictionary","value":[[[83],{"kind":"name","value":[85,82,73]}],[[78,101,120,116],{"kind":"array","value":[{"kind":"dictionary","value":[[[83],{"kind":"name","value":[71,111,84,111]}],[[68],{"kind":"array","value":[{"kind":"page","value":"gone"},{"kind":"name","value":[70,105,116]}]}]]},{"kind":"dictionary","value":[[[83],{"kind":"name","value":[78,97,109,101,100]}],[[78],{"kind":"name","value":[78,101,120,116,80,97,103,101]}]]}]}]]}})).unwrap();
    let mapped = bookmarks::remap(&[item], &std::collections::HashMap::new());
    let folio_engine::PdfValue::Dictionary(d) = mapped[0].action.as_ref().unwrap() else {
        panic!()
    };
    let folio_engine::PdfValue::Array(next) = &d[1].1 else {
        panic!()
    };
    assert_eq!(next.len(), 1);
    assert!(!matches!(next[0], folio_engine::PdfValue::Null));
}

#[test]
fn recovery_preserves_bookmarks_and_rejects_missing_page_references() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.pdf");
    fs::write(&source, fixture()).unwrap();
    let engine = engine();
    let info = engine.open_document(&source).unwrap();
    let items = bookmarks::remap(
        &info.bookmarks,
        &(0..3).map(|i| (i.to_string(), format!("p{i}"))).collect(),
    );
    let mut snapshot = serde_json::json!({"version":1,"activeId":"a","tabs":[{"id":"a","savedDigest":"","dirty":true,"zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"Book.pdf","pages":plans(&info.id),"selectedPageId":"p0","selectedOverlayId":null,"bookmarks":items}}]});
    let store = folio_engine::RecoveryStore::new(temp.path().join("recovery")).unwrap();
    store.save(&engine, snapshot.clone()).unwrap();
    snapshot["tabs"][0]["document"]["bookmarks"][0]["destination"]["value"][0]["value"] =
        serde_json::json!("missing");
    assert!(store.save(&engine, snapshot).is_err());
    fs::remove_file(source).unwrap();
    engine.close_document(&info.id).unwrap();
    let recovered = store.load(&engine).unwrap().unwrap();
    let restored: Vec<DocumentBookmark> =
        serde_json::from_value(recovered["tabs"][0]["document"]["bookmarks"].clone()).unwrap();
    assert_eq!(bookmarks::primary_page(&restored[0]), Some("p1"));
    let pages = serde_json::from_value(recovered["tabs"][0]["document"]["pages"].clone()).unwrap();
    let out = temp.path().join("recovered.pdf");
    engine
        .export_pdf_with_bookmarks(
            ExportRequest {
                pages,
                flatten: false,
            },
            Some(restored),
            &out,
        )
        .unwrap();
    assert_eq!(bookmarks::read(&fs::read(out).unwrap()).unwrap().len(), 3);
}

#[test]
fn accepts_null_links_indirect_style_values_and_pdf2_utf8_titles() {
    let mut doc = Document::load_mem(&fixture()).unwrap();
    let root = doc
        .catalog()
        .unwrap()
        .get(b"Outlines")
        .unwrap()
        .as_reference()
        .unwrap();
    let first = doc
        .get_dictionary(root)
        .unwrap()
        .get(b"First")
        .unwrap()
        .as_reference()
        .unwrap();
    let last = doc
        .get_dictionary(root)
        .unwrap()
        .get(b"Last")
        .unwrap()
        .as_reference()
        .unwrap();
    let flags = doc.add_object(3);
    let count = doc.add_object(-1);
    let red = doc.add_object(Object::Real(1.0));
    let mut title = vec![0xef, 0xbb, 0xbf];
    title.extend_from_slice("UTF8 Ω".as_bytes());
    let d = doc.get_dictionary_mut(first).unwrap();
    d.set("Title", Object::String(title, StringFormat::Literal));
    d.set("F", flags);
    d.set("Count", count);
    d.set("C", vec![Object::Reference(red), 0.into(), 0.into()]);
    let d = doc.get_dictionary_mut(last).unwrap();
    d.set("Next", Object::Null);
    d.set("First", Object::Null);
    d.set("Dest", Object::Null);
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    let items = bookmarks::read(&bytes).unwrap();
    assert_eq!(items[0].title, "UTF8 Ω");
    assert!(items[0].bold && items[0].italic && items[0].closed);
    assert_eq!(items[0].color, Some([1., 0., 0.]));
    assert_eq!(items.len(), 3);
}

#[test]
fn bounded_export_validation_leaves_existing_destination_untouched() {
    use folio_engine::PdfValue;
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.pdf");
    fs::write(&source, fixture()).unwrap();
    let engine = engine();
    let info = engine.open_document(&source).unwrap();
    let good = bookmarks::remap(
        &info.bookmarks,
        &(0..3).map(|i| (i.to_string(), format!("p{i}"))).collect(),
    );
    let output = temp.path().join("keep.pdf");
    fs::write(&output, b"existing file").unwrap();
    let mut cases = vec![];
    let mut duplicate = good.clone();
    duplicate[1].id = duplicate[0].id.clone();
    cases.push(duplicate);
    let mut level = good.clone();
    level[1].level = 32;
    cases.push(level);
    let mut title = good.clone();
    title[0].title = "a".repeat(4097);
    cases.push(title);
    let mut target = good.clone();
    target[0].destination = Some(PdfValue::Array(vec![
        PdfValue::Page("absent".into()),
        PdfValue::Name(b"Fit".to_vec()),
    ]));
    cases.push(target);
    let mut big = good.clone();
    big[2].action = Some(PdfValue::Dictionary(vec![
        (b"S".to_vec(), PdfValue::Name(b"URI".to_vec())),
        (
            b"URI".to_vec(),
            PdfValue::String(vec![65; 4 * 1024 * 1024 + 1]),
        ),
    ]));
    cases.push(big);
    for items in cases {
        assert!(engine
            .export_pdf_with_bookmarks(
                ExportRequest {
                    pages: plans(&info.id),
                    flatten: false
                },
                Some(items),
                &output
            )
            .is_err());
        assert_eq!(fs::read(&output).unwrap(), b"existing file");
    }
}

#[test]
fn preserves_opaque_action_streams_without_executing_them() {
    let mut doc = Document::load_mem(&fixture()).unwrap();
    let root = doc
        .catalog()
        .unwrap()
        .get(b"Outlines")
        .unwrap()
        .as_reference()
        .unwrap();
    let last = doc
        .get_dictionary(root)
        .unwrap()
        .get(b"Last")
        .unwrap()
        .as_reference()
        .unwrap();
    let mut stream = lopdf::Stream::new(
        dictionary! {},
        b"/* retained bookmark action; never executed by Folio */".to_vec(),
    );
    stream.compress().unwrap();
    let stream = doc.add_object(stream);
    doc.get_dictionary_mut(last)
        .unwrap()
        .set("A", dictionary! {"S"=>"JavaScript","JS"=>stream});
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    let before = bookmarks::read(&bytes).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.pdf");
    fs::write(&source, bytes).unwrap();
    let engine = engine();
    let info = engine.open_document(&source).unwrap();
    let output = temp.path().join("copy.pdf");
    engine
        .export_pdf(
            ExportRequest {
                pages: plans(&info.id),
                flatten: false,
            },
            &output,
        )
        .unwrap();
    let after = bookmarks::read(&fs::read(output).unwrap()).unwrap();
    assert_eq!(before[2].action, after[2].action);
}
