//! Public fill regressions: a native answer update must not erase static AP art.
use folio_engine::{
    DocumentInfo, ExportRequest, FillFormFieldRequest, FormFieldValue, PagePlan, PdfEngine,
};
use lopdf::{content::Content, dictionary, Document, Object, ObjectId, Stream};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy)]
enum Kind {
    Text,
    Combo,
    List,
}

fn fixture(kind: Kind, widgets: usize, cross_page: bool) -> Vec<u8> {
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let owner = doc.new_object_id();
    let font =
        doc.add_object(dictionary! {"Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"Helvetica"});
    let answer = if matches!(kind, Kind::Text) {
        "Edited answer"
    } else {
        "Second"
    };
    // Hand-authored independent source: the red square follows the answer layer.
    // A renderer that replaces the whole AP loses this square on the first fill.
    let appearance = doc.add_object(Stream::new(
        dictionary! {"Type"=>"XObject", "Subtype"=>"Form", "BBox"=>vec![0.into(),0.into(),280.into(),50.into()],
            "Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}},
        format!("q 0 g 0 0 280 50 re 0.5 0.5 279 49 re f* Q\n/Tx BMC q BT 0 g /F1 12 Tf 0.5 20.836 Td ({answer}) Tj ET Q EMC\nq 1 0 0 rg 250 20 5 5 re f Q\n").into_bytes(),
    ));
    let page_count = if cross_page { widgets } else { 1 };
    let page_ids: Vec<_> = (0..page_count).map(|_| doc.new_object_id()).collect();
    let mut annots = vec![vec![]; page_count];
    let mut children = vec![];
    for index in 0..widgets {
        let page = if cross_page { index } else { 0 };
        let bottom = if cross_page {
            120
        } else {
            120 - 70 * index as i64
        };
        let widget = doc.add_object(dictionary! {"Type"=>"Annot", "Subtype"=>"Widget", "Parent"=>owner,
            "P"=>page_ids[page], "F"=>4, "Rect"=>vec![20.into(),bottom.into(),300.into(),(bottom+50).into()],
            "MK"=>dictionary!{"BC"=>vec![0.into()]}, "BS"=>dictionary!{"W"=>Object::Real(0.5),"S"=>"S"},
            "AP"=>dictionary!{"N"=>appearance}});
        annots[page].push(Object::Reference(widget));
        children.push(Object::Reference(widget));
    }
    let mut field = dictionary! {"FT"=>if matches!(kind,Kind::Text) {"Tx"} else {"Ch"},
    "T"=>Object::string_literal("artwork"), "V"=>Object::string_literal(answer),
    "DV"=>Object::string_literal(if matches!(kind,Kind::Text) {"Default answer"} else {"Second"}),
    "Kids"=>children};
    if !matches!(kind, Kind::Text) {
        field.set(
            "Opt",
            vec![
                Object::string_literal("First"),
                Object::string_literal("Second"),
            ],
        );
        field.set("I", vec![Object::Integer(1)]);
        field.set(
            "Ff",
            if matches!(kind, Kind::Combo) {
                1 << 17
            } else {
                0
            },
        );
    }
    doc.objects.insert(owner, field.into());
    for (index, id) in page_ids.iter().enumerate() {
        doc.objects.insert(
            *id,
            dictionary! {"Type"=>"Page", "Parent"=>pages,
            "MediaBox"=>vec![0.into(),0.into(),320.into(),240.into()], "Resources"=>dictionary!{},
            "Annots"=>annots[index].clone()}
            .into(),
        );
    }
    doc.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages", "Count"=>page_count as i64,
        "Kids"=>page_ids.iter().copied().map(Object::Reference).collect::<Vec<_>>()}
        .into(),
    );
    let form = doc.add_object(dictionary! {"Fields"=>vec![Object::Reference(owner)],
        "DA"=>Object::string_literal("/F1 12 Tf 0 g"), "DR"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},
        "NeedAppearances"=>false});
    let catalog = doc.add_object(dictionary! {"Type"=>"Catalog", "Pages"=>pages, "AcroForm"=>form});
    doc.trailer.set("Root", catalog);
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    bytes
}

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}

#[test]
fn review_empty_caption_refuses_but_inherited_linked_logos_survive_filling() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for case in ["caption", "inherited", "font-only"] {
        let mut doc = Document::load_mem(&fixture(Kind::Text, 2, true)).unwrap();
        let widget = widgets(&doc)[0];
        let owner = doc
            .get_dictionary(widget)
            .unwrap()
            .get(b"Parent")
            .unwrap()
            .as_reference()
            .unwrap();
        let ap = doc
            .get_dictionary(widget)
            .unwrap()
            .get(b"AP")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"N")
            .unwrap()
            .as_reference()
            .unwrap();
        if case == "caption" {
            doc.get_dictionary_mut(owner)
                .unwrap()
                .set("V", Object::string_literal(""));
            doc.get_object_mut(ap)
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .set_plain_content(b"BT /F1 8 Tf 5 30 Td (Account number:) Tj ET".to_vec());
        } else {
            let resources = doc
                .get_object(ap)
                .unwrap()
                .as_stream()
                .unwrap()
                .dict
                .get(b"Resources")
                .unwrap()
                .clone();
            for (index, page) in doc.get_pages().values().enumerate() {
                let logo = doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),280.into(),50.into()]}, format!("q {} 0 0 rg 250 20 5 5 re f Q",if index==0 {1.0} else {0.5}).into_bytes()));
                let mut page_resources = resources.as_dict().unwrap().clone();
                page_resources.set("XObject", dictionary! {"Logo"=>logo});
                doc.get_dictionary_mut(*page)
                    .unwrap()
                    .set("Resources", page_resources);
            }
            let stream = doc.get_object_mut(ap).unwrap().as_stream_mut().unwrap();
            if case == "inherited" {
                stream.dict.remove(b"Resources");
            }
            stream.set_plain_content(
                b"/Tx BMC q BT /F1 12 Tf (Edited answer) Tj ET Q EMC /Logo Do".to_vec(),
            );
        }
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        let path = temp.path().join(format!("{case}.pdf"));
        fs::write(&path, &bytes).unwrap();
        let source = e.open_document(&path).unwrap();
        let before: Vec<_> = (0..2)
            .map(|page| e.render_page(&source.id, page, 640).unwrap())
            .collect();
        if case != "caption" {
            assert_red_pixels(&e, &source, 1, false);
            let second = image::load_from_memory(&before[1]).unwrap().to_rgb8();
            assert_eq!(
                second.get_pixel(545, 195).0,
                [128, 0, 0],
                "second placement must use its distinct inherited logo"
            );
        }
        let field = e.list_form_fields(&source.id, 0).unwrap().remove(0);
        let result = e.fill_form_field(
            &source.id,
            FillFormFieldRequest {
                page_index: 0,
                annotation_index: field.annotation_index,
                expected_value: field.value,
                value: FormFieldValue::Text {
                    text: "12345".into(),
                },
            },
        );
        if case == "caption" {
            assert!(result.is_err(), "ambiguous caption was published");
        } else {
            let filled = result.expect("valid inherited artwork must remain fillable");
            let saved_path = temp.path().join(format!("{case}-filled.pdf"));
            let saved = export(&e, &filled, &saved_path);
            let refilled = apply(&e, &saved, FormFieldValue::Text { text: "Again".into() });
            for (document, expected) in [(&filled, "12345"), (&saved, "12345"), (&refilled, "Again")] {
                for page in 0..2 {
                    assert_eq!(e.list_form_fields(&document.id, page).unwrap()[0].value,
                        FormFieldValue::Text { text: expected.into() });
                    let actual = image::load_from_memory(&e.render_page(&document.id, page, 640).unwrap()).unwrap().to_rgb8();
                    let original = image::load_from_memory(&before[page]).unwrap().to_rgb8();
                    for y in 191..199 { for x in 541..549 {
                        assert_eq!(actual.get_pixel(x,y), original.get_pixel(x,y),
                            "{case}: inherited logo changed on page {page}");
                    }}
                }
                let graph = Document::load_mem(&e.source_bytes(&document.id).unwrap()).unwrap();
                let parent = graph.get_dictionary(owner).unwrap();
                assert_eq!(parent.get(b"DV").unwrap().as_str().unwrap(), b"Default answer");
                assert_eq!(parent.get(b"Kids").unwrap().as_array().unwrap().len(), 2);
            }
            for document in [&refilled, &saved, &filled] { e.close_document(&document.id).unwrap(); }
        }
        assert_eq!(
            e.source_bytes(&source.id).unwrap().as_ref(),
            bytes.as_slice()
        );
        for page in 0..2 {
            assert_eq!(e.render_page(&source.id, page, 640).unwrap(), before[page]);
        }
        assert_eq!(fs::read(path).unwrap(), bytes);
        e.close_document(&source.id).unwrap();
    }
}
fn resolve<'a>(doc: &'a Document, value: &'a Object) -> &'a Object {
    doc.dereference(value).unwrap().1
}
fn widgets(doc: &Document) -> Vec<ObjectId> {
    doc.get_pages()
        .values()
        .flat_map(|id| {
            resolve(
                doc,
                doc.get_dictionary(*id).unwrap().get(b"Annots").unwrap(),
            )
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_reference().unwrap())
            .collect::<Vec<_>>()
        })
        .collect()
}
fn assert_source_art_and_defaults(bytes: &[u8], kind: Kind, count: usize) {
    let doc = Document::load_mem(bytes).unwrap();
    let ids = widgets(&doc);
    assert_eq!(ids.len(), count);
    for id in ids {
        let widget = doc.get_dictionary(id).unwrap();
        let ap = resolve(&doc, widget.get(b"AP").unwrap()).as_dict().unwrap();
        let stream = resolve(&doc, ap.get(b"N").unwrap()).as_stream().unwrap();
        let content = Content::decode(
            &stream
                .get_plain_content_with_limit(4 * 1024 * 1024)
                .unwrap(),
        )
        .unwrap();
        assert!(
            content.operations.iter().any(|op| op.operator == "re"
                && op.operands.len() == 4
                && op
                    .operands
                    .iter()
                    .map(|v| v.as_float().unwrap())
                    .eq([250., 20., 5., 5.])),
            "fill erased the original non-answer artwork rectangle"
        );
        let mut node = widget;
        for _ in 0..64 {
            if let Ok(value) = node.get(b"DV") {
                assert_eq!(
                    lopdf::decode_text_string(resolve(&doc, value)).unwrap(),
                    if matches!(kind, Kind::Text) {
                        "Default answer"
                    } else {
                        "Second"
                    }
                );
                break;
            }
            node = resolve(&doc, node.get(b"Parent").expect("original default missing"))
                .as_dict()
                .unwrap();
        }
    }
}
fn assert_red_pixels(e: &PdfEngine, info: &DocumentInfo, widgets: usize, cross_page: bool) {
    for index in 0..widgets {
        let page = if cross_page { index } else { 0 };
        let png = e.render_page(&info.id, page, 640).unwrap();
        let image = image::load_from_memory(&png).unwrap().into_rgb8();
        assert_eq!(image.dimensions(), (640, 480));
        let top = if cross_page {
            191
        } else {
            191 + 140 * index as u32
        };
        for y in top..top + 8 {
            for x in 541..549 {
                let p = image.get_pixel(x, y).0;
                assert!(
                    p[0] > 240 && p[1] < 15 && p[2] < 15,
                    "static red artwork changed at {page}/{x},{y}: {p:?}"
                );
            }
        }
    }
}
fn apply(e: &PdfEngine, source: &DocumentInfo, wanted: FormFieldValue) -> DocumentInfo {
    let field = e.list_form_fields(&source.id, 0).unwrap().remove(0);
    e.fill_form_field(
        &source.id,
        FillFormFieldRequest {
            page_index: 0,
            annotation_index: field.annotation_index,
            expected_value: field.value,
            value: wanted,
        },
    )
    .unwrap()
}
fn export(e: &PdfEngine, info: &DocumentInfo, path: &Path) -> DocumentInfo {
    let pages: Vec<PagePlan> = info
        .pages
        .iter()
        .enumerate()
        .map(|(i, p)| {
            serde_json::from_value(serde_json::json!({
        "id":format!("page-{i}"),"sourceId":info.id,"pageIndex":i,"width":p.width,"height":p.height,
        "rotation":0,"overlays":p.overlays}))
            .unwrap()
        })
        .collect();
    e.export_pdf(
        ExportRequest {
            pages,
            flatten: false,
        },
        path,
    )
    .unwrap();
    e.open_document(path).unwrap()
}
fn exercise(kind: Kind, count: usize, cross_page: bool) {
    let temp = tempfile::tempdir().unwrap();
    let original = fixture(kind, count, cross_page);
    let path = temp.path().join("original.pdf");
    fs::write(&path, &original).unwrap();
    let e = engine();
    let source = e.open_document(&path).unwrap();
    assert_source_art_and_defaults(&original, kind, count);
    assert_red_pixels(&e, &source, count, cross_page);
    let values = match kind {
        Kind::Text => vec![
            FormFieldValue::Text {
                text: "Later answer".into(),
            },
            FormFieldValue::Text {
                text: String::new(),
            },
            FormFieldValue::Text {
                text: "Refilled café".into(),
            },
        ],
        Kind::Combo => vec![
            FormFieldValue::Choice { indices: vec![0] },
            FormFieldValue::Choice { indices: vec![1] },
        ],
        Kind::List => vec![
            FormFieldValue::Choice { indices: vec![0] },
            FormFieldValue::Choice { indices: vec![] },
            FormFieldValue::Choice { indices: vec![1] },
        ],
    };
    let mut current = source.clone();
    let mut owned = vec![source.id.clone()];
    for (step, wanted) in values.into_iter().enumerate() {
        let filled = apply(&e, &current, wanted.clone());
        owned.push(filled.id.clone());
        for page in 0..filled.pages.len() {
            for field in e.list_form_fields(&filled.id, page).unwrap() {
                assert_eq!(field.value, wanted, "linked widget value differs");
            }
        }
        assert_source_art_and_defaults(&e.source_bytes(&filled.id).unwrap(), kind, count);
        assert_red_pixels(&e, &filled, count, cross_page);
        let saved = temp.path().join(format!("saved-{step}.pdf"));
        current = export(&e, &filled, &saved);
        owned.push(current.id.clone());
        assert_source_art_and_defaults(&fs::read(&saved).unwrap(), kind, count);
        assert_red_pixels(&e, &current, count, cross_page);
        for page in 0..current.pages.len() {
            for field in e.list_form_fields(&current.id, page).unwrap() {
                assert_eq!(field.value, wanted, "saved answer differs");
            }
        }
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(
            e.source_bytes(&source.id).unwrap().as_ref(),
            original.as_slice()
        );
    }
    for id in owned {
        e.close_document(&id).unwrap();
    }
}

#[test]
fn text_fill_preserves_artwork_through_clear_refill_and_saved_reopen() {
    exercise(Kind::Text, 1, false)
}
#[test]
fn dropdown_fill_preserves_artwork_and_original_default() {
    exercise(Kind::Combo, 1, false)
}
#[test]
fn list_fill_preserves_artwork_through_clear_refill_and_saved_reopen() {
    exercise(Kind::List, 1, false)
}
#[test]
fn linked_same_page_text_widgets_keep_artwork_and_values() {
    exercise(Kind::Text, 2, false)
}
#[test]
fn linked_cross_page_text_widgets_keep_artwork_and_values() {
    exercise(Kind::Text, 2, true)
}

#[test]
fn unchanged_valid_answer_keeps_the_exact_source_bytes() {
    let temp = tempfile::tempdir().unwrap();
    let bytes = fixture(Kind::Text, 1, false);
    let path = temp.path().join("unchanged.pdf");
    fs::write(&path, &bytes).unwrap();
    let e = engine();
    let source = e.open_document(&path).unwrap();
    let value = e.list_form_fields(&source.id, 0).unwrap().remove(0).value;
    let unchanged = apply(&e, &source, value);
    assert!(
        e.source_bytes(&unchanged.id).unwrap().as_ref() == bytes.as_slice(),
        "unchanged fill reserialized or modified its source"
    );
    e.close_document(&unchanged.id).unwrap();
    e.close_document(&source.id).unwrap();
}

fn form_id(doc: &Document) -> ObjectId {
    doc.catalog()
        .unwrap()
        .get(b"AcroForm")
        .unwrap()
        .as_reference()
        .unwrap()
}
fn independent_fields(global: bool, missing_selected: bool) -> Vec<u8> {
    let mut doc = Document::load_mem(&fixture(Kind::Text, 2, false)).unwrap();
    let ids = widgets(&doc);
    let owner = doc
        .get_dictionary(ids[0])
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    let mut second = doc.get_dictionary(owner).unwrap().clone();
    second.set("T", Object::string_literal("unrelated"));
    second.set("V", Object::string_literal("Stay"));
    second.set("Kids", vec![Object::Reference(ids[1])]);
    let second = doc.add_object(second);
    doc.get_dictionary_mut(ids[1])
        .unwrap()
        .set("Parent", second);
    doc.get_dictionary_mut(owner)
        .unwrap()
        .set("Kids", vec![Object::Reference(ids[0])]);
    let form = form_id(&doc);
    doc.get_dictionary_mut(form).unwrap().set(
        "Fields",
        vec![Object::Reference(owner), Object::Reference(second)],
    );
    doc.get_dictionary_mut(form)
        .unwrap()
        .set("NeedAppearances", global);
    if global {
        for id in &ids {
            doc.get_dictionary_mut(*id).unwrap().remove(b"AP");
        }
    } else if missing_selected {
        let old = doc
            .get_dictionary(ids[0])
            .unwrap()
            .get(b"AP")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"N")
            .unwrap()
            .clone();
        doc.get_dictionary_mut(ids[0])
            .unwrap()
            .set("AP", dictionary! {"R"=>old});
    }
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}

#[test]
fn missing_normal_appearance_is_materialized_without_changing_unrelated_artwork() {
    let original = independent_fields(false, true);
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("missing.pdf");
    fs::write(&path, &original).unwrap();
    let e = engine();
    let source = e.open_document(&path).unwrap();
    let wanted = FormFieldValue::Text {
        text: "New answer".into(),
    };
    let filled = apply(&e, &source, wanted.clone());
    let bytes = e.source_bytes(&filled.id).unwrap();
    let after = Document::load_mem(&bytes).unwrap();
    let before = Document::load_mem(&original).unwrap();
    let old_ids = widgets(&before);
    let ids = widgets(&after);
    let ap = resolve(
        &after,
        after.get_dictionary(ids[0]).unwrap().get(b"AP").unwrap(),
    )
    .as_dict()
    .unwrap();
    assert!(resolve(&after, ap.get(b"N").unwrap()).as_stream().is_ok());
    assert!(ap.has(b"R"));
    let normal = |doc: &Document, id: ObjectId| {
        resolve(
            doc,
            resolve(doc, doc.get_dictionary(id).unwrap().get(b"AP").unwrap())
                .as_dict()
                .unwrap()
                .get(b"N")
                .unwrap(),
        )
        .as_stream()
        .unwrap()
        .content
        .clone()
    };
    assert_eq!(
        normal(&before, old_ids[1]),
        normal(&after, ids[1]),
        "unrelated appearance changed"
    );
    assert_eq!(
        e.list_form_fields(&filled.id, 0).unwrap()[1].value,
        FormFieldValue::Text {
            text: "Stay".into()
        }
    );
    let saved = export(&e, &filled, &temp.path().join("saved.pdf"));
    assert_eq!(e.list_form_fields(&saved.id, 0).unwrap()[0].value, wanted);
    assert_eq!(fs::read(&path).unwrap(), original);
    for id in [&source.id, &filled.id, &saved.id] {
        e.close_document(id).unwrap();
    }
}

#[test]
fn global_regeneration_materializes_unrelated_fields_before_clearing_the_flag() {
    let original = independent_fields(true, true);
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("global.pdf");
    fs::write(&path, &original).unwrap();
    let e = engine();
    let source = e.open_document(&path).unwrap();
    let filled = apply(
        &e,
        &source,
        FormFieldValue::Text {
            text: "New answer".into(),
        },
    );
    let bytes = e.source_bytes(&filled.id).unwrap();
    let doc = Document::load_mem(&bytes).unwrap();
    assert_eq!(
        doc.get_dictionary(form_id(&doc))
            .unwrap()
            .get(b"NeedAppearances")
            .unwrap(),
        &Object::Boolean(false)
    );
    for id in widgets(&doc) {
        let ap = resolve(&doc, doc.get_dictionary(id).unwrap().get(b"AP").unwrap())
            .as_dict()
            .unwrap();
        let appearance = resolve(&doc, ap.get(b"N").unwrap()).as_stream().unwrap();
        assert!(!appearance
            .get_plain_content_with_limit(4 * 1024 * 1024)
            .unwrap()
            .is_empty());
    }
    let saved = export(&e, &filled, &temp.path().join("saved.pdf"));
    assert_eq!(
        e.list_form_fields(&saved.id, 0).unwrap()[1].value,
        FormFieldValue::Text {
            text: "Stay".into()
        }
    );
    let png = e.render_page(&saved.id, 0, 640).unwrap();
    let image = image::load_from_memory(&png).unwrap().into_rgb8();
    assert!(
        (240..380).any(|y| (40..300).any(|x| {
            let p = image.get_pixel(x, y).0;
            p[0] < 180 && p[1] < 180 && p[2] < 180
        })),
        "unrelated field became blank"
    );
    assert_eq!(fs::read(&path).unwrap(), original);
    for id in [&source.id, &filled.id, &saved.id] {
        e.close_document(id).unwrap();
    }
}

#[test]
fn repeated_fills_preserve_artwork_without_accumulating_appearance_graphs() {
    let original = fixture(Kind::Text, 1, false);
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("repeat.pdf");
    fs::write(&path, &original).unwrap();
    let e = engine();
    let mut current = e.open_document(&path).unwrap();
    let mut baseline = None;
    for n in 0..20 {
        let next = apply(
            &e,
            &current,
            FormFieldValue::Text {
                text: format!("Round {n:02}"),
            },
        );
        let bytes = e.source_bytes(&next.id).unwrap();
        let doc = Document::load_mem(&bytes).unwrap();
        assert_source_art_and_defaults(&bytes, Kind::Text, 1);
        assert_red_pixels(&e, &next, 1, false);
        if let Some((count, size)) = baseline {
            assert!(
                doc.objects.len() <= count + 3,
                "appearance objects keep accumulating: {} > {count}+3",
                doc.objects.len()
            );
            assert!(
                bytes.len() <= size + 4096,
                "repeated fill bytes keep accumulating"
            );
        } else {
            baseline = Some((doc.objects.len(), bytes.len()));
        }
        e.close_document(&current.id).unwrap();
        current = next;
    }
    e.close_document(&current.id).unwrap();
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn missing_normal_can_be_cleared_first_then_refilled() {
    let original = independent_fields(false, true);
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("missing-clear.pdf");
    fs::write(&path, &original).unwrap();
    let e = engine();
    let mut current = e.open_document(&path).unwrap();
    for text in ["", "Filled again", "", "Last answer"] {
        let wanted = FormFieldValue::Text { text: text.into() };
        let next = apply(&e, &current, wanted.clone());
        assert_eq!(e.list_form_fields(&next.id, 0).unwrap()[0].value, wanted);
        e.close_document(&current.id).unwrap();
        current = next;
    }
    let saved = export(&e, &current, &temp.path().join("saved.pdf"));
    assert_eq!(
        e.list_form_fields(&saved.id, 0).unwrap()[0].value,
        FormFieldValue::Text {
            text: "Last answer".into()
        }
    );
    e.close_document(&saved.id).unwrap();
    e.close_document(&current.id).unwrap();
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn initially_empty_unmarked_text_retains_its_static_artwork_when_filled() {
    let mut doc = Document::load_mem(&fixture(Kind::Text, 1, false)).unwrap();
    let widget = widgets(&doc)[0];
    let owner = doc
        .get_dictionary(widget)
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    doc.get_dictionary_mut(owner)
        .unwrap()
        .set("V", Object::string_literal(""));
    let appearance = doc
        .get_dictionary(widget)
        .unwrap()
        .get(b"AP")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"N")
        .unwrap()
        .as_reference()
        .unwrap();
    doc.get_object_mut(appearance)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .set_plain_content(
            b"q 0 g 0 0 280 50 re 0.5 0.5 279 49 re f* Q q 1 0 0 rg 250 20 5 5 re f Q".to_vec(),
        );
    let mut original = Vec::new();
    doc.save_to(&mut original).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("blank.pdf");
    fs::write(&path, &original).unwrap();
    let e = engine();
    let source = e.open_document(&path).unwrap();
    let filled = apply(
        &e,
        &source,
        FormFieldValue::Text {
            text: "First answer".into(),
        },
    );
    assert_source_art_and_defaults(&e.source_bytes(&filled.id).unwrap(), Kind::Text, 1);
    assert_red_pixels(&e, &filled, 1, false);
    e.close_document(&filled.id).unwrap();
    e.close_document(&source.id).unwrap();
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn ordinary_fill_keeps_the_existing_long_unicode_limit() {
    let mut doc = Document::load_mem(&fixture(Kind::Text, 1, false)).unwrap();
    let widget = widgets(&doc)[0];
    let owner = doc
        .get_dictionary(widget)
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    doc.get_dictionary_mut(owner).unwrap().set("Ff", 1 << 12);
    let mut original = Vec::new();
    doc.save_to(&mut original).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("long.pdf");
    fs::write(&path, &original).unwrap();
    let e = engine();
    let source = e.open_document(&path).unwrap();
    let text = "éA\nB".repeat(8192);
    assert_eq!(text.encode_utf16().count(), 32768);
    let value = FormFieldValue::Text { text };
    let filled = apply(&e, &source, value.clone());
    assert_eq!(e.list_form_fields(&filled.id, 0).unwrap()[0].value, value);
    let saved = export(&e, &filled, &temp.path().join("saved.pdf"));
    assert_eq!(e.list_form_fields(&saved.id, 0).unwrap()[0].value, value);
    assert_source_art_and_defaults(&e.source_bytes(&filled.id).unwrap(), Kind::Text, 1);
    let FormFieldValue::Text { text } = value else {
        unreachable!()
    };
    assert!(e
        .fill_form_field(
            &filled.id,
            FillFormFieldRequest {
                page_index: 0,
                annotation_index: 0,
                expected_value: FormFieldValue::Text { text: text.clone() },
                value: FormFieldValue::Text {
                    text: format!("{text}x")
                }
            }
        )
        .is_err());
    for id in [&source.id, &filled.id, &saved.id] {
        e.close_document(id).unwrap();
    }
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn unprovable_global_baseline_refuses_without_changing_the_source() {
    let mut doc = Document::load_mem(&independent_fields(true, true)).unwrap();
    let second = widgets(&doc)[1];
    let owner = doc
        .get_dictionary(second)
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    let field = doc.get_dictionary_mut(owner).unwrap();
    field.set("FT", "Sig");
    field.remove(b"V");
    field.remove(b"DV");
    let mut original = Vec::new();
    doc.save_to(&mut original).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("unprovable.pdf");
    fs::write(&path, &original).unwrap();
    let e = engine();
    let source = e.open_document(&path).unwrap();
    let before = e.list_form_fields(&source.id, 0).unwrap()[0].value.clone();
    let result = e.fill_form_field(
        &source.id,
        FillFormFieldRequest {
            page_index: 0,
            annotation_index: 0,
            expected_value: before.clone(),
            value: FormFieldValue::Text {
                text: "Refused".into(),
            },
        },
    );
    assert!(
        result.is_err(),
        "unmaterializable global field was silently discarded"
    );
    assert_eq!(e.list_form_fields(&source.id, 0).unwrap()[0].value, before);
    assert_eq!(
        e.source_bytes(&source.id).unwrap().as_ref(),
        original.as_slice()
    );
    assert_eq!(fs::read(&path).unwrap(), original);
    e.close_document(&source.id).unwrap();
}

#[test]
fn custom_button_states_and_modes_are_preserved_and_unavailable_states_refuse() {
    let mut doc = Document::load_mem(&fixture(Kind::Text, 1, false)).unwrap();
    let widget = widgets(&doc)[0];
    let owner = doc
        .get_dictionary(widget)
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    let field = doc.get_dictionary_mut(owner).unwrap();
    field.set("FT", "Btn");
    field.set("V", "Accepted");
    field.set("DV", "Accepted");
    let on = doc
        .get_dictionary(widget)
        .unwrap()
        .get(b"AP")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"N")
        .unwrap()
        .clone();
    let initial = resolve(&doc, &on).as_stream().unwrap().content.clone();
    let field = doc.get_dictionary_mut(widget).unwrap();
    field.set("AS", "Accepted");
    field.set(
        "AP",
        dictionary! {"N"=>dictionary!{"Accepted"=>on.clone()},"R"=>on.clone(),"D"=>on},
    );
    let mut original = Vec::new();
    doc.save_to(&mut original).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("button.pdf");
    fs::write(&path, &original).unwrap();
    let e = engine();
    let mut current = e.open_document(&path).unwrap();
    let result = e.fill_form_field(
        &current.id,
        FillFormFieldRequest {
            page_index: 0,
            annotation_index: 0,
            expected_value: FormFieldValue::Toggle { checked: true },
            value: FormFieldValue::Toggle { checked: false },
        },
    );
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("no preserved or native-generated appearance"));
    assert_eq!(
        e.source_bytes(&current.id).unwrap().as_ref(),
        original.as_slice()
    );
    e.close_document(&current.id).unwrap();
    let off=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),280.into(),50.into()]},b"q 0 g 0 0 280 50 re 0.5 0.5 279 49 re f* Q".to_vec()));
    doc.get_dictionary_mut(widget)
        .unwrap()
        .get_mut(b"AP")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .get_mut(b"N")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Off", off);
    let mut valid = Vec::new();
    doc.save_to(&mut valid).unwrap();
    let valid_path = temp.path().join("valid-button.pdf");
    fs::write(&valid_path, &valid).unwrap();
    current = e.open_document(&valid_path).unwrap();
    for checked in [false, true, false] {
        let next = apply(&e, &current, FormFieldValue::Toggle { checked });
        let bytes = e.source_bytes(&next.id).unwrap();
        let doc = Document::load_mem(&bytes).unwrap();
        let id = widgets(&doc)[0];
        let appearance = resolve(&doc, doc.get_dictionary(id).unwrap().get(b"AP").unwrap())
            .as_dict()
            .unwrap();
        let normal = resolve(&doc, appearance.get(b"N").unwrap())
            .as_dict()
            .unwrap();
        assert_eq!(
            resolve(&doc, normal.get(b"Accepted").unwrap())
                .as_stream()
                .unwrap()
                .content,
            initial
        );
        for mode in [b"R", b"D"] {
            assert_eq!(
                resolve(&doc, appearance.get(mode).unwrap())
                    .as_stream()
                    .unwrap()
                    .content,
                initial
            );
        }
        assert!(resolve(&doc, normal.get(b"Off").unwrap())
            .as_stream()
            .is_ok());
        assert_eq!(
            e.list_form_fields(&next.id, 0).unwrap()[0].value,
            FormFieldValue::Toggle { checked }
        );
        e.close_document(&current.id).unwrap();
        current = next;
    }
    e.close_document(&current.id).unwrap();
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn linked_widget_captions_keep_distinct_local_font_bindings() {
    let mut doc = Document::load_mem(&fixture(Kind::Text, 2, true)).unwrap();
    for (index, id) in widgets(&doc).into_iter().enumerate() {
        let normal = doc
            .get_dictionary(id)
            .unwrap()
            .get(b"AP")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"N")
            .unwrap();
        let mut appearance = resolve(&doc, normal).as_stream().unwrap().clone();
        let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>if index==0{"Courier"}else{"Times-Roman"}});
        let resources = appearance
            .dict
            .get_mut(b"Resources")
            .unwrap()
            .as_dict_mut()
            .unwrap();
        resources
            .get_mut(b"Font")
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("FC", font);
        let mut content = b"q BT /FC 8 Tf 0 g 5 38 Td (Static caption) Tj ET Q\n".to_vec();
        content.extend_from_slice(&appearance.content);
        appearance.set_plain_content(content);
        let normal = doc.add_object(appearance);
        doc.get_dictionary_mut(id)
            .unwrap()
            .set("AP", dictionary! {"N"=>normal});
    }
    let mut original = Vec::new();
    doc.save_to(&mut original).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("local-fonts.pdf");
    fs::write(&path, &original).unwrap();
    let e = engine();
    let source = e.open_document(&path).unwrap();
    let before = (0..2)
        .map(|p| {
            image::load_from_memory(&e.render_page(&source.id, p, 640).unwrap())
                .unwrap()
                .into_rgb8()
        })
        .collect::<Vec<_>>();
    let filled = apply(
        &e,
        &source,
        FormFieldValue::Text {
            text: "Changed answer".into(),
        },
    );
    // Static captions force isolated answer forms rather than the inline path.
    let mut current = filled.clone();
    let initial = e.source_bytes(&current.id).unwrap();
    let initial_objects = Document::load_mem(&initial).unwrap().objects.len();
    for round in 0..12 {
        let next = apply(
            &e,
            &current,
            FormFieldValue::Text {
                text: if round % 3 == 0 {
                    String::new()
                } else {
                    format!("Caption round {round:02}")
                },
            },
        );
        let bytes = e.source_bytes(&next.id).unwrap();
        assert!(
            Document::load_mem(&bytes).unwrap().objects.len() <= initial_objects + 6,
            "isolated answer graphs accumulate"
        );
        assert!(
            bytes.len() <= initial.len() + 8192,
            "isolated answer bytes accumulate"
        );
        if current.id != filled.id {
            e.close_document(&current.id).unwrap();
        }
        current = next;
    }
    let saved = export(&e, &current, &temp.path().join("saved.pdf"));
    e.close_document(&current.id).unwrap();
    for (page, before) in before.iter().enumerate() {
        let after = image::load_from_memory(&e.render_page(&saved.id, page, 640).unwrap())
            .unwrap()
            .into_rgb8();
        let mut ink = 0;
        for y in 150..174 {
            for x in 45..260 {
                let p = before.get_pixel(x, y);
                if p.0[0] < 200 {
                    ink += 1;
                }
                assert_eq!(after.get_pixel(x, y), p, "caption changed on page{page}");
            }
        }
        assert!(ink > 20, "caption pixel oracle was blank");
    }
    assert_red_pixels(&e, &saved, 2, true);
    for id in [&source.id, &filled.id, &saved.id] {
        e.close_document(id).unwrap();
    }
}


#[test]
fn unmarked_answer_keeps_inherited_font_context_through_saved_reedit() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for mode in ["local", "absent", "empty"] {
        let mut doc = Document::load_mem(&fixture(Kind::Text, 1, false)).unwrap();
        let widget = widgets(&doc)[0];
        let ap = doc.get_dictionary(widget).unwrap().get(b"AP").unwrap().as_dict().unwrap()
            .get(b"N").unwrap().as_reference().unwrap();
        let resources = doc.get_object(ap).unwrap().as_stream().unwrap().dict.get(b"Resources").unwrap().clone();
        for page in doc.get_pages().values() { doc.get_dictionary_mut(*page).unwrap().set("Resources", resources.clone()); }
        let stream = doc.get_object_mut(ap).unwrap().as_stream_mut().unwrap();
        let program = String::from_utf8(stream.get_plain_content().unwrap()).unwrap()
            .replace("/Tx BMC ", "").replace(" EMC", "");
        let parsed = Content::decode(program.as_bytes()).unwrap();
        assert!(!parsed.operations.iter().any(|op| matches!(op.operator.as_str(), "BMC" | "BDC")),
            "unmarked control was accidentally marked");
        stream.set_plain_content(program.into_bytes());
        match mode {
            "absent" => { stream.dict.remove(b"Resources"); },
            "empty" => stream.dict.set("Resources", dictionary!{}),
            _ => {},
        }
        let mut original = Vec::new(); doc.save_to(&mut original).unwrap();
        let path = temp.path().join(format!("unmarked-{mode}.pdf")); fs::write(&path, &original).unwrap();
        let source = e.open_document(&path).unwrap();
        assert_red_pixels(&e, &source, 1, false);
        let filled = apply(&e, &source, FormFieldValue::Text { text: "New answer".into() });
        let saved = export(&e, &filled, &temp.path().join(format!("saved-{mode}.pdf")));
        let again = apply(&e, &saved, FormFieldValue::Text { text: "Again".into() });
        for (document, expected) in [(&filled, "New answer"), (&saved, "New answer"), (&again, "Again")] {
            assert_eq!(e.list_form_fields(&document.id, 0).unwrap()[0].value,
                FormFieldValue::Text { text: expected.into() });
            assert_red_pixels(&e, document, 1, false);
        }
        assert_eq!(fs::read(path).unwrap(), original);
        assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(), original.as_slice());
        for document in [&again, &saved, &filled, &source] { e.close_document(&document.id).unwrap(); }
    }
}

#[test]
fn inherited_nested_artwork_and_local_category_precedence_survive_saved_reedits() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for mode in ["nested", "local", "default-color"] {
        let mut doc = Document::load_mem(&fixture(Kind::Text, 1, false)).unwrap();
        let widget = widgets(&doc)[0];
        let ap = doc.get_dictionary(widget).unwrap().get(b"AP").unwrap().as_dict().unwrap()
            .get(b"N").unwrap().as_reference().unwrap();
        let mut resources = doc.get_object(ap).unwrap().as_stream().unwrap().dict
            .get(b"Resources").unwrap().as_dict().unwrap().clone();
        let logo = doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form",
            "BBox"=>vec![0.into(),0.into(),280.into(),50.into()]},
            b"q 1 0 0 rg 250 20 5 5 re f Q".to_vec()));
        let nested = doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form",
            "BBox"=>vec![0.into(),0.into(),280.into(),50.into()]},b"/Inner Do".to_vec()));
        resources.set("XObject", dictionary!{"Logo"=>if mode == "nested" { nested } else { logo }, "Inner"=>logo});
        if mode == "default-color" {
            resources.set("ColorSpace", dictionary!{"DefaultRGB"=>vec![Object::Name(b"CalRGB".to_vec()),
                dictionary!{"WhitePoint"=>vec![Object::Real(0.9505),1.into(),Object::Real(1.089)]}.into()]});
        }
        let page = *doc.get_pages().values().next().unwrap();
        doc.get_dictionary_mut(page).unwrap().set("Resources", resources.clone());
        let stream = doc.get_object_mut(ap).unwrap().as_stream_mut().unwrap();
        stream.dict.remove(b"Resources");
        stream.set_plain_content(b"/Tx BMC q BT /F1 12 Tf 0.5 20.836 Td (Edited answer) Tj ET Q EMC /Logo Do".to_vec());
        if mode == "local" {
            stream.dict.set("Resources", resources.clone());
            let wrong = doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form",
                "BBox"=>vec![0.into(),0.into(),280.into(),50.into()]},b"q 0 1 0 rg 250 20 5 5 re f Q".to_vec()));
            resources.set("XObject", dictionary!{"Logo"=>wrong});
            doc.get_dictionary_mut(page).unwrap().set("Resources", resources);
        }
        let mut original = Vec::new(); doc.save_to(&mut original).unwrap();
        let path = temp.path().join(format!("context-{mode}.pdf")); fs::write(&path,&original).unwrap();
        let source = e.open_document(&path).unwrap();
        let before = image::load_from_memory(&e.render_page(&source.id,0,640).unwrap()).unwrap().to_rgb8();
        let logo_pixel = before.get_pixel(545,195).0;
        assert!(logo_pixel[0] > 150 && logo_pixel[1] < 80, "{mode}: source logo missing or wrong {logo_pixel:?}");
        let mut current = source.clone();
        let mut baseline = None;
        for round in 0..6 {
            let wanted = format!("Round {round:02}");
            let next = apply(&e,&current,FormFieldValue::Text{text:wanted.clone()});
            let saved = export(&e,&next,&temp.path().join(format!("{mode}-{round}.pdf")));
            let bytes = e.source_bytes(&saved.id).unwrap();
            let graph = Document::load_mem(&bytes).unwrap();
            if let Some((objects,size)) = baseline {
                assert!(graph.objects.len() <= objects+3, "{mode}: unbounded object growth");
                assert!(bytes.len() <= size+4096, "{mode}: unbounded byte growth");
            } else { baseline=Some((graph.objects.len(),bytes.len())); }
            assert_eq!(e.list_form_fields(&saved.id,0).unwrap()[0].value,FormFieldValue::Text{text:wanted});
            let after = image::load_from_memory(&e.render_page(&saved.id,0,640).unwrap()).unwrap().to_rgb8();
            assert_eq!(after.dimensions(),before.dimensions());
            for (x,y,pixel) in before.enumerate_pixels() {
                // Native-generated answer may move within its field, but it must
                // not change the separate logo or paint outside that field.
                let outside = x < 38 || x >= 602 || y < 138 || y >= 242;
                let logo = (541..549).contains(&x) && (191..199).contains(&y);
                if outside || logo { assert_eq!(after.get_pixel(x,y),pixel,"{mode}: unrelated pixel {x},{y}"); }
            }
            e.close_document(&next.id).unwrap();
            if current.id != source.id { e.close_document(&current.id).unwrap(); }
            current=saved;
        }
        assert_eq!(fs::read(path).unwrap(),original);
        assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),original.as_slice());
        e.close_document(&current.id).unwrap(); e.close_document(&source.id).unwrap();
    }
}

fn shared_widget_page_context_case(mode: &str, should_fill: bool) {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut doc = Document::load_mem(&fixture(Kind::Text, 1, false)).unwrap();
    let first = *doc.get_pages().values().next().unwrap();
    let tree = doc.get_dictionary(first).unwrap().get(b"Parent").unwrap().as_reference().unwrap();
    let widget = widgets(&doc)[0];
    let ap = doc.get_dictionary(widget).unwrap().get(b"AP").unwrap().as_dict().unwrap()
        .get(b"N").unwrap().as_reference().unwrap();
    doc.get_dictionary_mut(widget).unwrap().remove(b"P");
    let second = doc.add_object(doc.get_dictionary(first).unwrap().clone());
    doc.get_dictionary_mut(tree).unwrap().set("Kids",vec![Object::Reference(first),Object::Reference(second)]);
    doc.get_dictionary_mut(tree).unwrap().set("Count",2);
    for (index,page) in [first,second].into_iter().enumerate() {
        let state = doc.add_object(dictionary!{"Type"=>"ExtGState", "ca"=>Object::Real(if index==0 {1.} else {0.5})});
        doc.get_dictionary_mut(page).unwrap().set("Resources",dictionary!{"ExtGState"=>dictionary!{"Ink"=>state}});
    }
    let old = doc.get_object(ap).unwrap().as_stream().unwrap().get_plain_content().unwrap();
    if mode == "unmarked" {
        doc.get_object_mut(ap).unwrap().as_stream_mut().unwrap().set_plain_content(
            String::from_utf8(old).unwrap().replace("/Tx BMC ","").replace(" EMC","").into_bytes());
    } else {
        let program = if mode == "nested-consumed" { b"/Ink gs q 1 0 0 rg 250 20 5 5 re f Q".as_slice() }
                      else { b"q 1 0 0 rg 250 20 5 5 re f Q".as_slice() };
        let logo = doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form",
            "BBox"=>vec![0.into(),0.into(),280.into(),50.into()]}, program.to_vec()));
        let stream = doc.get_object_mut(ap).unwrap().as_stream_mut().unwrap();
        stream.dict.get_mut(b"Resources").unwrap().as_dict_mut().unwrap().set("XObject",dictionary!{"Logo"=>logo});
        stream.set_plain_content(b"/Tx BMC q BT /F1 12 Tf 0.5 20.836 Td (Edited answer) Tj ET Q EMC /Logo Do".to_vec());
    }
    assert_eq!(widgets(&doc),vec![widget,widget]);
    let mut original=Vec::new();doc.save_to(&mut original).unwrap();
    let path=temp.path().join(format!("alias-{mode}.pdf"));fs::write(&path,&original).unwrap();
    let source=e.open_document(&path).unwrap();
    let before:Vec<_>=(0..2).map(|p|e.render_page(&source.id,p,640).unwrap()).collect();
    let field=e.list_form_fields(&source.id,0).unwrap().remove(0);
    let result=e.fill_form_field(&source.id,FillFormFieldRequest{page_index:0,annotation_index:field.annotation_index,
        expected_value:field.value,value:FormFieldValue::Text{text:"Shared answer".into()}});
    if should_fill {
        let filled=result.expect("unused page resources must not block a resource-local alias");
        let saved=export(&e,&filled,&temp.path().join("saved.pdf"));
        let again=apply(&e,&saved,FormFieldValue::Text{text:"Shared again".into()});
        for (info,text) in [(&filled,"Shared answer"),(&saved,"Shared answer"),(&again,"Shared again")] {
            let graph=Document::load_mem(&e.source_bytes(&info.id).unwrap()).unwrap();
            let aliases=widgets(&graph);assert_eq!(aliases[0],aliases[1],"widget alias was split");
            for page in 0..2 {
                assert_eq!(e.list_form_fields(&info.id,page).unwrap()[0].value,FormFieldValue::Text{text:text.into()});
                let old=image::load_from_memory(&before[page]).unwrap().to_rgb8();
                let new=image::load_from_memory(&e.render_page(&info.id,page,640).unwrap()).unwrap().to_rgb8();
                assert!(old.get_pixel(545,195).0[0]>240 && old.get_pixel(545,195).0[1]<15);
                for (x,y,pixel) in old.enumerate_pixels() {
                    if x<38||x>=602||y<138||y>=242||((541..549).contains(&x)&&(191..199).contains(&y)) {
                        assert_eq!(new.get_pixel(x,y),pixel,"{mode}: retained pixel {page}/{x},{y}");
                    }
                }
            }
        }
        for info in [&again,&saved,&filled] {e.close_document(&info.id).unwrap();}
    } else {
        let error=result.unwrap_err().to_string();
        assert!(error.contains("conflicting inherited appearance resource contexts"),"{error}");
    }
    assert_eq!(fs::read(&path).unwrap(),original);
    assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),original.as_slice());
    for page in 0..2 {assert_eq!(e.render_page(&source.id,page,640).unwrap(),before[page]);}
    e.close_document(&source.id).unwrap();
}

#[test]
fn shared_widget_local_unmarked_font_ignores_unused_page_states() {
    shared_widget_page_context_case("unmarked",true);
}
#[test]
fn shared_widget_local_logo_ignores_unused_page_states() {
    shared_widget_page_context_case("logo",true);
}
#[test]
fn shared_widget_nested_consumed_page_states_still_require_consistency() {
    shared_widget_page_context_case("nested-consumed",false);
}
