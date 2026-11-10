import {Channel,invoke} from "@tauri-apps/api/core";
import {bookmarkPage} from "./bookmarks";
import {canArrangePages} from "./protection";
import {selectPages} from "./page-selection";
import type {DocumentBookmark,PagePlan} from "./types";

export type SplitMode="every"|"after"|"bookmarks";
export interface SplitPdfPart{start:number;end:number}
export interface SplitPdfRequest{pages:PagePlan[];parts:SplitPdfPart[];bookmarks?:DocumentBookmark[];metadata?:import('./document-properties').DocumentMetadataEdit}
export interface SplitPdfResult{folder:string;count:number}
export interface SplitPdfProgress{jobId:string;stage:string;completed:number;total:number}
export function splitPdfSelection(pages:PagePlan[],mode:SplitMode,input:string,bookmarks?:DocumentBookmark[],metadata?:import('./document-properties').DocumentMetadataEdit){
 if(!pages.length||pages.length>10000)throw Error("Splitting supports 1 to 10,000 workspace pages.");
 if(!canArrangePages(pages))throw Error("Some pages do not allow document assembly. Unlock their source with its owner password first.");
 const boundaries=new Set<number>();
 if(mode==="every"){
  const size=input.trim()?Number(input):NaN;
  if(!Number.isInteger(size)||size<1||size>10000)throw Error("Enter a whole number of pages from 1 to 10,000.");
  for(let i=size;i<pages.length;i+=size)boundaries.add(i);
 }else if(mode==="after"){
  if(pages.length===1)throw Error("A one-page document has no internal split boundaries.");
  const selected=selectPages(pages.slice(0,-1),"range",null,input);
  const indexes=new Map(pages.map((p,i)=>[p.id,i+1]));
  for(const p of selected)boundaries.add(indexes.get(p.id)!);
 }else{
  const indexes=new Map(pages.map((p,i)=>[p.id,i]));
  for(const bookmark of bookmarks??[]){if(bookmark.level!==0)continue;const target=bookmarkPage(bookmark);const index=target===null?undefined:indexes.get(target);if(index!==undefined&&index>0)boundaries.add(index);}
  if(!boundaries.size)throw Error("No top-level bookmark defines a boundary after page 1. Choose another split method.");
 }
 if(boundaries.size>=1000)throw Error("A batch can contain at most 1,000 PDFs. Choose larger parts or fewer boundaries.");
 const ends=[...boundaries].sort((a,b)=>a-b);ends.push(pages.length);
 const parts=ends.map((end,i)=>({start:i?ends[i-1]:0,end}));
 return {request:{pages,parts,...(bookmarks!==undefined?{bookmarks}:{}),...(metadata!==undefined?{metadata}:{})} as SplitPdfRequest,parts:parts.map((part,i)=>({...part,name:`part-${String(i+1).padStart(4,"0")}.pdf`}))};
}
export const splitPdfApi={
 register:()=>invoke<string>("register_pdf_split"),
 cancel:(jobId:string)=>invoke<boolean>("cancel_pdf_split",{jobId}),
 start:(jobId:string,request:SplitPdfRequest,onProgress:(event:SplitPdfProgress)=>void)=>{const progress=new Channel<SplitPdfProgress>();progress.onmessage=onProgress;return invoke<SplitPdfResult|null>("split_pdf",{jobId,request,progress});},
};
export function startPdfSplit(request:SplitPdfRequest,onProgress:(event:SplitPdfProgress)=>void){
 let jobId:string|null=null,cancelled=false,finished=false;
 const result=(async()=>{try{
  jobId=await splitPdfApi.register();
  if(cancelled){await splitPdfApi.cancel(jobId);return null;}
  return await splitPdfApi.start(jobId,request,event=>{if(!finished&&!cancelled&&event.jobId===jobId)onProgress(event);});
 }finally{finished=true;if(jobId)await splitPdfApi.cancel(jobId).catch(()=>{});}})();
 return {result,cancel:async()=>{if(finished)return false;cancelled=true;return jobId?splitPdfApi.cancel(jobId):true;}};
}
