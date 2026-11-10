import {Channel,invoke} from '@tauri-apps/api/core';
import {selectPages} from './page-selection';
import {pageAllows} from './protection';
import type {PagePlan} from './types';
export interface TextExportRequest{pages:PagePlan[];includeFormValues:boolean}
export interface TextExportResult{path:string;pageCount:number;emptyPages:number;skippedFields:number;bytes:number}
export interface TextExportProgress{jobId:string;completed:number;total:number;stage:string}
export function textExportSelection(pages:PagePlan[],mode:'all'|'current'|'range',currentPageId:string|null,range:string,includeFormValues:boolean):TextExportRequest{
 if(pages.length>10000)throw Error('Text export supports at most 10,000 workspace pages.');
 const selected=selectPages(pages,mode,currentPageId,range);
 if(selected.some(page=>!pageAllows(page,'copyContent')))throw Error('Some selected pages do not allow copying content. Choose permitted pages or unlock with the owner password.');
 return {pages:selected,includeFormValues};
}
export const textExportApi={
 register:()=>invoke<string>('register_text_export'),
 cancel:(jobId:string)=>invoke<boolean>('cancel_text_export',{jobId}),
 start:(jobId:string,request:TextExportRequest,onProgress:(event:TextExportProgress)=>void)=>{const progress=new Channel<TextExportProgress>();progress.onmessage=onProgress;return invoke<TextExportResult|null>('export_text',{jobId,request,progress});},
};
export function startTextExport(request:TextExportRequest,onProgress:(event:TextExportProgress)=>void){
 let jobId:string|null=null,cancelled=false,finished=false;
 const result=(async()=>{try{
  jobId=await textExportApi.register();if(cancelled){await textExportApi.cancel(jobId);return null;}
  return await textExportApi.start(jobId,request,event=>{if(!finished&&!cancelled&&event.jobId===jobId)onProgress(event);});
 }finally{finished=true;if(jobId)await textExportApi.cancel(jobId).catch(()=>{});}})();
 return {result,cancel:async()=>{if(finished)return false;cancelled=true;return jobId?textExportApi.cancel(jobId):true;}};
}
