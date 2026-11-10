use super::*;
use crate::{
    ChoiceDraftOption, ChoiceEdit, ChoiceEditSelection as Edit, ChoiceSelection,
    SetFormChoicesRequest,
};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

fn runtime() -> WorkerRuntime {
    WorkerRuntime::new(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
        Arc::new(FontRegistry::new()),
    )
    .unwrap()
}
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../tests/fixtures/choice-text/{name}-source.pdf")),
    )
    .unwrap()
}
fn request(bytes: &[u8]) -> SetFormChoicesRequest {
    let info = crate::inspect_choice_pdf(bytes, 0, 0).unwrap();
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
fn widget(doc: &Document, page: usize) -> ObjectId {
    let p = *doc.get_pages().values().nth(page).unwrap();
    doc.dereference(doc.get_dictionary(p).unwrap().get(b"Annots").unwrap())
        .unwrap()
        .1
        .as_array()
        .unwrap()[0]
        .as_reference()
        .unwrap()
}
fn ap(doc: &Document, id: ObjectId) -> Dictionary {
    doc.dereference(doc.get_dictionary(id).unwrap().get(b"AP").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap()
        .clone()
}
fn normal(doc: &Document, id: ObjectId) -> Stream {
    doc.dereference(ap(doc, id).get(b"N").unwrap())
        .unwrap()
        .1
        .as_stream()
        .unwrap()
        .clone()
}
fn form(doc: &Document) -> Dictionary {
    doc.dereference(doc.catalog().unwrap().get(b"AcroForm").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap()
        .clone()
}
fn pixels(r: &WorkerRuntime, bytes: &[u8], index: i32) -> image::RgbaImage {
    let doc = r.pdfium.load_pdf_from_byte_slice(bytes, None).unwrap();
    doc.pages()
        .get(index)
        .unwrap()
        .render_with_config(&PdfRenderConfig::new().set_target_width(600))
        .unwrap()
        .as_image()
        .unwrap()
        .to_rgba8()
}
fn change(
    r: &WorkerRuntime,
    bytes: &[u8],
    request: &SetFormChoicesRequest,
) -> EngineResult<Vec<u8>> {
    crate::form_choice_publish::change(r.pdfium, r.bindings, bytes, request)
}
fn record(name: &str, bytes: &[u8]) {
    use std::io::Write;
    if let Some(path) = std::env::var_os("FOLIO_CHOICE_TRANSACTION_EVIDENCE") {
        let dir = PathBuf::from(path);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(dir.join(format!("{name}.pdf")))
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }
}

#[test]
fn choice_transaction_publishes_all_linked_widgets_and_returns_exact_noops() {
    let r = runtime();
    let raw = fixture("combo");
    let original = Document::load_mem(&raw).unwrap();
    let mut edit = request(&raw);
    assert!(
        change(&r, &raw, &edit).unwrap() == raw,
        "no-op changed source bytes"
    );
    edit.edit.options[1].display = "Updated label".into();
    let output = change(&r, &raw, &edit).unwrap();
    let before = crate::inspect_choice_pdf(&raw, 0, 0).unwrap();
    let after = crate::inspect_choice_pdf(&output, 0, 0).unwrap();
    assert_eq!(after.widgets, before.widgets);
    assert_eq!(after.model.selection, before.model.selection);
    assert_eq!(after.model.options[1].display, "Updated label");
    let doc = Document::load_mem(&output).unwrap();
    for page in 0..2 {
        assert!(
            pixels(&r, &output, page) != pixels(&r, &raw, page),
            "linked page {page} did not repaint"
        );
        let a = normal(&original, widget(&original, page as usize));
        let b = normal(&doc, widget(&doc, page as usize));
        for key in [b"BBox".as_slice(), b"Matrix", b"Resources"] {
            assert_eq!(a.dict.get(key).ok(), b.dict.get(key).ok());
        }
    }
    record("linked-source", &raw);
    record("linked-changed", &output);
}

#[test]
fn choice_transaction_metadata_defaults_leave_all_saved_appearance_bytes_intact() {
    let r = runtime();
    let raw = fixture("combo");
    let mut edit = request(&raw);
    edit.edit.defaults = Edit::Clear;
    let output = change(&r, &raw, &edit).unwrap();
    let original = Document::load_mem(&raw).unwrap();
    let doc = Document::load_mem(&output).unwrap();
    assert!(crate::inspect_choice_pdf(&output, 0, 0)
        .unwrap()
        .model
        .defaults
        .is_none());
    for page in 0..2 {
        let a = normal(&original, widget(&original, page));
        let b = normal(&doc, widget(&doc, page));
        assert_eq!(a.dict, b.dict);
        assert!(a.content == b.content, "metadata edit rewrote appearance");
        assert!(pixels(&r, &raw, page as i32) == pixels(&r, &output, page as i32));
    }
    record("metadata-default", &output);
}

#[test]
fn choice_transaction_transformed_text_keeps_artwork_and_repeated_edits_do_not_grow() {
    use lopdf::content::Content;
    let r = runtime();
    for kind in ["combo", "list"] {
        for rotation in [0, 90] {
            for variant in ["canonical", "shift", "scale", "affine-matrix"] {
                let name = format!("{kind}-{rotation}-{variant}");
                let raw = std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
                    "../tests/fixtures/choice-appearance/{name}-source.pdf"
                )))
                .unwrap();
                let info = crate::inspect_choice_pdf(&raw, 0, 0).unwrap();
                let row = match &info.model.selection {
                    ChoiceSelection::Rows(rows) => rows[0],
                    _ => panic!("fixture selection"),
                };
                let mut edit = request(&raw);
                edit.edit.options[row].display = "Updated".into();
                let changed = change(&r, &raw, &edit).unwrap_or_else(|e| panic!("{name}: {e}"));
                let mut restore = request(&changed);
                restore.edit.options[row].display = info.model.options[row].display.clone();
                let restored = change(&r, &changed, &restore)
                    .unwrap_or_else(|e| panic!("{name} restore: {e}"));
                let mut again = request(&restored);
                again.edit.options[row].display = "Updated".into();
                let repeated = change(&r, &restored, &again).unwrap();
                let original = Document::load_mem(&raw).unwrap();
                let new = Document::load_mem(&changed).unwrap();
                let third = Document::load_mem(&repeated).unwrap();
                for page in 0..original.get_pages().len() {
                    assert!(
                        pixels(&r, &raw, page as i32) == pixels(&r, &restored, page as i32),
                        "{name} restored {page}"
                    );
                    assert!(
                        pixels(&r, &raw, page as i32) != pixels(&r, &changed, page as i32),
                        "{name} changed {page}"
                    );
                    let a = normal(&original, widget(&original, page));
                    let b = normal(&new, widget(&new, page));
                    let c = normal(&third, widget(&third, page));
                    assert!(
                        b.get_plain_content().unwrap() == c.get_plain_content().unwrap(),
                        "{name}: repeated appearance grew or drifted"
                    );
                    // PDF serialization may spell an integral Real as an Integer;
                    // compare coordinate values, while preserving resource operands.
                    for key in [b"BBox".as_slice(), b"Matrix"] {
                        let values = |s: &Stream| {
                            s.dict
                                .get(key)
                                .unwrap()
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|v| v.as_float().unwrap())
                                .collect::<Vec<_>>()
                        };
                        assert_eq!(values(&a), values(&b), "{name}");
                    }
                    assert_eq!(
                        a.dict.get(b"Resources").ok(),
                        b.dict.get(b"Resources").ok(),
                        "{name}"
                    );
                    let outside = |s: &Stream| {
                        let ops = Content::decode(&s.get_plain_content().unwrap())
                            .unwrap()
                            .operations;
                        let start = ops.iter().position(|o| o.operator == "BMC").unwrap();
                        let end = ops.iter().position(|o| o.operator == "EMC").unwrap();
                        Content {
                            operations: ops[..start]
                                .iter()
                                .chain(&ops[end + 1..])
                                .cloned()
                                .collect::<Vec<_>>(),
                        }
                        .encode()
                        .unwrap()
                    };
                    assert!(
                        outside(&a) == outside(&b),
                        "{name}: surrounding artwork changed"
                    );
                }
                record(&format!("geometry-{name}-source"), &raw);
                record(&format!("geometry-{name}-changed"), &changed);
                record(&format!("geometry-{name}-restored"), &restored);
            }
        }
    }
}

#[test]
fn choice_transaction_empty_and_free_text_answers_restore_without_moving_artwork() {
    let r = runtime();
    for name in ["empty", "free", "artwork"] {
        let raw = fixture(name);
        // The producer's "empty" fixture lacks V and displays its DV fallback.
        // Explicit Clear creates the actual blank state exercised by this test.
        let raw = if name == "empty" {
            let mut clear = request(&raw);
            clear.edit.selection = Edit::Clear;
            change(&r, &raw, &clear).unwrap()
        } else {
            raw
        };
        let info = crate::inspect_choice_pdf(&raw, 0, 0).unwrap();
        let mut edit = request(&raw);
        edit.edit.selection = if info.model.editable {
            Edit::Text("Changed answer".into())
        } else {
            Edit::Rows(vec![0])
        };
        let changed = change(&r, &raw, &edit).unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut restore = request(&changed);
        restore.edit.selection = match info.model.selection {
            ChoiceSelection::None => Edit::Clear,
            ChoiceSelection::Rows(rows) => Edit::Rows(rows),
            ChoiceSelection::Text(text) => Edit::Text(text),
        };
        let restored =
            change(&r, &changed, &restore).unwrap_or_else(|e| panic!("{name} restore: {e}"));
        for page in 0..info.widgets.len() {
            assert!(
                pixels(&r, &raw, page as i32) == pixels(&r, &restored, page as i32),
                "{name}: restored pixels differ"
            );
            assert!(
                pixels(&r, &raw, page as i32) != pixels(&r, &changed, page as i32),
                "{name}: answer did not change"
            );
        }
        record(&format!("{name}-source"), &raw);
        record(&format!("{name}-changed"), &changed);
        record(&format!("{name}-restored"), &restored);
    }
}

#[test]
fn choice_transaction_reused_widget_checks_every_distinct_physical_scale() {
    let r = runtime();
    let mut ctx = crate::form_properties::load(&fixture("combo"), 0, 0).unwrap();
    let first = widget(&ctx.doc, 0);
    let pages: Vec<_> = ctx.doc.get_pages().values().copied().collect();
    ctx.doc
        .get_dictionary_mut(ctx.owner)
        .unwrap()
        .set("Kids", vec![Object::Reference(first)]);
    ctx.doc
        .get_dictionary_mut(pages[1])
        .unwrap()
        .set("Annots", vec![Object::Reference(first)]);
    ctx.doc
        .get_dictionary_mut(pages[0])
        .unwrap()
        .set("UserUnit", Object::Real(0.5));
    let unit = ctx.doc.add_object(Object::Real(1.7));
    ctx.doc
        .get_dictionary_mut(pages[1])
        .unwrap()
        .set("UserUnit", unit);
    let raw = crate::pdf_output::save(&mut ctx.doc).unwrap();
    let mut edit = request(&raw);
    edit.edit.options[1].display = "Updated".into();
    let changed = change(&r, &raw, &edit).unwrap();
    assert_eq!(
        crate::inspect_choice_pdf(&changed, 0, 0)
            .unwrap()
            .widgets
            .len(),
        2
    );
    for page in 0..2 {
        assert!(pixels(&r, &raw, page) != pixels(&r, &changed, page));
    }
    ctx.doc
        .get_dictionary_mut(pages[1])
        .unwrap()
        .set("UserUnit", 75000);
    let large = crate::pdf_output::save(&mut ctx.doc).unwrap();
    let mut edit = request(&large);
    edit.edit.options[1].display = "Updated".into();
    let error = change(&r, &large, &edit).unwrap_err();
    assert!(
        error.to_string().contains("limit") || error.to_string().contains("budget"),
        "{error}"
    );
    record("reused-source", &raw);
    record("reused-changed", &changed);
}

#[test]
fn choice_transaction_clear_restore_and_reorder_keep_answers_defaults_and_top_row() {
    let r = runtime();
    let raw = fixture("list");
    let original = crate::inspect_choice_pdf(&raw, 0, 0).unwrap();
    assert_eq!(original.model.selection, ChoiceSelection::Rows(vec![0, 2]));
    let mut edit = request(&raw);
    edit.edit.selection = Edit::Clear;
    let cleared = change(&r, &raw, &edit).unwrap();
    assert_eq!(
        crate::inspect_choice_pdf(&cleared, 0, 0)
            .unwrap()
            .model
            .selection,
        ChoiceSelection::None
    );
    let mut restore = request(&cleared);
    restore.edit.selection = Edit::Rows(vec![0, 2]);
    let restored = change(&r, &cleared, &restore).unwrap();
    record("list-source", &raw);
    record("list-cleared", &cleared);
    record("list-restored", &restored);
    for page in 0..2 {
        assert!(
            pixels(&r, &restored, page) == pixels(&r, &raw, page),
            "restored page {page}"
        );
    }
    let mut reorder = request(&restored);
    reorder.edit.options.rotate_left(1);
    let reordered = change(&r, &restored, &reorder).unwrap();
    let after = crate::inspect_choice_pdf(&reordered, 0, 0).unwrap();
    assert_eq!(after.model.selection, ChoiceSelection::Rows(vec![1, 2]));
    assert_eq!(after.model.defaults, original.model.defaults);
    assert_eq!(
        after.model.options[after.model.top_index].export_value,
        original.model.options[original.model.top_index].export_value
    );
    record("list-reordered", &reordered);
}

#[test]
fn choice_transaction_shared_normal_and_rollover_resources_do_not_change_other_fields() {
    let r = runtime();
    let mut ctx = crate::form_properties::load(&fixture("combo"), 0, 0).unwrap();
    let first = widget(&ctx.doc, 0);
    let page = *ctx.doc.get_pages().values().next().unwrap();
    let shared = ap(&ctx.doc, first).get(b"N").unwrap().clone();
    for id in ctx.widgets.clone() {
        let mut a = ap(&ctx.doc, id);
        a.set("R", shared.clone());
        a.set("D", shared.clone());
        ctx.doc.get_dictionary_mut(id).unwrap().set("AP", a);
    }
    let mut other = ctx.doc.get_dictionary(first).unwrap().clone();
    other.remove(b"Parent");
    for (key, value) in ctx.doc.get_dictionary(ctx.owner).unwrap().iter() {
        if key.as_slice() != b"Kids" {
            other.set(key.clone(), value.clone());
        }
    }
    other.set("T", lopdf::text_string("Unrelated"));
    other.set("Rect", vec![320.into(), 60.into(), 580.into(), 150.into()]);
    let other = ctx.doc.add_object(other);
    let mut f = form(&ctx.doc);
    let mut fields = f.get(b"Fields").unwrap().as_array().unwrap().clone();
    fields.push(other.into());
    f.set("Fields", fields);
    ctx.doc.catalog_mut().unwrap().set("AcroForm", f);
    let page_dict = ctx.doc.get_dictionary_mut(page).unwrap();
    page_dict.set("Annots", vec![first.into(), other.into()]);
    page_dict.set("MediaBox", vec![0.into(), 0.into(), 600.into(), 300.into()]);
    let raw = crate::pdf_output::save(&mut ctx.doc).unwrap();
    let mut edit = request(&raw);
    edit.edit.options[1].display = "Updated label".into();
    let output = change(&r, &raw, &edit).unwrap();
    let doc = Document::load_mem(&output).unwrap();
    assert_eq!(
        doc.get_dictionary(other).unwrap(),
        ctx.doc.get_dictionary(other).unwrap()
    );
    assert_eq!(ap(&doc, other).get(b"N").unwrap(), &shared);
    for id in ctx.widgets {
        let a = ap(&doc, id);
        assert_ne!(a.get(b"N").unwrap(), &shared);
        for key in [b"R", b"D"] {
            assert_eq!(a.get(key).unwrap(), &shared);
        }
    }
    let a = pixels(&r, &raw, 0);
    let b = pixels(&r, &output, 0);
    assert_eq!(a.dimensions(), b.dimensions());
    assert!(
        (0..a.height()).all(|y| (300..a.width()).all(|x| a.get_pixel(x, y) == b.get_pixel(x, y))),
        "unrelated field pixels changed"
    );
    record("aliases-source", &raw);
    record("aliases-changed", &output);
}

#[test]
fn choice_transaction_every_linked_font_and_new_hidden_label_are_verified_before_publication() {
    let r = runtime();
    let raw = fixture("combo");
    let mut edit = request(&raw);
    edit.edit.options[1].display = "Updated".into();
    assert!(change(&r, &raw, &edit).is_ok());
    let mut doc = Document::load_mem(&raw).unwrap();
    let second = widget(&doc, 1);
    doc.get_dictionary_mut(second)
        .unwrap()
        .set("DA", Object::string_literal("/Missing 12 Tf 0 g"));
    let bad = crate::pdf_output::save(&mut doc).unwrap();
    let mut changed = request(&bad);
    changed.edit.options[1].display = "Updated".into();
    let error = change(&r, &bad, &changed).unwrap_err();
    assert!(error.to_string().contains("font"), "{error}");
    edit.edit.options[2].display = "Missing 字".into();
    assert!(change(&r, &raw, &edit).is_err());
    assert_eq!(
        crate::inspect_choice_pdf(&raw, 0, 0).unwrap().model.options[1].display,
        "Beta"
    );
    assert_eq!(
        crate::form_fill::NativeForm::open(r.bindings, &raw)
            .unwrap()
            .fields(1)
            .unwrap()[0]
            .selected_indices,
        vec![1]
    );
}

#[test]
fn choice_transaction_global_regeneration_and_stale_keys_are_refused() {
    let r = runtime();
    let raw = fixture("combo");
    let mut edit = request(&raw);
    edit.edit.options[1].display = "Updated".into();
    for flag in [Object::Boolean(true), Object::Integer(0), Object::Null] {
        let mut doc = Document::load_mem(&raw).unwrap();
        let mut f = form(&doc);
        f.set("NeedAppearances", flag);
        doc.catalog_mut().unwrap().set("AcroForm", f);
        let bytes = crate::pdf_output::save(&mut doc).unwrap();
        let req = request(&bytes);
        let error = change(&r, &bytes, &req).unwrap_err();
        assert!(error.to_string().contains("regeneration"), "{error}");
    }
    edit.expected_key = "stale".into();
    let error = change(&r, &raw, &edit).unwrap_err();
    assert!(error.to_string().contains("changed"), "{error}");
}

#[test]
fn choice_transaction_native_selection_disagreement_cannot_be_repaired_silently() {
    let r = runtime();
    let mut ctx = crate::form_properties::load(&fixture("combo"), 0, 0).unwrap();
    let selected = ctx.doc.add_object(1);
    let owner = ctx.doc.get_dictionary_mut(ctx.owner).unwrap();
    owner.remove(b"V");
    owner.set("I", vec![Object::Reference(selected)]);
    let raw = crate::pdf_output::save(&mut ctx.doc).unwrap();
    assert_eq!(
        crate::inspect_choice_pdf(&raw, 0, 0)
            .unwrap()
            .model
            .selection,
        ChoiceSelection::Rows(vec![1])
    );
    assert!(crate::form_fill::NativeForm::open(r.bindings, &raw)
        .unwrap()
        .fields(0)
        .unwrap()[0]
        .selected_indices
        .is_empty());
    let error = change(&r, &raw, &request(&raw)).unwrap_err();
    assert!(
        error.to_string().contains("native") && error.to_string().contains("selection"),
        "{error}"
    );
}

#[test]
fn choice_transaction_staged_appearance_copies_have_one_aggregate_byte_budget() {
    let r = runtime();
    let mut ctx = crate::form_properties::load(&fixture("combo"), 0, 0).unwrap();
    let page = *ctx.doc.get_pages().values().next().unwrap();
    let first = widget(&ctx.doc, 0);
    let mut shared = normal(&ctx.doc, first);
    shared.dict.set(
        "FolioBudgetWitness",
        Object::string_literal(vec![b'x'; 1024 * 1024]),
    );
    let shared = ctx.doc.add_object(shared);
    let mut appearance = ap(&ctx.doc, first);
    appearance.set("N", shared);
    for id in ctx.widgets.clone() {
        ctx.doc
            .get_dictionary_mut(id)
            .unwrap()
            .set("AP", appearance.clone());
    }
    let template = ctx.doc.get_dictionary(first).unwrap().clone();
    let mut kids = ctx
        .doc
        .get_dictionary(ctx.owner)
        .unwrap()
        .get(b"Kids")
        .unwrap()
        .as_array()
        .unwrap()
        .clone();
    let mut annots = vec![Object::Reference(first)];
    for _ in 0..31 {
        let id = ctx.doc.add_object(template.clone());
        kids.push(id.into());
        annots.push(id.into());
    }
    ctx.doc
        .get_dictionary_mut(ctx.owner)
        .unwrap()
        .set("Kids", kids);
    ctx.doc
        .get_dictionary_mut(page)
        .unwrap()
        .set("Annots", annots);
    let raw = crate::pdf_output::save(&mut ctx.doc).unwrap();
    let mut edit = request(&raw);
    edit.edit.options[1].display = "Updated".into();
    let error = change(&r, &raw, &edit).unwrap_err();
    assert!(
        error.to_string().contains("retained") && error.to_string().contains("budget"),
        "{error}"
    );
    assert_eq!(
        crate::inspect_choice_pdf(&raw, 0, 0).unwrap().model.options[1].display,
        "Beta"
    );
}
