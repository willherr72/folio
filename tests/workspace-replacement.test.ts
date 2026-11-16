import {expect,it} from 'vitest';
import {nativeAdapter} from '../src/editor/adapter';
import {createSession} from '../src/editor/workspace';
import {commit,redo,undo} from '../src/editor/model';
import {captureWorkspaceReplacement,commitWorkspaceReplacement,workspaceReplacementPages,type WorkspaceReplacementResult} from '../src/editor/workspace-replacement';
import type {ProtectionInfo,TextOverlay} from '../src/editor/types';

const size={width:300,height:400};
function setup(){
 const session=createSession({id:'source',name:'Source.pdf',pages:[size,size]},nativeAdapter,1);
 const overlay:TextOverlay={type:'text',id:'answer',text:'Alpha',x:12,y:24,fontSize:12,color:'#123456',area:{version:1,width:100,height:40,inset:2,lineSpacing:1.2,alignment:'left'}};
 const first={...session.history.present.pages[0],id:'first',crop:{x:10,y:20,width:200,height:300},rotation:90 as const,overlays:[overlay]};
 const document={...session.history.present,pages:[first,{...first,id:'import',sourceId:'imported'}, {...session.history.present.pages[1],id:'second'}, {...first,id:'duplicate'}],metadata:{mode:'update' as const,title:'Pending title'},bookmarks:[{id:'bookmark',title:'Pending bookmark',level:0,destination:{kind:'page' as const,value:first.id}}],selectedPageId:'duplicate',selectedOverlayId:'answer'};
 const current={...session,sourceIds:['source','imported','undo-version'],history:{...session.history,present:document}};
 const borrowed=[...current.sourceIds,'other-tab'];
 const result:WorkspaceReplacementResult={sources:[{sourceId:'source',document:{id:'candidate',name:'Untrusted native title',pages:[size,size,size]},pageIndices:[0,1,2]},{sourceId:'imported',document:{id:'import-candidate',name:'Imported',pages:[size]},pageIndices:[0]}],overlays:[{pageId:'first',overlayId:'answer',expectedText:'Alpha',text:'Beta'}]};
 return {session:current,document,borrowed,result,overlay};
}
it('captures a detached frozen model without freezing the live editor',()=>{
 const {session,borrowed}=setup(),request=captureWorkspaceReplacement(session,borrowed);
 expect(request.history).toBe(session.history);expect(request.adapter).toBe(session.adapter);
 expect(request.document).toEqual(session.history.present);expect(request.document).not.toBe(session.history.present);
 expect(Object.isFrozen(request.document.pages[0].overlays[0])).toBe(true);
 expect(Object.isFrozen(session.history.present.pages[0])).toBe(false);
 borrowed.push('later');expect(request.borrowedSourceIds).not.toContain('later');
});
it('applies multiple source and overlay edits atomically and preserves model state through one Undo/Redo',()=>{
 const {session,document,borrowed,result,overlay}=setup(),request=captureWorkspaceReplacement(session,borrowed);
 const next=commitWorkspaceReplacement(session,request,result,borrowed),after=next.history.present;
 expect(after.pages.map(p=>[p.sourceId,p.pageIndex])).toEqual([['candidate',0],['import-candidate',0],['candidate',1],['candidate',2]]);
 expect(after.pages.map(p=>p.id)).toEqual(document.pages.map(p=>p.id));
 expect(after.pages[0].crop).toBe(document.pages[0].crop);expect(after.pages[0].rotation).toBe(90);
 expect(after.pages[0].overlays[0]).toEqual({...overlay,text:'Beta'});
 expect(after.pages[3].overlays[0]).toBe(overlay);
 expect(after.metadata).toBe(document.metadata);expect(after.bookmarks).toBe(document.bookmarks);
 expect(after.name).toBe(document.name);expect(after.selectedPageId).toBe('duplicate');expect(after.selectedOverlayId).toBeNull();
 expect(next.sourceIds).toEqual([...session.sourceIds,'candidate','import-candidate']);
 expect(next.savedDigest).toBe(session.savedDigest);expect(next.navigationHistory).toBe(session.navigationHistory);
 expect(next.history.past).toHaveLength(1);expect(undo(next.history).present).toBe(document);
 expect(redo(undo(next.history)).present).toBe(after);expect(overlay.text).toBe('Alpha');
});
it('preserves unaffected source identity and permits page-qualified overlay-only edits and removal',()=>{
 const {session,document,borrowed}=setup(),request=captureWorkspaceReplacement(session,borrowed);
 const result:WorkspaceReplacementResult={sources:[],overlays:[{pageId:'duplicate',overlayId:'answer',expectedText:'Alpha',text:null}]};
 const next=commitWorkspaceReplacement(session,request,result,borrowed);
 expect(next.history.present.pages[0]).toBe(document.pages[0]);expect(next.history.present.pages[3].overlays).toEqual([]);
 expect(next.sourceIds).toBe(session.sourceIds);expect(undo(next.history).present).toBe(document);
});
it('rejects stale tab, adapter, history and in-place model changes',()=>{
 const {session,borrowed,result}=setup(),request=captureWorkspaceReplacement(session,borrowed);
 for(const other of [{...session,id:'different'}, {...session,adapter:{...nativeAdapter}}, {...session,history:commit(session.history,d=>({...d,name:'Renamed'}))}, {...session,history:{...session.history}}]){
  expect(()=>commitWorkspaceReplacement(other,request,result,borrowed)).toThrow(/changed/);
 }
 const liveOverlay=session.history.present.pages[0].overlays[0];
 if(liveOverlay.type==='text')liveOverlay.color='#ffffff';
 expect(()=>commitWorkspaceReplacement(session,request,result,borrowed)).toThrow(/changed/);
});
it('rejects global borrowed IDs, duplicate candidates and source aliases, including newly opened tabs',()=>{
 const {session,borrowed,result}=setup(),request=captureWorkspaceReplacement(session,borrowed);
 for(const id of ['source','undo-version','other-tab','later-tab']){
  const bad=structuredClone(result);bad.sources[0].document.id=id;
  expect(()=>commitWorkspaceReplacement(session,request,bad,[...borrowed,'later-tab'])).toThrow(/new source/);
 }
 const duplicate=structuredClone(result);duplicate.sources[1].document.id='candidate';
 expect(()=>workspaceReplacementPages(request,duplicate,borrowed)).toThrow(/new source/);
 const alias=structuredClone(result);alias.sources[1].sourceId='source';
 expect(()=>workspaceReplacementPages(request,alias,borrowed)).toThrow();
});
it('rejects malformed, swapped, conflated and dimension-changing mappings before committing anything',()=>{
 const {session,document,borrowed,result}=setup(),request=captureWorkspaceReplacement(session,borrowed);
 for(const mapping of [[0,1],[1,0,2],[0,0,2],[0,1,-1],[0,1,1.5],[0,1,99]]){
  const bad=structuredClone(result);bad.sources[0].pageIndices=mapping;
  expect(()=>commitWorkspaceReplacement(session,request,bad,borrowed)).toThrow();
 }
 const bad=structuredClone(result);bad.sources[1].document.pages[0].height=401;
 expect(()=>commitWorkspaceReplacement(session,request,bad,borrowed)).toThrow(/dimensions/);
 expect(session.history.present).toBe(document);expect(session.history.past).toEqual([]);
});
it('retains protection exactly and checks content/copy/annotation/duplicate permissions',()=>{
 const {session,borrowed,result}=setup();
 const protection:ProtectionInfo={documentId:'origin',owner:false,revision:4,permissions:{print:true,printHighQuality:true,copyContent:true,editContent:true,annotate:true,fillForms:true,assemble:true}};
 for(const page of session.history.present.pages)page.protection=structuredClone(protection);
 for(const source of result.sources)source.document.protection=structuredClone(protection);
 expect(()=>commitWorkspaceReplacement(session,captureWorkspaceReplacement(session,borrowed),result,borrowed)).not.toThrow();
 const changed=structuredClone(result);changed.sources[1].document.protection!.documentId='other-origin';
 expect(()=>commitWorkspaceReplacement(session,captureWorkspaceReplacement(session,borrowed),changed,borrowed)).toThrow(/protection/);
 for(const permission of ['copyContent','editContent','annotate','assemble'] as const){
  const current=setup();for(const page of current.session.history.present.pages)page.protection={...protection,permissions:{...protection.permissions,[permission]:false}};
  for(const source of current.result.sources)source.document.protection=structuredClone(current.session.history.present.pages[0].protection);
  expect(()=>commitWorkspaceReplacement(current.session,captureWorkspaceReplacement(current.session,current.borrowed),current.result,current.borrowed)).toThrow(/permission/);
 }
});
it('rejects stale, duplicate and non-text overlay patches and refuses empty transactions',()=>{
 const {session,borrowed,result}=setup(),request=captureWorkspaceReplacement(session,borrowed);
 for(const patch of [{...result.overlays[0],expectedText:'Changed'},{...result.overlays[0],pageId:'missing'},{...result.overlays[0],overlayId:'missing'},{...result.overlays[0],text:''}]){
  expect(()=>commitWorkspaceReplacement(session,request,{...result,overlays:[patch]},borrowed)).toThrow();
 }
 expect(()=>commitWorkspaceReplacement(session,request,{sources:[],overlays:[result.overlays[0],result.overlays[0]]},borrowed)).toThrow();
 expect(()=>commitWorkspaceReplacement(session,request,{sources:[],overlays:[]},borrowed)).toThrow();
});
it('rejects invalid capture page identities and a workspace outside the supported page bounds',()=>{
 const {session,borrowed}=setup();
 for(const pages of [[],[session.history.present.pages[0],session.history.present.pages[0]],[{...session.history.present.pages[0],pageIndex:-1}],[{...session.history.present.pages[0],width:NaN}]]){
  expect(()=>captureWorkspaceReplacement({...session,history:{...session.history,present:{...session.history.present,pages}}},borrowed)).toThrow();
 }
});
it('keeps reordered sparse source pages and several independent duplicate occurrences',()=>{
 const {session,borrowed}=setup();
 const first=session.history.present.pages[0],second=session.history.present.pages[2];
 const document={...session.history.present,pages:[{...second,pageIndex:7},first,{...first,id:'dup-two'},{...first,id:'dup-three'}]};
 const current={...session,history:{...session.history,present:document}},request=captureWorkspaceReplacement(current,borrowed);
 const result:WorkspaceReplacementResult={sources:[{sourceId:'source',document:{id:'candidate',name:'New',pages:Array.from({length:10},()=>({...size}))},pageIndices:[7,0,8,9]}],overlays:[]};
 const next=commitWorkspaceReplacement(current,request,result,borrowed);
 expect(next.history.present.pages.map(p=>p.pageIndex)).toEqual([7,0,8,9]);
 expect(next.history.present.pages.map(p=>p.id)).toEqual(document.pages.map(p=>p.id));
 const bad=structuredClone(result);bad.sources[0].pageIndices=[8,8,9,9];
 expect(()=>workspaceReplacementPages(request,bad,borrowed)).toThrow(/conflated/);
});
it('copies replacement text into pages so later response mutation cannot alter committed history',()=>{
 const {session,borrowed,result}=setup(),request=captureWorkspaceReplacement(session,borrowed);
 const next=commitWorkspaceReplacement(session,request,result,borrowed);
 result.overlays[0].text='Changed reply';result.sources[0].document.id='Changed ID';result.sources[0].pageIndices[0]=2;
 const first=next.history.present.pages[0];expect(first.sourceId).toBe('candidate');expect(first.pageIndex).toBe(0);
 expect(first.overlays[0]).toMatchObject({type:'text',text:'Beta'});
});
it('supports multiline and non-BMP added text while rejecting invalid Unicode and excessive text batches',()=>{
 const {session,borrowed,result}=setup(),request=captureWorkspaceReplacement(session,borrowed);
 const replacement={sources:[],overlays:[{...result.overlays[0],text:'First\nSecond 😀'}]};
 expect(commitWorkspaceReplacement(session,request,replacement,borrowed).history.present.pages[0].overlays[0]).toMatchObject({text:'First\nSecond 😀'});
 for(const text of ['bad\u0000text','bad\ud800text','x'.repeat(512*1024)]){
  expect(()=>commitWorkspaceReplacement(session,request,{sources:[],overlays:[{...result.overlays[0],text}]},borrowed)).toThrow();
 }
});
it('rejects ambiguous overlay identities rather than editing more than one object',()=>{
 const {session,borrowed,result,overlay}=setup();session.history.present.pages[0].overlays.push({...overlay});
 expect(()=>captureWorkspaceReplacement(session,borrowed)).toThrow(/overlay/);
 const current=setup();current.session.history.present.pages[0].overlays=[{type:'comment',id:'answer',x:0,y:0,text:'Alpha',color:'#000000'}];
 const request=captureWorkspaceReplacement(current.session,current.borrowed);
 expect(()=>commitWorkspaceReplacement(current.session,request,{sources:[],overlays:result.overlays},current.borrowed)).toThrow(/overlay/);
});
it('enforces capture bounds without rejecting the last supported workspace page',()=>{
 const {session,borrowed}=setup(),first=session.history.present.pages[0];
 const pages=Array.from({length:10_000},(_,i)=>({...first,id:`page-${i}`}));
 const current={...session,history:{...session.history,present:{...session.history.present,pages}}};
 expect(captureWorkspaceReplacement(current,borrowed).document.pages).toHaveLength(10_000);
 expect(()=>captureWorkspaceReplacement({...current,history:{...current.history,present:{...current.history.present,pages:[...pages,{...first,id:'one-too-many'}]}}},borrowed)).toThrow(/10,000/);
});
