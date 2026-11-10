//! Preserve resources whose names collide with PDFium's page-tree key handling.
//!
//! CPDF_PageOrganizer::UpdateReference skips First/Prev/Parent in every dictionary,
//! including resource dictionaries. Importing a legal /First XObject therefore
//! leaves a source-document reference in the destination. Before loading imported
//! pages or adding overlays, restore affected resource graphs with fresh object IDs.
//! Content operators, resource names, stream bytes, and intra-page aliases stay intact.
use crate::{acroforms::SourceForm, EngineError, EngineResult};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use std::collections::{HashMap, HashSet};

const MAX_DEPTH: usize = 64;
const MAX_VALUES: usize = 1_000_000;
const MAX_BYTES: usize = 512 * 1024 * 1024;
fn bad(message: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("PDF page resources: {message}"))
}
#[derive(Default)]
struct Budget {
    values: usize,
    bytes: usize,
}
impl Budget {
    fn take(&mut self, bytes: usize, depth: usize) -> EngineResult<()> {
        self.values += 1;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| bad("byte limit"))?;
        if depth > MAX_DEPTH || self.values > MAX_VALUES || self.bytes > MAX_BYTES {
            return Err(bad("resource graph exceeds preservation limits"));
        }
        Ok(())
    }
}
fn resources(source: &Document, page: ObjectId) -> EngineResult<Option<&Object>> {
    let mut current = page;
    let mut seen = HashSet::new();
    loop {
        if seen.len() >= MAX_DEPTH || !seen.insert(current) {
            return Err(bad("cyclic page inheritance"));
        }
        let dict = source.get_dictionary(current).map_err(bad)?;
        if let Ok(value) = dict.get(b"Resources") {
            return Ok(Some(value));
        }
        match dict.get(b"Parent") {
            Ok(value) => current = value.as_reference().map_err(bad)?,
            Err(_) => return Ok(None),
        }
    }
}
fn affected(
    source: &Document,
    value: &Object,
    seen: &mut HashSet<ObjectId>,
    budget: &mut Budget,
    depth: usize,
) -> EngineResult<bool> {
    budget.take(0, depth)?;
    match value {
        Object::Reference(id) if seen.insert(*id) => affected(
            source,
            source.get_object(*id).map_err(bad)?,
            seen,
            budget,
            depth + 1,
        ),
        Object::Array(values) => {
            for value in values {
                if affected(source, value, seen, budget, depth + 1)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Object::Dictionary(dict) => affected_dictionary(source, dict, seen, budget, depth),
        Object::Stream(stream) => affected_dictionary(source, &stream.dict, seen, budget, depth),
        _ => Ok(false),
    }
}
fn affected_dictionary(
    source: &Document,
    dict: &Dictionary,
    seen: &mut HashSet<ObjectId>,
    budget: &mut Budget,
    depth: usize,
) -> EngineResult<bool> {
    for (key, value) in dict.iter() {
        // Even a direct value may contain references below it. Restore the whole
        // graph for any such key rather than infer targets from imported IDs.
        if matches!(key.as_slice(), b"First" | b"Prev" | b"Parent") {
            return Ok(true);
        }
        if affected(source, value, seen, budget, depth + 1)? {
            return Ok(true);
        }
    }
    Ok(false)
}
struct Copier<'a> {
    source: &'a Document,
    output: &'a mut Document,
    map: HashMap<ObjectId, ObjectId>,
    budget: &'a mut Budget,
}
impl Copier<'_> {
    fn value(&mut self, value: &Object, depth: usize) -> EngineResult<Object> {
        self.budget.take(0, depth)?;
        Ok(match value {
            Object::Reference(id) => {
                if let Some(target) = self.map.get(id) {
                    return Ok(Object::Reference(*target));
                }
                let original = self.source.get_object(*id).map_err(bad)?;
                let target = self.output.new_object_id();
                self.map.insert(*id, target);
                let copied = self.value(original, depth + 1)?;
                self.output.objects.insert(target, copied);
                Object::Reference(target)
            }
            Object::Dictionary(dict) => Object::Dictionary(self.dictionary(dict, depth)?),
            Object::Array(values) => Object::Array(
                values
                    .iter()
                    .map(|v| self.value(v, depth + 1))
                    .collect::<EngineResult<_>>()?,
            ),
            Object::Stream(stream) => {
                self.budget.take(stream.content.len(), depth)?;
                Object::Stream(Stream::new(
                    self.dictionary(&stream.dict, depth)?,
                    stream.content.clone(),
                ))
            }
            Object::Name(bytes) | Object::String(bytes, _) => {
                self.budget.take(bytes.len(), depth)?;
                value.clone()
            }
            Object::Real(number) if !number.is_finite() => {
                return Err(bad("nonfinite resource value"))
            }
            _ => value.clone(),
        })
    }
    fn dictionary(&mut self, dict: &Dictionary, depth: usize) -> EngineResult<Dictionary> {
        let kind = dict
            .get(b"Type")
            .ok()
            .map(|value| self.source.dereference(value).map_err(bad))
            .transpose()?
            .and_then(|(_, value)| value.as_name().ok());
        if matches!(kind, Some(b"Page" | b"Pages" | b"Catalog")) {
            return Err(bad("resource graph refers to a document page tree"));
        }
        let mut copied = Dictionary::new();
        for (key, value) in dict.iter() {
            self.budget.take(key.len(), depth)?;
            copied.set(key.clone(), self.value(value, depth + 1)?);
        }
        Ok(copied)
    }
}

/// Must run before imported pages are loaded or modified. Separate maps per page
/// preserve aliases within a page without coupling resource mutations on duplicates.
pub(crate) fn needed(sources: &[SourceForm<'_>]) -> EngineResult<bool> {
    let mut budget = Budget::default();
    for request in sources {
        let source = request.document()?;
        let pages: Vec<_> = source.get_pages().values().copied().collect();
        let mut checked = HashSet::new();
        for &(old_index, _) in &request.pages {
            if !checked.insert(old_index) {
                continue;
            }
            let page = *pages
                .get(old_index)
                .ok_or_else(|| bad("unknown source page"))?;
            if let Some(value) = resources(&source, page)? {
                if affected(&source, value, &mut HashSet::new(), &mut budget, 0)? {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

pub(crate) fn preserve(output: &mut Document, sources: &[SourceForm<'_>]) -> EngineResult<bool> {
    let output_pages: Vec<_> = output.get_pages().values().copied().collect();
    let mut changed = false;
    let mut scan_budget = Budget::default();
    let mut copy_budget = Budget::default();
    for request in sources {
        let source = request.document()?;
        let source_pages: Vec<_> = source.get_pages().values().copied().collect();
        let mut needs_copy = HashMap::new();
        for &(old_index, new_index) in &request.pages {
            let old = *source_pages
                .get(old_index)
                .ok_or_else(|| bad("unknown source page"))?;
            let new = *output_pages
                .get(new_index)
                .ok_or_else(|| bad("unknown destination page"))?;
            let Some(value) = resources(&source, old)? else {
                continue;
            };
            let needs = match needs_copy.get(&old) {
                Some(needs) => *needs,
                None => {
                    let needs = affected(&source, value, &mut HashSet::new(), &mut scan_budget, 0)?;
                    needs_copy.insert(old, needs);
                    needs
                }
            };
            if needs {
                let mut copier = Copier {
                    source: &source,
                    output,
                    map: HashMap::new(),
                    budget: &mut copy_budget,
                };
                let copied = copier.value(value, 0)?;
                copier
                    .output
                    .get_object_mut(new)
                    .map_err(bad)?
                    .as_dict_mut()
                    .map_err(bad)?
                    .set("Resources", copied);
                changed = true;
            }
        }
    }
    Ok(changed)
}

pub(crate) fn preserve_bytes(
    bytes: &[u8],
    sources: &[SourceForm<'_>],
) -> EngineResult<Option<Vec<u8>>> {
    let mut output = crate::pdf_output::load(bytes)?;
    if preserve(&mut output, sources)? {
        prune_unreachable(&mut output)?;
        Ok(Some(crate::pdf_output::save(&mut output)?))
    } else {
        Ok(None)
    }
}

pub(crate) fn prune_unreachable(output: &mut Document) -> EngineResult<()> {
    let mut seen = HashSet::new();
    let mut pending: Vec<_> = output.trailer.iter().map(|(_, value)| value).collect();
    let mut visits = 0;
    while let Some(value) = pending.pop() {
        visits += 1;
        if visits > MAX_VALUES {
            return Err(bad("resource reachability exceeds preservation limits"));
        }
        match value {
            Object::Reference(id) if seen.insert(*id) => {
                // Other import repairs (e.g. widget parents) run later. Missing
                // references have no object to retain; do not change their value.
                if let Some(value) = output.objects.get(id) {
                    pending.push(value);
                }
            }
            Object::Array(values) => pending.extend(values),
            Object::Dictionary(dict) => pending.extend(dict.iter().map(|(_, value)| value)),
            Object::Stream(stream) => pending.extend(stream.dict.iter().map(|(_, value)| value)),
            _ => {}
        }
        if pending.len() > MAX_VALUES {
            return Err(bad("resource reachability exceeds preservation limits"));
        }
    }
    output.objects.retain(|id, _| seen.contains(id));
    Ok(())
}

/// Isolated page resource import, with the same graph limits as import repair.
pub(crate) fn import_page_resources(source: &Document, page: ObjectId, output: &mut Document) -> EngineResult<Object> {
    let empty = Object::Dictionary(Dictionary::new());
    let value = resources(source, page)?.unwrap_or(&empty);
    let mut budget = Budget::default();
    Copier { source, output, map: HashMap::new(), budget: &mut budget }.value(value, 0)
}

pub(crate) fn inherited_page_resources(source: &Document, page: ObjectId) -> EngineResult<Option<&Object>> {
    resources(source, page)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    #[test]
    fn repaired_resources_do_not_double_the_intermediate_size_budget() {
        let mut source = Document::with_version("1.7");
        let pages = source.new_object_id();
        let payload = source.add_object(Stream::new(dictionary! {}, vec![b'x'; 64 * 1024]));
        let resource =
            dictionary! {"XObject" => dictionary! {"First" => payload, "Alias" => payload}};
        let page = source.add_object(dictionary! {"Type"=>"Page", "Parent"=>pages, "Resources"=>resource, "MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()]});
        source.objects.insert(
            pages,
            dictionary! {"Type"=>"Pages", "Kids"=>vec![page.into()], "Count"=>1}.into(),
        );
        let root = source.add_object(dictionary! {"Type"=>"Catalog", "Pages"=>pages});
        source.trailer.set("Root", root);
        let bytes = crate::pdf_output::save(&mut source).unwrap();
        let repaired = preserve_bytes(
            &bytes,
            &[SourceForm::new(&bytes, vec![(0, 0)])],
        )
        .unwrap()
        .unwrap();
        assert!(
            repaired.len() < bytes.len() + 4096,
            "obsolete resource graph doubled the repaired document: {} -> {}",
            bytes.len(),
            repaired.len()
        );
        let output = crate::pdf_output::load(&repaired).unwrap();
        assert!(!output.objects.contains_key(&payload));
        let resources = output
            .get_dictionary(page)
            .unwrap()
            .get(b"Resources")
            .unwrap()
            .as_dict()
            .unwrap();
        let xobjects = resources.get(b"XObject").unwrap().as_dict().unwrap();
        assert_eq!(
            xobjects.get(b"First").unwrap().as_reference().unwrap(),
            xobjects.get(b"Alias").unwrap().as_reference().unwrap()
        );
        // A tight writer budget admits the reachable document, not two copies.
        let mut output = output;
        let mut writer = crate::pdf_output::BoundedWriter::new(bytes.len() + 4096);
        output.save_to(&mut writer).unwrap();
        assert!(writer.finish().is_ok());
    }

    #[test]
    fn pruning_keeps_resources_reachable_from_an_unaffected_page_and_annotation() {
        let mut source = Document::with_version("1.7");
        let pages = source.new_object_id();
        let shared = source.add_object(Stream::new(dictionary! {}, vec![b'x'; 1000]));
        let annotation = source.add_object(
            dictionary! {"Type"=>"Annot","Subtype"=>"Stamp","AP"=>dictionary!{"N"=>shared}},
        );
        let first=source.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"Resources"=>dictionary!{"XObject"=>dictionary!{"First"=>shared}},"Annots"=>vec![annotation.into()]});
        let second=source.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"Resources"=>dictionary!{"XObject"=>dictionary!{"Safe"=>shared}}});
        source.objects.insert(
            pages,
            dictionary! {"Type"=>"Pages","Kids"=>vec![first.into(),second.into()],"Count"=>2}
                .into(),
        );
        let catalog = source.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
        source.trailer.set("Root", catalog);
        let bytes = crate::pdf_output::save(&mut source).unwrap();
        let repaired = preserve_bytes(
            &bytes,
            &[SourceForm::new(&bytes, vec![(0, 0), (1, 1)])],
        )
        .unwrap()
        .unwrap();
        let output = crate::pdf_output::load(&repaired).unwrap();
        assert!(output.objects.contains_key(&shared));
        assert!(output.objects.contains_key(&annotation));
        assert_eq!(
            output.get_dictionary(second).unwrap(),
            source.get_dictionary(second).unwrap()
        );
        assert_eq!(
            output.get_dictionary(annotation).unwrap(),
            source.get_dictionary(annotation).unwrap()
        );
    }

    #[test]
    fn inherited_resources_and_graph_cycles_are_preserved_but_page_trees_refused() {
        for indirect_type in [false, true] {
            let mut source = Document::with_version("1.7");
            let pages = source.new_object_id();
            let resource = source.new_object_id();
            source
                .objects
                .insert(resource, dictionary! {"First"=>resource}.into());
            let page = source.add_object(dictionary! {"Type"=>"Page","Parent"=>pages});
            source.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1,"Resources"=>resource}.into());
            let catalog = source.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
            source.trailer.set("Root", catalog);
            let bytes = crate::pdf_output::save(&mut source).unwrap();
            assert!(needed(&[SourceForm::new(&bytes, vec![(0, 0)])])
            .unwrap());
            let repaired = preserve_bytes(
                &bytes,
                &[SourceForm::new(&bytes, vec![(0, 0)])],
            )
            .unwrap()
            .unwrap();
            let output = crate::pdf_output::load(&repaired).unwrap();
            let new_resource = output
                .get_dictionary(page)
                .unwrap()
                .get(b"Resources")
                .unwrap()
                .as_reference()
                .unwrap();
            assert_eq!(
                output
                    .get_dictionary(new_resource)
                    .unwrap()
                    .get(b"First")
                    .unwrap()
                    .as_reference()
                    .unwrap(),
                new_resource
            );
            let kind = if indirect_type {
                Object::Reference(source.add_object(Object::Name(b"Page".to_vec())))
            } else {
                Object::Name(b"Page".to_vec())
            };
            source
                .get_dictionary_mut(resource)
                .unwrap()
                .set("Type", kind);
            let bytes = crate::pdf_output::save(&mut source).unwrap();
            assert!(
                preserve_bytes(
                    &bytes,
                    &[SourceForm::new(&bytes, vec![(0, 0)])]
                )
                .is_err(),
                "page-tree guard, indirect Type={indirect_type}"
            );
        }
    }
}
