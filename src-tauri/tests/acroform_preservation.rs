use folio_engine::{ExportRequest, PagePlan, PdfEngine};
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};
use std::{collections::HashSet, fs, path::PathBuf};

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
fn plans(id: &str, order: &[usize]) -> Vec<PagePlan> {
    order
        .iter()
        .enumerate()
        .map(|(i, index)| PagePlan {
            crop: None,
            id: format!("p{i}"),
            source_id: id.into(),
            page_index: *index,
            width: 300.,
            height: 400.,
            rotation: 0,
            overlays: vec![],
        })
        .collect()
}
fn form(d: &Document) -> &Dictionary {
    d.catalog()
        .unwrap()
        .get(b"AcroForm")
        .map(|o| d.dereference(o).unwrap().1.as_dict().unwrap())
        .expect("export must retain AcroForm")
}
fn fields(d: &Document) -> Vec<(String, ObjectId)> {
    fn visit(
        d: &Document,
        id: ObjectId,
        prefix: String,
        out: &mut Vec<(String, ObjectId)>,
        seen: &mut HashSet<ObjectId>,
    ) {
        assert!(seen.insert(id), "field appears twice");
        let node = d.get_dictionary(id).unwrap();
        let name = node
            .get(b"T")
            .ok()
            .map(|o| lopdf::decode_text_string(o).unwrap());
        let full = match name {
            Some(n) if prefix.is_empty() => n,
            Some(n) => format!("{prefix}.{n}"),
            None => prefix,
        };
        if node.has(b"FT") {
            out.push((full.clone(), id));
        }
        if let Ok(kids) = node.get(b"Kids") {
            for child in kids.as_array().unwrap() {
                let child = child.as_reference().unwrap();
                assert_eq!(
                    d.get_dictionary(child)
                        .unwrap()
                        .get(b"Parent")
                        .unwrap()
                        .as_reference()
                        .unwrap(),
                    id
                );
                visit(d, child, full.clone(), out, seen);
            }
        }
    }
    let mut out = vec![];
    let mut seen = HashSet::new();
    for root in form(d).get(b"Fields").unwrap().as_array().unwrap() {
        visit(
            d,
            root.as_reference().unwrap(),
            String::new(),
            &mut out,
            &mut seen,
        );
    }
    out
}

#[test]
fn preserves_shared_fields_values_and_widget_links_through_page_operations() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.pdf");
    let original = fixture();
    fs::write(&source, &original).unwrap();
    let engine = engine();
    let info = engine.open_document(&source).unwrap();
    for (label, order, expected_fields, expected_widgets) in [
        ("all", vec![2, 0, 1], 3, 4),
        ("subset", vec![1], 1, 1),
        ("duplicate", vec![0, 0, 1], 2, 5),
    ] {
        let path = temp.path().join(format!("{label}.pdf"));
        engine
            .export_pdf(
                ExportRequest {
                    pages: plans(&info.id, &order),
                    flatten: false,
                },
                &path,
            )
            .unwrap();
        let d = Document::load(&path).unwrap();
        let f = fields(&d);
        assert_eq!(f.len(), expected_fields);
        assert!(f.iter().any(|(n, _)| n == "person.name"));
        let named = d
            .get_dictionary(f.iter().find(|(n, _)| n == "person.name").unwrap().1)
            .unwrap();
        assert_eq!(
            lopdf::decode_text_string(named.get(b"V").unwrap()).unwrap(),
            "Original name"
        );
        assert_eq!(named.get(b"MaxLen").unwrap().as_i64().unwrap(), 80);
        let mut seen = HashSet::new();
        for page_id in d.get_pages().values() {
            let page = d.get_dictionary(*page_id).unwrap();
            for w in page.get(b"Annots").unwrap().as_array().unwrap() {
                let id = w.as_reference().unwrap();
                assert!(seen.insert(id), "duplicate pages need distinct widgets");
                let w = d.get_dictionary(id).unwrap();
                assert_eq!(w.get(b"P").unwrap().as_reference().unwrap(), *page_id);
                assert!(w.has(b"Parent"));
                assert!(w.has(b"AP"));
            }
        }
        assert_eq!(seen.len(), expected_widgets);
        assert_eq!(form(&d).get(b"CO").unwrap().as_array().unwrap().len(), 1);
        assert!(form(&d).has(b"DR"));
    }
    assert_eq!(fs::read(source).unwrap(), original);
}

fn mutate_source(change: impl FnOnce(&mut Document)) -> Vec<u8> {
    let mut d = Document::load_mem(&fixture()).unwrap();
    change(&mut d);
    let mut bytes = vec![];
    d.save_to(&mut bytes).unwrap();
    bytes
}

#[test]
fn cropping_one_occurrence_preserves_shared_fields_and_hidden_widgets() {
    let temp=tempfile::tempdir().unwrap();let path=temp.path().join("forms.pdf");let original=fixture();fs::write(&path,&original).unwrap();
    let engine=engine();let source=engine.open_document(&path).unwrap();let mut pages=plans(&source.id,&[0,0,1]);
    pages[0].crop=Some(folio_engine::AnnotationRect{x:100.,y:20.,width:100.,height:100.});
    let output=temp.path().join("cropped-forms.pdf");engine.export_pdf(ExportRequest{pages,flatten:false},&output).unwrap();
    let pdf=Document::load(&output).unwrap();assert_eq!(fields(&pdf).len(),2);
    let name=fields(&pdf).into_iter().find(|(name,_)|name=="person.name").unwrap().1;
    assert_eq!(pdf.get_dictionary(name).unwrap().get(b"Kids").unwrap().as_array().unwrap().len(),3);
    for page in pdf.get_pages().values(){for widget in pdf.get_dictionary(*page).unwrap().get(b"Annots").unwrap().as_array().unwrap(){let w=pdf.get_dictionary(widget.as_reference().unwrap()).unwrap();assert_eq!(w.get(b"P").unwrap().as_reference().unwrap(),*page);assert!(w.has(b"AP"));}}
    let opened=engine.open_document(&output).unwrap();assert_eq!((opened.pages[0].width,opened.pages[0].height),(100.,100.));assert_eq!((opened.pages[1].width,opened.pages[1].height),(300.,400.));
    assert_eq!(fs::read(&path).unwrap(),original);engine.close_document(&opened.id).unwrap();engine.close_document(&source.id).unwrap();
}
fn form_id(d: &Document) -> ObjectId {
    d.catalog()
        .unwrap()
        .get(b"AcroForm")
        .unwrap()
        .as_reference()
        .unwrap()
}

#[test]
fn combines_forms_without_aliasing_names_fonts_or_widgets() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    let mut combined = vec![];
    for i in 0..2 {
        let source = temp.path().join(format!("source{i}.pdf"));
        let bytes = mutate_source(|d| {
            if i == 1 {
                let fonts = form(d)
                    .get(b"DR")
                    .unwrap()
                    .as_dict()
                    .unwrap()
                    .get(b"Font")
                    .unwrap()
                    .as_dict()
                    .unwrap();
                let font = fonts.get(b"F1").unwrap().as_reference().unwrap();
                d.get_dictionary_mut(font)
                    .unwrap()
                    .set("BaseFont", "Courier");
            }
        });
        fs::write(&source, bytes).unwrap();
        let info = engine.open_document(&source).unwrap();
        let mut p = plans(&info.id, &[0]);
        p[0].id = format!("p{i}");
        combined.extend(p);
    }
    let out = temp.path().join("combined.pdf");
    engine
        .export_pdf(
            ExportRequest {
                pages: combined,
                flatten: false,
            },
            &out,
        )
        .unwrap();
    let d = Document::load(out).unwrap();
    let names = fields(&d)
        .into_iter()
        .map(|(n, _)| n)
        .collect::<HashSet<_>>();
    assert_eq!(
        names,
        HashSet::from([
            "person.name".into(),
            "consent".into(),
            "person_2.name".into(),
            "consent_2".into()
        ])
    );
    let fonts = form(&d)
        .get(b"DR")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Font")
        .unwrap()
        .as_dict()
        .unwrap();
    assert_eq!(fonts.len(), 2);
    let roots = form(&d).get(b"Fields").unwrap().as_array().unwrap();
    let second_person = d.get_dictionary(roots[2].as_reference().unwrap()).unwrap();
    let da = second_person.get(b"DA").unwrap().as_str().unwrap();
    assert!(String::from_utf8_lossy(da).contains("/Folio2_F1"));
}

#[test]
fn refuses_unpreservable_forms_without_publishing_or_overwriting() {
    for case in ["cycle", "orphan", "xfa", "signed"] {
        let bytes = mutate_source(|d| {
            let form = form_id(d);
            let first = d
                .get_dictionary(form)
                .unwrap()
                .get(b"Fields")
                .unwrap()
                .as_array()
                .unwrap()[0]
                .as_reference()
                .unwrap();
            match case {
                "cycle" => {
                    d.get_dictionary_mut(first)
                        .unwrap()
                        .set("Kids", vec![Object::Reference(first)]);
                }
                "orphan" => {
                    d.get_dictionary_mut(form)
                        .unwrap()
                        .set("Fields", Vec::<Object>::new());
                }
                "xfa" => {
                    d.get_dictionary_mut(form)
                        .unwrap()
                        .set("XFA", Object::string_literal("<xfa/>"));
                }
                _ => {
                    d.get_dictionary_mut(first).unwrap().set("FT", "Sig");
                    d.get_dictionary_mut(first).unwrap().set(
                        "V",
                        dictionary! {"Type"=>"Sig","Contents"=>Object::string_literal("signed")},
                    );
                }
            }
        });
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.pdf");
        fs::write(&source, &bytes).unwrap();
        let engine = engine();
        let info = engine.open_document(&source).unwrap();
        let target = temp.path().join("existing.pdf");
        fs::write(&target, b"KEEP EXISTING").unwrap();
        let result = engine.export_pdf(
            ExportRequest {
                pages: plans(&info.id, &[0]),
                flatten: false,
            },
            &target,
        );
        assert!(result.is_err(), "case {case}");
        assert_eq!(fs::read(&target).unwrap(), b"KEEP EXISTING");
        assert_eq!(fs::read(source).unwrap(), bytes);
    }
}

#[test]
fn source_text_editing_refuses_to_separate_cross_page_shared_fields() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.pdf");
    fs::write(&source, fixture()).unwrap();
    let engine = engine();
    let info = engine.open_document(&source).unwrap();
    let result = engine.list_text_runs(&info.id, 0);
    assert!(
        result.is_err(),
        "shared field page must not become an isolated source"
    );
    assert!(result.unwrap_err().to_string().contains("shared"));
}

#[test]
fn source_text_editing_retains_independent_page_fields() {
    let bytes = mutate_source(|d| {
        let font =
            d.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
        let content = d.add_object(Stream::new(
            dictionary! {},
            b"BT /F1 12 Tf 40 350 Td (Editable text) Tj ET".to_vec(),
        ));
        let page = *d.get_pages().values().nth(2).unwrap();
        let page = d.get_dictionary_mut(page).unwrap();
        page.set("Resources", dictionary! {"Font"=>dictionary!{"F1"=>font}});
        page.set("Contents", content);
    });
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.pdf");
    fs::write(&source, bytes).unwrap();
    let engine = engine();
    let info = engine.open_document(&source).unwrap();
    let runs = engine.list_text_runs(&info.id, 2).unwrap();
    let run = runs
        .runs
        .iter()
        .find(|r| r.text == "Editable text")
        .unwrap();
    assert!(run.supported, "{:?}", run.reason);
    let changed = engine
        .replace_text(
            &info.id,
            2,
            run.object_index,
            "Editable text",
            "Edited text",
        )
        .unwrap();
    let out = temp.path().join("edited.pdf");
    engine
        .export_pdf(
            ExportRequest {
                pages: plans(&changed.id, &[0]),
                flatten: false,
            },
            &out,
        )
        .unwrap();
    let d = Document::load(out).unwrap();
    assert_eq!(
        fields(&d)
            .iter()
            .map(|(n, _)| n.as_str())
            .collect::<Vec<_>>(),
        vec!["choice"]
    );
}

#[test]
fn indirect_widget_types_and_inherited_signature_types_are_not_lost() {
    for signed in [false, true] {
        let bytes = mutate_source(|d| {
            let root = form(d).get(b"Fields").unwrap().as_array().unwrap()[0]
                .as_reference()
                .unwrap();
            let child = d
                .get_dictionary(root)
                .unwrap()
                .get(b"Kids")
                .unwrap()
                .as_array()
                .unwrap()[0]
                .as_reference()
                .unwrap();
            if signed {
                d.get_dictionary_mut(root).unwrap().set("FT", "Sig");
                let child = d.get_dictionary_mut(child).unwrap();
                child.remove(b"FT");
                child.set(
                    "V",
                    dictionary! {"Type"=>"Sig","Contents"=>Object::string_literal("signed")},
                );
            } else {
                let subtype = d.add_object(Object::Name(b"Widget".to_vec()));
                let page = *d.get_pages().values().next().unwrap();
                let widget = d
                    .get_dictionary(page)
                    .unwrap()
                    .get(b"Annots")
                    .unwrap()
                    .as_array()
                    .unwrap()[0]
                    .as_reference()
                    .unwrap();
                d.get_dictionary_mut(widget)
                    .unwrap()
                    .set("Subtype", subtype);
            }
        });
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.pdf");
        fs::write(&source, bytes).unwrap();
        let engine = engine();
        let info = engine.open_document(&source).unwrap();
        let target = temp.path().join("out.pdf");
        let result = engine.export_pdf(
            ExportRequest {
                pages: plans(&info.id, &[0]),
                flatten: false,
            },
            &target,
        );
        if signed {
            assert!(
                result.is_err(),
                "inherited signed type must block rewriting"
            );
            assert!(!target.exists());
        } else {
            result.unwrap();
            let d = Document::load(target).unwrap();
            let page = *d.get_pages().values().next().unwrap();
            let widget_id = d
                .get_dictionary(page)
                .unwrap()
                .get(b"Annots")
                .unwrap()
                .as_array()
                .unwrap()[0]
                .as_reference()
                .unwrap();
            let widget = d.get_dictionary(widget_id).unwrap();
            assert!(widget.has(b"Parent"));
            assert_eq!(widget.get(b"P").unwrap().as_reference().unwrap(), page);
            let all_fields = fields(&d);
            let field = d
                .get_dictionary(
                    all_fields
                        .iter()
                        .find(|(n, _)| n == "person.name")
                        .unwrap()
                        .1,
                )
                .unwrap();
            assert_eq!(
                field.get(b"Kids").unwrap().as_array().unwrap()[0]
                    .as_reference()
                    .unwrap(),
                widget_id,
                "page widget must be the field tree widget"
            );
        }
    }
}

#[test]
fn preserves_radio_options_states_and_shared_group_after_duplication() {
    let bytes = mutate_source(|d| {
        let root = form(d).get(b"Fields").unwrap().as_array().unwrap()[0]
            .as_reference()
            .unwrap();
        let group = d
            .get_dictionary(root)
            .unwrap()
            .get(b"Kids")
            .unwrap()
            .as_array()
            .unwrap()[0]
            .as_reference()
            .unwrap();
        let children = d
            .get_dictionary(group)
            .unwrap()
            .get(b"Kids")
            .unwrap()
            .as_array()
            .unwrap()
            .clone();
        let field = d.get_dictionary_mut(group).unwrap();
        field.set("FT", "Btn");
        field.set("Ff", 32768);
        field.set("V", "One");
        field.set(
            "Opt",
            vec![
                Object::string_literal("First"),
                Object::string_literal("Second"),
            ],
        );
        for (i, child) in children.iter().enumerate() {
            let id = child.as_reference().unwrap();
            let appearance = d
                .get_dictionary(id)
                .unwrap()
                .get(b"AP")
                .unwrap()
                .as_dict()
                .unwrap()
                .get(b"N")
                .unwrap()
                .clone();
            let data = d.get_dictionary_mut(id).unwrap();
            data.set("AP",dictionary!{"N"=>dictionary!{"Off"=>appearance.clone(),if i==0 {"One"} else {"Two"}=>appearance}});
            data.set("AS", if i == 0 { "One" } else { "Off" });
        }
    });
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("radio.pdf");
    fs::write(&source, bytes).unwrap();
    let engine = engine();
    let info = engine.open_document(&source).unwrap();
    let target = temp.path().join("out.pdf");
    engine
        .export_pdf(
            ExportRequest {
                pages: plans(&info.id, &[0, 1, 0]),
                flatten: false,
            },
            &target,
        )
        .unwrap();
    let d = Document::load(target).unwrap();
    let fields = fields(&d);
    let group = d
        .get_dictionary(fields.iter().find(|(n, _)| n == "person.name").unwrap().1)
        .unwrap();
    assert_eq!(group.get(b"V").unwrap().as_name().unwrap(), b"One");
    assert_eq!(group.get(b"Kids").unwrap().as_array().unwrap().len(), 3);
    assert_eq!(
        group
            .get(b"Opt")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|o| lopdf::decode_text_string(o).unwrap())
            .collect::<Vec<_>>(),
        vec!["First", "First", "Second"]
    );
}

#[test]
fn blocks_scripted_field_name_collisions_and_bounds_form_graphs() {
    for case in ["script", "nodes", "resources"] {
        let bytes = mutate_source(|d| {
            let form = form_id(d);
            match case {
                "script" => {
                    let first = d
                        .get_dictionary(form)
                        .unwrap()
                        .get(b"Fields")
                        .unwrap()
                        .as_array()
                        .unwrap()[0]
                        .as_reference()
                        .unwrap();
                    d.get_dictionary_mut(first).unwrap().set("AA",dictionary!{"C"=>dictionary!{"S"=>"JavaScript","JS"=>Object::string_literal("this.getField('person.name').value;")}});
                }
                "nodes" => {
                    let mut fields = d
                        .get_dictionary(form)
                        .unwrap()
                        .get(b"Fields")
                        .unwrap()
                        .as_array()
                        .unwrap()
                        .clone();
                    for i in 0..4001 {
                        fields.push(Object::Reference(d.add_object(dictionary!{"FT"=>"Tx","T"=>Object::string_literal(format!("hidden{i}"))})));
                    }
                    d.get_dictionary_mut(form).unwrap().set("Fields", fields);
                }
                _ => {
                    let page = *d.get_pages().values().next().unwrap();
                    let widget = d
                        .get_dictionary(page)
                        .unwrap()
                        .get(b"Annots")
                        .unwrap()
                        .as_array()
                        .unwrap()[0]
                        .as_reference()
                        .unwrap();
                    let ap = d
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
                    let stream = d.get_object_mut(ap).unwrap().as_stream_mut().unwrap();
                    stream.dict.set(
                        "Resources",
                        dictionary! {"XObject"=>dictionary!{"Loop"=>ap}},
                    );
                }
            }
        });
        let temp = tempfile::tempdir().unwrap();
        let engine = engine();
        let mut pages = vec![];
        for i in 0..if case == "script" { 2 } else { 1 } {
            let path = temp.path().join(format!("source{i}.pdf"));
            fs::write(&path, &bytes).unwrap();
            let info = engine.open_document(&path).unwrap();
            let mut p = plans(&info.id, &[0]);
            p[0].id = format!("p{i}");
            pages.extend(p);
        }
        let out = temp.path().join("out.pdf");
        assert!(
            engine
                .export_pdf(
                    ExportRequest {
                        pages,
                        flatten: false
                    },
                    &out
                )
                .is_err(),
            "{case}"
        );
        assert!(!out.exists());
    }
}

#[test]
fn retains_procset_catalog_scripts_and_annotation_hide_targets() {
    let bytes = mutate_source(|d| {
        let form = form_id(d);
        d.get_dictionary_mut(form)
            .unwrap()
            .get_mut(b"DR")
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set(
                "ProcSet",
                vec![
                    Object::Name(b"PDF".to_vec()),
                    Object::Name(b"Text".to_vec()),
                ],
            );
        let script=d.add_object(dictionary!{"S"=>"JavaScript","JS"=>Object::string_literal("function helper() { return 42; }")});
        d.catalog_mut().unwrap().set("Names",dictionary!{"JavaScript"=>dictionary!{"Names"=>vec![Object::string_literal("helpers"),Object::Reference(script)]}});
        let roots = d
            .get_dictionary(form)
            .unwrap()
            .get(b"Fields")
            .unwrap()
            .as_array()
            .unwrap()
            .clone();
        let merged = roots[1].as_reference().unwrap();
        let parent = roots[0].as_reference().unwrap();
        d.get_dictionary_mut(parent).unwrap().set("AA",dictionary!{"C"=>dictionary!{"S"=>"JavaScript","JS"=>Object::string_literal("event.value = helper();")}});
        d.get_dictionary_mut(merged)
            .unwrap()
            .set("A", dictionary! {"S"=>"Hide","T"=>merged});
    });
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.pdf");
    fs::write(&source, bytes).unwrap();
    let engine = engine();
    let info = engine.open_document(&source).unwrap();
    let out = temp.path().join("out.pdf");
    engine
        .export_pdf(
            ExportRequest {
                pages: plans(&info.id, &[0]),
                flatten: false,
            },
            &out,
        )
        .unwrap();
    let d = Document::load(out).unwrap();
    assert_eq!(
        form(&d)
            .get(b"DR")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"ProcSet")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let names = d
        .catalog()
        .unwrap()
        .get(b"Names")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"JavaScript")
        .unwrap();
    let scripts = d
        .dereference(names)
        .unwrap()
        .1
        .as_dict()
        .unwrap()
        .get(b"Names")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(scripts.len(), 2);
    let action = d.dereference(&scripts[1]).unwrap().1.as_dict().unwrap();
    assert!(lopdf::decode_text_string(action.get(b"JS").unwrap())
        .unwrap()
        .contains("helper"));
    let page = *d.get_pages().values().next().unwrap();
    let checkbox = d
        .get_dictionary(page)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()[1]
        .as_reference()
        .unwrap();
    let a = d
        .get_dictionary(checkbox)
        .unwrap()
        .get(b"A")
        .unwrap()
        .as_dict()
        .unwrap();
    assert_eq!(
        a.get(b"T").unwrap().as_reference().unwrap(),
        checkbox,
        "Hide target is the widget, not its field parent"
    );
}

#[test]
fn unnamed_roots_keep_distinct_values_when_combined_and_name_actions_cannot_redirect() {
    for action in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let engine = engine();
        let mut pages = vec![];
        for index in 0..2 {
            let bytes = mutate_source(|d| {
                let roots = form(d).get(b"Fields").unwrap().as_array().unwrap().clone();
                let root = roots[0].as_reference().unwrap();
                d.get_dictionary_mut(root).unwrap().remove(b"T");
                let child = d
                    .get_dictionary(root)
                    .unwrap()
                    .get(b"Kids")
                    .unwrap()
                    .as_array()
                    .unwrap()[0]
                    .as_reference()
                    .unwrap();
                d.get_dictionary_mut(child)
                    .unwrap()
                    .set("V", Object::string_literal(format!("Value {index}")));
                if action {
                    let button = roots[1].as_reference().unwrap();
                    d.get_dictionary_mut(button).unwrap().set("A",dictionary!{"S"=>"ResetForm","Fields"=>vec![Object::string_literal("name")]});
                }
            });
            let source = temp.path().join(format!("source{index}.pdf"));
            fs::write(&source, bytes).unwrap();
            let info = engine.open_document(&source).unwrap();
            let mut p = plans(&info.id, &[0]);
            p[0].id = format!("p{index}");
            pages.extend(p);
        }
        let out = temp.path().join("out.pdf");
        let result = engine.export_pdf(
            ExportRequest {
                pages,
                flatten: false,
            },
            &out,
        );
        if action {
            assert!(
                result.is_err(),
                "name-based actions must not silently redirect"
            );
            assert!(!out.exists());
        } else {
            result.unwrap();
            let d = Document::load(out).unwrap();
            let fields = fields(&d);
            let names = fields.iter().map(|(n, _)| n).collect::<HashSet<_>>();
            assert_eq!(
                names.len(),
                fields.len(),
                "effective names must remain distinct"
            );
            let values = fields
                .iter()
                .filter_map(|(_, id)| d.get_dictionary(*id).unwrap().get(b"V").ok())
                .filter_map(|o| lopdf::decode_text_string(o).ok())
                .collect::<HashSet<_>>();
            assert!(values.contains("Value 0") && values.contains("Value 1"));
        }
    }
}

#[test]
fn unnamed_widgets_with_explicit_types_still_share_a_field() {
    let bytes = mutate_source(|d| {
        for page in d.get_pages().values().take(2).copied().collect::<Vec<_>>() {
            let widget = d
                .get_dictionary(page)
                .unwrap()
                .get(b"Annots")
                .unwrap()
                .as_array()
                .unwrap()[0]
                .as_reference()
                .unwrap();
            d.get_dictionary_mut(widget).unwrap().set("FT", "Tx");
        }
    });
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.pdf");
    fs::write(&source, bytes).unwrap();
    let engine = engine();
    let info = engine.open_document(&source).unwrap();
    assert!(
        engine.list_text_runs(&info.id, 0).is_err(),
        "unnamed widget types do not create independent identities"
    );
}

#[test]
fn form_navigation_resolves_named_targets_and_removes_omitted_page_actions() {
    for named in [false, true] {
        let bytes = mutate_source(|d| {
            let page = *d.get_pages().values().nth(2).unwrap();
            let destination =
                Object::Array(vec![Object::Reference(page), Object::Name(b"Fit".to_vec())]);
            let roots = form(d).get(b"Fields").unwrap().as_array().unwrap().clone();
            let button = roots[1].as_reference().unwrap();
            d.get_dictionary_mut(button).unwrap().set("A",dictionary!{"S"=>"GoTo","D"=>if named{Object::string_literal("last")}else{destination.clone()}});
            if named {
                d.catalog_mut().unwrap().set("Names",dictionary!{"Dests"=>dictionary!{"Names"=>vec![Object::string_literal("last"),destination]}});
            }
        });
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.pdf");
        fs::write(&source, bytes).unwrap();
        let engine = engine();
        let info = engine.open_document(&source).unwrap();
        for order in [vec![0], vec![2, 0]] {
            let out = temp.path().join(format!("out{}.pdf", order.len()));
            engine
                .export_pdf(
                    ExportRequest {
                        pages: plans(&info.id, &order),
                        flatten: false,
                    },
                    &out,
                )
                .unwrap();
            let d = Document::load(out).unwrap();
            let page = *d.get_pages().values().last().unwrap();
            let checkbox = d
                .get_dictionary(page)
                .unwrap()
                .get(b"Annots")
                .unwrap()
                .as_array()
                .unwrap()[1]
                .as_reference()
                .unwrap();
            let action = d.get_dictionary(checkbox).unwrap().get(b"A").unwrap();
            let action = d.dereference(action).unwrap().1;
            if order.len() == 1 {
                assert!(
                    matches!(action, Object::Null),
                    "an omitted destination must not remain a malformed action"
                );
            } else {
                let dest = action
                    .as_dict()
                    .unwrap()
                    .get(b"D")
                    .unwrap()
                    .as_array()
                    .unwrap();
                assert_eq!(
                    dest[0].as_reference().unwrap(),
                    *d.get_pages().values().next().unwrap()
                );
            }
        }
    }
}
