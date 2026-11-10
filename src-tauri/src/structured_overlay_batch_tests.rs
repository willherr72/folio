use super::*;
use super::super::{structured_overlay_render::tests as fixtures,structured_overlay_publish::attach_generated,imported_overlay_sources::publication::OwnedChange};
use lopdf::{dictionary,Object};

#[path="structured_overlay_layering_tests.rs"]
mod layering;

#[test]
fn structured_overlay_batch_traversal_allowance_covers_nested_publication_work() {
    let mut rt=fixtures::runtime();let info=fixtures::source(&mut rt,0,1.,b"q 0.2 0.4 0.8 rg 160 300 20 20 re f Q");
    let original=rt.document(&info.id).unwrap().bytes().unwrap();
    let doc=crate::pdf_output::load(&original).unwrap();let page=doc.get_pages()[&1];
    let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let language=crate::composition_annotation_additions::SourceLanguage::capture(&doc,catalog).unwrap();
    for flatten in [false,true] {
        let mut request=fixtures::request(&rt,&info);request.flatten=flatten;
        let changes=super::super::imported_overlay_sources::publication::PageChanges{draft:&request.pages[0].overlays,owned:vec![],added:request.pages[0].overlays.iter().collect()};
        let prepare=||rt.prepare_structured_overlay_changes(&request,&[&changes],&mut ||Ok(())).unwrap();
        let prepared=prepare();let targets=[TargetPage{page_id:page,geometry:prepared.pages[0].geometry,unit:1.}];
        let plans=[PagePlan{source_page:page,source_page_index:0,source_language:&language,changes:&changes,groups:&[]}];
        // Count actual nested cancellation checkpoints, including attachment,
        // semantic owners, typed removal, layering and retained-graph checks.
        let mut calls=0;
        let baseline=publish_mode(doc.clone(),&targets,&plans,&[],prepared,flatten,false,16*1024*1024,1_000_000,&mut ||{calls+=1;Ok(())}).unwrap();
        assert!(calls>100,"fixture must exercise nested graph work");
        let exact=publish_mode(doc.clone(),&targets,&plans,&[],prepare(),flatten,false,16*1024*1024,calls,&mut ||Ok(())).unwrap();
        assert_eq!(exact.document.objects,baseline.document.objects);assert_eq!(exact.document.trailer,baseline.document.trailer);
        let mut seen=0;
        let insufficient=publish_mode(doc.clone(),&targets,&plans,&[],prepare(),flatten,false,16*1024*1024,calls-1,&mut ||{seen+=1;Ok(())});
        assert!(matches!(insufficient,Err(EngineError::InvalidRequest(ref message)) if message.contains("Structured overlay transaction: traversal limit")),"nested work must consume the same batch allowance, flatten={flatten}; observed {calls} checkpoints");
        assert!(seen<=calls,"refusal must occur at the exhausted boundary");
        assert_eq!(rt.document(&info.id).unwrap().bytes().unwrap().as_ref(),original.as_ref());
    }
}

#[test]
fn structured_overlay_batch_repeated_edits_prune_obsolete_appearances_and_keep_other_copy() {
    let mut rt=fixtures::runtime();let seed=fixtures::source(&mut rt,0,1.7,b"q 0.2 0.4 0.8 rg 160 300 20 20 re f Q");
    let mut input=crate::pdf_output::load(rt.document(&seed.id).unwrap().bytes().unwrap().as_ref()).unwrap();input.catalog_mut().unwrap().remove(b"SourceOnly");
    let original=crate::pdf_output::save(&mut input).unwrap();let path=std::env::temp_dir().join(format!("folio-cleanup-source-{}.pdf",Uuid::new_v4()));
    let first=rt.open_bytes(original.clone(),path.clone(),path,true,true,false,OpenSecurity::Restored(None)).unwrap();
    let mut request=fixtures::request(&rt,&first);let mut second=request.pages[0].clone();second.id="other-copy".into();request.pages.push(second);
    let original_id=first.id.clone();let mut info=first;let mut baseline:Option<(u32,u32,Vec<u8>)>=None;let mut sizes=vec![];let mut counts=vec![];
    for round in 0..7 {
        if round>0 {
            for (index,page) in request.pages.iter_mut().enumerate(){page.source_id=info.id.clone();page.page_index=index;page.overlays=info.pages[index].overlays.clone();}
            let text=request.pages[1].overlays.iter_mut().find_map(|o|if let Overlay::Text(t)=o{Some(t)}else{None}).unwrap();text.text=format!("Edit {round:02}");
        }
        let result=layering::transaction(&rt,&info,&request);let before_count=result.document.objects.len();
        let mut doc=crate::composition_projection::prune_final(result.document,&mut ||Ok(())).unwrap();
        if round>1 {assert!(doc.objects.len()<before_count,"replaced appearance graph must become unreachable");}
        counts.push(doc.objects.len());let saved=crate::pdf_output::save(&mut doc).unwrap();sizes.push(saved.len());
        let pixels=fixtures::pixels(&rt,&saved,0);if let Some(baseline)=&baseline{assert_eq!((pixels.0,pixels.1),(baseline.0,baseline.1));assert_eq!(pixels.2.iter().zip(&baseline.2).filter(|(a,b)|a!=b).count(),0,"unchanged copy differs");}else{baseline=Some(pixels);}
        let reference=rt.compose_imported_pdf(&request,&[],true,CompositionIntent::Preview,None).unwrap();
        for page in [0,1]{let a=fixtures::pixels(&rt,&saved,page);let b=fixtures::pixels(&rt,&reference,page);assert_eq!(a.2.iter().zip(&b.2).filter(|(a,b)|a!=b).count(),0,"cleanup round {round} page {page}");}
        if let Some(root)=std::env::var_os("FOLIO_STRUCTURED_OVERLAY_CAPTURE_DIR") {use std::io::Write;let root=PathBuf::from(root);std::fs::create_dir_all(&root).unwrap();for (kind,bytes) in [("actual",saved.as_slice()),("reference",reference.as_slice())]{std::fs::OpenOptions::new().write(true).create_new(true).open(root.join(format!("cleanup-round-{round}-{kind}.pdf"))).unwrap().write_all(bytes).unwrap();}}
        let path=std::env::temp_dir().join(format!("folio-cleanup-round-{}.pdf",Uuid::new_v4()));info=rt.open_bytes(saved,path.clone(),path,true,true,false,OpenSecurity::Restored(None)).unwrap();
        assert_eq!(info.pages.len(),2);for page in &info.pages{assert_eq!(page.overlays.len(),4);}
        if round>0{assert!(info.pages[1].overlays.iter().any(|o|matches!(o,Overlay::Text(t) if t.text==format!("Edit {round:02}"))));}
    }
    assert!(counts[2..].iter().all(|n|*n==counts[2]),"object count grows after warmup: {counts:?}");
    assert!(sizes[2..].iter().all(|n|n.abs_diff(sizes[2])<=128),"serialized size grows after warmup: {sizes:?}");
    assert_eq!(rt.document(&original_id).unwrap().bytes().unwrap().as_ref(),original.as_slice());
}

#[test]
fn structured_overlay_batch_new_owners_use_original_mixed_source_languages() {
    let mut rt=fixtures::runtime();let mut infos=Vec::new();
    for (index,language) in [Some("en-US"),Some("fr-CA"),None].into_iter().enumerate() {
        let content=if index==0 {b"/P <</MCID 0>> BDC q 0.2 0.4 0.8 rg 160 300 20 20 re f Q EMC".as_slice()} else {b"q 0.2 0.4 0.8 rg 160 300 20 20 re f Q"};
        let info=fixtures::source(&mut rt,0,1.,content);
        let mut doc=crate::pdf_output::load(rt.document(&info.id).unwrap().bytes().unwrap().as_ref()).unwrap();
        doc.catalog_mut().unwrap().remove(b"SourceOnly");
        if let Some(language)=language {doc.catalog_mut().unwrap().set("Lang",Object::string_literal(language));}
        if index==0 {
            let page=doc.get_pages()[&1];let root=doc.new_object_id();
            let element=doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"P","P"=>root,"Pg"=>page,"K"=>0,"Alt"=>Object::string_literal("Original marked artwork")});
            let tree=doc.add_object(dictionary!{"Nums"=>vec![7.into(),Object::Array(vec![element.into(),Object::Null])]});
            doc.objects.insert(root,dictionary!{"Type"=>"StructTreeRoot","K"=>element,"ParentTree"=>tree,"ParentTreeNextKey"=>9}.into());
            doc.get_dictionary_mut(page).unwrap().set("StructParents",7);
            doc.catalog_mut().unwrap().set("StructTreeRoot",root);doc.catalog_mut().unwrap().set("MarkInfo",dictionary!{"Marked"=>true});
        }
        let bytes=crate::pdf_output::save(&mut doc).unwrap();let path=std::env::temp_dir().join(format!("folio-language-source-{}.pdf",Uuid::new_v4()));
        infos.push(rt.open_bytes(bytes,path.clone(),path,true,true,false,OpenSecurity::Restored(None)).unwrap());
    }
    let order=[2,1,1,0];let mut request=fixtures::request(&rt,&infos[0]);request.pages.clear();
    for (index,source) in order.iter().enumerate(){let mut page=fixtures::request(&rt,&infos[*source]).pages.remove(0);page.id=format!("occurrence-{index}");request.pages.push(page);}
    let mut graph=crate::composition_graph::CompositionGraph::new();let mut origins=vec![];let mut publications=vec![];
    for (index,info) in infos.iter().enumerate(){let source=rt.document(&info.id).unwrap();graph.import_bytes(index,source.bytes().unwrap().as_ref(),Arc::new(||false)).unwrap();origins.push(graph.source_pages(index,&mut ||Ok(())).unwrap()[0]);publications.push(source.imported_overlay_sources.publication(source.source_bytes.identity(),1,&mut ||Ok(())).unwrap());}
    let mut changes=vec![];for (page,source) in request.pages.iter().zip(order){changes.push(publications[source].page(0,&page.overlays,&mut ||Ok(())).unwrap());}
    let languages=graph.source_languages(&mut ||Ok(())).unwrap();let doc=graph.assemble(&[0,1,2],&mut ||Ok(())).unwrap();let selected=order.map(|source|origins[source]);
    let (doc,pages,receipts)=crate::composition_occurrences::project_forms_tracked(doc,&selected,&[],&mut ||Ok(())).unwrap();assert!(receipts.is_empty());
    let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();let before=crate::composition_structure::inspect(&doc,catalog,&pages,&mut ||Ok(())).unwrap();
    assert!(!before.elements.is_empty());let old_elements=before.elements.iter().map(|id|(*id,doc.objects[id].clone())).collect::<BTreeMap<_,_>>();
    let refs=changes.iter().collect::<Vec<_>>();let prepared=rt.prepare_structured_overlay_changes(&request,&refs,&mut ||Ok(())).unwrap();
    let targets=prepared.pages.iter().zip(&pages).map(|(page,id)|TargetPage{page_id:*id,geometry:page.geometry,unit:page.unit}).collect::<Vec<_>>();
    let plans=changes.iter().zip(order).map(|(changes,source)|PagePlan{source_page:origins[source],source_page_index:0,source_language:&languages[&source],changes,groups:&[]}).collect::<Vec<_>>();
    let reference_stage=rt.prepare_structured_overlay_changes(&request,&refs,&mut ||Ok(())).unwrap();
    let mut reference=attach_generated(doc.clone(),&targets,reference_stage,&mut ||Ok(())).unwrap().document;
    let reference=crate::pdf_output::save(&mut reference).unwrap();
    let mut result=publish(doc,&targets,&plans,&receipts,prepared,&mut ||Ok(())).unwrap();
    let after=crate::composition_structure::inspect(&result.document,catalog,&pages,&mut ||Ok(())).unwrap();
    assert_eq!(after.top[..before.top.len()],before.top);for (id,value) in old_elements{assert_eq!(result.document.objects[&id],value);}
    for (key,owner) in &before.owners{assert_eq!(&after.owners[key],owner);}
    for (index,expected) in ["","fr-CA","fr-CA","en-US"].into_iter().enumerate(){assert_eq!(result.added[index].len(),5);
        for id in &result.added[index]{
            let annotation=result.document.get_dictionary(*id).unwrap();
            assert!(annotation.has(b"StructParent"),"new annotation must receive its own structure owner");
            let key=annotation.get(b"StructParent").unwrap().as_i64().unwrap();let owner=&after.owners[&key];assert_eq!(owner.object,*id);
            let crate::composition_structure::Parents::Whole(parent)=owner.parents else{panic!("whole annotation owner required")};
            let parent=result.document.get_dictionary(parent).unwrap();assert_eq!(parent.get(b"Lang").unwrap().as_str().unwrap(),expected.as_bytes());assert_eq!(parent.get(b"Pg").unwrap().as_reference().unwrap(),pages[index]);
        }
    }
    let saved=crate::pdf_output::save(&mut result.document).unwrap();
    for page in 0..4{assert_eq!(fixtures::pixels(&rt,&saved,page),fixtures::pixels(&rt,&reference,page));}
    capture_language("reference",&reference);capture_language("after",&saved);
    let path=std::env::temp_dir().join(format!("folio-language-output-{}.pdf",Uuid::new_v4()));let reopened=rt.open_bytes(saved.clone(),path.clone(),path,true,true,false,OpenSecurity::Restored(None)).unwrap();
    assert_eq!(reopened.pages.len(),4);for page in &reopened.pages{assert_eq!(page.overlays.len(),4,"tagged drawings reopen; native comments stay native");}
    // Reopen the combined PDF, then change only the first French copy. The
    // leading catalog is English, but existing annotation owners stay French.
    let mut edit=fixtures::request(&rt,&reopened);let template=edit.pages.remove(0);
    for (index,page) in reopened.pages.iter().enumerate(){let mut plan=template.clone();plan.id=format!("reedit-{index}");plan.page_index=index;plan.width=page.width;plan.height=page.height;plan.overlays=page.overlays.clone();edit.pages.push(plan);}
    let text=edit.pages[1].overlays.iter_mut().find_map(|o|if let Overlay::Text(t)=o{Some(t)}else{None}).unwrap();let old_text=text.text.clone();text.text.push_str(" edited");let expected=text.text.clone();
    let source=rt.document(&reopened.id).unwrap();let mut publication=source.imported_overlay_sources.publication(source.source_bytes.identity(),4,&mut ||Ok(())).unwrap();
    let mut graph=crate::composition_graph::CompositionGraph::new();graph.import_bytes(0,source.bytes().unwrap().as_ref(),Arc::new(||false)).unwrap();
    let origins=graph.source_pages(0,&mut ||Ok(())).unwrap();let languages=graph.source_languages(&mut ||Ok(())).unwrap();let mut changes=vec![];let mut groups=vec![];
    for (index,page) in edit.pages.iter().enumerate(){
        let change=publication.page(index,&page.overlays,&mut ||Ok(())).unwrap();
        let slots=change.owned.iter().map(|c|match c{OwnedChange::Keep(s,_)|OwnedChange::Replace(s,_)|OwnedChange::Delete(s)=>s.source_group()}).collect::<Vec<_>>();
        groups.push(graph.source_annotation_groups(0,&slots,&mut ||Ok(())).unwrap());changes.push(change);
    }
    let anchors=groups.iter().flatten().flat_map(|g|std::iter::once(g.annotation).chain(g.popups.iter().copied())).collect::<Vec<_>>();
    let doc=graph.assemble(&[0],&mut ||Ok(())).unwrap();let (doc,pages,receipts)=crate::composition_occurrences::project_forms_tracked(doc,&origins,&anchors,&mut ||Ok(())).unwrap();
    let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();let structure=crate::composition_structure::inspect(&doc,catalog,&pages,&mut ||Ok(())).unwrap();
    let before=crate::pdf_output::save(&mut doc.clone()).unwrap();
    let refs=changes.iter().collect::<Vec<_>>();let prepared=rt.prepare_structured_overlay_changes(&edit,&refs,&mut ||Ok(())).unwrap();assert_eq!(prepared.pages.iter().map(|p|p.overlays.len()).collect::<Vec<_>>(),vec![0,1,0,0]);
    let targets=prepared.pages.iter().zip(&pages).map(|(page,id)|TargetPage{page_id:*id,geometry:page.geometry,unit:page.unit}).collect::<Vec<_>>();
    let plans=changes.iter().enumerate().map(|(index,change)|PagePlan{source_page:origins[index],source_page_index:index,source_language:&languages[&0],changes:change,groups:&groups[index]}).collect::<Vec<_>>();
    let mut result=publish(doc,&targets,&plans,&receipts,prepared,&mut ||Ok(())).unwrap();assert!(result.added.iter().all(Vec::is_empty));
    assert_eq!(crate::composition_structure::inspect(&result.document,catalog,&pages,&mut ||Ok(())).unwrap(),structure);
    let output=crate::pdf_output::save(&mut result.document).unwrap();assert_eq!(source.bytes().unwrap().as_ref(),saved.as_slice());
    capture_language("reedit-before",&before);capture_language("reedit-after",&output);
    let path=std::env::temp_dir().join(format!("folio-language-reedit-{}.pdf",Uuid::new_v4()));let again=rt.open_bytes(output,path.clone(),path,true,true,false,OpenSecurity::Restored(None)).unwrap();
    for (index,page) in again.pages.iter().enumerate(){assert_eq!(page.overlays.len(),4);assert!(page.overlays.iter().any(|o|matches!(o,Overlay::Text(t) if t.text==if index==1{&expected}else{&old_text}.as_str())));}
}

fn capture_language(kind:&str,bytes:&[u8]) {
    if let Some(root)=std::env::var_os("FOLIO_STRUCTURED_OVERLAY_CAPTURE_DIR"){
        use std::io::Write;let directory=PathBuf::from(root);std::fs::create_dir_all(&directory).unwrap();
        std::fs::OpenOptions::new().write(true).create_new(true).open(directory.join(format!("language-batch-{kind}.pdf"))).unwrap().write_all(bytes).unwrap();
    }
}

#[test]
fn structured_overlay_batch_mixed_copy_edits_keep_originals_and_return_final_bindings() {
    for rotation in [0,90,180,270]{for unit in [1.,1.7]{run_case(rotation,unit);}}
}
fn run_case(rotation:u16,unit:f32) {
    let mut rt=fixtures::runtime();let background=fixtures::source(&mut rt,rotation,unit,b"q 0.2 0.4 0.8 rg 160 300 20 20 re f Q");
    let original=rt.document(&background.id).unwrap().bytes().unwrap();
    let mut initial=fixtures::request(&rt,&background);initial.pages[0].overlays.retain(|o|!matches!(o,Overlay::Comment(_)));
    let stage=rt.prepare_structured_overlays(&initial,&mut ||Ok(())).unwrap();
    let mut doc=crate::pdf_output::load(&original).unwrap();doc.catalog_mut().unwrap().remove(b"SourceOnly");let page=doc.get_pages()[&1];
    let target=TargetPage{page_id:page,geometry:stage.pages[0].geometry,unit};
    let mut attached=attach_generated(doc,&[target],stage,&mut ||Ok(())).unwrap();
    let highlight=attached.pages[0].overlays.iter().find(|b|b.overlay_id=="highlight").unwrap().annotation_id;
    let popup=attached.document.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Popup","Parent"=>highlight,"P"=>page,"Rect"=>vec![0.into(),40.into(),100.into(),140.into()],"Open"=>false});
    attached.document.get_dictionary_mut(highlight).unwrap().set("Popup",popup);attached.document.get_dictionary_mut(page).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().push(popup.into());
    let input=crate::pdf_output::save(&mut attached.document).unwrap();
    let path=std::env::temp_dir().join(format!("folio-mixed-source-{}.pdf",Uuid::new_v4()));let info=rt.open_bytes(input.clone(),path.clone(),path,true,true,false,OpenSecurity::Restored(None)).unwrap();assert_eq!(info.pages[0].overlays.len(),4);
    let mut request=fixtures::request(&rt,&info);request.pages[0].overlays=info.pages[0].overlays.clone();let mut second=request.pages[0].clone();second.id="copy".into();
    second.overlays.retain(|o|!matches!(o,Overlay::Highlight(_)));
    let changed=second.overlays.iter_mut().find_map(|o|if let Overlay::Text(t)=o{Some(t)}else{None}).unwrap();changed.text="Changed imported".into();changed.x+=9.;let changed_id=changed.id.clone();
    let mut added=changed.clone();added.id="new-text".into();added.x=210.;added.y=150.;added.text="Added".into();second.overlays.push(Overlay::Text(added));
    second.overlays.push(Overlay::Comment(CommentOverlay{id:"new-comment".into(),x:270.,y:190.,text:"New note".into(),color:"#ffcc00".into()}));request.pages.push(second);
    let source=rt.document(&info.id).unwrap();let mut publication=source.imported_overlay_sources.publication(source.source_bytes.identity(),1,&mut ||Ok(())).unwrap();
    let keep=publication.page(0,&request.pages[0].overlays,&mut ||Ok(())).unwrap();let changed=publication.page(0,&request.pages[1].overlays,&mut ||Ok(())).unwrap();
    let groups=keep.owned.iter().map(|c|match c{OwnedChange::Keep(s,_)|OwnedChange::Replace(s,_)|OwnedChange::Delete(s)=>s.source_group()}).collect::<Vec<_>>();
    let mut graph=crate::composition_graph::CompositionGraph::new();graph.import_bytes(0,source.bytes().unwrap().as_ref(),Arc::new(||false)).unwrap();let groups=graph.source_annotation_groups(0,&groups,&mut ||Ok(())).unwrap();
    let anchors=groups.iter().flat_map(|g|std::iter::once(g.annotation).chain(g.popups.iter().copied())).collect::<Vec<_>>();let origin=anchors[0].page;
    let languages=graph.source_languages(&mut ||Ok(())).unwrap();let assembled=graph.assemble(&[0],&mut ||Ok(())).unwrap();let (document,pages,receipts)=crate::composition_occurrences::project_forms_tracked(assembled,&[origin,origin],&anchors,&mut ||Ok(())).unwrap();
    let prepare=||rt.prepare_structured_overlay_changes(&request,&[&keep,&changed],&mut ||Ok(())).unwrap();
    let prepared=prepare();assert!(prepared.pages[0].overlays.is_empty());assert_eq!(prepared.pages[1].overlays.len(),3);
    let targets=prepared.pages.iter().zip(&pages).map(|(p,id)|TargetPage{page_id:*id,geometry:p.geometry,unit:p.unit}).collect::<Vec<_>>();
    let plans=[PagePlan{source_page:origin,source_page_index:0,source_language:&languages[&0],changes:&keep,groups:&groups},PagePlan{source_page:origin,source_page_index:0,source_language:&languages[&0],changes:&changed,groups:&groups}];
    let unchanged_page=document.objects[&pages[0]].clone();let mut unchanged=BTreeMap::new();
    for receipt in receipts.iter().filter(|r|r.output_page==pages[0]){unchanged.insert(receipt.annotation,document.objects[&receipt.annotation].clone());}
    let changed_index=changed.owned.iter().position(|c|matches!(c,OwnedChange::Replace(s,_) if s.overlay_id==changed_id)).unwrap();
    let replacement_id=receipts.iter().find(|r|r.output_page==pages[1] && r.origin==groups[changed_index].annotation).unwrap().annotation;
    for (change,group) in changed.owned.iter().zip(&groups){if matches!(change,OwnedChange::Keep(_,_)){let id=receipts.iter().find(|r|r.output_page==pages[1] && r.origin==group.annotation).unwrap().annotation;unchanged.insert(id,document.objects[&id].clone());}}
    if rotation==0 && unit==1. {
        let mut untouched_request=request.clone();untouched_request.pages[1].overlays=untouched_request.pages[0].overlays.clone();
        let untouched=rt.prepare_structured_overlay_changes(&untouched_request,&[&keep,&keep],&mut ||Ok(())).unwrap();
        let keep_plans=[PagePlan{source_page:origin,source_page_index:0,source_language:&languages[&0],changes:&keep,groups:&groups},PagePlan{source_page:origin,source_page_index:0,source_language:&languages[&0],changes:&keep,groups:&groups}];
        let same=publish(document.clone(),&targets,&keep_plans,&receipts,untouched,&mut ||Ok(())).unwrap();
        assert_eq!(same.document.objects,document.objects);assert_eq!(same.document.trailer,document.trailer);assert!(same.added.iter().all(Vec::is_empty));
        let mut empty_request=request.clone();empty_request.pages[1].overlays.clear();let deleted=publication.page(0,&empty_request.pages[1].overlays,&mut ||Ok(())).unwrap();
        let stage=rt.prepare_structured_overlay_changes(&empty_request,&[&keep,&deleted],&mut ||Ok(())).unwrap();
        let delete_plans=[PagePlan{source_page:origin,source_page_index:0,source_language:&languages[&0],changes:&keep,groups:&groups},PagePlan{source_page:origin,source_page_index:0,source_language:&languages[&0],changes:&deleted,groups:&groups}];
        let mut with_native=document.clone();let note=with_native.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","P"=>pages[1],"Rect"=>vec![200.into(),200.into(),220.into(),220.into()],"Contents"=>Object::string_literal("Unowned native note")});
        let mut list=with_native.dereference(with_native.get_dictionary(pages[1]).unwrap().get(b"Annots").unwrap()).unwrap().1.as_array().unwrap().clone();list.push(note.into());with_native.get_dictionary_mut(pages[1]).unwrap().set("Annots",list);
        let note_before=with_native.objects[&note].clone();let deleted=publish(with_native,&targets,&delete_plans,&receipts,stage,&mut ||Ok(())).unwrap();
        assert!(deleted.pages[1].overlays.is_empty());assert_eq!(deleted.document.get_dictionary(pages[1]).unwrap().get(b"Annots").unwrap().as_array().unwrap(),&vec![Object::Reference(note)]);assert_eq!(deleted.document.objects[&note],note_before);
        // Composition may place a deleted root before a replacement. Receipts describe
        // the final copied list; replacing after deletion would invalidate these slots.
        let deleted_index=changed.owned.iter().position(|c|matches!(c,OwnedChange::Delete(_))).unwrap();
        let deleted_group=&groups[deleted_index];
        let deleted_root=receipts.iter().find(|r|r.output_page==pages[1] && r.origin==deleted_group.annotation).unwrap().annotation;
        let mut reordered=document.clone();
        let mut list=reordered.dereference(reordered.get_dictionary(pages[1]).unwrap().get(b"Annots").unwrap()).unwrap().1.as_array().unwrap().clone();
        let old_slot=list.iter().position(|v|reordered.dereference(v).unwrap().0==Some(deleted_root)).unwrap();
        let removed=list.remove(old_slot);list.insert(0,removed);
        let mut mapped=receipts.clone();
        for receipt in mapped.iter_mut().filter(|r|r.output_page==pages[1]){receipt.output_slot=list.iter().position(|v|reordered.dereference(v).unwrap().0==Some(receipt.annotation)).unwrap();}
        assert_eq!(mapped.iter().find(|r|r.annotation==deleted_root).unwrap().output_slot,0);
        assert!(mapped.iter().find(|r|r.annotation==replacement_id).unwrap().output_slot>0);
        reordered.get_dictionary_mut(pages[1]).unwrap().set("Annots",list);
        let reordered=publish(reordered,&targets,&plans,&mapped,prepare(),&mut ||Ok(())).unwrap();
        assert_eq!(reordered.pages[1].overlays.iter().find(|b|b.overlay_id==changed_id).unwrap().annotation_id,replacement_id);
        let list=reordered.document.dereference(reordered.document.get_dictionary(pages[1]).unwrap().get(b"Annots").unwrap()).unwrap().1.as_array().unwrap();
        assert_eq!(list.len(),5);
        for receipt in mapped.iter().filter(|r|r.output_page==pages[1] && (r.origin==deleted_group.annotation || deleted_group.popups.contains(&r.origin))){assert!(!list.iter().any(|v|reordered.document.dereference(v).unwrap().0==Some(receipt.annotation)));}
        for mode in ["duplicate-receipt","stale-slot","missing-receipt","unexpected-generated","missing-generated"] {
            let mut mapped=receipts.clone();let mut stage=prepare();
            match mode {
                "duplicate-receipt"=>mapped.push(mapped[0]),"stale-slot"=>mapped[0].output_slot+=1,"missing-receipt"=>{mapped.pop();},
                "unexpected-generated"=>stage.pages[1].overlays[0].overlay_id="not-in-draft".into(),"missing-generated"=>{stage.pages[1].overlays.pop();},_=>unreachable!(),
            }
            assert!(publish(document.clone(),&targets,&plans,&mapped,stage,&mut ||Ok(())).is_err(),"{mode}");
        }
        for (metadata,steps) in [(0,1_000_000),(1024,1_000_000),(16*1024*1024,1)] {assert!(publish_with_budget(document.clone(),&targets,&plans,&receipts,prepare(),metadata,steps,&mut ||Ok(())).is_err());}
        let mut calls=0;publish(document.clone(),&targets,&plans,&receipts,prepare(),&mut ||{calls+=1;Ok(())}).unwrap();
        for stop in [1,calls/2,calls] {let mut seen=0;assert!(publish(document.clone(),&targets,&plans,&receipts,prepare(),&mut ||{seen+=1;if seen==stop{Err(bad("cancelled"))}else{Ok(())}}).is_err());}
    }
    let mut result=publish(document,&targets,&plans,&receipts,prepared,&mut ||Ok(())).unwrap();
    assert_eq!(result.document.objects[&pages[0]],unchanged_page);for (id,value) in unchanged{assert_eq!(result.document.objects[&id],value);}
    assert_eq!(result.pages[0].overlays.len(),4);assert_eq!(result.pages[1].overlays.len(),5);assert!(result.added[0].is_empty());assert_eq!(result.added[1].len(),2);
    for (index,page) in result.pages.iter().enumerate(){
        let list=result.document.dereference(result.document.get_dictionary(page.page_id).unwrap().get(b"Annots").unwrap()).unwrap().1.as_array().unwrap();
        for binding in &page.overlays {assert!(list.iter().any(|v|result.document.dereference(v).unwrap().0==Some(binding.annotation_id)));}
        assert_eq!(list.len(),5,"page {index}");
    }
    assert_eq!(result.pages[1].overlays.iter().find(|b|b.overlay_id==changed_id).unwrap().annotation_id,replacement_id);
    assert_eq!(result.added[1], ["new-text","new-comment"].map(|name|result.pages[1].overlays.iter().find(|b|b.overlay_id==name).unwrap().annotation_id));
    let reference=rt.compose_imported_pdf(&request,&[],true,CompositionIntent::Save,None).unwrap();let saved=crate::pdf_output::save(&mut result.document).unwrap();
    for page in [0,1]{assert_eq!(fixtures::pixels(&rt,&saved,page),fixtures::pixels(&rt,&reference,page));}
    assert_eq!(source.bytes().unwrap().as_ref(),input.as_slice());
    if let Some(root)=std::env::var_os("FOLIO_STRUCTURED_OVERLAY_CAPTURE_DIR") {
        use std::io::Write;let directory=PathBuf::from(root);std::fs::create_dir_all(&directory).unwrap();
        for (kind,bytes) in [("source",input.as_slice()),("actual",saved.as_slice()),("reference",reference.as_slice())]{std::fs::OpenOptions::new().write(true).create_new(true).open(directory.join(format!("batch-{rotation}-{unit}-{kind}.pdf"))).unwrap().write_all(bytes).unwrap();}
    }
    let path=std::env::temp_dir().join(format!("folio-mixed-output-{}.pdf",Uuid::new_v4()));let reopened=rt.open_bytes(saved,path.clone(),path,true,true,false,OpenSecurity::Restored(None)).unwrap();
    assert_eq!(reopened.pages[0].overlays.len(),4);assert_eq!(reopened.pages[1].overlays.len(),4,"new native comment remains native");
    assert!(reopened.pages[1].overlays.iter().any(|o|matches!(o,Overlay::Text(t) if t.text=="Changed imported")));
}

#[test]
fn structured_overlay_generated_owners_keep_all_five_appearances_across_physical_frames(){
    use crate::composition_annotation_additions::{SourceLanguage,NewOwners,append};
    for rotation in [0,90,180,270]{for unit in [1.,1.7]{
        let mut rt=fixtures::runtime();let info=fixtures::source(&mut rt,rotation,unit,b"q 0.2 0.4 0.8 rg 160 300 20 20 re f Q");
        let request=fixtures::request(&rt,&info);let prepared=rt.prepare_structured_overlays(&request,&mut ||Ok(())).unwrap();
        let doc=crate::pdf_output::load(rt.document(&info.id).unwrap().bytes().unwrap().as_ref()).unwrap();let page=doc.get_pages()[&1];let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let language=SourceLanguage::capture(&doc,catalog).unwrap();
        let targets=[TargetPage{page_id:page,geometry:prepared.pages[0].geometry,unit}];
        let attached=attach_generated(doc,&targets,prepared,&mut ||Ok(())).unwrap();
        let ids=attached.pages[0].overlays.iter().map(|o|o.annotation_id).collect::<Vec<_>>();assert_eq!(ids.len(),5);
        let owners=[NewOwners{page,annotations:&ids,language:&language}];
        let mut before=attached.document;let mut after=append(before.clone(),&owners,&mut ||Ok(())).unwrap();
        let structure=crate::composition_structure::inspect(&after,catalog,&[page],&mut ||Ok(())).unwrap();assert_eq!(structure.top.len(),5);
        for (index,id) in ids.iter().enumerate(){assert_eq!(structure.owners[&(index as i64)].object,*id);}
        let before=crate::pdf_output::save(&mut before).unwrap();let after=crate::pdf_output::save(&mut after).unwrap();
        assert_eq!(fixtures::pixels(&rt,&before,0),fixtures::pixels(&rt,&after,0));
        if let Some(root)=std::env::var_os("FOLIO_STRUCTURED_OVERLAY_CAPTURE_DIR"){
            use std::io::Write;let directory=PathBuf::from(root);std::fs::create_dir_all(&directory).unwrap();
            let stem=format!("generated-owners-{rotation}-{unit}");
            for (kind,bytes) in [("before",before),("after",after)]{std::fs::OpenOptions::new().create_new(true).write(true).open(directory.join(format!("{stem}-{kind}.pdf"))).unwrap().write_all(&bytes).unwrap();}
            let names=attached.pages[0].overlays.iter().map(|o|o.overlay_id.as_str()).collect::<Vec<_>>();
            let file=std::fs::OpenOptions::new().create_new(true).write(true).open(directory.join(format!("{stem}-request.json"))).unwrap();serde_json::to_writer_pretty(file,&names).unwrap();
        }
    }}
}

fn flatten_case(rotation:u16,unit:f32,tagged:bool) {
    let mut rt=fixtures::runtime();let background=fixtures::source(&mut rt,rotation,unit,b"q 0.2 0.4 0.8 rg 160 300 20 20 re f Q");
    let original=rt.document(&background.id).unwrap().bytes().unwrap();
    let mut initial=fixtures::request(&rt,&background);initial.pages[0].overlays.retain(|o|!matches!(o,Overlay::Comment(_)));
    let stage=rt.prepare_structured_overlays(&initial,&mut ||Ok(())).unwrap();
    let mut doc=crate::pdf_output::load(&original).unwrap();doc.catalog_mut().unwrap().remove(b"SourceOnly");let page=doc.get_pages()[&1];
    let target=TargetPage{page_id:page,geometry:stage.pages[0].geometry,unit};
    let mut attached=attach_generated(doc,&[target],stage,&mut ||Ok(())).unwrap();
    let highlight=attached.pages[0].overlays.iter().find(|b|b.overlay_id=="highlight").unwrap().annotation_id;
    let popup=attached.document.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Popup","Parent"=>highlight,"P"=>page,"Rect"=>vec![0.into(),40.into(),100.into(),140.into()],"Open"=>false});
    attached.document.get_dictionary_mut(highlight).unwrap().set("Popup",popup);attached.document.get_dictionary_mut(page).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().push(popup.into());
    let text=attached.pages[0].overlays.iter().find(|b|b.overlay_id=="text").unwrap().annotation_id;
    let text_popup=attached.document.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Popup","Parent"=>text,"P"=>page,"Rect"=>vec![100.into(),40.into(),200.into(),140.into()],"Open"=>false});
    attached.document.get_dictionary_mut(text).unwrap().set("Popup",text_popup);attached.document.get_dictionary_mut(page).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().push(text_popup.into());
    let note=attached.document.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","P"=>page,"Rect"=>vec![220.into(),300.into(),240.into(),320.into()],"Contents"=>Object::string_literal("Original native note"),"NM"=>Object::string_literal("native-note")});attached.document.get_dictionary_mut(page).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().push(note.into());
    attached.document.catalog_mut().unwrap().set("Lang",Object::string_literal("fr-CA"));
    let catalog=attached.document.trailer.get(b"Root").unwrap().as_reference().unwrap();let language=crate::composition_annotation_additions::SourceLanguage::capture(&attached.document,catalog).unwrap();
    let owned=attached.pages[0].overlays.iter().map(|v|v.annotation_id).collect::<Vec<_>>();
    if tagged{attached.document=crate::composition_annotation_additions::append(attached.document,&[crate::composition_annotation_additions::NewOwners{page,annotations:&owned,language:&language}],&mut ||Ok(())).unwrap();}
    let input=crate::pdf_output::save(&mut attached.document).unwrap();
    let path=std::env::temp_dir().join(format!("folio-mixed-source-{}.pdf",Uuid::new_v4()));let info=rt.open_bytes(input.clone(),path.clone(),path,true,true,false,OpenSecurity::Restored(None)).unwrap();assert_eq!(info.pages[0].overlays.len(),4);
    let mut request=fixtures::request(&rt,&info);request.pages[0].overlays=info.pages[0].overlays.clone();let mut second=request.pages[0].clone();second.id="copy".into();
    second.overlays.retain(|o|!matches!(o,Overlay::Highlight(_)));
    let changed=second.overlays.iter_mut().find_map(|o|if let Overlay::Text(t)=o{Some(t)}else{None}).unwrap();changed.text="Changed imported".into();changed.x+=9.;
    let mut added=changed.clone();added.id="new-text".into();added.x=210.;added.y=150.;added.text="Added".into();second.overlays.push(Overlay::Text(added));
    second.overlays.push(Overlay::Comment(CommentOverlay{id:"new-comment".into(),x:270.,y:190.,text:"New note".into(),color:"#ffcc00".into()}));request.pages.push(second);request.flatten=true;
    let source=rt.document(&info.id).unwrap();let mut publication=source.imported_overlay_sources.publication(source.source_bytes.identity(),1,&mut ||Ok(())).unwrap();
    let keep=publication.page(0,&request.pages[0].overlays,&mut ||Ok(())).unwrap();let changed=publication.page(0,&request.pages[1].overlays,&mut ||Ok(())).unwrap();
    let groups=keep.owned.iter().map(|c|match c{OwnedChange::Keep(s,_)|OwnedChange::Replace(s,_)|OwnedChange::Delete(s)=>s.source_group()}).collect::<Vec<_>>();
    let mut graph=crate::composition_graph::CompositionGraph::new();graph.import_bytes(0,source.bytes().unwrap().as_ref(),Arc::new(||false)).unwrap();let groups=graph.source_annotation_groups(0,&groups,&mut ||Ok(())).unwrap();
    let anchors=groups.iter().flat_map(|g|std::iter::once(g.annotation).chain(g.popups.iter().copied())).collect::<Vec<_>>();let origin=anchors[0].page;
    let languages=graph.source_languages(&mut ||Ok(())).unwrap();let assembled=graph.assemble(&[0],&mut ||Ok(())).unwrap();let (document,pages,receipts)=crate::composition_occurrences::project_forms_tracked(assembled,&[origin,origin],&anchors,&mut ||Ok(())).unwrap();
    let prepare=||rt.prepare_structured_overlay_changes(&request,&[&keep,&changed],&mut ||Ok(())).unwrap();
    let prepared=prepare();assert_eq!(prepared.pages[0].overlays.len(),3);assert_eq!(prepared.pages[1].overlays.len(),5);
    let targets=prepared.pages.iter().zip(&pages).map(|(p,id)|TargetPage{page_id:*id,geometry:p.geometry,unit:p.unit}).collect::<Vec<_>>();
    let plans=[PagePlan{source_page:origin,source_page_index:0,source_language:&languages[&0],changes:&keep,groups:&groups},PagePlan{source_page:origin,source_page_index:0,source_language:&languages[&0],changes:&changed,groups:&groups}];
    let before=crate::pdf_output::save(&mut document.clone()).unwrap();
    if tagged&&rotation==0&&unit==1.{
        // No drawing remains, but the new native comment still needs its owner.
        let mut comments=request.clone();for page in &mut comments.pages{page.overlays.retain(|o|matches!(o,Overlay::Comment(_)));}
        let comment_changes=comments.pages.iter().map(|p|publication.page(0,&p.overlays,&mut ||Ok(())).unwrap()).collect::<Vec<_>>();
        let comment_refs=comment_changes.iter().collect::<Vec<_>>();let comment_stage=rt.prepare_structured_overlay_changes(&comments,&comment_refs,&mut ||Ok(())).unwrap();
        let comment_plans=comment_changes.iter().map(|changes|PagePlan{source_page:origin,source_page_index:0,source_language:&languages[&0],changes,groups:&groups}).collect::<Vec<_>>();
        let comments=publish_flattened(document.clone(),&targets,&comment_plans,&receipts,comment_stage,&mut ||Ok(())).unwrap();
        let catalog=comments.document.trailer.get(b"Root").unwrap().as_reference().unwrap();let owners=crate::composition_structure::inspect(&comments.document,catalog,&pages,&mut ||Ok(())).unwrap();
        assert_eq!(owners.owners.len(),1);assert!(matches!(owners.owners.values().next().unwrap().parents,crate::composition_structure::Parents::Whole(_)));
        assert!(comments.pages[0].overlays.is_empty());assert_eq!(comments.pages[1].overlays[0].overlay_id,"new-comment");assert_eq!(comments.added[1].len(),1);
        for page in &pages{assert_eq!(comments.document.get_dictionary(*page).unwrap().get(b"Contents").unwrap(),document.get_dictionary(*page).unwrap().get(b"Contents").unwrap());}

        for (metadata,steps) in [(0,1_000_000),(16*1024*1024,1)]{assert!(publish_mode(document.clone(),&targets,&plans,&receipts,prepare(),true,false,metadata,steps,&mut ||Ok(())).is_err());}
        let mut calls=0;publish_flattened(document.clone(),&targets,&plans,&receipts,prepare(),&mut ||{calls+=1;Ok(())}).unwrap();
        for stop in [1,calls/2,calls]{let mut seen=0;assert!(publish_flattened(document.clone(),&targets,&plans,&receipts,prepare(),&mut ||{seen+=1;if seen==stop{Err(bad("cancelled"))}else{Ok(())}}).is_err());}
    }
    let mut result=publish_flattened(document,&targets,&plans,&receipts,prepared,&mut ||Ok(())).unwrap();
    assert_eq!(result.pages.iter().map(|p|p.overlays.iter().map(|o|o.overlay_id.as_str()).collect::<Vec<_>>()).collect::<Vec<_>>(),vec![vec![request.pages[0].overlays.iter().find_map(|o|if let Overlay::Highlight(v)=o{Some(v.id.as_str())}else{None}).unwrap()],vec!["new-comment"]]);
    assert!(result.added[0].is_empty());assert_eq!(result.added[1].len(),1);
    let catalog=result.document.trailer.get(b"Root").unwrap().as_reference().unwrap();let structure=crate::composition_structure::inspect(&result.document,catalog,&pages,&mut ||Ok(())).unwrap();
    assert_eq!(structure.owners.values().filter(|o|matches!(o.parents,crate::composition_structure::Parents::Marked(_))).count(),7);assert_eq!(structure.owners.len(),if tagged{9}else{8});
    if !tagged{
        let mut names=BTreeMap::new();
        for value in result.document.objects.values(){if let Object::Dictionary(dict)=value{if let (Ok(name),Ok(ap))=(dict.get(b"NM"),dict.get(b"AP").and_then(Object::as_dict)){if let Ok(form)=ap.get(b"N").and_then(Object::as_reference){names.insert(form,lopdf::decode_text_string(name).unwrap());}}}}
        let mut actual=Vec::new();
        for parent in &structure.top{
            let element=result.document.get_dictionary(*parent).unwrap();assert_eq!(element.get(b"Lang").unwrap().as_str().unwrap(),b"fr-CA");let page=element.get(b"Pg").unwrap().as_reference().unwrap();let kid=element.get(b"K").unwrap().as_dict().unwrap();
            let name=if kid.get(b"Type").unwrap().as_name().unwrap()==b"MCR"{let form=kid.get(b"Stm").unwrap().as_reference().unwrap();let stream=result.document.objects[&form].as_stream().unwrap();let drawing=stream.dict.get(b"Resources").unwrap().as_dict().unwrap().get(b"XObject").unwrap().as_dict().unwrap().get(b"Drawing").unwrap().as_reference().unwrap();names[&drawing].clone()}else{let annotation=kid.get(b"Obj").unwrap().as_reference().unwrap();lopdf::decode_text_string(result.document.get_dictionary(annotation).unwrap().get(b"NM").unwrap()).unwrap()};
            actual.push((page,name));
        }
        let expected=request.pages.iter().zip(&pages).flat_map(|(plan,page)|plan.overlays.iter().filter(|o|!matches!(o,Overlay::Highlight(_))).map(move |o|(*page,label(o).to_owned()))).collect::<Vec<_>>();assert_eq!(actual,expected,"new flattened owners must follow output page and draft order");
    }
    let public=if tagged{Some(rt.compose_imported_pdf(&request,&[],true,CompositionIntent::Save,None).unwrap())}else{None};
    if tagged && rotation==0 && unit==1. {
        for intent in [CompositionIntent::Preview,CompositionIntent::Print,CompositionIntent::ImageExport] {
            let control=Arc::new(crate::PdfSaveControl::new());let cancel=control.clone();
            let monitor=crate::pdf_save::PdfSaveMonitor{control:control.clone(),total:2,report:Arc::new(move|event|{if event.stage=="preserving" {cancel.cancel();}})};
            let result=rt.compose_imported_pdf(&request,&[],true,intent,Some(&monitor));
            assert!(control.cancelled());
            assert!(matches!(result,Err(EngineError::InvalidRequest(ref message)) if message=="PDF save cancelled"),"render compatibility must not swallow cancellation");
        }
    }
    let reference=rt.compose_imported_pdf(&request,&[],true,CompositionIntent::Preview,None).unwrap();let saved=crate::pdf_output::save(&mut result.document).unwrap();
    if let Some(root)=std::env::var_os("FOLIO_STRUCTURED_OVERLAY_CAPTURE_DIR"){
        use std::io::Write;let directory=PathBuf::from(root);std::fs::create_dir_all(&directory).unwrap();
        for (kind,bytes) in [("source",input.as_slice()),("before",before.as_slice()),("after",saved.as_slice()),("reference",reference.as_slice())]{std::fs::OpenOptions::new().write(true).create_new(true).open(directory.join(format!("flatten-{}batch-{rotation}-{unit}-{kind}.pdf",if tagged{""}else{"unowned-"}))).unwrap().write_all(bytes).unwrap();}
    }
    for page in [0,1]{let a=fixtures::pixels(&rt,&saved,page);let b=fixtures::pixels(&rt,&reference,page);assert_eq!((a.0,a.1),(b.0,b.1));assert_eq!(a.2.iter().zip(&b.2).filter(|(a,b)|a!=b).count(),0,"{rotation}/{unit}/{page}");}
    if let Some(public)=public {for page in [0,1]{let a=fixtures::pixels(&rt,&public,page);let b=fixtures::pixels(&rt,&saved,page);assert_eq!((a.0,a.1),(b.0,b.1));assert_eq!(a.2.iter().zip(&b.2).filter(|(a,b)|a!=b).count(),0,"public/private flattened publication differs");}}
    assert_eq!(source.bytes().unwrap().as_ref(),input.as_slice());
    // Exercise print preparation, which keeps drawings in the full rendered
    // document instead of importing them into editable canvas drafts.
    let print=rt.prepare_print(request.clone()).unwrap();
    let print_bytes=rt.document(&print.id).unwrap().bytes().unwrap();
    for page in [0,1] {
        let expected=fixtures::pixels(&rt,&reference,page);
        let printed=fixtures::pixels(&rt,&print_bytes,page);
        assert_eq!(printed,expected,"print preparation differs at {rotation}/{unit}/{page}");
    }
    let path=std::env::temp_dir().join(format!("folio-flat-batch-output-{}.pdf",Uuid::new_v4()));let reopened=rt.open_bytes(saved.clone(),path.clone(),path,true,true,false,OpenSecurity::Restored(None)).unwrap();
    assert_eq!(reopened.pages[0].overlays.len(),1);assert!(reopened.pages[1].overlays.is_empty());

}
#[test]
fn structured_overlay_batch_flatten_keeps_source_owners_across_mixed_copies(){for rotation in [0,90,180,270]{for unit in [1.,1.7]{flatten_case(rotation,unit,true);}}}

#[test]
fn structured_overlay_batch_flatten_unowned_reading_order_matches_the_draft(){for rotation in [0,90,180,270]{for unit in [1.,1.7]{flatten_case(rotation,unit,false);}}}
