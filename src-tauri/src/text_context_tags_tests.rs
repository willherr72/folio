use super::*;
use lopdf::{content::Content, dictionary, Object};

struct Fixture {
    pdf: Document,
    page: ObjectId,
    other: ObjectId,
    root: ObjectId,
    document: ObjectId,
    owner: ObjectId,
    neighbor: ObjectId,
    parent_tree: ObjectId,
}
fn fixture() -> Fixture {
    let mut pdf = Document::new();
    let page = pdf.add_object(dictionary! {"Type"=>"Page","StructParents"=>0});
    let other = pdf.add_object(dictionary! {"Type"=>"Page","StructParents"=>1});
    let root = pdf.new_object_id();
    let document = pdf.new_object_id();
    let owner = pdf.add_object(
        dictionary! {"Type"=>"StructElem","S"=>"P","P"=>document,"Pg"=>page,"K"=>vec![0.into()]},
    );
    let neighbor = pdf
        .add_object(dictionary! {"Type"=>"StructElem","S"=>"P","P"=>document,"Pg"=>other,"K"=>0});
    pdf.objects.insert(document,dictionary! {"Type"=>"StructElem","S"=>"Document","P"=>root,"K"=>vec![Object::Reference(owner),Object::Reference(neighbor)]}.into());
    let owners = pdf.add_object(Object::Array(vec![Object::Reference(owner)]));
    let others = pdf.add_object(Object::Array(vec![Object::Reference(neighbor)]));
    let parent_tree =
        pdf.add_object(dictionary! {"Nums"=>vec![0.into(),owners.into(),1.into(),others.into()]});
    pdf.objects.insert(
        root,
        dictionary! {"Type"=>"StructTreeRoot","K"=>document,"ParentTree"=>parent_tree}.into(),
    );
    let catalog = pdf.add_object(dictionary! {"Type"=>"Catalog","StructTreeRoot"=>root});
    pdf.trailer.set("Root", catalog);
    Fixture {
        pdf,
        page,
        other,
        root,
        document,
        owner,
        neighbor,
        parent_tree,
    }
}
fn selected() -> Vec<ShowContext> {
    super::super::text_context::inspect(
        &Content::decode_strict(b"/Span << /MCID 0 /Lang (en-US) >> BDC BT (word) Tj ET EMC")
            .unwrap()
            .operations,
    )
    .unwrap()
    .shows
    .into_iter()
    .map(|(_, c)| c)
    .collect()
}

#[test]
fn reciprocal_word_topology_and_unrelated_alternate_text_are_preserved() {
    let mut f = fixture();
    prove(&f.pdf, f.page, &selected()).unwrap();
    f.pdf
        .get_dictionary_mut(f.neighbor)
        .unwrap()
        .set("Alt", Object::string_literal("unrelated"));
    prove(&f.pdf, f.page, &selected()).unwrap();
}

fn table_cell() -> Fixture {
    let mut f = fixture();
    let cell = f.pdf.add_object(dictionary! {
        "Type"=>"StructElem", "S"=>"TD", "P"=>f.document, "K"=>f.owner,
        "A"=>vec![
            Object::Dictionary(dictionary! {"O"=>"Table", "Headers"=>vec![Object::string_literal("header-one")]}),
            Object::Dictionary(dictionary! {"O"=>"Table", "RowSpan"=>1}),
            Object::Dictionary(dictionary! {"O"=>"Table", "ColSpan"=>1}),
        ]
    });
    f.pdf.get_dictionary_mut(f.owner).unwrap().set("P", cell);
    f.pdf.get_dictionary_mut(f.neighbor).unwrap().set("S", "TH");
    f.pdf.get_dictionary_mut(f.neighbor).unwrap().set("ID", Object::string_literal("header-one"));
    f.pdf.get_dictionary_mut(f.document).unwrap().set("K", vec![Object::Reference(cell), f.neighbor.into()]);
    f
}

#[test]
fn browser_table_relationship_attributes_preserve_selected_ownership() {
    let mut f = table_cell();
    let before = format!("{:?}", f.pdf.objects);
    prove(&f.pdf, f.page, &selected()).unwrap();
    assert_eq!(format!("{:?}", f.pdf.objects), before);
    let cell = reference(&f.pdf, f.pdf.get_dictionary(f.owner).unwrap().get(b"P").unwrap()).unwrap();
    // Dictionary form and bounded indirect attributes are equally immutable.
    let attributes = f.pdf.add_object(dictionary! {"O"=>"Table", "RowSpan"=>2, "ColSpan"=>3});
    f.pdf.get_dictionary_mut(cell).unwrap().set("A", attributes);
    prove(&f.pdf, f.page, &selected()).unwrap();
    f.pdf.get_dictionary_mut(cell).unwrap().set("S", "TH");
    f.pdf.get_dictionary_mut(attributes).unwrap().set("Scope", "Column");
    prove(&f.pdf, f.page, &selected()).unwrap();
}

#[test]
fn table_attributes_do_not_authorize_stale_or_malformed_semantics() {
    let cases: Vec<Object> = vec![
        dictionary! {"O"=>"Layout", "RowSpan"=>1}.into(),
        dictionary! {"O"=>"Table", "Summary"=>Object::string_literal("old answer")}.into(),
        dictionary! {"O"=>"Table", "BBox"=>vec![0.into(),0.into(),10.into(),10.into()]}.into(),
        dictionary! {"O"=>"Table", "RowSpan"=>0}.into(),
        dictionary! {"O"=>"Table", "ColSpan"=>-1}.into(),
        dictionary! {"O"=>"Table", "RowSpan"=>1.5_f32}.into(),
        dictionary! {"O"=>"Table", "Headers"=>vec![Object::string_literal("")]}.into(),
        dictionary! {"O"=>"Table", "Headers"=>vec![Object::Name(b"header-one".to_vec())]}.into(),
        dictionary! {"O"=>"Table", "Headers"=>vec![Object::string_literal(vec![b'x'; 1025])]}.into(),
        dictionary! {"O"=>"Table", "Scope"=>"Column"}.into(), // Only TH may own Scope.
        Object::Array(vec![dictionary! {"O"=>"Table", "RowSpan"=>1}.into(), 0.into()]),
        Object::Array(vec![]),
        Object::Null,
    ];
    for (index, attributes) in cases.into_iter().enumerate() {
        let mut f = table_cell();
        let cell = reference(&f.pdf, f.pdf.get_dictionary(f.owner).unwrap().get(b"P").unwrap()).unwrap();
        f.pdf.get_dictionary_mut(cell).unwrap().set("A", attributes);
        assert!(prove(&f.pdf, f.page, &selected()).is_err(), "case {index}");
    }
    for key in ["ActualText", "Alt", "E", "C", "Ref", "AF"] {
        let mut f = table_cell();
        f.pdf.get_dictionary_mut(f.owner).unwrap().set(key, Object::string_literal("old"));
        assert!(prove(&f.pdf, f.page, &selected()).is_err(), "{key}");
    }
    let mut f = table_cell();
    let cell = reference(&f.pdf, f.pdf.get_dictionary(f.owner).unwrap().get(b"P").unwrap()).unwrap();
    let cycle = f.pdf.new_object_id();
    f.pdf.objects.insert(cycle, Object::Reference(cycle));
    f.pdf.get_dictionary_mut(cell).unwrap().set("A", cycle);
    assert!(prove(&f.pdf, f.page, &selected()).is_err());
}

#[test]
fn repeated_table_attribute_aliases_share_a_finite_validation_budget() {
    let mut f = fixture();
    let attributes = f.pdf.add_object(dictionary! {"O"=>"Table", "RowSpan"=>1});
    let dict = dictionary! {"A"=>attributes};
    let mut budget = 8;
    assert!(table_relationship_attributes(&f.pdf, &dict, b"TD", &mut budget));
    assert!(table_relationship_attributes(&f.pdf, &dict, b"TD", &mut budget));
    assert_eq!(budget, 0);
    assert!(!table_relationship_attributes(&f.pdf, &dict, b"TD", &mut budget));
    // An unrelated ordinary element consumes no attribute work.
    assert!(table_relationship_attributes(&f.pdf, &Dictionary::new(), b"P", &mut budget));
}

#[test]
fn inherited_page_mcr_and_number_tree_kids_have_unique_ownership() {
    let mut f = fixture();
    f.pdf
        .get_dictionary_mut(f.document)
        .unwrap()
        .set("Pg", f.page);
    let owner = f.pdf.get_dictionary_mut(f.owner).unwrap();
    owner.remove(b"Pg");
    owner.set("K", dictionary! {"Type"=>"MCR","MCID"=>0});
    let branch = f
        .pdf
        .add_object(f.pdf.get_object(f.parent_tree).unwrap().clone());
    f.pdf.objects.insert(
        f.parent_tree,
        dictionary! {"Kids"=>vec![Object::Reference(branch)]}.into(),
    );
    prove(&f.pdf, f.page, &selected()).unwrap();
}

#[test]
fn selected_and_ancestor_semantics_are_not_silently_staled() {
    for target_ancestor in [false, true] {
        for key in ["Alt", "ActualText", "E", "A", "C", "Ref", "AF"] {
            let mut f = fixture();
            let target = if target_ancestor { f.document } else { f.owner };
            f.pdf
                .get_dictionary_mut(target)
                .unwrap()
                .set(key, Object::string_literal("stale"));
            assert!(
                prove(&f.pdf, f.page, &selected()).is_err(),
                "{key} {target_ancestor}"
            );
        }
    }
}

#[test]
fn duplicate_wrong_parent_or_wrong_page_ownership_is_rejected() {
    for case in 0..6 {
        let mut f = fixture();
        match case {
            0 => {
                f.pdf
                    .get_dictionary_mut(f.neighbor)
                    .unwrap()
                    .set("Pg", f.page);
            }
            1 => {
                f.pdf.get_dictionary_mut(f.owner).unwrap().set("P", f.owner);
            }
            2 => {
                f.pdf
                    .get_dictionary_mut(f.owner)
                    .unwrap()
                    .set("Pg", f.other);
            }
            3 => {
                f.pdf.get_dictionary_mut(f.parent_tree).unwrap().set(
                    "Nums",
                    vec![0.into(), Object::Array(vec![f.neighbor.into()])],
                );
            }
            4 => {
                f.pdf.get_dictionary_mut(f.parent_tree).unwrap().set(
                    "Nums",
                    vec![
                        0.into(),
                        Object::Array(vec![f.owner.into()]),
                        0.into(),
                        Object::Array(vec![f.owner.into()]),
                    ],
                );
            }
            _ => {
                f.pdf.get_dictionary_mut(f.document).unwrap().set(
                    "K",
                    vec![Object::Reference(f.owner), Object::Reference(f.owner)],
                );
            }
        }
        assert!(prove(&f.pdf, f.page, &selected()).is_err(), "case {case}");
    }
}

#[test]
fn unrelated_stream_mcr_and_alias_cycles_cannot_evade_validation() {
    let mut f = fixture();
    f.pdf
        .get_dictionary_mut(f.neighbor)
        .unwrap()
        .set("K", dictionary! {"Type"=>"MCR","MCID"=>0,"Stm"=>(999,0)});
    assert!(prove(&f.pdf, f.page, &selected()).is_err());
    let mut f = fixture();
    let cyclic = f.pdf.new_object_id();
    f.pdf.objects.insert(cyclic, Object::Reference(cyclic));
    f.pdf
        .get_dictionary_mut(f.root)
        .unwrap()
        .set("ParentTree", cyclic);
    assert!(prove(&f.pdf, f.page, &selected()).is_err());
}

#[test]
fn untagged_source_has_no_fabricated_ownership() {
    let mut pdf = Document::new();
    let page = pdf.add_object(dictionary! {"Type"=>"Page"});
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog"});
    pdf.trailer.set("Root", root);
    let plain = super::super::text_context::inspect(
        &Content::decode_strict(b"BT (plain) Tj ET")
            .unwrap()
            .operations,
    )
    .unwrap();
    prove(&pdf, page, &[plain.shows[0].1.clone()]).unwrap();
    assert!(prove(&pdf, page, &selected()).is_err());
    let f = fixture();
    assert!(prove(&f.pdf, f.page, &[plain.shows[0].1.clone()]).is_err());
}
