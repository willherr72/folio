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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditableTextRun {
    pub object_index: usize,
    pub text: String,
    pub font_name: String,
    pub font_size: f32,
    pub bounds: AnnotationRect,
    pub supported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRuns {
    pub runs: Vec<EditableTextRun>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
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
    #[serde(default)]
    pub font_name: TextFont,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_id: Option<String>,
    pub color: String,
    #[serde(default)]
    pub rotation: u16,
}

/// Portable PDF standard fonts; unknown wire values are rejected by serde.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextFont {
    #[default]
    Helvetica,
    #[serde(rename = "Helvetica-Bold")]
    HelveticaBold,
    #[serde(rename = "Helvetica-Oblique")]
    HelveticaOblique,
    #[serde(rename = "Helvetica-BoldOblique")]
    HelveticaBoldOblique,
    #[serde(rename = "Times-Roman")]
    TimesRoman,
    #[serde(rename = "Times-Bold")]
    TimesBold,
    #[serde(rename = "Times-Italic")]
    TimesItalic,
    #[serde(rename = "Times-BoldItalic")]
    TimesBoldItalic,
    Courier,
    #[serde(rename = "Courier-Bold")]
    CourierBold,
    #[serde(rename = "Courier-Oblique")]
    CourierOblique,
    #[serde(rename = "Courier-BoldOblique")]
    CourierBoldOblique,
}

impl TextFont {
    pub(crate) fn pdf_name(self) -> &'static str {
        match self {
            Self::Helvetica => "Helvetica",
            Self::HelveticaBold => "Helvetica-Bold",
            Self::HelveticaOblique => "Helvetica-Oblique",
            Self::HelveticaBoldOblique => "Helvetica-BoldOblique",
            Self::TimesRoman => "Times-Roman",
            Self::TimesBold => "Times-Bold",
            Self::TimesItalic => "Times-Italic",
            Self::TimesBoldItalic => "Times-BoldItalic",
            Self::Courier => "Courier",
            Self::CourierBold => "Courier-Bold",
            Self::CourierOblique => "Courier-Oblique",
            Self::CourierBoldOblique => "Courier-BoldOblique",
        }
    }
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
