use crate::fonts::{FontAsset, FontRegistry};
use crate::types::*;
use image::ImageFormat;
use pdfium_render::prelude::*;
use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, OnceLock};
use uuid::Uuid;

#[path = "persistence.rs"]
mod persistence;
#[path = "text_edit.rs"]
mod text_edit;

pub type EngineResult<T> = Result<T, EngineError>;
pub(crate) const MAX_SOURCE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
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
}

static SHARED_ENGINE: OnceLock<Result<PdfEngine, String>> = OnceLock::new();

impl PdfEngine {
    pub fn fonts(&self) -> Arc<FontRegistry> {
        self.inner.fonts.clone()
    }

    pub fn list_text_runs(&self, source_id: &str, page_index: usize) -> EngineResult<TextRuns> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::ListTextRuns {
            source_id: source_id.into(),
            page_index,
            reply,
        })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }

    pub fn replace_text(
        &self,
        source_id: &str,
        page_index: usize,
        object_index: usize,
        expected_text: &str,
        replacement: &str,
    ) -> EngineResult<DocumentInfo> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::ReplaceText {
            source_id: source_id.into(),
            page_index,
            object_index,
            expected_text: expected_text.into(),
            replacement: replacement.into(),
            reply,
        })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }

    pub fn start(engine_path: impl AsRef<Path>) -> EngineResult<Self> {
        let path = engine_path.as_ref().to_path_buf();
        match SHARED_ENGINE.get_or_init(|| start_worker(path)) {
            Ok(engine) => Ok(engine.clone()),
            Err(error) => Err(EngineError::Initialization(error.clone())),
        }
    }

    pub fn open_document(&self, path: impl AsRef<Path>) -> EngineResult<DocumentInfo> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::Open {
            path: path.as_ref().to_path_buf(),
            original_path: None,
            editable_annotations: true,
            register_fonts: true,
            reply,
        })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }

    /// Read only the private snapshot; retain the original path as an export guard.
    /// Current recovery plans already supply their fonts. They strip verified
    /// source annotations without retaining fonts that the user later removed.
    /// Legacy annotation migration passes `register_fonts = true`.
    pub(crate) fn open_recovery_document(
        &self,
        snapshot: impl AsRef<Path>,
        original_path: impl AsRef<Path>,
        register_fonts: bool,
    ) -> EngineResult<DocumentInfo> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::Open {
            path: snapshot.as_ref().to_path_buf(),
            original_path: Some(original_path.as_ref().to_path_buf()),
            editable_annotations: true,
            register_fonts,
            reply,
        })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }

    /// Print/export previews retain standard annotation appearances in the PDF raster.
    pub(crate) fn open_document_for_printing(
        &self,
        path: impl AsRef<Path>,
    ) -> EngineResult<DocumentInfo> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::Open {
            path: path.as_ref().to_path_buf(),
            original_path: None,
            editable_annotations: false,
            register_fonts: false,
            reply,
        })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
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
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::Export {
            request,
            destination: destination.as_ref().to_path_buf(),
            reply,
        })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
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
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
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

    std::thread::Builder::new()
        .name("folio-pdfium-worker".into())
        .spawn(
            move || match WorkerRuntime::new(&engine_path, worker_fonts) {
                Ok(mut runtime) => {
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
        inner: Arc::new(EngineInner { sender, fonts }),
    })
}

enum WorkerRequest {
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
        reply: mpsc::Sender<EngineResult<DocumentInfo>>,
    },
    Open {
        path: PathBuf,
        original_path: Option<PathBuf>,
        editable_annotations: bool,
        register_fonts: bool,
        reply: mpsc::Sender<EngineResult<DocumentInfo>>,
    },
    Render {
        source_id: String,
        page_index: usize,
        width: u32,
        reply: mpsc::Sender<EngineResult<Vec<u8>>>,
    },
    Export {
        request: ExportRequest,
        destination: PathBuf,
        reply: mpsc::Sender<EngineResult<()>>,
    },
    Close {
        source_id: String,
        reply: mpsc::Sender<EngineResult<()>>,
    },
    SourceBytes {
        source_id: String,
        reply: mpsc::Sender<EngineResult<Arc<[u8]>>>,
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

struct OpenDocument {
    path: PathBuf,
    original_path: PathBuf,
    document: PdfDocument<'static>,
    source_bytes: Arc<[u8]>,
    for_printing: bool,
    text_edit_restriction: OnceLock<Option<String>>,
}

struct WorkerRuntime {
    pdfium: &'static Pdfium,
    documents: HashMap<String, OpenDocument>,
    status: String,
    fonts: Arc<FontRegistry>,
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
        let bindings = Pdfium::bind_to_library(&canonical)
            .map_err(|error| EngineError::Initialization(error.to_string()))?;
        let pdfium = Box::leak(Box::new(Pdfium::new(bindings)));
        let status = "Native PDF engine ready".to_string();
        Ok(Self {
            pdfium,
            documents: HashMap::new(),
            status,
            fonts,
        })
    }

    fn run(&mut self, receiver: mpsc::Receiver<WorkerRequest>) {
        while let Ok(request) = receiver.recv() {
            match request {
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
                    reply,
                } => {
                    let _ = reply.send(self.replace_text(
                        &source_id,
                        page_index,
                        object_index,
                        &expected_text,
                        &replacement,
                    ));
                }
                WorkerRequest::Open {
                    path,
                    original_path,
                    editable_annotations,
                    register_fonts,
                    reply,
                } => {
                    let _ = reply.send(self.open_document(
                        path,
                        original_path,
                        editable_annotations,
                        register_fonts,
                    ));
                }
                WorkerRequest::Render {
                    source_id,
                    page_index,
                    width,
                    reply,
                } => {
                    let _ = reply.send(self.render_page(&source_id, page_index, width));
                }
                WorkerRequest::Export {
                    request,
                    destination,
                    reply,
                } => {
                    let _ = reply.send(self.export_pdf(request, destination));
                }
                WorkerRequest::Close { source_id, reply } => {
                    let _ = reply.send(self.close_document(&source_id));
                }
                WorkerRequest::SourceBytes { source_id, reply } => {
                    let _ = reply.send(
                        self.document(&source_id)
                            .map(|source| source.source_bytes.clone()),
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
        }
    }

    fn open_document(
        &mut self,
        path: PathBuf,
        original_path: Option<PathBuf>,
        editable_annotations: bool,
        register_fonts: bool,
    ) -> EngineResult<DocumentInfo> {
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
        let source_bytes: Arc<[u8]> = bytes.into();
        let document = self
            .pdfium
            .load_pdf_from_reader(Cursor::new(source_bytes.clone()), None)?;
        let mut pages = Vec::with_capacity(document.pages().len() as usize);
        let mut annotation_metadata = None;
        let mut imported_fonts = Vec::new();
        for index in 0..document.pages().len() {
            let mut page = document.pages().get(index)?;
            let geometry = page_geometry(&page)?;
            let mut info = geometry.displayed_size();
            if editable_annotations {
                info.overlays = import_annotations(
                    &mut page,
                    geometry,
                    index as usize,
                    &source_bytes,
                    &mut annotation_metadata,
                    &mut imported_fonts,
                )?;
            }
            pages.push(info);
        }
        let id = Uuid::new_v4().to_string();
        let name = canonical
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("document.pdf")
            .to_string();
        let info = DocumentInfo {
            id: id.clone(),
            name,
            pages,
        };
        // No global font registration occurs until every page has opened and
        // imported successfully. The batch is atomic even when capacity is exceeded.
        if register_fonts {
            self.fonts.register_assets(&imported_fonts)?;
        }
        self.documents.insert(
            id,
            OpenDocument {
                path: canonical,
                original_path,
                document,
                source_bytes,
                for_printing: !editable_annotations,
                text_edit_restriction: OnceLock::new(),
            },
        );
        Ok(info)
    }

    fn render_page(&self, source_id: &str, page_index: usize, width: u32) -> EngineResult<Vec<u8>> {
        let document = self.document(source_id)?;
        let page = document.document.pages().get(page_index_i32(page_index)?)?;
        let target_width = width.clamp(64, 2400) as i32;
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
        let document = self.document(source_id)?;
        let page = document.document.pages().get(page_index_i32(page_index)?)?;
        let bounds = page.boundaries().bounding()?.bounds;
        let text = page.text()?.inside_rect(bounds);
        Ok(text)
    }

    fn page_text(&self, source_id: &str, page_index: usize) -> EngineResult<PageText> {
        let document = self.document(source_id)?;
        let page = document.document.pages().get(page_index_i32(page_index)?)?;
        let geometry = page_geometry(&page)?;
        let text = page.text()?;
        let chars = text.chars();
        let mut characters = Vec::with_capacity(chars.len() as usize);
        let mut last_position = Point { x: 0.0, y: 0.0 };
        for character in chars.iter() {
            let value = character
                .unicode_char()
                .unwrap_or(char::REPLACEMENT_CHARACTER);
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
            characters.push(PdfTextCharacter {
                text: value.to_string(),
                x: position.x,
                y: position.y,
                width,
                height,
            });
            last_position = position;
        }
        Ok(PageText {
            intrinsic_rotation: geometry.rotation,
            characters,
        })
    }

    fn close_document(&mut self, source_id: &str) -> EngineResult<()> {
        if self.documents.remove(source_id).is_some() {
            Ok(())
        } else {
            Err(EngineError::InvalidRequest(format!(
                "unknown sourceId {source_id}"
            )))
        }
    }

    fn export_pdf(&mut self, request: ExportRequest, destination: PathBuf) -> EngineResult<()> {
        let destination = self.validated_destination(&destination)?;
        self.validate_export(&request)?;

        let mut output = self.pdfium.create_new_pdf()?;
        let mut fonts = HashMap::new();
        if request.flatten {
            for text in request
                .pages
                .iter()
                .filter(|page| {
                    !page.overlays.iter().any(
                        |overlay| matches!(overlay, Overlay::Text(text) if text.font_id.is_some()),
                    )
                })
                .flat_map(|page| &page.overlays)
                .filter_map(|overlay| {
                    if let Overlay::Text(text) = overlay {
                        if text.font_id.is_some() {
                            return None;
                        }
                        Some(text)
                    } else {
                        None
                    }
                })
            {
                fonts.entry(text.font_name).or_insert_with(|| {
                    let fonts = output.fonts_mut();
                    match text.font_name {
                        TextFont::Helvetica => fonts.helvetica(),
                        TextFont::HelveticaBold => fonts.helvetica_bold(),
                        TextFont::HelveticaOblique => fonts.helvetica_oblique(),
                        TextFont::HelveticaBoldOblique => fonts.helvetica_bold_oblique(),
                        TextFont::TimesRoman => fonts.times_roman(),
                        TextFont::TimesBold => fonts.times_bold(),
                        TextFont::TimesItalic => fonts.times_italic(),
                        TextFont::TimesBoldItalic => fonts.times_bold_italic(),
                        TextFont::Courier => fonts.courier(),
                        TextFont::CourierBold => fonts.courier_bold(),
                        TextFont::CourierOblique => fonts.courier_oblique(),
                        TextFont::CourierBoldOblique => fonts.courier_bold_oblique(),
                    }
                });
            }
        }
        let mut geometries = Vec::with_capacity(request.pages.len());

        for plan in &request.pages {
            // A page containing custom text paints every text/ink overlay through
            // the portable appearance path, retaining their relative order.
            let portable_flatten = request.flatten
                && plan.overlays.iter().any(
                    |overlay| matches!(overlay, Overlay::Text(text) if text.font_id.is_some()),
                );
            let destination_index = output.pages().len();
            {
                let source = self.document(&plan.source_id)?;
                output.pages_mut().copy_page_from_document(
                    &source.document,
                    page_index_i32(plan.page_index)?,
                    destination_index,
                )?;
            }

            let mut page = output.pages().get(destination_index)?;
            let geometry = page_geometry(&page)?;
            geometries.push(geometry);
            for overlay in &plan.overlays {
                match overlay {
                    Overlay::Text(text) => {
                        if request.flatten && !portable_flatten && text.font_id.is_none() {
                            add_text_overlay(&mut page, geometry, text, fonts[&text.font_name])?;
                        }
                    }
                    Overlay::Ink(ink) => {
                        if request.flatten && !portable_flatten {
                            add_ink_overlay(&mut page, geometry, ink)?;
                        }
                    }
                    Overlay::Highlight(highlight) => {
                        add_highlight_annotation(&mut page, geometry, highlight)?
                    }
                    Overlay::Comment(comment) => {
                        add_comment_annotation(&mut page, geometry, comment)?
                    }
                }
            }
            page.set_rotation(rotation_from_degrees(
                (geometry.rotation + plan.rotation) % 360,
            )?);
            page.regenerate_content()?;
        }

        let parent = destination
            .parent()
            .ok_or_else(|| EngineError::InvalidRequest("destination has no parent".into()))?;
        let temporary = tempfile::Builder::new()
            .prefix(".folio-export-")
            .suffix(".pdf")
            .tempfile_in(parent)
            .map_err(|error| EngineError::Io(format!("creating export temp file: {error}")))?;
        let temporary = temporary.into_temp_path();
        output.save_to_file(&temporary)?;
        if request.pages.iter().any(|page| {
            page.overlays.iter().any(|overlay| {
                !request.flatten
                    || matches!(overlay, Overlay::Highlight(_) | Overlay::Comment(_))
                    || matches!(overlay, Overlay::Text(text) if text.font_id.is_some())
            })
        }) {
            persistence::finalize(&temporary, &request, &geometries, &self.fonts)?;
        }
        std::fs::OpenOptions::new()
            .write(true)
            .open(&temporary)
            .and_then(|file| file.sync_all())
            .map_err(|error| EngineError::Io(format!("syncing export temp file: {error}")))?;
        temporary.persist(&destination).map_err(|error| {
            EngineError::Io(format!("persisting export temp file: {}", error.error))
        })
    }

    fn document(&self, source_id: &str) -> EngineResult<&OpenDocument> {
        self.documents
            .get(source_id)
            .ok_or_else(|| EngineError::InvalidRequest(format!("unknown sourceId {source_id}")))
    }

    fn validated_destination(&self, destination: &Path) -> EngineResult<PathBuf> {
        let file_name = destination.file_name().ok_or_else(|| {
            EngineError::InvalidRequest("destination must name a PDF file".into())
        })?;
        if destination
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            != Some("pdf".to_string())
        {
            return Err(EngineError::InvalidRequest(
                "destination must use the .pdf extension".into(),
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
                .document
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
            for overlay in &plan.overlays {
                let id = match overlay {
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
                    Overlay::Text(text) => {
                        let custom = text
                            .font_id
                            .as_deref()
                            .map(|id| self.fonts.get(id))
                            .transpose()?;
                        validate_text(text, size.width, size.height, custom.as_deref())?;
                    }
                    Overlay::Ink(ink) => validate_ink(ink, size.width, size.height)?,
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
}

type SourceAnnotationMetadata = HashMap<usize, Vec<AnnotationMetadata>>;

fn parse_annotation_metadata(bytes: &[u8]) -> Result<SourceAnnotationMetadata, lopdf::Error> {
    let document = lopdf::Document::load_mem_with_options(
        bytes,
        lopdf::LoadOptions::with_max_decompressed_size(64 * 1024 * 1024),
    )?;
    let mut pages = HashMap::new();
    let mut fonts = persistence::ImportCache::default();
    for (number, page_id) in document.get_pages() {
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
            metadata.push(AnnotationMetadata {
                owned: persistence::decode(&document, dictionary, &mut fonts),
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
    Ok(pages)
}

fn editable_annotation(annotation: &PdfPageAnnotation<'_>) -> bool {
    matches!(
        annotation.annotation_type(),
        PdfPageAnnotationType::Highlight
            | PdfPageAnnotationType::Text
            | PdfPageAnnotationType::FreeText
            | PdfPageAnnotationType::Ink
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
    page: &mut PdfPage<'static>,
    geometry: PageGeometry,
    page_index: usize,
    bytes: &[u8],
    metadata: &mut Option<Result<SourceAnnotationMetadata, ()>>,
    imported_fonts: &mut Vec<Arc<FontAsset>>,
) -> EngineResult<Vec<Overlay>> {
    let mut imported = Vec::new();
    let annotations = page.annotations_mut();
    if !annotations
        .iter()
        .any(|annotation| editable_annotation(&annotation))
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
    let Some(metadata) = metadata
        .get(&page_index)
        .filter(|metadata| metadata.len() == annotations.len() as usize)
    else {
        return Ok(imported);
    };
    let mut deleted = HashSet::new();
    let mut imported_ids = HashSet::new();
    for index in 0..annotations.len() {
        let annotation = annotations.get(index)?;
        if !editable_annotation(&annotation) {
            continue;
        }
        let metadata_item = &metadata[index as usize];
        let Some(color) = &metadata_item.color else {
            continue;
        };
        let mut overlay = match annotation.annotation_type() {
            PdfPageAnnotationType::FreeText | PdfPageAnnotationType::Ink => {
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
        for (popup_index, popup) in metadata.iter().enumerate() {
            if popup.subtype == b"Popup"
                && ((metadata_item.id.is_some() && popup.parent == metadata_item.id)
                    || (metadata_item.popup.is_some() && popup.id == metadata_item.popup))
            {
                deleted.insert(popup_index);
            }
        }
        imported.push(overlay);
    }
    // Remove parents and associated popups together, from the highest index down.
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
) -> EngineResult<()> {
    let mut annotation = page
        .annotations_mut()
        .create_text_annotation(&comment.text)?;
    annotation.set_bounds(annotation_bounds(
        geometry,
        &AnnotationRect {
            x: comment.x,
            y: comment.y,
            width: 24.0,
            height: 24.0,
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

fn add_text_overlay(
    page: &mut PdfPage<'static>,
    geometry: PageGeometry,
    overlay: &TextOverlay,
    font: PdfFontToken,
) -> EngineResult<()> {
    let color = parse_color(&overlay.color)?;
    for (line_index, line) in overlay.text.lines().enumerate() {
        let relative = persistence::rotated_point(
            Point {
                x: 0.0,
                y: overlay.font_size + line_index as f32 * overlay.font_size * 1.2,
            },
            overlay.rotation,
        );
        let baseline = Point {
            x: overlay.x + relative.x,
            y: overlay.y + relative.y,
        };
        let (x, y) = geometry.displayed_to_pdf(baseline);
        let mut object = page.objects_mut().create_text_object(
            PdfPoints::ZERO,
            PdfPoints::ZERO,
            if line.is_empty() { " " } else { line },
            font,
            PdfPoints::new(overlay.font_size),
        )?;
        object.set_fill_color(color)?;
        let rotation = (geometry.rotation + 360 - overlay.rotation) % 360;
        if rotation != 0 {
            object.rotate_counter_clockwise_degrees(rotation as f32)?;
        }
        object.translate(x, y)?;
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

fn validate_text(
    overlay: &TextOverlay,
    width: f32,
    height: f32,
    custom: Option<&FontAsset>,
) -> EngineResult<()> {
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
        font.validate_text(&overlay.text)?;
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

fn validate_ink(overlay: &InkOverlay, width: f32, height: f32) -> EngineResult<()> {
    validate_color(&overlay.color)?;
    if !overlay.stroke_width.is_finite()
        || overlay.stroke_width <= 0.0
        || overlay.stroke_width > 100.0
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
