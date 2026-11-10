//! Shared source/font admission and measurement for attributed layout/publication.
use super::*;
use styled_paragraph_layout::{GlyphMetric,StyleMetrics};

impl Prepared {
    fn source_baseline(&self)->Option<(f32,f32)>{
        let index=self.selection.object_indices.first()?;
        let run=self.before.iter().find(|run|run.index==*index)?;
        Some((run.matrix[4],self.first_baseline.unwrap_or(run.matrix[5])))
    }
    pub(super) fn baseline_anchor(&self)->Option<Point>{
        self.source_baseline().map(|(x,y)|self.geometry.pdf_to_displayed(x,y))
    }
    fn requested_baseline(&self,anchor:Option<Point>)->EngineResult<Option<f32>>{
        let Some(anchor)=anchor else{return Ok(None);};
        if [anchor.x,anchor.y].iter().any(|v|!v.is_finite()||v.abs()>1_000_000.){
            return Err(invalid("Use a finite paragraph baseline inside the page."));
        }
        // Subtracting the crop origin and reversing rotation can lose one f32
        // ulp. Preserve the source value when its relevant displayed coordinate
        // is exactly unchanged; the other axis does not position this baseline.
        if let (Some(original),Some((_,baseline)))=(self.baseline_anchor(),self.source_baseline()){
            let same=match self.geometry.rotation {0|180=>anchor.y==original.y,_=>anchor.x==original.x};
            if same{return Ok(Some(baseline));}
        }
        Ok(Some(self.geometry.displayed_to_pdf(anchor).1.value))
    }
}

#[derive(Clone)]
pub(super) struct ResolvedStyle {
    pub source:RunSnapshot,
    pub resource_name:Vec<u8>,
    pub size:f32,
    pub color:[u8;4],
    pub substitute:Option<Arc<FontAsset>>,
    pub background_id:Option<usize>,
}
impl ResolvedStyle {
    pub(super) fn original(style:&BoundStyle)->Self {
        Self{source:style.run.clone(),resource_name:style.resource_name.clone(),size:style.run.font_size*style.run.matrix[0],color:style.run.color,substitute:None,background_id:style.background_id}
    }
    pub fn encode(&self,text:&str)->EngineResult<Vec<u8>> {
        if let Some(asset)=&self.substitute {asset.validate_text(text)?;Ok(paragraph_font::encode(text))}
        else {positioned_text::encoded(&self.source,text)}
    }
    pub fn advance(&self,ch:char)->EngineResult<f32> {
        Ok(if let Some(asset)=&self.substitute {paragraph_font::integral_advance(asset,ch)?}else{advance(&self.source,&ch.to_string())?}*self.size)
    }
}
pub(super) struct ResolvedPlan {
    pub prepared:Prepared,
    pub original:image::RgbaImage,
    pub layout:StyledParagraphLayout,
    pub styles:Vec<ResolvedStyle>,
}

pub(super) struct MeasuredPlan {
    pub prepared:Prepared,
    pub original:image::RgbaImage,
    pub styles:Vec<ResolvedStyle>,
    metrics:Vec<StyleMetrics>,
    padded:Option<Vec<StyleMetrics>>,
}
impl MeasuredPlan {
    pub(super) fn anchored_fit_bounds(&self,request:&StyledParagraphLayoutRequest,mut bounds:[f32;4])->EngineResult<[f32;4]>{
        if let Some(baseline)=self.prepared.requested_baseline(request.baseline_anchor)?{
            let reservation=self.padded.as_deref().unwrap_or(&self.metrics);
            let text=if self.prepared.text_alignment{self.metrics.as_slice()}else{reservation};
            let inner=paragraph_highlight_geometry::text_bounds(bounds,self.prepared.text_insets,self.prepared.text_alignment);
            let ascent=styled_paragraph_layout::first_line_ascent_with_text_metrics(&request.runs,reservation,text,inner[2]-inner[0],self.prepared.unit,self.prepared.ink_vertical)?;
            bounds[3]=bounds[3].max(baseline+ascent);
        }
        Ok(bounds)
    }
    pub(super) fn layout(&self,request:&StyledParagraphLayoutRequest,bounds:[f32;4])->EngineResult<StyledParagraphLayout>{
        self.prepared.check_context_area(bounds)?;
        let baseline=self.prepared.requested_baseline(request.baseline_anchor)?;
        let layout=paragraph_highlight_geometry::layout_with_baseline(&request.runs,&self.metrics,self.padded.as_deref(),bounds,request.alignment,request.line_spacing,self.prepared.unit,self.prepared.text_insets,self.prepared.text_alignment,self.prepared.ink_vertical,baseline)?;
        if !self.prepared.backgrounds.is_empty(){let ids=self.styles.iter().map(|s|s.background_id).collect::<Vec<_>>();paragraph_highlight_geometry::rectangles_in_units(&layout,&self.metrics,&ids,&self.prepared.backgrounds,self.prepared.unit)?;}
        Ok(layout)
    }
}

/// Compare attributed characters, not wire run segmentation or dense style IDs.
/// Both palettes already resolve through this page's independently checked font
/// resources. An explicit substitution is an edit; display names are no authority.
fn same_attribution(prepared:&Prepared,runs:&[StyledParagraphRun],styles:&[ResolvedStyle])->bool{
    let equivalent=styles.iter().map(|style|prepared.styles.iter().map(|source|
        style.substitute.is_none()&&style.resource_name==source.resource_name
        &&style.source.program==source.run.program&&style.source.font==source.run.font
        &&style.size==source.run.font_size*source.run.matrix[0]
        &&style.color==source.run.color&&style.background_id==source.background_id
    ).collect::<Vec<_>>()).collect::<Vec<_>>();
    let mut original=prepared.runs.iter().flat_map(|r|r.text.chars().map(move|ch|(ch,r.style_id)));
    for (ch,id) in runs.iter().flat_map(|r|r.text.chars().map(move|ch|(ch,r.style_id))){
        let Some((before,source_id))=original.next()else{return false;};
        if ch!=before||!equivalent[id][source_id]{return false;}
    }
    original.next().is_none()
}

impl WorkerRuntime {
    pub(in crate::engine) fn layout_styled_paragraph(&self,source_id:&str,page_index:usize,request:&StyledParagraphLayoutRequest)->EngineResult<StyledParagraphLayout> {
        self.prepare_styled_paragraph(source_id,page_index,request).map(|plan|plan.layout)
    }
    pub(super) fn prepare_styled_paragraph(&self,source_id:&str,page_index:usize,request:&StyledParagraphLayoutRequest)->EngineResult<ResolvedPlan> {
        let measured=self.measure_styled_paragraph(source_id,page_index,request)?;
        let layout=measured.layout(request,pdf_bounds(measured.prepared.geometry,&request.bounds))?;
        Ok(ResolvedPlan{prepared:measured.prepared,original:measured.original,layout,styles:measured.styles})
    }
    pub(super) fn measure_styled_paragraph(&self,source_id:&str,page_index:usize,request:&StyledParagraphLayoutRequest)->EngineResult<MeasuredPlan> {
        styled_paragraph_layout::validate_draft(&request.runs)?;
        if request.style_changes.len()>64 {return Err(invalid("Use at most 64 paragraph styles."));}
        let (mut prepared,original)=self.paragraph_baseline_mode(source_id,page_index,&request.object_indices,true)?;
        if request.expected_text!=prepared.selection.text {return Err(invalid("The text changed since it was selected. Check the selection again."));}
        let mut slots:Vec<_>=prepared.styles.iter().map(|s|Some(ResolvedStyle::original(s))).collect();
        let mut targets=HashSet::new();
        for change in &request.style_changes {
            if change.style_id>=64||!targets.insert(change.style_id) {return Err(invalid("Paragraph style targets must be unique and below 64."));}
            let source=prepared.styles.get(change.source_style_id).ok_or_else(||invalid("A paragraph style is not part of the checked source selection."))?;
            // Always resolve from the immutable source palette: change order has
            // no effect, and derived entries cannot authorize another binding.
            let mut style=ResolvedStyle::original(source);
            if let Some(size)=change.font_size {
                if !physical_units::paragraph_size(size,prepared.unit){return Err(invalid("Use a font size from 1 to 300 pt."));}
                style.size=size;
            }
            if let Some(color)=&change.color {let rgb=parse_color(color)?;style.color=[rgb.red(),rgb.green(),rgb.blue(),255];}
            style.substitute=change.font_id.as_deref().map(|id|self.fonts.get(id)).transpose()?;
            if slots.len()<=change.style_id {slots.resize_with(change.style_id+1,||None);}
            slots[change.style_id]=Some(style);
        }
        let styles:Vec<_>=slots.into_iter().map(|s|s.ok_or_else(||invalid("New paragraph styles must use consecutive IDs."))).collect::<EngineResult<_>>()?;
        if request.runs.iter().any(|r|r.style_id>=styles.len()) {return Err(invalid("A paragraph style is not part of the checked source selection."));}
        let bounds=&request.bounds;let page_size=prepared.geometry.displayed_size();
        if ![bounds.x,bounds.y,bounds.width,bounds.height].iter().all(|v|v.is_finite())
            ||bounds.x<0.||bounds.y<0.||bounds.width<=0.||bounds.height<=0.
            ||bounds.x+bounds.width>page_size.width||bounds.y+bounds.height>page_size.height {
            return Err(invalid("Keep the paragraph area inside the page."));
        }
        if !request.line_spacing.is_finite()||!(0.6..=3.).contains(&request.line_spacing) {return Err(invalid("Use line spacing from 0.6 to 3."));}
        let definitions=styles.iter().map(styled_paragraph_memory::Style::source).collect::<EngineResult<Vec<_>>>()?;
        let (probes,_)=self.styled_font_probes(&prepared.pdf,prepared.page_id,&definitions)?;
        
        let mut metrics=measure(&probes,&request.runs,&styles)?;
        // Legacy no-op drafts retain their exact measurement policy. New or
        // explicitly changed marked full-font drafts use independently measured ink.
        prepared.native_ink|=styles.iter().any(|s|(prepared.ink_vertical||s.background_id.is_some())&&styled_paragraph_native_metrics::full_font(s))
            &&(prepared.ink_vertical||!prepared.remembered||!same_attribution(&prepared,&request.runs,&styles));
        if prepared.native_ink{self.native_highlight_metrics(&prepared,&request.runs,&styles,&mut metrics,prepared.ink_vertical)?;}
        let ids=styles.iter().map(|s|s.background_id).collect::<Vec<_>>();
        if prepared.text_alignment&&prepared.backgrounds.iter().any(|b|b.cell.is_some()){
            prepared.text_insets=paragraph_highlight_geometry::style_insets(&prepared.backgrounds,styles.iter().map(|s|(s.background_id,s.size)))?;
        }
        let padded=if prepared.backgrounds.is_empty(){None}else{Some(paragraph_highlight_geometry::padded_metrics_in_units(&metrics,&ids,&prepared.backgrounds,prepared.unit)?)};
        Ok(MeasuredPlan{prepared,original,styles,metrics,padded})
    }
}

/// Shared measurement for fresh layout and independent memory replay. The probe
/// document contains one isolated verified resource sample per page, in ID order.
pub(super) fn measure(document:&PdfDocument<'_>,runs:&[StyledParagraphRun],styles:&[ResolvedStyle])->EngineResult<Vec<StyleMetrics>> {

        let mut metrics=Vec::new();
        for (style_id,style) in styles.iter().enumerate(){
            let size=style.size;
            if let Some(asset)=&style.substitute {
                let face=asset.face()?;let scale=size/face.units_per_em() as f32;
                let mut glyphs=HashMap::new();
                for ch in runs.iter().filter(|r|r.style_id==style_id).flat_map(|r|r.text.chars()).filter(|c|*c!='\n') {
                    if glyphs.contains_key(&ch){continue;}
                    if !embedded_font::latin(&format!("a{ch}")){return Err(invalid("This paragraph supports unshaped Latin text only."));}
                    let glyph=asset.glyph_id(ch)?;let bbox=face.glyph_bounding_box(ttf_parser::GlyphId(glyph));
                    let ink=if ch==' ' {[0.;4]}else{bbox.map_or([0.;4],|b|[b.x_min as f32*scale,b.y_min as f32*scale,b.x_max as f32*scale,b.y_max as f32*scale])};
                    glyphs.insert(ch,GlyphMetric{advance:style.advance(ch)?,ink});
                }
                metrics.push(StyleMetrics{em:size,ascent:face.ascender() as f32*scale,descent:-(face.descender() as f32)*scale,glyphs});
                continue;
            }
            let run=&style.source;
            let page=document.pages().get(style_id as i32)?;
            let object=page.objects().get(0)?;
            let text=object.as_text_object().ok_or_else(||invalid("Missing paragraph font."))?;
            let font=text.font();let token=font.token();
            // Our integral full-font resource was initially laid out with the
            // TrueType extents. Reuse those verified program metrics on re-edit,
            // rather than PDFium's integer-rounded descriptor/glyph metrics.
            if run.program.as_ref().is_some_and(|id|run.font==format!("Folio{id}Paragraph"))&&run.proof.is_some(){
                let bytes=font.data()?;let face=ttf_parser::Face::parse(&bytes,0).map_err(|_|invalid("Invalid saved paragraph font."))?;
                let scale=size/face.units_per_em() as f32;let proof=run.proof.as_ref().unwrap();let mut glyphs=HashMap::new();
                for ch in runs.iter().filter(|r|r.style_id==style_id).flat_map(|r|r.text.chars()).filter(|c|*c!='\n'){
                    if glyphs.contains_key(&ch){continue;}let glyph=proof.glyph(ch).ok_or_else(||invalid("Missing saved paragraph glyph."))?;
                    let ink=if ch==' ' {[0.;4]}else{face.glyph_bounding_box(ttf_parser::GlyphId(glyph)).map_or([0.;4],|b|[b.x_min as f32*scale,b.y_min as f32*scale,b.x_max as f32*scale,b.y_max as f32*scale])};
                    glyphs.insert(ch,GlyphMetric{advance:style.advance(ch)?,ink});
                }
                metrics.push(StyleMetrics{em:size,ascent:face.ascender() as f32*scale,descent:-(face.descender() as f32)*scale,glyphs});continue;
            }
            let ascent=font.ascent(PdfPoints::new(size))?.value;
            let descent=-font.descent(PdfPoints::new(size))?.value;
            let mut glyphs=HashMap::new();
            for ch in runs.iter().filter(|r|r.style_id==style_id).flat_map(|r|r.text.chars()).filter(|c|*c!='\n'){
                if glyphs.contains_key(&ch){continue;}
                let value=ch.to_string();
                // Prove that the writer can encode this exact glyph using
                // the bound source font; display metadata is not sufficient.
                positioned_text::encoded(run,&value)?;
                let width=advance(run,&value)?*size;
                let probe=PdfPageTextObject::new(document,&value,token,PdfPoints::new(size))?;
                let rect=probe.bounds()?;
                let ink=if ch==' ' {[0.;4]}else{[rect.left().value,rect.bottom().value,rect.right().value,rect.top().value]};
                glyphs.insert(ch,GlyphMetric{advance:width,ink});
            }
            metrics.push(StyleMetrics{em:size,ascent,descent,glyphs});
        }
        Ok(metrics)
}
