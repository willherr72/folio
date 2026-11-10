use super::*;
fn aliases()->BTreeMap<Vec<u8>,Vec<u8>>{[(b"F1".to_vec(),b"Folio2_F1".to_vec())].into()}
fn run(bytes:&[u8],aliases:&BTreeMap<Vec<u8>,Vec<u8>>)->EngineResult<Option<Vec<u8>>>{rewrite(bytes,aliases,&mut Budget::default(),Arc::new(||false),&mut ||Ok(()))}

#[test]
fn composition_form_da_rewrites_only_font_token_bytes(){
    let input=b"% /F1 1 Tf\r\nq /F#31 12.123456789012 Tf 0.123456789012 g /F1 9 Tf Q %tail";
    let expected=b"% /F1 1 Tf\r\nq /Folio2_F1 12.123456789012 Tf 0.123456789012 g /Folio2_F1 9 Tf Q %tail";
    assert_eq!(run(input,&aliases()).unwrap().unwrap(),expected);
}

#[test]
fn composition_form_da_names_inside_strings_arrays_and_dictionaries_are_untouched(){
    let input=b"/P << /Name /F1 /Data [(text \\( /F1 12 Tf \\)) <2f4631> [/F1]] >> BDC /F1 12 Tf (literal /F1 14 Tf) Tj EMC";
    let expected=b"/P << /Name /F1 /Data [(text \\( /F1 12 Tf \\)) <2f4631> [/F1]] >> BDC /Folio2_F1 12 Tf (literal /F1 14 Tf) Tj EMC";
    assert_eq!(run(input,&aliases()).unwrap().unwrap(),expected);
}

#[test]
fn composition_form_da_identity_empty_and_comment_only_programs_keep_original_objects(){
    let identity=[(b"F1".to_vec(),b"F1".to_vec())].into();
    for input in [b"/F#31 12.000000001 Tf %tail".as_slice(),b"",b"% /F1 12 Tf"] {
        assert!(run(input,&identity).unwrap().is_none());
    }
}

#[test]
fn composition_form_da_binary_aliases_are_escaped_and_unmapped_names_refuse(){
    let aliases=[(vec![0x80,0xfe],vec![0x81,b'/',b' ',b'#'])].into();
    assert_eq!(run(b"/#80#FE 12 Tf",&aliases).unwrap().unwrap(),b"/#81#2F#20#23 12 Tf");
    assert!(run(b"/Other 12 Tf",&self::aliases()).is_err());
    for input in [b"/F1 Tf".as_slice(),b"/F1 -2 Tf",b"12 12 Tf",b"(unclosed",b"BI /W 1 /H 1 ID a EI"] {assert!(run(input,&self::aliases()).is_err());}
}

#[test]
fn composition_form_da_budget_and_cancellation_never_modify_the_input(){
    let input=b"/F1 12 Tf 0.123456789 g".to_vec();let before=input.clone();let mut calls=0;
    rewrite(&input,&aliases(),&mut Budget::default(),Arc::new(||false),&mut ||{calls+=1;Ok(())}).unwrap();
    for stop in 1..=calls {let mut count=0;assert!(rewrite(&input,&aliases(),&mut Budget::default(),Arc::new(||false),&mut ||{count+=1;if count==stop{Err(EngineError::InvalidRequest("cancel".into()))}else{Ok(())}}).is_err());}
    assert!(rewrite(&input,&aliases(),&mut Budget::default(),Arc::new(||true),&mut ||Ok(())).is_err());
    assert!(rewrite(&input,&aliases(),&mut Budget{metadata:1,..Default::default()},Arc::new(||false),&mut ||Ok(())).is_err());
    assert!(run(&vec![b' ';65537],&aliases()).is_err());
    let oversized=[(b"F1".to_vec(),vec![b'A';65536])].into();assert!(run(&input,&oversized).is_err());
    assert_eq!(before,input);
}

#[test]
fn composition_form_da_spans_follow_scalar_and_aggregate_lexical_boundaries(){
    for prefix in [
        b"1-2+.3 4. 5Op\n".as_slice(),
        b"true false null X\n",
        b"(line\\\r\ncontinue\\(pair\\)) Tj\n",
        b"[12 3 true null << /K <01> >>] X\n",
        b"/P << /Nested << /Name /F#31 >> >> BDC\n",
        b"% /F1 2 Tf\r\n",
    ] {
        let input=[prefix,b"/F#31 +.125Tf\r\n% EOF"].concat();
        let expected=[prefix,b"/Folio2_F1 +.125Tf\r\n% EOF"].concat();
        assert_eq!(run(&input,&aliases()).unwrap().unwrap(),expected,"prefix {:?}",String::from_utf8_lossy(prefix));
    }
}

#[test]
fn composition_form_da_both_parses_share_limits_and_parser_cancellation(){
    use std::sync::atomic::{AtomicUsize,Ordering};
    let input=b"/F1 12 Tf";let expected=b"/Folio2_F1 12 Tf";
    let mut exact=Budget::default();exact.parser.input_bytes=input.len()+expected.len();
    assert_eq!(rewrite(input,&aliases(),&mut exact,Arc::new(||false),&mut ||Ok(())).unwrap().unwrap(),expected);
    assert_eq!(exact.parser.input_bytes,0);
    assert!(rewrite(input,&aliases(),&mut exact,Arc::new(||false),&mut ||Ok(())).is_err());
    let mut short=Budget::default();short.parser.input_bytes=input.len();
    assert!(rewrite(input,&aliases(),&mut short,Arc::new(||false),&mut ||Ok(())).is_err(),"verification parse cannot reset the transaction budget");
    let calls=Arc::new(AtomicUsize::new(0));let observed=calls.clone();
    rewrite(input,&aliases(),&mut Budget::default(),Arc::new(move||{observed.fetch_add(1,Ordering::Relaxed);false}),&mut ||Ok(())).unwrap();
    let total=calls.load(Ordering::Relaxed);assert!(total>20);
    for stop in [1,total/2,total-1] {
        let calls=Arc::new(AtomicUsize::new(0));
        assert!(rewrite(input,&aliases(),&mut Budget::default(),Arc::new(move||calls.fetch_add(1,Ordering::Relaxed)>=stop),&mut ||Ok(())).is_err());
    }
}
