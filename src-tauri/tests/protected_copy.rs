use folio_engine::{ExportRequest,PdfEngine,PdfSaveControl,PdfSaveProgress,ProtectionChange};
use lopdf::{dictionary,Document,Object,Stream};
use serde_json::json;
use std::{fs,path::{Path,PathBuf},sync::Arc};

fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture()->Vec<u8>{
 let mut doc=Document::with_version("1.7");let tree=doc.new_object_id();
 let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
 let content=doc.add_object(Stream::new(dictionary!{},b"BT /F1 16 Tf 30 70 Td (Protected copy sentinel) Tj ET".to_vec()));
 let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),200.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
 doc.objects.insert(tree,Object::Dictionary(dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}));
 let root=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree});doc.trailer.set("Root",root);let mut bytes=vec![];doc.save_to(&mut bytes).unwrap();bytes
}
fn options(user:&str,owner:&str)->ProtectionChange{serde_json::from_value(json!({"mode":"passwords","userPassword":user,"ownerPassword":owner,"permissions":{"print":true,"printHighQuality":false,"copyContent":false,"editContent":false,"annotate":false,"fillForms":true,"assemble":false}})).unwrap()}
fn plan(source:&folio_engine::DocumentInfo)->ExportRequest{serde_json::from_value(json!({"pages":source.pages.iter().enumerate().map(|(i,p)|json!({"id":format!("{}-{i}",source.id),"sourceId":source.id,"pageIndex":i,"width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})).collect::<Vec<_>>()})).unwrap()}
fn save(engine:&PdfEngine,request:ExportRequest,path:&Path,change:ProtectionChange)->folio_engine::EngineResult<bool>{engine.export_pdf_with_security(request,None,path,change,Arc::new(PdfSaveControl::new()),Arc::new(|_|{}))}

#[test]fn new_revision_six_security_retains_pixels_and_distinct_credentials(){
 let engine=engine();let dir=tempfile::tempdir().unwrap();let input=dir.path().join("input.pdf");let original=fixture();fs::write(&input,&original).unwrap();let source=engine.open_document(&input).unwrap();
 let out=dir.path().join("encrypted.pdf");assert!(save(&engine,plan(&source),&out,options("reader-new","owner-new")).unwrap());
 assert!(engine.open_document(&out).is_err());assert!(engine.open_document_with_password(&out,"wrong").is_err());
 let reader=engine.open_document_with_password(&out,"reader-new").unwrap();let owner=engine.open_document_with_password(&out,"owner-new").unwrap();
 let rights=reader.protection.as_ref().unwrap();assert!(!rights.owner);assert!(rights.permissions.print);assert!(!rights.permissions.print_high_quality);assert!(!rights.permissions.edit_content);assert!(!rights.permissions.copy_content);assert!(rights.permissions.fill_forms);
 assert!(owner.protection.as_ref().unwrap().owner);
 let unlocked=folio_engine::protection::unlock_pdf(&fs::read(&out).unwrap(),Some("reader-new")).unwrap();assert_eq!(unlocked.protection.unwrap().revision(),6);
 assert_eq!(engine.render_page(&source.id,0,300).unwrap(),engine.render_page(&reader.id,0,300).unwrap());assert_eq!(fs::read(&input).unwrap(),original);
 let out2=dir.path().join("another.pdf");save(&engine,plan(&source),&out2,options("reader-new","owner-new")).unwrap();assert_ne!(fs::read(&out).unwrap(),fs::read(&out2).unwrap());
 assert!(save(&engine,plan(&source),&input,options("reader-new","owner-new")).is_err());assert_eq!(fs::read(&input).unwrap(),original);
}

#[test]fn remove_and_replace_require_owner_even_when_reader_can_edit(){
 let engine=engine();let dir=tempfile::tempdir().unwrap();let input=dir.path().join("input.pdf");fs::write(&input,fixture()).unwrap();let source=engine.open_document(&input).unwrap();
 let protected=dir.path().join("protected.pdf");let mut all:serde_json::Value=serde_json::from_str(&serde_json::to_string(&json!({"mode":"passwords","userPassword":"reader","ownerPassword":"owner","permissions":{"print":true,"printHighQuality":true,"copyContent":true,"editContent":true,"annotate":true,"fillForms":true,"assemble":true}})).unwrap()).unwrap();
 save(&engine,plan(&source),&protected,serde_json::from_value(all.take()).unwrap()).unwrap();let original=fs::read(&protected).unwrap();
 let reader=engine.open_document_with_password(&protected,"reader").unwrap();let owner=engine.open_document_with_password(&protected,"owner").unwrap();
 for change in [ProtectionChange::Remove{},options("next-reader","next-owner")]{let out=dir.path().join("denied.pdf");fs::write(&out,b"keep").unwrap();assert!(save(&engine,plan(&reader),&out,change).is_err());assert_eq!(fs::read(out).unwrap(),b"keep");}
 let plain=dir.path().join("removed.pdf");save(&engine,plan(&owner),&plain,ProtectionChange::Remove{}).unwrap();assert!(engine.open_document(&plain).unwrap().protection.is_none());
 let replaced=dir.path().join("replaced.pdf");save(&engine,plan(&owner),&replaced,options("next-reader","next-owner")).unwrap();assert!(engine.open_document_with_password(&replaced,"reader").is_err());assert!(engine.open_document_with_password(&replaced,"owner").is_err());assert!(engine.open_document_with_password(&replaced,"next-owner").unwrap().protection.unwrap().owner);assert!(engine.open_document_with_password(&replaced,"next-reader").is_ok());assert_eq!(fs::read(protected).unwrap(),original);
 assert_eq!(engine.render_page(&owner.id,0,300).unwrap(),engine.render_page(&engine.open_document(&plain).unwrap().id,0,300).unwrap());
}

#[test]fn mixed_sources_need_every_owner_and_an_explicit_security_choice(){
 let engine=engine();let dir=tempfile::tempdir().unwrap();let input=dir.path().join("input.pdf");fs::write(&input,fixture()).unwrap();let source=engine.open_document(&input).unwrap();let mut owners=vec![];let mut readers=vec![];
 for n in 0..2{let path=dir.path().join(format!("protected{n}.pdf"));save(&engine,plan(&source),&path,options(&format!("reader{n}"),&format!("owner{n}"))).unwrap();owners.push(engine.open_document_with_password(&path,&format!("owner{n}")).unwrap());readers.push(engine.open_document_with_password(&path,&format!("reader{n}")).unwrap());}
 let mut request=plan(&owners[0]);request.pages.extend(plan(&owners[1]).pages);let output=dir.path().join("mixed.pdf");assert!(engine.export_pdf_controlled(request.clone(),None,&output,Arc::new(PdfSaveControl::new()),Arc::new(|_|{})).is_err());
 save(&engine,request.clone(),&output,options("combined-reader","combined-owner")).unwrap();assert_eq!(engine.open_document_with_password(&output,"combined-reader").unwrap().pages.len(),2);
 request.pages[1]=plan(&readers[1]).pages.remove(0);let previous=fs::read(&output).unwrap();assert!(save(&engine,request,&output,ProtectionChange::Remove{}).is_err());assert_eq!(fs::read(output).unwrap(),previous);
}

#[test]fn password_validation_rejects_normalized_collisions_lengths_and_invalid_permissions(){
 for (user,owner) in [("same","same"),("A","Ａ"),("","\u{00ad}"),("okay",""),("\u{0007}","owner"),("reader",&"x".repeat(128)),("\u{FDFA}",&"\u{FDFA}".repeat(20))]{assert!(options(user,owner).validate().is_err(),"invalid credentials accepted");}
 let valid=options("café","管理者");valid.validate().unwrap();let debug=format!("{valid:?}");assert!(!debug.contains("café")&&!debug.contains("管理者"));
 let mut inconsistent=json!({"mode":"passwords","userPassword":"reader","ownerPassword":"owner","permissions":{"print":false,"printHighQuality":true,"copyContent":true,"editContent":true,"annotate":false,"fillForms":true,"assemble":true}});
 assert!(serde_json::from_value::<ProtectionChange>(inconsistent.clone()).unwrap().validate().is_err());inconsistent["permissions"]["printHighQuality"]=json!(false);inconsistent["permissions"]["annotate"]=json!(true);inconsistent["permissions"]["fillForms"]=json!(false);assert!(serde_json::from_value::<ProtectionChange>(inconsistent).unwrap().validate().is_err());
 assert!(serde_json::from_value::<ProtectionChange>(json!({"mode":"remove","userPassword":"ignored"})).is_err());
}

#[test]fn permissions_only_and_unicode_credentials_round_trip(){
 let engine=engine();let dir=tempfile::tempdir().unwrap();let input=dir.path().join("input.pdf");fs::write(&input,fixture()).unwrap();let source=engine.open_document(&input).unwrap();
 for (n,user,owner) in [(0,"","管理者"),(1,"café","管理者")]{let out=dir.path().join(format!("unicode{n}.pdf"));save(&engine,plan(&source),&out,options(user,owner)).unwrap();let reader=engine.open_document_with_password(&out,user).unwrap();assert!(!reader.protection.unwrap().owner);assert!(engine.open_document_with_password(&out,owner).unwrap().protection.unwrap().owner);if user.is_empty(){assert!(engine.open_document(&out).is_ok());}}
}

#[test]fn cancellation_at_security_boundaries_never_publishes(){
 let engine=engine();let dir=tempfile::tempdir().unwrap();let input=dir.path().join("input.pdf");fs::write(&input,fixture()).unwrap();let source=engine.open_document(&input).unwrap();
 for stage in ["securing","writing"]{let output=dir.path().join(format!("{stage}.pdf"));fs::write(&output,b"preserved destination").unwrap();let control=Arc::new(PdfSaveControl::new());let cancel=control.clone();let report=Arc::new(move|event:PdfSaveProgress|{if event.stage==stage{assert!(cancel.cancel());}});
 assert!(!engine.export_pdf_with_security(plan(&source),None,&output,options("reader","owner"),control,report).unwrap());assert_eq!(fs::read(output).unwrap(),b"preserved destination");}
 assert!(fs::read_dir(dir.path()).unwrap().all(|e|!e.unwrap().file_name().to_string_lossy().starts_with(".folio-export-")));
}
