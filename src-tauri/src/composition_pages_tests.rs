use super::*;
use lopdf::dictionary;

fn fixture() -> (Document,ObjectId,ObjectId,ObjectId,Vec<ObjectId>) {
    let mut doc=Document::with_version("1.7");
    let root=doc.new_object_id();
    let branch=doc.new_object_id();
    let first=doc.add_object(dictionary! {"Type"=>"Page","Parent"=>branch,"UserUnit"=>1.7});
    let second=doc.add_object(dictionary! {"Type"=>"Page","Parent"=>root,"Rotate"=>180});
    doc.objects.insert(branch,dictionary! {"Type"=>"Pages","Parent"=>root,"Kids"=>vec![Object::Reference(first)],"Count"=>1,"Rotate"=>90,"CropBox"=>vec![0.into(),0.into(),400.into(),200.into()]}.into());
    doc.objects.insert(root,dictionary! {"Type"=>"Pages","Kids"=>vec![Object::Reference(branch),second.into()],"Count"=>2,"MediaBox"=>vec![(-10).into(),(-10).into(),410.into(),210.into()],"Resources"=>dictionary! {"ProcSet"=>vec![Object::Name(b"PDF".to_vec())]}}.into());
    let catalog=doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>root});
    doc.trailer.set("Root",catalog);
    (doc,catalog,root,branch,vec![first,second])
}
fn check()->EngineResult<()>{Ok(())}

#[test]
fn composition_pages_retains_source_order_and_inherited_geometry_without_mutation() {
    let (doc,catalog,root,_,pages)=fixture();
    let before=doc.objects.clone();
    assert_eq!(inspect(&doc,catalog,&mut check).unwrap(),SourcePages{catalog,root,pages});
    assert_eq!(doc.objects,before);
}

#[test]
fn composition_pages_resolves_parent_child_and_catalog_aliases_consistently() {
    let (mut doc,catalog,root,branch,pages)=fixture();
    let root_alias=doc.add_object(Object::Reference(root));
    let child_alias=doc.add_object(Object::Reference(pages[0]));
    let catalog_alias=doc.add_object(Object::Reference(catalog));
    doc.get_dictionary_mut(catalog).unwrap().set("Pages",root_alias);
    doc.get_dictionary_mut(branch).unwrap().set("Parent",root_alias);
    doc.get_dictionary_mut(branch).unwrap().set("Kids",vec![Object::Reference(child_alias)]);
    assert_eq!(inspect(&doc,catalog_alias,&mut check).unwrap(),SourcePages{catalog,root,pages});
}

#[test]
fn composition_pages_refuses_inconsistent_counts_parents_cycles_and_shared_leaves() {
    for fault in 0..10 {
        let (mut doc,catalog,root,branch,pages)=fixture();
        match fault {
            0=>doc.get_dictionary_mut(root).unwrap().set("Count",1),
            1=>doc.get_dictionary_mut(branch).unwrap().set("Count",2),
            2=>doc.get_dictionary_mut(pages[0]).unwrap().set("Parent",root),
            3=>doc.get_dictionary_mut(root).unwrap().set("Parent",branch),
            4=>doc.get_dictionary_mut(branch).unwrap().set("Kids",vec![Object::Reference(root)]),
            5=>doc.get_dictionary_mut(branch).unwrap().set("Kids",vec![Object::Reference(pages[0]),pages[0].into()]),
            6=>doc.get_dictionary_mut(root).unwrap().set("Kids",vec![Object::Reference(branch),branch.into()]),
            7=>doc.get_dictionary_mut(pages[0]).unwrap().set("Kids",vec![Object::Reference(pages[1])]),
            8=>doc.get_dictionary_mut(root).unwrap().set("Count",-1),
            _=>doc.get_dictionary_mut(branch).unwrap().set("Kids",vec![Object::Dictionary(dictionary!{"Type"=>"Page","Parent"=>branch})]),
        }
        let before=doc.objects.clone();
        assert!(inspect(&doc,catalog,&mut check).is_err(),"fault {fault}");
        assert_eq!(doc.objects,before);
    }
}

#[test]
fn composition_pages_cancellation_and_depth_limit_are_deterministic() {
    let (mut doc,catalog,root,_,_)=fixture();
    let mut completed=false;
    for stop in 0..2000 {
        let mut calls=0;
        let result=inspect(&doc,catalog,&mut ||{calls+=1;if calls>stop {Err(EngineError::InvalidRequest("test cancellation".into()))} else {Ok(())}});
        if result.is_ok(){completed=true;break;}
        assert!(result.unwrap_err().to_string().contains("test cancellation"));
    }
    assert!(completed);
    let mut parent=root;
    for _ in 0..66 {
        let child=doc.add_object(dictionary! {"Type"=>"Pages","Parent"=>parent,"Count"=>0,"Kids"=>Vec::<Object>::new()});
        doc.get_dictionary_mut(parent).unwrap().set("Kids",vec![Object::Reference(child)]);
        doc.get_dictionary_mut(parent).unwrap().set("Count",0);
        parent=child;
    }
    assert!(inspect(&doc,catalog,&mut check).is_err());
}
