//! Disable native internal actions during editing without dropping their graph.
use super::{bad, load, resolve, Budget};
use crate::EngineResult;
use lopdf::{Document, Object, ObjectId};
use std::collections::HashMap;

const ACTIONS: &[&[u8]] = &[
    b"GoTo",
    b"GoToR",
    b"GoToE",
    b"Launch",
    b"Thread",
    b"URI",
    b"Sound",
    b"Movie",
    b"Hide",
    b"Named",
    b"SubmitForm",
    b"ResetForm",
    b"ImportData",
    b"JavaScript",
    b"SetOCGState",
    b"Rendition",
    b"Trans",
    b"GoTo3DView",
    b"RichMediaExecute",
];
pub(crate) struct ActionMask {
    prefix: Vec<u8>,
    count: usize,
}
impl ActionMask {
    pub(crate) fn apply(doc: &mut Document) -> EngineResult<Self> {
        let mut this = Self {
            prefix: format!("FolioNoAction{}_", uuid::Uuid::new_v4().simple()).into_bytes(),
            count: 0,
        };
        let mut names = HashMap::new();
        if doc.objects.len() > 100_000 {
            return Err(bad("form editing document exceeds 100000 objects"));
        }
        // Native serialization drops unreachable objects. Count and mask only
        // the live graph so obsolete imported actions cannot make restoration
        // fail. This is a private editing copy; source bytes remain untouched.
        crate::page_resources::prune_unreachable(doc)?;
        // Resolve indirect S names before taking mutable references. All changes
        // keep the action graph reachable; only recognized action types become an
        // unknown per-edit name that PDFium cannot dispatch.
        for (id, value) in &doc.objects {
            if matches!(value, Object::Name(_) | Object::Reference(_)) {
                if let Object::Name(name) = resolve(doc, value)? {
                    names.insert(*id, name.clone());
                }
            }
        }
        let mut budget = Budget::default();
        for value in doc.objects.values_mut() {
            this.walk(value, &names, &mut budget, 0, false)?;
        }
        Ok(this)
    }
    pub(crate) fn changed(&self) -> bool {
        self.count != 0
    }
    fn walk(
        &mut self,
        value: &mut Object,
        names: &HashMap<ObjectId, Vec<u8>>,
        budget: &mut Budget,
        depth: usize,
        restore: bool,
    ) -> EngineResult<()> {
        budget.take(0, depth)?;
        let dict = match value {
            Object::Array(values) => {
                for value in values {
                    self.walk(value, names, budget, depth + 1, restore)?;
                }
                return Ok(());
            }
            Object::Dictionary(dict) => dict,
            Object::Stream(stream) => &mut stream.dict,
            _ => return Ok(()),
        };
        if let Ok(value) = dict.get(b"S") {
            let name = match value {
                Object::Name(name) => Some(name.as_slice()),
                Object::Reference(id) => names.get(id).map(Vec::as_slice),
                _ => None,
            };
            if let Some(name) = name {
                if restore {
                    if let Some(original) = name.strip_prefix(self.prefix.as_slice()) {
                        if !ACTIONS.contains(&original) {
                            return Err(bad("invalid temporary action marker"));
                        }
                        let original = original.to_vec();
                        dict.set("S", Object::Name(original));
                        self.count += 1;
                    }
                } else if ACTIONS.contains(&name) {
                    let mut marker = self.prefix.clone();
                    marker.extend_from_slice(name);
                    dict.set("S", Object::Name(marker));
                    self.count += 1;
                }
            }
        }
        for (_, value) in dict.iter_mut() {
            self.walk(value, names, budget, depth + 1, restore)?;
        }
        Ok(())
    }
    pub(crate) fn restore(&self, bytes: Vec<u8>) -> EngineResult<Vec<u8>> {
        if !self.changed() {
            return Ok(bytes);
        }
        let mut doc = load(&bytes)?;
        let mut restored = Self {
            prefix: self.prefix.clone(),
            count: 0,
        };
        let mut budget = Budget::default();
        for value in doc.objects.values_mut() {
            restored.walk(value, &HashMap::new(), &mut budget, 0, true)?;
        }
        if restored.count != self.count {
            return Err(bad("native editing did not preserve all document actions"));
        }
        let mut output = vec![];
        doc.save_to(&mut output).map_err(bad)?;
        if output.len() > 512 * 1024 * 1024 {
            return Err(bad("filled document exceeds 512 MiB"));
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reachable_actions_remain_mandatory_after_obsolete_actions_are_pruned() {
        let mut doc = Document::with_version("1.7");
        let action = doc.add_object(
            lopdf::dictionary! {"S"=>"ResetForm","Fields"=>vec![Object::string_literal("answer")]},
        );
        let obsolete =
            doc.add_object(lopdf::dictionary! {"S"=>"Hide","T"=>Object::string_literal("unused")});
        let root = doc.add_object(lopdf::dictionary! {"Type"=>"Catalog","A"=>action});
        doc.trailer.set("Root", root);
        let mask = ActionMask::apply(&mut doc).unwrap();
        assert_eq!(mask.count, 1);
        assert!(!doc.objects.contains_key(&obsolete));
        let mut intact = vec![];
        doc.save_to(&mut intact).unwrap();
        let restored = Document::load_mem(&mask.restore(intact).unwrap()).unwrap();
        assert_eq!(
            restored
                .get_dictionary(action)
                .unwrap()
                .get(b"S")
                .unwrap()
                .as_name()
                .unwrap(),
            b"ResetForm"
        );
        doc.get_dictionary_mut(root).unwrap().remove(b"A");
        crate::page_resources::prune_unreachable(&mut doc).unwrap();
        let mut lost = vec![];
        doc.save_to(&mut lost).unwrap();
        assert!(
            mask.restore(lost).is_err(),
            "lost reachable action was silently accepted"
        );
    }
}
