//! Reproducible synthetic PDF compatibility checks and opt-in measured workloads.
use folio_engine::{ExportRequest, PdfEngine};
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::Instant,
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}
fn source_fingerprint(path: &Path) -> u64 {
    // Streaming mutation detection; the runner separately verifies manifest SHA-256.
    use std::hash::Hasher;
    let mut digest = std::collections::hash_map::DefaultHasher::new();
    let mut file = fs::File::open(path).unwrap();
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer).unwrap();
        if n == 0 {
            break;
        }
        digest.write(&buffer[..n]);
    }
    digest.finish()
}
fn elapsed(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}
fn text(engine: &PdfEngine, id: &str, page: usize) -> String {
    engine
        .page_text(id, page)
        .unwrap()
        .characters
        .into_iter()
        .map(|c| c.text)
        .collect()
}

#[cfg(windows)]
mod memory {
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    };
    use std::thread::{self, JoinHandle};
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
    pub struct Sampler {
        active: Arc<AtomicBool>,
        private: Arc<AtomicUsize>,
        working: Arc<AtomicUsize>,
        task: Option<JoinHandle<()>>,
    }
    impl Sampler {
        pub fn start() -> Self {
            let active = Arc::new(AtomicBool::new(true));
            let private = Arc::new(AtomicUsize::new(0));
            let working = Arc::new(AtomicUsize::new(0));
            let (run, p, w) = (active.clone(), private.clone(), working.clone());
            let task = thread::spawn(move || {
                while run.load(Ordering::Relaxed) {
                    let mut c = Counters {
                        cb: std::mem::size_of::<Counters>() as u32,
                        ..Counters::default()
                    };
                    if unsafe {
                        GetProcessMemoryInfo(-1, &mut c, std::mem::size_of::<Counters>() as u32)
                    } != 0
                    {
                        p.fetch_max(c.private, Ordering::Relaxed);
                        w.fetch_max(c.peak_working, Ordering::Relaxed);
                    }
                    thread::sleep(std::time::Duration::from_millis(10));
                }
            });
            Self {
                active,
                private,
                working,
                task: Some(task),
            }
        }
        pub fn finish(mut self) -> serde_json::Value {
            self.active.store(false, Ordering::Relaxed);
            self.task.take().unwrap().join().unwrap();
            serde_json::json!({"sampledPeakPrivateBytes": self.private.load(Ordering::Relaxed), "processPeakWorkingSetBytes": self.working.load(Ordering::Relaxed), "sampleIntervalMs":10})
        }
    }
    impl Drop for Sampler {
        fn drop(&mut self) {
            self.active.store(false, Ordering::Relaxed);
            if let Some(task) = self.task.take() {
                let _ = task.join();
            }
        }
    }
}
#[cfg(not(windows))]
mod memory {
    pub struct Sampler;
    impl Sampler {
        pub fn start() -> Self {
            Self
        }
        pub fn finish(self) -> serde_json::Value {
            serde_json::json!({"unavailable":true})
        }
    }
}

fn check_fixture(directory: &Path, fixture: &Value, sequential_count: usize) -> Value {
    let filename = fixture["file"].as_str().unwrap();
    let source = directory.join(filename);
    let fingerprint = source_fingerprint(&source);
    let temporary = tempfile::tempdir().unwrap();
    let sampler = memory::Sampler::start();
    let engine = PdfEngine::start(
        std::env::var_os("FOLIO_PDFIUM_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| root().join("src-tauri/resources/pdfium/pdfium.dll")),
    )
    .unwrap();
    let begin = Instant::now();
    let document = engine.open_document(&source).unwrap();
    let open_ms = elapsed(begin);
    assert_eq!(
        document.pages.len(),
        fixture["pages"].as_u64().unwrap() as usize,
        "{filename}"
    );
    for (page, dimensions) in document
        .pages
        .iter()
        .zip(fixture["dimensions"].as_array().unwrap())
    {
        assert!((page.width as f64 - dimensions[0].as_f64().unwrap()).abs() < 0.1);
        assert!((page.height as f64 - dimensions[1].as_f64().unwrap()).abs() < 0.1);
    }
    for probe in fixture["textProbes"].as_array().unwrap() {
        assert!(
            text(&engine, &document.id, 0).contains(probe.as_str().unwrap()),
            "embedded Unicode source extraction"
        );
    }
    let mut samples = vec![];
    for sample in fixture["samples"].as_array().unwrap() {
        let page = sample["page"].as_u64().unwrap() as usize;
        let begin = Instant::now();
        let extracted = text(&engine, &document.id, page);
        let extraction_ms = elapsed(begin);
        assert!(
            extracted.contains(sample["text"].as_str().unwrap()),
            "{filename} page {page}: expected source text"
        );
        if fixture["kind"] == "scan" {
            assert!(extracted.trim().is_empty(), "scans must stay image-only");
        }
        let begin = Instant::now();
        let png = engine.render_page(&document.id, page, 900).unwrap();
        let render_ms = elapsed(begin);
        let pixels = image::load_from_memory(&png).unwrap().into_rgb8();
        assert_eq!(pixels.width(), 900);
        assert!(
            pixels
                .pixels()
                .any(|p| p.0.iter().any(|value| *value < 180)),
            "{filename}: blank raster"
        );
        assert!(
            (pixels.height() as f64 / pixels.width() as f64
                - document.pages[page].height as f64 / document.pages[page].width as f64)
                .abs()
                < 0.01
        );
        samples.push(json!({"page":page,"extractMs":extraction_ms,"renderMs":render_ms,"pngBytes":png.len(),"characters":extracted.chars().count()}));
    }
    let begin = Instant::now();
    let repeat = engine.render_page(&document.id, 0, 900).unwrap();
    let repeat_render_ms = elapsed(begin);
    drop(repeat);
    let mut sequential_ms = Vec::new();
    for page in 0..document.pages.len().min(sequential_count) {
        let begin = Instant::now();
        drop(engine.render_page(&document.id, page, 900).unwrap());
        sequential_ms.push(elapsed(begin));
    }
    let begin = Instant::now();
    let mut total_characters = 0;
    for page in 0..document.pages.len() {
        total_characters += text(&engine, &document.id, page).chars().count();
    }
    let extract_all_ms = elapsed(begin);
    let pages: Vec<Value> = document.pages.iter().enumerate().rev().map(|(index, page)| {
        let mut overlays = serde_json::to_value(&page.overlays).unwrap().as_array().unwrap().clone();
        if index == document.pages.len() - 1 {
            overlays.extend([
                json!({"type":"comment","id":"corpus-comment","x":40,"y":40,"text":"CORPUS EDIT COMMENT","color":"#ff8000"}),
                json!({"type":"highlight","id":"corpus-highlight","rects":[{"x":40,"y":70,"width":80,"height":12}],"text":"CORPUS EDIT HIGHLIGHT","color":"#00ff00","opacity":0.3}),
                json!({"type":"text","id":"corpus-text","x":40,"y":100,"text":"CORPUS EDIT TEXT","fontSize":12,"color":"#0000ff","rotation":0}),
                json!({"type":"ink","id":"corpus-ink","paths":[[{"x":40,"y":130},{"x":65,"y":145},{"x":90,"y":130}]],"color":"#000000","strokeWidth":2})]);
        }
        json!({"id":format!("page-{index}"),"sourceId":document.id,"pageIndex":index,"width":page.width,"height":page.height,"rotation":0,"overlays":overlays})
    }).collect();
    let request: ExportRequest =
        serde_json::from_value(json!({"pages":pages,"flatten":false})).unwrap();
    let output = temporary.path().join("edited-reordered.pdf");
    let begin = Instant::now();
    engine.export_pdf(request, &output).unwrap();
    let export_ms = elapsed(begin);
    let begin = Instant::now();
    let reopened = engine.open_document(&output).unwrap();
    let reopen_ms = elapsed(begin);
    assert_eq!(reopened.pages.len(), document.pages.len());
    for sample in fixture["samples"].as_array().unwrap() {
        let original = sample["page"].as_u64().unwrap() as usize;
        let destination = document.pages.len() - 1 - original;
        assert!(
            text(&engine, &reopened.id, destination).contains(sample["text"].as_str().unwrap()),
            "{filename}: reorder/source text loss"
        );
        assert!((reopened.pages[destination].width - document.pages[original].width).abs() < 0.1);
        assert!((reopened.pages[destination].height - document.pages[original].height).abs() < 0.1);
    }
    for probe in fixture["textProbes"].as_array().unwrap() {
        assert!(
            text(&engine, &reopened.id, document.pages.len() - 1).contains(probe.as_str().unwrap()),
            "embedded Unicode survives export/reorder"
        );
    }
    let overlays = serde_json::to_value(&reopened.pages[0].overlays).unwrap();
    let comment = overlays
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["text"] == "CORPUS EDIT COMMENT")
        .expect("editable comment survives reopen");
    assert!((comment["x"].as_f64().unwrap() - 40.0).abs() < 1.0);
    assert!((comment["y"].as_f64().unwrap() - 40.0).abs() < 1.0);
    let highlight = overlays
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["text"] == "CORPUS EDIT HIGHLIGHT")
        .expect("highlight survives reopen");
    for (key, expected) in [("x", 40.0), ("y", 70.0), ("width", 80.0), ("height", 12.0)] {
        assert!((highlight["rects"][0][key].as_f64().unwrap() - expected).abs() < 0.1);
    }
    let editable_text = overlays
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["type"] == "text" && o["text"] == "CORPUS EDIT TEXT")
        .expect("inserted text remains editable after reopen");
    assert_eq!(editable_text["x"], 40.0);
    assert_eq!(editable_text["y"], 100.0);
    assert_eq!(editable_text["fontSize"], 12.0);
    assert_eq!(editable_text["color"], "#0000ff");
    let editable_ink = overlays
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["type"] == "ink")
        .expect("inserted ink remains editable after reopen");
    assert_eq!(
        editable_ink["paths"],
        json!([[{"x":40.0,"y":130.0},{"x":65.0,"y":145.0},{"x":90.0,"y":130.0}]])
    );
    assert_eq!(editable_ink["strokeWidth"], 2.0);
    for (original_index, page) in document.pages.iter().enumerate() {
        let originals = serde_json::to_value(&page.overlays).unwrap();
        let restored = serde_json::to_value(
            &reopened.pages[document.pages.len() - 1 - original_index].overlays,
        )
        .unwrap();
        for original in originals.as_array().unwrap() {
            let found = restored
                .as_array()
                .unwrap()
                .iter()
                .find(|o| o["type"] == original["type"] && o["text"] == original["text"])
                .expect("source annotation survives reverse/edit/export");
            assert_eq!(found["color"], original["color"]);
            if original["type"] == "highlight" {
                assert_eq!(found["rects"], original["rects"]);
                assert!(
                    (found["opacity"].as_f64().unwrap() - original["opacity"].as_f64().unwrap())
                        .abs()
                        < 0.01
                );
            }
        }
    }
    drop(engine.render_page(&reopened.id, 0, 900).unwrap());
    engine.close_document(&reopened.id).unwrap();
    engine.close_document(&document.id).unwrap();
    assert_eq!(
        fingerprint,
        source_fingerprint(&source),
        "source mutated: {filename}"
    );
    json!({"fixture":filename,"sourceBytes":fs::metadata(source).unwrap().len(),"pages":document.pages.len(),"processId":std::process::id(),"openMs":open_ms,"samples":samples,"repeatFirstPageRenderMs":repeat_render_ms,"sequentialRenderMs":sequential_ms,"extractAllMs":extract_all_ms,"totalCharacters":total_characters,"exportReverseWithEditsMs":export_ms,"exportBytes":fs::metadata(output).unwrap().len(),"reopenMs":reopen_ms,"memory":sampler.finish(),"passed":true})
}

#[test]
fn small_synthetic_corpus_preserves_sources_geometry_and_edits() {
    let directory = root().join("tests/fixtures/corpus");
    let manifest: Value =
        serde_json::from_slice(&fs::read(directory.join("manifest.json")).unwrap()).unwrap();
    for fixture in manifest["fixtures"].as_array().unwrap() {
        check_fixture(&directory, fixture, 0);
    }
}

#[test]
#[ignore = "Generate --large and run scripts/corpus-benchmark.ps1 for isolated measured workloads"]
fn measured_large_corpus() {
    let directory = std::env::var_os("FOLIO_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root().join("artifacts/corpus"));
    let manifest: Value = serde_json::from_slice(
        &fs::read(directory.join("manifest.json")).expect("run corpus-generate.py --large"),
    )
    .unwrap();
    let selected = std::env::var("FOLIO_CORPUS_FILE")
        .expect("runner must select one fixture per process for meaningful peak memory");
    let fixture = manifest["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["file"] == selected)
        .expect("fixture not in manifest");
    let report = check_fixture(&directory, fixture, 24);
    let output = std::env::var_os("FOLIO_CORPUS_REPORT")
        .map(PathBuf::from)
        .expect("set report output");
    fs::write(output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("{}", serde_json::to_string(&report).unwrap());
}
