//! Whole-source catalog assembly for admitted structured Save requests.
#![allow(dead_code)]
use crate::{EngineError,EngineResult};
use lopdf::{dictionary,Document,Object,ObjectId};

pub(crate) struct Source {
    pub catalog:ObjectId,
    pub version:String,
    pub info:Option<ObjectId>,
}
#[derive(Clone,Copy,PartialEq,Eq)]
pub(crate) enum OutlineIntent {Guarded,Preserve,Discard,CompatibleReplace}
/// Form-aware whole-source assembly used by graph-preserving Save. Selection
/// and occurrence handling must use the matching form-aware ownership adapters.
pub(crate) fn assemble_forms(document:Document,sources:Vec<Source>,intent:OutlineIntent,check:&mut impl FnMut()->EngineResult<()>,cancelled:std::sync::Arc<dyn Fn()->bool+Send+Sync>)->EngineResult<Document>{
    assemble_forms_metadata(document,sources,intent,true,check,cancelled)
}
pub(crate) fn assemble_forms_metadata(document:Document,sources:Vec<Source>,intent:OutlineIntent,preserve_metadata:bool,check:&mut impl FnMut()->EngineResult<()>,cancelled:std::sync::Arc<dyn Fn()->bool+Send+Sync>)->EngineResult<Document>{
    let cancellation=cancelled.clone();
    let mut guarded=||{
        if cancellation(){return Err(bad("cancelled"));}
        check()?;
        if cancellation(){return Err(bad("cancelled"));}Ok(())
    };
    assemble_impl(document,sources,intent,true,preserve_metadata,&mut guarded,cancelled)
}
pub(crate) fn assemble(
    document:Document,
    sources:Vec<Source>,
    check:&mut impl FnMut()->EngineResult<()>,
    cancelled:std::sync::Arc<dyn Fn()->bool+Send+Sync>,
) -> EngineResult<Document> {
    assemble_navigation(document,sources,OutlineIntent::Guarded,check,cancelled)
}
pub(crate) fn assemble_navigation(
    document:Document,
    sources:Vec<Source>,
    intent:OutlineIntent,
    check:&mut impl FnMut()->EngineResult<()>,
    cancelled:std::sync::Arc<dyn Fn()->bool+Send+Sync>,
) -> EngineResult<Document> {
    assemble_impl(document,sources,intent,false,true,check,cancelled)
}
fn assemble_impl(
    mut document:Document,
    sources:Vec<Source>,
    intent:OutlineIntent,
    allow_forms:bool,
    preserve_metadata:bool,
    check:&mut impl FnMut()->EngineResult<()>,
    cancelled:std::sync::Arc<dyn Fn()->bool+Send+Sync>,
) -> EngineResult<Document> {
    check()?;
    require(!sources.is_empty() && sources.len()<=100_000)?;
    let mut trees=Vec::new();
    let mut all_pages=Vec::new();
    let mut structures=Vec::new();
    let mut source_pdf2=Vec::new();
    let mut version=(1,7);
    for source in &sources {
        check()?;
        let mut source_version=pdf_version(source.version.as_bytes())?;
        let tree=crate::composition_pages::inspect(&document,source.catalog,check)?;
        let catalog=document.get_dictionary(tree.catalog).map_err(bad)?;
        for (key,value) in catalog.iter() {
            check()?;
            if matches!(value,Object::Null){continue;}
            require([b"Type".as_slice(),b"Pages",b"Version",b"Lang",b"ViewerPreferences",b"Metadata",b"StructTreeRoot",b"MarkInfo"].contains(&key.as_slice())
                || (allow_forms && key==b"AcroForm")
                || (intent!=OutlineIntent::Guarded && [b"Outlines".as_slice(),b"Dests",b"Names"].contains(&key.as_slice())))?;
        }
        if let Ok(value)=catalog.get(b"Version") {source_version=source_version.max(pdf_version(resolved(&document,value)?.as_name().map_err(bad)?)?);}
        version=version.max(source_version);source_pdf2.push(source_version>=(2,0));
        if let Some(info)=source.info {document.get_dictionary(info).map_err(bad)?;}
        let structure=crate::composition_structure::inspect(&document,tree.catalog,&tree.pages,check)?;
        if structure.root.is_none() {
          if let Ok(mark)=catalog.get(b"MarkInfo") {
            if !matches!(resolved(&document,mark)?,Object::Null) {
                let mark=resolved(&document,mark)?.as_dict().map_err(bad)?;
                require(mark.iter().all(|(key,value)| key==b"Marked" && value==&Object::Boolean(false)))?;
            }
          }
        }
        structures.push(structure);
        require(all_pages.len()+tree.pages.len()<=100_000)?;
        all_pages.extend_from_slice(&tree.pages);
        trees.push(tree);
    }
    require(!all_pages.is_empty())?;
    let mut form_budget=crate::composition_form_budget::Budget::default();
    let mut form_plan=if allow_forms {
        let plan=crate::composition_form_catalog::prepare(&document,&trees,&mut form_budget,cancelled.clone(),check)?;
        // A widget's OBJR does not establish the language of parent/nonvisual
        // field prose. Keep that unsupported case guarded before mutation.
        for (tree,form) in trees.iter().zip(&plan.forms){
            form_budget.step(0,check)?;
            if form.present {require(crate::composition_language::same_default(&document,trees[0].catalog,tree.catalog)?)?;}
        }
        Some(plan)
    }else{None};
    // Inspect before any joining. Deliberate discard does not require a readable
    // original tree, and must remove it from subsequent language traversal.
    let mut outlines=Vec::new();
    for (tree,structure) in trees.iter().zip(&structures) {
        check()?;
        let info=if matches!(intent,OutlineIntent::Preserve|OutlineIntent::CompatibleReplace) {
            crate::composition_outlines::inspect(&document,tree.catalog,structure,check)?
        } else {crate::composition_outlines::OutlineInfo::default()};
        outlines.push(info);
    }
    if intent==OutlineIntent::CompatibleReplace {
        crate::composition_outlines::compatible_replacement(&document,&trees,&outlines,check)?;
    }
    if intent==OutlineIntent::Discard {
        for tree in &trees {check()?;document.get_dictionary_mut(tree.catalog).map_err(bad)?.remove(b"Outlines");}
        outlines.iter_mut().for_each(|info|*info=Default::default());
    }
    // Compatible reconstruction must still admit every original title's source
    // language. Keep those trees reachable until the per-source language pass;
    // changing one title cannot erase another source's language requirements.
    let navigation=if let Some(plan)=form_plan.as_ref().filter(|plan|plan.namespace.need_appearances.is_some()) {
        Some(crate::composition_navigation::reconcile_forms(&mut document,&trees,&outlines,&plan.forms,&plan.namespace,&mut form_budget,check)?)
    } else if intent!=OutlineIntent::Guarded {
        Some(crate::composition_navigation::reconcile(&mut document,&trees,&outlines,check)?)
    } else {None};
    if let Some(plan)=&mut form_plan {
        let patches=std::mem::replace(&mut plan.patches,crate::composition_form_patch::Patches{entries:Default::default(),global_da:None});
        document=crate::composition_form_patch::apply(document,patches,&mut form_budget,check)?;
    }
    document.max_id=document.max_id.max(document.objects.keys().next_back().map_or(0,|id|id.0));
    require(document.max_id.checked_add(if trees.len()==1{1}else{2}).is_some())?;
    // Re-saving one source does not join independent page trees. Reusing its
    // validated root preserves inherited fields and avoids a live wrapper on
    // every round, which reachability cleanup correctly cannot remove.
    let pages_root=if trees.len()==1{trees[0].root}else{document.new_object_id()};
    let catalog_id=document.new_object_id();
    let mut catalog=dictionary!{"Type"=>"Catalog","Pages"=>pages_root};
    if let Some(form)=form_plan.as_mut().and_then(|plan|plan.form.take()){catalog.set("AcroForm",form);}
    // Only small header values may be copied. Large streams/structure/resources
    // remain shared by their relocated identities, never copied here.
    let mut header_budget=1_048_576usize;
    for key in [b"ViewerPreferences".as_slice()] {
        if let Ok(value)=document.get_dictionary(trees[0].catalog).map_err(bad)?.get(key) {
            catalog.set(key,copy_header(value,&mut header_budget,check,0)?);
        }
    }
    // The leading source owns the retained ordinary Info dictionary. Preserve
    // its default and adapt other sources' content/annotation language scopes.
    let language_catalog=trees[0].catalog;
    let language=source_language(&document,language_catalog)?;
    let default_language=if language.is_some() {
        let value=document.get_dictionary(language_catalog).map_err(bad)?.get(b"Lang").map_err(bad)?;
        copy_header(value,&mut header_budget,check,0)?
    } else {Object::string_literal("")};
    catalog.set("Lang",default_language);
    let structured=crate::composition_language::adapt(&mut document,&trees,&structures,&source_pdf2,cancelled,check)?;
    // Joining earlier would let one source's language walk reach another's
    // bookmark titles through the newly connected sibling chain.
    if intent==OutlineIntent::Preserve {
        if let Some(root)=crate::composition_outlines::merge(&mut document,&outlines,check)? {catalog.set("Outlines",root);}
    }
    if let Some(navigation)=navigation {
        if let Some(dests)=navigation.dests {catalog.set("Dests",dests);}
        if let Some(names)=navigation.names {catalog.set("Names",names);}
    }
    if !structured.is_empty() {
      let mut combined_mark=lopdf::Dictionary::new();
      for (source_catalog,structure) in &structured {
        let source_catalog=document.get_dictionary(*source_catalog).map_err(bad)?;
        let language=match source_catalog.get(b"Lang") {
            Ok(value) if !matches!(resolved(&document,value)?,Object::Null)=>copy_header(value,&mut header_budget,check,0)?,
            _=>Object::string_literal(""),
        };
        let mark=match source_catalog.get(b"MarkInfo") {
            Ok(value) if !matches!(resolved(&document,value)?,Object::Null)=>{
                let dictionary=resolved(&document,value)?.as_dict().map_err(bad)?;
                let mut mark=lopdf::Dictionary::new();
                for (key,value) in dictionary.iter() {
                    require([b"Marked".as_slice(),b"UserProperties",b"Suspects"].contains(&key.as_slice()))?;
                    mark.set(key.clone(),resolved(&document,value)?.as_bool().map_err(bad)?);
                }
                mark
            }
            _=>lopdf::Dictionary::new(),
        };
        for (key,value) in mark.iter() {
            if key!=b"Marked" {
                let previous=combined_mark.get(key).ok().and_then(|value|value.as_bool().ok()).unwrap_or(false);
                combined_mark.set(key.clone(),previous || value.as_bool().map_err(bad)?);
            }
        }
        {
            for child in &structure.top {
                check()?;
                let dictionary=document.get_dictionary(*child).map_err(bad)?;
                let missing=match dictionary.get(b"Lang") {Ok(value)=>matches!(resolved(&document,value)?,Object::Null),Err(_)=>true};
                if missing {
                    let language=copy_header(&language,&mut header_budget,check,0)?;
                    document.get_dictionary_mut(*child).map_err(bad)?.set("Lang",language);
                }
            }
        }
      }
      // Retain an original single hierarchy exactly; reconcile multiple roots
      // only after their independent ownership inspections have succeeded.
      let root=if structured.len()==1 {structured[0].1.root.unwrap()}else{
          let structures=structured.into_iter().map(|(_,structure)|structure).collect::<Vec<_>>();
          crate::composition_structure_merge::merge(&mut document,&structures,check)?
      };
      if document.get_dictionary(root).map_err(bad)?.has(b"Namespaces"){version=version.max((2,0));}
      combined_mark.set("Marked",false);
      catalog.set("MarkInfo",combined_mark);
      catalog.set("StructTreeRoot",root);
    }
    // The merger may promote the final version for retained namespaces too.
    // Check every older source, including leading/equal-language sources that
    // needed no language framing, before that promotion can be published.
    if version>=(2,0) {
        for (tree,pdf2) in trees.iter().zip(&source_pdf2) {
            if !pdf2 {crate::composition_annotation_language::validate_version_promotion(&document,&tree.pages,check)?;}
        }
    }
    if trees.len()>1 {
        for tree in &trees {
            check()?;
            document.get_dictionary_mut(tree.root).map_err(bad)?.set("Parent",pages_root);
        }
        document.objects.insert(pages_root,dictionary!{"Type"=>"Pages","Count"=>all_pages.len() as i64,"Kids"=>trees.iter().map(|tree|Object::Reference(tree.root)).collect::<Vec<_>>()}.into());
    }
    let info=if preserve_metadata {
        let original_info=sources[0].info.map(Object::Reference);
        let prepared=crate::document_metadata::prepare(&document,trees[0].catalog,original_info.as_ref(),crate::document_metadata::base(&document),false,check)?;
        let (metadata,info)=prepared.install(&mut document);
        if let Some(metadata)=metadata {catalog.set("Metadata",metadata);}
        info
    } else {sources[0].info.map(Object::Reference)};
    document.objects.insert(catalog_id,catalog.into());
    // Source trailer/xref offsets and encryption IDs do not describe the new
    // document. Ordinary Info/XMP belong to its leading source; recognized
    // conformance properties have been removed from private catalog roots.
    document.trailer= dictionary!{"Root"=>catalog_id};
    if let Some(info)=info {document.trailer.set("Info",info);}
    document.version=format!("{}.{}",version.0,version.1);
    let actual=crate::composition_pages::inspect(&document,catalog_id,check)?;
    require(actual.pages==all_pages)?;
    crate::composition_structure::inspect(&document,catalog_id,&all_pages,check)?;
    if let Some(plan)=form_plan {
        validate_forms(&document,&actual,&plan,&mut form_budget,check)?;
    }
    check()?;
    Ok(document)
}

fn validate_forms(document:&Document,pages:&crate::composition_pages::SourcePages,plan:&crate::composition_form_catalog::Plan,budget:&mut crate::composition_form_budget::Budget,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<()> {
    // Count unique nodes afresh for the final graph, while retaining all
    // cumulative traversal, parser and metadata charges from preparation.
    budget.nodes=0;
    let actual=crate::composition_forms::inspect(document,pages,budget,check)?;
    require(actual.present==plan.namespace.need_appearances.is_some()
        && actual.roots==plan.namespace.roots && actual.calculation_order==plan.namespace.calculation_order
        && plan.namespace.need_appearances.map_or(true,|value|actual.need_appearances==value))?;
    let mut count=0;
    for form in &plan.forms {for (id,old) in &form.nodes {
        budget.step(0,check)?;count+=1;
        let new=actual.nodes.get(id).ok_or_else(||bad("final form lost an original node"))?;
        require(new.parent==old.parent && new.children==old.children && new.owner==old.owner
            && new.widget_page==old.widget_page && new.field==old.field && new.kind==old.kind
            && plan.namespace.names.get(id)==Some(&new.full_name))?;
    }}
    require(count==actual.nodes.len())?;budget.step(0,check)
}

fn bad(message:impl std::fmt::Display)->EngineError {EngineError::InvalidRequest(format!("PDF catalog assembly: {message}"))}
fn require(value:bool)->EngineResult<()> {if value {Ok(())} else {Err(bad("source needs an unsupported catalog or structure adapter"))}}
fn pdf_version(value:&[u8])->EngineResult<(u8,u8)> {
    match value { [b'1',b'.',minor @ b'0'..=b'7']=>Ok((1,minor-b'0')),b"2.0"=>Ok((2,0)),_=>Err(bad("unsupported PDF version")) }
}
fn resolved<'a>(doc:&'a Document,value:&'a Object)->EngineResult<&'a Object> {
    // lopdf's bounded dereference retains alias identity without stream cloning.
    doc.dereference(value).map(|(_,value)|value).map_err(bad)
}
fn source_language(doc:&Document,catalog:ObjectId)->EngineResult<Option<&[u8]>> {
    match doc.get_dictionary(catalog).map_err(bad)?.get(b"Lang") {
        Ok(value)=>match resolved(doc,value)? {Object::Null=>Ok(None),value=>Ok(Some(value.as_str().map_err(bad)?))},
        Err(_)=>Ok(None),
    }
}
fn copy_header(value:&Object,budget:&mut usize,check:&mut impl FnMut()->EngineResult<()>,depth:usize)->EngineResult<Object> {
    fn charge(value:&Object,budget:&mut usize,check:&mut impl FnMut()->EngineResult<()>,depth:usize)->EngineResult<()> {
        check()?;require(depth<=64)?;
        let bytes=match value {Object::Name(bytes)|Object::String(bytes,_)=>bytes.len(),Object::Stream(_)=>return Err(bad("stream-valued header")),_=>0};
        *budget=budget.checked_sub(bytes.checked_add(32).ok_or_else(||bad("header limit"))?).ok_or_else(||bad("header limit"))?;
        match value {
            Object::Array(values)=>for value in values{charge(value,budget,check,depth+1)?;},
            Object::Dictionary(dictionary)=>for (key,value) in dictionary.iter(){*budget=budget.checked_sub(key.len()).ok_or_else(||bad("header limit"))?;charge(value,budget,check,depth+1)?;},
            _=>{},
        }
        Ok(())
    }
    charge(value,budget,check,depth)?;
    Ok(value.clone())
}

#[cfg(test)]
#[path="composition_catalog_tests.rs"]
mod tests;

#[cfg(test)]
#[path="composition_catalog_form_tests.rs"]
mod form_tests;
