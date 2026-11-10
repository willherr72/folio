//! Public measurements are physical points; PDF workers and persisted PDF records
//! retain raw user-space coordinates. Only source-owned UserUnit chooses a scale.
use super::*;
use lopdf::Document;

fn bad(message:impl std::fmt::Display)->EngineError{EngineError::InvalidRequest(format!("PDF page units: {message}"))}
pub(super) fn read(bytes:&[u8],count:usize)->EngineResult<Vec<f32>>{
    let pdf=crate::request_parse::read(bytes,crate::request_parse::Policy::Compatible64).map_err(bad)?;
    let pages=pdf.get_pages();if pages.len()!=count{return Err(bad("page mapping differs from the renderer"));}
    pages.values().map(|id|page_unit(&pdf,*id)).collect()
}
pub(super) fn page_unit(pdf:&Document,id:lopdf::ObjectId)->EngineResult<f32>{
    let page=pdf.get_dictionary(id).map_err(bad)?;
    // UserUnit is page-local, unlike MediaBox, CropBox, Rotate and Resources.
    let value=match page.get(b"UserUnit"){
        Ok(value)=>pdf.dereference(value).map_err(bad)?.1.as_float().map_err(bad)?,
        Err(_)=>1.,
    };
    if !value.is_finite()||value<=0.||value>75_000.||!value.recip().is_finite(){return Err(bad("unsupported or invalid UserUnit"));}
    Ok(value)
}
// Compare in the raw domain using the same conversion as public requests. This
// keeps exact physical endpoints valid despite f32 multiplication rounding.
pub(super) fn paragraph_size(raw:f32,unit:f32)->bool{
    raw.is_finite()&&unit.is_finite()&&unit>0.&&raw>=unit.recip()&&raw<=300.*unit.recip()
}
fn paragraph_size_in_points(raw:&mut f32,unit:f32)->EngineResult<()>{
    if unit==1. {return Ok(());}
    // Exact inverse-converted policy endpoints must remain valid public values.
    // For example, (1 / 1.7) * 1.7 can otherwise report 0.99999994 pt.
    let endpoint=if *raw==unit.recip(){Some(1.)}else if *raw==300.*unit.recip(){Some(300.)}else{None};
    raw.scale(unit)?;
    if let Some(endpoint)=endpoint {*raw=endpoint;}
    Ok(())
}
// Shaping and UI policy operate in physical points. Only the generated
// appearance is mapped back into raw page coordinates by persistence.
pub(super) fn text_in_points(text:&TextOverlay,unit:f32)->EngineResult<TextOverlay>{
    let mut value=Overlay::Text(text.clone());value.scale(unit)?;
    let Overlay::Text(mut points)=value else{unreachable!()};
    if unit!=1. {
        let endpoint=|raw:f32,physical:f32,limits:&[f32]|limits.iter().copied().find(|limit|raw==*limit*unit.recip()).unwrap_or(physical);
        points.font_size=endpoint(text.font_size,points.font_size,&[1.,512.]);
        if let (Some(raw),Some(area))=(&text.area,&mut points.area){
            area.width=endpoint(raw.width,area.width,&[14400.]);
            area.height=endpoint(raw.height,area.height,&[14400.]);
        }
    }
    Ok(points)
}
fn public_overlay_policy(overlay:&Overlay)->EngineResult<()>{
    match overlay {
        Overlay::Text(text)=>{
            if !text.font_size.is_finite()||text.font_size<=0.||text.font_size>512.{return Err(bad("invalid fontSize"));}
            text.validate_area_settings()?;
        },
        Overlay::Ink(ink)=>if !ink.stroke_width.is_finite()||ink.stroke_width<=0.||ink.stroke_width>100.{return Err(bad("invalid strokeWidth"));},
        _=>(),
    }
    Ok(())
}
fn overlay_in_points(overlay:&mut Overlay,unit:f32)->EngineResult<()>{
    if unit==1. {return Ok(());}
    if let Overlay::Text(text)=overlay {*text=text_in_points(text,unit)?;return Ok(());}
    let maximum_ink=matches!(overlay,Overlay::Ink(ink) if ink.stroke_width==100.*unit.recip());
    overlay.scale(unit)?;
    if maximum_ink {if let Overlay::Ink(ink)=overlay {ink.stroke_width=100.;}}
    Ok(())
}
impl WorkerRuntime {
    pub(super) fn source_page_unit(&self,source_id:&str,page_index:usize)->EngineResult<f32>{
        self.document(source_id)?.units()?.get(page_index).copied().ok_or_else(||bad("page index is outside source"))
    }
}

pub(super) trait Scale {fn scale(&mut self,factor:f32)->EngineResult<()>;}
fn display_size_in_points(size:&mut Option<f32>,unit:f32){
    *size=size.and_then(|raw|{let points=(raw as f64*unit as f64) as f32;(points.is_finite()&&points>0.).then_some(points)});
}
impl Scale for f32 {fn scale(&mut self,factor:f32)->EngineResult<()>{if factor==1.{return Ok(());}let value=*self*factor;if !value.is_finite(){return Err(bad("physical measurement overflow"));}*self=value;Ok(())}}
impl<T:Scale> Scale for Vec<T>{fn scale(&mut self,factor:f32)->EngineResult<()>{for value in self{value.scale(factor)?;}Ok(())}}
impl<T:Scale> Scale for Option<T>{fn scale(&mut self,factor:f32)->EngineResult<()>{if let Some(value)=self{value.scale(factor)?;}Ok(())}}
impl Scale for Point{fn scale(&mut self,f:f32)->EngineResult<()>{self.x.scale(f)?;self.y.scale(f)}}
impl Scale for AnnotationRect{fn scale(&mut self,f:f32)->EngineResult<()>{self.x.scale(f)?;self.y.scale(f)?;self.width.scale(f)?;self.height.scale(f)}}
impl Scale for Overlay{fn scale(&mut self,f:f32)->EngineResult<()>{match self{
    Overlay::Image(v)=>{v.x.scale(f)?;v.y.scale(f)?;v.width.scale(f)?;v.height.scale(f)?;},
    Overlay::Text(v)=>{v.x.scale(f)?;v.y.scale(f)?;v.font_size.scale(f)?;if let Some(a)=&mut v.area{a.width.scale(f)?;a.height.scale(f)?;a.inset.scale(f)?;}},
    Overlay::Ink(v)=>{v.paths.scale(f)?;v.stroke_width.scale(f)?;},
    Overlay::Highlight(v)=>v.rects.scale(f)?,
    Overlay::Comment(v)=>{v.x.scale(f)?;v.y.scale(f)?;},
}Ok(())}}
impl Scale for PageInfo{fn scale(&mut self,f:f32)->EngineResult<()>{self.width.scale(f)?;self.height.scale(f)?;for overlay in &mut self.overlays{overlay_in_points(overlay,f)?;}Ok(())}}
impl Scale for PagePlan{fn scale(&mut self,f:f32)->EngineResult<()>{self.width.scale(f)?;self.height.scale(f)?;self.crop.scale(f)?;self.overlays.scale(f)}}
impl Scale for TextParagraphSelection{fn scale(&mut self,f:f32)->EngineResult<()>{paragraph_size_in_points(&mut self.font_size,f)?;self.bounds.scale(f)}}
impl Scale for TextGroupPreview{fn scale(&mut self,f:f32)->EngineResult<()>{self.font_size.scale(f)?;display_size_in_points(&mut self.display_font_size,f);Ok(())}}
impl Scale for StyledParagraphSelection{fn scale(&mut self,f:f32)->EngineResult<()>{self.baseline_anchor.scale(f)?;for s in &mut self.styles{paragraph_size_in_points(&mut s.font_size,f)?;}self.bounds.scale(f)}}
impl Scale for StyledParagraphLayoutRequest{fn scale(&mut self,f:f32)->EngineResult<()>{self.baseline_anchor.scale(f)?;for s in &mut self.style_changes{s.font_size.scale(f)?;}self.bounds.scale(f)}}
impl Scale for ParagraphFormatting{fn scale(&mut self,f:f32)->EngineResult<()>{self.font_size.scale(f)}}
impl Scale for TextRuns{fn scale(&mut self,f:f32)->EngineResult<()>{for r in &mut self.runs{r.font_size.scale(f)?;display_size_in_points(&mut r.display_font_size,f);r.bounds.scale(f)?;}Ok(())}}
impl Scale for FormParagraphRuns{fn scale(&mut self,f:f32)->EngineResult<()>{for r in &mut self.runs{r.font_size.scale(f)?;display_size_in_points(&mut r.display_font_size,f);r.bounds.scale(f)?;}Ok(())}}
impl Scale for FormTextRuns{fn scale(&mut self,f:f32)->EngineResult<()>{for r in &mut self.runs{r.font_size.scale(f)?;display_size_in_points(&mut r.display_font_size,f);r.bounds.scale(f)?;}Ok(())}}
impl Scale for PageText{fn scale(&mut self,f:f32)->EngineResult<()>{for c in &mut self.characters{c.x.scale(f)?;c.y.scale(f)?;c.width.scale(f)?;c.height.scale(f)?;}Ok(())}}
impl Scale for PageSearchResult{fn scale(&mut self,f:f32)->EngineResult<()>{self.matches.scale(f)}}
impl Scale for crate::FormField{fn scale(&mut self,f:f32)->EngineResult<()>{self.bounds.scale(f)}}

impl PdfEngine {
    pub(super) fn styled_in_raw(&self,source_id:&str,page_index:usize,request:&StyledParagraphLayoutRequest)->EngineResult<StyledParagraphLayoutRequest>{
        self.styled_in_raw_mode(source_id,page_index,request,false)
    }
    pub(super) fn styled_in_raw_mode(&self,source_id:&str,page_index:usize,request:&StyledParagraphLayoutRequest,occurrence:bool)->EngineResult<StyledParagraphLayoutRequest>{
        if request.style_changes.iter().filter_map(|s|s.font_size).any(|size|!paragraph_size(size,1.)){return Err(bad("Use a font size from 1 to 300 pt."));}
        let unit=self.page_unit(source_id,page_index)?;
        let mut raw=request.clone();raw.scale(unit.recip())?;
        if unit!=1. {
            let checked=if occurrence {self.inspect_occurrence_styled_raw(source_id,page_index,&request.object_indices)?}else{
            let (reply,receive)=mpsc::channel();
            self.send(WorkerRequest::InspectStyledParagraph{source_id:source_id.into(),page_index,object_indices:request.object_indices.clone(),reply})?;
            receive.recv().map_err(|_|EngineError::WorkerStopped)??};
            restore_rect(&mut raw.bounds,&request.bounds,&checked.bounds,unit);
            restore_anchor(&mut raw.baseline_anchor,request.baseline_anchor,checked.baseline_anchor,unit);
            for (change,submitted) in raw.style_changes.iter_mut().zip(&request.style_changes){
                if let Some(source)=checked.styles.iter().find(|s|s.style_id==submitted.source_style_id){
                    if let (Some(value),Some(physical))=(&mut change.font_size,submitted.font_size){restore_value(value,physical,source.font_size,unit);}
                }
            }
        }
        Ok(raw)
    }
    pub(super) fn uniform_in_raw(&self,source_id:&str,page_index:usize,indices:&[usize],bounds:&AnnotationRect,formatting:&ParagraphFormatting)->EngineResult<(AnnotationRect,ParagraphFormatting)>{
        self.uniform_in_raw_mode(source_id,page_index,indices,bounds,formatting,false)
    }
    pub(super) fn uniform_in_raw_mode(&self,source_id:&str,page_index:usize,indices:&[usize],bounds:&AnnotationRect,formatting:&ParagraphFormatting,occurrence:bool)->EngineResult<(AnnotationRect,ParagraphFormatting)>{
        let unit=self.page_unit(source_id,page_index)?;
        let mut raw_bounds=bounds.clone();raw_bounds.scale(unit.recip())?;
        let mut raw_format=formatting.clone();raw_format.scale(unit.recip())?;
        if unit!=1. {
            let checked=if occurrence {self.inspect_occurrence_text_raw(source_id,page_index,indices)?}else{
            let (reply,receive)=mpsc::channel();
            self.send(WorkerRequest::InspectTextParagraph{source_id:source_id.into(),page_index,object_indices:indices.to_vec(),reply})?;
            receive.recv().map_err(|_|EngineError::WorkerStopped)??};
            restore_rect(&mut raw_bounds,bounds,&checked.bounds,unit);
            if let (Some(value),Some(physical))=(&mut raw_format.font_size,formatting.font_size){restore_value(value,physical,checked.font_size,unit);}
        }
        Ok((raw_bounds,raw_format))
    }
    pub(crate) fn recovery_page_in_points(&self,source_id:&str,mut page:PagePlan)->EngineResult<PagePlan>{
        let unit=self.page_unit(source_id,page.page_index)?;page.width.scale(unit)?;page.height.scale(unit)?;page.crop.scale(unit)?;
        for overlay in &mut page.overlays{overlay_in_points(overlay,unit)?;}Ok(page)
    }
    pub(super) fn outcome_in_points(&self,result:EngineResult<OpenOutcome>)->EngineResult<OpenOutcome>{match result?{OpenOutcome::Opened{document}=>self.document_in_points(Ok(document)).map(|document|OpenOutcome::Opened{document}),other=>Ok(other)}}
    pub(super) fn replacements_in_points(&self,result:EngineResult<Vec<SourceReplacement>>)->EngineResult<Vec<SourceReplacement>>{
        let replacements=result?;let ids=replacements.iter().map(|v|v.document.id.clone()).collect::<Vec<_>>();
        let result=replacements.into_iter().map(|r|Ok(SourceReplacement{source_id:r.source_id,document:self.document_in_points(Ok(r.document))?})).collect();
        if let Err(error)=result{for id in ids{let _=self.close_document(&id);}return Err(error);}result
    }
    pub(crate) fn source_units(&self,source_id:&str)->EngineResult<Vec<f32>>{
        let (reply,receive)=mpsc::channel();self.send(WorkerRequest::SourceUnits{source_id:source_id.into(),reply})?;
        receive.recv().map_err(|_|EngineError::WorkerStopped)?
    }
    pub(super) fn page_unit(&self,source_id:&str,page_index:usize)->EngineResult<f32>{self.source_units(source_id)?.get(page_index).copied().ok_or_else(||bad("page index is outside source"))}
    pub(super) fn in_points<T:Scale>(&self,source_id:&str,page_index:usize,result:EngineResult<T>)->EngineResult<T>{let mut value=result?;value.scale(self.page_unit(source_id,page_index)?)?;Ok(value)}
    pub(super) fn in_raw<T:Scale+Clone>(&self,source_id:&str,page_index:usize,value:&T)->EngineResult<T>{let mut value=value.clone();value.scale(self.page_unit(source_id,page_index)?.recip())?;Ok(value)}
    pub(super) fn document_in_points(&self,result:EngineResult<DocumentInfo>)->EngineResult<DocumentInfo>{
        let mut info=result?;
        let converted=(||{let units=self.source_units(&info.id)?;if units.len()!=info.pages.len(){return Err(bad("document page count changed"));}for(page,unit)in info.pages.iter_mut().zip(units){page.scale(unit)?;}Ok(())})();
        if let Err(error)=converted{let _=self.close_document(&info.id);return Err(error);}Ok(info)
    }
    pub(super) fn export_in_raw(&self,request:ExportRequest)->EngineResult<ExportRequest>{
        self.export_in_raw_checked(request, |_| Ok(()))
    }
    pub(super) fn export_in_raw_checked(&self,mut request:ExportRequest,mut check:impl FnMut(usize)->EngineResult<()>)->EngineResult<ExportRequest>{
        let mut units=HashMap::new();
        for (index,page) in request.pages.iter_mut().enumerate(){
            check(index)?;
            page.validate_crop()?;
            for overlay in &page.overlays{public_overlay_policy(overlay)?;}
            if !units.contains_key(&page.source_id){units.insert(page.source_id.clone(),self.source_units(&page.source_id)?);}
            let unit=*units[&page.source_id].get(page.page_index).ok_or_else(||bad("page index is outside source"))?;
            let crop=page.crop.take();page.scale(unit.recip())?;
            page.crop=crop.map(|crop|page_crop::in_raw(&crop,unit)).transpose()?;
        }
        check(request.pages.len())?;
        Ok(request)
    }
}
// Reuse a raw value only when the submitted measurement is exactly the public
// representation of an independently inspected source selection. No tolerance or
// caller-supplied PDF metadata can turn an intentional coordinate change into a
// no-op. Ordinary edits retain the regular inverse conversion.
pub(super) fn restore_anchor(raw:&mut Option<Point>,submitted:Option<Point>,checked:Option<Point>,unit:f32){
    if let (Some(raw),Some(submitted),Some(checked))=(raw,submitted,checked){
        restore_value(&mut raw.x,submitted.x,checked.x,unit);
        restore_value(&mut raw.y,submitted.y,checked.y,unit);
    }
}
pub(super) fn restore_value(raw:&mut f32,submitted:f32,checked:f32,unit:f32){
    if submitted==checked*unit {*raw=checked;}
}
pub(super) fn restore_rect(raw:&mut AnnotationRect,submitted:&AnnotationRect,checked:&AnnotationRect,unit:f32){
    restore_value(&mut raw.x,submitted.x,checked.x,unit);
    restore_value(&mut raw.y,submitted.y,checked.y,unit);
    restore_value(&mut raw.width,submitted.width,checked.width,unit);
    restore_value(&mut raw.height,submitted.height,checked.height,unit);
}
impl OpenDocument{
    pub(super) fn units(&self)->EngineResult<&[f32]>{{ if self.physical_units.get().is_none() { let bytes=self.bytes()?; let _=self.physical_units.set(read(&bytes,self.document.page_count()).map_err(|e|e.to_string())); } self.physical_units.get().expect("initialized source units") }.as_ref().map(Vec::as_slice).map_err(bad)}
}

#[cfg(test)]mod tests{
    use super::*;use lopdf::{dictionary,Object};
    #[test]fn unrepresentable_display_size_does_not_reject_other_public_measurements(){
        let mut group=TextGroupPreview{object_indices:vec![0,1],text:"Old words".into(),font_name:"Helvetica".into(),font_display_name:None,font_size:1.,display_font_size:Some(f32::MAX)};
        group.scale(2.).unwrap();assert_eq!(group.font_size,2.);assert_eq!(group.display_font_size,None);assert_eq!(group.object_indices,vec![0,1]);
    }
    #[test]fn units_are_local_indirect_and_bound_to_page_order(){
        let mut d=Document::with_version("1.7");let pages=d.new_object_id();let unit=d.add_object(Object::Real(2.));let a=d.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"UserUnit"=>unit});let b=d.add_object(dictionary!{"Type"=>"Page","Parent"=>pages});d.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![a.into(),b.into()],"Count"=>2,"UserUnit"=>9}.into());let cat=d.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});d.trailer.set("Root",cat);let mut bytes=vec![];d.save_to(&mut bytes).unwrap();assert_eq!(read(&bytes,2).unwrap(),vec![2.,1.]);assert!(read(&bytes,1).is_err());
        for value in [Object::Real(0.),Object::Integer(-1),Object::Real(75_001.),Object::Name(b"bad".to_vec())]{d.get_dictionary_mut(a).unwrap().set("UserUnit",value);bytes.clear();d.save_to(&mut bytes).unwrap();assert!(read(&bytes,2).is_err());}
    }
    #[test]fn restoration_requires_exact_public_value_and_preserves_intentional_changes(){
        let checked=90.99999_f32;let unit=1.1_f32;let shown=checked*unit;
        let mut raw=shown*unit.recip();assert_ne!(raw,checked);
        restore_value(&mut raw,shown,checked,unit);assert_eq!(raw,checked);
        for submitted in [f32::from_bits(shown.to_bits()+1),shown+0.125] {
            let expected=submitted*unit.recip();let mut raw=expected;
            restore_value(&mut raw,submitted,checked,unit);assert_eq!(raw,expected);
        }
    }
    #[test]fn lengths_scale_but_ratios_and_identity_stay_unchanged(){
        let mut value=Overlay::Text(TextOverlay{id:"x".into(),x:10.,y:20.,text:"A".into(),font_size:18.,font_name:TextFont::Helvetica,font_id:Some("font".into()),shaping:None,area:Some(TextAreaSettings{version:1,width:200.,height:100.,inset:5.,line_spacing:1.5,alignment:TextAreaAlignment::Left}),color:"#123456".into(),rotation:90});let original=value.clone();value.scale(2.).unwrap();let Overlay::Text(t)=&value else{panic!()};assert_eq!((t.x,t.y,t.font_size),(20.,40.,36.));assert_eq!(t.area.as_ref().unwrap().line_spacing,1.5);assert_eq!(t.rotation,90);value.scale(0.5).unwrap();assert_eq!(value,original);
    }
}
