use super::*;
use lopdf::{dictionary,Object,Stream};

struct Fixture {doc:Document,pages:SourcePages,form:ObjectId,group:ObjectId,field:ObjectId,widgets:[ObjectId;2],hidden:ObjectId,appearance:ObjectId}
fn fixture()->Fixture {
    let mut doc=Document::with_version("1.7");let tree=doc.new_object_id();let group=doc.new_object_id();let field=doc.new_object_id();
    let appearance=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),20.into()]},vec![b' ';1024*1024]));
    let mut pages=vec![];let mut widgets=vec![];
    for _ in 0..2 {
        let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary!{}});
        let widget=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","Parent"=>field,"P"=>page,"Rect"=>vec![20.into(),20.into(),120.into(),40.into()],"AP"=>dictionary!{"N"=>appearance}});
        doc.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(widget)]);pages.push(page);widgets.push(widget);
    }
    doc.objects.insert(field,dictionary!{"Parent"=>group,"T"=>Object::string_literal("name"),"FT"=>"Tx","V"=>Object::string_literal("Ada"),"Kids"=>widgets.iter().copied().map(Object::Reference).collect::<Vec<_>>()}.into());
    doc.objects.insert(group,dictionary!{"T"=>Object::string_literal("person"),"Kids"=>vec![Object::Reference(field)]}.into());
    let hidden=doc.add_object(dictionary!{"T"=>Object::string_literal("internal"),"FT"=>"Tx","V"=>Object::string_literal("kept")});
    let form=doc.add_object(dictionary!{"Fields"=>vec![Object::Reference(group),hidden.into()],"CO"=>vec![Object::Reference(hidden),field.into()],"NeedAppearances"=>false});
    doc.objects.insert(tree,dictionary!{"Type"=>"Pages","Count"=>2,"Kids"=>pages.iter().copied().map(Object::Reference).collect::<Vec<_>>()}.into());
    let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree,"AcroForm"=>form});doc.trailer.set("Root",catalog);
    let pages=crate::composition_pages::inspect(&doc,catalog,&mut ||Ok(())).unwrap();
    Fixture{doc,pages,form,group,field,widgets:[widgets[0],widgets[1]],hidden,appearance}
}
fn inspect_fixture(f:&Fixture)->EngineResult<FormInfo>{inspect(&f.doc,&f.pages,&mut Budget::default(),&mut ||Ok(()))}

#[test]
fn composition_forms_inventory_retains_shared_answer_and_nonvisual_field_without_payload_copies(){
    let f=fixture();let before=f.doc.objects.clone();let pointer=f.doc.get_object(f.appearance).unwrap().as_stream().unwrap().content.as_ptr();
    let mut budget=Budget::default();let info=inspect(&f.doc,&f.pages,&mut budget,&mut ||Ok(())).unwrap();
    assert!(info.present);assert_eq!(info.form_id,Some(f.form));assert_eq!(info.roots,vec![f.group,f.hidden]);assert_eq!(info.nodes.len(),5);
    assert_eq!(info.calculation_order,vec![f.hidden,f.field]);assert!(!info.need_appearances);
    assert_eq!(info.nodes[&f.field].full_name,"person.name");assert_eq!(info.nodes[&f.field].kind,Some(FieldType::Text));
    for (index,widget) in f.widgets.iter().enumerate(){assert_eq!(info.nodes[widget].owner,f.field);assert!(!info.nodes[widget].field);assert_eq!(info.nodes[widget].widget_page,Some(f.pages.pages[index]));}
    assert_eq!(info.nodes[&f.hidden].widget_page,None);assert_eq!(info.nodes[&f.hidden].owner,f.hidden);
    assert!(budget.metadata>15*1024*1024,"appearance bytes must not be copied or charged as metadata");
    assert_eq!(f.doc.objects,before);assert_eq!(pointer,f.doc.get_object(f.appearance).unwrap().as_stream().unwrap().content.as_ptr());
}

#[test]
fn composition_forms_inventory_resolves_aliases_and_preserves_merged_widget_identity(){
    let mut f=fixture();
    let field_alias=f.doc.add_object(Object::Reference(f.field));let page_alias=f.doc.add_object(Object::Reference(f.pages.pages[0]));
    let group_alias=f.doc.add_object(Object::Reference(f.group));let widget_alias=f.doc.add_object(Object::Reference(f.widgets[0]));
    f.doc.get_dictionary_mut(f.group).unwrap().set("Kids",vec![Object::Reference(field_alias)]);
    f.doc.get_dictionary_mut(f.field).unwrap().set("Parent",group_alias);
    f.doc.get_dictionary_mut(f.field).unwrap().set("Kids",vec![Object::Reference(widget_alias),f.widgets[1].into()]);
    f.doc.get_dictionary_mut(f.widgets[0]).unwrap().set("Parent",field_alias);f.doc.get_dictionary_mut(f.widgets[0]).unwrap().set("P",page_alias);
    let subtype=f.doc.add_object(Object::Name(b"Widget".to_vec()));f.doc.get_dictionary_mut(f.widgets[0]).unwrap().set("Subtype",subtype);
    let merged=f.doc.add_object(dictionary!{"Subtype"=>"Widget","FT"=>"Btn","T"=>Object::string_literal("agree"),"P"=>page_alias});
    f.doc.get_dictionary_mut(f.pages.pages[0]).unwrap().set("Annots",vec![Object::Reference(widget_alias),merged.into()]);
    f.doc.get_dictionary_mut(f.form).unwrap().set("Fields",vec![Object::Reference(group_alias),f.hidden.into(),merged.into()]);
    f.doc.get_dictionary_mut(f.form).unwrap().set("CO",vec![Object::Reference(field_alias)]);
    let info=inspect_fixture(&f).unwrap();assert_eq!(info.roots[0],f.group);assert_eq!(info.calculation_order,vec![f.field]);
    assert!(info.nodes[&merged].field);assert_eq!(info.nodes[&merged].owner,merged);assert_eq!(info.nodes[&merged].kind,Some(FieldType::Button));assert!(!info.nodes.contains_key(&field_alias));
}

#[test]
fn composition_forms_inventory_distinguishes_absent_empty_and_null_optional_forms(){
    let mut f=fixture();for p in &f.pages.pages{f.doc.get_dictionary_mut(*p).unwrap().remove(b"Annots");}
    f.doc.get_dictionary_mut(f.form).unwrap().set("Fields",Object::Null);f.doc.get_dictionary_mut(f.form).unwrap().set("CO",Object::Null);f.doc.get_dictionary_mut(f.form).unwrap().set("NeedAppearances",Object::Null);
    let info=inspect_fixture(&f).unwrap();assert!(info.present && info.nodes.is_empty() && !info.need_appearances);
    f.doc.get_dictionary_mut(f.pages.catalog).unwrap().set("AcroForm",Object::Null);assert!(!inspect_fixture(&f).unwrap().present);
    f.doc.get_dictionary_mut(f.pages.catalog).unwrap().remove(b"AcroForm");assert!(!inspect_fixture(&f).unwrap().present);
}

#[test]
fn composition_forms_inventory_refuses_inconsistent_ownership_and_signed_or_dynamic_forms(){
    for mode in ["orphan","missing-widget","wrong-page","two-pages","repeat-slot","missing-parent","wrong-parent","cycle","alias-cycle","mixed","widget-kids","bad-co","signed","xfa"] {
        let mut f=fixture();
        match mode {
            "orphan"=>{f.doc.get_dictionary_mut(f.form).unwrap().set("Fields",vec![Object::Reference(f.hidden)]);},
            "missing-widget"=>{f.doc.get_dictionary_mut(f.pages.pages[0]).unwrap().remove(b"Annots");},
            "wrong-page"=>{f.doc.get_dictionary_mut(f.widgets[0]).unwrap().set("P",f.pages.pages[1]);},
            "two-pages"=>{f.doc.get_dictionary_mut(f.pages.pages[1]).unwrap().set("Annots",vec![Object::Reference(f.widgets[0])]);},
            "repeat-slot"=>{f.doc.get_dictionary_mut(f.pages.pages[0]).unwrap().set("Annots",vec![Object::Reference(f.widgets[0]),f.widgets[0].into()]);},
            "missing-parent"=>{f.doc.get_dictionary_mut(f.widgets[0]).unwrap().remove(b"Parent");},
            "wrong-parent"=>{f.doc.get_dictionary_mut(f.widgets[0]).unwrap().set("Parent",f.group);},
            "cycle"=>{f.doc.get_dictionary_mut(f.group).unwrap().set("Kids",vec![Object::Reference(f.group)]);},
            "alias-cycle"=>{let alias=f.doc.new_object_id();f.doc.objects.insert(alias,Object::Reference(alias));f.doc.get_dictionary_mut(f.form).unwrap().set("Fields",vec![Object::Reference(alias)]);},
            "mixed"=>{let child=f.doc.add_object(dictionary!{"Parent"=>f.field,"T"=>Object::string_literal("child"),"FT"=>"Tx"});f.doc.get_dictionary_mut(f.field).unwrap().set("Kids",vec![Object::Reference(child),f.widgets[0].into(),f.widgets[1].into()]);},
            "widget-kids"=>{f.doc.get_dictionary_mut(f.widgets[0]).unwrap().set("Kids",Vec::<Object>::new());},
            "bad-co"=>{f.doc.get_dictionary_mut(f.form).unwrap().set("CO",vec![Object::Reference(f.widgets[0])]);},
            "signed"=>{f.doc.get_dictionary_mut(f.hidden).unwrap().set("FT","Sig");f.doc.get_dictionary_mut(f.hidden).unwrap().set("V",dictionary!{});},
            "xfa"=>{f.doc.get_dictionary_mut(f.form).unwrap().set("XFA",Object::string_literal("dynamic"));},
            _=>unreachable!(),
        }
        let before=f.doc.objects.clone();assert!(inspect_fixture(&f).is_err(),"{mode}");assert_eq!(before,f.doc.objects,"{mode}");
    }
}

#[test]
fn composition_forms_inventory_canonical_names_detect_encoding_collisions_and_bad_utf16(){
    for value in [Object::string_literal("person.name"),Object::String(b"\xef\xbb\xbfperson.name".to_vec(),lopdf::StringFormat::Hexadecimal),lopdf::text_string("person.name"),Object::String(vec![0xfe,0xff,0x00],lopdf::StringFormat::Hexadecimal)] {
        let mut f=fixture();f.doc.get_dictionary_mut(f.hidden).unwrap().set("T",value);assert!(inspect_fixture(&f).is_err());
    }
    let mut f=fixture();f.doc.get_dictionary_mut(f.hidden).unwrap().set("T",lopdf::text_string("café"));assert_eq!(inspect_fixture(&f).unwrap().nodes[&f.hidden].full_name,"café");
}

#[test]
fn composition_forms_inventory_budgets_are_cumulative_and_cancellation_does_not_mutate(){
    let f=fixture();let mut budget=Budget{nodes:3995,..Default::default()};inspect(&f.doc,&f.pages,&mut budget,&mut ||Ok(())).unwrap();assert_eq!(budget.nodes,4000);
    assert!(inspect(&f.doc,&f.pages,&mut budget,&mut ||Ok(())).is_err());
    for mut budget in [Budget{metadata:1,..Default::default()},Budget{steps:99_999,..Default::default()}]{assert!(inspect(&f.doc,&f.pages,&mut budget,&mut ||Ok(())).is_err());}
    let mut steps=0;inspect(&f.doc,&f.pages,&mut Budget::default(),&mut ||{steps+=1;Ok(())}).unwrap();
    let before=f.doc.objects.clone();
    for stop in 1..=steps {let mut seen=0;assert!(inspect(&f.doc,&f.pages,&mut Budget::default(),&mut ||{seen+=1;if seen==stop{Err(EngineError::InvalidRequest("cancel".into()))}else{Ok(())}}).is_err(),"checkpoint {stop}");}
    assert_eq!(f.doc.objects,before);
}

#[test]
fn composition_forms_inventory_distinguishes_inherited_types_null_barriers_and_unsigned_signatures(){
    for (name,kind) in [("Tx",FieldType::Text),("Btn",FieldType::Button),("Ch",FieldType::Choice),("Sig",FieldType::Signature)] {
        let mut f=fixture();let value=f.doc.add_object(Object::Name(name.as_bytes().to_vec()));
        f.doc.get_dictionary_mut(f.group).unwrap().set("FT",value);f.doc.get_dictionary_mut(f.field).unwrap().remove(b"FT");
        if name=="Sig"{f.doc.get_dictionary_mut(f.field).unwrap().set("V",Object::Null);}
        f.doc.get_dictionary_mut(f.widgets[0]).unwrap().remove(b"P");
        let info=inspect_fixture(&f).unwrap();assert_eq!(info.nodes[&f.field].kind,Some(kind));assert_eq!(info.nodes[&f.widgets[0]].kind,Some(kind));
        f.doc.get_dictionary_mut(f.field).unwrap().set("FT",Object::Null);assert!(inspect_fixture(&f).is_err(),"explicit null must not inherit {name}");
    }
}

#[test]
fn composition_review_signature_value_inherits_from_untyped_ancestor(){
 for visual in [false,true]{for local in 0..3{
  let mut f=fixture();let signature=f.doc.add_object(dictionary!{"Type"=>"Sig","Contents"=>Object::string_literal("signed bytes")});let alias=f.doc.add_object(Object::Reference(signature));
  f.doc.get_dictionary_mut(f.group).unwrap().set("V",alias);
  let parent_alias=f.doc.add_object(Object::Reference(f.group));let null=f.doc.add_object(Object::Null);
  let child=f.doc.get_dictionary_mut(f.field).unwrap();child.set("FT","Sig");child.set("Parent",parent_alias);child.remove(b"V");
  if local==1{child.set("V",Object::Null);}if local==2{child.set("V",null);}
  if !visual{child.remove(b"Kids");for page in &f.pages.pages{f.doc.get_dictionary_mut(*page).unwrap().remove(b"Annots");}}
  let before=f.doc.objects.clone();let error=inspect_fixture(&f).err().expect("inherited signature value must be refused").to_string();assert!(error.contains("signed form"),"{error}");assert_eq!(f.doc.objects,before);
  // The same hierarchy is unsigned when the ancestor has no effective value.
  f.doc.get_dictionary_mut(f.group).unwrap().set("V",null);assert_eq!(inspect_fixture(&f).unwrap().nodes[&f.field].kind,Some(FieldType::Signature));
 }}
}
