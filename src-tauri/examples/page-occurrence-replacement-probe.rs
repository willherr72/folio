//! Throwaway feasibility probe: one source with separate occurrence pages.
use folio_engine::{DocumentInfo,ExportRequest,FillFormFieldRequest,FormFieldValue,PagePlan,PdfEngine};
use serde_json::json;
use std::{path::{Path,PathBuf},io::Write};

fn plans(source:&DocumentInfo,indices:&[usize])->Vec<PagePlan> {
    indices.iter().enumerate().map(|(i,p)|serde_json::from_value(json!({
        "id":format!("page-{i}"),"sourceId":source.id,"pageIndex":p,
        "width":source.pages[*p].width,"height":source.pages[*p].height,
        "rotation":0,"overlays":source.pages[*p].overlays
    })).unwrap()).collect()
}
fn write(path:&Path,bytes:&[u8])->Result<(),Box<dyn std::error::Error>> {
    std::fs::OpenOptions::new().write(true).create_new(true).open(path)?.write_all(bytes)?;Ok(())
}
fn main()->Result<(),Box<dyn std::error::Error>> {
    let root=PathBuf::from(std::env::args().nth(1).ok_or("Pass a fresh output directory")?);
    std::fs::create_dir(&root)?;
    let inputs=PathBuf::from("artifacts/page-replacement-navigation-20261024");
    let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))?;
    let original=engine.open_document(inputs.join("target.pdf"))?;
    let incoming=engine.open_document(inputs.join("incoming.pdf"))?;
    let expanded_path=root.join("expanded.pdf");
    engine.export_pdf(ExportRequest{pages:plans(&original,&[0,1,0]),flatten:false},&expanded_path)?;
    let expanded=engine.open_document(&expanded_path)?;
    for (new,old) in [(0,0),(1,1),(2,0)] {
        assert_eq!(engine.render_page(&expanded.id,new,600)?,engine.render_page(&original.id,old,600)?);
    }
    let mut results=Vec::new();
    for selected in [0,2] {
        let result=engine.replace_page_content(&expanded.id,selected,&incoming.id,0)?;
        let unchanged=2-selected;
        assert_eq!(engine.render_page(&result.id,unchanged,600)?,engine.render_page(&original.id,0,600)?);
        assert_eq!(engine.render_page(&result.id,1,600)?,engine.render_page(&original.id,1,600)?);
        assert!(engine.extract_text(&result.id,selected)?.contains(engine.extract_text(&incoming.id,0)?.trim()));
        let raw=engine.source_bytes(&result.id)?;write(&root.join(format!("selected-{selected}.pdf")),&raw)?;
        let field=engine.list_form_fields(&result.id,selected)?.remove(0);
        let filled=engine.fill_form_field(&result.id,FillFormFieldRequest{page_index:selected,annotation_index:field.annotation_index,expected_value:field.value,value:FormFieldValue::Text{text:"Shared after replacement".into()}})?;
        for page in 0..3 {assert_eq!(engine.list_form_fields(&filled.id,page)?[0].value,FormFieldValue::Text{text:"Shared after replacement".into()});}
        write(&root.join(format!("selected-{selected}-filled.pdf")),&engine.source_bytes(&filled.id)?)?;
        results.push(json!({"selected":selected,"otherCopyUnchanged":true,"secondPageUnchanged":true,"allSharedFieldsUpdated":true}));
        engine.close_document(&filled.id)?;engine.close_document(&result.id)?;
    }
    for source in [&expanded,&incoming,&original] {engine.close_document(&source.id)?;}
    write(&root.join("native.json"),&serde_json::to_vec_pretty(&results)?)?;
    println!("{}",json!({"passed":true,"cases":results.len(),"feasibilityOnly":true}));Ok(())
}
