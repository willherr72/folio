use folio_engine::{DocumentInfo, ExportRequest, PagePlan, PdfEngine};
use lopdf::{Document, Object};
use std::path::PathBuf;
#[path = "support/embedded_forms.rs"]
mod fixture;
fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn plan(d: &DocumentInfo, i: usize) -> PagePlan {
    PagePlan {
        crop: None,
        id: format!("p{i}"),
        source_id: d.id.clone(),
        page_index: i,
        width: d.pages[i].width,
        height: d.pages[i].height,
        rotation: 0,
        overlays: vec![],
    }
}

#[test]
fn verified_embedded_shared_occurrence_preserves_other_copies_and_original_font() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for cid in [false, true] {
        for rotation in [0, 90, 180, 270] {
            let path = temp.path().join("original.pdf");
            fixture::fixture(&path, cid, rotation, rotation % 180 == 0, "Café old");
            let original = std::fs::read(&path).unwrap();
            if let Some(root) = std::env::var_os("FOLIO_EMBEDDED_FORM_ARTIFACTS") {
                let root = PathBuf::from(root);
                std::fs::create_dir_all(&root).unwrap();
                std::fs::copy(
                    &path,
                    root.join(format!(
                        "{}-{rotation}-original.pdf",
                        if cid { "cid" } else { "simple" }
                    )),
                )
                .unwrap();
            }
            let source = e.open_document(&path).unwrap();
            if let Some(root) = std::env::var_os("FOLIO_EMBEDDED_FORM_ARTIFACTS") {
                e.export_pdf(
                    ExportRequest {
                        pages: vec![plan(&source, 0)],
                        flatten: false,
                    },
                    PathBuf::from(root).join(format!(
                        "{}-{rotation}-isolated.pdf",
                        if cid { "cid" } else { "simple" }
                    )),
                )
                .unwrap();
            }
            let runs = e.list_form_text_runs(&source.id, 0).unwrap();
            assert_eq!(
                runs.runs.len(),
                2,
                "cid={cid} rotation={rotation}: {:?}",
                runs.reason
            );
            assert!(runs.runs.iter().all(|r| r.supported));
            assert_eq!(runs.runs[0].text, "Café old");
            assert_eq!(
                runs.runs[0].font_display_name.as_deref(),
                Some("DejaVu Serif")
            );
            assert_eq!(
                serde_json::to_value(&runs.runs[0]).unwrap()["isEmbedded"],
                true
            );
            let noop = e
                .replace_form_text(
                    &source.id,
                    0,
                    &runs.runs[0].object_path,
                    "Café old",
                    "Café old",
                )
                .unwrap();
            assert_eq!(
                e.render_page(&source.id, 0, 800).unwrap(),
                e.render_page(&noop.id, 0, 800).unwrap()
            );
            let changed = e
                .replace_form_text(
                    &source.id,
                    0,
                    &runs.runs[0].object_path,
                    "Café old",
                    "Café edit",
                )
                .unwrap();
            let after = e.list_form_text_runs(&changed.id, 0).unwrap();
            assert_eq!(after.runs[0].text, "Café edit");
            assert_eq!(after.runs[1], runs.runs[1]);
            assert_eq!(after.runs[0].font_name, runs.runs[0].font_name);
            assert_eq!(
                fixture::programs(&e.source_bytes(&changed.id).unwrap()),
                fixture::programs(&original)
            );
            let reference = temp.path().join("reference.pdf");
            fixture::fixture(&reference, cid, rotation, rotation % 180 == 0, "Café edit");
            let expected = e.open_document(&reference).unwrap();
            assert_eq!(
                e.render_page(&changed.id, 0, 800).unwrap(),
                e.render_page(&expected.id, 0, 800).unwrap()
            );
            let output = temp.path().join("edited.pdf");
            e.export_pdf(
                ExportRequest {
                    pages: vec![plan(&changed, 0), plan(&source, 1)],
                    flatten: false,
                },
                &output,
            )
            .unwrap();
            let reopened = e.open_document(&output).unwrap();
            assert_eq!(
                e.list_form_text_runs(&reopened.id, 0).unwrap().runs[0].text,
                "Café edit"
            );
            assert_eq!(
                e.list_form_text_runs(&reopened.id, 1).unwrap().runs[0].text,
                "Café old"
            );
            assert_eq!(
                e.render_page(&reopened.id, 1, 800).unwrap(),
                e.render_page(&source.id, 1, 800).unwrap()
            );
            let again = e
                .replace_form_text(
                    &reopened.id,
                    0,
                    &after.runs[0].object_path,
                    "Café edit",
                    "Café old",
                )
                .unwrap();
            assert_eq!(
                e.render_page(&again.id, 0, 800).unwrap(),
                e.render_page(&source.id, 0, 800).unwrap()
            );
            assert_eq!(std::fs::read(&path).unwrap(), original);
            assert_eq!(
                e.source_bytes(&source.id).unwrap().as_ref(),
                original.as_slice()
            );
            if let Some(root) = std::env::var_os("FOLIO_EMBEDDED_FORM_ARTIFACTS") {
                let root = PathBuf::from(root);
                std::fs::create_dir_all(&root).unwrap();
                let prefix = format!("{}-{rotation}", if cid { "cid" } else { "simple" });
                for (file, suffix) in [
                    (&path, "original"),
                    (&reference, "reference"),
                    (&output, "edited"),
                ] {
                    std::fs::copy(file, root.join(format!("{prefix}-{suffix}.pdf"))).unwrap();
                }
            }
            for id in [
                &source.id,
                &noop.id,
                &changed.id,
                &expected.id,
                &reopened.id,
                &again.id,
            ] {
                e.close_document(id).unwrap();
            }
        }
    }
}

#[test]
fn missing_glyph_stale_identity_and_overflow_preserve_embedded_form_source() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for cid in [false, true] {
        let path = temp.path().join("original.pdf");
        fixture::fixture(&path, cid, 0, true, "Café old");
        let original = std::fs::read(&path).unwrap();
        let source = e.open_document(&path).unwrap();
        let runs = e.list_form_text_runs(&source.id, 0).unwrap();
        assert_eq!(runs.runs.len(), 2, "{:?}", runs.reason);
        let error = e
            .replace_form_text(
                &source.id,
                0,
                &runs.runs[0].object_path,
                "Café old",
                "Cafè old",
            )
            .unwrap_err()
            .to_string();
        assert!(error.contains("glyph"), "{error}");
        assert!(error.contains("Choose an explicit substitute"));
        assert!(!error.contains("invalid request: invalid request:"), "{error}");
        for replacement in [" Café old", "Café old "] {
            let changed = e
                .replace_form_text(
                    &source.id,
                    0,
                    &runs.runs[0].object_path,
                    "Café old",
                    replacement,
                )
                .unwrap();
            let after = e.list_form_text_runs(&changed.id, 0).unwrap();
            assert_eq!(after.runs[0].text, replacement);
            assert_eq!(after.runs[1], runs.runs[1]);
            assert_eq!(
                e.source_bytes(&source.id).unwrap().as_ref(),
                original.as_slice()
            );
            e.close_document(&changed.id).unwrap();
        }
        for (expected, replacement) in [
            ("Stale", "Café edit"),
            ("Café old", "   "),
            ("Café old", "Café\nold"),
            (
                "Café old",
                "Old Old Old Old Old Old Old Old Old Old Old Old Old Old Old Old",
            ),
        ] {
            assert!(e
                .replace_form_text(
                    &source.id,
                    0,
                    &runs.runs[0].object_path,
                    expected,
                    replacement
                )
                .is_err());
            assert_eq!(
                e.source_bytes(&source.id).unwrap().as_ref(),
                original.as_slice()
            );
        }
        assert_eq!(std::fs::read(&path).unwrap(), original);
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn malformed_font_mapping_and_odd_cid_strings_remain_uneditable() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for kind in ["mapping", "odd", "ambiguous"] {
        let path = temp.path().join("bad.pdf");
        fixture::fixture(&path, true, 0, false, "Café old");
        let mut pdf = Document::load(&path).unwrap();
        if kind == "mapping" {
            let map = pdf
                .objects
                .values()
                .filter_map(|v| v.as_dict().ok())
                .find_map(|d| d.get(b"ToUnicode").ok().and_then(|v| v.as_reference().ok()))
                .unwrap();
            let stream = pdf.get_object_mut(map).unwrap().as_stream_mut().unwrap();
            let bytes = String::from_utf8(stream.get_plain_content().unwrap())
                .unwrap()
                .replace("<00E9> <00E9>", "<00E9> <0065>");
            stream.set_plain_content(bytes.into_bytes());
        } else if kind == "ambiguous" {
            // Same name and program, separately valid but smaller Unicode map.
            // The native identity alone cannot disambiguate these resources.
            let mut font = pdf
                .objects
                .values()
                .find_map(|o| {
                    o.as_dict().ok().filter(|d| {
                        d.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Type0")
                    })
                })
                .unwrap()
                .clone();
            let mut map = pdf
                .get_object(font.get(b"ToUnicode").unwrap().as_reference().unwrap())
                .unwrap()
                .as_stream()
                .unwrap()
                .clone();
            let text = String::from_utf8(map.get_plain_content().unwrap()).unwrap();
            let count = text
                .lines()
                .find_map(|l| {
                    l.strip_suffix(" beginbfchar")
                        .and_then(|n| n.parse::<usize>().ok())
                })
                .unwrap();
            assert!(text.contains("<0045> <0045>"));
            map.set_plain_content(
                text.replace("<0045> <0045>\n", "")
                    .replace(
                        &format!("{count} beginbfchar"),
                        &format!("{} beginbfchar", count - 1),
                    )
                    .into_bytes(),
            );
            let map = pdf.add_object(map);
            font.set("ToUnicode", map);
            let alias = pdf.add_object(font);
            for object in pdf.objects.values_mut() {
                if let Ok(d) = object.as_dict_mut() {
                    if let Ok(resources) = d.get_mut(b"Resources").and_then(Object::as_dict_mut) {
                        if let Ok(fonts) = resources.get_mut(b"Font").and_then(Object::as_dict_mut)
                        {
                            fonts.set("Alias", alias);
                        }
                    }
                }
            }
        } else {
            for object in pdf.objects.values_mut() {
                if let Ok(s) = object.as_stream_mut() {
                    if s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Form") {
                        let mut content =
                            lopdf::content::Content::decode(&s.get_plain_content().unwrap())
                                .unwrap();
                        for op in &mut content.operations {
                            if op.operator == "Tj" {
                                let mut value = op.operands[0].as_str().unwrap().to_vec();
                                value.pop();
                                op.operands[0] = Object::string_literal(value);
                            }
                        }
                        s.set_plain_content(content.encode().unwrap());
                    }
                }
            }
        }
        pdf.save(&path).unwrap();
        let original = std::fs::read(&path).unwrap();
        let source = e.open_document(&path).unwrap();
        let runs = e.list_form_text_runs(&source.id, 0).unwrap();
        assert!(
            runs.runs.is_empty() && runs.reason.is_some(),
            "{kind}: {runs:?}"
        );
        assert!(e
            .replace_form_text(&source.id, 0, &[1, 0, 0], "Café old", "Café edit")
            .is_err());
        assert_eq!(
            e.source_bytes(&source.id).unwrap().as_ref(),
            original.as_slice()
        );
        e.close_document(&source.id).unwrap();
    }
}
