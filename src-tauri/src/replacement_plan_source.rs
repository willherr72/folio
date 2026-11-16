//! Native reading order and object ownership for read-only replacement plans.
use super::*;
use crate::engine::{page_geometry,reading_characters,PageText,PdfTextCharacter};
use pdfium_render::prelude::*;
use sha2::{Digest,Sha256};
use std::collections::HashMap;

struct Binding {run:PlanningRun,expected:String,bounds:AnnotationRect,reason:Option<String>}
#[cfg(test)]
thread_local!{pub(super) static LAST_DIRECT:std::cell::Cell<bool>=const{std::cell::Cell::new(true)};}

fn collect<'a>(object:PdfPageObject<'a>,path:Vec<usize>,lookup:&HashMap<Vec<usize>,usize>,ordered:&[&[usize]],objects:&mut Vec<PdfPageTextObject<'a>>,indices:&mut Vec<usize>,visits:&mut usize)->EngineResult<()> {
    *visits+=1;if *visits>100_000||path.len()>13{return Err(invalid());}
    let at=ordered.partition_point(|candidate|*candidate<path.as_slice());
    if !ordered.get(at).is_some_and(|candidate|candidate.starts_with(&path)){return Ok(());}
    match object {
        PdfPageObject::Text(text)=>if let Some(index)=lookup.get(&path){objects.push(text);indices.push(*index);},
        PdfPageObject::XObjectForm(form)=>for i in form.as_range(){let mut child=path.clone();child.push(i as usize);collect(form.get(i)?,child,lookup,ordered,objects,indices,visits)?;},
        _=>(),
    }
    Ok(())
}

#[derive(Hash,PartialEq,Eq)]
struct CharacterKey {text:String,bounds:[u32;4]}
fn key(character:&PdfTextCharacter)->Option<CharacterKey>{
    let bounds=[character.x,character.y,character.width,character.height];
    bounds.iter().all(|v|v.is_finite()).then(||CharacterKey{text:character.text.clone(),bounds:bounds.map(|v|if v==0.{0}else{v.to_bits()})})
}
fn intersects(character:&PdfTextCharacter,bounds:&AnnotationRect)->bool {
    // This only distinguishes invocations already proved to share one actual
    // native object identity. A rectangle never establishes ownership itself.
    let right=character.x+character.width;let bottom=character.y+character.height;
    character.x<=bounds.x+bounds.width&&right>=bounds.x&&character.y<=bounds.y+bounds.height&&bottom>=bounds.y
}

impl WorkerRuntime {
    pub(in crate::engine) fn plan_text_replacement_page_raw(&mut self,source:&str,page_index:usize,query:&str,replacement:&str,options:PageSearchOptions,selected:Option<&[usize]>)->EngineResult<TextReplacementPagePlan>{
        self.copying_document(source)?;self.editing_document(source)?;
        if query.len()>1024||query.chars().all(whitespace)||replacement.len()>4096||replacement.contains('\0')||selected.is_some_and(|s|s.len()>1000){return Err(invalid());}
        let roots=self.list_transaction_text_runs(source,page_index,true)?;
        let nested=self.list_complete_form_text_runs(source,page_index)?;
        let mut bindings=Vec::with_capacity(roots.runs.len()+nested.runs.len());
        for run in roots.runs {
            let expected=run.text;let text=run.source_text.unwrap_or_else(||expected.clone());
            bindings.push(Binding{run:PlanningRun{object_path:vec![run.object_index],text,supported:run.supported},expected,bounds:run.bounds,reason:run.reason});
        }
        for run in nested.runs {
            bindings.push(Binding{run:PlanningRun{object_path:run.object_path,text:run.text.clone(),supported:run.supported},expected:run.text,bounds:run.bounds,reason:run.reason});
        }
        let runs=bindings.iter().map(|binding|binding.run.clone()).collect::<Vec<_>>();
        validate(&[],&runs,query,replacement)?;
        // Canonical reading may use a rotation-normalized private page copy.
        // Reconcile its glyphs with actual source-owned characters below.
        #[cfg(feature="shaped-text")]
        let ocr=self.read_ocr_page_text(source,page_index,Some(250_000))?;
        #[cfg(not(feature="shaped-text"))]
        let ocr:Option<PageText>=None;
        let ocr_override=ocr.is_some();
        let canonical=match ocr{Some(text)=>text,None=>self.read_document_page_text(self.document(source)?.pdf()?,page_index,Some(250_000))?};
        let document=self.document(source)?;
        let fingerprint=Sha256::digest(document.bytes()?.as_ref()).iter().map(|b|format!("{b:02x}")).collect();
        let page=document.pdf()?.pages().get(crate::engine::page_index_i32(page_index)?)?;
        let geometry=page_geometry(&page)?;let size=geometry.displayed_size();let text=page.text()?;
        let raw=reading_characters::read(&text,geometry,Some(250_000))?;
        let lookup=bindings.iter().enumerate().map(|(i,b)|(b.run.object_path.clone(),i)).collect::<HashMap<_,_>>();
        let mut ordered=bindings.iter().map(|b|b.run.object_path.as_slice()).collect::<Vec<_>>();ordered.sort_unstable();
        let page_objects=page.objects();let mut objects=Vec::new();let mut object_bindings=Vec::new();let mut visits=0;
        for(index,object)in page_objects.iter().enumerate(){collect(object,vec![index],&lookup,&ordered,&mut objects,&mut object_bindings,&mut visits)?;}
        let object_refs=objects.iter().collect::<Vec<_>>();
        let groups=text.character_groups_for_objects(&object_refs);
        let mut native_groups=vec![None;text.len()as usize];let mut group_bindings=Vec::with_capacity(groups.len());
        for(group,(object_indices,character_indices))in groups.into_iter().enumerate(){
            let mut candidates=object_indices.into_iter().map(|i|object_bindings[i]).collect::<Vec<_>>();candidates.sort_unstable();candidates.dedup();
            group_bindings.push(candidates);
            for index in character_indices{native_groups[index]=Some(group);}
        }
        let mut by_run=vec![Vec::new();bindings.len()];
        for(index,entry)in raw.iter().enumerate(){
            if entry.generated {continue;}
            let group=entry.indices.clone().filter_map(|i|native_groups[i]).collect::<Vec<_>>();
            if group.len()!=entry.indices.len()||group.is_empty()||group.iter().any(|g|*g!=group[0]){continue;}
            let candidates=&group_bindings[group[0]];
            let selected=if candidates.len()==1{Some(candidates[0])}else{
                let mut hits=candidates.iter().copied().filter(|i|intersects(&entry.character,&bindings[*i].bounds));
                match(hits.next(),hits.next()){(Some(i),None)=>Some(i),_=>None}
            };
            if let Some(run)=selected{by_run[run].push(index);}
        }
        let mut sources=vec![None;raw.len()];
        for(run,indices)in by_run.iter().enumerate(){
            // Prove the complete logical string, including scalar boundaries.
            // Reader-generated separators never become source byte authority.
            let original=&bindings[run].run.text;let mut at=0;let mut valid=true;
            for &index in indices {
                let value=&raw[index].character.text;let end=at+value.len();
                if original.get(at..end)!=Some(value.as_str()){valid=false;break;}
                sources[index]=Some(SourceRange{run_index:run,start:at,end});at=end;
            }
            if !valid||at!=original.len(){for &index in indices{sources[index]=None;}}
        }
        // If the canonical reader used this very text page, native character
        // ordinals are authoritative even for repeated boxes inside a ligature.
        // An OCR override or rotation-normalized copy cannot use this shortcut.
        let direct=!ocr_override&&(geometry.rotation==0||!crate::engine::simple_forward_text(&page,&text))
            &&canonical.characters.len()==raw.len()&&canonical.characters.iter().zip(&raw).all(|(a,b)|a==&b.character);
        #[cfg(test)]LAST_DIRECT.with(|value|value.set(direct));
        let owned=if direct {
            canonical.characters.iter().zip(&sources).map(|(character,source)|OwnedCharacter{text:character.text.clone(),source:source.clone()}).collect::<Vec<_>>()
        }else{
            // A copied reader needs exact, unique reconciliation with the
            // already source-owned glyph; geometry alone grants no authority.
            let mut raw_lookup:HashMap<CharacterKey,Option<usize>>=HashMap::new();
            for(index,entry)in raw.iter().enumerate(){if let Some(key)=key(&entry.character){raw_lookup.entry(key).and_modify(|value|*value=None).or_insert(Some(index));}}
            canonical.characters.iter().map(|character|{
                let source=key(character).and_then(|key|raw_lookup.get(&key).copied().flatten()).and_then(|index|sources[index].clone());
                OwnedCharacter{text:character.text.clone(),source}
            }).collect::<Vec<_>>()
        };
        let planned=plan_page(&owned,&runs,query,replacement,options,selected)?;
        let matches=planned.matches.into_iter().enumerate().map(|(index,found)|{
            let rects=crate::engine::page_search::merge_rects(&canonical.characters[found.characters.clone()],matches!(canonical.intrinsic_rotation,90|270));
            let source_reason=if found.blocker==Some(Blocker::Ineligible){found.source.as_ref().and_then(|range|bindings[range.run_index].reason.clone())}else{None};
            TextReplacementMatch{match_index:index,character_start:found.characters.start,character_end:found.characters.end,rects,
                source_range:found.source.map(|range|TextReplacementSourceRange{object_path:bindings[range.run_index].run.object_path.clone(),start:range.start,end:range.end}),selected:found.selected,blocker:found.blocker,source_reason}
        }).collect();
        let edits=planned.edits.into_iter().map(|edit|TextBatchEdit{page_index,object_path:edit.object_path,expected_text:bindings[edit.run_index].expected.clone(),replacement:edit.replacement,font_id:None}).collect();
        let mut result=TextReplacementPagePlan{source_id:source.into(),source_fingerprint:fingerprint,page_index,width:size.width,height:size.height,query:query.into(),replacement:replacement.into(),options,
            matches,edits,selected_count:planned.selected_count,blocked_count:planned.blocked_count,batch_blocker:planned.batch_blocker,can_apply:planned.can_apply};
        result.enforce_batch_budget();
        result.check_size()?;Ok(result)
    }
}
