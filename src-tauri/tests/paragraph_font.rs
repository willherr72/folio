use folio_engine::{AnnotationRect, ParagraphFormatting, PdfEngine};
use lopdf::{dictionary, Document, Stream};
use std::path::{Path, PathBuf};
// PdfEngine::start shares a worker/font registry across callers in this process.
static REGISTRY: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn root() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")) }
fn engine() -> PdfEngine { PdfEngine::start(root().join("resources/pdfium/pdfium.dll")).unwrap() }
fn font(e: &PdfEngine) -> folio_engine::FontInfo {
    e.fonts().register(std::fs::read(root().join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap()
}
fn format(id: &str) -> ParagraphFormatting {
    serde_json::from_value(serde_json::json!({"fontId":id,"fontSize":16,"alignment":"right","lineSpacing":1.5})).unwrap()
}
fn fixture(path: &Path, rotation: i32) {
    let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
    let f=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
    let data=b"BT /F1 12 Tf 0.2 0.3 0.4 rg 60 400 Td (Old words) Tj 0 -150 Td (Neighbor) Tj ET";
    let content=pdf.add_object(Stream::new(dictionary!{},data.to_vec()));
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),500.into()],"Rotate"=>rotation,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>f,"FolioParagraph"=>f}},"Contents"=>content});
    pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());
    let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",catalog);pdf.save(path).unwrap();
}
fn area(rotation: i32) -> AnnotationRect {
    let (x,y,width,height)=match rotation {90=>(340.,60.,80.,180.),180=>(160.,340.,180.,80.),270=>(80.,160.,80.,180.),_=>(60.,80.,180.,80.)};
    AnnotationRect{x,y,width,height}
}

#[test]
fn explicit_font_supplies_a_missing_subset_letter_and_survives_release_and_reedit() {
    let _guard=REGISTRY.lock().unwrap();
    let e=engine();let f=font(&e);let input=root().join("../tests/fixtures/embedded-text/reportlab-subset.pdf");
    let original=std::fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();
    let selected=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();let bounds=AnnotationRect{x:30.,y:84.,width:130.,height:55.};
    let error=e.preview_text_paragraph(&source.id,0,&[0],&selected.text,"Cafè edit",&bounds).unwrap_err().to_string();
    assert!(error.contains("U+00E8"),"{error}");
    let edited=e.preview_text_paragraph_with_format(&source.id,0,&[0],&selected.text,"Cafè edit\njj   ",&bounds,&format(&f.id)).unwrap();
    // The resulting PDF owns the font; the registry is no longer needed.
    e.fonts().remove(&f.id).unwrap();
    let temp=tempfile::tempdir().unwrap();let saved=temp.path().join("saved.pdf");
    e.export_pdf(folio_engine::ExportRequest{flatten:false,pages:vec![folio_engine::PagePlan{crop: None,id:"p".into(),source_id:edited.id.clone(),page_index:0,width:edited.pages[0].width,height:edited.pages[0].height,rotation:0,overlays:vec![]}]},&saved).unwrap();
    let reopened=e.open_document(&saved).unwrap();let runs=e.list_text_runs(&reopened.id,0).unwrap().runs;
    let changed:Vec<_>=runs.iter().filter(|r|r.text!="Neighbor").collect();assert_eq!(changed.len(),2);
    assert!(changed.iter().all(|r|r.supported&&r.is_embedded&&(r.font_size-16.).abs()<0.01));
    assert!(changed.iter().any(|r|r.text=="Cafè edit"));
    let indices:Vec<_>=changed.iter().map(|r|r.object_index).collect();let selected=e.inspect_text_paragraph(&reopened.id,0,&indices).unwrap();
    let again=e.preview_text_paragraph(&reopened.id,0,&indices,&selected.text,"Cafè again",&bounds).unwrap();
    assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),original.as_slice());assert_eq!(std::fs::read(&input).unwrap(),original);
    for id in [&source.id,&edited.id,&reopened.id,&again.id]{e.close_document(id).unwrap();}
}

#[test]
fn substitute_composes_with_rotation_spacing_and_alignment_without_moving_neighbors() {
    let _guard=REGISTRY.lock().unwrap();
    let e=engine();let f=font(&e);let temp=tempfile::tempdir().unwrap();
    for rotation in [0,90,180,270] {
        let input=temp.path().join("source.pdf");fixture(&input,rotation);let original=std::fs::read(&input).unwrap();
        let source=e.open_document(&input).unwrap();let selected=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
        let neighbor=e.list_text_runs(&source.id,0).unwrap().runs[1].clone();
        let edited=e.preview_text_paragraph_with_format(&source.id,0,&[0],&selected.text,"Cafè words\njj   ",&area(rotation),&format(&f.id)).unwrap();
        let runs=e.list_text_runs(&edited.id,0).unwrap().runs;let after=runs.iter().find(|r|r.text=="Neighbor").unwrap();
        assert_eq!(after.bounds,neighbor.bounds);assert_eq!(after.font_name,neighbor.font_name);
        let indices:Vec<_>=runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect();
        let selected=e.inspect_text_paragraph(&edited.id,0,&indices).unwrap();
        assert_eq!(selected.alignment,Some(folio_engine::ParagraphAlignment::Right));assert!((selected.line_spacing.unwrap()-1.5).abs()<0.002);
        let pdf=Document::load_mem(&e.source_bytes(&edited.id).unwrap()).unwrap();
        assert!(pdf.objects.values().any(|o|o.as_stream().is_ok_and(|s|s.content==e.fonts().get(&f.id).unwrap().bytes.as_ref())));
        assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),original.as_slice());
        for id in [&source.id,&edited.id]{e.close_document(id).unwrap();}
    }
}

#[test]
fn bad_font_or_text_and_overflow_preserve_the_source() {
    let _guard=REGISTRY.lock().unwrap();
    let e=engine();let f=font(&e);let temp=tempfile::tempdir().unwrap();let input=temp.path().join("source.pdf");fixture(&input,0);
    let source=e.open_document(&input).unwrap();let bytes=e.source_bytes(&source.id).unwrap();let selected=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
    for (id,text,bounds) in [("unknown","New",area(0)),(f.id.as_str(),"New\u{0301}",area(0)),(f.id.as_str(),"New\ttext",area(0)),(f.id.as_str(),"Longword",AnnotationRect{x:60.,y:80.,width:5.,height:20.})] {
        assert!(e.preview_text_paragraph_with_format(&source.id,0,&[0],&selected.text,text,&bounds,&format(id)).is_err());
        assert_eq!(e.source_bytes(&source.id).unwrap(),bytes);
    }
    e.close_document(&source.id).unwrap();
}

#[test]
fn embedded_font_labels_survive_registry_release_without_changing_pdf_identity() {
    let _guard=REGISTRY.lock().unwrap();let e=engine();let f=font(&e);let temp=tempfile::tempdir().unwrap();let input=temp.path().join("source.pdf");fixture(&input,0);
    let source=e.open_document(&input).unwrap();let selected=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
    let edited=e.preview_text_paragraph_with_format(&source.id,0,&[0],&selected.text,"Readable font",&area(0),&format(&f.id)).unwrap();
    let identity=e.list_text_runs(&edited.id,0).unwrap().runs.into_iter().find(|r|r.text=="Readable font").unwrap().font_name;
    assert!(identity.starts_with("Folio"));e.fonts().remove(&f.id).unwrap();
    let saved=temp.path().join("saved.pdf");e.export_pdf(folio_engine::ExportRequest{flatten:false,pages:vec![folio_engine::PagePlan{crop: None,id:"p".into(),source_id:edited.id.clone(),page_index:0,width:edited.pages[0].width,height:edited.pages[0].height,rotation:0,overlays:vec![]}]},&saved).unwrap();
    let reopened=e.open_document(&saved).unwrap();let run=e.list_text_runs(&reopened.id,0).unwrap().runs.into_iter().find(|r|r.text=="Readable font").unwrap();assert_eq!(run.font_name,identity);
    assert_eq!(serde_json::to_value(&run).unwrap()["fontDisplayName"],"DejaVu Serif");
    let selection=e.inspect_text_paragraph(&reopened.id,0,&[run.object_index]).unwrap();assert_eq!(selection.font_name,identity);assert_eq!(serde_json::to_value(selection).unwrap()["fontDisplayName"],"DejaVu Serif");
    for id in [&source.id,&edited.id,&reopened.id]{e.close_document(id).unwrap();}
}
