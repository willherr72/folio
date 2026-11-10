//! Explicit local performance/fidelity workload. Ordinary regression runs skip
//! it because its generated corpus and evidence directory must be supplied.
use folio_engine::PdfEngine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, time::Instant};

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|v| format!("{v:02x}")).collect()
}

fn discovery(engine: &PdfEngine, id: &str, expected: &str) -> (usize, Value, Value) {
    let start = Instant::now();
    let mut snapshots = vec![];
    let mut timings = vec![];
    let mut selected = None;
    for page in 0..2 {
        let before = Instant::now();
        let runs = engine.list_occurrence_text_runs(id, page).unwrap();
        let text_ns = before.elapsed().as_nanos();
        if page == 0 {
            let run = runs.runs.iter().find(|r| r.text == expected).expect("exact header");
            assert!(run.supported, "{:?}", run.reason);
            selected = Some(run.object_index);
        }
        let before = Instant::now();
        let nested = engine.list_occurrence_form_text_runs(id, page).unwrap();
        let nested_ns = before.elapsed().as_nanos();
        assert!(nested.runs.is_empty() && nested.reason.is_none(), "dense page discovery refused: {:?}", nested.reason);
        snapshots.push(json!({"text":runs,"nested":nested}));
        timings.push(json!({"page":page,"textNs":text_ns,"nestedNs":nested_ns}));
    }
    let elapsed = start.elapsed().as_nanos();
    (selected.unwrap(), json!(snapshots), json!({"elapsedNs":elapsed,"pages":timings}))
}

#[test]
#[ignore = "requires local dense corpus and an explicit fresh FOLIO_ADMISSION_OUTPUT directory"]
fn repeated_selection_and_seven_edits_keep_exact_discovery() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let output = PathBuf::from(std::env::var_os("FOLIO_ADMISSION_OUTPUT").expect("explicit evidence destination"));
    let parent = output.parent().unwrap().canonicalize().unwrap();
    let authorized = workspace.join("artifacts/form-admission-reuse-20261103").canonicalize().unwrap();
    assert!(parent.starts_with(&authorized));
    // create_dir refuses an existing run, preserving earlier evidence.
    std::fs::create_dir(&output).unwrap();
    let engine = PdfEngine::start(workspace.join("src-tauri/resources/pdfium/pdfium.dll")).unwrap();
    for (name, relative, pages, expected_hash) in [
        ("two", "artifacts/large-edit-attribution-20261103/dense-two-pages.pdf", 2,
         "4a725da7329179ce5d71cbe01fb389e26c554204f5e780f7c214e4d7b861fdd7"),
        ("dense", "artifacts/corpus-current-20261023/fixtures/dense-300-pages.pdf", 300,
         "476785062c3549b718de185653d5ad8ad8f3987fdae84ebcf65de15c4bd1c3f1"),
    ] {
        let input = workspace.join(relative);
        assert_eq!(hash(&std::fs::read(&input).unwrap()), expected_hash);
        let mut document = engine.open_document(&input).unwrap();
        assert_eq!(document.pages.len(), pages);
        let mut held = vec![document.id.clone()];
        let mut rows = vec![];
        for cycle in 1..=7 {
            let expected = if cycle % 2 == 1 {"FOLIO DENSE PAGE 0001"} else {"FOLIO DENSE PAGE 0001 this"};
            let replacement = if cycle % 2 == 1 {"FOLIO DENSE PAGE 0001 this"} else {"FOLIO DENSE PAGE 0001"};
            let (index, cold_snapshot, cold) = discovery(&engine, &document.id, expected);
            let (again, warm_snapshot, warm) = discovery(&engine, &document.id, expected);
            assert_eq!(index, again);
            assert_eq!(cold_snapshot, warm_snapshot, "cached discovery changed content or eligibility");
            let started = Instant::now();
            let edited = engine.replace_text_occurrence(&document.id, &(0..pages).collect::<Vec<_>>(), 0, index, expected, replacement, None).unwrap();
            let replace_ns = started.elapsed().as_nanos();
            assert_eq!(edited.page_indices, (0..pages).collect::<Vec<_>>());
            document = edited.document;
            assert_eq!(document.pages.len(), pages);
            assert!(engine.extract_text(&document.id, 0).unwrap().contains(replacement));
            held.push(document.id.clone());
            rows.push(json!({"cycle":cycle,"firstDiscovery":cold,"repeatedDiscovery":warm,"replaceNs":replace_ns,
                "firstDiscoverySha256":hash(&serde_json::to_vec(&cold_snapshot).unwrap()),
                "repeatedDiscoverySha256":hash(&serde_json::to_vec(&warm_snapshot).unwrap())}));
        }
        let bytes = engine.source_bytes(&document.id).unwrap();
        std::fs::write(output.join(format!("{name}.pdf")), &bytes).unwrap();
        assert_eq!(held.len(), 8);
        for id in &held {
            engine.close_document(id).unwrap();
            assert!(engine.source_bytes(id).is_err(), "closed source is still accessible");
        }
        assert_eq!(hash(&std::fs::read(&input).unwrap()), expected_hash);
        std::fs::write(output.join(format!("{name}.json")), serde_json::to_vec_pretty(&json!({
            "source":input,"sourceSha256":expected_hash,"outputSha256":hash(&bytes),"pageCount":pages,
            "cycles":7,"retainedSources":8,"closedAllSources":true,"rows":rows,
            "scope":"optimized public PdfEngine, persistent worker, first and repeated discovery on each immutable version; not GUI/Save or pure worker CPU time"
        })).unwrap()).unwrap();
    }
}
