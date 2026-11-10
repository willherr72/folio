import {useCallback,useLayoutEffect,useRef,useState,type Dispatch,type SetStateAction} from "react";
import {commit,uniqueId} from "./model";
import {fitImage,imageBounds,moveImage} from "./image-geometry";
import {adoptImageLease,importImageLease,retainImages,type ImageLease} from "./image-resources";
import {workspaceImageIds} from "./use-image-resources";
import {pageAllows} from "./protection";
import type {ImageOverlay,InkPoint,Rotation} from "./types";
import type {Workspace} from "./workspace";

interface Target {tabId:string;pageId:string;sourceId:string;pageIndex:number;width:number;height:number;rotation:Rotation}
export interface PendingImage {target:Target;image:ImageOverlay}
interface Options {workspace:Workspace;setWorkspace:Dispatch<SetStateAction<Workspace>>;disabled:boolean;onPlaced?:()=>void}
function targetOf(workspace:Workspace):Target|null {
  const tab=workspace.tabs.find(tab=>tab.id===workspace.activeId);
  if(!tab||tab.adapter.kind!=="native")return null;
  const document=tab.history.present,page=document.pages.find(page=>page.id===document.selectedPageId)??document.pages[0];
  if(!page||!pageAllows(page,"annotate"))return null;
  return {tabId:tab.id,pageId:page.id,sourceId:page.sourceId,pageIndex:page.pageIndex,width:page.width,height:page.height,rotation:page.rotation};
}
function matches(a:Target|null,b:Target|null):boolean {
  return !!a&&!!b&&a.tabId===b.tabId&&a.pageId===b.pageId&&a.sourceId===b.sourceId&&a.pageIndex===b.pageIndex&&a.width===b.width&&a.height===b.height&&a.rotation===b.rotation;
}
function positioned(pending:PendingImage,point?:InkPoint):PendingImage {
  if(!point)return pending;
  const {image,target}=pending,box=imageBounds(image);
  return {...pending,image:moveImage(image,image.x+point.x-box.x-box.width/2,image.y+point.y-box.y-box.height/2,target.width,target.height)};
}

/** Pending imports own their native lease until cancellation or a single history
 * commit. The workspace then owns that lease across present, undo and redo. */
export function useImagePlacement(options:Options) {
  const latest=useRef(options);latest.current=options;
  const alive=useRef(true),generation=useRef(0),operation=useRef<Target|null>(null);
  const owned=useRef<ImageLease|null>(null),draft=useRef<PendingImage|null>(null),picking=useRef(false);
  const reconcile=useRef<{tabId:string;imageId:string}|null>(null);
  const [pending,setPending]=useState<PendingImage|null>(null),[importing,setImporting]=useState(false),[error,setError]=useState<string|null>(null);
  const releasePending=useCallback(()=>{const value=owned.current;owned.current=null;if(value)void value.release().catch(()=>{});draft.current=null;},[]);
  const cancel=useCallback(()=>{generation.current++;operation.current=null;picking.current=false;releasePending();setPending(null);setImporting(false);setError(null);},[releasePending]);
  useLayoutEffect(()=>{
    alive.current=true;
    return()=>{alive.current=false;generation.current++;operation.current=null;picking.current=false;releasePending();};
  },[]);
  useLayoutEffect(()=>{
    if(operation.current&&(options.disabled||!matches(operation.current,targetOf(options.workspace))))cancel();
    const placed=reconcile.current;
    if(placed){
      reconcile.current=null;
      void retainImages(workspaceImageIds(options.workspace)).catch(()=>{});
      const tab=options.workspace.tabs.find(tab=>tab.id===placed.tabId);
      const exists=tab&&[...tab.history.past,tab.history.present,...tab.history.future].some(document=>document.pages.some(page=>page.overlays.some(image=>image.id===placed.imageId)));
      if(exists)options.onPlaced?.();else setError("The page changed before image placement. Try again.");
    }
  },[options.workspace,options.disabled]);
  async function begin():Promise<void> {
    if(latest.current.disabled||picking.current)return;
    const target=targetOf(latest.current.workspace);if(!target)return;
    cancel();const ticket=generation.current;operation.current=target;picking.current=true;setImporting(true);
    try {
      const lease=await importImageLease();
      if(!lease)return;
      if(!alive.current||ticket!==generation.current||latest.current.disabled||!matches(target,targetOf(latest.current.workspace))){await lease.release();return;}
      const image=fitImage(uniqueId("image"),lease.registration.image,target.width,target.height);
      if(!image){await lease.release();throw new Error("This image cannot fit on the selected page.");}
      owned.current=lease;draft.current={target,image};setPending(draft.current);
    }catch(reason){if(alive.current&&ticket===generation.current)setError(reason instanceof Error?reason.message:String(reason));}
    finally{if(alive.current&&ticket===generation.current){picking.current=false;setImporting(false);if(!draft.current)operation.current=null;}}
  }
  function move(point:InkPoint){if(!draft.current)return;draft.current=positioned(draft.current,point);setPending(draft.current);}
  function place(point?:InkPoint){
    if(!draft.current||!owned.current)return;
    if(latest.current.disabled||!matches(draft.current.target,targetOf(latest.current.workspace))){cancel();return;}
    const {target,image}=positioned(draft.current,point),lease=owned.current;
    // Clear caller ownership before publishing; repeated clicks cannot place twice.
    owned.current=null;draft.current=null;operation.current=null;generation.current++;
    adoptImageLease(lease);
    reconcile.current={tabId:target.tabId,imageId:image.id};
    latest.current.setWorkspace(workspace=>{
      if(!matches(target,targetOf(workspace)))return workspace;
      return {...workspace,tabs:workspace.tabs.map(tab=>tab.id!==target.tabId?tab:{...tab,history:commit(tab.history,document=>({...document,
        selectedPageId:target.pageId,selectedOverlayId:image.id,
        pages:document.pages.map(page=>page.id!==target.pageId?page:{...page,overlays:[...page.overlays,image]}),
      }))})};
    });
    setPending(null);
  }
  return {pending,importing,error,begin,move,place,cancel};
}
