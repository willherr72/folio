//! Original CFF glyphs, source codes and saved editing, over public-font witnesses.
use folio_engine::{DocumentInfo,ExportRequest,PagePlan,PdfEngine};
use std::{fs,path::{Path,PathBuf}};

fn fixtures()->PathBuf {PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/source-cff/ligatures-v1")}
fn engine()->PdfEngine {PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn cases()->Vec<serde_json::Value>{serde_json::from_slice::<serde_json::Value>(&fs::read(fixtures().join("authority.json")).unwrap()).unwrap()["cases"].as_array().unwrap().clone()}
fn save(e:&PdfEngine,d:&DocumentInfo,path:&Path){
    e.export_pdf(ExportRequest{flatten:false,pages:d.pages.iter().enumerate().map(|(page_index,p)|PagePlan{crop:None,id:format!("p{page_index}"),source_id:d.id.clone(),page_index,width:p.width,height:p.height,rotation:0,overlays:vec![]}).collect()},path).unwrap();
}
fn retain(name:&str,original:&Path,edited:&Path,restored:&Path){
    if let Some(root)=std::env::var_os("FOLIO_CFF_LIGATURE_OUTPUTS"){
        let dir=PathBuf::from(root).join(name);fs::create_dir_all(&dir).unwrap();
        for (source,target) in [(original,"original.pdf"),(edited,"edited.pdf"),(restored,"restored.pdf")]{fs::copy(source,dir.join(target)).unwrap();}
    }
}
#[test]
fn cff_source_ligatures_original_font_noop_growth_delete_save_and_reedit(){
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for row in cases().into_iter().filter(|row|row["layout"]=="single"){
        let name=row["name"].as_str().unwrap();let input=fixtures().join(row["path"].as_str().unwrap());let original=fs::read(&input).unwrap();
        let source=e.open_document(&input).unwrap();let before=e.list_occurrence_text_runs(&source.id,0).unwrap().runs;
        let selected=before.iter().find(|r|r.text=="Office").expect("Exact source text");assert!(selected.supported,"{name}: {before:?}");
        let noop=e.replace_text_occurrence(&source.id,&[0],0,selected.object_index,"Office","Office",None).unwrap().document;
        assert!(e.render_page(&noop.id,0,1000).unwrap()==e.render_page(&source.id,0,1000).unwrap(),"No-op changed whole-page pixels: {name}");
        assert_eq!(first_show(&e.source_bytes(&noop.id).unwrap()),first_show(&original),"No-op changed physical source glyph codes: {name}");
        let edited=e.replace_text_occurrence(&source.id,&[0],0,selected.object_index,"Office","Office this",None).unwrap().document;
        let output=temp.path().join(format!("{name}-edited.pdf"));save(&e,&edited,&output);
        let reopened=e.open_document(&output).unwrap();let after=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
        let selected=after.iter().find(|r|r.text=="Office this").unwrap();assert!(selected.supported,"{name}: {after:?}");
        let neighbor=before.iter().find(|r|r.text=="Unchanged neighbor").unwrap();assert_eq!(after.iter().find(|r|r.text==neighbor.text).unwrap(),neighbor);
        let changed=e.replace_text_occurrence(&reopened.id,&[0],0,selected.object_index,"Office this","Ofice",None).unwrap().document;
        let restored=temp.path().join(format!("{name}-deleted.pdf"));save(&e,&changed,&restored);
        let again=e.open_document(&restored).unwrap();assert!(e.list_occurrence_text_runs(&again.id,0).unwrap().runs.iter().any(|r|r.text=="Ofice"&&r.supported));
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
fn cff_source_ligatures_groups_retain_physical_atoms_across_pieces(){
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for row in cases().into_iter().filter(|row|row["layout"]=="single"){
        let name=format!("{}-split-group",row["name"].as_str().unwrap());let input=temp.path().join(format!("{name}.pdf"));split_input(&row,&input);let original=fs::read(&input).unwrap();
        let source=e.open_document(&input).unwrap();let selected=e.inspect_text_group(&source.id,0,&[0,1]).unwrap();assert_eq!(selected.text,"Office");
        let noop=e.replace_text_group(&source.id,0,&[0,1],"Office","Office").unwrap();assert_eq!(e.render_page(&source.id,0,1000).unwrap(),e.render_page(&noop.id,0,1000).unwrap());
        let edited=e.replace_text_group(&source.id,0,&[0,1],"Office","Office this").unwrap();let output=temp.path().join(format!("{name}-edited.pdf"));save(&e,&edited,&output);
        let reopened=e.open_document(&output).unwrap();let runs=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;let selected=runs.iter().find(|r|r.text=="Office this").unwrap();assert!(selected.supported);
        let changed=e.replace_text_occurrence(&reopened.id,&[0],0,selected.object_index,&selected.text,"Ofice",None).unwrap().document;let deleted=temp.path().join(format!("{name}-deleted.pdf"));save(&e,&changed,&deleted);
        retain(&name,&input,&output,&deleted);assert_eq!(fs::read(&input).unwrap(),original);
        for id in [&changed.id,&reopened.id,&edited.id,&noop.id,&source.id]{e.close_document(id).unwrap();}
    }
}
#[test]
fn cff_source_ligatures_shared_form_edits_keep_the_other_invocation(){
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for row in cases().into_iter().filter(|row|row["layout"]=="shared"){
        let name=row["name"].as_str().unwrap();let input=fixtures().join(row["path"].as_str().unwrap());let original=fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();
        let before=e.list_occurrence_form_text_runs(&source.id,0).unwrap().runs;assert_eq!(before.len(),2);assert!(before.iter().all(|r|r.text=="Office"&&r.supported),"{name}: {before:?}");
        let noop=e.replace_form_text_occurrence(&source.id,&[0],0,&before[0].object_path,"Office","Office",None).unwrap().document;assert_eq!(e.render_page(&source.id,0,1000).unwrap(),e.render_page(&noop.id,0,1000).unwrap());
        let edited=e.replace_form_text_occurrence(&source.id,&[0],0,&before[0].object_path,"Office","Office this",None).unwrap().document;let output=temp.path().join(format!("{name}-edited.pdf"));save(&e,&edited,&output);
        let reopened=e.open_document(&output).unwrap();let after=e.list_occurrence_form_text_runs(&reopened.id,0).unwrap().runs;assert_eq!(after.len(),2);assert_eq!(after[0].text,"Office this");assert!(after[0].supported);assert_eq!(after[1].text,before[1].text);assert_eq!(after[1].bounds,before[1].bounds);
        let changed=e.replace_form_text_occurrence(&reopened.id,&[0],0,&after[0].object_path,"Office this","Ofice",None).unwrap().document;let deleted=temp.path().join(format!("{name}-deleted.pdf"));save(&e,&changed,&deleted);
        let again=e.open_document(&deleted).unwrap();let reedited=e.list_occurrence_form_text_runs(&again.id,0).unwrap().runs;assert_eq!(reedited[0].text,"Ofice");assert!(reedited[0].supported);assert_eq!(reedited[1].text,"Office");assert_eq!(reedited[1].bounds,before[1].bounds);
        retain(name,&input,&output,&deleted);assert_eq!(fs::read(&input).unwrap(),original);
        for id in [&again.id,&changed.id,&reopened.id,&edited.id,&noop.id,&source.id]{e.close_document(id).unwrap();}
    }
}
fn request(s:folio_engine::StyledParagraphSelection)->folio_engine::StyledParagraphLayoutRequest{
    folio_engine::StyledParagraphLayoutRequest{baseline_anchor:s.baseline_anchor,object_indices:s.object_indices,expected_text:s.text,runs:s.runs,style_changes:vec![],bounds:s.bounds,alignment:s.alignment.unwrap_or(folio_engine::ParagraphAlignment::Left),line_spacing:s.line_spacing.unwrap_or(1.2)}
}
#[test]
fn cff_source_ligatures_paragraph_reflow_and_saved_memory_keep_original_font(){
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for row in cases().into_iter().filter(|row|row["layout"]=="group"){
        let name=row["name"].as_str().unwrap();let input=fixtures().join(row["path"].as_str().unwrap());let original=fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();
        let selected=e.inspect_occurrence_styled_paragraph(&source.id,0,&[0,1]).unwrap();assert_eq!(selected.text,"Office safe");let mut next=request(selected);
        let noop=e.preview_styled_paragraph(&source.id,0,&next).unwrap();assert_eq!(e.render_page(&source.id,0,1000).unwrap(),e.render_page(&noop.id,0,1000).unwrap());
        next.bounds.width=300.;next.bounds.height=100.;for run in &mut next.runs{run.text=run.text.replace("Office","Office this");}
        let edited=e.preview_styled_paragraph(&source.id,0,&next).unwrap();let output=temp.path().join(format!("{name}-edited.pdf"));save(&e,&edited,&output);
        let reopened=e.open_document(&output).unwrap();let runs=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;let anchor=runs.iter().find(|r|r.text.contains("Office")).unwrap();
        let selected=e.suggest_occurrence_styled_paragraph(&reopened.id,0,anchor.object_index,&anchor.text).unwrap();assert_eq!(selected.text,"Office this safe");let mut next=request(selected);
        let replay=e.preview_styled_paragraph(&reopened.id,0,&next).unwrap();assert_eq!(e.render_page(&reopened.id,0,1000).unwrap(),e.render_page(&replay.id,0,1000).unwrap());
        for run in &mut next.runs{run.text=run.text.replace("Office this","Ofice");}
        let changed=e.preview_styled_paragraph(&reopened.id,0,&next).unwrap();let deleted=temp.path().join(format!("{name}-deleted.pdf"));save(&e,&changed,&deleted);retain(name,&input,&output,&deleted);
        assert_eq!(fs::read(&input).unwrap(),original);for id in [&changed.id,&replay.id,&reopened.id,&edited.id,&noop.id,&source.id]{e.close_document(id).unwrap();}
    }
}

fn first_show(bytes:&[u8])->Vec<u8>{
    let pdf=lopdf::Document::load_mem(bytes).unwrap();let page=*pdf.get_pages().values().next().unwrap();
    let content=lopdf::content::Content::decode(&pdf.get_page_content_with_limit(page,16*1024*1024).unwrap()).unwrap();
    content.operations.iter().find(|op|op.operator=="Tj").unwrap().operands[0].as_str().unwrap().to_vec()
}
