//! Logical field metadata edits keep the original widget and appearance graph.
use crate::{EngineError, EngineResult};
use lopdf::{Dictionary, Document, Object, ObjectId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
#[path = "form_property_actions.rs"]
mod actions;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormProperties {
    pub expected_key: String,
    pub name: String,
    pub full_name: String,
    pub label: String,
    pub read_only: bool,
    pub required: bool,
    pub linked_widgets: usize,
    pub rename_reason: Option<String>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FormPropertyChanges {
    pub name: Option<String>,
    pub label: Option<String>,
    pub read_only: Option<bool>,
    pub required: Option<bool>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetFormPropertiesRequest {
    pub page_index: usize,
    pub annotation_index: usize,
    pub expected_key: String,
    pub changes: FormPropertyChanges,
}
pub(super) fn bad(e: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("Field properties: {e}"))
}
fn unsafe_char(c: char) -> bool {
    c.is_control()
        || matches!(c,'\u{061c}'|'\u{200e}'|'\u{200f}'|'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')
}
impl SetFormPropertiesRequest {
    pub(crate) fn validate(&self) -> EngineResult<()> {
        let c = &self.changes;
        if self.expected_key.len() > 128
            || (c.name.is_none()
                && c.label.is_none()
                && c.read_only.is_none()
                && c.required.is_none())
        {
            return Err(bad("change a setting before applying"));
        }
        if c.name.as_ref().is_some_and(|s| {
            s.trim().is_empty() || s.len() > 1024 || s.contains('.') || s.chars().any(unsafe_char)
        }) {
            return Err(bad(
                "use a field name of at most 1024 bytes without periods or control characters",
            ));
        }
        if c.label.as_ref().is_some_and(|s| {
            s.len() > 4096
                || s.chars()
                    .any(|c| unsafe_char(c) && !matches!(c, '\n' | '\r' | '\t'))
        }) {
            return Err(bad(
                "use a tooltip of at most 4096 bytes without control characters",
            ));
        }
        Ok(())
    }
}
pub(super) fn resolve<'a>(d: &'a Document, v: &'a Object) -> EngineResult<&'a Object> {
    Ok(d.dereference(v).map_err(bad)?.1)
}
pub(super) fn text(d: &Document, v: &Object, limit: usize) -> EngineResult<String> {
    let v = resolve(d, v)?;
    let raw = v.as_str().map_err(bad)?;
    if raw.len() > limit {
        return Err(bad("field text exceeds the inspection limit"));
    }
    let s = if let Some(b) = raw.strip_prefix(b"\xef\xbb\xbf") {
        std::str::from_utf8(b).map_err(bad)?.to_owned()
    } else {
        if raw.starts_with(b"\xfe\xff") && raw.len() % 2 != 0 {
            return Err(bad("incomplete UTF-16 field text"));
        }
        lopdf::decode_text_string(v).map_err(bad)?
    };
    if s.len() > limit {
        return Err(bad("decoded field text exceeds the inspection limit"));
    }
    Ok(s)
}
fn optional<'a>(
    d: &'a Document,
    node: &'a Dictionary,
    key: &[u8],
) -> EngineResult<Option<&'a Object>> {
    node.get(key)
        .ok()
        .map(|v| resolve(d, v))
        .transpose()
        .map(|v| v.filter(|v| !matches!(v, Object::Null)))
}
struct Node {
    parent: Option<ObjectId>,
    owner: ObjectId,
    widget: bool,
    field: bool,
    named: bool,
    name: String,
    full: String,
}
/// PDF field paths omit a separator while the accumulated prefix is empty.
/// Share this rule with consumers that reconstruct names for action checks.
pub(crate) fn qualified_name(prefix: &str, name: &str) -> String {
    if prefix.is_empty() { name.to_owned() } else { format!("{prefix}.{name}") }
}
fn tree(
    d: &Document,
    id: ObjectId,
    parent: Option<ObjectId>,
    owner: Option<ObjectId>,
    prefix: &str,
    depth: usize,
    nodes: &mut HashMap<ObjectId, Node>,
    bytes: &mut usize,
) -> EngineResult<()> {
    if depth > 64 || nodes.len() >= 4000 || nodes.contains_key(&id) {
        return Err(bad("cyclic or excessive field tree"));
    }
    let n = d.get_dictionary(id).map_err(bad)?;
    let widget = optional(d, n, b"Subtype")?.and_then(|v| v.as_name().ok()) == Some(b"Widget");
    let field = !widget || parent.is_none() || n.has(b"T") || n.has(b"FT");
    let partial_name = optional(d, n, b"T")?;
    let named = partial_name.is_some();
    let name = partial_name
        .map(|v| text(d, v, 32768))
        .transpose()?
        .unwrap_or_default();
    let full_len = if named {
        if prefix.is_empty() {
            name.len()
        } else {
            prefix.len() + 1 + name.len()
        }
    } else {
        prefix.len()
    };
    *bytes = bytes
        .checked_sub(name.len() + full_len * 2 + 192)
        .ok_or_else(|| bad("field model exceeds 8 MiB"))?;
    let full = if named {
        qualified_name(prefix, &name)
    } else {
        prefix.into()
    };
    let owner = if field {
        id
    } else {
        owner.ok_or_else(|| bad("widget has no logical field"))?
    };
    nodes.insert(
        id,
        Node {
            parent,
            owner,
            widget,
            field,
            named,
            name,
            full: full.clone(),
        },
    );
    if let Some(kids) = optional(d, n, b"Kids")? {
        for child in kids.as_array().map_err(bad)? {
            tree(
                d,
                child.as_reference().map_err(bad)?,
                Some(id),
                Some(owner),
                &full,
                depth + 1,
                nodes,
                bytes,
            )?;
        }
    }
    Ok(())
}
fn inherited<'a>(
    d: &'a Document,
    nodes: &HashMap<ObjectId, Node>,
    mut id: ObjectId,
    key: &[u8],
) -> EngineResult<Option<&'a Object>> {
    for _ in 0..65 {
        if let Some(v) = optional(d, d.get_dictionary(id).map_err(bad)?, key)? {
            return Ok(Some(v));
        }
        match nodes.get(&id).and_then(|n| n.parent) {
            Some(p) => id = p,
            None => return Ok(None),
        }
    }
    Err(bad("field inheritance exceeds the limit"))
}
pub(crate) struct Context {
    pub(crate) doc: Document,
    pub(crate) owner: ObjectId,
    names: HashSet<String>,
    prefix: String,
    flags: i64,
    pub(crate) properties: FormProperties,
    pub(crate) ancestors: Vec<ObjectId>,
    pub(crate) widgets: Vec<ObjectId>,
}
impl Context {
    pub(crate) fn validate_native_links(&self) -> EngineResult<()> {
        for pair in self.ancestors.windows(2) {
            let parent = self.doc.get_dictionary(pair[0]).map_err(bad)?
                .get(b"Parent").map_err(|_| bad("field Parent is missing from its hierarchy"))?
                .as_reference().map_err(bad)?;
            if parent != pair[1] {return Err(bad("field Parent disagrees with its hierarchy"));}
        }
        for id in self.widgets.iter().filter(|id| **id != self.owner) {
            let parent = self.doc.get_dictionary(*id).map_err(bad)?
                .get(b"Parent").map_err(|_| bad("widget Parent is missing from its hierarchy"))?
                .as_reference().map_err(bad)?;
            if parent != self.owner {return Err(bad("widget Parent disagrees with its logical owner"));}
        }
        Ok(())
    }
    /// Native field lookup stops at explicit null, unlike the metadata helper.
    /// PDFium follows the owner plus at most32ancestor levels.
    pub(crate) fn raw_attribute(&self, key: &[u8]) -> EngineResult<Option<&Object>> {
        for id in self.ancestors.iter().take(33) {
            if let Ok(value) = self.doc.get_dictionary(*id).map_err(bad)?.get(key) {
                return Ok(Some(resolve(&self.doc, value)?));
            }
        }
        Ok(None)
    }
}
pub(crate) fn load(bytes: &[u8], page: usize, index: usize) -> EngineResult<Context> {
    let (doc, _) = crate::acroforms::authoring_document(bytes)?;
    let pages = doc.get_pages();
    let p = *pages
        .values()
        .nth(page)
        .ok_or_else(|| bad("page outside source"))?;
    let a = optional(&doc, doc.get_dictionary(p).map_err(bad)?, b"Annots")?
        .ok_or_else(|| bad("page has no fields"))?
        .as_array()
        .map_err(bad)?;
    let selected = a
        .get(index)
        .ok_or_else(|| bad("field no longer exists"))?
        .as_reference()
        .map_err(bad)?;
    let form = optional(&doc, doc.catalog().map_err(bad)?, b"AcroForm")?
        .ok_or_else(|| bad("document has no form"))?
        .as_dict()
        .map_err(bad)?;
    let roots = optional(&doc, form, b"Fields")?
        .ok_or_else(|| bad("form has no fields"))?
        .as_array()
        .map_err(bad)?;
    let mut nodes = HashMap::new();
    let mut budget = 8 * 1024 * 1024;
    for root in roots {
        tree(
            &doc,
            root.as_reference().map_err(bad)?,
            None,
            None,
            "",
            0,
            &mut nodes,
            &mut budget,
        )?;
    }
    // Use the same bounded PDF text decoder for field paths, collision checks,
    // and action targets. The general authoring validator may retain UTF-8 BOMs.
    let mut names = HashSet::new();
    for node in nodes.values().filter(|n| n.field && n.named) {
        if !names.insert(node.full.clone()) {
            return Err(bad("duplicate canonical field names in one tree"));
        }
    }
    let selected = nodes
        .get(&selected)
        .filter(|n| n.widget)
        .ok_or_else(|| bad("selected annotation is not a form widget"))?;
    let owner = selected.owner;
    let node = &nodes[&owner];
    if nodes
        .iter()
        .any(|(id, n)| *id != owner && n.parent == Some(owner) && n.field)
    {
        return Err(bad("selected field has nested field definitions"));
    }
    let kind = inherited(&doc, &nodes, owner, b"FT")?
        .ok_or_else(|| bad("field type is missing"))?
        .as_name()
        .map_err(bad)?;
    let flags = inherited(&doc, &nodes, owner, b"Ff")?
        .map(|v| v.as_i64().map_err(bad))
        .transpose()?
        .unwrap_or(0);
    if !(0..=u32::MAX as i64).contains(&flags)
        || !matches!(kind, b"Tx" | b"Btn" | b"Ch")
        || (kind == b"Btn" && flags & (1 << 16) != 0)
    {
        return Err(bad("this field type cannot be edited"));
    }
    let mut linked = 0;
    for (id, n) in &nodes {
        if n.widget && n.owner == owner {
            linked += 1;
            let flags = optional(&doc, doc.get_dictionary(*id).map_err(bad)?, b"F")?
                .map(|v| v.as_i64().map_err(bad))
                .transpose()?
                .unwrap_or(0);
            if flags & (128 | 512) != 0 {
                return Err(bad("a linked widget is locked"));
            }
        }
    }
    let label = inherited(&doc, &nodes, owner, b"TU")?
        .map(|v| text(&doc, v, 32768))
        .transpose()?
        .unwrap_or_default();
    let digest = Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let properties = FormProperties {
        expected_key: format!("{digest}-{page}-{index}"),
        name: node.name.clone(),
        full_name: node.full.clone(),
        label,
        read_only: flags & 1 != 0,
        required: flags & 2 != 0,
        linked_widgets: linked,
        rename_reason: if !node.named {
            Some("This field has no partial name; renaming could change ancestor action targets. Labels and flags can still be changed.".into())
        } else {
            actions::rename_reason(&doc)?
        },
    };
    let prefix = node
        .parent
        .map(|id| nodes[&id].full.clone())
        .unwrap_or_default();
    // The tree's per-node budget includes these small ID-only projections.
    let mut ancestors = vec![owner];
    while let Some(parent) = nodes[ancestors.last().unwrap()].parent {
        ancestors.push(parent);
    }
    let widgets = nodes.iter().filter(|(_, n)| n.widget && n.owner == owner)
        .map(|(id, _)| *id).collect();
    Ok(Context {
        doc,
        owner,
        names,
        prefix,
        flags,
        properties,
        ancestors,
        widgets,
    })
}
pub(crate) fn inspect(bytes: &[u8], page: usize, index: usize) -> EngineResult<FormProperties> {
    Ok(load(bytes, page, index)?.properties)
}
pub(crate) fn removal_action_reason(doc: &Document) -> EngineResult<Option<String>> {
    Ok(actions::rename_reason(doc)?.map(|_| "This document contains scripts or unsupported actions; field removal cannot preserve their behavior yet.".into()))
}
pub(crate) fn change(bytes: &[u8], request: &SetFormPropertiesRequest) -> EngineResult<Vec<u8>> {
    request.validate()?;
    let mut ctx = load(bytes, request.page_index, request.annotation_index)?;
    if ctx.properties.expected_key != request.expected_key {
        return Err(bad("the field changed; reopen properties"));
    }
    let c = &request.changes;
    let mut expected_full = ctx.properties.full_name.clone();
    if let Some(name) = &c.name {
        if *name != ctx.properties.name {
            if let Some(reason) = &ctx.properties.rename_reason {
                return Err(bad(reason));
            }
            let full = if ctx.prefix.is_empty() {
                name.clone()
            } else {
                format!("{}.{name}", ctx.prefix)
            };
            if ctx.names.contains(&full) {
                return Err(bad("another field already uses that full name"));
            }
            actions::rename(&mut ctx.doc, &ctx.properties.full_name, &full)?;
            expected_full = full;
            ctx.doc
                .get_dictionary_mut(ctx.owner)
                .map_err(bad)?
                .set("T", lopdf::text_string(name));
        }
    }
    let owner = ctx.doc.get_dictionary_mut(ctx.owner).map_err(bad)?;
    if let Some(label) = &c.label {
        owner.set("TU", lopdf::text_string(label));
    }
    if c.read_only.is_some() || c.required.is_some() {
        let mut flags = ctx.flags;
        for (value, bit) in [(c.read_only, 1), (c.required, 2)] {
            if let Some(value) = value {
                flags = if value { flags | bit } else { flags & !bit };
            }
        }
        owner.set("Ff", flags);
    }
    let output = crate::pdf_output::save(&mut ctx.doc)?;
    let confirmed = inspect(&output, request.page_index, request.annotation_index)?;
    if c.name.as_ref().is_some_and(|v| *v != confirmed.name)
        || c.label.as_ref().is_some_and(|v| *v != confirmed.label)
        || c.read_only.is_some_and(|v| v != confirmed.read_only)
        || c.required.is_some_and(|v| v != confirmed.required)
        || confirmed.linked_widgets != ctx.properties.linked_widgets
        || confirmed.full_name != expected_full
    {
        return Err(bad("saved field properties differ from the request"));
    }
    Ok(output)
}
