use folio_engine::{ExportRequest, PagePlan, PdfEngine};
use std::path::{Path, PathBuf};

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}

fn fixture(path: &Path, font: &str, stream: &str, rotation: u16) {
    let mut doc = lopdf::Document::with_version("1.7");
    let pages = doc.new_object_id();
    let font = doc.add_object(
        lopdf::dictionary! {"Type" => "Font", "Subtype" => "Type1", "BaseFont" => font},
    );
    let content = doc.add_object(lopdf::Stream::new(
        lopdf::dictionary! {},
        stream.as_bytes().to_vec(),
    ));
    let page = doc.add_object(lopdf::dictionary! {
        "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(),0.into(),300.into(),400.into()],
        "CropBox" => vec![40.into(),50.into(),240.into(),350.into()], "Rotate" => rotation as i64,
        "Resources" => lopdf::dictionary! {"Font" => lopdf::dictionary! {"F1" => font}}, "Contents" => content
    });
    doc.objects.insert(
        pages,
        lopdf::dictionary! {"Type"=>"Pages", "Kids"=>vec![page.into()], "Count"=>1}.into(),
    );
    let root = doc.add_object(lopdf::dictionary! {"Type"=>"Catalog", "Pages"=>pages});
    doc.trailer.set("Root", root);
    doc.save(path).unwrap();
}

fn plan(source: &folio_engine::DocumentInfo) -> ExportRequest {
    ExportRequest {
        flatten: false,
        pages: vec![PagePlan {
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
fn longer_replacements_use_free_space_preserving_style_and_source_for_all_rotations() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for rotation in [0, 90, 180, 270] {
        let path = temp.path().join(format!("longer-{rotation}.pdf"));
        fixture(
            &path,
            "Helvetica",
            "BT /F1 16 Tf 60 300 Td (Old) Tj 0 -40 Td (Neighbor) Tj ET",
            rotation,
        );
        let file_bytes = std::fs::read(&path).unwrap();
        let source = engine.open_document(&path).unwrap();
        let bytes = engine.source_bytes(&source.id).unwrap();
        let before = engine.list_text_runs(&source.id, 0).unwrap();
        let run = &before.runs[0];
        let edited = engine
            .replace_text(&source.id, 0, run.object_index, "Old", "Longer replacement")
            .unwrap();
        let after = engine.list_text_runs(&edited.id, 0).unwrap();
        let changed = &after.runs[0];
        assert_eq!(changed.text, "Longer replacement");
        assert_eq!(changed.font_name, run.font_name);
        assert_eq!(changed.font_size, run.font_size);
        assert!(
            changed.bounds.width * changed.bounds.height > run.bounds.width * run.bounds.height
        );
        assert_eq!(after.runs[1].text, before.runs[1].text);
        assert_eq!(engine.source_bytes(&source.id).unwrap(), bytes);
        assert_eq!(std::fs::read(&path).unwrap(), file_bytes);
        assert!(engine.extract_text(&source.id, 0).unwrap().contains("Old"));
        let output = temp.path().join(format!("longer-export-{rotation}.pdf"));
        engine.export_pdf(plan(&edited), &output).unwrap();
        let reopened = engine.open_document(&output).unwrap();
        assert!(engine
            .extract_text(&reopened.id, 0)
            .unwrap()
            .contains("Longer replacement"));
        for id in [source.id, edited.id, reopened.id] {
            engine.close_document(&id).unwrap();
        }
    }
}

#[test]
fn replacement_rejects_new_or_increased_text_overlap_and_crop_overflow() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for rotation in [0, 90, 180, 270] {
        for (name, stream, old, replacement, reason) in [
            (
                "same-line-neighbor",
                "BT /F1 16 Tf 60 300 Td (Old ) Tj ET BT /F1 16 Tf 120 300 Td (Neighbor) Tj ET",
                "Old",
                "Longer replacement ",
                "overlap neighboring text",
            ),
            (
                "neighbor",
                "BT /F1 16 Tf 0 1 -1 0 120 295 Tm (II) Tj ET BT /F1 16 Tf 60 300 Td (Old) Tj ET",
                "Old",
                "Longer replacement",
                "overlap neighboring text",
            ),
            (
                "existing-overlap",
                "BT /F1 16 Tf 0 1 -1 0 90 295 Tm (II) Tj ET BT /F1 16 Tf 60 300 Td (Old ) Tj ET",
                "Old",
                "Old extended ",
                "overlap neighboring text",
            ),
            (
                "crop",
                "BT /F1 16 Tf 200 300 Td (Old) Tj ET",
                "Old",
                "Longer replacement",
                "outside the visible page",
            ),
        ] {
            let path = temp.path().join(format!("{name}-{rotation}.pdf"));
            fixture(&path, "Helvetica", stream, rotation);
            let source = engine.open_document(&path).unwrap();
            let bytes = engine.source_bytes(&source.id).unwrap();
            let run = engine
                .list_text_runs(&source.id, 0)
                .unwrap()
                .runs
                .into_iter()
                .find(|run| run.text.trim() == old)
                .unwrap();
            let error = engine
                .replace_text(&source.id, 0, run.object_index, &run.text, replacement)
                .unwrap_err()
                .to_string();
            assert!(error.contains(reason), "{name} {rotation}: {error}");
            assert_eq!(engine.source_bytes(&source.id).unwrap(), bytes);
            engine.close_document(&source.id).unwrap();
        }
    }
}

fn graphic_fixture(path: &Path, graphic: &str, before_text: bool, image: bool) {
    let text = "BT /F1 16 Tf 60 300 Td (Old) Tj ET";
    let stream = if before_text {
        format!("{graphic} {text}")
    } else {
        format!("{text} {graphic}")
    };
    fixture(path, "Helvetica", &stream, 0);
    if image {
        let mut pdf = lopdf::Document::load(path).unwrap();
        let image = pdf.add_object(lopdf::Stream::new(
            lopdf::dictionary! {
                "Type" => "XObject", "Subtype" => "Image", "Width" => 2, "Height" => 1,
                "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8
            },
            vec![200, 210, 220, 30, 40, 50],
        ));
        let page = *pdf.get_pages().values().next().unwrap();
        pdf.get_object_mut(page)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .get_mut(b"Resources")
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("XObject", lopdf::dictionary! {"Im1" => image});
        pdf.save(path).unwrap();
    }
}

#[test]
fn pattern_filled_page_rectangle_in_a_separate_stream_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("pattern-background.pdf");
    fixture(&path, "Helvetica", "BT /F1 16 Tf 60 300 Td (Old) Tj ET", 0);
    let mut pdf = lopdf::Document::load(&path).unwrap();
    let pattern = pdf.add_object(lopdf::Stream::new(
        lopdf::dictionary! {
            "Type" => "Pattern", "PatternType" => 1, "PaintType" => 1, "TilingType" => 1,
            "BBox" => vec![0.into(), 0.into(), 300.into(), 400.into()],
            "XStep" => 300, "YStep" => 400, "Resources" => lopdf::dictionary! {}
        },
        b"1 0 0 rg 120 299 30 20 re f".to_vec(),
    ));
    let background = pdf.add_object(lopdf::Stream::new(
        lopdf::dictionary! {},
        b"q /Pattern cs /P1 scn 0 0 300 400 re f Q".to_vec(),
    ));
    let page = *pdf.get_pages().values().next().unwrap();
    let page = pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap();
    let text = page.get(b"Contents").unwrap().clone();
    page.set("Contents", vec![background.into(), text]);
    page.get_mut(b"Resources")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Pattern", lopdf::dictionary! {"P1" => pattern});
    pdf.save(&path).unwrap();
    let engine = engine();
    let source = engine.open_document(&path).unwrap();
    let bytes = engine.source_bytes(&source.id).unwrap();
    let run = engine.list_text_runs(&source.id, 0).unwrap().runs.remove(0);
    let error = engine
        .replace_text(&source.id, 0, run.object_index, "Old", "Longer replacement")
        .unwrap_err()
        .to_string();
    assert!(error.contains("pattern"), "{error}");
    assert_eq!(engine.source_bytes(&source.id).unwrap(), bytes);
    engine.close_document(&source.id).unwrap();
}

#[test]
fn replacement_respects_graphics_and_allows_page_backgrounds() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    let mut unexpected_successes = Vec::new();
    for (name, graphic, before_text, image, allowed) in [
        (
            "page-border-with-interior-box",
            "0 0 300 400 re 120 299 30 20 re S",
            true,
            false,
            false,
        ),
        (
            "page-with-filled-interior-hole",
            "0 0 300 400 re 120 299 30 20 re f*",
            true,
            false,
            false,
        ),
        (
            "page-sized-trapezoid",
            "0 0 m 300 0 l 240 400 l 0 400 l h f",
            true,
            false,
            false,
        ),
        (
            "background-implicitly-closed-rectangle",
            "0 0 m 300 0 l 300 400 l 0 400 l f",
            true,
            false,
            true,
        ),
        (
            "background-explicit-closed-rectangle",
            "0 0 m 300 0 l 300 400 l 0 400 l h f",
            true,
            false,
            true,
        ),
        (
            "background-transformed-rectangle",
            "q 200 0 0 300 40 50 cm 0 0 1 1 re f Q",
            true,
            false,
            true,
        ),
        (
            "page-sized-skewed-rectangle",
            "q 1 0 0.2 1 0 0 cm 0 0 300 400 re f Q",
            true,
            false,
            false,
        ),
        (
            "thin-line",
            "0.2 w 120 299 m 120 320 l S",
            false,
            false,
            false,
        ),
        (
            "existing-graphic-overlap",
            "0.2 0.3 0.4 rg 80 299 40 20 re f",
            false,
            false,
            false,
        ),
        (
            "drawing",
            "0.2 0.3 0.4 rg 120 299 30 20 re f",
            false,
            false,
            false,
        ),
        (
            "drawing-before",
            "0.2 0.3 0.4 rg 120 299 30 20 re f",
            true,
            false,
            false,
        ),
        (
            "image",
            "q 30 0 0 20 120 299 cm /Im1 Do Q",
            false,
            true,
            false,
        ),
        (
            "image-before",
            "q 30 0 0 20 120 299 cm /Im1 Do Q",
            true,
            true,
            false,
        ),
        (
            "background-drawing",
            "0.8 0.9 1 rg 0 0 300 400 re f 0 0 0 rg",
            true,
            false,
            true,
        ),
        (
            "page-sized-nonuniform-image",
            "q 300 0 0 400 0 0 cm /Im1 Do Q",
            true,
            true,
            false,
        ),
        (
            "foreground-page",
            "0.8 0.9 1 rg 0 0 300 400 re f",
            false,
            false,
            false,
        ),
    ] {
        let path = temp.path().join(format!("{name}.pdf"));
        graphic_fixture(&path, graphic, before_text, image);
        let source = engine.open_document(&path).unwrap();
        let bytes = engine.source_bytes(&source.id).unwrap();
        let run = engine.list_text_runs(&source.id, 0).unwrap().runs.remove(0);
        let result =
            engine.replace_text(&source.id, 0, run.object_index, "Old", "Longer replacement");
        if allowed {
            let edited = result.unwrap_or_else(|error| panic!("{name}: {error}"));
            assert!(engine
                .extract_text(&edited.id, 0)
                .unwrap()
                .contains("Longer replacement"));
            engine.close_document(&edited.id).unwrap();
        } else if let Ok(edited) = result {
            unexpected_successes.push(name);
            engine.close_document(&edited.id).unwrap();
        } else {
            let error = result.unwrap_err().to_string();
            assert!(
                error.contains("overlap neighboring graphics"),
                "{name}: {error}"
            );
        }
        assert_eq!(engine.source_bytes(&source.id).unwrap(), bytes);
        engine.close_document(&source.id).unwrap();
    }
    assert!(
        unexpected_successes.is_empty(),
        "Allowed graphics collisions: {unexpected_successes:?}"
    );
}

#[test]
fn replacement_changes_real_pdf_text_and_preserves_source_for_all_latin_base_fonts_and_rotations() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for font in [
        "Helvetica",
        "Helvetica-Bold",
        "Helvetica-Oblique",
        "Helvetica-BoldOblique",
        "Times-Roman",
        "Times-Bold",
        "Times-Italic",
        "Times-BoldItalic",
        "Courier",
        "Courier-Bold",
        "Courier-Oblique",
        "Courier-BoldOblique",
    ] {
        for rotation in [0, 90, 180, 270] {
            let path = temp.path().join(format!("{font}-{rotation}.pdf"));
            fixture(
                &path,
                font,
                "BT /F1 16 Tf 60 300 Td (Original words) Tj 0 -40 Td (Neighbor) Tj ET",
                rotation,
            );
            let original = engine.open_document(&path).unwrap();
            let original_bytes = engine.source_bytes(&original.id).unwrap();
            let runs = engine.list_text_runs(&original.id, 0).unwrap();
            let run = runs
                .runs
                .iter()
                .find(|r| r.text == "Original words")
                .unwrap();
            assert!(run.supported, "{font} {rotation}: {:?}", run.reason);
            let changed = engine
                .replace_text(&original.id, 0, run.object_index, &run.text, "Edited")
                .unwrap();
            assert_ne!(changed.id, original.id);
            assert_eq!(changed.pages.len(), 1);
            assert_eq!(engine.source_bytes(&original.id).unwrap(), original_bytes);
            assert!(engine
                .extract_text(&original.id, 0)
                .unwrap()
                .contains("Original words"));
            let output = temp.path().join("edited.pdf");
            engine.export_pdf(plan(&changed), &output).unwrap();
            let reopened = engine.open_document(&output).unwrap();
            let text = engine.extract_text(&reopened.id, 0).unwrap();
            assert!(
                text.contains("Edited") && text.contains("Neighbor") && !text.contains("Original"),
                "{text}"
            );
            assert!(engine
                .list_text_runs(&reopened.id, 0)
                .unwrap()
                .runs
                .iter()
                .any(|r| r.text == "Edited" && r.supported));
            assert!(engine.export_pdf(plan(&changed), &path).is_err());
            for id in [original.id, changed.id, reopened.id] {
                engine.close_document(&id).unwrap();
            }
        }
    }
}

#[test]
fn unsupported_layouts_fonts_and_invalid_replacements_leave_source_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for (font, stream) in [
        ("CustomFont", "BT /F1 16 Tf 60 300 Td (Original) Tj ET"),
        (
            "Helvetica",
            "BT /F1 16 Tf 60 300 Td [(Orig) 150 (inal)] TJ ET",
        ),
        (
            "Helvetica",
            "BT /F1 16 Tf 1 0.2 0 1 60 300 Tm (Original) Tj ET",
        ),
        ("Helvetica", "BT /F1 16 Tf 3 Tr 60 300 Td (Original) Tj ET"),
        (
            "Helvetica",
            "q 60 299 20 20 re W n BT /F1 16 Tf 60 300 Td (Original) Tj ET Q",
        ),
    ] {
        let path = temp.path().join("unsupported.pdf");
        fixture(&path, font, stream, 0);
        let original = engine.open_document(&path).unwrap();
        let runs = engine.list_text_runs(&original.id, 0).unwrap();
        assert!(runs.runs.iter().all(|r| !r.supported), "{stream}: {runs:?}");
        assert!(engine
            .replace_text(&original.id, 0, 0, "Original", "Edit")
            .is_err());
        engine.close_document(&original.id).unwrap();
    }
    let path = temp.path().join("valid.pdf");
    fixture(
        &path,
        "Helvetica",
        "BT /F1 16 Tf 60 300 Td (Original) Tj ET",
        0,
    );
    let original = engine.open_document(&path).unwrap();
    let bytes = engine.source_bytes(&original.id).unwrap();
    for replacement in [
        "",
        "\n",
        "non-ascii é",
        "An excessively wide replacement that does not fit",
    ] {
        assert!(
            engine
                .replace_text(&original.id, 0, 0, "Original", replacement)
                .is_err(),
            "{replacement}"
        );
        assert_eq!(engine.source_bytes(&original.id).unwrap(), bytes);
    }
    assert!(engine
        .replace_text(&original.id, 0, 0, "stale", "Edit")
        .is_err());
    assert!(engine
        .replace_text(&original.id, 0, 999, "Original", "Edit")
        .is_err());
    engine.close_document(&original.id).unwrap();
}

#[test]
fn custom_font_encodings_are_explained_and_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("encoding.pdf");
    fixture(
        &path,
        "Helvetica",
        "BT /F1 16 Tf 60 300 Td (Original) Tj ET",
        0,
    );
    let mut pdf = lopdf::Document::load(&path).unwrap();
    for object in pdf.objects.values_mut() {
        if let Ok(dict) = object.as_dict_mut() {
            if dict.has(b"BaseFont") {
                dict.set("Encoding",lopdf::dictionary! {"Type"=>"Encoding", "BaseEncoding"=>"WinAnsiEncoding", "Differences"=>vec![65.into(),lopdf::Object::Name(b"A".to_vec())]});
            }
        }
    }
    pdf.save(&path).unwrap();
    let engine = engine();
    let source = engine.open_document(&path).unwrap();
    let runs = engine.list_text_runs(&source.id, 0).unwrap();
    assert!(!runs.runs[0].supported);
    assert!(runs.runs[0].reason.as_ref().unwrap().contains("encoding"));
    assert!(engine
        .replace_text(&source.id, 0, 0, "Original", "Edit")
        .is_err());
    engine.close_document(&source.id).unwrap();
}

#[test]
fn edits_preserve_colored_text_and_every_pixel_outside_the_changed_run() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for rotation in [0, 90, 180, 270] {
        let path = temp.path().join(format!("appearance-{rotation}.pdf"));
        fixture(&path,"Helvetica","0.2 0.6 0.8 rg 50 100 170 100 re f 0.7 0.1 0.2 rg BT /F1 16 Tf 60 300 Td (Original words) Tj 0 -40 Td (Neighbor) Tj ET",rotation);
        let source = engine.open_document(&path).unwrap();
        let runs = engine.list_text_runs(&source.id, 0).unwrap();
        let run = &runs.runs[0];
        let changed = engine
            .replace_text(&source.id, 0, run.object_index, &run.text, "Edited")
            .unwrap();
        let before = image::load_from_memory(&engine.render_page(&source.id, 0, 600).unwrap())
            .unwrap()
            .to_rgb8();
        let after = image::load_from_memory(&engine.render_page(&changed.id, 0, 600).unwrap())
            .unwrap()
            .to_rgb8();
        let scale = 600.0 / source.pages[0].width;
        let bounds = &run.bounds;
        let mut differences = 0;
        for (x, y, pixel) in before.enumerate_pixels() {
            if pixel != after.get_pixel(x, y) {
                differences += 1;
                assert!(
                    (x as f32) >= bounds.x * scale - 3.0
                        && (x as f32) <= (bounds.x + bounds.width) * scale + 3.0
                        && (y as f32) >= bounds.y * scale - 3.0
                        && (y as f32) <= (bounds.y + bounds.height) * scale + 3.0,
                    "changed pixel outside edited run at {x},{y} for {rotation}"
                );
            }
        }
        assert!(differences > 20);
        let second = engine
            .replace_text(&changed.id, 0, run.object_index, "Edited", "Edit")
            .unwrap();
        assert!(engine.export_pdf(plan(&second), &path).is_err());
        let bytes = engine.source_bytes(&second.id).unwrap();
        let snapshot = temp.path().join("snapshot.pdf");
        std::fs::write(&snapshot, bytes.as_ref()).unwrap();
        let recovered = engine.open_document(&snapshot).unwrap();
        assert!(engine
            .extract_text(&recovered.id, 0)
            .unwrap()
            .contains("Edit"));
        for id in [source.id, changed.id, second.id, recovered.id] {
            engine.close_document(&id).unwrap();
        }
    }
}

#[test]
fn editing_one_page_keeps_other_pages_and_imported_annotations_independent() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    let path = temp.path().join("two-pages.pdf");
    fixture(
        &path,
        "Helvetica",
        "BT /F1 16 Tf 60 300 Td (Original words) Tj ET",
        0,
    );
    let mut pdf = lopdf::Document::load(&path).unwrap();
    let page_id = *pdf.get_pages().values().next().unwrap();
    let page = pdf.get_object(page_id).unwrap().clone();
    let second = pdf.add_object(page);
    let parent = pdf
        .get_object(page_id)
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    let pages = pdf.get_object_mut(parent).unwrap().as_dict_mut().unwrap();
    pages.set("Kids", vec![page_id.into(), second.into()]);
    pages.set("Count", 2);
    pdf.save(&path).unwrap();
    let source = engine.open_document(&path).unwrap();
    let changed = engine
        .replace_text(&source.id, 1, 0, "Original words", "Edited")
        .unwrap();
    assert_eq!(changed.pages.len(), 1);
    for index in [0, 1] {
        assert!(engine
            .extract_text(&source.id, index)
            .unwrap()
            .contains("Original words"));
    }
    let mut request = plan(&source);
    request.pages[0]
        .overlays
        .push(folio_engine::Overlay::Comment(
            folio_engine::CommentOverlay {
                id: "comment".into(),
                x: 10.0,
                y: 10.0,
                text: "Keep comment".into(),
                color: "#336699".into(),
            },
        ));
    let annotated = temp.path().join("annotated.pdf");
    engine.export_pdf(request, &annotated).unwrap();
    let imported = engine.open_document(&annotated).unwrap();
    assert_eq!(imported.pages[0].overlays.len(), 1);
    let edited = engine
        .replace_text(&imported.id, 0, 0, "Original words", "Edited")
        .unwrap();
    assert!(edited.pages[0].overlays.is_empty());
    let mut request = plan(&edited);
    request.pages[0].overlays = imported.pages[0].overlays.clone();
    let final_path = temp.path().join("final.pdf");
    engine.export_pdf(request, &final_path).unwrap();
    let reopened = engine.open_document(&final_path).unwrap();
    assert_eq!(reopened.pages[0].overlays.len(), 1);
    assert!(engine
        .extract_text(&reopened.id, 0)
        .unwrap()
        .contains("Edited"));
    for id in [source.id, changed.id, imported.id, edited.id, reopened.id] {
        engine.close_document(&id).unwrap();
    }
}

#[test]
fn listing_dense_pages_returns_candidates_without_applying_edits() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    let path = temp.path().join("dense.pdf");
    let stream = (0..400)
        .map(|index| {
            format!(
                "BT /F1 8 Tf {} {} Td (Run {index}) Tj ET\n",
                45 + index % 8 * 24,
                60 + index / 8 * 5
            )
        })
        .collect::<String>();
    fixture(&path, "Helvetica", &stream, 0);
    let source = engine.open_document(&path).unwrap();
    let bytes = engine.source_bytes(&source.id).unwrap();
    let started = std::time::Instant::now();
    let runs = engine.list_text_runs(&source.id, 0).unwrap();
    eprintln!("Listed 400 runs in {:?}", started.elapsed());
    assert_eq!(runs.runs.len(), 400);
    assert!(runs.runs.iter().all(|run| run.supported));
    assert_eq!(engine.source_bytes(&source.id).unwrap(), bytes);
    engine.close_document(&source.id).unwrap();
}

#[test]
fn hidden_optional_content_inside_edited_bounds_is_rejected_before_page_copy() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("optional-content.pdf");
    fixture(&path,"Helvetica","BT /F1 16 Tf 60 300 Td (Original words) Tj ET /OC /Layer BDC 1 0 0 rg 65 302 20 5 re f EMC",0);
    let mut pdf = lopdf::Document::load(&path).unwrap();
    let oc = pdf.add_object(
        lopdf::dictionary! {"Type"=>"OCG", "Name"=>lopdf::Object::string_literal("Hidden")},
    );
    let catalog = pdf.trailer.get(b"Root").unwrap().as_reference().unwrap();
    pdf.get_object_mut(catalog).unwrap().as_dict_mut().unwrap().set("OCProperties",lopdf::dictionary! {
        "OCGs"=>vec![oc.into()], "D"=>lopdf::dictionary! {"BaseState"=>"ON", "OFF"=>vec![oc.into()], "Order"=>vec![oc.into()]}
    });
    let page = *pdf.get_pages().values().next().unwrap();
    pdf.get_object_mut(page)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .get_mut(b"Resources")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Properties", lopdf::dictionary! {"Layer"=>oc});
    pdf.save(&path).unwrap();
    let engine = engine();
    let source = engine.open_document(&path).unwrap();
    let bytes = engine.source_bytes(&source.id).unwrap();
    let runs = engine.list_text_runs(&source.id, 0).unwrap();
    assert!(runs.runs.is_empty() || runs.runs.iter().all(|run| !run.supported));
    let reason = runs.reason.or_else(|| runs.runs[0].reason.clone()).unwrap();
    assert!(reason.contains("optional content"), "{reason}");
    assert!(engine
        .replace_text(&source.id, 0, 0, "Original words", "Edited")
        .is_err());
    assert_eq!(engine.source_bytes(&source.id).unwrap(), bytes);
    engine.close_document(&source.id).unwrap();
}

#[test]
fn recovery_restores_derived_text_bytes_and_overlays_after_original_is_removed() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("original.pdf");
    fixture(
        &path,
        "Helvetica",
        "BT /F1 16 Tf 60 300 Td (Original words) Tj ET",
        90,
    );
    let engine = engine();
    let original = engine.open_document(&path).unwrap();
    let derived = engine
        .replace_text(&original.id, 0, 0, "Original words", "Edited")
        .unwrap();
    let derived_bytes = engine.source_bytes(&derived.id).unwrap();
    engine.close_document(&original.id).unwrap();
    std::fs::remove_file(&path).unwrap();

    let overlays = serde_json::json!([
        {"type":"comment","id":"comment","x":20,"y":30,"text":"Keep editable comment","color":"#336699"},
        {"type":"text","id":"note","x":40,"y":50,"text":"Keep editable note","fontSize":14,"color":"#123456","rotation":0}
    ]);
    let workspace = serde_json::json!({"version":1,"activeId":"tab","tabs":[{
        "id":"tab","savedDigest":"before edit","dirty":true,"zoom":125,
        "scrollPosition":{"top":55,"left":0},
        "document":{"name":"original.pdf","selectedPageId":"stable-page","selectedOverlayId":"comment","pages":[{
            "id":"stable-page","sourceId":derived.id,"pageIndex":0,
            "width":derived.pages[0].width,"height":derived.pages[0].height,"rotation":270,"overlays":overlays
        }]}
    }]});
    let root = temp.path().join("recovery");
    let store = folio_engine::RecoveryStore::new(root.clone()).unwrap();
    store.save(&engine, workspace).unwrap();
    let snapshots: Vec<_> = std::fs::read_dir(root.join("sources"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(
        std::fs::read(&snapshots[0]).unwrap(),
        derived_bytes.as_ref()
    );
    engine.close_document(&derived.id).unwrap();
    drop(store);

    let store = folio_engine::RecoveryStore::new(root).unwrap();
    let restored = store.load(&engine).unwrap().expect("saved workspace");
    let tab = &restored["tabs"][0];
    let page = &tab["document"]["pages"][0];
    let restored_id = page["sourceId"].as_str().unwrap();
    assert_ne!(restored_id, derived.id);
    let text = engine.extract_text(restored_id, 0).unwrap();
    assert!(text.contains("Edited"));
    assert!(!text.contains("Original words"));
    assert!(!text.contains("Keep editable note"));
    assert_eq!(engine.source_bytes(restored_id).unwrap(), derived_bytes);
    assert_eq!(page["id"], "stable-page");
    assert_eq!(page["pageIndex"], 0);
    assert_eq!(page["rotation"], 270);
    assert_eq!(page["overlays"], overlays);
    assert_eq!(tab["dirty"], true);
    assert_eq!(tab["zoom"], 125);
    let request: ExportRequest =
        serde_json::from_value(serde_json::json!({"pages":[page]})).unwrap();
    assert!(engine.export_pdf(request.clone(), &path).is_err());
    let output = temp.path().join("recovered.pdf");
    engine.export_pdf(request, &output).unwrap();
    let reopened = engine.open_document(&output).unwrap();
    assert!(engine
        .extract_text(&reopened.id, 0)
        .unwrap()
        .contains("Edited"));
    assert_eq!(reopened.pages[0].overlays.len(), 2);
    engine.close_document(&reopened.id).unwrap();
    engine.close_document(restored_id).unwrap();
}

#[test]
fn replacement_fit_depends_on_glyph_width_and_preserves_text_state_or_rejects_it() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for (index, stream, old, new) in [
        (0, "BT /F1 16 Tf 60 300 Td (WWWW) Tj ET", "WWWW", "iiiiiiii"),
        (1, "BT /F1 16 Tf 60 300 Td (Words) Tj ET", "Words", "Words"),
        (
            2,
            "BT /F1 16 Tf 2 Tc 60 300 Td (Original) Tj ET",
            "Original",
            "Edited",
        ),
        (
            3,
            "BT /F1 16 Tf 5 Tw 60 300 Td (Original words) Tj ET",
            "Original words",
            "Edited words",
        ),
        (
            4,
            "BT /F1 16 Tf 3 Ts 60 300 Td (Original words) Tj ET",
            "Original words",
            "Edited",
        ),
    ] {
        let path = temp.path().join(format!("state-{index}.pdf"));
        fixture(&path, "Helvetica", stream, 0);
        let source = engine.open_document(&path).unwrap();
        let bytes = engine.source_bytes(&source.id).unwrap();
        let changed = engine.replace_text(&source.id, 0, 0, old, new);
        assert_eq!(
            changed.is_ok(),
            !matches!(index, 2 | 3),
            "text state fixture {index}: {changed:?}"
        );
        if let Ok(changed) = changed {
            assert!(engine.extract_text(&changed.id, 0).unwrap().contains(new));
            engine.close_document(&changed.id).unwrap();
        }
        assert_eq!(engine.source_bytes(&source.id).unwrap(), bytes);
        engine.close_document(&source.id).unwrap();
    }
}
