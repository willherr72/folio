use crate::types::*;
use image::ImageFormat;
use pdfium_render::prelude::*;
use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, OnceLock};
use uuid::Uuid;

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
}

static SHARED_ENGINE: OnceLock<Result<PdfEngine, String>> = OnceLock::new();

impl PdfEngine {
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
            reply,
        })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }

    /// Read only the private snapshot; retain the original path as an export guard.
    pub(crate) fn open_recovery_document(
        &self,
        snapshot: impl AsRef<Path>,
        original_path: impl AsRef<Path>,
    ) -> EngineResult<DocumentInfo> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::Open {
            path: snapshot.as_ref().to_path_buf(),
            original_path: Some(original_path.as_ref().to_path_buf()),
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

    std::thread::Builder::new()
        .name("folio-pdfium-worker".into())
        .spawn(move || match WorkerRuntime::new(&engine_path) {
            Ok(mut runtime) => {
                let _ = init_sender.send(Ok(runtime.status.clone()));
                runtime.run(receiver);
            }
            Err(error) => {
                let _ = init_sender.send(Err(error.to_string()));
            }
        })
        .map_err(|error| error.to_string())?;

    init_receiver
        .recv()
        .map_err(|_| "PDFium worker exited during initialization".to_string())??;

    Ok(PdfEngine {
        inner: Arc::new(EngineInner { sender }),
    })
}

enum WorkerRequest {
    Open {
        path: PathBuf,
        original_path: Option<PathBuf>,
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
}

struct WorkerRuntime {
    pdfium: &'static Pdfium,
    documents: HashMap<String, OpenDocument>,
    status: String,
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
            }
        } else {
            PageInfo {
                width: self.raw_width(),
                height: self.raw_height(),
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
    fn new(engine_path: &Path) -> EngineResult<Self> {
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
        })
    }

    fn run(&mut self, receiver: mpsc::Receiver<WorkerRequest>) {
        while let Ok(request) = receiver.recv() {
            match request {
                WorkerRequest::Open {
                    path,
                    original_path,
                    reply,
                } => {
                    let _ = reply.send(self.open_document(path, original_path));
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
        for index in 0..document.pages().len() {
            let page = document.pages().get(index)?;
            pages.push(page_geometry(&page)?.displayed_size());
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
        self.documents.insert(
            id,
            OpenDocument {
                path: canonical,
                original_path,
                document,
                source_bytes,
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
        let font = output.fonts_mut().helvetica();

        for plan in &request.pages {
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
            for overlay in &plan.overlays {
                match overlay {
                    Overlay::Text(text) => {
                        add_text_overlay(&mut page, geometry, text, font)?;
                    }
                    Overlay::Ink(ink) => {
                        add_ink_overlay(&mut page, geometry, ink)?;
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
                };
                validate_id("overlay id", id)?;
                if !overlay_ids.insert(id.as_str()) {
                    return Err(EngineError::InvalidRequest(format!(
                        "duplicate overlay id {id}"
                    )));
                }
                match overlay {
                    Overlay::Text(text) => validate_text(text, size.width, size.height)?,
                    Overlay::Ink(ink) => validate_ink(ink, size.width, size.height)?,
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

fn add_text_overlay(
    page: &mut PdfPage<'static>,
    geometry: PageGeometry,
    overlay: &TextOverlay,
    font: PdfFontToken,
) -> EngineResult<()> {
    let color = parse_color(&overlay.color)?;
    for (line_index, line) in overlay.text.lines().enumerate() {
        let baseline = Point {
            x: overlay.x,
            y: overlay.y + overlay.font_size + line_index as f32 * overlay.font_size * 1.2,
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
        if geometry.rotation != 0 {
            object.rotate_counter_clockwise_degrees(geometry.rotation as f32)?;
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

fn validate_text(overlay: &TextOverlay, width: f32, height: f32) -> EngineResult<()> {
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
    if let Some(character) = overlay
        .text
        .chars()
        .find(|character| !helvetica_supports(*character))
    {
        return Err(EngineError::InvalidRequest(format!(
            "Helvetica does not support character U+{:04X}; use Latin text and common punctuation",
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
