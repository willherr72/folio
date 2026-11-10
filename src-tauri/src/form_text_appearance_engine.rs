use super::*;
use crate::form_text_appearance::{
    bad, FormTextAppearance, SetFormTextAppearanceRequest, TextFieldAppearanceChanges,
};

impl PdfEngine {
    pub fn inspect_form_text_appearance(
        &self,
        source_id: &str,
        page_index: usize,
        annotation_index: usize,
    ) -> EngineResult<FormTextAppearance> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::InspectFormTextAppearance {
            source_id: source_id.into(),
            page_index,
            annotation_index,
            reply,
        })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }
    pub fn set_form_text_appearance_occurrence(
        &self,
        source_id: &str,
        occurrences: &[usize],
        selected: usize,
        request: SetFormTextAppearanceRequest,
    ) -> EngineResult<PageContentReplacement> {
        request.validate()?;
        selection(occurrences, selected)?;
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::SetFormTextAppearance {
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
fn selection(occurrences: &[usize], selected: usize) -> EngineResult<()> {
    if occurrences.is_empty() || occurrences.len() > 100000 || selected >= occurrences.len() {
        Err(bad("invalid occurrence selection"))
    } else {
        Ok(())
    }
}
fn same(a: &AnnotationRect, b: &AnnotationRect) -> bool {
    [a.x - b.x, a.y - b.y, a.width - b.width, a.height - b.height]
        .iter()
        .all(|v| v.abs() <= 0.01)
}
impl WorkerRuntime {
    pub(super) fn inspect_form_text_appearance(
        &self,
        source_id: &str,
        page_index: usize,
        annotation_index: usize,
    ) -> EngineResult<FormTextAppearance> {
        let source = self.editing_document(source_id)?;
        if source
            .policy
            .as_ref()
            .is_some_and(|p| !p.protection.permissions().annotate)
        {
            return Err(EngineError::PermissionDenied("editing field appearance"));
        }
        let unit = self.source_page_unit(source_id, page_index)?;
        let mut style = crate::form_text_appearance::inspect(
            self.pdfium,
            source.bytes()?.as_ref(),
            page_index,
            annotation_index,
        )?;
        style.font_size *= unit;
        for endpoint in [1., 512.] {
            if (style.font_size - endpoint).abs() <= endpoint * f32::EPSILON * 2. {
                style.font_size = endpoint;
            }
        }
        if !style.font_size.is_finite() {
            return Err(bad("font size exceeds physical coordinates"));
        }
        Ok(style)
    }
    pub(super) fn set_form_text_appearance_occurrence(
        &mut self,
        source_id: &str,
        occurrences: &[usize],
        selected: usize,
        request: &SetFormTextAppearanceRequest,
    ) -> EngineResult<PageContentReplacement> {
        request.validate()?;
        selection(occurrences, selected)?;
        let source = self.editing_document(source_id)?;
        if source
            .policy
            .as_ref()
            .is_some_and(|p| !p.protection.permissions().annotate)
        {
            return Err(EngineError::PermissionDenied("editing field appearance"));
        }
        let count = source.document.page_count() as usize;
        if occurrences.iter().any(|p| *p >= count) {
            return Err(bad("page outside source"));
        }
        let page_index = occurrences[selected];
        let unit = self.source_page_unit(source_id, page_index)?;
        let style =
            self.inspect_form_text_appearance(source_id, page_index, request.annotation_index)?;
        if style.style_key != request.expected_style_key {
            return Err(bad("field settings changed; reopen appearance controls"));
        }
        let original = crate::form_fill::NativeForm::open(self.bindings, source.bytes()?.as_ref())?
            .field_bounded(page_index, request.annotation_index, 128 * 1024)?;
        if original.kind != "text" || !original.visible || original.no_rotate {
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
        if !same(&expected, &request.expected_bounds)
            || original.value != crate::form_fill::normalized(&request.expected_value)
        {
            return Err(bad(
                "field value or bounds changed; reopen appearance controls",
            ));
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
            request.annotation_index,
            128 * 1024,
        )?;
        let changes = TextFieldAppearanceChanges {
            font_size: request.changes.font_size.map(|n| n / unit),
            color: request.changes.color.clone(),
            alignment: request.changes.alignment,
        };
        if changes.font_size.is_some_and(|n| !n.is_finite()) {
            return Err(bad("font size exceeds source coordinates"));
        }
        let bytes = crate::form_text_appearance::change(
            self.bindings,
            bytes,
            target,
            request.annotation_index,
            &before.value,
            &changes,
        )?;
        let after = crate::form_fill::NativeForm::open(self.bindings, &bytes)?.field_bounded(
            target,
            request.annotation_index,
            128 * 1024,
        )?;
        if serde_json::to_value(&before).map_err(bad)?
            != serde_json::to_value(&after).map_err(bad)?
        {
            return Err(bad(
                "appearance changed field definition, geometry or value",
            ));
        }
        let confirmed = crate::form_text_appearance::inspect(
            self.pdfium,
            &bytes,
            target,
            request.annotation_index,
        )?;
        if changes
            .font_size
            .is_some_and(|size| (confirmed.font_size - size).abs() > size.max(1.) * 0.00001)
            || changes.alignment.is_some_and(|q| confirmed.alignment != q)
            || changes
                .color
                .as_ref()
                .is_some_and(|c| !confirmed.color.eq_ignore_ascii_case(c))
        {
            return Err(bad("saved text settings differ from the preview request"));
        }
        let bookmarks = source.source_bookmarks()?
            .clone();
        let path = std::env::temp_dir().join(format!("folio-styled-field-{}.pdf", Uuid::new_v4()));
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
        document.name = "Styled form field.pdf".into();
        document.bookmarks = bookmarks.as_ref().cloned().unwrap_or_default();
        document.bookmark_error = bookmarks.as_ref().err().cloned();
        self.documents.get_mut(&document.id).unwrap().bookmarks = OnceLock::from(bookmarks);
        Ok(PageContentReplacement {
            document,
            page_indices,
        })
    }
}
