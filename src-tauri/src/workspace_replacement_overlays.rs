use super::*;
use super::super::search_matching::{SearchMatches,whitespace};

fn rotate(text:&TextOverlay,r:&mut AnnotationRect){
    let(x,y,w,h)=(r.x,r.y,r.width,r.height);
    *r=match text.rotation{
        90=>AnnotationRect{x:text.x-y-h,y:text.y+x,width:h,height:w},
        180=>AnnotationRect{x:text.x-x-w,y:text.y-y-h,width:w,height:h},
        270=>AnnotationRect{x:text.x+y,y:text.y-x-w,width:h,height:w},
        _=>AnnotationRect{x:text.x+x,y:text.y+y,width:w,height:h},
    };
}
impl WorkerRuntime{
    fn workspace_overlay_characters(&self,text:&TextOverlay,page:&PagePlan)->EngineResult<Vec<PdfTextCharacter>>{
        let custom=text.font_id.as_deref().map(|id|self.fonts.get(id)).transpose()?;
        validate_text(text,page.width,page.height,custom.as_deref())?;
        if text.text.chars().count()>250_000{return Err(invalid());}
        let mut characters=if text.shaping.is_some(){
            #[cfg(feature="shaped-text")]
            {crate::prepare_text_overlay(custom.as_deref().ok_or_else(invalid)?,text)?.characters}
            #[cfg(not(feature="shaped-text"))]
            {return Err(invalid());}
        }else{
            let mut chars=Vec::new();let mut x=0.;let mut y=0.;
            for ch in text.text.chars(){
                let delimiter=matches!(ch,'\n'|'\r');let value=ch.to_string();
                let width=if delimiter{0.}else{custom.as_ref().map_or_else(||super::super::persistence::text_advance(&value,text.font_name),|font|font.advance(ch))?*text.font_size};
                chars.push(PdfTextCharacter{text:value,x,y,width,height:text.font_size*1.2});
                if ch=='\n'{x=0.;y+=text.font_size*1.2;}else{x+=width;}
            }chars
        };
        if characters.iter().map(|c|c.text.as_str()).collect::<String>()!=text.text{return Err(invalid());}
        for c in &mut characters{let mut r=AnnotationRect{x:c.x,y:c.y,width:c.width,height:c.height};rotate(text,&mut r);c.x=r.x;c.y=r.y;c.width=r.width;c.height=r.height;}
        Ok(characters)
    }
    pub(super) fn plan_workspace_overlays(&self,page:&PagePlan,intent:&WorkspaceReplacementIntent,chosen:&impl Fn(&str,usize)->bool,rows:&mut Vec<WorkspaceReplacementMatch>,patches:&mut Vec<WorkspaceOverlayReplacement>)->EngineResult<()>{
        let annotate=self.document(&page.source_id)?.policy.as_ref().is_none_or(|p|p.protection.permissions().annotate);
        let mut characters_total=0usize;
        for overlay in &page.overlays{
            let Overlay::Text(text)=overlay else{continue};
            characters_total=characters_total.checked_add(text.text.chars().count()).filter(|n|*n<=250_000).ok_or_else(invalid)?;
            let characters=self.workspace_overlay_characters(text,page)?;
            let mut offsets=Vec::with_capacity(characters.len()+1);let mut offset=0;for c in &characters{offsets.push(offset);offset+=c.text.len();}offsets.push(offset);
            let mut edits=Vec::new();let start=rows.len();
            for range in SearchMatches::new(characters.iter().map(|c|c.text.as_str()),&intent.query,intent.options){
                let rects=super::super::page_search::merge_rects(&characters[range.clone()],matches!(text.rotation,90|270));if !visible(&rects,page.crop.as_ref()){continue;}
                let index=rows.len();let selected=chosen(&page.id,index);if selected{edits.push((offsets[range.start],offsets[range.end]));}
                rows.push(WorkspaceReplacementMatch{match_index:index,overlay_id:Some(text.id.clone()),rects,selected,blocker:(!annotate).then(||"This PDF does not permit changes to added text annotations.".into())});
                if rows.len()>1000{return Err(invalid());}
            }
            if edits.is_empty()||!annotate{continue;}
            let length=edits.iter().try_fold(text.text.len(),|n,(a,b)|n.checked_sub(b-a)?.checked_add(intent.replacement.len())).ok_or_else(invalid)?;
            let mut replacement=String::new();let mut reason=None;
            if length>512*1024{reason=Some("The added text exceeds the replacement batch limit.");}
            else{
                replacement=text.text.clone();for(a,b)in edits.into_iter().rev(){replacement.replace_range(a..b,&intent.replacement);}
                if replacement==text.text{continue;}
                if !replacement.is_empty(){
                    let mut candidate=text.clone();candidate.text=replacement.clone();
                    let fits=self.workspace_overlay_characters(&candidate,page).is_ok_and(|characters|characters.iter().filter(|c|!c.text.chars().all(whitespace)).all(|c|[c.x,c.y,c.width,c.height].iter().all(|v|v.is_finite())&&c.x>=-0.01&&c.y>=-0.01&&c.x+c.width<=page.width+0.01&&c.y+c.height<=page.height+0.01));
                    if !fits{reason=Some("The replacement does not fit the page or its saved text area with the current font.");}
                }
            }
            if let Some(reason)=reason{for row in &mut rows[start..]{if row.selected{row.blocker=Some(reason.into());}}}
            else{patches.push(WorkspaceOverlayReplacement{page_id:page.id.clone(),overlay_id:text.id.clone(),expected_text:text.text.clone(),text:(!replacement.is_empty()).then_some(replacement)});}
        }Ok(())
    }
}
