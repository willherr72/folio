//! Actual authorized multi-document MCP composition workflows.
use super::*;
#[path="page_organization.rs"]mod fixture;
#[path="protected_fixture.rs"]mod protected_fixture;
use std::fs;
fn invoke(s:&mut Session,id:u32,legacy:bool,name:&str,args:Value)->Value{
 if legacy{s.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}}));let value=s.receive();assert_eq!(value["id"],id);value}else{call(s,id,name,args)}
}
fn capture(name:&str,inputs:&[&Path],output:&Path,selection:Value){
 if let Some(root)=std::env::var_os("FOLIO_COMPOSITION_AUTOMATION_OUTPUTS"){
  let dir=std::path::PathBuf::from(root).join(name);fs::create_dir_all(&dir).unwrap();for(i,path)in inputs.iter().enumerate(){fs::copy(path,dir.join(format!("source-{}.pdf",i+1))).unwrap();}
  fs::copy(output,dir.join("output.pdf")).unwrap();fs::write(dir.join("selection.json"),serde_json::to_vec(&selection).unwrap()).unwrap();
 }
}
#[test]fn composition_both_protocols_preserve_independent_forms_and_continue_text_edits(){
 for legacy in[false,true]{
  let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a.pdf");let original=fixture::write(&a);let b=dir.path().join("b.pdf");fs::write(&b,&original).unwrap();let output=dir.path().join("out");fs::create_dir(&output).unwrap();let mut s=Session::start_many(&[&a,&b],&output);
  if legacy{s.send(json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"composition-qa","version":"1"}}}));assert_eq!(s.receive()["result"]["protocolVersion"],"2025-11-25");s.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));}
  let before=success(invoke(&mut s,1,legacy,"folio_info",json!({"documentId":"document-1"})));
  let merged=success(invoke(&mut s,2,legacy,"folio_merge_documents",json!({"documentIds":["document-1","document-2"],"pages":[{"source":2,"page":2,"rotation":90},{"source":1,"page":2},{"source":2,"page":2,"rotation":270},{"source":1,"page":3}]})));assert_eq!(merged["documentId"],"document-3");assert_eq!(merged["pageCount"],4);assert!(merged.get("path").is_none());assert!(merged.get("protectionSource").is_none());assert_eq!(fs::read_dir(&output).unwrap().count(),0);
  let mut fields=Vec::new();for(page,id)in[(1,3),(2,4),(3,5)]{fields.push(success(invoke(&mut s,id,legacy,"folio_inspect_forms",json!({"documentId":merged["documentId"],"page":page}))));}
  assert_eq!(fields[0]["fields"][0]["name"],fields[2]["fields"][0]["name"]);assert_ne!(fields[0]["fields"][0]["name"],fields[1]["fields"][0]["name"]);
  let inspected=success(invoke(&mut s,6,legacy,"folio_inspect_text",json!({"documentId":merged["documentId"],"page":4})));let target=inspected["runs"].as_array().unwrap().iter().find(|v|v["text"]=="Page three content").unwrap();assert_eq!(target["supported"],true);
  let preview=success(invoke(&mut s,7,legacy,"folio_preview_text",json!({"documentId":merged["documentId"],"page":4,"selectionId":target["selectionId"],"replacement":"Page three contents"})));
  let applied=success(invoke(&mut s,8,legacy,"folio_apply_preview",json!({"previewId":preview["previewId"]})));assert_eq!(applied["documentId"],"document-4");
  let saved=success(invoke(&mut s,9,legacy,"folio_export_composition",json!({"documentIds":[applied["documentId"]]})));assert_eq!(saved["sha256"],applied["sha256"]);assert!(saved.get("protectionSource").is_none());
  let again=success(invoke(&mut s,10,legacy,"folio_export_composition",json!({"documentIds":[applied["documentId"]]})));assert_ne!(saved["path"],again["path"]);assert_eq!(saved["sha256"],again["sha256"]);
  assert_eq!(success(invoke(&mut s,11,legacy,"folio_info",json!({"documentId":"document-1"}))),before);success(invoke(&mut s,12,legacy,"folio_close_document",json!({"documentId":merged["documentId"]})));
  let path=Path::new(saved["path"].as_str().unwrap());capture(&format!("mcp-composition-{legacy}"),&[&a,&b],path,json!({"version":1,"pages":[{"source":2,"page":2,"rotation":90},{"source":1,"page":2},{"source":2,"page":2,"rotation":270},{"source":1,"page":3}],"replacementPage":4,"oldText":"Page three content","replacement":"Page three contents"}));s.stop();assert!(path.exists());assert_eq!(fs::read(&a).unwrap(),original);assert_eq!(fs::read(&b).unwrap(),original);
 }
}

#[test]fn composition_schema_and_invalid_arguments_keep_sources_and_output_unchanged(){
 let dir=tempfile::tempdir().unwrap();let source=dir.path().join("a.pdf");let original=fixture::write(&source);let output=dir.path().join("out");fs::create_dir(&output).unwrap();let mut s=Session::start(&source,&output);
 let tools=s.request(1,"tools/list",json!({}));for name in["folio_merge_documents","folio_export_composition"]{
  let tool=tools["result"]["tools"].as_array().unwrap().iter().find(|v|v["name"]==name).unwrap();assert_eq!(tool["inputSchema"]["additionalProperties"],false);assert_eq!(tool["inputSchema"]["properties"]["documentIds"]["maxItems"],64);assert_eq!(tool["annotations"],json!({"readOnlyHint":false,"destructiveHint":false,"idempotentHint":false,"openWorldHint":false}));
 }
 for(i,args)in[json!({"documentIds":[]}),json!({"documentIds":["unknown"]}),json!({"documentIds":["document-1"],"pages":null}),json!({"documentIds":["document-1"],"pages":[]}),json!({"documentIds":["document-1"],"pages":[{"source":2,"page":1}]}),json!({"documentIds":["document-1"],"input":"outside.pdf"}),json!({"documentIds":["document-1"],"pages":[{"source":1,"page":1,"rotation":45}]})].into_iter().enumerate(){let result=call(&mut s,10+i as u32,"folio_merge_documents",args);assert!(result.get("error").is_some()||result["result"]["isError"]==true);}
 assert_eq!(success(call(&mut s,30,"folio_list_documents",json!({})))["documents"].as_array().unwrap().len(),1);assert_eq!(fs::read_dir(output).unwrap().count(),0);s.stop();assert_eq!(fs::read(source).unwrap(),original);
}

#[test]fn composition_later_protected_source_credential_survives_source_id_closure(){
 use lopdf::encryption::Permissions;
 let dir=tempfile::tempdir().unwrap();let a=dir.path().join("plain.pdf");let bytes=fixture::write(&a);let b=dir.path().join("protected.pdf");fs::write(&b,protected_fixture::encrypt(&bytes,Permissions::ASSEMBLABLE|Permissions::MODIFIABLE|Permissions::ANNOTABLE|Permissions::FILLABLE|Permissions::COPYABLE)).unwrap();let output=dir.path().join("out");fs::create_dir(&output).unwrap();let credentials=dir.path().join("credentials.json");fs::write(&credentials,br#"[null,"reader"]"#).unwrap();let mut s=Session::start_with_credentials(&[&a,&b],&output,Some(&credentials));
 let merged=success(call(&mut s,1,"folio_merge_documents",json!({"documentIds":["document-1","document-2"]})));assert_eq!(merged["pageCount"],6);assert!(merged.get("protectionSource").is_none());success(call(&mut s,2,"folio_close_document",json!({"documentId":"document-1"})));success(call(&mut s,3,"folio_close_document",json!({"documentId":"document-2"})));
 assert_eq!(success(call(&mut s,4,"folio_info",json!({"documentId":merged["documentId"]})))["pageCount"],6);
 let inspected=success(call(&mut s,5,"folio_inspect_text",json!({"documentId":merged["documentId"],"page":6})));let target=inspected["runs"].as_array().unwrap().iter().find(|v|v["text"]=="Page three content").unwrap();
 let preview=success(call(&mut s,6,"folio_preview_text",json!({"documentId":merged["documentId"],"page":6,"selectionId":target["selectionId"],"replacement":"Page three contents"})));let applied=success(call(&mut s,7,"folio_apply_preview",json!({"previewId":preview["previewId"]})));
 let saved=success(call(&mut s,8,"folio_export_composition",json!({"documentIds":[applied["documentId"]]})));assert!(!saved.to_string().contains("reader"));let path=Path::new(saved["path"].as_str().unwrap());capture("mcp-protected-composition",&[&a,&b],path,json!({"replacementPage":6,"oldText":"Page three content","replacement":"Page three contents"}));s.stop();assert!(path.exists());let engine=folio_engine::PdfEngine::start(Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();assert!(engine.open_document(path).is_err());let opened=engine.open_document_with_password(path,"reader").unwrap();assert!(engine.extract_text(&opened.id,5).unwrap().contains("Page three contents"));assert_eq!(fs::read(a).unwrap(),bytes);
}

#[test]fn composition_actual_maximum_frame_id_is_lossless_before_publication(){
 let dir=tempfile::tempdir().unwrap();let source=dir.path().join("source.pdf");fixture::write(&source);let output=dir.path().join("out");fs::create_dir(&output).unwrap();let mut s=Session::start(&source,&output);
 let mut message=json!({"jsonrpc":"2.0","id":"","method":"tools/call","params":{"name":"folio_export_composition","arguments":{"documentIds":["document-1"]},"_meta":meta()}});
 let cap=512*1024;let overhead=message.to_string().len()+1;let mut id="\"\\é".repeat((cap-overhead)/6);id.push_str(&"x".repeat((cap-overhead)%6));message["id"]=json!(id.clone());let frame=format!("{message}\n");assert_eq!(frame.len(),cap);s.raw(&frame);
 let reply=s.receive();assert_eq!(reply["id"].as_str(),Some(id.as_str()));let saved=success(reply);assert_eq!(saved["pageCount"],3);let path=Path::new(saved["path"].as_str().unwrap());capture("mcp-maximum-frame-copy",&[&source],path,Value::Null);s.stop();assert!(path.exists());
}

#[test]fn composition_full_session_refusal_recovers_without_consuming_document_id(){
 let dir=tempfile::tempdir().unwrap();let source=dir.path().join("source.pdf");fixture::write(&source);let output=dir.path().join("out");fs::create_dir(&output).unwrap();let inputs=vec![source.as_path();64];let mut s=Session::start_many(&inputs,&output);
 let denied=call(&mut s,1,"folio_merge_documents",json!({"documentIds":["document-1","document-2"]}));assert_eq!(denied["result"]["isError"],true);assert_eq!(denied["result"]["structuredContent"]["error"]["code"],"document_limit");
 success(call(&mut s,2,"folio_close_document",json!({"documentId":"document-64"})));let merged=success(call(&mut s,3,"folio_merge_documents",json!({"documentIds":["document-1","document-2"],"pages":[{"source":1,"page":1}]})));assert_eq!(merged["documentId"],"document-65");assert_eq!(merged["pageCount"],1);assert_eq!(fs::read_dir(output).unwrap().count(),0);s.stop();
}
