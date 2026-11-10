//! Staged AcroForm assembly before typed action/name and page projection work.
#![allow(dead_code)]
use crate::{composition_form_assembly::Namespace,composition_form_budget::Budget,composition_form_patch::Patches,composition_pages::SourcePages,EngineError,EngineResult};
use lopdf::{Dictionary,Document,Object};
use std::{collections::BTreeSet,sync::Arc};
pub(crate) struct Plan {pub form:Option<Dictionary>,pub namespace:Namespace,pub patches:Patches,pub forms:Vec<crate::composition_forms::FormInfo>}
/// Read-only preparation. Field titles are deliberately kept in namespace,
/// separate from default/resource patches: publishing them requires the typed
/// field action adapter and final ownership validation first.
pub(crate) fn prepare(doc:&Document,pages:&[SourcePages],budget:&mut Budget,cancelled:Arc<dyn Fn()->bool+Send+Sync>,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Plan>{
    let mut checked=||{if cancelled(){return Err(bad("cancelled"));}check()};
    budget.step(0,&mut checked)?;require(!pages.is_empty() && pages.len()<=100_000,"invalid source count")?;
    let mut forms=Vec::new();let mut defaults=Vec::new();let mut flags=0i64;let mut has_flags=false;
    for pages in pages {
        budget.step(0,&mut checked)?;budget.reserve(256)?;
        let form=crate::composition_forms::inspect(doc,pages,budget,&mut checked)?;
        if form.present {
            let catalog=doc.get_dictionary(pages.catalog).map_err(bad)?;
            let dictionary=resolve(doc,catalog.get(b"AcroForm").map_err(bad)?,budget,&mut checked)?.as_dict().map_err(bad)?;
            for (key,value) in dictionary.iter() {
                budget.step(0,&mut checked)?;budget.reserve(key.len())?;
                let value=resolve(doc,value,budget,&mut checked)?;
                if matches!(value,Object::Null){continue;}
                require([b"Fields".as_slice(),b"NeedAppearances",b"SigFlags",b"CO",b"DR",b"DA",b"Q"].contains(&key.as_slice()),"unsupported AcroForm entry")?;
                match key.as_slice() {
                    b"SigFlags"=>{let value=value.as_i64().map_err(bad)?;require((0..=3).contains(&value),"unsupported signature flags")?;flags|=value;has_flags=true;},
                    b"DA"=>{require(value.as_str().map_err(bad)?.len()<=64*1024,"DA exceeds 64 KiB")?;},
                    b"Q"=>{require(matches!(value,Object::Integer(0..=2)),"invalid default alignment")?;},
                    _=>{},
                }
            }
        }
        forms.push(form);
    }
    for (pages,form) in pages.iter().zip(&forms) {
        defaults.push(crate::composition_form_defaults::inspect(doc,pages,form,budget,cancelled.clone(),&mut checked)?);
    }
    budget.reserve(forms.len().checked_mul(16).ok_or_else(||bad("source count overflow"))?)?;
    let namespace=crate::composition_form_assembly::namespace(&forms.iter().collect::<Vec<_>>(),budget,&mut checked)?;
    if namespace.need_appearances.is_none(){return Ok(Plan{form:None,namespace,patches:Patches{entries:Default::default(),global_da:None},forms});}
    budget.reserve(forms.len().checked_mul(32).ok_or_else(||bad("source count overflow"))?)?;
    let resource_sources=pages.iter().zip(&defaults).map(|(pages,defaults)|crate::composition_form_resources::Source{catalog:pages.catalog,defaults}).collect::<Vec<_>>();
    let mut resources=crate::composition_form_resources::stage(doc,&resource_sources,budget,&mut checked)?;
    let mut forbidden=BTreeSet::new();
    if resources.font_names.iter().any(|map|map.iter().any(|(old,new)|old!=new)) {
        budget.reserve(pages.len().checked_mul(16).ok_or_else(||bad("source count overflow"))?)?;
        let roots=pages.iter().map(|pages|pages.catalog).collect::<Vec<_>>();
        forbidden=crate::composition_form_scope::reserve_names(doc,&roots,budget,cancelled.clone(),&mut checked)?;
        if resources.font_names.iter().any(|map|map.iter().any(|(old,new)|old!=new && forbidden.contains(new))) {
            // All temporary work remains charged to the same cumulative budget.
            resources=crate::composition_form_resources::stage_reserved(doc,&resource_sources,&forbidden,budget,&mut checked)?;
        }
    }
    budget.reserve(forms.len().checked_mul(32).ok_or_else(||bad("source count overflow"))?)?;
    let patch_sources=forms.iter().zip(&defaults).zip(&resources.font_names).map(|((forms,defaults),aliases)|crate::composition_form_patch::Source{forms,defaults,aliases}).collect::<Vec<_>>();
    let mut patches=crate::composition_form_patch::stage(doc,&patch_sources,&forbidden,budget,cancelled.clone(),&mut checked)?;
    let mut form=Dictionary::new();
    budget.reserve(namespace.roots.len().checked_mul(32).and_then(|n|n.checked_add(256)).ok_or_else(||bad("field array overflow"))?)?;
    form.set("Fields",namespace.roots.iter().copied().map(Object::Reference).collect::<Vec<_>>());
    form.set("NeedAppearances",namespace.need_appearances.unwrap());
    if has_flags{form.set("SigFlags",flags);}
    if !namespace.calculation_order.is_empty() {
        budget.reserve(namespace.calculation_order.len().checked_mul(32).ok_or_else(||bad("calculation array overflow"))?)?;
        form.set("CO",namespace.calculation_order.iter().copied().map(Object::Reference).collect::<Vec<_>>());
    }
    if !resources.dictionary.is_empty(){form.set("DR",resources.dictionary);}
    if let Some(value)=patches.global_da.take(){form.set("DA",value);}
    if namespace.roots.is_empty() {
        // No existing fields can carry per-source defaults. Preserve the first
        // form's raw defaults only when every other empty form agrees after
        // font-name mapping; otherwise the merged authoring default is unclear.
        let mut expected_da:Option<Vec<u8>>=None;let mut expected_q=None;
        for ((pages,source),aliases) in pages.iter().zip(&forms).zip(&resources.font_names) {
            if !source.present{continue;}
            budget.step(0,&mut checked)?;
            let catalog=doc.get_dictionary(pages.catalog).map_err(bad)?;
            let original=resolve(doc,catalog.get(b"AcroForm").map_err(bad)?,budget,&mut checked)?.as_dict().map_err(bad)?;
            let raw_da=original.get(b"DA").ok();
            let bytes=match raw_da.map(|v|resolve(doc,v,budget,&mut checked)).transpose()? {Some(Object::String(bytes,_))=>bytes.as_slice(),Some(Object::Null)|None=>b"",_=>return Err(bad("invalid empty form DA"))};
            let rewritten=crate::composition_form_da::rewrite(bytes,aliases,budget,cancelled.clone(),&mut checked)?;
            let output=rewritten.as_deref().unwrap_or(bytes);
            budget.reserve(output.len())?;
            let alignment=match original.get(b"Q").ok().map(|v|resolve(doc,v,budget,&mut checked)).transpose()? {Some(Object::Integer(n))=>*n,Some(Object::Null)|None=>0,_=>return Err(bad("invalid empty form alignment"))};
            if let Some(expected)=&expected_da {
                require(expected==output && expected_q==Some(alignment),"incompatible defaults on empty source forms")?;
            } else {
                budget.reserve(output.len().checked_add(64).ok_or_else(||bad("default size overflow"))?)?;
                expected_da=Some(output.to_vec());expected_q=Some(alignment);
                if let Some(bytes)=rewritten {form.set("DA",Object::string_literal(bytes));}
                else if let Some(value)=raw_da {form.set("DA",crate::composition_form_resources::copy_metadata(doc,value,budget,&mut checked)?);}
                if let Ok(value)=original.get(b"Q"){form.set("Q",crate::composition_form_resources::copy_metadata(doc,value,budget,&mut checked)?);}
            }
        }
    }
    budget.step(0,&mut checked)?;Ok(Plan{form:Some(form),namespace,patches,forms})
}
fn bad(message:impl std::fmt::Display)->EngineError{EngineError::InvalidRequest(format!("PDF form catalog staging: {message}"))}
fn require(value:bool,message:&str)->EngineResult<()>{if value{Ok(())}else{Err(bad(message))}}
fn resolve<'a>(doc:&'a Document,mut value:&'a Object,budget:&mut Budget,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<&'a Object>{
    for depth in 0..=64{budget.step(depth,check)?;match value{Object::Reference(id)=>value=doc.objects.get(id).ok_or_else(||bad("missing object"))?,_=>return Ok(value)}}Err(bad("cyclic or excessive aliases"))
}
#[cfg(test)]
#[path="composition_form_catalog_tests.rs"]
mod tests;
