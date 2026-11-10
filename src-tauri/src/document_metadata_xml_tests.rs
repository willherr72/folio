use super::*;

fn packet(body: &str) -> String {
    format!(r#"<?xpacket begin="﻿" id="W5M0MpCehiHzreSzNTczkc9d"?><x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">{body}</rdf:RDF></x:xmpmeta><?xpacket end="w"?>"#)
}
fn changed(input: &[u8]) -> String {
    String::from_utf8(filter(input,&mut ||Ok(())).unwrap().expect("claims must be removed")).unwrap()
}

#[test]
fn ordinary_multilingual_rights_and_custom_xmp_stay_byte_identical() {
    let xml=packet(r#"<rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:v="urn:custom"><dc:title><rdf:Alt><rdf:li xml:lang="x-default">Résumé &amp; résumé</rdf:li><rdf:li xml:lang="ja">文書</rdf:li></rdf:Alt></dc:title><dc:rights>Author rights</dc:rights><v:Nested rdf:parseType="Resource"><v:Value>📝</v:Value></v:Nested></rdf:Description>"#);
    assert!(filter(xml.as_bytes(),&mut ||Ok(())).unwrap().is_none());
}
#[test]
fn xmp_claim_attributes_and_elements_are_removed_by_normalized_namespace() {
    let xml=packet(r#"<rdf:Description rdf:about="" xmlns:a="http://www.aiim.org/pdfa/ns/&#x69;d/" xmlns:u="http://www.aiim.org/pdfua/ns/id/" xmlns:v="urn:custom" a:part="4" u:rev="2024" v:part="ordinary"><a:conformance>A</a:conformance><u:part>2</u:part><v:Keep>Important &amp; unchanged</v:Keep></rdf:Description>"#);
    let out=changed(xml.as_bytes());let doc=roxmltree::Document::parse(&out).unwrap();
    for node in doc.descendants().filter(|n|n.is_element()) {
        assert!(![Some("http://www.aiim.org/pdfa/ns/id/"),Some("http://www.aiim.org/pdfua/ns/id/")].contains(&node.tag_name().namespace()));
        assert!(node.attributes().all(|a|![Some("http://www.aiim.org/pdfa/ns/id/"),Some("http://www.aiim.org/pdfua/ns/id/")].contains(&a.namespace())));
    }
    assert!(out.contains("v:part=\"ordinary\"") && out.contains("<v:Keep>Important &amp; unchanged</v:Keep>"));
    assert_eq!(out,xml.replace("a:part=\"4\"","").replace("u:rev=\"2024\"","").replace("<a:conformance>A</a:conformance>","").replace("<u:part>2</u:part>",""));
}
#[test]
fn xmp_prefix_spoofing_and_legacy_custom_pdfx_fields_are_preserved() {
    let xml=packet(r#"<rdf:Description xmlns:pdfaid="urn:ordinary" xmlns:p="http://ns.adobe.com/pdfx/1.3/" xmlns:i="http://www.npes.org/pdfx/ns/id/" xmlns:t="http://www.npes.org/pdfvt/ns/id/" p:Company="Keep" p:GTS_PDFXVersion="PDF/X-1a:2003"><pdfaid:part>Keep fake prefix</pdfaid:part><p:GTS_PDFXConformance>PDF/X-1a:2003</p:GTS_PDFXConformance><i:GTS_PDFXVersion>PDF/X-4</i:GTS_PDFXVersion><t:GTS_PDFVTVersion>PDF/VT-1</t:GTS_PDFVTVersion></rdf:Description>"#);
    let out=changed(xml.as_bytes());assert!(out.contains("Keep fake prefix")&&out.contains("p:Company=\"Keep\""));
    assert!(!out.contains("GTS_PDF"));
}
#[test]
fn nested_claims_do_not_remove_neighboring_qualified_values() {
    let xml=packet(r#"<rdf:Description xmlns:a="http://www.aiim.org/pdfa/ns/id/" xmlns:v="urn:custom"><a:claim><a:nested>Remove all</a:nested></a:claim><v:value rdf:parseType="Resource"><rdf:value>Keep</rdf:value><v:qualifier>Also keep</v:qualifier></v:value></rdf:Description>"#);
    let out=changed(xml.as_bytes());assert!(!out.contains("Remove all"));assert!(out.contains("<rdf:value>Keep</rdf:value><v:qualifier>Also keep</v:qualifier>"));
    assert!(filter(out.as_bytes(),&mut ||Ok(())).unwrap().is_none(),"repeated saves must stabilize");
}
#[test]
fn utf16_and_utf32_claim_filtering_keeps_unicode_and_valid_utf8_declaration() {
    let body=packet(r#"<rdf:Description xmlns:a="http://www.aiim.org/pdfa/ns/id/" xmlns:v="urn:custom" a:part="2"><v:Title>Résumé 文書 📝</v:Title></rdf:Description>"#);
    for wide in [16,32] {for little in [false,true] {
        let text=format!("<?xml version=\"1.0\" encoding=\"UTF-{wide}\"?>{body}");let mut bytes=vec![];
        if wide==16 {bytes.extend(if little{[0xff,0xfe]}else{[0xfe,0xff]});for c in text.encode_utf16(){bytes.extend(if little{c.to_le_bytes()}else{c.to_be_bytes()});}}
        else {bytes.extend(if little{[0xff,0xfe,0,0]}else{[0,0,0xfe,0xff]});for c in text.chars(){bytes.extend(if little{(c as u32).to_le_bytes()}else{(c as u32).to_be_bytes()});}}
        let out=changed(&bytes);assert!(out.contains("Résumé 文書 📝"));assert!(!out.contains("encoding=\"UTF-"));roxmltree::Document::parse(&out).unwrap();
    }}
}
#[test]
fn malformed_xml_entities_duplicates_and_size_limits_are_refused() {
    for xml in ["<x>","<x/><y/>","<!DOCTYPE x [<!ENTITY e SYSTEM 'file:///private'>]><x>&e;</x>","<x xmlns:a='urn:same' xmlns:b='urn:same' a:z='1' b:z='2'/>","<x xmlns:a='urn:a'><b:z/></x>","<1bad/>","<x>&unknown;</x>"] {
        assert!(filter(xml.as_bytes(),&mut ||Ok(())).is_err(),"accepted {xml}");
    }
    assert!(filter(&vec![b' ';4*1024*1024+1],&mut ||Ok(())).is_err());
    let deep=format!("{}{}","<x>".repeat(65),"</x>".repeat(65));assert!(filter(deep.as_bytes(),&mut ||Ok(())).is_err());
    let names=format!("<x {} />",(0..257).map(|i|format!("xmlns:n{i}='urn:{i}'")).collect::<Vec<_>>().join(" "));
    assert!(filter(names.as_bytes(),&mut ||Ok(())).is_err());
}
#[test]
fn cancellation_is_checked_before_and_during_metadata_work() {
    let xml=packet("<rdf:Description/>");let mut count=0;
    let result=filter(xml.as_bytes(),&mut ||{count+=1;if count>=3 {Err(crate::EngineError::InvalidRequest("test cancellation".into()))}else{Ok(())}});
    assert!(result.is_err());assert_eq!(count,3);
}

#[test]
fn valid_xmp_is_refused_at_each_independent_preflight_limit() {
    let attrs = |count: usize| (0..count).map(|i| format!("k{i}='v'")).collect::<Vec<_>>().join(" ");
    let cases = [
        packet(&format!("<rdf:Description {} />", attrs(257))),
        packet(&format!("<rdf:Description>{}{}</rdf:Description>", "<rdf:Bag>".repeat(62), "</rdf:Bag>".repeat(62))),
        packet(&format!("<rdf:Description>{}</rdf:Description>", "<rdf:Bag/>".repeat(131_072))),
        packet(&format!("<rdf:Description>{}</rdf:Description>", format!("<rdf:Bag {}/>", attrs(256)).repeat(129))),
        packet(&format!("<rdf:Description>{}</rdf:Description>", (0..257).map(|i|format!("<rdf:Bag xmlns:n{i}='urn:{i}'/>")).collect::<String>())),
        packet(&format!("<rdf:Description><{}/></rdf:Description>", "a".repeat(1025))),
        packet(&format!("<rdf:Description>{}</rdf:Description>", "x".repeat(4*1024*1024))),
    ];
    for (i, xml) in cases.iter().enumerate() {
        // All input syntax is independently valid. No malformed-root shortcut
        // can satisfy the Folio refusal assertion.
        roxmltree::Document::parse(xml).unwrap();
        assert!(filter(xml.as_bytes(), &mut ||Ok(())).is_err(), "limit {i} was ignored");
    }
    let control=packet(&format!("<rdf:Description {} />",attrs(256)));
    assert!(filter(control.as_bytes(),&mut ||Ok(())).unwrap().is_none());
}

#[test]
fn encoding_declarations_and_invalid_unicode_cannot_be_copied_as_valid_xmp() {
    let ordinary=packet("<rdf:Description/>");
    for declaration in ["<?xml version='1.1'?>", "<?xml version='1.0' encoding='UTF-16'?>", "<?xml version='1.0' encoding='ISO-8859-1'?>", "<?xml version='1.0' standalone='maybe'?>"] {
        assert!(filter(format!("{declaration}{ordinary}").as_bytes(),&mut ||Ok(())).is_err());
    }
    for bytes in [vec![0xff,0xfe,0,0xd8],vec![0xff,0xfe,0],vec![0,0,0xfe,0xff,0,0x11,0,0],vec![0xef,0xbb,0xbf,0xff]] {
        assert!(filter(&bytes,&mut ||Ok(())).is_err());
    }
    let text=format!("<?xml version='1.0' encoding='UTF-16LE'?>{ordinary}");
    let bytes=text.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>();
    assert!(filter(&bytes,&mut ||Ok(())).unwrap().is_none(),"valid unchanged UTF-16 without a BOM must stay exact");
}

#[test]
fn whole_attribute_ranges_preserve_long_spacing_and_adjacent_unicode() {
    let attribute=format!("a:part{}={}\"2\""," ".repeat(300)," ".repeat(300));
    let xml=packet(&format!("<rdf:Description xmlns:a='http://www.aiim.org/pdfa/ns/id/' xmlns:v='urn:custom' v:title='文書' {attribute} v:tail='résumé'/ >").replace("/ >","/>"));
    let output=changed(xml.as_bytes());
    assert_eq!(output,xml.replace(&attribute,""));
}

#[test]
fn cancellation_during_filter_validation_and_wide_decoding_is_propagated() {
    let xml=packet("<rdf:Description xmlns:a='http://www.aiim.org/pdfa/ns/id/' a:part='2'/>");
    let mut total=0;
    filter(xml.as_bytes(),&mut ||{total+=1;Ok(())}).unwrap();
    for stop in [total/2,total-1,total] {
        let mut count=0;
        assert!(filter(xml.as_bytes(),&mut ||{count+=1;if count==stop{Err(crate::EngineError::InvalidRequest("cancelled".into()))}else{Ok(())}}).is_err());
        assert_eq!(count,stop);
    }
    let wide=format!("<?xml version='1.0' encoding='UTF-16LE'?>{}",packet(&format!("<rdf:Description>{}</rdf:Description>","x".repeat(10_000))));
    let bytes=wide.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>();let mut count=0;
    assert!(filter(&bytes,&mut ||{count+=1;if count==4{Err(crate::EngineError::InvalidRequest("cancelled".into()))}else{Ok(())}}).is_err());
    assert_eq!(count,4);
}
