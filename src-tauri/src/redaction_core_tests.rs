use crate::redaction_core::{inspect_standard_text, remove_standard_text};
use lopdf::{dictionary, Document, Object, ObjectId, Stream};

const SECRET: &str = "Confidential-84912";

#[test]
fn redaction_publish_rechecks_newly_generated_numeric_operands() {
    use crate::redaction_core::{publish,Page};
    use crate::redaction_content::Program;
    use lopdf::content::Operation;
    let make=|value:f32| Page {
        width:256.into(),height:256.into(),fonts:vec![],graphics:vec![],images:vec![],
        program:Program{blocks:vec![],operations:vec![Operation::new("cm",
            vec![1.into(),0.into(),0.into(),1.into(),value.into(),0.into()])]}
    };
    for value in [f32::NAN,1_000_001.,1e-22] {
        assert!(publish(vec![make(value)]).is_err(),"{value}");
    }
    let bytes=publish(vec![make(0.125)]).unwrap();
    let saved=Document::load_mem(&bytes).unwrap();
    let page=*saved.get_pages().values().next().unwrap();
    let program=lopdf::content::Content::decode_strict(&saved.get_page_content(page)).unwrap();
    assert_eq!(program.operations[0].operands[4].as_float().unwrap(),0.125);
}

#[test]
fn redaction_numeric_reader_disagreement_refuses_without_changing_source() {
    for token in [format!("0000000000.{}","123456789".repeat(5)),
        format!("0.{}1","0".repeat(45)),"1.000000059604644775390626".to_owned()] {
        let data=format!("q 1 0 0 1 {token} 0 cm BT /PrivateAlias_84912 12 Tf (Public) Tj ET Q");
        let (doc,_)=fixture(data.as_bytes());
        let page=*doc.get_pages().values().next().unwrap();
        let before=doc.get_page_content(page);
        assert!(inspect_standard_text(&doc).is_err(),"{token}");
        assert_eq!(doc.get_page_content(page),before);
    }
}

#[test]
fn redaction_strict_flate_checks_exact_budget_and_preserves_ordinary_recovery() {
    let payload = b"% harmless padding padding padding padding\n".repeat(1000);
    let mut stream = Stream::new(dictionary! {}, payload.clone());
    stream.compress().unwrap();
    assert!(stream.dict.has(b"Filter"));
    assert_eq!(
        stream
            .decompressed_content_strict_with_limit(payload.len())
            .unwrap(),
        payload
    );
    assert!(stream
        .decompressed_content_strict_with_limit(payload.len() - 1)
        .is_err());
    assert!(stream.decompressed_content_strict_with_limit(0).is_err());
    *stream.content.last_mut().unwrap() ^= 0xff;
    assert!(stream
        .decompressed_content_strict_with_limit(payload.len())
        .is_err());
    // The existing permissive reader may return only a prefix on checksum
    // failure. Preserve that API behavior; redaction must use the strict API.
    let recovered = stream
        .decompressed_content_with_limit(payload.len())
        .unwrap();
    assert!(!recovered.is_empty() && payload.starts_with(&recovered));
    let empty = Stream::new(
        dictionary! {"Filter"=>"FlateDecode"},
        vec![0x78, 0x9c, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01],
    );
    assert_eq!(
        empty.decompressed_content_strict_with_limit(0).unwrap(),
        Vec::<u8>::new()
    );
}

#[test]
fn redaction_numeric_aliases_cannot_expand_array_beyond_admission_budget() {
    let operand = format!("{} ", "+1".repeat(32));
    let (doc, _) = fixture(
        format!(
            "BT /PrivateAlias_84912 12 Tf (Public) Tj ET [{}] 0 d",
            operand.repeat(129)
        )
        .as_bytes(),
    );
    assert!(inspect_standard_text(&doc).is_err());
}

#[test]
fn redaction_compressed_corruption_is_not_a_successful_partial_page() {
    let prefix = b"BT /PrivateAlias_84912 12 Tf (Public prefix) Tj ET";
    let n = prefix.len() as u16;
    let mut bad_tail = vec![0x78, 0x01, 0x00];
    bad_tail.extend_from_slice(&n.to_le_bytes());
    bad_tail.extend_from_slice(&(!n).to_le_bytes());
    bad_tail.extend_from_slice(prefix);
    bad_tail.push(0x07);
    let mut valid=Stream::new(dictionary!{},b"% padding padding padding padding padding padding padding\nBT /PrivateAlias_84912 12 Tf (Public compressed) Tj ET".to_vec());
    valid.compress().unwrap();
    assert!(valid.dict.has(b"Filter"));
    let mut checksum = valid.content.clone();
    *checksum.last_mut().unwrap() ^= 0xff;
    let mut truncated = valid.content.clone();
    truncated.truncate(truncated.len() - 4);
    let mut trailing = valid.content.clone();
    trailing.extend_from_slice(b"junk");
    let mut accepted = vec![];
    for (label, encoded) in [
        ("header", vec![0x78, 0x9c]),
        ("bad-tail", bad_tail),
        ("checksum", checksum),
        ("truncated", truncated),
        ("trailing", trailing),
    ] {
        let (mut doc, page) = source();
        let existing = doc
            .get_dictionary(page)
            .unwrap()
            .get(b"Contents")
            .unwrap()
            .clone();
        let corrupt = doc.add_object(Stream::new(dictionary! {"Filter"=>"FlateDecode"}, encoded));
        doc.get_dictionary_mut(page)
            .unwrap()
            .set("Contents", vec![existing, corrupt.into()]);
        if inspect_standard_text(&doc).is_ok() {
            accepted.push(label);
        }
    }
    assert!(
        accepted.is_empty(),
        "Accepted corrupt streams: {accepted:?}"
    );
}
fn fixture(content: &[u8]) -> (Document, ObjectId) {
    let mut doc = Document::with_version("1.4");
    let pages = doc.new_object_id();
    let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica","Encoding"=>"WinAnsiEncoding"});
    let stream = doc.add_object(Stream::new(dictionary! {}, content.to_vec()));
    let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),300.into(),200.into()],"Contents"=>stream,"Resources"=>dictionary!{"Font"=>dictionary!{"PrivateAlias_84912"=>font}}});
    doc.objects.insert(
        pages,
        Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}),
    );
    let catalog = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    doc.trailer.set("Root", catalog);
    (doc, page)
}
fn source() -> (Document, ObjectId) {
    fixture(format!("BT /PrivateAlias_84912 12 Tf 1 0 0 1 20 150 Tm ({SECRET}) Tj ET\nBT 1 0 0 1 20 110 Tm (Public remainder.) Tj ET").as_bytes())
}
fn bytes(doc: &Document) -> Vec<u8> {
    let mut out = vec![];
    doc.clone().save_to(&mut out).unwrap();
    out
}
fn content(doc: &Document, page: ObjectId) -> String {
    String::from_utf8(doc.get_page_content(page)).unwrap()
}

#[test]
fn redaction_inspection_binds_complete_document_and_retains_block_identity() {
    let (doc, page) = source();
    let blocks = inspect_standard_text(&doc).unwrap();
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].text, SECRET);
    assert_eq!(blocks[0].page_index, 0);
    assert_eq!(blocks[1].block_index, 1);
    assert_ne!(blocks[0].selection_id, blocks[1].selection_id);
    let mut changed = doc.clone();
    let stream = changed
        .get_dictionary(page)
        .unwrap()
        .get(b"Contents")
        .unwrap()
        .as_reference()
        .unwrap();
    changed
        .get_object_mut(stream)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .content
        .extend_from_slice(b"\n0.5 g");
    assert!(remove_standard_text(&changed, &[blocks[0].selection_id.clone()]).is_err());
}

#[test]
fn redaction_erases_real_show_operands_and_hidden_graph_copies_without_mutating_source() {
    let (mut doc, page) = source();
    let metadata = doc.add_object(Stream::new(
        dictionary! {"Type"=>"Metadata","Subtype"=>"XML"},
        SECRET.as_bytes().to_vec(),
    ));
    let info = doc.add_object(dictionary! {"Subject"=>Object::string_literal(SECRET)});
    doc.trailer.set("Info", info);
    let root = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
    doc.get_dictionary_mut(root)
        .unwrap()
        .set("Metadata", metadata);
    doc.add_object(Stream::new(dictionary! {}, SECRET.as_bytes().to_vec()));
    let stream = doc
        .get_dictionary(page)
        .unwrap()
        .get(b"Contents")
        .unwrap()
        .as_reference()
        .unwrap();
    doc.get_object_mut(stream)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .content
        .extend_from_slice(format!("\n% {SECRET}\n").as_bytes());
    let before = bytes(&doc);
    let selected = inspect_standard_text(&doc).unwrap()[0].selection_id.clone();
    let result = remove_standard_text(&doc, &[selected]).unwrap();
    assert_eq!(result.removed_blocks, 1);
    assert!(!result.omitted_metadata.is_empty());
    assert_eq!(bytes(&doc), before);
    assert!(!result
        .bytes
        .windows(SECRET.len())
        .any(|v| v == SECRET.as_bytes()));
    assert!(!result
        .bytes
        .windows(b"PrivateAlias_84912".len())
        .any(|v| v == b"PrivateAlias_84912"));
    let output = Document::load_mem(&result.bytes).unwrap();
    let page = *output.get_pages().get(&1).unwrap();
    let decoded = content(&output, page);
    assert!(decoded.contains("Public remainder."));
    assert!(!decoded.contains(SECRET));
    assert!(!output.trailer.has(b"Info"));
    assert!(!output.catalog().unwrap().has(b"Metadata"));
    for object in output.objects.values() {
        if let Object::Stream(stream) = object {
            assert!(!stream
                .decompressed_content()
                .unwrap()
                .windows(SECRET.len())
                .any(|v| v == SECRET.as_bytes()));
        }
    }
}

#[test]
fn redaction_quote_removal_preserves_persistent_character_and_word_spacing() {
    let (doc,_)=fixture(b"BT /PrivateAlias_84912 12 Tf 14 TL 1 0 0 1 20 150 Tm 3 2 (Confidential-84912) \" ET BT 1 0 0 1 20 90 Tm (Public words) Tj ET");
    let block = inspect_standard_text(&doc).unwrap().remove(0);
    let result = remove_standard_text(&doc, &[block.selection_id]).unwrap();
    let saved = Document::load_mem(&result.bytes).unwrap();
    let c = content(&saved, *saved.get_pages().get(&1).unwrap());
    assert!(c.contains("3 Tw"));
    assert!(c.contains("2 Tc"));
    assert!(c.contains("Public words"));
    assert!(!c.contains(SECRET));
}

#[test]
fn redaction_rejects_duplicate_unknown_and_empty_targets() {
    let (doc, _) = source();
    let id = inspect_standard_text(&doc).unwrap()[0].selection_id.clone();
    assert!(remove_standard_text(&doc, &[]).is_err());
    assert!(remove_standard_text(&doc, &[id.clone(), id]).is_err());
    assert!(remove_standard_text(&doc, &["other".into()]).is_err());
}

#[test]
fn redaction_rejects_inline_images_malformed_tails_and_unknown_operators() {
    for tail in [
        "BI /W 9223372036854775807 /H 9223372036854775807 /BPC 32 /CS /RGB ID x EI",
        "(unclosed",
        "/Im0 Do",
        "/Unknown gs",
        "1 2 3 unknown",
    ] {
        let (doc, _) =
            fixture(format!("BT /PrivateAlias_84912 12 Tf (Public) Tj ET\n{tail}").as_bytes());
        assert!(inspect_standard_text(&doc).is_err(), "{tail}");
    }
}

#[test]
fn redaction_rejects_inherited_implicit_color_spaces() {
    let (mut doc, page) = source();
    let parent = doc
        .get_dictionary(page)
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    let resources = doc
        .get_dictionary_mut(page)
        .unwrap()
        .remove(b"Resources")
        .unwrap();
    doc.get_dictionary_mut(parent)
        .unwrap()
        .set("Resources", resources);
    let colors = doc.add_object(dictionary! {"DefaultRGB"=>"DeviceRGB"});
    doc.get_dictionary_mut(parent)
        .unwrap()
        .get_mut(b"Resources")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("ColorSpace", colors);
    assert!(inspect_standard_text(&doc).is_err());
}

#[test]
fn redaction_keeps_shared_source_page_occurrences_separate() {
    let (mut doc, page) = source();
    let parent = doc
        .get_dictionary(page)
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    let another = doc.add_object(doc.get_dictionary(page).unwrap().clone());
    let tree = doc.get_dictionary_mut(parent).unwrap();
    tree.set("Kids", vec![page.into(), another.into()]);
    tree.set("Count", 2);
    let blocks = inspect_standard_text(&doc).unwrap();
    let result = remove_standard_text(&doc, &[blocks[0].selection_id.clone()]).unwrap();
    let saved = Document::load_mem(&result.bytes).unwrap();
    let pages = saved.get_pages();
    assert!(!content(&saved, pages[&1]).contains(SECRET));
    assert!(content(&saved, pages[&2]).contains(SECRET));
}

#[test]
fn redaction_requires_a_pages_root_and_consistent_acyclic_tree() {
    let (mut doc, page) = source();
    let root = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
    doc.get_dictionary_mut(page).unwrap().remove(b"Parent");
    doc.get_dictionary_mut(root).unwrap().set("Pages", page);
    assert!(
        inspect_standard_text(&doc).is_err(),
        "Catalog Pages must name a Pages node"
    );
    let (mut doc, page) = source();
    let parent = doc
        .get_dictionary(page)
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    doc.get_dictionary_mut(parent)
        .unwrap()
        .set("Kids", vec![page.into(), page.into()]);
    doc.get_dictionary_mut(parent).unwrap().set("Count", 2);
    assert!(inspect_standard_text(&doc).is_err());
    doc.get_dictionary_mut(parent)
        .unwrap()
        .set("Kids", vec![parent.into()]);
    assert!(inspect_standard_text(&doc).is_err());
    let (mut doc, page) = source();
    doc.get_dictionary_mut(page).unwrap().set("Parent", page);
    assert!(inspect_standard_text(&doc).is_err());
}

#[test]
fn redaction_rejects_lexical_and_decoded_budgets() {
    let cases = [
        format!("[{}] 0 d", "1 ".repeat(4097)),
        format!("{}g", "0 ".repeat(17)),
        "0 g\n".repeat(100_001),
        format!(
            "BT /PrivateAlias_84912 12 Tf ({}) Tj ET",
            "A".repeat(1024 * 1024 + 1)
        ),
        format!("%{}", "x".repeat(8 * 1024 * 1024)),
        format!("{}{}", "q ".repeat(65), "Q ".repeat(65)),
        "[[(x)]] TJ".into(),
        "BT /PrivateAlias_84912 12 Tf <ff> Tj ET".into(),
        "0 0 1000001 20 re f".into(),
    ];
    for (i, content) in cases.iter().enumerate() {
        let (doc, _) = fixture(content.as_bytes());
        assert!(inspect_standard_text(&doc).is_err(), "budget case {i}");
    }
}

#[test]
fn redaction_checks_operand_shapes_and_balanced_graphics_and_text_state() {
    for content in [
        "Q",
        "q",
        "BT BT ET ET",
        "BT",
        "ET",
        "BT (No font) Tj ET",
        "0 0 Td",
        "1 2 cm",
        "BT /PrivateAlias_84912 12 Tf 3 Tr (Invisible) Tj ET",
        "BT /PrivateAlias_84912 12 Tf (x) Tj 10 10 m ET",
        "BT /PrivateAlias_84912 12 Tf [(x) /name] TJ ET",
        "BT /PrivateAlias_84912 12 Tf (x) (y) Tj ET",
        "q BT /PrivateAlias_84912 12 Tf (x) Tj ET Q BT (undefined again) Tj ET",
        "0 0 0 1 2 rg",
        "[0 0] 0 d",
    ] {
        let (doc, _) = fixture(content.as_bytes());
        assert!(inspect_standard_text(&doc).is_err(), "{content}");
    }
}

#[test]
fn redaction_admits_inherited_resources_and_compressed_streams() {
    let (mut doc, page) = source();
    let parent = doc
        .get_dictionary(page)
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    for key in [b"Resources".as_slice(), b"MediaBox"] {
        let value = doc.get_dictionary_mut(page).unwrap().remove(key).unwrap();
        doc.get_dictionary_mut(parent).unwrap().set(key, value);
    }
    let stream = doc
        .get_dictionary(page)
        .unwrap()
        .get(b"Contents")
        .unwrap()
        .as_reference()
        .unwrap();
    doc.get_object_mut(stream)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .compress()
        .unwrap();
    let blocks = inspect_standard_text(&doc).unwrap();
    assert_eq!(blocks.len(), 2);
    let result = remove_standard_text(&doc, &[blocks[0].selection_id.clone()]).unwrap();
    assert!(!result
        .bytes
        .windows(SECRET.len())
        .any(|s| s == SECRET.as_bytes()));
}

#[test]
fn redaction_refuses_rendering_dependencies_and_reference_cycles() {
    for key in ["Annots", "Group", "AA"] {
        let (mut doc, page) = source();
        doc.get_dictionary_mut(page).unwrap().set(key, Object::Null);
        assert!(inspect_standard_text(&doc).is_err(), "{key}");
    }
    for key in [
        "AcroForm",
        "OCProperties",
        "OutputIntents",
        "StructTreeRoot",
    ] {
        let (mut doc, _) = source();
        let root = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        doc.get_dictionary_mut(root).unwrap().set(key, Object::Null);
        assert!(inspect_standard_text(&doc).is_err(), "{key}");
    }
    let (mut doc, page) = source();
    let id = doc.new_object_id();
    doc.objects.insert(id, Object::Reference(id));
    doc.get_dictionary_mut(page).unwrap().set("Resources", id);
    assert!(inspect_standard_text(&doc).is_err());
    let (mut doc, page) = source();
    let f = doc
        .get_dictionary(page)
        .unwrap()
        .get(b"Resources")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Font")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"PrivateAlias_84912")
        .unwrap()
        .as_reference()
        .unwrap();
    doc.get_dictionary_mut(f)
        .unwrap()
        .set("Widths", vec![500.into()]);
    assert!(inspect_standard_text(&doc).is_err());
}

#[test]
fn redaction_multiple_targets_remove_tj_arrays_and_quote_shows() {
    let (doc,_)=fixture(b"BT /PrivateAlias_84912 12 Tf [(Confi) 12 (dential-84912)] TJ ET BT 12 TL (Confidential-84912) ' ET BT (Public) Tj ET");
    let blocks = inspect_standard_text(&doc).unwrap();
    assert_eq!(blocks[0].text, SECRET);
    assert_eq!(blocks[1].text, SECRET);
    let result = remove_standard_text(
        &doc,
        &[
            blocks[0].selection_id.clone(),
            blocks[1].selection_id.clone(),
        ],
    )
    .unwrap();
    assert_eq!(result.removed_blocks, 2);
    assert!(!result
        .bytes
        .windows(SECRET.len())
        .any(|s| s == SECRET.as_bytes()));
    let output = Document::load_mem(&result.bytes).unwrap();
    assert_eq!(inspect_standard_text(&output).unwrap()[0].text, "Public");
}

#[test]
fn redaction_exports_synthetic_independent_reader_fixtures() {
    let Some(root) = std::env::var_os("FOLIO_REDACTION_FIXTURES") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    std::fs::create_dir_all(&root).unwrap();
    let mut manifest = vec![];
    for case in ["hidden", "quotes", "array", "shared"] {
        let (mut doc, page) = if case == "quotes" {
            fixture(b"BT /PrivateAlias_84912 12 Tf 14 TL 1 0 0 1 20 150 Tm 3 2 (Confidential-84912) \" ET BT 1 0 0 1 20 90 Tm (Public words) Tj ET")
        } else if case == "array" {
            fixture(b"0.7 g 250 160 20 20 re f 0 g BT /PrivateAlias_84912 12 Tf 1 0 0 1 20 150 Tm [(Confidential-) 0 (84912)] TJ ET BT 1 0 0 1 20 110 Tm (Public remainder.) Tj ET")
        } else {
            source()
        };
        if case == "hidden" {
            let info = doc.add_object(dictionary! {"Subject"=>Object::string_literal(SECRET)});
            doc.trailer.set("Info", info);
            let hidden = doc.add_object(Stream::new(dictionary! {}, SECRET.as_bytes().to_vec()));
            let catalog = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
            doc.get_dictionary_mut(catalog)
                .unwrap()
                .set("Metadata", hidden);
            doc.add_object(Stream::new(dictionary! {}, SECRET.as_bytes().to_vec()));
        }
        if case == "shared" {
            let parent = doc
                .get_dictionary(page)
                .unwrap()
                .get(b"Parent")
                .unwrap()
                .as_reference()
                .unwrap();
            let another = doc.add_object(doc.get_dictionary(page).unwrap().clone());
            let tree = doc.get_dictionary_mut(parent).unwrap();
            tree.set("Kids", vec![page.into(), another.into()]);
            tree.set("Count", 2);
        }
        let blocks = inspect_standard_text(&doc).unwrap();
        let result = remove_standard_text(&doc, &[blocks[0].selection_id.clone()]).unwrap();
        let before = format!("{case}-before.pdf");
        let after = format!("{case}-after.pdf");
        std::fs::write(root.join(&before), bytes(&doc)).unwrap();
        std::fs::write(root.join(&after), result.bytes).unwrap();
        manifest
            .push(serde_json::json!({"name":case,"before":before,"after":after,"selected":[0]}));
    }
    std::fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
}
