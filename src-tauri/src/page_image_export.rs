//! Exact-resolution image export contracts; rendering stays on the PDF worker.
use crate::{EngineError,EngineResult,PagePlan};
use serde::{Deserialize,Serialize};
use std::{io::{self,Write},path::PathBuf,sync::atomic::{AtomicU8,Ordering}};
use image::ImageEncoder;

pub(crate) const IMAGE_BYTES:usize=128*1024*1024;
pub(crate) const BATCH_BYTES:usize=1024*1024*1024;
#[derive(Debug,Clone,Copy,Deserialize,Serialize,PartialEq)]
#[serde(rename_all="lowercase")]
pub enum PageImageFormat{Png,Jpeg}
impl PageImageFormat{pub(crate) fn extension(self)->&'static str{match self{Self::Png=>"png",Self::Jpeg=>"jpg"}}}
#[derive(Debug,Clone,Copy,Deserialize,Serialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct PageImageOptions{pub format:PageImageFormat,pub dpi:u32,pub jpeg_quality:u8}
impl PageImageOptions{
 pub fn validate(self)->EngineResult<()>{
  if ![72,150,300,600].contains(&self.dpi)||!(1..=100).contains(&self.jpeg_quality){return Err(bad("Choose 72, 150, 300 or 600 dpi and JPEG quality from 1 to 100."));}Ok(())
 }
 pub fn dimensions(self,width:f64,height:f64)->EngineResult<(u32,u32)>{
  self.validate()?;
  if !width.is_finite()||!height.is_finite()||width<=0.||height<=0.{return Err(bad("Image export page dimensions are invalid."));}
  let w=(width*self.dpi as f64/72.).round().max(1.);let h=(height*self.dpi as f64/72.).round().max(1.);
  if w>12000.||h>12000.||w*h>24_000_000.{return Err(bad("This page exceeds the image size limit at the selected resolution. Choose a lower DPI or a smaller crop."));}Ok((w as u32,h as u32))
 }
}
#[derive(Debug,Clone,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct PageImageExportRequest{pub pages:Vec<PagePlan>,pub page_numbers:Vec<u32>,pub workspace_page_count:u32,pub options:PageImageOptions}
impl PageImageExportRequest{
 pub(crate) fn validate(&self)->EngineResult<()>{
  self.options.validate()?;
  if self.pages.is_empty()||self.pages.len()>10000||self.pages.len()!=self.page_numbers.len()||!(1..=10000).contains(&self.workspace_page_count)||self.page_numbers.iter().any(|n|*n==0||*n>self.workspace_page_count)||self.page_numbers.windows(2).any(|w|w[0]>=w[1]){return Err(bad("Image export requires distinct selected pages in workspace order."));}Ok(())
 }
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct PageImageExportResult{pub folder:PathBuf,pub count:usize}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct PageImageProgress{pub completed:usize,pub total:usize,pub stage:&'static str}
/// Cancellation and completion share one atomic decision; late cancel cannot erase output.
#[derive(Default)]
pub struct PageImageControl{state:AtomicU8}
impl PageImageControl{
 pub fn new()->Self{Self::default()}
 pub fn cancel(&self)->bool{self.state.compare_exchange(0,1,Ordering::AcqRel,Ordering::Acquire).is_ok()||self.cancelled()}
 pub fn cancelled(&self)->bool{self.state.load(Ordering::Acquire)==1}
 pub(crate) fn commit(&self)->bool{self.state.compare_exchange(0,2,Ordering::AcqRel,Ordering::Acquire).is_ok()}
 pub(crate) fn completed(&self){self.state.store(3,Ordering::Release);}
}
pub(crate) fn bad(message:impl Into<String>)->EngineError{EngineError::InvalidRequest(message.into())}
pub(crate) fn io_error(error:impl std::fmt::Display)->EngineError{EngineError::Io(error.to_string())}
struct BoundedImageWriter<'a>{bytes:Vec<u8>,limit:usize,control:&'a PageImageControl}
impl Write for BoundedImageWriter<'_>{
 fn write(&mut self,bytes:&[u8])->io::Result<usize>{
  if self.control.cancelled(){return Err(io::Error::other("Image export cancelled"));}
  if bytes.len()>self.limit.saturating_sub(self.bytes.len()){return Err(io::Error::other("Image export exceeds its encoded byte limit"));}
  let needed=self.bytes.len()+bytes.len();
  if needed>self.bytes.capacity(){let capacity=needed.max(self.bytes.capacity().saturating_mul(2)).min(self.limit);self.bytes.try_reserve_exact(capacity-self.bytes.len()).map_err(|_|io::Error::other("Image export allocation failed"))?;}
  self.bytes.extend_from_slice(bytes);Ok(bytes.len())
 }
 fn flush(&mut self)->io::Result<()>{Ok(())}
}
pub(crate) fn encode(image:&image::RgbImage,options:PageImageOptions,remaining:usize,control:&PageImageControl)->EngineResult<Vec<u8>>{
 let limit=remaining.min(IMAGE_BYTES);let metadata=if options.format==PageImageFormat::Png{21}else{0};
 let mut writer=BoundedImageWriter{bytes:Vec::new(),limit:limit.saturating_sub(metadata),control};
 match options.format{
  PageImageFormat::Png=>image::codecs::png::PngEncoder::new(&mut writer).write_image(image.as_raw(),image.width(),image.height(),image::ExtendedColorType::Rgb8).map_err(io_error)?,
  PageImageFormat::Jpeg=>{let mut encoder=image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer,options.jpeg_quality);encoder.set_pixel_density(image::codecs::jpeg::PixelDensity::dpi(options.dpi as u16));encoder.encode(image.as_raw(),image.width(),image.height(),image::ExtendedColorType::Rgb8).map_err(io_error)?;}
 }
 if options.format==PageImageFormat::Png{
  // The owned encoder writes signature + IHDR first. Add standard pixels-per-metre metadata.
  let density=(options.dpi as f64/0.0254).round()as u32;let mut chunk=Vec::with_capacity(21);
  chunk.extend_from_slice(&9u32.to_be_bytes());chunk.extend_from_slice(b"pHYs");chunk.extend_from_slice(&density.to_be_bytes());chunk.extend_from_slice(&density.to_be_bytes());chunk.push(1);let crc=crc32fast::hash(&chunk[4..]);chunk.extend_from_slice(&crc.to_be_bytes());
  if writer.bytes.len()<33||&writer.bytes[12..16]!=b"IHDR"{return Err(bad("PNG encoder produced an invalid header."));}
  writer.bytes.try_reserve_exact(21).map_err(io_error)?;writer.bytes.splice(33..33,chunk);
 }
 if writer.bytes.len()>limit{return Err(bad("Image export exceeds its encoded byte limit."));}Ok(writer.bytes)
}

#[cfg(test)]
mod tests{
 use super::*;
 #[test]fn cancellation_has_a_single_commit_boundary(){let c=PageImageControl::new();assert!(c.cancel());assert!(!c.commit());let c=PageImageControl::new();assert!(c.commit());assert!(!c.cancel());c.completed();assert!(!c.cancel());}
 #[test]fn byte_budgets_are_enforced_during_encoding(){let image=image::RgbImage::from_pixel(8,8,image::Rgb([255,255,255]));for format in [PageImageFormat::Png,PageImageFormat::Jpeg]{assert!(encode(&image,PageImageOptions{format,dpi:150,jpeg_quality:90},8,&PageImageControl::new()).is_err());}}
 #[test]fn dimensions_reject_invalid_domains_and_round_each_axis(){let o=PageImageOptions{format:PageImageFormat::Png,dpi:150,jpeg_quality:90};assert_eq!(o.dimensions(10.8,2.64).unwrap(),(23,6));assert_eq!(o.dimensions(0.01,0.01).unwrap(),(1,1));for(w,h)in[(f64::NAN,1.),(1.,0.),(100000.,1.),(5000.,5000.)]{assert!(o.dimensions(w,h).is_err());}assert!(PageImageOptions{jpeg_quality:0,..o}.validate().is_err());assert!(PageImageOptions{dpi:151,..o}.validate().is_err());}
}
