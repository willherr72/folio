use super::*;
use crate::composition_structure::StructureInfo;
use lopdf::{dictionary, ObjectId};
fn check() -> EngineResult<()> {
    Ok(())
}
struct Fixture {
    pages: SourcePages,
    outline: OutlineInfo,
    link: ObjectId,
    comment: ObjectId,
    local: ObjectId,
    remote: ObjectId,
    chain: ObjectId,
    sequence: ObjectId,
    operand: ObjectId,
    opaque: ObjectId,
}
fn fixture(doc: &mut Document, key: &str) -> Fixture {
    let page0 = doc.add_object(dictionary! {"Type"=>"Page"});
    let page1 = doc.add_object(dictionary! {"Type"=>"Page"});
    let root=doc.add_object(dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(page0),Object::Reference(page1)],"Count"=>2});
    let operand = doc.add_object(Object::string_literal(key));
    let local=doc.add_object(dictionary!{"Type"=>"Action","S"=>"GoTo","D"=>operand,"Private"=>Object::string_literal("opaque action metadata")});
    let remote=doc.add_object(dictionary!{"S"=>"GoToR","D"=>operand,"F"=>Object::string_literal("external.pdf"),"NewWindow"=>true});
    let sequence = doc.add_object(vec![Object::Reference(local), Object::Reference(remote)]);
    let chain=doc.add_object(dictionary!{"S"=>"URI","URI"=>Object::string_literal("https://example.com/"),"Next"=>sequence});
    let link=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Link","P"=>page0,"StructParent"=>17,"Rect"=>vec![0.into(),0.into(),10.into(),10.into()],"A"=>chain,"AA"=>dictionary!{"E"=>local}});
    let comment=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","P"=>page0,"Rect"=>vec![20.into(),0.into(),30.into(),10.into()],"Contents"=>Object::string_literal("note"),"AA"=>dictionary!{"PO"=>local}});
    let annotations = doc.add_object(vec![Object::Reference(link), Object::Reference(comment)]);
    doc.get_dictionary_mut(page0)
        .unwrap()
        .set("Annots", annotations);
    doc.get_dictionary_mut(page0)
        .unwrap()
        .set("AA", dictionary! {"O"=>local});
    let outline_root = doc.new_object_id();
    let item=doc.add_object(dictionary!{"Title"=>Object::string_literal("Action bookmark"),"Parent"=>outline_root,"A"=>chain});
    doc.objects.insert(
        outline_root,
        dictionary! {"Type"=>"Outlines","Count"=>1,"First"=>item,"Last"=>item}.into(),
    );
    let target=doc.add_object(dictionary!{"D"=>vec![Object::Reference(page1),"XYZ".into(),Object::Null,73.into(),Object::Null],"Private"=>operand});
    let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>root,"Outlines"=>outline_root,"Names"=>dictionary!{"Dests"=>dictionary!{"Names"=>vec![Object::string_literal(key),Object::Reference(target)]}}});
    let opaque = doc.add_object(dictionary! {"D"=>operand,"A"=>local,"Next"=>sequence});
    let outline =
        crate::composition_outlines::inspect(doc, catalog, &StructureInfo::default(), &mut check)
            .unwrap();
    Fixture {
        pages: SourcePages {
            catalog,
            root,
            pages: vec![page0, page1],
        },
        outline,
        link,
        comment,
        local,
        remote,
        chain,
        sequence,
        operand,
        opaque,
    }
}
fn source(f: &Fixture) -> SourcePages {
    SourcePages {
        catalog: f.pages.catalog,
        root: f.pages.root,
        pages: f.pages.pages.clone(),
    }
}
fn raw<'a>(doc: &'a Document, value: &'a Object) -> &'a Object {
    doc.dereference(value).unwrap().1
}
fn action_target(doc: &Document, action: &Object, names: &Object) -> ObjectId {
    let dictionary = raw(doc, action).as_dict().unwrap();
    assert_eq!(dictionary.get(b"S").unwrap().as_name().unwrap(), b"GoTo");
    let key = raw(doc, dictionary.get(b"D").unwrap()).as_str().unwrap();
    let pairs = names
        .as_dict()
        .unwrap()
        .get(b"Dests")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Names")
        .unwrap()
        .as_array()
        .unwrap();
    let target = &pairs
        .chunks_exact(2)
        .find(|pair| pair[0].as_str().unwrap() == key)
        .unwrap()[1];
    let target = raw(doc, target)
        .as_dict()
        .unwrap()
        .get(b"D")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(
        &target[1..],
        &[
            Object::Name(b"XYZ".to_vec()),
            Object::Null,
            73.into(),
            Object::Null
        ]
    );
    target[0].as_reference().unwrap()
}
#[test]
fn composition_navigation_binds_all_typed_sites_and_keeps_remote_and_opaque_aliases() {
    let mut doc = Document::with_version("1.7");
    let a = fixture(&mut doc, "same");
    let b = fixture(&mut doc, "same");
    let before = doc.objects.clone();
    let values = reconcile(
        &mut doc,
        &[source(&a), source(&b)],
        &[a.outline.clone(), b.outline.clone()],
        &mut check,
    )
    .unwrap();
    let names = values.names.unwrap();
    assert!(values.dests.is_none());
    for f in [&a, &b] {
        let link = doc.get_dictionary(f.link).unwrap();
        assert_eq!(link.get(b"StructParent").unwrap().as_i64().unwrap(), 17);
        let new_chain = link.get(b"A").unwrap();
        assert_ne!(new_chain, &Object::Reference(f.chain));
        assert_eq!(
            doc.get_dictionary(f.outline.items[0])
                .unwrap()
                .get(b"A")
                .unwrap(),
            new_chain,
            "shared typed actions share one revised object"
        );
        let new_sequence = raw(
            &doc,
            raw(&doc, new_chain)
                .as_dict()
                .unwrap()
                .get(b"Next")
                .unwrap(),
        )
        .as_array()
        .unwrap();
        assert_eq!(new_sequence[1], Object::Reference(f.remote));
        let new_local = &new_sequence[0];
        assert_ne!(new_local, &Object::Reference(f.local));
        assert_eq!(action_target(&doc, new_local, &names), f.pages.pages[1]);
        for (owner, trigger) in [
            (f.link, b"E".as_slice()),
            (f.comment, b"PO"),
            (f.pages.pages[0], b"O"),
        ] {
            let additional = raw(&doc, doc.get_dictionary(owner).unwrap().get(b"AA").unwrap())
                .as_dict()
                .unwrap();
            assert_eq!(additional.get(trigger).unwrap(), new_local);
        }
        for id in [f.local, f.remote, f.chain, f.sequence, f.operand, f.opaque] {
            assert_eq!(
                doc.objects[&id], before[&id],
                "original shared object {id:?}"
            );
        }
        assert_eq!(
            doc.get_dictionary(f.link).unwrap().get(b"P").unwrap(),
            &Object::Reference(f.pages.pages[0])
        );
    }
}
#[test]
fn composition_navigation_keeps_unrenamed_graphs_exact_and_rewrites_direct_annotations() {
    let mut doc = Document::with_version("1.7");
    let a = fixture(&mut doc, "one");
    let b = fixture(&mut doc, "two");
    let before = doc.objects.clone();
    let max = doc.max_id;
    reconcile(
        &mut doc,
        &[source(&a), source(&b)],
        &[a.outline.clone(), b.outline.clone()],
        &mut check,
    )
    .unwrap();
    assert_eq!(doc.objects, before);
    assert_eq!(doc.max_id, max);
    let mut doc = Document::with_version("1.7");
    let a = fixture(&mut doc, "same");
    let b = fixture(&mut doc, "same");
    doc.get_dictionary_mut(a.pages.pages[1]).unwrap().set("Annots",vec![Object::Dictionary(dictionary!{"Subtype"=>"Link","Rect"=>vec![0.into(),0.into(),10.into(),10.into()],"Dest"=>Object::string_literal("same")})]);
    let values = reconcile(
        &mut doc,
        &[source(&a), source(&b)],
        &[a.outline.clone(), b.outline.clone()],
        &mut check,
    )
    .unwrap();
    let link = raw(
        &doc,
        doc.get_dictionary(a.pages.pages[1])
            .unwrap()
            .get(b"Annots")
            .unwrap(),
    )
    .as_array()
    .unwrap()[0]
        .as_dict()
        .unwrap();
    let action =
        Object::Dictionary(dictionary! {"S"=>"GoTo","D"=>link.get(b"Dest").unwrap().clone()});
    assert_eq!(
        action_target(&doc, &action, &values.names.unwrap()),
        a.pages.pages[1]
    );
}
#[test]
fn composition_navigation_invalid_actions_foreign_targets_and_opaque_dependencies_refuse_atomically(
) {
    for fault in [
        "missing-d",
        "foreign-page",
        "both",
        "cycle",
        "missing-object",
        "script",
        "unknown",
        "unknown-trigger",
        "nonlink-a",
        "nonwidget-focus",
        "bad-remote",
        "bad-type",
    ] {
        let mut doc = Document::with_version("1.7");
        let a = fixture(&mut doc, "same");
        let b = fixture(&mut doc, "same");
        match fault {
            "missing-d" => {
                doc.get_dictionary_mut(a.local).unwrap().remove(b"D");
            }
            "foreign-page" => doc
                .get_dictionary_mut(a.local)
                .unwrap()
                .set("D", vec![Object::Reference(b.pages.pages[1]), "Fit".into()]),
            "both" => doc
                .get_dictionary_mut(a.link)
                .unwrap()
                .set("Dest", Object::string_literal("same")),
            "cycle" => doc
                .get_dictionary_mut(a.local)
                .unwrap()
                .set("Next", a.chain),
            "missing-object" => doc
                .get_dictionary_mut(a.local)
                .unwrap()
                .set("Next", Object::Reference((u32::MAX, 0))),
            "script" => {
                doc.get_dictionary_mut(a.remote)
                    .unwrap()
                    .set("S", "JavaScript");
                doc.get_dictionary_mut(a.remote)
                    .unwrap()
                    .set("JS", Object::string_literal("this.gotoNamedDest('same')"));
            }
            "unknown" => doc
                .get_dictionary_mut(a.remote)
                .unwrap()
                .set("S", "VendorAction"),
            "unknown-trigger" => doc
                .get_dictionary_mut(a.pages.pages[1])
                .unwrap()
                .set("AA", dictionary! {"VendorTrigger"=>a.local}),
            "nonlink-a" => doc.get_dictionary_mut(a.comment).unwrap().set("A", a.local),
            "nonwidget-focus" => doc
                .get_dictionary_mut(a.comment)
                .unwrap()
                .set("AA", dictionary! {"Fo"=>a.local}),
            "bad-remote" => doc
                .get_dictionary_mut(a.remote)
                .unwrap()
                .set("D", vec![Object::Reference(a.pages.pages[0]), "Fit".into()]),
            "bad-type" => doc.get_dictionary_mut(a.local).unwrap().set("Type", "Bad"),
            _ => unreachable!(),
        }
        let before = doc.objects.clone();
        let max = doc.max_id;
        assert!(
            reconcile(
                &mut doc,
                &[source(&a), source(&b)],
                &[a.outline.clone(), b.outline.clone()],
                &mut check
            )
            .is_err(),
            "{fault}"
        );
        assert_eq!(doc.objects, before, "{fault}");
        assert_eq!(doc.max_id, max, "{fault}");
    }
}
#[test]
fn composition_navigation_final_cancellation_and_allocation_failure_are_atomic() {
    let mut doc = Document::with_version("1.7");
    let a = fixture(&mut doc, "same");
    let b = fixture(&mut doc, "same");
    let before = doc.objects.clone();
    let max = doc.max_id;
    let sources = [source(&a), source(&b)];
    let outlines = [a.outline.clone(), b.outline.clone()];
    let mut probe = doc.clone();
    let mut calls = 0;
    reconcile(&mut probe, &sources, &outlines, &mut || {
        calls += 1;
        Ok(())
    })
    .unwrap();
    for stop in [1, calls / 2, calls - 1, calls] {
        let mut n = 0;
        assert!(reconcile(&mut doc, &sources, &outlines, &mut || {
            n += 1;
            if n == stop {
                Err(EngineError::InvalidRequest("cancel".into()))
            } else {
                Ok(())
            }
        })
        .is_err());
        assert_eq!(doc.objects, before);
        assert_eq!(doc.max_id, max);
    }
    doc.max_id = u32::MAX;
    assert!(reconcile(&mut doc, &sources, &outlines, &mut check).is_err());
    assert_eq!(doc.objects, before);
    assert_eq!(doc.max_id, u32::MAX);
}

#[test]
fn composition_navigation_presentation_nodes_cannot_keep_unqualified_local_actions() {
    for collision in [false, true] {
        let mut doc = Document::with_version("1.7");
        let a = fixture(&mut doc, "one");
        let b = fixture(&mut doc, if collision { "one" } else { "two" });
        let node = doc.add_object(dictionary! {"Type"=>"NavNode","NA"=>a.local});
        doc.get_dictionary_mut(a.pages.pages[0])
            .unwrap()
            .set("PresSteps", node);
        let before = doc.objects.clone();
        let max = doc.max_id;
        let result = reconcile(
            &mut doc,
            &[source(&a), source(&b)],
            &[a.outline.clone(), b.outline.clone()],
            &mut check,
        );
        assert_eq!(
            result.is_err(),
            collision,
            "presentation navigation needs its own adapter before qualification"
        );
        assert_eq!(doc.objects, before);
        assert_eq!(doc.max_id, max);
    }
}

fn chain(doc: &mut Document, mut tail: ObjectId, length: usize) -> ObjectId {
    for _ in 0..length {
        tail = doc.add_object(dictionary! {"S"=>"Named","N"=>"NextPage","Next"=>tail});
    }
    tail
}
#[test]
fn composition_navigation_cached_action_height_cannot_bypass_depth_limit() {
    for prefix in [15, 16] {
        let mut doc = Document::with_version("1.7");
        let a = fixture(&mut doc, "same");
        let b = fixture(&mut doc, "same");
        let tail = chain(&mut doc, a.local, 15);
        doc.get_dictionary_mut(a.outline.items[0])
            .unwrap()
            .set("A", tail);
        let head = chain(&mut doc, tail, prefix);
        doc.get_dictionary_mut(a.pages.pages[0])
            .unwrap()
            .set("AA", dictionary! {"O"=>head});
        let before = doc.objects.clone();
        let max = doc.max_id;
        let result = reconcile(
            &mut doc,
            &[source(&a), source(&b)],
            &[a.outline.clone(), b.outline.clone()],
            &mut check,
        );
        assert_eq!(result.is_ok(), prefix == 15, "prefix {prefix}");
        if result.is_err() {
            assert_eq!(doc.objects, before);
            assert_eq!(doc.max_id, max);
        }
    }
}

#[test]
fn composition_navigation_many_links_share_one_rewritten_action_and_keep_annotations() {
    let mut doc = Document::with_version("1.7");
    let a = fixture(&mut doc, "same");
    let b = fixture(&mut doc, "same");
    let alias = doc.add_object(Object::Reference(a.local));
    let mut links = vec![];
    for index in 0..2000 {
        links.push(doc.add_object(dictionary!{"Subtype"=>"Link","P"=>a.pages.pages[1],"Rect"=>vec![0.into(),0.into(),10.into(),10.into()],"NM"=>Object::string_literal(format!("link {index}")),"A"=>alias}));
    }
    doc.get_dictionary_mut(a.pages.pages[1]).unwrap().set(
        "Annots",
        links
            .iter()
            .map(|id| Object::Reference(*id))
            .collect::<Vec<_>>(),
    );
    let count = doc.objects.len();
    let original = doc.objects[&a.local].clone();
    let values = reconcile(
        &mut doc,
        &[source(&a), source(&b)],
        &[a.outline.clone(), b.outline.clone()],
        &mut check,
    )
    .unwrap();
    let target = doc.get_dictionary(links[0]).unwrap().get(b"A").unwrap();
    assert_ne!(target, &Object::Reference(alias));
    assert_eq!(
        action_target(&doc, target, &values.names.unwrap()),
        a.pages.pages[1]
    );
    for (index, id) in links.iter().enumerate() {
        let link = doc.get_dictionary(*id).unwrap();
        assert_eq!(link.get(b"A").unwrap(), target);
        assert_eq!(
            link.get(b"NM").unwrap().as_str().unwrap(),
            format!("link {index}").as_bytes()
        );
    }
    assert!(
        doc.objects.len() - count < 10,
        "shared actions must not expand once per link"
    );
    assert_eq!(doc.objects[&a.local], original);
    assert_eq!(doc.objects[&alias], Object::Reference(a.local));
}

#[test]
fn composition_navigation_staged_action_metadata_budget_is_cumulative_and_atomic() {
    for size in [1024, 9 * 1024 * 1024] {
        let mut doc = Document::with_version("1.7");
        let a = fixture(&mut doc, "same");
        let b = fixture(&mut doc, "same");
        for f in [&a, &b] {
            doc.get_dictionary_mut(f.local)
                .unwrap()
                .set("Private", Object::string_literal(vec![b'x'; size]));
        }
        let before = doc.objects.clone();
        let max = doc.max_id;
        let result = reconcile(
            &mut doc,
            &[source(&a), source(&b)],
            &[a.outline.clone(), b.outline.clone()],
            &mut check,
        );
        assert_eq!(result.is_ok(), size == 1024);
        if result.is_err() {
            assert_eq!(doc.objects, before);
            assert_eq!(doc.max_id, max);
        }
    }
}

#[test]
fn composition_navigation_unqualified_scripts_and_extensions_stay_exact_but_catalog_actions_remain_guarded(
) {
    let mut doc = Document::with_version("1.7");
    let a = fixture(&mut doc, "one");
    let b = fixture(&mut doc, "two");
    doc.get_dictionary_mut(a.remote)
        .unwrap()
        .set("S", "JavaScript");
    doc.get_dictionary_mut(a.remote)
        .unwrap()
        .set("JS", Object::string_literal("this.gotoNamedDest('one')"));
    doc.get_dictionary_mut(b.remote)
        .unwrap()
        .set("S", "VendorAction");
    let before = doc.objects.clone();
    let max = doc.max_id;
    reconcile(
        &mut doc,
        &[source(&a), source(&b)],
        &[a.outline.clone(), b.outline.clone()],
        &mut check,
    )
    .unwrap();
    assert_eq!(doc.objects, before);
    assert_eq!(doc.max_id, max);
    for field in ["OpenAction", "AA"] {
        doc.get_dictionary_mut(a.pages.catalog)
            .unwrap()
            .set(field, Object::Null);
        reconcile(
            &mut doc,
            &[source(&a), source(&b)],
            &[a.outline.clone(), b.outline.clone()],
            &mut check,
        )
        .unwrap();
        doc.get_dictionary_mut(a.pages.catalog).unwrap().set(
            field,
            if field == "AA" {
                Object::Dictionary(dictionary! {"WC"=>a.local})
            } else {
                Object::Reference(a.local)
            },
        );
        let before = doc.objects.clone();
        assert!(reconcile(
            &mut doc,
            &[source(&a), source(&b)],
            &[a.outline.clone(), b.outline.clone()],
            &mut check
        )
        .is_err());
        assert_eq!(doc.objects, before);
        doc.get_dictionary_mut(a.pages.catalog)
            .unwrap()
            .remove(field.as_bytes());
    }
}

#[test]
fn composition_navigation_selection_cancellation_is_atomic_at_every_checkpoint() {
    let mut doc=Document::with_version("1.7");let f=fixture(&mut doc,"chapter");
    let keep=BTreeSet::from([f.pages.pages[0]]);let before=doc.objects.clone();let max=doc.max_id;
    let mut calls=0;let mut probe=doc.clone();
    project(&mut probe,&source(&f),&StructureInfo::default(),&keep,&BTreeSet::new(),&mut ||{calls+=1;Ok(())}).unwrap();
    assert!(calls>100);
    for stop in 1..=calls {
        let mut candidate=doc.clone();let mut step=0;
        let result=project(&mut candidate,&source(&f),&StructureInfo::default(),&keep,&BTreeSet::new(),&mut ||{step+=1;if step==stop {Err(bad("cancel"))}else{Ok(())}});
        assert!(result.is_err(),"checkpoint {stop}");assert_eq!(candidate.objects,before,"checkpoint {stop}");assert_eq!(candidate.max_id,max);
    }
}

#[test]
fn composition_navigation_selection_unknown_contexts_and_deleted_action_cycles_refuse_atomically() {
    for fault in ["script","vendor","trigger","presentation","widget","nonlink","cycle","invalid-removed-name"] {
        let mut doc=Document::with_version("1.7");let f=fixture(&mut doc,"chapter");
        match fault {
            "script"=>{doc.get_dictionary_mut(f.remote).unwrap().set("S","JavaScript");doc.get_dictionary_mut(f.remote).unwrap().set("JS",Object::string_literal("go()"));},
            "vendor"=>doc.get_dictionary_mut(f.remote).unwrap().set("S","VendorAction"),
            "trigger"=>doc.get_dictionary_mut(f.pages.pages[0]).unwrap().set("AA",dictionary!{"Vendor"=>f.local}),
            "presentation"=>doc.get_dictionary_mut(f.pages.pages[0]).unwrap().set("PresSteps",dictionary!{"NA"=>f.local}),
            "widget"=>doc.get_dictionary_mut(f.link).unwrap().set("Subtype","Widget"),
            "nonlink"=>doc.get_dictionary_mut(f.comment).unwrap().set("A",f.local),
            "cycle"=>doc.get_dictionary_mut(f.local).unwrap().set("Next",f.local),
            _=>doc.get_dictionary_mut(f.pages.catalog).unwrap().set("Dests",dictionary!{"unused"=>vec![Object::Reference(f.pages.pages[1]),"InvalidFit".into()]}),
        }
        let before=doc.objects.clone();let max=doc.max_id;
        assert!(project(&mut doc,&source(&f),&StructureInfo::default(),&BTreeSet::from([f.pages.pages[0]]),&BTreeSet::new(),&mut check).is_err(),"{fault}");
        assert_eq!(doc.objects,before,"{fault}");assert_eq!(doc.max_id,max);
    }
}

#[test]
fn composition_navigation_selection_cached_removed_action_height_and_copy_budget_are_bounded() {
    for prefix in [15,16] {
        let mut doc=Document::with_version("1.7");let f=fixture(&mut doc,"chapter");
        let tail=chain(&mut doc,f.local,15);doc.get_dictionary_mut(f.outline.items[0]).unwrap().set("A",tail);
        let head=chain(&mut doc,tail,prefix);doc.get_dictionary_mut(f.pages.pages[0]).unwrap().set("AA",dictionary!{"O"=>head});
        let before=doc.objects.clone();
        let result=project(&mut doc,&source(&f),&StructureInfo::default(),&BTreeSet::from([f.pages.pages[0]]),&BTreeSet::new(),&mut check);
        assert_eq!(result.is_ok(),prefix==15,"prefix {prefix}");if result.is_err(){assert_eq!(doc.objects,before);}
    }
    let mut doc=Document::with_version("1.7");let f=fixture(&mut doc,"chapter");
    doc.get_dictionary_mut(f.chain).unwrap().set("Private",Object::string_literal(vec![b'x';17*1024*1024]));
    let before=doc.objects.clone();
    assert!(project(&mut doc,&source(&f),&StructureInfo::default(),&BTreeSet::from([f.pages.pages[0]]),&BTreeSet::new(),&mut check).is_err());
    assert_eq!(doc.objects,before);
}
