//! Atomic whole-source choice edits; no registry or filesystem side effects.
#![allow(dead_code)]
use crate::{
    form_choice_appearance::{self, AppearanceBudget},
    form_choice_probe::generate_choice_appearance,
    form_choice_text::ChoiceTextBlock,
    form_properties::Context,
    ChoiceSelection, EngineError, EngineResult, FormChoices, SetFormChoicesRequest,
};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use pdfium_render::prelude::{Pdfium, PdfiumLibraryBindings};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    io::{self, Write},
};

fn bad(reason: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("Choice edit: {reason}"))
}
fn resolve<'a>(doc: &'a Document, value: &'a Object) -> EngineResult<&'a Object> {
    Ok(doc.dereference(value).map_err(bad)?.1)
}
fn dict<'a>(doc: &'a Document, value: &'a Object) -> EngineResult<&'a Dictionary> {
    resolve(doc, value)?.as_dict().map_err(bad)
}
fn appearance<'a>(
    doc: &'a Document,
    widget: ObjectId,
) -> EngineResult<(&'a Dictionary, &'a Stream)> {
    let w = doc.get_dictionary(widget).map_err(bad)?;
    let ap = dict(doc, w.get(b"AP").map_err(bad)?)?;
    let normal = resolve(doc, ap.get(b"N").map_err(bad)?)?
        .as_stream()
        .map_err(bad)?;
    Ok((ap, normal))
}
fn text_block(bytes: &[u8]) -> EngineResult<ChoiceTextBlock> {
    ChoiceTextBlock::parse(bytes).or_else(|_| ChoiceTextBlock::parse_empty_background(bytes))
}
fn regeneration_policy(doc: &Document) -> EngineResult<()> {
    let form = dict(
        doc,
        doc.catalog().map_err(bad)?.get(b"AcroForm").map_err(bad)?,
    )?;
    if let Ok(value) = form.get(b"NeedAppearances") {
        if !matches!(resolve(doc, value)?, Object::Boolean(false)) {
            return Err(bad(
                "this document requests or has an invalid global form regeneration policy",
            ));
        }
    }
    Ok(())
}

/// Preserve native values/settings of unrelated fields without retaining their
/// text models or allocating an unbounded serialized snapshot. Selected field
/// values/options are checked against the source-bound model, then omitted from
/// the equality digest. They still count toward the verification work budget.
struct NativeWitness {
    hash: Sha256,
    remaining: usize,
    hashing: bool,
}
impl NativeWitness {
    fn new() -> Self {
        Self {
            hash: Sha256::new(),
            remaining: 64 * 1024 * 1024,
            hashing: true,
        }
    }
}
impl Write for NativeWitness {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.remaining = self
            .remaining
            .checked_sub(bytes.len())
            .ok_or_else(|| io::Error::other("native field verification exceeds its budget"))?;
        if self.hashing {
            self.hash.update(bytes);
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn native_witness(
    bindings: &dyn PdfiumLibraryBindings,
    bytes: &[u8],
    ctx: &Context,
    info: &FormChoices,
) -> EngineResult<[u8; 32]> {
    use crate::form_fill::{FormFieldValue, NativeForm};
    let native = NativeForm::open(bindings, bytes)?;
    let selected: HashSet<_> = info
        .widgets
        .iter()
        .map(|v| (v.page_index, v.annotation_index))
        .collect();
    let mut found = HashSet::new();
    let mut writer = NativeWitness::new();
    let pages = ctx.doc.get_pages().len();
    serde_json::to_writer(&mut writer, &pages).map_err(bad)?;
    let expected_rows = match &info.model.selection {
        ChoiceSelection::Rows(rows) => rows.as_slice(),
        _ => &[],
    };
    for page in 0..pages {
        let fields = native.fields_bounded(page, 8 * 1024 * 1024)?;
        serde_json::to_writer(&mut writer, &(page, fields.len())).map_err(bad)?;
        for mut field in fields {
            if selected.contains(&(page, field.annotation_index)) {
                found.insert((page, field.annotation_index));
                if field.selected_indices != expected_rows {
                    return Err(bad(
                        "native selection disagrees with the saved choice model",
                    ));
                }
                if field.name != info.name
                    || field.kind != if info.model.combo { "combo" } else { "list" }
                    || field.editable_choice != info.model.editable
                    || field.multi_select != info.model.multiple
                    || field.options.len() != info.model.options.len()
                    || field
                        .options
                        .iter()
                        .zip(&info.model.options)
                        .any(|(a, b)| a != &b.display)
                {
                    return Err(bad("native field type, labels or ownership disagree with the saved choice model"));
                }
                if info.model.editable {
                    let text = match &info.model.selection {
                        ChoiceSelection::Rows(rows) => {
                            info.model.options[rows[0]].export_value.as_str()
                        }
                        ChoiceSelection::Text(text) => text.as_str(),
                        ChoiceSelection::None if ctx.raw_attribute(b"V")?.is_none() => info
                            .model
                            .defaults
                            .as_ref()
                            .and_then(|v| v.first())
                            .map(String::as_str)
                            .unwrap_or(""),
                        ChoiceSelection::None => "",
                    };
                    let expected = crate::form_fill::normalized(&FormFieldValue::Text {
                        text: text.to_owned(),
                    });
                    if field.value != expected {
                        return Err(bad(
                            "native current answer disagrees with the saved choice model",
                        ));
                    }
                }
                writer.hashing = false;
                serde_json::to_writer(&mut writer, &field).map_err(bad)?;
                writer.hashing = true;
                field.value = FormFieldValue::Choice { indices: vec![] };
                field.options.clear();
                field.selected_indices.clear();
            }
            serde_json::to_writer(&mut writer, &field).map_err(bad)?;
        }
    }
    if found != selected {
        return Err(bad(
            "a linked widget is missing from native form inspection",
        ));
    }
    Ok(writer.hash.finalize().into())
}

fn placements(
    ctx: &Context,
    info: &FormChoices,
) -> EngineResult<BTreeMap<ObjectId, Vec<ObjectId>>> {
    let pages: Vec<_> = ctx.doc.get_pages().values().copied().collect();
    let mut found = BTreeMap::<ObjectId, Vec<ObjectId>>::new();
    let mut scales = HashSet::new();
    for location in &info.widgets {
        let page = *pages
            .get(location.page_index)
            .ok_or_else(|| bad("linked page is missing"))?;
        let a = resolve(
            &ctx.doc,
            ctx.doc
                .get_dictionary(page)
                .map_err(bad)?
                .get(b"Annots")
                .map_err(bad)?,
        )?
        .as_array()
        .map_err(bad)?;
        let widget = a
            .get(location.annotation_index)
            .ok_or_else(|| bad("linked widget is missing"))?
            .as_reference()
            .map_err(bad)?;
        if !ctx.widgets.contains(&widget) {
            return Err(bad("linked widget is outside the source field"));
        }
        let unit = crate::engine::source_unit_from_pdf(&ctx.doc, page)?;
        if scales.insert((widget, unit.to_bits())) {
            found.entry(widget).or_default().push(page);
        }
    }
    Ok(found)
}

struct StagedAppearance {
    widget: ObjectId,
    ap: Dictionary,
    normal: Stream,
}

pub(crate) fn change(
    pdfium: &Pdfium,
    bindings: &dyn PdfiumLibraryBindings,
    bytes: &[u8],
    request: &SetFormChoicesRequest,
) -> EngineResult<Vec<u8>> {
    if request.expected_key.len() > 128 {
        return Err(bad("invalid source key"));
    }
    let ctx = crate::form_properties::load(bytes, request.page_index, request.annotation_index)?;
    let before = crate::form_choices::inspect_context(&ctx)?;
    if before.expected_key != request.expected_key {
        return Err(bad("the field changed; reopen choices"));
    }
    regeneration_policy(&ctx.doc)?;
    let witness = native_witness(bindings, bytes, &ctx, &before)?;
    let locations = placements(&ctx, &before)?;
    let mut budget = AppearanceBudget::default();
    for (&widget, pages) in &locations {
        let (_, original) = appearance(&ctx.doc, widget)?;
        let generated = generate_choice_appearance(bindings, &ctx, widget)?;
        let old = text_block(
            &original
                .get_plain_content_with_limit(256 * 1024)
                .map_err(bad)?,
        )?;
        let new = text_block(
            &generated
                .appearance
                .get_plain_content_with_limit(256 * 1024)
                .map_err(bad)?,
        )?;
        let mapping = form_choice_appearance::geometry(
            &ctx.doc,
            original,
            &generated.document,
            &generated.appearance,
        )?;
        let unchanged = old.transplant_mapped(&new, &generated.font_names, mapping)?;
        for &page in pages {
            form_choice_appearance::verify_unchanged(
                pdfium,
                &ctx.doc,
                widget,
                page,
                &unchanged,
                &mut budget,
            )?;
        }
    }
    let mut prepared = crate::form_choice_draft::prepare_choice_edit_context(ctx, request)?;
    if !prepared.has_changes() {
        let mut exact = Vec::new();
        exact.try_reserve_exact(bytes.len()).map_err(bad)?;
        exact.extend_from_slice(bytes);
        return Ok(exact);
    }
    let expected = crate::form_choices::inspect_context(&prepared.ctx)?;
    let repaint = prepared.needs_appearance_update();
    let mut staged = Vec::new();
    if repaint {
        budget.reserve_staged(
            locations
                .len()
                .checked_mul(std::mem::size_of::<StagedAppearance>())
                .ok_or_else(|| bad("retained appearance count overflow"))?,
        )?;
        staged.try_reserve_exact(locations.len()).map_err(bad)?;
    }
    for &widget in locations.keys() {
        let generated = generate_choice_appearance(bindings, &prepared.ctx, widget)?;
        if !repaint {
            continue;
        }
        let (ap, original) = appearance(&prepared.ctx.doc, widget)?;
        let mut copies = crate::form_graph::CopyBudget::with_byte_limit(32 * 1024 * 1024);
        copies.dictionary(&original.dict)?;
        copies.array(&[])?;
        for (key, value) in ap.iter().filter(|(key, _)| key.as_slice() != b"N") {
            copies.entry(key, value)?;
        }
        copies.entry(b"N", &Object::Null)?;
        budget.reserve_staged(copies.bytes_used())?;
        let old = text_block(
            &original
                .get_plain_content_with_limit(256 * 1024)
                .map_err(bad)?,
        )?;
        let new = text_block(
            &generated
                .appearance
                .get_plain_content_with_limit(256 * 1024)
                .map_err(bad)?,
        )?;
        let mapping = form_choice_appearance::geometry(
            &prepared.ctx.doc,
            original,
            &generated.document,
            &generated.appearance,
        )?;
        let content = old.transplant_mapped_bounded(
            &new,
            &generated.font_names,
            mapping,
            budget.remaining_bytes(),
        )?;
        budget.reserve_staged(content.len())?;
        form_choice_appearance::bound_resources(&prepared.ctx.doc, original, &content)?;
        let mut normal = Stream::new(original.dict.clone(), Vec::new());
        normal.set_plain_content(content);
        let mut next = Dictionary::new();
        for (key, value) in ap.iter().filter(|(key, _)| key.as_slice() != b"N") {
            next.set(key.clone(), value.clone());
        }
        staged.push(StagedAppearance {
            widget,
            ap: next,
            normal,
        });
    }
    // No source AP has been touched until every linked candidate has passed.
    for mut item in staged {
        item.ap.set("N", prepared.ctx.doc.add_object(item.normal));
        let ap = prepared.ctx.doc.add_object(item.ap);
        prepared
            .ctx
            .doc
            .get_dictionary_mut(item.widget)
            .map_err(bad)?
            .set("AP", ap);
    }
    crate::page_resources::prune_unreachable(&mut prepared.ctx.doc)?;
    let output = crate::pdf_output::save(&mut prepared.ctx.doc)?;
    let confirmed =
        crate::inspect_choice_pdf(&output, request.page_index, request.annotation_index)?;
    if confirmed.model != expected.model
        || confirmed.name != expected.name
        || confirmed.widgets != expected.widgets
    {
        return Err(bad(
            "the published choice state differs from the resolved edit",
        ));
    }
    if native_witness(bindings, &output, &prepared.ctx, &expected)? != witness {
        return Err(bad(
            "native settings or unrelated field values changed during publication",
        ));
    }
    Ok(output)
}
