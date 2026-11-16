//! Identical pinned Unicode policy to src/editor/search-matching.ts.
use super::PageSearchOptions;
use serde::Deserialize;
use std::{ops::Range,sync::OnceLock};

#[derive(Deserialize)]
#[serde(rename_all="camelCase")]
struct Tables {folds:Vec<(u32,String)>,word_ranges:Vec<[u32;2]>}
fn tables()->&'static Tables {
    static TABLES:OnceLock<Tables>=OnceLock::new();
    TABLES.get_or_init(||serde_json::from_str(include_str!("../../src/shared/search-unicode.json")).expect("Valid generated Unicode search tables"))
}
pub(super) fn whitespace(c:char)->bool {matches!(c,'\u{0009}'..='\u{000d}'|'\u{0020}'|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}')}
fn word_code(c:char)->bool {
    let cp=c as u32;let ranges=&tables().word_ranges;
    ranges.binary_search_by(|[start,end]|if cp<*start{std::cmp::Ordering::Greater}else if cp>*end{std::cmp::Ordering::Less}else{std::cmp::Ordering::Equal}).is_ok()
}
fn previous(value:&str,offset:usize)->Option<usize> {value[..offset].char_indices().next_back().map(|(index,_)|index)}
fn word_at(value:&str,offset:Option<usize>)->bool {
    let Some(offset)=offset else{return false};
    let Some(c)=value[offset..].chars().next()else{return false};
    if word_code(c){return true;}
    matches!(c,'\''|'\u{2019}')&&value[..offset].chars().next_back().is_some_and(word_code)&&value[offset+c.len_utf8()..].chars().next().is_some_and(word_code)
}
struct Normalized {value:String,glyphs:Vec<usize>,edges:Vec<bool>}
fn normalize<'a>(parts:impl IntoIterator<Item=&'a str>,match_case:bool)->Normalized {
    let mut result=Normalized {value:String::new(),glyphs:Vec::new(),edges:vec![true]};
    for(index,part)in parts.into_iter().enumerate(){
        for c in part.chars(){
            let mut encoded=[0;4];let original=c.encode_utf8(&mut encoded);
            let piece=if whitespace(c){
                if result.value.is_empty()||result.value.ends_with(' '){continue;}" "
            }else if match_case{original}else{
                tables().folds.binary_search_by_key(&(c as u32),|(cp,_)|*cp).ok().map(|at|tables().folds[at].1.as_str()).unwrap_or(original)
            };
            let start=result.value.len();result.value.push_str(piece);
            result.glyphs.resize(result.value.len(),index);result.edges.resize(result.value.len()+1,false);
            result.edges[start]=true;result.edges[result.value.len()]=true;
        }
    }
    if result.value.ends_with(' '){result.value.pop();result.glyphs.pop();result.edges.pop();}
    result
}

pub(super) struct SearchMatches {source:Normalized,needle:String,from:usize,whole_words:bool}
impl SearchMatches {
    pub(super) fn new<'a>(parts:impl IntoIterator<Item=&'a str>,query:&str,options:PageSearchOptions)->Self {
        Self {source:normalize(parts,options.match_case),needle:normalize([query],options.match_case).value,from:0,whole_words:options.whole_words}
    }
    pub(super) fn has_text(&self)->bool {!self.source.value.is_empty()}
}
impl Iterator for SearchMatches {
    type Item=Range<usize>;
    fn next(&mut self)->Option<Self::Item>{
        if self.needle.is_empty(){return None;}
        let value=&self.source.value;
        while self.from<value.len(){
            let offset=self.from+value[self.from..].find(&self.needle)?;let end=offset+self.needle.len();
            let accepted=self.source.edges[offset]&&self.source.edges[end]&&(!self.whole_words||
                (word_at(value,Some(offset))&&word_at(value,previous(value,end))&&!word_at(value,previous(value,offset))&&!word_at(value,Some(end))));
            if accepted{self.from=end;return Some(self.source.glyphs[offset]..self.source.glyphs[end-1]+1);}
            self.from=offset+value[offset..].chars().next()?.len_utf8();
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Fixture {name:String,text:Option<String>,parts:Option<Vec<String>>,query:String,#[serde(default)]options:PageSearchOptions,expected:Vec<[usize;2]>}
    #[test]
    fn shared_authored_cases_preserve_exact_original_glyph_ranges(){
        let cases:Vec<Fixture>=serde_json::from_str(include_str!("../../tests/fixtures/search/matching.json")).unwrap();
        for case in cases {
            let parts=case.parts.unwrap_or_else(||case.text.unwrap().chars().map(|c|c.to_string()).collect());
            let found:Vec<_>=SearchMatches::new(parts.iter().map(String::as_str),&case.query,case.options).map(|r|[r.start,r.end]).collect();
            assert_eq!(found,case.expected,"{}",case.name);
        }
    }
    #[test]
    fn every_retained_fold_matches_its_complete_source_scalar_without_partial_expansion(){
        for(cp,fold)in &tables().folds {
            let scalar=char::from_u32(*cp).unwrap().to_string();
            assert_eq!(SearchMatches::new([scalar.as_str()],fold,PageSearchOptions::default()).collect::<Vec<_>>(),vec![0..1],"U+{cp:04X}");
            if fold.chars().count()>1 {
                let prefix=fold.chars().next().unwrap().to_string();
                assert!(SearchMatches::new([scalar.as_str()],&prefix,PageSearchOptions::default()).next().is_none(),"Partial U+{cp:04X}");
            }
        }
    }
}
