//! Bound Form XObject expansion before new editing APIs copy/traverse pages.
//! Existing document opening/rendering is outside this editing safety check.
use crate::engine::{EngineError, EngineResult};
use lopdf::{content::Content, Dictionary, Document, Object, ObjectId, Stream};
use std::collections::HashSet;

const MAX_BYTES: usize = 64 * 1024 * 1024;
const MAX_OPERATIONS: usize = 1_000_000;
const MAX_VISITS: usize = 100_000;
const MAX_DEPTH: usize = 16;
const MAX_COPY_BYTES: usize = 16 * 1024 * 1024;
const MAX_COPY_VALUES: usize = 100_000;

/// Charge direct data before cloning; indirect references are copied tokens,
/// never recursively followed. One budget covers the whole expanded model.
#[derive(Default)]
pub(crate) struct CopyBudget {
    bytes: usize,
    values: usize,
}

impl CopyBudget {
    fn charge(&mut self, bytes: usize, values: usize) -> EngineResult<()> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| refused("direct-object copy byte overflow"))?;
        self.values = self
            .values
            .checked_add(values)
            .ok_or_else(|| refused("direct-object value overflow"))?;
        if self.bytes > MAX_COPY_BYTES || self.values > MAX_COPY_VALUES {
            return Err(refused(
                "direct-object copies exceed 16 MiB or 100000 values",
            ));
        }
        Ok(())
    }

    pub(crate) fn dictionary(&mut self, dictionary: &Dictionary) -> EngineResult<()> {
        self.dict_at(dictionary, 0)
    }

    pub(crate) fn stream(&mut self, stream: &Stream) -> EngineResult<()> {
        self.charge(stream.content.len(), 0)?;
        self.dict_at(&stream.dict, 0)
    }

    fn dict_at(&mut self, dictionary: &Dictionary, depth: usize) -> EngineResult<()> {
        if depth > 64 {
            return Err(refused("direct-object nesting exceeds 64 levels"));
        }
        self.charge(64, 1)?;
        for (key, value) in dictionary.iter() {
            self.charge(key.len().saturating_add(64), 0)?;
            self.object(value, depth + 1)?;
        }
        Ok(())
    }

    fn object(&mut self, value: &Object, depth: usize) -> EngineResult<()> {
        if depth > 64 {
            return Err(refused("direct-object nesting exceeds 64 levels"));
        }
        self.charge(64, 1)?;
        match value {
            Object::Name(bytes) | Object::String(bytes, _) => self.charge(bytes.len(), 0)?,
            Object::Array(values) => {
                for value in values {
                    self.object(value, depth + 1)?;
                }
            }
            Object::Dictionary(dict) => self.dict_at(dict, depth + 1)?,
            Object::Stream(stream) => {
                self.charge(stream.content.len(), 0)?;
                self.dict_at(&stream.dict, depth + 1)?;
            }
            _ => {}
        }
        Ok(())
    }
}

fn refused(reason: &str) -> EngineError {
    EngineError::InvalidRequest(format!("PDF form safety check: {reason}"))
}

fn named(doc: &Document, dict: &Dictionary, key: &[u8], expected: &[u8]) -> EngineResult<bool> {
    let value = match dict.get(key) {
        Ok(value) => value,
        Err(_) => return Ok(false),
    };
    let (_, value) = doc
        .dereference(value)
        .map_err(|_| refused("invalid type reference"))?;
    Ok(value
        .as_name()
        .map_err(|_| refused("invalid object type"))?
        == expected)
}

struct Guard<'a> {
    doc: &'a Document,
    bytes: usize,
    operations: usize,
    visits: usize,
    active: HashSet<ObjectId>,
    copies: CopyBudget,
}

impl<'a> Guard<'a> {
    fn decode(stream: &Stream, limit: usize) -> EngineResult<Vec<u8>> {
        if stream.dict.has(b"Filter") {
            stream
                .filters()
                .map_err(|_| refused("invalid stream filter"))?;
        }
        stream
            .get_plain_content_with_limit(limit)
            .map_err(|_| refused("stream exceeds decoding limit or has an unsupported filter"))
    }

    fn page_content(
        &mut self,
        value: &Object,
        output: &mut Vec<u8>,
        depth: usize,
    ) -> EngineResult<()> {
        if depth > MAX_DEPTH {
            return Err(refused("page content nesting exceeds limit"));
        }
        self.visits += 1;
        if self.visits > MAX_VISITS {
            return Err(refused("content object visits exceed limit"));
        }
        let (_, object) = self
            .doc
            .dereference(value)
            .map_err(|_| refused("invalid page content reference"))?;
        match object {
            Object::Stream(stream) => {
                let remaining = MAX_BYTES
                    .saturating_sub(self.bytes)
                    .saturating_sub(output.len());
                let data = Self::decode(stream, remaining.saturating_sub(1))?;
                if remaining == 0 {
                    return Err(refused("page content exceeds decoding limit"));
                }
                output.extend_from_slice(&data);
                output.push(b'\n');
            }
            Object::Array(values) => {
                self.copies.object(object, 0)?;
                let values = values.clone();
                for value in values {
                    self.page_content(&value, output, depth + 1)?;
                }
            }
            Object::Null => {}
            _ => return Err(refused("invalid page content object")),
        }
        Ok(())
    }

    fn dictionary<'b>(doc: &'b Document, object: &'b Object) -> EngineResult<&'b Dictionary> {
        doc.dereference(object)
            .and_then(|(_, o)| o.as_dict())
            .map_err(|_| refused("invalid resource dictionary"))
    }

    fn page_resources(&mut self, mut id: ObjectId) -> EngineResult<Dictionary> {
        let mut seen = HashSet::new();
        for _ in 0..64 {
            if !seen.insert(id) {
                return Err(refused("cyclic page inheritance"));
            }
            let dict = self
                .doc
                .get_dictionary(id)
                .map_err(|_| refused("invalid page dictionary"))?;
            if let Ok(value) = dict.get(b"Resources") {
                let resources = Self::dictionary(self.doc, value)?;
                self.copies.dictionary(resources)?;
                return Ok(resources.clone());
            }
            match dict.get(b"Parent") {
                Ok(parent) => {
                    id = parent
                        .as_reference()
                        .map_err(|_| refused("invalid page parent"))?
                }
                Err(_) => return Ok(Dictionary::new()),
            }
        }
        Err(refused("page inheritance exceeds depth limit"))
    }

    fn content(&mut self, data: Vec<u8>, resources: &Dictionary, depth: usize) -> EngineResult<()> {
        self.bytes = self
            .bytes
            .checked_add(data.len())
            .ok_or_else(|| refused("decoded byte overflow"))?;
        if self.bytes > MAX_BYTES {
            return Err(refused("expanded decoded content exceeds 64 MiB"));
        }
        // A lexical over-approximation is intentional in this bounded editor:
        // BI inside a literal/comment may also cause a conservative refusal.
        if data.windows(2).enumerate().any(|(i, token)| {
            token == b"BI"
                && (i == 0
                    || data[i - 1] == 0
                    || data[i - 1].is_ascii_whitespace()
                    || b"()<>[]{}/%".contains(&data[i - 1]))
                && (i + 2 == data.len()
                    || data[i + 2] == 0
                    || data[i + 2].is_ascii_whitespace()
                    || b"()<>[]{}/%".contains(&data[i + 2]))
        }) {
            return Err(refused(
                "inline image syntax is unsupported by form editing",
            ));
        }
        let content = std::panic::catch_unwind(|| Content::decode_strict(&data))
            .map_err(|_| refused("content parser rejected unsafe input"))?
            .map_err(|_| refused("content cannot be safely inspected"))?;
        if content.operations.iter().any(|op| op.operator == "BI") {
            return Err(refused("inline images are unsupported by form editing"));
        }
        self.operations = self
            .operations
            .checked_add(content.operations.len())
            .ok_or_else(|| refused("operation count overflow"))?;
        if self.operations > MAX_OPERATIONS {
            return Err(refused("expanded content exceeds operation limit"));
        }
        for op in content.operations {
            if op.operator != "Do" {
                continue;
            }
            if op.operands.len() != 1 {
                return Err(refused("invalid form invocation"));
            }
            let name = op.operands[0]
                .as_name()
                .map_err(|_| refused("invalid form resource name"))?;
            let table = match resources.get(b"XObject") {
                Ok(value) => Self::dictionary(self.doc, value)?,
                Err(_) => continue, // Missing resources are harmless to expansion.
            };
            let value = match table.get(name) {
                Ok(value) => {
                    self.copies.object(value, 0)?;
                    value.clone()
                }
                Err(_) => continue,
            };
            self.form(&value, resources, depth + 1)?;
        }
        Ok(())
    }

    fn form(&mut self, value: &Object, inherited: &Dictionary, depth: usize) -> EngineResult<()> {
        self.visits += 1;
        if self.visits > MAX_VISITS {
            return Err(refused("form invocation count exceeds limit"));
        }
        let (id, object) = self
            .doc
            .dereference(value)
            .map_err(|_| refused("cyclic or invalid object reference"))?;
        let stream = match object.as_stream() {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };
        if !named(self.doc, &stream.dict, b"Subtype", b"Form")? {
            return Ok(());
        }
        if depth > MAX_DEPTH {
            return Err(refused("form nesting exceeds 16 levels"));
        }
        if let Some(id) = id {
            if !self.active.insert(id) {
                return Err(refused("cyclic form invocation"));
            }
        }
        let resources = match stream.dict.get(b"Resources") {
            Ok(value) => self
                .doc
                .dereference(value)
                .and_then(|(_, o)| o.as_dict())
                .map_err(|_| refused("invalid form resources"))?,
            Err(_) => inherited,
        };
        self.copies.dictionary(resources)?;
        let resources = resources.clone();
        // Do not fall back to compressed bytes when decoding fails.
        let data = Self::decode(stream, MAX_BYTES.saturating_sub(self.bytes))?;
        self.content(data, &resources, depth)?;
        if let Some(id) = id {
            self.active.remove(&id);
        }
        Ok(())
    }
}

pub(crate) fn preflight(bytes: &[u8]) -> EngineResult<()> {
    let doc = Document::load_mem_with_options(
        bytes,
        lopdf::LoadOptions::with_max_decompressed_size(MAX_BYTES),
    )
    .map_err(|_| refused("document cannot be safely inspected"))?;
    let mut guard = Guard {
        doc: &doc,
        bytes: 0,
        operations: 0,
        visits: 0,
        active: HashSet::new(),
        copies: CopyBudget::default(),
    };
    // Resolve /Type explicitly: lopdf's page iterator skips indirect names.
    // Every page dictionary is a root, including pages omitted by a damaged
    // tree that PDFium might repair differently.
    for (id, object) in &doc.objects {
        let dict = match object.as_dict() {
            Ok(dict) => dict,
            Err(_) => continue,
        };
        if !named(&doc, dict, b"Type", b"Page")? {
            continue;
        }
        let id = *id;
        let resources = guard.page_resources(id)?;
        let mut data = Vec::new();
        if let Ok(contents) = doc
            .get_dictionary(id)
            .map_err(|_| refused("invalid page"))?
            .get(b"Contents")
        {
            guard.page_content(contents, &mut data, 0)?;
        }
        guard.content(data, &resources, 0)?;
    }
    // Unreferenced forms are not traversed by the editing operation.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Stream};

    fn fixture(depth: usize, cyclic: bool, repeated: usize) -> Vec<u8> {
        let mut doc = Document::with_version("1.7");
        let pages = doc.new_object_id();
        let ids: Vec<_> = (0..depth).map(|_| doc.new_object_id()).collect();
        for (index, id) in ids.iter().enumerate() {
            let next = ids
                .get(index + 1)
                .copied()
                .or(if cyclic { Some(ids[0]) } else { None });
            let resources = next
                .map(|n| dictionary! { "XObject" => dictionary! { "F" => n } })
                .unwrap_or_default();
            let content = if next.is_some() {
                b"/F Do".to_vec()
            } else {
                b"q Q".to_vec()
            };
            doc.objects.insert(*id, Object::Stream(Stream::new(dictionary! {
                "Type" => "XObject", "Subtype" => "Form", "BBox" => vec![0.into(),0.into(),100.into(),100.into()],
                "Resources" => resources
            }, content)));
        }
        let content = doc.add_object(Stream::new(Dictionary::new(), b"/F Do\n".repeat(repeated)));
        let page = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages,
        "MediaBox" => vec![0.into(),0.into(),200.into(),200.into()], "Contents" => content,
        "Resources" => dictionary! { "XObject" => dictionary! { "F" => ids[0] } } });
        doc.objects.insert(
            pages,
            dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }.into(),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
        doc.trailer.set("Root", catalog);
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn admits_repeated_shared_form_and_nested_forms() {
        preflight(&fixture(2, false, 3)).unwrap();
    }
    #[test]
    fn refuses_cycle_before_edit_traversal() {
        assert!(preflight(&fixture(2, true, 1))
            .unwrap_err()
            .to_string()
            .contains("cyclic form"));
    }
    #[test]
    fn refuses_depth_before_edit_traversal() {
        assert!(preflight(&fixture(MAX_DEPTH + 1, false, 1))
            .unwrap_err()
            .to_string()
            .contains("nesting"));
    }
    #[test]
    fn refuses_expansion_visits() {
        let doc = Document::new();
        let mut guard = Guard {
            doc: &doc,
            bytes: 0,
            operations: 0,
            visits: MAX_VISITS,
            active: HashSet::new(),
            copies: CopyBudget::default(),
        };
        assert!(guard
            .form(&Object::Null, &Dictionary::new(), 1)
            .unwrap_err()
            .to_string()
            .contains("invocation count"));
    }
    #[test]
    fn aggregate_budgets_are_not_per_stream() {
        let doc = Document::new();
        let mut guard = Guard {
            doc: &doc,
            bytes: MAX_BYTES - 2,
            operations: 0,
            visits: 0,
            active: HashSet::new(),
            copies: CopyBudget::default(),
        };
        assert!(guard
            .content(b"q Q".to_vec(), &Dictionary::new(), 0)
            .unwrap_err()
            .to_string()
            .contains("decoded content"));
        guard.bytes = 0;
        guard.operations = MAX_OPERATIONS - 1;
        assert!(guard
            .content(b"q Q".to_vec(), &Dictionary::new(), 0)
            .unwrap_err()
            .to_string()
            .contains("operation limit"));
    }
    #[test]
    fn indirect_subtype_cannot_hide_a_cycle() {
        let mut doc = Document::load_mem(&fixture(2, true, 1)).unwrap();
        let name = doc.add_object(Object::Name(b"Form".to_vec()));
        for object in doc.objects.values_mut() {
            if let Ok(stream) = object.as_stream_mut() {
                if stream.dict.has(b"Subtype") {
                    stream.dict.set("Subtype", name);
                }
            }
        }
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        assert!(preflight(&bytes)
            .unwrap_err()
            .to_string()
            .contains("cyclic form"));
    }
    #[test]
    fn indirect_page_type_cannot_hide_content() {
        let mut doc = Document::load_mem(&fixture(1, false, 1)).unwrap();
        let page = *doc.get_pages().values().next().unwrap();
        let name = doc.add_object(Object::Name(b"Page".to_vec()));
        let malformed = doc.add_object(Stream::new(Dictionary::new(), b"q (unterminated".to_vec()));
        let dict = doc.get_dictionary_mut(page).unwrap();
        dict.set("Type", name);
        dict.set("Contents", malformed);
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        assert!(preflight(&bytes)
            .unwrap_err()
            .to_string()
            .contains("safely inspected"));
    }
    #[test]
    fn refuses_inline_image_before_parser_arithmetic() {
        let doc = Document::new();
        let mut guard = Guard {
            doc: &doc,
            bytes: 0,
            operations: 0,
            visits: 0,
            active: HashSet::new(),
            copies: CopyBudget::default(),
        };
        let data = b"BI /W 9223372036854775807 /H 2 /BPC 8 /CS /RGB ID x EI";
        assert!(guard
            .content(data.to_vec(), &Dictionary::new(), 0)
            .unwrap_err()
            .to_string()
            .contains("inline image"));
    }
    #[test]
    fn refuses_repeated_large_direct_resources() {
        let mut doc = Document::load_mem(&fixture(1, false, 80)).unwrap();
        for object in doc.objects.values_mut() {
            if let Ok(stream) = object.as_stream_mut() {
                if stream.dict.has(b"Subtype") {
                    stream.dict.set("Resources", dictionary! { "Font" => dictionary! {
                        "LargeDirectValue" => Object::String(vec![b'x'; 256 * 1024], lopdf::StringFormat::Hexadecimal)
                    }});
                }
            }
        }
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        assert!(preflight(&bytes)
            .unwrap_err()
            .to_string()
            .contains("direct-object"));
    }
    #[test]
    fn copy_budget_counts_nested_values_and_raw_streams() {
        let nested = dictionary! { "Array" => Object::Array(vec![
            Object::Dictionary(dictionary! { "Payload" => Object::String(vec![b'x'; 256 * 1024], lopdf::StringFormat::Literal) })
        ]) };
        let mut budget = CopyBudget::default();
        for _ in 0..60 {
            budget.dictionary(&nested).unwrap();
        }
        assert!((0..20).any(|_| budget.dictionary(&nested).is_err()));
        let stream = Stream::new(Dictionary::new(), vec![b'x'; 2 * 1024 * 1024]);
        let mut budget = CopyBudget::default();
        for _ in 0..7 {
            budget.stream(&stream).unwrap();
        }
        assert!(budget.stream(&stream).is_err());
        let mut budget = CopyBudget::default();
        budget
            .dictionary(&dictionary! { "Reference" => Object::Reference((999, 0)) })
            .unwrap();
        assert!(budget.bytes < 1024); // No dereference or indirect payload charge.
    }
}
