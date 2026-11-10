use super::*;
use crate::{form_choice_probe::generate_choice_appearance, form_choice_text::ChoiceTextBlock};
use lopdf::{Document, Object};

fn runtime() -> WorkerRuntime {
    let library = std::env::var_os("FOLIO_TEST_PDFIUM_LIBRARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")
        });
    WorkerRuntime::new(&library, Arc::new(FontRegistry::new())).unwrap()
}
fn fixture(name: &str) -> Vec<u8> {
    if name == "embedded" {
        return std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../tests/fixtures/choice-probe/embedded-source.pdf"),
        )
        .unwrap();
    }
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../tests/fixtures/choice-text/{name}-source.pdf")),
    )
    .unwrap()
}
fn normal(d: &Document, id: lopdf::ObjectId) -> lopdf::Stream {
    let w = d.get_dictionary(id).unwrap();
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
        .clone()
}
fn choice_request(raw: &[u8]) -> crate::SetFormChoicesRequest {
    let info = crate::inspect_choice_pdf(raw, 0, 0).unwrap();
    crate::SetFormChoicesRequest {
        page_index: 0,
        annotation_index: 0,
        expected_key: info.expected_key,
        edit: crate::ChoiceEdit {
            options: info
                .model
                .options
                .iter()
                .enumerate()
                .map(|(i, o)| crate::ChoiceDraftOption {
                    source_index: Some(i),
                    export_value: o.export_value.clone(),
                    display: o.display.clone(),
                })
                .collect(),
            selection: crate::ChoiceEditSelection::Keep,
            defaults: crate::ChoiceEditSelection::Keep,
            manual_order: false,
        },
    }
}
fn pixels(r: &WorkerRuntime, bytes: &[u8]) -> Vec<Vec<u8>> {
    let doc = r.pdfium.load_pdf_from_byte_slice(bytes, None).unwrap();
    doc.pages()
        .iter()
        .map(|p| {
            p.render_with_config(&PdfRenderConfig::new().set_target_width(600))
                .unwrap()
                .as_image()
                .unwrap()
                .to_rgba8()
                .into_raw()
        })
        .collect()
}
fn record(name: &str, bytes: &[u8]) {
    use std::io::Write;
    if let Some(dir) = std::env::var_os("FOLIO_CHOICE_PROBE_EVIDENCE") {
        let path = PathBuf::from(dir);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path.join(format!("{name}.pdf")))
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }
}

fn cid_choice_context(labels: [&str; 3]) -> crate::form_properties::Context {
    let raw = fixture("combo");
    let mut request = choice_request(&raw);
    for (option, label) in request.edit.options.iter_mut().zip(labels) {
        option.display = label.into();
    }
    let mut ctx = crate::prepare_choice_edit_pdf(&raw, &request).unwrap().ctx;
    let chars: std::collections::BTreeSet<char> = labels.iter().flat_map(|s| s.chars()).collect();
    let (font_doc, font, _) =
        crate::choice_font_tests::cid_for(&chars.into_iter().collect::<Vec<_>>());
    crate::choice_font::ChoiceFontProof::for_texts(&font_doc, &font, &labels).unwrap();
    let copied = crate::form_appearance::Copier::new(&font_doc, &mut ctx.doc)
        .copy(&font)
        .unwrap();
    for id in ctx.widgets.clone() {
        let mut appearance = normal(&ctx.doc, id);
        appearance.dict.set(
            "Resources",
            lopdf::dictionary! {"Font"=>lopdf::dictionary!{"F1"=>copied.clone()}},
        );
        // A real CID operand for the currently selected second option, rather
        // than retaining the fixture's old one-byte Helvetica string.
        let codes = crate::choice_font::ChoiceFontProof::for_texts(&font_doc, &font, &labels)
            .unwrap()
            .encode(labels[1])
            .unwrap();
        let hex: String = codes.iter().map(|b| format!("{b:02X}")).collect();
        appearance.set_plain_content(
            format!("/Tx BMC q BT /F1 12 Tf 0 g 4 16 Td <{hex}> Tj ET Q EMC").into_bytes(),
        );
        replace_normal(&mut ctx.doc, id, appearance);
    }
    ctx
}

#[test]
fn hidden_supplementary_text_requires_native_encoding_evidence() {
    let r = runtime();
    let ctx = cid_choice_context(["𝔸", "A", "B"]);
    let before = ctx.doc.objects.clone();
    let error = generate_choice_appearance(r.bindings, &ctx, ctx.widgets[0])
        .err()
        .expect("nominal font coverage cannot authorize unobserved native supplementary text")
        .to_string();
    assert!(error.contains("native choice text"), "{error}");
    assert_eq!(ctx.doc.objects, before);
}

#[test]
fn native_greek_choices_keep_exact_cid_character_codes() {
    let r = runtime();
    let ctx = cid_choice_context(["A", "Ω", "é"]);
    let output = apply_probe_text(&r, &ctx);
    assert_eq!(
        crate::inspect_choice_pdf(&output, 0, 0)
            .unwrap()
            .model
            .options[1]
            .display,
        "Ω"
    );
    record("greek-codec", &output);
}

#[test]
fn native_choice_codec_handles_maximum_rows_without_retaining_audit_widgets() {
    let r = runtime();
    for kind in ["combo", "list"] {
        let raw = fixture(kind);
        let mut request = choice_request(&raw);
        for i in request.edit.options.len()..2000 {
            request.edit.options.push(crate::ChoiceDraftOption {
                source_index: None,
                export_value: format!("key-{i}"),
                display: format!("  Row {i}  "),
            });
        }
        request.edit.options[0].display = "".into();
        let ctx = crate::prepare_choice_edit_pdf(&raw, &request).unwrap().ctx;
        let start = std::time::Instant::now();
        let g = generate_choice_appearance(r.bindings, &ctx, ctx.widgets[0]).unwrap();
        let page = *g.document.get_pages().values().next().unwrap();
        let annots = g
            .document
            .get_dictionary(page)
            .unwrap()
            .get(b"Annots")
            .unwrap();
        assert_eq!(
            g.document
                .dereference(annots)
                .unwrap()
                .1
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let form = g
            .document
            .dereference(g.document.catalog().unwrap().get(b"AcroForm").unwrap())
            .unwrap()
            .1
            .as_dict()
            .unwrap();
        assert_eq!(
            g.document
                .dereference(form.get(b"Fields").unwrap())
                .unwrap()
                .1
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(!g
            .document
            .objects
            .values()
            .filter_map(|o| o.as_dict().ok())
            .any(|d| d
                .get(b"T")
                .ok()
                .and_then(|v| v.as_str().ok())
                .is_some_and(|v| v.starts_with(b"ChoiceCodec"))));
        println!(
            "2000-row {kind} choice codec: {:?}, {} retained objects",
            start.elapsed(),
            g.document.objects.len()
        );
    }
}

#[test]
fn absent_value_displays_raw_default_but_null_and_selected_values_do_not() {
    let r = runtime();
    for state in ["absent", "null", "selected"] {
        let raw = fixture("combo");
        let mut ctx = crate::form_properties::load(&raw, 0, 0).unwrap();
        let model = crate::form_choices::inspect_context(&ctx).unwrap().model;
        let export = model.options[1].export_value.clone();
        assert_ne!(export, model.options[1].display);
        let owner = ctx.doc.get_dictionary_mut(ctx.owner).unwrap();
        owner.set("DV", lopdf::text_string(&export));
        if state != "selected" {
            owner.remove(b"V");
            owner.remove(b"I");
        }
        if state == "null" {
            owner.set("V", Object::Null);
        }
        let generated = generate_choice_appearance(r.bindings, &ctx, ctx.widgets[0]).unwrap();
        let content = lopdf::content::Content::decode_strict(
            &generated.appearance.get_plain_content().unwrap(),
        )
        .unwrap();
        let shown: Vec<u8> = content
            .operations
            .iter()
            .filter(|op| op.operator == "Tj")
            .flat_map(|op| op.operands[0].as_str().unwrap().iter().copied())
            .collect();
        let expected = match state {
            "absent" => export.as_str(),
            "null" => "",
            _ => model.options[1].display.as_str(),
        };
        assert_eq!(shown, expected.as_bytes(), "{state}");
        record(&format!("fallback-{state}"), &apply_probe_text(&r, &ctx));
    }
}

#[test]
fn fallback_export_is_display_text_only_when_the_current_value_is_absent() {
    let r = runtime();
    for absent in [false, true] {
        let raw = fixture("combo");
        let mut request = choice_request(&raw);
        request.edit.options[1].export_value = "Ω export".into();
        request.edit.defaults = crate::ChoiceEditSelection::Rows(vec![1]);
        let mut ctx = crate::prepare_choice_edit_pdf(&raw, &request).unwrap().ctx;
        if absent {
            let owner = ctx.doc.get_dictionary_mut(ctx.owner).unwrap();
            owner.remove(b"V");
            owner.remove(b"I");
        }
        let result = generate_choice_appearance(r.bindings, &ctx, ctx.widgets[0]);
        assert_eq!(result.is_ok(), !absent);
        if let Err(error) = result {
            assert!(error.to_string().contains("original choice font"));
        }
    }
}

#[test]
fn long_embedded_batches_keep_one_font_program_and_remove_audit_appearances() {
    let r = runtime();
    let mut ctx = cid_choice_context(["A", "Ω", "é"]);
    let raw = crate::pdf_output::save(&mut ctx.doc).unwrap();
    let mut request = choice_request(&raw);
    let text = "AΩé".repeat(819);
    for i in 0..30 {
        request.edit.options.push(crate::ChoiceDraftOption {
            source_index: None,
            export_value: format!("long-{i}"),
            display: text.clone(),
        });
    }
    let ctx = crate::prepare_choice_edit_pdf(&raw, &request).unwrap().ctx;
    let start = std::time::Instant::now();
    let g = generate_choice_appearance(r.bindings, &ctx, ctx.widgets[0]).unwrap();
    let descriptors: Vec<_> = g
        .document
        .objects
        .values()
        .filter_map(|o| o.as_dict().ok())
        .filter(|d| d.has(b"FontFile2"))
        .collect();
    assert_eq!(descriptors.len(), 1);
    let appearances = g
        .document
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .filter(|s| s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Form"))
        .count();
    assert_eq!(appearances, 1);
    println!(
        "122850-byte embedded choice codec: {:?}, {} retained objects",
        start.elapsed(),
        g.document.objects.len()
    );
    let mut doc = g.document;
    record(
        "long-embedded-probe",
        &crate::pdf_output::save(&mut doc).unwrap(),
    );
}

#[test]
fn unselected_dropdown_label_requires_original_font_coverage() {
    let r = runtime();
    let raw = fixture("combo");
    let mut request = choice_request(&raw);
    request.edit.options[0].display = "Ω unavailable".into();
    let ctx = crate::prepare_choice_edit_pdf(&raw, &request).unwrap().ctx;
    assert_eq!(
        crate::form_choices::inspect_context(&ctx)
            .unwrap()
            .model
            .selection,
        crate::ChoiceSelection::Rows(vec![1])
    );
    let before = ctx.doc.objects.clone();
    let error = generate_choice_appearance(r.bindings, &ctx, ctx.widgets[0])
        .err()
        .expect("an unsupported unselected label must fail admission")
        .to_string();
    assert!(error.contains("original choice font"), "{error}");
    assert_eq!(ctx.doc.objects, before);
}

#[test]
fn offscreen_list_label_requires_original_font_coverage() {
    let r = runtime();
    let raw = fixture("list");
    let mut request = choice_request(&raw);
    for i in request.edit.options.len()..50 {
        request.edit.options.push(crate::ChoiceDraftOption {
            source_index: None,
            export_value: format!("row-{i}"),
            display: format!("Row {i}"),
        });
    }
    request.edit.options[49].display = "Ω unavailable".into();
    let ctx = crate::prepare_choice_edit_pdf(&raw, &request).unwrap().ctx;
    let error = generate_choice_appearance(r.bindings, &ctx, ctx.widgets[0])
        .err()
        .expect("an unsupported offscreen label must fail admission")
        .to_string();
    assert!(error.contains("original choice font"), "{error}");
}

#[test]
fn free_current_and_default_text_require_original_font_coverage() {
    let r = runtime();
    for default in [false, true] {
        let raw = fixture("free");
        let mut request = choice_request(&raw);
        if default {
            request.edit.defaults = crate::ChoiceEditSelection::Text("Ω".into());
        } else {
            request.edit.selection = crate::ChoiceEditSelection::Text("Ω".into());
        }
        let ctx = crate::prepare_choice_edit_pdf(&raw, &request).unwrap().ctx;
        let error = generate_choice_appearance(r.bindings, &ctx, ctx.widgets[0])
            .err()
            .expect("unsupported free text must fail admission")
            .to_string();
        assert!(
            error.contains("original choice font"),
            "default={default}: {error}"
        );
    }
}

#[test]
fn unicode_export_keys_do_not_require_display_glyphs() {
    let r = runtime();
    let raw = fixture("combo");
    let mut request = choice_request(&raw);
    request.edit.options[1].export_value = "字".into();
    request.edit.defaults = crate::ChoiceEditSelection::Rows(vec![1]);
    let ctx = crate::prepare_choice_edit_pdf(&raw, &request).unwrap().ctx;
    let output = apply_probe_text(&r, &ctx);
    assert!(
        pixels(&r, &output) == pixels(&r, &raw),
        "export identifiers must not change displayed labels"
    );
    let model = crate::inspect_choice_pdf(&output, 0, 0).unwrap().model;
    assert_eq!(model.options[1].export_value, "字");
    assert_eq!(model.defaults, Some(vec!["字".into()]));
    record("unicode-export", &output);
}

#[test]
fn accented_embedded_label_keeps_the_original_widget_font() {
    let r = runtime();
    let raw = fixture("embedded");
    let mut request = choice_request(&raw);
    request.edit.options[1].display = "Café".into();
    let ctx = crate::prepare_choice_edit_pdf(&raw, &request).unwrap().ctx;
    let output = apply_probe_text(&r, &ctx);
    assert!(
        pixels(&r, &output) != pixels(&r, &raw),
        "new selected label should be visible"
    );
    assert_eq!(
        crate::inspect_choice_pdf(&output, 0, 0)
            .unwrap()
            .model
            .options[1]
            .display,
        "Café"
    );
    record("accented-embedded", &output);
}

#[test]
fn linked_widgets_prove_their_own_font_resource_even_with_the_same_name() {
    let r = runtime();
    let raw = fixture("combo");
    let mut request = choice_request(&raw);
    request.edit.options[1].display = "Café".into();
    let mut ctx = crate::prepare_choice_edit_pdf(&raw, &request).unwrap().ctx;
    assert_eq!(ctx.widgets.len(), 2);
    let second = ctx.widgets[1];
    for (id, encoding) in [
        (ctx.widgets[0], "WinAnsiEncoding"),
        (second, "StandardEncoding"),
    ] {
        let mut appearance = normal(&ctx.doc, id);
        let mut resources = ctx
            .doc
            .dereference(appearance.dict.get(b"Resources").unwrap())
            .unwrap()
            .1
            .as_dict()
            .unwrap()
            .clone();
        let mut fonts = ctx
            .doc
            .dereference(resources.get(b"Font").unwrap())
            .unwrap()
            .1
            .as_dict()
            .unwrap()
            .clone();
        let (name, font) = fonts
            .iter()
            .next()
            .map(|(k, v)| (k.clone(), v.clone()))
            .unwrap();
        assert_eq!(name, b"F1");
        let mut font = ctx
            .doc
            .dereference(&font)
            .unwrap()
            .1
            .as_dict()
            .unwrap()
            .clone();
        font.set("Encoding", encoding);
        fonts.set(name, font);
        resources.set("Font", fonts);
        appearance.dict.set("Resources", resources);
        replace_normal(&mut ctx.doc, id, appearance);
    }
    let before = ctx.doc.objects.clone();
    generate_choice_appearance(r.bindings, &ctx, ctx.widgets[0]).unwrap();
    let error = generate_choice_appearance(r.bindings, &ctx, second)
        .err()
        .expect("another widget's WinAnsi coverage cannot authorize this resource")
        .to_string();
    assert!(error.contains("original choice font"), "{error}");
    assert_eq!(ctx.doc.objects, before);
}

#[test]
fn generated_choices_match_saved_no_op_text_and_preserve_source_artwork() {
    let r = runtime();
    for case in ["combo", "list", "empty", "free", "artwork", "embedded"] {
        let raw = fixture(case);
        let original = pixels(&r, &raw);
        record(&format!("{case}-original"), &raw);
        for changed in [false, true] {
            let mut request = choice_request(&raw);
            if changed {
                request.edit.options[1].display = "Renamed Beta".into();
            }
            let ctx = crate::prepare_choice_edit_pdf(&raw, &request).unwrap().ctx;
            let before = ctx.doc.objects.clone();
            let mut output = ctx.doc.clone();
            for id in ctx.widgets.clone() {
                let generated = generate_choice_appearance(r.bindings, &ctx, id).unwrap();
                assert_eq!(ctx.doc.objects, before, "generator mutated its source");
                let old = normal(&ctx.doc, id);
                let replacement = ChoiceTextBlock::parse(&old.get_plain_content().unwrap())
                    .unwrap()
                    .transplant(
                        &ChoiceTextBlock::parse(&generated.appearance.get_plain_content().unwrap())
                            .unwrap_or_else(|e| {
                                panic!(
                                    "{case} generated text: {e}; {:?}",
                                    String::from_utf8_lossy(
                                        &generated.appearance.get_plain_content().unwrap()
                                    )
                                )
                            }),
                        &generated.font_names,
                    )
                    .unwrap();
                let mut appearance = old;
                appearance.set_plain_content(replacement);
                let w = output.get_dictionary(id).unwrap();
                let ap = output
                    .dereference(w.get(b"AP").unwrap())
                    .unwrap()
                    .1
                    .as_dict()
                    .unwrap()
                    .clone();
                let mut ap = ap;
                ap.set("N", output.add_object(appearance));
                output.get_dictionary_mut(id).unwrap().set("AP", ap);
            }
            let bytes = crate::pdf_output::save(&mut output).unwrap();
            record(&format!("{case}-changed{changed}"), &bytes);
            let output = pixels(&r, &bytes);
            let visible = changed && matches!(case, "combo" | "list" | "artwork" | "embedded");
            assert_eq!(output != original, visible, "{case}, changed={changed}");
            // Keep the source borrowed through generation; output has separate ownership.
            assert_eq!(
                crate::form_choices::inspect_context(&ctx)
                    .unwrap()
                    .model
                    .options[1]
                    .display,
                if changed { "Renamed Beta" } else { "Beta" }
            );
        }
    }
}
#[test]
fn private_snapshot_rejects_small_budgets_without_corrupting_the_native_document() {
    let r = runtime();
    let bytes = fixture("empty");
    let native = crate::form_fill::NativeForm::open(r.bindings, &bytes).unwrap();
    assert!(native.snapshot_private(32).is_err());
    assert!(native.snapshot_private(0).is_err());
    let saved = native.snapshot_private(1024 * 1024).unwrap();
    assert!(saved.len() < 1024 * 1024);
    assert_eq!(
        crate::inspect_choice_pdf(&saved, 0, 0)
            .unwrap()
            .model
            .selection,
        crate::ChoiceSelection::None
    );
    assert!(native.snapshot_private(usize::MAX).is_err());
}
#[test]
fn widget_local_font_wins_over_a_conflicting_form_default() {
    let r = runtime();
    let raw = fixture("combo");
    let mut ctx = crate::form_properties::load(&raw, 0, 0).unwrap();
    let form = ctx.doc.catalog().unwrap().get(b"AcroForm").unwrap().clone();
    let mut form = ctx
        .doc
        .dereference(&form)
        .unwrap()
        .1
        .as_dict()
        .unwrap()
        .clone();
    form.set("DR",lopdf::dictionary!{"Font"=>lopdf::dictionary!{"F1"=>lopdf::dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"}}});
    ctx.doc.catalog_mut().unwrap().set("AcroForm", form);
    let g = generate_choice_appearance(r.bindings, &ctx, ctx.widgets[0]).unwrap();
    let resources = g
        .document
        .dereference(g.appearance.dict.get(b"Resources").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    let fonts = g
        .document
        .dereference(resources.get(b"Font").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    let font = g
        .document
        .dereference(fonts.get(b"F1").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    assert_eq!(
        font.get(b"BaseFont").unwrap().as_name().unwrap(),
        b"Helvetica"
    );
}
#[test]
fn missing_font_invalid_appearance_and_foreign_widget_are_refused() {
    let r = runtime();
    let raw = fixture("combo");
    let mut ctx = crate::form_properties::load(&raw, 0, 0).unwrap();
    assert!(generate_choice_appearance(r.bindings, &ctx, (99999, 0)).is_err());
    ctx.doc
        .get_dictionary_mut(ctx.owner)
        .unwrap()
        .set("DA", Object::string_literal("/Missing 12 Tf 0 g"));
    assert!(generate_choice_appearance(r.bindings, &ctx, ctx.widgets[0]).is_err());
    ctx.doc
        .get_dictionary_mut(ctx.owner)
        .unwrap()
        .set("DA", Object::string_literal("/F1 12 Tf /Bad gs"));
    assert!(generate_choice_appearance(r.bindings, &ctx, ctx.widgets[0]).is_err());
}

#[test]
fn selected_text_cannot_silently_introduce_a_fallback_font() {
    let r = runtime();
    let raw = fixture("combo");
    let mut request = choice_request(&raw);
    request.edit.options[1].display = "Ω字".into();
    let ctx = crate::prepare_choice_edit_pdf(&raw, &request).unwrap().ctx;
    assert!(
        generate_choice_appearance(r.bindings, &ctx, ctx.widgets[0]).is_err(),
        "missing glyphs must not become substituted text"
    );
}

#[test]
fn clearing_all_list_options_generates_an_empty_native_appearance() {
    let r = runtime();
    let raw = fixture("list");
    let mut request = choice_request(&raw);
    request.edit.options.clear();
    request.edit.selection = crate::ChoiceEditSelection::Clear;
    request.edit.defaults = crate::ChoiceEditSelection::Clear;
    let ctx = crate::prepare_choice_edit_pdf(&raw, &request).unwrap().ctx;
    let generated = generate_choice_appearance(r.bindings, &ctx, ctx.widgets[0]).unwrap();
    let c =
        lopdf::content::Content::decode_strict(&generated.appearance.get_plain_content().unwrap())
            .unwrap();
    assert!(!c
        .operations
        .iter()
        .any(|op| matches!(op.operator.as_str(), "Tj" | "TJ" | "'" | "\"")));
}

#[test]
fn absent_current_value_requires_explicit_answer_when_changing_its_fallback_default() {
    let raw = fixture("empty");
    let info = crate::inspect_choice_pdf(&raw, 0, 0).unwrap();
    assert_eq!(info.model.selection, crate::ChoiceSelection::None);
    assert_eq!(info.model.defaults, Some(vec!["a".into()]));
    let edit = crate::ChoiceEdit {
        options: info
            .model
            .options
            .iter()
            .enumerate()
            .map(|(i, o)| crate::ChoiceDraftOption {
                source_index: Some(i),
                export_value: o.export_value.clone(),
                display: o.display.clone(),
            })
            .collect(),
        selection: crate::ChoiceEditSelection::Keep,
        defaults: crate::ChoiceEditSelection::Rows(vec![2]),
        manual_order: false,
    };
    let mut request = crate::SetFormChoicesRequest {
        page_index: 0,
        annotation_index: 0,
        expected_key: info.expected_key,
        edit,
    };
    assert!(
        crate::prepare_choice_edit_pdf(&raw, &request).is_err(),
        "default-only edit silently changes the native fallback answer"
    );
    request.edit.selection = crate::ChoiceEditSelection::Clear;
    let prepared = crate::prepare_choice_edit_pdf(&raw, &request).unwrap();
    assert_eq!(prepared.resolved().selection, crate::ChoiceSelection::None);
    assert!(matches!(
        prepared.ctx.raw_attribute(b"V").unwrap(),
        Some(Object::Null)
    ));
}

#[test]
fn clearing_a_fallback_dropdown_requires_a_new_empty_appearance() {
    let r = runtime();
    let raw = fixture("empty");
    let mut request = choice_request(&raw);
    request.edit.selection = crate::ChoiceEditSelection::Clear;
    let prepared = crate::prepare_choice_edit_pdf(&raw, &request).unwrap();
    assert!(
        prepared.needs_appearance_update(),
        "clearing a displayed fallback must remove the old text"
    );
    let g = generate_choice_appearance(r.bindings, &prepared.ctx, prepared.ctx.widgets[0]).unwrap();
    let c =
        lopdf::content::Content::decode_strict(&g.appearance.get_plain_content().unwrap()).unwrap();
    assert!(!c
        .operations
        .iter()
        .any(|op| matches!(op.operator.as_str(), "Tj" | "TJ" | "'" | "\"")));
}

#[test]
fn adding_defaults_to_an_absent_value_requires_an_answer_but_explicit_empty_does_not() {
    for explicit in [false, true] {
        let raw = fixture("empty");
        let mut ctx = crate::form_properties::load(&raw, 0, 0).unwrap();
        let owner = ctx.doc.get_dictionary_mut(ctx.owner).unwrap();
        owner.remove(b"DV");
        if explicit {
            owner.set("V", Object::Null);
            owner.set("I", Vec::<Object>::new());
        }
        let raw = crate::pdf_output::save(&mut ctx.doc).unwrap();
        let mut request = choice_request(&raw);
        request.edit.defaults = crate::ChoiceEditSelection::Rows(vec![2]);
        let result = crate::prepare_choice_edit_pdf(&raw, &request);
        assert_eq!(result.is_ok(), explicit);
    }
}

fn apply_probe_text(r: &WorkerRuntime, ctx: &crate::form_properties::Context) -> Vec<u8> {
    let mut output = ctx.doc.clone();
    for id in &ctx.widgets {
        let generated = generate_choice_appearance(r.bindings, ctx, *id).unwrap();
        let old = normal(&ctx.doc, *id);
        let source = ChoiceTextBlock::parse(&old.get_plain_content().unwrap())
            .or_else(|_| ChoiceTextBlock::parse_empty_background(&old.get_plain_content().unwrap()))
            .unwrap();
        let new = ChoiceTextBlock::parse(&generated.appearance.get_plain_content().unwrap())
            .or_else(|_| {
                ChoiceTextBlock::parse_empty_background(
                    &generated.appearance.get_plain_content().unwrap(),
                )
            })
            .unwrap();
        let mut ap = old;
        ap.set_plain_content(source.transplant(&new, &generated.font_names).unwrap());
        replace_normal(&mut output, *id, ap);
    }
    crate::pdf_output::save(&mut output).unwrap()
}
fn replace_normal(doc: &mut Document, id: lopdf::ObjectId, appearance: lopdf::Stream) {
    let w = doc.get_dictionary(id).unwrap();
    let ap = doc
        .dereference(w.get(b"AP").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap()
        .clone();
    let mut ap = ap;
    ap.set("N", doc.add_object(appearance));
    doc.get_dictionary_mut(id).unwrap().set("AP", ap);
}
#[test]
fn clear_restore_preserves_choice_values_pixels_and_foreground_artwork() {
    let r = runtime();
    for case in ["combo", "list", "artwork", "embedded"] {
        let mut raw = fixture(case);
        if case == "artwork" {
            // Put foreground artwork directly across the text so incorrect layer
            // restoration cannot pass a crop-only or non-overlapping fixture.
            let mut ctx = crate::form_properties::load(&raw, 0, 0).unwrap();
            for id in ctx.widgets.clone() {
                let mut ap = normal(&ctx.doc, id);
                let mut data = ap.get_plain_content().unwrap();
                let ops = lopdf::content::Content::decode_strict(&data).unwrap();
                let y = ops
                    .operations
                    .iter()
                    .find(|op| op.operator == "Td")
                    .unwrap()
                    .operands[1]
                    .as_float()
                    .unwrap();
                data.extend(format!("\nq 1 0 0 rg 3 {} 16 18 re f Q\n", y - 2.).as_bytes());
                ap.set_plain_content(data);
                replace_normal(&mut ctx.doc, id, ap);
            }
            raw = crate::pdf_output::save(&mut ctx.doc).unwrap();
        }
        record(&format!("transition-{case}-original"), &raw);
        let original = crate::inspect_choice_pdf(&raw, 0, 0).unwrap();
        let mut request = choice_request(&raw);
        request.edit.selection = crate::ChoiceEditSelection::Clear;
        let prepared = crate::prepare_choice_edit_pdf(&raw, &request).unwrap();
        let cleared = apply_probe_text(&r, &prepared.ctx);
        record(&format!("transition-{case}-cleared"), &cleared);
        assert_eq!(
            crate::inspect_choice_pdf(&cleared, 0, 0)
                .unwrap()
                .model
                .selection,
            crate::ChoiceSelection::None
        );
        let ctx = crate::form_properties::load(&cleared, 0, 0).unwrap();
        for id in &ctx.widgets {
            assert!(
                ChoiceTextBlock::parse(&normal(&ctx.doc, *id).get_plain_content().unwrap()).is_ok(),
                "clear lost its text slot"
            );
        }
        let native = crate::form_fill::NativeForm::open(r.bindings, &cleared).unwrap();
        assert!(native.fields(0).unwrap()[0].selected_indices.is_empty());
        drop(native);
        let mut request = choice_request(&cleared);
        request.edit.selection = match &original.model.selection {
            crate::ChoiceSelection::Rows(rows) => crate::ChoiceEditSelection::Rows(rows.clone()),
            _ => unreachable!(),
        };
        let prepared = crate::prepare_choice_edit_pdf(&cleared, &request).unwrap();
        let restored = apply_probe_text(&r, &prepared.ctx);
        record(&format!("transition-{case}-restored"), &restored);
        assert_eq!(
            crate::inspect_choice_pdf(&restored, 0, 0).unwrap().model,
            original.model
        );
        assert!(
            pixels(&r, &restored) == pixels(&r, &raw),
            "{case}: restoring text changed the original pixels/layers"
        );
    }
}

#[test]
fn originally_blank_native_background_supports_no_op_and_new_selection() {
    let r = runtime();
    let raw = fixture("combo");
    let mut request = choice_request(&raw);
    request.edit.selection = crate::ChoiceEditSelection::Clear;
    let mut ctx = crate::prepare_choice_edit_pdf(&raw, &request).unwrap().ctx;
    for id in ctx.widgets.clone() {
        let generated = generate_choice_appearance(r.bindings, &ctx, id).unwrap();
        let content = generated.appearance.get_plain_content().unwrap();
        assert!(ChoiceTextBlock::parse(&content).is_err());
        assert!(ChoiceTextBlock::parse_empty_background(&content).is_ok());
        let mut ap = normal(&ctx.doc, id);
        ap.set_plain_content(content);
        replace_normal(&mut ctx.doc, id, ap);
    }
    let blank = crate::pdf_output::save(&mut ctx.doc).unwrap();
    record("native-empty-original", &blank);
    let keep = crate::prepare_choice_edit_pdf(&blank, &choice_request(&blank)).unwrap();
    let unchanged = apply_probe_text(&r, &keep.ctx);
    record("native-empty-noop", &unchanged);
    assert!(pixels(&r, &blank) == pixels(&r, &unchanged));
    let mut request = choice_request(&blank);
    request.edit.selection = crate::ChoiceEditSelection::Rows(vec![1]);
    let selected = crate::prepare_choice_edit_pdf(&blank, &request).unwrap();
    let output = apply_probe_text(&r, &selected.ctx);
    record("native-empty-selected", &output);
    assert_eq!(
        crate::inspect_choice_pdf(&output, 0, 0)
            .unwrap()
            .model
            .selection,
        crate::ChoiceSelection::Rows(vec![1])
    );
    assert!(
        pixels(&r, &output) == pixels(&r, &raw),
        "new selection differs from the original selected appearance"
    );
}
