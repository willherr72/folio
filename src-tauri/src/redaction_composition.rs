//! Owned Save-composed snapshots; strict removal admission remains separate.
use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TextRedactionSource {
    pub document: DocumentInfo,
    pub omitted_metadata: Vec<&'static str>,
}
pub(super) struct PreparedSource {
    pub(super) owner: String,
    pub(super) allocation: crate::source_storage::SourceIdentity,
    pub(super) inputs: Vec<(String, crate::source_storage::SourceIdentity)>,
    pub(super) created: Instant,
    pub(super) omitted_metadata: Vec<&'static str>,
}
fn checked_request(request: &ExportRequest) -> EngineResult<()> {
    require(!request.flatten && (1..=1000).contains(&request.pages.len()))?;
    let mut total = 0usize;
    for page in &request.pages {
        total = total.checked_add(page.overlays.len()).ok_or_else(invalid)?;
        require(total <= 10_000)?;
    }
    Ok(())
}
impl PdfEngine {
    pub(crate) fn prepare_text_redaction_source(
        &self,
        owner: &str,
        request: ExportRequest,
        bookmarks: Option<Vec<crate::DocumentBookmark>>,
    ) -> EngineResult<TextRedactionSource> {
        checked_id(owner)?;
        checked_request(&request)?;
        let resources = self.capture_export_resources(&request.pages)?;
        let request = self.export_in_raw(request)?;
        let prepared = self.text_redaction_request(|reply| Request::Prepare {
            owner: owner.into(),
            request,
            bookmarks,
            resources,
            reply,
        })?;
        let document = self.document_in_points(Ok(prepared.document))?;
        Ok(TextRedactionSource {
            document,
            omitted_metadata: prepared.omitted_metadata,
        })
    }
}
impl WorkerRuntime {
    pub(super) fn retained_redaction_bytes(&self) -> usize {
        self.redaction_previews
            .pending
            .values()
            .map(|p| p.allocation.len())
            .sum::<usize>()
            + self
                .redaction_previews
                .prepared
                .values()
                .map(|p| p.allocation.len())
                .sum::<usize>()
    }
    pub(super) fn prepare_redaction_source(
        &mut self,
        owner: &str,
        request: ExportRequest,
        bookmarks: Option<Vec<crate::DocumentBookmark>>,
    ) -> EngineResult<TextRedactionSource> {
        checked_id(owner)?;
        checked_request(&request)?;
        self.prune_redaction_previews();
        require(self.redaction_previews.owners.contains(owner))?;
        require(
            self.redaction_previews.prepared.len() < 4
                && self.retained_redaction_bytes() < self.redaction_previews.byte_limit,
        )?;
        self.validate_export(&request)?;
        let mut inputs = vec![];
        let mut seen = HashSet::new();
        let mut omitted = BTreeSet::new();
        for plan in &request.pages {
            if !seen.insert(plan.source_id.clone()) {
                continue;
            }
            require(
                !self
                    .redaction_previews
                    .prepared
                    .contains_key(&plan.source_id)
                    && !self
                        .redaction_previews
                        .pending
                        .values()
                        .any(|p| p.candidate.id == plan.source_id),
            )?;
            let source = self.redaction_source(&plan.source_id)?;
            let parsed = crate::redaction_numbers::load_pdf(source.bytes()?.as_ref())
            .map_err(|_| {
                EngineError::InvalidRequest(
                    "Text-removal input could not be loaded within the resource limits.".into(),
                )
            })?;
            let catalog = parsed.catalog().map_err(|_| invalid())?;
            if catalog.iter().any(|(key, _)| {
                !matches!(
                    key.as_slice(),
                    b"Type" | b"Pages" | b"AcroForm" | b"Outlines"
                )
            }) {
                omitted.insert(
                    "catalog metadata, navigation settings and attachments (Save composition)",
                );
            }
            if parsed
                .trailer
                .iter()
                .any(|(key, _)| !matches!(key.as_slice(), b"Root" | b"Size" | b"Prev" | b"XRefStm"))
            {
                omitted.insert("document information and identifiers (Save composition)");
            }
            inputs.push((plan.source_id.clone(), source.source_bytes.identity().clone()));
        }
        let original_path = self.document(&inputs[0].0)?.original_path.clone();
        let (bytes, retained) =
            self.compose_pdf_with_route_metadata(request, bookmarks, CompositionIntent::Save, None, None, false)?;
        require(
            bytes.len() <= 64 * 1024 * 1024
                && self
                    .retained_redaction_bytes()
                    .checked_add(bytes.len())
                    .is_some_and(|n| n <= self.redaction_previews.byte_limit),
        )?;
        let bookmarks = crate::bookmarks::read(&bytes)?;
        let mut document =
            self.register_generated_document(bytes, "Text removal working copy.pdf")?;
        document.bookmarks = bookmarks;
        let source = self
            .documents
            .get_mut(&document.id)
            .expect("registered working copy");
        source.original_path = original_path;
        let allocation = source.source_bytes.identity().clone();
        let omitted_metadata = if retained {
            vec![]
        } else {
            omitted.into_iter().collect::<Vec<_>>()
        };
        self.redaction_previews.prepared.insert(
            document.id.clone(),
            PreparedSource {
                owner: owner.into(),
                allocation,
                inputs,
                created: Instant::now(),
                omitted_metadata: omitted_metadata.clone(),
            },
        );
        Ok(TextRedactionSource {
            document,
            omitted_metadata,
        })
    }
    pub(super) fn validate_prepared_redaction_source(
        &self,
        owner: &str,
        id: &str,
    ) -> EngineResult<()> {
        if let Some(p) = self.redaction_previews.prepared.get(id) {
            require(p.owner == owner && p.created.elapsed() < TTL)?;
            require(self.redaction_source(id)?.source_bytes.identity().same_source(&p.allocation))?;
            for (input, allocation) in &p.inputs {
                require(self.redaction_source(input)?.source_bytes.identity().same_source(allocation))?;
            }
        }
        Ok(())
    }
    pub(super) fn discard_redaction_prepared(&mut self, id: &str) {
        if let Some(p) = self.redaction_previews.prepared.remove(id) {
            // Invalidate dependents even if a corrupted ID now names other bytes.
            self.detach_redaction_document(id);
            if self
                .documents
                .get(id)
                .is_some_and(|d| d.source_bytes.identity().same_source(&p.allocation))
            {
                self.discard_unpublished_source_unhooked(id);
            }
        }
    }
    pub(super) fn prune_redaction_prepared(&mut self) {
        let ids = self
            .redaction_previews
            .prepared
            .iter()
            .filter(|(_, p)| p.created.elapsed() >= TTL)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in ids {
            self.discard_redaction_prepared(&id);
        }
    }
    pub(super) fn detach_redaction_prepared_input(&mut self, id: &str) {
        let ids = self
            .redaction_previews
            .prepared
            .iter()
            .filter(|(key, p)| key.as_str() == id || p.inputs.iter().any(|(input, _)| input == id))
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for key in ids {
            if key == id {
                self.redaction_previews.prepared.remove(id);
            } else {
                self.discard_redaction_prepared(&key);
            }
        }
    }
    pub(super) fn release_redaction_prepared(&mut self, owner: &str) {
        let ids = self
            .redaction_previews
            .prepared
            .iter()
            .filter(|(_, p)| p.owner == owner)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in ids {
            self.discard_redaction_prepared(&id);
        }
    }
}

#[cfg(test)]
#[path = "redaction_composition_tests.rs"]
mod tests;
