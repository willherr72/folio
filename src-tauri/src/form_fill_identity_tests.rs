use super::*;
use crate::FormFieldValue;
use lopdf::dictionary;

pub(crate) struct Fixture {
    pub(crate) bytes: Vec<u8>,
    pub(crate) owner: ObjectId,
    widgets: Vec<ObjectId>,
    other: ObjectId,
}
pub(crate) fn fixture() -> Fixture {
    let mut d = Document::with_version("1.7");
    let pages = d.new_object_id();
    let prefix = d.new_object_id();
    let owner = d.new_object_id();
    let mut ids = vec![];
    let mut widgets = vec![];
    let other=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>Object::string_literal("other"),"V"=>Object::string_literal("unchanged"),"Rect"=>vec![5.into(),5.into(),50.into(),20.into()]});
    let ap=d.add_object(lopdf::Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),50.into(),20.into()]},b"q Q".to_vec()));
    for index in 0..2 {
        let page = d.new_object_id();
        ids.push(Object::Reference(page));
        let widget=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","Parent"=>owner,"P"=>page,"Rect"=>vec![10.into(),30.into(),60.into(),50.into()],"AP"=>dictionary!{"N"=>ap}});
        widgets.push(widget);
        let mut annots = vec![Object::Reference(widget)];
        if index == 0 {
            annots.push(Object::Reference(other));
        }
        d.objects.insert(page,dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()],"Annots"=>annots}.into());
    }
    d.objects.insert(owner,dictionary!{"Parent"=>prefix,"T"=>Object::string_literal("answer"),"FT"=>"Tx","V"=>Object::string_literal("old"),"DV"=>Object::string_literal("default"),"Kids"=>widgets.iter().copied().map(Object::Reference).collect::<Vec<_>>()}.into());
    d.objects.insert(
        prefix,
        dictionary! {"T"=>Object::string_literal("group"),"Kids"=>vec![Object::Reference(owner)]}
            .into(),
    );
    d.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Count"=>2,"Kids"=>ids}.into(),
    );
    let font =
        d.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let root=d.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages,"AcroForm"=>dictionary!{"Fields"=>vec![Object::Reference(prefix),Object::Reference(other)],"DA"=>Object::string_literal("/F1 12 Tf 0 g"),"DR"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}}});
    d.trailer.set("Root", root);
    let mut bytes = vec![];
    d.save_to(&mut bytes).unwrap();
    Fixture {
        bytes,
        owner,
        widgets,
        other,
    }
}
pub(crate) fn request() -> FillFormFieldRequest {
    FillFormFieldRequest {
        page_index: 0,
        annotation_index: 0,
        expected_value: FormFieldValue::Text { text: "old".into() },
        value: FormFieldValue::Text { text: "new".into() },
    }
}
fn probe(f: &Fixture) -> (FillIdentity, Document) {
    let mut identity = FillIdentity::prepare(&f.bytes, &request()).unwrap();
    let bytes = identity.probe_bytes().unwrap();
    let mut native = Document::load_mem(&bytes).unwrap();
    native
        .get_dictionary_mut(f.owner)
        .unwrap()
        .set("V", Object::string_literal("new"));
    (identity, native)
}
#[test]
fn linked_widgets_bind_after_native_renumbering_without_changing_source() {
    let f = fixture();
    let (identity, mut native) = probe(&f);
    native.renumber_objects_with(200);
    let bound = identity.bind_native(&native).unwrap();
    let linked: Vec<_> = bound
        .nodes
        .iter()
        .filter(|n| n.selected && n.widget)
        .collect();
    assert_eq!(linked.len(), 2);
    assert!(linked.iter().all(|n| n.native.0 >= 200));
    assert_eq!(linked.iter().map(|n| n.placements.len()).sum::<usize>(), 2);
    assert_eq!(bound.changes.len(), 1);
    let change = &bound.changes[0];
    assert_eq!(change.original, f.owner);
    assert_eq!(change.key, b"V");
    assert_eq!(change.value.as_ref().unwrap().as_str().unwrap(), b"new");
    assert_eq!(
        identity
            .original
            .get_dictionary(f.owner)
            .unwrap()
            .get(b"V")
            .unwrap()
            .as_str()
            .unwrap(),
        b"old"
    );
    assert!(identity
        .original
        .objects
        .values()
        .filter_map(|v| v.as_dict().ok())
        .all(|d| !d.has(&identity.marker)));
}
#[test]
fn merged_field_widget_is_bound_without_relying_on_its_name() {
    let f = fixture();
    let mut request = request();
    request.annotation_index = 1;
    request.expected_value = FormFieldValue::Text {
        text: "unchanged".into(),
    };
    let mut identity = FillIdentity::prepare(&f.bytes, &request).unwrap();
    let mut native = Document::load_mem(&identity.probe_bytes().unwrap()).unwrap();
    native
        .get_dictionary_mut(f.other)
        .unwrap()
        .set("V", Object::string_literal("new"));
    let bound = identity.bind_native(&native).unwrap();
    assert_eq!(
        bound
            .nodes
            .iter()
            .filter(|v| v.selected && v.widget)
            .count(),
        1
    );
    assert_eq!(bound.changes[0].original, f.other);
}
#[test]
fn missing_repeated_and_detached_markers_cannot_authorize_an_answer() {
    for mode in 0..3 {
        let f = fixture();
        let (identity, mut native) = probe(&f);
        let token = native
            .get_dictionary(f.widgets[0])
            .unwrap()
            .get(&identity.marker)
            .unwrap()
            .clone();
        if mode == 0 {
            native
                .get_dictionary_mut(f.widgets[0])
                .unwrap()
                .remove(&identity.marker);
        } else if mode == 1 {
            native
                .get_dictionary_mut(f.widgets[1])
                .unwrap()
                .set(identity.marker.clone(), token);
        } else {
            let moved = native.get_dictionary(f.widgets[0]).unwrap().clone();
            native.add_object(moved);
            native
                .get_dictionary_mut(f.widgets[0])
                .unwrap()
                .remove(&identity.marker);
        }
        assert!(
            identity.bind_native(&native).is_err(),
            "bad marker mode {mode} accepted"
        );
    }
}
#[test]
fn role_parent_and_page_correspondence_are_required() {
    for mode in 0..3 {
        let f = fixture();
        let (identity, mut native) = probe(&f);
        if mode == 0 {
            native
                .get_dictionary_mut(f.widgets[0])
                .unwrap()
                .set("Subtype", "Text");
        } else if mode == 1 {
            native
                .get_dictionary_mut(f.widgets[0])
                .unwrap()
                .set("Parent", f.other);
        } else {
            let first = *native.get_pages().get(&1).unwrap();
            native.get_dictionary_mut(first).unwrap().set(
                "Annots",
                vec![Object::Reference(f.widgets[1]), Object::Reference(f.other)],
            );
        }
        assert!(
            identity.bind_native(&native).is_err(),
            "bad topology mode {mode} accepted"
        );
    }
}
#[test]
fn native_unrelated_answer_and_default_changes_are_refused() {
    for key in [b"V".as_slice(), b"DV"] {
        let f = fixture();
        let (identity, mut native) = probe(&f);
        let id = if key == b"V" { f.other } else { f.owner };
        native
            .get_dictionary_mut(id)
            .unwrap()
            .set(key.to_vec(), Object::string_literal("unexpected"));
        assert!(identity.bind_native(&native).is_err());
    }
}
#[test]
fn reference_cycles_and_excessive_answer_data_are_refused_before_staging() {
    for excessive in [false, true] {
        let f = fixture();
        let (identity, mut native) = probe(&f);
        let value = if excessive {
            Object::string_literal(vec![b'x'; 4 * 1024 * 1024])
        } else {
            let id = native.new_object_id();
            native.objects.insert(id, Object::Reference(id));
            Object::Reference(id)
        };
        native.get_dictionary_mut(f.owner).unwrap().set("V", value);
        assert!(identity.bind_native(&native).is_err());
        assert_eq!(
            identity
                .original
                .get_dictionary(f.owner)
                .unwrap()
                .get(b"V")
                .unwrap()
                .as_str()
                .unwrap(),
            b"old"
        );
    }
}

#[test]
fn declared_page_change_cannot_rebind_a_widget_appearance() {
    let f = fixture();
    let (identity, mut native) = probe(&f);
    let second = *native.get_pages().get(&2).unwrap();
    native
        .get_dictionary_mut(f.widgets[0])
        .unwrap()
        .set("P", second);
    assert!(
        identity.bind_native(&native).is_err(),
        "changed native page ownership was accepted"
    );
}

#[test]
fn failed_probe_serialization_removes_all_private_markers() {
    struct Refuse;
    impl std::io::Write for Refuse {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("test writer is full"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let f = fixture();
    let mut identity = FillIdentity::prepare(&f.bytes, &request()).unwrap();
    assert!(identity.write_probe(&mut Refuse).is_err());
    assert!(markers(&identity.original, &identity.marker)
        .unwrap()
        .is_empty());
    let native = Document::load_mem(&identity.probe_bytes().unwrap()).unwrap();
    assert!(identity.bind_native(&native).unwrap().changes.is_empty());
}

#[test]
fn repeated_placements_and_aliased_appearances_remain_bound() {
    let f = fixture();
    let mut source = Document::load_mem(&f.bytes).unwrap();
    let second = *source.get_pages().get(&2).unwrap();
    source
        .get_dictionary_mut(second)
        .unwrap()
        .set("Annots", vec![Object::Reference(f.widgets[0])]);
    let mut bytes = vec![];
    source.save_to(&mut bytes).unwrap();
    let mut identity = FillIdentity::prepare(&bytes, &request()).unwrap();
    let native = Document::load_mem(&identity.probe_bytes().unwrap()).unwrap();
    let bound = identity.bind_native(&native).unwrap();
    let repeated = bound
        .nodes
        .iter()
        .find(|v| v.original == f.widgets[0])
        .unwrap();
    assert_eq!(repeated.placements, vec![(0, 0), (1, 0)]);
    let ap = |id| {
        identity
            .original
            .get_dictionary(id)
            .unwrap()
            .get(b"AP")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"N")
            .unwrap()
            .as_reference()
            .unwrap()
    };
    assert_eq!(ap(f.widgets[0]), ap(f.widgets[1]));
}

#[test]
fn bound_answer_values_do_not_retain_native_object_references() {
    let f = fixture();
    let (identity, mut native) = probe(&f);
    let answer = native.add_object(Object::string_literal("new"));
    native.get_dictionary_mut(f.owner).unwrap().set("V", answer);
    let bound = identity.bind_native(&native).unwrap();
    assert!(matches!(bound.changes[0].value, Some(Object::String(..))));
    assert_eq!(
        bound.changes[0].value.as_ref().unwrap().as_str().unwrap(),
        b"new"
    );
}

#[test]
fn graph_and_placement_limits_reject_excess_before_probe_creation() {
    for placements in [false, true] {
        let f = fixture();
        let mut source = Document::load_mem(&f.bytes).unwrap();
        if placements {
            let page = *source.get_pages().get(&1).unwrap();
            source.get_dictionary_mut(page).unwrap().set(
                "Annots",
                vec![Object::Reference(f.widgets[0]); MAX_NODES + 1],
            );
        } else {
            let mut roots = Vec::new();
            for i in 0..=MAX_NODES {
                roots.push(Object::Reference(source.add_object(
                    dictionary! {"FT"=>"Tx", "T"=>Object::string_literal(format!("field{i}"))},
                )));
            }
            source
                .catalog_mut()
                .unwrap()
                .set("AcroForm", dictionary! {"Fields"=>roots});
        }
        let mut bytes = Vec::new();
        source.save_to(&mut bytes).unwrap();
        assert!(FillIdentity::prepare(&bytes, &request()).is_err());
        assert_eq!(
            source
                .get_dictionary(f.owner)
                .unwrap()
                .get(b"V")
                .unwrap()
                .as_str()
                .unwrap(),
            b"old"
        );
    }
}
