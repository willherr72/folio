//! Isolate an invocation by cloning its stream/resource chain. Flattening is used
//! only as a private validation instrument; published pages retain their forms.
use super::*;
use lopdf::{
    content::{Content, Operation},
    dictionary, Dictionary, Document, Object, Stream, StringFormat,
};
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
    text: String,
    boxes: Vec<[f32; 4]>,
}
struct Model {
    pdf: Document,
    page: lopdf::ObjectId,
    root: Node,
    occurrences: Vec<Occurrence>,
    flat: Vec<u8>,
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
// Positive axis-aligned transforms keep ancestor boxes and collision bounds exact.
fn matrix(values: &[Object]) -> EngineResult<[f32; 6]> {
    let n = numbers(values, 6)?;
    if n[0] <= 0.0001
        || n[3] <= 0.0001
        || n[0] > 100.0
        || n[3] > 100.0
        || n[1] != 0.0
        || n[2] != 0.0
    {
        return Err(invalid("Form editing currently supports positive scale and translation; rotated, sheared, or singular transforms are unsupported."));
    }
    Ok([n[0], n[1], n[2], n[3], n[4], n[5]])
}
fn compose(a: [f32; 6], b: [f32; 6]) -> EngineResult<[f32; 6]> {
    matrix(&[
        Object::Real(a[0] * b[0]),
        0.into(),
        0.into(),
        Object::Real(a[3] * b[3]),
        Object::Real(a[0] * b[4] + a[4]),
        Object::Real(a[3] * b[5] + a[5]),
    ])
}
fn font_proof(
    pdf: &Document,
    resources: &Dictionary,
    name: &[u8],
    copies: &mut crate::form_graph::CopyBudget,
) -> EngineResult<Object> {
    let value = resource(pdf, resources, b"Font", name)?;
    let font = dictionary_ref(pdf, value)?;

    if font.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Type1")
        || !font
            .get(b"BaseFont")
            .and_then(Object::as_name)
            .ok()
            .and_then(|n| std::str::from_utf8(n).ok())
            .is_some_and(base_font)
    {
        return Err(invalid("Form text currently requires a standard Latin Type1 font; embedded and custom fonts are unsupported."));
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
    if let Ok(v) = font.get(b"Encoding") {
        let (_, v) = pdf.dereference(v).map_err(err)?;
        if !matches!(
            v.as_name().ok(),
            Some(b"WinAnsiEncoding" | b"StandardEncoding")
        ) {
            return Err(invalid(
                "Form font encoding must be standard ASCII-compatible Latin.",
            ));
        }
    }
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
    mut boxes: Vec<[f32; 4]>,
    out: &mut Vec<Operation>,
    fonts: &mut Dictionary,
    occurrences: &mut Vec<Occurrence>,
    copies: &mut crate::form_graph::CopyBudget,
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
        if b[0] >= b[2] || b[1] >= b[3] {
            return Err(invalid("Malformed form bounding box."));
        }
        boxes.push([
            b[0] * transform[0] + transform[4],
            b[1] * transform[3] + transform[5],
            b[2] * transform[0] + transform[4],
            b[3] * transform[3] + transform[5],
        ]);
    }
    let mut stack = vec![];
    let mut in_text = false;
    let mut have_font = false;
    let mut have_position = false;
    let mut shown = false;
    let mut index = 0;
    for (opindex, op) in node.operations.iter().enumerate() {
        let mut op = op.clone();
        match op.operator.as_str() {
            "q" => {
                if in_text || stack.len() >= 64 {
                    return Err(invalid("Unsupported nested graphics/text state."));
                }
                stack.push(transform);
            }
            "Q" => {
                if in_text {
                    return Err(invalid(
                        "Graphics state inside text objects is unsupported.",
                    ));
                }
                transform = stack
                    .pop()
                    .ok_or_else(|| invalid("Unbalanced form graphics state."))?;
            }
            "cm" => {
                if in_text {
                    return Err(invalid("Graphics transforms inside text are unsupported."));
                }
                transform = compose(transform, matrix(&op.operands)?)?;
            }
            "BT" => {
                if in_text {
                    return Err(invalid("Nested text objects are unsupported."));
                }
                in_text = true;
                have_font = false;
                have_position = false;
                shown = false;
            }
            "ET" => {
                if !in_text || !shown {
                    return Err(invalid("Empty or unbalanced text objects are unsupported."));
                }
                in_text = false;
            }
            "Tf" => {
                if !in_text || shown {
                    return Err(invalid(
                        "Form text must select its font before one text-show operator.",
                    ));
                }
                let [name, size] = op.operands.as_slice() else {
                    return Err(err("Tf"));
                };
                let n = numbers(std::slice::from_ref(size), 1)?;
                if n[0] <= 0.0 || n[0] > 1000.0 {
                    return Err(invalid("Form font size is outside safe bounds."));
                }
                let font = font_proof(pdf, &node.resources, name.as_name().map_err(err)?, copies)?;
                let alias = format!("FolioFlat{}", fonts.len());
                fonts.set(alias.as_bytes(), font);
                op.operands[0] = Object::Name(alias.into_bytes());
                have_font = true;
            }
            "Tm" => {
                if !in_text || shown || have_position {
                    return Err(invalid("Form text requires one explicit text position."));
                }
                matrix(&op.operands)?;
                have_position = true;
            }
            "Td" => {
                if !in_text || shown || have_position {
                    return Err(invalid("Relative form text positioning is unsupported."));
                }
                numbers(&op.operands, 2)?;
                have_position = true;
            }
            "Tj" => {
                if !in_text || !have_font || !have_position || shown {
                    return Err(invalid("Form text requires explicit local font/position and one Tj per text object."));
                }
                let [value] = op.operands.as_slice() else {
                    return Err(err("Tj"));
                };
                let text = std::str::from_utf8(value.as_str().map_err(err)?).map_err(err)?;
                if !printable(text) || text.trim() != text {
                    return Err(invalid("Form text currently supports bounded printable ASCII without surrounding whitespace."));
                }
                let mut path = prefix.to_vec();
                path.push(index);
                occurrences.push(Occurrence {
                    path,
                    text: text.into(),
                    boxes: boxes.clone(),
                });
                index += 1;
                shown = true;
            }
            "Do" => {
                if in_text {
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
                out.push(Operation::new("q", vec![]));
                flatten(
                    pdf,
                    child,
                    &path,
                    transform,
                    boxes.clone(),
                    out,
                    fonts,
                    occurrences,
                    copies,
                )?;
                out.push(Operation::new("Q", vec![]));
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
            _ => {
                return Err(invalid(&format!(
                    "Form editing does not support the {} operator or its inherited state.",
                    op.operator
                )))
            }
        }
        out.push(op);
    }
    if in_text || !stack.is_empty() {
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
        flatten(
            &pdf,
            &root,
            &[],
            [1., 0., 0., 1., 0., 0.],
            vec![],
            &mut operations,
            &mut fonts,
            &mut occurrences,
            &mut copies,
        )?;
        if occurrences.len() > 5000 {
            return Err(invalid("Form text object limit exceeded."));
        }
        let mut flat = pdf.clone();
        let content = flat.add_object(Stream::new(
            dictionary! {},
            Content { operations }.encode().map_err(err)?,
        ));
        let d = flat.get_dictionary_mut(page).map_err(err)?;
        d.set("Contents", content);
        d.set("Resources", dictionary! {"Font"=>fonts});
        let flat = save(&mut flat)?;
        Ok(Self {
            pdf,
            page,
            root,
            occurrences,
            flat,
        })
    }
    fn replace(&self, path: &[usize], replacement: &str) -> EngineResult<Vec<u8>> {
        let mut pdf = self.pdf.clone();
        let (content, resources) = clone_chain(
            &mut pdf,
            &self.root,
            path,
            replacement,
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
    replacement: &str,
    copies: &mut crate::form_graph::CopyBudget,
) -> EngineResult<(Vec<u8>, Dictionary)> {
    let mut ops = node.operations.clone();
    copies.dictionary(&node.resources)?;
    let mut resources = node.resources.clone();
    let mut object_index = 0;
    let mut found = false;
    for (opindex, op) in ops.iter_mut().enumerate() {
        if !matches!(op.operator.as_str(), "Tj" | "Do") {
            continue;
        }
        if Some(&object_index) == path.first() {
            if path.len() == 1 && op.operator == "Tj" {
                op.operands = vec![Object::String(
                    replacement.as_bytes().to_vec(),
                    StringFormat::Hexadecimal,
                )];
            } else if path.len() > 1 && op.operator == "Do" {
                let child = &node
                    .children
                    .iter()
                    .find(|(i, _)| *i == opindex)
                    .ok_or_else(|| err("child"))?
                    .1;
                let (bytes, child_resources) =
                    clone_chain(pdf, child, &path[1..], replacement, copies)?;
                let stream = child.stream.as_ref().ok_or_else(|| err("stream"))?;
                copies.stream(stream)?;
                let mut stream = stream.clone();
                stream.set_plain_content(bytes);
                if path.len() > 2 || stream.dict.has(b"Resources") {
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
                    .filter(|op| matches!(op.operator.as_str(), "Tj" | "Do"))
                    .count()
        {
            return Err(invalid(
                "Form operators cannot be mapped uniquely to native object paths.",
            ));
        }
        let mut objects = objects.into_iter();
        for (i, op) in expected.operations.iter().enumerate() {
            match op.operator.as_str() {
                "Tj" => {
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
        || raster(&original)? != raster(&flat)?
    {
        return Err(invalid(
            "Form clipping or inherited state cannot be preserved by the validation page.",
        ));
    }
    let mut runs = snapshot(&flat, None)?;
    if runs.len() != model.occurrences.len() {
        return Err(invalid("Form text operators cannot be mapped uniquely."));
    }
    for (run, occurrence) in runs.iter_mut().zip(&model.occurrences) {
        if run.reason.is_some() || run.text.trim_end() != occurrence.text {
            return Err(invalid(
                "Form text encoding or native glyph mapping is ambiguous.",
            ));
        }
        run.text = occurrence.text.clone();
        if occurrence.boxes.iter().any(|b| !inside(&run.bounds, b)) {
            return Err(invalid(
                "Form text extends outside an ancestor form bounding box.",
            ));
        }
    }
    Ok(runs)
}
fn inside(a: &[f32; 4], b: &[f32; 4]) -> bool {
    a[0] >= b[0] - EPSILON
        && a[1] >= b[1] - EPSILON
        && a[2] <= b[2] + EPSILON
        && a[3] <= b[3] + EPSILON
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
fn source_admission(bytes: &[u8]) -> EngineResult<()> {
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
            if d.has(b"Annots") || d.has(b"AP") {
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
impl WorkerRuntime {
    fn form_baseline(&self, source_id: &str, page_index: usize) -> EngineResult<Vec<u8>> {
        // A raw-source graph guard must run before native page copying/traversal.
        crate::form_graph::preflight(&self.document(source_id)?.source_bytes)?;
        source_admission(&self.document(source_id)?.source_bytes)?;
        let copy = self.editable_page_copy(source_id, page_index)?;
        let bytes = copy.save_to_bytes()?;
        let source = self
            .document(source_id)?
            .document
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
        if page_index >= self.document(source_id)?.document.pages().len() as usize {
            return Err(invalid("The requested page index is out of range."));
        }
        let inspect = || -> EngineResult<FormTextRuns> {
            if !source_has_forms(&self.document(source_id)?.source_bytes)? {
                return Ok(FormTextRuns {
                    runs: vec![],
                    reason: None,
                });
            }
            let bytes = self.form_baseline(source_id, page_index)?;
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
                        font_size: r.font_size,
                        bounds: AnnotationRect {
                            x: a.x.min(b.x),
                            y: a.y.min(b.y),
                            width: (a.x - b.x).abs(),
                            height: (a.y - b.y).abs(),
                        },
                        supported: true,
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
    ) -> EngineResult<DocumentInfo> {
        if path.len() < 2
            || path.len() > DEPTH + 1
            || !printable(replacement)
            || replacement.trim() != replacement
        {
            return Err(invalid("Select a bounded form occurrence and enter printable ASCII without surrounding whitespace."));
        }
        let baseline = self.form_baseline(source_id, page_index)?;
        let model = Model::read(&baseline)?;
        let before = validate_flat(self, &baseline, &model)?;
        let selected = model
            .occurrences
            .iter()
            .position(|o| o.path == path)
            .ok_or_else(|| invalid("The selected form object path is stale."))?;
        if model.occurrences[selected].text != expected_text {
            return Err(invalid(
                "The text changed since it was selected. Select the occurrence again.",
            ));
        }
        let noop = model.replace(path, expected_text)?;
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
        let bytes: Arc<[u8]> = model.replace(path, replacement)?.into();
        let after_model = Model::read(&bytes)?;
        let after = validate_flat(self, &bytes, &after_model)?;
        if before.len() != after.len()
            || model
                .occurrences
                .iter()
                .zip(&after_model.occurrences)
                .any(|(a, b)| a.path != b.path)
            || before.iter().zip(&after).enumerate().any(|(i, (a, b))| {
                if i == selected {
                    b.text != replacement || !same_style(a, b)
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
        let source = self.document(source_id)?;
        let path = source.path.clone();
        let original_path = source.original_path.clone();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document.pdf")
            .to_string();
        let id = Uuid::new_v4().to_string();
        self.documents.insert(
            id.clone(),
            OpenDocument {
                path,
                original_path,
                document,
                source_bytes: bytes,
                for_printing: false,
                text_edit_restriction: OnceLock::new(),
            },
        );
        Ok(DocumentInfo { id, name, pages })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
            model.replace(&[0, 0, 0], "New").is_err(),
            "Repeated resolved XObject table clones exceeded cumulative bound"
        );
    }
}
