//! Remove a complete logical field without rewriting its neighbors.
use crate::form_properties::{resolve, text, Context};
use crate::{EngineError, EngineResult};
use lopdf::{Dictionary, Document, Object, ObjectId};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormRemovalInfo {
    pub expected_key: String,
    pub full_name: String,
    pub linked_widgets: usize,
    pub page_indices: Vec<usize>,
    pub removal_reason: Option<String>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoveFormFieldRequest {
    pub page_index: usize,
    pub annotation_index: usize,
    pub expected_key: String,
}
fn bad(message: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("Field removal: {message}"))
}
impl RemoveFormFieldRequest {
    pub(crate) fn validate(&self) -> EngineResult<()> {
        if self.expected_key.is_empty() || self.expected_key.len() > 128 {
            return Err(bad("reopen the field removal dialog"));
        }
        Ok(())
    }
}
fn array<'a>(
    d: &'a Document,
    dict: &'a Dictionary,
    key: &[u8],
) -> EngineResult<Option<&'a Vec<Object>>> {
    match dict.get(key).ok().map(|v| resolve(d, v)).transpose()? {
        None | Some(Object::Null) => Ok(None),
        Some(value) => value.as_array().map(Some).map_err(bad),
    }
}
fn without(
    values: &[Object],
    removed: &HashSet<ObjectId>,
    budget: &mut crate::form_graph::CopyBudget,
) -> EngineResult<Vec<Object>> {
    budget.array(values)?;
    Ok(values
        .iter()
        .filter(|v| {
            !v.as_reference()
                .ok()
                .is_some_and(|id| removed.contains(&id))
        })
        .cloned()
        .collect())
}
fn full_name(d: &Document, mut id: ObjectId) -> EngineResult<String> {
    let mut parts = Vec::new();
    let mut bytes = 0;
    for _ in 0..65 {
        let node = d.get_dictionary(id).map_err(bad)?;
        if let Ok(v) = node.get(b"T") {
            if !matches!(resolve(d, v)?, Object::Null) {
                let part = text(d, v, 32768)?;
                bytes += part.len() + 1;
                if bytes > 32768 {
                    return Err(bad("field name exceeds the removal inspection limit"));
                }
                parts.push(part);
            }
        }
        match node.get(b"Parent") {
            Ok(v) => id = v.as_reference().map_err(bad)?,
            Err(_) => {
                parts.reverse();
                return Ok(parts.iter().fold(String::new(), |prefix, part| {
                    crate::form_properties::qualified_name(&prefix, part)
                }));
            }
        }
    }
    Err(bad("field parent hierarchy exceeds the limit"))
}
struct ScanBudget {
    values: usize,
    bytes: usize,
}
impl ScanBudget {
    fn take(&mut self, bytes: usize, depth: usize) -> EngineResult<()> {
        self.values = self
            .values
            .checked_sub(1)
            .ok_or_else(|| bad("reference inspection exceeds 100000 values"))?;
        self.bytes = self
            .bytes
            .checked_sub(bytes)
            .ok_or_else(|| bad("reference inspection exceeds 16 MiB"))?;
        if depth > 64 {
            return Err(bad("reference inspection exceeds 64 levels"));
        }
        Ok(())
    }
}
fn target_names(
    d: &Document,
    v: &Object,
    names: &HashSet<String>,
    b: &mut ScanBudget,
    depth: usize,
) -> EngineResult<()> {
    b.take(64, depth)?;
    match resolve(d, v)? {
        Object::String(raw, _) => {
            b.take(raw.len().saturating_mul(3), depth)?;
            if names.contains(&text(d, v, 32768)?) {
                return Err(bad(
                    "an action targets this field by name; remove or update that action first",
                ));
            }
        }
        Object::Array(values) => {
            for value in values {
                target_names(d, value, names, b, depth + 1)?;
            }
        }
        Object::Dictionary(_) => {} // Object references are checked by the graph walk.
        _ => return Err(bad("unsupported field action target")),
    }
    Ok(())
}
/// Only reachable references matter. Old shared arrays may become unreachable
/// after a private replacement; they are pruned after this proof succeeds.
fn check_references(
    d: &Document,
    removed: &HashSet<ObjectId>,
    names: &HashSet<String>,
) -> EngineResult<()> {
    fn walk(
        d: &Document,
        v: &Object,
        removed: &HashSet<ObjectId>,
        names: &HashSet<String>,
        seen: &mut HashSet<ObjectId>,
        b: &mut ScanBudget,
        depth: usize,
    ) -> EngineResult<()> {
        b.take(64, depth)?;
        match v {
            Object::Reference(id) => {
                if removed.contains(id) {
                    return Err(bad("another action, annotation or document structure still references this field"));
                }
                if seen.insert(*id) {
                    walk(
                        d,
                        d.objects
                            .get(id)
                            .ok_or_else(|| bad("missing referenced object"))?,
                        removed,
                        names,
                        seen,
                        b,
                        depth + 1,
                    )?;
                }
            }
            Object::Dictionary(dict) | Object::Stream(lopdf::Stream { dict, .. }) => {
                let kind = dict
                    .get(b"S")
                    .ok()
                    .map(|v| resolve(d, v))
                    .transpose()?
                    .and_then(|v| v.as_name().ok());
                let key = match kind {
                    Some(b"Hide") => Some(b"T".as_slice()),
                    Some(b"ResetForm" | b"SubmitForm") => Some(b"Fields".as_slice()),
                    _ => None,
                };
                if let Some(v) = key.and_then(|k| dict.get(k).ok()) {
                    target_names(d, v, names, b, depth + 1)?;
                }
                for (key, value) in dict.iter() {
                    b.take(key.len(), depth)?;
                    walk(d, value, removed, names, seen, b, depth + 1)?;
                }
            }
            Object::Array(values) => {
                for value in values {
                    walk(d, value, removed, names, seen, b, depth + 1)?;
                }
            }
            Object::String(raw, _) | Object::Name(raw) => b.take(raw.len(), depth)?,
            _ => {}
        }
        Ok(())
    }
    let mut seen = HashSet::new();
    let mut budget = ScanBudget {
        values: 100000,
        bytes: 16 * 1024 * 1024,
    };
    for (key, value) in d.trailer.iter() {
        budget.take(key.len(), 0)?;
        walk(d, value, removed, names, &mut seen, &mut budget, 0)?;
    }
    Ok(())
}
fn mutate(
    ctx: &mut Context,
    removed_widgets: &mut Vec<(usize, usize)>,
) -> EngineResult<Vec<usize>> {
    // The tree keys are the reference IDs, not necessarily the dictionary IDs.
    // Refuse aliased identities until removal can prove the entire alias closure.
    if ctx
        .ancestors
        .iter()
        .chain(&ctx.widgets)
        .any(|id| matches!(ctx.doc.objects.get(id), Some(Object::Reference(_))))
    {
        return Err(bad(
            "aliased field definitions cannot be removed safely yet",
        ));
    }
    ctx.validate_native_links()?;
    if let Some(reason) = crate::form_properties::removal_action_reason(&ctx.doc)? {
        return Err(bad(reason));
    }
    let mut removed: HashSet<_> = ctx.widgets.iter().copied().collect();
    removed.insert(ctx.owner);
    // Prune only ancestors whose entire child list is being removed.
    for id in ctx.ancestors.iter().skip(1) {
        let kids = array(&ctx.doc, ctx.doc.get_dictionary(*id).map_err(bad)?, b"Kids")?
            .ok_or_else(|| bad("ancestor child list is missing"))?;
        if kids.iter().all(|v| {
            v.as_reference()
                .ok()
                .is_some_and(|id| removed.contains(&id))
        }) {
            removed.insert(*id);
        } else {
            break;
        }
    }
    let mut names = HashSet::new();
    for id in ctx.ancestors.iter().filter(|id| removed.contains(id)) {
        names.insert(full_name(&ctx.doc, *id)?);
    }
    let mut budget = crate::form_graph::CopyBudget::default();
    for id in ctx
        .ancestors
        .iter()
        .skip(1)
        .filter(|id| !removed.contains(id))
    {
        let kids = array(&ctx.doc, ctx.doc.get_dictionary(*id).map_err(bad)?, b"Kids")?
            .ok_or_else(|| bad("ancestor child list is missing"))?;
        let kept = without(kids, &removed, &mut budget)?;
        ctx.doc
            .get_dictionary_mut(*id)
            .map_err(bad)?
            .set("Kids", kept);
        break;
    }
    let pages = ctx.doc.get_pages();
    let mut affected = Vec::new();
    let mut placements = std::collections::HashMap::new();
    for (index, id) in pages.values().enumerate() {
        if let Some(values) = array(
            &ctx.doc,
            ctx.doc.get_dictionary(*id).map_err(bad)?,
            b"Annots",
        )? {
            let mut changed = false;
            for (slot, value) in values.iter().enumerate() {
                if let Some(id) = value.as_reference().ok().filter(|id| removed.contains(id)) {
                    budget.reserve_bytes(std::mem::size_of::<(usize, usize)>())?;
                    removed_widgets.push((index, slot));
                    *placements.entry(id).or_insert(0usize) += 1;
                    changed = true;
                }
            }
            if changed {
                let kept = without(values, &removed, &mut budget)?;
                ctx.doc
                    .get_dictionary_mut(*id)
                    .map_err(bad)?
                    .set("Annots", kept);
                affected.push(index);
            }
        }
    }
    if ctx.widgets.iter().any(|id| placements.get(id) != Some(&1)) {
        return Err(bad(
            "each linked widget must have exactly one page placement",
        ));
    }
    let form = resolve(
        &ctx.doc,
        ctx.doc
            .catalog()
            .map_err(bad)?
            .get(b"AcroForm")
            .map_err(bad)?,
    )?
    .as_dict()
    .map_err(bad)?;
    budget.dictionary(form)?;
    let mut form = form.clone();
    for key in [b"Fields".as_slice(), b"CO".as_slice()] {
        if let Some(values) = array(&ctx.doc, &form, key)? {
            let kept = without(values, &removed, &mut budget)?;
            form.set(key.to_vec(), kept);
        }
    }
    let catalog = ctx
        .doc
        .trailer
        .get(b"Root")
        .map_err(bad)?
        .as_reference()
        .map_err(bad)?;
    ctx.doc
        .get_dictionary_mut(catalog)
        .map_err(bad)?
        .set("AcroForm", form);
    check_references(&ctx.doc, &removed, &names)?;
    // Retain object identity for every reachable unaffected object.
    for id in removed {
        ctx.doc.objects.remove(&id);
    }
    ctx.doc.prune_objects();
    Ok(affected)
}
fn prepare(
    bytes: &[u8],
    page: usize,
    index: usize,
) -> EngineResult<(Document, FormRemovalInfo, Vec<(usize, usize)>)> {
    let mut ctx = crate::form_properties::load(bytes, page, index)?;
    let mut info = FormRemovalInfo {
        expected_key: ctx.properties.expected_key.clone(),
        full_name: ctx.properties.full_name.clone(),
        linked_widgets: ctx.widgets.len(),
        page_indices: vec![],
        removal_reason: None,
    };
    let mut removed_widgets = Vec::new();
    match mutate(&mut ctx, &mut removed_widgets) {
        Ok(pages) => info.page_indices = pages,
        Err(e) => info.removal_reason = Some(e.to_string()),
    };
    Ok((ctx.doc, info, removed_widgets))
}
pub(crate) fn inspect(bytes: &[u8], page: usize, index: usize) -> EngineResult<FormRemovalInfo> {
    Ok(prepare(bytes, page, index)?.1)
}
pub(crate) struct PublishedRemoval {
    pub(crate) bytes: Vec<u8>,
    /// Original page/annotation slots removed by this private transaction.
    pub(crate) removed_widgets: Vec<(usize, usize)>,
}
pub(crate) fn change(
    bytes: &[u8],
    request: &RemoveFormFieldRequest,
) -> EngineResult<PublishedRemoval> {
    request.validate()?;
    let (mut doc, info, removed_widgets) =
        prepare(bytes, request.page_index, request.annotation_index)?;
    if info.expected_key != request.expected_key {
        return Err(bad("the field changed; reopen removal"));
    }
    if let Some(reason) = info.removal_reason {
        return Err(bad(reason));
    }
    let output = crate::pdf_output::save(&mut doc)?;
    crate::acroforms::authoring_document(&output)?;
    Ok(PublishedRemoval {
        bytes: output,
        removed_widgets,
    })
}
