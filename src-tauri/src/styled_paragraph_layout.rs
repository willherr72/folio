//! Deterministic layout for source-bound attributed text. Coordinates are raw PDF.
use super::*;

#[derive(Clone,Copy)]
pub(super) struct GlyphMetric { pub advance:f32, pub ink:[f32;4] }
pub(super) struct StyleMetrics {
    pub em:f32, pub ascent:f32, pub descent:f32,
    pub glyphs:HashMap<char,GlyphMetric>,
}
#[derive(Clone,Copy)]
struct Atom { ch:char, style:usize }
struct Row { atoms:Vec<Atom>, blank_style:usize, soft:bool }
struct Extents { left:f32, right:f32, ascent:f32, descent:f32, em:f32 }

pub(super) fn validate_draft(runs:&[StyledParagraphRun])->EngineResult<()> {
    if runs.is_empty() || runs.len()>4096 {return Err(invalid("Use 1 to 4096 paragraph formatting spans."));}
    let mut bytes=0usize;let mut visible=false;
    for run in runs {
        bytes=bytes.checked_add(run.text.len()).filter(|n|*n<=4096)
            .ok_or_else(||invalid("Use at most 4096 UTF-8 bytes for a paragraph."))?;
        if run.text.chars().any(|c|c.is_control()&&c!='\n') {return Err(invalid("Use printable text and LF hard breaks in the paragraph."));}
        visible|=!run.text.trim().is_empty();
    }
    if !visible {return Err(invalid("Enter some visible paragraph text."));}
    Ok(())
}

fn extent(atoms:&[Atom],styles:&[StyleMetrics],blank_style:usize,gaps:&[usize],extra:f32)->Extents {
    let fallback=&styles[atoms.first().map_or(blank_style,|atom|atom.style)];
    let mut result=Extents{left:0.,right:0.,ascent:fallback.ascent,descent:fallback.descent,em:fallback.em};
    let visible_end=atoms.iter().rposition(|a|a.ch!=' ').map_or(0,|i|i+1);
    let mut cursor=0.;
    for (index,atom) in atoms.iter().enumerate() {
        let style=&styles[atom.style];let metric=style.glyphs[&atom.ch];
        result.ascent=result.ascent.max(style.ascent).max(metric.ink[3]);
        result.descent=result.descent.max(style.descent).max(-metric.ink[1]);
        result.em=result.em.max(style.em);
        if index>=visible_end {continue;}
        if gaps.binary_search(&index).is_ok(){cursor+=extra;}
        result.left=result.left.min(cursor+metric.ink[0]);
        result.right=result.right.max(cursor+metric.ink[2]);
        cursor+=metric.advance;result.right=result.right.max(cursor);
    }
    result
}

// Horizontal measurement and nominal em stay identical across saved policies.
// Only painted glyphs reserve vertical ink; empty/space-only rows keep their
// complete font metrics so deliberate blank lines retain their style height.
fn row_extent(atoms:&[Atom],styles:&[StyleMetrics],blank_style:usize,gaps:&[usize],extra:f32,ink_vertical:bool)->Extents {
    let mut result=extent(atoms,styles,blank_style,gaps,extra);
    if ink_vertical {
        let mut ink:Option<(f32,f32)>=None;
        for atom in atoms {
            let bounds=styles[atom.style].glyphs[&atom.ch].ink;
            if atom.ch.is_whitespace()||bounds[2]<=bounds[0]||bounds[3]<=bounds[1]{continue;}
            let (ascent,descent)=ink.unwrap_or((0.,0.));
            ink=Some((ascent.max(bounds[3]),descent.max(-bounds[1])));
        }
        if let Some((ascent,descent))=ink {result.ascent=ascent;result.descent=descent;}
    }
    result
}

fn push_row(rows:&mut Vec<Row>,atoms:Vec<Atom>,blank_style:usize,soft:bool)->EngineResult<()> {
    if rows.len()>=256{return Err(invalid("The paragraph exceeds 256 lines."));}
    rows.push(Row{atoms,blank_style,soft});Ok(())
}

fn wrap_hard_line(atoms:&[Atom],blank_style:usize,styles:&[StyleMetrics],width:f32,rows:&mut Vec<Row>,allow_overwide_words:bool,first_only:bool)->EngineResult<()> {
    let mut current=Vec::new();let mut start=0;
    while start<atoms.len() {
        // A word includes all of its style spans and its following space group.
        // Leading spaces are kept with the first word, never discarded on wrap.
        let mut end=start;
        if start==0 {while end<atoms.len()&&atoms[end].ch==' ' {end+=1;}}
        while end<atoms.len()&&atoms[end].ch!=' ' {end+=1;}
        while end<atoms.len()&&atoms[end].ch==' ' {end+=1;}
        let word=&atoms[start..end];
        let word_bounds=extent(word,styles,word[0].style,&[],0.);
        if !allow_overwide_words&&word_bounds.right-word_bounds.left>width+EPSILON {
            return Err(invalid("A word does not fit in the paragraph area. Enlarge the area in the text's direction."));
        }
        let old_length=current.len();current.extend_from_slice(word);
        let combined=extent(&current,styles,blank_style,&[],0.);
        if old_length>0 && combined.right-combined.left>width+EPSILON {
            let next=current.split_off(old_length);
            push_row(rows,std::mem::replace(&mut current,next),blank_style,true)?;
            if first_only{return Ok(());}
        }
        start=end;
    }
    let style=current.first().map_or(blank_style,|a|a.style);
    push_row(rows,current,style,false)
}

fn gaps(atoms:&[Atom])->Vec<usize> {
    let mut result=Vec::new();let mut seen_word=false;let mut space=false;
    for (index,atom) in atoms.iter().enumerate(){
        if atom.ch==' ' {space=true;}else{
            if seen_word&&space {result.push(index);}
            seen_word=true;space=false;
        }
    }
    result
}

#[cfg(test)]
pub(super) fn layout(runs:&[StyledParagraphRun],styles:&[StyleMetrics],bounds:[f32;4],alignment:ParagraphAlignment,spacing:f32)->EngineResult<StyledParagraphLayout> {layout_in_units(runs,styles,bounds,alignment,spacing,1.)}

fn checked_atoms(runs:&[StyledParagraphRun],styles:&[StyleMetrics],unit:f32)->EngineResult<Vec<Atom>> {
    validate_draft(runs)?;
    if styles.is_empty()||styles.len()>64||runs.iter().any(|r|r.style_id>=styles.len()) {
        return Err(invalid("A paragraph style is not part of the checked source selection."));
    }
    for style in styles {
        if !physical_units::paragraph_size(style.em,unit)
            ||[style.ascent,style.descent].iter().any(|v|!v.is_finite()||!(0.0..=100_000.).contains(v))
            ||style.ascent+style.descent<=0. {
            return Err(invalid("The paragraph font has invalid vertical metrics."));
        }
    }
    let mut atoms=Vec::new();
    for run in runs {for ch in run.text.chars(){
        if ch!='\n' {
            let metric=styles[run.style_id].glyphs.get(&ch).ok_or_else(||invalid("The paragraph has an unmeasured or unsupported glyph."))?;
            if !metric.advance.is_finite()||!(0.0..=100_000.).contains(&metric.advance)
                ||metric.ink.iter().any(|v|!v.is_finite()||v.abs()>100_000.)
                ||metric.ink[2]<metric.ink[0]||metric.ink[3]<metric.ink[1] {
                return Err(invalid("The paragraph glyph has invalid metrics."));
            }
        }
        atoms.push(Atom{ch,style:run.style_id});
    }}
    Ok(atoms)
}

fn measured_rows(runs:&[StyledParagraphRun],styles:&[StyleMetrics],width:f32,unit:f32,allow_overwide_words:bool,first_only:bool)->EngineResult<Vec<Row>> {
    if !width.is_finite()||width<=0.||width>2_000_000. {return Err(invalid("Use a finite positive paragraph width."));}
    let atoms=checked_atoms(runs,styles,unit)?;
    let mut rows=Vec::new();let mut start=0;let mut last_break_style=atoms[0].style;
    for (index,atom) in atoms.iter().enumerate() {if atom.ch=='\n'{
        let style=if index==start {atom.style}else{atoms[start].style};
        wrap_hard_line(&atoms[start..index],style,styles,width,&mut rows,allow_overwide_words,first_only)?;
        if first_only{return Ok(rows);}
        start=index+1;last_break_style=atom.style;
    }}
    let style=atoms.get(start).map_or(last_break_style,|a|a.style);
    wrap_hard_line(&atoms[start..],style,styles,width,&mut rows,allow_overwide_words,first_only)?;

    Ok(rows)
}

/// Measurement only: an oversized word still has an ascent. Publication retains
/// the strict width check; automatic fitting may widen an initially narrow area.
#[cfg(test)]
pub(super) fn first_line_ascent(runs:&[StyledParagraphRun],styles:&[StyleMetrics],width:f32,unit:f32)->EngineResult<f32> {
    first_line_ascent_with_text_metrics(runs,styles,styles,width,unit,false)
}
pub(super) fn first_line_ascent_with_text_metrics(runs:&[StyledParagraphRun],styles:&[StyleMetrics],text_styles:&[StyleMetrics],width:f32,unit:f32,ink_vertical:bool)->EngineResult<f32> {
    // Validate all glyphs, but measure only the first row. A narrow starting
    // width may create more than 256 soft rows before Fit can widen it. The
    // complete-layout path below still enforces its original row limit.
    let rows=measured_rows(runs,text_styles,width,unit,true,true)?;
    if !std::ptr::eq(styles,text_styles){checked_atoms(runs,styles,unit)?;}
    let first=rows.first().ok_or_else(||invalid("The paragraph has no first line."))?;
    Ok(row_extent(&first.atoms,styles,first.blank_style,&[],0.,ink_vertical).ascent)
}

pub(super) fn layout_in_units(runs:&[StyledParagraphRun],styles:&[StyleMetrics],bounds:[f32;4],alignment:ParagraphAlignment,spacing:f32,unit:f32)->EngineResult<StyledParagraphLayout> {
    layout_with_metrics(runs,styles,styles,bounds,alignment,spacing,unit,false,false,None)
}

/// Text metrics determine wrapping and horizontal alignment. Reservation metrics
/// retain highlight ascent/descent without allowing a mark to move its glyphs.
/// Callers reserve horizontal highlight insets outside this text frame.
pub(super) fn layout_with_text_metrics(runs:&[StyledParagraphRun],styles:&[StyleMetrics],text_styles:&[StyleMetrics],bounds:[f32;4],alignment:ParagraphAlignment,spacing:f32,unit:f32,ink_vertical:bool)->EngineResult<StyledParagraphLayout> {
    layout_with_metrics(runs,styles,text_styles,bounds,alignment,spacing,unit,true,ink_vertical,None)
}
pub(super) fn layout_with_metrics(runs:&[StyledParagraphRun],styles:&[StyleMetrics],text_styles:&[StyleMetrics],bounds:[f32;4],alignment:ParagraphAlignment,spacing:f32,unit:f32,text_alignment:bool,ink_vertical:bool,first_baseline:Option<f32>)->EngineResult<StyledParagraphLayout> {
    if bounds.iter().any(|v|!v.is_finite()||v.abs()>1_000_000.)||bounds[2]<=bounds[0]||bounds[3]<=bounds[1]
        ||!spacing.is_finite()||!(0.6..=3.).contains(&spacing) {
        return Err(invalid("Use a finite paragraph area and line spacing from 0.6 to 3."));
    }
    let width=bounds[2]-bounds[0];
    let rows=measured_rows(runs,text_styles,width,unit,false,false)?;
    if !std::ptr::eq(styles,text_styles){
        // Validate both palettes before indexing either, including empty styles.
        checked_atoms(runs,styles,unit)?;
    }
    let mut lines=Vec::new();let mut previous:Option<(f32,f32,f32)>=None; // baseline, descent, em
    for row in rows {
        let natural=extent(&row.atoms,text_styles,row.blank_style,&[],0.);
        let gaps=if alignment==ParagraphAlignment::Justify&&row.soft {gaps(&row.atoms)}else{vec![]};
        let mut extra=0.;
        if !gaps.is_empty()&&natural.right-natural.left<width {
            // The feasible widths form an interval containing zero expansion.
            // This also handles a later glyph with a large negative bearing.
            let mut high=width+(natural.right-natural.left);let mut low=0.;
            for _ in 0..32 {
                let trial=extent(&row.atoms,text_styles,row.blank_style,&gaps,high);
                if trial.right-trial.left>width {break;}
                high*=2.;
            }
            for _ in 0..32 {
                let middle=(low+high)*0.5;let trial=extent(&row.atoms,text_styles,row.blank_style,&gaps,middle);
                if trial.right-trial.left>width {high=middle;}else{low=middle;}
            }
            extra=low;
        }
        let measured=row_extent(&row.atoms,styles,row.blank_style,&gaps,extra,ink_vertical);
        let text=extent(&row.atoms,text_styles,row.blank_style,&gaps,extra);
        // Wrapping admits only EPSILON overflow. Keep its center/right anchor
        // exact rather than shifting by half/all that tolerance. Old records
        // retain their original clamped placement equation.
        let spare=width-(text.right-text.left);
        let spare=if text_alignment{spare}else{spare.max(0.)};
        let offset=match alignment {ParagraphAlignment::Center=>spare*0.5,ParagraphAlignment::Right=>spare,_=>0.};
        let baseline=if let Some((baseline,descent,em))=previous {
            baseline-(descent+measured.ascent).max(em.max(measured.em)*spacing)
        }else{first_baseline.unwrap_or(bounds[3]-measured.ascent)};
        if !baseline.is_finite()||baseline-measured.descent<bounds[1]-EPSILON
            ||(first_baseline.is_some()&&baseline+measured.ascent>bounds[3]+EPSILON) {
            return Err(invalid("The text does not fit in the paragraph area. Enlarge the area or shorten the text."));
        }
        let mut placed:Vec<StyledParagraphPlacedRun>=Vec::new();let mut cursor=bounds[0]+offset-text.left;
        for (index,atom) in row.atoms.iter().enumerate(){
            let expanded=extra>0.&&gaps.binary_search(&index).is_ok();
            if expanded {cursor+=extra;}
            if let Some(last)=placed.last_mut().filter(|r|r.style_id==atom.style&&!expanded){last.text.push(atom.ch);}
            else{placed.push(StyledParagraphPlacedRun{text:atom.ch.to_string(),style_id:atom.style,origin:[cursor,baseline]});}
            cursor+=styles[atom.style].glyphs[&atom.ch].advance;
            if !cursor.is_finite()||cursor.abs()>1_000_000. {return Err(invalid("The paragraph cursor exceeds the supported limit."));}
        }
        lines.push(StyledParagraphLayoutLine{runs:placed,baseline,ascent:measured.ascent,descent:measured.descent,soft_wrapped:row.soft});
        previous=Some((baseline,measured.descent,measured.em));
    }
    Ok(StyledParagraphLayout{lines,pdf_bounds:bounds})
}

#[cfg(test)]
#[path="styled_paragraph_layout_tests.rs"]
mod tests;
