//! A crop changes only the visible box. Source and object coordinates stay intact.
use super::*;

impl PdfEngine {
    pub(crate) fn validate_page_crop(&self,plan:&PagePlan)->EngineResult<()> {
        let Some(crop)=&plan.crop else{return Ok(());};
        plan.validate_crop()?;
        let crop=in_raw(crop,self.page_unit(&plan.source_id,plan.page_index)?)?;
        let (reply,receive)=mpsc::channel();
        self.send(WorkerRequest::ValidateCrop{source_id:plan.source_id.clone(),page_index:plan.page_index,crop,reply})?;
        receive.recv().map_err(|_|EngineError::WorkerStopped)?
    }
}

pub(super) fn in_raw(crop:&AnnotationRect,unit:f32)->EngineResult<AnnotationRect>{
    // Division rounds once. Multiplication by a rounded reciprocal turns e.g.
    // 255 pt / 1.7 into 149.99998 and can move a rendered edge by one pixel.
    let raw=AnnotationRect{x:crop.x/unit,y:crop.y/unit,width:crop.width/unit,height:crop.height/unit};
    if ![raw.x,raw.y,raw.width,raw.height].iter().all(|v|v.is_finite()){return Err(EngineError::InvalidRequest("Crop coordinates overflow source units.".into()));}
    Ok(raw)
}
impl WorkerRuntime {
    pub(super) fn validate_page_crop(&self,source_id:&str,page_index:usize,crop:&AnnotationRect)->EngineResult<()> {
        let source=self.document(source_id)?;
        if source.for_printing||source.policy.as_ref().is_some_and(|p|!p.protection.permissions().edit_content){return Err(EngineError::PermissionDenied("cropping pages"));}
        let page=source.pdf()?.pages().get(page_index_i32(page_index)?)?;
        let size=page_geometry(&page)?.displayed_size();
        validate(Some(crop),size.width,size.height,source.units()?[page_index].recip())
    }
}

pub(super) fn preview_width(width:f32,height:f32,requested:u32)->EngineResult<i32>{
    let ratio=f64::from(height)/f64::from(width);
    if !ratio.is_finite()||ratio<=0. {return Err(EngineError::InvalidRequest("Preview dimensions are invalid.".into()));}
    let target=f64::from(requested.clamp(64,2400)).min((16_000_000./ratio).sqrt()).min(12_000./ratio).floor();
    if target<1. {return Err(EngineError::InvalidRequest("This page is too narrow to preview safely. Increase the retained width.".into()));}
    Ok(target as i32)
}

pub(crate) fn validate(crop: Option<&AnnotationRect>, width: f32, height: f32, minimum: f32) -> EngineResult<()> {
    let Some(crop) = crop else { return Ok(()); };
    // Only cover f32 roundtrips at the far edge. Publication intersects that
    // edge with the real source box, so tolerance cannot reveal hidden content.
    let tolerance = 4. * f32::EPSILON * width.max(height);
    if !width.is_finite() || !height.is_finite() || width <= 0. || height <= 0.
        || ![crop.x, crop.y, crop.width, crop.height, minimum].iter().all(|v| v.is_finite())
        || minimum <= 0. || crop.x < 0. || crop.y < 0.
        || crop.width < minimum || crop.height < minimum
        || crop.x + crop.width > width + tolerance || crop.y + crop.height > height + tolerance
        || crop.x >= width || crop.y >= height
    {
        return Err(EngineError::InvalidRequest("Crop must fit inside the source page and retain at least 1 pt in each direction.".into()));
    }
    Ok(())
}

pub(super) fn apply(pdf: &mut lopdf::Document, plans: &[PagePlan], geometries: &[PageGeometry]) -> EngineResult<bool> {
    if !plans.iter().any(|p| p.crop.is_some()) { return Ok(false); }
    let pages = pdf.get_pages();
    if pages.len() != plans.len() || geometries.len() != plans.len() {
        return Err(EngineError::InvalidRequest("Crop page mapping changed during composition.".into()));
    }
    for ((page_id, plan), geometry) in pages.values().zip(plans).zip(geometries) {
        apply_page(pdf,*page_id,plan,geometry)?;
    }
    Ok(true)
}

/// Applies a validated source-space crop to one retained original page.
pub(super) fn apply_page(pdf:&mut lopdf::Document,page_id:lopdf::ObjectId,plan:&PagePlan,geometry:&PageGeometry)->EngineResult<()> {
        let Some(crop) = &plan.crop else { return Ok(()); };
        let size = geometry.displayed_size();
        let right = (crop.x + crop.width).min(size.width);
        let bottom = (crop.y + crop.height).min(size.height);
        let corners = [(crop.x, crop.y), (right, crop.y), (crop.x, bottom), (right, bottom)]
            .map(|(x, y)| geometry.displayed_to_pdf(Point { x, y }));
        let left = corners.iter().map(|p| p.0.value).fold(f32::INFINITY, f32::min);
        let right = corners.iter().map(|p| p.0.value).fold(f32::NEG_INFINITY, f32::max);
        let bottom = corners.iter().map(|p| p.1.value).fold(f32::INFINITY, f32::min);
        let top = corners.iter().map(|p| p.1.value).fold(f32::NEG_INFINITY, f32::max);
        let page = pdf.get_dictionary_mut(page_id).map_err(|e| EngineError::InvalidRequest(e.to_string()))?;
        page.set("CropBox", vec![lopdf::Object::Real(left), bottom.into(), right.into(), top.into()]);
    Ok(())
}
