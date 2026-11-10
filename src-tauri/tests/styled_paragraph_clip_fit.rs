use folio_engine::{ParagraphAlignment as Align, PdfEngine, StyledParagraphLayoutRequest, StyledParagraphRun};
use lopdf::{dictionary, Document, Stream};
use std::{fs, path::{Path, PathBuf}};

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()
}

fn fixture(path: &Path, rotation: i32, unit: f32, transformed: bool) -> [f32; 4] {
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let font = pdf.add_object(dictionary!{"Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"Courier"});
    let transform = if transformed {"1.25 0 0 1.25 12 -20 cm"} else {""};
    let stream = pdf.add_object(Stream::new(dictionary!{}, format!(
        "q {transform} 45 540 100 90 re W n BT /F1 12 Tf 100 600 Td (AA) Tj ET Q \
         BT /F1 12 Tf 50 100 Td (Neighbor unchanged) Tj ET"
    ).into_bytes()));
    let page = pdf.add_object(dictionary!{"Type"=>"Page", "Parent"=>pages,
        "MediaBox"=>vec![0.into(),0.into(),600.into(),800.into()], "Rotate"=>rotation,
        "UserUnit"=>unit, "Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}, "Contents"=>stream});
    pdf.objects.insert(pages, dictionary!{"Type"=>"Pages", "Kids"=>vec![page.into()], "Count"=>1}.into());
    let catalog = pdf.add_object(dictionary!{"Type"=>"Catalog", "Pages"=>pages});
    pdf.trailer.set("Root", catalog);
    pdf.save(path).unwrap();
    if transformed {[68.25,655.,193.25,767.5]} else {[45.,540.,145.,630.]}
}

fn request(e: &PdfEngine, id: &str, alignment: Align) -> StyledParagraphLayoutRequest {
    let indices = e.list_occurrence_text_runs(id, 0).unwrap().runs.into_iter()
        .filter(|r| r.text != "Neighbor unchanged").map(|r| r.object_index).collect::<Vec<_>>();
    let s = e.inspect_occurrence_styled_paragraph(id, 0, &indices).unwrap();
    StyledParagraphLayoutRequest { baseline_anchor:None,object_indices: s.object_indices, expected_text: s.text,
        runs: s.runs, style_changes: vec![], bounds: s.bounds, alignment, line_spacing: 1.2 }
}

fn anchor(bounds: [f32; 4], alignment: Align) -> f32 {
    match alignment { Align::Center => (bounds[0]+bounds[2])*0.5, Align::Right => bounds[2], _ => bounds[0] }
}

fn record(name: &str, original: &Path, outputs: &[PathBuf]) {
    if let Some(root) = std::env::var_os("FOLIO_CLIP_FIT_OUTPUTS") {
        let root = PathBuf::from(root).join(name);
        fs::create_dir_all(&root).unwrap();
        fs::copy(original, root.join("original.pdf")).unwrap();
        for (i, output) in outputs.iter().enumerate() { fs::copy(output, root.join(format!("saved-{i}.pdf"))).unwrap(); }
    }
}

fn fitted_roundtrip(name: &str, rotation: i32, unit: f32, transformed: bool, alignment: Align, replacements: [&str; 2]) {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("original.pdf");
    let mut clip = fixture(&input, rotation, unit, transformed);
    if name == "nested-center" {
        let mut pdf = Document::load(&input).unwrap(); let page = *pdf.get_pages().values().next().unwrap();
        let content = String::from_utf8(pdf.get_page_content_with_limit(page,1024*1024).unwrap()).unwrap()
            .replace("re W n BT", "re W n 0.5 0 0 0.5 0 0 cm 100 1090 180 160 re W* n 2 0 0 2 0 0 cm BT");
        let stream = pdf.add_object(Stream::new(dictionary!{},content.into_bytes()));
        pdf.get_dictionary_mut(page).unwrap().set("Contents",stream); pdf.save(&input).unwrap();
        clip = [74.5,661.25,187.,761.25];
    }
    let bytes = fs::read(&input).unwrap();
    let mut current = e.open_document(&input).unwrap();
    let neighbor = e.list_occurrence_text_runs(&current.id, 0).unwrap().runs.into_iter().find(|r| r.text == "Neighbor unchanged").unwrap();
    let mut outputs = Vec::new();
    for (cycle, text) in replacements.into_iter().enumerate() {
        let mut draft = request(&e, &current.id, alignment);
        let before = e.layout_occurrence_styled_paragraph(&current.id, 0, &draft).unwrap();
        draft.runs = vec![StyledParagraphRun { text: text.into(), style_id: 0 }];
        assert!(e.layout_occurrence_styled_paragraph(&current.id, 0, &draft).is_err(), "draft must require expansion");
        draft.bounds = e.fit_occurrence_styled_paragraph(&current.id, 0, &draft)
            .unwrap_or_else(|err| panic!("{name} cycle {cycle}: valid clipped area must fit: {err}"));
        let layout = e.layout_occurrence_styled_paragraph(&current.id, 0, &draft).unwrap();
        let b = layout.pdf_bounds;
        assert!(b[0] >= clip[0] && b[1] >= clip[1] && b[2] <= clip[2] && b[3] <= clip[3], "{name}: {b:?} outside {clip:?}");
        assert!((anchor(b, alignment)-anchor(before.pdf_bounds, alignment)).abs() < 0.001);
        assert!((layout.lines[0].baseline-before.lines[0].baseline).abs() < 0.001,
            "{name} cycle {cycle}: baseline {} -> {}; bounds {:?} -> {:?}",
            before.lines[0].baseline,layout.lines[0].baseline,before.pdf_bounds,layout.pdf_bounds);
        let repeated = e.fit_occurrence_styled_paragraph(&current.id, 0, &draft).unwrap();
        // Public physical coordinates divide/multiply by UserUnit on each call.
        for (a,b) in [repeated.x,repeated.y,repeated.width,repeated.height].into_iter()
            .zip([draft.bounds.x,draft.bounds.y,draft.bounds.width,draft.bounds.height]) {
            assert!((a-b).abs() < 0.0001, "physical round trip drift: {a} vs {b}");
        }
        let preview = e.preview_styled_paragraph_occurrence(&current.id, &[0], 0, &draft)
            .unwrap_or_else(|error| {
                if let Some(root) = std::env::var_os("FOLIO_CLIP_FIT_OUTPUTS") {
                    let root = PathBuf::from(root).parent().unwrap().join("failure-inputs");
                    fs::create_dir_all(&root).unwrap();
                    fs::write(root.join(format!("{name}-{cycle}.pdf")), e.source_bytes(&current.id).unwrap()).unwrap();
                    fs::write(root.join(format!("{name}-{cycle}.json")), serde_json::to_vec_pretty(&draft).unwrap()).unwrap();
                }
                panic!("{name} cycle {cycle}: publication failed: {error}");
            }).document;
        let rendered=e.render_page(&preview.id,0,800).unwrap();
        let image=image::load_from_memory(&rendered).unwrap().to_rgb8();
        let scale=image.width() as f32/current.pages[0].width;
        assert!(image.enumerate_pixels().any(|(x,y,p)| {
            let x=x as f32/scale; let y=y as f32/scale;
            x>=draft.bounds.x && x<=draft.bounds.x+draft.bounds.width
                && y>=draft.bounds.y && y<=draft.bounds.y+draft.bounds.height
                && p.0.iter().any(|channel|*channel<200)
        }),"{name} cycle {cycle}: generated paragraph must have visible ink");
        let output = dir.path().join(format!("saved-{cycle}.pdf"));
        fs::write(&output, e.source_bytes(&preview.id).unwrap()).unwrap();
        let reopened = e.open_document(&output).unwrap();
        let expected = format!("{text} Neighbor unchanged");
        assert_eq!(e.extract_text(&reopened.id, 0).unwrap().split_whitespace().collect::<Vec<_>>(), expected.split_whitespace().collect::<Vec<_>>());
        let kept = e.list_occurrence_text_runs(&reopened.id, 0).unwrap().runs.into_iter().find(|r| r.text == neighbor.text).unwrap();
        assert_eq!(kept.bounds, neighbor.bounds);
        assert!(e.render_page(&reopened.id,0,800).unwrap()==rendered,"saved raster changed on reopen");
        outputs.push(output);
        e.close_document(&preview.id).unwrap();
        e.close_document(&current.id).unwrap();
        current = reopened;
    }
    record(name, &input, &outputs);
    assert_eq!(fs::read(input).unwrap(), bytes);
    e.close_document(&current.id).unwrap();
}

#[test]
fn clipped_width_search_keeps_left_center_and_right_anchors() {
    for (name, rotation, unit, alignment) in [
        ("left", 0, 1., Align::Left), ("center-rotated", 90, 1.7, Align::Center),
        ("right-rotated", 180, 2., Align::Right),
    ] {
        // Keep first-line ink ascent identical when isolating fit anchoring.
        fitted_roundtrip(name, rotation, unit, false, alignment, ["AA AA", "AA AA AA"]);
    }
}

#[test]
fn clipped_height_and_combined_search_keep_source_baseline() {
    fitted_roundtrip("height-rotated", 270, 1., false, Align::Left, ["AA\nBB", "AA\nBB\nCC"]);
    fitted_roundtrip("combined-center", 0, 1., true, Align::Center, ["AAAA\nBBBB", "AAAAAA\nBBBBBB\nCCCCCC"]);
    fitted_roundtrip("combined-right", 0, 1., true, Align::Right, ["AAAA\nBBBB", "AAAAAA\nBBBBBB\nCCCCCC"]);
    fitted_roundtrip("nested-center", 0, 1., true, Align::Center, ["AAAA\nBBBB", "AAAAAA\nBBBBBB\nCCCCCC"]);
}

#[test]
fn legacy_transformed_envelope_reopens_and_reedits_without_wrapper_growth() {
    let e = engine(); let dir = tempfile::tempdir().unwrap();
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/paragraphs");
    let input = fixtures.join("legacy-transformed-clip-v024.pdf");
    let reference = fixtures.join("legacy-transformed-clip-reference.pdf");
    let original = fs::read(&input).unwrap(); let reference_bytes = fs::read(&reference).unwrap();
    let mut current = e.open_document(&input).unwrap(); let control = e.open_document(&reference).unwrap();
    // The old envelope has extractable objects but invisible text in both
    // renderers. The reference restores intended ink with an exact clip.
    let legacy_raster=e.render_page(&current.id,0,800).unwrap();
    let reference_raster=e.render_page(&control.id,0,800).unwrap();
    if let Some(root)=std::env::var_os("FOLIO_CLIP_FIT_OUTPUTS") {
        let root=PathBuf::from(root).parent().unwrap().join("legacy-reference-check");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("legacy.png"),&legacy_raster).unwrap();
        fs::write(root.join("reference.png"),&reference_raster).unwrap();
    }
    assert!(legacy_raster!=reference_raster,"The legacy fixture must reproduce invisible ink");
    assert_eq!(e.extract_text(&current.id,0).unwrap(),e.extract_text(&control.id,0).unwrap());
    let mut outputs = Vec::new(); let mut count = None;
    for (cycle,text) in ["AAAAAA\nBBBBBB", "AAAAAAAA\nBBBBBBBB"].into_iter().enumerate() {
        let mut draft = request(&e,&current.id,Align::Center);
        draft.runs = vec![StyledParagraphRun{text:text.into(),style_id:0}];
        draft.bounds = e.fit_occurrence_styled_paragraph(&current.id,0,&draft).unwrap();
        let edited = e.preview_styled_paragraph_occurrence(&current.id,&[0],0,&draft).unwrap();
        let output = dir.path().join(format!("saved-{cycle}.pdf"));
        let bytes = e.source_bytes(&edited.document.id).unwrap(); fs::write(&output,&bytes).unwrap();
        let pdf = Document::load_mem(&bytes).unwrap(); let page = *pdf.get_pages().values().next().unwrap();
        let size = lopdf::content::Content::decode(&pdf.get_page_content_with_limit(page,1024*1024).unwrap()).unwrap().operations.len();
        if let Some(count) = count { assert_eq!(size,count,"saved clip wrappers must not accumulate"); } count = Some(size);
        let reopened = e.open_document(&output).unwrap();
        assert_eq!(e.extract_text(&reopened.id,0).unwrap().split_whitespace().collect::<Vec<_>>(),format!("{text} Neighbor unchanged").split_whitespace().collect::<Vec<_>>());
        outputs.push(output); e.close_document(&edited.document.id).unwrap(); e.close_document(&current.id).unwrap(); current = reopened;
    }
    // This audit baseline is an explicit rendering reference, not the broken
    // legacy PDF. Retain the true legacy input and provenance separately.
    record("legacy-reference",&reference,&outputs);
    if let Some(root) = std::env::var_os("FOLIO_CLIP_FIT_OUTPUTS") {
        fs::copy(&input,PathBuf::from(root).parent().unwrap().join("legacy-input.pdf")).unwrap();
    }
    assert_eq!(fs::read(&input).unwrap(),original); assert_eq!(fs::read(reference).unwrap(),reference_bytes);
    e.close_document(&current.id).unwrap(); e.close_document(&control.id).unwrap();
}

#[test]
fn fresh_initial_area_does_not_expand_above_a_tight_source_clip() {
    let e = engine(); let dir = tempfile::tempdir().unwrap(); let input = dir.path().join("tight.pdf");
    fixture(&input, 0, 1., false);
    let mut pdf = Document::load(&input).unwrap(); let page = *pdf.get_pages().values().next().unwrap();
    let contents = String::from_utf8(pdf.get_page_content_with_limit(page, 1024*1024).unwrap()).unwrap()
        .replace("45 540 100 90 re W n", "45 595 100 13 re W n");
    let contents = pdf.add_object(Stream::new(dictionary!{}, contents.into_bytes()));
    pdf.get_dictionary_mut(page).unwrap().set("Contents",contents); pdf.save(&input).unwrap();
    let original = fs::read(&input).unwrap(); let source = e.open_document(&input).unwrap();
    let draft = request(&e, &source.id, Align::Left);
    let top = 800.-draft.bounds.y; let bottom = top-draft.bounds.height;
    assert!(top <= 608. && bottom >= 595., "initial suggestion must stay clipped: {bottom}..{top}");
    assert_eq!(fs::read(input).unwrap(), original); e.close_document(&source.id).unwrap();
}

#[test]
fn explicit_valid_area_is_a_control_and_outside_or_impossible_areas_still_refuse() {
    let e = engine(); let dir = tempfile::tempdir().unwrap(); let input = dir.path().join("source.pdf");
    fixture(&input, 0, 1., false); let source = e.open_document(&input).unwrap();
    let original = e.source_bytes(&source.id).unwrap(); let mut draft = request(&e, &source.id, Align::Left);
    draft.runs[0].text = "AA BB".into(); draft.bounds.width = 40.;
    assert!(e.layout_occurrence_styled_paragraph(&source.id, 0, &draft).is_ok());
    let candidate = e.preview_styled_paragraph_occurrence(&source.id, &[0], 0, &draft).unwrap().document; e.close_document(&candidate.id).unwrap();
    draft.bounds.width = 45.01;
    assert!(e.layout_occurrence_styled_paragraph(&source.id, 0, &draft).is_err());
    assert!(e.preview_styled_paragraph_occurrence(&source.id, &[0], 0, &draft).is_err());
    assert!(e.fit_occurrence_styled_paragraph(&source.id, 0, &draft).is_err());
    let mut draft = request(&e, &source.id, Align::Left); draft.runs[0].text = "A".repeat(200);
    assert!(e.fit_occurrence_styled_paragraph(&source.id, 0, &draft).is_err());
    assert_eq!(e.source_bytes(&source.id).unwrap(), original);
    assert_eq!(fs::read(input).unwrap(), original.as_ref()); e.close_document(&source.id).unwrap();
}

#[test]
#[ignore = "Requires immutable local Word originals in FOLIO_CLIP_FIT_INPUTS"]
fn original_word_tables_fit_save_and_reedit_with_complete_source_preservation() {
    let root = PathBuf::from(std::env::var_os("FOLIO_CLIP_FIT_INPUTS").expect("Word original folder"));
    let e = engine(); let dir = tempfile::tempdir().unwrap();
    for name in ["word-False", "word-True"] {
        let input = root.join(format!("{name}.pdf")); let original = fs::read(&input).unwrap();
        let mut current = e.open_document(&input).unwrap(); let untouched = e.render_page(&current.id, 1, 800).unwrap();
        let before = e.extract_text(&current.id, 0).unwrap(); let mut outputs = Vec::new();
        for (cycle, text) in ["Design this", "Design this this"].into_iter().enumerate() {
            let run = e.list_occurrence_text_runs(&current.id, 0).unwrap().runs.into_iter().find(|r| r.text.starts_with("Design")).unwrap();
            let s = e.inspect_occurrence_styled_paragraph(&current.id, 0, &[run.object_index]).unwrap();
            assert_eq!(s.runs.len(), 1);
            let mut draft = StyledParagraphLayoutRequest { baseline_anchor:None,object_indices:s.object_indices, expected_text:s.text,
                runs:vec![StyledParagraphRun{text:text.into(),style_id:s.runs[0].style_id}], style_changes:vec![],
                bounds:s.bounds, alignment:s.alignment.unwrap_or(Align::Left), line_spacing:s.line_spacing.unwrap_or(1.2) };
            draft.bounds = e.fit_occurrence_styled_paragraph(&current.id, 0, &draft)
                .unwrap_or_else(|err| panic!("{name} cycle {cycle}: {err}"));
            let candidate = e.preview_styled_paragraph_occurrence(&current.id, &[0,1], 0, &draft).unwrap();
            let output = dir.path().join(format!("{name}-{cycle}.pdf"));
            fs::write(&output, e.source_bytes(&candidate.document.id).unwrap()).unwrap();
            let reopened = e.open_document(&output).unwrap();
            assert_eq!(e.render_page(&reopened.id, 1, 800).unwrap(), untouched);
            assert_eq!(e.extract_text(&reopened.id, 0).unwrap().split_whitespace().collect::<Vec<_>>(), before.replace("Design",text).split_whitespace().collect::<Vec<_>>());
            outputs.push(output); e.close_document(&candidate.document.id).unwrap(); e.close_document(&current.id).unwrap(); current = reopened;
        }
        record(name, &input, &outputs); assert_eq!(fs::read(input).unwrap(), original); e.close_document(&current.id).unwrap();
    }
}
