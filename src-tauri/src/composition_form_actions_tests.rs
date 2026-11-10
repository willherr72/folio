use super::*;
use lopdf::dictionary;
struct Fixture{doc:Document,pages:SourcePages,forms:FormInfo,other:SourcePages,other_forms:FormInfo,namespace:Namespace,group:ObjectId,first:ObjectId,button:ObjectId,widget:ObjectId,note:ObjectId,foreign:ObjectId}
fn catalog(doc:&mut Document,fields:Vec<ObjectId>)->SourcePages {
    let tree=doc.new_object_id();let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()]});
    doc.objects.insert(tree,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into());
    let form=doc.add_object(dictionary!{"Fields"=>fields.into_iter().map(Object::Reference).collect::<Vec<_>>()});
    let root=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree,"AcroForm"=>form});
    crate::composition_pages::inspect(doc,root,&mut ||Ok(())).unwrap()
}
fn fixture()->Fixture {
    let mut doc=Document::with_version("1.7");
    let foreign=doc.add_object(dictionary!{"T"=>Object::string_literal("group"),"FT"=>"Tx"});let other=catalog(&mut doc,vec![foreign]);
    let group=doc.new_object_id();let first=doc.new_object_id();let button=doc.add_object(dictionary!{"T"=>Object::string_literal("button"),"Parent"=>group,"FT"=>"Btn","Ff"=>65536});
    let pages=catalog(&mut doc,vec![group]);let page=pages.pages[0];
    let widget=doc.add_object(dictionary!{"Subtype"=>"Widget","Parent"=>first,"P"=>page});
    let note=doc.add_object(dictionary!{"Subtype"=>"Text","P"=>page});
    doc.objects.insert(first,dictionary!{"T"=>Object::string_literal("first"),"Parent"=>group,"FT"=>"Tx","Kids"=>vec![Object::Reference(widget)],"V"=>Object::string_literal("answer")}.into());
    doc.objects.insert(group,dictionary!{"T"=>Object::string_literal("group"),"Kids"=>vec![Object::Reference(first),Object::Reference(button)]}.into());
    doc.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(widget),Object::Reference(note)]);
    let forms=crate::composition_forms::inspect(&doc,&pages,&mut Budget::default(),&mut ||Ok(())).unwrap();
    let other_forms=crate::composition_forms::inspect(&doc,&other,&mut Budget::default(),&mut ||Ok(())).unwrap();
    let namespace=crate::composition_form_assembly::namespace(&[&other_forms,&forms],&mut Budget::default(),&mut ||Ok(())).unwrap();
    Fixture{doc,pages,forms,other,other_forms,namespace,group,first,button,widget,note,foreign}
}
fn run(f:&Fixture,action:&Dictionary)->EngineResult<Changes>{
    let mut budget=Budget::default();let scope=bind(&f.doc,&f.pages,&f.forms,&f.namespace,&mut budget,&mut ||Ok(()))?;
    rewrite(&f.doc,&scope,action,&mut budget,&mut ||Ok(()))
}
fn targets(changes:&Changes)->&Vec<Object>{changes.action.get(b"Fields".as_slice()).unwrap().as_array().unwrap()}

#[test]
fn composition_form_actions_reset_all_and_exclusions_stay_in_source_domain(){
    let f=fixture();let before=f.doc.objects.clone();
    let all=run(&f,&dictionary!{"S"=>"ResetForm","Flags"=>1}).unwrap();
    assert_eq!(targets(&all),&vec![Object::Reference(f.group)]);assert_eq!(all.action[b"Flags".as_slice()],Object::Integer(0));
    let excluded=run(&f,&dictionary!{"S"=>"ResetForm","Fields"=>vec![Object::string_literal("group.first")],"Flags"=>1}).unwrap();
    assert_eq!(targets(&excluded),&vec![Object::string_literal("group_2.first"),Object::Reference(f.foreign)]);
    let empty=run(&f,&dictionary!{"S"=>"ResetForm","Fields"=>Vec::<Object>::new()}).unwrap();
    assert!(targets(&empty).is_empty(),"explicit empty inclusion stays empty rather than becoming all fields");
    assert_eq!(before,f.doc.objects);
}

#[test]
fn composition_form_actions_explicit_reset_targets_preserve_canonical_aliases_and_next(){
    let mut f=fixture();let alias=f.doc.add_object(Object::Reference(f.first));let name=f.doc.add_object(Object::string_literal("group.button"));
    let next=f.doc.add_object(dictionary!{"S"=>"URI","URI"=>Object::string_literal("https://example.invalid")});
    let changes=run(&f,&dictionary!{"S"=>"ResetForm","Fields"=>vec![Object::Reference(alias),Object::Reference(name)],"Next"=>next}).unwrap();
    assert_eq!(targets(&changes),&vec![Object::Reference(alias),Object::string_literal("group_2.button")]);
    assert!(!changes.action.contains_key(b"Next".as_slice()),"the chain walker retains Next");
}

#[test]
fn composition_form_actions_hide_names_and_annotation_references_remain_distinct(){
    let f=fixture();let changes=run(&f,&dictionary!{"S"=>"Hide","T"=>vec![Object::Reference(f.note),Object::Reference(f.widget),Object::string_literal("group.first")],"H"=>false}).unwrap();
    assert_eq!(changes.action[b"T".as_slice()],Object::Array(vec![Object::Reference(f.note),Object::Reference(f.widget),Object::string_literal("group_2.first")]));
    assert!(!changes.action.contains_key(b"H".as_slice()));assert!(changes.fields.is_empty());
    assert!(run(&f,&dictionary!{"S"=>"Hide","T"=>f.first}).is_err(),"field-parent dictionaries are not annotation identities");
    assert!(run(&f,&dictionary!{"S"=>"Hide","T"=>f.foreign}).is_err());
}

#[test]
fn composition_form_actions_submit_all_fdf_keeps_pushbutton_omission_and_export_names(){
    let f=fixture();let changes=run(&f,&dictionary!{"S"=>"SubmitForm","F"=>Object::string_literal("https://example.invalid"),"Flags"=>2}).unwrap();
    assert_eq!(targets(&changes),&vec![Object::Reference(f.first)]);
    assert_eq!(changes.action[b"Flags".as_slice()],Object::Integer(2));
    assert_eq!(changes.fields[&(f.first,b"TM".to_vec())].as_str().unwrap(),b"group.first");
    assert!(!changes.fields.contains_key(&(f.button,b"TM".to_vec())));
    let html=run(&f,&dictionary!{"S"=>"SubmitForm","F"=>Object::string_literal("https://example.invalid"),"Flags"=>4}).unwrap();
    assert_eq!(targets(&html),&vec![Object::Reference(f.group)]);
    assert_eq!(html.fields[&(f.button,b"TM".to_vec())].as_str().unwrap(),b"group.button");
}

#[test]
fn composition_form_actions_submit_keeps_existing_mapping_and_noexport_behavior(){
    let mut f=fixture();f.doc.get_dictionary_mut(f.first).unwrap().set("TM",Object::string_literal("server_answer"));
    f.doc.get_dictionary_mut(f.button).unwrap().set("Ff",65536|4);
    let changes=run(&f,&dictionary!{"S"=>"SubmitForm","F"=>Object::string_literal("https://example.invalid"),"Fields"=>vec![Object::string_literal("group")]}).unwrap();
    assert_eq!(targets(&changes),&vec![Object::string_literal("group_2")]);assert!(changes.fields.is_empty());
    assert_eq!(f.doc.get_dictionary(f.first).unwrap().get(b"TM").unwrap().as_str().unwrap(),b"server_answer");
}

#[test]
fn composition_form_actions_refuse_unbound_targets_and_unrepresented_document_submissions(){
    let f=fixture();assert!(run(&f,&dictionary!{"S"=>"ResetForm"}).is_ok());
    for action in [dictionary!{"S"=>"ResetForm","Fields"=>vec![Object::string_literal("missing")]},
        dictionary!{"S"=>"ResetForm","Fields"=>vec![Object::Reference(f.foreign)]},
        dictionary!{"S"=>"ResetForm","Fields"=>vec![Object::Reference(f.widget)]},
        dictionary!{"S"=>"ResetForm","Flags"=>2},dictionary!{"S"=>"Hide","T"=>Object::string_literal("group"),"H"=>42}] {
        assert!(run(&f,&action).is_err(),"{action:?}");
    }
    for flags in [64,128,256,8192,4096,-1] {
        assert!(run(&f,&dictionary!{"S"=>"SubmitForm","F"=>Object::string_literal("https://example.invalid"),"Flags"=>flags}).is_err(),"{flags}");
    }
}

#[test]
fn composition_form_actions_navigation_rewrites_widget_field_and_page_chains_with_names(){
    let Fixture{mut doc,pages,forms,other,other_forms,namespace,group,first,widget,foreign,..}=fixture();
    let uri=doc.add_object(dictionary!{"S"=>"URI","URI"=>Object::string_literal("https://example.invalid/next")});
    let reset=doc.add_object(dictionary!{"S"=>"ResetForm","Next"=>uri});
    let submit=doc.add_object(dictionary!{"S"=>"SubmitForm","F"=>Object::string_literal("https://example.invalid/submit"),"Fields"=>vec![Object::string_literal("group.first")]});
    doc.get_dictionary_mut(widget).unwrap().set("A",reset);
    doc.get_dictionary_mut(widget).unwrap().set("AA",dictionary!{"Fo"=>submit});
    let second_submit=doc.add_object(doc.get_dictionary(submit).unwrap().clone());
    doc.get_dictionary_mut(first).unwrap().set("AA",dictionary!{"V"=>second_submit});
    doc.get_dictionary_mut(pages.pages[0]).unwrap().set("AA",dictionary!{"O"=>dictionary!{"S"=>"Hide","T"=>Object::string_literal("group.first")}});
    crate::composition_navigation::reconcile_forms(&mut doc,&[other,pages],&[Default::default(),Default::default()],&[other_forms,forms],&namespace,&mut Budget::default(),&mut ||Ok(())).unwrap();
    assert_eq!(crate::form_properties::text(&doc,doc.get_dictionary(group).unwrap().get(b"T").unwrap(),32768).unwrap(),"group_2");
    assert_eq!(doc.get_dictionary(foreign).unwrap().get(b"T").unwrap().as_str().unwrap(),b"group");
    let action=doc.dereference(doc.get_dictionary(widget).unwrap().get(b"A").unwrap()).unwrap().1.as_dict().unwrap();
    assert_eq!(action.get(b"Fields").unwrap().as_array().unwrap(),&vec![Object::Reference(group)]);
    assert_eq!(action.get(b"Next").unwrap(),&Object::Reference(uri));
    let aa=doc.dereference(doc.get_dictionary(first).unwrap().get(b"AA").unwrap()).unwrap().1.as_dict().unwrap();
    let action=doc.dereference(aa.get(b"V").unwrap()).unwrap().1.as_dict().unwrap();
    assert_eq!(action.get(b"Fields").unwrap().as_array().unwrap(),&vec![Object::string_literal("group_2.first")]);
    assert_eq!(doc.get_dictionary(first).unwrap().get(b"TM").unwrap().as_str().unwrap(),b"group.first");
}

#[test]
fn composition_form_actions_navigation_refuses_scripts_atomically(){
    let Fixture{mut doc,pages,forms,other,other_forms,namespace,first,widget,..}=fixture();
    doc.get_dictionary_mut(widget).unwrap().set("A",dictionary!{"S"=>"ResetForm"});
    doc.get_dictionary_mut(first).unwrap().set("AA",dictionary!{"V"=>dictionary!{"S"=>"JavaScript","JS"=>Object::string_literal("this.resetForm()")}});
    let before=doc.objects.clone();
    assert!(crate::composition_navigation::reconcile_forms(&mut doc,&[other,pages],&[Default::default(),Default::default()],&[other_forms,forms],&namespace,&mut Budget::default(),&mut ||Ok(())).is_err());
    assert_eq!(doc.objects,before);
}

#[test]
fn composition_form_actions_preserve_bounded_existing_mapping_names_larger_than_lookup_limit(){
    let mut f=fixture();let mapping=Object::string_literal(vec![b'm';40*1024]);
    f.doc.get_dictionary_mut(f.first).unwrap().set("TM",mapping.clone());
    let changes=run(&f,&dictionary!{"S"=>"SubmitForm","F"=>Object::string_literal("https://example.invalid"),"Fields"=>vec![Object::Reference(f.first)]}).unwrap();
    assert!(changes.fields.is_empty());assert_eq!(f.doc.get_dictionary(f.first).unwrap().get(b"TM").unwrap(),&mapping);
}

#[test]
fn composition_form_actions_merged_widget_has_one_owner_and_preserves_its_value(){
    let Fixture{mut doc,pages,other,other_forms,group,first,note,..}=fixture();
    doc.get_dictionary_mut(first).unwrap().remove(b"Kids");
    doc.get_dictionary_mut(first).unwrap().set("Subtype","Widget");doc.get_dictionary_mut(first).unwrap().set("P",pages.pages[0]);
    doc.get_dictionary_mut(first).unwrap().set("A",dictionary!{"S"=>"ResetForm"});
    doc.get_dictionary_mut(first).unwrap().set("AA",dictionary!{"V"=>dictionary!{"S"=>"ResetForm","Fields"=>vec![Object::string_literal("group.first")]}});
    doc.get_dictionary_mut(pages.pages[0]).unwrap().set("Annots",vec![Object::Reference(first),Object::Reference(note)]);
    let forms=crate::composition_forms::inspect(&doc,&pages,&mut Budget::default(),&mut ||Ok(())).unwrap();
    let namespace=crate::composition_form_assembly::namespace(&[&other_forms,&forms],&mut Budget::default(),&mut ||Ok(())).unwrap();
    crate::composition_navigation::reconcile_forms(&mut doc,&[other,pages],&[Default::default(),Default::default()],&[other_forms,forms],&namespace,&mut Budget::default(),&mut ||Ok(())).unwrap();
    let field=doc.get_dictionary(first).unwrap();assert_eq!(field.get(b"Parent").unwrap(),&Object::Reference(group));
    assert_eq!(field.get(b"V").unwrap().as_str().unwrap(),b"answer");
    let aa=doc.dereference(field.get(b"AA").unwrap()).unwrap().1.as_dict().unwrap();
    let action=doc.dereference(aa.get(b"V").unwrap()).unwrap().1.as_dict().unwrap();
    assert_eq!(action.get(b"Fields").unwrap().as_array().unwrap(),&vec![Object::string_literal("group_2.first")]);
}

#[test]
fn composition_form_actions_share_budget_and_decode_bom_targets(){
    let f=fixture();let action=dictionary!{"S"=>"ResetForm","Fields"=>vec![Object::string_literal(b"\xef\xbb\xbfgroup.first".to_vec())]};
    assert_eq!(targets(&run(&f,&action).unwrap()),&vec![Object::string_literal("group_2.first")]);
    let mut budget=Budget::default();let initial=budget.metadata;
    let scope=bind(&f.doc,&f.pages,&f.forms,&f.namespace,&mut budget,&mut ||Ok(())).unwrap();
    rewrite(&f.doc,&scope,&action,&mut budget,&mut ||Ok(())).unwrap();let spent=initial-budget.metadata;
    let attempt=|budget:&mut Budget|->EngineResult<Changes>{let scope=bind(&f.doc,&f.pages,&f.forms,&f.namespace,budget,&mut ||Ok(()))?;rewrite(&f.doc,&scope,&action,budget,&mut ||Ok(()))};
    assert!(attempt(&mut Budget{metadata:spent-1,..Default::default()}).is_err());
    assert!(run(&f,&dictionary!{"S"=>"ResetForm","Fields"=>vec![Object::string_literal(b"\xfe\xffx".to_vec())]}).is_err());
}

#[test]
fn composition_form_actions_navigation_cancellation_keeps_actions_names_and_mapping_atomic(){
    let Fixture{mut doc,pages,forms,other,other_forms,namespace,first,widget,..}=fixture();
    doc.get_dictionary_mut(widget).unwrap().set("A",dictionary!{"S"=>"ResetForm"});
    doc.get_dictionary_mut(first).unwrap().set("AA",dictionary!{"V"=>dictionary!{"S"=>"SubmitForm","F"=>Object::string_literal("https://example.invalid"),"Fields"=>vec![Object::Reference(first)]}});
    let pages=[other,pages];let forms=[other_forms,forms];let outlines=[Default::default(),Default::default()];let before=doc.objects.clone();let mut calls=0;
    crate::composition_navigation::reconcile_forms(&mut doc.clone(),&pages,&outlines,&forms,&namespace,&mut Budget::default(),&mut ||{calls+=1;Ok(())}).unwrap();
    for stop in [1,calls/3,2*calls/3,calls] {
        let mut at=0;
        assert!(crate::composition_navigation::reconcile_forms(&mut doc,&pages,&outlines,&forms,&namespace,&mut Budget::default(),&mut ||{at+=1;if at==stop{Err(EngineError::InvalidRequest("cancel".into()))}else{Ok(())}}).is_err());
        assert_eq!(before,doc.objects);
    }
}
fn selected_action(f:&Fixture,action:&Dictionary)->EngineResult<Changes>{
 let retained=BTreeSet::from([f.group,f.button]);let pages=BTreeSet::new();let mut budget=Budget::default();
 let scope=bind(&f.doc,&f.pages,&f.forms,&f.namespace,&mut budget,&mut ||Ok(()))?;let scope=select(scope,&retained,&pages);rewrite(&f.doc,&scope,action,&mut budget,&mut ||Ok(()))
}
#[test]
fn composition_form_actions_selection_filters_original_targets_without_rebinding_names(){
 let f=fixture();let changes=selected_action(&f,&dictionary!{"S"=>"ResetForm","Fields"=>vec![Object::Reference(f.first),Object::string_literal("group.button")]}).unwrap();
 assert_eq!(targets(&changes),&vec![Object::string_literal("group_2.button")]);
 let excluded=selected_action(&f,&dictionary!{"S"=>"ResetForm","Fields"=>vec![Object::string_literal("group.first")],"Flags"=>1}).unwrap();assert_eq!(targets(&excluded),&vec![Object::Reference(f.foreign)]);assert!(!excluded.action.contains_key(b"Flags".as_slice()),"unchanged exclusion flag stays in the original action");
 let parent=selected_action(&f,&dictionary!{"S"=>"ResetForm","Fields"=>vec![Object::Reference(f.group)]}).unwrap();assert_eq!(targets(&parent),&vec![Object::Reference(f.group)]);
 assert!(selected_action(&f,&dictionary!{"S"=>"ResetForm","Fields"=>vec![Object::string_literal("unknown")]}).is_err());
}
#[test]
fn composition_form_actions_selection_preserves_empty_and_implicit_submission_domains(){
 let f=fixture();let fdf=selected_action(&f,&dictionary!{"S"=>"SubmitForm","F"=>Object::string_literal("https://example.invalid")}).unwrap();assert!(targets(&fdf).is_empty());assert!(fdf.fields.is_empty(),"removed fields must not receive export-name patches");
 let explicit=selected_action(&f,&dictionary!{"S"=>"SubmitForm","F"=>Object::string_literal("https://example.invalid"),"Fields"=>vec![Object::Reference(f.first)]}).unwrap();assert!(targets(&explicit).is_empty());assert!(explicit.fields.is_empty());
 let all=selected_action(&f,&dictionary!{"S"=>"ResetForm"}).unwrap();assert_eq!(targets(&all),&vec![Object::Reference(f.group)]);
 let retained=BTreeSet::new();let pages=BTreeSet::new();let mut budget=Budget::default();let scope=select(bind(&f.doc,&f.pages,&f.forms,&f.namespace,&mut budget,&mut ||Ok(())).unwrap(),&retained,&pages);
 let all=rewrite(&f.doc,&scope,&dictionary!{"S"=>"ResetForm"},&mut budget,&mut ||Ok(())).unwrap();assert!(targets(&all).is_empty());
}
#[test]
fn composition_form_actions_selection_hide_removes_deleted_annotations_and_keeps_next(){
 let f=fixture();let next=dictionary!{"S"=>"URI","URI"=>Object::string_literal("https://example.invalid/next")};
 let action=dictionary!{"S"=>"Hide","T"=>vec![Object::Reference(f.note),Object::string_literal("group.first"),Object::string_literal("group.button")],"Next"=>next.clone()};
 let changes=selected_action(&f,&action).unwrap();assert_eq!(changes.action[b"T".as_slice()],Object::Array(vec![Object::string_literal("group_2.button")]));assert!(!changes.action.contains_key(b"Next".as_slice()));assert_eq!(action.get(b"Next").unwrap(),&Object::Dictionary(next));
 let empty=selected_action(&f,&dictionary!{"S"=>"Hide","T"=>Object::Reference(f.note)}).unwrap();assert_eq!(empty.action[b"T".as_slice()],Object::Array(vec![]));
 let retained=f.forms.nodes.keys().copied().collect();let pages=f.pages.pages.iter().copied().collect();let mut budget=Budget::default();let scope=select(bind(&f.doc,&f.pages,&f.forms,&f.namespace,&mut budget,&mut ||Ok(())).unwrap(),&retained,&pages);
 let all=rewrite(&f.doc,&scope,&dictionary!{"S"=>"Hide","T"=>Object::Reference(f.note)},&mut budget,&mut ||Ok(())).unwrap();assert_eq!(all.action[b"T".as_slice()],Object::Reference(f.note));
}
