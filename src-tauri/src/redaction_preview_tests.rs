use super::*;
use lopdf::{dictionary, Document, Object, Stream};
fn runtime() -> WorkerRuntime {
    WorkerRuntime::new(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
        Arc::new(FontRegistry::new()),
    )
    .unwrap()
}
fn source(rt: &mut WorkerRuntime) -> DocumentInfo {
    let mut d = Document::with_version("1.7");
    let tree = d.new_object_id();
    let font=d.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica","Encoding"=>"WinAnsiEncoding"});
    let mut kids = vec![];
    for program in [
        "BT /F1 12 Tf 20 240 Td (Left SECRET Middle HIDDEN Right) Tj 0 -20 Td (Following text) Tj ET BT /F1 12 Tf 2 20 Td (EDGE safe) Tj ET",
        "BT /F1 12 Tf 20 240 Td (Left SECRET Middle HIDDEN Right) Tj ET BT /F1 12 Tf 20 140 Td (Second block) Tj ET",
        "BT /F1 12 Tf 20 240 Td (Entire page stays unchanged) Tj ET",
    ] {
        let content=d.add_object(Stream::new(dictionary!{},program.as_bytes().to_vec()));
        let page=d.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),500.into(),300.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});kids.push(Object::Reference(page));
    }
    d.objects.insert(
        tree,
        Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>kids,"Count"=>3}),
    );
    let root = d.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree});
    d.trailer.set("Root", root);
    let mut bytes = vec![];
    d.save_to(&mut bytes).unwrap();
    rt.register_generated_document(bytes, "preview-source.pdf")
        .unwrap()
}
fn marks(rt: &WorkerRuntime, id: &str) -> Vec<TextRedactionMark> {
    let mut out = vec![];
    for (page, block, start, end) in [(0, 0, 5, 11), (0, 0, 19, 25), (0, 1, 0, 4), (1, 0, 5, 11)] {
        let selected = rt.inspect_redaction_text_page(id, page).unwrap();
        let boxes = selected
            .glyphs()
            .iter()
            .filter(|g| g.block_index == block && (start..end).contains(&g.offset))
            .map(|g| g.bounds)
            .collect::<Vec<_>>();
        let rect = extent(&boxes).unwrap();
        out.push(TextRedactionMark {
            page_index: page,
            rect,
        });
    }
    out.push(out[0].clone());
    out
}
fn prepared(rt: &mut WorkerRuntime) -> (String, DocumentInfo, Vec<TextRedactionMark>) {
    let owner = rt.begin_redaction_session().unwrap();
    let info = source(rt);
    let marks = marks(rt, &info.id);
    (owner, info, marks)
}
#[test]
fn redaction_preview_multi_page_apply_is_once_only_and_preserves_original() {
    let mut rt = runtime();
    let (owner, source, marks) = prepared(&mut rt);
    let bytes = rt.document(&source.id).unwrap().bytes().unwrap();
    let p = rt.preview_redaction(&owner, &source.id, &marks).unwrap();
    assert_eq!(p.source_id, source.id);
    assert_eq!(p.expires_in_seconds, 900);
    assert!(p.pages.iter().all(|p| p.extent.valid()));
    assert_eq!(&*bytes, &*rt.document(&source.id).unwrap().bytes().unwrap());
    assert_eq!(p.removed_characters, 22);
    assert_eq!(p.affected_blocks, 3);
    assert_eq!(p.pages.len(), 2);
    assert_eq!(p.pages[0].selected_glyph_count, 16);
    assert_eq!(p.pages[1].selected_glyph_count, 6);
    assert_eq!(p.pages[0].page_index, 0);
    assert_eq!(p.pages[1].page_index, 1);
    assert_eq!(p.pages[0].width, 500.);
    assert_eq!(p.pages[0].height, 300.);
    assert_eq!(rt.documents.len(), 2);
    let page = rt.redaction_bounds(&owner, &p.preview_id, 0, 1, 2).unwrap();
    assert_eq!(page.boxes.len(), 2);
    assert_eq!(page.total, 16);
    assert_eq!(page.offset, 1);
    assert_eq!(page.page_index, 0);
    assert!(rt.redaction_bounds(&owner, &p.preview_id, 2, 0, 2).is_err());
    assert!(rt
        .redaction_bounds(&owner, &p.preview_id, 0, 17, 2)
        .is_err());
    assert!(rt
        .redaction_bounds(&owner, &p.preview_id, 0, 0, 1001)
        .is_err());
    assert_eq!(
        rt.document(&p.candidate.id).unwrap().original_path,
        rt.document(&source.id).unwrap().original_path
    );
    let saved = rt.apply_redaction(&owner, &p.preview_id).unwrap();
    assert!(rt.apply_redaction(&owner, &p.preview_id).is_err());
    rt.release_redaction_owner(&owner);
    rt.close_document(&source.id).unwrap();
    assert!(rt.document(&saved.id).is_ok());
    assert_eq!(rt.documents.len(), 1);
    let parsed = Document::load_mem(rt.document(&saved.id).unwrap().bytes().unwrap().as_ref()).unwrap();
    let blocks = crate::redaction_ranges::inspect_standard_glyphs(&parsed).unwrap();
    assert_eq!(
        blocks
            .iter()
            .map(|b| b.source.text.as_str())
            .collect::<Vec<_>>(),
        vec![
            "Left  Middle  RightFollowing text",
            " safe",
            "Left  Middle HIDDEN Right",
            "Second block",
            "Entire page stays unchanged"
        ]
    );
    if let Ok(path) = std::env::var("FOLIO_REDACTION_PREVIEW_FIXTURES") {
        let root = PathBuf::from(path);
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("before.pdf"), &*bytes).unwrap();
        std::fs::write(
            root.join("after.pdf"),
            &*rt.document(&saved.id).unwrap().bytes().unwrap(),
        )
        .unwrap();
        let mut bounds = vec![];
        // Source-native boxes are exported as display evidence, not removal authority.
        let original = rt
            .register_generated_document(bytes.to_vec(), "audit-original.pdf")
            .unwrap();
        for page in [0, 1] {
            let selection = rt.inspect_redaction_text_page(&original.id, page).unwrap();
            let rects = marks
                .iter()
                .filter(|m| m.page_index == page)
                .map(|m| m.rect)
                .collect::<Vec<_>>();
            let plan = selection.mark_many(&rects).unwrap();
            for b in plan.affected_glyphs() {
                bounds.push(serde_json::json!({"page":page,"x":b.x,"y":b.y,"width":b.width,"height":b.height}));
            }
        }
        std::fs::write(
            root.join("bounds.json"),
            serde_json::to_vec_pretty(&bounds).unwrap(),
        )
        .unwrap();
    }
}
#[test]
fn redaction_preview_wrong_owner_release_late_requests_and_pending_sources() {
    let mut rt = runtime();
    let (owner, s, m) = prepared(&mut rt);
    let other = rt.begin_redaction_session().unwrap();
    let p = rt.preview_redaction(&owner, &s.id, &m).unwrap();
    let q = rt.preview_redaction(&other, &s.id, &m).unwrap();
    assert!(rt.apply_redaction(&other, &p.preview_id).is_err());
    assert!(rt.discard_redaction(&other, &p.preview_id).is_err());
    assert!(rt.redaction_bounds(&other, &p.preview_id, 0, 0, 1).is_err());
    assert!(rt.preview_redaction(&owner, &p.candidate.id, &m).is_err());
    assert_eq!(rt.documents.len(), 3);
    rt.release_redaction_owner(&owner);
    assert!(rt.document(&p.candidate.id).is_err());
    assert!(rt.document(&q.candidate.id).is_ok());
    assert!(rt.preview_redaction(&owner, &s.id, &m).is_err());
    assert!(rt.apply_redaction(&owner, &p.preview_id).is_err());
    rt.discard_redaction(&other, &q.preview_id).unwrap();
    rt.discard_redaction(&other, &q.preview_id).unwrap();
    assert_eq!(rt.documents.len(), 1);
    rt.release_redaction_owner(&owner);
}
#[test]
fn redaction_preview_closing_source_or_candidate_cleans_only_owned_pending_documents() {
    for unpublished in [false, true] {
        for candidate in [false, true] {
            let mut rt = runtime();
            let (owner, s, m) = prepared(&mut rt);
            let p = rt.preview_redaction(&owner, &s.id, &m).unwrap();
            let id = if candidate { &p.candidate.id } else { &s.id };
            if unpublished {
                rt.discard_unpublished_source(id)
            } else {
                rt.close_document(id).unwrap()
            };
            assert!(rt.redaction_previews.pending.is_empty());
            assert!(rt.document(&p.candidate.id).is_err());
            assert_eq!(rt.documents.len(), usize::from(candidate));
        }
    }
}
#[test]
fn redaction_preview_expiry_and_stale_source_or_candidate_cannot_apply() {
    for mode in 0..5 {
        let mut rt = runtime();
        let (owner, s, m) = prepared(&mut rt);
        let p = rt.preview_redaction(&owner, &s.id, &m).unwrap();
        match mode {
            0 => {
                rt.redaction_previews
                    .pending
                    .get_mut(&p.preview_id)
                    .unwrap()
                    .created = Instant::now() - TTL - Duration::from_secs(1)
            }
            1 => {
                rt.documents.get_mut(&s.id).unwrap().source_bytes = crate::source_storage::SourceStore::new().capture(Arc::from(b"changed".as_slice())).unwrap()
            }
            2 => rt.documents.get_mut(&s.id).unwrap().for_printing = true,
            3 => {
                let old = rt.document(&p.candidate.id).unwrap().bytes().unwrap().to_vec();
                rt.documents.get_mut(&p.candidate.id).unwrap().source_bytes = crate::source_storage::SourceStore::new().capture(old.into()).unwrap();
            }
            _ => {
                rt.documents.get_mut(&p.candidate.id).unwrap().source_bytes =
                    crate::source_storage::SourceStore::new().capture(Arc::from(b"different allocation".as_slice())).unwrap()
            }
        }
        assert!(rt.redaction_bounds(&owner, &p.preview_id, 0, 0, 1).is_err());
        assert!(rt.apply_redaction(&owner, &p.preview_id).is_err());
        rt.release_redaction_owner(&owner);
        // A swapped allocation is unrelated and must never be deleted by cleanup.
        assert_eq!(rt.document(&p.candidate.id).is_ok(), mode >= 3);
    }
}
#[test]
fn redaction_preview_limits_preserve_existing_entries() {
    let mut rt = runtime();
    let (owner, s, m) = prepared(&mut rt);
    for _ in 1..16 {
        rt.begin_redaction_session().unwrap();
    }
    assert!(rt.begin_redaction_session().is_err());
    let mut previews = vec![];
    for _ in 0..4 {
        previews.push(rt.preview_redaction(&owner, &s.id, &m).unwrap());
    }
    assert!(rt.preview_redaction(&owner, &s.id, &m).is_err());
    assert_eq!(rt.documents.len(), 5);
    for p in &previews {
        rt.discard_redaction(&owner, &p.preview_id).unwrap();
    }
    let p = rt.preview_redaction(&owner, &s.id, &m).unwrap();
    let bytes = rt.document(&p.candidate.id).unwrap().source_bytes.identity().len();
    rt.redaction_previews.byte_limit = bytes * 2 - 1;
    assert!(rt.preview_redaction(&owner, &s.id, &m).is_err());
    assert_eq!(rt.documents.len(), 2);
    rt.redaction_previews.byte_limit = bytes * 2;
    let q = rt.preview_redaction(&owner, &s.id, &m).unwrap();
    rt.discard_redaction(&owner, &q.preview_id).unwrap();
    rt.redaction_previews.geometry_limit = 22 * 32 * 2 - 1;
    assert!(rt.preview_redaction(&owner, &s.id, &m).is_err());
    assert_eq!(rt.documents.len(), 2);
    rt.redaction_previews.geometry_limit = 22 * 32 * 2;
    rt.preview_redaction(&owner, &s.id, &m).unwrap();
}
#[test]
fn redaction_preview_dropped_success_replies_release_their_owned_resources() {
    let mut rt = runtime();
    let (tx, rx) = mpsc::channel();
    drop(rx);
    rt.handle_text_redaction(Request::Begin { reply: tx });
    assert!(rt.redaction_previews.owners.is_empty());
    let (owner, s, m) = prepared(&mut rt);
    let (tx, rx) = mpsc::channel();
    drop(rx);
    rt.handle_text_redaction(Request::Preview {
        owner: owner.clone(),
        source: s.id.clone(),
        marks: m.clone(),
        reply: tx,
    });
    assert_eq!(rt.documents.len(), 1);
    assert!(rt.redaction_previews.pending.is_empty());
    let p = rt.preview_redaction(&owner, &s.id, &m).unwrap();
    let (tx, rx) = mpsc::channel();
    drop(rx);
    rt.handle_text_redaction(Request::Apply {
        owner,
        preview: p.preview_id,
        reply: tx,
    });
    assert_eq!(rt.documents.len(), 1);
    assert!(rt.redaction_previews.pending.is_empty());
}

#[test]
fn redaction_preview_rechecks_late_rights_and_cleanup_needs_no_rights() {
    for (edit, copy) in [(false, true), (true, false), (true, true)] {
        let mut rt = runtime();
        let (owner, s, m) = prepared(&mut rt);
        let p = rt.preview_redaction(&owner, &s.id, &m).unwrap();
        let original = rt.document(&s.id).unwrap().bytes().unwrap();
        let change:crate::protection::ProtectionChange=serde_json::from_value(serde_json::json!({"mode":"passwords","userPassword":"test-reader","ownerPassword":"test-owner","permissions":{"print":true,"printHighQuality":true,"copyContent":copy,"editContent":edit,"annotate":false,"fillForms":false,"assemble":true}})).unwrap();
        let encrypted = change.apply(&original).unwrap();
        let unlocked = crate::protection::unlock_pdf(&encrypted, Some("test-reader")).unwrap();
        rt.documents.get_mut(&s.id).unwrap().policy = Some(SourcePolicy::new(
            unlocked.protection.unwrap(),
            &s.pages,
            &Ok(vec![]),
        ));
        assert!(rt.apply_redaction(&owner, &p.preview_id).is_err());
        assert!(rt.document(&p.candidate.id).is_err());
        rt.release_redaction_owner(&owner);
        assert_eq!(rt.documents.len(), 1);
        assert_eq!(&*original, &*rt.document(&s.id).unwrap().bytes().unwrap());
    }
}
#[test]
fn redaction_preview_worker_facade_round_trip_and_invalid_inputs() {
    let (sender, receiver) = mpsc::channel();
    let (ready, info) = mpsc::channel();
    let join = std::thread::spawn(move || {
        let mut rt = runtime();
        let s = source(&mut rt);
        let m = marks(&rt, &s.id);
        ready.send((s, m)).unwrap();
        rt.run(receiver);
        assert!(rt.documents.is_empty());
        assert!(rt.redaction_previews.owners.is_empty());
    });
    let engine = PdfEngine {
        inner: Arc::new(EngineInner {
            sender,
            fonts: Arc::new(FontRegistry::new()),
            images: Arc::new(crate::ImageRegistry::new()),
        }),
    };
    let (s, m) = info.recv().unwrap();
    let owner = engine.begin_text_redaction_session().unwrap();
    assert!(engine.preview_text_redaction(&owner, &s.id, &[]).is_err());
    assert!(engine
        .preview_text_redaction(&owner, &s.id, &vec![m[0].clone(); 65])
        .is_err());
    assert!(engine
        .preview_text_redaction("not an owner", &s.id, &m)
        .is_err());
    let mut bad = m.clone();
    bad[0].rect.x = f64::NAN;
    assert!(engine.preview_text_redaction(&owner, &s.id, &bad).is_err());
    let p = engine.preview_text_redaction(&owner, &s.id, &m).unwrap();
    assert_eq!(
        engine
            .text_redaction_preview_bounds(&owner, &p.preview_id, 1, 0, 1000)
            .unwrap()
            .total,
        6
    );
    engine
        .discard_text_redaction_preview(&owner, &p.preview_id)
        .unwrap();
    let p = engine.preview_text_redaction(&owner, &s.id, &m).unwrap();
    let adopted = engine
        .apply_text_redaction_preview(&owner, &p.preview_id)
        .unwrap();
    engine.release_text_redaction_owner(&owner).unwrap();
    assert!(engine.preview_text_redaction(&owner, &s.id, &m).is_err());
    engine.close_document(&s.id).unwrap();
    engine.close_document(&adopted.id).unwrap();
    drop(engine);
    join.join().unwrap();
}
#[test]
fn redaction_preview_adopted_output_can_be_previewed_again_and_source_close_cleans_all() {
    let mut rt = runtime();
    let (owner, s, m) = prepared(&mut rt);
    let p = rt.preview_redaction(&owner, &s.id, &m).unwrap();
    let adopted = rt.apply_redaction(&owner, &p.preview_id).unwrap();
    let selection = rt.inspect_redaction_text_page(&adopted.id, 2).unwrap();
    let next = vec![TextRedactionMark {
        page_index: 2,
        rect: selection.glyphs()[0].bounds,
    }];
    let q = rt.preview_redaction(&owner, &adopted.id, &next).unwrap();
    let r = rt.preview_redaction(&owner, &adopted.id, &next).unwrap();
    rt.close_document(&adopted.id).unwrap();
    assert!(rt.document(&q.candidate.id).is_err());
    assert!(rt.document(&r.candidate.id).is_err());
    assert!(rt.document(&s.id).is_ok());
    assert_eq!(rt.documents.len(), 1);
}

#[test]
fn redaction_preview_bounded_loader_accepts_modern_incremental_and_refuses_aggregate_input_without_leases(
) {
    let mut rt = runtime();
    let owner = rt.begin_redaction_session().unwrap();
    let original = source(&mut rt);
    let bytes = rt.document(&original.id).unwrap().bytes().unwrap().to_vec();
    let mut parsed = Document::load_mem(&bytes).unwrap();
    let mut modern = vec![];
    parsed.save_modern(&mut modern).unwrap();
    let mut incremental =
        lopdf::IncrementalDocument::create_from(bytes.clone(), Document::load_mem(&bytes).unwrap());
    incremental
        .new_document
        .add_object(Object::string_literal("edit history"));
    let mut updated = vec![];
    incremental.save_to(&mut updated).unwrap();
    for data in [modern, updated] {
        let input = rt
            .register_generated_document(data, "compressed-history.pdf")
            .unwrap();
        let m = marks(&rt, &input.id);
        let preview = rt.preview_redaction(&owner, &input.id, &m).unwrap();
        assert_eq!(preview.removed_characters, 22);
        rt.discard_redaction(&owner, &preview.preview_id).unwrap();
        rt.close_document(&input.id).unwrap();
    }
    // A valid incremental section with many free, unused IDs is cheap on disk
    // but must consume the same xref-entry allowance as retained entries.
    let parsed = Document::load_mem(&bytes).unwrap();
    let root = parsed.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let mut hostile = bytes.clone();
    let offset = hostile.len();
    hostile.extend_from_slice(b"\nxref\n10000 200001\n");
    for _ in 0..200001 {
        hostile.extend_from_slice(b"0000000000 65535 f \n");
    }
    hostile.extend_from_slice(
        format!(
            "trailer\n<< /Size 210001 /Root {} {} R /Prev {} >>\nstartxref\n{offset}\n%%EOF\n",
            root.0, root.1, parsed.xref_start
        )
        .as_bytes(),
    );
    // Offset points at whitespace; correct it for the strict loader/native reader.
    let offset_text = format!("startxref\n{offset}\n");
    let corrected = format!("startxref\n{}\n", offset + 1);
    let pos = hostile
        .windows(offset_text.len())
        .rposition(|w| w == offset_text.as_bytes())
        .unwrap();
    hostile.splice(pos..pos + offset_text.len(), corrected.bytes());
    assert!(Document::load_mem(&hostile).is_ok());
    let source = rt
        .register_generated_document(hostile, "bounded-refusal.pdf")
        .unwrap();
    let before = rt.document(&source.id).unwrap().bytes().unwrap();
    let count = rt.documents.len();
    let m = marks(&rt, &original.id);
    assert!(rt.preview_redaction(&owner, &source.id, &m).is_err());
    assert_eq!(rt.documents.len(), count);
    assert!(rt.redaction_previews.pending.is_empty());
    assert_eq!(&*before, &*rt.document(&source.id).unwrap().bytes().unwrap());
}

#[test]
fn pending_redaction_survives_cold_backing_without_raw_ownership() {
    let mut rt=runtime();rt.source_store=crate::source_storage::SourceStore::with_test_limits(0,1);
    let (owner,source,marks)=prepared(&mut rt);
    let before=rt.document(&source.id).unwrap().bytes().unwrap();
    let preview=rt.preview_redaction(&owner,&source.id,&marks).unwrap();
    let candidate=rt.document(&preview.candidate.id).unwrap().bytes().unwrap();
    rt.create_blank_document(200.,200.).unwrap();rt.trim_native_documents();
    assert_eq!(Arc::strong_count(&before),1);assert_eq!(Arc::strong_count(&candidate),1);
    let cold_bytes=rt.source_store.usage().raw_cached_bytes;
    rt.validate_redaction_preview(&owner,&preview.preview_id).unwrap();
    assert_eq!(rt.source_store.usage().raw_cached_bytes,cold_bytes,"Ownership validation must remain metadata-only");
    let applied=rt.apply_redaction(&owner,&preview.preview_id).unwrap();
    assert_eq!(rt.document(&applied.id).unwrap().bytes().unwrap(),candidate);
    assert_eq!(rt.document(&source.id).unwrap().bytes().unwrap(),before);
}
