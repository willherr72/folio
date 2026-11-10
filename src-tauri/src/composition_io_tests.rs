use lopdf::{dictionary, Document, Object, ResourceLimits, SaveOptions, Stream};
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

#[test]
fn composition_io_controlled_content_accounts_operations_operands_and_payloads() {
    use lopdf::content::Content;
    let input=b"q BT /F 12 Tf (literal EMC with \\( escapes) Tj ET Q";
    let (actual,left)=Content::decode_strict_with_resource_control(input,ResourceLimits::default(),Arc::new(||false)).unwrap();
    let expected=Content::decode_strict(input).unwrap();
    assert_eq!(actual.operations.len(),expected.operations.len());
    for (a,b) in actual.operations.iter().zip(&expected.operations) {assert_eq!(a.operator,b.operator);assert_eq!(a.operands,b.operands);}
    assert_eq!(left.input_bytes,ResourceLimits::default().input_bytes-input.len());
    assert!(left.object_nodes<ResourceLimits::default().object_nodes);
    assert!(left.object_bytes<ResourceLimits::default().object_bytes);
    for input in [b"q Q q Q q Q".as_slice(),b"1 2 3 4 5 6 cm".as_slice()] {
        assert!(Content::decode_strict_with_resource_control(input,ResourceLimits{object_nodes:4,..Default::default()},Arc::new(||false)).is_err());
    }
    for input in [b"(large literal) Tj".as_slice(),b"/LargeName MP".as_slice(),b"longoperator".as_slice()] {
        assert!(Content::decode_strict_with_resource_control(input,ResourceLimits{object_bytes:4,..Default::default()},Arc::new(||false)).is_err());
    }
    let (_,left)=Content::decode_strict_with_resource_control(b"q Q",ResourceLimits{input_bytes:5,..Default::default()},Arc::new(||false)).unwrap();
    assert!(Content::decode_strict_with_resource_control(b"q Q",left,Arc::new(||false)).is_err());
}

#[test]
fn composition_io_controlled_content_cancellation_and_refusal_restore_scope() {
    use lopdf::content::Content;
    let input=b"q BT /F 12 Tf [(one) 20 (two)] TJ ET Q";
    let observed=Arc::new(AtomicUsize::new(0));
    let calls=observed.clone();
    Content::decode_strict_with_resource_control(input,ResourceLimits::default(),Arc::new(move||{calls.fetch_add(1,Ordering::Relaxed);false})).unwrap();
    let total=observed.load(Ordering::Relaxed);
    assert!(total>8);
    for stop in [0,total/2,total-1] {
        let calls=Arc::new(AtomicUsize::new(0));
        let result=Content::decode_strict_with_resource_control(input,ResourceLimits::default(),Arc::new(move||calls.fetch_add(1,Ordering::Relaxed)>=stop));
        assert!(result.unwrap_err().to_string().contains("cancelled"));
        assert!(Content::decode_strict_with_resource_control(input,ResourceLimits::default(),Arc::new(||false)).is_ok());
    }
    for input in [b"(unclosed".as_slice(),b"q [".as_slice(),b"BI /W 9223372036854775807 /H 9223372036854775807 ID x EI".as_slice()] {
        assert!(Content::decode_strict_with_resource_control(input,ResourceLimits::default(),Arc::new(||false)).is_err());
        assert!(Content::decode_strict_with_resource_control(b"q Q",ResourceLimits::default(),Arc::new(||false)).is_ok());
    }
    assert!(Document::load_mem_with_resource_limits(&fixture(false),ResourceLimits::default()).is_ok());
}

#[test]
fn composition_io_strict_content_accepts_eof_comments_without_ignoring_other_tokens() {
    use lopdf::content::Content;
    for input in [b"q Q %tail".as_slice(),b"%only comment",b"%",b"q Q%tail",b"q Q %one\r\n %two"] {
        let ordinary=Content::decode_strict(input).unwrap();
        let (controlled,_)=Content::decode_strict_with_resource_control(input,ResourceLimits::default(),Arc::new(||false)).unwrap();
        assert_eq!(ordinary.operations.len(),controlled.operations.len());
        assert!(ordinary.operations.is_empty() || ordinary.operations.len()==2);
    }
    for input in [b"q Q %tail\n [".as_slice(),b"q Q 123 %tail",b"q Q (missing) %tail",b"q Q %tail\n >"] {
        assert!(Content::decode_strict(input).is_err());
        assert!(Content::decode_strict_with_resource_control(input,ResourceLimits::default(),Arc::new(||false)).is_err());
    }
}

#[test]
fn composition_io_controlled_content_nested_values_and_unwind_remain_bounded() {
    use lopdf::content::Content;
    for input in [b"[(one) [(two) (three)]] TJ".as_slice(),b"/P << /A [(one) (two)] /B << /C (three) >> >> BDC",b"<00112233445566> Tj"] {
        assert!(Content::decode_strict_with_resource_control(input,ResourceLimits{object_bytes:2,..Default::default()},Arc::new(||false)).is_err());
    }
    for input in [b"[[[[1 2 3 4]]]] TJ".as_slice(),b"/P << /A << /B << /C [1 2 3 4] >> >> >> BDC"] {
        assert!(Content::decode_strict_with_resource_control(input,ResourceLimits{object_nodes:4,..Default::default()},Arc::new(||false)).is_err());
    }
    assert!(std::panic::catch_unwind(||Content::decode_strict_with_resource_control(b"q Q",ResourceLimits::default(),Arc::new(||panic!("test probe unwind")))).is_err());
    assert!(Content::decode_strict_with_resource_control(b"q Q",ResourceLimits::default(),Arc::new(||false)).is_ok());
    let first=Arc::new(std::sync::atomic::AtomicBool::new(true));
    Content::decode_strict_with_resource_control(b"q Q",ResourceLimits::default(),Arc::new(move||{
        if first.swap(false,Ordering::Relaxed) {
            let error=Content::decode_strict_with_resource_control(b"q Q",ResourceLimits::default(),Arc::new(||false)).unwrap_err();
            assert!(error.to_string().contains("nested load scope"));
            std::thread::spawn(||Content::decode_strict_with_resource_control(b"q Q",ResourceLimits::default(),Arc::new(||false)).unwrap()).join().unwrap();
        }
        false
    })).unwrap();
}

fn fixture(modern: bool) -> Vec<u8> {
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let font = doc.add_object(dictionary! {"Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"Helvetica"});
    let contents = doc.add_object(Stream::new(dictionary! {}, b"BT /F 12 Tf 20 30 Td (Preserved words) Tj ET".to_vec()));
    let page = doc.add_object(dictionary! {"Type"=>"Page", "Parent"=>pages, "MediaBox"=>vec![0.into(),0.into(),200.into(),200.into()], "Resources"=>dictionary! {"Font"=>dictionary! {"F"=>font}}, "Contents"=>contents});
    doc.objects.insert(pages, Object::Dictionary(dictionary! {"Type"=>"Pages", "Count"=>1, "Kids"=>vec![Object::Reference(page)]}));
    let catalog = doc.add_object(dictionary! {"Type"=>"Catalog", "Pages"=>pages});
    doc.trailer.set("Root", catalog);
    for _ in 0..20 { doc.add_object(Object::string_literal(vec![b'A'; 2048])); }
    let mut bytes = Vec::new();
    if modern {
        doc.save_with_options(&mut bytes, SaveOptions { use_object_streams: true, use_xref_streams: true, ..Default::default() }).unwrap();
    } else {
        doc.save_to(&mut bytes).unwrap();
    }
    bytes
}

#[test]
fn composition_io_cancelled_loads_restore_scope_for_classic_and_compressed_input() {
    for modern in [false, true] {
        let bytes = fixture(modern);
        let observed = Arc::new(AtomicUsize::new(0));
        let calls = observed.clone();
        Document::load_mem_with_resource_control(&bytes, ResourceLimits::default(), Arc::new(move || {
            calls.fetch_add(1, Ordering::Relaxed); false
        })).unwrap();
        let total = observed.load(Ordering::Relaxed);
        assert!(total > 40, "cancellation must reach parser work");
        for stop in [0, total / 2, total - 1] {
            let calls = Arc::new(AtomicUsize::new(0));
            let error = Document::load_mem_with_resource_control(&bytes, ResourceLimits::default(), Arc::new(move || {
                calls.fetch_add(1, Ordering::Relaxed) >= stop
            })).unwrap_err();
            assert!(error.to_string().contains("cancelled"), "{error}");
            assert_eq!(Document::load_mem_with_resource_limits(&bytes, ResourceLimits::default()).unwrap().extract_text(&[1]).unwrap().trim(), "Preserved words");
            assert!(Document::load_mem(&bytes).is_ok());
        }
    }
}

#[test]
fn composition_io_returns_consumed_allowances_and_preserves_successful_objects() {
    for modern in [false, true] {
        let bytes = fixture(modern);
        let limits = ResourceLimits { input_bytes: bytes.len() * 2 - 1, ..Default::default() };
        let (loaded, remaining) = Document::load_mem_with_resource_control(&bytes, limits, Arc::new(|| false)).unwrap();
        let ordinary = Document::load_mem(&bytes).unwrap();
        assert_eq!(loaded.objects, ordinary.objects);
        assert_eq!(loaded.trailer, ordinary.trailer);
        assert_eq!(remaining.input_bytes, bytes.len() - 1);
        assert!(remaining.object_nodes < limits.object_nodes);
        assert!(remaining.object_bytes < limits.object_bytes);
        assert!(remaining.xref_entries < limits.xref_entries);
        if modern { assert!(remaining.decoded_bytes < limits.decoded_bytes); }
        let error = Document::load_mem_with_resource_control(&bytes, remaining, Arc::new(|| false)).unwrap_err();
        assert!(error.to_string().contains("input bytes"));
    }
}

#[test]
fn composition_io_probe_unwind_cleans_scope_and_controls_are_thread_local() {
    let bytes = fixture(false);
    let panic = std::panic::catch_unwind(|| {
        Document::load_mem_with_resource_control(&bytes, ResourceLimits::default(), Arc::new(|| panic!("test cancellation probe panic")))
    });
    assert!(panic.is_err());
    assert!(Document::load_mem_with_resource_limits(&bytes, ResourceLimits::default()).is_ok());
    let workers: Vec<_> = [false, true].into_iter().map(|cancelled| {
        let bytes = bytes.clone();
        std::thread::spawn(move || {
            for _ in 0..4 {
                let result = Document::load_mem_with_resource_control(&bytes, ResourceLimits::default(), Arc::new(move || cancelled));
                assert_eq!(result.is_err(), cancelled);
                assert!(Document::load_mem_with_resource_limits(&bytes, ResourceLimits::default()).is_ok());
            }
        })
    }).collect();
    for worker in workers { worker.join().unwrap(); }
}

#[test]
fn composition_io_long_tokens_have_periodic_cancellation_checks() {
    for kind in 0..3 {
        let mut counts = vec![];
        for size in [16, 1024 * 1024] {
            let mut doc = Document::with_version("1.7");
            let data = vec![b'A'; size];
            let token = match kind {
                0 => Object::Name(data),
                1 => Object::String(data, lopdf::StringFormat::Literal),
                _ => Object::String(data, lopdf::StringFormat::Hexadecimal),
            };
            let root = doc.add_object(dictionary! {"Type"=>"Catalog", "Payload"=>token});
            doc.trailer.set("Root", root);
            let mut bytes = vec![];
            doc.save_to(&mut bytes).unwrap();
            let count = Arc::new(AtomicUsize::new(0));
            let calls = count.clone();
            Document::load_mem_with_resource_control(&bytes, ResourceLimits::default(), Arc::new(move || {
                calls.fetch_add(1, Ordering::Relaxed); false
            })).unwrap();
            counts.push(count.load(Ordering::Relaxed));
            if size > 16 {
                let calls = Arc::new(AtomicUsize::new(0));
                let stop = counts[0] + 10;
                assert!(Document::load_mem_with_resource_control(&bytes, ResourceLimits::default(), Arc::new(move || {
                    calls.fetch_add(1, Ordering::Relaxed) >= stop
                })).is_err());
            }
        }
        assert!(counts[1] > counts[0] + 50, "long token kind {kind}: {counts:?}");
    }
}

#[test]
fn composition_io_nested_bounded_load_refusal_keeps_the_outer_scope() {
    let bytes = Arc::new(fixture(false));
    let nested_bytes = bytes.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let (doc, _) = Document::load_mem_with_resource_control(&bytes, ResourceLimits::default(), Arc::new(move || {
        if observed.fetch_add(1, Ordering::Relaxed) == 0 {
            let error = Document::load_mem_with_resource_limits(&nested_bytes, ResourceLimits::default()).unwrap_err();
            assert!(error.to_string().contains("nested load scope"));
        }
        false
    })).unwrap();
    assert_eq!(doc.extract_text(&[1]).unwrap().trim(), "Preserved words");
    assert!(calls.load(Ordering::Relaxed) > 40);
    assert!(Document::load_mem_with_resource_limits(&bytes, ResourceLimits::default()).is_ok());
}
