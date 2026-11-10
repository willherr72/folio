//! Evictable native handles; immutable source identity and ownership stay outside.
use super::*;
use std::cell::{Cell, OnceCell};
use std::time::Instant;

const IDLE_DOCUMENT_LIMIT: usize = 4;

pub(super) struct ResidentDocument {
    native: OnceCell<PdfDocument<'static>>,
    pdfium: &'static Pdfium,
    bytes: crate::source_storage::SourceHandle,
    pages: usize,
    used: Cell<Instant>,
}

impl ResidentDocument {
    pub(super) fn new(document: PdfDocument<'static>, pdfium: &'static Pdfium, bytes: crate::source_storage::SourceHandle) -> Self {
        let pages = document.pages().len() as usize;
        Self { native: OnceCell::from(document), pdfium, bytes, pages, used: Cell::new(Instant::now()) }
    }

    pub(super) fn evict(&mut self) { self.native.take(); }

    pub(super) fn page_count(&self) -> usize { self.pages }

    // Only the returned native reference borrows self. The removal recipe is
    // needed while loading, not for the returned lifetime, permitting imports
    // to update disjoint OpenDocument fields before the first request completes.
    pub(super) fn get(&self, removed: &HashMap<usize, HashSet<usize>>) -> EngineResult<&PdfDocument<'static>> {
        if self.native.get().is_none() {
            let document = self.pdfium.load_pdf_from_reader(Cursor::new(self.bytes.lease()?), None)?;
            if document.pages().len() as usize != self.pages {
                return Err(EngineError::InvalidRequest("The retained PDF page count changed during reopening.".into()));
            }
            let mut pages = removed.iter().collect::<Vec<_>>();
            pages.sort_unstable_by_key(|(page, _)| **page);
            for (page_index, indices) in pages {
                let mut page = document.pages().get(page_index_i32(*page_index)?)?;
                let annotations = page.annotations_mut();
                let mut indices = indices.iter().copied().collect::<Vec<_>>();
                indices.sort_unstable();
                if indices.last().is_some_and(|index| *index >= annotations.len() as usize) {
                    return Err(EngineError::InvalidRequest("Imported PDF annotations could not be restored during reopening.".into()));
                }
                for index in indices.into_iter().rev() {
                    let annotation = annotations.get(index)?;
                    annotations.delete_annotation(annotation)?;
                }
            }
            // A failed load/replay drops the temporary document. No incomplete
            // handle or permanent failure is published; a later call can retry.
            self.native.set(document).map_err(|_| EngineError::InvalidRequest("The retained PDF was reopened concurrently.".into()))?;
        }
        self.bytes.touch();
        self.used.set(Instant::now());
        self.native.get().ok_or_else(|| EngineError::InvalidRequest("The retained PDF could not be reopened.".into()))
    }
}

impl OpenDocument {
    pub(super) fn bytes(&self) -> EngineResult<Arc<[u8]>> { Ok(self.source_bytes.lease()?) }
    pub(super) fn source_bookmarks(&self) -> EngineResult<&Result<Vec<crate::DocumentBookmark>,String>> {
        if self.bookmarks.get().is_none() {
            let bytes = self.bytes()?;
            let _ = self.bookmarks.set(crate::bookmarks::read(&bytes).map_err(|e|e.to_string()));
        }
        Ok(self.bookmarks.get().expect("initialized source bookmarks"))
    }
    pub(super) fn pdf(&self) -> EngineResult<&PdfDocument<'static>> {
        self.document.get(&self.imported_annotations)
    }
}

impl WorkerRuntime {
    pub(super) fn trim_native_documents(&mut self) {
        let mut resident = self.documents.iter()
            .filter(|(_, source)| source.document.native.get().is_some())
            .map(|(id, source)| (source.document.used.get(), id.clone()))
            .collect::<Vec<_>>();

        resident.sort_unstable();
        let evict = resident.len().saturating_sub(IDLE_DOCUMENT_LIMIT);
        for (_, id) in resident.into_iter().take(evict) {
            if let Some(source) = self.documents.get_mut(&id) {
                source.document.evict();
            }
        }
        let documents = &mut self.documents;
        self.source_store.trim_raw_cache(|identity| {
            for source in documents.values_mut() {
                if source.source_bytes.identity().same_source(identity) { source.document.evict(); }
            }
        });
    }
}
