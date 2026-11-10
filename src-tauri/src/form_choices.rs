//! Read-only, source-bound choice inspection. Publication requires separate proof.
use crate::{
    form_choice_model::{source_valid, MAX_OPTIONS, MAX_PAYLOAD, MAX_STRING},
    form_properties::{self, Context},
    ChoiceModel, ChoiceOption, ChoiceSelection, EngineError, EngineResult,
};
use lopdf::{Document, Object};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChoiceWidgetLocation {
    pub page_index: usize,
    pub annotation_index: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FormChoices {
    pub expected_key: String,
    pub name: String,
    pub widgets: Vec<ChoiceWidgetLocation>,
    pub model: ChoiceModel,
}
fn bad(message: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("Choice inspection: {message}"))
}
fn resolve<'a>(doc: &'a Document, value: &'a Object) -> EngineResult<&'a Object> {
    Ok(doc.dereference(value).map_err(bad)?.1)
}
fn attr<'a>(ctx: &'a Context, key: &[u8]) -> EngineResult<Option<&'a Object>> {
    Ok(ctx
        .raw_attribute(key)?
        .filter(|v| !matches!(v, Object::Null)))
}
struct TextBudget {
    raw: usize,
    decoded: usize,
}
impl TextBudget {
    fn new() -> Self {
        Self {
            raw: 512 * 1024,
            decoded: MAX_PAYLOAD,
        }
    }
    fn decoded(&mut self, size: usize) -> EngineResult<()> {
        if size > MAX_STRING {
            return Err(bad("a choice string exceeds4096UTF-8bytes"));
        }
        self.decoded = self
            .decoded
            .checked_sub(size)
            .ok_or_else(|| bad("choice text exceeds128KiB"))?;
        Ok(())
    }
    fn text(&mut self, doc: &Document, value: &Object) -> EngineResult<String> {
        let value = resolve(doc, value)?;
        let raw = value.as_str().map_err(bad)?;
        if raw.len() > 8194 {
            return Err(bad("encoded choice text exceeds the per-string limit"));
        }
        self.raw = self
            .raw
            .checked_sub(raw.len())
            .ok_or_else(|| bad("encoded choice text exceeds512KiB"))?;
        if let Some(raw) = raw.strip_prefix(b"\xef\xbb\xbf") {
            let text = std::str::from_utf8(raw).map_err(bad)?;
            self.decoded(text.len())?;
            return Ok(text.to_owned());
        }
        if let Some(raw) = raw.strip_prefix(b"\xfe\xff") {
            if raw.len() % 2 != 0 {
                return Err(bad("incomplete UTF-16 choice text"));
            }
            let chars = || {
                char::decode_utf16(
                    raw.chunks_exact(2)
                        .map(|b| u16::from_be_bytes([b[0], b[1]])),
                )
            };
            let length =
                chars().try_fold(0usize, |n, c| c.map(|c| n + c.len_utf8()).map_err(bad))?;
            self.decoded(length)?;
            return chars().collect::<Result<String, _>>().map_err(bad);
        }
        // PDFDocEncoding consumes at least one UTF-8 byte per source byte. Bound
        // its temporary decode before allocation, then charge retained text.
        if raw.len() > MAX_STRING {
            return Err(bad("choice string exceeds the decoding limit"));
        }
        let text = lopdf::decode_text_string(value).map_err(bad)?;
        self.decoded(text.len())?;
        Ok(text)
    }
    fn values(&mut self, ctx: &Context, key: &[u8]) -> EngineResult<Option<Vec<String>>> {
        match ctx.raw_attribute(key)? {
            None => Ok(None),
            Some(Object::Null) if key == b"V" => Ok(Some(vec![])),
            Some(Object::Null) => Ok(None),
            Some(value @ Object::String(..)) => Ok(Some(vec![self.text(&ctx.doc, value)?])),
            Some(Object::Array(values)) if values.len() <= MAX_OPTIONS => values
                .iter()
                .map(|v| self.text(&ctx.doc, v))
                .collect::<EngineResult<Vec<_>>>()
                .map(Some),
            _ => Err(bad("invalid or excessive choice value array")),
        }
    }
}
fn options(ctx: &Context, budget: &mut TextBudget) -> EngineResult<Vec<ChoiceOption>> {
    let Some(value) = attr(ctx, b"Opt")? else {
        return Ok(vec![]);
    };
    let values = value.as_array().map_err(bad)?;
    if values.len() > MAX_OPTIONS {
        return Err(bad("too many choice options"));
    }
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        let value = resolve(&ctx.doc, value)?;
        let option = match value {
            Object::String(..) => {
                let export_value = budget.text(&ctx.doc, value)?;
                budget.decoded(export_value.len())?;
                ChoiceOption {
                    display: export_value.clone(),
                    export_value,
                }
            }
            Object::Array(pair) if pair.len() == 2 => ChoiceOption {
                export_value: budget.text(&ctx.doc, &pair[0])?,
                display: budget.text(&ctx.doc, &pair[1])?,
            },
            _ => {
                return Err(bad(
                    "each option must be a string or an export/display pair",
                ))
            }
        };
        result.push(option);
    }
    Ok(result)
}
fn indices(ctx: &Context, count: usize) -> EngineResult<Option<Vec<usize>>> {
    let Some(value) = attr(ctx, b"I")? else {
        return Ok(None);
    };
    let values = match value {
        Object::Array(values) if values.len() <= MAX_OPTIONS => values.as_slice(),
        Object::Integer(_) => std::slice::from_ref(value),
        _ => return Err(bad("invalid or excessive selected indices")),
    };
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        let i = usize::try_from(resolve(&ctx.doc, value)?.as_i64().map_err(bad)?).map_err(bad)?;
        if i >= count || result.last().is_some_and(|last| *last >= i) {
            return Err(bad(
                "selected indices must be unique, ascending and in range",
            ));
        }
        result.push(i);
    }
    Ok(Some(result))
}
fn from_values(
    options: &[ChoiceOption],
    values: &[String],
    editable: bool,
) -> EngineResult<ChoiceSelection> {
    if values.is_empty() {
        return Ok(ChoiceSelection::None);
    }
    let mut rows = Vec::with_capacity(values.len());
    for value in values {
        let mut matches = options
            .iter()
            .enumerate()
            .filter(|(_, o)| o.export_value == *value);
        match (matches.next(), matches.next()) {
            (Some((index, _)), None) => rows.push(index),
            (None, None) if values.len() == 1 && editable => {
                return Ok(ChoiceSelection::Text(value.clone()))
            }
            (None, None) if values.len() == 1 && value.is_empty() => {
                return Ok(ChoiceSelection::None)
            }
            _ => return Err(bad("saved answer has no unambiguous option identity")),
        }
    }
    rows.sort_unstable();
    Ok(ChoiceSelection::Rows(rows))
}
fn selection(
    options: &[ChoiceOption],
    values: Option<&[String]>,
    indices: Option<Vec<usize>>,
    editable: bool,
) -> EngineResult<ChoiceSelection> {
    if let Some(indices) = indices {
        if let Some(values) = values {
            let mut exports: Vec<_> = indices
                .iter()
                .map(|i| options[*i].export_value.as_str())
                .collect();
            let mut saved: Vec<_> = values.iter().map(String::as_str).collect();
            exports.sort_unstable();
            saved.sort_unstable();
            if exports != saved {
                if indices.is_empty() {
                    let parsed = from_values(options, values, editable)?;
                    if !matches!(parsed, ChoiceSelection::Rows(_)) {
                        return Ok(parsed);
                    }
                }
                return Err(bad("saved answer and selected indices disagree"));
            }
        }
        return Ok(if indices.is_empty() {
            ChoiceSelection::None
        } else {
            ChoiceSelection::Rows(indices)
        });
    }
    from_values(options, values.unwrap_or(&[]), editable)
}
fn locations(ctx: &Context) -> EngineResult<Vec<ChoiceWidgetLocation>> {
    let expected: HashSet<_> = ctx.widgets.iter().copied().collect();
    let mut found = HashSet::new();
    let mut result = Vec::new();
    for (page_index, id) in ctx.doc.get_pages().values().enumerate() {
        let page = ctx.doc.get_dictionary(*id).map_err(bad)?;
        let Ok(annotations) = page.get(b"Annots") else {
            continue;
        };
        let annotations = resolve(&ctx.doc, annotations)?;
        if matches!(annotations, Object::Null) {
            continue;
        }
        for (annotation_index, value) in annotations.as_array().map_err(bad)?.iter().enumerate() {
            if let Ok(id) = value.as_reference() {
                if expected.contains(&id) {
                    if result.len() >= 4000 {
                        return Err(bad("too many linked choice appearances"));
                    }
                    found.insert(id);
                    result.push(ChoiceWidgetLocation {
                        page_index,
                        annotation_index,
                    });
                }
            }
        }
    }
    if found != expected || result.is_empty() {
        return Err(bad("a linked field widget has no page location"));
    }
    Ok(result)
}
/// Read only: this does not authorize an edit or prove appearance compatibility.
pub fn inspect_choice_pdf(bytes: &[u8], page: usize, index: usize) -> EngineResult<FormChoices> {
    let ctx = form_properties::load(bytes, page, index)?;
    inspect_context(&ctx)
}
pub(crate) fn inspect_context(ctx: &Context) -> EngineResult<FormChoices> {
    ctx.validate_native_links()?;
    if attr(&ctx, b"FT")?.and_then(|v| v.as_name().ok()) != Some(b"Ch") {
        return Err(bad("selected field is not a choice field"));
    }
    let flags = attr(&ctx, b"Ff")?
        .map(|v| v.as_i64().map_err(bad))
        .transpose()?
        .unwrap_or(0);
    if !(0..=u32::MAX as i64).contains(&flags) {
        return Err(bad("invalid choice flags"));
    }
    let combo = flags & (1 << 17) != 0;
    let editable = combo && flags & (1 << 18) != 0;
    let multiple = !combo && flags & (1 << 21) != 0;
    let mut budget = TextBudget::new();
    let options = options(&ctx, &mut budget)?;
    let values = budget.values(&ctx, b"V")?;
    let indices = indices(&ctx, options.len())?;
    let selection = selection(&options, values.as_deref(), indices, editable)?;
    let mut defaults = budget.values(&ctx, b"DV")?;
    if !editable
        && defaults
            .as_ref()
            .is_some_and(|v| v.len() == 1 && v[0].is_empty())
        && !options.iter().any(|o| o.export_value.is_empty())
    {
        defaults = None;
    }
    // List-only settings can remain after a producer converts a field to a
    // dropdown. Match native semantics without rewriting those opaque entries.
    let top_index = if combo {
        0
    } else {
        attr(&ctx, b"TI")?
            .map(|v| usize::try_from(v.as_i64().map_err(bad)?).map_err(bad))
            .transpose()?
            .unwrap_or(0)
    };
    let model = ChoiceModel {
        options,
        selection,
        defaults,
        combo,
        editable,
        multiple,
        sorted: flags & (1 << 19) != 0,
        top_index,
    };
    source_valid(&model)?;
    let widgets = locations(&ctx)?;
    Ok(FormChoices {
        expected_key: format!("choice-{}", ctx.properties.expected_key),
        name: ctx.properties.full_name.clone(),
        widgets,
        model,
    })
}
