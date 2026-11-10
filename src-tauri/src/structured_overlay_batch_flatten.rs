//! Final flatten stage for already-proved mixed annotation transactions.
use super::*;
use crate::composition_annotation_receipts::Anchor;
use crate::composition_annotation_removal::{Removal,remove_owned};

fn slots(doc:&Document,pages:&[ObjectId],wanted:&BTreeSet<(ObjectId,ObjectId)>,work:&mut Work<'_>)->EngineResult<BTreeMap<(ObjectId,ObjectId),usize>>{
    work.reserve(wanted.len(),128)?;let mut found=BTreeMap::new();
    for page in pages{for (slot,value) in work.annotations(doc,*page)?.iter().enumerate(){
        work.step()?;if let Some(id)=work.resolve(doc,value)?.0{let key=(*page,id);if wanted.contains(&key){require(found.insert(key,slot).is_none(),"flattened identity occurs twice")?;}}
    }}
    require(found.len()==wanted.len(),"flattened annotation or popup became detached")?;Ok(found)
}
fn receipt(page:ObjectId,annotation:ObjectId,slot:usize,old:Option<Receipt>)->Receipt{
    Receipt{origin:old.map_or(Anchor{page,annotation,slot},|r|r.origin),output_page:page,annotation,output_slot:slot}
}

pub(super) fn discard_temporary(doc:Document,temporary:&[(ObjectId,ObjectId)],work:&mut Work<'_>)->EngineResult<Document>{
    if temporary.is_empty(){return Ok(doc);}
    work.reserve(temporary.len(),256)?;
    let wanted=temporary.iter().copied().collect::<BTreeSet<_>>();require(wanted.len()==temporary.len(),"duplicate temporary flatten root")?;
    let pages=temporary.iter().map(|(page,_)|*page).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
    let found=slots(&doc,&pages,&wanted,work)?;
    let removals=temporary.iter().map(|(page,id)|Removal{annotation:receipt(*page,*id,found[&(*page,*id)],None),popups:&[]}).collect::<Vec<_>>();
    remove_owned(doc,&removals,&mut ||work.step())
}

pub(super) fn finish(mut doc:Document,pages:&mut [GeneratedPage],added:&mut [Vec<ObjectId>],plans:&[PagePlan<'_,'_,'_>],entries:&[Vec<Entry<'_>>],order:&[Vec<usize>],work:&mut Work<'_>)->EngineResult<Document>{
    use crate::composition_annotation_additions::{NewOwners,FlattenOwner,mark_flattened};
    let selected=order.iter().try_fold(0usize,|n,p|n.checked_add(p.len()).ok_or_else(||bad("flatten count overflow")))?;
    let count=entries.iter().flatten().filter(|entry|entry.flatten).count();
    work.reserve(selected,512)?;work.reserve(pages.len(),128)?;
    let mut wanted=BTreeSet::new();let mut roots=Vec::with_capacity(count);let mut flattened=BTreeSet::new();let mut unowned=Vec::with_capacity(pages.len());let mut draw_counts=Vec::with_capacity(pages.len());
    for (index,page) in pages.iter().enumerate(){
        work.step()?;let mut needs_owner=Vec::new();let previous=roots.len();
        for position in &order[index]{
            work.step()?;let entry=&entries[index][*position];require(entry.kind!=Kind::Delete,"invalid flattened intent")?;
            let id=if entry.kind==Kind::Replace{entry.root.ok_or_else(||bad("replacement root missing"))?.annotation}else{entry.id.ok_or_else(||bad("flatten root missing"))?};
            let dict=doc.get_dictionary(id).map_err(bad)?;
            let has_owner=match dict.get(b"StructParent"){Ok(value)=>!matches!(work.resolve(&doc,value)?.1,Object::Null),Err(_)=>false};
            // New additions and previously unowned flattened drawings receive
            // owners together, in the exact prepared page/draft order. Existing
            // authored owners retain their positions and are never re-appended.
            if entry.kind==Kind::Add||(entry.flatten&&!has_owner){needs_owner.push(id);}
            if !entry.flatten{continue}
            require(wanted.insert((page.page_id,id))&&flattened.insert(id),"flatten root is shared")?;
            work.reserve(entry.popups.len(),192)?;
            for popup in &entry.popups{work.step()?;require(popup.output_page==page.page_id&&wanted.insert((page.page_id,popup.annotation)),"flatten popup is shared or foreign")?;}
            roots.push((index,*position,id));
        }
        unowned.push(needs_owner);draw_counts.push(roots.len()-previous);
    }
    let page_ids=pages.iter().map(|p|p.page_id).collect::<Vec<_>>();let found=slots(&doc,&page_ids,&wanted,work)?;
    let new_owners=pages.iter().zip(&unowned).zip(plans).map(|((page,annotations),plan)|NewOwners{page:page.page_id,annotations,language:plan.source_language}).collect::<Vec<_>>();
    doc=crate::composition_annotation_additions::append(doc,&new_owners,&mut ||work.step())?;
    if count==0{work.step()?;return Ok(doc);}
    let requests=roots.iter().map(|(page,position,id)|{
        let entry=&entries[*page][*position];Ok(FlattenOwner{page:pages[*page].page_id,annotation:*id,appearance:entry.appearance.ok_or_else(||bad("flattened appearance missing"))?})
    }).collect::<EngineResult<Vec<_>>>()?;
    let marked=mark_flattened(doc,&requests,&mut ||work.step())?;
    let mut at=0usize;let mut page_forms=Vec::with_capacity(pages.len());
    for (page,count) in pages.iter().zip(draw_counts){work.step()?;if count>0{page_forms.push(super::super::structured_overlay_flatten::PageForms{page:page.page_id,forms:&marked.forms[at..at+count]});}at+=count;}
    require(at==marked.forms.len(),"flattened wrapper count differs")?;
    doc=super::super::structured_overlay_flatten::append(marked.document,&page_forms,&mut ||work.step())?;
    let mut popup_receipts=Vec::with_capacity(roots.len());let mut root_receipts=Vec::with_capacity(roots.len());
    for (index,position,id) in &roots{
        work.step()?;let page=pages[*index].page_id;let entry=&entries[*index][*position];
        root_receipts.push(receipt(page,*id,found[&(page,*id)],entry.root));
        let mut popups=Vec::with_capacity(entry.popups.len());for popup in &entry.popups{work.step()?;popups.push(receipt(page,popup.annotation,found[&(page,popup.annotation)],Some(*popup)));}popup_receipts.push(popups);
    }
    let removals=root_receipts.iter().zip(&popup_receipts).map(|(root,popups)|Removal{annotation:*root,popups}).collect::<Vec<_>>();
    doc=remove_owned(doc,&removals,&mut ||work.step())?;
    for (page,added) in pages.iter_mut().zip(added){work.step()?;page.overlays.retain(|o|!flattened.contains(&o.annotation_id));added.retain(|id|!flattened.contains(id));}
    work.step()?;Ok(doc)
}
