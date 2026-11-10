//! Source-bound paragraph identities across disposable automation workers.
use super::{inspection, invalid, nested_inspection, paragraph_preview::OccurrenceScope, text_preview::valid_hash};
use crate::{DocumentInfo, EngineResult, FormTextRun, PdfEngine, StyledParagraphSelection};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const MAX_RESULT: usize = 256 * 1024;

/// Check the actual transport frame, reserving its trailing LF. MCP duplicates
/// and escapes content and adds the caller's request ID, so an inner cap alone
/// cannot establish the wire limit.
pub(crate) fn check_response(value: &Value) -> EngineResult<()> {
    inspection::bounded_json(value, MAX_RESULT - 1)
        .map(|_| ())
        .map_err(|_| {
            invalid(
                "Paragraph inspection exceeds its 256 KiB response limit; nothing was truncated.",
            )
        })
}

pub(crate) fn valid_seed(value: &str) -> bool {
    value.strip_prefix("text-v1-").is_some_and(valid_hash)
        || nested_inspection::valid_selection(value)
}

pub(super) enum ParagraphContext {
    Page,
    Nested { path: Vec<usize> },
}
impl ParagraphContext {
    pub fn scope(&self) -> EngineResult<Option<OccurrenceScope>> {
        match self {
            Self::Page => Ok(None),
            Self::Nested { path } => OccurrenceScope::from_path(path).map(Some),
        }
    }
}
pub(super) struct ResolvedParagraph {
    pub selection: StyledParagraphSelection,
    pub source_fingerprint: String,
    pub context: ParagraphContext,
}

fn from_seed(engine: &PdfEngine, info: &DocumentInfo, page: u32, seed: &str)
    -> EngineResult<(ResolvedParagraph, String)>
{
    let (run, fingerprint) = inspection::resolve_any_seed(engine, info, page, seed)?;
    let index = page as usize - 1;
    let (selection, context, id) = match run {
        inspection::ResolvedText::Page(run) => {
            let paragraph = engine.suggest_occurrence_styled_paragraph(&info.id, index, run.object_index, &run.text)?;
            let id = identity(page, &fingerprint, &paragraph)?;
            (paragraph, ParagraphContext::Page, id)
        }
        inspection::ResolvedText::Nested(run) => {
            let listed = engine.list_form_paragraph_runs(&info.id, index, &run.object_path, &run.text)?;
            // Object indices in the temporary projection are not path indices.
            let anchor = listed.runs.iter().find(|r| r.object_index == listed.anchor_index)
                .ok_or_else(|| invalid("The promoted paragraph anchor is unavailable."))?;
            let paragraph = engine.suggest_form_styled_paragraph(&info.id, index, &run.object_path, anchor.object_index, &anchor.text)?;
            let id = nested_identity(page, &fingerprint, &run, &paragraph)?;
            (paragraph, ParagraphContext::Nested { path: run.object_path }, id)
        }
    };
    Ok((ResolvedParagraph {selection, source_fingerprint: fingerprint, context}, id))
}

pub(super) fn inspect(
    engine: &PdfEngine,
    info: &DocumentInfo,
    page: u32,
    seed: &str,
) -> EngineResult<Value> {
    if !valid_seed(seed) {
        return Err(invalid("Choose a selectionId returned by text inspection."));
    }
    let (resolved, id) = from_seed(engine, info, page, seed)?;
    let index = page as usize - 1;
    let scope = resolved.context.scope()?;
    let mut value = encode(
        page,
        info.pages[index].width,
        info.pages[index].height,
        seed,
        &resolved.source_fingerprint,
        resolved.selection,
    )?;
    value["paragraphId"] = json!(id);
    if let Some(scope) = scope { value["occurrenceScope"] = json!(scope); }
    inspection::bounded_json(&value, MAX_RESULT)?;
    Ok(value)
}

pub(super) fn resolve(
    engine: &PdfEngine,
    info: &DocumentInfo,
    page: u32,
    seed: &str,
    expected: &str,
) -> EngineResult<ResolvedParagraph> {
    if !super::paragraph_preview::valid_pair(seed, expected) {
        return Err(invalid("Invalid paragraph selection."));
    }
    let (resolved, id) = from_seed(engine, info, page, seed)?;
    if id != expected {
        return Err(invalid(
            "The paragraph selection is stale. Inspect this PDF again.",
        ));
    }
    Ok(resolved)
}
fn nested_identity(page: u32, fingerprint: &str, seed: &FormTextRun, paragraph: &StyledParagraphSelection) -> EngineResult<String> {
    // Include complete source-bound seed provenance: two different leaves can
    // suggest identical paragraph metadata within the same outer invocation.
    let seed = inspection::bounded_json(seed, 16 * 1024)?;
    let paragraph = inspection::bounded_json(paragraph, MAX_RESULT)?;
    let mut digest = Sha256::new();
    digest.update(b"folio-paragraph-selection-v2\0");
    digest.update(fingerprint.as_bytes());
    digest.update(page.to_be_bytes());
    digest.update((seed.len() as u64).to_be_bytes());
    digest.update(seed);
    digest.update(paragraph);
    let hash: String = digest.finalize().iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!("paragraph-v2-{hash}"))
}
fn identity(
    page: u32,
    fingerprint: &str,
    paragraph: &StyledParagraphSelection,
) -> EngineResult<String> {
    // Hash only bounded native metadata. No worker, document or font UUIDs occur
    // in StyledParagraphSelection; local style IDs belong to this paragraph ID.
    let bytes = inspection::bounded_json(&paragraph, MAX_RESULT)?;
    let mut digest = Sha256::new();
    digest.update(b"folio-paragraph-selection-v1\0");
    digest.update(fingerprint.as_bytes());
    digest.update(page.to_be_bytes());
    digest.update(&bytes);
    let hash: String = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok(format!("paragraph-v1-{hash}"))
}

fn encode(
    page: u32,
    width: f32,
    height: f32,
    seed: &str,
    fingerprint: &str,
    paragraph: StyledParagraphSelection,
) -> EngineResult<Value> {
    let id = identity(page, fingerprint, &paragraph)?;
    let value = json!({
        "page":page, "width":width, "height":height,
        "coordinateSpace":"displayed-page-points", "sourceFingerprint":fingerprint,
        "seedSelectionId":seed, "paragraphId":id, "paragraph":paragraph
    });
    inspection::bounded_json(&value, MAX_RESULT)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AnnotationRect;

    fn paragraph(text: String) -> StyledParagraphSelection {
        StyledParagraphSelection {
            baseline_anchor: None,
            object_indices: vec![0],
            text,
            runs: vec![],
            styles: vec![],
            bounds: AnnotationRect {
                x: 0.,
                y: 0.,
                width: 100.,
                height: 100.,
            },
            alignment: None,
            line_spacing: None,
        }
    }

    #[test]
    fn result_limit_counts_escaping_and_selection_envelope_without_truncating() {
        let seed = format!("text-v1-{}", "0".repeat(64));
        let fingerprint = "1".repeat(64);
        for character in ['x', '"', '\u{0001}', 'é'] {
            let mut p = paragraph(String::new());
            let overhead = serde_json::to_vec(&p).unwrap().len();
            let char_bytes = serde_json::to_vec(&character.to_string()).unwrap().len() - 2;
            p.text = character
                .to_string()
                .repeat((MAX_RESULT - overhead - 16) / char_bytes);
            assert!(inspection::bounded_json(&p, MAX_RESULT).is_ok());
            assert!(
                encode(1, 100., 100., &seed, &fingerprint, p).is_err(),
                "The paragraph fits alone but its result envelope must not exceed the cap"
            );
            let p = paragraph(character.to_string().repeat(MAX_RESULT));
            assert!(encode(1, 100., 100., &seed, &fingerprint, p).is_err());
        }
        let p = paragraph("Exact café \"text\"".into());
        let result = encode(1, 100., 100., &seed, &fingerprint, p.clone()).unwrap();
        assert_eq!(
            serde_json::from_value::<StyledParagraphSelection>(result["paragraph"].clone())
                .unwrap(),
            p
        );
    }
}
