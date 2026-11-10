//! Feasibility probe; records navigation after composing mixed source versions.
use folio_engine::{PdfEngine,DocumentInfo,PagePlan,ExportRequest};
use serde_json::json;
use std::{path::PathBuf,io::Write};

fn main()->Result<(),Box<dyn std::error::Error>> {
    let root=PathBuf::from(std::env::args().nth(1).ok_or("Pass an output directory")?);
    let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))?;
    let before=engine.open_document(root.join("target.pdf"))?;
    let incoming=engine.open_document(root.join("incoming.pdf"))?;
    let replacement=engine.replace_page_content(&before.id,0,&incoming.id,0)?;
    println!("{}",json!({"targetBookmarkError":before.bookmark_error,"replacementBookmarkError":replacement.bookmark_error,"bookmarks":before.bookmarks}));
    let bookmarks=folio_engine::bookmarks::remap(&before.bookmarks,&[("0".into(),"p0".into()),("1".into(),"p1".into())].into_iter().collect());
    let mut results=Vec::new();
    for (name,selection) in [
        ("baseline",[(&before,0),(&before,1),(&before,0)]),
        ("selected-only",[(&replacement,0),(&before,1),(&before,0)]),
        ("other-indices-migrated",[(&replacement,0),(&replacement,1),(&before,0)]),
        ("all-occurrences-control",[(&replacement,0),(&replacement,1),(&replacement,0)]),
    ] {
        let pages:Vec<PagePlan>=selection.iter().enumerate().map(|(index,(source,page))|{
            let source:&DocumentInfo=source;
            serde_json::from_value(json!({"id":format!("p{index}"),"sourceId":source.id,"pageIndex":page,
                "width":source.pages[*page].width,"height":source.pages[*page].height,"rotation":0,"overlays":[]})).unwrap()
        }).collect();
        let path=root.join(format!("workspace-{name}.pdf"));if path.exists(){return Err("Preserve existing evidence".into());}
        engine.export_pdf_with_bookmarks(ExportRequest{pages,flatten:false},Some(bookmarks.clone()),&path).map_err(|e|format!("{name}: {e}"))?;
        let pdf=lopdf::Document::load(&path)?;let ids:Vec<_>=pdf.get_pages().values().copied().collect();
        let mut links=Vec::new();
        for (page_index,id) in ids.iter().enumerate() {
            if let Ok(annots)=pdf.get_dictionary(*id)?.get(b"Annots") {
                let (_,annots)=pdf.dereference(annots)?;
                for value in annots.as_array()? {
                    let (_,value)=pdf.dereference(value)?;let annotation=value.as_dict()?;
                    if !annotation.get(b"Subtype").and_then(lopdf::Object::as_name).is_ok_and(|n|n==b"Link"){continue;}
                    let dest=annotation.get(b"Dest").ok().or_else(||annotation.get(b"A").ok().and_then(|a|pdf.dereference(a).ok()).and_then(|(_,a)|a.as_dict().ok()).and_then(|a|a.get(b"D").ok()));
                    if let Some(dest)=dest {let (_,dest)=pdf.dereference(dest)?;
                        let target=dest.as_array().ok().and_then(|a|a.first()).and_then(|p|p.as_reference().ok()).and_then(|id|ids.iter().position(|p|*p==id));
                        links.push(json!({"from":page_index,"to":target,"raw":format!("{dest:?}")}));
                    } else {links.push(json!({"from":page_index,"action":format!("{:?}",annotation.get(b"A").ok())}));}
                }
            }
        }
        let reopened=engine.open_document(&path)?;
        let fields=(0..3).map(|p|engine.list_form_fields(&reopened.id,p)).collect::<Result<Vec<_>,_>>()?;
        let texts=(0..3).map(|p|engine.extract_text(&reopened.id,p)).collect::<Result<Vec<_>,_>>()?;
        results.push(json!({"name":name,"links":links,"fields":fields,"text":texts}));engine.close_document(&reopened.id)?;
    }
    for info in [&replacement,&incoming,&before] {engine.close_document(&info.id)?;}
    std::fs::OpenOptions::new().write(true).create_new(true).open(root.join("native.json"))?.write_all(&serde_json::to_vec_pretty(&results)?)?;
    println!("{}",json!({"cases":results.len(),"feasibilityOnly":true}));Ok(())
}
