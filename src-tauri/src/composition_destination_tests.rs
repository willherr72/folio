use super::*;
use lopdf::{dictionary, ObjectId, StringFormat};
fn check() -> EngineResult<()> {
    Ok(())
}
fn source(doc: &mut Document) -> SourcePages {
    let page = doc.add_object(dictionary! {"Type"=>"Page"});
    let second = doc.add_object(dictionary! {"Type"=>"Page"});
    let root=doc.add_object(dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(page),Object::Reference(second)],"Count"=>2});
    let catalog = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>root});
    SourcePages {
        catalog,
        root,
        pages: vec![page, second],
    }
}
fn destination(page: ObjectId) -> Object {
    Object::Array(vec![
        page.into(),
        "XYZ".into(),
        Object::Null,
        73.into(),
        Object::Null,
    ])
}
fn install(doc: &mut Document, source: &SourcePages, key: &[u8]) -> (ObjectId, ObjectId) {
    let legacy=doc.add_object(dictionary!{"D"=>destination(source.pages[0]),"Private"=>Object::string_literal("opaque payload")});
    let modern = doc.add_object(destination(source.pages[1]));
    let alias = doc.add_object(Object::Reference(modern));
    let tree=doc.add_object(dictionary!{"Names"=>vec![Object::String(key.to_vec(),StringFormat::Hexadecimal),Object::Reference(alias)]});
    let catalog = doc.get_dictionary_mut(source.catalog).unwrap();
    catalog.set("Dests", dictionary! {key.to_vec()=>legacy});
    catalog.set("Names", dictionary! {"Dests"=>tree});
    (legacy, alias)
}
fn entries(names: &Object) -> Vec<(Vec<u8>, Object)> {
    names
        .as_dict()
        .unwrap()
        .get(b"Dests")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Names")
        .unwrap()
        .as_array()
        .unwrap()
        .chunks_exact(2)
        .map(|pair| (pair[0].as_str().unwrap().to_vec(), pair[1].clone()))
        .collect()
}
#[test]
fn composition_destinations_preserve_separate_namespaces_raw_values_and_source_bindings() {
    let mut doc = Document::with_version("1.7");
    let a = source(&mut doc);
    let b = source(&mut doc);
    let a_ids = install(&mut doc, &a, b"same");
    let b_ids = install(&mut doc, &b, b"same");
    let alias = doc.add_object(Object::string_literal("same"));
    // A preexisting key must never be captured by generated collision names.
    doc.get_dictionary_mut(a.catalog)
        .unwrap()
        .get_mut(b"Dests")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Folio.Dest.1.1", a_ids.0);
    let before = doc.objects.clone();
    let max = doc.max_id;
    let mut prepared = prepare(&doc, &[a, b], &mut check).unwrap();
    let legacy_a = prepared
        .local(0, &Object::Name(b"same".to_vec()), &mut check)
        .unwrap()
        .unwrap();
    let legacy_b = prepared
        .local(1, &Object::Name(b"same".to_vec()), &mut check)
        .unwrap()
        .unwrap();
    let modern_a = prepared
        .local(0, &Object::Reference(alias), &mut check)
        .unwrap()
        .unwrap();
    let modern_b = prepared
        .local(1, &Object::string_literal("same"), &mut check)
        .unwrap()
        .unwrap();
    assert_ne!(legacy_a, legacy_b);
    assert_ne!(modern_a, modern_b);
    assert_ne!(legacy_a.as_name().unwrap(), b"Folio.Dest.1.1");
    assert!(prepared
        .local(0, &Object::Name(b"Folio.Dest.1.1".to_vec()), &mut check)
        .unwrap()
        .is_none());
    let (legacy, modern) = prepared.catalogs(&mut check).unwrap();
    let legacy = legacy.unwrap();
    let legacy = legacy.as_dict().unwrap();
    assert_eq!(
        legacy.get(legacy_a.as_name().unwrap()).unwrap(),
        &Object::Reference(a_ids.0)
    );
    assert_eq!(
        legacy.get(legacy_b.as_name().unwrap()).unwrap(),
        &Object::Reference(b_ids.0)
    );
    let entries = entries(&modern.unwrap());
    assert!(entries.windows(2).all(|p| p[0].0 < p[1].0));
    assert_eq!(
        entries
            .iter()
            .find(|(key, _)| key == modern_a.as_str().unwrap())
            .unwrap()
            .1,
        Object::Reference(a_ids.1)
    );
    assert_eq!(
        entries
            .iter()
            .find(|(key, _)| key == modern_b.as_str().unwrap())
            .unwrap()
            .1,
        Object::Reference(b_ids.1)
    );
    assert_eq!(doc.objects, before);
    assert_eq!(doc.max_id, max);
    assert_eq!(
        doc.objects[&alias],
        Object::string_literal("same"),
        "remote/opaque alias consumers keep their original bytes"
    );
}
#[test]
fn composition_destinations_noncolliding_names_are_stable_and_do_not_cross_namespaces() {
    let mut doc = Document::with_version("1.7");
    let a = source(&mut doc);
    let b = source(&mut doc);
    let a_ids = install(&mut doc, &a, b"same");
    let b_ids = install(&mut doc, &b, b"same");
    doc.get_dictionary_mut(a.catalog).unwrap().remove(b"Names");
    doc.get_dictionary_mut(b.catalog).unwrap().remove(b"Dests");
    let mut prepared = prepare(&doc, &[a, b], &mut check).unwrap();
    assert!(prepared
        .local(0, &Object::Name(b"same".to_vec()), &mut check)
        .unwrap()
        .is_none());
    assert!(prepared
        .local(1, &Object::string_literal("same"), &mut check)
        .unwrap()
        .is_none());
    assert!(prepared
        .local(0, &Object::string_literal("same"), &mut check)
        .is_err());
    assert!(prepared
        .local(1, &Object::Name(b"same".to_vec()), &mut check)
        .is_err());
    assert!(prepared
        .local(2, &Object::Name(b"same".to_vec()), &mut check)
        .is_err());
    let (legacy, modern) = prepared.catalogs(&mut check).unwrap();
    assert_eq!(
        legacy.unwrap().as_dict().unwrap().get(b"same").unwrap(),
        &Object::Reference(a_ids.0)
    );
    assert_eq!(
        entries(&modern.unwrap()),
        vec![(b"same".to_vec(), Object::Reference(b_ids.1))]
    );
}
#[test]
fn composition_destinations_validate_exact_local_arrays_and_page_aliases() {
    let mut doc = Document::with_version("1.7");
    let a = source(&mut doc);
    let b = source(&mut doc);
    let alias = doc.add_object(Object::Reference(a.pages[0]));
    let foreign = b.pages[0];
    let mut valid = vec![];
    for (name, args) in [
        ("Fit", vec![]),
        ("FitB", vec![]),
        ("FitH", vec![Object::Null]),
        ("FitBH", vec![2.into()]),
        ("FitV", vec![Object::Null]),
        ("FitBV", vec![(-2).into()]),
        ("XYZ", vec![Object::Null, 2.into(), 0.into()]),
        ("FitR", vec![0.into(), 0.into(), 20.into(), 30.into()]),
    ] {
        let mut values = vec![
            Object::Reference(alias),
            Object::Name(name.as_bytes().to_vec()),
        ];
        values.extend(args);
        valid.push(Object::Array(values));
    }
    let mut prepared = prepare(&doc, &[a, b], &mut check).unwrap();
    for value in valid {
        assert!(
            prepared.local(0, &value, &mut check).unwrap().is_none(),
            "{value:?}"
        );
    }
    for value in [
        Object::Array(vec![foreign.into(), "Fit".into()]),
        Object::Array(vec![0.into(), "Fit".into()]),
        Object::Array(vec![alias.into(), "Fit".into(), 0.into()]),
        Object::Array(vec![
            alias.into(),
            "XYZ".into(),
            Object::Null,
            Object::Null,
            (-1).into(),
        ]),
        Object::Array(vec![
            alias.into(),
            "FitH".into(),
            Object::Real(f32::INFINITY),
        ]),
        Object::Array(vec![
            alias.into(),
            "FitR".into(),
            Object::Null,
            0.into(),
            20.into(),
            30.into(),
        ]),
        Object::Array(vec![alias.into(), "Unknown".into()]),
    ] {
        assert!(prepared.local(0, &value, &mut check).is_err(), "{value:?}");
    }
}
fn branching(doc: &mut Document, a: &SourcePages) -> (ObjectId, ObjectId) {
    let leaf=doc.add_object(dictionary!{"Limits"=>vec![Object::string_literal("a"),Object::string_literal("z")],"Names"=>vec![Object::string_literal("a"),destination(a.pages[0]),Object::string_literal("z"),destination(a.pages[1])]});
    let alias = doc.add_object(Object::Reference(leaf));
    let root = doc.add_object(dictionary! {"Kids"=>vec![Object::Reference(alias)]});
    doc.get_dictionary_mut(a.catalog)
        .unwrap()
        .set("Names", dictionary! {"Dests"=>root});
    (root, leaf)
}
#[test]
fn composition_destinations_name_tree_order_limits_and_cycles_are_checked() {
    for fault in [
        "valid",
        "wrong-limit",
        "missing-limit",
        "unsorted",
        "duplicate",
        "both",
        "cycle",
        "foreign",
        "integer",
        "extension",
        "other-tree",
    ] {
        let mut doc = Document::with_version("1.7");
        let a = source(&mut doc);
        let b = source(&mut doc);
        let (root, leaf) = branching(&mut doc, &a);
        match fault {
            "wrong-limit" => doc.get_dictionary_mut(leaf).unwrap().set(
                "Limits",
                vec![Object::string_literal("b"), Object::string_literal("z")],
            ),
            "missing-limit" => {
                doc.get_dictionary_mut(leaf).unwrap().remove(b"Limits");
            }
            "unsorted" | "duplicate" => doc.get_dictionary_mut(leaf).unwrap().set(
                "Names",
                vec![
                    Object::string_literal("z"),
                    destination(a.pages[0]),
                    Object::string_literal(if fault == "duplicate" { "z" } else { "a" }),
                    destination(a.pages[1]),
                ],
            ),
            "both" => doc
                .get_dictionary_mut(root)
                .unwrap()
                .set("Names", Vec::<Object>::new()),
            "cycle" => doc
                .get_dictionary_mut(root)
                .unwrap()
                .set("Kids", vec![Object::Reference(root)]),
            "foreign" | "integer" => doc.get_dictionary_mut(leaf).unwrap().set(
                "Names",
                vec![
                    Object::string_literal("a"),
                    Object::Array(vec![
                        if fault == "foreign" {
                            b.pages[0].into()
                        } else {
                            0.into()
                        },
                        "Fit".into(),
                    ]),
                    Object::string_literal("z"),
                    destination(a.pages[1]),
                ],
            ),
            "extension" => doc.get_dictionary_mut(root).unwrap().set("Private", true),
            "other-tree" => doc
                .get_dictionary_mut(a.catalog)
                .unwrap()
                .get_mut(b"Names")
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("JavaScript", dictionary! {"Names"=>Vec::<Object>::new()}),
            _ => {}
        }
        let before = doc.objects.clone();
        assert_eq!(
            prepare(&doc, &[a, b], &mut check).is_ok(),
            fault == "valid",
            "{fault}"
        );
        assert_eq!(doc.objects, before);
    }
}
#[test]
fn composition_destinations_cancellation_and_staged_metadata_are_bounded() {
    let mut doc = Document::with_version("1.7");
    let a = source(&mut doc);
    let b = source(&mut doc);
    install(&mut doc, &a, b"same");
    install(&mut doc, &b, b"same");
    let sources = [a, b];
    let before = doc.objects.clone();
    let mut calls = 0;
    prepare(&doc, &sources, &mut || {
        calls += 1;
        Ok(())
    })
    .unwrap();
    for stop in [1, calls / 2, calls] {
        let mut n = 0;
        assert!(prepare(&doc, &sources, &mut || {
            n += 1;
            if n == stop {
                Err(EngineError::InvalidRequest("cancel".into()))
            } else {
                Ok(())
            }
        })
        .is_err());
    }
    assert_eq!(doc.objects, before);
    let mut prepared = prepare(&doc, &sources, &mut check).unwrap();
    let mut calls = 0;
    prepared
        .catalogs(&mut || {
            calls += 1;
            Ok(())
        })
        .unwrap();
    for stop in [1, calls / 2, calls] {
        let mut prepared = prepare(&doc, &sources, &mut check).unwrap();
        let mut n = 0;
        assert!(prepared
            .catalogs(&mut || {
                n += 1;
                if n == stop {
                    Err(EngineError::InvalidRequest("cancel staged catalogs".into()))
                } else {
                    Ok(())
                }
            })
            .is_err());
    }
    assert_eq!(doc.objects, before);
    // A direct destination dictionary carries opaque data that must be bounded
    // before cloning for the merged catalog, even though its D is tiny.
    doc.get_dictionary_mut(sources[0].catalog).unwrap().set("Dests",dictionary!{"large"=>dictionary!{"D"=>destination(sources[0].pages[0]),"Private"=>Object::String(vec![b'x';16*1024*1024],StringFormat::Literal)}});
    let mut prepared = prepare(&doc, &sources, &mut check).unwrap();
    assert!(prepared.catalogs(&mut check).is_err());
}

#[test]
fn composition_destinations_saved_catalogs_agree_with_portable_bookmark_projection() {
    let mut doc = Document::with_version("1.7");
    let a = source(&mut doc);
    let b = source(&mut doc);
    install(&mut doc, &a, b"same");
    install(&mut doc, &b, b"same");
    let sources = [a, b];
    let mut prepared = prepare(&doc, &sources, &mut check).unwrap();
    let mut operands = vec![];
    for source in 0..2 {
        for value in [
            Object::Name(b"same".to_vec()),
            Object::string_literal("same"),
        ] {
            operands.push(
                prepared
                    .local(source, &value, &mut check)
                    .unwrap()
                    .unwrap_or(value),
            );
        }
    }
    let (legacy, modern) = prepared.catalogs(&mut check).unwrap();
    drop(prepared);
    let pages = sources
        .iter()
        .flat_map(|s| s.pages.iter().copied())
        .collect::<Vec<_>>();
    let root=doc.add_object(dictionary!{"Type"=>"Pages","Count"=>4,"Kids"=>pages.iter().map(|id|Object::Reference(*id)).collect::<Vec<_>>()});
    for page in pages {
        doc.get_dictionary_mut(page).unwrap().set("Parent", root);
    }
    let outlines = doc.new_object_id();
    let ids = (0..4).map(|_| doc.new_object_id()).collect::<Vec<_>>();
    for (index, (id, operand)) in ids.iter().zip(operands).enumerate() {
        let mut node = dictionary! {"Title"=>Object::string_literal(format!("destination {index}")),"Parent"=>outlines,"Dest"=>operand};
        if index > 0 {
            node.set("Prev", ids[index - 1]);
        }
        if index < 3 {
            node.set("Next", ids[index + 1]);
        }
        doc.objects.insert(*id, node.into());
    }
    doc.objects.insert(
        outlines,
        dictionary! {"Type"=>"Outlines","First"=>ids[0],"Last"=>ids[3],"Count"=>4}.into(),
    );
    let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>root,"Outlines"=>outlines,"Dests"=>legacy.unwrap(),"Names"=>modern.unwrap()});
    doc.trailer.set("Root", catalog);
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    let projection = crate::bookmarks::read(&bytes).unwrap();
    assert_eq!(
        projection
            .iter()
            .map(crate::bookmarks::primary_page)
            .collect::<Vec<_>>(),
        vec![Some("0"), Some("1"), Some("2"), Some("3")]
    );
}

#[test]
fn composition_destinations_missing_objects_alias_depth_and_tree_depth_refuse() {
    for length in [32, 33] {
        let mut doc = Document::with_version("1.7");
        let a = source(&mut doc);
        let mut target = destination(a.pages[0]);
        for _ in 0..length {
            target = Object::Reference(doc.add_object(target));
        }
        doc.get_dictionary_mut(a.catalog)
            .unwrap()
            .set("Dests", dictionary! {"target"=>target});
        assert_eq!(
            prepare(&doc, &[a], &mut check).is_ok(),
            length == 32,
            "alias depth {length}"
        );
    }
    for depth in [32, 33] {
        let mut doc = Document::with_version("1.7");
        let a = source(&mut doc);
        let limits = vec![Object::string_literal("a"), Object::string_literal("a")];
        let mut tree=doc.add_object(dictionary!{"Names"=>vec![Object::string_literal("a"),destination(a.pages[0])],"Limits"=>limits.clone()});
        for _ in 0..depth {
            tree = doc.add_object(
                dictionary! {"Kids"=>vec![Object::Reference(tree)],"Limits"=>limits.clone()},
            );
        }
        doc.get_dictionary_mut(a.catalog)
            .unwrap()
            .set("Names", dictionary! {"Dests"=>tree});
        assert_eq!(
            prepare(&doc, &[a], &mut check).is_ok(),
            depth == 32,
            "tree depth {depth}"
        );
    }
    let mut doc = Document::with_version("1.7");
    let mut a = source(&mut doc);
    let id = doc.new_object_id();
    doc.objects.insert(id, Object::Reference(id));
    doc.get_dictionary_mut(a.catalog)
        .unwrap()
        .set("Dests", Object::Reference(id));
    assert!(prepare(&doc, std::slice::from_ref(&a), &mut check).is_err());
    doc.get_dictionary_mut(a.catalog)
        .unwrap()
        .set("Dests", Object::Reference((u32::MAX, 0)));
    assert!(prepare(&doc, std::slice::from_ref(&a), &mut check).is_err());
    a.catalog = (u32::MAX, 0);
    assert!(prepare(&doc, &[a], &mut check).is_err());
}
