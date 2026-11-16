//! Original accent-cluster glyphs, source codes and saved editing, over public-font witnesses.
use folio_engine::{DocumentInfo,ExportRequest,PagePlan,PdfEngine};
use std::{fs,path::{Path,PathBuf}};

fn fixtures()->PathBuf {PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/source-accent-clusters/v1")}
fn engine()->PdfEngine {PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn cases()->Vec<serde_json::Value>{serde_json::from_slice::<serde_json::Value>(&fs::read(fixtures().join("authority.json")).unwrap()).unwrap()["cases"].as_array().unwrap().clone()}
fn save(e:&PdfEngine,d:&DocumentInfo,path:&Path){
    e.export_pdf(ExportRequest{flatten:false,pages:d.pages.iter().enumerate().map(|(page_index,p)|PagePlan{crop:None,id:format!("p{page_index}"),source_id:d.id.clone(),page_index,width:p.width,height:p.height,rotation:0,overlays:vec![]}).collect()},path).unwrap();
}
fn retain(name:&str,original:&Path,edited:&Path,restored:&Path){
    if let Some(root)=std::env::var_os("FOLIO_ACCENT_CLUSTER_OUTPUTS"){
        let dir=PathBuf::from(root).join(name);fs::create_dir_all(&dir).unwrap();
        for (source,target) in [(original,"original.pdf"),(edited,"edited.pdf"),(restored,"restored.pdf")]{fs::copy(source,dir.join(target)).unwrap();}
    }
}
#[test]
fn source_accent_clusters_original_font_noop_growth_delete_save_and_reedit(){
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for row in cases().into_iter().filter(|row|row["layout"]=="single"){
        let name=row["name"].as_str().unwrap();let input=fixtures().join(row["path"].as_str().unwrap());let original=fs::read(&input).unwrap();
        let source=e.open_document(&input).unwrap();let before=e.list_occurrence_text_runs(&source.id,0).unwrap().runs;
        let selected=before.iter().find(|r|r.text=="Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301}").expect("Exact source text");assert!(selected.supported,"{name}: {before:?}");
        let noop=e.replace_text_occurrence(&source.id,&[0],0,selected.object_index,"Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301}","Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301}",None).unwrap().document;
        assert!(e.render_page(&noop.id,0,1000).unwrap()==e.render_page(&source.id,0,1000).unwrap(),"No-op changed whole-page pixels: {name}");
        assert_eq!(first_show(&e.source_bytes(&noop.id).unwrap()),first_show(&original),"No-op changed physical source glyph codes: {name}");
        let edited=e.replace_text_occurrence(&source.id,&[0],0,selected.object_index,"Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301}","Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301} this",None).unwrap().document;
        let output=temp.path().join(format!("{name}-edited.pdf"));save(&e,&edited,&output);
        let reopened=e.open_document(&output).unwrap();let after=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
        let selected=after.iter().find(|r|r.text=="Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301} this").unwrap();assert!(selected.supported,"{name}: {after:?}");
        let neighbor=before.iter().find(|r|r.text=="Unchanged neighbor").unwrap();assert_eq!(after.iter().find(|r|r.text==neighbor.text).unwrap(),neighbor);
        let changed=e.replace_text_occurrence(&reopened.id,&[0],0,selected.object_index,"Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301} this","Cafe safe",None).unwrap().document;
        let restored=temp.path().join(format!("{name}-deleted.pdf"));save(&e,&changed,&restored);
        let again=e.open_document(&restored).unwrap();assert!(e.list_occurrence_text_runs(&again.id,0).unwrap().runs.iter().any(|r|r.text=="Cafe safe"&&r.supported));
        retain(name,&input,&output,&restored);assert_eq!(fs::read(&input).unwrap(),original);
        for id in [&again.id,&changed.id,&reopened.id,&edited.id,&noop.id,&source.id]{e.close_document(id).unwrap();}
    }
}

fn split_input(row:&serde_json::Value,target:&Path){
    use lopdf::{content::{Content,Operation},Document,Object,Stream,dictionary};
    let mut pdf=Document::load(fixtures().join(row["path"].as_str().unwrap())).unwrap();let page=*pdf.get_pages().values().next().unwrap();
    let mut content=Content::decode(&pdf.get_page_content_with_limit(page,16*1024*1024).unwrap()).unwrap();
    let at=content.operations.iter().position(|op|op.operator=="Tj").unwrap();let bytes=content.operations[at].operands[0].as_str().unwrap().to_vec();let cut=row["codeLength"].as_u64().unwrap()as usize;
    content.operations.splice(at..=at,[Operation::new("Tj",vec![Object::string_literal(bytes[..cut].to_vec())]),Operation::new("Tj",vec![Object::string_literal(bytes[cut..].to_vec())])]);
    let stream=pdf.add_object(Stream::new(dictionary!{},content.encode().unwrap()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);pdf.save(target).unwrap();
}
#[test]
fn source_accent_clusters_groups_retain_physical_atoms_across_pieces(){
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for row in cases().into_iter().filter(|row|row["layout"]=="single"){
        let name=format!("{}-split-group",row["name"].as_str().unwrap());let input=temp.path().join(format!("{name}.pdf"));split_input(&row,&input);let original=fs::read(&input).unwrap();
        let source=e.open_document(&input).unwrap();let selected=e.inspect_text_group(&source.id,0,&[0,1]).unwrap();assert_eq!(selected.text,"Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301}");
        let noop=e.replace_text_group(&source.id,0,&[0,1],"Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301}","Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301}").unwrap();assert_eq!(e.render_page(&source.id,0,1000).unwrap(),e.render_page(&noop.id,0,1000).unwrap());
        let edited=e.replace_text_group(&source.id,0,&[0,1],"Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301}","Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301} this").unwrap();let output=temp.path().join(format!("{name}-edited.pdf"));save(&e,&edited,&output);
        let reopened=e.open_document(&output).unwrap();let runs=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;let selected=runs.iter().find(|r|r.text=="Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301} this").unwrap();assert!(selected.supported);
        let changed=e.replace_text_occurrence(&reopened.id,&[0],0,selected.object_index,&selected.text,"Cafe safe",None).unwrap().document;let deleted=temp.path().join(format!("{name}-deleted.pdf"));save(&e,&changed,&deleted);
        retain(&name,&input,&output,&deleted);assert_eq!(fs::read(&input).unwrap(),original);
        for id in [&changed.id,&reopened.id,&edited.id,&noop.id,&source.id]{e.close_document(id).unwrap();}
    }
}
#[test]
fn source_accent_clusters_shared_form_edits_keep_the_other_invocation(){
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for row in cases().into_iter().filter(|row|row["layout"]=="shared"){
        let name=row["name"].as_str().unwrap();let input=fixtures().join(row["path"].as_str().unwrap());let original=fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();
        let before=e.list_occurrence_form_text_runs(&source.id,0).unwrap().runs;assert_eq!(before.len(),2);assert!(before.iter().all(|r|r.text=="Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301}"&&r.supported),"{name}: {before:?}");
        let noop=e.replace_form_text_occurrence(&source.id,&[0],0,&before[0].object_path,"Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301}","Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301}",None).unwrap().document;assert_eq!(e.render_page(&source.id,0,1000).unwrap(),e.render_page(&noop.id,0,1000).unwrap());
        let edited=e.replace_form_text_occurrence(&source.id,&[0],0,&before[0].object_path,"Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301}","Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301} this",None).unwrap().document;let output=temp.path().join(format!("{name}-edited.pdf"));save(&e,&edited,&output);
        let reopened=e.open_document(&output).unwrap();let after=e.list_occurrence_form_text_runs(&reopened.id,0).unwrap().runs;assert_eq!(after.len(),2);assert_eq!(after[0].text,"Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301} this");assert!(after[0].supported);assert_eq!(after[1].text,before[1].text);assert_eq!(after[1].bounds,before[1].bounds);
        let changed=e.replace_form_text_occurrence(&reopened.id,&[0],0,&after[0].object_path,"Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301} this","Cafe safe",None).unwrap().document;let deleted=temp.path().join(format!("{name}-deleted.pdf"));save(&e,&changed,&deleted);
        let again=e.open_document(&deleted).unwrap();let reedited=e.list_occurrence_form_text_runs(&again.id,0).unwrap().runs;assert_eq!(reedited[0].text,"Cafe safe");assert!(reedited[0].supported);assert_eq!(reedited[1].text,"Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301}");assert_eq!(reedited[1].bounds,before[1].bounds);
        retain(name,&input,&output,&deleted);assert_eq!(fs::read(&input).unwrap(),original);
        for id in [&again.id,&changed.id,&reopened.id,&edited.id,&noop.id,&source.id]{e.close_document(id).unwrap();}
    }
}
fn request(s:folio_engine::StyledParagraphSelection)->folio_engine::StyledParagraphLayoutRequest{
    folio_engine::StyledParagraphLayoutRequest{baseline_anchor:s.baseline_anchor,object_indices:s.object_indices,expected_text:s.text,runs:s.runs,style_changes:vec![],bounds:s.bounds,alignment:s.alignment.unwrap_or(folio_engine::ParagraphAlignment::Left),line_spacing:s.line_spacing.unwrap_or(1.2)}
}
#[test]
fn source_accent_clusters_paragraph_reflow_and_saved_memory_keep_original_font(){
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for row in cases().into_iter().filter(|row|row["layout"]=="group"){
        let name=row["name"].as_str().unwrap();let input=fixtures().join(row["path"].as_str().unwrap());let original=fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();
        let selected=e.inspect_occurrence_styled_paragraph(&source.id,0,&[0,1]).unwrap();assert_eq!(selected.text,"Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301} safe");let mut next=request(selected);
        let noop=e.preview_styled_paragraph(&source.id,0,&next).unwrap();assert_eq!(e.render_page(&source.id,0,1000).unwrap(),e.render_page(&noop.id,0,1000).unwrap());
        next.bounds.width=300.;next.bounds.height=100.;for run in &mut next.runs{run.text=run.text.replace("Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301}","Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301} this");}
        let edited=e.preview_styled_paragraph(&source.id,0,&next).unwrap();let output=temp.path().join(format!("{name}-edited.pdf"));save(&e,&edited,&output);
        let reopened=e.open_document(&output).unwrap();let runs=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;let anchor=runs.iter().find(|r|r.text.contains("Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301}")).unwrap();
        let selected=e.suggest_occurrence_styled_paragraph(&reopened.id,0,anchor.object_index,&anchor.text).unwrap();assert_eq!(selected.text,"Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301} this safe");let mut next=request(selected);
        let replay=e.preview_styled_paragraph(&reopened.id,0,&next).unwrap();assert_eq!(e.render_page(&reopened.id,0,1000).unwrap(),e.render_page(&replay.id,0,1000).unwrap());
        for run in &mut next.runs{run.text=run.text.replace("Cafe\u{301} \u{3b1}\u{301} \u{438}\u{306} e\u{302}\u{301} this","Cafe safe");}
        let changed=e.preview_styled_paragraph(&reopened.id,0,&next).unwrap();let deleted=temp.path().join(format!("{name}-deleted.pdf"));save(&e,&changed,&deleted);retain(name,&input,&output,&deleted);
        assert_eq!(fs::read(&input).unwrap(),original);for id in [&changed.id,&replay.id,&reopened.id,&edited.id,&noop.id,&source.id]{e.close_document(id).unwrap();}
    }
}

fn first_show(bytes:&[u8])->Vec<u8>{
    let pdf=lopdf::Document::load_mem(bytes).unwrap();let page=*pdf.get_pages().values().next().unwrap();
    let content=lopdf::content::Content::decode(&pdf.get_page_content_with_limit(page,16*1024*1024).unwrap()).unwrap();
    content.operations.iter().find(|op|op.operator=="Tj").unwrap().operands[0].as_str().unwrap().to_vec()
}

#[test]
fn source_accent_clusters_reject_unproved_marks_and_unicode_normalization() {
    let e=engine();
    for row in cases().into_iter().filter(|r|r["layout"]=="single") {
        let input=fixtures().join(row["path"].as_str().unwrap());let original=fs::read(&input).unwrap();
        let source=e.open_document(&input).unwrap();let text=row["sourceText"].as_str().unwrap();
        let selected=e.list_occurrence_text_runs(&source.id,0).unwrap().runs.into_iter().find(|r|r.text==text).unwrap();
        assert!(selected.supported,"{}: {:?}",row["name"],selected);
        for rejected in ["\u{301}","e\u{306}","e\u{34f}","e\u{301}\u{301}","Café", "e\u{202e}","e\u{fe0f}"] {
            assert!(e.replace_text_occurrence(&source.id,&[0],0,selected.object_index,text,rejected,None).is_err(),"Admitted unavailable exact sequence: {rejected:?}");
        }
        assert_eq!(fs::read(&input).unwrap(),original);e.close_document(&source.id).unwrap();
    }
}

fn mutate_mapping(input:&Path,output:&Path,from:&str,to:&str){
    let mut pdf=lopdf::Document::load(input).unwrap();let page=*pdf.get_pages().values().next().unwrap();
    let resources=pdf.get_dictionary(page).unwrap().get(b"Resources").unwrap();
    let resources=pdf.dereference(resources).unwrap().1.as_dict().unwrap();
    let fonts=pdf.dereference(resources.get(b"Font").unwrap()).unwrap().1.as_dict().unwrap();
    let font=pdf.dereference(fonts.get(b"Source").unwrap()).unwrap().1.as_dict().unwrap();
    let id=font.get(b"ToUnicode").unwrap().as_reference().unwrap();
    let stream=pdf.get_object(id).unwrap().as_stream().unwrap();let data=stream.decompressed_content().unwrap_or_else(|_|stream.content.clone());
    let text=String::from_utf8(data).unwrap();assert!(text.contains(from));
    let stream=pdf.get_object_mut(id).unwrap().as_stream_mut().unwrap();stream.set_plain_content(text.replace(from,to).into_bytes());
    pdf.save(output).unwrap();
}
#[test]
fn source_accent_clusters_refuse_contradictory_aliases_and_declared_duplicate_codes(){
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for row in cases().into_iter().filter(|r|r["layout"]=="single") {
        let input=fixtures().join(row["path"].as_str().unwrap());let output=temp.path().join(format!("{}-alias.pdf",row["name"].as_str().unwrap()));
        // Raw CID CFF has no intrinsic Unicode alias/name; its source CMap owns
        // semantics. Only faces with actual alias evidence can contradict it.
        if row["label"]=="cid" && row["wrapped"]==false {continue;}
        // Known eacute cannot be relabelled as x + acute by ToUnicode alone.
        mutate_mapping(&input,&output,"<00650301>","<00780301>");
        let source=e.open_document(&output).unwrap();let runs=e.list_occurrence_text_runs(&source.id,0).unwrap().runs;
        assert!(runs.iter().any(|r|r.text.contains('\u{301}')&&!r.supported),"Contrary alias admitted: {runs:?}");e.close_document(&source.id).unwrap();
        if row["label"]=="ttf" {
            let duplicate=temp.path().join("duplicate.pdf");
            // Invalid alias participates in ambiguity proof before GID/width exclusion.
            mutate_mapping(&input,&duplicate,"endbfchar","endbfchar 1 beginbfchar <0258> <00650301> endbfchar");
            let source=e.open_document(&duplicate).unwrap();let runs=e.list_occurrence_text_runs(&source.id,0).unwrap().runs;
            assert!(runs.iter().any(|r|r.text.contains('\u{301}')&&!r.supported),"Duplicate source cluster admitted: {runs:?}");e.close_document(&source.id).unwrap();
        }
    }
}
#[test]
fn source_accent_clusters_do_not_relax_substitute_fonts_or_split_style_atoms(){
    let e=engine();let asset=e.fonts().register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec()).unwrap();
    for row in cases().into_iter().filter(|r|r["layout"]=="single") {
        let source=e.open_document(&fixtures().join(row["path"].as_str().unwrap())).unwrap();let text=row["sourceText"].as_str().unwrap();
        let selected=e.list_occurrence_text_runs(&source.id,0).unwrap().runs.into_iter().find(|r|r.text==text).unwrap();assert!(selected.supported);
        assert!(e.replace_text_occurrence(&source.id,&[0],0,selected.object_index,text,text,Some(&asset.id)).is_err(),"Explicit imported font gained combining layout authority");
        let selected=e.inspect_occurrence_styled_paragraph(&source.id,0,&[selected.object_index]).unwrap();let mut next=request(selected);
        next.runs=vec![folio_engine::StyledParagraphRun{text:"Cafe".into(),style_id:0},folio_engine::StyledParagraphRun{text:text[4..].into(),style_id:1}];
        next.style_changes=vec![folio_engine::StyledParagraphStyleChange{style_id:1,source_style_id:0,font_size:None,color:Some("#0044FF".into()),font_id:None}];
        assert!(e.preview_styled_paragraph(&source.id,0,&next).is_err(),"Style boundary split an indivisible physical accent glyph");
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn source_accent_clusters_mixed_intrinsic_aliases_refuse_but_equivalent_aliases_work(){
    let e=engine();let root=fixtures().parent().unwrap().join("alias-controls-v2");
    let value:serde_json::Value=serde_json::from_slice(&fs::read(root.join("authority.json")).unwrap()).unwrap();
    let mut rows=value["cases"].as_array().unwrap().clone();rows.sort_by_key(|row|!row["supported"].as_bool().unwrap());assert_eq!(rows.len(),8);
    for row in rows {
        let path=root.join(row["path"].as_str().unwrap());let source=e.open_document(&path).unwrap();let before=e.list_occurrence_text_runs(&source.id,0).unwrap().runs;
        let selected=before.iter().find(|r|r.text==row["sourceText"].as_str().unwrap()).unwrap();
        assert!(before.iter().any(|r|r.text=="Unchanged neighbor"&&r.supported),"Unrelated scalar was lost");
        assert_eq!(selected.supported,row["supported"].as_bool().unwrap(),"{}: {selected:?}",path.display());
        if selected.supported {
            let noop=e.replace_text_occurrence(&source.id,&[0],0,selected.object_index,&selected.text,&selected.text,None).unwrap().document;
            assert_eq!(e.render_page(&noop.id,0,1000).unwrap(),e.render_page(&source.id,0,1000).unwrap());e.close_document(&noop.id).unwrap();
        }
        e.close_document(&source.id).unwrap();
    }
}
