//! Included as a child of the existing real-process MCP integration tests.
use super::*;
#[path="page_organization.rs"]mod fixture;
use sha2::{Digest,Sha256};
fn digest(path:&Path)->String{Sha256::digest(std::fs::read(path).unwrap()).iter().map(|b|format!("{b:02x}")).collect()}
fn invoke(s:&mut Session,id:u32,legacy:bool,name:&str,args:Value)->Value{
 if legacy{s.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}}));let r=s.receive();assert_eq!(r["id"],id);r}else{call(s,id,name,args)}
}
fn capture(name:&str,source:&Path,outputs:&[(String,Value)]){
 if let Some(root)=std::env::var_os("FOLIO_PAGE_ORGANIZATION_OUTPUTS"){
  let dir=std::path::PathBuf::from(root).join(name);std::fs::create_dir_all(&dir).unwrap();std::fs::copy(source,dir.join("source.pdf")).unwrap();
  let mut records=Vec::new();for(i,(path,selection))in outputs.iter().enumerate(){let target=dir.join(format!("output-{i}.pdf"));assert!(!target.exists());std::fs::copy(path,&target).unwrap();records.push(json!({"file":target.file_name().unwrap().to_str().unwrap(),"sha256":digest(&target),"selection":selection}));}
  std::fs::write(dir.join("outputs.json"),serde_json::to_vec(&records).unwrap()).unwrap();
 }
}
#[test]fn page_organization_both_protocols_chain_private_documents_edit_export_and_close(){
 for legacy in[false,true]{
  let dir=tempfile::tempdir().unwrap();let source=dir.path().join("source.pdf");let original=fixture::write(&source);let output=dir.path().join("out");std::fs::create_dir(&output).unwrap();let mut s=Session::start(&source,&output);
  if legacy{s.send(json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"page-test","version":"1"}}}));assert_eq!(s.receive()["result"]["protocolVersion"],"2025-11-25");s.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));}
  let original_info=success(invoke(&mut s,1,legacy,"folio_info",json!({"documentId":"document-1"})));
  let selection=json!([{"page":2,"rotation":90},{"page":1},{"page":2,"rotation":270},{"page":3,"rotation":180}]);
  let organized=success(invoke(&mut s,2,legacy,"folio_organize_pages",json!({"documentId":"document-1","pages":selection})));assert_eq!(organized["documentId"],"document-2");assert_eq!(organized["pageCount"],4);assert!(organized.get("path").is_none());assert_eq!(std::fs::read_dir(&output).unwrap().count(),0);
  let organized_info=success(invoke(&mut s,3,legacy,"folio_info",json!({"documentId":organized["documentId"]})));assert_eq!(organized_info["pageCount"],4);assert_eq!(organized_info["pages"][0]["width"],original_info["pages"][1]["height"]);assert_eq!(organized_info["pages"][0]["height"],original_info["pages"][1]["width"]);
  let saved=success(invoke(&mut s,4,legacy,"folio_export_pages",json!({"documentId":organized["documentId"]})));assert_eq!(saved["sha256"],organized["sha256"]);let saved_path=saved["path"].as_str().unwrap().to_owned();assert_eq!(digest(Path::new(&saved_path)),organized["sha256"]);
  let repeated=success(invoke(&mut s,5,legacy,"folio_export_pages",json!({"documentId":organized["documentId"]})));assert_ne!(saved["path"],repeated["path"]);assert_eq!(saved["sha256"],repeated["sha256"]);
  let inspected=success(invoke(&mut s,6,legacy,"folio_inspect_text",json!({"documentId":organized["documentId"],"page":4})));let target=inspected["runs"].as_array().unwrap().iter().find(|r|r["text"]=="Page three content").unwrap();assert_eq!(target["supported"],true);
  let preview=success(invoke(&mut s,7,legacy,"folio_preview_text",json!({"documentId":organized["documentId"],"page":4,"selectionId":target["selectionId"],"replacement":"Page three contents"})));
  let edited=success(invoke(&mut s,8,legacy,"folio_apply_preview",json!({"previewId":preview["previewId"]})));assert_eq!(edited["documentId"],"document-3");
  let final_saved=success(invoke(&mut s,9,legacy,"folio_export_pages",json!({"documentId":edited["documentId"]})));assert_eq!(final_saved["sha256"],edited["sha256"]);let final_path=final_saved["path"].as_str().unwrap().to_owned();
  assert_eq!(success(invoke(&mut s,10,legacy,"folio_info",json!({"documentId":"document-1"}))),original_info);
  success(invoke(&mut s,11,legacy,"folio_close_document",json!({"documentId":"document-2"})));assert!(Path::new(&saved_path).exists());assert_eq!(digest(Path::new(&saved_path)),saved["sha256"]);
  let unknown=invoke(&mut s,12,legacy,"folio_info",json!({"documentId":"document-2"}));assert_eq!(unknown["result"]["isError"],true);
  let final_text=success(invoke(&mut s,13,legacy,"folio_inspect_text",json!({"documentId":"document-3","page":4})));assert!(final_text["runs"].as_array().unwrap().iter().any(|r|r["text"]=="Page three contents"));
  capture(&format!("mcp-chain-{legacy}"),&source,&[(saved_path.clone(),json!({"pages":selection})),(final_path.clone(),json!({"pages":selection,"replacementPage":4,"replacement":"Page three contents"}))]);
  s.stop();assert!(Path::new(&saved_path).exists());assert!(Path::new(&final_path).exists());assert_eq!(std::fs::read(&source).unwrap(),original);
 }
}

#[test]fn page_organization_schema_hints_and_invalid_arguments_preserve_session(){
 let dir=tempfile::tempdir().unwrap();let source=dir.path().join("source.pdf");fixture::write(&source);let output=dir.path().join("out");std::fs::create_dir(&output).unwrap();let mut s=Session::start(&source,&output);
 let listed=s.request(1,"tools/list",json!({}));
 for name in["folio_organize_pages","folio_export_pages"]{
  let tool=listed["result"]["tools"].as_array().unwrap().iter().find(|t|t["name"]==name).unwrap();for hint in["readOnlyHint","destructiveHint","idempotentHint","openWorldHint"]{assert_eq!(tool["annotations"][hint],false);}
  assert_eq!(tool["inputSchema"]["additionalProperties"],false);assert_eq!(tool["inputSchema"]["properties"]["pages"]["maxItems"],10000);
  for(args,id)in[(json!({"documentId":"missing"}),2),(json!({"documentId":"document-1","path":"outside.pdf"}),3),(json!({"documentId":"document-1","pages":null}),4),(json!({"documentId":"document-1","pages":[]}),5),(json!({"documentId":"document-1","pages":[{"page":0}]}),6),(json!({"documentId":"document-1","pages":[{"page":1,"rotation":45}]}),7),(json!({"documentId":"document-1","pages":[{"page":1,"sourceId":"injected"}]}),8)]{let reply=call(&mut s,id,name,args);assert_eq!(reply["result"]["isError"],true,"{reply}");}
 }
 let result=success(call(&mut s,20,"folio_list_documents",json!({})));assert_eq!(result["documents"].as_array().unwrap().len(),1);assert_eq!(std::fs::read_dir(output).unwrap().count(),0);s.stop();
}

#[test]fn page_organization_full_session_recovers_capacity_and_keeps_monotonic_ids(){
 let dir=tempfile::tempdir().unwrap();let source=dir.path().join("source.pdf");let original=fixture::write(&source);let output=dir.path().join("out");std::fs::create_dir(&output).unwrap();let sources=vec![source.as_path();64];let mut s=Session::start_many(&sources,&output);
 let denied=call(&mut s,1,"folio_organize_pages",json!({"documentId":"document-1","pages":[{"page":1}]}));assert_eq!(denied["result"]["isError"],true);assert_eq!(denied["result"]["structuredContent"]["error"]["code"],"document_limit");
 success(call(&mut s,2,"folio_close_document",json!({"documentId":"document-64"})));
 let result=success(call(&mut s,3,"folio_organize_pages",json!({"documentId":"document-1","pages":[{"page":1}]})));assert_eq!(result["documentId"],"document-65");assert_eq!(result["pageCount"],1);
 let info=success(call(&mut s,4,"folio_info",json!({"documentId":"document-65"})));assert_eq!(info["pageCount"],1);assert_eq!(std::fs::read_dir(output).unwrap().count(),0);s.stop();assert_eq!(std::fs::read(source).unwrap(),original);
}
