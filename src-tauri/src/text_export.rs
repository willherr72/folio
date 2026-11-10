//! Bounded UTF-8 export contracts and final-only publication.
use crate::{EngineError,EngineResult,PagePlan,PageText};
use serde::{Deserialize,Serialize};
use std::{io::Write,path::{Path,PathBuf}};
pub use crate::PageImageControl as TextExportControl;
pub(crate) const MAX_CHARACTERS:usize=250_000;
pub(crate) const MAX_BYTES:usize=32*1024*1024;
#[derive(Debug,Clone,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct TextExportRequest{pub pages:Vec<PagePlan>,pub include_form_values:bool}
impl TextExportRequest{pub(crate) fn validate(&self)->EngineResult<()>{if self.pages.is_empty()||self.pages.len()>10000{return Err(bad("Choose between 1 and 10,000 pages."));}Ok(())}}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct TextExportResult{pub path:PathBuf,pub page_count:usize,pub empty_pages:usize,pub skipped_fields:usize,pub bytes:usize}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct TextExportProgress{pub completed:usize,pub total:usize,pub stage:&'static str}
pub(crate) fn bad(value:impl std::fmt::Display)->EngineError{EngineError::InvalidRequest(format!("Text export: {value}"))}
pub(crate) fn append(output:&mut Vec<u8>,value:&str)->EngineResult<()>{
 if value.len()>MAX_BYTES.saturating_sub(output.len()){return Err(bad("Output exceeds the 32 MiB text limit."));}
 output.try_reserve(value.len()).map_err(bad)?;output.extend_from_slice(value.as_bytes());Ok(())
}
pub(crate) fn intersects(x:f32,y:f32,w:f32,h:f32,width:f32,height:f32)->bool{
 [x,y,w,h,width,height].iter().all(|n|n.is_finite())&&w>=0.&&h>=0.&&x<width&&y<height&&x+w>0.&&y+h>0.
}
/// Preserve extraction order; omitted glyph groups cannot join unrelated words.
pub(crate) fn page_text(text:&PageText,width:f32,height:f32)->EngineResult<String>{
 if text.characters.len()>MAX_CHARACTERS{return Err(bad("A page exceeds the 250,000-character limit."));}
 let mut output=String::new();let mut separators=String::new();let mut omitted=false;
 for c in &text.characters{
  if c.text.chars().all(char::is_whitespace){if !output.is_empty(){separators.push_str(&c.text);}continue;}
  if !intersects(c.x,c.y,c.width,c.height,width,height){omitted=true;continue;}
  if !output.is_empty(){if omitted{output.push('\n');}else{output.push_str(&separators);}}
  separators.clear();omitted=false;output.push_str(&c.text);
 }
 Ok(output.replace("\r\n","\n").replace('\r',"\n"))
}
pub(crate) fn publish(path:&Path,bytes:&[u8])->EngineResult<()>{
 if bytes.is_empty()||bytes.len()>MAX_BYTES{return Err(bad("Invalid output byte count."));}
 let parent=path.parent().ok_or_else(||bad("Destination has no parent."))?;
 let mut file=tempfile::Builder::new().prefix(".folio-text-").suffix(".txt").tempfile_in(parent).map_err(bad)?;
 file.write_all(bytes).map_err(bad)?;file.as_file().sync_all().map_err(bad)?;
 for attempt in 0..=10{
  match file.persist(path){Ok(_)=>return Ok(()),Err(error)=>{
   if !cfg!(windows)||!matches!(error.error.raw_os_error(),Some(5|32|33))||attempt==10{return Err(bad(error.error));}
   file=error.file;std::thread::sleep(std::time::Duration::from_millis(25));
  }}
 }unreachable!()
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn output_budget_is_checked_before_extension(){let mut bytes=vec![0;MAX_BYTES];assert!(append(&mut bytes,"x").is_err());assert_eq!(bytes.len(),MAX_BYTES);}
 #[test]fn crop_separates_retained_groups_and_normalizes_lines(){
  let chars=[("A",2.),(" ",8.),("hidden",30.),("B",4.),("\r",6.),("\n",6.),("C",4.)].into_iter().map(|(text,x)|crate::PdfTextCharacter{text:text.into(),x,y:2.,width:2.,height:2.}).collect();
  assert_eq!(page_text(&PageText{characters:chars,intrinsic_rotation:0},20.,20.).unwrap(),"A\nB\nC");
 }
}
