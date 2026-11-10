use folio_engine::PdfEngine;
use lopdf::{dictionary, Document, Stream};
use std::path::{Path, PathBuf};
#[path = "support/protected_fixture.rs"]
mod protected_fixture;

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn fixture(path: &Path, program: &str, rotation: i32) {
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let font =
        doc.add_object(dictionary! {"Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"Helvetica"});
    let bold = doc
        .add_object(dictionary! {"Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"Helvetica-Bold"});
    let mono =
        doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
    let contents = doc.add_object(Stream::new(dictionary! {}, program.as_bytes().to_vec()));
    let page = doc.add_object(dictionary! {"Type"=>"Page", "Parent"=>pages,
    "MediaBox"=>vec![0.into(),0.into(),400.into(),500.into()],
    "CropBox"=>vec![20.into(),30.into(),380.into(),480.into()], "Rotate"=>rotation,
    "Resources"=>dictionary! {"Font"=>dictionary! {"F1"=>font,"F2"=>bold,"F3"=>mono}}, "Contents"=>contents});
    doc.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into(),
    );
    let catalog = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    doc.trailer.set("Root", catalog);
    doc.save(path).unwrap();
}

#[test]
fn suggests_split_words_and_wrapped_lines_from_each_member_at_every_rotation() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    for rotation in [0, 90, 180, 270] {
        let path = temp.path().join(format!("question-{rotation}.pdf"));
        fixture(&path,"BT /F1 12 Tf 1 0 0 1 40 400 Tm (a.) Tj 1 0 0 1 60 400 Tm (Old wor) Tj (ds here) Tj 1 0 0 1 60 382 Tm (Next line.) Tj 1 0 0 1 40 364 Tm (b.) Tj 1 0 0 1 60 364 Tm (Another question.) Tj ET",rotation);
        let source = engine.open_document(&path).unwrap();
        let original = engine.source_bytes(&source.id).unwrap();
        let runs = engine.list_text_runs(&source.id, 0).unwrap().runs;
        assert_eq!(runs.len(), 6);
        let indices: Vec<_> = runs[1..4].iter().map(|r| r.object_index).collect();
        let explicit = engine
            .inspect_text_paragraph(&source.id, 0, &indices)
            .unwrap();
        for anchor in &runs[1..4] {
            let suggested = engine
                .suggest_text_paragraph(&source.id, 0, anchor.object_index, &anchor.text)
                .unwrap();
            assert_eq!(suggested.object_indices, indices);
            assert_eq!(suggested.text, "Old words here Next line.");
            assert_eq!(
                serde_json::to_value(&suggested).unwrap(),
                serde_json::to_value(&explicit).unwrap()
            );
        }
        assert!(engine
            .suggest_text_paragraph(&source.id, 0, runs[0].object_index, &runs[0].text)
            .is_err());
        assert!(engine
            .suggest_text_paragraph(&source.id, 0, runs[1].object_index, "Stale text")
            .is_err());
        assert!(engine
            .suggest_text_paragraph(&source.id, 0, usize::MAX, "Missing")
            .is_err());
        assert_eq!(
            engine.source_bytes(&source.id).unwrap().as_ref(),
            original.as_ref()
        );
        assert_eq!(std::fs::read(&path).unwrap(), original.as_ref());
        engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn paragraph_boundaries_keep_columns_lists_and_large_gaps_separate() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let cases = [
        ("BT /F1 12 Tf 60 400 Td (Left first) Tj 0 -18 Td (Left second) Tj 180 18 Td (Right first) Tj 0 -18 Td (Right second) Tj ET",vec![vec![0,1],vec![2,3]]),
        ("BT /F1 12 Tf 60 400 Td (First line) Tj 0 -18 Td (Continued.) Tj 0 -36 Td (Other paragraph) Tj ET",vec![vec![0,1],vec![2]]),
        ("BT /F1 12 Tf 60 400 Td (1. First question) Tj 0 -18 Td (Wrapped line) Tj 0 -18 Td (2. Second question) Tj ET",vec![vec![0,1],vec![2]]),
        ("BT /F1 12 Tf 78 400 Td (Indented first) Tj -18 -18 Td (Continued line) Tj 0 -18 Td (Last line) Tj ET",vec![vec![0,1,2]]),
    ];
    for (n, (program, groups)) in cases.into_iter().enumerate() {
        let path = temp.path().join(format!("boundaries-{n}.pdf"));
        fixture(&path, program, 0);
        let source = engine.open_document(&path).unwrap();
        let runs = engine.list_text_runs(&source.id, 0).unwrap().runs;
        for group in groups {
            for &index in &group {
                let result = engine
                    .suggest_text_paragraph(
                        &source.id,
                        0,
                        runs[index].object_index,
                        &runs[index].text,
                    )
                    .unwrap();
                assert_eq!(
                    result.object_indices,
                    group
                        .iter()
                        .map(|&i| runs[i].object_index)
                        .collect::<Vec<_>>(),
                    "case {n}, anchor {index}"
                );
            }
        }
        engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn mixed_rows_and_oversized_paragraphs_are_refused_without_truncating_the_suggestion() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let programs = [
        "BT /F1 12 Tf 60 400 Td (Normal ) Tj 1 0 0 rg (Red words) Tj ET".to_string(),
        "BT /F1 12 Tf 60 400 Td (Normal ) Tj /F2 12 Tf (Bold words) Tj ET".to_string(),
        format!("BT /F1 12 Tf 60 400 Td {} ET", "(x) Tj ".repeat(65)),
    ];
    for (n, program) in programs.iter().enumerate() {
        let path = temp.path().join(format!("unsupported-{n}.pdf"));
        fixture(&path, program, 0);
        let source = engine.open_document(&path).unwrap();
        let original = engine.source_bytes(&source.id).unwrap();
        let runs = engine.list_text_runs(&source.id, 0).unwrap().runs;
        for run in &runs[..2] {
            assert!(
                engine
                    .suggest_text_paragraph(&source.id, 0, run.object_index, &run.text)
                    .is_err(),
                "case {n}"
            );
        }
        assert_eq!(
            engine.source_bytes(&source.id).unwrap().as_ref(),
            original.as_ref()
        );
        engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn suggestions_retain_edit_permissions_and_refuse_unsupported_orientation() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("protected.pdf");
    fixture(&path, "BT /F1 12 Tf 60 400 Td (Original words) Tj ET", 0);
    let plain = std::fs::read(&path).unwrap();
    for (name, permissions, allowed) in [
        ("reader", lopdf::encryption::Permissions::PRINTABLE, false),
        ("editor", lopdf::encryption::Permissions::MODIFIABLE, true),
    ] {
        let encrypted = protected_fixture::encrypt(&plain, permissions);
        std::fs::write(&path, &encrypted).unwrap();
        let source = engine.open_document_with_password(&path, "reader").unwrap();
        let result = engine.suggest_text_paragraph(&source.id, 0, 0, "Original words");
        assert_eq!(result.is_ok(), allowed, "{name}: {result:?}");
        if !allowed {
            assert!(matches!(
                result,
                Err(folio_engine::EngineError::PermissionDenied(_))
            ));
        }
        assert_eq!(std::fs::read(&path).unwrap(), encrypted);
        engine.close_document(&source.id).unwrap();
    }
    fixture(
        &path,
        "BT /F1 12 Tf 1 0.2 0 1 60 400 Tm (Slanted words) Tj ET",
        0,
    );
    let source = engine.open_document(&path).unwrap();
    assert!(engine
        .suggest_text_paragraph(&source.id, 0, 0, "Slanted words")
        .is_err());
    engine.close_document(&source.id).unwrap();
}

#[test]
fn inline_baseline_changes_do_not_silently_truncate_a_mixed_paragraph() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    for (name, size, y) in [
        ("superscript", 8, 404.0),
        ("subscript", 8, 397.0),
        ("shifted", 12, 400.1),
    ] {
        let path = temp.path().join(format!("{name}.pdf"));
        fixture(&path,&format!("BT /F1 12 Tf 1 0 0 1 60 400 Tm (x) Tj /F1 {size} Tf 1 0 0 1 66 {y} Tm (2) Tj /F1 12 Tf 1 0 0 1 71 400 Tm ( value) Tj ET"),0);
        let source = engine.open_document(&path).unwrap();
        let runs = engine.list_text_runs(&source.id, 0).unwrap().runs;
        assert_eq!(runs.len(), 3);
        for index in [0, 2] {
            let result = engine.suggest_text_paragraph(
                &source.id,
                0,
                runs[index].object_index,
                &runs[index].text,
            );
            assert!(
                result.is_err(),
                "{name} anchor {index} incorrectly suggested {result:?}"
            );
        }
        engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn centered_lines_are_one_suggestion_and_mixed_centered_paragraphs_are_refused() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    for mixed in [false, true] {
        let path = temp.path().join(format!("centered-{mixed}.pdf"));
        let color = if mixed { "1 0 0 rg" } else { "0 0 0 rg" };
        fixture(&path,&format!("BT /F3 12 Tf 1 0 0 1 60 400 Tm (First centered row) Tj {color} 1 0 0 1 96 382 Tm (Next row) Tj 0 0 0 rg 1 0 0 1 60 364 Tm (Third centered row) Tj ET"),0);
        let source = engine.open_document(&path).unwrap();
        let runs = engine.list_text_runs(&source.id, 0).unwrap().runs;
        for run in &runs {
            let result = engine.suggest_text_paragraph(&source.id, 0, run.object_index, &run.text);
            if mixed {
                assert!(result.is_err(), "Partial mixed paragraph: {result:?}");
            } else {
                assert_eq!(
                    result.unwrap().object_indices,
                    runs.iter().map(|r| r.object_index).collect::<Vec<_>>()
                );
            }
        }
        engine.close_document(&source.id).unwrap();
    }
}
