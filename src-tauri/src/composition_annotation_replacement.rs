//! Replace trusted generated visual fields while keeping the source identity.
use super::*;
pub(crate) struct Replacement<'a> {
    pub original:Receipt,
    pub popups:&'a [Receipt],
    pub generated:ObjectId,
}
pub(crate) fn replace_owned(
    document:Document,requests:&[Replacement<'_>],check:&mut impl FnMut()->EngineResult<()>,
)->EngineResult<Document> {
    replace_with_budget(document,requests,16*1024*1024,1_000_000,check)
}

/// Both roots are already attached to the private composed document. Originals
/// require proved import/copy provenance; fresh roots come from our renderer.
/// Only drawing fields move. Authored identity, popups and logical owners stay.
fn replace_with_budget(mut document:Document,requests:&[Replacement<'_>],metadata:usize,steps:usize,
 check:&mut impl FnMut()->EngineResult<()>) -> EngineResult<Document> {
 check()?;
 if requests.is_empty(){return Ok(document)}
 let catalog=document.trailer.get(b"Root").and_then(Object::as_reference).map_err(bad)?;
 let pages=crate::composition_pages::inspect(&document,catalog,check)?.pages;
 let structure=crate::composition_structure::inspect(&document,catalog,&pages,check)?;
 let mut work=Walk{doc:&document,structure:&structure,catalog,check,metadata,steps,pages:BTreeSet::new(),targets:BTreeMap::new(),tag_parents:BTreeMap::new(),memberships:BTreeSet::new(),seen:BTreeSet::new()};
 work.reserve(pages.len(),64)?;work.pages.extend(pages.iter().copied());
 work.reserve(requests.len(),512)?;
 let mut wanted=BTreeMap::new();let mut originals=BTreeSet::new();
 for request in requests {
  work.step(0)?;
  require(originals.insert(request.original.annotation),"repeated replacement original")?;
  require(wanted.insert(request.generated,(request.original.output_page,None)).is_none(),"repeated generated replacement")?;
 }
 require(originals.iter().all(|id|!wanted.contains_key(id)),"replacement identities overlap")?;
 // One bounded scan, not a scan of all annotations for each request.
 for page in &pages {
  for (slot,value) in work.annotations(*page)?.iter().enumerate(){
   let (id,_)=work.resolve(value)?;
   if let Some((expected,found))=id.and_then(|id|wanted.get_mut(&id)) {
    require(*expected==*page && found.is_none(),"generated replacement is shared or on another page")?;
    *found=Some(slot);
   }
  }
 }
 let mut proofs=Vec::with_capacity(requests.len()*2);
 let mut discarded=BTreeMap::<ObjectId,BTreeSet<usize>>::new();
 for request in requests {
  work.step(0)?;
  let slot=wanted[&request.generated].1.ok_or_else(||bad("generated replacement is not attached"))?;
  work.reserve(1,128)?;discarded.entry(request.original.output_page).or_default().insert(slot);
  proofs.push(Removal{annotation:request.original,popups:request.popups});
  proofs.push(Removal{annotation:Receipt{annotation:request.generated,output_slot:slot,..request.original},popups:&[]});
 }
 let (_,retained)=work.prepare(&proofs,&pages)?;
 require(retained.is_empty(),"shared source replacement requires occurrence isolation")?;
 for (_,value) in document.trailer.iter(){work.visit(value,Role::Unknown,0)?;}
 for request in requests {
  let source=document.get_dictionary(request.original.annotation).map_err(bad)?;
  let generated=document.get_dictionary(request.generated).map_err(bad)?;
  admit_fields(&mut work,source,generated)?;
  // Changes may stale descriptions or layout attributes. Refuse these until
  // there is a typed semantic adapter; never silently rewrite authored tags.
  let mut parent=work.tag_parents.get(&request.original.annotation).copied();
  while let Some(id)=parent {
   work.step(0)?;
   if Some(id)==structure.root{break}
   let dict=document.get_dictionary(id).map_err(bad)?;
   for key in [b"Alt".as_slice(),b"ActualText",b"E",b"A",b"C"]{absent(&mut work,dict,key)?;}
   parent=work.optional_ref(dict,b"P")?;
  }
 }
 work.reserve(discarded.len(),128)?;
 let mut lists=Vec::with_capacity(discarded.len());
 for page in discarded.keys(){lists.push((work.annotations(*page)?,0));}
 crate::composition_graph::validate_overlay_additions(&document,&document,&BTreeSet::new(),&lists,&mut ||(work.check)())?;
 let mut patches=Vec::with_capacity(discarded.len());
 for (page,slots) in discarded {
  let old=work.annotations(page)?;
  work.reserve(old.len(),std::mem::size_of::<Object>())?;
  let mut kept=Vec::with_capacity(old.len()-slots.len());
  for (slot,value) in old.iter().enumerate(){work.step(0)?;if !slots.contains(&slot){kept.push(work.copy(value,0)?);}}
  patches.push((page,kept));
 }
 work.reserve(wanted.len(),64)?;
 let forbidden=wanted.keys().copied().collect();
 let mut metadata=work.metadata;let mut steps=work.steps;drop(work);
 for request in requests {
  check()?;
  // Move AP/resources/portable records. A null placeholder keeps unreachable
  // historical arrays well formed until final reachability pruning.
  let Object::Dictionary(mut generated)=document.objects.insert(request.generated,Object::Null).ok_or_else(||bad("missing generated root"))? else{return Err(bad("invalid generated root"))};
  let target=document.get_dictionary_mut(request.original.annotation).map_err(bad)?;
  for key in DRAWING_FIELDS {
   target.remove(key);
   if let Some(value)=generated.remove(key){target.set(key.to_vec(),value);}
  }
 }
 for (page,values) in patches {check()?;document.get_dictionary_mut(page).map_err(bad)?.set("Annots",values);}
 crate::composition_projection::validate_removed_owners(&document,&forbidden,&mut metadata,&mut steps,check)?;
 require(crate::composition_structure::inspect(&document,catalog,&pages,check)?==structure,"replacement changed structure ownership")?;
 crate::composition_graph::validate_retained(&document,check)?;
 check()?;Ok(document)
}

const DRAWING_FIELDS:&[&[u8]]=&[b"Rect",b"AP",b"C",b"CA",b"Contents",b"DA",b"Q",b"InkList",b"BS",b"Border",b"Name",b"QuadPoints",b"Folio",b"FolioImageAsset"];
fn absent(work:&mut Walk<'_,'_>,dict:&Dictionary,key:&[u8])->EngineResult<()> {
 if let Ok(value)=dict.get(key){require(matches!(work.resolve(value)?.1,Object::Null),"unsupported annotation presentation or semantic relationship")?;}
 Ok(())
}
fn admit_fields(work:&mut Walk<'_,'_>,source:&Dictionary,generated:&Dictionary)->EngineResult<()> {
 let kind=work.resolve(source.get(b"Subtype").map_err(bad)?)?.1.as_name().map_err(bad)?;
 require(matches!(kind,b"FreeText"|b"Ink"|b"Stamp"|b"Highlight"),"annotation subtype has no replacement adapter")?;
 require(work.resolve(generated.get(b"Subtype").map_err(bad)?)?.1.as_name().map_err(bad)?==kind,"replacement subtype differs")?;
 for dict in [source,generated] {
  for key in [b"AS".as_slice(),b"RC",b"DS",b"CL",b"RD",b"IT",b"LE",b"A",b"AA",b"OC",b"IRT",b"RT",b"State",b"StateModel"]{absent(work,dict,key)?;}
  if let Ok(ap)=dict.get(b"AP") {
   let ap=work.resolve(ap)?.1.as_dict().map_err(bad)?;
   require(ap.len()==1 && ap.has(b"N"),"alternate appearance needs a typed replacement adapter")?;
   require(matches!(work.resolve(ap.get(b"N").map_err(bad)?)?.1,Object::Stream(_)),"state appearance needs a typed replacement adapter")?;
  }
 }
 require(generated.has(b"AP") && generated.has(b"Rect"),"generated drawing is incomplete")?;
 let flags=|work:&mut Walk<'_,'_>,dict:&Dictionary|->EngineResult<i64>{match dict.get(b"F"){Ok(v)=>work.resolve(v)?.1.as_i64().map_err(bad),Err(_)=>Ok(0)}};
 require(flags(work,source)?==flags(work,generated)?,"replacement would change annotation flags")?;
 for (key,_) in generated.iter() {
  work.step(0)?;
  require(DRAWING_FIELDS.contains(&key.as_slice()) || matches!(key.as_slice(),b"Type"|b"Subtype"|b"P"|b"F"|b"NM"|b"T"|b"M"|b"CreationDate"),"unexpected generated annotation field")?;
 }
 for key in DRAWING_FIELDS {work.reserve(key.len()+96,1)?;}
 Ok(())
}

#[cfg(test)]
#[path="composition_annotation_replacement_tests.rs"]
mod tests;
