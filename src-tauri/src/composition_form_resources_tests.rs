use super::*;
use lopdf::{dictionary,Object,Stream};
use crate::{composition_pages::SourcePages,composition_forms::FormInfo};
use std::sync::Arc;
fn source(doc:&mut Document,dr:Object,da:&str)->(SourcePages,FormInfo) {
    let tree=doc.new_object_id();
    let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()]});
    let field=doc.add_object(dictionary!{"T"=>Object::string_literal("answer"),"FT"=>"Tx","DA"=>Object::string_literal(da)});
    let form=doc.add_object(dictionary!{"Fields"=>vec![Object::Reference(field)],"DR"=>dr});
    doc.objects.insert(tree,dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}.into());
    let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree,"AcroForm"=>form});
    let pages=crate::composition_pages::inspect(doc,catalog,&mut ||Ok(())).unwrap();
    let forms=crate::composition_forms::inspect(doc,&pages,&mut Budget::default(),&mut ||Ok(())).unwrap();(pages,forms)
}
fn defaults<'a>(doc:&'a Document,source:&(SourcePages,FormInfo))->Inventory<'a> {
    crate::composition_form_defaults::inspect(doc,&source.0,&source.1,&mut Budget::default(),Arc::new(||false),&mut ||Ok(())).unwrap()
}
fn fonts(result:&Resources)->&Dictionary {result.dictionary.get(b"Font").unwrap().as_dict().unwrap()}
fn model(doc:&Document,inputs:&[(SourcePages,FormInfo)])->EngineResult<Resources> {
    let models=inputs.iter().map(|input|defaults(doc,input)).collect::<Vec<_>>();
    let sources=inputs.iter().zip(&models).map(|(input,defaults)|Source{catalog:input.0.catalog,defaults}).collect::<Vec<_>>();
    stage(doc,&sources,&mut Budget::default(),&mut ||Ok(()))
}

#[test]
fn composition_form_resources_stage_collisions_without_copying_font_payloads(){
    let mut doc=Document::with_version("1.7");let payload=doc.add_object(Stream::new(dictionary!{},vec![7;1024*1024]));
    let a=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica","FontFile"=>payload});
    let b=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
    let c=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Times-Roman"});
    let inputs=vec![
        source(&mut doc,dictionary!{"Font"=>dictionary!{"F1"=>a},"ProcSet"=>vec![Object::Name(b"PDF".to_vec())]}.into(),"/F1 12 Tf 0 g"),
        source(&mut doc,dictionary!{"Font"=>dictionary!{"F1"=>b},"ColorSpace"=>dictionary!{"CS"=>"DeviceRGB"},"ProcSet"=>vec![Object::Name(b"Text".to_vec())]}.into(),"/F1 10 Tf 1 0 0 rg"),
        source(&mut doc,dictionary!{"Font"=>dictionary!{"Folio2_F1"=>c}}.into(),"/Folio2_F1 9 Tf"),
    ];
    let before=doc.objects.clone();let pointer=doc.get_object(payload).unwrap().as_stream().unwrap().content.as_ptr();
    let result=model(&doc,&inputs).unwrap();
    assert_eq!(result.font_names[0][b"F1".as_slice()],b"F1");assert_eq!(result.font_names[1][b"F1".as_slice()],b"Folio2_F2");
    assert_eq!(result.font_names[2][b"Folio2_F1".as_slice()],b"Folio2_F1");
    assert_eq!(fonts(&result).get(b"F1").unwrap(),&Object::Reference(a));assert_eq!(fonts(&result).get(b"Folio2_F2").unwrap(),&Object::Reference(b));
    assert_eq!(fonts(&result).get(b"Folio2_F1").unwrap(),&Object::Reference(c));
    assert_eq!(result.dictionary.get(b"ColorSpace").unwrap().as_dict().unwrap().get(b"CS").unwrap().as_name().unwrap(),b"DeviceRGB");
    assert_eq!(result.dictionary.get(b"ProcSet").unwrap().as_array().unwrap(),&vec![Object::Name(b"PDF".to_vec()),Object::Name(b"Text".to_vec())]);
    assert_eq!(before,doc.objects);assert_eq!(pointer,doc.get_object(payload).unwrap().as_stream().unwrap().content.as_ptr());
}

#[test]
fn composition_form_resources_missing_global_fonts_do_not_acquire_another_sources_binding(){
    let mut doc=Document::with_version("1.7");let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let inputs=vec![source(&mut doc,dictionary!{}.into(),"/F1 12 Tf"),source(&mut doc,dictionary!{"Font"=>dictionary!{"F1"=>font}}.into(),"/F1 12 Tf")];
    let result=model(&doc,&inputs).unwrap();
    assert_eq!(result.font_names[0][b"F1".as_slice()],b"F1");assert!(!fonts(&result).has(b"F1"));
    assert_eq!(fonts(&result).get(&result.font_names[1][b"F1".as_slice()]).unwrap(),&Object::Reference(font));
}

#[test]
fn composition_form_resources_nonfont_collisions_require_compatible_identity_or_values(){
    let mut doc=Document::with_version("1.7");let gs=doc.add_object(dictionary!{"ca"=>0.5});let alias=doc.add_object(Object::Reference(gs));
    let inputs=vec![
        source(&mut doc,dictionary!{"ExtGState"=>dictionary!{"G"=>gs},"ColorSpace"=>dictionary!{"C"=>"DeviceRGB"},"ProcSet"=>vec![Object::Name(b"PDF".to_vec())]}.into(),""),
        source(&mut doc,dictionary!{"ExtGState"=>dictionary!{"G"=>alias},"ColorSpace"=>dictionary!{"C"=>"DeviceRGB"},"ProcSet"=>vec![Object::Name(b"PDF".to_vec())]}.into(),""),
    ];
    let result=model(&doc,&inputs).unwrap();assert_eq!(result.dictionary.get(b"ExtGState").unwrap().as_dict().unwrap().get(b"G").unwrap(),&Object::Reference(gs));
    assert_eq!(result.dictionary.get(b"ProcSet").unwrap().as_array().unwrap().len(),1);
    let other=doc.add_object(dictionary!{"ca"=>0.5});
    let form=doc.get_dictionary(inputs[1].0.catalog).unwrap().get(b"AcroForm").unwrap().as_reference().unwrap();
    doc.get_dictionary_mut(form).unwrap().set("DR",dictionary!{"ExtGState"=>dictionary!{"G"=>other}});
    let before=doc.objects.clone();assert!(model(&doc,&inputs).is_err(),"different indirect identities are not deduplicated by similarity");assert_eq!(before,doc.objects);
}

#[test]
fn composition_form_resources_limits_cancellation_and_malformed_containers_refuse(){
    let mut doc=Document::with_version("1.7");let input=source(&mut doc,dictionary!{"Font"=>dictionary!{"F1"=>dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"}}}.into(),"/F1 12 Tf");
    let inventory=defaults(&doc,&input);let sources=[Source{catalog:input.0.catalog,defaults:&inventory}];
    let before=doc.objects.clone();let mut calls=0;
    stage(&doc,&sources,&mut Budget::default(),&mut ||{calls+=1;Ok(())}).unwrap();
    for stop in 1..=calls {let mut count=0;assert!(stage(&doc,&sources,&mut Budget::default(),&mut ||{count+=1;if count==stop{Err(EngineError::InvalidRequest("cancel".into()))}else{Ok(())}}).is_err());}
    assert!(stage(&doc,&sources,&mut Budget{metadata:1,..Default::default()},&mut ||Ok(())).is_err());assert_eq!(before,doc.objects);
    drop(inventory);
    let form=doc.get_dictionary(input.0.catalog).unwrap().get(b"AcroForm").unwrap().as_reference().unwrap();
    for dr in [Object::Integer(42),dictionary!{"Font"=>42}.into(),dictionary!{"ExtGState"=>42}.into(),dictionary!{"ProcSet"=>vec![Object::Integer(42)]}.into(),dictionary!{"Unknown"=>dictionary!{}}.into()] {
        doc.get_dictionary_mut(form).unwrap().set("DR",dr);
        // Empty provenance is sufficient for container validation; malformed
        // resources may also have been rejected by the earlier inspector.
        assert!(stage(&doc,&[Source{catalog:input.0.catalog,defaults:&Inventory::default()}],&mut Budget::default(),&mut ||Ok(())).is_err());
    }
}

#[test]
fn composition_form_resources_binary_font_names_and_shared_pipeline_budget(){
    let mut doc=Document::with_version("1.7");let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let key=vec![0x80,0xfe];let mut font_dict=Dictionary::new();font_dict.set(key.clone(),font);
    let inputs=vec![source(&mut doc,dictionary!{"Font"=>font_dict.clone()}.into(),"/#80#FE 12 Tf"),source(&mut doc,dictionary!{"Font"=>font_dict}.into(),"/#80#FE 12 Tf")];
    let result=model(&doc,&inputs).unwrap();assert_eq!(result.font_names[0][&key],key);
    assert!(result.font_names[1][&key].is_ascii());assert_eq!(fonts(&result).get(&result.font_names[1][&key]).unwrap(),&Object::Reference(font));
    let pipeline=|budget:&mut Budget,expect_resources:bool| {
        let models=inputs.iter().map(|input|crate::composition_forms::inspect(&doc,&input.0,budget,&mut ||Ok(())).unwrap()).collect::<Vec<_>>();
        let defaults=inputs.iter().zip(&models).map(|(input,model)|crate::composition_form_defaults::inspect(&doc,&input.0,model,budget,Arc::new(||false),&mut ||Ok(())).unwrap()).collect::<Vec<_>>();
        crate::composition_form_assembly::namespace(&models.iter().collect::<Vec<_>>(),budget,&mut ||Ok(())).unwrap();
        let sources=inputs.iter().zip(&defaults).map(|(input,defaults)|Source{catalog:input.0.catalog,defaults}).collect::<Vec<_>>();
        assert_eq!(stage(&doc,&sources,budget,&mut ||Ok(())).is_ok(),expect_resources);
    };
    let mut measured=Budget::default();let available=measured.metadata;pipeline(&mut measured,true);
    let mut short=Budget{metadata:available-measured.metadata-1,..Default::default()};pipeline(&mut short,false);
}

#[test]
fn composition_form_resources_repeated_large_names_charge_comparison_work(){
    for procset in [false,true] {
        let mut doc=Document::with_version("1.7");let name=vec![b'A';1024*1024];
        let resources=if procset {dictionary!{"ProcSet"=>vec![Object::Name(name)]}}else{
            let image=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image"},vec![0]));
            let mut entries=Dictionary::new();entries.set(name,image);dictionary!{"XObject"=>entries}
        };
        let resources=doc.add_object(resources);
        let inputs=(0..40).map(|_|source(&mut doc,Object::Reference(resources),"")).collect::<Vec<_>>();
        let before=doc.objects.clone();
        assert!(model(&doc,&inputs).is_err(),"repeated shared large names must not bypass cumulative comparison bounds: ProcSet={procset}");
        assert_eq!(before,doc.objects);
    }
}

#[test]
fn composition_form_resources_unbound_original_names_take_priority_in_every_source_order(){
    let mut doc=Document::with_version("1.7");let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let null=doc.add_object(Object::Null);
    let inputs=[
        source(&mut doc,dictionary!{"Font"=>dictionary!{"F1"=>font}}.into(),"/F1 12 Tf"),
        source(&mut doc,dictionary!{}.into(),"/F1 12 Tf"),
        source(&mut doc,dictionary!{"Font"=>dictionary!{"F1"=>null}}.into(),"/F1 12 Tf"),
    ];
    let models=inputs.iter().map(|input|defaults(&doc,input)).collect::<Vec<_>>();let before=doc.objects.clone();
    for order in [[0,1,2],[1,0,2],[2,1,0]] {
        let sources=order.iter().map(|index|Source{catalog:inputs[*index].0.catalog,defaults:&models[*index]}).collect::<Vec<_>>();
        let result=stage(&doc,&sources,&mut Budget::default(),&mut ||Ok(())).unwrap();
        assert!(!fonts(&result).has(b"F1"),"original missing name must stay globally unbound: {order:?}");
        for (position,source) in order.into_iter().enumerate() {
            let name=&result.font_names[position][b"F1".as_slice()];
            if source==0 {assert_ne!(name,b"F1");assert_eq!(fonts(&result).get(name).unwrap(),&Object::Reference(font));}
            else {assert_eq!(name,b"F1","field/page/AP-only scopes should not need an alias");}
        }
    }
    assert_eq!(before,doc.objects);
}

#[test]
fn composition_form_resources_fresh_aliases_respect_local_reserved_names(){
    let mut doc=Document::with_version("1.7");let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let inputs=[source(&mut doc,dictionary!{"Font"=>dictionary!{"F1"=>font}}.into(),"/F1 12 Tf"),source(&mut doc,dictionary!{"Font"=>dictionary!{"F1"=>font}}.into(),"/F1 12 Tf")];
    let models=inputs.iter().map(|input|defaults(&doc,input)).collect::<Vec<_>>();
    let sources=inputs.iter().zip(&models).map(|(input,defaults)|Source{catalog:input.0.catalog,defaults}).collect::<Vec<_>>();
    let forbidden=[b"F1".to_vec(),b"Folio2_F1".to_vec(),b"Folio2_F2".to_vec()].into();
    let result=stage_reserved(&doc,&sources,&forbidden,&mut Budget::default(),&mut ||Ok(())).unwrap();
    assert_eq!(result.font_names[0][b"F1".as_slice()],b"F1","an unchanged source name is not a new alias");
    assert_eq!(result.font_names[1][b"F1".as_slice()],b"Folio2_F3");
    assert_eq!(fonts(&result).get(b"Folio2_F3").unwrap(),&Object::Reference(font));
}

#[test]
fn composition_form_resources_avoid_names_used_only_in_nested_content(){
    let mut doc=Document::with_version("1.7");
    let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let inputs=[source(&mut doc,dictionary!{"Font"=>dictionary!{"F1"=>font}}.into(),"/F1 12 Tf"),source(&mut doc,dictionary!{"Font"=>dictionary!{"F1"=>font}}.into(),"/F1 12 Tf")];
    // No Font table defines this name. Adding it could turn existing invisible
    // text into painted text, so the content operand itself reserves the name.
    let nested=doc.add_object(Stream::new(dictionary!{"Subtype"=>"Form"},b"/Folio2_F1 10 Tf (existing) Tj".to_vec()));
    doc.objects.get_mut(&inputs[1].0.catalog).unwrap().as_dict_mut().unwrap().set("RetainedForm",nested);
    let before=doc.objects.clone();let mut budget=Budget::default();
    let models=inputs.iter().map(|input|defaults(&doc,input)).collect::<Vec<_>>();
    let sources=inputs.iter().zip(&models).map(|(input,defaults)|Source{catalog:input.0.catalog,defaults}).collect::<Vec<_>>();
    let roots=inputs.iter().map(|input|input.0.catalog).collect::<Vec<_>>();
    let forbidden=crate::composition_form_scope::reserve_names(&doc,&roots,&mut budget,Arc::new(||false),&mut ||Ok(())).unwrap();
    let result=stage_reserved(&doc,&sources,&forbidden,&mut budget,&mut ||Ok(())).unwrap();
    assert_eq!(result.font_names[1][b"F1".as_slice()],b"Folio2_F2");
    assert!(!fonts(&result).has(b"Folio2_F1"));assert_eq!(before,doc.objects);
}
