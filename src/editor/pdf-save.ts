import {Channel,invoke} from '@tauri-apps/api/core';
import type {DocumentBookmark,PagePlan} from './types';
import type {ProtectionChange} from './protected-copy';
export type PdfSaveStage='choosingFile'|'preparing'|'checking'|'copying'|'preserving'|'applying'|'renderingEdits'|'preparingFile'|'securing'|'writing'|'finishing';
export interface PdfSaveProgress{jobId:string;stage:PdfSaveStage;completed:number;total:number}
export interface PdfSaveOptions{flatten?:boolean;bookmarks?:DocumentBookmark[];security?:ProtectionChange;metadata?:import('./document-properties').DocumentMetadataEdit}
export interface PdfSaveRequest extends PdfSaveOptions{pages:PagePlan[]}
export interface PdfSaveTask{result:Promise<string|null>;cancel():Promise<boolean>}
export const pdfSaveApi={
 register:()=>invoke<string>('register_pdf_save'),
 cancel:(jobId:string)=>invoke<boolean>('cancel_pdf_save',{jobId}),
 start:(jobId:string,request:PdfSaveRequest,onProgress:(event:PdfSaveProgress)=>void)=>{
  const progress=new Channel<PdfSaveProgress>();progress.onmessage=onProgress;
  return invoke<string|null>('save_pdf',{jobId,request:{pages:request.pages,...(request.flatten?{flatten:true}:{})},...(request.bookmarks!==undefined?{bookmarks:request.bookmarks}:{}),...(request.security?{security:request.security}:{}),...(request.metadata!==undefined?{metadata:request.metadata}:{}),progress});
 },
};
export function startPdfSave(pages:PagePlan[],options:PdfSaveOptions,onProgress:(event:PdfSaveProgress)=>void):PdfSaveTask{
 let jobId:string|null=null,cancelled=false,acceptedCancel=false,finished=false,cancelInFlight:Promise<boolean>|null=null;
 const stages=new Set<PdfSaveStage>(['choosingFile','preparing','checking','copying','preserving','applying','renderingEdits','preparingFile','securing','writing','finishing']);
 const result=(async()=>{try{
  jobId=await pdfSaveApi.register();
  if(typeof jobId!=='string'||!jobId.length)throw Error('PDF save registration is unavailable.');
  if(cancelled){await pdfSaveApi.cancel(jobId);return null;}
  return await pdfSaveApi.start(jobId,{pages,...options},event=>{
   if(!finished&&!cancelled&&event.jobId===jobId&&stages.has(event.stage)&&Number.isInteger(event.completed)&&event.completed>=0&&event.completed<=event.total&&event.total===pages.length)onProgress(event);
  });
 }catch(error){
  // The cancel callback can mutate this while native start is pending.
  await (cancelInFlight as Promise<boolean>|null)?.catch(()=>{});
  if(acceptedCancel)return null;
  throw error;
 }finally{finished=true;if(jobId)await pdfSaveApi.cancel(jobId).catch(()=>{});}})();
 return {result,cancel:async()=>{
  if(finished)return false;
  if(cancelInFlight)return cancelInFlight;
  cancelled=true;if(!jobId)return true;
  const pending=pdfSaveApi.cancel(jobId).then(accepted=>{acceptedCancel=accepted;if(!accepted)cancelled=false;return accepted;},error=>{cancelled=false;throw error;});
  cancelInFlight=pending;
  try{return await pending;}finally{if(cancelInFlight===pending)cancelInFlight=null;}
 }};
}
