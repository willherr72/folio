use super::*;
use crate::{composition_form_defaults,composition_forms,composition_pages};
use lopdf::{dictionary,Stream};
struct Fixture {doc:Document,root:ObjectId,tree:ObjectId,page:ObjectId,form:ObjectId,field:ObjectId,widget:ObjectId,ap:ObjectId,fonts:[ObjectId;4]}
fn fixture()->Fixture {
    let mut doc=Document::with_version("1.7");let tree=doc.new_object_id();let field=doc.new_object_id();
    let fonts=["Helvetica","Courier","Times-Roman","Helvetica-Bold"].map(|name|doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>name}));
    let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()]});
    let ap=doc.add_object(Stream::new(dictionary!{"Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),20.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>fonts[2]}}},b"BT /F1 10 Tf (Answer) Tj ET".to_vec()));
    let widget=doc.add_object(dictionary!{"Subtype"=>"Widget","Parent"=>field,"P"=>page,"Rect"=>vec![0.into(),0.into(),100.into(),20.into()],"AP"=>dictionary!{"N"=>ap}});
    doc.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(widget)]);
    doc.objects.insert(field,dictionary!{"T"=>Object::string_literal("answer"),"FT"=>"Tx","V"=>Object::string_literal("Answer"),"DR"=>dictionary!{"Font"=>dictionary!{"F1"=>fonts[1]}},"Kids"=>vec![Object::Reference(widget)]}.into());
    let form=doc.add_object(dictionary!{"Fields"=>vec![Object::Reference(field)],"DA"=>Object::string_literal("% keep\n/F1 10.123456 Tf 0 g"),"Q"=>2,"DR"=>dictionary!{"Font"=>dictionary!{"F1"=>fonts[0]}}});
    doc.objects.insert(tree,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>fonts[3]}}}.into());
    let root=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree,"AcroForm"=>form});doc.trailer.set("Root",root);
    Fixture{doc,root,tree,page,form,field,widget,ap,fonts}
}
fn plan(f:&Fixture,budget:&mut Budget)->EngineResult<Patches> {
    let pages=composition_pages::inspect(&f.doc,f.root,&mut ||Ok(()))?;
    let forms=composition_forms::inspect(&f.doc,&pages,budget,&mut ||Ok(()))?;
    let defaults=composition_form_defaults::inspect(&f.doc,&pages,&forms,budget,Arc::new(||false),&mut ||Ok(()))?;
    let forbidden=crate::composition_form_scope::reserve_names(&f.doc,&[f.root],budget,Arc::new(||false),&mut ||Ok(()))?;
    let aliases=[(b"F1".to_vec(),b"FreshFont".to_vec())].into();
    stage(&f.doc,&[Source{forms:&forms,defaults:&defaults,aliases:&aliases}],&forbidden,budget,Arc::new(||false),&mut ||Ok(()))
}
fn font(doc:&Document,id:ObjectId,key:&[u8],name:&[u8])->ObjectId {
    let object=doc.get_object(id).unwrap();let dict=match object{Object::Stream(s)=>&s.dict,_=>object.as_dict().unwrap()};
    doc.dereference(dict.get(key).unwrap()).unwrap().1.as_dict().unwrap().get(b"Font").unwrap().as_dict().unwrap().get(name).unwrap().as_reference().unwrap()
}

#[test]
fn composition_form_patch_preserves_independent_scopes_and_materializes_page_and_field_defaults() {
    let f=fixture();let before=f.doc.objects.clone();let mut budget=Budget::default();let patches=plan(&f,&mut budget).unwrap();
    assert!(patches.global_da.is_none());assert_eq!(before,f.doc.objects);
    let ap_pointer=f.doc.get_object(f.ap).unwrap().as_stream().unwrap().content.as_ptr();
    let output=apply(f.doc,patches,&mut budget,&mut ||Ok(())).unwrap();
    assert_eq!(output.get_dictionary(f.field).unwrap().get(b"DA").unwrap().as_str().unwrap(),b"% keep\n/FreshFont 10.123456 Tf 0 g");
    assert_eq!(output.get_dictionary(f.field).unwrap().get(b"Q").unwrap(),&Object::Integer(2));
    assert_eq!(font(&output,f.field,b"DR",b"FreshFont"),f.fonts[1]);
    assert_eq!(font(&output,f.ap,b"Resources",b"FreshFont"),f.fonts[2]);
    assert_eq!(font(&output,f.page,b"Resources",b"FreshFont"),f.fonts[3]);
    for (id,key,n) in [(f.field,b"DR".as_slice(),1),(f.ap,b"Resources",2),(f.page,b"Resources",3)]{assert_eq!(font(&output,id,key,b"F1"),f.fonts[n]);}
    assert_eq!(output.objects[&f.tree],before[&f.tree],"shared page-tree resources stay untouched");
    assert_eq!(output.objects[&f.widget],before[&f.widget]);assert_eq!(output.objects[&f.form],before[&f.form]);
    assert_eq!(output.get_object(f.ap).unwrap().as_stream().unwrap().content.as_ptr(),ap_pointer,"do not copy appearance payloads");
}

#[test]
fn composition_form_patch_keeps_explicit_empty_controls_and_their_global_fill_fallback() {
    for value in [Object::Null,Object::string_literal("")] {
        let mut f=fixture();f.doc.get_dictionary_mut(f.widget).unwrap().set("DA",value.clone());
        f.doc.get_dictionary_mut(f.widget).unwrap().set("Q",Object::Null);
        let mut budget=Budget::default();let patches=plan(&f,&mut budget).unwrap();
        assert_eq!(patches.global_da.as_ref().unwrap().as_str().unwrap(),b"% keep\n/FreshFont 10.123456 Tf 0 g");
        let output=apply(f.doc,patches,&mut budget,&mut ||Ok(())).unwrap();
        assert_eq!(output.get_dictionary(f.widget).unwrap().get(b"DA").unwrap(),&value);
        assert_eq!(output.get_dictionary(f.widget).unwrap().get(b"Q").unwrap(),&Object::Null);
    }
}

#[test]
fn composition_form_patch_keeps_identity_da_edges_and_missing_local_resources() {
    let mut f=fixture();let da=f.doc.add_object(Object::string_literal("/F1 10 Tf"));
    f.doc.get_dictionary_mut(f.field).unwrap().set("DA",da);
    f.doc.get_dictionary_mut(f.field).unwrap().set("DR",Object::Null);
    f.doc.get_object_mut(f.ap).unwrap().as_stream_mut().unwrap().dict.remove(b"Resources");
    let pages=composition_pages::inspect(&f.doc,f.root,&mut ||Ok(())).unwrap();let mut budget=Budget::default();
    let forms=composition_forms::inspect(&f.doc,&pages,&mut budget,&mut ||Ok(())).unwrap();
    let defaults=composition_form_defaults::inspect(&f.doc,&pages,&forms,&mut budget,Arc::new(||false),&mut ||Ok(())).unwrap();
    let aliases=[(b"F1".to_vec(),b"F1".to_vec())].into();
    let patches=stage(&f.doc,&[Source{forms:&forms,defaults:&defaults,aliases:&aliases}],&BTreeSet::new(),&mut budget,Arc::new(||false),&mut ||Ok(())).unwrap();
    assert!(!patches.entries.contains_key(&(f.field,b"DA".to_vec())));
    assert!(!patches.entries.keys().any(|(_,key)|key==b"Resources" || key==b"DR"));
    let output=apply(f.doc,patches,&mut budget,&mut ||Ok(())).unwrap();
    assert_eq!(output.get_dictionary(f.field).unwrap().get(b"DA").unwrap(),&Object::Reference(da));
    assert_eq!(output.get_dictionary(f.field).unwrap().get(b"DR").unwrap(),&Object::Null);
    assert!(!output.get_object(f.ap).unwrap().as_stream().unwrap().dict.has(b"Resources"));
}

#[test]
fn composition_form_patch_bounds_work_and_cancels_before_consuming_original() {
    let f=fixture();let before=f.doc.objects.clone();let mut budget=Budget::default();let patches=plan(&f,&mut budget).unwrap();
    let mut calls=0;apply(f.doc.clone(),patches,&mut Budget::default(),&mut ||{calls+=1;Ok(())}).unwrap();assert!(calls>0);
    for stop in 1..=calls {
        let patches=plan(&f,&mut Budget::default()).unwrap();let mut at=0;
        assert!(apply(f.doc.clone(),patches,&mut Budget::default(),&mut ||{at+=1;if at==stop{Err(EngineError::InvalidRequest("cancel".into()))}else{Ok(())}}).is_err());
    }
    assert!(plan(&f,&mut Budget{metadata:1,..Default::default()}).is_err());assert_eq!(before,f.doc.objects);
}

fn another_source(doc:&mut Document,field_da:Option<Object>,global_da:Option<Object>)->(ObjectId,ObjectId) {
    let tree=doc.new_object_id();let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()]});
    doc.objects.insert(tree,dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}.into());
    let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
    let mut field=dictionary!{"T"=>Object::string_literal("other"),"FT"=>"Tx","V"=>Object::string_literal("kept")};
    if let Some(da)=field_da {field.set("DA",da);}let field=doc.add_object(field);
    let mut form=dictionary!{"Fields"=>vec![Object::Reference(field)],"DR"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}};
    if let Some(da)=global_da {form.set("DA",da);}
    let form=doc.add_object(form);
    (doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree,"AcroForm"=>form}),field)
}
fn combined(doc:&Document,roots:&[ObjectId])->EngineResult<Patches> {
    let mut budget=Budget::default();
    let pages=roots.iter().map(|root|composition_pages::inspect(doc,*root,&mut ||Ok(()))).collect::<EngineResult<Vec<_>>>()?;
    let forms=pages.iter().map(|pages|composition_forms::inspect(doc,pages,&mut budget,&mut ||Ok(()))).collect::<EngineResult<Vec<_>>>()?;
    let defaults=pages.iter().zip(&forms).map(|(pages,forms)|composition_form_defaults::inspect(doc,pages,forms,&mut budget,Arc::new(||false),&mut ||Ok(()))).collect::<EngineResult<Vec<_>>>()?;
    let forbidden=crate::composition_form_scope::reserve_names(doc,roots,&mut budget,Arc::new(||false),&mut ||Ok(()))?;
    let resource_sources=roots.iter().zip(&defaults).map(|(root,defaults)|crate::composition_form_resources::Source{catalog:*root,defaults}).collect::<Vec<_>>();
    let resources=crate::composition_form_resources::stage_reserved(doc,&resource_sources,&forbidden,&mut budget,&mut ||Ok(()))?;
    let sources=forms.iter().zip(&defaults).zip(&resources.font_names).map(|((forms,defaults),aliases)|Source{forms,defaults,aliases}).collect::<Vec<_>>();
    stage(doc,&sources,&forbidden,&mut budget,Arc::new(||false),&mut ||Ok(()))
}

#[test]
fn composition_form_patch_combined_sources_preserve_a_required_global_fallback_in_either_order() {
    let mut f=fixture();f.doc.get_dictionary_mut(f.widget).unwrap().set("DA",Object::Null);
    let (other,field)=another_source(&mut f.doc,Some(Object::string_literal("/F1 8 Tf")),Some(Object::string_literal("/F1 19 Tf")));
    for roots in [[f.root,other],[other,f.root]] {
        let patches=combined(&f.doc,&roots).unwrap();
        let name=if roots[0]==f.root{"F1"}else{"Folio2_F1"};
        assert_eq!(patches.global_da.as_ref().unwrap().as_str().unwrap(),format!("% keep\n/{name} 10.123456 Tf 0 g").as_bytes());
        let output=apply(f.doc.clone(),patches,&mut Budget::default(),&mut ||Ok(())).unwrap();
        assert_eq!(output.get_dictionary(f.widget).unwrap().get(b"DA").unwrap(),&Object::Null);
        let name=if roots[0]==other{"F1"}else{"Folio2_F1"};
        assert_eq!(output.get_dictionary(field).unwrap().get(b"DA").unwrap().as_str().unwrap(),format!("/{name} 8 Tf").as_bytes());
        assert_eq!(output.get_dictionary(field).unwrap().get(b"V").unwrap().as_str().unwrap(),b"kept");
    }
}

#[test]
fn composition_form_patch_refuses_conflicting_empty_field_fallbacks_and_preserves_absence() {
    for absent in [false,true] {
        let mut f=fixture();f.doc.get_dictionary_mut(f.widget).unwrap().set("DA",Object::string_literal(""));
        let (other,_)=another_source(&mut f.doc,if absent{None}else{Some(Object::Null)},if absent{None}else{Some(Object::string_literal("/F1 19 Tf"))});
        let before=f.doc.objects.clone();
        for roots in [[f.root,other],[other,f.root]] {
            let error=combined(&f.doc,&roots).err().expect("conflicting fallback must refuse");
            assert!(format!("{error:?}").contains("incompatible global fallbacks"));
        }
        assert_eq!(before,f.doc.objects);
    }
    let mut f=fixture();f.doc.get_dictionary_mut(f.form).unwrap().remove(b"DA");
    let (other,_)=another_source(&mut f.doc,Some(Object::Null),Some(Object::string_literal("")));
    assert!(combined(&f.doc,&[f.root,other]).unwrap().global_da.is_none());
}

#[test]
fn composition_form_patch_rechecks_alias_freshness_and_stage_cancellation() {
    let f=fixture();let mut budget=Budget::default();
    let pages=composition_pages::inspect(&f.doc,f.root,&mut ||Ok(())).unwrap();
    let forms=composition_forms::inspect(&f.doc,&pages,&mut budget,&mut ||Ok(())).unwrap();
    let defaults=composition_form_defaults::inspect(&f.doc,&pages,&forms,&mut budget,Arc::new(||false),&mut ||Ok(())).unwrap();
    let aliases=[(b"F1".to_vec(),b"FreshFont".to_vec())].into();
    let inputs=[Source{forms:&forms,defaults:&defaults,aliases:&aliases}];
    let forbidden=[b"FreshFont".to_vec()].into();
    assert!(stage(&f.doc,&inputs,&forbidden,&mut Budget::default(),Arc::new(||false),&mut ||Ok(())).is_err());
    assert!(stage(&f.doc,&inputs,&BTreeSet::new(),&mut Budget::default(),Arc::new(||true),&mut ||Ok(())).is_err());
    let mut calls=0;stage(&f.doc,&inputs,&BTreeSet::new(),&mut Budget::default(),Arc::new(||false),&mut ||{calls+=1;Ok(())}).unwrap();
    for stop in [1,calls/2,calls] {
        let mut at=0;
        assert!(stage(&f.doc,&inputs,&BTreeSet::new(),&mut Budget::default(),Arc::new(||false),&mut ||{at+=1;if at==stop{Err(EngineError::InvalidRequest("cancel".into()))}else{Ok(())}}).is_err());
    }
}
