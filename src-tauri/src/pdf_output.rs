//! Bounded in-memory PDF composition and final-only atomic publication.
use crate::{EngineError, EngineResult};
use std::io::{self, Read, Write};
use std::path::Path;

const LIMIT: usize = crate::engine::MAX_SOURCE_BYTES as usize;

pub(crate) struct BoundedWriter {
    bytes: Vec<u8>,
    limit: usize,
    failed: bool,
}
impl BoundedWriter {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
            failed: false,
        }
    }
    pub(crate) fn finish(self) -> EngineResult<Vec<u8>> {
        if self.failed {
            Err(EngineError::InvalidRequest(
                "PDF output could not be completed within its byte limit".into(),
            ))
        } else {
            Ok(self.bytes)
        }
    }
}
impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.failed || bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            self.failed = true;
            return Err(io::Error::other("PDF output exceeds its byte limit"));
        }
        let needed = self.bytes.len() + bytes.len();
        if needed > self.bytes.capacity() {
            let capacity = needed
                .max(self.bytes.capacity().saturating_mul(2))
                .min(self.limit);
            if self
                .bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .is_err()
            {
                self.failed = true;
                return Err(io::Error::other("PDF output memory allocation failed"));
            }
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.failed {
            Err(io::Error::other("PDF output is incomplete"))
        } else {
            Ok(())
        }
    }
}

pub(crate) fn from_pdfium(
    document: &pdfium_render::prelude::PdfDocument<'_>,
) -> EngineResult<Vec<u8>> {
    let mut writer = BoundedWriter::new(LIMIT);
    let result = document.save_to_writer(&mut writer);
    let bytes = writer.finish()?;
    result?;
    Ok(bytes)
}

pub(crate) fn load(bytes: &[u8]) -> EngineResult<lopdf::Document> {
    view(bytes).map(crate::request_parse::ReadDocument::into_owned)
}

pub(crate) fn view(bytes: &[u8]) -> EngineResult<crate::request_parse::ReadDocument> {
    if bytes.is_empty() || bytes.len() > LIMIT {
        return Err(EngineError::InvalidRequest(
            "PDF output must be between 1 byte and 512 MiB".into(),
        ));
    }
    crate::request_parse::read(bytes,crate::request_parse::Policy::Strict64)
    .map_err(|error| EngineError::InvalidRequest(format!("PDF composition: {error}")))
}

pub(crate) fn save(document: &mut lopdf::Document) -> EngineResult<Vec<u8>> {
    let mut writer = BoundedWriter::new(LIMIT);
    let result = document.save_to(&mut writer);
    let bytes = writer.finish()?;
    result.map_err(|error| EngineError::Io(format!("composing PDF: {error}")))?;
    Ok(bytes)
}

pub(crate) fn save_controlled(document: &mut lopdf::Document, control: Option<&crate::PdfSaveControl>) -> EngineResult<Vec<u8>> {
    let Some(control) = control else { return save(document); };
    save_checked(document, &mut || {
        if control.cancelled() { Err(EngineError::InvalidRequest("PDF save cancelled".into())) } else { Ok(()) }
    })
}

pub(crate) fn save_checked(document: &mut lopdf::Document, check: &mut impl FnMut() -> EngineResult<()>) -> EngineResult<Vec<u8>> {
    let mut writer = CheckedWriter { inner: BoundedWriter::new(LIMIT), check, error: None };
    let result = document.save_to(&mut writer);
    if result.is_ok() { let _ = writer.checkpoint(); }
    if let Some(error) = writer.error { return Err(error); }
    let bytes = writer.inner.finish()?;
    result.map_err(|error| EngineError::Io(format!("composing PDF: {error}")))?;
    Ok(bytes)
}

struct CheckedWriter<'a, F> {
    inner: BoundedWriter,
    check: &'a mut F,
    error: Option<EngineError>,
}
impl<F: FnMut() -> EngineResult<()>> CheckedWriter<'_, F> {
    fn checkpoint(&mut self) -> io::Result<()> {
        if self.error.is_some() { return Err(io::Error::other("PDF output is incomplete")); }
        if let Err(error) = (self.check)() {
            self.error = Some(error);
            return Err(io::Error::other("PDF output cancelled or refused"));
        }
        Ok(())
    }
}
impl<F: FnMut() -> EngineResult<()>> Write for CheckedWriter<'_, F> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.checkpoint()?;
        if bytes.is_empty() { return self.inner.write(bytes); }
        for (index, chunk) in bytes.chunks(64 * 1024).enumerate() {
            if index != 0 { self.checkpoint()?; }
            self.inner.write_all(chunk)?;
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.checkpoint()?;
        self.inner.flush()
    }
}

pub(crate) fn read(path: &Path) -> EngineResult<Vec<u8>> {
    let file = std::fs::File::open(path).map_err(io_error)?;
    if file.metadata().map_err(io_error)?.len() > LIMIT as u64 {
        return Err(EngineError::InvalidRequest(
            "PDF exceeds the 512 MiB limit".into(),
        ));
    }
    let mut bytes = Vec::new();
    file.take(LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() > LIMIT {
        return Err(EngineError::InvalidRequest(
            "PDF exceeds the 512 MiB limit".into(),
        ));
    }
    Ok(bytes)
}

/// Only call after composition, authorization and any required encryption finish.
pub(crate) fn publish(path: &Path, bytes: &[u8]) -> EngineResult<()> {
    publish_impl(path, bytes, None, || {}, |_| {}).map(|_| ())
}

pub(crate) fn publish_controlled(path: &Path, bytes: &[u8], control: &crate::PageImageControl, committed: impl FnOnce()) -> EngineResult<bool> {
    publish_impl(path, bytes, Some(control), committed, |_| {})
}

fn publish_impl(path: &Path, bytes: &[u8], control: Option<&crate::PageImageControl>, committed: impl FnOnce(), mut chunk_written: impl FnMut(usize)) -> EngineResult<bool> {
    if bytes.is_empty() || bytes.len() > LIMIT {
        return Err(EngineError::InvalidRequest(
            "PDF output must be between 1 byte and 512 MiB".into(),
        ));
    }
    if control.is_some_and(|c| c.cancelled()) { return Ok(false); }
    let parent = path
        .parent()
        .ok_or_else(|| EngineError::InvalidRequest("destination has no parent".into()))?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".folio-export-")
        .suffix(".pdf")
        .tempfile_in(parent)
        .map_err(|e| EngineError::Io(format!("creating PDF output temporary: {e}")))?;
    if let Some(control) = control {
        let mut written = 0;
        for chunk in bytes.chunks(256 * 1024) {
            if control.cancelled() { return Ok(false); }
            temporary.write_all(chunk).map_err(|e| EngineError::Io(format!("writing PDF output temporary: {e}")))?;
            written += chunk.len();
            chunk_written(written);
        }
        if control.cancelled() { return Ok(false); }
    } else {
        temporary.write_all(bytes).map_err(|e| EngineError::Io(format!("writing PDF output temporary: {e}")))?;
    }
    temporary.as_file().sync_all().map_err(|e| EngineError::Io(format!("syncing PDF output temporary: {e}")))?;
    if let Some(control) = control {
        if !control.commit() {
            return if control.cancelled() { Ok(false) } else {
                Err(EngineError::InvalidRequest("This PDF save has already reached publication.".into()))
            };
        }
    }
    committed();
    // Windows readers can briefly deny delete/rename sharing. Keep the same
    // fully written, synced temporary and retry only the final replacement.
    // Persistent access failures still preserve the destination and return an
    // error; dropping PersistError also removes our unpublished temporary.
    for attempt in 0..=10 {
        match temporary.persist(path) {
            Ok(_) => {
                if let Some(control) = control { control.completed(); }
                return Ok(true);
            },
            Err(error) => {
                let retry = cfg!(windows)
                    && matches!(error.error.raw_os_error(), Some(5 | 32 | 33))
                    && attempt < 10;
                if !retry {
                    return Err(EngineError::Io(format!("publishing PDF output: {}",error.error)));
                }
                temporary = error.file;
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
        }
    }
    unreachable!("the last publication attempt returns its result")
}
fn io_error(error: io::Error) -> EngineError {
    EngineError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn controlled_serialization_cancel_never_returns_partial_output_and_success_matches_ordinary() {
        let mut source = lopdf::Document::with_version("1.7");
        source.add_object(lopdf::Stream::new(lopdf::dictionary! {}, vec![42; 2 * 1024 * 1024]));
        let expected = save(&mut source.clone()).unwrap();
        assert_eq!(save_checked(&mut source.clone(), &mut || Ok(())).unwrap(), expected);
        for stop in [0, 5, 25] {
            let mut calls = 0;
            let result = save_checked(&mut source.clone(), &mut || {
                calls += 1;
                if calls > stop { Err(EngineError::InvalidRequest("test serialization cancellation".into())) } else { Ok(()) }
            });
            let error = result.err().expect("cancelled serialization must refuse");
            assert!(error.to_string().contains("test serialization cancellation"));
        }
    }

    #[test]
    fn controlled_serialization_checks_inside_large_writes_and_latches_errors() {
        let mut calls = 0;
        let mut check = || {
            calls += 1;
            if calls == 2 { Err(EngineError::InvalidRequest("test chunk cancellation".into())) } else { Ok(()) }
        };
        let mut writer = CheckedWriter { inner: BoundedWriter::new(1024 * 1024), check: &mut check, error: None };
        assert!(writer.write_all(&vec![0; 512 * 1024]).is_err());
        assert!(writer.inner.bytes.len() <= 64 * 1024);
        assert!(writer.write_all(b"ignored recovery").is_err());
        assert!(writer.flush().is_err());
        assert!(writer.error.as_ref().unwrap().to_string().contains("test chunk cancellation"));
    }

    #[test]
    fn controlled_serialization_final_checkpoint_and_inner_failures_are_not_hidden() {
        let source = lopdf::Document::with_version("1.7");
        let mut total = 0;
        save_checked(&mut source.clone(), &mut || { total += 1; Ok(()) }).unwrap();
        let mut calls = 0;
        let result = save_checked(&mut source.clone(), &mut || {
            calls += 1;
            if calls == total { Err(EngineError::InvalidRequest("final checkpoint".into())) } else { Ok(()) }
        });
        assert!(result.err().expect("final cancellation must refuse").to_string().contains("final checkpoint"));
        let mut check = || Ok(());
        let mut writer = CheckedWriter { inner: BoundedWriter::new(1), check: &mut check, error: None };
        assert!(writer.write_all(b"too big").is_err());
        assert!(writer.write(b"").is_err(), "empty writes cannot clear an underlying sticky failure");
        assert!(writer.flush().is_err());
        assert!(writer.inner.finish().is_err());
    }

    #[test]
    fn controlled_publish_cancel_preserves_existing_file_and_creates_no_temporary() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("saved.pdf");
        std::fs::write(&path, b"original complete PDF").unwrap();
        let control = crate::PageImageControl::new();
        assert!(control.cancel());
        assert!(!publish_controlled(&path, b"new complete PDF", &control, || panic!("cancelled publication cannot commit")).unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), b"original complete PDF");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn controlled_publish_late_cancel_cannot_hide_completed_output() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("saved.pdf");
        let control = crate::PageImageControl::new();
        assert!(publish_controlled(&path, b"new complete PDF", &control, || assert!(!control.cancel())).unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), b"new complete PDF");
        assert!(!control.cancel());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn controlled_publish_cancellation_during_write_removes_only_its_temporary() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("saved.pdf");
        let unrelated = directory.path().join("other.pdf");
        std::fs::write(&path, b"original complete PDF").unwrap();
        std::fs::write(&unrelated, b"unrelated").unwrap();
        let control = crate::PageImageControl::new();
        let mut writes = 0;
        let saved = publish_impl(&path, &vec![42; 1024 * 1024], Some(&control), || panic!("must not commit"), |written| {
            writes += 1;
            assert_eq!(written, 256 * 1024);
            assert!(control.cancel());
        }).unwrap();
        assert!(!saved); assert_eq!(writes, 1);
        assert_eq!(std::fs::read(&path).unwrap(), b"original complete PDF");
        assert_eq!(std::fs::read(&unrelated).unwrap(), b"unrelated");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    }

    #[cfg(windows)]
    fn hold_destination(path: &Path) -> std::fs::File {
        use std::os::windows::fs::OpenOptionsExt;
        // Permit reading/writing, but deliberately deny delete/rename sharing.
        std::fs::OpenOptions::new().read(true).share_mode(3).open(path).unwrap()
    }

    #[test]
    #[cfg(windows)]
    fn publish_waits_for_a_brief_destination_lock_without_exposing_partial_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("saved.pdf");
        std::fs::write(&path,b"previous complete PDF").unwrap();
        let locked = hold_destination(&path);
        let output = path.clone();
        let (started,receive) = std::sync::mpsc::channel();
        let job = std::thread::spawn(move || {
            started.send(()).unwrap();
            publish(&output,b"new complete PDF")
        });
        receive.recv().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(std::fs::read(&path).unwrap(),b"previous complete PDF");
        drop(locked);
        job.join().unwrap().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(),b"new complete PDF");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(),1);
    }

    #[test]
    #[cfg(windows)]
    fn publish_keeps_existing_bytes_when_destination_lock_persists() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("saved.pdf");
        std::fs::write(&path,b"previous complete PDF").unwrap();
        let locked = hold_destination(&path);
        let start=std::time::Instant::now();
        let result=publish(&path,b"new complete PDF");
        assert!(start.elapsed()<std::time::Duration::from_secs(2));
        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(),b"previous complete PDF");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(),1);
        drop(locked);
    }

    #[test]
    #[cfg(windows)]
    fn publish_does_not_remove_readonly_protection_or_replace_existing_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("saved.pdf");
        std::fs::write(&path,b"previous complete PDF").unwrap();
        let original=std::fs::metadata(&path).unwrap().permissions();
        let mut readonly=original.clone();readonly.set_readonly(true);
        std::fs::set_permissions(&path,readonly).unwrap();
        let result=publish(&path,b"new complete PDF");
        let retained=std::fs::read(&path).unwrap();
        let still_readonly=std::fs::metadata(&path).unwrap().permissions().readonly();
        std::fs::set_permissions(&path,original).unwrap();
        assert!(result.is_err());assert!(still_readonly);
        assert_eq!(retained,b"previous complete PDF");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(),1);
    }

    #[test]
    fn writer_accepts_exact_limit_and_empty_writes() {
        let mut writer = BoundedWriter::new(8);
        writer.write_all(b"123").unwrap();
        writer.write_all(b"45678").unwrap();
        writer.write_all(b"").unwrap();
        assert_eq!(writer.finish().unwrap(), b"12345678");
    }

    #[test]
    fn overflow_is_sticky_and_never_returns_partial_bytes() {
        let mut writer = BoundedWriter::new(8);
        writer.write_all(b"1234567").unwrap();
        assert!(writer.write_all(b"89").is_err());
        assert!(writer.write_all(b"8").is_err());
        assert!(writer.flush().is_err());
        assert!(writer.finish().is_err());
        let mut writer = BoundedWriter::new(0);
        assert!(writer.write_all(b"x").is_err());
        assert!(writer.finish().is_err());
    }

    #[test]
    fn real_pdf_serialization_propagates_writer_failure() {
        let mut document = lopdf::Document::with_version("1.7");
        let mut writer = BoundedWriter::new(16);
        assert!(document.save_to(&mut writer).is_err());
        assert!(writer.finish().is_err());
    }
}
