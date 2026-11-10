use lopdf::{
    dictionary, Document, Object, ObjectStreamConfig, ResourceLimits, SaveOptions, Stream,
};
fn fixture(extra: usize, payload: usize, modern: bool) -> Vec<u8> {
    let mut d = Document::with_version("1.7");
    let tree = d.new_object_id();
    let font =
        d.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let content = d.add_object(Stream::new(
        dictionary! {},
        b"BT /F1 12 Tf 20 100 Td (Keep SECRET safe) Tj ET".to_vec(),
    ));
    let page=d.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),200.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
    d.objects.insert(
        tree,
        Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}),
    );
    let root = d.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree});
    d.trailer.set("Root", root);
    for _ in 0..extra {
        d.add_object(Object::string_literal(vec![b'A'; payload]));
    }
    let mut bytes = vec![];
    if modern {
        d.save_with_options(
            &mut bytes,
            SaveOptions {
                use_object_streams: true,
                use_xref_streams: true,
                object_stream_config: ObjectStreamConfig {
                    max_objects_per_stream: 1,
                    compression_level: 6,
                },
                ..Default::default()
            },
        )
        .unwrap();
    } else {
        d.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
        d.save_to(&mut bytes).unwrap();
    }
    bytes
}
#[test]
fn redaction_loader_aggregate_inflation_refuses_many_individually_small_streams() {
    let bytes = fixture(8, 4096, true);
    assert!(bytes.len() < 8192);
    let mut limits = ResourceLimits::default();
    limits.decoded_bytes = 8192;
    assert!(Document::load_mem_with_resource_limits(&bytes, limits).is_err());
    assert!(Document::load_mem_with_resource_limits(&bytes, ResourceLimits::default()).is_ok());
    assert!(Document::load_mem(&bytes).is_ok());
}
#[test]
fn redaction_loader_input_nodes_payload_and_entries_are_independent_limits() {
    let bytes = fixture(30, 256, false);
    for kind in 0..4 {
        let mut limits = ResourceLimits::default();
        match kind {
            0 => limits.input_bytes = bytes.len() - 1,
            1 => limits.object_nodes = 20,
            2 => limits.object_bytes = 2048,
            _ => limits.xref_entries = 8,
        };
        assert!(
            Document::load_mem_with_resource_limits(&bytes, limits).is_err(),
            "limit {kind}"
        );
    }
    let mut limits = ResourceLimits::default();
    limits.input_bytes = bytes.len();
    assert!(Document::load_mem_with_resource_limits(&bytes, limits).is_ok());
}
#[test]
fn redaction_loader_compressed_and_classic_loads_preserve_text_and_scope() {
    for modern in [false, true] {
        let bytes = fixture(8, 128, modern);
        let original = Document::load_mem(&bytes).unwrap();
        let bounded =
            Document::load_mem_with_resource_limits(&bytes, ResourceLimits::default()).unwrap();
        assert_eq!(
            original.extract_text(&[1]).unwrap(),
            bounded.extract_text(&[1]).unwrap()
        );
        let mut limits = ResourceLimits::default();
        limits.object_nodes = 0;
        assert!(Document::load_mem_with_resource_limits(&bytes, limits).is_err());
        assert!(Document::load_mem(&bytes).is_ok());
        assert!(Document::load_mem_with_resource_limits(&bytes, ResourceLimits::default()).is_ok());
    }
}
#[test]
fn redaction_loader_concurrent_budgets_do_not_cross_threads() {
    let a = std::thread::spawn(|| {
        let b = fixture(20, 512, true);
        for _ in 0..4 {
            let mut l = ResourceLimits::default();
            l.decoded_bytes = 1024;
            assert!(Document::load_mem_with_resource_limits(&b, l).is_err());
        }
    });
    let b = std::thread::spawn(|| {
        let b = fixture(20, 512, true);
        for _ in 0..4 {
            assert!(Document::load_mem_with_resource_limits(&b, ResourceLimits::default()).is_ok());
        }
    });
    a.join().unwrap();
    b.join().unwrap();
}

fn raw(objects: Vec<Option<Vec<u8>>>, compressed: &[(usize, usize, usize)]) -> Vec<u8> {
    let count = objects.len() + 2;
    let xref_id = count - 1;
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = vec![0; count];
    for (i, body) in objects.into_iter().enumerate() {
        if let Some(body) = body {
            let id = i + 1;
            offsets[id] = out.len();
            out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
            out.extend(body);
            out.extend_from_slice(b"\nendobj\n");
        }
    }
    offsets[xref_id] = out.len();
    let mut table = vec![];
    for id in 0..count {
        let (kind, a, b) =
            if let Some((_, container, index)) = compressed.iter().find(|(n, _, _)| *n == id) {
                (2, *container, *index)
            } else if offsets[id] > 0 {
                (1, offsets[id], 0)
            } else {
                (0, 0, 65535)
            };
        table.push(kind);
        table.extend_from_slice(&(a as u32).to_be_bytes());
        table.extend_from_slice(&(b as u16).to_be_bytes());
    }
    out.extend_from_slice(format!("{xref_id} 0 obj\n<< /Type /XRef /Size {count} /W [1 4 2] /Root 1 0 R /Length {} >>\nstream\n",table.len()).as_bytes());
    out.extend(table);
    out.extend_from_slice(
        format!(
            "\nendstream\nendobj\nstartxref\n{}\n%%EOF\n",
            offsets[xref_id]
        )
        .as_bytes(),
    );
    out
}
fn stream(dict: &str, data: &[u8]) -> Vec<u8> {
    let mut out = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
    out.extend_from_slice(data);
    out.extend_from_slice(b"\nendstream");
    out
}

#[test]
fn redaction_numeric_loader_checks_direct_and_compressed_original_tokens() {
    let bad=format!("0000000000.{}","123456789".repeat(5));
    for token in [bad,format!("0.{}1","0".repeat(45)),"1.000000059604644775390626".to_owned()] {
        let direct=raw(vec![Some(format!("<< /Type /Catalog /Test [1 {token}] >>").into_bytes())],&[]);
        assert!(Document::load_mem_with_resource_limits(&direct,ResourceLimits::default()).is_ok());
        assert!(crate::redaction_numbers::load_pdf(&direct).is_err(),"{token}");
        let payload=format!("3 0 << /Value {token} >> % {}\n","padding ".repeat(40));
        let mut compressed=Stream::new(dictionary!{},payload.into_bytes());compressed.compress().unwrap();
        assert!(compressed.is_compressed());
        let packed=raw(vec![Some(b"<< /Type /Catalog /Test 3 0 R >>".to_vec()),
            Some(stream("/Type /ObjStm /N 1 /First 4 /Filter /FlateDecode",&compressed.content)),None],&[(3,2,0)]);
        assert!(Document::load_mem_with_resource_limits(&packed,ResourceLimits::default()).is_ok());
        assert!(crate::redaction_numbers::load_pdf(&packed).is_err(),"compressed {token}");
    }
}

#[test]
fn redaction_numeric_loader_skips_nonvalues_and_allows_large_integer_lengths() {
    let bad=format!("0000000000.{}","123456789".repeat(5));
    let bytes=raw(vec![Some(format!("<< /Type /Catalog /Safe 0000000000.25 /LengthLike 67108864 /Name /{bad} /String ({bad}) /Hex <312e303030303030303539363034363434373735333930363236> /Data 2 0 R >>").into_bytes()),Some(stream("",bad.as_bytes()))],&[]);
    assert!(crate::redaction_numbers::load_pdf(&bytes).is_ok());
    for token in ["2147483648","+2147483648","-2147483649"] {
        let bytes=raw(vec![Some(format!("<< /Type /Catalog /Value {token} >>").into_bytes())],&[]);
        assert!(crate::redaction_numbers::load_pdf(&bytes).is_err());
    }
}

#[test]
fn redaction_numeric_loader_predicate_unwind_nested_and_thread_scopes_are_isolated() {
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    let bytes=raw(vec![Some(b"<< /Type /Catalog /Value 0.25 >>".to_vec())],&[]);
    let calls=Arc::new(AtomicUsize::new(0));let counter=calls.clone();
    Document::load_mem_with_numeric_resource_control(&bytes,ResourceLimits::default(),Arc::new(||false),
        Arc::new(move|_,_|{counter.fetch_add(1,Ordering::Relaxed);true})).unwrap();
    assert!(calls.load(Ordering::Relaxed)>0);
    assert!(Document::load_mem_with_numeric_resource_control(&bytes,ResourceLimits::default(),Arc::new(||false),Arc::new(|_,_|false)).is_err());
    assert!(std::panic::catch_unwind(||Document::load_mem_with_numeric_resource_control(&bytes,ResourceLimits::default(),Arc::new(||false),Arc::new(|_,_|panic!("test numeric predicate")))).is_err());
    assert!(crate::redaction_numbers::load_pdf(&bytes).is_ok());
    let other=bytes.clone();
    Document::load_mem_with_numeric_resource_control(&bytes,ResourceLimits::default(),Arc::new(||false),Arc::new(move|_,_|{
        assert!(Document::load_mem_with_resource_limits(&other,ResourceLimits::default()).is_err());
        let other=other.clone();
        std::thread::spawn(move||Document::load_mem_with_resource_limits(&other,ResourceLimits::default()).unwrap()).join().unwrap();
        true
    })).unwrap();
}

#[test]
fn redaction_numeric_publication_walk_checks_direct_values_without_stream_payloads() {
    let mut doc=Document::new();
    doc.add_object(Stream::new(dictionary!{"Safe"=>0.25},b"NaN 0000000000.123456789123456789123456789123456789123456789".to_vec()));
    assert!(crate::redaction_numbers::validate_document_numbers(&doc).is_ok());
    doc.add_object(dictionary!{"Value"=>vec![Object::Real(f32::NAN)]});
    assert!(crate::redaction_numbers::validate_document_numbers(&doc).is_err());
}

#[test]
fn redaction_numeric_loader_refusals_latch_and_resource_limits_remain_active() {
    use std::sync::Arc;
    let bytes=raw(vec![Some(b"<< /Type /Catalog /Value 0.75 /Value 1 >>".to_vec()),Some(b"0.75".to_vec())],&[]);
    let error=Document::load_mem_with_numeric_resource_control(&bytes,ResourceLimits::default(),Arc::new(||false),Arc::new(|token,_|token!=b"0.75")).unwrap_err();
    assert!(error.to_string().contains("numeric token predicate"),"{error}");
    let error=Document::load_mem_with_numeric_resource_control(&bytes,ResourceLimits{object_nodes:1,..Default::default()},Arc::new(||false),Arc::new(|_,_|true)).unwrap_err();
    assert!(error.to_string().contains("parser object attempts"),"{error}");
    let error=Document::load_mem_with_numeric_resource_control(&bytes,ResourceLimits::default(),Arc::new(||true),Arc::new(|_,_|true)).unwrap_err();
    assert!(error.to_string().contains("cancelled"),"{error}");
    assert!(Document::load_mem_with_resource_limits(&bytes,ResourceLimits::default()).is_ok());
    let mut deep=Object::Real(0.25);
    for _ in 0..66 {deep=Object::Array(vec![deep]);}
    let mut doc=Document::new();doc.add_object(deep);
    assert!(crate::redaction_numbers::validate_document_numbers(&doc).is_err());
}

#[test]
fn redaction_numeric_loader_refuses_partial_numeric_lexemes() {
    for token in ["1e2","1.2.3","1+2"] {
        let bytes=raw(vec![Some(b"<< /Type /Catalog /Value 2 0 R >>".to_vec()),Some(token.as_bytes().to_vec())],&[]);
        assert!(Document::load_mem_with_resource_limits(&bytes,ResourceLimits::default()).is_ok());
        assert!(crate::redaction_numbers::load_pdf(&bytes).is_err(),"{token}");
        let payload=format!("3 0 {token} % {}\n","padding ".repeat(40));
        let mut packed=Stream::new(dictionary!{},payload.into_bytes());packed.compress().unwrap();assert!(packed.is_compressed());
        let bytes=raw(vec![Some(b"<< /Type /Catalog /Value 3 0 R >>".to_vec()),
            Some(stream("/Type /ObjStm /N 1 /First 4 /Filter /FlateDecode",&packed.content)),None],&[(3,2,0)]);
        assert!(Document::load_mem_with_resource_limits(&bytes,ResourceLimits::default()).is_ok());
        assert!(crate::redaction_numbers::load_pdf(&bytes).is_err(),"compressed {token}");
    }
}

#[test]
fn redaction_numeric_dictionary_owned_pages_preserve_proved_original_geometry() {
    use std::fmt::Write;
    let tiny=format!("0.{}1","0".repeat(45));
    let long=format!("0000000300.{}","123456789".repeat(5));
    let cases=[("integer","0","300",true),("fraction","0","300.25",true),
        ("leading","0","0000000300.25",true),("precision","0","300.123456789",true),
        ("halfway","0","300.000015258789063",false),("nonfinite","0",long.as_str(),false),
        ("underflow",tiny.as_str(),"300",false)];
    let root=std::env::var_os("FOLIO_DICTIONARY_OUTPUT").map(std::path::PathBuf::from);
    if let Some(root)=&root {std::fs::create_dir_all(root).unwrap();}
    let mut report=String::from("name\taccepted\tleft\theight\n");
    for (name,left,height,expected) in cases {
        for packed in [false,true] {
            let tree=format!("<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [{left} 0 400 {height}] >>");
            let mut objects=vec![Some(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()),Some(tree.as_bytes().to_vec()),
                Some(b"<< /Type /Page /Parent 2 0 R /Resources << >> /Contents 4 0 R >>".to_vec()),
                Some(stream("",b"0 g 20 30 40 10 re f"))];
            let compressed=if packed {
                let payload=format!("2 0 {tree} % {}\n","padding ".repeat(40));
                let mut encoded=Stream::new(dictionary!{},payload.into_bytes());encoded.compress().unwrap();assert!(encoded.is_compressed());
                objects[1]=None;objects.push(Some(stream("/Type /ObjStm /N 1 /First 4 /Filter /FlateDecode",&encoded.content)));
                vec![(2,5,0)]
            } else {vec![]};
            let bytes=raw(objects,&compressed);
            let label=format!("{name}-{}",if packed {"compressed"} else {"direct"});
            let loaded=crate::redaction_numbers::load_pdf(&bytes);
            assert_eq!(loaded.is_ok(),expected,"{label}");
            if let Some(root)=&root {std::fs::write(root.join(format!("{label}-source.pdf")),&bytes).unwrap();}
            if let Ok(doc)=loaded {
                let prepared=crate::redaction_core::prepare_area(&doc).unwrap();
                let saved=crate::redaction_core::publish(prepared.pages).unwrap();
                let reopened=crate::redaction_numbers::load_pdf(&saved).unwrap();
                assert!(crate::redaction_core::prepare_area(&reopened).is_ok());
                if let Some(root)=&root {std::fs::write(root.join(format!("{label}-saved.pdf")),saved).unwrap();}
            }
            writeln!(report,"{label}\t{expected}\t{}\t{}",f64::from(left.parse::<f32>().unwrap()),f64::from(height.parse::<f32>().unwrap())).unwrap();
        }
    }
    if let Some(root)=root {std::fs::write(root.join("manifest.tsv"),report).unwrap();}
}
#[test]
fn redaction_loader_predictor_dimensions_and_compressed_length_cycles_refuse() {
    for columns in [4096i64, i64::MAX] {
        let mut st = Stream::new(dictionary! {}, vec![0u8; 64]);
        st.compress().unwrap();
        assert!(st.is_compressed());
        let bytes = st.content;
        let body=stream(&format!("/Type /ObjStm /N 1 /First 4 /Filter /FlateDecode /DecodeParms << /Predictor 12 /Columns {columns} /Colors 2 /BitsPerComponent 8 >>"),&bytes);
        let bytes = raw(
            vec![Some(b"<< /Type /Catalog >>".to_vec()), Some(body)],
            &[],
        );
        assert!(
            Document::load_mem_with_resource_limits(&bytes, ResourceLimits::default()).is_err()
        );
    }
    let recursive =
        b"<< /Type /ObjStm /N 1 /First 4 /Length 3 0 R >>\nstream\n3 0 12\nendstream".to_vec();
    let bytes = raw(
        vec![
            Some(b"<< /Type /Catalog >>".to_vec()),
            Some(recursive),
            None,
        ],
        &[(3, 2, 0)],
    );
    let err = Document::load_mem_with_resource_limits(&bytes, ResourceLimits::default())
        .unwrap_err()
        .to_string();
    assert!(err.contains("resolution depth"), "{err}");
    let mut objects = vec![Some(b"<< /Type /Catalog >>".to_vec())];
    for id in 2..72 {
        objects.push(Some(
            format!("<< /Length {} 0 R >>\nstream\nx\nendstream", id + 1).into_bytes(),
        ));
    }
    objects.push(Some(b"1".to_vec()));
    let err =
        Document::load_mem_with_resource_limits(&raw(objects, &[]), ResourceLimits::default())
            .unwrap_err()
            .to_string();
    assert!(err.contains("resolution depth"), "{err}");
}
#[test]
fn redaction_loader_overlapping_object_offsets_and_index_tokens_are_charged() {
    let index = (10..90).map(|id| format!("{id} 0 ")).collect::<String>();
    let mut body = index.as_bytes().to_vec();
    body.extend_from_slice(format!("({})", "A".repeat(1024)).as_bytes());
    let bytes = raw(
        vec![
            Some(b"<< /Type /Catalog >>".to_vec()),
            Some(stream(
                &format!("/Type /ObjStm /N 80 /First {}", index.len()),
                &body,
            )),
        ],
        &[],
    );
    let mut limits = ResourceLimits::default();
    limits.object_bytes = 16 * 1024;
    assert!(Document::load_mem_with_resource_limits(&bytes, limits)
        .unwrap_err()
        .to_string()
        .contains("copied object bytes"));
    let mut limits = ResourceLimits::default();
    limits.xref_entries = 20;
    assert!(Document::load_mem_with_resource_limits(&bytes, limits)
        .unwrap_err()
        .to_string()
        .contains("xref/index"));
}
#[test]
fn redaction_loader_arrays_strings_and_incremental_sections_have_shared_limits() {
    let bytes = raw(
        vec![
            Some(b"<< /Type /Catalog >>".to_vec()),
            Some(format!("[{}]", "null ".repeat(500)).into_bytes()),
        ],
        &[],
    );
    let mut limits = ResourceLimits::default();
    limits.object_nodes = 100;
    assert!(Document::load_mem_with_resource_limits(&bytes, limits).is_err());
    let bytes = fixture(0, 0, false);
    let parsed = Document::load_mem(&bytes).unwrap();
    let mut inc = lopdf::IncrementalDocument::create_from(bytes, parsed);
    inc.new_document
        .add_object(Object::string_literal("incremental note"));
    let mut saved = vec![];
    inc.save_to(&mut saved).unwrap();
    let bounded =
        Document::load_mem_with_resource_limits(&saved, ResourceLimits::default()).unwrap();
    assert!(bounded
        .extract_text(&[1])
        .unwrap()
        .contains("Keep SECRET safe"));
    let mut limits = ResourceLimits::default();
    limits.xref_entries = 5;
    assert!(Document::load_mem_with_resource_limits(&saved, limits).is_err());
}
#[test]
fn redaction_loader_keeps_ordinary_reconstruction_and_bounds_top_level_roots() {
    let bytes = fixture(0, 0, false);
    let cut = bytes.windows(5).position(|w| w == b"xref\n").unwrap();
    let broken = [&bytes[..cut], b"trailer\n<< /Root 5 0 R >>\n%%EOF\n"].concat();
    assert!(Document::load_mem(&broken).is_ok());
    assert!(Document::load_mem_with_resource_limits(&broken, ResourceLimits::default()).is_err());
    let bytes = raw(vec![Some(b"42".to_vec())], &[]);
    let mut limits = ResourceLimits::default();
    limits.object_nodes = 0;
    assert!(Document::load_mem_with_resource_limits(&bytes, limits).is_err());
}

#[test]
fn redaction_loader_decoded_boundary_counts_all_structural_streams() {
    let bytes = fixture(4, 512, true);
    let ordinary = Document::load_mem(&bytes).unwrap();
    let total = ordinary
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .filter(|s| s.dict.has_type(b"ObjStm") || s.dict.has_type(b"XRef"))
        .map(|s| s.get_plain_content().unwrap().len())
        .sum::<usize>();
    let mut limits = ResourceLimits::default();
    limits.decoded_bytes = total;
    assert!(
        Document::load_mem_with_resource_limits(&bytes, limits).is_ok(),
        "{total}"
    );
    limits.decoded_bytes -= 1;
    assert!(Document::load_mem_with_resource_limits(&bytes, limits).is_err());
}
#[test]
fn redaction_loader_rejects_brotli_large_window_before_decoder_construction() {
    let bytes = raw(
        vec![
            Some(b"<< /Type /Catalog >>".to_vec()),
            Some(stream(
                "/Type /ObjStm /N 1 /First 4 /Filter /BrotliDecode",
                &[0x11, 0],
            )),
        ],
        &[],
    );
    let err = Document::load_mem_with_resource_limits(&bytes, ResourceLimits::default())
        .unwrap_err()
        .to_string();
    assert!(err.contains("Brotli window extension"), "{err}");
}

#[test]
fn redaction_loader_standard_brotli_and_filter_layer_budgets() {
    // Python Brotli lgwin16 fixture; ordinary six-byte object-stream body.
    let data = [0x50, 0x00, 0x10, 0x33, 0x20, 0x30, 0x20, 0x34, 0x32, 0x03];
    let bytes = raw(
        vec![
            Some(b"<< /Type /Catalog >>".to_vec()),
            Some(stream(
                "/Type /ObjStm /N 1 /First 4 /Filter /BrotliDecode",
                &data,
            )),
            None,
        ],
        &[(3, 2, 0)],
    );
    assert_eq!(
        Document::load_mem_with_resource_limits(&bytes, ResourceLimits::default())
            .unwrap()
            .get_object((3, 0))
            .unwrap()
            .as_i64()
            .unwrap(),
        42
    );
    let plain = format!("3 0 ({})", "A".repeat(1000));
    let mut st = Stream::new(dictionary! {}, plain.as_bytes().to_vec());
    st.compress().unwrap();
    assert!(st.is_compressed());
    let encoded = st
        .content
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
        + ">";
    let bytes = raw(
        vec![
            Some(b"<< /Type /Catalog >>".to_vec()),
            Some(stream(
                "/Type /ObjStm /N 1 /First 4 /Filter [/ASCIIHexDecode /FlateDecode]",
                encoded.as_bytes(),
            )),
            None,
        ],
        &[(3, 2, 0)],
    );
    let mut limits = ResourceLimits::default();
    limits.decoded_bytes = 35 + plain.len();
    assert!(Document::load_mem_with_resource_limits(&bytes, limits).is_err());
    limits.decoded_bytes += st.content.len();
    assert!(Document::load_mem_with_resource_limits(&bytes, limits).is_ok());
}
#[test]
fn redaction_loader_predictor_output_and_corrupt_partial_decodes_are_accounted() {
    let plain = format!("3 0 ({})", "A".repeat(1000));
    for predictor in [2, 12] {
        let data = if predictor == 12 {
            [&[0u8][..], plain.as_bytes()].concat()
        } else {
            let mut p = plain.as_bytes().to_vec();
            for i in (1..p.len()).rev() {
                p[i] = p[i].wrapping_sub(p[i - 1]);
            }
            p
        };
        let mut st = Stream::new(dictionary! {}, data.clone());
        st.compress().unwrap();
        assert!(st.is_compressed());
        let dict=format!("/Type /ObjStm /N 1 /First 4 /Filter /FlateDecode /DecodeParms << /Predictor {predictor} /Columns {} /Colors 1 /BitsPerComponent 8 >>",plain.len());
        let bytes = raw(
            vec![
                Some(b"<< /Type /Catalog >>".to_vec()),
                Some(stream(&dict, &st.content)),
                None,
            ],
            &[(3, 2, 0)],
        );
        let mut limits = ResourceLimits::default();
        limits.decoded_bytes = 35 + data.len() + if predictor == 12 { plain.len() } else { 0 };
        assert!(Document::load_mem_with_resource_limits(&bytes, limits).is_ok());
        limits.decoded_bytes -= 1;
        assert!(Document::load_mem_with_resource_limits(&bytes, limits).is_err());
    }
    let mut st = Stream::new(dictionary! {}, plain.as_bytes().to_vec());
    st.compress().unwrap();
    let n = st.content.len();
    st.content[n - 1] ^= 0xff;
    let bytes = raw(
        vec![
            Some(b"<< /Type /Catalog >>".to_vec()),
            Some(stream(
                "/Type /ObjStm /N 1 /First 4 /Filter /FlateDecode",
                &st.content,
            )),
            None,
        ],
        &[(3, 2, 0)],
    );
    assert!(Document::load_mem(&bytes).is_ok());
    let err = Document::load_mem_with_resource_limits(&bytes, ResourceLimits::default())
        .unwrap_err()
        .to_string();
    assert!(err.contains("decoder failure"), "{err}");
}

#[test]
fn redaction_loader_unfiltered_limit_cannot_hide_in_ignored_length_resolution() {
    let mut body = b"3 0 1".to_vec();
    body.resize(100, b' ');
    let bytes = raw(
        vec![
            Some(b"<< /Type /Catalog >>".to_vec()),
            Some(b"<< /Length 3 0 R >>\nstream\nx\nendstream".to_vec()),
            None,
            Some(stream("/N 1 /First 4", &body)),
        ],
        &[(3, 4, 0)],
    );
    let mut limits = ResourceLimits::default();
    limits.decoded_bytes = 64;
    assert!(
        Document::load_mem_with_resource_limits(&bytes, limits).is_err(),
        "unfiltered limit must poison the scope even if Length resolution ignores its error"
    );
    assert!(
        Document::load_mem(&bytes).is_ok(),
        "ordinary recovery stays available"
    );
}
