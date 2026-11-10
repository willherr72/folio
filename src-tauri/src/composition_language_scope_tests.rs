use super::*;
use lopdf::{content::Content, Object};
use std::sync::atomic::{AtomicUsize, Ordering};

fn frame(data:&[u8],language:&[u8])->Vec<u8> {
    let (envelope,_)=inspect(data,language,ResourceLimits::default(),Arc::new(||false)).unwrap();
    let mut bytes=envelope.prefix;
    bytes.extend_from_slice(data);
    bytes.extend_from_slice(envelope.suffix);
    bytes
}

#[test]
fn composition_language_scope_retains_exact_content_and_nested_overrides() {
    let data=b"q 0.123456789123 0 0 1 0 0 cm BT /F 12 Tf (Unmarked) Tj /P << /MCID 0 >> BDC (Tagged) Tj /Span << /Lang (es-MX) >> BDC (Spanish \\(EMC\\)) Tj EMC EMC ET Q %tail";
    let framed=frame(data,b"en-US");
    let original=Content::decode_strict(data).unwrap();
    let parsed=Content::decode_strict(&framed).unwrap();
    assert_eq!(parsed.operations.len(),original.operations.len()+2);
    assert_eq!(parsed.operations.first().unwrap().operator,"BDC");
    assert_eq!(parsed.operations.last().unwrap().operator,"EMC");
    for (a,b) in parsed.operations[1..parsed.operations.len()-1].iter().zip(&original.operations) {
        assert_eq!(a.operator,b.operator);assert_eq!(a.operands,b.operands);
    }
    assert!(framed.windows(data.len()).any(|bytes|bytes==data),"original bytes, including precise numbers and comments, must remain intact");
}

#[test]
fn composition_language_scope_encodes_language_as_data_not_operators() {
    for language in [b"".as_slice(),b"en) >> BDC EMC /Injected <<",b"\xfe\xff\0e\0n",b"\0\n\xff"] {
        let parsed=Content::decode_strict(&frame(b"BT (word) Tj ET",language)).unwrap();
        assert_eq!(parsed.operations.len(),5);
        assert_eq!(parsed.operations[0].operands[0],Object::Name(b"Span".to_vec()));
        assert_eq!(parsed.operations[0].operands[1].as_dict().unwrap().get(b"Lang").unwrap().as_str().unwrap(),language);
    }
}

#[test]
fn composition_language_scope_refuses_incomplete_or_crossing_graphics() {
    for data in ["EMC","/P BMC","ET","BT BT ET ET","BT /P BMC ET EMC","/P BMC BT EMC ET","q","Q","0 0 m","0 0 m /P BMC n EMC","BX","EX","BT /F Do ET","BT q Q ET","madeUpOperator"] {
        assert!(inspect(data.as_bytes(),b"en",ResourceLimits::default(),Arc::new(||false)).is_err(),"{data}");
    }
    for data in ["/P BMC BT (word) Tj ET EMC","BT /P BMC (word) Tj EMC ET","q Q","","BT (word) Tj ET"] {
        assert!(inspect(data.as_bytes(),b"en",ResourceLimits::default(),Arc::new(||false)).is_ok(),"{data}");
    }
}

#[test]
fn composition_language_scope_preserves_clips_without_creating_invalid_clipping_sequences() {
    for data in ["q 0 0 10 10 re W n BT 7 Tr (clip) Tj ET Q","q BT 7 Tr (clip) Tj ET Q","q 0 0 10 10 re W n BT 3 Tr (hidden) Tj ET Q"] {
        assert!(inspect(data.as_bytes(),b"en",ResourceLimits::default(),Arc::new(||false)).is_err(),"{data}");
    }
    for data in ["q 0 0 10 10 re W n BT (word) Tj ET Q","BT 7 Tr (clip) Tj ET","q 7 Tr Q BT (word) Tj ET"] {
        assert!(inspect(data.as_bytes(),b"en",ResourceLimits::default(),Arc::new(||false)).is_ok(),"{data}");
    }
}

#[test]
fn composition_language_scope_refuses_clipping_text_excluded_from_visible_outer_scope() {
    // PDF 1.7 10.5.2: a non-clipping outer sequence does not contain clipping
    // objects or nested marked clipping sequences. A syntactically legal Span
    // around these pages therefore cannot establish language for all text.
    for data in [
        "BT 7 Tr (clip) Tj ET /Image Do",
        "q BT 7 Tr (clip) Tj ET /Image Do Q",
        "/Clip BMC BT 7 Tr (clip) Tj ET EMC BT 0 Tr (visible) Tj ET",
        "BT 7 Tr (clip) Tj 0 Tr (visible) Tj ET",
    ] {
        assert!(inspect(data.as_bytes(),b"en",ResourceLimits::default(),Arc::new(||false)).is_err(),"clipping text needs its own language scope: {data}");
    }
    for data in [
        "BT 7 Tr (clip) Tj ET",
        "0 0 10 10 re W n BT (visible) Tj ET",
        "BT 4 Tr (visible clipping text) Tj ET",
    ] {
        assert!(inspect(data.as_bytes(),b"en",ResourceLimits::default(),Arc::new(||false)).is_ok(),"{data}");
    }
}

#[test]
fn composition_language_scope_enforces_limits_and_cancellation() {
    assert!(inspect(b"q Q",&vec![b'x';1025],ResourceLimits::default(),Arc::new(||false)).is_err());
    assert!(inspect(b"q Q",b"en",ResourceLimits{input_bytes:2,..Default::default()},Arc::new(||false)).is_err());
    assert!(inspect(b"q Q",b"en",ResourceLimits{object_nodes:1,..Default::default()},Arc::new(||false)).is_err());
    let nested=format!("{}{}","/P BMC ".repeat(65),"EMC ".repeat(65));
    assert!(inspect(nested.as_bytes(),b"en",ResourceLimits::default(),Arc::new(||false)).is_err());
    let data=b"q BT (word) Tj ET Q";
    let count=Arc::new(AtomicUsize::new(0));let calls=count.clone();
    let (_,left)=inspect(data,b"en",ResourceLimits::default(),Arc::new(move||{calls.fetch_add(1,Ordering::Relaxed);false})).unwrap();
    assert!(left.input_bytes<ResourceLimits::default().input_bytes);
    let total=count.load(Ordering::Relaxed);
    for stop in [0,total/2,total-1] {
        let calls=Arc::new(AtomicUsize::new(0));
        assert!(inspect(data,b"en",ResourceLimits::default(),Arc::new(move||calls.fetch_add(1,Ordering::Relaxed)>=stop)).is_err());
    }
    assert!(inspect(b"BI /W 1 /H 1 ID x EI",b"en",ResourceLimits::default(),Arc::new(||false)).is_err());
}

#[test]
fn composition_language_scope_preserves_unused_aggregate_allowances() {
    let limits=ResourceLimits::default();
    let (_,left)=inspect(b"",b"en",limits,Arc::new(||false)).unwrap();
    assert_eq!(left.object_bytes,limits.object_bytes,"a per-page ceiling must not consume unspent aggregate bytes");
    assert!(limits.object_nodes-left.object_nodes<20);
    let data="q Q ".repeat(100);
    let mut left=limits;
    for _ in 0..400 {
        let (_,next)=inspect(data.as_bytes(),b"en",left,Arc::new(||false)).unwrap();left=next;
    }
    assert!(limits.object_nodes-left.object_nodes>200_000);
    assert!(left.object_nodes>0);
}

#[test]
#[ignore="requires an explicit authored source and a new output path"]
fn composition_language_scope_authored_corpus_probe() {
    use lopdf::{dictionary,Document,Stream};
    use sha2::{Digest,Sha256};
    let source=std::path::PathBuf::from(std::env::var_os("FOLIO_LANGUAGE_SOURCE").unwrap());
    let output=std::path::PathBuf::from(std::env::var_os("FOLIO_LANGUAGE_OUTPUT").unwrap());
    let original=std::fs::read(&source).unwrap();
    let mut document=Document::load_mem_with_resource_limits(&original,ResourceLimits::default()).unwrap();
    let language=document.catalog().unwrap().get(b"Lang").unwrap().as_str().unwrap();
    assert!(language.len()<=1024);
    let language=language.to_vec();
    let mut remaining=ResourceLimits::default();
    for (_,page) in document.get_pages() {
        let data=document.get_page_content_with_limit(page,16*1024*1024).unwrap();
        let (envelope,left)=inspect(&data,&language,remaining,Arc::new(||false)).unwrap();remaining=left;
        let original=document.get_dictionary(page).unwrap().get(b"Contents").unwrap();
        let (_,resolved)=document.dereference(original).unwrap();
        let originals=match resolved {
            Object::Stream(_)=>{assert!(matches!(original,Object::Reference(_)));vec![original.clone()]},
            Object::Array(values)=>{assert!(values.iter().all(|value|matches!(value,Object::Reference(_))));values.clone()},
            _=>panic!("authored probe requires stream references"),
        };
        let first=document.add_object(Stream::new(dictionary!{},envelope.prefix));
        let last=document.add_object(Stream::new(dictionary!{},envelope.suffix.to_vec()));
        let mut contents=vec![Object::Reference(first)];contents.extend(originals);contents.push(last.into());
        document.get_dictionary_mut(page).unwrap().set("Contents",contents);
    }
    let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(output).unwrap();
    document.save_to(&mut file).unwrap();
    assert!(std::fs::read(source).unwrap()==original);
    println!("source_sha256={}",Sha256::digest(&original).iter().map(|byte|format!("{byte:02x}")).collect::<String>());
}
