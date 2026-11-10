use super::*;
use lopdf::{dictionary, Object, Stream};

fn check() -> EngineResult<()> { Ok(()) }

#[test]
fn composition_graph_content_copy_reservations_include_original_and_added_storage(){
    let doc=Document::with_version("1.7");let original=doc.objects.clone();
    assert!(reserve_content_additions(&doc,&[],100_000,999_999,256*1024*1024-3,&mut check).is_ok());
    for (objects,values,bytes) in [(100_001,0,0),(0,1_000_000,0),(0,0,256*1024*1024-2)]{assert!(reserve_content_additions(&doc,&[],objects,values,bytes,&mut check).is_err());}
    let resource=dictionary!{"Existing"=>Object::Array(vec![Object::Null;32])};
    assert!(reserve_content_additions(&doc,&[],0,999_990,0,&mut check).is_ok());assert!(reserve_content_additions(&doc,&[&resource],0,999_990,0,&mut check).is_err());
    assert!(reserve_content_additions(&doc,&[&resource],0,0,0,&mut ||Err(bad("cancelled"))).is_err());assert_eq!(doc.objects,original);
}

#[test]
fn composition_graph_source_languages_are_bounded_read_only_and_cancellable() {
    fn input(language:Option<Object>)->Document {
        let mut doc=Document::with_version("1.7");let mut cat=dictionary!{"Type"=>"Catalog"};
        if let Some(language)=language{let id=doc.add_object(language);cat.set("Lang",id);}
        let catalog=doc.add_object(cat);doc.trailer.set("Root",catalog);doc
    }
    let mut graph=CompositionGraph::new();
    for (key,value) in [(5,None),(2,Some(Object::string_literal("fr-CA"))),(9,Some(Object::string_literal(b"\xfe\xff\0d\0e".to_vec())))]{graph.import(key,input(value),&mut check).unwrap();}
    let before=snapshot(&graph);let languages=graph.source_languages(&mut check).unwrap();assert_eq!(languages.keys().copied().collect::<Vec<_>>(),vec![2,5,9]);assert_eq!(snapshot(&graph),before);
    assert!(graph.source_languages_with_budget(3*(1024+128)-1,1_000_000,&mut check).is_err());assert_eq!(snapshot(&graph),before);
    assert!(graph.source_languages_with_budget(3*(1024+128),1_000_000,&mut check).is_ok());
    let mut calls=0;graph.source_languages(&mut ||{calls+=1;Ok(())}).unwrap();assert!(calls>10);
    assert!(graph.source_languages_with_budget(usize::MAX,calls-1,&mut check).is_err());assert_eq!(snapshot(&graph),before);
    assert!(graph.source_languages_with_budget(usize::MAX,calls,&mut check).is_ok());
    for stop in 1..=calls {let mut seen=0;assert!(graph.source_languages(&mut ||{seen+=1;if seen==stop{Err(bad("cancelled"))}else{Ok(())}}).is_err());assert_eq!(snapshot(&graph),before);}
    for value in [Object::string_literal(vec![b'e';1025]),Object::string_literal("en--US"),Object::Integer(1)] {
        let mut graph=CompositionGraph::new();graph.import(0,input(Some(value)),&mut check).unwrap();let before=snapshot(&graph);assert!(graph.source_languages(&mut check).is_err());assert_eq!(snapshot(&graph),before);
    }
    for (depth,cycle) in [(64,false),(65,false),(2,true)]{
        let mut doc=input(None);let mut id=doc.add_object(Object::string_literal("fr-CA"));let leaf=id;
        for _ in 1..depth{id=doc.add_object(Object::Reference(id));}
        if cycle{doc.objects.insert(leaf,Object::Reference(id));}
        doc.catalog_mut().unwrap().set("Lang",id);let mut graph=CompositionGraph::new();graph.import(0,doc,&mut check).unwrap();let before=snapshot(&graph);
        let mut calls=0;let result=graph.source_languages(&mut ||{calls+=1;Ok(())});assert_eq!(result.is_ok(),depth==64&&!cycle);assert!(calls>depth);assert_eq!(snapshot(&graph),before);
    }
}

#[test]
fn annotation_receipts_direct_materialization_moves_payloads_and_rolls_back_on_limits() {
    use crate::composition_annotation_receipts::SourceSlot;
    fn input()->Document {
        let mut doc=Document::with_version("1.7");let tree=doc.new_object_id();
        let annotation=dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![0.into(),0.into(),20.into(),20.into()],"Contents"=>Object::string_literal(vec![b'x';4096])};
        let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()],"Annots"=>vec![Object::Dictionary(annotation)],"Resources"=>dictionary!{}});
        doc.objects.insert(tree,dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}.into());let root=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree});doc.trailer.set("Root",root);doc
    }
    let slot=SourceSlot{page_index:0,slot:0,annotation_id:None};
    for mode in 0..5 {
        let mut graph=CompositionGraph::new();graph.import(0,input(),&mut check).unwrap();
        let page=graph.source_pages(0,&mut check).unwrap()[0];
        let pointer=graph.document.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_dict().unwrap().get(b"Contents").unwrap().as_str().unwrap().as_ptr();
        match mode {1=>graph.limits.objects=graph.usage.objects,2=>graph.limits.values=graph.usage.values,3=>graph.annotation_metadata=1,4=>graph.document.max_id=u32::MAX,_=>()}
        let before=snapshot(&graph);let metadata=graph.annotation_metadata;
        let result=graph.source_annotations(0,&[slot],&mut check);
        if mode==0 {
            let annotation=result.unwrap()[0].annotation;
            assert_eq!(graph.document.get_dictionary(annotation).unwrap().get(b"Contents").unwrap().as_str().unwrap().as_ptr(),pointer,"direct payload must move without cloning");
            assert_eq!(graph.usage.objects,before.2.objects+1);assert_eq!(graph.usage.values,before.2.values+1);assert_eq!(graph.usage.bytes,before.2.bytes);
        }else{assert!(result.is_err());assert_eq!(snapshot(&graph),before);assert_eq!(graph.annotation_metadata,metadata);assert!(graph.normalized_annotations.is_empty());}
    }
    let mut count=0;let mut graph=CompositionGraph::new();graph.import(0,input(),&mut check).unwrap();graph.source_annotations(0,&[slot],&mut ||{count+=1;Ok(())}).unwrap();
    for stop in [1,count/2,count] {
        let mut graph=CompositionGraph::new();graph.import(0,input(),&mut check).unwrap();let before=snapshot(&graph);let mut calls=0;
        assert!(graph.source_annotations(0,&[slot],&mut ||{calls+=1;if calls==stop{Err(bad("cancelled"))}else{Ok(())}}).is_err());assert_eq!(snapshot(&graph),before);assert!(graph.normalized_annotations.is_empty());
    }
}

#[test]
fn composition_graph_generated_catalog_objects_consume_the_retained_allowance() {
    let mut source=Document::with_version("1.7");
    let pages=source.new_object_id();
    let page=source.add_object(dictionary!{"Type"=>"Page","Parent"=>pages});
    source.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)],"MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()],"Rotate"=>90,"Resources"=>dictionary!{}}.into());
    let catalog=source.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});
    source.trailer.set("Root",catalog);
    for sources in [1,2] {
        let generated=if sources==1{1}else{2};
        for allowance in [generated-1,generated] {
            let mut graph=CompositionGraph::new();for index in 0..sources{graph.import(index,source.clone(),&mut check).unwrap();}
            let first=graph.source_pages(0,&mut check).unwrap()[0];let root=graph.document.get_dictionary(first).unwrap().get(b"Parent").unwrap().as_reference().unwrap();
            let original=graph.document.objects[&root].clone();let count=graph.document.objects.len();graph.limits.objects=count+allowance;
            let result=graph.assemble(&(0..sources).collect::<Vec<_>>(),&mut check);
            if allowance<generated {assert!(result.unwrap_err().to_string().contains("preservation limits"));}
            else {
                let result=result.unwrap();assert_eq!(result.objects.len(),count+generated);
                if sources==1{assert_eq!(result.catalog().unwrap().get(b"Pages").unwrap().as_reference().unwrap(),root);assert_eq!(result.objects[&root],original,"single-source inherited page context stays exact");}
            }
        }
    }
}

fn source() -> Document {
    let mut doc = Document::with_version("2.0");
    doc.objects.insert((9, 2), Object::Dictionary(dictionary! {
        "First" => (3, 0), "Prev" => (9, 2), "Parent" => (3, 0),
        "Nested" => vec![Object::Reference((3, 0)), Object::Reference((9, 2))],
    }));
    doc.objects.insert((3, 0), Object::Dictionary(dictionary! {
        "Back" => (9, 2), "Alias" => (3, 0),
    }));
    // Same number, different generation: must remain distinct.
    doc.objects.insert((9, 0), Object::Integer(73));
    doc.trailer.set("Root", (9, 2));
    doc.max_id = 9;
    doc
}

#[test]
fn relocates_all_aliases_cycles_generations_and_trailer_without_source_collisions() {
    let mut graph = CompositionGraph::new();
    graph.import(10, source(), &mut check).unwrap();
    graph.import(20, source(), &mut check).unwrap();
    assert_eq!(graph.document.objects.len(), 6);
    for key in [10, 20] {
        let imported = &graph.sources[&key];
        let a = imported.ids[&(9, 2)];
        let b = imported.ids[&(3, 0)];
        assert_ne!(a, imported.ids[&(9, 0)]);
        assert_eq!(imported.version, "2.0");
        assert_eq!(imported.trailer.get(b"Root").unwrap().as_reference().unwrap(), a);
        let dict = graph.document.get_dictionary(a).unwrap();
        for name in [b"First".as_slice(), b"Parent"] {
            assert_eq!(dict.get(name).unwrap().as_reference().unwrap(), b);
        }
        assert_eq!(dict.get(b"Prev").unwrap().as_reference().unwrap(), a);
        assert_eq!(dict.get(b"Nested").unwrap().as_array().unwrap(), &vec![Object::Reference(b), Object::Reference(a)]);
        assert_eq!(graph.document.get_dictionary(b).unwrap().get(b"Back").unwrap().as_reference().unwrap(), a);
    }
    assert!(graph.sources[&10].ids.values().all(|id| !graph.sources[&20].ids.values().any(|other| id == other)));
}

#[test]
fn moves_encoded_stream_payload_and_rewrites_its_dictionary_without_stale_offsets() {
    let mut doc = source();
    let mut stream = Stream::new(dictionary! {"Owner" => (9, 2), "Filter" => "FlateDecode"}, vec![7, 2, 9, 4]);
    stream.allows_compression = false;
    stream.start_position = Some(192);
    let pointer = stream.content.as_ptr();
    doc.objects.insert((12, 0), Object::Stream(stream));
    let mut graph = CompositionGraph::new();
    graph.import(0, doc, &mut check).unwrap();
    let ids = &graph.sources[&0].ids;
    let moved = graph.document.get_object(ids[&(12, 0)]).unwrap().as_stream().unwrap();
    assert_eq!(moved.content, [7, 2, 9, 4]);
    assert_eq!(moved.content.as_ptr(), pointer, "the payload must be moved, not copied");
    assert!(!moved.allows_compression);
    assert_eq!(moved.start_position, None);
    assert_eq!(moved.dict.get(b"Owner").unwrap().as_reference().unwrap(), ids[&(9, 2)]);
    assert_eq!(moved.dict.get(b"Filter").unwrap().as_name().unwrap(), b"FlateDecode");
}

type SourceSnapshot = Vec<(usize, BTreeMap<ObjectId, ObjectId>, Dictionary, String)>;
type ParserSnapshot = (usize, usize, usize, usize, usize);
fn snapshot(graph: &CompositionGraph) -> (BTreeMap<ObjectId, Object>, u32, Usage, SourceSnapshot, ParserSnapshot) {
    let sources = graph.sources.iter().map(|(key, source)| (
        *key, source.ids.clone(), source.trailer.clone(), source.version.clone(),
    )).collect();
    let p = graph.parser_remaining;
    (graph.document.objects.clone(), graph.document.max_id, graph.usage, sources,
        (p.input_bytes, p.decoded_bytes, p.object_bytes, p.object_nodes, p.xref_entries))
}

#[test]
fn refuses_dangling_trailer_unreachable_and_generation_references_transactionally() {
    let mut graph = CompositionGraph::new();
    graph.import(0, source(), &mut check).unwrap();
    let before = snapshot(&graph);
    for place in 0..3 {
        let mut invalid = source();
        match place {
            0 => invalid.trailer.set("Info", (500, 0)),
            1 => { invalid.objects.insert((100, 0), Object::Reference((500, 0))); },
            _ => { invalid.objects.insert((100, 0), Object::Reference((9, 1))); },
        }
        assert!(graph.import(1, invalid, &mut check).is_err());
        assert_eq!(snapshot(&graph), before);
    }
    assert!(graph.import(0, source(), &mut check).is_err());
    assert_eq!(snapshot(&graph), before);
}

#[test]
fn cancellation_at_each_checkpoint_leaves_prior_sources_unchanged() {
    let mut completed = false;
    for stop in 0..200 {
        let mut graph = CompositionGraph::new();
        graph.import(0, source(), &mut check).unwrap();
        let before = snapshot(&graph);
        let mut calls = 0;
        let mut incoming = source();
        let mut stream = Stream::new(dictionary! {"Owner" => (9, 2)}, vec![1, 2, 3]);
        stream.start_position = Some(200);
        incoming.objects.insert((12, 0), Object::Stream(stream));
        let result = graph.import(1, incoming, &mut || {
            calls += 1;
            if calls > stop { Err(EngineError::InvalidRequest("cancelled by test".into())) } else { Ok(()) }
        });
        if result.is_ok() {
            assert!(stop > 10, "must check inside graph traversal, not only at entry");
            completed = true;
            break;
        }
        assert!(result.unwrap_err().to_string().contains("cancelled by test"));
        assert_eq!(snapshot(&graph), before, "cancel checkpoint {stop}");
    }
    assert!(completed);
}

#[test]
fn aggregate_object_value_and_payload_budgets_do_not_reset_for_new_sources() {
    let mut measured = CompositionGraph::new();
    measured.import(0, source(), &mut check).unwrap();
    let used = measured.usage;
    assert_eq!(used.objects, 3);
    assert!(used.values > 10 && used.bytes > 20, "count nested values and dictionary keys");
    for dimension in 0..3 {
        let mut graph = CompositionGraph::new();
        match dimension {
            0 => graph.limits.objects = used.objects,
            1 => graph.limits.values = used.values,
            _ => graph.limits.bytes = used.bytes,
        }
        graph.import(0, source(), &mut check).unwrap();
        let before = snapshot(&graph);
        assert!(graph.import(1, source(), &mut check).is_err());
        assert_eq!(snapshot(&graph), before);
    }
}

#[test]
fn direct_depth_and_individual_stream_limits_accept_endpoints_and_reject_overflow() {
    for (depth, expected) in [(4, true), (5, false)] {
        let mut doc = Document::with_version("1.7");
        let mut value = Object::Integer(42);
        for _ in 0..depth { value = Object::Array(vec![value]); }
        doc.objects.insert((1, 0), value);
        let mut graph = CompositionGraph::new();
        graph.limits.depth = 4;
        assert_eq!(graph.import(0, doc, &mut check).is_ok(), expected);
    }
    for (size, expected) in [(8, true), (9, false)] {
        let mut doc = Document::with_version("1.7");
        doc.objects.insert((1, 0), Object::Stream(Stream::new(dictionary! {}, vec![0; size])));
        let mut graph = CompositionGraph::new();
        graph.limits.stream_bytes = 8;
        assert_eq!(graph.import(0, doc, &mut check).is_ok(), expected);
    }
}

#[test]
fn rejects_nonfinite_values_object_zero_and_id_exhaustion_without_mutation() {
    for invalid in [Object::Real(f32::NAN), Object::Real(f32::INFINITY)] {
        let mut doc = source();
        doc.objects.insert((15, 0), invalid);
        let mut graph = CompositionGraph::new();
        assert!(graph.import(0, doc, &mut check).is_err());
        assert_eq!(graph.usage, Usage::default());
        assert!(graph.document.objects.is_empty());
    }
    let mut doc = source();
    doc.objects.insert((0, 0), Object::Null);
    let mut graph = CompositionGraph::new();
    assert!(graph.import(0, doc, &mut check).is_err());
    graph.document.max_id = u32::MAX - 2;
    let before = snapshot(&graph);
    assert!(graph.import(0, source(), &mut check).is_err());
    assert_eq!(snapshot(&graph), before);
}

#[test]
fn independently_calculated_limits_include_keys_strings_names_and_aggregate_stream_bytes() {
    fn tokens() -> Document {
        let mut doc = Document::with_version("1.7");
        doc.objects.insert((1, 0), Object::Dictionary(dictionary! {
            "A" => Object::Name(b"BC".to_vec()),
            "DEF" => Object::string_literal("GHIJ"),
        }));
        doc
    }
    // 3 version bytes + keys (1+3) + name (2) + string (4) = 13;
    // trailer dictionary + object dictionary + name + string = 4 values.
    let mut graph = CompositionGraph::new();
    graph.limits.bytes = 13;
    graph.limits.values = 4;
    graph.limits.objects = 1;
    graph.import(0, tokens(), &mut check).unwrap();
    assert_eq!(graph.usage, Usage { objects: 1, values: 4, bytes: 13 });
    for dimension in 0..3 {
        let mut graph = CompositionGraph::new();
        match dimension {
            0 => graph.limits.objects = 0,
            1 => graph.limits.values = 3,
            _ => graph.limits.bytes = 12,
        }
        assert!(graph.import(0, tokens(), &mut check).is_err());
        assert_eq!(graph.usage, Usage::default());
    }
    for (limit, accepted) in [(34, true), (33, false)] {
        let mut graph = CompositionGraph::new();
        graph.limits.bytes = limit;
        graph.limits.stream_bytes = 8;
        for key in 0..2 {
            let mut doc = Document::with_version("1.7");
            doc.objects.insert((1, 0), Object::Stream(Stream::new(dictionary! {}, vec![0; 8])));
            // Each source: version 3 + Length key 6 + stored payload 8 = 17.
            let before = snapshot(&graph);
            let result = graph.import(key, doc, &mut check);
            assert_eq!(result.is_ok(), key == 0 || accepted);
            if result.is_err() { assert_eq!(snapshot(&graph), before); }
        }
        assert_eq!(graph.usage.bytes, if accepted { 34 } else { 17 });
    }
}

#[test]
fn full_object_id_endpoint_is_usable_once_and_next_import_refuses_transactionally() {
    let mut graph = CompositionGraph::new();
    graph.document.max_id = u32::MAX - 3;
    graph.import(0, source(), &mut check).unwrap();
    assert_eq!(graph.document.max_id, u32::MAX);
    assert_eq!(graph.sources[&0].ids.values().copied().collect::<Vec<_>>(),
        vec![(u32::MAX - 2, 0), (u32::MAX - 1, 0), (u32::MAX, 0)]);
    let before = snapshot(&graph);
    assert!(graph.import(1, source(), &mut check).is_err());
    assert_eq!(snapshot(&graph), before);
}

fn serialized_source(modern: bool) -> Vec<u8> {
    let mut doc = source();
    // A serialized PDF's xref can name only one live generation per number.
    // Keep the referenced generation 2; the abstract relocation test separately
    // exercises disjoint (number, generation) keys in an in-memory graph.
    doc.objects.remove(&(9, 0));
    doc.add_object(Object::string_literal(vec![b'A'; 16 * 1024]));
    let mut bytes = vec![];
    if modern {
        doc.save_with_options(&mut bytes, lopdf::SaveOptions {
            use_object_streams: true, use_xref_streams: true, ..Default::default()
        }).unwrap();
    } else { doc.save_to(&mut bytes).unwrap(); }
    bytes
}

#[test]
fn bounded_composition_accepts_large_encoded_sources_without_expanding_object_allowance() {
    let mut doc = source();
    doc.objects.remove(&(9, 0));
    let payload_len = 33 * 1024 * 1024;
    let first = doc.add_object(Stream::new(dictionary! {}, vec![17; payload_len]));
    let second = doc.add_object(Stream::new(dictionary! {}, vec![31; payload_len]));
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    drop(doc);
    assert!(bytes.len() > 64 * 1024 * 1024);

    let mut graph = CompositionGraph::new();
    let input_allowance = graph.parser_remaining.input_bytes;
    let object_allowance = graph.parser_remaining.object_bytes;
    graph.import_bytes(0, &bytes, Arc::new(|| false)).unwrap();
    assert_eq!(graph.parser_remaining.input_bytes, input_allowance - bytes.len());
    for (id, expected) in [(first, 17), (second, 31)] {
        let relocated = graph.sources[&0].ids[&id];
        let content = &graph.document.get_object(relocated).unwrap().as_stream().unwrap().content;
        assert_eq!(content.len(), payload_len);
        assert!(content.iter().all(|value| *value == expected));
    }
    assert!(graph.parser_remaining.object_bytes < object_allowance - payload_len * 2);
    let before = snapshot(&graph);
    let failure = graph.import_bytes(1, &bytes, Arc::new(|| false)).unwrap_err();
    assert!(failure.to_string().contains("object bytes"), "{failure}");
    assert_eq!(snapshot(&graph), before, "refusal must preserve graph and allowances");
}

#[test]
fn bounded_composition_sources_share_input_and_decode_allowances_without_partial_import() {
    for modern in [false, true] {
        let bytes = serialized_source(modern);
        let mut graph = CompositionGraph::new();
        graph.parser_remaining.input_bytes = bytes.len() * 2 - 1;
        graph.import_bytes(0, &bytes, Arc::new(|| false)).unwrap();
        assert_eq!(graph.parser_remaining.input_bytes, bytes.len() - 1);
        let before = snapshot(&graph);
        assert!(graph.import_bytes(1, &bytes, Arc::new(|| false)).is_err());
        assert_eq!(snapshot(&graph), before);
    }
    let bytes = serialized_source(true);
    let mut graph = CompositionGraph::new();
    graph.parser_remaining.decoded_bytes = 24 * 1024;
    graph.import_bytes(0, &bytes, Arc::new(|| false)).unwrap();
    let before = snapshot(&graph);
    assert!(graph.import_bytes(1, &bytes, Arc::new(|| false)).is_err());
    assert_eq!(snapshot(&graph), before);
}

#[test]
fn bounded_composition_cancel_after_parsing_rolls_back_graph_and_allowances() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let bytes = serialized_source(false);
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    Document::load_mem_with_resource_control(&bytes, lopdf::ResourceLimits::default(), Arc::new(move || {
        calls.fetch_add(1, Ordering::Relaxed); false
    })).unwrap();
    let after_parse = count.load(Ordering::Relaxed) + 4;
    let mut graph = CompositionGraph::new();
    graph.import_bytes(0, &bytes, Arc::new(|| false)).unwrap();
    let before = snapshot(&graph);
    for stop in [0, after_parse] {
        let calls = Arc::new(AtomicUsize::new(0));
        assert!(graph.import_bytes(1, &bytes, Arc::new(move || {
            calls.fetch_add(1, Ordering::Relaxed) >= stop
        })).is_err());
        assert_eq!(snapshot(&graph), before);
    }
    graph.import_bytes(1, &bytes, Arc::new(|| false)).unwrap();
    assert!(graph.sources[&0].ids.values().all(|id| !graph.sources[&1].ids.values().any(|other| other == id)));
}
#[test]
fn structured_overlay_publish_combined_capacity_is_reserved_before_attachment() {
    let mut destination=Document::with_version("1.7");
    destination.add_object(Object::string_literal(vec![b'a';8]));
    let mut generated=Document::with_version("1.7");
    let root=generated.add_object(Object::string_literal(vec![b'b';8]));
    let ids=BTreeSet::from([root]);
    for limits in [
        Limits{objects:1,..Limits::default()},
        Limits{values:2,..Limits::default()},
        Limits{bytes:11,..Limits::default()},
    ] {
        validate_retained_with_limits(&destination,limits,&mut check).unwrap();
        validate_retained_with_limits(&generated,limits,&mut check).unwrap();
        assert!(validate_overlay_additions_with_limits(&destination,&generated,&ids,&[],limits,&mut check).is_err());
    }
    validate_overlay_additions_with_limits(&destination,&generated,&ids,&[],Limits{objects:2,values:3,bytes:19,..Limits::default()},&mut check).unwrap();
    let before=(destination.objects.clone(),generated.objects.clone());
    assert!(validate_overlay_additions(&destination,&generated,&ids,&[],&mut ||Err(bad("cancelled"))).is_err());
    assert_eq!((destination.objects,generated.objects),before);
}

#[test]
fn structured_overlay_publish_shared_lists_and_streams_consume_the_aggregate_budget() {
    let old=vec![Object::string_literal(vec![b'a';8])];
    let mut destination=Document::with_version("1.7");
    destination.add_object(Object::Array(old.clone()));
    let mut generated=Document::with_version("1.7");
    let root=generated.add_object(Object::string_literal(vec![b'b';8]));
    let ids=BTreeSet::from([root]);
    for (bytes,accepted) in [(27,true),(26,false)] {
        let result=validate_overlay_additions_with_limits(&destination,&generated,&ids,&[(&old,1)],Limits{bytes,values:7,..Limits::default()},&mut check);
        assert_eq!(result.is_ok(),accepted);
    }
    assert!(validate_overlay_additions_with_limits(&destination,&generated,&ids,&[(&old,1)],Limits{values:6,..Limits::default()},&mut check).is_err());
    generated.objects.insert(root,Object::Stream(Stream::new(dictionary!{},vec![0;8])));
    for (stream_bytes,accepted) in [(8,true),(7,false)] {
        assert_eq!(validate_overlay_additions_with_limits(&destination,&generated,&ids,&[],Limits{stream_bytes,..Limits::default()},&mut check).is_ok(),accepted);
    }
}
