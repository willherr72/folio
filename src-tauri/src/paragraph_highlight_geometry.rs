//! Source highlight padding, layout reservation and final native ink geometry.
use super::*;
use paragraph_highlights::BoundHighlight;
use styled_paragraph_layout::StyleMetrics;

#[derive(Clone,Debug,PartialEq,serde::Serialize,serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Background {pub color:[u8;3],pub padding:[f32;4],
 #[serde(default,skip_serializing_if="Option::is_none")]pub cell:Option<paragraph_highlight_cells::CellPolicy>}
#[derive(Clone,Debug,PartialEq)]
pub(super) struct PlacedBackground {pub background_id:usize,pub color:[u8;3],pub bounds:[f32;4]}
fn valid_box(r:[f32;4])->bool{r.iter().all(|v|v.is_finite()&&v.abs()<=1_000_000.)&&r[2]>r[0]&&r[3]>r[1]}
fn visible(glyph:&Glyph)->bool{!glyph.text.trim().is_empty()&&glyph.position[4]>glyph.position[2]&&glyph.position[5]>glyph.position[3]}
fn union(bounds:&mut Option<[f32;4]>,next:[f32;4]){
 if let Some(old)=bounds{old[0]=old[0].min(next[0]);old[1]=old[1].min(next[1]);old[2]=old[2].max(next[2]);old[3]=old[3].max(next[3]);}else{*bounds=Some(next);}
}
pub(super) fn valid_backgrounds(backgrounds:&[Background])->bool{backgrounds.len()<=64&&backgrounds.iter().all(|b|b.padding.iter().all(|p|p.is_finite()&&(0.0..=100_000.).contains(p))&&b.cell.is_none_or(|c|c.valid()&&b.padding==[0.;4]))}

/// Uniform frame reservation keeps a row's text anchor independent of which
/// words are marked. Insets are raw PDF units, as are verified source paddings.
pub(super) fn horizontal_insets(backgrounds:&[Background])->[f32;2]{
 backgrounds.iter().fold([0.;2],|p,b|[p[0].max(b.padding[0]),p[1].max(b.padding[2])])
}
pub(super) fn style_insets(backgrounds:&[Background],styles:impl IntoIterator<Item=(Option<usize>,f32)>)->EngineResult<[f32;2]>{
 let mut p=horizontal_insets(backgrounds);
 for (id,em) in styles{if let Some(id)=id{
  let b=backgrounds.get(id).ok_or_else(||invalid("Missing highlight inset style."))?;
  if let Some(c)=b.cell{if !c.valid()||!em.is_finite()||em<=0.{return Err(invalid("Invalid highlight inset size."));}
   p[0]=p[0].max(-c.offsets[0]*em);p[1]=p[1].max(c.offsets[2]*em);
  }
 }}
 if !valid_text_insets(p){return Err(invalid("Invalid highlight insets."));}Ok(p)
}
pub(super) fn valid_text_insets(insets:[f32;2])->bool{insets.iter().all(|v|v.is_finite()&&(0.0..=100_000.).contains(v))}
pub(super) fn text_bounds(mut bounds:[f32;4],insets:[f32;2],text_alignment:bool)->[f32;4]{
 if text_alignment{bounds[0]+=insets[0];bounds[2]-=insets[1];}bounds
}
pub(super) fn layout_in_units(runs:&[StyledParagraphRun],metrics:&[StyleMetrics],padded:Option<&[StyleMetrics]>,bounds:[f32;4],alignment:ParagraphAlignment,spacing:f32,unit:f32,insets:[f32;2],text_alignment:bool,ink_vertical:bool)->EngineResult<StyledParagraphLayout>{
 layout_with_baseline(runs,metrics,padded,bounds,alignment,spacing,unit,insets,text_alignment,ink_vertical,None)
}
pub(super) fn layout_with_baseline(runs:&[StyledParagraphRun],metrics:&[StyleMetrics],padded:Option<&[StyleMetrics]>,bounds:[f32;4],alignment:ParagraphAlignment,spacing:f32,unit:f32,insets:[f32;2],text_alignment:bool,ink_vertical:bool,first_baseline:Option<f32>)->EngineResult<StyledParagraphLayout>{
 let reservation=padded.unwrap_or(metrics);
 if !text_alignment{return styled_paragraph_layout::layout_with_metrics(runs,reservation,reservation,bounds,alignment,spacing,unit,false,false,first_baseline);}
 if !valid_text_insets(insets){return Err(invalid("Invalid paragraph text insets."));}
 let mut layout=styled_paragraph_layout::layout_with_metrics(runs,reservation,metrics,text_bounds(bounds,insets,true),alignment,spacing,unit,true,ink_vertical,first_baseline)?;
 // Publication and collision checks use the complete reserved frame.
 layout.pdf_bounds=bounds;Ok(layout)
}

/// Requires native/source-bound marks. Insets are physical PDF points; a font
/// substitution changes measured ink but does not silently rescale the inset.
/// A rectangle enclosing several baselines may be block shading, not a line
/// highlight. Keep that ambiguous graphic unchanged until block semantics exist.
pub(super) fn source_backgrounds(marks:&[BoundHighlight],glyphs:&HashMap<usize,Vec<Glyph>>,sources:&HashMap<usize,String>)->EngineResult<Vec<Background>>{
 if marks.len()>64{return Err(invalid("Too many source highlights."));}
 let mut indexed=HashMap::new();let mut result=Vec::new();let mut work=0;
 for mark in marks {
  if !valid_box(mark.rectangle.bounds){return Err(invalid("Invalid source highlight geometry."));}
  let mut ink=None;let mut baselines:Option<(f32,f32)>=None;
  for span in &mark.spans {
   if !indexed.contains_key(&span.object_index){
    let characters=glyphs.get(&span.object_index).ok_or_else(||invalid("Missing highlighted glyph owner."))?;
    let source=sources.get(&span.object_index).ok_or_else(||invalid("Missing highlighted source text."))?;
    if characters.len()>16_384||characters.iter().map(|g|g.text.as_str()).collect::<String>()!=*source{return Err(invalid("Highlighted glyphs do not match decoded source text."));}
    let mut offset=0;let entries=characters.iter().map(|glyph|{let start=offset;offset+=glyph.text.len();(start,offset,glyph)}).collect::<Vec<_>>();indexed.insert(span.object_index,entries);
   }
   let source=&sources[&span.object_index];if source.get(span.byte_start..span.byte_end).is_none_or(|s|s.is_empty()||s.trim()!=s){return Err(invalid("Invalid highlight source span."));}
   let entries=&indexed[&span.object_index];let start=entries.partition_point(|(_,end,_)|*end<=span.byte_start);
   let mut counted=false;
   for &(start,end,glyph) in entries[start..].iter().take_while(|(start,_,_)|*start<span.byte_end){
    work+=1;if work>16_384{return Err(invalid("Highlight geometry exceeds its glyph limit."));}
    if start<span.byte_start||end>span.byte_end||glyph.position.iter().any(|v|!v.is_finite())||glyph.position[4]<glyph.position[2]||glyph.position[5]<glyph.position[3]{return Err(invalid("Invalid highlighted glyph geometry."));}
    if !visible(glyph){continue;}
    let y=glyph.position[1];let range=baselines.map_or((y,y),|(low,high)|(low.min(y),high.max(y)));
    if range.1-range.0>EPSILON{return Err(invalid("A background spanning multiple text lines is not a line highlight."));}
    baselines=Some(range);counted=true;union(&mut ink,[glyph.position[2],glyph.position[3],glyph.position[4],glyph.position[5]]);
   }
   if !counted{return Err(invalid("A highlight span has no visible glyphs."));}
  }
  let ink=ink.ok_or_else(||invalid("A highlight has no visible glyphs."))?;let r=mark.rectangle.bounds;
  let background=Background{color:mark.rectangle.color,cell:mark.cell,padding:if mark.cell.is_some(){[0.;4]}else{[ink[0]-r[0],ink[1]-r[1],r[2]-ink[2],r[3]-ink[3]]}};
  if !valid_backgrounds(std::slice::from_ref(&background)){return Err(invalid("The source highlight does not fully enclose its text."));}
  result.push(background);
 }
 Ok(result)
}

fn validate(metrics:&[StyleMetrics],ids:&[Option<usize>],backgrounds:&[Background],unit:f32)->EngineResult<()>{
 if metrics.is_empty()||metrics.len()>64||ids.len()!=metrics.len()||!valid_backgrounds(backgrounds)||ids.iter().flatten().any(|id|*id>=backgrounds.len())
  ||metrics.iter().any(|s|!physical_units::paragraph_size(s.em,unit)||[s.ascent,s.descent].iter().any(|v|!v.is_finite()||*v<0.)){
  return Err(invalid("Invalid highlight layout styles."));
 }
 Ok(())
}
#[cfg(test)]
pub(super) fn padded_metrics(metrics:&[StyleMetrics],ids:&[Option<usize>],backgrounds:&[Background])->EngineResult<Vec<StyleMetrics>>{padded_metrics_in_units(metrics,ids,backgrounds,1.)}
pub(super) fn padded_metrics_in_units(metrics:&[StyleMetrics],ids:&[Option<usize>],backgrounds:&[Background],unit:f32)->EngineResult<Vec<StyleMetrics>>{
 validate(metrics,ids,backgrounds,unit)?;
 metrics.iter().zip(ids).map(|(style,id)|{
  let mut glyphs=style.glyphs.clone();
  for (&ch,metric) in &mut glyphs {
   if !metric.advance.is_finite()||metric.advance<0.||metric.ink.iter().any(|v|!v.is_finite())||metric.ink[2]<metric.ink[0]||metric.ink[3]<metric.ink[1]{return Err(invalid("Invalid highlight glyph metrics."));}
   if !ch.is_whitespace()&&metric.ink[2]>metric.ink[0]&&metric.ink[3]>metric.ink[1]{if let Some(id)=id{let b=&backgrounds[*id];let p=b.padding;
    let paint=if let Some(cell)=b.cell{cell.paint(0.,metric.advance,metric.ink,style.em)?}else{[metric.ink[0]-p[0],metric.ink[1]-p[1],metric.ink[2]+p[2],metric.ink[3]+p[3]]};
    metric.ink=[metric.ink[0].min(paint[0]),metric.ink[1].min(paint[1]),metric.ink[2].max(paint[2]),metric.ink[3].max(paint[3])];}}
  }
  Ok(StyleMetrics{em:style.em,ascent:style.ascent,descent:style.descent,glyphs})
 }).collect()
}

#[cfg(test)]
pub(super) fn rectangles(layout:&StyledParagraphLayout,metrics:&[StyleMetrics],ids:&[Option<usize>],backgrounds:&[Background])->EngineResult<Vec<PlacedBackground>>{rectangles_in_units(layout,metrics,ids,backgrounds,1.)}
pub(super) fn rectangles_in_units(layout:&StyledParagraphLayout,metrics:&[StyleMetrics],ids:&[Option<usize>],backgrounds:&[Background],unit:f32)->EngineResult<Vec<PlacedBackground>>{
 validate_layout(layout)?;
 validate(metrics,ids,backgrounds,unit)?;
 let mut fragments=Vec::new();
 for fragment in layout.lines.iter().flat_map(|l|&l.runs){
  let style=metrics.get(fragment.style_id).ok_or_else(||invalid("Unknown highlighted paragraph style."))?;
  let mut x=fragment.origin[0];let y=fragment.origin[1];let mut ink=Vec::new();
  if !x.is_finite()||!y.is_finite(){return Err(invalid("Invalid highlight origin."));}
  for ch in fragment.text.chars(){
   let metric=style.glyphs.get(&ch).ok_or_else(||invalid("Missing highlighted glyph metrics."))?;
   if !metric.advance.is_finite()||metric.advance<0.||metric.ink.iter().any(|v|!v.is_finite())||metric.ink[2]<metric.ink[0]||metric.ink[3]<metric.ink[1]{return Err(invalid("Invalid highlight glyph metrics."));}
   if !ch.is_whitespace()&&metric.ink[2]>metric.ink[0]&&metric.ink[3]>metric.ink[1]{let bounds=[x+metric.ink[0],y+metric.ink[1],x+metric.ink[2],y+metric.ink[3]];
    let paint=if let Some(cell)=ids[fragment.style_id].and_then(|id|backgrounds[id].cell){cell.paint(x,metric.advance,bounds,style.em)?}else{bounds};
    ink.push((bounds,paint));}
   x+=metric.advance;if !x.is_finite()||x.abs()>1_000_000.{return Err(invalid("Highlight cursor exceeds its limit."));}
  }
  fragments.push(ink);
 }
 rectangles_from_ink(layout,&fragments,ids,backgrounds)
}

/// Final geometry uses glyphs from reopened emitted text, after the caller has
/// verified fragment ownership, font, text state and origins. Font probe boxes
/// alone can differ from native glyph boxes by one ULP and change edge pixels.
pub(super) fn native_rectangles(layout:&StyledParagraphLayout,glyphs:&[Vec<Glyph>],ids:&[Option<usize>],backgrounds:&[Background])->EngineResult<Vec<PlacedBackground>>{
 native_rectangles_with_styles(layout,glyphs,ids,backgrounds,None)
}
pub(super) fn native_rectangles_with_styles(layout:&StyledParagraphLayout,glyphs:&[Vec<Glyph>],ids:&[Option<usize>],backgrounds:&[Background],styles:Option<&[styled_paragraph_plan::ResolvedStyle]>)->EngineResult<Vec<PlacedBackground>>{
 validate_layout(layout)?;
 let runs=layout.lines.iter().flat_map(|l|&l.runs).collect::<Vec<_>>();
 if runs.len()!=glyphs.len(){return Err(invalid("Missing saved highlighted fragments."));}
 let mut fragments=Vec::new();let mut count=0;
 for (run,glyphs) in runs.iter().zip(glyphs){
  count+=glyphs.len();if count>16_384{return Err(invalid("Saved highlighted glyphs exceed their limit."));}
  let extracted=glyphs.iter().map(|g|g.text.as_str()).collect::<String>();
  // PDFium can collapse or omit encoded ASCII spaces, including at a fragment
  // edge. The caller has already checked encoded bytes, native text/font/state
  // and every visible glyph origin. Spaces carry no painted ink here.
  if extracted.chars().filter(|c|*c!=' ').ne(run.text.chars().filter(|c|*c!=' ')){return Err(invalid("Saved highlighted glyphs do not match their fragment."));}
  let mut ink=Vec::new();
  for glyph in glyphs {
   let p=glyph.position;if p.iter().any(|v|!v.is_finite())||p[4]<p[2]||p[5]<p[3]{return Err(invalid("Invalid saved highlighted glyph geometry."));}
   if visible(glyph){
    let bounds=[p[2],p[3],p[4],p[5]];
    let id=*ids.get(run.style_id).ok_or_else(||invalid("Missing saved highlight style."))?;
    let cell=id.map(|id|backgrounds.get(id).ok_or_else(||invalid("Missing saved highlight background."))).transpose()?.and_then(|b|b.cell);
    let paint=if let Some(cell)=cell{
     let style=styles.and_then(|s|s.get(run.style_id)).ok_or_else(||invalid("Missing verified character-cell style."))?;
     let mut chars=glyph.text.chars();let ch=chars.next().ok_or_else(||invalid("Missing character-cell glyph."))?;
     if chars.next().is_some(){return Err(invalid("Ambiguous character-cell glyph."));}
     cell.paint(p[0],style.advance(ch)?,bounds,style.size)?
    }else{bounds};ink.push((bounds,paint));
   }
  }
  fragments.push(ink);
 }
 rectangles_from_ink(layout,&fragments,ids,backgrounds)
}

fn validate_layout(layout:&StyledParagraphLayout)->EngineResult<()>{
 if layout.lines.len()>256||!valid_box(layout.pdf_bounds){return Err(invalid("Highlight layout exceeds its limits."));}
 let mut bytes=0;let mut count=0;
 for fragment in layout.lines.iter().flat_map(|l|&l.runs){
  count+=1;bytes+=fragment.text.len();
  if count>4096||bytes>4096||fragment.origin.iter().any(|v|!v.is_finite()||v.abs()>1_000_000.){return Err(invalid("Highlight layout exceeds its limits."));}
 }
 Ok(())
}

fn rectangles_from_ink(layout:&StyledParagraphLayout,fragments:&[Vec<([f32;4],[f32;4])>],ids:&[Option<usize>],backgrounds:&[Background])->EngineResult<Vec<PlacedBackground>>{
 if ids.is_empty()||ids.len()>64||!valid_backgrounds(backgrounds)||ids.iter().flatten().any(|id|*id>=backgrounds.len()){return Err(invalid("Invalid highlight layout styles."));}
 let mut result=Vec::new();let mut all_ink=Vec::new();let mut fragments=fragments.iter();
 let flush=|id:Option<usize>,ink:Option<[f32;4]>,result:&mut Vec<PlacedBackground>|->EngineResult<()>{
  if let (Some(background_id),Some(ink))=(id,ink){let b=&backgrounds[background_id];let p=b.padding;let bounds=[ink[0]-p[0],ink[1]-p[1],ink[2]+p[2],ink[3]+p[3]];let area=layout.pdf_bounds;
   if !valid_box(bounds)||bounds[0]<area[0]-EPSILON||bounds[1]<area[1]-EPSILON||bounds[2]>area[2]+EPSILON||bounds[3]>area[3]+EPSILON{return Err(invalid("The highlight does not fit inside the paragraph area."));}
   result.push(PlacedBackground{background_id,color:b.color,bounds});
  }Ok(())
 };
 for line in &layout.lines {
  let mut current=None;let mut ink=None;
  for fragment in &line.runs {
   let id=*ids.get(fragment.style_id).ok_or_else(||invalid("Unknown highlighted paragraph style."))?;
   if id!=current{flush(current,ink.take(),&mut result)?;current=id;}
   for &(bounds,paint) in fragments.next().ok_or_else(||invalid("Missing highlight ink."))?{
    if !valid_box(bounds){return Err(invalid("Invalid highlighted ink geometry."));}
    all_ink.push((id,bounds));if id.is_some(){union(&mut ink,paint);}
   }
  }
  flush(current,ink,&mut result)?;
 }
 for (i,mark) in result.iter().enumerate(){
  if result[..i].iter().any(|other|adds_overlap(&[0.;4],&mark.bounds,&other.bounds))
   ||all_ink.iter().any(|(id,ink)|*id!=Some(mark.background_id)&&adds_overlap(&[0.;4],&mark.bounds,ink)){
   return Err(invalid("Reflow would paint a highlight over other text or another highlight."));
  }
 }
 Ok(result)
}

/// Emit isolated opaque RGB rectangles. Callers must establish source ownership,
/// graphics-state admission and collision checks before installing these operations.
pub(super) fn paint_operations(rectangles:&[PlacedBackground])->EngineResult<Vec<Operation>>{
 if rectangles.len()>4096||rectangles.iter().any(|r|r.background_id>=64||!valid_box(r.bounds)){
  return Err(invalid("Invalid highlight paint geometry."));
 }
 let mut operations=Vec::with_capacity(rectangles.len()*5);
 for rectangle in rectangles {
  let r=rectangle.bounds;
  operations.push(Operation::new("q",vec![]));
  operations.push(Operation::new("rg",rectangle.color.iter().map(|c|Object::Real(*c as f32/255.)).collect()));
  operations.push(Operation::new("re",vec![Object::Real(r[0]),Object::Real(r[1]),Object::Real(r[2]-r[0]),Object::Real(r[3]-r[1])]));
  operations.push(Operation::new("f",vec![]));
  operations.push(Operation::new("Q",vec![]));
 }
 Ok(operations)
}

#[cfg(test)]
#[path = "paragraph_highlight_geometry_tests.rs"]
mod tests;
