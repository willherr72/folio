use folio_engine::{PdfEngine,PagePlan,TextExportRequest,TextExportControl,PageImageExportRequest,PageImageOptions,PageImageFormat,PageImageControl};
use lopdf::{dictionary,Document,Object,Stream};
use image::GenericImageView;
use std::{fs,path::{Path,PathBuf},sync::Arc};

fn engine()->PdfEngine {PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path,xfa:bool) {
    let mut pdf=Document::with_version("1.7");let tree=pdf.new_object_id();
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let ap=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),40.into(),20.into()],"Resources"=>dictionary!{}},b"0 0.7 0 rg 0 0 40 20 re f".to_vec()));
    let mut kids=vec![];let mut fields=vec![];
    for (index,text) in ["First page","Second page"].iter().enumerate(){
        let stream=pdf.add_object(Stream::new(dictionary!{},format!("BT /F1 12 Tf 30 240 Td ({text}) Tj 0 -180 Td (Outside crop) Tj ET").into_bytes()));
        let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),300.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>stream});
        if index==1 {
            let widget=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>Object::string_literal("Visible field"),"V"=>Object::string_literal("Answer"),"Rect"=>vec![100.into(),180.into(),140.into(),200.into()],"P"=>page,"F"=>4,"AP"=>dictionary!{"N"=>ap}});
            pdf.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(widget)]);
            if xfa{fields.push(Object::Reference(widget));}
        }
        kids.push(Object::Reference(page));
    }
    pdf.objects.insert(tree,dictionary!{"Type"=>"Pages","Count"=>2,"Kids"=>kids}.into());
    let mut form=dictionary!{"Fields"=>fields,"NeedAppearances"=>false};
    if xfa{let packet=pdf.add_object(Stream::new(dictionary!{},b"<xdp:xdp xmlns:xdp=\"http://ns.adobe.com/xdp/\"/>".to_vec()));form.set("XFA",packet);}
    let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree,"AcroForm"=>form});pdf.trailer.set("Root",catalog);pdf.save(path).unwrap();
}
fn selected(e:&PdfEngine,path:&Path)->PagePlan {
    let source=e.open_document(path).unwrap();let p=&source.pages[1];
    serde_json::from_value(serde_json::json!({"id":"second","sourceId":source.id,"pageIndex":1,"width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})).unwrap()
}
fn text_case(xfa:bool){
    let e=engine();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("source.pdf");fixture(&source,xfa);let original=fs::read(&source).unwrap();let plan=selected(&e,&source);let out=temp.path().join("page.txt");
    e.export_text(TextExportRequest{pages:vec![plan],include_form_values:false},&out,Arc::new(TextExportControl::new()),Arc::new(|_|{})).unwrap().unwrap();
    let text=fs::read_to_string(out).unwrap();assert!(text.contains("Second page"));assert!(!text.contains("First page"));assert_eq!(fs::read(source).unwrap(),original);
}
fn image_case(xfa:bool){
    let e=engine();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("source.pdf");fixture(&source,xfa);let original=fs::read(&source).unwrap();let plan=selected(&e,&source);
    let result=e.export_page_images(PageImageExportRequest{pages:vec![plan],page_numbers:vec![2],workspace_page_count:2,options:PageImageOptions{format:PageImageFormat::Png,dpi:72,jpeg_quality:90}},temp.path(),Arc::new(PageImageControl::new()),Arc::new(|_|{})).unwrap().unwrap();
    let image=image::open(result.folder.join("page-0002.png")).unwrap().into_rgb8();assert_eq!(image.dimensions(),(300,300));
    assert!(image::imageops::crop_imm(&image,100,100,40,20).pixels().filter(|(_,_,p)|p[1]>140&&p[0]<50&&p[2]<50).count()>700,"Original widget appearance must survive read export");assert_eq!(fs::read(source).unwrap(),original);
}
#[test] fn orphan_widget_text_export_reads_selected_original_page(){text_case(false)}
#[test] fn xfa_static_text_export_reads_selected_original_page(){text_case(true)}
#[test] fn orphan_widget_image_export_keeps_original_appearance(){image_case(false)}
#[test] fn xfa_static_image_export_keeps_original_appearance(){image_case(true)}

fn edited_case(xfa:bool){
    let e=engine();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("source.pdf");fixture(&source,xfa);
    let mut pdf=Document::load(&source).unwrap();let id=pdf.get_pages()[&2];pdf.get_dictionary_mut(id).unwrap().set("UserUnit",1.7f32);pdf.save(&source).unwrap();
    let original=fs::read(&source).unwrap();let mut plan=selected(&e,&source);
    plan.crop=Some(folio_engine::AnnotationRect{x:0.,y:0.,width:510.,height:255.});plan.rotation=90;
    plan.overlays=serde_json::from_value(serde_json::json!([{"type":"text","id":"note","x":51,"y":136,"text":"Current draft","fontSize":20.4,"color":"#0000ff"}])).unwrap();
    let out=temp.path().join("edited.txt");
    e.export_text(TextExportRequest{pages:vec![plan.clone()],include_form_values:false},&out,Arc::new(TextExportControl::new()),Arc::new(|_|{})).unwrap().unwrap();
    let text=fs::read_to_string(out).unwrap();assert!(text.contains("Second page"));assert!(text.contains("Current draft"));assert!(!text.contains("First page"));assert!(!text.contains("Outside crop"));
    let result=e.export_page_images(PageImageExportRequest{pages:vec![plan.clone()],page_numbers:vec![2],workspace_page_count:2,options:PageImageOptions{format:PageImageFormat::Png,dpi:72,jpeg_quality:90}},temp.path(),Arc::new(PageImageControl::new()),Arc::new(|_|{})).unwrap().unwrap();
    let image=image::open(result.folder.join("page-0002.png")).unwrap().into_rgb8();assert_eq!(image.dimensions(),(255,510));
    assert!(image.pixels().filter(|p|p[1]>140&&p[0]<50&&p[2]<50).count()>1800,"Native field appearance survives edited export");
    assert!(image.pixels().filter(|p|p[2]>180&&p[0]<100&&p[1]<100).count()>50,"Current blue drawing survives edited export");
    let preview=image::load_from_memory(&e.render_page_plan(plan,255).unwrap()).unwrap().into_rgb8();assert_eq!(preview,image);
    assert_eq!(fs::read(source).unwrap(),original);
}
#[test] fn edited_orphan_widget_page_keeps_crop_rotation_units_and_draft(){edited_case(false)}
#[test] fn edited_xfa_static_page_keeps_crop_rotation_units_and_draft(){edited_case(true)}

#[test]
fn unsupported_form_page_projection_stays_guarded(){
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for xfa in [false,true]{
        let source=temp.path().join(format!("source-{xfa}.pdf"));fixture(&source,xfa);let original=fs::read(&source).unwrap();let plan=selected(&e,&source);
        let out=temp.path().join(format!("projected-{xfa}.pdf"));
        assert!(e.export_pdf(folio_engine::ExportRequest{pages:vec![plan],flatten:false},&out).is_err(),"Read compatibility must not authorize unsupported editable form projection");
        assert!(!out.exists());assert_eq!(fs::read(source).unwrap(),original);
    }
}

#[test]
fn repeated_source_leaf_keeps_existing_cropped_read_support(){
    let e=engine();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("repeated.pdf");fixture(&source,false);
    let mut pdf=Document::load(&source).unwrap();pdf.catalog_mut().unwrap().remove(b"AcroForm");
    let pages=pdf.get_pages();pdf.get_dictionary_mut(pages[&2]).unwrap().remove(b"Annots");
    let tree=pdf.catalog().unwrap().get(b"Pages").unwrap().as_reference().unwrap();
    pdf.get_dictionary_mut(tree).unwrap().set("Kids",vec![Object::Reference(pages[&1]),Object::Reference(pages[&2]),Object::Reference(pages[&1])]);pdf.get_dictionary_mut(tree).unwrap().set("Count",3);pdf.save(&source).unwrap();
    let original=fs::read(&source).unwrap();let mut plan=selected(&e,&source);plan.page_index=2;
    plan.crop=Some(folio_engine::AnnotationRect{x:10.,y:40.,width:200.,height:120.});
    let out=temp.path().join("selected.txt");e.export_text(TextExportRequest{pages:vec![plan.clone()],include_form_values:false},&out,Arc::new(TextExportControl::new()),Arc::new(|_|{})).unwrap().unwrap();
    let text=fs::read_to_string(out).unwrap();assert!(text.contains("First page"));assert!(!text.contains("Second page"));assert!(!text.contains("Outside crop"));
    let image=image::load_from_memory(&e.render_page_plan(plan,200).unwrap()).unwrap();assert_eq!(image.dimensions(),(200,120));assert_eq!(fs::read(source).unwrap(),original);
}

#[test]
fn xfa_read_view_applies_imported_drawing_replacement_and_deletion(){imported_drawing_case(false)}
#[test]
fn scripted_xfa_read_view_applies_imported_drawing_replacement_and_deletion(){imported_drawing_case(true)}

fn imported_drawing_case(script:bool){
    let e=engine();let temp=tempfile::tempdir().unwrap();let seed=temp.path().join("seed.pdf");fixture(&seed,true);
    // Create genuine Folio drawing appearances on an ordinary AcroForm, then
    // add a producer XFA packet. Reopening must use the imported-draft path.
    let mut pdf=Document::load(&seed).unwrap();pdf.catalog_mut().unwrap().get_mut(b"AcroForm").unwrap().as_dict_mut().unwrap().remove(b"XFA");pdf.save(&seed).unwrap();
    let mut plan=selected(&e,&seed);
    plan.overlays=serde_json::from_value(serde_json::json!([
        {"type":"text","id":"deleted","x":30,"y":40,"text":"Delete this note","fontSize":12,"color":"#ff00ff"},
        {"type":"text","id":"changed","x":30,"y":80,"text":"Old answer","fontSize":12,"color":"#0000ff"}
    ])).unwrap();
    let saved=temp.path().join("saved.pdf");e.export_pdf(folio_engine::ExportRequest{pages:vec![plan],flatten:false},&saved).unwrap();
    let mut pdf=Document::load(&saved).unwrap();let packet=pdf.add_object(Stream::new(dictionary!{},b"<xdp:xdp xmlns:xdp=\"http://ns.adobe.com/xdp/\"/>".to_vec()));
    let form=pdf.catalog().unwrap().get(b"AcroForm").unwrap().clone();let id=form.as_reference().ok();
    if let Some(id)=id{pdf.get_dictionary_mut(id).unwrap().set("XFA",packet);}else{pdf.catalog_mut().unwrap().get_mut(b"AcroForm").unwrap().as_dict_mut().unwrap().set("XFA",packet);}
    if script{pdf.catalog_mut().unwrap().set("OpenAction",dictionary!{"S"=>"JavaScript","JS"=>Object::string_literal("void 0;")});}
    pdf.save(&saved).unwrap();let original=fs::read(&saved).unwrap();let info=e.open_document(&saved).unwrap();let p=&info.pages[0];assert_eq!(p.overlays.len(),2);
    let mut plan:PagePlan=serde_json::from_value(serde_json::json!({"id":"edited","sourceId":info.id,"pageIndex":0,"width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})).unwrap();
    plan.overlays.retain(|v|!matches!(v,folio_engine::Overlay::Text(t) if t.id=="deleted"));
    let folio_engine::Overlay::Text(text)=&mut plan.overlays[0]else{panic!("imported text")};text.text="Updated answer".into();
    let out=temp.path().join("edited.txt");e.export_text(TextExportRequest{pages:vec![plan.clone()],include_form_values:false},&out,Arc::new(TextExportControl::new()),Arc::new(|_|{})).unwrap().unwrap();
    let text=fs::read_to_string(out).unwrap();assert!(text.contains("Updated answer"));assert!(!text.contains("Delete this note"));assert!(!text.contains("Old answer"));
    let png=e.render_page_plan(plan,300).unwrap();let image=image::load_from_memory(&png).unwrap().into_rgb8();
    assert_eq!(image.pixels().filter(|p|p[0]>180&&p[2]>180&&p[1]<100).count(),0,"Deleted magenta annotation must not reappear");
    assert!(image.pixels().filter(|p|p[2]>180&&p[0]<100&&p[1]<100).count()>30);assert_eq!(fs::read(saved).unwrap(),original);
}

#[test]
fn unrelated_document_script_does_not_block_added_text_export(){
    let e=engine();let temp=tempfile::tempdir().unwrap();let source=temp.path().join("script.pdf");fixture(&source,false);
    let mut pdf=Document::load(&source).unwrap();let page=pdf.get_pages()[&2];pdf.get_dictionary_mut(page).unwrap().remove(b"Annots");pdf.catalog_mut().unwrap().remove(b"AcroForm");
    pdf.catalog_mut().unwrap().set("OpenAction",dictionary!{"S"=>"JavaScript","JS"=>Object::string_literal("void 0;")});pdf.save(&source).unwrap();
    let original=fs::read(&source).unwrap();let mut plan=selected(&e,&source);
    plan.overlays=serde_json::from_value(serde_json::json!([{"type":"text","id":"note","x":30,"y":90,"text":"Unsaved note","fontSize":12,"color":"#000000"}])).unwrap();
    let out=temp.path().join("text.txt");e.export_text(TextExportRequest{pages:vec![plan],include_form_values:false},&out,Arc::new(TextExportControl::new()),Arc::new(|_|{})).unwrap().unwrap();
    let text=fs::read_to_string(out).unwrap();assert!(text.contains("Second page")&&text.contains("Unsaved note"));assert!(!text.contains("First page"));assert_eq!(fs::read(source).unwrap(),original);
}

#[test]
#[ignore = "requires explicit local producer corpus and completed CLI export evidence"]
fn real_producer_exports_match_direct_original_page_rendering(){
    let inputs=PathBuf::from(std::env::var_os("FOLIO_READ_CORPUS_INPUTS").expect("explicit inputs manifest"));
    let exports=PathBuf::from(std::env::var_os("FOLIO_READ_CORPUS_EXPORTS").expect("completed CLI exports"));
    let references=PathBuf::from(std::env::var_os("FOLIO_READ_CORPUS_REFERENCES").expect("new reference directory"));
    fs::create_dir(&references).unwrap();let e=engine();
    let manifest:serde_json::Value=serde_json::from_slice(&fs::read(inputs).unwrap()).unwrap();let mut records=vec![];
    for input in manifest["inputs"].as_array().unwrap(){
        let source=PathBuf::from(input["path"].as_str().unwrap());let original=fs::read(&source).unwrap();
        let info=e.open_document(&source).unwrap();let stem=source.file_stem().unwrap().to_str().unwrap();
        let folder=exports.join(stem);let image_folder=fs::read_dir(&folder).unwrap().map(|v|v.unwrap().path()).find(|p|p.is_dir()&&p.file_name().unwrap().to_string_lossy().starts_with("Folio images-")).unwrap();
        for (index,page) in info.pages.iter().enumerate(){
            // Source render deliberately bypasses prepare_read_page and overlay
            // composition. This corpus has no removed editable annotations.
            assert!(page.overlays.is_empty());
            let direct=e.render_page(&info.id,index,page.width.round() as u32).unwrap();
            let exported=image::open(image_folder.join(format!("page-{:04}.png",index+1))).unwrap().into_rgb8();
            let expected=image::load_from_memory(&direct).unwrap().into_rgb8();assert_eq!(exported.dimensions(),expected.dimensions());
            assert_eq!(exported.as_raw().iter().zip(expected.as_raw()).filter(|(a,b)|a!=b).count(),0,"{stem} page {}: export differs from direct original rendering",index+1);
            let name=format!("{stem}-{:04}.png",index+1);fs::write(references.join(&name),direct).unwrap();
            records.push(serde_json::json!({"source":stem,"page":index+1,"reference":name,"pixelsEqual":true}));
        }
        assert_eq!(fs::read(&source).unwrap(),original);e.close_document(&info.id).unwrap();
    }
    fs::write(references.join("result.json"),serde_json::to_vec_pretty(&records).unwrap()).unwrap();
}
