use super::*;
use super::super::structured_overlay_render::GeneratedOverlay;
use lopdf::{dictionary, Object, Stream};

#[test]
fn annotation_groups_follow_real_import_drafts_into_copied_page_deletion() {
    use super::super::structured_overlay_render::tests as fixtures;
    use super::super::imported_overlay_sources::publication::OwnedChange;
    let mut rt=fixtures::runtime();
    for rotation in [0,90,180,270] {for unit in [1.,1.7] {
        let background=fixtures::source(&mut rt,rotation,unit,b"q 0.2 0.4 0.8 rg 160 300 20 20 re f Q");
        let background_bytes=rt.document(&background.id).unwrap().bytes().unwrap();
        let mut draft=fixtures::request(&rt,&background);draft.pages[0].overlays.retain(|o|matches!(o,Overlay::Highlight(_)));
        let stage=rt.prepare_structured_overlays(&draft,&mut ||Ok(())).unwrap();let mut doc=crate::pdf_output::load(&background_bytes).unwrap();
        // This marker tests staging isolation elsewhere; arbitrary catalog
        // extensions deliberately remain outside catalog-assembly admission.
        doc.catalog_mut().unwrap().remove(b"SourceOnly");let page=doc.get_pages()[&1];
        let target=TargetPage{page_id:page,geometry:stage.pages[0].geometry,unit};
        let mut attached=attach_generated(doc,&[target],stage,&mut ||Ok(())).unwrap();let root=attached.pages[0].overlays[0].annotation_id;
        let popup=attached.document.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Popup","Parent"=>root,"P"=>page,"Rect"=>vec![0.into(),40.into(),100.into(),140.into()],"Open"=>false});
        attached.document.get_dictionary_mut(root).unwrap().set("Popup",popup);
        attached.document.get_dictionary_mut(page).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().push(popup.into());
        let bytes=crate::pdf_output::save(&mut attached.document).unwrap();
        let path=std::env::temp_dir().join(format!("folio-group-source-{}.pdf",Uuid::new_v4()));
        let info=rt.open_bytes(bytes.clone(),path.clone(),path,true,true,false,OpenSecurity::Restored(None)).unwrap();assert_eq!(info.pages[0].overlays.len(),1);
        let source=rt.document(&info.id).unwrap();let mut publication=source.imported_overlay_sources.publication(source.source_bytes.identity(),1,&mut ||Ok(())).unwrap();
        let keep=publication.page(0,&info.pages[0].overlays,&mut ||Ok(())).unwrap();assert!(matches!(keep.owned[0],OwnedChange::Keep(_,_)));
        let deleted=publication.page(0,&[],&mut ||Ok(())).unwrap();let OwnedChange::Delete(binding)=&deleted.owned[0] else{panic!("complete empty draft must delete its imported annotation")};
        let group=binding.source_group();assert_eq!(group.annotation.page_index,0);assert_eq!(group.annotation.annotation_id,Some(root));assert_eq!(group.popup_slots,&[1]);
        let mut graph=crate::composition_graph::CompositionGraph::new();graph.import_bytes(0,source.bytes().unwrap().as_ref(),Arc::new(||false)).unwrap();
        let groups=graph.source_annotation_groups(0,&[group],&mut ||Ok(())).unwrap();let anchors=[groups[0].annotation,groups[0].popups[0]];
        let assembled=graph.assemble(&[0],&mut ||Ok(())).unwrap();
        let (copied,pages,receipts)=crate::composition_occurrences::project_forms_tracked(assembled,&[anchors[0].page,anchors[0].page],&anchors,&mut ||Ok(())).unwrap();
        let root=receipts.iter().find(|r|r.output_page==pages[1] && r.origin==anchors[0]).copied().unwrap();
        let popups=receipts.iter().filter(|r|r.output_page==pages[1] && r.origin==anchors[1]).copied().collect::<Vec<_>>();assert_eq!(popups.len(),1);
        let mut output=crate::composition_annotation_removal::remove_owned(copied,&[crate::composition_annotation_removal::Removal{annotation:root,popups:&popups}],&mut ||Ok(())).unwrap();
        let saved=crate::pdf_output::save(&mut output).unwrap();
        assert_eq!(fixtures::pixels(&rt,&saved,0),fixtures::pixels(&rt,&bytes,0));assert_eq!(fixtures::pixels(&rt,&saved,1),fixtures::pixels(&rt,&background_bytes,0));
        assert_ne!(fixtures::pixels(&rt,&bytes,0),fixtures::pixels(&rt,&background_bytes,0));assert_eq!(source.bytes().unwrap().as_ref(),bytes.as_slice());
        if let Some(root)=std::env::var_os("FOLIO_STRUCTURED_OVERLAY_CAPTURE_DIR") {
            use std::io::Write;
            let directory=PathBuf::from(root);std::fs::create_dir_all(&directory).unwrap();
            for (kind,data) in [("source",bytes.as_slice()),("background",background_bytes.as_ref()),("actual",saved.as_slice())] {
                std::fs::OpenOptions::new().write(true).create_new(true).open(directory.join(format!("groups-{rotation}-{unit}-{kind}.pdf"))).unwrap().write_all(data).unwrap();
            }
        }
    }}
}

#[test]
fn annotation_replacement_matches_real_renderer_and_reopens_all_portable_types() {
    use super::super::structured_overlay_render::tests as fixtures;
    use crate::composition_annotation_receipts::{Anchor,Receipt};
    use crate::composition_annotation_removal::{replace_owned,Replacement};
    let mut rt=fixtures::runtime();
    let font=rt.fonts.register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec()).unwrap();
    for rotation in [0,90,180,270] {for unit in [1.,1.7] {
        let source=fixtures::source(&mut rt,rotation,unit,b"q 0.2 0.4 0.8 rg 160 300 20 20 re f Q");
        let original=rt.document(&source.id).unwrap().bytes().unwrap();
        let mut request=fixtures::request(&rt,&source);
        request.pages[0].overlays.retain(|o|!matches!(o,Overlay::Comment(_)));
        if let Overlay::Text(text)=&mut request.pages[0].overlays[0]{text.font_id=Some(font.id.clone());}
        let prepared=rt.prepare_structured_overlays(&request,&mut ||Ok(())).unwrap();
        let doc=crate::pdf_output::load(&original).unwrap();let page=doc.get_pages()[&1];
        let target=TargetPage{page_id:page,geometry:prepared.pages[0].geometry,unit};
        let mut initial=attach_generated(doc,&[target],prepared,&mut ||Ok(())).unwrap();
        let mut receipts=vec![];
        for binding in &initial.pages[0].overlays {
            let slot=initial.document.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap().iter().position(|v|v.as_reference().ok()==Some(binding.annotation_id)).unwrap();
            receipts.push(Receipt{origin:Anchor{page,slot,annotation:binding.annotation_id},output_page:page,output_slot:slot,annotation:binding.annotation_id});
            let dict=initial.document.get_dictionary_mut(binding.annotation_id).unwrap();
            dict.set("NM",Object::string_literal(format!("authored-{}",binding.overlay_id)));
            dict.set("T",Object::string_literal("Original author"));dict.set("CreationDate",Object::string_literal("D:20260101000000Z"));
        }
        let before=crate::pdf_output::save(&mut initial.document.clone()).unwrap();
        for overlay in &mut request.pages[0].overlays {match overlay {
            Overlay::Text(t)=>{t.text="Changed text".into();t.x+=9.;t.color="#336699".into();},
            Overlay::Highlight(h)=>{h.color="#33ff99".into();h.rects[0].y+=12.;h.text=Some("New marked words".into());},
            Overlay::Ink(i)=>{i.color="#1133aa".into();i.stroke_width=3.;i.paths[0][0].x+=11.;},
            Overlay::Image(i)=>{i.x+=17.;i.width*=0.8;},
            _=>unreachable!(),
        }}
        let reference=rt.compose_imported_pdf(&request,&[],true,CompositionIntent::Save,None).unwrap();
        let stage=rt.prepare_structured_overlays(&request,&mut ||Ok(())).unwrap();
        let target=TargetPage{page_id:page,geometry:stage.pages[0].geometry,unit};
        let attached=attach_generated(initial.document,&[target],stage,&mut ||Ok(())).unwrap();
        let replacements=attached.pages[0].overlays.iter().zip(&receipts).map(|(g,r)|Replacement{original:*r,popups:&[],generated:g.annotation_id}).collect::<Vec<_>>();
        let mut replaced=replace_owned(attached.document,&replacements,&mut ||Ok(())).unwrap();
        let saved=crate::pdf_output::save(&mut replaced).unwrap();
        let label=format!("replacement-{rotation}-{unit}");fixtures::same_pixels(&rt,&saved,&reference,&label);
        assert_ne!(fixtures::pixels(&rt,&before,0),fixtures::pixels(&rt,&saved,0),"unchanged drawing must fail the oracle");
        if let Some(root)=std::env::var_os("FOLIO_STRUCTURED_OVERLAY_CAPTURE_DIR") {
            use std::io::Write;
            std::fs::OpenOptions::new().write(true).create_new(true).open(PathBuf::from(root).join(format!("{label}-source.pdf"))).unwrap().write_all(&before).unwrap();
        }
        for receipt in &receipts {
            let dict=replaced.get_dictionary(receipt.annotation).unwrap();
            assert_eq!(dict.get(b"T").unwrap(),&Object::string_literal("Original author"));
        }
        let path=std::env::temp_dir().join(format!("folio-replacement-{}.pdf",Uuid::new_v4()));
        let reopened=rt.open_bytes(saved,path.clone(),path,true,true,false,OpenSecurity::Restored(None)).unwrap();
        assert_eq!(reopened.pages[0].overlays.len(),4,"portable text/ink/image and native-equivalent highlight remain editable");
        assert!(reopened.pages[0].overlays.iter().any(|o|matches!(o,Overlay::Text(t) if t.text=="Changed text")));
        assert_eq!(rt.document(&source.id).unwrap().bytes().unwrap().as_ref(),original.as_ref());
    }}
}

#[test]
fn annotation_receipts_direct_materialization_preserves_native_page_appearance() {
    use super::super::structured_overlay_render::tests as fixtures;
    use crate::composition_annotation_receipts::SourceSlot;
    let mut rt=fixtures::runtime();
    for rotation in [0,90,180,270] {for unit in [1.,1.7] {
        let source=fixtures::source(&mut rt,rotation,unit,b"q 0.2 0.4 0.8 rg 160 300 20 20 re f Q");
        let mut doc=crate::pdf_output::load(rt.document(&source.id).unwrap().bytes().unwrap().as_ref()).unwrap();doc.catalog_mut().unwrap().remove(b"SourceOnly");
        let page=doc.get_pages()[&1];
        let highlight=dictionary!{"Type"=>"Annot","Subtype"=>"Highlight","Rect"=>vec![30.into(),300.into(),120.into(),320.into()],"QuadPoints"=>vec![30.into(),320.into(),120.into(),320.into(),30.into(),300.into(),120.into(),300.into()],"C"=>vec![1.into(),1.into(),0.into()],"F"=>4};
        let note=dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![200.into(),200.into(),220.into(),220.into()],"Contents"=>Object::string_literal("Direct note")};
        doc.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Dictionary(highlight),Object::Dictionary(note)]);
        let original=crate::pdf_output::save(&mut doc).unwrap();let mut graph=crate::composition_graph::CompositionGraph::new();graph.import_bytes(0,&original,Arc::new(||false)).unwrap();
        graph.source_annotations(0,&[SourceSlot{page_index:0,slot:0,annotation_id:None},SourceSlot{page_index:0,slot:1,annotation_id:None}],&mut ||Ok(())).unwrap();
        let mut output=graph.assemble(&[0],&mut ||Ok(())).unwrap();let saved=crate::pdf_output::save(&mut output).unwrap();
        fixtures::same_pixels(&rt,&saved,&original,&format!("direct-annotations-{rotation}-{unit}"));
    }}
}

fn frame() -> PageGeometry { PageGeometry { left: 0., bottom: 0., right: 300., top: 400., rotation: 0 } }
fn destination() -> (Document, Vec<TargetPage>) {
    let mut doc=Document::with_version("1.4"); let tree=doc.new_object_id();
    let stream=doc.add_object(Stream::new(dictionary!{}, b"q 0 0 20 20 re f Q".to_vec()));
    let note=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Contents"=>Object::string_literal("original native note")});
    let list=doc.add_object(vec![Object::Reference(note)]);
    let mut targets=vec![];
    for _ in 0..2 {
        let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Contents"=>stream,"Resources"=>dictionary!{},"Annots"=>list});
        targets.push(TargetPage{page_id:page,geometry:frame(),unit:1.});
    }
    doc.objects.insert(tree,dictionary!{"Type"=>"Pages","Kids"=>targets.iter().map(|p|Object::Reference(p.page_id)).collect::<Vec<_>>(),"Count"=>2}.into());
    let root=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree,"SourceMarker"=>Object::string_literal("keep original catalog metadata")});
    doc.trailer.set("Root",root);
    (doc,targets)
}
fn generated() -> PreparedOverlays {
    let mut doc=Document::with_version("1.7"); let tree=doc.new_object_id();
    let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let resources=doc.add_object(dictionary!{"Font"=>dictionary!{"F1"=>font},"OpaqueAlias"=>dictionary!{"Parent"=>font}});
    let ap=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),30.into()],"Resources"=>resources},b"BT /F1 12 Tf 0 10 Td (Generated) Tj ET".to_vec()));
    doc.add_object(Stream::new(dictionary!{},b"unused staging sentinel".to_vec()));
    let mut pages=vec![];
    for _ in 0..2 {
        let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary!{}});
        let mut overlays=vec![];
        for label in ["same-id", "second-id"] {
            let annotation=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"FreeText","P"=>page,"Rect"=>vec![20.into(),200.into(),120.into(),230.into()],"NM"=>Object::string_literal(label),"AP"=>dictionary!{"N"=>ap}});
            overlays.push(GeneratedOverlay{overlay_id:label.into(),annotation_id:annotation});
        }
        // Paint order differs from request/binding order, as real finalization can.
        doc.get_dictionary_mut(page).unwrap().set("Annots",overlays.iter().rev().map(|a|Object::Reference(a.annotation_id)).collect::<Vec<_>>());
        pages.push(GeneratedPage{page_id:page,geometry:frame(),unit:1.,overlays});
    }
    doc.objects.insert(tree,dictionary!{"Type"=>"Pages","Kids"=>pages.iter().map(|p|Object::Reference(p.page_id)).collect::<Vec<_>>(),"Count"=>2}.into());
    let root=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree});doc.trailer.set("Root",root);
    PreparedOverlays{document:doc,pages}
}

#[test]
fn structured_overlay_publish_moves_only_generated_closures_and_preserves_original_graph() {
    let (doc,targets)=destination(); let original=doc.objects.clone();
    let stage=generated(); let original_stage=stage.document.objects.clone();
    let output=attach_generated(doc,&targets,stage,&mut ||Ok(())).unwrap();
    assert_eq!(output.document.version,"1.7");
    let mut appearances=vec![];
    for (index,target) in targets.iter().enumerate() {
        let page=output.document.get_dictionary(target.page_id).unwrap();
        let annotations=page.get(b"Annots").unwrap().as_array().unwrap();
        assert_eq!(annotations.len(),3);
        let bindings=&output.pages[index].overlays;
        assert_eq!(annotations[1].as_reference().unwrap(),bindings[1].annotation_id);
        assert_eq!(annotations[2].as_reference().unwrap(),bindings[0].annotation_id);
        for binding in bindings {
            let annotation=output.document.get_dictionary(binding.annotation_id).unwrap();
            assert_eq!(annotation.get(b"P").unwrap().as_reference().unwrap(),target.page_id);
            assert_eq!(lopdf::decode_text_string(annotation.get(b"NM").unwrap()).unwrap(),binding.overlay_id);
            appearances.push(annotation.get(b"AP").unwrap().as_dict().unwrap().get(b"N").unwrap().as_reference().unwrap());
        }
    }
    assert!(appearances.iter().all(|id|*id==appearances[0]),"shared drawing resources must remain shared");
    assert_ne!(output.pages[0].overlays[0].annotation_id,output.pages[1].overlays[0].annotation_id);
    for (id,value) in &original {
        let mut actual=output.document.objects[id].clone();
        if targets.iter().any(|p|p.page_id==*id) {
            actual.as_dict_mut().unwrap().set("Annots",value.as_dict().unwrap().get(b"Annots").unwrap().clone());
        }
        assert_eq!(&actual,value,"original object {id:?} changed outside page Annots");
    }
    assert_eq!(output.document.get_pages().len(),2);
    assert!(!output.document.objects.values().any(|v|v.as_stream().ok().is_some_and(|s|s.content==b"unused staging sentinel")));
    assert_eq!(output.document.objects.len(),original.len()+7,"four annotation roots plus three shared dependencies, no staging catalog/pages");
    assert!(original_stage.len()>7);
    crate::composition_graph::validate_retained(&output.document,&mut ||Ok(())).unwrap();
}

#[test]
fn structured_overlay_publish_rejects_wrong_page_cross_graph_edges_and_ambiguous_bindings() {
    for mode in ["page", "catalog-edge", "other-annotation", "duplicate-root", "duplicate-label", "unit", "target-order"] {
        let (doc,mut targets)=destination();let mut stage=generated();
        let first=stage.pages[0].overlays[0].annotation_id;
        match mode {
            "page"=>{stage.document.get_dictionary_mut(first).unwrap().set("P",stage.pages[1].page_id);},
            "catalog-edge"=>{let root=stage.document.trailer.get(b"Root").unwrap().clone();stage.document.get_dictionary_mut(first).unwrap().set("Hidden",root);},
            "other-annotation"=>{stage.document.get_dictionary_mut(first).unwrap().set("Hidden",stage.pages[1].overlays[0].annotation_id);},
            "duplicate-root"=>stage.pages[1].overlays[0].annotation_id=first,
            "duplicate-label"=>stage.pages[0].overlays[1].overlay_id="same-id".into(),
            "unit"=>targets[0].unit=1.7,
            "target-order"=>targets.swap(0,1),
            _=>unreachable!(),
        }
        assert!(attach_generated(doc,&targets,stage,&mut ||Ok(())).is_err(),"{mode}");
    }
}

#[test]
fn structured_overlay_publish_cancelled_attachment_never_returns_a_partial_document() {
    let (doc,targets)=destination();
    let mut calls=0;
    attach_generated(doc.clone(),&targets,generated(),&mut ||{calls+=1;Ok(())}).unwrap();
    assert!(calls>10);
    for stop in [1,calls/2,calls] {
        let mut seen=0;
        assert!(attach_generated(doc.clone(),&targets,generated(),&mut ||{seen+=1;if seen==stop {Err(EngineError::InvalidRequest("cancelled".into()))} else {Ok(())}}).is_err(),"checkpoint {stop}");
    }
}

#[test]
fn structured_overlay_publish_matches_real_renderer_and_reopens_all_overlay_types() {
    use super::super::structured_overlay_render::tests as fixtures;
    let mut rt=fixtures::runtime();
    let font=rt.fonts.register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec()).unwrap();
    for rotation in [0,90,180,270] {
        for unit in [1.,1.7] {
            let source=fixtures::source(&mut rt,rotation,unit,b"q 0.2 0.4 0.8 rg 160 300 20 20 re f Q");
            let original=rt.document(&source.id).unwrap().bytes().unwrap();
            let mut request=fixtures::request(&rt,&source);
            if let Overlay::Text(text)=&mut request.pages[0].overlays[0] {text.font_id=Some(font.id.clone());}
            let reference=rt.compose_imported_pdf(&request,&[],true,CompositionIntent::Save,None).unwrap();
            let prepared=rt.prepare_structured_overlays(&request,&mut ||Ok(())).unwrap();
            let doc=crate::pdf_output::load(&original).unwrap();
            let target=TargetPage{page_id:doc.get_pages()[&1],geometry:prepared.pages[0].geometry,unit};
            let mut attached=attach_generated(doc,&[target],prepared,&mut ||Ok(())).unwrap();
            let saved=crate::pdf_output::save(&mut attached.document).unwrap();
            fixtures::same_pixels(&rt,&saved,&reference,&format!("attached-{rotation}-{unit}"));
            let path=std::env::temp_dir().join(format!("folio-attachment-{}.pdf",Uuid::new_v4()));
            let reopened=rt.open_bytes(saved,path.clone(),path,true,true,false,OpenSecurity::Restored(None)).unwrap();
            assert_eq!(reopened.pages[0].overlays.len(),4,"text/ink/image/highlight should remain editable; comment remains native");
            assert_eq!(rt.document(&source.id).unwrap().bytes().unwrap().as_ref(),original.as_ref());
            assert!(attached.document.catalog().unwrap().has(b"SourceOnly"),"source metadata must survive");
        }
    }
}

#[test]
fn structured_overlay_publish_handles_empty_pages_cycles_and_sparse_id_limits() {
    let (mut doc,targets)=destination();let mut stage=generated();
    // A shared resource cycle is a finite graph, not a recursive clone request.
    let root=stage.pages[0].overlays[0].annotation_id;
    let ap=stage.document.get_dictionary(root).unwrap().get(b"AP").unwrap().as_dict().unwrap().get(b"N").unwrap().as_reference().unwrap();
    let resources=stage.document.get_object(ap).unwrap().as_stream().unwrap().dict.get(b"Resources").unwrap().as_reference().unwrap();
    stage.document.get_dictionary_mut(resources).unwrap().set("SelfAlias",resources);
    let result=attach_generated(doc.clone(),&targets,stage,&mut ||Ok(())).unwrap();
    crate::composition_graph::validate_retained(&result.document,&mut ||Ok(())).unwrap();
    let mut empty=generated();
    for page in &mut empty.pages {empty.document.get_dictionary_mut(page.page_id).unwrap().remove(b"Annots");page.overlays.clear();}
    let unchanged=attach_generated(doc.clone(),&targets,empty,&mut ||Ok(())).unwrap();
    assert_eq!(unchanged.document.objects,doc.objects);assert_eq!(unchanged.document.version,doc.version);
    doc.max_id=u32::MAX;
    assert!(attach_generated(doc,&targets,generated(),&mut ||Ok(())).is_err());
}

#[test]
fn structured_overlay_publish_reserves_direct_list_payloads_before_copying() {
    let object=Object::String(vec![b'x';1024],lopdf::StringFormat::Literal);
    let mut check=||Ok(());
    let mut work=Work{check:&mut check,metadata:512,steps:0};
    assert!(reserve_copy(&object,&mut work,0).is_err());
    let mut work=Work{check:&mut check,metadata:4096,steps:0};
    reserve_copy(&object,&mut work,0).unwrap();
    assert_eq!(copy_existing(&object,&mut work,0).unwrap(),object);
}

#[test]
fn structured_overlay_publish_version_promotion_keeps_annotation_language_semantics() {
    for catalog_version in [false,true] {
        let (mut doc,targets)=destination();let mut stage=generated();
        let note=doc.objects.iter().find_map(|(id,value)|value.as_dict().ok().filter(|d|d.get(b"Subtype").and_then(Object::as_name).ok()==Some(b"Text")).map(|_|*id)).unwrap();
        doc.get_dictionary_mut(note).unwrap().set("Lang",Object::string_literal("fr-FR"));
        if catalog_version {stage.document.catalog_mut().unwrap().set("Version",Object::Name(b"2.0".to_vec()));} else {stage.document.version="2.0".into();}
        assert!(attach_generated(doc.clone(),&targets,stage,&mut ||Ok(())).is_err(),"promotion must not activate a previously dormant annotation Lang");
        // Already-PDF2 language is not a promotion and must remain supported.
        doc.catalog_mut().unwrap().set("Version",Object::Name(b"2.0".to_vec()));
        let mut stage=generated();stage.document.version="2.0".into();
        let attached=attach_generated(doc,&targets,stage,&mut ||Ok(())).unwrap();
        assert_eq!(attached.document.version,"2.0");
        assert_eq!(attached.document.get_dictionary(note).unwrap().get(b"Lang").unwrap().as_str().unwrap(),b"fr-FR");
    }
}

#[test]
fn structured_overlay_publish_cropped_repeated_pages_share_assets_and_keep_independent_roots() {
    use super::super::structured_overlay_render::tests as fixtures;
    let mut rt=fixtures::runtime();
    let font=rt.fonts.register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec()).unwrap();
    let source=fixtures::source(&mut rt,90,1.7,b"q 0.2 0.4 0.8 rg 160 300 20 20 re f Q");
    let mut input=crate::pdf_output::load(rt.document(&source.id).unwrap().bytes().unwrap().as_ref()).unwrap();
    let page=input.get_pages()[&1];
    input.get_dictionary_mut(page).unwrap().set("CropBox",vec![10.into(),50.into(),280.into(),400.into()]);
    let input_bytes=crate::pdf_output::save(&mut input).unwrap();
    let source=rt.register_generated_document(input_bytes.clone(),"Cropped repeated source.pdf").unwrap();
    let mut request=fixtures::request(&rt,&source);
    if let Overlay::Text(text)=&mut request.pages[0].overlays[0] {text.font_id=Some(font.id.clone());}
    let mut second=request.pages[0].clone();second.id="copy".into();request.pages.push(second);
    let reference=rt.compose_imported_pdf(&request,&[],true,CompositionIntent::Save,None).unwrap();
    let prepared=rt.prepare_structured_overlays(&request,&mut ||Ok(())).unwrap();
    let assets=|document:&Document| {
        let fonts=document.objects.values().filter_map(|o|o.as_dict().ok()).filter_map(|d|d.get(b"FontFile2").ok()).map(|v|v.as_reference().unwrap()).collect::<BTreeSet<_>>();
        let images=document.objects.values().filter(|o|o.as_stream().ok().is_some_and(|s|s.dict.get(b"Subtype").and_then(Object::as_name).ok()==Some(b"Image"))).count();
        (fonts.len(),images)
    };
    let shared_assets=assets(&prepared.document);
    assert_eq!(shared_assets.0,1,"the same custom font should be embedded only once");
    assert!(shared_assets.1>0);
    let (document,page_ids)=crate::composition_occurrences::project_forms(input,&[page,page],&mut ||Ok(())).unwrap();
    let original=document.objects.clone();
    let targets=page_ids.iter().zip(&prepared.pages).map(|(id,page)|TargetPage{page_id:*id,geometry:page.geometry,unit:page.unit}).collect::<Vec<_>>();
    assert_eq!(targets[0].geometry.left,10.);
    let mut output=attach_generated(document,&targets,prepared,&mut ||Ok(())).unwrap();
    assert_eq!(assets(&output.document),shared_assets,"attachment must not duplicate shared image/font resources");
    let first=output.pages[0].overlays.iter().map(|o|o.annotation_id).collect::<BTreeSet<_>>();
    assert!(output.pages[1].overlays.iter().all(|o|!first.contains(&o.annotation_id)));
    for (id,value) in &original {
        let mut actual=output.document.objects[id].clone();
        if page_ids.contains(id){actual.as_dict_mut().unwrap().remove(b"Annots");}
        assert_eq!(&actual,value,"unchanged source object {id:?}");
    }
    let saved=crate::pdf_output::save(&mut output.document).unwrap();
    for page in 0..2 {assert_eq!(fixtures::pixels(&rt,&saved,page),fixtures::pixels(&rt,&reference,page),"copied page {page}");}
    fixtures::same_pixels(&rt,&saved,&reference,"attached-cropped-repeat");
    let path=std::env::temp_dir().join(format!("folio-attachment-{}.pdf",Uuid::new_v4()));
    let reopened=rt.open_bytes(saved,path.clone(),path,true,true,false,OpenSecurity::Restored(None)).unwrap();
    assert_eq!(reopened.pages.len(),2);
    for page in &reopened.pages {assert_eq!(page.overlays.len(),4);}
    assert_eq!(rt.document(&source.id).unwrap().bytes().unwrap().as_ref(),input_bytes);
}
