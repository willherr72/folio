//! Ordinary form filling publishes into its privately owned original graph.
use crate::{
    form_answer_appearance::{
        transplant_answer_with_resources, AnswerKind, FillAppearanceBudget,
        NormalAppearanceAdmission, StagedAnswerAppearance,
    },
    form_appearance::Copier,
    form_fill::NativeForm,
    form_fill_identity::{BoundFill, FillIdentity},
    EngineError, EngineResult, FillFormFieldRequest,
};
use lopdf::{Dictionary, Document, Object, ObjectId};
use pdfium_render::prelude::{Pdfium, PdfiumLibraryBindings};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    io::{self, Write},
};
#[path = "form_fill_context.rs"]
mod context;
const LIMIT: usize = 64 * 1024 * 1024;
#[cfg(test)]
#[path = "form_fill_publish_tests.rs"]
mod tests;
fn bad(reason: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("Form answer preservation: {reason}"))
}
fn resolve<'a>(doc: &'a Document, mut value: &'a Object) -> EngineResult<&'a Object> {
    let mut seen = HashSet::new();
    while let Object::Reference(id) = value {
        if seen.len() >= 64 || !seen.insert(*id) {
            return Err(bad("cyclic appearance reference"));
        }
        value = doc
            .objects
            .get(id)
            .ok_or_else(|| bad("missing appearance reference"))?;
    }
    Ok(value)
}
fn dict<'a>(doc: &'a Document, value: &'a Object) -> EngineResult<&'a Dictionary> {
    resolve(doc, value)?.as_dict().map_err(bad)
}
fn ap(doc: &Document, id: ObjectId) -> EngineResult<Option<&Dictionary>> {
    let w = doc.get_dictionary(id).map_err(bad)?;
    match w.get(b"AP").ok().map(|v| resolve(doc, v)).transpose()? {
        None | Some(Object::Null) => Ok(None),
        Some(v) => Ok(Some(v.as_dict().map_err(bad)?)),
    }
}
fn normal(doc: &Document, id: ObjectId) -> EngineResult<Option<&Object>> {
    match ap(doc, id)?.and_then(|d| d.get(b"N").ok()) {
        Some(v) if !matches!(resolve(doc, v)?, Object::Null) => Ok(Some(v)),
        _ => Ok(None),
    }
}
fn global_regeneration(doc: &Document) -> EngineResult<bool> {
    let form = dict(
        doc,
        doc.catalog().map_err(bad)?.get(b"AcroForm").map_err(bad)?,
    )?;
    match form
        .get(b"NeedAppearances")
        .ok()
        .map(|v| resolve(doc, v))
        .transpose()?
    {
        Some(Object::Boolean(v)) => Ok(*v),
        None | Some(Object::Null) => Ok(false),
        _ => Err(bad("invalid global form appearance policy")),
    }
}
fn clear_regeneration(doc: &mut Document) -> EngineResult<()> {
    let value = doc.catalog().map_err(bad)?.get(b"AcroForm").map_err(bad)?;
    if let Object::Reference(start) = value {
        let mut id = *start;
        let mut seen = HashSet::new();
        loop {
            if seen.len() >= 64 || !seen.insert(id) {
                return Err(bad("cyclic form owner"));
            }
            match doc
                .objects
                .get(&id)
                .ok_or_else(|| bad("missing form owner"))?
            {
                Object::Reference(next) => id = *next,
                Object::Dictionary(_) => break,
                _ => return Err(bad("invalid form owner")),
            }
        }
        doc.get_dictionary_mut(id)
            .map_err(bad)?
            .set("NeedAppearances", false);
    } else {
        doc.catalog_mut()
            .map_err(bad)?
            .get_mut(b"AcroForm")
            .map_err(bad)?
            .as_dict_mut()
            .map_err(bad)?
            .set("NeedAppearances", false);
    }
    Ok(())
}
struct Witness {
    hash: Sha256,
    remaining: usize,
}
impl Write for Witness {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.remaining = self
            .remaining
            .checked_sub(bytes.len())
            .ok_or_else(|| io::Error::other("native field witness exceeds 64 MiB"))?;
        self.hash.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn witness(
    native: &NativeForm<'_>,
    pages: usize,
    placements: &[(usize, usize)],
    selected: &HashSet<(usize, usize)>,
    exclude: bool,
) -> EngineResult<[u8; 32]> {
    let mut writer = Witness {
        hash: Sha256::new(),
        remaining: LIMIT,
    };
    serde_json::to_writer(&mut writer, &pages).map_err(bad)?;
    serde_json::to_writer(&mut writer, &placements.len()).map_err(bad)?;
    for &(page, index) in placements {
        if !exclude || !selected.contains(&(page, index)) {
            // Release each model after hashing. An unrelated page may exceed
            // the UI's retained-page budget without blocking a small valid edit.
            let field = native.field_bounded(page, index, 8 * 1024 * 1024)?;
            serde_json::to_writer(&mut writer, &(page, index, &field)).map_err(bad)?;
        }
    }
    Ok(writer.hash.finalize().into())
}
fn merge(doc: &mut Document, staging: Document) -> EngineResult<()> {
    if staging
        .objects
        .keys()
        .any(|id| doc.objects.contains_key(id))
    {
        return Err(bad("staged appearance identifiers collide"));
    }
    doc.max_id = doc.max_id.max(staging.max_id);
    doc.objects.extend(staging.objects);
    Ok(())
}
fn install_appearances(
    doc: &mut Document,
    updates: Vec<(ObjectId, Dictionary)>,
) -> EngineResult<()> {
    for (id, appearance) in updates {
        doc.get_dictionary_mut(id)
            .map_err(bad)?
            .set("AP", appearance);
    }
    Ok(())
}
fn materialize_baseline(
    identity: &mut FillIdentity,
    baseline: &Document,
    bound: &BoundFill,
    global: bool,
) -> EngineResult<usize> {
    let mut staging = Document::new();
    staging.max_id = identity.original.max_id;
    let mut copy = Copier::with_byte_limit(baseline, &mut staging, LIMIT);
    let mut admission = NormalAppearanceAdmission::new(baseline);
    let mut updates = Vec::new();
    for node in bound.nodes.iter().filter(|n| n.widget) {
        if !global && (!node.selected || normal(&identity.original, node.original)?.is_some()) {
            continue;
        }
        let value = match normal(baseline, node.native)? {
            Some(value) => value,
            None if !global && node.selected => continue,
            None => {
                return Err(bad(
                    "the initial native display has no materializable appearance",
                ))
            }
        };
        admission.admit(value)?;
        let new_normal = copy.copy(value)?;
        let old = ap(&identity.original, node.original)?;
        if let Some(d) = old {
            copy.reserve_dictionary(d)?;
        }
        let mut appearance = old.cloned().unwrap_or_default();
        appearance.set("N", new_normal);
        updates.push((node.original, appearance));
    }
    let used = copy.bytes_used();
    drop(copy);
    merge(&mut identity.original, staging)?;
    install_appearances(&mut identity.original, updates)?;
    if global {
        clear_regeneration(&mut identity.original)?;
    }
    Ok(used)
}
fn stage_buttons(
    doc: &Document,
    native: &Document,
    bound: &BoundFill,
    limit: usize,
) -> EngineResult<(Document, Vec<(ObjectId, Dictionary)>)> {
    let mut staging = Document::new();
    staging.max_id = doc.max_id;
    let mut copy = Copier::with_byte_limit(native, &mut staging, limit);
    let mut admission = NormalAppearanceAdmission::new(native);
    let mut updates = Vec::new();
    for node in bound.nodes.iter().filter(|n| n.widget && n.selected) {
        let widget = native.get_dictionary(node.native).map_err(bad)?;
        let state = resolve(native, widget.get(b"AS").map_err(bad)?)?
            .as_name()
            .map_err(bad)?;
        let empty_states = Dictionary::new();
        let states = normal(doc, node.original)?
            .map(|v| dict(doc, v))
            .transpose()?
            .unwrap_or(&empty_states);
        if let Ok(value) = states.get(state) {
            resolve(doc, value)?.as_stream().map_err(bad)?;
            continue;
        }
        let generated = dict(
            native,
            normal(native, node.native)?
                .ok_or_else(|| bad("button state could not be generated"))?,
        )?;
        let new_state = generated.get(state).map_err(|_| {
            bad("the selected button state has no preserved or native-generated appearance")
        })?;
        resolve(native, new_state)?.as_stream().map_err(bad)?;
        admission.admit(new_state)?;
        copy.reserve_dictionary(states)?;
        let mut states = states.clone();
        states.set(state.to_vec(), copy.copy(new_state)?);
        let old = ap(doc, node.original)?;
        if let Some(old) = old {
            copy.reserve_dictionary(old)?;
        }
        let mut appearance = old.cloned().unwrap_or_default();
        appearance.set("N", states);
        updates.push((node.original, appearance));
    }
    drop(copy);
    Ok((staging, updates))
}

pub(crate) fn fill(
    _pdfium: &Pdfium,
    bindings: &dyn PdfiumLibraryBindings,
    source: &[u8],
    request: &FillFormFieldRequest,
) -> EngineResult<Vec<u8>> {
    let mut identity = FillIdentity::prepare(source, request)?;
    let pages = identity.original.get_pages().len();
    let selected: HashSet<_> = identity.selected_placements().into_iter().collect();
    let placements = identity.widget_placements();
    let global = global_regeneration(&identity.original)?;
    let mut need_baseline = global;
    for id in identity.selected_widgets() {
        need_baseline |= normal(&identity.original, id)?.is_none();
    }
    let probe = identity.probe_bytes()?;
    let native_form = NativeForm::open(bindings, &probe)?;
    let validated = native_form.validate_fill(request)?;
    if native_form.matches_value(
        request.page_index,
        request.annotation_index,
        &validated.wanted,
    )? {
        return Ok(source.to_vec());
    }
    let before = witness(&native_form, pages, &placements, &selected, true)?;
    let baseline = if need_baseline {
        Some(crate::pdf_output::load(
            &native_form.snapshot_private(512 * 1024 * 1024)?,
        )?)
    } else {
        None
    };
    let generated_bytes = native_form.fill(request)?;
    if before != witness(&native_form, pages, &placements, &selected, true)? {
        return Err(bad("native filling changed another field"));
    }
    let expected = witness(&native_form, pages, &placements, &selected, false)?;
    drop(native_form);
    drop(probe);
    let generated = crate::pdf_output::load(&generated_bytes)?;
    drop(generated_bytes);
    let bound = identity.bind_native(&generated)?;
    let bound_placements: HashSet<_> = bound
        .nodes
        .iter()
        .filter(|n| n.selected && n.widget)
        .flat_map(|n| n.placements.iter().copied())
        .collect();
    if bound_placements != selected {
        return Err(bad("selected widget placements changed"));
    }
    let baseline_bound = baseline
        .as_ref()
        .map(|doc| identity.bind_native(doc))
        .transpose()?;
    let used = if let (Some(baseline), Some(baseline_bound)) = (&baseline, &baseline_bound) {
        materialize_baseline(&mut identity, baseline, baseline_bound, global)?
    } else {
        0
    };
    drop(baseline);
    drop(baseline_bound);
    let kind = match validated.field.kind.as_str() {
        "text" => Some(AnswerKind::Text),
        "combo" => Some(AnswerKind::Combo),
        "list" => Some(AnswerKind::List),
        "checkbox" | "radio" => None,
        _ => return Err(bad("unsupported field kind")),
    };
    let (staging, updates) = if let Some(kind) = kind {
        let mut staging = Document::new();
        let mut updates = Vec::new();
        let mut budget = FillAppearanceBudget::with_limit(
            &identity.original,
            &generated,
            &mut staging,
            LIMIT.saturating_sub(used),
        )?;
        let empty = match &validated.wanted {
            crate::FormFieldValue::Text { text } => text.is_empty(),
            crate::FormFieldValue::Choice { indices } => indices.is_empty(),
            _ => false,
        };
        let source_empty = match &validated.field.value {
            crate::FormFieldValue::Text { text } => text.is_empty(),
            crate::FormFieldValue::Choice { indices } => indices.is_empty(),
            _ => false,
        };
        let mut contexts = context::Contexts::new(&identity.original, pages, &mut budget)?;
        for node in bound.nodes.iter().filter(|n| n.widget && n.selected) {
            let next_value = normal(&generated, node.native)?
                .ok_or_else(|| bad("native answer has no generated appearance"))?;
            let next = resolve(&generated, next_value)?.as_stream().map_err(bad)?;
            let stream = if let Some(original) = normal(&identity.original, node.original)? {
                let original = resolve(&identity.original, original)?
                    .as_stream()
                    .map_err(bad)?;
                let resources = contexts.bind(original, &node.placements, kind, &mut budget)?;
                let StagedAnswerAppearance { appearance } = transplant_answer_with_resources(
                    &identity.original,
                    original,
                    &generated,
                    next,
                    kind,
                    &mut budget,
                    empty,
                    source_empty,
                    Some(&resources),
                )?;
                Object::Reference(budget.retain_local(Object::Stream(appearance))?)
            } else {
                // There is no original normal paint to erase. Retain R/D modes
                // separately and materialize the validated native normal form.
                let copied = budget.copy_native(next_value)?;
                if matches!(copied, Object::Stream(_)) {
                    Object::Reference(budget.retain_local(copied)?)
                } else {
                    copied
                }
            };
            let old = ap(&identity.original, node.original)?;
            if let Some(old) = old {
                budget.reserve_dictionary(old)?;
            }
            let mut appearance = old.cloned().unwrap_or_default();
            appearance.set("N", stream);
            updates.push((node.original, appearance));
        }
        drop(budget);
        (staging, updates)
    } else {
        stage_buttons(
            &identity.original,
            &generated,
            &bound,
            LIMIT.saturating_sub(used),
        )?
    };
    merge(&mut identity.original, staging)?;
    install_appearances(&mut identity.original, updates)?;
    for change in bound.changes {
        let owner = identity
            .original
            .get_dictionary_mut(change.original)
            .map_err(bad)?;
        if let Some(value) = change.value {
            owner.set(change.key, value);
        } else {
            owner.remove(&change.key);
        }
    }
    crate::page_resources::prune_unreachable(&mut identity.original)?;
    let output = crate::pdf_output::save(&mut identity.original)?;
    let saved = NativeForm::open(bindings, &output)?;
    if !saved.matches_value(
        request.page_index,
        request.annotation_index,
        &validated.wanted,
    )? || witness(&saved, pages, &placements, &selected, false)? != expected
    {
        return Err(bad(
            "saved field values or unrelated settings differ from the native result",
        ));
    }
    drop(saved);
    Ok(output)
}
