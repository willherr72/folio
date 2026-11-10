//! Exact operand text, distinct from PDFium's generated reading separators.
//! This does not authorize editing: the normal no-op/style/glyph/raster proofs do.
use super::*;
use lopdf::{content::Content, Dictionary, Document, Object};

const LIMIT: usize = 16 * 1024 * 1024;

fn operand_bytes(op: &lopdf::content::Operation, max_len: usize) -> EngineResult<Option<Vec<u8>>> {
    let error = || invalid("The source text operands cannot be mapped safely.");
    let encoded = match (op.operator.as_str(), op.operands.as_slice()) {
        ("Tj" | "'", [value]) | ("\"", [_, _, value]) => {
            let bytes = value.as_str().map_err(|_| error())?;
            if bytes.len() > max_len { return Ok(None); }
            bytes.to_vec()
        }
        ("TJ", [Object::Array(values)]) => {
            let mut total = 0usize;
            for value in values {
                if let Object::String(bytes, _) = value { total = total.checked_add(bytes.len()).ok_or_else(error)?; }
                else if !value.as_float().is_ok_and(f32::is_finite) { return Err(error()); }
            }
            if total > max_len { return Ok(None); }
            let mut encoded = Vec::with_capacity(total);
            for value in values {
                if let Object::String(bytes, _) = value { encoded.extend_from_slice(bytes); }
            }
            encoded
        }
        _ => return Err(error()),
    };
    Ok(Some(encoded))
}

pub(super) fn values(bytes: &[u8], runs: &[RunSnapshot]) -> EngineResult<Vec<Option<String>>> {
    if runs.is_empty() { return Ok(vec![]); }
    let error = || invalid("The source text operands cannot be mapped safely.");
    let pdf = Document::load_mem_with_options(bytes, lopdf::LoadOptions::with_max_decompressed_size(LIMIT)).map_err(|_| error())?;
    let pages = pdf.get_pages();
    if pages.len() != 1 { return Err(error()); }
    let page = *pages.values().next().unwrap();
    values_on_page(&pdf, page, runs)
}

// Paragraph font probes can contain several isolated pages. The caller supplies
// the exact page whose native snapshots are being checked; all operand and
// resource bounds are identical to the single-page editing path.
pub(super) fn values_on_page(pdf: &Document, page: lopdf::ObjectId, runs: &[RunSnapshot]) -> EngineResult<Vec<Option<String>>> {
    Ok(read_on_page(pdf, page, runs)?.into_iter().map(|row| row.0).collect())
}

pub(super) fn bind_standard(bytes: &[u8], runs: &mut [RunSnapshot]) -> EngineResult<()> {
    let pdf = Document::load_mem_with_options(bytes, lopdf::LoadOptions::with_max_decompressed_size(LIMIT))
        .map_err(|_| invalid("The source font bindings could not be decoded."))?;
    let pages = pdf.get_pages();
    if pages.len() != 1 { return Err(invalid("Source font binding requires an isolated page.")); }
    bind_standard_on_page(&pdf, *pages.values().next().unwrap(), runs)
}

pub(super) fn bind_standard_on_page(pdf: &Document, page: lopdf::ObjectId, runs: &mut [RunSnapshot]) -> EngineResult<()> {
    // Do not assign any authority until the complete operation/native mapping
    // succeeds. Each proof belongs to the actual Tf resource for that show.
    let values = read_on_page(pdf, page, runs)?;
    for (run, (_, proof)) in runs.iter_mut().zip(values) {
        if !run.embedded {
            run.standard = proof;
            if run.standard.is_some() { run.reason = run_content_reason(run); }
        }
    }
    Ok(())
}

type SourceValue = (Option<String>, Option<Arc<standard_font::StandardFont>>);
fn read_on_page(pdf: &Document, page: lopdf::ObjectId, runs: &[RunSnapshot]) -> EngineResult<Vec<SourceValue>> {
    if runs.is_empty() { return Ok(vec![]); }
    let error = || invalid("The source text operands cannot be mapped safely.");
    let data = pdf.get_page_content_with_limit(page, LIMIT).map_err(|_| error())?;
    let content = Content::decode_strict(&data).map_err(|_| error())?;
    if content.operations.len() > 100_000 { return Err(error()); }
    let mut current = page;
    let mut seen_pages = HashSet::new();
    let mut resources: Option<&Dictionary> = None;
    for _ in 0..64 {
        if !seen_pages.insert(current) { return Err(error()); }
        let d = pdf.get_dictionary(current).map_err(|_| error())?;
        if let Ok(value) = d.get(b"Resources") {
            resources = Some(pdf.dereference(value).map_err(|_| error())?.1.as_dict().map_err(|_| error())?);
            break;
        }
        current = d.get(b"Parent").and_then(Object::as_reference).map_err(|_| error())?;
    }
    let fonts = pdf.dereference(resources.ok_or_else(error)?.get(b"Font").map_err(|_| error())?).map_err(|_| error())?.1.as_dict().map_err(|_| error())?;
    let mut active: Option<(Vec<u8>, f32)> = None;
    let mut stack = vec![];
    let mut in_text = false;
    let mut result = vec![];
    for op in &content.operations {
        match op.operator.as_str() {
            "q" => { if stack.len() >= 64 { return Err(error()); } stack.push(active.clone()); }
            "Q" => active = stack.pop().ok_or_else(error)?,
            "BT" => { if in_text { return Err(error()); } in_text = true; }
            "ET" => { if !in_text { return Err(error()); } in_text = false; }
            "Tf" => {
                let [name, size] = op.operands.as_slice() else { return Err(error()); };
                let size = size.as_float().map_err(|_| error())?;
                if !size.is_finite() || size <= 0.0 || size > 10_000.0 { return Err(error()); }
                active = Some((name.as_name().map_err(|_| error())?.to_vec(), size));
            }
            "Tj" | "TJ" | "'" | "\"" => {
                if !in_text { return Err(error()); }
                if positioned_text::cursor_only(op) { continue; }
                let run = runs.get(result.len()).ok_or_else(error)?;
                let (name, size) = active.as_ref().ok_or_else(error)?;
                let font = pdf.dereference(fonts.get(name).map_err(|_| error())?).map_err(|_| error())?.1.as_dict().map_err(|_| error())?;
                if !embedded_font::matches_name(pdf, font, &run.font, run.embedded) || (size - run.font_size).abs() > EPSILON { return Err(error()); }
                let mut source = run.clone();
                if !source.embedded {
                    source.standard = standard_font::StandardFont::read(pdf, font).ok().map(Arc::new);
                }
                let run = &source;
                let max_len = if run.proof.as_ref().is_some_and(|proof| !proof.single_byte()) { 8192 } else { 4096 };
                let Some(encoded) = operand_bytes(op, max_len)? else { result.push((None, None)); continue; };
                // A unique mapping and verified encoding are required. Whitespace
                // may differ in reader extraction; every non-space character must
                // agree in the same order. The source owns the actual spaces.
                let value = (|| -> EngineResult<String> {
                    let value = if let Some(proof) = &run.proof {
                        // Bound decoder work before copying and stop decoded UTF-8
                        // output at the same byte limit used by editable text.
                        let code_len = if proof.single_byte() { 1 } else { 2 };
                        let mut value = String::new();
                        for code in encoded.chunks(code_len) {
                            let character = proof.decode(code)?;
                            if value.len() + character.len() > 4096 { return Err(error()); }
                            value.push_str(&character);
                        }
                        value
                    } else if let Some(proof) = &run.standard { proof.decode(&encoded)? }
                        else if !run.embedded && base_font(&run.font) { std::str::from_utf8(&encoded).map_err(|_| error())?.to_owned() }
                        else { return Err(error()); };
                    // Saved paragraph fragments can contain only literal spaces.
                    // Their encoding proof still belongs to the source resource,
                    // even when PDFium returns no visible text for the fragment.
                    let nominal_spaces = (run.standard.is_some() || run.proof.is_some()) && !value.is_empty()
                        && value.bytes().all(|byte| byte == b' ');
                    if !(nominal_spaces || if run.embedded || run.standard.is_some() { embedded_font::latin(&value) } else { printable(&value) })
                        || positioned_text::encoded(run, &value)? != encoded
                        || !value.chars().filter(|c| *c != ' ').eq(run.text.chars().filter(|c| !matches!(c, ' ' | '\r' | '\n'))) { return Err(error()); }
                    if let ("TJ", [Object::Array(values)]) = (op.operator.as_str(), op.operands.as_slice()) {
                        let boundaries = text_show::boundaries(&value, |c| positioned_text::encoded(run, c))?;
                        text_show::validate_segments(values, &encoded, &boundaries)?;
                    }
                    Ok(value)
                })().ok();
                let proof = value.as_ref().and(source.standard);
                result.push((value, proof));
            }
            _ => {}
        }
    }
    if result.len() != runs.len() || in_text || !stack.is_empty() { return Err(error()); }
    Ok(result)
}

pub(super) fn selected(bytes: &[u8], runs: &[RunSnapshot], index: usize) -> Option<String> {
    let ordinal = runs.iter().position(|run| run.index == index)?;
    values(bytes, runs).ok()?.get_mut(ordinal)?.take()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::Operation;
    #[test]
    fn excessive_single_show_is_skipped_before_copying_or_decoding() {
        for operator in ["Tj", "'", "\""] {
            let mut values = if operator == "\"" { vec![0.into(), 0.into()] } else { vec![] };
            values.push(Object::string_literal(vec![b'A'; 8193]));
            assert_eq!(operand_bytes(&Operation::new(operator, values), 8192).unwrap(), None);
        }
    }
    #[test]
    fn tj_budget_counts_combined_strings_and_preserves_boundary() {
        let values = vec![Object::string_literal(vec![b'A';4096]), 0.into(), Object::string_literal(vec![b'B';4096])];
        let op = Operation::new("TJ", vec![Object::Array(values)]);
        assert_eq!(operand_bytes(&op, 8191).unwrap(), None);
        assert_eq!(operand_bytes(&op, 8192).unwrap().unwrap().len(),8192);
    }
}
