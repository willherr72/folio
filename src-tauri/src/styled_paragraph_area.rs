//! Measure initial areas and explicitly fit current drafts with actual reflow metrics.
use super::*;
use styled_paragraph_plan::ResolvedStyle;

fn clear_area(bounds:[f32;4],page:[f32;4],obstacles:&[[f32;4]])->bool{
 bounds.iter().all(|v|v.is_finite())&&bounds[0]>=page[0]&&bounds[1]>=page[1]&&bounds[2]<=page[2]&&bounds[3]<=page[3]
  &&bounds[0]<bounds[2]&&bounds[1]<bounds[3]
  &&!obstacles.iter().any(|other|adds_overlap(&[0.;4],&bounds,other))
}
fn aligned_bounds(mut trial:[f32;4],original:[f32;4],alignment:ParagraphAlignment)->[f32;4]{
 let width=trial[2]-trial[0];
 trial[0]=match alignment{ParagraphAlignment::Center=>(original[0]+original[2]-width)*0.5,ParagraphAlignment::Right=>original[2]-width,_=>original[0]};
 trial[2]=trial[0]+width;trial
}
#[cfg(test)]
fn fit_area(original:[f32;4],page:[f32;4],obstacles:&[[f32;4]],alignment:ParagraphAlignment,fits:impl Fn([f32;4])->bool)->Option<[f32;4]>{
 fit_anchored_area(original,page,obstacles,|trial|Some(aligned_bounds(trial,original,alignment)),fits).filter(|bounds|*bounds!=original)
}
fn fit_anchored_area(original:[f32;4],page:[f32;4],obstacles:&[[f32;4]],anchor:impl Fn([f32;4])->Option<[f32;4]>,fits:impl Fn([f32;4])->bool)->Option<[f32;4]>{
 if page.iter().any(|v|!v.is_finite())||obstacles.iter().any(|r|r.iter().any(|v|!v.is_finite())||r[2]<r[0]||r[3]<r[1]){return None;}
 // Search width and bottom independently of the projected rectangle's edges.
 // A right-aligned rectangle has a fixed right edge; using that edge as the
 // width-search variable would prevent convergence. The anchor can also adjust
 // top to preserve the first source baseline as the measured first line changes.
 let safe=|trial:[f32;2]|anchor([original[0],trial[1],original[0]+trial[0],original[3]])
  .filter(|bounds|clear_area(*bounds,page,obstacles));
 let initial=[original[2]-original[0],original[1]];
 let initial_bounds=safe(initial)?;if fits(initial_bounds){return Some(initial_bounds);}
 let safe_endpoint=|base:[f32;2],target:[f32;2],axis:usize|->Option<[f32;2]>{
  safe(base)?;
  if safe(target).is_some(){return Some(target);}
  let mut good=base;let mut bad=target[axis];
  for _ in 0..20{
   if (good[axis]-bad).abs()<0.01{break;}
   let middle=(good[axis]+bad)*0.5;let mut trial=target;trial[axis]=middle;
   if safe(trial).is_some(){good=trial;}else{bad=middle;}
  }
  Some(good)
 };
 let refine=|base:[f32;2],target:[f32;2],axis:usize|->Option<[f32;2]>{
  safe(base)?;let target=safe_endpoint(base,target,axis)?;
  if !safe(target).is_some_and(&fits){return None;}
  let mut bad=base[axis];let mut good=target[axis];
  for _ in 0..20{
   if (good-bad).abs()<0.01{break;}
   let middle=(bad+good)*0.5;let mut trial=target;trial[axis]=middle;
   if safe(trial).is_some_and(&fits){good=middle;}else{bad=middle;}
  }
  let mut result=target;result[axis]=good;
  safe(result).filter(|bounds|fits(*bounds)).map(|_|result)
 };
 // Widen, extend downward, then combine. Keep the widest safe endpoint even
 // when it still needs height. Every returned rectangle passes both checks;
 // mixed-height wrapping is not assumed to have globally monotone fit behavior.
 let wider=safe_endpoint(initial,[page[2]-page[0],initial[1]],0)?;
 if let Some(result)=refine(initial,wider,0){return safe(result);}
 if let Some(result)=refine(initial,[initial[0],page[1]],1){return safe(result);}
 let combined=refine(wider,[wider[0],page[1]],1)?;
 refine([initial[0],combined[1]],combined,0).and_then(safe)
}

impl WorkerRuntime {
 pub(in crate::engine) fn fit_styled_paragraph(&self,source_id:&str,page_index:usize,request:&StyledParagraphLayoutRequest)->EngineResult<AnnotationRect>{
  let measured=self.measure_styled_paragraph(source_id,page_index,request)?;let prepared=&measured.prepared;
  let geometry=prepared.geometry;let page=[geometry.left,geometry.bottom,geometry.right,geometry.top];
  let original=pdf_bounds(geometry,&request.bounds);let obstacles=self.styled_area_obstacles(source_id,page_index,prepared)?;
  if !clear_area(original,page,&obstacles){return Err(invalid("The current paragraph area overlaps neighboring content. Adjust the selection or position first."));}
  let fits=|bounds|measured.layout(request,bounds).is_ok();
  if fits(original){return Ok(request.bounds.clone());}
  // A clear obstacle endpoint can still cross the source clip. Admit clipping
  // here so the search can find a smaller valid endpoint before testing layout.
  let anchor=|trial|measured.anchored_fit_bounds(request,aligned_bounds(trial,original,request.alignment)).ok()
   .filter(|bounds|prepared.check_context_area(*bounds).is_ok());
  fit_anchored_area(original,page,&obstacles,anchor,fits).filter(|bounds|*bounds!=original).map(|bounds|displayed_bounds(geometry,bounds))
   .ok_or_else(||invalid("No clear area on this page can fit the paragraph at its current font sizes."))
 }
 pub(super) fn initial_styled_area(&self,source_id:&str,page_index:usize,prepared:&Prepared)->EngineResult<Option<AnnotationRect>>{
  if prepared.remembered{return Ok(None);}
  let styles=prepared.styles.iter().map(ResolvedStyle::original).collect::<Vec<_>>();
  let definitions=styles.iter().map(styled_paragraph_memory::Style::source).collect::<EngineResult<Vec<_>>>()?;
  let (probes,_)=self.styled_font_probes(&prepared.pdf,prepared.page_id,&definitions)?;
  let mut metrics=styled_paragraph_plan::measure(&probes,&prepared.runs,&styles)?;
  // Fresh marked full-font publication uses verified native ink. Measure the
  // same ink before anchoring and padding so that its later ascent agrees.
  if prepared.native_ink||styles.iter().any(|s|(prepared.ink_vertical||s.background_id.is_some())&&styled_paragraph_native_metrics::full_font(s)){
   self.native_highlight_metrics(prepared,&prepared.runs,&styles,&mut metrics,prepared.ink_vertical)?;
  }
  let ids=styles.iter().map(|s|s.background_id).collect::<Vec<_>>();
  let padded=if prepared.backgrounds.is_empty(){None}else{Some(paragraph_highlight_geometry::padded_metrics_in_units(&metrics,&ids,&prepared.backgrounds,prepared.unit)?)};
  let mut original=pdf_bounds(prepared.geometry,&prepared.selection.bounds);let horizontal_frame=original;
  let source_index=*prepared.selection.object_indices.first().ok_or_else(||invalid("The paragraph has no source anchor."))?;
  let source_baseline=prepared.before.iter().find(|run|run.index==source_index).ok_or_else(||invalid("The paragraph source anchor is missing."))?.matrix[5];
  let alignment=prepared.selection.alignment.unwrap_or(ParagraphAlignment::Left);
  let reservation=padded.as_deref().unwrap_or(&metrics);
  let text_metrics=if prepared.text_alignment{metrics.as_slice()}else{reservation};
  let anchor=|bounds:[f32;4]|{
   let mut bounds=aligned_bounds(bounds,horizontal_frame,alignment);
   let inner=paragraph_highlight_geometry::text_bounds(bounds,prepared.text_insets,prepared.text_alignment);
   let ascent=styled_paragraph_layout::first_line_ascent_with_text_metrics(&prepared.runs,reservation,text_metrics,inner[2]-inner[0],prepared.unit,prepared.ink_vertical).ok()?;
   bounds[3]=source_baseline+ascent;Some(bounds)
  };
  let spacing=prepared.selection.line_spacing.unwrap_or(1.2);
  let layout=|bounds|paragraph_highlight_geometry::layout_in_units(&prepared.runs,&metrics,padded.as_deref(),bounds,alignment,spacing,prepared.unit,prepared.text_insets,prepared.text_alignment,prepared.ink_vertical);
  let fits=|bounds|layout(bounds)
   .is_ok_and(|layout|prepared.backgrounds.is_empty()||paragraph_highlight_geometry::rectangles_in_units(&layout,&metrics,&ids,&prepared.backgrounds,prepared.unit).is_ok());
  let obstacles=self.styled_area_obstacles(source_id,page_index,prepared)?;
  let geometry=prepared.geometry;
  // A source run's provisional quarter-em bottom can be shorter than the actual
  // layout descent. Before widening an unchanged paragraph, include its measured
  // last row at the source width if that extra vertical space is clear. Do not
  // make a newly wrapped row fit before trying to retain the source line count.
  let mut source_rows=0;let mut last_baseline:Option<f32>=None;
  for index in &prepared.selection.object_indices{
   if let Some(run)=prepared.before.iter().find(|r|r.index==*index&&!r.text.trim().is_empty()){
    if last_baseline.is_none_or(|y|(y-run.matrix[5]).abs()>EPSILON){source_rows+=1;last_baseline=Some(run.matrix[5]);}
   }
  }
  if let Some(mut trial)=anchor(original){
   trial[1]=geometry.bottom;
   if let Ok(layout)=layout(trial){
    if let Some(last)=layout.lines.last().filter(|_|layout.lines.len()<=source_rows){
     let mut candidate=original;candidate[1]=candidate[1].min(last.baseline-last.descent);
     if anchor(candidate).is_some_and(|bounds|clear_area(bounds,[geometry.left,geometry.bottom,geometry.right,geometry.top],&obstacles)){original=candidate;}
    }
   }
  }
  Ok(fit_anchored_area(original,[geometry.left,geometry.bottom,geometry.right,geometry.top],&obstacles,anchor,fits)
   .filter(|bounds|*bounds!=horizontal_frame).map(|bounds|displayed_bounds(geometry,bounds)))
 }
 fn styled_area_obstacles(&self,source_id:&str,page_index:usize,prepared:&Prepared)->EngineResult<Vec<[f32;4]>>{
  let copy=self.editable_page_copy(source_id,page_index)?;let page=copy.pages().get(0)?;
  // This optional convenience must not add unbounded work to large pages.
  if page.objects().len()>10_000{return Err(invalid("This page has too many objects for automatic area fitting."));}
  let mut obstacles=prepared.before.iter().filter(|run|!prepared.selection.object_indices.contains(&run.index)&&!run.text.trim().is_empty()).map(|run|run.bounds).collect::<Vec<_>>();
  for (index,object) in page.objects().iter().enumerate(){
   if object.as_text_object().is_none(){
    // A source-bound mark contributes padding to the measured area. It is not
    // an obstacle to this optional suggestion; publication independently proves
    // its removal and the replacement background geometry.
    if prepared.highlight_sources.iter().any(|mark|mark.rectangle.object_index==index){continue;}
    if index<prepared.selection.object_indices[0]&&solid_page_background(&object,prepared.geometry)?{continue;}
    obstacles.extend(occupied_regions(&object,prepared.geometry)?);
   }
  }
  Ok(obstacles)
 }
}

#[cfg(test)]
mod tests {
 use super::*;
 #[test]
 fn centered_and_right_fits_keep_their_anchor_and_respect_both_sides(){
  let page=[0.,0.,100.,100.];let center=[40.,80.,60.,90.];
  let fitted=fit_area(center,page,&[],ParagraphAlignment::Center,|r|r[2]-r[0]>=40.&&r[3]-r[1]>=20.).unwrap();
  assert!(((fitted[0]+fitted[2])*0.5-50.).abs()<0.001);assert!((40.0..40.01).contains(&(fitted[2]-fitted[0])));assert!((20.0..20.01).contains(&(fitted[3]-fitted[1])));
  for obstacle in [[25.,0.,35.,100.],[65.,0.,75.,100.]]{
   assert!(fit_area(center,page,&[obstacle],ParagraphAlignment::Center,|r|r[2]-r[0]>=40.).is_none());
  }
  let right=[60.,80.,80.,90.];
  let fitted=fit_area(right,page,&[[85.,0.,95.,100.]],ParagraphAlignment::Right,|r|r[2]-r[0]>=40.).unwrap();
  assert_eq!(fitted[2],80.);assert!((40.0..40.01).contains(&(fitted[2]-fitted[0])));
  assert!(fit_area(right,page,&[[45.,0.,55.,100.]],ParagraphAlignment::Right,|r|r[2]-r[0]>=40.).is_none());
  assert!(fit_area([0.,80.,20.,90.],page,&[],ParagraphAlignment::Center,|r|r[2]-r[0]>=21.).is_none());
 }
 #[test]
 fn combined_fitting_retains_safe_width_before_extending_height(){
  use styled_paragraph_layout::{GlyphMetric,StyleMetrics,first_line_ascent,layout_in_units};
  let metrics=(1..=2).map(|scale|{let scale=scale as f32;
   StyleMetrics{em:10.*scale,ascent:8.*scale,descent:2.*scale,
    glyphs:"abcdef ".chars().map(|ch|(ch,GlyphMetric{
     advance:if ch==' '{3.*scale}else{5.*scale},
     ink:if ch==' '{[0.;4]}else{[0.,-2.*scale,5.*scale,8.*scale]},
    })).collect()}
  }).collect::<Vec<_>>();
  let runs=[("a ",0),("b ",1),("c d e f",0)].into_iter()
   .map(|(text,style_id)|StyledParagraphRun{text:text.into(),style_id}).collect::<Vec<_>>();
  let anchor=|mut bounds:[f32;4]|{
   bounds[3]=80.+first_line_ascent(&runs,&metrics,bounds[2]-bounds[0],1.).ok()?;Some(bounds)
  };
  let layout=|bounds|layout_in_units(&runs,&metrics,bounds,ParagraphAlignment::Left,1.2,1.);
  let page=[0.,10.,100.,90.];let original=[10.,70.,20.,88.];
  // The full-width first line contains the tall style and crosses the page top.
  assert!(!clear_area(anchor([10.,70.,100.,88.]).unwrap(),page,&[]));
  assert!(layout([10.,10.,20.,88.]).is_err());
  assert!(layout([10.,18.,27.,88.]).is_ok());
  for obstacles in [vec![],vec![[28.,10.,100.,95.]]]{
   let result=fit_anchored_area(original,page,&obstacles,anchor,|bounds|layout(bounds).is_ok())
    .expect("a safe widened area can fit after extending its height");
   assert!(clear_area(result,page,&obstacles));
   assert!(result[2]>original[2]&&result[1]<original[1]);
   assert_eq!(layout(result).unwrap().lines[0].baseline,80.);
  }
 }
 #[test]
 fn measured_extensions_keep_anchor_and_are_bounded_without_moving_obstacles(){
  let original=[10.,80.,20.,90.];let page=[0.,0.,100.,100.];
  let wider=fit_area(original,page,&[],ParagraphAlignment::Left,|r|r[2]-r[0]>=12.).unwrap();assert_eq!([wider[0],wider[1],wider[3]],[10.,80.,90.]);assert!((22.0..22.01).contains(&wider[2]));
  let taller=fit_area(original,page,&[[20.,70.,25.,95.]],ParagraphAlignment::Left,|r|r[3]-r[1]>=12.).unwrap();assert_eq!([taller[0],taller[2],taller[3]],[10.,20.,90.]);assert!((77.99..=78.).contains(&taller[1]));
  let both=fit_area(original,page,&[],ParagraphAlignment::Left,|r|r[2]-r[0]>=12.&&r[3]-r[1]>=12.).unwrap();assert!((22.0..22.01).contains(&both[2]));assert!((77.99..=78.).contains(&both[1]));
 }
 #[test]
 fn fitting_blocked_invalid_and_page_limited_areas_are_not_changed(){
  let original=[10.,80.,20.,90.];let page=[0.,0.,100.,100.];
  assert!(fit_area(original,page,&[],ParagraphAlignment::Left,|_|true).is_none());
  assert!(fit_area(original,page,&[[15.,85.,16.,86.]],ParagraphAlignment::Left,|_|false).is_none());
  assert!(fit_area(original,page,&[[f32::NAN,0.,10.,10.]],ParagraphAlignment::Left,|_|true).is_none());
  assert!(fit_area(original,original,&[],ParagraphAlignment::Left,|_|false).is_none());
 }
}
