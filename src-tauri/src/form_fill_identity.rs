//! Private field correspondence for publication into the original PDF graph.
#![allow(dead_code)] // Staged ahead of the preserving publisher.
use crate::{EngineError, EngineResult, FillFormFieldRequest};
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::{
    collections::{BTreeMap, HashSet},
    io::Write,
};

const MAX_NODES: usize = 4000;
const MAX_VALUES: usize = 100_000;
const MAX_BYTES: usize = 64 * 1024 * 1024;
const ANSWER_KEYS: &[&[u8]] = &[b"V", b"I", b"TI", b"AS"];

pub(crate) struct FillIdentity {
    pub(crate) original: Document,
    marker: Vec<u8>,
    graph: Graph,
    tokens: Vec<ObjectId>,
    selected: ObjectId,
    owner: ObjectId,
    page: usize,
    annotation: usize,
}
pub(crate) struct NodeBinding {
    pub(crate) original: ObjectId,
    pub(crate) native: ObjectId,
    pub(crate) widget: bool,
    pub(crate) selected: bool,
    pub(crate) placements: Vec<(usize, usize)>,
}
pub(crate) struct AnswerChange {
    pub(crate) original: ObjectId,
    pub(crate) key: Vec<u8>,
    pub(crate) value: Option<Object>,
}
pub(crate) struct BoundFill {
    pub(crate) nodes: Vec<NodeBinding>,
    pub(crate) changes: Vec<AnswerChange>,
}
fn bad(reason: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("Form answer preservation: {reason}"))
}
fn resolve<'a>(doc: &'a Document, mut value: &'a Object) -> EngineResult<&'a Object> {
    let mut seen = HashSet::new();
    while let Object::Reference(id) = value {
        if seen.len() >= 64 || !seen.insert(*id) {
            return Err(bad("cyclic or deep reference"));
        }
        value = doc
            .objects
            .get(id)
            .ok_or_else(|| bad("missing referenced object"))?;
    }
    Ok(value)
}
fn reference(doc: &Document, value: &Object) -> EngineResult<ObjectId> {
    let mut id = value.as_reference().map_err(bad)?;
    let mut seen = HashSet::new();
    loop {
        if seen.len() >= 64 || !seen.insert(id) {
            return Err(bad("cyclic or deep owner reference"));
        }
        match doc.objects.get(&id).ok_or_else(|| bad("missing owner"))? {
            Object::Reference(next) => id = *next,
            Object::Dictionary(_) => return Ok(id),
            _ => return Err(bad("field owner is not a dictionary")),
        }
    }
}
fn dict<'a>(doc: &'a Document, value: &'a Object) -> EngineResult<&'a Dictionary> {
    resolve(doc, value)?.as_dict().map_err(bad)
}
fn optional<'a>(
    doc: &'a Document,
    d: &'a Dictionary,
    key: &[u8],
) -> EngineResult<Option<&'a Object>> {
    d.get(key)
        .ok()
        .map(|v| resolve(doc, v))
        .transpose()
        .map(|v| v.filter(|v| !matches!(v, Object::Null)))
}
struct Node {
    parent: Option<ObjectId>,
    declared_parent: Option<ObjectId>,
    declared_page: Option<ObjectId>,
    owner: ObjectId,
    widget: bool,
    field: bool,
    children: Vec<ObjectId>,
    placements: Vec<(usize, usize)>,
}
struct Graph {
    roots: Vec<ObjectId>,
    nodes: BTreeMap<ObjectId, Node>,
    annotation_counts: Vec<usize>,
    page_positions: BTreeMap<ObjectId, Vec<usize>>,
}
impl Graph {
    fn scan(doc: &Document) -> EngineResult<Self> {
        if doc.objects.len() > MAX_VALUES {
            return Err(bad("document has too many objects"));
        }
        let mut result = Self {
            roots: vec![],
            nodes: BTreeMap::new(),
            annotation_counts: vec![],
            page_positions: BTreeMap::new(),
        };
        let form = dict(
            doc,
            doc.catalog().map_err(bad)?.get(b"AcroForm").map_err(bad)?,
        )?;
        let roots = optional(doc, form, b"Fields")?
            .ok_or_else(|| bad("missing fields"))?
            .as_array()
            .map_err(bad)?;
        if roots.len() > MAX_NODES {
            return Err(bad("too many root fields"));
        }
        let mut budget = crate::form_graph::CopyBudget::with_byte_limit(MAX_BYTES);
        budget.dictionary(form)?;
        for value in roots {
            let id = reference(doc, value)?;
            result.visit(doc, id, None, None, 0, &mut budget)?;
            result.roots.push(id);
        }
        let mut placements = 0usize;
        let pages = doc.get_pages();
        if pages.len() > MAX_VALUES {
            return Err(bad("too many page references"));
        }
        for (page, id) in pages.values().enumerate() {
            let canonical = reference(doc, &Object::Reference(*id))?;
            result
                .page_positions
                .entry(canonical)
                .or_default()
                .push(page);
            let p = doc.get_dictionary(*id).map_err(bad)?;
            let annotations = optional(doc, p, b"Annots")?
                .map(|v| v.as_array().map_err(bad))
                .transpose()?;
            let annotations = annotations.map(Vec::as_slice).unwrap_or_default();
            if annotations.len() > MAX_VALUES {
                return Err(bad("too many page annotations"));
            }
            budget.array(annotations)?;
            result.annotation_counts.push(annotations.len());
            for (index, value) in annotations.iter().enumerate() {
                let d = dict(doc, value)?;
                if optional(doc, d, b"Subtype")?.and_then(|v| v.as_name().ok()) != Some(b"Widget") {
                    continue;
                }
                let id = reference(doc, value)?;
                let node = result
                    .nodes
                    .get_mut(&id)
                    .ok_or_else(|| bad("widget is outside the field tree"))?;
                if !node.widget {
                    return Err(bad("page annotation has a different field role"));
                }
                placements += 1;
                if placements > MAX_NODES {
                    return Err(bad("too many widget placements"));
                }
                node.placements.push((page, index));
            }
        }
        for node in result.nodes.values() {
            if let Some(id) = node.declared_page {
                let positions = result
                    .page_positions
                    .get(&id)
                    .ok_or_else(|| bad("widget page is outside the page tree"))?;
                if !node.placements.is_empty()
                    && !node
                        .placements
                        .iter()
                        .any(|(page, _)| positions.contains(page))
                {
                    return Err(bad("widget page disagrees with its placements"));
                }
            }
        }
        Ok(result)
    }
    fn visit(
        &mut self,
        doc: &Document,
        id: ObjectId,
        parent: Option<ObjectId>,
        owner: Option<ObjectId>,
        depth: usize,
        budget: &mut crate::form_graph::CopyBudget,
    ) -> EngineResult<()> {
        if depth >= 64 || self.nodes.len() >= MAX_NODES || self.nodes.contains_key(&id) {
            return Err(bad("cyclic, repeated or excessive field tree"));
        }
        let d = doc.get_dictionary(id).map_err(bad)?;
        // Charge all direct data, including indirect type targets, before cloning
        // or invoking the legacy graph admission which owns node dictionaries.
        budget.dictionary(d)?;
        for key in [b"Type".as_slice(), b"Subtype", b"FT"] {
            if let Some(value) = optional(doc, d, key)? {
                budget.array(std::slice::from_ref(value))?;
            }
        }
        let widget =
            optional(doc, d, b"Subtype")?.and_then(|v| v.as_name().ok()) == Some(b"Widget");
        let field = !widget || parent.is_none() || d.has(b"T") || d.has(b"FT");
        let owner = if field {
            id
        } else {
            owner.ok_or_else(|| bad("widget has no field owner"))?
        };
        let declared_parent = if optional(doc, d, b"Parent")?.is_some() {
            Some(reference(doc, d.get(b"Parent").map_err(bad)?)?)
        } else {
            None
        };
        let declared_page = if widget && optional(doc, d, b"P")?.is_some() {
            Some(reference(doc, d.get(b"P").map_err(bad)?)?)
        } else {
            None
        };
        if declared_parent.is_some() && declared_parent != parent {
            return Err(bad("field parent disagrees with hierarchy"));
        }
        let kids = optional(doc, d, b"Kids")?
            .map(|v| v.as_array().map_err(bad))
            .transpose()?;
        if kids.is_some_and(|v| v.len() > MAX_NODES) || (widget && kids.is_some()) {
            return Err(bad("invalid widget children"));
        }
        self.nodes.insert(
            id,
            Node {
                parent,
                declared_parent,
                declared_page,
                owner,
                widget,
                field,
                children: vec![],
                placements: vec![],
            },
        );
        for child in kids.into_iter().flatten() {
            let child = reference(doc, child)?;
            self.visit(doc, child, Some(id), Some(owner), depth + 1, budget)?;
            self.nodes.get_mut(&id).unwrap().children.push(child);
        }
        Ok(())
    }
    fn at(&self, page: usize, annotation: usize) -> EngineResult<ObjectId> {
        self.nodes
            .iter()
            .find_map(|(id, n)| n.placements.contains(&(page, annotation)).then_some(*id))
            .ok_or_else(|| bad("selected annotation is not a bound form widget"))
    }
    fn button(&self, doc: &Document, mut id: ObjectId) -> EngineResult<bool> {
        for _ in 0..64 {
            let d = doc.get_dictionary(id).map_err(bad)?;
            if let Ok(value) = d.get(b"FT") {
                return Ok(resolve(doc, value)?.as_name().ok() == Some(b"Btn"));
            }
            match self.nodes.get(&id).and_then(|n| n.parent) {
                Some(parent) => id = parent,
                None => return Ok(false),
            }
        }
        Err(bad("field type inheritance is too deep"))
    }
}
#[derive(Default)]
struct ValueBudget {
    values: usize,
    bytes: usize,
}
impl ValueBudget {
    fn take(&mut self, bytes: usize, depth: usize) -> EngineResult<()> {
        self.values = self
            .values
            .checked_add(1)
            .ok_or_else(|| bad("value traversal overflow"))?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| bad("value byte overflow"))?;
        if depth > 64 || self.values > MAX_VALUES || self.bytes > MAX_BYTES {
            return Err(bad("field values exceed preservation limits"));
        }
        Ok(())
    }
}
fn markers(doc: &Document, key: &[u8]) -> EngineResult<Vec<(ObjectId, usize)>> {
    fn walk(
        value: &Object,
        id: Option<ObjectId>,
        key: &[u8],
        depth: usize,
        budget: &mut ValueBudget,
        out: &mut Vec<(ObjectId, usize)>,
    ) -> EngineResult<()> {
        budget.take(0, depth)?;
        let d = match value {
            Object::Dictionary(d) => d,
            Object::Stream(s) => &s.dict,
            Object::Array(a) => {
                for v in a {
                    walk(v, None, key, depth + 1, budget, out)?;
                }
                return Ok(());
            }
            _ => return Ok(()),
        };
        if let Ok(token) = d.get(key) {
            let n = token.as_i64().map_err(bad)?;
            if n < 0 || n as u64 >= MAX_NODES as u64 || out.len() >= MAX_NODES {
                return Err(bad("invalid or excessive probe marker"));
            }
            out.push((
                id.ok_or_else(|| bad("probe marker on an inline object"))?,
                n as usize,
            ));
        }
        for (_, v) in d.iter() {
            walk(v, None, key, depth + 1, budget, out)?;
        }
        Ok(())
    }
    let mut out = vec![];
    let mut budget = ValueBudget::default();
    for (id, value) in &doc.objects {
        walk(value, Some(*id), key, 0, &mut budget, &mut out)?;
    }
    Ok(out)
}
fn equal(
    a: &Document,
    av: &Object,
    b: &Document,
    bv: &Object,
    budget: &mut ValueBudget,
    depth: usize,
) -> EngineResult<bool> {
    budget.take(0, depth)?;
    let av = resolve(a, av)?;
    let bv = resolve(b, bv)?;
    Ok(match (av, bv) {
        (Object::String(x, _), Object::String(y, _)) | (Object::Name(x), Object::Name(y)) => {
            budget.take(x.len().saturating_add(y.len()), depth)?;
            x == y
        }
        (Object::Array(x), Object::Array(y)) => {
            if x.len() != y.len() {
                return Ok(false);
            }
            if x.len() > MAX_VALUES {
                return Err(bad("excessive field array"));
            }
            for (x, y) in x.iter().zip(y) {
                if !equal(a, x, b, y, budget, depth + 1)? {
                    return Ok(false);
                }
            }
            true
        }
        (Object::Null, Object::Null) => true,
        (Object::Boolean(x), Object::Boolean(y)) => x == y,
        (Object::Integer(x), Object::Integer(y)) => x == y,
        (Object::Real(x), Object::Real(y)) if x.is_finite() && y.is_finite() => x == y,
        (Object::Integer(x), Object::Real(y)) | (Object::Real(y), Object::Integer(x))
            if y.is_finite() =>
        {
            *x as f64 == *y as f64
        }
        (Object::Dictionary(_) | Object::Stream(_), _)
        | (_, Object::Dictionary(_) | Object::Stream(_)) => {
            return Err(bad("unsupported field value graph"))
        }
        _ => false,
    })
}
fn same_entry(
    a: &Document,
    ad: &Dictionary,
    b: &Document,
    bd: &Dictionary,
    key: &[u8],
    budget: &mut ValueBudget,
) -> EngineResult<bool> {
    match (ad.get(key).ok(), bd.get(key).ok()) {
        (None, None) => Ok(true),
        (Some(x), Some(y)) => equal(a, x, b, y, budget, 0),
        _ => Ok(false),
    }
}
fn copy_answer(
    doc: &Document,
    value: &Object,
    key: &[u8],
    budget: &mut ValueBudget,
    depth: usize,
) -> EngineResult<Object> {
    budget.take(64, depth)?;
    let value = resolve(doc, value)?;
    match value {
        Object::Null if key == b"V" => Ok(Object::Null),
        Object::String(raw, format) if key == b"V" => {
            if raw.len() > 131_075 {
                return Err(bad(
                    "answer string exceeds its bounded native representation",
                ));
            }
            budget.take(raw.len(), depth)?;
            Ok(Object::String(raw.clone(), *format))
        }
        Object::Name(name) if key == b"V" || key == b"AS" => {
            if name.len() > 131_072 {
                return Err(bad("answer state exceeds its limit"));
            }
            budget.take(name.len(), depth)?;
            Ok(Object::Name(name.clone()))
        }
        Object::Integer(n) if (key == b"I" || key == b"TI") && *n >= 0 => Ok(Object::Integer(*n)),
        Object::Array(values) if key == b"V" || key == b"I" => {
            if depth != 0 || values.len() > MAX_VALUES {
                return Err(bad("invalid answer array"));
            }
            budget.take(
                values
                    .len()
                    .checked_mul(std::mem::size_of::<Object>())
                    .ok_or_else(|| bad("array size overflow"))?,
                depth,
            )?;
            let mut out = Vec::new();
            out.try_reserve_exact(values.len()).map_err(bad)?;
            for value in values {
                out.push(copy_answer(doc, value, key, budget, depth + 1)?);
            }
            Ok(Object::Array(out))
        }
        _ => Err(bad("native answer has an unsupported value type")),
    }
}
impl FillIdentity {
    pub(crate) fn selected_widgets(&self)->Vec<ObjectId> {
        self.graph.nodes.iter().filter_map(|(id,n)|(n.widget&&n.owner==self.owner).then_some(*id)).collect()
    }
    pub(crate) fn selected_placements(&self)->Vec<(usize,usize)> {
        self.graph.nodes.values().filter(|n|n.widget&&n.owner==self.owner).flat_map(|n|n.placements.iter().copied()).collect()
    }
    pub(crate) fn widget_placements(&self)->Vec<(usize,usize)> {
        let mut placements:Vec<_>=self.graph.nodes.values().filter(|n|n.widget).flat_map(|n|n.placements.iter().copied()).collect();
        placements.sort_unstable();placements
    }
    pub(crate) fn prepare(bytes: &[u8], request: &FillFormFieldRequest) -> EngineResult<Self> {
        let original = crate::pdf_output::load(bytes)?;
        let graph = Graph::scan(&original)?;
        crate::acroforms::validate_filling_document(bytes, &original)?;
        let selected = graph.at(request.page_index, request.annotation_index)?;
        let owner = graph.nodes[&selected].owner;
        let marker = format!("FolioFillProbe_{}", uuid::Uuid::new_v4().simple()).into_bytes();
        if !markers(&original, &marker)?.is_empty() {
            return Err(bad("probe marker collides with source metadata"));
        }
        let tokens = graph.nodes.keys().copied().collect();
        Ok(Self {
            original,
            marker,
            graph,
            tokens,
            selected,
            owner,
            page: request.page_index,
            annotation: request.annotation_index,
        })
    }
    pub(crate) fn probe_bytes(&mut self) -> EngineResult<Vec<u8>> {
        let mut writer = crate::pdf_output::BoundedWriter::new(512 * 1024 * 1024);
        self.write_probe(&mut writer)?;
        writer.finish()
    }
    fn write_probe<W: Write>(&mut self, writer: &mut W) -> EngineResult<()> {
        let mut budget = crate::form_graph::CopyBudget::default();
        for (token, id) in self.tokens.iter().enumerate() {
            if self
                .original
                .get_dictionary(*id)
                .map_err(bad)?
                .has(&self.marker)
            {
                return Err(bad("temporary marker already present"));
            }
            budget.entry(&self.marker, &Object::Integer(token as i64))?;
        }
        for (token, id) in self.tokens.iter().enumerate() {
            self.original
                .get_dictionary_mut(*id)
                .unwrap()
                .set(self.marker.clone(), token as i64);
        }
        let saved = self.original.save_to(writer).map_err(bad);
        // Serialization errors must not leave the publication graph marked.
        for id in &self.tokens {
            self.original
                .get_dictionary_mut(*id)
                .unwrap()
                .remove(&self.marker);
        }
        saved
    }
    pub(crate) fn bind_native(&self, native: &Document) -> EngineResult<BoundFill> {
        let graph = Graph::scan(native)?;
        let marked = markers(native, &self.marker)?;
        if marked.len() != self.tokens.len()
            || graph.nodes.len() != self.tokens.len()
            || graph.annotation_counts != self.graph.annotation_counts
        {
            return Err(bad("native field or page membership changed"));
        }
        let mut mapping = BTreeMap::new();
        let mut seen = HashSet::new();
        for (id, token) in marked {
            let original = *self
                .tokens
                .get(token)
                .ok_or_else(|| bad("unknown probe token"))?;
            if !graph.nodes.contains_key(&id)
                || !seen.insert(original)
                || mapping.insert(id, original).is_some()
            {
                return Err(bad("repeated or detached probe marker"));
            }
        }
        let mapped = |id: ObjectId| {
            mapping
                .get(&id)
                .copied()
                .ok_or_else(|| bad("unbound native owner"))
        };
        if graph
            .roots
            .iter()
            .map(|id| mapped(*id))
            .collect::<EngineResult<Vec<_>>>()?
            != self.graph.roots
            || mapped(graph.at(self.page, self.annotation)?)? != self.selected
        {
            return Err(bad("native field ordering changed"));
        }
        let mut nodes = vec![];
        let mut changes = vec![];
        let mut compare = ValueBudget::default();
        let mut copied = ValueBudget::default();
        for (&id, node) in &graph.nodes {
            let original = mapped(id)?;
            let old = &self.graph.nodes[&original];
            if node.widget != old.widget
                || node.field != old.field
                || node.parent.map(&mapped).transpose()? != old.parent
                || node.declared_parent.map(&mapped).transpose()? != old.declared_parent
                || mapped(node.owner)? != old.owner
                || node.declared_page.map(|p| &graph.page_positions[&p])
                    != old.declared_page.map(|p| &self.graph.page_positions[&p])
                || node
                    .children
                    .iter()
                    .map(|id| mapped(*id))
                    .collect::<EngineResult<Vec<_>>>()?
                    != old.children
                || node.placements != old.placements
            {
                return Err(bad("native field roles or topology changed"));
            }
            let a = self.original.get_dictionary(original).map_err(bad)?;
            let b = native.get_dictionary(id).map_err(bad)?;
            for key in [
                b"FT".as_slice(),
                b"T",
                b"TU",
                b"TM",
                b"Ff",
                b"DV",
                b"Opt",
                b"MaxLen",
                b"DA",
                b"Q",
                b"DS",
                b"RV",
                b"Rect",
                b"F",
            ] {
                if !same_entry(&self.original, a, native, b, key, &mut compare)? {
                    return Err(bad("native filling changed field settings"));
                }
            }
            for &key in ANSWER_KEYS {
                if same_entry(&self.original, a, native, b, key, &mut compare)? {
                    continue;
                }
                let allowed = if key == b"AS" {
                    old.widget
                        && old.owner == self.owner
                        && self.graph.button(&self.original, original)?
                } else {
                    original == self.owner
                };
                if !allowed {
                    return Err(bad("native filling changed an unrelated answer"));
                }
                let value = b
                    .get(key)
                    .ok()
                    .map(|value| copy_answer(native, value, key, &mut copied, 0))
                    .transpose()?;
                changes.push(AnswerChange {
                    original,
                    key: key.to_vec(),
                    value,
                });
            }
            nodes.push(NodeBinding {
                original,
                native: id,
                widget: old.widget,
                selected: old.owner == self.owner,
                placements: old.placements.clone(),
            });
        }
        Ok(BoundFill { nodes, changes })
    }
}

#[cfg(test)]
#[path = "form_fill_identity_tests.rs"]
pub(crate) mod tests;
