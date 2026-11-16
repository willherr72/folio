import {afterEach,expect,it,vi} from 'vitest';
import {waitFor} from '@testing-library/react';
import {nativeAdapter} from '../src/editor/adapter';
import {createSession} from '../src/editor/workspace';
import {captureWorkspaceReplacement} from '../src/editor/workspace-replacement';
import {WorkspaceReplacementWorkflow} from '../src/editor/workspace-replacement-workflow';
import type {WorkspaceReplacementIntent} from '../src/editor/workspace-replacement-transport';
afterEach(()=>vi.restoreAllMocks());
function deferred<T>(){let resolve!:(value:T)=>void;const promise=new Promise<T>(done=>{resolve=done;});return {promise,resolve};}
function setup(){
 const size={width:300,height:400};let pageId='';
 const result={sources:[{sourceId:'source',document:{id:'candidate',name:'Native.pdf',pages:[size]},pageIndices:[0]}],overlays:[]};
 const adapter={...nativeAdapter,beginWorkspaceReplacement:vi.fn().mockResolvedValue('session'),planWorkspaceReplacement:vi.fn(async(_session:string,intent:WorkspaceReplacementIntent)=>({planId:'plan',query:intent.query,replacement:intent.replacement,options:intent.options,pages:[{pageId,matches:[{matchIndex:0,overlayId:null,rects:[{x:30,y:40,width:30,height:12}],selected:true,blocker:null}]}],selectedCount:1,blockedCount:0,affectedPageIds:[pageId],batchBlocker:null,canApply:true})),previewWorkspaceReplacement:vi.fn(async()=>({previewId:'preview',result,affectedPageIds:[pageId],expiresInSeconds:900})),applyWorkspaceReplacement:vi.fn(async()=>result),cancelWorkspaceReplacement:vi.fn().mockResolvedValue(undefined),releaseWorkspaceReplacement:vi.fn().mockResolvedValue(undefined),closeDocument:vi.fn().mockResolvedValue(undefined)};
 const session=createSession({id:'source',name:'Authored.pdf',pages:[size]},adapter,1);pageId=session.history.present.pages[0].id;
 session.history.present.pages[0].crop={x:10,y:20,width:200,height:300};session.history.present.pages[0].overlays=[{type:'text',id:'added',text:'Retain',x:20,y:80,fontSize:12,color:'#123456'}];
 let current=true;const borrowed=['source'];const request=captureWorkspaceReplacement(session,borrowed),flow=new WorkspaceReplacementWorkflow(request,{isCurrent:()=>current,borrowedIds:()=>borrowed});
 const intent={query:'Alpha',replacement:'New',options:{matchCase:false,wholeWords:false}};
 return {session,adapter,result,borrowed,request,flow,intent,setCurrent:(value:boolean)=>{current=value;}};
}
async function preview(value:ReturnType<typeof setup>){await value.flow.begin();const plan=await value.flow.check(value.intent);return value.flow.preview(plan);}
it('detaches preview data without freezing or mutating the live editor model',async()=>{
 const value=setup(),p=await preview(value);value.result.sources[0].document.name='Mutated response';
 expect(p.result.sources[0].document.name).toBe('Native.pdf');expect(Object.isFrozen(p.after[0].crop)).toBe(true);
 expect(Object.isFrozen(value.session.history.present.pages[0].crop)).toBe(false);expect(Object.isFrozen(value.session.history.present.pages[0].overlays[0])).toBe(false);
 expect(value.session.history.present.pages[0].sourceId).toBe('source');value.flow.dispose();
});
it('releases a session whose Begin response arrives after disposal',async()=>{
 const value=setup(),pending=deferred<string>();value.adapter.beginWorkspaceReplacement.mockImplementationOnce(()=>pending.promise);
 const begin=value.flow.begin();value.flow.dispose();pending.resolve('late-session');await expect(begin).rejects.toThrow(/cancelled/);
 await waitFor(()=>expect(value.adapter.releaseWorkspaceReplacement).toHaveBeenCalledTimes(1));expect(value.adapter.releaseWorkspaceReplacement).toHaveBeenCalledWith('late-session');expect(value.adapter.closeDocument).not.toHaveBeenCalled();
});
it('disposes late preview ownership through the native session without closing unvalidated IDs',async()=>{
 const value=setup();await value.flow.begin();const plan=await value.flow.check(value.intent);const pending=deferred<Awaited<ReturnType<typeof value.adapter.previewWorkspaceReplacement>>>();value.adapter.previewWorkspaceReplacement.mockImplementationOnce(()=>pending.promise);
 const work=value.flow.preview(plan);value.flow.dispose();pending.resolve({previewId:'late',result:{...value.result,sources:[{...value.result.sources[0],document:{...value.result.sources[0].document,id:'foreign'}}]},affectedPageIds:plan.affectedPageIds,expiresInSeconds:900});
 await expect(work).rejects.toThrow(/cancelled/);await waitFor(()=>expect(value.adapter.releaseWorkspaceReplacement).toHaveBeenCalledWith('session'));expect(value.adapter.closeDocument).not.toHaveBeenCalled();
});
it('rejects a changed Apply result and closes only its previously validated candidate',async()=>{
 const value=setup(),p=await preview(value);value.adapter.applyWorkspaceReplacement.mockResolvedValueOnce({...value.result,sources:[{...value.result.sources[0],document:{...value.result.sources[0].document,id:'foreign'}}]});const adopt=vi.fn(()=>true);
 await expect(value.flow.apply(p,adopt)).rejects.toThrow(/different/);await waitFor(()=>expect(value.adapter.closeDocument).toHaveBeenCalledTimes(1));expect(value.adapter.closeDocument).toHaveBeenCalledWith('candidate');
 expect(value.adapter.closeDocument).not.toHaveBeenCalledWith('foreign');expect(value.adapter.closeDocument).not.toHaveBeenCalledWith('source');expect(adopt).not.toHaveBeenCalled();
});
it('rechecks current ownership before asynchronously closing a cancelled Apply candidate',async()=>{
 const value=setup(),p=await preview(value),pending=deferred<typeof value.result>();value.adapter.applyWorkspaceReplacement.mockImplementationOnce(()=>pending.promise);
 const work=value.flow.apply(p,()=>true);value.flow.dispose();value.borrowed.push('candidate');pending.resolve(value.result);await expect(work).rejects.toThrow(/cancelled/);
 await waitFor(()=>expect(value.adapter.releaseWorkspaceReplacement).toHaveBeenCalled());expect(value.adapter.closeDocument).not.toHaveBeenCalled();
});
it('does not apply an expired preview or a response for a no-longer-current document',async()=>{
 const clock=vi.spyOn(Date,'now').mockReturnValue(0);const value=setup(),p=await preview(value);clock.mockReturnValue(900_001);
 await expect(value.flow.apply(p,()=>true)).rejects.toThrow(/again/);expect(value.adapter.applyWorkspaceReplacement).not.toHaveBeenCalled();value.setCurrent(false);
 await expect(value.flow.check(value.intent)).rejects.toThrow(/document changed/);await waitFor(()=>expect(value.adapter.releaseWorkspaceReplacement).toHaveBeenCalled());
});
