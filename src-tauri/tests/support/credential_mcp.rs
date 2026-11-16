//! Real-process authentication and private preview capability lifetime tests.
use super::*;
#[path="protected_fixture.rs"]mod protected_fixture;
use std::fs;

#[test]fn credentials_follow_protected_preview_after_original_closes(){
 let dir=tempfile::tempdir().unwrap();let source=dir.path().join("protected.pdf");
 let original=protected_fixture::encrypt(&automation_forms::fixture(),lopdf::encryption::Permissions::ASSEMBLABLE|lopdf::encryption::Permissions::FILLABLE|lopdf::encryption::Permissions::COPYABLE|lopdf::encryption::Permissions::MODIFIABLE|lopdf::encryption::Permissions::ANNOTABLE);
 fs::write(&source,&original).unwrap();let output=dir.path().join("out");fs::create_dir(&output).unwrap();let credentials=dir.path().join("credentials.json");fs::write(&credentials,br#"["reader"]"#).unwrap();
 let mut s=Session::start_with_credentials(&[&source],&output,Some(&credentials));let mut replies=Vec::new();
 let first=success(call(&mut s,1,"folio_inspect_forms",json!({"documentId":"document-1","page":1})));replies.push(first.clone());let field=&first["fields"][0];
 let preview=success(call(&mut s,2,"folio_preview_form",json!({"documentId":"document-1","page":1,"fieldId":field["fieldId"],"value":{"kind":"text","text":"MCP answer"}})));replies.push(preview.clone());
 success(call(&mut s,3,"folio_close_document",json!({"documentId":"document-1"})));
 let applied=success(call(&mut s,4,"folio_apply_preview",json!({"previewId":preview["previewId"]})));replies.push(applied.clone());
 let next=success(call(&mut s,5,"folio_inspect_forms",json!({"documentId":applied["documentId"],"page":2})));assert_eq!(next["fields"][0]["value"]["text"],"MCP answer");replies.push(next.clone());
 let revised=success(call(&mut s,6,"folio_preview_form",json!({"documentId":applied["documentId"],"page":2,"fieldId":next["fields"][0]["fieldId"],"value":{"kind":"text","text":"Second answer"}})));replies.push(revised.clone());
 let saved=success(call(&mut s,7,"folio_save_preview",json!({"previewId":revised["previewId"]})));replies.push(saved.clone());
 let exported=success(call(&mut s,8,"folio_export_pages",json!({"documentId":applied["documentId"]})));replies.push(exported.clone());
 let listed=success(call(&mut s,9,"folio_list_documents",json!({})));replies.push(listed);
 assert!(!serde_json::to_string(&replies).unwrap().contains("reader"));
 success(call(&mut s,10,"folio_close_document",json!({"documentId":applied["documentId"]})));s.stop();
 let engine=folio_engine::PdfEngine::start(Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
 for(result,answer)in[(&saved,"Second answer"),(&exported,"MCP answer")]{let path=Path::new(result["path"].as_str().unwrap());assert!(path.exists());assert!(engine.open_document(path).is_err());let reopened=engine.open_document_with_password(path,"reader").unwrap();assert_eq!(reopened.pages.len(),3);assert!(serde_json::to_string(&engine.list_form_fields(&reopened.id,1).unwrap()).unwrap().contains(answer));assert!(!fs::read(path).unwrap().windows(6).any(|v|v==b"reader"));}
 assert_eq!(fs::read(source).unwrap(),original);assert_eq!(fs::read(credentials).unwrap(),br#"["reader"]"#);
}

#[test]fn credentials_launch_validation_never_echoes_secret_values(){
 let dir=tempfile::tempdir().unwrap();let source=dir.path().join("source.pdf");pdf(&source);let output=dir.path().join("out");fs::create_dir(&output).unwrap();let file=dir.path().join("credentials.json");
 for value in[br#"{"secret":"PRIVATE_AUTH_SENTINEL"}"#.as_slice(),br#"["PRIVATE_AUTH_SENTINEL",null]"#,br#"["PRIVATE_AUTH_SENTINEL\n"]"#]{
  fs::write(&file,value).unwrap();let r=Command::new(env!("CARGO_BIN_EXE_folio-cli")).arg("mcp").arg("--input").arg(&source).arg("--out-dir").arg(&output).arg("--credentials").arg(&file).stdin(Stdio::null()).output().unwrap();assert_eq!(r.status.code(),Some(2));assert!(r.stdout.is_empty());assert!(!String::from_utf8_lossy(&r.stderr).contains("PRIVATE_AUTH_SENTINEL"));
 }
 assert_eq!(fs::read_dir(output).unwrap().count(),0);
}

#[test]fn wrong_launch_credential_keeps_protocol_alive_without_export(){
 let dir=tempfile::tempdir().unwrap();let source=dir.path().join("protected.pdf");let original=protected_fixture::encrypt(&automation_forms::fixture(),lopdf::encryption::Permissions::ASSEMBLABLE);fs::write(&source,&original).unwrap();let output=dir.path().join("out");fs::create_dir(&output).unwrap();let file=dir.path().join("credentials.json");fs::write(&file,br#"["WRONG_PRIVATE_CREDENTIAL"]"#).unwrap();
 let mut s=Session::start_with_credentials(&[&source],&output,Some(&file));let reply=call(&mut s,1,"folio_info",json!({"documentId":"document-1"}));assert_eq!(reply["result"]["isError"],true);assert_eq!(reply["result"]["structuredContent"]["error"]["code"],"incorrect_password");assert!(!reply.to_string().contains("WRONG_PRIVATE_CREDENTIAL"));
 assert_eq!(success(call(&mut s,2,"folio_list_documents",json!({})))["documents"].as_array().unwrap().len(),1);s.stop();assert_eq!(fs::read_dir(output).unwrap().count(),0);assert_eq!(fs::read(source).unwrap(),original);
}
