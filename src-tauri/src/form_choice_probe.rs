//! One-widget native generation. Never publishes the probe or owns source APs.
// Native publication integration follows this private, independently tested layer.
#![allow(dead_code)]
use crate::{
    form_appearance::{Copier, Equivalence},
    form_properties::Context,
    EngineError, EngineResult,
};
use lopdf::{
    content::{Content, Operation},
    dictionary, Dictionary, Document, Object, ObjectId, Stream,
};
use pdfium_render::prelude::PdfiumLibraryBindings;
use std::collections::{BTreeMap, HashSet};

const PROBE_LIMIT: usize = 32 * 1024 * 1024;
#[path = "choice_codec.rs"]
mod codec;
fn bad(s: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("Choice appearance: {s}"))
}
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
    let mut seen = HashSet::new();
    for _ in 0..33 {
        if !seen.insert(id) {
            return Err(bad("cyclic widget settings"));
        }
        let node = d.get_dictionary(id).map_err(bad)?;
        if let Ok(v) = node.get(key) {
            return Ok(Some(resolve(d, v)?));
        }
        match node.get(b"Parent") {
            Ok(v) => id = v.as_reference().map_err(bad)?,
            Err(_) => return Ok(None),
        }
    }
    Ok(None)
}
fn normal<'a>(d: &'a Document, w: &'a Dictionary) -> EngineResult<&'a Stream> {
    // Resolve the AP reference from the dictionary stored in this document so the
    // returned borrow always belongs to d, including direct normal streams.
    let ap = w.get(b"AP").map_err(bad)?;
    let ap = dict(d, ap)?;
    resolve(d, ap.get(b"N").map_err(bad)?)?
        .as_stream()
        .map_err(bad)
}
fn fonts<'a>(d: &'a Document, r: &'a Dictionary) -> EngineResult<Option<&'a Dictionary>> {
    r.get(b"Font").ok().map(|v| dict(d, v)).transpose()
}
fn default_font(da: &[u8]) -> EngineResult<Vec<u8>> {
    if da.len() > 64 * 1024 {
        return Err(bad("default appearance exceeds64KiB"));
    }
    let content = Content::decode_strict(da).map_err(bad)?;
    let mut font = None;
    for op in content.operations {
        let count = match op.operator.as_str() {
            "Tf" => {
                if font.is_some() || op.operands.len() != 2 {
                    return Err(bad("one explicit default font is required"));
                }
                let name = op.operands[0].as_name().map_err(bad)?;
                let size = op.operands[1].as_float().map_err(bad)?;
                if name.is_empty() || !size.is_finite() || size < 0. {
                    return Err(bad("invalid default font"));
                }
                font = Some(name.to_vec());
                continue;
            }
            "g" => 1,
            "rg" => 3,
            "k" => 4,
            _ => return Err(bad("unsupported default text settings")),
        };
        if op.operands.len() != count
            || op.operands.iter().any(|v| {
                v.as_float()
                    .map(|n| !n.is_finite() || !(0. ..=1.).contains(&n))
                    .unwrap_or(true)
            })
        {
            return Err(bad("invalid default text color"));
        }
    }
    font.ok_or_else(|| bad("missing default font"))
}
pub(crate) struct GeneratedChoiceAppearance {
    pub(crate) document: Document,
    pub(crate) appearance: Stream,
    pub(crate) font_names: BTreeMap<Vec<u8>, Vec<u8>>,
}
pub(crate) fn generate_choice_appearance(
    bindings: &dyn PdfiumLibraryBindings,
    ctx: &Context,
    widget: ObjectId,
) -> EngineResult<GeneratedChoiceAppearance> {
    // Generate exactly the staged PDF state, never a parallel client-side model.
    let model = crate::form_choices::inspect_context(ctx)?.model;
    if !ctx.widgets.contains(&widget) {
        return Err(bad("widget is outside the selected logical field"));
    }
    let source = &ctx.doc;
    let w = source.get_dictionary(widget).map_err(bad)?;
    let form = dict(
        source,
        source
            .catalog()
            .map_err(bad)?
            .get(b"AcroForm")
            .map_err(bad)?,
    )?;
    let da = inherited(source, widget, b"DA")?
        .or(form
            .get(b"DA")
            .ok()
            .map(|v| resolve(source, v))
            .transpose()?)
        .ok_or_else(|| bad("missing default appearance"))?;
    let font_name = default_font(da.as_str().map_err(bad)?)?;
    let saved = normal(source, w)?;
    let local = saved
        .dict
        .get(b"Resources")
        .ok()
        .map(|v| dict(source, v))
        .transpose()?
        .map(|r| fonts(source, r))
        .transpose()?
        .flatten();
    let global = form
        .get(b"DR")
        .ok()
        .map(|v| dict(source, v))
        .transpose()?
        .map(|r| fonts(source, r))
        .transpose()?
        .flatten();
    let font = local
        .and_then(|f| f.get(&font_name).ok())
        .or_else(|| global.and_then(|f| f.get(&font_name).ok()))
        .ok_or_else(|| bad("the original widget font is unavailable"))?;
    let fd = dict(source, font)?;
    if resolve(source, fd.get(b"Type").map_err(bad)?)?
        .as_name()
        .map_err(bad)?
        != b"Font"
        || !matches!(
            resolve(source, fd.get(b"Subtype").map_err(bad)?)?
                .as_name()
                .map_err(bad)?,
            b"Type1" | b"TrueType" | b"Type0"
        )
    {
        return Err(bad(
            "the original font type is not supported by choice generation",
        ));
    }
    // Visible selection alone is insufficient: the dropdown menu and offscreen
    // list rows can expose any label later. Opaque export identifiers are not
    // display text except the raw DV fallback of an absent-value combo.
    // inspect_context has bounded the model before this slice collection.
    let mut required: Vec<&str> = model.options.iter().map(|o| o.display.as_str()).collect();
    if let crate::ChoiceSelection::Text(text) = &model.selection {
        required.push(text);
    }
    if model.editable {
        for value in model.defaults.iter().flatten() {
            if !model.options.iter().any(|o| o.export_value == *value) {
                required.push(value);
            }
        }
    }
    let displayed: Vec<&str> = if model.combo {
        let text = match &model.selection {
            crate::ChoiceSelection::Rows(rows) => {
                rows.first().map(|i| model.options[*i].display.as_str())
            }
            crate::ChoiceSelection::Text(text) => Some(text.as_str()),
            crate::ChoiceSelection::None if ctx.raw_attribute(b"V")?.is_none() => model
                .defaults
                .as_ref()
                .and_then(|v| v.first())
                .map(String::as_str),
            crate::ChoiceSelection::None => None,
        };
        text.filter(|s| !s.is_empty()).into_iter().collect()
    } else {
        model
            .options
            .iter()
            .skip(model.top_index)
            .map(|o| o.display.as_str())
            .collect()
    };
    for text in &displayed {
        if !required.contains(text) {
            required.push(text);
        }
    }
    let text_proof = crate::choice_font::ChoiceFontProof::for_texts(source, font, &required)?;
    let batches = codec::batches(&text_proof, &required)?;
    // Repertoire is a prerequisite, not native encoder/layout proof. Generated
    // font graph checks below remain mandatory; publication has further gates.
    let rect = resolve(source, w.get(b"Rect").map_err(bad)?)?
        .as_array()
        .map_err(bad)?;
    if rect.len() != 4 {
        return Err(bad("invalid widget rectangle"));
    }
    let xy = rect
        .iter()
        .map(|v| {
            let n = resolve(source, v)?.as_float().map_err(bad)?;
            if !n.is_finite() || n.abs() > 1_000_000. {
                return Err(bad("widget rectangle exceeds the probe limit"));
            }
            Ok(n)
        })
        .collect::<EngineResult<Vec<_>>>()?;
    if xy[2] <= xy[0] || xy[3] <= xy[1] {
        return Err(bad("empty widget rectangle"));
    }
    let mut probe = Document::with_version("1.7");
    let pages = probe.new_object_id();
    let page = probe.new_object_id();
    let field = probe.new_object_id();
    let flags = if model.combo { 1 << 17 } else { 0 }
        | if model.editable { 1 << 18 } else { 0 }
        | if model.multiple { 1 << 21 } else { 0 }
        | if model.sorted { 1 << 19 } else { 0 };
    let options = model
        .options
        .iter()
        .map(|o| {
            Object::Array(vec![
                lopdf::text_string(&o.export_value),
                lopdf::text_string(&o.display),
            ])
        })
        .collect::<Vec<_>>();
    let mut widget_dict = dictionary! {"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Ch","T"=>Object::string_literal("ChoiceProbe"),"P"=>page,"Ff"=>flags,"Opt"=>options,"TI"=>model.top_index as i64};
    let copied_font;
    {
        let mut copy = Copier::new(source, &mut probe);
        copied_font = copy.copy(font)?;
        widget_dict.set("DA", copy.copy(da)?);
        // Absence and explicit null differ: a missing combo V can display DV.
        // Preserve staged values/indices/defaults exactly, including that fallback.
        for key in [b"V".as_slice(), b"I", b"DV"] {
            if let Some(v) = ctx.raw_attribute(key)? {
                widget_dict.set(key.to_vec(), copy.copy(v)?);
            }
        }
        for key in [b"Rect".as_slice(), b"BS", b"Border", b"F"] {
            if let Ok(v) = w.get(key) {
                widget_dict.set(key.to_vec(), copy.copy(v)?);
            }
        }
        if let Ok(mk) = w.get(b"MK") {
            let mk = dict(source, mk)?;
            let mut selected = Dictionary::new();
            for key in [b"R".as_slice(), b"BG", b"BC"] {
                if let Ok(v) = mk.get(key) {
                    selected.set(key.to_vec(), copy.copy(v)?);
                }
            }
            widget_dict.set("MK", selected);
        }
        if let Some(q) = inherited(source, widget, b"Q")?.or(form.get(b"Q").ok()) {
            widget_dict.set("Q", copy.copy(q)?);
        }
    }
    probe.objects.insert(field, widget_dict.into());
    // All codec widgets share the one copied DR graph. Their large private
    // frames avoid incidental fit constraints; these frames are never published.
    let codec_da = Content {
        operations: vec![
            Operation::new("Tf", vec![Object::Name(font_name.clone()), 12.into()]),
            Operation::new("g", vec![0.into()]),
        ],
    }
    .encode()
    .map_err(bad)?;
    let mut fields = vec![Object::Reference(field)];
    for (index, range) in batches.iter().enumerate() {
        let options = required[range.clone()]
            .iter()
            .map(|s| lopdf::text_string(s))
            .collect::<Vec<_>>();
        let id = probe.add_object(dictionary! {
            "Type"=>"Annot", "Subtype"=>"Widget", "FT"=>"Ch", "P"=>page,
            "T"=>Object::string_literal(format!("ChoiceCodec{index}")),
            "Rect"=>vec![0.into(),0.into(),65536.into(),256.into()],
            "DA"=>Object::string_literal(codec_da.clone()), "Opt"=>options,
            "Ff"=>0, "TI"=>0, "V"=>Object::Null, "I"=>Vec::<Object>::new(),
        });
        fields.push(id.into());
    }
    probe.objects.insert(page,dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![(xy[0].min(0.)-10.).into(),(xy[1].min(0.)-10.).into(),(xy[2].max(0.)+10.).into(),(xy[3].max(0.)+10.).into()],"Annots"=>fields.clone(),"Resources"=>Dictionary::new()}.into());
    probe.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into(),
    );
    let mut probe_fonts = Dictionary::new();
    probe_fonts.set(font_name.clone(), copied_font);
    let catalog=probe.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages,"AcroForm"=>dictionary!{"Fields"=>fields,"DR"=>dictionary!{"Font"=>probe_fonts},"NeedAppearances"=>true}});
    probe.trailer.set("Root", catalog);
    let mut writer = crate::pdf_output::BoundedWriter::new(PROBE_LIMIT);
    let saved = probe.save_to(&mut writer);
    let bytes = writer.finish()?;
    saved.map_err(bad)?;
    drop(probe);
    let native = crate::form_fill::NativeForm::open(bindings, &bytes)?;
    let generated = native.snapshot_private(PROBE_LIMIT)?;
    drop(native);
    let mut document = Document::load_mem_with_options(
        &generated,
        lopdf::LoadOptions::with_max_decompressed_size(PROBE_LIMIT),
    )
    .map_err(bad)?;
    let p = *document
        .get_pages()
        .values()
        .next()
        .ok_or_else(|| bad("generated page is missing"))?;
    let annots = resolve(
        &document,
        document
            .get_dictionary(p)
            .map_err(bad)?
            .get(b"Annots")
            .map_err(bad)?,
    )?
    .as_array()
    .map_err(bad)?;
    if annots.len() != batches.len() + 1 {
        return Err(bad("unexpected generated widget ownership"));
    }
    let mut graph_proof = Equivalence::default();
    let mut verified_fonts = HashSet::new();
    let mut main = None;
    for (index, annot) in annots.iter().enumerate() {
        let widget = dict(&document, annot)?;
        let expected_name = if index == 0 {
            "ChoiceProbe".to_owned()
        } else {
            format!("ChoiceCodec{}", index - 1)
        };
        if resolve(&document, widget.get(b"T").map_err(bad)?)?
            .as_str()
            .map_err(bad)?
            != expected_name.as_bytes()
        {
            return Err(bad("unexpected generated widget ordering"));
        }
        let appearance = normal(&document, widget)?;
        let (content, names) = generated_content(
            source,
            font,
            &font_name,
            &document,
            appearance,
            &mut graph_proof,
            &mut verified_fonts,
        )?;
        let expected = if index == 0 {
            displayed.as_slice()
        } else {
            &required[batches[index - 1].clone()]
        };
        codec::verify_rows(&content, &text_proof, &names, expected)?;
        if index == 0 {
            main = Some((appearance.clone(), names));
        }
    }
    let main_field = annots[0].clone();
    // Discard the audit graph before a caller can copy from the returned doc.
    document
        .get_dictionary_mut(p)
        .map_err(bad)?
        .set("Annots", vec![main_field.clone()]);
    let form = dict(
        &document,
        document
            .catalog()
            .map_err(bad)?
            .get(b"AcroForm")
            .map_err(bad)?,
    )?;
    let mut form = form.clone();
    form.set("Fields", vec![main_field]);
    document.catalog_mut().map_err(bad)?.set("AcroForm", form);
    document.prune_objects();
    let (appearance, names) = main.ok_or_else(|| bad("missing generated appearance"))?;
    Ok(GeneratedChoiceAppearance {
        document,
        appearance,
        font_names: names,
    })
}

fn generated_content(
    source: &Document,
    font: &Object,
    font_name: &[u8],
    document: &Document,
    appearance: &Stream,
    proof: &mut Equivalence,
    verified_fonts: &mut HashSet<ObjectId>,
) -> EngineResult<(Content, BTreeMap<Vec<u8>, Vec<u8>>)> {
    let contents = appearance
        .get_plain_content_with_limit(256 * 1024)
        .map_err(bad)?;
    let content = Content::decode_strict(&contents).map_err(bad)?;
    if content.operations.len() > 16384 {
        return Err(bad("generated appearance has too many operations"));
    }
    let resources = dict(&document, appearance.dict.get(b"Resources").map_err(bad)?)?;
    if resources
        .iter()
        .any(|(k, _)| !matches!(k.as_slice(), b"Font" | b"ProcSet"))
    {
        return Err(bad("unexpected generated appearance resources"));
    }
    let generated_fonts =
        fonts(&document, resources)?.ok_or_else(|| bad("generated font resources are missing"))?;
    let mut names = BTreeMap::new();
    for (name, new_font) in generated_fonts.iter() {
        // Source font and generated document are fixed for this entire audit.
        // Cache only successful indirect roots, never failed/partial traversals.
        // The same shared font program must not consume the budget per batch.
        let (id, _) = document.dereference(new_font).map_err(bad)?;
        if !id.is_some_and(|id| verified_fonts.contains(&id))
            && !proof.check(source, font, &document, new_font)?
        {
            return Err(bad(
                "native choice text generation substituted or changed the original font",
            ));
        }
        if let Some(id) = id {
            verified_fonts.insert(id);
        }
        names.insert(name.clone(), font_name.to_vec());
    }
    for op in &content.operations {
        if op.operator == "Tf"
            && (op.operands.len() != 2
                || !names.contains_key(op.operands[0].as_name().map_err(bad)?))
        {
            return Err(bad("generated text has an unbound font"));
        }
    }
    Ok((content, names))
}
