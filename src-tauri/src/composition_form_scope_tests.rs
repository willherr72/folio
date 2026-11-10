use super::*;
use lopdf::{dictionary,Dictionary,Object,Stream};
fn scan(doc:&Document,root:ObjectId)->EngineResult<BTreeSet<Vec<u8>>> {
    reserve_names(doc,&[root],&mut Budget::default(),Arc::new(||false),&mut ||Ok(()))
}
fn stream(doc:&mut Document,dict:Dictionary,bytes:&[u8])->ObjectId {doc.add_object(Stream::new(dict,bytes.to_vec()))}
fn has(names:&BTreeSet<Vec<u8>>,values:&[&[u8]]) {for name in values {assert!(names.contains(*name),"missing {name:?}");}}

#[test]
fn composition_form_scope_reserves_split_page_tokens_and_binary_names_without_mutation() {
    let mut doc=Document::with_version("1.7");
    let a=stream(&mut doc,Dictionary::new(),b"BT /Folio2_");
    let b=stream(&mut doc,Dictionary::new(),b"F1 12 Tf (hello) Tj ET /A#FF 2 Tf");
    let root=doc.add_object(dictionary!{"Type"=>"Page","Contents"=>vec![Object::Reference(a),Object::Reference(b)],
        "Resources"=>dictionary!{"Font"=>dictionary!{"UnusedLocal"=>Object::Null}}});
    let before=doc.objects.clone();let names=scan(&doc,root).unwrap();
    has(&names,&[b"Folio2_F1",b"A\xff",b"UnusedLocal"]);assert!(!names.contains(b"Folio2_".as_slice()));
    assert_eq!(doc.objects,before);
}

#[test]
fn composition_form_scope_covers_nested_forms_patterns_type3_and_all_appearance_states() {
    let mut doc=Document::with_version("1.7");
    let form=stream(&mut doc,dictionary!{"Subtype"=>"Form"},b"/Nested 9 Tf");
    let pattern=stream(&mut doc,dictionary!{"PatternType"=>1},b"/Tiling 7 Tf");
    let glyph=stream(&mut doc,Dictionary::new(),b"/GlyphFont 8 Tf");
    let normal=stream(&mut doc,Dictionary::new(),b"/Normal 9 Tf");
    let rollover=stream(&mut doc,Dictionary::new(),b"/Rollover 9 Tf");
    let down=stream(&mut doc,Dictionary::new(),b"/DownState 9 Tf");
    let image=stream(&mut doc,dictionary!{"Subtype"=>"Image"},b"not a content program \xff");
    let binary=stream(&mut doc,Dictionary::new(),b"invalid font binary \xff");
    let root=doc.add_object(dictionary!{"Type"=>"Page","Resources"=>dictionary!{
        "XObject"=>dictionary!{"Fm"=>form,"Im"=>image},"Pattern"=>dictionary!{"P"=>pattern},
        "Font"=>dictionary!{"T3"=>dictionary!{"Subtype"=>"Type3","CharProcs"=>dictionary!{"a"=>glyph}},
            "BinaryFont"=>dictionary!{"FontDescriptor"=>dictionary!{"FontFile2"=>binary}}}},
        "Annots"=>vec![Object::Dictionary(dictionary!{"Subtype"=>"Widget","AP"=>dictionary!{
            "N"=>normal,"R"=>rollover,"D"=>dictionary!{"Off"=>down}}})]});
    has(&scan(&doc,root).unwrap(),&[b"Nested",b"Tiling",b"GlyphFont",b"Normal",b"Rollover",b"DownState"]);
}

#[test]
fn composition_form_scope_uses_role_after_shared_object_was_already_visited() {
    let mut doc=Document::with_version("1.7");
    let ap=stream(&mut doc,Dictionary::new(),b"/PreviouslyUnbound 10 Tf");
    let alias=doc.add_object(Object::Reference(ap));
    let root=doc.add_object(dictionary!{"Subtype"=>"Widget","EarlierOpaqueEdge"=>ap,"AP"=>dictionary!{"N"=>alias}});
    has(&scan(&doc,root).unwrap(),&[b"PreviouslyUnbound"]);
}

#[test]
fn composition_form_scope_handles_graph_cycles_and_reuses_stream_proof() {
    let mut doc=Document::with_version("1.7");
    let ap=stream(&mut doc,Dictionary::new(),b"/Shared 10 Tf");
    let root=doc.add_object(dictionary!{"Subtype"=>"Widget","AP"=>dictionary!{"N"=>ap,"R"=>ap,"D"=>ap}});
    doc.objects.get_mut(&root).unwrap().as_dict_mut().unwrap().set("Parent",root);
    let mut budget=Budget::default();budget.parser.input_bytes=b"/Shared 10 Tf".len();
    let names=reserve_names(&doc,&[root],&mut budget,Arc::new(||false),&mut ||Ok(())).unwrap();
    has(&names,&[b"Shared"]);assert_eq!(budget.parser.input_bytes,0);
}

#[test]
fn composition_form_scope_refuses_unproved_content_and_obeys_shared_limits_and_cancel() {
    let mut doc=Document::with_version("1.7");
    let ap=stream(&mut doc,Dictionary::new(),b"/Safe 10 Tf");
    let root=doc.add_object(dictionary!{"Subtype"=>"Widget","AP"=>dictionary!{"N"=>ap}});
    let mut budget=Budget::default();budget.metadata=1;
    assert!(reserve_names(&doc,&[root],&mut budget,Arc::new(||false),&mut ||Ok(())).is_err());
    let mut budget=Budget::default();budget.parser.input_bytes=3;
    assert!(reserve_names(&doc,&[root],&mut budget,Arc::new(||false),&mut ||Ok(())).is_err());
    assert!(reserve_names(&doc,&[root],&mut Budget::default(),Arc::new(||true),&mut ||Ok(())).is_err());
    for bytes in [b"BT (unterminated".as_slice(),b"BI /W 1 /H 1 ID abc EI"] {
        doc.objects.get_mut(&ap).unwrap().as_stream_mut().unwrap().set_content(bytes.to_vec());
        assert!(scan(&doc,root).is_err());
    }
}

#[test]
fn composition_form_scope_rejects_alias_cycles_but_not_unreferenced_payloads() {
    let mut doc=Document::with_version("1.7");
    let root=doc.add_object(Dictionary::new());
    stream(&mut doc,dictionary!{"Subtype"=>"Form"},b"(unterminated");
    assert!(scan(&doc,root).is_ok());
    let alias=doc.add_object(Object::Reference((99,0)));
    doc.objects.insert((99,0),Object::Reference(alias));
    doc.objects.get_mut(&root).unwrap().as_dict_mut().unwrap().set("AP",dictionary!{"N"=>alias});
    assert!(scan(&doc,root).is_err());
}

#[test]
fn composition_form_scope_distinguishes_annotation_programs_from_opaque_metadata() {
    let mut doc=Document::with_version("1.7");
    let ap=stream(&mut doc,Dictionary::new(),b"/TextNoteFont 10 Tf");
    let annot=doc.add_object(dictionary!{"Subtype"=>"Text","AP"=>dictionary!{"N"=>ap}});
    let root=doc.add_object(dictionary!{"Type"=>"Page","Annots"=>vec![Object::Reference(annot)],
        "PrivateMetadata"=>dictionary!{"AP"=>Object::string_literal("opaque, not an appearance dictionary")}});
    has(&scan(&doc,root).unwrap(),&[b"TextNoteFont"]);
}

#[test]
fn composition_form_scope_bounds_compressed_and_aggregate_content_before_parsing() {
    let mut doc=Document::with_version("1.7");
    let data=b"/Compressed 10 Tf\n".repeat(1000);
    let mut compressed=Stream::new(Dictionary::new(),data);compressed.compress().unwrap();
    assert!(compressed.dict.has(b"Filter"));let ap=doc.add_object(compressed);
    let root=doc.add_object(dictionary!{"Subtype"=>"Widget","AP"=>dictionary!{"N"=>ap}});
    has(&scan(&doc,root).unwrap(),&[b"Compressed"]);
    let mut budget=Budget::default();budget.parser.input_bytes=20;
    assert!(reserve_names(&doc,&[root],&mut budget,Arc::new(||false),&mut ||Ok(())).is_err());
    let other=stream(&mut doc,dictionary!{"Subtype"=>"Form"},b"/Second 10 Tf");
    doc.objects.get_mut(&root).unwrap().as_dict_mut().unwrap().set("Other",other);
    let mut budget=Budget::default();budget.parser.input_bytes=b"/Compressed 10 Tf\n".len()*1000;
    assert!(reserve_names(&doc,&[root],&mut budget,Arc::new(||false),&mut ||Ok(())).is_err());
}
