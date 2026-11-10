//! Prove selected MCIDs against the complete original document, never a projection.
use super::{invalid, text_context::ShowContext, EngineResult};
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::collections::{HashMap, HashSet};

const MAX_NODES: usize = 65_536;
const MAX_ENTRIES: usize = 100_000;
fn bad() -> super::EngineError {
    invalid("The original text tag ownership cannot be preserved safely.")
}
fn resolve<'a>(
    pdf: &'a Document,
    mut value: &'a Object,
) -> EngineResult<(Option<ObjectId>, &'a Object)> {
    let mut seen = HashSet::new();
    let mut resolved = None;
    while let Object::Reference(id) = value {
        if seen.len() >= 64 || !seen.insert(*id) {
            return Err(bad());
        }
        resolved = Some(*id);
        value = pdf.objects.get(id).ok_or_else(bad)?;
    }
    Ok((resolved, value))
}
fn reference(pdf: &Document, value: &Object) -> EngineResult<ObjectId> {
    resolve(pdf, value)?.0.ok_or_else(bad)
}
fn page_reference(pdf: &Document, value: &Object) -> EngineResult<ObjectId> {
    let (id, value) = resolve(pdf, value)?;
    let dict = value.as_dict().map_err(|_| bad())?;
    if dict.get(b"Type").and_then(Object::as_name).ok() != Some(b"Page") {
        return Err(bad());
    }
    id.ok_or_else(bad)
}
fn mcid(value: &Object) -> EngineResult<i64> {
    let value = value.as_i64().map_err(|_| bad())?;
    if !(0..=65_535).contains(&value) {
        return Err(bad());
    }
    Ok(value)
}
fn no_stale_semantics(dict: &Dictionary) -> bool {
    ![
        b"ActualText".as_slice(),
        b"Alt",
        b"E",
        b"C",
        b"Ref",
        b"AF",
    ]
    .iter()
    .any(|key| dict.has(key))
}

// Table relationships do not depend on the cell's visible text. Preserve these
// attributes verbatim; never admit layout geometry, summaries, or custom owners.
// This is not a table-remediation pass: existing IDs/topology are not rewritten.
fn table_relationship_attributes(pdf: &Document, dict: &Dictionary, role: &[u8], remaining: &mut usize) -> bool {
    let Ok(value) = dict.get(b"A") else { return true; };
    if !matches!(role, b"TH" | b"TD") { return false; }
    // A shared indirect attribute array may occur on many structure elements.
    // Bound cumulative validation work as well as each individual collection.
    if *remaining == 0 { return false; }
    *remaining -= 1;
    let mut valid_dictionary = |value: &Object| -> EngineResult<()> {
        *remaining = remaining.checked_sub(1).ok_or_else(bad)?;
        let attributes = resolve(pdf, value)?.1.as_dict().map_err(|_| bad())?;
        if attributes.len() > 5
            || resolve(pdf, attributes.get(b"O").map_err(|_| bad())?)?.1.as_name().ok() != Some(b"Table")
        { return Err(bad()); }
        for (key, value) in attributes.iter() {
            *remaining = remaining.checked_sub(1).ok_or_else(bad)?;
            let value = resolve(pdf, value)?.1;
            match key.as_slice() {
                b"O" => {},
                b"RowSpan" | b"ColSpan" if value.as_i64().is_ok_and(|n| n > 0) => {},
                b"Scope" if role == b"TH"
                    && value.as_name().is_ok_and(|name| matches!(name, b"Row" | b"Column" | b"Both")) => {},
                b"Headers" => {
                    let headers = value.as_array().map_err(|_| bad())?;
                    if headers.is_empty() || headers.len() > 256 { return Err(bad()); }
                    for header in headers {
                        *remaining = remaining.checked_sub(1).ok_or_else(bad)?;
                        let id = resolve(pdf, header)?.1.as_str().map_err(|_| bad())?;
                        if id.is_empty() || id.len() > 1024 { return Err(bad()); }
                    }
                },
                _ => return Err(bad()),
            }
        }
        Ok(())
    };
    let Ok((_, value)) = resolve(pdf, value) else { return false; };
    match value {
        Object::Dictionary(_) => valid_dictionary(value).is_ok(),
        Object::Array(items) => !items.is_empty() && items.len() <= 16
            && items.iter().all(|item| valid_dictionary(item).is_ok()),
        _ => false,
    }
}
struct Element {
    parent: ObjectId,
    safe: bool,
}
struct Structure<'a> {
    pdf: &'a Document,
    nodes: usize,
    seen: HashSet<ObjectId>,
    elements: HashMap<ObjectId, Element>,
    owners: HashMap<(ObjectId, i64), ObjectId>,
    attribute_budget: usize,
}
impl Structure<'_> {
    fn own(&mut self, page: Option<ObjectId>, id: i64, parent: ObjectId) -> EngineResult<()> {
        let page = page.ok_or_else(bad)?;
        if !self.elements.contains_key(&parent) || self.owners.insert((page, id), parent).is_some()
        {
            return Err(bad());
        }
        Ok(())
    }
    fn walk(
        &mut self,
        value: &Object,
        parent: ObjectId,
        page: Option<ObjectId>,
        depth: usize,
    ) -> EngineResult<()> {
        self.nodes += 1;
        if self.nodes > MAX_NODES || depth > 64 {
            return Err(bad());
        }
        let (id, value) = resolve(self.pdf, value)?;
        if id.is_some_and(|id| !self.seen.insert(id)) {
            return Err(bad());
        }
        match value {
            Object::Integer(_) => self.own(page, mcid(value)?, parent),
            Object::Array(items) => {
                if items.len() > MAX_NODES - self.nodes {
                    return Err(bad());
                }
                for item in items {
                    self.walk(item, parent, page, depth + 1)?;
                }
                Ok(())
            }
            Object::Dictionary(dict) => {
                let kind = dict
                    .get(b"Type")
                    .ok()
                    .map(|v| resolve(self.pdf, v).and_then(|(_, v)| v.as_name().map_err(|_| bad())))
                    .transpose()?;
                let page = match dict.get(b"Pg") {
                    Ok(v) => Some(page_reference(self.pdf, v)?),
                    Err(_) => page,
                };
                if kind == Some(b"MCR") {
                    if dict
                        .iter()
                        .any(|(key, _)| !matches!(key.as_slice(), b"Type" | b"MCID" | b"Pg"))
                    {
                        return Err(bad());
                    }
                    return self.own(
                        page,
                        mcid(resolve(self.pdf, dict.get(b"MCID").map_err(|_| bad())?)?.1)?,
                        parent,
                    );
                }
                if kind == Some(b"OBJR") {
                    // Annotation ownership is preserved verbatim and never
                    // authorizes selected text. Stream MCIDs are not OBJRs.
                    if dict.has(b"Stm") || dict.has(b"StmOwn") || dict.has(b"MCID") {
                        return Err(bad());
                    }
                    return Ok(());
                }
                if kind.is_some() && kind != Some(b"StructElem") {
                    return Err(bad());
                }
                let id = id.ok_or_else(bad)?;
                let role = resolve(self.pdf, dict.get(b"S").map_err(|_| bad())?)?
                    .1
                    .as_name()
                    .map_err(|_| bad())?;
                if role.is_empty()
                    || role.len() > 128
                    || reference(self.pdf, dict.get(b"P").map_err(|_| bad())?)? != parent
                {
                    return Err(bad());
                }
                let safe = no_stale_semantics(dict)
                    && table_relationship_attributes(self.pdf, dict, role, &mut self.attribute_budget)
                    && dict.iter().all(|(key, _)| {
                        matches!(
                            key.as_slice(),
                            b"Type" | b"S" | b"P" | b"K" | b"Pg" | b"ID" | b"T" | b"Lang" | b"A"
                        )
                    });
                self.elements.insert(id, Element { parent, safe });
                if let Ok(children) = dict.get(b"K") {
                    self.walk(children, id, page, depth + 1)?;
                }
                Ok(())
            }
            _ => Err(bad()),
        }
    }
}

struct NumberTree<'a> {
    pdf: &'a Document,
    seen: HashSet<ObjectId>,
    nodes: usize,
    entries: HashMap<i64, &'a Object>,
}
impl<'a> NumberTree<'a> {
    fn walk(&mut self, value: &'a Object, depth: usize) -> EngineResult<(i64, i64)> {
        self.nodes += 1;
        if self.nodes > MAX_NODES || depth > 64 {
            return Err(bad());
        }
        let (id, value) = resolve(self.pdf, value)?;
        if id.is_some_and(|id| !self.seen.insert(id)) {
            return Err(bad());
        }
        let dict = value.as_dict().map_err(|_| bad())?;
        if dict.has(b"Nums") == dict.has(b"Kids") {
            return Err(bad());
        }
        let key = if dict.has(b"Nums") {
            b"Nums".as_slice()
        } else {
            b"Kids"
        };
        let values = resolve(self.pdf, dict.get(key).map_err(|_| bad())?)?
            .1
            .as_array()
            .map_err(|_| bad())?;
        if values.is_empty() || values.len() > MAX_ENTRIES * 2 {
            return Err(bad());
        }
        let mut range = None;
        if key == b"Nums" {
            if values.len() % 2 != 0 || self.entries.len() + values.len() / 2 > MAX_ENTRIES {
                return Err(bad());
            }
            for pair in values.chunks_exact(2) {
                let key = resolve(self.pdf, &pair[0])?.1.as_i64().map_err(|_| bad())?;
                if key < 0
                    || range.is_some_and(|(_, last)| key <= last)
                    || self.entries.insert(key, &pair[1]).is_some()
                {
                    return Err(bad());
                }
                range = Some((range.map_or(key, |(first, _)| first), key));
            }
        } else {
            if values.len() > MAX_NODES - self.nodes {
                return Err(bad());
            }
            for child in values {
                let (first, last) = self.walk(child, depth + 1)?;
                if range.is_some_and(|(_, prior)| first <= prior) {
                    return Err(bad());
                }
                range = Some((range.map_or(first, |(start, _)| start), last));
            }
        }
        let range = range.ok_or_else(bad)?;
        if let Ok(limits) = dict.get(b"Limits") {
            let values = resolve(self.pdf, limits)?.1.as_array().map_err(|_| bad())?;
            let [a, b] = values.as_slice() else {
                return Err(bad());
            };
            if (
                a.as_i64().map_err(|_| bad())?,
                b.as_i64().map_err(|_| bad())?,
            ) != range
            {
                return Err(bad());
            }
        }
        Ok(range)
    }
}

// Parse the original structure once per page, rather than traversing the entire
// document for every row in the editing palette.
pub(super) struct Ownership { allowed: Option<HashSet<i64>> }
impl Ownership {
pub(super) fn read(pdf: &Document, page: ObjectId) -> EngineResult<Self> {
    let catalog = pdf.catalog().map_err(|_| bad())?;
    let page_dict = pdf.get_dictionary(page).map_err(|_| bad())?;
    let Ok(root_value) = catalog.get(b"StructTreeRoot") else {
        return if !page_dict.has(b"StructParents") {
            Ok(Self {allowed:None})
        } else {
            Err(bad())
        };
    };
    let (root_id, root) = resolve(pdf, root_value)?;
    let root_id = root_id.ok_or_else(bad)?;
    let root = root.as_dict().map_err(|_| bad())?;
    if !no_stale_semantics(root)
        || root.has(b"A")
        || root.get(b"Type").and_then(Object::as_name).ok() != Some(b"StructTreeRoot")
    {
        return Err(bad());
    }
    let mut structure = Structure {
        pdf,
        nodes: 0,
        seen: HashSet::from([root_id]),
        elements: HashMap::new(),
        owners: HashMap::new(),
        attribute_budget: MAX_ENTRIES,
    };
    structure.walk(root.get(b"K").map_err(|_| bad())?, root_id, None, 0)?;
    let mut tree = NumberTree {
        pdf,
        seen: HashSet::new(),
        nodes: 0,
        entries: HashMap::new(),
    };
    tree.walk(root.get(b"ParentTree").map_err(|_| bad())?, 0)?;
    let key = resolve(pdf, page_dict.get(b"StructParents").map_err(|_| bad())?)?
        .1
        .as_i64()
        .map_err(|_| bad())?;
    let slots = resolve(pdf, tree.entries.get(&key).copied().ok_or_else(bad)?)?
        .1
        .as_array()
        .map_err(|_| bad())?;
    if slots.len() > 65_536 {
        return Err(bad());
    }
    let mut allowed=HashSet::new();
    for ((owner_page,id),owner) in &structure.owners {
        if *owner_page!=page {continue;}
        let Some(slot)=slots.get(*id as usize) else {continue;};
        if reference(pdf,slot).ok()!=Some(*owner) {continue;}
        let mut current = *owner;
        let mut seen = HashSet::new();
        let mut safe=true;
        while current != root_id {
            if seen.len() >= 64 || !seen.insert(current) {
                return Err(bad());
            }
            let element = structure.elements.get(&current).ok_or_else(bad)?;
            safe &= element.safe;
            current = element.parent;
        }
        if safe {allowed.insert(*id);}
    }
    Ok(Self {allowed:Some(allowed)})
}
pub(super) fn tagged(&self) -> bool {self.allowed.is_some()}
pub(super) fn check(&self, selected: &[ShowContext]) -> EngineResult<()> {
    if selected.is_empty() || selected.len()>4096 {return Err(bad());}
    let binding=selected[0].binding()?;
    for context in selected {
        if context.binding()?!=binding {return Err(bad());}
        match (&self.allowed,context.mcid()?) {
            (None,None)=>{},
            (Some(allowed),Some(id)) if allowed.contains(&id)=>{},
            _=>return Err(bad()),
        }
    }
    Ok(())
}
}
pub(super) fn prove(pdf: &Document, page: ObjectId, selected: &[ShowContext]) -> EngineResult<()> {
    Ownership::read(pdf,page)?.check(selected)
}

#[cfg(test)]
#[path = "text_context_tags_tests.rs"]
mod tests;
