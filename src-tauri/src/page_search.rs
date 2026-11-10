//! Find-only transport: locations cross IPC; extracted page text stays native.
use super::*;
const MAX_QUERY:usize=1024;
const MAX_CHARACTERS:usize=250_000;
const MAX_MATCHES:usize=1000;
const MAX_RECTS:usize=10_000;

impl PdfEngine {
    pub fn search_page(&self,source_id:&str,page_index:usize,query:&str)->EngineResult<PageSearchResult> {
        validate_query(query)?;
        let (reply,receive)=mpsc::channel();
        self.send(WorkerRequest::SearchPage {source_id:source_id.into(),page_index,query:query.into(),reply})?;
        self.in_points(source_id,page_index,receive.recv().map_err(|_|EngineError::WorkerStopped)?)
    }
}
impl WorkerRuntime {
    pub(super) fn search_page(&self,source_id:&str,page_index:usize,query:&str)->EngineResult<PageSearchResult> {
        validate_query(query)?;
        if self.document(source_id)?.for_printing {return Err(EngineError::PermissionDenied("searching a prepared print document"));}
        let text=self.read_page_text(source_id,page_index,Some(MAX_CHARACTERS))?;
        Ok(find_matches(&text,query))
    }
}
fn validate_query(query:&str)->EngineResult<()> {
    if query.len()>MAX_QUERY {return Err(EngineError::InvalidRequest("Search text exceeds 1,024 bytes.".into()));}
    if query.chars().all(whitespace) {return Err(EngineError::InvalidRequest("Enter text to find.".into()));}
    Ok(())
}
// Match browser whitespace normalization, including NBSP and BOM (but not NEL).
fn whitespace(c:char)->bool {matches!(c,'\u{0009}'..='\u{000d}'|'\u{0020}'|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}')}
fn append(value:&mut String,glyphs:&mut Vec<usize>,text:&str,index:usize) {
    for c in text.chars() {
        if whitespace(c) {
            if !value.is_empty()&&!value.ends_with(' ') {value.push(' ');glyphs.push(index);}
        } else {
            for lower in c.to_lowercase() {
                value.push(lower);glyphs.extend(std::iter::repeat_n(index,lower.len_utf8()));
            }
        }
    }
}
fn normalized_query(query:&str)->String {
    let mut value=String::new();let mut ignored=Vec::new();
    append(&mut value,&mut ignored,&query.to_lowercase(),0);
    if value.ends_with(' '){value.pop();}value
}
fn find_matches(text:&PageText,query:&str)->PageSearchResult {
    let needle=normalized_query(query);
    let mut value=String::new();let mut glyphs=Vec::new();
    for (index,glyph) in text.characters.iter().enumerate() {append(&mut value,&mut glyphs,&glyph.text,index);}
    if value.ends_with(' '){value.pop();glyphs.pop();}
    let mut result=PageSearchResult {matches:Vec::new(),has_text:!value.is_empty(),truncated:false};
    if needle.is_empty(){return result;}
    let mut start=0;let mut rectangles=0;
    while let Some(found)=value[start..].find(&needle) {
        let offset=start+found;
        if result.matches.len()>=MAX_MATCHES {result.truncated=true;break;}
        let first=glyphs[offset];let last=glyphs[offset+needle.len()-1];
        let rects=merge_rects(&text.characters[first..=last],matches!(text.intrinsic_rotation,90|270));
        if rectangles+rects.len()>MAX_RECTS {result.truncated=true;break;}
        rectangles+=rects.len();result.matches.push(rects);start=offset+needle.len();
    }
    result
}
fn merge_rects(characters:&[PdfTextCharacter],vertical:bool)->Vec<AnnotationRect> {
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
    fn text(value:&str,rotation:u16)->PageText {PageText {intrinsic_rotation:rotation,characters:value.chars().enumerate().map(|(i,c)|PdfTextCharacter{text:c.to_string(),x:if rotation==90{20.0}else{i as f32*6.0},y:if rotation==90{i as f32*6.0}else{20.0},width:6.0,height:6.0}).collect()}}
    #[test]
    fn unicode_expansions_supplementary_scalars_and_whitespace_keep_their_glyph_bounds() {
        let source=text("İ𝄞\u{a0}\r\nCAFÉ",0);
        let found=find_matches(&source," i\u{307}𝄞 café ");assert_eq!(found.matches.len(),1);
        assert_eq!(found.matches[0][0].x,0.0);assert_eq!(found.matches[0].last().unwrap().x+found.matches[0].last().unwrap().width,54.0);
        assert!(find_matches(&source,"no match").has_text);
        assert!(!find_matches(&text("\u{feff} \r\n",0),"a").has_text);
        assert_eq!(find_matches(&text("ABCD",90),"bc").matches[0],vec![AnnotationRect{x:20.0,y:6.0,width:6.0,height:12.0}]);
    }
    #[test]
    fn bounded_results_report_truncation_and_nonoverlapping_matches() {
        let result=find_matches(&text(&"a ".repeat(MAX_MATCHES+1),0),"a");assert!(result.truncated);assert_eq!(result.matches.len(),MAX_MATCHES);
        assert!(!find_matches(&text(&"a ".repeat(MAX_MATCHES),0),"a").truncated);
        assert_eq!(find_matches(&text("aaaaa",0),"aa").matches.len(),2);
        assert!(validate_query(" \u{feff}").is_err());assert!(validate_query(&"é".repeat(513)).is_err());
    }
}
