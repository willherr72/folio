use super::*;
use lopdf::{dictionary,Stream};

struct Fixture{doc:Document,page:ObjectId,annotation:ObjectId,form:ObjectId,resources:ObjectId,font:ObjectId,image:ObjectId,alias:ObjectId,cycle:ObjectId}
fn fixture()->Fixture {
    let mut doc=Document::with_version("1.7");let page=doc.new_object_id();let annotation=doc.new_object_id();
    let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let image=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image"},vec![8;1024*1024]));
    let form=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","StructParents"=>2},b"/P <</MCID 0>> BDC EMC".to_vec()));
    let alias=doc.add_object(Object::Reference(form));let cycle=doc.new_object_id();
    let resources=doc.add_object(dictionary!{"Font"=>dictionary!{"F1"=>font},"XObject"=>dictionary!{"Fm"=>alias,"Again"=>alias,"Image"=>image},"Cycle"=>cycle});
    doc.objects.insert(cycle,dictionary!{"Back"=>resources,"Owner"=>form}.into());
    doc.objects.insert(annotation,dictionary!{"Subtype"=>"Text","P"=>page,"AP"=>dictionary!{"N"=>form}}.into());
    doc.objects.insert(page,dictionary!{"Type"=>"Page","Resources"=>resources,"Annots"=>vec![Object::Reference(annotation)],"Dest"=>vec![Object::Reference(page),"Fit".into()]}.into());
    Fixture{doc,page,annotation,form,resources,font,image,alias,cycle}
}
#[test]
fn composition_copy_isolates_owned_alias_cycles_without_copying_immutable_payloads() {
    let mut f=fixture();let original=f.doc.objects.clone();let mut budget=Budget::new(&f.doc,&mut ||Ok(())).unwrap();
    let map=copy(&mut f.doc,&BTreeMap::from([(f.page,false),(f.annotation,true),(f.form,true)]),&BTreeSet::from([f.page]),&Fields::new(),&mut budget,&mut ||Ok(())).unwrap();
    for id in [f.page,f.annotation,f.form,f.resources,f.alias,f.cycle] {assert_ne!(map[&id],id);assert_eq!(f.doc.objects[&id],original[&id]);}
    assert!(!map.contains_key(&f.font));assert!(!map.contains_key(&f.image));
    assert_eq!(f.doc.objects[&map[&f.alias]],Object::Reference(map[&f.form]));
    let resources=f.doc.get_dictionary(map[&f.resources]).unwrap();let x=resources.get(b"XObject").unwrap().as_dict().unwrap();
    assert_eq!(x.get(b"Fm").unwrap(),x.get(b"Again").unwrap());assert_eq!(x.get(b"Image").unwrap(),&Object::Reference(f.image));
    assert_eq!(f.doc.get_dictionary(map[&f.cycle]).unwrap().get(b"Back").unwrap(),&Object::Reference(map[&f.resources]));
    assert_eq!(f.doc.get_dictionary(map[&f.annotation]).unwrap().get(b"P").unwrap(),&Object::Reference(f.page),"typed ownership must patch Page edges explicitly");
    assert_eq!(f.doc.get_dictionary(map[&f.page]).unwrap().get(b"Dest").unwrap().as_array().unwrap()[0],Object::Reference(f.page),"generic copying must not retarget navigation");
    assert_eq!(f.doc.objects[&map[&f.form]].as_stream().unwrap().content,original[&f.form].as_stream().unwrap().content);
}

#[test]
fn composition_copy_uses_effective_overrides_and_preserves_boundary_references() {
    let mut f=fixture();let mut budget=Budget::new(&f.doc,&mut ||Ok(())).unwrap();
    let fields=BTreeMap::from([(f.page,BTreeMap::from([(b"Annots".to_vec(),None),(b"New".to_vec(),Some(Object::Integer(17)))]))]);
    let map=copy(&mut f.doc,&BTreeMap::from([(f.page,false)]),&BTreeSet::from([f.page,f.resources]),&fields,&mut budget,&mut ||Ok(())).unwrap();
    assert_eq!(map.len(),1);let page=f.doc.get_dictionary(map[&f.page]).unwrap();
    assert!(!page.has(b"Annots"));assert_eq!(page.get(b"New").unwrap(),&Object::Integer(17));assert_eq!(page.get(b"Resources").unwrap(),&Object::Reference(f.resources));
    assert!(f.doc.get_dictionary(f.page).unwrap().has(b"Annots"));
}

#[test]
fn composition_copy_cumulative_stored_bytes_refuse_before_commit() {
    let mut f=fixture();let mut budget=Budget::new(&f.doc,&mut ||Ok(())).unwrap();
    let original=f.doc.objects.clone();let max=f.doc.max_id;
    budget.max_bytes=budget.usage.bytes+f.doc.objects[&f.image].as_stream().unwrap().content.len()-1;
    let before=budget;
    assert!(copy(&mut f.doc,&BTreeMap::from([(f.image,true)]),&BTreeSet::new(),&Fields::new(),&mut budget,&mut ||Ok(())).is_err());
    assert_eq!(f.doc.objects,original);assert_eq!(f.doc.max_id,max);assert_eq!(budget,before);
    budget.max_bytes=budget.usage.bytes+f.doc.objects[&f.image].as_stream().unwrap().content.len()+200;
    copy(&mut f.doc,&BTreeMap::from([(f.image,true)]),&BTreeSet::new(),&Fields::new(),&mut budget,&mut ||Ok(())).unwrap();
    let after=f.doc.objects.clone();let before=budget;
    assert!(copy(&mut f.doc,&BTreeMap::from([(f.image,true)]),&BTreeSet::new(),&Fields::new(),&mut budget,&mut ||Ok(())).is_err());
    assert_eq!(f.doc.objects,after);assert_eq!(budget,before);
}

#[test]
fn composition_copy_every_cancellation_checkpoint_preserves_document_and_budget() {
    let f=fixture();let initial=Budget::new(&f.doc,&mut ||Ok(())).unwrap();let seeds=BTreeMap::from([(f.page,false),(f.annotation,true),(f.form,true)]);
    let bounds=BTreeSet::from([f.page]);let mut calls=0;let mut doc=f.doc.clone();let mut budget=initial;
    copy(&mut doc,&seeds,&bounds,&Fields::new(),&mut budget,&mut ||{calls+=1;Ok(())}).unwrap();
    assert!(calls>100);
    for stop in 1..=calls {
        let mut doc=f.doc.clone();let mut budget=initial;let mut current=0;
        assert!(copy(&mut doc,&seeds,&bounds,&Fields::new(),&mut budget,&mut ||{current+=1;if current==stop{Err(bad("cancel"))}else{Ok(())}}).is_err());
        assert_eq!(doc.objects,f.doc.objects,"checkpoint {stop}");assert_eq!(doc.max_id,f.doc.max_id);assert_eq!(budget,initial);
    }
}
