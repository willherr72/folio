use folio_engine::{AnnotationRect, PdfEngine};
use std::path::{Path, PathBuf};

#[test]
fn chosen_font_size_reflows_scaled_rotated_paragraphs_and_preserves_neighbors() {
    let engine=engine();
    for rotation in [0,90,180,270] { for size in [8.0,18.0] {
        let temp=tempfile::tempdir().unwrap();let input=temp.path().join("size.pdf");
        fixture(&input,"BT /F1 12 Tf 0.2 0.3 0.4 rg 2 0 0 2 60 400 Tm (Old words here) Tj ET BT /F1 12 Tf 60 200 Td (Neighbor) Tj ET");
        let mut pdf=lopdf::Document::load(&input).unwrap();let page=*pdf.get_pages().get(&1).unwrap();pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap().set("Rotate",rotation);pdf.save(&input).unwrap();
        let bytes=std::fs::read(&input).unwrap();let source=engine.open_document(&input).unwrap();let selection=engine.inspect_text_paragraph(&source.id,0,&[0]).unwrap();assert_eq!(selection.font_size,24.0);
        let before=engine.list_text_runs(&source.id,0).unwrap().runs;
        let area=displayed_paragraph_area(&AnnotationRect{x:60.0,y:70.0,width:250.0,height:140.0},400.0,500.0,rotation);
        let edited=engine.preview_text_paragraph_with_font_size(&source.id,0,&[0],&selection.text,"New words here with a longer sentence to wrap.",&area,Some(size)).unwrap();
        let after=engine.list_text_runs(&edited.id,0).unwrap().runs;
        let neighbor=after.iter().find(|r|r.text=="Neighbor").unwrap();assert_eq!(neighbor.bounds,before[1].bounds);assert_eq!(neighbor.font_size,before[1].font_size);
        for run in after.iter().filter(|r|r.text!="Neighbor") {assert!((run.font_size-size).abs()<0.01);assert_eq!(run.font_name,selection.font_name);}
        let output=temp.path().join("saved.pdf");engine.export_pdf(folio_engine::ExportRequest{flatten:false,pages:vec![folio_engine::PagePlan{crop: None,id:"p".into(),source_id:edited.id.clone(),page_index:0,width:edited.pages[0].width,height:edited.pages[0].height,rotation:0,overlays:vec![]}]},&output).unwrap();
        let reopened=engine.open_document(&output).unwrap();let runs=engine.list_text_runs(&reopened.id,0).unwrap().runs;let indices:Vec<_>=runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect();
        let inspected=engine.inspect_text_paragraph(&reopened.id,0,&indices).unwrap();assert!((inspected.font_size-size).abs()<0.01);assert!(inspected.text.contains("longer sentence"));
        assert_eq!(engine.source_bytes(&source.id).unwrap().as_ref(),bytes.as_slice());assert_eq!(std::fs::read(&input).unwrap(),bytes);
        for id in [&source.id,&edited.id,&reopened.id]{engine.close_document(id).unwrap();}
    }}
}

#[test]
fn font_size_rejects_invalid_values_and_overflow_without_mutating_source() {
    let temp=tempfile::tempdir().unwrap();let input=temp.path().join("size.pdf");fixture(&input,"BT /F1 12 Tf 60 400 Td (Old words) Tj ET");
    let bytes=std::fs::read(&input).unwrap();let engine=engine();let source=engine.open_document(&input).unwrap();let selection=engine.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
    for size in [f32::NAN,f32::INFINITY,f32::NEG_INFINITY,0.0,-12.0,0.5,300.1] {
        let error=engine.preview_text_paragraph_with_font_size(&source.id,0,&[0],&selection.text,"Old words",&selection.bounds,Some(size)).unwrap_err().to_string();assert!(error.contains("font size"),"{error}");
    }
    let error=engine.preview_text_paragraph_with_font_size(&source.id,0,&[0],&selection.text,"Old words",&selection.bounds,Some(60.0)).unwrap_err().to_string();assert!(error.contains("fit"),"{error}");
    for size in [1.0,300.0] {
        let edited=engine.preview_text_paragraph_with_font_size(&source.id,0,&[0],&selection.text,"X",&AnnotationRect{x:0.0,y:0.0,width:400.0,height:500.0},Some(size)).unwrap();engine.close_document(&edited.id).unwrap();
    }
    assert_eq!(engine.source_bytes(&source.id).unwrap().as_ref(),bytes.as_slice());assert_eq!(std::fs::read(&input).unwrap(),bytes);engine.close_document(&source.id).unwrap();
}

#[test]
fn embedded_font_size_preserves_program_and_missing_glyph_rejection() {
    let engine=engine();let input=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/embedded-text/reportlab-subset.pdf");
    let original=std::fs::read(&input).unwrap();let source=engine.open_document(&input).unwrap();let selection=engine.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
    let area=AnnotationRect{x:30.0,y:84.0,width:130.0,height:55.0};
    for size in [10.0,20.0] {
        let error=engine.preview_text_paragraph_with_font_size(&source.id,0,&[0],&selection.text,"Cafè edit",&area,Some(size)).unwrap_err().to_string();assert!(error.contains("U+00E8"),"{error}");
        let edited=engine.preview_text_paragraph_with_font_size(&source.id,0,&[0],&selection.text,"Café edit\nCafé edit",&area,Some(size)).unwrap();
        let runs=engine.list_text_runs(&edited.id,0).unwrap().runs;let changed:Vec<_>=runs.iter().filter(|r|r.text=="Café edit").collect();assert_eq!(changed.len(),2);
        for run in changed {assert!((run.font_size-size).abs()<0.01);assert_eq!(run.font_name,selection.font_name);}
        assert_eq!(runs.iter().filter(|r|r.text=="Neighbor").count(),1);
        engine.close_document(&edited.id).unwrap();
    }
    assert_eq!(engine.source_bytes(&source.id).unwrap().as_ref(),original.as_slice());assert_eq!(std::fs::read(&input).unwrap(),original);engine.close_document(&source.id).unwrap();
}
#[path="support/protected_fixture.rs"]
mod protected_fixture;

#[test]
fn protected_paragraph_preview_retains_rights_and_original_credentials() {
    use folio_engine::{EngineError,ExportRequest,PagePlan,protection::unlock_pdf};
    for rotation in [0,270] {
    let temp=tempfile::tempdir().unwrap();let path=temp.path().join("paragraph.pdf");
    fixture(&path,"BT /F1 12 Tf 60 400 Td (Old words) Tj ET");
    let mut pdf=lopdf::Document::load(&path).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
    pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap().set("Rotate",rotation);pdf.save(&path).unwrap();
    let original=protected_fixture::encrypt(&std::fs::read(&path).unwrap(),lopdf::encryption::Permissions::MODIFIABLE);
    std::fs::write(&path,&original).unwrap();let e=engine();let source=e.open_document_with_password(&path,"reader").unwrap();
    let selected=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
    let area=displayed_paragraph_area(&AnnotationRect {x:180.0,y:200.0,width:160.0,height:45.0},400.0,500.0,rotation);
    let edited=e.preview_text_paragraph(&source.id,0,&[0],&selected.text,"Old this words",&area).unwrap();
    assert_eq!(edited.protection,source.protection);
    assert!(matches!(e.page_text(&edited.id,0),Err(EngineError::PermissionDenied(_))));
    let plan=PagePlan {crop: None,id:"page".into(),source_id:edited.id.clone(),page_index:0,width:source.pages[0].width,height:source.pages[0].height,rotation:0,overlays:vec![]};
    let output=temp.path().join("edited.pdf");e.export_pdf(ExportRequest{pages:vec![plan],flatten:false},&output).unwrap();
    protected_fixture::record(&format!("paragraph-{rotation}"),&original,&std::fs::read(&output).unwrap());
    for password in ["reader","owner"] {
        let unlocked=unlock_pdf(&std::fs::read(&output).unwrap(),Some(password)).unwrap();
        assert!(lopdf::Document::load_mem(&unlocked.bytes).unwrap().extract_text(&[1]).unwrap().contains("Old this words"));
    }
    assert_eq!(std::fs::read(path).unwrap(),original);
    for id in [&source.id,&edited.id] {e.close_document(id).unwrap();}
    }
}

#[test]
fn paragraph_edit_composes_and_roundtrips_existing_overlays_once() {
    for rotation in [0,90,180,270] {
    let temp=tempfile::tempdir().unwrap();let path=temp.path().join("annotated-paragraph.pdf");
    fixture(&path,"BT /F1 12 Tf 60 400 Td (Old words) Tj ET");
    let mut pdf=lopdf::Document::load(&path).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
    pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap().set("Rotate",rotation);pdf.save(&path).unwrap();
    let engine=engine();let source=engine.open_document(&path).unwrap();
    let selection=engine.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
    let area=displayed_paragraph_area(&AnnotationRect{x:60.0,y:88.0,width:160.0,height:45.0},400.0,500.0,rotation);
    let edited=engine.preview_text_paragraph(&source.id,0,&[0],&selection.text,"Old this words",&area).unwrap();
    let plan:folio_engine::PagePlan=serde_json::from_value(serde_json::json!({
        "id":"page","sourceId":edited.id,"pageIndex":0,"width":source.pages[0].width,"height":source.pages[0].height,"rotation":0,"overlays":[
            {"type":"text","id":"text","x":40,"y":200,"text":"Keep text","fontSize":14,"color":"#0000FF"},
            {"type":"ink","id":"ink","paths":[[{"x":40,"y":260},{"x":150,"y":260}]],"strokeWidth":3,"color":"#FF0000"},
            {"type":"highlight","id":"highlight","rects":[{"x":40,"y":300,"width":100,"height":20}],"color":"#FFFF00","opacity":0.5},
            {"type":"comment","id":"note","x":40,"y":360,"text":"Keep note","color":"#FFFF00"}
        ]
    })).unwrap();
    let preview=engine.render_page_plan(plan.clone(),800).unwrap();
    let output=temp.path().join("edited.pdf");
    engine.export_pdf(folio_engine::ExportRequest{pages:vec![plan],flatten:false},&output).unwrap();
    let reopened=engine.open_document(&output).unwrap();
    assert_eq!(reopened.pages[0].overlays.len(),3);
    assert_eq!(engine.list_native_comments(&reopened.id).unwrap()[0].text,"Keep note");
    assert!(engine.extract_text(&reopened.id,0).unwrap().contains("Old this words"));
    let restored=folio_engine::PagePlan{crop: None,id:"restored".into(),source_id:reopened.id.clone(),page_index:0,width:source.pages[0].width,height:source.pages[0].height,rotation:0,overlays:reopened.pages[0].overlays.clone()};
    assert_eq!(preview,engine.render_page_plan(restored,800).unwrap());
    assert!(engine.extract_text(&source.id,0).unwrap().contains("Old words"));
    for id in [&source.id,&edited.id,&reopened.id] {engine.close_document(id).unwrap();}
    }
}

#[test]
fn paragraph_output_remains_editable_as_a_paragraph_and_single_run() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("repeat.pdf");
    fixture(
        &path,
        "BT /F1 12 Tf 0.001 Tc 60 400 Td (Old words) Tj 0 -100 Td (Neighbor) Tj ET",
    );
    let engine = engine();
    let source = engine.open_document(&path).unwrap();
    let area = AnnotationRect {
        x: 60.0,
        y: 88.0,
        width: 150.0,
        height: 45.0,
    };
    let selection = engine.inspect_text_paragraph(&source.id, 0, &[0]).unwrap();
    let first = engine
        .preview_text_paragraph(
            &source.id,
            0,
            &[0],
            &selection.text,
            "Old this words",
            &area,
        )
        .unwrap();
    let run = engine
        .list_text_runs(&first.id, 0)
        .unwrap()
        .runs
        .into_iter()
        .find(|r| r.text == "Old this words")
        .unwrap();
    let selection = engine
        .inspect_text_paragraph(&first.id, 0, &[run.object_index])
        .unwrap();
    let second = engine
        .preview_text_paragraph(
            &first.id,
            0,
            &[run.object_index],
            &selection.text,
            "New this words",
            &area,
        )
        .unwrap();
    let run = engine
        .list_text_runs(&second.id, 0)
        .unwrap()
        .runs
        .into_iter()
        .find(|r| r.text == "New this words")
        .unwrap();
    let third = engine
        .replace_text(
            &second.id,
            0,
            run.object_index,
            &run.text,
            "New these words",
        )
        .unwrap();
    assert!(engine
        .extract_text(&third.id, 0)
        .unwrap()
        .contains("New these words"));
    for id in [third.id, second.id, first.id, source.id] {
        engine.close_document(&id).unwrap();
    }
}

#[test]
fn single_run_can_be_edited_after_paragraph_reflow() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("single-after.pdf");
    fixture(
        &path,
        "BT /F1 12 Tf 0.001 Tc 60 400 Td (Old words) Tj 0 -100 Td (Neighbor) Tj ET",
    );
    let engine = engine();
    let source = engine.open_document(&path).unwrap();
    let selection = engine.inspect_text_paragraph(&source.id, 0, &[0]).unwrap();
    let first = engine
        .preview_text_paragraph(
            &source.id,
            0,
            &[0],
            &selection.text,
            "Old this words",
            &AnnotationRect {
                x: 60.0,
                y: 88.0,
                width: 150.0,
                height: 45.0,
            },
        )
        .unwrap();
    let run = engine
        .list_text_runs(&first.id, 0)
        .unwrap()
        .runs
        .into_iter()
        .find(|r| r.text == "Old this words")
        .unwrap();
    let second = engine
        .replace_text(&first.id, 0, run.object_index, &run.text, "Old these words")
        .unwrap();
    assert!(engine
        .extract_text(&second.id, 0)
        .unwrap()
        .contains("Old these words"));
    for id in [second.id, first.id, source.id] {
        engine.close_document(&id).unwrap();
    }
}

#[test]
fn negative_bearing_glyphs_fit_inside_the_area() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("bearing.pdf");
    fixture(&path, "BT /F1 12 Tf 60 400 Td (Old words) Tj ET");
    let mut pdf = lopdf::Document::load(&path).unwrap();
    for object in pdf.objects.values_mut() {
        if let Ok(dict) = object.as_dict_mut() {
            if dict.has(b"BaseFont") {
                dict.set("BaseFont", "Times-Italic");
            }
        }
    }
    pdf.save(&path).unwrap();
    let engine = engine();
    let source = engine.open_document(&path).unwrap();
    let selection = engine.inspect_text_paragraph(&source.id, 0, &[0]).unwrap();
    let edited = engine
        .preview_text_paragraph(
            &source.id,
            0,
            &[0],
            &selection.text,
            "jumps",
            &AnnotationRect {
                x: 60.0,
                y: 88.0,
                width: 150.0,
                height: 30.0,
            },
        )
        .unwrap();
    let run = engine.list_text_runs(&edited.id, 0).unwrap().runs.remove(0);
    assert!(run.bounds.x >= 60.0 - 0.02);
    assert_eq!(run.text, "jumps");
    engine.close_document(&edited.id).unwrap();
    engine.close_document(&source.id).unwrap();
}

#[test]
fn unchanged_group_remains_editable_after_paragraph_reflow() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("group-after.pdf");
    fixture(
        &path,
        "BT /F1 12 Tf 60 400 Td (Old words) Tj 0 -100 Td (Invoice ) Tj (total) Tj ET",
    );
    let engine = engine();
    let source = engine.open_document(&path).unwrap();
    let selection = engine.inspect_text_paragraph(&source.id, 0, &[0]).unwrap();
    let edited = engine
        .preview_text_paragraph(
            &source.id,
            0,
            &[0],
            &selection.text,
            "New words",
            &AnnotationRect {
                x: 60.0,
                y: 88.0,
                width: 150.0,
                height: 30.0,
            },
        )
        .unwrap();
    let group = engine.inspect_text_group(&edited.id, 0, &[0, 1]).unwrap();
    assert_eq!(group.text, "Invoice total");
    let grouped = engine
        .replace_text_group(&edited.id, 0, &[0, 1], &group.text, "Invoice grand total")
        .unwrap();
    assert!(engine
        .extract_text(&grouped.id, 0)
        .unwrap()
        .contains("Invoice grand total"));
    for id in [grouped.id, edited.id, source.id] {
        engine.close_document(&id).unwrap();
    }
}

#[test]
fn repeated_edits_do_not_accumulate_graphics_nesting() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("many-edits.pdf");
    fixture(&path, "BT /F1 12 Tf 60 400 Td (Old words) Tj ET");
    let engine = engine();
    let mut source = engine.open_document(&path).unwrap();
    let area = AnnotationRect {
        x: 60.0,
        y: 88.0,
        width: 150.0,
        height: 30.0,
    };
    for iteration in 0..70 {
        let selection = engine.inspect_text_paragraph(&source.id, 0, &[0]).unwrap();
        let next = engine
            .preview_text_paragraph(
                &source.id,
                0,
                &[0],
                &selection.text,
                if iteration % 2 == 0 {
                    "New words"
                } else {
                    "Old words"
                },
                &area,
            )
            .unwrap();
        engine.close_document(&source.id).unwrap();
        source = next;
    }
    engine.close_document(&source.id).unwrap();
}

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn fixture(path: &Path, stream: &str) {
    use lopdf::{dictionary, Document, Stream};
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let font =
        doc.add_object(dictionary! {"Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"Helvetica"});
    let content = doc.add_object(Stream::new(dictionary! {}, stream.as_bytes().to_vec()));
    let page = doc.add_object(dictionary! {"Type"=>"Page", "Parent"=>pages,
    "MediaBox"=>vec![0.into(),0.into(),400.into(),500.into()],
    "Resources"=>dictionary! {"Font"=>dictionary! {"F1"=>font}}, "Contents"=>content});
    doc.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages", "Kids"=>vec![page.into()], "Count"=>1}.into(),
    );
    let root = doc.add_object(dictionary! {"Type"=>"Catalog", "Pages"=>pages});
    doc.trailer.set("Root", root);
    doc.save(path).unwrap();
}

#[test]
fn reflows_split_words_and_preserves_neighbors_and_source() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("paragraph.pdf");
    fixture(&path,"BT /F1 12 Tf 0.001 Tc 1 Tw 1 0 0 1 60 400 Tm (Old wor) Tj (ds here) Tj 0 -18 Td [(Next) -20 ( line.)] TJ 0 -80 Td (Neighbor stays.) Tj ET");
    let engine = engine();
    let source = engine.open_document(&path).unwrap();
    let original = engine.source_bytes(&source.id).unwrap();
    let runs = engine.list_text_runs(&source.id, 0).unwrap().runs;
    let indices: Vec<_> = runs[..3].iter().map(|r| r.object_index).collect();
    let selection = engine
        .inspect_text_paragraph(&source.id, 0, &indices)
        .unwrap();
    assert_eq!(selection.text, "Old words here Next line.");
    assert_eq!(selection.font_size, 12.0);
    let bounds = AnnotationRect {
        x: 60.0,
        y: 88.0,
        width: 130.0,
        height: 60.0,
    };
    let edited = engine
        .preview_text_paragraph(
            &source.id,
            0,
            &indices,
            &selection.text,
            "Old this words here Next line.",
            &bounds,
        )
        .unwrap();
    assert_eq!(engine.source_bytes(&source.id).unwrap(), original);
    let after = engine.list_text_runs(&edited.id, 0).unwrap().runs;
    let neighbor = after.iter().find(|r| r.text == "Neighbor stays.").unwrap();
    assert_eq!(neighbor.bounds, runs.last().unwrap().bounds);
    let text = after
        .iter()
        .filter(|r| r.text != "Neighbor stays.")
        .map(|r| r.text.trim())
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(text, "Old this words here Next line.");
    assert!(after.len() >= 3);
    engine.close_document(&edited.id).unwrap();
    engine.close_document(&source.id).unwrap();
}

#[test]
fn moved_paragraph_preserves_neighbors_crop_coordinates_and_export_rotation() {
    use folio_engine::{ExportRequest,PagePlan};
    for cropped in [false,true] {
        let temp=tempfile::tempdir().unwrap();let path=temp.path().join("position.pdf");
        fixture(&path,"BT /F1 12 Tf 60 400 Td (Original words.) Tj 0 -300 Td (Neighbor stays.) Tj ET");
        if cropped {
            let mut pdf=lopdf::Document::load(&path).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
            pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap().set("CropBox",vec![20.into(),30.into(),390.into(),480.into()]);
            pdf.save(&path).unwrap();
        }
        let engine=engine();let source=engine.open_document(&path).unwrap();let original=std::fs::read(&path).unwrap();
        let before=engine.list_text_runs(&source.id,0).unwrap().runs;
        let selection=engine.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
        let area=AnnotationRect{x:180.0,y:200.0,width:150.0,height:45.0};
        let edited=engine.preview_text_paragraph(&source.id,0,&[0],&selection.text,"Moved this paragraph.",&area).unwrap();
        let after=engine.list_text_runs(&edited.id,0).unwrap().runs;
        let moved=after.iter().find(|run|run.text=="Moved this paragraph.").unwrap();
        // Glyph ink can start right of the text origin because of its side bearing.
        assert!(moved.bounds.x>=area.x-0.02&&moved.bounds.x+moved.bounds.width<=area.x+area.width+0.02,"{:?}",moved.bounds);
        assert!(moved.bounds.y>=area.y-0.02&&moved.bounds.y+moved.bounds.height<=area.y+area.height+0.02);
        let neighbor=after.iter().find(|run|run.text=="Neighbor stays.").unwrap();
        assert_eq!(neighbor.bounds,before[1].bounds);
        assert_eq!(engine.source_bytes(&source.id).unwrap().as_ref(),original.as_slice());
        for (bounds,reason) in [
            (AnnotationRect{x:399.0,y:200.0,..area.clone()},"inside the page"),
            (AnnotationRect{x:before[1].bounds.x,y:before[1].bounds.y,..area.clone()},"overlap")
        ] {
            let error=engine.preview_text_paragraph(&source.id,0,&[0],&selection.text,"Moved this paragraph.",&bounds).unwrap_err().to_string();
            assert!(error.contains(reason),"{error}");
        }
        for rotation in [0,90] {
            let output=temp.path().join(format!("moved-{rotation}.pdf"));
            let plan=PagePlan{crop: None,id:"page".into(),source_id:edited.id.clone(),page_index:0,width:source.pages[0].width,height:source.pages[0].height,rotation,overlays:vec![]};
            engine.export_pdf(ExportRequest{pages:vec![plan],flatten:false},&output).unwrap();
            let reopened=engine.open_document(&output).unwrap();
            assert!(engine.extract_text(&reopened.id,0).unwrap().contains("Moved this paragraph."));
            if let Some(root)=std::env::var_os("FOLIO_PARAGRAPH_POSITION_ARTIFACTS") {
                let root=PathBuf::from(root);std::fs::create_dir_all(&root).unwrap();let prefix=if cropped{"cropped"}else{"regular"};
                std::fs::write(root.join(format!("{prefix}-original.pdf")),&original).unwrap();
                std::fs::copy(&output,root.join(format!("{prefix}-moved-{rotation}.pdf"))).unwrap();
                std::fs::write(root.join(format!("{prefix}.json")),serde_json::to_vec_pretty(&serde_json::json!({"old":selection.bounds,"new":area,"neighbor":before[1].bounds})).unwrap()).unwrap();
            }
            engine.close_document(&reopened.id).unwrap();
        }
        assert_eq!(std::fs::read(&path).unwrap(),original);
        for id in [&source.id,&edited.id] {engine.close_document(id).unwrap();}
    }
}

#[test]
fn rejects_mixed_formatting_overflow_stale_and_collisions() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("limits.pdf");
    fixture(&path,"BT /F1 12 Tf 60 400 Td (First line.) Tj 0 -18 Td (Second line.) Tj 1 0 0 rg 0 -18 Td (Red text.) Tj ET");
    let engine = engine();
    let source = engine.open_document(&path).unwrap();
    let original = engine.source_bytes(&source.id).unwrap();
    let error = engine
        .inspect_text_paragraph(&source.id, 0, &[0, 1, 2])
        .unwrap_err()
        .to_string();
    assert!(error.contains("same font, size, and color"), "{error}");
    let selection = engine
        .inspect_text_paragraph(&source.id, 0, &[0, 1])
        .unwrap();
    for (expected, text, bounds, reason) in [
        ("stale", "New", selection.bounds.clone(), "changed"),
        (
            selection.text.as_str(),
            "Many words that will not fit",
            AnnotationRect {
                x: 60.0,
                y: 88.0,
                width: 20.0,
                height: 12.0,
            },
            "fit",
        ),
        (
            selection.text.as_str(),
            "New words",
            AnnotationRect {
                x: 60.0,
                y: 124.0,
                width: 150.0,
                height: 40.0,
            },
            "overlap",
        ),
    ] {
        let error = engine
            .preview_text_paragraph(&source.id, 0, &[0, 1], expected, text, &bounds)
            .unwrap_err()
            .to_string();
        assert!(error.contains(reason), "{error}");
        assert_eq!(engine.source_bytes(&source.id).unwrap(), original);
    }
    engine.close_document(&source.id).unwrap();
}

fn displayed_paragraph_area(raw: &AnnotationRect, width:f32, height:f32, rotation:i32) -> AnnotationRect {
    let (x,y,w,h)=match rotation {
        0=>(raw.x,raw.y,raw.width,raw.height),
        90=>(height-raw.y-raw.height,raw.x,raw.height,raw.width),
        180=>(width-raw.x-raw.width,height-raw.y-raw.height,raw.width,raw.height),
        270=>(raw.y,width-raw.x-raw.width,raw.height,raw.width),
        _=>unreachable!(),
    };
    AnnotationRect{x,y,width:w,height:h}
}

#[test]
fn intrinsic_rotations_preserve_paragraph_layout_crop_and_reediting() {
    use folio_engine::{ExportRequest,PagePlan};
    for rotation in [90,180,270,0] {
        for cropped in [false,true] {
            let temp=tempfile::tempdir().unwrap();let path=temp.path().join("rotated.pdf");
            fixture(&path,"BT /F1 12 Tf 60 400 Td (Original words.) Tj 0 -18 Td (Next line.) Tj 0 -282 Td (Neighbor stays.) Tj ET");
            let mut pdf=lopdf::Document::load(&path).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
            let page_dict=pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap();page_dict.set("Rotate",rotation);
            if cropped {page_dict.set("CropBox",vec![20.into(),30.into(),390.into(),480.into()]);}
            pdf.save(&path).unwrap();
            let original=std::fs::read(&path).unwrap();let e=engine();let source=e.open_document(&path).unwrap();
            let before=e.list_text_runs(&source.id,0).unwrap().runs;
            let selection=e.inspect_text_paragraph(&source.id,0,&[0,1]).unwrap();
            assert_eq!(selection.text,"Original words. Next line.");
            for run in &before[..2] {
                assert!(run.bounds.x>=selection.bounds.x-0.02&&run.bounds.y>=selection.bounds.y-0.02);
                assert!(run.bounds.x+run.bounds.width<=selection.bounds.x+selection.bounds.width+0.02);
                assert!(run.bounds.y+run.bounds.height<=selection.bounds.y+selection.bounds.height+0.02);
            }
            let (width,height)=if cropped{(370.0,450.0)}else{(400.0,500.0)};
            let raw=AnnotationRect{x:130.0,y:170.0,width:160.0,height:65.0};
            let area=displayed_paragraph_area(&raw,width,height,rotation);
            let replacement="Moved this paragraph with more words.\nSecond line.";
            let edited=e.preview_text_paragraph(&source.id,0,&[0,1],&selection.text,replacement,&area).unwrap();
            assert_eq!(edited.pages[0].width,source.pages[0].width);
            assert_eq!(edited.pages[0].height,source.pages[0].height);
            let after=e.list_text_runs(&edited.id,0).unwrap().runs;
            assert_eq!(after[0].text,"Neighbor stays.");assert_eq!(after[0].bounds,before[2].bounds);
            assert_eq!(after[1..].iter().map(|r|r.text.trim()).collect::<Vec<_>>(),vec!["Moved this paragraph with","more words.","Second line."]);
            for run in &after[1..] {
                assert!(run.bounds.x>=area.x-0.02&&run.bounds.y>=area.y-0.02);
                assert!(run.bounds.x+run.bounds.width<=area.x+area.width+0.02);
                assert!(run.bounds.y+run.bounds.height<=area.y+area.height+0.02);
            }
            let overlap=displayed_paragraph_area(&AnnotationRect{x:if cropped{40.0}else{60.0},y:if cropped{368.0}else{388.0},width:160.0,height:45.0},width,height,rotation);
            let overflow=displayed_paragraph_area(&AnnotationRect{height:12.0,..raw.clone()},width,height,rotation);
            for (bounds,text,reason) in [
                (AnnotationRect{x:source.pages[0].width,..area.clone()},"New","inside the page"),
                (overlap,"New","overlap"),(overflow,replacement,"fit"),
            ] {
                let err=e.preview_text_paragraph(&source.id,0,&[0,1],&selection.text,text,&bounds).unwrap_err().to_string();
                assert!(err.contains(reason),"rotation{rotation}, crop{cropped}: {err}");
            }
            let output=temp.path().join("edited.pdf");
            let plan=PagePlan{crop: None,id:"page".into(),source_id:edited.id.clone(),page_index:0,width:edited.pages[0].width,height:edited.pages[0].height,rotation:0,overlays:vec![]};
            e.export_pdf(ExportRequest{pages:vec![plan],flatten:false},&output).unwrap();
            let reopened=e.open_document(&output).unwrap();
            let runs=e.list_text_runs(&reopened.id,0).unwrap().runs;
            let indices=runs.iter().filter(|r|r.text!="Neighbor stays.").map(|r|r.object_index).collect::<Vec<_>>();
            let checked=e.inspect_text_paragraph(&reopened.id,0,&indices).unwrap();
            let again=e.preview_text_paragraph(&reopened.id,0,&indices,&checked.text,"Edited again.",&area).unwrap();
            assert!(e.extract_text(&again.id,0).unwrap().contains("Edited again."));
            assert_eq!(std::fs::read(&path).unwrap(),original);
            assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),original.as_slice());
            if let Some(root)=std::env::var_os("FOLIO_ROTATED_PARAGRAPH_ARTIFACTS") {
                let root=PathBuf::from(root);std::fs::create_dir_all(&root).unwrap();let prefix=format!("{}-{rotation}",if cropped{"cropped"}else{"regular"});
                std::fs::write(root.join(format!("{prefix}-original.pdf")),&original).unwrap();
                std::fs::copy(&output,root.join(format!("{prefix}-edited.pdf"))).unwrap();
                std::fs::write(root.join(format!("{prefix}.json")),serde_json::to_vec_pretty(&serde_json::json!({"old":selection.bounds,"new":area,"raw":raw,"rotation":rotation})).unwrap()).unwrap();
            }
            for id in [&source.id,&edited.id,&reopened.id,&again.id] {e.close_document(id).unwrap();}
        }
    }
}

#[test]
fn generated_space_after_list_label_does_not_block_reflow() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("list.pdf");
    fixture(&path,"BT /F1 12 Tf 1 0 0 1 40 400 Tm (a.) Tj 1 0 0 1 60 400 Tm (First paragraph.) Tj 1 0 0 1 60 340 Tm (Neighbor.) Tj ET");
    let engine = engine();
    let source = engine.open_document(&path).unwrap();
    let selection = engine.inspect_text_paragraph(&source.id, 0, &[1]).unwrap();
    let edited = engine
        .preview_text_paragraph(
            &source.id,
            0,
            &[1],
            &selection.text,
            "First this paragraph.",
            &AnnotationRect {
                x: 60.0,
                y: 88.0,
                width: 100.0,
                height: 40.0,
            },
        )
        .unwrap();
    let before = engine.list_text_runs(&source.id, 0).unwrap();
    let after = engine.list_text_runs(&edited.id, 0).unwrap();
    assert_eq!(before.runs[0].bounds, after.runs[0].bounds);
    assert_eq!(after.runs[0].text.trim(), "a.");
    engine.close_document(&edited.id).unwrap();
    engine.close_document(&source.id).unwrap();
}

#[test]
fn numeric_advance_keeps_relative_following_text_at_the_same_position() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("relative.pdf");
    fixture(
        &path,
        "BT /F1 1 Tf 0.01 Tc 0.02 Tw 12 0 0 12 60 400 Tm [(Old words) -10000] TJ (Following) Tj ET",
    );
    let engine = engine();
    let source = engine.open_document(&path).unwrap();
    let original = engine.list_text_runs(&source.id, 0).unwrap();
    let selection = engine.inspect_text_paragraph(&source.id, 0, &[0]).unwrap();
    assert_eq!(selection.font_size, 12.0);
    let edited = engine
        .preview_text_paragraph(
            &source.id,
            0,
            &[0],
            &selection.text,
            "Old this words",
            &AnnotationRect {
                x: 60.0,
                y: 88.0,
                width: 90.0,
                height: 45.0,
            },
        )
        .unwrap();
    let after = engine.list_text_runs(&edited.id, 0).unwrap();
    assert_eq!(after.runs[0].text, original.runs[1].text);
    assert!((after.runs[0].bounds.x - original.runs[1].bounds.x).abs() < 0.02);
    assert_eq!(after.runs[0].bounds.y, original.runs[1].bounds.y);
    engine.close_document(&edited.id).unwrap();
    engine.close_document(&source.id).unwrap();
}

#[test]
fn embedded_font_reflow_survives_export_and_rejects_missing_glyphs() {
    let engine = engine();
    for rotation in [0,90,180,270] {
    let temp = tempfile::tempdir().unwrap();
    let fixture=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/embedded-text/reportlab-subset.pdf");
    let mut pdf=lopdf::Document::load(&fixture).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
    pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap().set("Rotate",rotation);
    let input=temp.path().join("embedded.pdf");pdf.save(&input).unwrap();
    let source = engine.open_document(&input).unwrap();
    let selection = engine.inspect_text_paragraph(&source.id, 0, &[0]).unwrap();
    let area = displayed_paragraph_area(&AnnotationRect {x:30.0,y:84.0,width:130.0,height:55.0},300.0,400.0,rotation);
    let error = engine
        .preview_text_paragraph(&source.id, 0, &[0], &selection.text, "Cafè edit", &area)
        .unwrap_err()
        .to_string();
    assert!(error.contains("U+00E8"), "{error}");
    let edited = engine
        .preview_text_paragraph(
            &source.id,
            0,
            &[0],
            &selection.text,
            "Café edit\nCafé edit",
            &area,
        )
        .unwrap();
    let output = temp.path().join("exported.pdf");
    engine
        .export_pdf(
            folio_engine::ExportRequest {
                flatten: false,
                pages: vec![folio_engine::PagePlan {
                    crop: None,
                    id: "page".into(),
                    source_id: edited.id.clone(),
                    page_index: 0,
                    width: edited.pages[0].width,
                    height: edited.pages[0].height,
                    rotation: 0,
                    overlays: vec![],
                }],
            },
            &output,
        )
        .unwrap();
    let reopened = engine.open_document(output).unwrap();
    let runs = engine.list_text_runs(&reopened.id, 0).unwrap();
    assert_eq!(
        runs.runs.iter().filter(|r| r.text == "Café edit").count(),
        2
    );
    assert!(!engine
        .extract_text(&reopened.id, 0)
        .unwrap()
        .contains("Café old"));
    for id in [reopened.id, edited.id, source.id] {
        engine.close_document(&id).unwrap();
    }
    }
}

#[test]
fn selection_and_bounds_are_bounded_and_graphics_are_untouched() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("graphics.pdf");
    fixture(&path,"q 1 1 0 rg 55 380 180 35 re f Q BT /F1 12 Tf 60 400 Td (Highlighted words.) Tj 0 -18 Td (Next line.) Tj 0 -18 Td (Last line.) Tj ET");
    let engine = engine();
    let source = engine.open_document(&path).unwrap();
    let runs = engine.list_text_runs(&source.id, 0).unwrap();
    let indices: Vec<_> = runs.runs.iter().map(|r| r.object_index).collect();
    for invalid in [
        vec![],
        vec![indices[0]; 65],
        vec![indices[1], indices[0]],
        vec![indices[0], indices[2]],
        vec![9999],
    ] {
        assert!(engine
            .inspect_text_paragraph(&source.id, 0, &invalid)
            .is_err());
    }
    let selection = engine
        .inspect_text_paragraph(&source.id, 0, &indices[..1])
        .unwrap();
    for width in [0.0, -1.0, f32::NAN, f32::INFINITY, 5000.0] {
        assert!(engine
            .preview_text_paragraph(
                &source.id,
                0,
                &indices[..1],
                &selection.text,
                "New",
                &AnnotationRect {
                    width,
                    ..selection.bounds.clone()
                }
            )
            .is_err());
    }
    let error = engine
        .preview_text_paragraph(
            &source.id,
            0,
            &indices[..1],
            &selection.text,
            "New",
            &selection.bounds,
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("graphics or highlighting"), "{error}");
    engine.close_document(&source.id).unwrap();
}
