//! Explicit generated-fixture workload; no external raw history owners.
use super::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    time::Instant,
};

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect()
}
fn create(path: &Path, bytes: &[u8]) {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap()
        .write_all(bytes)
        .unwrap();
}
fn record(path: &Path, value: &Value) {
    create(path, &serde_json::to_vec_pretty(value).unwrap());
}
fn call<T>(
    rt: &mut WorkerRuntime,
    build: impl FnOnce(mpsc::Sender<EngineResult<T>>) -> WorkerRequest,
) -> T {
    let (reply, receive) = mpsc::channel();
    let (send, requests) = mpsc::channel();
    send.send(build(reply)).unwrap();
    drop(send);
    rt.run(requests);
    receive.recv().unwrap().unwrap()
}
fn bytes(rt: &mut WorkerRuntime, id: &str) -> Arc<[u8]> {
    call(rt, |reply| WorkerRequest::SourceBytes {
        source_id: id.into(),
        reply,
    })
}
fn open(rt: &mut WorkerRuntime, path: &Path) -> DocumentInfo {
    call(rt, |reply| WorkerRequest::Open {
        path: path.into(),
        password: None,
        original_path: None,
        editable_annotations: true,
        register_fonts: true,
        reply,
    })
}
fn close(rt: &mut WorkerRuntime, id: &str) {
    call(rt, |reply| WorkerRequest::Close {
        source_id: id.into(),
        reply,
    });
}
fn observations(rt: &mut WorkerRuntime, id: &str, pages: usize) -> Value {
    let mut indices = vec![0, 1, pages - 1];
    indices.sort_unstable();
    indices.dedup();
    Value::Array(indices.into_iter().map(|page| {
        let text:PageText=call(rt,|reply|WorkerRequest::PageText{source_id:id.into(),page_index:page,reply});
        let png:Vec<u8>=call(rt,|reply|WorkerRequest::Render{source_id:id.into(),page_index:page,width:551,reply});
        json!({"page":page,"textSha256":hash(&serde_json::to_vec(&text).unwrap()),"pngSha256":hash(&png)})
    }).collect())
}
fn private_bytes() -> usize {
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        page_faults: u32,
        peak_working: usize,
        working: usize,
        peak_paged: usize,
        paged: usize,
        peak_nonpaged: usize,
        nonpaged: usize,
        pagefile: usize,
        peak_pagefile: usize,
        private: usize,
    }
    #[link(name = "psapi")]
    unsafe extern "system" {
        fn GetProcessMemoryInfo(process: isize, counters: *mut Counters, size: u32) -> i32;
    }
    let size = std::mem::size_of::<Counters>() as u32;
    let mut counters = Counters {
        cb: size,
        ..Default::default()
    };
    assert_ne!(unsafe { GetProcessMemoryInfo(-1, &mut counters, size) }, 0);
    assert!(counters.private > 0);
    counters.private
}
struct Version {
    id: String,
    sha: String,
    length: usize,
    path: PathBuf,
    observations: Value,
    geometry: Value,
}
fn capture(rt: &mut WorkerRuntime, document: &DocumentInfo, output: &Path) -> Version {
    let source = bytes(rt, &document.id);
    let sha = hash(&source);
    let path = output.join("versions").join(format!("{sha}.pdf"));
    if !path.exists() {
        create(&path, &source);
    }
    assert_eq!(hash(&fs::read(&path).unwrap()), sha);
    let length = source.len();
    drop(source);
    Version {
        id: document.id.clone(),
        sha,
        length,
        path,
        observations: observations(rt, &document.id, document.pages.len()),
        geometry: serde_json::to_value(&document.pages).unwrap(),
    }
}
fn sample(
    rt: &WorkerRuntime,
    output: &Path,
    start: Instant,
    phase: &str,
    versions: &[Version],
    rows: &mut Vec<Value>,
) {
    let usage = rt.source_store.usage();
    let before = start.elapsed().as_nanos();
    let private = private_bytes();
    let after = start.elapsed().as_nanos();
    assert!(usage.raw_cached_bytes <= 128 * 1024 * 1024);
    assert!(usage.encoded_resident_bytes <= 64 * 1024 * 1024);
    assert_eq!(usage.sources, rt.documents.len());
    let expected_live=if matches!(phase,"closed-all"|"reopened-all"){0}else{versions.len()};
    assert_eq!(usage.sources,expected_live);
    assert!(
        private < 6 * 1024usize.pow(3),
        "operational process-private stop"
    );
    let row = json!({"phase":phase,"pid":std::process::id(),"startNs":before,"endNs":after,"privateBytes":private,
        "rawCachedBytes":usage.raw_cached_bytes,"encodedResidentBytes":usage.encoded_resident_bytes,"liveSources":usage.sources,
        "spilledSources":usage.spilled_sources,"spilledBackingBytes":rt.source_store.test_spilled_bytes(),"retainedVersionBytes":versions.iter().map(|v|v.length).sum::<usize>(),"externalRawLeases":0});
    record(&output.join(format!("sample-{:02}.json", rows.len())), &row);
    eprintln!("SAMPLE {phase}: {row}");
    rows.push(row);
}
fn force_cold(rt: &mut WorkerRuntime) {
    for source in rt.documents.values_mut() {
        source.document.evict();
    }
    rt.source_store.clear_test_raw_cache();
    assert_eq!(rt.source_store.usage().raw_cached_bytes, 0);
}
#[test]
#[ignore = "requires fresh FOLIO_STORAGE_OUTPUT and explicit FOLIO_STORAGE_CASE small/large/incompressible"]
fn retained_source_history_workload() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let output =
        PathBuf::from(std::env::var_os("FOLIO_STORAGE_OUTPUT").expect("explicit fresh output"));
    assert_eq!(
        output.parent().unwrap().canonicalize().unwrap(),
        root.join("artifacts/retained-source-storage-20261103")
            .canonicalize()
            .unwrap()
    );
    fs::create_dir(&output).unwrap();
    fs::create_dir(output.join("versions")).unwrap();
    let case = std::env::var("FOLIO_STORAGE_CASE").expect("explicit fixture case");
    let (relative, pages, pinned) = match case.as_str() {
        "small" | "incompressible" => (
            "artifacts/large-edit-attribution-20261103/dense-two-pages.pdf",
            2,
            "4a725da7329179ce5d71cbe01fb389e26c554204f5e780f7c214e4d7b861fdd7",
        ),
        "large" => (
            "artifacts/corpus-current-20261023/fixtures/dense-300-pages.pdf",
            300,
            "476785062c3549b718de185653d5ad8ad8f3987fdae84ebcf65de15c4bd1c3f1",
        ),
        _ => panic!("unknown fixture case"),
    };
    let fixture = root.join(relative);
    let original = fs::read(&fixture).unwrap();
    assert_eq!(hash(&original), pinned);
    let input = if case == "incompressible" {
        let mut pdf = crate::pdf_output::load(&original).unwrap();
        let mut state = 0x6a09e667u32;
        let payload = (0..4 * 1024 * 1024)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state >> 24) as u8
            })
            .collect();
        let stream = pdf.add_object(lopdf::Stream::new(lopdf::dictionary! {}, payload));
        pdf.catalog_mut()
            .unwrap()
            .set("FolioMemoryProbePayload", stream);
        let path = output.join("incompressible-input.pdf");
        create(&path, &crate::pdf_output::save(&mut pdf).unwrap());
        path
    } else {
        fixture.clone()
    };
    drop(original);
    let input_sha = hash(&fs::read(&input).unwrap());
    let mut rt = WorkerRuntime::new(
        &root.join("src-tauri/resources/pdfium/pdfium.dll"),
        Arc::new(FontRegistry::new()),
    )
    .unwrap();
    let start = Instant::now();
    let mut samples = vec![];
    let mut versions = vec![];
    sample(&rt, &output, start, "empty", &versions, &mut samples);
    let mut document = open(&mut rt, &input);
    assert_eq!(document.pages.len(), pages);
    assert!(
        rt.document(&document.id)
            .unwrap()
            .units()
            .unwrap()
            .iter()
            .all(|v| *v == 1.),
        "worker probes require canonical point fixtures"
    );
    versions.push(capture(&mut rt, &document, &output));
    sample(&rt, &output, start, "opened", &versions, &mut samples);
    let mut timings = vec![];
    let indices = (0..pages).collect::<Vec<_>>();
    for index in 1..=100 {
        let edit_start = Instant::now();
        let expected = if index % 2 == 1 {
            "FOLIO DENSE PAGE 0001"
        } else {
            "FOLIO DENSE PAGE 0001 this"
        };
        let replacement = if index % 2 == 1 {
            "FOLIO DENSE PAGE 0001 this"
        } else {
            "FOLIO DENSE PAGE 0001"
        };
        let runs: TextRuns = call(&mut rt, |reply| WorkerRequest::ListOccurrenceTextRuns {
            source_id: document.id.clone(),
            page_index: 0,
            reply,
        });
        let selected = runs
            .runs
            .iter()
            .find(|r| r.text == expected)
            .expect("exact header");
        assert!(selected.supported);
        let edited: PageContentReplacement =
            call(&mut rt, |reply| WorkerRequest::ReplaceTextOccurrence {
                source_id: document.id.clone(),
                occurrences: indices.clone(),
                selected_occurrence: 0,
                object_index: selected.object_index,
                expected_text: expected.into(),
                replacement: replacement.into(),
                font_id: None,
                reply,
            });
        assert_eq!(edited.page_indices, indices);
        document = edited.document;
        assert_eq!(document.pages.len(), pages);
        versions.push(capture(&mut rt, &document, &output));
        assert_eq!(
            versions[index].observations,
            versions[index % 2].observations,
            "repeated state drift"
        );
        timings.push(json!({"index":index,"elapsedNs":edit_start.elapsed().as_nanos()}));
        if [1, 5, 10, 25, 50, 75, 100].contains(&index) {
            sample(
                &rt,
                &output,
                start,
                &format!("after-edit-{index}"),
                &versions,
                &mut samples,
            );
        }
    }
    assert_eq!(versions.len(), 101);
    assert_eq!(
        versions.iter().map(|v| &v.id).collect::<HashSet<_>>().len(),
        101
    );
    if case == "incompressible" {
        assert!(rt.source_store.usage().spilled_sources > 0);
    }
    record(&output.join("versions.json"),&json!(versions.iter().enumerate().map(|(index,v)|json!({"index":index,"sourceId":v.id,"sha256":v.sha,"bytes":v.length,"file":v.path,"observations":v.observations,"geometry":v.geometry})).collect::<Vec<_>>()));
    record(&output.join("edit-timings.json"), &json!(timings));
    let mut reloaded = vec![];
    for (index, v) in versions.iter().enumerate() {
        force_cold(&mut rt);
        let raw_before = rt.source_store.usage().raw_cached_bytes;
        let stamp = Instant::now();
        let actual = bytes(&mut rt, &v.id);
        assert_eq!(hash(&actual), v.sha);
        assert_eq!(actual.as_ref(), fs::read(&v.path).unwrap());
        drop(actual);
        let observation = observations(&mut rt, &v.id, pages);
        assert_eq!(observation, v.observations);
        reloaded.push(json!({"index":index,"sourceId":v.id,"sha256":v.sha,"observations":observation,"elapsedNs":stamp.elapsed().as_nanos(),"forcedCold":true,"rawBeforeBytes":raw_before,"rawAfterBytes":rt.source_store.usage().raw_cached_bytes}));
        if [0, 50, 100].contains(&index) {
            sample(
                &rt,
                &output,
                start,
                &format!("reload-{index}"),
                &versions,
                &mut samples,
            );
        }
    }
    record(&output.join("retained-reloads.json"), &json!(reloaded));
    for v in &versions {
        close(&mut rt, &v.id);
        assert!(rt.document(&v.id).is_err());
    }
    sample(&rt, &output, start, "closed-all", &versions, &mut samples);
    let usage = rt.source_store.usage();
    assert_eq!(
        (
            usage.sources,
            usage.raw_cached_bytes,
            usage.encoded_resident_bytes,
            usage.spilled_sources
        ),
        (0, 0, 0, 0)
    );
    let mut reopened = vec![];
    for (index, v) in versions.iter().enumerate() {
        let info = open(&mut rt, &v.path);
        assert_ne!(info.id, v.id);
        assert_eq!(serde_json::to_value(&info.pages).unwrap(), v.geometry);
        assert_eq!(hash(&bytes(&mut rt, &info.id)), v.sha);
        let observed = observations(&mut rt, &info.id, pages);
        assert_eq!(observed, v.observations);
        close(&mut rt, &info.id);
        reopened.push(json!({"index":index,"sourceId":info.id,"sha256":v.sha,"observations":observed,"closed":true}));
    }
    sample(&rt, &output, start, "reopened-all", &versions, &mut samples);
    assert_eq!(rt.source_store.usage().sources, 0);
    assert_eq!(hash(&fs::read(&fixture).unwrap()), pinned);
    assert_eq!(hash(&fs::read(&input).unwrap()), input_sha);
    record(
        &output.join("result.json"),
        &json!({"passed":true,"case":case,"pid":std::process::id(),"pages":pages,"edits":100,"retainedVersions":101,
        "source":input,"sourceSha256":input_sha,"samples":samples,"reopened":reopened,"allOriginalIdsClosed":true,"allReopenedIdsClosed":true,"allRetainedIdsReloadedBeforeClose":true,
        "scope":"Real serialized worker requests with private cache diagnostics. No external raw history owners; exact saved bytes and sampled native pages checked. Full independent page audit and GUI acceptance remain separate."}),
    );
}
