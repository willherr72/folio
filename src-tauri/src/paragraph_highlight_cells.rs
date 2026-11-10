//! Whole-source character advances are a separate authority from enclosing ink.
use super::*;
use paragraph_highlights::{Coverage,Rectangle};

#[derive(Clone,Copy,Debug,PartialEq,serde::Serialize,serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CellPolicy {pub offsets:[f32;4]}
impl CellPolicy {
 pub(super) fn valid(self)->bool {
  let [left,below,right,above]=self.offsets;
  [left,right].iter().all(|v|v.is_finite()&&(-0.005..=0.005).contains(v))
   &&[below,above].iter().all(|v|v.is_finite()&&(-0.005..=2.).contains(v))
 }
 pub(super) fn paint(self,origin:f32,advance:f32,ink:[f32;4],em:f32)->EngineResult<[f32;4]>{
  if !self.valid()||!em.is_finite()||em<=0.||!advance.is_finite()||advance<=0.
   ||!origin.is_finite()||ink.iter().any(|v|!v.is_finite())||ink[2]<=ink[0]||ink[3]<=ink[1]{return Err(invalid("Invalid character-cell highlight metrics."));}
  let p=self.offsets;let r=[origin+p[0]*em,ink[1]-p[1]*em,origin+advance+p[2]*em,ink[3]+p[3]*em];
  if r.iter().any(|v|!v.is_finite()||v.abs()>1_000_000.)||r[2]<=r[0]||r[3]<=r[1]{return Err(invalid("Degenerate character-cell highlight."));}Ok(r)
 }
}

pub(super) fn infer(prepared:&Prepared,mark:&Rectangle,glyphs:&HashMap<usize,Vec<Glyph>>)->EngineResult<Option<(Coverage,CellPolicy)>>{
 if !prepared.reproducible_text_state||glyphs.values().map(Vec::len).sum::<usize>()>16_384{return Ok(None);}
 let r=mark.bounds;
 if r.iter().any(|v|!v.is_finite())||r[2]<=r[0]||r[3]<=r[1]{return Ok(None);}
 let overlaps=|g:&Glyph|r[2].min(g.position[4])-r[0].max(g.position[2])>EPSILON&&r[3].min(g.position[5])-r[1].max(g.position[3])>EPSILON;
 let visible=|g:&Glyph|!g.text.trim().is_empty()&&g.position[4]>g.position[2]&&g.position[5]>g.position[3];
 let owners=glyphs.iter().filter(|(_,gs)|gs.iter().any(|g|visible(g)&&overlaps(g))).collect::<Vec<_>>();
 let [(id,characters)]=owners.as_slice() else{return Ok(None);};let id=**id;
 if id<=mark.object_index||!prepared.selection.object_indices.contains(&id){return Ok(None);}
 let Some(source)=prepared.source_text.get(&id) else{return Ok(None);};
 if source.is_empty()||source.len()>4096||source.trim()!=source||characters.iter().map(|g|g.text.as_str()).collect::<String>()!=*source{return Ok(None);}
 let Some(run)=prepared.before.iter().find(|run|run.index==id) else{return Ok(None);};
 let em=run.font_size*run.matrix[0];
 if !physical_units::paragraph_size(em,prepared.unit){return Ok(None);}
 let mut indices=Vec::new();let mut frame=[f32::INFINITY,f32::INFINITY,f32::NEG_INFINITY,f32::NEG_INFINITY];
 let mut last=None;let mut baseline:Option<(f32,f32)>=None;
 for (index,glyph) in characters.iter().enumerate(){
  let p=glyph.position;let mut chars=glyph.text.chars();let Some(ch)=chars.next() else{return Ok(None);};
  if chars.next().is_some()||p.iter().any(|v|!v.is_finite())||p[4]<p[2]||p[5]<p[3]||last.is_some_and(|x|p[0]<=x){return Ok(None);}
  last=Some(p[0]);let a=advance(run,&ch.to_string())?*em;
  if !a.is_finite()||a<=0.{return Ok(None);}
  let y=baseline.map_or((p[1],p[1]),|(low,high)|(low.min(p[1]),high.max(p[1])));
  if y.1-y.0>EPSILON||p[1]<r[1]||p[1]>r[3]{return Ok(None);}baseline=Some(y);
  frame[0]=frame[0].min(p[0]);frame[2]=frame[2].max(p[0]+a);
  if visible(glyph){
   if !overlaps(glyph){return Ok(None);}indices.push(index);frame[1]=frame[1].min(p[3]);frame[3]=frame[3].max(p[5]);
  }
 }
 if indices.is_empty()||indices.len()>4096{return Ok(None);}
 let policy=CellPolicy{offsets:[(r[0]-frame[0])/em,(frame[1]-r[1])/em,(r[2]-frame[2])/em,(r[3]-frame[3])/em]};
 if !policy.valid(){return Ok(None);}
 for glyph in characters.iter().filter(|g|visible(g)){
  let p=glyph.position;
  if policy.paint(p[0],advance(run,&glyph.text)?*em,[p[2],p[3],p[4],p[5]],em).is_err(){return Ok(None);}
 }
 Ok(Some((Coverage{object_index:id,glyph_indices:indices,whole_visible_run:true},policy)))
}
