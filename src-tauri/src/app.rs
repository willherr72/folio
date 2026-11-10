use crate::printing::{print_document, PrintOptions};
use crate::TextGroupPreview;
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
async fn prepare_ocr_page(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,rotation:u16)->Result<crate::OcrPreparedPage,String> {
    #[cfg(feature="shaped-text")]
    {let engine=engine.inner().clone();on_worker(move||engine.prepare_ocr_page(&source_id,page_index,rotation)).await}
    #[cfg(not(feature="shaped-text"))]
    {let _=(engine,source_id,page_index,rotation);Err("This build does not include OCR.".into())}
}
#[tauri::command]
async fn apply_ocr(engine:State<'_,PdfEngine>,source_id:String,updates:Vec<crate::OcrUpdate>)->Result<DocumentInfo,String> {
    #[cfg(feature="shaped-text")]
    {let engine=engine.inner().clone();on_worker(move||engine.apply_ocr(&source_id,updates)).await}
    #[cfg(not(feature="shaped-text"))]
    {let _=(engine,source_id,updates);Err("This build does not include OCR.".into())}
}

#[tauri::command]
async fn list_native_comments(engine:State<'_,PdfEngine>,source_id:String)->Result<Vec<crate::NativeComment>,String> {
    let engine=engine.inner().clone();on_worker(move||engine.list_native_comments(&source_id)).await
}
#[tauri::command]
async fn delete_native_comment(engine:State<'_,PdfEngine>,source_id:String,request:crate::DeleteNativeCommentRequest)->Result<DocumentInfo,String> {
    let engine=engine.inner().clone();on_worker(move||engine.delete_native_comment(&source_id,request)).await
}
#[tauri::command]
async fn edit_native_comment(engine:State<'_,PdfEngine>,source_id:String,request:crate::EditNativeCommentRequest)->Result<DocumentInfo,String> {
    let engine=engine.inner().clone();on_worker(move||engine.edit_native_comment(&source_id,request)).await
}

#[tauri::command]
async fn list_form_fields(engine:State<'_,PdfEngine>,source_id:String,page_index:usize)->Result<Vec<crate::FormField>,String> {
    let engine=engine.inner().clone();on_worker(move||engine.list_form_fields(&source_id,page_index)).await
}
#[tauri::command]
async fn fill_form_field(engine:State<'_,PdfEngine>,source_id:String,request:crate::FillFormFieldRequest)->Result<DocumentInfo,String> {
    let engine=engine.inner().clone();on_worker(move||engine.fill_form_field(&source_id,request)).await
}

#[tauri::command]
async fn replace_page_content_occurrence(engine:State<'_,PdfEngine>,destination_id:String,occurrences:Vec<usize>,selected_occurrence:usize,incoming_id:String,incoming_page:usize)->Result<crate::PageContentReplacement,String> {
    let engine=engine.inner().clone();
    on_worker(move||engine.replace_page_content_occurrence(&destination_id,&occurrences,selected_occurrence,&incoming_id,incoming_page)).await
}

#[tauri::command]
async fn create_form_field_occurrence(engine:State<'_,PdfEngine>,source_id:String,occurrences:Vec<usize>,selected:usize,request:crate::CreateFormFieldRequest)->Result<crate::PageContentReplacement,String> {
    let engine=engine.inner().clone();on_worker(move||engine.create_form_field_occurrence(&source_id,&occurrences,selected,request)).await
}

#[tauri::command]
async fn position_form_field_occurrence(engine:State<'_,PdfEngine>,source_id:String,occurrences:Vec<usize>,selected:usize,request:crate::PositionFormFieldRequest)->Result<crate::PageContentReplacement,String>{
    let engine=engine.inner().clone();on_worker(move||engine.position_form_field_occurrence(&source_id,&occurrences,selected,request)).await
}
#[tauri::command]
async fn inspect_form_text_appearance(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,annotation_index:usize)->Result<crate::FormTextAppearance,String>{
    let engine=engine.inner().clone();on_worker(move||engine.inspect_form_text_appearance(&source_id,page_index,annotation_index)).await
}
#[tauri::command]
async fn inspect_form_frame(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,annotation_index:usize)->Result<crate::FormFrameAppearance,String>{
    let engine=engine.inner().clone();on_worker(move||engine.inspect_form_frame(&source_id,page_index,annotation_index)).await
}
#[tauri::command]
async fn set_form_frame_occurrence(engine:State<'_,PdfEngine>,source_id:String,occurrences:Vec<usize>,selected:usize,request:crate::SetFormFrameRequest)->Result<crate::PageContentReplacement,String>{
    let engine=engine.inner().clone();on_worker(move||engine.set_form_frame_occurrence(&source_id,&occurrences,selected,request)).await
}
#[tauri::command]
async fn inspect_form_properties(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,annotation_index:usize)->Result<crate::FormProperties,String>{
    let engine=engine.inner().clone();on_worker(move||engine.inspect_form_properties(&source_id,page_index,annotation_index)).await
}
#[tauri::command]
async fn set_form_properties(engine:State<'_,PdfEngine>,source_id:String,request:crate::SetFormPropertiesRequest)->Result<crate::DocumentInfo,String>{
    let engine=engine.inner().clone();on_worker(move||engine.set_form_properties(&source_id,request)).await
}
#[tauri::command]
async fn inspect_form_removal(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,annotation_index:usize)->Result<crate::FormRemovalInfo,String>{
    let engine=engine.inner().clone();on_worker(move||engine.inspect_form_removal(&source_id,page_index,annotation_index)).await
}
#[tauri::command]
async fn remove_form_field(engine:State<'_,PdfEngine>,source_id:String,request:crate::RemoveFormFieldRequest)->Result<crate::DocumentInfo,String>{
    let engine=engine.inner().clone();on_worker(move||engine.remove_form_field(&source_id,request)).await
}
#[tauri::command]
async fn inspect_form_choices(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,annotation_index:usize)->Result<crate::FormChoices,String>{
    let engine=engine.inner().clone();on_worker(move||engine.inspect_form_choices(&source_id,page_index,annotation_index)).await
}
#[tauri::command]
async fn set_form_choices(engine:State<'_,PdfEngine>,source_id:String,request:crate::SetFormChoicesRequest)->Result<crate::DocumentInfo,String>{
    let engine=engine.inner().clone();on_worker(move||engine.set_form_choices(&source_id,request)).await
}
#[tauri::command]
async fn set_form_text_appearance_occurrence(engine:State<'_,PdfEngine>,source_id:String,occurrences:Vec<usize>,selected:usize,request:crate::SetFormTextAppearanceRequest)->Result<crate::PageContentReplacement,String>{
    let engine=engine.inner().clone();on_worker(move||engine.set_form_text_appearance_occurrence(&source_id,&occurrences,selected,request)).await
}

#[tauri::command]
async fn prepare_text_overlay(
    engine: State<'_, PdfEngine>,
    overlay: crate::TextOverlay,
) -> Result<serde_json::Value, String> {
    #[cfg(feature = "shaped-text")]
    {
        // Acquire exact immutable ownership before queuing background preparation.
        let id = overlay
            .font_id
            .as_deref()
            .ok_or("Shaped text requires a custom font.")?;
        let font = engine.fonts().get(id).map_err(|e| e.to_string())?;
        on_worker(move || {
            serde_json::to_value(crate::prepare_text_overlay(&font, &overlay)?)
                .map_err(|e| EngineError::InvalidRequest(e.to_string()))
        })
        .await
    }
    #[cfg(not(feature = "shaped-text"))]
    {
        let _ = (engine, overlay);
        Err("Shaped text is unavailable in this build.".into())
    }
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
async fn image_info(engine: State<'_, PdfEngine>, id: String) -> Result<crate::ImageInfo, String> {
    let images=engine.images();on_worker(move||Ok(images.get(&id)?.info().clone())).await
}
#[tauri::command]
async fn acquire_image(engine: State<'_, PdfEngine>, id: String) -> Result<crate::ImageRegistration, String> {
    let images=engine.images();on_worker(move||images.acquire(&id)).await
}
#[tauri::command]
async fn release_image(engine: State<'_, PdfEngine>, token: String) -> Result<(), String> {
    let images=engine.images();on_worker(move||images.release(&token)).await
}
#[tauri::command]
async fn import_image(engine: State<'_, PdfEngine>) -> Result<Option<crate::ImageRegistration>, String> {
    let engine=engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move|| {
        let Some(path)=rfd::FileDialog::new().set_title("Place an image").add_filter("PNG and JPEG images", &["png","jpg","jpeg"]).pick_file() else {return Ok(None);};
        engine.import_image(path).map(Some).map_err(|error|error.to_string())
    }).await.map_err(|error|format!("Image dialog failed: {error}"))?
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
async fn begin_open_pdf(engine:State<'_,PdfEngine>)->Result<Option<crate::OpenOutcome>,String> {
    let engine=engine.inner().clone();
    on_worker(move||rfd::FileDialog::new().set_title("Open a PDF").add_filter("PDF documents",&["pdf"]).pick_file()
        .map(|path|engine.begin_open_document(path)).transpose()).await
}
#[tauri::command]
async fn submit_open_password(engine:State<'_,PdfEngine>,challenge_id:String,password:String)->Result<crate::OpenOutcome,String> {
    let engine=engine.inner().clone();let password=crate::private_storage::Sensitive(password.into_bytes());
    on_worker(move||engine.submit_open_password(&challenge_id,std::str::from_utf8(&password.0).expect("UTF-8 password"))).await
}
#[tauri::command]
async fn cancel_open_pdf(engine:State<'_,PdfEngine>,challenge_id:String)->Result<(),String> {
    let engine=engine.inner().clone();on_worker(move||engine.cancel_open_document(&challenge_id)).await
}
#[tauri::command]
async fn unlock_sources(engine:State<'_,PdfEngine>,source_ids:Vec<String>,password:String)->Result<Option<Vec<crate::SourceReplacement>>,String> {
    let engine=engine.inner().clone();let password=crate::private_storage::Sensitive(password.into_bytes());
    on_worker(move||match engine.unlock_document_sources(&source_ids,std::str::from_utf8(&password.0).expect("UTF-8 password")) {
        Ok(replacements)=>Ok(Some(replacements)),
        Err(EngineError::Protection(crate::protection::ProtectionError::IncorrectPassword))=>Ok(None),
        Err(error)=>Err(error),
    }).await
}

#[tauri::command]
async fn create_blank_document(engine: State<'_, PdfEngine>, width:f32, height:f32) -> Result<DocumentInfo,String> {
    let engine = engine.inner().clone();
    on_worker(move || engine.create_blank_document(width,height)).await
}

#[tauri::command]
async fn pick_images() -> Result<Option<Vec<std::path::PathBuf>>,String> {
    tauri::async_runtime::spawn_blocking(||rfd::FileDialog::new()
        .set_title("Choose images")
        .add_filter("PNG and JPEG images",&["png","jpg","jpeg"])
        .pick_files()).await.map_err(|e|format!("Image dialog failed: {e}"))
}

#[tauri::command]
async fn create_image_document(engine:State<'_,PdfEngine>,paths:Vec<std::path::PathBuf>,options:crate::ImagePageOptions)->Result<DocumentInfo,String> {
    let engine=engine.inner().clone();
    on_worker(move||engine.create_image_document(paths,options)).await
}

#[tauri::command]
async fn render_page_plan(engine:State<'_,PdfEngine>,plan:crate::PagePlan,width:u32) -> Result<tauri::ipc::Response,String> {
    let engine=engine.inner().clone();
    on_worker(move || engine.render_page_plan(plan,width)).await.map(tauri::ipc::Response::new)
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
async fn search_page(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,query:String)->Result<crate::PageSearchResult,String> {
    let engine=engine.inner().clone();on_worker(move||engine.search_page(&source_id,page_index,&query)).await
}

#[tauri::command]
async fn check_source_combination(engine:State<'_,PdfEngine>,source_ids:Vec<String>)->Result<(),String> {
    let engine=engine.inner().clone();on_worker(move||engine.check_source_combination(&source_ids)).await
}

#[tauri::command]
async fn list_occurrence_text_runs(engine:State<'_,PdfEngine>,source_id:String,page_index:usize)->Result<TextRuns,String>{
    let engine=engine.inner().clone();
    on_worker(move||engine.list_occurrence_text_runs(&source_id,page_index)).await
}

#[tauri::command]
async fn replace_text_occurrence(engine:State<'_,PdfEngine>,source_id:String,occurrences:Vec<usize>,selected_occurrence:usize,object_index:usize,expected_text:String,replacement:String,font_id:Option<String>)->Result<crate::PageContentReplacement,String>{
    let engine=engine.inner().clone();
    on_worker(move||engine.replace_text_occurrence(&source_id,&occurrences,selected_occurrence,object_index,&expected_text,&replacement,font_id.as_deref())).await
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
async fn inspect_occurrence_text_paragraph(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,object_indices:Vec<usize>)->Result<crate::TextParagraphSelection,String>{
    let engine=engine.inner().clone();on_worker(move||engine.inspect_occurrence_text_paragraph(&source_id,page_index,&object_indices)).await
}
#[tauri::command]
async fn suggest_occurrence_text_paragraph(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,object_index:usize,expected_text:String)->Result<crate::TextParagraphSelection,String>{
    let engine=engine.inner().clone();on_worker(move||engine.suggest_occurrence_text_paragraph(&source_id,page_index,object_index,&expected_text)).await
}
#[tauri::command]
async fn inspect_occurrence_styled_paragraph(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,object_indices:Vec<usize>)->Result<crate::StyledParagraphSelection,String>{
    let engine=engine.inner().clone();on_worker(move||engine.inspect_occurrence_styled_paragraph(&source_id,page_index,&object_indices)).await
}
#[tauri::command]
async fn list_existing_images(engine:State<'_,PdfEngine>,source_id:String,page_index:usize)->Result<crate::ExistingImages,String>{
    let engine=engine.inner().clone();on_worker(move||engine.list_existing_images(&source_id,page_index)).await
}
#[tauri::command]
async fn preview_image_edit_occurrence(engine:State<'_,PdfEngine>,source_id:String,occurrences:Vec<usize>,selected_occurrence:usize,object_path:Vec<usize>,selection_id:String,edit:crate::ExistingImageEdit)->Result<crate::PageContentReplacement,String>{
    let engine=engine.inner().clone();on_worker(move||engine.preview_image_edit_occurrence(&source_id,&occurrences,selected_occurrence,&object_path,&selection_id,edit)).await
}
#[tauri::command]
async fn preview_image_replacement_occurrence(engine:State<'_,PdfEngine>,source_id:String,occurrences:Vec<usize>,selected_occurrence:usize,object_path:Vec<usize>,selection_id:String,asset_id:String,fit_mode:crate::ImageFitMode)->Result<crate::PageContentReplacement,String>{
    let engine=engine.inner().clone();on_worker(move||engine.preview_image_replacement_occurrence(&source_id,&occurrences,selected_occurrence,&object_path,&selection_id,&asset_id,fit_mode)).await
}
#[tauri::command]
async fn list_form_paragraph_runs(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,object_path:Vec<usize>,expected_text:String)->Result<crate::FormParagraphRuns,String>{
    let engine=engine.inner().clone();on_worker(move||engine.list_form_paragraph_runs(&source_id,page_index,&object_path,&expected_text)).await
}
#[tauri::command]
async fn map_paragraph_draft(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,object_indices:Vec<usize>,expected_paragraph_text:String,anchor_index:usize,expected_source_text:String,kind:String)->Result<crate::ParagraphDraftMapping,String>{
    let styled=match kind.as_str(){"styled"=>true,"uniform"=>false,_=>return Err("Choose a valid paragraph editor.".into())};
    let engine=engine.inner().clone();on_worker(move||engine.map_paragraph_draft(&source_id,page_index,&object_indices,&expected_paragraph_text,anchor_index,&expected_source_text,styled)).await
}
#[tauri::command]
async fn map_form_paragraph_draft(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,object_path:Vec<usize>,object_indices:Vec<usize>,expected_paragraph_text:String,anchor_index:usize,expected_source_text:String)->Result<crate::ParagraphDraftMapping,String>{
    let engine=engine.inner().clone();on_worker(move||engine.map_form_paragraph_draft(&source_id,page_index,&object_path,&object_indices,&expected_paragraph_text,anchor_index,&expected_source_text)).await
}
#[tauri::command]
async fn inspect_form_styled_paragraph(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,object_path:Vec<usize>,object_indices:Vec<usize>)->Result<crate::StyledParagraphSelection,String>{
    let engine=engine.inner().clone();on_worker(move||engine.inspect_form_styled_paragraph(&source_id,page_index,&object_path,&object_indices)).await
}
#[tauri::command]
async fn suggest_form_styled_paragraph(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,object_path:Vec<usize>,object_index:usize,expected_text:String)->Result<crate::StyledParagraphSelection,String>{
    let engine=engine.inner().clone();on_worker(move||engine.suggest_form_styled_paragraph(&source_id,page_index,&object_path,object_index,&expected_text)).await
}
#[tauri::command]
async fn fit_form_styled_paragraph(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,object_path:Vec<usize>,request:crate::StyledParagraphLayoutRequest)->Result<crate::AnnotationRect,String>{
    let engine=engine.inner().clone();on_worker(move||engine.fit_form_styled_paragraph(&source_id,page_index,&object_path,&request)).await
}
#[tauri::command]
async fn preview_form_styled_paragraph_occurrence(engine:State<'_,PdfEngine>,source_id:String,occurrences:Vec<usize>,selected_occurrence:usize,object_path:Vec<usize>,request:crate::StyledParagraphLayoutRequest)->Result<crate::PageContentReplacement,String>{
    let engine=engine.inner().clone();on_worker(move||engine.preview_form_styled_paragraph_occurrence(&source_id,&occurrences,selected_occurrence,&object_path,&request)).await
}
#[tauri::command]
async fn suggest_occurrence_styled_paragraph(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,object_index:usize,expected_text:String)->Result<crate::StyledParagraphSelection,String>{
    let engine=engine.inner().clone();on_worker(move||engine.suggest_occurrence_styled_paragraph(&source_id,page_index,object_index,&expected_text)).await
}
#[tauri::command]
async fn fit_occurrence_styled_paragraph(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,request:crate::StyledParagraphLayoutRequest)->Result<crate::AnnotationRect,String>{
    let engine=engine.inner().clone();on_worker(move||engine.fit_occurrence_styled_paragraph(&source_id,page_index,&request)).await
}
#[tauri::command]
async fn preview_styled_paragraph_occurrence(engine:State<'_,PdfEngine>,source_id:String,occurrences:Vec<usize>,selected_occurrence:usize,request:crate::StyledParagraphLayoutRequest)->Result<crate::PageContentReplacement,String>{
    let engine=engine.inner().clone();on_worker(move||engine.preview_styled_paragraph_occurrence(&source_id,&occurrences,selected_occurrence,&request)).await
}
#[tauri::command]
async fn preview_text_paragraph_occurrence(engine:State<'_,PdfEngine>,source_id:String,occurrences:Vec<usize>,selected_occurrence:usize,object_indices:Vec<usize>,expected_text:String,replacement:String,bounds:crate::AnnotationRect,font_size:Option<f32>,alignment:Option<crate::ParagraphAlignment>,line_spacing:Option<f32>,font_id:Option<String>,color:Option<String>)->Result<crate::PageContentReplacement,String>{
    let engine=engine.inner().clone();on_worker(move||engine.preview_text_paragraph_occurrence(&source_id,&occurrences,selected_occurrence,&object_indices,&expected_text,&replacement,&bounds,&crate::ParagraphFormatting{font_size,alignment,line_spacing,font_id,color})).await
}
#[tauri::command]
async fn suggest_text_paragraph(
    engine: State<'_, PdfEngine>, source_id: String, page_index: usize, object_index: usize, expected_text: String,
) -> Result<crate::TextParagraphSelection, String> {
    let engine=engine.inner().clone();
    on_worker(move||engine.suggest_text_paragraph(&source_id,page_index,object_index,&expected_text)).await
}

#[tauri::command]
async fn inspect_text_paragraph(
    engine: State<'_, PdfEngine>,
    source_id: String,
    page_index: usize,
    object_indices: Vec<usize>,
) -> Result<crate::TextParagraphSelection, String> {
    let engine = engine.inner().clone();
    on_worker(move || engine.inspect_text_paragraph(&source_id, page_index, &object_indices)).await
}
#[tauri::command]
async fn inspect_styled_paragraph(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,object_indices:Vec<usize>)->Result<crate::StyledParagraphSelection,String>{
    let engine=engine.inner().clone();on_worker(move||engine.inspect_styled_paragraph(&source_id,page_index,&object_indices)).await
}
#[tauri::command]
async fn suggest_styled_paragraph(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,object_index:usize,expected_text:String)->Result<crate::StyledParagraphSelection,String>{
    let engine=engine.inner().clone();on_worker(move||engine.suggest_styled_paragraph(&source_id,page_index,object_index,&expected_text)).await
}
#[tauri::command]
async fn preview_styled_paragraph(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,request:crate::StyledParagraphLayoutRequest)->Result<DocumentInfo,String>{
    let engine=engine.inner().clone();on_worker(move||engine.preview_styled_paragraph(&source_id,page_index,&request)).await
}
#[tauri::command]
async fn fit_styled_paragraph(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,request:crate::StyledParagraphLayoutRequest)->Result<crate::AnnotationRect,String>{
    let engine=engine.inner().clone();on_worker(move||engine.fit_styled_paragraph(&source_id,page_index,&request)).await
}
#[tauri::command]
async fn preview_text_paragraph(
    engine: State<'_, PdfEngine>,
    source_id: String,
    page_index: usize,
    object_indices: Vec<usize>,
    expected_text: String,
    replacement: String,
    bounds: crate::AnnotationRect,
    font_size: Option<f32>,
    alignment: Option<crate::ParagraphAlignment>,
    line_spacing: Option<f32>,
    font_id: Option<String>,
    color: Option<String>,
) -> Result<DocumentInfo, String> {
    let engine = engine.inner().clone();
    on_worker(move || {
        engine.preview_text_paragraph_with_format(
            &source_id,
            page_index,
            &object_indices,
            &expected_text,
            &replacement,
            &bounds,
            &crate::ParagraphFormatting{font_size,alignment,line_spacing,font_id,color},
        )
    })
    .await
}
#[tauri::command]
async fn inspect_occurrence_text_group(engine:State<'_,PdfEngine>,source_id:String,page_index:usize,object_indices:Vec<usize>)->Result<TextGroupPreview,String>{
    let engine=engine.inner().clone();on_worker(move||engine.inspect_occurrence_text_group(&source_id,page_index,&object_indices)).await
}
#[tauri::command]
async fn replace_text_group_occurrence(engine:State<'_,PdfEngine>,source_id:String,occurrences:Vec<usize>,selected_occurrence:usize,object_indices:Vec<usize>,expected_text:String,replacement:String)->Result<crate::PageContentReplacement,String>{
    let engine=engine.inner().clone();on_worker(move||engine.replace_text_group_occurrence(&source_id,&occurrences,selected_occurrence,&object_indices,&expected_text,&replacement)).await
}
#[tauri::command]
async fn inspect_text_group(
    engine: State<'_, PdfEngine>,
    source_id: String,
    page_index: usize,
    object_indices: Vec<usize>,
) -> Result<TextGroupPreview, String> {
    let engine = engine.inner().clone();
    on_worker(move || engine.inspect_text_group(&source_id, page_index, &object_indices)).await
}
#[tauri::command]
async fn replace_text_group(
    engine: State<'_, PdfEngine>,
    source_id: String,
    page_index: usize,
    object_indices: Vec<usize>,
    expected_text: String,
    replacement: String,
) -> Result<DocumentInfo, String> {
    let engine = engine.inner().clone();
    on_worker(move || {
        engine.replace_text_group(
            &source_id,
            page_index,
            &object_indices,
            &expected_text,
            &replacement,
        )
    })
    .await
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
async fn inspect_document_properties(engine:State<'_,PdfEngine>,source_id:String)->Result<crate::DocumentProperties,String>{
    let engine=engine.inner().clone();tauri::async_runtime::spawn_blocking(move||engine.inspect_document_properties(&source_id).map_err(|e|e.to_string())).await.map_err(|e|e.to_string())?
}

#[tauri::command]
async fn export_pdf(
    engine: State<'_, PdfEngine>,
    request: ExportRequest,
    bookmarks:Option<Vec<crate::DocumentBookmark>>,
    metadata:Option<crate::DocumentMetadataEdit>,
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
                match metadata{Some(metadata)=>engine.export_pdf_with_metadata(request,bookmarks,metadata,&path),None=>engine.export_pdf_with_bookmarks(request,bookmarks,&path)}.map_err(|error|error.to_string())?;
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
async fn close_document(host: State<'_, crate::redaction_host::RedactionHost>, source_id: String) -> Result<(), String> {
    let host = host.inner().clone();
    tauri::async_runtime::spawn_blocking(move || host.close_document(&source_id))
        .await.map_err(|error| format!("Background PDF task failed: {error}"))?
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
            app.manage(engine.clone());
            let text_removal=crate::redaction_host::RedactionHost::new(engine.clone());
            for label in app.webview_windows().keys() {
                text_removal.register_window(label).map_err(std::io::Error::other)?;
            }
            app.manage(text_removal);
            app.manage(crate::image_export_commands::ImageExportJobs::default());
            app.manage(crate::pdf_split_commands::PdfSplitJobs::default());
            app.manage(crate::text_export_commands::TextExportJobs::default());
            app.manage(crate::pdf_save_commands::PdfSaveJobs::default());
            let recovery_root = std::env::var_os("FOLIO_DATA_DIR")
                .map(PathBuf::from)
                .map(Ok)
                .unwrap_or_else(|| app.path().app_local_data_dir().map_err(|e| e.to_string()))
                .map(|root| root.join("recovery"));
            app.manage(RecoveryState(
                recovery_root.and_then(|root|RecoveryStore::for_application(root,&engine)).map(Arc::new),
            ));
            Ok(())
        })
        .on_window_event(|window,event|{
            if matches!(event,tauri::WindowEvent::Destroyed) {
                if let Some(host)=window.try_state::<crate::redaction_host::RedactionHost>() {
                    if let Some(owned)=host.detach(window.label()) {
                        tauri::async_runtime::spawn_blocking(move||owned.cleanup());
                    }
                }
            }
            if matches!(event,tauri::WindowEvent::Destroyed){if let Some(jobs)=window.try_state::<crate::pdf_save_commands::PdfSaveJobs>(){jobs.abandon(window.label());}}
            if matches!(event,tauri::WindowEvent::Destroyed){if let Some(jobs)=window.try_state::<crate::pdf_split_commands::PdfSplitJobs>(){jobs.abandon(window.label());}if let Some(jobs)=window.try_state::<crate::text_export_commands::TextExportJobs>(){jobs.abandon(window.label());}if let Some(jobs)=window.try_state::<crate::image_export_commands::ImageExportJobs>(){jobs.abandon(window.label());}}
        })
        .invoke_handler(tauri::generate_handler![
            inspect_document_properties,
            crate::redaction_commands::begin_text_removal_session,
            crate::redaction_commands::prepare_text_removal_source,
            crate::redaction_commands::preview_source_text_removal,
            crate::redaction_commands::preview_source_content_removal,
            crate::redaction_commands::render_content_removal,
            crate::redaction_commands::text_removal_preview_bounds,
            crate::redaction_commands::apply_text_removal_preview,
            crate::redaction_commands::discard_text_removal_preview,
            crate::redaction_commands::release_text_removal_session,
            crate::pdf_save_commands::register_pdf_save,
            crate::pdf_save_commands::cancel_pdf_save,
            crate::pdf_save_commands::save_pdf,
            crate::text_export_commands::register_text_export,
            crate::text_export_commands::cancel_text_export,
            crate::text_export_commands::export_text,
            crate::pdf_split_commands::register_pdf_split,
            crate::pdf_split_commands::cancel_pdf_split,
            crate::pdf_split_commands::split_pdf,
            crate::image_export_commands::register_image_export,
            crate::image_export_commands::cancel_image_export,
            crate::image_export_commands::export_page_images,
            check_source_combination,
            search_page,
            begin_open_pdf,
            submit_open_password,
            cancel_open_pdf,
            unlock_sources,
            open_pdf,
            create_blank_document,
            pick_images,
            create_image_document,
            render_page_plan,
            list_installed_fonts,
            prepare_text_overlay,
            load_installed_font,
            import_font,
            font_info,
            image_info,
            acquire_image,
            release_image,
            import_image,
            font_bytes,
            release_font,
            render_page,
            page_text,
            list_text_runs,
            list_occurrence_text_runs,
            inspect_occurrence_text_paragraph,
            suggest_occurrence_text_paragraph,
            inspect_occurrence_styled_paragraph,
            list_form_paragraph_runs,
            map_paragraph_draft,
            map_form_paragraph_draft,
            list_existing_images,
            preview_image_replacement_occurrence,
            preview_image_edit_occurrence,
            inspect_form_styled_paragraph,
            suggest_form_styled_paragraph,
            fit_form_styled_paragraph,
            preview_form_styled_paragraph_occurrence,
            suggest_occurrence_styled_paragraph,
            fit_occurrence_styled_paragraph,
            preview_styled_paragraph_occurrence,
            preview_text_paragraph_occurrence,
            replace_text_occurrence,
            list_native_comments,
            edit_native_comment,
            delete_native_comment,
            list_form_fields,
            fill_form_field,
            create_form_field_occurrence,
            position_form_field_occurrence,
              inspect_form_text_appearance,
              inspect_form_frame,
              set_form_frame_occurrence,
              inspect_form_properties,
              set_form_properties,
              inspect_form_removal,
              remove_form_field,
              inspect_form_choices,
              set_form_choices,
              set_form_text_appearance_occurrence,
            replace_page_content_occurrence,
            prepare_ocr_page,
            apply_ocr,
            list_form_text_runs,
            replace_form_text,
            list_occurrence_form_text_runs,
            replace_form_text_occurrence,
            replace_text,
            inspect_text_group,
            inspect_occurrence_text_group,
            replace_text_group_occurrence,
            inspect_text_paragraph,
            suggest_text_paragraph,
            preview_text_paragraph,
            inspect_styled_paragraph,
            suggest_styled_paragraph,
            preview_styled_paragraph,
            fit_styled_paragraph,
            replace_text_group,
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

#[tauri::command]
async fn list_occurrence_form_text_runs(engine:State<'_,PdfEngine>,source_id:String,page_index:usize)->Result<crate::FormTextRuns,String>{
    let engine=engine.inner().clone();
    on_worker(move||engine.list_occurrence_form_text_runs(&source_id,page_index)).await
}
#[tauri::command]
async fn replace_form_text_occurrence(engine:State<'_,PdfEngine>,source_id:String,occurrences:Vec<usize>,selected_occurrence:usize,object_path:Vec<usize>,expected_text:String,replacement:String,font_id:Option<String>)->Result<crate::PageContentReplacement,String>{
    let engine=engine.inner().clone();
    on_worker(move||engine.replace_form_text_occurrence(&source_id,&occurrences,selected_occurrence,&object_path,&expected_text,&replacement,font_id.as_deref())).await
}
#[tauri::command]
async fn list_form_text_runs(
    engine: State<'_, PdfEngine>,
    source_id: String,
    page_index: usize,
) -> Result<crate::FormTextRuns, String> {
    let engine = engine.inner().clone();
    on_worker(move || engine.list_form_text_runs(&source_id, page_index)).await
}
#[tauri::command]
async fn replace_form_text(
    engine: State<'_, PdfEngine>,
    source_id: String,
    page_index: usize,
    object_path: Vec<usize>,
    expected_text: String,
    replacement: String,
    font_id: Option<String>,
) -> Result<DocumentInfo, String> {
    let engine = engine.inner().clone();
    on_worker(move || {
        engine.replace_form_text_with_font(
            &source_id,
            page_index,
            &object_path,
            &expected_text,
            &replacement,
            font_id.as_deref(),
        )
    })
    .await
}
