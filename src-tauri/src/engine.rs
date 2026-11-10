use crate::fonts::{FontAsset, FontRegistry};
use crate::types::*;
use image::ImageFormat;
use pdfium_render::prelude::*;
#[path = "reader_order.rs"]
mod reader_order;
use reader_order::simple_forward_text;
use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, OnceLock};
use uuid::Uuid;

#[path = "native_residency.rs"]
mod native_residency;

#[cfg(test)]
#[path = "native_residency_tests.rs"]
mod native_residency_tests;

#[cfg(test)]
#[path = "retained_history_workload.rs"]
mod retained_history_workload;

#[path = "form_admission.rs"]
mod form_admission;
#[cfg(test)]
#[path = "form_admission_tests.rs"]
mod form_admission_tests;

#[cfg(test)]
#[path = "engine_image_tests.rs"]
mod image_owner_tests;
#[cfg(test)]
#[path = "engine_choice_probe_tests.rs"]
mod choice_probe_tests;
#[cfg(test)]
#[path = "redaction_native_tests.rs"]
mod redaction_native_tests;
#[cfg(test)]
#[path = "engine_choice_publication_tests.rs"]
mod choice_publication_tests;
#[cfg(test)]
#[path = "engine_choice_transaction_tests.rs"]
mod choice_transaction_tests;
#[cfg(test)]
#[path = "engine_form_fill_identity_tests.rs"]
mod form_fill_identity_native_tests;

#[path = "persistence.rs"]
mod persistence;
pub(crate) use persistence::same_object;
#[path = "text_edit.rs"]
mod text_edit;
#[path = "text_context_group.rs"]
mod text_context_group;
pub(crate) use text_edit::{choice_font_codes,redaction_font_codes};
pub(crate) use text_edit::nominal_standard_ascii_metrics;
#[path = "image_pages.rs"]
mod image_pages;
#[path = "redaction_selection.rs"]
mod redaction_selection;
#[path = "redaction_area_selection.rs"]
mod redaction_area_selection;
#[path = "redaction_preview.rs"]
mod redaction_preview;
pub(crate) use redaction_preview::{TextRedactionMark, TextRedactionPreview, TextRedactionBounds, TextRedactionPage, TextRedactionSource};
pub(crate) use redaction_preview::{ContentRedactionPreview,ContentRedactionRaster};
pub(crate) use redaction_selection::TextMarkRect as TextRedactionRect;
#[path = "document_policy.rs"]
pub(crate) mod document_policy;
use document_policy::{CompositionIntent, OpenSecurity, RecoveryPolicy, SourcePolicy};
#[path = "password_workflow.rs"]
mod password_workflow;
#[path = "physical_units.rs"]
mod physical_units;
use physical_units::Scale;
pub(crate) fn source_unit_from_pdf(document: &lopdf::Document, page: lopdf::ObjectId) -> EngineResult<f32> {
    physical_units::page_unit(document, page)
}
#[path = "page_crop.rs"]
pub(crate) mod page_crop;
#[path = "page_content_replace.rs"]
mod page_content_replace;
#[path = "page_admission.rs"]
mod page_admission;
#[path = "page_occurrence_replace.rs"]
mod page_occurrence_replace;
#[path = "occurrence_text_edit.rs"]
mod occurrence_text_edit;
#[path = "occurrence_paragraph.rs"]
mod occurrence_paragraph;
#[path = "occurrence_group.rs"]
mod occurrence_group;
#[path = "occurrence_form.rs"]
mod occurrence_form;
#[path = "form_paragraph.rs"]
mod form_paragraph;
#[path="existing_image_edit.rs"]
mod existing_image_edit;
#[path = "native_comments.rs"]
pub(crate) mod native_comments;
#[path = "page_search.rs"]
mod page_search;
#[path="page_image_render.rs"]
mod page_image_render;
#[path="pdf_split_render.rs"]
mod pdf_split_render;
#[path="text_export_engine.rs"]
mod text_export_engine;
#[path="read_page.rs"]
mod read_page;
#[path="complete_source_save.rs"]
mod complete_source_save;
#[path = "structured_source_save.rs"]
mod structured_source_save;
#[cfg(test)]
#[path = "highlight_admission_tests.rs"]
mod highlight_admission_tests;

#[path = "highlight_appearance.rs"]
mod highlight_appearance;
#[path = "structured_overlay_render.rs"]
mod structured_overlay_render;
#[path = "structured_overlay_publish.rs"]
mod structured_overlay_publish;
#[path = "structured_overlay_batch.rs"]
mod structured_overlay_batch;
#[path = "structured_overlay_flatten.rs"]
mod structured_overlay_flatten;
#[path = "imported_overlay_sources.rs"]
pub(crate) mod imported_overlay_sources;
#[path="form_author_engine.rs"]
mod form_author_engine;
#[path="form_position_engine.rs"]
mod form_position_engine;
#[path="form_text_appearance_engine.rs"]
mod form_text_appearance_engine;
#[path="form_frame_engine.rs"]
mod form_frame_engine;
#[path="form_properties_engine.rs"]
mod form_properties_engine;
#[path="document_properties_engine.rs"]
mod document_properties_engine;
#[path="form_removal_engine.rs"]
mod form_removal_engine;
#[path="form_choices_engine.rs"]
mod form_choices_engine;
#[cfg(feature="shaped-text")]
#[path="ocr_engine.rs"]
mod ocr_engine;

pub type EngineResult<T> = Result<T, EngineError>;
pub(crate) const MAX_SOURCE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    Protection(#[from] crate::protection::ProtectionError),
    #[error("This PDF does not permit {0}.")]
    PermissionDenied(&'static str),
    #[error("PDF engine initialization failed: {0}")]
    Initialization(String),
    #[error("PDF engine worker stopped")]
    WorkerStopped,
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("PDF operation failed: {0}")]
    Pdfium(String),
    #[error("file operation failed: {0}")]
    Io(String),
}

impl From<std::io::Error> for EngineError {
    fn from(value: std::io::Error) -> Self { Self::Io(value.to_string()) }
}

impl From<PdfiumError> for EngineError {
    fn from(value: PdfiumError) -> Self {
        Self::Pdfium(value.to_string())
    }
}

#[derive(Clone)]
pub struct PdfEngine {
    inner: Arc<EngineInner>,
}

struct EngineInner {
    sender: mpsc::Sender<WorkerRequest>,
    fonts: Arc<FontRegistry>,
    images: Arc<crate::ImageRegistry>,
}

static SHARED_ENGINE: OnceLock<Result<PdfEngine, String>> = OnceLock::new();

impl PdfEngine {
    pub fn list_native_comments(&self, source_id:&str)->EngineResult<Vec<crate::NativeComment>> {
        let (reply,receive)=mpsc::channel();self.send(WorkerRequest::ListNativeComments{source_id:source_id.into(),reply})?;
        let mut comments=receive.recv().map_err(|_|EngineError::WorkerStopped)??;
        let units=self.source_units(source_id)?;
        for comment in &mut comments{let unit=*units.get(comment.page_index).ok_or_else(||EngineError::InvalidRequest("Comment page is outside source.".into()))?;comment.bounds.scale(unit)?;comment.hit_bounds.scale(unit)?;}
        Ok(comments)
    }
    pub fn delete_native_comment(&self,source_id:&str,request:crate::DeleteNativeCommentRequest)->EngineResult<DocumentInfo> {
        let (reply,receive)=mpsc::channel();self.send(WorkerRequest::DeleteNativeComment{source_id:source_id.into(),request,reply})?;
        self.document_in_points(receive.recv().map_err(|_|EngineError::WorkerStopped)?)
    }
    pub fn edit_native_comment(&self,source_id:&str,request:crate::EditNativeCommentRequest)->EngineResult<DocumentInfo> {
        let mut request=request;request.position=self.in_raw(source_id,request.page_index,&request.position)?;
        let (reply,receive)=mpsc::channel();self.send(WorkerRequest::EditNativeComment{source_id:source_id.into(),request,reply})?;
        self.document_in_points(receive.recv().map_err(|_|EngineError::WorkerStopped)?)
    }

    pub fn list_form_fields(&self, source_id:&str,page_index:usize)->EngineResult<Vec<crate::FormField>> {
        let (reply,receive)=mpsc::channel();
        self.send(WorkerRequest::ListFormFields{source_id:source_id.into(),page_index,reply})?;
        self.in_points(source_id,page_index,receive.recv().map_err(|_|EngineError::WorkerStopped)?)
    }
    pub fn fill_form_field(&self,source_id:&str,request:crate::FillFormFieldRequest)->EngineResult<DocumentInfo> {
        let (reply,receive)=mpsc::channel();
        self.send(WorkerRequest::FillFormField{source_id:source_id.into(),request,reply})?;
        self.document_in_points(receive.recv().map_err(|_|EngineError::WorkerStopped)?)
    }
    pub fn create_image_document(&self, paths:Vec<PathBuf>, options:ImagePageOptions) -> EngineResult<DocumentInfo> {
        let (reply,receive)=mpsc::channel();
        self.send(WorkerRequest::CreateImages {paths,options,reply})?;
        self.document_in_points(receive.recv().map_err(|_|EngineError::WorkerStopped)?)
    }
    pub fn render_page_plan(&self, plan:PagePlan, width:u32) -> EngineResult<Vec<u8>> {
        let plan=self.export_in_raw(ExportRequest{pages:vec![plan],flatten:false})?.pages.remove(0);
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::RenderPlan {plan,width,reply})?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }
    pub fn create_blank_document(&self, width: f32, height: f32) -> EngineResult<DocumentInfo> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::CreateBlank {width,height,reply})?;
        self.document_in_points(receive.recv().map_err(|_| EngineError::WorkerStopped)?)
    }
    #[cfg(feature = "shaped-text")]
    pub fn prepare_text_overlay(
        &self,
        overlay: &TextOverlay,
    ) -> EngineResult<crate::PreparedTextOverlay> {
        let id = overlay.font_id.as_deref().ok_or_else(|| {
            EngineError::InvalidRequest("Shaped text requires a custom font.".into())
        })?;
        let font = self.inner.fonts.get(id)?;
        crate::prepare_text_overlay(&font, overlay)
    }
    pub fn images(&self) -> Arc<crate::ImageRegistry> { self.inner.images.clone() }
    pub fn import_image(&self, path: impl AsRef<Path>) -> EngineResult<crate::ImageRegistration> {
        let asset = Arc::new(crate::ImageAsset::read(path.as_ref())?);
        // Admission for placement includes supported color/appearance metadata,
        // not merely a decodable preview. Publish a lease only after both pass.
        crate::prepare_pdf_image(&asset)?;
        Ok(self.inner.images.register_assets(&[asset])?.remove(0))
    }
    pub fn fonts(&self) -> Arc<FontRegistry> {
        self.inner.fonts.clone()
    }

    pub fn suggest_text_paragraph(
        &self, source_id: &str, page_index: usize, object_index: usize, expected_text: &str,
    ) -> EngineResult<TextParagraphSelection> {
        let (reply, receive)=mpsc::channel();
        self.send(WorkerRequest::SuggestTextParagraph {
            source_id:source_id.into(),page_index,object_index,expected_text:expected_text.into(),reply,
        })?;
        self.in_points(source_id,page_index,receive.recv().map_err(|_|EngineError::WorkerStopped)?)
    }
    pub fn inspect_text_paragraph(
        &self,
        source_id: &str,
        page_index: usize,
        object_indices: &[usize],
    ) -> EngineResult<TextParagraphSelection> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::InspectTextParagraph {
            source_id: source_id.into(),
            page_index,
            object_indices: object_indices.to_vec(),
            reply,
        })?;
        self.in_points(source_id,page_index,receive.recv().map_err(|_| EngineError::WorkerStopped)?)
    }
    /// Source-bound attributed inspection, including verified saved drafts.
    pub fn inspect_styled_paragraph(&self,source_id:&str,page_index:usize,object_indices:&[usize])->EngineResult<StyledParagraphSelection> {
        let (reply,receive)=mpsc::channel();
        self.send(WorkerRequest::InspectStyledParagraph{source_id:source_id.into(),page_index,object_indices:object_indices.to_vec(),reply})?;
        self.in_points(source_id,page_index,receive.recv().map_err(|_|EngineError::WorkerStopped)?)
    }
    pub fn suggest_styled_paragraph(&self,source_id:&str,page_index:usize,object_index:usize,expected_text:&str)->EngineResult<StyledParagraphSelection>{
        let (reply,receive)=mpsc::channel();
        self.send(WorkerRequest::SuggestStyledParagraph{source_id:source_id.into(),page_index,object_index,expected_text:expected_text.into(),reply})?;
        self.in_points(source_id,page_index,receive.recv().map_err(|_|EngineError::WorkerStopped)?)
    }
    /// Read-only source-bound layout; used internally by mixed publication.
    pub fn layout_styled_paragraph(&self,source_id:&str,page_index:usize,request:&StyledParagraphLayoutRequest)->EngineResult<StyledParagraphLayout> {
        let (reply,receive)=mpsc::channel();
        self.send(WorkerRequest::LayoutStyledParagraph{source_id:source_id.into(),page_index,request:self.styled_in_raw(source_id,page_index,request)?,reply})?;
        receive.recv().map_err(|_|EngineError::WorkerStopped)?
    }
    /// Suggest a measured area for the current draft without changing its source.
    pub fn fit_styled_paragraph(&self,source_id:&str,page_index:usize,request:&StyledParagraphLayoutRequest)->EngineResult<AnnotationRect>{
        let (reply,receive)=mpsc::channel();
        self.send(WorkerRequest::FitStyledParagraph{source_id:source_id.into(),page_index,request:self.styled_in_raw(source_id,page_index,request)?,reply})?;
        self.in_points(source_id,page_index,receive.recv().map_err(|_|EngineError::WorkerStopped)?)
    }
    /// Publish an independently checked immutable mixed-paragraph preview.
    pub fn preview_styled_paragraph(&self,source_id:&str,page_index:usize,request:&StyledParagraphLayoutRequest)->EngineResult<DocumentInfo> {
        let (reply,receive)=mpsc::channel();
        self.send(WorkerRequest::PreviewStyledParagraph{source_id:source_id.into(),page_index,request:self.styled_in_raw(source_id,page_index,request)?,reply})?;
        self.document_in_points(receive.recv().map_err(|_|EngineError::WorkerStopped)?)
    }
    pub fn preview_text_paragraph(
        &self,
        source_id: &str,
        page_index: usize,
        object_indices: &[usize],
        expected_text: &str,
        replacement: &str,
        bounds: &AnnotationRect,
    ) -> EngineResult<DocumentInfo> {
        self.preview_text_paragraph_with_font_size(source_id,page_index,object_indices,expected_text,replacement,bounds,None)
    }
    pub fn preview_text_paragraph_with_font_size(
        &self,
        source_id: &str,
        page_index: usize,
        object_indices: &[usize],
        expected_text: &str,
        replacement: &str,
        bounds: &AnnotationRect,
        font_size: Option<f32>,
    ) -> EngineResult<DocumentInfo> {
        self.preview_text_paragraph_with_format(source_id,page_index,object_indices,expected_text,replacement,bounds,&ParagraphFormatting{font_size,..Default::default()})
    }
    pub fn preview_text_paragraph_with_format(
        &self,
        source_id: &str,
        page_index: usize,
        object_indices: &[usize],
        expected_text: &str,
        replacement: &str,
        bounds: &AnnotationRect,
        formatting: &ParagraphFormatting,
    ) -> EngineResult<DocumentInfo> {
        if formatting.font_size.is_some_and(|size|!physical_units::paragraph_size(size,1.)){
            return Err(EngineError::InvalidRequest("Use a font size from 1 to 300 pt.".into()));
        }
        let (bounds,formatting)=self.uniform_in_raw(source_id,page_index,object_indices,bounds,formatting)?;
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::PreviewTextParagraph {
            source_id: source_id.into(),
            page_index,
            object_indices: object_indices.to_vec(),
            expected_text: expected_text.into(),
            replacement: replacement.into(),
            bounds,
            formatting,
            reply,
        })?;
        self.document_in_points(receive.recv().map_err(|_| EngineError::WorkerStopped)?)
    }
    pub fn inspect_text_group(
        &self,
        source_id: &str,
        page_index: usize,
        object_indices: &[usize],
    ) -> EngineResult<TextGroupPreview> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::InspectTextGroup {
            source_id: source_id.into(),
            page_index,
            object_indices: object_indices.to_vec(),
            reply,
        })?;
        self.in_points(source_id,page_index,receive.recv().map_err(|_| EngineError::WorkerStopped)?)
    }
    pub fn replace_text_group(
        &self,
        source_id: &str,
        page_index: usize,
        object_indices: &[usize],
        expected_text: &str,
        replacement: &str,
    ) -> EngineResult<DocumentInfo> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::ReplaceTextGroup {
            source_id: source_id.into(),
            page_index,
            object_indices: object_indices.to_vec(),
            expected_text: expected_text.into(),
            replacement: replacement.into(),
            reply,
        })?;
        self.document_in_points(receive.recv().map_err(|_| EngineError::WorkerStopped)?)
    }
    pub fn list_form_text_runs(
        &self,
        source_id: &str,
        page_index: usize,
    ) -> EngineResult<FormTextRuns> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::ListFormTextRuns {
            source_id: source_id.into(),
            page_index,
            reply,
        })?;
        self.in_points(source_id,page_index,receive.recv().map_err(|_| EngineError::WorkerStopped)?)
    }
    pub fn replace_form_text(
        &self,
        source_id: &str,
        page_index: usize,
        object_path: &[usize],
        expected_text: &str,
        replacement: &str,
    ) -> EngineResult<DocumentInfo> {
        self.replace_form_text_with_font(source_id, page_index, object_path, expected_text, replacement, None)
    }
    pub fn replace_form_text_with_font(
        &self,
        source_id: &str,
        page_index: usize,
        object_path: &[usize],
        expected_text: &str,
        replacement: &str,
        font_id: Option<&str>,
    ) -> EngineResult<DocumentInfo> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::ReplaceFormText {
            source_id: source_id.into(),
            page_index,
            object_path: object_path.to_vec(),
            expected_text: expected_text.into(),
            replacement: replacement.into(),
            font_id: font_id.map(str::to_owned),
            reply,
        })?;
        self.document_in_points(receive.recv().map_err(|_| EngineError::WorkerStopped)?)
    }
    pub fn list_text_runs(&self, source_id: &str, page_index: usize) -> EngineResult<TextRuns> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::ListTextRuns {
            source_id: source_id.into(),
            page_index,
            reply,
        })?;
        self.in_points(source_id,page_index,receive.recv().map_err(|_| EngineError::WorkerStopped)?)
    }

    pub fn replace_text(
        &self,
        source_id: &str,
        page_index: usize,
        object_index: usize,
        expected_text: &str,
        replacement: &str,
    ) -> EngineResult<DocumentInfo> {
        self.replace_text_with_font(
            source_id,
            page_index,
            object_index,
            expected_text,
            replacement,
            None,
        )
    }

    pub fn replace_text_with_font(
        &self,
        source_id: &str,
        page_index: usize,
        object_index: usize,
        expected_text: &str,
        replacement: &str,
        font_id: Option<&str>,
    ) -> EngineResult<DocumentInfo> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::ReplaceText {
            source_id: source_id.into(),
            page_index,
            object_index,
            expected_text: expected_text.into(),
            replacement: replacement.into(),
            font_id: font_id.map(str::to_owned),
            reply,
        })?;
        self.document_in_points(receive.recv().map_err(|_| EngineError::WorkerStopped)?)
    }

    pub fn start(engine_path: impl AsRef<Path>) -> EngineResult<Self> {
        let path = engine_path.as_ref().to_path_buf();
        match SHARED_ENGINE.get_or_init(|| start_worker(path)) {
            Ok(engine) => Ok(engine.clone()),
            Err(error) => Err(EngineError::Initialization(error.clone())),
        }
    }

    pub fn open_document(&self, path: impl AsRef<Path>) -> EngineResult<DocumentInfo> {
        self.open_with_password(path.as_ref(), None)
    }

    pub fn open_document_with_password(&self, path: impl AsRef<Path>, password: &str) -> EngineResult<DocumentInfo> {
        if password.len() > 1024 {
            return Err(crate::protection::ProtectionError::Limit("password exceeds 1024 bytes").into());
        }
        self.open_with_password(path.as_ref(), Some(crate::private_storage::Sensitive(password.as_bytes().to_vec())))
    }

    fn open_with_password(&self, path: &Path, password: Option<crate::private_storage::Sensitive>) -> EngineResult<DocumentInfo> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::Open {
            path: path.to_path_buf(),
            password,
            original_path: None,
            editable_annotations: true,
            register_fonts: true,
            reply,
        })?;
        self.document_in_points(receive.recv().map_err(|_| EngineError::WorkerStopped)?)
    }

    /// Read only the private snapshot; retain the original path as an export guard.
    /// Current recovery plans already supply their fonts. They strip verified
    /// source annotations without retaining fonts that the user later removed.
    /// Legacy annotation migration passes `register_fonts = true`.
    pub(crate) fn open_recovery_document(
        &self,
        bytes: Vec<u8>,
        snapshot: impl AsRef<Path>,
        original_path: impl AsRef<Path>,
        register_fonts: bool,
        legacy_comment_import: bool,
        policy: Option<RecoveryPolicy>,
        annotation_sources: Option<imported_overlay_sources::Snapshot>,
        images: Vec<Arc<crate::ImageAsset>>,
        font_imports: Arc<crate::fonts::FontImportScope>,
    ) -> EngineResult<DocumentInfo> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::OpenBytes {
            bytes,
            path: snapshot.as_ref().to_path_buf(),
            original_path: original_path.as_ref().to_path_buf(),
            register_fonts,
            legacy_comment_import,
            policy,
            annotation_sources,
            images,
            font_imports,
            reply,
        })?;
        self.document_in_points(receive.recv().map_err(|_| EngineError::WorkerStopped)?)
    }

    /// Prepare an in-memory print source retaining annotation appearances.
    pub(crate) fn prepare_print(&self, request: ExportRequest) -> EngineResult<DocumentInfo> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::PreparePrint { request:self.export_in_raw(request)?, reply })?;
        self.document_in_points(receive.recv().map_err(|_| EngineError::WorkerStopped)?)
    }

    pub fn render_page(
        &self,
        source_id: &str,
        page_index: usize,
        width: u32,
    ) -> EngineResult<Vec<u8>> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::Render {
            source_id: source_id.to_owned(),
            page_index,
            width,
            reply,
        })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }

    pub fn export_pdf(
        &self,
        request: ExportRequest,
        destination: impl AsRef<Path>,
    ) -> EngineResult<()> {
        self.export_pdf_with_bookmarks(request,None,destination)
    }
    pub fn export_pdf_with_bookmarks(&self, request:ExportRequest, bookmarks:Option<Vec<crate::DocumentBookmark>>, destination:impl AsRef<Path>) -> EngineResult<()> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::Export {
            request:self.export_in_raw(request)?,
            bookmarks,
            destination: destination.as_ref().to_path_buf(),
            reply,
        })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }

    pub fn export_pdf_controlled(
        &self, request: ExportRequest, bookmarks: Option<Vec<crate::DocumentBookmark>>,
        destination: impl AsRef<Path>, control: Arc<crate::PdfSaveControl>,
        progress: Arc<dyn Fn(crate::PdfSaveProgress) + Send + Sync>,
    ) -> EngineResult<bool> {
        self.export_pdf_security_controlled(request,bookmarks,destination,None,control,progress)
    }
    pub fn export_pdf_with_security(
        &self, request:ExportRequest, bookmarks:Option<Vec<crate::DocumentBookmark>>,
        destination:impl AsRef<Path>, security:crate::ProtectionChange, control:Arc<crate::PdfSaveControl>,
        progress:Arc<dyn Fn(crate::PdfSaveProgress)+Send+Sync>,
    )->EngineResult<bool>{
        security.validate()?;
        self.export_pdf_security_controlled(request,bookmarks,destination,Some(security),control,progress)
    }
    fn export_pdf_security_controlled(
        &self, request:ExportRequest, bookmarks:Option<Vec<crate::DocumentBookmark>>,
        destination:impl AsRef<Path>, security:Option<crate::ProtectionChange>,control:Arc<crate::PdfSaveControl>,
        progress:Arc<dyn Fn(crate::PdfSaveProgress)+Send+Sync>,
    )->EngineResult<bool>{
        self.export_pdf_with_options(request,bookmarks,destination,security,None,control,progress)
    }
    pub fn export_pdf_with_options(
        &self,request:ExportRequest,bookmarks:Option<Vec<crate::DocumentBookmark>>,destination:impl AsRef<Path>,
        security:Option<crate::ProtectionChange>,metadata:Option<crate::DocumentMetadataEdit>,control:Arc<crate::PdfSaveControl>,
        progress:Arc<dyn Fn(crate::PdfSaveProgress)+Send+Sync>,
    )->EngineResult<bool>{
        if let Some(metadata)=&metadata{metadata.validate()?;}
        if let Some(security)=&security{security.validate()?;}
        let monitor = crate::pdf_save::PdfSaveMonitor { control: control.clone(), total: request.pages.len(), report: progress };
        let result = (|| {
            let request = self.export_in_raw_checked(request, |done| monitor.checkpoint("preparing", done))?;
            let (reply, receive) = mpsc::channel();
            self.send(WorkerRequest::ControlledExport { request, bookmarks, security, metadata, destination: destination.as_ref().to_path_buf(), monitor, reply })?;
            receive.recv().map_err(|_| EngineError::WorkerStopped)?
        })();
        if control.cancelled() { Ok(false) } else { result }
    }

    pub fn close_document(&self, source_id: &str) -> EngineResult<()> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::Close {
            source_id: source_id.to_owned(),
            reply,
        })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }

    /// Immutable bytes captured when this source was opened, before editor overlays.
    pub(crate) fn source_recovery_policy(&self, source_id: &str) -> EngineResult<RecoveryPolicy> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::SourceRecoveryPolicy {source_id:source_id.into(),reply})?;
        receive.recv().map_err(|_|EngineError::WorkerStopped)?
    }

    pub(crate) fn source_legacy_comment_import(&self, source_id: &str) -> EngineResult<bool> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::SourceLegacyCommentImport { source_id: source_id.into(), reply })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }

    pub(crate) fn source_imported_overlay_snapshot(&self, source_id: &str) -> EngineResult<Option<imported_overlay_sources::Snapshot>> {
        self.source_imported_overlay_snapshot_with_budget(source_id, imported_overlay_sources::MAX_METADATA)
    }
    pub(crate) fn source_imported_overlay_snapshot_with_budget(&self, source_id: &str, limit: usize) -> EngineResult<Option<imported_overlay_sources::Snapshot>> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::SourceImportedOverlaySnapshot { source_id: source_id.into(), limit, reply })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }

    pub fn source_bytes(&self, source_id: &str) -> EngineResult<Arc<[u8]>> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::SourceBytes {
            source_id: source_id.to_owned(),
            reply,
        })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }

    pub(crate) fn source_original_path(&self, source_id: &str) -> EngineResult<PathBuf> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::SourceOriginalPath {
            source_id: source_id.to_owned(),
            reply,
        })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }

    /// Raw PDFium extraction for engine interoperability checks. The selectable
    /// reader/search path is page_text(), which compensates for page rotation.
    pub fn extract_text(&self, source_id: &str, page_index: usize) -> EngineResult<String> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::ExtractText {
            source_id: source_id.to_owned(),
            page_index,
            reply,
        })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }

    pub fn page_text(&self, source_id: &str, page_index: usize) -> EngineResult<PageText> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::PageText {
            source_id: source_id.to_owned(),
            page_index,
            reply,
        })?;
        self.in_points(source_id,page_index,receive.recv().map_err(|_| EngineError::WorkerStopped)?)
    }

    pub fn status(&self) -> EngineResult<String> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::Status { reply })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)
    }

    fn send(&self, request: WorkerRequest) -> EngineResult<()> {
        self.inner
            .sender
            .send(request)
            .map_err(|_| EngineError::WorkerStopped)
    }
}

fn start_worker(engine_path: PathBuf) -> Result<PdfEngine, String> {
    let (sender, receiver) = mpsc::channel();
    let (init_sender, init_receiver) = mpsc::sync_channel(1);
    let fonts = Arc::new(FontRegistry::default());
    let worker_fonts = fonts.clone();
    let images = Arc::new(crate::ImageRegistry::new());
    let worker_images = images.clone();

    std::thread::Builder::new()
        .name("folio-pdfium-worker".into())
        .spawn(
            move || match WorkerRuntime::new(&engine_path, worker_fonts) {
                Ok(mut runtime) => {
                    runtime.images = worker_images;
                    let _ = init_sender.send(Ok(runtime.status.clone()));
                    runtime.run(receiver);
                }
                Err(error) => {
                    let _ = init_sender.send(Err(error.to_string()));
                }
            },
        )
        .map_err(|error| error.to_string())?;

    init_receiver
        .recv()
        .map_err(|_| "PDFium worker exited during initialization".to_string())??;

    Ok(PdfEngine {
        inner: Arc::new(EngineInner { sender, fonts, images }),
    })
}

enum WorkerRequest {
    TextRedaction(redaction_preview::Request),
    OccurrenceParagraph(occurrence_paragraph::Request),
    OccurrenceGroup(occurrence_group::Request),
    OccurrenceForm(occurrence_form::Request),
    FormParagraph(form_paragraph::Request),
    ExistingImage(existing_image_edit::Request),
    ListOccurrenceTextRuns {source_id:String,page_index:usize,reply:mpsc::Sender<EngineResult<TextRuns>>},
    ReplaceTextOccurrence {source_id:String,occurrences:Vec<usize>,selected_occurrence:usize,object_index:usize,expected_text:String,replacement:String,font_id:Option<String>,reply:mpsc::Sender<EngineResult<PageContentReplacement>>},
    ReplacePageContent {destination_id:String,destination_page:usize,incoming_id:String,incoming_page:usize,reply:mpsc::Sender<EngineResult<DocumentInfo>>},
    ReplacePageContentOccurrence {destination_id:String,occurrences:Vec<usize>,selected_occurrence:usize,incoming_id:String,incoming_page:usize,reply:mpsc::Sender<EngineResult<PageContentReplacement>>},
    ValidateCrop {source_id:String,page_index:usize,crop:AnnotationRect,reply:mpsc::Sender<EngineResult<()>>},
    SourceUnits {source_id:String,reply:mpsc::Sender<EngineResult<Vec<f32>>>},
    #[cfg(feature="shaped-text")]
    PrepareOcr {source_id:String,page_index:usize,rotation:u16,reply:mpsc::Sender<EngineResult<crate::OcrPreparedPage>>},
    #[cfg(feature="shaped-text")]
    ApplyOcr {source_id:String,updates:Vec<crate::OcrUpdate>,reply:mpsc::Sender<EngineResult<DocumentInfo>>},
    CheckSourceCombination {source_ids:Vec<String>,reply:mpsc::Sender<EngineResult<()>>},
    SearchPage {source_id:String,page_index:usize,query:String,reply:mpsc::Sender<EngineResult<PageSearchResult>>},
    BeginOpen {path:PathBuf,reply:mpsc::Sender<EngineResult<OpenOutcome>>},
    SubmitOpenPassword {challenge_id:String,password:crate::private_storage::Sensitive,reply:mpsc::Sender<EngineResult<OpenOutcome>>},
    CancelOpen {challenge_id:String,reply:mpsc::Sender<EngineResult<()>>},
    UnlockSources {source_ids:Vec<String>,password:crate::private_storage::Sensitive,reply:mpsc::Sender<EngineResult<Vec<SourceReplacement>>>},
    ListNativeComments {source_id:String,reply:mpsc::Sender<EngineResult<Vec<crate::NativeComment>>>},
    DeleteNativeComment {source_id:String,request:crate::DeleteNativeCommentRequest,reply:mpsc::Sender<EngineResult<DocumentInfo>>},
    EditNativeComment {source_id:String,request:crate::EditNativeCommentRequest,reply:mpsc::Sender<EngineResult<DocumentInfo>>},
    ListFormFields {source_id:String,page_index:usize,reply:mpsc::Sender<EngineResult<Vec<crate::FormField>>>},
    FillFormField {source_id:String,request:crate::FillFormFieldRequest,reply:mpsc::Sender<EngineResult<DocumentInfo>>},
    InspectFormProperties {source_id:String,page_index:usize,annotation_index:usize,reply:mpsc::Sender<EngineResult<crate::FormProperties>>},
    SetFormProperties {source_id:String,request:crate::SetFormPropertiesRequest,reply:mpsc::Sender<EngineResult<DocumentInfo>>},
    InspectFormRemoval {source_id:String,page_index:usize,annotation_index:usize,reply:mpsc::Sender<EngineResult<crate::FormRemovalInfo>>},
    RemoveFormField {source_id:String,request:crate::RemoveFormFieldRequest,reply:mpsc::Sender<EngineResult<DocumentInfo>>},
    InspectFormChoices {source_id:String,page_index:usize,annotation_index:usize,reply:mpsc::Sender<EngineResult<crate::FormChoices>>},
    SetFormChoices {source_id:String,request:crate::SetFormChoicesRequest,reply:mpsc::Sender<EngineResult<DocumentInfo>>},
    CreateFormField {source_id:String,occurrences:Vec<usize>,selected:usize,request:crate::CreateFormFieldRequest,reply:mpsc::Sender<EngineResult<PageContentReplacement>>},
    PositionFormField {source_id:String,occurrences:Vec<usize>,selected:usize,request:crate::PositionFormFieldRequest,reply:mpsc::Sender<EngineResult<PageContentReplacement>>},
    InspectFormTextAppearance {source_id:String,page_index:usize,annotation_index:usize,reply:mpsc::Sender<EngineResult<crate::FormTextAppearance>>},
    InspectFormFrame {source_id:String,page_index:usize,annotation_index:usize,reply:mpsc::Sender<EngineResult<crate::FormFrameAppearance>>},
    SetFormFrame {source_id:String,occurrences:Vec<usize>,selected:usize,request:crate::SetFormFrameRequest,reply:mpsc::Sender<EngineResult<PageContentReplacement>>},
    SetFormTextAppearance {source_id:String,occurrences:Vec<usize>,selected:usize,request:crate::SetFormTextAppearanceRequest,reply:mpsc::Sender<EngineResult<PageContentReplacement>>},
    CreateImages {paths:Vec<PathBuf>,options:ImagePageOptions,reply:mpsc::Sender<EngineResult<DocumentInfo>>},
    RenderPlan {plan:PagePlan,width:u32,reply:mpsc::Sender<EngineResult<Vec<u8>>>},
    CreateBlank { width:f32, height:f32, reply:mpsc::Sender<EngineResult<DocumentInfo>> },
    ListFormTextRuns {
        source_id: String,
        page_index: usize,
        reply: mpsc::Sender<EngineResult<FormTextRuns>>,
    },
    ReplaceFormText {
        source_id: String,
        page_index: usize,
        object_path: Vec<usize>,
        expected_text: String,
        replacement: String,
        font_id: Option<String>,
        reply: mpsc::Sender<EngineResult<DocumentInfo>>,
    },
    SuggestTextParagraph {
        source_id: String,
        page_index: usize,
        object_index: usize,
        expected_text: String,
        reply: mpsc::Sender<EngineResult<TextParagraphSelection>>,
    },
    InspectTextParagraph {
        source_id: String,
        page_index: usize,
        object_indices: Vec<usize>,
        reply: mpsc::Sender<EngineResult<TextParagraphSelection>>,
    },
    InspectStyledParagraph {
        source_id:String,page_index:usize,object_indices:Vec<usize>,
        reply:mpsc::Sender<EngineResult<StyledParagraphSelection>>,
    },
    SuggestStyledParagraph {
        source_id:String,page_index:usize,object_index:usize,expected_text:String,
        reply:mpsc::Sender<EngineResult<StyledParagraphSelection>>,
    },
    LayoutStyledParagraph {
        source_id:String,page_index:usize,request:StyledParagraphLayoutRequest,
        reply:mpsc::Sender<EngineResult<StyledParagraphLayout>>,
    },
    PreviewStyledParagraph {
        source_id:String,page_index:usize,request:StyledParagraphLayoutRequest,
        reply:mpsc::Sender<EngineResult<DocumentInfo>>,
    },
    FitStyledParagraph {
        source_id:String,page_index:usize,request:StyledParagraphLayoutRequest,
        reply:mpsc::Sender<EngineResult<AnnotationRect>>,
    },
    PreviewTextParagraph {
        source_id: String,
        page_index: usize,
        object_indices: Vec<usize>,
        expected_text: String,
        replacement: String,
        bounds: AnnotationRect,
        formatting: ParagraphFormatting,
        reply: mpsc::Sender<EngineResult<DocumentInfo>>,
    },
    InspectTextGroup {
        source_id: String,
        page_index: usize,
        object_indices: Vec<usize>,
        reply: mpsc::Sender<EngineResult<TextGroupPreview>>,
    },
    ReplaceTextGroup {
        source_id: String,
        page_index: usize,
        object_indices: Vec<usize>,
        expected_text: String,
        replacement: String,
        reply: mpsc::Sender<EngineResult<DocumentInfo>>,
    },
    ListTextRuns {
        source_id: String,
        page_index: usize,
        reply: mpsc::Sender<EngineResult<TextRuns>>,
    },
    ReplaceText {
        source_id: String,
        page_index: usize,
        object_index: usize,
        expected_text: String,
        replacement: String,
        font_id: Option<String>,
        reply: mpsc::Sender<EngineResult<DocumentInfo>>,
    },
    Open {
        path: PathBuf,
        password: Option<crate::private_storage::Sensitive>,
        original_path: Option<PathBuf>,
        editable_annotations: bool,
        register_fonts: bool,
        reply: mpsc::Sender<EngineResult<DocumentInfo>>,
    },
    OpenBytes {
        bytes: Vec<u8>,
        path: PathBuf,
        original_path: PathBuf,
        register_fonts: bool,
        legacy_comment_import: bool,
        policy: Option<RecoveryPolicy>,
        annotation_sources: Option<imported_overlay_sources::Snapshot>,
        images: Vec<Arc<crate::ImageAsset>>,
        font_imports: Arc<crate::fonts::FontImportScope>,
        reply: mpsc::Sender<EngineResult<DocumentInfo>>,
    },
    Render {
        source_id: String,
        page_index: usize,
        width: u32,
        reply: mpsc::Sender<EngineResult<Vec<u8>>>,
    },
    SplitPdf {
        request:crate::SplitPdfRequest,parent:PathBuf,control:Arc<crate::PageImageControl>,
        progress:page_image_render::Progress,resources:page_image_render::Resources,
        reply:mpsc::Sender<EngineResult<Option<crate::PageImageExportResult>>>,
    },
    ExportPageImages {
        request:crate::PageImageExportRequest,parent:PathBuf,control:Arc<crate::PageImageControl>,
        progress:page_image_render::Progress,resources:page_image_render::Resources,
        reply:mpsc::Sender<EngineResult<Option<crate::PageImageExportResult>>>,
    },
    ExportText {
        request:crate::TextExportRequest,destination:PathBuf,control:Arc<crate::TextExportControl>,
        progress:text_export_engine::Progress,resources:page_image_render::Resources,
        reply:mpsc::Sender<EngineResult<Option<crate::TextExportResult>>>,
    },
    PreparePrint {
        request: ExportRequest,
        reply: mpsc::Sender<EngineResult<DocumentInfo>>,
    },
    Export {
        request: ExportRequest,
        bookmarks:Option<Vec<crate::DocumentBookmark>>,
        destination: PathBuf,
        reply: mpsc::Sender<EngineResult<()>>,
    },
    ControlledExport {
        request: ExportRequest,
        bookmarks: Option<Vec<crate::DocumentBookmark>>,
        security: Option<crate::ProtectionChange>,
        metadata:Option<crate::DocumentMetadataEdit>,
        destination: PathBuf,
        monitor: crate::pdf_save::PdfSaveMonitor,
        reply: mpsc::Sender<EngineResult<bool>>,
    },
    InspectDocumentProperties {source_id:String,reply:mpsc::Sender<EngineResult<crate::DocumentProperties>>},
    Close {
        source_id: String,
        reply: mpsc::Sender<EngineResult<()>>,
    },
    SourceBytes {
        source_id: String,
        reply: mpsc::Sender<EngineResult<Arc<[u8]>>>,
    },
    SourceLegacyCommentImport {
        source_id: String,
        reply: mpsc::Sender<EngineResult<bool>>,
    },
    SourceRecoveryPolicy {
        source_id: String,
        reply: mpsc::Sender<EngineResult<RecoveryPolicy>>,
    },
    SourceImportedOverlaySnapshot {
        source_id: String,
        limit: usize,
        reply: mpsc::Sender<EngineResult<Option<imported_overlay_sources::Snapshot>>>,
    },
    SourceOriginalPath {
        source_id: String,
        reply: mpsc::Sender<EngineResult<PathBuf>>,
    },
    ExtractText {
        source_id: String,
        page_index: usize,
        reply: mpsc::Sender<EngineResult<String>>,
    },
    PageText {
        source_id: String,
        page_index: usize,
        reply: mpsc::Sender<EngineResult<PageText>>,
    },
    Status {
        reply: mpsc::Sender<String>,
    },
}

struct ImageSourceLeases {
    registry: Arc<crate::ImageRegistry>,
    registrations: Vec<crate::ImageRegistration>,
}
impl Drop for ImageSourceLeases {
    fn drop(&mut self) { let _ = self.registry.release_all(&self.registrations); }
}

struct OpenDocument {
    physical_units: OnceLock<Result<Vec<f32>,String>>,
    // Old recovery sources keep Text annotations owned by workspace overlays.
    legacy_comment_import: bool,
    imported_annotations: HashMap<usize,HashSet<usize>>,
    imported_overlay_sources: imported_overlay_sources::Index,
    image_leases: Option<ImageSourceLeases>,
    unpublished_fonts: Vec<crate::fonts::FontRegistration>,
    path: PathBuf,
    original_path: PathBuf,
    document: native_residency::ResidentDocument,
    source_bytes: crate::source_storage::SourceHandle,
    for_printing: bool,
    print_max_dpi: Option<u32>,
    policy: Option<SourcePolicy>,
    text_edit_restriction: OnceLock<Option<String>>,
    // Admission proves immutable source bytes only; current policy is checked
    // separately on every request. New document versions start unvalidated.
    form_admission: form_admission::Admission,
    bookmarks:OnceLock<Result<Vec<crate::DocumentBookmark>,String>>,
}

struct WorkerRuntime {
    source_store: crate::source_storage::SourceStore,
    #[cfg(feature="shaped-text")]
    ocr_reading:ocr_engine::ReadingCache,
    pdfium: &'static Pdfium,
    bindings: &'static dyn PdfiumLibraryBindings,
    documents: HashMap<String, OpenDocument>,
    redaction_previews: redaction_preview::PreviewStore,
    paragraph_contexts: HashMap<String, Arc<text_edit::ParagraphSourcePage>>,
    pending_opens: HashMap<String,password_workflow::PendingOpen>,
    status: String,
    fonts: Arc<FontRegistry>,
    images: Arc<crate::ImageRegistry>,
}

#[derive(Debug, Clone, Copy)]
struct PageGeometry {
    left: f32,
    bottom: f32,
    right: f32,
    top: f32,
    rotation: u16,
}

impl PageGeometry {
    fn raw_width(self) -> f32 {
        self.right - self.left
    }

    fn raw_height(self) -> f32 {
        self.top - self.bottom
    }

    fn displayed_size(self) -> PageInfo {
        if self.rotation == 90 || self.rotation == 270 {
            PageInfo {
                width: self.raw_height(),
                height: self.raw_width(),
                overlays: Vec::new(),
            }
        } else {
            PageInfo {
                width: self.raw_width(),
                height: self.raw_height(),
                overlays: Vec::new(),
            }
        }
    }

    fn pdf_to_displayed(self, x: f32, y: f32) -> Point {
        let (x, y) = match self.rotation {
            0 => (x - self.left, self.top - y),
            90 => (y - self.bottom, x - self.left),
            180 => (self.right - x, y - self.bottom),
            270 => (self.top - y, self.right - x),
            _ => unreachable!("PDFium only reports quarter-turn rotations"),
        };
        Point { x, y }
    }

    fn displayed_to_pdf(self, point: Point) -> (PdfPoints, PdfPoints) {
        let (x, y) = match self.rotation {
            0 => (self.left + point.x, self.top - point.y),
            90 => (self.left + point.y, self.bottom + point.x),
            180 => (self.right - point.x, self.bottom + point.y),
            270 => (self.right - point.y, self.top - point.x),
            _ => unreachable!("PDFium only reports quarter-turn rotations"),
        };
        (PdfPoints::new(x), PdfPoints::new(y))
    }
}

impl WorkerRuntime {
    fn new(engine_path: &Path, fonts: Arc<FontRegistry>) -> EngineResult<Self> {
        if !engine_path.is_file() {
            return Err(EngineError::Initialization(format!(
                "PDFium library was not found at {}",
                engine_path.display()
            )));
        }
        let canonical = std::fs::canonicalize(engine_path)
            .map_err(|error| EngineError::Io(error.to_string()))?;
        #[cfg(not(test))]
        let (pdfium,bindings) = {
        let bindings = Pdfium::bind_to_library(&canonical)
            .map_err(|error| EngineError::Initialization(error.to_string()))?;
        let raw_bindings=bindings.as_ref() as *const dyn PdfiumLibraryBindings;
        let pdfium = Box::leak(Box::new(Pdfium::new(bindings)));
        // The single SHARED_ENGINE initializer is the sole Pdfium initializer in
        // Folio. bind_to_library rejects an already initialized global. Pdfium::new
        // moves this Box unchanged into its process-lifetime OnceCell; pdfium itself
        // is also leaked. Borrow that same thread-safe binding (including its mutex),
        // never another DLL binding or unguarded function pointer.
        let bindings=unsafe {&*raw_bindings};
            (pdfium,bindings)
        };
        #[cfg(test)]
        let (pdfium,bindings) = {
            // Library tests create private runtimes concurrently. Initialize the
            // process-global binding once, then give each runtime an independent
            // wrapper using that same thread-safe binding. Production continues
            // to initialize through SHARED_ENGINE above.
            static TEST_BINDINGS:OnceLock<Result<&'static dyn PdfiumLibraryBindings,String>>=OnceLock::new();
            let bindings=TEST_BINDINGS.get_or_init(||{
                let bindings=Pdfium::bind_to_library(&canonical).map_err(|e|e.to_string())?;
                let raw=bindings.as_ref() as *const dyn PdfiumLibraryBindings;
                let _=Box::leak(Box::new(Pdfium::new(bindings)));
                // The Box moved into PDFium's process-lifetime OnceCell. The
                // initializer above is serialized and this reference never drops.
                Ok(unsafe {&*raw})
            }).as_ref().map_err(|e|EngineError::Initialization(e.clone()))?;
            (Box::leak(Box::new(Pdfium::default())),*bindings)
        };
        let status = "Native PDF engine ready".to_string();
        Ok(Self {
            pdfium,
            bindings,
            documents: HashMap::new(),
            source_store: crate::source_storage::SourceStore::new(),
            redaction_previews: Default::default(),
            paragraph_contexts: HashMap::new(),
            pending_opens: HashMap::new(),
            #[cfg(feature="shaped-text")]
            ocr_reading:Default::default(),
            status,
            fonts,
            images: Arc::new(crate::ImageRegistry::new()),
        })
    }

    fn run(&mut self, receiver: mpsc::Receiver<WorkerRequest>) {
        while let Ok(request) = receiver.recv() {
            match request {
                WorkerRequest::TextRedaction(request) => self.handle_text_redaction(request),
                #[cfg(feature="shaped-text")]
                WorkerRequest::PrepareOcr{source_id,page_index,rotation,reply}=>{let _=reply.send(self.prepare_ocr_page(&source_id,page_index,rotation));}
                #[cfg(feature="shaped-text")]
                WorkerRequest::ApplyOcr{source_id,updates,reply}=>{
                    let result=self.apply_ocr(&source_id,updates);
                    if let Err(mpsc::SendError(Ok(candidate)))=reply.send(result){self.documents.remove(&candidate.id);}
                }
                WorkerRequest::CheckSourceCombination {source_ids,reply} => {
                    let _=reply.send(self.check_source_combination(&source_ids));
                }
                WorkerRequest::SearchPage {source_id,page_index,query,reply}=>{
                    let _=reply.send(self.search_page(&source_id,page_index,&query));
                }
                WorkerRequest::BeginOpen {path,reply}=>{
                    let result=self.begin_open_document(path);self.reply_open(reply,result);
                }
                WorkerRequest::SubmitOpenPassword {challenge_id,password,reply}=>{
                    let result=self.submit_open_password(&challenge_id,password_workflow::password_str(&password));self.reply_open(reply,result);
                }
                WorkerRequest::CancelOpen {challenge_id,reply}=>{
                    self.expire_opens();self.pending_opens.remove(&challenge_id);let _=reply.send(Ok(()));
                }
                WorkerRequest::UnlockSources {source_ids,password,reply}=>{
                    let result=self.unlock_document_sources(&source_ids,password_workflow::password_str(&password));
                    if let Err(mpsc::SendError(Ok(replacements)))=reply.send(result) {
                        for replacement in replacements {self.documents.remove(&replacement.document.id);}
                    }
                }
                WorkerRequest::ListNativeComments{source_id,reply}=>{let _=reply.send(self.list_native_comments(&source_id));},
                WorkerRequest::DeleteNativeComment{source_id,request,reply}=>{
                    let result=self.delete_native_comment(&source_id,&request);self.reply_document(reply,result);
                },
                WorkerRequest::EditNativeComment{source_id,request,reply}=>{
                    if let Err(mpsc::SendError(Ok(info)))=reply.send(self.edit_native_comment(&source_id,&request)){self.documents.remove(&info.id);}
                },
                WorkerRequest::ListFormFields{source_id,page_index,reply}=>{
                    let result=self.document(&source_id).and_then(|source| {
                        if source.for_printing || source.policy.as_ref().is_some_and(|p|{let r=p.protection.permissions(); !r.fill_forms && !r.copy_content}) {
                            return Err(EngineError::PermissionDenied("reading form fields"));
                        }
                        let mut fields=crate::form_fill::NativeForm::open(self.bindings,source.bytes()?.as_ref())?.fields(page_index)?;
                        if !fields.is_empty() {
                            let page=source.pdf()?.pages().get(i32::try_from(page_index).map_err(|_|EngineError::InvalidRequest("Page index is too large.".into()))?)?;
                            let geometry=page_geometry(&page)?;
                            for field in &mut fields {
                                if let Some(raw)=field.bounds.take() {
                                    let anchor=geometry.pdf_to_displayed(raw.x,raw.y+raw.height);
                                    let opposite=geometry.pdf_to_displayed(raw.x+raw.width,raw.y);
                                    field.bounds=Some(if field.no_rotate {
                                        AnnotationRect {x:anchor.x,y:anchor.y,width:raw.width,height:raw.height}
                                    }else{
                                        AnnotationRect {x:anchor.x.min(opposite.x),y:anchor.y.min(opposite.y),width:(opposite.x-anchor.x).abs(),height:(opposite.y-anchor.y).abs()}
                                    });
                                }
                            }
                        }
                        Ok(fields)
                    });
                    let _=reply.send(result);
                },
                WorkerRequest::FillFormField{source_id,request,reply}=>{
                    let _=reply.send(self.fill_form_field(&source_id,&request));
                },
                WorkerRequest::CreateFormField{source_id,occurrences,selected,request,reply}=>{
                    let result=self.create_form_field_occurrence(&source_id,&occurrences,selected,&request);
                    if let Err(mpsc::SendError(Ok(result)))=reply.send(result) {self.discard_unpublished_source(&result.document.id);}
                },
                WorkerRequest::PositionFormField{source_id,occurrences,selected,request,reply}=>{
                    let result=self.position_form_field_occurrence(&source_id,&occurrences,selected,&request);
                    if let Err(mpsc::SendError(Ok(result)))=reply.send(result){self.discard_unpublished_source(&result.document.id);}
                },
                WorkerRequest::InspectFormTextAppearance{source_id,page_index,annotation_index,reply}=>{
                    let _=reply.send(self.inspect_form_text_appearance(&source_id,page_index,annotation_index));
                },
                WorkerRequest::InspectFormFrame{source_id,page_index,annotation_index,reply}=>{
                    let _=reply.send(self.inspect_form_frame(&source_id,page_index,annotation_index));
                },
                WorkerRequest::SetFormFrame{source_id,occurrences,selected,request,reply}=>{
                    let result=self.set_form_frame_occurrence(&source_id,&occurrences,selected,&request);
                    if let Err(mpsc::SendError(Ok(result)))=reply.send(result){self.discard_unpublished_source(&result.document.id);}
                },
                WorkerRequest::InspectFormProperties{source_id,page_index,annotation_index,reply}=>{
                    let _=reply.send(self.inspect_form_properties(&source_id,page_index,annotation_index));
                },
                WorkerRequest::SetFormProperties{source_id,request,reply}=>{
                    let result=self.set_form_properties(&source_id,&request);
                    if let Err(mpsc::SendError(Ok(result)))=reply.send(result){self.discard_unpublished_source(&result.id);}
                },
                WorkerRequest::InspectFormChoices{source_id,page_index,annotation_index,reply}=>{
                    let _=reply.send(self.inspect_form_choices(&source_id,page_index,annotation_index));
                },
                WorkerRequest::SetFormChoices{source_id,request,reply}=>{
                    let result=self.set_form_choices(&source_id,&request);
                    if let Err(mpsc::SendError(Ok(result)))=reply.send(result){self.discard_unpublished_source(&result.id);}
                },
                WorkerRequest::SetFormTextAppearance{source_id,occurrences,selected,request,reply}=>{
                    let result=self.set_form_text_appearance_occurrence(&source_id,&occurrences,selected,&request);
                    if let Err(mpsc::SendError(Ok(result)))=reply.send(result){self.discard_unpublished_source(&result.document.id);}
                },
                WorkerRequest::ListOccurrenceTextRuns {source_id,page_index,reply} => {
                    let _=reply.send(self.list_transaction_text_runs(&source_id,page_index,true));
                },
                WorkerRequest::OccurrenceParagraph(request) => self.handle_occurrence_paragraph(request),
                WorkerRequest::OccurrenceGroup(request) => self.handle_occurrence_group(request),
                WorkerRequest::OccurrenceForm(request) => self.handle_occurrence_form(request),
                WorkerRequest::FormParagraph(request) => self.handle_form_paragraph(request),
                WorkerRequest::ExistingImage(request) => self.handle_existing_image(request),
                WorkerRequest::ReplaceTextOccurrence {source_id,occurrences,selected_occurrence,object_index,expected_text,replacement,font_id,reply} => {
                    let result=self.replace_text_occurrence(&source_id,&occurrences,selected_occurrence,object_index,&expected_text,&replacement,font_id.as_deref());
                    if let Err(mpsc::SendError(Ok(result)))=reply.send(result) {self.discard_unpublished_source(&result.document.id);}
                },
                WorkerRequest::ReplacePageContent {destination_id,destination_page,incoming_id,incoming_page,reply} => {
                    let result=self.replace_page_content(&destination_id,destination_page,&incoming_id,incoming_page);
                    self.reply_document(reply,result);
                },
                WorkerRequest::ReplacePageContentOccurrence {destination_id,occurrences,selected_occurrence,incoming_id,incoming_page,reply} => {
                    let result=self.replace_page_content_occurrence(&destination_id,&occurrences,selected_occurrence,&incoming_id,incoming_page);
                    if let Err(mpsc::SendError(Ok(result)))=reply.send(result) {
                        self.discard_unpublished_source(&result.document.id);
                    }
                },
                WorkerRequest::ListFormTextRuns {
                    source_id,
                    page_index,
                    reply,
                } => {
                    let _ = reply.send(self.list_form_text_runs(&source_id, page_index));
                }
                WorkerRequest::InspectFormRemoval{source_id,page_index,annotation_index,reply}=>{
                    let _=reply.send(self.inspect_form_removal(&source_id,page_index,annotation_index));
                }
                WorkerRequest::RemoveFormField{source_id,request,reply}=>{
                    let result=self.remove_form_field(&source_id,&request);let _=reply.send(result);
                }
                WorkerRequest::ReplaceFormText {
                    source_id,
                    page_index,
                    object_path,
                    expected_text,
                    replacement,
                    font_id,
                    reply,
                } => {
                    let _ = reply.send(self.replace_form_text(
                        &source_id,
                        page_index,
                        &object_path,
                        &expected_text,
                        &replacement,
                        font_id.as_deref(),
                    ));
                }
                WorkerRequest::SuggestTextParagraph {source_id,page_index,object_index,expected_text,reply} => {
                    let _=reply.send(self.suggest_text_paragraph(&source_id,page_index,object_index,&expected_text));
                }
                WorkerRequest::InspectTextParagraph {
                    source_id,
                    page_index,
                    object_indices,
                    reply,
                } => {
                    let _ = reply.send(self.inspect_text_paragraph(
                        &source_id,
                        page_index,
                        &object_indices,
                    ));
                }
                WorkerRequest::InspectStyledParagraph {source_id,page_index,object_indices,reply} => {
                    let _=reply.send(self.inspect_styled_paragraph(&source_id,page_index,&object_indices));
                }
                WorkerRequest::SuggestStyledParagraph {source_id,page_index,object_index,expected_text,reply} => {
                    let _=reply.send(self.suggest_styled_paragraph(&source_id,page_index,object_index,&expected_text));
                }
                WorkerRequest::LayoutStyledParagraph {source_id,page_index,request,reply} => {
                    let _=reply.send(self.layout_styled_paragraph(&source_id,page_index,&request));
                }
                WorkerRequest::PreviewStyledParagraph {source_id,page_index,request,reply} => {
                    let _=reply.send(self.preview_styled_paragraph(&source_id,page_index,&request));
                }
                WorkerRequest::FitStyledParagraph {source_id,page_index,request,reply} => {
                    let _=reply.send(self.fit_styled_paragraph(&source_id,page_index,&request));
                }
                WorkerRequest::PreviewTextParagraph {
                    source_id,
                    page_index,
                    object_indices,
                    expected_text,
                    replacement,
                    bounds,
                    formatting,
                    reply,
                } => {
                    let _ = reply.send(self.preview_text_paragraph(
                        &source_id,
                        page_index,
                        &object_indices,
                        &expected_text,
                        &replacement,
                        &bounds,
                        &formatting,
                    ));
                }
                WorkerRequest::InspectTextGroup {
                    source_id,
                    page_index,
                    object_indices,
                    reply,
                } => {
                    let _ = reply.send(self.inspect_text_group(
                        &source_id,
                        page_index,
                        &object_indices,
                    ));
                }
                WorkerRequest::ReplaceTextGroup {
                    source_id,
                    page_index,
                    object_indices,
                    expected_text,
                    replacement,
                    reply,
                } => {
                    let _ = reply.send(self.replace_text_group(
                        &source_id,
                        page_index,
                        &object_indices,
                        &expected_text,
                        &replacement,
                    ));
                }
                WorkerRequest::ListTextRuns {
                    source_id,
                    page_index,
                    reply,
                } => {
                    let _ = reply.send(self.list_text_runs(&source_id, page_index));
                }
                WorkerRequest::ReplaceText {
                    source_id,
                    page_index,
                    object_index,
                    expected_text,
                    replacement,
                    font_id,
                    reply,
                } => {
                    let _ = reply.send(self.replace_text(
                        &source_id,
                        page_index,
                        object_index,
                        &expected_text,
                        &replacement,
                        font_id.as_deref(),
                    ));
                }
                WorkerRequest::CreateBlank {width,height,reply} => {
                    let _ = reply.send(self.create_blank_document(width,height));
                }
                WorkerRequest::CreateImages {paths,options,reply} => {
                    let result=image_pages::create(self.pdfium,&paths,&options)
                        .and_then(|bytes|self.register_generated_document(bytes,"Images.pdf"));
                    let _ = reply.send(result);
                }
                WorkerRequest::Open {
                    path,
                    password,
                    original_path,
                    editable_annotations,
                    register_fonts,
                    reply,
                } => {
                    let result = self.open_document(
                        path,
                        password.as_ref().map(|p|std::str::from_utf8(&p.0).expect("password originates as UTF-8")),
                        original_path,
                        editable_annotations,
                        register_fonts,
                    );
                    self.reply_document(reply, result);
                }
                WorkerRequest::OpenBytes { bytes, path, original_path, register_fonts, legacy_comment_import, policy, annotation_sources, images, font_imports, reply } => {
                    let result = self.recovery_security(bytes, policy)
                        .and_then(|(bytes,security)| self.open_bytes_scoped(bytes,path,original_path,true,register_fonts,legacy_comment_import,security,Some(&font_imports)))
                        .and_then(|document| self.restore_annotation_sources(document, annotation_sources))
                        .and_then(|document| {
                            // Recovery payloads were fully validated before publication. Give
                            // this source its own tokens, including on repeated restore attempts.
                            let restriction=self.documents.get(&document.id).and_then(|source|source.policy.as_ref()).and_then(SourcePolicy::image_restriction);
                            let registrations=match self.images.register_source_assets(&images,restriction) {
                                Ok(owned)=>owned,
                                Err(error)=>{self.discard_unpublished_source(&document.id);return Err(error);}
                            };
                            let source=self.documents.get_mut(&document.id).expect("newly opened source");
                            source.image_leases.get_or_insert_with(||ImageSourceLeases{registry:self.images.clone(),registrations:Vec::new()}).registrations.extend(registrations);
                            Ok(document)
                        });
                    self.reply_document(reply, result);
                }
                WorkerRequest::SplitPdf{request,parent,control,progress,resources,reply}=>{
                    let _=reply.send(self.split_pdf(request,parent,control,progress,resources));
                }
                WorkerRequest::ExportPageImages{request,parent,control,progress,resources,reply}=>{
                    let _=reply.send(self.export_page_images(request,parent,control,progress,resources));
                }
                WorkerRequest::ExportText{request,destination,control,progress,resources,reply}=>{
                    let _=reply.send(self.export_text(request,destination,control,progress,resources));
                }
                WorkerRequest::RenderPlan {plan,width,reply} => {
                    let _ = reply.send(self.render_page_plan(plan,width));
                }
                WorkerRequest::ValidateCrop {source_id,page_index,crop,reply} => {
                    let _=reply.send(self.validate_page_crop(&source_id,page_index,&crop));
                }
                WorkerRequest::Render {
                    source_id,
                    page_index,
                    width,
                    reply,
                } => {
                    let _ = reply.send(self.render_page(&source_id, page_index, width));
                }
                WorkerRequest::PreparePrint { request, reply } => {
                    let _ = reply.send(self.prepare_print(request));
                }
                WorkerRequest::Export {
                    request,
                    bookmarks,
                    destination,
                    reply,
                } => {
                    let _ = reply.send(self.export_pdf_with_bookmarks(request, bookmarks, destination));
                }
                WorkerRequest::Close { source_id, reply } => {
                    let _ = reply.send(self.close_document(&source_id));
                }
                WorkerRequest::InspectDocumentProperties{source_id,reply}=>{let _=reply.send(self.inspect_document_properties(&source_id));}
                WorkerRequest::ControlledExport { request, bookmarks, security, metadata, destination, monitor, reply } => {
                    let result = (|| {
                        monitor.check_cancelled()?;
                        let destination = self.validated_destination(&destination)?;
                        let (bytes,_) = self.compose_pdf_with_route_options(request, bookmarks, CompositionIntent::Save, Some(&monitor),security.as_ref(),true,metadata.as_ref())?;
                        monitor.checkpoint("writing", 0)?;
                        crate::pdf_output::publish_controlled(&destination, &bytes, &monitor.control, || monitor.emit("finishing", monitor.total))
                    })();
                    let _ = reply.send(result);
                }
                WorkerRequest::SourceRecoveryPolicy {source_id, reply} => {
                    let result = self.document(&source_id).and_then(|source| {
                        if source.for_printing { return Err(EngineError::PermissionDenied("recovering a prepared print document")); }
                        source.policy.as_ref().map_or(Ok(RecoveryPolicy::Unprotected),|p|p.snapshot(source.bytes()?.as_ref()))
                    });
                    let _ = reply.send(result);
                }
                WorkerRequest::SourceLegacyCommentImport { source_id, reply } => {
                    let _ = reply.send(self.document(&source_id).map(|source| source.legacy_comment_import));
                }
                WorkerRequest::SourceImportedOverlaySnapshot { source_id, limit, reply } => {
                    let result = self.document(&source_id).and_then(|source| {
                        if source.for_printing { return Err(EngineError::PermissionDenied("recovering a prepared print document")); }
                        source.imported_overlay_sources.snapshot_with_budget(limit)
                    });
                    let _ = reply.send(result);
                }
                WorkerRequest::SourceUnits {source_id,reply} => {
                    let _=reply.send(self.document(&source_id).and_then(|s|s.units().map(<[f32]>::to_vec)));
                }
                WorkerRequest::SourceBytes { source_id, reply } => {
                    let _ = reply.send(
                        self.document(&source_id)
                            .and_then(|source| source.bytes()),
                    );
                }
                WorkerRequest::SourceOriginalPath { source_id, reply } => {
                    let _ = reply.send(
                        self.document(&source_id)
                            .map(|source| source.original_path.clone()),
                    );
                }
                WorkerRequest::ExtractText {
                    source_id,
                    page_index,
                    reply,
                } => {
                    let _ = reply.send(self.extract_text(&source_id, page_index));
                }
                WorkerRequest::PageText {
                    source_id,
                    page_index,
                    reply,
                } => {
                    let _ = reply.send(self.page_text(&source_id, page_index));
                }
                WorkerRequest::Status { reply } => {
                    let _ = reply.send(self.status.clone());
                }
            }
            // Every request-local page/font/object has dropped. Multi-source
            // operations may exceed the idle bound while they are executing.
            self.trim_native_documents();
        }
    }

    fn create_blank_document(&mut self, width:f32, height:f32) -> EngineResult<DocumentInfo> {
        if ![width,height].iter().all(|n| n.is_finite() && (144.0..=1440.0).contains(n)) {
            return Err(EngineError::InvalidRequest("Page dimensions must be between 144 and 1440 points (2–20 inches).".into()));
        }
        let mut blank = self.pdfium.create_new_pdf()?;
        blank.pages_mut().create_page_at_end(PdfPagePaperSize::Custom(PdfPoints::new(width),PdfPoints::new(height)))?;
        let bytes = blank.save_to_bytes()?;
        drop(blank);
        self.register_generated_document(bytes,"Untitled.pdf")
    }

    fn fill_form_field(&mut self,source_id:&str,request:&crate::FillFormFieldRequest)->EngineResult<DocumentInfo> {
        let source=self.document(source_id)?;
        if source.for_printing || source.policy.as_ref().is_some_and(|p|!p.protection.permissions().fill_forms) {
            return Err(EngineError::PermissionDenied("filling forms"));
        }
        let policy=source.policy.clone();
        let original_path=source.original_path.clone();
        let legacy_comment_import=source.legacy_comment_import;
        let bytes=crate::form_fill_publish::fill(&self.pdfium,self.bindings,source.bytes()?.as_ref(),request)?;
        // Verify the serialized value in a fresh native document before ownership
        // is published. Source bytes and the runtime source remain untouched.
        if !crate::form_fill::NativeForm::open(self.bindings,&bytes)?.matches_value(request.page_index,request.annotation_index,&request.value)? {
            return Err(EngineError::InvalidRequest("The saved form value differs from the requested value.".into()));
        }
        let mut info=self.register_generated_document(bytes,"Filled form.pdf")?;
        info.protection=policy.as_ref().map(SourcePolicy::info);
        let result=(|| {
            let candidate=self.documents.get_mut(&info.id).unwrap();
            candidate.original_path=original_path;
            candidate.legacy_comment_import=legacy_comment_import;
            candidate.policy=policy;
            candidate.imported_overlay_sources=imported_overlay_sources::Index::capturing(candidate.source_bytes.identity().clone());
            let mut metadata=None;let mut fonts=vec![];let mut images=vec![];
            for index in 0..candidate.document.page_count() {
                let mut page=candidate.document.get(&candidate.imported_annotations)?.pages().get(page_index_i32(index)?)?;
                let geometry=page_geometry(&page)?;
                // Existing session overlays already own these annotations. Strip
                // their original appearances exactly as opening the source did.
                info.pages[index as usize].overlays=import_annotations(self.pdfium,&mut page,geometry,index as usize,candidate.bytes()?.as_ref(),&mut metadata,&mut fonts,&mut images,&mut candidate.imported_annotations,&mut candidate.imported_overlay_sources,candidate.legacy_comment_import)?;
            }
            candidate.image_leases=Some(ImageSourceLeases{registry:self.images.clone(),registrations:self.images.register_source_assets(&images,candidate.policy.as_ref().and_then(SourcePolicy::image_restriction))?});
            Ok(())
        })();
        if let Err(error)=result { self.discard_unpublished_source(&info.id); return Err(error); }
        self.retain_session_annotation_sources(source_id,info,None)
    }

    fn register_generated_document(&mut self, bytes:Vec<u8>, name:&str) -> EngineResult<DocumentInfo> {
        if bytes.len() as u64 > MAX_SOURCE_BYTES {
            return Err(EngineError::InvalidRequest("Generated PDF exceeds the 512 MiB limit.".into()));
        }
        let source_bytes:Arc<[u8]> = bytes.into();
        let retained = self.source_store.capture(source_bytes.clone())?;
        let document = self.pdfium.load_pdf_from_reader(Cursor::new(source_bytes.clone()),None)?;
        let pages = document.pages().iter().map(|page|page_geometry(&page).map(|g|g.displayed_size())).collect::<EngineResult<Vec<_>>>()?;
        let id = Uuid::new_v4().to_string();
        // Generated sources have no disk file. Keep a unique absolute identity for
        // the existing recovery/overwrite contract; recovery stores the bytes.
        let path = std::env::temp_dir().join(format!("folio-generated-{id}.pdf"));
        self.documents.insert(id.clone(),OpenDocument {
                physical_units:OnceLock::from(physical_units::read(&source_bytes,document.pages().len() as usize).map_err(|e|e.to_string())),
            legacy_comment_import:false,
            imported_annotations:HashMap::new(),
            imported_overlay_sources:imported_overlay_sources::Index::unbound(retained.identity().clone()),
            unpublished_fonts:vec![],
            image_leases:None,
            original_path:path.clone(),path,document: native_residency::ResidentDocument::new(document, self.pdfium, retained.clone()),source_bytes:retained,for_printing:false,print_max_dpi:None,policy:None,
            text_edit_restriction:OnceLock::new(),bookmarks:OnceLock::new(),
            form_admission:form_admission::Admission::default(),
        });
        Ok(DocumentInfo {id,name:name.into(),pages,bookmarks:vec![],bookmark_error:None,protection:None})
    }

    fn open_document(
        &mut self,
        path: PathBuf,
        password: Option<&str>,
        original_path: Option<PathBuf>,
        editable_annotations: bool,
        register_fonts: bool,
    ) -> EngineResult<DocumentInfo> {
        let (canonical,bytes)=Self::read_source_file(&path)?;
        let original_path = original_path.unwrap_or_else(|| canonical.clone());
        if !original_path.is_absolute()
            || original_path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(EngineError::InvalidRequest(
                "original source path is invalid".into(),
            ));
        }
        let (bytes, security) = self.authenticate_bytes(bytes, password)?;
        self.open_bytes(bytes, canonical, original_path, editable_annotations, register_fonts, false, security)
    }

    fn read_source_file(path:&Path)->EngineResult<(PathBuf,Vec<u8>)> {
        let canonical = std::fs::canonicalize(&path)
            .map_err(|error| EngineError::Io(format!("{}: {error}", path.display())))?;
        let metadata = canonical
            .metadata()
            .map_err(|error| EngineError::Io(error.to_string()))?;
        if !metadata.is_file() {
            return Err(EngineError::InvalidRequest("source is not a file".into()));
        }
        if metadata.len() > MAX_SOURCE_BYTES {
            return Err(EngineError::InvalidRequest(
                "source PDF exceeds the 512 MiB limit".into(),
            ));
        }
        // PDFium reads lazily: retain immutable bytes so edits to the original path
        // cannot change an open document or the source copied into recovery.
        let file = std::fs::File::open(&canonical)
            .map_err(|error| EngineError::Io(format!("{}: {error}", canonical.display())))?;
        let mut bytes = Vec::new();
        // A growing file must not bypass the metadata check or allocate without bound.
        file.take(MAX_SOURCE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| EngineError::Io(format!("{}: {error}", canonical.display())))?;
        if bytes.len() as u64 > MAX_SOURCE_BYTES {
            return Err(EngineError::InvalidRequest(
                "source PDF exceeds the 512 MiB limit".into(),
            ));
        }
        Ok((canonical,bytes))
    }

    fn is_unprotected(&self,bytes:&[u8])->bool {
        self.pdfium.load_pdf_from_byte_slice(bytes,None).ok().is_some_and(|doc|
            matches!(doc.permissions().security_handler_revision(),Ok(PdfSecurityHandlerRevision::Unprotected)))
    }

    fn authenticate_bytes(&self, bytes: Vec<u8>, password: Option<&str>) -> EngineResult<(Vec<u8>,OpenSecurity)> {
        // Preserve PDFium's tolerance for ordinary PDFs. Only a definite
        // Unprotected result bypasses the strict protection codec (including
        // empty-user-password inputs and revisions unknown to this wrapper).
        if self.is_unprotected(&bytes) { return Ok((bytes,OpenSecurity::Authenticated(None))); }
        let unlocked=crate::protection::unlock_pdf(&bytes,password)?;
        Ok((unlocked.bytes,OpenSecurity::Authenticated(unlocked.protection)))
    }

    fn recovery_security(&self, bytes: Vec<u8>, policy: Option<RecoveryPolicy>) -> EngineResult<(Vec<u8>,OpenSecurity)> {
        match policy {
            Some(RecoveryPolicy::Protected {..}) => {
                let restored=policy.unwrap().restore(&bytes)?;
                Ok((bytes,OpenSecurity::Restored(restored)))
            }
            Some(RecoveryPolicy::Unprotected) => {
                let (bytes,security)=self.authenticate_bytes(bytes,None)?;
                if !matches!(security,OpenSecurity::Authenticated(None)) {
                    return Err(EngineError::InvalidRequest("Recovery source is missing its protection context.".into()));
                }
                Ok((bytes,security))
            }
            None => self.authenticate_bytes(bytes,None), // Legacy recovery.
        }
    }

    // Verified recovery bytes stay in memory; paths are overwrite guards only.
    fn open_bytes(
        &mut self,
        bytes: Vec<u8>,
        canonical: PathBuf,
        original_path: PathBuf,
        editable_annotations: bool,
        register_fonts: bool,
        legacy_comment_import: bool,
        security: OpenSecurity,
    ) -> EngineResult<DocumentInfo> {
        self.open_bytes_scoped(bytes, canonical, original_path, editable_annotations,
            register_fonts, legacy_comment_import, security, None)
    }

    fn open_bytes_scoped(
        &mut self,
        bytes: Vec<u8>,
        canonical: PathBuf,
        original_path: PathBuf,
        editable_annotations: bool,
        register_fonts: bool,
        legacy_comment_import: bool,
        security: OpenSecurity,
        font_imports: Option<&crate::fonts::FontImportScope>,
    ) -> EngineResult<DocumentInfo> {
        self.open_bytes_admitted(bytes, canonical, original_path, editable_annotations,
            register_fonts, legacy_comment_import, security, font_imports, None)
    }

    fn open_text_publication(
        &mut self,
        publication: page_admission::Publication,
        canonical: PathBuf,
        original_path: PathBuf,
        legacy_comment_import: bool,
        security: OpenSecurity,
    ) -> EngineResult<DocumentInfo> {
        let (bytes, proof) = publication.into_parts();
        self.open_bytes_admitted(bytes, canonical, original_path, true, true,
            legacy_comment_import, security, None, proof)
    }

    fn open_bytes_admitted(
        &mut self,
        bytes: Vec<u8>,
        canonical: PathBuf,
        original_path: PathBuf,
        editable_annotations: bool,
        register_fonts: bool,
        legacy_comment_import: bool,
        security: OpenSecurity,
        font_imports: Option<&crate::fonts::FontImportScope>,
        admission: Option<page_admission::Proof>,
    ) -> EngineResult<DocumentInfo> {
        let _phase=crate::native_phase!("open.total");
        if bytes.is_empty() || bytes.len() as u64 > MAX_SOURCE_BYTES {
            return Err(EngineError::InvalidRequest("source PDF is empty or exceeds 512 MiB".into()));
        }
        for path in [&canonical, &original_path] {
            if !path.is_absolute() || path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                return Err(EngineError::InvalidRequest("source identity path is invalid".into()));
            }
        }
        let source_bytes: Arc<[u8]> = bytes.into();
        let retained = {let _phase=crate::native_phase!("open.retained_capture");self.source_store.capture(source_bytes.clone())?};
        let document = {let _phase=crate::native_phase!("open.renderer_load");self
            .pdfium
            .load_pdf_from_reader(Cursor::new(source_bytes.clone()), None)?};
        let size_only = admission.as_ref().is_some_and(|proof|
            proof.matches(&source_bytes, document.pages().len() as usize));
        let mut pages = Vec::with_capacity(document.pages().len() as usize);
        let mut imported_annotations=HashMap::new();
        let mut imported_overlay_sources=imported_overlay_sources::Index::capturing(retained.identity().clone());
        let mut annotation_metadata = None;
        let mut imported_fonts = Vec::new();
        let mut imported_images = Vec::new();
        {let _phase=crate::native_phase!("open.page_admission");
        for index in 0..document.pages().len() {
            if size_only {
                if let Some(info) = page_admission::native_size(&document,index) {
                    pages.push(info);
                    continue;
                }
            }
            let mut page = document.pages().get(index)?;
            let geometry = page_geometry(&page)?;
            let mut info = geometry.displayed_size();
            if editable_annotations {
                info.overlays = import_annotations(
                    self.pdfium,
                    &mut page,
                    geometry,
                    index as usize,
                    &source_bytes,
                    &mut annotation_metadata,
                    &mut imported_fonts,
                    &mut imported_images,
                    &mut imported_annotations,
                    &mut imported_overlay_sources,
                    legacy_comment_import,
                )?;
            }
            pages.push(info);
        }}
        let id = Uuid::new_v4().to_string();
        let name = canonical
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("document.pdf")
            .to_string();
        // PDFium can hide malformed outline roots. Inspect the original metadata
        // independently so saving never silently treats an unreadable tree as empty.
        let bookmark_result={let _phase=crate::native_phase!("open.bookmarks");crate::bookmarks::read(&source_bytes).map_err(|e|e.to_string())};
        let policy=match security {
            OpenSecurity::Authenticated(protection)=>protection.map(|p|SourcePolicy::new(p,&pages,&bookmark_result)),
            OpenSecurity::Restored(policy)=>policy,
        };
        if let Some(policy)=&policy {policy.validate_pages(pages.len())?;}
        let info = DocumentInfo {
            protection:policy.as_ref().map(SourcePolicy::info),
            id: id.clone(),
            name,
            pages,
            bookmarks:bookmark_result.as_ref().cloned().unwrap_or_default(),
            bookmark_error:bookmark_result.as_ref().err().cloned(),
        };
        // No global font registration occurs until every page has opened and
        // imported successfully. The batch is atomic even when capacity is exceeded.
        let image_leases = if register_fonts {
            Some(ImageSourceLeases {registry:self.images.clone(), registrations:self.images.register_source_assets(&imported_images,policy.as_ref().and_then(SourcePolicy::image_restriction))?})
        } else { None };
        let unpublished_fonts = if register_fonts {
            if let Some(scope) = font_imports {
                scope.register_assets(&imported_fonts)?;
                vec![] // Recovery owns rollback, including repeated imports.
            } else {
                self.fonts.register_assets(&imported_fonts)?
            }
        } else { vec![] };
        self.documents.insert(
            id,
            OpenDocument {
                physical_units:{let _phase=crate::native_phase!("open.physical_units");OnceLock::from(physical_units::read(&source_bytes,document.pages().len() as usize).map_err(|e|e.to_string()))},
                legacy_comment_import,
                imported_annotations,
                imported_overlay_sources,
                unpublished_fonts,
                image_leases,
                path: canonical,
                original_path,
                document: native_residency::ResidentDocument::new(document, self.pdfium, retained.clone()),
                source_bytes: retained,
                for_printing: !editable_annotations,
                print_max_dpi: None,
                policy,
                text_edit_restriction: OnceLock::new(),
                form_admission:form_admission::Admission::default(),
                bookmarks:OnceLock::from(bookmark_result),
            },
        );
        Ok(info)
    }

    fn prepare_print(&mut self, request: ExportRequest) -> EngineResult<DocumentInfo> {
        let low_quality=request.pages.iter().any(|p|self.document(&p.source_id).ok()
            .and_then(|s|s.policy.as_ref()).is_some_and(|p|!p.protection.permissions().print_high_quality));
        let bytes = self.compose_pdf(request, Some(vec![]), CompositionIntent::Print)?;
        // Identity only. No file is created or reopened at this path.
        let path = std::env::temp_dir().join(format!("Folio-print-{}.pdf", Uuid::new_v4()));
        let info=self.open_bytes(bytes, path.clone(), path, false, false,false,OpenSecurity::Restored(None))?;
        if low_quality {self.documents.get_mut(&info.id).unwrap().print_max_dpi=Some(150);}
        Ok(info)
    }

    fn render_page_plan(&mut self, plan:PagePlan, width:u32) -> EngineResult<Vec<u8>> {
        let view=self.prepare_read_page(plan,CompositionIntent::Preview,Arc::new(||false))?;
        // Raw bytes retain annotations for the complete-page preview.
        let document=self.pdfium.load_pdf_from_reader(Cursor::new(view.bytes),None)?;
        let page=document.pages().get(page_index_i32(view.page_index)?)?;
        let target_width=page_crop::preview_width(page.width().value,page.height().value,width)?;
        let bitmap=page.render_with_config(&PdfRenderConfig::new()
            .set_target_width(target_width).set_maximum_height(12_000).render_form_data(true))?;
        let mut png=Cursor::new(Vec::new());
        bitmap.as_image()?.write_to(&mut png,ImageFormat::Png)
            .map_err(|error|EngineError::Io(error.to_string()))?;
        Ok(png.into_inner())
    }

    fn render_page(&self, source_id: &str, page_index: usize, width: u32) -> EngineResult<Vec<u8>> {
        let document = self.document(source_id)?;
        let page = document.pdf()?.pages().get(page_index_i32(page_index)?)?;
        let mut target_width = width.clamp(64, 2400) as i32;
        if let Some(dpi)=document.print_max_dpi {
            let unit=*document.units()?.get(page_index).ok_or_else(||EngineError::InvalidRequest("Print page is outside source.".into()))?;
            target_width=target_width.min((page.width().value*unit*dpi as f32/72.0).floor().max(1.0) as i32);
        }
        let bitmap = page.render_with_config(
            &PdfRenderConfig::new()
                .set_target_width(target_width)
                .use_print_quality(document.for_printing)
                .render_form_data(true),
        )?;
        let image = bitmap.as_image()?;
        let mut png = Cursor::new(Vec::new());
        image
            .write_to(&mut png, ImageFormat::Png)
            .map_err(|error| EngineError::Io(error.to_string()))?;
        Ok(png.into_inner())
    }

    fn extract_text(&self, source_id: &str, page_index: usize) -> EngineResult<String> {
        let document = self.copying_document(source_id)?;
        #[cfg(feature="shaped-text")]
        if let Some(text)=self.read_ocr_page_text(source_id,page_index,None)? {return Ok(text.characters.into_iter().map(|c|c.text).collect());}
        let page = document.pdf()?.pages().get(page_index_i32(page_index)?)?;
        let bounds = page.boundaries().bounding()?.bounds;
        let text = page.text()?.inside_rect(bounds);
        Ok(text)
    }

    fn page_text(&self, source_id: &str, page_index: usize) -> EngineResult<PageText> {
        self.copying_document(source_id)?;
        self.read_page_text(source_id,page_index,None)
    }

    fn read_page_text(&self, source_id:&str,page_index:usize,character_limit:Option<usize>)->EngineResult<PageText> {
        let document=self.document(source_id)?;
        #[cfg(feature="shaped-text")]
        if let Some(text)=self.read_ocr_page_text(source_id,page_index,character_limit)?{return Ok(text);}
        self.read_document_page_text(document.pdf()?,page_index,character_limit)
    }

    fn read_document_page_text(&self,document:&PdfDocument<'_>,page_index:usize,character_limit:Option<usize>)->EngineResult<PageText> {
        let page = document.pages().get(page_index_i32(page_index)?)?;
        let geometry = page_geometry(&page)?;
        let source_text = page.text()?;
        if character_limit.is_some_and(|limit|source_text.chars().len() as usize>limit){
            return Err(EngineError::InvalidRequest("This page exceeds the 250,000-character reading limit.".into()));
        }
        // PDFium sorts separate text-show objects by rotated display X, which
        // reverses collinear segments on 180-degree pages. Read in source axes
        // but map character boxes through the ORIGINAL displayed geometry.
        // Import only this page into a temporary document: changing /Rotate on
        // the open source would mutate its dictionary and could affect export.
        let mut reading_document =
            if geometry.rotation != 0 && simple_forward_text(&page, &source_text) {
                Some(self.pdfium.create_new_pdf()?)
            } else {
                None
            };
        let mut reading_page = None;
        if let Some(copy) = reading_document.as_mut() {
            copy.pages_mut().copy_page_from_document(
                document,
                page_index_i32(page_index)?,
                0,
            )?;
            let mut copied_page = copy.pages().get(0)?;
            copied_page.set_rotation(PdfPageRenderRotation::None);
            reading_page = Some(copied_page);
        }
        let text = if let Some(reading_page) = reading_page.as_ref() {
            reading_page.text()?
        } else {
            source_text
        };
        let chars = text.chars();
        if character_limit.is_some_and(|limit|chars.len() as usize>limit) {
            return Err(EngineError::InvalidRequest("This page exceeds the 250,000-character search limit.".into()));
        }
        let mut characters: Vec<PdfTextCharacter> = Vec::with_capacity(chars.len() as usize);
        let mut pending_high_surrogate: Option<u32> = None;
        let mut last_position = Point { x: 0.0, y: 0.0 };
        for character in chars.iter() {
            let unicode = character.unicode_value();
            let value = char::from_u32(unicode).unwrap_or(char::REPLACEMENT_CHARACTER);
            // Generated whitespace can lack glyph bounds. Keep it in reading order
            // so browser range copying retains word and line separators.
            let bounds = if value.is_whitespace() {
                character
                    .loose_bounds()
                    .or_else(|_| character.tight_bounds())
            } else {
                character
                    .tight_bounds()
                    .or_else(|_| character.loose_bounds())
            };
            let (position, width, height) = match bounds {
                Ok(bounds) => {
                    let a = geometry.pdf_to_displayed(bounds.left().value, bounds.bottom().value);
                    let b = geometry.pdf_to_displayed(bounds.right().value, bounds.top().value);
                    (
                        Point {
                            x: a.x.min(b.x),
                            y: a.y.min(b.y),
                        },
                        (a.x - b.x).abs(),
                        (a.y - b.y).abs(),
                    )
                }
                Err(_) => {
                    let position = character
                        .origin()
                        .map(|(x, y)| geometry.pdf_to_displayed(x.value, y.value))
                        .unwrap_or(last_position);
                    (position, 0.0, 0.0)
                }
            };
            last_position = position;
            // PDFium can expose a ToUnicode/ActualText supplementary scalar as
            // two UTF-16 entries. Selection must carry the same Unicode as
            // extract_text(), with one box covering both entries. An isolated
            // surrogate retains the replacement character; never skip a unit.
            if let Some(high) = pending_high_surrogate.take() {
                if (0xdc00..=0xdfff).contains(&unicode) {
                    let previous = characters
                        .last_mut()
                        .expect("a pending surrogate has an entry");
                    previous.text =
                        char::from_u32(0x10000 + ((high - 0xd800) << 10) + unicode - 0xdc00)
                            .expect("a surrogate pair encodes a Unicode scalar")
                            .to_string();
                    let right = (previous.x + previous.width).max(position.x + width);
                    let bottom = (previous.y + previous.height).max(position.y + height);
                    previous.x = previous.x.min(position.x);
                    previous.y = previous.y.min(position.y);
                    previous.width = right - previous.x;
                    previous.height = bottom - previous.y;
                    continue;
                }
            }
            if (0xd800..=0xdbff).contains(&unicode) {
                pending_high_surrogate = Some(unicode);
            }
            characters.push(PdfTextCharacter {
                text: value.to_string(),
                x: position.x,
                y: position.y,
                width,
                height,
            });
        }
        Ok(PageText {
            intrinsic_rotation: geometry.rotation,
            characters,
        })
    }

    fn close_document(&mut self, source_id: &str) -> EngineResult<()> {
        self.detach_redaction_document(source_id);
        #[cfg(feature="shaped-text")]
        self.ocr_reading.forget(source_id);
        if self.documents.remove(source_id).is_some() {
            Ok(())
        } else {
            Err(EngineError::InvalidRequest(format!(
                "unknown sourceId {source_id}"
            )))
        }
    }

    fn discard_unpublished_source(&mut self, source_id: &str) {
        self.detach_redaction_document(source_id);
        self.discard_unpublished_source_unhooked(source_id);
    }
    fn discard_unpublished_source_unhooked(&mut self, source_id: &str) {
        self.paragraph_contexts.remove(source_id);
        if let Some(source) = self.documents.remove(source_id) {
            let _ = self.fonts.rollback_imports(&source.unpublished_fonts);
            // Dropping the source releases its independent image lease guard.
        }
    }
    fn reply_document(&mut self, reply: mpsc::Sender<EngineResult<DocumentInfo>>, result: EngineResult<DocumentInfo>) {
        if let Err(mpsc::SendError(Ok(document))) = reply.send(result) {
            self.discard_unpublished_source(&document.id);
        }
    }

    fn export_pdf_with_bookmarks(&mut self, request:ExportRequest, bookmarks:Option<Vec<crate::DocumentBookmark>>, destination:PathBuf)->EngineResult<()> {
        let destination = self.validated_destination(&destination)?;
        let bytes = self.compose_pdf(request, bookmarks, CompositionIntent::Save)?;
        crate::pdf_output::publish(&destination, &bytes)
    }

    fn compose_pdf(&mut self, request: ExportRequest, bookmarks: Option<Vec<crate::DocumentBookmark>>, intent: CompositionIntent) -> EngineResult<Vec<u8>> {
        self.compose_pdf_monitored(request, bookmarks, intent, None,None)
    }

    fn compose_pdf_monitored(&mut self, request: ExportRequest, bookmarks: Option<Vec<crate::DocumentBookmark>>, intent: CompositionIntent, monitor: Option<&crate::pdf_save::PdfSaveMonitor>,security:Option<&crate::ProtectionChange>) -> EngineResult<Vec<u8>> {
        self.compose_pdf_with_route(request,bookmarks,intent,monitor,security).map(|(bytes,_)|bytes)
    }

    // The route receipt lets owned previews disclose losses before inspection.
    fn compose_pdf_with_route(&mut self, request: ExportRequest, bookmarks: Option<Vec<crate::DocumentBookmark>>, intent: CompositionIntent, monitor: Option<&crate::pdf_save::PdfSaveMonitor>,security:Option<&crate::ProtectionChange>) -> EngineResult<(Vec<u8>,bool)> {
        self.compose_pdf_with_route_metadata(request,bookmarks,intent,monitor,security,true)
    }
    // Redaction's private working copy keeps Save authorization but deliberately
    // omits document metadata during composition before its final stripping pass.
    fn compose_pdf_with_route_metadata(&mut self, request: ExportRequest, bookmarks: Option<Vec<crate::DocumentBookmark>>, intent: CompositionIntent, monitor: Option<&crate::pdf_save::PdfSaveMonitor>,security:Option<&crate::ProtectionChange>,preserve_metadata:bool) -> EngineResult<(Vec<u8>,bool)> {
        self.compose_pdf_with_route_options(request,bookmarks,intent,monitor,security,preserve_metadata,None)
    }
    fn compose_pdf_with_route_options(&mut self, request:ExportRequest,bookmarks:Option<Vec<crate::DocumentBookmark>>,intent:CompositionIntent,monitor:Option<&crate::pdf_save::PdfSaveMonitor>,security:Option<&crate::ProtectionChange>,preserve_metadata:bool,metadata:Option<&crate::DocumentMetadataEdit>)->EngineResult<(Vec<u8>,bool)>{
        let check = |stage, completed| monitor.map_or(Ok(()), |m| m.checkpoint(stage, completed));
        check("checking", 0)?;
        self.validate_export(&request)?;

        if let Some(metadata)=metadata{metadata.validate()?;self.authorize_metadata_edit(&request)?;}
        let preserve_metadata=preserve_metadata&&!metadata.is_some_and(crate::DocumentMetadataEdit::clears_existing);

        let discard_bookmarks=bookmarks.as_ref().is_some_and(Vec::is_empty);
        let bookmarks=match bookmarks {
            Some(items)=>items,
            None=>self.inherited_bookmarks_controlled(&request,&mut ||monitor.map_or(Ok(()),|m|m.check_cancelled()))?,
        };
        crate::bookmarks::validate(&bookmarks,&request.pages.iter().map(|p|p.id.clone()).collect())?;

        if let Some(security)=security{security.validate()?;}
        let protection=self.authorize_composition_security(&request,&bookmarks,intent,security.is_some())?;
        let retained = if intent == CompositionIntent::Save {
            self.complete_source_save(&request, &bookmarks, monitor)?
        } else { None };
        let retained_complete_source=retained.is_some();
        let bytes = match retained {
            Some(bytes) => bytes,
            None => if preserve_metadata {
                self.compose_imported_pdf(&request, &bookmarks, discard_bookmarks, intent, monitor)?
            } else {
                self.compose_imported_pdf_metadata(&request, &bookmarks, discard_bookmarks, intent, false, monitor)?
            },
        };
        check("preparingFile", 0)?;
        let bytes=if let Some(metadata)=metadata{
            let control=monitor.map(|m|m.control.clone());
            let(mut document,_)=lopdf::Document::load_mem_with_resource_control(&bytes,lopdf::ResourceLimits::default(),Arc::new(move||control.as_ref().is_some_and(|c|c.cancelled()))).map_err(crate::document_properties::bad)?;
            drop(bytes);
            crate::document_properties::apply(&mut document,metadata,&mut||monitor.map_or(Ok(()),|m|m.check_cancelled()))?;
            crate::pdf_output::save_controlled(&mut document,monitor.map(|m|m.control.as_ref()))?
        }else{bytes};
        let bytes = if let Some(security)=security {
            check("securing",0)?;
            match security {crate::ProtectionChange::Remove{}=>bytes,_=>security.apply(&bytes)?}
        } else {match protection {Some(p)=>p.protect(&bytes)?,None=>bytes}};
        check("preparingFile", 0)?;
        Ok((bytes,retained_complete_source))
    }

    // Page-import composition for requests that cannot retain one complete source.
    fn compose_imported_pdf(&self, request: &ExportRequest, bookmarks: &[crate::DocumentBookmark], discard_bookmarks:bool, intent: CompositionIntent, monitor: Option<&crate::pdf_save::PdfSaveMonitor>) -> EngineResult<Vec<u8>> {
        self.compose_imported_pdf_metadata(request,bookmarks,discard_bookmarks,intent,true,monitor)
    }
    fn compose_imported_pdf_metadata(&self, request: &ExportRequest, bookmarks: &[crate::DocumentBookmark], discard_bookmarks:bool, intent: CompositionIntent, preserve_metadata:bool, monitor: Option<&crate::pdf_save::PdfSaveMonitor>) -> EngineResult<Vec<u8>> {
        let check = |stage, completed| monitor.map_or(Ok(()), |m| m.checkpoint(stage, completed));
        if matches!(intent,CompositionIntent::Preview|CompositionIntent::Print|CompositionIntent::ImageExport) {
            let mut imported=false;let mut bound=true;
            for page in &request.pages {
                check("checking",0)?;
                let source=self.document(&page.source_id)?;
                let has_imports=source.imported_annotations.get(&page.page_index).is_some_and(|v|!v.is_empty());
                imported|=has_imports;
                if has_imports || !page.overlays.is_empty() {bound&=source.imported_overlay_sources.bound_to(source.source_bytes.identity());}
            }
            if imported && bound {
                // Reuse the exact owned APs for unchanged drawings, including
                // after crop/rotation/unit conversions. Regenerating every AP
                // from public coordinates can change the displayed pixels.
                match self.structured_source_save(request,bookmarks,discard_bookmarks,false,monitor) {
                    Ok(bytes)=>return Ok(bytes),
                    Err(EngineError::InvalidRequest(_))=>{
                        // Display/print can still render graphs that Save does
                        // not support. This is the existing visual-only route,
                        // never a publication fallback or an ownership grant.
                        // A cancellation must not be reinterpreted as that case.
                        if let Some(monitor)=monitor {monitor.check_cancelled()?;}
                    },
                    Err(error)=>return Err(error),
                }
            }
        }
        let mut form_leases = HashMap::new();
        for plan in &request.pages {
            if !form_leases.contains_key(&plan.source_id) {
                form_leases.insert(plan.source_id.clone(), self.document(&plan.source_id)?.bytes()?);
            }
        }
        let mut form_sources=Vec::<crate::acroforms::SourceForm<'_>>::new();
        let mut form_indices=HashMap::new();
        for (output_index,plan) in request.pages.iter().enumerate() {
            let index=if let Some(index)=form_indices.get(&plan.source_id){*index}else{
                let index=form_sources.len();form_indices.insert(plan.source_id.clone(),index);
                form_sources.push(crate::acroforms::SourceForm::new(&form_leases[&plan.source_id], vec![]));index
            };
            form_sources[index].pages.push((plan.page_index,output_index));
        }
        if form_sources.len() == 1 { form_sources[0].enable_parse_reuse(); }
        if intent == CompositionIntent::Save
            && complete_source_save::requires_graph_composition(&form_sources, || {
                monitor.map_or(Ok(()), |m| m.check_cancelled())
            })?
        {
            // This route returns before any PDFium import or graph repair.
            // Unsupported graph operations remain errors, never a lossy fallback.
            return self.structured_source_save(request, bookmarks, discard_bookmarks, preserve_metadata, monitor);
        }
        // Bound widget expansion before PDFium duplicates page objects.
        crate::acroforms::preflight(&form_sources)?;
        check("checking", 0)?;
        let link_navigation=crate::bookmarks::links::prepare(&form_sources)?;

        let mut output = self.pdfium.create_new_pdf()?;
        check("copying", 0)?;
        // Repair import references before loading pages or adding overlays. A
        // resource named First/Prev/Parent is legal but skipped by PDFium's import.
        for plan in &request.pages {
            let destination_index = output.pages().len();
            let source = self.document(&plan.source_id)?;
            output.pages_mut().copy_page_from_document(
                source.pdf()?, page_index_i32(plan.page_index)?, destination_index,
            )?;
            check("copying", destination_index as usize + 1)?;
        }
        check("preserving", 0)?;
        if crate::page_resources::needed(&form_sources)? {
            check("preserving", 0)?;
            if let Some(bytes) = crate::page_resources::preserve_bytes(&crate::pdf_output::from_pdfium(&output)?, &form_sources)? {
                output = self.pdfium.load_pdf_from_reader(Cursor::new(bytes), None)?;
            }
        }
        let mut geometries = Vec::with_capacity(request.pages.len());
        let mut highlight_publication = highlight_appearance::Publication::default();

        check("applying", 0)?;
        for (destination_index, plan) in request.pages.iter().enumerate() {
            // A page containing text paints every text/ink overlay through
            // the portable appearance path, retaining their relative order.
            let portable_flatten = request.flatten
                && plan.overlays.iter().any(
                    |overlay| matches!(overlay, Overlay::Text(_) | Overlay::Image(_)),
                );
            let mut page = output.pages().get(page_index_i32(destination_index)?)?;
            let geometry = page_geometry(&page)?;
            geometries.push(geometry);
            for (overlay_index,overlay) in plan.overlays.iter().enumerate() {
                if let Some(monitor) = monitor { monitor.check_cancelled()?; }
                match overlay {
                    Overlay::Image(_) => {},
                    Overlay::Text(_) => {},
                    Overlay::Ink(ink) => {
                        if request.flatten && !portable_flatten {
                            add_ink_overlay(&mut page, geometry, ink)?;
                        }
                    }
                    Overlay::Highlight(highlight) => {
                        highlight_publication.record(destination_index,page.annotations().len() as usize,overlay_index)?;
                        add_highlight_annotation(&mut page, geometry, highlight)?
                    }
                    Overlay::Comment(comment) => {
                        let unit=self.document(&plan.source_id)?.units()?[plan.page_index];
                        add_comment_annotation(&mut page, geometry, comment,unit)?
                    }
                }
            }
            page.set_rotation(rotation_from_degrees(
                (geometry.rotation + plan.rotation) % 360,
            )?);
            page.regenerate_content()?;
            check("applying", destination_index + 1)?;
        }

        check("preparingFile", 0)?;
        let bytes = crate::pdf_output::from_pdfium(&output)?;
        drop(output);
        check("preserving", 0)?;
        let mut document = crate::pdf_output::load(&bytes)?;
        let generated_highlights = highlight_publication.apply(self.pdfium,&mut document,&request,&geometries,&mut ||monitor.map_or(Ok(()),|m|m.check_cancelled()))?;
        let comment_sources = form_indices.iter().map(|(id,index)| Ok(native_comments::SourceComments {
            source: &form_sources[*index],
            excluded: &self.document(id)?.imported_annotations,
        })).collect::<EngineResult<Vec<_>>>()?;
        let mut changed = generated_highlights | native_comments::preserve_geometry(&mut document, &comment_sources)?;
        check("preserving", 0)?;
        changed |= native_comments::repair_popup_parents(&mut document)?;
        check("preserving", 0)?;
        changed |= crate::acroforms::preserve(&mut document, &form_sources)?;
        check("preserving", 0)?;
        changed |= link_navigation.apply(&mut document)?;
        check("preserving", 0)?;
        if request.pages.iter().any(|page| {
            page.overlays.iter().any(|overlay| {
                !request.flatten
                    || matches!(overlay, Overlay::Highlight(_) | Overlay::Comment(_))
                    || matches!(overlay, Overlay::Text(_) | Overlay::Image(_))
            })
        }) {
            persistence::finalize(&mut document, &request, &geometries, &self.fonts, &self.images,matches!(intent,CompositionIntent::Save),|page,overlay|{
                monitor.map_or(Ok(()),|m|if overlay.is_some(){m.check_cancelled()}else{m.checkpoint("renderingEdits",page)})
            })?;
            changed = true;
        }
        if !bookmarks.is_empty() {
            crate::bookmarks::write_document(&mut document, &bookmarks,
                &request.pages.iter().map(|p|p.id.clone()).collect::<Vec<_>>())?;
            changed = true;
        }
        changed |= page_crop::apply(&mut document, &request.pages, &geometries)?;
        if intent == CompositionIntent::Save && preserve_metadata {
            let leading=form_sources.first().ok_or_else(||EngineError::InvalidRequest("Missing metadata source".into()))?.document()?;
            changed |= crate::document_metadata::preserve_imported(&mut document,&leading,&mut ||monitor.map_or(Ok(()),|m|m.check_cancelled()))?;
        }
        drop(comment_sources);
        drop(form_sources); // Release retained source parses before serialization/protection.
        check("preparingFile", 0)?;
        let bytes=if changed {
            // The parsed document owns its streams. Keeping the superseded PDF
            // buffer while serializing another copy inflates large-file peaks.
            drop(bytes);
            crate::pdf_output::save_controlled(&mut document, monitor.map(|m| m.control.as_ref()))?
        } else { bytes };
        // Protection reparses the serialized output; the composition tree is no
        // longer needed and can otherwise duplicate every image stream again.
        drop(document);
        Ok(bytes)
    }

    fn document(&self, source_id: &str) -> EngineResult<&OpenDocument> {
        self.documents
            .get(source_id)
            .ok_or_else(|| EngineError::InvalidRequest(format!("unknown sourceId {source_id}")))
    }

    fn validated_destination(&self, destination: &Path) -> EngineResult<PathBuf> {
        self.validated_export_destination(destination,"pdf")
    }
    fn validated_export_destination(&self, destination: &Path, extension:&str) -> EngineResult<PathBuf> {
        let file_name = destination.file_name().ok_or_else(|| {
            EngineError::InvalidRequest("destination must name a PDF file".into())
        })?;
        if destination
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            != Some(extension.to_string())
        {
            return Err(EngineError::InvalidRequest(
                format!("destination must use the .{extension} extension"),
            ));
        }
        let parent = destination.parent().unwrap_or_else(|| Path::new("."));
        let parent = std::fs::canonicalize(parent)
            .map_err(|error| EngineError::Io(format!("{}: {error}", parent.display())))?;
        let resolved = parent.join(file_name);
        let normalized = if resolved.exists() {
            normalize_path(
                &std::fs::canonicalize(&resolved)
                    .map_err(|error| EngineError::Io(error.to_string()))?,
            )
        } else {
            normalize_path(&resolved)
        };
        if self.documents.values().any(|document| {
            [&document.path, &document.original_path]
                .into_iter()
                .any(|source_path| {
                    normalize_path(source_path) == normalized
                        || (resolved.exists()
                            && same_file::is_same_file(source_path, &resolved).unwrap_or(false))
                })
        }) {
            return Err(EngineError::InvalidRequest(
                "export destination cannot overwrite an open source PDF".into(),
            ));
        }
        Ok(resolved)
    }

    fn validate_export(&self, request: &ExportRequest) -> EngineResult<()> {
        if request.pages.is_empty() {
            return Err(EngineError::InvalidRequest(
                "export requires at least one page".into(),
            ));
        }
        if request.pages.len() > 10_000 {
            return Err(EngineError::InvalidRequest("too many pages".into()));
        }

        let mut page_ids = HashSet::new();
        for plan in &request.pages {
            let mut overlay_ids = HashSet::new();
            validate_id("page id", &plan.id)?;
            if !page_ids.insert(plan.id.as_str()) {
                return Err(EngineError::InvalidRequest(format!(
                    "duplicate page id {}",
                    plan.id
                )));
            }
            if !matches!(plan.rotation, 0 | 90 | 180 | 270) {
                return Err(EngineError::InvalidRequest(format!(
                    "invalid rotation {}",
                    plan.rotation
                )));
            }
            let document = self.document(&plan.source_id)?;
            let page = document
                .pdf()?
                .pages()
                .get(page_index_i32(plan.page_index)?)?;
            let size = page_geometry(&page)?.displayed_size();
            if !finite_positive(plan.width)
                || !finite_positive(plan.height)
                || (plan.width - size.width).abs() > 0.25
                || (plan.height - size.height).abs() > 0.25
            {
                return Err(EngineError::InvalidRequest(format!(
                    "page {} dimensions do not match its source page",
                    plan.id
                )));
            }
            if plan.overlays.len() > 10_000 {
                return Err(EngineError::InvalidRequest("too many overlays".into()));
            }
            page_crop::validate(plan.crop.as_ref(), size.width, size.height,
                document.units()?[plan.page_index].recip())?;
            for overlay in &plan.overlays {
                let id = match overlay {
                    Overlay::Image(image) => &image.id,
                    Overlay::Text(text) => &text.id,
                    Overlay::Ink(ink) => &ink.id,
                    Overlay::Highlight(highlight) => &highlight.id,
                    Overlay::Comment(comment) => &comment.id,
                };
                validate_id("overlay id", id)?;
                if !overlay_ids.insert(id.as_str()) {
                    return Err(EngineError::InvalidRequest(format!(
                        "duplicate overlay id {id}"
                    )));
                }
                match overlay {
                    Overlay::Image(image) => { image.validate(size.width,size.height)?; self.images.authorize(&image.asset_id,document.policy.as_ref().map(SourcePolicy::image_origin))?; },
                    Overlay::Text(text) => {
                        let custom = text
                            .font_id
                            .as_deref()
                            .map(|id| self.fonts.get(id))
                            .transpose()?;
                        validate_text_in_units(text, size.width, size.height, custom.as_deref(),self.source_page_unit(&plan.source_id,plan.page_index)?)?;
                    }
                    Overlay::Ink(ink) => validate_ink(ink, size.width, size.height,self.source_page_unit(&plan.source_id,plan.page_index)?)?,
                    Overlay::Highlight(highlight) => validate_highlight(highlight)?,
                    Overlay::Comment(comment) => validate_comment(comment)?,
                }
            }
        }
        Ok(())
    }
}

fn page_geometry(page: &PdfPage<'_>) -> EngineResult<PageGeometry> {
    let bounds = page.boundaries().bounding()?.bounds;
    Ok(PageGeometry {
        left: bounds.left().value,
        bottom: bounds.bottom().value,
        right: bounds.right().value,
        top: bounds.top().value,
        rotation: page.rotation()?.as_degrees() as u16,
    })
}

#[derive(Clone)]
struct AnnotationMetadata {
    id: Option<lopdf::ObjectId>,
    parent: Option<lopdf::ObjectId>,
    popup: Option<lopdf::ObjectId>,
    subtype: Vec<u8>,
    color: Option<String>,
    opacity: Option<f32>,
    owned: Option<persistence::PortableRecord>,
    highlight_appearance: highlight_appearance::Evidence,
    semantic_editable: bool,
    page_unit: f32,
}

struct SourceAnnotationMetadata {
    pages: HashMap<usize, Vec<AnnotationMetadata>>,
    appearance_budget: highlight_appearance::Budget,
}

#[cfg(test)]
#[path="tagged_overlay_import_tests.rs"]
mod tagged_overlay_import_tests;

#[path = "annotation_import_budget.rs"]
mod annotation_import_budget;
#[path="annotation_import_semantics.rs"]
mod annotation_import_semantics;

fn parse_annotation_metadata(bytes: &[u8]) -> Result<SourceAnnotationMetadata, lopdf::Error> {
    let document = lopdf::Document::load_mem_with_resource_limits(
        bytes,
        lopdf::ResourceLimits {
            input_bytes: MAX_SOURCE_BYTES as usize,
            object_bytes: 256 * 1024 * 1024,
            xref_entries: 100_000,
            ..Default::default()
        },
    )?;
    let source_pages = document.get_pages();
    annotation_import_budget::check(&document, &source_pages)?;
    let mut pages = HashMap::new();
    let mut fonts = persistence::ImportCache::default();
    let mut images = crate::image_persistence::ImageImportCache::new(&document);
    let mut appearance_budget = highlight_appearance::Budget::default();
    let mut semantics:Option<Option<annotation_import_semantics::Inspection<'_>>>=None;
    let structured_source=document.catalog().map_or(true,|catalog|catalog.has(b"StructTreeRoot"));
    for (number, page_id) in source_pages {
        let page = document.get_dictionary(page_id)?;
        let Ok(annotations) = page.get(b"Annots") else {
            continue;
        };
        let mut metadata = Vec::new();
        for annotation in document.dereference(annotations)?.1.as_array()? {
            let (id, annotation) = document.dereference(annotation)?;
            let dictionary = annotation.as_dict()?;
            let subtype = dictionary.get(b"Subtype")?.as_name()?.to_vec();
            let color = match dictionary.get(b"C") {
                Ok(color) => color
                    .as_array()
                    .ok()
                    .filter(|values| matches!(values.len(), 1 | 3 | 4))
                    .and_then(|values| {
                        values
                            .iter()
                            .map(|value| value.as_float().ok())
                            .collect::<Option<Vec<_>>>()
                    })
                    .and_then(|values| {
                        if values
                            .iter()
                            .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
                        {
                            return None;
                        }
                        let rgb = match values.as_slice() {
                            [gray] => [*gray, *gray, *gray],
                            [r, g, b] => [*r, *g, *b],
                            [c, m, y, k] => [
                                (1.0 - c) * (1.0 - k),
                                (1.0 - m) * (1.0 - k),
                                (1.0 - y) * (1.0 - k),
                            ],
                            _ => return None,
                        };
                        Some(format!(
                            "#{:02x}{:02x}{:02x}",
                            (rgb[0] * 255.0).round() as u8,
                            (rgb[1] * 255.0).round() as u8,
                            (rgb[2] * 255.0).round() as u8
                        ))
                    }),
                Err(_) => Some(
                    if subtype == b"Text" {
                        "#ffcc00"
                    } else {
                        "#ffff00"
                    }
                    .to_owned(),
                ),
            };
            let opacity = match dictionary.get(b"CA") {
                Ok(value) => value
                    .as_float()
                    .ok()
                    .filter(|value| value.is_finite() && (0.0..=1.0).contains(value)),
                Err(_) => Some(1.0),
            };
            let sensitive=annotation_import_semantics::sensitive(dictionary);
            let inspection=if sensitive || structured_source {semantics.get_or_insert_with(||annotation_import_semantics::Inspection::inspect(&document)).as_ref()}else{None};
            let proof=inspection.and_then(|s|s.proof(page_id,id,dictionary));
            // A missing reverse key in a malformed tagged graph must not turn
            // its annotation into an ordinary untagged import. Valid unowned
            // annotations (including direct dictionaries) keep the old path.
            let semantic_editable=(!structured_source || inspection.is_some()) && (!sensitive || proof.is_some());
            metadata.push(AnnotationMetadata {
                semantic_editable,
                highlight_appearance: if semantic_editable && subtype == b"Highlight" { highlight_appearance::inspect(&document, dictionary, &mut appearance_budget) } else { highlight_appearance::Evidence::Unsupported },
                page_unit: physical_units::page_unit(&document, page_id).unwrap_or(1.0),
                owned: if semantic_editable {physical_units::page_unit(&document,page_id).ok().and_then(|unit|persistence::decode_bound(&document, dictionary, &mut fonts, &mut images,unit,proof.as_ref()))} else {None},
                id,
                subtype,
                color,
                opacity,
                parent: dictionary
                    .get(b"Parent")
                    .ok()
                    .and_then(|value| value.as_reference().ok()),
                popup: dictionary
                    .get(b"Popup")
                    .ok()
                    .and_then(|value| value.as_reference().ok()),
            });
        }
        pages.insert((number - 1) as usize, metadata);
    }
    Ok(SourceAnnotationMetadata { pages, appearance_budget })
}

fn editable_annotation(annotation: &PdfPageAnnotation<'_>) -> bool {
    matches!(
        annotation.annotation_type(),
        PdfPageAnnotationType::Highlight
            | PdfPageAnnotationType::Text
            | PdfPageAnnotationType::FreeText
            | PdfPageAnnotationType::Ink
            | PdfPageAnnotationType::Stamp
    ) && !annotation.is_hidden()
        && !annotation.is_printable_but_not_viewable()
        && annotation.is_printed()
        && annotation.is_zoomable()
        && annotation.is_rotatable()
        && !annotation.is_read_only()
        && !annotation.is_locked()
        && annotation.is_editable()
}

fn import_annotations(
    pdfium: &'static Pdfium,
    page: &mut PdfPage<'static>,
    geometry: PageGeometry,
    page_index: usize,
    bytes: &[u8],
    metadata: &mut Option<Result<SourceAnnotationMetadata, ()>>,
    imported_fonts: &mut Vec<Arc<FontAsset>>,
    imported_images: &mut Vec<Arc<crate::ImageAsset>>,
    imported_indices: &mut HashMap<usize,HashSet<usize>>,
    imported_sources:&mut imported_overlay_sources::Index,
    legacy_comment_import: bool,
) -> EngineResult<Vec<Overlay>> {
    let mut imported = Vec::new();
    let annotations = page.annotations_mut();
    if !annotations
        .iter()
        .any(|annotation| editable_annotation(&annotation) && (legacy_comment_import || annotation.annotation_type() != PdfPageAnnotationType::Text))
    {
        return Ok(imported);
    }
    // Parse source dictionaries only when editable annotation candidates exist.
    // If metadata cannot be read safely, leave originals native and visually intact.
    let Ok(metadata) =
        metadata.get_or_insert_with(|| parse_annotation_metadata(bytes).map_err(|_| ()))
    else {
        return Ok(imported);
    };
    let appearance_budget = &mut metadata.appearance_budget;
    let Some(metadata) = metadata.pages
        .get(&page_index)
        .filter(|metadata| metadata.len() == annotations.len() as usize)
    else {
        return Ok(imported);
    };
    let mut deleted = HashSet::new();
    let mut imported_ids = HashSet::new();
    for index in 0..annotations.len() {
        let annotation = annotations.get(index)?;
        if !editable_annotation(&annotation) || (!legacy_comment_import && annotation.annotation_type() == PdfPageAnnotationType::Text) {
            continue;
        }
        let metadata_item = &metadata[index as usize];
        if !metadata_item.semantic_editable {continue;}
        let Some(color) = &metadata_item.color else {
            continue;
        };
        let mut overlay = match annotation.annotation_type() {
            PdfPageAnnotationType::FreeText | PdfPageAnnotationType::Ink | PdfPageAnnotationType::Stamp => {
                let Some(overlay) = metadata_item
                    .owned
                    .as_ref()
                    .and_then(|record| record.displayed(geometry))
                else {
                    continue;
                };
                overlay
            }
            PdfPageAnnotationType::Highlight => {
                if metadata_item.subtype != b"Highlight" || metadata_item.opacity.is_none() {
                    continue;
                }
                let points = annotation.attachment_points();
                if points.is_empty() || points.len() > 10_000 {
                    continue;
                }
                let mut rects = Vec::new();
                let mut representable = true;
                for index in 0..points.len() {
                    let points = points.get(index)?;
                    let corners = [
                        geometry.pdf_to_displayed(points.x1().value, points.y1().value),
                        geometry.pdf_to_displayed(points.x2().value, points.y2().value),
                        geometry.pdf_to_displayed(points.x3().value, points.y3().value),
                        geometry.pdf_to_displayed(points.x4().value, points.y4().value),
                    ];
                    let left = corners.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
                    let top = corners.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
                    let right = corners
                        .iter()
                        .map(|p| p.x)
                        .fold(f32::NEG_INFINITY, f32::max);
                    let bottom = corners
                        .iter()
                        .map(|p| p.y)
                        .fold(f32::NEG_INFINITY, f32::max);
                    // A skewed quadrilateral cannot be represented by editor rectangles.
                    if corners.iter().any(|p| {
                        ((p.x - left).abs() > 0.25 && (p.x - right).abs() > 0.25)
                            || ((p.y - top).abs() > 0.25 && (p.y - bottom).abs() > 0.25)
                    }) {
                        representable = false;
                        break;
                    }
                    rects.push(AnnotationRect {
                        x: left,
                        y: top,
                        width: right - left,
                        height: bottom - top,
                    });
                }
                let highlight = HighlightOverlay {
                    id: Uuid::new_v4().to_string(),
                    rects,
                    color: color.clone(),
                    text: annotation.contents(),
                    opacity: metadata_item.opacity,
                };
                if !representable || validate_highlight(&highlight).is_err() {
                    continue;
                }
                if !highlight_appearance::admits(pdfium, &metadata_item.highlight_appearance, geometry, metadata_item.page_unit, &highlight, appearance_budget) { continue; }
                Overlay::Highlight(highlight)
            }
            PdfPageAnnotationType::Text => {
                if metadata_item.subtype != b"Text" || metadata_item.opacity != Some(1.0) {
                    continue;
                }
                let bounds = annotation.bounds()?;
                let a = geometry.pdf_to_displayed(bounds.left().value, bounds.bottom().value);
                let b = geometry.pdf_to_displayed(bounds.right().value, bounds.top().value);
                let comment = CommentOverlay {
                    id: Uuid::new_v4().to_string(),
                    x: a.x.min(b.x),
                    y: a.y.min(b.y),
                    text: annotation.contents().unwrap_or_default(),
                    color: color.clone(),
                };
                if validate_comment(&comment).is_err() {
                    continue;
                }
                Overlay::Comment(comment)
            }
            _ => continue,
        };
        let id = match &mut overlay {
            Overlay::Image(overlay) => &mut overlay.id,
            Overlay::Text(overlay) => &mut overlay.id,
            Overlay::Ink(overlay) => &mut overlay.id,
            Overlay::Highlight(overlay) => &mut overlay.id,
            Overlay::Comment(overlay) => &mut overlay.id,
        };
        // Other readers can duplicate an annotation including its private ID.
        if !imported_ids.insert(id.clone()) {
            *id = Uuid::new_v4().to_string();
            imported_ids.insert(id.clone());
        }
        // Inspect all source parents before stripping a popup. A later parent
        // may stay native, so sequential duplicate detection alone is too late.
        if !imported_sources.reserve_scan(metadata.len()) { continue; }
        let claims_popup = |parent: &AnnotationMetadata, popup: &AnnotationMetadata| {
            parent.subtype != b"Popup" && popup.subtype == b"Popup"
                && ((parent.id.is_some() && popup.parent == parent.id)
                    || (parent.popup.is_some() && popup.id == parent.popup))
        };
        let popup_indices=metadata.iter().enumerate().filter_map(|(popup_index,popup)|{
            claims_popup(metadata_item, popup).then_some(popup_index)
        }).take(imported_sources.popup_capacity().saturating_add(1)).collect::<Vec<_>>();
        if popup_indices.iter().any(|i|!metadata[*i].semantic_editable){continue;}
        let Some(scan_steps) = metadata.len().checked_mul(popup_indices.len()) else { continue; };
        if !imported_sources.reserve_scan(scan_steps) { continue; }
        if popup_indices.iter().any(|popup_index| metadata.iter().enumerate().any(|(parent_index, parent)| {
            parent_index != index as usize && claims_popup(parent, &metadata[*popup_index])
        })) { continue; }
        // Record the final workspace ID after duplicate repair, while original
        // source slots and canonical annotation references are still available.
        // A capacity refusal leaves the original annotation native and visible.
        if !imported_sources.capture_in_units(page_index,index,metadata_item.id,&popup_indices,&overlay,metadata_item.page_unit){continue;}
        deleted.insert(index);
        if let Some(font) = metadata_item
            .owned
            .as_ref()
            .and_then(|record| record.custom_font.as_ref())
        {
            if !imported_fonts
                .iter()
                .any(|existing| existing.info.id == font.info.id)
            {
                imported_fonts.push(font.clone());
            }
        }
        if let Some(image) = metadata_item.owned.as_ref().and_then(|record| record.image.as_ref()) {
            if !imported_images.iter().any(|existing| existing.info().id == image.asset.info().id) { imported_images.push(image.asset.clone()); }
        }
        for popup_index in popup_indices {deleted.insert(popup_index);}
        imported.push(overlay);
    }
    // Remove parents and associated popups together, from the highest index down.
    imported_indices.insert(page_index,deleted.clone());
    let mut deleted: Vec<_> = deleted.into_iter().collect();
    deleted.sort_unstable();
    for index in deleted.into_iter().rev() {
        let annotation = annotations.get(index)?;
        annotations.delete_annotation(annotation)?;
    }
    Ok(imported)
}

fn annotation_bounds(geometry: PageGeometry, rect: &AnnotationRect) -> PdfRect {
    let a = geometry.displayed_to_pdf(Point {
        x: rect.x,
        y: rect.y,
    });
    let b = geometry.displayed_to_pdf(Point {
        x: rect.x + rect.width,
        y: rect.y + rect.height,
    });
    PdfRect::new(
        PdfPoints::new(a.1.value.min(b.1.value)),
        PdfPoints::new(a.0.value.min(b.0.value)),
        PdfPoints::new(a.1.value.max(b.1.value)),
        PdfPoints::new(a.0.value.max(b.0.value)),
    )
}

fn add_highlight_annotation(
    page: &mut PdfPage<'static>,
    geometry: PageGeometry,
    highlight: &HighlightOverlay,
) -> EngineResult<()> {
    let mut annotation = page.annotations_mut().create_highlight_annotation()?;
    let color = parse_color(&highlight.color)?;
    let alpha = (highlight.opacity.unwrap_or(96.0 / 255.0) * 255.0).round() as u8;
    annotation.set_stroke_color(PdfColor::new(
        color.red(),
        color.green(),
        color.blue(),
        alpha,
    ))?;
    annotation.set_is_printed(true)?;
    annotation.set_creator("Folio")?;
    if let Some(text) = &highlight.text {
        annotation.set_contents(text)?;
    }
    let left = highlight
        .rects
        .iter()
        .map(|r| r.x)
        .fold(f32::INFINITY, f32::min);
    let top = highlight
        .rects
        .iter()
        .map(|r| r.y)
        .fold(f32::INFINITY, f32::min);
    let right = highlight
        .rects
        .iter()
        .map(|r| r.x + r.width)
        .fold(f32::NEG_INFINITY, f32::max);
    let bottom = highlight
        .rects
        .iter()
        .map(|r| r.y + r.height)
        .fold(f32::NEG_INFINITY, f32::max);
    annotation.set_bounds(annotation_bounds(
        geometry,
        &AnnotationRect {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        },
    ))?;
    for rect in &highlight.rects {
        let tl = geometry.displayed_to_pdf(Point {
            x: rect.x,
            y: rect.y,
        });
        let tr = geometry.displayed_to_pdf(Point {
            x: rect.x + rect.width,
            y: rect.y,
        });
        let bl = geometry.displayed_to_pdf(Point {
            x: rect.x,
            y: rect.y + rect.height,
        });
        let br = geometry.displayed_to_pdf(Point {
            x: rect.x + rect.width,
            y: rect.y + rect.height,
        });
        annotation
            .attachment_points_mut()
            .create_attachment_point_at_end(PdfQuadPoints::new(
                tl.0, tl.1, tr.0, tr.1, bl.0, bl.1, br.0, br.1,
            ))?;
    }
    Ok(())
}

fn add_comment_annotation(
    page: &mut PdfPage<'static>,
    geometry: PageGeometry,
    comment: &CommentOverlay,
    unit: f32,
) -> EngineResult<()> {
    let mut icon_size=24.;icon_size.scale(unit.recip())?;
    let mut annotation = page
        .annotations_mut()
        .create_text_annotation(&comment.text)?;
    annotation.set_bounds(annotation_bounds(
        geometry,
        &AnnotationRect {
            x: comment.x,
            y: comment.y,
            width: icon_size,
            height: icon_size,
        },
    ))?;
    annotation.set_stroke_color(parse_color(&comment.color)?)?;
    annotation.set_is_printed(true)?;
    annotation.set_is_zoomable(true)?;
    annotation.set_is_rotatable(true)?;
    annotation.set_creator("Folio")?;
    Ok(())
}

pub(crate) fn validate_highlight(highlight: &HighlightOverlay) -> EngineResult<()> {
    validate_color(&highlight.color)?;
    if highlight
        .opacity
        .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
    {
        return Err(EngineError::InvalidRequest(
            "highlight opacity must be between 0 and 1".into(),
        ));
    }
    if highlight.rects.is_empty() || highlight.rects.len() > 10_000 {
        return Err(EngineError::InvalidRequest(
            "highlight requires between 1 and 10000 rectangles".into(),
        ));
    }
    for rect in &highlight.rects {
        validate_point(rect.x, rect.y, 0.0, 0.0)?;
        if !finite_positive(rect.width)
            || !finite_positive(rect.height)
            || rect.width > 10_000_000.0
            || rect.height > 10_000_000.0
        {
            return Err(EngineError::InvalidRequest(
                "invalid highlight rectangle".into(),
            ));
        }
        validate_point(rect.x + rect.width, rect.y + rect.height, 0.0, 0.0)?;
    }
    if let Some(text) = &highlight.text {
        validate_annotation_text(text)?;
    }
    Ok(())
}

pub(crate) fn validate_comment(comment: &CommentOverlay) -> EngineResult<()> {
    validate_color(&comment.color)?;
    validate_point(comment.x, comment.y, 0.0, 0.0)?;
    validate_annotation_text(&comment.text)
}

fn validate_annotation_text(text: &str) -> EngineResult<()> {
    if text.len() > 1_000_000 || text.contains('\0') {
        return Err(EngineError::InvalidRequest(
            "annotation text exceeds 1000000 bytes or contains a null character".into(),
        ));
    }
    Ok(())
}

fn add_ink_overlay(
    page: &mut PdfPage<'static>,
    geometry: PageGeometry,
    overlay: &InkOverlay,
) -> EngineResult<()> {
    let color = parse_color(&overlay.color)?;
    for path in &overlay.paths {
        let (x1, y1) = geometry.displayed_to_pdf(path[0]);
        let (x2, y2) = geometry.displayed_to_pdf(path[1]);
        let mut object = page.objects_mut().create_path_object_line(
            x1,
            y1,
            x2,
            y2,
            color,
            PdfPoints::new(overlay.stroke_width),
        )?;
        object.set_line_cap(PdfPageObjectLineCap::Round)?;
        object.set_line_join(PdfPageObjectLineJoin::Round)?;
        let path_object = object
            .as_path_object_mut()
            .ok_or_else(|| EngineError::Pdfium("created ink object was not a path".into()))?;
        for point in path.iter().skip(2) {
            let (x, y) = geometry.displayed_to_pdf(*point);
            path_object.line_to(x, y)?;
        }
    }
    Ok(())
}

fn validate_raw_text_geometry(text:&TextOverlay,width:f32,height:f32)->EngineResult<()>{
    // Independent raw geometry bound, separate from the physical UI limits.
    validate_point(text.x,text.y,width,height)?;
    if !text.font_size.is_finite()||text.font_size<=0.||text.font_size>10_000_000.
        ||text.area.as_ref().is_some_and(|area|[area.width,area.height,area.inset].iter().any(|v|!v.is_finite()||v.abs()>10_000_000.)){
        return Err(EngineError::InvalidRequest("Text geometry exceeds the page-coordinate limit.".into()));
    }
    Ok(())
}
fn validate_text_in_units(text:&TextOverlay,width:f32,height:f32,custom:Option<&FontAsset>,unit:f32)->EngineResult<()>{
    if unit==1. {return validate_text(text,width,height,custom);}
    validate_raw_text_geometry(text,width,height)?;
    let physical=physical_units::text_in_points(text,unit)?;
    validate_text(&physical,width*unit,height*unit,custom)
}

fn validate_text(
    overlay: &TextOverlay,
    width: f32,
    height: f32,
    custom: Option<&FontAsset>,
) -> EngineResult<()> {
    overlay.validate_area_settings()?;
    if overlay.shaping.is_some() && overlay.font_id.is_none() {
        return Err(EngineError::InvalidRequest(
            "Shaped text requires a custom font.".into(),
        ));
    }
    if !matches!(overlay.rotation, 0 | 90 | 180 | 270) {
        return Err(EngineError::InvalidRequest("invalid text rotation".into()));
    }
    validate_color(&overlay.color)?;
    validate_point(overlay.x, overlay.y, width, height)?;
    if overlay.text.len() > 1_000_000 {
        return Err(EngineError::InvalidRequest(
            "text must contain no more than 1000000 bytes".into(),
        ));
    }
    if overlay
        .text
        .chars()
        .any(|character| character == char::from(0))
    {
        return Err(EngineError::InvalidRequest(
            "text contains a null character".into(),
        ));
    }
    if let Some(id) = &overlay.font_id {
        let font = custom.filter(|font| &font.info.id == id).ok_or_else(|| {
            EngineError::InvalidRequest(format!("custom font {id} is unavailable"))
        })?;
        if overlay.shaping.is_some() {
            #[cfg(feature = "shaped-text")]
            crate::prepare_text_overlay(font, overlay)?;
            #[cfg(not(feature = "shaped-text"))]
            return Err(EngineError::InvalidRequest(
                "Shaped text is unavailable in this build.".into(),
            ));
        } else {
            font.validate_text(&overlay.text)?;
        }
    } else if let Some(character) = overlay
        .text
        .chars()
        .find(|character| !helvetica_supports(*character))
    {
        return Err(EngineError::InvalidRequest(format!(
            "{} does not support character U+{:04X}; use Latin text and common punctuation",
            overlay.font_name.pdf_name(),
            character as u32
        )));
    }
    if !overlay.font_size.is_finite() || overlay.font_size <= 0.0 || overlay.font_size > 512.0 {
        return Err(EngineError::InvalidRequest("invalid fontSize".into()));
    }
    let _ = (width, height);
    Ok(())
}

fn validate_ink(overlay: &InkOverlay, width: f32, height: f32,unit:f32) -> EngineResult<()> {
    validate_color(&overlay.color)?;
    if !overlay.stroke_width.is_finite()
        || overlay.stroke_width <= 0.0
        || overlay.stroke_width > 100.0*unit.recip()
        || overlay.stroke_width > 10_000_000.
    {
        return Err(EngineError::InvalidRequest("invalid strokeWidth".into()));
    }
    if overlay.paths.is_empty() || overlay.paths.len() > 100_000 {
        return Err(EngineError::InvalidRequest("invalid ink paths".into()));
    }
    for path in &overlay.paths {
        if path.len() < 2 || path.len() > 1_000_000 {
            return Err(EngineError::InvalidRequest(
                "each ink path requires between 2 and 1000000 points".into(),
            ));
        }
        for point in path {
            validate_point(point.x, point.y, width, height)?;
        }
    }
    Ok(())
}

fn validate_point(x: f32, y: f32, width: f32, height: f32) -> EngineResult<()> {
    let _ = (width, height);
    if !x.is_finite() || !y.is_finite() || x.abs() > 10_000_000.0 || y.abs() > 10_000_000.0 {
        return Err(EngineError::InvalidRequest(
            "overlay point lies outside the page".into(),
        ));
    }
    Ok(())
}

fn validate_id(label: &str, value: &str) -> EngineResult<()> {
    if value.trim().is_empty() || value.len() > 1024 {
        Err(EngineError::InvalidRequest(format!("invalid {label}")))
    } else {
        Ok(())
    }
}

fn validate_color(value: &str) -> EngineResult<()> {
    parse_color(value).map(|_| ())
}

fn parse_color(value: &str) -> EngineResult<PdfColor> {
    let bytes = value.as_bytes();
    if bytes.len() != 7 || bytes[0] != b'#' || !bytes[1..].iter().all(u8::is_ascii_hexdigit) {
        return Err(EngineError::InvalidRequest(format!(
            "invalid color {value}; expected #RRGGBB"
        )));
    }
    let component = |start| {
        u8::from_str_radix(&value[start..start + 2], 16)
            .map_err(|_| EngineError::InvalidRequest(format!("invalid color {value}")))
    };
    Ok(PdfColor::new(
        component(1)?,
        component(3)?,
        component(5)?,
        255,
    ))
}

fn helvetica_supports(character: char) -> bool {
    matches!(
        character,
        '\n' | '\r'
            | '\u{0020}'..='\u{007E}'
            | '\u{00A1}'..='\u{00AC}'
            | '\u{00AE}'..='\u{00FF}'
            | '\u{20AC}'
            | '\u{201A}'
            | '\u{0192}'
            | '\u{201E}'
            | '\u{2026}'
            | '\u{2020}'
            | '\u{2021}'
            | '\u{02C6}'
            | '\u{2030}'
            | '\u{0160}'
            | '\u{2039}'
            | '\u{0152}'
            | '\u{017D}'
            | '\u{2018}'
            | '\u{2019}'
            | '\u{201C}'
            | '\u{201D}'
            | '\u{2022}'
            | '\u{2013}'
            | '\u{2014}'
            | '\u{02DC}'
            | '\u{2122}'
            | '\u{0161}'
            | '\u{203A}'
            | '\u{0153}'
            | '\u{017E}'
            | '\u{0178}'
    )
}

fn finite_positive(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

fn page_index_i32(index: usize) -> EngineResult<i32> {
    i32::try_from(index).map_err(|_| EngineError::InvalidRequest("pageIndex is too large".into()))
}

fn rotation_from_degrees(value: u16) -> EngineResult<PdfPageRenderRotation> {
    match value {
        0 => Ok(PdfPageRenderRotation::None),
        90 => Ok(PdfPageRenderRotation::Degrees90),
        180 => Ok(PdfPageRenderRotation::Degrees180),
        270 => Ok(PdfPageRenderRotation::Degrees270),
        _ => Err(EngineError::InvalidRequest(format!(
            "invalid rotation {value}"
        ))),
    }
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().to_ascii_lowercase()
}
