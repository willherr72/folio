//! Copy changed/new values and flattened kept drawings after exact draft proof.
use super::*;
use crate::engine::imported_overlay_sources::publication::{OwnedChange,PageChanges};
use std::collections::BTreeMap;

struct Budget{bytes:usize,steps:usize}
impl Budget {
    fn take(&mut self,count:usize,size:usize)->EngineResult<()> {self.bytes=self.bytes.checked_sub(count.checked_mul(size).ok_or_else(||bad("selection allocation overflow"))?).ok_or_else(||bad("selection metadata limit"))?;Ok(())}
    fn step(&mut self,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<()> {check()?;self.steps=self.steps.checked_sub(1).ok_or_else(||bad("selection traversal limit"))?;Ok(())}
    fn overlay(&mut self,overlay:&Overlay,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<()> {
        self.step(check)?;self.take(1,std::mem::size_of::<Overlay>())?;
        match overlay {
            Overlay::Text(v)=>{for s in [&v.id,&v.text,&v.color]{self.take(s.len(),1)?;}if let Some(id)=&v.font_id{self.take(id.len(),1)?;}},
            Overlay::Image(v)=>{self.take(v.id.len(),1)?;self.take(v.asset_id.len(),1)?;},
            Overlay::Comment(v)=>{for s in [&v.id,&v.text,&v.color]{self.take(s.len(),1)?;}},
            Overlay::Highlight(v)=>{self.take(v.id.len(),1)?;self.take(v.color.len(),1)?;if let Some(s)=&v.text{self.take(s.len(),1)?;}self.take(v.rects.len(),std::mem::size_of::<AnnotationRect>())?;},
            Overlay::Ink(v)=>{self.take(v.id.len(),1)?;self.take(v.color.len(),1)?;self.take(v.paths.len(),std::mem::size_of::<Vec<Point>>())?;for path in &v.paths{self.step(check)?;self.take(path.len(),std::mem::size_of::<Point>())?;}},
        }Ok(())
    }
}

fn selected_request(request:&ExportRequest,changes:&[&PageChanges<'_,'_>],bytes:usize,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<ExportRequest> {
    let mut budget=Budget{bytes,steps:1_000_000};budget.step(check)?;
    if request.pages.len()!=changes.len() || request.pages.is_empty() || request.pages.len()>10_000{return Err(bad("selection page count differs"));}
    budget.take(request.pages.len(),std::mem::size_of::<PagePlan>())?;
    let mut pages=Vec::with_capacity(request.pages.len());
    for (plan,changes) in request.pages.iter().zip(changes) {
        budget.step(check)?;budget.take(plan.id.len(),1)?;budget.take(plan.source_id.len(),1)?;
        budget.take(changes.owned.len().checked_add(changes.added.len()).ok_or_else(||bad("selection count overflow"))?,96)?;
        let mut wanted=BTreeMap::new();
        for change in &changes.owned {
            budget.step(check)?;
            let source=match change{OwnedChange::Keep(s,_)|OwnedChange::Replace(s,_)|OwnedChange::Delete(s)=>s};
            if source.page_index!=plan.page_index{return Err(bad("selection source page differs"));}
            if source.overlay_id.len()>1024 || source.overlay_id.trim().is_empty(){return Err(bad("invalid source selection identity"));}
            budget.take(source.overlay_id.len(),1)?;
            let selected=match change{OwnedChange::Replace(_,overlay)=>Some(*overlay),OwnedChange::Keep(_,overlay) if request.flatten&&matches!(overlay,Overlay::Text(_)|Overlay::Ink(_)|Overlay::Image(_))=>Some(*overlay),_=>None};
            if let Some(overlay)=selected {if overlay_id(overlay)!=source.overlay_id || wanted.insert(source.overlay_id,overlay).is_some(){return Err(bad("ambiguous replacement selection"));}}
        }
        for overlay in &changes.added {budget.step(check)?;let id=overlay_id(overlay);if id.len()>1024 || id.trim().is_empty(){return Err(bad("invalid added selection identity"));}budget.take(id.len(),1)?;if wanted.insert(id,*overlay).is_some(){return Err(bad("ambiguous added selection"));}}
        if wanted.len()>10_000 || plan.overlays.len()>10_000{return Err(bad("selection overlay limit"));}
        budget.take(wanted.len(),std::mem::size_of::<&Overlay>())?;
        let mut selected=Vec::with_capacity(wanted.len());
        for overlay in &plan.overlays {
            budget.step(check)?;
            if let Some(bound)=wanted.remove(overlay_id(overlay)) {
                if !std::ptr::eq(bound,overlay){return Err(bad("selection belongs to another draft"));}
                budget.overlay(overlay,check)?;selected.push(overlay);
            }
        }
        if !wanted.is_empty(){return Err(bad("selected overlay is absent from draft"));}
        // Every nested vector/string in these clones was admitted above.
        let mut overlays=Vec::with_capacity(selected.len());
        for overlay in selected{budget.step(check)?;overlays.push(overlay.clone());}
        pages.push(PagePlan{id:plan.id.clone(),source_id:plan.source_id.clone(),page_index:plan.page_index,width:plan.width,height:plan.height,rotation:plan.rotation,crop:plan.crop.clone(),overlays});
    }
    budget.step(check)?;Ok(ExportRequest{flatten:false,pages})
}

impl WorkerRuntime {
    pub(in crate::engine) fn prepare_structured_overlay_changes(&self,request:&ExportRequest,changes:&[&PageChanges<'_,'_>],check:&mut impl FnMut()->EngineResult<()>)->EngineResult<PreparedOverlays> {
        let selected=selected_request(request,changes,MAX_METADATA_BYTES,check)?;
        self.prepare_structured_overlays(&selected,check)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn structured_overlay_batch_flatten_selection_includes_proved_unchanged_drawings(){
        use crate::engine::imported_overlay_sources::publication::AnnotationSource;
        let mut rt=crate::engine::structured_overlay_render::tests::runtime();let info=crate::engine::structured_overlay_render::tests::source(&mut rt,0,1.,b"");
        let mut request=crate::engine::structured_overlay_render::tests::request(&rt,&info);request.flatten=true;
        let changes=PageChanges{draft:&request.pages[0].overlays,owned:request.pages[0].overlays.iter().enumerate().map(|(slot,overlay)|OwnedChange::Keep(AnnotationSource{page_index:0,annotation_index:slot,annotation_id:Some((100+slot as u32,0)),popup_indices:&[],overlay_id:overlay_id(overlay)},overlay)).collect(),added:vec![]};
        let selected=selected_request(&request,&[&changes],MAX_METADATA_BYTES,&mut ||Ok(())).unwrap();assert!(!selected.flatten);
        assert_eq!(selected.pages[0].overlays.iter().map(overlay_id).collect::<Vec<_>>(),vec!["text","ink","image"]);
        assert!(selected_request(&request.clone(),&[&changes],MAX_METADATA_BYTES,&mut ||Ok(())).is_err(),"unchanged flatten appearances must belong to the same draft");
    }
    #[test]
    fn structured_overlay_batch_selection_rejects_foreign_draft_and_bounds_nested_copies() {
        let mut rt=crate::engine::structured_overlay_render::tests::runtime();let info=crate::engine::structured_overlay_render::tests::source(&mut rt,0,1.,b"");
        let request=crate::engine::structured_overlay_render::tests::request(&rt,&info);
        let changes=PageChanges{draft:&request.pages[0].overlays,owned:vec![],added:request.pages[0].overlays.iter().collect()};
        let selected=selected_request(&request,&[&changes],MAX_METADATA_BYTES,&mut ||Ok(())).unwrap();assert_eq!(selected.pages[0].overlays,request.pages[0].overlays);
        assert!(selected_request(&request.clone(),&[&changes],MAX_METADATA_BYTES,&mut ||Ok(())).is_err());
        assert!(selected_request(&request,&[&changes],128,&mut ||Ok(())).is_err());
        let keep=PageChanges{draft:&[],owned:vec![],added:vec![]};assert!(selected_request(&request,&[&keep],MAX_METADATA_BYTES,&mut ||Ok(())).unwrap().pages[0].overlays.is_empty());
        let mut large=request.clone();large.pages[0].overlays=vec![Overlay::Ink(InkOverlay{id:"large".into(),paths:vec![vec![Point{x:5.,y:5.},Point{x:8.,y:8.}];1000],color:"#112233".into(),stroke_width:2.})];
        let large_changes=PageChanges{draft:&large.pages[0].overlays,owned:vec![],added:large.pages[0].overlays.iter().collect()};
        assert!(selected_request(&large,&[&large_changes],10_000,&mut ||Ok(())).is_err(),"nested path-vector storage must be reserved before copying");
        assert!(selected_request(&large,&[&keep],1024,&mut ||Ok(())).unwrap().pages[0].overlays.is_empty(),"an unselected drawing must not consume the copy allowance");
        let mut calls=0;selected_request(&request,&[&changes],MAX_METADATA_BYTES,&mut ||{calls+=1;Ok(())}).unwrap();
        for stop in [1,calls/2,calls]{let mut seen=0;assert!(selected_request(&request,&[&changes],MAX_METADATA_BYTES,&mut ||{seen+=1;if seen==stop{Err(bad("cancelled"))}else{Ok(())}}).is_err());}
    }
}
