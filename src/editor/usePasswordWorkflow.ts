import {useCallback,useEffect,useRef,useState} from "react";
import type {FolioAdapter} from "./adapter";
import type {SourceReplacement} from "./types";
import {discardOpenedDocument} from "./discard-opened-document";
import {awaitOrCancel} from "./await-or-cancel";

export interface PasswordPrompt {mode:"open"|"owner";name:string;busy:boolean;error:string|null}
interface Session {controller:AbortController;answer:((value:string|null)=>void)|null}

/** Passwords live only in the input and the current invocation, never workspace state. */
export function usePasswordWorkflow() {
 const [prompt,setPrompt]=useState<PasswordPrompt|null>(null);
 const [opening,setOpening]=useState(false);
 const active=useRef<Session|null>(null),alive=useRef(true);
 const cancel=useCallback(()=>{
  const session=active.current;if(!session||session.controller.signal.aborted)return false;
  session.controller.abort();session.answer?.(null);session.answer=null;
  if(alive.current)setPrompt(null);
  return true;
 },[]);
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;cancel();};},[cancel]);
 const start=(isOpening:boolean):Session=>{
  if(active.current)throw new Error("Finish the current password request first.");
  const session={controller:new AbortController(),answer:null};active.current=session;
  if(alive.current)setOpening(isOpening);return session;
 };
 const finish=(session:Session)=>{if(active.current===session){active.current=null;if(alive.current){setPrompt(null);setOpening(false);}}};
 const ask=(session:Session,mode:"open"|"owner",name:string,error:string|null):Promise<string|null>=>{
  if(session.controller.signal.aborted||!alive.current)return Promise.resolve(null);
  setPrompt({mode,name,error,busy:false});
  return new Promise(resolve=>{session.answer=resolve;});
 };
 const submit=(password:string)=>{
  const session=active.current,resolve=session?.answer;if(!session||!resolve||session.controller.signal.aborted)return;
  session.answer=null;setPrompt(current=>current?{...current,error:null,busy:true}:null);resolve(password);
 };
 const open=async(adapter:FolioAdapter,prepare?:()=>Promise<void>)=>{
  const session=start(true),signal=session.controller.signal;
  const discard=(document:Awaited<ReturnType<FolioAdapter["openPdf"]>>)=>document?discardOpenedDocument(document,id=>adapter.closeDocument(id)):Promise.resolve();
  try {
   if(prepare){const prepared=await awaitOrCancel(prepare(),signal,()=>{});if(prepared.cancelled)return null;}
   if(signal.aborted||!alive.current)return null;
   const result=await awaitOrCancel(adapter.openPdf({signal,requestPassword:challenge=>ask(session,"open",challenge.name,challenge.incorrect?"That password is incorrect. Try again.":null)}),signal,discard);
   if(result.cancelled)return null;
   const document=result.value;
   if(signal.aborted||!alive.current){void discard(document).catch(()=>{});return null;}
   return document;
  } catch(error) {if(session.controller.signal.aborted||!alive.current)return null;throw error;}
  finally {finish(session);}
 };
 const unlock=async(adapter:FolioAdapter,sourceIds:string[],name:string):Promise<SourceReplacement[]|null>=>{
  if(!adapter.unlockSources)throw new Error("Owner unlock is unavailable.");
  const session=start(false);let error:string|null=null;
  const discard=(result:SourceReplacement[]|null)=>Promise.allSettled((result??[]).map(item=>adapter.closeDocument(item.document.id))).then(()=>{});
  try {
   while(!session.controller.signal.aborted&&alive.current) {
    let password=await ask(session,"owner",name,error);if(password===null)return null;
    let result:SourceReplacement[]|null;
    try {
     const pending=await awaitOrCancel(adapter.unlockSources(sourceIds,password),session.controller.signal,discard);
     if(pending.cancelled)return null;
     result=pending.value;
    }finally {password="";}
    if(session.controller.signal.aborted||!alive.current){void discard(result);return null;}
    if(result)return result;
    error="That owner password is incorrect. It may differ from the password used to open the PDF.";
   }
   return null;
  } catch(error) {if(session.controller.signal.aborted||!alive.current)return null;throw error;}
  finally {finish(session);}
 };
 return {prompt,opening,open,unlock,submit,cancel};
}
