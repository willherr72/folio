//! Preserve source graphs while combining selected structured PDF occurrences.
use super::*;

fn eligibility_error()->EngineError {
    EngineError::InvalidRequest(
        "Saving this document structure requires valid source pages within the supported composition limits.".into()
    )
}

fn validate_output_pages(document:&lopdf::Document,expected_pages:&[lopdf::ObjectId],check:&mut impl FnMut()->EngineResult<()>)->EngineResult<()> {
    check()?;
    let catalog=document.trailer.get(b"Root").and_then(lopdf::Object::as_reference)
        .map_err(|error|EngineError::InvalidRequest(error.to_string()))?;
    if crate::composition_pages::inspect(document,catalog,check)?.pages!=expected_pages {
        return Err(EngineError::InvalidRequest("Combined source page identities changed during saving.".into()));
    }
    Ok(())
}

impl WorkerRuntime {
    // Outer composition already validated the request, bookmarks and source
    // permissions. Protection and atomic publication still run after this method.
    pub(super) fn structured_source_save(
        &self,
        request:&ExportRequest,
        bookmarks:&[crate::DocumentBookmark],
        discard_bookmarks:bool,
        preserve_metadata:bool,
        monitor:Option<&crate::pdf_save::PdfSaveMonitor>,
    )->EngineResult<Vec<u8>> {
        let check=|stage,completed|monitor.map_or(Ok(()),|m|m.checkpoint(stage,completed));
        let mut cancel_check=||monitor.map_or(Ok(()),|m|m.check_cancelled());
        let mut sources=Vec::new();let mut indices=HashMap::new();let mut total_pages=0usize;let mut selected_counts=Vec::new();
        for page in &request.pages {
            cancel_check()?;
            let id=page.source_id.as_str();
            let source=self.document(id)?;
            let count=source.document.page_count() as usize;
            if count==0 || page.page_index>=count {return Err(eligibility_error());}
            if !indices.contains_key(id) {
                total_pages=total_pages.checked_add(count).filter(|n|*n<=100_000).ok_or_else(eligibility_error)?;
                indices.insert(id,sources.len());sources.push((id,count));selected_counts.push(0usize);
            }
            selected_counts[indices[id]]+=1;
        }
        if sources.is_empty(){return Err(eligibility_error());}
        // Stable comparison keys include omitted pages too. They are internal to
        // this transaction and cannot collide with user/workspace page IDs.
        let token=|source:usize,index:usize|format!("source{source}/page{index}");
        let mut occurrences=HashMap::new();let mut final_tokens=Vec::with_capacity(request.pages.len());
        for page in &request.pages {
            cancel_check()?;let source=indices[page.source_id.as_str()];let occurrence=occurrences.entry((source,page.page_index)).or_insert(0usize);
            let base=token(source,page.page_index);
            final_tokens.push(if *occurrence==0 {base}else{format!("{base}/copy{occurrence}")});*occurrence+=1;
        }
        let requested_map=request.pages.iter().zip(&final_tokens).map(|(page,key)|(page.id.clone(),key.clone())).collect();
        let normalized=crate::bookmarks::remap_complete(std::iter::once(Ok((bookmarks,requested_map))),&mut cancel_check)?;
        // Compare current source projections, independent of historical security
        // provenance. Authorization has already run against the original policy.
        use crate::composition_catalog::OutlineIntent;
        let intent=if discard_bookmarks {
            debug_assert!(bookmarks.is_empty());
            OutlineIntent::Discard
        } else {
        let mut selected_maps=vec![HashMap::new();sources.len()];
        for page in &request.pages {
            cancel_check()?;
            let key=indices[page.source_id.as_str()];
            selected_maps[key].insert(page.page_index.to_string(),token(key,page.page_index));
        }
        let originals=sources.iter().zip(selected_maps).map(|((id,count),mapping)|{
            let source=self.document(id)?;
            let items=source.source_bookmarks()?
                .as_ref().map_err(|error|EngineError::InvalidRequest(error.clone()))?;
            Ok((items.as_slice(),mapping,*count))
        });
        let original=crate::bookmarks::remap_selected(originals,&mut cancel_check);
        cancel_check()?;
        match original {
            Ok(original) if complete_source_save::same_bookmarks(&original,&normalized)=>OutlineIntent::Preserve,
            Ok(_)=>OutlineIntent::CompatibleReplace,
            Err(error)=>return Err(error),
        }
        };
        let control=monitor.map(|m|m.control.clone());
        let cancelled:Arc<dyn Fn()->bool+Send+Sync>=Arc::new(move||control.as_ref().is_some_and(|control|control.cancelled()));
        let mut graph=crate::composition_graph::CompositionGraph::new();
        let mut all_pages=Vec::new();let mut relocated=Vec::new();
        use imported_overlay_sources::publication::{PageChanges,OwnedChange};
        let has_drawings=request.pages.iter().any(|page|!page.overlays.is_empty()
            || self.document(&page.source_id).is_ok_and(|s|s.imported_annotations.get(&page.page_index).is_some_and(|v|!v.is_empty())));
        // Hold borrowed intentions, not parsed source documents or asset copies.
        // Publication is temporary per source; its unused allowance passes to
        // the next source while all returned page intentions remain live.
        let mut plan_metadata=if has_drawings {16usize.checked_mul(1024*1024)
            .and_then(|n|n.checked_sub(request.pages.len().checked_mul(256)?)).ok_or_else(eligibility_error)?}else{0};
        let mut drawing_changes:Vec<Option<PageChanges<'_,'_>>>=if has_drawings {(0..request.pages.len()).map(|_|None).collect()}else{vec![]};
        let mut drawing_groups=std::collections::BTreeMap::new();
        check("copying",0)?;
        let mut copied=0usize;
        for (key,(id,count)) in sources.iter().enumerate(){
            let source=self.document(id)?;
            graph.import_bytes(key,source.bytes()?.as_ref(),cancelled.clone())?;
            let pages=graph.source_pages(key,&mut cancel_check)?;
            if pages.len()!=*count{return Err(EngineError::InvalidRequest("Source page mapping changed during structured saving.".into()));}
            if has_drawings {
                let selected=request.pages.iter().enumerate().filter(|(_,page)|page.source_id==*id);
                let source_work=selected.clone().any(|(_,page)|!page.overlays.is_empty()
                    || source.imported_annotations.get(&page.page_index).is_some_and(|v|!v.is_empty()));
                let mut publication=if source_work {Some(source.imported_overlay_sources.publication_with_metadata(source.source_bytes.identity(),*count,plan_metadata,&mut cancel_check)?)}else{None};
                let mut unique=std::collections::BTreeMap::new();
                for (output,page) in selected {
                    cancel_check()?;
                    let changes=if let Some(publication)=publication.as_mut(){publication.page(page.page_index,&page.overlays,&mut cancel_check)?}
                        else{PageChanges{owned:vec![],added:vec![],draft:&page.overlays}};
                    unique.entry(page.page_index).or_insert(output);
                    drawing_changes[output]=Some(changes);
                }
                // Invert source identities once for all selected original pages,
                // never once per output occurrence (or per empty page).
                let mut slots=Vec::new();let mut ranges=Vec::new();
                for (page,output) in unique {
                    let changes=drawing_changes[output].as_ref().expect("planned source page");
                    ranges.push((page,changes.owned.len()));
                    slots.extend(changes.owned.iter().map(|c|match c{OwnedChange::Keep(s,_)|OwnedChange::Replace(s,_)|OwnedChange::Delete(s)=>s.source_group()}));
                }
                let bound=if slots.is_empty(){vec![]}else{graph.source_annotation_groups(key,&slots,&mut cancel_check)?};
                let mut bound=bound.into_iter();
                for (page,count) in ranges {drawing_groups.insert((key,page),bound.by_ref().take(count).collect::<Vec<_>>());}
                debug_assert!(bound.next().is_none());
                if let Some(publication)=publication {plan_metadata=publication.remaining_metadata();}
            }
            all_pages.extend_from_slice(&pages);relocated.push(pages);
            copied+=selected_counts[key];check("copying",copied)?;
        }
        check("preserving",0)?;
        let languages=if has_drawings {graph.source_languages(&mut cancel_check)?}else{std::collections::BTreeMap::new()};
        let anchors=drawing_groups.values().flatten().flat_map(|g|std::iter::once(g.annotation).chain(g.popups.iter().copied())).collect::<Vec<_>>();
        let mut document=graph.assemble_navigation_metadata_controlled(&(0..sources.len()).collect::<Vec<_>>(),intent,preserve_metadata,cancelled,&mut cancel_check)?;
        validate_output_pages(&document,&all_pages,&mut cancel_check)?;
        if intent==OutlineIntent::CompatibleReplace {
            // Original metadata/language/external-consumer checks ran during
            // assembly. A replacement may address a copy that does not yet exist.
            document.catalog_mut().map_err(|e|EngineError::InvalidRequest(e.to_string()))?.remove(b"Outlines");
        }
        let selected=request.pages.iter().map(|page|relocated[indices[page.source_id.as_str()]][page.page_index]).collect::<Vec<_>>();
        let (mut document,expected_pages,receipts)=if has_drawings {
            crate::composition_occurrences::project_forms_tracked(document,&selected,&anchors,&mut cancel_check)?
        }else{let (document,pages)=crate::composition_occurrences::project_forms(document,&selected,&mut cancel_check)?;(document,pages,vec![])};
        if intent==OutlineIntent::CompatibleReplace {
            crate::bookmarks::write_document(&mut document,&normalized,&final_tokens)?;
            let catalog=document.trailer.get(b"Root").and_then(lopdf::Object::as_reference).map_err(|e|EngineError::InvalidRequest(e.to_string()))?;
            let pages=crate::composition_pages::inspect(&document,catalog,&mut cancel_check)?;
            let structure=crate::composition_structure::inspect(&document,catalog,&pages.pages,&mut cancel_check)?;
            let outlines=crate::composition_outlines::inspect(&document,catalog,&structure,&mut cancel_check)?;
            let mut budget=crate::composition_form_budget::Budget::default();
            let forms=crate::composition_forms::inspect(&document,&pages,&mut budget,&mut cancel_check)?;
            let values=if forms.present{
                let namespace=crate::composition_form_assembly::namespace(&[&forms],&mut budget,&mut cancel_check)?;
                crate::composition_navigation::reconcile_forms(&mut document,&[pages],&[outlines],&[forms],&namespace,&mut budget,&mut cancel_check)?
            }else{crate::composition_navigation::reconcile(&mut document,&[pages],&[outlines],&mut cancel_check)?};
            let dictionary=document.get_dictionary_mut(catalog).map_err(|e|EngineError::InvalidRequest(e.to_string()))?;
            for (key,value) in [(b"Dests".as_slice(),values.dests),(b"Names".as_slice(),values.names)]{
                match value{Some(value)=>dictionary.set(key,value),None=>{dictionary.remove(key);}}
            }
        }
        validate_output_pages(&document,&expected_pages,&mut cancel_check)?;
        check("preserving",request.pages.len())?;
        let mut geometries=Vec::with_capacity(request.pages.len());
        check("applying",0)?;
        if has_drawings {
            let changes=drawing_changes.iter().map(|c|c.as_ref().expect("every selected source was planned")).collect::<Vec<_>>();
            let prepared=self.prepare_structured_overlay_changes(request,&changes,&mut cancel_check)?;
            let targets=prepared.pages.iter().zip(&expected_pages).map(|(page,id)|structured_overlay_publish::TargetPage{page_id:*id,geometry:page.geometry,unit:page.unit}).collect::<Vec<_>>();
            let plans=request.pages.iter().zip(&changes).map(|(page,changes)|{
                let key=indices[page.source_id.as_str()];
                structured_overlay_batch::PagePlan{source_page:relocated[key][page.page_index],source_page_index:page.page_index,
                    source_language:&languages[&key],changes,groups:&drawing_groups[&(key,page.page_index)]}
            }).collect::<Vec<_>>();
            document=if request.flatten {structured_overlay_batch::publish_flattened(document,&targets,&plans,&receipts,prepared,&mut cancel_check)?}
                else{structured_overlay_batch::publish(document,&targets,&plans,&receipts,prepared,&mut cancel_check)?}.document;
        }
        for (index,(page,id)) in request.pages.iter().zip(&expected_pages).enumerate(){
            cancel_check()?;
            let source=self.document(&page.source_id)?;
            let geometry=page_geometry(&source.pdf()?.pages().get(page_index_i32(page.page_index)?)?)?;
            if page.rotation!=0 {
                document.get_dictionary_mut(*id).map_err(|error|EngineError::InvalidRequest(error.to_string()))?
                    .set("Rotate",i64::from((geometry.rotation+page.rotation)%360));
            }
            geometries.push(geometry);check("applying",index+1)?;
        }
        page_crop::apply(&mut document,&request.pages,&geometries)?;
        if has_drawings {document=crate::composition_projection::prune_final(document,&mut cancel_check)?;}
        crate::composition_graph::validate_retained(&document,&mut cancel_check)?;
        check("preparingFile",0)?;
        let bytes=crate::pdf_output::save_controlled(&mut document,monitor.map(|m|m.control.as_ref()))?;
        cancel_check()?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary,Document,Object};
    #[test]
    fn structured_save_mapping_checks_canonical_alias_and_indirect_type_ids() {
        for indirect_type in [false,true] {
            let mut doc=Document::with_version("1.7");let pages=doc.new_object_id();
            let kind=if indirect_type {Object::Reference(doc.add_object(Object::Name(b"Page".to_vec())))} else {Object::Name(b"Page".to_vec())};
            let page=doc.add_object(dictionary!{"Type"=>kind,"Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()]});
            let alias=doc.add_object(Object::Reference(page));
            doc.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(alias)]}.into());
            let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});doc.trailer.set("Root",catalog);
            validate_output_pages(&doc,&[page],&mut ||Ok(())).unwrap();
            assert!(validate_output_pages(&doc,&[alias],&mut ||Ok(())).is_err());
            assert!(validate_output_pages(&doc,&[page],&mut ||Err(EngineError::InvalidRequest("cancel".into()))).is_err());
        }
    }
}
