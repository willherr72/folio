mod app;
mod engine;
mod fonts;
#[cfg(feature = "shaped-text")]
mod semantic_pdf;
#[cfg(feature = "shaped-text")]
mod shaped_pdf;
#[cfg(feature = "shaped-text")]
pub use semantic_pdf::{create_semantic_font_banks_probe, create_semantic_pdf};
#[cfg(feature = "shaped-text")]
mod shaping;
#[cfg(feature = "shaped-text")]
pub use shaped_pdf::{create_shaped_pdf, ShapedPdf};
#[cfg(feature = "shaped-text")]
pub use shaping::{
    shape_text, PositionedGlyph, ShapedRun, ShapedText, TextBoundary, TextBounds, TextDirection,
    TextRange,
};
mod types;
pub use fonts::{FontAsset, FontInfo, FontRegistry, InstalledFont};

pub use engine::{EngineError, EngineResult, PdfEngine};
pub use types::{
    AnnotationRect, CommentOverlay, DocumentInfo, EditableTextRun, ExportRequest, HighlightOverlay,
    InkOverlay, Overlay, PageInfo, PagePlan, PageText, PdfTextCharacter, Point, TextFont,
    TextOverlay, TextRuns,
};

pub use app::run;

mod recovery;
pub use recovery::RecoveryStore;
pub mod printing;
