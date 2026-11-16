//! Registered-window ownership for pending desktop replacement candidates.
use crate::{PdfEngine,PagePlan,WorkspaceReplacementIntent,WorkspaceReplacementPlan,WorkspaceReplacementResult,ReplacementPlanningControl};
use crate::engine::workspace_replacement::{WorkspaceReplacementSnapshot,MAX_PENDING_BYTES};
use serde::Serialize;
use std::collections::{HashMap,HashSet};
use std::sync::{Arc,Mutex,atomic::{AtomicBool,AtomicUsize,Ordering}};
use std::time::{Duration,Instant};
type Result<T>=std::result::Result<T,String>;
const TTL:Duration=Duration::from_secs(900);
const MAX_REPLY:usize=4*1024*1024-16*1024;
#[derive(Clone)]
pub(crate) struct WorkspaceReplacementHost(Arc<Host>);
struct Host{engine:PdfEngine,windows:Mutex<HashMap<String,Arc<Window>>>,bytes:Arc<AtomicUsize>,sessions:Arc<AtomicUsize>}
struct Window{alive:AtomicBool,data:Mutex<WindowData>,controls:Mutex<HashMap<String,ReplacementPlanningControl>>}
#[derive(Default)]
struct WindowData{sessions:HashMap<String,Session>,adopted:HashSet<String>}
struct Session{snapshot:WorkspaceReplacementSnapshot,plan:Option<WorkspaceReplacementPlan>,preview:Option<Pending>,touched:Instant,_slot:Reservation}
struct Reservation{counter:Arc<AtomicUsize>,size:usize}
impl Drop for Reservation{fn drop(&mut self){self.counter.fetch_sub(self.size,Ordering::AcqRel);}}
fn reserve(counter:&Arc<AtomicUsize>,size:usize,limit:usize)->Result<Reservation>{
    counter.fetch_update(Ordering::AcqRel,Ordering::Acquire,|current|current.checked_add(size).filter(|next|*next<=limit)).map_err(|_|"Close or cancel another replacement session before continuing.")?;
    Ok(Reservation{counter:counter.clone(),size})
}
struct Pending{engine:PdfEngine,id:String,result:Option<WorkspaceReplacementResult>,affected:Vec<String>,created:Instant,_bytes:Reservation}
impl Drop for Pending{fn drop(&mut self){if let Some(result)=self.result.take(){for source in result.sources{let _=self.engine.close_document(&source.document.id);}}}}
#[derive(Clone)]
pub(crate) struct ReplacementWindow{host:Arc<Host>,window:Arc<Window>}
#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct WorkspaceReplacementPreview {
    pub preview_id:String,pub result:WorkspaceReplacementResult,pub affected_page_ids:Vec<String>,pub expires_in_seconds:u32,
}
fn unavailable()->String{"Workspace replacement session is unavailable. Reopen Replace and try again.".into()}
fn bounded(value:&impl Serialize)->Result<()>{let mut writer=crate::pdf_output::BoundedWriter::new(MAX_REPLY);serde_json::to_writer(&mut writer,value).map_err(|_|unavailable())?;writer.finish().map(|_|()).map_err(|_|unavailable())}
impl WorkspaceReplacementHost{
    pub(crate) fn new(engine:PdfEngine)->Self{Self(Arc::new(Host{engine,windows:Mutex::new(HashMap::new()),bytes:Arc::new(AtomicUsize::new(0)),sessions:Arc::new(AtomicUsize::new(0))}))}
    pub(crate) fn register_window(&self,label:&str)->Result<()>{
        let mut windows=self.0.windows.lock().map_err(|_|unavailable())?;
        if windows.contains_key(label)||windows.len()>=16{return Err(unavailable());}
        windows.insert(label.into(),Arc::new(Window{alive:AtomicBool::new(true),data:Mutex::new(WindowData::default()),controls:Mutex::new(HashMap::new())}));Ok(())
    }
    pub(crate) fn capture(&self,label:&str)->Result<ReplacementWindow>{
        let window=self.0.windows.lock().map_err(|_|unavailable())?.get(label).filter(|w|w.alive.load(Ordering::Acquire)).cloned().ok_or_else(unavailable)?;
        Ok(ReplacementWindow{host:self.0.clone(),window})
    }
    pub(crate) fn detach(&self,label:&str)->Option<ReplacementWindow>{
        let window=self.0.windows.lock().unwrap_or_else(|e|e.into_inner()).remove(label)?;window.alive.store(false,Ordering::Release);
        for control in window.controls.lock().unwrap_or_else(|e|e.into_inner()).values(){control.cancel();}
        Some(ReplacementWindow{host:self.0.clone(),window})
    }
    pub(crate) fn forget_document(&self,id:&str){
        let windows=self.0.windows.lock().unwrap_or_else(|e|e.into_inner()).values().cloned().collect::<Vec<_>>();
        for window in windows{window.data.lock().unwrap_or_else(|e|e.into_inner()).adopted.remove(id);}
    }
}
impl ReplacementWindow{
    fn control(&self,session:&str)->Result<ReplacementPlanningControl>{self.window.controls.lock().map_err(|_|unavailable())?.get(session).cloned().ok_or_else(unavailable)}
    fn clear_locked(&self,data:&mut WindowData){
        for control in self.window.controls.lock().unwrap_or_else(|e|e.into_inner()).values(){control.cancel();}
        data.sessions.clear();self.window.controls.lock().unwrap_or_else(|e|e.into_inner()).clear();
        for id in data.adopted.drain(){let _=self.host.engine.close_document(&id);}
    }
    fn run<T>(&self,keep:Option<&str>,work:impl FnOnce(&mut WindowData)->Result<T>)->Result<T>{
        if !self.window.alive.load(Ordering::Acquire){return Err(unavailable());}
        let mut data=self.window.data.lock().map_err(|_|unavailable())?;
        if !self.window.alive.load(Ordering::Acquire){self.clear_locked(&mut data);return Err(unavailable());}
        let expired=data.sessions.iter().filter(|(id,s)|Some(id.as_str())!=keep&&s.touched.elapsed()>=TTL).map(|(id,_)|id.clone()).collect::<Vec<_>>();
        for id in expired{data.sessions.remove(&id);self.window.controls.lock().map_err(|_|unavailable())?.remove(&id);}
        let result=work(&mut data);
        if !self.window.alive.load(Ordering::Acquire){self.clear_locked(&mut data);return Err(unavailable());}result
    }
    pub(crate) fn begin(&self,pages:Vec<PagePlan>)->Result<String>{
        self.run(None,|data|{
            if data.sessions.len()>=4{return Err(unavailable());}let slot=reserve(&self.host.sessions,1,16)?;let id=uuid::Uuid::new_v4().to_string();let control=ReplacementPlanningControl::default();
            self.window.controls.lock().map_err(|_|unavailable())?.insert(id.clone(),control.clone());
            let snapshot=self.host.engine.capture_workspace_replacement(pages,control.clone()).map_err(|e|e.to_string());
            match snapshot{
                Ok(snapshot)=>{if let Err(error)=control.check(){self.window.controls.lock().map_err(|_|unavailable())?.remove(&id);return Err(error.to_string());}data.sessions.insert(id.clone(),Session{snapshot,plan:None,preview:None,touched:Instant::now(),_slot:slot});Ok(id)}
                Err(error)=>{self.window.controls.lock().map_err(|_|unavailable())?.remove(&id);Err(error)}
            }
        })
    }
    pub(crate) fn plan(&self,session:&str,intent:&WorkspaceReplacementIntent)->Result<WorkspaceReplacementPlan>{
        self.run(Some(session),|data|{
            let state=data.sessions.get_mut(session).ok_or_else(unavailable)?;state.preview=None;state.plan=None;
            let control=ReplacementPlanningControl::default();self.window.controls.lock().map_err(|_|unavailable())?.insert(session.into(),control.clone());
            let plan=self.host.engine.plan_workspace_replacement(&state.snapshot.pages,intent,control.clone()).map_err(|e|e.to_string())?;control.check().map_err(|e|e.to_string())?;bounded(&plan)?;
            state.touched=Instant::now();state.plan=Some(plan.clone());Ok(plan)
        })
    }
    pub(crate) fn preview(&self,session:&str,plan_id:&str)->Result<WorkspaceReplacementPreview>{
        self.run(Some(session),|data|{
            let state=data.sessions.get_mut(session).ok_or_else(unavailable)?;let plan=state.plan.as_ref().filter(|p|p.plan_id==plan_id&&p.can_apply).ok_or_else(unavailable)?;
            let control=self.control(session)?;control.check().map_err(|e|e.to_string())?;state.preview=None;
            let result=self.host.engine.preview_workspace_replacement(&state.snapshot.pages,plan,control.clone()).map_err(|e|e.to_string())?;
            let reservation=reserve(&self.host.bytes,result.retained_bytes,MAX_PENDING_BYTES);
            let bytes=match reservation{Ok(bytes)=>bytes,Err(error)=>{for source in result.sources{let _=self.host.engine.close_document(&source.document.id);}return Err(error);}};
            let pending=Pending{engine:self.host.engine.clone(),id:uuid::Uuid::new_v4().to_string(),result:Some(result),affected:plan.affected_page_ids.clone(),created:Instant::now(),_bytes:bytes};
            control.check().map_err(|e|e.to_string())?;
            let reply=WorkspaceReplacementPreview{preview_id:pending.id.clone(),result:pending.result.as_ref().ok_or_else(unavailable)?.clone(),affected_page_ids:pending.affected.clone(),expires_in_seconds:900};bounded(&reply)?;
            state.touched=Instant::now();state.preview=Some(pending);Ok(reply)
        })
    }
    pub(crate) fn apply(&self,session:&str,preview_id:&str)->Result<WorkspaceReplacementResult>{
        self.run(Some(session),|data|{
            let control=self.control(session)?;control.check().map_err(|e|e.to_string())?;
            let state=data.sessions.get_mut(session).ok_or_else(unavailable)?;
            if state.preview.as_ref().is_some_and(|p|p.created.elapsed()>=TTL){state.preview=None;return Err("This preview expired. Preview the replacements again.".into());}
            let pending=state.preview.as_ref().filter(|p|p.id==preview_id).ok_or_else(unavailable)?;
            let sources=pending.result.as_ref().ok_or_else(unavailable)?.sources.iter().map(|s|s.document.id.clone()).collect::<Vec<_>>();
            if data.adopted.len()+sources.len()>4096{return Err("Close unused replacement results before applying more changes.".into());}
            let originals=state.snapshot.pages.iter().map(|p|p.source_id.clone()).collect::<HashSet<_>>().into_iter().collect();
            self.host.engine.check_workspace_replacement_sources(originals,sources.clone()).map_err(|e|e.to_string())?;control.check().map_err(|e|e.to_string())?;
            let mut pending=state.preview.take().ok_or_else(unavailable)?;let result=pending.result.take().ok_or_else(unavailable)?;
            data.adopted.extend(sources);data.sessions.remove(session);self.window.controls.lock().map_err(|_|unavailable())?.remove(session);Ok(result)
        })
    }
    /// Cancellation never waits for the window's PDF-operation mutex.
    pub(crate) fn cancel(&self,session:&str)->Result<()>{self.control(session)?.cancel();Ok(())}
    pub(crate) fn discard_cancelled(&self,session:&str)->Result<()>{
        self.run(Some(session),|data|{
            if self.control(session).is_ok_and(|control|control.check().is_err()){
                if let Some(state)=data.sessions.get_mut(session){state.preview=None;state.plan=None;}
            }Ok(())
        })
    }
    pub(crate) fn release(&self,session:&str)->Result<()>{
        let _=self.cancel(session);self.run(None,|data|{data.sessions.remove(session);self.window.controls.lock().map_err(|_|unavailable())?.remove(session);Ok(())})
    }
    pub(crate) fn cleanup(&self){self.window.alive.store(false,Ordering::Release);self.clear_locked(&mut self.window.data.lock().unwrap_or_else(|e|e.into_inner()));}
}
#[cfg(test)]
#[path="workspace_replacement_host_tests.rs"]mod tests;
