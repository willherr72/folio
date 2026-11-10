//! Opt-in measurement of composition on the redistributable large-page corpus.
use folio_engine::{ExportRequest, PdfEngine};
use serde_json::json;
use std::{path::PathBuf, time::Instant};

#[test]
#[ignore = "requires FOLIO_LARGE_EXPORT_INPUT and a fresh FOLIO_LARGE_EXPORT_OUTPUT directory"]
fn measured_large_export() {
    let input = PathBuf::from(std::env::var_os("FOLIO_LARGE_EXPORT_INPUT").unwrap());
    let out = PathBuf::from(std::env::var_os("FOLIO_LARGE_EXPORT_OUTPUT").unwrap());
    std::fs::create_dir(&out).unwrap();
    let engine = PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
    let start = Instant::now();
    let source = engine.open_document(&input).unwrap();
    eprintln!("OPEN_MS {:.3}",start.elapsed().as_secs_f64()*1000.0);
    let n = source.pages.len();
    assert!(n >= 5);
    let mut measurements = vec![];
    for (name, indices) in [("last-five", (n-5..n).collect::<Vec<_>>()), ("all", (0..n).collect())] {
        let pages: Vec<_> = indices.iter().map(|&i| json!({"id":format!("page-{i}"),"sourceId":source.id,"pageIndex":i,"width":source.pages[i].width,"height":source.pages[i].height,"rotation":0,"overlays":source.pages[i].overlays})).collect();
        let request: ExportRequest = serde_json::from_value(json!({"pages":pages,"flatten":false})).unwrap();
        let target = out.join(format!("{name}.pdf"));
        let start = Instant::now();
        engine.export_pdf(request,&target).unwrap();
        let ms=start.elapsed().as_secs_f64()*1000.0;
        eprintln!("EXPORT_TOTAL {name} {ms:.3}");
        measurements.push(json!({"case":name,"pages":indices.len(),"milliseconds":ms,"bytes":std::fs::metadata(target).unwrap().len()}));
    }
    std::fs::write(out.join("timings.json"),serde_json::to_vec_pretty(&measurements).unwrap()).unwrap();
}

#[cfg(windows)]
fn private_memory_bytes() -> usize {
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32, page_faults: u32, peak_working: usize, working: usize,
        peak_paged: usize, paged: usize, peak_nonpaged: usize, nonpaged: usize,
        pagefile: usize, peak_pagefile: usize, private: usize,
    }
    #[link(name = "psapi")]
    unsafe extern "system" {
        fn GetProcessMemoryInfo(process: isize, counters: *mut Counters, size: u32) -> i32;
    }
    let mut counters = Counters { cb: std::mem::size_of::<Counters>() as u32, ..Default::default() };
    assert_ne!(unsafe { GetProcessMemoryInfo(-1, &mut counters, std::mem::size_of::<Counters>() as u32) }, 0);
    counters.private
}

/// Keep one engine/source alive for repeated composition, without retaining
/// reopened outputs. Measure live private memory after each completed operation.
#[cfg(windows)]
#[test]
#[ignore = "requires FOLIO_LARGE_EXPORT_INPUT and a fresh FOLIO_LARGE_EXPORT_OUTPUT directory"]
fn measured_repeated_export() {
    let input = PathBuf::from(std::env::var_os("FOLIO_LARGE_EXPORT_INPUT").unwrap());
    let original = std::fs::read(&input).unwrap();
    let out = PathBuf::from(std::env::var_os("FOLIO_LARGE_EXPORT_OUTPUT").unwrap());
    std::fs::create_dir(&out).unwrap();
    let engine = PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
    let source = engine.open_document(&input).unwrap();
    let after_open = private_memory_bytes();
    let mut measurements = vec![];
    for repeat in 0..16 {
        let pages: Vec<_> = source.pages.iter().enumerate().map(|(i,p)| json!({"id":format!("page-{i}"),"sourceId":source.id,"pageIndex":i,"width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})).collect();
        let request: ExportRequest = serde_json::from_value(json!({"pages":pages,"flatten":false})).unwrap();
        let target = out.join(format!("save-{repeat:02}.pdf"));
        let start = Instant::now();
        engine.export_pdf(request, &target).unwrap();
        measurements.push(json!({"repeat":repeat,"milliseconds":start.elapsed().as_secs_f64()*1000.0,"privateBytesAfterSave":private_memory_bytes(),"bytes":std::fs::metadata(target).unwrap().len()}));
    }
    engine.close_document(&source.id).unwrap();
    assert_eq!(std::fs::read(input).unwrap(), original);
    std::fs::write(out.join("repeated.json"),serde_json::to_vec_pretty(&json!({"afterOpenPrivateBytes":after_open,"afterClosePrivateBytes":private_memory_bytes(),"measurements":measurements})).unwrap()).unwrap();
}
