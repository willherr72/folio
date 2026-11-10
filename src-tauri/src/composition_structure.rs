//! Bidirectional ownership for copying unchanged logical structure graphs.
//! This does not authorize text edits or certify complete tagging/rendered coverage.
#![allow(dead_code)] // Consumed by the graph-native catalog assembly stage.
use crate::{EngineError, EngineResult};
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Parents {
    Marked(Vec<Option<ObjectId>>),
    Whole(ObjectId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Owner {
    pub object: ObjectId,
    pub parents: Parents,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct StructureInfo {
    pub root: Option<ObjectId>,
    pub top: Vec<ObjectId>,
    pub elements: BTreeSet<ObjectId>,
    pub owners: BTreeMap<i64, Owner>,
}

pub(crate) fn inspect(
    document: &Document,
    catalog: ObjectId,
    pages: &[ObjectId],
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<StructureInfo> {
    let mut work = Work { check, steps: 0 };
    let reachable = reachable(document, catalog, &mut work)?;
    let (_, catalog) = object_at(document, catalog, &mut work)?;
    let catalog = catalog.as_dict().map_err(bad)?;
    require(name(document, catalog, b"Type", &mut work)? == Some(b"Catalog"))?;
    let mut page_set = BTreeSet::new();
    for page in pages {
        let (id, value) = object_at(document, *page, &mut work)?;
        require(reachable.contains(&id) && page_set.insert(id))?;
        require(name(document, value.as_dict().map_err(bad)?, b"Type", &mut work)? == Some(b"Page"))?;
    }

    // Only live catalog-reachable dictionaries own keys. Unreachable historical
    // objects remain preserved by relocation, but cannot poison this proof.
    let mut keys = BTreeMap::new();
    for id in reachable {
        work.step(0)?;
        let dictionary = match &document.objects[&id] {
            Object::Dictionary(dictionary) => dictionary,
            Object::Stream(stream) => &stream.dict,
            _ => continue,
        };
        let whole = optional(document, dictionary, b"StructParent", &mut work)?;
        let marked = optional(document, dictionary, b"StructParents", &mut work)?;
        require(whole.is_none() || marked.is_none())?;
        if let Some(value) = whole.or(marked) {
            let key = integer(document, value, &mut work)?;
            require(key >= 0 && keys.len() < MAX_NODES)?;
            require(keys.insert(key, (id, marked.is_some())).is_none())?;
        }
    }

    let Some(root_value) = optional(document, catalog, b"StructTreeRoot", &mut work)? else {
        require(keys.is_empty())?;
        return Ok(StructureInfo::default());
    };
    let root_id = reference(document, root_value, &mut work)?;
    let root = object_at(document, root_id, &mut work)?.1.as_dict().map_err(bad)?;
    require(name(document, root, b"Type", &mut work)? == Some(b"StructTreeRoot"))?;
    let mut hierarchy = Hierarchy {
        root: root_id, top: vec![], elements: BTreeSet::new(),
        marked: BTreeMap::new(), whole: BTreeMap::new(), occurrences: BTreeSet::new(),
    };
    if let Some(kids) = optional(document, root, b"K", &mut work)? {
        children(document, kids, root_id, None, &page_set, &mut hierarchy, &mut work, 0)?;
    }
    let mut entries = BTreeMap::new();
    if let Some(tree) = optional(document, root, b"ParentTree", &mut work)? {
        number_tree(document, tree, &mut entries, &mut BTreeSet::new(), &mut work, 0)?;
    }
    if let Some(next) = optional(document, root, b"ParentTreeNextKey", &mut work)? {
        let next = integer(document, next, &mut work)?;
        require(next >= 0 && entries.keys().next_back().is_none_or(|key| next > *key))?;
    }
    require(entries.len() == keys.len())?;
    let mut owners = BTreeMap::new();
    let mut found_marked = BTreeSet::new();
    let mut found_whole = BTreeSet::new();
    for (key, (object, marked)) in keys {
        work.step(0)?;
        let value = entries.get(&key).ok_or_else(invalid)?;
        let parents = if marked {
            let values = resolve(document, value, &mut work)?.1.as_array().map_err(bad)?;
            require(values.len() <= 65_536)?;
            let mut slots = Vec::with_capacity(values.len());
            for (index, value) in values.iter().enumerate() {
                work.step(0)?;
                let resolved = resolve(document, value, &mut work)?.1;
                let parent = if matches!(resolved, Object::Null) { None }
                    else { Some(reference(document, value, &mut work)?) };
                require(parent.is_none_or(|id| hierarchy.elements.contains(&id)))?;
                require(hierarchy.marked.get(&(object, index as i64)).copied() == parent)?;
                if parent.is_some() { found_marked.insert((object, index as i64)); }
                slots.push(parent);
            }
            Parents::Marked(slots)
        } else {
            let parent = reference(document, value, &mut work)?;
            require(hierarchy.elements.contains(&parent) && hierarchy.whole.get(&object) == Some(&parent))?;
            found_whole.insert(object);
            Parents::Whole(parent)
        };
        owners.insert(key, Owner { object, parents });
    }
    require(found_marked.len() == hierarchy.marked.len() && found_whole.len() == hierarchy.whole.len())?;
    work.step(0)?;
    Ok(StructureInfo { root: Some(root_id), top: hierarchy.top, elements: hierarchy.elements, owners })
}

const MAX_NODES: usize = 100_000;
const MAX_STEPS: usize = 1_000_000;
fn bad(message: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("PDF structure ownership: {message}"))
}
fn invalid() -> EngineError { bad("inconsistent or unsupported ownership graph") }
fn require(value: bool) -> EngineResult<()> { if value { Ok(()) } else { Err(invalid()) } }
struct Work<'a> { check: &'a mut dyn FnMut() -> EngineResult<()>, steps: usize }
impl Work<'_> {
    fn step(&mut self, depth: usize) -> EngineResult<()> {
        (self.check)()?;
        self.steps += 1;
        require(depth <= 64 && self.steps <= MAX_STEPS)
    }
}
fn object_at<'a>(doc: &'a Document, mut id: ObjectId, work: &mut Work<'_>) -> EngineResult<(ObjectId, &'a Object)> {
    let mut seen = BTreeSet::new();
    loop {
        work.step(seen.len())?;
        require(seen.insert(id))?;
        let value = doc.objects.get(&id).ok_or_else(invalid)?;
        match value {
            Object::Reference(next) => id = *next,
            _ => return Ok((id, value)),
        }
    }
}
fn resolve<'a>(doc: &'a Document, value: &'a Object, work: &mut Work<'_>) -> EngineResult<(Option<ObjectId>, &'a Object)> {
    work.step(0)?;
    match value {
        Object::Reference(id) => object_at(doc, *id, work).map(|(id, value)| (Some(id), value)),
        _ => Ok((None, value)),
    }
}
fn reference(doc: &Document, value: &Object, work: &mut Work<'_>) -> EngineResult<ObjectId> {
    resolve(doc, value, work)?.0.ok_or_else(invalid)
}
fn integer(doc: &Document, value: &Object, work: &mut Work<'_>) -> EngineResult<i64> {
    resolve(doc, value, work)?.1.as_i64().map_err(bad)
}
fn optional<'a>(doc: &'a Document, dictionary: &'a Dictionary, key: &[u8], work: &mut Work<'_>) -> EngineResult<Option<&'a Object>> {
    match dictionary.get(key) {
        Ok(value) if !matches!(resolve(doc, value, work)?.1, Object::Null) => Ok(Some(value)),
        _ => Ok(None),
    }
}
fn name<'a>(doc: &'a Document, dictionary: &'a Dictionary, key: &[u8], work: &mut Work<'_>) -> EngineResult<Option<&'a [u8]>> {
    optional(doc, dictionary, key, work)?.map(|value| resolve(doc, value, work)?.1.as_name().map_err(bad)).transpose()
}

fn reachable(doc: &Document, catalog: ObjectId, work: &mut Work<'_>) -> EngineResult<BTreeSet<ObjectId>> {
    fn collect(value: &Object, pending: &mut Vec<ObjectId>, seen: &mut BTreeSet<ObjectId>, work: &mut Work<'_>, depth: usize) -> EngineResult<()> {
        work.step(depth)?;
        match value {
            Object::Reference(id) => {
                if seen.insert(*id) { require(seen.len() <= MAX_NODES)?; pending.push(*id); }
            }
            Object::Array(values) => { for value in values { collect(value, pending, seen, work, depth + 1)?; } }
            Object::Dictionary(dictionary) => {
                for (_, value) in dictionary.iter() { collect(value, pending, seen, work, depth + 1)?; }
            }
            Object::Stream(stream) => {
                for (_, value) in stream.dict.iter() { collect(value, pending, seen, work, depth + 1)?; }
            }
            _ => {}
        }
        Ok(())
    }
    let mut seen = BTreeSet::from([catalog]);
    let mut pending = vec![catalog];
    while let Some(id) = pending.pop() {
        work.step(0)?;
        collect(doc.objects.get(&id).ok_or_else(invalid)?, &mut pending, &mut seen, work, 0)?;
    }
    Ok(seen)
}

struct Hierarchy {
    root: ObjectId,
    top: Vec<ObjectId>,
    elements: BTreeSet<ObjectId>,
    marked: BTreeMap<(ObjectId, i64), ObjectId>,
    whole: BTreeMap<ObjectId, ObjectId>,
    occurrences: BTreeSet<(ObjectId, Option<i64>, ObjectId)>,
}
impl Hierarchy {
    fn own(&mut self, object: ObjectId, mcid: Option<i64>, page: ObjectId, parent: ObjectId) -> EngineResult<()> {
        require(self.elements.contains(&parent) && self.occurrences.len() < MAX_NODES)?;
        require(self.occurrences.insert((object, mcid, page)))?;
        let old = if let Some(mcid) = mcid {
            require((0..=65_535).contains(&mcid))?;
            self.marked.insert((object, mcid), parent)
        } else { self.whole.insert(object, parent) };
        require(old.is_none_or(|old| old == parent))
    }
}

fn children(doc: &Document, value: &Object, parent: ObjectId, page: Option<ObjectId>, pages: &BTreeSet<ObjectId>, hierarchy: &mut Hierarchy, work: &mut Work<'_>, depth: usize) -> EngineResult<()> {
    work.step(depth)?;
    match resolve(doc, value, work)?.1 {
        Object::Array(values) => {
            for value in values { child(doc, value, parent, page, pages, hierarchy, work, depth + 1)?; }
        }
        _ => child(doc, value, parent, page, pages, hierarchy, work, depth + 1)?,
    }
    Ok(())
}
fn child(doc: &Document, value: &Object, parent: ObjectId, mut page: Option<ObjectId>, pages: &BTreeSet<ObjectId>, hierarchy: &mut Hierarchy, work: &mut Work<'_>, depth: usize) -> EngineResult<()> {
    work.step(depth)?;
    let (id, value) = resolve(doc, value, work)?;
    if let Object::Integer(mcid) = value {
        require(parent != hierarchy.root)?;
        let page = page.ok_or_else(invalid)?;
        return hierarchy.own(page, Some(*mcid), page, parent);
    }
    let dictionary = value.as_dict().map_err(bad)?;
    if let Some(value) = optional(doc, dictionary, b"Pg", work)? {
        let id = reference(doc, value, work)?;
        require(pages.contains(&id))?;
        page = Some(id);
    }
    match name(doc, dictionary, b"Type", work)? {
        Some(b"MCR") => {
            require(parent != hierarchy.root)?;
            let page = page.ok_or_else(invalid)?;
            let object = if let Some(value) = optional(doc, dictionary, b"Stm", work)? {
                let stream = reference(doc, value, work)?;
                require(matches!(object_at(doc, stream, work)?.1, Object::Stream(_)))?;
                stream
            } else { page };
            if let Some(owner) = optional(doc, dictionary, b"StmOwn", work)? {
                require(optional(doc, dictionary, b"Stm", work)?.is_some())?;
                reference(doc, owner, work)?;
            }
            let mcid = integer(doc, dictionary.get(b"MCID").map_err(bad)?, work)?;
            hierarchy.own(object, Some(mcid), page, parent)
        }
        Some(b"OBJR") => {
            require(parent != hierarchy.root)?;
            require(!dictionary.has(b"Stm") && !dictionary.has(b"StmOwn") && !dictionary.has(b"MCID"))?;
            let object = reference(doc, dictionary.get(b"Obj").map_err(bad)?, work)?;
            hierarchy.own(object, None, page.ok_or_else(invalid)?, parent)
        }
        None | Some(b"StructElem") => {
            let id = id.ok_or_else(invalid)?;
            require(hierarchy.elements.len() < MAX_NODES && hierarchy.elements.insert(id))?;
            require(reference(doc, dictionary.get(b"P").map_err(bad)?, work)? == parent)?;
            require(name(doc, dictionary, b"S", work)?.is_some_and(|role| !role.is_empty()))?;
            if parent == hierarchy.root { hierarchy.top.push(id); }
            if let Some(value) = optional(doc, dictionary, b"K", work)? {
                children(doc, value, id, page, pages, hierarchy, work, depth + 1)?;
            }
            Ok(())
        }
        _ => Err(invalid()),
    }
}

fn number_tree<'a>(doc: &'a Document, value: &'a Object, entries: &mut BTreeMap<i64, &'a Object>, seen: &mut BTreeSet<ObjectId>, work: &mut Work<'_>, depth: usize) -> EngineResult<Option<(i64, i64)>> {
    work.step(depth)?;
    let (id, value) = resolve(doc, value, work)?;
    if let Some(id) = id { require(seen.len() < MAX_NODES && seen.insert(id))?; }
    let dictionary = value.as_dict().map_err(bad)?;
    let nums = optional(doc, dictionary, b"Nums", work)?;
    let kids = optional(doc, dictionary, b"Kids", work)?;
    require(nums.is_some() != kids.is_some())?;
    let mut range: Option<(i64, i64)> = None;
    if let Some(nums) = nums {
        let values = resolve(doc, nums, work)?.1.as_array().map_err(bad)?;
        require(values.len() % 2 == 0 && values.len() / 2 <= MAX_NODES.saturating_sub(entries.len()))?;
        for pair in values.chunks_exact(2) {
            work.step(depth)?;
            let key = integer(doc, &pair[0], work)?;
            require(key >= 0 && range.is_none_or(|(_, prior)| key > prior))?;
            require(entries.insert(key, &pair[1]).is_none())?;
            range = Some((range.map_or(key, |(first, _)| first), key));
        }
    } else {
        let values = resolve(doc, kids.unwrap(), work)?.1.as_array().map_err(bad)?;
        require(!values.is_empty())?;
        for value in values {
            let child = number_tree(doc, value, entries, seen, work, depth + 1)?.ok_or_else(invalid)?;
            require(range.is_none_or(|(_, prior)| child.0 > prior))?;
            range = Some((range.map_or(child.0, |(first, _)| first), child.1));
        }
    }
    if let Some(limits) = optional(doc, dictionary, b"Limits", work)? {
        let values = resolve(doc, limits, work)?.1.as_array().map_err(bad)?;
        require(values.len() == 2)?;
        require(range == Some((integer(doc, &values[0], work)?, integer(doc, &values[1], work)?)))?;
    }
    Ok(range)
}

#[cfg(test)]
#[path = "composition_structure_tests.rs"]
mod tests;
