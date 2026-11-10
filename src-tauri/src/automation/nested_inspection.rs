//! Source-bound discovery of text inside reusable PDF Form XObjects.
use super::{inspection::bounded_json, invalid, text_preview::valid_hash};
use crate::{DocumentInfo, EngineError, EngineResult, FormTextRun, PdfEngine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const BATCH: usize = 100;
const MAX_RUN: usize = 16 * 1024;
const MAX_RESULT: usize = 2 * 1024 * 1024;

pub(crate) fn valid_selection(value: &str) -> bool {
    value
        .strip_prefix("nested-text-v1-")
        .is_some_and(valid_hash)
}

/// Includes the actual CLI/MCP envelope, duplicate escaped content, request ID
/// and the final newline. Discovery never truncates a source text record.
pub(crate) fn check_response(value: &Value) -> EngineResult<()> {
    bounded_json(value, MAX_RESULT - 1)
        .map(|_| ())
        .map_err(|_| {
            invalid(
                "Nested text inspection exceeds its 2 MiB response limit; nothing was truncated.",
            )
        })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn authorize(info: &DocumentInfo, page: u32) -> EngineResult<()> {
    if page == 0 || page > 10_000 || page as usize > info.pages.len() {
        return Err(invalid(
            "The selected nested text page is outside this PDF.",
        ));
    }
    if info
        .protection
        .as_ref()
        .is_some_and(|p| !p.permissions.copy_content || !p.permissions.edit_content)
    {
        return Err(EngineError::PermissionDenied(
            "inspecting or editing nested text through automation",
        ));
    }
    Ok(())
}

fn selection(source_hash: &[u8], page: u32, run: &FormTextRun) -> EngineResult<(String, Value)> {
    let bytes = bounded_json(run, MAX_RUN)?;
    let mut digest = Sha256::new();
    digest.update(b"folio-nested-text-selection-v1\0");
    digest.update(source_hash);
    digest.update(page.to_be_bytes());
    digest.update(&bytes);
    let id = format!("nested-text-v1-{}", hex(&digest.finalize()));
    let mut value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| invalid("Cannot encode inspected nested text."))?;
    value["selectionId"] = json!(id);
    bounded_json(&value, MAX_RUN)?;
    Ok((id, value))
}

/// Single-run eligibility is checked by resolve_any; paragraph seeds use the
/// native paragraph proof independently. Neither route trusts caller paths.
pub(super) fn resolve(
    engine: &PdfEngine,
    info: &DocumentInfo,
    page: u32,
    id: &str,
) -> EngineResult<(FormTextRun, String)> {
    authorize(info, page)?;
    if !valid_selection(id) {
        return Err(invalid(
            "Choose a selectionId returned by nested text inspection.",
        ));
    }
    let hash = Sha256::digest(engine.source_bytes(&info.id)?.as_ref());
    let listed = engine.list_occurrence_form_text_runs(&info.id, page as usize - 1)?;
    let mut matches = listed
        .runs
        .into_iter()
        .filter(|run| selection(&hash, page, run).is_ok_and(|(value, _)| value == id));
    let run = matches.next().ok_or_else(|| {
        invalid("The nested text selection is stale or unavailable. Inspect this PDF again.")
    })?;
    if matches.next().is_some() {
        return Err(invalid("The nested text selection is ambiguous."));
    }
    Ok((run, hex(&hash)))
}

pub(super) fn inspect(
    engine: &PdfEngine,
    info: &DocumentInfo,
    page: u32,
    offset: usize,
) -> EngineResult<Value> {
    authorize(info, page)?;
    if offset > 100_000 {
        return Err(invalid(
            "Nested text inspection offset is outside its bounds.",
        ));
    }
    let index = page as usize - 1;
    let listed = engine.list_occurrence_form_text_runs(&info.id, index)?;
    if offset > listed.runs.len() {
        return Err(invalid(
            "Nested text inspection offset is beyond the available runs.",
        ));
    }
    let hash = Sha256::digest(engine.source_bytes(&info.id)?.as_ref());
    let end = offset.saturating_add(BATCH).min(listed.runs.len());
    let runs = listed.runs[offset..end]
        .iter()
        .map(|run| selection(&hash, page, run).map(|(_, value)| value))
        .collect::<EngineResult<Vec<_>>>()?;
    let mut result = json!({
        "page":page, "width":info.pages[index].width, "height":info.pages[index].height,
        "coordinateSpace":"displayed-page-points", "selectionKind":"nested",
        "sourceFingerprint":hex(&hash), "offset":offset, "totalRuns":listed.runs.len(),
        "nextOffset":if end < listed.runs.len() {Some(end)} else {None}, "runs":runs
    });
    if let Some(reason) = listed.reason {
        result["reason"] = json!(reason);
    }
    check_response(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: String) -> FormTextRun {
        serde_json::from_value(json!({
            "objectPath":[0,1],"text":text,"fontName":"Helvetica","fontSize":12,
            "isEmbedded":false,"bounds":{"x":10,"y":20,"width":100,"height":14},
            "supported":true,"canSubstitute":false
        }))
        .unwrap()
    }

    #[test]
    fn complete_nested_record_includes_selection_id_in_byte_limit() {
        let empty = run(String::new());
        let native = serde_json::to_vec(&empty).unwrap().len();
        let (_, record) = selection(&[1; 32], 1, &empty).unwrap();
        let complete = serde_json::to_vec(&record).unwrap().len();
        assert!(complete > native);
        let accepted = run("x".repeat(MAX_RUN - complete));
        assert_eq!(
            serde_json::to_vec(&selection(&[1; 32], 1, &accepted).unwrap().1)
                .unwrap()
                .len(),
            MAX_RUN
        );
        let rejected = run("x".repeat(MAX_RUN - complete + 1));
        assert!(serde_json::to_vec(&rejected).unwrap().len() < MAX_RUN);
        assert!(selection(&[1; 32], 1, &rejected).is_err());
    }

    #[test]
    fn nested_identity_includes_page_source_and_every_path_component() {
        let mut run = run("Exact café \"text\"".into());
        let id = selection(&[1; 32], 1, &run).unwrap().0;
        assert!(valid_selection(&id));
        assert_eq!(selection(&[1; 32], 1, &run).unwrap().0, id);
        assert_ne!(selection(&[2; 32], 1, &run).unwrap().0, id);
        assert_ne!(selection(&[1; 32], 2, &run).unwrap().0, id);
        run.object_path[1] = 2;
        assert_ne!(selection(&[1; 32], 1, &run).unwrap().0, id);
        assert!(!valid_selection(&format!("text-v1-{}", "a".repeat(64))));
        assert!(!valid_selection(&format!(
            "nested-text-v1-{}",
            "A".repeat(64)
        )));
    }

    #[test]
    fn final_nested_frame_reserves_newline_and_counts_utf8_escapes() {
        let frame = |text: String| json!({"ok":true,"command":"inspect-nested-text","result":{"text":text}});
        let overhead = serde_json::to_vec(&frame(String::new())).unwrap().len();
        assert!(check_response(&frame("x".repeat(MAX_RESULT - overhead - 1))).is_ok());
        assert!(check_response(&frame("x".repeat(MAX_RESULT - overhead))).is_err());
        assert!(check_response(&frame("é".repeat(MAX_RESULT / 2))).is_err());
        assert!(check_response(&frame("\"".repeat(MAX_RESULT / 2))).is_err());
    }
}
