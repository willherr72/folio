import {Channel,invoke} from "@tauri-apps/api/core";
import {pageVisibleRect} from "./geometry";
import {selectPages} from "./page-selection";
import {pageAllows} from "./protection";
import type {PagePlan} from "./types";
export interface PageImageOptions{format:"png"|"jpeg";dpi:72|150|300|600;jpegQuality:number}
export interface PageImageRequest{pages:PagePlan[];pageNumbers:number[];workspacePageCount:number;options:PageImageOptions}
export interface PageImageResult{folder:string;count:number}
export interface PageImageProgress{jobId:string;stage:string;completed:number;total:number}
export function imageExportSelection(pages:PagePlan[],mode:"all"|"current"|"range",currentPageId:string|null,range:string,options:PageImageOptions){
 if(![72,150,300,600].includes(options.dpi)||!Number.isInteger(options.jpegQuality)||options.jpegQuality<1||options.jpegQuality>100)throw Error("Choose a valid resolution and JPEG quality from 1 to 100.");
 if(pages.length>10000)throw Error("Image export supports at most 10,000 workspace pages.");
 const selected=selectPages(pages,mode,currentPageId,range);
 if(selected.some(page=>!pageAllows(page,"copyContent")))throw Error("Some selected pages do not allow copying content. Choose permitted pages or unlock the document with its owner password.");
 const sizes=selected.map(page=>{const box=pageVisibleRect(page),width=Math.max(1,Math.round(box.width*options.dpi/72)),height=Math.max(1,Math.round(box.height*options.dpi/72));
  if(!Number.isFinite(width)||!Number.isFinite(height)||box.width<=0||box.height<=0||width>12000||height>12000||width*height>24000000)throw Error(`Page ${pages.indexOf(page)+1} exceeds the image size limit. Choose a lower DPI or a smaller crop.`);
  return {width,height};});
 return {request:{pages:selected,pageNumbers:selected.map(page=>pages.indexOf(page)+1),workspacePageCount:pages.length,options},firstSize:sizes[0]};
}
export const imageExportApi={
 register:()=>invoke<string>("register_image_export"),
 cancel:(jobId:string)=>invoke<boolean>("cancel_image_export",{jobId}),
 start:(jobId:string,request:PageImageRequest,onProgress:(event:PageImageProgress)=>void)=>{const progress=new Channel<PageImageProgress>();progress.onmessage=onProgress;return invoke<PageImageResult|null>("export_page_images",{jobId,request,progress});},
};
export function startPageImageExport(request:PageImageRequest,onProgress:(event:PageImageProgress)=>void){
 let jobId:string|null=null,cancelled=false,finished=false;
 const result=(async()=>{
  try{
   jobId=await imageExportApi.register();
   if(cancelled){await imageExportApi.cancel(jobId);return null;}
   return await imageExportApi.start(jobId,request,event=>{if(!finished&&!cancelled&&event.jobId===jobId)onProgress(event);});
  }finally{finished=true;if(jobId)await imageExportApi.cancel(jobId).catch(()=>{});}
 })();
 return {result,cancel:async()=>{if(finished)return false;cancelled=true;return jobId?imageExportApi.cancel(jobId):true;}};
}
