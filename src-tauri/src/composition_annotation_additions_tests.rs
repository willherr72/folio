use super::*;
use crate::composition_structure::{inspect,Parents};
use lopdf::{dictionary,Object,Stream};

struct Fixture {doc:Document,catalog:ObjectId,pages:Vec<ObjectId>,root:Option<ObjectId>,old:ObjectId,new:Vec<ObjectId>}
fn fixture(tagged:bool)->Fixture {
    let mut doc=Document::with_version("1.7");let parent=doc.new_object_id();let page=doc.new_object_id();let second=doc.new_object_id();
    let root=tagged.then(||doc.new_object_id());let old=doc.new_object_id();
    let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let content=doc.add_object(Stream::new(dictionary!{},b"/P <</MCID 0>> BDC BT /F1 12 Tf 20 150 Td (Original tagged text) Tj ET EMC".to_vec()));
    let a=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","P"=>page,"Rect"=>vec![20.into(),50.into(),40.into(),70.into()],"Contents"=>Object::string_literal("New note one"),"NM"=>Object::string_literal("one")});
    let b=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","P"=>second,"Rect"=>vec![60.into(),50.into(),80.into(),70.into()],"Contents"=>Object::string_literal("New note two"),"NM"=>Object::string_literal("two")});
    let mut pd=dictionary!{"Type"=>"Page","Parent"=>parent,"MediaBox"=>vec![0.into(),0.into(),200.into(),200.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content,"Annots"=>vec![Object::Reference(a)]};
    if tagged{pd.set("StructParents",4);}
    doc.objects.insert(page,pd.into());
    doc.objects.insert(second,dictionary!{"Type"=>"Page","Parent"=>parent,"MediaBox"=>vec![0.into(),0.into(),200.into(),200.into()],"Resources"=>dictionary!{},"Annots"=>vec![Object::Reference(b)]}.into());
    doc.objects.insert(parent,dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(page),second.into()],"Count"=>2}.into());
    let mut cat=dictionary!{"Type"=>"Catalog","Pages"=>parent,"Lang"=>Object::string_literal("en-US")};
    if let Some(root)=root {
        doc.objects.insert(old,dictionary!{"Type"=>"StructElem","S"=>"P","P"=>root,"Pg"=>page,"K"=>0,"Alt"=>Object::string_literal("Original description"),"ID"=>Object::string_literal("stable-original")}.into());
        let tree=doc.add_object(dictionary!{"Nums"=>vec![4.into(),Object::Array(vec![old.into(),Object::Null])]});
        doc.objects.insert(root,dictionary!{"Type"=>"StructTreeRoot","K"=>old,"ParentTree"=>tree,"ParentTreeNextKey"=>9}.into());
        cat.set("StructTreeRoot",root);cat.set("MarkInfo",dictionary!{"Marked"=>true,"Suspects"=>true});
    }
    let catalog=doc.add_object(cat);doc.trailer.set("Root",catalog);
    Fixture{doc,catalog,pages:vec![page,second],root,old,new:vec![a,b]}
}
fn language(bytes:&[u8])->SourceLanguage {
    let mut doc=Document::with_version("1.7");let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Lang"=>Object::string_literal(bytes)});
    SourceLanguage::capture(&doc,catalog).unwrap()
}
#[test]
fn composition_annotation_additions_preserve_existing_tree_and_source_languages() {
    let f=fixture(true);let before=inspect(&f.doc,f.catalog,&f.pages,&mut ||Ok(())).unwrap();
    let original=f.doc.objects[&f.old].clone();let french=language(b"fr-CA");let unspecified=language(b"");
    let requests=[NewOwners{page:f.pages[1],annotations:&f.new[1..],language:&french},NewOwners{page:f.pages[0],annotations:&f.new[..1],language:&unspecified}];
    let mut result=append(f.doc.clone(),&requests,&mut ||Ok(())).unwrap();
    let after=inspect(&result,f.catalog,&f.pages,&mut ||Ok(())).unwrap();
    assert_eq!(after.root,f.root);assert_eq!(after.top[0],f.old);assert_eq!(after.top.len(),3);assert_eq!(after.owners[&4],before.owners[&4]);assert_eq!(result.objects[&f.old],original);
    for (i,(annotation,lang)) in [(f.new[1],b"fr-CA".as_slice()),(f.new[0],b"".as_slice())].into_iter().enumerate(){
        let owner=after.top[i+1];let key=result.get_dictionary(annotation).unwrap().get(b"StructParent").unwrap().as_i64().unwrap();
        assert_eq!(key,9+i as i64);assert_eq!(after.owners[&key].object,annotation);assert_eq!(after.owners[&key].parents,Parents::Whole(owner));
        assert_eq!(result.get_dictionary(owner).unwrap().get(b"Lang").unwrap().as_str().unwrap(),lang);
    }
    for page in &f.pages{assert_eq!(result.objects[page],f.doc.objects[page]);}
    assert_eq!(result.get_dictionary(f.catalog).unwrap().get(b"MarkInfo").unwrap(),f.doc.get_dictionary(f.catalog).unwrap().get(b"MarkInfo").unwrap());
    capture("tagged",&f.doc,&result);
    let bytes=crate::pdf_output::save(&mut result).unwrap();let reopened=crate::pdf_output::load(&bytes).unwrap();assert_eq!(inspect(&reopened,f.catalog,&f.pages,&mut ||Ok(())).unwrap(),after);
}
#[test]
fn composition_annotation_additions_create_partial_tree_without_claiming_full_tagging() {
    let f=fixture(false);let lang=language(b"de");
    let requests=[NewOwners{page:f.pages[0],annotations:&f.new[..1],language:&lang}];
    let result=append(f.doc.clone(),&requests,&mut ||Ok(())).unwrap();
    let info=inspect(&result,f.catalog,&f.pages,&mut ||Ok(())).unwrap();assert_eq!(info.top.len(),1);assert_eq!(info.owners.len(),1);
    let mark=result.get_dictionary(f.catalog).unwrap().get(b"MarkInfo").unwrap().as_dict().unwrap();assert!(!mark.get(b"Marked").unwrap().as_bool().unwrap());
    assert!(!result.get_dictionary(f.new[1]).unwrap().has(b"StructParent"));
    capture("partial",&f.doc,&result);
}

fn capture(name:&str,before:&Document,after:&Document){
    if let Some(root)=std::env::var_os("FOLIO_STRUCTURED_OVERLAY_CAPTURE_DIR") {
        use std::io::Write;let directory=std::path::PathBuf::from(root);std::fs::create_dir_all(&directory).unwrap();
        for (kind,doc) in [("before",before),("after",after)] {
            let bytes=crate::pdf_output::save(&mut doc.clone()).unwrap();
            std::fs::OpenOptions::new().write(true).create_new(true).open(directory.join(format!("owners-{name}-{kind}.pdf"))).unwrap().write_all(&bytes).unwrap();
        }
    }
}
#[test]
fn composition_annotation_additions_refuse_wrong_or_shared_roots_and_semantic_reuse(){
    for mode in ["wrong-page","missing-page","detached","shared-page","repeat-slot","duplicate-request","already-owned","remapped-role","tree-extension","key-overflow"]{
        let mut f=fixture(true);let lang=language(b"ja");let mut page=f.pages[0];let mut ids=vec![f.new[0]];
        match mode {
            "wrong-page"=>page=f.pages[1],"missing-page"=>page=(99999,0),
            "detached"=>{f.doc.get_dictionary_mut(f.pages[0]).unwrap().remove(b"Annots");},
            "shared-page"=>{f.doc.get_dictionary_mut(f.pages[1]).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().push(f.new[0].into());},
            "repeat-slot"=>{f.doc.get_dictionary_mut(f.pages[0]).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().push(f.new[0].into());},
            "duplicate-request"=>ids.push(f.new[0]),
            "already-owned"=>{f.doc.get_dictionary_mut(f.new[0]).unwrap().set("Lang",Object::string_literal("de"));},
            "remapped-role"=>{f.doc.get_dictionary_mut(f.root.unwrap()).unwrap().set("RoleMap",dictionary!{"Annot"=>"Figure"});},
            "tree-extension"=>{let tree=f.doc.get_dictionary(f.root.unwrap()).unwrap().get(b"ParentTree").unwrap().as_reference().unwrap();f.doc.get_dictionary_mut(tree).unwrap().set("ProducerExtension",Object::string_literal("preserve me"));},
            "key-overflow"=>{f.doc.get_dictionary_mut(f.root.unwrap()).unwrap().set("ParentTreeNextKey",i64::MAX);},_=>unreachable!(),
        }
        let request=[NewOwners{page,annotations:&ids,language:&lang}];
        assert!(append(f.doc,&request,&mut ||Ok(())).is_err(),"{mode}");
    }
}
#[test]
fn composition_annotation_additions_aliases_number_trees_and_repeated_edits_preserve_owners(){
    let mut f=fixture(true);let lang=language(b"es");let root=f.root.unwrap();
    let old_tree=f.doc.get_dictionary(root).unwrap().get(b"ParentTree").unwrap().as_reference().unwrap();
    f.doc.get_dictionary_mut(old_tree).unwrap().set("Limits",vec![4.into(),4.into()]);
    let branch=f.doc.add_object(dictionary!{"Kids"=>vec![Object::Reference(old_tree)],"Limits"=>vec![4.into(),4.into()]});
    f.doc.get_dictionary_mut(root).unwrap().set("ParentTree",branch);
    let old_alias=f.doc.add_object(Object::Reference(f.old));let kids=f.doc.add_object(Object::Array(vec![old_alias.into()]));f.doc.get_dictionary_mut(root).unwrap().set("K",kids);
    let annotation_alias=f.doc.add_object(Object::Reference(f.new[0]));f.doc.get_dictionary_mut(f.pages[0]).unwrap().set("Annots",vec![Object::Reference(annotation_alias)]);
    let original=f.doc.objects.clone();let request=[NewOwners{page:f.pages[0],annotations:&f.new[..1],language:&lang}];
    let first=append(f.doc,&request,&mut ||Ok(())).unwrap();let initial=inspect(&first,f.catalog,&f.pages,&mut ||Ok(())).unwrap();
    assert_eq!(first.get_dictionary(root).unwrap().get(b"K").unwrap().as_array().unwrap()[0],Object::Reference(old_alias));
    for id in [old_tree,branch,old_alias,kids,annotation_alias,f.old]{assert_eq!(first.objects[&id],original[&id]);}
    let request=[NewOwners{page:f.pages[1],annotations:&f.new[1..],language:&lang}];
    let second=append(first,&request,&mut ||Ok(())).unwrap();let after=inspect(&second,f.catalog,&f.pages,&mut ||Ok(())).unwrap();
    assert!(after.top.starts_with(&initial.top));for (key,value) in initial.owners{assert_eq!(after.owners[&key],value);}
    assert_eq!(second.get_dictionary(root).unwrap().get(b"ParentTreeNextKey").unwrap().as_i64().unwrap(),11);
    assert!(append(second.clone(),&request,&mut ||Ok(())).is_err(),"replaying a new-owner request must not retag existing objects");
    let empty=[NewOwners{page:f.pages[0],annotations:&[],language:&lang}];let same=append(second.clone(),&empty,&mut ||Ok(())).unwrap();assert_eq!(same.objects,second.objects);assert_eq!(same.trailer,second.trailer);
}
#[test]
fn composition_annotation_additions_bound_allocations_and_cancel_before_return(){
    let f=fixture(true);let lang=language(b"en");let request=[NewOwners{page:f.pages[0],annotations:&f.new[..1],language:&lang}];
    for (bytes,steps) in [(0,1_000_000),(1024,1_000_000),(16*1024*1024,1)]{assert!(append_bounded(f.doc.clone(),&request,bytes,steps,&mut ||Ok(())).is_err());}
    let mut calls=0;append(f.doc.clone(),&request,&mut ||{calls+=1;Ok(())}).unwrap();
    for stop in [1,calls/2,calls]{let mut seen=0;assert!(append(f.doc.clone(),&request,&mut ||{seen+=1;if seen==stop{Err(bad("cancelled"))}else{Ok(())}}).is_err());}
    let mut oversized=f.doc.clone();oversized.catalog_mut().unwrap().set("MarkInfo",dictionary!{"Custom"=>Object::string_literal(vec![b'x';100_000])});
    oversized.catalog_mut().unwrap().remove(b"StructTreeRoot");oversized.get_dictionary_mut(f.pages[0]).unwrap().remove(b"StructParents");
    assert!(append_bounded(oversized,&request,20_000,1_000_000,&mut ||Ok(())).is_err(),"MarkInfo must be charged before copying");
}
#[test]
fn composition_annotation_additions_capture_validates_source_language(){
    for bytes in [b"en--US".as_slice(),b"en US",b"\xff",b"\xfe\xff\x00",b"123456789"]{
        let mut doc=Document::with_version("1.7");let cat=doc.add_object(dictionary!{"Type"=>"Catalog","Lang"=>Object::string_literal(bytes)});assert!(SourceLanguage::capture(&doc,cat).is_err());
    }
    for bytes in [b"".as_slice(),b"fr-CA",b"\xfe\xff\x00e\x00n",b"\xef\xbb\xbfen"]{assert_eq!(language(bytes).0,bytes);}
}
