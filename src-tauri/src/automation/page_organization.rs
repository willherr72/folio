//! Source-owned page organization and exact exclusive PDF-copy publication.
use super::{invalid,io,path_result,Source,original_snapshot::OriginalSnapshot,text_preview::{read_bounded,copy_hash,MAX_BUNDLE}};
use crate::{DocumentInfo,EngineResult,ExportRequest,PagePlan,PdfEngine};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use std::{fs,path::Path};

pub(crate) const MAX_REQUEST:u64=512*1024;
pub(crate) const MAX_RESULT:usize=512*1024;
pub(crate) const MAX_CLI_RESPONSE:usize=1024*1024;
pub(crate) const MAX_MCP_RESPONSE:usize=2*1024*1024;

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct PageSelection{
 pub page:u32,
 #[serde(default)]pub rotation:u16,
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct PageOrganization{
 pub version:u8,
 pub pages:Vec<PageSelection>,
}
impl PageOrganization{
 pub fn validate(&self)->EngineResult<()>{
  if self.version!=1||self.pages.is_empty()||self.pages.len()>10000||self.pages.iter().any(|p|!(1..=10000).contains(&p.page)||!matches!(p.rotation,0|90|180|270)){
   return Err(invalid("Use version 1 with 1 to 10,000 pages, 1-based page numbers and rotations of 0, 90, 180 or 270 degrees."));
  }
  Ok(())
 }
 pub fn read(path:&Path)->EngineResult<Self>{
  let bytes=read_bounded(path,MAX_REQUEST)?;
  let request:Self=serde_json::from_slice(&bytes).map_err(|_|invalid("Page organization JSON does not match the version 1 schema."))?;
  request.validate()?;Ok(request)
 }
 pub fn identity(&self,count:usize)->bool{
  self.pages.len()==count&&self.pages.iter().enumerate().all(|(i,p)|p.page as usize==i+1&&p.rotation==0)
 }
}
fn within(value:&Value,limit:usize)->EngineResult<()>{
 let count=serde_json::to_vec(value).map_err(io)?.len();
 if count.checked_add(1).is_none_or(|n|n>limit){return Err(invalid("Page organization response exceeds its byte limit."));}Ok(())
}
pub(crate) fn check_cli_response(value:&Value)->EngineResult<()>{within(value,MAX_CLI_RESPONSE)}
pub(crate) fn check_mcp_response(value:&Value)->EngineResult<()>{within(value,MAX_MCP_RESPONSE)}

pub(super) fn export(engine:&PdfEngine,info:&DocumentInfo,organization:Option<&PageOrganization>,original:&OriginalSnapshot,output:&Path,password:Option<&str>)->EngineResult<Value>{
 if !output.extension().is_some_and(|e|e.eq_ignore_ascii_case("pdf")){return Err(invalid("Choose a new .pdf output filename."));}
 super::ensure_absent(output)?;
 if info.pages.is_empty()||info.pages.len()>10000{return Err(invalid("Choose a PDF with 1 to 10,000 pages."));}
 let selections=if let Some(request)=organization{request.validate()?;request.pages.clone()}else{(1..=info.pages.len()).map(|page|PageSelection{page:page as u32,rotation:0}).collect()};
 let pages:Vec<PagePlan>=selections.iter().enumerate().map(|(i,selected)|{
  let index=selected.page as usize-1;let source=info.pages.get(index).ok_or_else(||invalid("A selected page is outside this PDF."))?;
  Ok(PagePlan{id:format!("page-{}",i+1),source_id:info.id.clone(),page_index:index,width:source.width,height:source.height,rotation:selected.rotation,crop:None,overlays:source.overlays.clone()})
 }).collect::<EngineResult<_>>()?;
 let count=pages.len();
 let parent=output.parent().filter(|p|!p.as_os_str().is_empty()).unwrap_or(Path::new(".")).canonicalize().map_err(io)?;
 let output=parent.join(output.file_name().ok_or_else(||invalid("Choose a PDF output filename."))?);
 let stage=tempfile::Builder::new().prefix(".folio-pages-").tempdir_in(&parent).map_err(io)?;
 let candidate=stage.path().join("candidate.pdf");
 let proof=if organization.is_none_or(|o|o.identity(info.pages.len())){original.write_candidate(&candidate)?}else{
  engine.export_pdf(ExportRequest{pages,flatten:false},&candidate)?;
  copy_hash(&candidate,MAX_BUNDLE,std::io::sink())?
 };
 if proof.bytes==0{return Err(invalid("The exported PDF is empty."));}
 // The native export path and exact-copy path have both completed before any
 // public name exists. Reopen through native parsing before committing a change.
 if organization.is_some_and(|o|!o.identity(info.pages.len())){
  let reopened=Source{engine,info:match password{Some(password)=>engine.open_document_with_password(&candidate,password)?,None=>engine.open_document(&candidate)?}};
  if reopened.info.pages.len()!=count{return Err(invalid("Page export changed the requested page count."));}
 }
 let result=path_result(json!({"bytes":proof.bytes,"sha256":proof.sha256,"pageCount":count,"protected":info.protection.is_some()}),"path",&output);
 within(&result,MAX_RESULT)?;
 check_cli_response(&json!({"schemaVersion":1,"ok":true,"command":"export-pages","result":result}))?;
 fs::OpenOptions::new().read(true).write(true).open(&candidate).and_then(|f|f.sync_all()).map_err(io)?;
 publish_new(&candidate,&output)?;Ok(result)
}
pub(super) fn publish_new(candidate:&Path,output:&Path)->EngineResult<()>{
 tempfile::TempPath::try_from_path(candidate).map_err(io)?.persist_noclobber(output).map_err(|e|{
  if e.error.kind()==std::io::ErrorKind::AlreadyExists{invalid("The output already exists. Choose a new filename.")}else{io(&e.error)}
 })?;Ok(())
}

#[cfg(test)]mod page_organization_tests{
 use super::*;
 #[test]fn page_organization_late_publication_target_is_untouched(){
  let dir=tempfile::tempdir().unwrap();let output=dir.path().join("out.pdf");super::super::ensure_absent(&output).unwrap();
  let candidate=dir.path().join("candidate.pdf");fs::write(&candidate,b"new bytes").unwrap();fs::write(&output,b"late owner").unwrap();
  assert!(publish_new(&candidate,&output).is_err());assert_eq!(fs::read(output).unwrap(),b"late owner");
 }
 #[test]fn page_organization_exact_cli_and_mcp_wire_boundaries(){
  for(cap,check)in[(MAX_CLI_RESPONSE,check_cli_response as fn(&Value)->EngineResult<()>),(MAX_MCP_RESPONSE,check_mcp_response as fn(&Value)->EngineResult<()>)]{
   let base=json!({"payload":""});let overhead=serde_json::to_vec(&base).unwrap().len()+1;
   check(&json!({"payload":"x".repeat(cap-overhead)})).unwrap();
   assert!(check(&json!({"payload":"x".repeat(cap-overhead+1)})).is_err());
   assert!(check(&json!({"payload":"\"\\".repeat(cap/2)})).is_err());
  }
 }
}
