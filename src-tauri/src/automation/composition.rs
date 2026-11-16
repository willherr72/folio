//! Multi-document assembly through the existing graph-preserving native engine.
use super::{credentials::Credentials,invalid,io,original_snapshot::OriginalSnapshot,page_organization as pages,path_result,Source,text_preview::{copy_hash,read_bounded,MAX_BUNDLE}};
use crate::{EngineResult,ExportRequest,PagePlan,PdfEngine};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use std::{fs,path::{Path,PathBuf}};

#[derive(Clone,Debug,Deserialize,Serialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct CompositionPage{pub source:u8,pub page:u32,#[serde(default)]pub rotation:u16}
#[derive(Clone,Debug,Deserialize,Serialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct Composition{pub version:u8,pub pages:Vec<CompositionPage>}
impl Composition{
 pub fn validate(&self,source_count:usize)->EngineResult<()>{
  if self.version!=1||self.pages.is_empty()||self.pages.len()>10000||self.pages.iter().any(|p|p.source==0||p.source as usize>source_count||!(1..=10000).contains(&p.page)||!matches!(p.rotation,0|90|180|270)){
   return Err(invalid("Use version 1 with 1 to 10,000 pages, valid 1-based sources/pages and rotations of 0, 90, 180 or 270 degrees."));
  }Ok(())
 }
 fn read(path:&Path,count:usize)->EngineResult<Self>{
  let value:Self=serde_json::from_slice(&read_bounded(path,pages::MAX_REQUEST)?).map_err(|_|invalid("Composition JSON does not match the version 1 schema."))?;value.validate(count)?;Ok(value)
 }
}
pub(crate) struct ComposeRequest{pub inputs:Vec<PathBuf>,pub request_file:Option<PathBuf>,pub output:PathBuf}
struct OpenSource<'a>{identity:same_file::Handle,original:OriginalSnapshot,source:Source<'a>,credential_slot:usize}
pub(crate) fn execute(engine:&PdfEngine,request:ComposeRequest,credentials:&Credentials)->EngineResult<Value>{
 let count=request.inputs.len();
 if !(1..=64).contains(&count)||credentials.len()!=count{return Err(invalid("Supply 1 to 64 input slots and matching credentials."));}
 let organization=request.request_file.as_ref().map(|p|Composition::read(p,count)).transpose()?;
 let output=&request.output;
 if !output.extension().is_some_and(|e|e.eq_ignore_ascii_case("pdf")){return Err(invalid("Choose a new .pdf output filename."));}super::ensure_absent(output)?;
 let mut sources:Vec<OpenSource<'_>>=Vec::new();let mut slots=Vec::new();let mut remaining=MAX_BUNDLE;let mut total_pages=0usize;
 for(slot,path)in request.inputs.iter().enumerate(){
  let mut file=fs::File::open(path).map_err(io)?;
  let identity=same_file::Handle::from_file(file.try_clone().map_err(io)?).map_err(io)?;
  if let Some(index)=sources.iter().position(|s|s.identity==identity){
   if credentials.get(slot)!=credentials.get(sources[index].credential_slot){return Err(invalid("Aliased input slots must have identical credentials."));}
   slots.push(index);continue;
  }
  let original=OriginalSnapshot::capture_open_file(&mut file,remaining)?;remaining=remaining.checked_sub(original.bytes()).ok_or_else(||invalid("Composition inputs exceed 512 MiB."))?;
  let info=match credentials.get(slot){Some(p)=>engine.open_document_with_password(original.path(),p)?,None=>engine.open_document(original.path())?};
  let source=Source{engine,info};
  total_pages=total_pages.checked_add(source.info.pages.len()).filter(|n|*n<=10000).ok_or_else(||invalid("Composition inputs exceed 10,000 pages."))?;
  if source.info.pages.is_empty(){return Err(invalid("Each source PDF must have at least one page."));}
  slots.push(sources.len());sources.push(OpenSource{identity,original,source,credential_slot:slot});
 }
 let selection=if let Some(organization)=organization{organization}else{
  let mut selection=Vec::new();for(slot,index)in slots.iter().enumerate(){for page in 1..=sources[*index].source.info.pages.len(){
   if selection.len()>=10000{return Err(invalid("Composition output exceeds 10,000 pages."));}
   selection.push(CompositionPage{source:(slot+1)as u8,page:page as u32,rotation:0});
  }}Composition{version:1,pages:selection}
 };
 let mut plans=Vec::new();let mut protection_source=None;
 for(i,p)in selection.pages.iter().enumerate(){
  let source=&sources[slots[p.source as usize-1]].source.info;let page=source.pages.get(p.page as usize-1).ok_or_else(||invalid("A selected page is outside its source PDF."))?;
  if protection_source.is_none()&&source.protection.is_some(){protection_source=Some(p.source);}
  plans.push(PagePlan{id:format!("page-{}",i+1),source_id:source.id.clone(),page_index:p.page as usize-1,width:page.width,height:page.height,rotation:p.rotation,crop:None,overlays:page.overlays.clone()});
 }
 let first=slots[selection.pages[0].source as usize-1];
 let identity=selection.pages.len()==sources[first].source.info.pages.len()&&selection.pages.iter().enumerate().all(|(i,p)|slots[p.source as usize-1]==first&&p.page as usize==i+1&&p.rotation==0);
 let parent=output.parent().filter(|p|!p.as_os_str().is_empty()).unwrap_or(Path::new(".")).canonicalize().map_err(io)?;
 let output=parent.join(output.file_name().ok_or_else(||invalid("Choose a PDF output filename."))?);
 let stage=tempfile::Builder::new().prefix(".folio-compose-").tempdir_in(parent).map_err(io)?;let candidate=stage.path().join("candidate.pdf");
 let proof=if identity{sources[first].original.write_candidate(&candidate)?}else{
  engine.export_pdf(ExportRequest{pages:plans,flatten:false},&candidate)?;copy_hash(&candidate,MAX_BUNDLE,std::io::sink())?
 };
 if proof.bytes==0{return Err(invalid("The composed PDF is empty."));}
 if !identity{
  let credential=protection_source.and_then(|slot|credentials.get(slot as usize-1));
  let reopened=Source{engine,info:match credential{Some(p)=>engine.open_document_with_password(&candidate,p)?,None=>engine.open_document(&candidate)?}};
  if reopened.info.pages.len()!=selection.pages.len(){return Err(invalid("Composition changed the requested page count."));}
 }
 let result=path_result(json!({"bytes":proof.bytes,"sha256":proof.sha256,"pageCount":selection.pages.len(),"protectionSource":protection_source}),"path",&output);
 if serde_json::to_vec(&result).map_err(io)?.len()+1>pages::MAX_RESULT{return Err(invalid("Composition response exceeds its byte limit."));}
 pages::check_cli_response(&json!({"schemaVersion":1,"ok":true,"command":"compose-pages","result":result}))?;
 fs::OpenOptions::new().read(true).write(true).open(&candidate).and_then(|f|f.sync_all()).map_err(io)?;
 pages::publish_new(&candidate,&output)?;Ok(result)
}
