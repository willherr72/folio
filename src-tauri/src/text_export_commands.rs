use crate::{TextExportControl,TextExportRequest,TextExportResult,TextExportProgress,PdfEngine};
use std::sync::{Arc,Mutex};
use std::time::{Duration,Instant};
use tauri::{State,WebviewWindow,ipc::Channel};
#[derive(Default,Clone)]
pub(crate) struct TextExportJobs(Arc<Mutex<Option<Job>>>);
struct Job{id:String,owner:String,control:Arc<TextExportControl>,started:bool,created:Instant}
struct JobGuard{jobs:TextExportJobs,id:String}
impl Drop for JobGuard{fn drop(&mut self){if let Ok(mut state)=self.jobs.0.lock(){if state.as_ref().is_some_and(|job|job.id==self.id){state.take();}}}}
impl TextExportJobs{
 fn register(&self,owner:&str)->Result<String,String>{
  let mut state=self.0.lock().map_err(|_|"Text export state is unavailable.")?;
  if state.as_ref().is_some_and(|job|!job.started&&job.created.elapsed()>Duration::from_secs(120)){if let Some(job)=state.take(){job.control.cancel();}}
  if state.is_some(){return Err("A text export is already open. Finish or cancel it first.".into());}
  let id=uuid::Uuid::new_v4().to_string();*state=Some(Job{id:id.clone(),owner:owner.into(),control:Arc::new(TextExportControl::new()),started:false,created:Instant::now()});Ok(id)
 }
 fn begin(&self,owner:&str,id:&str)->Result<(Arc<TextExportControl>,JobGuard),String>{
  let mut state=self.0.lock().map_err(|_|"Text export state is unavailable.")?;
  let job=state.as_mut().filter(|job|job.id==id&&job.owner==owner).ok_or("Text export registration is no longer available.")?;
  if job.started{return Err("This text export has already started.".into());}job.started=true;Ok((job.control.clone(),JobGuard{jobs:self.clone(),id:id.into()}))
 }
 fn cancel(&self,owner:&str,id:&str)->Result<bool,String>{
  let mut state=self.0.lock().map_err(|_|"Text export state is unavailable.")?;
  let Some(job)=state.as_ref().filter(|job|job.id==id)else{return Ok(false)};
  if job.owner!=owner{return Err("This text export belongs to another window.".into());}
  let accepted=job.control.cancel();if !job.started{state.take();}Ok(accepted)
 }
 pub(crate) fn abandon(&self,owner:&str){if let Ok(mut state)=self.0.lock(){if let Some(job)=state.as_ref().filter(|job|job.owner==owner){job.control.cancel();if !job.started{state.take();}}}}
}
#[derive(Clone,serde::Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct TextExportEvent{job_id:String,#[serde(flatten)]progress:TextExportProgress}
#[tauri::command]
pub(crate) fn register_text_export(window:WebviewWindow,jobs:State<'_,TextExportJobs>)->Result<String,String>{jobs.register(window.label())}
#[tauri::command]
pub(crate) fn cancel_text_export(window:WebviewWindow,jobs:State<'_,TextExportJobs>,job_id:String)->Result<bool,String>{jobs.cancel(window.label(),&job_id)}
#[tauri::command]
pub(crate) async fn export_text(window:WebviewWindow,jobs:State<'_,TextExportJobs>,engine:State<'_,PdfEngine>,job_id:String,request:TextExportRequest,progress:Channel<TextExportEvent>)->Result<Option<TextExportResult>,String>{
 let(control,guard)=jobs.begin(window.label(),&job_id)?;request.validate().map_err(|e|e.to_string())?;
 let engine=engine.inner().clone();
 tauri::async_runtime::spawn_blocking(move||{
  let _guard=guard;
  if control.cancelled(){return Ok(None);}
  let total=request.pages.len();let report=Arc::new(move|value:TextExportProgress|{let _=progress.send(TextExportEvent{job_id:job_id.clone(),progress:value});});
  report(TextExportProgress{completed:0,total,stage:"choosingFile"});
  let parent=rfd::FileDialog::new().set_title("Export text").add_filter("UTF-8 text", &["txt"]).set_file_name("Document.txt").save_file();
  if control.cancelled(){return Ok(None);}
  match parent{None=>Ok(None),Some(parent)=>engine.export_text(request,parent,control,report).map_err(|e|e.to_string())}
 }).await.map_err(|e|format!("Text export task failed: {e}"))?
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]fn registration_is_bounded_and_bound_to_the_window(){let jobs=TextExportJobs::default();let id=jobs.register("main").unwrap();assert!(jobs.register("main").is_err());assert!(jobs.begin("other",&id).is_err());assert!(jobs.cancel("other",&id).is_err());assert!(jobs.cancel("main",&id).unwrap());assert!(jobs.register("main").is_ok());}
 #[test]fn duplicate_start_cannot_clear_the_running_job(){let jobs=TextExportJobs::default();let id=jobs.register("main").unwrap();let(control,guard)=jobs.begin("main",&id).unwrap();assert!(jobs.begin("main",&id).is_err());assert!(jobs.register("main").is_err());assert!(jobs.cancel("main",&id).unwrap());assert!(control.cancelled());assert!(jobs.register("main").is_err());drop(guard);assert!(jobs.register("main").is_ok());}
 #[test]fn window_destruction_cancels_running_and_releases_unstarted_jobs(){let jobs=TextExportJobs::default();let id=jobs.register("main").unwrap();jobs.abandon("main");assert!(jobs.begin("main",&id).is_err());let id=jobs.register("main").unwrap();let(control,guard)=jobs.begin("main",&id).unwrap();jobs.abandon("main");assert!(control.cancelled());drop(guard);assert!(jobs.register("main").is_ok());}
}
