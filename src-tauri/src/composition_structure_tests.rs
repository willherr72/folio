use super::*;
use lopdf::{dictionary, Object, Stream};

struct Fixture {
    doc: Document,
    catalog: ObjectId,
    page: ObjectId,
    root: ObjectId,
    element: ObjectId,
    annotation: ObjectId,
    form: ObjectId,
    parent_tree: ObjectId,
}
fn fixture() -> Fixture {
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let page = doc.new_object_id();
    let root = doc.new_object_id();
    let element = doc.new_object_id();
    let form = doc.add_object(Stream::new(dictionary! {"Type"=>"XObject", "Subtype"=>"Form", "StructParents"=>2, "BBox"=>vec![0.into(),0.into(),100.into(),100.into()]}, b"/Span <</MCID 0>> BDC EMC".to_vec()));
    let annotation = doc.add_object(dictionary! {"Type"=>"Annot", "Subtype"=>"Link", "StructParent"=>1, "AP"=>dictionary! {"N"=>form}, "Rect"=>vec![0.into(),0.into(),10.into(),10.into()], "A"=>dictionary! {"S"=>"URI","URI"=>Object::string_literal("https://example.com")}});
    doc.objects.insert(page, Object::Dictionary(dictionary! {"Type"=>"Page","Parent"=>pages,"StructParents"=>0,"MediaBox"=>vec![0.into(),0.into(),200.into(),200.into()], "Annots"=>vec![Object::Reference(annotation)], "Resources"=>dictionary! {"XObject"=>dictionary! {"Fm"=>form}}}));
    doc.objects.insert(pages, Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}));
    let children = vec![Object::Integer(0), Object::Integer(2), Object::Dictionary(dictionary! {"Type"=>"MCR","Stm"=>form,"StmOwn"=>annotation,"MCID"=>0}), Object::Dictionary(dictionary! {"Type"=>"OBJR","Obj"=>annotation})];
    doc.objects.insert(element, Object::Dictionary(dictionary! {"Type"=>"StructElem","S"=>"P","P"=>root,"Pg"=>page,"K"=>children,"Alt"=>Object::string_literal("retained alternate description"),"A"=>dictionary! {"O"=>"Layout","Placement"=>"Block"}}));
    let nums = vec![0.into(), Object::Array(vec![element.into(),Object::Null,element.into()]), 1.into(), element.into(), 2.into(), Object::Array(vec![element.into()])];
    let parent_tree = doc.add_object(dictionary! {"Nums"=>nums});
    doc.objects.insert(root, Object::Dictionary(dictionary! {"Type"=>"StructTreeRoot","K"=>vec![Object::Reference(element)],"ParentTree"=>parent_tree,"ParentTreeNextKey"=>3}));
    let catalog = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages,"StructTreeRoot"=>root});
    doc.trailer.set("Root", catalog);
    Fixture {doc,catalog,page,root,element,annotation,form,parent_tree}
}
fn check() -> EngineResult<()> { Ok(()) }

#[test]
fn composition_structure_preserves_marked_and_whole_owners_null_slots_and_attributes() {
    let f = fixture();
    let before = f.doc.objects.clone();
    let info = inspect(&f.doc,f.catalog,&[f.page],&mut check).unwrap();
    assert_eq!(info.root,Some(f.root));
    assert_eq!(info.top,vec![f.element]);
    assert_eq!(info.elements,BTreeSet::from([f.element]));
    assert_eq!(info.owners[&0],Owner {object:f.page,parents:Parents::Marked(vec![Some(f.element),None,Some(f.element)])});
    assert_eq!(info.owners[&1],Owner {object:f.annotation,parents:Parents::Whole(f.element)});
    assert_eq!(info.owners[&2],Owner {object:f.form,parents:Parents::Marked(vec![Some(f.element)])});
    assert_eq!(f.doc.objects,before);
}

#[test]
fn composition_structure_rejects_stale_reverse_slots_and_wrong_whole_parent() {
    for fault in 0..5 {
        let mut f = fixture();
        let other=f.doc.add_object(dictionary! {"Type"=>"StructElem","S"=>"Span","P"=>f.root});
        let nums=f.doc.get_dictionary_mut(f.parent_tree).unwrap().get_mut(b"Nums").unwrap().as_array_mut().unwrap();
        match fault {
            0=>nums[1]=Object::Array(vec![Object::Null,Object::Null,f.element.into()]),
            1=>nums[1]=Object::Array(vec![f.element.into(),f.element.into(),f.element.into()]),
            2=>nums[3]=other.into(),
            3=>nums[5]=Object::Array(vec![other.into()]),
            _=>nums[5]=Object::Array(vec![]),
        }
        let before=f.doc.objects.clone();
        assert!(inspect(&f.doc,f.catalog,&[f.page],&mut check).is_err(),"fault {fault}");
        assert_eq!(f.doc.objects,before);
    }
}

#[test]
fn composition_structure_aliases_are_canonical_and_unreachable_history_does_not_collide() {
    let mut f=fixture();
    let alias=f.doc.add_object(Object::Reference(f.element));
    f.doc.get_dictionary_mut(f.root).unwrap().set("K",vec![Object::Reference(alias)]);
    let nums=f.doc.get_dictionary_mut(f.parent_tree).unwrap().get_mut(b"Nums").unwrap().as_array_mut().unwrap();
    nums[3]=alias.into();
    f.doc.add_object(dictionary! {"Type"=>"Page","StructParents"=>0});
    let info=inspect(&f.doc,f.catalog,&[f.page],&mut check).unwrap();
    assert_eq!(info.top,vec![f.element]);
    assert_eq!(info.owners.len(),3);
}

#[test]
fn composition_structure_refuses_collisions_foreign_pages_and_semantic_cycles() {
    for fault in 0..6 {
        let mut f=fixture();
        match fault {
            0=>{f.doc.get_dictionary_mut(f.annotation).unwrap().set("StructParent",0);},
            1=>{f.doc.get_dictionary_mut(f.annotation).unwrap().set("StructParents",7);},
            2=>{f.doc.get_dictionary_mut(f.element).unwrap().set("Pg",f.catalog);},
            3=>{f.doc.get_dictionary_mut(f.element).unwrap().set("P",f.element);},
            4=>{f.doc.get_dictionary_mut(f.element).unwrap().set("K",f.element);},
            _=>{f.doc.get_dictionary_mut(f.parent_tree).unwrap().remove(b"Nums");f.doc.get_dictionary_mut(f.parent_tree).unwrap().set("Kids",vec![Object::Reference(f.parent_tree)]);},
        }
        assert!(inspect(&f.doc,f.catalog,&[f.page],&mut check).is_err(),"fault {fault}");
    }
}

#[test]
fn composition_structure_empty_and_untagged_documents_have_explicit_empty_ownership() {
    let mut f=fixture();
    f.doc.get_dictionary_mut(f.catalog).unwrap().remove(b"StructTreeRoot");
    assert!(inspect(&f.doc,f.catalog,&[f.page],&mut check).is_err(),"orphaned owner keys cannot be accepted");
    f.doc.get_dictionary_mut(f.page).unwrap().remove(b"StructParents");
    f.doc.get_dictionary_mut(f.annotation).unwrap().remove(b"StructParent");
    f.doc.get_object_mut(f.form).unwrap().as_stream_mut().unwrap().dict.remove(b"StructParents");
    assert_eq!(inspect(&f.doc,f.catalog,&[f.page],&mut check).unwrap(),StructureInfo::default());
    f.doc.get_dictionary_mut(f.catalog).unwrap().set("StructTreeRoot",f.root);
    f.doc.get_dictionary_mut(f.root).unwrap().remove(b"K");
    f.doc.get_dictionary_mut(f.root).unwrap().remove(b"ParentTree");
    let info=inspect(&f.doc,f.catalog,&[f.page],&mut check).unwrap();
    assert_eq!(info.root,Some(f.root));assert!(info.elements.is_empty() && info.owners.is_empty());
}

#[test]
fn composition_structure_cancellation_never_mutates_the_source() {
    let f=fixture();let before=f.doc.objects.clone();let mut completed=false;
    for stop in 0..3000 {
        let mut calls=0;
        let result=inspect(&f.doc,f.catalog,&[f.page],&mut || {
            calls+=1;
            if calls>stop {Err(EngineError::InvalidRequest("test cancellation".into()))} else {Ok(())}
        });
        assert_eq!(f.doc.objects,before);
        if result.is_ok(){completed=true;break;}
        assert!(result.unwrap_err().to_string().contains("test cancellation"));
    }
    assert!(completed);
}

#[test]
fn composition_structure_shared_objects_keep_one_parent_across_page_occurrences() {
    let mut f = fixture();
    let pages = f.doc.get_dictionary(f.page).unwrap().get(b"Parent").unwrap().as_reference().unwrap();
    let second = f.doc.add_object(dictionary! {"Type"=>"Page", "Parent"=>pages,
        "Annots"=>vec![Object::Reference(f.annotation)], "Resources"=>dictionary! {"XObject"=>dictionary! {"Fm"=>f.form}}});
    f.doc.get_dictionary_mut(pages).unwrap().set("Kids",vec![Object::Reference(f.page),second.into()]);
    f.doc.get_dictionary_mut(pages).unwrap().set("Count",2);
    let more = vec![Object::Dictionary(dictionary! {"Type"=>"OBJR","Obj"=>f.annotation,"Pg"=>second}),
        Object::Dictionary(dictionary! {"Type"=>"MCR","Stm"=>f.form,"MCID"=>0,"Pg"=>second})];
    f.doc.get_dictionary_mut(f.element).unwrap().get_mut(b"K").unwrap().as_array_mut().unwrap().extend(more.clone());
    assert_eq!(inspect(&f.doc,f.catalog,&[f.page,second],&mut check).unwrap().owners.len(),3);
    let good = f.doc.clone();
    for repeated in more.clone() {
        f.doc = good.clone();
        f.doc.get_dictionary_mut(f.element).unwrap().get_mut(b"K").unwrap().as_array_mut().unwrap().push(repeated);
        assert!(inspect(&f.doc,f.catalog,&[f.page,second],&mut check).is_err(),"duplicate owner/page");
    }
    for conflicting in more {
        f.doc = good.clone();
        // Move the second-page reference under a different logical parent.
        let kids=f.doc.get_dictionary_mut(f.element).unwrap().get_mut(b"K").unwrap().as_array_mut().unwrap();
        kids.retain(|value| value != &conflicting);
        let other=f.doc.add_object(dictionary! {"Type"=>"StructElem","S"=>"Span","P"=>f.root,"K"=>conflicting});
        f.doc.get_dictionary_mut(f.root).unwrap().set("K",vec![Object::Reference(f.element),other.into()]);
        assert!(inspect(&f.doc,f.catalog,&[f.page,second],&mut check).is_err(),"conflicting owner parent");
    }
}

#[test]
fn composition_structure_number_tree_ranges_and_keys_are_exact() {
    for fault in 0..10 {
        let mut f=fixture();
        let nums=f.doc.get_dictionary_mut(f.parent_tree).unwrap().remove(b"Nums").unwrap().as_array().unwrap().clone();
        let first=f.doc.add_object(dictionary! {"Nums"=>nums[..2].to_vec(),"Limits"=>vec![0.into(),0.into()]});
        let last=f.doc.add_object(dictionary! {"Nums"=>nums[2..].to_vec(),"Limits"=>vec![1.into(),2.into()]});
        f.doc.get_dictionary_mut(f.parent_tree).unwrap().set("Kids",vec![Object::Reference(first),last.into()]);
        assert!(inspect(&f.doc,f.catalog,&[f.page],&mut check).is_ok());
        match fault {
            0=>f.doc.get_dictionary_mut(first).unwrap().set("Limits",vec![0.into(),1.into()]),
            1=>f.doc.get_dictionary_mut(last).unwrap().set("Nums",vec![2.into(),nums[5].clone(),1.into(),nums[3].clone()]),
            2=>f.doc.get_dictionary_mut(last).unwrap().set("Nums",vec![0.into(),nums[1].clone(),2.into(),nums[5].clone()]),
            3=>f.doc.get_dictionary_mut(first).unwrap().set("Nums",vec![(-1).into(),nums[1].clone()]),
            4=>f.doc.get_dictionary_mut(first).unwrap().set("Nums",vec![0.into()]),
            5=>f.doc.get_dictionary_mut(first).unwrap().set("Kids",vec![Object::Reference(last)]),
            6=>f.doc.get_dictionary_mut(first).unwrap().set("Nums",Vec::<Object>::new()),
            7=>f.doc.get_dictionary_mut(f.parent_tree).unwrap().set("Kids",vec![Object::Reference(last),first.into()]),
            8=>f.doc.get_dictionary_mut(f.root).unwrap().set("ParentTreeNextKey",2),
            _=>f.doc.get_dictionary_mut(f.parent_tree).unwrap().set("Kids",vec![Object::Reference(first),first.into(),last.into()]),
        }
        assert!(inspect(&f.doc,f.catalog,&[f.page],&mut check).is_err(),"fault {fault}");
    }
}

#[test]
#[ignore = "requires explicit authored Word/Pictures corpus path"]
fn composition_structure_authored_corpus() {
    use sha2::{Digest,Sha256};
    let root=std::path::PathBuf::from(std::env::var_os("FOLIO_STRUCTURE_CORPUS").expect("explicit authored corpus directory"));
    for (name,hash,expected_owners) in [
        ("Word.pdf","4ec7d9338f3c9a5802af41be2d73a6d9a022168c96b529dcea898a10770181f2",3),
        ("Pictures.pdf","09a1fc181586d6735d9899c3f89d22bdb1badaad28b922b76cb470059daba43c",0),
    ] {
        let bytes=std::fs::read(root.join(name)).unwrap();
        let actual_hash=Sha256::digest(&bytes).iter().map(|byte|format!("{byte:02x}")).collect::<String>();
        assert_eq!(actual_hash,hash,"authored input identity");
        let doc=Document::load_mem_with_resource_limits(&bytes,lopdf::ResourceLimits::default()).unwrap();
        let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let pages:Vec<_>=doc.get_pages().into_values().collect();
        assert_eq!(pages.len(),2);
        let info=inspect(&doc,catalog,&pages,&mut check).unwrap();
        assert_eq!(info.owners.len(),expected_owners);
        assert_eq!(info.root.is_some(),expected_owners>0);
        println!("{name}: sha256={hash}, pages={}, elements={}, owners={}",pages.len(),info.elements.len(),info.owners.len());
    }
}
