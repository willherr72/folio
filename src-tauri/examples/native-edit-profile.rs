//! Local diagnostic only; neither the desktop nor CLI package builds this example.
use folio_engine::{EngineResult, PdfEngine};
use serde_json::{json, Value};
use sha2::{Digest,Sha256};
use std::{error::Error,fs::{self,OpenOptions},io::Write,path::{Path,PathBuf},time::Instant};

type Result<T> = std::result::Result<T,Box<dyn Error>>;
fn sha(bytes:&[u8])->String {Sha256::digest(bytes).iter().map(|byte|format!("{byte:02x}")).collect()}
fn write(path:&Path,bytes:&[u8])->Result<()> {OpenOptions::new().write(true).create_new(true).open(path)?.write_all(bytes)?;Ok(())}

#[cfg(feature="native-profiling")]
fn trace_json(report:folio_engine::native_profile::Report)->Value {
    json!({"valid":report.valid,"errors":report.errors,"elapsed_ns":report.elapsed_ns,
        "records":report.records.into_iter().map(|r|json!({"id":r.id,"parent":r.parent,"label":r.label,"start_ns":r.start_ns,"end_ns":r.end_ns})).collect::<Vec<_>>()})
}

fn timed<T>(folder:&Path,name:&str,observe:bool,action:impl FnOnce()->EngineResult<T>)->Result<T> {
    #[cfg(feature="native-profiling")]
    let session=if observe {Some(folio_engine::native_profile::start()?)}else{None};
    let started=Instant::now();let result=action();let wall_ns=started.elapsed().as_nanos();
    #[cfg(feature="native-profiling")]
    let trace=session.map(|session|trace_json(session.finish()));
    #[cfg(not(feature="native-profiling"))]
    let trace:Option<Value>=None;
    let record=json!({"operation":name,"observe":observe,"operationSucceeded":result.is_ok(),"error":result.as_ref().err().map(ToString::to_string),"wall_ns":wall_ns as u64,"trace":trace});
    write(&folder.join(format!("{name}.json")),&serde_json::to_vec_pretty(&record)?)?;
    if trace.as_ref().is_some_and(|r|r["valid"]!=true) {return Err("Invalid native timing trace; preserve evidence".into());}
    Ok(result?)
}

fn main()->Result<()> {
    let args=std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len()!=4 {return Err("Usage: native-edit-profile <pdfium.dll> <source.pdf> <new-run-root> <observe|control>".into());}
    let dll=PathBuf::from(&args[0]).canonicalize()?;let source=PathBuf::from(&args[1]).canonicalize()?;
    let folder=PathBuf::from(&args[2]);let observe=match args[3].to_str(){Some("observe")=>true,Some("control")=>false,_=>return Err("Expected observe or control".into())};
    #[cfg(not(feature="native-profiling"))]
    if observe {return Err("Rebuild with --features native-profiling to record subphases".into());}
    fs::create_dir(&folder)?;let folder=folder.canonicalize()?;
    let source_sha=sha(&fs::read(&source)?);let exe=std::env::current_exe()?;
    let identity=json!({"source":source,"sourceSha256":source_sha,"executable":exe,"executableSha256":sha(&fs::read(&exe)?),"renderer":dll,"rendererSha256":sha(&fs::read(&dll)?),"pid":std::process::id(),"observe":observe,"featureEnabled":cfg!(feature="native-profiling"),"cycles":7});
    write(&folder.join("identity.json"),&serde_json::to_vec_pretty(&identity)?)?;
    let engine=PdfEngine::start(&dll)?;
    let original=timed(&folder,"open",observe,||engine.open_document(&source))?;
    let pages=original.pages.len();let order=(0..pages).collect::<Vec<_>>();let mut current=original.id.clone();let mut retained=vec![current.clone()];
    let header="FOLIO DENSE PAGE 0001";let grown=format!("{header} this");
    for i in 1..=7 {
        let before=if i%2==1 {header}else{&grown};let replacement=if i%2==1 {&grown}else{header};
        let runs=timed(&folder,&format!("discover-{i}"),observe,||engine.list_occurrence_text_runs(&current,0))?;
        let matching=runs.runs.iter().filter(|r|r.text==before).collect::<Vec<_>>();
        if matching.len()!=1 || !matching[0].supported {return Err("Expected one supported source header".into());}
        let edited=timed(&folder,&format!("replace-{i}"),observe,||engine.replace_text_occurrence(&current,&order,0,matching[0].object_index,before,replacement,None))?;
        if edited.document.pages.len()!=pages || edited.page_indices!=order {return Err("Changed page occurrence mapping".into());}
        current=edited.document.id;retained.push(current.clone());println!("Completed edit {i}");
    }
    let bytes=timed(&folder,"final-source",observe,||engine.source_bytes(&current))?;
    write(&folder.join("saved.pdf"),&bytes)?;
    let saved_sha=sha(&bytes);drop(bytes);
    for id in retained.iter().rev(){engine.close_document(id)?;}
    if sha(&fs::read(&source)?)!=source_sha {return Err("Original source changed".into());}
    write(&folder.join("result.json"),&serde_json::to_vec_pretty(&json!({"passed":true,"pages":pages,"cycles":7,"retainedSourcesUntilClose":retained.len(),"allSourcesClosed":true,"savedSha256":saved_sha,"sourceSha256":source_sha,"observe":observe,"identity":identity,"scope":"Native worker calls only; no UI, IPC or generalized performance/parity certification"}))?)?;
    println!("Completed seven immutable edits and closed all retained sources");Ok(())
}
