//! Conservative fresh attributed-paragraph alignment from checked source glyphs.
use super::*;

#[derive(Clone,Copy)]
pub(super) struct Proposal {pub alignment:ParagraphAlignment,pub anchor:f32}

fn from_rows(rows:&[[f32;3]],tolerance:f32)->Option<Proposal>{
 if rows.len()<2||rows.len()>256{return None;}
 let spread=|values:Vec<f32>|values.iter().copied().fold(f32::NEG_INFINITY,f32::max)-values.iter().copied().fold(f32::INFINITY,f32::min);
 if spread(rows.iter().map(|r|r[2]-r[1]).collect())<=tolerance*4.{return None;}
 let candidates=[(ParagraphAlignment::Left,spread(rows.iter().map(|r|r[1]).collect())),
  (ParagraphAlignment::Center,spread(rows.iter().map(|r|(r[1]+r[2])*0.5).collect())),
  (ParagraphAlignment::Right,spread(rows.iter().map(|r|r[2]).collect()))];
 let matching=candidates.into_iter().filter(|(_,delta)|*delta<=tolerance).collect::<Vec<_>>();
 if matching.len()!=1{return None;}
 let alignment=matching[0].0;
 // Use a real source line as the anchor, not an average that shifts every line.
 // The widest line supplies the natural frame; other rows establish alignment.
 let widest=rows.iter().max_by(|a,b|(a[2]-a[1]).total_cmp(&(b[2]-b[1])))?;
 let anchor=match alignment{ParagraphAlignment::Center=>(widest[1]+widest[2])*0.5,ParagraphAlignment::Right=>widest[2],_=>rows.iter().map(|r|r[1]).fold(f32::INFINITY,f32::min)};
 Some(Proposal{alignment,anchor})
}

pub(super) fn infer(selected:&[&RunSnapshot],glyphs:&HashMap<usize,Vec<Glyph>>,unit:f32)->Option<Proposal>{
 if !unit.is_finite()||unit<=0.{return None;}
 let mut rows:Vec<[f32;3]>=vec![];let mut smallest_em=f32::INFINITY;
 for run in selected{
  if run.text.trim().is_empty(){continue;}
  let size=run.font_size*run.matrix[0];if !size.is_finite()||size<=0.{return None;}
  let [left,right]=alignment_extents(run,glyphs.get(&run.index)?,size)?;
  if !left.is_finite()||!right.is_finite()||right<=left{return None;}
  smallest_em=smallest_em.min(size);
  if let Some(row)=rows.last_mut().filter(|r|(r[0]-run.matrix[5]).abs()<=EPSILON){row[1]=row[1].min(left);row[2]=row[2].max(right);}
  else{rows.push([run.matrix[5],left,right]);}
 }
 // Allow small producer/ink differences, bounded in physical points. Distinct
 // widths and exactly one matching anchor remain mandatory; no single-line guess.
 let tolerance=(smallest_em*unit*0.15).clamp(0.1,2.)/unit;
 from_rows(&rows,tolerance)
}

pub(super) fn line_spacing(selected:&[&RunSnapshot],unit:f32)->Option<f32>{
 if !unit.is_finite()||unit<=0.{return None;}
 let mut rows:Vec<[f32;2]>=vec![];
 for run in selected.iter().filter(|run|!run.text.trim().is_empty()){
  let em=run.font_size*run.matrix[0];if !em.is_finite()||em<=0.{return None;}
  if let Some(row)=rows.last_mut().filter(|r|(r[0]-run.matrix[5]).abs()<=EPSILON){row[1]=row[1].max(em);}
  else{rows.push([run.matrix[5],em]);}
 }
 if rows.len()<2||rows.len()>256{return None;}
 let pairs=rows.windows(2).map(|p|(p[0][0]-p[1][0],p[0][1].max(p[1][1]))).collect::<Vec<_>>();
 let ratio=pairs.iter().map(|(gap,em)|gap/em).sum::<f32>()/pairs.len() as f32;
 let minimum_em=rows.iter().map(|r|r[1]).fold(f32::INFINITY,f32::min);
 if !ratio.is_finite()||!(0.6-EPSILON/minimum_em..=3.+EPSILON/minimum_em).contains(&ratio){return None;}
 // Layout spaces successive baselines using the larger em of the two rows.
 // A first-piece-only estimate enlarges mixed-size rows a second time.
 if pairs.iter().any(|(gap,em)|!gap.is_finite()||*gap<=0.||(*gap-ratio*em).abs()>(em*unit*0.02).clamp(0.02,0.25)/unit){return None;}
 Some(ratio.clamp(0.6,3.))
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn distinct_rows_need_one_consistent_anchor(){
  assert!(from_rows(&[[100.,10.,30.]],1.).is_none());
  assert!(from_rows(&[[100.,10.,30.],[80.,10.,30.]],1.).is_none());
  assert!(from_rows(&[[100.,10.,30.],[80.,15.,55.]],1.).is_none());
  let center=from_rows(&[[100.,30.,70.],[80.,10.,90.],[60.,20.,80.]],1.).unwrap();assert_eq!(center.alignment,ParagraphAlignment::Center);assert_eq!(center.anchor,50.);
  let right=from_rows(&[[100.,30.,90.],[80.,10.,90.]],1.).unwrap();assert_eq!(right.alignment,ParagraphAlignment::Right);assert_eq!(right.anchor,90.);
 }
}
