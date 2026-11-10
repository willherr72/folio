//! Source-bound form inspection and native faithful filling across short-lived workers.
use super::{inspection::bounded_json, invalid, text_preview};
use crate::{
    DocumentInfo, EngineResult, FillFormFieldRequest, FormField, FormFieldValue,
    PageContentReplacement, PdfEngine,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;

pub(crate) const MAX_REQUEST: usize = 256 * 1024;
pub(crate) const MAX_MANIFEST: usize = 512 * 1024;
const MAX_INSPECTION: usize = 512 * 1024;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum FieldValue {
    Text { text: String },
    Toggle { checked: bool },
    Choice { indices: Vec<usize> },
}
impl FieldValue {
    pub fn normalized(&self) -> Self {
        match self {
            Self::Text { text } => Self::Text {
                text: text.replace("\r\n", "\n").replace('\r', "\n"),
            },
            _ => self.clone(),
        }
    }
    pub fn validate(&self) -> EngineResult<()> {
        match self.normalized() {
            Self::Text { text } if text.len() > 32768 || text.contains('\0') => Err(invalid(
                "Form text contains NUL or exceeds 32768 UTF-8 bytes.",
            )),
            Self::Choice { indices }
                if indices.len() > 2000
                    || indices.iter().any(|i| *i >= 2000)
                    || indices.windows(2).any(|w| w[0] >= w[1]) =>
            {
                Err(invalid(
                    "Choice indices must be ordered, unique and between 0 and 1999.",
                ))
            }
            _ => Ok(()),
        }
    }
    fn native(&self) -> FormFieldValue {
        match self.normalized() {
            Self::Text { text } => FormFieldValue::Text { text },
            Self::Toggle { checked } => FormFieldValue::Toggle { checked },
            Self::Choice { indices } => FormFieldValue::Choice { indices },
        }
    }
    fn from_native(value: &FormFieldValue) -> Self {
        match value {
            FormFieldValue::Text { text } => Self::Text { text: text.clone() },
            FormFieldValue::Toggle { checked } => Self::Toggle { checked: *checked },
            FormFieldValue::Choice { indices } => Self::Choice {
                indices: indices.clone(),
            },
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FormEdit {
    pub page: u32,
    pub field_id: String,
    pub value: FieldValue,
}
impl FormEdit {
    pub fn validate(&self) -> EngineResult<()> {
        if !(1..=10000).contains(&self.page) || !valid_field(&self.field_id) {
            return Err(invalid("Choose a fieldId and page from form inspection."));
        }
        self.value.validate()?;
        bounded_json(self, MAX_REQUEST)?;
        Ok(())
    }
    pub fn read(path: &Path) -> EngineResult<Self> {
        let bytes = text_preview::read_bounded(path, MAX_REQUEST as u64)?;
        let edit: Self =
            serde_json::from_slice(&bytes).map_err(|_| invalid("Invalid form request schema."))?;
        edit.validate()?;
        Ok(edit)
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FormMetadata {
    pub old_value: FieldValue,
    pub value: FieldValue,
}
impl FormMetadata {
    pub fn validate(&self) -> EngineResult<()> {
        for value in [&self.old_value, &self.value] {
            value.validate()?;
            if *value != value.normalized() {
                return Err(invalid(
                    "Form preview values must use normalized line endings.",
                ));
            }
        }
        Ok(())
    }
}
pub(crate) fn valid_field(id: &str) -> bool {
    id.strip_prefix("field-v1-")
        .is_some_and(text_preview::valid_hash)
}
pub(crate) fn check_response(value: &Value) -> EngineResult<()> {
    bounded_json(value, 2 * 1024 * 1024 - 1)
        .map(|_| ())
        .map_err(|_| {
            invalid("Form inspection exceeds its 2 MiB response limit; nothing was truncated.")
        })
}
fn page_index(info: &DocumentInfo, page: u32) -> EngineResult<usize> {
    if page == 0 || page > 10000 || page as usize > info.pages.len() {
        return Err(invalid("The form page is outside this PDF."));
    }
    Ok(page as usize - 1)
}
fn identity(hash: &[u8], page: u32, field: &FormField) -> EngineResult<String> {
    let bytes = bounded_json(field, MAX_INSPECTION)?;
    let mut digest = Sha256::new();
    digest.update(b"folio-form-field-v1\0");
    digest.update(hash);
    digest.update(page.to_be_bytes());
    digest.update(bytes);
    Ok(format!("field-v1-{}", hex(&digest.finalize())))
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub(super) fn inspect(
    engine: &PdfEngine,
    info: &DocumentInfo,
    page: u32,
    offset: usize,
) -> EngineResult<Value> {
    let index = page_index(info, page)?;
    let fields = engine.list_form_fields(&info.id, index)?;
    if offset > fields.len() {
        return Err(invalid("Form offset is outside the field list."));
    }
    let hash = Sha256::digest(engine.source_bytes(&info.id)?.as_ref());
    let result = |values: Vec<Value>| {
        let end = offset + values.len();
        json!({"page":page,"width":info.pages[index].width,"height":info.pages[index].height,"coordinateSpace":"displayed-page-points","sourceFingerprint":hex(&hash),"offset":offset,"totalFields":fields.len(),"nextOffset":if end<fields.len(){Some(end)}else{None},"fields":values})
    };
    let mut values = Vec::new();
    for field in fields.iter().skip(offset).take(25) {
        let mut value =
            serde_json::to_value(field).map_err(|_| invalid("Cannot encode form field."))?;
        value["fieldId"] = json!(identity(&hash, page, field)?);
        values.push(value);
        if bounded_json(&result(values.clone()), MAX_INSPECTION).is_err() {
            values.pop();
            if values.is_empty() {
                return Err(invalid(
                    "This field exceeds the form inspection response limit.",
                ));
            }
            break;
        }
    }
    Ok(result(values))
}
pub(super) fn preview(
    engine: &PdfEngine,
    info: &DocumentInfo,
    edit: FormEdit,
    parent: &Path,
    password: Option<&str>,
) -> EngineResult<Value> {
    edit.validate()?;
    let index = page_index(info, edit.page)?;
    let fields = engine.list_form_fields(&info.id, index)?;
    let hash = Sha256::digest(engine.source_bytes(&info.id)?.as_ref());
    let mut found = None;
    for field in fields {
        if identity(&hash, edit.page, &field)? == edit.field_id {
            if found.is_some() {
                return Err(invalid("The form field identity is ambiguous."));
            }
            found = Some(field);
        }
    }
    let field = found.ok_or_else(|| {
        invalid("The form selection is stale or unavailable. Inspect this PDF again.")
    })?;
    let metadata = FormMetadata {
        old_value: FieldValue::from_native(&field.value).normalized(),
        value: edit.value.normalized(),
    };
    metadata.validate()?;
    let changed = engine.fill_form_field(
        &info.id,
        FillFormFieldRequest {
            page_index: index,
            annotation_index: field.annotation_index,
            expected_value: field.value,
            value: metadata.value.native(),
        },
    )?;
    text_preview::publish(
        engine,
        info,
        PageContentReplacement {
            document: changed,
            page_indices: (0..info.pages.len()).collect(),
        },
        text_preview::PreviewDescription {
            unchanged_input: None,
            source_fingerprint: hex(&hash),
            page: edit.page,
            selection_id: edit.field_id,
            old_text: String::new(),
            replacement: String::new(),
            substitute_font: None,
            paragraph: None,
            form: Some(metadata),
        },
        parent,
        password,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn value_limits_follow_normalized_utf8_and_strict_nested_schema() {
        for text in ["x".repeat(32768), "é".repeat(16384), "\r\n".repeat(32768)] {
            assert!(FieldValue::Text { text }.validate().is_ok());
        }
        assert!(FieldValue::Text {
            text: "é".repeat(16385)
        }
        .validate()
        .is_err());
        assert!(FieldValue::Choice {
            indices: (0..2000).collect()
        }
        .validate()
        .is_ok());
        for indices in [vec![0, 0], vec![2, 1], vec![2000]] {
            assert!(FieldValue::Choice { indices }.validate().is_err());
        }
        for value in [
            json!({"kind":"text","text":"x","extra":true}),
            json!({"kind":"toggle","checked":null}),
            json!({"kind":"choice","indices":[-1]}),
        ] {
            assert!(serde_json::from_value::<FieldValue>(value).is_err());
        }
    }
    #[test]
    fn form_request_file_limit_is_checked_before_decoding() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("edit.json");
        let edit = FormEdit {
            page: 1,
            field_id: format!("field-v1-{}", "a".repeat(64)),
            value: FieldValue::Text { text: "".into() },
        };
        let mut bytes = serde_json::to_vec(&edit).unwrap();
        bytes.resize(MAX_REQUEST, b' ');
        std::fs::write(&p, &bytes).unwrap();
        assert!(FormEdit::read(&p).is_ok());
        bytes.push(b' ');
        std::fs::write(&p, &bytes).unwrap();
        assert!(FormEdit::read(&p).is_err());
    }
}
