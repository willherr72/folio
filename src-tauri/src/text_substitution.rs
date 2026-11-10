//! Private source-bound single-show font publication; no caller-supplied recipes.
use super::*;
use lopdf::{
    content::{Content, Operation},
    dictionary, Dictionary, Document, Object, Stream, StringFormat,
};

const LIMIT: usize = 16 * 1024 * 1024;
fn bad() -> EngineError {
    invalid("The selected text cannot preserve its source font and positioning.")
}
fn number(value: &Object) -> EngineResult<f32> {
    value
        .as_float()
        .ok()
        .filter(|v| v.is_finite() && v.abs() <= 1_000_000.)
        .ok_or_else(bad)
}
#[derive(Clone, Copy)]
enum FontMode {
    Original,
    Substitute,
}
fn spacing(value: &Object, mode: FontMode) -> EngineResult<f32> {
    match mode {
        // Retained state may be reset before use, or Tw may remain dormant
        // without code32. Preserve finite values; visible geometry and any
        // generated cursor correction are still validated independently.
        FontMode::Original => value
            .as_float()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or_else(bad),
        FontMode::Substitute => number(value),
    }
}
fn scalar(op: &Operation, mode: FontMode) -> EngineResult<f32> {
    if op.operands.len() != 1 {
        return Err(bad());
    }
    spacing(&op.operands[0], mode)
}
fn equal(a: &[Operation], b: &[Operation]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(a, b)| a.operator == b.operator && a.operands == b.operands)
}
#[derive(Clone, Default)]
struct State {
    font: Option<Vec<Object>>,
    character: f32,
    word: f32,
}
struct Mapped {
    pdf: Document,
    page: lopdf::ObjectId,
    resources: Dictionary,
    fonts: Dictionary,
    ops: Vec<Operation>,
    shows: Vec<(usize, State)>,
    index: usize,
    ordinal: usize,
    state: State,
    encoded: Vec<u8>,
}
fn page_map(bytes: &[u8], runs: &[RunSnapshot], mode: FontMode) -> EngineResult<Mapped> {
    let pdf = Document::load_mem_with_options(
        bytes,
        lopdf::LoadOptions::with_max_decompressed_size(LIMIT),
    )
    .map_err(|_| bad())?;
    let pages = pdf.get_pages();
    if pages.len() != 1 {
        return Err(bad());
    }
    let page = *pages.values().next().unwrap();
    let data = pdf
        .get_page_content_with_limit(page, LIMIT)
        .map_err(|_| bad())?;
    let ops = Content::decode_strict(&data).map_err(|_| bad())?.operations;
    if ops.len() > 100_000 {
        return Err(bad());
    }
    let mut current = page;
    let mut resources = None;
    for _ in 0..64 {
        let node = pdf.get_dictionary(current).map_err(|_| bad())?;
        if let Ok(value) = node.get(b"Resources") {
            resources = Some(
                pdf.dereference(value)
                    .and_then(|(_, v)| v.as_dict())
                    .map_err(|_| bad())?
                    .clone(),
            );
            break;
        }
        current = node
            .get(b"Parent")
            .and_then(Object::as_reference)
            .map_err(|_| bad())?;
    }
    let resources = resources.ok_or_else(bad)?;
    let fonts = pdf
        .dereference(resources.get(b"Font").map_err(|_| bad())?)
        .and_then(|(_, v)| v.as_dict())
        .map_err(|_| bad())?
        .clone();
    let mut state = State::default();
    let mut stack = Vec::new();
    let mut inside = false;
    let mut shows = Vec::new();
    for (index, op) in ops.iter().enumerate() {
        match op.operator.as_str() {
            "q" => {
                if stack.len() >= 64 {
                    return Err(bad());
                }
                stack.push(state.clone());
            }
            "Q" => state = stack.pop().ok_or_else(bad)?,
            "BT" => {
                if inside {
                    return Err(bad());
                }
                inside = true;
            }
            "ET" => {
                if !inside {
                    return Err(bad());
                }
                inside = false;
            }
            "Tf" => {
                if op.operands.len() != 2 || op.operands[0].as_name().is_err() {
                    return Err(bad());
                }
                let size = number(&op.operands[1])?;
                if size <= 0. || size > 10_000. {
                    return Err(bad());
                }
                state.font = Some(op.operands.clone());
            }
            "Tc" => state.character = scalar(op, mode)?,
            "Tw" => state.word = scalar(op, mode)?,
            "Tj" | "TJ" | "'" | "\"" => {
                if !inside {
                    return Err(bad());
                }
                if positioned_text::cursor_only(op) {
                    continue;
                }
                if op.operator == "\"" {
                    if op.operands.len() != 3 {
                        return Err(bad());
                    }
                    state.word = spacing(&op.operands[0], mode)?;
                    state.character = spacing(&op.operands[1], mode)?;
                }
                shows.push((index, state.clone()));
            }
            _ => {}
        }
    }
    if inside || !stack.is_empty() || shows.len() != runs.len() {
        return Err(bad());
    }
    Ok(Mapped {
        pdf,
        page,
        resources,
        fonts,
        ops,
        shows,
        index: 0,
        ordinal: 0,
        state: State::default(),
        encoded: vec![],
    })
}
fn bind(m: &mut Mapped, ordinal: usize, selected: &RunSnapshot) -> EngineResult<()> {
    let (index, state) = m.shows.get(ordinal).cloned().ok_or_else(bad)?;
    let font = state.font.as_ref().ok_or_else(bad)?;
    let resource = m
        .pdf
        .dereference(
            m.fonts
                .get(font[0].as_name().map_err(|_| bad())?)
                .map_err(|_| bad())?,
        )
        .and_then(|(_, v)| v.as_dict())
        .map_err(|_| bad())?;
    if !embedded_font::matches_name(&m.pdf, resource, &selected.font, selected.embedded)
        || (number(&font[1])? - selected.font_size).abs() > EPSILON
    {
        return Err(bad());
    }
    let encoded = positioned_text::encoded(selected, &selected.text)?;
    match (
        m.ops[index].operator.as_str(),
        m.ops[index].operands.as_slice(),
    ) {
        ("Tj" | "'", [v]) | ("\"", [_, _, v]) => {
            if v.as_str().ok() != Some(encoded.as_slice()) {
                return Err(bad());
            }
        }
        ("TJ", [Object::Array(values)]) => {
            let bounds =
                text_show::boundaries(&selected.text, |ch| positioned_text::encoded(selected, ch))?;
            text_show::validate_segments(values, &encoded, &bounds)?;
        }
        _ => return Err(bad()),
    }
    m.index = index;
    m.ordinal = ordinal;
    m.state = state;
    m.encoded = encoded;
    Ok(())
}
fn map(
    bytes: &[u8],
    selected: &RunSnapshot,
    runs: &[RunSnapshot],
    mode: FontMode,
) -> EngineResult<Mapped> {
    let mut m = page_map(bytes, runs, mode)?;
    let ordinal = runs
        .iter()
        .position(|r| r.index == selected.index)
        .ok_or_else(bad)?;
    bind(&mut m, ordinal, selected)?;
    Ok(m)
}
pub(super) fn admit_all(
    bytes: &[u8],
    runs: &[RunSnapshot],
    values: &[Option<String>],
) -> EngineResult<Vec<bool>> {
    let mut m = page_map(bytes, runs, FontMode::Substitute)?;
    if m.fonts.len() > 64 {
        return Err(bad());
    }
    Ok(runs
        .iter()
        .enumerate()
        .map(|(ordinal, run)| {
            let source = source_edit_run(run, values.get(ordinal).and_then(|v| v.as_deref()));
            bind(&mut m, ordinal, &source).is_ok()
        })
        .collect())
}
fn gaps(values: &[Object]) -> EngineResult<f64> {
    values
        .iter()
        .filter(|v| v.as_str().is_err())
        .try_fold(0., |sum, v| Ok(sum + f64::from(number(v)?)))
}
fn depends(ops: &[Operation]) -> bool {
    for op in ops {
        match op.operator.as_str() {
            "BT" | "ET" | "Tm" | "Td" | "TD" | "T*" | "'" | "\"" => return false,
            "Tj" | "TJ" if !positioned_text::cursor_only(op) => return true,
            _ => {}
        }
    }
    false
}
fn numeric(value: f64) -> EngineResult<Object> {
    if !value.is_finite() || value.abs() > 1_000_000. {
        return Err(bad());
    }
    Ok(Object::Real(value as f32))
}
fn source_width(run: &RunSnapshot, text: &str) -> EngineResult<f64> {
    if let Some(proof) = &run.proof {
        return Ok(f64::from(proof.advance(text)?));
    }
    if let Some(proof) = &run.standard {
        return Ok(f64::from(proof.advance(text)?));
    }
    let font: TextFont =
        serde_json::from_value(serde_json::Value::String(run.font.clone())).map_err(|_| bad())?;
    Ok(f64::from(persistence::text_advance(text, font)?))
}
fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_be_bytes).collect()
}
fn inject_spaces(values: Vec<Object>, word: f32, size: f32) -> EngineResult<Vec<Object>> {
    if word == 0. {
        return Ok(values);
    }
    let adjustment = numeric(-f64::from(word) / f64::from(size) * 1000.)?;
    let mut result = Vec::new();
    for value in values {
        if let Object::String(bytes, _) = value {
            if bytes.len() % 2 != 0 {
                return Err(bad());
            }
            let mut start = 0;
            for (i, ch) in bytes.chunks_exact(2).enumerate() {
                if ch == [0, 32] {
                    let end = (i + 1) * 2;
                    result.push(Object::String(
                        bytes[start..end].to_vec(),
                        StringFormat::Hexadecimal,
                    ));
                    result.push(adjustment.clone());
                    start = end;
                }
            }
            if start < bytes.len() {
                result.push(Object::String(
                    bytes[start..].to_vec(),
                    StringFormat::Hexadecimal,
                ));
            }
        } else {
            result.push(value);
        }
    }
    Ok(result)
}

pub(super) struct Plan {
    source: Arc<Vec<Operation>>,
    candidate: Arc<Vec<Operation>>,
    ordinal: usize,
    bytes: Vec<u8>,
}
impl Plan {
    pub(super) fn matches_source(&self, ops: &[Operation], ordinal: usize) -> bool {
        self.ordinal == ordinal && equal(&self.source, ops)
    }
    pub(super) fn candidate(&self) -> Arc<Vec<Operation>> {
        self.candidate.clone()
    }
    pub(super) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
    /// Preserve the original font and show operator, correcting only the cursor
    /// consumed by subsequent relative text. The text line matrix is unchanged.
    pub(super) fn prepare_original(
        bytes: &[u8],
        selected: &RunSnapshot,
        runs: &[RunSnapshot],
        replacement: &str,
    ) -> EngineResult<Self> {
        // Outer context planning runs before the transaction's public checks.
        // Bound the input here as well, before mapping or embedded encoding.
        if !embedded_font::latin(replacement) {
            return Err(bad());
        }
        let mut m = map(bytes, selected, runs, FontMode::Original)?;
        let source = Arc::new(m.ops.clone());
        let original_font = m.state.font.as_ref().ok_or_else(bad)?;
        let size = number(&original_font[1])?;
        let encoded = positioned_text::encoded(selected, replacement)?;
        let old = m.ops[m.index].clone();
        let mut changed = old.clone();
        let (old_gaps, new_gaps) = match (old.operator.as_str(), changed.operands.as_mut_slice()) {
            ("Tj" | "'", [v]) | ("\"", [_, _, v]) => {
                if replacement != selected.text {
                    *v = Object::String(encoded.clone(), StringFormat::Hexadecimal);
                }
                (0., 0.)
            }
            ("TJ", [Object::Array(values)]) => {
                let old_gaps = gaps(values)?;
                *values = text_show::rewrite_array(
                    values,
                    &m.encoded,
                    &encoded,
                    &text_show::boundaries(&selected.text, |ch| {
                        positioned_text::encoded(selected, ch)
                    })?,
                    &text_show::boundaries(replacement, |ch| {
                        positioned_text::encoded(selected, ch)
                    })?,
                    replacement == selected.text,
                )?;
                (old_gaps, gaps(values)?)
            }
            _ => return Err(bad()),
        };
        let end = m.index + 1;
        let correction = if replacement != selected.text && depends(&m.ops[end..]) {
            // Tw applies to encoded single-byte code32, never a byte inside a
            // multibyte code. The original font/encoding remain unchanged.
            let spaces = if selected.proof.as_ref().is_none_or(|p| p.single_byte()) {
                encoded.iter().filter(|&&b| b == 32).count() as f64
                    - m.encoded.iter().filter(|&&b| b == 32).count() as f64
            } else {
                0.
            };
            let chars = replacement.chars().count() as f64 - selected.text.chars().count() as f64;
            let spacing = chars * f64::from(m.state.character) + spaces * f64::from(m.state.word);
            (source_width(selected, replacement)? - source_width(selected, &selected.text)?
                + spacing / f64::from(size))
                * 1000.
                - new_gaps
                + old_gaps
        } else {
            0.
        };
        m.ops[m.index] = changed;
        if correction != 0. {
            if m.ops.get(end).is_some_and(positioned_text::cursor_only) {
                let [Object::Array(values)] = m.ops[end].operands.as_mut_slice() else {
                    return Err(bad());
                };
                if let Some(first) = values.first_mut() {
                    *first = numeric(f64::from(number(first)?) + correction)?;
                } else {
                    values.push(numeric(correction)?);
                }
            } else {
                m.ops.insert(
                    end,
                    Operation::new("TJ", vec![Object::Array(vec![numeric(correction)?])]),
                );
            }
        }
        if m.ops.len() > 100_000 {
            return Err(bad());
        }
        let candidate = Arc::new(m.ops);
        let data = Content {
            operations: (*candidate).clone(),
        }
        .encode()
        .map_err(|_| bad())?;
        if data.len() > LIMIT {
            return Err(bad());
        }
        let content = m.pdf.add_object(Stream::new(dictionary! {}, data));
        m.pdf
            .get_dictionary_mut(m.page)
            .map_err(|_| bad())?
            .set("Contents", content);
        let mut bytes = Vec::new();
        m.pdf.save_to(&mut bytes).map_err(|_| bad())?;
        if bytes.len() as u64 > MAX_SOURCE_BYTES {
            return Err(bad());
        }
        Ok(Self {
            source,
            candidate,
            ordinal: m.ordinal,
            bytes,
        })
    }
    pub(super) fn prepare(
        bytes: &[u8],
        selected: &RunSnapshot,
        runs: &[RunSnapshot],
        replacement: &str,
        asset: &FontAsset,
    ) -> EngineResult<Self> {
        asset.validate_text(replacement)?;
        if !embedded_font::latin(replacement) {
            return Err(bad());
        }
        let mut m = map(bytes, selected, runs, FontMode::Substitute)?;
        // Only substitution searches all aliases for a reusable font. Keeping
        // the source font uses one direct lookup and retains its prior policy.
        if m.fonts.len() > 64 {
            return Err(bad());
        }
        let source = Arc::new(m.ops.clone());
        let original_font = m.state.font.clone().ok_or_else(bad)?;
        let size = number(&original_font[1])?;
        let old = &m.ops[m.index];
        let old_values = match (old.operator.as_str(), old.operands.as_slice()) {
            ("TJ", [Object::Array(values)]) => values.clone(),
            ("Tj" | "'", [v]) | ("\"", [_, _, v]) => vec![v.clone()],
            _ => return Err(bad()),
        };
        let old_boundaries =
            text_show::boundaries(&selected.text, |ch| positioned_text::encoded(selected, ch))?;
        let new_boundaries = text_show::boundaries(replacement, |ch| Ok(utf16(ch)))?;
        let encoded = utf16(replacement);
        let values = text_show::rewrite_array(
            &old_values,
            &m.encoded,
            &encoded,
            &old_boundaries,
            &new_boundaries,
            replacement == selected.text,
        )?;
        let single_byte = selected
            .proof
            .as_ref()
            .is_none_or(|proof| proof.single_byte());
        let old_spaces = if single_byte {
            m.encoded.iter().filter(|&&b| b == 32).count()
        } else {
            0
        };
        let word_applies =
            single_byte && positioned_text::encoded(selected, " ").is_ok_and(|v| v == [32]);
        if single_byte && m.state.word != 0. && old_spaces > 0 && !word_applies {
            return Err(bad());
        }
        let values = inject_spaces(values, if word_applies { m.state.word } else { 0. }, size)?;
        let desired_name = format!("Folio{}Paragraph", asset.info.id);
        let mut reuse = None;
        for (name, value) in m.fonts.iter() {
            let Ok((_, resource)) = m.pdf.dereference(value) else {
                continue;
            };
            let Ok(font) = resource.as_dict() else {
                continue;
            };
            if embedded_font::renderer_name(&m.pdf, font).ok().as_deref()
                != Some(desired_name.as_str())
            {
                continue;
            }
            if embedded_font::matches_asset_resource(&m.pdf, font, asset, replacement)? {
                reuse = Some(name.clone());
                break;
            }
        }
        let alias = if let Some(name) = reuse {
            name
        } else {
            let mut name = b"FolioReplacement".to_vec();
            while m.fonts.has(&name) {
                name.push(b'_');
                if name.len() > 1024 {
                    return Err(bad());
                }
            }
            let font = persistence::install_paragraph_font_resource(&mut m.pdf, asset)?;
            m.fonts.set(name.clone(), font);
            name
        };
        let mut block = Vec::new();
        if old.operator == "\"" {
            block.push(Operation::new("Tw", vec![old.operands[0].clone()]));
            block.push(Operation::new("Tc", vec![old.operands[1].clone()]));
        }
        if matches!(old.operator.as_str(), "'" | "\"") {
            block.push(Operation::new("T*", vec![]));
        }
        block.push(Operation::new(
            "Tf",
            vec![Object::Name(alias), original_font[1].clone()],
        ));
        block.push(Operation::new("TJ", vec![Object::Array(values.clone())]));
        let mut end = m.index + 1;
        let correction = if depends(&m.ops[end..]) {
            let new_width = replacement
                .chars()
                .try_fold(0f64, |sum, ch| -> EngineResult<f64> {
                    Ok(sum + f64::from((asset.advance(ch)? * 1000.).round() / 1000.))
                })?;
            let chars = replacement.chars().count() as f64 - selected.text.chars().count() as f64;
            let spacing =
                chars * f64::from(m.state.character) - old_spaces as f64 * f64::from(m.state.word);
            (new_width - source_width(selected, &selected.text)? + spacing / f64::from(size))
                * 1000.
                - gaps(&values)?
                + gaps(&old_values)?
        } else {
            0.
        };
        if m.ops.get(end).is_some_and(positioned_text::cursor_only) {
            let mut cursor = m.ops[end].clone();
            end += 1;
            let [Object::Array(ref mut values)] = cursor.operands.as_mut_slice() else {
                return Err(bad());
            };
            if correction != 0. {
                if let Some(first) = values.first_mut() {
                    *first = numeric(f64::from(number(first)?) + correction)?;
                } else {
                    values.push(numeric(correction)?);
                }
            }
            block.push(cursor);
        } else if correction != 0. {
            block.push(Operation::new(
                "TJ",
                vec![Object::Array(vec![numeric(correction)?])],
            ));
        }
        // Replace only adjacent, semantically redundant selections at this show.
        // A previous local restore is reused rather than nested on every edit.
        if m.ops.get(end).is_some_and(|op| op.operator == "Tf") {
            block.push(m.ops[end].clone());
            end += 1;
        } else {
            block.push(Operation::new("Tf", original_font));
        }
        let start = if m.index > 0 && m.ops[m.index - 1].operator == "Tf" {
            m.index - 1
        } else {
            m.index
        };
        m.ops.splice(start..end, block);
        if m.ops.len() > 100_000 {
            return Err(bad());
        }
        let candidate = Arc::new(m.ops);
        let data = Content {
            operations: (*candidate).clone(),
        }
        .encode()
        .map_err(|_| bad())?;
        if data.len() > LIMIT {
            return Err(bad());
        }
        m.resources.set("Font", m.fonts);
        let content = m.pdf.add_object(Stream::new(dictionary! {}, data));
        let page = m.pdf.get_dictionary_mut(m.page).map_err(|_| bad())?;
        page.set("Resources", m.resources);
        page.set("Contents", content);
        let mut bytes = Vec::new();
        m.pdf.save_to(&mut bytes).map_err(|_| bad())?;
        if bytes.len() as u64 > MAX_SOURCE_BYTES {
            return Err(bad());
        }
        Ok(Self {
            source,
            candidate,
            ordinal: m.ordinal,
            bytes,
        })
    }
}
