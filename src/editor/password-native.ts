import {invoke} from "@tauri-apps/api/core";
import type {DocumentInfo} from "./types";
import {discardOpenedDocument} from "./discard-opened-document";

export interface PasswordChallenge {status:"passwordRequired";challengeId:string;name:string;incorrect:boolean}
type OpenOutcome={status:"opened";document:DocumentInfo}|PasswordChallenge;
export interface OpenPdfOptions {
 signal:AbortSignal;
 requestPassword(challenge:PasswordChallenge):Promise<string|null>;
}

/** A late reply owns a native source even after cancellation: release it explicitly. */
export async function openProtectedPdf({signal,requestPassword}:OpenPdfOptions):Promise<DocumentInfo|null> {
 if(signal.aborted)return null;
 let token:string|null=null;
 const cancelToken=()=>{if(token)void invoke("cancel_open_pdf",{challengeId:token}).catch(()=>{});};
 signal.addEventListener("abort",cancelToken);
 try {
  let outcome=await invoke<OpenOutcome|null>("begin_open_pdf");
  while(outcome?.status==="passwordRequired") {
   token=outcome.challengeId;
   if(signal.aborted)return null;
   let password=await requestPassword(outcome);
   if(password===null||signal.aborted)return null;
   try {outcome=await invoke<OpenOutcome>("submit_open_password",{challengeId:token,password});}
   finally {password="";}
  }
  token=null;
  if(outcome?.status!=="opened")return null;
  if(signal.aborted){await discardOpenedDocument(outcome.document,sourceId=>invoke<void>("close_document",{sourceId}));return null;}
  return outcome.document;
 } finally {
  signal.removeEventListener("abort",cancelToken);
  if(token)await invoke("cancel_open_pdf",{challengeId:token}).catch(()=>{});
 }
}
