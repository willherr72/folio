//! Read-only provenance after the existing paragraph admission and replay proof.
use super::*;
use crate::types::{ParagraphDraftMapping,ParagraphDraftRequest};
use paragraph_spans::SourceRange;

/// Every non-LF logical byte must be consumed by an independently verified
/// placed fragment, in order. No substring search or metadata-only authority.
pub(super) fn verified_fragments(text:&str,layout:&StyledParagraphLayout,indices:&[usize],sources:&HashMap<usize,String>)->EngineResult<Vec<SourceRange>>{
    if text.len()>4096||indices.len()>4096{return Err(invalid("The paragraph mapping exceeds its limits."));}
    let fragments=layout.lines.iter().flat_map(|line|&line.runs).collect::<Vec<_>>();
    if fragments.len()!=indices.len(){return Err(invalid("The saved paragraph fragments changed."));}
    let mut cursor=0;let mut result=Vec::new();
    for (fragment,id) in fragments.into_iter().zip(indices){
        while text.as_bytes().get(cursor)==Some(&b'\n'){cursor+=1;}
        let end=cursor.checked_add(fragment.text.len()).ok_or_else(||invalid("Invalid paragraph range."))?;
        if fragment.text.is_empty()||sources.get(id)!=Some(&fragment.text)||text.get(cursor..end)!=Some(fragment.text.as_str()){
            return Err(invalid("The saved paragraph text has no exact source mapping."));
        }
        result.push(SourceRange{object_index:*id,source_start:0,source_end:fragment.text.len(),logical_start:cursor});cursor=end;
    }
    if !text.get(cursor..).is_some_and(|tail|tail.bytes().all(|b|b==b'\n')){return Err(invalid("The saved paragraph mapping is incomplete."));}
    Ok(result)
}
fn utf16(text:&str,byte:usize)->EngineResult<usize>{
    text.get(..byte).map(|s|s.encode_utf16().count()).ok_or_else(||invalid("The paragraph range splits a character."))
}
impl WorkerRuntime {
    pub(in crate::engine) fn map_paragraph_draft_inner(&self,id:&str,page:usize,request:&ParagraphDraftRequest)->EngineResult<ParagraphDraftMapping>{
        let (p,_)=self.paragraph_baseline_with_mapping(id,page,&request.indices,request.styled,true)?;
        if p.selection.text!=request.paragraph||p.source_text.get(&request.anchor)!=Some(&request.source){return Err(invalid("The paragraph or selected source text changed. Select it again."));}
        let ranges=if p.remembered{p.draft_ranges.as_deref().unwrap_or(&[])}else{&p.source_ranges};
        let mut matches=ranges.iter().filter(|r|r.object_index==request.anchor);
        let range=matches.next().ok_or_else(||invalid("The selected text has no retained paragraph range."))?;
        if matches.next().is_some(){return Err(invalid("The selected paragraph text is ambiguous."));}
        let length=range.source_end.checked_sub(range.source_start).filter(|n|*n>0).ok_or_else(||invalid("Invalid source range."))?;
        let end=range.logical_start.checked_add(length).ok_or_else(||invalid("Invalid paragraph range."))?;
        let retained=request.source.get(range.source_start..range.source_end).ok_or_else(||invalid("Invalid source range."))?;
        if request.paragraph.get(range.logical_start..end)!=Some(retained)
            ||!request.source.get(..range.source_start).is_some_and(|s|s.trim().is_empty())
            ||!request.source.get(range.source_end..).is_some_and(|s|s.trim().is_empty()){
            return Err(invalid("The paragraph text could not be mapped exactly."));
        }
        Ok(ParagraphDraftMapping{source_id:id.into(),page_index:page,object_indices:request.indices.clone(),anchor_index:request.anchor,
            paragraph_text:request.paragraph.clone(),source_text:request.source.clone(),source_start:utf16(&request.source,range.source_start)?,source_end:utf16(&request.source,range.source_end)?,paragraph_start:utf16(&request.paragraph,range.logical_start)?,paragraph_end:utf16(&request.paragraph,end)?})
    }
}
