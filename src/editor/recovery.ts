import { invoke } from "@tauri-apps/api/core";
import type { Workspace, ScrollPosition } from "./workspace";
import {emptyNavigationHistory} from './navigation-history';
import {createHistory,planDigest} from "./model";
import type { EditorDocument } from "./types";
import type { FolioAdapter } from "./adapter";
import {validPageCrop} from "./page-crop";
import {validateMetadata} from './document-properties';
export interface RecoveryTab { id:string; document:EditorDocument; savedDigest:string; dirty:boolean; zoom:number; scrollPosition:ScrollPosition }
export interface RecoverySnapshot { version:1; activeId:string|null; tabs:RecoveryTab[] }
/** Source ownership/history bookkeeping is not part of the durable document. */
export function sameRecoveryWorkspace(expected:Workspace,actual:Workspace):boolean {
 return expected.activeId===actual.activeId&&expected.tabs.length===actual.tabs.length&&expected.tabs.every((tab,index)=>{
  const other=actual.tabs[index];
  return tab.id===other.id&&tab.adapter===other.adapter&&tab.history.present===other.history.present&&tab.savedDigest===other.savedDigest&&tab.zoom===other.zoom;
 });
}
export const recoveryApi={
 load:()=>invoke<RecoverySnapshot|null>("load_recovery"),
 save:(workspace:RecoverySnapshot)=>invoke<void>("save_recovery",{workspace}),
 clear:()=>invoke<void>("clear_recovery"),
};
export function snapshotWorkspace(workspace:Workspace,positions:Map<string,ScrollPosition>):RecoverySnapshot {
 const tabs=workspace.tabs.filter(tab=>tab.adapter.kind==="native").map(tab=>({
  id:tab.id,document:tab.history.present,savedDigest:tab.savedDigest,dirty:planDigest(tab.history.present)!==tab.savedDigest,
  zoom:tab.zoom,scrollPosition:positions.get(tab.id)??{top:0,left:0}
 }));
 return {version:1,activeId:tabs.some(tab=>tab.id===workspace.activeId)?workspace.activeId:tabs[0]?.id??null,tabs};
}
export function restoreWorkspace(snapshot:RecoverySnapshot,adapter:FolioAdapter):{workspace:Workspace;positions:Map<string,ScrollPosition>} {
 if(snapshot.version!==1||!Array.isArray(snapshot.tabs))throw new Error("This recovery format is not supported.");
 for(const tab of snapshot.tabs)if(tab.document.metadata!==undefined)validateMetadata(tab.document.metadata);
 for(const tab of snapshot.tabs)for(const page of tab.document.pages)if(page.crop&&!validPageCrop(page,page.crop))throw new Error("The recovered page crop is invalid.");
 const tabs=snapshot.tabs.map(tab=>({
  id:tab.id,adapter,sourceIds:[...new Set(tab.document.pages.map(page=>page.sourceId))],
  history:createHistory(tab.document),savedDigest:tab.dirty?"recovered-unsaved":planDigest(tab.document),zoom:tab.zoom,
  navigationHistory:emptyNavigationHistory(),navigationRequest:tab.document.selectedPageId?{pageId:tab.document.selectedPageId,revision:1}:null
 }));
 return {workspace:{tabs,activeId:tabs.some(tab=>tab.id===snapshot.activeId)?snapshot.activeId:tabs[0]?.id??null},
  positions:new Map(snapshot.tabs.map(tab=>[tab.id,tab.scrollPosition]))};
}
/** Coalesces activity and serializes filesystem mutations, including final discard. */
export class RecoveryQueue {
 private tail:Promise<void>=Promise.resolve();
 private pending:RecoverySnapshot|undefined;
 private idle:ReturnType<typeof setTimeout>|undefined;
 private deadline:ReturnType<typeof setTimeout>|undefined;
 private stopped=false;
 private writers=new Map<object,RecoverySnapshot>();
 constructor(private api:{save(snapshot:RecoverySnapshot):Promise<void>;clear():Promise<void>},private onError:(error:unknown)=>void,private onSourcesChanged:()=>void=()=>{}) {}
 /** Pending and queued writes own their sources even after the UI drops a branch. */
 sourceIds():Set<string> {
  const snapshots=[...this.writers.values(),...(this.pending?[this.pending]:[])];
  return new Set(snapshots.flatMap(snapshot=>snapshot.tabs.flatMap(tab=>tab.document.pages.map(page=>page.sourceId))));
 }
 settled():Promise<void>{return this.tail;}
 private cancelTimers(){clearTimeout(this.idle);clearTimeout(this.deadline);this.idle=undefined;this.deadline=undefined;}
 schedule(snapshot:RecoverySnapshot):void {
  if(this.stopped)return;
  this.pending=snapshot;clearTimeout(this.idle);
  this.onSourcesChanged();
  this.idle=setTimeout(()=>{void this.flush().catch(()=>{});},1000);
  this.deadline??=setTimeout(()=>{void this.flush().catch(()=>{});},5000);
 }
 flush(snapshot?:RecoverySnapshot):Promise<void> {
  if(this.stopped)return this.tail;
  if(snapshot)this.pending=snapshot;
  this.cancelTimers();
  const next=this.pending;this.pending=undefined;
  if(!next)return this.tail;
  const owner={};this.writers.set(owner,next);this.onSourcesChanged();
  const work=this.tail.then(()=>this.api.save(next)).finally(()=>{this.writers.delete(owner);this.onSourcesChanged();});
  this.tail=work.catch(error=>{this.onError(error);});
  return work;
 }
 async close():Promise<void> {
  this.stopped=true;this.cancelTimers();this.pending=undefined;
  this.onSourcesChanged();
  await this.tail;
  try{await this.api.clear();}catch(error){this.stopped=false;throw error;}
 }
 dispose():void {this.stopped=true;this.cancelTimers();this.pending=undefined;this.onSourcesChanged();}
}
