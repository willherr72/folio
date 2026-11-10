//! Prove which obsolete generated answer names have no remaining consumers.
use super::*;

pub(super) fn verify_resources(
    doc: &Document,
    resources: &Dictionary,
    ops: &[Operation],
    constructed: &str,
) -> EngineResult<()> {
    Walk {
        doc,
        root: resources,
        constructed: Some(constructed),
        candidates: std::collections::HashSet::new(),
        visited: std::collections::HashSet::new(),
        remaining: 64 * 1024 * 1024,
        visits: 0,
        publication: None,
    }
    .operations(ops, resources, 0)
}

pub(super) fn unused_answers(
    doc: &Document,
    resources: &Dictionary,
    ops: &[Operation],
    constructed: &str,
) -> EngineResult<Vec<Vec<u8>>> {
    let Some(objects) = resources.get(b"XObject").ok() else {
        return Ok(Vec::new());
    };
    let objects = dictionary(doc, objects)?;
    let mut walk = Walk {
        doc,
        root: resources,
        constructed: Some(constructed),
        candidates: std::collections::HashSet::new(),
        visited: std::collections::HashSet::new(),
        remaining: 64 * 1024 * 1024,
        visits: 0,
        publication: None,
    };
    for (name, _) in objects.iter() {
        walk.tick()?;
        if name.starts_with(b"FolioAnswer_") {
            walk.charge(name.len().saturating_add(64))?;
            walk.candidates.insert(name.clone());
        }
    }
    if !walk.candidates.is_empty() {
        walk.operations(ops, resources, 0)?;
    }
    Ok(walk.candidates.into_iter().collect())
}

/// Test the retained dependency graph without inheriting any page categories.
/// Charge this additional proof to the same aggregate publication budget.
pub(super) fn verify_local_resources(
    doc: &Document,
    resources: &Dictionary,
    ops: &[Operation],
    omitted: std::ops::Range<usize>,
    budget: &mut FillAppearanceBudget<'_>,
) -> EngineResult<()> {
    let mut walk = Walk {
        doc,
        root: resources,
        constructed: None,
        candidates: std::collections::HashSet::new(),
        visited: std::collections::HashSet::new(),
        remaining: 64 * 1024 * 1024,
        visits: 0,
        publication: Some(budget),
    };
    walk.operations(&ops[..omitted.start], resources, 0)?;
    walk.operations(&ops[omitted.end..], resources, 0)
}

struct Walk<'a, 'b, 'c> {
    doc: &'a Document,
    root: &'a Dictionary,
    constructed: Option<&'a str>,
    candidates: std::collections::HashSet<Vec<u8>>,
    visited: std::collections::HashSet<(usize, usize)>,
    remaining: usize,
    visits: usize,
    publication: Option<&'b mut FillAppearanceBudget<'c>>,
}
impl<'a, 'b, 'c> Walk<'a, 'b, 'c> {
    fn tick(&mut self) -> EngineResult<()> {
        if let Some(budget) = &mut self.publication {
            budget.reserve_bytes(64)?;
        }
        self.visits += 1;
        if self.visits > 100_000 {
            return Err(bad("answer resource proof exceeds its traversal limit"));
        }
        Ok(())
    }
    fn charge(&mut self, bytes: usize) -> EngineResult<()> {
        if let Some(budget) = &mut self.publication {
            budget.reserve_bytes(bytes)?;
        }
        self.remaining = self
            .remaining
            .checked_sub(bytes)
            .ok_or_else(|| bad("answer resource proof exceeds 64 MiB"))?;
        Ok(())
    }
    fn resource(
        &self,
        resources: &'a Dictionary,
        category: &[u8],
        name: &[u8],
    ) -> EngineResult<&'a Object> {
        let group = dictionary(
            self.doc,
            resources
                .get(category)
                .map_err(|_| bad("unbound retained appearance resources"))?,
        )?;
        resolve(
            self.doc,
            group
                .get(name)
                .map_err(|_| bad("unbound retained appearance resource name"))?,
        )
    }
    fn local(
        &self,
        dict: &'a Dictionary,
        inherited: &'a Dictionary,
    ) -> EngineResult<&'a Dictionary> {
        dict.get(b"Resources")
            .ok()
            .map(|v| dictionary(self.doc, v))
            .transpose()
            .map(|v| v.unwrap_or(inherited))
    }
    fn stream(
        &mut self,
        value: &'a Object,
        inherited: &'a Dictionary,
        depth: usize,
    ) -> EngineResult<()> {
        self.tick()?;
        if depth > 64 {
            return Err(bad("answer resource proof exceeds 64 levels"));
        }
        let stream = resolve(self.doc, value)?
            .as_stream()
            .map_err(|_| bad("retained appearance dependency is not a stream"))?;
        let resources = self.local(&stream.dict, inherited)?;
        let key = (
            stream as *const Stream as usize,
            resources as *const Dictionary as usize,
        );
        if self.visited.contains(&key) {
            return Ok(());
        }
        self.charge(64)?;
        self.visited.insert(key);
        let bytes = stream
            .get_plain_content_with_limit(MAX_BYTES.min(self.remaining))
            .map_err(|_| bad("retained appearance dependency exceeds decoding limit"))?;
        self.charge(preflight_cost(&bytes)?)?;
        let content = Content::decode_strict(&bytes)
            .map_err(|_| bad("malformed retained appearance dependency"))?;
        self.operations(&content.operations, resources, depth)
    }
    fn font(
        &mut self,
        value: &'a Object,
        inherited: &'a Dictionary,
        depth: usize,
    ) -> EngineResult<()> {
        let font = dictionary(self.doc, value)?;
        if font
            .get(b"Subtype")
            .ok()
            .map(|v| resolve(self.doc, v))
            .transpose()?
            .and_then(|v| v.as_name().ok())
            != Some(b"Type3")
        {
            return Ok(());
        }
        let resources = self.local(font, inherited)?;
        let chars = dictionary(
            self.doc,
            font.get(b"CharProcs")
                .map_err(|_| bad("missing Type3 character programs"))?,
        )?;
        for (_, value) in chars.iter() {
            self.stream(value, resources, depth + 1)?;
        }
        Ok(())
    }
    fn operations(
        &mut self,
        ops: &[Operation],
        resources: &'a Dictionary,
        depth: usize,
    ) -> EngineResult<()> {
        for op in ops {
            self.tick()?;
            // These dependencies are not traversed by the answer-pruning proof.
            // Keep the full inherited context instead of certifying them local.
            if self.publication.is_some() && matches!(op.operator.as_str(), "sh" | "BDC" | "DP") {
                return Err(bad("retained resource operator requires its page context"));
            }
            let category = match op.operator.as_str() {
                "Do" => b"XObject".as_slice(),
                "Tf" => b"Font".as_slice(),
                "gs" => b"ExtGState".as_slice(),
                "SCN" | "scn" => b"Pattern".as_slice(),
                _ => continue,
            };
            let operand = if category == b"Pattern" {
                op.operands.last()
            } else {
                op.operands.first()
            };
            let Some(operand) = operand else {
                return Err(bad("missing retained appearance operand"));
            };
            let name = match operand.as_name() {
                Ok(name) => name,
                Err(_) if category == b"Pattern" => continue,
                _ => return Err(bad("invalid retained appearance resource name")),
            };
            let root = std::ptr::eq(resources, self.root);
            if root
                && self
                    .constructed
                    .is_some_and(|constructed| name == constructed.as_bytes())
                && matches!(category, b"XObject" | b"ExtGState")
            {
                continue;
            }
            if root && category == b"XObject" {
                self.candidates.remove(name);
            }
            let value = self.resource(resources, category, name)?;
            match category {
                b"XObject" => {
                    let stream = value
                        .as_stream()
                        .map_err(|_| bad("invalid retained XObject"))?;
                    let subtype = resolve(
                        self.doc,
                        stream
                            .dict
                            .get(b"Subtype")
                            .map_err(|_| bad("missing XObject subtype"))?,
                    )?
                    .as_name()
                    .map_err(|_| bad("invalid XObject subtype"))?;
                    match subtype {
                        b"Form" => self.stream(value, resources, depth + 1)?,
                        b"Image" => {}
                        _ => return Err(bad("unsupported retained XObject")),
                    }
                }
                b"Font" => self.font(value, resources, depth)?,
                b"ExtGState" => {
                    let state = dictionary(self.doc, value)?;
                    if let Ok(mask) = state.get(b"SMask") {
                        let mask = resolve(self.doc, mask)?;
                        if mask.as_name().ok() != Some(b"None") {
                            let mask = mask
                                .as_dict()
                                .map_err(|_| bad("invalid retained soft mask"))?;
                            self.stream(
                                mask.get(b"G")
                                    .map_err(|_| bad("missing soft-mask program"))?,
                                resources,
                                depth + 1,
                            )?;
                        }
                    }
                    if let Ok(font) = state.get(b"Font") {
                        let font = resolve(self.doc, font)?
                            .as_array()
                            .map_err(|_| bad("invalid state font"))?;
                        self.font(
                            font.first().ok_or_else(|| bad("missing state font"))?,
                            resources,
                            depth,
                        )?;
                    }
                }
                b"Pattern" => {
                    if let Object::Stream(_) = value {
                        self.stream(value, resources, depth + 1)?;
                    }
                }
                _ => unreachable!(),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn local_proof(bytes: &[u8], limit: usize) -> (bool, bool) {
        let doc = Document::new();
        let native = Document::new();
        let mut staging = Document::new();
        let mut budget =
            FillAppearanceBudget::with_limit(&doc, &native, &mut staging, limit).unwrap();
        let ops = Content::decode_strict(bytes).unwrap().operations;
        let result = verify_local_resources(&doc, &Dictionary::new(), &ops, 0..0, &mut budget);
        (result.is_ok(), budget.reserve_bytes(1).is_ok())
    }
    #[test]
    fn local_proof_has_no_constructed_empty_name_exemption() {
        assert!(!local_proof(b"/ gs", 4096).0);
        assert!(!local_proof(b"/ Do", 4096).0);
    }
    #[test]
    fn local_proof_does_not_certify_untracked_resource_operators() {
        for bytes in [
            b"/Shade sh".as_slice(),
            b"/Tag /Props BDC EMC",
            b"/Tag /Props DP",
        ] {
            assert!(!local_proof(bytes, 4096).0);
        }
    }
    #[test]
    fn local_proof_charges_visits_and_keeps_exhaustion_sticky() {
        assert_eq!(local_proof(b"q Q", 129), (true, true));
        assert_eq!(local_proof(b"q Q", 100), (false, false));
    }
    fn form(bytes: &[u8], resources: Option<Dictionary>) -> Stream {
        let mut dict = dictionary! {"Type"=>"XObject", "Subtype"=>"Form"};
        if let Some(resources) = resources {
            dict.set("Resources", resources);
        }
        Stream::new(dict, bytes.to_vec())
    }
    fn fixture() -> (Document, Dictionary) {
        let mut doc = Document::new();
        let old = doc.add_object(form(b"", Some(Dictionary::new())));
        (
            doc,
            dictionary! {"XObject"=>dictionary! {"FolioAnswer_old"=>old}},
        )
    }
    fn unused(doc: &Document, resources: &Dictionary, bytes: &[u8]) -> EngineResult<Vec<Vec<u8>>> {
        unused_answers(
            doc,
            resources,
            &Content::decode_strict(bytes).unwrap().operations,
            "FolioAnswer_new",
        )
    }
    #[test]
    fn obsolete_answer_is_removed_but_direct_use_and_constructed_answer_are_retained() {
        let (doc, resources) = fixture();
        assert_eq!(
            unused(&doc, &resources, b"/FolioAnswer_new Do").unwrap(),
            vec![b"FolioAnswer_old".to_vec()]
        );
        assert!(unused(&doc, &resources, b"/FolioAnswer_old Do")
            .unwrap()
            .is_empty());
    }
    #[test]
    fn nested_forms_inherit_resources_only_when_they_have_no_local_dictionary() {
        for own in [false, true] {
            let (mut doc, mut resources) = fixture();
            let local = own.then(|| resources.clone());
            let caption = doc.add_object(form(b"/FolioAnswer_old Do", local));
            resources
                .get_mut(b"XObject")
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("Caption", caption);
            assert_eq!(
                unused(&doc, &resources, b"/Caption Do").unwrap().len(),
                usize::from(own)
            );
        }
    }
    #[test]
    fn type3_softmask_and_tiling_pattern_dependencies_keep_inherited_answers() {
        for kind in ["font", "mask", "pattern"] {
            let (mut doc, mut resources) = fixture();
            let stream = doc.add_object(form(b"/FolioAnswer_old Do", None));
            let code = match kind {
                "font" => {
                    resources.set("Font", dictionary! {"F1"=>dictionary! {"Subtype"=>"Type3", "CharProcs"=>dictionary! {"A"=>stream}}});
                    b"BT /F1 12 Tf (A) Tj ET".as_slice()
                }
                "mask" => {
                    resources.set(
                        "ExtGState",
                        dictionary! {"GS"=>dictionary! {"SMask"=>dictionary! {"G"=>stream}}},
                    );
                    b"/GS gs".as_slice()
                }
                _ => {
                    doc.get_object_mut(stream)
                        .unwrap()
                        .as_stream_mut()
                        .unwrap()
                        .dict
                        .set("PatternType", 1);
                    resources.set("Pattern", dictionary! {"P1"=>stream});
                    b"/Pattern cs /P1 scn".as_slice()
                }
            };
            assert!(unused(&doc, &resources, code).unwrap().is_empty(), "{kind}");
        }
    }
    #[test]
    fn aliases_cycles_and_unprovable_inline_images_are_bounded() {
        let (mut doc, mut resources) = fixture();
        let nested = doc.add_object(form(b"/Caption Do /FolioAnswer_old Do", None));
        let alias = doc.add_object(Object::Reference(nested));
        resources
            .get_mut(b"XObject")
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("Caption", alias);
        assert!(unused(&doc, &resources, b"/Caption Do").unwrap().is_empty());
        doc.get_object_mut(nested)
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .set_plain_content(b"BI /W 1 /H 1 /BPC 8 /CS /G ID x EI".to_vec());
        assert!(unused(&doc, &resources, b"/Caption Do").is_err());
    }
}
