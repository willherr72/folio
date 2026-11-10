//! One session's native worker and private document/preview lifetimes.
use super::{
    config::Config,
    previews::PreviewStore,
    protocol::{self, Action, Call, Protocol},
    worker::Worker,
};
use serde_json::Value;

pub(super) struct Session {
    // Struct fields drop in declaration order: reap the worker before its inputs.
    active: Option<Worker>,
    previews: PreviewStore,
    config: Config,
    protocol: Protocol,
}
impl Session {
    pub fn new(config: Config) -> Self {
        Self {
            active: None,
            previews: PreviewStore::default(),
            config,
            protocol: Protocol::default(),
        }
    }
    pub fn prune(&mut self) {
        if self.active.is_none() {
            self.previews.prune();
        }
    }
    pub fn dispatch(&mut self, value: Value) -> Option<Value> {
        match self.protocol.dispatch(value, &self.config.documents) {
            Action::Reply(value) => Some(value),
            Action::Call {id,modern,call:Call::ListFonts} => Some(protocol::tool_result(id,modern,self.config.fonts.catalog(),false)),
            Action::Call {
                id,
                modern,
                call: Call::List,
            } => {
                let mut result = self.config.list();
                result["previews"] = self.previews.list();
                Some(protocol::tool_result(id, modern, result, false))
            }
            Action::Call { id, modern, call } => self.call(id, modern, call),
            Action::Cancel(id) => {
                if self.active.as_ref().is_some_and(|w| w.id == id) {
                    drop(self.active.take());
                }
                None
            }
            Action::None => None,
        }
    }
    fn call(&mut self, id: Value, modern: bool, call: Call) -> Option<Value> {
        if self.active.is_some() {
            return Some(protocol::tool_error(
                id,
                modern,
                "busy",
                "Another PDF operation is running. Wait for its result or cancel it.",
            ));
        }
        if let Call::CloseDocument { document } = &call {
            return Some(match self.config.close(*document) {
                Ok(result) => protocol::tool_result(id, modern, result, false),
                Err(message) => protocol::tool_error(id, modern, "unknown_document", message),
            });
        }
        if let Call::DiscardPreview { preview } = &call {
            return Some(match self.previews.discard(preview) {
                Ok(result) => protocol::tool_result(id, modern, result, false),
                Err(message) => protocol::tool_error(id, modern, "unknown_preview", message),
            });
        }
        if matches!(&call, Call::PreviewText { .. } | Call::PreviewParagraph {..} | Call::PreviewForm {..}) && !self.previews.has_room() {
            return Some(protocol::tool_error(
                id,
                modern,
                "preview_limit",
                "Preview storage is full. Discard a preview before creating another.",
            ));
        }
        let preview = match &call {
            Call::SavePreview { preview } | Call::ApplyPreview { preview } => {
                match self.previews.get(preview) {
                    Some(p) => Some(p),
                    None => {
                        return Some(protocol::tool_error(
                            id,
                            modern,
                            "unknown_preview",
                            "This preview is unavailable or expired. Create a fresh preview.",
                        ))
                    }
                }
            }
            _ => None,
        };
        if matches!(&call, Call::ApplyPreview { .. })
            && !self.config.can_add(preview.unwrap().candidate.bytes)
        {
            return Some(protocol::tool_error(id,modern,"document_limit","Session document storage is full. Close an unused document before applying this preview."));
        }
        match Worker::start(&self.config, id.clone(), modern, call, preview) {
            Ok(worker) => {
                self.active = Some(worker);
                None
            }
            Err(message) => Some(protocol::tool_error(id, modern, "worker_failed", message)),
        }
    }
    pub fn poll(&mut self) -> Option<Value> {
        let result = self
            .active
            .as_mut()
            .and_then(|worker| worker.poll(&mut self.previews, &mut self.config));
        if result.is_some() {
            drop(self.active.take());
        }
        result
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::{fs, path::PathBuf};
    fn fixture() -> (tempfile::TempDir, Session) {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.pdf");
        fs::write(&source, b"fixture").unwrap();
        let config = Config::load(vec![
            "--input".into(),
            source.into_os_string(),
            "--out-dir".into(),
            dir.path().as_os_str().to_owned(),
        ])
        .unwrap();
        (dir, Session::new(config))
    }
    fn preview(session: &mut Session) -> (String, PathBuf) {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("edit.json"), b"{}").unwrap();
        let bundle = directory.path().join("Folio preview-test");
        fs::create_dir(&bundle).unwrap();
        for name in ["candidate.pdf", "before.png", "after.png"] {
            fs::write(bundle.join(name), b"fixture").unwrap();
        }
        let digest = Sha256::digest(b"fixture")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let proof = json!({"bytes":7,"sha256":digest});
        let manifest = json!({"version":1,"sourceFingerprint":"a".repeat(64),"page":1,"selectionId":format!("text-v1-{}","a".repeat(64)),"oldText":"Old","replacement":"New","candidate":proof,"before":proof,"after":proof});
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let hash = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        fs::write(bundle.join("manifest.json"), bytes).unwrap();
        let result = session
            .previews
            .adopt(
                directory,
                json!({"bundleName":"Folio preview-test","manifestSha256":hash}),
            )
            .unwrap();
        (result["previewId"].as_str().unwrap().to_owned(), bundle)
    }
    fn owned_worker(
        session: &mut Session,
        kind: &str,
    ) -> (PathBuf, windows_sys::Win32::Foundation::HANDLE) {
        use windows_sys::Win32::System::Threading::OpenProcess;
        let stage = tempfile::tempdir().unwrap();
        let path = stage.path().to_owned();
        fs::write(path.join("partial"), b"unfinished").unwrap();
        let worker = Worker::sleeping_for_test(stage, kind);
        let process = unsafe { OpenProcess(0x00100000, 0, worker.process_id_for_test()) };
        assert!(!process.is_null());
        session.active = Some(worker);
        (path, process)
    }
    fn reaped(process: windows_sys::Win32::Foundation::HANDLE) {
        use windows_sys::Win32::{Foundation::CloseHandle, System::Threading::WaitForSingleObject};
        assert_eq!(unsafe { WaitForSingleObject(process, 1000) }, 0);
        unsafe { CloseHandle(process) };
    }
    #[test]
    fn preview_and_save_cancellation_reap_worker_before_expired_preview_cleanup() {
        for kind in ["preview", "save", "apply"] {
            let (_dir, mut session) = fixture();
            let (id, bundle) = preview(&mut session);
            session.previews.expire_for_test(&id);
            let (stage, process) = owned_worker(&mut session, kind);
            let fonts=session.active.as_ref().unwrap().font_paths_for_test();assert_eq!(fonts.len(),1);assert!(fonts[0].exists());
            session.prune();
            assert!(bundle.exists(), "Active worker pins expired preview");
            let busy = session
                .call(
                    json!(2),
                    true,
                    Call::DiscardPreview {
                        preview: id.clone(),
                    },
                )
                .unwrap();
            assert_eq!(busy["result"]["structuredContent"]["error"]["code"], "busy");
            let close = session
                .call(json!(3), true, Call::CloseDocument { document: 0 })
                .unwrap();
            assert_eq!(
                close["result"]["structuredContent"]["error"]["code"],
                "busy"
            );
            assert_eq!(session.config.documents.len(), 1);
            assert!(session.dispatch(json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}})).is_none());
            reaped(process);
            assert!(fonts.iter().all(|path|!path.exists()),"Cancellation releases owned font files after reaping");
            assert!(!stage.exists());
            assert!(bundle.exists());
            session.prune();
            assert!(!bundle.exists());
            assert!(session.active.is_none());
        }
    }
    #[test]
    fn session_drop_reaps_worker_and_releases_private_inputs_and_previews() {
        for kind in ["preview", "save", "apply"] {
            let (_dir, mut session) = fixture();
            let (_, bundle) = preview(&mut session);
            let input = session.config.documents[0].path.clone();
            let (stage, process) = owned_worker(&mut session, kind);
            let fonts=session.active.as_ref().unwrap().font_paths_for_test();assert_eq!(fonts.len(),1);assert!(fonts[0].exists());
            drop(session);
            reaped(process);
            assert!(fonts.iter().all(|path|!path.exists()),"Session drop releases owned font files after reaping");
            assert!(!stage.exists());
            assert!(!bundle.exists());
            assert!(!input.exists());
        }
    }
}
