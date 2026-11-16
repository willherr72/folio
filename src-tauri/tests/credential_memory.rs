//! Dedicated process: observe initialized allocation bytes immediately before free.
//! The allocator zero-initializes every allocation, so inspecting full capacity
//! reads defined bytes. It never observes freed memory or real user credentials.
use std::{alloc::{GlobalAlloc,Layout,System},ffi::OsString,sync::atomic::{AtomicBool,AtomicUsize,Ordering}};
const SENTINEL:&[u8]=b"FolioSecretWipeProbe_68d4c12a";
static ACTIVE:AtomicBool=AtomicBool::new(false);
static PLAINTEXT_FREES:AtomicUsize=AtomicUsize::new(0);
struct ObservedAllocator;
unsafe impl GlobalAlloc for ObservedAllocator{
 unsafe fn alloc(&self,layout:Layout)->*mut u8{
  let p=unsafe{System.alloc(layout)};
  if !p.is_null(){unsafe{std::ptr::write_bytes(p,0,layout.size())};}p
 }
 unsafe fn alloc_zeroed(&self,layout:Layout)->*mut u8{unsafe{System.alloc_zeroed(layout)}}
 unsafe fn dealloc(&self,p:*mut u8,layout:Layout){
  if ACTIVE.load(Ordering::SeqCst)&&layout.size()>=SENTINEL.len(){
   // Every allocation starts fully initialized above. The default GlobalAlloc
   // realloc also uses our initialized allocation/deallocation methods.
   let bytes=unsafe{std::slice::from_raw_parts(p,layout.size())};
   if bytes.windows(SENTINEL.len()).any(|v|v==SENTINEL){PLAINTEXT_FREES.fetch_add(1,Ordering::SeqCst);}
  }
  unsafe{System.dealloc(p,layout)};
 }
}
#[global_allocator]static ALLOCATOR:ObservedAllocator=ObservedAllocator;

#[test]fn credential_decoder_zeroes_owned_buffers_on_errors_and_escaped_success(){
 // Positive control: the observer must catch an intentionally unmanaged buffer.
 PLAINTEXT_FREES.store(0,Ordering::SeqCst);ACTIVE.store(true,Ordering::SeqCst);
 let control=SENTINEL.to_vec();std::hint::black_box(&control);drop(control);
 ACTIVE.store(false,Ordering::SeqCst);assert!(PLAINTEXT_FREES.swap(0,Ordering::SeqCst)>0);
 let cases:[(&[u8],i32);5]=[
  (br#"["FolioSecretWipeProbe_68d4c12a"]"#,2),
  (br#"["FolioSecretWipeProbe_68d4c12a",1]"#,2),
  (br#"["bad\nline","FolioSecretWipeProbe_68d4c12a"]"#,2),
  (br#"["\u0046olioSecretWipeProbe_68d4c12a"]"#,2),
  (br#"["\u0046olioSecretWipeProbe_68d4c12a",null]"#,1),
 ];
 let mut frees=Vec::new();
 for(packet,expected_status)in cases{
  let args:[OsString;10]=["compose-pages","--input","missing-A.pdf","--input","missing-B.pdf","--out","never-created.pdf","--passwords-stdin","--engine","credential-probe-unavailable-68d4c12a.dll"].map(Into::into);
  let(mut out,mut err)=(Vec::new(),Vec::new());PLAINTEXT_FREES.store(0,Ordering::SeqCst);ACTIVE.store(true,Ordering::SeqCst);
  let status=folio_engine::cli::run(args,std::hint::black_box(packet),&mut out,&mut err);
  ACTIVE.store(false,Ordering::SeqCst);frees.push(PLAINTEXT_FREES.swap(0,Ordering::SeqCst));
  assert_eq!(status,expected_status);assert!(out.is_empty());assert!(!err.windows(SENTINEL.len()).any(|v|v==SENTINEL));
  let reply:serde_json::Value=serde_json::from_slice(&err).unwrap();assert_eq!(reply["error"]["code"],if expected_status==2{"invalid_arguments"}else{"engine_unavailable"});
 }
 assert_eq!(frees,vec![0;5],"Owned decoded or scratch credential allocations were released with plaintext");
}
