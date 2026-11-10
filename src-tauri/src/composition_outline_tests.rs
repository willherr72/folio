use super::*;
use lopdf::{dictionary,Object,StringFormat};
use std::collections::BTreeSet;

fn check()->EngineResult<()>{Ok(())}
struct Fixture {catalog:ObjectId,root:ObjectId,first:ObjectId,child:ObjectId,last:ObjectId,structure:StructureInfo}
fn fixture(doc:&mut Document,closed:bool)->Fixture {
    let root=doc.new_object_id();let first=doc.new_object_id();let child=doc.new_object_id();let last=doc.new_object_id();
    let se=doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"P"});
    let action=doc.add_object(dictionary!{"S"=>"URI","URI"=>Object::string_literal("https://example.test/retained")});
    doc.objects.insert(root,dictionary!{"Type"=>"Outlines","First"=>first,"Last"=>last,"Count"=>if closed{2}else{3}}.into());
    doc.objects.insert(first,dictionary!{"Title"=>Object::String(b"\xfe\xff\0C\0h\0a\0p\0t\0e\0r".to_vec(),StringFormat::Hexadecimal),"Parent"=>root,"Next"=>last,"First"=>child,"Last"=>child,"Count"=>if closed{-1}else{1},"F"=>3,"C"=>vec![Object::Real(0.25),0.into(),1.into()],"SE"=>se,"A"=>action,"VendorData"=>Object::string_literal("unchanged metadata")}.into());
    doc.objects.insert(child,dictionary!{"Title"=>Object::string_literal("Child"),"Parent"=>first,"A"=>action}.into());
    doc.objects.insert(last,dictionary!{"Title"=>Object::string_literal("Last"),"Parent"=>root,"Prev"=>first,"A"=>action}.into());
    let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Outlines"=>root});
    Fixture{catalog,root,first,child,last,structure:StructureInfo{elements:BTreeSet::from([se]),..Default::default()}}
}
fn info(doc:&Document,f:&Fixture)->OutlineInfo {inspect(doc,f.catalog,&f.structure,&mut check).unwrap()}

#[test]
fn composition_outlines_inspects_open_closed_and_empty_without_changing_metadata(){
    for closed in [false,true] {
        let mut doc=Document::with_version("1.7");let f=fixture(&mut doc,closed);let before=doc.objects.clone();
        let outline=info(&doc,&f);
        assert_eq!(outline.root,Some(f.root));assert_eq!(outline.top,vec![f.first,f.last]);
        assert_eq!(outline.items,vec![f.first,f.child,f.last]);assert_eq!(outline.visible_count,if closed{2}else{3});
        assert_eq!(doc.objects,before);
    }
    for value in [Object::Null,Object::Dictionary(dictionary!{"Type"=>"Outlines"}),Object::Dictionary(dictionary!{"First"=>Object::Null,"Last"=>Object::Null,"Count"=>0})] {
        let mut doc=Document::with_version("1.7");let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Outlines"=>value});
        let result=inspect(&doc,catalog,&StructureInfo::default(),&mut check).unwrap();assert!(result.items.is_empty());assert_eq!(result.visible_count,0);
    }
}

#[test]
fn composition_outlines_joins_only_original_top_boundaries_and_keeps_aliases(){
    let mut doc=Document::with_version("1.7");let a=fixture(&mut doc,false);let b=fixture(&mut doc,true);
    let alias_first=doc.add_object(Object::Reference(a.first));let alias_last=doc.add_object(Object::Reference(a.last));
    doc.get_dictionary_mut(a.root).unwrap().set("First",alias_first);
    doc.get_dictionary_mut(a.first).unwrap().set("Next",alias_last);
    doc.get_dictionary_mut(a.last).unwrap().set("Prev",alias_first);
    doc.get_dictionary_mut(a.child).unwrap().set("Parent",alias_first);
    let sources=[info(&doc,&a),info(&doc,&b)];let mut expected=doc.objects.clone();
    let root=merge(&mut doc,&sources,&mut check).unwrap().unwrap();
    for id in [a.first,a.last,b.first,b.last]{expected.get_mut(&id).unwrap().as_dict_mut().unwrap().set("Parent",root);}
    expected.get_mut(&a.last).unwrap().as_dict_mut().unwrap().set("Next",b.first);
    expected.get_mut(&b.first).unwrap().as_dict_mut().unwrap().set("Prev",a.last);
    for (id,value) in expected {assert_eq!(doc.objects[&id],value,"unexpected original-object change: {id:?}");}
    let actual=doc.get_dictionary(root).unwrap();assert_eq!(actual.get(b"First").unwrap().as_reference().unwrap(),a.first);assert_eq!(actual.get(b"Last").unwrap().as_reference().unwrap(),b.last);assert_eq!(actual.get(b"Count").unwrap().as_i64().unwrap(),5);
    let combined_catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Outlines"=>root});
    let structure=StructureInfo{elements:a.structure.elements.union(&b.structure.elements).copied().collect(),..Default::default()};
    let combined=inspect(&doc,combined_catalog,&structure,&mut check).unwrap();assert_eq!(combined.items,vec![a.first,a.child,a.last,b.first,b.child,b.last]);assert_eq!(combined.visible_count,5);
}

#[test]
fn composition_outlines_rejects_inconsistent_hierarchy_and_foreign_structure(){
    for case in ["parent","prev","last","cycle","count","missing-count","root-count","foreign-se","title","flags","color","root-extension"] {
        let mut doc=Document::with_version("1.7");let f=fixture(&mut doc,false);
        match case {
            "parent"=>doc.get_dictionary_mut(f.child).unwrap().set("Parent",f.root),
            "prev"=>doc.get_dictionary_mut(f.last).unwrap().set("Prev",f.child),
            "last"=>doc.get_dictionary_mut(f.root).unwrap().set("Last",f.child),
            "cycle"=>doc.get_dictionary_mut(f.last).unwrap().set("Next",f.first),
            "count"=>doc.get_dictionary_mut(f.first).unwrap().set("Count",i64::MIN),
            "missing-count"=>{doc.get_dictionary_mut(f.first).unwrap().remove(b"Count");},
            "root-count"=>doc.get_dictionary_mut(f.root).unwrap().set("Count",2),
            "foreign-se"=>doc.get_dictionary_mut(f.first).unwrap().set("SE",f.child),
            "title"=>doc.get_dictionary_mut(f.child).unwrap().set("Title",Object::string_literal(vec![b'x';8195])),
            "flags"=>doc.get_dictionary_mut(f.child).unwrap().set("F",4),
            "color"=>doc.get_dictionary_mut(f.child).unwrap().set("C",vec![Object::Real(f32::INFINITY),0.into(),0.into()]),
            _=>doc.get_dictionary_mut(f.root).unwrap().set("VendorRootData",Object::string_literal("cannot silently drop")),
        }
        let before=doc.objects.clone();assert!(inspect(&doc,f.catalog,&f.structure,&mut check).is_err(),"{case}");assert_eq!(doc.objects,before);
    }
}

fn chain(doc:&mut Document,length:usize)->ObjectId {
    let root=doc.new_object_id();let ids=(0..length).map(|_|doc.new_object_id()).collect::<Vec<_>>();
    for (index,id) in ids.iter().copied().enumerate(){
        let mut node=dictionary!{"Title"=>Object::string_literal("node"),"Parent"=>if index==0{root}else{ids[index-1]}};
        if index+1<length{node.set("First",ids[index+1]);node.set("Last",ids[index+1]);node.set("Count",(length-index-1) as i64);}
        doc.objects.insert(id,node.into());
    }
    doc.objects.insert(root,dictionary!{"Type"=>"Outlines","First"=>ids[0],"Last"=>ids[0],"Count"=>length as i64}.into());
    doc.add_object(dictionary!{"Type"=>"Catalog","Outlines"=>root})
}

#[test]
fn composition_outlines_depth_and_cancellation_are_bounded(){
    for (length,accepted) in [(32,true),(33,false)] {
        let mut doc=Document::with_version("1.7");let catalog=chain(&mut doc,length);
        assert_eq!(inspect(&doc,catalog,&StructureInfo::default(),&mut check).is_ok(),accepted);
    }
    let mut doc=Document::with_version("1.7");let a=fixture(&mut doc,false);let b=fixture(&mut doc,true);let sources=[info(&doc,&a),info(&doc,&b)];
    let before=doc.objects.clone();let max_id=doc.max_id;
    let mut visits=0;let mut cancel=||{visits+=1;if visits>=5{Err(EngineError::InvalidRequest("cancel".into()))}else{Ok(())}};
    assert!(merge(&mut doc,&sources,&mut cancel).is_err());assert_eq!(doc.objects,before);assert_eq!(doc.max_id,max_id);
    let mut visits=0;assert!(inspect(&doc,a.catalog,&a.structure,&mut ||{visits+=1;if visits>=5{Err(EngineError::InvalidRequest("cancel".into()))}else{Ok(())}}).is_err());assert_eq!(doc.objects,before);
    assert!(merge(&mut doc,&[sources[0].clone(),sources[0].clone()],&mut check).is_err());assert_eq!(doc.objects,before);
    let mut probe=doc.clone();let mut calls=0;merge(&mut probe,&sources,&mut ||{calls+=1;Ok(())}).unwrap();
    for boundary in [calls-1,calls] {
        let mut probe=doc.clone();let mut count=0;
        assert!(merge(&mut probe,&sources,&mut ||{count+=1;if count==boundary{Err(EngineError::InvalidRequest("final cancellation".into()))}else{Ok(())}}).is_err());
        assert_eq!(probe.objects,before);assert_eq!(probe.max_id,max_id);
    }
    doc.max_id=u32::MAX;assert!(merge(&mut doc,&sources,&mut check).is_err());assert_eq!(doc.objects,before);assert_eq!(doc.max_id,u32::MAX);
}

fn wide(doc:&mut Document,length:usize)->ObjectId {
    let root=doc.new_object_id();let ids=(0..length).map(|_|doc.new_object_id()).collect::<Vec<_>>();
    for (index,id) in ids.iter().copied().enumerate(){
        let mut item=dictionary!{"Title"=>Object::string_literal("leaf"),"Parent"=>root};
        if index>0{item.set("Prev",ids[index-1]);}if index+1<length{item.set("Next",ids[index+1]);}
        doc.objects.insert(id,item.into());
    }
    doc.objects.insert(root,dictionary!{"Type"=>"Outlines","First"=>ids[0],"Last"=>*ids.last().unwrap(),"Count"=>length as i64}.into());
    doc.add_object(dictionary!{"Type"=>"Catalog","Outlines"=>root})
}

#[test]
fn composition_outlines_entry_limit_is_aggregate_and_single_tree_identity_is_retained(){
    let mut doc=Document::with_version("1.7");let catalog=wide(&mut doc,4000);
    let info=inspect(&doc,catalog,&StructureInfo::default(),&mut check).unwrap();assert_eq!(info.items.len(),4000);
    let before=doc.objects.clone();assert_eq!(merge(&mut doc,&[info.clone()],&mut check).unwrap(),info.root);assert_eq!(doc.objects,before);
    let extra=wide(&mut doc,1);let extra=inspect(&doc,extra,&StructureInfo::default(),&mut check).unwrap();
    let before=doc.objects.clone();let max_id=doc.max_id;assert!(merge(&mut doc,&[info,extra],&mut check).is_err());assert_eq!(doc.objects,before);assert_eq!(doc.max_id,max_id);
    let mut doc=Document::with_version("1.7");let catalog=wide(&mut doc,4001);assert!(inspect(&doc,catalog,&StructureInfo::default(),&mut check).is_err());
}
