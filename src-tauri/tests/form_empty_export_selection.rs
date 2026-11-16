//! A selected empty export must remain visibly selected after clear/save/re-edit.
use folio_engine::{DocumentInfo, ExportRequest, FillFormFieldRequest, FormFieldValue, PagePlan, PdfEngine};
use lopdf::{Document, Object};
use std::{fs, path::{Path, PathBuf}};

fn source(initially_selected: bool) -> Vec<u8> {
    let mut doc = Document::load_mem(include_bytes!("fixtures/form-compound-list-single-selection-paint.pdf")).unwrap();
    let page = doc.get_pages()[&1];
    let widget = doc.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap();
    let field = doc.get_dictionary_mut(widget).unwrap();
    field.set("Opt", vec![Object::Array(vec![Object::string_literal(""), Object::string_literal("Red")]),
                          Object::string_literal("Green"), Object::string_literal("Blue")]);
    if initially_selected {
        field.set("V", Object::string_literal(""));
        field.set("I", vec![Object::Integer(0)]);
    } else {
        field.remove(b"V"); field.remove(b"I");
        let ap = field.get(b"AP").unwrap().as_dict().unwrap().get(b"N").unwrap().as_reference().unwrap();
        let stream = doc.get_object_mut(ap).unwrap().as_stream_mut().unwrap();
        let data = String::from_utf8(stream.get_plain_content_with_limit(1024 * 1024).unwrap()).unwrap();
        let paint = "q 0.7 0.8 1 rg 1 49.000000 248 16 re f Q";
        assert!(data.contains(paint), "fixture no longer has the independently authored selection");
        stream.set_plain_content(data.replace(paint, "").into_bytes());
    }
    let mut bytes = Vec::new(); doc.save_to(&mut bytes).unwrap(); bytes
}

fn save(e: &PdfEngine, doc: &DocumentInfo, path: &Path) -> DocumentInfo {
    let pages: Vec<PagePlan> = doc.pages.iter().enumerate().map(|(i,p)| {
        serde_json::from_value(serde_json::json!({"id":format!("page-{i}"),"sourceId":doc.id,
            "pageIndex":i,"width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})).unwrap()
    }).collect();
    e.export_pdf(ExportRequest {pages,flatten:false},path).unwrap();
    e.open_document(path).unwrap()
}

fn visible_selection(e: &PdfEngine, doc: &DocumentInfo, original: &image::RgbImage, wanted: &[usize]) {
    let current = image::load_from_memory(&e.render_page(&doc.id, 0, 840).unwrap()).unwrap().to_rgb8();
    assert_eq!(current.dimensions(), original.dimensions());
    for (row, y) in [148, 179, 210].into_iter().enumerate() {
        // Fixed source-space row centers, far from text. Checking the native
        // model alone missed the original bug: I[0]/Vempty but no blue row.
        let rgb = current.get_pixel(660, y).0;
        if wanted.contains(&row) {
            assert!(rgb == [0, 51, 112] || rgb == [0, 51, 113],
                    "selected row {row} lost its visible highlight: {rgb:?}");
        } else {
            assert_eq!(rgb, [255, 255, 255], "unselected row {row} gained paint");
        }
    }
    // Exact 1pt inset; the complete source border and outside remain checked.
    for (x,y,pixel) in current.enumerate_pixels() {
        if !(202..698).contains(&x) || !(132..268).contains(&y) {
            assert_eq!(pixel, original.get_pixel(x,y), "source frame/outside changed at {x},{y}");
        }
    }
}

fn round_trip(initially_selected: bool) {
    let bytes = source(initially_selected);
    let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("original.pdf"); fs::write(&path, &bytes).unwrap();
    let e = PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
    let original = e.open_document(&path).unwrap();
    let pixels = image::load_from_memory(&e.render_page(&original.id, 0, 840).unwrap()).unwrap().to_rgb8();
    let first = e.list_form_fields(&original.id,0).unwrap().remove(0);
    assert!(first.reason.is_none(), "{:?}", first.reason);
    assert_eq!(first.options, vec!["Red", "Green", "Blue"]);
    assert_eq!(first.selected_indices, if initially_selected {vec![0]} else {vec![]});
    let mut current = original.clone();
    let states = [vec![1], vec![], vec![0], vec![2], vec![], vec![0], vec![1], vec![], vec![0]];
    for (step, indices) in states.into_iter().enumerate() {
        let old = e.list_form_fields(&current.id,0).unwrap().remove(0);
        let wanted = FormFieldValue::Choice {indices: indices.clone()};
        let filled = e.fill_form_field(&current.id, FillFormFieldRequest {page_index:0,
            annotation_index:old.annotation_index, expected_value:old.value, value:wanted.clone()}).unwrap();
        visible_selection(&e,&filled,&pixels,&indices);
        let destination = dir.path().join(format!("saved-{step}.pdf"));
        let reopened = save(&e,&filled,&destination);
        visible_selection(&e,&reopened,&pixels,&indices);
        let field = e.list_form_fields(&reopened.id,0).unwrap().remove(0);
        assert_eq!(field.value, wanted); assert_eq!(field.selected_indices, indices);
        assert_eq!(field.options, first.options);
        let saved = fs::read(&destination).unwrap(); let parsed = Document::load_mem(&saved).unwrap();
        let page = parsed.get_pages()[&1];
        let widget = parsed.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap();
        let raw = parsed.get_dictionary(widget).unwrap();
        assert_eq!(raw.get(b"Opt").unwrap().as_array().unwrap()[0],
            Object::Array(vec![Object::string_literal(""), Object::string_literal("Red")]));
        if indices.is_empty() {
            assert!(!raw.has(b"I") && !raw.has(b"V"), "clear is different from selected empty export");
        } else {
            assert_eq!(raw.get(b"I").unwrap(), &Object::Array(indices.iter().map(|i|Object::Integer(*i as i64)).collect()));
            let export = match indices[0] {0=>"", 1=>"Green", 2=>"Blue", _=>unreachable!()};
            assert_eq!(raw.get(b"V").unwrap().as_str().unwrap(), export.as_bytes());
        }
        if let Some(root) = std::env::var_os("FOLIO_EMPTY_EXPORT_OUTPUTS") {
            let folder = PathBuf::from(root).join(if initially_selected {"selected"} else {"empty"});
            fs::create_dir_all(&folder).unwrap(); let p = folder.join(format!("{step}.pdf")); assert!(!p.exists()); fs::write(p,&saved).unwrap();
            if step==0 {fs::write(folder.join("original.pdf"), &bytes).unwrap();}
        }
        assert_eq!(fs::read(&path).unwrap(),bytes);
        assert_eq!(e.source_bytes(&original.id).unwrap().as_ref(),bytes);
        e.close_document(&filled.id).unwrap();
        if current.id!=original.id {e.close_document(&current.id).unwrap();}
        current=reopened;
    }
    e.close_document(&current.id).unwrap(); e.close_document(&original.id).unwrap();
}

#[test]
fn selected_empty_export_clears_and_reselects_visibly_after_save() {round_trip(true);}

#[test]
fn initially_empty_list_selects_empty_export_visibly_after_save() {round_trip(false);}
