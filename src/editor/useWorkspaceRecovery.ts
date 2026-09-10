import {useCallback,useEffect,useRef,useState,type Dispatch,type SetStateAction,type MutableRefObject} from "react";
import {nativeAdapter} from "./adapter";
import {recoveryApi,RecoveryQueue,restoreWorkspace,snapshotWorkspace,type RecoverySnapshot} from "./recovery";
import type {Workspace,ScrollPosition} from "./workspace";
const message=(error:unknown)=>error instanceof Error?error.message:String(error);
async function releasePending(snapshot:RecoverySnapshot|null){
 if(snapshot)await Promise.allSettled([...new Set(snapshot.tabs.flatMap(tab=>tab.document.pages.map(page=>page.sourceId)))].map(id=>nativeAdapter.closeDocument(id)));
}
export function useWorkspaceRecovery(desktop:boolean,workspace:Workspace,setWorkspace:Dispatch<SetStateAction<Workspace>>,positions:MutableRefObject<Map<string,ScrollPosition>>){
 const [ready,setReady]=useState(!desktop),[pending,setPending]=useState<RecoverySnapshot|null>(null),[error,setError]=useState<string|null>(null);
 const [working,setWorking]=useState(false),[warning,setWarning]=useState<string|null>(null),[enabled,setEnabled]=useState(desktop);
 const latest=useRef(workspace);latest.current=workspace;
 const transaction=useRef<Workspace|null>(null);if(transaction.current===workspace)transaction.current=null;
 const queue=useRef<RecoveryQueue|null>(null);
 const load=useRef<Promise<RecoverySnapshot|null>|null>(null);
 useEffect(()=>{
  if(!desktop)return;
  let active=true;
  load.current??=recoveryApi.load();
  load.current.then(value=>{if(!active)return;if(value?.tabs.length)setPending(value);else setReady(true);})
   .catch(value=>{if(active)setError(message(value));});
  return()=>{active=false;};
 },[desktop]);
 useEffect(()=>{
  if(!ready||!enabled)return;
  const next=new RecoveryQueue(recoveryApi,value=>setWarning("Recovery could not save your latest changes: "+message(value)));
  queue.current=next;
  return()=>{next.dispose();if(queue.current===next)queue.current=null;};
 },[ready,enabled]);
 const schedule=useCallback(()=>queue.current?.schedule(snapshotWorkspace(transaction.current??latest.current,positions.current)),[positions]);
 useEffect(()=>{if(ready)schedule();},[workspace,ready,schedule]);
 const checkpoint=useCallback(async(next:Workspace)=>{
  transaction.current=next;
  try{await queue.current?.flush(snapshotWorkspace(next,positions.current));}
  catch(error){transaction.current=null;queue.current?.schedule(snapshotWorkspace(latest.current,positions.current));throw error;}
 },[positions]);
 const finishClose=useCallback(async()=>{await queue.current?.close();},[]);
 const restore=useCallback(async()=>{
  if(pending&&!error){
   const restored=restoreWorkspace(pending,nativeAdapter);
   positions.current=restored.positions;setWorkspace(restored.workspace);setPending(null);setReady(true);return;
  }
  setWorking(true);
  try{await releasePending(pending);setPending(null);const value=await recoveryApi.load();setError(null);if(value?.tabs.length)setPending(value);else setReady(true);}
  catch(value){setError(message(value));}finally{setWorking(false);}
 },[pending,error,positions,setWorkspace]);
 const discard=useCallback(async()=>{
  setWorking(true);
  try{
   await recoveryApi.clear();
   await releasePending(pending);
   setPending(null);setError(null);setReady(true);
  }catch(value){setError(message(value));}finally{setWorking(false);}
 },[pending]);
 const skip=useCallback(async()=>{
  setWorking(true);await releasePending(pending);setEnabled(false);setWarning("Recovery is disabled for this session: "+error);
  setError(null);setPending(null);setReady(true);setWorking(false);
 },[error,pending]);
 return {ready,pending,error,working,warning,restore,discard,skip,schedule,checkpoint,finishClose};
}
