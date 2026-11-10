use super::*;
use lopdf::Stream;
use std::sync::Arc;
struct Handles{catalog:ObjectId,page:ObjectId,field:ObjectId,appearance:ObjectId,font:ObjectId}
fn source(doc:&mut Document,answer:&str,font:&str,lang:&str,tagged:bool)->(Source,Handles){
 let tree=doc.new_object_id();let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()]});
 doc.objects.insert(tree,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into());
 let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>font});
 let appearance=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),20.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}},format!("BT /F1 10 Tf 2 4 Td ({answer}) Tj ET").into_bytes()));
 let field=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>Object::string_literal("answer"),"V"=>Object::string_literal(answer),"DV"=>Object::string_literal("default"),"P"=>page,"Rect"=>vec![10.into(),10.into(),110.into(),30.into()],"AP"=>dictionary!{"N"=>appearance},"A"=>dictionary!{"S"=>"ResetForm"}});
 doc.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(field)]);
 let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree,"Lang"=>Object::string_literal(lang),"AcroForm"=>dictionary!{"Fields"=>vec![Object::Reference(field)],"DA"=>Object::string_literal("/F1 10 Tf 0 g"),"DR"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"CO"=>vec![Object::Reference(field)]}});
 if tagged{
  let root=doc.new_object_id();let element=doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"Form","P"=>root,"Pg"=>page,"K"=>dictionary!{"Type"=>"OBJR","Obj"=>field},"Alt"=>Object::string_literal("Answer field")});
  let parents=doc.add_object(dictionary!{"Nums"=>vec![0.into(),Object::Reference(element)]});
  doc.objects.insert(root,dictionary!{"Type"=>"StructTreeRoot","K"=>vec![Object::Reference(element)],"ParentTree"=>parents}.into());
  doc.get_dictionary_mut(field).unwrap().set("StructParent",0);doc.get_dictionary_mut(catalog).unwrap().set("StructTreeRoot",root);
 }
 (Source{catalog,version:"1.7".into(),info:None},Handles{catalog,page,field,appearance,font})
}
fn pair()->(Document,Vec<Source>,[Handles;2]){
 let mut doc=Document::with_version("1.7");let(a,ah)=source(&mut doc,"first answer","Helvetica","en-US",true);let(b,bh)=source(&mut doc,"second answer","Courier","en-US",true);(doc,vec![a,b],[ah,bh])
}
fn assemble(doc:Document,sources:Vec<Source>)->EngineResult<Document>{assemble_forms(doc,sources,OutlineIntent::Preserve,&mut ||Ok(()),Arc::new(||false))}
#[test]
fn composition_catalog_forms_join_original_widgets_names_defaults_actions_and_tags(){
 let(doc,sources,h)=pair();let ap=h.iter().map(|h|doc.get_object(h.appearance).unwrap().as_stream().unwrap().content.clone()).collect::<Vec<_>>();
 let pointers=h.iter().map(|h|doc.get_object(h.appearance).unwrap().as_stream().unwrap().content.as_ptr()).collect::<Vec<_>>();
 let mut out=assemble(doc,sources).unwrap();
 let form=out.dereference(out.catalog().unwrap().get(b"AcroForm").unwrap()).unwrap().1.as_dict().unwrap();
 assert_eq!(form.get(b"Fields").unwrap().as_array().unwrap(),&h.iter().map(|h|Object::Reference(h.field)).collect::<Vec<_>>());
 assert_eq!(form.get(b"CO").unwrap(),form.get(b"Fields").unwrap());
 for (i,h) in h.iter().enumerate(){
  let field=out.get_dictionary(h.field).unwrap();assert_eq!(field.get(b"V").unwrap().as_str().unwrap(),if i==0{b"first answer".as_slice()}else{b"second answer"});
  assert_eq!(field.get(b"T").unwrap().as_str().unwrap(),if i==0{b"answer".as_slice()}else{b"answer_2"});
  assert_eq!(field.get(b"DV").unwrap().as_str().unwrap(),b"default");
  let reset=out.dereference(field.get(b"A").unwrap()).unwrap().1.as_dict().unwrap();assert_eq!(reset.get(b"Fields").unwrap().as_array().unwrap(),&vec![Object::Reference(h.field)]);
  let stream=out.get_object(h.appearance).unwrap().as_stream().unwrap();assert_eq!(stream.content,ap[i]);assert_eq!(stream.content.as_ptr(),pointers[i]);
  assert_eq!(stream.dict.get(b"Resources").unwrap().as_dict().unwrap().get(b"Font").unwrap().as_dict().unwrap().get(b"F1").unwrap(),&Object::Reference(h.font));
 }
 assert_eq!(out.get_dictionary(h[1].field).unwrap().get(b"DA").unwrap().as_str().unwrap(),b"/Folio2_F1 10 Tf 0 g");
 let mut bytes=vec![];out.save_to(&mut bytes).unwrap();let reopened=Document::load_mem(&bytes).unwrap();let catalog=reopened.trailer.get(b"Root").unwrap().as_reference().unwrap();
 let pages=crate::composition_pages::inspect(&reopened,catalog,&mut ||Ok(())).unwrap();assert_eq!(pages.pages,vec![h[0].page,h[1].page]);
 let forms=crate::composition_forms::inspect(&reopened,&pages,&mut Default::default(),&mut ||Ok(())).unwrap();assert_eq!(forms.nodes.len(),2);assert_eq!(forms.nodes[&h[1].field].full_name,"answer_2");
 let structure=crate::composition_structure::inspect(&reopened,catalog,&pages.pages,&mut ||Ok(())).unwrap();
 for h in &h{assert!(structure.owners.values().any(|owner|owner.object==h.field));}
}
#[test]
fn composition_catalog_forms_legacy_guarded_wrapper_and_scripts_remain_refused(){
 let(doc,sources,_)=pair();assert!(assemble_navigation(doc,sources,OutlineIntent::Preserve,&mut ||Ok(()),Arc::new(||false)).is_err());
 let(mut doc,sources,h)=pair();doc.get_dictionary_mut(h[1].field).unwrap().set("AA",dictionary!{"V"=>dictionary!{"S"=>"JavaScript","JS"=>Object::string_literal("this.resetForm()")}});assert!(assemble(doc,sources).is_err());
}
#[test]
fn composition_catalog_forms_preserve_equal_language_encodings_and_refuse_different_defaults(){
 let(mut doc,sources,h)=pair();doc.get_dictionary_mut(h[1].catalog).unwrap().set("Lang",Object::string_literal(b"\xef\xbb\xbfEN-us".to_vec()));assert!(assemble(doc,sources).is_ok());
 let(mut doc,sources,h)=pair();doc.get_dictionary_mut(h[1].catalog).unwrap().set("Lang",Object::string_literal("fr-FR"));assert!(assemble(doc,sources).is_err(),"widget OBJR is not field-prose language ownership");
}
#[test]
fn composition_catalog_forms_handle_empty_and_absent_forms_deliberately(){
 for empty in [false,true]{let(mut doc,sources,h)=pair();doc.get_dictionary_mut(h[1].page).unwrap().remove(b"Annots");doc.get_dictionary_mut(h[1].catalog).unwrap().remove(b"StructTreeRoot");
  if empty{doc.get_dictionary_mut(h[1].catalog).unwrap().set("AcroForm",dictionary!{"Fields"=>Vec::<Object>::new()});}else{doc.get_dictionary_mut(h[1].catalog).unwrap().remove(b"AcroForm");}
  let out=assemble(doc,sources).unwrap();let form=out.dereference(out.catalog().unwrap().get(b"AcroForm").unwrap()).unwrap().1.as_dict().unwrap();assert_eq!(form.get(b"Fields").unwrap().as_array().unwrap(),&vec![Object::Reference(h[0].field)]);
 }
}
#[test]
fn composition_catalog_forms_cancellation_never_returns_a_partial_document(){
 let(doc,sources,_)=pair();let mut calls=0;assemble_forms(doc,sources,OutlineIntent::Preserve,&mut ||{calls+=1;Ok(())},Arc::new(||false)).unwrap();
 for stop in [1,calls/3,2*calls/3,calls]{let(doc,sources,_)=pair();let mut at=0;assert!(assemble_forms(doc,sources,OutlineIntent::Preserve,&mut ||{at+=1;if at==stop{Err(EngineError::InvalidRequest("cancelled".into()))}else{Ok(())}},Arc::new(||false)).is_err());}
 let(doc,sources,_)=pair();assert!(assemble_forms(doc,sources,OutlineIntent::Preserve,&mut ||Ok(()),Arc::new(||true)).is_err());
}
#[test]
fn composition_catalog_forms_observe_late_cancellation_token_without_callback_error(){
 use std::sync::atomic::{AtomicBool,Ordering};
 let(doc,sources,_)=pair();let mut calls=0;assemble_forms(doc,sources,OutlineIntent::Preserve,&mut ||{calls+=1;Ok(())},Arc::new(||false)).unwrap();
 let(doc,sources,_)=pair();let flag=Arc::new(AtomicBool::new(false));let observed=flag.clone();let mut at=0;
 let result=assemble_forms(doc,sources,OutlineIntent::Preserve,&mut ||{at+=1;if at==calls-1{flag.store(true,Ordering::SeqCst);}Ok(())},Arc::new(move||observed.load(Ordering::SeqCst)));
 assert!(flag.load(Ordering::SeqCst));assert!(result.is_err(),"a late token must be observed before returning a document");
}
#[test]
fn composition_catalog_forms_absent_form_path_preserves_existing_navigation_admission(){
 let(mut doc,sources,h)=pair();
 for h in &h{let catalog=doc.get_dictionary_mut(h.catalog).unwrap();catalog.remove(b"AcroForm");catalog.remove(b"StructTreeRoot");doc.get_dictionary_mut(h.page).unwrap().remove(b"Annots");}
 let action=dictionary!{"S"=>"JavaScript","JS"=>Object::string_literal("console.println('preserve only')")};
 doc.get_dictionary_mut(h[0].page).unwrap().set("AA",dictionary!{"O"=>action.clone()});
 let baseline_sources=sources.iter().map(|s|Source{catalog:s.catalog,version:s.version.clone(),info:s.info}).collect();
 assert!(assemble_navigation(doc.clone(),baseline_sources,OutlineIntent::Preserve,&mut ||Ok(()),Arc::new(||false)).is_ok());
 let output=assemble(doc,sources).unwrap();assert!(!output.catalog().unwrap().has(b"AcroForm"));
 let aa=output.dereference(output.get_dictionary(h[0].page).unwrap().get(b"AA").unwrap()).unwrap().1.as_dict().unwrap();
 assert_eq!(output.dereference(aa.get(b"O").unwrap()).unwrap().1.as_dict().unwrap(),&action);
}
#[test]
fn composition_catalog_forms_combined_duplicate_fields_keep_independent_names_fonts_and_answers(){
 let(mut doc,sources,h)=pair();for h in &h{doc.get_dictionary_mut(h.page).unwrap().set("Resources",dictionary!{});}
 let doc=assemble(doc,sources).unwrap();let order=vec![h[0].page,h[1].page,h[0].page,h[1].page];let (mut doc,pages)=crate::composition_occurrences::project_forms(doc,&order,&mut ||Ok(())).unwrap();
 let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();let page_info=crate::composition_pages::inspect(&doc,catalog,&mut ||Ok(())).unwrap();let forms=crate::composition_forms::inspect(&doc,&page_info,&mut Default::default(),&mut ||Ok(())).unwrap();assert_eq!(forms.roots.len(),2);
 for (i,h) in h.iter().enumerate(){let field=forms.nodes[&h.field].owner;assert_eq!(forms.nodes[&field].children.len(),2);assert_eq!(forms.nodes[&field].full_name,if i==0{"answer"}else{"answer_2"});let d=doc.get_dictionary(field).unwrap();assert_eq!(d.get(b"V").unwrap().as_str().unwrap(),if i==0{b"first answer".as_slice()}else{b"second answer"});assert_eq!(d.get(b"DA").unwrap().as_str().unwrap(),if i==0{b"/F1 10 Tf 0 g".as_slice()}else{b"/Folio2_F1 10 Tf 0 g"});
  for &widget in &forms.nodes[&field].children{let w=doc.get_dictionary(widget).unwrap();let a=doc.dereference(w.get(b"A").unwrap()).unwrap().1.as_dict().unwrap();assert_eq!(a.get(b"Fields").unwrap().as_array().unwrap(),&vec![Object::Reference(field)]);let ap=doc.dereference(w.get(b"AP").unwrap()).unwrap().1.as_dict().unwrap();assert_eq!(ap.get(b"N").unwrap(),&Object::Reference(h.appearance));}
 }
 assert_ne!(forms.nodes[&h[0].field].owner,forms.nodes[&h[1].field].owner);assert_eq!(crate::composition_structure::inspect(&doc,catalog,&pages,&mut ||Ok(())).unwrap().owners.len(),4);let mut bytes=Vec::new();doc.save_to(&mut bytes).unwrap();let saved=Document::load_mem(&bytes).unwrap();assert_eq!(crate::composition_forms::inspect(&saved,&page_info,&mut Default::default(),&mut ||Ok(())).unwrap(),forms);
}
