//! Bounded, disposable hints for exact paragraph re-editing. Text stays in the
//! actual PDF operators; these records contain only geometry, hashes and breaks.
use super::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const KEY: &[u8] = b"FolioParagraphs";
const MAX_BYTES: usize = 512 * 1024;
const MAX_RECORDS: usize = 64;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    digest: String,
    font: String,
    program: Option<String>,
    size: f32,
    matrix: [f32; 6],
    color: [u8; 4],
    newlines: usize,
    glyphs: Vec<[f32; 6]>,
}
impl Row {
    fn matches(&self, run: &RunSnapshot) -> bool {
        self.font == run.font && self.program == run.program && self.color == run.color
            && self.size.is_finite() && (self.size - run.font_size).abs() <= EPSILON
            && self.matrix.iter().all(|n| n.is_finite())
            && close_values(&self.matrix, &run.matrix)
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    bounds: [f32; 4],
    size: f32,
    alignment: ParagraphAlignment,
    spacing: f32,
    rows: Vec<Row>,
    trailing: usize,
    #[serde(default,skip_serializing_if="Option::is_none")]
    context:Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Records { version: u32, paragraphs: Vec<Record> }

fn read(pdf: &Document, page: lopdf::ObjectId) -> Vec<Record> {
    let Ok(unit)=physical_units::page_unit(pdf,page)else{return vec![];};
    let Some(bytes) = pdf.get_dictionary(page).ok().and_then(|d| d.get(KEY).ok()).and_then(|v| v.as_str().ok()) else { return vec![]; };
    if bytes.len() > MAX_BYTES { return vec![]; }
    let Ok(records) = serde_json::from_slice::<Records>(bytes) else { return vec![]; };
    if !matches!(records.version,1|2) || (records.version==1 && records.paragraphs.iter().any(|r|r.context.is_some())) || records.paragraphs.len() > MAX_RECORDS { return vec![]; }
    records.paragraphs.into_iter().filter(|r| {
        r.context.as_ref().is_none_or(|s|s.len()==64&&s.bytes().all(|b|b.is_ascii_hexdigit()))
            && !r.rows.is_empty() && r.rows.len() <= 64 && r.trailing <= 256
            && r.rows.iter().all(|row| row.newlines <= 256 && row.digest.len() == 64 && row.font.len() <= 512)
            && r.rows.iter().map(|row|row.glyphs.len()).sum::<usize>() <= 4096
            && r.rows.iter().flat_map(|row|&row.glyphs).flatten().all(|v|v.is_finite() && v.abs()<=1_000_000.)
            && r.rows.iter().map(|row| row.newlines).sum::<usize>() + r.trailing <= 256
            && physical_units::paragraph_size(r.size,unit)
            && r.spacing.is_finite() && (0.6..=3.0).contains(&r.spacing)
            && r.bounds.iter().all(|n| n.is_finite() && n.abs() <= 1_000_000.)
            && r.bounds[0] < r.bounds[2] && r.bounds[1] < r.bounds[3]
    }).collect()
}

// Canonicalize only the admitted show operator, ignoring lexical string format.
// No recursive object walk or PDF object IDs enter a row's binding.
fn show_values(op: &Operation) -> Option<Vec<&Object>> {
    match (op.operator.as_str(), op.operands.as_slice()) {
        ("Tj", [value @ Object::String(..)]) => Some(vec![value]),
        ("TJ", [Object::Array(values)]) => Some(values.iter().collect()),
        _ => None,
    }
}
fn digest(op: &Operation) -> Option<String> {
    let mut hash = Sha256::new();
    hash.update(op.operator.as_bytes());
    for value in show_values(op)? {
        if let Ok(bytes) = value.as_str() {
            hash.update([0]); hash.update((bytes.len() as u64).to_le_bytes()); hash.update(bytes);
        } else {
            let n = value.as_float().ok()?;
            if !n.is_finite() { return None; }
            hash.update([1]); hash.update(n.to_le_bytes());
        }
    }
    Some(hash.finalize().iter().map(|byte|format!("{byte:02x}")).collect())
}
fn same_show(a: &Operation, b: &Operation, size: f32) -> bool {
    let (Some(a), Some(b)) = (show_values(a), show_values(b)) else { return false; };
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| {
        match (a.as_str(), b.as_str()) {
            (Ok(a), Ok(b)) => a == b,
            _ => match (a.as_float(), b.as_float()) {
                (Ok(a), Ok(b)) => a.is_finite() && b.is_finite() && (a-b).abs()*size/1000. <= EPSILON,
                _ => false,
            },
        }
    })
}

fn indices(record: &Record, before: &[RunSnapshot]) -> Option<Vec<usize>> {
    let mut result = Vec::new();
    for row in &record.rows {
        let mut matches = before.iter().filter(|run| row.matches(run));
        let run = matches.next()?;
        if matches.next().is_some() { return None; }
        result.push(run.index);
    }
    result.windows(2).all(|pair| pair[0] < pair[1]).then_some(result)
}
pub(super) fn candidates(bytes: &[u8], before: &[RunSnapshot], anchor: usize) -> Vec<Vec<usize>> {
    let Ok(pdf) = Document::load_mem_with_options(bytes, lopdf::LoadOptions::with_max_decompressed_size(LIMIT)) else { return vec![]; };
    let Some(page) = pdf.get_pages().values().next().copied() else { return vec![]; };
    let candidates: Vec<_> = read(&pdf, page).iter().filter_map(|r| indices(r, before)).filter(|v| v.contains(&anchor)).collect();
    if candidates.len()==1 { candidates } else { vec![] }
}

fn restore_record(record: &Record, p: &Prepared, document: &PdfDocument<'_>, page: &PdfPage<'_>,capture:bool) -> Option<(TextParagraphSelection,Vec<paragraph_spans::SourceRange>)> {
    if indices(record, &p.before)? != p.selection.object_indices || record.context!=p.context_binding().ok()? { return None; }
    p.check_context_area(record.bounds).ok()?;
    let g = p.geometry;
    if record.bounds[0] < g.left || record.bounds[1] < g.bottom || record.bounds[2] > g.right || record.bounds[3] > g.top { return None; }
    let mut text = String::new();
    let mut ranges=Vec::new();
    for ((row, index), (op_index, _)) in record.rows.iter().zip(&p.selection.object_indices).zip(&p.removals) {
        if digest(&p.content.operations[*op_index])?.as_str() != row.digest { return None; }
        let glyphs = p.glyphs.get(index)?;
        if glyphs.len()!=row.glyphs.len() || !glyphs.iter().zip(&row.glyphs).all(|(a,b)|close_values(&a.position,b)) { return None; }
        let source = p.source_text.get(index)?;
        if text.len() + row.newlines + source.len() > 4096 { return None; }
        text.extend(std::iter::repeat_n('\n', row.newlines));
        if capture{ranges.push(paragraph_spans::SourceRange{object_index:*index,source_start:0,source_end:source.len(),logical_start:text.len()});}
        text.push_str(source);
    }
    if text.len() + record.trailing > 4096 { return None; }
    text.extend(std::iter::repeat_n('\n', record.trailing));
    let first = p.before.iter().find(|r| r.index == p.selection.object_indices[0])?;
    if (first.font_size - record.size).abs() > EPSILON || !close_values(&first.matrix[..4], &[1., 0., 0., 1.]) { return None; }
    let metrics = ink_metrics(document, page, first, &text, record.size).ok()?;
    let lines = wrap(first, false, &text, record.bounds[2]-record.bounds[0], &metrics).ok()?;
    if record.size*1.25 + (lines.len()-1) as f32*record.size*record.spacing > record.bounds[3]-record.bounds[1]+EPSILON { return None; }
    let visible: Vec<_> = lines.iter().enumerate().filter(|(_, line)| !line.text.is_empty()).collect();
    if visible.len() != record.rows.len() { return None; }
    for (((line_index, line), index), (op_index, _)) in visible.into_iter().zip(&p.selection.object_indices).zip(&p.removals) {
        if &line.text != p.source_text.get(index)? { return None; }
        let run = p.before.iter().find(|r| r.index == *index)?;
        let (origin, op) = paragraph_layout::positioned_line(first, false, line, line_index, record.bounds, record.size, record.alignment, record.spacing, &metrics).ok()?;
        if !close_values(&run.matrix[4..], &origin) || !same_show(&op, &p.content.operations[*op_index], record.size) { return None; }
    }
    let mut selection = p.selection.clone();
    selection.text = text; selection.font_size = record.size;
    selection.bounds = displayed_bounds(g, record.bounds);
    selection.alignment = Some(record.alignment); selection.line_spacing = Some(record.spacing);
    Some((selection,ranges))
}
pub(super) fn restore(p: &Prepared, document: &PdfDocument<'_>, page: &PdfPage<'_>) -> Option<TextParagraphSelection> {
    restore_with_mapping(p,document,page,false).map(|(selection,_)|selection)
}
pub(super) fn restore_with_mapping(p: &Prepared, document: &PdfDocument<'_>, page: &PdfPage<'_>,capture:bool) -> Option<(TextParagraphSelection,Vec<paragraph_spans::SourceRange>)> {
    // Glyph positions in a record are not independent proof. Verify the actual
    // content's effective text state agrees with what our layout emits, even if
    // an external writer refreshed every stored coordinate after changing Tc/Tw.
    if !p.reproducible_text_state { return None; }
    let records = read(&p.pdf, p.page_id);
    let overlapping = records.iter().filter_map(|r|indices(r,&p.before))
        .filter(|ids|ids.iter().any(|id|p.selection.object_indices.contains(id))).count();
    if overlapping!=1 { return None; }
    let mut matches = records.iter().filter_map(|r| restore_record(r, p, document, page,capture));
    let selected = matches.next()?;
    matches.next().is_none().then_some(selected)
}

pub(super) fn write(bytes: &[u8], after: &[RunSnapshot], glyphs: &HashMap<usize,Vec<Glyph>>, lines: &[paragraph_layout::Line], bounds: [f32; 4], size: f32, alignment: ParagraphAlignment, spacing: f32, context:Option<String>) -> EngineResult<Vec<u8>> {
    let count = lines.iter().filter(|line| !line.text.is_empty()).count();
    if count == 0 || count > 64 { return Ok(bytes.to_vec()); }
    let mut pdf = Document::load_mem_with_options(bytes, lopdf::LoadOptions::with_max_decompressed_size(LIMIT)).map_err(|_| invalid("The paragraph settings could not be saved."))?;
    let page = *pdf.get_pages().values().next().ok_or_else(|| invalid("Missing paragraph page."))?;
    let content = Content::decode(&pdf.get_page_content_with_limit(page, LIMIT).map_err(|_| invalid("Invalid paragraph content."))?).map_err(|_| invalid("Invalid paragraph content."))?;
    let show: Vec<_> = content.operations.iter().filter(|op| matches!(op.operator.as_str(), "Tj"|"TJ"|"'"|"\"") && !positioned_text::cursor_only(op)).collect();
    if show.len() != after.len() || count > after.len() { return Err(invalid("The paragraph settings could not be matched to the saved text.")); }
    let mut records: Vec<_> = read(&pdf, page).into_iter().filter(|r| {
        indices(r, after).is_some_and(|ids| ids.iter().all(|id| !after[after.len()-count..].iter().any(|run|run.index==*id)) && r.rows.iter().zip(ids).all(|(row, index)| {
            after.iter().position(|run| run.index == index).is_some_and(|ordinal| digest(show[ordinal]).as_deref() == Some(row.digest.as_str()))
        }))
    }).collect();
    let mut rows = Vec::new(); let mut newlines = 0;
    for line in lines {
        if !line.text.is_empty() {
            let ordinal = after.len()-count+rows.len(); let run = &after[ordinal];
            rows.push(Row { digest: digest(show[ordinal]).ok_or_else(|| invalid("Invalid paragraph text operation."))?, font:run.font.clone(), program:run.program.clone(), size:run.font_size, matrix:run.matrix, color:run.color, newlines, glyphs:glyphs[&run.index].iter().map(|g|g.position).collect() });
            newlines = 0;
        }
        if !line.soft_wrapped { newlines += 1; }
    }
    records.push(Record {bounds, size, alignment, spacing, rows, trailing: newlines.saturating_sub(1),context});
    while records.len() > MAX_RECORDS { records.remove(0); }
    let data = loop {
        let data = serde_json::to_vec(&Records {version:if records.iter().any(|r|r.context.is_some()){2}else{1}, paragraphs:records.clone()}).map_err(|_| invalid("The paragraph settings could not be encoded."))?;
        if data.len() <= MAX_BYTES { break data; }
        records.remove(0);
    };
    pdf.get_dictionary_mut(page).map_err(|_| invalid("Missing paragraph page."))?.set(KEY, Object::String(data, StringFormat::Hexadecimal));
    let mut output = Vec::new(); pdf.save_to(&mut output).map_err(|_| invalid("The paragraph settings could not be saved."))?;
    if output.len() as u64 > MAX_SOURCE_BYTES { return Err(invalid("The edited page exceeds the source size limit.")); }
    Ok(output)
}
