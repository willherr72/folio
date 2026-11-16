use folio_engine::{PdfEngine,PagePlan};
use serde_json::{json,Value};
use std::{fs,io::Write,path::{Path,PathBuf},process::{Command,Output,Stdio}};
#[path="support/page_organization.rs"]mod fixture;
#[path="support/protected_fixture.rs"]mod protected_fixture;

fn engine_path()->PathBuf{Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")}
fn invoke(input:&Path,output:&Path,request:Option<&Path>,password:Option<&[u8]>)->Output{
 let mut cmd=Command::new(env!("CARGO_BIN_EXE_folio-cli"));cmd.arg("export-pages").arg(input).arg("--out").arg(output).arg("--engine").arg(engine_path());
 if let Some(request)=request{cmd.arg("--request").arg(request);}if password.is_some(){cmd.arg("--password-stdin");}
 let mut child=cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
 if let Some(password)=password{child.stdin.as_mut().unwrap().write_all(password).unwrap();}drop(child.stdin.take());child.wait_with_output().unwrap()
}
fn success(out:Output)->Value{
 assert!(out.status.success(),"{}",String::from_utf8_lossy(&out.stderr));assert!(out.stderr.is_empty());
 let value:Value=serde_json::from_slice(&out.stdout).unwrap();assert_eq!(value["ok"],true);assert_eq!(value["command"],"export-pages");value["result"].clone()
}
fn rejected(out:Output,code:&str){assert!(!out.status.success());assert!(out.stdout.is_empty());let value:Value=serde_json::from_slice(&out.stderr).unwrap();assert_eq!(value["error"]["code"],code,"{value}");}
fn request(dir:&Path,value:Value)->PathBuf{let path=dir.join("pages.json");fs::write(&path,serde_json::to_vec(&value).unwrap()).unwrap();path}
fn plans(info:&folio_engine::DocumentInfo)->Vec<PagePlan>{info.pages.iter().enumerate().map(|(i,p)|PagePlan{id:format!("p{i}"),source_id:info.id.clone(),page_index:i,width:p.width,height:p.height,rotation:0,crop:None,overlays:p.overlays.clone()}).collect()}
fn capture(name:&str,source:&Path,output:&Path,selection:Value){
 if let Some(root)=std::env::var_os("FOLIO_PAGE_ORGANIZATION_OUTPUTS"){
  let dir=PathBuf::from(root).join(name);fs::create_dir_all(&dir).unwrap();
  for(name,path)in[("source.pdf",source),("output.pdf",output)]{let copy=dir.join(name);assert!(!copy.exists());fs::copy(path,copy).unwrap();}
  fs::write(dir.join("selection.json"),serde_json::to_vec(&selection).unwrap()).unwrap();
 }
}

#[test]fn page_organization_identity_preserves_original_bytes_and_reports_complete_hash(){
 use sha2::{Digest,Sha256};
 let dir=tempfile::tempdir().unwrap();let input=dir.path().join("source.pdf");let original=fixture::write(&input);
 for explicit in[false,true]{
  let selection=json!({"version":1,"pages":[{"page":1},{"page":2,"rotation":0},{"page":3}]});
  let req=explicit.then(||request(dir.path(),selection.clone()));let output=dir.path().join(format!("identity-{explicit}.pdf"));let result=success(invoke(&input,&output,req.as_deref(),None));
  assert_eq!(fs::read(&output).unwrap(),original);assert_eq!(result["bytes"],original.len());assert_eq!(result["pageCount"],3);
  assert_eq!(result["sha256"],Sha256::digest(&original).iter().map(|b|format!("{b:02x}")).collect::<String>());
  capture(&format!("identity-{explicit}"),&input,&output,selection);
 }
 assert_eq!(fs::read(input).unwrap(),original);
}

#[test]fn page_organization_order_duplicates_rotations_and_native_rendering_agree(){
 let dir=tempfile::tempdir().unwrap();let input=dir.path().join("source.pdf");let original=fixture::write(&input);let output=dir.path().join("organized.pdf");
 let selection=json!({"version":1,"pages":[{"page":2,"rotation":90},{"page":1},{"page":2,"rotation":270},{"page":3,"rotation":180}]});let req=request(dir.path(),selection.clone());
 let result=success(invoke(&input,&output,Some(&req),None));assert_eq!(result["pageCount"],4);
 let engine=PdfEngine::start(engine_path()).unwrap();let source=engine.open_document(&input).unwrap();let saved=engine.open_document(&output).unwrap();assert_eq!(saved.pages.len(),4);
 let source_plans=plans(&source);let output_plans=plans(&saved);
 for(i,(page,rotation))in[(1,90),(0,0),(1,270),(2,180)].into_iter().enumerate(){
  let mut expected=source_plans[page].clone();expected.rotation=rotation;
  let before=engine.render_page_plan(expected,480).unwrap();let after=engine.render_page_plan(output_plans[i].clone(),480).unwrap();
  assert!(image::load_from_memory(&before).unwrap().to_rgba8()==image::load_from_memory(&after).unwrap().to_rgba8(),"Page {i} pixels changed");
  assert_eq!(engine.extract_text(&source.id,page).unwrap(),engine.extract_text(&saved.id,i).unwrap());
  if page==1{let fields=engine.list_form_fields(&saved.id,i).unwrap();assert_eq!(fields.len(),1);assert!(serde_json::to_string(&fields).unwrap().contains("Retained answer"));}
 }
 let pdf=lopdf::Document::load(&output).unwrap();let ids:Vec<_>=pdf.get_pages().into_values().collect();
 for index in[0,2]{let page=pdf.get_dictionary(ids[index]).unwrap();let annots=page.get(b"Annots").unwrap().as_array().unwrap();let link=annots.iter().map(|a|pdf.get_dictionary(a.as_reference().unwrap()).unwrap()).find(|a|a.get(b"Subtype").unwrap().as_name().unwrap()==b"Link").unwrap();assert_eq!(link.get(b"Dest").unwrap().as_array().unwrap()[0].as_reference().unwrap(),ids[index],"Copied self-link must target its own occurrence");}
 let root=pdf.get_dictionary(pdf.trailer.get(b"Root").unwrap().as_reference().unwrap()).unwrap();assert!(root.has(b"StructTreeRoot"));assert!(root.has(b"Outlines"));assert!(root.has(b"AcroForm"));
 assert_eq!(fs::read(&input).unwrap(),original);capture("ordered-rotated",&input,&output,selection);
}

#[test]fn page_organization_invalid_requests_never_publish_or_modify_sources(){
 let dir=tempfile::tempdir().unwrap();let input=dir.path().join("source.pdf");let original=fixture::write(&input);let output=dir.path().join("output.pdf");
 for value in[json!({}),json!({"version":2,"pages":[{"page":1}]}),json!({"version":1,"pages":[]}),json!({"version":1,"pages":null}),json!({"version":1,"pages":[null]}),json!({"version":1,"pages":[{"page":0}]}),json!({"version":1,"pages":[{"page":4}]}),json!({"version":1,"pages":[{"page":1.5}]}),json!({"version":1,"pages":[{"page":1,"rotation":45}]}),json!({"version":1,"pages":[{"page":1,"width":30}]}),json!({"version":1,"pages":[{"page":1}],"input":"unapproved.pdf"}),json!({"version":1,"pages":vec![json!({"page":1});10001]})]{
  let req=request(dir.path(),value);rejected(invoke(&input,&output,Some(&req),None),"invalid_request");assert!(!output.exists());assert_eq!(fs::read(&input).unwrap(),original);
 }
 let req=dir.path().join("oversized.json");fs::write(&req,vec![b' ';512*1024+1]).unwrap();rejected(invoke(&input,&output,Some(&req),None),"invalid_request");assert!(!output.exists());
 let upper=dir.path().join("not-pdf.txt");rejected(invoke(&input,&upper,None,None),"invalid_request");assert!(!upper.exists());
}

#[test]fn page_organization_existing_and_hardlinked_destinations_are_preserved(){
 let dir=tempfile::tempdir().unwrap();let input=dir.path().join("source.pdf");let original=fixture::write(&input);let output=dir.path().join("existing.pdf");fs::write(&output,b"existing output").unwrap();
 rejected(invoke(&input,&output,None,None),"invalid_request");assert_eq!(fs::read(output).unwrap(),b"existing output");
 let alias=dir.path().join("alias.pdf");fs::hard_link(&input,&alias).unwrap();rejected(invoke(&input,&alias,None,None),"invalid_request");assert_eq!(fs::read(input).unwrap(),original);
}

#[test]fn page_organization_protected_identity_and_assembly_preserve_security(){
 use lopdf::encryption::Permissions;
 let dir=tempfile::tempdir().unwrap();let plain=dir.path().join("plain.pdf");let bytes=fixture::write(&plain);
 let engine=PdfEngine::start(engine_path()).unwrap();
 for allowed in[false,true]{
  let input=dir.path().join(format!("protected-{allowed}.pdf"));let permissions=if allowed{Permissions::ASSEMBLABLE}else{Permissions::empty()};let original=protected_fixture::encrypt(&bytes,permissions);fs::write(&input,&original).unwrap();
  let identity=dir.path().join(format!("identity-{allowed}.pdf"));success(invoke(&input,&identity,None,Some(b"reader\n")));assert_eq!(fs::read(identity).unwrap(),original);
  let req=request(dir.path(),json!({"version":1,"pages":[{"page":2,"rotation":90},{"page":1}]}));let output=dir.path().join(format!("changed-{allowed}.pdf"));
  let result=invoke(&input,&output,Some(&req),Some(b"reader\n"));
  if allowed{success(result);assert!(engine.open_document(&output).is_err());let saved=engine.open_document_with_password(&output,"reader").unwrap();let protection=saved.protection.unwrap();assert!(protection.permissions.assemble);assert!(!protection.permissions.copy_content);capture("protected-assembly",&input,&output,json!({"version":1,"pages":[{"page":2,"rotation":90},{"page":1}]}));}
  else{rejected(result,"permission_denied");assert!(!output.exists());}assert_eq!(fs::read(input).unwrap(),original);
 }
}

#[cfg(windows)]#[test]fn page_organization_non_unicode_destination_has_lossless_native_path(){
 use std::{ffi::OsString,os::windows::ffi::{OsStringExt,OsStrExt}};
 let dir=tempfile::tempdir().unwrap();let input=dir.path().join("source.pdf");let original=fixture::write(&input);let output=dir.path().join(OsString::from_wide(&[0xd800,46,112,100,102]));
 let result=success(invoke(&input,&output,None,None));assert_eq!(result["path"],Value::Null);assert_eq!(result["pathNative"]["encoding"],"windows-utf16");
 let units:Vec<u16>=serde_json::from_value(result["pathNative"]["units"].clone()).unwrap();assert_eq!(units,output.canonicalize().unwrap().as_os_str().encode_wide().collect::<Vec<_>>());assert_eq!(fs::read(output).unwrap(),original);
}

#[test]fn page_organization_saved_folio_overlays_remain_editable_and_render_once(){
 let dir=tempfile::tempdir().unwrap();let input=dir.path().join("original.pdf");fixture::write(&input);let engine=PdfEngine::start(engine_path()).unwrap();
 let info=engine.open_document(&input).unwrap();let mut source_plans=plans(&info);
 source_plans[0].overlays.push(serde_json::from_value(json!({"type":"text","id":"saved-overlay","x":30,"y":110,"text":"Saved overlay","fontSize":14,"color":"#0000ff"})).unwrap());
 let saved_source=dir.path().join("folio-source.pdf");engine.export_pdf(folio_engine::ExportRequest{pages:source_plans,flatten:false},&saved_source).unwrap();let original=fs::read(&saved_source).unwrap();
 let source=engine.open_document(&saved_source).unwrap();assert_eq!(source.pages[0].overlays.len(),1);
 let selection=json!({"version":1,"pages":[{"page":1},{"page":1,"rotation":180}]});let req=request(dir.path(),selection.clone());let output=dir.path().join("organized.pdf");success(invoke(&saved_source,&output,Some(&req),None));
 let saved=engine.open_document(&output).unwrap();assert_eq!(saved.pages.len(),2);
 let before=plans(&source);let after=plans(&saved);
 for(i,rotation)in[0,180].into_iter().enumerate(){
  assert_eq!(saved.pages[i].overlays.len(),1);assert_eq!(serde_json::to_value(&saved.pages[i].overlays).unwrap()[0]["text"],"Saved overlay");
  let mut expected=before[0].clone();expected.rotation=rotation;
  let a=engine.render_page_plan(expected,480).unwrap();let b=engine.render_page_plan(after[i].clone(),480).unwrap();
  assert!(image::load_from_memory(&a).unwrap().to_rgba8()==image::load_from_memory(&b).unwrap().to_rgba8(),"Overlay pixels differ or repeat on page {i}");
 }
 assert_eq!(fs::read(&saved_source).unwrap(),original);capture("folio-overlays",&saved_source,&output,selection);
}
