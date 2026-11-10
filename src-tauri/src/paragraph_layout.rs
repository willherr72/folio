//! Word wrapping and explicit interword positioning for uniform paragraphs.
use super::*;

pub(super) struct Line { pub text:String, pub soft_wrapped:bool }

// Shared by publishing and validating saved paragraph hints. Validation must
// reproduce the layout, not merely trust a record's claimed settings.
pub(super) fn positioned_line(
    run: &RunSnapshot, substituted: bool, wrapped: &Line, line_index: usize,
    bounds: [f32; 4], size: f32, alignment: ParagraphAlignment, spacing: f32,
    metrics: &HashMap<char, [f32; 3]>,
) -> EngineResult<([f32; 2], Operation)> {
    let line = &wrapped.text;
    let justified = (alignment == ParagraphAlignment::Justify && wrapped.soft_wrapped)
        .then(|| justify(line, bounds[2] - bounds[0], metrics));
    let (left, right) = justified.as_ref().map(|j| (j.left, j.right))
        .unwrap_or_else(|| ink_extents(line.trim_end_matches(' '), metrics));
    let spare = (bounds[2] - bounds[0] - (right - left)).max(0.0);
    let offset = match alignment {
        ParagraphAlignment::Left | ParagraphAlignment::Justify => 0.0,
        ParagraphAlignment::Center => spare * 0.5,
        ParagraphAlignment::Right => spare,
    };
    let origin = [bounds[0] + offset - left, bounds[3] - size - line_index as f32 * size * spacing];
    let encode = |text: &str| -> EngineResult<Object> {
        Ok(Object::String(if substituted { paragraph_font::encode(text) }
            else { positioned_text::encoded(run, text)? }, StringFormat::Hexadecimal))
    };
    let operation = if let Some(j) = justified.filter(|j| j.extra > 0. && !j.gaps.is_empty()) {
        let gap = -j.extra * 1000. / size;
        if !gap.is_finite() || gap.abs() > 1_000_000. {
            return Err(invalid("Justified word spacing exceeds the supported limit."));
        }
        let mut values = Vec::new();
        let mut start = 0;
        for end in j.gaps { values.push(encode(&line[start..end])?); values.push(Object::Real(gap)); start = end; }
        values.push(encode(&line[start..])?);
        Operation::new("TJ", vec![Object::Array(values)])
    } else { Operation::new("Tj", vec![encode(line)?]) };
    Ok((origin, operation))
}
// PDFium's object-text API collapses consecutive ASCII spaces even when the PDF
// retains every space and its advance. Compare that extraction view while keeping
// the original decoded strings for layout, encoding and the editing draft.
pub(super) fn extraction_text(text:&str)->String {
    let mut previous_space=false;
    text.trim_end().chars().filter(|c|{
        let repeated=*c==' '&&previous_space;previous_space=*c==' ';!repeated
    }).collect()
}
pub(super) fn ink_extents(text:&str,metrics:&HashMap<char,[f32;3]>)->(f32,f32) {
    spaced_extents(text,metrics,&[],0.)
}
fn spaced_extents(text:&str,metrics:&HashMap<char,[f32;3]>,gaps:&[usize],extra:f32)->(f32,f32) {
    let mut cursor=0_f32;let mut left=0_f32;let mut right=0_f32;
    for (index,c) in text.char_indices() {
        if gaps.binary_search(&index).is_ok(){cursor+=extra;}
        let glyph=metrics[&c];left=left.min(cursor+glyph[1]);right=right.max(cursor+glyph[2]);cursor+=glyph[0];
    }
    (left,right.max(cursor))
}
pub(super) fn wrap(run:&RunSnapshot,substituted:bool,text:&str,width:f32,metrics:&HashMap<char,[f32;3]>)->EngineResult<Vec<Line>> {
    if text.trim().is_empty()||text.len()>4096||text.contains('\r')||text.split('\n').any(|s|!s.is_empty()&&!(if run.embedded||run.standard.is_some()||substituted {s.bytes().all(|c|c==b' ')||embedded_font::latin(s)}else{s.bytes().all(|c|(32..=126).contains(&c))})) {
        return Err(invalid("Enter printable Latin text within 4096 UTF-8 bytes. Use newlines for hard breaks."));
    }
    let measured=|s:&str|{let (left,right)=ink_extents(s.trim_end_matches(' '),metrics);right-left};
    let mut lines=Vec::new();
    for hard_line in text.split('\n') {
        let mut line=String::new();
        for word in hard_line.split_inclusive(' ') {
            if !line.is_empty()&&measured(&format!("{line}{word}"))>width {lines.push(Line{text:std::mem::take(&mut line),soft_wrapped:true});}
            if measured(word)>width{return Err(invalid("A word does not fit in the paragraph area. Enlarge the area in the text's direction."));}
            line.push_str(word);
        }
        lines.push(Line{text:line,soft_wrapped:false});
    }
    if lines.len()>256{return Err(invalid("The paragraph exceeds 256 lines."));}
    Ok(lines)
}

pub(super) struct Justified { pub gaps:Vec<usize>, pub extra:f32, pub left:f32, pub right:f32 }
pub(super) fn justify(text:&str,width:f32,metrics:&HashMap<char,[f32;3]>)->Justified {
    let text=text.trim_end_matches(' ');let mut gaps=Vec::new();let mut seen_word=false;let mut in_space=false;
    for (index,c) in text.char_indices() {
        if c==' ' {in_space=true;} else {
            if seen_word&&in_space {gaps.push(index);}
            seen_word=true;in_space=false;
        }
    }
    let (left,right)=ink_extents(text,metrics);
    if gaps.is_empty()||right-left>=width{return Justified{gaps,extra:0.,left,right};}
    // Ink overhang can make a prefix wider than the final glyph's advance. Solve
    // for the complete positioned ink width instead of assuming width grows by
    // exactly gap_count * extra. The width is monotone for nonnegative spacing.
    let mut low=0.;let mut high=width+(right-left);
    for _ in 0..32 {
        let middle=(low+high)*0.5;let (a,b)=spaced_extents(text,metrics,&gaps,middle);
        if b-a>width {high=middle;}else{low=middle;}
    }
    let (left,right)=spaced_extents(text,metrics,&gaps,low);
    Justified{gaps,extra:low,left,right}
}

/// Recognize a conservative multi-row justification proposal from actual glyph
/// positions. Ordinary equal-width left/center/right text stays ambiguous.
pub(super) fn inferred_justification(selected:&[&RunSnapshot],size:f32,glyphs:&HashMap<usize,Vec<Glyph>>,source_text:&HashMap<usize,String>)->bool {
    if selected.len()<2{return false;}
    let tolerance=(size*0.02).clamp(0.02,0.25);
    let mut rows=Vec::new();
    for run in selected {
        let Some(chars)=glyphs.get(&run.index) else{return false;};
        let Some(extents)=alignment_extents(run,chars,size) else{return false;};
        let Some(logical)=source_text.get(&run.index) else{return false;};
        // Use source advances for all encoded spaces, including the ones omitted
        // from PDFium's extracted glyph sequence. Compare visible word origins.
        let mut natural=Vec::new();let mut cursor=0.;let mut spaces=false;
        for c in logical.chars() {
            let Ok(width)=advance(run,&c.to_string()) else{return false;};
            if c!=' ' {natural.push((c.to_string(),cursor,spaces));spaces=false;} else {spaces=true;}
            cursor+=width*size;
        }
        let visible:Vec<_>=chars.iter().filter(|g|g.text!=" ").collect();
        if visible.len()!=natural.len()||visible.iter().zip(&natural).any(|(g,n)|g.text!=n.0){return false;}
        let mut stretches=Vec::new();
        for index in 1..visible.len() {
            let difference=visible[index].position[0]-visible[index-1].position[0]-(natural[index].1-natural[index-1].1);
            if natural[index].2 {stretches.push(difference);}
            else if difference.abs()>tolerance {return false;}
        }
        // Include natural gaps: a row mixing expanded and unexpanded gaps is not
        // evidence of uniform justification and must not change default re-edits.
        let stretch=stretches.first().is_some_and(|first|*first>size*0.05&&stretches.iter().all(|value|*value>size*0.05&&(value-first).abs()<=tolerance));
        rows.push((run.matrix[5],extents,stretch));
    }
    let first=rows[0];
    rows.iter().enumerate().all(|(index,row)|{
        (row.1[0]-first.1[0]).abs()<=tolerance && row.1[1]<=first.1[1]+tolerance
        && (index==0||rows[index-1].0-row.0>size*0.5)
        && (index==rows.len()-1||(row.2&&(row.1[1]-first.1[1]).abs()<=tolerance))
    })
}
