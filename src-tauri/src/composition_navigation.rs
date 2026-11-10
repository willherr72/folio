//! Typed navigation consumers and atomic, source-bound destination reconciliation.
#![allow(dead_code)]
use crate::composition_destinations::{self, Prepared};
use crate::{
    composition_outlines::OutlineInfo, composition_pages::SourcePages, EngineError, EngineResult,
};
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::collections::{BTreeMap, BTreeSet};
pub(crate) struct CatalogValues {
    pub dests: Option<Object>,
    pub names: Option<Object>,
}
pub(crate) fn reconcile(
    document: &mut Document,
    sources: &[SourcePages],
    outlines: &[OutlineInfo],
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<CatalogValues> {
    run(document, sources, outlines, None, &BTreeSet::new(), None, None, check).map(|(catalogs, _)| catalogs)
}
/// Whole-source form combination only. Page deletion/duplication still requires
/// a projection-aware form scope. Title changes are staged before traversal and
/// committed with the successfully reconciled actions; no actions are executed.
pub(crate) fn reconcile_forms(
    document:&mut Document,sources:&[SourcePages],outlines:&[OutlineInfo],
    forms:&[crate::composition_forms::FormInfo],namespace:&crate::composition_form_assembly::Namespace,
    budget:&mut crate::composition_form_budget::Budget,check:&mut impl FnMut()->EngineResult<()>,
)->EngineResult<CatalogValues>{
    require(sources.len()==forms.len())?;let mut scopes=Vec::new();let mut titles=Vec::new();let mut owned_roots=BTreeSet::new();
    for (pages,form) in sources.iter().zip(forms){
        for &id in &form.roots{budget.step(0,check)?;budget.reserve(64)?;require(owned_roots.insert(id))?;}
        budget.reserve(64)?;scopes.push(crate::composition_form_actions::bind(document,pages,form,namespace,budget,check)?);
    }
    for (id,value) in &namespace.root_titles {
        budget.step(0,check)?;require(owned_roots.contains(id))?;
        document.get_dictionary(*id).map_err(bad)?;budget.reserve(64)?;
        titles.push((*id,crate::composition_form_resources::copy_metadata(document,value,budget,check)?));
    }
    let (catalogs,_)=run(document,sources,outlines,None,&BTreeSet::new(),None,Some((scopes,budget)),check)?;
    // All target identities and allocations were proved before run's final
    // checkpoint. No fallible work remains between action and title commits.
    for (id,value) in titles{document.objects.get_mut(&id).unwrap().as_dict_mut().unwrap().set("T",value);}
    Ok(catalogs)
}

pub(crate) fn project(
    document: &mut Document, original: &SourcePages,
    structure: &crate::composition_structure::StructureInfo,
    keep: &BTreeSet<ObjectId>, removed_elements: &BTreeSet<ObjectId>,
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<BTreeSet<ObjectId>> {
    let outline=crate::composition_outlines::inspect(document,original.catalog,structure,check)?;
    let (catalogs, removed)=run(document,std::slice::from_ref(original),&[outline],Some(keep),removed_elements,None,None,check)?;
    let catalog=document.get_dictionary_mut(original.catalog).map_err(bad)?;
    for (key,value) in [(b"Dests".as_slice(),catalogs.dests),(b"Names".as_slice(),catalogs.names)] {
        match value {Some(value)=>catalog.set(key,value),None=>{catalog.remove(key);}}
    }
    Ok(removed)
}
pub(crate) fn project_forms(
    document:&mut Document,original:&SourcePages,structure:&crate::composition_structure::StructureInfo,
    keep:&BTreeSet<ObjectId>,removed_elements:&BTreeSet<ObjectId>,forms:&crate::composition_forms::FormInfo,
    retained:&BTreeSet<ObjectId>,budget:&mut crate::composition_form_budget::Budget,
    check:&mut impl FnMut()->EngineResult<()>,
)->EngineResult<BTreeSet<ObjectId>>{
    let outline=crate::composition_outlines::inspect(document,original.catalog,structure,check)?;
    let namespace=crate::composition_form_assembly::namespace(&[forms],budget,check)?;
    let scope=crate::composition_form_actions::bind(document,original,forms,&namespace,budget,check)?;
    let scope=crate::composition_form_actions::select(scope,retained,keep);budget.reserve(64)?;
    let (catalogs,removed)=run(document,std::slice::from_ref(original),&[outline],Some(keep),removed_elements,None,Some((vec![scope],budget)),check)?;
    let catalog=document.get_dictionary_mut(original.catalog).map_err(bad)?;
    for (key,value) in [(b"Dests".as_slice(),catalogs.dests),(b"Names".as_slice(),catalogs.names)]{
        match value{Some(value)=>catalog.set(key,value),None=>{catalog.remove(key);}}
    }
    Ok(removed)
}
/// Rebind logical field identities while the original merged widgets and
/// action ownership still exist. The caller owns the private normalization graph.
pub(crate) fn rebind_form_fields(
    document:&mut Document,pages:&SourcePages,outlines:&OutlineInfo,
    forms:&crate::composition_forms::FormInfo,ids:&BTreeMap<ObjectId,ObjectId>,
    budget:&mut crate::composition_form_budget::Budget,check:&mut impl FnMut()->EngineResult<()>,
)->EngineResult<()> {
    let namespace=crate::composition_form_assembly::namespace(&[forms],budget,check)?;
    let scope=crate::composition_form_actions::bind(document,pages,forms,&namespace,budget,check)?;
    let scope=crate::composition_form_actions::rebind(scope,ids);budget.reserve(64)?;
    let (catalogs,_)=run(document,std::slice::from_ref(pages),std::slice::from_ref(outlines),None,&BTreeSet::new(),None,Some((vec![scope],budget)),check)?;
    let catalog=document.get_dictionary_mut(pages.catalog).map_err(bad)?;
    for (key,value) in [(b"Dests".as_slice(),catalogs.dests),(b"Names".as_slice(),catalogs.names)]{
        match value{Some(value)=>catalog.set(key,value),None=>{catalog.remove(key);}}
    }
    Ok(())
}
/// Only the listed independent copies are rewritten; original pages/outlines
/// retain their original typed targets. All copies share one traversal budget.
pub(crate) fn retarget_copies(document:&mut Document,pages:&SourcePages,copies:&BTreeMap<ObjectId,ObjectId>,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<()> {
    let (catalogs,_)=run(document,std::slice::from_ref(pages),&[OutlineInfo::default()],None,&BTreeSet::new(),Some(copies),None,check)?;
    let catalog=document.get_dictionary_mut(pages.catalog).map_err(bad)?;
    for (key,value) in [(b"Dests".as_slice(),catalogs.dests),(b"Names".as_slice(),catalogs.names)] {
        match value {Some(value)=>catalog.set(key,value),None=>{catalog.remove(key);}}
    }
    Ok(())
}
/// Copy-local actions use the final shared field hierarchy, while original
/// field actions keep their original page targets until ordinary reconciliation.
pub(crate) fn retarget_copies_forms(document:&mut Document,pages:&SourcePages,copies:&BTreeMap<ObjectId,ObjectId>,forms:&crate::composition_forms::FormInfo,budget:&mut crate::composition_form_budget::Budget,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<()> {
    let namespace=crate::composition_form_assembly::namespace(&[forms],budget,check)?;
    let scope=crate::composition_form_actions::bind(document,pages,forms,&namespace,budget,check)?;budget.reserve(64)?;
    let (catalogs,_)=run(document,std::slice::from_ref(pages),&[OutlineInfo::default()],None,&BTreeSet::new(),Some(copies),Some((vec![scope],budget)),check)?;
    let catalog=document.get_dictionary_mut(pages.catalog).map_err(bad)?;
    for (key,value) in [(b"Dests".as_slice(),catalogs.dests),(b"Names".as_slice(),catalogs.names)]{
        match value{Some(value)=>catalog.set(key,value),None=>{catalog.remove(key);}}
    }
    Ok(())
}
type FormContext<'a>=(Vec<crate::composition_form_actions::Scope<'a>>,&'a mut crate::composition_form_budget::Budget);
fn run(
    document: &mut Document, sources: &[SourcePages], outlines: &[OutlineInfo],
    keep: Option<&BTreeSet<ObjectId>>, removed_elements: &BTreeSet<ObjectId>,
    copies: Option<&BTreeMap<ObjectId,ObjectId>>,
    form_context:Option<FormContext<'_>>,
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<(CatalogValues,BTreeSet<ObjectId>)> {
    require(sources.len() == outlines.len())?;
    let destinations = composition_destinations::prepare(document, sources, check)?;
    let (form_scopes,form_budget)=match form_context{Some((scopes,budget))=>{require(scopes.len()==sources.len())?;(Some(scopes),Some(budget))},None=>(None,None)};
    let renaming = destinations.any_renames() || keep.is_some() || copies.is_some() || form_scopes.is_some();
    let mut walker = Walker {
        document,
        destinations,
        renaming,
        form_scopes,
        form_budget,
        keep,
        retarget: None,
        max_id: document
            .max_id
            .max(document.objects.keys().next_back().map_or(0, |id| id.0)),
        additions: BTreeMap::new(),
        fields: BTreeMap::new(),
        owners: BTreeMap::new(),
        cache: BTreeMap::new(),
        active: BTreeSet::new(),
    };
    let mut removed_outlines=BTreeSet::new();
    for (source, (pages, outline)) in sources.iter().zip(outlines).enumerate() {
        walker.destinations.tick(0, check)?;
        let catalog = walker.document.get_dictionary(pages.catalog).map_err(bad)?;
        for key in [b"OpenAction".as_slice(), b"AA"] {
            require(walker.optional(catalog, key, check)?.is_none())?;
        }
        let dropped=if keep.is_some() {walker.project_outline(outline,source,removed_elements,check)?} else {BTreeSet::new()};
        for id in &outline.items {
            if copies.is_some(){continue;}
            if dropped.contains(id) {continue;}
            let dictionary = walker.document.get_dictionary(*id).map_err(bad)?;
            if walker.owner(*id, source, Owner::Outline, check)? {
                let changes = walker.navigation(dictionary, source, check)?;
                walker.stage_fields(*id, changes, check)?;
            }
        }
        removed_outlines.extend(dropped);
        for page in &pages.pages {
            if let Some(copies)=copies {let Some(old)=copies.get(page) else{continue};walker.retarget=Some((*old,*page));}
            if keep.is_some_and(|keep|!keep.contains(page)) {continue;}
            walker.destinations.tick(0, check)?;
            require(walker.owner(*page, source, Owner::Page, check)?)?;
            let dictionary = walker.document.get_dictionary(*page).map_err(bad)?;
            // PresSteps/NA/PA is a separate, possibly cyclic navigation graph.
            // Preserve it byte-for-byte when keys stay stable; do not leave its
            // local actions bound to obsolete names during namespace changes.
            if walker.renaming && walker.optional(dictionary, b"PresSteps", check)?.is_some() {
                return Err(bad("destination-name conflicts in presentation navigation require an additional preservation adapter"));
            }
            let mut changes = BTreeMap::new();
            if let Some(value) = walker.optional(dictionary, b"AA", check)? {
                if let Some(value) = walker
                    .node(value, source, Kind::PageAA, 0, check)?
                    .replacement
                {
                    changes.insert(b"AA".to_vec(), value);
                }
            }
            if let Some(value) = walker.optional(dictionary, b"Annots", check)? {
                if let Some(value) = walker
                    .node(value, source, Kind::Annotations, 0, check)?
                    .replacement
                {
                    changes.insert(b"Annots".to_vec(), value);
                }
            }
            walker.stage_fields(*page, changes, check)?;
        }
        if let Some(scopes)=&walker.form_scopes {
            // Widgets (including merged fields) were visited in Annots. Visit
            // separate field parents/nonvisual fields once, with field events.
            let forms=scopes[source].forms;
            for (&id,node) in &forms.nodes {
                if copies.is_some() || node.widget_page.is_some() || !walker.form_scopes.as_ref().unwrap()[source].retains_field(id){continue;}
                walker.destinations.tick(0,check)?;require(walker.owner(id,source,Owner::Field,check)?)?;
                let dictionary=walker.document.get_dictionary(id).map_err(bad)?;
                let mut changes=walker.navigation(dictionary,source,check)?;
                if let Some(value)=walker.optional(dictionary,b"AA",check)? {
                    if let Some(value)=walker.node(value,source,Kind::FieldAA,0,check)?.replacement {changes.insert(b"AA".to_vec(),value);}
                }
                walker.stage_fields(id,changes,check)?;
            }
        }
    }
    let (dests, names) = walker.destinations.selected_catalogs(keep, check)?;
    // All updates are staged against canonical dictionaries while the input is
    // exclusively borrowed. No fallible traversal remains after this checkpoint.
    for (id, _) in walker.fields.keys() {
        walker.destinations.tick(0, check)?;
        require(matches!(
            walker.document.objects.get(id),
            Some(Object::Dictionary(_))
        ))?;
    }
    for id in walker.additions.keys() {
        walker.destinations.tick(0, check)?;
        require(!walker.document.objects.contains_key(id))?;
    }
    walker.destinations.tick(0, check)?;
    let Walker {
        fields,
        additions,
        max_id,
        ..
    } = walker;
    let allocated = !additions.is_empty();
    for ((id, key), value) in fields {
        let dictionary=document.objects.get_mut(&id).unwrap().as_dict_mut().unwrap();
        if matches!(value,Object::Null) {dictionary.remove(&key);} else {dictionary.set(key,value);}
    }
    document.objects.extend(additions);
    if allocated {
        document.max_id = max_id;
    }
    Ok((CatalogValues { dests, names },removed_outlines))
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Action,
    Sequence,
    PageAA,
    AnnotationAA,
    FieldAA,
    Annotations,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Owner {
    Page,
    Outline,
    Annotation,
    Field,
}
type Fields = BTreeMap<Vec<u8>, Object>;
type CacheKey = (usize, Option<ObjectId>, ObjectId, Kind);
#[derive(Clone, Copy)]
struct Memo {
    replacement: Option<ObjectId>,
    height: usize,
    removed: bool,
}
struct Walked {
    replacement: Option<Object>,
    height: usize,
}
struct Walker<'a, 'selection, 'forms> {
    document: &'a Document,
    destinations: Prepared<'a>,
    renaming: bool,
    form_scopes:Option<Vec<crate::composition_form_actions::Scope<'forms>>>,
    form_budget:Option<&'forms mut crate::composition_form_budget::Budget>,
    keep: Option<&'selection BTreeSet<ObjectId>>,
    retarget: Option<(ObjectId,ObjectId)>,
    max_id: u32,
    additions: BTreeMap<ObjectId, Object>,
    fields: BTreeMap<(ObjectId, Vec<u8>), Object>,
    owners: BTreeMap<ObjectId, (usize, Owner)>,
    cache: BTreeMap<CacheKey, Memo>,
    active: BTreeSet<CacheKey>,
}
impl<'a> Walker<'a, '_, '_> {
    fn optional(
        &mut self,
        dictionary: &'a Dictionary,
        key: &[u8],
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<Option<&'a Object>> {
        self.destinations.tick(0, check)?;
        match dictionary.get(key) {
            Ok(value) => Ok(
                (!matches!(self.destinations.resolve(value, check)?.1, Object::Null))
                    .then_some(value),
            ),
            Err(_) => Ok(None),
        }
    }
    fn owner(
        &mut self,
        id: ObjectId,
        source: usize,
        kind: Owner,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<bool> {
        self.destinations.tick(0, check)?;
        if let Some(prior) = self.owners.get(&id) {
            require(*prior == (source, kind))?;
            return Ok(false);
        }
        self.destinations.visit(check)?;
        self.destinations.charge(64, check)?;
        self.owners.insert(id, (source, kind));
        Ok(true)
    }
    fn stage_fields(
        &mut self,
        id: ObjectId,
        changes: Fields,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<()> {
        for (key, value) in changes {
            self.destinations.charge(key.len() + 96, check)?;
            if let Some(previous)=self.fields.get(&(id,key.clone())) {
                require(key==b"TM")?;
                self.destinations.charge(previous.as_str().map_err(bad)?.len()+value.as_str().map_err(bad)?.len(),check)?;
                require(previous==&value)?;
            }else{self.fields.insert((id,key),value);}
        }
        Ok(())
    }
    fn navigation(
        &mut self,
        dictionary: &'a Dictionary,
        source: usize,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<Fields> {
        let destination = self.optional(dictionary, b"Dest", check)?;
        let action = self.optional(dictionary, b"A", check)?;
        require(!(destination.is_some() && action.is_some()))?;
        let mut fields = Fields::new();
        if let Some(value) = destination {
            if let Some(value) = self.local(source, value, check)? {
                fields.insert(b"Dest".to_vec(), value);
            }
        }
        if let Some(value) = action {
            if let Some(value) = self
                .node(value, source, Kind::Action, 0, check)?
                .replacement
            {
                fields.insert(b"A".to_vec(), value);
            }
        }
        Ok(fields)
    }
    // Null is an internal removal marker, never published as an action or
    // dictionary field. Memo stores it separately from an unchanged result.
    fn local(&mut self, source:usize, value:&Object, check:&mut impl FnMut()->EngineResult<()>) -> EngineResult<Option<Object>> {
        if let Some(keep)=self.keep {
            if !keep.contains(&self.destinations.target(source,value,check)?) {return Ok(Some(Object::Null));}
        }
        if let Some((old,new))=self.retarget{return self.destinations.for_copy(source,value,old,new,check);}
        self.destinations.local(source,value,check)
    }
    fn annotation(
        &mut self,
        value: &'a Object,
        source: usize,
        depth: usize,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<Option<Object>> {
        self.destinations.tick(depth, check)?;
        let (id, value) = self.destinations.resolve(value, check)?;
        if matches!(value, Object::Null) {
            return Ok(None);
        }
        let dictionary = value.as_dict().map_err(bad)?;
        if let Some(id) = id {
            if !self.owner(id, source, Owner::Annotation, check)? {
                return Ok(None);
            }
        }
        let subtype = self
            .optional(dictionary, b"Subtype", check)?
            .map(|value| {
                self.destinations
                    .resolve(value, check)?
                    .1
                    .as_name()
                    .map_err(bad)
            })
            .transpose()?;
        let bound_widget=subtype==Some(b"Widget") && self.form_scopes.as_ref().is_some_and(|scopes|id.is_some_and(|id|scopes[source].forms.nodes.get(&id).is_some_and(|node|node.widget_page.is_some())));
        let mut fields = if subtype == Some(b"Link") || bound_widget {
            self.navigation(dictionary, source, check)?
        } else {
            if self.renaming {
                require(
                    subtype != Some(b"Widget")
                        && self.optional(dictionary, b"A", check)?.is_none()
                        && self.optional(dictionary, b"Dest", check)?.is_none(),
                )?;
            }
            Fields::new()
        };
        if let Some(value) = self.optional(dictionary, b"AA", check)? {
            if let Some(value) = self
                .node(value, source, if bound_widget{Kind::FieldAA}else{Kind::AnnotationAA}, depth + 1, check)?
                .replacement
            {
                fields.insert(b"AA".to_vec(), value);
            }
        }
        if let Some(id) = id {
            self.stage_fields(id, fields, check)?;
            Ok(None)
        } else if fields.is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.dictionary(dictionary, fields, check)?))
        }
    }
    fn dictionary(
        &mut self,
        dictionary: &Dictionary,
        mut changes: Fields,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<Object> {
        self.destinations.charge(64, check)?;
        let mut output = Dictionary::new();
        for (key, value) in dictionary.iter() {
            self.destinations.charge(key.len() + 64, check)?;
            let value = if let Some(value) = changes.remove(key) {
                if matches!(value,Object::Null) {continue;}
                value
            } else {
                self.destinations.copy(value, check)?
            };
            output.set(key.clone(), value);
        }
        for (key,value) in changes {
            self.destinations.charge(key.len()+64,check)?;
            if !matches!(value,Object::Null){output.set(key,value);}
        }
        Ok(Object::Dictionary(output))
    }
    fn boolean(
        &mut self,
        dictionary: &'a Dictionary,
        key: &[u8],
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<()> {
        if let Some(value) = self.optional(dictionary, key, check)? {
            self.destinations
                .resolve(value, check)?
                .1
                .as_bool()
                .map_err(bad)?;
        }
        Ok(())
    }
    fn node(
        &mut self,
        value: &'a Object,
        source: usize,
        kind: Kind,
        depth: usize,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<Walked> {
        self.destinations.tick(depth, check)?;
        let (id, value) = self.destinations.resolve(value, check)?;
        let key = id.map(|id| (source, self.retarget.map(|(_,new)|new), id, kind));
        if let Some(key) = key {
            require(!self.active.contains(&key))?;
            if let Some(memo) = self.cache.get(&key) {
                require(depth.checked_add(memo.height).is_some_and(|n| n <= 32))?;
                return Ok(Walked {
                    replacement: if memo.removed {Some(Object::Null)} else {memo.replacement.map(Object::Reference)},
                    height: memo.height,
                });
            }
            self.destinations.charge(128, check)?;
            self.active.insert(key);
        }
        self.destinations.visit(check)?;
        let mut height = 1usize;
        let mut removed = false;
        let replacement = match kind {
            Kind::Action => {
                let dictionary = value.as_dict().map_err(bad)?;
                if let Some(value) = self.optional(dictionary, b"Type", check)? {
                    require(
                        self.destinations
                            .resolve(value, check)?
                            .1
                            .as_name()
                            .map_err(bad)?
                            == b"Action",
                    )?;
                }
                let action = self
                    .destinations
                    .resolve(dictionary.get(b"S").map_err(bad)?, check)?
                    .1
                    .as_name()
                    .map_err(bad)?;
                let mut changes = Fields::new();
                match action {
                    b"GoTo" => {
                        if let Some(value) = self.local(
                            source,
                            dictionary.get(b"D").map_err(bad)?,
                            check,
                        )? {
                            removed=matches!(value,Object::Null);
                            changes.insert(b"D".to_vec(), value);
                        }
                    }
                    b"GoToR" => {
                        require(matches!(
                            self.destinations
                                .resolve(dictionary.get(b"F").map_err(bad)?, check)?
                                .1,
                            Object::String(..) | Object::Dictionary(_)
                        ))?;
                        self.destinations
                            .remote(dictionary.get(b"D").map_err(bad)?, check)?;
                        self.boolean(dictionary, b"NewWindow", check)?;
                    }
                    b"URI" => {
                        self.destinations
                            .resolve(dictionary.get(b"URI").map_err(bad)?, check)?
                            .1
                            .as_str()
                            .map_err(bad)?;
                        self.boolean(dictionary, b"IsMap", check)?;
                    }
                    b"Named" => {
                        let name = self
                            .destinations
                            .resolve(dictionary.get(b"N").map_err(bad)?, check)?
                            .1
                            .as_name()
                            .map_err(bad)?;
                        require(
                            !self.renaming
                                || [
                                    b"NextPage".as_slice(),
                                    b"PrevPage",
                                    b"FirstPage",
                                    b"LastPage",
                                ]
                                .contains(&name),
                        )?;
                    }
                    b"JavaScript" => {
                        require(!self.renaming)?;
                        require(matches!(
                            self.destinations
                                .resolve(dictionary.get(b"JS").map_err(bad)?, check)?
                                .1,
                            Object::String(..) | Object::Stream(_)
                        ))?;
                    }
                    b"Hide"|b"ResetForm"|b"SubmitForm"=>{
                        if let Some(scopes)=&self.form_scopes {
                            let rewritten=crate::composition_form_actions::rewrite(self.document,&scopes[source],dictionary,self.form_budget.as_deref_mut().unwrap(),check)?;
                            changes.extend(rewritten.action);
                            for ((id,key),value) in rewritten.fields {self.stage_fields(id,[(key,value)].into(),check)?;}
                        }else{require(!self.renaming)?;}
                    }
                    _ => require(!self.renaming)?,
                }
                if let Some(next) = self.optional(dictionary, b"Next", check)? {
                    let next_kind =
                        if matches!(self.destinations.resolve(next, check)?.1, Object::Array(_)) {
                            Kind::Sequence
                        } else {
                            Kind::Action
                        };
                    let next = self.node(next, source, next_kind, depth + 1, check)?;
                    height = height.max(1 + next.height);
                    if let Some(value) = next.replacement {
                        changes.insert(b"Next".to_vec(), value);
                    }
                }
                if removed {Some(Object::Null)} else if changes.is_empty() {
                    None
                } else {
                    Some(self.dictionary(dictionary, changes, check)?)
                }
            }
            Kind::PageAA | Kind::AnnotationAA | Kind::FieldAA => {
                let dictionary = value.as_dict().map_err(bad)?;
                let mut changes = Fields::new();
                for (trigger, value) in dictionary.iter() {
                    self.destinations.tick(depth, check)?;
                    if matches!(self.destinations.resolve(value, check)?.1, Object::Null) {
                        continue;
                    }
                    let known = match kind {
                        Kind::PageAA => [b"O".as_slice(), b"C"].contains(&trigger.as_slice()),
                        Kind::FieldAA=>[b"K".as_slice(),b"F",b"V",b"C",b"E",b"X",b"D",b"U",b"Fo",b"Bl",b"PO",b"PC",b"PV",b"PI"].contains(&trigger.as_slice()),
                        _ => [
                            b"E".as_slice(),
                            b"X",
                            b"D",
                            b"U",
                            b"PO",
                            b"PC",
                            b"PV",
                            b"PI",
                        ]
                        .contains(&trigger.as_slice()),
                    };
                    if !known {
                        require(!self.renaming)?;
                        continue;
                    }
                    let action = self.node(value, source, Kind::Action, depth + 1, check)?;
                    height = height.max(1 + action.height);
                    if let Some(value) = action.replacement {
                        changes.insert(trigger.clone(), value);
                    }
                }
                if changes.is_empty() {
                    None
                } else {
                    Some(self.dictionary(dictionary, changes, check)?)
                }
            }
            Kind::Sequence | Kind::Annotations => {
                let values = value.as_array().map_err(bad)?;
                require(values.len() <= 100_000)?;
                let mut changes = BTreeMap::new();
                for (index, value) in values.iter().enumerate() {
                    self.destinations.tick(depth, check)?;
                    let replacement = if kind == Kind::Annotations {
                        self.annotation(value, source, depth + 1, check)?
                    } else {
                        let action = self.node(value, source, Kind::Action, depth + 1, check)?;
                        height = height.max(1 + action.height);
                        action.replacement
                    };
                    if let Some(value) = replacement {
                        self.destinations.charge(64, check)?;
                        changes.insert(index, value);
                    }
                }
                if changes.is_empty() {
                    None
                } else {
                    self.destinations.charge(64, check)?;
                    let mut output = Vec::new();
                    for (index, value) in values.iter().enumerate() {
                        self.destinations.charge(64, check)?;
                        output.push(if let Some(value) = changes.remove(&index) {
                            if matches!(value,Object::Null) {continue;}
                            value
                        } else {
                            self.destinations.copy(value, check)?
                        });
                    }
                    if output.is_empty() && kind==Kind::Sequence {removed=true;Some(Object::Null)} else {Some(Object::Array(output))}
                }
            }
        };
        require(depth.checked_add(height).is_some_and(|n| n <= 32))?;
        let replacement = if let Some(key) = key {
            self.active.remove(&key);
            let changed = if removed {None} else if let Some(value) = replacement {
                self.destinations.charge(96, check)?;
                self.max_id = self.max_id.checked_add(1).ok_or_else(invalid)?;
                let id = (self.max_id, 0);
                self.additions.insert(id, value);
                Some(id)
            } else {
                None
            };
            self.cache.insert(
                key,
                Memo {
                    replacement: changed,
                    height,
                    removed,
                },
            );
            if removed {Some(Object::Null)} else {changed.map(Object::Reference)}
        } else {
            replacement
        };
        Ok(Walked {
            replacement,
            height,
        })
    }
}
fn bad(error: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("PDF navigation: {error}"))
}
fn invalid() -> EngineError {
    bad("unsupported or inconsistent action context")
}
fn require(condition: bool) -> EngineResult<()> {
    if condition {
        Ok(())
    } else {
        Err(invalid())
    }
}
#[cfg(test)]
#[path = "composition_navigation_tests.rs"]
mod tests;

#[path = "composition_outline_projection.rs"]
mod outline_projection;
