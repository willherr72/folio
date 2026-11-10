use folio_engine::{DocumentInfo, ExportRequest, PdfEngine};
use lopdf::{dictionary, Document, Stream};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn capture(path:&Path){if let Some(root)=std::env::var_os("FOLIO_CROP_CAPTURE_DIR"){let root=PathBuf::from(root);std::fs::create_dir_all(&root).unwrap();let target=root.join(path.file_name().unwrap());assert!(!target.exists(),"Preserve prior crop evidence");std::fs::copy(path,target).unwrap();}}

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()
}

fn fixture(path: &Path, unit: f32, rotation: i32) {
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let font = pdf.add_object(dictionary! {"Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"Helvetica"});
    let content = pdf.add_object(Stream::new(dictionary!{}, b"q 0.9 0.1 0.1 rg 30 40 50 60 re f 0 0 0 rg BT /F1 12 Tf 80 100 Td (Retained content) Tj ET Q".to_vec()));
    let page = pdf.add_object(dictionary! {
        "Type"=>"Page", "Parent"=>pages,
        "MediaBox"=>vec![0.into(),0.into(),400.into(),500.into()],
        "CropBox"=>vec![10.into(),20.into(),310.into(),420.into()],
        "Rotate"=>rotation, "UserUnit"=>unit,
        "Resources"=>dictionary! {"Font"=>dictionary! {"F1"=>font}}, "Contents"=>content
    });
    pdf.objects.insert(pages,dictionary! {"Type"=>"Pages", "Kids"=>vec![page.into()], "Count"=>1}.into());
    let root=pdf.add_object(dictionary! {"Type"=>"Catalog", "Pages"=>pages});
    pdf.trailer.set("Root",root);
    pdf.save(path).unwrap();
}

fn plan(source: &DocumentInfo, crop: Value, rotation: u16) -> Value {
    json!({"id":"page", "sourceId":source.id, "pageIndex":0,
        "width":source.pages[0].width,"height":source.pages[0].height,
        "rotation":rotation,"overlays":[],"crop":crop})
}

fn request(page: Value) -> ExportRequest {
    serde_json::from_value(json!({"pages":[page],"flatten":false})).unwrap()
}

#[test]
fn tall_crop_preview_has_a_bounded_raster() {
    let engine=engine();let dir=tempfile::tempdir().unwrap();let input=dir.path().join("source.pdf");
    fixture(&input,1.,0);let source=engine.open_document(&input).unwrap();
    let page=request(plan(&source,json!({"x":0,"y":0,"width":2,"height":30}),0)).pages.remove(0);
    let png=engine.render_page_plan(page,900).unwrap();let image=image::load_from_memory(&png).unwrap();
    assert!(image.height()<=12_000,"Tall crop raster was {} pixels high",image.height());
    assert!(u64::from(image.width())*u64::from(image.height())<=16_000_000);
    engine.close_document(&source.id).unwrap();
}

#[test]
fn crop_changes_visible_dimensions_across_units_and_rotations_without_changing_source() {
    let engine=engine();let dir=tempfile::tempdir().unwrap();
    for unit in [0.5_f32,1.,1.7,2.] { for intrinsic in [0,90,180,270] {
        let input=dir.path().join(format!("source-{unit}-{intrinsic}.pdf"));
        fixture(&input,unit,intrinsic);let before=std::fs::read(&input).unwrap();
        capture(&input);
        let source=engine.open_document(&input).unwrap();
        for rotation in [0,90,180,270] {
            let crop=json!({"x":20.*unit,"y":30.*unit,"width":100.*unit,"height":150.*unit});
            let output=dir.path().join(format!("crop-{unit}-{intrinsic}-{rotation}.pdf"));
            engine.export_pdf(request(plan(&source,crop,rotation)),&output).unwrap();
            let opened=engine.open_document(&output).unwrap();
            let (width,height)=if rotation==90||rotation==270 {(150.*unit,100.*unit)}else{(100.*unit,150.*unit)};
            assert!((opened.pages[0].width-width).abs()<0.001,"unit {unit}, intrinsic {intrinsic}, rotation {rotation}: width {} expected {width}",opened.pages[0].width);
            assert!((opened.pages[0].height-height).abs()<0.001);
            let pdf=Document::load(&output).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
            let expected=match intrinsic {0=>[30.,240.,130.,390.],90=>[40.,40.,190.,140.],180=>[190.,50.,290.,200.],270=>[130.,300.,280.,400.],_=>unreachable!()};
            let bounds=pdf.get_dictionary(page).unwrap().get(b"CropBox").unwrap().as_array().unwrap();
            for (actual,expected) in bounds.iter().zip(expected){assert!((actual.as_float().unwrap()-expected).abs()<0.0001);}
            let contents=pdf.get_page_content(page);
            assert!(String::from_utf8_lossy(&contents).contains("Retained content"),"Cropping must retain hidden source text");
            capture(&output);
            engine.close_document(&opened.id).unwrap();
        }
        engine.close_document(&source.id).unwrap();assert_eq!(before,std::fs::read(&input).unwrap());
    }}
}

#[test]
fn cropped_portable_text_reopens_and_reexports_at_the_same_position() {
    let engine=engine();let dir=tempfile::tempdir().unwrap();
    for unit in [0.5_f32,1.7,2.] {for rotation in [0,90,180,270]{
        let input=dir.path().join(format!("overlay-source-{unit}-{rotation}.pdf"));fixture(&input,unit,rotation);
        let source=engine.open_document(&input).unwrap();
        let mut p=plan(&source,json!({"x":20.*unit,"y":30.*unit,"width":200.*unit,"height":250.*unit}),90);
        p["overlays"]=json!([{"type":"text","id":"note","x":40.*unit,"y":70.*unit,"fontSize":12,"fontName":"Helvetica","text":"Editable after crop","color":"#123456"}]);
        let output=dir.path().join(format!("overlay-{unit}-{rotation}.pdf"));engine.export_pdf(request(p.clone()),&output).unwrap();
        let opened=engine.open_document(&output).unwrap();assert_eq!(opened.pages[0].overlays.len(),1,"{unit} {rotation}");
        let again=dir.path().join(format!("overlay-{unit}-{rotation}-again.pdf"));
        let mut replay=plan(&opened,Value::Null,0);replay["overlays"]=serde_json::to_value(&opened.pages[0].overlays).unwrap();
        engine.export_pdf(request(replay),&again).unwrap();
        let flat=dir.path().join(format!("overlay-{unit}-{rotation}-flat.pdf"));let mut flat_request=request(p);flat_request.flatten=true;engine.export_pdf(flat_request,&flat).unwrap();
        for path in [&output,&again,&flat]{capture(path);}
        engine.close_document(&opened.id).unwrap();engine.close_document(&source.id).unwrap();
    }}
}

#[test]
fn one_point_crop_endpoints_and_duplicate_occurrences_are_independent() {
    let engine=engine();let dir=tempfile::tempdir().unwrap();
    for unit in [0.5_f32,1.7,2.] {
        let input=dir.path().join(format!("source-{unit}.pdf"));fixture(&input,unit,0);let source=engine.open_document(&input).unwrap();
        let a=plan(&source,json!({"x":10,"y":20,"width":1,"height":1}),0);
        let mut b=plan(&source,Value::Null,0);b["id"]=json!("duplicate");
        let request=serde_json::from_value(json!({"pages":[a,b],"flatten":false})).unwrap();
        let output=dir.path().join(format!("duplicate-{unit}.pdf"));engine.export_pdf(request,&output).unwrap();
        let opened=engine.open_document(&output).unwrap();assert!((opened.pages[0].width-1.).abs()<0.0001);assert!((opened.pages[0].height-1.).abs()<0.0001);
        assert_eq!((opened.pages[1].width,opened.pages[1].height),(source.pages[0].width,source.pages[0].height));
        engine.close_document(&opened.id).unwrap();engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn invalid_crop_is_rejected_before_publication_or_preview() {
    let engine=engine();let dir=tempfile::tempdir().unwrap();let input=dir.path().join("source.pdf");
    fixture(&input,1.,0);let source=engine.open_document(&input).unwrap();
    for crop in [
        json!({"x":-1,"y":0,"width":100,"height":100}),
        json!({"x":0,"y":0,"width":0.9,"height":100}),
        json!({"x":0,"y":0,"width":100,"height":0}),
        json!({"x":250,"y":0,"width":100,"height":100}),
        json!({"x":0,"y":350,"width":100,"height":100}),
    ] {
        let output=dir.path().join("must-not-exist.pdf");let request=request(plan(&source,crop.clone(),0));
        assert!(engine.export_pdf(request.clone(),&output).is_err(),"Accepted invalid crop {crop}");
        assert!(!output.exists());
        assert!(engine.render_page_plan(request.pages[0].clone(),300).is_err());
    }
    engine.close_document(&source.id).unwrap();
}
