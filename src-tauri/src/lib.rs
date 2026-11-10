#[cfg(feature="native-profiling")]
pub mod native_profile;
#[cfg(all(test,feature="native-profiling"))]
mod native_profile_tests;
macro_rules! native_phase {
    ($label:literal) => {{
        #[cfg(feature="native-profiling")] { $crate::native_profile::span($label) }
        #[cfg(not(feature="native-profiling"))] { () }
    }};
}
pub(crate) use native_phase;

mod request_parse;
mod app;
mod redaction_content;
mod redaction_numbers;
mod redaction_fonts;
mod redaction_graphics;
mod redaction_core;
mod redaction_ranges;
mod redaction_text_advances;
mod redaction_area_paths;
mod redaction_area_content;
mod redaction_images;
mod redaction_jpeg;
mod redaction_jpeg_coverage;
mod redaction_sampling;
mod redaction_render_sizing;
mod redaction_render_transform;
mod redaction_image_pixels;
mod redaction_area_images;
mod redaction_host;
mod redaction_commands;
#[cfg(test)]
mod redaction_loader_tests;
#[cfg(test)]
mod redaction_ranges_tests;
#[cfg(test)]
mod redaction_core_tests;
#[cfg(test)]
mod redaction_jpeg_publication_tests;
#[cfg(test)]
mod redaction_graphics_tests;
pub mod automation;
pub mod cli;
pub mod mcp;
mod acroforms;
mod form_fill;
mod form_fill_identity;
mod form_fill_publish;
mod form_answer_appearance;
#[cfg(test)]
mod form_answer_resources_tests;
mod form_author;
mod form_position;
mod form_frame_paint;
mod form_frame;
pub use form_frame::{FormFrameAppearance,FormFrameColor,FormFrameChanges,FrameColorChange,FrameStyle,SetFormFrameRequest};
mod form_frame_render;
mod form_appearance;
mod form_reflow;
mod form_text_appearance;
mod form_appearance_value;
mod form_properties;
mod form_removal;
pub use form_removal::{FormRemovalInfo, RemoveFormFieldRequest};
mod form_choice_model;
mod form_choices;
mod form_choice_draft;
mod form_choice_probe;
mod form_choice_appearance;
mod form_choice_publish;
mod choice_font;
#[cfg(test)]
mod choice_font_tests;
mod form_choice_text;
#[cfg(test)]
mod form_choice_text_tests;
pub use form_choice_draft::{prepare_choice_edit_pdf, PreparedChoiceEdit, SetFormChoicesRequest};
#[cfg(test)]
mod form_choice_draft_tests;
pub use form_choices::{inspect_choice_pdf, ChoiceWidgetLocation, FormChoices};
pub use form_choice_model::{resolve_choice_edit, ChoiceDraftOption, ChoiceEdit, ChoiceEditSelection, ChoiceModel, ChoiceOption, ChoiceSelection, ResolvedChoiceEdit};
pub use form_properties::{FormProperties,FormPropertyChanges,SetFormPropertiesRequest};
pub use form_text_appearance::{FormTextAppearance,TextFieldAlignment,TextFieldAppearanceChanges,SetFormTextAppearanceRequest};
pub use form_position::PositionFormFieldRequest;
pub use form_author::{CreateFormFieldRequest, NewFormField};
pub mod protection;
pub use protection::ProtectionChange;
pub mod private_storage;
mod source_storage;
pub use form_fill::{FillFormFieldRequest, FormField, FormFieldValue};
mod engine;
mod pdf_output;
mod document_metadata_xml;
mod document_metadata;
mod document_properties;
mod document_properties_xml;
pub use document_properties::{DocumentMetadataEdit,DocumentProperties};
mod composition_graph;
mod composition_annotation_receipts;
mod composition_annotation_removal;
mod composition_copy;
mod composition_occurrences;
mod composition_structure;
mod composition_pages;
mod composition_projection;
mod composition_catalog;
mod composition_forms;
mod composition_form_defaults;
mod composition_form_budget;
mod composition_form_assembly;
mod composition_form_resources;
mod composition_form_da;
mod composition_form_scope;
mod composition_form_patch;
mod composition_form_catalog;
mod composition_form_actions;
mod composition_form_selection;
mod composition_button_order;
mod composition_form_normalize;
mod composition_outlines;
mod composition_destinations;
mod composition_navigation;
mod composition_language_scope;
mod composition_annotation_language;
mod composition_annotation_additions;
mod composition_language;
mod composition_structure_merge;
#[cfg(test)]
mod composition_io_tests;
mod pdf_save;
mod pdf_save_commands;
pub use pdf_save::{PdfSaveControl, PdfSaveProgress};
mod page_image_export;
mod text_export;
mod text_export_commands;
pub use text_export::{TextExportRequest,TextExportResult,TextExportProgress,TextExportControl};
mod image_export_folder;
mod image_export_commands;
mod pdf_split;
mod pdf_split_commands;
pub use pdf_split::{SplitPdfRequest,SplitPdfPart};
pub use page_image_export::{PageImageFormat,PageImageOptions,PageImageExportRequest,PageImageExportResult,PageImageProgress,PageImageControl};
mod page_resources;
mod fonts;
mod image_assets;
mod image_pdf;
mod existing_image_model;
pub use existing_image_model::{ImageCrop,ImageFitMode,ImageTransform,ExistingImageEdit};
pub use types::{ExistingImage,ExistingImages};
mod image_persistence;
pub use image_pdf::{prepare_pdf_image, PreparedPdfImage};
pub use image_assets::{DecodedImage, ImageAsset, ImageInfo, ImageRegistration, ImageRegistry};
#[cfg(feature = "shaped-text")]
mod semantic_font;
#[cfg(feature = "shaped-text")]
mod semantic_pdf;
#[cfg(feature = "shaped-text")]
mod shaped_pdf;
#[cfg(feature = "shaped-text")]
pub use semantic_pdf::{
    create_semantic_font_banks_probe, create_semantic_pdf, prepare_semantic_text,
    prepare_text_overlay, NativeGlyphOutline, NativeGlyphPlacement, NativeTextPreview,
    PreparedSemanticText, PreparedTextOverlay,
};
#[cfg(feature = "shaped-text")]
mod shaping;
#[cfg(feature = "shaped-text")]
mod text_area;
#[cfg(feature = "shaped-text")]
mod text_area_overlay;
#[cfg(feature = "shaped-text")]
pub use shaped_pdf::{create_shaped_pdf, ShapedPdf};
#[cfg(feature = "shaped-text")]
pub use shaping::{
    shape_text, PositionedGlyph, ShapedRun, ShapedText, TextBoundary, TextBounds, TextDirection,
    TextRange,
};
#[cfg(feature = "shaped-text")]
pub use text_area::{
    layout_text_area, TextAreaBreak, TextAreaLayout, TextAreaLine, TextAreaOverflow,
    TextAreaRequest,
};
mod types;
mod ocr_types;
#[cfg(feature="shaped-text")]
mod ocr_pdf;
#[cfg(feature="shaped-text")]
mod ocr_font;
pub use ocr_types::{OcrBounds,OcrWord,OcrLine,OcrParagraph,OcrBlock,OcrPage,OcrUpdate,OcrPreparedPage};
pub mod bookmarks;
pub use bookmarks::{DocumentBookmark,PdfValue};
pub use fonts::{FontAsset, FontInfo, FontRegistry, InstalledFont};

pub use engine::native_comments::{NativeComment,EditNativeCommentRequest,DeleteNativeCommentRequest};
pub use engine::{EngineError, EngineResult, PdfEngine};
pub use types::{
    AnnotationRect, CommentOverlay, ImageOverlay, DocumentInfo, ProtectionInfo, OpenOutcome, SourceReplacement, PageContentReplacement, PageSearchResult, EditableTextRun, ExportRequest, FormTextRun, ImagePageOptions,
    FormTextRuns, FormParagraphRuns, HighlightOverlay, InkOverlay, Overlay, PageInfo, PagePlan, PageText,
    PdfTextCharacter, Point, ShapingDirection, TextAreaAlignment, TextAreaSettings, TextFont,
    TextGroupPreview, TextOverlay, TextParagraphSelection, ParagraphFormatting, ParagraphAlignment, TextRuns, TextShaping,
    ParagraphDraftMapping, StyledParagraphRun, StyledParagraphStyle, StyledParagraphSelection, StyledParagraphStyleChange,
    StyledParagraphLayoutRequest, StyledParagraphPlacedRun, StyledParagraphLayoutLine, StyledParagraphLayout,
};

pub use app::run;

mod recovery;
pub use recovery::RecoveryStore;
pub mod printing;

mod form_graph;
