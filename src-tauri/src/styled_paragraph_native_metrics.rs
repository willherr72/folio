//! Reproducible native ink for full-font styles. No coordinates from hints.
use super::*;
use styled_paragraph_layout::StyleMetrics;
use styled_paragraph_plan::ResolvedStyle;

pub(super) fn full_font(style:&ResolvedStyle)->bool{
 style.substitute.is_some()||style.source.program.as_ref().is_some_and(|id|style.source.font==format!("Folio{id}Paragraph"))
}

// Each style gets its own scratch page at the same zero origin. Coincident
// identical text objects can disappear from native extraction even when exact
// owner scans are used. Moving probes instead would change f32 ink rounding.
pub(super) fn metric_page_template(pdf:&Document,page_id:lopdf::ObjectId)->EngineResult<lopdf::Dictionary>{
 let source=pdf.get_dictionary(page_id).map_err(|_|invalid("Missing metric probe page."))?;
 // Do not multiply saved hints, annotations or unrelated page data by64.
 let mut page=dictionary!{"Type"=>"Page"};if let Ok(unit)=source.get(b"UserUnit"){page.set("UserUnit",unit.clone());}
 let mut current=page_id;let mut seen=HashSet::new();
 for _ in 0..64{
  if !seen.insert(current){return Err(invalid("Cyclic metric page inheritance."));}
  let node=pdf.get_dictionary(current).map_err(|_|invalid("Invalid metric page inheritance."))?;
  for key in [b"Resources".as_slice(),b"MediaBox",b"CropBox",b"Rotate"]{
   if !page.has(key){if let Ok(value)=node.get(key){page.set(key,value.clone());}}
  }
  match node.get(b"Parent"){
   Ok(parent)=>current=parent.as_reference().map_err(|_|invalid("Invalid metric page parent."))?,
   Err(_)=>return Ok(page),
  }
 }
 Err(invalid("Metric page inheritance exceeds its limit."))
}
impl WorkerRuntime {
 pub(super) fn native_highlight_metrics(&self,prepared:&Prepared,runs:&[StyledParagraphRun],styles:&[ResolvedStyle],metrics:&mut [StyleMetrics],include_unmarked:bool)->EngineResult<()> {
  let mut pdf=prepared.pdf.clone();let mut bound=styles.to_vec();
  styled_paragraph_publish::install_substitutes(&mut pdf,prepared.page_id,&prepared.resources,&mut bound)?;
  let template=metric_page_template(&pdf,prepared.page_id)?;let pages=pdf.new_object_id();
  let font_resources=pdf.dereference(template.get(b"Resources").map_err(|_|invalid("Missing metric resources."))?)
   .and_then(|(_,r)|r.as_dict()).and_then(|r|r.get(b"Font")).and_then(|f|pdf.dereference(f)).and_then(|(_,f)|f.as_dict())
   .map_err(|_|invalid("Missing metric font resources."))?.clone();
  let mut probes=Vec::new();let mut kids=Vec::new();let mut content_bytes=0usize;
  for (id,style) in bound.iter().enumerate().filter(|(_,s)|(include_unmarked||s.background_id.is_some())&&full_font(s)){
   let chars=runs.iter().filter(|r|r.style_id==id).flat_map(|r|r.text.chars()).filter(|c|!c.is_whitespace()).collect::<std::collections::BTreeSet<_>>();
   if chars.is_empty(){continue;}
   let text=chars.into_iter().collect::<String>();let encoded=style.encode(&text)?;
   let operations=vec![
    Operation::new("q",vec![]),Operation::new("BT",vec![]),Operation::new("Tf",vec![Object::Name(style.resource_name.clone()),style.size.into()]),
    Operation::new("Tc",vec![0.into()]),Operation::new("Tw",vec![0.into()]),Operation::new("Tz",vec![100.into()]),Operation::new("Ts",vec![0.into()]),Operation::new("Tr",vec![0.into()]),
    Operation::new("Tm",vec![1.into(),0.into(),0.into(),1.into(),0.into(),0.into()]),Operation::new("Tj",vec![Object::String(encoded,StringFormat::Hexadecimal)]),Operation::new("ET",vec![]),Operation::new("Q",vec![]),
   ];
   let data=Content{operations}.encode().map_err(|_|invalid("Native paragraph metrics could not be encoded."))?;
   content_bytes+=data.len();if content_bytes>LIMIT{return Err(invalid("Native paragraph metrics exceed the content limit."));}
   let content=pdf.add_object(Stream::new(dictionary!{},data));let mut page=template.clone();page.set("Parent",pages);page.set("Contents",content);
   let font=font_resources.get(&style.resource_name).map_err(|_|invalid("Missing bound metric font."))?.clone();
   let mut fonts=lopdf::Dictionary::new();fonts.set(style.resource_name.clone(),font);page.set("Resources",dictionary!{"Font"=>fonts});
   kids.push(Object::Reference(pdf.add_object(page)));probes.push((id,text));
  }
  if probes.is_empty(){return Ok(());}
  pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>kids.len() as i64,"Kids"=>kids}.into());
  pdf.catalog_mut().map_err(|_|invalid("Missing metric probe catalog."))?.set("Pages",pages);
  let mut bytes=Vec::new();pdf.save_to(&mut bytes).map_err(|_|invalid("Native paragraph metrics could not be measured."))?;
  if bytes.len() as u64>MAX_SOURCE_BYTES{return Err(invalid("Native paragraph metrics exceed the source limit."));}
  let document=self.pdfium.load_pdf_from_reader(Cursor::new(bytes),None)?;
  if document.pages().len() as usize!=probes.len(){return Err(invalid("Native paragraph metric pages are ambiguous."));}
  for (index,(id,text)) in probes.iter().enumerate(){
   let page=document.pages().get(index as i32)?;
   if page.objects().len()!=1{return Err(invalid("Native paragraph metric ownership is ambiguous."));}
   let glyphs=page_object_glyphs(&page)?;let glyphs=glyphs.get(&0).ok_or_else(||invalid("Missing native paragraph glyphs."))?;
   if glyphs.len()!=text.chars().count(){return Err(invalid("Native paragraph glyphs do not match the font probe."));}
   let mut x=0.;
   for (ch,glyph) in text.chars().zip(glyphs){
    if glyph.text!=ch.to_string()||!close_values(&glyph.position[..2],&[x,0.])||glyph.position.iter().any(|v|!v.is_finite()) {return Err(invalid("Native paragraph glyph geometry could not be verified."));}
    let [gx,gy,left,bottom,right,top]=glyph.position;
    let ink=[left-gx,bottom-gy,right-gx,top-gy];
    if ink.iter().any(|v|v.abs()>100_000.)||ink[2]<ink[0]||ink[3]<ink[1]{return Err(invalid("Native paragraph ink exceeds its limits."));}
    metrics[*id].glyphs.get_mut(&ch).ok_or_else(||invalid("Missing paragraph metric."))?.ink=ink;
    x+=bound[*id].advance(ch)?;
   }
  }
  Ok(())
 }
}
