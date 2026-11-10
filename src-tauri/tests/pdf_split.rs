use folio_engine::{PdfEngine,PagePlan,SplitPdfRequest,SplitPdfPart,PageImageControl,ExportRequest};
use lopdf::{dictionary,Document,Stream,Object};
use std::{fs,path::{Path,PathBuf},sync::Arc};
fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path){
 let mut d=Document::with_version("1.7");let root_pages=d.new_object_id();
 let font=d.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
 let ids:Vec<_>=(0..4).map(|_|d.new_object_id()).collect();let mut fields=vec![];
 for(i,id)in ids.iter().enumerate(){
  let stream=d.add_object(Stream::new(dictionary!{},format!("BT /F1 18 Tf 40 200 Td (Page {}) Tj ET",i+1).into_bytes()));
  let ap=d.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),20.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}},b"BT /F1 10 Tf 2 4 Td (Filled value) Tj ET".to_vec()));
  let field=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>Object::string_literal(format!("Field{}",i+1)),"V"=>Object::string_literal("Filled value"),"DA"=>Object::string_literal("/F1 10 Tf 0 g"),"P"=>*id,"Rect"=>vec![40.into(),100.into(),140.into(),120.into()],"AP"=>dictionary!{"N"=>ap},"F"=>4});fields.push(field.into());
  let target=ids[if i%2==0{i+1}else{i-1}];
  let link=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Link","Rect"=>vec![40.into(),180.into(),100.into(),195.into()],"Dest"=>vec![target.into(),Object::Name(b"Fit".to_vec())]});
  d.objects.insert(*id,dictionary!{"Type"=>"Page","Parent"=>root_pages,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"UserUnit"=>1.7,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>stream,"Annots"=>vec![field.into(),link.into()]}.into());
 }
 d.objects.insert(root_pages,dictionary!{"Type"=>"Pages","Kids"=>ids.iter().copied().map(Object::Reference).collect::<Vec<_>>(),"Count"=>4}.into());
 let catalog=d.add_object(dictionary!{"Type"=>"Catalog","Pages"=>root_pages,"AcroForm"=>dictionary!{"Fields"=>fields,"DR"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}}});d.trailer.set("Root",catalog);d.save(path).unwrap();
}
fn plans(e:&PdfEngine,path:&Path)->Vec<PagePlan>{let info=e.open_document(path).unwrap();info.pages.iter().enumerate().map(|(i,p)|serde_json::from_value(serde_json::json!({"id":format!("p{i}"),"sourceId":info.id,"pageIndex":i,"width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})).unwrap()).collect()}
fn request(pages:Vec<PagePlan>)->SplitPdfRequest{SplitPdfRequest{metadata:None,pages,parts:vec![SplitPdfPart{start:0,end:2},SplitPdfPart{start:2,end:4}],bookmarks:None}}
fn export(e:&PdfEngine,r:SplitPdfRequest,parent:&Path)->folio_engine::EngineResult<Option<folio_engine::PageImageExportResult>>{e.split_pdf(r,parent,Arc::new(PageImageControl::new()),Arc::new(|_|{}))}
#[test]fn partitions_retain_current_editable_pages_forms_links_and_bookmarks(){
 let e=engine();let t=tempfile::tempdir().unwrap();let source=t.path().join("source.pdf");fixture(&source);let original=fs::read(&source).unwrap();let mut p=plans(&e,&source);
 p[0].rotation=90;p[0].crop=Some(folio_engine::AnnotationRect{x:17.,y:34.,width:425.,height:561.});p[1].overlays=serde_json::from_value(serde_json::json!([{"type":"text","id":"edit","x":40,"y":50,"text":"Added text","fontSize":18,"color":"#0000ff"}])).unwrap();
 let mut r=request(p.clone());r.bookmarks=Some((0..4).map(|i|serde_json::from_value(serde_json::json!({"id":format!("b{i}"),"title":format!("Chapter {}",i+1),"level":0,"destination":{"kind":"array","value":[{"kind":"page","value":format!("p{i}")},{"kind":"name","value":[70,105,116]}]}})).unwrap()).collect());
 let result=export(&e,r,t.path()).unwrap().unwrap();assert_eq!(result.count,2);assert!(result.folder.file_name().unwrap().to_string_lossy().starts_with("Folio split-"));assert_eq!(fs::read_dir(&result.folder).unwrap().count(),2);
 for i in 0..2{let path=result.folder.join(format!("part-{:04}.pdf",i+1));let bytes=fs::read(&path).unwrap();let pdf=Document::load_mem(&bytes).unwrap();assert_eq!(pdf.get_pages().len(),2);assert_eq!(folio_engine::bookmarks::read(&bytes).unwrap().iter().map(|b|b.title.as_str()).collect::<Vec<_>>(),vec![if i==0{"Chapter 1"}else{"Chapter 3"},if i==0{"Chapter 2"}else{"Chapter 4"}]);
  let form=pdf.catalog().unwrap().get(b"AcroForm").unwrap();let (_,form)=pdf.dereference(form).unwrap();let fields=form.as_dict().unwrap().get(b"Fields").unwrap().as_array().unwrap();assert_eq!(fields.len(),2);for field in fields{let(_,f)=pdf.dereference(field).unwrap();assert_eq!(f.as_dict().unwrap().get(b"V").unwrap().as_str().unwrap(),b"Filled value");}
  let page_ids:Vec<_>=pdf.get_pages().values().copied().collect();for page in &page_ids{let mut retained_links=0;let annots=pdf.get_object(*page).unwrap().as_dict().unwrap().get(b"Annots").unwrap().as_array().unwrap();for annot in annots{let(_,a)=pdf.dereference(annot).unwrap();let a=a.as_dict().unwrap();if let Ok(dest)=a.get(b"Dest"){retained_links+=1;assert!(page_ids.contains(&dest.as_array().unwrap()[0].as_reference().unwrap()));}}assert_eq!(retained_links,1,"The within-part link must actually survive");}
  let saved=e.open_document(&path).unwrap();for j in 0..2{let expected=e.render_page_plan(p[i*2+j].clone(),400).unwrap();let info=&saved.pages[j];let reopened:PagePlan=serde_json::from_value(serde_json::json!({"id":format!("reopened-{i}-{j}"),"sourceId":saved.id,"pageIndex":j,"width":info.width,"height":info.height,"rotation":0,"overlays":info.overlays})).unwrap();let actual=e.render_page_plan(reopened,400).unwrap();assert!(image::load_from_memory(&expected).unwrap().to_rgba8()==image::load_from_memory(&actual).unwrap().to_rgba8(),"Composed pixels differ for part {i}, page {j}");}
 }
 assert_eq!(fs::read(source).unwrap(),original);
}
#[test]fn malformed_partitions_and_later_invalid_pages_create_no_folder(){
 let e=engine();let t=tempfile::tempdir().unwrap();let source=t.path().join("source.pdf");fixture(&source);let p=plans(&e,&source);let out=t.path().join("out");fs::create_dir(&out).unwrap();
 for bounds in [vec![],vec![(0,1),(2,4)],vec![(0,3),(2,4)],vec![(1,4)],vec![(0,5)],vec![(0,0),(0,4)]]{let mut r=request(p.clone());r.parts=bounds.into_iter().map(|(start,end)|SplitPdfPart{start,end}).collect();assert!(export(&e,r,&out).is_err());}
 let mut r=request(p);r.pages[3].width=1.;assert!(export(&e,r,&out).is_err());assert_eq!(fs::read_dir(out).unwrap().count(),0);
}
#[test]fn cancellation_after_first_part_rolls_back_only_owned_outputs(){
 let e=engine();let t=tempfile::tempdir().unwrap();let source=t.path().join("source.pdf");fixture(&source);let out=t.path().join("out");fs::create_dir(&out).unwrap();fs::write(out.join("keep.txt"),b"keep").unwrap();let c=Arc::new(PageImageControl::new());let cancel=c.clone();
 let result=e.split_pdf(request(plans(&e,&source)),&out,c,Arc::new(move|p|{if p.completed==1{cancel.cancel();}})).unwrap();assert!(result.is_none());assert_eq!(fs::read_dir(out).unwrap().count(),1);
}
#[path="support/protected_fixture.rs"]mod protected_fixture;
#[test]fn assembly_denial_on_a_later_source_is_checked_before_publication(){
 let e=engine();let t=tempfile::tempdir().unwrap();let source=t.path().join("source.pdf");fixture(&source);let mut p=plans(&e,&source);let protected=t.path().join("protected.pdf");fs::write(&protected,protected_fixture::encrypt(&fs::read(&source).unwrap(),lopdf::encryption::Permissions::COPYABLE)).unwrap();let info=e.open_document_with_password(&protected,"reader").unwrap();p[3].source_id=info.id;
 let out=t.path().join("out");fs::create_dir(&out).unwrap();assert!(matches!(export(&e,request(p.clone()),&out),Err(folio_engine::EngineError::PermissionDenied(_))));let r=SplitPdfRequest{metadata:None,pages:vec![p.pop().unwrap()],parts:vec![SplitPdfPart{start:0,end:1}],bookmarks:Some(vec![])};assert!(export(&e,r,&out).is_err());assert_eq!(fs::read_dir(out).unwrap().count(),0);
}
#[test]fn one_part_can_be_reopened_and_saved_as_an_editable_document(){
 let e=engine();let t=tempfile::tempdir().unwrap();let source=t.path().join("source.pdf");fixture(&source);let p=plans(&e,&source);let r=SplitPdfRequest{metadata:None,pages:p,parts:vec![SplitPdfPart{start:0,end:4}],bookmarks:Some(vec![])};let result=export(&e,r,t.path()).unwrap().unwrap();let p=plans(&e,&result.folder.join("part-0001.pdf"));e.export_pdf(ExportRequest{pages:p,flatten:false},t.path().join("resaved.pdf")).unwrap();
}
#[test]fn excluded_link_targets_are_removed_and_late_cancel_cannot_remove_success(){
 let e=engine();let t=tempfile::tempdir().unwrap();let source=t.path().join("source.pdf");fixture(&source);let r=SplitPdfRequest{metadata:None,pages:plans(&e,&source),parts:(0..4).map(|start|SplitPdfPart{start,end:start+1}).collect(),bookmarks:Some(vec![])};let control=Arc::new(PageImageControl::new());let late=control.clone();let result=e.split_pdf(r,t.path(),control,Arc::new(move|p|{if p.stage=="completed"{assert!(!late.cancel());}})).unwrap().unwrap();
 for i in 1..=4{let pdf=Document::load(result.folder.join(format!("part-{i:04}.pdf"))).unwrap();let id=*pdf.get_pages().values().next().unwrap();if let Ok(annots)=pdf.get_object(id).unwrap().as_dict().unwrap().get(b"Annots"){for a in annots.as_array().unwrap(){let(_,a)=pdf.dereference(a).unwrap();assert!(a.as_dict().unwrap().get(b"Dest").is_err());}}}
}
#[test]fn permitted_protected_parts_remain_encrypted(){
 let e=engine();let t=tempfile::tempdir().unwrap();let source=t.path().join("source.pdf");fixture(&source);let encrypted=t.path().join("protected.pdf");fs::write(&encrypted,protected_fixture::encrypt(&fs::read(&source).unwrap(),lopdf::encryption::Permissions::ASSEMBLABLE)).unwrap();let info=e.open_document_with_password(&encrypted,"reader").unwrap();let pages=info.pages.iter().enumerate().map(|(i,p)|serde_json::from_value(serde_json::json!({"id":format!("p{i}"),"sourceId":info.id,"pageIndex":i,"width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})).unwrap()).collect();let result=export(&e,request(pages),t.path()).unwrap().unwrap();
 for i in 1..=2{let path=result.folder.join(format!("part-{i:04}.pdf"));assert!(e.open_document(&path).is_err());let info=e.open_document_with_password(&path,"reader").unwrap();assert_eq!(info.pages.len(),2);let protection=info.protection.unwrap();assert!(!protection.permissions.copy_content);assert!(protection.permissions.assemble);}
}
#[test]fn cancellation_preserves_foreign_files_and_reports_incomplete_output(){
 let e=engine();let t=tempfile::tempdir().unwrap();let source=t.path().join("source.pdf");fixture(&source);let out=t.path().join("out");fs::create_dir(&out).unwrap();let target=out.clone();let c=Arc::new(PageImageControl::new());let cancel=c.clone();
 let error=e.split_pdf(request(plans(&e,&source)),&out,c,Arc::new(move|p|{if p.completed==1&&p.stage=="writing"{let folder=fs::read_dir(&target).unwrap().next().unwrap().unwrap().path();fs::write(folder.join("foreign.txt"),b"retain").unwrap();cancel.cancel();}})).unwrap_err();assert!(error.to_string().contains("Incomplete export remains"));let folder=fs::read_dir(out).unwrap().next().unwrap().unwrap().path();assert_eq!(fs::read(folder.join("foreign.txt")).unwrap(),b"retain");assert!(folder.join("EXPORT-IN-PROGRESS.txt").exists());assert!(folder.join("part-0001.pdf").exists(),"The ownership protocol preserves the entire incomplete folder when unexpected files appear");
}
