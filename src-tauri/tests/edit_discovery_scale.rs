//! Dense-page discovery probes use the same two requests as the desktop layer.
use folio_engine::PdfEngine;
use lopdf::{dictionary, Document, Object, Stream};
use std::{path::{Path,PathBuf},time::Instant};
#[path="support/protected_fixture.rs"]
mod protected_fixture;

fn fixture(path:&Path,count:usize,forms:bool){
    let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let leaf=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),24.into(),8.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}},b"BT /F1 4 Tf 1 0 0 1 2 2 Tm (Old) Tj ET".to_vec()));
    let content=(0..count).map(|i|{
        let x=20+(i%20)*28;let y=20+(i/20)*9;
        if forms{format!("q 1 0 0 1 {x} {y} cm /Shared Do Q\n")}
        else{format!("BT /F1 4 Tf 1 0 0 1 {} {} Tm (Old) Tj ET\n",x+2,y+2)}
    }).collect::<String>();
    let content=pdf.add_object(Stream::new(dictionary!{},content.into_bytes()));
    let mut resources=dictionary!{"Font"=>dictionary!{"F1"=>font}};
    if forms{resources.set("XObject",dictionary!{"Shared"=>leaf});}else{pdf.objects.remove(&leaf);}
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),612.into(),792.into()],"Resources"=>resources,"Contents"=>content});
    pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into());
    let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);pdf.save(path).unwrap();
}

#[test]
#[ignore="Explicit performance/corpus probe; run with --ignored --nocapture"]
fn dense_edit_discovery_preserves_all_candidates_and_source(){
    let temp=tempfile::tempdir().unwrap();
    let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
    for count in [100,400,1600]{for forms in [false,true]{
        let path=temp.path().join(format!("dense-{count}-{}.pdf",if forms{"forms"}else{"top"}));
        fixture(&path,count,forms);let original=std::fs::read(&path).unwrap();
        let source=engine.open_document(&path).unwrap();let bytes=engine.source_bytes(&source.id).unwrap();
        let started=Instant::now();let top=engine.list_occurrence_text_runs(&source.id,0).unwrap();let top_ms=started.elapsed().as_millis();
        let started=Instant::now();let nested=engine.list_occurrence_form_text_runs(&source.id,0).unwrap();let form_ms=started.elapsed().as_millis();
        eprintln!("DISCOVERY {}",serde_json::json!({"count":count,"forms":forms,"topMs":top_ms,"formMs":form_ms,"topRuns":top.runs.len(),"formRuns":nested.runs.len(),"formReason":nested.reason}));
        if forms{assert_eq!(nested.runs.len(),count,"{:?}",nested.reason);assert!(nested.runs.iter().all(|r|r.supported&&r.text=="Old"));}
        // Native top-level extraction can append generated inter-object spaces.
        else{assert_eq!(top.runs.len(),count);assert!(top.runs.iter().all(|r|r.supported&&r.text.trim_end_matches(' ')=="Old"),"{:?}",top.runs.iter().find(|r|!r.supported||r.text.trim_end_matches(' ')!="Old"));assert!(nested.runs.is_empty());}
        assert_eq!(engine.source_bytes(&source.id).unwrap(),bytes);assert_eq!(std::fs::read(&path).unwrap(),original);
        if let Some(root)=std::env::var_os("FOLIO_DISCOVERY_ARTIFACTS"){
            let root=PathBuf::from(root);std::fs::create_dir_all(&root).unwrap();std::fs::copy(&path,root.join(path.file_name().unwrap())).unwrap();
        }
        engine.close_document(&source.id).unwrap();
    }}
}

#[test]
fn no_form_discovery_does_not_require_top_level_edit_validation(){
    let temp=tempfile::tempdir().unwrap();let path=temp.path().join("many-objects.pdf");
    // No form exists. The top-level mutation object limit is irrelevant to this
    // empty discovery result, so no private editing-page proof is needed.
    fixture(&path,5001,false);
    let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
    let source=engine.open_document(&path).unwrap();let before=engine.source_bytes(&source.id).unwrap();
    let result=engine.list_occurrence_form_text_runs(&source.id,0).unwrap();
    assert!(result.runs.is_empty());assert_eq!(result.reason,None);
    assert_eq!(engine.source_bytes(&source.id).unwrap(),before);engine.close_document(&source.id).unwrap();
}

#[test]
fn empty_form_discovery_distinguishes_page_content_from_resources_and_appearances(){
    let temp=tempfile::tempdir().unwrap();
    let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
    for case in ["unused","other-page","appearance"]{
        let path=temp.path().join(format!("{case}.pdf"));fixture(&path,1,true);
        let mut pdf=Document::load(&path).unwrap();let page=*pdf.get_pages().values().next().unwrap();
        let content=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 12 Tf 20 200 Td (Ordinary text) Tj ET".to_vec()));
        if case=="other-page"{
            let mut other=pdf.get_dictionary(page).unwrap().clone();let parent=other.get(b"Parent").unwrap().as_reference().unwrap();
            other.set("Parent",parent);let other=pdf.add_object(other);
            let pages=pdf.get_dictionary_mut(parent).unwrap();pages.set("Kids",vec![Object::Reference(page),Object::Reference(other)]);pages.set("Count",2);
        }
        if case=="appearance"{
            let form=pdf.get_dictionary(page).unwrap().get(b"Resources").unwrap().as_dict().unwrap().get(b"XObject").unwrap().as_dict().unwrap().get(b"Shared").unwrap().clone();
            let annotation=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Stamp","Rect"=>vec![200.into(),200.into(),224.into(),208.into()],"AP"=>dictionary!{"N"=>form}});
            pdf.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(annotation)]);
        }
        pdf.get_dictionary_mut(page).unwrap().set("Contents",content);pdf.save(&path).unwrap();
        let bytes=std::fs::read(&path).unwrap();let source=engine.open_document(&path).unwrap();
        let result=engine.list_occurrence_form_text_runs(&source.id,0).unwrap();assert!(result.runs.is_empty(),"{case}");assert_eq!(result.reason,None,"{case}");
        if case=="other-page"{let result=engine.list_occurrence_form_text_runs(&source.id,1).unwrap();assert_eq!(result.runs.len(),1);assert_eq!(result.runs[0].text,"Old");}
        assert_eq!(std::fs::read(&path).unwrap(),bytes);engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn empty_form_discovery_retains_source_permission_and_page_checks(){
    let temp=tempfile::tempdir().unwrap();
    let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
    for case in ["invalid-page","tagged","layer","denied","permitted"]{
        let path=temp.path().join(format!("{case}.pdf"));fixture(&path,1,false);
        let mut pdf=Document::load(&path).unwrap();let root=pdf.trailer.get(b"Root").unwrap().as_reference().unwrap();
        if case=="tagged"{pdf.get_dictionary_mut(root).unwrap().set("StructTreeRoot",dictionary!{});}
        // An unattached OCG still triggers the source text restriction.
        if case=="layer"{pdf.add_object(dictionary!{"Type"=>"OCG","Name"=>Object::string_literal("Layer")});}
        pdf.save(&path).unwrap();
        let encrypted=matches!(case,"denied"|"permitted");
        if encrypted{let p=if case=="denied"{lopdf::encryption::Permissions::COPYABLE}else{lopdf::encryption::Permissions::MODIFIABLE};std::fs::write(&path,protected_fixture::encrypt(&std::fs::read(&path).unwrap(),p)).unwrap();}
        let source=if encrypted{engine.open_document_with_password(&path,"reader")}else{engine.open_document(&path)}.unwrap();
        let result=engine.list_occurrence_form_text_runs(&source.id,if case=="invalid-page"{99}else{0});
        match case{
            "denied"=>assert!(matches!(result,Err(folio_engine::EngineError::PermissionDenied(_)))),
            "invalid-page"=>assert!(result.is_err()),
            "tagged"|"layer"=>{let result=result.unwrap();assert!(result.runs.is_empty());assert!(result.reason.is_some(),"{case}");},
            "permitted"=>{let result=result.unwrap();assert!(result.runs.is_empty());assert_eq!(result.reason,None);},
            _=>unreachable!(),
        }
        engine.close_document(&source.id).unwrap();
    }
    assert!(engine.list_occurrence_form_text_runs("missing",0).is_err());
}

#[test]
fn generated_inter_object_space_does_not_prevent_a_real_text_edit(){
    let temp=tempfile::tempdir().unwrap();let path=temp.path().join("separated-text.pdf");
    fixture(&path,100,false);let original=std::fs::read(&path).unwrap();
    let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
    let source=engine.open_document(&path).unwrap();let bytes=engine.source_bytes(&source.id).unwrap();
    let run=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs.remove(0);
    assert!(run.supported);assert_eq!(run.source_text.as_deref().unwrap_or(&run.text),"Old");
    // The authored show is (Old), but PDFium associates a generated separator
    // with this object. Both routes must edit its real characters successfully.
    let legacy=engine.replace_text(&source.id,0,run.object_index,&run.text,"New");
    let complete=engine.replace_text_occurrence(&source.id,&[0],0,run.object_index,&run.text,"New",None);
    eprintln!("GENERATED_SPACE {:?}: legacy={:?}, complete={:?}",run.text,
        legacy.as_ref().map(|_|()).map_err(|e|e.to_string()),complete.as_ref().map(|_|()).map_err(|e|e.to_string()));
    assert_eq!(engine.source_bytes(&source.id).unwrap(),bytes);assert_eq!(std::fs::read(&path).unwrap(),original);
    engine.close_document(&source.id).unwrap();
    let legacy=legacy.expect("A generated inter-object separator must not prevent editing source text");
    let complete=complete.expect("Complete-source edits must support the same real text");
    for id in [&legacy.id,&complete.document.id]{
        let runs=engine.list_occurrence_text_runs(id,0).unwrap();assert_eq!(runs.runs[0].source_text.as_deref().unwrap_or(&runs.runs[0].text),"New");
        assert!(runs.runs.iter().skip(1).all(|r|r.source_text.as_deref().unwrap_or(&r.text)=="Old"));engine.close_document(id).unwrap();
    }
}

#[test]
fn actual_spaces_remain_part_of_editable_text(){
    let temp=tempfile::tempdir().unwrap();let path=temp.path().join("spaces.pdf");
    let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
    for value in [" Old","Old ","Old  "," Old "]{
        fixture(&path,20,false);let mut pdf=Document::load(&path).unwrap();let page=*pdf.get_pages().values().next().unwrap();
        let data=String::from_utf8(pdf.get_page_content(page)).unwrap();
        let content=pdf.add_object(Stream::new(dictionary!{},data.replacen("(Old)",&format!("({value})"),1).into_bytes()));
        pdf.get_dictionary_mut(page).unwrap().set("Contents",content);pdf.save(&path).unwrap();
        let source=engine.open_document(&path).unwrap();let runs=engine.list_occurrence_text_runs(&source.id,0).unwrap();
        assert_eq!(runs.runs[0].source_text.as_deref().unwrap_or(&runs.runs[0].text),value,"Real source spaces must remain");
        let changed=engine.replace_text_occurrence(&source.id,&[0],0,0,&runs.runs[0].text," New ",None).unwrap();
        let run=engine.list_occurrence_text_runs(&changed.document.id,0).unwrap().runs.remove(0);
        assert_eq!(run.source_text.as_deref().unwrap_or(&run.text)," New ");
        engine.close_document(&changed.document.id).unwrap();engine.close_document(&source.id).unwrap();
    }
}
