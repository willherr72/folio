use super::*;
use lopdf::{dictionary,Dictionary,Object,Stream};
struct Fixture{doc:Document,pages:SourcePages,structure:StructureInfo,forms:FormInfo,widget:ObjectId,alias:ObjectId,form:ObjectId,element:ObjectId,ap:ObjectId}
fn fixture()->Fixture{
 let mut doc=Document::with_version("1.7");let tree=doc.new_object_id();let root=doc.new_object_id();
 let ap=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),20.into()]},vec![b' ';1024*1024]));
 let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary!{}});
 let widget=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","P"=>page,"Rect"=>vec![0.into(),0.into(),100.into(),20.into()],"FT"=>"Tx","T"=>Object::string_literal("answer"),"TU"=>Object::string_literal("Helpful name"),"V"=>Object::string_literal("saved answer"),"DV"=>Object::string_literal("default answer"),"Ff"=>4096,"DA"=>Object::string_literal("/F1 11 Tf 0.2 g"),"Q"=>1,"AP"=>dictionary!{"N"=>ap},"StructParent"=>0,"NM"=>Object::string_literal("original widget")});
 let alias=doc.add_object(Object::Reference(widget));doc.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(alias)]);
 let element=doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"Form","P"=>root,"Pg"=>page,"K"=>dictionary!{"Type"=>"OBJR","Obj"=>alias}});
 let parent=doc.add_object(dictionary!{"Nums"=>vec![0.into(),Object::Reference(element)]});doc.objects.insert(root,dictionary!{"Type"=>"StructTreeRoot","K"=>vec![Object::Reference(element)],"ParentTree"=>parent}.into());
 let second=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary!{}});
 doc.objects.insert(tree,dictionary!{"Type"=>"Pages","Count"=>2,"Kids"=>vec![Object::Reference(page),second.into()]}.into());
 let form=doc.add_object(dictionary!{"Fields"=>vec![Object::Reference(alias)],"CO"=>vec![Object::Reference(alias),alias.into()],"NeedAppearances"=>false});
 let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree,"AcroForm"=>form,"StructTreeRoot"=>root});doc.trailer.set("Root",catalog);
 let pages=crate::composition_pages::inspect(&doc,catalog,&mut ||Ok(())).unwrap();let structure=crate::composition_structure::inspect(&doc,catalog,&pages.pages,&mut ||Ok(())).unwrap();let forms=crate::composition_forms::inspect(&doc,&pages,&mut Budget::default(),&mut ||Ok(())).unwrap();
 Fixture{doc,pages,structure,forms,widget,alias,form,element,ap}
}
fn run(f:Fixture)->EngineResult<(Document,BTreeMap<ObjectId,ObjectId>)>{normalize(f.doc,&f.pages,&f.structure,&f.forms,&BTreeSet::from([f.pages.pages[0]]),&mut Budget::default(),&mut ||Ok(()))}
fn dict<'a>(doc:&'a Document,v:&'a Object)->&'a Dictionary{doc.dereference(v).unwrap().1.as_dict().unwrap()}
#[test]
fn composition_form_normalize_keeps_widget_tag_alias_and_appearance_identity(){
 let f=fixture();let (widget,alias,form,element,ap,catalog,page)=(f.widget,f.alias,f.form,f.element,f.ap,f.pages.catalog,f.pages.pages[0]);let pointer=f.doc.get_object(ap).unwrap().as_stream().unwrap().content.as_ptr();
 let (mut doc,map)=run(f).unwrap();let field=map[&widget];assert_ne!(field,widget);
 let w=doc.get_dictionary(widget).unwrap();assert!(!w.has(b"FT") && !w.has(b"T") && !w.has(b"V") && !w.has(b"DA"));assert_eq!(w.get(b"Parent").unwrap(),&Object::Reference(field));assert!(!w.has(b"Kids"));
 let d=doc.get_dictionary(field).unwrap();assert!(!d.has(b"Subtype") && !d.has(b"AP") && !d.has(b"StructParent"));assert_eq!(d.get(b"V").unwrap().as_str().unwrap(),b"saved answer");assert_eq!(d.get(b"DA").unwrap().as_str().unwrap(),b"/F1 11 Tf 0.2 g");assert_eq!(d.get(b"Kids").unwrap().as_array().unwrap(),&vec![Object::Reference(widget)]);
 assert_eq!(doc.get_dictionary(form).unwrap().get(b"Fields").unwrap().as_array().unwrap(),&vec![Object::Reference(field)]);assert_eq!(doc.get_dictionary(form).unwrap().get(b"CO").unwrap().as_array().unwrap(),&vec![Object::Reference(field),field.into()]);
 assert_eq!(doc.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap(),&vec![Object::Reference(alias)]);assert_eq!(dict(&doc,doc.get_dictionary(element).unwrap().get(b"K").unwrap()).get(b"Obj").unwrap(),&Object::Reference(alias));assert_eq!(doc.objects[&alias],Object::Reference(widget));assert_eq!(doc.get_object(ap).unwrap().as_stream().unwrap().content.as_ptr(),pointer);
 let mut saved=Vec::new();doc.save_to(&mut saved).unwrap();let doc=Document::load_mem(&saved).unwrap();let pages=crate::composition_pages::inspect(&doc,catalog,&mut ||Ok(())).unwrap();let forms=crate::composition_forms::inspect(&doc,&pages,&mut Budget::default(),&mut ||Ok(())).unwrap();assert_eq!(forms.nodes[&widget].owner,field);assert!(!forms.nodes[&widget].field);assert_eq!(forms.nodes[&field].full_name,"answer");assert_eq!(crate::composition_structure::inspect(&doc,catalog,&pages.pages,&mut ||Ok(())).unwrap().owners[&0].object,widget);
}
#[test]
fn composition_form_normalize_splits_events_and_rebinds_only_field_actions(){
 let mut f=fixture();let page=f.pages.pages[0];let widget=f.widget;let alias=f.alias;
 let reset=f.doc.add_object(dictionary!{"S"=>"ResetForm","Fields"=>vec![Object::Reference(alias)]});
 let hide=f.doc.add_object(dictionary!{"S"=>"Hide","T"=>vec![Object::Reference(alias),Object::string_literal("answer")],"Next"=>dictionary!{"S"=>"SubmitForm","F"=>Object::string_literal("https://example.invalid"),"Fields"=>vec![Object::Reference(alias)],"Flags"=>4}});
 let aa=f.doc.add_object(dictionary!{"V"=>reset,"Fo"=>hide});f.doc.get_dictionary_mut(widget).unwrap().set("AA",aa);f.doc.get_dictionary_mut(widget).unwrap().set("A",hide);f.doc.get_dictionary_mut(page).unwrap().set("AA",dictionary!{"O"=>dictionary!{"S"=>"ResetForm"}});
 let (doc,map)=run(f).unwrap();let field=map[&widget];let fa= dict(&doc,doc.get_dictionary(field).unwrap().get(b"AA").unwrap());let wa=dict(&doc,doc.get_dictionary(widget).unwrap().get(b"AA").unwrap());assert!(fa.has(b"V") && !fa.has(b"Fo"));assert!(wa.has(b"Fo") && !wa.has(b"V"));
 assert_eq!(dict(&doc,fa.get(b"V").unwrap()).get(b"Fields").unwrap().as_array().unwrap(),&vec![Object::Reference(field)]);
 let hide=dict(&doc,wa.get(b"Fo").unwrap());assert_eq!(hide.get(b"T").unwrap().as_array().unwrap(),&vec![Object::Reference(alias),Object::string_literal("answer")]);assert_eq!(dict(&doc,hide.get(b"Next").unwrap()).get(b"Fields").unwrap().as_array().unwrap(),&vec![Object::Reference(field)]);
 let pa=dict(&doc,doc.get_dictionary(page).unwrap().get(b"AA").unwrap());assert_eq!(dict(&doc,pa.get(b"O").unwrap()).get(b"Fields").unwrap().as_array().unwrap(),&vec![Object::Reference(field)]);
}
#[test]
fn composition_form_normalize_nested_field_and_direct_form_keep_parent_order(){
 let mut f=fixture();let parent=f.doc.add_object(dictionary!{"T"=>Object::string_literal("group"),"Kids"=>vec![Object::Reference(f.alias)]});f.doc.get_dictionary_mut(f.widget).unwrap().set("Parent",parent);f.doc.get_dictionary_mut(f.form).unwrap().set("Fields",vec![Object::Reference(parent)]);let direct=f.doc.get_dictionary(f.form).unwrap().clone();f.doc.get_dictionary_mut(f.pages.catalog).unwrap().set("AcroForm",direct);f.forms=crate::composition_forms::inspect(&f.doc,&f.pages,&mut Budget::default(),&mut ||Ok(())).unwrap();let widget=f.widget;let catalog=f.pages.catalog;
 let (doc,map)=run(f).unwrap();let field=map[&widget];assert_eq!(doc.get_dictionary(field).unwrap().get(b"Parent").unwrap(),&Object::Reference(parent));assert_eq!(doc.get_dictionary(parent).unwrap().get(b"Kids").unwrap().as_array().unwrap(),&vec![Object::Reference(field)]);assert_eq!(dict(&doc,doc.get_dictionary(catalog).unwrap().get(b"AcroForm").unwrap()).get(b"CO").unwrap().as_array().unwrap(),&vec![Object::Reference(field),field.into()]);
}
#[test]
fn composition_form_normalize_refuses_opaque_consumers_shared_containers_and_unknown_roles(){
 for mode in 0..6{let mut f=fixture();match mode{
  0=>f.doc.get_dictionary_mut(f.pages.catalog).unwrap().set("PrivateField",f.alias),
  1=>{let array=f.doc.add_object(vec![Object::Reference(f.alias)]);f.doc.get_dictionary_mut(f.form).unwrap().set("Fields",array);f.doc.get_dictionary_mut(f.pages.catalog).unwrap().set("OpaqueArray",array);},
  2=>f.doc.get_dictionary_mut(f.widget).unwrap().set("PrivateMeaning",Object::string_literal("unknown role")),
  3=>f.doc.get_dictionary_mut(f.widget).unwrap().set("AA",dictionary!{"Unknown"=>dictionary!{"S"=>"ResetForm"}}),
  4=>f.doc.get_dictionary_mut(f.widget).unwrap().set("AA",dictionary!{"V"=>dictionary!{"S"=>"JavaScript","JS"=>Object::string_literal("this.resetForm()")}}),
  _=>{let action=f.doc.add_object(dictionary!{"S"=>"ResetForm","Fields"=>vec![Object::Reference(f.alias)]});f.doc.get_dictionary_mut(f.widget).unwrap().set("A",action);f.doc.get_dictionary_mut(f.pages.catalog).unwrap().set("PrivateAction",action);},
 }assert!(run(f).is_err(),"mode {mode}");}
}
#[test]
fn composition_form_normalize_no_target_is_exactly_unchanged(){
 let f=fixture();let before=f.doc.objects.clone();let (doc,map)=normalize(f.doc,&f.pages,&f.structure,&f.forms,&BTreeSet::from([f.pages.pages[1]]),&mut Budget::default(),&mut ||Ok(())).unwrap();assert!(map.is_empty());assert_eq!(doc.objects,before);
}
#[test]
fn composition_form_normalize_null_extensions_and_unused_history_are_preserved(){
 let mut f=fixture();let null=f.doc.add_object(Object::Null);f.doc.get_dictionary_mut(f.widget).unwrap().set("AA",dictionary!{"Unknown"=>null});f.doc.get_dictionary_mut(f.widget).unwrap().set("PrivateNull",null);f.doc.get_dictionary_mut(f.widget).unwrap().set("Kids",Object::Null);let orphan=f.doc.add_object(dictionary!{"PrivateOldField"=>f.alias});let widget=f.widget;
 let (doc,_)=run(f).unwrap();assert_eq!(doc.get_dictionary(widget).unwrap().get(b"PrivateNull").unwrap(),&Object::Reference(null));assert_eq!(dict(&doc,doc.get_dictionary(widget).unwrap().get(b"AA").unwrap()).get(b"Unknown").unwrap(),&Object::Reference(null));assert!(doc.objects.contains_key(&orphan));
}
#[test]
fn composition_form_normalize_limits_cancellation_and_input_immutability(){
 let f=fixture();let before=f.doc.objects.clone();let repeated=BTreeSet::from([f.pages.pages[0]]);let mut budget=Budget::default();let initial=budget.metadata;let mut calls=0;
 normalize(f.doc.clone(),&f.pages,&f.structure,&f.forms,&repeated,&mut budget,&mut ||{calls+=1;Ok(())}).unwrap();let spent=initial-budget.metadata;
 assert!(normalize(f.doc.clone(),&f.pages,&f.structure,&f.forms,&repeated,&mut Budget{metadata:spent-1,..Default::default()},&mut ||Ok(())).is_err());
 assert!(normalize(f.doc.clone(),&f.pages,&f.structure,&f.forms,&repeated,&mut Budget{steps:99_999,..Default::default()},&mut ||Ok(())).is_err());
 for stop in [1,calls/3,2*calls/3,calls]{let mut at=0;assert!(normalize(f.doc.clone(),&f.pages,&f.structure,&f.forms,&repeated,&mut Budget::default(),&mut ||{at+=1;if at==stop{Err(EngineError::InvalidRequest("cancel".into()))}else{Ok(())}}).is_err());}
 assert_eq!(f.doc.objects,before);
 let mut overflow=f.doc.clone();overflow.max_id=u32::MAX;assert!(normalize(overflow,&f.pages,&f.structure,&f.forms,&repeated,&mut Budget::default(),&mut ||Ok(())).is_err());
}
#[test]
fn composition_form_normalize_implicit_submission_and_named_targets_keep_answers(){
 let mut f=fixture();let widget=f.widget;f.doc.get_dictionary_mut(widget).unwrap().set("A",dictionary!{"S"=>"SubmitForm","F"=>Object::string_literal("https://example.invalid"),"Next"=>dictionary!{"S"=>"ResetForm","Fields"=>vec![Object::string_literal("answer")]}});
 let (doc,map)=run(f).unwrap();let action=dict(&doc,doc.get_dictionary(widget).unwrap().get(b"A").unwrap());assert_eq!(action.get(b"Fields").unwrap().as_array().unwrap(),&vec![Object::Reference(map[&widget])]);assert_eq!(dict(&doc,action.get(b"Next").unwrap()).get(b"Fields").unwrap().as_array().unwrap(),&vec![Object::string_literal("answer")]);
}
