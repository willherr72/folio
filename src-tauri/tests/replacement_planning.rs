//! Public optimized native API witnesses, independent of worker-private tests.
use folio_engine::{PdfEngine,ExportRequest,PagePlan,TextReplacementBlocker};
use lopdf::{dictionary,Document,Object,Stream};
use sha2::{Digest,Sha256};
use std::{fs,path::{Path,PathBuf}};
#[path="support/protected_fixture.rs"]
mod protected_fixture;
fn hash(bytes:&[u8])->String{Sha256::digest(bytes).iter().map(|b|format!("{b:02x}")).collect()}
fn fixture()->Vec<u8>{
    let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let leaf=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),200.into(),30.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}},b"0.15 g BT /F1 12 Tf 5 10 Td (Shared) Tj ET".to_vec()));
    let mut kids=vec![];
    for i in 0..2 {
        let stream=if i==0{"0.1 0.2 0.5 rg BT /F1 12 Tf 30 330 Td (Alpha) Tj ET BT /F1 12 Tf 30 280 Td (Beta) Tj ET q 1 0 0 1 30 150 cm /Shared Do Q q 1 0 0 1 30 100 cm /Shared Do Q 0 g BT /F1 10 Tf 30 20 Td (Footer page 1 stays) Tj ET"}else{"0.1 0.2 0.5 rg BT /F1 12 Tf 30 330 Td (Gamma) Tj ET BT /F1 12 Tf 30 280 Td (Delta) Tj ET q 1 0 0 1 30 150 cm /Shared Do Q 0 g BT /F1 10 Tf 30 20 Td (Footer page 2 stays) Tj ET"};
        let contents=pdf.add_object(Stream::new(dictionary!{},stream.as_bytes().to_vec()));let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),500.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font},"XObject"=>dictionary!{"Shared"=>leaf}},"Contents"=>contents});kids.push(page);
    }
    let note=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![350.into(),20.into(),370.into(),40.into()],"Contents"=>Object::string_literal("Keep this note"),"F"=>4});
    let link=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Link","Rect"=>vec![280.into(),280.into(),380.into(),300.into()],"Border"=>vec![0.into(),0.into(),0.into()],"Dest"=>vec![Object::Reference(kids[1]),Object::Name(b"Fit".to_vec())]});
    pdf.get_dictionary_mut(kids[0]).unwrap().set("Annots",vec![Object::Reference(note),Object::Reference(link)]);
    let appearance=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),30.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}},b"q 1 g 0 0 100 30 re f 0 g BT /F1 10 Tf 5 10 Td (Kept answer) Tj ET Q".to_vec()));
    let field=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>Object::string_literal("answer"),"V"=>Object::string_literal("Kept answer"),"DV"=>Object::string_literal("Kept answer"),"DA"=>Object::string_literal("/F1 10 Tf 0 g"),"Rect"=>vec![280.into(),40.into(),380.into(),70.into()],"F"=>4,"P"=>kids[1],"AP"=>dictionary!{"N"=>appearance}});
    pdf.get_dictionary_mut(kids[1]).unwrap().set("Annots",vec![Object::Reference(field)]);
    let acro=pdf.add_object(dictionary!{"Fields"=>vec![Object::Reference(field)],"DR"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"DA"=>Object::string_literal("/F1 10 Tf 0 g"),"NeedAppearances"=>false});
    let outlines=pdf.new_object_id();let item=pdf.add_object(dictionary!{"Title"=>Object::string_literal("Second page"),"Parent"=>outlines,"Dest"=>vec![Object::Reference(kids[1]),Object::Name(b"Fit".to_vec())]});pdf.objects.insert(outlines,dictionary!{"Type"=>"Outlines","First"=>item,"Last"=>item,"Count"=>1}.into());
    pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>2,"Kids"=>kids.into_iter().map(Object::Reference).collect::<Vec<_>>()}.into());
    let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages,"AcroForm"=>acro,"Outlines"=>outlines,"Lang"=>Object::string_literal("en-US")});pdf.trailer.set("Root",catalog);let info=pdf.add_object(dictionary!{"Title"=>Object::string_literal("Public planner fixture"),"Author"=>Object::string_literal("Folio authored QA")});pdf.trailer.set("Info",info);let mut bytes=vec![];pdf.save_to(&mut bytes).unwrap();bytes
}
fn retain(name:&str,source:&Path,saved:&Path,plan:&folio_engine::TextReplacementPagePlan){
    if let Some(root)=std::env::var_os("FOLIO_REPLACEMENT_PLAN_OUTPUTS"){
        let root=PathBuf::from(root);fs::create_dir_all(&root).unwrap();fs::copy(source,root.join(format!("{name}.source.pdf"))).unwrap();fs::copy(saved,root.join(format!("{name}.saved.pdf"))).unwrap();fs::write(root.join(format!("{name}.plan.json")),serde_json::to_vec_pretty(plan).unwrap()).unwrap();
    }
}
#[test]
fn public_planner_preview_edits_apply_save_reopen_and_preserve_originals(){
    let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();let temp=tempfile::tempdir().unwrap();
    for(name,query,replacement,selected,protected)in[("root","Alpha","First",None,false),("shared-first","Shared","Only here",Some(&[0][..]),false),("shared-both","Shared","New",None,false),("protected","Alpha","First",None,true)]{
        let original=if protected{protected_fixture::encrypt(&fixture(),lopdf::encryption::Permissions::all())}else{fixture()};let path=temp.path().join(format!("{name}.pdf"));fs::write(&path,&original).unwrap();
        let source=if protected{engine.open_document_with_password(&path,"reader").unwrap()}else{engine.open_document(&path).unwrap()};let snapshot=engine.source_bytes(&source.id).unwrap();
        let plan=engine.plan_text_replacement_page(&source.id,0,query,replacement,Default::default(),selected).unwrap();assert!(plan.can_apply,"{:?}",plan.matches);assert_eq!(plan.source_fingerprint,hash(&snapshot));
        let candidate=engine.replace_text_batch(&source.id,&plan.edits).unwrap();let saved=temp.path().join(format!("{name}-saved.pdf"));
        engine.export_pdf(ExportRequest{pages:candidate.document.pages.iter().enumerate().map(|(i,p)|PagePlan{id:format!("{name}-{i}"),source_id:candidate.document.id.clone(),page_index:i,width:p.width,height:p.height,rotation:0,crop:None,overlays:p.overlays.clone()}).collect(),flatten:false},&saved).unwrap();
        let reopened=if protected{engine.open_document_with_password(&saved,"reader").unwrap()}else{engine.open_document(&saved).unwrap()};let text=engine.extract_text(&reopened.id,0).unwrap();assert!(text.contains(replacement),"{text}");assert_eq!(hash(&engine.source_bytes(&source.id).unwrap()),hash(&snapshot));assert_eq!(hash(&fs::read(&path).unwrap()),hash(&original));
        if name=="shared-first"{assert_eq!(text.matches("Only here").count(),1);assert_eq!(text.matches("Shared").count(),1);}
        if name=="shared-both"{assert_eq!(text.matches("New").count(),2);assert!(!text.contains("Shared"));}
        assert!(engine.extract_text(&reopened.id,1).unwrap().contains("Gamma"));retain(name,&path,&saved,&plan);
        for id in [&source.id,&candidate.document.id,&reopened.id]{engine.close_document(id).unwrap();}
    }
}
#[test]
fn public_planner_cross_object_and_stale_subsets_return_no_candidate_edits(){
    let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("cross.pdf");let bytes=fixture();fs::write(&path,&bytes).unwrap();let source=engine.open_document(&path).unwrap();
    let plan=engine.plan_text_replacement_page(&source.id,0,"Alpha Beta","Both",Default::default(),None).unwrap();assert_eq!(plan.matches.len(),1);assert_eq!(plan.matches[0].blocker,Some(TextReplacementBlocker::CrossObject));assert!(!plan.can_apply);assert!(plan.edits.is_empty());
    assert!(engine.plan_text_replacement_page(&source.id,0,"Alpha","First",Default::default(),Some(&[1])).is_err());assert_eq!(hash(&engine.source_bytes(&source.id).unwrap()),hash(&bytes));assert_eq!(hash(&fs::read(path).unwrap()),hash(&bytes));engine.close_document(&source.id).unwrap();
}
