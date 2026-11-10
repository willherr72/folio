//! Source-range edits of standard document XMP properties.
use crate::{
    document_properties::{bad, DocumentMetadataEdit, FIELD_LIMIT},
    EngineResult,
};
use roxmltree::{Document, Node};
use std::ops::Range;
const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
const DC: &str = "http://purl.org/dc/elements/1.1/";
const PDF: &str = "http://ns.adobe.com/pdf/1.3/";
const XML: &str = "http://www.w3.org/XML/1998/namespace";
const FIELDS: [(&str, &str); 8] = [
    (DC, "title"),
    (DC, "creator"),
    (DC, "description"),
    (PDF, "Keywords"),
    ("http://ns.adobe.com/xap/1.0/", "CreatorTool"),
    (PDF, "Producer"),
    ("http://ns.adobe.com/xap/1.0/", "CreateDate"),
    ("http://ns.adobe.com/xap/1.0/", "ModifyDate"),
];
#[derive(Default, Debug)]
pub(crate) struct Values {
    pub fields: [Option<String>; 4],
    pub details: [Option<String>; 4],
    pub multiple_authors: bool,
}
fn require(ok: bool) -> EngineResult<()> {
    if ok {
        Ok(())
    } else {
        Err(bad(
            "the XMP property has an ambiguous or unsupported structure",
        ))
    }
}
fn parsed(text: &str) -> EngineResult<Document<'_>> {
    Document::parse(text).map_err(bad)
}
fn rdf<'a, 'i>(doc: &'a Document<'i>) -> EngineResult<Option<Node<'a, 'i>>> {
    let root = doc.root_element();
    let nodes: Vec<_> = if root.has_tag_name((RDF, "RDF")) {
        vec![root]
    } else {
        root.children()
            .filter(|n| n.has_tag_name((RDF, "RDF")))
            .collect()
    };
    require(nodes.len() <= 1)?;
    Ok(nodes.first().copied())
}
fn descriptions<'a, 'i>(rdf: Node<'a, 'i>) -> EngineResult<Vec<Node<'a, 'i>>> {
    let nodes: Vec<_> = rdf.children().filter(|n| n.has_tag_name((RDF, "Description"))).collect();
    // Missing about denotes a blank subject, not the document URI. URI-fragment
    // identities need base resolution before they can safely join an about group.
    for node in &nodes {
        require(node.attribute((RDF, "ID")).is_none())?;
        require(!(node.attribute((RDF, "about")).is_some() && node.attribute((RDF, "nodeID")).is_some()))?;
    }
    let Some(selected) = nodes.iter().find(|n| n.attribute((RDF, "about")) == Some("")).or(nodes.first()).copied() else { return Ok(Vec::new()); };
    Ok(nodes.into_iter().filter(|node| {
        if let Some(about) = selected.attribute((RDF, "about")) {
            node.attribute((RDF, "about")) == Some(about)
        } else if let Some(blank) = selected.attribute((RDF, "nodeID")) {
            node.attribute((RDF, "nodeID")) == Some(blank)
        } else {
            node.id() == selected.id()
        }
    }).collect())
}

enum Property<'a, 'i> {
    Attribute(String, Range<usize>),
    Element(Node<'a, 'i>),
}
fn property<'a, 'i>(
    descriptions: &[Node<'a, 'i>],
    index: usize,
) -> EngineResult<Option<Property<'a, 'i>>> {
    let key = FIELDS[index];
    let mut found = None;
    for node in descriptions {
        for attribute in node
            .attributes()
            .filter(|a| a.namespace() == Some(key.0) && a.name() == key.1)
        {
            require(found.is_none())?;
            require(attribute.value().len() <= FIELD_LIMIT)?;
            found = Some(Property::Attribute(
                attribute.value().into(),
                attribute.range(),
            ));
        }
        for child in node.children().filter(|n| n.has_tag_name(key)) {
            require(found.is_none())?;
            found = Some(Property::Element(child));
        }
    }
    Ok(found)
}
fn value_node<'a, 'i>(node: Node<'a, 'i>) -> EngineResult<Node<'a, 'i>> {
    require(
        node.attribute((RDF, "resource")).is_none() && node.attribute((RDF, "nodeID")).is_none(),
    )?;
    let values: Vec<_> = node
        .children()
        .filter(|n| n.has_tag_name((RDF, "value")))
        .collect();
    require(values.len() <= 1)?;
    Ok(values.first().copied().unwrap_or(node))
}
fn literal(node: Node<'_, '_>) -> EngineResult<String> {
    require(
        !node.children().any(|n| n.is_element()) && node.attribute((RDF, "resource")).is_none(),
    )?;
    let mut out = String::new();
    for child in node.children().filter(|n| n.is_text()) {
        let text = child.text().unwrap_or("");
        require(out.len() + text.len() <= FIELD_LIMIT)?;
        out.push_str(text);
    }
    Ok(out)
}
fn array<'a, 'i>(
    value: Node<'a, 'i>,
    kind: &str,
) -> EngineResult<Option<(Node<'a, 'i>, Vec<Node<'a, 'i>>)>> {
    let children: Vec<_> = value.children().filter(|n| n.is_element()).collect();
    if children.is_empty() {
        return Ok(None);
    }
    require(children.len() == 1 && children[0].has_tag_name((RDF, kind)))?;
    let parent = children[0];
    let items: Vec<_> = parent.children().filter(|n| n.is_element()).collect();
    require(items.iter().all(|n| n.has_tag_name((RDF, "li"))))?;
    // Text outside array items is not a valid array value.
    require(
        parent
            .children()
            .filter(|n| n.is_text())
            .all(|n| n.text().unwrap_or("").trim().is_empty()),
    )?;
    Ok(Some((parent, items)))
}
fn read_property(property: Property<'_, '_>, index: usize) -> EngineResult<(String, bool)> {
    match property {
        Property::Attribute(value, _) => Ok((value, false)),
        Property::Element(node) => {
            let node = value_node(node)?;
            if index >= 3 {
                return Ok((literal(node)?, false));
            }
            let kind = if index == 1 { "Seq" } else { "Alt" };
            let Some((_, items)) = array(node, kind)? else {
                return Ok((literal(node)?, false));
            };
            let values: Vec<_> = items
                .iter()
                .map(|n| literal(*n))
                .collect::<EngineResult<_>>()?;
            if index == 1 {
                let length = values.iter().map(String::len).sum::<usize>()
                    + items.len().saturating_sub(1) * 2;
                require(length <= FIELD_LIMIT)?;
                return Ok((values.join("; "), items.len() > 1));
            }
            let defaults: Vec<_> = items
                .iter()
                .enumerate()
                .filter(|(_, n)| {
                    n.attribute((XML, "lang"))
                        .is_some_and(|s| s.eq_ignore_ascii_case("x-default"))
                })
                .map(|(i, _)| i)
                .collect();
            require(defaults.len() <= 1)?;
            Ok((
                defaults
                    .first()
                    .or_else(|| if values.is_empty() { None } else { Some(&0) })
                    .map_or(String::new(), |i| values[*i].clone()),
                false,
            ))
        }
    }
}
pub(crate) fn inspect(bytes: &[u8]) -> EngineResult<Values> {
    let text = crate::document_metadata_xml::normalized(bytes, &mut || Ok(()))?;
    let doc = parsed(&text)?;
    let mut out = Values::default();
    if let Some(rdf) = rdf(&doc)? {
        let nodes = descriptions(rdf)?;
        for index in 0..8 {
            if let Some(p) = property(&nodes, index)? {
                let (value, multiple) = read_property(p, index)?;
                if index < 4 {
                    out.fields[index] = Some(value);
                } else {
                    out.details[index - 4] = Some(value);
                }
                out.multiple_authors |= multiple;
            }
        }
    }
    Ok(out)
}
fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\r', "&#13;")
}
struct Edit {
    range: Range<usize>,
    replacement: String,
}
fn contents(text: &str, node: Node<'_, '_>, new: String, append: bool) -> EngineResult<Edit> {
    let range = node.range();
    let raw = &text[range.clone()];
    let mut reader = quick_xml::Reader::from_str(raw);
    let event = reader.read_event().map_err(bad)?;
    let open = reader.buffer_position() as usize;
    match event {
        quick_xml::events::Event::Empty(element) => {
            let name = element.name().as_ref().to_owned();
            Ok(Edit {
                range: range.end - 2..range.end,
                replacement: format!(">{new}</{name}>"),
            })
        }
        quick_xml::events::Event::Start(_) => {
            let close = raw
                .rfind("</")
                .ok_or_else(|| bad("missing XMP closing tag"))?;
            Ok(Edit {
                range: range.start + if append { close } else { open }..range.start + close,
                replacement: new,
            })
        }
        _ => Err(bad("invalid XMP property element")),
    }
}
fn list_item(value: &str, default: bool) -> String {
    format!(
        "<r:li xmlns:r=\"{RDF}\"{}>{}</r:li>",
        if default {
            " xml:lang=\"x-default\""
        } else {
            ""
        },
        escaped(value)
    )
}
fn canonical(index: usize, value: &str) -> String {
    let (namespace, name) = FIELDS[index];
    let body = if index == 3 {
        escaped(value)
    } else {
        let kind = if index == 1 { "Seq" } else { "Alt" };
        format!(
            "<r:{kind} xmlns:r=\"{RDF}\">{}</r:{kind}>",
            list_item(value, index != 1)
        )
    };
    format!("<p:{name} xmlns:p=\"{namespace}\">{body}</p:{name}>")
}
fn replace_property(
    text: &str,
    node: Node<'_, '_>,
    index: usize,
    value: &str,
) -> EngineResult<Edit> {
    let node = value_node(node)?;
    if index == 3 {
        literal(node)?;
        return contents(text, node, escaped(value), false);
    }
    let kind = if index == 1 { "Seq" } else { "Alt" };
    if let Some((parent, items)) = array(node, kind)? {
        for item in &items {
            literal(*item)?;
        }
        if index == 1 {
            return contents(text, parent, list_item(value, false), false);
        }
        let defaults: Vec<_> = items
            .iter()
            .filter(|n| {
                n.attribute((XML, "lang"))
                    .is_some_and(|s| s.eq_ignore_ascii_case("x-default"))
            })
            .collect();
        require(defaults.len() <= 1)?;
        if let Some(default) = defaults.first() {
            contents(text, **default, escaped(value), false)
        } else {
            contents(text, parent, list_item(value, true), true)
        }
    } else {
        literal(node)?;
        contents(
            text,
            node,
            format!(
                "<r:{kind} xmlns:r=\"{RDF}\">{}</r:{kind}>",
                list_item(value, index != 1)
            ),
            false,
        )
    }
}
pub(crate) fn update(
    bytes: Option<&[u8]>,
    edit: &DocumentMetadataEdit,
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<Vec<u8>> {
    edit.validate()?;
    check()?;
    let text = crate::document_metadata_xml::normalized(
        bytes.unwrap_or(b"<x:xmpmeta xmlns:x='adobe:ns:meta/'/>"),
        check,
    )?;
    let doc = parsed(&text)?;
    let rdf = rdf(&doc)?;
    let nodes = rdf.map(descriptions).transpose()?.unwrap_or_default();
    let mut edits = Vec::new();
    let mut appended = String::new();
    for (index, value) in edit.fields().iter().enumerate() {
        check()?;
        let Some(value) = value else { continue };
        match property(&nodes, index)? {
            Some(Property::Element(node)) => {
                edits.push(replace_property(&text, node, index, value)?)
            }
            Some(Property::Attribute(_, range)) => {
                edits.push(Edit {
                    range,
                    replacement: String::new(),
                });
                appended.push_str(&canonical(index, value));
            }
            None => appended.push_str(&canonical(index, value)),
        }
    }
    if !appended.is_empty() {
        if let Some(node) = nodes.first() {
            edits.push(contents(&text, *node, appended, true)?)
        } else {
            let description =
                format!("<r:Description xmlns:r=\"{RDF}\" r:about=\"\">{appended}</r:Description>");
            if let Some(rdf) = rdf {
                edits.push(contents(&text, rdf, description, true)?)
            } else {
                edits.push(contents(
                    &text,
                    doc.root_element(),
                    format!("<r:RDF xmlns:r=\"{RDF}\">{description}</r:RDF>"),
                    true,
                )?)
            }
        }
    }
    edits.sort_by_key(|e| (e.range.start, e.range.end));
    let mut out = String::new();
    let mut cursor = 0;
    for edit in edits {
        check()?;
        require(edit.range.start >= cursor)?;
        let prefix = &text[cursor..edit.range.start];
        require(
            out.len() + prefix.len() + edit.replacement.len()
                <= crate::document_metadata_xml::MAX_BYTES,
        )?;
        out.push_str(prefix);
        out.push_str(&edit.replacement);
        cursor = edit.range.end;
    }
    require(out.len() + text.len() - cursor <= crate::document_metadata_xml::MAX_BYTES)?;
    out.push_str(&text[cursor..]);
    crate::document_metadata_xml::normalized(out.as_bytes(), check)?;
    Ok(crate::document_metadata_xml::filter(out.as_bytes(), check)?
        .unwrap_or_else(|| out.into_bytes()))
}
#[cfg(test)]
#[path = "document_properties_xml_tests.rs"]
mod tests;
