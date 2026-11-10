//! Small, bounded appearance graphs. Never import page or field ownership trees.
use crate::{form_graph::CopyBudget, form_position::bad, EngineResult};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use std::collections::{HashMap, HashSet};

pub(crate) struct Copier<'a> {
    source: &'a Document,
    output: &'a mut Document,
    ids: HashMap<ObjectId, ObjectId>,
    budget: CopyBudget,
    failed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;
    #[test]
    fn review_untyped_field_widget_and_action_graphs_are_excluded() {
        for node in [
            dictionary! {"FT"=>"Tx","T"=>Object::string_literal("Other")},
            dictionary! {"Kids"=>vec![Object::Null]},
            dictionary! {"Subtype"=>"Widget"},
            dictionary! {"S"=>"JavaScript","JS"=>Object::string_literal("marker")},
        ] {
            let mut source = Document::new();
            let owner = source.add_object(node);
            let font = source.add_object(dictionary! {"Type"=>"Font","FolioOwner"=>owner});
            let mut output = Document::new();
            assert!(
                Copier::new(&source, &mut output)
                    .copy(&font.into())
                    .is_err(),
                "untyped ownership imported"
            );
        }
    }
    #[test]
    fn graph_budget_precedes_large_copies_and_keeps_aliases() {
        let mut source = Document::new();
        let large = source.add_object(Object::String(
            vec![0; 16 * 1024 * 1024],
            lopdf::StringFormat::Literal,
        ));
        let mut output = Document::new();
        assert!(Copier::new(&source, &mut output)
            .copy(&large.into())
            .is_err());
        assert!(output.objects.is_empty());
        let leaf = source.add_object(dictionary! {"Value"=>42});
        let result = Copier::new(&source, &mut output)
            .copy(&Object::Array(vec![leaf.into(), leaf.into()]))
            .unwrap();
        let a = result.as_array().unwrap();
        assert_eq!(a[0], a[1]);
    }
    #[test]
    fn graph_import_rejects_page_ownership_but_bounds_cycles() {
        let mut source = Document::new();
        let id = source.new_object_id();
        source.objects.insert(id, dictionary! {"Self"=>id}.into());
        let mut output = Document::new();
        let copied = Copier::new(&source, &mut output).copy(&id.into()).unwrap();
        assert_eq!(
            output
                .dereference(&copied)
                .unwrap()
                .1
                .as_dict()
                .unwrap()
                .get(b"Self")
                .unwrap(),
            &copied
        );
        let page = source.add_object(dictionary! {"Type"=>"Page"});
        assert!(Copier::new(&source, &mut output)
            .copy(&page.into())
            .unwrap_err()
            .to_string()
            .contains("ownership"));
    }
    #[test]
    fn font_comparison_detects_changes_even_after_a_failed_comparison() {
        let mut a = Document::new();
        let mut b = Document::new();
        let x = a.add_object(dictionary! {"BaseFont"=>"Courier"});
        let y = b.add_object(dictionary! {"BaseFont"=>"Helvetica"});
        let mut check = Equivalence::default();
        for _ in 0..2 {
            assert!(!check.check(&a, &x.into(), &b, &y.into()).unwrap());
        }
        b.get_dictionary_mut(y).unwrap().set("BaseFont", "Courier");
        assert!(check.check(&a, &x.into(), &b, &y.into()).unwrap());
    }
    #[test]
    fn font_program_comparison_accepts_compression_and_refuses_expansion() {
        let a = Document::new();
        let b = Document::new();
        let original = Stream::new(Dictionary::new(), vec![b'x'; 2000]);
        let mut compressed = original.clone();
        compressed.compress().unwrap();
        assert!(Equivalence::default()
            .check(
                &a,
                &Object::Stream(original),
                &b,
                &Object::Stream(compressed)
            )
            .unwrap());
        let mut huge = Stream::new(Dictionary::new(), vec![b'x'; 17 * 1024 * 1024]);
        huge.compress().unwrap();
        assert!(Equivalence::default()
            .check(&a, &Object::Stream(huge.clone()), &b, &Object::Stream(huge))
            .is_err());
    }
}
impl<'a> Copier<'a> {
    pub(crate) fn bytes_used(&self)->usize {self.budget.bytes_used()}
    pub(crate) fn with_byte_limit(
        source: &'a Document,
        output: &'a mut Document,
        limit: usize,
    ) -> Self {
        Self {
            source,
            output,
            ids: HashMap::new(),
            budget: CopyBudget::with_byte_limit(limit.min(64 * 1024 * 1024)),
            failed: false,
        }
    }
    pub(crate) fn retain_local(&mut self, value: Object) -> EngineResult<ObjectId> {
        self.ready()?;
        let result = (|| {
            self.budget.array(std::slice::from_ref(&value))?;
            let id = self.next_id()?;
            self.output.objects.insert(id, value);
            Ok(id)
        })();
        self.failed = result.is_err();
        result
    }
    pub(crate) fn reserve_dictionary(&mut self, value: &Dictionary) -> EngineResult<()> {
        self.ready()?;
        let result = self.budget.dictionary(value);
        self.failed = result.is_err();
        result
    }
    pub(crate) fn reserve_object(&mut self, value: &Object) -> EngineResult<()> {
        self.ready()?;
        let result = self.budget.array(std::slice::from_ref(value));
        self.failed = result.is_err();
        result
    }
    pub(crate) fn reserve_bytes(&mut self, bytes: usize) -> EngineResult<()> {
        self.ready()?;
        let result = self.budget.reserve_bytes(bytes);
        self.failed = result.is_err();
        result
    }
    pub(crate) fn copy_dictionary(&mut self, value: &Dictionary) -> EngineResult<Dictionary> {
        self.reserve_dictionary(value)?;
        let result = self.dictionary(value, 0);
        self.failed = result.is_err();
        result
    }
    fn ready(&self) -> EngineResult<()> {
        if self.failed {
            Err(bad(
                "appearance staging was invalidated by an earlier failure",
            ))
        } else {
            Ok(())
        }
    }
    fn next_id(&mut self) -> EngineResult<ObjectId> {
        if self.output.max_id == u32::MAX {
            return Err(bad("appearance object identifiers are exhausted"));
        }
        Ok(self.output.new_object_id())
    }
    pub(crate) fn new(source: &'a Document, output: &'a mut Document) -> Self {
        Self {
            source,
            output,
            ids: HashMap::new(),
            budget: CopyBudget::default(),
            failed: false,
        }
    }
    pub(crate) fn copy(&mut self, value: &Object) -> EngineResult<Object> {
        self.ready()?;
        let result = self.root(value, 0);
        self.failed = result.is_err();
        result
    }
    fn root(&mut self, value: &Object, depth: usize) -> EngineResult<Object> {
        // Charge every direct allocation before cloning, and each indirect graph once.
        self.budget.array(std::slice::from_ref(value))?;
        self.value(value, depth)
    }
    fn value(&mut self, value: &Object, depth: usize) -> EngineResult<Object> {
        if depth > 64 {
            return Err(bad("appearance graph exceeds 64 levels"));
        }
        Ok(match value {
            Object::Reference(id) => {
                if let Some(id) = self.ids.get(id) {
                    return Ok(Object::Reference(*id));
                }
                let next = self.next_id()?;
                self.ids.insert(*id, next);
                let copied = self.root(self.source.get_object(*id).map_err(bad)?, depth + 1)?;
                self.output.objects.insert(next, copied);
                Object::Reference(next)
            }
            Object::Dictionary(d) => Object::Dictionary(self.dictionary(d, depth)?),
            Object::Stream(s) => Object::Stream(Stream::new(
                self.dictionary(&s.dict, depth)?,
                s.content.clone(),
            )),
            Object::Array(a) => Object::Array(
                a.iter()
                    .map(|v| self.value(v, depth + 1))
                    .collect::<EngineResult<_>>()?,
            ),
            Object::Real(n) if !n.is_finite() => return Err(bad("nonfinite appearance value")),
            _ => value.clone(),
        })
    }
    fn dictionary(&mut self, d: &Dictionary, depth: usize) -> EngineResult<Dictionary> {
        // Field and action dictionaries need not carry /Type. Follow structure,
        // including nonterminal field parents, before traversing opaque edges.
        let name = |key: &[u8]| -> EngineResult<Option<&[u8]>> {
            d.get(key)
                .ok()
                .map(|v| self.source.dereference(v).map_err(bad))
                .transpose()
                .map(|v| v.and_then(|(_, v)| v.as_name().ok()))
        };
        let action = name(b"S")?
            .is_some_and(|s| !matches!(s, b"S" | b"D" | b"B" | b"I" | b"U" | b"Transparency"));
        if d.has(b"FT")
            || d.has(b"Kids")
            || d.has(b"JS")
            || name(b"Subtype")? == Some(b"Widget")
            || action
        {
            return Err(bad("appearance graph refers to field or action ownership"));
        }
        if let Ok(kind) = d.get(b"Type") {
            let kind = self.source.dereference(kind).map_err(bad)?.1.as_name().ok();
            if matches!(
                kind,
                Some(
                    b"Page"
                        | b"Pages"
                        | b"Catalog"
                        | b"Annot"
                        | b"Action"
                        | b"Outlines"
                        | b"StructTreeRoot"
                )
            ) {
                return Err(bad("appearance graph refers to document ownership"));
            }
        }
        let mut output = Dictionary::new();
        for (key, value) in d.iter() {
            output.set(key.clone(), self.value(value, depth + 1)?);
        }
        Ok(output)
    }
}

/// Compare font meaning across PDFium serialization, including decompressed programs.
/// Reference numbering, string syntax and stream compression are immaterial.
#[derive(Default)]
pub(crate) struct Equivalence {
    seen: HashSet<(usize, usize)>,
    bytes: usize,
    visits: usize,
}
impl Equivalence {
    pub(crate) fn check(
        &mut self,
        a: &Document,
        av: &Object,
        b: &Document,
        bv: &Object,
    ) -> EngineResult<bool> {
        self.seen.clear();
        self.value(a, av, b, bv, 0)
    }
    fn charge(&mut self, bytes: usize, depth: usize) -> EngineResult<()> {
        self.visits += 1;
        self.bytes = self.bytes.saturating_add(bytes);
        if depth > 64 || self.visits > 100000 || self.bytes > 16 * 1024 * 1024 {
            return Err(bad("font comparison exceeds preservation limits"));
        }
        Ok(())
    }
    fn value(
        &mut self,
        a: &Document,
        av: &Object,
        b: &Document,
        bv: &Object,
        depth: usize,
    ) -> EngineResult<bool> {
        self.charge(0, depth)?;
        let av = a.dereference(av).map_err(bad)?.1;
        let bv = b.dereference(bv).map_err(bad)?.1;
        if !self
            .seen
            .insert((av as *const Object as usize, bv as *const Object as usize))
        {
            return Ok(true);
        }
        Ok(match (av, bv) {
            (Object::Null, Object::Null) => true,
            (Object::Boolean(x), Object::Boolean(y)) => x == y,
            (Object::Integer(_) | Object::Real(_), Object::Integer(_) | Object::Real(_)) => {
                av.as_float().map_err(bad)? == bv.as_float().map_err(bad)?
            }
            (Object::Name(x), Object::Name(y)) | (Object::String(x, _), Object::String(y, _)) => {
                self.charge(x.len().saturating_add(y.len()), depth)?;
                x == y
            }
            (Object::Array(x), Object::Array(y)) => {
                if x.len() != y.len() {
                    return Ok(false);
                }
                for (x, y) in x.iter().zip(y) {
                    if !self.value(a, x, b, y, depth + 1)? {
                        return Ok(false);
                    }
                }
                true
            }
            (Object::Dictionary(x), Object::Dictionary(y)) => {
                self.dict(a, x, b, y, depth, false)?
            }
            (Object::Stream(x), Object::Stream(y)) => {
                if !self.dict(a, &x.dict, b, &y.dict, depth, true)? {
                    return Ok(false);
                }
                let x = x
                    .get_plain_content_with_limit(
                        (16 * 1024 * 1024usize).saturating_sub(self.bytes),
                    )
                    .map_err(bad)?;
                self.charge(x.len(), depth)?;
                let y = y
                    .get_plain_content_with_limit(
                        (16 * 1024 * 1024usize).saturating_sub(self.bytes),
                    )
                    .map_err(bad)?;
                self.charge(y.len(), depth)?;
                x == y
            }
            _ => false,
        })
    }
    fn dict(
        &mut self,
        a: &Document,
        x: &Dictionary,
        b: &Document,
        y: &Dictionary,
        depth: usize,
        stream: bool,
    ) -> EngineResult<bool> {
        let ignored = |k: &[u8]| stream && matches!(k, b"Length" | b"Filter" | b"DecodeParms");
        if x.iter().filter(|(k, _)| !ignored(k)).count()
            != y.iter().filter(|(k, _)| !ignored(k)).count()
        {
            return Ok(false);
        }
        for (k, v) in x.iter().filter(|(k, _)| !ignored(k)) {
            self.charge(k.len(), depth)?;
            let Ok(other) = y.get(k) else {
                return Ok(false);
            };
            if !self.value(a, v, b, other, depth + 1)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
