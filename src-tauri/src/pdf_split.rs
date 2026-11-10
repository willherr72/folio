//! Contiguous, complete workspace partitions for editable PDF publication.
use crate::{DocumentBookmark,EngineResult,PagePlan};
use crate::page_image_export::bad;
use serde::Deserialize;
#[derive(Debug,Clone,Copy,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct SplitPdfPart{pub start:usize,pub end:usize}
#[derive(Debug,Clone,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct SplitPdfRequest{
 pub pages:Vec<PagePlan>,pub parts:Vec<SplitPdfPart>,
 #[serde(default)]pub bookmarks:Option<Vec<DocumentBookmark>>,
 #[serde(default)]pub metadata:Option<crate::DocumentMetadataEdit>,
}
impl SplitPdfRequest{
 pub(crate) fn validate(&self)->EngineResult<()>{
  if let Some(metadata)=&self.metadata{metadata.validate()?;}
  if self.pages.is_empty()||self.pages.len()>10000||self.parts.is_empty()||self.parts.len()>1000{return Err(bad("Splitting supports 1 to 10,000 pages and 1 to 1,000 PDF parts."));}
  let mut end=0;
  for part in &self.parts{if part.start!=end||part.end<=part.start||part.end>self.pages.len(){return Err(bad("Split parts must cover every workspace page exactly once, in order."));}end=part.end;}
  if end!=self.pages.len(){return Err(bad("Split parts must include the final workspace page."));}Ok(())
 }
}
