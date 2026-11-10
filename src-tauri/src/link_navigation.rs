//! Recover Link navigation that PDFium cannot import one page at a time.
//! Preparation runs before native copying and bounds all reconstructed metadata.
use super::*;
use crate::acroforms::SourceForm;

const MAX_LINKS: usize = 10_000;
fn error(message: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("PDF links: {message}"))
}
#[derive(Clone)]
struct Link {
    rect: [f32; 4],
    name: Option<Vec<u8>>,
    navigation: Vec<(Vec<u8>, PdfValue)>,
}
pub(crate) struct Plan {
    pages: Vec<(usize, Vec<Link>)>,
    page_count: usize,
}

fn page_links<'a>(
    doc: &'a Document,
    page: ObjectId,
) -> EngineResult<Vec<(&'a Object, &'a Dictionary)>> {
    let mut links = Vec::new();
    let page = doc.get_dictionary(page).map_err(error)?;
    if let Some(annots) = optional(doc, page, b"Annots")? {
        let annotations = resolve(doc, annots)?.as_array().map_err(error)?;
        if annotations.len() > 100_000 {
            return Err(error("annotation count exceeds limits"));
        }
        for annotation in annotations {
            let data = dict(doc, annotation)?;
            let subtype = optional(doc, data, b"Subtype")?
                .map(|value| resolve(doc, value))
                .transpose()?;
            if subtype.is_some_and(|value| value.as_name().is_ok_and(|name| name == b"Link")) {
                if links.len() >= MAX_LINKS {
                    return Err(error("more than 10000 links"));
                }
                links.push((annotation, data));
            }
        }
    }
    Ok(links)
}
fn identity(doc: &Document, data: &Dictionary) -> EngineResult<([f32; 4], Option<Vec<u8>>)> {
    let values = resolve(doc, data.get(b"Rect").map_err(error)?)?
        .as_array()
        .map_err(error)?;
    if values.len() != 4 {
        return Err(error("link rectangle is malformed"));
    }
    let mut r = [0.; 4];
    for (result, value) in r.iter_mut().zip(values) {
        *result = resolve(doc, value)?.as_float().map_err(error)?;
        if !result.is_finite() {
            return Err(error("nonfinite link rectangle"));
        }
    }
    let rect = [
        r[0].min(r[2]),
        r[1].min(r[3]),
        r[0].max(r[2]),
        r[1].max(r[3]),
    ];
    let name = optional(doc, data, b"NM")?
        .map(|v| resolve(doc, v)?.as_str().map_err(error))
        .transpose()?;
    if name.is_some_and(|n| n.len() > 4096) {
        return Err(error("link identity exceeds limits"));
    }
    Ok((rect, name.map(<[u8]>::to_vec)))
}
// PdfValue intentionally carries only page identity. Actions which address
// annotations, fields or optional-content objects cannot safely use that model.
// Validate the original bounded values, before remapping can erase a malformed
// GoTo as though its valid target page had simply been omitted.
fn validate_action(value: &PdfValue) -> EngineResult<()> {
    let PdfValue::Dictionary(entries) = value else {
        return Err(error("an action must be a dictionary"));
    };
    let Some(PdfValue::Name(kind)) = entry(entries, b"S") else {
        return Err(error("an action must specify its type"));
    };
    let valid = match kind.as_slice() {
        b"GoTo" => {
            validate_destination(
                entry(entries, b"D").ok_or_else(|| error("GoTo action lacks destination"))?,
            )?;
            true
        }
        b"GoToR" => {
            matches!(
                entry(entries, b"F"),
                Some(PdfValue::String(_) | PdfValue::Dictionary(_))
            ) && match entry(entries, b"D") {
                Some(PdfValue::String(_) | PdfValue::Name(_)) => true,
                Some(PdfValue::Array(values)) => {
                    matches!(values.first(),Some(PdfValue::Integer(index)) if *index>=0)
                }
                _ => false,
            }
        }
        b"URI" => matches!(entry(entries, b"URI"), Some(PdfValue::String(_))),
        b"Named" => matches!(entry(entries, b"N"), Some(PdfValue::Name(_))),
        b"JavaScript" => matches!(
            entry(entries, b"JS"),
            Some(PdfValue::String(_) | PdfValue::Stream { .. })
        ),
        _ => {
            return Err(error(
                "this action type cannot yet be preserved without changing its targets",
            ))
        }
    };
    if !valid {
        return Err(error("an action has missing or unsupported parameters"));
    }
    match entry(entries, b"Next") {
        None | Some(PdfValue::Null) => (),
        Some(PdfValue::Array(actions)) => {
            for action in actions {
                validate_action(action)?;
            }
        }
        Some(action) => validate_action(action)?,
    }
    Ok(())
}

fn import_links(import: &mut Import<'_>, page: ObjectId) -> EngineResult<Vec<Link>> {
    let mut records = Vec::new();
    for (_, data) in page_links(import.doc, page)? {
        let (rect, name) = identity(import.doc, data)?;
        let mut navigation = Vec::new();
        for key in [b"Dest".as_slice(), b"A", b"AA"] {
            if let Some(value) = optional(import.doc, data, key)? {
                let value = if key == b"Dest" {
                    import.destination(value, 0)?
                } else {
                    if !matches!(resolve(import.doc, value)?, Object::Dictionary(_)) {
                        return Err(error("link actions must be dictionaries"));
                    }
                    let imported = import.value(value, 0, &mut HashSet::new())?;
                    if key == b"AA" {
                        let PdfValue::Dictionary(actions) = &imported else {
                            unreachable!()
                        };
                        for (_, action) in actions {
                            if !matches!(action, PdfValue::Null) {
                                validate_action(action)?;
                            }
                        }
                    } else {
                        validate_action(&imported)?;
                    }
                    imported
                };
                navigation.push((key.to_vec(), value));
            }
        }
        records.push(Link {
            rect,
            name,
            navigation,
        });
    }
    Ok(records)
}

pub(crate) fn prepare(sources: &[SourceForm<'_>]) -> EngineResult<Plan> {
    let page_count = sources.iter().try_fold(0usize, |sum, s| {
        sum.checked_add(s.pages.len())
            .ok_or_else(|| error("page count overflow"))
    })?;
    if page_count > 100_000 {
        return Err(error("too many output pages"));
    }
    let keys: HashSet<_> = (0..page_count).map(|p| p.to_string()).collect();
    let mut output_budget = Budget::default();
    let mut pages = Vec::new();
    let mut assigned = HashSet::new();
    let mut link_count = 0usize;
    for source in sources {
        let document = source.document()?;
        let ids: Vec<_> = document.get_pages().values().copied().collect();
        let mut mapping = HashMap::new();
        for (old, new) in &source.pages {
            if *old >= ids.len() || *new >= page_count || !assigned.insert(*new) {
                return Err(error("source page mapping is invalid"));
            }
            mapping
                .entry(old.to_string())
                .or_insert_with(|| new.to_string());
        }
        let mut import = None;
        let mut cache = HashMap::<usize, Vec<Link>>::new();
        for (old, new) in &source.pages {
            if !cache.contains_key(old) {
                let records = if page_links(&document, ids[*old])?.is_empty() {
                    Vec::new()
                } else {
                    if import.is_none() {
                        import = Some(Import::new(&document)?);
                    }
                    import_links(import.as_mut().unwrap(), ids[*old])?
                };
                cache.insert(*old, records);
            }
            // A link to this same source page belongs to this particular copy.
            let previous = mapping.insert(old.to_string(), new.to_string());
            let mut records = Vec::new();
            for link in &cache[old] {
                link_count += 1;
                if link_count > MAX_LINKS {
                    return Err(error("more than 10000 output links"));
                }
                output_budget.take(link.name.as_ref().map_or(0, Vec::len), 0)?;
                let mut navigation = Vec::new();
                for (key, value) in &link.navigation {
                    let mut value = map_value(value, &mapping);
                    if key == b"Dest" && dest_page(&value).is_none() {
                        continue;
                    }
                    if key == b"AA" {
                        if let PdfValue::Dictionary(entries) = &mut value {
                            entries.retain(|(_, v)| !matches!(v, PdfValue::Null));
                            if entries.is_empty() {
                                continue;
                            }
                        }
                    }
                    if matches!(value, PdfValue::Null) {
                        continue;
                    }
                    // One budget across all sources and duplicate output copies.
                    check_value(&value, &keys, &mut output_budget, 0)?;
                    navigation.push((key.clone(), value));
                }
                records.push(Link {
                    rect: link.rect,
                    name: link.name.clone(),
                    navigation,
                });
            }
            if let Some(previous) = previous {
                mapping.insert(old.to_string(), previous);
            } else {
                mapping.remove(&old.to_string());
            }
            pages.push((*new, records));
        }
    }
    Ok(Plan { pages, page_count })
}

impl Plan {
    pub(crate) fn apply(self, output: &mut Document) -> EngineResult<bool> {
        let ids: Vec<_> = output.get_pages().values().copied().collect();
        if ids.len() != self.page_count {
            return Err(error("output page count changed"));
        }
        let page_objects: HashMap<_, _> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| (i.to_string(), *id))
            .collect();
        let mut seen = HashSet::new();
        let mut changed = false;
        for (index, records) in self.pages {
            let output_links = page_links(output, ids[index])?;
            if output_links.len() != records.len() {
                return Err(error("native page import changed the link count"));
            }
            let mut link_ids = Vec::new();
            for ((value, data), record) in output_links.iter().zip(&records) {
                let id = value
                    .as_reference()
                    .map_err(|_| error("imported link lacks an indirect identity"))?;
                if !seen.insert(id) {
                    return Err(error("native link identity is shared between occurrences"));
                }
                let (rect, name) = identity(output, data)?;
                if rect
                    .iter()
                    .zip(record.rect)
                    .any(|(a, b)| (a - b).abs() > 0.01)
                    || name != record.name
                {
                    return Err(error("native page import changed link correspondence"));
                }
                link_ids.push(id);
            }
            for (id, record) in link_ids.into_iter().zip(records) {
                let mut navigation = Vec::new();
                for (key, value) in record.navigation {
                    navigation.push((key, pdf_value(&value, output, &page_objects)?));
                }
                let annotation = output.get_dictionary_mut(id).map_err(error)?;
                for key in [b"Dest".as_slice(), b"A", b"AA"] {
                    annotation.remove(key);
                }
                for (key, value) in navigation {
                    annotation.set(key, value);
                }
                annotation.set("P", ids[index]);
                changed = true;
            }
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Document, ObjectId, Vec<ObjectId>) {
        let mut doc = Document::with_version("1.7");
        let tree = doc.new_object_id();
        let page = doc.add_object(dictionary! {"Type"=>"Page","Parent"=>tree,
        "MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()]});
        let links: Vec<_> = (0..2).map(|i| doc.add_object(dictionary! {
            "Subtype"=>"Link", "NM"=>Object::string_literal(format!("link{i}")),
            "Rect"=>vec![0.into(),(i*30).into(),100.into(),(i*30+20).into()],
            "Border"=>vec![0.into(),0.into(),1.into()], "Dest"=>vec![Object::Reference(page),Object::Name(b"Fit".to_vec())]
        })).collect();
        doc.get_dictionary_mut(page).unwrap().set(
            "Annots",
            links
                .iter()
                .map(|id| Object::Reference(*id))
                .collect::<Vec<_>>(),
        );
        doc.objects.insert(
            tree,
            dictionary! {"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}.into(),
        );
        let root = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree});
        doc.trailer.set("Root", root);
        (doc, page, links)
    }
    fn plan(doc: &Document) -> Plan {
        let mut source = doc.clone();
        let mut bytes = Vec::new();
        source.save_to(&mut bytes).unwrap();
        prepare(&[SourceForm::new(&bytes, vec![(0, 0)])])
        .unwrap()
    }
    #[test]
    fn indirect_subtype_is_included_in_prepared_links() {
        let (mut doc, _, ids) = fixture();
        let kind = doc.add_object(Object::Name(b"Link".to_vec()));
        for id in ids {
            doc.get_dictionary_mut(id).unwrap().set("Subtype", kind);
        }
        assert_eq!(plan(&doc).pages[0].1.len(), 2);
    }
    #[test]
    fn rejects_changed_link_slots_and_shared_identity() {
        for kind in [
            "missing",
            "reordered",
            "rectangle",
            "name",
            "shared",
            "direct",
            "pages",
        ] {
            let (mut doc, page, ids) = fixture();
            let expected = plan(&doc);
            match kind {
                "missing" => doc
                    .get_dictionary_mut(page)
                    .unwrap()
                    .set("Annots", vec![Object::Reference(ids[0])]),
                "reordered" => doc.get_dictionary_mut(page).unwrap().set(
                    "Annots",
                    vec![Object::Reference(ids[1]), Object::Reference(ids[0])],
                ),
                "rectangle" => doc
                    .get_dictionary_mut(ids[0])
                    .unwrap()
                    .set("Rect", vec![0.into(), 0.into(), 90.into(), 20.into()]),
                "name" => doc
                    .get_dictionary_mut(ids[0])
                    .unwrap()
                    .set("NM", Object::string_literal("wrong")),
                "shared" => doc.get_dictionary_mut(page).unwrap().set(
                    "Annots",
                    vec![Object::Reference(ids[0]), Object::Reference(ids[0])],
                ),
                "direct" => {
                    let value = doc.get_object(ids[0]).unwrap().clone();
                    doc.get_dictionary_mut(page)
                        .unwrap()
                        .set("Annots", vec![value, Object::Reference(ids[1])]);
                }
                "pages" => {
                    let tree = doc
                        .get_dictionary(page)
                        .unwrap()
                        .get(b"Parent")
                        .unwrap()
                        .as_reference()
                        .unwrap();
                    doc.get_dictionary_mut(tree)
                        .unwrap()
                        .set("Kids", Vec::<Object>::new());
                }
                _ => unreachable!(),
            }
            assert!(expected.apply(&mut doc).is_err(), "{kind}");
        }
    }
    #[test]
    fn retains_link_identity_appearance_and_nonlink_annotations() {
        let (mut doc, page, ids) = fixture();
        let note = doc.add_object(
            dictionary! {"Subtype"=>"Text","Contents"=>Object::string_literal("untouched")},
        );
        doc.get_dictionary_mut(page).unwrap().set(
            "Annots",
            vec![
                Object::Reference(ids[0]),
                Object::Reference(note),
                Object::Reference(ids[1]),
            ],
        );
        let before = doc.get_object(note).unwrap().clone();
        let border = doc
            .get_dictionary(ids[0])
            .unwrap()
            .get(b"Border")
            .unwrap()
            .clone();
        assert!(plan(&doc).apply(&mut doc).unwrap());
        assert_eq!(doc.get_object(note).unwrap(), &before);
        assert_eq!(
            doc.get_dictionary(ids[0]).unwrap().get(b"Border").unwrap(),
            &border
        );
        let slots = doc
            .get_dictionary(page)
            .unwrap()
            .get(b"Annots")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(
            slots,
            &vec![
                Object::Reference(ids[0]),
                Object::Reference(note),
                Object::Reference(ids[1])
            ]
        );
    }
    #[test]
    fn rejects_ambiguous_or_out_of_range_page_maps() {
        let (mut doc, _, _) = fixture();
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        for pages in [vec![(0, 0), (0, 0)], vec![(1, 0)], vec![(0, 1)]] {
            assert!(prepare(&[SourceForm::new(&bytes, pages)])
            .is_err());
        }
    }
}
