use super::*;
use crate::composition_annotation_receipts::{Anchor,Receipt};
use lopdf::{dictionary,Stream};
fn fixture()->(Document,ObjectId,ObjectId,ObjectId,ObjectId) {
    let mut doc=Document::with_version("1.7");let tree=doc.new_object_id();
    let content=doc.add_object(Stream::new(dictionary!{},b"0 0 1 rg 10 10 30 30 re f".to_vec()));
    let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary!{},"Contents"=>content});
    let old_ap=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),30.into(),20.into()]},b"1 0 0 rg 0 0 30 20 re f".to_vec()));
    let original=doc.new_object_id();let popup=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Popup","Parent"=>original,"P"=>page,"Rect"=>vec![80.into(),80.into(),200.into(),150.into()],"Open"=>true});
    doc.objects.insert(original,dictionary!{"Type"=>"Annot","Subtype"=>"FreeText","P"=>page,"Rect"=>vec![50.into(),50.into(),80.into(),70.into()],"AP"=>dictionary!{"N"=>old_ap},"Contents"=>Object::string_literal("Original"),"F"=>4,"NM"=>Object::string_literal("authored-name"),"T"=>Object::string_literal("Original author"),"CreationDate"=>Object::string_literal("D:20260101000000Z"),"Popup"=>popup,"PrivateData"=>dictionary!{"Keep"=>Object::string_literal("metadata sentinel")}}.into());
    let new_ap=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),30.into(),20.into()]},b"0 1 0 rg 0 0 30 20 re f".to_vec()));
    let generated=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"FreeText","P"=>page,"Rect"=>vec![100.into(),100.into(),130.into(),120.into()],"AP"=>dictionary!{"N"=>new_ap},"Contents"=>Object::string_literal("Changed"),"F"=>4,"NM"=>Object::string_literal("workspace-label"),"T"=>Object::string_literal("Folio"),"DA"=>Object::string_literal("/F1 12 Tf 0 1 0 rg"),"Q"=>0,"C"=>vec![0.into(),1.into(),0.into()]});
    let alias=doc.add_object(Object::Reference(original));let list=doc.add_object(vec![Object::Reference(alias),popup.into(),generated.into()]);
    doc.get_dictionary_mut(page).unwrap().set("Annots",list);
    doc.objects.insert(tree,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into());
    let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree});doc.trailer.set("Root",catalog);
    (doc,page,original,popup,generated)
}
fn receipt(page:ObjectId,slot:usize,id:ObjectId)->Receipt {Receipt{origin:Anchor{page,slot,annotation:id},output_page:page,output_slot:slot,annotation:id}}
#[test]
fn annotation_replacement_keeps_source_identity_popup_and_authored_metadata() {
    let (doc,page,original,popup,generated)=fixture();let before=doc.objects.clone();let created=doc.get_dictionary(generated).unwrap().clone();
    let mut output=replace_owned(doc,&[Replacement{original:receipt(page,0,original),popups:&[receipt(page,1,popup)],generated}],&mut ||Ok(())).unwrap();
    let target=output.get_dictionary(original).unwrap();
    for key in [b"NM".as_slice(),b"T",b"CreationDate",b"Popup",b"PrivateData",b"P",b"F"] {assert_eq!(target.get(key).unwrap(),before[&original].as_dict().unwrap().get(key).unwrap());}
    for key in [b"Rect".as_slice(),b"AP",b"Contents",b"DA",b"Q",b"C"] {assert_eq!(target.get(key).unwrap(),created.get(key).unwrap());}
    let list=output.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap();assert_eq!(list.len(),2);
    assert_eq!(output.dereference(&list[0]).unwrap().0,Some(original));assert_eq!(list[1],Object::Reference(popup));
    assert_eq!(output.get_dictionary(popup).unwrap(),before[&popup].as_dict().unwrap());
    for (id,value) in before {if ![page,original,generated].contains(&id){assert_eq!(output.objects.get(&id),Some(&value));}}
    let mut bytes=vec![];output.save_to(&mut bytes).unwrap();let reopened=Document::load_mem(&bytes).unwrap();
    assert_eq!(reopened.get_dictionary(original).unwrap().get(b"Contents").unwrap(),&Object::string_literal("Changed"));
}
#[test]
fn annotation_replacement_retains_tag_tree_and_parent_tree_identities() {
    let (mut doc,page,original,popup,generated)=fixture();let root=doc.new_object_id();
    let element=doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"Annot","P"=>root,"Pg"=>page,"Lang"=>Object::string_literal("en-GB"),"K"=>dictionary!{"Type"=>"OBJR","Obj"=>original}});
    doc.objects.insert(root,dictionary!{"Type"=>"StructTreeRoot","K"=>element,"ParentTree"=>dictionary!{"Nums"=>vec![Object::Integer(0),element.into()]},"ParentTreeNextKey"=>1}.into());
    doc.get_dictionary_mut(original).unwrap().set("StructParent",0);
    let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();doc.get_dictionary_mut(catalog).unwrap().set("StructTreeRoot",root);
    let before=crate::composition_structure::inspect(&doc,catalog,&[page],&mut ||Ok(())).unwrap();let root_before=doc.objects[&root].clone();let element_before=doc.objects[&element].clone();
    let output=replace_owned(doc,&[Replacement{original:receipt(page,0,original),popups:&[receipt(page,1,popup)],generated}],&mut ||Ok(())).unwrap();
    assert_eq!(output.objects[&root],root_before);assert_eq!(output.objects[&element],element_before);
    assert_eq!(crate::composition_structure::inspect(&output,catalog,&[page],&mut ||Ok(())).unwrap(),before);
}

#[test]
fn annotation_replacement_refuses_stale_ambiguous_and_unknown_consumers() {
 for mode in ["stale-slot","stale-id","omitted-popup","generated-is-original","duplicate-generated","generated-opaque","source-opaque","source-script","generated-popup","generated-tag","wrong-subtype","alternate-ap","changed-flags","rich-text","generated-unknown"] {
  let (mut doc,page,original,popup,mut generated)=fixture();let mut binding=receipt(page,0,original);
  let popup_receipts=[receipt(page,1,popup)];let mut popups=popup_receipts.as_slice();
  let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
  match mode {
   "stale-slot"=>binding.output_slot=1,
   "stale-id"=>binding.annotation=generated,
   "omitted-popup"=>popups=&[],
   "generated-is-original"=>generated=original,
   "duplicate-generated"=>{doc.get_dictionary_mut(page).unwrap().set("Annots",vec![original.into(),popup.into(),generated.into(),generated.into()]);},
   "generated-opaque"=>{doc.get_dictionary_mut(catalog).unwrap().set("Opaque",generated);},
   "source-opaque"=>{doc.get_dictionary_mut(catalog).unwrap().set("Opaque",original);},
   "source-script"=>{doc.get_dictionary_mut(catalog).unwrap().set("OpenAction",dictionary!{"S"=>"JavaScript","JS"=>Object::string_literal("this.getAnnots()")});},
   "generated-popup"=>{doc.get_dictionary_mut(generated).unwrap().set("Popup",popup);},
   "generated-tag"=>{doc.get_dictionary_mut(generated).unwrap().set("StructParent",0);},
   "wrong-subtype"=>{doc.get_dictionary_mut(generated).unwrap().set("Subtype","Stamp");},
   "alternate-ap"=>{let ap=doc.get_dictionary_mut(original).unwrap().get_mut(b"AP").unwrap().as_dict_mut().unwrap();let normal=ap.get(b"N").unwrap().clone();ap.set("R",normal);},
   "changed-flags"=>{doc.get_dictionary_mut(original).unwrap().set("F",12);},
   "rich-text"=>{doc.get_dictionary_mut(original).unwrap().set("RC",Object::string_literal("authored rich text"));},
   "generated-unknown"=>{doc.get_dictionary_mut(generated).unwrap().set("UnknownDrawing",true);},
   _=>unreachable!(),
  }
  assert!(replace_owned(doc,&[Replacement{original:binding,popups,generated}],&mut ||Ok(())).is_err(),"{mode}");
 }
}

#[test]
fn annotation_replacement_refuses_stale_tag_descriptions_and_attributes() {
 for key in ["Alt","ActualText","E","A","C"] {
  let (mut doc,page,original,popup,generated)=fixture();let root=doc.new_object_id();let group=doc.new_object_id();
  let element=doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"Annot","P"=>group,"Pg"=>page,"K"=>dictionary!{"Type"=>"OBJR","Obj"=>original}});
  let mut parent=dictionary!{"Type"=>"StructElem","S"=>"Div","P"=>root,"K"=>element};parent.set(key,Object::string_literal("authored semantics"));doc.objects.insert(group,parent.into());
  doc.objects.insert(root,dictionary!{"Type"=>"StructTreeRoot","K"=>group,"ParentTree"=>dictionary!{"Nums"=>vec![Object::Integer(0),element.into()]}}.into());
  doc.get_dictionary_mut(original).unwrap().set("StructParent",0);
  let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();doc.get_dictionary_mut(catalog).unwrap().set("StructTreeRoot",root);
  crate::composition_structure::inspect(&doc,catalog,&[page],&mut ||Ok(())).unwrap();
  let error=replace_owned(doc,&[Replacement{original:receipt(page,0,original),popups:&[receipt(page,1,popup)],generated}],&mut ||Ok(())).unwrap_err();
  assert!(error.to_string().contains("semantic relationship"),"{key}: {error}");
 }
}

#[test]
fn annotation_replacement_keeps_independent_copy_and_refuses_shared_root() {
 for shared in [false,true] {
  let (mut doc,page,original,popup,generated)=fixture();
  let tree=doc.get_dictionary(page).unwrap().get(b"Parent").unwrap().as_reference().unwrap();
  let mut copied_page=doc.get_dictionary(page).unwrap().clone();copied_page.remove(b"Annots");let second=doc.add_object(copied_page);
  let other=if shared {doc.get_dictionary_mut(original).unwrap().remove(b"P");original}else{
   let mut copy=doc.get_dictionary(original).unwrap().clone();copy.set("P",second);copy.remove(b"Popup");doc.add_object(copy)
  };
  doc.get_dictionary_mut(second).unwrap().set("Annots",vec![Object::Reference(other)]);
  doc.get_dictionary_mut(tree).unwrap().set("Kids",vec![Object::Reference(page),second.into()]);doc.get_dictionary_mut(tree).unwrap().set("Count",2);
  let before=doc.objects[&other].clone();let page_before=doc.objects[&second].clone();
  let result=replace_owned(doc,&[Replacement{original:receipt(page,0,original),popups:&[receipt(page,1,popup)],generated}],&mut ||Ok(()));
  if shared {assert!(result.unwrap_err().to_string().contains("occurrence isolation"));}else{
   let output=result.unwrap();assert_eq!(output.objects[&other],before);assert_eq!(output.objects[&second],page_before);
  }
 }
}

#[test]
fn annotation_replacement_budget_and_cancellation_never_return_partial_changes() {
 let (doc,page,original,popup,generated)=fixture();let popups=[receipt(page,1,popup)];let requests=[Replacement{original:receipt(page,0,original),popups:&popups,generated}];
 for (metadata,steps) in [(1,1_000_000),(16*1024*1024,1),(2048,1_000_000)] {
  assert!(replace_with_budget(doc.clone(),&requests,metadata,steps,&mut ||Ok(())).is_err());
 }
 let mut calls=0;replace_owned(doc.clone(),&requests,&mut ||{calls+=1;Ok(())}).unwrap();
 for stop in [1,calls/2,calls-1,calls] {
  let mut seen=0;let result=replace_owned(doc.clone(),&requests,&mut ||{seen+=1;if seen==stop{Err(bad("cancelled"))}else{Ok(())}});
  assert!(result.unwrap_err().to_string().contains("cancelled"),"checkpoint {stop}");
 }
 let mut large=doc.clone();let alias=large.add_object(Object::Dictionary(dictionary!{"Payload"=>Object::string_literal(vec![b'x';8192])}));
 large.get_dictionary_mut(page).unwrap().set("Annots",vec![original.into(),popup.into(),generated.into(),alias.into()]);
 assert!(replace_with_budget(large,&requests,4096,1_000_000,&mut ||Ok(())).is_err());
}

#[test]
fn annotation_replacement_repeated_edit_preserves_identity_without_dangling_references() {
 let (doc,page,original,popup,generated)=fixture();let popups=[receipt(page,1,popup)];
 let mut output=replace_owned(doc,&[Replacement{original:receipt(page,0,original),popups:&popups,generated}],&mut ||Ok(())).unwrap();
 for value in ["Second edit","Third edit"] {
  let mut next=output.get_dictionary(original).unwrap().clone();next.remove(b"Popup");next.remove(b"PrivateData");next.set("Contents",Object::string_literal(value));let generated=output.add_object(next);
  output.get_dictionary_mut(page).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().push(generated.into());
  output=replace_owned(output,&[Replacement{original:receipt(page,0,original),popups:&popups,generated}],&mut ||Ok(())).unwrap();
  assert_eq!(output.get_dictionary(original).unwrap().get(b"Contents").unwrap(),&Object::string_literal(value));
  assert_eq!(output.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap().len(),2);
  crate::composition_graph::validate_retained(&output,&mut ||Ok(())).unwrap();
 }
}
