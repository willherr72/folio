use super::*;
use lopdf::{dictionary,Object,ObjectId,Stream};
struct Input {pages:SourcePages,form:ObjectId,field:Option<ObjectId>,font:ObjectId}
fn input(doc:&mut Document,name:Option<&str>,font_name:&str,da:Option<&str>)->Input {
    let tree=doc.new_object_id();let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()]});
    doc.objects.insert(tree,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into());
    let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>font_name});
    let field=name.map(|name|doc.add_object(dictionary!{"T"=>Object::string_literal(name),"FT"=>"Tx","V"=>Object::string_literal(font_name)}));
    let mut form=dictionary!{"Fields"=>field.into_iter().map(Object::Reference).collect::<Vec<_>>(),"DR"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}};
    if let Some(da)=da{form.set("DA",Object::string_literal(da));}
    if let Some(field)=field{form.set("CO",vec![Object::Reference(field)]);}
    let form=doc.add_object(form);let root=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree,"AcroForm"=>form});
    let pages=crate::composition_pages::inspect(doc,root,&mut ||Ok(())).unwrap();Input{pages,form,field,font}
}
fn prepared(doc:&Document,inputs:&[Input])->EngineResult<Plan> {
    let pages=inputs.iter().map(|i|SourcePages{catalog:i.pages.catalog,root:i.pages.root,pages:i.pages.pages.clone()}).collect::<Vec<_>>();
    prepare(doc,&pages,&mut Budget::default(),Arc::new(||false),&mut ||Ok(()))
}

#[test]
fn composition_form_catalog_stages_complete_dictionary_without_changing_fields_or_payloads() {
    let mut doc=Document::with_version("1.7");
    let a=input(&mut doc,Some("answer"),"Helvetica",Some("/F1 12 Tf"));
    let b=input(&mut doc,Some("answer"),"Courier",Some("/F1 10 Tf"));
    doc.get_dictionary_mut(a.form).unwrap().set("SigFlags",1);doc.get_dictionary_mut(b.form).unwrap().set("SigFlags",2);
    let expected=vec![Object::Reference(a.field.unwrap()),Object::Reference(b.field.unwrap())];
    let before=doc.objects.clone();let plan=prepared(&doc,&[a,b]).unwrap();let form=plan.form.unwrap();
    assert_eq!(form.get(b"Fields").unwrap().as_array().unwrap(),&expected);
    assert_eq!(plan.forms.len(),2);
    assert_eq!(plan.forms.iter().flat_map(|form|form.roots.iter().copied()).map(Object::Reference).collect::<Vec<_>>(),expected);
    assert_eq!(form.get(b"CO").unwrap().as_array().unwrap(),&expected);
    assert_eq!(form.get(b"SigFlags").unwrap(),&Object::Integer(3));assert_eq!(form.get(b"NeedAppearances").unwrap(),&Object::Boolean(false));
    assert_eq!(plan.namespace.names[&plan.namespace.roots[1]],"answer_2");
    assert!(plan.patches.entries.keys().all(|(_,key)|key!=b"T"),"names wait for typed action authorization");
    assert!(plan.patches.global_da.is_none(),"global fallback must be transferred into the catalog plan");
    assert_eq!(doc.objects,before);
}

#[test]
fn composition_form_catalog_preserves_a_required_global_fallback_and_distinct_fonts() {
    let mut doc=Document::with_version("1.7");
    let a=input(&mut doc,Some("a"),"Helvetica",Some("/F1 12 Tf"));let b=input(&mut doc,Some("b"),"Courier",Some("/F1 10 Tf"));
    doc.get_dictionary_mut(a.field.unwrap()).unwrap().set("DA",Object::Null);
    let fonts=[a.font,b.font];let fields=[a.field.unwrap(),b.field.unwrap()];
    let plan=prepared(&doc,&[a,b]).unwrap();let form=plan.form.unwrap();
    assert_eq!(form.get(b"DA").unwrap().as_str().unwrap(),b"/F1 12 Tf");
    let dr=form.get(b"DR").unwrap().as_dict().unwrap().get(b"Font").unwrap().as_dict().unwrap();
    assert_eq!(dr.get(b"F1").unwrap(),&Object::Reference(fonts[0]));assert_eq!(dr.get(b"Folio2_F1").unwrap(),&Object::Reference(fonts[1]));
    assert!(!plan.patches.entries.contains_key(&(fields[0],b"DA".to_vec())));
    assert_eq!(plan.patches.entries[&(fields[1],b"DA".to_vec())].as_str().unwrap(),b"/Folio2_F1 10 Tf");
}

#[test]
fn composition_form_catalog_preserves_empty_form_defaults_and_distinguishes_absent_forms() {
    let mut doc=Document::with_version("1.7");let a=input(&mut doc,None,"Helvetica",Some("/Missing 9 Tf"));
    doc.get_dictionary_mut(a.form).unwrap().set("Q",2);
    let plan=prepared(&doc,std::slice::from_ref(&a)).unwrap();let form=plan.form.unwrap();
    assert!(form.get(b"Fields").unwrap().as_array().unwrap().is_empty());
    assert_eq!(form.get(b"DA").unwrap().as_str().unwrap(),b"/Missing 9 Tf");assert_eq!(form.get(b"Q").unwrap(),&Object::Integer(2));
    doc.get_dictionary_mut(a.pages.catalog).unwrap().remove(b"AcroForm");
    assert!(prepared(&doc,&[a]).unwrap().form.is_none());
}

#[test]
fn composition_form_catalog_skips_content_freshness_scan_when_all_font_names_stay_unchanged() {
    let mut doc=Document::with_version("1.7");let a=input(&mut doc,Some("a"),"Helvetica",Some("/F1 12 Tf"));
    let content=doc.add_object(Stream::new(Dictionary::new(),b"q BI /W 1 /H 1 /BPC 8 /CS /G ID x EI Q".to_vec()));
    doc.get_dictionary_mut(a.pages.pages[0]).unwrap().set("Contents",content);
    assert!(prepared(&doc,&[a]).is_ok(),"identity maps need no content namespace extension proof");
}

#[test]
fn composition_form_catalog_rejects_unknown_flags_policies_and_cancellation_without_mutation() {
    let mut doc=Document::with_version("1.7");let a=input(&mut doc,Some("a"),"Helvetica",Some("/F1 12 Tf"));
    assert!(prepared(&doc,std::slice::from_ref(&a)).is_ok());
    for (key,value) in [("SigFlags",Object::Integer(4)),("PrivateBehavior",Object::Boolean(true)),("Q",Object::Integer(9))] {
        doc.get_dictionary_mut(a.form).unwrap().set(key,value);let before=doc.objects.clone();
        assert!(prepared(&doc,std::slice::from_ref(&a)).is_err(),"{key}");assert_eq!(before,doc.objects);doc.get_dictionary_mut(a.form).unwrap().remove(key.as_bytes());
    }
    let pages=vec![SourcePages{catalog:a.pages.catalog,root:a.pages.root,pages:a.pages.pages.clone()}];
    assert!(prepare(&doc,&pages,&mut Budget::default(),Arc::new(||true),&mut ||Ok(())).is_err());
    assert!(prepare(&doc,&pages,&mut Budget{metadata:1,..Default::default()},Arc::new(||false),&mut ||Ok(())).is_err());
    let b=input(&mut doc,Some("b"),"Courier",Some("/F1 10 Tf"));doc.get_dictionary_mut(b.form).unwrap().set("NeedAppearances",true);
    assert!(prepared(&doc,&[a,b]).is_err());
}

#[test]
fn composition_form_catalog_reallocates_an_alias_used_only_by_existing_content() {
    let mut doc=Document::with_version("1.7");let a=input(&mut doc,Some("a"),"Helvetica",Some("/F1 12 Tf"));
    let b=input(&mut doc,Some("b"),"Courier",Some("/F1 10 Tf"));let field=b.field.unwrap();let font=b.font;
    let stream=doc.add_object(Stream::new(Dictionary::new(),b"BT /Folio2_F1 10 Tf (existing) Tj ET".to_vec()));
    doc.get_dictionary_mut(b.pages.pages[0]).unwrap().set("Contents",stream);
    let before=doc.objects.clone();let plan=prepared(&doc,&[a,b]).unwrap();
    let dr=plan.form.as_ref().unwrap().get(b"DR").unwrap().as_dict().unwrap().get(b"Font").unwrap().as_dict().unwrap();
    assert!(!dr.has(b"Folio2_F1"));assert_eq!(dr.get(b"Folio2_F2").unwrap(),&Object::Reference(font));
    assert_eq!(plan.patches.entries[&(field,b"DA".to_vec())].as_str().unwrap(),b"/Folio2_F2 10 Tf");assert_eq!(doc.objects,before);
}

#[test]
fn composition_form_catalog_empty_forms_require_compatible_authoring_defaults() {
    let mut doc=Document::with_version("1.7");
    let a=input(&mut doc,None,"Helvetica",Some("0 g"));let b=input(&mut doc,None,"Courier",Some("0 g"));
    doc.get_dictionary_mut(a.form).unwrap().set("Q",2);doc.get_dictionary_mut(b.form).unwrap().set("Q",2);
    let inputs=[a,b];let form=prepared(&doc,&inputs).unwrap().form.unwrap();
    assert_eq!(form.get(b"DA").unwrap().as_str().unwrap(),b"0 g");assert_eq!(form.get(b"Q").unwrap(),&Object::Integer(2));
    doc.get_dictionary_mut(inputs[1].form).unwrap().set("Q",1);assert!(prepared(&doc,&inputs).is_err());
    doc.get_dictionary_mut(inputs[1].form).unwrap().set("Q",2);doc.get_dictionary_mut(inputs[1].form).unwrap().set("DA",Object::string_literal("1 g"));
    assert!(prepared(&doc,&inputs).is_err());
}

#[test]
fn composition_form_catalog_complete_pipeline_uses_one_metadata_budget_and_checks_midflight_cancellation() {
    let mut doc=Document::with_version("1.7");
    let a=input(&mut doc,Some("answer"),"Helvetica",Some("/F1 12 Tf"));let b=input(&mut doc,Some("answer"),"Courier",Some("/F1 10 Tf"));
    let pages=[a.pages,b.pages];let before=doc.objects.clone();let mut calls=0;let mut budget=Budget::default();let initial=budget.metadata;
    prepare(&doc,&pages,&mut budget,Arc::new(||false),&mut ||{calls+=1;Ok(())}).unwrap();
    let mut short=Budget{metadata:initial-budget.metadata-1,..Default::default()};
    assert!(prepare(&doc,&pages,&mut short,Arc::new(||false),&mut ||Ok(())).is_err());
    for stop in [1,calls/2,calls] {
        let mut at=0;
        assert!(prepare(&doc,&pages,&mut Budget::default(),Arc::new(||false),&mut ||{at+=1;if at==stop{Err(EngineError::InvalidRequest("cancel".into()))}else{Ok(())}}).is_err());
    }
    assert_eq!(before,doc.objects);
}
