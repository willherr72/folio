//! Private choice graph mutations. A prepared graph is not a publishable PDF.
use crate::{
    form_choices::inspect_context,
    form_properties::{self, Context},
    resolve_choice_edit, ChoiceEdit, ChoiceEditSelection, ChoiceModel, ChoiceOption,
    ChoiceSelection, EngineError, EngineResult, FormChoices, ResolvedChoiceEdit,
};
use lopdf::Object;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetFormChoicesRequest {
    pub page_index: usize,
    pub annotation_index: usize,
    pub expected_key: String,
    pub edit: ChoiceEdit,
}
/// Deliberately has no public byte serialization or source-registration method.
pub struct PreparedChoiceEdit {
    pub(crate) ctx: Context,
    original: FormChoices,
    resolved: ResolvedChoiceEdit,
    changed: bool,
    repaint: bool,
}
impl PreparedChoiceEdit {
    pub fn original(&self) -> &FormChoices {
        &self.original
    }
    pub fn resolved(&self) -> &ResolvedChoiceEdit {
        &self.resolved
    }
    pub fn has_changes(&self) -> bool {
        self.changed
    }
    /// False does not waive font coverage or source-appearance validation.
    pub fn needs_appearance_update(&self) -> bool {
        self.repaint
    }
    pub fn page_count(&self) -> usize {
        self.ctx.doc.get_pages().len()
    }
}
fn bad(message: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("Choice edit: {message}"))
}
fn indices(selection: &ChoiceSelection) -> &[usize] {
    match selection {
        ChoiceSelection::Rows(rows) => rows,
        _ => &[],
    }
}
fn same_answer(old: &ChoiceModel, new: &ResolvedChoiceEdit) -> bool {
    match (&old.selection, &new.selection) {
        (ChoiceSelection::None, ChoiceSelection::None) => true,
        (ChoiceSelection::Text(a), ChoiceSelection::Text(b)) => a == b,
        (ChoiceSelection::Rows(a), ChoiceSelection::Rows(b)) => {
            let mut a: Vec<_> = a
                .iter()
                .map(|i| old.options[*i].export_value.as_str())
                .collect();
            let mut b: Vec<_> = b
                .iter()
                .map(|i| new.options[*i].export_value.as_str())
                .collect();
            a.sort_unstable();
            b.sort_unstable();
            a == b
        }
        _ => false,
    }
}
fn blank_text(value: Option<&Object>) -> bool {
    match value {
        Some(Object::String(raw, _)) => {
            matches!(raw.as_slice(), [] | [0xfe, 0xff] | [0xef, 0xbb, 0xbf])
        }
        _ => false,
    }
}
fn values_object(values: impl IntoIterator<Item = impl AsRef<str>>) -> Object {
    let mut values: Vec<_> = values
        .into_iter()
        .map(|v| lopdf::text_string(v.as_ref()))
        .collect();
    match values.len() {
        0 => Object::Null,
        1 => values.remove(0),
        _ => Object::Array(values),
    }
}
fn answer_object(resolved: &ResolvedChoiceEdit) -> Object {
    match &resolved.selection {
        ChoiceSelection::None => Object::Null,
        ChoiceSelection::Text(text) => lopdf::text_string(text),
        ChoiceSelection::Rows(rows) => values_object(
            rows.iter()
                .map(|i| resolved.options[*i].export_value.as_str()),
        ),
    }
}
fn set(ctx: &mut Context, key: &[u8], value: Object) -> EngineResult<bool> {
    let owner = ctx.doc.get_dictionary_mut(ctx.owner).map_err(bad)?;
    if owner.get(key).ok() == Some(&value) {
        return Ok(false);
    }
    owner.set(key.to_vec(), value);
    Ok(true)
}
fn option_array(ctx: &Context, edit: &ChoiceEdit, original: &ChoiceModel) -> EngineResult<Object> {
    let operands = match ctx.raw_attribute(b"Opt")? {
        Some(Object::Array(values)) => values.as_slice(),
        None | Some(Object::Null) => &[],
        _ => return Err(bad("source option operands changed")),
    };
    let mut budget = crate::form_graph::CopyBudget::default();
    let mut output = Vec::with_capacity(edit.options.len());
    for row in &edit.options {
        if let Some(index) = row.source_index {
            let old = &original.options[index];
            if old.export_value == row.export_value && old.display == row.display {
                let operand = operands
                    .get(index)
                    .ok_or_else(|| bad("source option operand is missing"))?;
                budget.array(std::slice::from_ref(operand))?;
                output.push(operand.clone());
                continue;
            }
        }
        let value = if row.export_value == row.display {
            lopdf::text_string(&row.export_value)
        } else {
            Object::Array(vec![
                lopdf::text_string(&row.export_value),
                lopdf::text_string(&row.display),
            ])
        };
        budget.array(std::slice::from_ref(&value))?;
        output.push(value);
    }
    Ok(Object::Array(output))
}
fn displayed<'a>(options: &'a [ChoiceOption], selection: &'a ChoiceSelection) -> &'a str {
    match selection {
        ChoiceSelection::None => "",
        ChoiceSelection::Text(text) => text,
        ChoiceSelection::Rows(rows) => rows
            .first()
            .map(|i| options[*i].display.as_str())
            .unwrap_or(""),
    }
}
fn repaint(old: &ChoiceModel, new: &ResolvedChoiceEdit) -> bool {
    if old.combo {
        displayed(&old.options, &old.selection) != displayed(&new.options, &new.selection)
    } else {
        old.options
            .iter()
            .map(|o| &o.display)
            .ne(new.options.iter().map(|o| &o.display))
            || indices(&old.selection) != indices(&new.selection)
            || old.top_index != new.top_index
    }
}

pub fn prepare_choice_edit_pdf(
    bytes: &[u8],
    request: &SetFormChoicesRequest,
) -> EngineResult<PreparedChoiceEdit> {
    if request.expected_key.len() > 128 {
        return Err(bad("invalid source key"));
    }
    let ctx = form_properties::load(bytes, request.page_index, request.annotation_index)?;
    prepare_choice_edit_context(ctx, request)
}

pub(crate) fn prepare_choice_edit_context(
    mut ctx: Context,
    request: &SetFormChoicesRequest,
) -> EngineResult<PreparedChoiceEdit> {
    if request.expected_key.len() > 128 {
        return Err(bad("invalid source key"));
    }
    let original = inspect_context(&ctx)?;
    if original.expected_key != request.expected_key {
        return Err(bad("the field changed; reopen choices"));
    }
    let resolved = resolve_choice_edit(&original.model, &request.edit)?;
    let old = &original.model;
    let absent_combo_value = old.combo && ctx.raw_attribute(b"V")?.is_none();
    // PDFium's combo value lookup falls back to DV only when V is absent. Changing
    // that default must not silently change the displayed current answer. Null V
    // is an explicit empty answer and does not have this ambiguity.
    if absent_combo_value
        && matches!(request.edit.selection, ChoiceEditSelection::Keep)
        && old.defaults != resolved.defaults
    {
        return Err(bad("this dropdown displays its default as the current value; choose or clear the current answer before changing its default"));
    }
    let mut changed = false;
    let options_changed = old.options != resolved.options;
    let has_blank = resolved.options.iter().any(|o| o.export_value.is_empty());
    // A raw empty string used to mean no option. Do not let a newly added blank
    // option retroactively adopt that answer/default.
    let mask_blank_answer = matches!(old.selection, ChoiceSelection::None)
        && matches!(resolved.selection, ChoiceSelection::None)
        && has_blank
        && blank_text(ctx.raw_attribute(b"V")?);
    let mask_blank_default = old.defaults.is_none()
        && resolved.defaults.is_none()
        && has_blank
        && blank_text(ctx.raw_attribute(b"DV")?);
    let explicit_answer = !matches!(request.edit.selection, ChoiceEditSelection::Keep);
    let write_value = explicit_answer || !same_answer(old, &resolved) || mask_blank_answer;
    let missing_indices = matches!(ctx.raw_attribute(b"I")?, None | Some(Object::Null));
    let new_ambiguity = missing_indices
        && indices(&resolved.selection).iter().any(|i| {
            resolved
                .options
                .iter()
                .filter(|o| o.export_value == resolved.options[*i].export_value)
                .take(2)
                .count()
                > 1
        });
    let write_indices = explicit_answer
        || write_value
        || new_ambiguity
        || indices(&old.selection) != indices(&resolved.selection);
    if options_changed {
        let options = option_array(&ctx, &request.edit, old)?;
        changed |= set(&mut ctx, b"Opt", options)?;
    }
    if write_value {
        changed |= set(&mut ctx, b"V", answer_object(&resolved))?;
    }
    if write_indices {
        changed |= set(
            &mut ctx,
            b"I",
            Object::Array(
                indices(&resolved.selection)
                    .iter()
                    .map(|i| Object::Integer(*i as i64))
                    .collect(),
            ),
        )?;
    }
    if resolved.defaults_changed || mask_blank_default {
        let value = match &resolved.defaults {
            None => Object::Null,
            Some(values) => values_object(values),
        };
        changed |= set(&mut ctx, b"DV", value)?;
    }
    if !old.combo && old.top_index != resolved.top_index {
        changed |= set(&mut ctx, b"TI", Object::Integer(resolved.top_index as i64))?;
    }
    if old.sorted != resolved.sorted {
        let flags = ctx
            .raw_attribute(b"Ff")?
            .filter(|v| !matches!(v, Object::Null))
            .map(|v| v.as_i64().map_err(bad))
            .transpose()?
            .unwrap_or(0);
        changed |= set(&mut ctx, b"Ff", Object::Integer(flags & !(1 << 19)))?;
    }
    let confirmed = inspect_context(&ctx)?;
    let expected = ChoiceModel {
        options: resolved.options.clone(),
        selection: resolved.selection.clone(),
        defaults: resolved.defaults.clone(),
        combo: old.combo,
        editable: old.editable,
        multiple: old.multiple,
        sorted: resolved.sorted,
        top_index: resolved.top_index,
    };
    if confirmed.model != expected
        || confirmed.widgets != original.widgets
        || confirmed.name != original.name
    {
        return Err(bad("private field settings differ from the resolved edit"));
    }
    let repaint = repaint(old, &resolved)
        || (absent_combo_value
            && explicit_answer
            && old
                .defaults
                .as_ref()
                .is_some_and(|v| v.iter().any(|s| !s.is_empty())));
    Ok(PreparedChoiceEdit {
        ctx,
        original,
        resolved,
        changed,
        repaint,
    })
}
