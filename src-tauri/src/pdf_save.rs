//! Cooperative ordinary PDF saving with one final publication boundary.
use crate::{EngineError, EngineResult};
use std::sync::Arc;

pub use crate::PageImageControl as PdfSaveControl;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfSaveProgress {
    pub stage: &'static str,
    pub completed: usize,
    pub total: usize,
}

pub(crate) struct PdfSaveMonitor {
    pub control: Arc<PdfSaveControl>,
    pub total: usize,
    pub report: Arc<dyn Fn(PdfSaveProgress) + Send + Sync>,
}
impl PdfSaveMonitor {
    pub fn check_cancelled(&self) -> EngineResult<()> {
        if self.control.cancelled() {
            Err(EngineError::InvalidRequest("PDF save cancelled".into()))
        } else { Ok(()) }
    }
    pub fn emit(&self, stage: &'static str, completed: usize) {
        (self.report)(PdfSaveProgress { stage, completed, total: self.total });
    }
    pub fn checkpoint(&self, stage: &'static str, completed: usize) -> EngineResult<()> {
        self.check_cancelled()?;
        // Bound channel traffic, while checking cancellation on every page.
        if completed == 0 || completed == self.total || completed % 32 == 0 {
            self.emit(stage, completed);
        }
        self.check_cancelled()
    }
}
