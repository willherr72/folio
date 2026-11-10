use super::*;
use crate::composition_structure::{inspect,Parents};
use lopdf::{dictionary,Stream};

struct Fixture{doc:Document,catalog:ObjectId,page:ObjectId,second:ObjectId,annotations:Vec<ObjectId>,appearance:ObjectId}
fn fixture()->Fixture{
    let mut doc=Document::with_version("1.7");let pages=doc.new_object_id();let page=doc.new_object_id();let second=doc.new_object_id();
    let appearance=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),200.into(),200.into()],"Resources"=>dictionary!{}},b"0 0 1 rg 10 10 30 40 re f".to_vec()));
    let annotations=(0..2).map(|i|doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Stamp","P"=>page,"Rect"=>vec![0.into(),0.into(),200.into(),200.into()],"NM"=>Object::string_literal(format!("drawing-{i}")),"AP"=>dictionary!{"N"=>appearance}})).collect::<Vec<_>>();
    doc.objects.insert(page,dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),200.into(),200.into()],"Resources"=>dictionary!{},"Annots"=>annotations.iter().map(|id|Object::Reference(*id)).collect::<Vec<_>>()}.into());
    doc.objects.insert(second,dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),200.into(),200.into()],"Resources"=>dictionary!{}}.into());
    doc.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>2,"Kids"=>vec![Object::Reference(page),second.into()]}.into());
    let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages,"Lang"=>Object::string_literal("en-US")});doc.trailer.set("Root",catalog);
    let french=SourceLanguage(b"fr-CA".to_vec());doc=super::super::append(doc,&[NewOwners{page,annotations:&annotations,language:&french}],&mut ||Ok(())).unwrap();
    Fixture{doc,catalog,page,second,annotations,appearance}
}
fn requests(f:&Fixture)->Vec<FlattenOwner>{f.annotations.iter().map(|id|FlattenOwner{page:f.page,annotation:*id,appearance:f.appearance}).collect()}
#[test]
fn composition_annotation_flatten_retains_order_language_and_owners_after_save(){
    let f=fixture();let before=inspect(&f.doc,f.catalog,&[f.page,f.second],&mut ||Ok(())).unwrap();let original=f.doc.objects[&f.appearance].clone();
    let mut out=mark_flattened(f.doc.clone(),&requests(&f),&mut ||Ok(())).unwrap();
    assert_eq!(out.forms.len(),2);assert_ne!(out.forms[0],out.forms[1]);
    let after=inspect(&out.document,f.catalog,&[f.page,f.second],&mut ||Ok(())).unwrap();
    assert_eq!(after.top,before.top);assert_eq!(after.elements,before.elements);assert_eq!(after.root,before.root);assert_eq!(after.owners.len(),before.owners.len());
    for (i,id) in f.annotations.iter().enumerate(){
        let key=f.doc.get_dictionary(*id).unwrap().get(b"StructParent").unwrap().as_i64().unwrap();let parent=before.top[i];
        assert_eq!(after.owners[&key].object,out.forms[i]);assert_eq!(after.owners[&key].parents,Parents::Marked(vec![Some(parent)]));
        let mut expected=f.doc.get_dictionary(parent).unwrap().clone();expected.set("K",dictionary!{"Type"=>"MCR","Pg"=>f.page,"Stm"=>out.forms[i],"MCID"=>0});assert_eq!(out.document.get_dictionary(parent).unwrap(),&expected);
        assert!(!out.document.get_dictionary(*id).unwrap().has(b"StructParent"));
        let stream=out.document.objects[&out.forms[i]].as_stream().unwrap();assert_eq!(stream.dict.get(b"StructParents").unwrap().as_i64().unwrap(),key);
        let ops=lopdf::content::Content::decode(&stream.content).unwrap().operations;
        assert_eq!(ops.iter().map(|o|o.operator.as_str()).collect::<Vec<_>>(),vec!["BDC","Do","EMC"]);
        assert_eq!(ops[0].operands[1].as_dict().unwrap().get(b"MCID").unwrap().as_i64().unwrap(),0);
        assert_eq!(stream.dict.get(b"Resources").unwrap().as_dict().unwrap().get(b"XObject").unwrap().as_dict().unwrap().get(b"Drawing").unwrap().as_reference().unwrap(),f.appearance);
    }
    assert_eq!(out.document.objects[&f.appearance],original);assert_eq!(out.document.objects[&f.page],f.doc.objects[&f.page]);assert_eq!(out.document.objects[&f.second],f.doc.objects[&f.second]);
    let bytes=crate::pdf_output::save(&mut out.document).unwrap();let reopened=crate::pdf_output::load(&bytes).unwrap();assert_eq!(inspect(&reopened,f.catalog,&[f.page,f.second],&mut ||Ok(())).unwrap(),after);
}
#[test]
fn composition_annotation_flatten_empty_is_exact_and_cancellable(){
    let f=fixture();let out=mark_flattened(f.doc.clone(),&[],&mut ||Ok(())).unwrap();assert_eq!(out.document.objects,f.doc.objects);assert!(out.forms.is_empty());
    assert!(mark_flattened(f.doc,&[],&mut ||Err(EngineError::InvalidRequest("cancel".into()))).is_err());
}

#[test]
fn composition_annotation_flatten_refuses_stale_shared_and_opaque_ownership(){
    for mode in ["wrong-page","detached","shared","repeat","unowned","language","tree-extension","objr-extension","matrix","bounds","not-form","owned-form","max-id"]{
        let mut f=fixture();let mut plan=requests(&f);let info=inspect(&f.doc,f.catalog,&[f.page,f.second],&mut ||Ok(())).unwrap();
        match mode{
            "wrong-page"=>plan[0].page=f.second,
            "detached"=>{f.doc.get_dictionary_mut(f.page).unwrap().remove(b"Annots");},
            "shared"=>f.doc.get_dictionary_mut(f.second).unwrap().set("Annots",vec![Object::Reference(f.annotations[0])]),
            "repeat"=>plan.push(FlattenOwner{page:f.page,annotation:f.annotations[0],appearance:f.appearance}),
            "unowned"=>{f.doc.get_dictionary_mut(f.annotations[0]).unwrap().remove(b"StructParent");},
            "language"=>f.doc.get_dictionary_mut(f.annotations[0]).unwrap().set("Lang",Object::string_literal("de")),
            "tree-extension"=>{let tree=f.doc.get_dictionary(info.root.unwrap()).unwrap().get(b"ParentTree").unwrap().as_reference().unwrap();f.doc.get_dictionary_mut(tree).unwrap().set("Custom",true);},
            "objr-extension"=>f.doc.get_dictionary_mut(info.top[0]).unwrap().get_mut(b"K").unwrap().as_dict_mut().unwrap().set("Custom",true),
            "matrix"=>f.doc.objects.get_mut(&f.appearance).unwrap().as_stream_mut().unwrap().dict.set("Matrix",vec![2.into(),0.into(),0.into(),2.into(),0.into(),0.into()]),
            "bounds"=>f.doc.objects.get_mut(&f.appearance).unwrap().as_stream_mut().unwrap().dict.set("BBox",vec![0.into(),0.into(),0.into(),1.into()]),
            "not-form"=>f.doc.objects.get_mut(&f.appearance).unwrap().as_stream_mut().unwrap().dict.set("Subtype","Image"),
            "owned-form"=>f.doc.objects.get_mut(&f.appearance).unwrap().as_stream_mut().unwrap().dict.set("StructParents",30),
            "max-id"=>f.doc.max_id=u32::MAX,_=>unreachable!(),
        }
        assert!(mark_flattened(f.doc,&plan,&mut ||Ok(())).is_err(),"{mode}");
    }
}
#[test]
fn composition_annotation_flatten_bounds_and_cancel_never_return_partial_graph(){
    let f=fixture();let plan=requests(&f);let mut calls=0;mark_flattened(f.doc.clone(),&plan,&mut ||{calls+=1;Ok(())}).unwrap();
    for stop in [1,calls/3,calls/2,calls]{let mut seen=0;assert!(mark_flattened(f.doc.clone(),&plan,&mut ||{seen+=1;if seen==stop{Err(bad("cancelled"))}else{Ok(())}}).is_err());}
    for (bytes,steps) in [(0,1_000_000),(16*1024*1024,1),(1024,1_000_000)]{assert!(convert(f.doc.clone(),&plan,bytes,steps,&mut ||Ok(())).is_err());}
}
#[test]
fn composition_annotation_flatten_copies_alias_kids_without_mutating_other_consumers(){
    let mut f=fixture();let info=inspect(&f.doc,f.catalog,&[f.page,f.second],&mut ||Ok(())).unwrap();let first=info.top[0];let second=info.top[1];
    let a=f.doc.get_dictionary(first).unwrap().get(b"K").unwrap().clone();let b=f.doc.get_dictionary(second).unwrap().get(b"K").unwrap().clone();
    let objr=f.doc.add_object(a);let alias=f.doc.add_object(Object::Reference(objr));let array=f.doc.add_object(Object::Array(vec![alias.into(),b]));
    f.doc.get_dictionary_mut(first).unwrap().set("K",array);f.doc.get_dictionary_mut(first).unwrap().set("Alt",Object::string_literal("authored description"));
    f.doc.get_dictionary_mut(second).unwrap().remove(b"K");let tree=f.doc.get_dictionary(info.root.unwrap()).unwrap().get(b"ParentTree").unwrap().as_reference().unwrap();
    let nums=f.doc.get_dictionary_mut(tree).unwrap().get_mut(b"Nums").unwrap().as_array_mut().unwrap();nums[3]=first.into();
    let original_array=f.doc.objects[&array].clone();let original_objr=f.doc.objects[&objr].clone();
    let out=mark_flattened(f.doc.clone(),&requests(&f)[..1],&mut ||Ok(())).unwrap();
    assert_eq!(out.document.objects[&array],original_array);assert_eq!(out.document.objects[&objr],original_objr);
    let after=inspect(&out.document,f.catalog,&[f.page,f.second],&mut ||Ok(())).unwrap();assert_eq!(after.top,info.top);assert_eq!(after.owners[&1].object,f.annotations[1]);assert_eq!(after.owners[&1].parents,Parents::Whole(first));
    assert_eq!(out.document.get_dictionary(first).unwrap().get(b"Alt").unwrap().as_str().unwrap(),b"authored description");
    // The untouched indirect OBJR still points to the annotation. A later
    // removal must not hide an opaque live consumer by rewriting that object.
    assert_eq!(out.document.get_dictionary(objr).unwrap().get(b"Obj").unwrap().as_reference().unwrap(),f.annotations[0]);
}

#[test]
fn composition_annotation_flatten_preserves_original_marked_slots_and_descriptions(){
    let mut f=fixture();let mut before=inspect(&f.doc,f.catalog,&[f.page,f.second],&mut ||Ok(())).unwrap();let root=before.root.unwrap();
    let old=f.doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"P","P"=>root,"Pg"=>f.second,"Lang"=>Object::string_literal("ja"),"Alt"=>Object::string_literal("Original author description"),"K"=>0});
    let content=f.doc.add_object(Stream::new(dictionary!{},b"/P <</MCID 0>> BDC 0 0 20 20 re f EMC".to_vec()));f.doc.get_dictionary_mut(f.second).unwrap().set("Contents",content);f.doc.get_dictionary_mut(f.second).unwrap().set("StructParents",9);
    f.doc.get_dictionary_mut(root).unwrap().get_mut(b"K").unwrap().as_array_mut().unwrap().insert(0,old.into());f.doc.get_dictionary_mut(root).unwrap().set("ParentTreeNextKey",10);
    let tree=f.doc.get_dictionary(root).unwrap().get(b"ParentTree").unwrap().as_reference().unwrap();f.doc.get_dictionary_mut(tree).unwrap().get_mut(b"Nums").unwrap().as_array_mut().unwrap().extend([9.into(),Object::Array(vec![old.into(),Object::Null])]);
    before=inspect(&f.doc,f.catalog,&[f.page,f.second],&mut ||Ok(())).unwrap();let out=mark_flattened(f.doc.clone(),&requests(&f),&mut ||Ok(())).unwrap();let after=inspect(&out.document,f.catalog,&[f.page,f.second],&mut ||Ok(())).unwrap();
    assert_eq!(after.owners[&9],before.owners[&9]);assert_eq!(after.top,before.top);assert_eq!(out.document.objects[&old],f.doc.objects[&old]);assert_eq!(out.document.objects[&content],f.doc.objects[&content]);assert_eq!(out.document.objects[&f.second],f.doc.objects[&f.second]);
}

#[test]
fn composition_annotation_flatten_cannot_hide_opaque_objr_consumers_from_removal(){
    use crate::composition_annotation_receipts::{Anchor,Receipt};
    use crate::composition_annotation_removal::{Removal,remove_owned};
    for opaque in [false,true]{
        let mut f=fixture();let before=inspect(&f.doc,f.catalog,&[f.page,f.second],&mut ||Ok(())).unwrap();let parent=before.top[0];let objr=f.doc.add_object(f.doc.get_dictionary(parent).unwrap().get(b"K").unwrap().clone());f.doc.get_dictionary_mut(parent).unwrap().set("K",objr);
        if opaque{f.doc.get_dictionary_mut(f.catalog).unwrap().set("CustomConsumer",objr);}
        let popup=f.doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Popup","P"=>f.page,"Parent"=>f.annotations[0],"Rect"=>vec![0.into(),0.into(),20.into(),20.into()]});f.doc.get_dictionary_mut(f.annotations[0]).unwrap().set("Popup",popup);f.doc.get_dictionary_mut(f.page).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().push(popup.into());
        let marked=mark_flattened(f.doc.clone(),&requests(&f)[..1],&mut ||Ok(())).unwrap();
        let receipt=|id,slot|Receipt{origin:Anchor{page:f.page,slot,annotation:id},output_page:f.page,output_slot:slot,annotation:id};let popups=[receipt(popup,2)];
        let output=remove_owned(marked.document,&[Removal{annotation:receipt(f.annotations[0],0),popups:&popups}],&mut ||Ok(()));
        if opaque{assert!(output.is_err());}else{let doc=output.unwrap();let after=inspect(&doc,f.catalog,&[f.page,f.second],&mut ||Ok(())).unwrap();assert_eq!(after.top,before.top);assert_eq!(after.owners[&0].parents,Parents::Marked(vec![Some(parent)]));assert_eq!(doc.get_dictionary(f.page).unwrap().get(b"Annots").unwrap().as_array().unwrap(),&vec![Object::Reference(f.annotations[1])]);}
    }
}
