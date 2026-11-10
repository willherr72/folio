//! Disposable pixel/text views. Keep original objects and semantic consumers;
//! change only the selected page's drawing list and optional drawing content.
//! These bytes must never be published as an editable PDF or retained source.
use super::*;

pub(super) fn finish(
    mut document:Document,mut pages:Vec<GeneratedPage>,plans:&[PagePlan<'_,'_,'_>],
    entries:&[Vec<Entry<'_>>],paint_order:&[Vec<usize>],work:&mut Work<'_>,
)->EngineResult<Published>{
    require(pages.len()==1 && plans.len()==1 && entries.len()==1,"read view requires one selected page")?;
    let page=pages[0].page_id;let plan=&plans[0];let entries=&entries[0];
    work.reserve(entries.len(),512)?;
    let labels=entries.iter().enumerate().filter(|(_,e)|e.kind!=Kind::Delete).map(|(i,e)|(e.label,i)).collect::<BTreeMap<_,_>>();
    require(labels.len()==plan.changes.draft.len(),"read draft membership differs")?;
    let unchanged=entries.iter().all(|e|e.kind==Kind::Keep&&!e.flatten)
        && plan.changes.draft.iter().map(label).eq(entries.iter().map(|e|e.label));
    let mut selected=BTreeMap::new();
    for (index,entry) in entries.iter().enumerate(){
        work.step()?;work.reserve(1+entry.popups.len(),128)?;
        if let Some(root)=entry.root{require(selected.insert((Some(root.output_slot),root.annotation),(index,false)).is_none(),"read source annotation repeats")?;}
        for popup in &entry.popups{work.step()?;require(selected.insert((Some(popup.output_slot),popup.annotation),(index,false)).is_none(),"read source popup repeats")?;}
    }
    for generated in &pages[0].overlays{
        work.step()?;work.reserve(1,128)?;
        let index=*labels.get(generated.overlay_id.as_str()).ok_or_else(||bad("unknown read drawing"))?;
        require(selected.insert((None,generated.annotation_id),(index,true)).is_none(),"read drawing identity repeats")?;
    }
    if !unchanged {
        let old=work.annotations(&document,page)?;
        // Bound a page-local copy even when the source Annots array is shared.
        crate::composition_graph::validate_overlay_additions(&document,&document,&BTreeSet::new(),&[(old,0)],&mut ||work.step())?;
        work.reserve(old.len(),2*std::mem::size_of::<Object>())?;
        let mut native=Vec::new();let mut groups=vec![Vec::new();entries.len()];
        for (slot,value) in old.iter().enumerate() {
            work.step()?;let id=work.resolve(&document,value)?.0;
            // An original object may occur more than once around other artwork.
            // The source receipt authorizes one exact slot, not its first alias.
            if let Some((index,generated))=id.and_then(|id|selected.remove(&(Some(slot),id)).or_else(||selected.remove(&(None,id)))){
                let entry=&entries[index];
                // Replaced/deleted originals stay in the object table so unrelated
                // scripts/tags/actions retain valid references. They simply stop
                // drawing on this private selected page. Never run Save's graph GC.
                if !entry.flatten && (generated || entry.kind==Kind::Keep){
                    groups[index].push(layering::copy(value,0,work)?);
                }
            }else{native.push(layering::copy(value,0,work)?);}
        }
        require(selected.is_empty(),"read annotation or popup became detached")?;
        let mut drawings=Vec::new();let mut notes=Vec::new();let mut has_image=false;
        for overlay in plan.changes.draft {
            work.step()?;let index=*labels.get(label(overlay)).ok_or_else(||bad("read draft drawing missing"))?;
            if entries[index].flatten{continue;}
            has_image|=matches!(overlay,Overlay::Image(_));
            if matches!(overlay,Overlay::Text(_)|Overlay::Ink(_)|Overlay::Image(_)){drawings.append(&mut groups[index]);}
            else{notes.append(&mut groups[index]);}
        }
        let mut annotations=Vec::with_capacity(old.len());
        if has_image{annotations.append(&mut drawings);annotations.append(&mut native);annotations.append(&mut notes);}
        else{annotations.append(&mut native);annotations.append(&mut notes);annotations.append(&mut drawings);}
        document.get_dictionary_mut(page).map_err(bad)?.set("Annots",annotations);
    }
    work.reserve(entries.len(),128)?;
    let forms=paint_order[0].iter().filter_map(|index|entries[*index].appearance).collect::<Vec<_>>();
    if !forms.is_empty(){
        // Generated appearances already use the complete source page frame.
        // Text extraction needs their content, not newly authored tag owners.
        document=super::super::structured_overlay_flatten::append(document,&[super::super::structured_overlay_flatten::PageForms{page,forms:&forms}],&mut ||work.step())?;
    }
    pages[0].overlays=entries.iter().filter(|e|e.kind!=Kind::Delete&&!e.flatten)
        .map(|e|GeneratedOverlay{overlay_id:e.label.to_owned(),annotation_id:e.id.expect("surviving read drawing")}).collect();
    let added=entries.iter().filter(|e|e.kind==Kind::Add&&!e.flatten).map(|e|e.id.expect("added read drawing")).collect();
    crate::composition_graph::validate_retained(&document,&mut ||work.step())?;
    Ok(Published{document,pages,added:vec![added]})
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;
    #[test]
    fn private_read_deletion_uses_the_bound_slot_when_an_annotation_is_repeated(){
        let mut doc=Document::with_version("1.7");let tree=doc.new_object_id();
        let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),300.into()],"Resources"=>dictionary!{}});
        let note=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"FreeText","P"=>page});
        let native=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Square","P"=>page});
        doc.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(note),Object::Reference(native),Object::Reference(note)]);
        doc.objects.insert(tree,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into());let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree});doc.trailer.set("Root",catalog);
        let anchor=crate::composition_annotation_receipts::Anchor{page,slot:2,annotation:note};
        let receipt=Receipt{origin:anchor,output_page:page,output_slot:2,annotation:note};
        let groups=[GroupAnchors{annotation:anchor,popups:vec![]}];
        let changes=PageChanges{owned:vec![OwnedChange::Delete(AnnotationSource{page_index:0,annotation_index:2,annotation_id:Some(note),popup_indices:&[],overlay_id:"selected"})],added:vec![],draft:&[]};
        let language=crate::composition_annotation_additions::SourceLanguage::default();
        let plan=PagePlan{source_page:page,source_page_index:0,source_language:&language,changes:&changes,groups:&groups};
        let entries=vec![Entry{label:"selected",kind:Kind::Delete,root:Some(receipt),popups:vec![],id:Some(note),flatten:false,appearance:None}];
        let pages=vec![GeneratedPage{page_id:page,geometry:crate::engine::PageGeometry{left:0.,bottom:0.,right:300.,top:300.,rotation:0},unit:1.,overlays:vec![]}];
        let mut check=||Ok(());let mut work=Work{metadata:16*1024*1024,steps:1_000_000,check:&mut check};
        let result=finish(doc,pages,&[plan],&[entries],&[vec![]],&mut work).unwrap();
        let annotations=result.document.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap();
        assert_eq!(annotations,&vec![Object::Reference(note),Object::Reference(native)],"Deleting the last note must leave the first note below the native artwork");
        assert!(result.document.objects.contains_key(&note),"Private reads retain source objects and their consumers");
    }
}
