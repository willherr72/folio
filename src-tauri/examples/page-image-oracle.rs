//! Separate reader of the saved PDF; does not use Folio composition or image-export helpers.
use pdfium_render::prelude::*;
use std::path::PathBuf;
fn main()->Result<(),Box<dyn std::error::Error>>{
 let root=PathBuf::from(std::env::args().nth(1).ok_or("corpus root required")?);
 let pdfium=Pdfium::new(Pdfium::bind_to_library(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))?);
 for line in std::fs::read_to_string(root.join("cases.jsonl"))?.lines(){
  let row:serde_json::Value=serde_json::from_str(line)?;let name=row["name"].as_str().ok_or("name")?;let dpi=row["dpi"].as_f64().ok_or("dpi")?;
  let source=root.join(format!("{name}.oracle.pdf"));let structural=lopdf::Document::load(&source)?;let pages=structural.get_pages();let raw=structural.get_dictionary(*pages.get(&1).ok_or("page")?)?;
  let unit=raw.get(b"UserUnit").ok().map(|v|v.as_float()).transpose()?.unwrap_or(1.)as f64;
  let doc=pdfium.load_pdf_from_file(&source,None)?;let page=doc.pages().get(0)?;
  let width=(page.width().value as f64*unit*dpi/72.).round().max(1.)as i32;let height=(page.height().value as f64*unit*dpi/72.).round().max(1.)as i32;
  let bitmap=page.render_with_config(&PdfRenderConfig::new().set_target_size(width,height).set_clear_color(PdfColor::WHITE).render_annotations(true).render_form_data(true).use_print_quality(false))?;
  let target=root.join(format!("{name}.pdfium-oracle.png"));assert!(!target.exists(),"Preserve prior oracle");bitmap.as_image()?.into_rgb8().save(target)?;
 }
 Ok(())
}
