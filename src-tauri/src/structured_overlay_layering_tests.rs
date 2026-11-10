use super::*;

#[test]
fn structured_overlay_batch_flatten_orders_changed_surviving_highlight_without_drawings(){flatten_survivor_order(false);}
#[test]
fn structured_overlay_batch_flatten_orders_changed_surviving_highlight_with_drawing(){flatten_survivor_order(true);}

fn flatten_survivor_order(drawing:bool){
    let mut rt=fixtures::runtime();let seed=fixtures::source(&mut rt,0,1.,b"q 0.2 0.4 0.8 rg 160 300 20 20 re f Q");
    let mut doc=crate::pdf_output::load(rt.document(&seed.id).unwrap().bytes().unwrap().as_ref()).unwrap();doc.catalog_mut().unwrap().remove(b"SourceOnly");
    let page=doc.get_pages()[&1];
    let ap=doc.add_object(lopdf::Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),320.into(),420.into()],"Resources"=>dictionary!{}},b"0.8 0.2 0.4 rg 0 0 320 420 re f".to_vec()));
    let art=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Square","P"=>page,"Rect"=>vec![(-20).into(),30.into(),300.into(),450.into()],"AP"=>dictionary!{"N"=>ap},"NM"=>Object::string_literal("native-art"),"F"=>4});
    doc.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(art)]);
    let path=std::env::temp_dir().join(format!("folio-flatten-order-{}.pdf",Uuid::new_v4()));
    let info=rt.open_bytes(crate::pdf_output::save(&mut doc).unwrap(),path.clone(),path,true,true,false,OpenSecurity::Restored(None)).unwrap();
    let mut initial=fixtures::request(&rt,&info);initial.pages[0].overlays.retain(|o|matches!(o,Overlay::Highlight(_)));
    let mut published=transaction(&rt,&info,&initial);let page=published.pages[0].page_id;
    // Valid producer order: the native opaque artwork covers the highlight.
    let mut annotations=published.document.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap().clone();assert_eq!(annotations.len(),2);annotations.reverse();published.document.get_dictionary_mut(page).unwrap().set("Annots",annotations);
    let original=crate::pdf_output::save(&mut published.document).unwrap();
    let path=std::env::temp_dir().join(format!("folio-flatten-order-saved-{}.pdf",Uuid::new_v4()));
    let opened=rt.open_bytes(original.clone(),path.clone(),path,true,true,false,OpenSecurity::Restored(None)).unwrap();
    assert_eq!(opened.pages[0].overlays.len(),1,"highlight must pass actual appearance admission");
    let mut request=initial.clone();request.flatten=true;request.pages[0].source_id=opened.id.clone();request.pages[0].overlays=opened.pages[0].overlays.clone();
    let mut copy=request.pages[0].clone();copy.id="changed-copy".into();
    let Overlay::Highlight(highlight)=&mut copy.overlays[0]else{panic!("highlight")};highlight.color="#00ff00".into();
    if drawing{copy.overlays.push(serde_json::from_value(serde_json::json!({"type":"ink","id":"flatten-me","paths":[[{"x":30,"y":90},{"x":150,"y":120}]],"color":"#bb2244","strokeWidth":2})).unwrap());}
    request.pages.push(copy);
    let actual=rt.compose_imported_pdf(&request,&[],true,CompositionIntent::Save,None).unwrap();let saved=crate::pdf_output::load(&actual).unwrap();
    let pages=saved.get_pages();let mut expected=saved.clone();
    let first=saved.get_dictionary(pages[&1]).unwrap().get(b"Annots").unwrap().as_array().unwrap();
    let second=saved.get_dictionary(pages[&2]).unwrap().get(b"Annots").unwrap().as_array().unwrap();
    let kind=|v:&Object|saved.get_dictionary(v.as_reference().unwrap()).unwrap().get(b"Subtype").unwrap().as_name().unwrap().to_vec();
    assert_eq!(first.iter().map(kind).collect::<Vec<_>>(),vec![b"Highlight".to_vec(),b"Square".to_vec()],"unchanged occurrence retains exact authored order");
    assert_eq!(second.len(),2,"flattened drawing has no surviving annotation");
    let art=second.iter().find(|v|kind(v)==b"Square").unwrap().clone();let highlight=second.iter().find(|v|kind(v)==b"Highlight").unwrap().clone();
    // Independent order oracle: explicitly arrange the same saved appearances
    // by the established changed-draft convention, without the layering helper.
    expected.get_dictionary_mut(pages[&2]).unwrap().set("Annots",vec![art,highlight]);
    let expected=crate::pdf_output::save(&mut expected).unwrap();let name=format!("flatten-survivor-{drawing}");capture(&format!("{name}-actual"),&actual);capture(&format!("{name}-expected"),&expected);capture(&format!("{name}-source"),&original);
    let a=fixtures::pixels(&rt,&actual,1);let b=fixtures::pixels(&rt,&expected,1);assert_eq!(a.2.iter().zip(&b.2).filter(|(a,b)|a!=b).count(),0,"edited highlight must remain above native artwork when flattening; drawing={drawing}");
    assert_eq!(fixtures::pixels(&rt,&actual,0).2,fixtures::pixels(&rt,&original,0).2,"unchanged copy pixels");
    assert_eq!(second.iter().map(kind).collect::<Vec<_>>(),vec![b"Square".to_vec(),b"Highlight".to_vec()]);
    crate::composition_graph::validate_retained(&saved,&mut ||Ok(())).unwrap();assert_eq!(rt.document(&opened.id).unwrap().bytes().unwrap().as_ref(),original.as_slice());
}

pub(super) fn transaction(rt:&WorkerRuntime,info:&DocumentInfo,request:&ExportRequest)->Published {
    let source=rt.document(&info.id).unwrap();
    let mut publication=source.imported_overlay_sources.publication(source.source_bytes.identity(),info.pages.len(),&mut ||Ok(())).unwrap();
    let mut graph=crate::composition_graph::CompositionGraph::new();
    graph.import_bytes(0,source.bytes().unwrap().as_ref(),Arc::new(||false)).unwrap();
    let origins=graph.source_pages(0,&mut ||Ok(())).unwrap();
    let languages=graph.source_languages(&mut ||Ok(())).unwrap();
    let mut changes=vec![];let mut groups=vec![];
    for page in &request.pages {
        let change=publication.page(page.page_index,&page.overlays,&mut ||Ok(())).unwrap();
        let slots=change.owned.iter().map(|c|super::super::source(c).source_group()).collect::<Vec<_>>();
        groups.push(graph.source_annotation_groups(0,&slots,&mut ||Ok(())).unwrap());changes.push(change);
    }
    let anchors=groups.iter().flatten().flat_map(|g|std::iter::once(g.annotation).chain(g.popups.iter().copied())).map(|a|((a.page,a.slot,a.annotation),a)).collect::<BTreeMap<_,_>>().into_values().collect::<Vec<_>>();
    let selected=request.pages.iter().map(|p|origins[p.page_index]).collect::<Vec<_>>();
    let document=graph.assemble(&[0],&mut ||Ok(())).unwrap();
    let (document,pages,receipts)=crate::composition_occurrences::project_forms_tracked(document,&selected,&anchors,&mut ||Ok(())).unwrap();
    let prepared=rt.prepare_structured_overlay_changes(request,&changes.iter().collect::<Vec<_>>(),&mut ||Ok(())).unwrap();
    let targets=prepared.pages.iter().zip(&pages).map(|(p,id)|TargetPage{page_id:*id,geometry:p.geometry,unit:p.unit}).collect::<Vec<_>>();
    let plans=changes.iter().zip(&request.pages).zip(&groups).map(|((changes,page),groups)|PagePlan{source_page:origins[page.page_index],source_page_index:page.page_index,source_language:&languages[&0],changes,groups}).collect::<Vec<_>>();
    publish(document,&targets,&plans,&receipts,prepared,&mut ||Ok(())).unwrap()
}

fn capture(name:&str,bytes:&[u8]) {
    if let Some(root)=std::env::var_os("FOLIO_STRUCTURED_OVERLAY_CAPTURE_DIR") {
        use std::io::Write;let path=PathBuf::from(root);std::fs::create_dir_all(&path).unwrap();
        std::fs::OpenOptions::new().write(true).create_new(true).open(path.join(format!("{name}.pdf"))).unwrap().write_all(bytes).unwrap();
    }
}

#[test]
fn structured_overlay_batch_editable_layering_matches_preview_with_native_artwork() {
    for rotation in [0,90,180,270] {for unit in [1.,1.7] {for image in [false,true] {
        let mut rt=fixtures::runtime();let seed=fixtures::source(&mut rt,rotation,unit,b"q 0.2 0.4 0.8 rg 160 300 20 20 re f Q");
        let mut doc=crate::pdf_output::load(rt.document(&seed.id).unwrap().bytes().unwrap().as_ref()).unwrap();doc.catalog_mut().unwrap().remove(b"SourceOnly");
        let page=doc.get_pages()[&1];
        let ap=doc.add_object(lopdf::Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),320.into(),420.into()],"Resources"=>dictionary!{}},b"0.8 0.2 0.4 rg 0 0 320 420 re f".to_vec()));
        let note=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Square","P"=>page,"Rect"=>vec![(-20).into(),30.into(),300.into(),450.into()],"AP"=>dictionary!{"N"=>ap},"NM"=>Object::string_literal("native-art"),"F"=>4});
        doc.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(note)]);
        let input=crate::pdf_output::save(&mut doc).unwrap();let path=std::env::temp_dir().join(format!("folio-layer-source-{}.pdf",Uuid::new_v4()));
        let info=rt.open_bytes(input.clone(),path.clone(),path,true,true,false,OpenSecurity::Restored(None)).unwrap();assert!(info.pages[0].overlays.is_empty());
        let mut request=fixtures::request(&rt,&info);
        if !image {request.pages[0].overlays.retain(|o|!matches!(o,Overlay::Image(_)));}
        let mut copy=request.pages[0].clone();copy.id="second-copy".into();request.pages.push(copy);
        let reference=rt.compose_imported_pdf(&request,&[],true,CompositionIntent::Preview,None).unwrap();
        let mut result=transaction(&rt,&info,&request);let actual=crate::pdf_output::save(&mut result.document).unwrap();
        let label=format!("editable-layer-{rotation}-{unit}-{image}");capture(&format!("{label}-source"),&input);capture(&format!("{label}-actual"),&actual);capture(&format!("{label}-reference"),&reference);
        for index in [0,1] {let a=fixtures::pixels(&rt,&actual,index);let b=fixtures::pixels(&rt,&reference,index);assert_eq!((a.0,a.1),(b.0,b.1));assert_eq!(a.2.iter().zip(&b.2).filter(|(a,b)|a!=b).count(),0,"{label} page {index}: whole-page appearance");}
        let path=std::env::temp_dir().join(format!("folio-layer-saved-{}.pdf",Uuid::new_v4()));let reopened=rt.open_bytes(actual.clone(),path.clone(),path,true,true,false,OpenSecurity::Restored(None)).unwrap();
        let mut edit=request.clone();for (index,page) in edit.pages.iter_mut().enumerate(){page.source_id=reopened.id.clone();page.page_index=index;page.overlays=reopened.pages[index].overlays.clone();}
        let text=edit.pages[1].overlays.iter_mut().find_map(|o|if let Overlay::Text(t)=o{Some(t)}else{None}).unwrap();text.text="Changed drawing".into();
        let mut added=text.clone();added.id="added-on-reedit".into();added.text="New drawing".into();added.color="#1177cc".into();added.y+=5.;edit.pages[1].overlays.push(Overlay::Text(added));
        let reference=rt.compose_imported_pdf(&edit,&[],true,CompositionIntent::Preview,None).unwrap();let mut result=transaction(&rt,&reopened,&edit);let saved=crate::pdf_output::save(&mut result.document).unwrap();
        capture(&format!("{label}-reedit"),&saved);capture(&format!("{label}-reedit-reference"),&reference);
        for index in [0,1] {let a=fixtures::pixels(&rt,&saved,index);let b=fixtures::pixels(&rt,&reference,index);assert_eq!(a.2.iter().zip(&b.2).filter(|(a,b)|a!=b).count(),0,"{label} re-edit page {index}");}
        assert_eq!(rt.document(&info.id).unwrap().bytes().unwrap().as_ref(),input.as_slice());
    }}}
}
