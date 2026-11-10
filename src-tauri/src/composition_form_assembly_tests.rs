use super::*;
use crate::composition_forms::{Node,FieldType};

fn group(base:u32,title:Option<&str>,children:&[&str])->FormInfo {
    let root=(base,0);let mut nodes=BTreeMap::new();let mut kids=Vec::new();
    for (offset,name) in children.iter().enumerate() {
        let id=(base+offset as u32+1,0);kids.push(id);
        nodes.insert(id,Node{parent:Some(root),children:vec![],field:true,owner:id,widget_page:None,
            partial_name:Some((*name).into()),full_name:crate::form_properties::qualified_name(title.unwrap_or(""),name),kind:Some(FieldType::Text)});
    }
    nodes.insert(root,Node{parent:None,children:kids.clone(),field:true,owner:root,widget_page:None,
        partial_name:title.map(str::to_owned),full_name:title.unwrap_or("").into(),kind:if kids.is_empty(){Some(FieldType::Text)}else{None}});
    FormInfo{present:true,form_id:Some((base+90,0)),roots:vec![root],nodes,calculation_order:kids.into_iter().rev().collect(),need_appearances:false}
}
fn stage(sources:&[&FormInfo])->EngineResult<Namespace>{namespace(sources,&mut Budget::default(),&mut ||Ok(()))}
fn title(plan:&Namespace,id:ObjectId)->String {lopdf::decode_text_string(&plan.root_titles[&id]).unwrap()}

#[test]
fn composition_form_assembly_names_preserve_unique_roots_and_reserve_future_originals(){
    let a=group(10,Some("answer"),&["given","family"]);
    let b=group(110,Some("answer"),&["given","family"]);
    let c=group(210,Some("answer_2"),&["given"]);
    let result=stage(&[&a,&b,&c]).unwrap();
    assert_eq!(result.root_titles.len(),1);assert_eq!(title(&result,(110,0)),"answer_3");
    assert_eq!(result.names[&(10,0)],"answer");assert_eq!(result.names[&(111,0)],"answer_3.given");
    assert_eq!(result.names[&(210,0)],"answer_2");
    assert_eq!(result.roots,vec![(10,0),(110,0),(210,0)]);
    assert_eq!(result.calculation_order,vec![(12,0),(11,0),(112,0),(111,0),(211,0)]);
    assert_eq!(stage(&[&a,&b,&c]).unwrap(),result);
}

#[test]
fn composition_form_assembly_descendant_collisions_and_unnamed_roots_are_isolated(){
    let a=group(10,Some("person.name"),&[]);
    let b=group(110,Some("person"),&["name","age"]);
    let result=stage(&[&a,&b]).unwrap();assert_eq!(title(&result,(110,0)),"person_2");assert_eq!(result.names[&(111,0)],"person_2.name");
    let a=group(10,None,&["given"]);let b=group(110,None,&["given"]);
    let result=stage(&[&a,&b]).unwrap();assert_eq!(title(&result,(110,0)),"Folio_2");assert_eq!(result.names[&(111,0)],"Folio_2.given");assert_eq!(result.names[&(11,0)],"given");
}

#[test]
fn composition_form_assembly_preserves_empty_and_unicode_descendant_names(){
    let a=group(10,Some("résumé"),&["","名字"]);let b=group(110,Some("résumé"),&["","名字"]);
    let result=stage(&[&a,&b]).unwrap();assert_eq!(title(&result,(110,0)),"résumé_2");
    assert_eq!(result.names[&(111,0)],"résumé_2.");assert_eq!(result.names[&(112,0)],"résumé_2.名字");
}

#[test]
fn composition_form_assembly_policy_votes_include_empty_forms_but_not_absent_forms(){
    let empty=FormInfo{present:true,..Default::default()};let absent=FormInfo::default();let mut a=group(10,Some("a"),&[]);
    assert_eq!(stage(&[&absent]).unwrap().need_appearances,None);
    assert_eq!(stage(&[&a,&empty,&absent]).unwrap().need_appearances,Some(false));
    a.need_appearances=true;
    assert_eq!(stage(&[&a,&absent]).unwrap().need_appearances,Some(true));
    assert!(stage(&[&a,&empty]).is_err(),"conflicting source policy must not be ORed or cleared");
}

#[test]
fn composition_form_assembly_cancellation_limits_and_duplicate_source_identity_refuse(){
    let a=group(10,Some("same"),&["x"]);let b=group(110,Some("same"),&["x"]);
    let sources=[&a,&b];let mut calls=0;
    let expected=namespace(&sources,&mut Budget::default(),&mut ||{calls+=1;Ok(())}).unwrap();
    for stop in 1..=calls {let mut count=0;assert!(namespace(&sources,&mut Budget::default(),&mut ||{count+=1;if count==stop{Err(EngineError::InvalidRequest("cancel".into()))}else{Ok(())}}).is_err());}
    assert!(namespace(&sources,&mut Budget{metadata:1,..Default::default()},&mut ||Ok(())).is_err());
    assert!(namespace(&sources,&mut Budget{steps:100_000,..Default::default()},&mut ||Ok(())).is_err());
    assert!(stage(&[&a,&a]).is_err());assert_eq!(stage(&sources).unwrap(),expected);
}

#[test]
fn composition_form_assembly_many_repeated_names_fit_the_transaction_budget(){
    let sources=(0..1000).map(|index|group(index*100+10,Some("answer"),&[])).collect::<Vec<_>>();
    let refs=sources.iter().collect::<Vec<_>>();let mut budget=Budget::default();
    let result=namespace(&refs,&mut budget,&mut ||Ok(())).unwrap();
    assert_eq!(result.root_titles.len(),999);assert_eq!(title(&result,(99910,0)),"answer_1000");
    assert_eq!(result.names.values().collect::<std::collections::BTreeSet<_>>().len(),1000);
    assert!(budget.steps<30_000,"namespace retries must not rescan every previously assigned suffix");
}
