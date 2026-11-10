import {expect,it,vi} from 'vitest';
import {captureTextRemoval,commitTextRemoval,TextRemovalWorkflow} from '../src/editor/text-removal-workflow';
import type {TextRemovalPreview} from '../src/editor/text-removal';
import {createSession} from '../src/editor/workspace';
import {createDemoAdapter,type FolioAdapter} from '../src/editor/adapter';
import {undo,redo,planDigest} from '../src/editor/model';
import type {DocumentInfo} from '../src/editor/types';

function fixture(){
 const adapter=createDemoAdapter();
 const tab=createSession({id:'original',name:'Original.pdf',pages:[{width:500,height:300},{width:500,height:300}]},adapter,100);
 tab.sourceIds.push('earlier-edit');
 tab.history.present.pages[0]={...tab.history.present.pages[0],rotation:90,crop:{x:10,y:20,width:200,height:100},overlays:[{id:'text',type:'text',x:20,y:30,text:'Added',fontSize:12,fontName:'Helvetica',color:'#000000'}]};
 tab.history.present.bookmarks=[{id:'bookmark',title:'Old',level:0,destination:{kind:'array',value:[{kind:'page',value:tab.history.present.pages[0].id},{kind:'name',value:[70,105,116]}]}}];
 const result:DocumentInfo={id:'candidate',name:'Text removal preview.pdf',pages:[{width:100,height:200},{width:500,height:300}],bookmarks:[]};
 return {tab,adapter,result};
}
it('adopts the complete source once, preserves page identity and all inputs, and exactly undoes/redoes',()=>{
 const {tab,result}=fixture(),before=tab.history.present,request=captureTextRemoval(tab);
 const next=commitTextRemoval(tab,request,result);
 expect(next.history.past).toEqual([before]);
 expect(next.sourceIds).toEqual(['original','earlier-edit','candidate']);
 expect(next.savedDigest).toBe(tab.savedDigest);expect(next.history.present.name).toBe('Original.pdf');
 expect(next.history.present.pages.map(p=>p.id)).toEqual(before.pages.map(p=>p.id));
 expect(next.history.present.pages.map(p=>[p.sourceId,p.pageIndex,p.width,p.height,p.rotation,p.crop,p.overlays])).toEqual([['candidate',0,100,200,0,undefined,[]],['candidate',1,500,300,0,undefined,[]]]);
 expect(next.history.present.bookmarks).toEqual([]);expect(next.history.present.selectedOverlayId).toBeNull();
 expect(undo(next.history).present).toBe(before);expect(redo(undo(next.history)).present).toBe(next.history.present);
 expect(planDigest(next.history.present)).not.toBe(next.savedDigest);
});
it('refuses stale history, tab, adapter and reused sources without changing the session',()=>{
 const {tab,result}=fixture(),request=captureTextRemoval(tab);
 for(const changed of [{...tab,id:'other'},{...tab,adapter:createDemoAdapter()},{...tab,history:{...tab.history}},{...tab,sourceIds:[...tab.sourceIds,'candidate']}])expect(()=>commitTextRemoval(changed,request,result)).toThrow();
 expect(tab.history.past).toHaveLength(0);
});
it('rejects omitted pages, invalid geometry and baked-state DTOs',()=>{
 const {tab,result}=fixture(),request=captureTextRemoval(tab);
 for(const info of [{...result,id:''},{...result,pages:[]},{...result,pages:[{width:NaN,height:200},result.pages[1]]},{...result,pages:[{width:101,height:200},result.pages[1]]},{...result,pages:[{...result.pages[0],overlays:tab.history.present.pages[0].overlays},result.pages[1]]},{...result,bookmarkError:'unreadable'}])expect(()=>commitTextRemoval(tab,request,info)).toThrow();
});
it('rejects unreadable navigation and remaps actual candidate bookmark destinations',()=>{
 const {tab,result}=fixture();
 expect(()=>captureTextRemoval({...tab,history:{...tab.history,present:{...tab.history.present,bookmarkError:'cannot read'}}})).toThrow(/bookmark/i);
 result.bookmarks=[{id:'new',title:'Kept',level:0,destination:{kind:'array',value:[{kind:'page',value:'1'},{kind:'name',value:[70,105,116]}]}}];
 const next=commitTextRemoval(tab,captureTextRemoval(tab),result);
 expect(next.history.present.bookmarks?.[0].title).toBe('Kept');
 expect(next.history.present.bookmarks?.[0].destination).toEqual({kind:'array',value:[{kind:'page',value:tab.history.present.pages[1].id},{kind:'name',value:[70,105,116]}]});
});
it('rejects malformed bookmark metadata and nested values before they enter history',()=>{
 const {tab,result}=fixture(),request=captureTextRemoval(tab);
 const malformed=[{id:'bad',title:{unexpected:true},level:0},{id:'bad',title:'Bad',level:0,color:'red'},{id:'bad',title:'Bad',level:-1},{id:'bad',title:'Bad',level:0,destination:{kind:'array',value:[{kind:'page',value:'99'},{kind:'name',value:[70,105,116]}]}},{id:'bad',title:'Bad',level:0,action:{kind:'dictionary',value:[[[83],{kind:'name',value:[-1]}]]}}];
 for(const bookmark of malformed)expect(()=>commitTextRemoval(tab,request,{...result,bookmarks:[bookmark] as any})).toThrow();
 expect(tab.history.present).toBe(request.history.present);expect(tab.history.past).toHaveLength(0);
});

function deferred<T>(){let resolve!:(value:T)=>void;let reject!:(error:Error)=>void;const promise=new Promise<T>((a,b)=>{resolve=a;reject=b;});return {promise,resolve,reject};}
const marks=[{pageIndex:0,rect:{x:10,y:10,width:20,height:10}}];
function workflow(){
 const {tab,adapter,result}=fixture();
 const source={document:{...structuredClone(result),id:'working'},omittedMetadata:['metadata']};
 const preview:TextRemovalPreview={previewId:'preview',sourceId:'working',candidate:structuredClone(result),removedCharacters:3,affectedBlocks:1,expiresInSeconds:900,omittedMetadata:['metadata'],pages:[{pageIndex:0,width:100,height:200,selectedGlyphCount:3,extent:marks[0].rect}]};
 Object.assign(adapter,{beginTextRemovalSession:vi.fn().mockResolvedValue('session'),prepareTextRemovalSource:vi.fn().mockResolvedValue(source),previewSourceTextRemoval:vi.fn().mockResolvedValue(preview),textRemovalPreviewBounds:vi.fn().mockResolvedValue({pageIndex:0,offset:0,total:3,boxes:Array.from({length:3},()=>({...marks[0].rect}))}),applyTextRemovalPreview:vi.fn().mockResolvedValue(result),discardTextRemovalPreview:vi.fn().mockResolvedValue(undefined),releaseTextRemovalSession:vi.fn().mockResolvedValue(undefined),closeDocument:vi.fn().mockResolvedValue(undefined)});
 const guards={isCurrent:vi.fn(()=>true),isBorrowed:vi.fn((id:string)=>tab.sourceIds.includes(id)||id==='foreign')};
 return {tab,adapter:adapter as Required<FolioAdapter>,source,preview,result,guards,flow:new TextRemovalWorkflow(captureTextRemoval(tab),guards)};
}
it('prepares current edits, copies settled DTOs and transfers one exact preview on Apply',async()=>{
 const {flow,adapter,source,preview,result,tab}=workflow();
 const prepared=await flow.prepare();source.document.id='changed';source.omittedMetadata.length=0;
 expect(prepared.document.id).toBe('working');expect(prepared.omittedMetadata).toEqual(['metadata']);
 expect(adapter.prepareTextRemovalSource).toHaveBeenCalledWith('session',tab.history.present.pages,tab.history.present.bookmarks);
 const p=await flow.preview(marks);preview.candidate.id='changed';preview.omittedMetadata.length=0;
 expect(p.candidate.id).toBe('candidate');expect(p.omittedMetadata).toEqual(['metadata']);
 expect(await flow.bounds(p,0,0)).toMatchObject({total:3});
 const adopt=vi.fn(()=>true);await flow.apply(p,adopt);
 expect(adopt).toHaveBeenCalledWith(result,p);expect(adapter.closeDocument).not.toHaveBeenCalled();
 await expect(flow.apply(p,adopt)).rejects.toThrow();expect(adapter.applyTextRemovalPreview).toHaveBeenCalledTimes(1);
 flow.dispose();await Promise.resolve();expect(adapter.releaseTextRemovalSession).toHaveBeenCalledWith('session');
});
it('releases a late Begin after Cancel and never starts preparation',async()=>{
 const {flow,adapter}=workflow(),begin=deferred<string>();vi.mocked(adapter.beginTextRemovalSession).mockReturnValue(begin.promise);
 const operation=flow.prepare();const refused=expect(operation).rejects.toThrow();flow.dispose();flow.dispose();begin.resolve('late-session');await refused;
 expect(adapter.prepareTextRemovalSource).not.toHaveBeenCalled();expect(vi.mocked(adapter.releaseTextRemovalSession).mock.calls).toEqual([['late-session']]);
});
it('releases an abandoned Prepare without closing any original',async()=>{
 const {flow,adapter,source}=workflow(),prepare=deferred<typeof source>();vi.mocked(adapter.prepareTextRemovalSource).mockReturnValue(prepare.promise);
 const operation=flow.prepare(),refused=expect(operation).rejects.toThrow();await vi.waitFor(()=>expect(adapter.prepareTextRemovalSource).toHaveBeenCalled());flow.dispose();prepare.resolve(source);await refused;
 expect(vi.mocked(adapter.releaseTextRemovalSession).mock.calls).toEqual([['session']]);expect(adapter.closeDocument).not.toHaveBeenCalled();
});
it('releases an abandoned Preview and refuses stale history before native Apply',async()=>{
 const {flow,adapter,preview,guards}=workflow();await flow.prepare();
 const pending=deferred<TextRemovalPreview>();vi.mocked(adapter.previewSourceTextRemoval).mockReturnValue(pending.promise);
 const operation=flow.preview(marks),refused=expect(operation).rejects.toThrow();await vi.waitFor(()=>expect(adapter.previewSourceTextRemoval).toHaveBeenCalled());guards.isCurrent.mockReturnValue(false);pending.resolve(preview);await refused;
 expect(adapter.releaseTextRemovalSession).toHaveBeenCalledWith('session');expect(adapter.applyTextRemovalPreview).not.toHaveBeenCalled();
});
it('closes only its known candidate when Apply finishes after Cancel',async()=>{
 const {flow,adapter,result}=workflow();await flow.prepare();const p=await flow.preview(marks);
 const pending=deferred<DocumentInfo>();vi.mocked(adapter.applyTextRemovalPreview).mockReturnValue(pending.promise);
 const adopt=vi.fn(()=>true),operation=flow.apply(p,adopt),refused=expect(operation).rejects.toThrow();flow.dispose();pending.resolve(result);await refused;
 expect(adopt).not.toHaveBeenCalled();expect(vi.mocked(adapter.closeDocument).mock.calls).toEqual([['candidate']]);
});
it('rejects borrowed and malformed prepared documents without generic closing',async()=>{
 for(const id of ['foreign','original','']){
  const {flow,adapter,source}=workflow();source.document.id=id;await expect(flow.prepare()).rejects.toThrow();flow.dispose();
  expect(adapter.closeDocument).not.toHaveBeenCalled();expect(adapter.releaseTextRemovalSession).toHaveBeenCalledWith('session');
 }
});
it('refuses forged candidate IDs, page summaries and incomplete omission receipts',async()=>{
 for(const mutate of [(p:TextRemovalPreview)=>{p.candidate.id='foreign';},(p:TextRemovalPreview)=>{p.sourceId='other';},(p:TextRemovalPreview)=>{p.pages[0].pageIndex=3;},(p:TextRemovalPreview)=>{p.omittedMetadata=[];}]){
  const {flow,preview,adapter}=workflow();await flow.prepare();mutate(preview);await expect(flow.preview(marks)).rejects.toThrow();flow.dispose();expect(adapter.applyTextRemovalPreview).not.toHaveBeenCalled();expect(adapter.closeDocument).not.toHaveBeenCalled();
 }
});
it('invalidates marks before discarding and never applies a stale or caller-modified preview',async()=>{
 const {flow,adapter}=workflow();await flow.prepare();const p=await flow.preview(marks);flow.invalidate();
 await expect(flow.apply(p,()=>true)).rejects.toThrow();expect(adapter.applyTextRemovalPreview).not.toHaveBeenCalled();
 await vi.waitFor(()=>expect(adapter.discardTextRemovalPreview).toHaveBeenCalledWith('session','preview'));flow.dispose();
});
it('does not accept malformed marks or unbounded bounds pages',async()=>{
 const {flow,adapter}=workflow();await flow.prepare();
 for(const values of [[],Array(65).fill(marks[0]),[{pageIndex:2,rect:marks[0].rect}],[{pageIndex:0,rect:{x:-1,y:0,width:10,height:10}}]])await expect(flow.preview(values)).rejects.toThrow();
 expect(adapter.previewSourceTextRemoval).not.toHaveBeenCalled();const p=await flow.preview(marks);
 vi.mocked(adapter.textRemovalPreviewBounds).mockResolvedValue({pageIndex:0,offset:1,total:3,boxes:[]});await expect(flow.bounds(p,0,0)).rejects.toThrow();
 vi.mocked(adapter.textRemovalPreviewBounds).mockResolvedValue({pageIndex:0,offset:0,total:5000000,boxes:[]});await expect(flow.bounds(p,0,0)).rejects.toThrow();flow.dispose();
});
it('closes the known candidate on rejected, mismatched or declined Apply without closing a returned foreign ID',async()=>{
 for(const mode of ['reject','wrong','decline']){
  const {flow,adapter}=workflow();await flow.prepare();const p=await flow.preview(marks);
  if(mode==='reject')vi.mocked(adapter.applyTextRemovalPreview).mockRejectedValue(new Error('lost response'));
  if(mode==='wrong')vi.mocked(adapter.applyTextRemovalPreview).mockResolvedValue({...p.candidate,id:'foreign'});
  await expect(flow.apply(p,()=>mode!=='decline')).rejects.toThrow();expect(vi.mocked(adapter.closeDocument).mock.calls).toEqual([['candidate']]);
  await expect(flow.apply(p,()=>true)).rejects.toThrow();expect(adapter.applyTextRemovalPreview).toHaveBeenCalledTimes(1);flow.dispose();
 }
});
it('invalidates preview authority after failed bounds verification even on another page',async()=>{
 const {flow,adapter}=workflow();await flow.prepare();const p=await flow.preview(marks);
 vi.mocked(adapter.textRemovalPreviewBounds).mockRejectedValue(new Error('Bounds transport failed'));
 await expect(flow.bounds(p,0,0)).rejects.toThrow('Bounds transport failed');
 await expect(flow.apply(p,()=>true)).rejects.toThrow();expect(adapter.applyTextRemovalPreview).not.toHaveBeenCalled();
 await vi.waitFor(()=>expect(adapter.discardTextRemovalPreview).toHaveBeenCalledWith('session','preview'));flow.dispose();
});
