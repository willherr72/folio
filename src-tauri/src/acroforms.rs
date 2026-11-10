//! Preserve document-level AcroForms when PDFium imports individual pages.
use crate::{EngineError, EngineResult};
use lopdf::{content::Content, dictionary, Dictionary, Document, Object, ObjectId, Stream};
use std::{
    collections::{HashMap, HashSet},
};
#[path = "acroform_actions.rs"]
mod actions;
#[path = "form_fill_actions.rs"]
mod filling_actions;
pub(crate) use filling_actions::ActionMask;

pub(crate) struct SourceForm<'a> {
    bytes: &'a [u8],
    parsed: Option<std::cell::OnceCell<crate::request_parse::ReadDocument>>,
    /// Original page index, output page index. Repeated originals are intentional.
    pub pages: Vec<(usize, usize)>,
}
impl<'a> SourceForm<'a> {
    pub(crate) fn new(bytes: &'a [u8], pages: Vec<(usize, usize)>) -> Self {
        Self { bytes, pages, parsed: None }
    }
    /// Single-source composition and text-page preparation can opt in. Keep the
    /// retained parse small and transaction-local; large sources stay uncached.
    pub(crate) fn enable_parse_reuse(&mut self) {
        if self.bytes.len() <= 8 * 1024 * 1024 {
            self.parsed = Some(std::cell::OnceCell::new());
        }
    }
    pub(crate) fn document(&self) -> EngineResult<std::borrow::Cow<'_, Document>> {
        use std::borrow::Cow;
        if let Some(document) = self.parsed.as_ref().and_then(|cell| cell.get()) {
            return Ok(Cow::Borrowed(document));
        }
        let document = read(self.bytes)?;
        if let Some(cell) = &self.parsed {
            if document.objects.len() <= 100_000 {
                // No mutation or re-entrant callbacks occur between get and set.
                let _ = cell.set(document);
                return Ok(Cow::Borrowed(cell.get().expect("source parse initialized")));
            }
        }
        Ok(Cow::Owned(document.into_owned()))
    }
}
const MAX_NODES: usize = 4000;
const MAX_DEPTH: usize = 64;
const MAX_VALUES: usize = 100_000;
const MAX_BYTES: usize = 64 * 1024 * 1024;
fn bad(s: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("PDF forms: {s}"))
}
#[cfg(test)]
std::thread_local! {
    static SOURCE_LOAD_WATCH: std::cell::Cell<Option<(usize, usize, usize)>> = const { std::cell::Cell::new(None) };
}
#[cfg(test)]
pub(crate) fn count_source_loads<T>(bytes: &[u8], action: impl FnOnce() -> T) -> (T, usize) {
    struct Reset;
    impl Drop for Reset { fn drop(&mut self) { SOURCE_LOAD_WATCH.set(None); } }
    assert!(SOURCE_LOAD_WATCH.get().is_none());
    SOURCE_LOAD_WATCH.set(Some((bytes.as_ptr() as usize, bytes.len(), 0)));
    let _reset = Reset;
    let result = action();
    (result, SOURCE_LOAD_WATCH.get().unwrap().2)
}
fn load(bytes: &[u8]) -> EngineResult<Document> {
    read(bytes).map(crate::request_parse::ReadDocument::into_owned)
}
fn read(bytes: &[u8]) -> EngineResult<crate::request_parse::ReadDocument> {
    #[cfg(test)]
    if let Some((pointer, length, count)) = SOURCE_LOAD_WATCH.get() {
        if pointer == bytes.as_ptr() as usize && length == bytes.len() {
            SOURCE_LOAD_WATCH.set(Some((pointer, length, count + 1)));
        }
    }
    if bytes.len() > 512 * 1024 * 1024 {
        return Err(bad("document exceeds 512 MiB"));
    }
    crate::request_parse::read(bytes,crate::request_parse::Policy::Strict64)
    .map_err(bad)
}
fn resolve<'a>(doc: &'a Document, mut value: &'a Object) -> EngineResult<&'a Object> {
    let mut seen = HashSet::new();
    while let Object::Reference(id) = value {
        if seen.len() >= MAX_DEPTH || !seen.insert(*id) {
            return Err(bad("cyclic or deep indirect reference"));
        }
        value = doc
            .objects
            .get(id)
            .ok_or_else(|| bad("missing referenced object"))?;
    }
    Ok(value)
}
fn dict<'a>(doc: &'a Document, value: &'a Object) -> EngineResult<&'a Dictionary> {
    resolve(doc, value)?.as_dict().map_err(bad)
}
fn optional<'a>(
    doc: &'a Document,
    d: &'a Dictionary,
    key: &[u8],
) -> EngineResult<Option<&'a Object>> {
    match d.get(key) {
        Ok(o) if !matches!(resolve(doc, o)?, Object::Null) => Ok(Some(o)),
        _ => Ok(None),
    }
}
fn named(d: &Dictionary, key: &[u8], value: &[u8]) -> bool {
    d.get(key)
        .and_then(Object::as_name)
        .is_ok_and(|n| n == value)
}
fn widget(d: &Dictionary) -> bool {
    named(d, b"Subtype", b"Widget")
}
fn is_widget(doc: &Document, d: &Dictionary) -> EngineResult<bool> {
    Ok(optional(doc, d, b"Subtype")?
        .map(|o| resolve(doc, o)?.as_name().map_err(bad))
        .transpose()?
        == Some(b"Widget"))
}
fn inherited_type(doc: &Document, data: &Dictionary) -> EngineResult<Option<Vec<u8>>> {
    let mut current = data;
    let mut seen = HashSet::new();
    loop {
        if let Some(value) = optional(doc, current, b"FT")? {
            return Ok(Some(resolve(doc, value)?.as_name().map_err(bad)?.to_vec()));
        }
        let Some(parent) = optional(doc, current, b"Parent")? else {
            return Ok(None);
        };
        let id = parent.as_reference().map_err(bad)?;
        if seen.len() >= MAX_DEPTH || !seen.insert(id) {
            return Err(bad("cyclic or deep field inheritance"));
        }
        current = dict(doc, parent)?;
    }
}
fn text(o: &Object) -> EngineResult<String> {
    lopdf::decode_text_string(o).map_err(bad)
}
fn requires_stable_names(doc: &Document, o: &Object, depth: usize) -> bool {
    if depth > MAX_DEPTH {
        return true;
    }
    match o {
        Object::Array(a) => a.iter().any(|v| requires_stable_names(doc, v, depth + 1)),
        Object::Dictionary(d) => {
            let action = d
                .get(b"S")
                .ok()
                .and_then(|o| resolve(doc, o).ok())
                .and_then(|o| o.as_name().ok());
            // Name-based targets may also live in ordinary page annotations, not
            // only widgets. Refuse an ambiguous rename throughout the source.
            matches!(
                action,
                Some(b"JavaScript" | b"ResetForm" | b"SubmitForm" | b"Hide" | b"ImportData")
            ) || d.has(b"JS")
                || d.iter()
                    .any(|(_, v)| requires_stable_names(doc, v, depth + 1))
        }
        Object::Stream(s) => {
            requires_stable_names(doc, &Object::Dictionary(s.dict.clone()), depth + 1)
        }
        _ => false,
    }
}
fn effective_names(
    source: &Document,
    node: &Node,
    prefix: &str,
    result: &mut HashSet<String>,
) -> EngineResult<()> {
    let own = optional(source, &node.data, b"T")?
        .map(|v| text(resolve(source, v)?))
        .transpose()?;
    let name = match &own {
        Some(t) if prefix.is_empty() => t.clone(),
        Some(t) => format!("{prefix}.{t}"),
        None => prefix.into(),
    };
    if own.is_some() && !result.insert(name.clone()) {
        return Err(bad("duplicate effective field names in one tree"));
    }
    for child in &node.children {
        effective_names(source, child, &name, result)?;
    }
    Ok(())
}

fn preflight_documents(documents: &[&Document], sources: &[SourceForm<'_>]) -> EngineResult<()> {
    let mut source_nodes = 0usize;
    let mut output_nodes = 0usize;
    for (doc, request) in documents.iter().zip(sources) {
        let mut page_widgets = HashMap::new();
        let pages = doc.get_pages().values().copied().collect::<Vec<_>>();
        for (old, _) in &request.pages {
            let count = if let Some(count) = page_widgets.get(old) {
                *count
            } else {
                let id = pages
                    .get(*old)
                    .ok_or_else(|| bad("unknown original page"))?;
                let page = doc.get_dictionary(*id).map_err(bad)?;
                let mut count = 0;
                if let Some(annots) = optional(doc, page, b"Annots")? {
                    for value in resolve(doc, annots)?.as_array().map_err(bad)? {
                        if is_widget(doc, dict(doc, value)?)? {
                            count += 1;
                        }
                    }
                }
                page_widgets.insert(*old, count);
                count
            };
            output_nodes = output_nodes.saturating_add(count);
            if output_nodes > MAX_NODES {
                return Err(bad("export exceeds 4000 form fields and widget placements"));
            }
        }
        if let Some(form) = optional(doc, doc.catalog().map_err(bad)?, b"AcroForm")? {
            let form = dict(doc, form)?;
            let mut seen = HashSet::new();
            fn fields(node: &Node) -> usize {
                usize::from(node.field) + node.children.iter().map(fields).sum::<usize>()
            }
            if let Some(roots) = optional(doc, form, b"Fields")? {
                for root in resolve(doc, roots)?.as_array().map_err(bad)? {
                    let node = read_node(
                        doc,
                        root.as_reference().map_err(bad)?,
                        None,
                        &HashMap::new(),
                        &mut seen,
                        0,
                    )?;
                    output_nodes = output_nodes.saturating_add(fields(&node));
                    if output_nodes > MAX_NODES {
                        return Err(bad("export exceeds 4000 form fields and widget placements"));
                    }
                }
            }
            source_nodes = source_nodes.saturating_add(seen.len());
            if source_nodes > MAX_NODES {
                return Err(bad("export exceeds 4000 source form nodes"));
            }
        }
    }
    Ok(())
}
pub(crate) fn preflight(sources: &[SourceForm<'_>]) -> EngineResult<()> {
    let docs = sources
        .iter()
        .map(SourceForm::document)
        .collect::<EngineResult<Vec<_>>>()?;
    preflight_documents(&docs.iter().map(|d| d.as_ref()).collect::<Vec<_>>(), sources)
}

/// Validate before native interactive editing and load only pages with widgets.
pub(crate) fn filling_document(bytes: &[u8]) -> EngineResult<(Document,Vec<usize>)> {
    let doc = load(bytes)?;
    let found = validate_filling_document(bytes, &doc)?;
    Ok((doc, found))
}
/// Reuse the existing fill admission after a caller has bounded a parsed graph.
pub(crate) fn validate_filling_document(bytes: &[u8], doc: &Document) -> EngineResult<Vec<usize>> {
    if let Some(form)=optional(&doc,doc.catalog().map_err(bad)?,b"AcroForm")? {
        if optional(&doc,dict(&doc,form)?,b"XFA")?.is_some(){return Err(bad("XFA forms cannot be filled safely yet"));}
    }
    let pages = doc.get_pages();
    let source = SourceForm::new(bytes, (0..pages.len()).map(|i|(i,i)).collect());
    preflight_documents(&[&doc], &[source])?;
    let mut found = vec![];
    for (index, id) in pages.values().enumerate() {
        if let Some(annots) = optional(&doc, doc.get_dictionary(*id).map_err(bad)?, b"Annots")? {
            for annot in resolve(&doc, annots)?.as_array().map_err(bad)? {
                if is_widget(&doc, dict(&doc, annot)?)? {found.push(index);break;}
            }
        }
    }
    Ok(found)
}

/// Authoring keeps the original object graph. Bound direct field data before
/// the existing graph validator clones nodes, then verify unambiguous names.
fn authoring_names(source:&Document,node:&Node,prefix:&str,result:&mut HashSet<String>,remaining:&mut usize)->EngineResult<()> {
    fn take(remaining:&mut usize,bytes:usize)->EngineResult<()> {
        *remaining=remaining.checked_sub(bytes).ok_or_else(||bad("expanded field name budget exceeds 8 MiB"))?;
        Ok(())
    }
    let Some(value)=optional(source,&node.data,b"T")? else {
        for child in &node.children {authoring_names(source,child,prefix,result,remaining)?;}
        return Ok(());
    };
    let value=resolve(source,value)?;
    let raw=value.as_str().map_err(bad)?;
    // PDFDocEncoding can expand one byte into three UTF-8 bytes. Charge
    // before decoding, including repeated indirect strings, not afterward.
    take(remaining,raw.len().checked_mul(3).and_then(|n|n.checked_add(64)).ok_or_else(||bad("field name budget overflow"))?)?;
    let own=text(value)?;
    let size=prefix.len().checked_add(own.len()).and_then(|n|n.checked_add(1)).ok_or_else(||bad("field name budget overflow"))?;
    // One qualified name remains on the recursive stack, one in the set.
    take(remaining,size.checked_mul(2).and_then(|n|n.checked_add(64)).ok_or_else(||bad("field name budget overflow"))?)?;
    let name=if prefix.is_empty(){own}else{format!("{prefix}.{own}")};
    if !result.insert(name.clone()) {return Err(bad("duplicate effective field names in one tree"));}
    for child in &node.children {authoring_names(source,child,&name,result,remaining)?;}
    Ok(())
}

pub(crate) fn authoring_document(bytes: &[u8]) -> EngineResult<(Document, HashSet<String>)> {
    let doc = load(bytes)?;
    let mut names = HashSet::new();
    if let Some(form) = optional(&doc, doc.catalog().map_err(bad)?, b"AcroForm")? {
        let form = dict(&doc, form)?;
        if optional(&doc, form, b"XFA")?.is_some() { return Err(bad("XFA forms cannot be authored safely yet")); }
        let mut budget = crate::form_graph::CopyBudget::default();
        budget.dictionary(form)?;
        if let Some(roots) = optional(&doc, form, b"Fields")? {
            let roots = resolve(&doc, roots)?.as_array().map_err(bad)?;
            if roots.len() > MAX_NODES { return Err(bad("too many root fields")); }
            budget.array(roots)?;
            let mut pending: Vec<_> = roots.iter().collect();
            let mut seen = HashSet::new();
            while let Some(value) = pending.pop() {
                let id = value.as_reference().map_err(bad)?;
                if seen.len() >= MAX_NODES || !seen.insert(id) { return Err(bad("repeated or excessive form nodes")); }
                let node = dict(&doc, value)?;
                budget.dictionary(node)?;
                // read_node normalizes these references into owned objects.
                // Charge each resolved value before any node tree is cloned.
                for key in [b"Type".as_slice(),b"Subtype".as_slice(),b"FT".as_slice()] {
                    if let Some(value)=optional(&doc,node,key)? {budget.array(std::slice::from_ref(resolve(&doc,value)?))?;}
                }
                if let Some(kids) = optional(&doc, node, b"Kids")? {
                    let kids = resolve(&doc, kids)?.as_array().map_err(bad)?;
                    if kids.len() > MAX_NODES { return Err(bad("too many child fields")); }
                    pending.extend(kids);
                }
            }
            let mut validated = HashSet::new();
            let mut name_budget=8*1024*1024;
            for root in roots {
                let node = read_node(&doc, root.as_reference().map_err(bad)?, None, &HashMap::new(), &mut validated, 0)?;
                authoring_names(&doc, &node, "", &mut names, &mut name_budget)?;
            }
        }
    }
    let count = doc.get_pages().len();
    preflight_documents(&[&doc], &[SourceForm::new(bytes, (0..count).map(|i|(i,i)).collect())])?;
    Ok((doc, names))
}

#[derive(Default)]
struct Budget {
    values: usize,
    bytes: usize,
}
impl Budget {
    fn take(&mut self, bytes: usize, depth: usize) -> EngineResult<()> {
        self.values += 1;
        self.bytes = self.bytes.saturating_add(bytes);
        if self.values > MAX_VALUES || self.bytes > MAX_BYTES || depth > MAX_DEPTH {
            Err(bad("form resources exceed the supported size/depth limit"))
        } else {
            Ok(())
        }
    }
}
#[derive(Clone)]
struct Node {
    original: ObjectId,
    data: Dictionary,
    children: Vec<Node>,
    /// A pure widget belongs to its parent's field; a merged widget has its own field.
    field: bool,
    placements: Vec<(usize, usize)>,
    output: Option<ObjectId>,
    widgets: Vec<ObjectId>,
}
fn read_node(
    doc: &Document,
    id: ObjectId,
    parent: Option<ObjectId>,
    placements: &HashMap<ObjectId, Vec<(usize, usize)>>,
    seen: &mut HashSet<ObjectId>,
    depth: usize,
) -> EngineResult<Node> {
    if depth >= MAX_DEPTH || seen.len() >= MAX_NODES || !seen.insert(id) {
        return Err(bad("cyclic, repeated or excessive field hierarchy"));
    }
    let mut data = dict(
        doc,
        doc.objects.get(&id).ok_or_else(|| bad("missing field"))?,
    )?
    .clone();
    for key in [b"Type".as_slice(), b"Subtype".as_slice(), b"FT".as_slice()] {
        if let Some(value) = optional(doc, &data, key)? {
            data.set(key.to_vec(), resolve(doc, value)?.clone());
        }
    }
    if let Some(p) = optional(doc, &data, b"Parent")? {
        if Some(p.as_reference().map_err(bad)?) != parent {
            return Err(bad("field Parent disagrees with its hierarchy"));
        }
    }
    if inherited_type(doc, &data)?.as_deref() == Some(b"Sig")
        && optional(doc, &data, b"V")?.is_some()
    {
        return Err(bad(
            "signed forms cannot be rewritten without invalidating their signatures",
        ));
    }
    let is_widget = widget(&data);
    let field = !is_widget || parent.is_none() || data.has(b"T") || data.has(b"FT");
    let mut children = vec![];
    if let Some(kids) = optional(doc, &data, b"Kids")? {
        if is_widget {
            return Err(bad("a widget cannot contain child fields"));
        }
        for child in resolve(doc, kids)?.as_array().map_err(bad)? {
            children.push(read_node(
                doc,
                child.as_reference().map_err(bad)?,
                Some(id),
                placements,
                seen,
                depth + 1,
            )?);
        }
    }
    Ok(Node {
        original: id,
        data,
        children,
        field,
        placements: placements.get(&id).cloned().unwrap_or_default(),
        output: None,
        widgets: vec![],
    })
}
fn allocate(
    node: &mut Node,
    out: &mut Document,
    map: &mut HashMap<ObjectId, Object>,
    all_fields: &mut HashSet<ObjectId>,
) -> bool {
    let had_children = !node.children.is_empty();
    node.children
        .retain_mut(|child| allocate(child, out, map, all_fields));
    let retained = if widget(&node.data) {
        !node.placements.is_empty()
    } else {
        !had_children || !node.children.is_empty()
    };
    if !retained {
        map.insert(node.original, Object::Null);
        return false;
    }
    if node.field {
        let id = out.new_object_id();
        node.output = Some(id);
        map.insert(node.original, Object::Reference(id));
        all_fields.insert(id);
    }
    for _ in &node.placements {
        node.widgets.push(out.new_object_id());
    }
    if !node.field {
        map.insert(node.original, Object::Reference(node.widgets[0]));
    }
    true
}
struct Copier<'a> {
    source: &'a Document,
    out: &'a mut Document,
    map: HashMap<ObjectId, Object>,
    annotations: HashMap<ObjectId, Vec<ObjectId>>,
    active: HashSet<ObjectId>,
    budget: &'a mut Budget,
}
impl Copier<'_> {
    fn value(&mut self, value: &Object, depth: usize) -> EngineResult<Object> {
        self.budget.take(0, depth)?;
        if let Object::Reference(id) = value {
            if self.active.contains(id) {
                return Err(bad("cyclic form resource graph"));
            }
            if let Some(mapped) = self.map.get(id) {
                return Ok(mapped.clone());
            }
            let source = self
                .source
                .objects
                .get(id)
                .ok_or_else(|| bad("missing form resource"))?
                .clone();
            if source.as_dict().is_ok_and(|d| {
                named(d, b"Type", b"Catalog")
                    || named(d, b"Type", b"Pages")
                    || named(d, b"Type", b"Page")
            }) {
                return Err(bad("form resource refers to an unsupported page tree"));
            }
            let target = self.out.new_object_id();
            self.map.insert(*id, Object::Reference(target));
            self.active.insert(*id);
            let copied = self.value(&source, depth + 1)?;
            self.active.remove(id);
            self.out.objects.insert(target, copied);
            return Ok(Object::Reference(target));
        }
        Ok(match value {
            Object::Array(a) => Object::Array(
                a.iter()
                    .map(|v| self.value(v, depth + 1))
                    .collect::<EngineResult<_>>()?,
            ),
            Object::Dictionary(d) => {
                if let Some(action) = self.action(d, depth)? {
                    action
                } else {
                    Object::Dictionary(self.dictionary(d, depth)?)
                }
            }
            Object::Stream(s) => {
                self.budget.take(s.content.len(), depth)?;
                Object::Stream(Stream::new(
                    self.dictionary(&s.dict, depth)?,
                    s.content.clone(),
                ))
            }
            Object::String(b, _) | Object::Name(b) => {
                self.budget.take(b.len(), depth)?;
                value.clone()
            }
            Object::Real(n) if !n.is_finite() => return Err(bad("nonfinite form value")),
            _ => value.clone(),
        })
    }
    fn dictionary(&mut self, d: &Dictionary, depth: usize) -> EngineResult<Dictionary> {
        let mut out = Dictionary::new();
        for (k, v) in d.iter() {
            self.budget.take(k.len(), depth)?;
            out.set(k.clone(), self.value(v, depth + 1)?);
        }
        Ok(out)
    }
}
fn rewritten_da(
    source: &Document,
    value: &Object,
    fonts: &HashMap<Vec<u8>, Vec<u8>>,
) -> EngineResult<Object> {
    let bytes = resolve(source, value)?.as_str().map_err(bad)?;
    let mut content = Content::decode(bytes).map_err(bad)?;
    for operation in &mut content.operations {
        if operation.operator == "Tf" {
            if let Some(Object::Name(name)) = operation.operands.first_mut() {
                if let Some(new) = fonts.get(name) {
                    *name = new.clone();
                }
            }
        }
    }
    Ok(Object::string_literal(content.encode().map_err(bad)?))
}
// Field entries belong to the terminal field, not each of its widget children.
const FIELD_KEYS: &[&[u8]] = &[
    b"FT", b"T", b"TU", b"TM", b"Ff", b"V", b"DV", b"DA", b"Q", b"DS", b"RV", b"Opt", b"MaxLen",
    b"TI", b"I", b"Lock", b"SV",
];
const ANNOT_KEYS: &[&[u8]] = &[
    b"Type",
    b"Subtype",
    b"Rect",
    b"P",
    b"AP",
    b"AS",
    b"MK",
    b"BS",
    b"Border",
    b"F",
    b"NM",
    b"M",
    b"C",
    b"OC",
    b"A",
    b"StructParent",
    b"H",
];
fn emit(
    node: &Node,
    parent: Option<ObjectId>,
    copier: &mut Copier<'_>,
    output_pages: &[ObjectId],
    fonts: &HashMap<Vec<u8>, Vec<u8>>,
    placements: &mut HashMap<(usize, usize), ObjectId>,
) -> EngineResult<Vec<Object>> {
    let owner = node
        .output
        .or(parent)
        .ok_or_else(|| bad("widget has no field owner"))?;
    let mut kids = vec![];
    for child in &node.children {
        kids.extend(emit(
            child,
            Some(owner),
            copier,
            output_pages,
            fonts,
            placements,
        )?);
    }
    for ((page, slot), id) in node.placements.iter().zip(&node.widgets) {
        let mut data = node.data.clone();
        data.remove(b"Parent");
        data.remove(b"Kids");
        data.remove(b"P");
        if node.field {
            for key in FIELD_KEYS {
                data.remove(key);
            }
        }
        let mut data = copier.dictionary(&data, 0)?;
        data.set("Parent", owner);
        data.set("P", output_pages[*page]);
        copier.out.objects.insert(*id, Object::Dictionary(data));
        placements.insert((*page, *slot), *id);
        kids.push(Object::Reference(*id));
    }
    if let Some(id) = node.output {
        let mut data = node.data.clone();
        if inherited_type(copier.source, &node.data)?.as_deref() == Some(b"Btn") {
            if let Some(options) = optional(copier.source, &node.data, b"Opt")? {
                let options = resolve(copier.source, options)?.as_array().map_err(bad)?;
                let mut mapped = vec![];
                if widget(&node.data) {
                    let option = options
                        .first()
                        .ok_or_else(|| bad("missing button export value"))?;
                    mapped.extend(std::iter::repeat_n(option.clone(), node.widgets.len()));
                } else if let Some(original_kids) = optional(copier.source, &node.data, b"Kids")? {
                    let original_kids = resolve(copier.source, original_kids)?
                        .as_array()
                        .map_err(bad)?;
                    if options.len() != original_kids.len() {
                        return Err(bad("button export values do not match its controls"));
                    }
                    for child in &node.children {
                        let index = original_kids
                            .iter()
                            .position(|kid| kid.as_reference().ok() == Some(child.original))
                            .ok_or_else(|| bad("unknown button control"))?;
                        mapped.extend(std::iter::repeat_n(
                            options[index].clone(),
                            child.widgets.len(),
                        ));
                    }
                }
                data.set("Opt", mapped);
            }
        }
        data.remove(b"Parent");
        data.remove(b"Kids");
        if widget(&data) {
            for key in ANNOT_KEYS {
                data.remove(key);
            }
        }
        if let Ok(da) = data.get(b"DA") {
            data.set("DA", rewritten_da(copier.source, da, fonts)?);
        }
        let mut data = copier.dictionary(&data, 0)?;
        if let Some(p) = parent {
            data.set("Parent", p);
        }
        if !kids.is_empty() {
            data.set("Kids", kids);
        }
        copier.out.objects.insert(id, Object::Dictionary(data));
        Ok(vec![Object::Reference(id)])
    } else {
        Ok(kids)
    }
}

pub(crate) fn preserve(out: &mut Document, sources: &[SourceForm<'_>]) -> EngineResult<bool> {
    preserve_impl(out, sources, false)
}

/// Only for appending to a clone of one complete source whose original IDs
/// still exist. Rebind retained page/annotation actions as well as form actions.
pub(crate) fn preserve_retained_document(out: &mut Document, sources: &[SourceForm<'_>]) -> EngineResult<bool> {
    if sources.len()!=1 {return Err(bad("retained form repair requires one source"));}
    preserve_impl(out, sources, true)
}

fn preserve_impl(out: &mut Document, sources: &[SourceForm<'_>], retained: bool) -> EngineResult<bool> {
    let documents = sources
        .iter()
        .map(SourceForm::document)
        .collect::<EngineResult<Vec<_>>>()?;
    preflight_documents(&documents.iter().map(|d| d.as_ref()).collect::<Vec<_>>(), sources)?;
    let stable_names = documents
        .iter()
        .any(|d| d.objects.values().any(|o| requires_stable_names(d, o, 0)));
    let output_pages = out.get_pages().values().copied().collect::<Vec<_>>();
    let mut roots = vec![];
    let mut names = HashSet::new();
    let mut resources = Dictionary::new();
    let mut calculation = vec![];
    let mut budget = Budget::default();
    let mut need_appearances = false;
    let mut sig_flags = 0i64;
    let mut replacements = HashMap::new();
    let mut form_seen = false;
    let mut scripts = HashMap::new();
    let mut open_action = None;
    let mut document_actions = Dictionary::new();
    let mut retired = HashSet::new();
    for (source_index, (request, source)) in sources.iter().zip(&documents).enumerate() {
        let pages = source.get_pages().values().copied().collect::<Vec<_>>();
        let mut locations: HashMap<ObjectId, Vec<(usize, usize)>> = HashMap::new();
        let mut map = HashMap::new();
        for id in &pages {
            map.insert(*id, Object::Null);
        }
        for (old, new) in &request.pages {
            let original = *pages
                .get(*old)
                .ok_or_else(|| bad("unknown original page"))?;
            let output = *output_pages
                .get(*new)
                .ok_or_else(|| bad("unknown output page"))?;
            if matches!(map.get(&original), Some(Object::Null)) {
                map.insert(original, Object::Reference(output));
            }
            let page = source.get_dictionary(original).map_err(bad)?;
            let mut slot = 0;
            if let Some(annots) = optional(source, page, b"Annots")? {
                for value in resolve(source, annots)?.as_array().map_err(bad)? {
                    if is_widget(source, dict(source, value)?)? {
                        locations
                            .entry(value.as_reference().map_err(bad)?)
                            .or_default()
                            .push((*new, slot));
                        slot += 1;
                    }
                }
            }
        }
        let catalog = source.catalog().map_err(bad)?;
        let Some(form_value) = optional(source, catalog, b"AcroForm")? else {
            if !locations.is_empty() {
                return Err(bad("page widgets have no AcroForm definition"));
            }
            continue;
        };
        let form = dict(source, form_value)?;
        if optional(source, form, b"XFA")?.is_some() {
            return Err(bad(
                "dynamic XFA forms cannot yet be preserved by this export",
            ));
        }
        form_seen = true;
        let mut seen = HashSet::new();
        let mut nodes = vec![];
        if let Some(fields) = optional(source, form, b"Fields")? {
            for field in resolve(source, fields)?.as_array().map_err(bad)? {
                nodes.push(read_node(
                    source,
                    field.as_reference().map_err(bad)?,
                    None,
                    &locations,
                    &mut seen,
                    0,
                )?);
            }
        }
        if locations.keys().any(|id| !seen.contains(id)) {
            return Err(bad("a widget is missing from the field hierarchy"));
        }
        if retained {retired.extend(seen.iter().copied());}
        let mut retained_fields = HashSet::new();
        nodes.retain_mut(|node| allocate(node, out, &mut map, &mut retained_fields));
        fn widget_map(node: &Node, map: &mut HashMap<ObjectId, Vec<ObjectId>>) {
            if widget(&node.data) {
                map.insert(node.original, node.widgets.clone());
            }
            for child in &node.children {
                widget_map(child, map);
            }
        }
        let mut annotations = HashMap::new();
        for node in &nodes {
            widget_map(node, &mut annotations);
        }
        let mut copier = Copier {
            source,
            out,
            map,
            annotations,
            active: HashSet::new(),
            budget: &mut budget,
        };
        let mut font_names = HashMap::new();
        if let Some(dr) = optional(source, form, b"DR")? {
            for (category, entries) in dict(source, dr)?.iter() {
                if category == b"ProcSet" {
                    let mut procsets = resources
                        .get(category)
                        .ok()
                        .and_then(|v| v.as_array().ok())
                        .cloned()
                        .unwrap_or_default();
                    let mut known = procsets
                        .iter()
                        .filter_map(|v| v.as_name().ok().map(|n| n.to_vec()))
                        .collect::<HashSet<_>>();
                    for entry in resolve(source, entries)?.as_array().map_err(bad)? {
                        let value = resolve(source, entry)?;
                        let name = value.as_name().map_err(bad)?;
                        copier.budget.take(name.len(), 0)?;
                        if known.insert(name.to_vec()) {
                            procsets.push(value.clone());
                        }
                    }
                    resources.set(category.clone(), procsets);
                    continue;
                }
                let entries = dict(source, entries)?;
                let mut merged = resources
                    .get(category)
                    .ok()
                    .and_then(|o| o.as_dict().ok())
                    .cloned()
                    .unwrap_or_default();
                for (name, value) in entries.iter() {
                    let mut alias = name.clone();
                    if merged.has(name) {
                        if category != b"Font" {
                            return Err(bad(
                                "combined forms have conflicting non-font default resources",
                            ));
                        }
                        alias = format!(
                            "Folio{}_{}",
                            source_index + 1,
                            String::from_utf8_lossy(name)
                        )
                        .into_bytes();
                        while merged.has(&alias) {
                            alias.push(b'_');
                        }
                    }
                    if category == b"Font" {
                        font_names.insert(name.clone(), alias.clone());
                    }
                    merged.set(alias, copier.value(value, 0)?);
                }
                resources.set(category.clone(), merged);
            }
        }
        let mut source_names = HashSet::new();
        for node in &mut nodes {
            let mut effective = HashSet::new();
            effective_names(source, node, "", &mut effective)?;
            if !source_names.is_disjoint(&effective) {
                return Err(bad("duplicate effective field names in one source"));
            }
            source_names.extend(effective.iter().cloned());
            if !names.is_disjoint(&effective) {
                if stable_names {
                    return Err(bad("combining forms with duplicate field names and scripts or field-target actions needs manual renaming"));
                }
                let title = optional(source, &node.data, b"T")?
                    .map(|v| text(resolve(source, v)?))
                    .transpose()?
                    .unwrap_or_else(|| "Folio".into());
                let mut number = 2;
                loop {
                    node.data
                        .set("T", lopdf::text_string(&format!("{title}_{number}")));
                    number += 1;
                    effective.clear();
                    effective_names(source, node, "", &mut effective)?;
                    if names.is_disjoint(&effective) {
                        break;
                    }
                }
            }
            names.extend(effective);
            for key in [b"DA".as_slice(), b"Q".as_slice()] {
                if !node.data.has(key) {
                    if let Some(value) = optional(source, form, key)? {
                        node.data.set(key.to_vec(), value.clone());
                    }
                }
            }
            roots.extend(emit(
                node,
                None,
                &mut copier,
                &output_pages,
                &font_names,
                &mut replacements,
            )?);
        }
        if let Some(co) = optional(source, form, b"CO")? {
            for field in resolve(source, co)?.as_array().map_err(bad)? {
                if let Ok(id) = field.as_reference() {
                    if let Some(Object::Reference(mapped)) = copier.map.get(&id) {
                        if retained_fields.contains(mapped) {
                            calculation.push(Object::Reference(*mapped));
                        }
                    }
                }
            }
        }
        actions::catalog_dependencies(
            &mut copier,
            &mut scripts,
            &mut open_action,
            &mut document_actions,
        )?;
        if retained {
            actions::retained_page_dependencies(&mut copier, request, &pages, &output_pages, &seen)?;
        }
        need_appearances |= optional(source, form, b"NeedAppearances")?
            .map(|o| resolve(source, o)?.as_bool().map_err(bad))
            .transpose()?
            .unwrap_or(false);
        sig_flags |= optional(source, form, b"SigFlags")?
            .map(|o| resolve(source, o)?.as_i64().map_err(bad))
            .transpose()?
            .unwrap_or(0);
    }
    if !form_seen {
        return Ok(false);
    }
    for (index, id) in output_pages.iter().enumerate() {
        let mut page = out.get_dictionary(*id).map_err(bad)?.clone();
        let mut slot = 0;
        if let Some(annots) = optional(out, &page, b"Annots")? {
            let mut annots = resolve(out, annots)?.as_array().map_err(bad)?.clone();
            for value in &mut annots {
                if is_widget(out, dict(out, value)?)? {
                    let replacement = replacements
                        .remove(&(index, slot))
                        .ok_or_else(|| bad("copied widgets do not match their source"))?;
                    *value = Object::Reference(replacement);
                    slot += 1;
                }
            }
            page.set("Annots", annots);
            out.objects.insert(*id, Object::Dictionary(page));
        }
    }
    if !replacements.is_empty() {
        return Err(bad("the page copier omitted a form widget"));
    }
    let mut form =
        dictionary! {"Fields"=>roots,"DR"=>resources,"NeedAppearances"=>need_appearances};
    if !calculation.is_empty() {
        form.set("CO", calculation);
    }
    if sig_flags != 0 {
        form.set("SigFlags", sig_flags);
    }
    let form = out.add_object(form);
    out.catalog_mut().map_err(bad)?.set("AcroForm", form);
    if !scripts.is_empty() {
        let mut scripts = scripts.into_iter().collect::<Vec<_>>();
        scripts.sort_by(|a, b| a.0.cmp(&b.0));
        let entries = scripts
            .into_iter()
            .flat_map(|(key, value)| [Object::string_literal(key), value])
            .collect::<Vec<_>>();
        let tree = out.add_object(dictionary! {"Names"=>entries});
        let mut names = optional(out, out.catalog().map_err(bad)?, b"Names")?
            .map(|o| dict(out, o).cloned())
            .transpose()?
            .unwrap_or_default();
        names.set("JavaScript", tree);
        out.catalog_mut().map_err(bad)?.set("Names", names);
    }
    if let Some(action) = open_action {
        out.catalog_mut().map_err(bad)?.set("OpenAction", action);
    }
    if !document_actions.is_empty() {
        out.catalog_mut().map_err(bad)?.set("AA", document_actions);
    }
    if retained {
        crate::page_resources::prune_unreachable(out)?;
        // Unknown inbound references must never keep a detached old field tree
        // alive. Such contexts need explicit support before being accepted.
        if retired.iter().any(|id|out.objects.contains_key(id)) {
            return Err(bad("an unsupported retained reference targets the old form fields"));
        }
    }
    Ok(true)
}

/// Isolated text-edit sources cannot represent a field shared with another page.
/// Independent page fields can be transplanted before any text mutation occurs.
pub(crate) fn preserve_page_copy(
    request: &SourceForm<'_>,
    copied: &[u8],
) -> EngineResult<Option<Vec<u8>>> {
    let page_index = text_page_index(request)?;
    let source = request.document()?;
    let Some(form) = optional(&source, source.catalog().map_err(bad)?, b"AcroForm")? else {
        return Ok(None);
    };
    let form = dict(&source, form)?;
    let mut placements: HashMap<ObjectId, Vec<(usize, usize)>> = HashMap::new();
    for (index, id) in source.get_pages().values().enumerate() {
        let page = source.get_dictionary(*id).map_err(bad)?;
        if let Some(annots) = optional(&source, page, b"Annots")? {
            for value in resolve(&source, annots)?.as_array().map_err(bad)? {
                if is_widget(&source, dict(&source, value)?)? {
                    placements
                        .entry(value.as_reference().map_err(bad)?)
                        .or_default()
                        .push((index, 0));
                }
            }
        }
    }
    fn shared(node: &Node, page: usize) -> bool {
        fn identity_pages(node: &Node, pages: &mut HashSet<usize>) {
            pages.extend(node.placements.iter().map(|(p, _)| *p));
            for child in &node.children {
                if !child.data.has(b"T") {
                    identity_pages(child, pages);
                }
            }
        }
        let mut pages = HashSet::new();
        identity_pages(node, &mut pages);
        (pages.len() > 1 && pages.contains(&page))
            || node.children.iter().any(|child| shared(child, page))
    }
    if let Some(fields) = optional(&source, form, b"Fields")? {
        let mut seen = HashSet::new();
        for field in resolve(&source, fields)?.as_array().map_err(bad)? {
            let node = read_node(
                &source,
                field.as_reference().map_err(bad)?,
                None,
                &placements,
                &mut seen,
                0,
            )?;
            if shared(&node, page_index) {
                return Err(bad("this page has shared fields on other pages; existing-text editing cannot yet keep those fields linked"));
            }
        }
    }
    project_page_for_text_transaction(request, copied)
}

fn text_page_index(request: &SourceForm<'_>) -> EngineResult<usize> {
    match request.pages.as_slice() {
        [(page, 0)] => Ok(*page),
        _ => Err(bad("text page preparation requires one source page mapped to output page zero")),
    }
}

/// A temporary working page only. Callers must publish into the complete source
/// before returning an edited document; this projection cannot own shared fields.
pub(crate) fn project_page_for_text_transaction(
    request: &SourceForm<'_>, copied: &[u8],
) -> EngineResult<Option<Vec<u8>>> {
    text_page_index(request)?;
    let source = request.document()?;
    if optional(&source, source.catalog().map_err(bad)?, b"AcroForm")?.is_none() {
        return Ok(None);
    }
    let mut output = load(copied)?;
    preserve(&mut output, std::slice::from_ref(request))?;
    let mut bytes = vec![];
    output.save_to(&mut bytes).map_err(bad)?;
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(hidden: usize) -> Vec<u8> {
        let mut d = Document::with_version("1.7");
        let pages = d.new_object_id();
        let page = d.new_object_id();
        let widget=d.add_object(dictionary!{"Subtype"=>"Widget","FT"=>"Tx","T"=>Object::string_literal("name"),"Rect"=>vec![0.into(),0.into(),20.into(),20.into()]});
        let mut fields = vec![Object::Reference(widget)];
        for i in 0..hidden {
            fields.push(Object::Reference(d.add_object(
                dictionary! {"FT"=>"Tx","T"=>Object::string_literal(format!("hidden{i}"))},
            )));
        }
        d.objects.insert(page,Object::Dictionary(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()],"Annots"=>vec![Object::Reference(widget)]}));
        d.objects.insert(
            pages,
            Object::Dictionary(
                dictionary! {"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]},
            ),
        );
        let root=d.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages,"AcroForm"=>dictionary!{"Fields"=>fields}});
        d.trailer.set("Root", root);
        let mut bytes = vec![];
        d.save_to(&mut bytes).unwrap();
        bytes
    }
    #[test]
    fn composition_parse_reuse_is_immutable_and_bound_to_one_source() {
        let bytes = source(0);
        let mut source = SourceForm::new(&bytes, vec![(0,0)]);
        assert!(matches!(source.document().unwrap(), std::borrow::Cow::Owned(_)));
        source.enable_parse_reuse();
        let first = source.document().unwrap();
        let second = source.document().unwrap();
        assert!(matches!(first, std::borrow::Cow::Borrowed(_)), "enabled parse must be borrowed");
        assert!(std::ptr::eq(first.as_ref(), second.as_ref()), "one source must be parsed only once");
        assert_eq!(first.get_pages().len(), 1);
        preflight(std::slice::from_ref(&source)).unwrap();
        assert!(std::ptr::eq(first.as_ref(), source.document().unwrap().as_ref()));
    }
    #[test]
    fn composition_parse_reuse_rejects_malformed_input_on_every_attempt() {
        let mut source = SourceForm::new(b"%PDF-1.7\ninvalid", vec![]);
        source.enable_parse_reuse();
        assert!(source.document().is_err());
        assert!(source.document().is_err());
    }
    #[test]
    fn composition_parse_reuse_keeps_large_sources_uncached() {
        let mut doc = load(&source(0)).unwrap();
        doc.add_object(Stream::new(Dictionary::new(), vec![b' '; 8 * 1024 * 1024]));
        let mut bytes = vec![];
        doc.save_to(&mut bytes).unwrap();
        let mut source = SourceForm::new(&bytes, vec![(0,0)]);
        source.enable_parse_reuse();
        assert!(matches!(source.document().unwrap(), std::borrow::Cow::Owned(_)));
    }
    #[test]
    fn composition_parse_reuse_keeps_object_dense_sources_uncached() {
        let mut doc = load(&source(0)).unwrap();
        while doc.objects.len() <= 100_000 { doc.add_object(Object::Integer(42)); }
        let mut bytes = vec![];
        doc.save_to(&mut bytes).unwrap();
        assert!(bytes.len() < 8 * 1024 * 1024);
        let mut source = SourceForm::new(&bytes, vec![(0,0)]);
        source.enable_parse_reuse();
        assert!(matches!(source.document().unwrap(), std::borrow::Cow::Owned(_)));
    }
    #[test]
    fn composition_parse_reuse_does_not_cache_mapping_validation() {
        let bytes = source(0);
        let mut source = SourceForm::new(&bytes, vec![(0,0)]);
        source.enable_parse_reuse();
        preflight(std::slice::from_ref(&source)).unwrap();
        source.pages = (0..4000).map(|i| (0,i)).collect();
        assert!(preflight(std::slice::from_ref(&source)).is_err());
        source.pages = vec![(1,0)];
        assert!(preflight(std::slice::from_ref(&source)).is_err());
    }
    #[test]
    fn text_page_parse_reuse_checks_mapping_after_cache_fill() {
        let bytes = source(0);
        let mut request = SourceForm::new(&bytes, vec![(0,0)]);
        request.enable_parse_reuse();
        request.document().unwrap();
        for pages in [vec![], vec![(0,1)], vec![(0,0),(0,0)]] {
            request.pages = pages;
            for result in [preserve_page_copy(&request, &bytes), project_page_for_text_transaction(&request, &bytes)] {
                assert!(result.unwrap_err().to_string().contains("one source page"));
            }
        }
        request.pages = vec![(0,0)];
        assert!(project_page_for_text_transaction(&request, &bytes).unwrap().is_some());
    }
    #[test]
    fn preflight_bounds_duplicate_widget_expansion_before_allocation() {
        let bytes = source(0);
        assert!(preflight(&[SourceForm::new(&bytes, (0..3999).map(|i| (0, i)).collect())])
        .is_ok());
        assert!(preflight(&[SourceForm::new(&bytes, (0..4000).map(|i| (0, i)).collect())])
        .is_err());
    }
    #[test]
    fn preflight_counts_nodes_across_sources() {
        let bytes = source(2000);
        assert!(preflight(&[
            SourceForm::new(&bytes, vec![(0, 0)]),
            SourceForm::new(&bytes, vec![(0, 1)])
        ])
        .is_err());
    }
}
