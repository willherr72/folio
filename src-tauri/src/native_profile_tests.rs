use crate::native_profile as profile;
use std::sync::Mutex;
static SERIAL: Mutex<()> = Mutex::new(());

#[test]
fn inactive_spans_do_not_create_a_session() {
    let _lock=SERIAL.lock().unwrap();
    drop(profile::span("inactive"));
    let report=profile::start().unwrap().finish();
    assert!(report.valid); assert!(report.records.is_empty());
}

#[test]
fn nested_spans_have_monotonic_intervals_and_direct_parents() {
    let _lock=SERIAL.lock().unwrap();let session=profile::start().unwrap();
    {let _a=profile::span("outer");{let _b=profile::span("inner");}drop(profile::span("sibling"));}
    let r=session.finish();assert!(r.valid,"{:?}",r.errors);assert_eq!(r.records.len(),3);
    for (i,row) in r.records.iter().enumerate(){assert_eq!(row.id,i);assert!(row.start_ns<=row.end_ns.unwrap());assert!(row.end_ns.unwrap()<=r.elapsed_ns);}
    assert_eq!(r.records[0].parent,None);assert_eq!(r.records[1].parent,Some(0));assert_eq!(r.records[2].parent,Some(0));
    assert!(r.records[0].start_ns<=r.records[1].start_ns);assert!(r.records[1].end_ns.unwrap()<=r.records[2].start_ns);
    assert!(r.records[2].end_ns.unwrap()<=r.records[0].end_ns.unwrap());assert_eq!(r.records[0].label,"outer");
}

#[test]
fn concurrent_start_is_refused_without_disturbing_the_first_session() {
    let _lock=SERIAL.lock().unwrap();let session=profile::start().unwrap();assert!(profile::start().is_err());
    drop(profile::span("original"));assert!(session.finish().valid);
}

#[test]
fn errors_and_unwinding_keep_the_original_result_and_close_the_scope() {
    let _lock=SERIAL.lock().unwrap();let session=profile::start().unwrap();
    let result=(||->Result<(),&str>{let _scope=profile::span("error");Err("original error")})();
    assert_eq!(result,Err("original error"));
    let result=std::panic::catch_unwind(||{let _scope=profile::span("unwind");panic!("original panic")});
    assert!(result.is_err());assert!(session.finish().valid);
}

#[test]
fn unfinished_scope_is_invalid_and_stale_guard_cannot_write_into_a_new_session() {
    let _lock=SERIAL.lock().unwrap();let session=profile::start().unwrap();let stale=profile::span("stale");
    let report=session.finish();assert!(!report.valid);assert_eq!(report.records[0].end_ns,None);
    let session=profile::start().unwrap();drop(stale);drop(profile::span("new"));let report=session.finish();
    assert!(report.valid);assert_eq!(report.records.len(),1);assert_eq!(report.records[0].label,"new");
}

#[test]
fn out_of_order_guard_destruction_invalidates_the_trace() {
    let _lock=SERIAL.lock().unwrap();let session=profile::start().unwrap();
    let outer=profile::span("outer");let inner=profile::span("inner");drop(outer);drop(inner);
    let report=session.finish();assert!(!report.valid);assert!(!report.errors.is_empty());
}

#[test]
fn row_budget_is_bounded_and_overflow_is_not_silently_accepted() {
    let _lock=SERIAL.lock().unwrap();let session=profile::start().unwrap();
    for _ in 0..4097 {drop(profile::span("row"));}
    let report=session.finish();assert!(!report.valid);assert_eq!(report.records.len(),4096);
}

#[test]
fn depth_budget_is_bounded_and_overflow_is_not_silently_accepted() {
    let _lock=SERIAL.lock().unwrap();let session=profile::start().unwrap();
    let mut guards=vec![];for _ in 0..129{guards.push(profile::span("deep"));}
    while let Some(guard)=guards.pop(){drop(guard);}
    let report=session.finish();assert!(!report.valid);assert_eq!(report.records.len(),128);
}

#[test]
fn a_second_worker_thread_cannot_contaminate_the_selected_worker() {
    let _lock=SERIAL.lock().unwrap();let session=profile::start().unwrap();drop(profile::span("owner"));
    std::thread::spawn(||{drop(profile::span("other worker"));}).join().unwrap();
    let report=session.finish();assert!(!report.valid);assert_eq!(report.records.len(),1);
}

#[test]
fn session_controller_may_be_separate_from_the_single_recorded_worker() {
    let _lock=SERIAL.lock().unwrap();let session=profile::start().unwrap();
    std::thread::spawn(||{let _worker=profile::span("worker");drop(profile::span("child"));}).join().unwrap();
    let report=session.finish();assert!(report.valid);assert_eq!(report.records.len(),2);
}

#[test]
fn abandoning_a_session_disables_recording_without_a_stuck_global_owner() {
    let _lock=SERIAL.lock().unwrap();{let _session=profile::start().unwrap();drop(profile::span("old"));}
    drop(profile::span("inactive"));let report=profile::start().unwrap().finish();assert!(report.valid);assert!(report.records.is_empty());
}
