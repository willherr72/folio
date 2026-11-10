use super::*;
use lopdf::{dictionary,Stream};
struct Fixture {doc:Document,pages:SourcePages,form:ObjectId,group:ObjectId,field:ObjectId,widgets:[ObjectId;2],hidden:ObjectId,ap:ObjectId}
fn fixture()->Fixture{
 let mut doc=Document::with_version("1.7");let tree=doc.new_object_id();let group=doc.new_object_id();let field=doc.new_object_id();
 let ap=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),20.into()]},vec![b' ';1024*1024]));
 let mut pages=Vec::new();let mut widgets=Vec::new();
 for _ in 0..2{let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary!{}});let widget=doc.add_object(dictionary!{"Subtype"=>"Widget","Parent"=>field,"P"=>page,"Rect"=>vec![0.into(),0.into(),100.into(),20.into()],"AP"=>dictionary!{"N"=>ap}});doc.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(widget)]);pages.push(page);widgets.push(widget);}
 pages.push(doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary!{}}));
 doc.objects.insert(field,dictionary!{"T"=>Object::string_literal("answer"),"FT"=>"Tx","Parent"=>group,"V"=>Object::string_literal("shared answer"),"DV"=>Object::string_literal("default"),"Kids"=>widgets.iter().copied().map(Object::Reference).collect::<Vec<_>>()}.into());
 doc.objects.insert(group,dictionary!{"T"=>Object::string_literal("group"),"Kids"=>vec![Object::Reference(field)]}.into());
 let hidden=doc.add_object(dictionary!{"T"=>Object::string_literal("hidden"),"FT"=>"Tx","V"=>Object::string_literal("nonvisual answer")});
 let form=doc.add_object(dictionary!{"Fields"=>vec![Object::Reference(group),hidden.into()],"CO"=>vec![Object::Reference(hidden),field.into(),hidden.into()],"DA"=>Object::string_literal("/F1 10 Tf"),"NeedAppearances"=>false});
 doc.objects.insert(tree,dictionary!{"Type"=>"Pages","Count"=>3,"Kids"=>pages.iter().copied().map(Object::Reference).collect::<Vec<_>>()}.into());
 let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree,"AcroForm"=>form});doc.trailer.set("Root",catalog);
 let pages=crate::composition_pages::inspect(&doc,catalog,&mut ||Ok(())).unwrap();Fixture{doc,pages,form,group,field,widgets:[widgets[0],widgets[1]],hidden,ap}
}
fn plan(f:&Fixture,keep:&[usize])->Selection{
 let mut budget=Budget::default();let forms=crate::composition_forms::inspect(&f.doc,&f.pages,&mut budget,&mut ||Ok(())).unwrap();
 prepare(&f.doc,&f.pages,&forms,&keep.iter().map(|i|f.pages.pages[*i]).collect(),&mut budget,&mut ||Ok(())).unwrap()
}
#[test]
fn composition_form_selection_keeps_shared_answers_and_original_nonvisual_fields(){
 let f=fixture();let before=f.doc.objects.clone();let selection=plan(&f,&[1,2]);
 assert_eq!(selection.removed,BTreeSet::from([f.widgets[0]]));assert!(selection.retained.contains(&f.hidden));assert!(selection.retained.contains(&f.field));
 assert_eq!(selection.patches[&(f.field,b"Kids".to_vec())],Object::Array(vec![Object::Reference(f.widgets[1])]));
 assert!(!selection.patches.contains_key(&(f.form,b"Fields".to_vec())));assert!(!selection.patches.contains_key(&(f.form,b"CO".to_vec())));assert_eq!(f.doc.objects,before);
 let pointer=f.doc.get_object(f.ap).unwrap().as_stream().unwrap().content.as_ptr();let doc=apply(f.doc,selection,&mut Budget::default(),&mut ||Ok(())).unwrap();
 assert_eq!(doc.get_dictionary(f.field).unwrap().get(b"V").unwrap().as_str().unwrap(),b"shared answer");assert_eq!(doc.get_dictionary(f.field).unwrap().get(b"DV").unwrap().as_str().unwrap(),b"default");assert_eq!(doc.get_object(f.ap).unwrap().as_stream().unwrap().content.as_ptr(),pointer);
}
#[test]
fn composition_form_selection_prunes_visual_branches_and_only_their_calculation_entries(){
 let f=fixture();let selection=plan(&f,&[2]);assert_eq!(selection.removed,BTreeSet::from([f.group,f.field,f.widgets[0],f.widgets[1]]));assert_eq!(selection.retained,BTreeSet::from([f.hidden]));
 assert_eq!(selection.patches[&(f.form,b"Fields".to_vec())],Object::Array(vec![Object::Reference(f.hidden)]));assert_eq!(selection.patches[&(f.form,b"CO".to_vec())],Object::Array(vec![Object::Reference(f.hidden),f.hidden.into()]));
 assert!(!selection.patches.keys().any(|(id,_)|selection.removed.contains(id)),"removed subtrees need no mutation");
}
#[test]
fn composition_form_selection_retains_ancestors_of_nonvisual_descendants(){
 let mut f=fixture();f.doc.get_dictionary_mut(f.hidden).unwrap().set("Parent",f.group);f.doc.get_dictionary_mut(f.group).unwrap().set("Kids",vec![Object::Reference(f.field),f.hidden.into()]);f.doc.get_dictionary_mut(f.form).unwrap().set("Fields",vec![Object::Reference(f.group)]);
 let selection=plan(&f,&[2]);assert_eq!(selection.retained,BTreeSet::from([f.group,f.hidden]));assert_eq!(selection.patches[&(f.group,b"Kids".to_vec())],Object::Array(vec![Object::Reference(f.hidden)]));assert!(!selection.patches.contains_key(&(f.form,b"Fields".to_vec())));
}
#[test]
fn composition_form_selection_keeps_retained_alias_edges_and_direct_catalog_forms(){
 let mut f=fixture();let alias=f.doc.add_object(Object::Reference(f.widgets[1]));f.doc.get_dictionary_mut(f.field).unwrap().set("Kids",vec![Object::Reference(f.widgets[0]),alias.into()]);
 let selection=plan(&f,&[1]);assert_eq!(selection.patches[&(f.field,b"Kids".to_vec())],Object::Array(vec![Object::Reference(alias)]));
 let form=f.doc.get_dictionary(f.form).unwrap().clone();f.doc.get_dictionary_mut(f.pages.catalog).unwrap().set("AcroForm",form);let selection=plan(&f,&[2]);assert!(selection.patches.contains_key(&(f.pages.catalog,b"AcroForm".to_vec())));
 let doc=apply(f.doc,selection,&mut Budget::default(),&mut ||Ok(())).unwrap();let form=doc.get_dictionary(f.pages.catalog).unwrap().get(b"AcroForm").unwrap().as_dict().unwrap();assert_eq!(form.get(b"Fields").unwrap().as_array().unwrap(),&vec![Object::Reference(f.hidden)]);assert_eq!(form.get(b"DA").unwrap().as_str().unwrap(),b"/F1 10 Tf");
}
#[test]
fn composition_form_selection_merged_widget_is_removed_without_creating_hidden_answer(){
 let mut f=fixture();let merged=f.doc.add_object(dictionary!{"Subtype"=>"Widget","T"=>Object::string_literal("merged"),"FT"=>"Btn","P"=>f.pages.pages[0],"V"=>"Yes"});
 f.doc.get_dictionary_mut(f.pages.pages[0]).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().push(merged.into());f.doc.get_dictionary_mut(f.form).unwrap().get_mut(b"Fields").unwrap().as_array_mut().unwrap().push(merged.into());
 let selected=plan(&f,&[1]);assert!(selected.removed.contains(&merged));assert!(selected.retained.contains(&f.field));assert!(!selected.patches.keys().any(|(id,_)|*id==merged));
}
#[test]
fn composition_form_selection_identity_is_read_only_and_limits_are_cumulative(){
 let f=fixture();let selected=plan(&f,&[2,1,0]);assert!(selected.removed.is_empty() && selected.patches.is_empty());
 let mut budget=Budget::default();let forms=crate::composition_forms::inspect(&f.doc,&f.pages,&mut budget,&mut ||Ok(())).unwrap();let keep=BTreeSet::from([f.pages.pages[1]]);let initial=budget.metadata;let mut calls=0;
 prepare(&f.doc,&f.pages,&forms,&keep,&mut budget,&mut ||{calls+=1;Ok(())}).unwrap();let spent=initial-budget.metadata;
 assert!(prepare(&f.doc,&f.pages,&forms,&keep,&mut Budget{metadata:spent-1,..Default::default()},&mut ||Ok(())).is_err());
 let before=f.doc.objects.clone();for stop in [1,calls/2,calls]{let mut at=0;assert!(prepare(&f.doc,&f.pages,&forms,&keep,&mut Budget::default(),&mut ||{at+=1;if at==stop{Err(EngineError::InvalidRequest("cancel".into()))}else{Ok(())}}).is_err());assert_eq!(f.doc.objects,before);}
 assert!(prepare(&f.doc,&f.pages,&forms,&BTreeSet::from([(u32::MAX,0)]),&mut Budget::default(),&mut ||Ok(())).is_err());
}
fn tag_widgets(f:&mut Fixture){
 let root=f.doc.new_object_id();let mut nums=Vec::new();let mut kids=Vec::new();
 for (i,&widget) in f.widgets.iter().enumerate(){let element=f.doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"Form","P"=>root,"Pg"=>f.pages.pages[i],"K"=>dictionary!{"Type"=>"OBJR","Obj"=>widget}});f.doc.get_dictionary_mut(widget).unwrap().set("StructParent",i as i64);nums.extend([Object::Integer(i as i64),element.into()]);kids.push(Object::Reference(element));}
 let parent=f.doc.add_object(dictionary!{"Nums"=>nums});f.doc.objects.insert(root,dictionary!{"Type"=>"StructTreeRoot","K"=>kids,"ParentTree"=>parent}.into());f.doc.get_dictionary_mut(f.pages.catalog).unwrap().set("StructTreeRoot",root);
}
fn projected(f:Fixture,indices:&[usize])->EngineResult<Document>{let pages=indices.iter().map(|i|f.pages.pages[*i]).collect::<Vec<_>>();crate::composition_projection::project_forms(f.doc,&pages,&mut ||Ok(()))}
#[test]
fn composition_form_selection_graph_preserves_shared_field_tagged_widget_and_hidden_answer(){
 let mut f=fixture();tag_widgets(&mut f);let (field,hidden,removed,kept,ap)=(f.field,f.hidden,f.widgets[0],f.widgets[1],f.ap);let bytes=f.doc.get_object(ap).unwrap().as_stream().unwrap().content.clone();let mut out=projected(f,&[1,2]).unwrap();
 assert!(!out.objects.contains_key(&removed));assert_eq!(out.get_dictionary(field).unwrap().get(b"Kids").unwrap().as_array().unwrap(),&vec![Object::Reference(kept)]);assert_eq!(out.get_dictionary(field).unwrap().get(b"V").unwrap().as_str().unwrap(),b"shared answer");assert_eq!(out.get_dictionary(hidden).unwrap().get(b"V").unwrap().as_str().unwrap(),b"nonvisual answer");assert_eq!(out.get_object(ap).unwrap().as_stream().unwrap().content,bytes);
 let mut saved=Vec::new();out.save_to(&mut saved).unwrap();let doc=Document::load_mem(&saved).unwrap();let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();let pages=crate::composition_pages::inspect(&doc,catalog,&mut ||Ok(())).unwrap();let forms=crate::composition_forms::inspect(&doc,&pages,&mut Budget::default(),&mut ||Ok(())).unwrap();assert_eq!(forms.nodes.len(),4);
 let structure=crate::composition_structure::inspect(&doc,catalog,&pages.pages,&mut ||Ok(())).unwrap();assert_eq!(structure.owners.len(),1);assert!(structure.owners.values().any(|owner|owner.object==kept));
}
#[test]
fn composition_form_selection_graph_removes_all_visual_nodes_but_keeps_nonvisual_calculations(){
 let mut f=fixture();tag_widgets(&mut f);let hidden=f.hidden;let old=[f.group,f.field,f.widgets[0],f.widgets[1]];let doc=projected(f,&[2]).unwrap();for id in old{assert!(!doc.objects.contains_key(&id));}
 let form=doc.dereference(doc.catalog().unwrap().get(b"AcroForm").unwrap()).unwrap().1.as_dict().unwrap();assert_eq!(form.get(b"Fields").unwrap().as_array().unwrap(),&vec![Object::Reference(hidden)]);assert_eq!(form.get(b"CO").unwrap().as_array().unwrap(),&vec![Object::Reference(hidden),hidden.into()]);
}
fn add_merged(f:&mut Fixture)->ObjectId{
 let merged=f.doc.add_object(dictionary!{"Subtype"=>"Widget","T"=>Object::string_literal("merged"),"FT"=>"Btn","P"=>f.pages.pages[0],"V"=>"Yes"});f.doc.get_dictionary_mut(f.pages.pages[0]).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().push(merged.into());f.doc.get_dictionary_mut(f.form).unwrap().get_mut(b"Fields").unwrap().as_array_mut().unwrap().push(merged.into());merged
}
#[test]
fn composition_form_selection_graph_prunes_typed_actions_and_preserves_next_chains(){
 let mut f=fixture();let merged=add_merged(&mut f);let page=f.pages.pages[1];let widget=f.widgets[1];let field=f.field;let hidden=f.hidden;
 let next=f.doc.add_object(dictionary!{"S"=>"URI","URI"=>Object::string_literal("https://example.invalid/next")});
 f.doc.get_dictionary_mut(page).unwrap().set("AA",dictionary!{"O"=>dictionary!{"S"=>"ResetForm","Fields"=>vec![Object::Reference(merged),f.field.into()],"Next"=>next}});
 f.doc.get_dictionary_mut(widget).unwrap().set("AA",dictionary!{"Fo"=>dictionary!{"S"=>"Hide","T"=>vec![Object::Reference(f.widgets[0]),widget.into(),merged.into()]}});
 f.doc.get_dictionary_mut(hidden).unwrap().set("AA",dictionary!{"V"=>dictionary!{"S"=>"SubmitForm","F"=>Object::string_literal("https://example.invalid"),"Fields"=>vec![Object::string_literal("merged"),Object::Reference(hidden)]}});
 let doc=projected(f,&[1,2]).unwrap();assert!(!doc.objects.contains_key(&merged));
 let get_action=|id,trigger:&[u8]|{let aa=doc.dereference(doc.get_dictionary(id).unwrap().get(b"AA").unwrap()).unwrap().1.as_dict().unwrap();doc.dereference(aa.get(trigger).unwrap()).unwrap().1.as_dict().unwrap()};
 let reset=get_action(page,b"O");assert_eq!(reset.get(b"Fields").unwrap().as_array().unwrap(),&vec![Object::Reference(field)]);assert_eq!(reset.get(b"Next").unwrap(),&Object::Reference(next));
 assert_eq!(get_action(widget,b"Fo").get(b"T").unwrap().as_array().unwrap(),&vec![Object::Reference(widget)]);assert_eq!(get_action(hidden,b"V").get(b"Fields").unwrap().as_array().unwrap(),&vec![Object::Reference(hidden)]);
}
#[test]
fn composition_form_selection_graph_refuses_opaque_removed_consumers_and_surviving_scripts(){
 for script in [false,true]{let mut f=fixture();let merged=add_merged(&mut f);
  if script{f.doc.get_dictionary_mut(f.hidden).unwrap().set("AA",dictionary!{"V"=>dictionary!{"S"=>"JavaScript","JS"=>Object::string_literal("this.resetForm()")}});}else{f.doc.get_dictionary_mut(f.hidden).unwrap().set("PrivateTarget",merged);}
  assert!(projected(f,&[1,2]).is_err());
 }
}
#[test]
fn composition_form_selection_graph_supports_direct_forms_reordering_and_cancellation(){
 let mut f=fixture();let direct=f.doc.get_dictionary(f.form).unwrap().clone();f.doc.get_dictionary_mut(f.pages.catalog).unwrap().set("AcroForm",direct);let expected=vec![f.pages.pages[2],f.pages.pages[1]];let mut calls=0;
 let doc=crate::composition_projection::project_forms(f.doc.clone(),&expected,&mut ||{calls+=1;Ok(())}).unwrap();assert_eq!(doc.get_pages().values().copied().collect::<Vec<_>>(),expected);
 let before=f.doc.objects.clone();for stop in [1,calls/3,2*calls/3,calls]{let mut at=0;assert!(crate::composition_projection::project_forms(f.doc.clone(),&expected,&mut ||{at+=1;if at==stop{Err(EngineError::InvalidRequest("cancel".into()))}else{Ok(())}}).is_err());assert_eq!(f.doc.objects,before);}
 let unchanged=crate::composition_projection::project_forms(f.doc.clone(),&f.pages.pages,&mut ||Ok(())).unwrap();assert_eq!(unchanged.objects,before);
}
#[test]
fn composition_form_selection_graph_keeps_indirect_shared_actions_after_final_validation(){
 let mut f=fixture();let merged=add_merged(&mut f);let hidden=f.hidden;let widget=f.widgets[1];
 let action=f.doc.add_object(dictionary!{"S"=>"ResetForm","Fields"=>vec![Object::Reference(merged),hidden.into()]});
 let aa=f.doc.add_object(dictionary!{"V"=>action});f.doc.get_dictionary_mut(hidden).unwrap().set("AA",aa);f.doc.get_dictionary_mut(widget).unwrap().set("A",action);
 let mut out=projected(f,&[1,2]).unwrap();let mut saved=Vec::new();out.save_to(&mut saved).unwrap();let doc=Document::load_mem(&saved).unwrap();
 let aa=doc.dereference(doc.get_dictionary(hidden).unwrap().get(b"AA").unwrap()).unwrap().1.as_dict().unwrap();let action=aa.get(b"V").unwrap();assert_eq!(action,doc.get_dictionary(widget).unwrap().get(b"A").unwrap());let action=doc.dereference(action).unwrap().1.as_dict().unwrap();assert_eq!(action.get(b"Fields").unwrap().as_array().unwrap(),&vec![Object::Reference(hidden)]);
}
fn indexed_buttons(inherited:bool)->Fixture{
 let mut f=fixture();let off=f.doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),20.into()]},b"q Q".to_vec()));
 f.doc.get_dictionary_mut(f.field).unwrap().set("FT","Btn");f.doc.get_dictionary_mut(f.field).unwrap().set("Ff",32768);
 let owner=if inherited{f.group}else{f.field};f.doc.get_dictionary_mut(owner).unwrap().set("Opt",vec![Object::string_literal("first"),lopdf::text_string("second é")]);
 f.doc.get_dictionary_mut(f.field).unwrap().set("V","1");f.doc.get_dictionary_mut(f.field).unwrap().set("DV","1");
 for (i,&widget) in f.widgets.iter().enumerate(){let mut states=dictionary!{"Off"=>off};states.set(i.to_string(),f.ap);let states=f.doc.add_object(states);let appearance=f.doc.add_object(dictionary!{"N"=>states,"R"=>states,"D"=>off});f.doc.get_dictionary_mut(widget).unwrap().set("AP",appearance);f.doc.get_dictionary_mut(widget).unwrap().set("AS",if i==1{"1"}else{"Off"});}
 f
}
#[test]
fn composition_form_selection_button_order_preserves_exports_defaults_and_exact_appearances(){
 for inherited in [false,true]{let f=indexed_buttons(inherited);let field=f.field;let widget=f.widgets[1];let ap=f.ap;let expected=f.doc.get_object(ap).unwrap().as_stream().unwrap().content.clone();let doc=projected(f,&[1,2]).unwrap();let field=doc.get_dictionary(field).unwrap();
  let opt=doc.dereference(field.get(b"Opt").unwrap()).unwrap().1.as_array().unwrap();assert_eq!(opt.len(),1);assert_eq!(crate::form_properties::text(&doc,&opt[0],1024).unwrap(),"second é");assert_eq!(field.get(b"V").unwrap().as_name().unwrap(),b"0");assert_eq!(field.get(b"DV").unwrap().as_name().unwrap(),b"0");
  let widget=doc.get_dictionary(widget).unwrap();assert_eq!(widget.get(b"AS").unwrap().as_name().unwrap(),b"0");let appearance=doc.dereference(widget.get(b"AP").unwrap()).unwrap().1.as_dict().unwrap();
  for key in [b"N".as_slice(),b"R"]{let states=doc.dereference(appearance.get(key).unwrap()).unwrap().1.as_dict().unwrap();assert_eq!(states.get(b"0").unwrap(),&Object::Reference(ap));assert!(!states.has(b"1"));assert!(states.has(b"Off"));}
  assert_eq!(doc.get_object(ap).unwrap().as_stream().unwrap().content,expected);
 }
}
#[test]
fn composition_form_selection_button_order_keeps_choice_options_independent_of_widgets(){
 let mut f=fixture();f.doc.get_dictionary_mut(f.field).unwrap().set("FT","Ch");let options=vec![Object::string_literal("A"),Object::string_literal("B")];f.doc.get_dictionary_mut(f.field).unwrap().set("Opt",options.clone());let field=f.field;
 let doc=projected(f,&[1,2]).unwrap();assert_eq!(doc.get_dictionary(field).unwrap().get(b"Opt").unwrap().as_array().unwrap(),&options);
}
#[test]
fn composition_form_selection_button_order_refuses_unrepresentable_removed_answers_and_malformed_states(){
 for malformed in [false,true]{let mut f=indexed_buttons(false);if malformed{f.doc.get_dictionary_mut(f.field).unwrap().set("Opt",vec![Object::string_literal("wrong length")]);}else{f.doc.get_dictionary_mut(f.field).unwrap().set("V","0");f.doc.get_dictionary_mut(f.widgets[0]).unwrap().set("AS","0");f.doc.get_dictionary_mut(f.widgets[1]).unwrap().set("AS","Off");}
 assert!(projected(f,&[1,2]).is_err());}
}
#[test]
fn composition_form_selection_button_order_keeps_off_null_and_nonindexed_states(){
 for indexed in [false,true]{let mut f=indexed_buttons(false);if !indexed{f.doc.get_dictionary_mut(f.field).unwrap().remove(b"Opt");}f.doc.get_dictionary_mut(f.field).unwrap().set("V","Off");f.doc.get_dictionary_mut(f.field).unwrap().set("DV",Object::Null);f.doc.get_dictionary_mut(f.widgets[1]).unwrap().set("AS","Off");let field=f.field;
 let doc=projected(f,&[1,2]).unwrap();let field=doc.get_dictionary(field).unwrap();assert_eq!(field.get(b"V").unwrap().as_name().unwrap(),b"Off");assert_eq!(field.get(b"DV").unwrap(),&Object::Null);
 }
}
#[test]
fn composition_form_selection_button_order_preserves_equivalent_surviving_checked_exports(){
 let mut f=indexed_buttons(false);f.doc.get_dictionary_mut(f.field).unwrap().set("Opt",vec![lopdf::text_string("same é"),Object::string_literal(b"\xef\xbb\xbfsame \xc3\xa9".to_vec())]);
 f.doc.get_dictionary_mut(f.field).unwrap().set("Ff",32768|(1<<25));f.doc.get_dictionary_mut(f.field).unwrap().set("V","0");f.doc.get_dictionary_mut(f.field).unwrap().set("DV","0");f.doc.get_dictionary_mut(f.widgets[0]).unwrap().set("AS","0");let field=f.field;let widget=f.widgets[1];
 let doc=projected(f,&[1,2]).unwrap();let field=doc.get_dictionary(field).unwrap();assert_eq!(field.get(b"V").unwrap().as_name().unwrap(),b"0");assert_eq!(field.get(b"DV").unwrap().as_name().unwrap(),b"0");assert_eq!(field.get(b"Ff").unwrap().as_i64().unwrap(),32768|(1<<25));assert_eq!(doc.get_dictionary(widget).unwrap().get(b"AS").unwrap().as_name().unwrap(),b"0");
 assert_eq!(crate::form_properties::text(&doc,&field.get(b"Opt").unwrap().as_array().unwrap()[0],1024).unwrap(),"same é");
}
#[test]
fn composition_form_selection_button_order_stages_copied_control_slots_with_shared_budget(){
 let f=indexed_buttons(false);let forms=crate::composition_forms::inspect(&f.doc,&f.pages,&mut Budget::default(),&mut ||Ok(())).unwrap();let copy=(f.doc.max_id+1,0);let controls=[(f.widgets[0],f.widgets[0]),(f.widgets[1],f.widgets[1]),(f.widgets[1],copy)];let before=f.doc.objects.clone();
 let mut budget=Budget::default();let initial=budget.metadata;let mut calls=0;let patches=crate::composition_button_order::remap(&f.doc,&forms,f.field,f.field,&controls,&mut budget,&mut ||{calls+=1;Ok(())}).unwrap();
 let options=patches[&(f.field,b"Opt".to_vec())].as_array().unwrap();assert_eq!(options.len(),3);assert_eq!(options[1],options[2]);assert!(!patches.contains_key(&(f.field,b"V".to_vec())));assert_eq!(patches[&(copy,b"AS".to_vec())].as_name().unwrap(),b"2");let normal=patches[&(copy,b"AP".to_vec())].as_dict().unwrap().get(b"N").unwrap().as_dict().unwrap();assert_eq!(normal.get(b"2").unwrap(),&Object::Reference(f.ap));assert!(!normal.has(b"1"));assert_eq!(f.doc.objects,before);
 let spent=initial-budget.metadata;assert!(crate::composition_button_order::remap(&f.doc,&forms,f.field,f.field,&controls,&mut Budget{metadata:spent-1,..Default::default()},&mut ||Ok(())).is_err());
 for stop in [1,calls/2,calls]{let mut at=0;assert!(crate::composition_button_order::remap(&f.doc,&forms,f.field,f.field,&controls,&mut Budget::default(),&mut ||{at+=1;if at==stop{Err(EngineError::InvalidRequest("cancel".into()))}else{Ok(())}}).is_err());assert_eq!(f.doc.objects,before);}
}
#[test]
fn composition_form_selection_button_order_refuses_mismatched_state_names_and_invalid_export_types(){
 for case in 0..4{let mut f=indexed_buttons(false);match case{
  0=>f.doc.get_dictionary_mut(f.widgets[1]).unwrap().set("AS","Other"),
  1=>f.doc.get_dictionary_mut(f.widgets[1]).unwrap().set("AP",dictionary!{"N"=>dictionary!{"Off"=>f.ap,"Yes"=>f.ap}}),
  2=>f.doc.get_dictionary_mut(f.field).unwrap().set("Opt",vec![Object::Integer(1),Object::string_literal("second")]),
  _=>f.doc.get_dictionary_mut(f.field).unwrap().set("DV","99"),
 };assert!(projected(f,&[1,2]).is_err(),"case {case}");}
}

fn named_buttons(same:bool)->Fixture{
 let mut f=indexed_buttons(false);let d=f.doc.get_dictionary_mut(f.field).unwrap();d.remove(b"Opt");d.set("V","B");d.set("DV","B");d.set("Ff",32768|33554432);
 for (i,&widget) in f.widgets.iter().enumerate(){let old=i.to_string();let name=if same || i==1{"B"}else{"A"};let ap=f.doc.get_dictionary(widget).unwrap().get(b"AP").unwrap().as_reference().unwrap();let states=f.doc.get_dictionary(ap).unwrap().get(b"N").unwrap().as_reference().unwrap();let states=f.doc.get_dictionary_mut(states).unwrap();let on=states.remove(old.as_bytes()).unwrap();states.set(name,on);f.doc.get_dictionary_mut(widget).unwrap().set("AS",if same || i==1{"B"}else{"Off"});}f
}
#[test]
fn composition_form_selection_named_buttons_preserve_equivalent_checked_controls_and_defaults(){
 for same in [false,true]{let f=named_buttons(same);let field=f.field;let ap=f.ap;let widget=f.widgets[1];let pointer=f.doc.get_object(ap).unwrap().as_stream().unwrap().content.as_ptr();let doc=projected(f,&[1,2]).unwrap();assert_eq!(doc.get_dictionary(field).unwrap().get(b"V").unwrap().as_name().unwrap(),b"B");assert_eq!(doc.get_dictionary(field).unwrap().get(b"DV").unwrap().as_name().unwrap(),b"B");assert_eq!(doc.get_dictionary(widget).unwrap().get(b"AS").unwrap().as_name().unwrap(),b"B");assert_eq!(doc.get_object(ap).unwrap().as_stream().unwrap().content.as_ptr(),pointer);}
}
#[test]
fn composition_form_selection_named_buttons_refuse_stranded_answers_defaults_and_checked_states(){
 for case in 0..4{let mut f=named_buttons(false);match case{
  0=>{f.doc.get_dictionary_mut(f.field).unwrap().set("V","A");f.doc.get_dictionary_mut(f.widgets[0]).unwrap().set("AS","A");f.doc.get_dictionary_mut(f.widgets[1]).unwrap().set("AS","Off");},
  1=>{f.doc.get_dictionary_mut(f.field).unwrap().set("DV","A");},
  2=>{f.doc.get_dictionary_mut(f.field).unwrap().set("V",Object::Null);f.doc.get_dictionary_mut(f.field).unwrap().set("DV",Object::Null);f.doc.get_dictionary_mut(f.widgets[0]).unwrap().set("AS","A");},
  _=>{f.doc.get_dictionary_mut(f.widgets[1]).unwrap().set("AS","Missing");},
 }assert!(projected(f,&[1,2]).is_err(),"case {case}");}
}
#[test]
fn composition_form_selection_named_button_budget_and_cancellation_are_read_only(){
 let f=named_buttons(false);let forms=crate::composition_forms::inspect(&f.doc,&f.pages,&mut Budget::default(),&mut ||Ok(())).unwrap();let controls=[(f.widgets[1],f.widgets[1])];let before=f.doc.objects.clone();let mut b=Budget::default();let initial=b.metadata;let mut calls=0;
 assert!(crate::composition_button_order::remap(&f.doc,&forms,f.field,f.field,&controls,&mut b,&mut ||{calls+=1;Ok(())}).unwrap().is_empty());let spent=initial-b.metadata;
 assert!(crate::composition_button_order::remap(&f.doc,&forms,f.field,f.field,&controls,&mut Budget{metadata:spent-1,..Default::default()},&mut ||Ok(())).is_err());
 for stop in [1,calls/2,calls]{let mut at=0;assert!(crate::composition_button_order::remap(&f.doc,&forms,f.field,f.field,&controls,&mut Budget::default(),&mut ||{at+=1;if at==stop{Err(EngineError::InvalidRequest("cancel".into()))}else{Ok(())}}).is_err());}assert_eq!(f.doc.objects,before);
}
