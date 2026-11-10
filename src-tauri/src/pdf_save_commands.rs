use crate::{PdfSaveControl,ExportRequest,DocumentBookmark,PdfSaveProgress,PdfEngine};
use std::sync::{Arc,Mutex};
use std::time::{Duration,Instant};
use tauri::{State,WebviewWindow,ipc::Channel};
#[derive(Default,Clone)]
pub(crate) struct PdfSaveJobs(Arc<Mutex<Option<Job>>>);
struct Job{id:String,owner:String,control:Arc<PdfSaveControl>,started:bool,created:Instant}
struct JobGuard{jobs:PdfSaveJobs,id:String}
impl Drop for JobGuard{fn drop(&mut self){if let Ok(mut state)=self.jobs.0.lock(){if state.as_ref().is_some_and(|job|job.id==self.id){state.take();}}}}
impl PdfSaveJobs{
 fn register(&self,owner:&str)->Result<String,String>{
  let mut state=self.0.lock().map_err(|_|"PDF save state is unavailable.")?;
  if state.as_ref().is_some_and(|job|!job.started&&job.created.elapsed()>Duration::from_secs(120)){if let Some(job)=state.take(){job.control.cancel();}}
  if state.is_some(){return Err("A PDF save is already open. Finish or cancel it first.".into());}
  let id=uuid::Uuid::new_v4().to_string();*state=Some(Job{id:id.clone(),owner:owner.into(),control:Arc::new(PdfSaveControl::new()),started:false,created:Instant::now()});Ok(id)
 }
 fn begin(&self,owner:&str,id:&str)->Result<(Arc<PdfSaveControl>,JobGuard),String>{
  let mut state=self.0.lock().map_err(|_|"PDF save state is unavailable.")?;
  let job=state.as_mut().filter(|job|job.id==id&&job.owner==owner).ok_or("PDF save registration is no longer available.")?;
  if job.started{return Err("This PDF save has already started.".into());}
  if job.created.elapsed()>Duration::from_secs(120){job.control.cancel();state.take();return Err("PDF save registration expired. Start the export again.".into());}
  job.started=true;Ok((job.control.clone(),JobGuard{jobs:self.clone(),id:id.into()}))
 }
 fn cancel(&self,owner:&str,id:&str)->Result<bool,String>{
  let mut state=self.0.lock().map_err(|_|"PDF save state is unavailable.")?;
  let Some(job)=state.as_ref().filter(|job|job.id==id)else{return Ok(false)};
  if job.owner!=owner{return Err("This PDF save belongs to another window.".into());}
  let accepted=job.control.cancel();if !job.started{state.take();}Ok(accepted)
 }
 pub(crate) fn abandon(&self,owner:&str){if let Ok(mut state)=self.0.lock(){if let Some(job)=state.as_ref().filter(|job|job.owner==owner){job.control.cancel();if !job.started{state.take();}}}}
}
#[derive(Clone,serde::Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct PdfSaveEvent{job_id:String,#[serde(flatten)]progress:PdfSaveProgress}
#[tauri::command]
pub(crate) fn register_pdf_save(window:WebviewWindow,jobs:State<'_,PdfSaveJobs>)->Result<String,String>{jobs.register(window.label())}
#[tauri::command]
pub(crate) fn cancel_pdf_save(window:WebviewWindow,jobs:State<'_,PdfSaveJobs>,job_id:String)->Result<bool,String>{jobs.cancel(window.label(),&job_id)}
#[tauri::command]
pub(crate) async fn save_pdf(window:WebviewWindow,jobs:State<'_,PdfSaveJobs>,engine:State<'_,PdfEngine>,job_id:String,request:ExportRequest,bookmarks:Option<Vec<DocumentBookmark>>,security:Option<crate::ProtectionChange>,metadata:Option<crate::DocumentMetadataEdit>,progress:Channel<PdfSaveEvent>)->Result<Option<String>,String>{
 let(control,guard)=jobs.begin(window.label(),&job_id)?;
 if request.pages.is_empty()||request.pages.len()>10000{return Err("Choose between 1 and 10,000 pages.".into());}
 if let Some(security)=&security{security.validate().map_err(|error|error.to_string())?;}
 if let Some(metadata)=&metadata{metadata.validate().map_err(|error|error.to_string())?;}
 let engine=engine.inner().clone();
 tauri::async_runtime::spawn_blocking(move||{
  let _guard=guard;
  if control.cancelled(){return Ok(None);}
  let total=request.pages.len();
  let report:Arc<dyn Fn(PdfSaveProgress)+Send+Sync>=Arc::new(move|value|{let _=progress.send(PdfSaveEvent{job_id:job_id.clone(),progress:value});});
  report(PdfSaveProgress{completed:0,total,stage:"choosingFile"});
  let path=rfd::FileDialog::new().set_title("Save a PDF copy").set_file_name("Folio edited.pdf").add_filter("PDF documents", &["pdf"]).save_file();
  if control.cancelled(){return Ok(None);}
  let Some(mut path)=path else{return Ok(None)};
  if path.extension().is_none(){path.set_extension("pdf");}
  let saved=engine.export_pdf_with_options(request,bookmarks,&path,security,metadata,control,report).map_err(|e|e.to_string())?;
  Ok(saved.then(||path.to_string_lossy().into_owned()))
 }).await.map_err(|e|format!("PDF save task failed: {e}"))?
}

#[cfg(test)]
mod tests{
 use super::*;
 #[test]fn expired_pending_job_cannot_start_without_another_registration(){
  let jobs=PdfSaveJobs::default();let id=jobs.register("main").unwrap();
  let control={let mut state=jobs.0.lock().unwrap();let job=state.as_mut().unwrap();job.created=Instant::now()-Duration::from_secs(121);job.control.clone()};
  assert!(jobs.begin("other",&id).is_err());assert!(!control.cancelled(),"Another window cannot cancel this pending job");
  assert!(jobs.begin("main",&id).is_err());assert!(control.cancelled());assert!(jobs.register("main").is_ok());
 }
 #[test]fn expiration_never_reaps_a_running_job(){let jobs=PdfSaveJobs::default();let id=jobs.register("main").unwrap();let(control,guard)=jobs.begin("main",&id).unwrap();jobs.0.lock().unwrap().as_mut().unwrap().created=Instant::now()-Duration::from_secs(121);assert!(jobs.begin("main",&id).is_err());assert!(jobs.register("main").is_err());assert!(!control.cancelled());drop(guard);assert!(jobs.register("main").is_ok());}
 #[test]fn registration_is_bounded_and_bound_to_the_window(){let jobs=PdfSaveJobs::default();let id=jobs.register("main").unwrap();assert!(jobs.register("main").is_err());assert!(jobs.begin("other",&id).is_err());assert!(jobs.cancel("other",&id).is_err());assert!(jobs.cancel("main",&id).unwrap());assert!(jobs.register("main").is_ok());}
 #[test]fn duplicate_start_cannot_clear_the_running_job(){let jobs=PdfSaveJobs::default();let id=jobs.register("main").unwrap();let(control,guard)=jobs.begin("main",&id).unwrap();assert!(jobs.begin("main",&id).is_err());assert!(jobs.register("main").is_err());assert!(jobs.cancel("main",&id).unwrap());assert!(control.cancelled());assert!(jobs.register("main").is_err());drop(guard);assert!(jobs.register("main").is_ok());}
 #[test]fn window_destruction_cancels_running_and_releases_unstarted_jobs(){let jobs=PdfSaveJobs::default();let id=jobs.register("main").unwrap();jobs.abandon("main");assert!(jobs.begin("main",&id).is_err());let id=jobs.register("main").unwrap();let(control,guard)=jobs.begin("main",&id).unwrap();jobs.abandon("main");assert!(control.cancelled());drop(guard);assert!(jobs.register("main").is_ok());}
}
