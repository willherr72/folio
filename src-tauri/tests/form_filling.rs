use folio_engine::{FillFormFieldRequest, FormFieldValue, PdfEngine};
use lopdf::{dictionary, Dictionary, Document, Object, Stream};
use std::{fs, path::PathBuf};
#[path="support/protected_fixture.rs"]
mod protected_fixture;
fn fixture() -> Vec<u8> {
    let mut d = Document::with_version("1.7");
    let pages = d.new_object_id();
    let font =
        d.add_object(dictionary! {"Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"Helvetica"});
    let ap = d.add_object(Stream::new(dictionary! {"Type"=>"XObject", "Subtype"=>"Form", "BBox"=>vec![0.into(),0.into(),180.into(),30.into()], "Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}}, b"0 0 180 30 re S BT /F1 12 Tf 5 10 Td (Original name) Tj ET".to_vec()));
    let off = d.add_object(Stream::new(dictionary! {"Type"=>"XObject", "Subtype"=>"Form", "BBox"=>vec![0.into(),0.into(),18.into(),18.into()]}, b"0 0 18 18 re S".to_vec()));
    let on = d.add_object(Stream::new(dictionary! {"Type"=>"XObject", "Subtype"=>"Form", "BBox"=>vec![0.into(),0.into(),18.into(),18.into()]}, b"0 0 18 18 re S 2 2 m 16 16 l S".to_vec()));
    let person = d.new_object_id();
    let name = d.new_object_id();
    let mut page_ids = vec![];
    let mut widgets = vec![];
    let mut fields = vec![Object::Reference(person)];
    for index in 0..3 {
        let page = d.new_object_id();
        let mut annotations = vec![];
        if index < 2 {
            let widget = d.add_object(dictionary! {"Type"=>"Annot", "Subtype"=>"Widget", "Parent"=>name, "P"=>page, "Rect"=>vec![40.into(),200.into(),220.into(),230.into()], "F"=>4, "AP"=>dictionary!{"N"=>ap}});
            widgets.push(Object::Reference(widget));
            annotations.push(Object::Reference(widget));
        }
        if index == 0 {
            let widget = d.add_object(dictionary! {"Type"=>"Annot", "Subtype"=>"Widget", "FT"=>"Btn", "T"=>Object::string_literal("consent"), "V"=>"Accepted", "DV"=>"Off", "AS"=>"Accepted", "P"=>page, "Rect"=>vec![40.into(),150.into(),58.into(),168.into()], "F"=>4, "AP"=>dictionary!{"N"=>dictionary!{"Off"=>off,"Accepted"=>on}}});
            fields.push(Object::Reference(widget));
            annotations.push(Object::Reference(widget));
        }
        if index == 2 {
            let widget = d.add_object(dictionary! {"Type"=>"Annot", "Subtype"=>"Widget", "FT"=>"Ch", "T"=>Object::string_literal("choice"), "Opt"=>vec![Object::string_literal("One"),Object::string_literal("Two")], "V"=>Object::string_literal("Two"), "P"=>page, "Rect"=>vec![40.into(),200.into(),220.into(),230.into()], "F"=>4, "AP"=>dictionary!{"N"=>ap}});
            fields.push(Object::Reference(widget));
            annotations.push(Object::Reference(widget));
        }
        d.objects.insert(page, Object::Dictionary(dictionary! {"Type"=>"Page", "Parent"=>pages, "MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()], "Resources"=>dictionary!{}, "Annots"=>annotations}));
        page_ids.push(Object::Reference(page));
    }
    d.objects.insert(name, Object::Dictionary(dictionary! {"T"=>Object::string_literal("name"), "FT"=>"Tx", "Parent"=>person, "V"=>Object::string_literal("Original name"), "DV"=>Object::string_literal("Default name"), "MaxLen"=>80, "Kids"=>widgets}));
    d.objects.insert(person, Object::Dictionary(dictionary! {"T"=>Object::string_literal("person"), "Kids"=>vec![Object::Reference(name)]}));
    d.objects.insert(
        pages,
        Object::Dictionary(dictionary! {"Type"=>"Pages", "Count"=>3, "Kids"=>page_ids}),
    );
    let form = d.add_object(dictionary! {"Fields"=>fields, "DA"=>Object::string_literal("/F1 12 Tf 0 g"), "DR"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}, "CO"=>vec![Object::Reference(name)]});
    let root = d.add_object(dictionary! {"Type"=>"Catalog", "Pages"=>pages, "AcroForm"=>form});
    d.trailer.set("Root", root);
    let mut bytes = vec![];
    d.save_to(&mut bytes).unwrap();
    bytes
}
fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn open(bytes: &[u8]) -> (tempfile::TempDir, PdfEngine, folio_engine::DocumentInfo) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("form.pdf");
    fs::write(&path, bytes).unwrap();
    let e = engine();
    let info = e.open_document(&path).unwrap();
    (temp, e, info)
}

#[test]
fn widget_geometry_tracks_crop_rotation_visibility_and_preserves_source() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for (rotation,normal,unrotated) in [
        (0,[30.,150.,180.,30.],[30.,150.,180.,30.]),
        (90,[180.,30.,30.,180.],[210.,30.,180.,30.]),
        (180,[70.,180.,180.,30.],[250.,210.,180.,30.]),
        (270,[150.,70.,30.,180.],[150.,250.,180.,30.]),
    ] {
        for no_rotate in [false,true] {
            let mut doc=Document::load_mem(&fixture()).unwrap();
            let page=*doc.get_pages().get(&1).unwrap();
            let widgets=doc.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap().clone();
            let first=widgets[0].as_reference().unwrap();let second=widgets[1].as_reference().unwrap();
            doc.get_dictionary_mut(page).unwrap().set("CropBox",vec![10.into(),20.into(),290.into(),380.into()]);
            doc.get_dictionary_mut(page).unwrap().set("Rotate",rotation);
            doc.get_dictionary_mut(first).unwrap().set("F",if no_rotate {4|16}else{4});
            doc.get_dictionary_mut(second).unwrap().set("F",4|2);
            let mut bytes=vec![];doc.save_to(&mut bytes).unwrap();
            let path=temp.path().join(format!("{rotation}-{no_rotate}.pdf"));fs::write(&path,&bytes).unwrap();
            let source=e.open_document(&path).unwrap();
            let fields=e.list_form_fields(&source.id,0).unwrap();let value=serde_json::to_value(&fields[0]).unwrap();
            let expected=if no_rotate {unrotated}else{normal};
            for (key,wanted) in ["x","y","width","height"].into_iter().zip(expected) {
                assert!((value["bounds"][key].as_f64().unwrap_or(-999.)-wanted).abs()<0.01,"{rotation}/{no_rotate} {value}");
            }
            assert_eq!(value["visible"],true);assert_eq!(value["noRotate"],no_rotate);
            assert_eq!(serde_json::to_value(&fields[1]).unwrap()["visible"],false);
            assert!(fields[1].reason.is_some());assert_eq!(fs::read(&path).unwrap(),bytes);
            e.close_document(&source.id).unwrap();
        }
    }
    let mut doc=Document::load_mem(&fixture()).unwrap();let page=*doc.get_pages().get(&1).unwrap();
    let widgets=doc.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap().clone();
    doc.get_dictionary_mut(widgets[0].as_reference().unwrap()).unwrap().set("F",4|64);
    doc.get_dictionary_mut(widgets[1].as_reference().unwrap()).unwrap().set("Rect",vec![40.into(),150.into(),40.into(),168.into()]);
    let mut bytes=vec![];doc.save_to(&mut bytes).unwrap();let path=temp.path().join("readonly-invalid.pdf");fs::write(&path,&bytes).unwrap();
    let source=e.open_document(&path).unwrap();let fields=e.list_form_fields(&source.id,0).unwrap();
    let first=serde_json::to_value(&fields[0]).unwrap();assert_eq!(first["visible"],true);assert!(fields[0].reason.is_some());assert!(first["bounds"].is_object());
    assert!(serde_json::to_value(&fields[1]).unwrap()["bounds"].is_null());e.close_document(&source.id).unwrap();
}

#[test]
fn protected_form_filling_retains_permissions_and_both_passwords() {
    use folio_engine::{EngineError,ExportRequest,PagePlan};
    use folio_engine::protection::unlock_pdf;
    use lopdf::encryption::Permissions;
    let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("protected-form.pdf");
    let original=protected_fixture::encrypt(&fixture(),Permissions::FILLABLE);
    fs::write(&path,&original).unwrap();
    let source=e.open_document_with_password(&path,"reader").unwrap();
    let field=e.list_form_fields(&source.id,0).unwrap().into_iter().find(|f|f.name=="person.name").unwrap();
    let request=FillFormFieldRequest {page_index:0,annotation_index:field.annotation_index,expected_value:field.value,value:FormFieldValue::Text {text:"Protected form value".into()}};
    let filled=e.fill_form_field(&source.id,request.clone()).unwrap();
    assert_eq!(filled.protection,source.protection);
    assert!(matches!(e.page_text(&filled.id,0),Err(EngineError::PermissionDenied(_))));
    assert!(matches!(e.list_text_runs(&filled.id,0),Err(EngineError::PermissionDenied(_))));
    let plan:Vec<PagePlan>=filled.pages.iter().enumerate().map(|(i,p)|serde_json::from_value(serde_json::json!({"id":format!("p{i}"),"sourceId":filled.id,"pageIndex":i,"width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})).unwrap()).collect();
    let output=temp.path().join("filled.pdf");e.export_pdf(ExportRequest{pages:plan,flatten:false},&output).unwrap();
    protected_fixture::record("filled-form",&original,&fs::read(&output).unwrap());
    for password in ["reader","owner"] {
        let copy=e.open_document_with_password(&output,password).unwrap();
        assert!(e.list_form_fields(&copy.id,0).unwrap().iter().any(|f|f.value==request.value));
        e.close_document(&copy.id).unwrap();
        assert!(unlock_pdf(&fs::read(&output).unwrap(),Some(password)).unwrap().protection.is_some());
    }
    let no_fill=temp.path().join("cannot-fill.pdf");fs::write(&no_fill,protected_fixture::encrypt(&fixture(),Permissions::COPYABLE)).unwrap();
    let blocked=e.open_document_with_password(&no_fill,"reader").unwrap();
    assert!(e.list_form_fields(&blocked.id,0).is_ok());
    assert!(matches!(e.fill_form_field(&blocked.id,request),Err(EngineError::PermissionDenied(_))));
    assert_eq!(fs::read(path).unwrap(),original);
    for id in [&source.id,&filled.id,&blocked.id] {e.close_document(id).unwrap();}
}

#[test]
fn shared_composition_preserves_forms_overlays_bookmarks_and_preview_together() {
    use folio_engine::{DocumentBookmark, ExportRequest, PagePlan};
    let original = fixture();
    let (temp, e, info) = open(&original);
    let field = e.list_form_fields(&info.id, 0).unwrap().remove(0);
    let filled = e
        .fill_form_field(
            &info.id,
            FillFormFieldRequest {
                page_index: 0,
                annotation_index: field.annotation_index,
                expected_value: field.value,
                value: FormFieldValue::Text {
                    text: "Composed value".into(),
                },
            },
        )
        .unwrap();
    let pages: Vec<PagePlan> = filled.pages.iter().enumerate().map(|(index, page)| {
        serde_json::from_value(serde_json::json!({
            "id":format!("stable-{index}"),"sourceId":filled.id,"pageIndex":index,
            "width":page.width,"height":page.height,"rotation":0,
            "overlays":if index==0 {serde_json::json!([
                {"type":"text","id":"note","text":"Composed note","x":20,"y":320,"fontSize":14,"color":"#123456"},
                {"type":"comment","id":"comment","text":"Composed comment","x":250,"y":340,"color":"#ff8000"}
            ])} else {serde_json::json!([])}
        })).unwrap()
    }).collect();
    let bookmarks: Vec<DocumentBookmark> = serde_json::from_value(serde_json::json!([
        {"id":"jump","title":"Second page","level":0,"destination":{"kind":"array","value":[
            {"kind":"page","value":"stable-1"},{"kind":"name","value":[70,105,116]}
        ]}}
    ]))
    .unwrap();
    let before = e.render_page_plan(pages[0].clone(), 600).unwrap();
    let path = temp.path().join("composed.pdf");
    e.export_pdf_with_bookmarks(
        ExportRequest {
            pages,
            flatten: false,
        },
        Some(bookmarks),
        &path,
    )
    .unwrap();
    let reopened = e.open_document(&path).unwrap();
    assert_eq!(reopened.bookmarks.len(), 1);
    assert_eq!(reopened.bookmarks[0].title, "Second page");
    assert_eq!(
        folio_engine::bookmarks::primary_page(&reopened.bookmarks[0]),
        Some("1")
    );
    assert_eq!(reopened.pages[0].overlays.len(), 1);
    assert_eq!(e.list_native_comments(&reopened.id).unwrap()[0].text,"Composed comment");
    for index in 0..3 {
        let values = |id: &str| {
            e.list_form_fields(id, index)
                .unwrap()
                .into_iter()
                .map(|f| (f.name, f.value))
                .collect::<Vec<_>>()
        };
        assert_eq!(values(&filled.id), values(&reopened.id));
    }
    let after: PagePlan = serde_json::from_value(serde_json::json!({
        "id":"again","sourceId":reopened.id,"pageIndex":0,"width":300,"height":400,"rotation":0,
        "overlays":reopened.pages[0].overlays
    }))
    .unwrap();
    let after = e.render_page_plan(after, 600).unwrap();
    let decode = |png: &[u8]| image::load_from_memory(png).unwrap().into_rgba8();
    assert_eq!(decode(&before), decode(&after));
    assert_eq!(fs::read(temp.path().join("form.pdf")).unwrap(), original);
    assert!(fs::read_dir(temp.path()).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".folio-export-")));
    for id in [&info.id, &filled.id, &reopened.id] {
        e.close_document(id).unwrap();
    }
}
#[test]
fn fills_shared_text_and_custom_checkbox_without_mutating_source() {
    let original = fixture();
    let (_temp, e, info) = open(&original);
    let fields = e.list_form_fields(&info.id, 0).unwrap();
    assert_eq!(fields.len(), 2);
    let text = &fields[0];
    assert_eq!(text.name, "person.name");
    assert_eq!(text.kind, "text");
    let value = FormFieldValue::Text {
        text: "Filled café".into(),
    };
    let next = e
        .fill_form_field(
            &info.id,
            FillFormFieldRequest {
                page_index: 0,
                annotation_index: text.annotation_index,
                expected_value: text.value.clone(),
                value: value.clone(),
            },
        )
        .unwrap();
    assert_eq!(next.pages.len(), 3);
    for p in [0, 1] {
        assert_eq!(e.list_form_fields(&next.id, p).unwrap()[0].value, value);
    }
    assert_eq!(
        e.source_bytes(&info.id).unwrap().as_ref(),
        original.as_slice()
    );
    let old = Document::load_mem(&original).unwrap();
    let new = Document::load_mem(&e.source_bytes(&next.id).unwrap()).unwrap();
    let appearances = |d: &Document| {
        d.objects
            .values()
            .filter_map(|o| o.as_stream().ok())
            .map(|s| s.decompressed_content().unwrap_or(s.content.clone()))
            .collect::<Vec<_>>()
    };
    assert_ne!(
        appearances(&old),
        appearances(&new),
        "field appearance must be regenerated"
    );
    let check = e.list_form_fields(&next.id, 0).unwrap().remove(1);
    assert_eq!(check.value, FormFieldValue::Toggle { checked: true });
    let unchecked = e
        .fill_form_field(
            &next.id,
            FillFormFieldRequest {
                page_index: 0,
                annotation_index: check.annotation_index,
                expected_value: check.value,
                value: FormFieldValue::Toggle { checked: false },
            },
        )
        .unwrap();
    let check = e.list_form_fields(&unchecked.id, 0).unwrap().remove(1);
    assert_eq!(check.value, FormFieldValue::Toggle { checked: false });
    let checked = e
        .fill_form_field(
            &unchecked.id,
            FillFormFieldRequest {
                page_index: 0,
                annotation_index: check.annotation_index,
                expected_value: check.value,
                value: FormFieldValue::Toggle { checked: true },
            },
        )
        .unwrap();
    let final_doc = Document::load_mem(&e.source_bytes(&checked.id).unwrap()).unwrap();
    assert!(final_doc
        .objects
        .values()
        .any(|o| o.as_dict().is_ok_and(|d| d
            .get(b"AS")
            .is_ok_and(|v| v.as_name().ok() == Some(b"Accepted")))));
    for id in [&info.id, &next.id, &unchecked.id, &checked.id] {
        e.close_document(id).unwrap();
    }
}
#[test]
fn rejects_stale_readonly_truncated_and_invalid_values() {
    let bytes = fixture();
    let (_temp, e, info) = open(&bytes);
    let text = e.list_form_fields(&info.id, 0).unwrap().remove(0);
    for (expected, value) in [
        (
            FormFieldValue::Text {
                text: "stale".into(),
            },
            FormFieldValue::Text { text: "new".into() },
        ),
        (
            text.value.clone(),
            FormFieldValue::Text {
                text: "x".repeat(81),
            },
        ),
        (
            text.value.clone(),
            FormFieldValue::Text {
                text: "bad\0value".into(),
            },
        ),
        (text.value.clone(), FormFieldValue::Toggle { checked: true }),
    ] {
        assert!(e
            .fill_form_field(
                &info.id,
                FillFormFieldRequest {
                    page_index: 0,
                    annotation_index: text.annotation_index,
                    expected_value: expected,
                    value
                }
            )
            .is_err());
    }
    let mut doc = Document::load_mem(&bytes).unwrap();
    for obj in doc.objects.values_mut() {
        if let Ok(d) = obj.as_dict_mut() {
            if d.get(b"FT").and_then(Object::as_name).ok() == Some(b"Tx") {
                d.set("Ff", 1);
            }
        }
    }
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    let (_temp2, e, locked) = open(&bytes);
    let field = e.list_form_fields(&locked.id, 0).unwrap().remove(0);
    assert!(field.read_only);
    assert!(field.reason.is_some());
    assert!(e
        .fill_form_field(
            &locked.id,
            FillFormFieldRequest {
                page_index: 0,
                annotation_index: field.annotation_index,
                expected_value: field.value,
                value: FormFieldValue::Text { text: "new".into() }
            }
        )
        .is_err());
    e.close_document(&info.id).unwrap();
    e.close_document(&locked.id).unwrap();
}

fn controls() -> Vec<u8> {
    let mut d = Document::load_mem(&fixture()).unwrap();
    let page = *d.get_pages().values().next().unwrap();
    let form = d
        .catalog()
        .unwrap()
        .get(b"AcroForm")
        .unwrap()
        .as_reference()
        .unwrap();
    let mut annots = d
        .get_dictionary(page)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()
        .clone();
    let mut fields = d
        .get_dictionary(form)
        .unwrap()
        .get(b"Fields")
        .unwrap()
        .as_array()
        .unwrap()
        .clone();
    for (name, flags) in [
        ("combo", 1 << 17),
        ("list", 1 << 21),
        ("free", (1 << 17) | (1 << 18)),
        ("multi", 1 << 12),
    ] {
        let field=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>if name=="multi"{"Tx"}else{"Ch"},"Ff"=>flags,"T"=>Object::string_literal(name),"P"=>page,"F"=>4,"Rect"=>vec![80.into(),40.into(),260.into(),100.into()],"Opt"=>vec![Object::Array(vec![Object::string_literal("a"),Object::string_literal("Alpha")]),Object::Array(vec![Object::string_literal("b"),Object::string_literal("Beta")])],"V"=>Object::string_literal(if name=="multi"{"start"}else{"a"}),"I"=>vec![0.into()]});
        annots.push(field.into());
        fields.push(field.into());
    }
    let parent = d.new_object_id();
    let mut kids = vec![];
    for (name, x) in [("Left", 10), ("Right", 40)] {
        let on=d.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),18.into(),18.into()]},b"0 0 18 18 re f".to_vec()));
        let off=d.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),18.into(),18.into()]},vec![]));
        let mut states = Dictionary::new();
        states.set("Off", off);
        states.set(name, on);
        let child=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","Parent"=>parent,"P"=>page,"F"=>4,"Rect"=>vec![x.into(),10.into(),(x+18).into(),28.into()],"AS"=>if name=="Left"{"Left"}else{"Off"},"AP"=>dictionary!{"N"=>states}});
        kids.push(Object::Reference(child));
        annots.push(child.into());
    }
    d.objects.insert(parent,Object::Dictionary(dictionary!{"FT"=>"Btn","Ff"=>1<<15,"T"=>Object::string_literal("side"),"V"=>"Left","Kids"=>kids}));
    fields.push(parent.into());
    d.get_dictionary_mut(form).unwrap().set("Fields", fields);
    d.get_dictionary_mut(page).unwrap().set("Annots", annots);
    let mut bytes = vec![];
    d.save_to(&mut bytes).unwrap();
    bytes
}

#[test]
fn fills_choices_multiline_and_radio_with_saved_native_appearances() {
    let (_temp, e, mut source) = open(&controls());
    for (name, value) in [
        ("combo", FormFieldValue::Choice { indices: vec![1] }),
        (
            "list",
            FormFieldValue::Choice {
                indices: vec![0, 1],
            },
        ),
        (
            "free",
            FormFieldValue::Text {
                text: "Custom choice".into(),
            },
        ),
        (
            "multi",
            FormFieldValue::Text {
                text: "First line\nSecond line".into(),
            },
        ),
    ] {
        let field = e
            .list_form_fields(&source.id, 0)
            .unwrap()
            .into_iter()
            .find(|f| f.name == name)
            .unwrap();
        if name == "combo" {
            assert_eq!(field.options, vec!["Alpha", "Beta"]);
        }
        let next = e
            .fill_form_field(
                &source.id,
                FillFormFieldRequest {
                    page_index: 0,
                    annotation_index: field.annotation_index,
                    expected_value: field.value,
                    value: value.clone(),
                },
            )
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(
            e.list_form_fields(&next.id, 0)
                .unwrap()
                .into_iter()
                .find(|f| f.name == name)
                .unwrap()
                .value,
            value
        );
        e.close_document(&source.id).unwrap();
        source = next;
    }
    let field = e
        .list_form_fields(&source.id, 0)
        .unwrap()
        .into_iter()
        .filter(|f| f.name == "side")
        .last()
        .unwrap();
    assert_eq!(field.options, vec!["Right"]);
    let next = e
        .fill_form_field(
            &source.id,
            FillFormFieldRequest {
                page_index: 0,
                annotation_index: field.annotation_index,
                expected_value: field.value,
                value: FormFieldValue::Toggle { checked: true },
            },
        )
        .unwrap();
    let radios = e
        .list_form_fields(&next.id, 0)
        .unwrap()
        .into_iter()
        .filter(|f| f.name == "side")
        .map(|f| f.value)
        .collect::<Vec<_>>();
    assert_eq!(
        radios,
        vec![
            FormFieldValue::Toggle { checked: false },
            FormFieldValue::Toggle { checked: true }
        ]
    );
    let doc = Document::load_mem(&e.source_bytes(&next.id).unwrap()).unwrap();
    let combo = doc
        .objects
        .values()
        .filter_map(|o| o.as_dict().ok())
        .find(|d| {
            d.get(b"T")
                .is_ok_and(|v| lopdf::decode_text_string(v).ok().as_deref() == Some("combo"))
        })
        .unwrap();
    assert_eq!(
        lopdf::decode_text_string(combo.get(b"V").unwrap()).unwrap(),
        "b",
        "save export value rather than display label"
    );
    assert!(combo.has(b"AP"));
    e.close_document(&source.id).unwrap();
    e.close_document(&next.id).unwrap();
}

#[test]
fn supports_empty_values_and_repeated_edits() {
    let (_temp, e, mut source) = open(&controls());
    for name in ["person.name", "free", "list"] {
        for pass in 0..3 {
            let field = e
                .list_form_fields(&source.id, 0)
                .unwrap()
                .into_iter()
                .find(|f| f.name == name)
                .unwrap();
            let value = if name == "list" {
                FormFieldValue::Choice {
                    indices: if pass == 1 { vec![1] } else { vec![] },
                }
            } else {
                FormFieldValue::Text {
                    text: if pass == 1 {
                        "Refilled".into()
                    } else {
                        String::new()
                    },
                }
            };
            let next = e
                .fill_form_field(
                    &source.id,
                    FillFormFieldRequest {
                        page_index: 0,
                        annotation_index: field.annotation_index,
                        expected_value: field.value,
                        value: value.clone(),
                    },
                )
                .unwrap();
            assert_eq!(
                e.list_form_fields(&next.id, 0)
                    .unwrap()
                    .into_iter()
                    .find(|f| f.name == name)
                    .unwrap()
                    .value,
                value
            );
            e.close_document(&source.id).unwrap();
            source = next;
        }
    }
    e.close_document(&source.id).unwrap();
}

#[test]
fn filled_source_preserves_imported_annotations_through_export_and_recovery() {
    use folio_engine::{ExportRequest, PagePlan, RecoveryStore};
    use serde_json::json;
    let mut doc = Document::load_mem(&fixture()).unwrap();
    let page = *doc.get_pages().values().next().unwrap();
    let comment=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","P"=>page,"Rect"=>vec![250.into(),350.into(),270.into(),370.into()],"Contents"=>Object::string_literal("Keep this note"),"C"=>vec![1.into(),0.into(),0.into()],"F"=>4});
    doc.get_dictionary_mut(page)
        .unwrap()
        .get_mut(b"Annots")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .push(comment.into());
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    let (temp, e, info) = open(&bytes);
    assert!(info.pages[0].overlays.is_empty());
    assert_eq!(e.list_native_comments(&info.id).unwrap()[0].text,"Keep this note");
    let field = e.list_form_fields(&info.id, 0).unwrap().remove(0);
    let filled = e
        .fill_form_field(
            &info.id,
            FillFormFieldRequest {
                page_index: 0,
                annotation_index: field.annotation_index,
                expected_value: field.value,
                value: FormFieldValue::Text {
                    text: "Recovered value".into(),
                },
            },
        )
        .unwrap();
    assert!(
        filled.pages.iter().all(|p| p.overlays.is_empty()),
        "existing workspace owns imported overlays"
    );
    let plans = info
        .pages
        .iter()
        .enumerate()
        .map(|(i, p)| PagePlan {
            crop: None,
            id: format!("p{i}"),
            source_id: filled.id.clone(),
            page_index: i,
            width: p.width,
            height: p.height,
            rotation: 0,
            overlays: p.overlays.clone(),
        })
        .collect::<Vec<_>>();
    let output = temp.path().join("filled.pdf");
    e.export_pdf(
        ExportRequest {
            pages: plans.clone(),
            flatten: false,
        },
        &output,
    )
    .unwrap();
    let reopened = e.open_document(&output).unwrap();
    assert!(reopened.pages[0].overlays.is_empty());
    assert_eq!(e.list_native_comments(&reopened.id).unwrap()[0].text,"Keep this note");
    assert_eq!(
        e.list_form_fields(&reopened.id, 1).unwrap()[0].value,
        FormFieldValue::Text {
            text: "Recovered value".into()
        }
    );
    assert_eq!(
        e.render_page(&filled.id, 0, 300).unwrap(),
        e.render_page(&reopened.id, 0, 300).unwrap(),
        "imported comments must not be rasterized twice"
    );
    let store = RecoveryStore::new(temp.path().join("recovery")).unwrap();
    store.save(&e,json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","savedDigest":"old","dirty":true,"zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"Form.pdf","selectedPageId":"p0","selectedOverlayId":null,"pages":plans}}]})).unwrap();
    e.close_document(&filled.id).unwrap();
    let restored = store.load(&e).unwrap().unwrap();
    let source = restored["tabs"][0]["document"]["pages"][0]["sourceId"]
        .as_str()
        .unwrap();
    assert_eq!(
        e.list_form_fields(source, 1).unwrap()[0].value,
        FormFieldValue::Text {
            text: "Recovered value".into()
        }
    );
    assert_eq!(
        restored["tabs"][0]["document"]["pages"][0]["overlays"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(e.list_native_comments(source).unwrap()[0].text,"Keep this note");
    assert_eq!(fs::read(temp.path().join("form.pdf")).unwrap(), bytes);
    for id in [source, info.id.as_str(), reopened.id.as_str()] {
        e.close_document(id).unwrap();
    }
}

#[test]
fn focus_and_blur_actions_are_preserved_but_never_run_while_filling() {
    let mut doc = Document::load_mem(&fixture()).unwrap();
    let page = *doc.get_pages().values().next().unwrap();
    let field = doc
        .get_dictionary(page)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .as_reference()
        .unwrap();
    doc.get_dictionary_mut(field).unwrap().set(
        "AA",
        dictionary! {
            "Fo"=>dictionary!{"S"=>"ResetForm","Fields"=>vec![Object::string_literal("consent")]},
            "Bl"=>dictionary!{"S"=>"Hide","T"=>Object::string_literal("consent"),"H"=>true}
        },
    );
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    let (_temp, e, info) = open(&bytes);
    let fields = e.list_form_fields(&info.id, 0).unwrap();
    let text = &fields[0];
    let consent = &fields[1];
    let next = e
        .fill_form_field(
            &info.id,
            FillFormFieldRequest {
                page_index: 0,
                annotation_index: text.annotation_index,
                expected_value: text.value.clone(),
                value: FormFieldValue::Text {
                    text: "Edited without actions".into(),
                },
            },
        )
        .unwrap();
    let after = e.list_form_fields(&next.id, 0).unwrap();
    assert_eq!(
        after[1].value, consent.value,
        "focus ResetForm must not overwrite another value"
    );
    assert_eq!(
        after[1].reason, consent.reason,
        "blur Hide must not hide another field"
    );
    let saved = Document::load_mem(&e.source_bytes(&next.id).unwrap()).unwrap();
    let aa = saved
        .get_dictionary(field)
        .unwrap()
        .get(b"AA")
        .unwrap()
        .as_dict()
        .unwrap();
    assert_eq!(
        aa.get(b"Fo")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"S")
            .unwrap()
            .as_name()
            .unwrap(),
        b"ResetForm"
    );
    assert_eq!(
        aa.get(b"Bl")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"S")
            .unwrap()
            .as_name()
            .unwrap(),
        b"Hide"
    );
    assert_eq!(e.source_bytes(&info.id).unwrap().as_ref(), bytes.as_slice());
    e.close_document(&info.id).unwrap();
    e.close_document(&next.id).unwrap();
}

#[test]
fn rejects_hybrid_xfa_before_inspection_or_editing() {
    let mut doc = Document::load_mem(&fixture()).unwrap();
    let form = doc
        .catalog()
        .unwrap()
        .get(b"AcroForm")
        .unwrap()
        .as_reference()
        .unwrap();
    let xfa = doc.add_object(Stream::new(
        Dictionary::new(),
        b"<xdp:xdp xmlns:xdp='http://ns.adobe.com/xdp/'/>".to_vec(),
    ));
    doc.get_dictionary_mut(form).unwrap().set("XFA", xfa);
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    let (_temp, e, info) = open(&bytes);
    assert!(e
        .list_form_fields(&info.id, 0)
        .unwrap_err()
        .to_string()
        .contains("XFA"));
    assert!(e
        .fill_form_field(
            &info.id,
            FillFormFieldRequest {
                page_index: 0,
                annotation_index: 0,
                expected_value: FormFieldValue::Text {
                    text: "Original name".into()
                },
                value: FormFieldValue::Text { text: "new".into() }
            }
        )
        .unwrap_err()
        .to_string()
        .contains("XFA"));
    e.close_document(&info.id).unwrap();
}

#[test]
fn editable_combo_selects_export_value_by_index_even_with_duplicate_labels() {
    let mut doc = Document::load_mem(&controls()).unwrap();
    let field = doc
        .objects
        .iter()
        .find(|(_, o)| {
            o.as_dict().is_ok_and(|d| {
                d.get(b"T")
                    .is_ok_and(|t| lopdf::decode_text_string(t).ok().as_deref() == Some("free"))
            })
        })
        .unwrap()
        .0
        .to_owned();
    doc.get_dictionary_mut(field)
        .unwrap()
        .get_mut(b"Opt")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .push(Object::Array(vec![
            Object::string_literal("c"),
            Object::string_literal("Beta"),
        ]));
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    let (_temp, e, info) = open(&bytes);
    let field = e
        .list_form_fields(&info.id, 0)
        .unwrap()
        .into_iter()
        .find(|f| f.name == "free")
        .unwrap();
    assert_eq!(field.options, vec!["Alpha", "Beta", "Beta"]);
    let next = e
        .fill_form_field(
            &info.id,
            FillFormFieldRequest {
                page_index: 0,
                annotation_index: field.annotation_index,
                expected_value: field.value,
                value: FormFieldValue::Choice { indices: vec![2] },
            },
        )
        .unwrap();
    assert_eq!(
        e.list_form_fields(&next.id, 0)
            .unwrap()
            .into_iter()
            .find(|f| f.name == "free")
            .unwrap()
            .value,
        FormFieldValue::Text { text: "c".into() }
    );
    e.close_document(&info.id).unwrap();
    e.close_document(&next.id).unwrap();
}


#[test]
fn mixed_physical_page_units_survive_form_filling_and_export() {
    let mut pdf=Document::load_mem(&fixture()).unwrap();let ids=pdf.get_pages().values().copied().collect::<Vec<_>>();
    let units=[0.5_f32,2.,1.3];for(id,unit)in ids.iter().zip(units){pdf.get_dictionary_mut(*id).unwrap().set("UserUnit",unit);}
    let mut bytes=vec![];pdf.save_to(&mut bytes).unwrap();let(temp,e,info)=open(&bytes);
    for(index,unit)in units.iter().enumerate(){assert_eq!((info.pages[index].width,info.pages[index].height),(300.*unit,400.*unit));let field=e.list_form_fields(&info.id,index).unwrap().remove(0);let b=field.bounds.unwrap();assert_eq!([b.x,b.y,b.width,b.height],[40.*unit,170.*unit,180.*unit,30.*unit]);}
    let field=e.list_form_fields(&info.id,0).unwrap().remove(0);let filled=e.fill_form_field(&info.id,FillFormFieldRequest{page_index:0,annotation_index:field.annotation_index,expected_value:field.value,value:FormFieldValue::Text{text:"Physical value".into()}}).unwrap();
    let request:folio_engine::ExportRequest=serde_json::from_value(serde_json::json!({"pages":([2,0,1,0].iter().enumerate().map(|(slot,i)|serde_json::json!({"id":format!("p-{slot}"),"sourceId":filled.id,"pageIndex":i,"width":filled.pages[*i].width,"height":filled.pages[*i].height,"rotation":0,"overlays":[{"type":"comment","id":format!("note-{slot}"),"x":20,"y":20,"text":"Physical note","color":"#FFFF00"}]})).collect::<Vec<_>>())})).unwrap();
    let output=temp.path().join("mixed-units-export.pdf");e.export_pdf(request,&output).unwrap();let opened=e.open_document(&output).unwrap();
    for(index,original)in [2,0,1,0].iter().enumerate(){let unit=units[*original];assert_eq!((opened.pages[index].width,opened.pages[index].height),(300.*unit,400.*unit));let field=e.list_form_fields(&opened.id,index).unwrap().remove(0);if *original!=2{assert_eq!(field.value,FormFieldValue::Text{text:"Physical value".into()});}let b=field.bounds.unwrap();assert_eq!([b.x,b.y,b.width,b.height],[40.*unit,170.*unit,180.*unit,30.*unit]);}
    let exported=Document::load(&output).unwrap();for(index,id)in exported.get_pages().values().enumerate(){
        let page=exported.get_dictionary(*id).unwrap();let annots=exported.dereference(page.get(b"Annots").unwrap()).unwrap().1.as_array().unwrap();
        let note=annots.iter().map(|a|exported.dereference(a).unwrap().1.as_dict().unwrap()).find(|a|a.get(b"Subtype").is_ok_and(|v|v.as_name().is_ok_and(|name|name==b"Text"))).unwrap();
        let rect=note.get(b"Rect").unwrap().as_array().unwrap().iter().map(|v|v.as_float().unwrap()).collect::<Vec<_>>();let unit=units[[2,0,1,0][index]];
        assert!(((rect[2]-rect[0])*unit-24.).abs()<0.001,"Comment icon must remain 24 physical points");
    }
    assert_eq!(fs::read(temp.path().join("form.pdf")).unwrap(),bytes);for id in [&info.id,&filled.id,&opened.id]{e.close_document(id).unwrap();}
}
