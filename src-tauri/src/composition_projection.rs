//! Owned, graph-preserving page selection after catalog composition.
use crate::{composition_structure::{Parents, StructureInfo}, EngineError, EngineResult};
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct StructureSelection {
    pub fields:BTreeMap<(ObjectId,Vec<u8>),Option<Object>>,
    pub elements:BTreeSet<ObjectId>,
    pub marked:BTreeSet<(ObjectId,i64)>,
    pub whole:BTreeSet<ObjectId>,
    pub records:BTreeSet<ObjectId>,
}
pub(crate) struct SelectionBudget {steps:usize,remaining:usize}
impl Default for SelectionBudget {fn default()->Self{Self{steps:0,remaining:16*1024*1024}}}
/// Original page and structure identities must come from the validated source.
/// Excluding an ID from a new copy does not delete it from the original graph.
pub(crate) fn structure_for_copy(
    document:&Document,structure:&StructureInfo,page:ObjectId,budget:&mut SelectionBudget,
    check:&mut impl FnMut()->EngineResult<()>,
)->EngineResult<StructureSelection> {
    let mut work=Work{check,steps:budget.steps,remaining:budget.remaining};
    let keep=BTreeSet::from([page]);
    let result=select_structure(document,structure,&keep,true,&mut work)?;
    budget.steps=work.steps;budget.remaining=work.remaining;
    Ok(result)
}
fn select_structure(
    doc:&Document,structure:&StructureInfo,keep:&BTreeSet<ObjectId>,cloning:bool,work:&mut Work<'_>,
)->EngineResult<StructureSelection> {
    select_structure_excluding(doc,structure,keep,cloning,&BTreeSet::new(),work).map(|(selection,_)|selection)
}
/// Annotation-only filtering keeps every page and marked-content owner. The
/// caller proves incoming consumers and applies the returned fields atomically.
pub(crate) fn structure_without_annotations(
    doc:&Document,structure:&StructureInfo,pages:&[ObjectId],excluded:&BTreeSet<ObjectId>,
    metadata:&mut usize,steps_remaining:&mut usize,check:&mut impl FnMut()->EngineResult<()>,
)->EngineResult<(StructureSelection,BTreeSet<ObjectId>)> {
    let mut work=Work{check,steps:1_000_000usize.checked_sub(*steps_remaining).ok_or_else(||bad("invalid remaining steps"))?,remaining:*metadata};
    work.charge(pages.len().checked_mul(64).ok_or_else(||bad("page budget overflow"))?)?;
    work.charge(structure.elements.len().checked_mul(192).ok_or_else(||bad("element budget overflow"))?)?;
    work.charge(structure.owners.len().checked_mul(160).ok_or_else(||bad("owner budget overflow"))?)?;
    let keep=pages.iter().copied().collect();
    let whole=structure.owners.values().filter_map(|owner|matches!(owner.parents,Parents::Whole(_)).then_some(owner.object)).collect::<BTreeSet<_>>();
    require(!excluded.is_empty() && excluded.is_subset(&whole),"removed annotation has no validated whole owner")?;
    // Rebuilding the number tree may change its standard representation, but
    // must not silently discard producer metadata on an active tree node.
    fn plain_tree(doc:&Document,value:&Object,work:&mut Work<'_>,depth:usize)->EngineResult<()> {
        work.step(depth)?;
        let dict=resolve(doc,value,work)?.1.as_dict().map_err(bad)?;
        for (key,_) in dict.iter(){work.step(depth)?;require([b"Nums".as_slice(),b"Kids",b"Limits"].contains(&key.as_slice()),"parent-tree extension metadata needs a preservation adapter")?;}
        if let Some(kids)=optional(doc,dict,b"Kids",work)? {
            for kid in resolve(doc,kids,work)?.1.as_array().map_err(bad)? {plain_tree(doc,kid,work,depth+1)?;}
        }
        Ok(())
    }
    if let Some(root)=structure.root {
        if let Some(tree)=optional(doc,doc.get_dictionary(root).map_err(bad)?,b"ParentTree",&mut work)? {plain_tree(doc,tree,&mut work,0)?;}
    }
    let result=select_structure_excluding(doc,structure,&keep,false,excluded,&mut work);
    *metadata=work.remaining;*steps_remaining=1_000_000usize.saturating_sub(work.steps);
    result
}
/// Check after typed patches, before any publication. The full object-count
/// reservation bounds the existing reachability set/queue without payload copies.
pub(crate) fn validate_removed_owners(
    doc:&Document,removed:&BTreeSet<ObjectId>,metadata:&mut usize,steps_remaining:&mut usize,
    check:&mut impl FnMut()->EngineResult<()>,
)->EngineResult<()> {
    let mut work=Work{check,steps:1_000_000usize.checked_sub(*steps_remaining).ok_or_else(||bad("invalid remaining steps"))?,remaining:*metadata};
    work.charge(doc.objects.len().checked_mul(128).ok_or_else(||bad("reachability budget overflow"))?)?;
    let result=reachable(doc,removed,&mut work).map(|_|());
    *metadata=work.remaining;*steps_remaining=1_000_000usize.saturating_sub(work.steps);
    result
}
fn select_structure_excluding(
    doc:&Document,structure:&StructureInfo,keep:&BTreeSet<ObjectId>,cloning:bool,
    excluded:&BTreeSet<ObjectId>,work:&mut Work<'_>,
)->EngineResult<(StructureSelection,BTreeSet<ObjectId>)> {
    work.step(0)?;let mut fields=BTreeMap::new();
    let mut filter=Filter{doc,keep,work,fields:&mut fields,cloning,excluded,removed_records:BTreeSet::new(),
        elements:BTreeSet::new(),marked:BTreeSet::new(),whole:BTreeSet::new(),records:BTreeSet::new()};
    if let Some(root)=structure.root {
        if let Some(kids)=optional(doc,doc.get_dictionary(root).map_err(bad)?,b"K",filter.work)? {
            let (kids,_)=filter.kids(kids,None,0)?;
            filter.fields.insert((root,b"K".to_vec()),kids);
        }
        filter.parents(root,structure)?;
    }
    let Filter{elements,marked,whole,records,removed_records,..}=filter;
    Ok((StructureSelection{fields,elements,marked,whole,records},removed_records))
}

pub(crate) fn project(
    document: Document,
    pages: &[ObjectId],
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<Document> {
    project_inner(document,pages,false,false,&mut crate::composition_form_budget::Budget::default(),check)
}
/// Internal form-aware selection; public occurrence routing is connected after
/// repeated-widget ownership is implemented and verified.
pub(crate) fn project_forms(document:Document,pages:&[ObjectId],check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Document>{project_inner(document,pages,false,true,&mut crate::composition_form_budget::Budget::default(),check)}
/// Copying requires independent inherited geometry even when selection itself
/// leaves every original page in place.
pub(crate) fn prepare_occurrences(document:Document,pages:&[ObjectId],check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Document> {
    project_inner(document,pages,true,false,&mut crate::composition_form_budget::Budget::default(),check)
}
pub(crate) fn prepare_occurrences_forms(document:Document,pages:&[ObjectId],budget:&mut crate::composition_form_budget::Budget,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Document>{
    project_inner(document,pages,true,true,budget,check)
}
fn project_inner(mut document:Document,pages:&[ObjectId],materialize:bool,allow_forms:bool,form_budget:&mut crate::composition_form_budget::Budget,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Document> {
    check()?;
    let catalog=document.trailer.get(b"Root").and_then(Object::as_reference).map_err(bad)?;
    let original=crate::composition_pages::inspect(&document,catalog,check)?;
    let structure=crate::composition_structure::inspect(&document,catalog,&original.pages,check)?;
    require(!pages.is_empty() && pages.len()<=original.pages.len(),"empty or oversized selection")?;
    let available=original.pages.iter().copied().collect::<BTreeSet<_>>();
    let mut keep=BTreeSet::new();
    for page in pages {check()?;require(available.contains(page) && keep.insert(*page),"unknown or repeated page")?;}
    if !materialize && pages==original.pages {return Ok(document);}
    let form_state=if allow_forms{
        let forms=crate::composition_forms::inspect(&document,&original,form_budget,check)?;
        if forms.present{let selected=crate::composition_form_selection::prepare(&document,&original,&forms,&keep,form_budget,check)?;Some((forms,selected))}else{None}
    }else{None};
    let mut work=Work{check,steps:0,remaining:16*1024*1024};
    validate_branches(&document,pages,&mut work)?;
    let mut fields=BTreeMap::new();
    // Inherited attributes belong to the old page tree. Materialize them while
    // every original ancestor is still available, without copying any streams.
    for page in pages {
        for key in [b"Resources".as_slice(),b"MediaBox",b"CropBox",b"Rotate"] {
            let value=inherited(&document,*page,key,&mut work)?;
            require(value.is_some() || ![b"Resources".as_slice(),b"MediaBox"].contains(&key),"missing required inherited page field")?;
            if let Some(value)=value {fields.insert((*page,key.to_vec()),Some(work.copy(value,0)?));}
        }
    }
    let mut removed=available.difference(&keep).copied().collect::<BTreeSet<_>>();
    if !removed.is_empty() {
        let selected=select_structure(&document,&structure,&keep,false,&mut work)?;
        removed.extend(structure.elements.difference(&selected.elements).copied());
        fields.extend(selected.fields);
    }
    if !removed.is_empty() {
        let dropped_outlines=if let Some((forms,selected))=&form_state{
            crate::composition_navigation::project_forms(&mut document,&original,&structure,&keep,&removed,forms,&selected.retained,form_budget,&mut ||(work.check)())?
        }else{crate::composition_navigation::project(&mut document,&original,&structure,&keep,&removed,&mut ||(work.check)())?};
        removed.extend(dropped_outlines);
    }
    let expected_forms=if let Some((_,mut selected))=form_state{
        work.charge(selected.removed.len().checked_mul(64).ok_or_else(||bad("removed form node overflow"))?)?;
        removed.append(&mut selected.removed);
        let expected=std::mem::take(&mut selected.retained);
        document=crate::composition_form_selection::apply(document,selected,form_budget,&mut ||(work.check)())?;
        Some(expected)
    }else{None};
    let new_root=(document.max_id.max(document.objects.keys().next_back().map_or(0,|id|id.0))
        .checked_add(1).ok_or_else(||bad("object ID overflow"))?,0);
    work.charge(pages.len().checked_mul(32).ok_or_else(||bad("page budget overflow"))?)?;
    work.step(0)?;
    for ((id,key),value) in fields {
        let target=document.get_dictionary_mut(id).map_err(bad)?;
        match value {Some(value)=>target.set(key,value),None=>{target.remove(&key);}}
    }
    for page in pages {work.step(0)?;document.get_dictionary_mut(*page).map_err(bad)?.set("Parent",new_root);}
    document.objects.insert(new_root,dictionary!{"Type"=>"Pages","Count"=>pages.len() as i64,
        "Kids"=>pages.iter().copied().map(Object::Reference).collect::<Vec<_>>()}.into());
    document.max_id=new_root.0;
    document.get_dictionary_mut(original.catalog).map_err(bad)?.set("Pages",new_root);
    let selected_structure=crate::composition_structure::inspect(&document,original.catalog,pages,&mut ||(work.check)())?;
    let actual=crate::composition_pages::inspect(&document,original.catalog,&mut ||(work.check)())?;
    require(actual.pages==pages,"projected page order changed")?;
    let selected_outline=crate::composition_outlines::inspect(&document,original.catalog,&selected_structure,&mut ||(work.check)())?;
    // Validate every surviving local consumer against the selected catalog.
    if let Some(expected)=expected_forms{
        form_budget.nodes=0;
        let forms=crate::composition_forms::inspect(&document,&actual,form_budget,&mut ||(work.check)())?;
        require(forms.nodes.len()==expected.len() && forms.nodes.keys().all(|id|expected.contains(id)),"projected form ownership changed")?;
        let namespace=crate::composition_form_assembly::namespace(&[&forms],form_budget,&mut ||(work.check)())?;
        crate::composition_navigation::reconcile_forms(&mut document,&[actual],&[selected_outline],&[forms],&namespace,form_budget,&mut ||(work.check)())?;
    }else{crate::composition_navigation::reconcile(&mut document,&[actual],&[selected_outline],&mut ||(work.check)())?;}
    // Final navigation can allocate new shared action dictionaries. Compute
    // reachability only after that pass, while still rejecting every retained
    // typed or opaque edge to a removed page, tag, widget or field.
    let live=reachable(&document,&removed,&mut work)?;
    // Only unreachable objects are removed, after all ownership checks. The
    // caller owns this scratch document; refusals cannot modify an open source.
    work.step(0)?;
    document.objects.retain(|id,_|live.contains(id));
    crate::composition_graph::validate_retained(&document,&mut ||(work.check)())?;
    Ok(document)
}

fn bad(message:impl std::fmt::Display)->EngineError {
    EngineError::InvalidRequest(format!("PDF page selection: {message}"))
}
/// Final scratch-document cleanup after all typed mutations have completed.
/// This does not authorize edits or remove resources with any surviving edge.
pub(crate) fn prune_final(document:Document,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Document> {
    prune_final_with_budget(document,16*1024*1024,1_000_000,check)
}
fn prune_final_with_budget(mut document:Document,metadata:usize,steps:usize,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Document> {
    let mut work=Work{check,steps:1_000_000usize.checked_sub(steps).ok_or_else(||bad("invalid final traversal allowance"))?,remaining:metadata};
    work.step(0)?;
    // Reserve the live set, traversal queue and removal IDs before allocating.
    // Stream payloads and metadata are borrowed, never cloned or decoded here.
    work.charge(document.objects.len().checked_mul(128).ok_or_else(||bad("final reachability budget overflow"))?)?;
    crate::composition_graph::validate_retained(&document,&mut ||work.step(0))?;
    let live=reachable(&document,&BTreeSet::new(),&mut work)?;
    let mut obsolete=Vec::new();
    for id in document.objects.keys(){work.step(0)?;if !live.contains(id){obsolete.push(*id);}}
    for id in obsolete {work.step(0)?;document.objects.remove(&id);}
    // Keep existing identities, trailer and high-water ID unchanged. A caller
    // may still hold exact occurrence/annotation receipts for surviving objects.
    crate::composition_graph::validate_retained(&document,&mut ||work.step(0))?;
    work.step(0)?;Ok(document)
}
fn require(condition:bool,message:&str)->EngineResult<()> {if condition{Ok(())}else{Err(bad(message))}}
struct Work<'a>{check:&'a mut dyn FnMut()->EngineResult<()>,steps:usize,remaining:usize}
impl Work<'_>{
    fn step(&mut self,depth:usize)->EngineResult<()> {
        (self.check)()?;self.steps+=1;require(depth<=64 && self.steps<=1_000_000,"traversal limit")
    }
    fn charge(&mut self,bytes:usize)->EngineResult<()> {
        self.step(0)?;self.remaining=self.remaining.checked_sub(bytes).ok_or_else(||bad("metadata budget exceeded"))?;Ok(())
    }
    fn copy(&mut self,value:&Object,depth:usize)->EngineResult<Object> {
        fn measure(work:&mut Work<'_>,value:&Object,depth:usize)->EngineResult<()> {
            work.step(depth)?;work.charge(32)?;
            match value {
                Object::Name(bytes)|Object::String(bytes,_)=>work.charge(bytes.len())?,
                Object::Array(values)=>for v in values{measure(work,v,depth+1)?;},
                Object::Dictionary(dict)=>for (key,v) in dict.iter(){work.charge(key.len())?;measure(work,v,depth+1)?;},
                Object::Stream(_)=>return Err(bad("direct stream in staged metadata")),
                _=>{},
            }Ok(())
        }
        measure(self,value,depth)?;Ok(value.clone())
    }
}
fn resolve<'a>(doc:&'a Document,mut value:&'a Object,work:&mut Work<'_>)->EngineResult<(Option<ObjectId>,&'a Object)> {
    let mut seen=BTreeSet::new();let mut canonical=None;
    while let Object::Reference(id)=value {
        work.step(seen.len())?;require(seen.insert(*id),"cyclic alias")?;
        canonical=Some(*id);value=doc.objects.get(id).ok_or_else(||bad("missing reference"))?;
    }
    Ok((canonical,value))
}
fn reference(doc:&Document,value:&Object,work:&mut Work<'_>)->EngineResult<ObjectId> {
    resolve(doc,value,work)?.0.ok_or_else(||bad("expected indirect identity"))
}
fn optional<'a>(doc:&'a Document,dict:&'a Dictionary,key:&[u8],work:&mut Work<'_>)->EngineResult<Option<&'a Object>> {
    match dict.get(key) {
        Ok(value) if !matches!(resolve(doc,value,work)?.1,Object::Null)=>Ok(Some(value)),
        _=>Ok(None),
    }
}
fn inherited<'a>(doc:&'a Document,mut id:ObjectId,key:&[u8],work:&mut Work<'_>)->EngineResult<Option<&'a Object>> {
    let mut seen=BTreeSet::new();
    loop {
        work.step(seen.len())?;require(seen.insert(id),"cyclic page ancestry")?;
        let dict=doc.get_dictionary(id).map_err(bad)?;
        if let Some(value)=optional(doc,dict,key,work)? {return Ok(Some(value));}
        let Some(parent)=optional(doc,dict,b"Parent",work)? else{return Ok(None)};
        id=reference(doc,parent,work)?;
    }
}
fn validate_branches(doc:&Document,pages:&[ObjectId],work:&mut Work<'_>)->EngineResult<()> {
    let mut seen=BTreeSet::new();
    for page in pages {
        let mut id=*page;
        loop {
            work.step(0)?;
            let dict=doc.get_dictionary(id).map_err(bad)?;
            let Some(parent)=optional(doc,dict,b"Parent",work)? else{break};
            id=reference(doc,parent,work)?;
            if !seen.insert(id){break;}
            for (key,value) in doc.get_dictionary(id).map_err(bad)?.iter() {
                work.step(0)?;
                require([b"Type".as_slice(),b"Parent",b"Kids",b"Count",b"Resources",b"MediaBox",b"CropBox",b"Rotate"].contains(&key.as_slice())
                    || matches!(resolve(doc,value,work)?.1,Object::Null),"unsupported page branch metadata would be discarded")?;
            }
        }
    }Ok(())
}

type Fields=BTreeMap<(ObjectId,Vec<u8>),Option<Object>>;
struct Filter<'a,'b,'c> {
    doc:&'a Document,keep:&'a BTreeSet<ObjectId>,work:&'b mut Work<'c>,fields:&'b mut Fields,
    elements:BTreeSet<ObjectId>,marked:BTreeSet<(ObjectId,i64)>,whole:BTreeSet<ObjectId>,
    records:BTreeSet<ObjectId>,cloning:bool,
    excluded:&'a BTreeSet<ObjectId>,removed_records:BTreeSet<ObjectId>,
}
impl Filter<'_,'_,'_> {
    // The boolean records any lost descendants, even if the direct K reference
    // remains unchanged. Alternate text on an ancestor covers those descendants.
    fn kids(&mut self,value:&Object,page:Option<ObjectId>,depth:usize)->EngineResult<(Option<Object>,bool)> {
        self.work.step(depth)?;
        let (_,resolved)=resolve(self.doc,value,self.work)?;
        if let Object::Array(values)=resolved {
            let mut kept=Vec::new();let mut lost=false;
            for child in values {
                let (next,changed)=self.child(child,page,depth+1)?;
                lost|=changed;
                if let Some(next)=next {self.work.charge(32)?;kept.push(next);}else{lost=true;}
            }
            if !lost {return Ok((Some(self.work.copy(value,depth)?),false));}
            Ok(((!kept.is_empty()).then_some(Object::Array(kept)),true))
        } else {self.child(value,page,depth+1)}
    }
    fn child(&mut self,value:&Object,mut page:Option<ObjectId>,depth:usize)->EngineResult<(Option<Object>,bool)> {
        self.work.step(depth)?;
        let (id,resolved)=resolve(self.doc,value,self.work)?;
        if let Object::Integer(mcid)=resolved {
            let page=page.ok_or_else(||bad("missing marked-content page"))?;
            if !self.keep.contains(&page){return Ok((None,true));}
            self.marked.insert((page,*mcid));return Ok((Some(self.work.copy(value,depth)?),false));
        }
        let dict=resolved.as_dict().map_err(bad)?;
        if let Some(pg)=optional(self.doc,dict,b"Pg",self.work)? {page=Some(reference(self.doc,pg,self.work)?);}
        let kind=optional(self.doc,dict,b"Type",self.work)?.map(|v|resolve(self.doc,v,self.work)?.1.as_name().map_err(bad)).transpose()?;
        match kind {
            Some(b"MCR")|Some(b"OBJR")=>{
                let page=page.ok_or_else(||bad("missing content page"))?;
                if !self.keep.contains(&page){return Ok((None,true));}
                if kind==Some(b"OBJR".as_slice()) && self.excluded.contains(&reference(self.doc,dict.get(b"Obj").map_err(bad)?,self.work)?) {
                    if let Some(id)=id {self.work.charge(64)?;self.removed_records.insert(id);}
                    return Ok((None,true));
                }
                if let Some(id)=id {self.work.charge(32)?;self.records.insert(id);}
                if kind==Some(b"MCR".as_slice()) {
                    let owner=optional(self.doc,dict,b"Stm",self.work)?.map(|v|reference(self.doc,v,self.work)).transpose()?.unwrap_or(page);
                    let mcid=resolve(self.doc,dict.get(b"MCID").map_err(bad)?,self.work)?.1.as_i64().map_err(bad)?;
                    self.marked.insert((owner,mcid));
                } else {self.whole.insert(reference(self.doc,dict.get(b"Obj").map_err(bad)?,self.work)?);}
                Ok((Some(self.work.copy(value,depth)?),false))
            }
            None|Some(b"StructElem")=>{
                let id=id.ok_or_else(||bad("direct structure element"))?;
                let old=optional(self.doc,dict,b"K",self.work)?;
                let empty=match old {None=>true,Some(value)=>matches!(resolve(self.doc,value,self.work)?.1,Object::Array(values) if values.is_empty())};
                require(!self.cloning || !empty || page.is_some(),"unbound empty structure needs an occurrence ownership adapter")?;
                let (next,lost)=if let Some(old)=old{self.kids(old,page,depth+1)?}else{(None,false)};
                if (lost && next.is_none()) || (empty && page.is_some_and(|p|!self.keep.contains(&p))) {
                    // IDs can be addressed by strings (for example table Headers),
                    // so the final indirect-reference scan alone cannot prove loss.
                    require(self.cloning || optional(self.doc,dict,b"ID",self.work)?.is_none(),"removed structure identifier needs a relationship adapter")?;
                    return Ok((None,true));
                }
                if lost {
                    for key in [b"Alt".as_slice(),b"ActualText",b"E"] {
                        require(optional(self.doc,dict,key,self.work)?.is_none(),"partial selection changes alternate or replacement text")?;
                    }
                    for key in [b"A".as_slice(),b"C"] {
                        require(optional(self.doc,dict,key,self.work)?.is_none(),"partial selection needs a structure attribute adapter")?;
                    }
                    self.fields.insert((id,b"K".to_vec()),next);
                }
                if page.is_some_and(|p|!self.keep.contains(&p)) {self.fields.insert((id,b"Pg".to_vec()),None);}
                self.elements.insert(id);
                Ok((Some(self.work.copy(value,depth)?),lost))
            }
            _=>Err(bad("unsupported structure child")),
        }
    }
    fn parents(&mut self,root:ObjectId,original:&StructureInfo)->EngineResult<()> {
        let mut nums=Vec::new();
        for (key,owner) in &original.owners {
            self.work.step(0)?;
            let entry=match &owner.parents {
                Parents::Whole(parent)=>self.whole.contains(&owner.object).then_some(Object::Reference(*parent)),
                Parents::Marked(slots)=>{
                    let mut any=false;
                    self.work.charge(slots.len().checked_mul(32).ok_or_else(||bad("slot budget overflow"))?)?;
                    let slots=slots.iter().enumerate().map(|(i,parent)|{
                        if self.marked.contains(&(owner.object,i as i64)) {any=true;parent.map_or(Object::Null,Object::Reference)}else{Object::Null}
                    }).collect::<Vec<_>>();
                    // Annotation-only filtering never removes marked owners,
                    // including empty/null-only arrays on retained form XObjects.
                    (any || self.keep.contains(&owner.object) || !self.excluded.is_empty()).then_some(Object::Array(slots))
                },
            };
            if let Some(entry)=entry {self.work.charge(64)?;nums.extend([Object::Integer(*key),entry]);}
        }
        self.fields.insert((root,b"ParentTree".to_vec()),Some(dictionary!{"Nums"=>nums}.into()));
        Ok(())
    }
}

fn reachable(doc:&Document,removed:&BTreeSet<ObjectId>,work:&mut Work<'_>)->EngineResult<BTreeSet<ObjectId>> {
    fn collect(value:&Object,seen:&mut BTreeSet<ObjectId>,pending:&mut Vec<ObjectId>,removed:&BTreeSet<ObjectId>,work:&mut Work<'_>,depth:usize)->EngineResult<()> {
        work.step(depth)?;
        match value {
            Object::Reference(id)=>{
                require(!removed.contains(id),"retained navigation or metadata refers to a removed page or tag")?;
                if seen.insert(*id) {require(seen.len()<=100_000,"reachable object limit")?;pending.push(*id);}
            },
            Object::Array(values)=>for v in values {collect(v,seen,pending,removed,work,depth+1)?;},
            Object::Dictionary(dict)=>for (_,v) in dict.iter(){collect(v,seen,pending,removed,work,depth+1)?;},
            Object::Stream(stream)=>for (_,v) in stream.dict.iter(){collect(v,seen,pending,removed,work,depth+1)?;},
            _=>{},
        }Ok(())
    }
    let mut seen=BTreeSet::new();let mut pending=Vec::new();
    for (_,value) in doc.trailer.iter(){collect(value,&mut seen,&mut pending,removed,work,0)?;}
    while let Some(id)=pending.pop(){work.step(0)?;collect(doc.objects.get(&id).ok_or_else(||bad("missing reachable object"))?,&mut seen,&mut pending,removed,work,0)?;}
    Ok(seen)
}

#[cfg(test)]
#[path = "composition_projection_tests.rs"]
mod tests;
