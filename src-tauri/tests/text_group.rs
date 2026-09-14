use folio_engine::PdfEngine;
use std::path::{Path, PathBuf};
fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn fixture(path: &Path, stream: &str, rotation: u16) {
    let mut doc = lopdf::Document::with_version("1.7");
    let pages = doc.new_object_id();
    let font = doc
        .add_object(lopdf::dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
    let content = doc.add_object(lopdf::Stream::new(
        lopdf::dictionary! {},
        stream.as_bytes().to_vec(),
    ));
    let page=doc.add_object(lopdf::dictionary! {"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"CropBox"=>vec![40.into(),50.into(),240.into(),350.into()],"Rotate"=>rotation as i64,"Resources"=>lopdf::dictionary!{"Font"=>lopdf::dictionary!{"F1"=>font}},"Contents"=>content});
    doc.objects.insert(
        pages,
        lopdf::dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into(),
    );
    let root = doc.add_object(lopdf::dictionary! {"Type"=>"Catalog","Pages"=>pages});
    doc.trailer.set("Root", root);
    doc.save(path).unwrap();
}
#[test]
fn boundary_spaces_and_split_words_are_logical_and_immutable() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for (pieces, expected) in [
        ("(Hel) Tj (lo) Tj", "Hello"),
        ("(Old) Tj ( words) Tj", "Old words"),
        ("(Old ) Tj (words) Tj", "Old words"),
    ] {
        let path = temp.path().join("source.pdf");
        fixture(
            &path,
            &format!("BT /F1 20 Tf 60 300 Td {pieces} 0 -40 Td (Neighbor) Tj ET"),
            0,
        );
        let original = std::fs::read(&path).unwrap();
        let source = engine.open_document(&path).unwrap();
        let preview = engine.inspect_text_group(&source.id, 0, &[0, 1]).unwrap();
        assert_eq!(preview.text, expected);
        let edited = engine
            .replace_text_group(&source.id, 0, &[0, 1], expected, "New text")
            .unwrap();
        assert!(engine
            .extract_text(&edited.id, 0)
            .unwrap()
            .contains("New text"));
        assert_eq!(
            engine.source_bytes(&source.id).unwrap().as_ref(),
            original.as_slice()
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
        engine.close_document(&edited.id).unwrap();
        engine.close_document(&source.id).unwrap();
    }
}

fn export_plan(source: &folio_engine::DocumentInfo) -> folio_engine::ExportRequest {
    folio_engine::ExportRequest {
        flatten: false,
        pages: vec![folio_engine::PagePlan {
            id: "page".into(),
            source_id: source.id.clone(),
            page_index: 0,
            width: source.pages[0].width,
            height: source.pages[0].height,
            rotation: 0,
            overlays: vec![],
        }],
    }
}
#[test]
fn replacements_and_noop_preserve_cropped_rotations_and_export_references() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    let artifacts = std::env::var_os("FOLIO_GROUP_ARTIFACTS").map(PathBuf::from);
    if let Some(dir) = &artifacts {
        std::fs::create_dir_all(dir).unwrap();
    }
    let mut records = Vec::new();
    let mut refusals = Vec::new();
    for rotation in [0, 90, 180, 270] {
        for (case, pieces, expected) in [
            ("within", "(Hel) Tj (lo) Tj", "Hello"),
            ("leading", "(Old) Tj ( words) Tj", "Old words"),
            ("trailing", "(Old ) Tj (words) Tj", "Old words"),
        ] {
            let path = temp.path().join("source.pdf");
            fixture(
                &path,
                &format!("BT /F1 20 Tf 60 300 Td {pieces} 0 -40 Td (Neighbor) Tj ET"),
                rotation,
            );
            let original = std::fs::read(&path).unwrap();
            let source = engine.open_document(&path).unwrap();
            let inspection = engine.inspect_text_group(&source.id, 0, &[0, 1]);
            // Fixed crop [40,50,240,350] at Courier20: rotated space boundaries
            // differ in exact antialiasing; keep this conservative admission matrix.
            if case != "within" && matches!(rotation, 90 | 270) {
                let error = inspection.unwrap_err();
                assert_eq!(error.to_string(),"invalid request: These pieces cannot be joined without changing character positions or appearance. Select contiguous pieces on the same line.");
                assert!(engine
                    .replace_text_group(&source.id, 0, &[0, 1], expected, "New")
                    .is_err());
                assert_eq!(
                    engine.source_bytes(&source.id).unwrap().as_ref(),
                    original.as_slice()
                );
                refusals.push(serde_json::json!({"case":format!("{case}-{rotation}"),"reason":error.to_string()}));
                engine.close_document(&source.id).unwrap();
                continue;
            }
            assert_eq!(inspection.unwrap().text, expected);
            for (kind, replacement) in [
                ("short", "New"),
                (
                    "equal",
                    if expected == "Hello" {
                        "World"
                    } else {
                        "New words"
                    },
                ),
                ("long", "Longer words"),
                ("noop", expected),
            ] {
                let edited = engine
                    .replace_text_group(&source.id, 0, &[0, 1], expected, replacement)
                    .unwrap_or_else(|e| panic!("{case}-{rotation}-{kind}: {e}"));
                let runs = engine.list_text_runs(&edited.id, 0).unwrap();
                assert_eq!(runs.runs.len(), 2);
                assert_eq!(runs.runs[0].text, replacement);
                assert_eq!(runs.runs[1].text, "Neighbor");
                assert_eq!(edited.pages[0].width, source.pages[0].width);
                assert_eq!(edited.pages[0].height, source.pages[0].height);
                let reference_path = temp.path().join("reference.pdf");
                fixture(
                    &reference_path,
                    &format!("BT /F1 20 Tf 60 300 Td ({replacement}) Tj 0 -40 Td (Neighbor) Tj ET"),
                    rotation,
                );
                let reference = engine.open_document(&reference_path).unwrap();
                assert_eq!(
                    engine.render_page(&edited.id, 0, 1000).unwrap(),
                    engine.render_page(&reference.id, 0, 1000).unwrap(),
                    "{case}-{rotation}-{kind} raster"
                );
                let actual = engine.page_text(&edited.id, 0).unwrap();
                let wanted = engine.page_text(&reference.id, 0).unwrap();
                assert_eq!(actual.characters.len(), wanted.characters.len());
                for (a, b) in actual.characters.iter().zip(&wanted.characters) {
                    assert_eq!(a.text, b.text);
                    for (x, y) in [a.x, a.y, a.width, a.height]
                        .iter()
                        .zip([b.x, b.y, b.width, b.height])
                    {
                        assert!((x - y).abs() <= 0.02, "{case}-{rotation}-{kind} glyph");
                    }
                }
                engine.close_document(&reference.id).unwrap();
                let out = temp.path().join("export.pdf");
                engine.export_pdf(export_plan(&edited), &out).unwrap();
                let reopened = engine.open_document(&out).unwrap();
                assert!(engine
                    .extract_text(&reopened.id, 0)
                    .unwrap()
                    .contains(replacement));
                assert!(engine.export_pdf(export_plan(&edited), &path).is_err());
                assert_eq!(std::fs::read(&path).unwrap(), original);
                if let Some(dir) = &artifacts {
                    let name = format!("{case}-{rotation}-{kind}");
                    std::fs::copy(&path, dir.join(format!("{name}-original.pdf"))).unwrap();
                    std::fs::copy(&out, dir.join(format!("{name}-exported.pdf"))).unwrap();
                    fixture(
                        &dir.join(format!("{name}-reference.pdf")),
                        &format!(
                            "BT /F1 20 Tf 60 300 Td ({replacement}) Tj 0 -40 Td (Neighbor) Tj ET"
                        ),
                        rotation,
                    );
                    records.push(serde_json::json!({"case":name,"originalText":expected,"replacement":replacement,"rotation":rotation}));
                }
                engine.close_document(&reopened.id).unwrap();
                engine.close_document(&edited.id).unwrap();
            }
            assert_eq!(
                engine.source_bytes(&source.id).unwrap().as_ref(),
                original.as_slice()
            );
            engine.close_document(&source.id).unwrap();
        }
    }
    if let Some(dir) = &artifacts {
        std::fs::write(
            dir.join("results.json"),
            serde_json::to_vec_pretty(&records).unwrap(),
        )
        .unwrap();
        std::fs::write(
            dir.join("refusals.json"),
            serde_json::to_vec_pretty(&refusals).unwrap(),
        )
        .unwrap();
    }
}
#[test]
fn refuses_ambiguous_spacing_styles_and_intervening_objects() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for (name, body) in [
        ("rows", "(Old) Tj 0 -30 Td (words) Tj"),
        ("gap", "(Old) Tj 30 0 Td (words) Tj"),
        ("generated", "(Old) Tj 45 0 Td (words) Tj"),
        ("double-space", "(Old  ) Tj (words) Tj"),
        ("trailing-omitted", "(Old) Tj (words  ) Tj"),
        ("space-only", "(Old) Tj ( ) Tj (words) Tj"),
        ("size", "(Old) Tj /F1 18 Tf (words) Tj"),
        ("color", "(Old) Tj 1 0 0 rg (words) Tj"),
        ("transform", "(Old) Tj 1 0.1 0 1 90 300 Tm (words) Tj"),
        ("char-spacing", "1 Tc (Old) Tj (words) Tj"),
        ("word-spacing", "1 Tw (Old) Tj (words) Tj"),
        ("kern", "[(Old) -50] TJ (words) Tj"),
        ("marked", "/Span BMC (Old) Tj (words) Tj EMC"),
        (
            "graphics",
            "(Old) Tj ET 0 0 5 5 re f BT /F1 20 Tf 90 300 Td (words) Tj",
        ),
    ] {
        let path = temp.path().join(format!("{name}.pdf"));
        fixture(&path, &format!("BT /F1 20 Tf 60 300 Td {body} ET"), 0);
        let source = engine.open_document(&path).unwrap();
        assert!(
            engine.inspect_text_group(&source.id, 0, &[0, 1]).is_err(),
            "{name} admitted"
        );
        assert!(
            engine
                .replace_text_group(&source.id, 0, &[0, 1], "Oldwords", "New")
                .is_err(),
            "{name} applied"
        );
        engine.close_document(&source.id).unwrap();
    }
}
#[test]
fn selection_bounds_stale_text_and_invalid_replacements_leave_source_usable() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    let path = temp.path().join("source.pdf");
    fixture(
        &path,
        "BT /F1 20 Tf 60 300 Td (Hel) Tj (lo) Tj 0 -40 Td (Neighbor) Tj ET",
        0,
    );
    let source = engine.open_document(&path).unwrap();
    for indices in [
        vec![],
        vec![0],
        vec![1, 0],
        vec![0, 0],
        vec![0, 2],
        vec![0, 1, 2, 3, 4, 5, 6, 7, 8],
        vec![usize::MAX - 1, usize::MAX],
    ] {
        assert!(engine.inspect_text_group(&source.id, 0, &indices).is_err());
    }
    assert!(engine.inspect_text_group(&source.id, 1, &[0, 1]).is_err());
    assert!(engine
        .replace_text_group(&source.id, 0, &[0, 1], "stale", "New")
        .is_err());
    for replacement in [
        "",
        "two\nlines",
        "very long text that must exceed the crop boundary",
    ] {
        assert!(engine
            .replace_text_group(&source.id, 0, &[0, 1], "Hello", replacement)
            .is_err());
    }
    assert_eq!(
        engine
            .inspect_text_group(&source.id, 0, &[0, 1])
            .unwrap()
            .text,
        "Hello"
    );
    let edited = engine
        .replace_text_group(&source.id, 0, &[0, 1], "Hello", "New")
        .unwrap();
    engine.close_document(&source.id).unwrap();
    assert!(engine.inspect_text_group(&source.id, 0, &[0, 1]).is_err());
    assert!(engine.extract_text(&edited.id, 0).unwrap().contains("New"));
    engine.close_document(&edited.id).unwrap();
}

#[test]
fn packaged_practice_invoice_can_be_grouped() {
    let engine = engine();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples/Edit text together.pdf");
    let source = engine.open_document(&path).unwrap();
    let runs = engine.list_text_runs(&source.id, 0).unwrap();
    for (first, count, expected, replacement) in [
        ("In", 3, "Invoice total", "Invoice grand total"),
        ("Annual ", 2, "Annual report", "Updated annual report"),
        ("Quarterly", 3, "Quarterly sales report", "Quarterly report"),
    ] {
        let start = runs
            .runs
            .iter()
            .position(|r| r.text.trim() == first.trim())
            .unwrap();
        let indices: Vec<_> = runs.runs[start..start + count]
            .iter()
            .map(|r| r.object_index)
            .collect();
        let preview = engine
            .inspect_text_group(&source.id, 0, &indices)
            .unwrap_or_else(|e| panic!("practice {expected}: {e}"));
        assert_eq!(preview.text, expected);
        let edited = engine
            .replace_text_group(&source.id, 0, &indices, &preview.text, replacement)
            .unwrap_or_else(|e| panic!("practice {expected}: {e}"));
        assert!(engine
            .extract_text(&edited.id, 0)
            .unwrap()
            .contains(replacement));
        engine.close_document(&edited.id).unwrap();
    }
    engine.close_document(&source.id).unwrap();
}

#[test]
fn font_identity_is_a_resource_not_a_name_and_tagged_documents_are_refused() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for (case, distinct, tagged) in [
        ("alias", false, false),
        ("distinct", true, false),
        ("tagged", false, true),
    ] {
        let path = temp.path().join(format!("{case}.pdf"));
        fixture(
            &path,
            "BT /F1 20 Tf 60 300 Td (Old ) Tj /F2 20 Tf (words) Tj ET",
            0,
        );
        let mut pdf = lopdf::Document::load(&path).unwrap();
        let page = *pdf.get_pages().values().next().unwrap();
        let first = pdf
            .get_dictionary(page)
            .unwrap()
            .get(b"Resources")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Font")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"F1")
            .unwrap()
            .as_reference()
            .unwrap();
        let second = if distinct {
            pdf.add_object(pdf.get_object(first).unwrap().clone())
        } else {
            first
        };
        pdf.get_dictionary_mut(page)
            .unwrap()
            .get_mut(b"Resources")
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .get_mut(b"Font")
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("F2", second);
        if tagged {
            let root = pdf.trailer.get(b"Root").unwrap().as_reference().unwrap();
            pdf.get_dictionary_mut(root).unwrap().set(
                "StructTreeRoot",
                lopdf::dictionary! {"Type"=>"StructTreeRoot"},
            );
        }
        pdf.save(&path).unwrap();
        let source = engine.open_document(&path).unwrap();
        let preview = engine.inspect_text_group(&source.id, 0, &[0, 1]);
        if !distinct && !tagged {
            assert_eq!(preview.unwrap().text, "Old words");
        } else {
            assert!(preview.is_err(), "{case}");
        }
        engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn verified_embedded_font_group_preserves_original_font_and_rejects_missing_glyphs() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    let asset = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/embedded-text/reportlab-subset.pdf");
    let mut pdf = lopdf::Document::load(asset).unwrap();
    let page = *pdf.get_pages().values().next().unwrap();
    let bytes = pdf.get_page_content(page);
    let mut content = lopdf::content::Content::decode(&bytes).unwrap();
    let ordinal = content
        .operations
        .iter()
        .position(|o| o.operator == "Tj")
        .unwrap();
    let encoded = content.operations[ordinal].operands[0]
        .as_str()
        .unwrap()
        .to_vec();
    content.operations.splice(
        ordinal..=ordinal,
        [
            lopdf::content::Operation::new(
                "Tj",
                vec![lopdf::Object::string_literal(encoded[..2].to_vec())],
            ),
            lopdf::content::Operation::new(
                "Tj",
                vec![lopdf::Object::string_literal(encoded[2..].to_vec())],
            ),
        ],
    );
    let stream = pdf.add_object(lopdf::Stream::new(
        lopdf::dictionary! {},
        content.encode().unwrap(),
    ));
    pdf.get_dictionary_mut(page)
        .unwrap()
        .set("Contents", stream);
    let path = temp.path().join("embedded.pdf");
    pdf.save(&path).unwrap();
    let source = engine.open_document(&path).unwrap();
    let preview = engine.inspect_text_group(&source.id, 0, &[0, 1]).unwrap();
    assert_eq!(preview.text, "Caf\u{e9} old");
    let edited = engine
        .replace_text_group(&source.id, 0, &[0, 1], &preview.text, "Edit caf\u{e9}")
        .unwrap();
    let after = engine.list_text_runs(&edited.id, 0).unwrap();
    assert_eq!(after.runs[0].font_name, preview.font_name);
    assert_eq!(after.runs[0].text, "Edit caf\u{e9}");
    assert!(engine
        .replace_text_group(&source.id, 0, &[0, 1], &preview.text, "\u{100}")
        .is_err());
    engine.close_document(&edited.id).unwrap();
    engine.close_document(&source.id).unwrap();
}

#[test]
fn maximum_explicit_group_and_rotated_direction_guard() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    let path = temp.path().join("eight.pdf");
    fixture(
        &path,
        "BT /F1 20 Tf 60 300 Td (a) Tj (b) Tj (c) Tj (d) Tj (e) Tj (f) Tj (g) Tj (h) Tj ET",
        0,
    );
    let source = engine.open_document(&path).unwrap();
    assert_eq!(
        engine
            .inspect_text_group(&source.id, 0, &[0, 1, 2, 3, 4, 5, 6, 7])
            .unwrap()
            .text,
        "abcdefgh"
    );
    engine.close_document(&source.id).unwrap();
    for matrix in ["0 1 -1 0 100 200", "-1 0 0 -1 200 300", "1 0.2 0 1 60 250"] {
        fixture(
            &path,
            &format!("BT /F1 20 Tf {matrix} Tm (Hel) Tj (lo) Tj ET"),
            90,
        );
        let source = engine.open_document(&path).unwrap();
        assert!(engine
            .inspect_text_group(&source.id, 0, &[0, 1])
            .unwrap_err()
            .to_string()
            .contains("text directions"));
        engine.close_document(&source.id).unwrap();
    }
}
