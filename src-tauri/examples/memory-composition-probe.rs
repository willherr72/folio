//! Explicit public-fixture probe for independent composition verification.
use folio_engine::{
    DocumentBookmark, ExportRequest, FillFormFieldRequest, FormFieldValue, PagePlan, PdfEngine,
};
use std::{fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: memory-composition-probe INPUT.pdf OUTPUT.pdf".into());
    }
    let input = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);
    if output.exists() {
        return Err("probe output must be a new file".into());
    }
    let engine = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )?;
    let original = fs::read(&input)?;
    let source = engine.open_document(&input)?;
    let field = engine
        .list_form_fields(&source.id, 0)?
        .into_iter()
        .find(|f| f.name == "full_name")
        .ok_or("public form fixture requires full_name")?;
    let edited = engine.fill_form_field(
        &source.id,
        FillFormFieldRequest {
            page_index: 0,
            annotation_index: field.annotation_index,
            expected_value: field.value,
            value: FormFieldValue::Text {
                text: "Memory composition".into(),
            },
        },
    )?;
    let mut pages: Vec<PagePlan> = edited
        .pages
        .iter()
        .enumerate()
        .map(|(i, p)| {
            serde_json::from_value(
                serde_json::json!({"id":format!("p{i}"),"sourceId":edited.id,"pageIndex":i,
            "width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays}),
            )
            .unwrap()
        })
        .collect();
    pages[0].overlays.push(serde_json::from_value(serde_json::json!({
        "type":"text","id":"memory-note","text":"Composed note","x":40,"y":720,"fontSize":14,"color":"#123456"
    }))?);
    let bookmarks: Vec<DocumentBookmark> = serde_json::from_value(serde_json::json!([
        {"id":"memory-bookmark","title":"Second page","level":0,"destination":{"kind":"array","value":[
            {"kind":"page","value":"p1"},{"kind":"name","value":[70,105,116]}
        ]}}
    ]))?;
    engine.export_pdf_with_bookmarks(
        ExportRequest {
            pages,
            flatten: false,
        },
        Some(bookmarks),
        &output,
    )?;
    if fs::read(input)? != original {
        return Err("probe changed original".into());
    }
    engine.close_document(&edited.id)?;
    engine.close_document(&source.id)?;
    println!("Composed public form, annotation and bookmark PDF; original unchanged.");
    Ok(())
}
