use folio_engine::{DocumentInfo, ExportRequest, PagePlan, PdfEngine};
use lopdf::{content::{Content, Operation}, dictionary, Document, Object, Stream};
use std::{fs, path::{Path, PathBuf}};

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()
}

fn fixture(path: &Path, encoding: &str) {
    fixture_font(path,encoding,"Helvetica");
}
fn fixture_font(path: &Path, encoding: &str, face: &str) {
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let font = pdf.add_object(dictionary! {"Type"=>"Font", "Subtype"=>"Type1",
        "BaseFont"=>face, "Encoding"=>encoding});
    // Fixed independent fixture bytes: eacute is E9 in WinAnsi, 8E in MacRoman.
    let encoded = if encoding == "WinAnsiEncoding" { b"R\xe9sum\xe9 caf\xe9".to_vec() }
                  else { b"R\x8esum\x8e caf\x8e".to_vec() };
    let content = Content { operations: vec![
        Operation::new("BT",vec![]), Operation::new("Tf",vec![Object::Name(b"F1".to_vec()),14.into()]),
        Operation::new("Td",vec![50.into(),300.into()]),
        Operation::new("Tj",vec![Object::string_literal(encoded)]), Operation::new("ET",vec![]),
        Operation::new("BT",vec![]), Operation::new("Tf",vec![Object::Name(b"F1".to_vec()),14.into()]),
        Operation::new("Td",vec![50.into(),150.into()]),
        Operation::new("Tj",vec![Object::string_literal("Neighbor safe")]), Operation::new("ET",vec![]),
    ]}.encode().unwrap();
    let content = pdf.add_object(Stream::new(dictionary!{},content));
    let page = pdf.add_object(dictionary! {"Type"=>"Page", "Parent"=>pages,
        "MediaBox"=>vec![0.into(),0.into(),400.into(),400.into()],
        "Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
    pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());
    let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",catalog);
    pdf.save(path).unwrap();
}

fn export(engine: &PdfEngine, source: &DocumentInfo, path: &Path) {
    engine.export_pdf(ExportRequest {flatten:false,pages:vec![PagePlan {
        id:"page".into(),source_id:source.id.clone(),page_index:0,width:source.pages[0].width,
        height:source.pages[0].height,rotation:0,crop:None,overlays:vec![],
    }]},path).unwrap();
}

fn single_run(encoding: &str) {
    single_run_font(encoding,"Helvetica");
}
fn dump(name: &str, input: &Path, outputs: &[(&str, &Path)]) {
    if let Some(root)=std::env::var_os("FOLIO_STANDARD_LATIN_OUTPUTS") {
        let root=PathBuf::from(root);fs::create_dir_all(&root).unwrap();
        let folder=root.join(name);fs::create_dir(&folder).unwrap();
        fs::copy(input,folder.join("original.pdf")).unwrap();
        for (name,path) in outputs {fs::copy(path,folder.join(name)).unwrap();}
    }
}
fn single_run_font(encoding: &str, face: &str) {
    let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("original.pdf");
    fixture_font(&input,encoding,face);let original=fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();
    let before=e.list_occurrence_text_runs(&source.id,0).unwrap().runs;
    let run=&before[0];assert_eq!(run.text,"Résumé café");
    assert!(run.supported,"{encoding}: {:?}",run.reason);
    let noop=e.replace_text_occurrence(&source.id,&[0],0,run.object_index,&run.text,&run.text,None).unwrap();
    assert_eq!(e.render_page(&source.id,0,800).unwrap(),e.render_page(&noop.document.id,0,800).unwrap());
    e.close_document(&noop.document.id).unwrap();
    let edited=e.replace_text_occurrence(&source.id,&[0],0,run.object_index,&run.text,"Résumé this café",None).unwrap();
    let output=temp.path().join("saved.pdf");export(&e,&edited.document,&output);
    let reopened=e.open_document(&output).unwrap();let after=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
    assert_eq!(after[0].text,"Résumé this café");assert!(after[0].supported);
    assert_eq!(after[0].font_name,run.font_name);assert_eq!(after[0].font_size,run.font_size);
    assert_eq!(after[1].text,before[1].text);assert_eq!(after[1].bounds,before[1].bounds);
    let again=e.replace_text_occurrence(&reopened.id,&[0],0,after[0].object_index,&after[0].text,"Café résumé",None).unwrap();
    assert!(e.list_occurrence_text_runs(&again.document.id,0).unwrap().runs.iter().any(|r|r.text=="Café résumé"));
    let second=temp.path().join("re-edited.pdf");export(&e,&again.document,&second);
    dump(&format!("single-{encoding}-{face}"),&input,&[("edited.pdf",&output),("reedited.pdf",&second)]);
    assert_eq!(fs::read(input).unwrap(),original);
    for id in [&again.document.id,&reopened.id,&edited.document.id,&source.id] {e.close_document(id).unwrap();}
}

#[test]
fn winansi_accented_source_font_insertion_and_saved_reedit() {single_run("WinAnsiEncoding");}
#[test]
fn macroman_accented_source_font_insertion_and_saved_reedit() {single_run("MacRomanEncoding");}

#[test]
fn every_standard_latin_face_preserves_accented_source_font_reediting() {
    for face in ["Helvetica-Bold","Helvetica-Oblique","Helvetica-BoldOblique",
        "Times-Roman","Times-Bold","Times-Italic","Times-BoldItalic",
        "Courier","Courier-Bold","Courier-Oblique","Courier-BoldOblique"] {
        for encoding in ["WinAnsiEncoding","MacRomanEncoding"] {single_run_font(encoding,face);}
    }
}

#[test]
fn winansi_accented_source_font_paragraph_reflow_and_saved_reinspection() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("paragraph.pdf");
    fixture(&input,"WinAnsiEncoding");let original=fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();
    let selection=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();assert_eq!(selection.text,"Résumé café");
    let mut area=selection.bounds.clone();area.width=240.;area.height=50.;
    let edited=e.preview_text_paragraph(&source.id,0,&[0],&selection.text,"Résumé this café",&area).unwrap();
    let output=temp.path().join("saved.pdf");export(&e,&edited,&output);let reopened=e.open_document(&output).unwrap();
    let runs=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
    let selected=runs.iter().find(|r|r.text=="Résumé this café").unwrap();assert!(selected.supported);
    let inspected=e.inspect_text_paragraph(&reopened.id,0,&[selected.object_index]).unwrap();
    assert_eq!(inspected.text,"Résumé this café");assert_eq!(inspected.font_name,selection.font_name);
    dump("uniform-paragraph",&input,&[("edited.pdf",&output)]);
    assert_eq!(fs::read(input).unwrap(),original);
    for id in [&reopened.id,&edited.id,&source.id] {e.close_document(id).unwrap();}
}

#[test]
fn identically_named_standard_fonts_bind_their_own_source_encoding() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("two-encodings.pdf");
    fixture(&input,"WinAnsiEncoding");let mut pdf=Document::load(&input).unwrap();
    let page=*pdf.get_pages().values().next().unwrap();
    let other=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica","Encoding"=>"MacRomanEncoding"});
    let resources=pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap().get_mut(b"Resources").unwrap().as_dict_mut().unwrap();
    resources.get_mut(b"Font").unwrap().as_dict_mut().unwrap().set("F2",other);
    let mut content=pdf.get_page_content_with_limit(page,16*1024*1024).unwrap();
    content.extend_from_slice(b" BT /F2 14 Tf 50 220 Td <528e73756d8e206361668e> Tj ET");
    let content=pdf.add_object(Stream::new(dictionary!{},content));
    pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap().set("Contents",content);pdf.save(&input).unwrap();
    let original=fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();
    let runs=e.list_occurrence_text_runs(&source.id,0).unwrap().runs;
    let targets:Vec<_>=runs.iter().filter(|r|r.text=="Résumé café").collect();assert_eq!(targets.len(),2);
    assert!(targets.iter().all(|r|r.supported));assert_eq!(targets[0].font_name,targets[1].font_name);
    let selected=targets[1];let changed=e.replace_text_occurrence(&source.id,&[0],0,selected.object_index,&selected.text,"Café this",None).unwrap();
    let after=e.list_occurrence_text_runs(&changed.document.id,0).unwrap().runs;
    assert!(after.iter().any(|r|r.text=="Café this"&&r.supported));
    for before in runs.iter().filter(|r|r.object_index!=selected.object_index) {
        let actual=after.iter().find(|r|r.object_index==before.object_index).unwrap();
        assert_eq!(actual.text,before.text);assert_eq!(actual.bounds,before.bounds);
    }
    assert_eq!(fs::read(input).unwrap(),original);
    for id in [&changed.document.id,&source.id] {e.close_document(id).unwrap();}
}

#[test]
fn nested_standard_font_latin_uses_the_same_source_bound_encoding() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("form.pdf");
    fixture(&input,"MacRomanEncoding");let mut pdf=Document::load(&input).unwrap();
    let page=*pdf.get_pages().values().next().unwrap();
    let resources=pdf.get_dictionary(page).unwrap().get(b"Resources").unwrap().clone();
    let content=pdf.get_page_content_with_limit(page,16*1024*1024).unwrap();
    let form=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form",
        "BBox"=>vec![0.into(),0.into(),400.into(),400.into()],"Resources"=>resources},content));
    let content=pdf.add_object(Stream::new(dictionary!{},b"q /Fm Do Q".to_vec()));
    let page=pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap();
    page.set("Contents",content);page.set("Resources",dictionary!{"XObject"=>dictionary!{"Fm"=>form}});
    pdf.save(&input).unwrap();let original=fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();
    let listed=e.list_form_text_runs(&source.id,0).unwrap();
    assert!(listed.reason.is_none(),"{:?}",listed.reason);
    let run=listed.runs.iter().find(|r|r.text=="Résumé café").unwrap();assert!(run.supported,"{:?}",run.reason);
    let changed=e.replace_form_text(&source.id,0,&run.object_path,&run.text,"Résumé this café").unwrap();
    let output=temp.path().join("saved.pdf");export(&e,&changed,&output);let reopened=e.open_document(&output).unwrap();
    let after=e.list_form_text_runs(&reopened.id,0).unwrap();
    assert!(after.runs.iter().any(|r|r.text=="Résumé this café"&&r.supported));
    dump("nested-macroman",&input,&[("edited.pdf",&output)]);
    assert_eq!(fs::read(input).unwrap(),original);
    for id in [&reopened.id,&changed.id,&source.id] {e.close_document(id).unwrap();}
}

#[test]
fn styled_source_font_latin_paragraph_reopens_and_replays_exactly() {
    use folio_engine::{ParagraphAlignment,StyledParagraphLayoutRequest,StyledParagraphRun};
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for encoding in ["WinAnsiEncoding","MacRomanEncoding"] {
        let input=temp.path().join(format!("{encoding}.pdf"));fixture(&input,encoding);
        let source=e.open_document(&input).unwrap();let selected=e.inspect_styled_paragraph(&source.id,0,&[0]).unwrap();
        let mut bounds=selected.bounds.clone();bounds.width=240.;bounds.height=55.;
        let request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:vec![0],expected_text:selected.text,
            runs:vec![StyledParagraphRun{text:"Résumé this café".into(),style_id:0}],style_changes:vec![],
            bounds,alignment:ParagraphAlignment::Left,line_spacing:1.2};
        let edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();
        let output=temp.path().join(format!("{encoding}-saved.pdf"));export(&e,&edited,&output);
        let reopened=e.open_document(&output).unwrap();
        let runs=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
        let ids=runs.iter().filter(|r|r.text!="Neighbor safe").map(|r|r.object_index).collect::<Vec<_>>();
        let selected=e.inspect_styled_paragraph(&reopened.id,0,&ids).unwrap();assert_eq!(selected.text,"Résumé this café");
        let request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:selected.object_indices,expected_text:selected.text,
            runs:selected.runs,style_changes:vec![],bounds:selected.bounds,
            alignment:selected.alignment.unwrap(),line_spacing:selected.line_spacing.unwrap()};
        let replay=e.preview_styled_paragraph(&reopened.id,0,&request).unwrap();
        assert_eq!(e.render_page(&reopened.id,0,800).unwrap(),e.render_page(&replay.id,0,800).unwrap());
        dump(&format!("styled-{encoding}"),&input,&[("edited.pdf",&output)]);
        for id in [&replay.id,&reopened.id,&edited.id,&source.id] {e.close_document(id).unwrap();}
    }
}

#[test]
fn explicit_font_substitution_accepts_accented_standard_source() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    let font=e.fonts().register(fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
    for encoding in ["WinAnsiEncoding","MacRomanEncoding"] {
        let input=temp.path().join(format!("{encoding}.pdf"));fixture(&input,encoding);
        let source=e.open_document(&input).unwrap();let listed=e.list_occurrence_text_runs(&source.id,0).unwrap();let run=&listed.runs[0];
        assert!(run.can_substitute,"{:?}",run.reason);
        let changed=e.replace_text_occurrence(&source.id,&[0],0,run.object_index,&run.text,"Résumé this café",Some(&font.id)).unwrap();
        let output=temp.path().join(format!("{encoding}-saved.pdf"));export(&e,&changed.document,&output);
        let reopened=e.open_document(&output).unwrap();let after=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
        assert_eq!(after[0].text,"Résumé this café");assert!(after[0].is_embedded);
        assert_eq!(after[1].bounds,listed.runs[1].bounds);
        dump(&format!("substitute-{encoding}"),&input,&[("edited.pdf",&output)]);
        for id in [&reopened.id,&changed.document.id,&source.id] {e.close_document(id).unwrap();}
    }
}

#[test]
fn accented_positioned_segments_preserve_dependent_following_text() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("positioned.pdf");
    fixture(&input,"WinAnsiEncoding");let mut pdf=Document::load(&input).unwrap();
    let page=*pdf.get_pages().values().next().unwrap();
    let content=pdf.add_object(Stream::new(dictionary!{},
        b"BT /F1 14 Tf 50 300 Td [(R\xe9su) -12 (m\xe9 caf\xe9)] TJ [-6000] TJ (Suffix safe) Tj ET BT /F1 14 Tf 50 150 Td (Neighbor safe) Tj ET".to_vec()));
    pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap().set("Contents",content);pdf.save(&input).unwrap();
    let source=e.open_document(&input).unwrap();let before=e.list_occurrence_text_runs(&source.id,0).unwrap().runs;
    let run=&before[0];assert!(run.supported,"{:?}",run.reason);
    let changed=e.replace_text_occurrence(&source.id,&[0],0,run.object_index,&run.text,"Résumé this café",None).unwrap();
    let output=temp.path().join("saved.pdf");export(&e,&changed.document,&output);let reopened=e.open_document(&output).unwrap();
    let after=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
    assert_eq!(after[0].source_text.as_deref().unwrap_or(&after[0].text),"Résumé this café");
    for expected in &before[1..] {
        let actual=after.iter().find(|r|r.text==expected.text).unwrap();assert_eq!(actual.bounds,expected.bounds);
    }
    dump("positioned-dependent",&input,&[("edited.pdf",&output)]);
    for id in [&reopened.id,&changed.document.id,&source.id] {e.close_document(id).unwrap();}
}

#[test]
fn accented_insertion_keeps_source_tj_gap_and_refuses_neighbor_overlap() {
    // A trailing gap inside the source show belongs to its original character
    // slot. Appending through it would reach the suffix; it must not be moved.
    let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("gap.pdf");
    fixture(&input,"WinAnsiEncoding");let mut pdf=Document::load(&input).unwrap();
    let page=*pdf.get_pages().values().next().unwrap();
    let content=pdf.add_object(Stream::new(dictionary!{},
        b"BT /F1 14 Tf 50 300 Td [(R\xe9su) -12 (m\xe9 caf\xe9) -6000] TJ (Suffix safe) Tj ET".to_vec()));
    pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap().set("Contents",content);pdf.save(&input).unwrap();
    let original=fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();
    let before=e.render_page(&source.id,0,800).unwrap();let runs=e.list_occurrence_text_runs(&source.id,0).unwrap().runs;
    let result=e.replace_text_occurrence(&source.id,&[0],0,runs[0].object_index,&runs[0].text,"Résumé this café",None);
    assert!(result.unwrap_err().to_string().contains("overlap neighboring text"));
    assert_eq!(before,e.render_page(&source.id,0,800).unwrap());assert_eq!(fs::read(&input).unwrap(),original);
    e.close_document(&source.id).unwrap();
}

#[test]
fn accented_source_edit_preserves_rotated_physical_pages() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for rotation in [0,90,180,270] {for unit in [1.0f32,1.7] {
        let input=temp.path().join(format!("{rotation}-{unit}.pdf"));fixture(&input,"WinAnsiEncoding");
        let mut pdf=Document::load(&input).unwrap();let page=*pdf.get_pages().values().next().unwrap();
        let page=pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap();page.set("Rotate",rotation);page.set("UserUnit",unit);pdf.save(&input).unwrap();
        let original=fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();
        let before=e.list_occurrence_text_runs(&source.id,0).unwrap().runs;assert!(before[0].supported,"{rotation}/{unit}: {:?}",before[0].reason);
        let changed=e.replace_text_occurrence(&source.id,&[0],0,before[0].object_index,&before[0].text,"Résumé this café",None).unwrap();
        let output=temp.path().join(format!("{rotation}-{unit}-saved.pdf"));export(&e,&changed.document,&output);
        let reopened=e.open_document(&output).unwrap();let after=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
        assert_eq!(after[0].text,"Résumé this café");assert!(after[0].supported);
        assert_eq!(after[0].font_size,before[0].font_size);assert_eq!(after[1].bounds,before[1].bounds);
        assert_eq!(source.pages[0].width,reopened.pages[0].width);assert_eq!(source.pages[0].height,reopened.pages[0].height);
        dump(&format!("physical-{rotation}-{unit}"),&input,&[("edited.pdf",&output)]);
        assert_eq!(fs::read(&input).unwrap(),original);
        for id in [&reopened.id,&changed.document.id,&source.id] {e.close_document(id).unwrap();}
    }}
}

#[test]
fn standard_encoding_punctuation_and_quote_operators_preserve_following_lines() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for (name,encoding,operand,expected,replacement) in [
        ("standard","StandardEncoding",b"Price \xa3 'today'".as_slice(),"Price £ ’today’","Price £ ’this day’"),
        ("winansi","WinAnsiEncoding",b"R\xe9sum\xe9 caf\xe9".as_slice(),"Résumé café","Résumé this café"),
    ] {for operator in ["'","\""] {
        let input=temp.path().join("quote.pdf");fixture(&input,encoding);
        let mut pdf=Document::load(&input).unwrap();let page=*pdf.get_pages().values().next().unwrap();
        let mut operations=vec![Operation::new("BT",vec![]),Operation::new("Tf",vec![Object::Name(b"F1".to_vec()),14.into()]),
            Operation::new("Td",vec![50.into(),330.into()]),Operation::new("TL",vec![30.into()])];
        let mut args=if operator=="\"" {vec![0.into(),0.into()]} else {vec![]};args.push(Object::string_literal(operand.to_vec()));
        operations.push(Operation::new(operator,args));
        operations.push(Operation::new("'",vec![Object::string_literal("Neighbor safe")]));operations.push(Operation::new("ET",vec![]));
        let content=pdf.add_object(Stream::new(dictionary!{},Content{operations}.encode().unwrap()));
        pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap().set("Contents",content);pdf.save(&input).unwrap();
        let source=e.open_document(&input).unwrap();let before=e.list_occurrence_text_runs(&source.id,0).unwrap().runs;
        assert_eq!(before[0].text,expected);assert!(before[0].supported,"{:?}",before[0].reason);
        let changed=e.replace_text_occurrence(&source.id,&[0],0,before[0].object_index,expected,replacement,None).unwrap();
        let output=temp.path().join("saved.pdf");export(&e,&changed.document,&output);let reopened=e.open_document(&output).unwrap();
        let after=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;assert_eq!(after[0].text,replacement);assert_eq!(after[1].bounds,before[1].bounds);
        dump(&format!("quote-{name}-{}",if operator=="'" {"single"} else {"double"}),&input,&[("edited.pdf",&output)]);
        for id in [&reopened.id,&changed.document.id,&source.id] {e.close_document(id).unwrap();}
    }}
}

fn grouped_standard_latin(encoding: &str) {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for rotation in [0, 90, 180, 270] {
        for unit in [1.0f32, 1.7] {
            let input = temp.path().join(format!("group-{rotation}-{unit}.pdf"));
            fixture_font(&input, encoding, "Courier");
            let mut pdf = Document::load(&input).unwrap();
            let page = *pdf.get_pages().values().next().unwrap();
            let hex = if encoding == "WinAnsiEncoding" { "e9" } else { "8e" };
            let content = pdf.add_object(Stream::new(dictionary!{}, format!(
                "BT /F1 14 Tf 50 300 Td <436166{hex}> Tj <21> Tj ET BT /F1 14 Tf 50 150 Td (Neighbor safe) Tj ET"
            ).into_bytes()));
            let page_dict = pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap();
            page_dict.set("Contents", content);
            page_dict.set("Rotate", rotation);
            page_dict.set("UserUnit", unit);
            pdf.save(&input).unwrap();
            let original = fs::read(&input).unwrap();
            let source = e.open_document(&input).unwrap();
            let runs = e.list_occurrence_text_runs(&source.id, 0).unwrap().runs;
            assert!(runs[0].supported && runs[1].supported, "{encoding}/{rotation}/{unit}: {runs:?}");
            let selection = e.inspect_occurrence_text_group(&source.id, 0, &[0, 1])
                .unwrap_or_else(|error| panic!("{encoding}/{rotation}/{unit}: {error}"));
            assert_eq!(selection.text, "Café!");
            let noop = e.replace_text_group_occurrence(&source.id, &[0], 0, &[0, 1], "Café!", "Café!").unwrap();
            assert_eq!(e.render_page(&source.id, 0, 800).unwrap(), e.render_page(&noop.document.id, 0, 800).unwrap());
            let noop_path = temp.path().join("noop.pdf");
            export(&e, &noop.document, &noop_path);
            let noop_open = e.open_document(&noop_path).unwrap();
            assert_eq!(e.inspect_occurrence_text_group(&noop_open.id, 0, &[0, 1]).unwrap().text, "Café!");
            let edited = e.replace_text_group_occurrence(&source.id, &[0], 0, &[0, 1], "Café!", "Café this!").unwrap();
            let output = temp.path().join("edited.pdf");
            export(&e, &edited.document, &output);
            let reopened = e.open_document(&output).unwrap();
            let after = e.list_occurrence_text_runs(&reopened.id, 0).unwrap().runs;
            let changed = after.iter().find(|run| run.text == "Café this!").unwrap();
            assert!(changed.supported);
            assert_eq!(changed.font_name, runs[0].font_name);
            assert_eq!(changed.font_size, runs[0].font_size);
            let neighbor = after.iter().find(|run| run.text == "Neighbor safe").unwrap();
            assert_eq!(neighbor.bounds, runs[2].bounds);
            let reedited = e.replace_text_occurrence(&reopened.id, &[0], 0, changed.object_index,
                "Café this!", "Café again!", None).unwrap();
            let second = temp.path().join("reedited.pdf");
            export(&e, &reedited.document, &second);
            dump(&format!("group-{encoding}-{rotation}-{unit}"), &input,
                 &[("noop.pdf", &noop_path), ("edited.pdf", &output), ("reedited.pdf", &second)]);
            assert_eq!(fs::read(&input).unwrap(), original);
            assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(), original.as_slice());
            for id in [&reedited.document.id, &reopened.id, &edited.document.id,
                       &noop_open.id, &noop.document.id, &source.id] {
                e.close_document(id).unwrap();
            }
        }
    }
}

#[test]
fn grouped_winansi_standard_latin_preserves_saved_reediting() {
    grouped_standard_latin("WinAnsiEncoding");
}

#[test]
fn grouped_macroman_standard_latin_preserves_saved_reediting() {
    grouped_standard_latin("MacRomanEncoding");
}
