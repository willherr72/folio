//! Find-only transport: locations cross IPC; extracted page text stays native.
use super::*;
use super::search_matching::{SearchMatches,whitespace};
const MAX_QUERY:usize=1024;
const MAX_CHARACTERS:usize=250_000;
const MAX_MATCHES:usize=1000;
const MAX_RECTS:usize=10_000;

impl PdfEngine {
    pub fn search_page(&self,source_id:&str,page_index:usize,query:&str)->EngineResult<PageSearchResult> {
        self.search_page_with_options(source_id,page_index,query,PageSearchOptions::default())
    }
    pub fn search_page_with_options(&self,source_id:&str,page_index:usize,query:&str,options:PageSearchOptions)->EngineResult<PageSearchResult> {
        validate_query(query)?;
        let (reply,receive)=mpsc::channel();
        self.send(WorkerRequest::SearchPage {source_id:source_id.into(),page_index,query:query.into(),options,reply})?;
        self.in_points(source_id,page_index,receive.recv().map_err(|_|EngineError::WorkerStopped)?)
    }
}
impl WorkerRuntime {
    pub(super) fn search_page(&self,source_id:&str,page_index:usize,query:&str,options:PageSearchOptions)->EngineResult<PageSearchResult> {
        validate_query(query)?;
        if self.document(source_id)?.for_printing {return Err(EngineError::PermissionDenied("searching a prepared print document"));}
        let text=self.read_page_text(source_id,page_index,Some(MAX_CHARACTERS))?;
        Ok(find_matches(&text,query,options))
    }
}
fn validate_query(query:&str)->EngineResult<()> {
    if query.len()>MAX_QUERY {return Err(EngineError::InvalidRequest("Search text exceeds 1,024 bytes.".into()));}
    if query.chars().all(whitespace) {return Err(EngineError::InvalidRequest("Enter text to find.".into()));}
    Ok(())
}
fn find_matches(text:&PageText,query:&str,options:PageSearchOptions)->PageSearchResult {
    let ranges=SearchMatches::new(text.characters.iter().map(|c|c.text.as_str()),query,options);
    let mut result=PageSearchResult {matches:Vec::new(),has_text:ranges.has_text(),truncated:false};
    let mut rectangles=0;
    for range in ranges {
        if result.matches.len()>=MAX_MATCHES {result.truncated=true;break;}
        let rects=merge_rects(&text.characters[range],matches!(text.intrinsic_rotation,90|270));
        if rectangles+rects.len()>MAX_RECTS {result.truncated=true;break;}
        rectangles+=rects.len();result.matches.push(rects);
    }
    result
}
pub(super) fn merge_rects(characters:&[PdfTextCharacter],vertical:bool)->Vec<AnnotationRect> {
    let mut rects:Vec<AnnotationRect>=Vec::new();
    for c in characters {
        if c.text.chars().all(whitespace)||c.width<=0.0||c.height<=0.0||![c.x,c.y,c.width,c.height].iter().all(|n|n.is_finite()){continue;}
        if let Some(last)=rects.last_mut() {
            let (overlap,gap,thickness)=if vertical {
                ((last.x+last.width).min(c.x+c.width)-last.x.max(c.x),
                 (c.y-last.y-last.height).max(last.y-c.y-c.height).max(0.0),last.width.min(c.width))
            } else {
                ((last.y+last.height).min(c.y+c.height)-last.y.max(c.y),
                 (c.x-last.x-last.width).max(last.x-c.x-c.width).max(0.0),last.height.min(c.height))
            };
            if overlap>=thickness*0.5&&gap<=thickness {
                let right=(last.x+last.width).max(c.x+c.width);let bottom=(last.y+last.height).max(c.y+c.height);
                last.x=last.x.min(c.x);last.y=last.y.min(c.y);last.width=right-last.x;last.height=bottom-last.y;continue;
            }
        }
        rects.push(AnnotationRect{x:c.x,y:c.y,width:c.width,height:c.height});
    }rects
}

#[cfg(test)]
mod tests {
    use super::*;
    fn find_default(text:&PageText,query:&str)->PageSearchResult {find_matches(text,query,PageSearchOptions::default())}
    #[test]
    fn uppercase_greek_queries_find_their_original_glyphs() {
        let found=find_default(&text("ΟΣ ος οσ",0),"ΟΣ");
        assert_eq!(found.matches.len(),3);
        assert_eq!(found.matches[0][0].x,0.0);
        assert_eq!(found.matches[2][0].x,36.0);
    }
    fn text(value:&str,rotation:u16)->PageText {PageText {intrinsic_rotation:rotation,characters:value.chars().enumerate().map(|(i,c)|PdfTextCharacter{text:c.to_string(),x:if rotation==90{20.0}else{i as f32*6.0},y:if rotation==90{i as f32*6.0}else{20.0},width:6.0,height:6.0}).collect()}}
    #[test]
    fn unicode_expansions_supplementary_scalars_and_whitespace_keep_their_glyph_bounds() {
        let source=text("İ𝄞\u{a0}\r\nCAFÉ",0);
        let found=find_default(&source," i\u{307}𝄞 café ");assert_eq!(found.matches.len(),1);
        assert_eq!(found.matches[0][0].x,0.0);assert_eq!(found.matches[0].last().unwrap().x+found.matches[0].last().unwrap().width,54.0);
        assert!(find_default(&source,"no match").has_text);
        assert!(!find_default(&text("\u{feff} \r\n",0),"a").has_text);
        assert_eq!(find_default(&text("ABCD",90),"bc").matches[0],vec![AnnotationRect{x:20.0,y:6.0,width:6.0,height:12.0}]);
    }
    #[test]
    fn bounded_results_report_truncation_and_nonoverlapping_matches() {
        let result=find_default(&text(&"a ".repeat(MAX_MATCHES+1),0),"a");assert!(result.truncated);assert_eq!(result.matches.len(),MAX_MATCHES);
        assert!(!find_default(&text(&"a ".repeat(MAX_MATCHES),0),"a").truncated);
        assert_eq!(find_default(&text("aaaaa",0),"aa").matches.len(),2);
        assert!(validate_query(" \u{feff}").is_err());assert!(validate_query(&"é".repeat(513)).is_err());
    }
    #[test]
    fn whole_word_rejections_do_not_spend_the_result_budget_and_options_are_strict(){
        let source=text(&format!("{}a","ba ".repeat(MAX_MATCHES+1)),0);
        let options=PageSearchOptions {match_case:false,whole_words:true};
        let found=find_matches(&source,"a",options);assert_eq!(found.matches.len(),1);assert!(!found.truncated);
        assert_eq!(serde_json::from_str::<PageSearchOptions>("{}").unwrap(),PageSearchOptions::default());
        assert!(serde_json::from_str::<PageSearchOptions>(r#"{"matchCase":"true"}"#).is_err());
        assert!(serde_json::from_str::<PageSearchOptions>(r#"{"wholeWord":true}"#).is_err());
    }
}
