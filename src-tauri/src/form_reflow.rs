//! Regenerate one text widget in isolation, then import only its normal appearance.
use crate::{
    form_appearance::{Copier, Equivalence},
    form_position::bad,
    EngineResult, FillFormFieldRequest, FormFieldValue,
};
use lopdf::{content::Content, dictionary, Dictionary, Document, Object, ObjectId};
use pdfium_render::prelude::PdfiumLibraryBindings;

fn resolve<'a>(d: &'a Document, v: &'a Object) -> EngineResult<&'a Object> {
    Ok(d.dereference(v).map_err(bad)?.1)
}
fn dict<'a>(d: &'a Document, v: &'a Object) -> EngineResult<&'a Dictionary> {
    resolve(d, v)?.as_dict().map_err(bad)
}
fn inherited<'a>(
    d: &'a Document,
    mut id: ObjectId,
    key: &[u8],
) -> EngineResult<Option<&'a Object>> {
    let mut seen = std::collections::HashSet::new();
    for _ in 0..64 {
        if !seen.insert(id) {
            return Err(bad("cyclic field settings"));
        }
        let field = d.get_dictionary(id).map_err(bad)?;
        if let Ok(v) = field.get(key) {
            return Ok(Some(resolve(d, v)?));
        }
        match field.get(b"Parent") {
            Ok(v) => id = v.as_reference().map_err(bad)?,
            Err(_) => return Ok(None),
        }
    }
    Err(bad("field settings exceed inheritance limit"))
}
fn widget(d: &Document, page: usize, index: usize) -> EngineResult<ObjectId> {
    let page = *d
        .get_pages()
        .values()
        .nth(page)
        .ok_or_else(|| bad("missing field page"))?;
    resolve(
        d,
        d.get_dictionary(page)
            .map_err(bad)?
            .get(b"Annots")
            .map_err(bad)?,
    )?
    .as_array()
    .map_err(bad)?
    .get(index)
    .ok_or_else(|| bad("missing field"))?
    .as_reference()
    .map_err(bad)
}
fn normal(d: &Document, id: ObjectId) -> EngineResult<&Object> {
    resolve(
        d,
        dict(
            d,
            d.get_dictionary(id).map_err(bad)?.get(b"AP").map_err(bad)?,
        )?
        .get(b"N")
        .map_err(bad)?,
    )
}
fn fonts<'a>(d: &'a Document, resources: &'a Dictionary) -> EngineResult<Option<&'a Dictionary>> {
    resources.get(b"Font").ok().map(|v| dict(d, v)).transpose()
}
fn canonical_appearance(
    content: &Content,
    font: &lopdf::content::Operation,
) -> EngineResult<Object> {
    // PDFium's DA lookup searches operator kinds, rather than evaluating their
    // order. Supply one final color to the probe; never rewrite the source DA.
    let mut color = lopdf::content::Operation::new("g", vec![0.into()]);
    for op in &content.operations {
        let count = match op.operator.as_str() {
            "Tf" => continue,
            "g" => 1,
            "rg" => 3,
            "k" => 4,
            _ => {
                return Err(bad(
                    "unsupported field text settings; use Scale saved appearance",
                ))
            }
        };
        if op.operands.len() != count
            || op.operands.iter().any(|v| {
                v.as_float()
                    .map(|v| !v.is_finite() || !(0. ..=1.).contains(&v))
                    .unwrap_or(true)
            })
        {
            return Err(bad("invalid field text color; use Scale saved appearance"));
        }
        color = op.clone();
    }
    Ok(Object::String(
        Content {
            operations: vec![font.clone(), color],
        }
        .encode()
        .map_err(bad)?,
        lopdf::StringFormat::Literal,
    ))
}

fn regenerable_border(source: &Document, widget: &Dictionary) -> EngineResult<()> {
    let refusal =
        || bad("this border cannot be preserved by text reflow; use Scale saved appearance");
    let width = |value: &Object| -> EngineResult<f32> {
        let n = resolve(source, value)?.as_float().map_err(bad)?;
        // The pinned renderer retains fractional widths during generation and
        // interactive filling. Keep finite/range checks without truncating them.
        if !n.is_finite() || n < 0. || n > 10000. {
            return Err(refusal());
        }
        Ok(n)
    };
    let bs = widget
        .get(b"BS")
        .ok()
        .map(|v| dict(source, v))
        .transpose()?;
    let bs_width = if let Some(bs) = bs {
        if let Ok(style) = bs.get(b"S") {
            if resolve(source, style)?.as_name().map_err(bad)? != b"S" {
                return Err(refusal());
            }
        }
        Some(bs.get(b"W").ok().map(width).transpose()?.unwrap_or(1.))
    } else {
        None
    };
    if let Ok(border) = widget.get(b"Border") {
        let border = resolve(source, border)?.as_array().map_err(bad)?;
        if !(3..=4).contains(&border.len()) {
            return Err(refusal());
        }
        if border[..2].iter().any(|v| {
            resolve(source, v)
                .and_then(|v| v.as_float().map_err(bad))
                .map(|v| v != 0.)
                .unwrap_or(true)
        }) {
            return Err(refusal());
        }
        let w = width(&border[2])?;
        // /BS and /Border precedence differs across native generation paths.
        if bs_width.is_some_and(|n| n != w) {
            return Err(refusal());
        }
        if border.len() == 4
            && !resolve(source, &border[3])?
                .as_array()
                .map_err(bad)?
                .is_empty()
        {
            return Err(refusal());
        }
    }
    Ok(())
}
/// Before resizing, prove there is no artwork that regeneration would discard.
/// Reflow deliberately uses the stored answer and prefers appearance-local fonts;
/// those existing semantics differ from editing the visible text's own settings.
pub(crate) fn validate_saved_paint(bytes: &[u8], page: usize, index: usize) -> EngineResult<()> {
    let (doc, _) = crate::acroforms::authoring_document(bytes)?;
    let id = widget(&doc, page, index)?;
    let w = doc.get_dictionary(id).map_err(bad)?;
    let n = normal(&doc, id)?.as_stream().map_err(bad)?;
    let data = n.get_plain_content_with_limit(1024 * 1024).map_err(bad)?;
    crate::form_answer_appearance::preflight_program(&data)?;
    let ops = Content::decode_strict(&data).map_err(bad)?;
    if ops.operations.len() > 8192 {
        return Err(bad("saved appearance has too many instructions"));
    }
    let form = dict(
        &doc,
        doc.catalog().map_err(bad)?.get(b"AcroForm").map_err(bad)?,
    )?;
    let da = inherited(&doc, id, b"DA")?
        .or(form.get(b"DA").ok().map(|v| resolve(&doc, v)).transpose()?)
        .ok_or_else(|| bad("missing default text appearance"))?
        .as_str()
        .map_err(bad)?;
    if da.len() > 64 * 1024 {
        return Err(bad("field default appearance is too large"));
    }
    let content = Content::decode_strict(da).map_err(bad)?;
    let fonts = content
        .operations
        .iter()
        .filter(|op| op.operator == "Tf")
        .collect::<Vec<_>>();
    if fonts.len() != 1 || fonts[0].operands.len() != 2 {
        return Err(bad("requires one explicit default font"));
    }
    let size = fonts[0].operands[1].as_float().map_err(bad)?;
    if !size.is_finite() || size < 0. {
        return Err(bad("invalid default font size"));
    }
    let canonical = canonical_appearance(&content, fonts[0])?;
    let settings = Content::decode_strict(canonical.as_str().map_err(bad)?).map_err(bad)?;
    if ops.operations.iter().any(|op| matches!(op.operator.as_str(), "Tj" | "TJ" | "'" | "\"")) {
        use crate::form_answer_appearance::{AnswerKind, AnswerSlot};
        AnswerSlot::parse(&data, AnswerKind::Text)
            .and_then(|slot| slot.require_all_text_in_answer())
            .map_err(|e| bad(format!("saved caption cannot be safely regenerated; use Scale saved appearance ({e})")))?;
    }
    crate::form_text_appearance::compatible_saved_appearance(
        &doc,
        w,
        n,
        &ops,
        size,
        &settings.operations[1],
    )
    .map_err(|e| {
        bad(format!(
            "saved field artwork cannot be safely regenerated; use Scale saved appearance ({e})"
        ))
    })
}
pub(crate) fn change(
    bindings: &dyn PdfiumLibraryBindings,
    bytes: &[u8],
    page: usize,
    index: usize,
    value: &FormFieldValue,
) -> EngineResult<Vec<u8>> {
    let (mut source, _) = crate::acroforms::authoring_document(bytes)?;
    let id = widget(&source, page, index)?;
    let w = source.get_dictionary(id).map_err(bad)?;
    regenerable_border(&source, w)?;
    if inherited(&source, id, b"FT")?.and_then(|v| v.as_name().ok()) != Some(b"Tx") {
        return Err(bad("text reflow requires a text field"));
    }
    let form = dict(
        &source,
        source
            .catalog()
            .map_err(bad)?
            .get(b"AcroForm")
            .map_err(bad)?,
    )?;
    let da = inherited(&source, id, b"DA")?
        .or(form
            .get(b"DA")
            .ok()
            .map(|v| resolve(&source, v))
            .transpose()?)
        .ok_or_else(|| bad("text field has no default appearance"))?;
    let da_bytes = da.as_str().map_err(bad)?;
    if da_bytes.len() > 64 * 1024 {
        return Err(bad("field default appearance is too large"));
    }
    let content = Content::decode_strict(da_bytes).map_err(bad)?;
    let tf: Vec<_> = content
        .operations
        .iter()
        .filter(|op| op.operator == "Tf")
        .collect();
    if tf.len() != 1 || tf[0].operands.len() != 2 {
        return Err(bad("text reflow requires one explicit default font"));
    }
    let name = tf[0].operands[0].as_name().map_err(bad)?;
    let size = tf[0].operands[1].as_float().map_err(bad)?;
    if name.is_empty() || !size.is_finite() || size < 0. {
        return Err(bad("invalid default font"));
    }
    let probe_da = canonical_appearance(&content, tf[0])?;
    let old = normal(&source, id)?.as_stream().map_err(bad)?;
    let local = old
        .dict
        .get(b"Resources")
        .ok()
        .map(|v| dict(&source, v))
        .transpose()?
        .map(|r| fonts(&source, r))
        .transpose()?
        .flatten();
    let global = form
        .get(b"DR")
        .ok()
        .map(|v| dict(&source, v))
        .transpose()?
        .map(|r| fonts(&source, r))
        .transpose()?
        .flatten();
    let font = local
        .and_then(|f| f.get(name).ok())
        .or_else(|| global.and_then(|f| f.get(name).ok()))
        .ok_or_else(|| {
            bad("the original field font cannot be preserved; use Scale saved appearance")
        })?;
    let font_dict = dict(&source, font)?;
    if font_dict
        .get(b"Type")
        .ok()
        .map(|v| resolve(&source, v))
        .transpose()?
        .and_then(|v| v.as_name().ok())
        != Some(b"Font")
        || !font_dict.has(b"Subtype")
    {
        return Err(bad("invalid field font"));
    }
    let flags = inherited(&source, id, b"Ff")?
        .map(|v| v.as_i64().map_err(bad))
        .transpose()?
        .unwrap_or(0);
    if flags & ((1 << 20) | (1 << 25)) != 0 {
        return Err(bad(
            "rich text and file-selection fields require Scale saved appearance",
        ));
    }
    let rect = resolve(&source, w.get(b"Rect").map_err(bad)?)?
        .as_array()
        .map_err(bad)?;
    if rect.len() != 4 {
        return Err(bad("invalid field rectangle"));
    }
    let xy: Vec<f32> = rect
        .iter()
        .map(|v| resolve(&source, v)?.as_float().map_err(bad))
        .collect::<EngineResult<_>>()?;
    let mut probe = Document::with_version("1.7");
    let pages = probe.new_object_id();
    let page_id = probe.new_object_id();
    let field_id = probe.new_object_id();
    let mut probe_widget = dictionary! {"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>Object::string_literal("Reflow"),"P"=>page_id,"Ff"=>flags&!1};
    let mut probe_fonts = Dictionary::new();
    {
        let mut copy = Copier::new(&source, &mut probe);
        probe_widget.set("DA", copy.copy(&probe_da)?);
        for key in [b"V".as_slice(), b"Q", b"MaxLen"] {
            let v = inherited(&source, id, key)?.or(if key == b"Q" {
                form.get(key).ok()
            } else {
                None
            });
            if let Some(v) = v {
                probe_widget.set(key, copy.copy(v)?);
            }
        }
        for key in [b"Rect".as_slice(), b"MK", b"BS", b"Border", b"F"] {
            if let Ok(v) = w.get(key) {
                probe_widget.set(key, copy.copy(v)?);
            }
        }
        if let Some(local) = local {
            for (k, v) in local.iter() {
                probe_fonts.set(k.clone(), copy.copy(v)?);
            }
        }
        probe_fonts.set(name, copy.copy(font)?);
    }
    probe.objects.insert(field_id, probe_widget.into());
    probe.objects.insert(page_id,dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![(xy[0].min(0.)-10.).into(),(xy[1].min(0.)-10.).into(),(xy[2].max(0.)+10.).into(),(xy[3].max(0.)+10.).into()],"Annots"=>vec![field_id.into()],"Resources"=>Dictionary::new()}.into());
    probe.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Kids"=>vec![page_id.into()],"Count"=>1}.into(),
    );
    let catalog=probe.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages,"AcroForm"=>dictionary!{"Fields"=>vec![field_id.into()],"DR"=>dictionary!{"Font"=>probe_fonts}}});
    probe.trailer.set("Root", catalog);
    let probe_bytes = crate::pdf_output::save(&mut probe)?;
    let native = crate::form_fill::NativeForm::open(bindings, &probe_bytes)
        .map_err(|e| bad(format!("probe open: {e}")))?;
    let generated = native
        .fill(&FillFormFieldRequest {
            page_index: 0,
            annotation_index: 0,
            expected_value: value.clone(),
            value: value.clone(),
        })
        .map_err(|e| bad(format!("probe fill: {e}")))?;
    let generated = Document::load_mem_with_options(
        &generated,
        lopdf::LoadOptions::with_max_decompressed_size(16 * 1024 * 1024),
    )
    .map_err(bad)?;
    let appearance = normal(&generated, widget(&generated, 0, 0)?)?;
    let stream = appearance.as_stream().map_err(bad)?;
    let resources = dict(&generated, stream.dict.get(b"Resources").map_err(bad)?)?;
    // Native text appearances should not need external artwork or inherited resources.
    if resources
        .iter()
        .any(|(k, _)| !matches!(k.as_slice(), b"Font" | b"ProcSet"))
    {
        return Err(bad("unexpected generated appearance resources"));
    }
    let new_fonts = fonts(&generated, resources)?
        .ok_or_else(|| bad("generated appearance has no font resources"))?;
    let mut proof = Equivalence::default();
    for (k, v) in new_fonts.iter() {
        let original = if k.as_slice() == name {
            Some(font)
        } else {
            local.and_then(|f| f.get(k).ok())
        };
        if !original
            .map(|f| proof.check(&source, f, &generated, v))
            .transpose()?
            .unwrap_or(false)
        {
            return Err(bad(
                "native reflow substituted a font; use Scale saved appearance",
            ));
        }
    }
    let bbox = resolve(&generated, stream.dict.get(b"BBox").map_err(bad)?)?
        .as_array()
        .map_err(bad)?;
    if bbox.len() != 4
        || bbox
            .iter()
            .any(|v| v.as_float().map(|n| !n.is_finite()).unwrap_or(true))
    {
        return Err(bad("invalid regenerated appearance geometry"));
    }
    let copied = Copier::new(&generated, &mut source).copy(appearance)?;
    // PDF streams must be indirect objects, even when the surrounding AP is direct.
    let copied = source.add_object(copied);
    let original_ap = dict(
        &source,
        source
            .get_dictionary(id)
            .map_err(bad)?
            .get(b"AP")
            .map_err(bad)?,
    )?;
    let mut budget = crate::form_graph::CopyBudget::default();
    budget.dictionary(original_ap)?;
    let mut ap = original_ap.clone();
    ap.set("N", copied);
    source.get_dictionary_mut(id).map_err(bad)?.set("AP", ap);
    crate::page_resources::prune_unreachable(&mut source)?;
    crate::pdf_output::save(&mut source)
}
