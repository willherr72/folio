use super::*;
impl PdfEngine {
    pub fn inspect_document_properties(
        &self,
        source_id: &str,
    ) -> EngineResult<crate::DocumentProperties> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::InspectDocumentProperties {
            source_id: source_id.into(),
            reply,
        })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }
    pub fn export_pdf_with_metadata(
        &self,
        request: ExportRequest,
        bookmarks: Option<Vec<crate::DocumentBookmark>>,
        metadata: crate::DocumentMetadataEdit,
        destination: impl AsRef<Path>,
    ) -> EngineResult<()> {
        self.export_pdf_with_options(
            request,
            bookmarks,
            destination,
            None,
            Some(metadata),
            Arc::new(crate::PdfSaveControl::new()),
            Arc::new(|_| {}),
        )
        .map(|_| ())
    }
}
impl WorkerRuntime {
    pub(super) fn inspect_document_properties(
        &self,
        source_id: &str,
    ) -> EngineResult<crate::DocumentProperties> {
        let source = self.document(source_id)?;
        let (document, _) = lopdf::Document::load_mem_with_resource_control(
            source.bytes()?.as_ref(),
            lopdf::ResourceLimits::default(),
            Arc::new(|| false),
        )
        .map_err(crate::document_properties::bad)?;
        crate::document_properties::inspect(&document)
    }
    pub(super) fn authorize_metadata_edit(&self, request: &ExportRequest) -> EngineResult<()> {
        let mut seen = HashSet::new();
        for page in &request.pages {
            if seen.insert(page.source_id.as_str()) {
                let source = self.document(&page.source_id)?;
                if source.for_printing
                    || source
                        .policy
                        .as_ref()
                        .is_some_and(|p| !p.protection.permissions().edit_content)
                {
                    return Err(EngineError::PermissionDenied(
                        "changing document properties",
                    ));
                }
            }
        }
        Ok(())
    }
}
