use folio_engine::{PdfEngine,PagePlan};
use serde_json::{json,Value};
use std::{fs,io::Write,path::{Path,PathBuf},process::{Command,Output,Stdio}};
#[path="support/page_organization.rs"]mod fixture;
#[path="support/protected_fixture.rs"]mod protected_fixture;

fn engine_path()->PathBuf{Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")}
fn invoke(inputs:&[&Path],output:&Path,request:Option<&Path>,credentials:Option<&[u8]>)->Output{
 let mut cmd=Command::new(env!("CARGO_BIN_EXE_folio-cli"));cmd.arg("compose-pages");
 for input in inputs{cmd.arg("--input").arg(input);}cmd.arg("--out").arg(output).arg("--engine").arg(engine_path());
 if let Some(request)=request{cmd.arg("--request").arg(request);}if credentials.is_some(){cmd.arg("--passwords-stdin");}
 let mut child=cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
 if let Some(bytes)=credentials{child.stdin.as_mut().unwrap().write_all(bytes).unwrap();}drop(child.stdin.take());child.wait_with_output().unwrap()
}
fn success(output:Output)->Value{
 assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));assert!(output.stderr.is_empty());
 let value:Value=serde_json::from_slice(&output.stdout).unwrap();assert_eq!(value["ok"],true);assert_eq!(value["command"],"compose-pages");value["result"].clone()
}
fn rejected(output:Output,code:&str){assert!(!output.status.success());assert!(output.stdout.is_empty());let value:Value=serde_json::from_slice(&output.stderr).unwrap();assert_eq!(value["error"]["code"],code,"{value}");}
fn request(dir:&Path,value:Value)->PathBuf{let p=dir.join("composition.json");fs::write(&p,serde_json::to_vec(&value).unwrap()).unwrap();p}
fn second(path:&Path)->Vec<u8>{
 let bytes=fixture::write(path);let mut d=lopdf::Document::load_mem(&bytes).unwrap();
 for o in d.objects.values_mut(){match o{
  lopdf::Object::Stream(s)=>{if let Ok(t)=std::str::from_utf8(&s.content){let changed=t.replace("Page ","Other ").replace("Retained answer","Second answer");s.set_content(changed.into_bytes());}},
  lopdf::Object::Dictionary(v)=>{if v.has(b"FT"){v.set("V",lopdf::Object::string_literal("Second answer"));}},_=>{}
 }}
 let info=d.trailer.get(b"Info").unwrap().as_reference().unwrap();d.get_dictionary_mut(info).unwrap().set("Title",lopdf::Object::string_literal("Source B title"));
 let mut bytes=Vec::new();d.save_to(&mut bytes).unwrap();fs::write(path,&bytes).unwrap();bytes
}
fn plan(info:&folio_engine::DocumentInfo,index:usize,rotation:u16)->PagePlan{let p=&info.pages[index];PagePlan{id:format!("p{index}"),source_id:info.id.clone(),page_index:index,width:p.width,height:p.height,rotation,crop:None,overlays:p.overlays.clone()}}
fn capture(name:&str,inputs:&[&Path],output:&Path,selection:Option<Value>){
 if let Some(root)=std::env::var_os("FOLIO_COMPOSITION_AUTOMATION_OUTPUTS"){
  let dir=PathBuf::from(root).join(name);fs::create_dir_all(&dir).unwrap();for(i,path)in inputs.iter().enumerate(){fs::copy(path,dir.join(format!("source-{}.pdf",i+1))).unwrap();}
  fs::copy(output,dir.join("output.pdf")).unwrap();fs::write(dir.join("selection.json"),serde_json::to_vec(&selection).unwrap()).unwrap();
 }
}

#[test]fn composition_concatenates_independent_sources_and_names_colliding_forms(){
 let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a.pdf");let b=dir.path().join("b.pdf");let original_a=fixture::write(&a);let original_b=second(&b);let output=dir.path().join("combined.pdf");
 let result=success(invoke(&[&a,&b],&output,None,None));assert_eq!(result["pageCount"],6);assert!(result["protectionSource"].is_null());
 let engine=PdfEngine::start(engine_path()).unwrap();let saved=engine.open_document(&output).unwrap();assert_eq!(saved.pages.len(),6);
 for i in 0..6{let text=engine.extract_text(&saved.id,i).unwrap();assert!(text.contains(if i<3{fixture::LABELS[i]}else{["Other one content","Other two content","Other three content"][i-3]}),"{text}");}
 let first=engine.list_form_fields(&saved.id,1).unwrap();let second=engine.list_form_fields(&saved.id,4).unwrap();assert_eq!(first.len(),1);assert_eq!(second.len(),1);assert_ne!(first[0].name,second[0].name);
 assert!(serde_json::to_string(&first).unwrap().contains("Retained answer"));assert!(serde_json::to_string(&second).unwrap().contains("Second answer"));
 assert_eq!(fs::read(&a).unwrap(),original_a);assert_eq!(fs::read(&b).unwrap(),original_b);capture("concatenated",&[&a,&b],&output,None);
}

#[test]fn composition_interleaves_rotates_and_duplicates_without_changing_pixels(){
 let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a.pdf");let b=dir.path().join("b.pdf");fixture::write(&a);second(&b);
 let selection=json!({"version":1,"pages":[{"source":2,"page":2,"rotation":90},{"source":1,"page":1},{"source":2,"page":2,"rotation":270},{"source":1,"page":3}]});let req=request(dir.path(),selection.clone());let output=dir.path().join("interleaved.pdf");
 assert_eq!(success(invoke(&[&a,&b],&output,Some(&req),None))["pageCount"],4);
 let engine=PdfEngine::start(engine_path()).unwrap();let aa=engine.open_document(&a).unwrap();let bb=engine.open_document(&b).unwrap();let saved=engine.open_document(&output).unwrap();
 for(i,(source,page,rotation))in[(&bb,1,90),(&aa,0,0),(&bb,1,270),(&aa,2,0)].into_iter().enumerate(){
  assert_eq!(engine.extract_text(&source.id,page).unwrap(),engine.extract_text(&saved.id,i).unwrap());
  let expected=engine.render_page_plan(plan(source,page,rotation),480).unwrap();let actual=engine.render_page_plan(plan(&saved,i,0),480).unwrap();
  assert!(image::load_from_memory(&expected).unwrap().to_rgba8()==image::load_from_memory(&actual).unwrap().to_rgba8(),"Page {i} pixels changed");
 }
 let first=engine.list_form_fields(&saved.id,0).unwrap();let repeat=engine.list_form_fields(&saved.id,2).unwrap();assert_eq!(first[0].name,repeat[0].name);capture("interleaved",&[&a,&b],&output,Some(selection));
}

#[test]fn composition_aliases_share_fields_but_identical_independent_files_do_not(){
 let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a.pdf");let bytes=fixture::write(&a);let alias=dir.path().join("alias.pdf");fs::hard_link(&a,&alias).unwrap();let copy=dir.path().join("copy.pdf");fs::write(&copy,&bytes).unwrap();
 let engine=PdfEngine::start(engine_path()).unwrap();
 for(name,inputs,shared)in[("repeat",vec![a.as_path(),a.as_path()],true),("hardlink",vec![a.as_path(),alias.as_path()],true),("independent",vec![a.as_path(),copy.as_path()],false)]{
  let output=dir.path().join(format!("{name}.pdf"));success(invoke(&inputs,&output,None,None));let saved=engine.open_document(&output).unwrap();
  let x=engine.list_form_fields(&saved.id,1).unwrap();let y=engine.list_form_fields(&saved.id,4).unwrap();assert_eq!(x[0].name==y[0].name,shared,"{name}");capture(name,&inputs,&output,None);
 }
}

#[test]fn composition_single_selected_source_identity_copies_original_protected_bytes(){
 let dir=tempfile::tempdir().unwrap();let a=dir.path().join("plain.pdf");let bytes=fixture::write(&a);let b=dir.path().join("protected.pdf");let original=protected_fixture::encrypt(&bytes,lopdf::encryption::Permissions::empty());fs::write(&b,&original).unwrap();
 let selection=json!({"version":1,"pages":[{"source":2,"page":1},{"source":2,"page":2},{"source":2,"page":3}]});let req=request(dir.path(),selection);let output=dir.path().join("identity.pdf");
 let value=success(invoke(&[&a,&b],&output,Some(&req),Some(br#"[null,"reader"]"#)));assert_eq!(value["protectionSource"],2);assert_eq!(fs::read(output).unwrap(),original);
}

#[test]fn composition_retains_later_source_encryption_and_refuses_permissions_or_conflicts(){
 use lopdf::encryption::Permissions;
 let dir=tempfile::tempdir().unwrap();let a=dir.path().join("plain.pdf");let bytes=fixture::write(&a);let b=dir.path().join("protected.pdf");let encrypted=protected_fixture::encrypt(&bytes,Permissions::ASSEMBLABLE);fs::write(&b,&encrypted).unwrap();let output=dir.path().join("protected-output.pdf");
 let value=success(invoke(&[&a,&b],&output,None,Some(br#"[null,"reader"]"#)));assert_eq!(value["protectionSource"],2);
 let engine=PdfEngine::start(engine_path()).unwrap();assert!(engine.open_document(&output).is_err());let saved=engine.open_document_with_password(&output,"reader").unwrap();assert_eq!(saved.pages.len(),6);assert!(saved.protection.unwrap().permissions.assemble);capture("protected-late",&[&a,&b],&output,None);
 let denied=dir.path().join("denied.pdf");fs::write(&denied,protected_fixture::encrypt(&bytes,Permissions::empty())).unwrap();let target=dir.path().join("refused.pdf");rejected(invoke(&[&a,&denied],&target,None,Some(br#"[null,"reader"]"#)),"permission_denied");assert!(!target.exists());
 let different=dir.path().join("different.pdf");fs::write(&different,protected_fixture::encrypt(&bytes,Permissions::ASSEMBLABLE)).unwrap();rejected(invoke(&[&b,&different],&target,None,Some(br#"["reader","reader"]"#)),"invalid_request");assert!(!target.exists());assert_eq!(fs::read(b).unwrap(),encrypted);
}

#[test]fn composition_rejects_malformed_credentials_maps_and_existing_outputs(){
 let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a.pdf");let original=fixture::write(&a);let b=dir.path().join("b.pdf");second(&b);let output=dir.path().join("new.pdf");
 for value in[json!({"version":2,"pages":[{"source":1,"page":1}]}),json!({"version":1,"pages":[]}),json!({"version":1,"pages":[{"source":0,"page":1}]}),json!({"version":1,"pages":[{"source":3,"page":1}]}),json!({"version":1,"pages":[{"source":1,"page":4}]}),json!({"version":1,"pages":[{"source":1,"page":1,"rotation":45}]}),json!({"version":1,"pages":[{"source":1,"page":1,"input":"secret"}]}),json!({"version":1,"pages":null}),json!({"version":1,"pages":vec![json!({"source":1,"page":1});10001]})]{let req=request(dir.path(),value);rejected(invoke(&[&a,&b],&output,Some(&req),None),"invalid_request");assert!(!output.exists());}
 for bytes in[br#"["PRIVATE_SENTINEL"]"#.as_slice(),br#"[null,5]"#,br#"{"password":"PRIVATE_SENTINEL"}"#,br#"[null,"bad\npassword"]"#]{let result=invoke(&[&a,&b],&output,None,Some(bytes));assert!(!String::from_utf8_lossy(&result.stderr).contains("PRIVATE_SENTINEL"));rejected(result,"invalid_arguments");assert!(!output.exists());}
 rejected(invoke(&[&a,&a],&output,None,Some(br#"[null,"conflict"]"#)),"invalid_request");assert!(!output.exists());
 fs::write(&output,b"existing destination").unwrap();rejected(invoke(&[&a,&b],&output,None,None),"invalid_request");assert_eq!(fs::read(output).unwrap(),b"existing destination");assert_eq!(fs::read(a).unwrap(),original);
}

#[cfg(windows)]#[test]fn composition_non_unicode_input_and_output_paths_are_lossless(){
 use std::{ffi::OsString,os::windows::ffi::{OsStringExt,OsStrExt}};
 let dir=tempfile::tempdir().unwrap();let a=dir.path().join(OsString::from_wide(&[0xd800,46,112,100,102]));let bytes=fixture::write(&a);let output=dir.path().join(OsString::from_wide(&[0xd801,46,112,100,102]));
 let value=success(invoke(&[&a],&output,None,None));assert!(value["path"].is_null());let units:Vec<u16>=serde_json::from_value(value["pathNative"]["units"].clone()).unwrap();assert_eq!(units,output.canonicalize().unwrap().as_os_str().encode_wide().collect::<Vec<_>>());assert_eq!(fs::read(output).unwrap(),bytes);
}

#[test]fn composition_authentication_and_secret_limits_fail_without_publication(){
 let dir=tempfile::tempdir().unwrap();let a=dir.path().join("protected.pdf");let plain=dir.path().join("plain.pdf");let bytes=fixture::write(&plain);let original=protected_fixture::encrypt(&bytes,lopdf::encryption::Permissions::ASSEMBLABLE);fs::write(&a,&original).unwrap();let output=dir.path().join("new.pdf");
 rejected(invoke(&[&a],&output,None,None),"password_required");rejected(invoke(&[&a],&output,None,Some(br#"["wrong-private-secret"]"#)),"incorrect_password");assert!(!output.exists());
 for secret in[serde_json::to_vec(&vec!["é".repeat(513)]).unwrap(),vec![b' ';128*1024+1]]{rejected(invoke(&[&plain],&output,None,Some(&secret)),"invalid_arguments");assert!(!output.exists());}
 let exact=serde_json::to_vec(&vec!["é".repeat(512)]).unwrap();success(invoke(&[&plain],&output,None,Some(&exact)));assert_eq!(fs::read(output).unwrap(),bytes);assert_eq!(fs::read(a).unwrap(),original);
}

#[test]fn composition_rejects_large_request_and_unrelated_or_duplicate_options(){
 let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a.pdf");fixture::write(&a);let output=dir.path().join("new.pdf");let req=dir.path().join("oversized.json");fs::write(&req,vec![b' ';512*1024+1]).unwrap();rejected(invoke(&[&a],&output,Some(&req),None),"invalid_request");assert!(!output.exists());
 for options in[vec!["--password-stdin"],vec!["--page","1"],vec!["--out","again.pdf"],vec!["--passwords-stdin","--passwords-stdin"],vec!["--request"]]{
  let out=Command::new(env!("CARGO_BIN_EXE_folio-cli")).arg("compose-pages").arg("--input").arg(&a).arg("--out").arg(&output).args(options).output().unwrap();assert_eq!(out.status.code(),Some(2));rejected(out,"invalid_arguments");assert!(!output.exists());
 }
 let out=Command::new(env!("CARGO_BIN_EXE_folio-cli")).args(["compose-pages","--out"]).arg(output).output().unwrap();assert_eq!(out.status.code(),Some(2));rejected(out,"invalid_arguments");
}

#[test]fn composition_retains_native_refusal_for_mixed_form_default_languages(){
 let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a.pdf");let original_a=fixture::write(&a);let b=dir.path().join("b.pdf");second(&b);
 let mut d=lopdf::Document::load(&b).unwrap();d.catalog_mut().unwrap().set("Lang",lopdf::Object::string_literal("fr-FR"));let mut bytes=Vec::new();d.save_to(&mut bytes).unwrap();fs::write(&b,&bytes).unwrap();
 let output=dir.path().join("refused.pdf");rejected(invoke(&[&a,&b],&output,None,None),"invalid_request");assert!(!output.exists());assert_eq!(fs::read(a).unwrap(),original_a);assert_eq!(fs::read(b).unwrap(),bytes);
}

#[cfg(windows)]fn held_source()->tempfile::NamedTempFile{
 use std::os::windows::fs::OpenOptionsExt;
 use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_DELETE_ON_CLOSE;
 let file=tempfile::Builder::new().suffix(".pdf").make(|path|fs::OpenOptions::new().read(true).write(true).create_new(true).custom_flags(FILE_FLAG_DELETE_ON_CLOSE).open(path)).unwrap();fixture::write(file.path());file
}

#[cfg(windows)]#[test]fn composition_reads_a_source_with_parent_owned_snapshot_handle(){
 let source=held_source();let original=fs::read(source.path()).unwrap();let dir=tempfile::tempdir().unwrap();let output=dir.path().join("copy.pdf");success(invoke(&[source.path()],&output,None,None));assert_eq!(fs::read(output).unwrap(),original);assert_eq!(fs::read(source.path()).unwrap(),original);
}

#[cfg(windows)]#[test]fn composition_mcp_launch_accepts_a_read_shared_parent_owned_source(){
 let source=held_source();let dir=tempfile::tempdir().unwrap();let result=Command::new(env!("CARGO_BIN_EXE_folio-cli")).arg("mcp").arg("--input").arg(source.path()).arg("--out-dir").arg(dir.path()).stdin(Stdio::null()).output().unwrap();assert!(result.status.success(),"{}",String::from_utf8_lossy(&result.stderr));assert!(source.path().exists());assert_eq!(fs::read_dir(dir.path()).unwrap().count(),0);
}
