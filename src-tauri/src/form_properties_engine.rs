use super::*;
impl PdfEngine {
    pub fn inspect_form_properties(
        &self,
        source_id: &str,
        page_index: usize,
        annotation_index: usize,
    ) -> EngineResult<crate::FormProperties> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::InspectFormProperties {
            source_id: source_id.into(),
            page_index,
            annotation_index,
            reply,
        })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }
    pub fn set_form_properties(
        &self,
        source_id: &str,
        request: crate::SetFormPropertiesRequest,
    ) -> EngineResult<DocumentInfo> {
        request.validate()?;
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::SetFormProperties {
            source_id: source_id.into(),
            request,
            reply,
        })?;
        self.document_in_points(receive.recv().map_err(|_| EngineError::WorkerStopped)?)
    }
}
impl WorkerRuntime {
    pub(super) fn inspect_form_properties(
        &self,
        source_id: &str,
        page_index: usize,
        annotation_index: usize,
    ) -> EngineResult<crate::FormProperties> {
        let source = self.editing_document(source_id)?;
        if source
            .policy
            .as_ref()
            .is_some_and(|p| !p.protection.permissions().annotate)
        {
            return Err(EngineError::PermissionDenied("editing field properties"));
        }
        crate::form_properties::inspect(source.bytes()?.as_ref(), page_index, annotation_index)
    }
    pub(super) fn set_form_properties(
        &mut self,
        source_id: &str,
        request: &crate::SetFormPropertiesRequest,
    ) -> EngineResult<DocumentInfo> {
        let source = self.editing_document(source_id)?;
        if source
            .policy
            .as_ref()
            .is_some_and(|p| !p.protection.permissions().annotate)
        {
            return Err(EngineError::PermissionDenied("editing field properties"));
        }
        let before = crate::form_fill::NativeForm::open(self.bindings, source.bytes()?.as_ref())?
            .field_bounded(request.page_index, request.annotation_index, 128 * 1024)?;
        let bytes = crate::form_properties::change(source.bytes()?.as_ref(), request)?;
        let after = crate::form_fill::NativeForm::open(self.bindings, &bytes)?.field_bounded(
            request.page_index,
            request.annotation_index,
            128 * 1024,
        )?;
        if before.value != after.value
            || before.options != after.options
            || before.selected_indices != after.selected_indices
            || before.kind != after.kind
        {
            return Err(crate::form_properties::bad(
                "saved field value or type changed",
            ));
        }
        let properties =
            crate::form_properties::inspect(&bytes, request.page_index, request.annotation_index)?;
        // NativeForm exposes a display label, falling back to the full name
        // when the actual alternate name is empty. Raw /TU was checked above.
        let display_label = if properties.label.is_empty() {
            &properties.full_name
        } else {
            &properties.label
        };
        if after.name != properties.full_name
            || &after.label != display_label
            || after.read_only != properties.read_only
            || after.required != properties.required
        {
            return Err(crate::form_properties::bad(
                "native field properties differ from the saved settings",
            ));
        }
        let bookmarks = source.source_bookmarks()?
            .clone();
        let policy = source.policy.clone();
        let original_path = source.original_path.clone();
        let legacy = source.legacy_comment_import;
        let path =
            std::env::temp_dir().join(format!("folio-field-properties-{}.pdf", Uuid::new_v4()));
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
