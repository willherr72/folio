use folio_engine::{PdfEngine,PagePlan,PageImageOptions,PageImageFormat,PageImageProgress,PageImageExportResult,PageImageExportRequest,PageImageControl};
use lopdf::{dictionary,Document,Stream};
use std::{path::{Path,PathBuf},sync::Arc,fs};
use image::GenericImageView;
fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path: &Path, unit: f32, rotation: i32) {
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let font = pdf.add_object(dictionary! {"Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"Helvetica"});
    let content = pdf.add_object(Stream::new(dictionary!{}, b"q 0.9 0.1 0.1 rg 30 40 50 60 re f 0 0 0 rg BT /F1 12 Tf 80 100 Td (Retained content) Tj ET Q".to_vec()));
    let appearance=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),40.into(),20.into()],"Resources"=>dictionary!{}},b"0 0.7 0 rg 0 0 40 20 re f".to_vec()));
    let widget=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>lopdf::Object::string_literal("Test field"),"V"=>lopdf::Object::string_literal("Form value"),"Rect"=>vec![100.into(),180.into(),140.into(),200.into()],"F"=>4,"AP"=>dictionary!{"N"=>appearance}});
    let annotation=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Square","Rect"=>vec![160.into(),230.into(),200.into(),250.into()],"F"=>4,"AP"=>dictionary!{"N"=>appearance}});
    let page = pdf.add_object(dictionary! {
        "Type"=>"Page", "Parent"=>pages,
        "MediaBox"=>vec![0.into(),0.into(),400.into(),500.into()],
        "CropBox"=>vec![10.into(),20.into(),310.into(),420.into()],
        "Rotate"=>rotation, "UserUnit"=>unit,"Annots"=>vec![widget.into(),annotation.into()],
        "Resources"=>dictionary! {"Font"=>dictionary! {"F1"=>font}}, "Contents"=>content
    });
    pdf.objects.insert(pages,dictionary! {"Type"=>"Pages", "Kids"=>vec![page.into()], "Count"=>1}.into());
    let root=pdf.add_object(dictionary! {"Type"=>"Catalog", "Pages"=>pages,"AcroForm"=>dictionary!{"Fields"=>vec![widget.into()],"NeedAppearances"=>false}});
    pdf.trailer.set("Root",root);
    pdf.save(path).unwrap();
}

fn plans(engine:&PdfEngine,path:&Path)->Vec<PagePlan>{
 let source=engine.open_document(path).unwrap();source.pages.iter().enumerate().map(|(i,p)|serde_json::from_value(serde_json::json!({"id":format!("p{i}"),"sourceId":source.id,"pageIndex":i,"width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})).unwrap()).collect()
}
fn options(format:PageImageFormat,dpi:u32)->PageImageOptions{PageImageOptions{format,dpi,jpeg_quality:90}}
fn export(engine:&PdfEngine,pages:Vec<PagePlan>,labels:Vec<u32>,format:PageImageFormat,dpi:u32,parent:&Path)->folio_engine::EngineResult<Option<PageImageExportResult>>{
 engine.export_page_images(PageImageExportRequest{workspace_page_count:labels.iter().copied().max().unwrap_or(1).max(1),pages,page_numbers:labels,options:options(format,dpi)},parent,Arc::new(PageImageControl::new()),Arc::new(|_|{}))
}
#[test]
fn exports_real_resolution_with_crop_rotation_and_source_owned_units(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for unit in [0.5,1.,1.7,2.]{for intrinsic in [0,90,180,270]{for rotation in [0,90,180,270]{
  let source=temp.path().join(format!("source-{unit}-{intrinsic}-{rotation}.pdf"));fixture(&source,unit,intrinsic);let original=fs::read(&source).unwrap();
  let mut p=plans(&e,&source).remove(0);p.crop=Some(folio_engine::AnnotationRect{x:10.*unit,y:20.*unit,width:100.*unit,height:150.*unit});p.rotation=rotation;
  let root=temp.path().join(format!("output-{unit}-{intrinsic}-{rotation}"));fs::create_dir(&root).unwrap();
  let result=export(&e,vec![p.clone()],vec![7],PageImageFormat::Png,300,&root).unwrap().unwrap();
  let png=image::open(result.folder.join("page-0007.png")).unwrap();
  let(w,h)=if rotation%180==0{(100.,150.)}else{(150.,100.)};assert_eq!(png.dimensions(),((w*unit*300./72.).round()as u32,(h*unit*300./72.).round()as u32));
  capture(&e,&p,&source,&result,300,&format!("matrix-{unit}-{intrinsic}-{rotation}"));
  assert_eq!(result.count,1);assert_eq!(fs::read_dir(&result.folder).unwrap().count(),1);assert_eq!(fs::read(&source).unwrap(),original);
  e.close_document(&p.source_id).unwrap();
 }}}
}
#[test]
fn png_and_jpeg_capture_current_annotations_and_white_background(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("source.pdf");fixture(&source,1.,0);
 let mut p=plans(&e,&source).remove(0);p.overlays=serde_json::from_value(serde_json::json!([{"type":"text","id":"added","x":30,"y":70,"text":"Visible edit","fontSize":18,"color":"#0000ff"}])).unwrap();
 let png=export(&e,vec![p.clone()],vec![1],PageImageFormat::Png,72,temp.path()).unwrap().unwrap();
 let jpg=export(&e,vec![p.clone()],vec![1],PageImageFormat::Jpeg,72,temp.path()).unwrap().unwrap();
 let a=image::open(png.folder.join("page-0001.png")).unwrap().to_rgb8();let b=image::open(jpg.folder.join("page-0001.jpg")).unwrap().to_rgb8();
 assert_eq!(a.dimensions(),(300,400));assert_eq!(a.dimensions(),b.dimensions());assert_eq!(a.get_pixel(0,0).0,[255,255,255]);
 assert!(a.pixels().filter(|p|p[2]>200&&p[0]<100&&p[1]<100).count()>30,"Current blue text annotation is rendered");
 // The two green appearances occupy separate regions in physical display coordinates.
 for (x,y) in [(90,220),(150,170)] {assert!(image::imageops::crop_imm(&a,x,y,40,20).pixels().filter(|(_,_,p)|p[1]>140&&p[0]<50&&p[2]<50).count()>700,"Form/annotation appearance at {x},{y} is rendered");}
 capture(&e,&p,&source,&png,72,"full-annotations");
 let expected=image::load_from_memory(&e.render_page_plan(p,300).unwrap()).unwrap().to_rgb8();assert_eq!(a,expected);
 let error:f64=a.as_raw().iter().zip(b.as_raw()).map(|(a,b)|(*a as f64-*b as f64).abs()).sum::<f64>()/a.as_raw().len()as f64;assert!(error<3.,"JPEG mean channel error {error}");
}
#[test]
fn cancellation_discards_only_the_owned_partial_folder(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("source.pdf");fixture(&source,1.,0);let p=plans(&e,&source).remove(0);let mut second=p.clone();second.id="second".into();
 let output=temp.path().join("outputs");fs::create_dir(&output).unwrap();fs::write(output.join("existing.txt"),b"keep").unwrap();
 let cancel=Arc::new(PageImageControl::new());let flag=cancel.clone();
 let result=e.export_page_images(PageImageExportRequest{pages:vec![p,second],page_numbers:vec![1,2],workspace_page_count:2,options:options(PageImageFormat::Png,150)},&output,cancel,Arc::new(move|progress:PageImageProgress|{if progress.completed==1{flag.cancel();}})).unwrap();
 assert!(result.is_none());assert_eq!(fs::read_dir(&output).unwrap().count(),1);assert_eq!(fs::read(output.join("existing.txt")).unwrap(),b"keep");
}
#[test]
fn invalid_or_oversized_batch_never_creates_output(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("source.pdf");fixture(&source,2.,0);let p=plans(&e,&source).remove(0);let output=temp.path().join("outputs");fs::create_dir(&output).unwrap();
 assert!(export(&e,vec![p.clone()],vec![1],PageImageFormat::Png,600,&output).is_err());
 assert!(export(&e,vec![p.clone()],vec![],PageImageFormat::Png,150,&output).is_err());
 assert!(export(&e,vec![p.clone()],vec![0],PageImageFormat::Png,150,&output).is_err());
 assert!(export(&e,vec![p.clone()],vec![1],PageImageFormat::Png,0,&output).is_err());
 let mut second=p.clone();second.id="second".into();second.height=4.;
 assert!(export(&e,vec![p,second],vec![1,2],PageImageFormat::Png,150,&output).is_err());
 assert_eq!(fs::read_dir(&output).unwrap().count(),0);
}

#[path="support/protected_fixture.rs"]mod protected_fixture;
#[test]
fn copy_permission_is_distinct_from_print_assembly_and_forged_ui_rights(){
 use lopdf::encryption::Permissions;use folio_engine::EngineError;
 let e=engine();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("source.pdf");fixture(&source,1.,0);let bytes=fs::read(&source).unwrap();
 let denied=temp.path().join("print-only.pdf");fs::write(&denied,protected_fixture::encrypt(&bytes,Permissions::PRINTABLE)).unwrap();
 let info=e.open_document_with_password(&denied,"reader").unwrap();let base=&info.pages[0];
 let forged:PagePlan=serde_json::from_value(serde_json::json!({"id":"forged","sourceId":info.id,"pageIndex":0,"width":base.width,"height":base.height,"rotation":0,"overlays":base.overlays,"protection":{"owner":true,"permissions":{"copyContent":true}}})).unwrap();
 let output=temp.path().join("outputs");fs::create_dir(&output).unwrap();assert!(matches!(export(&e,vec![forged],vec![1],PageImageFormat::Png,72,&output),Err(EngineError::PermissionDenied(_))));assert_eq!(fs::read_dir(&output).unwrap().count(),0);
 let allowed=temp.path().join("copy-only.pdf");fs::write(&allowed,protected_fixture::encrypt(&bytes,Permissions::COPYABLE)).unwrap();let info=e.open_document_with_password(&allowed,"reader").unwrap();let base=&info.pages[0];
 let p:PagePlan=serde_json::from_value(serde_json::json!({"id":"allowed","sourceId":info.id,"pageIndex":0,"width":base.width,"height":base.height,"rotation":90,"overlays":base.overlays})).unwrap();
 let result=export(&e,vec![p],vec![1],PageImageFormat::Png,72,&output).unwrap().unwrap();assert_eq!(image::open(result.folder.join("page-0001.png")).unwrap().dimensions(),(400,300));
}
#[test]
fn a_running_export_owns_font_and_image_resources_until_terminal_cleanup(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("source.pdf");fixture(&source,1.,0);let mut p=plans(&e,&source).remove(0);
 let font=e.fonts().register(fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/shaped-text/DejaVuSans.ttf")).unwrap()).unwrap();
 let image_path=temp.path().join("image.png");image::RgbImage::from_pixel(8,8,image::Rgb([255,0,0])).save(&image_path).unwrap();let image=e.import_image(&image_path).unwrap();
 p.overlays=serde_json::from_value(serde_json::json!([{"type":"text","id":"text","x":20,"y":40,"text":"Snapshot font","fontId":font.id,"fontSize":12,"color":"#000000"},{"type":"image","id":"image","assetId":image.image.id,"x":20,"y":80,"width":30,"height":30,"rotation":0}])).unwrap();
 let fonts=e.fonts();let images=e.images();let font_id=font.id.clone();let token=image.token.clone();
 let result=e.export_page_images(PageImageExportRequest{pages:vec![p],page_numbers:vec![1],workspace_page_count:1,options:options(PageImageFormat::Png,72)},temp.path(),Arc::new(PageImageControl::new()),Arc::new(move|progress|{if progress.stage=="validating"{fonts.remove(&font_id).unwrap();images.release(&token).unwrap();}})).unwrap().unwrap();
 assert!(result.folder.join("page-0001.png").is_file());assert!(e.fonts().get(&font.id).is_err());assert!(e.images().get(&image.image.id).is_err(),"Job image lease is released");
}
#[test]
fn cleanup_never_deletes_unexpected_files_and_reports_the_incomplete_folder(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("source.pdf");fixture(&source,1.,0);let p=plans(&e,&source).remove(0);let output=temp.path().join("outputs");fs::create_dir(&output).unwrap();
 let root=output.clone();let control=Arc::new(PageImageControl::new());let cancel=control.clone();
 let result=e.export_page_images(PageImageExportRequest{pages:vec![p],page_numbers:vec![1],workspace_page_count:1,options:options(PageImageFormat::Png,72)},&output,control,Arc::new(move|progress|{if progress.completed==1&&progress.stage=="writing"{let folder=fs::read_dir(&root).unwrap().next().unwrap().unwrap().path();fs::write(folder.join("unexpected.txt"),b"keep me").unwrap();cancel.cancel();}}));
 let error=result.unwrap_err().to_string();assert!(error.contains("Incomplete export remains at"));let folder=fs::read_dir(&output).unwrap().next().unwrap().unwrap().path();assert_eq!(fs::read(folder.join("unexpected.txt")).unwrap(),b"keep me");assert!(folder.join("EXPORT-IN-PROGRESS.txt").exists());
}

fn capture(e:&PdfEngine,page:&PagePlan,source:&Path,png:&PageImageExportResult,dpi:u32,name:&str){
 use std::io::Write;
 let Some(root)=std::env::var_os("FOLIO_IMAGE_EXPORT_CAPTURE_DIR")else{return};let root=PathBuf::from(root);fs::create_dir_all(&root).unwrap();
 let png_path=fs::read_dir(&png.folder).unwrap().next().unwrap().unwrap().path();
 let temporary=tempfile::tempdir().unwrap();let jpg=export(e,vec![page.clone()],vec![1],PageImageFormat::Jpeg,dpi,temporary.path()).unwrap().unwrap();
 for(suffix,input)in [("source.pdf",source.to_path_buf()),("png",png_path),("jpg",jpg.folder.join("page-0001.jpg"))]{let target=root.join(format!("{name}.{suffix}"));assert!(!target.exists());fs::copy(input,target).unwrap();}
 let oracle=root.join(format!("{name}.oracle.pdf"));assert!(!oracle.exists());e.export_pdf(folio_engine::ExportRequest{pages:vec![page.clone()],flatten:false},&oracle).unwrap();
 let mut rows=fs::OpenOptions::new().create(true).append(true).open(root.join("cases.jsonl")).unwrap();writeln!(rows,"{}",serde_json::json!({"name":name,"dpi":dpi,"jpegQuality":90})).unwrap();
}
#[test]
fn all_resolution_options_write_true_sized_images(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("source.pdf");fixture(&source,1.,0);let mut p=plans(&e,&source).remove(0);p.crop=Some(folio_engine::AnnotationRect{x:10.,y:20.,width:50.,height:80.});
 for dpi in [72,150,300,600]{let result=export(&e,vec![p.clone()],vec![1],PageImageFormat::Png,dpi,temp.path()).unwrap().unwrap();assert_eq!(image::open(result.folder.join("page-0001.png")).unwrap().dimensions(),((50.*dpi as f64/72.).round()as u32,(80.*dpi as f64/72.).round()as u32));capture(&e,&p,&source,&result,dpi,&format!("dpi-{dpi}"));}
}
