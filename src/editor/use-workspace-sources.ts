import {useEffect,useRef,type Dispatch,type SetStateAction} from 'react';
import type {FolioAdapter} from './adapter';
import type {Workspace} from './workspace';
import {drainImageAcquisitions} from './image-resources';
import {clearRenderCache} from '../components/PageView';

type Source={adapter:FolioAdapter;sourceId:string};
type RecoveryOwners={revision:number;sourceIds():Set<string>};

/** Retain every available history state, across tabs sharing the same adapter. */
export function unusedWorkspaceSources(workspace:Workspace,recoverySources:ReadonlySet<string>):Source[] {
 const owned=new Map<FolioAdapter,Set<string>>(),used=new Map<FolioAdapter,Set<string>>();
 for(const tab of workspace.tabs){
  const all=owned.get(tab.adapter)??new Set<string>(),live=used.get(tab.adapter)??new Set<string>();
  for(const id of tab.sourceIds)all.add(id);
  for(const doc of [...tab.history.past,tab.history.present,...tab.history.future])for(const page of doc.pages)live.add(page.sourceId);
  if(tab.adapter.kind==='native')for(const id of recoverySources)live.add(id);
  owned.set(tab.adapter,all);used.set(tab.adapter,live);
 }
 return [...owned].flatMap(([adapter,ids])=>[...ids].filter(id=>!used.get(adapter)?.has(id)).map(sourceId=>({adapter,sourceId})));
}

export function forgetReleasedSource(workspace:Workspace,adapter:FolioAdapter,sourceId:string,recoverySources:ReadonlySet<string>):Workspace {
 if(!unusedWorkspaceSources(workspace,recoverySources).some(source=>source.adapter===adapter&&source.sourceId===sourceId))return workspace;
 return {...workspace,tabs:workspace.tabs.map(tab=>tab.adapter===adapter&&tab.sourceIds.includes(sourceId)?{...tab,sourceIds:tab.sourceIds.filter(id=>id!==sourceId)}:tab)};
}

/** Release unreachable immutable versions, never an available Undo/Redo state. */
export function useWorkspaceSources(workspace:Workspace,setWorkspace:Dispatch<SetStateAction<Workspace>>,recovery?:RecoveryOwners):void {
 const latest=useRef({workspace,recovery});latest.current={workspace,recovery};
 const alive=useRef(false),pending=useRef(new Map<FolioAdapter,Set<string>>());
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
 useEffect(()=>{
  for(const {adapter,sourceId} of unusedWorkspaceSources(workspace,recovery?.sourceIds()??new Set())){
   const active=pending.current.get(adapter)??new Set<string>();pending.current.set(adapter,active);
   if(active.has(sourceId))continue;active.add(sourceId);
   void (async()=>{
    await drainImageAcquisitions();
    const now=latest.current;
    if(!alive.current||!unusedWorkspaceSources(now.workspace,now.recovery?.sourceIds()??new Set()).some(source=>source.adapter===adapter&&source.sourceId===sourceId))return;
    clearRenderCache([sourceId]);await adapter.closeDocument(sourceId);
    if(alive.current)setWorkspace(value=>forgetReleasedSource(value,adapter,sourceId,latest.current.recovery?.sourceIds()??new Set()));
   })().catch(()=>{/* Keep ownership on failure; retry on later workspace activity. */}).finally(()=>{active.delete(sourceId);if(!active.size)pending.current.delete(adapter);});
  }
 },[workspace,setWorkspace,recovery?.revision,recovery?.sourceIds]);
}
