use super::*;
use crate::{composition_graph::CompositionGraph,composition_occurrences};
use lopdf::{dictionary,Stream};

fn fixture()->(Document,Vec<ObjectId>,ObjectId,ObjectId) {
    let mut doc=Document::with_version("1.7");let tree=doc.new_object_id();let mut pages=vec![];
    let contents=doc.add_object(Stream::new(dictionary!{},b"q 0.2 0.4 0.8 rg 10 10 20 20 re f Q".to_vec()));
    for _ in 0..2 {pages.push(doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary!{},"Contents"=>contents}));}
    let link=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Link","Rect"=>vec![5.into(),5.into(),20.into(),20.into()],"P"=>pages[0],"Dest"=>vec![Object::Reference(pages[1]),Object::Name(b"Fit".to_vec())]});
    let note=doc.new_object_id();let popup=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Popup","Rect"=>vec![100.into(),100.into(),200.into(),200.into()],"P"=>pages[0],"Parent"=>note});
    doc.objects.insert(note,dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![30.into(),30.into(),50.into(),50.into()],"P"=>pages[0],"Popup"=>popup,"Contents"=>Object::string_literal("owned"),"NM"=>Object::string_literal("same-label")}.into());
    let alias=doc.add_object(Object::Reference(note));let list=doc.add_object(vec![Object::Reference(link),Object::Reference(alias),Object::Reference(popup)]);
    doc.get_dictionary_mut(pages[0]).unwrap().set("Annots",list);
    doc.objects.insert(tree,dictionary!{"Type"=>"Pages","Count"=>2,"Kids"=>pages.iter().copied().map(Object::Reference).collect::<Vec<_>>()}.into());
    let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree});doc.trailer.set("Root",catalog);
    (doc,pages,note,popup)
}
fn import(graph:&mut CompositionGraph,key:usize,doc:&mut Document) {
    let mut bytes=vec![];doc.save_to(&mut bytes).unwrap();graph.import_bytes(key,&bytes,std::sync::Arc::new(||false)).unwrap();
}
#[test]
fn annotation_groups_derive_popup_identity_from_aliased_bound_source_slots() {
    let (mut doc,_,note,popup)=fixture();let mut graph=CompositionGraph::new();import(&mut graph,0,&mut doc);import(&mut graph,1,&mut doc);
    let groups=[SourceGroup{annotation:SourceSlot{page_index:0,slot:1,annotation_id:Some(note)},popup_slots:&[2]}];
    let a=graph.source_annotation_groups(0,&groups,&mut ||Ok(())).unwrap();let b=graph.source_annotation_groups(1,&groups,&mut ||Ok(())).unwrap();
    assert_eq!(a.len(),1);assert_eq!(a[0].popups.len(),1);assert_eq!(a[0].annotation.slot,1);assert_eq!(a[0].popups[0].slot,2);
    assert_ne!(a[0].annotation.annotation,b[0].annotation.annotation);assert_ne!(a[0].popups[0].annotation,b[0].popups[0].annotation);
    let explicit=graph.source_annotations(0,&[SourceSlot{page_index:0,slot:2,annotation_id:Some(popup)}],&mut ||Ok(())).unwrap();assert_eq!(a[0].popups,explicit);
    assert_eq!(graph.source_annotation_groups(0,&groups,&mut ||Ok(())).unwrap(),a);
}
#[test]
fn annotation_groups_direct_root_and_indirect_popup_survive_copy_then_selected_delete() {
    let (mut doc,pages,note,popup)=fixture();
    let mut direct=doc.get_dictionary(note).unwrap().clone();direct.remove(b"P");
    doc.get_dictionary_mut(popup).unwrap().remove(b"Parent");
    doc.get_dictionary_mut(pages[0]).unwrap().set("Annots",vec![Object::Dictionary(direct),popup.into()]);
    let mut graph=CompositionGraph::new();import(&mut graph,0,&mut doc);
    let groups=[SourceGroup{annotation:SourceSlot{page_index:0,slot:0,annotation_id:None},popup_slots:&[1]}];
    let bound=graph.source_annotation_groups(0,&groups,&mut ||Ok(())).unwrap();
    let anchors=[bound[0].annotation,bound[0].popups[0]];let source_page=anchors[0].page;
    let assembled=graph.assemble(&[0],&mut ||Ok(())).unwrap();
    let (copied,selected,receipts)=composition_occurrences::project_forms_tracked(assembled,&[source_page,source_page],&anchors,&mut ||Ok(())).unwrap();
    assert_eq!(receipts.len(),4);assert_ne!(receipts[0].annotation,receipts[2].annotation);
    let first=copied.get_dictionary(selected[0]).unwrap().clone();
    let output=crate::composition_annotation_removal::remove_owned(copied,&[crate::composition_annotation_removal::Removal{annotation:receipts[2],popups:&receipts[3..4]}],&mut ||Ok(())).unwrap();
    assert_eq!(output.get_dictionary(selected[0]).unwrap(),&first);
    assert!(output.get_dictionary(selected[1]).unwrap().get(b"Annots").unwrap().as_array().unwrap().is_empty());
}
#[test]
fn annotation_groups_direct_popup_keeps_reverse_ownership_and_idempotent_identity() {
    let (mut doc,pages,note,popup)=fixture();let direct=doc.get_dictionary(popup).unwrap().clone();
    doc.get_dictionary_mut(note).unwrap().remove(b"Popup");doc.get_dictionary_mut(pages[0]).unwrap().set("Annots",vec![note.into(),Object::Dictionary(direct)]);
    let mut graph=CompositionGraph::new();import(&mut graph,0,&mut doc);
    let groups=[SourceGroup{annotation:SourceSlot{page_index:0,slot:0,annotation_id:Some(note)},popup_slots:&[1]}];
    let bound=graph.source_annotation_groups(0,&groups,&mut ||Ok(())).unwrap();assert_eq!(bound[0].popups.len(),1);
    assert_eq!(graph.source_annotation_groups(0,&groups,&mut ||Ok(())).unwrap(),bound);
    let assembled=graph.assemble(&[0],&mut ||Ok(())).unwrap();
    assert_eq!(assembled.get_dictionary(bound[0].popups[0].annotation).unwrap().get(b"Parent").unwrap().as_reference().unwrap(),bound[0].annotation.annotation);
}
#[test]
fn annotation_groups_wrong_slots_links_and_roles_refuse_without_materialization() {
    for mode in ["stale-root","wrong-popup","omitted-popup","duplicate-popup","foreign-parent","foreign-page","not-popup","duplicate-alias"] {
        let (mut doc,pages,note,popup)=fixture();let mut root=SourceSlot{page_index:0,slot:1,annotation_id:Some(note)};let mut slots=vec![2];
        match mode {
            "stale-root"=>root.annotation_id=Some(popup),
            "wrong-popup"=>slots=vec![0],
            "omitted-popup"=>slots.clear(),
            "duplicate-popup"=>slots.push(2),
            "foreign-parent"=>{doc.get_dictionary_mut(popup).unwrap().set("Parent",pages[1]);},
            "foreign-page"=>{doc.get_dictionary_mut(popup).unwrap().set("P",pages[1]);},
            "not-popup"=>{doc.get_dictionary_mut(popup).unwrap().set("Subtype","Text");},
            "duplicate-alias"=>{let alias=doc.add_object(Object::Reference(popup));let array=doc.get_dictionary(pages[0]).unwrap().get(b"Annots").unwrap().as_reference().unwrap();doc.get_object_mut(array).unwrap().as_array_mut().unwrap().push(alias.into());slots.push(3);},
            _=>unreachable!(),
        }
        let mut graph=CompositionGraph::new();import(&mut graph,0,&mut doc);
        assert!(graph.source_annotation_groups(0,&[SourceGroup{annotation:root,popup_slots:&slots}],&mut ||Ok(())).is_err(),"{mode}");
        // Failed proof leaves the exact graph available for unrelated callers.
        let mut clean=CompositionGraph::new();import(&mut clean,0,&mut doc);
        let probe=[SourceSlot{page_index:0,slot:1,annotation_id:Some(note)}];
        assert_eq!(graph.source_annotations(0,&probe,&mut ||Ok(())).unwrap(),clean.source_annotations(0,&probe,&mut ||Ok(())).unwrap(),"{mode}");
    }
}
#[test]
fn annotation_groups_late_failure_and_cancellation_do_not_consume_direct_ids() {
    let (mut doc,pages,note,popup)=fixture();let mut direct=doc.get_dictionary(note).unwrap().clone();direct.remove(b"Popup");
    doc.get_dictionary_mut(pages[0]).unwrap().set("Annots",vec![Object::Dictionary(direct),popup.into()]);
    let root=SourceSlot{page_index:0,slot:0,annotation_id:None};let mut graph=CompositionGraph::new();import(&mut graph,0,&mut doc);
    assert!(graph.source_annotation_groups(0,&[SourceGroup{annotation:root,popup_slots:&[1]}],&mut ||Ok(())).is_err(),"the popup still refers to the old separate root");
    let mut clean=CompositionGraph::new();import(&mut clean,0,&mut doc);
    let expected=clean.source_annotations(0,&[root],&mut ||Ok(())).unwrap();assert_eq!(graph.source_annotations(0,&[root],&mut ||Ok(())).unwrap(),expected);
    let mut calls=0;let mut graph=CompositionGraph::new();import(&mut graph,0,&mut doc);
    let groups=[SourceGroup{annotation:root,popup_slots:&[]}];graph.source_annotation_groups(0,&groups,&mut ||{calls+=1;Ok(())}).unwrap();
    for stop in [1,calls/2,calls] {
        let mut graph=CompositionGraph::new();import(&mut graph,0,&mut doc);let mut seen=0;
        assert!(graph.source_annotation_groups(0,&groups,&mut ||{seen+=1;if seen==stop{Err(bad("cancelled"))}else{Ok(())}}).is_err());
        assert_eq!(graph.source_annotations(0,&[root],&mut ||Ok(())).unwrap(),expected);
    }
}
#[test]
fn annotation_groups_reserve_source_inverse_and_group_metadata_before_allocating() {
    let (doc,pages,note,_)=fixture();let ids=doc.objects.keys().map(|id|(*id,*id)).collect();let normalized=BTreeMap::new();
    let groups=[SourceGroup{annotation:SourceSlot{page_index:0,slot:1,annotation_id:Some(note)},popup_slots:&[2]}];
    for budget in [0,128,512,1024] {assert!(bind_source_groups(&doc,&pages,&ids,&groups,&normalized,budget,&mut ||Ok(())).is_err());}
    let (_,bound)=bind_source_groups(&doc,&pages,&ids,&groups,&normalized,16*1024*1024,&mut ||Ok(())).unwrap();assert_eq!(bound[0].annotation.annotation,note);
}
#[test]
fn annotation_receipts_source_relocation_uses_canonical_ids_and_exact_slots() {
    let (mut doc,_,note,popup)=fixture();let mut graph=CompositionGraph::new();import(&mut graph,0,&mut doc);import(&mut graph,1,&mut doc);
    let slots=[SourceSlot{page_index:0,slot:1,annotation_id:Some(note)},SourceSlot{page_index:0,slot:2,annotation_id:Some(popup)}];
    let a=graph.source_annotations(0,&slots,&mut ||Ok(())).unwrap();let b=graph.source_annotations(1,&slots,&mut ||Ok(())).unwrap();
    assert_eq!(a.len(),2);assert_eq!(a[0].slot,1);assert_eq!(a[1].slot,2);assert_ne!(a[0].annotation,b[0].annotation);assert_ne!(a[0].page,b[0].page);
    for wrong in [SourceSlot{slot:0,..slots[0]},SourceSlot{page_index:9,..slots[0]},SourceSlot{annotation_id:Some((u32::MAX,0)),..slots[0]},SourceSlot{annotation_id:None,..slots[0]}] {
        assert!(graph.source_annotations(0,&[wrong],&mut ||Ok(())).is_err());
    }
    assert!(graph.source_annotations(9,&slots,&mut ||Ok(())).is_err());
    assert!(graph.source_annotations(0,&[slots[0],slots[0]],&mut ||Ok(())).is_err());
}
#[test]
fn annotation_receipts_follow_repeated_pages_popups_and_disabled_link_destinations() {
    let (doc,pages,note,popup)=fixture();
    let anchors=[Anchor{page:pages[0],slot:1,annotation:note},Anchor{page:pages[0],slot:2,annotation:popup}];
    let input=doc.objects.clone();
    let (output,selected,receipts)=composition_occurrences::project_forms_tracked(doc.clone(),&[pages[0],pages[0]],&anchors,&mut ||Ok(())).unwrap();
    assert_eq!(receipts.len(),4);assert_eq!(selected.len(),2);
    assert_eq!(receipts[0].annotation,note);assert_eq!(receipts[1].annotation,popup);
    assert_ne!(receipts[2].annotation,note);assert_ne!(receipts[3].annotation,popup);
    for (index,page) in selected.iter().enumerate() {
        let annotations=output.dereference(output.get_dictionary(*page).unwrap().get(b"Annots").unwrap()).unwrap().1.as_array().unwrap();
        let link=output.dereference(&annotations[0]).unwrap().1.as_dict().unwrap();assert!(!link.has(b"Dest"),"omitted-page destination is disabled, not redirected");
        for receipt in &receipts[index*2..index*2+2] {
            assert_eq!(receipt.output_page,*page);
            assert_eq!(output.dereference(&annotations[receipt.output_slot]).unwrap().0,Some(receipt.annotation));
            assert_eq!(output.get_dictionary(receipt.annotation).unwrap().get(b"P").unwrap().as_reference().unwrap(),*page);
        }
        assert_eq!(output.get_dictionary(receipts[index*2].annotation).unwrap().get(b"Popup").unwrap().as_reference().unwrap(),receipts[index*2+1].annotation);
        assert_eq!(output.get_dictionary(receipts[index*2+1].annotation).unwrap().get(b"Parent").unwrap().as_reference().unwrap(),receipts[index*2].annotation);
    }
    assert_eq!(doc.objects,input);
}
#[test]
fn annotation_receipts_unique_selection_and_cancellation_never_guess_identity() {
    let (doc,pages,note,_)=fixture();let anchor=Anchor{page:pages[0],slot:1,annotation:note};
    let (_,selected,receipts)=composition_occurrences::project_forms_tracked(doc.clone(),&[pages[1],pages[0]],&[anchor],&mut ||Ok(())).unwrap();
    assert_eq!(selected,vec![pages[1],pages[0]]);assert_eq!(receipts.len(),1);assert_eq!(receipts[0].annotation,note);
    for wrong in [Anchor{slot:0,..anchor},Anchor{page:pages[1],..anchor},Anchor{annotation:(u32::MAX,0),..anchor}] {
        assert!(composition_occurrences::project_forms_tracked(doc.clone(),&[pages[0]],&[wrong],&mut ||Ok(())).is_err());
    }
    let mut calls=0;
    composition_occurrences::project_forms_tracked(doc.clone(),&[pages[0],pages[0]],&[anchor],&mut ||{calls+=1;Ok(())}).unwrap();
    for stop in [1,calls/2,calls] {let mut seen=0;assert!(composition_occurrences::project_forms_tracked(doc.clone(),&[pages[0],pages[0]],&[anchor],&mut ||{seen+=1;if seen==stop{Err(bad("cancelled"))}else{Ok(())}}).is_err());}
}

#[test]
fn annotation_receipts_direct_slots_get_stable_shared_identity_before_occurrence_copy() {
    let (mut doc,pages,_,_)=fixture();
    let direct=Object::Dictionary(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![30.into(),30.into(),50.into(),50.into()],"Contents"=>Object::string_literal("direct source sentinel")});
    let list=doc.add_object(vec![direct.clone()]);
    for page in &pages {doc.get_dictionary_mut(*page).unwrap().set("Annots",list);}
    let mut graph=CompositionGraph::new();import(&mut graph,0,&mut doc);
    let slots=[SourceSlot{page_index:0,slot:0,annotation_id:None},SourceSlot{page_index:1,slot:0,annotation_id:None}];
    // An invalid later request cannot consume IDs or change earlier source slots.
    assert!(graph.source_annotations(0,&[slots[0],SourceSlot{slot:99,..slots[1]}],&mut ||Ok(())).is_err());
    let anchors=graph.source_annotations(0,&slots,&mut ||Ok(())).unwrap();
    assert_eq!(anchors[0].annotation,anchors[1].annotation,"shared source array retains its shared annotation identity");
    assert_eq!(graph.source_annotations(0,&slots,&mut ||Ok(())).unwrap(),anchors,"materialization must be idempotent");
    let mut clean=CompositionGraph::new();import(&mut clean,0,&mut doc);
    assert_eq!(clean.source_annotations(0,&slots,&mut ||Ok(())).unwrap(),anchors,"failed binding must not consume identities");
    let source_pages=graph.source_pages(0,&mut ||Ok(())).unwrap();
    let assembled=graph.assemble(&[0],&mut ||Ok(())).unwrap();
    assert_eq!(assembled.get_object(anchors[0].annotation).unwrap(),&direct);
    let (result,output,receipts)=composition_occurrences::project_forms_tracked(assembled,&[source_pages[0],source_pages[0],source_pages[1]],&anchors,&mut ||Ok(())).unwrap();
    assert_eq!(receipts.len(),3);
    assert_eq!(receipts[0].annotation,receipts[2].annotation);
    assert_ne!(receipts[0].annotation,receipts[1].annotation);
    for (receipt,page) in receipts.iter().zip(output) {assert_eq!(receipt.output_page,page);assert_eq!(result.get_object(receipt.annotation).unwrap(),&direct);}
}

#[test]
fn annotation_receipts_equal_direct_values_keep_distinct_slot_identities() {
    let (mut doc,pages,_,_)=fixture();
    let direct=Object::Dictionary(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![30.into(),30.into(),50.into(),50.into()],"Contents"=>Object::string_literal("equal values")});
    doc.get_dictionary_mut(pages[0]).unwrap().set("Annots",vec![direct.clone(),direct]);
    let mut graph=CompositionGraph::new();import(&mut graph,0,&mut doc);
    let anchors=graph.source_annotations(0,&[SourceSlot{page_index:0,slot:0,annotation_id:None},SourceSlot{page_index:0,slot:1,annotation_id:None}],&mut ||Ok(())).unwrap();
    assert_ne!(anchors[0].annotation,anchors[1].annotation);
    let assembled=graph.assemble(&[0],&mut ||Ok(())).unwrap();
    let (_,_,receipts)=composition_occurrences::project_forms_tracked(assembled,&[anchors[0].page,anchors[0].page],&anchors,&mut ||Ok(())).unwrap();
    assert_eq!(receipts.iter().map(|r|r.annotation).collect::<BTreeSet<_>>().len(),4);
}

#[test]
fn annotation_receipts_finish_checks_real_slots_repetition_and_resource_limits() {
    let (mut doc,pages,note,_)=fixture();let anchor=Anchor{page:pages[0],slot:1,annotation:note};
    let mut tracker=Tracker::new(&doc,&[anchor],&mut ||Ok(())).unwrap();
    tracker.record(pages[0],pages[0],None,&mut ||Ok(())).unwrap();
    let array=doc.get_dictionary(pages[0]).unwrap().get(b"Annots").unwrap().as_reference().unwrap();
    doc.get_object_mut(array).unwrap().as_array_mut().unwrap().swap(0,1);
    let receipts=tracker.finish(&doc,&mut ||Ok(())).unwrap();assert_eq!(receipts[0].output_slot,0);
    let anchor=Anchor{slot:0,..anchor};let mut tracker=Tracker::new(&doc,&[anchor],&mut ||Ok(())).unwrap();
    tracker.record(pages[0],pages[0],None,&mut ||Ok(())).unwrap();
    doc.get_object_mut(array).unwrap().as_array_mut().unwrap().push(Object::Reference(note));
    assert!(tracker.finish(&doc,&mut ||Ok(())).is_err());
    let mut tracker=Tracker::new(&doc,&[anchor],&mut ||Ok(())).unwrap();tracker.work.metadata=1;
    assert!(tracker.record(pages[0],pages[0],None,&mut ||Ok(())).is_err());
    let mut tracker=Tracker::new(&doc,&[anchor],&mut ||Ok(())).unwrap();tracker.work.steps=1_000_000;
    assert!(tracker.record(pages[0],pages[0],None,&mut ||Ok(())).is_err());
}

