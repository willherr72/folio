//! Final paint order, after every operation that consumes original slot receipts.
//! This changes page-local Annots lists, never annotation or authored tag order.
use super::*;

pub(super) fn finish(mut document:Document,pages:&[GeneratedPage],plans:&[PagePlan<'_,'_,'_>],entries:&[Vec<Entry<'_>>],flattened:bool,work:&mut Work<'_>)->EngineResult<Document> {
    work.reserve(pages.len(),128)?;let mut orders=Vec::new();
    for ((page,plan),entries) in pages.iter().zip(plans).zip(entries) {
        work.step()?;
        let draft=plan.changes.draft;
        let mut removed=0usize;
        for entry in entries {work.step()?;if flattened&&entry.flatten&&entry.kind!=Kind::Delete{removed+=1;}}
        require(page.overlays.len().checked_add(removed)==Some(draft.len()),"complete layering draft differs")?;
        // A genuinely unchanged draft keeps the source's exact authored list,
        // even if that producer chose a different stacking convention. A pure
        // workspace reorder is still a requested change and is handled below.
        if removed==0 && entries.iter().all(|e|e.kind==Kind::Keep) && draft.iter().map(label).eq(entries.iter().map(|e|e.label)){continue}
        let old=work.annotations(&document,page.page_id)?;
        work.reserve(old.len(),256)?;work.reserve(entries.len(),256)?;
        let mut selected=BTreeMap::new();let mut labels=BTreeMap::new();
        for (index,entry) in entries.iter().enumerate() {
            work.step()?;if entry.kind==Kind::Delete {continue}
            // Validate the complete borrowed draft, including consumed drawings,
            // but require membership only for annotations that still exist.
            require(labels.insert(entry.label,index).is_none(),"ambiguous layering label")?;
            if flattened&&entry.flatten{continue}
            let id=if entry.kind==Kind::Replace {entry.root.expect("owned replacement").annotation}else{entry.id.expect("surviving root")};
            require(selected.insert(id,index).is_none(),"ambiguous layering root")?;
            for popup in &entry.popups {work.step()?;work.reserve(1,96)?;require(selected.insert(popup.annotation,index).is_none(),"ambiguous layering popup")?;}
        }
        let mut needed=selected.keys().copied().collect::<BTreeSet<_>>();
        let mut groups=vec![Vec::new();entries.len()];let mut native=Vec::new();
        for (slot,value) in old.iter().enumerate() {
            work.step()?;let (id,_)=work.resolve(&document,value)?;
            if let Some((id,index))=id.and_then(|id|selected.get(&id).map(|index|(id,*index))) {require(needed.remove(&id),"layering annotation is repeated")?;groups[index].push(slot);}
            else {native.push(slot);}
        }
        require(needed.is_empty(),"layering annotation or popup is detached")?;
        let mut drawings=Vec::new();let mut notes=Vec::new();let mut has_image=false;
        for overlay in draft {
            work.step()?;let index=labels.remove(label(overlay)).ok_or_else(||bad("layering draft identity differs"))?;
            let bound=if let Some(change)=plan.changes.owned.get(index){match change{OwnedChange::Keep(_,o)|OwnedChange::Replace(_,o)=>Some(*o),OwnedChange::Delete(_)=>None}}
                else{plan.changes.added.get(index-plan.changes.owned.len()).copied()};
            require(bound.is_some_and(|v|std::ptr::eq(v,overlay)),"layering belongs to another draft")?;
            if flattened&&entries[index].flatten{continue}
            let drawing=matches!(overlay,Overlay::Text(_)|Overlay::Ink(_)|Overlay::Image(_));
            has_image|=matches!(overlay,Overlay::Image(_));
            if drawing {drawings.append(&mut groups[index]);}else{notes.append(&mut groups[index]);}
        }
        require(labels.is_empty(),"incomplete layering draft")?;
        let mut order=Vec::with_capacity(old.len());
        // Match persistence::finalize and render_page_plan. Preserve the
        // relative order of every native annotation and every draft category.
        if has_image {order.append(&mut drawings);order.append(&mut native);order.append(&mut notes);}
        else {order.append(&mut native);order.append(&mut notes);order.append(&mut drawings);}
        require(order.len()==old.len(),"layering membership differs")?;
        if !order.iter().copied().eq(0..old.len()){orders.push((page.page_id,order));}
    }
    if orders.is_empty(){return Ok(document)}
    // Shared indirect arrays and direct metadata may require a local copy.
    // Reserve the aggregate graph expansion before copying any payload.
    let mut lists=Vec::with_capacity(orders.len());
    for (page,_) in &orders{lists.push((work.annotations(&document,*page)?,0));}
    crate::composition_graph::validate_overlay_additions(&document,&document,&BTreeSet::new(),&lists,&mut ||work.step())?;
    let mut patches=Vec::with_capacity(orders.len());
    for (page,order) in orders {
        let old=work.annotations(&document,page)?;let mut values=Vec::with_capacity(order.len());
        for slot in order {values.push(copy(&old[slot],0,work)?);}
        patches.push((page,values));
    }
    for (page,values) in patches {work.step()?;document.get_dictionary_mut(page).map_err(bad)?.set("Annots",values);}
    work.step()?;Ok(document)
}

pub(super) fn copy(value:&Object,depth:usize,work:&mut Work<'_>)->EngineResult<Object> {
    work.step()?;require(depth<=64,"layering metadata depth")?;work.reserve(1,std::mem::size_of::<Object>())?;
    match value {
        Object::String(bytes,format)=>{work.reserve(bytes.len(),1)?;Ok(Object::String(bytes.clone(),*format))},
        Object::Name(bytes)=>{work.reserve(bytes.len(),1)?;Ok(Object::Name(bytes.clone()))},
        Object::Array(values)=>{work.reserve(values.len(),std::mem::size_of::<Object>())?;let mut copied=Vec::with_capacity(values.len());for value in values{copied.push(copy(value,depth+1,work)?);}Ok(Object::Array(copied))},
        Object::Dictionary(dict)=>{let mut copied=lopdf::Dictionary::new();for (key,value) in dict.iter(){work.reserve(key.len()+96,1)?;copied.set(key.clone(),copy(value,depth+1,work)?);}Ok(Object::Dictionary(copied))},
        Object::Stream(_)=>Err(bad("direct stream metadata cannot be reordered")),
        _=>Ok(value.clone()),
    }
}
