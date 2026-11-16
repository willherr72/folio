export type CancellableResult<T>={cancelled:true}|{cancelled:false;value:T};

/** Stop waiting on abort; the work still owns its value until delivered or disposed. */
export function awaitOrCancel<T>(work:Promise<T>,signal:AbortSignal,dispose:(value:T)=>void|Promise<void>):Promise<CancellableResult<T>> {
 return new Promise((resolve,reject)=>{
  let finished=false,cancelled=false;
  const detach=()=>signal.removeEventListener("abort",abort);
  const abort=()=>{
   if(finished)return;
   finished=true;cancelled=true;detach();resolve({cancelled:true});
  };
  signal.addEventListener("abort",abort,{once:true});
  if(signal.aborted)abort();
  void work.then(value=>{
   if(cancelled){void Promise.resolve().then(()=>dispose(value)).catch(()=>{});return;}
   finished=true;detach();resolve({cancelled:false,value});
  },error=>{
   if(cancelled)return;
   finished=true;detach();reject(error);
  });
 });
}
