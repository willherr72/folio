//! Source-row structure. Native ink and captured source state are separate gates.
use super::*;
use crate::choice_font::ChoiceFontProof;
use lopdf::{Dictionary, Document};
use std::collections::{BTreeSet, HashMap, HashSet};
use lopdf::{ObjectId, Stream};
use pdfium_render::prelude::Pdfium;
#[path = "form_list_source_model.rs"]
mod model;
#[path = "form_list_source_ink.rs"]
mod ink;
pub(crate) use model::CapturedListSources;

#[cfg(test)]
#[path = "form_list_source_tests.rs"]
mod tests;

const MAX_CHARACTERS: usize = 250_000;

struct ListModel {
    options: Vec<String>,
    selected: Vec<usize>,
    multiple: bool,
}

struct Row {
    index: usize,
    label: String,
    origin: [f64; 2],
    show: usize,
}

struct SelectionPaint {
    rectangle: [f64; 4],
    path: usize,
    paint: usize,
    rows: Vec<usize>,
}

/// A structural plan is deliberately not publication authority. It still needs
/// independently measured source ink before any recorded path may be removed.
struct Layout {
    operations: Vec<Operation>,
    marked: Range<usize>,
    text: Range<usize>,
    rows: Vec<Row>,
    paints: Vec<SelectionPaint>,
    clip: [f64; 4],
    basis: [f64; 6],
    fonts: BTreeSet<Vec<u8>>,
    characters: usize,
}

fn resolve<'a>(doc: &'a Document, mut value: &'a Object) -> EngineResult<&'a Object> {
    let mut seen = HashSet::new();
    for _ in 0..64 {
        let Object::Reference(id) = value else { return Ok(value) };
        if !seen.insert(*id) { return Err(bad("cyclic source list reference")); }
        value = doc.objects.get(id).ok_or_else(|| bad("missing source list reference"))?;
    }
    Err(bad("source list reference depth exceeds its limit"))
}

fn numeric(op: &Operation, index: usize) -> EngineResult<f64> {
    let value = op.operands.get(index).ok_or_else(|| bad("missing source list coordinate"))?
        .as_float().map_err(|_| bad("invalid source list coordinate"))? as f64;
    if !value.is_finite() { return Err(bad("nonfinite source list coordinate")); }
    Ok(value)
}

fn matrix(op: &Operation) -> EngineResult<[f64; 6]> {
    Ok([numeric(op, 0)?, numeric(op, 1)?, numeric(op, 2)?,
        numeric(op, 3)?, numeric(op, 4)?, numeric(op, 5)?])
}

fn point(m: [f64; 6], x: f64, y: f64) -> [f64; 2] {
    [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]]
}

fn epsilon(value: f64) -> f64 {
    let value = (value.abs() as f32).max(1.);
    let next = f32::from_bits(value.to_bits() + 1);
    (f64::from(next - value) * 4.).max(0.00001)
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= epsilon(a).max(epsilon(b))
}

/// Ownership tolerance follows the feature's extent, never its translation.
/// A large BBox origin must not turn half a column into a full-column paint.
fn extent_tolerance(span:f64)->f64 {
    epsilon(span).min(span.abs()*0.000001)
}

fn rectangle(values: [f64; 4], transform: [f64; 6]) -> EngineResult<[f64; 4]> {
    if !(near(transform[1], 0.) && near(transform[2], 0.)
        || near(transform[0], 0.) && near(transform[3], 0.)) {
        return Err(bad("source list rectangle is not aligned with its text frame"));
    }
    let a = point(transform, values[0], values[1]);
    let b = point(transform, values[0] + values[2], values[1] + values[3]);
    let bounds = [a[0].min(b[0]), a[1].min(b[1]), a[0].max(b[0]), a[1].max(b[1])];
    if bounds.iter().any(|v| !v.is_finite()) || bounds[0] >= bounds[2] || bounds[1] >= bounds[3] {
        return Err(bad("empty source list rectangle"));
    }
    Ok(bounds)
}

fn intersection(a: [f64; 4], b: [f64; 4]) -> EngineResult<[f64; 4]> {
    let value = [a[0].max(b[0]), a[1].max(b[1]), a[2].min(b[2]), a[3].min(b[3])];
    if value[0] >= value[2] || value[1] >= value[3] { return Err(bad("empty source list clip")); }
    Ok(value)
}

#[derive(Clone)]
struct State {
    matrix: [f64; 6],
    clip: Option<usize>,
    font: Option<Vec<u8>>,
    size: f64,
    leading: f64,
    mode: i64,
    rise: f64,
}
impl Default for State {
    fn default() -> Self {
        Self { matrix: IDENTITY, clip: None, font: None, size: 0., leading: 0., mode: 0, rise: 0. }
    }
}

fn analyse(
    source: &Document, resources: &Dictionary, bytes: &[u8], bounds: [f64; 4],
    model: &ListModel, top: usize,
) -> EngineResult<Layout> {
    preflight(bytes)?;
    if top >= model.options.len()
        || !model.multiple && model.selected.len() > 1
        || model.selected.iter().any(|i| *i >= model.options.len())
        || model.selected.windows(2).any(|p| p[0] >= p[1]) {
        return Err(bad("source list state is inconsistent"));
    }
    let operations = Content::decode_strict(bytes).map_err(|_| bad("malformed source list program"))?.operations;
    let mut starts = operations.iter().enumerate().filter(|(_, op)| op.operator == "BMC");
    let mut ends = operations.iter().enumerate().filter(|(_, op)| op.operator == "EMC");
    let start = starts.next().ok_or_else(||bad("source list has no answer marker"))?;
    let end = ends.next().ok_or_else(||bad("source list has no answer end"))?;
    if starts.next().is_some() || ends.next().is_some() || start.0 >= end.0
        || name(start.1)? != b"Tx" {
        return Err(bad("source list has no unique answer marker"));
    }
    let marked = start.0..end.0 + 1;
    validate(&operations, &marked, true, AnswerKind::List, true)?;
    let first = operations[marked.clone()].iter().position(|op| op.operator == "BT")
        .ok_or_else(|| bad("source list has no option text"))? + marked.start;
    let last = operations[marked.clone()].iter().rposition(|op| op.operator == "ET")
        .ok_or_else(|| bad("source list has no text end"))? + marked.start;
    let text = first..last + 1;
    let mut depth = 0usize;
    for op in &operations[text.clone()] {
        match op.operator.as_str() {
            "q" => depth += 1,
            "Q" => depth = depth.checked_sub(1).ok_or_else(|| bad("source list text escapes its frame"))?,
            _ => {}
        }
    }
    if depth != 0 { return Err(bad("source list text leaves an open frame")); }
    let (entry, answer_clip, clips) = capture(&operations, &text)?;
    let basis = inverse(entry.matrix)?;
    let mut clip = rectangle([bounds[0], bounds[1], bounds[2] - bounds[0], bounds[3] - bounds[1]], basis)?;
    let mut chain = answer_clip;
    while let Some(index) = chain {
        let node = &clips[index];
        let mut paths = operations[node.path.clone()].iter().filter(|op| matches!(op.operator.as_str(), "m" | "l" | "c" | "v" | "y" | "h" | "re"));
        let op=paths.next().ok_or_else(||bad("source list clip has no path"))?;
        if paths.next().is_some() || op.operator != "re" { return Err(bad("source list clip is not one rectangle")); }
        clip = intersection(clip, rectangle([numeric(op,0)?,numeric(op,1)?,numeric(op,2)?,numeric(op,3)?], compose(basis,node.matrix)?)?)?;
        chain = node.parent;
    }

    // Existing static-frame admission identifies the border/background, never
    // the semantics of an internal rectangle. Keep its original index mapping.
    let mut working = crate::form_graph::CopyBudget::with_byte_limit(64*1024*1024);
    for op in &operations {
        working.reserve_bytes(std::mem::size_of::<Operation>()+op.operator.len())?;
        working.array(&op.operands)?;
    }
    working.reserve_bytes(clips.len().checked_mul(64).ok_or_else(||bad("source list clip map overflow"))?)?;
    let clip_ends: HashMap<_,_> = clips.iter().enumerate().map(|(index,node)|(node.path.end,index)).collect();
    let unmarked: Vec<_> = operations.iter().enumerate()
        .filter(|(i,_)| *i != marked.start && *i + 1 != marked.end).map(|(_,op)|op.clone()).collect();
    let frame = crate::form_frame_paint::FramePaint::inspect_with_absence(&encode_bounded(&unmarked)?,bounds,false,true)
        .map_err(|_| bad("source list frame cannot be proved"))?;
    let mut static_paints = frame.edge_paints.clone();
    if frame.border != usize::MAX { static_paints.push(frame.border); }
    if let Some(index) = frame.background { static_paints.push(index); }
    drop(frame);

    let mut state = State::default();
    let mut stack = Vec::new();
    let mut line = IDENTITY;
    let mut text_matrix = IDENTITY;
    let mut path: Option<(usize, [f64;4], bool)> = None;
    // PDF paths survive q/Q. An unrecognized subpath must not become empty
    // merely because no rectangle has been recorded yet.
    let mut path_open = false;
    let mut pending = false;
    let mut rows = Vec::new();
    let mut paints = Vec::new();
    let mut fonts = BTreeSet::new();
    let mut characters = 0usize;
    let mut decoders = HashMap::<Vec<u8>,ChoiceFontProof>::new();
    for (i, op) in operations.iter().enumerate() {
        match op.operator.as_str() {
            "q" => {
                working.reserve_bytes(std::mem::size_of::<State>()+state.font.as_ref().map_or(0,Vec::len))?;
                stack.push(state.clone());
            },
            "Q" => state = stack.pop().ok_or_else(|| bad("source list graphics restore is unmatched"))?,
            "cm" => state.matrix = compose(state.matrix, matrix(op)?)?,
            "BT" => { line=IDENTITY; text_matrix=IDENTITY; },
            "Tf" => {
                let font=op.operands[0].as_name().map_err(|_|bad("invalid source list font name"))?;
                working.reserve_bytes(font.len())?;
                state.font=Some(font.to_vec()); state.size=numeric(op,1)?;
            },
            "TL" => state.leading=numeric(op,0)?,
            "Ts" => state.rise=numeric(op,0)?,
            "Tr" => state.mode=op.operands[0].as_i64().map_err(|_|bad("invalid list text mode"))?,
            "Tm" => { line=matrix(op)?; text_matrix=line; },
            "Td" | "TD" => {
                let y=numeric(op,1)?;
                line=compose(line,[1.,0.,0.,1.,numeric(op,0)?,y])?; text_matrix=line;
                if op.operator=="TD" { state.leading=-y; }
            },
            "T*" | "'" | "\"" => {
                line=compose(line,[1.,0.,0.,1.,0.,-state.leading])?; text_matrix=line;
            },
            "re" => {
                let r=rectangle([numeric(op,0)?,numeric(op,1)?,numeric(op,2)?,numeric(op,3)?],compose(basis,state.matrix)?)?;
                path=Some((i,r,!path_open));path_open=true;
            },
            "m" | "l" | "c" | "v" | "y" | "h" => {
                path_open=true;
                if let Some(path)=&mut path {path.2=false;}
            },
            "W" | "W*" => pending=true,
            _ => {}
        }
        if marked.contains(&i) && matches!(op.operator.as_str(),"Tj"|"TJ"|"'"|"\"") {
            if state.mode != 0 || !near(state.rise,0.) || state.size<=0. || state.clip!=answer_clip {
                return Err(bad("source list row has unproved visible text state"));
            }
            let font=state.font.as_ref().ok_or_else(||bad("source list row has no font"))?;
            if !decoders.contains_key(font) {
                let table=resolve(source,resources.get(b"Font").map_err(|_|bad("source list fonts are missing"))?)?.as_dict().map_err(|_|bad("invalid source list fonts"))?;
                let value=table.get(font).map_err(|_|bad("source list font is missing"))?;
                let dictionary=resolve(source,value)?.as_dict().map_err(|_|bad("invalid source list font"))?;
                if resolve(source,dictionary.get(b"Type").map_err(|_|bad("source list font has no type"))?)?.as_name().ok()!=Some(b"Font") {
                    return Err(bad("invalid source list font type"));
                }
                // A verified code domain has at most 65536 entries. Charge a
                // conservative bound for both maps once per resource alias;
                // alternating fonts cannot repeatedly rebuild an unbounded map.
                let standard=dictionary.get(b"Subtype").and_then(Object::as_name).ok()==Some(b"Type1");
                working.reserve_bytes(if standard{128*1024}else{32*1024*1024})?;
                working.reserve_bytes(font.len()+128)?;
                decoders.insert(font.clone(),ChoiceFontProof::for_texts(source,value,&[])?);
            }
            working.reserve_bytes(font.len())?;
            if !fonts.contains(font){working.reserve_bytes(font.len()+64)?;fonts.insert(font.clone());}
            let proof=&decoders[font];
            let index=top.checked_add(rows.len()).ok_or_else(||bad("source list row index overflow"))?;
            let expected=model.options.get(index).ok_or_else(||bad("source list has more rows than options"))?;
            working.reserve_bytes(expected.len())?;
            let mut label=String::with_capacity(expected.len());
            let mut append=|bytes:&[u8]|->EngineResult<()> {
                // decode() bounds UTF-8 to128KiB; charge its geometric String
                // capacity before reading codes, and never grow the row beyond
                // its already bounded captured option.
                working.reserve_bytes(bytes.len().checked_mul(8).ok_or_else(||bad("source list decode work overflow"))?.min(256*1024))?;
                let piece=proof.decode(bytes)?;
                if piece.len()>expected.len().saturating_sub(label.len()){return Err(bad("source list label does not match its option index"));}
                label.push_str(&piece);Ok(())
            };
            if op.operator=="TJ" {
                for value in op.operands[0].as_array().map_err(|_|bad("invalid source list text array"))? {
                    if let Object::String(bytes,_)=value {append(bytes)?;}
                    else if value.as_float().ok()!=Some(0.) {return Err(bad("source list row has an unproved text adjustment"));}
                }
            } else {
                append(op.operands.last().ok_or_else(||bad("missing source list label"))?.as_str().map_err(|_|bad("invalid source list label"))?)?;
            }
            if expected!=&label {return Err(bad("source list label does not match its option index"));}
            characters=characters.checked_add(label.chars().count()).ok_or_else(||bad("source list character count overflow"))?;
            if characters>MAX_CHARACTERS {return Err(bad("source list glyph work exceeds its bound"));}
            let transform=compose(compose(basis,state.matrix)?,text_matrix)?;
            if !near(transform[1],0.) || !near(transform[2],0.) || transform[0]<=0. || transform[3]<=0. {
                return Err(bad("source list row has an unsupported writing direction"));
            }
            working.reserve_bytes(std::mem::size_of::<Row>()+label.capacity())?;
            rows.push(Row{index,label,origin:[transform[4],transform[5]],show:i});
        }
        if matches!(op.operator.as_str(),"n"|"S"|"s"|"f"|"F"|"f*"|"B"|"B*"|"b"|"b*") {
            if marked.contains(&i) && op.operator!="n" && !static_paints.contains(&(i-1)) {
                let Some((start,rectangle,true))=path else{return Err(bad("source list contains unidentified paint"))};
                let tolerance=extent_tolerance(clip[2]-clip[0]);
                if !matches!(op.operator.as_str(),"f"|"F"|"f*") || pending || state.clip!=answer_clip
                    || (rectangle[0]-clip[0]).abs()>tolerance || (rectangle[2]-clip[2]).abs()>tolerance {
                    return Err(bad("source list paint does not span its row column"));
                }
                if paints.len()>=model.selected.len() {return Err(bad("source list paint exceeds selected rows"));}
                working.reserve_bytes(std::mem::size_of::<SelectionPaint>())?;
                paints.push(SelectionPaint{rectangle,path:start,paint:i,rows:Vec::new()});
            }
            if let Some(index)=clip_ends.get(&i) {state.clip=Some(*index);}
            path=None; path_open=false; pending=false;
        }
    }
    if rows.is_empty() || paints.is_empty() {return Err(bad("source list has no provable selected rows"));}
    for pair in rows.windows(2) {
        if pair[0].origin[1]<=pair[1].origin[1] {return Err(bad("source list rows are not in display order"));}
    }
    let mut painted=HashSet::new();
    for paint in &mut paints {
        for (position,row) in rows.iter().enumerate() {
            if row.origin[1]>paint.rectangle[1] && row.origin[1]<paint.rectangle[3] {
                if !model.selected.contains(&row.index) || !painted.insert(row.index) {
                    return Err(bad("source list paint does not match selected indices"));
                }
                if paint.paint>=row.show {
                    return Err(bad("no proved source list background precedes its row text"));
                }
                paint.rows.push(position);
            }
        }
        if paint.rows.is_empty() {return Err(bad("source list paint has no selected row"));}
        let first=*paint.rows.first().unwrap(); let last=*paint.rows.last().unwrap();
        if last-first+1!=paint.rows.len() {return Err(bad("source list paint skips an intervening row"));}
        // Only observed adjacent baselines in this common basis establish a
        // row band. Raw Td/TL may be an initial jump or use another scale.
        let step=rows.windows(2).enumerate()
            .filter(|(position,_)|*position>=first.saturating_sub(1)&&*position<=last)
            .map(|(_,pair)|pair[0].origin[1]-pair[1].origin[1])
            .filter(|step|step.is_finite()&&*step>0.)
            .min_by(f64::total_cmp)
            .ok_or_else(||bad("source list row height cannot be proved"))?;
        if paint.rectangle[3]-paint.rectangle[1]>(rows[first].origin[1]-rows[last].origin[1]+step)*1.25 {
            return Err(bad("source list paint exceeds its row band"));
        }
    }
    for row in &rows {
        if row.origin[1]>clip[1]&&row.origin[1]<clip[3]&&model.selected.contains(&row.index)&&!painted.contains(&row.index) {
            return Err(bad("source list selected row is missing its paint"));
        }
    }
    Ok(Layout{operations,marked,text,rows,paints,clip,basis,fonts,characters})
}

pub(crate) fn prepare_list_source(
    pdfium:&Pdfium,source:&Document,original:&Stream,resources:&Dictionary,
    widget:ObjectId,captured:&CapturedListSources,budget:&mut FillAppearanceBudget<'_>,
)->EngineResult<Option<Stream>> {
    let bytes=original.get_plain_content_with_limit(MAX_BYTES).map_err(|_|bad("source list exceeds its decoding bound"))?;
    let bounds=model::bounds(source,original)?;
    if AnswerSlot::parse_bounds(&bytes,AnswerKind::List,Some(bounds)).is_ok(){return Ok(None);}
    let (model,top)=captured.get(source,widget,original,&bytes)?;
    if original.dict.has(b"Group")||original.dict.has(b"OC") {
        return Err(bad("source list has unsupported compositing or visibility context"));
    }
    let layout=analyse(source,resources,&bytes,bounds,model,top)?;
    ink::verify(pdfium,source,resources,bounds,&layout)?;
    // A native-proved single rectangle contributes exactly one path and fill.
    // Preserve all colors and balanced state; omit only those old paints before
    // reusing the established compound frame/text normalizer.
    let removed:HashSet<_>=layout.paints.iter().flat_map(|paint|[paint.path,paint.paint]).collect();
    budget.reserve_bytes(removed.len().saturating_mul(64))?;
    let mut operations=Vec::new();
    for(index,op)in layout.operations.into_iter().enumerate(){if !removed.contains(&index){operations.push(op);}}
    let temporary=encode_bounded(&operations)?;
    let slot=AnswerSlot::parse_bounds(&temporary,AnswerKind::List,Some(bounds))?;
    let canonical=encode_bounded(&slot.content.operations)?;
    budget.reserve_dictionary(&original.dict)?;
    budget.reserve_bytes(canonical.len())?;
    let mut dictionary=original.dict.clone();
    dictionary.remove(b"Filter");dictionary.remove(b"DecodeParms");
    Ok(Some(Stream::new(dictionary,canonical)))
}
