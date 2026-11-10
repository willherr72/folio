use sha2::Digest;
use super::*;

fn cold_runtime() -> WorkerRuntime {
    let mut rt = runtime();
    rt.source_store = crate::source_storage::SourceStore::with_test_limits(0, 1);
    rt
}

fn make_cold(rt: &mut WorkerRuntime) {
    rt.create_blank_document(200., 200.).unwrap();
    let (reply, receive) = mpsc::channel();
    request(rt, WorkerRequest::Status { reply });
    receive.recv().unwrap();
}

#[test]
fn cold_source_releases_native_and_raw_owners_but_keeps_annotation_authority() {
    let mut rt = cold_runtime();
    let info = opened(&mut rt);
    let bytes = rt.document(&info.id).unwrap().bytes().unwrap();
    let identity = rt.document(&info.id).unwrap().source_bytes.identity().clone();
    let snapshot = rt.document(&info.id).unwrap().imported_overlay_sources.snapshot().unwrap();
    let before = render(&mut rt, &info.id, 1).unwrap();
    make_cold(&mut rt);
    assert_eq!(Arc::strong_count(&bytes), 1, "Both cached bytes and PDFium reader must release the old allocation");
    let resident = rt.source_store.usage().raw_cached_bytes;
    let (reply, receive) = mpsc::channel();
    request(&mut rt, WorkerRequest::SourceUnits { source_id: info.id.clone(), reply });
    assert_eq!(receive.recv().unwrap().unwrap(), vec![1., 1.7]);
    assert_eq!(rt.source_store.usage().raw_cached_bytes, resident, "Metadata must not materialize a cold source");
    let source = rt.document(&info.id).unwrap();
    let restored = source.bytes().unwrap();
    assert_eq!(restored.as_ref(), bytes.as_ref());
    assert!(!Arc::ptr_eq(&restored, &bytes));
    assert!(identity.same_source(source.source_bytes.identity()));
    assert!(source.imported_overlay_sources.bound_to(source.source_bytes.identity()));
    assert_eq!(source.imported_overlay_sources.snapshot().unwrap(), snapshot);
    assert_eq!(render(&mut rt, &info.id, 1).unwrap(), before);
    assert_eq!(rt.document(&info.id).unwrap().pdf().unwrap().pages().get(1).unwrap().annotations().len(), 1);
    rt.close_document(&info.id).unwrap();
    assert_eq!(<[u8;32]>::from(sha2::Sha256::digest(&bytes)), *identity.sha256());
}

#[test]
fn equal_byte_foreign_capture_and_unbound_index_stay_untrusted_after_reload() {
    let mut rt = cold_runtime();
    let first = opened(&mut rt);
    let original = rt.document(&first.id).unwrap().bytes().unwrap();
    let path = std::env::temp_dir().join("same-bytes-foreign.pdf");
    let second = rt.open_bytes(original.to_vec(), path.clone(), path, true, true, false, OpenSecurity::Authenticated(None)).unwrap();
    let foreign = rt.document(&second.id).unwrap().source_bytes.identity().clone();
    let source = rt.document(&first.id).unwrap();
    let own = source.source_bytes.identity().clone();
    assert_eq!(own.sha256(), foreign.sha256());
    assert!(!source.imported_overlay_sources.bound_to(&foreign));
    rt.documents.get_mut(&first.id).unwrap().imported_overlay_sources = imported_overlay_sources::Index::unbound(own.clone());
    make_cold(&mut rt);
    assert_eq!(rt.document(&first.id).unwrap().bytes().unwrap().as_ref(), original.as_ref());
    let source = rt.document(&first.id).unwrap();
    assert!(own.same_source(source.source_bytes.identity()));
    assert!(!source.imported_overlay_sources.bound_to(source.source_bytes.identity()));
    assert!(source.imported_overlay_sources.snapshot().is_none());
}

#[test]
fn cold_backing_failure_is_retryable_without_caching_metadata_failure() {
    let mut rt=cold_runtime();let info=opened(&mut rt);
    let before=render(&mut rt,&info.id,1).unwrap();
    let handle=rt.document(&info.id).unwrap().source_bytes.clone();
    make_cold(&mut rt);
    let saved=handle.take_test_backing();
    let source=rt.documents.get_mut(&info.id).unwrap();
    source.physical_units.take();source.bookmarks.take();source.text_edit_restriction.take();
    for _ in 0..2 {
        let source=rt.document(&info.id).unwrap();
        assert!(matches!(source.bytes(),Err(EngineError::Io(_))));
        assert!(matches!(source.pdf(),Err(EngineError::Io(_))));
        assert!(matches!(source.units(),Err(EngineError::Io(_))));
        assert!(matches!(source.source_bookmarks(),Err(EngineError::Io(_))));
        assert!(matches!(rt.text_page_has_form_objects(&info.id,0),Err(EngineError::Io(_))));
        assert!(source.physical_units.get().is_none());assert!(source.bookmarks.get().is_none());assert!(source.text_edit_restriction.get().is_none());
    }
    handle.restore_test_backing(&saved);
    assert_eq!(rt.document(&info.id).unwrap().units().unwrap(), &[1.,1.7]);
    assert!(rt.document(&info.id).unwrap().source_bookmarks().unwrap().is_ok());
    assert!(!rt.text_page_has_form_objects(&info.id,0).unwrap());
    assert_eq!(render(&mut rt,&info.id,1).unwrap(),before);
}

#[test]
fn failed_spill_capture_keeps_existing_document_and_can_retry() {
    let mut rt=cold_runtime();let original=opened(&mut rt);
    let before=rt.document(&original.id).unwrap().bytes().unwrap();
    let count=rt.documents.len();let sources=rt.source_store.usage().sources;
    let folder=tempfile::tempdir().unwrap();
    rt.source_store.set_test_spill_directory(Some(folder.path().join("does-not-exist")));
    assert!(matches!(rt.create_blank_document(200.,200.),Err(EngineError::Io(_))));
    assert_eq!(rt.documents.len(),count);assert_eq!(rt.source_store.usage().sources,sources);
    assert_eq!(rt.document(&original.id).unwrap().bytes().unwrap(),before);
    rt.source_store.set_test_spill_directory(None);
    rt.create_blank_document(200.,200.).unwrap();
    assert_eq!(rt.documents.len(),count+1);
}

#[test]
fn failed_transfer_backing_does_not_publish_unbound_candidate() {
    for mode in 0..4 {
        let mut rt=cold_runtime();let original=opened(&mut rt);let candidate=opened(&mut rt);
        let candidate_id=candidate.id.clone();
        let original_handle=rt.document(&original.id).unwrap().source_bytes.clone();
        let snapshot=rt.document(&original.id).unwrap().imported_overlay_sources.snapshot().unwrap();
        make_cold(&mut rt);let saved=original_handle.take_test_backing();
        let result=match mode {
            0=>rt.retain_annotation_sources(&original.id,candidate),
            1=>rt.retain_session_annotation_sources(&original.id,candidate,None),
            2=>rt.retain_annotation_sources_after_widget_removal(&original.id,candidate,&[]),
            _=>rt.retain_annotation_sources_with_copy(&original.id,candidate,Some(0)),
        };
        assert!(matches!(result,Err(EngineError::Io(_))),"mode {mode} must surface backing failure, not return an unbound success");
        assert!(!rt.documents.contains_key(&candidate_id));
        assert_eq!(rt.document(&original.id).unwrap().imported_overlay_sources.snapshot().unwrap(),snapshot);
        original_handle.restore_test_backing(&saved);
        assert!(rt.document(&original.id).unwrap().bytes().is_ok());
    }
}
