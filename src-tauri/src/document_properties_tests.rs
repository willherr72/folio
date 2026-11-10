use super::*;
use lopdf::{dictionary, Stream};
#[test]
fn presence_distinguishes_resolved_null_from_unreadable_metadata() {
    for kind in 0..5 {
        let mut d = fixture();
        let value = match kind {
            0 => Object::Null,
            1 => { let id = d.add_object(Object::Null); Object::Reference(id) },
            2 => { let id = d.add_object(Object::Null); let alias = d.add_object(Object::Reference(id)); Object::Reference(alias) },
            3 => Object::Integer(42),
            _ => { let id = d.new_object_id(); d.objects.insert(id, Object::Reference(id)); Object::Reference(id) },
        };
        d.trailer.set("Info", value.clone());
        let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
        d.get_dictionary_mut(root).unwrap().set("Metadata", value);
        let properties = inspect(&d).unwrap();
        assert_eq!(properties.has_info, kind >= 3, "Info case {kind}");
        assert_eq!(properties.has_xmp, kind >= 3, "XMP case {kind}");
        assert_eq!(properties.error.is_some(), kind >= 3, "error case {kind}");
    }
}
fn fixture() -> Document {
    let mut d = Document::with_version("1.7");
    let info=d.add_object(dictionary!{"Title"=>lopdf::text_string("Info title"),"Author"=>lopdf::text_string("Original author"),"Custom"=>lopdf::text_string("Keep")});
    let packet = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><r:RDF xmlns:r="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><r:Description r:about="" xmlns:d="http://purl.org/dc/elements/1.1/"><d:title><r:Alt><r:li xml:lang="x-default">XMP title</r:li></r:Alt></d:title></r:Description></r:RDF></x:xmpmeta>"#;
    let meta = d.add_object(Stream::new(
        dictionary! {"Type"=>"Metadata","Subtype"=>"XML"},
        packet.as_bytes().to_vec(),
    ));
    let root = d.add_object(dictionary! {"Type"=>"Catalog","Metadata"=>meta});
    d.trailer.set("Root", root);
    d.trailer.set("Info", info);
    d
}
fn edit() -> DocumentMetadataEdit {
    DocumentMetadataEdit::Update {
        clear_existing: false,
        title: Some("Résumé 文書".into()),
        author: None,
        subject: Some("".into()),
        keywords: None,
    }
}
#[test]
fn inspection_reports_conflicts_and_updates_only_explicit_fields() {
    let mut d = fixture();
    let old = inspect(&d).unwrap();
    assert_eq!(old.title, "Info title");
    assert_eq!(old.conflicts, vec!["title"]);
    assert!(old.has_info && old.has_xmp);
    apply(&mut d, &edit(), &mut || Ok(())).unwrap();
    let new = inspect(&d).unwrap();
    assert_eq!(new.title, "Résumé 文書");
    assert_eq!(new.author, "Original author");
    assert!(new.conflicts.is_empty());
    let id = d.trailer.get(b"Info").unwrap().as_reference().unwrap();
    assert_eq!(
        lopdf::decode_text_string(d.get_dictionary(id).unwrap().get(b"Custom").unwrap()).unwrap(),
        "Keep"
    );
}
#[test]
fn filtered_roots_are_private_and_original_aliases_remain_unchanged() {
    let mut d = fixture();
    let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let meta = d
        .catalog()
        .unwrap()
        .get(b"Metadata")
        .unwrap()
        .as_reference()
        .unwrap();
    let original = d.get_object(meta).unwrap().clone();
    d.get_dictionary_mut(root).unwrap().set("KeepAlias", meta);
    apply(&mut d, &edit(), &mut || Ok(())).unwrap();
    assert_ne!(
        d.catalog()
            .unwrap()
            .get(b"Metadata")
            .unwrap()
            .as_reference()
            .unwrap(),
        meta
    );
    assert_eq!(
        format!("{:?}", d.get_object(meta).unwrap()),
        format!("{original:?}")
    );
}
#[test]
fn explicit_removal_handles_malformed_metadata_and_prunes_detached_bytes() {
    let mut d = fixture();
    let meta = d
        .catalog()
        .unwrap()
        .get(b"Metadata")
        .unwrap()
        .as_reference()
        .unwrap();
    d.get_object_mut(meta)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .content = b"SECRET_MALFORMED_XMP".to_vec();
    apply(&mut d, &DocumentMetadataEdit::Remove {}, &mut || Ok(())).unwrap();
    assert!(!d.catalog().unwrap().has(b"Metadata"));
    assert!(!d.trailer.has(b"Info"));
    let mut bytes = Vec::new();
    d.save_to(&mut bytes).unwrap();
    assert!(!bytes.windows(20).any(|s| s == b"SECRET_MALFORMED_XMP"));
    assert!(!d.objects.contains_key(&meta));
}
#[test]
fn validation_and_cancelled_edits_do_not_mutate_the_document() {
    assert!(edit().validate().is_ok());
    assert!(DocumentMetadataEdit::Update {
        clear_existing: false,
        title: None,
        author: None,
        subject: None,
        keywords: None
    }
    .validate()
    .is_err());
    for s in ["a\0b".into(), "x".repeat(FIELD_LIMIT + 1)] {
        assert!(DocumentMetadataEdit::Update {
            clear_existing: false,
            title: Some(s),
            author: None,
            subject: None,
            keywords: None
        }
        .validate()
        .is_err());
    }
    assert!(
        serde_json::from_str::<DocumentMetadataEdit>(r#"{"mode":"remove","title":"ignored"}"#)
            .is_err()
    );
    let mut d = fixture();
    let before = format!("{d:?}");
    assert!(apply(&mut d, &edit(), &mut || Err(bad("cancelled"))).is_err());
    assert_eq!(before, format!("{d:?}"));
}
#[test]
fn cancellation_at_every_checkpoint_is_atomic_and_repeated_edits_stay_bounded() {
    let mut trial = fixture();
    let mut calls = 0;
    apply(&mut trial, &edit(), &mut || {
        calls += 1;
        Ok(())
    })
    .unwrap();
    for cancel_at in 1..=calls {
        let mut d = fixture();
        let before = format!("{d:?}");
        let mut n = 0;
        assert!(apply(&mut d, &edit(), &mut || {
            n += 1;
            if n == cancel_at {
                Err(bad("cancelled"))
            } else {
                Ok(())
            }
        })
        .is_err());
        assert_eq!(before, format!("{d:?}"), "checkpoint {cancel_at}");
    }
    let count = trial.objects.len();
    for _ in 0..12 {
        apply(&mut trial, &edit(), &mut || Ok(())).unwrap();
        assert_eq!(trial.objects.len(), count);
        assert_eq!(inspect(&trial).unwrap().title, "Résumé 文書");
    }
}
#[test]
fn removed_metadata_reachable_through_another_consumer_is_preserved() {
    let mut d = fixture();
    let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let meta = d
        .catalog()
        .unwrap()
        .get(b"Metadata")
        .unwrap()
        .as_reference()
        .unwrap();
    d.get_dictionary_mut(root)
        .unwrap()
        .set("PageMetadataAlias", meta);
    let old = format!("{:?}", d.get_object(meta).unwrap());
    apply(&mut d, &DocumentMetadataEdit::Remove {}, &mut || Ok(())).unwrap();
    assert_eq!(format!("{:?}", d.get_object(meta).unwrap()), old);
    assert!(!d.catalog().unwrap().has(b"Metadata"));
    assert!(!d.trailer.has(b"Info"));
}
#[test]
fn info_utf8_bom_is_not_visible_and_invalid_wide_text_is_reported() {
    let mut d = fixture();
    let id = d.trailer.get(b"Info").unwrap().as_reference().unwrap();
    d.get_dictionary_mut(id).unwrap().set(
        "Author",
        lopdf::Object::string_literal(b"\xef\xbb\xbfAuthor".to_vec()),
    );
    assert_eq!(inspect(&d).unwrap().author, "Author");
    d.get_dictionary_mut(id)
        .unwrap()
        .set("Author", lopdf::Object::string_literal(vec![254, 255, 0]));
    assert!(inspect(&d).unwrap().error.is_some());
}

#[test]
fn adding_properties_after_removal_keeps_the_cleared_base() {
    let edit: DocumentMetadataEdit = serde_json::from_value(
        serde_json::json!({"mode":"update","clearExisting":true,"title":"Public"}),
    )
    .unwrap();
    let mut d = fixture();
    apply(&mut d, &edit, &mut || Ok(())).unwrap();
    let out = inspect(&d).unwrap();
    assert_eq!(out.title, "Public");
    assert_eq!(out.author, "");
    let info = d
        .get_dictionary(d.trailer.get(b"Info").unwrap().as_reference().unwrap())
        .unwrap();
    assert!(!info.has(b"Custom"));
}

#[test]
fn inspection_uses_xmp_creator_producer_and_dates_when_info_is_absent() {
    let mut d = fixture();
    d.trailer.remove(b"Info");
    let meta = d
        .catalog()
        .unwrap()
        .get(b"Metadata")
        .unwrap()
        .as_reference()
        .unwrap();
    d.get_object_mut(meta).unwrap().as_stream_mut().unwrap().set_content(br#"<r:RDF xmlns:r="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><r:Description r:about="" xmlns:x="http://ns.adobe.com/xap/1.0/" xmlns:p="http://ns.adobe.com/pdf/1.3/" x:CreatorTool="Tool" p:Producer="Producer" x:CreateDate="2026-11-01T00:00:00Z" x:ModifyDate="2026-11-02T00:00:00Z"/></r:RDF>"#.to_vec());
    let data = inspect(&d).unwrap();
    assert_eq!(data.creator, "Tool");
    assert_eq!(data.producer, "Producer");
    assert_eq!(data.creation_date, "2026-11-01T00:00:00Z");
    assert_eq!(data.modification_date, "2026-11-02T00:00:00Z");
}
