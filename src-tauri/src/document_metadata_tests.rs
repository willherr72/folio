use super::*;
use lopdf::{dictionary, Stream};

fn fixture(claims: bool, compressed: bool) -> (Document,ObjectId,ObjectId,ObjectId) {
    let mut doc=Document::with_version("1.7");
    let title=doc.add_object(Object::string_literal("ordinary title"));
    let info=doc.add_object(dictionary!{"Title"=>title,"Company"=>Object::string_literal("Keep custom Info")});
    if claims {doc.get_dictionary_mut(info).unwrap().set("GTS_PDFXVersion",Object::string_literal("PDF/X-4"));}
    let mut xml=format!("<x:xmpmeta xmlns:x='adobe:ns:meta/' xmlns:a='http://www.aiim.org/pdfa/ns/id/' xmlns:v='urn:custom' {}><v:Title>Résumé 文書</v:Title></x:xmpmeta>",if claims {"a:part='2'"}else{""});
    if compressed {xml.push_str(&" ".repeat(256));}
    let mut stream=Stream::new(dictionary!{"Type"=>"Metadata","Subtype"=>"XML","Custom"=>title},xml.into_bytes());
    if compressed {stream.compress().unwrap();assert!(stream.dict.has(b"Filter"));}
    let metadata=doc.add_object(stream);
    let root=doc.add_object(dictionary!{"Type"=>"Catalog","Metadata"=>metadata});
    doc.trailer.set("Root",root);doc.trailer.set("Info",info);
    (doc,root,info,metadata)
}
fn bytes(doc:&Document,value:&Object)->Vec<u8> {doc.get_object(value.as_reference().unwrap()).unwrap().as_stream().unwrap().decompressed_content_strict_with_limit(4*1024*1024).unwrap()}

#[test]
fn clean_local_metadata_and_info_keep_exact_source_identities() {
    let (doc,root,info,metadata)=fixture(false,true);
    let prepared=prepare(&doc,root,Some(&Object::Reference(info)),doc.max_id,false,&mut ||Ok(())).unwrap();
    assert_eq!(prepared.metadata,Some(metadata.into()));assert_eq!(prepared.info,Some(info.into()));assert!(prepared.objects.is_empty());
}
#[test]
fn filtered_local_roots_are_private_and_other_aliases_stay_unchanged() {
    let (mut doc,root,info,metadata)=fixture(true,true);
    let original_metadata=doc.get_object(metadata).unwrap().clone();let original_info=doc.get_object(info).unwrap().clone();
    let prepared=prepare(&doc,root,Some(&Object::Reference(info)),doc.max_id,false,&mut ||Ok(())).unwrap();
    let (new_metadata,new_info)=prepared.install(&mut doc);
    assert_ne!(new_metadata,Some(metadata.into()));assert_ne!(new_info,Some(info.into()));
    assert_eq!(doc.get_object(metadata).unwrap(),&original_metadata);assert_eq!(doc.get_object(info).unwrap(),&original_info);
    let xml=String::from_utf8(bytes(&doc,new_metadata.as_ref().unwrap())).unwrap();
    assert!(!xml.contains("a:part"));assert!(xml.contains("Résumé 文書"));
    let saved_info=doc.get_dictionary(new_info.unwrap().as_reference().unwrap()).unwrap();
    assert!(!saved_info.has(b"GTS_PDFXVersion"));assert!(saved_info.has(b"Title")&&saved_info.has(b"Company"));
}
#[test]
fn importing_clean_metadata_relocates_shared_custom_references_once() {
    let (source,root,info,metadata)=fixture(false,true);
    let prepared=prepare(&source,root,Some(&Object::Reference(info)),100,true,&mut ||Ok(())).unwrap();
    let mut output=Document::with_version("1.7");output.max_id=100;
    let (new_metadata,new_info)=prepared.install(&mut output);
    let new_metadata=new_metadata.unwrap();let new_info=new_info.unwrap();
    assert_eq!(bytes(&source,&metadata.into()),bytes(&output,&new_metadata));
    let stream=output.get_object(new_metadata.as_reference().unwrap()).unwrap().as_stream().unwrap();
    let old_stream=source.get_object(metadata).unwrap().as_stream().unwrap();assert_eq!(stream.content,old_stream.content);assert_eq!(stream.dict.get(b"Filter").unwrap(),old_stream.dict.get(b"Filter").unwrap());
    let title=output.get_dictionary(new_info.as_reference().unwrap()).unwrap().get(b"Title").unwrap();
    assert_eq!(title,stream.dict.get(b"Custom").unwrap());assert!(title.as_reference().unwrap().0>100);
    assert_eq!(output.get_object(title.as_reference().unwrap()).unwrap().as_str().unwrap(),b"ordinary title");
}
#[test]
fn malformed_compression_xml_and_budget_fail_without_mutating_source() {
    for case in 0..5 {
        let (mut doc,root,info,metadata)=fixture(true,false);
        let stream=doc.get_object_mut(metadata).unwrap().as_stream_mut().unwrap();
        match case {
            0=>{stream.content=b"<broken>".to_vec();},
            1=>{stream.dict.set("Filter",Object::Name(b"Unknown".to_vec()));},
            2=>{stream.content=vec![b' ';4*1024*1024+1];},
            3=>{stream.content=vec![b' ';4*1024*1024+1];stream.compress().unwrap();},
            _=>{stream.dict.set("Custom",Object::string_literal(vec![b'x';8*1024*1024+1]));},
        }
        let before=format!("{:?}",doc.objects);
        assert!(prepare(&doc,root,Some(&info.into()),doc.max_id,true,&mut ||Ok(())).is_err(),"case {case}");
        assert_eq!(format!("{:?}",doc.objects),before);
    }
}
#[test]
fn cancellation_and_id_overflow_do_not_publish_staged_metadata() {
    let (doc,root,info,_)=fixture(true,true);let mut total=0;
    prepare(&doc,root,Some(&info.into()),100,true,&mut ||{total+=1;Ok(())}).unwrap();assert!(total>10);
    for stop in [1,total/2,total] {let mut calls=0;
        assert!(prepare(&doc,root,Some(&info.into()),100,true,&mut ||{calls+=1;if calls==stop{Err(crate::EngineError::InvalidRequest("cancelled".into()))}else{Ok(())}}).is_err());
        assert_eq!(calls,stop);
    }
    assert!(prepare(&doc,root,Some(&info.into()),u32::MAX,true,&mut ||Ok(())).is_err());
}
#[test]
fn imported_cyclic_custom_values_are_finite_and_repeated_filtering_stabilizes() {
    let (mut doc,root,info,_)=fixture(true,false);
    let custom=doc.add_object(dictionary!{});doc.get_dictionary_mut(custom).unwrap().set("Self",custom);doc.get_dictionary_mut(info).unwrap().set("Custom",custom);
    let prepared=prepare(&doc,root,Some(&info.into()),100,true,&mut ||Ok(())).unwrap();
    let mut output=Document::with_version("1.7");output.max_id=100;let(metadata,info)=prepared.install(&mut output);
    let root=output.add_object(dictionary!{"Type"=>"Catalog","Metadata"=>metadata.unwrap()});
    let again=prepare(&output,root,info.as_ref(),output.max_id,false,&mut ||Ok(())).unwrap();assert!(again.objects.is_empty());assert_eq!(again.info,info);
    let custom=output.get_dictionary(info.unwrap().as_reference().unwrap()).unwrap().get(b"Custom").unwrap().as_reference().unwrap();
    assert_eq!(output.get_dictionary(custom).unwrap().get(b"Self").unwrap(),&Object::Reference(custom));
}

#[test]
fn external_stream_descriptors_are_refused_but_null_optional_entries_are_retained() {
    for key in [b"F".as_slice(),b"FFilter",b"FDecodeParms"] {
        for indirect in [false,true] {
            let(mut doc,root,info,metadata)=fixture(false,false);
            let descriptor=if key==b"F" {Object::string_literal("outside.xml")} else if key==b"FFilter" {Object::Name(b"ASCIIHexDecode".to_vec())} else {Object::Dictionary(dictionary!{})};
            let descriptor=if indirect {Object::Reference(doc.add_object(descriptor))}else{descriptor};
            doc.get_object_mut(metadata).unwrap().as_stream_mut().unwrap().dict.set(key,descriptor);
            assert!(prepare(&doc,root,Some(&info.into()),100,true,&mut ||Ok(())).is_err());
            let null=doc.add_object(Object::Null);
            doc.get_object_mut(metadata).unwrap().as_stream_mut().unwrap().dict.set(key,if indirect {Object::Reference(null)}else{Object::Null});
            assert!(prepare(&doc,root,Some(&info.into()),100,true,&mut ||Ok(())).is_ok());
        }
    }
}
