use super::*;
use serde_json::json;
use lopdf::{dictionary,Document,Object,Stream};
fn intent()->Value{json!({"version":1,"query":"Alpha","replacement":"First"})}
fn request(value:Value)->ReplacementRequest{serde_json::from_value(value).unwrap()}
#[test]
fn replacement_automation_intent_is_strict_bounded_and_distinguishes_preview(){
    request(intent()).validate(false).unwrap();
    let mut selected=intent();selected["selected"]=json!([]);request(selected).validate(false).unwrap();
    let mut preview=intent();preview["expectedPlanId"]=json!(format!("replacement-plan-v1-{}","a".repeat(64)));request(preview.clone()).validate(true).unwrap();assert!(request(preview).validate(false).is_err());assert!(request(intent()).validate(true).is_err());
    for(field,value)in [("version",json!(0)),("query",json!(" ")),("query",json!("a".repeat(1025))),("query",json!("a\u{0}")),("replacement",json!("a".repeat(4097))),("replacement",json!("x\u{0}")),("selected",json!([{"page":0,"matchIndex":0}])),("selected",json!([{"page":1,"matchIndex":1000}])),("selected",json!([{"page":1,"matchIndex":0},{"page":1,"matchIndex":0}]))]{let mut value2=intent();value2[field]=value;assert!(request(value2).validate(false).is_err(),"{field}");}
    for(field,value)in [("selected",Value::Null),("expectedPlanId",Value::Null),("path",json!([0])),("options",json!({"wholeWords":true,"unknown":true}))]{let mut value2=intent();value2[field]=value;assert!(serde_json::from_value::<ReplacementRequest>(value2).is_err());}
}
#[test]
fn replacement_automation_final_response_includes_escape_and_envelope_bytes(){
    check_response(&json!({"result":{"query":"Alpha"}})).unwrap();
    assert!(check_response(&json!({"result":{"text":"x".repeat(2*1024*1024)}})).is_err());
    assert!(check_response(&json!({"result":{"text":"\u{1}".repeat(400_000)}})).is_err());
}
fn input(path:&Path,generated:bool){
    let mut pdf=Document::with_version("1.7");let parent=pdf.new_object_id();let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let leaf=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),200.into(),30.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}},b"BT /F1 12 Tf 5 10 Td (Shared) Tj ET".to_vec()));let mut kids=vec![];
    for index in 0..2{
        let root=if generated&&index==0{"BT /F1 12 Tf 30 400 Td [(Alpha) -700 (Beta)] TJ ET"}else{"BT /F1 12 Tf 30 400 Td (Alpha tail) Tj ET"};let stream=pdf.add_object(Stream::new(dictionary!{},format!("{root} q 1 0 0 1 30 100 cm /S Do Q q 1 0 0 1 30 50 cm /S Do Q BT /F1 12 Tf 250 30 Td (Keep footer) Tj ET").into_bytes()));
        let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>parent,"MediaBox"=>vec![0.into(),0.into(),400.into(),500.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font},"XObject"=>dictionary!{"S"=>leaf}},"Contents"=>stream});kids.push(Object::Reference(page));
    }
    pdf.objects.insert(parent,dictionary!{"Type"=>"Pages","Count"=>2,"Kids"=>kids}.into());let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>parent,"Lang"=>Object::string_literal("en-US")});pdf.trailer.set("Root",root);std::fs::write(path,crate::pdf_output::save(&mut pdf).unwrap()).unwrap();
}
fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn store(path:&Path,value:&Value){std::fs::write(path,serde_json::to_vec(value).unwrap()).unwrap();}
fn planned(e:&PdfEngine,input:&Path,file:&Path)->Value{super::super::execute(e,Request::PlanReplacement{input:input.into(),request_file:file.into()},None).unwrap()}
fn previewed(e:&PdfEngine,input:&Path,file:&Path,parent:&Path)->EngineResult<Value>{super::super::execute(e,Request::PreviewReplacement{input:input.into(),request_file:file.into(),parent:parent.into()},None)}
#[test]
fn replacement_automation_reopen_identity_preview_save_and_original_are_stable(){
    let dir=tempfile::tempdir().unwrap();let input_file=dir.path().join("Original.pdf");input(&input_file,false);let original=std::fs::read(&input_file).unwrap();let file=dir.path().join("intent.json");store(&file,&intent());let e=engine();
    let a=planned(&e,&input_file,&file);let b=planned(&e,&input_file,&file);assert_eq!(a["planId"],b["planId"]);assert_eq!(a["matchCount"],2);assert_eq!(a["pages"][1]["page"],2);assert_eq!(a["canApply"],true);assert!(a.get("sourceId").is_none());
    let parent=dir.path().join("Previews");std::fs::create_dir(&parent).unwrap();let mut value=intent();value["expectedPlanId"]=a["planId"].clone();store(&file,&value);let p=previewed(&e,&input_file,&file,&parent).unwrap();assert_eq!(p["editCount"],2);
    let saved=dir.path().join("Saved.pdf");super::super::execute(&e,Request::SaveTextPreview{input:PathBuf::from(p["folder"].as_str().unwrap()),manifest_hash:p["manifestSha256"].as_str().unwrap().into(),output:saved.clone()},None).unwrap();
    let document=e.open_document(&saved).unwrap();for page in 0..2{let text=e.extract_text(&document.id,page).unwrap();assert!(text.contains("First tail")&&text.contains("Keep footer"));assert_eq!(text.matches("Shared").count(),2);}e.close_document(&document.id).unwrap();assert_eq!(std::fs::read(&input_file).unwrap(),original);
}
#[test]
fn replacement_automation_nested_subset_and_generated_native_identity_reach_existing_batch(){
    for generated in [false,true]{
        let dir=tempfile::tempdir().unwrap();let input_file=dir.path().join("Original.pdf");input(&input_file,generated);let e=engine();let file=dir.path().join("intent.json");
        let mut value=json!({"version":1,"query":if generated{"Beta"}else{"Shared"},"replacement":"New","selected":[{"page":1,"matchIndex":0}]});store(&file,&value);
        let p=planned(&e,&input_file,&file);assert_eq!(p["canApply"],true);value["expectedPlanId"]=p["planId"].clone();store(&file,&value);
        let result=previewed(&e,&input_file,&file,dir.path()).unwrap();assert_eq!(result["editCount"],1);
        let candidate=PathBuf::from(result["candidate"].as_str().unwrap());let doc=e.open_document(&candidate).unwrap();let text=e.extract_text(&doc.id,0).unwrap();
        if generated{
            assert!(text.contains("Alpha New"),"{text}");
            let runs=super::super::execute(&e,Request::InspectText{input:candidate,page:1,offset:0},None).unwrap();
            let run=runs["runs"].as_array().unwrap().iter().find(|run|run["text"]=="Alpha New").unwrap();assert_eq!(run["sourceText"],"AlphaNew");
            let manifest:Value=serde_json::from_slice(&std::fs::read(Path::new(result["folder"].as_str().unwrap()).join("manifest.json")).unwrap()).unwrap();assert_eq!(manifest["batch"]["edits"][0]["oldText"],"AlphaBeta");assert_eq!(manifest["batch"]["edits"][0]["replacement"],"AlphaNew");
        }else{
            assert!(text.contains("New"),"{text}");assert_eq!(text.matches("Shared").count(),1);assert_eq!(e.extract_text(&doc.id,1).unwrap().matches("Shared").count(),2);
        }
        e.close_document(&doc.id).unwrap();
    }
}
#[test]
fn replacement_automation_stale_blocked_noop_and_fit_fail_without_bundle(){
    let dir=tempfile::tempdir().unwrap();let input_file=dir.path().join("Original.pdf");input(&input_file,false);let original=std::fs::read(&input_file).unwrap();let file=dir.path().join("intent.json");let parent=dir.path().join("Previews");std::fs::create_dir(&parent).unwrap();let e=engine();
    for new in ["First".to_owned(),"Alpha".into(),"two\nlines".into(),"W".repeat(300)]{
        let mut value=json!({"version":1,"query":"Alpha","replacement":new});store(&file,&value);let p=planned(&e,&input_file,&file);value["expectedPlanId"]=if new=="First"{json!(format!("replacement-plan-v1-{}","0".repeat(64)))}else{p["planId"].clone()};store(&file,&value);assert!(previewed(&e,&input_file,&file,&parent).is_err());assert_eq!(std::fs::read_dir(&parent).unwrap().count(),0);
    }
    assert_eq!(std::fs::read(&input_file).unwrap(),original);
}
#[test]
fn replacement_automation_late_collision_after_applicable_later_page_leaves_no_partial_bundle(){
    let dir=tempfile::tempdir().unwrap();let input_file=dir.path().join("Crowded.pdf");input(&input_file,false);
    let mut pdf=Document::load(&input_file).unwrap();let page=*pdf.get_pages().values().next().unwrap();let content=pdf.get_dictionary(page).unwrap().get(b"Contents").unwrap().as_reference().unwrap();let stream=pdf.get_object_mut(content).unwrap().as_stream_mut().unwrap();let mut data=stream.get_plain_content_with_limit(16*1024*1024).unwrap();data.extend_from_slice(b" BT /F1 12 Tf 100 400 Td (Barrier) Tj ET");stream.set_content(data);let bytes=crate::pdf_output::save(&mut pdf).unwrap();std::fs::write(&input_file,&bytes).unwrap();
    let e=engine();let file=dir.path().join("intent.json");let mut value=json!({"version":1,"query":"Alpha","replacement":"Expanded value","selected":[{"page":2,"matchIndex":0}]});store(&file,&value);let last=planned(&e,&input_file,&file);assert_eq!(last["canApply"],true);value["expectedPlanId"]=last["planId"].clone();store(&file,&value);let control=dir.path().join("control");std::fs::create_dir(&control).unwrap();assert_eq!(previewed(&e,&input_file,&file,&control).unwrap()["editCount"],1);
    value.as_object_mut().unwrap().remove("selected");value.as_object_mut().unwrap().remove("expectedPlanId");store(&file,&value);let all=planned(&e,&input_file,&file);assert_eq!(all["canApply"],true);assert_eq!(all["editCount"],2);value["expectedPlanId"]=all["planId"].clone();store(&file,&value);
    let parent=dir.path().join("failed");std::fs::create_dir(&parent).unwrap();let error=previewed(&e,&input_file,&file,&parent).unwrap_err().to_string().to_lowercase();assert!(error.contains("preserv")||error.contains("overlap")||error.contains("placement"),"{error}");assert!(!error.contains("stale"));assert_eq!(std::fs::read_dir(&parent).unwrap().count(),0);assert_eq!(std::fs::read(&input_file).unwrap(),bytes);
}
