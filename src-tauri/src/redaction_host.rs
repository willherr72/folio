//! Trusted desktop ownership for immutable-source text-removal previews.
use crate::engine::{
    TextRedactionBounds, TextRedactionMark, TextRedactionPreview, TextRedactionSource,ContentRedactionPreview,ContentRedactionRaster,
};
use crate::{DocumentInfo, PdfEngine};
use std::collections::{HashMap, HashSet};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

type Result<T> = std::result::Result<T, String>;
#[derive(Clone)]
pub(crate) struct RedactionHost(Arc<Host>);
struct Host {
    engine: PdfEngine,
    windows: Mutex<HashMap<String, Arc<Window>>>,
}
struct Window {
    alive: AtomicBool,
    data: Mutex<WindowData>,
}
#[derive(Default)]
struct WindowData {
    sessions: HashMap<String, String>,
    adopted: HashSet<String>,
}
#[derive(Clone)]
pub(crate) struct WindowHandle {
    engine: PdfEngine,
    window: Arc<Window>,
}
impl RedactionHost {
    pub(crate) fn new(engine: PdfEngine) -> Self {
        Self(Arc::new(Host {
            engine,
            windows: Mutex::new(HashMap::new()),
        }))
    }
    /// Called only for real windows at setup, never from a renderer request.
    pub(crate) fn register_window(&self, label: &str) -> Result<()> {
        let mut windows = self
            .0
            .windows
            .lock()
            .map_err(|_| "Text-removal windows are unavailable.")?;
        if windows.contains_key(label) || windows.len() >= 16 {
            return Err("Text-removal window registration is unavailable.".into());
        }
        windows.insert(
            label.into(),
            Arc::new(Window {
                alive: AtomicBool::new(true),
                data: Mutex::new(WindowData::default()),
            }),
        );
        Ok(())
    }
    pub(crate) fn capture(&self, label: &str) -> Result<WindowHandle> {
        let windows = self
            .0
            .windows
            .lock()
            .map_err(|_| "Text-removal windows are unavailable.")?;
        let window = windows
            .get(label)
            .filter(|w| w.alive.load(Ordering::Acquire))
            .ok_or("This text-removal window has closed.")?
            .clone();
        Ok(WindowHandle {
            engine: self.0.engine.clone(),
            window,
        })
    }
    /// Nonblocking with respect to PDF work: invalidate before scheduling cleanup.
    pub(crate) fn detach(&self, label: &str) -> Option<WindowHandle> {
        let window = self
            .0
            .windows
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(label)?;
        window.alive.store(false, Ordering::Release);
        Some(WindowHandle {
            engine: self.0.engine.clone(),
            window,
        })
    }
    pub(crate) fn close_document(&self, id: &str) -> Result<()> {
        self.0
            .engine
            .close_document(id)
            .map_err(|e| e.to_string())?;
        // Do not hold the registry lock while waiting for a window operation.
        let windows = self
            .0
            .windows
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for window in windows {
            window
                .data
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .adopted
                .remove(id);
        }
        Ok(())
    }
}
impl WindowHandle {
    pub(crate) fn preview_content(&self,session:&str,source:&str,marks:&[TextRedactionMark]) -> Result<ContentRedactionPreview> {
        self.run(|data|self.engine.preview_content_redaction(data.owner(session)?,source,marks).map_err(|e|e.to_string()))
    }
    pub(crate) fn render_content(&self,session:&str,source:&str,preview:Option<&str>,page:usize,width:u32) -> Result<ContentRedactionRaster> {
        self.run(|data|self.engine.render_content_redaction(data.owner(session)?,source,preview,page,width).map_err(|e|e.to_string()))
    }
    fn run<T>(&self, work: impl FnOnce(&mut WindowData) -> Result<T>) -> Result<T> {
        let mut data = self
            .window
            .data
            .lock()
            .map_err(|_| "Text-removal session state is unavailable.")?;
        if !self.window.alive.load(Ordering::Acquire) {
            self.clean_locked(&mut data);
            return Err("This text-removal window has closed.".into());
        }
        let result = work(&mut data);
        if !self.window.alive.load(Ordering::Acquire) {
            self.clean_locked(&mut data);
            return Err("This text-removal window has closed.".into());
        }
        result
    }
    pub(crate) fn begin(&self) -> Result<String> {
        self.run(|data| {
            if data.sessions.len() >= 16 {
                return Err("Too many open text-removal sessions.".into());
            }
            let owner = self
                .engine
                .begin_text_redaction_session()
                .map_err(|e| e.to_string())?;
            let session = uuid::Uuid::new_v4().to_string();
            data.sessions.insert(session.clone(), owner);
            Ok(session)
        })
    }
    pub(crate) fn preview(
        &self,
        session: &str,
        source: &str,
        marks: &[TextRedactionMark],
    ) -> Result<TextRedactionPreview> {
        self.run(|data| {
            self.engine
                .preview_text_redaction(data.owner(session)?, source, marks)
                .map_err(|e| e.to_string())
        })
    }
    pub(crate) fn prepare(
        &self,
        session: &str,
        request: crate::ExportRequest,
        bookmarks: Option<Vec<crate::DocumentBookmark>>,
    ) -> Result<TextRedactionSource> {
        self.run(|data| {
            self.engine
                .prepare_text_redaction_source(data.owner(session)?, request, bookmarks)
                .map_err(|e| e.to_string())
        })
    }
    pub(crate) fn bounds(
        &self,
        session: &str,
        preview: &str,
        page: usize,
        offset: usize,
        limit: usize,
    ) -> Result<TextRedactionBounds> {
        self.run(|data| {
            self.engine
                .text_redaction_preview_bounds(data.owner(session)?, preview, page, offset, limit)
                .map_err(|e| e.to_string())
        })
    }
    pub(crate) fn apply(&self, session: &str, preview: &str) -> Result<DocumentInfo> {
        self.run(|data| {
            let owner = data.owner(session)?;
            if data.adopted.len() >= 4096 {
                return Err(
                    "Close unused text-removal results before applying another preview.".into(),
                );
            }
            let document = self
                .engine
                .apply_text_redaction_preview(owner, preview)
                .map_err(|e| e.to_string())?;
            // Retain ownership even if the IPC receiver disappears after this call.
            data.adopted.insert(document.id.clone());
            Ok(document)
        })
    }
    pub(crate) fn discard(&self, session: &str, preview: &str) -> Result<()> {
        self.run(|data| {
            self.engine
                .discard_text_redaction_preview(data.owner(session)?, preview)
                .map_err(|e| e.to_string())
        })
    }
    pub(crate) fn release(&self, session: &str) -> Result<()> {
        self.run(|data| {
            self.engine
                .release_text_redaction_owner(data.owner(session)?)
                .map_err(|e| e.to_string())?;
            data.sessions.remove(session);
            Ok(())
        })
    }
    fn clean_locked(&self, data: &mut WindowData) {
        for (_, owner) in data.sessions.drain() {
            let _ = self.engine.release_text_redaction_owner(&owner);
        }
        for id in data.adopted.drain() {
            let _ = self.engine.close_document(&id);
        }
    }
    pub(crate) fn cleanup(&self) {
        self.window.alive.store(false, Ordering::Release);
        self.clean_locked(&mut self.window.data.lock().unwrap_or_else(|e| e.into_inner()));
    }
}
impl WindowData {
    fn owner(&self, session: &str) -> Result<&str> {
        self.sessions
            .get(session)
            .map(String::as_str)
            .ok_or_else(|| {
                "This text-removal session is no longer available in this window.".into()
            })
    }
}
#[cfg(test)]
#[path = "redaction_host_tests.rs"]
mod redaction_host_tests;
