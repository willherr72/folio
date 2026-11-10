//! Source-bound character geometry and partial text removal; private groundwork.
use crate::{
    redaction_content::{invalid, number, require},
    redaction_core::{self, Prepared, TextBlock},
    EngineResult,
};
use lopdf::{content::Operation, Document, Object};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug)]
pub(crate) struct Glyph {
    pub offset: usize,
    pub code: u16,
    pub unicode: u32,
    pub origin: [f64; 2],
    pub end: [f64; 2],
}
pub(crate) struct GlyphBlock {
    pub source: TextBlock,
    pub glyphs: Vec<Glyph>,
}
pub(crate) struct TextRange {
    pub selection_id: String,
    pub start: usize,
    pub end: usize,
}
pub(crate) struct RedactedTextRangesPdf {
    pub bytes: Vec<u8>,
    pub removed_characters: usize,
    pub affected_blocks: usize,
    pub omitted_metadata: Vec<&'static str>,
}

type Intervals = Vec<(usize, usize)>;
type Selections = BTreeMap<(usize, usize), Intervals>;
#[derive(Clone, Copy)]
struct Matrix([f64; 6]);
impl Matrix {
    const ID: Self = Self([1., 0., 0., 1., 0., 0.]);
    fn point(self, p: [f64; 2]) -> EngineResult<[f64; 2]> {
        let [a, b, c, d, e, f] = self.0;
        let out = [a * p[0] + c * p[1] + e, b * p[0] + d * p[1] + f];
        bounded(&out)?;
        Ok(out)
    }
    fn compose(self, local: Self) -> EngineResult<Self> {
        let [a, b, c, d, e, f] = self.0;
        let [g, h, i, j, k, l] = local.0;
        let out = [
            a * g + c * h,
            b * g + d * h,
            a * i + c * j,
            b * i + d * j,
            a * k + c * l + e,
            b * k + d * l + f,
        ];
        bounded(&out)?;
        Ok(Self(out))
    }
    fn translate(self, x: f64, y: f64) -> EngineResult<Self> {
        self.compose(Self([1., 0., 0., 1., x, y]))
    }
    fn operands(a: &[Object]) -> EngineResult<Self> {
        let mut out = [0.; 6];
        for (i, o) in a.iter().enumerate() {
            out[i] = number(o)?;
        }
        Ok(Self(out))
    }
}
fn bounded(values: &[f64]) -> EngineResult<()> {
    require(
        values
            .iter()
            .all(|n| n.is_finite() && n.abs() <= 1_000_000.),
    )
}
#[derive(Clone, Copy)]
struct State {
    ctm: Matrix,
    tm: Matrix,
    line: Matrix,
    font: Option<usize>,
    size: f64,
    tc: f64,
    tw: f64,
    scale: f64,
    leading: f64,
    rise: f64,
}
impl Default for State {
    fn default() -> Self {
        Self {
            ctm: Matrix::ID,
            tm: Matrix::ID,
            line: Matrix::ID,
            font: None,
            size: 0.,
            tc: 0.,
            tw: 0.,
            scale: 1.,
            leading: 0.,
            rise: 0.,
        }
    }
}
impl State {
    fn move_line(&mut self, x: f64, y: f64) -> EngineResult<()> {
        self.line = self.line.translate(x, y)?;
        self.tm = self.line;
        Ok(())
    }
    fn origin(&self, advance: f64) -> EngineResult<[f64; 2]> {
        self.ctm.point(self.tm.point([advance, self.rise])?)
    }
}
fn selected(ranges: &[(usize, usize)], offset: usize) -> bool {
    let i = ranges.partition_point(|(_, end)| *end <= offset);
    ranges.get(i).is_some_and(|(start, _)| *start <= offset)
}
fn intersects(ranges: &[(usize, usize)], start: usize, end: usize) -> bool {
    let i = ranges.partition_point(|(_, b)| *b <= start);
    ranges.get(i).is_some_and(|(a, _)| *a < end)
}

#[derive(Default)]
struct OutputBudget {
    operations: usize,
    tokens: usize,
}
impl OutputBudget {
    fn tokens(&mut self, n: usize) -> EngineResult<()> {
        self.tokens = self.tokens.checked_add(n).ok_or_else(invalid)?;
        require(self.tokens <= 500_000)
    }
    fn operation(&mut self) -> EngineResult<()> {
        self.operations += 1;
        require(self.operations <= 100_000)
    }
    fn unchanged(&mut self, op: &Operation, output: &mut Vec<Operation>) -> EngineResult<()> {
        self.operation()?;
        let tokens = 1 + op
            .operands
            .iter()
            .map(|o| match o {
                Object::Array(a) => 2 + a.len(),
                _ => 1,
            })
            .sum::<usize>();
        self.tokens(tokens)?;
        output.push(op.clone());
        Ok(())
    }
    fn flush(&mut self, parts: &mut Vec<Object>, output: &mut Vec<Operation>) -> EngineResult<()> {
        if !parts.is_empty() {
            self.operation()?;
            self.tokens(3)?;
            output.push(Operation::new(
                "TJ",
                vec![Object::Array(std::mem::take(parts))],
            ));
        }
        Ok(())
    }
    fn piece(
        &mut self,
        piece: Object,
        parts: &mut Vec<Object>,
        output: &mut Vec<Operation>,
    ) -> EngineResult<()> {
        self.tokens(1)?;
        parts.push(piece);
        if parts.len() == 4096 {
            self.flush(parts, output)?;
        }
        Ok(())
    }
    fn kept(
        &mut self,
        bytes: Vec<u8>,
        parts: &mut Vec<Object>,
        output: &mut Vec<Operation>,
        separate: bool,
    ) -> EngineResult<()> {
        if separate {
            self.flush(parts, output)?;
        }
        self.piece(Object::string_literal(bytes), parts, output)?;
        if separate {
            self.flush(parts, output)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    #[test]
    fn redaction_ranges_generated_output_counts_envelopes_and_operations() {
        let mut budget = OutputBudget {
            operations: 0,
            tokens: 499_999,
        };
        let mut parts = vec![];
        let mut output = vec![];
        budget
            .piece(Object::Integer(-667), &mut parts, &mut output)
            .unwrap();
        assert!(budget.flush(&mut parts, &mut output).is_err());
        assert!(output.is_empty());
        let mut budget = OutputBudget {
            operations: 100_000,
            tokens: 0,
        };
        assert!(budget
            .unchanged(&Operation::new("q", vec![]), &mut output)
            .is_err());
        assert!(output.is_empty());
    }
}
struct Scan {
    blocks: Vec<GlyphBlock>,
    operations: Vec<Vec<Operation>>,
}
fn scan(prepared: &Prepared, selections: Option<&Selections>) -> EngineResult<Scan> {
    let mut result = Scan {
        blocks: vec![],
        operations: vec![],
    };
    let mut budget = OutputBudget::default();
    for (page_index, page) in prepared.pages.iter().enumerate() {
        let mut state = State::default();
        let mut stack = vec![];
        let mut active: Option<GlyphBlock> = None;
        let mut ordinal = 0;
        let mut output = vec![];
        for op in &page.program.operations {
            let a = &op.operands;
            let mut rewritten = false;
            match op.operator.as_str() {
                "BT" => {
                    state.tm = Matrix::ID;
                    state.line = Matrix::ID;
                    active = Some(GlyphBlock {
                        source: TextBlock {
                            page_index,
                            block_index: ordinal,
                            text: String::new(),
                            selection_id: redaction_core::selection_id(
                                prepared, page_index, ordinal,
                            ),
                        },
                        glyphs: vec![],
                    });
                    ordinal += 1;
                }
                "ET" => {
                    let block = active.take().ok_or_else(invalid)?;
                    if !block.glyphs.is_empty() {
                        result.blocks.push(block);
                    }
                }
                "q" => stack.push(state),
                "Q" => state = stack.pop().ok_or_else(invalid)?,
                "cm" => state.ctm = state.ctm.compose(Matrix::operands(a)?)?,
                "Tf" => {
                    let name = a[0].as_name().map_err(|_| invalid())?;
                    let name = std::str::from_utf8(name).map_err(|_| invalid())?;
                    let index = name
                        .strip_prefix('F')
                        .ok_or_else(invalid)?
                        .parse::<usize>()
                        .map_err(|_| invalid())?;
                    require(index < page.fonts.len())?;
                    state.font = Some(index);
                    state.size = number(&a[1])?;
                }
                "Tc" => state.tc = number(&a[0])?,
                "Tw" => state.tw = number(&a[0])?,
                "Tz" => state.scale = number(&a[0])? / 100.,
                "TL" => state.leading = number(&a[0])?,
                "Ts" => state.rise = number(&a[0])?,
                "Tm" => {
                    state.tm = Matrix::operands(a)?;
                    state.line = state.tm;
                }
                "Td" | "TD" => {
                    let x = number(&a[0])?;
                    let y = number(&a[1])?;
                    if op.operator == "TD" {
                        state.leading = -y;
                    }
                    state.move_line(x, y)?;
                }
                "T*" => state.move_line(0., -state.leading)?,
                "Tj" | "TJ" => {
                    let block = active.as_mut().ok_or_else(invalid)?;
                    let ranges = selections
                        .and_then(|s| s.get(&(page_index, block.source.block_index)))
                        .map(Vec::as_slice)
                        .unwrap_or(&[]);
                    let items = if op.operator == "Tj" {
                        a.as_slice()
                    } else {
                        a[0].as_array().map_err(|_| invalid())?.as_slice()
                    };
                    let font = &page.fonts[state.font.ok_or_else(invalid)?];
                    let length: usize = items
                        .iter()
                        .map(|o| match o {
                            Object::String(s, _) => s.len() / font.code_len(),
                            _ => 0,
                        })
                        .sum();
                    rewritten = intersects(ranges, block.glyphs.len(), block.glyphs.len() + length);
                    // PDFium aggregates numerical entries in a mixed TJ show.
                    // For nominal unspaced Tj text, numerical-only shows retain
                    // the original per-glyph float advances. Active spacing or
                    // horizontal scaling uses the general mixed compensation.
                    let nominal = op.operator == "Tj"
                        && state.tc == 0.0
                        && state.tw == 0.0
                        && state.scale == 1.0;
                    let preserve_one_show = if nominal && rewritten {
                        let bytes = a[0].as_str().map_err(|_| invalid())?;
                        let code_len = font.code_len();
                        crate::redaction_text_advances::nominal_mixed_preserves(
                            state.size as f32,
                            bytes.len() / code_len,
                            |index| {
                                let code = crate::redaction_fonts::code(
                                    &bytes[index * code_len..(index + 1) * code_len],
                                );
                                Ok((
                                    font.glyph(code)?.1,
                                    selected(ranges, block.glyphs.len() + index),
                                ))
                            },
                        )?
                    } else {
                        false
                    };
                    let separate = nominal && !preserve_one_show;
                    let mut parts = vec![];
                    for item in items {
                        if let Object::String(bytes, _) = item {
                            let mut kept = vec![];
                            for chunk in bytes.chunks(font.code_len()) {
                                let code = crate::redaction_fonts::code(chunk);
                                let offset = block.glyphs.len();
                                let (character, width) = font.glyph(code)?;
                                let width = width as f64;
                                let spacing = state.tc
                                    + if font.code_len() == 1 && code == 32 {
                                        state.tw
                                    } else {
                                        0.
                                    };
                                let advance = (width / 1000. * state.size + spacing) * state.scale;
                                bounded(&[advance])?;
                                let glyph = Glyph {
                                    offset,
                                    code,
                                    unicode: character as u32,
                                    origin: state.origin(0.)?,
                                    end: state.origin(advance)?,
                                };
                                block.source.text.push(font.source_character(code)?);
                                block.glyphs.push(glyph);
                                state.tm = state.tm.translate(advance, 0.)?;
                                if rewritten {
                                    if selected(ranges, offset) {
                                        if !kept.is_empty() {
                                            budget.kept(
                                                std::mem::take(&mut kept),
                                                &mut parts,
                                                &mut output,
                                                separate,
                                            )?;
                                        }
                                        let compensation = -(width + 1000. * spacing / state.size);
                                        bounded(&[compensation])?;
                                        let number = if compensation.fract() == 0. {
                                            Object::Integer(compensation as i64)
                                        } else {
                                            Object::Real(compensation as f32)
                                        };
                                        budget.piece(number, &mut parts, &mut output)?;
                                    } else {
                                        kept.extend_from_slice(chunk);
                                    }
                                }
                            }
                            if rewritten && !kept.is_empty() {
                                budget.kept(kept, &mut parts, &mut output, separate)?;
                            }
                        } else {
                            let advance = -number(item)? / 1000. * state.size * state.scale;
                            bounded(&[advance])?;
                            state.tm = state.tm.translate(advance, 0.)?;
                            if rewritten {
                                budget.piece(item.clone(), &mut parts, &mut output)?;
                            }
                        }
                    }
                    if rewritten {
                        budget.flush(&mut parts, &mut output)?;
                    }
                }
                _ => {}
            }
            if selections.is_some() && !rewritten {
                budget.unchanged(op, &mut output)?;
            }
        }
        require(active.is_none() && stack.is_empty())?;
        result.operations.push(output);
    }
    Ok(result)
}
pub(crate) fn inspect_standard_glyphs(doc: &Document) -> EngineResult<Vec<GlyphBlock>> {
    Ok(scan(&redaction_core::prepare(doc)?, None)?.blocks)
}
pub(crate) fn inspect_area_glyphs(doc: &Document) -> EngineResult<Vec<GlyphBlock>> {
    inspect_area_glyphs_with_context(doc, &mut Default::default())
}
pub(crate) fn inspect_area_glyphs_with_context(
    doc: &Document,
    images: &mut crate::redaction_images::ImageContext,
) -> EngineResult<Vec<GlyphBlock>> {
    Ok(scan(
        &redaction_core::prepare_area_with_context(doc, images)?,
        None,
    )?
    .blocks)
}

pub(crate) fn remove_standard_text_ranges(
    doc: &Document,
    ranges: &[TextRange],
) -> EngineResult<RedactedTextRangesPdf> {
    remove_ranges(doc, ranges, None)
}
pub(crate) fn remove_area_text_ranges(
    doc: &Document,
    ranges: &[TextRange],
) -> EngineResult<RedactedTextRangesPdf> {
    remove_area_text_ranges_with_context(doc, ranges, &mut Default::default())
}
pub(crate) fn remove_area_text_ranges_with_context(
    doc: &Document,
    ranges: &[TextRange],
    images: &mut crate::redaction_images::ImageContext,
) -> EngineResult<RedactedTextRangesPdf> {
    remove_ranges(doc, ranges, Some(images))
}
fn remove_ranges(
    doc: &Document,
    ranges: &[TextRange],
    mut images: Option<&mut crate::redaction_images::ImageContext>,
) -> EngineResult<RedactedTextRangesPdf> {
    require(!ranges.is_empty() && ranges.len() <= 4096)?;
    let mut prepared = if let Some(images) = images.as_deref_mut() {
        redaction_core::prepare_area_with_context(doc, images)?
    } else {
        redaction_core::prepare(doc)?
    };
    let mut lookup = HashMap::new();
    for (page_index, page) in prepared.pages.iter().enumerate() {
        for block in &page.program.blocks {
            lookup.insert(
                redaction_core::selection_id(&prepared, page_index, block.index),
                (page_index, block.index, block.text.chars().count()),
            );
        }
    }
    let mut selections = Selections::new();
    let mut removed_characters = 0usize;
    for r in ranges {
        require(r.selection_id.len() == 64)?;
        let &(page, block, len) = lookup.get(&r.selection_id).ok_or_else(invalid)?;
        require(r.start < r.end && r.end <= len)?;
        removed_characters = removed_characters
            .checked_add(r.end - r.start)
            .ok_or_else(invalid)?;
        require(removed_characters <= 1024 * 1024)?;
        selections
            .entry((page, block))
            .or_default()
            .push((r.start, r.end));
    }
    for list in selections.values_mut() {
        list.sort_unstable();
        require(list.windows(2).all(|p| p[0].1 <= p[1].0))?;
    }
    let original = scan(&prepared, Some(&selections))?;
    for (page, operations) in prepared.pages.iter_mut().zip(original.operations) {
        page.program.operations = operations;
    }
    let bytes = redaction_core::publish(prepared.pages)?;
    let reopened = crate::redaction_numbers::load_pdf(&bytes)?;
    let actual = if let Some(images) = images {
        inspect_area_glyphs_with_context(&reopened, images)?
    } else {
        inspect_standard_glyphs(&reopened)?
    };
    let mut kept = original.blocks.iter().flat_map(|b| {
        b.glyphs.iter().filter_map(|g| {
            let key = (b.source.page_index, b.source.block_index);
            let selected = selections.get(&key).is_some_and(|r| selected(r, g.offset));
            (!selected).then_some((key, g))
        })
    });
    for block in &actual {
        for glyph in &block.glyphs {
            let (key, expected) = kept.next().ok_or_else(invalid)?;
            require(
                key == (block.source.page_index, block.source.block_index)
                    && expected.code == glyph.code
                    && expected.unicode == glyph.unicode,
            )?;
            require(
                expected
                    .origin
                    .iter()
                    .zip(glyph.origin)
                    .chain(expected.end.iter().zip(glyph.end))
                    .all(|(a, b)| (a - b).abs() <= 0.002),
            )?;
        }
    }
    require(kept.next().is_none())?;
    Ok(RedactedTextRangesPdf {
        bytes,
        removed_characters,
        affected_blocks: selections.len(),
        omitted_metadata: prepared.omitted.into_iter().collect(),
    })
}
