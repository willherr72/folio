//! A preview owns one immutable copy of its input, including original ciphertext.
use super::{invalid, io, text_preview::{copy_hash, FileProof, MAX_BUNDLE}};
use crate::EngineResult;
use std::{fs, path::Path};

pub(super) struct OriginalSnapshot {
    file: tempfile::NamedTempFile,
    proof: FileProof,
}

// Unit-test-only metadata/read boundaries make real file changes deterministic.
#[cfg(test)]std::thread_local!{
 static CAPTURE_HOOKS:std::cell::RefCell<[Option<Box<dyn FnOnce()>>;2]>=std::cell::RefCell::new([None,None]);
}
#[cfg(test)]pub(crate) fn install_capture_hook_for_test(after_read:bool,hook:impl FnOnce()+'static){
 CAPTURE_HOOKS.with(|hooks|hooks.borrow_mut()[after_read as usize]=Some(Box::new(hook)));
}
#[cfg(test)]pub(crate) fn run_capture_hook_for_test(after_read:bool){
 let hook=CAPTURE_HOOKS.with(|hooks|hooks.borrow_mut()[after_read as usize].take());if let Some(hook)=hook{hook();}
}

// Both observations and all reads use the caller's held file, never a reopened
// path. This rejects observed size changes; it is not a same-size write lock.
pub(crate) fn copy_stable_file(input:&mut fs::File,output:&mut impl std::io::Write,expected:u64,limit:u64)->EngineResult<FileProof>{
    use std::io::Read;
    use sha2::{Digest,Sha256};
    if expected==0||expected>limit{return Err(invalid("The source PDF exceeds its snapshot byte limit."));}
    #[cfg(test)]run_capture_hook_for_test(false);
    let mut hash=Sha256::new();let mut total=0u64;let mut chunk=[0u8;32768];
    let mut bounded=(&mut *input).take(expected.saturating_add(1));
    loop{
        let n=match bounded.read(&mut chunk){Err(e)if e.kind()==std::io::ErrorKind::Interrupted=>continue,result=>result.map_err(io)?};
        if n==0{break;}
        total=total.checked_add(n as u64).filter(|n|*n<=expected).ok_or_else(||invalid("The source PDF changed size during capture."))?;
        hash.update(&chunk[..n]);output.write_all(&chunk[..n]).map_err(io)?;
    }
    #[cfg(test)]run_capture_hook_for_test(true);
    let final_meta=input.metadata().map_err(io)?;
    if total!=expected||!final_meta.is_file()||final_meta.len()!=expected{return Err(invalid("The source PDF changed size during capture."));}
    let sha256=hash.finalize().iter().map(|b|format!("{b:02x}")).collect();
    Ok(FileProof{bytes:total,sha256})
}

impl OriginalSnapshot {
    pub fn bytes(&self)->u64{self.proof.bytes}

    pub fn capture_open_file(input:&mut fs::File,limit:u64)->EngineResult<Self>{
        Self::capture_file(input,limit,"folio-compose-input-")
    }

    fn capture_file(input:&mut fs::File,limit:u64,prefix:&str)->EngineResult<Self>{
        let meta=input.metadata().map_err(io)?;
        if !meta.is_file()||meta.len()==0||meta.len()>limit{return Err(invalid("The source PDF must be a nonempty regular file within its byte limit."));}
        let mut file=tempfile::Builder::new().prefix(prefix).make(|path|{
            let mut options=fs::OpenOptions::new();options.read(true).write(true).create_new(true);
            #[cfg(windows)]{use std::os::windows::fs::OpenOptionsExt;use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_DELETE_ON_CLOSE;options.custom_flags(FILE_FLAG_DELETE_ON_CLOSE);}
            #[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
            options.open(path)
        }).map_err(io)?;
        let proof=copy_stable_file(input,file.as_file_mut(),meta.len(),limit)?;
        file.as_file_mut().sync_all().map_err(io)?;
        Ok(Self{file,proof})
    }

    pub fn capture(input:&Path)->EngineResult<Self>{Self::capture_bounded(input,MAX_BUNDLE)}

    fn capture_bounded(input:&Path,limit:u64)->EngineResult<Self>{
        let input=input.canonicalize().map_err(io)?;
        let mut input=fs::File::open(input).map_err(io)?;
        Self::capture_file(&mut input,limit,"folio-preview-input-")
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
    fn rejects_change(path_api:bool,after_read:bool,grow:bool){
        use std::io::{Seek,SeekFrom,Write};
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("source.pdf");fs::write(&path,vec![b'a';64]).unwrap();
        let mut writer=fs::OpenOptions::new().read(true).write(true).open(&path).unwrap();
        install_capture_hook_for_test(after_read,move||{
            if grow{writer.seek(SeekFrom::End(0)).unwrap();writer.write_all(b"appended update").unwrap();}else{writer.set_len(8).unwrap();}writer.sync_all().unwrap();
        });
        let result=if path_api{OriginalSnapshot::capture_bounded(&path,256)}else{let mut file=fs::File::open(&path).unwrap();OriginalSnapshot::capture_open_file(&mut file,256)};
        assert!(result.is_err(),"Observed below-budget source size change must refuse before native opening");
    }
    #[test]fn capture_rejects_observed_growth_before_read(){rejects_change(false,false,true)}
    #[test]fn capture_rejects_observed_shrink_before_read(){rejects_change(false,false,false)}
    #[test]fn capture_rejects_observed_growth_after_read(){rejects_change(false,true,true)}
    #[test]fn capture_rejects_observed_shrink_after_read(){rejects_change(false,true,false)}
    #[test]fn capture_rejects_observed_path_growth_before_read(){rejects_change(true,false,true)}
    #[test]fn capture_rejects_observed_path_shrink_before_read(){rejects_change(true,false,false)}
    #[test]fn capture_rejects_observed_path_growth_after_read(){rejects_change(true,true,true)}
    #[test]fn capture_rejects_observed_path_shrink_after_read(){rejects_change(true,true,false)}

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
