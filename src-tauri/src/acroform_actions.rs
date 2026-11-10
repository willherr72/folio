//! Action dependencies carried with form fields; none are executed by Folio.
use super::*;

fn needs_retained_action(doc:&Document, annotation:&Dictionary)->EngineResult<bool> {
    if is_widget(doc,annotation)? {return Ok(false);}
    let link=optional(doc,annotation,b"Subtype")?.map(|v|resolve(doc,v)).transpose()?.and_then(|v|v.as_name().ok())==Some(b"Link");
    // Link A is reconstructed later by the dedicated navigation repair. AA
    // still belongs here because it may refer to the new shared field graph.
    Ok(annotation.has(b"AA")||(!link&&annotation.has(b"A")))
}

pub(super) fn retained_page_dependencies(
    copier: &mut Copier<'_>, request: &SourceForm<'_>, pages: &[ObjectId], output: &[ObjectId], fields: &HashSet<ObjectId>,
) -> EngineResult<()> {
    // Each occurrence has its own action graph and self-page mapping. Do not
    // reuse copied action IDs from another page or the catalog action pass.
    let page_ids:HashSet<_>=pages.iter().copied().collect();
    let map:HashMap<_,_>=copier.map.iter().filter(|(id,_)|fields.contains(id)||page_ids.contains(id)).map(|(k,v)|(*k,v.clone())).collect();
    for &(old_index,new_index) in &request.pages {
        copier.budget.take(0,0)?;
        let old=pages[old_index];let new=output[new_index];
        let source=copier.source.get_dictionary(old).map_err(bad)?;
        let mut needed=source.has(b"AA");
        if let Some(values)=optional(copier.source,source,b"Annots")? {
            for value in resolve(copier.source,values)?.as_array().map_err(bad)? {
                copier.budget.take(0,0)?;
                let annotation=dict(copier.source,value)?;
                if needs_retained_action(copier.source,annotation)? {needed=true;}
            }
        }
        if !needed {continue;}
        // Budget even memo-table copying, before allocation; otherwise many
        // pages sharing a large field graph make this pass quadratic for free.
        for _ in 0..map.len()+copier.annotations.len()+copier.annotations.values().map(Vec::len).sum::<usize>() {
            copier.budget.take(32,0)?;
        }
        let mut local=Copier {source:copier.source,out:copier.out,map:map.clone(),annotations:copier.annotations.clone(),active:HashSet::new(),budget:copier.budget};
        local.map.insert(old,Object::Reference(new));
        let source=local.source.get_dictionary(old).map_err(bad)?;
        if let Some(value)=optional(local.source,source,b"AA")? {
            let value=local.value(value,0)?;
            local.out.get_dictionary_mut(new).map_err(bad)?.set("AA",value);
        }
        let Some(values)=optional(local.source,source,b"Annots")? else {continue};
        let originals=resolve(local.source,values)?.as_array().map_err(bad)?;
        let target=local.out.get_dictionary(new).map_err(bad)?.get(b"Annots").map_err(bad)?;
        let target=resolve(local.out,target)?.as_array().map_err(bad)?;
        if originals.len()!=target.len() {return Err(bad("retained annotation count changed"));}
        let mut direct_budget=crate::form_graph::CopyBudget::default();
        for value in target {local.budget.take(0,0)?;if let Object::Dictionary(dict)=value {direct_budget.dictionary(dict)?;}}
        let mut target=target.clone();
        for (original,destination) in originals.iter().zip(&mut target) {
            let original=dict(local.source,original)?;
            if !needs_retained_action(local.source,original)? {continue;}
            if optional(local.source,original,b"Subtype")?.map(|v|resolve(local.source,v)).transpose()?.and_then(|v|v.as_name().ok())
                != optional(local.out,dict(local.out,destination)?,b"Subtype")?.map(|v|resolve(local.out,v)).transpose()?.and_then(|v|v.as_name().ok()) {
                return Err(bad("retained annotation identity changed"));
            }
            for key in [b"A".as_slice(),b"AA".as_slice()] {
                if key==b"A" && optional(local.source,original,b"Subtype")?.map(|v|resolve(local.source,v)).transpose()?.and_then(|v|v.as_name().ok())==Some(b"Link") {continue;}
                if let Some(value)=optional(local.source,original,key)? {
                    let value=local.value(value,0)?;
                    let dest=match &mut *destination {Object::Reference(id)=>local.out.get_dictionary_mut(*id).map_err(bad)?,value=>value.as_dict_mut().map_err(bad)?};
                    dest.set(key.to_vec(),value);
                }
            }
        }
        local.out.get_dictionary_mut(new).map_err(bad)?.set("Annots",target);
    }
    Ok(())
}

impl Copier<'_> {
    pub(super) fn action(&mut self, d: &Dictionary, depth: usize) -> EngineResult<Option<Object>> {
        let kind = d
            .get(b"S")
            .ok()
            .and_then(|v| resolve(self.source, v).ok())
            .and_then(|v| v.as_name().ok());
        if kind == Some(b"GoTo") {
            let destination = self.local_destination(d.get(b"D").map_err(bad)?, depth + 1)?;
            let destination = self.value(&destination, depth + 1)?;
            let array = destination.as_array().map_err(bad)?;
            if matches!(array.first(), Some(Object::Null)) {
                return Ok(Some(Object::Null));
            }
            let mut rest = d.clone();
            rest.remove(b"D");
            let mut result = self.dictionary(&rest, depth)?;
            result.set("D", destination);
            return Ok(Some(Object::Dictionary(result)));
        }
        if kind == Some(b"Hide") {
            let target = d.get(b"T").map_err(bad)?;
            let targets = self.hide_targets(target, depth + 1)?;
            if targets.is_empty() {
                return Ok(Some(Object::Null));
            }
            let mut rest = d.clone();
            rest.remove(b"T");
            let mut result = self.dictionary(&rest, depth)?;
            result.set(
                "T",
                if targets.len() == 1 {
                    targets[0].clone()
                } else {
                    Object::Array(targets)
                },
            );
            return Ok(Some(Object::Dictionary(result)));
        }
        Ok(None)
    }
    fn local_destination(&mut self, value: &Object, depth: usize) -> EngineResult<Object> {
        self.budget.take(0, depth)?;
        let value = resolve(self.source, value)?.clone();
        match value {
            Object::Array(mut values) => {
                if values.len() < 2 {
                    return Err(bad("empty local destination"));
                }
                if let Object::Integer(index) = values[0] {
                    let pages = self.source.get_pages();
                    let id = pages
                        .values()
                        .nth(usize::try_from(index).map_err(bad)?)
                        .ok_or_else(|| bad("unknown destination page"))?;
                    values[0] = Object::Reference(*id);
                }
                let id = values[0].as_reference().map_err(bad)?;
                if !self.source.get_pages().values().any(|page| *page == id) {
                    return Err(bad("local destination does not target a page"));
                }
                Ok(Object::Array(values))
            }
            Object::Dictionary(d) => self.local_destination(d.get(b"D").map_err(bad)?, depth + 1),
            Object::Name(key) | Object::String(key, _) => {
                let catalog = self.source.catalog().map_err(bad)?;
                if let Some(dests) = optional(self.source, catalog, b"Dests")? {
                    if let Ok(value) = dict(self.source, dests)?.get(&key) {
                        let value = value.clone();
                        return self.local_destination(&value, depth + 1);
                    }
                }
                if let Some(tree) = optional(self.source, catalog, b"Names")? {
                    if let Some(dests) = optional(self.source, dict(self.source, tree)?, b"Dests")?
                    {
                        let mut entries = vec![];
                        names(
                            self.source,
                            dests,
                            depth + 1,
                            &mut HashSet::new(),
                            self.budget,
                            &mut entries,
                        )?;
                        let mut matching = entries.into_iter().filter(|(name, _)| *name == key);
                        if let Some((_, value)) = matching.next() {
                            if matching.next().is_some() {
                                return Err(bad("duplicate named destination"));
                            }
                            return self.local_destination(&value, depth + 1);
                        }
                    }
                }
                Err(bad("unresolved named form destination"))
            }
            _ => Err(bad("invalid local form destination")),
        }
    }
    fn hide_targets(&mut self, value: &Object, depth: usize) -> EngineResult<Vec<Object>> {
        self.budget.take(0, depth)?;
        match value {
            Object::Reference(id) => {
                if let Some(ids) = self.annotations.get(id) {
                    return Ok(ids.iter().copied().map(Object::Reference).collect());
                }
                if matches!(self.map.get(id), Some(Object::Null)) {
                    return Ok(vec![]);
                }
                let value = self
                    .source
                    .objects
                    .get(id)
                    .ok_or_else(|| bad("missing hide target"))?
                    .clone();
                if matches!(value, Object::Dictionary(_)) {
                    return Err(bad(
                        "a Hide action targets an annotation outside retained form widgets",
                    ));
                }
                self.hide_targets(&value, depth + 1)
            }
            Object::String(..) => Ok(vec![self.value(value, depth)?]),
            Object::Array(values) => {
                let mut targets = vec![];
                for value in values {
                    targets.extend(self.hide_targets(value, depth + 1)?);
                }
                Ok(targets)
            }
            Object::Null => Ok(vec![]),
            _ => Err(bad("unsupported Hide target")),
        }
    }
}

pub(super) fn names(
    doc: &Document,
    node: &Object,
    depth: usize,
    seen: &mut HashSet<ObjectId>,
    budget: &mut Budget,
    result: &mut Vec<(Vec<u8>, Object)>,
) -> EngineResult<()> {
    budget.take(0, depth)?;
    if let Object::Reference(id) = node {
        if !seen.insert(*id) {
            return Err(bad("cyclic or repeated name tree node"));
        }
    }
    let node = dict(doc, node)?;
    if let Some(entries) = optional(doc, node, b"Names")? {
        let entries = resolve(doc, entries)?.as_array().map_err(bad)?;
        if entries.len() % 2 != 0 {
            return Err(bad("invalid name tree pairs"));
        }
        for pair in entries.chunks_exact(2) {
            let key = resolve(doc, &pair[0])?.as_str().map_err(bad)?;
            budget.take(key.len(), depth + 1)?;
            result.push((key.to_vec(), pair[1].clone()));
        }
    }
    if let Some(kids) = optional(doc, node, b"Kids")? {
        for kid in resolve(doc, kids)?.as_array().map_err(bad)? {
            names(doc, kid, depth + 1, seen, budget, result)?;
        }
    }
    Ok(())
}

pub(super) fn catalog_dependencies(
    copier: &mut Copier<'_>,
    scripts: &mut HashMap<Vec<u8>, Object>,
    open: &mut Option<Object>,
    additional: &mut Dictionary,
) -> EngineResult<()> {
    let catalog = copier.source.catalog().map_err(bad)?;
    if let Some(tree) = optional(copier.source, catalog, b"Names")? {
        if let Some(js) = optional(copier.source, dict(copier.source, tree)?, b"JavaScript")? {
            let mut entries = vec![];
            names(
                copier.source,
                js,
                0,
                &mut HashSet::new(),
                copier.budget,
                &mut entries,
            )?;
            for (key, value) in entries {
                if scripts.contains_key(&key) {
                    return Err(bad(
                        "combined documents have conflicting document-level script names",
                    ));
                }
                scripts.insert(key, copier.value(&value, 0)?);
            }
        }
    }
    if let Some(action) = optional(copier.source, catalog, b"OpenAction")? {
        let action = copier.value(action, 0)?;
        if open.is_some() {
            return Err(bad("combined form documents have competing open actions"));
        }
        // A direct destination can also target a page omitted by extraction.
        if !matches!(&action,Object::Array(a) if matches!(a.first(),Some(Object::Null)))
            && !matches!(action, Object::Null)
        {
            *open = Some(action);
        }
    }
    if let Some(actions) = optional(copier.source, catalog, b"AA")? {
        for (key, value) in dict(copier.source, actions)?.iter() {
            if additional.has(key) {
                return Err(bad(
                    "combined form documents have competing document actions",
                ));
            }
            additional.set(key.clone(), copier.value(value, 0)?);
        }
    }
    Ok(())
}
