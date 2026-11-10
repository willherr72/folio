//! Opt-in, bounded scalar timings. No document data or operation results enter here.
use std::{marker::PhantomData, rc::Rc, sync::Mutex, thread::ThreadId, time::Instant};

const ROW_LIMIT: usize = 4096;
const DEPTH_LIMIT: usize = 128;
const WRONG_THREAD: u8 = 1;
const ROW_OVERFLOW: u8 = 2;
const DEPTH_OVERFLOW: u8 = 4;
const ORDER: u8 = 8;
const UNCLOSED: u8 = 16;

#[derive(Debug)]
pub struct Record {
    pub id: usize,
    pub parent: Option<usize>,
    pub label: &'static str,
    pub start_ns: u64,
    pub end_ns: Option<u64>,
}

#[derive(Debug)]
pub struct Report {
    pub valid: bool,
    pub errors: Vec<&'static str>,
    pub elapsed_ns: u64,
    pub records: Vec<Record>,
}

struct Trace {
    generation: u64,
    started: Instant,
    owner: Option<ThreadId>,
    stack: Vec<usize>,
    records: Vec<Record>,
    errors: u8,
}
struct State { next: u64, trace: Option<Trace> }
static STATE: Mutex<State> = Mutex::new(State { next: 1, trace: None });

pub struct Session { generation: u64, finished: bool, _local: PhantomData<Rc<()>> }
pub struct Span { record: Option<(u64, usize)>, _local: PhantomData<Rc<()>> }

pub fn start() -> Result<Session, &'static str> {
    let mut state=STATE.lock().unwrap_or_else(|e|e.into_inner());
    if state.trace.is_some() {return Err("Another native timing session is active");}
    let generation=state.next;state.next=state.next.wrapping_add(1).max(1);
    state.trace=Some(Trace {generation,started:Instant::now(),owner:None,
        stack:Vec::with_capacity(DEPTH_LIMIT),records:Vec::with_capacity(ROW_LIMIT),errors:0});
    Ok(Session {generation,finished:false,_local:PhantomData})
}

pub fn span(label: &'static str) -> Span {
    let mut state=STATE.lock().unwrap_or_else(|e|e.into_inner());
    let record=state.trace.as_mut().and_then(|trace| {
        let thread=std::thread::current().id();
        if trace.owner.is_some_and(|owner|owner!=thread) {trace.errors|=WRONG_THREAD;return None;}
        trace.owner=Some(thread);
        if trace.records.len()>=ROW_LIMIT {trace.errors|=ROW_OVERFLOW;return None;}
        if trace.stack.len()>=DEPTH_LIMIT {trace.errors|=DEPTH_OVERFLOW;return None;}
        let id=trace.records.len();
        trace.records.push(Record {id,parent:trace.stack.last().copied(),label,
            start_ns:nanos(trace.started),end_ns:None});
        trace.stack.push(id);Some((trace.generation,id))
    });
    Span {record,_local:PhantomData}
}

fn nanos(started: Instant) -> u64 {started.elapsed().as_nanos().min(u64::MAX as u128) as u64}

impl Drop for Span {
    fn drop(&mut self) {
        let Some((generation,id))=self.record else {return;};
        let mut state=STATE.lock().unwrap_or_else(|e|e.into_inner());
        let Some(trace)=state.trace.as_mut().filter(|trace|trace.generation==generation) else {return;};
        if trace.stack.last()==Some(&id) {trace.stack.pop();}
        else {trace.errors|=ORDER;if let Some(pos)=trace.stack.iter().position(|value|*value==id){trace.stack.remove(pos);}}
        if let Some(row)=trace.records.get_mut(id) {row.end_ns=Some(nanos(trace.started));}
        else {trace.errors|=ORDER;}
    }
}

impl Session {
    pub fn finish(mut self) -> Report {
        let mut state=STATE.lock().unwrap_or_else(|e|e.into_inner());
        // Only this exclusive controller can remove its generation.
        let mut trace=state.trace.take().expect("Live timing session");
        debug_assert_eq!(trace.generation,self.generation);
        if !trace.stack.is_empty() {trace.errors|=UNCLOSED;}
        let elapsed_ns=nanos(trace.started);
        let errors=[(WRONG_THREAD,"wrong-worker-thread"),(ROW_OVERFLOW,"record-budget-exceeded"),
            (DEPTH_OVERFLOW,"depth-budget-exceeded"),(ORDER,"non-nested-guard-destruction"),(UNCLOSED,"unclosed-scope")]
            .into_iter().filter_map(|(flag,name)|(trace.errors&flag!=0).then_some(name)).collect();
        self.finished=true;
        Report {valid:trace.errors==0,errors,elapsed_ns,records:trace.records}
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if self.finished {return;}
        let mut state=STATE.lock().unwrap_or_else(|e|e.into_inner());
        if state.trace.as_ref().is_some_and(|trace|trace.generation==self.generation) {state.trace=None;}
    }
}
