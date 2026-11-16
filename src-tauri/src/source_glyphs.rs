//! Encoding model for already verified PDF glyphs. This is not a font verifier.
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
    sync::Arc,
};

type Result<T> = std::result::Result<T, &'static str>;
const MAX_TEXT: usize = 4096;
const MAX_ENCODED: usize = 8192;
const MAX_CELLS: usize = (MAX_TEXT + 1) * (MAX_TEXT + 1);
const IMPOSSIBLE: u16 = u16::MAX;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Symbol {
    pub code: u16,
    pub text: String,
    pub gid: u16,
    pub width: f32,
}

#[derive(Debug, PartialEq)]
pub(super) struct Codebook {
    symbols: BTreeMap<u16, Symbol>,
    first: BTreeMap<char, Vec<u16>>,
    code_len: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct GlyphAtom {
    pub code: u16,
    pub gid: u16,
    pub unicode: Range<usize>,
    pub width: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct EncodedText {
    text: String,
    bytes: Vec<u8>,
    atoms: Vec<GlyphAtom>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct BoundText {
    book: Arc<Codebook>,
    encoded: EncodedText,
}

impl Codebook {
    pub(super) fn retained_bytes(&self) -> usize {
        // Conservative charge for both trees, node slack, text allocations,
        // first-character vectors and their allocator overhead. Size is bounded
        // by new(); callers include this in the aggregate font-proof budget.
        std::mem::size_of::<Self>()
            + self.symbols.len() * 384
            + self
                .symbols
                .values()
                .map(|s| s.text.capacity())
                .sum::<usize>()
            + self
                .first
                .values()
                .map(|v| v.capacity() * std::mem::size_of::<u16>())
                .sum::<usize>()
    }
    pub(super) fn new(symbols: Vec<Symbol>, code_len: usize) -> Result<Self> {
        if !matches!(code_len, 1 | 2)
            || symbols.is_empty()
            || symbols.len() > if code_len == 1 { 256 } else { 65536 }
        {
            return Err("Invalid glyph codebook size or code length.");
        }
        let mut codes = BTreeSet::new();
        let mut texts = BTreeSet::new();
        let mut sequences = 0;
        let mut sequence_bytes = 0;
        for symbol in &symbols {
            let count = symbol.text.chars().count();
            if !(1..=16).contains(&count)
                || symbol.text.len() > 64
                || symbol.text.chars().any(char::is_control)
                || (count > 1 && symbol.text.chars().any(char::is_whitespace))
                || symbol.gid == 0
                || (code_len == 1 && symbol.code > 255)
                || !symbol.width.is_finite()
                || !(0.0..=10000.).contains(&symbol.width)
                || !codes.insert(symbol.code)
                || !texts.insert(symbol.text.as_str())
            {
                return Err("Invalid or ambiguous verified glyph symbol.");
            }
            if count > 1 {
                sequences += 1;
                sequence_bytes += symbol.text.len();
                if sequences > 512 || sequence_bytes > 32768 {
                    return Err("Verified glyph sequences exceed their budget.");
                }
            }
        }
        let symbols: BTreeMap<_, _> = symbols.into_iter().map(|s| (s.code, s)).collect();
        let mut first: BTreeMap<char, Vec<u16>> = BTreeMap::new();
        for symbol in symbols.values() {
            first
                .entry(symbol.text.chars().next().unwrap())
                .or_default()
                .push(symbol.code);
        }
        for values in first.values_mut() {
            // New text prefers individual glyphs, then the longest complete sequence.
            values.sort_by_key(|code| {
                let count = symbols[code].text.chars().count();
                (count != 1, std::cmp::Reverse(count), *code)
            });
        }
        Ok(Self {
            symbols,
            first,
            code_len,
        })
    }
    pub(super) fn bind(self: &Arc<Self>, bytes: &[u8]) -> Result<BoundText> {
        if bytes.len() > MAX_ENCODED || bytes.len() % self.code_len != 0 {
            return Err("Incomplete or excessive encoded source glyphs.");
        }
        let mut encoded = EncodedText::empty();
        for value in bytes.chunks_exact(self.code_len) {
            let code = if self.code_len == 1 {
                u16::from(value[0])
            } else {
                u16::from_be_bytes([value[0], value[1]])
            };
            self.push(&mut encoded, code)?;
        }
        Ok(BoundText {
            book: Arc::clone(self),
            encoded,
        })
    }
    pub(super) fn encode(&self, text: &str) -> Result<EncodedText> {
        self.encode_separated(text, &[])
    }
    fn encode_range(
        &self,
        text: &str,
        range: Range<usize>,
        boundaries: &[usize],
    ) -> Result<EncodedText> {
        if range.is_empty() {
            return Ok(EncodedText::empty());
        }
        let first = boundaries.partition_point(|b| *b <= range.start);
        let last = boundaries.partition_point(|b| *b < range.end);
        let local: Vec<_> = boundaries[first..last]
            .iter()
            .map(|b| *b - range.start)
            .collect();
        self.encode_separated(&text[range], &local)
    }
    fn encode_separated(&self, text: &str, boundaries: &[usize]) -> Result<EncodedText> {
        let paths = self.paths_separated(text, boundaries)?;
        let mut chosen = vec![None; paths.matches.len()];
        let last = paths.matches.len() - 1;
        for index in (0..last).rev() {
            chosen[index] = paths.matches[index]
                .iter()
                .copied()
                .find(|(end, _)| *end == last || chosen[*end].is_some());
        }
        if !text.is_empty() && chosen[0].is_none() {
            return Err("Text has no complete verified glyph encoding.");
        }
        let mut output = EncodedText::empty();
        let mut index = 0;
        while index < last {
            let (end, code) = chosen[index].ok_or("Incomplete glyph encoding path.")?;
            self.push(&mut output, code)?;
            index = end;
        }
        Ok(output)
    }
    fn paths_separated(&self, text: &str, boundaries: &[usize]) -> Result<Paths> {
        check_text(text)?;
        check_separators(text, boundaries)?;
        let mut offsets: Vec<_> = text.char_indices().map(|(i, _)| i).collect();
        offsets.push(text.len());
        let mut matches = vec![Vec::new(); offsets.len()];
        for (index, ch) in text.chars().enumerate() {
            for code in self.first.get(&ch).into_iter().flatten() {
                let symbol = &self.symbols[code];
                if text[offsets[index]..].starts_with(&symbol.text)
                    && span_allowed(
                        boundaries,
                        offsets[index],
                        offsets[index] + symbol.text.len(),
                    )
                {
                    matches[index].push((index + symbol.text.chars().count(), *code));
                }
            }
        }
        Ok(Paths { offsets, matches })
    }
    fn push(&self, output: &mut EncodedText, code: u16) -> Result<()> {
        let symbol = self
            .symbols
            .get(&code)
            .ok_or("Source code has no verified glyph.")?;
        if output.text.len() + symbol.text.len() > MAX_TEXT
            || output.bytes.len() + self.code_len > MAX_ENCODED
        {
            return Err("Encoded glyph text exceeds its budget.");
        }
        let start = output.text.len();
        output.text.push_str(&symbol.text);
        let bytes = code.to_be_bytes();
        output.bytes.extend_from_slice(&bytes[2 - self.code_len..]);
        output.atoms.push(GlyphAtom {
            code,
            gid: symbol.gid,
            unicode: start..output.text.len(),
            width: symbol.width,
        });
        Ok(())
    }
}

struct Paths {
    offsets: Vec<usize>,
    // End scalar index and code, in the encoder's deterministic preference order.
    matches: Vec<Vec<(usize, u16)>>,
}

fn check_text(text: &str) -> Result<()> {
    if text.len() > MAX_TEXT || text.chars().any(char::is_control) {
        return Err("Text exceeds the bounded printable glyph model.");
    }
    Ok(())
}

// Required byte boundaries represent caller-owned layout/style controls. They
// do not become PDF character codes and can never cut through a selected atom.
fn check_separators(text: &str, boundaries: &[usize]) -> Result<()> {
    if boundaries.len() >= MAX_TEXT
        || boundaries
            .iter()
            .any(|b| *b == 0 || *b >= text.len() || !text.is_char_boundary(*b))
        || boundaries.windows(2).any(|p| p[0] >= p[1])
    {
        return Err("Invalid or excessive required glyph boundaries.");
    }
    Ok(())
}
fn span_allowed(boundaries: &[usize], start: usize, end: usize) -> bool {
    boundaries
        .get(boundaries.partition_point(|b| *b <= start))
        .is_none_or(|b| *b >= end)
}

impl BoundText {
    pub(super) fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + 128
            + self.encoded.text.capacity()
            + self.encoded.bytes.capacity()
            + self.encoded.atoms.capacity() * std::mem::size_of::<GlyphAtom>()
    }
    pub(super) fn encoded(&self) -> &EncodedText {
        &self.encoded
    }
    pub(super) fn replace(&self, text: &str) -> Result<EncodedText> {
        self.replace_separated(text, &[])
    }
    pub(super) fn replace_separated(
        &self,
        text: &str,
        boundaries: &[usize],
    ) -> Result<EncodedText> {
        check_text(text)?;
        check_separators(text, boundaries)?;
        if text == self.encoded.text
            && self
                .encoded
                .atoms
                .iter()
                .all(|atom| span_allowed(boundaries, atom.unicode.start, atom.unicode.end))
        {
            return Ok(self.encoded.clone());
        }
        let anchors = self.reuse_separated(text, boundaries)?;
        let mut output = EncodedText::empty();
        let mut cursor = 0;
        for (source, at) in anchors {
            let gap = self.book.encode_range(text, cursor..at, boundaries)?;
            for atom in gap.atoms {
                self.book.push(&mut output, atom.code)?;
            }
            let atom = &self.encoded.atoms[source];
            self.book.push(&mut output, atom.code)?;
            cursor = at + atom.unicode.len();
        }
        let gap = self
            .book
            .encode_range(text, cursor..text.len(), boundaries)?;
        for atom in gap.atoms {
            self.book.push(&mut output, atom.code)?;
        }
        Ok(output)
    }

    // Maximize retained Unicode length using ONLY whole source atoms. Encoding
    // transitions ensure gaps also have a complete path; an unconstrained LCS
    // can retain a tempting atom while stranding an unencodable gap beside it.
    #[cfg(test)]
    fn reuse(&self, text: &str) -> Result<Vec<(usize, usize)>> {
        self.reuse_separated(text, &[])
    }
    fn reuse_separated(&self, text: &str, boundaries: &[usize]) -> Result<Vec<(usize, usize)>> {
        let paths = self.book.paths_separated(text, boundaries)?;
        let atoms = &self.encoded.atoms;
        let count = paths.offsets.len() - 1;
        if atoms.is_empty() {
            self.book.encode_separated(text, boundaries)?;
            return Ok(Vec::new());
        }

        // Normal insertion between intact glyphs retains ALL source text, an
        // upper-bound optimum. Prefix/suffix matching alone is not otherwise
        // a proof: a short early atom may block a longer reusable later one.
        let mut prefix = 0;
        let mut start = 0;
        while prefix < atoms.len() {
            let value = &self.encoded.text[atoms[prefix].unicode.clone()];
            if !text[start..].starts_with(value)
                || !span_allowed(boundaries, start, start + value.len())
            {
                break;
            }
            start += value.len();
            prefix += 1;
        }
        let mut suffix = atoms.len();
        let mut end = text.len();
        while suffix > prefix {
            let value = &self.encoded.text[atoms[suffix - 1].unicode.clone()];
            if end - start < value.len()
                || !text[..end].ends_with(value)
                || !span_allowed(boundaries, end - value.len(), end)
            {
                break;
            }
            end -= value.len();
            suffix -= 1;
        }
        if prefix == suffix && self.book.encode_range(text, start..end, boundaries).is_ok() {
            let mut anchors = Vec::with_capacity(atoms.len());
            let mut at = 0;
            for (i, atom) in atoms.iter().enumerate().take(prefix) {
                anchors.push((i, at));
                at += atom.unicode.len();
            }
            at = end;
            for (i, atom) in atoms.iter().enumerate().skip(suffix) {
                anchors.push((i, at));
                at += atom.unicode.len();
            }
            return Ok(anchors);
        }

        let columns = count + 1;
        let cells = (atoms.len() + 1)
            .checked_mul(columns)
            .filter(|n| *n <= MAX_CELLS)
            .ok_or("Source glyph alignment exceeds its work budget.")?;
        let mut scores = vec![IMPOSSIBLE; cells];
        scores[atoms.len() * columns + count] = 0;
        for row in (0..=atoms.len()).rev() {
            scores[row * columns + count] = 0;
            let source = atoms
                .get(row)
                .map(|atom| &self.encoded.text[atom.unicode.clone()]);
            let length = source.map_or(0, |s| s.chars().count());
            for col in (0..count).rev() {
                let mut best = if row < atoms.len() {
                    scores[(row + 1) * columns + col]
                } else {
                    IMPOSSIBLE
                };
                let mut consider = |score| {
                    if score != IMPOSSIBLE && (best == IMPOSSIBLE || score > best) {
                        best = score;
                    }
                };
                for (end, _) in &paths.matches[col] {
                    consider(scores[row * columns + end]);
                }
                if let Some(value) = source.filter(|s| {
                    text[paths.offsets[col]..].starts_with(*s)
                        && span_allowed(
                            boundaries,
                            paths.offsets[col],
                            paths.offsets[col] + s.len(),
                        )
                }) {
                    let next = col + length;
                    debug_assert_eq!(paths.offsets[next] - paths.offsets[col], value.len());
                    let tail = scores[(row + 1) * columns + next];
                    if tail != IMPOSSIBLE {
                        consider(tail + length as u16);
                    }
                }
                scores[row * columns + col] = best;
            }
        }
        if scores[0] == IMPOSSIBLE {
            return Err("Text has no complete verified glyph encoding.");
        }
        let mut anchors = Vec::new();
        let mut row = 0;
        let mut col = 0;
        while row < atoms.len() && col < count {
            let value = &self.encoded.text[atoms[row].unicode.clone()];
            let length = value.chars().count();
            let best = scores[row * columns + col];
            if text[paths.offsets[col]..].starts_with(value)
                && span_allowed(
                    boundaries,
                    paths.offsets[col],
                    paths.offsets[col] + value.len(),
                )
                && scores[(row + 1) * columns + col + length] != IMPOSSIBLE
                && scores[(row + 1) * columns + col + length] + length as u16 == best
            {
                anchors.push((row, paths.offsets[col]));
                row += 1;
                col += length;
            } else if scores[(row + 1) * columns + col] == best {
                row += 1;
            } else {
                col = paths.matches[col]
                    .iter()
                    .find(|(end, _)| scores[row * columns + end] == best)
                    .map(|(end, _)| *end)
                    .ok_or("Incomplete source glyph alignment.")?;
            }
        }
        Ok(anchors)
    }
}

impl EncodedText {
    pub(super) fn uses_sequences(&self) -> bool {
        self.atoms.len() < self.text.chars().count()
    }
    pub(super) fn unicode_boundaries(&self) -> Vec<usize> {
        let mut result = Vec::with_capacity(self.atoms.len() + 1);
        let mut count = 0;
        result.push(0);
        for atom in &self.atoms {
            count += self.text[atom.unicode.clone()].chars().count();
            result.push(count);
        }
        result
    }
    fn empty() -> Self {
        Self {
            text: String::new(),
            bytes: Vec::new(),
            atoms: Vec::new(),
        }
    }
    pub(super) fn text(&self) -> &str {
        &self.text
    }
    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(super) fn atoms(&self) -> &[GlyphAtom] {
        &self.atoms
    }
    pub(super) fn advance(&self) -> f32 {
        self.atoms
            .iter()
            .map(|a| f64::from(a.width) / 1000.)
            .sum::<f64>() as f32
    }
    pub(super) fn byte_boundaries(&self) -> Vec<usize> {
        let length = if self.atoms.is_empty() {
            1
        } else {
            self.bytes.len() / self.atoms.len()
        };
        (0..=self.atoms.len()).map(|i| i * length).collect()
    }
}

#[cfg(test)]
#[path = "source_glyphs_tests.rs"]
mod tests;
