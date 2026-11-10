//! Source-bound destination namespaces. Typed action integration precedes Save admission.
#![allow(dead_code)]
use crate::{composition_pages::SourcePages, EngineError, EngineResult};
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, StringFormat};
use std::collections::{BTreeMap, BTreeSet};

struct Entry<'a> {
    value: &'a Object,
    renamed: Option<Vec<u8>>,
}
type Names<'a> = BTreeMap<Vec<u8>, Entry<'a>>;
struct Source<'a> {
    pages: BTreeSet<ObjectId>,
    legacy: Names<'a>,
    modern: Names<'a>,
    renamed: bool,
}
/// Borrows original values; no graph mutations or deep copies during inspection.
/// This budget stays cumulative through qualification and catalog staging.
pub(crate) struct Prepared<'a> {
    document: &'a Document,
    sources: Vec<Source<'a>>,
    copied_bindings: BTreeMap<(usize,ObjectId,bool,Vec<u8>),Vec<u8>>,
    copied_entries: BTreeMap<(bool,Vec<u8>),Object>,
    work: Work,
}
pub(crate) fn prepare<'a>(
    document: &'a Document,
    sources: &[SourcePages],
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<Prepared<'a>> {
    let mut work = Work::default();
    work.step(check, 0)?;
    require(sources.len() <= 100_000)?;
    let mut output = Vec::new();
    let mut all_pages = BTreeSet::new();
    let mut catalogs = BTreeSet::new();
    for source in sources {
        work.charge(128, check)?;
        let (catalog_id, catalog) = work.resolve(
            document,
            document.objects.get(&source.catalog).ok_or_else(invalid)?,
            check,
        )?;
        // SourcePages is already canonical, but do not accept repeated sources.
        require(catalogs.insert(catalog_id.unwrap_or(source.catalog)))?;
        let catalog = catalog.as_dict().map_err(bad)?;
        let mut pages = BTreeSet::new();
        for page in &source.pages {
            work.charge(64, check)?;
            require(all_pages.insert(*page) && pages.insert(*page))?;
        }
        let mut legacy = Names::new();
        let mut modern = Names::new();
        if let Some(value) = work.optional(document, catalog, b"Dests", check)? {
            let value = work
                .resolve(document, value, check)?
                .1
                .as_dict()
                .map_err(bad)?;
            for (key, value) in value.iter() {
                work.entry(check)?;
                validate_destination(document, value, &pages, true, &mut work, check)?;
                work.charge(key.len().checked_add(96).ok_or_else(invalid)?, check)?;
                legacy.insert(
                    key.clone(),
                    Entry {
                        value,
                        renamed: None,
                    },
                );
            }
        }
        if let Some(value) = work.optional(document, catalog, b"Names", check)? {
            let names = work
                .resolve(document, value, check)?
                .1
                .as_dict()
                .map_err(bad)?;
            for (key, value) in names.iter() {
                work.step(check, 0)?;
                require(
                    key == b"Dests"
                        || matches!(work.resolve(document, value, check)?.1, Object::Null),
                )?;
            }
            if let Some(value) = work.optional(document, names, b"Dests", check)? {
                read_tree(
                    document,
                    value,
                    &pages,
                    &mut modern,
                    &mut BTreeSet::new(),
                    &mut work,
                    check,
                    0,
                )?;
            }
        }
        output.push(Source {
            pages,
            legacy,
            modern,
            renamed: false,
        });
    }
    // Namespace identity is the operand type, never decoded text or a common map.
    for modern in [false, true] {
        let mut reserved = BTreeMap::<Vec<u8>, usize>::new();
        for source in &output {
            for key in (if modern {
                &source.modern
            } else {
                &source.legacy
            })
            .keys()
            {
                work.step(check, 0)?;
                if let Some(count) = reserved.get_mut(key) {
                    *count += 1;
                } else {
                    work.charge(key.len() + 64, check)?;
                    reserved.insert(key.clone(), 1);
                }
            }
        }
        for (source_index, source) in output.iter_mut().enumerate() {
            let mut serial = 0usize;
            for (key, entry) in if modern {
                &mut source.modern
            } else {
                &mut source.legacy
            } {
                work.step(check, 0)?;
                if reserved[key] < 2 {
                    continue;
                }
                loop {
                    work.step(check, 0)?;
                    serial = serial.checked_add(1).ok_or_else(invalid)?;
                    let candidate =
                        format!("Folio.Dest.{}.{}", source_index + 1, serial).into_bytes();
                    if !reserved.contains_key(&candidate) {
                        work.charge(candidate.len() * 2 + 64, check)?;
                        reserved.insert(candidate.clone(), 0);
                        entry.renamed = Some(candidate);
                        source.renamed = true;
                        break;
                    }
                }
            }
        }
    }
    work.step(check, 0)?;
    Ok(Prepared {
        document,
        sources: output,
        copied_bindings: BTreeMap::new(),
        copied_entries: BTreeMap::new(),
        work,
    })
}
impl<'doc> Prepared<'doc> {
    pub(crate) fn tick(
        &mut self,
        depth: usize,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<()> {
        self.work.step(check, depth)
    }
    pub(crate) fn visit(
        &mut self,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<()> {
        self.work.entry(check)
    }
    pub(crate) fn charge(
        &mut self,
        bytes: usize,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<()> {
        self.work.charge(bytes, check)
    }
    pub(crate) fn resolve<'a>(
        &mut self,
        value: &'a Object,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<(Option<ObjectId>, &'a Object)>
    where
        'doc: 'a,
    {
        self.work.resolve(self.document, value, check)
    }
    pub(crate) fn copy(
        &mut self,
        value: &Object,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<Object> {
        self.work.copy(value, check, 0)
    }
    pub(crate) fn any_renames(&self) -> bool {
        self.sources.iter().any(|source| source.renamed)
    }
    pub(crate) fn remote(
        &mut self,
        value: &Object,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<()> {
        let value = self.work.resolve(self.document, value, check)?.1;
        if matches!(value, Object::Name(_) | Object::String(..)) {
            return Ok(());
        }
        let values = value.as_array().map_err(bad)?;
        require(values.len() >= 2 && values.len() <= 6)?;
        require(
            self.work
                .resolve(self.document, &values[0], check)?
                .1
                .as_i64()
                .map_err(bad)?
                >= 0,
        )?;
        validate_view(self.document, values, &mut self.work, check)
    }
    /// Only call at a proved local Dest or GoTo/D edge. A replacement is a new
    /// typed operand; the original indirect string/name remains untouched.
    pub(crate) fn local(
        &mut self,
        source: usize,
        value: &Object,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<Option<Object>> {
        self.work.step(check, 0)?;
        let source = self.sources.get(source).ok_or_else(invalid)?;
        let value = self.work.resolve(self.document, value, check)?.1;
        let (entry, format) = match value {
            Object::Name(key) => (source.legacy.get(key).ok_or_else(invalid)?, None),
            Object::String(key, format) => {
                (source.modern.get(key).ok_or_else(invalid)?, Some(*format))
            }
            _ => {
                validate_destination(
                    self.document,
                    value,
                    &source.pages,
                    false,
                    &mut self.work,
                    check,
                )?;
                return Ok(None);
            }
        };
        if let Some(key) = &entry.renamed {
            self.work.charge(key.len() + 32, check)?;
            Ok(Some(match format {
                Some(format) => Object::String(key.clone(), format),
                None => Object::Name(key.clone()),
            }))
        } else {
            Ok(None)
        }
    }
    pub(crate) fn has_renames(&self, source: usize) -> EngineResult<bool> {
        let source = self.sources.get(source).ok_or_else(invalid)?;
        Ok(source.renamed)
    }
    /// Resolve only a proved local navigation operand, keeping Name and String
    /// namespaces distinct. Never use this for remote destinations.
    pub(crate) fn target(
        &mut self, source: usize, value: &Object,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<ObjectId> {
        let source = self.sources.get(source).ok_or_else(invalid)?;
        let value = self.work.resolve(self.document, value, check)?.1;
        let (value, named) = match value {
            Object::Name(key) => (source.legacy.get(key).ok_or_else(invalid)?.value, true),
            Object::String(key, _) => (source.modern.get(key).ok_or_else(invalid)?.value, true),
            _ => (value, false),
        };
        validate_destination(self.document, value, &source.pages, named, &mut self.work, check)
    }
    /// Staged catalog values, not publication. The caller must first inspect all
    /// typed action sites and prove every qualified local operand is rewritten.
    pub(crate) fn catalogs(
        &mut self,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<(Option<Object>, Option<Object>)> {
        self.selected_catalogs(None, check)
    }
    pub(crate) fn selected_catalogs(
        &mut self, keep: Option<&BTreeSet<ObjectId>>,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<(Option<Object>, Option<Object>)> {
        let mut legacy = Dictionary::new();
        let mut modern = BTreeMap::new();
        for source in &self.sources {
            for (key, entry) in &source.legacy {
                if let Some(keep) = keep {
                    let target=validate_destination(self.document,entry.value,&source.pages,true,&mut self.work,check)?;
                    if !keep.contains(&target) {continue;}
                }
                let key = entry.renamed.as_ref().unwrap_or(key);
                self.work.charge(key.len() + 64, check)?;
                require(!legacy.has(key))?;
                legacy.set(key.clone(), self.work.copy(entry.value, check, 0)?);
            }
            for (key, entry) in &source.modern {
                if let Some(keep) = keep {
                    let target=validate_destination(self.document,entry.value,&source.pages,true,&mut self.work,check)?;
                    if !keep.contains(&target) {continue;}
                }
                let key = entry.renamed.as_ref().unwrap_or(key);
                self.work.charge(key.len() + 96, check)?;
                require(!modern.contains_key(key))?;
                modern.insert(key.clone(), self.work.copy(entry.value, check, 0)?);
            }
        }
        for ((is_modern,key),value) in &self.copied_entries {
            self.work.charge(key.len()+96,check)?;
            if *is_modern {require(!modern.contains_key(key))?;modern.insert(key.clone(),self.work.copy(value,check,0)?);}
            else {require(!legacy.has(key))?;legacy.set(key.clone(),self.work.copy(value,check,0)?);}
        }
        let legacy = (!legacy.is_empty()).then_some(Object::Dictionary(legacy));
        let modern = if modern.is_empty() {
            None
        } else {
            self.work
                .charge(modern.len().checked_mul(64).ok_or_else(invalid)?, check)?;
            let mut names = Vec::with_capacity(modern.len() * 2);
            for (key, value) in modern {
                self.work.step(check, 0)?;
                names.push(Object::String(key, StringFormat::Hexadecimal));
                names.push(value);
            }
            Some(Object::Dictionary(
                dictionary! {"Dests"=>dictionary!{"Names"=>names}},
            ))
        };
        self.work.step(check, 0)?;
        Ok((legacy, modern))
    }
}

#[path="composition_destination_occurrences.rs"]
mod occurrences;

fn read_tree<'a>(
    doc: &'a Document,
    value: &'a Object,
    pages: &BTreeSet<ObjectId>,
    entries: &mut Names<'a>,
    seen: &mut BTreeSet<ObjectId>,
    work: &mut Work,
    check: &mut impl FnMut() -> EngineResult<()>,
    depth: usize,
) -> EngineResult<Option<(Vec<u8>, Vec<u8>)>> {
    work.step(check, depth)?;
    work.entry(check)?;
    let (id, value) = work.resolve(doc, value, check)?;
    if let Some(id) = id {
        require(seen.insert(id))?;
    }
    let node = value.as_dict().map_err(bad)?;
    for (key, _) in node.iter() {
        work.step(check, depth)?;
        require([b"Kids".as_slice(), b"Names", b"Limits"].contains(&key.as_slice()))?;
    }
    let names = work.optional(doc, node, b"Names", check)?;
    let kids = work.optional(doc, node, b"Kids", check)?;
    require(names.is_some() != kids.is_some())?;
    let mut range: Option<(Vec<u8>, Vec<u8>)> = None;
    if let Some(names) = names {
        let values = work.resolve(doc, names, check)?.1.as_array().map_err(bad)?;
        require(values.len() % 2 == 0)?;
        for pair in values.chunks_exact(2) {
            work.entry(check)?;
            let key = work
                .resolve(doc, &pair[0], check)?
                .1
                .as_str()
                .map_err(bad)?;
            require(
                range.as_ref().is_none_or(|(_, last)| last.as_slice() < key)
                    && !entries.contains_key(key),
            )?;
            validate_destination(doc, &pair[1], pages, true, work, check)?;
            work.charge(
                key.len()
                    .checked_mul(3)
                    .and_then(|n| n.checked_add(96))
                    .ok_or_else(invalid)?,
                check,
            )?;
            entries.insert(
                key.to_vec(),
                Entry {
                    value: &pair[1],
                    renamed: None,
                },
            );
            range = Some((
                range.map_or_else(|| key.to_vec(), |(first, _)| first),
                key.to_vec(),
            ));
        }
    } else {
        let values = work
            .resolve(doc, kids.unwrap(), check)?
            .1
            .as_array()
            .map_err(bad)?;
        require(!values.is_empty())?;
        for child in values {
            require(matches!(child, Object::Reference(_)))?;
            let child_range = read_tree(doc, child, pages, entries, seen, work, check, depth + 1)?
                .ok_or_else(invalid)?;
            require(range.as_ref().is_none_or(|(_, last)| last < &child_range.0))?;
            range = Some((
                range.map_or(child_range.0, |(first, _)| first),
                child_range.1,
            ));
        }
    }
    if let Some(limits) = work.optional(doc, node, b"Limits", check)? {
        let limits = work
            .resolve(doc, limits, check)?
            .1
            .as_array()
            .map_err(bad)?;
        require(limits.len() == 2)?;
        let low = work
            .resolve(doc, &limits[0], check)?
            .1
            .as_str()
            .map_err(bad)?;
        let high = work
            .resolve(doc, &limits[1], check)?
            .1
            .as_str()
            .map_err(bad)?;
        require(
            range
                .as_ref()
                .is_some_and(|(first, last)| first.as_slice() == low && last.as_slice() == high),
        )?;
    } else {
        require(depth == 0)?;
    }
    Ok(range)
}
fn validate_destination(
    doc: &Document,
    value: &Object,
    pages: &BTreeSet<ObjectId>,
    named: bool,
    work: &mut Work,
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<ObjectId> {
    let mut value = work.resolve(doc, value, check)?.1;
    if named {
        if let Object::Dictionary(dictionary) = value {
            value = work
                .resolve(doc, dictionary.get(b"D").map_err(bad)?, check)?
                .1;
        }
    }
    let values = value.as_array().map_err(bad)?;
    require(values.len() >= 2 && values.len() <= 6)?;
    require(matches!(values[0], Object::Reference(_)))?;
    let (page, _) = work.resolve(doc, &values[0], check)?;
    require(page.is_some_and(|id| pages.contains(&id)))?;
    validate_view(doc, values, work, check)?;
    page.ok_or_else(invalid)
}
fn validate_view(
    doc: &Document,
    values: &[Object],
    work: &mut Work,
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<()> {
    let fit = work
        .resolve(doc, &values[1], check)?
        .1
        .as_name()
        .map_err(bad)?;
    let (length, nullable) = match fit {
        b"XYZ" => (5, true),
        b"Fit" | b"FitB" => (2, false),
        b"FitH" | b"FitBH" | b"FitV" | b"FitBV" => (3, true),
        b"FitR" => (6, false),
        _ => return Err(invalid()),
    };
    require(values.len() == length)?;
    for (index, value) in values.iter().enumerate().skip(2) {
        let value = work.resolve(doc, value, check)?.1;
        if nullable && matches!(value, Object::Null) {
            continue;
        }
        let number = value.as_float().map_err(bad)?;
        require(number.is_finite())?;
        if fit == b"XYZ" && index == 4 {
            require(number >= 0.0)?;
        }
    }
    Ok(())
}

fn bad(error: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("PDF destinations: {error}"))
}
fn invalid() -> EngineError {
    bad("inconsistent or unsupported source navigation")
}
fn require(condition: bool) -> EngineResult<()> {
    if condition {
        Ok(())
    } else {
        Err(invalid())
    }
}
struct Work {
    steps: usize,
    entries: usize,
    bytes: usize,
}
impl Default for Work {
    fn default() -> Self {
        Self {
            steps: 0,
            entries: 0,
            bytes: 16 * 1024 * 1024,
        }
    }
}
impl Work {
    fn step(
        &mut self,
        check: &mut impl FnMut() -> EngineResult<()>,
        depth: usize,
    ) -> EngineResult<()> {
        check()?;
        self.steps += 1;
        require(self.steps <= 1_000_000 && depth <= 32)
    }
    fn entry(&mut self, check: &mut impl FnMut() -> EngineResult<()>) -> EngineResult<()> {
        self.step(check, 0)?;
        self.entries += 1;
        require(self.entries <= 100_000)
    }
    fn charge(
        &mut self,
        bytes: usize,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<()> {
        self.step(check, 0)?;
        self.bytes = self.bytes.checked_sub(bytes).ok_or_else(invalid)?;
        Ok(())
    }
    fn resolve<'a>(
        &mut self,
        doc: &'a Document,
        mut value: &'a Object,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<(Option<ObjectId>, &'a Object)> {
        self.step(check, 0)?;
        let mut seen = BTreeSet::new();
        let mut canonical = None;
        while let Object::Reference(id) = value {
            self.step(check, 0)?;
            require(seen.len() < 32 && seen.insert(*id))?;
            canonical = Some(*id);
            value = doc.objects.get(id).ok_or_else(invalid)?;
        }
        Ok((canonical, value))
    }
    fn optional<'a>(
        &mut self,
        doc: &'a Document,
        dictionary: &'a Dictionary,
        key: &[u8],
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<Option<&'a Object>> {
        self.step(check, 0)?;
        dictionary
            .get(key)
            .ok()
            .map(|value| {
                self.resolve(doc, value, check)
                    .map(|(_, resolved)| (!matches!(resolved, Object::Null)).then_some(value))
            })
            .transpose()
            .map(Option::flatten)
    }
    fn copy(
        &mut self,
        value: &Object,
        check: &mut impl FnMut() -> EngineResult<()>,
        depth: usize,
    ) -> EngineResult<Object> {
        self.step(check, depth)?;
        self.charge(64, check)?;
        Ok(match value {
            Object::Array(values) => {
                let mut output = Vec::new();
                for value in values {
                    output.push(self.copy(value, check, depth + 1)?);
                }
                Object::Array(output)
            }
            Object::Dictionary(dictionary) => {
                let mut output = Dictionary::new();
                for (key, value) in dictionary.iter() {
                    self.charge(key.len() + 64, check)?;
                    output.set(key.clone(), self.copy(value, check, depth + 1)?);
                }
                Object::Dictionary(output)
            }
            // A valid destination can retain stream metadata through indirect
            // references, which are copied as references without expansion.
            Object::Stream(_) => return Err(invalid()),
            Object::String(bytes, format) => {
                self.charge(bytes.len(), check)?;
                Object::String(bytes.clone(), *format)
            }
            Object::Name(bytes) => {
                self.charge(bytes.len(), check)?;
                Object::Name(bytes.clone())
            }
            _ => value.clone(),
        })
    }
}
#[cfg(test)]
#[path = "composition_destination_tests.rs"]
mod tests;
