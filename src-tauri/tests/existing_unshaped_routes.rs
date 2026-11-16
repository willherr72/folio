use folio_engine::{AnnotationRect,ParagraphAlignment,ParagraphFormatting,StyledParagraphLayoutRequest,StyledParagraphRun,StyledParagraphStyleChange};
use lopdf::{Document,Object,Stream,dictionary,content::{Content,Operation}};
use std::{fs,path::Path};
#[path="support/unshaped_fixture.rs"]
#[allow(dead_code)]
mod support;
use support::{engine,fixture,export,capture,font_streams,FONT};

const VALUES:[&str;3]=["Δοκιμή Привет","Νέα δοκιμή Мир","Αλλαγή Проверка"];

fn group_fixture(path:&Path) {
    fixture(path,&VALUES,0,1.,false);
    let mut pdf=Document::load(path).unwrap();let page=*pdf.get_pages().values().next().unwrap();
    let mut content=Content::decode(&pdf.get_page_content(page)).unwrap();
    let at=content.operations.iter().position(|o|o.operator=="Tj").unwrap();
    let bytes=content.operations[at].operands[0].as_str().unwrap().to_vec();
    content.operations[at].operands[0]=Object::string_literal(bytes[..4].to_vec());
    content.operations.insert(at+1,Operation::new("Tj",vec![Object::string_literal(bytes[4..].to_vec())]));
    let stream=pdf.add_object(Stream::new(dictionary!{},content.encode().unwrap()));
    pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap().set("Contents",stream);pdf.save(path).unwrap();
}

fn form_fixture(path:&Path,rotation:i32,unit:f32) {
    fixture(path,&VALUES,rotation as u16,unit,false);
    let mut pdf=Document::load(path).unwrap();let page=*pdf.get_pages().values().next().unwrap();
    let resources=pdf.get_dictionary(page).unwrap().get(b"Resources").unwrap().clone();
    let form=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form",
      "BBox"=>vec![0.into(),0.into(),400.into(),400.into()],"Resources"=>resources},pdf.get_page_content(page)));
    let stream=pdf.add_object(Stream::new(dictionary!{},b"q /Shared Do Q q 1 0 0 1 400 0 cm /Shared Do Q".to_vec()));
    let p=pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap();p.set("MediaBox",vec![0.into(),0.into(),800.into(),400.into()]);
    p.set("Resources",dictionary!{"XObject"=>dictionary!{"Shared"=>form}});p.set("Contents",stream);pdf.save(path).unwrap();
}

fn area(rotation:u16,unit:f32)->AnnotationRect {
    let(x,y,w,h)=match rotation {0=>(35.,60.,300.,100.),90=>(240.,35.,100.,300.),180=>(65.,240.,300.,100.),270=>(60.,65.,100.,300.),_=>unreachable!()};
    AnnotationRect{x:x*unit,y:y*unit,width:w*unit,height:h*unit}
}

#[test]
fn grouped_unicode_keeps_font_neighbors_and_saved_reedit() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("original.pdf");group_fixture(&input);
    capture("group",&input);let bytes=fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();
    let before=e.list_occurrence_text_runs(&source.id,0).unwrap().runs;assert_eq!(before.len(),3);
    let selected=e.inspect_occurrence_text_group(&source.id,0,&[0,1]).unwrap();assert_eq!(selected.text,VALUES[0]);
    let edited=e.replace_text_group_occurrence(&source.id,&[0],0,&[0,1],&selected.text,VALUES[1]).unwrap();
    let saved=temp.path().join("saved.pdf");export(&e,&edited.document,&saved);capture("group",&saved);
    assert_eq!(font_streams(&input),font_streams(&saved));
    let reopened=e.open_document(&saved).unwrap();let after=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
    let changed=after.iter().find(|r|r.text==VALUES[1]).unwrap();let neighbor=after.iter().find(|r|r.text=="Neighbor safe").unwrap();
    assert_eq!(neighbor.bounds,before[2].bounds);
    let again=e.replace_text_occurrence(&reopened.id,&[0],0,changed.object_index,VALUES[1],VALUES[2],None).unwrap();
    let second=temp.path().join("re-edited.pdf");export(&e,&again.document,&second);capture("group",&second);
    assert_eq!(font_streams(&input),font_streams(&second));assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),bytes.as_slice());
    for id in [&again.document.id,&reopened.id,&edited.document.id,&source.id]{e.close_document(id).unwrap();}
}

#[test]
fn uniform_unicode_paragraph_saves_and_reedits_at_physical_rotations() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for rotation in [0,90,180,270]{for unit in [1.,1.7]{
        let label=format!("uniform-{rotation}-{unit}");let input=temp.path().join("original.pdf");fixture(&input,&VALUES,rotation,unit,false);capture(&label,&input);
        let source=e.open_document(&input).unwrap();let bytes=fs::read(&input).unwrap();let before=e.list_occurrence_text_runs(&source.id,0).unwrap().runs;
        let selected=e.inspect_occurrence_text_paragraph(&source.id,0,&[0]).unwrap();assert_eq!(selected.text,VALUES[0]);
        let bounds=area(rotation,unit);
        let edited=e.preview_text_paragraph_occurrence(&source.id,&[0],0,&[0],&selected.text,VALUES[1],&bounds,&ParagraphFormatting::default()).unwrap();
        let saved=temp.path().join("saved.pdf");export(&e,&edited.document,&saved);capture(&label,&saved);
        let reopened=e.open_document(&saved).unwrap();let runs=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
        let changed=runs.iter().find(|r|r.text==VALUES[1]).unwrap();assert_eq!(changed.font_name,before[0].font_name);
        assert_eq!(runs.iter().find(|r|r.text=="Neighbor safe").unwrap().bounds,before[1].bounds);
        let again=e.inspect_occurrence_text_paragraph(&reopened.id,0,&[changed.object_index]).unwrap();assert_eq!(again.text,VALUES[1]);
        let second=e.preview_text_paragraph_occurrence(&reopened.id,&[0],0,&again.object_indices,&again.text,VALUES[2],&bounds,&ParagraphFormatting::default()).unwrap();
        let output=temp.path().join("re-edited.pdf");export(&e,&second.document,&output);capture(&label,&output);
        assert_eq!(font_streams(&input),font_streams(&output));assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),bytes.as_slice());
        for id in [&second.document.id,&reopened.id,&edited.document.id,&source.id]{e.close_document(id).unwrap();}
    }}
}

fn styled(substitute:bool) {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for rotation in [0,90,180,270]{for unit in [1.,1.7]{
        let label=format!("styled-{substitute}-{rotation}-{unit}");let input=temp.path().join("original.pdf");fixture(&input,&VALUES,rotation,unit,false);capture(&label,&input);
        let bytes=fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();let selected=e.inspect_occurrence_styled_paragraph(&source.id,0,&[0]).unwrap();
        let asset=if substitute{Some(e.fonts().register(FONT.to_vec()).unwrap())}else{None};
        let mut request=StyledParagraphLayoutRequest{baseline_anchor:selected.baseline_anchor,object_indices:selected.object_indices,
          expected_text:selected.text,runs:vec![StyledParagraphRun{text:VALUES[1].into(),style_id:0}],
          style_changes:asset.as_ref().map(|f|vec![StyledParagraphStyleChange{style_id:0,source_style_id:0,font_id:Some(f.id.clone()),font_size:None,color:None}]).unwrap_or_default(),
          bounds:area(rotation,unit),alignment:ParagraphAlignment::Left,line_spacing:1.2};
        let edited=e.preview_styled_paragraph_occurrence(&source.id,&[0],0,&request).unwrap();
        if let Some(asset)=asset{e.fonts().remove(&asset.id).unwrap();}
        let saved=temp.path().join("saved.pdf");export(&e,&edited.document,&saved);capture(&label,&saved);
        let reopened=e.open_document(&saved).unwrap();let runs=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
        let changed=runs.iter().find(|r|r.text==VALUES[1]).unwrap();
        let again=e.suggest_occurrence_styled_paragraph(&reopened.id,0,changed.object_index,&changed.text).unwrap();
        assert_eq!(again.text,VALUES[1]);assert_eq!(again.styles[0].color,selected.styles[0].color);
        request.object_indices=again.object_indices;request.expected_text=again.text;request.bounds=again.bounds;
        request.baseline_anchor=again.baseline_anchor;request.style_changes.clear();request.runs[0].text=VALUES[2].into();
        let second=e.preview_styled_paragraph_occurrence(&reopened.id,&[0],0,&request).unwrap();
        let output=temp.path().join("re-edited.pdf");export(&e,&second.document,&output);capture(&label,&output);
        assert!(e.extract_text(&second.document.id,0).unwrap().contains(VALUES[2]));
        assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),bytes.as_slice());
        for id in [&second.document.id,&reopened.id,&edited.document.id,&source.id]{e.close_document(id).unwrap();}
    }}
}
#[test]fn styled_unicode_original_font_and_memory_reedit(){styled(false);}
#[test]fn styled_unicode_explicit_font_and_memory_reedit(){styled(true);}

#[test]
fn greek_only_substitute_reopens_without_a_latin_probe_glyph() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("original.pdf");fixture(&input,&VALUES,0,1.,false);
    let source=e.open_document(&input).unwrap();let bytes=fs::read(&input).unwrap();
    let font=e.fonts().register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans-greek-test.ttf").to_vec()).unwrap();
    let selected=e.inspect_occurrence_styled_paragraph(&source.id,0,&[0]).unwrap();
    let mut request=StyledParagraphLayoutRequest{baseline_anchor:selected.baseline_anchor,object_indices:selected.object_indices,
      expected_text:selected.text,runs:vec![StyledParagraphRun{text:"Νέα δοκιμή".into(),style_id:0}],
      style_changes:vec![StyledParagraphStyleChange{style_id:0,source_style_id:0,font_id:Some(font.id.clone()),font_size:None,color:None}],
      bounds:area(0,1.),alignment:ParagraphAlignment::Left,line_spacing:1.2};
    let edited=e.preview_styled_paragraph_occurrence(&source.id,&[0],0,&request).unwrap();e.fonts().remove(&font.id).unwrap();
    let saved=temp.path().join("saved.pdf");export(&e,&edited.document,&saved);capture("greek-only",&input);capture("greek-only",&saved);
    let reopened=e.open_document(&saved).unwrap();let runs=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
    let changed=runs.iter().find(|r|r.text=="Νέα δοκιμή").unwrap();let restored=e.suggest_occurrence_styled_paragraph(&reopened.id,0,changed.object_index,&changed.text).unwrap();
    assert_eq!(restored.text,"Νέα δοκιμή");request.object_indices=restored.object_indices;request.expected_text=restored.text;
    request.bounds=restored.bounds;request.baseline_anchor=restored.baseline_anchor;request.style_changes.clear();request.runs[0].text="Αλλαγή".into();
    let second=e.preview_styled_paragraph_occurrence(&reopened.id,&[0],0,&request).unwrap();let output=temp.path().join("re-edited.pdf");export(&e,&second.document,&output);capture("greek-only",&output);
    assert!(e.extract_text(&second.document.id,0).unwrap().contains("Αλλαγή"));assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),bytes.as_slice());
    for id in [&second.document.id,&reopened.id,&edited.document.id,&source.id]{e.close_document(id).unwrap();}
}

#[test]
fn unicode_shared_form_edit_preserves_other_occurrences_and_font() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for rotation in [0,90,180,270]{for unit in [1.,1.7]{
        let label=format!("form-{rotation}-{unit}");let input=temp.path().join("original.pdf");form_fixture(&input,rotation,unit);capture(&label,&input);
        let bytes=fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();let before=e.list_occurrence_form_text_runs(&source.id,0).unwrap();
        assert_eq!(before.runs.len(),4,"{:?}",before.reason);let selected=&before.runs[0];assert!(selected.supported,"{:?}",selected.reason);
        let noop=e.replace_form_text_occurrence(&source.id,&[0],0,&selected.object_path,VALUES[0],VALUES[0],None).unwrap();
        assert_eq!(e.render_page(&source.id,0,800).unwrap(),e.render_page(&noop.document.id,0,800).unwrap());e.close_document(&noop.document.id).unwrap();
        let edited=e.replace_form_text_occurrence(&source.id,&[0],0,&selected.object_path,VALUES[0],VALUES[1],None).unwrap();
        let after=e.list_occurrence_form_text_runs(&edited.document.id,0).unwrap().runs;assert_eq!(after[0].text,VALUES[1]);assert_eq!(after[1..],before.runs[1..]);
        let saved=temp.path().join("saved.pdf");export(&e,&edited.document,&saved);capture(&label,&saved);
        let reopened=e.open_document(&saved).unwrap();let runs=e.list_occurrence_form_text_runs(&reopened.id,0).unwrap().runs;
        assert!(runs[0].supported);let second=e.replace_form_text_occurrence(&reopened.id,&[0],0,&runs[0].object_path,VALUES[1],VALUES[2],None).unwrap();
        let output=temp.path().join("re-edited.pdf");export(&e,&second.document,&output);capture(&label,&output);
        assert_eq!(font_streams(&input),font_streams(&output));assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),bytes.as_slice());
        for id in [&second.document.id,&reopened.id,&edited.document.id,&source.id]{e.close_document(id).unwrap();}
    }}
}

#[test]
fn unicode_form_paragraph_promotes_one_invocation_and_saved_memory_reedits() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for rotation in [0,90,180,270]{for unit in [1.,1.7]{
        let label=format!("form-paragraph-{rotation}-{unit}");let input=temp.path().join("original.pdf");
        form_fixture(&input,rotation,unit);capture(&label,&input);let bytes=fs::read(&input).unwrap();
        let source=e.open_document(&input).unwrap();let original=e.list_occurrence_form_text_runs(&source.id,0).unwrap().runs;
        let anchor=&original[0];let listing=e.list_form_paragraph_runs(&source.id,0,&anchor.object_path,&anchor.text).unwrap();
        let selected=e.inspect_form_styled_paragraph(&source.id,0,&anchor.object_path,&[listing.anchor_index]).unwrap();
        assert_eq!(selected.text,VALUES[0]);
        let mut bounds=area(rotation as u16,unit);
        if rotation==180{bounds.x+=400.*unit;}else if rotation==270{bounds.y+=400.*unit;}
        let request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:selected.object_indices,
          expected_text:selected.text,runs:vec![StyledParagraphRun{text:VALUES[1].into(),style_id:0}],
          style_changes:vec![],bounds,alignment:ParagraphAlignment::Left,line_spacing:1.2};
        let edited=e.preview_form_styled_paragraph_occurrence(&source.id,&[0],0,&anchor.object_path,&request).unwrap();
        let siblings=e.list_occurrence_form_text_runs(&edited.document.id,0).unwrap().runs;
        assert_eq!(siblings.len(),2);
        for (after,before) in siblings.iter().zip(&original[2..]){
            // Promotion inserts a top-level object; its sibling invocation index
            // changes while the leaf, text, style and physical geometry stay exact.
            assert_eq!(after.object_path[1..],before.object_path[1..]);
            let mut comparable=after.clone();comparable.object_path=before.object_path.clone();assert_eq!(&comparable,before);
        }
        let saved=temp.path().join("saved.pdf");export(&e,&edited.document,&saved);capture(&label,&saved);
        let reopened=e.open_document(&saved).unwrap();assert_eq!(e.render_page(&edited.document.id,0,800).unwrap(),e.render_page(&reopened.id,0,800).unwrap());
        let runs=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
        let changed=runs.iter().find(|r|r.text==VALUES[1]).unwrap();
        let restored=e.suggest_occurrence_styled_paragraph(&reopened.id,0,changed.object_index,&changed.text).unwrap();
        assert_eq!(restored.text,VALUES[1]);
        let again=StyledParagraphLayoutRequest{baseline_anchor:restored.baseline_anchor,object_indices:restored.object_indices,
          expected_text:restored.text,runs:vec![StyledParagraphRun{text:VALUES[2].into(),style_id:0}],style_changes:vec![],
          bounds:restored.bounds,alignment:restored.alignment.unwrap_or(ParagraphAlignment::Left),line_spacing:restored.line_spacing.unwrap_or(1.2)};
        let second=e.preview_styled_paragraph_occurrence(&reopened.id,&[0],0,&again).unwrap();
        let output=temp.path().join("re-edited.pdf");export(&e,&second.document,&output);capture(&label,&output);
        assert!(e.extract_text(&second.document.id,0).unwrap().contains(VALUES[2]));
        assert_eq!(font_streams(&input),font_streams(&output));assert_eq!(fs::read(&input).unwrap(),bytes);
        assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),bytes.as_slice());
        for id in [&second.document.id,&reopened.id,&edited.document.id,&source.id]{e.close_document(id).unwrap();}
    }}
}

#[test]
fn explicit_single_run_font_uses_its_repertoire_without_silent_fallback() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("original.pdf");
    fixture(&input,&VALUES,0,1.,false);let bytes=fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();
    let font=e.fonts().register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans-greek-test.ttf").to_vec()).unwrap();
    let before=e.list_occurrence_text_runs(&source.id,0).unwrap().runs;
    for text in ["Δοκιμή","Νέα Ψ","α\u{301}","😀"]{
        assert!(e.replace_text_occurrence(&source.id,&[0],0,0,VALUES[0],text,Some(&font.id)).is_err(),"Unexpected substitute acceptance: {text}");
    }
    let edited=e.replace_text_occurrence(&source.id,&[0],0,0,VALUES[0],"Νέα δοκιμή",Some(&font.id)).unwrap();
    e.fonts().remove(&font.id).unwrap();let saved=temp.path().join("saved.pdf");export(&e,&edited.document,&saved);
    capture("single-substitute",&input);capture("single-substitute",&saved);
    let reopened=e.open_document(&saved).unwrap();let runs=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
    assert_eq!(runs[0].text,"Νέα δοκιμή");assert_eq!(runs[1].bounds,before[1].bounds);
    assert!(e.replace_text_occurrence(&reopened.id,&[0],0,runs[0].object_index,&runs[0].text,"Δοκιμή",None).is_err());
    let second=e.replace_text_occurrence(&reopened.id,&[0],0,runs[0].object_index,&runs[0].text,"Αλλαγή",None).unwrap();
    let output=temp.path().join("re-edited.pdf");export(&e,&second.document,&output);capture("single-substitute",&output);
    assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),bytes.as_slice());assert_eq!(fs::read(&input).unwrap(),bytes);
    for id in [&second.document.id,&reopened.id,&edited.document.id,&source.id]{e.close_document(id).unwrap();}
}

#[test]
fn grouped_and_paragraph_requests_refuse_unsupported_unicode_and_missing_maps_atomically() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("group.pdf");group_fixture(&input);
    let bytes=fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();
    let original=e.render_page(&source.id,0,800).unwrap();
    let group=e.inspect_occurrence_text_group(&source.id,0,&[0,1]).unwrap();
    let styled=e.inspect_occurrence_styled_paragraph(&source.id,0,&[0,1]).unwrap();
    let oversized="Δ".repeat(2049);
    for text in ["α\u{301}","α\u{202e}","مرحبا","क","😀","Ψ",oversized.as_str()]{
        assert!(e.replace_text_group_occurrence(&source.id,&[0],0,&[0,1],&group.text,text).is_err(),"group accepted {text:?}");
        assert!(e.preview_text_paragraph_occurrence(&source.id,&[0],0,&[0,1],&group.text,text,&area(0,1.),&ParagraphFormatting::default()).is_err(),"uniform accepted {text:?}");
        let request=StyledParagraphLayoutRequest{baseline_anchor:styled.baseline_anchor,object_indices:styled.object_indices.clone(),
          expected_text:styled.text.clone(),runs:vec![StyledParagraphRun{text:text.into(),style_id:0}],style_changes:vec![],
          bounds:area(0,1.),alignment:ParagraphAlignment::Left,line_spacing:1.2};
        assert!(e.preview_styled_paragraph_occurrence(&source.id,&[0],0,&request).is_err(),"styled accepted {text:?}");
        assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),bytes.as_slice());
    }
    assert!(e.replace_text_group_occurrence(&source.id,&[0],0,&[0,1],&group.text,"Νέα\nδοκιμή").is_err());
    assert_eq!(e.render_page(&source.id,0,800).unwrap(),original);assert_eq!(fs::read(&input).unwrap(),bytes);
    e.close_document(&source.id).unwrap();
}

#[test]
fn shared_form_text_and_paragraph_requests_refuse_unsupported_unicode_atomically() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("form.pdf");form_fixture(&input,0,1.);
    let bytes=fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();let original=e.render_page(&source.id,0,800).unwrap();
    let before=e.list_occurrence_form_text_runs(&source.id,0).unwrap().runs;let anchor=&before[0];
    let listing=e.list_form_paragraph_runs(&source.id,0,&anchor.object_path,&anchor.text).unwrap();
    let selected=e.inspect_form_styled_paragraph(&source.id,0,&anchor.object_path,&[listing.anchor_index]).unwrap();
    let oversized="Δ".repeat(2049);
    for text in ["α\u{301}","α\u{202e}","مرحبا","क","😀","Ψ",oversized.as_str()]{
        assert!(e.replace_form_text_occurrence(&source.id,&[0],0,&anchor.object_path,&anchor.text,text,None).is_err(),"form accepted {text:?}");
        let request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:selected.object_indices.clone(),
          expected_text:selected.text.clone(),runs:vec![StyledParagraphRun{text:text.into(),style_id:0}],style_changes:vec![],
          bounds:area(0,1.),alignment:ParagraphAlignment::Left,line_spacing:1.2};
        assert!(e.preview_form_styled_paragraph_occurrence(&source.id,&[0],0,&anchor.object_path,&request).is_err(),"form paragraph accepted {text:?}");
        assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),bytes.as_slice());
    }
    assert_eq!(e.list_occurrence_form_text_runs(&source.id,0).unwrap().runs,before);
    assert_eq!(e.render_page(&source.id,0,800).unwrap(),original);assert_eq!(fs::read(&input).unwrap(),bytes);e.close_document(&source.id).unwrap();
}
