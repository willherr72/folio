use crate::{PageImageControl,SplitPdfRequest,PageImageExportResult,PageImageProgress,PdfEngine};
use std::sync::{Arc,Mutex};
use std::time::{Duration,Instant};
use tauri::{State,WebviewWindow,ipc::Channel};
#[derive(Default,Clone)]
pub(crate) struct PdfSplitJobs(Arc<Mutex<Option<Job>>>);
struct Job{id:String,owner:String,control:Arc<PageImageControl>,started:bool,created:Instant}
struct JobGuard{jobs:PdfSplitJobs,id:String}
impl Drop for JobGuard{fn drop(&mut self){if let Ok(mut state)=self.jobs.0.lock(){if state.as_ref().is_some_and(|job|job.id==self.id){state.take();}}}}
impl PdfSplitJobs{
 fn register(&self,owner:&str)->Result<String,String>{
  let mut state=self.0.lock().map_err(|_|"PDF split state is unavailable.")?;
  if state.as_ref().is_some_and(|job|!job.started&&job.created.elapsed()>Duration::from_secs(120)){if let Some(job)=state.take(){job.control.cancel();}}
  if state.is_some(){return Err("An PDF split is already open. Finish or cancel it first.".into());}
  let id=uuid::Uuid::new_v4().to_string();*state=Some(Job{id:id.clone(),owner:owner.into(),control:Arc::new(PageImageControl::new()),started:false,created:Instant::now()});Ok(id)
 }
 fn begin(&self,owner:&str,id:&str)->Result<(Arc<PageImageControl>,JobGuard),String>{
  let mut state=self.0.lock().map_err(|_|"PDF split state is unavailable.")?;
  let job=state.as_mut().filter(|job|job.id==id&&job.owner==owner).ok_or("PDF split registration is no longer available.")?;
  if job.started{return Err("This PDF split has already started.".into());}
  if job.created.elapsed()>Duration::from_secs(120){job.control.cancel();state.take();return Err("PDF split registration expired. Start the export again.".into());}
  job.started=true;Ok((job.control.clone(),JobGuard{jobs:self.clone(),id:id.into()}))
 }
 fn cancel(&self,owner:&str,id:&str)->Result<bool,String>{
  let mut state=self.0.lock().map_err(|_|"PDF split state is unavailable.")?;
  let Some(job)=state.as_ref().filter(|job|job.id==id)else{return Ok(false)};
  if job.owner!=owner{return Err("This PDF split belongs to another window.".into());}
  let accepted=job.control.cancel();if !job.started{state.take();}Ok(accepted)
 }
 pub(crate) fn abandon(&self,owner:&str){if let Ok(mut state)=self.0.lock(){if let Some(job)=state.as_ref().filter(|job|job.owner==owner){job.control.cancel();if !job.started{state.take();}}}}
}
#[derive(Clone,serde::Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct PdfSplitEvent{job_id:String,#[serde(flatten)]progress:PageImageProgress}
#[tauri::command]
pub(crate) fn register_pdf_split(window:WebviewWindow,jobs:State<'_,PdfSplitJobs>)->Result<String,String>{jobs.register(window.label())}
#[tauri::command]
pub(crate) fn cancel_pdf_split(window:WebviewWindow,jobs:State<'_,PdfSplitJobs>,job_id:String)->Result<bool,String>{jobs.cancel(window.label(),&job_id)}
#[tauri::command]
pub(crate) async fn split_pdf(window:WebviewWindow,jobs:State<'_,PdfSplitJobs>,engine:State<'_,PdfEngine>,job_id:String,request:SplitPdfRequest,progress:Channel<PdfSplitEvent>)->Result<Option<PageImageExportResult>,String>{
 let(control,guard)=jobs.begin(window.label(),&job_id)?;request.validate().map_err(|e|e.to_string())?;
 let engine=engine.inner().clone();
 tauri::async_runtime::spawn_blocking(move||{
  let _guard=guard;
  if control.cancelled(){return Ok(None);}
  let total=request.parts.len();let report=Arc::new(move|value:PageImageProgress|{let _=progress.send(PdfSplitEvent{job_id:job_id.clone(),progress:value});});
  report(PageImageProgress{completed:0,total,stage:"choosingFolder"});
  let parent=rfd::FileDialog::new().set_title("Choose a folder for split PDFs").pick_folder();
  if control.cancelled(){return Ok(None);}
  match parent{None=>Ok(None),Some(parent)=>engine.split_pdf(request,parent,control,report).map_err(|e|e.to_string())}
 }).await.map_err(|e|format!("PDF split task failed: {e}"))?
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]fn expired_pending_job_cannot_start_without_another_registration(){
  let jobs=PdfSplitJobs::default();let id=jobs.register("main").unwrap();
  let control={let mut state=jobs.0.lock().unwrap();let job=state.as_mut().unwrap();job.created=Instant::now()-Duration::from_secs(121);job.control.clone()};
  assert!(jobs.begin("other",&id).is_err());assert!(!control.cancelled(),"Another window cannot cancel this pending job");
  assert!(jobs.begin("main",&id).is_err());assert!(control.cancelled());assert!(jobs.register("main").is_ok());
 }
 #[test]fn expiration_never_reaps_a_running_job(){let jobs=PdfSplitJobs::default();let id=jobs.register("main").unwrap();let(control,guard)=jobs.begin("main",&id).unwrap();jobs.0.lock().unwrap().as_mut().unwrap().created=Instant::now()-Duration::from_secs(121);assert!(jobs.begin("main",&id).is_err());assert!(jobs.register("main").is_err());assert!(!control.cancelled());drop(guard);assert!(jobs.register("main").is_ok());}
 #[test]fn registration_is_bounded_and_bound_to_the_window(){let jobs=PdfSplitJobs::default();let id=jobs.register("main").unwrap();assert!(jobs.register("main").is_err());assert!(jobs.begin("other",&id).is_err());assert!(jobs.cancel("other",&id).is_err());assert!(jobs.cancel("main",&id).unwrap());assert!(jobs.register("main").is_ok());}
 #[test]fn duplicate_start_cannot_clear_the_running_job(){let jobs=PdfSplitJobs::default();let id=jobs.register("main").unwrap();let(control,guard)=jobs.begin("main",&id).unwrap();assert!(jobs.begin("main",&id).is_err());assert!(jobs.register("main").is_err());assert!(jobs.cancel("main",&id).unwrap());assert!(control.cancelled());assert!(jobs.register("main").is_err());drop(guard);assert!(jobs.register("main").is_ok());}
 #[test]fn window_destruction_cancels_running_and_releases_unstarted_jobs(){let jobs=PdfSplitJobs::default();let id=jobs.register("main").unwrap();jobs.abandon("main");assert!(jobs.begin("main",&id).is_err());let id=jobs.register("main").unwrap();let(control,guard)=jobs.begin("main",&id).unwrap();jobs.abandon("main");assert!(control.cancelled());drop(guard);assert!(jobs.register("main").is_ok());}
}
