//! Source-owned background emission and independent canonical-block binding.
use super::*;
use paragraph_highlight_geometry::{Background,PlacedBackground};
use paragraph_highlights::{BoundHighlight,Rectangle};
use styled_paragraph_plan::ResolvedStyle;

fn final_rectangles(prepared:&Prepared,layout:&StyledParagraphLayout,styles:&[ResolvedStyle],page:&PdfPage<'_>,actual:&[&RunSnapshot],source_values:&HashMap<usize,String>)->EngineResult<Vec<PlacedBackground>>{
 styled_paragraph_publish::verify_fragments(layout,styles,actual,page,source_values)?;
 let glyphs=page_object_glyphs(page)?;
 let fragments=actual.iter().map(|r|glyphs.get(&r.index).cloned().ok_or_else(||invalid("Missing saved highlighted glyphs."))).collect::<EngineResult<Vec<_>>>()?;
 let ids=styles.iter().map(|s|s.background_id).collect::<Vec<_>>();
 paragraph_highlight_geometry::native_rectangles_with_styles(layout,&fragments,&ids,&prepared.backgrounds,Some(styles))
}

/// Remove templates with no painted evidence, including deletion of a mark's
/// final visible word. Empty metadata cannot authorize a future background.
fn retain_painted(backgrounds:&mut Vec<Background>,styles:&mut [ResolvedStyle],rectangles:&mut [PlacedBackground]){
 let used=rectangles.iter().map(|r|r.background_id).collect::<HashSet<_>>();let mut mapping=HashMap::new();let mut kept=Vec::new();
 for (id,background) in backgrounds.iter().enumerate(){if used.contains(&id){mapping.insert(id,kept.len());kept.push(background.clone());}}
 for style in styles{style.background_id=style.background_id.and_then(|id|mapping.get(&id).copied());}
 for rectangle in rectangles{rectangle.background_id=mapping[&rectangle.background_id];}
 *backgrounds=kept;
}

pub(super) fn emit(runtime:&WorkerRuntime,prepared:&mut Prepared,layout:&StyledParagraphLayout,styles:&mut [ResolvedStyle])->EngineResult<Vec<u8>>{
 let styled_paragraph_publish::Emission{bytes,block_start}=styled_paragraph_publish::emit_located(prepared,layout,styles)?;
 if prepared.backgrounds.is_empty(){return Ok(bytes);}
 let count=layout.lines.iter().map(|line|line.runs.len()).sum::<usize>();
 let mut rectangles={
  let document=runtime.pdfium.load_pdf_from_byte_slice(&bytes,None)?;let page=document.pages().get(0)?;let proofs=FontProofs::read(&bytes)?;let after=bound_snapshot(&page,Some(&proofs),&bytes)?;
  let range=styled_paragraph_publish::fragment_range(prepared,count,after.len())?;
  let values=source_text::values(&bytes,&after).unwrap_or_default();
  let source_values=after.iter().zip(values).filter_map(|(run,value)|value.map(|value|(run.index,value))).collect();
  let actual=after[range].iter().collect::<Vec<_>>();final_rectangles(prepared,layout,styles,&page,&actual,&source_values)?
 };
 retain_painted(&mut prepared.backgrounds,styles,&mut rectangles);
 if rectangles.is_empty(){return Ok(bytes);}
 let mut pdf=Document::load_mem_with_options(&bytes,lopdf::LoadOptions::with_max_decompressed_size(LIMIT)).map_err(|_|invalid("Invalid highlighted output."))?;
 let id=prepared.page_id;let mut content=Content::decode(&pdf.get_page_content(id)).map_err(|_|invalid("Invalid highlighted content."))?;
 let start=block_start;
 let blocks=content.operations.get(start..start+count*13).ok_or_else(||invalid("Missing emitted paragraph blocks."))?;
 const BLOCK:[&str;13]=["q","BT","Tf","Tc","Tw","Tz","Ts","Tr","rg","Tm","Tj","ET","Q"];
 if blocks.chunks_exact(13).any(|ops|ops.iter().zip(BLOCK).any(|(op,name)|op.operator!=name)){return Err(invalid("Ambiguous emitted paragraph blocks."));}
 content.operations.splice(start..start,paragraph_highlight_geometry::paint_operations(&rectangles)?);
 let data=content.encode().map_err(|_|invalid("The highlighted paragraph could not be encoded."))?;if data.len()>LIMIT{return Err(invalid("The highlighted content exceeds the size limit."));}
 let stream=pdf.add_object(Stream::new(dictionary!{},data));pdf.get_dictionary_mut(id).map_err(|_|invalid("Missing highlighted page."))?.set("Contents",stream);
 let mut output=Vec::new();pdf.save_to(&mut output).map_err(|_|invalid("The highlighted paragraph could not be saved."))?;
 if output.len() as u64>MAX_SOURCE_BYTES{return Err(invalid("The highlighted page exceeds the source size limit."));}Ok(output)
}

/// A saved hint supplies candidate semantics only. Independently remeasure text,
/// reproduce every exact paint block, bind its native shape/order and reject any
/// ownership of unselected glyphs before it may authorize removal or restoration.
pub(super) fn bind_saved(prepared:&Prepared,layout:&StyledParagraphLayout,styles:&[ResolvedStyle],page:&PdfPage<'_>)->EngineResult<Vec<BoundHighlight>>{
 if prepared.backgrounds.is_empty(){return Ok(vec![]);}
 let actual=prepared.selection.object_indices.iter().map(|id|prepared.before.iter().find(|r|r.index==*id).ok_or_else(||invalid("Missing saved paragraph owner."))).collect::<EngineResult<Vec<_>>>()?;
 let rectangles=final_rectangles(prepared,layout,styles,page,&actual,&prepared.source_text)?;
 let used=rectangles.iter().map(|r|r.background_id).collect::<HashSet<_>>();if used.len()!=prepared.backgrounds.len(){return Err(invalid("Saved backgrounds lack painted evidence."));}
 let mut blocks=HashMap::<Vec<u8>,Option<usize>>::new();
 for (start,ops) in prepared.content.operations.windows(5).enumerate(){
  if ops.iter().zip(["q","rg","re","f","Q"]).all(|(op,name)|op.operator==name){
   let key=Content{operations:ops.to_vec()}.encode().map_err(|_|invalid("Invalid saved background block."))?;
   blocks.entry(key).and_modify(|entry|*entry=None).or_insert(Some(start+3));
  }
 }
 let order=paragraph_highlights::paint_order(&prepared.content,page)?;let glyphs=page_object_glyphs(page)?;let mut result=Vec::new();let mut owned=HashSet::new();
 for rectangle in rectangles {
  let ops=paragraph_highlight_geometry::paint_operations(std::slice::from_ref(&rectangle))?;let key=Content{operations:ops}.encode().map_err(|_|invalid("Invalid expected background block."))?;
  let paint_operator=blocks.get(&key).copied().flatten().ok_or_else(||invalid("The saved highlight block cannot be bound uniquely."))?;
  let object_index=*order.get(&paint_operator).ok_or_else(||invalid("Missing saved native highlight."))?;
  if !owned.insert(object_index)||object_index>=prepared.selection.object_indices[0]{return Err(invalid("Invalid saved highlight ordering."));}
  let object=page.objects().get(object_index)?;let bounds=solid_rectangle_bounds(&object)?.ok_or_else(||invalid("The saved highlight is not an opaque rectangle."))?;
  let color=object.as_path_object().unwrap().fill_color()?;
  if !close_values(&bounds,&rectangle.bounds)||[color.red(),color.green(),color.blue()]!=rectangle.color{return Err(invalid("The saved highlight geometry or color changed."));}
  for (id,glyphs) in &glyphs {if !prepared.selection.object_indices.contains(id)&&glyphs.iter().any(|g|!g.text.trim().is_empty()&&adds_overlap(&[0.;4],&bounds,&[g.position[2],g.position[3],g.position[4],g.position[5]])){return Err(invalid("The saved highlight covers unselected text."));}}
  result.push(BoundHighlight{rectangle:Rectangle{object_index,bounds,color:rectangle.color},paint_operator,cell:prepared.backgrounds[rectangle.background_id].cell,spans:vec![]});
 }
 Ok(result)
}
