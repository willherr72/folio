//! Read-only page-wide matching and grouping. Ownership is native worker data.
use super::{EngineError,EngineResult,PageSearchOptions,PdfEngine,WorkerRuntime,WorkerRequest,AnnotationRect,mpsc};
use crate::TextBatchEdit;
use super::search_matching::{SearchMatches,whitespace};
use serde::{Deserialize,Serialize};
use std::collections::{BTreeMap,HashSet};
use std::ops::Range;

#[derive(Debug,Clone)]
pub(super) struct PlanningRun {pub object_path:Vec<usize>,pub text:String,pub supported:bool}
#[derive(Debug,Clone,PartialEq,Eq)]
pub(super) struct SourceRange {pub run_index:usize,pub start:usize,pub end:usize}
#[derive(Debug,Clone)]
pub(super) struct OwnedCharacter {pub text:String,pub source:Option<SourceRange>}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub enum TextReplacementBlocker {Unmapped,CrossObject,Ineligible,InvalidMapping,OverlappingSource,NeedsRemoval,NeedsParagraphLayout,ReplacementTooLong,TooManyEdits,BatchBytes}
use TextReplacementBlocker as Blocker;
#[derive(Debug,PartialEq,Eq)]
pub(super) struct PlanMatch {pub characters:Range<usize>,pub source:Option<SourceRange>,pub selected:bool,pub blocker:Option<Blocker>}
#[derive(Debug,PartialEq,Eq)]
pub(super) struct PlannedRunEdit {pub run_index:usize,pub object_path:Vec<usize>,pub source_text:String,pub replacement:String}
#[derive(Debug,PartialEq,Eq)]
pub(super) struct PagePlan {pub matches:Vec<PlanMatch>,pub edits:Vec<PlannedRunEdit>,pub selected_count:usize,pub blocked_count:usize,pub batch_blocker:Option<Blocker>,pub can_apply:bool}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct TextReplacementSourceRange {pub object_path:Vec<usize>,pub start:usize,pub end:usize}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct TextReplacementMatch {
    pub match_index:usize,pub character_start:usize,pub character_end:usize,pub rects:Vec<AnnotationRect>,
    pub source_range:Option<TextReplacementSourceRange>,pub selected:bool,pub blocker:Option<TextReplacementBlocker>,pub source_reason:Option<String>,
}
#[derive(Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct TextReplacementPagePlan {
    pub source_id:String,pub source_fingerprint:String,pub page_index:usize,pub width:f32,pub height:f32,
    pub query:String,pub replacement:String,pub options:PageSearchOptions,pub matches:Vec<TextReplacementMatch>,
    pub edits:Vec<TextBatchEdit>,pub selected_count:usize,pub blocked_count:usize,
    pub batch_blocker:Option<TextReplacementBlocker>,pub can_apply:bool,
}
pub(super) struct Request {
    pub source:String,pub page:usize,pub query:String,pub replacement:String,pub options:PageSearchOptions,
    pub selected:Option<Vec<usize>>,pub reply:mpsc::Sender<EngineResult<TextReplacementPagePlan>>,
}
impl PdfEngine {
    /// Read-only candidate planning, bound to this immutable source. A nonempty
    /// executable edit list still requires the final font/layout/preservation
    /// checks in replace_text_batch(); this method publishes no candidate.
    pub fn plan_text_replacement_page(&self,source_id:&str,page_index:usize,query:&str,replacement:&str,options:PageSearchOptions,selected:Option<&[usize]>)->EngineResult<TextReplacementPagePlan>{
        if query.len()>1024||query.chars().all(whitespace)||replacement.len()>4096||replacement.contains('\0')||selected.is_some_and(|s|s.len()>1000){return Err(invalid());}
        let(reply,receive)=mpsc::channel();
        self.send(WorkerRequest::ReplacementPlan(Request{source:source_id.into(),page:page_index,query:query.into(),replacement:replacement.into(),options,selected:selected.map(<[usize]>::to_vec),reply}))?;
        let plan=self.in_points(source_id,page_index,receive.recv().map_err(|_|EngineError::WorkerStopped)?)?;
        plan.check_size()?;Ok(plan)
    }
}
impl super::physical_units::Scale for TextReplacementPagePlan {
    fn scale(&mut self,factor:f32)->EngineResult<()> {
        self.width.scale(factor)?;self.height.scale(factor)?;
        for found in &mut self.matches{found.rects.scale(factor)?;}Ok(())
    }
}
impl TextReplacementPagePlan {
    fn enforce_batch_budget(&mut self){
        if !super::text_batch::batch_strings_fit(&self.edits){self.batch_blocker=Some(Blocker::BatchBytes);self.edits.clear();self.can_apply=false;}
    }
    pub(super) fn check_size(&self)->EngineResult<()> {
        let mut writer=crate::pdf_output::BoundedWriter::new(2*1024*1024);
        serde_json::to_writer(&mut writer,self).map_err(|_|invalid())?;
        writer.finish().map(|_|()).map_err(|_|invalid())
    }
}
impl WorkerRuntime {
    pub(super) fn handle_replacement_plan(&mut self,request:Request){
        let result=self.plan_text_replacement_page_raw(&request.source,request.page,&request.query,&request.replacement,request.options,request.selected.as_deref());
        let _=request.reply.send(result);
    }
}
#[path="replacement_plan_source.rs"]
mod source;
#[cfg(test)]
#[path="replacement_plan_native_tests.rs"]
mod native_tests;

fn invalid()->EngineError {EngineError::InvalidRequest("Replacement planning input is invalid or exceeds its bounds; no text was truncated.".into())}

fn validate(characters:&[OwnedCharacter],runs:&[PlanningRun],query:&str,replacement:&str)->EngineResult<()> {
    if query.len()>1024||query.chars().all(whitespace)||replacement.len()>4096||replacement.contains('\0')||characters.len()>250_000||runs.len()>100_000{return Err(invalid());}
    let mut bytes=0usize;let mut paths=Vec::with_capacity(runs.len());
    for run in runs {
        if run.text.len()>16*1024||run.object_path.is_empty()||run.object_path.len()>13{return Err(invalid());}
        bytes=bytes.checked_add(run.text.len()).filter(|n|*n<=16*1024*1024).ok_or_else(invalid)?;
        paths.push(run.object_path.as_slice());
    }
    paths.sort_unstable();
    if paths.windows(2).any(|pair|pair[1].starts_with(pair[0])){return Err(invalid());}
    let mut bytes=0usize;
    for character in characters {bytes=bytes.checked_add(character.text.len()).filter(|n|*n<=16*1024*1024).ok_or_else(invalid)?;}
    Ok(())
}

/// A valid source byte range is never inferred from bounds or text similarity.
fn valid_source(character:&OwnedCharacter,runs:&[PlanningRun])->bool {
    let Some(source)=&character.source else{return false};
    source.start<source.end&&character.text.chars().count()==1&&runs.get(source.run_index)
        .and_then(|run|run.text.get(source.start..source.end))==Some(character.text.as_str())
}

fn ownership_overlap(characters:&[OwnedCharacter],runs:&[PlanningRun])->Vec<bool> {
    let mut grouped=vec![Vec::new();runs.len()];let mut overlaps=vec![false;characters.len()];
    for(index,character)in characters.iter().enumerate(){
        if valid_source(character,runs){let source=character.source.as_ref().unwrap();grouped[source.run_index].push((source.start,source.end,index));}
    }
    for entries in &mut grouped {
        entries.sort_unstable();let mut latest:Option<(usize,usize)>=None;
        for &(start,end,index)in entries.iter(){
            if let Some((max_end,previous))=latest {
                if start<max_end {overlaps[index]=true;overlaps[previous]=true;}
                if end>max_end {latest=Some((end,index));}
            }else{latest=Some((end,index));}
        }
    }
    overlaps
}

fn map_match(range:Range<usize>,characters:&[OwnedCharacter],runs:&[PlanningRun],overlaps:&[bool],selected:bool)->PlanMatch {
    let entries=&characters[range.clone()];let mut source=None;let mut blocker=None;
    if entries.iter().any(|entry|entry.source.is_some()&&!valid_source(entry,runs)){blocker=Some(Blocker::InvalidMapping);}
    else {
        let owners=entries.iter().filter_map(|entry|entry.source.as_ref().map(|s|s.run_index)).collect::<HashSet<_>>();
        if owners.len()>1 {blocker=Some(Blocker::CrossObject);}
        else if owners.is_empty()||entries.iter().any(|entry|entry.source.is_none()){blocker=Some(Blocker::Unmapped);}
        else if overlaps[range.clone()].iter().any(|v|*v){blocker=Some(Blocker::OverlappingSource);}
        else {
            let first=entries[0].source.as_ref().unwrap();let last=entries.last().unwrap().source.as_ref().unwrap();
            if entries.windows(2).any(|pair|pair[0].source.as_ref().unwrap().end!=pair[1].source.as_ref().unwrap().start){blocker=Some(Blocker::InvalidMapping);}
            else {
                source=Some(SourceRange{run_index:first.run_index,start:first.start,end:last.end});
                if !runs[first.run_index].supported {blocker=Some(Blocker::Ineligible);}
            }
        }
    }
    PlanMatch{characters:range,source,selected,blocker}
}

pub(super) fn plan_page(characters:&[OwnedCharacter],runs:&[PlanningRun],query:&str,replacement:&str,options:PageSearchOptions,selected:Option<&[usize]>)->EngineResult<PagePlan>{
    validate(characters,runs,query,replacement)?;
    let selected_set=selected.map(|indices|indices.iter().copied().collect::<HashSet<_>>());
    if selected.is_some_and(|indices|indices.len()>1000||selected_set.as_ref().unwrap().len()!=indices.len()){return Err(invalid());}
    let overlaps=ownership_overlap(characters,runs);let mut matches=Vec::new();
    for range in SearchMatches::new(characters.iter().map(|c|c.text.as_str()),query,options){
        if matches.len()==1000{return Err(invalid());}
        let selected=selected_set.as_ref().is_none_or(|set|set.contains(&matches.len()));
        matches.push(map_match(range,characters,runs,&overlaps,selected));
    }
    if selected_set.as_ref().is_some_and(|set|set.iter().any(|index|*index>=matches.len())){return Err(invalid());}
    let mut grouped:BTreeMap<usize,Vec<usize>>=BTreeMap::new();
    for(index,found)in matches.iter().enumerate(){
        if found.selected&&found.blocker.is_none(){grouped.entry(found.source.as_ref().unwrap().run_index).or_default().push(index);}
    }
    let mut edits=Vec::new();let mut batch_blocker=None;let mut bytes=0usize;
    for(run_index,mut indices)in grouped {
        indices.sort_unstable_by_key(|i|matches[*i].source.as_ref().unwrap().start);
        let run=&runs[run_index];let mut length=run.text.len();
        for index in &indices{let source=matches[*index].source.as_ref().unwrap();length=length.checked_sub(source.end-source.start).and_then(|n|n.checked_add(replacement.len())).ok_or_else(invalid)?;}
        let blocker=if length>4096 {Some(Blocker::ReplacementTooLong)}else if length==0 {Some(Blocker::NeedsRemoval)}else{None};
        if let Some(blocker)=blocker {for index in indices{matches[index].blocker=Some(blocker);}continue;}
        let mut final_text=String::with_capacity(length);let mut at=0;
        for index in &indices{let source=matches[*index].source.as_ref().unwrap();final_text.push_str(&run.text[at..source.start]);final_text.push_str(replacement);at=source.end;}
        final_text.push_str(&run.text[at..]);
        if final_text==run.text {continue;}
        if final_text.chars().any(|c|matches!(c,'\r'|'\n'|'\u{000b}'|'\u{000c}'|'\u{0085}'|'\u{2028}'|'\u{2029}')){for index in indices{matches[index].blocker=Some(Blocker::NeedsParagraphLayout);}continue;}
        if edits.len()==64 {batch_blocker.get_or_insert(Blocker::TooManyEdits);continue;}
        bytes=bytes.checked_add(run.text.len()).and_then(|n|n.checked_add(final_text.len())).ok_or_else(invalid)?;
        if bytes>512*1024 {batch_blocker.get_or_insert(Blocker::BatchBytes);continue;}
        edits.push(PlannedRunEdit{run_index,object_path:run.object_path.clone(),source_text:run.text.clone(),replacement:final_text});
    }
    let selected_count=matches.iter().filter(|found|found.selected).count();
    let blocked_count=matches.iter().filter(|found|found.selected&&found.blocker.is_some()).count();
    if blocked_count>0||batch_blocker.is_some(){edits.clear();}
    let can_apply=!edits.is_empty();
    Ok(PagePlan{matches,edits,selected_count,blocked_count,batch_blocker,can_apply})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn data(parts:&[&str])->(Vec<OwnedCharacter>,Vec<PlanningRun>){
        let runs=parts.iter().enumerate().map(|(i,text)|PlanningRun{object_path:vec![i],text:(*text).into(),supported:true}).collect::<Vec<_>>();
        let mut characters=vec![];
        for (i,text)in parts.iter().enumerate(){for(start,c)in text.char_indices(){characters.push(OwnedCharacter{text:c.to_string(),source:Some(SourceRange{run_index:i,start,end:start+c.len_utf8()})});}}
        (characters,runs)
    }
    fn plan(parts:&[&str],query:&str,replacement:&str)->PagePlan{
        let(c,r)=data(parts);plan_page(&c,&r,query,replacement,PageSearchOptions::default(),None).unwrap()
    }
    #[test]
    fn page_replacement_planning_groups_matches_once_and_preserves_unselected_bytes(){
        let(c,r)=data(&["before cat CAT cat after"]);
        let all=plan_page(&c,&r,"cat","$&\\1",PageSearchOptions::default(),None).unwrap();
        assert_eq!(all.matches.len(),3);assert_eq!(all.edits.len(),1);assert!(all.can_apply);
        assert_eq!(all.edits[0].source_text,r[0].text);assert_eq!(all.edits[0].replacement,"before $&\\1 $&\\1 $&\\1 after");
        let subset=plan_page(&c,&r,"cat","dog",PageSearchOptions::default(),Some(&[1])).unwrap();
        assert_eq!(subset.edits[0].replacement,"before cat dog cat after");assert_eq!(subset.selected_count,1);
        assert_eq!(c[7].text,"c");assert_eq!(r[0].text,"before cat CAT cat after");
    }
    #[test]
    fn page_replacement_planning_uses_page_neighbors_for_whole_words_and_reports_cross_objects(){
        let(c,r)=data(&["cat","s cat"]);let flags=PageSearchOptions{match_case:false,whole_words:true};
        let found=plan_page(&c,&r,"cat","dog",flags,None).unwrap();
        assert_eq!(found.matches.len(),1);assert_eq!(found.edits[0].run_index,1);assert_eq!(found.edits[0].replacement,"s dog");
        let cross=plan(&["red ","fox"],"red fox","animal");
        assert_eq!(cross.matches.len(),1);assert_eq!(cross.matches[0].blocker,Some(Blocker::CrossObject));assert!(!cross.can_apply);assert!(cross.edits.is_empty());
    }
    #[test]
    fn page_replacement_planning_retains_utf8_and_complete_casefold_expansions(){
        let found=plan(&["😀 Straße Σσς"],"STRASSE","Road");
        assert_eq!(found.matches[0].source,Some(SourceRange{run_index:0,start:5,end:12}));assert_eq!(found.edits[0].replacement,"😀 Road Σσς");
        assert!(plan(&["ß"],"s","x").matches.is_empty());
        assert_eq!(plan(&["ß"],"ss","x").edits[0].replacement,"x");
        assert_eq!(plan(&["😀 cat é"],"😀","X").edits[0].replacement,"X cat é");
    }
    #[test]
    fn page_replacement_planning_collapsed_whitespace_keeps_exact_source_range(){
        let found=plan(&["left one \t  two right"],"one two","pair");
        assert_eq!(found.matches.len(),1);assert_eq!(found.edits[0].replacement,"left pair right");
        let(c,r)=data(&["CAT cat"]);let flags=PageSearchOptions{match_case:true,whole_words:false};
        assert_eq!(plan_page(&c,&r,"CAT","dog",flags,None).unwrap().edits[0].replacement,"dog cat");
    }
    #[test]
    fn page_replacement_planning_blocks_selected_failures_atomically_but_keeps_unselected_rows(){
        let(c,mut r)=data(&["cat ","cat"]);r[1].supported=false;
        let all=plan_page(&c,&r,"cat","dog",PageSearchOptions::default(),None).unwrap();
        assert_eq!(all.blocked_count,1);assert!(all.edits.is_empty());assert!(!all.can_apply);
        let subset=plan_page(&c,&r,"cat","dog",PageSearchOptions::default(),Some(&[0])).unwrap();
        assert_eq!(subset.blocked_count,0);assert!(subset.can_apply);assert_eq!(subset.edits.len(),1);
        assert_eq!(subset.matches[1].blocker,Some(Blocker::Ineligible));assert!(!subset.matches[1].selected);
    }
    #[test]
    fn page_replacement_planning_reports_removal_layout_noop_and_empty_selection(){
        let removal=plan(&["cat"],"cat","");assert_eq!(removal.matches[0].blocker,Some(Blocker::NeedsRemoval));assert!(removal.edits.is_empty());
        assert_eq!(plan(&["cat after"],"cat","").edits[0].replacement," after");
        let layout=plan(&["cat"],"cat","two\nlines");assert_eq!(layout.matches[0].blocker,Some(Blocker::NeedsParagraphLayout));assert!(layout.edits.is_empty());
        let noop=plan(&["cat"],"cat","cat");assert_eq!(noop.matches.len(),1);assert!(noop.edits.is_empty());assert!(!noop.can_apply);
        let(c,r)=data(&["cat"]);let none=plan_page(&c,&r,"cat","dog",PageSearchOptions::default(),Some(&[])).unwrap();assert_eq!(none.selected_count,0);assert!(!none.can_apply);
    }
    #[test]
    fn page_replacement_planning_never_guesses_missing_or_inconsistent_source_ranges(){
        let(c,r)=data(&["redfox"]);let mut separated=c.clone();separated.insert(3,OwnedCharacter{text:"\n".into(),source:None});
        let found=plan_page(&separated,&r,"red fox","x",PageSearchOptions::default(),None).unwrap();assert_eq!(found.matches[0].blocker,Some(Blocker::Unmapped));assert!(found.edits.is_empty());
        for source in [SourceRange{run_index:4,start:0,end:1},SourceRange{run_index:0,start:1,end:2},SourceRange{run_index:0,start:0,end:100}]{
            let mut bad=c.clone();bad[0].source=Some(source);
            let result=plan_page(&bad,&r,"red","x",PageSearchOptions::default(),None).unwrap();assert_eq!(result.matches[0].blocker,Some(Blocker::InvalidMapping));assert!(result.edits.is_empty());
        }
        let(c,r)=data(&["é"]);let mut bad=c;bad[0].source.as_mut().unwrap().start=1;
        assert_eq!(plan_page(&bad,&r,"é","x",PageSearchOptions::default(),None).unwrap().matches[0].blocker,Some(Blocker::InvalidMapping));
        let(_,r)=data(&["ffi"]);let multiple=vec![OwnedCharacter{text:"ffi".into(),source:Some(SourceRange{run_index:0,start:0,end:3})}];
        assert_eq!(plan_page(&multiple,&r,"ff","x",PageSearchOptions::default(),None).unwrap().matches[0].blocker,Some(Blocker::InvalidMapping));
    }
    #[test]
    fn page_replacement_planning_rejects_repeated_source_mapping_even_for_one_selected_match(){
        let(c,r)=data(&["cat"]);let repeated=[c.clone(),c].concat();
        let result=plan_page(&repeated,&r,"cat","dog",PageSearchOptions::default(),Some(&[0])).unwrap();
        assert_eq!(result.matches.len(),2);assert_eq!(result.matches[0].blocker,Some(Blocker::OverlappingSource));assert!(result.edits.is_empty());
    }
    #[test]
    fn page_replacement_planning_rejects_invalid_requests_without_truncation(){
        let(c,r)=data(&["cat"]);
        for(q,replacement,selected)in [(" ".to_string(),"x".to_string(),None),("x".repeat(1025),"x".to_string(),None),("cat".to_string(),"x".repeat(4097),None),("cat".to_string(),"\0".to_string(),None),("cat".to_string(),"x".to_string(),Some(vec![0,0])),("cat".to_string(),"x".to_string(),Some(vec![1]))]{
            assert!(plan_page(&c,&r,&q,&replacement,PageSearchOptions::default(),selected.as_deref()).is_err());
        }
        let many="cat ".repeat(1001);let(c,r)=data(&[&many]);assert!(plan_page(&c,&r,"cat","dog",PageSearchOptions::default(),None).is_err());
    }
    #[test]
    fn page_replacement_planning_exposes_edit_and_text_budgets_instead_of_partial_edits(){
        let parts=vec!["cat";65];let many=plan(&parts,"cat","dog");assert_eq!(many.matches.len(),65);assert_eq!(many.batch_blocker,Some(Blocker::TooManyEdits));assert!(many.edits.is_empty());
        let long=format!("cat{}","x".repeat(4094));let too_long=plan(&[&long],"cat","dog");assert_eq!(too_long.matches[0].blocker,Some(Blocker::ReplacementTooLong));assert!(too_long.edits.is_empty());
        let text="😀".repeat(4096);let query="😀".repeat(256);let parts=vec![text.as_str();33];let bytes=plan(&parts,&query,"x");
        assert_eq!(bytes.matches.len(),528);assert_eq!(bytes.batch_blocker,Some(Blocker::BatchBytes));assert!(bytes.edits.is_empty());
    }
    #[test]
    fn review_replacement_planning_budget_counts_native_identity_after_conversion(){
        let old=format!("Σ{}","α".repeat(2047));let parts=vec![old.as_str();64];let(c,r)=data(&parts);
        let model=plan_page(&c,&r,"Σ","xx",PageSearchOptions::default(),None).unwrap();assert_eq!(model.edits.len(),64);assert!(model.batch_blocker.is_none());
        for extra in [false,true]{
            let mut plan=TextReplacementPagePlan{source_id:"source".into(),source_fingerprint:"0".repeat(64),page_index:0,width:400.,height:500.,query:"Σ".into(),replacement:"xx".into(),options:Default::default(),
                matches:model.matches.iter().enumerate().map(|(i,m)|TextReplacementMatch{match_index:i,character_start:m.characters.start,character_end:m.characters.end,rects:vec![],source_range:None,selected:true,blocker:None,source_reason:None}).collect(),
                edits:model.edits.iter().map(|edit|TextBatchEdit{page_index:0,object_path:edit.object_path.clone(),expected_text:format!("{}{}",edit.source_text,if extra{" "}else{""}),replacement:edit.replacement.clone(),font_id:None}).collect(),selected_count:64,blocked_count:0,batch_blocker:None,can_apply:true};
            plan.enforce_batch_budget();assert_eq!(plan.matches.len(),64);
            if extra{assert_eq!(plan.batch_blocker,Some(Blocker::BatchBytes));assert!(plan.edits.is_empty());assert!(!plan.can_apply);}
            else{assert!(plan.batch_blocker.is_none());assert_eq!(plan.edits.len(),64);assert!(plan.can_apply);}
        }
    }
    #[test]
    fn review_replacement_planning_unicode_breaks_block_selected_batches_and_subsets(){
        let(c,r)=data(&["cat ","cat"]);
        for separator in ['\u{2028}','\u{2029}','\u{0085}','\u{000b}','\u{000c}']{
            let replacement=format!("one{separator}two");
            for selected in [None,Some(&[1][..])]{
                let result=plan_page(&c,&r,"cat",&replacement,Default::default(),selected).unwrap();assert_eq!(result.matches.len(),2);assert_eq!(result.selected_count,if selected.is_some(){1}else{2});assert_eq!(result.blocked_count,result.selected_count);assert!(result.edits.is_empty());assert!(!result.can_apply);
                assert!(result.matches.iter().filter(|m|m.selected).all(|m|m.blocker==Some(Blocker::NeedsParagraphLayout)));
            }
        }
    }
}
