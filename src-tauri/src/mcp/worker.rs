//! One owned CLI process per operation; PDFium state dies with that worker.
use super::{
    config::Config,
    previews::{Preview, PreviewStore},
    protocol::{self, Call},
};
use crate::automation::text_preview::FileProof;
use serde_json::{json, Value};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use tempfile::TempDir;

const MAX_REPLY: u64 = 16 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(120);
enum Kind {
    Info,
    InspectText,
    InspectNestedText,
    InspectParagraph,
    InspectForms,
    PreviewForm,
    Text,
    Render,
    Preview,
    PreviewParagraph,
    Save,
    Apply,
}
pub(super) struct Worker {
    pub id: Value,
    modern: bool,
    child: Child,
    stdout: File,
    stderr: File,
    directory: Option<TempDir>,
    started: Instant,
    kind: Kind,
    name: String,
    candidate: Option<FileProof>,
    // Drop runs after stop() has killed/reaped the child, including cancellation.
    _font_files: Vec<tempfile::NamedTempFile>,
}
impl Worker {
    #[cfg(all(test, windows))]
    pub(super) fn sleeping_for_test(directory: TempDir, kind: &str) -> Self {
        use std::os::windows::process::CommandExt;
        let mut font=super::config::font_snapshot().unwrap();
        font.write_all(b"owned test font input").unwrap();
        let child = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 30",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        Self {
            id: json!(1),
            modern: true,
            child,
            stdout: tempfile::tempfile().unwrap(),
            stderr: tempfile::tempfile().unwrap(),
            directory: Some(directory),
            started: Instant::now(),
            kind: match kind {
                "preview" => Kind::Preview,
                "save" => Kind::Save,
                "apply" => Kind::Apply,
                _ => panic!("Unknown test operation"),
            },
            name: "test".into(),
            candidate: None,
            _font_files: vec![font],
        }
    }
    #[cfg(all(test, windows))]
    pub(super) fn process_id_for_test(&self) -> u32 {
        self.child.id()
    }
    #[cfg(all(test, windows))]
    pub(super) fn font_paths_for_test(&self) -> Vec<std::path::PathBuf> {
        self._font_files.iter().map(|file|file.path().to_owned()).collect()
    }
    pub fn start(
        config: &Config,
        id: Value,
        modern: bool,
        call: Call,
        preview: Option<&Preview>,
    ) -> Result<Self, &'static str> {
        let (document, kind) = match &call {
            Call::InspectForms {document,..}=>(*document,Kind::InspectForms),
            Call::PreviewForm {document,..}=>(*document,Kind::PreviewForm),
            Call::Info { document } => (*document, Kind::Info),
            Call::InspectParagraph {document,..} => (*document,Kind::InspectParagraph),
            Call::InspectNestedText {document,..}=>(*document,Kind::InspectNestedText),
            Call::InspectText { document, .. } => (*document, Kind::InspectText),
            Call::Text { document, .. } => (*document, Kind::Text),
            Call::Render { document, .. } => (*document, Kind::Render),
            Call::PreviewParagraph {document,..} => (*document,Kind::PreviewParagraph),
            Call::PreviewText { document, .. } => (*document, Kind::Preview),
            Call::SavePreview { .. } => (0, Kind::Save),
            Call::ApplyPreview { .. } => (0, Kind::Apply),
            Call::List | Call::ListFonts | Call::DiscardPreview { .. } | Call::CloseDocument { .. } => {
                return Err("This operation does not require a worker.")
            }
        };
        let mut font_assets=std::collections::BTreeMap::new();
        if let Call::PreviewText {edit,..}=&call {
            if let Some(id)=&edit.font_id {
                let asset=config.fonts.get(id).map_err(|_|"Unknown authorized font ID. Use folio_list_fonts; font files must be configured at launch.")?;
                font_assets.insert(id.clone(),asset);
            }
        }
        if let Call::PreviewParagraph {edit,..}=&call {
            for change in &edit.font_overrides {
                let asset=config.fonts.get(&change.font_id).map_err(|_|"Unknown authorized font ID. Use folio_list_fonts; font files must be configured at launch.")?;
                font_assets.insert(change.font_id.clone(),asset);
            }
        }
        let (input, name) = if matches!(kind, Kind::Save | Kind::Apply) {
            (
                preview
                    .ok_or("This preview is unavailable or expired.")?
                    .bundle
                    .as_path(),
                String::new(),
            )
        } else {
            let document = &config.documents[document];
            (document.path.as_path(), document.name.clone())
        };
        let directory = if matches!(
            kind,
            Kind::Info | Kind::InspectForms | Kind::PreviewForm | Kind::InspectText | Kind::InspectNestedText | Kind::InspectParagraph | Kind::Preview | Kind::PreviewParagraph | Kind::Apply
        ) {
            tempfile::tempdir()
        } else {
            tempfile::Builder::new()
                .prefix("Folio export-")
                .tempdir_in(&config.output)
        }
        .map_err(|_| "Cannot create a private operation folder.")?;
        let mut command = Command::new(&config.executable);
        let verb = match kind {
            Kind::Info => "info",
            Kind::InspectForms => "inspect-forms",
            Kind::PreviewForm => "preview-form",
            Kind::InspectText => "inspect-text",
            Kind::InspectNestedText => "inspect-nested-text",
            Kind::InspectParagraph => "inspect-paragraph",
            Kind::Text => "text",
            Kind::Render => "render",
            Kind::Preview => "preview-text",
            Kind::PreviewParagraph => "preview-paragraph",
            Kind::Save | Kind::Apply => "save-text-preview",
        };
        command
            .arg(verb)
            .arg(input)
            .arg("--engine")
            .arg(&config.engine);
        let mut font_files=Vec::new();
        for asset in font_assets.values() {
            let mut file=super::config::font_snapshot().map_err(|_|"Cannot create a private font input.")?;
            file.write_all(&asset.bytes).and_then(|_|file.as_file().sync_all()).map_err(|_|"Cannot store a private font input.")?;
            command.arg("--font").arg(file.path());font_files.push(file);
        }
        match call {
            Call::InspectForms {page,offset,..}=>{command.arg("--page").arg(page.to_string()).arg("--offset").arg(offset.to_string());}
            Call::PreviewForm {edit,..}=>{
                let request=directory.path().join("edit.json");
                let bytes=serde_json::to_vec(&edit).map_err(|_|"Cannot serialize the form request.")?;
                std::fs::write(&request,bytes).map_err(|_|"Cannot store the form request.")?;
                command.arg("--request").arg(request).arg("--out-dir").arg(directory.path());
            }
            Call::PreviewParagraph {edit,..} => {
                let request=directory.path().join("edit.json");
                let bytes=serde_json::to_vec(&edit).map_err(|_|"Cannot serialize the paragraph request.")?;
                std::fs::write(&request,bytes).map_err(|_|"Cannot store the paragraph request.")?;
                command.arg("--request").arg(request).arg("--out-dir").arg(directory.path());
            }
            Call::PreviewText { edit, .. } => {
                let request = directory.path().join("edit.json");
                let bytes =
                    serde_json::to_vec(&edit).map_err(|_| "Cannot serialize the edit request.")?;
                std::fs::write(&request, bytes).map_err(|_| "Cannot store the edit request.")?;
                command
                    .arg("--request")
                    .arg(request)
                    .arg("--out-dir")
                    .arg(directory.path());
            }
            Call::SavePreview { .. } | Call::ApplyPreview { .. } => {
                command
                    .arg("--manifest-sha256")
                    .arg(&preview.unwrap().manifest_hash)
                    .arg("--out")
                    .arg(directory.path().join("Edited.pdf"));
            }
            Call::InspectParagraph {page,selection_id,..} => {
                command.arg("--page").arg(page.to_string()).arg("--selection-id").arg(selection_id);
            }
            Call::InspectText { page, offset, .. } | Call::InspectNestedText {page,offset,..} => {
                command
                    .arg("--page")
                    .arg(page.to_string())
                    .arg("--offset")
                    .arg(offset.to_string());
            }
            Call::Text {
                page,
                include_form_values,
                ..
            } => {
                command.arg("--out").arg(directory.path().join("text.txt"));
                if let Some(page) = page {
                    command.arg("--page").arg(page.to_string());
                }
                if include_form_values {
                    command.arg("--include-form-values");
                }
            }
            Call::Render { page, dpi, .. } => {
                command
                    .arg("--out-dir")
                    .arg(directory.path())
                    .arg("--page")
                    .arg(page.to_string())
                    .arg("--dpi")
                    .arg(dpi.to_string());
            }
            _ => {}
        }
        let stdout = tempfile::tempfile().map_err(|_| "Cannot create worker output storage.")?;
        let stderr = tempfile::tempfile().map_err(|_| "Cannot create worker error storage.")?;
        command
            .stdin(Stdio::null())
            .stdout(
                stdout
                    .try_clone()
                    .map_err(|_| "Cannot connect worker output.")?,
            )
            .stderr(
                stderr
                    .try_clone()
                    .map_err(|_| "Cannot connect worker errors.")?,
            )
            .current_dir(directory.path());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let child = command
            .spawn()
            .map_err(|_| "Cannot start the Folio PDF worker.")?;
        Ok(Self {
            id,
            modern,
            child,
            stdout,
            stderr,
            directory: Some(directory),
            started: Instant::now(),
            kind,
            name,
            candidate: preview.map(|p| p.candidate.clone()),
            _font_files: font_files,
        })
    }
    fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
    fn failure(&self, code: &str, message: &str) -> Value {
        protocol::tool_error(self.id.clone(), self.modern, code, message)
    }
    pub fn poll(&mut self, previews: &mut PreviewStore, config: &mut Config) -> Option<Value> {
        if self.started.elapsed() > DEADLINE {
            self.stop();
            return Some(self.failure("timeout", "PDF operation exceeded the 120-second limit."));
        }
        let oversized = [&self.stdout, &self.stderr]
            .iter()
            .any(|file| file.metadata().map(|m| m.len() > MAX_REPLY).unwrap_or(true));
        if oversized {
            self.stop();
            return Some(self.failure("limit_exceeded", "PDF worker output exceeded its limit."));
        }
        match self.child.try_wait() {
            Ok(None) => None,
            Err(_) => {
                self.stop();
                Some(self.failure("worker_failed", "Cannot wait for the PDF worker."))
            }
            Ok(Some(status)) => {
                let parsed = read_json(if status.success() {
                    &mut self.stdout
                } else {
                    &mut self.stderr
                });
                let Ok(value) = parsed else {
                    return Some(self.failure(
                        "worker_failed",
                        "The PDF worker did not return a valid bounded result.",
                    ));
                };
                if !status.success() || value["ok"] != true {
                    let code = value["error"]["code"].as_str().unwrap_or("worker_failed");
                    let (code, message) = safe_error(code);
                    return Some(self.failure(code, message));
                }
                let mut result = value["result"].clone();
                if !result.is_object() {
                    return Some(self.failure("worker_failed", "Invalid PDF worker result."));
                }
                match self.kind {
                    Kind::Info => result["name"] = json!(self.name),
                    Kind::Text => {
                        match excerpt(&self.directory.as_ref().unwrap().path().join("text.txt")) {
                            Ok((text, truncated)) => {
                                result["text"] = json!(text);
                                result["truncated"] = json!(truncated);
                            }
                            Err(_) => {
                                return Some(
                                    self.failure(
                                        "io_error",
                                        "Cannot read the exported text excerpt.",
                                    ),
                                )
                            }
                        }
                    }
                    Kind::Preview | Kind::PreviewParagraph | Kind::PreviewForm => match previews.adopt(self.directory.take().unwrap(), result) {
                        Ok(adopted) => result = adopted,
                        Err(message) => return Some(self.failure("preview_rejected", message)),
                    },
                    Kind::Apply => {
                        let proof = self.candidate.as_ref().unwrap();
                        if result["bytes"].as_u64() != Some(proof.bytes)
                            || result["sha256"].as_str() != Some(proof.sha256.as_str())
                        {
                            return Some(self.failure(
                                "worker_failed",
                                "The applied document did not match its validated preview.",
                            ));
                        }
                        match config.adopt(self.directory.take().unwrap(), proof) {
                            Ok(adopted) => result = adopted,
                            Err(message) => {
                                return Some(self.failure("document_rejected", message))
                            }
                        }
                    }
                    Kind::Render | Kind::InspectText | Kind::InspectNestedText | Kind::InspectParagraph | Kind::InspectForms | Kind::Save => {}
                }
                // The response is composed before keeping any exported files. The CLI
                // already uses lossless native-path JSON rather than serializing PathBuf.
                let response = if matches!(self.kind,Kind::InspectParagraph) {
                    protocol::paragraph_result(self.id.clone(), self.modern, result)
                } else if matches!(self.kind,Kind::InspectNestedText) {protocol::nested_result(self.id.clone(),self.modern,result)} else if matches!(self.kind,Kind::InspectForms) {protocol::forms_result(self.id.clone(),self.modern,result)} else {protocol::tool_result(self.id.clone(), self.modern, result, false)};
                if matches!(self.kind, Kind::Text | Kind::Render | Kind::Save) {
                    let _ = self.directory.take().unwrap().keep();
                }
                Some(response)
            }
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop();
    }
}
fn read_json(file: &mut File) -> Result<Value, ()> {
    file.seek(SeekFrom::Start(0)).map_err(|_| ())?;
    let mut data = Vec::new();
    file.take(MAX_REPLY + 1)
        .read_to_end(&mut data)
        .map_err(|_| ())?;
    if data.len() as u64 > MAX_REPLY {
        return Err(());
    }
    serde_json::from_slice(&data).map_err(|_| ())
}
fn excerpt(path: &std::path::Path) -> std::io::Result<(String, bool)> {
    let file = File::open(path)?;
    let truncated = file.metadata()?.len() > 65536;
    let mut bytes = Vec::new();
    file.take(65536).read_to_end(&mut bytes)?;
    if let Err(e) = std::str::from_utf8(&bytes) {
        if e.error_len().is_some() {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        bytes.truncate(e.valid_up_to());
    }
    let text = String::from_utf8(bytes).map_err(|_| std::io::ErrorKind::InvalidData)?;
    Ok((
        text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned(),
        truncated,
    ))
}
fn safe_error(code: &str) -> (&'static str, &'static str) {
    match code {
        "password_required" => (
            "password_required",
            "This PDF requires a password. MCP password entry is not supported yet.",
        ),
        "permission_denied" => (
            "permission_denied",
            "The PDF does not permit this operation.",
        ),
        "unsupported_protection" => (
            "unsupported_protection",
            "This PDF protection is not supported.",
        ),
        "engine_unavailable" => (
            "engine_unavailable",
            "Cannot load PDFium. Configure --engine or package the renderer beside Folio.",
        ),
        "invalid_document" => ("invalid_document", "The PDF could not be processed."),
        "invalid_request" => (
            "invalid_request",
            "The PDF operation was rejected. Check page selection and document limits.",
        ),
        "limit_exceeded" => ("limit_exceeded", "The PDF exceeds an operation limit."),
        "io_error" => (
            "io_error",
            "The file operation failed. Check access permissions and available space.",
        ),
        _ => (
            "worker_failed",
            "The PDF worker could not complete this operation.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn excerpt_preserves_utf8_at_the_exact_byte_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("text.txt");
        let text = format!("{}é", "x".repeat(65535));
        std::fs::write(&path, text).unwrap();
        let (text, truncated) = excerpt(&path).unwrap();
        assert!(truncated);
        assert_eq!(text, "x".repeat(65535));
    }

    #[test]
    fn worker_json_has_a_hard_byte_limit() {
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(b"{}").unwrap();
        assert_eq!(read_json(&mut file).unwrap(), json!({}));
        file.set_len(MAX_REPLY + 1).unwrap();
        assert!(read_json(&mut file).is_err());
    }

    #[cfg(windows)]
    fn sleeping_worker() -> Worker {
        use std::os::windows::process::CommandExt;
        let child = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 30",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        Worker {
            id: json!(1),
            modern: true,
            child,
            stdout: tempfile::tempfile().unwrap(),
            stderr: tempfile::tempfile().unwrap(),
            directory: Some(tempfile::tempdir().unwrap()),
            started: Instant::now(),
            kind: Kind::Info,
            name: "test".into(),
            candidate: None,
            _font_files: vec![],
        }
    }

    #[cfg(windows)]
    #[test]
    fn deadline_reaps_the_owned_process() {
        let mut worker = sleeping_worker();
        worker.started = Instant::now() - DEADLINE - Duration::from_secs(1);
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.pdf");
        std::fs::write(&source, b"source").unwrap();
        let mut config = Config::load(vec![
            "--input".into(),
            source.into_os_string(),
            "--out-dir".into(),
            dir.path().as_os_str().to_owned(),
        ])
        .unwrap();
        let response = worker
            .poll(&mut PreviewStore::default(), &mut config)
            .unwrap();
        assert_eq!(
            response["result"]["structuredContent"]["error"]["code"],
            "timeout"
        );
        assert!(worker.child.try_wait().unwrap().is_some());
    }

    #[cfg(windows)]
    #[test]
    fn dropping_worker_stops_process_and_cleans_its_stage() {
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            System::Threading::{OpenProcess, WaitForSingleObject},
        };
        let worker = sleeping_worker();
        let path = worker.directory.as_ref().unwrap().path().to_owned();
        let process = unsafe { OpenProcess(0x00100000, 0, worker.child.id()) };
        assert!(!process.is_null());
        drop(worker);
        let wait = unsafe { WaitForSingleObject(process, 1000) };
        unsafe { CloseHandle(process) };
        assert_eq!(wait, 0, "Worker must have terminated");
        assert!(!path.exists());
    }
}
