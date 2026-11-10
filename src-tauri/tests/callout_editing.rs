use folio_engine::{DocumentInfo, PdfEngine, StyledParagraphLayoutRequest, ParagraphAlignment};
use lopdf::{dictionary, Document, Stream};
use std::{fs, path::{Path, PathBuf}};

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()
}

fn fixture(path: &Path, border: &str, extra: &str) {
    let mut pdf=Document::with_version("1.7"); let pages=pdf.new_object_id();
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let content=pdf.add_object(Stream::new(dictionary!{},format!(
        "q {border} Q BT /F1 12 Tf 70 300 Td (Contact team) Tj ET \
         BT /F1 12 Tf 50 100 Td (Neighbor unchanged) Tj ET {extra}"
    ).into_bytes()));
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,
        "MediaBox"=>vec![0.into(),0.into(),400.into(),400.into()],
        "Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
    pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());
    let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages}); pdf.trailer.set("Root",catalog);
    pdf.save(path).unwrap();
}

fn save(e: &PdfEngine, doc: &DocumentInfo, path: &Path) -> DocumentInfo {
    fs::write(path,e.source_bytes(&doc.id).unwrap()).unwrap();
    e.open_document(path).unwrap()
}

fn record(name: &str, original: &Path, outputs: &[&Path]) {
    if let Some(out)=std::env::var_os("FOLIO_CALLOUT_OUTPUTS") {
        let root=PathBuf::from(out).join(name); fs::create_dir_all(&root).unwrap();
        fs::copy(original,root.join("original.pdf")).unwrap();
        for (i,path) in outputs.iter().enumerate() {fs::copy(path,root.join(format!("saved-{i}.pdf"))).unwrap();}
    }
}

#[test]
fn text_grows_inside_hollow_border_and_reedits_saved_source() {
    let e=engine(); let dir=tempfile::tempdir().unwrap(); let input=dir.path().join("source.pdf");
    fixture(&input,"1 w 50 270 300 70 re S","");
    let bytes=fs::read(&input).unwrap(); let original=e.open_document(&input).unwrap();
    let before=e.list_text_runs(&original.id,0).unwrap().runs;
    let selected=before.iter().find(|r|r.text=="Contact team").unwrap();
    let neighbor=before.iter().find(|r|r.text=="Neighbor unchanged").unwrap();
    let first=e.replace_text(&original.id,0,selected.object_index,&selected.text,"Contact this team").expect("Clear rectangle interior must permit insertion");
    let one=dir.path().join("saved.pdf"); let reopened=save(&e,&first,&one);
    let runs=e.list_text_runs(&reopened.id,0).unwrap().runs;
    let edited=runs.iter().find(|r|r.text=="Contact this team").unwrap();
    assert_eq!(edited.font_name,selected.font_name); assert_eq!(edited.font_size,selected.font_size);
    let retained=runs.iter().find(|r|r.text==neighbor.text).unwrap(); assert_eq!(retained.bounds,neighbor.bounds);
    let second=e.replace_text(&reopened.id,0,edited.object_index,&edited.text,"Contact this this team").unwrap();
    let two=dir.path().join("saved2.pdf"); let last=save(&e,&second,&two);
    assert!(e.list_text_runs(&last.id,0).unwrap().runs.iter().any(|r|r.text=="Contact this this team"));
    record("text",&input,&[&one,&two]); assert_eq!(fs::read(input).unwrap(),bytes);
    for id in [&last.id,&second.id,&reopened.id,&first.id,&original.id] {e.close_document(id).unwrap();}
}

fn draft(e:&PdfEngine,id:&str) -> StyledParagraphLayoutRequest {
    let runs=e.list_text_runs(id,0).unwrap().runs;
    let run=runs.iter().find(|r|r.text.starts_with("Contact")).unwrap();
    let s=e.inspect_occurrence_styled_paragraph(id,0,&[run.object_index]).unwrap();
    StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:s.object_indices,expected_text:s.text,runs:s.runs,
        style_changes:vec![],bounds:s.bounds,alignment:s.alignment.unwrap_or(ParagraphAlignment::Left),line_spacing:s.line_spacing.unwrap_or(1.2)}
}

#[test]
fn paragraph_fits_and_publishes_inside_hollow_border() {
    let e=engine(); let dir=tempfile::tempdir().unwrap(); let input=dir.path().join("source.pdf");
    fixture(&input,"1 w 50 270 300 70 re S","");
    let bytes=fs::read(&input).unwrap(); let original=e.open_document(&input).unwrap();
    let mut request=draft(&e,&original.id); request.runs[0].text="Contact this team".into();
    request.bounds=e.fit_styled_paragraph(&original.id,0,&request).expect("Clear rectangle interior must permit paragraph fitting");
    let first=e.preview_styled_paragraph(&original.id,0,&request).expect("Fitted interior must publish");
    let one=dir.path().join("saved.pdf"); let reopened=save(&e,&first,&one);
    let mut again=draft(&e,&reopened.id); assert_eq!(again.expected_text,"Contact this team");
    again.runs[0].text="Contact this this team".into(); again.bounds=e.fit_styled_paragraph(&reopened.id,0,&again).unwrap();
    let second=e.preview_styled_paragraph(&reopened.id,0,&again).unwrap();
    let two=dir.path().join("saved2.pdf"); let last=save(&e,&second,&two); assert_eq!(draft(&e,&last.id).expected_text,"Contact this this team");
    record("styled",&input,&[&one,&two]); assert_eq!(fs::read(input).unwrap(),bytes);
    for id in [&last.id,&second.id,&reopened.id,&first.id,&original.id] {e.close_document(id).unwrap();}
}

#[test]
fn stroke_variants_and_axis_preserving_transforms_keep_clear_interiors_editable() {
    let e=engine(); let dir=tempfile::tempdir().unwrap();
    for (name,border) in [
        ("hairline","0 w 50 270 300 70 re S"),
        ("thick","16 w 50 270 300 70 re S"),
        ("dashed","4 w 2 J 0 j [4 7] 2 d 50 270 300 70 re S"),
        ("nonuniform","2 0 0 0.5 0 0 cm 4 w 25 540 150 140 re S"),
        ("reflected","-1 0 0 1 400 0 cm 2 w 50 270 300 70 re S"),
        ("quarter-turn","0 1 -1 0 400 0 cm 2 w 270 50 70 300 re S"),
        ("explicit-close","1 w 50 270 m 350 270 l 350 340 l 50 340 l 50 270 l h S"),
        ("clipped","50 270 300 70 re W n 1 w 50 270 300 70 re S"),
    ] {
        let input=dir.path().join(format!("{name}.pdf")); fixture(&input,border,"");
        let source=e.open_document(&input).unwrap(); let request=draft(&e,&source.id);
        let index=request.object_indices[0];
        let single=e.replace_text_occurrence(&source.id,&[0],0,index,&request.expected_text,"Contact this team",None).unwrap_or_else(|error|panic!("{name} text: {error}")).document;
        let mut styled=request.clone(); styled.runs[0].text="Contact this team".into();
        styled.bounds=e.fit_occurrence_styled_paragraph(&source.id,0,&styled).unwrap_or_else(|error|panic!("{name} fit: {error}"));
        let paragraph=e.preview_styled_paragraph_occurrence(&source.id,&[0],0,&styled).unwrap_or_else(|error|panic!("{name} paragraph: {error}")).document;
        let mut area=request.bounds.clone(); area.width=200.; area.height=20.;
        // The legacy uniform emitter appends its replacement after neighboring
        // text. Keep its prior refusal until it can preserve source reading order.
        let uniform=e.preview_text_paragraph_occurrence(&source.id,&[0],0,&[index],&request.expected_text,"Contact this team",&area,&folio_engine::ParagraphFormatting::default());
        assert!(uniform.is_err(),"{name}: legacy uniform publication must not newly admit reordered text");
        assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),fs::read(&input).unwrap().as_slice());
        let paths=[dir.path().join(format!("{name}-text.pdf")),dir.path().join(format!("{name}-styled.pdf"))];
        for (doc,path) in [&single,&paragraph].into_iter().zip(&paths) {
            let reopened=save(&e,doc,path);
            assert_eq!(e.extract_text(&reopened.id,0).unwrap().split_whitespace().collect::<Vec<_>>(),
                ["Contact","this","team","Neighbor","unchanged"],"{name}: saved reading order changed");
            e.close_document(&reopened.id).unwrap();
        }
        record(name,&input,&paths.iter().map(PathBuf::as_path).collect::<Vec<_>>());
        for id in [&single.id,&paragraph.id,&source.id] {e.close_document(id).unwrap();}
    }
}

#[test]
fn borders_fills_compound_shapes_and_interior_artwork_still_block_growth() {
    let e=engine(); let dir=tempfile::tempdir().unwrap();
    for (name,border,extra) in [
        ("border","1 w 50 270 100 70 re S",""),
        ("fill","1 w 50 270 300 70 re B",""),
        ("compound","1 w 50 270 300 70 re 200 280 20 20 re S",""),
        ("open","1 w 50 270 m 350 270 l 350 340 l 50 340 l S",""),
        ("curve","1 w 50 270 m 350 270 l 350 340 l 200 345 100 345 50 340 c h S",""),
        ("skew","1 0.01 0 1 0 -2 cm 1 w 50 270 300 70 re S",""),
        ("artwork","1 w 50 270 300 70 re S","q 145 290 10 30 re f Q"),
        ("thick-neighbor","1 w 50 270 300 70 re S","q 12 w 164 270 100 70 re S Q"),
    ] {
        let path=dir.path().join(format!("{name}.pdf")); fixture(&path,border,extra);
        let bytes=fs::read(&path).unwrap(); let source=e.open_document(&path).unwrap();
        let mut request=draft(&e,&source.id); let index=request.object_indices[0];
        assert!(e.replace_text(&source.id,0,index,&request.expected_text,"Contact this team").is_err(),"{name} text crossed occupied graphics");
        request.runs[0].text="Contact this team".into(); request.bounds.width=200.; request.bounds.height=35.;
        assert!(e.preview_styled_paragraph(&source.id,0,&request).is_err(),"{name} paragraph crossed occupied graphics");
        assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),bytes.as_slice());
        assert_eq!(fs::read(&path).unwrap(),bytes); e.close_document(&source.id).unwrap();
    }
}

#[test]
fn paragraph_preserves_unselected_pagination_artifact_without_authorizing_its_edit() {
    let e=engine(); let dir=tempfile::tempdir().unwrap(); let input=dir.path().join("artifact.pdf");
    fixture(&input,"1 w 50 270 300 70 re S",
        "/Artifact << /Type /Pagination /Subtype /Header >> BDC BT /F1 12 Tf 50 380 Td (Header unchanged) Tj ET EMC");
    let original=e.open_document(&input).unwrap();
    let header=e.list_occurrence_text_runs(&original.id,0).unwrap().runs.into_iter().find(|r|r.text=="Header unchanged").unwrap();
    assert!(!header.supported,"Unselected context preservation must not authorize editing its semantics");
    let mut request=draft(&e,&original.id); request.runs[0].text="Contact this team".into();
    request.bounds=e.fit_occurrence_styled_paragraph(&original.id,0,&request).unwrap();
    let edited=e.preview_styled_paragraph_occurrence(&original.id,&[0],0,&request).expect("Unselected header context is unchanged");
    let after=e.list_occurrence_text_runs(&edited.document.id,0).unwrap().runs;
    let kept=after.iter().find(|r|r.text==header.text).unwrap(); assert_eq!(kept.bounds,header.bounds); assert!(!kept.supported);
    let saved=dir.path().join("saved.pdf"); fs::write(&saved,e.source_bytes(&edited.document.id).unwrap()).unwrap();
    record("pagination-artifact",&input,&[&saved]);
    e.close_document(&edited.document.id).unwrap(); e.close_document(&original.id).unwrap();
}

#[test]
fn grouped_text_keeps_border_and_neighbor_when_joined_and_changed() {
    let e=engine(); let dir=tempfile::tempdir().unwrap(); let input=dir.path().join("group.pdf");
    fixture(&input,"1 w 50 270 300 70 re S","");
    let mut pdf=Document::load(&input).unwrap();let page=*pdf.get_pages().values().next().unwrap();
    let contents=String::from_utf8(pdf.get_page_content_with_limit(page,1024*1024).unwrap()).unwrap()
        .replace("(Contact team) Tj","(Contact ) Tj (team) Tj");
    let content=pdf.add_object(Stream::new(dictionary!{},contents.into_bytes()));
    pdf.get_dictionary_mut(page).unwrap().set("Contents",content);pdf.save(&input).unwrap();
    let source=e.open_document(&input).unwrap();let runs=e.list_text_runs(&source.id,0).unwrap().runs;
    let indices=runs.iter().filter(|r|r.text!="Neighbor unchanged").map(|r|r.object_index).collect::<Vec<_>>();
    assert_eq!(indices.len(),2);let selected=e.inspect_text_group(&source.id,0,&indices).unwrap();
    assert_eq!(selected.text,"Contact team");
    let changed=e.replace_text_group(&source.id,0,&indices,&selected.text,"Contact this team").unwrap();
    let output=dir.path().join("saved.pdf");let reopened=save(&e,&changed,&output);
    assert!(e.list_text_runs(&reopened.id,0).unwrap().runs.iter().any(|r|r.text=="Contact this team"));
    record("group",&input,&[&output]);
    for id in [&reopened.id,&changed.id,&source.id] {e.close_document(id).unwrap();}
}

#[test]
#[ignore="Requires immutable local Word/ReportLab/MuPDF original exports in FOLIO_CALLOUT_INPUTS"]
fn original_producer_callouts_save_and_reedit_through_complete_source_routes() {
    let input_root=PathBuf::from(std::env::var_os("FOLIO_CALLOUT_INPUTS").expect("Original producer folder"));
    let e=engine(); let dir=tempfile::tempdir().unwrap();
    for name in ["mupdf-False","mupdf-True","reportlab-False","reportlab-True","word-False","word-True"] {
        let input=input_root.join(format!("{name}.pdf")); let original=fs::read(&input).unwrap();
        for paragraph in [false,true] {
            let phase=if paragraph {"paragraph"} else {"text"}; let mut current=e.open_document(&input).unwrap();
            let original_neighbor=e.render_page(&current.id,1,800).unwrap(); let mut outputs=Vec::new();
            for cycle in 0..2 {
                let runs=e.list_occurrence_text_runs(&current.id,0).unwrap().runs;
                let run=runs.iter().find(|r|r.text.starts_with("Contact:")).unwrap();
                let replacement=run.text.replace("café","café this");
                let result=if paragraph {
                    let s=e.inspect_occurrence_styled_paragraph(&current.id,0,&[run.object_index]).unwrap();
                    assert_eq!(s.runs.len(),1);
                    let mut request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:s.object_indices,expected_text:s.text,runs:s.runs,
                        style_changes:vec![],bounds:s.bounds,alignment:s.alignment.unwrap_or(ParagraphAlignment::Left),line_spacing:s.line_spacing.unwrap_or(1.2)};
                    request.runs[0].text=replacement.clone();
                    request.bounds=e.fit_occurrence_styled_paragraph(&current.id,0,&request).unwrap_or_else(|err|panic!("{name} {phase} fit {cycle}: {err}"));
                    e.preview_styled_paragraph_occurrence(&current.id,&[0,1],0,&request)
                } else { e.replace_text_occurrence(&current.id,&[0,1],0,run.object_index,&run.text,&replacement,None) }
                    .unwrap_or_else(|err|panic!("{name} {phase} save {cycle}: {err}"));
                let output=dir.path().join(format!("{name}-{phase}-{cycle}.pdf"));
                let reopened=save(&e,&result.document,&output);
                assert_eq!(reopened.pages.len(),2);
                assert_eq!(e.render_page(&reopened.id,1,800).unwrap(),original_neighbor);
                assert!(e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs.iter().any(|r|r.text==replacement));
                outputs.push(output); e.close_document(&result.document.id).unwrap();e.close_document(&current.id).unwrap();current=reopened;
            }
            record(&format!("producer-{name}-{phase}"),&input,&outputs.iter().map(PathBuf::as_path).collect::<Vec<_>>());
            e.close_document(&current.id).unwrap();
        }
        assert_eq!(fs::read(input).unwrap(),original);
    }
}
