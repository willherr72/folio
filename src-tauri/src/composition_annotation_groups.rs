//! Popup slots acquire identity only inside the exact relocated source graph.
//! A group proves correspondence and links, not authorization to edit consumers.
use super::*;

pub(crate) fn bind_source_groups(doc:&Document,pages:&[ObjectId],ids:&BTreeMap<ObjectId,ObjectId>,groups:&[SourceGroup<'_>],normalized:&BTreeMap<DirectSlot,ObjectId>,metadata:usize,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<(SourceBindings,Vec<GroupAnchors>)> {
    let mut work=Work{metadata,..Work::default()};work.step(check)?;
    work.reserve(groups.len(),192)?;
    let mut count=groups.len();
    for group in groups {work.step(check)?;count=count.checked_add(group.popup_slots.len()).ok_or_else(||bad("group size overflow"))?;}
    work.reserve(count,192)?;
    // Canonical inverse lookup is bounded once per source, not a complete
    // object-map scan for each popup. Newly materialized direct slots have a
    // separate exact container/slot proof and never masquerade as source IDs.
    work.reserve(ids.len(),96)?;
    let mut inverse=BTreeMap::new();
    for (old,new) in ids {work.step(check)?;require(inverse.insert(*new,*old).is_none(),"ambiguous source relocation")?;}
    let mut slots=Vec::with_capacity(count);
    for group in groups {
        work.step(check)?;slots.push(group.annotation);
        let page=*pages.get(group.annotation.page_index).ok_or_else(||bad("group page out of range"))?;
        let raw=doc.get_dictionary(page).map_err(bad)?.get(b"Annots").map_err(bad)?;
        let (container,array)=work.resolve(doc,raw,check)?;
        let container=container.map_or(Container::Page(page),Container::Array);
        let values=array.as_array().map_err(bad)?;
        require(values.len()<=100_000,"annotation count limit")?;
        for slot in group.popup_slots {
            work.step(check)?;
            let value=values.get(*slot).ok_or_else(||bad("popup slot out of range"))?;
            let (canonical,_)=work.resolve(doc,value,check)?;
            let original=match canonical {
                None=>{require(!normalized.contains_key(&(container,*slot)),"materialized popup reverted to direct")?;None},
                Some(id) if normalized.get(&(container,*slot))==Some(&id)=>None,
                Some(id)=>Some(*inverse.get(&id).ok_or_else(||bad("popup is not from the bound source"))?),
            };
            slots.push(SourceSlot{page_index:group.annotation.page_index,slot:*slot,annotation_id:original});
        }
    }
    let mut prepared=bind_source_inner(doc,pages,ids,&slots,normalized,&mut work,check)?;
    let mut output=Vec::with_capacity(groups.len());let mut offset=0;
    let mut identities=BTreeSet::new();
    for group in groups {
        work.step(check)?;
        let root=prepared.anchors[offset];offset+=1;
        require(identities.insert((root.page,root.annotation)),"repeated group identity on one page")?;
        let list=annotations(doc,root.page,&mut work,check)?;
        let source=work.resolve(doc,&list[root.slot],check)?.1.as_dict().map_err(bad)?;
        let kind=work.resolve(doc,source.get(b"Subtype").map_err(bad)?,check)?.1.as_name().map_err(bad)?;
        require(matches!(kind,b"Text"|b"FreeText"|b"Ink"|b"Stamp"|b"Highlight"),"unsupported group root")?;
        let forward=relationship(doc,source,b"Popup",&mut work,check)?;
        let mut popups=Vec::with_capacity(group.popup_slots.len());let mut found_forward=forward.is_none();
        for _ in group.popup_slots {
            work.step(check)?;
            let popup=prepared.anchors[offset];offset+=1;
            require(identities.insert((popup.page,popup.annotation)),"repeated or shared popup identity")?;
            let dict=work.resolve(doc,&list[popup.slot],check)?.1.as_dict().map_err(bad)?;
            require(work.resolve(doc,dict.get(b"Subtype").map_err(bad)?,check)?.1.as_name().map_err(bad)?==b"Popup","group slot is not a popup")?;
            let parent=relationship(doc,dict,b"Parent",&mut work,check)?;
            require(parent==Some(root.annotation) || (parent.is_none() && forward==Some(popup.annotation)),"popup ownership differs from bound source")?;
            found_forward|=forward==Some(popup.annotation);popups.push(popup);
        }
        require(found_forward,"forward popup was omitted")?;
        output.push(GroupAnchors{annotation:root,popups});
    }
    work.step(check)?;prepared.metadata_remaining=work.metadata;
    Ok((prepared,output))
}

fn relationship(doc:&Document,dict:&lopdf::Dictionary,key:&[u8],work:&mut Work,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Option<ObjectId>> {
    let Ok(value)=dict.get(key) else{return Ok(None)};
    let (id,value)=work.resolve(doc,value,check)?;
    if matches!(value,Object::Null){return Ok(None)}
    id.map(Some).ok_or_else(||bad("popup relationship has no canonical identity"))
}
