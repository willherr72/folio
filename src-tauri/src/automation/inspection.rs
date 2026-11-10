//! Stable text selections without a persistent engine session or native UUIDs.
use crate::{DocumentInfo, EditableTextRun, FormTextRun, EngineError, EngineResult, PdfEngine};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Write;

pub(super) enum ResolvedText {
    Page(EditableTextRun),
    Nested(FormTextRun),
}

pub(super) fn resolve_any_seed(engine: &PdfEngine, info: &DocumentInfo, page: u32, id: &str)
    -> EngineResult<(ResolvedText, String)> {
    if super::nested_inspection::valid_selection(id) {
        super::nested_inspection::resolve(engine,info,page,id).map(|(run,hash)|(ResolvedText::Nested(run),hash))
    } else {
        resolve_seed(engine,info,page,id).map(|(run,hash)|(ResolvedText::Page(run),hash))
    }
}

pub(super) fn resolve_any(engine: &PdfEngine, info: &DocumentInfo, page: u32, id: &str)
    -> EngineResult<(ResolvedText, String)> {
    let (run, hash) = resolve_any_seed(engine,info,page,id)?;
    let supported = match &run {ResolvedText::Page(run)=>run.supported,ResolvedText::Nested(run)=>run.supported};
    if !supported {return Err(invalid("This source text is not eligible for preserved-format replacement."));}
    Ok((run,hash))
}

const BATCH: usize = 100;
const MAX_RUN: usize = 16 * 1024;
const MAX_RESULT: usize = 2 * 1024 * 1024;

fn invalid(message: &str) -> EngineError {
    EngineError::InvalidRequest(message.into())
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

// Stop before extending the serialization buffer, including JSON escape expansion.
struct BoundedJson {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for BoundedJson {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(std::io::Error::other(
                "Text inspection response exceeds its byte limit.",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(super) fn bounded_json(value: &impl Serialize, limit: usize) -> EngineResult<Vec<u8>> {
    let mut writer = BoundedJson {
        bytes: Vec::new(),
        limit,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| {
        invalid("Text inspection exceeds its response byte limit; text was not truncated.")
    })?;
    Ok(writer.bytes)
}

fn selection(
    source_hash: &[u8],
    page: u32,
    run: &EditableTextRun,
) -> EngineResult<(String, Vec<u8>)> {
    let bytes = bounded_json(run, MAX_RUN)?;
    let mut digest = Sha256::new();
    digest.update(b"folio-text-selection-v1\0");
    digest.update(source_hash);
    digest.update(page.to_be_bytes());
    digest.update(&bytes);
    Ok((format!("text-v1-{}", hex(&digest.finalize())), bytes))
}

pub(super) fn resolve(
    engine: &PdfEngine,
    info: &DocumentInfo,
    page: u32,
    id: &str,
) -> EngineResult<(EditableTextRun, String)> {
    let (run, hash) = resolve_seed(engine, info, page, id)?;
    if !run.supported {
        return Err(invalid(
            "This source text is not eligible for preserved-format replacement.",
        ));
    }
    Ok((run, hash))
}

/// Paragraph eligibility is decided by the native paragraph proof, independently
/// of whether this run admits a single-line replacement.
pub(super) fn resolve_seed(
    engine: &PdfEngine,
    info: &DocumentInfo,
    page: u32,
    id: &str,
) -> EngineResult<(EditableTextRun, String)> {
    if page == 0 || page > 10_000 || page as usize > info.pages.len() {
        return Err(invalid("The selected text page is outside this PDF."));
    }
    if info
        .protection
        .as_ref()
        .is_some_and(|p| !p.permissions.copy_content || !p.permissions.edit_content)
    {
        return Err(EngineError::PermissionDenied(
            "editing existing text through automation",
        ));
    }
    let hash = Sha256::digest(engine.source_bytes(&info.id)?.as_ref());
    let listed = engine.list_occurrence_text_runs(&info.id, page as usize - 1)?;
    let mut matches = listed
        .runs
        .into_iter()
        .filter(|run| selection(&hash, page, run).is_ok_and(|(value, _)| value == id));
    let run = matches.next().ok_or_else(|| {
        invalid("The text selection is stale or unavailable. Inspect this PDF again.")
    })?;
    if matches.next().is_some() {
        return Err(invalid("The text selection is ambiguous."));
    }
    Ok((run, hex(&hash)))
}

pub(super) fn inspect(
    engine: &PdfEngine,
    info: &DocumentInfo,
    page: u32,
    offset: usize,
) -> EngineResult<Value> {
    if page == 0 || page > 10_000 || page as usize > info.pages.len() || offset > 100_000 {
        return Err(invalid(
            "Text inspection page or offset is outside its bounds.",
        ));
    }
    if info
        .protection
        .as_ref()
        .is_some_and(|p| !p.permissions.copy_content || !p.permissions.edit_content)
    {
        return Err(EngineError::PermissionDenied(
            "inspecting existing text for editing",
        ));
    }
    let index = page as usize - 1;
    let listed = engine.list_occurrence_text_runs(&info.id, index)?;
    if offset > listed.runs.len() {
        return Err(invalid(
            "Text inspection offset is beyond the available runs.",
        ));
    }
    let source_hash = Sha256::digest(engine.source_bytes(&info.id)?.as_ref());
    let end = offset.saturating_add(BATCH).min(listed.runs.len());
    let mut runs = Vec::with_capacity(end - offset);
    for run in &listed.runs[offset..end] {
        let (selection, bytes) = selection(&source_hash, page, run)?;
        let mut value: Value =
            serde_json::from_slice(&bytes).map_err(|_| invalid("Cannot encode inspected text."))?;
        value["selectionId"] = json!(selection);
        // The wire limit includes the identifier added to the native record.
        bounded_json(&value, MAX_RUN)?;
        runs.push(value);
    }
    let mut result = json!({
        "page": page, "width":info.pages[index].width, "height":info.pages[index].height,
        "coordinateSpace":"displayed-page-points", "sourceFingerprint":hex(&source_hash),
        "offset":offset,"totalRuns":listed.runs.len(),
        "nextOffset":if end < listed.runs.len() {Some(end)} else {None}, "runs":runs
    });
    if let Some(reason) = listed.reason {
        result["reason"] = json!(reason);
    }
    bounded_json(&result, MAX_RESULT)?;
    Ok(result)
}
