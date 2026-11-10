//! Read-only geometric suggestions. Final admission belongs to paragraph_baseline.
use super::*;

fn marker_token(token: &str) -> bool {
    if matches!(token, "-" | "•" | "‣" | "◦") {
        return true;
    }
    let body = if let Some(value) = token.strip_suffix('.') {
        value
    } else if let Some(value) = token.strip_suffix(')') {
        value.trim_start_matches('(')
    } else {
        return false;
    };
    !body.is_empty()
        && body.len() <= 6
        && (body.bytes().all(|c| c.is_ascii_digit())
            || (body.len() == 1 && body.bytes().all(|c| c.is_ascii_alphabetic()))
            || body
                .bytes()
                .all(|c| matches!(c, b'I' | b'V' | b'X' | b'L' | b'C')))
}
fn bare_marker(text: &str) -> bool {
    let text = text.trim();
    !text.contains(char::is_whitespace) && marker_token(text)
}
fn starts_list(text: &str) -> bool {
    text.split_whitespace().next().is_some_and(marker_token)
}
fn horizontal(run: &RunSnapshot,unit:f32) -> bool {
    run.matrix
        .iter()
        .chain(run.bounds.iter())
        .all(|v| v.is_finite())
        && run.matrix[1].abs() <= 0.0001
        && run.matrix[2].abs() <= 0.0001
        && run.matrix[0] > 0.0
        && (run.matrix[0] - run.matrix[3]).abs() <= 0.0001
        && physical_units::paragraph_size(run.font_size * run.matrix[0],unit)
}
struct Row<'a> {
    runs: Vec<&'a RunSnapshot>,
    bounds: [f32;2],
}
impl Row<'_> {
    fn first(&self) -> &RunSnapshot {
        self.runs[0]
    }
    fn last(&self) -> &RunSnapshot {
        self.runs[self.runs.len() - 1]
    }
    fn size(&self,mixed:bool) -> f32 {
        if mixed {self.runs.iter().map(|r|r.font_size*r.matrix[0]).fold(0.,f32::max)}
        else {self.first().font_size * self.first().matrix[0]}
    }
    fn left(&self) -> f32 {
        self.bounds[0]
    }
    fn center(&self) -> f32 {
        (self.bounds[0]+self.bounds[1])*0.5
    }
    fn marker(&self) -> bool {
        self.runs.len() == 1 && bare_marker(&self.first().text)
    }
}

pub(super) fn suggest_mode(before:&[RunSnapshot],anchor:usize,expected:&str,extents:&HashMap<usize,[f32;2]>,mixed:bool,unit:f32)->EngineResult<Vec<usize>>{
    let anchor_run = before
        .iter()
        .find(|run| run.index == anchor)
        .ok_or_else(|| invalid("The selected text piece is missing."))?;
    if anchor_run.text != expected || expected.len() > 4096 {
        return Err(invalid(
            "The text changed since it was selected. Select it again.",
        ));
    }
    if !horizontal(anchor_run,unit) || anchor_run.text.trim().is_empty() || bare_marker(&anchor_run.text)
    {
        return Err(invalid(
            "Select the paragraph text, or choose its pieces manually.",
        ));
    }
    let mut rows: Vec<Row<'_>> = Vec::new();
    for run in before.iter().filter(|run| !run.text.trim().is_empty()) {
        let bounds=extents.get(&run.index).copied().unwrap_or([run.matrix[4],run.bounds[2]]);
        let join = rows.last().is_some_and(|row| {
            let last = row.last();
            // Horizontal adjacency is local: earlier large emphasis must not
            // bridge a later column gap between two smaller pieces.
            let size = (if mixed {last.font_size*last.matrix[0]}else{row.size(false)}).max(run.font_size * run.matrix[0]);
            horizontal(last,unit)
                && horizontal(run,unit)
                && !row.marker()
                && !bare_marker(&run.text)
                && (last.matrix[5] - run.matrix[5]).abs() <= EPSILON
                && (!mixed || {
                    let low=row.runs.iter().map(|r|r.matrix[5]).fold(run.matrix[5],f32::min);
                    let high=row.runs.iter().map(|r|r.matrix[5]).fold(run.matrix[5],f32::max);
                    high-low<=EPSILON
                })
                && run.matrix[4] >= last.matrix[4]
                && run.bounds[0] - last.bounds[2] >= -size * 0.25
                && run.bounds[0] - last.bounds[2] <= size
        });
        if join {
            let row=rows.last_mut().unwrap();row.runs.push(run);row.bounds[0]=row.bounds[0].min(bounds[0]);row.bounds[1]=row.bounds[1].max(bounds[1]);
        } else {
            rows.push(Row { runs: vec![run],bounds });
        }
    }
    let mut blocks: Vec<Vec<usize>> = Vec::new();
    for index in 0..rows.len() {
        let row = &rows[index];
        let join = blocks.last().is_some_and(|block| {
            let previous = &rows[*block.last().unwrap()];
            let size = previous.size(mixed).max(if mixed{row.size(true)}else{0.});
            // Require one body size across the whole proposed block, rather
            // than allowing a chain of pairwise matches to cross a heading.
            let shared_size=if mixed {rows[block[0]].runs.iter().map(|r|r.font_size*r.matrix[0]).find(|size|
                block.iter().copied().chain(std::iter::once(index)).all(|i|rows[i].runs.iter().any(|r|(r.font_size*r.matrix[0]-size).abs()<=EPSILON)))}else{None};
            let margin_size=shared_size.unwrap_or(size);
            let leading = previous.first().matrix[5] - row.first().matrix[5];
            let margin = if block.len() > 1 {
                rows[block[1]].left()
            } else {
                previous.left()
            };
            let aligned = (margin - row.left()).abs() <= margin_size * 0.5;
            let centered = (rows[block[0]].center() - row.center()).abs() <= margin_size * 0.5;
            let right_aligned = (rows[block[0]].bounds[1] - row.bounds[1]).abs() <= margin_size * 0.5;
            let first_indent = block.len() == 1
                && previous.left() - row.left() > 0.0
                && previous.left() - row.left() <= margin_size * 2.0;
            horizontal(previous.first(),unit)
                && horizontal(row.first(),unit)
                && !previous.marker()
                && !row.marker()
                && !starts_list(&row.first().text)
                && if mixed {shared_size.is_some()}else{(size-row.size(false)).abs()<=EPSILON}
                && leading + EPSILON >= size * 0.6
                && leading <= size * 1.65
                && (aligned || first_indent || centered || right_aligned)
        });
        if join {
            blocks.last_mut().unwrap().push(index);
        } else {
            blocks.push(vec![index]);
        }
    }
    let block = blocks
        .iter()
        .find(|block| {
            block
                .iter()
                .any(|&i| rows[i].runs.iter().any(|run| run.index == anchor))
        })
        .ok_or_else(|| {
            invalid("The paragraph could not be identified. Select its pieces manually.")
        })?;
    let indices: Vec<_> = block
        .iter()
        .flat_map(|&i| rows[i].runs.iter().map(|run| run.index))
        .collect();
    if indices.len() > 64 {
        return Err(invalid(
            "The suggested paragraph exceeds 64 pieces. Select a smaller paragraph manually.",
        ));
    }
    // Baseline/size changes can split one visual line (for example x² value)
    // into separate blocks. Do not validate a truncated ordinary fragment while
    // silently leaving its superscript or shifted neighbor outside the selection.
    for selected in before.iter().filter(|run| indices.contains(&run.index)) {
        let size = selected.font_size * selected.matrix[0];
        for other in before
            .iter()
            .filter(|run| !indices.contains(&run.index) && !run.text.trim().is_empty())
        {
            if !other
                .bounds
                .iter()
                .chain(other.matrix.iter())
                .all(|v| v.is_finite())
            {
                return Err(invalid("Nearby text geometry could not be verified. Select the paragraph pieces manually."));
            }
            let dy = (selected.matrix[5] - other.matrix[5]).abs();
            // A genuine list label to the left on the same baseline is a boundary.
            if bare_marker(&other.text) && dy <= EPSILON && other.bounds[2] <= selected.bounds[0] {
                continue;
            }
            let gap = (selected.bounds[0] - other.bounds[2])
                .max(other.bounds[0] - selected.bounds[2])
                .max(0.0);
            if gap <= size
                && dy <= size * 0.75
                && other.bounds[3] > selected.matrix[5] - size * 0.25
                && other.bounds[1] < selected.matrix[5] + size * 1.1
            {
                return Err(invalid("Nearby text uses a different baseline or layout. Select the paragraph pieces manually."));
            }
        }
    }
    Ok(indices)
}
