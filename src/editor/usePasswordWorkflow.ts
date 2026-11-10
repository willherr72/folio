import {useCallback,useEffect,useRef,useState} from "react";
import type {FolioAdapter} from "./adapter";
import type {SourceReplacement} from "./types";
import {discardOpenedDocument} from "./discard-opened-document";

export interface PasswordPrompt {mode:"open"|"owner";name:string;busy:boolean;error:string|null}
interface Session {controller:AbortController;answer:((value:string|null)=>void)|null}

/** Passwords live only in the input and the current invocation, never workspace state. */
export function usePasswordWorkflow() {
 const [prompt,setPrompt]=useState<PasswordPrompt|null>(null);
 const active=useRef<Session|null>(null),alive=useRef(true);
 const cancel=useCallback(()=>{
  const session=active.current;if(!session)return;
  session.controller.abort();session.answer?.(null);session.answer=null;
  if(alive.current)setPrompt(null);
 },[]);
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;cancel();};},[cancel]);
 const start=():Session=>{
  if(active.current)throw new Error("Finish the current password request first.");
  const session={controller:new AbortController(),answer:null};active.current=session;return session;
 };
 const finish=(session:Session)=>{if(active.current===session){active.current=null;if(alive.current)setPrompt(null);}};
 const ask=(session:Session,mode:"open"|"owner",name:string,error:string|null):Promise<string|null>=>{
  if(session.controller.signal.aborted||!alive.current)return Promise.resolve(null);
  setPrompt({mode,name,error,busy:false});
  return new Promise(resolve=>{session.answer=resolve;});
 };
 const submit=(password:string)=>{
  const session=active.current,resolve=session?.answer;if(!session||!resolve||session.controller.signal.aborted)return;
  session.answer=null;setPrompt(current=>current?{...current,error:null,busy:true}:null);resolve(password);
 };
 const open=async(adapter:FolioAdapter)=>{
  const session=start();
  try {
   const document=await adapter.openPdf({signal:session.controller.signal,requestPassword:challenge=>ask(session,"open",challenge.name,challenge.incorrect?"That password is incorrect. Try again.":null)});
   if(session.controller.signal.aborted||!alive.current){if(document)await discardOpenedDocument(document,id=>adapter.closeDocument(id));return null;}
   return document;
  } catch(error) {if(session.controller.signal.aborted||!alive.current)return null;throw error;}
  finally {finish(session);}
 };
 const unlock=async(adapter:FolioAdapter,sourceIds:string[],name:string):Promise<SourceReplacement[]|null>=>{
  if(!adapter.unlockSources)throw new Error("Owner unlock is unavailable.");
  const session=start();let error:string|null=null;
  try {
   while(!session.controller.signal.aborted&&alive.current) {
    let password=await ask(session,"owner",name,error);if(password===null)return null;
    let result:SourceReplacement[]|null;
    try {result=await adapter.unlockSources(sourceIds,password);}finally {password="";}
    if(session.controller.signal.aborted||!alive.current){if(result)await Promise.allSettled(result.map(item=>adapter.closeDocument(item.document.id)));return null;}
    if(result)return result;
    error="That owner password is incorrect. It may differ from the password used to open the PDF.";
   }
   return null;
  } catch(error) {if(session.controller.signal.aborted||!alive.current)return null;throw error;}
  finally {finish(session);}
 };
 return {prompt,open,unlock,submit,cancel};
}
