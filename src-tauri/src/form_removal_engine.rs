use super::*;
impl PdfEngine {
    pub fn inspect_form_removal(
        &self,
        source_id: &str,
        page_index: usize,
        annotation_index: usize,
    ) -> EngineResult<crate::FormRemovalInfo> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::InspectFormRemoval {
            source_id: source_id.into(),
            page_index,
            annotation_index,
            reply,
        })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }
    pub fn remove_form_field(
        &self,
        source_id: &str,
        request: crate::RemoveFormFieldRequest,
    ) -> EngineResult<DocumentInfo> {
        request.validate()?;
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::RemoveFormField {
            source_id: source_id.into(),
            request,
            reply,
        })?;
        self.document_in_points(receive.recv().map_err(|_| EngineError::WorkerStopped)?)
    }
}
impl WorkerRuntime {
    pub(super) fn inspect_form_removal(
        &self,
        source_id: &str,
        page_index: usize,
        annotation_index: usize,
    ) -> EngineResult<crate::FormRemovalInfo> {
        let source = self.editing_document(source_id)?;
        if source
            .policy
            .as_ref()
            .is_some_and(|p| !p.protection.permissions().annotate)
        {
            return Err(EngineError::PermissionDenied("removing form fields"));
        }
        crate::form_removal::inspect(source.bytes()?.as_ref(), page_index, annotation_index)
    }
    pub(super) fn remove_form_field(
        &mut self,
        source_id: &str,
        request: &crate::RemoveFormFieldRequest,
    ) -> EngineResult<DocumentInfo> {
        let source = self.editing_document(source_id)?;
        if source
            .policy
            .as_ref()
            .is_some_and(|p| !p.protection.permissions().annotate)
        {
            return Err(EngineError::PermissionDenied("removing form fields"));
        }
        let publication = crate::form_removal::change(source.bytes()?.as_ref(), request)?;
        let bookmarks = source.source_bookmarks()?
            .clone();
        let policy = source.policy.clone();
        let original_path = source.original_path.clone();
        let legacy = source.legacy_comment_import;
        let path = std::env::temp_dir().join(format!("folio-field-removal-{}.pdf", Uuid::new_v4()));
        let mut document = self.open_bytes(
            publication.bytes,
            path,
            original_path,
            true,
            true,
            legacy,
            OpenSecurity::Restored(policy),
        )?;
        document = self.retain_annotation_sources_after_widget_removal(source_id, document, &publication.removed_widgets)?;
        document.name = "Updated form.pdf".into();
        document.bookmarks = bookmarks.as_ref().cloned().unwrap_or_default();
        document.bookmark_error = bookmarks.as_ref().err().cloned();
        self.documents.get_mut(&document.id).unwrap().bookmarks = OnceLock::from(bookmarks);
        Ok(document)
    }
}
