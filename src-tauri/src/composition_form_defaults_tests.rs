use super::*;
use lopdf::{dictionary,Stream};
struct Fixture {doc:Document,pages:SourcePages,form:ObjectId,field:ObjectId,widgets:[ObjectId;2],appearances:[ObjectId;2],fonts:[ObjectId;4]}
fn fixture()->Fixture {
    let mut doc=Document::with_version("1.7");let tree=doc.new_object_id();let field=doc.new_object_id();
    let fonts=["Helvetica","Courier","Times-Roman","Helvetica-Bold"].map(|name|doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>name}));
    let mut pages=Vec::new();let mut widgets=Vec::new();let mut appearances=Vec::new();
    for index in 0..2 {
        let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>fonts[3]}}});
        let ap=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),20.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>fonts[index+1]}}},b"BT /F1 10 Tf (Answer) Tj ET".to_vec()));
        let widget=doc.add_object(dictionary!{"Subtype"=>"Widget","Parent"=>field,"P"=>page,"Rect"=>vec![0.into(),0.into(),100.into(),20.into()],"AP"=>dictionary!{"N"=>ap}});
        doc.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(widget)]);pages.push(page);widgets.push(widget);appearances.push(ap);
    }
    doc.objects.insert(field,dictionary!{"T"=>Object::string_literal("answer"),"FT"=>"Tx","DA"=>Object::string_literal("/F1 12 Tf 0 g"),"Q"=>2,"DR"=>dictionary!{"Font"=>dictionary!{"F1"=>fonts[2]}},"Kids"=>widgets.iter().copied().map(Object::Reference).collect::<Vec<_>>()}.into());
    let form=doc.add_object(dictionary!{"Fields"=>vec![Object::Reference(field)],"DA"=>Object::string_literal("/F1 10 Tf 1 0 0 rg"),"Q"=>1,"DR"=>dictionary!{"Font"=>dictionary!{"F1"=>fonts[0]}}});
    doc.objects.insert(tree,dictionary!{"Type"=>"Pages","Count"=>2,"Kids"=>pages.iter().copied().map(Object::Reference).collect::<Vec<_>>()}.into());
    let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree,"AcroForm"=>form});doc.trailer.set("Root",catalog);
    let pages=crate::composition_pages::inspect(&doc,catalog,&mut ||Ok(())).unwrap();
    Fixture{doc,pages,form,field,widgets:[widgets[0],widgets[1]],appearances:[appearances[0],appearances[1]],fonts}
}
fn model(f:&Fixture)->crate::composition_forms::FormInfo {crate::composition_forms::inspect(&f.doc,&f.pages,&mut Default::default(),&mut ||Ok(())).unwrap()}
fn inspect_fixture(f:&Fixture)->EngineResult<Inventory<'_>>{inspect(&f.doc,&f.pages,&model(f),&mut Budget::default(),Arc::new(||false),&mut ||Ok(()))}
fn id(value:Option<&Object>)->Option<ObjectId>{value.map(|v|v.as_reference().unwrap())}

#[test]
fn composition_form_defaults_keeps_both_font_lookup_paths_and_shared_program_provenance(){
    let f=fixture();let before=f.doc.objects.clone();let inventory=inspect_fixture(&f).unwrap();
    assert_eq!(inventory.programs.len(),1,"field default parsed once for field and both widgets");
    for (index,widget) in f.widgets.iter().enumerate(){
        let node=&inventory.nodes[widget];assert_eq!(node.control_da,Some(Origin::Field(f.field)));assert_eq!(node.fill_da,node.control_da);
        assert_eq!(node.alignment.unwrap().origin,Origin::Field(f.field));assert_eq!(node.alignment.unwrap().value,&Object::Integer(2));
        let fonts=&node.fonts[b"F1".as_slice()];assert_eq!(id(fonts.field),Some(f.fonts[2]));assert_eq!(id(fonts.form),Some(f.fonts[0]));assert_eq!(id(fonts.normal),Some(f.fonts[index+1]));assert_eq!(id(fonts.page),Some(f.fonts[3]));
        assert_eq!(node.normal_resources.unwrap().origin,Origin::Appearance(f.appearances[index]));
    }
    let raw=f.doc.get_dictionary(f.field).unwrap().get(b"DA").unwrap();assert!(std::ptr::eq(inventory.programs[&Origin::Field(f.field)].raw,raw));assert_eq!(before,f.doc.objects);
}

#[test]
fn composition_form_defaults_empty_and_null_control_settings_keep_distinct_fill_fallback(){
    for value in [Object::Null,Object::string_literal("")] {
        let mut f=fixture();f.doc.get_dictionary_mut(f.widgets[0]).unwrap().set("DA",value);f.doc.get_dictionary_mut(f.widgets[0]).unwrap().set("Q",Object::Null);
        let inventory=inspect_fixture(&f).unwrap();let node=&inventory.nodes[&f.widgets[0]];
        assert_eq!(node.control_da,Some(Origin::Field(f.widgets[0])));assert_eq!(node.fill_da,Some(Origin::Form(f.pages.catalog)));
        assert!(inventory.programs[&node.control_da.unwrap()].bytes.is_empty());assert!(!inventory.programs[&node.fill_da.unwrap()].bytes.is_empty());
        assert_eq!(node.alignment.unwrap().origin,Origin::Field(f.widgets[0]));assert_eq!(node.alignment.unwrap().value,&Object::Null);
    }
    let mut f=fixture();f.doc.get_dictionary_mut(f.field).unwrap().remove(b"DA");
    let inventory=inspect_fixture(&f).unwrap();assert_eq!(inventory.nodes[&f.widgets[0]].control_da,Some(Origin::Form(f.pages.catalog)));
}

#[test]
fn composition_form_defaults_missing_appearance_resources_are_not_fabricated_from_page_fonts(){
    let mut f=fixture();f.doc.get_object_mut(f.appearances[0]).unwrap().as_stream_mut().unwrap().dict.remove(b"Resources");
    f.doc.get_dictionary_mut(f.widgets[0]).unwrap().set("DR",Object::Null);
    let inventory=inspect_fixture(&f).unwrap();let node=&inventory.nodes[&f.widgets[0]];let fonts=&node.fonts[b"F1".as_slice()];
    assert!(node.normal_resources.is_none());assert!(fonts.normal.is_none() && fonts.field.is_none());assert_eq!(id(fonts.page),Some(f.fonts[3]));assert_eq!(id(fonts.form),Some(f.fonts[0]));
    assert_eq!(node.field_resources.unwrap().value,&Object::Null);
}

#[test]
fn composition_form_defaults_parses_escaped_font_names_and_refuses_malformed_programs(){
    let mut f=fixture();f.doc.get_dictionary_mut(f.field).unwrap().set("DA",Object::string_literal("% /Wrong 12 Tf\n /F#31 12 Tf /F1 10 Tf 0 g"));
    let inventory=inspect_fixture(&f).unwrap();assert_eq!(inventory.programs[&Origin::Field(f.field)].font_names,vec![b"F1".to_vec(),b"F1".to_vec()]);
    for program in ["/F1 Tf","/F1 -2 Tf","12 10 Tf","/F1 99999999999999999999999999999999999999999999999999 Tf","(unclosed"] {
        f.doc.get_dictionary_mut(f.field).unwrap().set("DA",Object::string_literal(program));assert!(inspect_fixture(&f).is_err(),"{program}");
    }
}

#[test]
fn composition_form_defaults_limits_and_cancellation_preserve_source_payloads(){
    let mut f=fixture();let forms=model(&f);let before=f.doc.objects.clone();let mut calls=0;
    inspect(&f.doc,&f.pages,&forms,&mut Budget::default(),Arc::new(||false),&mut ||{calls+=1;Ok(())}).unwrap();
    for stop in 1..=calls {let mut count=0;assert!(inspect(&f.doc,&f.pages,&forms,&mut Budget::default(),Arc::new(||false),&mut ||{count+=1;if count==stop{Err(EngineError::InvalidRequest("cancel".into()))}else{Ok(())}}).is_err());}
    assert!(inspect(&f.doc,&f.pages,&forms,&mut Budget::default(),Arc::new(||true),&mut ||Ok(())).is_err());
    assert!(inspect(&f.doc,&f.pages,&forms,&mut Budget{metadata:1,..Default::default()},Arc::new(||false),&mut ||Ok(())).is_err());assert_eq!(before,f.doc.objects);
    f.doc.get_dictionary_mut(f.field).unwrap().set("DA",Object::string_literal(vec![b' ';65537]));assert!(inspect_fixture(&f).is_err());
}

#[test]
fn composition_form_defaults_indirect_defaults_direct_form_and_page_inheritance_keep_source_edges(){
    let mut f=fixture();
    let da=f.doc.add_object(Object::string_literal("/F1 12 Tf 0 g"));
    let da_alias=f.doc.add_object(Object::Reference(da));
    f.doc.get_dictionary_mut(f.field).unwrap().set("DA",da_alias);
    let resources=f.doc.get_dictionary(f.pages.pages[0]).unwrap().get(b"Resources").unwrap().clone();
    let resources=f.doc.add_object(resources);let alias=f.doc.add_object(Object::Reference(resources));
    f.doc.get_dictionary_mut(f.pages.root).unwrap().set("Resources",alias);
    for page in &f.pages.pages {f.doc.get_dictionary_mut(*page).unwrap().remove(b"Resources");}
    let ap_alias=f.doc.add_object(Object::Reference(f.appearances[0]));
    f.doc.get_dictionary_mut(f.widgets[0]).unwrap().set("AP",dictionary!{"N"=>ap_alias});
    let form=f.doc.get_dictionary(f.form).unwrap().clone();
    f.doc.get_dictionary_mut(f.pages.catalog).unwrap().set("AcroForm",form);
    let before=f.doc.objects.clone();let inventory=inspect_fixture(&f).unwrap();
    let program=&inventory.programs[&Origin::Field(f.field)];
    assert_eq!(program.raw,&Object::Reference(da_alias));assert_eq!(program.bytes,b"/F1 12 Tf 0 g");
    let node=&inventory.nodes[&f.widgets[0]];
    assert_eq!(node.page_resources.unwrap().origin,Origin::Page(f.pages.root));
    assert_eq!(node.page_resources.unwrap().value,&Object::Reference(alias));
    assert_eq!(node.normal_resources.unwrap().origin,Origin::Appearance(f.appearances[0]));
    assert_eq!(id(node.fonts[b"F1".as_slice()].page),Some(f.fonts[3]));
    assert_eq!(before,f.doc.objects);
}

#[test]
fn composition_form_defaults_cumulative_parser_and_metadata_limits_apply_across_sources(){
    let f=fixture();let forms=model(&f);let before=f.doc.objects.clone();
    let mut budget=Budget::default();budget.parser.input_bytes=b"/F1 12 Tf 0 g".len();
    inspect(&f.doc,&f.pages,&forms,&mut budget,Arc::new(||false),&mut ||Ok(())).unwrap();
    assert_eq!(budget.parser.input_bytes,0,"shared inherited default is parsed once");
    assert!(inspect(&f.doc,&f.pages,&forms,&mut budget,Arc::new(||false),&mut ||Ok(())).is_err());
    let mut measured=Budget::default();let available=measured.metadata;
    inspect(&f.doc,&f.pages,&forms,&mut measured,Arc::new(||false),&mut ||Ok(())).unwrap();
    let mut exact=Budget{metadata:available-measured.metadata,..Default::default()};
    inspect(&f.doc,&f.pages,&forms,&mut exact,Arc::new(||false),&mut ||Ok(())).unwrap();
    assert_eq!(exact.metadata,0);
    assert!(inspect(&f.doc,&f.pages,&forms,&mut exact,Arc::new(||false),&mut ||Ok(())).is_err());
    let mut no_steps=Budget{steps:100_000,..Default::default()};
    assert!(inspect(&f.doc,&f.pages,&forms,&mut no_steps,Arc::new(||false),&mut ||Ok(())).is_err());
    assert_eq!(before,f.doc.objects);
}

#[test]
fn composition_form_defaults_refuses_malformed_resources_and_does_not_choose_a_button_state(){
    for kind in 0..5 {
        let mut f=fixture();
        match kind {
            0=>{f.doc.get_dictionary_mut(f.field).unwrap().set("DR",42);},
            1=>{f.doc.get_dictionary_mut(f.field).unwrap().set("DR",dictionary!{"Font"=>"not-a-dictionary"});},
            2=>{let cycle=f.doc.new_object_id();f.doc.objects.insert(cycle,Object::Reference(cycle));f.doc.get_dictionary_mut(f.field).unwrap().set("DA",cycle);},
            3=>{f.doc.get_dictionary_mut(f.widgets[0]).unwrap().set("AP",dictionary!{"N"=>42});},
            _=>{f.doc.get_dictionary_mut(f.field).unwrap().set("Q",3);},
        }
        let before=f.doc.objects.clone();assert!(inspect_fixture(&f).is_err(),"case {kind}");assert_eq!(before,f.doc.objects);
    }
    let mut f=fixture();
    f.doc.get_dictionary_mut(f.widgets[0]).unwrap().set("AP",dictionary!{"N"=>dictionary!{"Off"=>f.appearances[0],"Yes"=>f.appearances[1]}});
    let inventory=inspect_fixture(&f).unwrap();
    assert!(inventory.nodes[&f.widgets[0]].normal_resources.is_none());
    assert!(inventory.nodes[&f.widgets[0]].fonts[b"F1".as_slice()].normal.is_none());
}

#[test]
fn composition_form_defaults_missing_widget_p_does_not_invent_control_font_fallback(){
    for explicit_null in [false,true] {
        let mut f=fixture();
        let widget=f.doc.get_dictionary_mut(f.widgets[0]).unwrap();
        if explicit_null {widget.set("P",Object::Null);} else {widget.remove(b"P");}
        let inventory=inspect_fixture(&f).unwrap();let node=&inventory.nodes[&f.widgets[0]];
        assert!(node.page_resources.is_some(),"physical page resources still govern appearance rendering");
        assert!(node.fonts[b"F1".as_slice()].page.is_none(),"native control lookup requires widget P; annotation membership alone does not provide it");
        assert_eq!(id(inventory.nodes[&f.widgets[1]].fonts[b"F1".as_slice()].page),Some(f.fonts[3]));
    }
}

#[test]
fn composition_form_defaults_ownership_defaults_and_namespace_share_one_budget(){
    let f=fixture();let before=f.doc.objects.clone();let mut budget=Budget::default();let available=budget.metadata;
    let forms=crate::composition_forms::inspect(&f.doc,&f.pages,&mut budget,&mut ||Ok(())).unwrap();
    let after_ownership=budget.metadata;
    inspect(&f.doc,&f.pages,&forms,&mut budget,Arc::new(||false),&mut ||Ok(())).unwrap();
    let after_defaults=budget.metadata;
    crate::composition_form_assembly::namespace(&[&forms],&mut budget,&mut ||Ok(())).unwrap();
    assert!(available>after_ownership && after_ownership>after_defaults && after_defaults>budget.metadata);
    assert_eq!(budget.nodes,3);assert_eq!(budget.parser.input_bytes,16*1024*1024-b"/F1 12 Tf 0 g".len());
    let mut short=Budget{metadata:available-budget.metadata-1,..Default::default()};
    let forms=crate::composition_forms::inspect(&f.doc,&f.pages,&mut short,&mut ||Ok(())).unwrap();
    inspect(&f.doc,&f.pages,&forms,&mut short,Arc::new(||false),&mut ||Ok(())).unwrap();
    assert!(crate::composition_form_assembly::namespace(&[&forms],&mut short,&mut ||Ok(())).is_err(),"the final stage cannot reset earlier metadata usage");
    assert_eq!(before,f.doc.objects);
}
