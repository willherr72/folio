use super::*;
use lopdf::{dictionary, Object, Stream};

#[test]
fn projection_final_pruning_preserves_live_aliases_cycles_payloads_and_unknown_metadata() {
    let mut f=fixture();let original=f.doc.clone();
    let payload=f.doc.add_object(Stream::new(dictionary!{},vec![73;8192]));let alias=f.doc.add_object(Object::Reference(payload));
    f.doc.trailer.set("PrivateMetadata",alias);
    let a=f.doc.new_object_id();let b=f.doc.add_object(dictionary!{"Next"=>a});f.doc.objects.insert(a,dictionary!{"Next"=>b}.into());f.doc.catalog_mut().unwrap().set("OpaqueCycle",a);
    let orphan=f.doc.add_object(Stream::new(dictionary!{},vec![99;65536]));let orphan_alias=f.doc.add_object(Object::Reference(orphan));
    let dead_a=f.doc.new_object_id();let dead_b=f.doc.add_object(dictionary!{"Next"=>dead_a});f.doc.objects.insert(dead_a,dictionary!{"Next"=>dead_b}.into());
    let before=f.doc.clone();let output=prune_final(f.doc,&mut ||Ok(())).unwrap();
    for id in [orphan,orphan_alias,dead_a,dead_b]{assert!(!output.objects.contains_key(&id),"obsolete resource survived final pruning");}
    for (id,value) in &before.objects {if output.objects.contains_key(id){assert_eq!(&output.objects[id],value);}}
    for id in original.objects.keys(){assert!(output.objects.contains_key(id),"original live object lost");}
    for id in [payload,alias,a,b]{assert_eq!(output.objects[&id],before.objects[&id]);}
    assert_eq!(output.trailer,before.trailer);assert_eq!(output.max_id,before.max_id);
    let again=prune_final(output.clone(),&mut ||Ok(())).unwrap();assert_eq!(again.objects,output.objects);assert_eq!(again.trailer,output.trailer);
}

#[test]
fn projection_final_pruning_checks_limits_and_cancellation_before_returning() {
    let f=fixture();let original=f.doc.objects.clone();
    for (metadata,steps) in [(0,1_000_000),(16*1024*1024,0),(128,1_000_000)] {assert!(prune_final_with_budget(f.doc.clone(),metadata,steps,&mut ||Ok(())).is_err());}
    let mut calls=0;prune_final(f.doc.clone(),&mut ||{calls+=1;Ok(())}).unwrap();
    for stop in [1,calls/2,calls] {let mut seen=0;assert!(prune_final(f.doc.clone(),&mut ||{seen+=1;if seen==stop{Err(bad("cancelled"))}else{Ok(())}}).is_err());}
    let mut missing=f.doc.clone();missing.catalog_mut().unwrap().set("MissingLive",Object::Reference((99999,0)));assert!(prune_final(missing,&mut ||Ok(())).is_err());
    assert_eq!(f.doc.objects,original);
}

struct Fixture {
    doc: Document,
    catalog: ObjectId,
    pages: Vec<ObjectId>,
    root: ObjectId,
    group: ObjectId,
    elements: Vec<ObjectId>,
    font: ObjectId,
    note: ObjectId,
    form: ObjectId,
}

fn fixture() -> Fixture {
    let mut doc=Document::with_version("1.7");
    let page_root=doc.new_object_id();let branch=doc.new_object_id();
    let root=doc.new_object_id();let group=doc.new_object_id();
    let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let form=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),10.into(),10.into()],"StructParents"=>7},b"/P <</MCID 0>> BDC EMC".to_vec()));
    let resources=doc.add_object(dictionary!{"Font"=>dictionary!{"F1"=>font}});
    let mut pages=vec![];let mut elements=vec![];let mut nums=vec![];
    for index in 0..3 {
        let stream=doc.add_object(Stream::new(dictionary!{},format!("/P <</MCID 2>> BDC BT /F1 12 Tf 20 30 Td (Page {index}) Tj ET EMC").into_bytes()));
        let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>branch,"Contents"=>stream,"StructParents"=>index,"UserUnit"=>1.7});
        let element=doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"P","P"=>group,"Pg"=>page,"K"=>2,"Alt"=>Object::string_literal(format!("Page {index}"))});
        pages.push(page);elements.push(element);nums.extend([Object::Integer(index),Object::Array(vec![Object::Null,Object::Null,element.into()])]);
    }
    let note=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","P"=>pages[1],"StructParent"=>6,"Rect"=>vec![0.into(),0.into(),10.into(),10.into()],"Contents"=>Object::string_literal("retained note")});
    doc.get_dictionary_mut(pages[1]).unwrap().set("Annots",vec![Object::Reference(note)]);
    doc.get_dictionary_mut(pages[1]).unwrap().set("Resources",dictionary!{"Font"=>dictionary!{"F1"=>font},"XObject"=>dictionary!{"Fm"=>form}});
    doc.get_dictionary_mut(elements[1]).unwrap().set("K",vec![2.into(),Object::Dictionary(dictionary!{"Type"=>"OBJR","Obj"=>note}),Object::Dictionary(dictionary!{"Type"=>"MCR","Stm"=>form,"MCID"=>0})]);
    nums.extend([6.into(),elements[1].into(),7.into(),Object::Array(vec![elements[1].into()])]);
    doc.objects.insert(group,dictionary!{"Type"=>"StructElem","S"=>"Document","P"=>root,"Pg"=>pages[0],"K"=>elements.iter().copied().map(Object::Reference).collect::<Vec<_>>()}.into());
    doc.objects.insert(root,dictionary!{"Type"=>"StructTreeRoot","K"=>group,"ParentTree"=>dictionary!{"Nums"=>nums},"ParentTreeNextKey"=>8}.into());
    doc.objects.insert(branch,dictionary!{"Type"=>"Pages","Parent"=>page_root,"Count"=>3,"Kids"=>pages.iter().copied().map(Object::Reference).collect::<Vec<_>>(),"MediaBox"=>vec![10.into(),20.into(),310.into(),420.into()],"Resources"=>resources,"Rotate"=>90,"CropBox"=>vec![20.into(),30.into(),300.into(),400.into()]}.into());
    doc.objects.insert(page_root,dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(branch)],"Count"=>3}.into());
    let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>page_root,"StructTreeRoot"=>root});doc.trailer.set("Root",catalog);
    Fixture{doc,catalog,pages,root,group,elements,font,note,form}
}

fn stream_bytes(doc:&Document)->std::collections::BTreeMap<ObjectId,Vec<u8>> {
    doc.objects.iter().filter_map(|(id,v)|v.as_stream().ok().map(|s|(*id,s.content.clone()))).collect()
}

#[test]
fn projection_deleted_navigation_prunes_local_names_but_keeps_remote_and_shared_siblings() {
    let mut f=fixture();
    let gone=f.doc.add_object(dictionary!{"S"=>"GoTo","D"=>Object::Name(b"chapter".to_vec()),"Next"=>dictionary!{"S"=>"URI","URI"=>Object::string_literal("https://must-not-be-promoted.example/")}});
    let stay=f.doc.add_object(dictionary!{"S"=>"GoTo","D"=>Object::string_literal("chapter"),"Private"=>17});
    let remote=f.doc.add_object(dictionary!{"S"=>"GoToR","D"=>Object::Name(b"chapter".to_vec()),"F"=>Object::string_literal("other.pdf")});
    let chain=f.doc.add_object(dictionary!{"S"=>"URI","URI"=>Object::string_literal("https://example.com/"),"Next"=>vec![gone.into(),stay.into(),remote.into(),stay.into()]});
    let link=f.doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Link","P"=>f.pages[1],"Rect"=>vec![0.into(),0.into(),10.into(),10.into()],"A"=>gone,"AA"=>dictionary!{"E"=>chain,"X"=>gone}});
    f.doc.get_dictionary_mut(f.pages[1]).unwrap().set("Annots",vec![Object::Reference(f.note),link.into()]);
    f.doc.get_dictionary_mut(f.pages[1]).unwrap().set("AA",dictionary!{"O"=>chain,"C"=>gone});
    f.doc.get_dictionary_mut(f.catalog).unwrap().set("Dests",dictionary!{"chapter"=>vec![Object::Reference(f.pages[0]),"Fit".into()]});
    f.doc.get_dictionary_mut(f.catalog).unwrap().set("Names",dictionary!{"Dests"=>dictionary!{"Names"=>vec![Object::string_literal("chapter"),Object::Array(vec![f.pages[1].into(),"Fit".into()])]}});
    let out=project(f.doc,&[f.pages[1]],&mut ||Ok(())).expect("supported deleted destinations should be projected");
    assert!(!out.get_dictionary(f.catalog).unwrap().has(b"Dests"));
    assert!(out.get_dictionary(f.catalog).unwrap().has(b"Names"));
    let l=out.get_dictionary(link).unwrap(); assert!(!l.has(b"A"));
    let aa=out.dereference(l.get(b"AA").unwrap()).unwrap().1.as_dict().unwrap(); assert!(!aa.has(b"X"));
    let pa=out.dereference(out.get_dictionary(f.pages[1]).unwrap().get(b"AA").unwrap()).unwrap().1.as_dict().unwrap();
    assert!(!pa.has(b"C")); assert_eq!(pa.get(b"O").unwrap(),aa.get(b"E").unwrap());
    let a=out.dereference(pa.get(b"O").unwrap()).unwrap().1.as_dict().unwrap();
    let next=out.dereference(a.get(b"Next").unwrap()).unwrap().1.as_array().unwrap();
    assert_eq!(next,&vec![Object::Reference(stay),remote.into(),stay.into()]);
    assert!(!out.objects.contains_key(&gone)); assert_eq!(out.get_dictionary(remote).unwrap().get(b"D").unwrap(),&Object::Name(b"chapter".to_vec()));
}

#[test]
fn projection_deleted_navigation_keeps_native_outline_folders_and_clears_lost_se() {
    let mut f=fixture(); let root=f.doc.new_object_id();let folder=f.doc.new_object_id();
    let title=Object::String(vec![0xfe,0xff,0,0x41],lopdf::StringFormat::Hexadecimal);
    let child=f.doc.add_object(dictionary!{"Title"=>title.clone(),"Parent"=>folder,"Dest"=>vec![Object::Reference(f.pages[1]),"Fit".into()],"SE"=>f.elements[0],"F"=>3,"C"=>vec![1.into(),0.into(),0.into()],"Private"=>77});
    let gone=f.doc.add_object(dictionary!{"Title"=>Object::string_literal("gone"),"Parent"=>root,"Prev"=>folder,"Dest"=>vec![Object::Reference(f.pages[0]),"Fit".into()]});
    f.doc.objects.insert(folder,dictionary!{"Title"=>Object::string_literal("folder"),"Parent"=>root,"Next"=>gone,"Dest"=>vec![Object::Reference(f.pages[0]),"Fit".into()],"First"=>child,"Last"=>child,"Count"=>-1}.into());
    f.doc.objects.insert(root,dictionary!{"Type"=>"Outlines","First"=>folder,"Last"=>gone,"Count"=>2}.into());
    f.doc.get_dictionary_mut(f.catalog).unwrap().set("Outlines",root);
    let out=project(f.doc,&[f.pages[1]],&mut ||Ok(())).expect("native outline selection should preserve surviving metadata");
    let p=out.get_dictionary(folder).unwrap();assert!(!p.has(b"Dest"));assert!(!p.has(b"Next"));assert_eq!(p.get(b"Count").unwrap(),&Object::Integer(-1));
    let c=out.get_dictionary(child).unwrap();assert_eq!(c.get(b"Title").unwrap(),&title);assert_eq!(c.get(b"Private").unwrap(),&Object::Integer(77));assert!(!c.has(b"SE"));
    assert!(!out.objects.contains_key(&gone));assert_eq!(out.get_dictionary(root).unwrap().get(b"Count").unwrap(),&Object::Integer(1));
    crate::composition_outlines::inspect(&out,f.catalog,&structure(&out),&mut ||Ok(())).unwrap();
}
fn structure(doc:&Document)->crate::composition_structure::StructureInfo {
    let c=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let p=crate::composition_pages::inspect(doc,c,&mut ||Ok(())).unwrap();
    crate::composition_structure::inspect(doc,c,&p.pages,&mut ||Ok(())).unwrap()
}

#[test]
fn projection_occurrences_copy_marked_forms_annotations_and_inherited_geometry() {
    let f=fixture();let old_streams=stream_bytes(&f.doc);let order=vec![f.pages[0],f.pages[1],f.pages[1],f.pages[2]];
    let (out,ids)=crate::composition_occurrences::project(f.doc,&order,&mut ||Ok(())).unwrap();
    assert_eq!(&ids[..2],&f.pages[..2]);assert_ne!(ids[2],f.pages[1]);assert_eq!(ids[3],f.pages[2]);
    let before=out.get_dictionary(ids[1]).unwrap();let copy=out.get_dictionary(ids[2]).unwrap();
    assert_eq!(before.get(b"Contents").unwrap(),copy.get(b"Contents").unwrap());
    for page in &ids {let page=out.get_dictionary(*page).unwrap();assert_eq!(page.get(b"Rotate").unwrap(),&Object::Integer(90));assert!(page.has(b"MediaBox"));}
    assert_ne!(before.get(b"StructParents").unwrap(),copy.get(b"StructParents").unwrap());
    let resources=out.dereference(copy.get(b"Resources").unwrap()).unwrap().1.as_dict().unwrap();
    let form=resources.get(b"XObject").unwrap().as_dict().unwrap().get(b"Fm").unwrap().as_reference().unwrap();
    assert_ne!(form,f.form);assert_eq!(out.get_object(form).unwrap().as_stream().unwrap().content,old_streams[&f.form]);
    let font=resources.get(b"Font").unwrap().as_dict().unwrap().get(b"F1").unwrap().as_reference().unwrap();assert_eq!(font,f.font);
    let note=copy.get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap();assert_ne!(note,f.note);
    assert_eq!(out.get_dictionary(note).unwrap().get(b"P").unwrap(),&Object::Reference(ids[2]));
    let info=structure(&out);assert_eq!(info.owners.len(),8);assert_eq!(info.elements.len(),6);
    assert_eq!(out.get_object(f.form).unwrap().as_stream().unwrap().content,old_streams[&f.form]);
}

#[test]
fn projection_structure_copy_plan_keeps_sparse_owners_and_does_not_delete_original_ids() {
    let mut f=fixture();
    for (i,id) in f.elements.iter().enumerate(){f.doc.get_dictionary_mut(*id).unwrap().set("ID",Object::string_literal(format!("element-{i}")));}
    let record=f.doc.add_object(dictionary!{"Type"=>"MCR","Pg"=>f.pages[1],"Stm"=>f.form,"MCID"=>0});
    f.doc.get_dictionary_mut(f.elements[1]).unwrap().get_mut(b"K").unwrap().as_array_mut().unwrap()[2]=record.into();
    let info=structure(&f.doc);let original=f.doc.objects.clone();
    let plan=structure_for_copy(&f.doc,&info,f.pages[1],&mut SelectionBudget::default(),&mut ||Ok(())).unwrap();
    assert_eq!(f.doc.objects,original);assert_eq!(plan.elements,BTreeSet::from([f.group,f.elements[1]]));
    assert_eq!(plan.marked,BTreeSet::from([(f.pages[1],2),(f.form,0)]));assert_eq!(plan.whole,BTreeSet::from([f.note]));assert_eq!(plan.records,BTreeSet::from([record]));
    let parent=plan.fields[&(f.root,b"ParentTree".to_vec())].as_ref().unwrap().as_dict().unwrap();
    let nums=parent.get(b"Nums").unwrap().as_array().unwrap();assert_eq!(nums.iter().step_by(2).map(|v|v.as_i64().unwrap()).collect::<Vec<_>>(),vec![1,6,7]);
    assert_eq!(nums[1].as_array().unwrap(),&vec![Object::Null,Object::Null,f.elements[1].into()]);
    assert_eq!(plan.fields[&(f.group,b"Pg".to_vec())],None);
    assert!(project(f.doc,&[f.pages[1]],&mut ||Ok(())).is_err(),"deleting an original ID remains distinct from excluding it from a copy");
}

#[test]
fn projection_structure_copy_plan_keeps_partial_description_guards_and_cancellation() {
    let mut f=fixture();let original=f.doc.objects.clone();let info=structure(&f.doc);
    assert!(structure_for_copy(&f.doc,&info,f.pages[0],&mut SelectionBudget::default(),&mut ||Err(bad("cancel"))).is_err());assert_eq!(f.doc.objects,original);
    f.doc.get_dictionary_mut(f.group).unwrap().set("ActualText",Object::string_literal("Description spans three pages"));
    let info=structure(&f.doc);
    assert!(structure_for_copy(&f.doc,&info,f.pages[1],&mut SelectionBudget::default(),&mut ||Ok(())).is_err());
}

#[test]
fn projection_permutation_keeps_logical_order_streams_and_inherited_geometry() {
    let f=fixture();let streams=stream_bytes(&f.doc);let before=structure(&f.doc);
    let expected=vec![f.pages[2],f.pages[0],f.pages[1]];
    let out=project(f.doc,&expected,&mut ||Ok(())).unwrap();
    assert_eq!(crate::composition_pages::inspect(&out,f.catalog,&mut ||Ok(())).unwrap().pages,expected);
    assert_eq!(structure(&out),before);assert_eq!(stream_bytes(&out),streams);
    for page in &expected {
        let p=out.get_dictionary(*page).unwrap();
        assert_eq!(p.get(b"Rotate").unwrap(),&Object::Integer(90));
        assert_eq!(p.get(b"MediaBox").unwrap().as_array().unwrap()[0],Object::Integer(10));
        assert_eq!(p.get(b"CropBox").unwrap().as_array().unwrap()[0],Object::Integer(20));
        assert_eq!(p.get(b"UserUnit").unwrap(),&Object::Real(1.7));
        let r=out.dereference(p.get(b"Resources").unwrap()).unwrap().1.as_dict().unwrap();
        assert_eq!(r.get(b"Font").unwrap().as_dict().unwrap().get(b"F1").unwrap().as_reference().unwrap(),f.font);
    }
}

#[test]
fn projection_subset_prunes_ancestors_reverse_slots_and_unreachable_page_payloads() {
    let f=fixture();let expected=vec![f.pages[1]];let streams=stream_bytes(&f.doc);
    let out=project(f.doc,&expected,&mut ||Ok(())).unwrap();let info=structure(&out);
    assert_eq!(info.elements,std::collections::BTreeSet::from([f.group,f.elements[1]]));
    assert_eq!(info.owners.len(),3);assert_eq!(info.owners[&1].parents,crate::composition_structure::Parents::Marked(vec![None,None,Some(f.elements[1])]));
    assert_eq!(info.owners[&6].object,f.note);assert_eq!(info.owners[&7].object,f.form);
    assert!(!out.get_dictionary(f.group).unwrap().has(b"Pg"));
    assert!(!out.objects.contains_key(&f.pages[0]));assert!(!out.objects.contains_key(&f.elements[0]));
    for (id,bytes) in stream_bytes(&out){assert_eq!(bytes,streams[&id]);}
    assert_eq!(out.get_dictionary(f.elements[1]).unwrap().get(b"Alt").unwrap().as_str().unwrap(),b"Page 1");
}

#[test]
fn projection_refuses_opaque_references_and_partial_alternate_descriptions() {
    for fault in ["page","element","alias","alt","actual","expansion"] {
        let mut f=fixture();
        match fault {
            "page"=>{f.doc.get_dictionary_mut(f.note).unwrap().set("Private",f.pages[0]);},
            "element"=>{f.doc.get_dictionary_mut(f.note).unwrap().set("Private",f.elements[0]);},
            "alias"=>{let alias=f.doc.add_object(Object::Reference(f.pages[0]));f.doc.get_dictionary_mut(f.note).unwrap().set("Private",alias);},
            key=>{f.doc.get_dictionary_mut(f.group).unwrap().set(match key{"alt"=>"Alt","actual"=>"ActualText",_=>"E"},Object::string_literal("all pages together"));},
        }
        assert!(project(f.doc,&[f.pages[1]],&mut ||Ok(())).is_err(),"{fault}");
    }
}

#[test]
fn projection_null_inherited_values_and_aliases_keep_nearest_nonnull_ancestor() {
    let mut f=fixture();let null=f.doc.add_object(Object::Null);
    f.doc.get_dictionary_mut(f.pages[0]).unwrap().set("Resources",null);
    f.doc.get_dictionary_mut(f.pages[0]).unwrap().set("MediaBox",Object::Null);
    let out=project(f.doc,&[f.pages[0]],&mut ||Ok(())).unwrap();
    assert!(out.get_dictionary(f.pages[0]).unwrap().get(b"Resources").is_ok());
    assert!(out.get_dictionary(f.pages[0]).unwrap().get(b"MediaBox").unwrap().as_array().is_ok());
}

#[test]
fn projection_rejects_empty_duplicate_foreign_and_cancelled_requests() {
    for case in 0..4 {
        let f=fixture();let pages=match case{0=>vec![],1=>vec![f.pages[0],f.pages[0]],2=>vec![f.font],_=>vec![f.pages[1]]};
        assert!(project(f.doc,&pages,&mut ||if case==3{Err(EngineError::InvalidRequest("cancelled".into()))}else{Ok(())}).is_err());
    }
}

#[test]
fn projection_drops_empty_elements_bound_to_removed_pages_but_keeps_unbound_metadata() {
    let mut f=fixture();
    let removed=f.doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"Span","P"=>f.root,"Pg"=>f.pages[0],"K"=>Vec::<Object>::new(),"Alt"=>Object::string_literal("empty first page")});
    let retained=f.doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"Span","P"=>f.root,"Alt"=>Object::string_literal("document metadata")});
    f.doc.get_dictionary_mut(f.root).unwrap().set("K",vec![Object::Reference(f.group),removed.into(),retained.into()]);
    let out=project(f.doc,&[f.pages[1]],&mut ||Ok(())).unwrap();
    let info=structure(&out);assert!(!info.elements.contains(&removed));assert!(info.elements.contains(&retained));
}

#[test]
fn projection_shared_marked_form_keeps_only_surviving_occurrence_and_exact_parent() {
    let mut f=fixture();
    f.doc.get_dictionary_mut(f.pages[0]).unwrap().set("Resources",dictionary!{"Font"=>dictionary!{"F1"=>f.font},"XObject"=>dictionary!{"Fm"=>f.form}});
    f.doc.get_dictionary_mut(f.elements[1]).unwrap().remove(b"Alt");
    f.doc.get_dictionary_mut(f.elements[1]).unwrap().get_mut(b"K").unwrap().as_array_mut().unwrap().push(Object::Dictionary(dictionary!{"Type"=>"MCR","Pg"=>f.pages[0],"Stm"=>f.form,"MCID"=>0}));
    assert_eq!(structure(&f.doc).owners[&7].object,f.form);
    let out=project(f.doc,&[f.pages[0]],&mut ||Ok(())).unwrap();
    let info=structure(&out);assert_eq!(info.owners.len(),2);
    assert_eq!(info.owners[&7].parents,crate::composition_structure::Parents::Marked(vec![Some(f.elements[1])]));
    assert!(!out.objects.contains_key(&f.note));assert!(!out.get_dictionary(f.elements[1]).unwrap().has(b"Pg"));
}

#[test]
fn projection_rejects_structure_id_tree_that_still_addresses_removed_element() {
    let mut f=fixture();
    f.doc.get_dictionary_mut(f.root).unwrap().set("IDTree",dictionary!{"Names"=>vec![Object::string_literal("first"),f.elements[0].into()]});
    project(f.doc.clone(),&[f.pages[1],f.pages[0]],&mut ||Ok(())).expect("the same ID tree is valid when its target survives");
    let error=project(f.doc,&[f.pages[1]],&mut ||Ok(())).unwrap_err();
    // Final structure validation can detect the dangling logical target before
    // the reachability pass, which now follows all action allocations.
    let message=error.to_string();
    assert!(message.contains("removed page or tag") || message.contains("PDF structure ownership"),"{error}");
}

#[test]
fn projection_checks_cancellation_through_validation_and_pruning() {
    let f=fixture();let mut total=0;
    project(f.doc,&[f.pages[1]],&mut ||{total+=1;Ok(())}).unwrap();
    assert!(total>50);
    for stop in [0,1,total/4,total/2,total-2,total-1] {
        let f=fixture();let mut calls=0;
        let error=project(f.doc,&[f.pages[1]],&mut ||{calls+=1;if calls>stop{Err(EngineError::InvalidRequest("projection cancelled".into()))}else{Ok(())}}).unwrap_err();
        assert!(error.to_string().contains("projection cancelled"),"checkpoint {stop}: {error}");
    }
}

#[test]
fn projection_refuses_large_direct_inherited_metadata_before_copying_it() {
    let mut f=fixture();
    f.doc.get_dictionary_mut(f.pages[0]).unwrap().set("Resources",dictionary!{"Private"=>Object::String(vec![b'x';17*1024*1024],lopdf::StringFormat::Literal)});
    let error=project(f.doc,&[f.pages[0]],&mut ||Ok(())).unwrap_err();
    assert!(error.to_string().contains("metadata budget"),"{error}");
}

#[test]
fn projection_refuses_to_discard_unknown_page_branch_metadata() {
    let mut f=fixture();let branch=f.doc.get_dictionary(f.pages[0]).unwrap().get(b"Parent").unwrap().as_reference().unwrap();
    f.doc.get_dictionary_mut(branch).unwrap().set("PrivateProducerData",Object::string_literal("must not silently disappear"));
    let error=project(f.doc,&[f.pages[1]],&mut ||Ok(())).unwrap_err();
    assert!(error.to_string().contains("page branch metadata"),"{error}");
}

#[test]
fn projection_refuses_unadapted_attributes_and_removed_string_identifiers() {
    for mode in ["attributes","class","identifier"] {
        let mut f=fixture();
        match mode {
            "attributes"=>{f.doc.get_dictionary_mut(f.group).unwrap().set("A",dictionary!{"O"=>"Table","RowSpan"=>3});},
            "class"=>{f.doc.get_dictionary_mut(f.root).unwrap().set("ClassMap",dictionary!{"Rows"=>dictionary!{"O"=>"Table","RowSpan"=>3}});f.doc.get_dictionary_mut(f.group).unwrap().set("C",Object::Name(b"Rows".to_vec()));},
            _=>{f.doc.get_dictionary_mut(f.elements[0]).unwrap().set("ID",Object::string_literal("removed-header"));f.doc.get_dictionary_mut(f.elements[1]).unwrap().set("A",dictionary!{"O"=>"Table","Headers"=>vec![Object::string_literal("removed-header")]});},
        }
        assert!(project(f.doc,&[f.pages[1]],&mut ||Ok(())).is_err(),"{mode}");
    }
}

#[test]
fn projection_deleted_navigation_resolves_aliases_and_refuses_opaque_removed_outline_references() {
    for opaque in [false,true] {
        let mut f=fixture();let root=f.doc.new_object_id();let page_alias=f.doc.add_object(Object::Reference(f.pages[0]));
        let target=f.doc.add_object(vec![Object::Reference(page_alias),"Fit".into()]);let target_alias=f.doc.add_object(Object::Reference(target));
        let name=f.doc.add_object(Object::Name(b"gone".to_vec()));let name_alias=f.doc.add_object(Object::Reference(name));
        let item=f.doc.add_object(dictionary!{"Title"=>Object::string_literal("gone"),"Parent"=>root,"Dest"=>name_alias});
        f.doc.objects.insert(root,dictionary!{"Type"=>"Outlines","Count"=>1,"First"=>item,"Last"=>item}.into());
        f.doc.get_dictionary_mut(f.catalog).unwrap().set("Outlines",root);f.doc.get_dictionary_mut(f.catalog).unwrap().set("Dests",dictionary!{"gone"=>target_alias});
        let link=f.doc.add_object(dictionary!{"Subtype"=>"Link","P"=>f.pages[1],"Rect"=>vec![0.into(),0.into(),10.into(),10.into()],"Dest"=>name_alias});
        f.doc.get_dictionary_mut(f.pages[1]).unwrap().set("Annots",vec![Object::Reference(f.note),link.into()]);
        if opaque {let alias=f.doc.add_object(Object::Reference(item));f.doc.get_dictionary_mut(f.pages[1]).unwrap().set("PrivateBookmark",alias);}
        let result=project(f.doc,&[f.pages[1]],&mut ||Ok(()));
        if opaque {assert!(result.is_err());}else{
            let out=result.unwrap();assert!(!out.get_dictionary(link).unwrap().has(b"Dest"));assert!(!out.objects.contains_key(&item));
            let outline=crate::composition_outlines::inspect(&out,f.catalog,&structure(&out),&mut ||Ok(())).unwrap();assert!(outline.items.is_empty());
        }
    }
}

#[test]
fn projection_occurrences_keep_indirect_page_edges_and_sparse_record_arrays_independent() {
    let mut f=fixture();
    let page_alias=f.doc.add_object(Object::Reference(f.pages[1]));
    let mcr_type=f.doc.add_object(Object::Name(b"MCR".to_vec()));
    let records=f.doc.add_object(Object::Array(vec![Object::Dictionary(dictionary!{"Type"=>mcr_type,"Pg"=>page_alias,"MCID"=>2}),Object::Dictionary(dictionary!{"Type"=>"OBJR","Pg"=>page_alias,"Obj"=>f.note}),Object::Dictionary(dictionary!{"Type"=>"MCR","Pg"=>page_alias,"Stm"=>f.form,"StmOwn"=>page_alias,"MCID"=>0})]));
    f.doc.get_dictionary_mut(f.elements[1]).unwrap().set("K",records);
    f.doc.get_dictionary_mut(f.elements[1]).unwrap().set("Pg",page_alias);
    f.doc.get_dictionary_mut(f.note).unwrap().set("P",page_alias);
    let order=vec![f.pages[0],f.pages[1],f.pages[1],f.pages[2],f.pages[1]];
    let (out,ids)=crate::composition_occurrences::project(f.doc,&order,&mut ||Ok(())).unwrap();
    let info=structure(&out);assert_eq!(info.owners.len(),11);
    for copy in [ids[2],ids[4]] {
        let element=info.owners.values().find_map(|owner|if owner.object==copy {match &owner.parents{Parents::Marked(p)=>p[2],_=>None}}else{None}).unwrap();
        assert_eq!(out.get_dictionary(element).unwrap().get(b"Pg").unwrap(),&Object::Reference(copy));
        let array=out.get_dictionary(element).unwrap().get(b"K").unwrap().as_reference().unwrap();assert_ne!(array,records);
        for record in out.get_object(array).unwrap().as_array().unwrap(){let d=record.as_dict().unwrap();assert_eq!(d.get(b"Pg").unwrap(),&Object::Reference(copy));if d.has(b"StmOwn"){assert_eq!(d.get(b"StmOwn").unwrap(),&Object::Reference(copy));}}
        let note=out.get_dictionary(copy).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap();
        assert_eq!(out.get_dictionary(note).unwrap().get(b"P").unwrap(),&Object::Reference(copy));
    }
    assert_eq!(out.get_dictionary(f.elements[1]).unwrap().get(b"K").unwrap(),&Object::Reference(records));
}

#[test]
fn projection_occurrences_reconcile_unique_ids_and_popup_relations_without_changing_originals() {
    let mut f=fixture();let mut names=vec![];
    for (i,id) in f.elements.iter().enumerate(){let name=Object::string_literal(format!("item-{i}"));f.doc.get_dictionary_mut(*id).unwrap().set("ID",name.clone());names.extend([name,Object::Reference(*id)]);}
    f.doc.get_dictionary_mut(f.root).unwrap().set("IDTree",dictionary!{"Names"=>names});
    let popup=f.doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Popup","P"=>f.pages[1],"Parent"=>f.note,"Rect"=>vec![0.into(),0.into(),10.into(),10.into()]});
    f.doc.get_dictionary_mut(f.note).unwrap().set("Popup",popup);
    let array=f.doc.add_object(Object::Array(vec![f.note.into(),popup.into()]));let alias=f.doc.add_object(Object::Reference(array));
    f.doc.get_dictionary_mut(f.pages[1]).unwrap().set("Annots",alias);
    let (out,ids)=crate::composition_occurrences::project(f.doc,&[f.pages[0],f.pages[1],f.pages[1],f.pages[2]],&mut ||Ok(())).unwrap();
    let a=out.dereference(out.get_dictionary(ids[2]).unwrap().get(b"Annots").unwrap()).unwrap().1.as_array().unwrap();
    let note=a[0].as_reference().unwrap();let copy_popup=a[1].as_reference().unwrap();assert_ne!(note,f.note);assert_ne!(copy_popup,popup);
    assert_eq!(out.get_dictionary(note).unwrap().get(b"Popup").unwrap(),&Object::Reference(copy_popup));assert_eq!(out.get_dictionary(copy_popup).unwrap().get(b"Parent").unwrap(),&Object::Reference(note));
    assert_eq!(out.get_dictionary(copy_popup).unwrap().get(b"P").unwrap(),&Object::Reference(ids[2]));
    assert_eq!(out.get_dictionary(f.note).unwrap().get(b"Popup").unwrap(),&Object::Reference(popup));
    let info=structure(&out);let identifiers=info.elements.iter().filter_map(|id|out.get_dictionary(*id).unwrap().get(b"ID").ok()).map(|v|v.as_str().unwrap().to_vec()).collect::<BTreeSet<_>>();assert_eq!(identifiers.len(),4);
}

#[test]
fn projection_occurrences_retarget_only_copy_local_links_and_keep_remote_action_sharing() {
    let mut f=fixture();
    let remote=f.doc.add_object(dictionary!{"S"=>"GoToR","F"=>Object::string_literal("other.pdf"),"D"=>Object::Name(b"chapter".to_vec())});
    let action=f.doc.add_object(dictionary!{"S"=>"GoTo","D"=>Object::Name(b"chapter".to_vec()),"Next"=>remote,"Private"=>77});
    let link=f.doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Link","P"=>f.pages[1],"Rect"=>vec![0.into(),0.into(),10.into(),10.into()],"A"=>action});
    let cross=f.doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Link","P"=>f.pages[1],"Rect"=>vec![0.into(),0.into(),10.into(),10.into()],"Dest"=>Object::string_literal("chapter")});
    let explicit=f.doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Link","P"=>f.pages[1],"Rect"=>vec![0.into(),0.into(),10.into(),10.into()],"Dest"=>vec![Object::Reference(f.pages[1]),"XYZ".into(),17.into(),Object::Null,0.into()]});
    f.doc.get_dictionary_mut(f.pages[1]).unwrap().set("Annots",vec![f.note.into(),link.into(),cross.into(),explicit.into()]);
    f.doc.get_dictionary_mut(f.pages[1]).unwrap().set("AA",dictionary!{"O"=>action});
    let named=f.doc.add_object(dictionary!{"D"=>vec![Object::Reference(f.pages[1]),"Fit".into()],"Private"=>19});
    f.doc.get_dictionary_mut(f.catalog).unwrap().set("Dests",dictionary!{"chapter"=>named});
    f.doc.get_dictionary_mut(f.catalog).unwrap().set("Names",dictionary!{"Dests"=>dictionary!{"Names"=>vec![Object::string_literal("chapter"),Object::Array(vec![f.pages[0].into(),"FitH".into(),23.into()])]}});
    let order=vec![f.pages[0],f.pages[1],f.pages[1],f.pages[2],f.pages[1]];
    let (out,ids)=crate::composition_occurrences::project(f.doc,&order,&mut ||Ok(())).unwrap();
    let pages=crate::composition_pages::inspect(&out,f.catalog,&mut ||Ok(())).unwrap();
    let mut destinations=crate::composition_destinations::prepare(&out,&[pages],&mut ||Ok(())).unwrap();
    let mut copied_actions=BTreeSet::new();let mut copied_names=BTreeSet::new();
    for copy in [ids[2],ids[4]] {
        let page=out.get_dictionary(copy).unwrap();let annots=out.dereference(page.get(b"Annots").unwrap()).unwrap().1.as_array().unwrap();
        let l=out.dereference(&annots[1]).unwrap().1.as_dict().unwrap();let a=l.get(b"A").unwrap().as_reference().unwrap();assert_ne!(a,action);assert!(copied_actions.insert(a));
        let aa=out.dereference(page.get(b"AA").unwrap()).unwrap().1.as_dict().unwrap();assert_eq!(aa.get(b"O").unwrap(),&Object::Reference(a));
        let a=out.get_dictionary(a).unwrap();assert_eq!(a.get(b"Next").unwrap(),&Object::Reference(remote));assert_eq!(a.get(b"Private").unwrap(),&Object::Integer(77));
        let name=a.get(b"D").unwrap();assert_eq!(destinations.target(0,name,&mut ||Ok(())).unwrap(),copy);let key=name.as_name().unwrap();assert_ne!(key,b"chapter");assert!(copied_names.insert(key.to_vec()));
        let entry=out.get_dictionary(f.catalog).unwrap().get(b"Dests").unwrap().as_dict().unwrap().get(key).unwrap().as_dict().unwrap();assert_eq!(entry.get(b"Private").unwrap(),&Object::Integer(19));
        let cross=out.dereference(&annots[2]).unwrap().1.as_dict().unwrap();assert_eq!(cross.get(b"Dest").unwrap(),&Object::string_literal("chapter"));assert_eq!(destinations.target(0,cross.get(b"Dest").unwrap(),&mut ||Ok(())).unwrap(),ids[0]);
        let explicit=out.dereference(&annots[3]).unwrap().1.as_dict().unwrap();assert_eq!(explicit.get(b"Dest").unwrap(),&Object::Array(vec![copy.into(),"XYZ".into(),17.into(),Object::Null,0.into()]));
    }
    assert_eq!(out.get_dictionary(action).unwrap().get(b"D").unwrap(),&Object::Name(b"chapter".to_vec()));
    assert_eq!(out.get_dictionary(remote).unwrap().get(b"D").unwrap(),&Object::Name(b"chapter".to_vec()));
    assert_eq!(out.get_dictionary(f.catalog).unwrap().get(b"Dests").unwrap().as_dict().unwrap().get(b"chapter").unwrap(),&Object::Reference(named));
}

#[test]
fn projection_occurrences_do_not_clone_discarded_structure_containers() {
    let mut f=fixture();let old_k=f.doc.add_object(Object::Array(f.elements.iter().copied().map(Object::Reference).collect()));
    f.doc.get_dictionary_mut(f.group).unwrap().set("K",old_k);
    let order=vec![f.pages[0],f.pages[1],f.pages[1],f.pages[2]];
    let (out,_)=crate::composition_occurrences::project(f.doc,&order,&mut ||Ok(())).unwrap();
    let arrays=out.objects.values().filter(|v|matches!(v,Object::Array(items) if items.len()==3 && items.iter().all(|v|matches!(v,Object::Reference(_))))).count();
    assert_eq!(arrays,1,"filtered copy K must not also retain an unused clone of the original multi-page K array");
}

#[test]
fn projection_occurrences_refuse_external_reply_ownership_but_keep_page_local_replies() {
    for external in [false,true] {
        let mut f=fixture();
        let reply=f.doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![0.into(),0.into(),10.into(),10.into()],"Contents"=>Object::string_literal("reply target")});
        f.doc.get_dictionary_mut(f.note).unwrap().set("IRT",reply);
        let owner=if external{f.pages[0]}else{f.pages[1]};
        if external {f.doc.get_dictionary_mut(owner).unwrap().set("Annots",vec![Object::Reference(reply)]);}
        else {f.doc.get_dictionary_mut(owner).unwrap().set("Annots",vec![Object::Reference(f.note),reply.into()]);}
        let order=vec![f.pages[0],f.pages[1],f.pages[1],f.pages[2]];
        let result=crate::composition_occurrences::project(f.doc,&order,&mut ||Ok(()));
        if external {assert!(result.is_err(),"a reply target owned by another page cannot be silently cloned as an unattached annotation");}
        else {let (out,ids)=result.unwrap();let annotations=out.get_dictionary(ids[2]).unwrap().get(b"Annots").unwrap().as_array().unwrap();let note=annotations[0].as_reference().unwrap();assert_eq!(out.get_dictionary(note).unwrap().get(b"IRT").unwrap(),&annotations[1]);assert_ne!(annotations[1],Object::Reference(reply));}
    }
}

#[test]
fn projection_occurrences_copy_names_keep_operand_namespaces_and_avoid_existing_bindings() {
    let mut f=fixture();
    let link=f.doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Link","Rect"=>vec![0.into(),0.into(),10.into(),10.into()],"Dest"=>Object::Name(b"same".to_vec()),"AA"=>dictionary!{"E"=>dictionary!{"S"=>"GoTo","D"=>Object::string_literal("same")}}});
    f.doc.get_dictionary_mut(f.pages[1]).unwrap().set("Annots",vec![f.note.into(),link.into()]);
    f.doc.get_dictionary_mut(f.catalog).unwrap().set("Dests",dictionary!{"same"=>vec![Object::Reference(f.pages[1]),"Fit".into()]});
    f.doc.get_dictionary_mut(f.catalog).unwrap().set("Names",dictionary!{"Dests"=>dictionary!{"Names"=>vec![Object::string_literal("same"),Object::Array(vec![f.pages[1].into(),"FitH".into(),29.into()])]}});
    let order=vec![f.pages[0],f.pages[1],f.pages[1],f.pages[2]];
    let (_,ids)=crate::composition_occurrences::project(f.doc.clone(),&order,&mut ||Ok(())).unwrap();
    let reserved=format!("Folio.Copy.{}.{}.1",ids[2].0,ids[2].1).into_bytes();
    f.doc.get_dictionary_mut(f.catalog).unwrap().get_mut(b"Dests").unwrap().as_dict_mut().unwrap().set(reserved.clone(),vec![Object::Reference(f.pages[0]),"Fit".into()]);
    let (out,new_ids)=crate::composition_occurrences::project(f.doc,&order,&mut ||Ok(())).unwrap();assert_eq!(ids,new_ids);
    let annotations=out.get_dictionary(ids[2]).unwrap().get(b"Annots").unwrap().as_array().unwrap();let link=out.dereference(&annotations[1]).unwrap().1.as_dict().unwrap();
    let local_name=link.get(b"Dest").unwrap().as_name().unwrap();assert_ne!(local_name,reserved);
    let aa=out.dereference(link.get(b"AA").unwrap()).unwrap().1.as_dict().unwrap();let action=out.dereference(aa.get(b"E").unwrap()).unwrap().1.as_dict().unwrap();
    let local_string=action.get(b"D").unwrap();assert!(matches!(local_string,Object::String(..)));assert_eq!(local_string.as_str().unwrap(),reserved);
    let pages=crate::composition_pages::inspect(&out,f.catalog,&mut ||Ok(())).unwrap();let mut names=crate::composition_destinations::prepare(&out,&[pages],&mut ||Ok(())).unwrap();
    assert_eq!(names.target(0,link.get(b"Dest").unwrap(),&mut ||Ok(())).unwrap(),ids[2]);assert_eq!(names.target(0,local_string,&mut ||Ok(())).unwrap(),ids[2]);
    assert_eq!(names.target(0,&Object::Name(reserved),&mut ||Ok(())).unwrap(),ids[0]);
}

#[test]
fn projection_occurrences_keep_table_headers_local_through_indirect_attributes_and_classes() {
    for mode in ["direct","dictionary","stream","array","class"] {
        let mut f=fixture();let header=f.doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"TH","P"=>f.group,"Pg"=>f.pages[1],"K"=>4,"ID"=>Object::string_literal("header")});
        f.doc.get_dictionary_mut(f.elements[1]).unwrap().set("S","TD");
        f.doc.get_dictionary_mut(f.group).unwrap().set("K",vec![f.elements[0].into(),header.into(),f.elements[1].into(),f.elements[2].into()]);
        let root=f.doc.get_dictionary_mut(f.root).unwrap();root.set("IDTree",dictionary!{"Names"=>vec![Object::string_literal("header"),Object::Reference(header)]});
        let nums=root.get_mut(b"ParentTree").unwrap().as_dict_mut().unwrap().get_mut(b"Nums").unwrap().as_array_mut().unwrap();nums[3]=Object::Array(vec![Object::Null,Object::Null,f.elements[1].into(),Object::Null,header.into()]);
        let contents=f.doc.get_dictionary(f.pages[1]).unwrap().get(b"Contents").unwrap().as_reference().unwrap();let stream=f.doc.get_object_mut(contents).unwrap().as_stream_mut().unwrap();stream.content.extend_from_slice(b" /TH <</MCID 4>> BDC BT /F1 12 Tf 20 60 Td (Header) Tj ET EMC");stream.dict.set("Length",stream.content.len() as i64);
        let attr=dictionary!{"O"=>"Table","Headers"=>vec![Object::string_literal("header")],"RowSpan"=>2};
        let value=match mode {
            "direct"=>Object::Dictionary(attr),
            "stream"=>f.doc.add_object(Stream::new(attr,b"attribute bytes".to_vec())).into(),
            "array"=>{let array=f.doc.add_object(Object::Array(vec![Object::Dictionary(attr),3.into()]));let alias=f.doc.add_object(Object::Reference(array));alias.into()},
            _=>{let object=f.doc.add_object(attr);let alias=f.doc.add_object(Object::Reference(object));alias.into()},
        };
        if mode=="class" {
            f.doc.get_dictionary_mut(f.elements[1]).unwrap().set("A",value.clone());
            f.doc.get_dictionary_mut(f.root).unwrap().set("ClassMap",dictionary!{"HeadersClass"=>value});f.doc.get_dictionary_mut(f.elements[1]).unwrap().set("C","HeadersClass");
        }else{f.doc.get_dictionary_mut(f.elements[1]).unwrap().set("A",value);}
        let output=std::env::var_os("FOLIO_OCCURRENCE_HEADERS_OUTPUT").map(std::path::PathBuf::from);
        if let Some(root)=&output {let path=root.join(format!("{mode}-source.pdf"));assert!(!path.exists());f.doc.clone().save(&path).unwrap();}
        let order=vec![f.pages[0],f.pages[1],f.pages[1],f.pages[2],f.pages[1]];
        let (out,ids)=crate::composition_occurrences::project(f.doc,&order,&mut ||Ok(())).unwrap();let info=structure(&out);let mut seen_attributes=BTreeSet::new();let mut seen_headers=BTreeSet::new();
        for page in [ids[1],ids[2],ids[4]] {
            let owner=info.owners.values().find(|owner|owner.object==page).unwrap();let Parents::Marked(slots)=&owner.parents else{panic!("page owner")};let head=slots[4].unwrap();let cell=out.get_dictionary(slots[2].unwrap()).unwrap();
            let value=if mode=="class" {let root=out.get_dictionary(info.root.unwrap()).unwrap();root.get(b"ClassMap").unwrap().as_dict().unwrap().get(cell.get(b"C").unwrap().as_name().unwrap()).unwrap()}else{cell.get(b"A").unwrap()};
            if mode=="class"{assert_eq!(cell.get(b"A").unwrap(),value,"A and ClassMap aliases must remain shared within an occurrence");}
            let (id,mut value)=out.dereference(value).unwrap();if let Some(id)=id{assert!(seen_attributes.insert(id),"{mode}: mutable attribute container is shared");}
            if let Object::Array(values)=value {assert_eq!(values[1],Object::Integer(3));value=out.dereference(&values[0]).unwrap().1;}
            let dictionary=match value{Object::Dictionary(d)=>d,Object::Stream(s)=>{assert_eq!(s.content,b"attribute bytes");&s.dict},_=>panic!("attribute")};
            let actual=dictionary.get(b"Headers").unwrap().as_array().unwrap()[0].as_str().unwrap();let expected=out.get_dictionary(head).unwrap().get(b"ID").unwrap().as_str().unwrap();
            assert_eq!(actual,expected,"{mode}: header must bind to the same page occurrence");assert!(seen_headers.insert(actual.to_vec()));assert_eq!(dictionary.get(b"RowSpan").unwrap(),&Object::Integer(2));
        }
        if let Some(root)=output {let path=root.join(format!("{mode}-copied.pdf"));assert!(!path.exists());let mut out=out;out.save(path).unwrap();}
    }
}

#[test]
fn projection_occurrences_keep_primary_reply_actions_independent_of_annotation_order() {
    for target_first in [false,true] {
        let mut f=fixture();let action=f.doc.add_object(dictionary!{"S"=>"GoTo","D"=>vec![Object::Reference(f.pages[1]),"Fit".into()]});
        let reply=f.doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","P"=>f.pages[1],"Rect"=>vec![0.into(),0.into(),10.into(),10.into()],"AA"=>dictionary!{"E"=>action}});
        f.doc.get_dictionary_mut(f.note).unwrap().set("IRT",reply);
        let annotations=if target_first{vec![reply.into(),Object::Reference(f.note)]}else{vec![Object::Reference(f.note),reply.into()]};f.doc.get_dictionary_mut(f.pages[1]).unwrap().set("Annots",annotations);
        let order=vec![f.pages[0],f.pages[1],f.pages[1],f.pages[2]];let (out,ids)=crate::composition_occurrences::project(f.doc,&order,&mut ||Ok(())).unwrap();
        let a=out.get_dictionary(ids[2]).unwrap().get(b"Annots").unwrap().as_array().unwrap();let (note,reply)=if target_first{(a[1].as_reference().unwrap(),a[0].as_reference().unwrap())}else{(a[0].as_reference().unwrap(),a[1].as_reference().unwrap())};
        assert_eq!(out.get_dictionary(note).unwrap().get(b"IRT").unwrap(),&Object::Reference(reply));
        let aa=out.dereference(out.get_dictionary(reply).unwrap().get(b"AA").unwrap()).unwrap().1.as_dict().unwrap();let copied=out.dereference(aa.get(b"E").unwrap()).unwrap().1.as_dict().unwrap();
        assert_eq!(copied.get(b"D").unwrap().as_array().unwrap()[0],Object::Reference(ids[2]));assert_eq!(out.get_dictionary(action).unwrap().get(b"D").unwrap().as_array().unwrap()[0],Object::Reference(ids[1]));
    }
}

#[test]
fn projection_occurrences_keep_proved_empty_nodes_and_refuse_unbound_descriptions() {
    for mode in ["page","inherited","unbound"] {
        let mut f=fixture();let mut empty=dictionary!{"Type"=>"StructElem","S"=>"P","P"=>if mode=="inherited"{f.elements[1]}else{f.group},"Alt"=>Object::string_literal("Empty description")};
        if mode=="page"{empty.set("Pg",f.pages[1]);}
        let empty=f.doc.add_object(empty);
        if mode=="inherited"{f.doc.get_dictionary_mut(f.elements[1]).unwrap().get_mut(b"K").unwrap().as_array_mut().unwrap().push(empty.into());}
        else {f.doc.get_dictionary_mut(f.group).unwrap().get_mut(b"K").unwrap().as_array_mut().unwrap().push(empty.into());}
        if mode=="unbound"{f.doc.get_dictionary_mut(f.group).unwrap().remove(b"Pg");}
        let order=vec![f.pages[0],f.pages[1],f.pages[1],f.pages[2]];let result=crate::composition_occurrences::project(f.doc,&order,&mut ||Ok(()));
        if mode=="unbound"{assert!(result.is_err(),"do not guess a copied page for a document-level empty description");}
        else {let (out,_)=result.unwrap();let info=structure(&out);assert_eq!(info.elements.iter().filter(|id|out.get_dictionary(**id).unwrap().get(b"Alt").is_ok_and(|v|v.as_str().is_ok_and(|s|s==b"Empty description"))).count(),2);}
    }
}
