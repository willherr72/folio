use super::*;
use crate::engine::TextRedactionRect;
use lopdf::{dictionary, Document, Stream};
fn fixture() -> (tempfile::TempDir, PdfEngine, RedactionHost, DocumentInfo) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("source.pdf");
    let mut pdf = Document::with_version("1.7");
    let tree = pdf.new_object_id();
    let font =
        pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let content = pdf.add_object(Stream::new(
        dictionary! {},
        b"BT /F1 12 Tf 20 240 Td (SECRET) Tj 0 -40 Td (Keep this text) Tj ET".to_vec(),
    ));
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),500.into(),300.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
    pdf.objects.insert(
        tree,
        dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into(),
    );
    let catalog = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree});
    pdf.trailer.set("Root", catalog);
    pdf.save(&path).unwrap();
    let engine = PdfEngine::start(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let source = engine.open_document(&path).unwrap();
    let host = RedactionHost::new(engine.clone());
    (root, engine, host, source)
}
fn marks() -> Vec<TextRedactionMark> {
    vec![TextRedactionMark {
        page_index: 0,
        rect: TextRedactionRect {
            x: 19.,
            y: 45.,
            width: 65.,
            height: 20.,
        },
    }]
}
fn text(engine: &PdfEngine, id: &str) -> String {
    engine
        .page_text(id, 0)
        .unwrap()
        .characters
        .iter()
        .map(|c| c.text.as_str())
        .collect()
}

#[test]
fn redaction_host_combined_pair_requires_each_page_and_keeps_adoption_window_owned() {
    let (_root, engine, host, source) = fixture();
    host.register_window("content").unwrap();
    host.register_window("foreign").unwrap();
    let window = host.capture("content").unwrap();
    let other = host.capture("foreign").unwrap();
    let session = window.begin().unwrap();
    let foreign = other.begin().unwrap();
    assert!(window.preview_content(&session, &source.id, &marks()).is_err());
    assert!(other.preview_content(&foreign, &source.id, &marks()).is_err());
    assert!(text(&engine, &source.id).contains("SECRET"));
    let prepared = window
        .prepare(
            &session,
            crate::ExportRequest {
                pages: (0..2)
                    .map(|i| crate::PagePlan {
                        id: format!("page-{i}"),
                        source_id: source.id.clone(),
                        page_index: 0,
                        width: 500.,
                        height: 300.,
                        rotation: 0,
                        crop: None,
                        overlays: vec![],
                    })
                    .collect(),
                flatten: false,
            },
            Some(vec![]),
        )
        .unwrap();
    assert!(other
        .render_content(&foreign, &prepared.document.id, None, 0, 900)
        .is_err());
    assert!(other.preview_content(&foreign, &prepared.document.id, &marks()).is_err());
    assert!(window
        .render_content(&session, &prepared.document.id, None, 0, 900)
        .is_ok());
    let mut selected = marks();
    selected.push(TextRedactionMark {
        page_index: 1,
        rect: marks()[0].rect,
    });
    let preview = window
        .preview_content(&session, &prepared.document.id, &selected)
        .unwrap();
    assert_eq!(preview.removed_characters, 12);
    assert_eq!(preview.pages.len(), 2);
    assert!(other
        .render_content(
            &foreign,
            &prepared.document.id,
            Some(&preview.preview_id),
            0,
            900
        )
        .is_err());
    assert!(window.apply(&session, &preview.preview_id).is_err());
    window
        .render_content(
            &session,
            &prepared.document.id,
            Some(&preview.preview_id),
            0,
            900,
        )
        .unwrap();
    assert!(window.apply(&session, &preview.preview_id).is_err());
    window
        .render_content(
            &session,
            &prepared.document.id,
            Some(&preview.preview_id),
            1,
            900,
        )
        .unwrap();
    let applied = window.apply(&session, &preview.preview_id).unwrap();
    window.release(&session).unwrap();
    assert_eq!(applied.id, preview.candidate.id);
    assert!(!text(&engine, &applied.id).contains("SECRET"));
    assert!(text(&engine, &source.id).contains("SECRET"));
    host.detach("content").unwrap().cleanup();
    assert!(engine.page_text(&applied.id, 0).is_err());
    assert!(window
        .render_content(
            &session,
            &prepared.document.id,
            Some(&preview.preview_id),
            0,
            900
        )
        .is_err());
    host.detach("foreign").unwrap().cleanup();
}

#[test]
fn redaction_host_sessions_isolate_previews_and_release_keeps_original() {
    let (root, engine, host, source) = fixture();
    let before = std::fs::read(root.path().join("source.pdf")).unwrap();
    assert!(
        host.register_window("main").is_ok(),
        "window registration must be supported"
    );
    host.register_window("other").unwrap();
    let main = host.capture("main").unwrap();
    let other = host.capture("other").unwrap();
    let session = main.begin().unwrap();
    let sibling = main.begin().unwrap();
    let foreign = other.begin().unwrap();
    let preview = main.preview(&session, &source.id, &marks()).unwrap();
    assert_eq!(preview.removed_characters, 6);
    assert!(!text(&engine, &preview.candidate.id).contains("SECRET"));
    assert!(text(&engine, &preview.candidate.id).contains("Keep this text"));
    assert!(main.apply(&sibling, &preview.preview_id).is_err());
    assert!(other.apply(&session, &preview.preview_id).is_err());
    assert!(other.discard(&foreign, &preview.preview_id).is_err());
    assert!(other.release(&session).is_err());
    assert_eq!(
        main.bounds(&session, &preview.preview_id, 0, 0, 100)
            .unwrap()
            .total,
        6
    );
    main.release(&session).unwrap();
    assert!(engine.page_text(&preview.candidate.id, 0).is_err());
    assert!(main.preview(&session, &source.id, &marks()).is_err());
    assert!(text(&engine, &source.id).contains("SECRET"));
    assert_eq!(
        before,
        std::fs::read(root.path().join("source.pdf")).unwrap()
    );
    host.detach("main").unwrap().cleanup();
    host.detach("other").unwrap().cleanup();
}
#[test]
fn redaction_host_applied_results_survive_dialog_release_and_die_with_window() {
    let (_root, engine, host, source) = fixture();
    assert!(
        host.register_window("main").is_ok(),
        "window registration must be supported"
    );
    let window = host.capture("main").unwrap();
    let session = window.begin().unwrap();
    let preview = window.preview(&session, &source.id, &marks()).unwrap();
    let applied = window.apply(&session, &preview.preview_id).unwrap();
    assert_eq!(applied.id, preview.candidate.id);
    assert!(window.apply(&session, &preview.preview_id).is_err());
    window.release(&session).unwrap();
    assert!(text(&engine, &applied.id).contains("Keep this text"));
    host.detach("main").unwrap().cleanup();
    assert!(engine.page_text(&applied.id, 0).is_err());
    assert!(text(&engine, &source.id).contains("SECRET"));
    assert!(window.begin().is_err());
    assert!(host.capture("main").is_err());
}

#[test]
fn redaction_host_generations_and_queued_work_cannot_resurrect_a_closed_window() {
    let (_root, engine, host, source) = fixture();
    host.register_window("main").unwrap();
    let old = host.capture("main").unwrap();
    let session = old.begin().unwrap();
    assert!(host.register_window("main").is_err());
    // Hold the operation lock so this invocation is definitely queued at detach.
    let held = old.window.data.lock().unwrap();
    let queued = old.clone();
    let id = source.id.clone();
    let task = std::thread::spawn(move || queued.preview(&session, &id, &marks()));
    let retired = host.detach("main").unwrap();
    assert!(host.capture("main").is_err());
    host.register_window("main").unwrap();
    let fresh = host.capture("main").unwrap();
    drop(held);
    assert!(task.join().unwrap().is_err());
    retired.cleanup();
    retired.cleanup();
    assert!(old.begin().is_err());
    let current = fresh.begin().unwrap();
    let p = fresh.preview(&current, &source.id, &marks()).unwrap();
    assert!(engine.page_text(&p.candidate.id, 0).is_ok());
    retired.cleanup(); // Must not touch same-label replacement generation.
    assert_eq!(
        fresh
            .bounds(&current, &p.preview_id, 0, 0, 100)
            .unwrap()
            .total,
        6
    );
    host.detach("main").unwrap().cleanup();
    assert!(engine.page_text(&source.id, 0).is_ok());
}

#[test]
fn redaction_host_destruction_during_work_cleans_pending_and_adopted_results() {
    for apply in [false, true] {
        let (_root, engine, host, source) = fixture();
        host.register_window("main").unwrap();
        let window = host.capture("main").unwrap();
        let session = window.begin().unwrap();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (go_tx, go_rx) = std::sync::mpsc::channel();
        let worker = window.clone();
        let id = source.id.clone();
        // Exercise the shared host admission/completion boundary with real native
        // resources and a deterministic pause before the result can be delivered.
        let task = std::thread::spawn(move || {
            worker.run(|data| {
                let p = worker
                    .engine
                    .preview_text_redaction(data.owner(&session)?, &id, &marks())
                    .map_err(|e| e.to_string())?;
                if apply {
                    let doc = worker
                        .engine
                        .apply_text_redaction_preview(data.owner(&session)?, &p.preview_id)
                        .map_err(|e| e.to_string())?;
                    data.adopted.insert(doc.id);
                }
                ready_tx.send(p.candidate.id.clone()).unwrap();
                go_rx.recv().unwrap();
                Ok(p)
            })
        });
        let candidate = ready_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        let retired = host.detach("main").unwrap(); // Must not block on work's mutex.
        go_tx.send(()).unwrap();
        assert!(task.join().unwrap().is_err());
        assert!(engine.page_text(&candidate, 0).is_err());
        retired.cleanup();
        assert!(engine.page_text(&source.id, 0).is_ok());
        // Every native owner slot was returned, even after a result was adopted.
        let owners = (0..16)
            .map(|_| engine.begin_text_redaction_session().unwrap())
            .collect::<Vec<_>>();
        for owner in owners {
            engine.release_text_redaction_owner(&owner).unwrap();
        }
    }
}

#[test]
fn redaction_host_concurrent_apply_is_one_shot_and_close_releases_bookkeeping() {
    let (_root, engine, host, source) = fixture();
    host.register_window("main").unwrap();
    let window = host.capture("main").unwrap();
    let session = window.begin().unwrap();
    let p = window.preview(&session, &source.id, &marks()).unwrap();
    let a = window.clone();
    let b = window.clone();
    let sa = session.clone();
    let sb = session.clone();
    let pa = p.preview_id.clone();
    let pb = p.preview_id.clone();
    let first = std::thread::spawn(move || a.apply(&sa, &pa));
    let second = std::thread::spawn(move || b.apply(&sb, &pb));
    let results = [first.join().unwrap(), second.join().unwrap()];
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(window.window.data.lock().unwrap().adopted.len(), 1);
    host.close_document(&p.candidate.id).unwrap();
    assert!(window.window.data.lock().unwrap().adopted.is_empty());
    assert!(host.close_document(&p.candidate.id).is_err());
    assert!(engine.page_text(&source.id, 0).is_ok());
    host.detach("main").unwrap().cleanup();
}

#[test]
fn redaction_host_capacity_refuses_before_consuming_previews_and_reuses_released_slots() {
    let (_root, _engine, host, source) = fixture();
    host.register_window("main").unwrap();
    let window = host.capture("main").unwrap();
    let sessions = (0..16).map(|_| window.begin().unwrap()).collect::<Vec<_>>();
    assert!(window.begin().is_err());
    window.release(&sessions[1]).unwrap();
    let replacement = window.begin().unwrap();
    assert_ne!(replacement, sessions[1]);
    let data = window.window.data.lock().unwrap();
    assert!(data
        .sessions
        .iter()
        .all(|(session, owner)| session != owner));
    drop(data);
    let p = window.preview(&sessions[0], &source.id, &marks()).unwrap();
    // Fill only the host's ID accounting, without allocating thousands of PDFs.
    window.window.data.lock().unwrap().adopted =
        (0..4096).map(|i| format!("closed-test-{i}")).collect();
    assert!(window.apply(&sessions[0], &p.preview_id).is_err());
    assert_eq!(
        window
            .bounds(&sessions[0], &p.preview_id, 0, 0, 100)
            .unwrap()
            .total,
        6
    );
    window.window.data.lock().unwrap().adopted.clear();
    window.apply(&sessions[0], &p.preview_id).unwrap();
    host.detach("main").unwrap().cleanup();
}

#[test]
fn redaction_host_invalid_marks_and_bounds_do_not_consume_valid_work() {
    let (_root, engine, host, source) = fixture();
    host.register_window("main").unwrap();
    let window = host.capture("main").unwrap();
    let session = window.begin().unwrap();
    for bad in [
        vec![],
        vec![marks()[0].clone(); 65],
        vec![TextRedactionMark {
            page_index: 0,
            rect: TextRedactionRect {
                x: f64::NAN,
                y: 1.,
                width: 5.,
                height: 5.,
            },
        }],
    ] {
        assert!(window.preview(&session, &source.id, &bad).is_err());
    }
    let p = window.preview(&session, &source.id, &marks()).unwrap();
    for (page, offset, limit) in [(0, 0, 0), (0, 0, 1001), (1, 0, 1), (0, 7, 1)] {
        assert!(window
            .bounds(&session, &p.preview_id, page, offset, limit)
            .is_err());
    }
    assert_eq!(
        window
            .bounds(&session, &p.preview_id, 0, 6, 1)
            .unwrap()
            .boxes
            .len(),
        0
    );
    window.discard(&session, &p.preview_id).unwrap();
    window.discard(&session, &p.preview_id).unwrap();
    assert!(engine.page_text(&p.candidate.id, 0).is_err());
    let p = window.preview(&session, &source.id, &marks()).unwrap();
    host.close_document(&source.id).unwrap();
    assert!(window.apply(&session, &p.preview_id).is_err());
    assert!(engine.page_text(&p.candidate.id, 0).is_err());
    host.detach("main").unwrap().cleanup();
}

#[test]
fn redaction_host_wire_types_match_camel_case_and_reject_extra_mark_fields() {
    use serde_json::{from_value, json, to_value};
    let (_root, _engine, host, source) = fixture();
    host.register_window("main").unwrap();
    let window = host.capture("main").unwrap();
    let session = window.begin().unwrap();
    let mark = json!({"pageIndex":0,"rect":{"x":19,"y":45,"width":65,"height":20}});
    let parsed: TextRedactionMark = from_value(mark.clone()).unwrap();
    // JSON input accepts integral coordinates; the native f64 output is real.
    assert_eq!(
        to_value(parsed).unwrap(),
        json!({"pageIndex":0,"rect":{"x":19.0,"y":45.0,"width":65.0,"height":20.0}})
    );
    for invalid in [
        json!({"pageIndex":0,"owner":"forged","rect":mark["rect"]}),
        json!({"page_index":0,"rect":mark["rect"]}),
        json!({"pageIndex":0,"rect":{"x":1,"y":1,"width":5,"height":5,"owner":"forged"}}),
    ] {
        assert!(from_value::<TextRedactionMark>(invalid).is_err());
    }
    let p = window.preview(&session, &source.id, &marks()).unwrap();
    let encoded = to_value(&p).unwrap();
    let expected = [
        "previewId",
        "sourceId",
        "candidate",
        "removedCharacters",
        "affectedBlocks",
        "omittedMetadata",
        "expiresInSeconds",
        "pages",
    ]
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        encoded
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        expected
    );
    assert_eq!(encoded["pages"][0]["selectedGlyphCount"], 6);
    let bounds = to_value(window.bounds(&session, &p.preview_id, 0, 0, 2).unwrap()).unwrap();
    assert_eq!(bounds["pageIndex"], 0);
    assert_eq!(bounds["total"], 6);
    assert_eq!(bounds["boxes"].as_array().unwrap().len(), 2);
    let owner = window.window.data.lock().unwrap().sessions[&session].clone();
    assert!(!encoded.to_string().contains(&owner));
    host.detach("main").unwrap().cleanup();
}

#[test]
fn redaction_host_apply_racing_explicit_close_does_not_retain_dead_result_ids() {
    let (_root, engine, host, source) = fixture();
    host.register_window("main").unwrap();
    let window = host.capture("main").unwrap();
    let session = window.begin().unwrap();
    for _ in 0..8 {
        let p = window.preview(&session, &source.id, &marks()).unwrap();
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let worker = window.clone();
        let s = session.clone();
        let preview = p.preview_id.clone();
        let a = barrier.clone();
        let apply = std::thread::spawn(move || {
            a.wait();
            worker.apply(&s, &preview)
        });
        let closer = host.clone();
        let candidate = p.candidate.id.clone();
        let b = barrier.clone();
        let close = std::thread::spawn(move || {
            b.wait();
            closer.close_document(&candidate)
        });
        barrier.wait();
        let _ = apply.join().unwrap();
        close.join().unwrap().unwrap();
        assert!(engine.page_text(&p.candidate.id, 0).is_err());
        assert!(window.window.data.lock().unwrap().adopted.is_empty());
    }
    assert!(engine.page_text(&source.id, 0).is_ok());
    host.detach("main").unwrap().cleanup();
}

#[test]
fn redaction_host_preparation_uses_session_ownership_and_window_cleanup() {
    let (_root, engine, host, source) = fixture();
    host.register_window("main").unwrap();
    host.register_window("other").unwrap();
    let window = host.capture("main").unwrap();
    let other = host.capture("other").unwrap();
    let session = window.begin().unwrap();
    let request = crate::ExportRequest {
        flatten: false,
        pages: vec![crate::PagePlan {
            id: "page".into(),
            source_id: source.id.clone(),
            page_index: 0,
            width: 500.,
            height: 300.,
            rotation: 0,
            crop: None,
            overlays: vec![],
        }],
    };
    assert!(other.prepare(&session, request.clone(), None).is_err());
    let prepared = window.prepare(&session, request, None).unwrap();
    assert!(text(&engine, &prepared.document.id).contains("SECRET"));
    let preview = window
        .preview(&session, &prepared.document.id, &marks())
        .unwrap();
    host.detach("main").unwrap().cleanup();
    assert!(engine.page_text(&prepared.document.id, 0).is_err());
    assert!(engine.page_text(&preview.candidate.id, 0).is_err());
    assert!(engine.page_text(&source.id, 0).is_ok());
    host.detach("other").unwrap().cleanup();
}
