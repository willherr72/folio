use super::*;
use lopdf::dictionary;

fn document(resources: Vec<Dictionary>) -> (Document, Vec<ObjectId>) {
    let mut doc = Document::new();
    let tree = doc.new_object_id();
    let pages: Vec<_> = resources
        .into_iter()
        .map(|resources| {
            doc.add_object(dictionary! {"Type"=>"Page", "Parent"=>tree, "Resources"=>resources})
        })
        .collect();
    doc.objects.insert(
        tree,
        dictionary! {"Type"=>"Pages", "Count"=>pages.len() as i64,
        "Kids"=>pages.iter().copied().map(Object::Reference).collect::<Vec<_>>()}
        .into(),
    );
    let root = doc.add_object(dictionary! {"Type"=>"Catalog", "Pages"=>tree});
    doc.trailer.set("Root", root);
    (doc, pages)
}
fn appearance(resources: Option<Dictionary>) -> Stream {
    let mut dict = dictionary! {"Type"=>"XObject", "Subtype"=>"Form", "BBox"=>vec![0.into(),0.into(),100.into(),40.into()]};
    if let Some(resources) = resources {
        dict.set("Resources", resources);
    }
    // Resource-context tests need an actual consumer. Empty programs correctly
    // leave unused inherited categories unlocalized; that case is tested below.
    Stream::new(dict, b"BT /F1 12 Tf (Old) Tj ET".to_vec())
}
fn font(name: &str) -> Object {
    dictionary! {"Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>name}.into()
}

#[test]
fn missing_appearance_bounds_are_explicitly_refused() {
    let (doc, _) = document(vec![dictionary! {"Font"=>dictionary! {"F1"=>font("Helvetica")}}]);
    let native = Document::new();
    let mut staging = Document::new();
    let mut budget = FillAppearanceBudget::new(&doc, &native, &mut staging).unwrap();
    let mut contexts = Contexts::new(&doc, 1, &mut budget).unwrap();
    let mut stream = appearance(None);
    stream.dict.remove(b"BBox");
    let error = contexts
        .bind(&stream, &[(0, 0)], AnswerKind::Text, &mut budget)
        .unwrap_err();
    assert!(error.to_string().contains("missing source bounds"), "{error}");
}

#[test]
fn local_categories_win_without_merging_names_and_missing_defaults_survive() {
    let page = dictionary! {"Font"=>dictionary! {"F1"=>font("Courier"), "PageOnly"=>font("Helvetica")},
    "ColorSpace"=>dictionary! {"DefaultRGB"=>Object::Name(b"DeviceRGB".to_vec())}};
    let (doc, _) = document(vec![page]);
    let local = dictionary! {"Font"=>dictionary! {"F1"=>font("Times-Roman")}};
    let native = Document::new();
    let mut staging = Document::new();
    let mut budget = FillAppearanceBudget::new(&doc, &native, &mut staging).unwrap();
    let mut contexts = Contexts::new(&doc, 1, &mut budget).unwrap();
    let bound = contexts
        .bind(
            &appearance(Some(local.clone())),
            &[(0, 0)],
            AnswerKind::Text,
            &mut budget,
        )
        .unwrap();
    assert_eq!(bound.get(b"Font").unwrap(), local.get(b"Font").unwrap());
    assert!(!bound
        .get(b"Font")
        .unwrap()
        .as_dict()
        .unwrap()
        .has(b"PageOnly"));
    assert!(bound
        .get(b"ColorSpace")
        .unwrap()
        .as_dict()
        .unwrap()
        .has(b"DefaultRGB"));
}

#[test]
fn page_tree_resources_are_cached_and_different_widgets_keep_their_contexts() {
    let (mut doc, pages) = document(vec![
        dictionary! {},
        dictionary! {"Font"=>dictionary! {"F1"=>font("Courier")}},
    ]);
    let parent = doc
        .get_dictionary(pages[0])
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    doc.get_dictionary_mut(pages[0])
        .unwrap()
        .remove(b"Resources");
    doc.get_dictionary_mut(parent).unwrap().set(
        "Resources",
        dictionary! {"Font"=>dictionary! {"F1"=>font("Helvetica")}},
    );
    let native = Document::new();
    let mut staging = Document::new();
    let mut budget = FillAppearanceBudget::new(&doc, &native, &mut staging).unwrap();
    let mut contexts = Contexts::new(&doc, 2, &mut budget).unwrap();
    let stream = appearance(None);
    let a = contexts
        .bind(&stream, &[(0, 0)], AnswerKind::Text, &mut budget)
        .unwrap();
    let b = contexts
        .bind(&stream, &[(1, 0)], AnswerKind::Text, &mut budget)
        .unwrap();
    assert_ne!(a, b);
    assert_eq!(
        contexts
            .bind(&stream, &[(0, 1)], AnswerKind::Text, &mut budget)
            .unwrap(),
        a
    );
    assert_eq!(contexts.resources.len(), 2);
    assert!(
        contexts
            .bind(&stream, &[(0, 0), (1, 0)], AnswerKind::Text, &mut budget)
            .is_err(),
        "an aliased widget must not silently adopt its first placement"
    );
}

#[test]
fn local_font_hides_different_page_fonts_for_an_aliased_widget() {
    let (doc, _) = document(vec![
        dictionary! {"Font"=>dictionary! {"F1"=>font("Helvetica")}},
        dictionary! {"Font"=>dictionary! {"F1"=>font("Courier")}},
    ]);
    let native = Document::new();
    let mut staging = Document::new();
    let mut budget = FillAppearanceBudget::new(&doc, &native, &mut staging).unwrap();
    let mut contexts = Contexts::new(&doc, 2, &mut budget).unwrap();
    let local = dictionary! {"Font"=>dictionary! {"F1"=>font("Times-Roman")}};
    assert_eq!(
        contexts
            .bind(
                &appearance(Some(local.clone())),
                &[(0, 0), (1, 0)],
                AnswerKind::Text,
                &mut budget
            )
            .unwrap(),
        local
    );
}

#[test]
fn copied_direct_data_and_indirect_comparison_are_bounded() {
    for indirect in [false, true] {
        let (mut doc, pages) = document(vec![dictionary! {}]);
        let large = dictionary! {"Payload"=>Object::string_literal(vec![b'x';4096])};
        let value = if indirect {
            Object::Reference(doc.add_object(large))
        } else {
            Object::Dictionary(large)
        };
        doc.get_dictionary_mut(pages[0])
            .unwrap()
            .set("Resources", dictionary! {"Font"=>value});
        let native = Document::new();
        let mut staging = Document::new();
        let mut budget =
            FillAppearanceBudget::with_limit(&doc, &native, &mut staging, 2048).unwrap();
        let mut contexts = Contexts::new(&doc, 1, &mut budget).unwrap();
        assert!(contexts
            .bind(
                &appearance(None),
                &[(0, 0), (0, 1)],
                AnswerKind::Text,
                &mut budget
            )
            .is_err());
        drop(budget);
        assert!(staging.objects.is_empty());
    }
}

#[test]
fn malformed_categories_and_page_or_resource_cycles_are_rejected() {
    for kind in ["category", "page-cycle", "resource-cycle"] {
        let (mut doc, pages) = document(vec![dictionary! {}]);
        match kind {
            "category" => doc
                .get_dictionary_mut(pages[0])
                .unwrap()
                .set("Resources", dictionary! {"Font"=>42}),
            "page-cycle" => {
                doc.get_dictionary_mut(pages[0])
                    .unwrap()
                    .remove(b"Resources");
                doc.get_dictionary_mut(pages[0])
                    .unwrap()
                    .set("Parent", pages[0]);
            }
            _ => {
                let id = doc.new_object_id();
                doc.objects.insert(id, Object::Reference(id));
                doc.get_dictionary_mut(pages[0])
                    .unwrap()
                    .set("Resources", id);
            }
        }
        let native = Document::new();
        let mut staging = Document::new();
        let mut budget = FillAppearanceBudget::new(&doc, &native, &mut staging).unwrap();
        let mut contexts = Contexts::new(&doc, 1, &mut budget).unwrap();
        assert!(
            contexts
                .bind(&appearance(None), &[(0, 0)], AnswerKind::Text, &mut budget)
                .is_err(),
            "{kind}"
        );
    }
}

#[test]
fn unused_page_resources_do_not_reject_a_plain_aliased_appearance() {
    let (doc, _) = document(vec![
        dictionary! {"Font"=>dictionary! {"F1"=>font("Helvetica")}, "XObject"=>dictionary! {"Unused"=>11}},
        dictionary! {"Font"=>dictionary! {"F1"=>font("Courier")}, "XObject"=>dictionary! {"Unused"=>22}},
    ]);
    let native = Document::new();
    let mut staging = Document::new();
    let mut budget = FillAppearanceBudget::new(&doc, &native, &mut staging).unwrap();
    let mut contexts = Contexts::new(&doc, 2, &mut budget).unwrap();
    let mut stream = appearance(None);
    stream
        .dict
        .set("BBox", vec![0.into(), 0.into(), 100.into(), 40.into()]);
    stream.set_plain_content(
        b"q 1 0 0 rg 0 0 3 3 re f Q /Tx BMC q BT /F1 12 Tf (Old) Tj ET Q EMC".to_vec(),
    );
    let bound = contexts
        .bind(&stream, &[(0, 0), (1, 0)], AnswerKind::Text, &mut budget)
        .unwrap();
    assert!(
        !bound.has(b"Font") && !bound.has(b"XObject"),
        "unused conflicting categories must remain unlocalized"
    );
    stream.set_plain_content(Vec::new());
    let empty_bound = contexts
        .bind(&stream, &[(0, 0), (1, 0)], AnswerKind::Text, &mut budget)
        .unwrap();
    assert!(
        !empty_bound.has(b"Font") && !empty_bound.has(b"XObject"),
        "an empty normal program must not acquire unrelated page resources"
    );
}

#[test]
fn plain_paint_still_requires_consistent_implicit_color_context() {
    let (doc, _) = document(vec![
        dictionary! {"ColorSpace"=>dictionary! {"DefaultRGB"=>Object::Name(b"DeviceRGB".to_vec())}},
        dictionary! {"ColorSpace"=>dictionary! {"DefaultRGB"=>Object::Name(b"DeviceCMYK".to_vec())}},
    ]);
    let native = Document::new();
    let mut staging = Document::new();
    let mut budget = FillAppearanceBudget::new(&doc, &native, &mut staging).unwrap();
    let mut contexts = Contexts::new(&doc, 2, &mut budget).unwrap();
    let mut stream = appearance(None);
    stream.set_plain_content(
        b"q 1 0 0 rg 0 0 3 3 re f Q /Tx BMC q BT /F1 12 Tf (Old) Tj ET Q EMC".to_vec(),
    );
    assert!(
        contexts
            .bind(&stream, &[(0, 0), (1, 0)], AnswerKind::Text, &mut budget)
            .is_err(),
        "plain paint implicitly consumes the page's DefaultRGB binding"
    );
}
