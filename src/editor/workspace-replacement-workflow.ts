import type {FolioAdapter} from './adapter';
import {normalizeSearchQuery} from './search-matching';
import {workspaceReplacementPages,type WorkspaceReplacementRequest,type WorkspaceReplacementResult} from './workspace-replacement';
import type {PagePlan} from './types';
import type {WorkspaceReplacementAdapter,WorkspaceReplacementIntent,WorkspaceReplacementPlan,NativeWorkspaceReplacementPreview} from './workspace-replacement-transport';

const capabilities=['beginWorkspaceReplacement','planWorkspaceReplacement','previewWorkspaceReplacement','applyWorkspaceReplacement','cancelWorkspaceReplacement','releaseWorkspaceReplacement'] as const;
export function supportsWorkspaceReplacement(adapter:FolioAdapter):adapter is FolioAdapter&WorkspaceReplacementAdapter {
 return adapter.kind==='native'&&typeof adapter.renderPagePlan==='function'&&capabilities.every(key=>typeof (adapter as unknown as Record<string,unknown>)[key]==='function');
}
function requireValue(value:unknown,message='The replacement service returned an invalid result.'):asserts value{if(!value)throw new Error(message);}
const id=(value:unknown):value is string=>typeof value==='string'&&value.length>0&&value.length<=256;
function frozen<T>(value:T):T{if(value&&typeof value==='object'){Object.freeze(value);for(const child of Object.values(value))frozen(child);}return value;}
function copy<T>(value:T):T{const cloned=structuredClone(value);requireValue(new TextEncoder().encode(JSON.stringify(cloned)).length<=4*1024*1024);return cloned;}
export interface CheckedWorkspaceReplacementPreview extends NativeWorkspaceReplacementPreview {before:PagePlan[];after:PagePlan[];expiresAt:number}
interface Guards{isCurrent():boolean;borrowedIds():Iterable<string>}
export class WorkspaceReplacementWorkflow {
 private readonly adapter:FolioAdapter&WorkspaceReplacementAdapter;
 private session:string|null=null;
 private closed=false;private released=false;private busy=false;private generation=0;
 private planValue:WorkspaceReplacementPlan|null=null;private previewValue:CheckedWorkspaceReplacementPreview|null=null;private deadline=0;
 private applying:CheckedWorkspaceReplacementPreview|null=null;private applied=false;
 private seen=new Set<string>();private cleaned=new Set<string>();
 private cancellation:Promise<void>=Promise.resolve();
 constructor(readonly request:WorkspaceReplacementRequest,private readonly guards:Guards){
  requireValue(supportsWorkspaceReplacement(request.adapter),'Replace text requires the desktop replacement service.');this.adapter=request.adapter;
 }
 get disposed():boolean{return this.closed;}
 private current(generation?:number):void{
  if(!this.guards.isCurrent()){this.dispose();throw new Error('The document changed. Reopen Replace and try again.');}
  requireValue(!this.closed&&(generation===undefined||generation===this.generation),'The replacement request changed or was cancelled.');
 }
 private async run<T>(work:()=>Promise<T>):Promise<T>{this.current();requireValue(!this.busy,'Replace is already working.');this.busy=true;try{return await work();}finally{this.busy=false;}}
 private release():void{
  if(!this.session||this.released)return;this.released=true;const session=this.session;
  void this.cancellation.then(()=>this.adapter.releaseWorkspaceReplacement(session)).catch(()=>{});
 }
 private closeKnown(preview:CheckedWorkspaceReplacementPreview|null):void{
  if(!preview)return;const borrowed=new Set(this.guards.borrowedIds());
  for(const source of preview.result.sources){const candidate=source.document.id;if(!borrowed.has(candidate)&&!this.cleaned.has(candidate)){this.cleaned.add(candidate);void Promise.resolve().then(()=>{if(!new Set(this.guards.borrowedIds()).has(candidate))return this.adapter.closeDocument(candidate);}).catch(()=>{});}}
 }
 invalidate():void{
  this.generation++;this.planValue=null;this.previewValue=null;this.deadline=0;
  if(this.applying&&!this.applied)this.closeKnown(this.applying);
  if(this.session&&!this.closed){const session=this.session;this.cancellation=this.cancellation.then(()=>this.adapter.cancelWorkspaceReplacement(session)).catch(()=>{});}
 }
 dispose():void{if(this.closed)return;this.invalidate();this.closed=true;if(this.applying&&!this.applied)this.closeKnown(this.applying);this.release();}
 async begin():Promise<void>{
  return this.run(async()=>{
   requireValue(!this.session);
   try{const session=await this.adapter.beginWorkspaceReplacement(structuredClone(this.request.document.pages));requireValue(id(session));this.session=session;if(this.closed)this.release();this.current();}
   catch(error){this.dispose();throw error;}
  });
 }
 async check(intent:WorkspaceReplacementIntent):Promise<WorkspaceReplacementPlan>{
  return this.run(async()=>{
   requireValue(this.session);requireValue(normalizeSearchQuery(intent.query,intent.options).length>0,'Enter text to find.');requireValue(new TextEncoder().encode(intent.replacement).length<=4096&&!intent.replacement.includes('\0'),'Use replacement text of at most 4 KiB without null characters.');
   const requested=copy(intent);this.invalidate();const generation=this.generation;await this.cancellation;this.current(generation);
   const plan=copy(await this.adapter.planWorkspaceReplacement(this.session,requested));this.current(generation);
   requireValue(id(plan.planId)&&plan.query===requested.query&&plan.replacement===requested.replacement&&plan.options?.matchCase===requested.options.matchCase&&plan.options?.wholeWords===requested.options.wholeWords);
   requireValue(Array.isArray(plan.pages)&&plan.pages.length<=10_000&&typeof plan.canApply==='boolean'&&(plan.batchBlocker===null||typeof plan.batchBlocker==='string'));
   const known=new Set(this.request.document.pages.map(p=>p.id)),pages=new Set<string>();let selected=0,blocked=0,total=0;
   const chosen=requested.selected==null?null:new Set(requested.selected.map(s=>JSON.stringify([s.pageId,s.matchIndex])));
   for(const page of plan.pages){
    requireValue(known.has(page.pageId)&&!pages.has(page.pageId)&&Array.isArray(page.matches)&&page.matches.length<=1000);pages.add(page.pageId);
    for(const [index,match] of page.matches.entries()){
     requireValue(match.matchIndex===index&&typeof match.selected==='boolean'&&(match.overlayId===null||typeof match.overlayId==='string')&&(match.blocker===null||typeof match.blocker==='string')&&Array.isArray(match.rects)&&match.rects.length<=10_000);
     requireValue(match.selected===(chosen===null||chosen.has(JSON.stringify([page.pageId,index]))));
     for(const rect of match.rects)requireValue(rect&&[rect.x,rect.y,rect.width,rect.height].every(Number.isFinite)&&rect.width>=0&&rect.height>=0);
     if(match.selected){selected++;if(match.blocker!==null)blocked++;}total++;requireValue(total<=10_000);
    }
   }
   requireValue(plan.selectedCount===selected&&plan.blockedCount===blocked&&Array.isArray(plan.affectedPageIds)&&plan.affectedPageIds.length<=32&&new Set(plan.affectedPageIds).size===plan.affectedPageIds.length&&plan.affectedPageIds.every(page=>known.has(page)&&plan.pages.some(p=>p.pageId===page&&p.matches.some(m=>m.selected&&m.blocker===null))));
   requireValue(!plan.canApply||(selected>0&&blocked===0&&plan.batchBlocker===null&&plan.affectedPageIds.length>0));
   this.planValue=frozen(plan);return this.planValue;
  });
 }
 async preview(plan:WorkspaceReplacementPlan):Promise<CheckedWorkspaceReplacementPreview>{
  return this.run(async()=>{
   requireValue(this.session&&plan===this.planValue&&plan.canApply,'Check these replacements before previewing.');const generation=this.generation;
   this.previewValue=null;this.deadline=0;
   const value=copy(await this.adapter.previewWorkspaceReplacement(this.session,plan.planId));this.current(generation);
   try{
    requireValue(id(value.previewId)&&Number.isInteger(value.expiresInSeconds)&&value.expiresInSeconds>0&&value.expiresInSeconds<=900);
    requireValue(JSON.stringify(value.affectedPageIds)===JSON.stringify(plan.affectedPageIds));
    const after=workspaceReplacementPages(this.request,value.result,this.guards.borrowedIds());
    for(const source of value.result.sources){requireValue(!this.seen.has(source.document.id),'Replace must return a new source version.');requireValue(plan.affectedPageIds.some(id=>this.request.document.pages.some(p=>p.id===id&&p.sourceId===source.sourceId)));}
    requireValue(value.result.overlays.every(p=>plan.affectedPageIds.includes(p.pageId)));
    for(const pageId of plan.affectedPageIds)requireValue(value.result.overlays.some(p=>p.pageId===pageId)||value.result.sources.some(s=>this.request.document.pages.some(p=>p.id===pageId&&p.sourceId===s.sourceId)));
    for(const source of value.result.sources)this.seen.add(source.document.id);
    this.deadline=Date.now()+value.expiresInSeconds*1000;this.previewValue=frozen({...value,before:this.request.document.pages,after:structuredClone(after),expiresAt:this.deadline});return this.previewValue;
   }catch(error){this.dispose();throw error;}
  });
 }
 async apply(preview:CheckedWorkspaceReplacementPreview,adopt:(result:WorkspaceReplacementResult)=>boolean):Promise<void>{
  return this.run(async()=>{
   requireValue(this.session&&preview===this.previewValue&&Date.now()<this.deadline,'Preview these replacements again before applying.');const generation=this.generation;this.applying=preview;
   try{
    const result=copy(await this.adapter.applyWorkspaceReplacement(this.session,preview.previewId));this.current(generation);
    requireValue(JSON.stringify(result)===JSON.stringify(preview.result),'Apply returned a different replacement preview.');workspaceReplacementPages(this.request,result,this.guards.borrowedIds());
    requireValue(adopt(frozen(result)),'The document changed. Reopen Replace and try again.');this.applied=true;this.dispose();
   }catch(error){this.closeKnown(preview);this.dispose();throw error;}finally{this.applying=null;}
  });
 }
}
