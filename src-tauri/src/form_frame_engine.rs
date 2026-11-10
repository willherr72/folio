use super::*;
use crate::{FormFrameAppearance, FormFrameChanges, FrameColorChange, SetFormFrameRequest};
fn bad(message: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("Form frame: {message}"))
}
fn selection(occurrences: &[usize], selected: usize) -> EngineResult<()> {
    if occurrences.is_empty() || occurrences.len() > 100000 || selected >= occurrences.len() {
        Err(bad("invalid occurrence selection"))
    } else {
        Ok(())
    }
}
impl PdfEngine {
    pub fn inspect_form_frame(
        &self,
        source_id: &str,
        page_index: usize,
        annotation_index: usize,
    ) -> EngineResult<FormFrameAppearance> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::InspectFormFrame {
            source_id: source_id.into(),
            page_index,
            annotation_index,
            reply,
        })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }
    pub fn set_form_frame_occurrence(
        &self,
        source_id: &str,
        occurrences: &[usize],
        selected: usize,
        request: SetFormFrameRequest,
    ) -> EngineResult<PageContentReplacement> {
        request.validate()?;
        selection(occurrences, selected)?;
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::SetFormFrame {
            source_id: source_id.into(),
            occurrences: occurrences.to_vec(),
            selected,
            request,
            reply,
        })?;
        let mut result = receive.recv().map_err(|_| EngineError::WorkerStopped)??;
        result.document = self.document_in_points(Ok(result.document))?;
        Ok(result)
    }
}
impl WorkerRuntime {
    pub(super) fn inspect_form_frame(
        &self,
        source_id: &str,
        page_index: usize,
        annotation_index: usize,
    ) -> EngineResult<FormFrameAppearance> {
        let source = self.editing_document(source_id)?;
        if source
            .policy
            .as_ref()
            .is_some_and(|p| !p.protection.permissions().annotate)
        {
            return Err(EngineError::PermissionDenied(
                "editing field borders and backgrounds",
            ));
        }
        let unit = self.source_page_unit(source_id, page_index)?;
        let mut info =
            crate::form_frame::inspect(source.bytes()?.as_ref(), page_index, annotation_index)?;
        info.width *= unit;
        if (info.width - 32.).abs() <= 32. * f32::EPSILON * 2. {
            info.width = 32.;
        }
        if !info.width.is_finite() {
            return Err(bad("border width exceeds physical coordinates"));
        }
        Ok(info)
    }
    pub(super) fn set_form_frame_occurrence(
        &mut self,
        source_id: &str,
        occurrences: &[usize],
        selected: usize,
        request: &SetFormFrameRequest,
    ) -> EngineResult<PageContentReplacement> {
        request.validate()?;
        selection(occurrences, selected)?;
        let source = self.editing_document(source_id)?;
        if source
            .policy
            .as_ref()
            .is_some_and(|p| !p.protection.permissions().annotate)
        {
            return Err(EngineError::PermissionDenied(
                "editing field borders and backgrounds",
            ));
        }
        let count = source.document.page_count() as usize;
        if occurrences.iter().any(|p| *p >= count) {
            return Err(bad("page outside source"));
        }
        let page_index = occurrences[selected];
        let index = request.annotation_index;
        let unit = self.source_page_unit(source_id, page_index)?;
        // Bind the request to the original complete source before any page is copied.
        let style = crate::form_frame::inspect(source.bytes()?.as_ref(), page_index, index)?;
        if style.style_key != request.expected_style_key {
            return Err(bad("field settings changed; reopen border controls"));
        }
        let original = crate::form_fill::NativeForm::open(self.bindings, source.bytes()?.as_ref())?
            .field_bounded(page_index, index, 128 * 1024)?;
        if !matches!(
            original.kind.as_str(),
            "text" | "combo" | "list" | "checkbox" | "radio"
        ) || !original.visible
            || original.no_rotate
        {
            return Err(bad("this field cannot be restyled"));
        }
        let geometry = page_geometry(&source.pdf()?.pages().get(page_index_i32(page_index)?)?)?;
        let raw = original
            .bounds
            .as_ref()
            .ok_or_else(|| bad("field has no bounds"))?;
        let a = geometry.pdf_to_displayed(raw.x, raw.y + raw.height);
        let b = geometry.pdf_to_displayed(raw.x + raw.width, raw.y);
        let expected = AnnotationRect {
            x: a.x.min(b.x) * unit,
            y: a.y.min(b.y) * unit,
            width: (a.x - b.x).abs() * unit,
            height: (a.y - b.y).abs() * unit,
        };
        let actual = &request.expected_bounds;
        if [
            expected.x - actual.x,
            expected.y - actual.y,
            expected.width - actual.width,
            expected.height - actual.height,
        ]
        .iter()
        .any(|v| v.abs() > 0.01)
            || original.value != crate::form_fill::normalized(&request.expected_value)
        {
            return Err(bad("field value or bounds changed; reopen border controls"));
        }
        let repeated = occurrences.iter().filter(|p| **p == page_index).count() > 1;
        let policy = if repeated {
            source
                .policy
                .as_ref()
                .map(|p| p.duplicated_page(page_index))
                .transpose()?
        } else {
            source.policy.clone()
        };
        let first = occurrences.iter().position(|p| *p == page_index).unwrap();
        let target = if repeated && first != selected {
            count
        } else {
            page_index
        };
        let page_indices = occurrences
            .iter()
            .enumerate()
            .map(|(i, p)| {
                if repeated && *p == page_index && (i == selected) != (first == selected) {
                    count
                } else {
                    *p
                }
            })
            .collect();
        let expanded;
        let original_bytes;
        let bytes: &[u8] = if repeated {
            expanded = self.append_page_copy(source, page_index)?;
            &expanded
        } else {
            original_bytes=source.bytes()?;
            &original_bytes
        };
        let before = crate::form_fill::NativeForm::open(self.bindings, bytes)?.field_bounded(
            target,
            index,
            128 * 1024,
        )?;
        let mut isolated = crate::form_frame::inspect(bytes, target, index)?;
        isolated.style_key = style.style_key.clone();
        if serde_json::to_value(&isolated).map_err(bad)?
            != serde_json::to_value(&style).map_err(bad)?
            || serde_json::to_value(&before).map_err(bad)?
                != serde_json::to_value(&original).map_err(bad)?
        {
            return Err(bad(
                "copying the selected page changed its field appearance or definition",
            ));
        }
        let changes = FormFrameChanges {
            // Preserve the exact raw width when a physical display value was re-entered.
            width: request
                .changes
                .width
                .filter(|w| *w != style.width * unit)
                .map(|w| w / unit),
            style: request.changes.style,
            border: request.changes.border.clone(),
            background: request.changes.background.clone(),
        };
        if changes.width.is_some_and(|w| !w.is_finite()) {
            return Err(bad("border width exceeds source coordinates"));
        }
        let bytes = crate::form_frame::change(bytes, target, index, &changes)?;
        let after = crate::form_fill::NativeForm::open(self.bindings, &bytes)?.field_bounded(
            target,
            index,
            128 * 1024,
        )?;
        if serde_json::to_value(&before).map_err(bad)?
            != serde_json::to_value(&after).map_err(bad)?
        {
            return Err(bad(
                "frame edit changed the field answer, definition or placement",
            ));
        }
        let confirmed = crate::form_frame::inspect(&bytes, target, index)?;
        let matches_color =
            |request: &Option<FrameColorChange>, actual: &crate::FormFrameColor| match request {
                None => true,
                Some(FrameColorChange::Transparent) => actual.color_space == "transparent",
                Some(FrameColorChange::Rgb(rgb)) => actual
                    .preview
                    .as_ref()
                    .is_some_and(|c| c.eq_ignore_ascii_case(rgb)),
            };
        if request
            .changes
            .width
            .is_some_and(|w| (confirmed.width * unit - w).abs() > w.max(1.) * 0.00001)
            || request
                .changes
                .style
                .is_some_and(|s| confirmed.style != Some(s))
            || !matches_color(&request.changes.border, &confirmed.border)
            || !matches_color(&request.changes.background, &confirmed.background)
        {
            return Err(bad("saved frame settings differ from the preview request"));
        }
        let bookmarks = source.source_bookmarks()?
            .clone();
        let path = std::env::temp_dir().join(format!("folio-field-frame-{}.pdf", Uuid::new_v4()));
        let original_path = source.original_path.clone();
        let legacy = source.legacy_comment_import;
        let mut document = self.open_bytes(
            bytes,
            path,
            original_path,
            true,
            true,
            legacy,
            OpenSecurity::Restored(policy),
        )?;
        document = self.retain_annotation_sources_with_copy(source_id, document, repeated.then_some(page_index))?;
        document.name = "Form border and background.pdf".into();
        document.bookmarks = bookmarks.as_ref().cloned().unwrap_or_default();
        document.bookmark_error = bookmarks.as_ref().err().cloned();
        self.documents.get_mut(&document.id).unwrap().bookmarks = OnceLock::from(bookmarks);
        Ok(PageContentReplacement {
            document,
            page_indices,
        })
    }
}
