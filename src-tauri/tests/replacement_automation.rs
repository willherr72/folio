//! Actual public automation consumer through an ordinary optimized target.
use folio_engine::{PdfEngine,automation::{self,Request}};
use lopdf::{dictionary,Document,Object,Stream};
use std::{fs,path::PathBuf};
#[path="support/protected_fixture.rs"]mod protected_fixture;
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
fn execute(engine:&PdfEngine,request:Request,protected:bool)->serde_json::Value{
    automation::execute(engine,request,protected.then_some("reader")).unwrap()
}
#[test]
fn replacement_automation_public_complete_plans_preview_save_reopen(){
    let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();let temp=tempfile::tempdir().unwrap();
    for(name,query,replacement,selected,protected,count)in [
        ("root","Footer page","Note page",None,false,2),
        ("shared-first","Shared","Only here",Some(serde_json::json!([{"page":1,"matchIndex":0}])),false,1),
        ("shared-all","Shared","New",None,false,3),
        ("protected","Shared","New",None,true,3),
    ]{
        let original=if protected{protected_fixture::encrypt(&fixture(),lopdf::encryption::Permissions::all())}else{fixture()};let source=temp.path().join(format!("{name}.pdf"));fs::write(&source,&original).unwrap();
        let intent=temp.path().join(format!("{name}.json"));let mut request=serde_json::json!({"version":1,"query":query,"replacement":replacement,"options":{"wholeWords":true}});if let Some(selected)=selected{request["selected"]=selected;}
        fs::write(&intent,serde_json::to_vec(&request).unwrap()).unwrap();
        let planned=execute(&engine,Request::PlanReplacement{input:source.clone(),request_file:intent.clone()},protected);assert_eq!(planned["canApply"],true);assert_eq!(planned["selectedCount"],count);assert_eq!(planned["scannedPages"],2);
        let again=execute(&engine,Request::PlanReplacement{input:source.clone(),request_file:intent.clone()},protected);assert_eq!(again["planId"],planned["planId"]);
        request["expectedPlanId"]=planned["planId"].clone();fs::write(&intent,serde_json::to_vec(&request).unwrap()).unwrap();let parent=temp.path().join(format!("{name}-previews"));fs::create_dir(&parent).unwrap();
        let preview=execute(&engine,Request::PreviewReplacement{input:source.clone(),request_file:intent,parent},protected);assert_eq!(preview["editCount"],count);
        let folder=PathBuf::from(preview["folder"].as_str().unwrap());let saved=temp.path().join(format!("{name}-saved.pdf"));execute(&engine,Request::SaveTextPreview{input:folder.clone(),manifest_hash:preview["manifestSha256"].as_str().unwrap().into(),output:saved.clone()},false);
        assert_eq!(fs::read(&saved).unwrap(),fs::read(folder.join("candidate.pdf")).unwrap());assert_eq!(fs::read(&source).unwrap(),original);
        let info=if protected{engine.open_document_with_password(&saved,"reader").unwrap()}else{engine.open_document(&saved).unwrap()};let first=engine.extract_text(&info.id,0).unwrap();let second=engine.extract_text(&info.id,1).unwrap();assert!(first.contains(replacement));
        if name=="shared-first"{assert_eq!(first.matches("Shared").count(),1);assert_eq!(second.matches("Shared").count(),1);}else{assert!(second.contains(replacement));}
        engine.close_document(&info.id).unwrap();
        if let Some(root)=std::env::var_os("FOLIO_DOCUMENT_REPLACEMENT_OUTPUTS"){
            let root=PathBuf::from(root);assert!(root.is_dir());fs::copy(&source,root.join(format!("{name}.source.pdf"))).unwrap();fs::copy(&saved,root.join(format!("{name}.saved.pdf"))).unwrap();fs::write(root.join(format!("{name}.plan.json")),serde_json::to_vec_pretty(&planned).unwrap()).unwrap();
        }
    }
}
#[test]
fn replacement_automation_public_stale_source_and_late_blocker_leave_no_bundle(){
    let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("Original.pdf");let original=fixture();fs::write(&source,&original).unwrap();let intent=temp.path().join("intent.json");
    let mut request=serde_json::json!({"version":1,"query":"Shared","replacement":"New"});fs::write(&intent,serde_json::to_vec(&request).unwrap()).unwrap();let plan=execute(&engine,Request::PlanReplacement{input:source.clone(),request_file:intent.clone()},false);
    let alternate=temp.path().join("Changed.pdf");let mut pdf=Document::load_mem(&original).unwrap();let info=pdf.trailer.get(b"Info").unwrap().as_reference().unwrap();pdf.get_dictionary_mut(info).unwrap().set("Title",Object::string_literal("Deliberately changed authored input"));let mut changed=vec![];pdf.save_to(&mut changed).unwrap();fs::write(&alternate,&changed).unwrap();
    request["expectedPlanId"]=plan["planId"].clone();fs::write(&intent,serde_json::to_vec(&request).unwrap()).unwrap();let parent=temp.path().join("Previews");fs::create_dir(&parent).unwrap();assert!(automation::execute(&engine,Request::PreviewReplacement{input:alternate.clone(),request_file:intent.clone(),parent:parent.clone()},None).is_err());
    request=serde_json::json!({"version":1,"query":"Shared","replacement":""});fs::write(&intent,serde_json::to_vec(&request).unwrap()).unwrap();let blocked=execute(&engine,Request::PlanReplacement{input:source.clone(),request_file:intent.clone()},false);assert_eq!(blocked["canApply"],false);assert_eq!(blocked["blockedCount"],3);request["expectedPlanId"]=blocked["planId"].clone();fs::write(&intent,serde_json::to_vec(&request).unwrap()).unwrap();assert!(automation::execute(&engine,Request::PreviewReplacement{input:source.clone(),request_file:intent,parent:parent.clone()},None).is_err());
    assert_eq!(fs::read_dir(&parent).unwrap().count(),0);assert_eq!(fs::read(source).unwrap(),original);assert_eq!(fs::read(alternate).unwrap(),changed);
}
