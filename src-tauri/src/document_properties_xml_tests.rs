use super::*;
const PACKET: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><r:RDF xmlns:r="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><r:Description r:about="" xmlns:d="http://purl.org/dc/elements/1.1/" xmlns:p="http://ns.adobe.com/pdf/1.3/" xmlns:c="urn:custom" p:Keywords="old &amp; keywords"><d:title><r:Alt><r:li xml:lang="x-default">Old</r:li><r:li xml:lang="fr">Ancien</r:li></r:Alt></d:title><d:creator><r:Seq><r:li>First</r:li><r:li>Second</r:li></r:Seq></d:creator><c:Value c:q="keep">Résumé 文書</c:Value></r:Description></r:RDF></x:xmpmeta>"#;
#[test]
fn blank_subjects_do_not_alias_the_document_or_each_other() {
    for identity in ["r:nodeID='other'", ""] {
        for existing in ["", "<d:title>Document title</d:title>"] {
            let packet=format!("<r:RDF xmlns:r='{RDF}' xmlns:d='{DC}'><r:Description {identity}><d:title>Unrelated</d:title></r:Description><r:Description r:about=''>{existing}</r:Description></r:RDF>");
            let out=update(Some(packet.as_bytes()),&edit(),&mut||Ok(())).unwrap();
            let text=String::from_utf8(out).unwrap();let doc=roxmltree::Document::parse(&text).unwrap();
            let nodes:Vec<_>=doc.root_element().children().filter(|n|n.is_element()).collect();
            assert_eq!(nodes[0].children().find(|n|n.has_tag_name((DC,"title"))).unwrap().text(),Some("Unrelated"));
            assert!(nodes[1].descendants().any(|n|n.is_text()&&n.text()==Some("New <&> 文書")));
            assert_eq!(inspect(text.as_bytes()).unwrap().fields[0].as_deref(),Some("New <&> 文書"));
        }
    }
    let packet=format!("<r:RDF xmlns:r='{RDF}' xmlns:d='{DC}'><r:Description><d:title>First</d:title></r:Description><r:Description><d:title>Second</d:title></r:Description></r:RDF>");
    let out=update(Some(packet.as_bytes()),&edit(),&mut||Ok(())).unwrap();
    assert!(String::from_utf8(out).unwrap().contains("<d:title>Second</d:title>"));
    let named=format!("<r:RDF xmlns:r='{RDF}' xmlns:d='{DC}'><r:Description r:nodeID='same'><d:title>First</d:title></r:Description><r:Description r:nodeID='same'><d:creator>Kept author</d:creator></r:Description></r:RDF>");
    let out=update(Some(named.as_bytes()),&edit(),&mut||Ok(())).unwrap();assert_eq!(inspect(&out).unwrap().fields[1].as_deref(),Some("Kept author"));
    for identity in ["r:about='' r:nodeID='other'","r:ID='fragment'"] {
        let unsupported=format!("<r:RDF xmlns:r='{RDF}'><r:Description {identity}/></r:RDF>");
        assert!(update(Some(unsupported.as_bytes()),&edit(),&mut||Ok(())).is_err());
    }

}
fn edit() -> DocumentMetadataEdit {
    DocumentMetadataEdit::Update {
        clear_existing: false,
        title: Some("New <&> 文書".into()),
        author: None,
        subject: Some("".into()),
        keywords: Some("a & b".into()),
    }
}
#[test]
fn edits_defaults_and_attributes_without_losing_translations_or_custom_values() {
    let old = inspect(PACKET.as_bytes()).unwrap();
    assert_eq!(old.fields[0].as_deref(), Some("Old"));
    assert!(old.multiple_authors);
    let bytes = update(Some(PACKET.as_bytes()), &edit(), &mut || Ok(())).unwrap();
    let new = inspect(&bytes).unwrap();
    assert_eq!(
        new.fields,
        [
            Some("New <&> 文書".into()),
            Some("First; Second".into()),
            Some("".into()),
            Some("a & b".into())
        ]
    );
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("<r:li xml:lang=\"fr\">Ancien</r:li>"));
    assert!(text.contains("<c:Value c:q=\"keep\">Résumé 文書</c:Value>"));
}
#[test]
fn author_replacement_is_a_single_literal_and_keeps_qualified_neighbor() {
    let source = PACKET
        .replace(
            "<d:creator>",
            "<d:creator r:parseType=\"Resource\"><r:value>",
        )
        .replace(
            "</d:creator>",
            "</r:value><c:role>Writer</c:role></d:creator>",
        );
    let edit = DocumentMetadataEdit::Update {
        clear_existing: false,
        title: None,
        author: Some("A; B & C".into()),
        subject: None,
        keywords: None,
    };
    let bytes = update(Some(source.as_bytes()), &edit, &mut || Ok(())).unwrap();
    let data = inspect(&bytes).unwrap();
    assert_eq!(data.fields[1].as_deref(), Some("A; B & C"));
    assert!(!data.multiple_authors);
    assert!(String::from_utf8(bytes)
        .unwrap()
        .contains("<c:role>Writer</c:role>"));
}
#[test]
fn missing_empty_and_wide_packets_are_created_or_updated_with_valid_encoding() {
    for source in [
        None,
        Some(b"<x:xmpmeta xmlns:x='adobe:ns:meta/'/>".as_slice()),
    ] {
        let out = update(source, &edit(), &mut || Ok(())).unwrap();
        assert_eq!(
            inspect(&out).unwrap().fields[0].as_deref(),
            Some("New <&> 文書")
        );
    }
    for little in [false, true] {
        let text = format!("<?xml version='1.0' encoding='UTF-16'?>{PACKET}");
        let mut bytes = if little {
            vec![255, 254]
        } else {
            vec![254, 255]
        };
        for w in text.encode_utf16() {
            bytes.extend(if little {
                w.to_le_bytes()
            } else {
                w.to_be_bytes()
            });
        }
        let out = update(Some(&bytes), &edit(), &mut || Ok(())).unwrap();
        assert_eq!(
            inspect(&out).unwrap().fields[0].as_deref(),
            Some("New <&> 文書")
        );
        assert!(!String::from_utf8(out)
            .unwrap()
            .contains("encoding='UTF-16'"));
    }
}
#[test]
fn absent_default_and_empty_arrays_accept_a_new_default() {
    for source in [PACKET.replace("<r:li xml:lang=\"x-default\">Old</r:li>",""),PACKET.replace("<r:Alt><r:li xml:lang=\"x-default\">Old</r:li><r:li xml:lang=\"fr\">Ancien</r:li></r:Alt>","<r:Alt/>")]{let out=update(Some(source.as_bytes()),&edit(),&mut||Ok(())).unwrap();assert_eq!(inspect(&out).unwrap().fields[0].as_deref(),Some("New <&> 文書"));}
}
#[test]
fn ambiguous_targets_invalid_xml_and_cancellation_do_not_produce_output() {
    let duplicate=PACKET.replace("</r:Description>","<d:title><r:Alt><r:li xml:lang=\"x-default\">Duplicate</r:li></r:Alt></d:title></r:Description>");
    assert!(update(Some(duplicate.as_bytes()), &edit(), &mut || Ok(())).is_err());
    assert!(update(Some(b"broken"), &edit(), &mut || Ok(())).is_err());
    assert!(update(Some(PACKET.as_bytes()), &edit(), &mut || Err(bad(
        "cancelled"
    )))
    .is_err());
}
#[test]
fn aliases_self_closing_descriptions_and_other_rdf_subjects_are_preserved() {
    let packet = r#"<r:RDF xmlns:r="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><r:Description r:about="urn:another" xmlns:d="http://purl.org/dc/elements/1.1/" d:title="Other subject"/><r:Description r:about="" xmlns:d="http://purl.org/dc/elements/1.&#49;/" d:title="Document"/></r:RDF>"#;
    let out = update(Some(packet.as_bytes()), &edit(), &mut || Ok(())).unwrap();
    assert_eq!(
        inspect(&out).unwrap().fields[0].as_deref(),
        Some("New <&> 文書")
    );
    assert!(String::from_utf8(out)
        .unwrap()
        .contains("d:title=\"Other subject\""));
}
