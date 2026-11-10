//! Attributed reflow uses only styles and membership re-proved from the source.
use super::{inspection, invalid, nested_inspection, paragraph_inspection::{self, ParagraphContext, ResolvedParagraph}, text_preview};
use super::original_snapshot::OriginalSnapshot;
use super::font_inputs::{FontChoice, FontInputs};
use crate::{
    AnnotationRect, DocumentInfo, EngineResult, ParagraphAlignment, PdfEngine, Point,
    StyledParagraphLayoutRequest, StyledParagraphRun, StyledParagraphSelection, StyledParagraphStyleChange,
};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::{collections::{BTreeMap, BTreeSet}, path::Path};

const MAX_REQUEST: usize = 32 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub(crate) struct FontOverride {
    pub style_id: usize,
    pub font_id: String,
}
fn validate_overrides(overrides: &[FontOverride], runs: &[Run]) -> EngineResult<()> {
    let mut seen=BTreeSet::new();
    if overrides.len()>64 || overrides.iter().any(|change| {
        change.style_id>=64 || !text_preview::valid_hash(&change.font_id)
            || !seen.insert(change.style_id)
            || !runs.iter().any(|run|run.style_id==change.style_id && !run.text.is_empty())
    }) {return Err(invalid("Font overrides must name unique used source styles and catalog fontIds."));}
    Ok(())
}

pub(super) fn non_null<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Run {
    pub text: String,
    pub style_id: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BaselineAnchor {
    pub x: f32,
    pub y: f32,
}
impl BaselineAnchor {
    fn validate(&self) -> EngineResult<()> {
        if [self.x, self.y].iter().any(|v| !v.is_finite() || v.abs() > 1_000_000.) {
            return Err(invalid("Use a finite paragraph baseline inside the page."));
        }
        Ok(())
    }
    fn native(&self) -> Point { Point { x: self.x, y: self.y } }
}
impl From<Point> for BaselineAnchor {
    fn from(point: Point) -> Self { Self { x: point.x, y: point.y } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Bounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
impl Bounds {
    fn validate(&self) -> EngineResult<()> {
        if ![self.x, self.y, self.width, self.height]
            .iter()
            .all(|v| v.is_finite())
            || self.x < 0.
            || self.y < 0.
            || self.width <= 0.
            || self.height <= 0.
        {
            return Err(invalid(
                "Use a finite positive paragraph rectangle inside the page.",
            ));
        }
        Ok(())
    }
    fn native(&self) -> AnnotationRect {
        AnnotationRect {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }
}
impl From<&AnnotationRect> for Bounds {
    fn from(b: &AnnotationRect) -> Self {
        Self {
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
        }
    }
}
fn validate_runs(runs: &[Run]) -> EngineResult<()> {
    let mut bytes = 0usize;
    if runs.is_empty() || runs.len() > 1024 {
        return Err(invalid("Use 1 to 1024 paragraph style runs."));
    }
    for run in runs {
        bytes = bytes
            .checked_add(run.text.len())
            .filter(|n| *n <= 4096)
            .ok_or_else(|| invalid("Use at most4096 UTF-8 bytes of paragraph text."))?;
        if run.style_id >= 64 || run.text.contains('\r') {
            return Err(invalid("Use original paragraph styles and LF line breaks."));
        }
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ParagraphEdit {
    pub page: u32,
    pub seed_selection_id: String,
    pub paragraph_id: String,
    pub runs: Vec<Run>,
    #[serde(default, skip_serializing_if="Vec::is_empty")]
    pub font_overrides: Vec<FontOverride>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "non_null"
    )]
    pub bounds: Option<Bounds>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "non_null")]
    pub baseline_anchor: Option<BaselineAnchor>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "non_null"
    )]
    pub alignment: Option<ParagraphAlignment>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "non_null"
    )]
    pub line_spacing: Option<f32>,
    #[serde(default)]
    pub fit: bool,
}
impl ParagraphEdit {
    // Run partitioning is representational; character/style pairs express intent.
    // Call only after schema and source palette validation. Unknown source layout
    // cannot authorize an explicit override as unchanged.
    fn preserves_source(&self, source: &StyledParagraphSelection) -> bool {
        self.font_overrides.is_empty() && !self.fit
            && self.baseline_anchor.as_ref().is_none_or(|anchor| source.baseline_anchor == Some(anchor.native()))
            && self.bounds.as_ref().is_none_or(|bounds| bounds.native() == source.bounds)
            && self.alignment.is_none_or(|alignment| source.alignment == Some(alignment))
            && self.line_spacing.is_none_or(|spacing| source.line_spacing == Some(spacing))
            && self.runs.iter()
                .flat_map(|run| run.text.chars().map(move |ch| (ch, run.style_id)))
                .eq(source.runs.iter()
                    .flat_map(|run| run.text.chars().map(move |ch| (ch, run.style_id))))
    }

    pub fn validate(&self) -> EngineResult<()> {
        if !(1..=10000).contains(&self.page)
            || !valid_pair(&self.seed_selection_id, &self.paragraph_id)
        {
            return Err(invalid(
                "Use a page, seedSelectionId and paragraphId from paragraph inspection.",
            ));
        }
        validate_runs(&self.runs)?;
        validate_overrides(&self.font_overrides, &self.runs)?;
        if let Some(anchor) = &self.baseline_anchor { anchor.validate()?; }
        if let Some(bounds) = &self.bounds {
            bounds.validate()?;
        }
        if self
            .line_spacing
            .is_some_and(|v| !v.is_finite() || !(0.6..=3.).contains(&v))
        {
            return Err(invalid("Use paragraph line spacing from0.6 to3."));
        }
        inspection::bounded_json(self, MAX_REQUEST)?;
        Ok(())
    }
    pub fn read(path: &Path) -> EngineResult<Self> {
        let bytes = text_preview::read_bounded(path, MAX_REQUEST as u64)?;
        let edit: Self = serde_json::from_slice(&bytes)
            .map_err(|_| invalid("The paragraph request must match the declared JSON schema."))?;
        edit.validate()?;
        Ok(edit)
    }
}
pub(crate) fn valid_paragraph(value: &str) -> bool {
    value
        .strip_prefix("paragraph-v1-")
        .is_some_and(text_preview::valid_hash)
}
pub(crate) fn valid_nested_paragraph(value: &str) -> bool {
    value.strip_prefix("paragraph-v2-").is_some_and(text_preview::valid_hash)
}
pub(crate) fn valid_pair(seed: &str, paragraph: &str) -> bool {
    if nested_inspection::valid_selection(seed) {
        valid_nested_paragraph(paragraph)
    } else {
        seed.strip_prefix("text-v1-").is_some_and(text_preview::valid_hash) && valid_paragraph(paragraph)
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum ScopeKind {
    #[serde(rename = "outer-form")]
    OuterForm,
}
/// Derived diagnostic metadata, never accepted in a caller edit request.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OccurrenceScope {
    kind: ScopeKind,
    object_path: [usize; 1],
}
impl OccurrenceScope {
    pub(super) fn from_path(path: &[usize]) -> EngineResult<Self> {
        let outer = path.first().ok_or_else(|| invalid("The nested occurrence has no outer path."))?;
        Ok(Self {kind: ScopeKind::OuterForm, object_path: [*outer]})
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ParagraphMetadata {
    pub seed_selection_id: String,
    pub runs: Vec<Run>,
    #[serde(default, skip_serializing_if="Vec::is_empty")]
    pub font_overrides: Vec<FontOverride>,
    #[serde(default, skip_serializing_if="Vec::is_empty")]
    pub substitute_fonts: Vec<FontChoice>,
    pub bounds: Bounds,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "non_null")]
    pub baseline_anchor: Option<BaselineAnchor>,
    pub alignment: ParagraphAlignment,
    pub line_spacing: f32,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "non_null")]
    pub occurrence_scope: Option<OccurrenceScope>,
}
impl ParagraphMetadata {
    pub fn validate(&self) -> EngineResult<()> {
        if !paragraph_inspection::valid_seed(&self.seed_selection_id)
            || nested_inspection::valid_selection(&self.seed_selection_id) != self.occurrence_scope.is_some()
            || !self.line_spacing.is_finite()
            || !(0.6..=3.).contains(&self.line_spacing)
        {
            return Err(invalid("Invalid paragraph preview metadata."));
        }
        validate_runs(&self.runs)?;
        validate_overrides(&self.font_overrides, &self.runs)?;
        if let Some(anchor) = &self.baseline_anchor { anchor.validate()?; }
        let expected=self.font_overrides.iter().map(|change|change.font_id.as_str()).collect::<BTreeSet<_>>();
        let mut actual=BTreeSet::new();let mut bytes=0usize;
        if self.substitute_fonts.len()>16 {return Err(invalid("Too many substitute fonts in preview metadata."));}
        for choice in &self.substitute_fonts {
            choice.validate()?;
            if !actual.insert(choice.font_id.as_str()) {return Err(invalid("Duplicate substitute font metadata."));}
            bytes=bytes.checked_add(choice.bytes).filter(|&n|n<=64*1024*1024).ok_or_else(||invalid("Substitute font metadata exceeds its byte limit."))?;
        }
        if actual!=expected {return Err(invalid("Preview font choices do not match the requested overrides."));}
        self.bounds.validate()
    }
    pub fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }
}

pub(super) fn preview(
    engine: &PdfEngine,
    info: &DocumentInfo,
    edit: ParagraphEdit,
    parent: &Path,
    password: Option<&str>,
    original: &OriginalSnapshot,
    fonts: &FontInputs,
) -> EngineResult<Value> {
    edit.validate()?;
    let ResolvedParagraph { selection: source, source_fingerprint, context } = paragraph_inspection::resolve(
        engine,
        info,
        edit.page,
        &edit.seed_selection_id,
        &edit.paragraph_id,
    )?;
    if edit.runs.iter().any(|run| {
        !source
            .styles
            .iter()
            .any(|style| style.style_id == run.style_id)
    }) {
        return Err(invalid(
            "A requested style is not in the inspected paragraph palette.",
        ));
    }
    let unchanged = edit.preserves_source(&source);
    let mut used_fonts=BTreeMap::new();
    let mut style_changes=Vec::new();
    for change in &edit.font_overrides {
        // Runs were bound to the source palette above; overrides must reference
        // a nonempty used run, never a caller-invented source style.
        let asset=fonts.get(&change.font_id)?;
        used_fonts.insert(change.font_id.clone(),FontChoice::from_asset(&asset)?);
        style_changes.push(StyledParagraphStyleChange {style_id:change.style_id,source_style_id:change.style_id,font_id:Some(change.font_id.clone()),font_size:None,color:None});
    }
    let _font_install=fonts.install(&engine.fonts())?;
    let index = edit.page as usize - 1;
    // Dimension changes resize the area and retain the source baseline. This
    // includes Fit's added ascent headroom. An equal-size rectangle is a move;
    // an explicit anchor supplies intent for a combined move and resize.
    let baseline_anchor = edit.baseline_anchor.as_ref().map(BaselineAnchor::native)
        .or_else(|| source.baseline_anchor.map(|mut anchor| {
            if let Some(bounds) = &edit.bounds {
                if bounds.width == source.bounds.width && bounds.height == source.bounds.height {
                    anchor.x += bounds.x - source.bounds.x;
                    anchor.y += bounds.y - source.bounds.y;
                }
            }
            anchor
        }));
    let mut request = StyledParagraphLayoutRequest {
        baseline_anchor,object_indices: source.object_indices,
        expected_text: source.text.clone(),
        runs: edit
            .runs
            .iter()
            .map(|r| StyledParagraphRun {
                text: r.text.clone(),
                style_id: r.style_id,
            })
            .collect(),
        style_changes,
        bounds: edit
            .bounds
            .as_ref()
            .map(Bounds::native)
            .unwrap_or(source.bounds),
        alignment: edit
            .alignment
            .or(source.alignment)
            .unwrap_or(ParagraphAlignment::Left),
        line_spacing: edit.line_spacing.or(source.line_spacing).unwrap_or(1.2),
    };
    if edit.fit {
        request.bounds = match &context {
            ParagraphContext::Page => engine.fit_occurrence_styled_paragraph(&info.id, index, &request)?,
            ParagraphContext::Nested { path } => engine.fit_form_styled_paragraph(&info.id, index, path, &request)?,
        };
    }
    let order = (0..info.pages.len()).collect::<Vec<_>>();
    let changed = match &context {
        ParagraphContext::Page => engine.preview_styled_paragraph_occurrence(&info.id, &order, index, &request)?,
        ParagraphContext::Nested { path } => engine.preview_form_styled_paragraph_occurrence(&info.id, &order, index, path, &request)?,
    };
    let paragraph = ParagraphMetadata {
        seed_selection_id: edit.seed_selection_id,
        runs: edit.runs,
        font_overrides: edit.font_overrides,
        substitute_fonts: used_fonts.into_values().collect(),
        bounds: Bounds::from(&request.bounds),
        baseline_anchor: request.baseline_anchor.map(BaselineAnchor::from),
        alignment: request.alignment,
        line_spacing: request.line_spacing,
        occurrence_scope: context.scope()?,
    };
    let description = text_preview::PreviewDescription {
        unchanged_input: unchanged.then_some(original),
        source_fingerprint,
        page: edit.page,
        selection_id: edit.paragraph_id,
        old_text: source.text,
        replacement: paragraph.text(),
        substitute_font: None,
        paragraph: Some(paragraph),
        form: None,
    };
    text_preview::publish(engine, info, changed, description, parent, password)
}

#[cfg(test)]
mod no_op_tests {
    use super::*;

    fn source() -> StyledParagraphSelection {
        StyledParagraphSelection {
            baseline_anchor: None,
            object_indices: vec![0, 1],
            text: "Café🙂 e\u{301}\nnext".into(),
            runs: vec![
                StyledParagraphRun { text: "Café🙂 e\u{301}".into(), style_id: 0 },
                StyledParagraphRun { text: "\nnext".into(), style_id: 1 },
            ],
            styles: vec![], // Palette authorization is exercised by preview, before this comparison.
            bounds: AnnotationRect { x: 10., y: 20., width: 100., height: 40. },
            alignment: None,
            line_spacing: None,
        }
    }

    fn request(source: &StyledParagraphSelection) -> ParagraphEdit {
        ParagraphEdit {
            page: 1,
            seed_selection_id: format!("text-v1-{}", "a".repeat(64)),
            paragraph_id: format!("paragraph-v1-{}", "b".repeat(64)),
            runs: source.runs.iter().map(|run| Run { text: run.text.clone(), style_id: run.style_id }).collect(),
            font_overrides: vec![],
            bounds: None,
            baseline_anchor: None,
            alignment: None,
            line_spacing: None,
            fit: false,
        }
    }

    #[test]
    fn explicit_font_override_is_never_a_no_op() {
        let source=source();let mut edit=request(&source);
        assert!(edit.preserves_source(&source));
        edit.font_overrides.push(FontOverride {style_id:0,font_id:"c".repeat(64)});
        assert!(!edit.preserves_source(&source));
        assert!(edit.validate().is_ok());
        edit.font_overrides.push(edit.font_overrides[0].clone());assert!(edit.validate().is_err());
        edit.font_overrides.truncate(1);edit.font_overrides[0].style_id=63;assert!(edit.validate().is_err());
    }

    #[test]
    fn unicode_run_partitioning_and_empty_runs_are_not_edits() {
        let source = source();
        let mut edit = request(&source);
        assert!(edit.preserves_source(&source));
        edit.runs = source.runs.iter().flat_map(|run| {
            run.text.chars().map(move |ch| Run { text: ch.to_string(), style_id: run.style_id })
        }).collect();
        edit.runs.insert(0, Run { text: String::new(), style_id: 0 });
        edit.runs.push(Run { text: String::new(), style_id: 1 });
        assert!(edit.preserves_source(&source));
        edit.runs.iter_mut().find(|run| run.text == "🙂").unwrap().style_id = 1;
        assert!(!edit.preserves_source(&source), "Same text with changed style remains an edit");
        let mut edit = request(&source);
        edit.runs[0].text.push(' ');
        assert!(!edit.preserves_source(&source), "Whitespace changes are edits");
    }

    #[test]
    fn only_exact_known_layout_values_can_be_echoed_without_an_edit() {
        let mut source = source();
        let mut edit = request(&source);
        edit.bounds = Some(Bounds::from(&source.bounds));
        assert!(edit.preserves_source(&source));
        edit.bounds.as_mut().unwrap().width = f32::from_bits(source.bounds.width.to_bits() + 1);
        assert!(!edit.preserves_source(&source), "Even a small intentional resize is an edit");
        edit.bounds = None;
        edit.alignment = Some(ParagraphAlignment::Left);
        assert!(!edit.preserves_source(&source), "Unknown alignment is not an implicit Left match");
        source.alignment = Some(ParagraphAlignment::Left);
        assert!(edit.preserves_source(&source));
        edit.alignment = Some(ParagraphAlignment::Right);
        assert!(!edit.preserves_source(&source));
        edit.alignment = None;
        edit.line_spacing = Some(1.2);
        assert!(!edit.preserves_source(&source), "Unknown spacing is not an implicit 1.2 match");
        source.line_spacing = Some(1.2);
        assert!(edit.preserves_source(&source));
        edit.line_spacing = Some(f32::from_bits(1.2f32.to_bits() + 1));
        assert!(!edit.preserves_source(&source));
        edit.line_spacing = None;
        edit.fit = true;
        assert!(!edit.preserves_source(&source), "Fit is an explicit layout operation");
    }
}
