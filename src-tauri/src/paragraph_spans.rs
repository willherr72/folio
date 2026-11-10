//! Track formatting provenance through the existing logical separator rules.
use super::*;
#[derive(Default)]
pub(super) struct Builder { pieces:Vec<Piece> }
struct Piece {text:String,style_id:usize,source:Option<(usize,usize)>}
#[derive(Clone,Debug,PartialEq,Eq)]
pub(super) struct SourceRange {
    pub object_index:usize,
    pub source_start:usize,
    pub source_end:usize,
    pub logical_start:usize,
}
pub(super) struct Assembly {pub runs:Vec<StyledParagraphRun>,pub sources:Vec<SourceRange>}
impl Builder {
    pub fn push(&mut self,text:&str,style_id:usize){
        if text.is_empty(){return;}
        self.pieces.push(Piece{text:text.into(),style_id,source:None});
    }
    pub fn push_source(&mut self,text:&str,style_id:usize,object_index:usize){
        if !text.is_empty(){self.pieces.push(Piece{text:text.into(),style_id,source:Some((object_index,0))});}
    }
    pub fn trim_end_spaces(&mut self){
        while let Some(last)=self.pieces.last_mut(){last.text.truncate(last.text.trim_end_matches(' ').len());if !last.text.is_empty(){break;}self.pieces.pop();}
    }
    pub fn finish(mut self)->Assembly{
        let mut skip=0;
        for first in &mut self.pieces {
            let removed=first.text.len()-first.text.trim_start().len();
            first.text.drain(..removed);
            if let Some((_,offset))=&mut first.source{*offset+=removed;}
            if !first.text.is_empty(){break;}skip+=1;
        }
        self.pieces.drain(..skip);
        while let Some(last)=self.pieces.last_mut(){last.text.truncate(last.text.trim_end().len());if !last.text.is_empty(){break;}self.pieces.pop();}
        let mut runs:Vec<StyledParagraphRun>=Vec::new();let mut sources=Vec::new();let mut logical_start=0;
        for piece in self.pieces {
            if let Some((object_index,source_start))=piece.source {
                sources.push(SourceRange{object_index,source_start,source_end:source_start+piece.text.len(),logical_start});
            }
            logical_start+=piece.text.len();
            if let Some(last)=runs.last_mut().filter(|r|r.style_id==piece.style_id){last.text.push_str(&piece.text);}
            else{runs.push(StyledParagraphRun{text:piece.text,style_id:piece.style_id});}
        }
        Assembly{runs,sources}
    }
}

#[cfg(test)]
mod tests {
 use super::*;
 #[test]
 fn trims_keep_exact_source_byte_ranges_for_repeated_multibyte_text(){
  let mut builder=Builder::default();builder.push_source("  é é  ",0,7);builder.trim_end_spaces();builder.push(" ",0);builder.push_source("é ",1,11);
  let assembled=builder.finish();
  assert_eq!(assembled.runs.iter().map(|r|(r.text.as_str(),r.style_id)).collect::<Vec<_>>(),vec![("é é ",0),("é",1)]);
  assert_eq!(assembled.sources.iter().map(|s|(s.object_index,s.source_start,s.source_end,s.logical_start)).collect::<Vec<_>>(),vec![(7,2,7,0),(11,0,2,6)]);
 }
 #[test]
 fn empty_trimmed_members_and_synthetic_spaces_never_acquire_source_ownership(){
  let mut builder=Builder::default();builder.push_source(" \u{2000}",0,1);builder.push_source(" x ",0,2);builder.trim_end_spaces();builder.push(" ",0);builder.push_source(" y  ",0,3);builder.push_source("   ",1,4);
  let assembled=builder.finish();assert_eq!(assembled.runs.len(),1);assert_eq!(assembled.runs[0].text,"x  y");
  assert_eq!(assembled.sources.iter().map(|s|(s.object_index,s.source_start,s.source_end,s.logical_start)).collect::<Vec<_>>(),vec![(2,1,2,0),(3,0,2,2)]);
 }
}
