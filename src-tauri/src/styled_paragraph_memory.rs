//! Disposable mixed-paragraph hints. Reproduce operators, state, fonts and actual
//! glyph geometry from current page resources before restoring any logical draft.
use super::*;
use serde::{Deserialize,Serialize};
use styled_paragraph_plan::ResolvedStyle;
const KEY:&[u8]=b"FolioStyledParagraphs";
const MAX_BYTES:usize=4*1024*1024;

#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Style {resource:Vec<u8>,sample:Vec<u8>,size:f32,color:[u8;4],#[serde(default,skip_serializing_if="Option::is_none")]background_id:Option<usize>}
impl Style {
    pub(super) fn source(style:&ResolvedStyle)->EngineResult<Self>{
        let ch=style.source.proof.as_ref().and_then(|p|p.sample()).unwrap_or('A');
        Ok(Self{resource:style.resource_name.clone(),sample:positioned_text::encoded(&style.source,&ch.to_string())?,size:style.size,color:style.color,background_id:style.background_id})
    }
    fn published(style:&ResolvedStyle)->EngineResult<Self>{
        if let Some(asset)=&style.substitute{
            let ch=asset.info.coverage.iter().flat_map(|r|r[0]..=r[1].min(0xffff)).filter_map(char::from_u32)
                .find(|c|embedded_font::latin(&c.to_string())&&asset.glyph_id(*c).is_ok()).ok_or_else(||invalid("The saved font has no verified Latin sample."))?;
            Ok(Self{resource:style.resource_name.clone(),sample:paragraph_font::encode(&ch.to_string()),size:style.size,color:style.color,background_id:style.background_id})
        }else{Self::source(style)}
    }
    fn valid(&self,unit:f32)->bool{!self.resource.is_empty()&&self.resource.len()<=512&&(1..=2).contains(&self.sample.len())
        &&physical_units::paragraph_size(self.size,unit)&&self.color[3]==255}
}
#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Fragment {font:String,program:Option<String>,size:f32,color:[u8;4],origin:[f32;2],digest:String}
#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {#[serde(default,deserialize_with="present_baseline",skip_serializing_if="Option::is_none")]first_baseline:Option<f32>,bounds:[f32;4],alignment:ParagraphAlignment,spacing:f32,runs:Vec<StyledParagraphRun>,styles:Vec<Style>,fragments:Vec<Fragment>,#[serde(default,skip_serializing_if="Vec::is_empty")]backgrounds:Vec<paragraph_highlight_geometry::Background>,#[serde(default,skip_serializing_if="is_false")]native_ink:bool,#[serde(default,skip_serializing_if="is_false")]text_alignment:bool,#[serde(default,skip_serializing_if="zero_insets")]text_insets:[f32;2],#[serde(default,deserialize_with="present_bool",skip_serializing_if="Option::is_none")]ink_vertical:Option<bool>,#[serde(default,skip_serializing_if="Option::is_none")]context:Option<String>}
// A supported flag may be absent or explicitly false. Keep that distinction
// until the envelope version is checked; null and non-booleans are invalid.
fn present_bool<'de,D:serde::Deserializer<'de>>(deserializer:D)->Result<Option<bool>,D::Error>{bool::deserialize(deserializer).map(Some)}
fn present_baseline<'de,D:serde::Deserializer<'de>>(deserializer:D)->Result<Option<f32>,D::Error>{f32::deserialize(deserializer).map(Some)}
fn is_false(value:&bool)->bool{!*value}
fn zero_insets(value:&[f32;2])->bool{*value==[0.;2]}
fn version(records:&[Record])->u32{if records.iter().any(|r|r.backgrounds.iter().any(|b|b.cell.is_some())){8}else if records.iter().any(|r|r.first_baseline.is_some()){7}else if records.iter().any(|r|r.context.is_some()){6}else if records.iter().any(|r|r.ink_vertical.is_some()){5}else if records.iter().any(|r|r.text_alignment){4}else if records.iter().any(|r|r.native_ink){3}else if records.iter().any(|r|!r.backgrounds.is_empty()){2}else{1}}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Records {version:u32,paragraphs:Vec<Record>}
fn read(pdf:&Document,page:lopdf::ObjectId)->Vec<Record>{
    let Ok(unit)=physical_units::page_unit(pdf,page)else{return vec![];};
    let Some(bytes)=pdf.get_dictionary(page).ok().and_then(|d|d.get(KEY).ok()).and_then(|v|v.as_str().ok())else{return vec![];};
    if bytes.len()>MAX_BYTES{return vec![];}
    let Ok(records)=serde_json::from_slice::<Records>(bytes)else{return vec![];};
    if !matches!(records.version,1|2|3|4|5|6|7|8)||records.paragraphs.len()>64||records.paragraphs.iter().map(|r|r.fragments.len()).sum::<usize>()>4096
        ||(records.version<8&&records.paragraphs.iter().any(|r|r.backgrounds.iter().any(|b|b.cell.is_some())))
        ||(records.version<7&&records.paragraphs.iter().any(|r|r.first_baseline.is_some()))
        ||(records.version<6&&records.paragraphs.iter().any(|r|r.context.is_some()))
        ||(records.version<5&&records.paragraphs.iter().any(|r|r.ink_vertical.is_some()))
        ||(records.version<4&&records.paragraphs.iter().any(|r|r.text_alignment||r.text_insets!=[0.;2]))
        ||(records.version<3&&records.paragraphs.iter().any(|r|r.native_ink))
        ||(records.version==1&&records.paragraphs.iter().any(|r|!r.backgrounds.is_empty()||r.styles.iter().any(|s|s.background_id.is_some()))){return vec![];}
    records.paragraphs.into_iter().filter(|r|!r.fragments.is_empty()&&r.fragments.len()<=4096
        &&r.first_baseline.is_none_or(|y|y.is_finite()&&y.abs()<=1_000_000.&&y>=r.bounds[1]&&y<=r.bounds[3])
        &&r.context.as_ref().is_none_or(|s|s.len()==64&&s.bytes().all(|b|b.is_ascii_hexdigit()))
        &&!r.styles.is_empty()&&r.styles.len()<=64&&r.styles.iter().all(|s|s.valid(unit))
        &&(!r.ink_vertical.unwrap_or(false)||r.text_alignment)
        &&paragraph_highlight_geometry::valid_text_insets(r.text_insets)&&(r.text_alignment||r.text_insets==[0.;2])
        &&paragraph_highlight_geometry::valid_backgrounds(&r.backgrounds)&&r.styles.iter().all(|s|s.background_id.is_none_or(|id|id<r.backgrounds.len()))
        &&styled_paragraph_layout::validate_draft(&r.runs).is_ok()&&r.runs.iter().all(|run|run.style_id<r.styles.len())
        &&r.bounds.iter().all(|v|v.is_finite()&&v.abs()<=1_000_000.)&&r.bounds[2]>r.bounds[0]&&r.bounds[3]>r.bounds[1]
        &&r.spacing.is_finite()&&(0.6..=3.).contains(&r.spacing)
        &&r.fragments.iter().all(|f|f.font.len()<=512&&f.program.as_ref().is_none_or(|p|p.len()==64)&&f.digest.len()==64
            &&physical_units::paragraph_size(f.size,unit)&&f.origin.iter().all(|v|v.is_finite()&&v.abs()<=1_000_000.))).collect()
}
fn indices(record:&Record,before:&[RunSnapshot])->Option<Vec<usize>>{
    // Writer-owned records persist exact f32 state. Index that state once rather
    // than scanning every object for every fragment (4096 x 4096 comparisons).
    // Duplicate actual bindings remain ambiguous; no record or object wins by
    // order. Canonicalize signed zero, which PDF readers can interchange.
    let bits=|v:f32|if v==0. {0}else{v.to_bits()};
    let mut lookup=HashMap::new();
    for run in before.iter().filter(|r|r.matrix[..4]==[1.,0.,0.,1.]){
        let key=(run.font.clone(),run.program.clone(),bits(run.font_size),run.color,[bits(run.matrix[4]),bits(run.matrix[5])]);
        lookup.entry(key).and_modify(|value|*value=None).or_insert(Some(run.index));
    }
    let mut result=Vec::new();
    for fragment in &record.fragments{
        let key=(fragment.font.clone(),fragment.program.clone(),bits(fragment.size),fragment.color,fragment.origin.map(bits));
        result.push((*lookup.get(&key)?)?);
    }
    result.windows(2).all(|p|p[0]<p[1]).then_some(result)
}
fn digest(bytes:&[u8])->String{embedded_font::key(bytes)}
/// A candidate is only a selection hint. The caller must independently restore
/// it against the current native page before returning its logical draft.
pub(super) fn candidate(bytes:&[u8],before:&[RunSnapshot],anchor:usize)->Option<Vec<usize>>{
    let pdf=Document::load_mem_with_options(bytes,lopdf::LoadOptions::with_max_decompressed_size(LIMIT)).ok()?;
    let page=*pdf.get_pages().values().next()?;
    let candidates=read(&pdf,page).iter().filter_map(|r|indices(r,before)).filter(|ids|ids.contains(&anchor)).collect::<Vec<_>>();
    if candidates.len()==1{candidates.into_iter().next()}else{None}
}
fn show_bytes(op:&Operation)->Option<&[u8]>{match(op.operator.as_str(),op.operands.as_slice()){
    ("Tj",[Object::String(bytes,_)])=>Some(bytes),_=>None,
}}

/// Prune disposable bindings before emission changes the operator list. Invalid
/// hints elsewhere must not consume the budget of an otherwise valid new edit.
pub(super) fn retain_unaffected(p:&mut Prepared)->EngineResult<()> {
    let shows=p.content.operations.iter().filter(|op|matches!(op.operator.as_str(),"Tj"|"TJ"|"'"|"\"")&&!positioned_text::cursor_only(op)).collect::<Vec<_>>();
    if shows.len()!=p.before.len(){return Err(invalid("Saved paragraph operators are ambiguous."));}
    let digests=p.before.iter().zip(shows).filter_map(|(run,op)|show_bytes(op).map(|bytes|(run.index,digest(bytes)))).collect::<HashMap<_,_>>();
    let candidates=read(&p.pdf,p.page_id).into_iter().filter_map(|r|indices(&r,&p.before).map(|ids|(r,ids))).collect::<Vec<_>>();
    let mut overlaps=HashMap::<usize,usize>::new();for (_,ids) in &candidates{for id in ids{*overlaps.entry(*id).or_default()+=1;}}
    let records=candidates.into_iter().filter(|(r,ids)|ids.iter().all(|id|overlaps[id]==1&&!p.selection.object_indices.contains(id))
        &&r.fragments.iter().zip(ids).all(|(f,id)|digests.get(id)==Some(&f.digest))).map(|(r,_)|r).collect::<Vec<_>>();
    let page=p.pdf.get_dictionary_mut(p.page_id).map_err(|_|invalid("Missing saved paragraph page."))?;
    if records.is_empty(){page.remove(KEY);}else{
        let version=version(&records);
        let data=serde_json::to_vec(&Records{version,paragraphs:records}).map_err(|_|invalid("The paragraph settings could not be encoded."))?;
        if data.len()>MAX_BYTES{return Err(invalid("The paragraph settings exceed the size limit."));}
        page.set(KEY,Object::String(data,StringFormat::Hexadecimal));
    }
    Ok(())
}

impl WorkerRuntime {
    /// Private probes bind even newline-only styles to verified actual resources.
    /// Nothing from this scratch page enters the user's PDF.
    pub(super) fn styled_font_probes(&self,pdf:&Document,page_id:lopdf::ObjectId,definitions:&[Style])->EngineResult<(PdfDocument<'_>,Vec<BoundStyle>)>{
        let unit=physical_units::page_unit(pdf,page_id)?;
        if definitions.is_empty()||definitions.len()>64||definitions.iter().any(|s|!s.valid(unit)){return Err(invalid("Invalid saved paragraph styles."));}
        let mut pdf=pdf.clone();
        let mut template=styled_paragraph_native_metrics::metric_page_template(&pdf,page_id)?;
        let fonts=pdf.dereference(template.get(b"Resources").map_err(|_|invalid("Missing style probe resources."))?)
            .and_then(|(_,r)|r.as_dict()).and_then(|r|r.get(b"Font")).and_then(|f|pdf.dereference(f)).and_then(|(_,f)|f.as_dict())
            .map_err(|_|invalid("Missing style probe fonts."))?.clone();
        // Do not multiply unrelated direct resource dictionaries by the palette
        // size. Each scratch page uses only its exact source font binding.
        template.remove(b"Resources");
        let pages=pdf.new_object_id();let mut ids=Vec::new();
        for s in definitions {
            let ops=vec![
                Operation::new("q",vec![]),Operation::new("BT",vec![]),Operation::new("Tf",vec![Object::Name(s.resource.clone()),s.size.into()]),
                Operation::new("rg",s.color[..3].iter().map(|c|Object::Real(*c as f32/255.)).collect()),
                Operation::new("Tm",vec![1.into(),0.into(),0.into(),1.into(),0.into(),100.into()]),
                Operation::new("Tj",vec![Object::String(s.sample.clone(),StringFormat::Hexadecimal)]),Operation::new("ET",vec![]),Operation::new("Q",vec![]),
            ];
            let data=Content{operations:ops}.encode().map_err(|_|invalid("The saved styles could not be measured."))?;
            let content=pdf.add_object(Stream::new(dictionary!{},data));let mut page=template.clone();
            let font=fonts.get(&s.resource).map_err(|_|invalid("Missing bound style probe font."))?.clone();
            let mut bound=lopdf::Dictionary::new();bound.set(s.resource.clone(),font);page.set("Resources",dictionary!{"Font"=>bound});
            page.set("Parent",pages);page.set("Contents",content);ids.push(pdf.add_object(page));
        }
        pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>ids.len() as i64,"Kids"=>ids.iter().map(|id|Object::Reference(*id)).collect::<Vec<_>>()}.into());
        pdf.catalog_mut().map_err(|_|invalid("Missing style probe catalog."))?.set("Pages",pages);
        let mut bytes=Vec::new();pdf.save_to(&mut bytes).map_err(|_|invalid("The saved styles could not be measured."))?;
        if bytes.len() as u64>MAX_SOURCE_BYTES{return Err(invalid("The saved style probes exceed the source size limit."));}
        let proofs=FontProofs::read(&bytes)?;let document=self.pdfium.load_pdf_from_reader(Cursor::new(bytes),None)?;
        if document.pages().len() as usize!=definitions.len(){return Err(invalid("The saved style pages cannot be mapped uniquely."));}
        let mut styles=Vec::new();
        for (index,id) in ids.into_iter().enumerate(){
            let page=document.pages().get(index as i32)?;let before=snapshot(&page,Some(&proofs))?;
            if before.len()!=1||before[0].index!=0{return Err(invalid("The saved style resources cannot be mapped uniquely."));}
            let checked=prepare_page(pdf.clone(),id,before,&[0],page_geometry(&page)?,true)?;
            if checked.styles.len()!=1{return Err(invalid("Missing saved style resource."));}
            let mut style=checked.styles[0].clone();style.run=checked.before[0].clone();style.object_indices.clear();styles.push(style);
        }
        Ok((document,styles))
    }
    pub(super) fn restore_styled_memory(&self,p:&mut Prepared,page:&PdfPage<'_>)->EngineResult<bool>{
        if !p.reproducible_text_state{return Ok(false);}
        let records=read(&p.pdf,p.page_id);
        let overlaps=records.iter().filter_map(|r|indices(r,&p.before)).filter(|ids|ids.iter().any(|id|p.selection.object_indices.contains(id))).count();
        if overlaps!=1{return Ok(false);}
        let Some(record)=records.iter().find(|r|indices(r,&p.before).as_ref()==Some(&p.selection.object_indices))else{return Ok(false);};
        if record.context!=p.context_binding()? {return Ok(false);}
        p.check_context_area(record.bounds)?;
        let g=p.geometry;if record.bounds[0]<g.left||record.bounds[1]<g.bottom||record.bounds[2]>g.right||record.bounds[3]>g.top{return Ok(false);}
        for (fragment,(op_index,_)) in record.fragments.iter().zip(&p.removals){
            if show_bytes(&p.content.operations[*op_index]).map(digest).as_deref()!=Some(fragment.digest.as_str()){return Ok(false);}
        }
        let (probes,mut bindings)=self.styled_font_probes(&p.pdf,p.page_id,&record.styles)?;
        for (binding,style) in bindings.iter_mut().zip(&record.styles){binding.background_id=style.background_id;}
        let styles=bindings.iter().map(ResolvedStyle::original).collect::<Vec<_>>();
        let mut metrics=styled_paragraph_plan::measure(&probes,&record.runs,&styles)?;
        if record.native_ink{self.native_highlight_metrics(p,&record.runs,&styles,&mut metrics,record.ink_vertical.unwrap_or(false))?;}
        let ids=styles.iter().map(|s|s.background_id).collect::<Vec<_>>();
        let padded=if record.backgrounds.is_empty(){None}else{Some(paragraph_highlight_geometry::padded_metrics_in_units(&metrics,&ids,&record.backgrounds,p.unit)?)};
        let layout=paragraph_highlight_geometry::layout_with_baseline(&record.runs,&metrics,padded.as_deref(),record.bounds,record.alignment,record.spacing,p.unit,record.text_insets,record.text_alignment,record.ink_vertical.unwrap_or(false),record.first_baseline)?;
        let fragments=layout.lines.iter().flat_map(|line|&line.runs).collect::<Vec<_>>();
        if fragments.len()!=p.removals.len(){return Ok(false);}
        for ((fragment,(op_index,_)),id) in fragments.iter().zip(&p.removals).zip(&p.selection.object_indices){
            if show_bytes(&p.content.operations[*op_index])!=Some(styles[fragment.style_id].encode(&fragment.text)?.as_slice()){return Ok(false);}
            let actual=p.before.iter().find(|r|r.index==*id).unwrap();
            // These are writer-owned identity matrices with persisted f32
            // origins. Even a subpixel shift that happens to rasterize equally
            // at one zoom must not authorize a different default placement.
            if actual.matrix[4..]!=fragment.origin||actual.matrix[..4]!=[1.,0.,0.,1.]||actual.font_size!=styles[fragment.style_id].size{return Ok(false);}
            bindings[fragment.style_id].object_indices.push(*id);
        }
        let actual=p.selection.object_indices.iter().map(|id|p.before.iter().find(|r|r.index==*id).unwrap()).collect::<Vec<_>>();
        styled_paragraph_publish::verify_fragments(&layout,&styles,&actual,page,&p.source_text)?;
        // Coordinate tolerances are not proof of a faithful default re-edit.
        // Re-emit from independently measured resources and require exact pixels.
        let mut candidate=p.clone();candidate.remembered=true;
        candidate.backgrounds=record.backgrounds.clone();candidate.highlight_sources=paragraph_highlight_publish::bind_saved(&candidate,&layout,&styles,page)?;
        let bound_highlights=candidate.highlight_sources.clone();
        let replay=paragraph_highlight_publish::emit(self,&mut candidate,&layout,&mut styles.clone())?;
        let replay_document=self.pdfium.load_pdf_from_reader(Cursor::new(replay),None)?;
        if raster(&replay_document.pages().get(0)?)?!=raster(page)?{return Ok(false);}
        if p.draft_ranges.is_some(){
            let logical=record.runs.iter().map(|r|r.text.as_str()).collect::<String>();
            p.draft_ranges=Some(paragraph_draft::verified_fragments(&logical,&layout,&p.selection.object_indices,&p.source_text)?);
        }
        p.runs=record.runs.clone();p.styles=bindings;p.selection.text=p.runs.iter().map(|r|r.text.as_str()).collect();
        p.selection.bounds=displayed_bounds(g,record.bounds);p.selection.alignment=Some(record.alignment);p.selection.line_spacing=Some(record.spacing);
        p.first_baseline=layout.lines.first().map(|line|line.baseline);
        p.remembered=true;p.native_ink=record.native_ink;p.text_alignment=record.text_alignment;p.text_insets=record.text_insets;p.ink_vertical=record.ink_vertical.unwrap_or(false);
        p.backgrounds=record.backgrounds.clone();p.highlight_sources=bound_highlights;
        Ok(true)
    }
}

pub(super) fn write(runtime:&WorkerRuntime,bytes:&[u8],prepared:&Prepared,layout:&StyledParagraphLayout,styles:&[ResolvedStyle],request:&StyledParagraphLayoutRequest)->EngineResult<Vec<u8>>{
    let mut pdf=Document::load_mem_with_options(bytes,lopdf::LoadOptions::with_max_decompressed_size(LIMIT)).map_err(|_|invalid("Invalid saved paragraph page."))?;
    let page=*pdf.get_pages().values().next().ok_or_else(||invalid("Missing saved paragraph page."))?;
    let mut records=read(&pdf,page).into_iter().filter(|r|indices(r,&prepared.before).is_some_and(|ids|!ids.iter().any(|id|prepared.selection.object_indices.contains(id)))).collect::<Vec<_>>();
    let fragments=layout.lines.iter().flat_map(|line|&line.runs).map(|fragment|{
        let s=&styles[fragment.style_id];Ok(Fragment{font:s.substitute.as_ref().map_or_else(||s.source.font.clone(),|a|format!("Folio{}Paragraph",a.info.id)),
            program:s.substitute.as_ref().map(|a|a.info.id.clone()).or_else(||s.source.program.clone()),size:s.size,color:s.color,origin:fragment.origin,digest:digest(&s.encode(&fragment.text)?)})
    }).collect::<EngineResult<Vec<_>>>()?;
    records.push(Record{first_baseline:request.baseline_anchor.and_then(|_|layout.lines.first().map(|line|line.baseline)),bounds:layout.pdf_bounds,alignment:request.alignment,spacing:request.line_spacing,runs:request.runs.clone(),styles:styles.iter().map(Style::published).collect::<EngineResult<_>>()?,fragments,backgrounds:prepared.backgrounds.clone(),native_ink:prepared.native_ink,text_alignment:prepared.text_alignment,text_insets:prepared.text_insets,ink_vertical:prepared.ink_vertical.then_some(true),context:prepared.context_binding()?});
    let encode=|records:&Vec<Record>|serde_json::to_vec(&Records{version:version(records),paragraphs:records.clone()}).map_err(|_|invalid("The paragraph settings could not be encoded."));
    let mut data=encode(&records)?;
    if records.len()>64||records.iter().map(|r|r.fragments.len()).sum::<usize>()>4096||data.len()>MAX_BYTES{
        // Binding/digest checks alone do not prove the logical draft replays.
        // Before refusing capacity, verify retained hints against this emitted
        // page. Only this exceptional path pays for full independent replay.
        // The new record is separately verified by publication after writing.
        let added=records.pop().unwrap();
        let document=runtime.pdfium.load_pdf_from_reader(Cursor::new(bytes.to_vec()),None)?;
        let native_page=document.pages().get(0)?;let proofs=FontProofs::read(bytes)?;
        let before=snapshot(&native_page,Some(&proofs))?;let geometry=page_geometry(&native_page)?;
        records.retain(|record|{
            let check=||->EngineResult<bool>{
                let Some(ids)=indices(record,&before)else{return Ok(false);};
                let mut candidate=prepare(bytes,before.clone(),&ids,geometry,true)?;
                runtime.restore_styled_memory(&mut candidate,&native_page)
            };
            check().unwrap_or(false)
        });
        records.push(added);data=encode(&records)?;
    }
    // Do not evict another large paragraph's only independently verifiable
    // re-edit record merely to fit a new one into the page's bounded hint budget.
    if records.len()>64||records.iter().map(|r|r.fragments.len()).sum::<usize>()>4096{return Err(invalid("The page's saved paragraph settings exceed the fragment limit."));}
    if data.len()>MAX_BYTES{return Err(invalid("The paragraph settings exceed the size limit."));}
    pdf.get_dictionary_mut(page).map_err(|_|invalid("Missing saved paragraph page."))?.set(KEY,Object::String(data,StringFormat::Hexadecimal));
    let mut result=Vec::new();pdf.save_to(&mut result).map_err(|_|invalid("The paragraph settings could not be saved."))?;
    if result.len() as u64>MAX_SOURCE_BYTES{return Err(invalid("The edited page exceeds the source size limit."));}Ok(result)
}


#[cfg(test)]
mod probe_tests {
    use super::*;
#[test]
fn cell_policy_requires_version_eight_and_single_geometry_policy(){
 let original=Document::load_mem(include_bytes!("../../tests/fixtures/paragraphs/legacy-tight-v4.pdf")).unwrap();
 let page=*original.get_pages().get(&1).unwrap();
 let saved:serde_json::Value=serde_json::from_slice(original.get_dictionary(page).unwrap().get(KEY).unwrap().as_str().unwrap()).unwrap();
 for v in 4..=8{for bad in [false,true]{
  let mut value=saved.clone();value["version"]=serde_json::json!(v);
  value["paragraphs"][0]["backgrounds"]=serde_json::json!([{"color":[255,255,0],"padding":if bad{[1.;4]}else{[0.;4]},"cell":{"offsets":[0.,-0.003,0.,0.1]}}]);
  let mut pdf=original.clone();pdf.get_dictionary_mut(page).unwrap().set(KEY,Object::string_literal(serde_json::to_vec(&value).unwrap()));
  let records=read(&pdf,page);
  if v==8&&!bad{assert_eq!(records.len(),1);assert_eq!(version(&records),8);}else{assert!(records.is_empty());}
 }}
}
#[test]
fn baseline_anchor_requires_version_seven_and_valid_present_number(){
    let original=Document::load_mem(include_bytes!("../../tests/fixtures/paragraphs/legacy-tight-v4.pdf")).unwrap();
    let page=*original.get_pages().get(&1).unwrap();
    let saved:serde_json::Value=serde_json::from_slice(original.get_dictionary(page).unwrap().get(KEY).unwrap().as_str().unwrap()).unwrap();
    let mut pdf=original.clone();
    for v in 1..=7 {
        let mut clean=saved.clone();clean["version"]=serde_json::json!(v);
        if v<4{clean["paragraphs"][0].as_object_mut().unwrap().remove("text_alignment");}
        pdf.get_dictionary_mut(page).unwrap().set(KEY,Object::string_literal(serde_json::to_vec(&clean).unwrap()));
        assert_eq!(read(&pdf,page).len(),1);
        for value in [serde_json::json!(700.),serde_json::Value::Null,serde_json::json!("700"),serde_json::json!(true),serde_json::json!(1e40),serde_json::json!(-1.)]{
            let mut changed=clean.clone();changed["paragraphs"][0]["first_baseline"]=value.clone();
            pdf.get_dictionary_mut(page).unwrap().set(KEY,Object::string_literal(serde_json::to_vec(&changed).unwrap()));
            let records=read(&pdf,page);
            if v==7&&value==serde_json::json!(700.){assert_eq!(records.len(),1);assert_eq!(version(&records),7);}
            else{assert!(records.is_empty(),"v{v} accepted first_baseline={value}");}
        }
    }
}
// Stage inside styled_paragraph_memory::probe_tests after the current native gate.
// Run this RED before changing Record's boolean representation.
#[test]
fn legacy_paragraph_versions_reject_presence_of_new_vertical_policy() {
    let original=Document::load_mem(include_bytes!("../../tests/fixtures/paragraphs/legacy-tight-v4.pdf")).unwrap();
    let page=*original.get_pages().get(&1).unwrap();
    let saved:serde_json::Value=serde_json::from_slice(original.get_dictionary(page).unwrap().get(KEY).unwrap().as_str().unwrap()).unwrap();
    for version in 1..=4 {
        let mut clean=saved.clone();clean["version"]=serde_json::json!(version);
        if version<4 {
            clean["paragraphs"][0].as_object_mut().unwrap().remove("text_alignment");
            clean["paragraphs"][0].as_object_mut().unwrap().remove("text_insets");
        }
        let mut pdf=original.clone();
        pdf.get_dictionary_mut(page).unwrap().set(KEY,Object::string_literal(serde_json::to_vec(&clean).unwrap()));
        assert_eq!(read(&pdf,page).len(),1,"valid legacy schema v{version}");
        for value in [serde_json::json!(false),serde_json::json!(true),serde_json::Value::Null] {
            let mut altered=clean.clone();altered["paragraphs"][0]["ink_vertical"]=value.clone();
            pdf.get_dictionary_mut(page).unwrap().set(KEY,Object::string_literal(serde_json::to_vec(&altered).unwrap()));
            assert!(read(&pdf,page).is_empty(),"legacy v{version} accepted ink_vertical={value}");
        }
    }
    // A v5 envelope may retain old records as well as explicit false and true.
    let mut mixed=saved.clone();mixed["version"]=serde_json::json!(5);
    let mut disabled=saved["paragraphs"][0].clone();disabled["ink_vertical"]=serde_json::json!(false);
    let mut enabled=saved["paragraphs"][0].clone();enabled["ink_vertical"]=serde_json::json!(true);
    mixed["paragraphs"].as_array_mut().unwrap().extend([disabled,enabled]);
    let mut pdf=original;
    pdf.get_dictionary_mut(page).unwrap().set(KEY,Object::string_literal(serde_json::to_vec(&mixed).unwrap()));
    let records=read(&pdf,page);assert_eq!(records.len(),3);assert_eq!(version(&records),5);
    // An explicitly disabled v5 field still requires a v5 envelope if retained.
    assert_eq!(version(&records[1..2]),5);
    let plain=serde_json::to_value(&records[0]).unwrap();assert!(plain.get("ink_vertical").is_none());
}

    #[test]
    fn direct_and_indirect_font_resources_are_not_amplified_by_palette_size() {
        let runtime=WorkerRuntime::new(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap();
        for indirect in [false,true] {
            let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
            let mut fonts=lopdf::Dictionary::new();for i in 0..5000{fonts.set(format!("F{i}"),font);}
            let resources=dictionary!{"Font"=>fonts};let resources=if indirect{Object::Reference(pdf.add_object(resources))}else{Object::Dictionary(resources)};
            let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>resources});
            pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}.into());let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",catalog);
            let mut before=Vec::new();pdf.clone().save_to(&mut before).unwrap();FontProofs::read(&before).unwrap();
            let definitions=(0..64).map(|i|Style{resource:b"F0".to_vec(),sample:b"A".to_vec(),size:36.,color:[i,30,60,255],background_id:None}).collect::<Vec<_>>();
            let(probes,bindings)=runtime.styled_font_probes(&pdf,page,&definitions).unwrap();assert_eq!(probes.pages().len(),64);assert_eq!(bindings.len(),64);
            for(index,binding)in bindings.iter().enumerate(){assert_eq!(binding.resource_name,b"F0");assert_eq!(binding.run.text,"A");assert_eq!(binding.run.color,[index as u8,30,60,255]);assert!(binding.object_indices.is_empty());}
            let mut after=Vec::new();pdf.clone().save_to(&mut after).unwrap();assert!(before==after,"Probe construction changed the source document");
        }
    }
}
