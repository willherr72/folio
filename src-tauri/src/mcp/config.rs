use crate::automation::text_preview::{valid_hash, FileProof};
use crate::automation::font_inputs::{FontInputs, MAX_FILES};
use std::{
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{self, Read},
    path::PathBuf,
};
use tempfile::{NamedTempFile, TempDir};

const MAX_INPUT_BYTES: u64 = 512 * 1024 * 1024;
pub(super) struct Document {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
    _snapshot: Snapshot,
}
enum Snapshot {
    Launch { _file: NamedTempFile },
    // Close the delete-on-close file before trying to remove its enclosing folder.
    Derived { _file: File, _directory: TempDir },
}
pub(super) struct Config {
    pub fonts: FontInputs,
    pub documents: Vec<Document>,
    pub output: PathBuf,
    pub engine: PathBuf,
    pub executable: PathBuf,
    next_document_id: u64,
}
impl Config {
    pub fn load(args: Vec<OsString>) -> Result<Self, &'static str> {
        let mut inputs = Vec::new();
        let mut font_paths=Vec::new();
        let (mut output, mut engine) = (None, None);
        let mut args = args.into_iter();
        while let Some(flag) = args.next() {
            let value = args
                .next()
                .filter(|v| !v.is_empty())
                .ok_or("Every MCP option requires a value.")?;
            match flag.to_str() {
                Some("--input") if inputs.len() < 64 => inputs.push(PathBuf::from(value)),
                Some("--font") if font_paths.len()<MAX_FILES => font_paths.push(PathBuf::from(value)),
                Some("--out-dir") if output.is_none() => output = Some(PathBuf::from(value)),
                Some("--engine") if engine.is_none() => engine = Some(PathBuf::from(value)),
                _ => return Err("Unknown, duplicate or excessive MCP option. See mcp --help."),
            }
        }
        if inputs.is_empty() {
            return Err("Specify 1 to 64 input PDFs with --input.");
        }
        let output = output
            .ok_or("Specify an existing output folder with --out-dir.")?
            .canonicalize()
            .map_err(|_| "Cannot open the output folder.")?;
        if !output.is_dir() {
            return Err("The output folder must be a directory.");
        }
        let engine = engine
            .map(Ok)
            .unwrap_or_else(crate::cli::engine_path)
            .map_err(|_| "Cannot resolve the PDF engine location.")?;
        // OS-aware resolution also handles Windows C:relative and rooted-without-
        // drive paths, without requiring the DLL to exist just for discovery.
        let engine =
            std::path::absolute(engine).map_err(|_| "Cannot resolve the PDF engine location.")?;
        let executable =
            std::env::current_exe().map_err(|_| "Cannot locate the Folio executable.")?;
        let fonts=FontInputs::load(&font_paths).map_err(|_|"Cannot load configured fonts. Supply supported standalone fonts, at most16 MiB each and64 MiB total; use the fonts command to validate them.")?;
        let mut documents = Vec::new();
        let mut remaining = MAX_INPUT_BYTES;
        for (index, input) in inputs.iter().enumerate() {
            let source = File::open(input).map_err(|_| "Cannot open a configured input file.")?;
            let metadata = source
                .metadata()
                .map_err(|_| "Cannot inspect a configured input file.")?;
            if !metadata.is_file() || metadata.len() > remaining {
                return Err("Inputs must be regular files totaling at most 512 MiB.");
            }
            let mut destination = snapshot().map_err(|_| "Cannot create an input snapshot.")?;
            let path = destination.path().to_owned();
            let bytes = io::copy(&mut source.take(remaining + 1), destination.as_file_mut())
                .map_err(|_| "Cannot copy a configured input file.")?;
            if bytes > remaining {
                return Err("Inputs exceed the 512 MiB snapshot limit.");
            }
            remaining -= bytes;
            let name = input
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            documents.push(Document {
                id: format!("document-{}", index + 1),
                name,
                path,
                bytes,
                _snapshot: Snapshot::Launch { _file: destination },
            });
        }
        Ok(Self {
            fonts,
            next_document_id: documents.len() as u64 + 1,
            documents,
            output,
            engine,
            executable,
        })
    }
    pub fn can_add(&self, bytes: u64) -> bool {
        self.documents.len() < 64
            && bytes > 0
            && bytes <= MAX_INPUT_BYTES
            && self.documents.iter().map(|d| d.bytes).sum::<u64>() <= MAX_INPUT_BYTES - bytes
            && self.next_document_id < u64::MAX
    }
    pub fn close(&mut self, index: usize) -> Result<serde_json::Value, &'static str> {
        let document = self
            .documents
            .get(index)
            .ok_or("Unknown document ID. Use folio_list_documents.")?;
        let reply = serde_json::json!({"closed":true,"documentId":document.id});
        drop(self.documents.remove(index));
        Ok(reply)
    }
    pub fn adopt(
        &mut self,
        directory: TempDir,
        proof: &FileProof,
    ) -> Result<serde_json::Value, &'static str> {
        if !self.can_add(proof.bytes) {
            return Err("Session document storage is full. Close an unused document before applying this preview.");
        }
        let bad = "Cannot adopt the worker's private document snapshot.";
        if !valid_hash(&proof.sha256) {
            return Err(bad);
        }
        let path = directory.path().join("Edited.pdf");
        let meta = std::fs::symlink_metadata(&path).map_err(|_| bad)?;
        if !meta.is_file() || meta.file_type().is_symlink() || meta.len() != proof.bytes {
            return Err(bad);
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_DELETE_ON_CLOSE;
            options.custom_flags(FILE_FLAG_DELETE_ON_CLOSE);
        }
        let file = options.open(&path).map_err(|_| bad)?;
        let meta = file.metadata().map_err(|_| bad)?;
        if !meta.is_file() || meta.len() != proof.bytes {
            return Err(bad);
        }
        let id = format!("document-{}", self.next_document_id);
        let name = format!("Edited-{}.pdf", self.next_document_id);
        let reply = serde_json::json!({"documentId":id,"name":name,"bytes":proof.bytes,"sha256":proof.sha256});
        self.documents.push(Document {
            id,
            name,
            path,
            bytes: proof.bytes,
            _snapshot: Snapshot::Derived {
                _file: file,
                _directory: directory,
            },
        });
        self.next_document_id += 1;
        Ok(reply)
    }
    pub fn list(&self) -> serde_json::Value {
        serde_json::json!({"documents":self.documents.iter().map(|d|
            serde_json::json!({"id":d.id,"name":d.name,"bytes":d.bytes})).collect::<Vec<_>>()})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn font_snapshot_exit_helper() {
        let Some(output)=std::env::var_os("FOLIO_MCP_FONT_EXIT_PROBE") else {return;};
        use std::io::Write;
        let mut file=font_snapshot().unwrap();file.write_all(b"private font input").unwrap();
        std::fs::write(output,serde_json::to_vec(file.path()).unwrap()).unwrap();
        std::process::exit(0);
    }
    #[cfg(windows)]
    #[test]
    fn forced_exit_releases_only_owned_font_snapshot() {
        use std::os::windows::process::CommandExt;
        let dir=tempfile::tempdir().unwrap();let report=dir.path().join("path.json");
        let original=dir.path().join("original.ttf");std::fs::write(&original,b"untouched").unwrap();
        let output=std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact","mcp::config::tests::font_snapshot_exit_helper","--nocapture"])
            .env("FOLIO_MCP_FONT_EXIT_PROBE",&report).creation_flags(0x08000000).output().unwrap();
        assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));
        let path:PathBuf=serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
        assert!(!path.exists());assert_eq!(std::fs::read(original).unwrap(),b"untouched");
    }
    fn config() -> (TempDir, Config) {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.pdf");
        std::fs::write(&source, b"source").unwrap();
        let config = Config::load(vec![
            "--input".into(),
            source.into_os_string(),
            "--out-dir".into(),
            dir.path().as_os_str().to_owned(),
        ])
        .unwrap();
        (dir, config)
    }
    fn stage() -> (TempDir, FileProof) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Edited.pdf"), b"result").unwrap();
        (
            dir,
            FileProof {
                bytes: 6,
                sha256: "a".repeat(64),
            },
        )
    }
    #[test]
    fn capacity_is_charged_and_recovered_without_reusing_ids() {
        let (_dir, mut config) = config();
        config.documents[0].bytes = MAX_INPUT_BYTES - 5;
        assert!(!config.can_add(6));
        let (dir, proof) = stage();
        let path = dir.path().to_owned();
        assert!(config.adopt(dir, &proof).is_err());
        assert!(!path.exists());
        config.documents[0].bytes = 6;
        for _ in 1..64 {
            let (dir, proof) = stage();
            config.adopt(dir, &proof).unwrap();
        }
        assert!(!config.can_add(6));
        let old = config.documents[1].path.clone();
        config.close(1).unwrap();
        assert!(!old.exists());
        let (dir, proof) = stage();
        let added = config.adopt(dir, &proof).unwrap();
        assert_eq!(added["documentId"], "document-65");
        assert_eq!(config.documents[1].id, "document-3");
        assert!(!config.can_add(0));
        assert!(!config.can_add(MAX_INPUT_BYTES + 1));
    }
    #[test]
    fn closing_and_dropping_derived_sources_releases_only_owned_files() {
        let (outside, mut config) = config();
        let (dir, proof) = stage();
        let folder = dir.path().to_owned();
        config.adopt(dir, &proof).unwrap();
        let live = config.documents[1].path.clone();
        assert!(live.exists());
        config.close(1).unwrap();
        assert!(!folder.exists());
        assert_eq!(
            std::fs::read(outside.path().join("source.pdf")).unwrap(),
            b"source"
        );
        let (dir, proof) = stage();
        let folder = dir.path().to_owned();
        config.adopt(dir, &proof).unwrap();
        drop(config);
        assert!(!folder.exists());
    }
    #[test]
    fn malformed_fixed_file_or_proof_does_not_publish_an_id() {
        let (_dir, mut config) = config();
        let (dir, mut proof) = stage();
        proof.bytes += 1;
        assert!(config.adopt(dir, &proof).is_err());
        assert_eq!(config.documents.len(), 1);
        assert_eq!(config.next_document_id, 2);
        let (dir, mut proof) = stage();
        proof.sha256 = "invalid".into();
        assert!(config.adopt(dir, &proof).is_err());
        let (dir, proof) = stage();
        std::fs::remove_file(dir.path().join("Edited.pdf")).unwrap();
        std::fs::create_dir(dir.path().join("Edited.pdf")).unwrap();
        assert!(config.adopt(dir, &proof).is_err());
        assert_eq!(config.documents.len(), 1);
    }
}

fn snapshot() -> io::Result<NamedTempFile> {
    snapshot_file("folio-mcp-input-", ".pdf")
}
pub(super) fn font_snapshot() -> io::Result<NamedTempFile> {
    snapshot_file("folio-mcp-font-", ".font")
}
fn snapshot_file(prefix: &str, suffix: &str) -> io::Result<NamedTempFile> {
    // Modern SDKs terminate disposable discovery probes without waiting for Rust
    // destructors. On Windows the kernel owns deletion of these input copies.
    // Keep the handle for the session; workers can open the path with shared delete.
    // Individual named files avoid leaving an empty directory after a killed probe.
    tempfile::Builder::new()
        .prefix(prefix)
        .suffix(suffix)
        .make(|path| {
            let mut options = OpenOptions::new();
            options.read(true).write(true).create_new(true);
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_DELETE_ON_CLOSE;
                options.custom_flags(FILE_FLAG_DELETE_ON_CLOSE);
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            options.open(path)
        })
}
