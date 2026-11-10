use folio_engine::{FillFormFieldRequest, FormFieldValue, PdfEngine};
use lopdf::{dictionary, Document, Object, Stream};
use std::{fs, path::PathBuf};
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
    PdfEngine::start(
        std::env::var_os("FOLIO_TEST_PDFIUM_LIBRARY")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")
            }),
    )
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

fn export(e: &PdfEngine, info: &folio_engine::DocumentInfo, path: &std::path::Path) {
    let pages=info.pages.iter().enumerate().map(|(i,p)|serde_json::from_value(serde_json::json!({"id":format!("p{i}"),"sourceId":info.id,"pageIndex":i,"width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})).unwrap()).collect();
    e.export_pdf(
        folio_engine::ExportRequest {
            pages,
            flatten: false,
        },
        path,
    )
    .unwrap();
}
fn normal<'a>(d: &'a Document, page: usize, index: usize) -> &'a Stream {
    let p = d
        .get_dictionary(*d.get_pages().values().nth(page).unwrap())
        .unwrap();
    let a = d
        .dereference(p.get(b"Annots").unwrap())
        .unwrap()
        .1
        .as_array()
        .unwrap();
    let w = d.dereference(&a[index]).unwrap().1.as_dict().unwrap();
    let ap = d
        .dereference(w.get(b"AP").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    d.dereference(ap.get(b"N").unwrap())
        .unwrap()
        .1
        .as_stream()
        .unwrap()
}
fn colors(doc: &Document,s: &Stream) -> Vec<Vec<f32>> {
    let mut out = vec![];
    paint_colors(doc,s,None,vec![0.,0.,0.],&mut out,0);out
}
fn paint_colors(doc:&Document,s:&Stream,inherited:Option<&lopdf::Dictionary>,mut color:Vec<f32>,out:&mut Vec<Vec<f32>>,depth:usize) {
    assert!(depth<64,"appearance recursion limit");
    let resources=s.dict.get(b"Resources").ok().map(|v|doc.dereference(v).unwrap().1.as_dict().unwrap()).or(inherited);
    let ops=lopdf::content::Content::decode(&s.get_plain_content_with_limit(4*1024*1024).unwrap()).unwrap();
    let mut stack=Vec::new();
    for op in ops.operations {
        match op.operator.as_str() {
            "q"=>stack.push(color.clone()),
            "Q"=>color=stack.pop().unwrap(),
            "Do"=>{
                let objects=doc.dereference(resources.unwrap().get(b"XObject").unwrap()).unwrap().1.as_dict().unwrap();
                let child=doc.dereference(objects.get(op.operands[0].as_name().unwrap()).unwrap()).unwrap().1.as_stream().unwrap();
                if child.dict.get(b"Subtype").unwrap().as_name().unwrap()==b"Form" {paint_colors(doc,child,resources,color.clone(),out,depth+1);}
            },
            "g" => color = vec![op.operands[0].as_float().unwrap(); 3],
            "rg" => color = op.operands.iter().map(|v| v.as_float().unwrap()).collect(),
            "k" => {
                let c: Vec<_> = op.operands.iter().map(|v| v.as_float().unwrap()).collect();
                color = (0..3).map(|i| 1. - (c[i] + c[3]).min(1.)).collect();
            }
            "Tj" | "TJ" => out.push(color.clone()),
            _ => {}
        }
    }
    assert!(stack.is_empty());
}
#[test]
fn ordered_default_colors_survive_ordinary_fill_and_shared_widgets() {
    let cases = [
        ("gray-rgb", "0 g 1 0 0 rg", vec![1., 0., 0.]),
        ("rgb-rgb", "0 1 0 rg 1 0 0 rg", vec![1., 0., 0.]),
        ("rgb-gray", "1 0 0 rg .5 g", vec![0.5, 0.5, 0.5]),
        ("gray-gray", "0 g .5 g", vec![0.5, 0.5, 0.5]),
        ("gray-cmyk", "0 g 0 1 1 0 k", vec![1., 0., 0.]),
        ("cmyk-rgb", "1 1 0 0 k 1 0 0 rg", vec![1., 0., 0.]),
    ];
    let mut failures = vec![];
    for (label, da, expected) in cases {
        let mut d = Document::load_mem(&fixture()).unwrap();
        let form = d
            .catalog()
            .unwrap()
            .get(b"AcroForm")
            .unwrap()
            .as_reference()
            .unwrap();
        let definition = format!("/F1 12 Tf {da}");
        d.get_dictionary_mut(form)
            .unwrap()
            .set("DA", Object::string_literal(definition.clone()));
        let mut bytes = vec![];
        d.save_to(&mut bytes).unwrap();
        let (temp, e, info) = open(&bytes);
        let field = e.list_form_fields(&info.id, 0).unwrap().remove(0);
        let updated = e
            .fill_form_field(
                &info.id,
                FillFormFieldRequest {
                    page_index: 0,
                    annotation_index: 0,
                    expected_value: field.value,
                    value: FormFieldValue::Text {
                        text: "Updated name".into(),
                    },
                },
            )
            .unwrap();
        let output = temp.path().join("saved.pdf");
        export(&e, &updated, &output);
        let saved = Document::load(&output).unwrap();
        for page in [0, 1] {
            let actual = colors(&saved,normal(&saved, page, 0));
            if actual.is_empty()
                || !actual
                    .iter()
                    .all(|c| c.iter().zip(&expected).all(|(x, y)| (x - y).abs() < 0.005))
            {
                failures.push(format!("{label}/page{page}: {actual:?}"));
            }
        }
        assert_eq!(
            lopdf::content::Content::decode(&effective_da(&saved, 0))
                .unwrap()
                .encode()
                .unwrap(),
            lopdf::content::Content::decode(definition.as_bytes())
                .unwrap()
                .encode()
                .unwrap()
        );
        assert_eq!(
            normal(&saved, 2, 0).get_plain_content().unwrap(),
            normal(&d, 2, 0).get_plain_content().unwrap()
        );
        let reopened = e.open_document(&output).unwrap();
        for page in [0, 1] {
            assert_eq!(
                e.list_form_fields(&reopened.id, page).unwrap()[0].value,
                FormFieldValue::Text {
                    text: "Updated name".into()
                }
            );
        }
        assert_eq!(fs::read(temp.path().join("form.pdf")).unwrap(), bytes);
        if let Some(root) = std::env::var_os("FOLIO_FORM_COLOR_OUTPUT") {
            let root = PathBuf::from(root);
            fs::create_dir_all(&root).unwrap();
            fs::copy(
                temp.path().join("form.pdf"),
                root.join(format!("{label}-before.pdf")),
            )
            .unwrap();
            fs::copy(&output, root.join(format!("{label}-after.pdf"))).unwrap();
        }
        for id in [&info.id, &updated.id, &reopened.id] {
            e.close_document(id).unwrap();
        }
    }
    assert!(failures.is_empty(), "color drift: {failures:?}");
}

fn effective_da(d: &Document, page: usize) -> Vec<u8> {
    let p = d
        .get_dictionary(*d.get_pages().values().nth(page).unwrap())
        .unwrap();
    let a = d
        .dereference(p.get(b"Annots").unwrap())
        .unwrap()
        .1
        .as_array()
        .unwrap();
    let mut field = d.dereference(&a[0]).unwrap().1.as_dict().unwrap();
    for _ in 0..64 {
        if let Ok(v) = field.get(b"DA") {
            return d.dereference(v).unwrap().1.as_str().unwrap().to_vec();
        }
        if let Ok(v) = field.get(b"Parent") {
            field = d.dereference(v).unwrap().1.as_dict().unwrap();
        } else {
            break;
        }
    }
    d.dereference(d.catalog().unwrap().get(b"AcroForm").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap()
        .get(b"DA")
        .unwrap()
        .as_str()
        .unwrap()
        .to_vec()
}

fn widget_id(d: &Document, page: usize) -> lopdf::ObjectId {
    let p = d
        .get_dictionary(*d.get_pages().values().nth(page).unwrap())
        .unwrap();
    d.dereference(p.get(b"Annots").unwrap())
        .unwrap()
        .1
        .as_array()
        .unwrap()[0]
        .as_reference()
        .unwrap()
}
#[test]
fn choices_and_editable_combos_keep_their_local_color() {
    for (label, flags) in [
        ("list", 0),
        ("combo", 1 << 17),
        ("editable-combo", (1 << 17) | (1 << 18)),
    ] {
        let mut d = Document::load_mem(&fixture()).unwrap();
        let w = widget_id(&d, 2);
        d.get_dictionary_mut(w).unwrap().set("Ff", flags);
        d.get_dictionary_mut(w)
            .unwrap()
            .set("DA", Object::string_literal("/F1 12 Tf 0 g 1 0 0 rg"));
        let mut bytes = vec![];
        d.save_to(&mut bytes).unwrap();
        let (temp, e, info) = open(&bytes);
        let f = e.list_form_fields(&info.id, 2).unwrap().remove(0);
        let value = if flags & (1 << 18) != 0 {
            FormFieldValue::Text {
                text: "Custom choice".into(),
            }
        } else {
            FormFieldValue::Choice { indices: vec![0] }
        };
        let updated = e
            .fill_form_field(
                &info.id,
                FillFormFieldRequest {
                    page_index: 2,
                    annotation_index: 0,
                    expected_value: f.value,
                    value,
                },
            )
            .unwrap();
        let output = temp.path().join("saved.pdf");
        export(&e, &updated, &output);
        let saved = Document::load(&output).unwrap();
        let actual = colors(&saved,normal(&saved, 2, 0));
        assert!(
            actual.iter().any(|c| c == &vec![1., 0., 0.]),
            "{label}: {actual:?}"
        );
        if label != "list" {
            assert!(actual.iter().all(|c| c == &vec![1., 0., 0.]));
        }
        for page in [0, 1] {
            assert_eq!(
                normal(&saved, page, 0).get_plain_content().unwrap(),
                normal(&d, page, 0).get_plain_content().unwrap()
            );
        }
        if let Some(root) = std::env::var_os("FOLIO_FORM_COLOR_OUTPUT") {
            let root = PathBuf::from(root);
            fs::create_dir_all(&root).unwrap();
            fs::copy(&output, root.join(format!("{label}-after.pdf"))).unwrap();
        }
        for id in [&info.id, &updated.id] {
            e.close_document(id).unwrap();
        }
    }
}
#[test]
fn shared_widgets_keep_distinct_inherited_colors_across_repeated_fills() {
    let mut d = Document::load_mem(&fixture()).unwrap();
    let w = widget_id(&d, 0);
    let parent = d
        .get_dictionary(w)
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    d.get_dictionary_mut(parent)
        .unwrap()
        .set("DA", Object::string_literal("/F1 12 Tf 0 g 0 0 1 rg"));
    d.get_dictionary_mut(w)
        .unwrap()
        .set("DA", Object::string_literal("/F1 12 Tf 0 g 1 0 0 rg"));
    let mut bytes = vec![];
    d.save_to(&mut bytes).unwrap();
    let (temp, e, info) = open(&bytes);
    let mut current = info.id.clone();
    for round in 0..3 {
        let f = e.list_form_fields(&current, 0).unwrap().remove(0);
        let updated = e
            .fill_form_field(
                &current,
                FillFormFieldRequest {
                    page_index: 0,
                    annotation_index: 0,
                    expected_value: f.value,
                    value: FormFieldValue::Text {
                        text: format!("Round {round}"),
                    },
                },
            )
            .unwrap();
        e.close_document(&current).unwrap();
        let output = temp.path().join(format!("round-{round}.pdf"));
        export(&e, &updated, &output);
        let saved = Document::load(&output).unwrap();
        for (page, expected) in [(0, vec![1., 0., 0.]), (1, vec![0., 0., 1.])] {
            let painted = colors(&saved,normal(&saved, page, 0));
            assert!(!painted.is_empty(), "round {round}, page {page}: missing text paint");
            assert!(painted.iter().all(|color| color == &expected));
        }
        let reopened = e.open_document(&output).unwrap();
        e.close_document(&updated.id).unwrap();
        current = reopened.id;
        for page in [0, 1] {
            assert_eq!(
                e.list_form_fields(&current, page).unwrap()[0].value,
                FormFieldValue::Text {
                    text: format!("Round {round}")
                }
            );
        }
        if let Some(root) = std::env::var_os("FOLIO_FORM_COLOR_OUTPUT") {
            let root = PathBuf::from(root);
            fs::create_dir_all(&root).unwrap();
            fs::copy(&output, root.join(format!("shared-round-{round}.pdf"))).unwrap();
        }
    }
    e.close_document(&current).unwrap();
    assert_eq!(fs::read(temp.path().join("form.pdf")).unwrap(), bytes);
}
