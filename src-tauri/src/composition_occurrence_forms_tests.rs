use super::*;
use crate::composition_form_budget::Budget;
use lopdf::Stream;
struct Fixture{doc:Document,pages:Vec<ObjectId>,field:ObjectId,widgets:Vec<ObjectId>,hidden:ObjectId,form:ObjectId,ap:ObjectId}
fn fixture(merged:bool)->Fixture{
 let mut doc=Document::with_version("1.7");let tree=doc.new_object_id();let field=doc.new_object_id();let tag_root=doc.new_object_id();
 let ap=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),20.into()],"Resources"=>dictionary!{}},vec![b' ';1024*1024]));
 let mut pages=Vec::new();let mut widgets=Vec::new();let mut tags=Vec::new();let mut parents=Vec::new();
 for i in 0..2{
  let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary!{}});pages.push(page);
  if merged && i==1{continue;}
  let widget=if merged{field}else{doc.new_object_id()};let mut dict=dictionary!{"Type"=>"Annot","Subtype"=>"Widget","P"=>page,"Rect"=>vec![10.into(),(20+i*30).into(),110.into(),(40+i*30).into()],"AP"=>dictionary!{"N"=>ap},"StructParent"=>i};
  if merged{dict.set("FT","Tx");dict.set("T",Object::string_literal("answer"));dict.set("V",Object::string_literal("saved"));dict.set("DV",Object::string_literal("default"));dict.set("DA",Object::string_literal("/F 10 Tf"));}else{dict.set("Parent",field);}
  doc.objects.insert(widget,dict.into());doc.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(widget)]);widgets.push(widget);
  let tag=doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"Form","P"=>tag_root,"Pg"=>page,"K"=>dictionary!{"Type"=>"OBJR","Obj"=>widget}});tags.push(Object::Reference(tag));parents.extend([Object::Integer(i),Object::Reference(tag)]);
 }
 let hidden=doc.add_object(dictionary!{"FT"=>"Tx","T"=>Object::string_literal("hidden"),"V"=>Object::string_literal("nonvisual")});
 if !merged{doc.objects.insert(field,dictionary!{"FT"=>"Tx","T"=>Object::string_literal("answer"),"V"=>Object::string_literal("saved"),"DV"=>Object::string_literal("default"),"DA"=>Object::string_literal("/F 10 Tf"),"Kids"=>widgets.iter().copied().map(Object::Reference).collect::<Vec<_>>()}.into());}
 let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});let form=doc.add_object(dictionary!{"Fields"=>vec![Object::Reference(field),hidden.into()],"CO"=>vec![Object::Reference(field),hidden.into()],"DR"=>dictionary!{"Font"=>dictionary!{"F"=>font}}});
 let parent=doc.add_object(dictionary!{"Nums"=>parents});doc.objects.insert(tag_root,dictionary!{"Type"=>"StructTreeRoot","K"=>tags,"ParentTree"=>parent}.into());doc.objects.insert(tree,dictionary!{"Type"=>"Pages","Count"=>2,"Kids"=>pages.iter().copied().map(Object::Reference).collect::<Vec<_>>()}.into());
 let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree,"AcroForm"=>form,"StructTreeRoot"=>tag_root});doc.trailer.set("Root",catalog);Fixture{doc,pages,field,widgets,hidden,form,ap}
}
fn info(doc:&Document)->crate::composition_forms::FormInfo{let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();let pages=crate::composition_pages::inspect(doc,catalog,&mut ||Ok(())).unwrap();crate::composition_forms::inspect(doc,&pages,&mut Budget::default(),&mut ||Ok(())).unwrap()}
fn widgets(doc:&Document,page:ObjectId)->Vec<ObjectId>{doc.dereference(doc.get_dictionary(page).unwrap().get(b"Annots").unwrap()).unwrap().1.as_array().unwrap().iter().map(|v|doc.dereference(v).unwrap().0.unwrap()).collect()}
fn dict<'a>(doc:&'a Document,value:&'a Object)->&'a Dictionary{doc.dereference(value).unwrap().1.as_dict().unwrap()}
#[test]
fn composition_occurrence_forms_repeats_separate_widgets_with_one_answer_and_independent_tags(){
 let f=fixture(false);let order=vec![f.pages[0],f.pages[0],f.pages[1],f.pages[1]];let pointer=f.doc.get_object(f.ap).unwrap().as_stream().unwrap().content.as_ptr();let original=f.widgets.clone();let field=f.field;let hidden=f.hidden;let ap=f.ap;
 let (mut doc,pages)=project_forms(f.doc,&order,&mut ||Ok(())).unwrap();let forms=info(&doc);assert_eq!(forms.roots,vec![field,hidden]);assert_eq!(forms.nodes[&field].children.len(),4);assert_eq!(&forms.nodes[&field].children[..2],&original);assert_eq!(forms.nodes.len(),6);
 let all=pages.iter().flat_map(|page|widgets(&doc,*page)).collect::<Vec<_>>();assert_eq!(all.iter().copied().collect::<BTreeSet<_>>().len(),4);assert_eq!(all[0],original[0]);assert_eq!(all[2],original[1]);
 for (&widget,&page) in all.iter().zip(&pages){assert_eq!(forms.nodes[&widget].owner,field);assert_eq!(forms.nodes[&widget].widget_page,Some(page));assert_eq!(dict(&doc,doc.get_dictionary(widget).unwrap().get(b"AP").unwrap()).get(b"N").unwrap(),&Object::Reference(ap));}
 assert_eq!(doc.get_object(ap).unwrap().as_stream().unwrap().content.as_ptr(),pointer);let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();assert_eq!(crate::composition_structure::inspect(&doc,catalog,&pages,&mut ||Ok(())).unwrap().owners.len(),4);
 let original_rect=doc.get_dictionary(original[0]).unwrap().get(b"Rect").unwrap().clone();doc.get_dictionary_mut(all[1]).unwrap().set("Rect",vec![50.into(),50.into(),150.into(),70.into()]);assert_eq!(doc.get_dictionary(original[0]).unwrap().get(b"Rect").unwrap(),&original_rect);
 let mut bytes=Vec::new();doc.save_to(&mut bytes).unwrap();let saved=Document::load_mem(&bytes).unwrap();assert_eq!(info(&saved),forms);assert_eq!(saved.get_dictionary(field).unwrap().get(b"V").unwrap().as_str().unwrap(),b"saved");assert_eq!(saved.get_dictionary(field).unwrap().get(b"DV").unwrap().as_str().unwrap(),b"default");assert_eq!(saved.get_dictionary(hidden).unwrap().get(b"V").unwrap().as_str().unwrap(),b"nonvisual");
}
#[test]
fn composition_occurrence_forms_repeats_merged_widget_without_replacing_original_tag_owner(){
 let f=fixture(true);let old=f.field;let hidden=f.hidden;let form=f.form;let order=vec![f.pages[1],f.pages[0],f.pages[0]];let (doc,pages)=project_forms(f.doc,&order,&mut ||Ok(())).unwrap();let forms=info(&doc);let field=forms.nodes[&old].owner;assert_ne!(old,field);assert_eq!(forms.roots,vec![field,hidden]);assert_eq!(forms.nodes[&field].children.len(),2);assert_eq!(widgets(&doc,pages[1]),vec![old]);assert_ne!(widgets(&doc,pages[2]),vec![old]);assert_eq!(doc.get_dictionary(form).unwrap().get(b"CO").unwrap().as_array().unwrap(),&vec![Object::Reference(field),hidden.into()]);assert_eq!(forms.nodes[&field].full_name,"answer");
}
#[test]
fn composition_occurrence_forms_copy_local_hide_and_navigation_keep_shared_field_actions(){
 let mut f=fixture(false);let field=f.field;let widget=f.widgets[0];let original_page=f.pages[0];let alias=f.doc.add_object(Object::Reference(field));let widget_alias=f.doc.add_object(Object::Reference(widget));
 f.doc.get_dictionary_mut(widget).unwrap().set("A",dictionary!{"S"=>"ResetForm","Fields"=>vec![Object::Reference(alias)],"Next"=>dictionary!{"S"=>"Hide","T"=>widget_alias,"Next"=>dictionary!{"S"=>"GoTo","D"=>vec![Object::Reference(original_page),Object::Name(b"Fit".to_vec())]}}});
 f.doc.get_dictionary_mut(field).unwrap().set("AA",dictionary!{"V"=>dictionary!{"S"=>"GoTo","D"=>vec![Object::Reference(original_page),Object::Name(b"Fit".to_vec())]}});
 let order=vec![f.pages[0],f.pages[0],f.pages[1]];let (doc,pages)=project_forms(f.doc,&order,&mut ||Ok(())).unwrap();let copy=widgets(&doc,pages[1])[0];let a=dict(&doc,doc.get_dictionary(copy).unwrap().get(b"A").unwrap());assert_eq!(doc.dereference(&a.get(b"Fields").unwrap().as_array().unwrap()[0]).unwrap().0,Some(field));let hide=dict(&doc,a.get(b"Next").unwrap());assert_eq!(doc.dereference(hide.get(b"T").unwrap()).unwrap().0,Some(copy));assert_eq!(dict(&doc,hide.get(b"Next").unwrap()).get(b"D").unwrap().as_array().unwrap()[0],Object::Reference(pages[1]));
 let field_aa=dict(&doc,doc.get_dictionary(field).unwrap().get(b"AA").unwrap());assert_eq!(dict(&doc,field_aa.get(b"V").unwrap()).get(b"D").unwrap().as_array().unwrap()[0],Object::Reference(original_page));let a=dict(&doc,doc.get_dictionary(widget).unwrap().get(b"A").unwrap());assert_eq!(doc.dereference(dict(&doc,a.get(b"Next").unwrap()).get(b"T").unwrap()).unwrap().0,Some(widget));
}
fn buttons(flags:i64)->Fixture{
 let mut f=fixture(false);let off=f.doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),20.into()]},b"q Q".to_vec()));
 let d=f.doc.get_dictionary_mut(f.field).unwrap();d.set("FT","Btn");d.set("Ff",flags);d.set("Opt",vec![Object::string_literal("A"),Object::string_literal("B")]);d.set("V","0");d.set("DV","1");
 for (i,&widget) in f.widgets.iter().enumerate(){let mut states=dictionary!{"Off"=>off};states.set(i.to_string(),f.ap);let d=f.doc.get_dictionary_mut(widget).unwrap();d.set("AP",dictionary!{"N"=>states.clone(),"R"=>states,"D"=>off});d.set("AS",Object::Name(if i==0{b"0".to_vec()}else{b"Off".to_vec()}));}f
}
#[test]
fn composition_occurrence_forms_indexed_buttons_keep_logical_exports_and_shared_states(){
 for flags in [0,32768,32768|33554432]{let f=buttons(flags);let field=f.field;let ap=f.ap;let pointer=f.doc.get_object(ap).unwrap().as_stream().unwrap().content.as_ptr();let order=vec![f.pages[0],f.pages[1],f.pages[0],f.pages[1]];let (doc,pages)=project_forms(f.doc,&order,&mut ||Ok(())).unwrap();let forms=info(&doc);let d=doc.get_dictionary(field).unwrap();assert_eq!(d.get(b"Ff").unwrap().as_i64().unwrap(),if flags&32768!=0{flags|33554432}else{flags});assert_eq!(d.get(b"V").unwrap().as_name().unwrap(),b"A");assert_eq!(d.get(b"DV").unwrap().as_name().unwrap(),b"B");assert_eq!(d.get(b"Opt").unwrap(),&Object::Null);
  for (i,&widget) in forms.nodes[&field].children.iter().enumerate(){let name=if i%2==0{b"A"}else{b"B"};let appearance=dict(&doc,doc.get_dictionary(widget).unwrap().get(b"AP").unwrap());for key in [b"N",b"R"]{assert_eq!(dict(&doc,appearance.get(key).unwrap()).get(name).unwrap(),&Object::Reference(ap));}assert_eq!(doc.get_dictionary(widget).unwrap().get(b"AS").unwrap().as_name().unwrap(),if i%2==0{b"A".as_slice()}else{b"Off"});}
  assert_eq!(doc.get_object(ap).unwrap().as_stream().unwrap().content.as_ptr(),pointer);assert_eq!(pages.len(),4);
  let order=vec![pages[0],pages[2],pages[2],pages[1]];let (mut doc,_)=project_forms(doc,&order,&mut ||Ok(())).unwrap();let mut bytes=vec![];doc.save_to(&mut bytes).unwrap();assert_eq!(info(&Document::load_mem(&bytes).unwrap()).nodes[&field].children.len(),4);
 }
}
#[test]
fn composition_occurrence_forms_button_conversion_refuses_ambiguous_or_invalid_states(){
 for case in 0..8{let mut f=buttons(32768);match case{
  0=>f.doc.get_dictionary_mut(f.field).unwrap().set("Opt",vec![Object::string_literal("same"),Object::string_literal("same")]),
  1=>f.doc.get_dictionary_mut(f.field).unwrap().set("Opt",vec![lopdf::text_string("漢字"),Object::string_literal("B")]),
  2=>f.doc.get_dictionary_mut(f.field).unwrap().set("Opt",vec![Object::string_literal("Off"),Object::string_literal("B")]),
  3=>f.doc.get_dictionary_mut(f.field).unwrap().set("Opt",vec![Object::string_literal(""),Object::string_literal("B")]),
  4=>f.doc.get_dictionary_mut(f.field).unwrap().set("DV","missing"),
  5=>f.doc.get_dictionary_mut(f.field).unwrap().set("V","1"),
  6=>f.doc.get_dictionary_mut(f.field).unwrap().set("Opt",vec![Object::string_literal("X".repeat(128)),Object::string_literal("B")]),
  _=>{f.doc.get_dictionary_mut(f.widgets[1]).unwrap().set("AS","1");},
 };let result=project_forms(f.doc,&[f.pages[0],f.pages[0],f.pages[1]],&mut ||Ok(()));assert!(result.is_err(),"unsafe conversion case {case}");}
}
#[test]
fn composition_occurrence_forms_button_conversion_inherited_defaults_and_latin_exports(){
 let mut f=buttons(32768|1|2);let group=f.doc.new_object_id();let d=f.doc.get_dictionary_mut(f.field).unwrap();for key in [b"Ff".as_slice(),b"V",b"DV"]{d.remove(key);}d.set("Parent",group);d.set("Opt",vec![lopdf::text_string("café"),Object::string_literal("B")]);
 f.doc.objects.insert(group,dictionary!{"T"=>Object::string_literal("group"),"Ff"=>32768|1|2,"V"=>"0","DV"=>"1","Kids"=>vec![Object::Reference(f.field)]}.into());f.doc.get_dictionary_mut(f.form).unwrap().set("Fields",vec![Object::Reference(group),f.hidden.into()]);let field=f.field;
 let (doc,_)=project_forms(f.doc,&[f.pages[0],f.pages[0],f.pages[1]],&mut ||Ok(())).unwrap();let d=doc.get_dictionary(field).unwrap();assert_eq!(d.get(b"Opt").unwrap(),&Object::Null);assert_eq!(d.get(b"Ff").unwrap().as_i64().unwrap(),32768|33554432|1|2);assert_eq!(d.get(b"V").unwrap().as_name().unwrap(),b"caf\xe9");assert_eq!(d.get(b"DV").unwrap().as_name().unwrap(),b"B");assert_eq!(doc.get_dictionary(group).unwrap().get(b"V").unwrap().as_name().unwrap(),b"0");
}
#[test]
fn composition_occurrence_forms_selection_then_duplication_keeps_hidden_and_alias_parents(){
 let mut f=fixture(false);let alias=f.doc.add_object(Object::Reference(f.field));for &w in &f.widgets{f.doc.get_dictionary_mut(w).unwrap().set("Parent",alias);}let removed=f.widgets[1];let field=f.field;let hidden=f.hidden;let order=vec![f.pages[0],f.pages[0]];let (doc,pages)=project_forms(f.doc,&order,&mut ||Ok(())).unwrap();let forms=info(&doc);assert!(!forms.nodes.contains_key(&removed));assert_eq!(forms.nodes[&field].children.len(),2);assert!(forms.nodes.contains_key(&hidden));assert_eq!(forms.nodes[&widgets(&doc,pages[1])[0]].owner,field);
}
#[test]
fn composition_occurrence_forms_choice_options_stay_per_field_and_direct_forms_survive(){
 let mut f=fixture(false);let options=vec![Object::Array(vec![Object::string_literal("one"),Object::string_literal("First row")]),Object::Array(vec![Object::string_literal("two"),Object::string_literal("Second row")])];let d=f.doc.get_dictionary_mut(f.field).unwrap();d.set("FT","Ch");d.set("Opt",options.clone());d.set("V",Object::string_literal("two"));d.set("DV",Object::string_literal("one"));d.set("I",vec![1.into()]);let form=f.doc.get_dictionary(f.form).unwrap().clone();f.doc.catalog_mut().unwrap().set("AcroForm",form);let field=f.field;
 let (doc,pages)=project_forms(f.doc,&[f.pages[0],f.pages[0],f.pages[1]],&mut ||Ok(())).unwrap();assert_eq!(info(&doc).nodes[&field].children.len(),3);let d=doc.get_dictionary(field).unwrap();assert_eq!(d.get(b"Opt").unwrap().as_array().unwrap(),&options);assert_eq!(d.get(b"I").unwrap().as_array().unwrap(),&vec![1.into()]);assert_eq!(d.get(b"V").unwrap().as_str().unwrap(),b"two");assert_eq!(pages.len(),3);
}
#[test]
fn composition_occurrence_forms_cancellation_and_capacity_refuse_without_touching_input(){
 let f=fixture(true);let before=f.doc.objects.clone();let order=vec![f.pages[0],f.pages[0],f.pages[1]];let mut calls=0;project_forms(f.doc.clone(),&order,&mut ||{calls+=1;Ok(())}).unwrap();
 for stop in [1,calls/3,2*calls/3,calls]{let mut at=0;assert!(project_forms(f.doc.clone(),&order,&mut ||{at+=1;if at==stop{Err(EngineError::InvalidRequest("cancelled".into()))}else{Ok(())}}).is_err());}
 let error=project_forms(f.doc.clone(),&vec![f.pages[0];4001],&mut ||Ok(())).err().unwrap().to_string();assert!(error.contains("4000 field/widget"),"{error}");assert_eq!(f.doc.objects,before);
}
#[test]
fn composition_occurrence_forms_rejects_unbound_widgets_and_opaque_merged_consumers(){
 for mode in 0..3{let mut f=fixture(true);match mode{
  0=>{f.doc.get_dictionary_mut(f.form).unwrap().set("Fields",vec![Object::Reference(f.hidden)]);},
  1=>{f.doc.catalog_mut().unwrap().set("OpaqueField",f.field);},
  _=>{f.doc.get_dictionary_mut(f.field).unwrap().set("AA",dictionary!{"V"=>dictionary!{"S"=>"JavaScript","JS"=>Object::string_literal("this.getField('answer')")}});},
 }assert!(project_forms(f.doc,&[f.pages[0],f.pages[0]],&mut ||Ok(())).is_err(),"mode {mode}");}
}

#[test]
fn composition_occurrence_forms_button_conversion_budget_and_cancellation(){
 let f=buttons(32768);let forms=info(&f.doc);let controls=[(f.widgets[0],f.widgets[0]),(f.widgets[1],f.widgets[1]),(f.widgets[0],(f.doc.max_id+1,0))];let before=f.doc.objects.clone();let mut budget=Budget::default();let initial=budget.metadata;let mut calls=0;
 crate::composition_button_order::repeat(&f.doc,&forms,f.field,&controls,&mut budget,&mut ||{calls+=1;Ok(())}).unwrap();let spent=initial-budget.metadata;
 assert!(crate::composition_button_order::repeat(&f.doc,&forms,f.field,&controls,&mut Budget{metadata:spent-1,..Default::default()},&mut ||Ok(())).is_err());
 for stop in [1,calls/2,calls]{let mut at=0;assert!(crate::composition_button_order::repeat(&f.doc,&forms,f.field,&controls,&mut Budget::default(),&mut ||{at+=1;if at==stop{Err(EngineError::InvalidRequest("cancel".into()))}else{Ok(())}}).is_err());}
 assert_eq!(f.doc.objects,before);
}

#[test]
fn composition_review_null_button_options_cannot_hide_ancestor_table(){
 for flags in [0,32768,32768|33554432]{for indirect in [false,true]{
  let mut f=buttons(flags);let options=f.doc.get_dictionary_mut(f.field).unwrap().remove(b"Opt").unwrap();
  let parent=f.doc.add_object(dictionary!{"T"=>Object::string_literal("group"),"Opt"=>options,"Kids"=>vec![Object::Reference(f.field)]});
  let null=if indirect{Object::Reference(f.doc.add_object(Object::Null))}else{Object::Null};
  let field=f.doc.get_dictionary_mut(f.field).unwrap();field.set("Parent",parent);field.set("Opt",null);
  f.doc.get_dictionary_mut(f.form).unwrap().set("Fields",vec![Object::Reference(parent),f.hidden.into()]);
  let result=project_forms(f.doc,&[f.pages[0],f.pages[0],f.pages[1]],&mut ||Ok(()));
  let error=result.err().expect("null must not bypass inherited indexed export refusal").to_string();assert!(error.contains("ancestor adapter"),"{error}");
 }}
}

#[test]
fn composition_review_null_button_defaults_preserve_effective_inherited_state(){
 for indirect in [false,true]{
  let mut f=buttons(32768);let parent=f.doc.add_object(dictionary!{"T"=>Object::string_literal("group"),"Ff"=>32768|1|2,"V"=>"0","DV"=>"1","Kids"=>vec![Object::Reference(f.field)]});
  let null=if indirect{Object::Reference(f.doc.add_object(Object::Null))}else{Object::Null};
  let field=f.doc.get_dictionary_mut(f.field).unwrap();field.set("Parent",parent);for key in [b"Ff".as_slice(),b"V",b"DV"]{field.set(key.to_vec(),null.clone());}
  f.doc.get_dictionary_mut(f.form).unwrap().set("Fields",vec![Object::Reference(parent),f.hidden.into()]);
  let field=f.field;let ap=f.ap;let pointer=f.doc.get_object(ap).unwrap().as_stream().unwrap().content.as_ptr();
  let (mut doc,_)=project_forms(f.doc,&[f.pages[0],f.pages[0],f.pages[1]],&mut ||Ok(())).expect("null attributes must use inherited radio state");
  let d=doc.get_dictionary(field).unwrap();assert_eq!(d.get(b"Ff").unwrap().as_i64().unwrap(),32768|33554432|1|2);assert_eq!(d.get(b"V").unwrap().as_name().unwrap(),b"A");assert_eq!(d.get(b"DV").unwrap().as_name().unwrap(),b"B");assert_eq!(doc.get_object(ap).unwrap().as_stream().unwrap().content.as_ptr(),pointer);
  let mut bytes=vec![];doc.save_to(&mut bytes).unwrap();let saved=Document::load_mem(&bytes).unwrap();assert_eq!(saved.get_dictionary(field).unwrap().get(b"DV").unwrap().as_name().unwrap(),b"B");
 }
}

#[test]
fn annotation_receipts_keep_tagged_forms_identical_while_tracking_native_notes() {
    use crate::composition_annotation_receipts::Anchor;
    let f=fixture(false);let mut doc=f.doc;let page=f.pages[0];
    let root=doc.catalog().unwrap().get(b"StructTreeRoot").unwrap().as_reference().unwrap();
    let parent_tree=doc.get_dictionary(root).unwrap().get(b"ParentTree").unwrap().as_reference().unwrap();
    let note=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","P"=>page,"Rect"=>vec![200.into(),200.into(),220.into(),220.into()],"Contents"=>Object::string_literal("tracked note"),"StructParent"=>99});
    doc.get_dictionary_mut(page).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().push(note.into());
    let element=doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"Annot","P"=>root,"Pg"=>page,"K"=>dictionary!{"Type"=>"OBJR","Obj"=>note}});
    doc.get_dictionary_mut(root).unwrap().get_mut(b"K").unwrap().as_array_mut().unwrap().push(element.into());
    doc.get_dictionary_mut(parent_tree).unwrap().get_mut(b"Nums").unwrap().as_array_mut().unwrap().extend([99.into(),element.into()]);
    let order=[page,page,f.pages[1]];
    let (expected,expected_pages)=project_forms(doc.clone(),&order,&mut ||Ok(())).unwrap();
    let (actual,actual_pages,receipts)=project_forms_tracked(doc,&order,&[Anchor{page,slot:1,annotation:note}],&mut ||Ok(())).unwrap();
    assert_eq!(actual_pages,expected_pages);assert!(actual.objects==expected.objects,"tracking changed the composed form/tag graph");
    assert_eq!(receipts.len(),2);assert_ne!(receipts[0].annotation,receipts[1].annotation);
    let catalog=actual.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let structure=crate::composition_structure::inspect(&actual,catalog,&actual_pages,&mut ||Ok(())).unwrap();
    for receipt in receipts {
        let key=actual.get_dictionary(receipt.annotation).unwrap().get(b"StructParent").unwrap().as_i64().unwrap();
        assert_eq!(structure.owners[&key].object,receipt.annotation);
    }
    assert_eq!(info(&actual).nodes[&f.field].widget_page,None);
}

