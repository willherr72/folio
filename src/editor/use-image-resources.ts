import {useEffect,useLayoutEffect,useRef,useSyncExternalStore} from "react";
import type {Workspace} from "./workspace";
import {imageResourceState,imageRevision,retainImages,subscribeImages} from "./image-resources";
export function workspaceImageIds(workspace:Workspace):Set<string> {
  const ids=new Set<string>();
  for(const tab of workspace.tabs)if(tab.adapter.kind==="native")
    for(const document of [...tab.history.past,tab.history.present,...tab.history.future])
      for(const page of document.pages)for(const overlay of page.overlays)
        if(overlay.type==="image")ids.add(overlay.assetId);
  return ids;
}
export function useWorkspaceImages(workspace:Workspace) {
  useSyncExternalStore(subscribeImages,imageRevision);
  const alive=useRef(false);
  useLayoutEffect(()=>{void retainImages(workspaceImageIds(workspace)).catch(()=>{});},[workspace]);
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;queueMicrotask(()=>{if(!alive.current)void retainImages(new Set());});};},[]);
}
export function useImageResource(id?:string) {
  useSyncExternalStore(subscribeImages,imageRevision);return id?imageResourceState(id):undefined;
}
