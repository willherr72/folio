//! Isolate an invocation by cloning its stream/resource chain. Flattening is used
//! only as a private validation instrument; published pages retain their forms.
use super::*;
use lopdf::{
    content::{Content, Operation},
    dictionary, Dictionary, Document, Object, Stream, StringFormat,
};
#[path = "form_geometry.rs"]
mod geometry;
use geometry::{compose, ClipBox};
#[path = "form_text_state.rs"]
mod text_state;
use text_state::TextState;
#[path = "form_text_flow.rs"]
mod text_flow;
use text_flow::{SelectedFont, TextFlow};
#[path = "form_text_advance.rs"]
mod advance;
use std::sync::Arc;
#[path = "form_paragraph_projection.rs"]
mod form_paragraph_projection;
#[cfg(test)]
#[path = "form_paragraph_projection_tests.rs"]
mod form_paragraph_projection_tests;
const LIMIT: usize = 16 * 1024 * 1024;
const DEPTH: usize = 12;
#[derive(Clone)]
struct Node {
    operations: Vec<Operation>,
    resources: Dictionary,
    stream: Option<Stream>,
    children: Vec<(usize, Node)>,
}
#[derive(Clone)]
struct Occurrence {
    path: Vec<usize>,
    encoded: Vec<u8>,
    show: Operation,
    boxes: Vec<ClipBox>,
    text_state: TextState,
    font: Arc<SelectedFont>,
    following_depends: bool,
}
fn glyph_show(operation: &Operation) -> bool {
    matches!(operation.operator.as_str(), "Tj" | "TJ" | "'" | "\"")
        && !positioned_text::cursor_only(operation)
}
struct Model {
    pdf: Document,
    page: lopdf::ObjectId,
    root: Node,
    occurrences: Vec<Occurrence>,
    flat: Vec<u8>,
    proofs: FontProofs,
    flat_operations: Vec<Operation>,
    flat_fonts: Dictionary,
    root_invocations: HashMap<usize, std::ops::Range<usize>>,
}
fn err(error: impl std::fmt::Display) -> EngineError {
    invalid(&format!(
        "The form resources or content are malformed: {error}"
    ))
}
fn dictionary_ref<'a>(pdf: &'a Document, value: &'a Object) -> EngineResult<&'a Dictionary> {
    pdf.dereference(value)
        .map_err(err)?
        .1
        .as_dict()
        .map_err(err)
}
fn dictionary_value(
    pdf: &Document,
    value: &Object,
    copies: &mut crate::form_graph::CopyBudget,
) -> EngineResult<Dictionary> {
    let dict = dictionary_ref(pdf, value)?;
    copies.dictionary(dict)?;
    Ok(dict.clone())
}
fn resource<'a>(
    pdf: &'a Document,
    resources: &'a Dictionary,
    category: &[u8],
    name: &[u8],
) -> EngineResult<&'a Object> {
    dictionary_ref(pdf, resources.get(category).map_err(err)?)?
        .get(name)
        .map_err(err)
}
fn inherited_resources(pdf: &Document, page: lopdf::ObjectId) -> EngineResult<&Dictionary> {
    let mut current = page;
    let mut seen = HashSet::new();
    for _ in 0..64 {
        if !seen.insert(current) {
            return Err(invalid("Cyclic page resources."));
        }
        let dict = pdf.get_dictionary(current).map_err(err)?;
        if let Ok(value) = dict.get(b"Resources") {
            return dictionary_ref(pdf, value);
        }
        current = dict
            .get(b"Parent")
            .and_then(Object::as_reference)
            .map_err(err)?;
    }
    Err(invalid("Page resources exceed the nesting limit."))
}
fn decode(data: &[u8]) -> EngineResult<Vec<Operation>> {
    let operations = Content::decode_strict(data).map_err(err)?.operations;
    if operations.len() > 100_000 {
        return Err(invalid("Form operation limit exceeded."));
    }
    Ok(operations)
}
fn read_node(
    pdf: &Document,
    operations: Vec<Operation>,
    resources: &Dictionary,
    stream: Option<&Stream>,
    active: &mut HashSet<lopdf::ObjectId>,
    depth: usize,
    budget: &mut usize,
    copies: &mut crate::form_graph::CopyBudget,
) -> EngineResult<Node> {
    if depth > DEPTH {
        return Err(invalid("Form nesting exceeds the editing depth limit."));
    }
    *budget = budget
        .checked_add(operations.len())
        .ok_or_else(|| invalid("Form operation limit exceeded."))?;
    if *budget > 100_000 {
        return Err(invalid("Form operation limit exceeded."));
    }
    copies.dictionary(resources)?;
    if let Some(stream) = stream {
        copies.stream(stream)?;
    }
    let mut children = vec![];
    for (index, op) in operations.iter().enumerate() {
        if op.operator != "Do" {
            continue;
        }
        let [name] = op.operands.as_slice() else {
            return Err(err("Do"));
        };
        let value = resource(pdf, resources, b"XObject", name.as_name().map_err(err)?)?;
        let id = value.as_reference().map_err(err)?;
        if !active.insert(id) {
            return Err(invalid("Cyclic form invocations cannot be edited."));
        }
        let child = pdf
            .get_object(id)
            .and_then(Object::as_stream)
            .map_err(err)?;
        if child.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {
            return Err(invalid("Form editing currently supports text-only pages; image and graphic objects are not supported."));
        }
        for key in [
            b"Group".as_slice(),
            b"OC",
            b"Ref",
            b"StructParent",
            b"StructParents",
        ] {
            if child.dict.has(key) {
                return Err(invalid(
                    "Tagged, optional, or transparency-group forms cannot be edited.",
                ));
            }
        }
        let r = match child.dict.get(b"Resources") {
            Ok(v) => dictionary_ref(pdf, v)?,
            Err(_) => resources,
        };
        let bytes = if child.dict.has(b"Filter") {
            child.decompressed_content_with_limit(LIMIT).map_err(err)?
        } else {
            if child.content.len() > LIMIT {
                return Err(invalid("Form decoded-byte limit exceeded."));
            }
            child.content.clone()
        };
        children.push((
            index,
            read_node(
                pdf,
                decode(&bytes)?,
                r,
                Some(child),
                active,
                depth + 1,
                budget,
                copies,
            )?,
        ));
        active.remove(&id);
    }
    Ok(Node {
        operations,
        resources: resources.clone(),
        stream: stream.cloned(),
        children,
    })
}
fn numbers(values: &[Object], count: usize) -> EngineResult<Vec<f32>> {
    if values.len() != count {
        return Err(err("number count"));
    }
    values
        .iter()
        .map(|v| {
            v.as_float().map_err(err).and_then(|n| {
                if n.is_finite() && n.abs() <= 1_000_000.0 {
                    Ok(n)
                } else {
                    Err(invalid("Form transform exceeds safe bounds."))
                }
            })
        })
        .collect()
}
fn matrix(values: &[Object]) -> EngineResult<[f32; 6]> {
    let n = numbers(values, 6)?;
    geometry::checked([n[0], n[1], n[2], n[3], n[4], n[5]])
}
fn font_proof(
    pdf: &Document,
    resources: &Dictionary,
    name: &[u8],
    copies: &mut crate::form_graph::CopyBudget,
    proofs: &FontProofs,
) -> EngineResult<Object> {
    let value = resource(pdf, resources, b"Font", name)?;
    let font = dictionary_ref(pdf, value)?;

    if value.as_reference().is_ok_and(|id| proofs.allows_resource(id)) {
        return Ok(value.clone());
    }

    if font.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Type1")
        || !font
            .get(b"BaseFont")
            .and_then(Object::as_name)
            .ok()
            .and_then(|n| std::str::from_utf8(n).ok())
            .is_some_and(base_font)
    {
        return Err(invalid("Form text requires a standard Latin font or a verified embedded TrueType font."));
    }
    for key in [
        b"FontDescriptor".as_slice(),
        b"Widths",
        b"ToUnicode",
        b"FirstChar",
        b"LastChar",
    ] {
        if font.has(key) {
            return Err(invalid(
                "Custom or embedded form font mappings cannot be proved safely.",
            ));
        }
    }
    standard_font::StandardFont::read(pdf, font)?;
    if value.as_reference().is_err() {
        copies.dictionary(font)?;
    }
    Ok(value.clone())
}
fn flatten(
    pdf: &Document,
    node: &Node,
    prefix: &[usize],
    mut transform: [f32; 6],
    mut text_state: TextState,
    mut active_font: Option<Arc<SelectedFont>>,
    mut boxes: Vec<ClipBox>,
    out: &mut Vec<Operation>,
    fonts: &mut Dictionary,
    occurrences: &mut Vec<Occurrence>,
    copies: &mut crate::form_graph::CopyBudget,
    proofs: &FontProofs,
    root_invocations: &mut HashMap<usize, std::ops::Range<usize>>,
) -> EngineResult<()> {
    if let Some(stream) = &node.stream {
        if let Ok(m) = stream.dict.get(b"Matrix") {
            let operands = m.as_array().map_err(err)?;
            transform = compose(transform, matrix(operands)?)?;
            out.push(Operation::new("cm", operands.clone()));
        }
        let b = numbers(
            stream
                .dict
                .get(b"BBox")
                .and_then(Object::as_array)
                .map_err(err)?,
            4,
        )?;
        boxes.push(ClipBox::new([b[0], b[1], b[2], b[3]], transform)?);
    }
    let mut stack = vec![];
    let mut flow = TextFlow::default();
    let mut index = 0;
    for (opindex, op) in node.operations.iter().enumerate() {
        let mut op = op.clone();
        if flow.position(&op, &mut text_state)? {
            out.push(op);
            continue;
        }
        match op.operator.as_str() {
            "q" => {
                if flow.in_text() || stack.len() >= 64 {
                    return Err(invalid("Unsupported nested graphics/text state."));
                }
                stack.push((transform, text_state, active_font.clone()));
            }
            "Q" => {
                if flow.in_text() {
                    return Err(invalid(
                        "Graphics state inside text objects is unsupported.",
                    ));
                }
                (transform, text_state, active_font) = stack
                    .pop()
                    .ok_or_else(|| invalid("Unbalanced form graphics state."))?;
            }
            "cm" => {
                if flow.in_text() {
                    return Err(invalid("Graphics transforms inside text are unsupported."));
                }
                transform = compose(transform, matrix(&op.operands)?)?;
            }
            "Tf" => {
                let [name, size] = op.operands.as_slice() else {
                    return Err(err("Tf"));
                };
                let n = numbers(std::slice::from_ref(size), 1)?;
                if n[0] <= 0.0 || n[0] > 1000.0 {
                    return Err(invalid("Form font size is outside safe bounds."));
                }
                let font = font_proof(pdf, &node.resources, name.as_name().map_err(err)?, copies, proofs)?;
                let alias = format!("FolioFlat{}", fonts.len());
                // The flattened dictionary and the retained selection each own
                // one direct copy. Occurrences share the latter through Arc.
                if let Object::Dictionary(dictionary) = &font { copies.dictionary(dictionary)?; }
                active_font = Some(Arc::new(SelectedFont { resource: font.clone(), size: size.clone() }));
                fonts.set(alias.as_bytes(), font);
                op.operands[0] = Object::Name(alias.into_bytes());
            }
            "Tj" | "TJ" | "'" | "\"" => {
                let Some(encoded) = flow.show(&op, &mut text_state)? else {
                    out.push(op);
                    continue;
                };
                let font = active_font.clone().ok_or_else(|| invalid("Form text requires a verified active font."))?;
                let mut path = prefix.to_vec();
                path.push(index);
                occurrences.push(Occurrence {
                    path,
                    encoded,
                    show: op.clone(),
                    boxes: boxes.clone(),
                    text_state,
                    font,
                    following_depends: advance::following_depends(&node.operations[opindex+1..]),
                });
                index += 1;
            }
            "Do" => {
                if flow.in_text() {
                    return Err(invalid(
                        "Form invocations inside text objects are unsupported.",
                    ));
                }
                let child = &node
                    .children
                    .iter()
                    .find(|(i, _)| *i == opindex)
                    .ok_or_else(|| err("child"))?
                    .1;
                let mut path = prefix.to_vec();
                path.push(index);
                let start = out.len();
                out.push(Operation::new("q", vec![]));
                flatten(
                    pdf,
                    child,
                    &path,
                    transform,
                    text_state,
                    active_font.clone(),
                    boxes.clone(),
                    out,
                    fonts,
                    occurrences,
                    copies,
                    proofs,
                    root_invocations,
                )?;
                out.push(Operation::new("Q", vec![]));
                if prefix.is_empty() { root_invocations.insert(index, start..out.len()); }
                index += 1;
                continue;
            }
            "g" | "G" | "rg" | "RG" => {
                let n = numbers(
                    &op.operands,
                    if matches!(op.operator.as_str(), "g" | "G") {
                        1
                    } else {
                        3
                    },
                )?;
                if n.iter().any(|v| *v < 0.0 || *v > 1.0) {
                    return Err(err("color"));
                }
            }
            "Tc" | "Tw" | "Tz" | "Ts" | "TL" | "Tr" => {
                text_state.apply(&op)?;
            }
            _ => {
                return Err(invalid(&format!(
                    "Form editing does not support the {} operator or its inherited state.",
                    op.operator
                )))
            }
        }
        out.push(op);
    }
    flow.finish()?;
    if !stack.is_empty() {
        return Err(invalid("Unbalanced form text or graphics state."));
    }
    Ok(())
}
fn save(pdf: &mut Document) -> EngineResult<Vec<u8>> {
    let mut bytes = vec![];
    pdf.save_to(&mut bytes).map_err(err)?;
    Ok(bytes)
}
impl Model {
    fn read(bytes: &[u8]) -> EngineResult<Self> {
        Self::read_mode(bytes,false)
    }
    fn read_mode(bytes: &[u8],paragraph_projection:bool) -> EngineResult<Self> {
        // Bounds font program allocation before any native font-data accessor.
        let proofs = FontProofs::read(bytes)?;
        let pdf = Document::load_mem_with_options(
            bytes,
            lopdf::LoadOptions::with_max_decompressed_size(LIMIT),
        )
        .map_err(err)?;
        for object in pdf.objects.values() {
            let d = match object {
                Object::Dictionary(d) => Some(d),
                Object::Stream(s) => Some(&s.dict),
                _ => None,
            };
            if d.is_some_and(|d| {
                [
                    b"StructTreeRoot".as_slice(),
                    b"MarkInfo",
                    b"OCProperties",
                    b"OC",
                    b"StructParents",
                ]
                .iter()
                .any(|k| d.has(k))
            }) {
                return Err(invalid(
                    "Tagged or optional-content pages cannot use form text editing.",
                ));
            }
        }
        let pages = pdf.get_pages();
        if pages.len() != 1 {
            return Err(invalid("Form editing requires an isolated page."));
        }
        let page = *pages.values().next().unwrap();
        let data = pdf.get_page_content_with_limit(page, LIMIT).map_err(err)?;
        let mut copies = crate::form_graph::CopyBudget::default();
        let root = read_node(
            &pdf,
            decode(&data)?,
            inherited_resources(&pdf, page)?,
            None,
            &mut HashSet::new(),
            0,
            &mut 0,
            &mut copies,
        )?;
        let mut operations = vec![];
        let mut fonts = Dictionary::new();
        let mut occurrences = vec![];
        let mut root_invocations = HashMap::new();
        flatten(
            &pdf,
            &root,
            &[],
            [1., 0., 0., 1., 0., 0.],
            TextState::default(),
            None,
            vec![],
            &mut operations,
            &mut fonts,
            &mut occurrences,
            &mut copies,
            &proofs,
            &mut root_invocations,
        )?;
        if occurrences.len() > 5000 {
            return Err(invalid("Form text object limit exceeded."));
        }
        let mut flat = pdf.clone();
        let flat_content = Content { operations };
        let content = flat.add_object(Stream::new(
            dictionary! {},
            flat_content.encode().map_err(err)?,
        ));
        let d = flat.get_dictionary_mut(page).map_err(err)?;
        d.set("Contents", content);
        let retained_fonts=if paragraph_projection {
            copies.dictionary(&fonts)?;
            let retained=fonts.clone();d.set("Resources",dictionary!{"Font"=>fonts});retained
        }else{d.set("Resources",dictionary!{"Font"=>fonts});Dictionary::new()};
        let flat = save(&mut flat)?;
        Ok(Self {
            pdf,
            page,
            root,
            occurrences,
            flat,
            proofs,
            flat_operations: if paragraph_projection{flat_content.operations}else{vec![]},
            flat_fonts: retained_fonts,
            root_invocations: if paragraph_projection{root_invocations}else{HashMap::new()},
        })
    }
    fn replace(&self, path: &[usize], replacement: &Operation) -> EngineResult<Vec<u8>> {
        self.replace_with_font(path, replacement, None, None)
    }
    fn replace_with_font(&self, path: &[usize], replacement: &Operation, font: Option<&FontAsset>, adjustment:Option<f32>) -> EngineResult<Vec<u8>> {
        let selected = self.occurrences.iter().find(|o| o.path == path)
            .ok_or_else(|| invalid("The selected form object path is stale."))?;
        let mut pdf = self.pdf.clone();
        let font = font.map(|asset| persistence::install_font_resource(&mut pdf, asset)).transpose()?;
        let (content, resources) = clone_chain(
            &mut pdf,
            &self.root,
            path,
            replacement,
            font.as_ref(),
            &selected.font,
            adjustment,
            &mut crate::form_graph::CopyBudget::default(),
        )?;
        let id = pdf.add_object(Stream::new(dictionary! {}, content));
        let page = pdf.get_dictionary_mut(self.page).map_err(err)?;
        page.set("Contents", id);
        page.set("Resources", resources);
        prune_unreachable(&mut pdf)?;
        save(&mut pdf)
    }
}
fn clone_chain(
    pdf: &mut Document,
    node: &Node,
    path: &[usize],
    replacement: &Operation,
    font: Option<&Object>,
    original_font: &SelectedFont,
    adjustment: Option<f32>,
    copies: &mut crate::form_graph::CopyBudget,
) -> EngineResult<(Vec<u8>, Dictionary)> {
    let mut ops = node.operations.clone();
    copies.dictionary(&node.resources)?;
    let mut resources = node.resources.clone();
    let mut object_index = 0;
    let mut found = false;
    let mut font_selection = None;
    let mut selected_index = None;
    for (opindex, op) in ops.iter_mut().enumerate() {
        if op.operator != "Do" && !glyph_show(op) {
            continue;
        }
        if Some(&object_index) == path.first() {
            if path.len() == 1 && glyph_show(op) {
                if let Some(font) = font {
                    let mut fonts = match resources.get(b"Font") {
                        Ok(value) => dictionary_value(pdf, value, copies)?,
                        Err(_) => Dictionary::new(),
                    };
                    let mut alias = b"FolioFormFont".to_vec();
                    while fonts.has(&alias) { alias.push(b'_'); }
                    fonts.set(alias.clone(), font.clone());
                    let mut restore = b"FolioFormRestore".to_vec();
                    while fonts.has(&restore) { restore.push(b'_'); }
                    if let Object::Dictionary(dictionary) = &original_font.resource { copies.dictionary(dictionary)?; }
                    fonts.set(restore.clone(), original_font.resource.clone());
                    resources.set("Font", fonts);
                    font_selection = Some((opindex, alias, restore));
                }
                *op = replacement.clone();
                selected_index = Some(opindex);
            } else if path.len() > 1 && op.operator == "Do" {
                let child = &node
                    .children
                    .iter()
                    .find(|(i, _)| *i == opindex)
                    .ok_or_else(|| err("child"))?
                    .1;
                let (bytes, child_resources) =
                    clone_chain(pdf, child, &path[1..], replacement, font, original_font, adjustment, copies)?;
                let stream = child.stream.as_ref().ok_or_else(|| err("stream"))?;
                copies.stream(stream)?;
                let mut stream = stream.clone();
                stream.set_plain_content(bytes);
                if font.is_some() || path.len() > 2 || stream.dict.has(b"Resources") {
                    stream.dict.set("Resources", child_resources);
                }
                let id = pdf.add_object(stream);
                let mut xobjects =
                    dictionary_value(pdf, resources.get(b"XObject").map_err(err)?, copies)?;
                let mut alias = String::from("FolioForm");
                while xobjects.has(alias.as_bytes()) {
                    alias.push('_');
                }
                xobjects.set(alias.as_bytes(), id);
                let old_name = op.operands[0].as_name().map_err(err)?;
                // Explicit intermediate scopes are required by source admission;
                // missing leaf scopes consume fonts only, so no descendant can
                // inherit this XObject alias. Retain it if another local Do uses it.
                if !node.operations.iter().enumerate().any(|(i, other)| {
                    i != opindex
                        && other.operator == "Do"
                        && other.operands.first().and_then(|v| v.as_name().ok()) == Some(old_name)
                }) {
                    xobjects.remove(old_name);
                }
                resources.set("XObject", xobjects);
                op.operands = vec![Object::Name(alias.into_bytes())];
            } else {
                return Err(invalid("The selected form object path is stale."));
            }
            found = true;
            break;
        }
        object_index += 1;
    }
    if !found {
        return Err(invalid("The selected form object path is stale."));
    }
    if let (Some(index),Some(adjustment))=(selected_index,adjustment) {
        advance::append(&mut ops,index,adjustment)?;
    }
    if let Some((index, alias, restore)) = font_selection {
        // Font state can originate in an earlier text object or an ancestor.
        // Select and restore around this show only; never rewrite its shared Tf.
        ops.insert(index, Operation::new("Tf", vec![Object::Name(alias), original_font.size.clone()]));
        // Numeric TJ uses size/scale, not glyph widths. Both fonts have the same
        // raw size here, so keep the correction next to its show across re-edits.
        let restore_index = if ops.get(index + 2).is_some_and(positioned_text::cursor_only) {
            index + 3
        } else { index + 2 };
        ops.insert(restore_index, Operation::new("Tf", vec![Object::Name(restore), original_font.size.clone()]));
        advance::compact_font_selections(&mut ops, &mut resources, node)?;
    }
    Ok((
        Content { operations: ops }.encode().map_err(err)?,
        resources,
    ))
}
fn verify_tree(page: &PdfPage<'_>, root: &Node) -> EngineResult<()> {
    fn node(objects: Vec<PdfPageObject<'_>>, expected: &Node, depth: usize) -> EngineResult<()> {
        if depth > DEPTH
            || objects.len()
                != expected
                    .operations
                    .iter()
                    .filter(|op| op.operator == "Do" || glyph_show(op))
                    .count()
        {
            return Err(invalid(
                "Form operators cannot be mapped uniquely to native object paths.",
            ));
        }
        let mut objects = objects.into_iter();
        for (i, op) in expected.operations.iter().enumerate() {
            match op.operator.as_str() {
                "Tj" | "TJ" | "'" | "\"" if glyph_show(op) => {
                    if objects.next().unwrap().as_text_object().is_none() {
                        return Err(err("native text"));
                    }
                }
                "Do" => {
                    let object = objects.next().unwrap();
                    let form = object
                        .as_x_object_form_object()
                        .ok_or_else(|| err("native form"))?;
                    let child = &expected
                        .children
                        .iter()
                        .find(|(n, _)| *n == i)
                        .ok_or_else(|| err("child"))?
                        .1;
                    node(form.iter().collect(), child, depth + 1)?;
                }
                _ => {}
            }
        }
        Ok(())
    }
    node(page.objects().iter().collect(), root, 0)
}
fn same_glyphs(a: &[Glyph], b: &[Glyph]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(a, b)| a.text == b.text && close_values(&a.position, &b.position))
}
// Flattening changes PDFium's generated inter-object separators. Compare only
// explicitly non-generated glyphs for this validation instrument. Actual page
// isolation and actual no-op cloning still compare the complete glyph sequence.
fn physical_glyphs(page: &PdfPage<'_>) -> EngineResult<Vec<Glyph>> {
    let text = page.text()?;
    let mut result = vec![];
    for character in text.chars().iter() {
        if character.is_generated()? {
            continue;
        }
        let rect = character.tight_bounds()?;
        let (x, y) = character.origin()?;
        result.push(Glyph {
            text: character.unicode_string().unwrap_or_default(),
            position: [
                x.value,
                y.value,
                rect.left().value,
                rect.bottom().value,
                rect.right().value,
                rect.top().value,
            ],
        });
    }
    Ok(result)
}
fn validate_flat(
    runtime: &WorkerRuntime,
    bytes: &[u8],
    model: &Model,
) -> EngineResult<Vec<RunSnapshot>> {
    let original = runtime.pdfium.load_pdf_from_byte_slice(bytes, None)?;
    let original = original.pages().get(0)?;
    verify_tree(&original, &model.root)?;
    let flat = runtime.pdfium.load_pdf_from_byte_slice(&model.flat, None)?;
    let flat = flat.pages().get(0)?;
    if !same_glyphs(&physical_glyphs(&original)?, &physical_glyphs(&flat)?)
        || !validation_rasters_match(&original,&flat)?
    {
        return Err(invalid(
            "Form clipping or inherited state cannot be preserved by the validation page.",
        ));
    }
    let mut runs = bound_snapshot(&flat, Some(&model.proofs), &model.flat)?;
    if runs.len() != model.occurrences.len() {
        return Err(invalid("Form text operators cannot be mapped uniquely."));
    }
    for (run, occurrence) in runs.iter_mut().zip(&model.occurrences) {
        let decoded = if run.embedded {
            run.proof.as_ref().ok_or_else(|| invalid("The embedded form font could not be verified."))?
                .decode(&occurrence.encoded)?
        } else if let Some(proof) = &run.standard {
            proof.decode(&occurrence.encoded)?
        } else {
            std::str::from_utf8(&occurrence.encoded).map_err(err)?.to_owned()
        };
        let valid = if run.embedded || run.standard.is_some() { embedded_font::latin(&decoded) } else { printable(&decoded) };
        // Source bytes own ordinary spaces; native extraction may collapse
        // repeated spaces or add separators. Compare the same word sequence,
        // then retain every verified encoded space in the snapshot and advance.
        if run.reason.is_some() || !valid
            || decoded.trim_matches(' ').trim() != decoded.trim_matches(' ')
            || !run.text.split(' ').filter(|part| !part.is_empty())
                .eq(decoded.split(' ').filter(|part| !part.is_empty())) {
            return Err(invalid(
                "Form text encoding or native glyph mapping is ambiguous.",
            ));
        }
        run.text = decoded;
        if let ("TJ", [Object::Array(values)]) = (occurrence.show.operator.as_str(), occurrence.show.operands.as_slice()) {
            let boundaries = text_show::boundaries(&run.text, |c| positioned_text::encoded(run, c))?;
            text_show::validate_segments(values, &occurrence.encoded, &boundaries)?;
        }
        let quad = flat.objects().get(run.index.try_into().map_err(|_| invalid("Form object index exceeds native limits."))?)?.bounds()?;
        let corners = [[quad.x1().value, quad.y1().value], [quad.x2().value, quad.y2().value],
            [quad.x3().value, quad.y3().value], [quad.x4().value, quad.y4().value]];
        if occurrence.boxes.iter().any(|b| !b.contains(&corners)) {
            return Err(invalid(
                "Form text extends outside an ancestor form bounding box.",
            ));
        }
    }
    Ok(runs)
}
// Nested forms and equivalent flattened text can differ by one coverage level at
// antialiased edges under a fractional raster scale. This exception is private to
// validation: actual page isolation, no-op cloning and publication remain exact.
// Require an exact second, higher-resolution rendering as well as matching glyphs.
fn only_edge_quantization(a:&image::RgbaImage,b:&image::RgbaImage)->bool {
    a.dimensions()==b.dimensions() && a.pixels().zip(b.pixels()).all(|(a,b)| {
        a[3]==b[3] && (0..3).all(|i|a[i]==b[i] ||
            (a[i]>0 && a[i]<255 && b[i]>0 && b[i]<255 && a[i].abs_diff(b[i])==1))
    })
}
fn validation_rasters_match(a:&PdfPage<'_>,b:&PdfPage<'_>)->EngineResult<bool>{
    let first=raster(a)?;let second=raster(b)?;
    if first==second{return Ok(true);}
    if !only_edge_quantization(&first,&second){return Ok(false);}
    let render=|page:&PdfPage<'_>|->EngineResult<image::RgbaImage>{Ok(page.render_with_config(
        &PdfRenderConfig::new().set_target_width(1200).set_maximum_height(1920))?.as_image()?.to_rgba8())};
    Ok(render(a)?==render(b)?)
}
fn inside(a: &[f32; 4], b: &[f32; 4]) -> bool {
    a[0] >= b[0] - EPSILON
        && a[1] >= b[1] - EPSILON
        && a[2] <= b[2] + EPSILON
        && a[3] <= b[3] + EPSILON
}
fn annotation_bounds(model:&Model)->EngineResult<Vec<[f32;4]>>{
    let page=model.pdf.get_dictionary(model.page).map_err(err)?;
    let Ok(value)=page.get(b"Annots") else{return Ok(vec![]);};
    let annotations=model.pdf.dereference(value).map_err(err)?.1.as_array().map_err(err)?;
    if annotations.len()>5000{return Err(invalid("Too many annotations for form text editing."));}
    annotations.iter().map(|value|{
        let dict=dictionary_ref(&model.pdf,value)?;
        let value=dict.get(b"Rect").map_err(|_|invalid("An annotation has no valid interaction rectangle."))?;
        let rect=model.pdf.dereference(value).map_err(err)?.1.as_array().map_err(err)?;
        let v=numbers(rect,4)?;
        if v.iter().any(|n|!n.is_finite()){return Err(invalid("An annotation has nonfinite bounds."));}
        Ok([v[0].min(v[2]),v[1].min(v[3]),v[0].max(v[2]),v[1].max(v[3])])
    }).collect()
}
fn source_has_forms(bytes: &[u8]) -> EngineResult<bool> {
    let pdf = Document::load_mem_with_options(
        bytes,
        lopdf::LoadOptions::with_max_decompressed_size(LIMIT),
    )
    .map_err(err)?;
    Ok(pdf
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .any(|stream| {
            stream
                .dict
                .get(b"Subtype")
                .ok()
                .and_then(|v| pdf.dereference(v).ok())
                .and_then(|(_, v)| v.as_name().ok())
                == Some(b"Form")
        }))
}
fn source_admission(bytes: &[u8], annotations:bool) -> EngineResult<()> {
    let pdf = Document::load_mem_with_options(
        bytes,
        lopdf::LoadOptions::with_max_decompressed_size(LIMIT),
    )
    .map_err(err)?;
    for object in pdf.objects.values() {
        let dict = match object {
            Object::Dictionary(d) => Some(d),
            Object::Stream(stream) => Some(&stream.dict),
            _ => None,
        };
        if let Some(d) = dict {
            if [
                b"StructTreeRoot".as_slice(),
                b"MarkInfo",
                b"OCProperties",
                b"OC",
                b"StructParents",
                b"StructParent",
            ]
            .iter()
            .any(|k| d.has(k))
            {
                return Err(invalid(
                    "Tagged or optional-content source documents cannot use form text editing.",
                ));
            }
            if !annotations && (d.has(b"Annots") || d.has(b"AP")) {
                return Err(invalid(
                    "Form text editing on source documents with annotations is unsupported.",
                ));
            }
        }
        if let Object::Stream(stream) = object {
            let is_form = stream
                .dict
                .get(b"Subtype")
                .ok()
                .and_then(|v| pdf.dereference(v).ok())
                .and_then(|(_, v)| v.as_name().ok())
                == Some(b"Form");
            if is_form && !stream.dict.has(b"Resources") {
                let data = stream.get_plain_content_with_limit(LIMIT).map_err(err)?;
                // Conservatively inspect tokens without parsing an unreferenced stream. The
                // actual invocation graph has already passed the guarded content parser.
                let invokes = data
                    .split(|c| c.is_ascii_whitespace() || *c == 0 || b"()<>[]{}/%".contains(c))
                    .any(|token| token == b"Do");
                if invokes {
                    return Err(invalid("An intermediate form without its own Resources cannot be isolated while preserving reader semantics."));
                }
            }
        }
    }
    Ok(())
}
fn prune_unreachable(pdf: &mut Document) -> EngineResult<()> {
    let mut seen = HashSet::new();
    let mut stack: Vec<&Object> = pdf.trailer.iter().map(|(_, v)| v).collect();
    let mut visits = 0;
    while let Some(value) = stack.pop() {
        visits += 1;
        if visits > 1_000_000 {
            return Err(invalid(
                "Form resource graph exceeds the object visit limit.",
            ));
        }
        match value {
            Object::Reference(id) => {
                if seen.insert(*id) {
                    stack.push(
                        pdf.objects
                            .get(id)
                            .ok_or_else(|| err("Missing indirect resource object"))?,
                    );
                }
            }
            Object::Array(a) => stack.extend(a),
            Object::Dictionary(d) => stack.extend(d.iter().map(|(_, v)| v)),
            Object::Stream(s) => stack.extend(s.dict.iter().map(|(_, v)| v)),
            _ => {}
        }
    }
    pdf.objects.retain(|id, _| seen.contains(id));
    Ok(())
}
const WORD_SPACING_FONT_REASON: &str = "Keep the original font to preserve this text’s word spacing.";
fn can_substitute(run: &RunSnapshot, occurrence: &Occurrence) -> bool {
    // Tw affects single-byte character code 32, but not Identity-H's two-byte
    // code 0020. Installing our Identity-H substitute would drop that advance.
    occurrence.text_state.word_spacing == 0.0
        || run.proof.as_ref().is_some_and(|proof| !proof.single_byte())
}
impl WorkerRuntime {
    pub(in crate::engine) fn form_source_admission(&self,source_id:&str,annotations:bool)->EngineResult<()> {
        // Recheck policy even on a hit. Both predicates below depend only on
        // this document's immutable bytes and the selected annotation mode.
        let source = self.editing_document(source_id)?;
        source.form_admission.check(annotations, || {
            // Prove the raw graph before native page copying/traversal.
            crate::form_graph::preflight(source.bytes()?.as_ref())?;
            source_admission(source.bytes()?.as_ref(),annotations)
        })
    }
    fn form_baseline(&self, source_id: &str, page_index: usize,annotations:bool) -> EngineResult<Vec<u8>> {
        self.form_source_admission(source_id,annotations)?;
        let copy = self.editable_page_copy(source_id, page_index)?;
        let bytes = if annotations {self.preserve_text_edit_comments(source_id,page_index,copy.save_to_bytes()?.into())?.to_vec()}else{copy.save_to_bytes()?};
        let source = self
            .editing_document(source_id)?
            .pdf()?
            .pages()
            .get(page_index_i32(page_index)?)?;
        let page = copy.pages().get(0)?;
        if raster(&source)? != raster(&page)?
            || !same_glyphs(&glyph_snapshot(&source)?, &glyph_snapshot(&page)?)
        {
            return Err(invalid(
                "Isolating this form page changes its text or appearance.",
            ));
        }
        Ok(bytes)
    }
    pub(in crate::engine) fn list_form_text_runs(
        &self,
        source_id: &str,
        page_index: usize,
    ) -> EngineResult<FormTextRuns> {
        self.list_transaction_form_text_runs(source_id,page_index,false)
    }
    pub(in crate::engine) fn list_transaction_form_text_runs(
        &self,source_id:&str,page_index:usize,annotations:bool,
    )->EngineResult<FormTextRuns>{
        if page_index >= self.editing_document(source_id)?.document.page_count() as usize {
            return Err(invalid("The requested page index is out of range."));
        }
        let inspect = || -> EngineResult<FormTextRuns> {
            if !source_has_forms(self.editing_document(source_id)?.bytes()?.as_ref())? {
                return Ok(FormTextRuns {
                    runs: vec![],
                    reason: None,
                });
            }
            let bytes = self.form_baseline(source_id, page_index,annotations)?;
            let model = Model::read(&bytes)?;
            let runs = validate_flat(self, &bytes, &model)?;
            let doc = self.pdfium.load_pdf_from_byte_slice(&bytes, None)?;
            let geometry = page_geometry(&doc.pages().get(0)?)?;
            let runs = runs
                .iter()
                .zip(&model.occurrences)
                .filter(|(_, o)| o.path.len() > 1)
                .map(|(r, o)| {
                    let a = geometry.pdf_to_displayed(r.bounds[0], r.bounds[1]);
                    let b = geometry.pdf_to_displayed(r.bounds[2], r.bounds[3]);
                    FormTextRun {
                        object_path: o.path.clone(),
                        text: r.text.clone(),
                        font_name: r.font.clone(),
                        font_display_name: r.proof.as_ref().and_then(|p|p.display_name.clone()),
                        is_embedded: r.embedded,
                        font_size: r.font_size,
                        display_font_size: r.display_font_size(),
                        bounds: AnnotationRect {
                            x: a.x.min(b.x),
                            y: a.y.min(b.y),
                            width: (a.x - b.x).abs(),
                            height: (a.y - b.y).abs(),
                        },
                        supported: true,
                        can_substitute: can_substitute(r, o),
                        font_substitution_reason: (!can_substitute(r, o))
                            .then(|| WORD_SPACING_FONT_REASON.to_owned()),
                        reason: None,
                    }
                })
                .collect();
            Ok(FormTextRuns { runs, reason: None })
        };
        match inspect() {
            Ok(v) => Ok(v),
            Err(EngineError::InvalidRequest(reason)) => Ok(FormTextRuns {
                runs: vec![],
                reason: Some(reason),
            }),
            Err(e) => Err(e),
        }
    }
    pub(in crate::engine) fn replace_form_text(
        &mut self,
        source_id: &str,
        page_index: usize,
        path: &[usize],
        expected_text: &str,
        replacement: &str,
        font_id: Option<&str>,
    ) -> EngineResult<DocumentInfo> {
        self.replace_transaction_form_text(source_id,page_index,path,expected_text,replacement,font_id,false)
    }
    pub(in crate::engine) fn replace_transaction_form_text(
        &mut self,source_id:&str,page_index:usize,path:&[usize],expected_text:&str,
        replacement:&str,font_id:Option<&str>,annotations:bool,
    )->EngineResult<DocumentInfo>{
        if path.len() < 2
            || path.len() > DEPTH + 1
            || !embedded_font::latin(replacement)
            || replacement.trim_matches(' ').trim() != replacement.trim_matches(' ')
        {
            return Err(invalid("Select a bounded form occurrence and enter nonblank single-line Latin text."));
        }
        let baseline = self.form_baseline(source_id, page_index,annotations)?;
        let model = Model::read(&baseline)?;
        let before = validate_flat(self, &baseline, &model)?;
        let selected = model
            .occurrences
            .iter()
            .position(|o| o.path == path)
            .ok_or_else(|| invalid("The selected form object path is stale."))?;
        if before[selected].text != expected_text {
            return Err(invalid(
                "The text changed since it was selected. Select the occurrence again.",
            ));
        }
        let substitute = font_id.map(|id| self.fonts.get(id)).transpose()?;
        let reuse = substitute.as_ref().filter(|font| before[selected].program.as_deref() == Some(font.info.id.as_str()))
            .and_then(|_| before[selected].proof.as_ref()).and_then(|proof| proof.encode(replacement).ok());
        let installed = if reuse.is_some() { None } else { substitute.as_deref() };
        // A verified source encoding can map characters absent from a subset's
        // Unicode cmap. Only a new Identity-H installation needs that cmap.
        if let Some(font) = installed { font.validate_text(replacement)?; }
        if installed.is_some() && !can_substitute(&before[selected], &model.occurrences[selected]) {
            return Err(invalid(WORD_SPACING_FONT_REASON));
        }
        let encoded = if let Some(encoded) = reuse {
            encoded
        } else if substitute.is_some() {
            replacement.encode_utf16().flat_map(u16::to_be_bytes).collect()
        } else if let Some(proof) = &before[selected].proof {
            proof.encode(replacement)?
        } else if let Some(proof) = &before[selected].standard {
            proof.encode(replacement)?
        } else if printable(replacement) {
            replacement.as_bytes().to_vec()
        } else {
            return Err(invalid("This standard form font supports printable ASCII only."));
        };
        let mut replacement_show = model.occurrences[selected].show.clone();
        match (replacement_show.operator.as_str(), replacement_show.operands.as_mut_slice()) {
            ("Tj" | "'", [value]) | ("\"", [_, _, value]) => *value = Object::String(encoded.clone(), StringFormat::Hexadecimal),
            ("TJ", [Object::Array(values)]) => {
                let source_boundaries = text_show::boundaries(&before[selected].text,
                    |c| positioned_text::encoded(&before[selected], c))?;
                let replacement_boundaries = text_show::boundaries(replacement, |c| {
                    if installed.is_some() { Ok(c.encode_utf16().flat_map(u16::to_be_bytes).collect()) }
                    else { positioned_text::encoded(&before[selected], c) }
                })?;
                *values = text_show::rewrite_array(values, &model.occurrences[selected].encoded,
                    &encoded, &source_boundaries, &replacement_boundaries, replacement == before[selected].text)?;
            }
            _ => return Err(invalid("The selected form text-show operator is malformed.")),
        }
        let noop = model.replace(path, &model.occurrences[selected].show)?;
        let original = self.pdfium.load_pdf_from_byte_slice(&baseline, None)?;
        let original_page = original.pages().get(0)?;
        let noop_doc = self.pdfium.load_pdf_from_byte_slice(&noop, None)?;
        let noop_page = noop_doc.pages().get(0)?;
        if !same_glyphs(
            &glyph_snapshot(&original_page)?,
            &glyph_snapshot(&noop_page)?,
        ) || raster(&original_page)? != raster(&noop_page)?
        {
            return Err(invalid(
                "Isolating the chosen form occurrence changes the page.",
            ));
        }
        let adjustment=model.occurrences[selected].following_depends.then(||
            advance::correction(&before[selected],&model.occurrences[selected],&replacement_show,replacement,installed)).transpose()?;
        let bytes: Arc<[u8]> = model.replace_with_font(path, &replacement_show, installed,adjustment)?.into();
        let after_model = Model::read(&bytes)?;
        let after = validate_flat(self, &bytes, &after_model)?;
        if before.len() != after.len()
            || model
                .occurrences
                .iter()
                .zip(&after_model.occurrences)
                .any(|(a, b)| a.path != b.path || a.text_state != b.text_state)
            || before.iter().zip(&after).enumerate().any(|(i, (a, b))| {
                if i == selected {
                    b.text != replacement || if let Some(font) = &substitute {
                        !same_layout_style(a, b) || b.program.as_deref() != Some(font.info.id.as_str())
                    } else { !same_style(a, b) }
                } else {
                    !same_run(a, b)
                }
            })
        {
            return Err(invalid(
                "The replacement changed another occurrence, font, or placement.",
            ));
        }
        let document = self
            .pdfium
            .load_pdf_from_reader(Cursor::new(bytes.clone()), None)?;
        let page = document.pages().get(0)?;
        let geometry = page_geometry(&page)?;
        let old = &before[selected];
        let changed = &after[selected];
        if !inside(
            &changed.bounds,
            &[geometry.left, geometry.bottom, geometry.right, geometry.top],
        ) {
            return Err(invalid(
                "The replacement would extend outside the visible page.",
            ));
        }
        for (i, other) in before.iter().enumerate() {
            if i != selected && adds_overlap(&old.bounds, &changed.bounds, &other.bounds) {
                return Err(invalid("The replacement would overlap neighboring text."));
            }
        }
        let original_raster = raster(&original_page)?;
        let changed_raster = raster(&page)?;
        if original_raster.dimensions() != changed_raster.dimensions() {
            return Err(invalid("The replacement changed page dimensions."));
        }
        let size = geometry.displayed_size();
        let sx = original_raster.width() as f32 / size.width;
        let sy = original_raster.height() as f32 / size.height;
        if annotations {
            // Protect interaction rectangles even for invisible links, plus native
            // rectangles (note icons may get native display geometry). Reserve the
            // permitted edit fringe and the neighboring annotation's own fringe.
            let padding=6.0/sx.min(sy);
            let padded=|b:&[f32;4]|[b[0]-padding,b[1]-padding,b[2]+padding,b[3]+padding];
            let old_bounds=padded(&old.bounds);let new_bounds=padded(&changed.bounds);
            let mut bounds=annotation_bounds(&model)?;
            for annotation in original_page.annotations().iter(){
                let rect=annotation.bounds()?;
                let b=[rect.left().value,rect.bottom().value,rect.right().value,rect.top().value];
                if b.iter().any(|v|!v.is_finite()){return Err(invalid("An annotation has invalid native bounds."));}
                bounds.push(b);
            }
            if bounds.iter().any(|b|adds_overlap(&old_bounds,&new_bounds,b)){
                return Err(invalid("The replacement would overlap a neighboring annotation or form field."));
            }
        }
        let a = geometry.pdf_to_displayed(
            old.bounds[0].min(changed.bounds[0]),
            old.bounds[1].min(changed.bounds[1]),
        );
        let b = geometry.pdf_to_displayed(
            old.bounds[2].max(changed.bounds[2]),
            old.bounds[3].max(changed.bounds[3]),
        );
        for (x, y, pixel) in original_raster.enumerate_pixels() {
            if pixel != changed_raster.get_pixel(x, y)
                && ((x as f32) < a.x.min(b.x) * sx - 3.0
                    || (x as f32) > a.x.max(b.x) * sx + 3.0
                    || (y as f32) < a.y.min(b.y) * sy - 3.0
                    || (y as f32) > a.y.max(b.y) * sy + 3.0)
            {
                return Err(invalid("The replacement changed surrounding page pixels."));
            }
        }
        if bytes.len() as u64 > MAX_SOURCE_BYTES {
            return Err(invalid(
                "The edited form page exceeds the source size limit.",
            ));
        }
        let pages = vec![geometry.displayed_size()];
        drop(page);
        let source = self.editing_document(source_id)?;
        let path = source.path.clone();
        let original_path = source.original_path.clone();
        let policy=source.policy.as_ref().map(|p|p.page(page_index)).transpose()?;
        let protection=policy.as_ref().map(SourcePolicy::info);
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document.pdf")
            .to_string();
        let id = Uuid::new_v4().to_string();
        let retained = self.source_store.capture(bytes.clone())?;
        self.documents.insert(
            id.clone(),
            OpenDocument {
                physical_units:OnceLock::from(physical_units::read(&bytes,document.pages().len() as usize).map_err(|e|e.to_string())),
                legacy_comment_import:false,
                imported_annotations: Default::default(),
                imported_overlay_sources:imported_overlay_sources::Index::unbound(retained.identity().clone()),
                unpublished_fonts: vec![],
                image_leases: None,
                path,
                original_path,
                document: native_residency::ResidentDocument::new(document, self.pdfium, retained.clone()),
                source_bytes: retained,
                for_printing: false,
                print_max_dpi: None,
                policy,
                text_edit_restriction: OnceLock::new(),
                form_admission:form_admission::Admission::default(),
                bookmarks:OnceLock::new(),
            },
        );
        Ok(DocumentInfo { id, name, pages,bookmarks:vec![],bookmark_error:None,protection })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validation_quantization_only_accepts_one_interior_channel_level(){
        let a=image::RgbaImage::from_pixel(2,2,image::Rgba([149,149,149,255]));
        for (pixel,expected) in [([148,148,148,255],true),([150,149,149,255],true),
            ([147,149,149,255],false),([149,149,149,254],false),([0,0,0,255],false)] {
            let b=image::RgbaImage::from_pixel(2,2,image::Rgba(pixel));assert_eq!(only_edge_quantization(&a,&b),expected);
        }
        for (a,b) in [(0,1),(254,255)] {
            assert!(!only_edge_quantization(&image::RgbaImage::from_pixel(1,1,image::Rgba([a,a,a,255])),&image::RgbaImage::from_pixel(1,1,image::Rgba([b,b,b,255]))));
        }
        assert!(!only_edge_quantization(&a,&image::RgbaImage::from_pixel(1,1,image::Rgba([149,149,149,255]))));
    }
    #[test]
    fn validation_quantization_still_requires_an_exact_second_render(){
        let root=PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let rt=WorkerRuntime::new(&root.join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap();
        let authored=|gray:&str|{
            let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
            let content=pdf.add_object(Stream::new(dictionary!{},format!("{gray} g 0 0 100 100 re f").into_bytes()));
            let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()],"Resources"=>dictionary!{},"Contents"=>content});
            pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into());
            let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);crate::pdf_output::save(&mut pdf).unwrap()
        };
        let bytes_a=authored("0.585");let bytes_b=authored("0.581");
        let a=rt.pdfium.load_pdf_from_byte_slice(&bytes_a,None).unwrap();let b=rt.pdfium.load_pdf_from_byte_slice(&bytes_b,None).unwrap();
        let a=a.pages().get(0).unwrap();let b=b.pages().get(0).unwrap();
        let first=raster(&a).unwrap();let second=raster(&b).unwrap();assert!(first!=second);
        assert!(only_edge_quantization(&first,&second));
        assert!(!validation_rasters_match(&a,&b).unwrap());
    }
    #[test]
    fn form_resource_pruning_retains_indirect_reference_chains() {
        let mut pdf = Document::with_version("1.7");
        let terminal = pdf.add_object(dictionary! {"Type"=>"Catalog"});
        let alias = pdf.add_object(Object::Reference(terminal));
        pdf.trailer.set("Root", alias);
        let unreachable = pdf.add_object(dictionary! {"Unused"=>true});
        prune_unreachable(&mut pdf).unwrap();
        assert!(pdf.objects.contains_key(&terminal));
        assert_eq!(
            pdf.get_dictionary(alias)
                .unwrap()
                .get(b"Type")
                .unwrap()
                .as_name()
                .unwrap(),
            b"Catalog"
        );
        assert!(!pdf.objects.contains_key(&unreachable));
    }
}
#[cfg(test)]
mod allocation_tests {
    use super::*;
    #[test]
    fn form_materialization_is_bounded_across_shared_invocations() {
        for resource_padding in [false, true] {
            let mut pdf = Document::with_version("1.7");
            let pages = pdf.new_object_id();
            let font = pdf.add_object(
                dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"},
            );
            let mut resources = dictionary! {"Font"=>dictionary!{"F1"=>font}};
            let mut content = b"BT /F1 12 Tf 0 0 Td (Old) Tj ET\n".to_vec();
            if resource_padding {
                resources.set("Padding", Object::string_literal(vec![b'a'; 256 * 1024]));
            } else {
                content.push(b'%');
                content.extend(vec![b'a'; 256 * 1024]);
                content.push(b'\n');
            }
            let form=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),100.into()],"Resources"=>resources},content));
            let content =
                pdf.add_object(Stream::new(dictionary! {}, b"q /Form Do Q\n".repeat(100)));
            let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),200.into(),200.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"Form"=>form}},"Contents"=>content});
            pdf.objects.insert(
                pages,
                dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into(),
            );
            let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
            pdf.trailer.set("Root", root);
            let bytes = save(&mut pdf).unwrap();
            assert!(
                Model::read(&bytes).is_err(),
                "Unbounded retained {} clones",
                if resource_padding {
                    "resources"
                } else {
                    "stream"
                }
            );
        }
    }
}
#[cfg(test)]
mod alias_allocation_tests {
    use super::*;
    #[test]
    fn form_alias_cloning_bounds_shared_indirect_tables() {
        let mut pdf = Document::with_version("1.7");
        let pages = pdf.new_object_id();
        let table = pdf.new_object_id();
        let font =
            pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
        let leaf=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),100.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}},b"BT /F1 12 Tf 0 0 Td (Old) Tj ET".to_vec()));
        let wrapper=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),100.into()],"Resources"=>dictionary!{"XObject"=>table}},b"/Leaf Do".to_vec()));
        pdf.objects.insert(table,dictionary!{"Wrapper"=>wrapper,"Leaf"=>leaf,"Unused"=>Object::string_literal(vec![b'a';9*1024*1024])}.into());
        let content = pdf.add_object(Stream::new(dictionary! {}, b"/Wrapper Do".to_vec()));
        let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),200.into(),200.into()],"Resources"=>dictionary!{"XObject"=>table},"Contents"=>content});
        pdf.objects.insert(
            pages,
            dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into(),
        );
        let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
        pdf.trailer.set("Root", root);
        let model = Model::read(&save(&mut pdf).unwrap()).unwrap();
        assert!(
            model.replace(&[0, 0, 0], &Operation::new("Tj", vec![Object::string_literal("New")])).is_err(),
            "Repeated resolved XObject table clones exceeded cumulative bound"
        );
    }
}
