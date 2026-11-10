//! Bounded value semantics for choice editing. PDF ownership and painting are separate.
use crate::{EngineError, EngineResult};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub(crate) const MAX_OPTIONS: usize = 2_000;
pub(crate) const MAX_STRING: usize = 4_096;
pub(crate) const MAX_PAYLOAD: usize = 128 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChoiceOption {
    pub export_value: String,
    pub display: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChoiceDraftOption {
    pub source_index: Option<usize>,
    pub export_value: String,
    pub display: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum ChoiceSelection {
    None,
    Rows(Vec<usize>),
    Text(String),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum ChoiceEditSelection {
    Keep,
    Clear,
    Rows(Vec<usize>),
    Text(String),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChoiceModel {
    pub options: Vec<ChoiceOption>,
    pub selection: ChoiceSelection,
    pub defaults: Option<Vec<String>>,
    pub combo: bool,
    pub editable: bool,
    pub multiple: bool,
    pub sorted: bool,
    pub top_index: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChoiceEdit {
    pub options: Vec<ChoiceDraftOption>,
    pub selection: ChoiceEditSelection,
    pub defaults: ChoiceEditSelection,
    pub manual_order: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedChoiceEdit {
    pub options: Vec<ChoiceOption>,
    pub selection: ChoiceSelection,
    pub defaults: Option<Vec<String>>,
    pub defaults_changed: bool,
    pub top_index: usize,
    pub sorted: bool,
}

fn bad(message: &str) -> EngineError {
    EngineError::InvalidRequest(format!("Choice editing: {message}"))
}
fn payload<'a>(strings: impl Iterator<Item = &'a str>) -> EngineResult<()> {
    let mut remaining = MAX_PAYLOAD;
    for text in strings {
        if text.len() > MAX_STRING || text.len() > remaining {
            return Err(bad("option and value text exceeds the editing limit"));
        }
        remaining -= text.len();
    }
    Ok(())
}
fn rows(indices: &[usize], count: usize, multiple: bool) -> EngineResult<Vec<usize>> {
    if indices.len() > MAX_OPTIONS || (!multiple && indices.len() > 1) {
        return Err(bad("too many selected rows for this field"));
    }
    // Cardinality is bounded before making this working copy.
    let mut result = indices.to_vec();
    result.sort_unstable();
    if result.iter().any(|i| *i >= count) || result.windows(2).any(|w| w[0] == w[1]) {
        return Err(bad("selected row indices are invalid or repeated"));
    }
    Ok(result)
}
pub(crate) fn source_valid(source: &ChoiceModel) -> EngineResult<()> {
    if source.options.len() > MAX_OPTIONS
        || source
            .defaults
            .as_ref()
            .is_some_and(|v| v.len() > MAX_OPTIONS)
        || (source.editable && !source.combo)
        || (source.combo && source.multiple)
        || (source.combo && source.top_index != 0)
        || (source.options.is_empty() && source.top_index != 0)
        || (!source.options.is_empty() && source.top_index >= source.options.len())
    {
        return Err(bad("invalid source choice structure"));
    }
    let text = match &source.selection {
        ChoiceSelection::Text(s) => Some(s.as_str()),
        _ => None,
    };
    payload(
        source
            .options
            .iter()
            .flat_map(|o| [o.export_value.as_str(), o.display.as_str()])
            .chain(text)
            .chain(source.defaults.iter().flatten().map(String::as_str)),
    )?;
    match &source.selection {
        ChoiceSelection::None => {}
        ChoiceSelection::Rows(selected) => {
            if rows(selected, source.options.len(), source.multiple)? != *selected {
                return Err(bad("source selected indices are not in ascending order"));
            }
        }
        ChoiceSelection::Text(text) => {
            if !source.editable || source.options.iter().any(|o| o.export_value == *text) {
                return Err(bad("source free text is unavailable or ambiguous"));
            }
        }
    }
    if let Some(defaults) = &source.defaults {
        if (!source.multiple && defaults.len() > 1)
            || (!source.editable
                && defaults
                    .iter()
                    .any(|v| !source.options.iter().any(|o| o.export_value == *v)))
        {
            return Err(bad("source default values do not match the field"));
        }
    }
    Ok(())
}
enum SelectionRef<'a> {
    None,
    Rows(Vec<usize>),
    Text(&'a str),
}
impl<'a> SelectionRef<'a> {
    fn text(&self) -> Option<&'a str> {
        if let Self::Text(text) = self {
            Some(text)
        } else {
            None
        }
    }
    fn owned(self) -> ChoiceSelection {
        match self {
            Self::None => ChoiceSelection::None,
            Self::Rows(v) if v.is_empty() => ChoiceSelection::None,
            Self::Rows(v) => ChoiceSelection::Rows(v),
            Self::Text(v) => ChoiceSelection::Text(v.into()),
        }
    }
}
fn final_text<'a>(source: &ChoiceModel, edit: &ChoiceEdit, text: &'a str) -> EngineResult<&'a str> {
    if !source.editable {
        return Err(bad("free text is available only in an editable dropdown"));
    }
    if edit.options.iter().any(|o| o.export_value == text) {
        return Err(bad(
            "this value matches an option; select its row explicitly",
        ));
    }
    Ok(text)
}
fn action_text(value: &ChoiceEditSelection) -> Option<&str> {
    match value {
        ChoiceEditSelection::Text(text) => Some(text.as_str()),
        _ => None,
    }
}

/// Resolve only value semantics. Callers must still validate source identity,
/// permissions, raw PDF objects, native interpretation and every widget appearance.
pub fn resolve_choice_edit(
    source: &ChoiceModel,
    edit: &ChoiceEdit,
) -> EngineResult<ResolvedChoiceEdit> {
    source_valid(source)?;
    if edit.options.len() > MAX_OPTIONS {
        return Err(bad("choice editing supports at most 2000 options"));
    }
    payload(
        edit.options
            .iter()
            .flat_map(|o| [o.export_value.as_str(), o.display.as_str()])
            .chain(action_text(&edit.selection))
            .chain(action_text(&edit.defaults)),
    )?;
    let mut positions = vec![None; source.options.len()];
    for (index, option) in edit.options.iter().enumerate() {
        if let Some(original) = option.source_index {
            let slot = positions
                .get_mut(original)
                .ok_or_else(|| bad("option refers to a missing original row"))?;
            if slot.replace(index).is_some() {
                return Err(bad("an original row appears more than once"));
            }
        }
    }
    let options_changed = edit.options.len() != source.options.len()
        || edit
            .options
            .iter()
            .zip(&source.options)
            .enumerate()
            .any(|(i, (a, b))| {
                a.source_index != Some(i)
                    || a.export_value != b.export_value
                    || a.display != b.display
            });
    if source.sorted && options_changed && !edit.manual_order {
        return Err(bad("enable manual order before changing sorted options"));
    }
    let selection = match &edit.selection {
        ChoiceEditSelection::Keep => match &source.selection {
            ChoiceSelection::None => SelectionRef::None,
            ChoiceSelection::Text(text) => SelectionRef::Text(final_text(source, edit, text)?),
            ChoiceSelection::Rows(old) => {
                let remapped = old
                    .iter()
                    .map(|i| {
                        positions[*i].ok_or_else(|| {
                            bad("a selected row was removed; choose or clear the answer")
                        })
                    })
                    .collect::<EngineResult<Vec<_>>>()?;
                SelectionRef::Rows(rows(&remapped, edit.options.len(), source.multiple)?)
            }
        },
        ChoiceEditSelection::Clear => SelectionRef::None,
        ChoiceEditSelection::Rows(v) => {
            SelectionRef::Rows(rows(v, edit.options.len(), source.multiple)?)
        }
        ChoiceEditSelection::Text(v) => SelectionRef::Text(final_text(source, edit, v)?),
    };
    let defaults = match &edit.defaults {
        ChoiceEditSelection::Keep => {
            if let Some(values) = &source.defaults {
                for value in values {
                    let mut old_matches = source
                        .options
                        .iter()
                        .enumerate()
                        .filter(|(_, o)| o.export_value == *value)
                        .map(|(i, _)| i)
                        .peekable();
                    if old_matches.peek().is_none() {
                        final_text(source, edit, value)?;
                    } else if !old_matches.any(|i| {
                        positions[i].is_some_and(|p| edit.options[p].export_value == *value)
                    }) {
                        return Err(bad(
                            "a default option changed or was removed; choose or clear the default",
                        ));
                    }
                }
                Some(values.iter().map(String::as_str).collect::<Vec<_>>())
            } else {
                None
            }
        }
        ChoiceEditSelection::Clear => None,
        ChoiceEditSelection::Rows(v) => {
            let indices = rows(v, edit.options.len(), source.multiple)?;
            if indices.is_empty() {
                None
            } else {
                let mut seen = HashSet::new();
                Some(
                    indices
                        .iter()
                        .map(|i| edit.options[*i].export_value.as_str())
                        .filter(|s| seen.insert(*s))
                        .collect(),
                )
            }
        }
        ChoiceEditSelection::Text(v) => Some(vec![final_text(source, edit, v)?]),
    };
    // Output can combine large source defaults with a large new option list.
    // Charge that combination before cloning any output text.
    payload(
        edit.options
            .iter()
            .flat_map(|o| [o.export_value.as_str(), o.display.as_str()])
            .chain(selection.text())
            .chain(defaults.iter().flatten().copied()),
    )?;
    let top_index = if source.combo || source.options.is_empty() {
        0
    } else {
        positions[source.top_index]
            .or_else(|| positions.iter().skip(source.top_index + 1).find_map(|v| *v))
            .or_else(|| positions[..source.top_index].iter().rev().find_map(|v| *v))
            .unwrap_or(0)
    };
    Ok(ResolvedChoiceEdit {
        options: edit
            .options
            .iter()
            .map(|o| ChoiceOption {
                export_value: o.export_value.clone(),
                display: o.display.clone(),
            })
            .collect(),
        selection: selection.owned(),
        defaults: defaults.map(|v| v.into_iter().map(str::to_owned).collect()),
        defaults_changed: !matches!(edit.defaults, ChoiceEditSelection::Keep),
        top_index,
        sorted: source.sorted && !edit.manual_order,
    })
}
