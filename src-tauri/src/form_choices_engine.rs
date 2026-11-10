use super::*;

impl PdfEngine {
    pub fn inspect_form_choices(
        &self,
        source_id: &str,
        page_index: usize,
        annotation_index: usize,
    ) -> EngineResult<crate::FormChoices> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::InspectFormChoices {
            source_id: source_id.into(),
            page_index,
            annotation_index,
            reply,
        })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }
    pub fn set_form_choices(
        &self,
        source_id: &str,
        request: crate::SetFormChoicesRequest,
    ) -> EngineResult<DocumentInfo> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::SetFormChoices {
            source_id: source_id.into(),
            request,
            reply,
        })?;
        self.document_in_points(receive.recv().map_err(|_| EngineError::WorkerStopped)?)
    }
}
impl WorkerRuntime {
    pub(super) fn inspect_form_choices(
        &self,
        source_id: &str,
        page_index: usize,
        annotation_index: usize,
    ) -> EngineResult<crate::FormChoices> {
        let source = self.editing_document(source_id)?;
        if source
            .policy
            .as_ref()
            .is_some_and(|p| !p.protection.permissions().annotate)
        {
            return Err(EngineError::PermissionDenied("editing field choices"));
        }
        crate::inspect_choice_pdf(source.bytes()?.as_ref(), page_index, annotation_index)
    }
    pub(super) fn set_form_choices(
        &mut self,
        source_id: &str,
        request: &crate::SetFormChoicesRequest,
    ) -> EngineResult<DocumentInfo> {
        let source = self.editing_document(source_id)?;
        if source
            .policy
            .as_ref()
            .is_some_and(|p| !p.protection.permissions().annotate)
        {
            return Err(EngineError::PermissionDenied("editing field choices"));
        }
        // The transaction owns a private complete-source graph. Register nothing
        // until all linked appearances and native values have been verified.
        let bytes = crate::form_choice_publish::change(
            self.pdfium,
            self.bindings,
            source.bytes()?.as_ref(),
            request,
        )?;
        let bookmarks = source.source_bookmarks()?
            .clone();
        let policy = source.policy.clone();
        let original_path = source.original_path.clone();
        let legacy = source.legacy_comment_import;
        let path = std::env::temp_dir().join(format!("folio-field-choices-{}.pdf", Uuid::new_v4()));
        let mut document = self.open_bytes(
            bytes,
            path,
            original_path,
            true,
            true,
            legacy,
            OpenSecurity::Restored(policy),
        )?;
        document = self.retain_annotation_sources(source_id, document)?;
        document.name = "Updated form.pdf".into();
        document.bookmarks = bookmarks.as_ref().cloned().unwrap_or_default();
        document.bookmark_error = bookmarks.as_ref().err().cloned();
        self.documents.get_mut(&document.id).unwrap().bookmarks = OnceLock::from(bookmarks);
        Ok(document)
    }
}
