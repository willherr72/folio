//! Independent retained-source backing; raw leases are separate from identity.
use crate::private_storage::{self, Purpose};
use flate2::{bufread::ZlibDecoder, write::ZlibEncoder, Compression};
use sha2::{Digest, Sha256};
use std::{cell::{Cell, RefCell}, fs::File, io::{self, Read, Seek, SeekFrom, Write},
          rc::{Rc, Weak}, sync::Arc, time::Instant};

const BLOCK_SIZE: usize = 1024 * 1024;
const MAX_SOURCE: usize = 512 * BLOCK_SIZE;
const FRAME_HEADER: usize = 33;
const FRAME_MAGIC: &[u8; 8] = b"FLSRC001";

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn validate_length(length: usize) -> io::Result<()> {
    if length == 0 || length > MAX_SOURCE { return Err(invalid("Retained PDF source length is invalid")); }
    Ok(())
}
fn digest(bytes: &[u8]) -> [u8; 32] { Sha256::digest(bytes).into() }

struct Identity { length: usize, hash: [u8; 32], nonce: [u8; 16] }
#[derive(Clone)]
pub(crate) struct SourceIdentity(Arc<Identity>);
impl SourceIdentity {
    pub(crate) fn same_source(&self, other: &Self) -> bool { Arc::ptr_eq(&self.0, &other.0) }
    pub(crate) fn len(&self) -> usize { self.0.length }
    pub(crate) fn sha256(&self) -> &[u8; 32] { &self.0.hash }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Codec { Raw = 0, Zlib = 1 }
enum Payload { Memory(Box<[u8]>), Spill { offset: u64, length: u32 } }
struct Block { decoded_len: usize, hash: [u8; 32], codec: Codec, payload_len: usize, payload: Payload }
struct Entry {
    identity: SourceIdentity,
    blocks: Vec<Block>,
    file: Option<RefCell<File>>,
    file_len: u64,
    cache: RefCell<Option<Arc<[u8]>>>,
    used: Cell<Instant>,
}
#[derive(Clone)]
pub(crate) struct SourceHandle(Rc<Entry>);

#[derive(Debug)]
pub(crate) struct StorageUsage {
    pub raw_cached_bytes: usize,
    pub encoded_resident_bytes: usize,
    pub sources: usize,
    pub spilled_sources: usize,
}
pub(crate) struct SourceStore {
    entries: RefCell<Vec<Weak<Entry>>>,
    encoded_limit: usize,
    raw_limit: usize,
    #[cfg(test)]
    spill_directory: Option<std::path::PathBuf>,
}
impl SourceStore {
    // Decoded bytes are hot duplicates; backing retains every history source.
    pub(crate) fn new() -> Self { Self::with_limits(64 * BLOCK_SIZE, 64 * BLOCK_SIZE) }
    #[cfg(test)]
    pub(crate) fn with_test_limits(encoded_limit: usize, raw_limit: usize) -> Self {
        Self::with_limits(encoded_limit, raw_limit)
    }
    #[cfg(test)]
    pub(crate) fn set_test_spill_directory(&mut self, directory: Option<std::path::PathBuf>) { self.spill_directory=directory; }
    #[cfg(test)]
    pub(crate) fn clear_test_raw_cache(&self) {
        // The workload evicts every native reader before forcing a cold lease.
        for entry in self.live() { entry.cache.borrow_mut().take(); }
    }
    #[cfg(test)]
    pub(crate) fn test_spilled_bytes(&self) -> u64 {
        self.live().iter().map(|entry| entry.file_len).sum()
    }
    fn with_limits(encoded_limit: usize, raw_limit: usize) -> Self {
        Self { entries: RefCell::new(vec![]), encoded_limit, raw_limit,
               #[cfg(test)] spill_directory: None }
    }
    fn live(&self) -> Vec<Rc<Entry>> {
        let mut live = vec![];
        self.entries.borrow_mut().retain(|weak| match weak.upgrade() {
            Some(entry) => { live.push(entry); true }, None => false,
        });
        live
    }
    pub(crate) fn usage(&self) -> StorageUsage {
        let entries = self.live();
        StorageUsage {
            sources: entries.len(),
            spilled_sources: entries.iter().filter(|entry| entry.file.is_some()).count(),
            raw_cached_bytes: entries.iter().map(|entry| entry.cache.borrow().as_ref().map_or(0, |bytes| bytes.len())).sum(),
            encoded_resident_bytes: entries.iter().flat_map(|entry| &entry.blocks).map(|block| match &block.payload {
                Payload::Memory(bytes) => bytes.len(), Payload::Spill { .. } => 0,
            }).sum(),
        }
    }
    fn spill_file(&self) -> io::Result<File> {
        #[cfg(test)]
        if let Some(directory) = &self.spill_directory { return tempfile::tempfile_in(directory); }
        tempfile::tempfile()
    }
    pub(crate) fn capture(&self, bytes: Arc<[u8]>) -> io::Result<SourceHandle> {
        validate_length(bytes.len())?;
        let identity = SourceIdentity(Arc::new(Identity { length: bytes.len(), hash: digest(&bytes), nonce: *uuid::Uuid::new_v4().as_bytes() }));
        let available = self.encoded_limit.saturating_sub(self.usage().encoded_resident_bytes);
        let mut blocks: Vec<Block> = Vec::with_capacity(bytes.len().div_ceil(BLOCK_SIZE));
        let mut resident = 0usize;
        let mut file: Option<File> = None;
        for raw in bytes.chunks(BLOCK_SIZE) {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
            encoder.write_all(raw)?;
            let encoded = encoder.finish()?;
            let (codec, payload) = if encoded.len().checked_add(256).is_some_and(|n| n < raw.len()) {
                (Codec::Zlib, encoded.into_boxed_slice())
            } else { (Codec::Raw, raw.to_vec().into_boxed_slice()) };
            if file.is_none() && payload.len() > available.saturating_sub(resident) {
                let mut spill = self.spill_file()?;
                for (index, block) in blocks.iter_mut().enumerate() {
                    let Payload::Memory(prior) = &block.payload else { return Err(invalid("Retained source backing is inconsistent")); };
                    block.payload = spill_block(&mut spill, &identity, index, block, prior)?;
                }
                file = Some(spill);
            }
            let mut block = Block { decoded_len: raw.len(), hash: digest(raw), codec,
                                    payload_len: payload.len(), payload: Payload::Memory(Box::new([])) };
            block.payload = if let Some(spill) = &mut file {
                spill_block(spill, &identity, blocks.len(), &block, &payload)?
            } else { resident += payload.len(); Payload::Memory(payload) };
            blocks.push(block);
        }
        let file_len = if let Some(spill) = &mut file { spill.flush()?; spill.metadata()?.len() } else { 0 };
        let entry = Rc::new(Entry { identity, blocks, file: file.map(RefCell::new), file_len,
                                   cache: RefCell::new(Some(bytes)), used: Cell::new(Instant::now()) });
        self.entries.borrow_mut().push(Rc::downgrade(&entry));
        Ok(SourceHandle(entry))
    }
    pub(crate) fn trim_raw_cache(&self, mut before_evict: impl FnMut(&SourceIdentity)) {
        let mut entries = self.live().into_iter().filter(|entry| entry.cache.borrow().is_some()).collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.used.get());
        let mut total: usize = entries.iter().map(|entry| entry.identity.len()).sum();
        let victims = entries.len().saturating_sub(1);
        for entry in entries.into_iter().take(victims) {
            if total <= self.raw_limit { break; }
            before_evict(&entry.identity);
            // No RefCell borrow survives the native eviction callback.
            entry.cache.borrow_mut().take();
            total -= entry.identity.len();
        }
    }
}

fn spill_block(file: &mut File, identity: &SourceIdentity, index: usize, block: &Block, payload: &[u8]) -> io::Result<Payload> {
    let mut frame = Vec::with_capacity(FRAME_HEADER + payload.len());
    frame.extend_from_slice(FRAME_MAGIC);
    frame.extend_from_slice(&identity.0.nonce);
    frame.extend_from_slice(&(index as u32).to_le_bytes());
    frame.extend_from_slice(&(block.decoded_len as u32).to_le_bytes());
    frame.push(block.codec as u8);
    frame.extend_from_slice(payload);
    let encrypted = private_storage::seal(Purpose::HistorySourceBlock, &frame).map_err(io::Error::other)?;
    let offset = file.stream_position()?;
    file.write_all(&encrypted)?;
    Ok(Payload::Spill { offset, length: u32::try_from(encrypted.len()).map_err(|_| invalid("Retained block is too large"))? })
}

fn decode_payload(block: &Block, payload: &[u8]) -> io::Result<Vec<u8>> {
    if block.decoded_len == 0 || block.decoded_len > BLOCK_SIZE || payload.len() != block.payload_len || payload.len() > BLOCK_SIZE {
        return Err(invalid("Retained block length is invalid"));
    }
    let decoded = match block.codec {
        Codec::Raw => payload.to_vec(),
        Codec::Zlib => {
            let mut decoder = ZlibDecoder::new(payload);
            let mut output = Vec::with_capacity(block.decoded_len);
            decoder.by_ref().take(block.decoded_len as u64 + 1).read_to_end(&mut output)?;
            if decoder.total_in() != payload.len() as u64 { return Err(invalid("Retained block has trailing data")); }
            output
        }
    };
    if decoded.len() != block.decoded_len || digest(&decoded) != block.hash { return Err(invalid("Retained block integrity check failed")); }
    Ok(decoded)
}

impl SourceHandle {
    #[cfg(test)]
    pub(crate) fn take_test_backing(&self) -> Vec<u8> {
        let mut file=self.0.file.as_ref().expect("spilled test source").borrow_mut();
        file.seek(SeekFrom::Start(0)).unwrap();let mut saved=vec![];file.read_to_end(&mut saved).unwrap();
        file.set_len(0).unwrap();saved
    }
    #[cfg(test)]
    pub(crate) fn restore_test_backing(&self, bytes:&[u8]) {
        let mut file=self.0.file.as_ref().expect("spilled test source").borrow_mut();
        file.seek(SeekFrom::Start(0)).unwrap();file.write_all(bytes).unwrap();file.set_len(bytes.len() as u64).unwrap();file.flush().unwrap();
    }

    pub(crate) fn touch(&self) { self.0.used.set(Instant::now()); }
    pub(crate) fn identity(&self) -> &SourceIdentity { &self.0.identity }
    pub(crate) fn lease(&self) -> io::Result<Arc<[u8]>> {
        if let Some(bytes) = self.0.cache.borrow().as_ref() {
            self.0.used.set(Instant::now());
            return Ok(bytes.clone());
        }
        let entry = &self.0;
        validate_length(entry.identity.len())?;
        if entry.blocks.len() != entry.identity.len().div_ceil(BLOCK_SIZE) { return Err(invalid("Retained block count differs")); }
        if let Some(file) = &entry.file {
            if file.borrow().metadata()?.len() != entry.file_len { return Err(invalid("Retained backing length differs")); }
        }
        let mut bytes = Vec::with_capacity(entry.identity.len());
        for (index, block) in entry.blocks.iter().enumerate() {
            let expected = (entry.identity.len() - index * BLOCK_SIZE).min(BLOCK_SIZE);
            if block.decoded_len != expected { return Err(invalid("Retained block order or length differs")); }
            let decoded = match &block.payload {
                Payload::Memory(payload) => decode_payload(block, payload)?,
                Payload::Spill { offset, length } => {
                    let length = *length as usize;
                    if length > BLOCK_SIZE + FRAME_HEADER + private_storage::MAX_OVERHEAD as usize ||
                       offset.checked_add(length as u64).is_none_or(|end| end > entry.file_len) {
                        return Err(invalid("Retained backing read exceeds its bound"));
                    }
                    let mut file = entry.file.as_ref().ok_or_else(|| invalid("Retained backing is missing"))?.borrow_mut();
                    file.seek(SeekFrom::Start(*offset))?;
                    let mut encrypted = vec![0; length];
                    file.read_exact(&mut encrypted)?;
                    let frame = private_storage::unseal(Purpose::HistorySourceBlock, &encrypted, FRAME_HEADER + BLOCK_SIZE).map_err(io::Error::other)?;
                    if frame.len() != FRAME_HEADER + block.payload_len || frame.len() < FRAME_HEADER ||
                       &frame[..8] != FRAME_MAGIC || frame[8..24] != entry.identity.0.nonce ||
                       frame[24..28] != (index as u32).to_le_bytes() || frame[28..32] != (block.decoded_len as u32).to_le_bytes() ||
                       frame[32] != block.codec as u8 { return Err(invalid("Retained block binding differs")); }
                    decode_payload(block, &frame[FRAME_HEADER..])?
                }
            };
            bytes.extend_from_slice(&decoded);
        }
        if bytes.len() != entry.identity.len() || digest(&bytes) != *entry.identity.sha256() { return Err(invalid("Retained source integrity check failed")); }
        let bytes: Arc<[u8]> = bytes.into();
        *entry.cache.borrow_mut() = Some(bytes.clone());
        entry.used.set(Instant::now());
        Ok(bytes)
    }
}

#[cfg(test)]
#[path = "source_storage_tests.rs"]
mod tests;
