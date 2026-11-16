use crate::{PagePlan,WorkspaceReplacementIntent,WorkspaceReplacementPlan,WorkspaceReplacementResult};
use crate::workspace_replacement_host::{WorkspaceReplacementHost,ReplacementWindow,WorkspaceReplacementPreview};
use tauri::{State,WebviewWindow};

async fn background<T:Send+'static>(window:ReplacementWindow,work:impl FnOnce(ReplacementWindow)->Result<T,String>+Send+'static)->Result<T,String>{
    tauri::async_runtime::spawn_blocking(move||work(window)).await.map_err(|e|format!("Replacement task failed: {e}"))?
}
#[tauri::command]
pub(crate) async fn begin_workspace_replacement(window:WebviewWindow,host:State<'_,WorkspaceReplacementHost>,pages:Vec<PagePlan>)->Result<String,String>{
    background(host.capture(window.label())?,move|owned|owned.begin(pages)).await
}
#[tauri::command]
pub(crate) async fn plan_workspace_replacement(window:WebviewWindow,host:State<'_,WorkspaceReplacementHost>,session_id:String,intent:WorkspaceReplacementIntent)->Result<WorkspaceReplacementPlan,String>{
    background(host.capture(window.label())?,move|owned|owned.plan(&session_id,&intent)).await
}
#[tauri::command]
pub(crate) async fn preview_workspace_replacement(window:WebviewWindow,host:State<'_,WorkspaceReplacementHost>,session_id:String,plan_id:String)->Result<WorkspaceReplacementPreview,String>{
    background(host.capture(window.label())?,move|owned|owned.preview(&session_id,&plan_id)).await
}
#[tauri::command]
pub(crate) async fn apply_workspace_replacement(window:WebviewWindow,host:State<'_,WorkspaceReplacementHost>,session_id:String,preview_id:String)->Result<WorkspaceReplacementResult,String>{
    background(host.capture(window.label())?,move|owned|owned.apply(&session_id,&preview_id)).await
}
#[tauri::command]
pub(crate) fn cancel_workspace_replacement(window:WebviewWindow,host:State<'_,WorkspaceReplacementHost>,session_id:String)->Result<(),String>{
    let owned=host.capture(window.label())?;owned.cancel(&session_id)?;
    tauri::async_runtime::spawn_blocking(move||{let _=owned.discard_cancelled(&session_id);});Ok(())
}
#[tauri::command]
pub(crate) async fn release_workspace_replacement(window:WebviewWindow,host:State<'_,WorkspaceReplacementHost>,session_id:String)->Result<(),String>{
    background(host.capture(window.label())?,move|owned|owned.release(&session_id)).await
}
