use crate::{DocumentInfo, EngineError, ExportRequest, PdfEngine};
use std::path::PathBuf;
use tauri::{ipc::Response, Manager, State};

async fn on_worker<T, F>(work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, EngineError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| format!("Background PDF task failed: {error}"))?
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn open_pdf(engine: State<'_, PdfEngine>) -> Result<Option<DocumentInfo>, String> {
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let path = rfd::FileDialog::new()
            .set_title("Open a PDF")
            .add_filter("PDF documents", &["pdf"])
            .pick_file();
        path.map(|path| {
            engine
                .open_document(path)
                .map_err(|error| error.to_string())
        })
        .transpose()
    })
    .await
    .map_err(|error| format!("Open dialog failed: {error}"))?
}

#[tauri::command]
async fn render_page(
    engine: State<'_, PdfEngine>,
    source_id: String,
    page_index: usize,
    width: u32,
) -> Result<Response, String> {
    let engine = engine.inner().clone();
    on_worker(move || engine.render_page(&source_id, page_index, width))
        .await
        .map(Response::new)
}

#[tauri::command]
async fn export_pdf(
    engine: State<'_, PdfEngine>,
    request: ExportRequest,
) -> Result<Option<String>, String> {
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let path = rfd::FileDialog::new()
            .set_title("Save a PDF copy")
            .set_file_name("Folio edited.pdf")
            .add_filter("PDF documents", &["pdf"])
            .save_file();
        match path {
            Some(mut path) => {
                if path.extension().is_none() {
                    path.set_extension("pdf");
                }
                engine
                    .export_pdf(request, &path)
                    .map_err(|error| error.to_string())?;
                Ok(Some(path.to_string_lossy().into_owned()))
            }
            None => Ok(None),
        }
    })
    .await
    .map_err(|error| format!("Save dialog failed: {error}"))?
}

#[tauri::command]
async fn close_document(engine: State<'_, PdfEngine>, source_id: String) -> Result<(), String> {
    let engine = engine.inner().clone();
    on_worker(move || engine.close_document(&source_id)).await
}

#[tauri::command]
async fn engine_status(engine: State<'_, PdfEngine>) -> Result<String, String> {
    let engine = engine.inner().clone();
    on_worker(move || engine.status()).await
}

fn engine_path(app: &tauri::App) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let library = if cfg!(target_os = "windows") {
        "pdfium.dll"
    } else if cfg!(target_os = "macos") {
        "libpdfium.dylib"
    } else {
        "libpdfium.so"
    };
    let relative = PathBuf::from("resources").join("pdfium").join(library);
    let mut candidates = vec![app.path().resource_dir()?.join(&relative)];
    if let Some(directory) = std::env::current_exe()?.parent() {
        candidates.push(directory.join(&relative));
    }
    if cfg!(debug_assertions) {
        candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&relative));
    }
    candidates.into_iter().find(|path| path.is_file()).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "The PDF engine is missing. Keep resources/pdfium beside Folio.exe, or run scripts/fetch-pdfium.ps1 before a development build.",
        )
        .into()
    })
}

pub fn run() {
    let result = tauri::Builder::default()
        .setup(|app| {
            let engine = PdfEngine::start(engine_path(app)?)?;
            app.manage(engine);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_pdf,
            render_page,
            export_pdf,
            close_document,
            engine_status
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("Folio could not start: {error}");
        rfd::MessageDialog::new()
            .set_title("Folio could not start")
            .set_description(error.to_string())
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}
