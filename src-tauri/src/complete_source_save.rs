//! Retain the immutable source graph when no page import or overlay replay is needed.
use super::*;

pub(super) fn same_bookmarks(a: &[crate::DocumentBookmark], b: &[crate::DocumentBookmark]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| {
        a.title==b.title && a.level==b.level && a.destination==b.destination && a.action==b.action
            && a.bold==b.bold && a.italic==b.italic && a.closed==b.closed && a.color==b.color
    })
}

impl WorkerRuntime {
    // Called only after normal request validation and composition authorization.
    pub(super) fn complete_source_save(
        &self,
        request: &ExportRequest,
        bookmarks: &[crate::DocumentBookmark],
        monitor: Option<&crate::pdf_save::PdfSaveMonitor>,
    ) -> EngineResult<Option<Vec<u8>>> {
        let Some(first) = request.pages.first() else { return Ok(None); };
        let source = self.document(&first.source_id)?;
        if request.pages.len() != source.document.page_count() as usize
            || source.imported_annotations.values().any(|indices| !indices.is_empty())
            || request.pages.iter().enumerate().any(|(index, page)| {
                page.source_id != first.source_id || page.page_index != index || !page.overlays.is_empty()
            })
        { return Ok(None); }

        let check = |completed| monitor.map_or(Ok(()), |m| m.checkpoint("preserving", completed));
        check(0)?;
        // Compare to this source, not the protected origin's historical outlines.
        // A generated source can have changed its outline graph in a prior edit.
        let original = source.source_bookmarks()?;
        let mapping = request.pages.iter().enumerate()
            .map(|(index, page)| (index.to_string(), page.id.clone())).collect();
        // An unreadable tree reaches this point only with an explicit, validated
        // override: inherited_bookmarks rejects it before authorization otherwise.
        // Honor that deliberate discard/replacement instead of retaining the error.
        let change_bookmarks = match original {
            Ok(original) => !same_bookmarks(&crate::bookmarks::remap(original, &mapping), bookmarks),
            Err(_) => true,
        };
        if !change_bookmarks && request.pages.iter().all(|page| page.rotation == 0 && page.crop.is_none()) {
            // Avoid parsing/reserializing an unchanged graph, including signatures
            // and producer metadata. Protection is still applied by the caller.
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(source.source_bytes.identity().len())
                .map_err(|_| EngineError::InvalidRequest("PDF output memory allocation failed".into()))?;
            for chunk in source.bytes()?.chunks(256 * 1024) {
                if let Some(monitor) = monitor { monitor.check_cancelled()?; }
                bytes.extend_from_slice(chunk);
            }
            check(request.pages.len())?;
            return Ok(Some(bytes));
        }

        let mut document = crate::pdf_output::load(source.bytes()?.as_ref())?;
        let pages = document.get_pages();
        if pages.len() != request.pages.len() {
            return Err(EngineError::InvalidRequest("Source page mapping changed during saving.".into()));
        }
        // Some page trees repeat the same leaf dictionary. Mutating it in place
        // would apply one occurrence's crop/rotation to another occurrence too.
        if request.pages.iter().any(|page| page.rotation != 0 || page.crop.is_some()) {
            let mut unique = HashSet::new();
            if pages.values().any(|id| !unique.insert(*id)) { return Ok(None); }
        }
        let mut geometries = Vec::with_capacity(pages.len());
        for ((index, plan), id) in request.pages.iter().enumerate().zip(pages.values()) {
            check(index)?;
            let geometry = page_geometry(&source.pdf()?.pages().get(page_index_i32(index)?)?)?;
            if plan.rotation != 0 {
                document.get_dictionary_mut(*id)
                    .map_err(|error| EngineError::InvalidRequest(error.to_string()))?
                    .set("Rotate", i64::from((geometry.rotation + plan.rotation) % 360));
            }
            geometries.push(geometry);
        }
        page_crop::apply(&mut document, &request.pages, &geometries)?;
        if change_bookmarks {
            crate::bookmarks::write_document(&mut document, bookmarks,
                &request.pages.iter().map(|page| page.id.clone()).collect::<Vec<_>>())?;
        }
        check(request.pages.len())?;
        let bytes = crate::pdf_output::save_controlled(&mut document, monitor.map(|m| m.control.as_ref()))?;
        if let Some(monitor) = monitor { monitor.check_cancelled()?; }
        Ok(Some(bytes))
    }
}

pub(super) fn requires_graph_composition(
    sources: &[crate::acroforms::SourceForm<'_>],
    mut check: impl FnMut() -> EngineResult<()>,
) -> EngineResult<bool> {
    for source in sources {
        check()?;
        let document = source.document()?;
        check()?;
        let catalog = document.catalog().map_err(|error| EngineError::InvalidRequest(error.to_string()))?;
        if [b"StructTreeRoot".as_slice(), b"MarkInfo", b"OCProperties", b"OutputIntents"]
            .iter().any(|key| catalog.has(key))
        {
            return Ok(true);
        }
    }
    Ok(false)
}
