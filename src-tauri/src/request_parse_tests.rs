use super::*;
use lopdf::{dictionary,Object};
use std::cell::{Cell,RefCell};

fn pdf(tag:u8,length:usize,declared:Option<usize>)->Vec<u8> {
    let mut bytes=b"%PDF-1.7\n".to_vec();let mut offsets=vec![];
    for (i,text) in ["<< /Type /Catalog /Pages 2 0 R >>","<< /Type /Pages /Kids [3 0 R] /Count 1 >>","<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] /Contents 4 0 R >>"].iter().enumerate(){
        offsets.push(bytes.len());bytes.extend_from_slice(format!("{} 0 obj\n{text}\nendobj\n",i+1).as_bytes());
    }
    offsets.push(bytes.len());bytes.extend_from_slice(format!("4 0 obj\n<< /Length {} >>\nstream\n",declared.unwrap_or(length)).as_bytes());
    bytes.resize(bytes.len()+length,tag);bytes.extend_from_slice(b"\nendstream\nendobj\n");
    let xref=bytes.len();bytes.extend_from_slice(b"xref\n0 5\n0000000000 65535 f \n");
    for offset in offsets {bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());}
    bytes.extend_from_slice(format!("trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes());bytes
}
fn counted(bytes:&[u8],policy:Policy,count:&Cell<usize>)->Document {
    load_with(bytes,policy,|b,p|{count.set(count.get()+1);Document::load_mem_with_options(b,p.options())}).unwrap()
}

#[test]
fn one_parse_per_exact_key_within_one_scope() {
    let bytes=pdf(b' ',80*1024,None);let count=Cell::new(0);let _scope=Scope::enter();
    counted(&bytes,Policy::Strict64,&count);counted(&bytes,Policy::Strict64,&count);
    assert_eq!(count.get(),1);assert_eq!(stats().0,1);
}
#[test]
fn outside_or_after_a_scope_never_reuses_an_entry() {
    let bytes=pdf(b' ',80*1024,None);let count=Cell::new(0);
    {let _scope=Scope::enter();counted(&bytes,Policy::Strict64,&count);counted(&bytes,Policy::Strict64,&count);}
    assert_eq!(stats(),(0,0,0,0));counted(&bytes,Policy::Strict64,&count);counted(&bytes,Policy::Strict64,&count);assert_eq!(count.get(),3);
}
#[test]
fn returned_document_mutation_cannot_change_the_template() {
    let bytes=pdf(b' ',80*1024,None);let count=Cell::new(0);let _scope=Scope::enter();
    let mut first=counted(&bytes,Policy::Strict64,&count);first.trailer.set("Mutation",true);first.objects.clear();
    let next=counted(&bytes,Policy::Strict64,&count);assert!(!next.trailer.has(b"Mutation"));assert!(!next.objects.is_empty());assert_eq!(count.get(),1);
}
#[test]
fn bytes_not_pointer_or_length_determine_identity() {
    let mut bytes=pdf(b' ',80*1024,None);let count=Cell::new(0);let _scope=Scope::enter();
    counted(&bytes,Policy::Strict64,&count);
    let offset=bytes.windows(7).position(|w|w==b"stream\n").unwrap()+7;bytes[offset]=b'%';
    counted(&bytes,Policy::Strict64,&count);counted(&bytes,Policy::Strict64,&count);assert_eq!(count.get(),2);
}
#[test]
fn parser_policies_are_distinct_and_options_are_exact() {
    let bytes=pdf(b' ',80*1024,None);let count=Cell::new(0);let _scope=Scope::enter();
    counted(&bytes,Policy::Strict64,&count);counted(&bytes,Policy::Compatible64,&count);counted(&bytes,Policy::Strict64,&count);assert_eq!(count.get(),2);
    for policy in [Policy::Strict64,Policy::Compatible64,Policy::Compatible16] {
        let options=policy.options();assert_eq!(options.strict,policy==Policy::Strict64);assert!(!options.skip_decryption && options.password.is_none() && options.filter.is_none());
        assert_eq!(options.max_decompressed_size,Some(if policy==Policy::Compatible16 {16*1024*1024}else{64*1024*1024}));
    }
}
#[test]
fn repaired_parse_never_authorizes_strict_loading_and_errors_are_retried() {
    let bytes=pdf(b' ',80*1024,Some(0));let _scope=Scope::enter();
    assert!(load(&bytes,Policy::Compatible16).is_ok());assert!(load(&bytes,Policy::Strict64).is_err());assert!(load(&bytes,Policy::Strict64).is_err());
    let calls=Cell::new(0);for _ in 0..2 {assert!(load_with(&bytes,Policy::Strict64,|_,_|{calls.set(calls.get()+1);Err(lopdf::Error::ObjectNotFound((999,0)))}).is_err());}assert_eq!(calls.get(),2);
}
#[test]
fn nested_scopes_share_until_the_last_guard_drops() {
    let bytes=pdf(b' ',80*1024,None);let count=Cell::new(0);let outer=Scope::enter();let inner=Scope::enter();
    counted(&bytes,Policy::Strict64,&count);drop(outer);counted(&bytes,Policy::Strict64,&count);assert_eq!(count.get(),1);
    drop(inner);assert_eq!(stats().0,0);let _new=Scope::enter();counted(&bytes,Policy::Strict64,&count);assert_eq!(count.get(),2);
}
#[test]
fn unwind_clears_entries_and_preserves_the_original_error() {
    let bytes=pdf(b' ',80*1024,None);let count=Cell::new(0);
    let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{let _scope=Scope::enter();counted(&bytes,Policy::Strict64,&count);panic!("original panic")}));
    assert!(result.is_err());assert_eq!(stats().0,0);let _new=Scope::enter();counted(&bytes,Policy::Strict64,&count);assert_eq!(count.get(),2);
}
#[test]
fn worker_threads_do_not_share_entries() {
    let bytes=pdf(b' ',80*1024,None);let count=Cell::new(0);let _scope=Scope::enter();counted(&bytes,Policy::Strict64,&count);
    std::thread::spawn(move||{let calls=Cell::new(0);let _own=Scope::enter();counted(&bytes,Policy::Strict64,&calls);counted(&bytes,Policy::Strict64,&calls);assert_eq!(calls.get(),1);}).join().unwrap();
    assert_eq!(stats().0,1);
}
#[test]
fn a_stale_parse_completion_cannot_fill_a_later_request() {
    let bytes=pdf(b' ',80*1024,None);let old=Scope::enter();let new=RefCell::new(None);
    load_with(&bytes,Policy::Strict64,|b,p|{drop(old);new.replace(Some(Scope::enter()));Document::load_mem_with_options(b,p.options())}).unwrap();
    assert_eq!(stats().0,0);drop(new);
}
#[test]
fn two_entry_lru_evicts_the_least_recently_used_key() {
    let a=pdf(b'A',80*1024,None);let b=pdf(b'B',80*1024,None);let c=pdf(b'C',80*1024,None);let count=Cell::new(0);let _scope=Scope::enter();
    counted(&a,Policy::Strict64,&count);counted(&b,Policy::Strict64,&count);counted(&a,Policy::Strict64,&count);counted(&c,Policy::Strict64,&count);counted(&b,Policy::Strict64,&count);
    assert_eq!(count.get(),4);assert_eq!(stats().0,2);
}
#[test]
fn small_and_oversized_encoded_inputs_stay_uncached() {
    let count=Cell::new(0);let _scope=Scope::enter();
    for bytes in [pdf(b' ',1024,None),pdf(b' ',MAX_INPUT+1,None)] {counted(&bytes,Policy::Strict64,&count);counted(&bytes,Policy::Strict64,&count);}
    assert_eq!(count.get(),4);assert_eq!(stats().0,0);
}
#[test]
fn aggregate_encoded_budget_is_enforced_before_a_second_copy() {
    let a=pdf(b'A',5*1024*1024,None);let b=pdf(b'B',5*1024*1024,None);let count=Cell::new(0);let _scope=Scope::enter();
    counted(&a,Policy::Strict64,&count);counted(&b,Policy::Strict64,&count);assert_eq!(stats().0,1);assert!(stats().1<=MAX_INPUT);
}
#[test]
fn large_decoded_payloads_dense_models_and_depth_are_not_retained() {
    let bytes=pdf(b' ',80*1024,None);let _scope=Scope::enter();let calls=Cell::new(0);
    for case in 0..3 {for _ in 0..2 {
        load_with(&bytes,Policy::Strict64,|_,_|{
            calls.set(calls.get()+1);let mut doc=Document::new();
            match case {
                0=>{doc.add_object(Object::String(vec![b'X';MAX_MODEL+1],lopdf::StringFormat::Literal));},
                1=>{for i in 1..=MAX_VALUES+1 {doc.objects.insert((i as u32,0),Object::Null);}},
                _=>{let mut object=Object::Null;for _ in 0..65{object=Object::Array(vec![object]);}doc.add_object(object);},
            }
            Ok(doc)
        }).unwrap();assert_eq!(stats().0,0);
    }}assert_eq!(calls.get(),6);
}
#[test]
fn populated_bookmark_tables_are_not_retained() {
    let bytes=pdf(b' ',80*1024,None);let _scope=Scope::enter();
    load_with(&bytes,Policy::Strict64,|_,_|{let mut doc=Document::new();doc.bookmarks.push(1);doc.trailer=dictionary!{"Root"=>Object::Reference((1,0))};Ok(doc)}).unwrap();
    assert_eq!(stats().0,0);
}

#[test]
fn aggregate_model_and_value_budgets_evict_before_extra_template_clones() {
    let a=pdf(b'A',80*1024,None);let b=pdf(b'B',80*1024,None);
    for dense in [false,true] {
        let _scope=Scope::enter();
        for bytes in [&a,&b] {
            load_with(bytes,Policy::Strict64,|_,_|{let mut doc=Document::new();
                if dense {for i in 1..=60_000{doc.objects.insert((i,0),Object::Null);}}
                else {doc.add_object(Object::String(vec![b'X';9*1024*1024],lopdf::StringFormat::Literal));}
                Ok(doc)
            }).unwrap();
        }
        let (entries,raw,model,values)=stats();assert_eq!(entries,1);assert!(raw<=MAX_INPUT && model<=MAX_MODEL && values<=MAX_VALUES);
    }
}

#[test]
fn returning_an_error_clears_the_request_cache() {
    let bytes=pdf(b' ',80*1024,None);
    let result=(||->Result<(),&str>{let _scope=Scope::enter();load(&bytes,Policy::Strict64).unwrap();assert_eq!(stats().0,1);Err("original validation failure")})();
    assert_eq!(result,Err("original validation failure"));assert_eq!(stats().0,0);
}

#[test]
fn automatically_decrypted_document_keys_are_not_retained() {
    use lopdf::encryption::{EncryptionState,EncryptionVersion,Permissions,crypt_filters::{Aes128CryptFilter,CryptFilter}};
    use std::{collections::BTreeMap,sync::Arc};
    let mut doc=Document::load_mem(&pdf(b' ',80*1024,None)).unwrap();
    let id=Object::string_literal(vec![7u8;16]);doc.trailer.set("ID",vec![id.clone(),id]);
    let mut filters:BTreeMap<Vec<u8>,Arc<dyn CryptFilter>>=BTreeMap::new();filters.insert(b"StdCF".to_vec(),Arc::new(Aes128CryptFilter));
    let state=EncryptionState::try_from(EncryptionVersion::V4{document:&doc,encrypt_metadata:true,crypt_filters:filters,stream_filter:b"StdCF".to_vec(),string_filter:b"StdCF".to_vec(),owner_password:"owner",user_password:"",permissions:Permissions::all()}).unwrap();
    doc.encrypt(&state).unwrap();let mut bytes=vec![];doc.save_to(&mut bytes).unwrap();assert!(bytes.len()>=MIN_INPUT);
    let count=Cell::new(0);let _scope=Scope::enter();
    assert!(counted(&bytes,Policy::Strict64,&count).encryption_state.is_some());assert!(counted(&bytes,Policy::Strict64,&count).encryption_state.is_some());
    assert_eq!(count.get(),2);assert_eq!(stats().0,0);
}

#[test]
fn policies_each_validate_once_and_share_only_an_identical_template() {
    let bytes=pdf(b' ',80*1024,None);let calls=Cell::new(0);let _scope=Scope::enter();
    for _ in 0..2 {for policy in [Policy::Strict64,Policy::Compatible64,Policy::Compatible16]{counted(&bytes,policy,&calls);}}
    assert_eq!(calls.get(),3);assert_eq!(stats().0,1);
}

#[test]
fn xref_and_key_storage_is_byte_charged_not_miscounted_as_pdf_object_values() {
    let bytes=pdf(b' ',80*1024,None);let calls=Cell::new(0);let _scope=Scope::enter();
    for _ in 0..2 {load_with(&bytes,Policy::Strict64,|_,_|{
        calls.set(calls.get()+1);let mut doc=Document::new();
        for i in 1..=70_000{doc.objects.insert((i,0),Object::Null);doc.reference_table.entries.insert(i,lopdf::xref::XrefEntry::Normal{offset:1,generation:0});}
        Ok(doc)
    }).unwrap();}
    assert_eq!(calls.get(),1);assert_eq!(stats().0,1);assert_eq!(stats().3,70_001);assert!(stats().2<=MAX_MODEL);
}

#[test]
fn policy_models_with_different_order_or_float_bits_cannot_share() {
    let bytes=pdf(b' ',80*1024,None);
    for ordered in [false,true] {
        let _scope=Scope::enter();
        for policy in [Policy::Strict64,Policy::Compatible64]{
            load_with(&bytes,policy,|_,p|{let mut doc=Document::new();
                if ordered {for key in if p==Policy::Strict64 {['A','B']}else{['B','A']}{doc.trailer.set(key.to_string(),Object::Integer(1));}}
                else {doc.trailer.set("Float",Object::Real(if p==Policy::Strict64 {0.0}else{-0.0}));}
                Ok(doc)
            }).unwrap();
        }
        assert_eq!(stats().0,2);
    }
}

#[test]
fn read_only_views_share_the_template_but_owned_mutation_is_independent() {
    let bytes=pdf(b' ',80*1024,None);let _scope=Scope::enter();
    let first=read(&bytes,Policy::Strict64).unwrap();let second=read(&bytes,Policy::Strict64).unwrap();
    assert!(std::ptr::eq(&*first,&*second));
    let mut owned=first.into_owned();owned.objects.clear();assert!(!second.objects.is_empty());
    assert!(!read(&bytes,Policy::Strict64).unwrap().objects.is_empty());
}

#[test]
fn a_read_view_remains_valid_after_its_request_scope_ends() {
    let bytes=pdf(b' ',80*1024,None);
    let view={let _scope=Scope::enter();read(&bytes,Policy::Strict64).unwrap()};
    assert_eq!(stats().0,0);assert!(!view.objects.is_empty());
}

#[test]
fn pressure_from_a_transient_output_cannot_evict_the_primary_model() {
    let a=pdf(b'A',80*1024,None);let b=pdf(b'B',80*1024,None);let calls=Cell::new(0);let _scope=Scope::enter();
    counted(&a,Policy::Strict64,&calls);
    load_with(&b,Policy::Strict64,|_,_|{let mut doc=Document::new();doc.add_object(Object::String(vec![b'X';MAX_MODEL-1024],lopdf::StringFormat::Literal));Ok(doc)}).unwrap();
    counted(&a,Policy::Strict64,&calls);assert_eq!(calls.get(),1);
}
