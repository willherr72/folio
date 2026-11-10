import {useState} from 'react';
import {act,cleanup,renderHook,waitFor} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
import {nativeAdapter} from '../src/editor/adapter';
import {createSession,removeSession,type Workspace} from '../src/editor/workspace';
import {useWorkspaceRecovery} from '../src/editor/useWorkspaceRecovery';
import {useWorkspaceSources} from '../src/editor/use-workspace-sources';
import {recoveryApi} from '../src/editor/recovery';
vi.mock('../src/editor/image-resources',()=>({drainImageAcquisitions:async()=>{}}));
vi.mock('../src/components/PageView',()=>({clearRenderCache:vi.fn()}));
afterEach(()=>{cleanup();vi.restoreAllMocks();});

it.each(['checkpoint-first','cleanup-first'])('saves later edits after checkpoint adoption overlaps source cleanup: %s',async order=>{
 let finishClose!:()=>void,finishSave!:()=>void;
 const close=vi.fn(async()=>{}).mockImplementationOnce(()=>new Promise<void>(resolve=>finishClose=resolve));
 const adapter={...nativeAdapter,closeDocument:close};
 const kept=createSession({id:'kept',name:'Kept.pdf',pages:[{width:300,height:400}]},adapter,100);
 kept.sourceIds.push('abandoned');
 const removed=createSession({id:'removed',name:'Closed.pdf',pages:[{width:300,height:400}]},adapter,100);
 const initial:Workspace={tabs:[kept,removed],activeId:removed.id},positions={current:new Map()};
 vi.spyOn(recoveryApi,'load').mockResolvedValue(null);
 const save=vi.spyOn(recoveryApi,'save').mockResolvedValue(undefined).mockImplementationOnce(()=>new Promise<void>(resolve=>finishSave=resolve));
 const {result}=renderHook(()=>{
  const [workspace,setWorkspace]=useState(initial);
  const recovery=useWorkspaceRecovery(true,workspace,setWorkspace,positions);
  useWorkspaceSources(workspace,setWorkspace,recovery.sourceOwnership);
  return {workspace,setWorkspace,recovery};
 });
 await waitFor(()=>{expect(result.current.recovery.ready).toBe(true);expect(close).toHaveBeenCalledWith('abandoned');});
 const remaining=removeSession(result.current.workspace,removed.id);let checkpoint!:Promise<void>;
 act(()=>{checkpoint=result.current.recovery.checkpoint(remaining);});await waitFor(()=>expect(save).toHaveBeenCalledOnce());
 if(order==='checkpoint-first'){
  await act(async()=>{finishSave();await checkpoint;result.current.setWorkspace(remaining);finishClose();await Promise.resolve();});
 }else{
  await act(async()=>{finishClose();await Promise.resolve();finishSave();await checkpoint;result.current.setWorkspace(remaining);});
 }
 await waitFor(()=>expect(result.current.workspace.tabs).toHaveLength(1));
 act(()=>result.current.setWorkspace(current=>({...current,tabs:current.tabs.map(tab=>({...tab,history:{...tab.history,present:{...tab.history.present,pages:tab.history.present.pages.map(page=>({...page,rotation:90}))}}}))})));
 await waitFor(()=>{
  const saved=save.mock.calls.at(-1)?.[0];expect(saved?.tabs).toHaveLength(1);
  expect(saved?.tabs[0].id).toBe(kept.id);expect(saved?.tabs[0].document.pages[0].rotation).toBe(90);
 },{timeout:3000});
 expect(save.mock.calls.every(([snapshot])=>snapshot.tabs.length===1&&snapshot.tabs[0].id===kept.id)).toBe(true);
});
