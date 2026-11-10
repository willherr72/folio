//! Candidate discovery only. This does not authorize removing a source graphic.
//! Publication must bind paint operators and all covered text before use, and
//! continue applying the existing page preflight and other-object collision gate.
use super::*;

const MAX_RECTANGLES:usize=64;
const MAX_GLYPHS:usize=16_384;
const MAX_COVERED_GLYPHS:usize=4096;

#[derive(Clone,Debug,PartialEq)]
pub(super) struct Rectangle {
 pub object_index:usize,
 pub bounds:[f32;4],
 pub color:[u8;3],
}
#[derive(Clone,Debug,PartialEq)]
pub(super) struct Coverage {
 pub object_index:usize,
 /// Indices into the verified native owner's glyph list, not logical offsets.
 pub glyph_indices:Vec<usize>,
 pub whole_visible_run:bool,
}
#[derive(Clone,Debug,PartialEq)]
pub(super) struct Candidate {pub rectangle:Rectangle,pub coverage:Vec<Coverage>}

#[derive(Clone,Debug,PartialEq)]
pub(super) struct SourceSpan {pub object_index:usize,pub byte_start:usize,pub byte_end:usize}

/// Internal source evidence, never accepted from a request or saved metadata.
/// The rectangle is kept separately from the palette so equal colors do not
/// erase original paint ownership, geometry, or padding.
#[derive(Clone,Debug)]
pub(super) struct BoundHighlight {
 pub rectangle:Rectangle,
 pub paint_operator:usize,
 pub cell:Option<paragraph_highlight_cells::CellPolicy>,
 pub spans:Vec<SourceSpan>,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub(super) struct AttributedStyle {
 pub source_style_id:usize,
 pub background_id:Option<usize>,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub(super) struct LogicalRange {pub start:usize,pub end:usize,pub background_id:usize}
pub(super) struct ParagraphAttribution {pub styles:Vec<AttributedStyle>,pub runs:Vec<StyledParagraphRun>}

/// Project source bytes using the builder's ownership map, never substring
/// search. This map is internal preparation evidence, not saved-hint authority.
fn project_ranges(logical:&str,mappings:&[paragraph_spans::SourceRange],sources:&HashMap<usize,String>,highlights:&[BoundHighlight])->EngineResult<Vec<LogicalRange>>{
 if logical.len()>4096||mappings.len()>4096||highlights.len()>MAX_RECTANGLES{return Err(invalid("The highlighted paragraph exceeds its source limits."));}
 let mut owners=HashMap::new();let mut previous_end=0;let mut previous_owner=None;
 for mapping in mappings {
  let source=sources.get(&mapping.object_index).ok_or_else(||invalid("Missing highlighted source owner."))?;
  let text=source.get(mapping.source_start..mapping.source_end).filter(|s|!s.is_empty()).ok_or_else(||invalid("Invalid paragraph source mapping."))?;
  let end=mapping.logical_start.checked_add(text.len()).ok_or_else(||invalid("Invalid paragraph source mapping."))?;
  if mapping.logical_start<previous_end||previous_owner.is_some_and(|id|id>=mapping.object_index)||logical.get(mapping.logical_start..end)!=Some(text){return Err(invalid("Ambiguous paragraph source mapping."));}
  previous_end=end;previous_owner=Some(mapping.object_index);owners.insert(mapping.object_index,mapping);
 }
 let mut result=Vec::new();let mut objects=HashSet::new();let mut operators=HashSet::new();
 for (background_id,highlight) in highlights.iter().enumerate(){
  if !objects.insert(highlight.rectangle.object_index)||!operators.insert(highlight.paint_operator)||highlight.spans.is_empty(){return Err(invalid("Ambiguous source highlight ownership."));}
  for span in &highlight.spans {
   if result.len()==MAX_COVERED_GLYPHS{return Err(invalid("The highlighted text exceeds its range limit."));}
   let owner=owners.get(&span.object_index).ok_or_else(||invalid("The highlighted text is missing from the paragraph."))?;
   let source=sources[&span.object_index].get(span.byte_start..span.byte_end).filter(|s|!s.is_empty()&&s.trim()==*s).ok_or_else(||invalid("Invalid highlight source range."))?;
   if span.byte_start<owner.source_start||span.byte_end>owner.source_end{return Err(invalid("Highlighted characters were removed during paragraph assembly."));}
   let start=owner.logical_start+span.byte_start-owner.source_start;
   result.push(LogicalRange{start,end:start+source.len(),background_id});
  }
 }
 result.sort_by_key(|span|(span.start,span.end));
 if result.windows(2).any(|w|w[0].end>w[1].start){return Err(invalid("Overlapping highlight source ranges."));}
 let mut joined:Vec<LogicalRange>=Vec::new();
 for span in result {
  if let Some(previous)=joined.last_mut().filter(|p|p.background_id==span.background_id&&logical[p.end..span.start].chars().all(char::is_whitespace)){previous.end=span.end;}
  else{joined.push(span);}
 }
 Ok(joined)
}

/// Build the final active palette after source trimming and logical joins. The
/// immutable original font ID is retained even when two font labels are equal.
fn attribute_runs(runs:&[StyledParagraphRun],logical:&str,ranges:&[LogicalRange])->EngineResult<ParagraphAttribution>{
 if logical.len()>4096||runs.len()>4096||runs.iter().any(|r|r.style_id>=64)||runs.iter().map(|r|r.text.as_str()).collect::<String>()!=logical
  ||ranges.len()>MAX_COVERED_GLYPHS||ranges.windows(2).any(|w|w[0].end>w[1].start)
  ||ranges.iter().any(|r|r.background_id>=MAX_RECTANGLES||r.start>=r.end||logical.get(r.start..r.end).is_none()){
  return Err(invalid("Invalid attributed paragraph ranges."));
 }
 let mut result=ParagraphAttribution{styles:Vec::new(),runs:Vec::new()};let mut offset=0;let mut mark=0;
 for run in runs {for ch in run.text.chars(){
  while mark<ranges.len()&&ranges[mark].end<=offset{mark+=1;}
  let background_id=ranges.get(mark).filter(|r|r.start<=offset&&offset<r.end).map(|r|r.background_id);
  let style=AttributedStyle{source_style_id:run.style_id,background_id};
  let style_id=if let Some(id)=result.styles.iter().position(|s|*s==style){id}else{
   if result.styles.len()==64{return Err(invalid("Use at most 64 paragraph styles, including highlighted styles."));}
   result.styles.push(style);result.styles.len()-1
  };
  if let Some(last)=result.runs.last_mut().filter(|r|r.style_id==style_id){last.text.push(ch);}
  else{result.runs.push(StyledParagraphRun{text:ch.to_string(),style_id});}
  offset+=ch.len_utf8();
 }}
 Ok(result)
}

/// Source attribution requires the existing preflight, native page-copy
/// equivalence and publication gates. Saved backgrounds use independent replay.
pub(super) fn bind_selection(prepared:&Prepared,page:&PdfPage<'_>)->EngineResult<(Vec<BoundHighlight>,ParagraphAttribution)>{
 if prepared.remembered{return Err(invalid("Saved highlights require verified background memory."));}
 let glyphs=page_object_glyphs(page)?;
 let rectangles=source_rectangles(page,prepared.geometry)?;
 let mut candidates=classify(rectangles.clone(),&glyphs)?.into_iter().map(|c|(c,None)).collect::<Vec<_>>();
 for (i,mark) in rectangles.iter().enumerate(){
  if rectangles.iter().enumerate().any(|(j,other)|i!=j&&overlaps(mark.bounds,other.bounds)){continue;}
  let existing=candidates.iter().position(|(c,_)|c.rectangle.object_index==mark.object_index);
  if let Some(index)=existing{
   // Discovery tolerates tiny native edge differences; legacy padding cannot
   // encode negative enclosure. Preserve genuinely enclosing legacy marks,
   // but give tolerance-only candidates the independent whole-source proof.
   let r=mark.bounds;
   let enclosed=candidates[index].0.coverage.iter().all(|owner|owner.glyph_indices.iter().all(|&i|{
    let p=glyphs[&owner.object_index][i].position;p[2]>=r[0]&&p[3]>=r[1]&&p[4]<=r[2]&&p[5]<=r[3]
   }));
   if enclosed{continue;}
  }
  if let Some((coverage,cell))=paragraph_highlight_cells::infer(prepared,mark,&glyphs)?{
   let candidate=(Candidate{rectangle:mark.clone(),coverage:vec![coverage]},Some(cell));
   if let Some(index)=existing{candidates[index]=candidate;}else{candidates.push(candidate);}
  }
 }
 candidates.sort_by_key(|(c,_)|c.rectangle.object_index);
 let selected=&prepared.selection.object_indices;
 let mut backgrounds=Vec::new();
 for (candidate,cell) in candidates {
  if !candidate.coverage.iter().any(|c|selected.contains(&c.object_index)){continue;}
  let spans=logical_ranges(&candidate.coverage,&glyphs,&prepared.source_text,selected)?;
  let paint_operator=bind_paint_operator(&candidate.rectangle,&prepared.content,page)?;
  backgrounds.push(BoundHighlight{rectangle:candidate.rectangle,paint_operator,cell,spans});
 }
 let projected=project_ranges(&prepared.selection.text,&prepared.source_ranges,&prepared.source_text,&backgrounds)?;
 let attribution=attribute_runs(&prepared.runs,&prepared.selection.text,&projected)?;
 Ok((backgrounds,attribution))
}

/// Install only complete native-proven attribution. Errors leave the original
/// palette and bounds intact; publication still has its separate graphics gate.
pub(super) fn install_source_backgrounds(prepared:&mut Prepared,page:&PdfPage<'_>)->EngineResult<()>{
 let (marks,attribution)=bind_selection(prepared,page)?;
 if marks.is_empty(){return Ok(());}
 let backgrounds=paragraph_highlight_geometry::source_backgrounds(&marks,&prepared.glyphs,&prepared.source_text)?;
 let styles=attribution.styles.iter().map(|entry|{
  let mut style=prepared.styles.get(entry.source_style_id).ok_or_else(||invalid("Missing original highlighted font style."))?.clone();
  style.background_id=entry.background_id;Ok(style)
 }).collect::<EngineResult<Vec<_>>>()?;
 let mut bounds=pdf_bounds(prepared.geometry,&prepared.selection.bounds);let text_frame=bounds;
 for mark in &marks{let r=mark.rectangle.bounds;bounds=[bounds[0].min(r[0]),bounds[1].min(r[1]),bounds[2].max(r[2]),bounds[3].max(r[3])];}
 if prepared.text_alignment{
  let p=paragraph_highlight_geometry::style_insets(&backgrounds,styles.iter().map(|s|(s.background_id,s.run.font_size*s.run.matrix[0])))?;
  bounds[0]=text_frame[0]-p[0];bounds[2]=text_frame[2]+p[1];
  prepared.text_insets=p;
 }
 prepared.selection.bounds=displayed_bounds(prepared.geometry,bounds);
 prepared.styles=styles;prepared.runs=attribution.runs;prepared.backgrounds=backgrounds;prepared.highlight_sources=marks;
 Ok(())
}

/// Byte ranges belong to decoded source strings, never PDFium extraction guesses.
/// UTF-16 editor positions are derived later from the resulting attributed text.
pub(super) fn logical_ranges(covered:&[Coverage],glyphs:&HashMap<usize,Vec<Glyph>>,source:&HashMap<usize,String>,selected:&[usize])->EngineResult<Vec<SourceSpan>>{
 let mut spans=Vec::new();
 for coverage in covered {
  let id=coverage.object_index;
  if !selected.contains(&id){return Err(invalid("Select all text covered by the highlight."));}
  let characters=glyphs.get(&id).ok_or_else(||invalid("Missing highlight glyph owner."))?;
  let logical=source.get(&id).ok_or_else(||invalid("Missing decoded highlighted text."))?;
  if logical.len()>4096||characters.iter().map(|g|g.text.as_str()).collect::<String>()!=*logical{return Err(invalid("Highlighted glyphs do not map exactly to the source text."));}
  if coverage.glyph_indices.is_empty()||coverage.glyph_indices.windows(2).any(|pair|pair[0]>=pair[1])||coverage.glyph_indices.iter().any(|i|*i>=characters.len()){return Err(invalid("Invalid highlight glyph range."));}
  let mut start=None;let mut end=0;let mut offset=0;
  for (index,glyph) in characters.iter().enumerate(){
   let next=offset+glyph.text.len();
   if coverage.glyph_indices.binary_search(&index).is_ok(){
    if glyph.text.is_empty()||glyph.text.chars().all(char::is_whitespace){return Err(invalid("Highlight coverage must identify visible characters."));}
    if start.is_none(){start=Some(offset);}end=next;
   }else if !glyph.text.chars().all(char::is_whitespace){
    if let Some(byte_start)=start.take(){spans.push(SourceSpan{object_index:id,byte_start,byte_end:end});}
   }
   offset=next;
  }
  if let Some(byte_start)=start{spans.push(SourceSpan{object_index:id,byte_start,byte_end:end});}
 }
 Ok(spans)
}

/// Bind a discovered native path to a source paint operator. Requires the
/// existing paragraph preflight/stream proof; this is an additional exact object
/// order check, not a replacement PDF parser or permission to trust saved hints.
pub(super) fn bind_paint_operator(mark:&Rectangle,content:&Content<Vec<Operation>>,page:&PdfPage<'_>)->EngineResult<usize>{
 let original=page.objects().get(mark.object_index)?;
 let bounds=solid_rectangle_bounds(&original)?.ok_or_else(||invalid("The highlight path changed."))?;
 let color=original.as_path_object().unwrap().fill_color()?;
 if bounds!=mark.bounds||[color.red(),color.green(),color.blue()]!=mark.color{return Err(invalid("The highlight path changed."));}
 let order=paint_order(content,page)?;
 order.into_iter().find_map(|(op,id)|(id==mark.object_index&&matches!(content.operations[op].operator.as_str(),"f"|"F"|"f*")).then_some(op))
  .ok_or_else(||invalid("Missing highlight paint operator."))
}

pub(super) fn paint_order(content:&Content<Vec<Operation>>,page:&PdfPage<'_>)->EngineResult<HashMap<usize,usize>>{
 let mut object_index=0;let mut bindings=HashMap::new();
 for (operator_index,operation) in content.operations.iter().enumerate(){
  let kind=match operation.operator.as_str(){
   "Tj"|"TJ"|"'"|"\""=>{if positioned_text::cursor_only(operation){continue;}0},
   "S"|"s"|"f"|"F"|"f*"|"B"|"B*"|"b"|"b*"=>1,
   "Do"=>2,
   "sh"|"BI"|"ID"|"EI"=>return Err(invalid("This page's graphic operators cannot be bound to the highlight.")),
   _=>continue,
  };
  let object=page.objects().get(object_index).map_err(|_|invalid("Ambiguous highlight paint ordering."))?;
  let matched=match kind{0=>object.as_text_object().is_some(),1=>object.as_path_object().is_some(),_=>object.as_image_object().is_some()||object.as_x_object_form_object().is_some()};
  if !matched{return Err(invalid("Ambiguous highlight paint ordering."));}
  bindings.insert(operator_index,object_index);
  object_index+=1;
 }
 if object_index!=page.objects().len() as usize{return Err(invalid("Ambiguous highlight paint ordering."));}
 Ok(bindings)
}

fn overlaps(a:[f32;4],b:[f32;4])->bool {
 a[2].min(b[2])-a[0].max(b[0])>EPSILON&&a[3].min(b[3])-a[1].max(b[1])>EPSILON
}
fn contains(a:[f32;4],b:[f32;4])->bool {
 b[0]>=a[0]-EPSILON&&b[1]>=a[1]-EPSILON&&b[2]<=a[2]+EPSILON&&b[3]<=a[3]+EPSILON
}
fn coverage(mark:&Rectangle,glyphs:&HashMap<usize,Vec<Glyph>>)->EngineResult<Option<Vec<Coverage>>>{
 if mark.bounds.iter().any(|v|!v.is_finite())||mark.bounds[2]<=mark.bounds[0]||mark.bounds[3]<=mark.bounds[1]{return Err(invalid("Invalid highlight rectangle geometry."));}
 let mut result=Vec::new();let mut covered_total=0;
 for (&object_index,characters) in glyphs {
  let mut indices=Vec::new();let mut visible=0;
  for (index,glyph) in characters.iter().enumerate(){
   if glyph.position.iter().any(|v|!v.is_finite()){return Err(invalid("Invalid highlight glyph geometry."));}
   // A space's advance rectangle is not painted ink. It must acquire logical
   // attribution later from independently mapped neighboring characters.
   if !glyph.text.is_empty()&&glyph.text.chars().all(char::is_whitespace){continue;}
   let bounds=[glyph.position[2],glyph.position[3],glyph.position[4],glyph.position[5]];
   if bounds[2]<bounds[0]||bounds[3]<bounds[1]{return Err(invalid("Invalid highlight glyph geometry."));}
   if bounds[2]==bounds[0]||bounds[3]==bounds[1]{continue;}
   visible+=1;
   if overlaps(mark.bounds,bounds){
    // Partial visible coverage and marks painted over text cannot establish a
    // text-background association, even when most of the glyph lies inside.
    if object_index<=mark.object_index||!contains(mark.bounds,bounds)||glyph.text.is_empty(){return Ok(None);}
    indices.push(index);covered_total+=1;
    if covered_total>MAX_COVERED_GLYPHS{return Err(invalid("The highlighted text exceeds its glyph limit."));}
   }
  }
  if !indices.is_empty(){result.push(Coverage{object_index,whole_visible_run:indices.len()==visible,glyph_indices:indices});}
 }
 result.sort_by_key(|coverage|coverage.object_index);
 Ok((!result.is_empty()).then_some(result))
}
fn classify(rectangles:Vec<Rectangle>,glyphs:&HashMap<usize,Vec<Glyph>>)->EngineResult<Vec<Candidate>>{
 if rectangles.len()>MAX_RECTANGLES||glyphs.values().map(Vec::len).sum::<usize>()>MAX_GLYPHS{return Err(invalid("The page exceeds highlight inspection limits."));}
 let mut candidates=Vec::new();
 for (index,mark) in rectangles.iter().enumerate(){
  // Competing rectangles can be stacked shading or compound highlighting; do
  // not choose one based on input order, color, or whichever is encountered first.
  if rectangles.iter().enumerate().any(|(other_index,other)|index!=other_index&&overlaps(mark.bounds,other.bounds)){continue;}
  if let Some(coverage)=coverage(mark,glyphs)?{candidates.push(Candidate{rectangle:mark.clone(),coverage});}
 }
 candidates.sort_by_key(|candidate|candidate.rectangle.object_index);Ok(candidates)
}
pub(super) fn discover(page:&PdfPage<'_>,geometry:PageGeometry,glyphs:&HashMap<usize,Vec<Glyph>>)->EngineResult<Vec<Candidate>>{
 classify(source_rectangles(page,geometry)?,glyphs)
}
fn source_rectangles(page:&PdfPage<'_>,geometry:PageGeometry)->EngineResult<Vec<Rectangle>>{
 let mut rectangles=Vec::new();
 for (object_index,object) in page.objects().iter().enumerate(){
  let Some(bounds)=solid_rectangle_bounds(&object)? else{continue;};
  if bounds[0]<=geometry.left+EPSILON&&bounds[1]<=geometry.bottom+EPSILON&&bounds[2]>=geometry.right-EPSILON&&bounds[3]>=geometry.top-EPSILON{continue;}
  let color=object.as_path_object().unwrap().fill_color()?;
  rectangles.push(Rectangle{object_index,bounds,color:[color.red(),color.green(),color.blue()]});
  if rectangles.len()>MAX_RECTANGLES{return Err(invalid("The page exceeds highlight inspection limits."));}
 }
 Ok(rectangles)
}

#[cfg(test)]
#[path = "paragraph_highlights_tests.rs"]
mod tests;
