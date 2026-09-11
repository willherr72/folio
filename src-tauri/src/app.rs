use crate::printing::{print_document, PrintOptions};
use crate::{
    DocumentInfo, EngineError, ExportRequest, PageText, PdfEngine, RecoveryStore, TextRuns,
};
use std::path::PathBuf;
use std::sync::Arc;
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
async fn list_installed_fonts(
    engine: State<'_, PdfEngine>,
) -> Result<Vec<crate::InstalledFont>, String> {
    let fonts = engine.fonts();
    on_worker(move || fonts.list_installed()).await
}
#[tauri::command]
async fn load_installed_font(
    engine: State<'_, PdfEngine>,
    id: String,
) -> Result<crate::FontInfo, String> {
    let fonts = engine.fonts();
    on_worker(move || fonts.load_installed(&id)).await
}
#[tauri::command]
async fn font_info(engine: State<'_, PdfEngine>, id: String) -> Result<crate::FontInfo, String> {
    let fonts = engine.fonts();
    on_worker(move || Ok(fonts.get(&id)?.info.clone())).await
}
#[tauri::command]
async fn font_bytes(engine: State<'_, PdfEngine>, id: String) -> Result<Response, String> {
    let fonts = engine.fonts();
    on_worker(move || Ok(fonts.get(&id)?.bytes.to_vec()))
        .await
        .map(Response::new)
}
#[tauri::command]
async fn release_font(engine: State<'_, PdfEngine>, id: String) -> Result<(), String> {
    let fonts = engine.fonts();
    on_worker(move || fonts.remove(&id)).await
}
#[tauri::command]
async fn import_font(engine: State<'_, PdfEngine>) -> Result<Option<crate::FontInfo>, String> {
    use std::io::Read;
    let fonts = engine.fonts();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import a font")
            .add_filter("Font files", &["ttf", "otf"])
            .pick_file()
        else {
            return Ok(None);
        };
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|error| error.to_string())?
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        fonts
            .register(bytes)
            .map(Some)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Font dialog failed: {error}"))?
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
async fn page_text(
    engine: State<'_, PdfEngine>,
    source_id: String,
    page_index: usize,
) -> Result<PageText, String> {
    let engine = engine.inner().clone();
    on_worker(move || engine.page_text(&source_id, page_index)).await
}

#[tauri::command]
async fn list_text_runs(
    engine: State<'_, PdfEngine>,
    source_id: String,
    page_index: usize,
) -> Result<TextRuns, String> {
    let engine = engine.inner().clone();
    on_worker(move || engine.list_text_runs(&source_id, page_index)).await
}

#[tauri::command]
async fn replace_text(
    engine: State<'_, PdfEngine>,
    source_id: String,
    page_index: usize,
    object_index: usize,
    expected_text: String,
    replacement: String,
    font_id: Option<String>,
) -> Result<DocumentInfo, String> {
    let engine = engine.inner().clone();
    on_worker(move || {
        engine.replace_text_with_font(
            &source_id,
            page_index,
            object_index,
            &expected_text,
            &replacement,
            font_id.as_deref(),
        )
    })
    .await
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

struct RecoveryState(Result<Arc<RecoveryStore>, String>);
#[tauri::command]
async fn save_recovery(
    engine: State<'_, PdfEngine>,
    recovery: State<'_, RecoveryState>,
    workspace: serde_json::Value,
) -> Result<(), String> {
    let store = recovery.0.as_ref().map_err(Clone::clone)?.clone();
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || store.save(&engine, workspace))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn load_recovery(
    engine: State<'_, PdfEngine>,
    recovery: State<'_, RecoveryState>,
) -> Result<Option<serde_json::Value>, String> {
    let store = recovery.0.as_ref().map_err(Clone::clone)?.clone();
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || store.load(&engine))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn clear_recovery(recovery: State<'_, RecoveryState>) -> Result<(), String> {
    let store = recovery.0.as_ref().map_err(Clone::clone)?.clone();
    tauri::async_runtime::spawn_blocking(move || store.clear())
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn print_pdf(
    engine: State<'_, PdfEngine>,
    request: ExportRequest,
    options: PrintOptions,
) -> Result<bool, String> {
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || print_document(engine, request, options))
        .await
        .map_err(|e| format!("Print task failed: {e}"))?
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
            let recovery_root = std::env::var_os("FOLIO_DATA_DIR")
                .map(PathBuf::from)
                .map(Ok)
                .unwrap_or_else(|| app.path().app_local_data_dir().map_err(|e| e.to_string()))
                .map(|root| root.join("recovery"));
            app.manage(RecoveryState(
                recovery_root.and_then(RecoveryStore::new).map(Arc::new),
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_pdf,
            list_installed_fonts,
            load_installed_font,
            import_font,
            font_info,
            font_bytes,
            release_font,
            render_page,
            page_text,
            list_text_runs,
            replace_text,
            export_pdf,
            close_document,
            engine_status,
            save_recovery,
            load_recovery,
            clear_recovery,
            print_pdf
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
