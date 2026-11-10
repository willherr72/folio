//! The native boundary repeats OCR validation; browser checks are not authority.
use crate::{EngineError, EngineResult};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OcrBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OcrWord {
    pub text: String,
    pub confidence: f32,
    pub bounds: OcrBounds,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OcrLine {
    pub words: Vec<OcrWord>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OcrParagraph {
    pub lines: Vec<OcrLine>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OcrBlock {
    pub paragraphs: Vec<OcrParagraph>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OcrPage {
    pub width: u32,
    pub height: u32,
    pub blocks: Vec<OcrBlock>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OcrUpdate {
    pub page_index: usize,
    pub rotation: u16,
    pub page: OcrPage,
    #[serde(default)]
    pub preserve_owned_orientation: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrPreparedPage {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub effective_dpi: f32,
    pub exclusions: Vec<OcrBounds>,
    pub owned: Option<OcrPage>,
}

pub(crate) fn invalid(reason: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("PDF OCR: {reason}"))
}
impl OcrPage {
    pub(crate) fn validate(&self) -> EngineResult<(usize, usize)> {
        if self.width == 0
            || self.height == 0
            || self.width > 6000
            || self.height > 6000
            || u64::from(self.width) * u64::from(self.height) > 20_000_000
        {
            return Err(invalid(
                "raster exceeds 6000 pixels per side or 20 million pixels",
            ));
        }
        let mut words = 0usize;
        let mut scalars = 0usize;
        let mut lines = 0usize;
        let mut containers = self.blocks.len();
        let mut bytes = 0usize;
        if containers > 30_000 {
            return Err(invalid("container limit"));
        }
        for block in &self.blocks {
            containers = containers.saturating_add(block.paragraphs.len());
            if containers > 30_000 {
                return Err(invalid("container limit"));
            }
            for paragraph in &block.paragraphs {
                containers = containers.saturating_add(paragraph.lines.len());
                lines = lines.saturating_add(paragraph.lines.len());
                if containers > 30_000 || lines > 10_000 {
                    return Err(invalid("line or container limit"));
                }
                for line in &paragraph.lines {
                    words = words.saturating_add(line.words.len());
                    if words > 20_000 {
                        return Err(invalid("word limit"));
                    }
                    for word in &line.words {
                        bytes = bytes.saturating_add(word.text.len());
                        if bytes > 2 * 1024 * 1024 {
                            return Err(invalid("text byte limit"));
                        }
                        scalars = scalars.saturating_add(word.text.chars().count());
                        if scalars > 100_000 {
                            return Err(invalid("Unicode scalar limit"));
                        }
                        if word.text.trim().is_empty() || word.text.chars().any(char::is_control) {
                            return Err(invalid("empty or control text"));
                        }
                        let b = &word.bounds;
                        if ![b.x, b.y, b.width, b.height, word.confidence]
                            .iter()
                            .all(|n| n.is_finite())
                            || word.confidence < 0.
                            || word.confidence > 100.
                            || b.x < 0.
                            || b.y < 0.
                            || b.width <= 0.
                            || b.height <= 0.
                            || b.x + b.width > self.width as f32
                            || b.y + b.height > self.height as f32
                        {
                            return Err(invalid(
                                "word geometry or confidence is outside the raster",
                            ));
                        }
                    }
                }
            }
        }
        Ok((words, scalars))
    }
    pub(crate) fn lines(&self) -> impl Iterator<Item = &OcrLine> {
        self.blocks
            .iter()
            .flat_map(|b| &b.paragraphs)
            .flat_map(|p| &p.lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn page() -> OcrPage {
        serde_json::from_value(
            serde_json::json!({"width":100,"height":80,"blocks":[{"paragraphs":[{"lines":[{"words":[
                {"text":"café 𐐀","confidence":92,"bounds":{"x":10,"y":10,"width":40,"height":12}}
            ]}]}]}]}),
        )
        .unwrap()
    }
    #[test]
    fn validates_unicode_geometry_and_counts_without_mutating() {
        let input = page();
        let counts = input.validate().unwrap();
        assert_eq!(counts, (1, 6));
        assert_eq!(
            input.blocks[0].paragraphs[0].lines[0].words[0].text,
            "café 𐐀"
        );
    }
    #[test]
    fn refuses_invalid_bounds_confidence_controls_and_limits() {
        for which in 0..6 {
            let mut input = page();
            let word = &mut input.blocks[0].paragraphs[0].lines[0].words[0];
            match which {
                0 => word.bounds.x = -1.,
                1 => word.bounds.width = 100.,
                2 => word.bounds.height = f32::NAN,
                3 => word.confidence = 101.,
                4 => word.text = "bad\nword".into(),
                _ => word.text = "x".repeat(100_001),
            };
            assert!(input.validate().is_err(), "case {which}");
        }
        let mut input = page();
        input.width = 6001;
        assert!(input.validate().is_err());
        let mut input = page();
        input.width = 5000;
        input.height = 5000;
        assert!(input.validate().is_err());
        let mut input = page();
        let word = input.blocks[0].paragraphs[0].lines[0].words[0].clone();
        input.blocks[0].paragraphs[0].lines[0].words = vec![word; 20_001];
        assert!(input.validate().is_err());
    }
    #[test]
    fn empty_page_is_valid_for_removal_and_unknown_properties_are_rejected() {
        let mut input = page();
        input.blocks.clear();
        assert_eq!(input.validate().unwrap(), (0, 0));
        let mut value = serde_json::to_value(input).unwrap();
        value["untrusted"] = true.into();
        assert!(serde_json::from_value::<OcrPage>(value).is_err());
    }
}
