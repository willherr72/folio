use folio_engine::{AnnotationRect, DocumentInfo, PagePlan, ParagraphFormatting, PdfEngine};
use lopdf::{dictionary, Document, Stream};
use std::path::{Path, PathBuf};

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()
}
fn fixture(path: &Path, rotation: i32) {
    let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
    let content=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 12 Tf 0.2 0.4 0.6 rg 60 400 Td (Original) Tj ET BT /F1 12 Tf 0.8 0.1 0.2 rg 60 250 Td (Neighbor) Tj ET".to_vec()));
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),500.into()],"Rotate"=>rotation,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
    pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());
    let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);pdf.save(path).unwrap();
}
fn area(rotation:i32)->AnnotationRect {
    let (x,y,width,height)=match rotation {90=>(280.,60.,140.,180.),180=>(160.,280.,180.,140.),270=>(80.,160.,140.,180.),_=>(60.,80.,180.,140.)};
    AnnotationRect{x,y,width,height}
}
fn plan(doc:&DocumentInfo)->PagePlan {PagePlan{crop: None,id:"p".into(),source_id:doc.id.clone(),page_index:0,width:doc.pages[0].width,height:doc.pages[0].height,rotation:0,overlays:vec![]}}
fn color(selection:&folio_engine::TextParagraphSelection)->serde_json::Value {serde_json::to_value(selection).unwrap()["color"].clone()}

#[test]
fn color_inspection_and_omission_preserve_original_rgb() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");fixture(&path,0);
    let source=e.open_document(&path).unwrap();let selected=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
    assert_eq!(color(&selected),"#336699");
    let edited=e.preview_text_paragraph(&source.id,0,&[0],&selected.text,"Changed",&area(0)).unwrap();
    let runs=e.list_text_runs(&edited.id,0).unwrap().runs;let index=runs.iter().find(|r|r.text=="Changed").unwrap().object_index;
    assert_eq!(color(&e.inspect_text_paragraph(&edited.id,0,&[index]).unwrap()),"#336699");
    for id in [&source.id,&edited.id]{e.close_document(id).unwrap();}
}

#[test]
fn color_composes_with_formatting_and_substitution_through_rotated_save_and_reedit() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    let font=e.fonts().register(std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
    for (rotation,chosen,substitute) in [(0,"#1a80ef",false),(90,"#FFFFFF",true),(180,"#000000",false),(270,"#a02bc3",true)] {
        let path=temp.path().join("source.pdf");fixture(&path,rotation);let original=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();
        let selected=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();let neighbor=e.inspect_text_paragraph(&source.id,0,&[1]).unwrap();
        let format:ParagraphFormatting=serde_json::from_value(serde_json::json!({"color":chosen,"fontSize":14,"alignment":"justify","lineSpacing":1.5,"fontId":if substitute {Some(&font.id)} else {None}})).unwrap();
        let draft=if substitute {"  jj Café words carry a paragraph\n  \n  More words  \n"} else {"  Alpha beta gamma delta epsilon\n  \n  More words  \n"};
        let edited=e.preview_text_paragraph_with_format(&source.id,0,&[0],&selected.text,draft,&area(rotation),&format).unwrap();
        let saved=temp.path().join("saved.pdf");e.export_pdf(folio_engine::ExportRequest{pages:vec![plan(&edited)],flatten:false},&saved).unwrap();
        let reopened=e.open_document(&saved).unwrap();let runs=e.list_text_runs(&reopened.id,0).unwrap().runs;
        let anchor=runs.iter().find(|r|r.text.contains("More words")).unwrap();
        let restored=e.suggest_text_paragraph(&reopened.id,0,anchor.object_index,&anchor.text).unwrap();
        assert_eq!(restored.text,draft);assert_eq!(color(&restored),chosen.to_uppercase());assert_eq!(restored.font_size,14.0);
        let n=runs.iter().find(|r|r.text=="Neighbor").unwrap();let after=e.inspect_text_paragraph(&reopened.id,0,&[n.object_index]).unwrap();
        assert_eq!(color(&after),color(&neighbor));assert_eq!(after.bounds,neighbor.bounds);
        let again=e.preview_text_paragraph(&reopened.id,0,&restored.object_indices,&restored.text,&restored.text,&restored.bounds).unwrap();
        assert_eq!(e.render_page_plan(plan(&reopened),800).unwrap(),e.render_page_plan(plan(&again),800).unwrap());
        assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),original.as_slice());assert_eq!(std::fs::read(&path).unwrap(),original);
        for id in [&source.id,&edited.id,&reopened.id,&again.id]{e.close_document(id).unwrap();}
    }
    e.fonts().remove(&font.id).unwrap();
}

#[test]
fn invalid_colors_fail_without_modifying_the_source() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");fixture(&path,0);
    let source=e.open_document(&path).unwrap();let original=e.source_bytes(&source.id).unwrap();let selected=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
    for value in ["", "red", "#fff", "#12345678", "#GG0000", "#１２３", " #000000"] {
        let format:ParagraphFormatting=serde_json::from_value(serde_json::json!({"color":value})).unwrap();
        let error=e.preview_text_paragraph_with_format(&source.id,0,&[0],&selected.text,"Changed",&area(0),&format).unwrap_err().to_string();
        assert!(error.contains("color"),"{error}");assert_eq!(e.source_bytes(&source.id).unwrap(),original);
    }
    assert_eq!(std::fs::read(&path).unwrap(),original.as_ref());e.close_document(&source.id).unwrap();
}
