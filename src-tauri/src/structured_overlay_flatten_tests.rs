use super::*;
use super::super::{structured_overlay_render::tests as fixtures,structured_overlay_publish::{attach_generated,TargetPage}};
use lopdf::{dictionary,Object,Stream};

#[path="structured_overlay_group_tests.rs"]
mod groups;

#[test]
fn composition_annotation_flatten_actual_drawings_keep_semantics_through_typed_removal(){
    use crate::composition_annotation_additions::{SourceLanguage,NewOwners,FlattenOwner,mark_flattened};
    use crate::composition_annotation_receipts::{Anchor,Receipt};
    use crate::composition_annotation_removal::{Removal,remove_owned};
    for rotation in [0,90,180,270]{for unit in [1.,1.7]{
        let mut rt=fixtures::runtime();let info=fixtures::source(&mut rt,rotation,unit,b"q 0.2 0.4 0.8 rg 160 300 20 20 re f Q\nq 0 0 10 10 re W Q");let request=fixtures::request(&rt,&info);
        let prepared=rt.prepare_structured_overlays(&request,&mut ||Ok(())).unwrap();let doc=crate::pdf_output::load(rt.document(&info.id).unwrap().bytes().unwrap().as_ref()).unwrap();let page=doc.get_pages()[&1];let target=TargetPage{page_id:page,geometry:prepared.pages[0].geometry,unit};
        let attached=attach_generated(doc,&[target],prepared,&mut ||Ok(())).unwrap();let mut doc=attached.document;let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        doc.get_dictionary_mut(catalog).unwrap().set("Lang",Object::string_literal("fr-CA"));let lang=SourceLanguage::capture(&doc,catalog).unwrap();
        let ids=attached.pages[0].overlays.iter().map(|o|o.annotation_id).collect::<Vec<_>>();
        doc=crate::composition_annotation_additions::append(doc,&[NewOwners{page,annotations:&ids,language:&lang}],&mut ||Ok(())).unwrap();
        let before=crate::pdf_output::save(&mut doc.clone()).unwrap();let original=crate::composition_structure::inspect(&doc,catalog,&[page],&mut ||Ok(())).unwrap();
        let drawings=attached.pages[0].overlays.iter().filter(|o|["text","ink","image"].contains(&o.overlay_id.as_str())).map(|o|o.annotation_id).collect::<Vec<_>>();
        let forms=drawings.iter().map(|id|doc.get_dictionary(*id).unwrap().get(b"AP").unwrap().as_dict().unwrap().get(b"N").unwrap().as_reference().unwrap()).collect::<Vec<_>>();
        let requests=drawings.iter().zip(&forms).map(|(annotation,appearance)|FlattenOwner{page,annotation:*annotation,appearance:*appearance}).collect::<Vec<_>>();
        let receipts=doc.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap().iter().enumerate().filter_map(|(slot,v)|{let annotation=v.as_reference().unwrap();drawings.contains(&annotation).then_some(Receipt{origin:Anchor{page,slot,annotation},output_page:page,output_slot:slot,annotation})}).collect::<Vec<_>>();
        let removals=receipts.iter().map(|receipt|Removal{annotation:*receipt,popups:&[]}).collect::<Vec<_>>();
        let mut legacy=doc.clone();legacy.get_dictionary_mut(page).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().retain(|v|!drawings.contains(&v.as_reference().unwrap()));
        super::super::persistence::legacy_flatten_reference(&mut legacy,page,&forms).unwrap();let legacy=crate::pdf_output::save(&mut legacy).unwrap();
        let marked=mark_flattened(doc,&requests,&mut ||Ok(())).unwrap();let output=append(marked.document,&[PageForms{page,forms:&marked.forms}],&mut ||Ok(())).unwrap();let mut output=remove_owned(output,&removals,&mut ||Ok(())).unwrap();
        let structure=crate::composition_structure::inspect(&output,catalog,&[page],&mut ||Ok(())).unwrap();assert_eq!(structure.top,original.top);assert_eq!(structure.elements,original.elements);assert_eq!(structure.owners.len(),5);assert_eq!(output.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap().len(),2);
        let output=crate::pdf_output::save(&mut output).unwrap();let reopened=crate::pdf_output::load(&output).unwrap();assert_eq!(crate::composition_structure::inspect(&reopened,catalog,&[page],&mut ||Ok(())).unwrap(),structure);
        if let Some(root)=std::env::var_os("FOLIO_STRUCTURED_OVERLAY_CAPTURE_DIR"){
            use std::io::Write;let directory=PathBuf::from(root);std::fs::create_dir_all(&directory).unwrap();
            for (kind,bytes) in [("before",before.as_slice()),("after",output.as_slice()),("legacy",legacy.as_slice())]{std::fs::OpenOptions::new().write(true).create_new(true).open(directory.join(format!("flatten-owner-{rotation}-{unit}-{kind}.pdf"))).unwrap().write_all(bytes).unwrap();}
        }
        let a=fixtures::pixels(&rt,&output,0);let b=fixtures::pixels(&rt,&legacy,0);assert_eq!((a.0,a.1),(b.0,b.1));assert_eq!(a.2.iter().zip(&b.2).filter(|(a,b)|a!=b).count(),0,"{rotation}/{unit}");
    }}
}

#[test]
fn structured_overlay_flatten_content_isolates_pending_paths_and_clips_in_physical_frames(){
    let endings=[b"".as_slice(),b"0 0 10 10 re W n",b"0 0 m 100 100 l",b"0 0 10 10 re W",b"q 0 0 10 10 re W Q"];
    for rotation in [0,90,180,270]{for unit in [1.,1.7]{for (case,ending) in endings.iter().enumerate(){
        let mut rt=fixtures::runtime();let mut content=b"q 0.2 0.4 0.8 rg 160 300 20 20 re f Q\n".to_vec();content.extend_from_slice(ending);
        let info=fixtures::source(&mut rt,rotation,unit,&content);let request=fixtures::request(&rt,&info);
        let prepared=rt.prepare_structured_overlays(&request,&mut ||Ok(())).unwrap();let mut doc=crate::pdf_output::load(rt.document(&info.id).unwrap().bytes().unwrap().as_ref()).unwrap();let page=doc.get_pages()[&1];
        let old_contents=doc.get_dictionary(page).unwrap().get(b"Contents").unwrap().as_reference().unwrap();let original=doc.objects[&old_contents].clone();
        let note=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","P"=>page,"Rect"=>vec![220.into(),300.into(),240.into(),320.into()],"Contents"=>Object::string_literal("Original note")});doc.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(note)]);
        let note_before=doc.objects[&note].clone();let target=TargetPage{page_id:page,geometry:prepared.pages[0].geometry,unit};let attached=attach_generated(doc,&[target],prepared,&mut ||Ok(())).unwrap();
        let drawings=attached.pages[0].overlays.iter().filter(|o|["text","ink","image"].contains(&o.overlay_id.as_str())).map(|o|o.annotation_id).collect::<Vec<_>>();assert_eq!(drawings.len(),3);
        let mut reference=attached.document.clone();
        // Flattened drawing content is beneath all native annotations. The
        // attachment primitive alone appends roots and does not impose this
        // final presentation order (the existing note overlaps at rotation270).
        let list=reference.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap();let mut order=drawings.iter().map(|id|Object::Reference(*id)).collect::<Vec<_>>();order.extend(list.iter().filter(|v|!drawings.contains(&v.as_reference().unwrap())).cloned());reference.get_dictionary_mut(page).unwrap().set("Annots",order);
        let reference=crate::pdf_output::save(&mut reference).unwrap();let mut doc=attached.document;
        let forms=drawings.iter().map(|id|doc.get_dictionary(*id).unwrap().get(b"AP").unwrap().as_dict().unwrap().get(b"N").unwrap().as_reference().unwrap()).collect::<Vec<_>>();
        doc.get_dictionary_mut(page).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().retain(|v|!drawings.contains(&v.as_reference().unwrap()));
        let mut legacy=doc.clone();super::super::persistence::legacy_flatten_reference(&mut legacy,page,&forms).unwrap();let legacy=crate::pdf_output::save(&mut legacy).unwrap();
        let mut output=append(doc,&[PageForms{page,forms:&forms}],&mut ||Ok(())).unwrap();
        assert_eq!(output.objects[&old_contents],original);assert_eq!(output.objects[&note],note_before);assert_eq!(output.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap().len(),3);
        let output=crate::pdf_output::save(&mut output).unwrap();
        if let Some(root)=std::env::var_os("FOLIO_STRUCTURED_OVERLAY_CAPTURE_DIR"){
            use std::io::Write;let directory=PathBuf::from(root);std::fs::create_dir_all(&directory).unwrap();
            for (kind,bytes) in [("reference",reference.as_slice()),("after",output.as_slice()),("legacy",legacy.as_slice())]{std::fs::OpenOptions::new().write(true).create_new(true).open(directory.join(format!("flatten-content-{rotation}-{unit}-{case}-{kind}.pdf"))).unwrap().write_all(bytes).unwrap();}
        }
        let a=fixtures::pixels(&rt,&output,0);let b=fixtures::pixels(&rt,&reference,0);assert_eq!((a.0,a.1),(b.0,b.1));let differences=a.2.iter().zip(&b.2).filter(|(a,b)|a!=b).count();assert_eq!(differences,0,"{rotation}/{unit}/{case}");
        let b=fixtures::pixels(&rt,&legacy,0);assert_eq!((a.0,a.1),(b.0,b.1));assert_eq!(a.2.iter().zip(&b.2).filter(|(a,b)|a!=b).count(),0,"legacy {rotation}/{unit}/{case}");
    }}}
}

fn minimal(content:&[u8])->(Document,ObjectId,ObjectId){
    let mut doc=Document::with_version("1.7");let tree=doc.new_object_id();let stream=doc.add_object(Stream::new(dictionary!{},content.to_vec()));
    let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),200.into(),200.into()],"Contents"=>stream,"Resources"=>dictionary!{}});
    doc.objects.insert(tree,dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}.into());let cat=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree});doc.trailer.set("Root",cat);
    let form=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),20.into(),20.into()],"Resources"=>dictionary!{}},b"q 1 0 0 rg 0 0 20 20 re f Q".to_vec()));(doc,page,form)
}
#[test]
fn structured_overlay_flatten_content_refuses_unbalanced_or_opaque_source_contexts(){
    for content in [b"Q".as_slice(),b"q",b"BT",b"/P BMC",b"BX",b"1 0 0 1 0 0 cm FuturePaint",b"BT q Q ET",b"/P BMC BT EMC ET"]{
        let (doc,page,form)=minimal(content);assert!(append(doc,&[PageForms{page,forms:&[form]}],&mut ||Ok(())).is_err(),"{}",String::from_utf8_lossy(content));
    }
}
#[test]
fn structured_overlay_flatten_content_empty_requests_leave_the_graph_exact(){
    let (doc,_,_)=minimal(b"FutureOperator");let output=append(doc.clone(),&[],&mut ||Ok(())).unwrap();assert_eq!(output.objects,doc.objects);assert_eq!(output.trailer,doc.trailer);
}

#[test]
fn structured_overlay_flatten_content_preserves_inherited_resources_and_reserves_nested_names(){
    let (mut doc,page,form)=minimal(b"/Original Do /FolioFlat0 Do");let tree=doc.get_dictionary(page).unwrap().get(b"Parent").unwrap().as_reference().unwrap();
    let original=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),200.into(),200.into()],"Resources"=>dictionary!{}},b"q 0 0 1 rg 100 100 20 20 re f Q /FolioFlat1 Do".to_vec()));
    let xobjects=doc.add_object(dictionary!{"Original"=>original});let resource=doc.add_object(dictionary!{"XObject"=>xobjects});doc.get_dictionary_mut(tree).unwrap().set("Resources",resource);doc.get_dictionary_mut(page).unwrap().remove(b"Resources");
    let mut second=doc.get_dictionary(page).unwrap().clone();second.remove(b"Annots");let second=doc.add_object(second);doc.get_dictionary_mut(tree).unwrap().set("Kids",vec![Object::Reference(page),second.into()]);doc.get_dictionary_mut(tree).unwrap().set("Count",2);
    let before=doc.clone();let output=append(doc,&[PageForms{page,forms:&[form]}],&mut ||Ok(())).unwrap();
    for (id,value) in &before.objects{if *id!=page{assert_eq!(&output.objects[id],value);}}
    let resources=output.get_dictionary(page).unwrap().get(b"Resources").unwrap().as_dict().unwrap();let objects=resources.get(b"XObject").unwrap().as_dict().unwrap();assert_eq!(objects.get(b"Original").unwrap().as_reference().unwrap(),original);assert_eq!(objects.get(b"FolioFlat2").unwrap().as_reference().unwrap(),form);assert!(!objects.has(b"FolioFlat0")&&!objects.has(b"FolioFlat1"));
    assert!(!output.get_dictionary(second).unwrap().has(b"Resources"));assert_eq!(output.objects[&resource],before.objects[&resource]);assert_eq!(output.objects[&xobjects],before.objects[&xobjects]);
    let mut reference=before;let annotation=reference.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Stamp","P"=>page,"Rect"=>vec![0.into(),0.into(),20.into(),20.into()],"F"=>4,"AP"=>dictionary!{"N"=>form}});reference.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(annotation)]);
    let mut output=output;let reference=crate::pdf_output::save(&mut reference).unwrap();let output=crate::pdf_output::save(&mut output).unwrap();let rt=fixtures::runtime();
    for page in [0,1]{let a=fixtures::pixels(&rt,&output,page);let b=fixtures::pixels(&rt,&reference,page);assert_eq!((a.0,a.1),(b.0,b.1));assert_eq!(a.2.iter().zip(&b.2).filter(|(a,b)|a!=b).count(),0);}
}

#[test]
fn structured_overlay_flatten_content_split_tokens_nested_scopes_and_type3_admission(){
    let (mut doc,page,form)=minimal(b"");let a=doc.add_object(Stream::new(dictionary!{},b"q 0 0 10 10 r".to_vec()));let b=doc.add_object(Stream::new(dictionary!{},b"e W Q".to_vec()));let alias=doc.add_object(Object::Array(vec![a.into(),b.into()]));doc.get_dictionary_mut(page).unwrap().set("Contents",alias);
    let output=append(doc,&[PageForms{page,forms:&[form]}],&mut ||Ok(())).unwrap();let contents=output.get_dictionary(page).unwrap().get(b"Contents").unwrap().as_array().unwrap();assert_eq!(contents.len(),5);assert_eq!(contents[1..3],[Object::Reference(a),Object::Reference(b)]);
    for program in [b"q".as_slice(),b"0 0 20 20 re W",b"/P BMC BT EMC ET"]{
        let (mut doc,page,form)=minimal(b"/Old Do");let old=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),100.into()],"Resources"=>dictionary!{}},program.to_vec()));doc.get_dictionary_mut(page).unwrap().set("Resources",dictionary!{"XObject"=>dictionary!{"Old"=>old}});
        assert!(crate::composition_form_scope::reserve_names(&doc,&[page],&mut Budget::default(),Arc::new(||false),&mut ||Ok(())).is_ok(),"existing name-only admission unchanged");
        assert!(append(doc,&[PageForms{page,forms:&[form]}],&mut ||Ok(())).is_err());
    }
    let operations=lopdf::content::Content::decode_strict_with_resource_control(b"20 0 0 0 20 20 d1 0 0 20 20 re f",lopdf::ResourceLimits::default(),Arc::new(||false)).unwrap().0.operations;
    assert_eq!(operations.iter().map(|o|o.operator.as_str()).collect::<Vec<_>>(),vec!["d1","re","f"]);assert_eq!(operations[0].operands.len(),6);
    assert!(inspect_program(&operations,ProgramKind::Type3,&mut Budget::default(),&mut ||Ok(())).is_ok());assert!(inspect_program(&operations,ProgramKind::Isolated,&mut Budget::default(),&mut ||Ok(())).is_err());
    let operations=lopdf::content::Content::decode(b"20 0 d0 0 0 20 20 re f").unwrap().operations;assert_eq!(operations[0].operator,"d0");assert_eq!(operations[0].operands.len(),2);assert!(inspect_program(&operations,ProgramKind::Type3,&mut Budget::default(),&mut ||Ok(())).is_ok());
    for input in [b"20 0 d01".as_slice(),b"[] 0 d12",b"q1"]{let operations=lopdf::content::Content::decode(input).unwrap().operations;assert_eq!(operations.len(),1);assert!(inspect_program(&operations,ProgramKind::Type3,&mut Budget::default(),&mut ||Ok(())).is_err());}
}

#[test]
fn structured_overlay_flatten_content_bounds_cancellation_and_form_admission(){
    let (doc,page,form)=minimal(b"q 0 0 20 20 re W Q");let plans=[PageForms{page,forms:&[form]}];let original=doc.objects.clone();
    let mut calls=0;append(doc.clone(),&plans,&mut ||{calls+=1;Ok(())}).unwrap();
    for stop in [1,calls/2,calls]{let mut seen=0;assert!(append(doc.clone(),&plans,&mut ||{seen+=1;if seen==stop{Err(bad("cancelled"))}else{Ok(())}}).is_err());}
    assert!(append_controlled(doc.clone(),&plans,Arc::new(||true),&mut ||Ok(())).is_err());
    for mode in 0..4 {let mut budget=Budget::default();match mode{0=>budget.metadata=0,1=>budget.steps=100_000,2=>budget.parser.input_bytes=1,_=>budget.parser.object_bytes=1};assert!(append_bounded(doc.clone(),&plans,budget,Arc::new(||false),&mut ||Ok(())).is_err());}
    assert_eq!(doc.objects,original);
    for mode in ["matrix","bounds","not-form","unknown-page","repeat-page","max-id"]{
        let mut doc=doc.clone();let mut target=page;match mode{
            "matrix"=>doc.objects.get_mut(&form).unwrap().as_stream_mut().unwrap().dict.set("Matrix",vec![2.into(),0.into(),0.into(),2.into(),0.into(),0.into()]),
            "bounds"=>doc.objects.get_mut(&form).unwrap().as_stream_mut().unwrap().dict.set("BBox",vec![0.into(),0.into(),0.into(),20.into()]),
            "not-form"=>doc.objects.get_mut(&form).unwrap().as_stream_mut().unwrap().dict.set("Subtype","Image"),
            "unknown-page"=>target=(99999,0),"max-id"=>doc.max_id=u32::MAX,_=>{},
        }
        let forms=[form];let mut plans=vec![PageForms{page:target,forms:&forms}];if mode=="repeat-page"{plans.push(PageForms{page:target,forms:&forms});}assert!(append(doc,&plans,&mut ||Ok(())).is_err(),"{mode}");
    }
    let (doc,page,form)=minimal(&vec![b'z';4096]);let error=append(doc,&[PageForms{page,forms:&[form]}],&mut ||Ok(())).unwrap_err().to_string();assert!(error.len()<256,"source operator bytes must not be echoed in errors");
}
