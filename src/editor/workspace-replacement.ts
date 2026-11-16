import type {DocumentSession} from './workspace';
import type {EditorDocument, PageContentReplacement, PagePlan} from './types';
import {commit} from './model';
import {pageAllows} from './protection';

export interface WorkspaceReplacementRequest {
 readonly tabId:string;
 readonly adapter:DocumentSession['adapter'];
 readonly history:DocumentSession['history'];
 readonly document:EditorDocument;
 readonly borrowedSourceIds:readonly string[];
}
export interface WorkspaceOverlayReplacement {
 pageId:string; overlayId:string; expectedText:string; text:string|null;
}
export interface WorkspaceReplacementResult {
 sources:Array<PageContentReplacement & {sourceId:string}>;
 overlays:WorkspaceOverlayReplacement[];
}
function requireValue(valid:unknown,message='The workspace replacement result is invalid.'):asserts valid {
 if(!valid)throw new Error(message);
}
const validId=(id:unknown):id is string=>typeof id==='string'&&id.length>0&&id.length<=256;
const validOverlayId=(id:unknown):id is string=>typeof id==='string'&&id.length>0&&id.length<=1024;
function freeze<T>(value:T):T {
 if(value&&typeof value==='object'){Object.freeze(value);for(const child of Object.values(value))freeze(child);}
 return value;
}
function unchanged(request:WorkspaceReplacementRequest):void {
 requireValue(JSON.stringify(request.history.present)===JSON.stringify(request.document),'The document changed. Reopen Replace and try again.');
}
export function captureWorkspaceReplacement(session:DocumentSession,borrowedSourceIds:Iterable<string>):WorkspaceReplacementRequest {
 const document=session.history.present;
 requireValue(document.pages.length>0&&document.pages.length<=10_000,'Replace supports workspaces with 1 to 10,000 pages.');
 const pages=new Set<string>();
 for(const page of document.pages){
  requireValue(validId(page.id)&&!pages.has(page.id)&&validId(page.sourceId)&&Number.isSafeInteger(page.pageIndex)&&page.pageIndex>=0);
  requireValue(Number.isFinite(page.width)&&Number.isFinite(page.height)&&page.width>0&&page.height>0);
  const overlays=new Set<string>();
  requireValue(Array.isArray(page.overlays),'Replace requires valid overlay identities.');
  for(const overlay of page.overlays){
   requireValue(overlay&&validOverlayId(overlay.id)&&!overlays.has(overlay.id),'Replace requires unique overlay identities on each page.');overlays.add(overlay.id);
  }
  pages.add(page.id);
 }
 const borrowed=[...new Set([...borrowedSourceIds,...session.sourceIds,...document.pages.map(page=>page.sourceId)])];
 return Object.freeze({tabId:session.id,adapter:session.adapter,history:session.history,document:freeze(structuredClone(document)),borrowedSourceIds:Object.freeze(borrowed)});
}
/** Additional model boundary; the native bound preview owns proof of PDF content. */
export function workspaceReplacementPages(request:WorkspaceReplacementRequest,result:WorkspaceReplacementResult,borrowedSourceIds:Iterable<string>):PagePlan[] {
 unchanged(request);
 requireValue(result&&Array.isArray(result.sources)&&result.sources.length<=32&&Array.isArray(result.overlays)&&result.overlays.length<=64);
 requireValue(result.sources.length+result.overlays.length>0,'Replace returned no changes.');
 const borrowed=new Set([...request.borrowedSourceIds,...borrowedSourceIds]);
 const replacements=new Map<string,Map<string,{sourceId:string;pageIndex:number}>>(),candidates=new Set<string>();
 const original=request.history.present;
 for(const source of result.sources){
  requireValue(source&&validId(source.sourceId)&&!replacements.has(source.sourceId));
  const pages=original.pages.filter(page=>page.sourceId===source.sourceId),info=source.document;
  requireValue(pages.length>0&&info&&validId(info.id)&&!borrowed.has(info.id)&&!candidates.has(info.id),'Replace must return a unique new source version.');
  requireValue(Array.isArray(info.pages)&&info.pages.length>0&&info.pages.length<=100_064);
  requireValue(Array.isArray(source.pageIndices)&&source.pageIndices.length===pages.length,'Replace omitted a page occurrence.');
  const maxOriginal=Math.max(...pages.map(page=>page.pageIndex)),origins=new Map<number,number>(),indices=new Map<number,Set<number>>();
  requireValue(info.pages.length>maxOriginal,'Replace omitted an original page.');
  const mapping=new Map<string,{sourceId:string;pageIndex:number}>();
  for(let i=0;i<pages.length;i++){
   const page=pages[i],index=source.pageIndices[i];
   requireValue(pageAllows(page,'editContent')&&pageAllows(page,'copyContent'),'This PDF does not grant permission to replace text.');
   const before=page.protection,after=info.protection;
   requireValue(!!before===!!after&&(!before||!!after&&before.documentId===after.documentId&&before.owner===after.owner&&before.revision===after.revision&&Object.entries(before.permissions).every(([key,value])=>after.permissions?.[key as keyof typeof after.permissions]===value)),"Replace changed the document’s protection.");
   requireValue(Number.isSafeInteger(index)&&index>=0&&index<info.pages.length&&(index===page.pageIndex||index>maxOriginal),'Replace changed the wrong page occurrence.');
   requireValue(!origins.has(index)||origins.get(index)===page.pageIndex,'Replace conflated distinct original pages.');
   origins.set(index,page.pageIndex);
   const group=indices.get(page.pageIndex)??new Set<number>();group.add(index);indices.set(page.pageIndex,group);
   const size=info.pages[index];
   requireValue(size&&Number.isFinite(size.width)&&Number.isFinite(size.height)&&size.width>0&&size.height>0&&Math.abs(size.width-page.width)<=.01&&Math.abs(size.height-page.height)<=.01,'Replace changed page dimensions or omitted a page.');
   mapping.set(page.id,{sourceId:info.id,pageIndex:index});
  }
  for(const page of pages)requireValue(indices.get(page.pageIndex)!.size===1||pageAllows(page,'assemble'),'Replacing independent duplicate pages also requires permission to arrange pages.');
  replacements.set(source.sourceId,mapping);candidates.add(info.id);
 }
 const patches=new Map<string,Map<string,string|null>>();let bytes=0;
 for(const patch of result.overlays){
  requireValue(patch&&validId(patch.pageId)&&validOverlayId(patch.overlayId)&&typeof patch.expectedText==='string');
  requireValue(patch.text===null||typeof patch.text==='string'&&patch.text.length>0&&!/[\u0000\ud800-\udfff]/u.test(patch.text));
  const page=original.pages.find(page=>page.id===patch.pageId),overlay=page?.overlays.find(overlay=>overlay.id===patch.overlayId);
  requireValue(page&&overlay?.type==='text'&&overlay.text===patch.expectedText&&patch.text!==patch.expectedText,'The replacement text overlay changed or is invalid.');
  requireValue(pageAllows(page,'copyContent')&&pageAllows(page,'annotate'),'This PDF does not grant permission to replace added text.');
  bytes+=new TextEncoder().encode(patch.expectedText).length+new TextEncoder().encode(patch.text??'').length;
  requireValue(bytes<=512*1024,'Replace exceeds the text batch limit.');
  const targets=patches.get(patch.pageId)??new Map<string,string|null>();
  requireValue(!targets.has(patch.overlayId),'Replace repeated an overlay target.');targets.set(patch.overlayId,patch.text);patches.set(patch.pageId,targets);
 }
 return original.pages.map(page=>{
  const mapping=replacements.get(page.sourceId)?.get(page.id),targets=patches.get(page.id);
  if(!mapping&&!targets)return page;
  const overlays=targets?page.overlays.flatMap(overlay=>{
   if(!targets.has(overlay.id))return [overlay];
   const text=targets.get(overlay.id)!;return text===null?[]:[{...overlay,text}];
  }):page.overlays;
  return {...page,...mapping,...(mapping?{contentSourceId:undefined}:{}),overlays};
 });
}
export function commitWorkspaceReplacement(session:DocumentSession,request:WorkspaceReplacementRequest,result:WorkspaceReplacementResult,borrowedSourceIds:Iterable<string>):DocumentSession {
 requireValue(session.id===request.tabId&&session.adapter===request.adapter&&session.history===request.history,'The document changed. Reopen Replace and try again.');
 const pages=workspaceReplacementPages(request,result,[...borrowedSourceIds,...session.sourceIds]);
 const sourceIds=result.sources.length?[...session.sourceIds,...result.sources.map(source=>source.document.id)]:session.sourceIds;
 return {...session,sourceIds,history:commit(session.history,document=>({...document,pages,selectedOverlayId:null}))};
}
