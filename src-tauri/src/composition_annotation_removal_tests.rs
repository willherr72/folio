use super::*;
use crate::{composition_annotation_receipts::{Anchor, Receipt}, composition_occurrences};
use lopdf::{dictionary, Object, ObjectId, Stream};

fn fixture() -> (Document, [ObjectId;2], ObjectId, ObjectId, ObjectId) {
    let mut doc=Document::with_version("1.7");let tree=doc.new_object_id();
    let content=doc.add_object(Stream::new(dictionary!{},b"0 0 1 rg 10 10 30 30 re f".to_vec()));
    let pages=[doc.new_object_id(),doc.new_object_id()];
    for page in pages {doc.objects.insert(page,dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary!{},"Contents"=>content}.into());}
    let note=doc.new_object_id();let popup=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Popup","Rect"=>vec![80.into(),80.into(),180.into(),160.into()],"Parent"=>note});
    let appearance=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),20.into(),20.into()]},b"1 1 0 rg 0 0 20 20 re f".to_vec()));
    doc.objects.insert(note,dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![40.into(),40.into(),60.into(),60.into()],"Popup"=>popup,"Contents"=>Object::string_literal("owned"),"AP"=>dictionary!{"N"=>appearance},"NM"=>Object::string_literal("duplicate label")}.into());
    let other=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![200.into(),40.into(),220.into(),60.into()],"Contents"=>Object::string_literal("unrelated"),"NM"=>Object::string_literal("duplicate label"),"AP"=>dictionary!{"N"=>appearance}});
    let alias=doc.add_object(Object::Reference(note));let list=doc.add_object(vec![Object::Reference(alias),Object::Reference(popup),Object::Reference(other)]);
    doc.get_dictionary_mut(pages[0]).unwrap().set("Annots",list);
    doc.objects.insert(tree,dictionary!{"Type"=>"Pages","Count"=>2,"Kids"=>pages.into_iter().map(Object::Reference).collect::<Vec<_>>()}.into());
    let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree});doc.trailer.set("Root",catalog);
    (doc,pages,note,popup,other)
}
fn receipt(page:ObjectId,slot:usize,id:ObjectId)->Receipt {
    Receipt{origin:Anchor{page,slot,annotation:id},output_page:page,output_slot:slot,annotation:id}
}
fn ids(doc:&Document,page:ObjectId)->Vec<ObjectId> {
    let dict=doc.get_dictionary(page).unwrap();let Ok(raw)=dict.get(b"Annots") else{return vec![]};
    doc.dereference(raw).unwrap().1.as_array().unwrap().iter().map(|v|doc.dereference(v).unwrap().0.unwrap()).collect()
}
#[test]
fn annotation_removal_copied_note_and_popup_leave_other_occurrence_and_resources_exact() {
    let (doc,pages,note,popup,_)=fixture();
    let anchors=[Anchor{page:pages[0],slot:0,annotation:note},Anchor{page:pages[0],slot:1,annotation:popup}];
    let (doc,pages,receipts)=composition_occurrences::project_forms_tracked(doc,&[pages[0],pages[0]],&anchors,&mut ||Ok(())).unwrap();
    let before=doc.objects.clone();let untouched=ids(&doc,pages[0]);let keep=ids(&doc,pages[1])[2];
    let result=remove_owned(doc,&[Removal{annotation:receipts[2],popups:&receipts[3..4]}],&mut ||Ok(())).unwrap();
    assert_eq!(ids(&result,pages[0]),untouched);assert_eq!(ids(&result,pages[1]),vec![keep]);
    for (id,value) in before {if id!=pages[1]{assert_eq!(result.objects.get(&id),Some(&value),"unrelated object {id:?} changed");}}
    let mut saved=result;let mut bytes=vec![];saved.save_to(&mut bytes).unwrap();let reopened=Document::load_mem(&bytes).unwrap();
    assert_eq!(ids(&reopened,pages[0]),untouched);assert_eq!(ids(&reopened,pages[1]),vec![keep]);
}
#[test]
fn annotation_removal_shared_original_array_is_detached_only_for_requested_page() {
    let (mut doc,pages,note,popup,other)=fixture();
    let raw=doc.get_dictionary(pages[0]).unwrap().get(b"Annots").unwrap().clone();doc.get_dictionary_mut(pages[1]).unwrap().set("Annots",raw);
    let before=doc.objects.clone();let popups=[receipt(pages[0],1,popup)];
    let result=remove_owned(doc,&[Removal{annotation:receipt(pages[0],0,note),popups:&popups}],&mut ||Ok(())).unwrap();
    assert_eq!(ids(&result,pages[0]),vec![other]);assert_eq!(ids(&result,pages[1]),vec![note,popup,other]);
    for (id,value) in before {if id!=pages[0]{assert_eq!(result.objects.get(&id),Some(&value));}}
}
#[test]
fn annotation_removal_empty_transaction_preserves_every_object() {
    let (doc,_,_,_,_)=fixture();let result=remove_owned(doc.clone(),&[],&mut ||Ok(())).unwrap();
    assert_eq!(doc.objects,result.objects);assert_eq!(doc.trailer,result.trailer);
}

#[test]
fn annotation_removal_requires_admission_for_live_symbolic_script_consumers() {
    let (mut doc,pages,note,popup,_)=fixture();
    let action=doc.add_object(dictionary!{"S"=>"JavaScript","JS"=>Object::string_literal("this.getAnnots({nPage:0});")});
    let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
    doc.get_dictionary_mut(catalog).unwrap().set("OpenAction",action);
    assert!(remove_owned(doc.clone(),&[],&mut ||Ok(())).is_ok(),"unchanged graphs do not need mutation admission");
    let requests=[Removal{annotation:receipt(pages[0],0,note),popups:&[receipt(pages[0],1,popup)]}];
    assert!(remove_owned(doc.clone(),&requests,&mut ||Ok(())).is_err(),"scripts can address annotations without indirect references");
    doc.get_dictionary_mut(action).unwrap().set("JS",Object::Null);
    assert!(remove_owned(doc,&requests,&mut ||Ok(())).is_ok(),"null optional script entries have no execution payload");
}

#[test]
fn annotation_removal_refuses_stale_ambiguous_and_incomplete_popup_requests() {
    let (doc,pages,note,popup,_)=fixture();let root=receipt(pages[0],0,note);let child=receipt(pages[0],1,popup);
    for wrong in [Receipt{output_slot:1,..root},Receipt{annotation:(u32::MAX,0),..root},Receipt{output_page:pages[1],..root}] {
        assert!(remove_owned(doc.clone(),&[Removal{annotation:wrong,popups:&[child]}],&mut ||Ok(())).is_err());
    }
    assert!(remove_owned(doc.clone(),&[Removal{annotation:root,popups:&[]}],&mut ||Ok(())).is_err());
    assert!(remove_owned(doc.clone(),&[Removal{annotation:root,popups:&[child,child]}],&mut ||Ok(())).is_err());
    assert!(remove_owned(doc.clone(),&[Removal{annotation:root,popups:&[child]},Removal{annotation:root,popups:&[child]}],&mut ||Ok(())).is_err());
    let mut repeated=doc.clone();let list=repeated.get_dictionary(pages[0]).unwrap().get(b"Annots").unwrap().as_reference().unwrap();
    repeated.get_object_mut(list).unwrap().as_array_mut().unwrap().push(Object::Reference(note));
    assert!(remove_owned(repeated,&[Removal{annotation:root,popups:&[child]}],&mut ||Ok(())).is_err());
    let mut wrong_parent=doc.clone();wrong_parent.get_dictionary_mut(popup).unwrap().set("Parent",pages[1]);
    assert!(remove_owned(wrong_parent,&[Removal{annotation:root,popups:&[child]}],&mut ||Ok(())).is_err());
}

#[test]
fn annotation_removal_refuses_opaque_consumers_even_through_shared_alias_containers() {
    for mode in 0..7 {
        let (mut doc,pages,note,popup,other)=fixture();let root=receipt(pages[0],0,note);let child=receipt(pages[0],1,popup);
        let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        match mode {
            0=>{doc.get_dictionary_mut(other).unwrap().set("IRT",note);},
            1=>{doc.get_dictionary_mut(other).unwrap().set("Popup",popup);},
            2=>{doc.get_dictionary_mut(catalog).unwrap().set("PrivateConsumer",note);},
            3=>{let list=doc.get_dictionary(pages[0]).unwrap().get(b"Annots").unwrap().clone();doc.get_dictionary_mut(catalog).unwrap().set("PrivateConsumer",list);},
            4=>{let alias=doc.add_object(Object::Reference(note));let array=doc.add_object(vec![Object::Reference(alias)]);doc.get_dictionary_mut(other).unwrap().set("A",dictionary!{"S"=>"Hide","T"=>array});},
            5=>{doc.get_dictionary_mut(note).unwrap().set("StructParent",0);},
            _=>{doc.get_dictionary_mut(note).unwrap().set("PrivateSelf",note);},
        }
        assert!(remove_owned(doc,&[Removal{annotation:root,popups:&[child]}],&mut ||Ok(())).is_err(),"unhandled consumer {mode}");
    }
}

#[test]
fn annotation_removal_handles_forward_and_reverse_only_popups_and_preserves_other_languages() {
    for mode in 0..3 {
        let (mut doc,pages,note,popup,other)=fixture();
        doc.get_dictionary_mut(other).unwrap().set("Lang",Object::string_literal("fr-CA"));
        match mode {0=>{doc.get_dictionary_mut(note).unwrap().remove(b"Popup");},1=>{doc.get_dictionary_mut(popup).unwrap().remove(b"Parent");},_=>{doc.get_dictionary_mut(popup).unwrap().set("Parent",Object::Null);}}
        let original=doc.get_dictionary(other).unwrap().clone();
        let result=remove_owned(doc,&[Removal{annotation:receipt(pages[0],0,note),popups:&[receipt(pages[0],1,popup)]}],&mut ||Ok(())).unwrap();
        assert_eq!(ids(&result,pages[0]),vec![other]);assert_eq!(result.get_dictionary(other).unwrap(),&original);
    }
    let (mut doc,pages,note,_,_)=fixture();doc.get_dictionary_mut(note).unwrap().remove(b"Popup");
    assert!(remove_owned(doc,&[Removal{annotation:receipt(pages[0],0,note),popups:&[]}],&mut ||Ok(())).is_err(),"a reverse-only popup cannot be silently retained");
}

#[test]
fn annotation_removal_cancellation_budgets_and_large_direct_metadata_never_publish() {
    let (doc,pages,note,popup,_)=fixture();let children=[receipt(pages[0],1,popup)];let requests=[Removal{annotation:receipt(pages[0],0,note),popups:&children}];
    let before=doc.objects.clone();let mut calls=0;
    remove_owned(doc.clone(),&requests,&mut ||{calls+=1;Ok(())}).unwrap();
    for stop in [1,calls/2,calls-1,calls] {
        let mut seen=0;
        assert!(remove_owned(doc.clone(),&requests,&mut ||{seen+=1;if seen==stop{Err(EngineError::InvalidRequest("cancelled".into()))}else{Ok(())}}).is_err());
    }
    for (metadata,steps) in [(1,1_000_000),(16*1024*1024,1)] {
        assert!(remove_with_budget(doc.clone(),&requests,metadata,steps,&mut ||Ok(())).is_err());
    }
    let mut large=doc.clone();let list=large.get_dictionary(pages[0]).unwrap().get(b"Annots").unwrap().as_reference().unwrap();
    large.get_object_mut(list).unwrap().as_array_mut().unwrap().push(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Contents"=>Object::string_literal(vec![b'x';16*1024*1024])}.into());
    assert!(remove_owned(large,&requests,&mut ||Ok(())).is_err(),"large direct payload cannot amplify copies");
    assert_eq!(doc.objects,before);
}

#[test]
fn annotation_removal_rotated_physical_occurrences_match_source_derived_reference() {
    for rotation in [0,90,180,270] {for unit in [1.0f32,1.7] {
        let (mut doc,pages,note,popup,_)=fixture();
        doc.get_dictionary_mut(pages[0]).unwrap().set("Rotate",rotation);
        doc.get_dictionary_mut(pages[0]).unwrap().set("UserUnit",unit);
        let anchors=[Anchor{page:pages[0],slot:0,annotation:note},Anchor{page:pages[0],slot:1,annotation:popup}];
        let (doc,pages,receipts)=composition_occurrences::project_forms_tracked(doc,&[pages[0],pages[0]],&anchors,&mut ||Ok(())).unwrap();
        let mut source=doc.clone();let mut expected=doc.clone();let remaining=ids(&expected,pages[1])[2];
        expected.get_dictionary_mut(pages[1]).unwrap().set("Annots",vec![Object::Reference(remaining)]);
        let mut actual=remove_owned(doc,&[Removal{annotation:receipts[2],popups:&receipts[3..4]}],&mut ||Ok(())).unwrap();
        assert_eq!(actual.objects,expected.objects);
        if let Some(path)=std::env::var_os("FOLIO_STRUCTURED_OVERLAY_CAPTURE_DIR") {
            let path=std::path::PathBuf::from(path);std::fs::create_dir_all(&path).unwrap();
            for (kind,doc) in [("source",&mut source),("expected",&mut expected),("actual",&mut actual)] {
                doc.save(path.join(format!("removal-{rotation}-{unit}-{kind}.pdf"))).unwrap();
            }
        }
    }}
}

fn tagged_fixture() -> (Document,[ObjectId;2],ObjectId,ObjectId,ObjectId,ObjectId,[ObjectId;4]) {
    let (mut doc,pages,note,popup,other)=fixture();
    let root=doc.new_object_id();let group=doc.new_object_id();
    let body=doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"P","P"=>group,"Pg"=>pages[0],"K"=>0,"Lang"=>Object::string_literal("en-GB")});
    let mut elements=vec![body];let mut nums=vec![];
    for (key,annotation) in [note,popup,other].into_iter().enumerate() {
        doc.get_dictionary_mut(annotation).unwrap().set("StructParent",key as i64);
        let objr=doc.add_object(dictionary!{"Type"=>"OBJR","Obj"=>annotation,"Pg"=>pages[0]});
        let element=doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"Annot","P"=>group,"Pg"=>pages[0],"K"=>objr});
        elements.push(element);nums.extend([Object::Integer(key as i64),Object::Reference(element)]);
    }
    nums.extend([5.into(),Object::Array(vec![Object::Reference(body)])]);
    let tree=doc.add_object(dictionary!{"Nums"=>nums});
    doc.get_dictionary_mut(pages[0]).unwrap().set("StructParents",5);
    let content=doc.get_dictionary(pages[0]).unwrap().get(b"Contents").unwrap().as_reference().unwrap();
    doc.get_object_mut(content).unwrap().as_stream_mut().unwrap().set_plain_content(b"/P <</MCID 0>> BDC 0 0 1 rg 10 10 30 30 re f EMC".to_vec());
    doc.objects.insert(group,dictionary!{"Type"=>"StructElem","S"=>"Document","P"=>root,"Pg"=>pages[0],"Lang"=>Object::string_literal("fr-CA"),"K"=>elements.iter().copied().map(Object::Reference).collect::<Vec<_>>()}.into());
    doc.objects.insert(root,dictionary!{"Type"=>"StructTreeRoot","K"=>group,"ParentTree"=>tree,"ParentTreeNextKey"=>6}.into());
    let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
    doc.get_dictionary_mut(catalog).unwrap().set("StructTreeRoot",root);
    doc.get_dictionary_mut(catalog).unwrap().set("MarkInfo",dictionary!{"Marked"=>true});
    structure(&doc);
    (doc,pages,note,popup,root,group,elements.try_into().unwrap())
}
fn structure(doc:&Document)->crate::composition_structure::StructureInfo {
    let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let pages=crate::composition_pages::inspect(doc,catalog,&mut ||Ok(())).unwrap();
    crate::composition_structure::inspect(doc,catalog,&pages.pages,&mut ||Ok(())).unwrap()
}
#[test]
fn annotation_removal_tagged_copy_keeps_original_tags_marked_content_and_language() {
    let (doc,pages,note,popup,_,_,_)=tagged_fixture();
    let anchors=[Anchor{page:pages[0],slot:0,annotation:note},Anchor{page:pages[0],slot:1,annotation:popup}];
    let (doc,pages,receipts)=composition_occurrences::project_forms_tracked(doc,&[pages[0],pages[0]],&anchors,&mut ||Ok(())).unwrap();
    let before=structure(&doc);let objects=doc.objects.clone();let original=ids(&doc,pages[0]);
    let keys=receipts[2..4].iter().map(|r|doc.get_dictionary(r.annotation).unwrap().get(b"StructParent").unwrap().as_i64().unwrap()).collect::<BTreeSet<_>>();
    let removed_elements=keys.iter().map(|key|match before.owners[key].parents {crate::composition_structure::Parents::Whole(id)=>id,_=>panic!()}).collect::<BTreeSet<_>>();
    let result=remove_owned(doc,&[Removal{annotation:receipts[2],popups:&receipts[3..4]}],&mut ||Ok(())).unwrap();
    let after=structure(&result);
    assert_eq!(after.owners,before.owners.into_iter().filter(|(key,_)|!keys.contains(key)).collect());
    assert_eq!(after.elements,before.elements.difference(&removed_elements).copied().collect());
    assert_eq!(ids(&result,pages[0]),original);assert_eq!(ids(&result,pages[1]).len(),1);
    for (id,value) in &objects {
        if let Object::Stream(_) = value {assert_eq!(result.objects.get(id),Some(value));}
        if let Ok(dict)=value.as_dict(){if dict.get(b"Pg").ok()==Some(&Object::Reference(pages[0])) {assert_eq!(result.objects.get(id),Some(value),"original occurrence metadata changed");}}
    }
    let mut saved=result;let mut bytes=vec![];saved.save_to(&mut bytes).unwrap();let reopened=Document::load_mem(&bytes).unwrap();assert_eq!(structure(&reopened),after);
}
#[test]
fn annotation_removal_partial_tag_keeps_mcid_and_refuses_stale_ancestor_descriptions() {
    let (mut doc,pages,note,popup,root,group,elements)=tagged_fixture();
    let [body,note_tag,popup_tag,other_tag]=elements;
    let objr=doc.get_dictionary(note_tag).unwrap().get(b"K").unwrap().clone();
    doc.get_dictionary_mut(body).unwrap().set("K",vec![Object::Integer(0),objr]);
    doc.get_dictionary_mut(group).unwrap().set("K",vec![Object::Reference(body),popup_tag.into(),other_tag.into()]);
    let tree=doc.get_dictionary(root).unwrap().get(b"ParentTree").unwrap().as_reference().unwrap();
    doc.get_dictionary_mut(tree).unwrap().get_mut(b"Nums").unwrap().as_array_mut().unwrap()[1]=body.into();
    structure(&doc);
    let requests=[Removal{annotation:receipt(pages[0],0,note),popups:&[receipt(pages[0],1,popup)]}];
    for (target,key) in [(body,"Alt"),(body,"ActualText"),(group,"E")] {
        let mut described=doc.clone();described.get_dictionary_mut(target).unwrap().set(key,Object::string_literal("description includes note"));
        assert!(remove_owned(described,&requests,&mut ||Ok(())).is_err());
    }
    let original=doc.clone();
    let output=remove_owned(doc,&requests,&mut ||Ok(())).unwrap();let info=structure(&output);
    assert!(!info.owners.contains_key(&0));assert!(!info.owners.contains_key(&1));
    assert_eq!(info.owners[&5].parents,crate::composition_structure::Parents::Marked(vec![Some(body)]));
    assert_eq!(output.get_dictionary(body).unwrap().get(b"K").unwrap(),&Object::Array(vec![Object::Integer(0)]));
    assert_eq!(output.get_dictionary(group).unwrap().get(b"Lang").unwrap(),&Object::string_literal("fr-CA"));
    capture_tagged("tagged-partial",original,output,0);
}

#[test]
fn annotation_removal_tagged_keeps_empty_form_parent_slots() {
    let (mut doc,pages,note,popup,root,_,_)=tagged_fixture();
    let form=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),10.into(),10.into()],"StructParents"=>6},b"q Q".to_vec()));
    doc.get_dictionary_mut(pages[0]).unwrap().set("Resources",dictionary!{"XObject"=>dictionary!{"UnusedForm"=>form}});
    let tree=doc.get_dictionary(root).unwrap().get(b"ParentTree").unwrap().as_reference().unwrap();
    doc.get_dictionary_mut(tree).unwrap().get_mut(b"Nums").unwrap().as_array_mut().unwrap().extend([Object::Integer(6),Object::Array(vec![])]);
    doc.get_dictionary_mut(root).unwrap().set("ParentTreeNextKey",7);
    let before=structure(&doc);
    let requests=[Removal{annotation:receipt(pages[0],0,note),popups:&[receipt(pages[0],1,popup)]}];
    let output=remove_owned(doc,&requests,&mut ||Ok(())).unwrap();
    assert_eq!(structure(&output).owners[&6],before.owners[&6]);
}

#[test]
fn annotation_removal_tagged_refuses_foreign_pages_opaque_links_and_parent_tree_extensions() {
    for mode in 0..7 {
        let (mut doc,pages,note,popup,root,group,elements)=tagged_fixture();
        let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let record=doc.get_dictionary(elements[1]).unwrap().get(b"K").unwrap().as_reference().unwrap();
        let tree=doc.get_dictionary(root).unwrap().get(b"ParentTree").unwrap().as_reference().unwrap();
        match mode {
            0=>{doc.get_dictionary_mut(record).unwrap().set("Pg",pages[1]);},
            1=>{doc.get_dictionary_mut(catalog).unwrap().set("PrivateRecord",record);},
            2=>{doc.get_dictionary_mut(catalog).unwrap().set("PrivateTag",elements[1]);},
            3=>{doc.get_dictionary_mut(catalog).unwrap().set("PrivateReverseTree",tree);},
            4=>{doc.get_dictionary_mut(tree).unwrap().set("PrivateMetadata",Object::string_literal("must survive"));},
            5=>{doc.get_dictionary_mut(group).unwrap().set("Alt",Object::string_literal("includes deleted note"));},
            _=>{doc.get_dictionary_mut(elements[1]).unwrap().set("ID",Object::string_literal("addressable-note"));},
        }
        assert!(remove_owned(doc,&[Removal{annotation:receipt(pages[0],0,note),popups:&[receipt(pages[0],1,popup)]}],&mut ||Ok(())).is_err(),"unsupported semantics {mode}");
    }
}

#[test]
fn annotation_removal_tagged_cancellation_keeps_input_and_limits_are_cumulative() {
    let (doc,pages,note,popup,_,_,_)=tagged_fixture();let before=doc.objects.clone();
    let requests=[Removal{annotation:receipt(pages[0],0,note),popups:&[receipt(pages[0],1,popup)]}];let mut count=0;
    remove_owned(doc.clone(),&requests,&mut ||{count+=1;Ok(())}).unwrap();
    for stop in [1,count/3,2*count/3,count-1,count] {
        let mut current=0;
        assert!(remove_owned(doc.clone(),&requests,&mut ||{current+=1;if current==stop{Err(bad("cancelled"))}else{Ok(())}}).is_err());
    }
    assert!(remove_with_budget(doc.clone(),&requests,1000,1_000_000,&mut ||Ok(())).is_err());
    assert!(remove_with_budget(doc.clone(),&requests,16*1024*1024,50,&mut ||Ok(())).is_err());
    assert_eq!(doc.objects,before);
}

fn capture_tagged(name:&str,mut source:Document,mut actual:Document,target_page:usize) {
    let Some(path)=std::env::var_os("FOLIO_STRUCTURED_OVERLAY_CAPTURE_DIR") else{return};
    let path=std::path::PathBuf::from(path);std::fs::create_dir_all(&path).unwrap();
    let page=*source.get_pages().values().nth(target_page).unwrap();let remaining=ids(&source,page).into_iter().skip(2).map(Object::Reference).collect::<Vec<_>>();
    let mut expected=source.clone();expected.get_dictionary_mut(page).unwrap().set("Annots",remaining);
    for (kind,doc) in [("source",&mut source),("expected",&mut expected),("actual",&mut actual)] {doc.save(path.join(format!("{name}-{kind}.pdf"))).unwrap();}
    std::fs::write(path.join(format!("{name}-case.json")),serde_json::to_vec_pretty(&serde_json::json!({"targetPage":target_page,"removedSlots":[0,1],"expectedPdfScope":"visual-only source-derived reference; tag relationships are independently derived from source"})).unwrap()).unwrap();
}

#[test]
fn annotation_removal_tagged_corpus_preserves_physical_copied_pages_and_owners() {
    for rotation in [0,90,180,270] {for unit in [1.0f32,1.7] {
        let (mut doc,pages,note,popup,_,_,_)=tagged_fixture();
        doc.get_dictionary_mut(pages[0]).unwrap().set("Rotate",rotation);
        doc.get_dictionary_mut(pages[0]).unwrap().set("UserUnit",unit);
        let anchors=[Anchor{page:pages[0],slot:0,annotation:note},Anchor{page:pages[0],slot:1,annotation:popup}];
        let (doc,_,receipts)=composition_occurrences::project_forms_tracked(doc,&[pages[0],pages[0]],&anchors,&mut ||Ok(())).unwrap();
        let source=doc.clone();let before=structure(&doc);
        let output=remove_owned(doc,&[Removal{annotation:receipts[2],popups:&receipts[3..4]}],&mut ||Ok(())).unwrap();
        assert_eq!(structure(&output).owners.len()+2,before.owners.len());
        capture_tagged(&format!("tagged-copy-{rotation}-{unit}"),source,output,1);
    }}
}

#[test]
fn annotation_removal_tagged_shared_roots_require_all_owning_occurrences() {
    let (mut doc,pages,note,popup,_,_,_)=tagged_fixture();
    let list=doc.get_dictionary(pages[0]).unwrap().get(b"Annots").unwrap().clone();doc.get_dictionary_mut(pages[1]).unwrap().set("Annots",list);
    let first=[receipt(pages[0],1,popup)];let second=[receipt(pages[1],1,popup)];
    assert!(remove_owned(doc.clone(),&[Removal{annotation:receipt(pages[0],0,note),popups:&first}],&mut ||Ok(())).is_err(),"partial shared tag ownership needs isolation");
    let output=remove_owned(doc,&[Removal{annotation:receipt(pages[0],0,note),popups:&first},Removal{annotation:receipt(pages[1],0,note),popups:&second}],&mut ||Ok(())).unwrap();
    let tags=structure(&output);assert!(!tags.owners.contains_key(&0));assert!(!tags.owners.contains_key(&1));
    assert_eq!(ids(&output,pages[0]).len(),1);assert_eq!(ids(&output,pages[1]).len(),1);
}

#[test]
fn annotation_removal_tagged_last_owned_subtree_is_removed_without_dangling_reverse_owners() {
    let (mut doc,pages,note,popup,root,group,elements)=tagged_fixture();
    doc.get_dictionary_mut(pages[0]).unwrap().remove(b"StructParents");
    doc.get_dictionary_mut(popup).unwrap().remove(b"StructParent");
    doc.get_dictionary_mut(pages[0]).unwrap().set("Annots",vec![Object::Reference(note),popup.into()]);
    let content=doc.get_dictionary(pages[0]).unwrap().get(b"Contents").unwrap().as_reference().unwrap();
    doc.get_object_mut(content).unwrap().as_stream_mut().unwrap().set_plain_content(b"0 0 1 rg 10 10 30 30 re f".to_vec());
    doc.get_dictionary_mut(group).unwrap().set("K",elements[1]);
    doc.get_dictionary_mut(root).unwrap().set("ParentTree",dictionary!{"Nums"=>vec![Object::Integer(0),elements[1].into()]});
    structure(&doc);
    let output=remove_owned(doc,&[Removal{annotation:receipt(pages[0],0,note),popups:&[receipt(pages[0],1,popup)]}],&mut ||Ok(())).unwrap();
    let tags=structure(&output);assert!(tags.elements.is_empty());assert!(tags.owners.is_empty());assert!(ids(&output,pages[0]).is_empty());
    assert!(!output.get_dictionary(root).unwrap().has(b"K"));
}
