use folio_engine::{AnnotationRect, ParagraphFormatting, PdfEngine};
use lopdf::{dictionary, Document, Stream};
use std::path::{Path, PathBuf};

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()
}
fn fixture(path: &Path, program: &str, rotation: i32) {
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let font = pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
    let content = pdf.add_object(Stream::new(dictionary! {}, program.as_bytes().to_vec()));
    let page = pdf.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),500.into()],"Rotate"=>rotation,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
    pdf.objects.insert(pages, dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    pdf.trailer.set("Root", root);
    pdf.save(path).unwrap();
}
fn area(rotation: i32) -> AnnotationRect {
    let (x,y,width,height) = match rotation {
        90 => (340.,60.,80.,180.), 180 => (160.,340.,180.,80.),
        270 => (80.,160.,80.,180.), _ => (60.,80.,180.,80.),
    };
    AnnotationRect {x,y,width,height}
}
fn baselines(bytes: &[u8], count: usize) -> Vec<f32> {
    let pdf = Document::load_mem(bytes).unwrap();
    let page = *pdf.get_pages().get(&1).unwrap();
    let content = lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap();
    let values: Vec<_> = content.operations.iter().filter(|op|op.operator=="Tm")
        .map(|op|op.operands[5].as_float().unwrap()).collect();
    values[values.len()-count..].to_vec()
}
fn spacing(selection: &folio_engine::TextParagraphSelection) -> Option<f64> {
    serde_json::to_value(selection).unwrap().get("lineSpacing").and_then(|v|v.as_f64())
}

#[test]
fn infers_regular_baselines_with_fragments_scaling_and_rotation() {
    let e=engine(); let temp=tempfile::tempdir().unwrap();
    for rotation in [0,90,180,270] {
        let path=temp.path().join("source.pdf");
        // The effective size is 12pt although Tf is 6pt. Two runs share row one.
        fixture(&path,"BT /F1 6 Tf 2 0 0 2 60 400 Tm (First) Tj ( row) Tj 2 0 0 2 60 382 Tm (Second row) Tj 2 0 0 2 60 364 Tm (Third) Tj ET",rotation);
        let source=e.open_document(&path).unwrap();
        let selection=e.inspect_text_paragraph(&source.id,0,&[0,1,2,3]).unwrap();
        assert_eq!(spacing(&selection),Some(1.5));
        assert_eq!(spacing(&e.inspect_text_paragraph(&source.id,0,&[0,1]).unwrap()),None);
        e.close_document(&source.id).unwrap();
    }
    let path=temp.path().join("irregular.pdf");
    fixture(&path,"BT /F1 12 Tf 60 400 Td (First) Tj 0 -18 Td (Second) Tj 0 -24 Td (Third) Tj ET",0);
    let source=e.open_document(&path).unwrap();
    assert_eq!(spacing(&e.inspect_text_paragraph(&source.id,0,&[0,1,2]).unwrap()),None);
    e.close_document(&source.id).unwrap();
}

#[test]
fn explicit_spacing_composes_with_size_alignment_rotation_and_save() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for rotation in [0,90,180,270] {for ratio in [0.6,1.0,1.5,2.0,3.0] {
        let path=temp.path().join("source.pdf");
        fixture(&path,"BT /F1 12 Tf 60 400 Td (Original) Tj ET BT /F1 12 Tf 60 250 Td (Neighbor) Tj ET",rotation);
        let original=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();
        let neighbor=e.list_text_runs(&source.id,0).unwrap().runs[1].clone();
        let selection=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
        let formatting:ParagraphFormatting=serde_json::from_value(serde_json::json!({"fontSize":16,"alignment":"center","lineSpacing":ratio})).unwrap();
        let edited=e.preview_text_paragraph_with_format(&source.id,0,&[0],&selection.text,"XXXXXXXXXX\nXX",&area(rotation),&formatting).unwrap();
        let bytes=e.source_bytes(&edited.id).unwrap();let ys=baselines(&bytes,2);
        assert!((ys[0]-ys[1]-16.0*ratio as f32).abs()<0.02,"{rotation} {ratio}: {ys:?}");
        let saved=temp.path().join("saved.pdf");
        e.export_pdf(folio_engine::ExportRequest{flatten:false,pages:vec![folio_engine::PagePlan{crop: None,id:"p".into(),source_id:edited.id.clone(),page_index:0,width:edited.pages[0].width,height:edited.pages[0].height,rotation:0,overlays:vec![]}]},&saved).unwrap();
        let reopened=e.open_document(&saved).unwrap();let runs=e.list_text_runs(&reopened.id,0).unwrap().runs;
        assert_eq!(runs.iter().find(|r|r.text=="Neighbor").unwrap().bounds,neighbor.bounds);
        let indices:Vec<_>=runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect();
        let after=e.inspect_text_paragraph(&reopened.id,0,&indices).unwrap();
        assert!((spacing(&after).unwrap()-ratio).abs()<0.002);
        assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),original.as_slice());
        assert_eq!(std::fs::read(&path).unwrap(),original);
        for id in [&source.id,&edited.id,&reopened.id]{e.close_document(id).unwrap();}
    }}
}

#[test]
fn omitted_spacing_preserves_inferred_leading_and_unknown_uses_default() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for (program,indices,expected) in [
        ("BT /F1 12 Tf 60 400 Td (Original) Tj 0 -18 Td (Second) Tj ET",vec![0,1],24.0),
        ("BT /F1 12 Tf 60 400 Td (Original) Tj ET",vec![0],19.2),
    ] {
        let path=temp.path().join("source.pdf");fixture(&path,program,0);
        let source=e.open_document(&path).unwrap();let selection=e.inspect_text_paragraph(&source.id,0,&indices).unwrap();
        let edited=e.preview_text_paragraph_with_font_size(&source.id,0,&indices,&selection.text,"First\nSecond",&area(0),Some(16.0)).unwrap();
        let ys=baselines(&e.source_bytes(&edited.id).unwrap(),2);assert!((ys[0]-ys[1]-expected).abs()<0.02,"{ys:?}");
        for id in [&source.id,&edited.id]{e.close_document(id).unwrap();}
    }
}

#[test]
fn spacing_counts_blank_lines_and_refuses_overflow_or_invalid_values() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");
    fixture(&path,"BT /F1 12 Tf 60 400 Td (Original) Tj ET",0);let source=e.open_document(&path).unwrap();
    let selection=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
    for ratio in [0.0,0.59,3.01,-1.0] {
        let formatting=serde_json::from_value(serde_json::json!({"lineSpacing":ratio})).unwrap();
        let error=e.preview_text_paragraph_with_format(&source.id,0,&[0],&selection.text,"First\nSecond",&area(0),&formatting).unwrap_err();
        assert!(error.to_string().contains("line spacing"),"{error}");
    }
    // Rust callers can supply values that JSON cannot represent.
    for ratio in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let formatting=ParagraphFormatting{line_spacing:Some(ratio),..Default::default()};
        let error=e.preview_text_paragraph_with_format(&source.id,0,&[0],&selection.text,"First\nSecond",&area(0),&formatting).unwrap_err();
        assert!(error.to_string().contains("line spacing"),"{error}");
    }
    let formatting=serde_json::from_value(serde_json::json!({"lineSpacing":2.0})).unwrap();
    let edited=e.preview_text_paragraph_with_format(&source.id,0,&[0],&selection.text,"First\n\nSecond",&area(0),&formatting).unwrap();
    let ys=baselines(&e.source_bytes(&edited.id).unwrap(),2);assert!((ys[0]-ys[1]-48.0).abs()<0.02);
    let mut short=area(0);short.height=50.0;
    assert!(e.preview_text_paragraph_with_format(&source.id,0,&[0],&selection.text,"First\n\nSecond",&short,&formatting).unwrap_err().to_string().contains("does not fit"));
    for id in [&source.id,&edited.id]{e.close_document(id).unwrap();}
}

#[test]
fn boundary_spacing_survives_coordinate_rounding_on_reselection_and_reedit() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for rotation in [0,90,180,270] {for (size,ratio,count) in [(14.0,0.6,2),(12.0,0.6,3),(16.0,0.6,4),(13.7,3.0,2)] {
        let path=temp.path().join("source.pdf");fixture(&path,"BT /F1 12 Tf 60 400 Td (Original) Tj ET",rotation);
        let source=e.open_document(&path).unwrap();let replacement=vec!["First";count].join("\n");
        let formatting=ParagraphFormatting{font_size:Some(size),line_spacing:Some(ratio),..Default::default()};
        let edited=e.preview_text_paragraph_with_format(&source.id,0,&[0],"Original",&replacement,&area(rotation),&formatting).unwrap();
        let path=temp.path().join("saved.pdf");e.export_pdf(folio_engine::ExportRequest{flatten:false,pages:vec![folio_engine::PagePlan{crop: None,id:"p".into(),source_id:edited.id.clone(),page_index:0,width:edited.pages[0].width,height:edited.pages[0].height,rotation:0,overlays:vec![]}]},&path).unwrap();
        let reopened=e.open_document(&path).unwrap();let runs=e.list_text_runs(&reopened.id,0).unwrap().runs;
        assert_eq!(runs.len(),count);
        let indices:Vec<_>=runs.iter().map(|run|run.object_index).collect();
        let selection=e.inspect_text_paragraph(&reopened.id,0,&indices).unwrap_or_else(|error|panic!("{rotation} {size} {ratio} {count}: {error}"));
        assert!((selection.line_spacing.unwrap()-ratio).abs()<0.001);
        if ratio<1.65 {for run in &runs {
            assert_eq!(run.text.trim(),"First");
            let suggested=e.suggest_text_paragraph(&reopened.id,0,run.object_index,&run.text).unwrap();
            assert_eq!(suggested.object_indices,indices,"{rotation} {size} {ratio} {count}");
        }}
        let again=e.preview_text_paragraph_with_format(&reopened.id,0,&indices,&selection.text,&replacement,&area(rotation),&ParagraphFormatting::default()).unwrap();
        for id in [&source.id,&edited.id,&reopened.id,&again.id]{e.close_document(id).unwrap();}
    }}
}
