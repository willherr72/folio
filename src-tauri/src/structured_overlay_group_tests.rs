use super::*;

#[test]
fn structured_overlay_flatten_content_page_groups_refuse_unproved_compositing() {
    for case in ["gray","cmyk","named","array","knockout","future","wrong-type","wrong-kind","invalid-isolation","invalid-knockout","not-dictionary","alias-cycle","default-rgb"] {
        let (mut doc,page,form)=minimal(b"q 0 0 1 rg 0 0 200 200 re f Q");
        let mut group=dictionary!{"S"=>"Transparency","CS"=>"DeviceRGB"};
        match case {
            "gray"=>group.set("CS","DeviceGray"),"cmyk"=>group.set("CS","DeviceCMYK"),"named"=>group.set("CS","OtherBlend"),
            "array"=>group.set("CS",vec![Object::Name(b"CalRGB".to_vec()),dictionary!{}.into()]),
            "knockout"=>group.set("K",true),"future"=>group.set("FutureCompositingMode",true),
            "wrong-type"=>group.set("Type","Other"),"wrong-kind"=>group.set("S","Other"),
            "invalid-isolation"=>group.set("I",1),"invalid-knockout"=>group.set("K",1),_=>{},
        }
        let value=if case=="alias-cycle"{let a=doc.new_object_id();let b=doc.add_object(Object::Reference(a));doc.objects.insert(a,Object::Reference(b));Object::Reference(a)}
            else if case=="not-dictionary"{1.into()}else{Object::Dictionary(group)};
        doc.get_dictionary_mut(page).unwrap().set("Group",value);
        if case=="default-rgb"{doc.get_dictionary_mut(page).unwrap().set("Resources",dictionary!{"ColorSpace"=>dictionary!{"DefaultRGB"=>vec![Object::Name(b"CalRGB".to_vec()),dictionary!{"WhitePoint"=>vec![1.into(),1.into(),1.into()]}.into()]}});}
        let original=doc.objects.clone();
        assert!(append(doc.clone(),&[PageForms{page,forms:&[form]}],&mut ||Ok(())).is_err(),"unproved page compositing must refuse: {case}");
        let unchanged=append(doc.clone(),&[PageForms{page,forms:&[]}],&mut ||Ok(())).unwrap();assert_eq!(unchanged.objects,original,"empty publication must not reject or alter {case}");
        assert_eq!(doc.objects,original);
    }
}

#[test]
fn structured_overlay_flatten_content_page_groups_preserve_admitted_aliases_and_defaults() {
    for case in ["absent","null","minimal","rgb","aliased"] {
        let (mut doc,page,form)=minimal(b"q 0 0 1 rg 0 0 200 200 re f Q");
        match case {
            "absent"=>{},"null"=>doc.get_dictionary_mut(page).unwrap().set("Group",Object::Null),
            "minimal"=>doc.get_dictionary_mut(page).unwrap().set("Group",dictionary!{"S"=>"Transparency"}),
            "rgb"=>doc.get_dictionary_mut(page).unwrap().set("Group",dictionary!{"Type"=>"Group","S"=>"Transparency","CS"=>"DeviceRGB","I"=>true,"K"=>false}),
            "aliased"=>{
                let cs=doc.add_object(Object::Name(b"DeviceRGB".to_vec()));let flag=doc.add_object(Object::Boolean(false));let s=doc.add_object(Object::Name(b"Transparency".to_vec()));
                let group=doc.add_object(dictionary!{"S"=>s,"CS"=>cs,"I"=>flag,"K"=>Object::Null});let alias=doc.add_object(Object::Reference(group));doc.get_dictionary_mut(page).unwrap().set("Group",alias);
            },_=>unreachable!(),
        }
        let original=doc.clone();let saved=append(doc,&[PageForms{page,forms:&[form]}],&mut ||Ok(())).unwrap();
        assert_eq!(saved.get_dictionary(page).unwrap().get(b"Group").ok(),original.get_dictionary(page).unwrap().get(b"Group").ok());
        for (id,value) in original.objects {if id!=page{assert_eq!(&saved.objects[&id],&value);}}
    }
}

#[test]
fn structured_overlay_flatten_content_page_groups_match_saved_rgb_appearances() {
    for rotation in [0,90,180,270]{for unit in [1.,1.7]{for aliased in [false,true]{
        let mut rt=fixtures::runtime();let seed=fixtures::source(&mut rt,rotation,unit,b"q /Translucent gs 0.2 0.4 0.8 rg -20 30 320 420 re f Q");
        let mut original=crate::pdf_output::load(rt.document(&seed.id).unwrap().bytes().unwrap().as_ref()).unwrap();let page=original.get_pages()[&1];
        original.get_dictionary_mut(page).unwrap().set("Resources",dictionary!{"ExtGState"=>dictionary!{"Translucent"=>dictionary!{"ca"=>0.6f32,"BM"=>"Multiply"}}});
        let group=dictionary!{"Type"=>"Group","S"=>"Transparency","CS"=>"DeviceRGB","I"=>aliased,"K"=>false};
        let group=if aliased{let root=original.add_object(group);Object::Reference(original.add_object(Object::Reference(root)))}else{Object::Dictionary(group)};
        original.get_dictionary_mut(page).unwrap().set("Group",group);let bytes=crate::pdf_output::save(&mut original).unwrap();
        let path=std::env::temp_dir().join(format!("folio-group-source-{}.pdf",Uuid::new_v4()));let info=rt.open_bytes(bytes.clone(),path.clone(),path,true,true,false,OpenSecurity::Restored(None)).unwrap();
        let request=fixtures::request(&rt,&info);let prepared=rt.prepare_structured_overlays(&request,&mut ||Ok(())).unwrap();
        let target=TargetPage{page_id:page,geometry:prepared.pages[0].geometry,unit};let attached=attach_generated(original,&[target],prepared,&mut ||Ok(())).unwrap();let mut doc=attached.document;
        let ids=attached.pages[0].overlays.iter().filter(|o|["text","ink","image"].contains(&o.overlay_id.as_str())).map(|o|o.annotation_id).collect::<Vec<_>>();
        let forms=ids.iter().map(|id|doc.get_dictionary(*id).unwrap().get(b"AP").unwrap().as_dict().unwrap().get(b"N").unwrap().as_reference().unwrap()).collect::<Vec<_>>();
        doc.get_dictionary_mut(page).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().retain(|v|!ids.contains(&v.as_reference().unwrap()));
        let before=doc.clone();let mut reference=doc.clone();super::super::super::persistence::legacy_flatten_reference(&mut reference,page,&forms).unwrap();
        let reference=crate::pdf_output::save(&mut reference).unwrap();let mut output=append(doc,&[PageForms{page,forms:&forms}],&mut ||Ok(())).unwrap();
        for (id,value) in before.objects {if id!=page{assert_eq!(output.objects[&id],value);}}
        let actual=crate::pdf_output::save(&mut output).unwrap();let label=format!("flatten-group-{rotation}-{unit}-{aliased}");
        fixtures::same_pixels(&rt,&actual,&reference,&label);
        if let Some(root)=std::env::var_os("FOLIO_STRUCTURED_OVERLAY_CAPTURE_DIR") {use std::io::Write;let root=PathBuf::from(root);std::fs::OpenOptions::new().write(true).create_new(true).open(root.join(format!("{label}-source.pdf"))).unwrap().write_all(&bytes).unwrap();}
    }}}
}
