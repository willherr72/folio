use crate::{
    inspect_choice_pdf, prepare_choice_edit_pdf, ChoiceDraftOption, ChoiceEdit,
    ChoiceEditSelection as Edit, ChoiceSelection, PdfEngine, SetFormChoicesRequest,
};
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};
use std::path::PathBuf;
struct Fixture {
    doc: Document,
    parent: ObjectId,
    owner: ObjectId,
    other: ObjectId,
    options: ObjectId,
    value: ObjectId,
}
fn text(s: &str) -> Object {
    lopdf::text_string(s)
}
fn fixture() -> Fixture {
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let parent = doc.new_object_id();
    let owner = doc.new_object_id();
    let other = doc.new_object_id();
    let font =
        doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let resources = dictionary! {"Font"=>dictionary!{"F1"=>font}};
    let ap=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),160.into(),100.into()],"Resources"=>resources.clone()},
        b"q 1 0 0 rg 0 0 10 10 re f Q /Tx BMC q BT /F1 12 Tf 0 g 5 40 Td (Beta) Tj ET Q EMC".to_vec()));
    let mut page_ids = vec![];
    let mut widgets = vec![];
    for i in 0..2 {
        let page = doc.new_object_id();
        let widget = doc.add_object(
            dictionary! {"Type"=>"Annot","Subtype"=>"Widget","Parent"=>owner,"P"=>page,"F"=>4,
            "Rect"=>vec![20.into(),30.into(),180.into(),130.into()],"AP"=>dictionary!{"N"=>ap}},
        );
        let mut annots = vec![widget.into()];
        if i == 0 {
            annots.push(other.into());
        }
        doc.objects.insert(page,dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),200.into()],"Resources"=>Dictionary::new(),"Annots"=>annots}.into());
        page_ids.push(page.into());
        widgets.push(widget.into());
        if i == 0 {
            doc.objects.insert(other,dictionary!{"Type"=>"Annot","Subtype"=>"Widget","T"=>text("other"),"Parent"=>parent,"P"=>page,"F"=>4,
            "Rect"=>vec![210.into(),30.into(),370.into(),130.into()],"AP"=>dictionary!{"N"=>ap}}.into());
        }
    }
    doc.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Kids"=>page_ids,"Count"=>2}.into(),
    );
    let options = doc.add_object(vec![
        Object::Array(vec![text("a"), text("Alpha")]),
        Object::Array(vec![text("b"), text("Beta")]),
        Object::Array(vec![text("c"), text("Gamma")]),
    ]);
    let value = doc.add_object(text("b"));
    let default = doc.add_object(text("a"));
    doc.objects.insert(parent,dictionary!{"T"=>text("Group"),"FT"=>"Ch","Ff"=>2,"Opt"=>options,"DV"=>default,"TI"=>1,"Kids"=>vec![owner.into(),other.into()]}.into());
    doc.objects.insert(
        owner,
        dictionary! {"T"=>text("selected"),"Parent"=>parent,"V"=>value,"Kids"=>widgets}.into(),
    );
    doc.get_dictionary_mut(other).unwrap().set("V", value);
    let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages,"AcroForm"=>dictionary!{"Fields"=>vec![parent.into()],"DR"=>resources,"DA"=>text("/F1 12 Tf 0 g"),"NeedAppearances"=>false}});
    doc.trailer.set("Root", catalog);
    Fixture {
        doc,
        parent,
        owner,
        other,
        options,
        value,
    }
}
fn bytes(f: &mut Fixture) -> Vec<u8> {
    let mut b = vec![];
    f.doc.save_to(&mut b).unwrap();
    b
}
fn request(raw: &[u8]) -> SetFormChoicesRequest {
    let info = inspect_choice_pdf(raw, 0, 0).unwrap();
    SetFormChoicesRequest {
        page_index: 0,
        annotation_index: 0,
        expected_key: info.expected_key,
        edit: ChoiceEdit {
            options: info
                .model
                .options
                .iter()
                .enumerate()
                .map(|(i, o)| ChoiceDraftOption {
                    source_index: Some(i),
                    export_value: o.export_value.clone(),
                    display: o.display.clone(),
                })
                .collect(),
            selection: Edit::Keep,
            defaults: Edit::Keep,
            manual_order: false,
        },
    }
}
fn only_owner_changed(before: &Document, after: &Document, owner: ObjectId) {
    assert_eq!(before.objects.len(), after.objects.len());
    for (id, object) in &before.objects {
        if *id != owner {
            assert_eq!(
                Some(object),
                after.objects.get(id),
                "unrelated object{id:?}"
            );
        }
    }
}
#[test]
fn no_op_retains_every_object_and_has_no_publication_or_repaint_claim() {
    let mut f = fixture();
    let raw = bytes(&mut f);
    let before = Document::load_mem(&raw).unwrap();
    let p = prepare_choice_edit_pdf(&raw, &request(&raw)).unwrap();
    assert!(!p.has_changes());
    assert!(!p.needs_appearance_update());
    assert_eq!(before.objects, p.ctx.doc.objects);
}
#[test]
fn reorder_changes_only_the_owner_and_keeps_shared_raw_values() {
    let mut f = fixture();
    let raw = bytes(&mut f);
    let before = Document::load_mem(&raw).unwrap();
    let mut r = request(&raw);
    r.edit.options.swap(0, 1);
    let p = prepare_choice_edit_pdf(&raw, &r).unwrap();
    assert_eq!(p.resolved().selection, ChoiceSelection::Rows(vec![0]));
    let owner = p.ctx.doc.get_dictionary(f.owner).unwrap();
    assert_eq!(owner.get(b"V").unwrap(), &Object::Reference(f.value));
    assert_eq!(owner.get(b"I").unwrap(), &Object::Array(vec![0.into()]));
    assert_eq!(owner.get(b"TI").unwrap(), &Object::Integer(0));
    assert!(!owner.has(b"DV"));
    assert!(p.needs_appearance_update());
    only_owner_changed(&before, &p.ctx.doc, f.owner);
    assert_eq!(
        p.ctx
            .doc
            .get_dictionary(f.parent)
            .unwrap()
            .get(b"Opt")
            .unwrap(),
        &Object::Reference(f.options)
    );
    assert_eq!(
        p.ctx
            .doc
            .get_dictionary(f.other)
            .unwrap()
            .get(b"V")
            .unwrap(),
        &Object::Reference(f.value)
    );
}
#[test]
fn default_only_and_clears_shadow_inherited_values_without_repainting() {
    let mut f = fixture();
    let raw = bytes(&mut f);
    let before = Document::load_mem(&raw).unwrap();
    let mut r = request(&raw);
    r.edit.defaults = Edit::Rows(vec![2]);
    let p = prepare_choice_edit_pdf(&raw, &r).unwrap();
    assert!(p.has_changes());
    assert!(!p.needs_appearance_update());
    assert_eq!(p.resolved().defaults, Some(vec!["c".into()]));
    only_owner_changed(&before, &p.ctx.doc, f.owner);
    r.edit.defaults = Edit::Clear;
    let p = prepare_choice_edit_pdf(&raw, &r).unwrap();
    assert_eq!(
        p.ctx
            .doc
            .get_dictionary(f.owner)
            .unwrap()
            .get(b"DV")
            .unwrap(),
        &Object::Null
    );
}
#[test]
fn new_duplicate_export_materializes_indices_without_changing_saved_answer() {
    let mut f = fixture();
    let raw = bytes(&mut f);
    let mut r = request(&raw);
    r.edit.options[2].export_value = "b".into();
    let p = prepare_choice_edit_pdf(&raw, &r).unwrap();
    let owner = p.ctx.doc.get_dictionary(f.owner).unwrap();
    assert_eq!(owner.get(b"V").unwrap(), &Object::Reference(f.value));
    assert_eq!(owner.get(b"I").unwrap(), &Object::Array(vec![1.into()]));
    assert_eq!(p.resolved().selection, ChoiceSelection::Rows(vec![1]));
    assert!(!p.needs_appearance_update());
}
#[test]
fn selected_export_change_keeps_visible_label_and_updates_current_value() {
    let mut f = fixture();
    let raw = bytes(&mut f);
    let mut r = request(&raw);
    r.edit.options[1].export_value = "new-b".into();
    let p = prepare_choice_edit_pdf(&raw, &r).unwrap();
    assert_eq!(p.resolved().options[1].export_value, "new-b");
    assert!(!p.needs_appearance_update());
    assert_eq!(
        lopdf::decode_text_string(
            p.ctx
                .doc
                .get_dictionary(f.owner)
                .unwrap()
                .get(b"V")
                .unwrap()
        )
        .unwrap(),
        "new-b"
    );
    assert_eq!(p.ctx.doc.get_object(f.value).unwrap(), &text("b"));
}
#[test]
fn blank_option_does_not_adopt_an_old_blank_answer_or_default() {
    let mut f = fixture();
    let owner = f.doc.get_dictionary_mut(f.owner).unwrap();
    owner.set("V", text(""));
    owner.set("DV", text(""));
    let raw = bytes(&mut f);
    let mut r = request(&raw);
    r.edit.options.push(ChoiceDraftOption {
        source_index: None,
        export_value: "".into(),
        display: "Placeholder".into(),
    });
    let p = prepare_choice_edit_pdf(&raw, &r).unwrap();
    let owner = p.ctx.doc.get_dictionary(f.owner).unwrap();
    assert_eq!(owner.get(b"V").unwrap(), &Object::Null);
    assert_eq!(owner.get(b"DV").unwrap(), &Object::Null);
    assert_eq!(p.resolved().selection, ChoiceSelection::None);
    assert_eq!(p.resolved().defaults, None);
}
#[test]
fn dropdown_classifies_visible_changes_and_retains_ignored_settings() {
    let mut f = fixture();
    f.doc
        .get_dictionary_mut(f.owner)
        .unwrap()
        .set("Ff", (1 << 17) | (1 << 21) | 2);
    let raw = bytes(&mut f);
    let mut r = request(&raw);
    r.edit.options[0].display = "New Alpha".into();
    let p = prepare_choice_edit_pdf(&raw, &r).unwrap();
    assert!(!p.needs_appearance_update());
    assert!(!p.ctx.doc.get_dictionary(f.owner).unwrap().has(b"TI"));
    assert_eq!(
        p.ctx
            .doc
            .get_dictionary(f.owner)
            .unwrap()
            .get(b"Ff")
            .unwrap(),
        &Object::Integer((1 << 17) | (1 << 21) | 2)
    );
    r.edit.options[1].display = "New Beta".into();
    assert!(prepare_choice_edit_pdf(&raw, &r)
        .unwrap()
        .needs_appearance_update());
}
#[test]
fn raw_numeric_indices_and_value_order_survive_unrelated_default_changes() {
    let mut f = fixture();
    f.doc.get_dictionary_mut(f.owner).unwrap().set("I", 1);
    let raw = bytes(&mut f);
    let mut r = request(&raw);
    r.edit.defaults = Edit::Clear;
    let p = prepare_choice_edit_pdf(&raw, &r).unwrap();
    assert_eq!(
        p.ctx
            .doc
            .get_dictionary(f.owner)
            .unwrap()
            .get(b"I")
            .unwrap(),
        &Object::Integer(1)
    );
    let owner = f.doc.get_dictionary_mut(f.owner).unwrap();
    owner.set("Ff", 1 << 21);
    owner.set("V", vec![text("c"), text("a")]);
    owner.set("I", vec![0.into(), 2.into()]);
    let raw = bytes(&mut f);
    let mut r = request(&raw);
    r.edit.options.swap(0, 2);
    let p = prepare_choice_edit_pdf(&raw, &r).unwrap();
    assert_eq!(
        p.ctx
            .doc
            .get_dictionary(f.owner)
            .unwrap()
            .get(b"V")
            .unwrap(),
        &Object::Array(vec![text("c"), text("a")])
    );
}
#[test]
fn stale_requests_and_removed_answers_leave_the_source_usable() {
    let mut f = fixture();
    let raw = bytes(&mut f);
    let mut r = request(&raw);
    r.expected_key.push('x');
    assert!(prepare_choice_edit_pdf(&raw, &r).is_err());
    let mut r = request(&raw);
    r.edit.options.remove(1);
    assert!(prepare_choice_edit_pdf(&raw, &r).is_err());
    assert_eq!(
        inspect_choice_pdf(&raw, 0, 0).unwrap().model.selection,
        ChoiceSelection::Rows(vec![1])
    );
}
#[test]
fn private_candidate_values_agree_with_native_fields_and_other_answers_remain() {
    let e = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    for case in 0..3 {
        let mut f = fixture();
        let raw = bytes(&mut f);
        let mut r = request(&raw);
        if case == 0 {
            r.edit.options.swap(0, 1)
        } else if case == 1 {
            r.edit.options[1].export_value = "new-b".into()
        } else {
            r.edit.selection = Edit::Clear;
            r.edit.defaults = Edit::Clear;
        }
        let mut p = prepare_choice_edit_pdf(&raw, &r).unwrap();
        // Test-only serialization checks logical values, not unfinished AP pixels.
        let data = crate::pdf_output::save(&mut p.ctx.doc).unwrap();
        let path = dir.path().join(format!("private-{case}.pdf"));
        std::fs::write(&path, &data).unwrap();
        let source = e.open_document(&path).unwrap();
        let expected = match &p.resolved().selection {
            ChoiceSelection::Rows(v) => v.clone(),
            ChoiceSelection::None => vec![],
            _ => unreachable!(),
        };
        let first = e.list_form_fields(&source.id, 0).unwrap();
        let second = e.list_form_fields(&source.id, 1).unwrap();
        assert_eq!(first[0].selected_indices, expected);
        assert_eq!(second[0].selected_indices, expected);
        assert_eq!(first[1].selected_indices, vec![1]);
        assert_eq!(first[1].options, vec!["Alpha", "Beta", "Gamma"]);
        e.close_document(&source.id).unwrap();
    }
}
