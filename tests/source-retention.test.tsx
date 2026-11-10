import {StrictMode,type ReactNode} from 'react';
import {act,cleanup,renderHook,waitFor} from '@testing-library/react';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {nativeAdapter} from '../src/editor/adapter';
import {createSession,type Workspace} from '../src/editor/workspace';
import {unusedWorkspaceSources,forgetReleasedSource,useWorkspaceSources} from '../src/editor/use-workspace-sources';
import {drainImageAcquisitions} from '../src/editor/image-resources';
import {useWorkspaceRecovery} from '../src/editor/useWorkspaceRecovery';
import {recoveryApi} from '../src/editor/recovery';
import {commit,undo,redo} from '../src/editor/model';
vi.mock('../src/editor/image-resources',()=>({drainImageAcquisitions:vi.fn(async()=>{})}));
vi.mock('../src/components/PageView',()=>({clearRenderCache:vi.fn()}));
afterEach(()=>{cleanup();vi.restoreAllMocks();});beforeEach(()=>{vi.mocked(drainImageAcquisitions).mockReset().mockResolvedValue(undefined);});
function branch(){
 const adapter={...nativeAdapter,closeDocument:vi.fn(async(_sourceId:string)=>{})};
 const tab=createSession({id:'original',name:'One.pdf',pages:[{width:300,height:400}]},adapter,100);
 const original=tab.history.present;
 tab.sourceIds.push('abandoned','current');tab.history={past:[original],present:{...original,pages:original.pages.map(p=>({...p,sourceId:'current'}))},future:[]};
 return {adapter,tab,workspace:{tabs:[tab],activeId:tab.id} as Workspace};
}
it('releases only aged-out history sources and retains the entire usable undo/redo window',async()=>{
 const {tab,adapter}=branch();tab.history={past:[],present:tab.history.past[0],future:[]};tab.sourceIds=['original'];
 for(let i=1;i<=105;i++){
  const sourceId='version-'+i;tab.sourceIds.push(sourceId);
  tab.history=commit(tab.history,doc=>({...doc,pages:doc.pages.map(p=>({...p,sourceId}))}));
 }
 const held=new Set(['original']);const workspace={tabs:[tab],activeId:tab.id};
 expect(unusedWorkspaceSources(workspace,held).map(x=>x.sourceId)).toEqual(['version-1','version-2','version-3','version-4']);
 for(let i=0;i<100;i++)tab.history=undo(tab.history);
 expect(tab.history.present.pages[0].sourceId).toBe('version-5');
 const update=vi.fn();renderHook(()=>useWorkspaceSources(workspace,update,{revision:0,sourceIds:()=>held}));
 await waitFor(()=>expect(adapter.closeDocument).toHaveBeenCalledTimes(4));
 expect(adapter.closeDocument.mock.calls.map(x=>x[0]).sort()).toEqual(['version-1','version-2','version-3','version-4']);
 for(let i=0;i<100;i++)tab.history=redo(tab.history);
 expect(tab.history.present.pages[0].sourceId).toBe('version-105');
});
it('finds only the discarded branch and preserves current, past, future and other-tab owners',()=>{
 const {workspace,tab,adapter}=branch();expect(unusedWorkspaceSources(workspace,new Set())).toEqual([{adapter,sourceId:'abandoned'}]);
 tab.history.future=[{...tab.history.present,pages:tab.history.present.pages.map(p=>({...p,sourceId:'abandoned'}))}];
 expect(unusedWorkspaceSources(workspace,new Set())).toEqual([]);tab.history.future=[];
 workspace.tabs.push({...tab,id:'other',history:{past:[],future:[],present:{...tab.history.present,pages:tab.history.present.pages.map(p=>({...p,sourceId:'abandoned'}))}}});
 expect(unusedWorkspaceSources(workspace,new Set())).toEqual([]);
});
it('preserves recovery owners and removes only successfully released unused ownership',()=>{
 const {workspace,adapter}=branch();expect(unusedWorkspaceSources(workspace,new Set(['abandoned']))).toEqual([]);
 expect(forgetReleasedSource(workspace,adapter,'original',new Set())).toBe(workspace);
 expect(forgetReleasedSource(workspace,adapter,'abandoned',new Set(['abandoned']))).toBe(workspace);
 expect(forgetReleasedSource(workspace,adapter,'abandoned',new Set()).tabs[0].sourceIds).toEqual(['original','current']);
});
it('keeps source ownership separate between adapter instances',()=>{
 const a=branch(),b=branch();b.tab.history.present.pages[0].sourceId='abandoned';
 const unused=unusedWorkspaceSources({tabs:[a.tab,b.tab],activeId:a.tab.id},new Set());
 expect(unused).toContainEqual({adapter:a.adapter,sourceId:'abandoned'});
 expect(unused).not.toContainEqual({adapter:b.adapter,sourceId:'abandoned'});
});
it('releases discarded sources once under StrictMode and preserves usable history',async()=>{
 const {workspace,adapter}=branch(),update=vi.fn();
 renderHook(()=>useWorkspaceSources(workspace,update),{wrapper:({children}:{children:ReactNode})=><StrictMode>{children}</StrictMode>});
 await waitFor(()=>{expect(adapter.closeDocument).toHaveBeenCalledOnce();expect(adapter.closeDocument).toHaveBeenCalledWith('abandoned');});
 await waitFor(()=>expect(update).toHaveBeenCalledOnce());
 const next=update.mock.calls[0][0](workspace);expect(next.tabs[0].sourceIds).toEqual(['original','current']);expect(next.tabs[0].history).toBe(workspace.tabs[0].history);
});
it('waits for recovery owners to settle',async()=>{
 const {workspace,adapter}=branch(),update=vi.fn(),held=new Set(['abandoned']);
 const {rerender}=renderHook(({revision})=>useWorkspaceSources(workspace,update,{revision,sourceIds:()=>held}),{initialProps:{revision:0}});
 await act(async()=>{});expect(adapter.closeDocument).not.toHaveBeenCalled();held.clear();rerender({revision:1});
 await waitFor(()=>expect(adapter.closeDocument).toHaveBeenCalledWith('abandoned'));
});
it('rechecks ownership after pending image acquisition drains',async()=>{
 let finish!:()=>void;vi.mocked(drainImageAcquisitions).mockImplementation(()=>new Promise<void>(resolve=>finish=resolve));
 const {workspace,adapter,tab}=branch(),update=vi.fn();const {rerender}=renderHook(({current})=>useWorkspaceSources(current,update),{initialProps:{current:workspace}});
 const next={...workspace,tabs:[{...tab,history:{...tab.history,future:[{...tab.history.present,pages:tab.history.present.pages.map(p=>({...p,sourceId:'abandoned'}))}]}}]};
 rerender({current:next});await act(async()=>finish());expect(adapter.closeDocument).not.toHaveBeenCalled();expect(update).not.toHaveBeenCalled();
});
it('retains failed-close ownership and retries after subsequent workspace activity',async()=>{
 const {workspace,adapter}=branch(),update=vi.fn();adapter.closeDocument.mockRejectedValueOnce(new Error('busy'));
 const {rerender}=renderHook(({current})=>useWorkspaceSources(current,update),{initialProps:{current:workspace}});
 await waitFor(()=>expect(adapter.closeDocument).toHaveBeenCalledOnce());await act(async()=>{});expect(update).not.toHaveBeenCalled();
 rerender({current:{...workspace}});await waitFor(()=>expect(adapter.closeDocument).toHaveBeenCalledTimes(2));await waitFor(()=>expect(update).toHaveBeenCalledOnce());
});
it('integrates live recovery ownership with abandoned-source cleanup',async()=>{
 const {workspace,adapter,tab}=branch(),update=vi.fn();
 const before={...workspace,tabs:[{...tab,sourceIds:['original','abandoned'],history:{...tab.history,present:{...tab.history.present,pages:tab.history.present.pages.map(p=>({...p,sourceId:'abandoned'}))}}}]};
 vi.spyOn(recoveryApi,'load').mockResolvedValue(null);
 let finish!:()=>void;vi.spyOn(recoveryApi,'save').mockImplementation(()=>new Promise<void>(resolve=>finish=resolve));
 const positions={current:new Map()};
 const {result,rerender}=renderHook(({current})=>{
  const recovery=useWorkspaceRecovery(true,current,update,positions);
  useWorkspaceSources(current,update,recovery.sourceOwnership);return recovery;
 },{initialProps:{current:before}});
 await waitFor(()=>expect(result.current.ready).toBe(true));let saving!:Promise<void>;
 act(()=>{saving=result.current.checkpoint(before);});await waitFor(()=>expect(recoveryApi.save).toHaveBeenCalledOnce());
 rerender({current:workspace});await act(async()=>{});expect(adapter.closeDocument).not.toHaveBeenCalled();
 await act(async()=>{finish();await saving;});await waitFor(()=>expect(adapter.closeDocument).toHaveBeenCalledWith('abandoned'));
});
