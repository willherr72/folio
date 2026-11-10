//! A preview owns one immutable copy of its input, including original ciphertext.
use super::{invalid, io, text_preview::{copy_hash, FileProof, MAX_BUNDLE}};
use crate::EngineResult;
use std::{fs, path::Path};

pub(super) struct OriginalSnapshot {
    file: tempfile::NamedTempFile,
    proof: FileProof,
}

impl OriginalSnapshot {
    pub fn capture(input: &Path) -> EngineResult<Self> {
        Self::capture_bounded(input, MAX_BUNDLE)
    }

    fn capture_bounded(input: &Path, limit: u64) -> EngineResult<Self> {
        // Match normal opening's symlink handling, then read the resolved file
        // once into bounded private storage before native authentication/opening.
        let input = input.canonicalize().map_err(io)?;
        let metadata = fs::metadata(&input).map_err(io)?;
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() > limit {
            return Err(invalid("The source PDF must be a nonempty regular file within its byte limit."));
        }
        let mut file = tempfile::Builder::new()
            .prefix("folio-preview-input-")
            .make(|path| {
                let mut options = fs::OpenOptions::new();
                options.read(true).write(true).create_new(true);
                #[cfg(windows)]
                {
                    use std::os::windows::fs::OpenOptionsExt;
                    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_DELETE_ON_CLOSE;
                    // MCP can terminate the worker without Rust unwinding. Match
                    // the existing session-input snapshot's kernel-owned lifetime.
                    options.custom_flags(FILE_FLAG_DELETE_ON_CLOSE);
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                options.open(path)
            })
            .map_err(io)?;
        // The streaming limit also rejects a file that grows after metadata.
        let proof = copy_hash(&input, limit, file.as_file_mut())?;
        if proof.bytes == 0 {
            return Err(invalid("The source PDF is empty."));
        }
        file.as_file_mut().sync_all().map_err(io)?;
        Ok(Self { file, proof })
    }

    pub fn path(&self) -> &Path {
        self.file.path()
    }

    pub fn write_candidate(&self, destination: &Path) -> EngineResult<FileProof> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .map_err(io)?;
        let proof = copy_hash(self.path(), MAX_BUNDLE, &mut file)?;
        if proof != self.proof {
            return Err(invalid("The private input snapshot changed. Create a fresh preview."));
        }
        file.sync_all().map_err(io)?;
        Ok(proof)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn snapshot_exit_helper() {
        let Some(directory) = std::env::var_os("FOLIO_SNAPSHOT_EXIT_PROBE") else { return; };
        let directory = std::path::PathBuf::from(directory);
        let snapshot = OriginalSnapshot::capture(&directory.join("source.pdf")).unwrap();
        fs::write(directory.join("private-path.json"), serde_json::to_vec(snapshot.path()).unwrap()).unwrap();
        // Disposable MCP workers may terminate without running Rust destructors.
        std::process::exit(0);
    }

    #[cfg(windows)]
    #[test]
    fn kernel_releases_snapshot_when_worker_exits_without_destructors() {
        use std::os::windows::process::CommandExt;
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("source.pdf"), b"synthetic private input").unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "automation::original_snapshot::tests::snapshot_exit_helper", "--nocapture"])
            .env("FOLIO_SNAPSHOT_EXIT_PROBE", dir.path())
            .creation_flags(0x08000000)
            .output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let private: std::path::PathBuf = serde_json::from_slice(&fs::read(dir.path().join("private-path.json")).unwrap()).unwrap();
        let leaked = private.exists();
        // Clean only this child-owned synthetic reproduction if the regression fails.
        if leaked { fs::remove_file(&private).unwrap(); }
        assert!(!leaked, "A terminated worker left its private input snapshot behind");
    }

    #[test]
    fn original_path_changes_cannot_change_an_open_preview_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.pdf");
        fs::write(&source, b"original encrypted or unprotected bytes").unwrap();
        let snapshot = OriginalSnapshot::capture(&source).unwrap();
        let private_path = snapshot.path().to_owned();
        fs::write(&source, b"a different file at the original path").unwrap();
        let candidate = dir.path().join("candidate.pdf");
        snapshot.write_candidate(&candidate).unwrap();
        assert_eq!(fs::read(candidate).unwrap(), b"original encrypted or unprotected bytes");
        assert_eq!(fs::read(source).unwrap(), b"a different file at the original path");
        drop(snapshot);
        assert!(!private_path.exists());
    }

    #[test]
    fn snapshot_copy_rejects_tampering_and_preserves_existing_destinations() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.pdf");
        fs::write(&source, b"original").unwrap();
        let snapshot = OriginalSnapshot::capture(&source).unwrap();
        let existing = dir.path().join("existing.pdf");
        fs::write(&existing, b"keep").unwrap();
        assert!(snapshot.write_candidate(&existing).is_err());
        assert_eq!(fs::read(existing).unwrap(), b"keep");
        fs::write(snapshot.path(), b"tampered").unwrap();
        assert!(snapshot.write_candidate(&dir.path().join("candidate.pdf")).is_err());
    }

    #[test]
    fn bounded_copy_rejects_growth_before_writing_over_budget() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.pdf");
        fs::write(&source, b"12345").unwrap();
        assert!(OriginalSnapshot::capture_bounded(&source, 4).is_err());
        let mut output = Vec::new();
        assert!(copy_hash(&source, 4, &mut output).is_err());
        assert!(output.len() <= 4);
        fs::write(&source, b"").unwrap();
        assert!(OriginalSnapshot::capture(&source).is_err());
        assert!(OriginalSnapshot::capture(dir.path()).is_err());
    }
}
