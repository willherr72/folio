use super::*;
use lopdf::{dictionary,Stream};

struct Fixture {doc:Document,catalog:ObjectId,pages:Vec<ObjectId>,note:ObjectId,link:ObjectId,appearance:ObjectId}
fn fixture()->Fixture {
    let mut doc=Document::with_version("1.7");let pages=doc.new_object_id();
    let appearance=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),10.into(),10.into()]},b"q 0 0 10 10 re f Q".to_vec()));
    let note=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Contents"=>Object::string_literal("Original comment"),"T"=>Object::string_literal("Original author"),"Subj"=>Object::string_literal("Original subject"),"AP"=>dictionary!{"N"=>appearance},"Rect"=>vec![0.into(),0.into(),10.into(),10.into()]});
    let alias=doc.add_object(Object::Reference(note));
    let link=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Link","Contents"=>Object::string_literal("Original link"),"Rect"=>vec![20.into(),0.into(),30.into(),10.into()],"A"=>dictionary!{"S"=>"URI","URI"=>Object::string_literal("https://example.com")}});
    let first=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()],"Annots"=>vec![Object::Reference(alias),link.into()]});
    let second=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()],"Annots"=>vec![Object::Reference(note)]});
    doc.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>2,"Kids"=>vec![Object::Reference(first),second.into()]}.into());
    let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages,"Lang"=>Object::string_literal("en")});doc.trailer.set("Root",catalog);
    Fixture{doc,catalog,pages:vec![first,second],note,link,appearance}
}
fn check()->EngineResult<()> {Ok(())}
fn inspect_original(f:&Fixture)->StructureInfo {crate::composition_structure::inspect(&f.doc,f.catalog,&f.pages,&mut check).unwrap()}

#[test]
fn annotation_language_preserves_strings_appearances_aliases_and_shared_occurrences() {
    let mut f=fixture();let original=inspect_original(&f);let before=f.doc.objects.clone();
    let prepared=prepare(&mut f.doc,&f.pages,&original,b"",false,&mut check).unwrap();
    assert_eq!(prepared.language_owned_annotations,BTreeSet::from([f.note,f.link]));
    let structure=prepared.structure.unwrap();assert_eq!(structure.elements.len(),2);
    f.doc.get_dictionary_mut(f.catalog).unwrap().set("StructTreeRoot",structure.root.unwrap());
    let checked=crate::composition_structure::inspect(&f.doc,f.catalog,&f.pages,&mut check).unwrap();
    assert_eq!(checked,structure);
    for id in [f.note,f.link] {
        let old=before[&id].as_dict().unwrap();let mut new=f.doc.get_dictionary(id).unwrap().clone();new.remove(b"StructParent");
        assert_eq!(&new,old);
        let owner=checked.owners.values().find(|owner|owner.object==id).unwrap();
        let Parents::Whole(parent)=owner.parents else {panic!("whole owner required")};
        let element=f.doc.get_dictionary(parent).unwrap();
        assert_eq!(element.get(b"Lang").unwrap().as_str().unwrap(),b"");
        assert_eq!(element.get(b"S").unwrap().as_name().unwrap(),if id==f.link {b"Link".as_slice()} else {b"Annot"});
        assert_eq!(element.get(b"K").unwrap().as_array().unwrap().len(),if id==f.note {2}else{1});
    }
    assert_eq!(f.doc.objects[&f.appearance],before[&f.appearance]);
    for id in &f.pages {assert_eq!(f.doc.objects[id],before[id]);}
}

#[test]
fn annotation_language_existing_owners_and_roles_are_not_reinterpreted() {
    let mut f=fixture();let root=f.doc.new_object_id();
    let element=f.doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"Annot","P"=>root,"Pg"=>f.pages[0],"Lang"=>Object::string_literal("fr"),"K"=>dictionary!{"Type"=>"OBJR","Obj"=>f.link,"Pg"=>f.pages[0]}});
    let parent=f.doc.add_object(dictionary!{"Nums"=>vec![0.into(),Object::Reference(element)]});
    f.doc.objects.insert(root,dictionary!{"Type"=>"StructTreeRoot","K"=>vec![Object::Reference(element)],"ParentTree"=>parent,"RoleMap"=>dictionary!{"Annot"=>"Figure"}}.into());
    f.doc.get_dictionary_mut(f.link).unwrap().set("StructParent",0);
    f.doc.get_dictionary_mut(f.catalog).unwrap().set("StructTreeRoot",root);
    let original=inspect_original(&f);
    let prepared=prepare(&mut f.doc,&f.pages,&original,b"de",false,&mut check).unwrap();
    let generated=prepared.structure.unwrap();assert_eq!(generated.elements.len(),1);
    let new_element=generated.top[0];
    let merged=crate::composition_structure_merge::merge(&mut f.doc,&[original,generated],&mut check).unwrap();
    f.doc.get_dictionary_mut(f.catalog).unwrap().set("StructTreeRoot",merged);
    crate::composition_structure::inspect(&f.doc,f.catalog,&f.pages,&mut check).unwrap();
    let old=f.doc.get_dictionary(element).unwrap();
    assert_eq!(old.get(b"Lang").unwrap().as_str().unwrap(),b"fr");
    let old_role=old.get(b"S").unwrap().as_name().unwrap();
    assert_eq!(f.doc.get_dictionary(merged).unwrap().get(b"RoleMap").unwrap().as_dict().unwrap().get(old_role).unwrap().as_name().unwrap(),b"Figure");
    assert_eq!(f.doc.get_dictionary(new_element).unwrap().get(b"S").unwrap().as_name().unwrap(),b"Annot");
}

#[test]
fn annotation_language_refusals_and_cancellation_leave_graph_unchanged() {
    for change in 0..3 {
        let mut f=fixture();let original=inspect_original(&f);
        match change {
            0=>f.doc.get_dictionary_mut(f.note).unwrap().set("Subtype","Widget"),
            1=>f.doc.objects.get_mut(&f.appearance).unwrap().as_stream_mut().unwrap().dict.set("StructParents",0),
            _=>f.doc.get_dictionary_mut(f.note).unwrap().set("Lang",Object::string_literal("fr")),
        }
        let before=f.doc.objects.clone();let max=f.doc.max_id;
        assert!(prepare(&mut f.doc,&f.pages,&original,b"en",false,&mut check).is_err());
        assert_eq!(f.doc.objects,before);assert_eq!(f.doc.max_id,max);
    }
    let mut f=fixture();let original=inspect_original(&f);let mut total=0;
    prepare(&mut f.doc,&f.pages,&original,b"en",false,&mut ||{total+=1;Ok(())}).unwrap();
    for stop in [0,total/2,total-1] {
        let mut f=fixture();let before=f.doc.objects.clone();let max=f.doc.max_id;let original=inspect_original(&f);let mut calls=0;
        assert!(prepare(&mut f.doc,&f.pages,&original,b"en",false,&mut ||{let refuse=calls>=stop;calls+=1;if refuse {Err(EngineError::InvalidRequest("cancel test".into()))}else{Ok(())}}).is_err());
        assert_eq!(f.doc.objects,before);assert_eq!(f.doc.max_id,max);
    }
}

#[test]
fn annotation_language_explicit_pdf2_override_stays_native() {
    let mut f=fixture();f.doc.get_dictionary_mut(f.note).unwrap().set("Lang",Object::string_literal("fr"));
    let original=inspect_original(&f);let before=f.doc.objects[&f.note].clone();
    let prepared=prepare(&mut f.doc,&f.pages,&original,b"en",true,&mut check).unwrap();
    assert_eq!(f.doc.objects[&f.note],before);
    assert!(prepared.language_owned_annotations.contains(&f.note));
    assert_eq!(prepared.structure.unwrap().elements.len(),1);
}

#[test]
fn annotation_language_does_not_absorb_valid_existing_appearance_structure() {
    let mut f=fixture();let root=f.doc.new_object_id();
    f.doc.get_dictionary_mut(f.pages[1]).unwrap().set("Annots",Vec::<Object>::new());
    let element=f.doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"P","P"=>root,"Pg"=>f.pages[0],"K"=>dictionary!{"Type"=>"MCR","Stm"=>f.appearance,"StmOwn"=>f.note,"MCID"=>0}});
    let parent=f.doc.add_object(dictionary!{"Nums"=>vec![0.into(),Object::Array(vec![element.into()])]});
    f.doc.objects.insert(root,dictionary!{"Type"=>"StructTreeRoot","K"=>vec![Object::Reference(element)],"ParentTree"=>parent}.into());
    let appearance=f.doc.objects.get_mut(&f.appearance).unwrap().as_stream_mut().unwrap();
    appearance.dict.set("StructParents",0);
    appearance.set_content(b"/P <</MCID 0>> BDC q 0 0 10 10 re f Q EMC".to_vec());
    f.doc.get_dictionary_mut(f.catalog).unwrap().set("StructTreeRoot",root);
    let original=inspect_original(&f);let before=f.doc.objects.clone();
    assert!(prepare(&mut f.doc,&f.pages,&original,b"en",false,&mut check).is_err());
    assert_eq!(f.doc.objects,before);
}

#[test]
fn annotation_language_metadata_budget_and_depth_fail_before_mutation() {
    let mut f=fixture();let original=inspect_original(&f);
    let mut annots=Vec::new();
    for _ in 0..12_000 {
        let id=f.doc.add_object(dictionary!{"Subtype"=>"Text","Rect"=>vec![0.into(),0.into(),1.into(),1.into()]});annots.push(Object::Reference(id));
    }
    f.doc.get_dictionary_mut(f.pages[0]).unwrap().set("Annots",annots);
    let before=f.doc.objects.clone();let max=f.doc.max_id;
    assert!(prepare(&mut f.doc,&f.pages,&original,&vec![b'x';1024],false,&mut check).is_err());
    assert_eq!(f.doc.objects,before);assert_eq!(f.doc.max_id,max);
    let mut f=fixture();let original=inspect_original(&f);
    let mut deep=Object::Null;for _ in 0..65 {deep=Object::Array(vec![deep]);}
    f.doc.get_dictionary_mut(f.note).unwrap().set("AP",deep);
    let before=f.doc.objects.clone();
    assert!(prepare(&mut f.doc,&f.pages,&original,b"en",false,&mut check).is_err());
    assert_eq!(f.doc.objects,before);
}
