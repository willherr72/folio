//! Opt-in optimized cache acceptance with all retained versions revisited before close.
#![cfg(windows)]
use folio_engine::PdfEngine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::{BTreeMap, HashSet}, fs::{self, OpenOptions}, io::Write,
    path::{Path, PathBuf}, sync::Arc, time::{Duration, Instant}};

fn hash(bytes: &[u8]) -> String { Sha256::digest(bytes).iter().map(|v|format!("{v:02x}")).collect() }
fn create(path: &Path, bytes: &[u8]) { OpenOptions::new().write(true).create_new(true).open(path).unwrap().write_all(bytes).unwrap(); }
fn record(path: &Path, value: &Value) { create(path,&serde_json::to_vec_pretty(value).unwrap()); }

// Same PROCESS_MEMORY_COUNTERS_EX layout and checked API as large_export.rs.
fn private_bytes() -> usize {
    #[repr(C)] #[derive(Default)]
    struct Counters { cb:u32,page_faults:u32,peak_working:usize,working:usize,
        peak_paged:usize,paged:usize,peak_nonpaged:usize,nonpaged:usize,
        pagefile:usize,peak_pagefile:usize,private:usize }
    #[link(name="psapi")]
    unsafe extern "system" { fn GetProcessMemoryInfo(process:isize,counters:*mut Counters,size:u32)->i32; }
    let size=std::mem::size_of::<Counters>() as u32;
    let mut counters=Counters{cb:size,..Default::default()};
    assert_ne!(unsafe{GetProcessMemoryInfo(-1,&mut counters,size)},0);
    assert!(counters.private>0);counters.private
}

struct Version { id:String, bytes:Arc<[u8]>, sha:String, observations:Value, geometry:Value }
fn observations(engine:&PdfEngine,id:&str,pages:usize)->Value {
    let mut indices=vec![0,1,pages-1];indices.sort_unstable();indices.dedup();
    Value::Array(indices.into_iter().map(|page|json!({"page":page,
        "textSha256":hash(&serde_json::to_vec(&engine.page_text(id,page).unwrap()).unwrap()),
        "pngSha256":hash(&engine.render_page(id,page,551).unwrap())})).collect())
}
fn capture(engine:&PdfEngine,id:&str,geometry:Value,pages:usize)->Version {
    let bytes=engine.source_bytes(id).unwrap();
    Version{id:id.into(),sha:hash(&bytes),bytes,observations:observations(engine,id,pages),geometry}
}
fn sample(output:&Path,start:Instant,phase:&str,live:usize,held:&[Version],samples:&mut Vec<Value>) {
    // Each buffer is subsequently verified and reopened, and remains strongly owned.
    let encoded=held.iter().map(|v|v.bytes.len()).sum::<usize>();
    let before=start.elapsed().as_nanos();let private=private_bytes();let after=start.elapsed().as_nanos();
    let row=json!({"phase":phase,"pid":std::process::id(),"startNs":before,"endNs":after,
        "logicalSourceIds":live,"retainedBuffers":held.len(),"encodedBytes":encoded,"privateBytes":private});
    record(&output.join(format!("sample-{:02}.json",samples.len())),&row);
    eprintln!("SAMPLE {phase} private={private} encoded={encoded} live={live}");samples.push(row);
    assert!(private<6*1024_usize.pow(3),"Operational 6 GiB process-private stop");
}

#[test]
#[ignore="requires a fresh FOLIO_RESIDENCY_OUTPUT and explicit FOLIO_RESIDENCY_CASE small/large"]
fn all_retained_versions_survive_bounded_native_document_residency() {
    let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let output=PathBuf::from(std::env::var_os("FOLIO_RESIDENCY_OUTPUT").expect("explicit fresh output"));
    assert_eq!(output.parent().unwrap().canonicalize().unwrap(),root.join("artifacts/lazy-native-documents-20261103").canonicalize().unwrap());
    fs::create_dir(&output).unwrap();fs::create_dir(output.join("versions")).unwrap();
    let case=std::env::var("FOLIO_RESIDENCY_CASE").expect("explicit fixture case");
    let (relative,pages,expected_hash)=match case.as_str(){
        "small"=>("artifacts/large-edit-attribution-20261103/dense-two-pages.pdf",2,"4a725da7329179ce5d71cbe01fb389e26c554204f5e780f7c214e4d7b861fdd7"),
        "large"=>("artifacts/corpus-current-20261023/fixtures/dense-300-pages.pdf",300,"476785062c3549b718de185653d5ad8ad8f3987fdae84ebcf65de15c4bd1c3f1"),
        _=>panic!("unknown fixture case")};
    let input=root.join(relative);assert_eq!(hash(&fs::read(&input).unwrap()),expected_hash);
    let engine=PdfEngine::start(root.join("src-tauri/resources/pdfium/pdfium.dll")).unwrap();
    let start=Instant::now();let mut samples=vec![];let mut held=Vec::new();
    sample(&output,start,"empty",0,&held,&mut samples);
    let mut document=engine.open_document(&input).unwrap();assert_eq!(document.pages.len(),pages);
    held.push(capture(&engine,&document.id,serde_json::to_value(&document.pages).unwrap(),pages));
    sample(&output,start,"opened",1,&held,&mut samples);
    engine.status().unwrap(); // Previous request completed its trim before this reply.
    let resident_owners = Arc::strong_count(&held[0].bytes);
    assert_eq!(resident_owners,5,"fixture owners: caller, source, index, wrapper, native reader");
    let mut edit_timings = vec![];
    let page_indices=(0..pages).collect::<Vec<_>>();
    for index in 1..=100 {
        let edit_started=Instant::now();
        let expected=if index%2==1{"FOLIO DENSE PAGE 0001"}else{"FOLIO DENSE PAGE 0001 this"};
        let replacement=if index%2==1{"FOLIO DENSE PAGE 0001 this"}else{"FOLIO DENSE PAGE 0001"};
        let runs=engine.list_occurrence_text_runs(&document.id,0).unwrap();
        let selected=runs.runs.iter().find(|r|r.text==expected).expect("selected exact header");assert!(selected.supported);
        let edited=engine.replace_text_occurrence(&document.id,&page_indices,0,selected.object_index,expected,replacement,None).unwrap();
        assert_eq!(edited.page_indices,page_indices);document=edited.document;assert_eq!(document.pages.len(),pages);
        assert!(engine.extract_text(&document.id,0).unwrap().contains(replacement));
        held.push(capture(&engine,&document.id,serde_json::to_value(&document.pages).unwrap(),pages));
        assert_eq!(held[index].observations,held[index%2].observations,"repeated native state drift");
        edit_timings.push(json!({"index":index,"elapsedNs":edit_started.elapsed().as_nanos()}));
        if [1,5,10,25,50,75,100].contains(&index){sample(&output,start,&format!("after-edit-{index}"),held.len(),&held,&mut samples);}
    }
    assert_eq!(held.len(),101);
    assert_eq!(held.iter().map(|v|v.id.as_str()).collect::<HashSet<_>>().len(),101);
    assert_eq!(held.iter().map(|v|Arc::as_ptr(&v.bytes) as *const u8 as usize).collect::<HashSet<_>>().len(),101,"distinct retained allocations");
    let mut files=BTreeMap::<String,PathBuf>::new();let mut versions=vec![];
    for (index,v) in held.iter().enumerate(){
        assert_eq!(hash(&v.bytes),v.sha);let path=output.join("versions").join(format!("{}.pdf",v.sha));
        if !files.contains_key(&v.sha){create(&path,&v.bytes);files.insert(v.sha.clone(),path.clone());}
        assert_eq!(hash(&fs::read(&path).unwrap()),v.sha);
        versions.push(json!({"index":index,"sourceId":v.id,"sha256":v.sha,"bytes":v.bytes.len(),"file":path,
            "observations":v.observations,"geometry":v.geometry}));
    }
    record(&output.join("versions.json"),&json!(versions));
    drop(versions);
    sample(&output,start,"before-close-all",101,&held,&mut samples);
    let mut retained_reloads=vec![];
    for (index,v) in held.iter().enumerate() {
        engine.status().unwrap();
        let before=held.iter().map(|v|Arc::strong_count(&v.bytes)).collect::<Vec<_>>();
        assert!(before.iter().all(|n|*n==resident_owners || *n==resident_owners-1));
        assert_eq!(before.iter().filter(|n|**n==resident_owners).count(),4,"idle native reader cap");
        // Ascending traversal must reload every old version, including those
        // initially warm at the tail: preceding accesses evict them first.
        assert_eq!(before[index],resident_owners-1,"retained version was not evicted");
        let reload_started=Instant::now();
        let actual=observations(&engine,&v.id,pages);
        assert_eq!(actual,v.observations,"retained version changed on reload");
        assert!(Arc::ptr_eq(&engine.source_bytes(&v.id).unwrap(),&v.bytes));
        assert_eq!(hash(&v.bytes),v.sha);
        engine.status().unwrap();
        let owners=held.iter().map(|v|Arc::strong_count(&v.bytes)).collect::<Vec<_>>();
        assert!(owners.iter().all(|n|*n==resident_owners || *n==resident_owners-1));
        assert_eq!(owners.iter().filter(|n|**n==resident_owners).count(),4);
        assert_eq!(owners[index],resident_owners);
        retained_reloads.push(json!({"index":index,"sourceId":v.id,"sha256":v.sha,
            "observations":actual,"ownersBefore":before,"ownersAfter":owners,
            "elapsedNs":reload_started.elapsed().as_nanos()}));
        if [0,50,100].contains(&index){sample(&output,start,&format!("retained-reload-{index}"),101,&held,&mut samples);}
    }
    record(&output.join("retained-reloads.json"),&json!(retained_reloads));
    record(&output.join("edit-timings.json"),&json!(edit_timings));
    for v in &held {engine.close_document(&v.id).unwrap();assert!(engine.source_bytes(&v.id).is_err());}
    assert!(held.iter().all(|v|Arc::strong_count(&v.bytes)==1),"retained buffer still has a hidden owner");
    sample(&output,start,"buffers-only",0,&held,&mut samples);
    std::thread::sleep(Duration::from_secs(10));sample(&output,start,"buffers-only-idle-10",0,&held,&mut samples);
    let mut reopened=vec![];
    for (index,v) in held.iter().enumerate(){
        assert_eq!(hash(&v.bytes),v.sha);let path=&files[&v.sha];assert_eq!(hash(&fs::read(path).unwrap()),v.sha);
        let reopened_document=engine.open_document(path).unwrap();assert_ne!(reopened_document.id,v.id);
        assert_eq!(serde_json::to_value(&reopened_document.pages).unwrap(),v.geometry);
        assert_eq!(hash(&engine.source_bytes(&reopened_document.id).unwrap()),v.sha);
        let actual=observations(&engine,&reopened_document.id,pages);assert_eq!(actual,v.observations);
        if [0,50,100].contains(&index){sample(&output,start,&format!("reopen-{index}-live"),1,&held,&mut samples);}
        engine.close_document(&reopened_document.id).unwrap();assert!(engine.source_bytes(&reopened_document.id).is_err());
        reopened.push(json!({"index":index,"sourceId":reopened_document.id,"sha256":v.sha,"observations":actual,"closed":true}));
        if index%25==0{eprintln!("REOPEN {index}");}
    }
    assert!(held.iter().all(|v|Arc::strong_count(&v.bytes)==1));
    sample(&output,start,"reopened-all-buffers-only",0,&held,&mut samples);
    for v in &held{assert_eq!(hash(&v.bytes),v.sha);}
    drop(held);sample(&output,start,"buffers-dropped",0,&[],&mut samples);
    std::thread::sleep(Duration::from_secs(10));sample(&output,start,"buffers-dropped-idle-10",0,&[],&mut samples);
    assert_eq!(hash(&fs::read(&input).unwrap()),expected_hash);
    record(&output.join("result.json"),&json!({"passed":true,"case":case,"pid":std::process::id(),"source":input,"sourceSha256":expected_hash,
        "pages":pages,"edits":100,"retainedVersions":101,"distinctBufferAllocations":101,"soleBufferOwnershipAfterClose":true,
        "allRetainedIdsReloadedBeforeClose":true,"idleNativeReaderLimit":4,"allOriginalIdsClosed":true,"allReopenedIdsClosed":true,"uniqueFiles":files,"reopened":reopened,"samples":samples,
        "scope":"Public PdfEngine with bounded native residency; 100 real edits, 101 immutable retained versions, every old ID forced cold then rendered and text-checked before closing. Reader residency inferred from explicitly checked fixture Arc ownership. All versions also file-reopened. Sampled pages per version; independent full-file audit still required. Not GUI acceptance or a byte/peak-memory limit."}));
}
