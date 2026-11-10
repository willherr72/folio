//! Portable, bounded PDF outlines. Page references use editor keys, never PDF IDs.
use crate::{EngineError, EngineResult};
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream, StringFormat};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

#[path = "link_navigation.rs"]
pub(crate) mod links;

pub const MAX_BOOKMARKS: usize = 4000;
const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_VALUES: usize = 100_000;
const MAX_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum PdfValue {
    Null,
    Boolean(bool),
    Integer(i64),
    Real(f32),
    Name(Vec<u8>),
    String(Vec<u8>),
    Array(Vec<PdfValue>),
    Dictionary(Vec<(Vec<u8>, PdfValue)>),
    Stream {
        dictionary: Vec<(Vec<u8>, PdfValue)>,
        content: Vec<u8>,
    },
    Page(String),
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentBookmark {
    pub id: String,
    pub title: String,
    pub level: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<PdfValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<PdfValue>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub closed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[f32; 3]>,
}
fn bad(message: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("Bookmarks: {message}"))
}
#[derive(Default)]
struct Budget {
    bytes: usize,
    values: usize,
}
impl Budget {
    fn take(&mut self, bytes: usize, depth: usize) -> EngineResult<()> {
        self.bytes = self.bytes.saturating_add(bytes);
        self.values += 1;
        if self.bytes > MAX_BYTES || self.values > MAX_VALUES || depth > MAX_DEPTH {
            return Err(bad("metadata exceeds the supported size or depth limit"));
        }
        Ok(())
    }
}
fn resolve<'a>(doc: &'a Document, value: &'a Object) -> EngineResult<&'a Object> {
    Ok(resolve_at(doc, value, 0)?.0)
}
fn raw_object(doc: &Document, id: ObjectId) -> EngineResult<&Object> {
    // get_object() follows a whole chain internally, bypassing our hop budget.
    doc.objects
        .get(&id)
        .ok_or_else(|| bad("missing referenced object"))
}
fn resolve_at<'a>(
    doc: &'a Document,
    value: &'a Object,
    mut depth: usize,
) -> EngineResult<(&'a Object, usize)> {
    if depth > MAX_DEPTH {
        return Err(bad("excessively deep metadata"));
    }
    let mut value = value;
    let mut seen = HashSet::new();
    while let Object::Reference(id) = value {
        if depth >= MAX_DEPTH || !seen.insert(*id) {
            return Err(bad("cyclic or excessively deep reference"));
        }
        depth += 1;
        value = raw_object(doc, *id)?;
    }
    Ok((value, depth))
}
fn dict<'a>(doc: &'a Document, value: &'a Object) -> EngineResult<&'a Dictionary> {
    resolve(doc, value)?.as_dict().map_err(bad)
}
fn optional<'a>(
    doc: &'a Document,
    d: &'a Dictionary,
    key: &[u8],
) -> EngineResult<Option<&'a Object>> {
    let Ok(value) = d.get(key) else {
        return Ok(None);
    };
    if matches!(resolve(doc, value)?, Object::Null) {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

struct Import<'a> {
    doc: &'a Document,
    pages: HashMap<ObjectId, String>,
    // PDF 1.7 §8.2.1: Name operands use Catalog/Dests; String operands
    // use Names/Dests. Equal bytes in these namespaces are not duplicates.
    legacy_named: HashMap<Vec<u8>, &'a Object>,
    string_named: HashMap<Vec<u8>, &'a Object>,
    budget: Budget,
}
impl<'a> Import<'a> {
    fn new(doc: &'a Document) -> EngineResult<Self> {
        let pages=doc.get_pages().iter().enumerate().map(|(i,(_,id))|(*id,i.to_string())).collect();
        let mut import=Self {doc,pages,legacy_named:HashMap::new(),string_named:HashMap::new(),budget:Budget::default()};
        let catalog=doc.catalog().map_err(bad)?;
        if let Some(dests)=optional(doc,catalog,b"Dests")? {
            for (name,value) in dict(doc,dests)?.iter() {
                import.budget.take(name.len(),0)?;import.legacy_named.insert(name.clone(),value);
            }
        }
        if let Some(names)=optional(doc,catalog,b"Names")? {
            if let Some(dests)=optional(doc,dict(doc,names)?,b"Dests")? {import.names(dests,0,&mut HashSet::new())?;}
        }
        Ok(import)
    }
    fn names(
        &mut self,
        node: &'a Object,
        depth: usize,
        seen: &mut HashSet<ObjectId>,
    ) -> EngineResult<()> {
        self.budget.take(0, depth)?;
        if let Object::Reference(id) = node {
            if !seen.insert(*id) {
                return Err(bad("cyclic destination name tree"));
            }
        }
        let (node, depth) = resolve_at(self.doc, node, depth)?;
        let node = node.as_dict().map_err(bad)?;
        if let Ok(names) = node.get(b"Names") {
            let (names, names_depth) = resolve_at(self.doc, names, depth + 1)?;
            let names = names.as_array().map_err(bad)?;
            if names.len() % 2 != 0 {
                return Err(bad("invalid destination name pairs"));
            }
            for pair in names.chunks_exact(2) {
                let key = resolve_at(self.doc, &pair[0], names_depth + 1)?
                    .0
                    .as_str()
                    .map_err(bad)?;
                self.budget.take(key.len(), depth)?;
                if self.string_named.insert(key.to_vec(), &pair[1]).is_some() {
                    return Err(bad("duplicate named destination"));
                }
            }
        }
        if let Ok(kids) = node.get(b"Kids") {
            let (kids, kids_depth) = resolve_at(self.doc, kids, depth + 1)?;
            for kid in kids.as_array().map_err(bad)? {
                self.names(kid, kids_depth + 1, seen)?;
            }
        }
        Ok(())
    }
    fn destination(&mut self, value: &'a Object, depth: usize) -> EngineResult<PdfValue> {
        self.budget.take(0, depth)?;
        let (value, depth) = resolve_at(self.doc, value, depth)?;
        let value = match value {
            Object::Name(name) => *self
                .legacy_named
                .get(name)
                .ok_or_else(|| bad("unresolved named destination"))?,
            Object::String(name, _) => *self
                .string_named
                .get(name)
                .ok_or_else(|| bad("unresolved named destination"))?,
            _ => value,
        };
        let (value, depth) = resolve_at(self.doc, value, depth)?;
        if let Object::Dictionary(d) = value {
            return self.destination(d.get(b"D").map_err(bad)?, depth + 1);
        }
        let array = value.as_array().map_err(bad)?;
        if array.is_empty() {
            return Err(bad("empty local destination"));
        }
        let first = match &array[0] {
            Object::Reference(id) => self.pages.get(id).cloned(),
            Object::Integer(index) if *index >= 0 && (*index as usize) < self.pages.len() => {
                Some(index.to_string())
            }
            _ => None,
        }
        .ok_or_else(|| bad("local destination does not point to a page"))?;
        let mut result = vec![PdfValue::Page(first)];
        for v in &array[1..] {
            result.push(self.value(v, depth + 1, &mut HashSet::new())?);
        }
        let value = PdfValue::Array(result);
        validate_destination(&value)?;
        Ok(value)
    }
    fn value(
        &mut self,
        value: &'a Object,
        depth: usize,
        seen: &mut HashSet<ObjectId>,
    ) -> EngineResult<PdfValue> {
        self.budget.take(0, depth)?;
        if let Object::Reference(id) = value {
            if let Some(page) = self.pages.get(id) {
                return Ok(PdfValue::Page(page.clone()));
            }
            if !seen.insert(*id) {
                return Err(bad("cyclic action reference"));
            }
            let result = self.value(raw_object(self.doc, *id)?, depth + 1, seen);
            seen.remove(id);
            return result;
        }
        Ok(match value {
            Object::Null => PdfValue::Null,
            Object::Boolean(b) => PdfValue::Boolean(*b),
            Object::Integer(n) => PdfValue::Integer(*n),
            Object::Real(n) if n.is_finite() => PdfValue::Real(*n),
            Object::Name(bytes) => {
                self.budget.take(bytes.len(), depth)?;
                PdfValue::Name(bytes.clone())
            }
            Object::String(bytes, _) => {
                self.budget.take(bytes.len(), depth)?;
                PdfValue::String(bytes.clone())
            }
            Object::Array(array) => PdfValue::Array(
                array
                    .iter()
                    .map(|v| self.value(v, depth + 1, seen))
                    .collect::<EngineResult<_>>()?,
            ),
            Object::Dictionary(d) => PdfValue::Dictionary(self.dictionary(d, depth, seen)?),
            Object::Stream(s) => {
                self.budget.take(s.content.len(), depth)?;
                PdfValue::Stream {
                    dictionary: self.dictionary(&s.dict, depth, seen)?,
                    content: s.content.clone(),
                }
            }
            _ => return Err(bad("invalid action value")),
        })
    }
    fn dictionary(
        &mut self,
        d: &'a Dictionary,
        depth: usize,
        seen: &mut HashSet<ObjectId>,
    ) -> EngineResult<Vec<(Vec<u8>, PdfValue)>> {
        let goto = d
            .get(b"S")
            .ok()
            .and_then(|o| resolve(self.doc, o).ok())
            .and_then(|o| o.as_name().ok())
            == Some(b"GoTo");
        d.iter()
            .map(|(key, value)| {
                self.budget.take(key.len(), depth)?;
                Ok((
                    key.clone(),
                    if goto && key == b"D" {
                        self.destination(value, depth + 1)?
                    } else {
                        self.value(value, depth + 1, seen)?
                    },
                ))
            })
            .collect()
    }
    fn outlines(
        &mut self,
        first: &'a Object,
        level: usize,
        seen: &mut HashSet<ObjectId>,
        result: &mut Vec<DocumentBookmark>,
    ) -> EngineResult<()> {
        let mut next = Some(first);
        while let Some(value) = next {
            if result.len() >= MAX_BOOKMARKS || level >= MAX_DEPTH {
                return Err(bad("too many bookmarks or outline levels"));
            }
            let id = value.as_reference().map_err(bad)?;
            if !seen.insert(id) {
                return Err(bad("cyclic outline tree"));
            }
            let d = dict(self.doc, value)?;
            let title = resolve(self.doc, d.get(b"Title").map_err(bad)?)?;
            if title.as_str().map_err(bad)?.len() > 8194 {
                return Err(bad("bookmark title is too long"));
            }
            let raw = title.as_str().map_err(bad)?;
            let title = if raw.starts_with(&[0xef, 0xbb, 0xbf]) {
                std::str::from_utf8(&raw[3..]).map_err(bad)?.to_owned()
            } else {
                lopdf::decode_text_string(title).map_err(bad)?
            };
            if title.len() > 4096 {
                return Err(bad("bookmark title exceeds 4096 UTF-8 bytes"));
            }
            self.budget.take(title.len(), level)?;
            let flags = optional(self.doc, d, b"F")?
                .map(|o| resolve(self.doc, o)?.as_i64().map_err(bad))
                .transpose()?
                .unwrap_or(0);
            let color = if let Some(c) = optional(self.doc, d, b"C")? {
                let c = resolve(self.doc, c)?.as_array().map_err(bad)?;
                if c.len() != 3 {
                    return Err(bad("invalid bookmark color"));
                }
                Some([
                    resolve(self.doc, &c[0])?.as_float().map_err(bad)?,
                    resolve(self.doc, &c[1])?.as_float().map_err(bad)?,
                    resolve(self.doc, &c[2])?.as_float().map_err(bad)?,
                ])
            } else {
                None
            };
            result.push(DocumentBookmark {
                id: uuid::Uuid::new_v4().to_string(),
                title,
                level,
                destination: optional(self.doc, d, b"Dest")?
                    .map(|o| self.destination(o, 0))
                    .transpose()?,
                action: optional(self.doc, d, b"A")?
                    .map(|o| self.value(o, 0, &mut HashSet::new()))
                    .transpose()?,
                bold: flags & 2 != 0,
                italic: flags & 1 != 0,
                closed: optional(self.doc, d, b"Count")?
                    .map(|o| resolve(self.doc, o)?.as_i64().map_err(bad))
                    .transpose()?
                    .is_some_and(|n| n < 0),
                color,
            });
            if let Some(child) = optional(self.doc, d, b"First")? {
                self.outlines(child, level + 1, seen, result)?;
            }
            next = optional(self.doc, d, b"Next")?;
        }
        Ok(())
    }
}

pub fn read(bytes: &[u8]) -> EngineResult<Vec<DocumentBookmark>> {
    let doc = crate::request_parse::read(bytes,crate::request_parse::Policy::Strict64)
    .map_err(bad)?;
    let catalog = doc.catalog().map_err(bad)?;
    let Some(root) = optional(&doc, catalog, b"Outlines")? else {
        return Ok(vec![]);
    };
    let root = dict(&doc, root)?;
    let Some(first) = optional(&doc, root, b"First")? else {
        return Ok(vec![]);
    };
    let mut import = Import::new(&doc)?;
    let mut result = vec![];
    import.outlines(first, 0, &mut HashSet::new(), &mut result)?;
    validate(&result, &import.pages.values().cloned().collect())?;
    Ok(result)
}
fn entry<'a>(dictionary: &'a [(Vec<u8>, PdfValue)], key: &[u8]) -> Option<&'a PdfValue> {
    dictionary.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}
fn goto(dictionary: &[(Vec<u8>, PdfValue)]) -> bool {
    matches!(entry(dictionary,b"S"),Some(PdfValue::Name(name)) if name==b"GoTo")
}
fn dest_page(value: &PdfValue) -> Option<&str> {
    if let PdfValue::Array(a) = value {
        if let Some(PdfValue::Page(page)) = a.first() {
            return Some(page);
        }
    }
    None
}
pub fn primary_page(bookmark: &DocumentBookmark) -> Option<&str> {
    bookmark
        .destination
        .as_ref()
        .and_then(dest_page)
        .or_else(|| {
            if let Some(PdfValue::Dictionary(d)) = &bookmark.action {
                if goto(d) {
                    return entry(d, b"D").and_then(dest_page);
                }
                None
            } else {
                None
            }
        })
}
fn validate_destination(value: &PdfValue) -> EngineResult<()> {
    let PdfValue::Array(a) = value else {
        return Err(bad("destination must be an array"));
    };
    if a.len() < 2 || !matches!(&a[0], PdfValue::Page(_)) {
        return Err(bad("destination must reference a page"));
    }
    let PdfValue::Name(mode) = &a[1] else {
        return Err(bad("missing destination view"));
    };
    let count = match mode.as_slice() {
        b"Fit" | b"FitB" => 2,
        b"FitH" | b"FitV" | b"FitBH" | b"FitBV" => 3,
        b"XYZ" => 5,
        b"FitR" => 6,
        _ => return Err(bad("unknown destination view")),
    };
    if a.len() != count
        || a[2..]
            .iter()
            .any(|v| !matches!(v, PdfValue::Null | PdfValue::Integer(_) | PdfValue::Real(_)))
    {
        return Err(bad("invalid destination parameters"));
    }
    Ok(())
}
fn check_value(
    value: &PdfValue,
    keys: &HashSet<String>,
    budget: &mut Budget,
    depth: usize,
) -> EngineResult<()> {
    budget.take(0, depth)?;
    match value {
        PdfValue::Page(key) => {
            budget.take(key.len(), depth)?;
            if !keys.contains(key) {
                return Err(bad("destination page is missing"));
            }
        }
        PdfValue::Real(n) if !n.is_finite() => return Err(bad("non-finite number")),
        PdfValue::Name(bytes) | PdfValue::String(bytes) => budget.take(bytes.len(), depth)?,
        PdfValue::Array(a) => {
            for v in a {
                check_value(v, keys, budget, depth + 1)?;
            }
        }
        PdfValue::Dictionary(d) | PdfValue::Stream { dictionary: d, .. } => {
            let mut names = HashSet::new();
            for (key, v) in d {
                budget.take(key.len(), depth)?;
                if !names.insert(key) {
                    return Err(bad("duplicate dictionary key"));
                }
                check_value(v, keys, budget, depth + 1)?;
            }
            if goto(d) {
                validate_destination(
                    entry(d, b"D").ok_or_else(|| bad("GoTo action lacks destination"))?,
                )?;
            }
            if let PdfValue::Stream { content, .. } = value {
                budget.take(content.len(), depth)?;
            }
        }
        _ => (),
    }
    Ok(())
}
pub fn validate(items: &[DocumentBookmark], page_keys: &HashSet<String>) -> EngineResult<()> {
    if items.len() > MAX_BOOKMARKS {
        return Err(bad("more than 4000 bookmarks"));
    }
    let mut ids = HashSet::new();
    let mut budget = Budget::default();
    for (i, b) in items.iter().enumerate() {
        if b.id.is_empty()
            || b.id.len() > 1024
            || !ids.insert(&b.id)
            || b.title.len() > 4096
            || b.level >= MAX_DEPTH
            || (i == 0 && b.level != 0)
            || (i > 0 && b.level > items[i - 1].level + 1)
            || b.color
                .is_some_and(|c| c.iter().any(|n| !n.is_finite() || !(0.0..=1.0).contains(n)))
        {
            return Err(bad("invalid bookmark metadata"));
        }
        budget.take(b.title.len() + b.id.len(), 0)?;
        if let Some(d) = &b.destination {
            validate_destination(d)?;
            check_value(d, page_keys, &mut budget, 0)?;
        }
        if let Some(a) = &b.action {
            if !matches!(a, PdfValue::Dictionary(_)) {
                return Err(bad("action must be a dictionary"));
            }
            check_value(a, page_keys, &mut budget, 0)?;
        }
    }
    Ok(())
}
fn map_value(value: &PdfValue, map: &HashMap<String, String>) -> PdfValue {
    match value {
        PdfValue::Page(key) => map
            .get(key)
            .map(|k| PdfValue::Page(k.clone()))
            .unwrap_or(PdfValue::Null),
        PdfValue::Array(a) => PdfValue::Array(a.iter().map(|v| map_value(v, map)).collect()),
        PdfValue::Dictionary(d) => {
            let mut mapped: Vec<_> = d
                .iter()
                .map(|(k, v)| (k.clone(), map_value(v, map)))
                .collect();
            if goto(&mapped) && entry(&mapped, b"D").and_then(dest_page).is_none() {
                return PdfValue::Null;
            }
            for (key, value) in &mut mapped {
                if key == b"Next" {
                    if let PdfValue::Array(a) = value {
                        a.retain(|v| !matches!(v, PdfValue::Null));
                    }
                }
            }
            mapped.retain(|(k, v)| {
                k != b"Next"
                    || !(matches!(v, PdfValue::Null)
                        || matches!(v,PdfValue::Array(a) if a.is_empty()))
            });
            PdfValue::Dictionary(mapped)
        }
        PdfValue::Stream {
            dictionary,
            content,
        } => PdfValue::Stream {
            dictionary: dictionary
                .iter()
                .map(|(k, v)| (k.clone(), map_value(v, map)))
                .collect(),
            content: content.clone(),
        },
        _ => value.clone(),
    }
}
pub fn remap(items: &[DocumentBookmark], map: &HashMap<String, String>) -> Vec<DocumentBookmark> {
    let mut result = vec![];
    for b in items.iter().rev() {
        let removed = primary_page(b).is_some_and(|p| !map.contains_key(p));
        let has_children = result
            .last()
            .is_some_and(|c: &DocumentBookmark| c.level > b.level);
        if removed && !has_children {
            continue;
        }
        let mut b = b.clone();
        b.destination = b
            .destination
            .as_ref()
            .map(|v| map_value(v, map))
            .filter(|v| dest_page(v).is_some());
        b.action = b
            .action
            .as_ref()
            .map(|v| map_value(v, map))
            .filter(|v| !matches!(v, PdfValue::Null));
        result.push(b);
    }
    result.reverse();
    result
}
/// Validate the aggregate output before cloning any bookmark payload. Sources
/// are lazy so a refusal does not initialize every remaining source's cache.
pub(crate) fn remap_complete<'a>(sources:impl IntoIterator<Item=EngineResult<(&'a [DocumentBookmark],HashMap<String,String>)>>,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Vec<DocumentBookmark>>{
    let mut pending=Vec::new();let mut budget=Budget::default();let mut ids=HashSet::new();let mut count=0usize;let mut source_count=0usize;
    let mut sources=sources.into_iter();
    loop{
        check()?;let Some(source)=sources.next() else{break};let (items,map)=source?;
        source_count+=1;if source_count>100_000{return Err(bad("too many bookmark sources"));}
        count=count.checked_add(items.len()).filter(|n|*n<=MAX_BOOKMARKS).ok_or_else(||bad("more than 4000 bookmarks"))?;
        validate(items,&map.keys().cloned().collect())?;
        for item in items{
            check()?;if !ids.insert(item.id.as_str()){return Err(bad("duplicate bookmark identity"));}
            budget.take(item.id.len()+item.title.len(),0)?;
            for value in [&item.destination,&item.action].into_iter().flatten(){complete_value_budget(value,&map,&mut budget,check,0)?;}
        }
        if !items.is_empty(){pending.push((items,map));}
    }
    check()?;let mut output=Vec::with_capacity(count);
    for (items,map) in pending{
        for item in items{
            check()?;
            // All targets exist. Reuse the established optional-Next normalization
            // without cloning the entire bookmark and then cloning its values again.
            let destination=item.destination.as_ref().map(|v|map_value(v,&map));
            check()?;let action=item.action.as_ref().map(|v|map_value(v,&map));
            output.push(DocumentBookmark{id:item.id.clone(),title:item.title.clone(),level:item.level,destination,action,
                bold:item.bold,italic:item.italic,closed:item.closed,color:item.color});
        }
    }
    check()?;Ok(output)
}
fn complete_value_budget(value:&PdfValue,map:&HashMap<String,String>,budget:&mut Budget,check:&mut impl FnMut()->EngineResult<()>,depth:usize)->EngineResult<()>{
    mapped_value_budget(value,map,budget,check,depth,true)
}
fn mapped_value_budget(value:&PdfValue,map:&HashMap<String,String>,budget:&mut Budget,check:&mut impl FnMut()->EngineResult<()>,depth:usize,complete:bool)->EngineResult<()>{
    check()?;budget.take(0,depth)?;
    match value{
        PdfValue::Page(key)=>match map.get(key){Some(key)=>budget.take(key.len(),depth)?,None if complete=>return Err(bad("complete source destination is missing")),None=>{}},
        PdfValue::Name(bytes)|PdfValue::String(bytes)=>budget.take(bytes.len(),depth)?,
        PdfValue::Array(values)=>for value in values{mapped_value_budget(value,map,budget,check,depth+1,complete)?;},
        PdfValue::Dictionary(dictionary)|PdfValue::Stream{dictionary,..}=>{
            for (key,value) in dictionary{budget.take(key.len(),depth)?;mapped_value_budget(value,map,budget,check,depth+1,complete)?;}
            if let PdfValue::Stream{content,..}=value{budget.take(content.len(),depth)?;}
        },
        _=>{},
    }Ok(())
}
/// Selection uses the existing bookmark pruning semantics, with aggregate
/// admission before any payload cloning. Current-source page counts validate
/// original destinations even when those pages are deliberately omitted.
pub(crate) fn remap_selected<'a>(sources:impl IntoIterator<Item=EngineResult<(&'a [DocumentBookmark],HashMap<String,String>,usize)>>,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Vec<DocumentBookmark>>{
    let mut pending=Vec::new();let mut budget=Budget::default();let mut ids=HashSet::new();let mut count=0usize;let mut pages=0usize;
    let mut sources=sources.into_iter();
    loop {
        check()?;let Some(source)=sources.next() else{break};let (items,map,page_count)=source?;
        pages=pages.checked_add(page_count).filter(|n|*n<=100_000).ok_or_else(||bad("selected source page limit"))?;
        if page_count==0{return Err(bad("empty selected source"));}
        count=count.checked_add(items.len()).filter(|n|*n<=MAX_BOOKMARKS).ok_or_else(||bad("more than 4000 bookmarks"))?;
        let mut keys=HashSet::new();for i in 0..page_count{check()?;keys.insert(i.to_string());}
        if map.keys().any(|key|!keys.contains(key)){return Err(bad("unknown selected source page"));}
        validate(items,&keys)?;
        for item in items {
            check()?;if !ids.insert(item.id.as_str()){return Err(bad("duplicate bookmark identity"));}
            budget.take(item.id.len()+item.title.len(),0)?;
            for value in [&item.destination,&item.action].into_iter().flatten(){mapped_value_budget(value,&map,&mut budget,check,0,false)?;}
        }
        if !items.is_empty(){pending.push((items,map));}
    }
    let mut output=Vec::with_capacity(count);
    for (items,map) in pending {
        let start=output.len();
        for item in items.iter().rev() {
            check()?;
            let removed=primary_page(item).is_some_and(|page|!map.contains_key(page));
            let children=output.len()>start && output.last().is_some_and(|b:&DocumentBookmark|b.level>item.level);
            if removed && !children{continue;}
            let destination=item.destination.as_ref().map(|v|map_value(v,&map)).filter(|v|dest_page(v).is_some());
            let action=item.action.as_ref().map(|v|map_value(v,&map)).filter(|v|!matches!(v,PdfValue::Null));
            output.push(DocumentBookmark{id:item.id.clone(),title:item.title.clone(),level:item.level,destination,action,
                bold:item.bold,italic:item.italic,closed:item.closed,color:item.color});
        }
        output[start..].reverse();
    }
    check()?;Ok(output)
}
#[cfg(test)]
#[path="bookmark_merge_tests.rs"]
mod merge_tests;
fn pdf_value(
    value: &PdfValue,
    doc: &mut Document,
    pages: &HashMap<String, ObjectId>,
) -> EngineResult<Object> {
    Ok(match value {
        PdfValue::Null => Object::Null,
        PdfValue::Boolean(b) => Object::Boolean(*b),
        PdfValue::Integer(n) => Object::Integer(*n),
        PdfValue::Real(n) => Object::Real(*n),
        PdfValue::Name(b) => Object::Name(b.clone()),
        PdfValue::String(b) => Object::String(b.clone(), StringFormat::Hexadecimal),
        PdfValue::Page(key) => Object::Reference(
            *pages
                .get(key)
                .ok_or_else(|| bad("destination page missing"))?,
        ),
        PdfValue::Array(a) => Object::Array(
            a.iter()
                .map(|v| pdf_value(v, doc, pages))
                .collect::<EngineResult<_>>()?,
        ),
        PdfValue::Dictionary(d) => Object::Dictionary(pdf_dictionary(d, doc, pages)?),
        PdfValue::Stream {
            dictionary,
            content,
        } => {
            let dict = pdf_dictionary(dictionary, doc, pages)?;
            Object::Reference(doc.add_object(Stream::new(dict, content.clone())))
        }
    })
}
fn pdf_dictionary(
    values: &[(Vec<u8>, PdfValue)],
    doc: &mut Document,
    pages: &HashMap<String, ObjectId>,
) -> EngineResult<Dictionary> {
    let mut d = Dictionary::new();
    for (k, v) in values {
        d.set(k.clone(), pdf_value(v, doc, pages)?);
    }
    Ok(d)
}
pub fn write(path: &Path, items: &[DocumentBookmark], page_keys: &[String]) -> EngineResult<()> {
    let bytes = crate::pdf_output::read(path)?;
    let mut doc = crate::pdf_output::load(&bytes)?;
    write_document(&mut doc, items, page_keys)?;
    crate::pdf_output::publish(path, &crate::pdf_output::save(&mut doc)?)
}

pub(crate) fn write_document(doc: &mut Document, items: &[DocumentBookmark], page_keys: &[String]) -> EngineResult<()> {
    validate(items, &page_keys.iter().cloned().collect())?;
    let page_ids = doc.get_pages();
    if page_ids.len() != page_keys.len() {
        return Err(bad("export page count changed"));
    }
    let pages = page_keys
        .iter()
        .cloned()
        .zip(page_ids.values().copied())
        .collect();
    if items.is_empty() {
        doc.catalog_mut().map_err(bad)?.remove(b"Outlines");
        return Ok(());
    }
    let root = doc.new_object_id();
    let ids: Vec<_> = items.iter().map(|_| doc.new_object_id()).collect();
    let mut children = vec![vec![]; items.len() + 1];
    let mut stack = vec![];
    let mut parents = vec![];
    for (i, b) in items.iter().enumerate() {
        stack.truncate(b.level);
        let parent = stack.last().copied().unwrap_or(items.len());
        parents.push(parent);
        children[parent].push(i);
        stack.push(i);
    }
    fn count(index: usize, items: &[DocumentBookmark], children: &[Vec<usize>]) -> i64 {
        children[index]
            .iter()
            .map(|&i| {
                1 + if items[i].closed {
                    0
                } else {
                    count(i, items, children)
                }
            })
            .sum()
    }
    for (i, b) in items.iter().enumerate() {
        let parent = parents[i];
        let siblings = &children[parent];
        let pos = siblings.iter().position(|n| *n == i).unwrap();
        let mut d = dictionary! {"Title"=>lopdf::text_string(&b.title),"Parent"=>if parent==items.len(){root}else{ids[parent]}};
        if pos > 0 {
            d.set("Prev", ids[siblings[pos - 1]]);
        }
        if pos + 1 < siblings.len() {
            d.set("Next", ids[siblings[pos + 1]]);
        }
        if let Some(first) = children[i].first() {
            d.set("First", ids[*first]);
            d.set("Last", ids[*children[i].last().unwrap()]);
            let n = count(i, items, &children);
            d.set("Count", if b.closed { -n } else { n });
        }
        let flags = (if b.bold { 2 } else { 0 }) + (if b.italic { 1 } else { 0 });
        if flags != 0 {
            d.set("F", flags);
        }
        if let Some(c) = b.color {
            d.set("C", c.into_iter().map(Object::Real).collect::<Vec<_>>());
        }
        if let Some(v) = &b.destination {
            d.set("Dest", pdf_value(v, doc, &pages)?);
        }
        if let Some(v) = &b.action {
            d.set("A", pdf_value(v, doc, &pages)?);
        }
        doc.objects.insert(ids[i], Object::Dictionary(d));
    }
    let roots = &children[items.len()];
    doc.objects.insert(root,Object::Dictionary(dictionary!{"Type"=>"Outlines","First"=>ids[roots[0]],"Last"=>ids[*roots.last().unwrap()],"Count"=>count(items.len(),items,&children)}));
    doc.catalog_mut().map_err(bad)?.set("Outlines", root);
    Ok(())
}
