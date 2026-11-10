//! Preflight borrowed annotation metadata before any import copies or decodes.
//! Asset caches and authored-AP proofs retain their separate existing limits.
use lopdf::{Document, Object};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use std::collections::BTreeMap;

struct Budget {
    metadata: usize,
    portable: usize,
    steps: usize,
}
impl Default for Budget {
    fn default() -> Self {
        Self { metadata: 16 * 1024 * 1024, portable: 32 * 1024 * 1024, steps: 1_000_000 }
    }
}
fn invalid() -> lopdf::Error {
    lopdf::Error::InvalidStream("Annotation import metadata exceeds its limits or cannot be inspected safely".into())
}
fn charge(left: &mut usize, amount: usize) -> Result<(), lopdf::Error> {
    *left = left.checked_sub(amount).ok_or_else(invalid)?;
    Ok(())
}
impl Budget {
    fn storage(&mut self, count: usize, size: usize) -> Result<(), lopdf::Error> {
        charge(&mut self.metadata, count.checked_mul(size).ok_or_else(invalid)?)
    }
    fn resolve<'a>(&mut self, doc: &'a Document, mut item: &'a Object) -> Result<&'a Object, lopdf::Error> {
        for _ in 0..=64 {
            charge(&mut self.steps, 1)?;
            match item {
                Object::Reference(id) => item = doc.objects.get(id).ok_or_else(invalid)?,
                _ => return Ok(item),
            }
        }
        Err(invalid())
    }
    fn json(&mut self, bytes: &[u8]) -> Result<(), lopdf::Error> {
        let mut exhausted = false;
        let mut decoder = serde_json::Deserializer::from_slice(bytes);
        let _ = Measure { budget: self, exhausted: &mut exhausted }.deserialize(&mut decoder);
        // Malformed/unknown records remain ineligible under persistence::decode.
        // Keep all charges even for failed attempts; only capacity failure
        // aborts metadata inspection for the complete source document.
        if exhausted { Err(invalid()) } else { Ok(()) }
    }
}

/// Walk without building a Value tree. Each value/key reserves container slack
/// and typed-record conversion before its decoder runs. Escaped-string scratch
/// is bounded separately by the cumulative 32 MiB input allowance; this is not
/// an exact process-RSS measurement.
struct Measure<'a> { budget: &'a mut Budget, exhausted: &'a mut bool }
impl Measure<'_> {
    fn reserve<E: de::Error>(&mut self, count: usize, size: usize) -> Result<(), E> {
        if self.budget.storage(count, size).is_err() || charge(&mut self.budget.steps, 2).is_err() {
            *self.exhausted = true;
            return Err(E::custom("portable metadata allocation budget"));
        }
        Ok(())
    }
}
impl<'de> DeserializeSeed<'de> for Measure<'_> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(mut self, decoder: D) -> Result<(), D::Error> {
        self.reserve(1, 128)?;
        decoder.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Measure<'_> {
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("bounded portable metadata") }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> { Ok(()) }
    fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> { Ok(()) }
    fn visit_i64<E: de::Error>(self, _: i64) -> Result<(), E> { Ok(()) }
    fn visit_u64<E: de::Error>(self, _: u64) -> Result<(), E> { Ok(()) }
    fn visit_f64<E: de::Error>(self, _: f64) -> Result<(), E> { Ok(()) }
    fn visit_str<E: de::Error>(mut self, value: &str) -> Result<(), E> { self.reserve(value.len(), 2) }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while seq.next_element_seed(Measure { budget: &mut *self.budget, exhausted: &mut *self.exhausted })?.is_some() {}
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while map.next_key_seed(Measure { budget: &mut *self.budget, exhausted: &mut *self.exhausted })?.is_some() {
            map.next_value_seed(Measure { budget: &mut *self.budget, exhausted: &mut *self.exhausted })?;
        }
        Ok(())
    }
}

pub(super) fn check(doc: &Document, pages: &BTreeMap<u32, lopdf::ObjectId>) -> Result<(), lopdf::Error> {
    check_with_budget(doc, pages, &mut Budget::default())
}
fn check_with_budget(doc: &Document, pages: &BTreeMap<u32, lopdf::ObjectId>, budget: &mut Budget) -> Result<(), lopdf::Error> {
    budget.storage(pages.len(), 128)?;
    for page in pages.values() {
        charge(&mut budget.steps, 1)?;
        let page = budget.resolve(doc, doc.objects.get(page).ok_or_else(invalid)?)?.as_dict()?;
        let Ok(annotations) = page.get(b"Annots") else { continue; };
        let annotations = budget.resolve(doc, annotations)?.as_array()?;
        // Account for Vec growth and per-slot metadata before constructing it.
        budget.storage(annotations.len(), 2 * std::mem::size_of::<super::AnnotationMetadata>())?;
        for item in annotations {
            let item = budget.resolve(doc, item)?.as_dict()?;
            let subtype = item.get(b"Subtype")?.as_name()?;
            budget.storage(subtype.len(), 1)?;
            if let Ok(note) = item.get(b"Contents") {
                let note = budget.resolve(doc, note)?;
                // Reserve for native UTF-16 conversion and owned UTF-8 text,
                // including transient buffers, before annotation.contents().
                if !matches!(note, Object::Null) { budget.storage(note.as_str()?.len(), 8)?; }
            }
            if let Ok(quads) = item.get(b"QuadPoints") {
                let quads = budget.resolve(doc, quads)?.as_array()?;
                charge(&mut budget.steps, quads.len())?;
                budget.storage(quads.len(), 2 * std::mem::size_of::<super::AnnotationRect>())?;
            }
            if let Ok(record) = item.get(b"Folio").and_then(Object::as_str) {
                // Charge attempts too, not only successfully decoded records.
                // Portable record storage has its own 32 MiB cumulative cap;
                // this is separate from the adapter's small scalar metadata.
                charge(&mut budget.portable, record.len())?;
                budget.json(record)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    #[test]
    fn highlight_admission_metadata_preflight_has_cumulative_and_alias_work_limits() {
        let mut doc = Document::with_version("1.7");
        let item = doc.add_object(dictionary! {"Subtype"=>"Highlight", "Contents"=>Object::string_literal("note")});
        let page = doc.add_object(dictionary! {"Annots"=>vec![Object::Reference(item); 2]});
        let pages = BTreeMap::from([(1, page)]);
        let mut budget = Budget::default();
        check_with_budget(&doc, &pages, &mut budget).unwrap();
        let used = 16 * 1024 * 1024 - budget.metadata;
        assert!(check_with_budget(&doc, &pages, &mut Budget { metadata: used - 1, ..Budget::default() }).is_err());
        assert!(check_with_budget(&doc, &pages, &mut Budget { steps: 2, ..Budget::default() }).is_err());
        doc.objects.insert(item, Object::Reference(item));
        assert!(check_with_budget(&doc, &pages, &mut Budget::default()).is_err());
    }

    #[test]
    fn highlight_admission_metadata_json_counts_keys_strings_and_failed_attempts() {
        let record = br#"{"text":"a\u0062c","values":[null,true,1,-1,1.25]}"#;
        let mut full = Budget::default(); full.json(record).unwrap();
        let used = 16 * 1024 * 1024 - full.metadata;
        assert!(used >= 10 * 128);
        assert!(Budget { metadata: used - 1, ..Budget::default() }.json(record).is_err());
        assert!(Budget { metadata: used, ..Budget::default() }.json(record).is_ok());
        assert!(Budget { steps: 1, ..Budget::default() }.json(record).is_err());
        let mut failed = Budget::default();
        failed.json(br#"["text", {"broken":"#).unwrap();
        assert!(failed.metadata < 16 * 1024 * 1024, "invalid records do not refund their allocations");
    }
}
