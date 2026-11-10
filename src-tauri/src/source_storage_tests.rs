use super::*;
use std::sync::Arc;

#[test]
fn default_hot_cache_releases_decoded_bytes_without_losing_history_sources() {
    const MIB: usize = 1024 * 1024;
    let store = SourceStore::new();
    let mut sources = Vec::new();
    for marker in 1..=20u8 {
        sources.push(store.capture(vec![marker; 4 * MIB].into()).unwrap());
        store.trim_raw_cache(|_| {});
    }
    let usage = store.usage();
    assert_eq!(usage.sources, 20, "hot-cache eviction must retain every source identity");
    assert!(usage.raw_cached_bytes <= 64 * MIB, "decoded hot source bytes: {}", usage.raw_cached_bytes);
    assert!(sources[0].0.cache.borrow().is_none(), "oldest retained source must exercise cold reconstruction");
    for (index, source) in sources.iter().enumerate() {
        let identity = source.identity().clone();
        let bytes = source.lease().unwrap();
        assert_eq!(bytes.len(), 4 * MIB);
        assert!(bytes.iter().all(|byte| *byte == (index + 1) as u8));
        assert!(identity.same_source(source.identity()));
        assert_eq!(digest(&bytes), *identity.sha256());
        drop(bytes);
        store.trim_raw_cache(|_| {});
        assert!(store.usage().raw_cached_bytes <= 64 * MIB);
    }
    drop(sources);
    assert_eq!(store.usage().sources, 0);
    assert_eq!(store.usage().raw_cached_bytes, 0);
    assert_eq!(store.usage().encoded_resident_bytes, 0);
}

fn noise(len: usize) -> Arc<[u8]> {
    let mut seed = 0x913af71c_u32;
    (0..len).map(|_| { seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5; seed as u8 }).collect::<Vec<_>>().into()
}

#[test]
fn compressed_and_raw_sources_reconstruct_without_expansion() {
    let store = SourceStore::with_limits(4 * 1024 * 1024, 0);
    let text: Arc<[u8]> = vec![b'x'; 2 * 1024 * 1024 + 17].into();
    let first = store.capture(text.clone()).unwrap();
    let random = noise(100_000);
    let second = store.capture(random.clone()).unwrap();
    assert!(store.usage().encoded_resident_bytes < text.len() + random.len());
    first.0.cache.borrow_mut().take();
    second.0.cache.borrow_mut().take();
    assert_eq!(first.lease().unwrap().as_ref(), text.as_ref());
    assert_eq!(second.lease().unwrap().as_ref(), random.as_ref());
    assert!(second.0.blocks.iter().all(|block| block.codec == Codec::Raw));
}

#[test]
fn equal_bytes_are_distinct_sources_but_clones_keep_identity() {
    let store = SourceStore::new();
    let bytes: Arc<[u8]> = b"same source bytes".to_vec().into();
    let a = store.capture(bytes.clone()).unwrap();
    let b = store.capture(bytes).unwrap();
    assert!(!a.identity().same_source(b.identity()));
    assert!(a.identity().same_source(a.clone().identity()));
    assert_eq!(a.identity().sha256(), b.identity().sha256());
    assert_eq!(a.identity().len(), 17);
}

#[test]
fn spill_reconstructs_exact_bytes_and_never_writes_plaintext() {
    use std::io::{Read, Seek, SeekFrom};
    let store = SourceStore::with_limits(0, 0);
    let bytes: Arc<[u8]> = b"PRIVATE SOURCE SENTINEL 95b3f8".repeat(1000).into();
    let source = store.capture(bytes.clone()).unwrap();
    assert_eq!(store.usage().encoded_resident_bytes, 0);
    assert_eq!(store.usage().spilled_sources, 1);
    let mut file = source.0.file.as_ref().unwrap().borrow_mut();
    file.seek(SeekFrom::Start(0)).unwrap();
    let mut stored = vec![]; file.read_to_end(&mut stored).unwrap();
    assert!(!stored.windows(24).any(|w| w == &bytes[..24]));
    drop(file);
    source.0.cache.borrow_mut().take();
    assert_eq!(source.lease().unwrap().as_ref(), bytes.as_ref());
}

#[test]
fn eviction_precedes_drop_and_external_lease_survives_close() {
    let store = SourceStore::with_limits(1024 * 1024, 100);
    let a = store.capture(vec![1; 80].into()).unwrap();
    let lease = a.lease().unwrap();
    let b = store.capture(vec![2; 80].into()).unwrap();
    let mut victims = vec![];
    store.trim_raw_cache(|identity| {
        assert!(a.0.cache.borrow().is_some());
        victims.push(identity.clone());
    });
    assert_eq!(victims.len(), 1);
    assert!(victims[0].same_source(a.identity()));
    assert!(a.0.cache.borrow().is_none());
    assert_eq!(store.usage().raw_cached_bytes, 80);
    drop(a); drop(b);
    assert_eq!(store.usage().sources, 0);
    assert_eq!(lease.as_ref(), &[1; 80]);
}

#[test]
fn only_most_recent_source_can_exceed_raw_budget() {
    let store = SourceStore::with_limits(1024, 20);
    let a = store.capture(vec![1; 50].into()).unwrap();
    let b = store.capture(vec![2; 80].into()).unwrap();
    store.trim_raw_cache(|_| {});
    assert!(a.0.cache.borrow().is_none());
    assert_eq!(store.usage().raw_cached_bytes, 80);
    a.lease().unwrap();
    store.trim_raw_cache(|_| {});
    assert!(b.0.cache.borrow().is_none());
    assert_eq!(store.usage().raw_cached_bytes, 50);
}

#[test]
fn resident_encoded_budget_is_aggregate_and_reclaimed_on_close() {
    let store = SourceStore::with_limits(100, 1024);
    let a = store.capture(noise(80)).unwrap();
    let b = store.capture(noise(80)).unwrap();
    assert_eq!(store.usage().encoded_resident_bytes, 80);
    assert_eq!(store.usage().spilled_sources, 1);
    drop(a); drop(b);
    assert_eq!(store.usage().sources, 0);
    assert_eq!(store.usage().encoded_resident_bytes, 0);
    assert_eq!(store.capture(noise(80)).unwrap().0.file.is_none(), true);
}

#[test]
fn crossing_memory_budget_spills_preceding_blocks_too() {
    let store = SourceStore::with_limits(BLOCK_SIZE, 0);
    let bytes = noise(2 * BLOCK_SIZE + 17);
    let source = store.capture(bytes.clone()).unwrap();
    assert_eq!(store.usage().encoded_resident_bytes, 0);
    assert_eq!(source.0.blocks.len(), 3);
    assert!(source.0.blocks.iter().all(|block| matches!(block.payload, Payload::Spill { .. })));
    source.0.cache.borrow_mut().take();
    assert_eq!(source.lease().unwrap().as_ref(), bytes.as_ref());
}

#[test]
fn empty_and_oversized_lengths_are_refused() {
    assert!(SourceStore::new().capture(Arc::from([])).is_err());
    assert!(validate_length(512 * 1024 * 1024).is_ok());
    assert!(validate_length(512 * 1024 * 1024 + 1).is_err());
}

#[test]
fn truncated_zlib_trailer_is_not_an_exact_stored_block() {
    use std::io::Write;
    let raw = vec![b'a'; 4096];
    let mut encoder = flate2::write::ZlibEncoder::new(vec![], flate2::Compression::fast());
    encoder.write_all(&raw).unwrap();
    let encoded = encoder.finish().unwrap();
    for cut in 1..=4 {
        let payload = &encoded[..encoded.len()-cut];
        let block = Block { decoded_len: raw.len(), hash: digest(&raw), codec: Codec::Zlib,
                            payload_len: payload.len(), payload: Payload::Memory(Box::new([])) };
        assert!(decode_payload(&block, payload).is_err(), "accepted missing {cut} checksum bytes");
    }
}

#[test]
fn malformed_decoding_is_bounded_and_rejects_trailing_data() {
    use std::io::Write;
    let raw = vec![1; BLOCK_SIZE];
    let mut encoder = flate2::write::ZlibEncoder::new(vec![], flate2::Compression::fast());
    encoder.write_all(&raw).unwrap();
    let mut encoded = encoder.finish().unwrap();
    let mut block = Block { decoded_len: 10, hash: digest(&raw[..10]), codec: Codec::Zlib,
                            payload_len: encoded.len(), payload: Payload::Memory(Box::new([])) };
    assert!(decode_payload(&block, &encoded).is_err());
    block.decoded_len = BLOCK_SIZE + 1;
    assert!(decode_payload(&block, &encoded).is_err());
    block.decoded_len = raw.len(); block.hash = digest(&raw);
    encoded.push(0); block.payload_len = encoded.len();
    assert!(decode_payload(&block, &encoded).is_err());
}

#[test]
fn failed_spill_capture_leaves_existing_source_and_retry_usable() {
    let directory = tempfile::tempdir().unwrap();
    let mut store = SourceStore::with_limits(100, 0);
    let existing = store.capture(noise(80)).unwrap();
    store.spill_directory = Some(directory.path().join("missing-directory"));
    assert!(store.capture(noise(80)).is_err());
    assert_eq!(store.usage().sources, 1);
    assert_eq!(existing.lease().unwrap().as_ref(), noise(80).as_ref());
    store.spill_directory = None;
    let retry = store.capture(noise(80)).unwrap();
    assert_eq!(retry.lease().unwrap().as_ref(), existing.lease().unwrap().as_ref());
    assert!(!retry.identity().same_source(existing.identity()));
}

#[test]
fn corrupt_spill_does_not_install_cache_and_can_retry_after_restoration() {
    use std::io::{Read, Seek, SeekFrom, Write};
    let store = SourceStore::with_limits(0, 0);
    let original = noise(100_000);
    let source = store.capture(original.clone()).unwrap();
    source.0.cache.borrow_mut().take();
    let mut file = source.0.file.as_ref().unwrap().borrow_mut();
    file.seek(SeekFrom::Start(0)).unwrap();
    let mut stored = vec![]; file.read_to_end(&mut stored).unwrap();
    let mut corrupt = stored.clone(); let last=corrupt.len()-1; corrupt[last] ^= 1;
    file.seek(SeekFrom::Start(0)).unwrap(); file.write_all(&corrupt).unwrap(); drop(file);
    assert!(source.lease().is_err()); assert!(source.0.cache.borrow().is_none());
    let mut file = source.0.file.as_ref().unwrap().borrow_mut();
    file.seek(SeekFrom::Start(0)).unwrap(); file.write_all(&stored).unwrap(); drop(file);
    assert_eq!(source.lease().unwrap().as_ref(), original.as_ref());
    source.0.cache.borrow_mut().take();
    source.0.file.as_ref().unwrap().borrow_mut().set_len(stored.len() as u64 - 1).unwrap();
    assert!(source.lease().is_err()); assert!(source.0.cache.borrow().is_none());
}

#[test]
fn sealed_blocks_bind_purpose_source_ordinal_length_and_codec() {
    use std::io::{Read, Seek, SeekFrom, Write};
    for changed in [None, Some(8), Some(24), Some(28), Some(32)] {
        let store = SourceStore::with_limits(0, 0);
        let source = store.capture(noise(4000)).unwrap();
        source.0.cache.borrow_mut().take();
        let mut file = source.0.file.as_ref().unwrap().borrow_mut();
        file.seek(SeekFrom::Start(0)).unwrap();
        let mut stored = vec![]; file.read_to_end(&mut stored).unwrap();
        let mut frame = private_storage::unseal(Purpose::HistorySourceBlock, &stored, FRAME_HEADER + BLOCK_SIZE).unwrap();
        if let Some(index) = changed { frame[index] ^= 1; }
        let purpose = if changed.is_some() { Purpose::HistorySourceBlock } else { Purpose::Source };
        let replacement = private_storage::seal(purpose, &frame).unwrap();
        assert_eq!(replacement.len(), stored.len());
        file.seek(SeekFrom::Start(0)).unwrap(); file.write_all(&replacement).unwrap(); drop(file);
        let error = source.lease().err().expect("forged block binding accepted");
        if changed.is_some() { assert!(error.to_string().contains("binding differs")); }
        assert!(source.0.cache.borrow().is_none());
    }
}
