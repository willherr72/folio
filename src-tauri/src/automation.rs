//! Saved-file automation using the desktop engine. Not a network authorization boundary.
mod inspection;
mod original_snapshot;
pub(crate) mod font_inputs;
pub(crate) mod nested_inspection;
pub(crate) mod form_automation;
pub(crate) mod paragraph_preview;
pub(crate) mod paragraph_inspection;
pub(crate) mod text_preview;
use crate::{
    DocumentInfo, EngineError, EngineResult, PageImageControl, PageImageExportRequest,
    PageImageFormat, PageImageOptions, PagePlan, PdfEngine, TextExportControl, TextExportRequest,
};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

pub enum Request {
    InspectNestedText {input:PathBuf,page:u32,offset:usize},
    InspectForms {input:PathBuf,page:u32,offset:usize},
    PreviewForm {input:PathBuf,request_file:PathBuf,parent:PathBuf},
    PreviewParagraph {input:PathBuf,request_file:PathBuf,parent:PathBuf},
    InspectParagraph {input:PathBuf,page:u32,selection_id:String},
    PreviewText {input:PathBuf,request_file:PathBuf,parent:PathBuf},
    SaveTextPreview {input:PathBuf,manifest_hash:String,output:PathBuf},
    InspectText {
        input: PathBuf,
        page: u32,
        offset: usize,
    },
    Info {
        input: PathBuf,
    },
    Text {
        input: PathBuf,
        output: PathBuf,
        page: Option<u32>,
        include_form_values: bool,
    },
    Render {
        input: PathBuf,
        parent: PathBuf,
        page: Option<u32>,
        dpi: u32,
    },
}
impl Request {
    pub fn command(&self) -> &'static str {
        match self {
            Self::InspectNestedText {..}=>"inspect-nested-text",
            Self::InspectForms {..}=>"inspect-forms",
            Self::PreviewForm {..}=>"preview-form",
            Self::InspectParagraph {..}=>"inspect-paragraph",
            Self::PreviewParagraph {..}=>"preview-paragraph",
            Self::PreviewText {..}=>"preview-text",
            Self::SaveTextPreview {..}=>"save-text-preview",
            Self::InspectText { .. } => "inspect-text",
            Self::Info { .. } => "info",
            Self::Text { .. } => "text",
            Self::Render { .. } => "render",
        }
    }
    fn input(&self) -> &Path {
        match self {
            Self::InspectNestedText {input,..} | Self::InspectForms {input,..} | Self::PreviewForm {input,..} | Self::Info { input } | Self::Text { input, .. } | Self::Render { input, .. }
            | Self::InspectText { input, .. } | Self::InspectParagraph {input,..} => input,
            Self::PreviewParagraph {input,..}|Self::PreviewText {input,..}|Self::SaveTextPreview {input,..}=>input,
        }
    }
}

struct Source<'a> {
    engine: &'a PdfEngine,
    info: DocumentInfo,
}
impl Drop for Source<'_> {
    fn drop(&mut self) {
        let _ = self.engine.close_document(&self.info.id);
    }
}
fn invalid(message: &str) -> EngineError {
    EngineError::InvalidRequest(message.into())
}
fn io(error: impl std::fmt::Display) -> EngineError {
    EngineError::Io(error.to_string())
}
fn selected(info: &DocumentInfo, page: Option<u32>) -> EngineResult<Vec<PagePlan>> {
    if info.pages.is_empty() || info.pages.len() > 10_000 {
        return Err(invalid("Choose a PDF with 1 to 10,000 pages."));
    }
    let range = match page {
        Some(p) if p > 0 && (p as usize) <= info.pages.len() => p as usize - 1..p as usize,
        Some(_) => return Err(invalid("The requested page is outside this PDF.")),
        None => 0..info.pages.len(),
    };
    Ok(range
        .map(|i| {
            let p = &info.pages[i];
            PagePlan {
                id: format!("page-{}", i + 1),
                source_id: info.id.clone(),
                page_index: i,
                width: p.width,
                height: p.height,
                rotation: 0,
                crop: None,
                overlays: p.overlays.clone(),
            }
        })
        .collect())
}

/// Each request owns its document until export has finished, including error exits.
/// Paths must be authorized by the caller; this API supplies no MCP root confinement.
pub fn execute(
    engine: &PdfEngine,
    request: Request,
    password: Option<&str>,
) -> EngineResult<Value> {
    execute_with_fonts(engine, request, password, &font_inputs::FontInputs::default())
}

pub(crate) fn execute_with_fonts(
    engine: &PdfEngine,
    request: Request,
    password: Option<&str>,
    fonts: &font_inputs::FontInputs,
) -> EngineResult<Value> {
    if let Request::SaveTextPreview {input,manifest_hash,output}=&request {
        return text_preview::save(input,manifest_hash,output);
    }
    // Keep the exact input (including ciphertext) for a validated no-op. Native
    // opening and eventual no-op publication use the same private snapshot.
    let original = if matches!(&request, Request::PreviewText { .. } | Request::PreviewParagraph { .. }) {
        Some(original_snapshot::OriginalSnapshot::capture(request.input())?)
    } else {
        None
    };
    let input = original.as_ref().map(|snapshot| snapshot.path()).unwrap_or_else(|| request.input());
    let info = match password {
        Some(p) => engine.open_document_with_password(input, p)?,
        None => engine.open_document(input)?,
    };
    let source = Source { engine, info };
    let info = &source.info;
    if info.pages.is_empty() || info.pages.len() > 10_000 {
        return Err(invalid("Choose a PDF with 1 to 10,000 pages."));
    }
    match request {
        Request::InspectNestedText {page,offset,..}=>nested_inspection::inspect(engine,info,page,offset),
        Request::InspectForms {page,offset,..}=>form_automation::inspect(engine,info,page,offset),
        Request::PreviewForm {request_file,parent,..}=>form_automation::preview(engine,info,form_automation::FormEdit::read(&request_file)?,&parent,password),
        Request::InspectParagraph {page,selection_id,..}=>paragraph_inspection::inspect(engine,info,page,&selection_id),
        Request::PreviewParagraph {request_file,parent,..}=>paragraph_preview::preview(engine,info,paragraph_preview::ParagraphEdit::read(&request_file)?,&parent,password,original.as_ref().ok_or_else(||invalid("Missing immutable preview input."))?,fonts),
        Request::PreviewText {request_file,parent,..}=>text_preview::preview(engine,info,text_preview::TextEdit::read(&request_file)?,&parent,password,original.as_ref().ok_or_else(||invalid("Missing immutable preview input."))?,fonts),
        Request::SaveTextPreview {..}=>unreachable!(),
        Request::InspectText { page, offset, .. } => inspection::inspect(engine, info, page, offset),
        Request::Info { .. } => Ok(json!({"name":info.name,"pageCount":info.pages.len(),
            "pages":info.pages.iter().enumerate().map(|(i,p)|json!({"page":i+1,"width":p.width,"height":p.height})).collect::<Vec<_>>(),
            "protection":info.protection.as_ref().map(|p|json!({"owner":p.owner,"revision":p.revision,"permissions":p.permissions}))})),
        Request::Text {
            output,
            page,
            include_form_values,
            ..
        } => {
            let pages = selected(info, page)?;
            if !output
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("txt"))
            {
                return Err(invalid("Text output must use the .txt extension."));
            }
            ensure_absent(&output)?;
            let parent = output
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            let parent = parent.canonicalize().map_err(io)?;
            let name = output
                .file_name()
                .ok_or_else(|| invalid("Choose a text output filename."))?;
            let output = parent.join(name);
            // Export through the same crop/OCR/forms/permission path as desktop. Publication
            // to the requested name is separately exclusive, including a late-created target.
            let staging = tempfile::Builder::new()
                .prefix(".folio-cli-")
                .tempdir_in(&parent)
                .map_err(io)?;
            let path = staging.path().join("export.txt");
            let result = engine
                .export_text(
                    TextExportRequest {
                        pages,
                        include_form_values,
                    },
                    &path,
                    Arc::new(TextExportControl::new()),
                    Arc::new(|_| {}),
                )?
                .ok_or_else(|| invalid("Text export was cancelled."))?;
            let response = path_result(
                json!({"pageCount":result.page_count,"emptyPages":result.empty_pages,
                    "skippedFields":result.skipped_fields,"bytes":result.bytes}),
                "path",
                &output,
            );
            publish_new_text(&path, &output)?;
            Ok(response)
        }
        Request::Render {
            parent, page, dpi, ..
        } => {
            let pages = selected(info, page)?;
            let page_numbers = pages.iter().map(|p| p.page_index as u32 + 1).collect();
            let result = engine
                .export_page_images(
                    PageImageExportRequest {
                        pages,
                        page_numbers,
                        workspace_page_count: info.pages.len() as u32,
                        options: PageImageOptions {
                            format: PageImageFormat::Png,
                            dpi,
                            jpeg_quality: 90,
                        },
                    },
                    parent,
                    Arc::new(PageImageControl::new()),
                    Arc::new(|_| {}),
                )?
                .ok_or_else(|| invalid("Image export was cancelled."))?;
            Ok(path_result(
                json!({"count":result.count}),
                "folder",
                &result.folder,
            ))
        }
    }
}
// Path's Serde implementation rejects native non-Unicode names. Never hand a Path
// to json!: a successful export must not then panic while describing its location.
pub(crate) fn path_result(mut result: Value, field: &str, path: &Path) -> Value {
    if let Some(text) = path.to_str() {
        result[field] = Value::String(text.to_owned());
    } else {
        result[field] = Value::Null;
        #[cfg(windows)]
        let native = {
            use std::os::windows::ffi::OsStrExt;
            json!({"encoding":"windows-utf16", "units":path.as_os_str().encode_wide().collect::<Vec<u16>>()})
        };
        #[cfg(unix)]
        let native = {
            use std::os::unix::ffi::OsStrExt;
            json!({"encoding":"unix-bytes", "bytes":path.as_os_str().as_bytes()})
        };
        #[cfg(not(any(windows, unix)))]
        let native = json!({"encoding":"platform-encoded-bytes", "bytes":path.as_os_str().as_encoded_bytes()});
        result[format!("{field}Native")] = native;
    }
    result
}
fn ensure_absent(path: &Path) -> EngineResult<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(invalid("The output already exists. Choose a new filename.")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io(e)),
    }
}
fn publish_new_text(staged: &Path, output: &Path) -> EngineResult<()> {
    // Moving the staged file on the same filesystem avoids a second plaintext buffer.
    // TempPath's no-clobber primitive, not the preflight check, arbitrates publication.
    let staged = tempfile::TempPath::try_from_path(staged).map_err(io)?;
    staged.persist_noclobber(output).map_err(|e| {
        if e.error.kind() == std::io::ErrorKind::AlreadyExists {
            invalid("The output already exists. Choose a new filename.")
        } else {
            io(&e.error)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_destination_creation_cannot_overwrite_data() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("output.txt");
        let staged = dir.path().join("staged.txt");
        fs::write(&staged, b"new").unwrap();
        ensure_absent(&output).unwrap();
        fs::write(&output, b"keep").unwrap();
        assert!(publish_new_text(&staged, &output).is_err());
        assert_eq!(fs::read(output).unwrap(), b"keep");
    }
    #[test]
    fn source_guard_releases_the_document_on_error() {
        let engine = PdfEngine::start(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
        )
        .unwrap();
        let info = engine.create_blank_document(200., 200.).unwrap();
        let id = info.id.clone();
        let result = (|| -> EngineResult<()> {
            let _source = Source {
                engine: &engine,
                info,
            };
            Err(invalid("failed operation"))
        })();
        assert!(result.is_err());
        assert!(engine.source_bytes(&id).is_err());
    }
}
