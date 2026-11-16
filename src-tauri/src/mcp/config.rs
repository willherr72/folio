use crate::automation::text_preview::{valid_hash, FileProof};
use crate::automation::font_inputs::{FontInputs, MAX_FILES};
use sha2::{Digest,Sha256};
use std::{
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{self, Read},
    path::{Path,PathBuf},
    sync::Arc,
};
use tempfile::{NamedTempFile, TempDir};

const MAX_INPUT_BYTES: u64 = 512 * 1024 * 1024;
pub(super) type Credential=Option<Arc<crate::private_storage::Sensitive>>;
pub(super) fn verified_candidate(path:&Path,proof:&FileProof,delete_on_close:bool)->Result<File,&'static str>{
    let bad="Cannot validate the worker's immutable document snapshot.";
    if proof.bytes==0||proof.bytes>MAX_INPUT_BYTES||!valid_hash(&proof.sha256){return Err(bad);}
    let meta=std::fs::symlink_metadata(path).map_err(|_|bad)?;
    if !meta.is_file()||meta.file_type().is_symlink()||meta.len()!=proof.bytes{return Err(bad);}
    let mut options=OpenOptions::new();options.read(true).write(true);
    #[cfg(windows)]{
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_DELETE_ON_CLOSE,FILE_FLAG_OPEN_REPARSE_POINT,FILE_SHARE_READ};
        options.share_mode(FILE_SHARE_READ).custom_flags(FILE_FLAG_OPEN_REPARSE_POINT|if delete_on_close{FILE_FLAG_DELETE_ON_CLOSE}else{0});
    }
    #[cfg(not(windows))]let _=delete_on_close;
    let mut file=options.open(path).map_err(|_|bad)?;let meta=file.metadata().map_err(|_|bad)?;
    if !meta.is_file()||meta.file_type().is_symlink()||meta.len()!=proof.bytes{return Err(bad);}
    let mut digest=Sha256::new();let mut count=0u64;let mut chunk=[0u8;32768];
    loop{let n=file.read(&mut chunk).map_err(|_|bad)?;if n==0{break;}count=count.checked_add(n as u64).filter(|n|*n<=MAX_INPUT_BYTES).ok_or(bad)?;digest.update(&chunk[..n]);}
    let actual:String=digest.finalize().iter().map(|b|format!("{b:02x}")).collect();
    if count!=proof.bytes||actual!=proof.sha256{return Err(bad);}Ok(file)
}
pub(super) struct Document {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub credential:Credential,
    _snapshot: Snapshot,
}
enum Snapshot {
    Launch { _file: Arc<NamedTempFile> },
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
        let (mut output, mut engine,mut credential_file) = (None, None,None);
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
                Some("--credentials") if credential_file.is_none()=>credential_file=Some(PathBuf::from(value)),
                _ => return Err("Unknown, duplicate or excessive MCP option. See mcp --help."),
            }
        }
        if inputs.is_empty() {
            return Err("Specify 1 to 64 input PDFs with --input.");
        }
        let credentials=if let Some(path)=credential_file{
            let file=File::open(path).map_err(|_|"Cannot open launch credentials.")?;
            if !file.metadata().map_err(|_|"Cannot inspect launch credentials.")?.is_file(){return Err("Launch credentials must be a regular bounded JSON file.");}
            crate::automation::credentials::Credentials::read(file,inputs.len())?
        }else{crate::automation::credentials::Credentials::none(inputs.len())};
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
        let mut documents:Vec<Document> = Vec::new();
        let mut identities:Vec<(same_file::Handle,usize)>=Vec::new();
        let mut remaining = MAX_INPUT_BYTES;
        for (index, input) in inputs.iter().enumerate() {
            let mut source = File::open(input).map_err(|_| "Cannot open a configured input file.")?;
            let identity=same_file::Handle::from_file(source.try_clone().map_err(|_|"Cannot identify a configured input.")?).map_err(|_|"Cannot identify a configured input.")?;
            let metadata = source
                .metadata()
                .map_err(|_| "Cannot inspect a configured input file.")?;
            if !metadata.is_file() ||metadata.len()==0|| metadata.len() > remaining {
                return Err("Inputs must be regular files totaling at most 512 MiB.");
            }
            let (destination,bytes)=if let Some((_,first))=identities.iter().find(|(id,_)|*id==identity){
                if credentials.get(index)!=credentials.get(*first){return Err("Aliased inputs must have identical launch credentials.");}
                let Snapshot::Launch{_file}= &documents[*first]._snapshot else{unreachable!()};(_file.clone(),documents[*first].bytes)
            }else{
                let mut destination = snapshot().map_err(|_| "Cannot create an input snapshot.")?;
                let bytes=crate::automation::copy_stable_file(&mut source,destination.as_file_mut(),metadata.len(),remaining).map_err(|_|"Cannot copy a configured input file or its size changed during capture.")?.bytes;
                destination.as_file().sync_all().map_err(|_|"Cannot synchronize an input snapshot.")?;
                identities.push((identity,index));(Arc::new(destination),bytes)
            };
            let path=destination.path().to_owned();
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
                credential:credentials.secret(index),
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
        self.adopt_checked(directory,proof,|_|Ok(()))
    }
    pub fn adopt_checked(&mut self,directory:TempDir,proof:&FileProof,check:impl FnOnce(&mut serde_json::Value)->Result<(),&'static str>)->Result<serde_json::Value,&'static str>{
        self.adopt_authenticated_checked(directory,proof,None,check)
    }
    pub fn adopt_authenticated(&mut self,directory:TempDir,proof:&FileProof,credential:Credential)->Result<serde_json::Value,&'static str>{
        self.adopt_authenticated_checked(directory,proof,credential,|_|Ok(()))
    }
    pub fn adopt_authenticated_checked(&mut self,directory:TempDir,proof:&FileProof,credential:Credential,check:impl FnOnce(&mut serde_json::Value)->Result<(),&'static str>)->Result<serde_json::Value,&'static str>{
        if !self.can_add(proof.bytes) {
            return Err("Session document storage is full. Close an unused document before applying this preview.");
        }
        let path = directory.path().join("Edited.pdf");
        let file=verified_candidate(&path,proof,true)?;
        let id = format!("document-{}", self.next_document_id);
        let name = format!("Edited-{}.pdf", self.next_document_id);
        let mut reply = serde_json::json!({"documentId":id,"name":name,"bytes":proof.bytes,"sha256":proof.sha256});
        check(&mut reply)?;
        self.documents.push(Document {
            id,
            name,
            path,
            bytes: proof.bytes,
            credential,
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
    fn rejects_changed_launch(after_read:bool,grow:bool){
        use std::io::{Seek,SeekFrom,Write};
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("source.pdf");std::fs::write(&path,vec![b'a';64]).unwrap();
        let mut writer=std::fs::OpenOptions::new().read(true).write(true).open(&path).unwrap();
        crate::automation::install_capture_hook_for_test(after_read,move||{
            if grow{writer.seek(SeekFrom::End(0)).unwrap();writer.write_all(b"appended update").unwrap();}else{writer.set_len(8).unwrap();}writer.sync_all().unwrap();
        });
        let args=vec!["--input".into(),path.into_os_string(),"--out-dir".into(),dir.path().as_os_str().to_owned()];
        assert!(Config::load(args).is_err(),"Observed launch source size changes must refuse below the aggregate budget");
    }
    #[test]fn capture_rejects_observed_mcp_growth_before_read(){rejects_changed_launch(false,true)}
    #[test]fn capture_rejects_observed_mcp_shrink_before_read(){rejects_changed_launch(false,false)}
    #[test]fn capture_rejects_observed_mcp_growth_after_read(){rejects_changed_launch(true,true)}
    #[test]fn capture_rejects_observed_mcp_shrink_after_read(){rejects_changed_launch(true,false)}
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
    #[test]
    fn credential_aliases_share_snapshot_and_reject_conflicting_capabilities(){
        let dir=tempfile::tempdir().unwrap();let source=dir.path().join("source.pdf");std::fs::write(&source,b"source bytes").unwrap();let alias=dir.path().join("alias.pdf");std::fs::hard_link(&source,&alias).unwrap();let file=dir.path().join("credentials.json");
        let args=vec!["--input".into(),source.as_os_str().to_owned(),"--input".into(),alias.as_os_str().to_owned(),"--out-dir".into(),dir.path().as_os_str().to_owned(),"--credentials".into(),file.as_os_str().to_owned()];
        std::fs::write(&file,br#"[null,"reader"]"#).unwrap();assert!(Config::load(args.clone()).is_err());
        std::fs::write(&file,br#"["reader","reader"]"#).unwrap();let mut config=Config::load(args).unwrap();let snapshot=config.documents[0].path.clone();assert_eq!(snapshot,config.documents[1].path);assert!(!config.list().to_string().contains("reader"));
        config.close(0).unwrap();assert!(snapshot.exists());config.close(0).unwrap();assert!(!snapshot.exists());assert_eq!(std::fs::read(source).unwrap(),b"source bytes");
    }
    fn stage() -> (TempDir, FileProof) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Edited.pdf"), b"result").unwrap();
        (
            dir,
            FileProof {
                bytes: 6,
                sha256: Sha256::digest(b"result").iter().map(|b|format!("{b:02x}")).collect(),
            },
        )
    }
    #[test]
    fn page_organization_rejects_wrong_candidate_hash_without_adding_document() {
        let (_dir,mut config)=config();let (dir,mut proof)=stage();proof.sha256="a".repeat(64);let path=dir.path().to_owned();
        assert!(config.adopt(dir,&proof).is_err(),"A valid-looking wrong hash must not authorize a new document");
        assert_eq!(config.documents.len(),1);assert!(!path.exists());
    }
    #[test]
    fn page_organization_rejects_same_size_candidate_corruption() {
        use sha2::{Digest,Sha256};
        let (_dir,mut config)=config();let (dir,mut proof)=stage();
        proof.sha256=Sha256::digest(b"result").iter().map(|b|format!("{b:02x}")).collect();
        std::fs::write(dir.path().join("Edited.pdf"),b"RESULT").unwrap();let path=dir.path().to_owned();
        assert!(config.adopt(dir,&proof).is_err(),"Matching length is insufficient after the candidate bytes change");
        assert_eq!(config.documents.len(),1);assert!(!path.exists());
    }
    #[cfg(windows)]
    #[test]
    fn page_organization_derived_snapshot_denies_mutation_until_close() {
        use sha2::{Digest,Sha256};
        let (_dir,mut config)=config();let (dir,mut proof)=stage();
        proof.sha256=Sha256::digest(b"result").iter().map(|b|format!("{b:02x}")).collect();
        config.adopt(dir,&proof).unwrap();let path=config.documents[1].path.clone();
        assert_eq!(std::fs::read(&path).unwrap(),b"result");
        assert!(std::fs::OpenOptions::new().write(true).open(&path).is_err(),"An adopted immutable source must deny other writers");
        config.close(1).unwrap();assert!(!path.exists());
    }
    #[test]
    fn page_organization_rejected_reply_does_not_adopt_or_consume_capacity() {
        let (_dir,mut config)=config();let (dir,proof)=stage();let path=dir.path().to_owned();
        assert!(config.adopt_checked(dir,&proof,|_|Err("response rejected")).is_err());
        assert_eq!(config.documents.len(),1);assert!(!path.exists());
        let (dir,proof)=stage();let accepted=config.adopt(dir,&proof).unwrap();assert_eq!(accepted["documentId"],"document-2");
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
