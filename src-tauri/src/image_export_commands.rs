use crate::{PageImageControl,PageImageExportRequest,PageImageExportResult,PageImageProgress,PdfEngine};
use std::sync::{Arc,Mutex};
use std::time::{Duration,Instant};
use tauri::{State,WebviewWindow,ipc::Channel};
#[derive(Default,Clone)]
pub(crate) struct ImageExportJobs(Arc<Mutex<Option<Job>>>);
struct Job{id:String,owner:String,control:Arc<PageImageControl>,started:bool,created:Instant}
struct JobGuard{jobs:ImageExportJobs,id:String}
impl Drop for JobGuard{fn drop(&mut self){if let Ok(mut state)=self.jobs.0.lock(){if state.as_ref().is_some_and(|job|job.id==self.id){state.take();}}}}
impl ImageExportJobs{
 fn register(&self,owner:&str)->Result<String,String>{
  let mut state=self.0.lock().map_err(|_|"Image export state is unavailable.")?;
  if state.as_ref().is_some_and(|job|!job.started&&job.created.elapsed()>Duration::from_secs(120)){if let Some(job)=state.take(){job.control.cancel();}}
  if state.is_some(){return Err("An image export is already open. Finish or cancel it first.".into());}
  let id=uuid::Uuid::new_v4().to_string();*state=Some(Job{id:id.clone(),owner:owner.into(),control:Arc::new(PageImageControl::new()),started:false,created:Instant::now()});Ok(id)
 }
 fn begin(&self,owner:&str,id:&str)->Result<(Arc<PageImageControl>,JobGuard),String>{
  let mut state=self.0.lock().map_err(|_|"Image export state is unavailable.")?;
  let job=state.as_mut().filter(|job|job.id==id&&job.owner==owner).ok_or("Image export registration is no longer available.")?;
  if job.started{return Err("This image export has already started.".into());}job.started=true;Ok((job.control.clone(),JobGuard{jobs:self.clone(),id:id.into()}))
 }
 fn cancel(&self,owner:&str,id:&str)->Result<bool,String>{
  let mut state=self.0.lock().map_err(|_|"Image export state is unavailable.")?;
  let Some(job)=state.as_ref().filter(|job|job.id==id)else{return Ok(false)};
  if job.owner!=owner{return Err("This image export belongs to another window.".into());}
  let accepted=job.control.cancel();if !job.started{state.take();}Ok(accepted)
 }
 pub(crate) fn abandon(&self,owner:&str){if let Ok(mut state)=self.0.lock(){if let Some(job)=state.as_ref().filter(|job|job.owner==owner){job.control.cancel();if !job.started{state.take();}}}}
}
#[derive(Clone,serde::Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ImageExportEvent{job_id:String,#[serde(flatten)]progress:PageImageProgress}
#[tauri::command]
pub(crate) fn register_image_export(window:WebviewWindow,jobs:State<'_,ImageExportJobs>)->Result<String,String>{jobs.register(window.label())}
#[tauri::command]
pub(crate) fn cancel_image_export(window:WebviewWindow,jobs:State<'_,ImageExportJobs>,job_id:String)->Result<bool,String>{jobs.cancel(window.label(),&job_id)}
#[tauri::command]
pub(crate) async fn export_page_images(window:WebviewWindow,jobs:State<'_,ImageExportJobs>,engine:State<'_,PdfEngine>,job_id:String,request:PageImageExportRequest,progress:Channel<ImageExportEvent>)->Result<Option<PageImageExportResult>,String>{
 let(control,guard)=jobs.begin(window.label(),&job_id)?;request.validate().map_err(|e|e.to_string())?;
 let engine=engine.inner().clone();
 tauri::async_runtime::spawn_blocking(move||{
  let _guard=guard;
  if control.cancelled(){return Ok(None);}
  let total=request.pages.len();let report=Arc::new(move|value:PageImageProgress|{let _=progress.send(ImageExportEvent{job_id:job_id.clone(),progress:value});});
  report(PageImageProgress{completed:0,total,stage:"choosingFolder"});
  let parent=rfd::FileDialog::new().set_title("Choose a folder for exported page images").pick_folder();
  if control.cancelled(){return Ok(None);}
  match parent{None=>Ok(None),Some(parent)=>engine.export_page_images(request,parent,control,report).map_err(|e|e.to_string())}
 }).await.map_err(|e|format!("Image export task failed: {e}"))?
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]fn registration_is_bounded_and_bound_to_the_window(){let jobs=ImageExportJobs::default();let id=jobs.register("main").unwrap();assert!(jobs.register("main").is_err());assert!(jobs.begin("other",&id).is_err());assert!(jobs.cancel("other",&id).is_err());assert!(jobs.cancel("main",&id).unwrap());assert!(jobs.register("main").is_ok());}
 #[test]fn duplicate_start_cannot_clear_the_running_job(){let jobs=ImageExportJobs::default();let id=jobs.register("main").unwrap();let(control,guard)=jobs.begin("main",&id).unwrap();assert!(jobs.begin("main",&id).is_err());assert!(jobs.register("main").is_err());assert!(jobs.cancel("main",&id).unwrap());assert!(control.cancelled());assert!(jobs.register("main").is_err());drop(guard);assert!(jobs.register("main").is_ok());}
 #[test]fn window_destruction_cancels_running_and_releases_unstarted_jobs(){let jobs=ImageExportJobs::default();let id=jobs.register("main").unwrap();jobs.abandon("main");assert!(jobs.begin("main",&id).is_err());let id=jobs.register("main").unwrap();let(control,guard)=jobs.begin("main",&id).unwrap();jobs.abandon("main");assert!(control.cancelled());drop(guard);assert!(jobs.register("main").is_ok());}
}
