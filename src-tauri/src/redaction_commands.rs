//! Source-only desktop transport. Composition and visible editor entry are separate.
use crate::engine::{
    TextRedactionBounds, TextRedactionMark, TextRedactionPreview, TextRedactionSource,ContentRedactionPreview,ContentRedactionRaster,
};
use crate::redaction_host::{RedactionHost, WindowHandle};
use crate::DocumentInfo;
use tauri::{State, WebviewWindow};

async fn background<T: Send + 'static>(
    window: WindowHandle,
    work: impl FnOnce(WindowHandle) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || work(window))
        .await
        .map_err(|e| format!("Text-removal task failed: {e}"))?
}

#[tauri::command]
pub(crate) async fn preview_source_content_removal(
    window:WebviewWindow,host:State<'_,RedactionHost>,session_id:String,source_id:String,marks:Vec<TextRedactionMark>,
) -> Result<ContentRedactionPreview,String> {
    let owned=host.capture(window.label())?;
    background(owned,move |owned|owned.preview_content(&session_id,&source_id,&marks)).await
}
#[tauri::command]
pub(crate) async fn render_content_removal(
    window:WebviewWindow,host:State<'_,RedactionHost>,session_id:String,source_id:String,preview_id:Option<String>,page_index:usize,width:u32,
) -> Result<ContentRedactionRaster,String> {
    let owned=host.capture(window.label())?;
    background(owned,move |owned|owned.render_content(&session_id,&source_id,preview_id.as_deref(),page_index,width)).await
}
#[tauri::command]
pub(crate) async fn prepare_text_removal_source(
    window: WebviewWindow,
    host: State<'_, RedactionHost>,
    session_id: String,
    request: crate::ExportRequest,
    bookmarks: Option<Vec<crate::DocumentBookmark>>,
) -> Result<TextRedactionSource, String> {
    let owned = host.capture(window.label())?;
    background(owned, move |owned| {
        owned.prepare(&session_id, request, bookmarks)
    })
    .await
}
#[tauri::command]
pub(crate) async fn begin_text_removal_session(
    window: WebviewWindow,
    host: State<'_, RedactionHost>,
) -> Result<String, String> {
    let owned = host.capture(window.label())?;
    background(owned, |owned| owned.begin()).await
}
#[tauri::command]
pub(crate) async fn preview_source_text_removal(
    window: WebviewWindow,
    host: State<'_, RedactionHost>,
    session_id: String,
    source_id: String,
    marks: Vec<TextRedactionMark>,
) -> Result<TextRedactionPreview, String> {
    let owned = host.capture(window.label())?;
    background(owned, move |owned| {
        owned.preview(&session_id, &source_id, &marks)
    })
    .await
}
#[tauri::command]
pub(crate) async fn text_removal_preview_bounds(
    window: WebviewWindow,
    host: State<'_, RedactionHost>,
    session_id: String,
    preview_id: String,
    page_index: usize,
    offset: usize,
    limit: usize,
) -> Result<TextRedactionBounds, String> {
    let owned = host.capture(window.label())?;
    background(owned, move |owned| {
        owned.bounds(&session_id, &preview_id, page_index, offset, limit)
    })
    .await
}
#[tauri::command]
pub(crate) async fn apply_text_removal_preview(
    window: WebviewWindow,
    host: State<'_, RedactionHost>,
    session_id: String,
    preview_id: String,
) -> Result<DocumentInfo, String> {
    let owned = host.capture(window.label())?;
    background(owned, move |owned| owned.apply(&session_id, &preview_id)).await
}
#[tauri::command]
pub(crate) async fn discard_text_removal_preview(
    window: WebviewWindow,
    host: State<'_, RedactionHost>,
    session_id: String,
    preview_id: String,
) -> Result<(), String> {
    let owned = host.capture(window.label())?;
    background(owned, move |owned| owned.discard(&session_id, &preview_id)).await
}
#[tauri::command]
pub(crate) async fn release_text_removal_session(
    window: WebviewWindow,
    host: State<'_, RedactionHost>,
    session_id: String,
) -> Result<(), String> {
    let owned = host.capture(window.label())?;
    background(owned, move |owned| owned.release(&session_id)).await
}
