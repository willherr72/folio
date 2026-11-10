//! Explicit synthetic-fixture probe; never modifies its input images.
use folio_engine::{ImageOverlay,Overlay,PagePlan,PdfEngine,prepare_pdf_image};
use serde_json::json;
use std::{path::PathBuf,time::Instant};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let inputs:Vec<PathBuf>=std::env::args_os().skip(1).map(PathBuf::from).collect();
    if inputs.is_empty(){return Err("usage: image-preview-probe IMAGE...".into());}
    let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))?;
    let source=engine.create_blank_document(612.,792.)?;
    let base=PagePlan{crop: None,id:"probe-page".into(),source_id:source.id.clone(),page_index:0,width:612.,height:792.,rotation:0,overlays:vec![]};
    let mut blank_ms=vec![];
    for _ in 0..4{let begin=Instant::now();engine.render_page_plan(base.clone(),1200)?;blank_ms.push(begin.elapsed().as_secs_f64()*1000.);}
    let mut cases=vec![];
    for path in inputs{
        eprintln!("Profiling {}",path.display());
        let original=std::fs::read(&path)?;
        let begin=Instant::now();let owned=engine.import_image(&path)?;let import_ms=begin.elapsed().as_secs_f64()*1000.;
        let asset=engine.images().get(&owned.image.id)?;
        let mut preparation_ms=vec![];
        for _ in 0..2{let begin=Instant::now();let prepared=prepare_pdf_image(&asset)?;std::hint::black_box(prepared.form());preparation_ms.push(begin.elapsed().as_secs_f64()*1000.);}
        let mut renders=vec![];
        for width in [240,1200]{
            let mut durations=vec![];let mut png_bytes=vec![];
            for step in 0..4{
                let mut plan=base.clone();plan.overlays.push(Overlay::Image(ImageOverlay{id:"probe-image".into(),asset_id:owned.image.id.clone(),x:60.+step as f32*5.,y:80.,width:400.,height:400.*owned.image.height as f32/owned.image.width as f32,rotation:0}));
                let begin=Instant::now();let png=engine.render_page_plan(plan,width)?;durations.push(begin.elapsed().as_secs_f64()*1000.);png_bytes.push(png.len());
            }
            renders.push(json!({"targetWidth":width,"milliseconds":durations,"pngBytes":png_bytes}));
        }
        if std::fs::read(&path)?!=original{return Err("probe changed its original image".into());}
        cases.push(json!({"file":path.file_name().unwrap().to_string_lossy(),"image":owned.image,"importMilliseconds":import_ms,"prepareMilliseconds":preparation_ms,"renders":renders}));
        engine.images().release(&owned.token)?;
    }
    engine.close_document(&source.id)?;
    println!("{}",serde_json::to_string_pretty(&json!({"debugBuild":cfg!(debug_assertions),"blankPageMilliseconds":blank_ms,"cases":cases}))?);
    Ok(())
}
