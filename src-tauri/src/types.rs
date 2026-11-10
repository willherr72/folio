use serde::{Deserialize, Serialize};

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct ExistingImage {
    pub object_path:Vec<usize>,
    pub selection_id:String,
    pub corners:[Point;4],
    pub bounds:AnnotationRect,
    pub pixel_width:u32,
    pub pixel_height:u32,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub crop:Option<crate::ImageCrop>,
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct ExistingImages {pub images:Vec<ExistingImage>}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagePageOptions {
    pub width: f32,
    pub height: f32,
    pub margin: f32,
    pub auto_orientation: bool,
}

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
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub bookmarks:Vec<crate::DocumentBookmark>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub bookmark_error:Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protection: Option<ProtectionInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProtectionInfo {
    pub document_id: String,
    pub owner: bool,
    pub revision: i64,
    pub permissions: crate::protection::DocumentPermissions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag="status", rename_all="camelCase", deny_unknown_fields)]
pub enum OpenOutcome {
    Opened {document:DocumentInfo},
    PasswordRequired {
        #[serde(rename="challengeId")]
        challenge_id:String,
        name:String,
        incorrect:bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct SourceReplacement {
    pub source_id:String,
    pub document:DocumentInfo,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct PageContentReplacement {
    pub document: DocumentInfo,
    /// New native page index for each submitted workspace occurrence, in order.
    pub page_indices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct PageSearchResult {
    pub matches:Vec<Vec<AnnotationRect>>,
    pub has_text:bool,
    pub truncated:bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditableTextRun {
    pub object_index: usize,
    pub text: String,
    /// Exact verified PDF operand text when native extraction adds/omits spaces.
    /// `text` remains the native selection identity used by stale-request checks.
    #[serde(default,skip_serializing_if = "Option::is_none")]
    pub source_text: Option<String>,
    pub font_name: String,
    #[serde(default,skip_serializing_if = "Option::is_none")]
    pub font_display_name: Option<String>,
    pub font_size: f32,
    /// Physical vertical em-vector length, including the complete text transform.
    #[serde(default,skip_serializing_if = "Option::is_none")]
    pub display_font_size: Option<f32>,
    pub bounds: AnnotationRect,
    pub supported: bool,
    #[serde(default)]
    pub is_embedded: bool,
    #[serde(default)]
    pub can_substitute: bool,
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

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct FormParagraphRuns {
    pub anchor_index:usize,
    pub runs:Vec<EditableTextRun>,
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
    /// Retained source-display rectangle, before workspace rotation. Public values
    /// are physical points; workers convert this with the source-owned UserUnit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop: Option<AnnotationRect>,
    pub overlays: Vec<Overlay>,
}

impl PagePlan {
    pub(crate) fn validate_crop(&self) -> crate::EngineResult<()> {
        crate::engine::page_crop::validate(self.crop.as_ref(), self.width, self.height, 1.)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Overlay {
    Image(ImageOverlay),
    Text(TextOverlay),
    Ink(InkOverlay),
    Highlight(HighlightOverlay),
    Comment(CommentOverlay),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageOverlay {
    pub id: String,
    pub asset_id: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub rotation: u16,
}
impl ImageOverlay {
    pub(crate) fn validate(&self, page_width: f32, page_height: f32) -> crate::EngineResult<()> {
        if self.id.is_empty() || self.id.len() > 1024
            || self.asset_id.len() != 64
            || !self.asset_id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || !matches!(self.rotation, 0 | 90 | 180 | 270)
            || ![self.x,self.y,self.width,self.height,page_width,page_height].iter().all(|v| v.is_finite())
            || self.width <= 0. || self.height <= 0.
            || self.corners().iter().any(|p| p.x < -0.01 || p.y < -0.01 || p.x > page_width + 0.01 || p.y > page_height + 0.01)
        { return Err(crate::EngineError::InvalidRequest("Image dimensions and rotation must fit inside the page and reference a valid image asset.".into())); }
        Ok(())
    }
    pub(crate) fn corners(&self) -> [Point; 4] {
        [(0.,0.),(self.width,0.),(self.width,self.height),(0.,self.height)].map(|(x,y)| {
            let (x,y) = match self.rotation {90=>(-y,x),180=>(-x,-y),270=>(y,-x),_=>(x,y)};
            Point {x:self.x+x,y:self.y+y}
        })
    }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shaping: Option<TextShaping>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<TextAreaSettings>,
    pub color: String,
    #[serde(default)]
    pub rotation: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextShaping {
    #[serde(deserialize_with = "shaping_version")]
    pub version: u8,
    pub direction: ShapingDirection,
    pub ligatures: bool,
}
fn shaping_version<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u8, D::Error> {
    let version = u8::deserialize(d)?;
    if version != 1 {
        return Err(serde::de::Error::custom("unsupported text shaping version"));
    }
    Ok(version)
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShapingDirection {
    Auto,
    Ltr,
    Rtl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextAreaAlignment {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextAreaSettings {
    #[serde(deserialize_with = "shaping_version")]
    pub version: u8,
    pub width: f32,
    pub height: f32,
    pub inset: f32,
    pub line_spacing: f32,
    pub alignment: TextAreaAlignment,
}

impl TextOverlay {
    /// Structural validation also used by recovery: never shape or discard draft text.
    pub(crate) fn validate_area_settings(&self) -> crate::EngineResult<()> {
        let Some(area) = &self.area else {
            return Ok(());
        };
        if area.version != 1
            || ![area.width, area.height]
                .iter()
                .all(|v| v.is_finite() && *v > 0. && *v <= 14400.)
            || !area.inset.is_finite()
            || area.inset < 0.
            || area.inset * 2. >= area.width.min(area.height)
            || !area.line_spacing.is_finite()
            || !(1. ..=3.).contains(&area.line_spacing)
            || self.font_id.as_ref().is_none_or(|id| id.is_empty())
            || !self
                .shaping
                .as_ref()
                .is_some_and(|s| s.version == 1 && s.direction == ShapingDirection::Ltr)
        {
            return Err(crate::EngineError::InvalidRequest("Text areas require valid version 1 dimensions, inset, spacing, a custom font and LTR shaping.".into()));
        }
        Ok(())
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextGroupPreview {
    pub object_indices: Vec<usize>,
    pub text: String,
    pub font_name: String,
    #[serde(default,skip_serializing_if = "Option::is_none")]
    pub font_display_name: Option<String>,
    pub font_size: f32,
    #[serde(default,skip_serializing_if = "Option::is_none")]
    pub display_font_size: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormTextRun {
    pub object_path: Vec<usize>,
    pub text: String,
    pub font_name: String,
    #[serde(default,skip_serializing_if = "Option::is_none")]
    pub font_display_name: Option<String>,
    pub font_size: f32,
    #[serde(default,skip_serializing_if = "Option::is_none")]
    pub display_font_size: Option<f32>,
    #[serde(default)]
    pub is_embedded: bool,
    pub bounds: AnnotationRect,
    pub supported: bool,
    #[serde(default)]
    pub can_substitute: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_substitution_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormTextRuns {
    pub runs: Vec<FormTextRun>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="lowercase")]
pub enum ParagraphAlignment { Left, Center, Right, Justify }

/// Uniform existing paragraph, with its suggested area in displayed page points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextParagraphSelection {
    pub object_indices: Vec<usize>,
    pub text: String,
    pub font_name: String,
    #[serde(default,skip_serializing_if = "Option::is_none")]
    pub font_display_name: Option<String>,
    pub font_size: f32,
    /// Current opaque RGB color; optional for older serialized callers.
    #[serde(default,skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub bounds: AnnotationRect,
    /// Estimated from multiple rows; None means the geometry is ambiguous.
    #[serde(default,skip_serializing_if = "Option::is_none")]
    pub alignment: Option<ParagraphAlignment>,
    /// Baseline distance as a multiple of effective font size, when regular.
    #[serde(default,skip_serializing_if = "Option::is_none")]
    pub line_spacing: Option<f32>,
}

#[derive(Debug,Clone,Default,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ParagraphFormatting {
    pub font_size: Option<f32>,
    pub alignment: Option<ParagraphAlignment>,
    pub line_spacing: Option<f32>,
    pub font_id: Option<String>,
    /// Explicit replacement color; omission preserves the source color.
    pub color: Option<String>,
}

/// Read-only attributed inspection. IDs are local to this immutable selection;
/// display metadata is never authority for a subsequent font/resource request.
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct StyledParagraphRun { pub text:String, pub style_id:usize }
/// Exact original object ownership in an inspected paragraph; UTF-16 offsets.
/// This desktop-only DTO does not change inspection or automation payloads.
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ParagraphDraftMapping {
    pub source_id:String,pub page_index:usize,pub object_indices:Vec<usize>,pub anchor_index:usize,
    pub paragraph_text:String,pub source_text:String,
    pub source_start:usize,pub source_end:usize,pub paragraph_start:usize,pub paragraph_end:usize,
}
#[derive(Clone)]
pub(crate) struct ParagraphDraftRequest {
    pub indices:Vec<usize>,pub paragraph:String,pub anchor:usize,pub source:String,pub styled:bool,
}
impl ParagraphDraftRequest {
    pub fn checked(indices:&[usize],paragraph:&str,anchor:usize,source:&str,styled:bool)->crate::EngineResult<Self>{
        if indices.is_empty()||indices.len()>4096||indices.windows(2).any(|w|w[0]>=w[1])
            ||!indices.contains(&anchor)||paragraph.is_empty()||paragraph.len()>4096||source.is_empty()||source.len()>4096{
            return Err(crate::EngineError::InvalidRequest("The paragraph draft selection exceeds its limits or changed.".into()));
        }
        Ok(Self{indices:indices.to_vec(),paragraph:paragraph.into(),anchor,source:source.into(),styled})
    }
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct StyledParagraphStyle {
    pub style_id:usize,
    pub object_indices:Vec<usize>,
    pub font_name:String,
    pub font_display_name:Option<String>,
    pub font_size:f32,
    pub color:String,
    pub is_embedded:bool,
    /// Display cue from verified native background ownership; never an override.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub background_color:Option<String>,
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct StyledParagraphSelection {
    /// First logical line baseline in displayed page points, before editor rotation.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub baseline_anchor:Option<Point>,
    pub object_indices:Vec<usize>,
    pub text:String,
    pub runs:Vec<StyledParagraphRun>,
    pub styles:Vec<StyledParagraphStyle>,
    pub bounds:AnnotationRect,
    pub alignment:Option<ParagraphAlignment>,
    pub line_spacing:Option<f32>,
}

/// A target style derived from an ORIGINAL inspected style, not another change.
/// New target IDs must extend the original palette contiguously (at most 64).
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct StyledParagraphStyleChange {
    pub style_id:usize,
    pub source_style_id:usize,
    pub font_size:Option<f32>,
    pub color:Option<String>,
    pub font_id:Option<String>,
}

/// Source-bound layout/publication request. Styles are resolved again from the source;
/// caller-supplied font names or geometry never authorize a font binding.
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct StyledParagraphLayoutRequest {
    /// Placement intent only. Absent retains the legacy frame-top layout policy.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub baseline_anchor:Option<Point>,
    pub object_indices:Vec<usize>,
    pub expected_text:String,
    pub runs:Vec<StyledParagraphRun>,
    #[serde(default)]
    pub style_changes:Vec<StyledParagraphStyleChange>,
    pub bounds:AnnotationRect,
    pub alignment:ParagraphAlignment,
    pub line_spacing:f32,
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct StyledParagraphPlacedRun {
    pub text:String,
    pub style_id:usize,
    /// Baseline origin in raw PDF coordinates, before page rotation/cropping.
    pub origin:[f32;2],
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct StyledParagraphLayoutLine {
    pub runs:Vec<StyledParagraphPlacedRun>,
    pub baseline:f32,
    pub ascent:f32,
    pub descent:f32,
    pub soft_wrapped:bool,
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct StyledParagraphLayout {
    pub lines:Vec<StyledParagraphLayoutLine>,
    /// [left, bottom, right, top] in raw PDF coordinates.
    pub pdf_bounds:[f32;4],
}
