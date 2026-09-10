use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub overlays: Vec<Overlay>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentInfo {
    pub id: String,
    pub name: String,
    pub pages: Vec<PageInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub pages: Vec<PagePlan>,
    #[serde(default)]
    pub flatten: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PagePlan {
    pub id: String,
    pub source_id: String,
    pub page_index: usize,
    pub width: f32,
    pub height: f32,
    pub rotation: u16,
    pub overlays: Vec<Overlay>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Overlay {
    Text(TextOverlay),
    Ink(InkOverlay),
    Highlight(HighlightOverlay),
    Comment(CommentOverlay),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextOverlay {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub font_size: f32,
    pub color: String,
    #[serde(default)]
    pub rotation: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InkOverlay {
    pub id: String,
    pub paths: Vec<Vec<Point>>,
    pub color: String,
    pub stroke_width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnnotationRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HighlightOverlay {
    pub id: String,
    pub rects: Vec<AnnotationRect>,
    pub color: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommentOverlay {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub color: String,
}

/// Embedded text in PDFium reading order, in displayed page points before editor rotation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfTextCharacter {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageText {
    /// Source page rotation, already included in the normalized character bounds.
    #[serde(default)]
    pub intrinsic_rotation: u16,
    pub characters: Vec<PdfTextCharacter>,
}
