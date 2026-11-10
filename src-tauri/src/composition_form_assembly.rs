//! Staged form assembly. Names are prepared before any document mutation;
//! applying them still requires the resource and typed-consumer adapters.
#![allow(dead_code)]
use crate::{composition_form_budget::Budget,composition_forms::FormInfo,EngineError,EngineResult};
use lopdf::{Object,ObjectId};
use std::collections::{BTreeMap,BTreeSet};

#[derive(Debug,Default,PartialEq)]
pub(crate) struct Namespace {
    pub roots:Vec<ObjectId>,pub calculation_order:Vec<ObjectId>,
    pub need_appearances:Option<bool>,
    pub root_titles:BTreeMap<ObjectId,Object>,
    pub names:BTreeMap<ObjectId,String>,
}
/// Inventories must describe disjoint, already relocated sources in one private
/// graph. This stage owns only small metadata; it never clones source payloads.
pub(crate) fn namespace(sources:&[&FormInfo],budget:&mut Budget,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Namespace> {
    budget.step(0,check)?;
    let mut result=Namespace::default();
    let mut reserved=BTreeSet::new();let mut ids=BTreeSet::new();
    for source in sources {
        budget.step(0,check)?;
        if !source.present {
            require(source.nodes.is_empty() && source.roots.is_empty() && source.calculation_order.is_empty(),"absent form has nodes")?;
            continue;
        }
        if let Some(policy)=result.need_appearances {require(policy==source.need_appearances,"conflicting NeedAppearances requires a normalization adapter")?;}
        result.need_appearances=Some(source.need_appearances);
        for (&id,node) in &source.nodes {
            budget.step(0,check)?;budget.reserve(96)?;
            require(ids.len()<4000 && ids.insert(id),"repeated or excessive source field identity")?;
            if node.field && node.partial_name.is_some(){reserved.insert(node.full_name.as_str());}
        }
    }
    let mut used=BTreeSet::new();let mut visited=BTreeSet::new();
    let mut next_suffix=BTreeMap::new();
    for source in sources {
        for &root in &source.roots {
            budget.step(0,check)?;
            let node=source.nodes.get(&root).ok_or_else(||bad("missing field root"))?;
            require(node.field && node.parent.is_none(),"invalid field root")?;
            let mut subtree=Vec::new();collect(source,root,None,0,&mut subtree,&mut visited,budget,check)?;
            let mut candidate=names(source,&subtree,None,budget,check)?;
            let collision=candidate.values().any(|(name,named)|*named && used.contains(name));
            if collision {
                let title=node.partial_name.as_deref().unwrap_or("Folio");
                // Monotone per-title suffixes avoid quadratic rescanning when
                // many independent source forms use the same field names.
                // Original paths remain reserved regardless of source order.
                if !next_suffix.contains_key(title){budget.reserve(64)?;next_suffix.insert(title,2usize);}
                let mut number=next_suffix[title];
                loop {
                    budget.step(0,check)?;
                    // Twenty decimal digits cover every usize suffix before
                    // allocation; the traversal limit bounds retry work.
                    budget.reserve(size(title.len(),32)?)?;
                    let replacement=format!("{title}_{number}");
                    candidate=names(source,&subtree,Some(&replacement),budget,check)?;
                    if candidate.values().all(|(name,named)|!named || (!reserved.contains(name.as_str()) && !used.contains(name))) {
                        budget.reserve(size(replacement.len().checked_mul(2).ok_or_else(||bad("name size overflow"))?,96)?)?;
                        result.root_titles.insert(root,lopdf::text_string(&replacement));
                        next_suffix.insert(title,number.checked_add(1).ok_or_else(||bad("name suffix overflow"))?);
                        break;
                    }
                    number=number.checked_add(1).ok_or_else(||bad("name suffix overflow"))?;
                }
            }
            for (id,(name,named)) in candidate {
                budget.step(0,check)?;
                if named {budget.reserve(size(name.len(),64)?)?;require(used.insert(name.clone()),"duplicate staged field name")?;}
                require(result.names.insert(id,name).is_none(),"repeated staged identity")?;
            }
            budget.reserve(32)?;result.roots.push(root);
        }
        for &id in &source.calculation_order {
            budget.step(0,check)?;
            let node=source.nodes.get(&id).ok_or_else(||bad("missing calculation target"))?;
            require(node.field && node.kind.is_some() && node.children.iter().all(|child|source.nodes.get(child).is_some_and(|node|!node.field)),"calculation target is not a terminal field")?;
            budget.reserve(32)?;result.calculation_order.push(id);
        }
    }
    require(visited==ids,"field inventory has unreachable nodes")?;
    budget.step(0,check)?;Ok(result)
}

fn bad(message:&str)->EngineError {EngineError::InvalidRequest(format!("PDF form namespace assembly: {message}"))}
fn require(condition:bool,message:&str)->EngineResult<()> {if condition{Ok(())}else{Err(bad(message))}}
fn size(bytes:usize,overhead:usize)->EngineResult<usize>{bytes.checked_add(overhead).ok_or_else(||bad("metadata size overflow"))}
fn collect(source:&FormInfo,id:ObjectId,parent:Option<ObjectId>,depth:usize,result:&mut Vec<ObjectId>,visited:&mut BTreeSet<ObjectId>,budget:&mut Budget,check:&mut dyn FnMut()->EngineResult<()>)->EngineResult<()> {
    budget.step(depth,check)?;budget.reserve(96)?;
    require(visited.insert(id),"cyclic or repeated field subtree")?;
    let node=source.nodes.get(&id).ok_or_else(||bad("missing subtree node"))?;
    require(node.parent==parent,"stale field parent")?;
    result.push(id);
    for &child in &node.children {collect(source,child,Some(id),depth+1,result,visited,budget,check)?;}
    Ok(())
}
fn names(source:&FormInfo,subtree:&[ObjectId],replacement:Option<&str>,budget:&mut Budget,check:&mut dyn FnMut()->EngineResult<()>)->EngineResult<BTreeMap<ObjectId,(String,bool)>> {
    let mut result:BTreeMap<ObjectId,(String,bool)>=BTreeMap::new();
    for (index,&id) in subtree.iter().enumerate() {
        budget.step(0,check)?;
        let node=&source.nodes[&id];
        let name=if let Some(title)=replacement {
            if index==0 {budget.reserve(size(title.len(),128)?)?;title.to_owned()}
            else {
                let parent=node.parent.and_then(|parent|result.get(&parent)).ok_or_else(||bad("missing staged parent"))?;
                let length=size(parent.0.len(),node.partial_name.as_ref().map_or(0,|name|name.len()))?;
                budget.reserve(size(length,129)?)?;
                match &node.partial_name {Some(name)=>crate::form_properties::qualified_name(&parent.0,name),None=>parent.0.clone()}
            }
        } else {budget.reserve(size(node.full_name.len(),128)?)?;node.full_name.clone()};
        let named=node.field && (node.partial_name.is_some() || (index==0 && replacement.is_some()));
        result.insert(id,(name,named));
    }
    Ok(result)
}
#[cfg(test)]
#[path="composition_form_assembly_tests.rs"]
mod tests;
