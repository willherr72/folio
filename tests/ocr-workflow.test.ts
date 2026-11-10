import {describe,expect,it,vi} from 'vitest';
import {ocrTargets,createOcrDraft,acceptedOcrPage,editOcrWord,removeOcrWord,ocrDraftChanged,applyOcrDrafts,commitOcrReplacements} from '../src/editor/ocr/workflow';
import {createSession} from '../src/editor/workspace';
import {nativeAdapter,type FolioAdapter} from '../src/editor/adapter';
import {undo,redo,planDigest} from '../src/editor/model';
import type {OcrPage} from '../src/editor/ocr/types';

const info={id:'source',name:'Scan.pdf',pages:[{width:200,height:100},{width:200,height:100}]};
const session=()=>createSession(info,nativeAdapter,100);
const page=():OcrPage=>({width:100,height:100,blocks:[{paragraphs:[{lines:[{words:[
 {text:'Native',confidence:97,bounds:{x:5,y:5,width:20,height:10}},
 {text:'café',confidence:64,bounds:{x:30,y:40,width:25,height:10}},
 {text:'final',confidence:95,bounds:{x:60,y:40,width:25,height:10}},
]}]}]}]});
const draft=()=>{const s=session();return createOcrDraft(ocrTargets(s.history.present.pages,s.history.present.pages)[0],page(),null,[{x:14,y:6,width:2,height:8}]);};
function adapter(apply=vi.fn().mockResolvedValue({...info,id:'candidate'})):FolioAdapter {
 return {...nativeAdapter,applyOcr:apply,closeDocument:vi.fn().mockResolvedValue(undefined)};
}
describe('OCR review and page selection',()=>{
 it('deduplicates source pages in selected order and identifies every workspace copy',()=>{
  const pages=session().history.present.pages,copy={...pages[0],id:'copy',rotation:90 as const};
  const targets=ocrTargets([...pages,copy],[copy,pages[1],pages[0]]);
  expect(targets).toHaveLength(2);expect(targets[0].page).toBe(copy);expect(targets[0].pageNumbers).toEqual([1,3]);
 });
 it('rejects empty, overlarge, unsupported and restricted selections',()=>{
  const pages=session().history.present.pages;
  expect(()=>ocrTargets(pages,[])).toThrow();
  const many=Array.from({length:101},(_,i)=>({...pages[0],id:`p${i}`,pageIndex:i}));expect(()=>ocrTargets(many,many)).toThrow(/100/);
  const restricted={...pages[0],protection:{documentId:'p',owner:false,revision:4,permissions:{editContent:true,copyContent:false,annotate:true,assemble:true,fillForms:true,print:true,printHighQuality:true}}};
  expect(()=>ocrTargets([restricted],[restricted])).toThrow(/copy/i);
 });
 it('shows excluded native words but applies only corrected scan text without mutating input',()=>{
  const original=draft();expect(original.excluded).toEqual([0]);expect(ocrDraftChanged(original)).toBe(true);
  const corrected=editOcrWord(original,1,'café niño');const removed=removeOcrWord(corrected,2,true);
  const accepted=acceptedOcrPage(removed);expect(accepted.blocks[0].paragraphs[0].lines[0].words.map(w=>w.text)).toEqual(['café niño']);
  expect(original.page.blocks[0].paragraphs[0].lines[0].words[1].text).toBe('café');
  expect(()=>editOcrWord(original,0,'duplicate')).toThrow(/existing/i);
  expect(()=>editOcrWord(original,1,'bad\nword')).toThrow();expect(()=>editOcrWord(original,1,'\ud800')).toThrow();
 });
 it('keeps unchanged, blank and native-only drafts clean, while allowing removal of a proven layer',()=>{
  const target=draft().target,blank={width:100,height:100,blocks:[]};
  expect(ocrDraftChanged(createOcrDraft(target,blank,null,[]))).toBe(false);
  const owned=createOcrDraft(target,page(),page(),[]);expect(ocrDraftChanged(owned)).toBe(false);
  let removed=owned;for(let i=0;i<3;i++)removed=removeOcrWord(removed,i,true);
  expect(ocrDraftChanged(removed)).toBe(true);expect(acceptedOcrPage(removed).blocks).toEqual([]);
  const native=createOcrDraft(target,page(),null,[{x:0,y:0,width:100,height:100}]);expect(ocrDraftChanged(native)).toBe(false);
 });
 it('excludes native words when their centers fall in gaps between letters, retaining adjacent scan words',()=>{
  const target=draft().target;
  const reviewed=createOcrDraft(target,page(),null,[{x:6,y:6,width:4,height:8},{x:20,y:6,width:4,height:8}]);
  expect(reviewed.excluded).toEqual([0]);
  expect(acceptedOcrPage(reviewed).blocks[0].paragraphs[0].lines[0].words.map(w=>w.text)).toEqual(['café','final']);
 });
});
describe('atomic source publication',()=>{
 it('commits multiple full sources once, including repeated pages, and undo/redo retains overlays and bookmarks',()=>{
  let s=session(),doc=s.history.present;const first={...doc.pages[0],contentSourceId:'old-alias',rotation:90 as const,overlays:[{type:'comment' as const,id:'note',x:5,y:5,text:'Keep',color:'#000000'}]};
  doc={...doc,pages:[first,{...first,id:'copy'},doc.pages[1],{...first,id:'other',sourceId:'other'}],bookmarks:[{id:'b',title:'Keep',level:0}]};
  s={...s,sourceIds:['source','other'],history:{...s.history,present:doc}};
  const next=commitOcrReplacements(s,doc,[{sourceId:'source',document:{...info,id:'new'}},{sourceId:'other',document:{...info,id:'new-other'}}]);
  expect(next.history.past).toHaveLength(1);expect(next.history.present.pages.map(p=>p.sourceId)).toEqual(['new','new','new','new-other']);
  expect(next.history.present.pages[0]).toEqual({...first,sourceId:'new',contentSourceId:undefined});
  expect(next.history.present.bookmarks).toBe(doc.bookmarks);expect(planDigest(next.history.present)).not.toBe(planDigest(doc));
  expect(undo(next.history).present).toBe(doc);expect(redo(undo(next.history)).present).toBe(next.history.present);
 });
 it('rejects stale state, reused candidate IDs, duplicated sources and changed dimensions before committing',()=>{
  const s=session(),doc=s.history.present,result={sourceId:'source',document:{...info,id:'new'}};
  expect(()=>commitOcrReplacements(s,{...doc},[result])).toThrow(/changed/i);
  for(const replacements of [[{...result,document:info}],[result,result],[{...result,document:{...info,id:'new',pages:[{width:201,height:100}]}}]]){
   expect(()=>commitOcrReplacements(s,doc,replacements)).toThrow();
  }expect(s.history.past).toEqual([]);
 });
 it('adopts once after all native calls and retains sources only on successful adoption',async()=>{
  const a=adapter(),adopt=vi.fn().mockReturnValue(true);
  expect(await applyOcrDrafts(a,[draft()],new Set(['source']),new AbortController().signal,adopt)).toBe(true);
  expect(adopt).toHaveBeenCalledTimes(1);expect(a.closeDocument).not.toHaveBeenCalled();
  expect(a.applyOcr).toHaveBeenCalledWith('source',[expect.objectContaining({pageIndex:0,rotation:0})]);
 });
 it('closes partial candidates on failure and retains the review draft for retry',async()=>{
  const a=adapter(vi.fn().mockResolvedValueOnce({...info,id:'candidate'}).mockRejectedValueOnce(new Error('write failed')));
  const first=draft(),second={...draft(),target:{...draft().target,page:{...draft().target.page,sourceId:'other'}}};const adopt=vi.fn();
  await expect(applyOcrDrafts(a,[first,second],new Set(['source','other']),new AbortController().signal,adopt)).rejects.toThrow('write failed');
  expect(a.closeDocument).toHaveBeenCalledTimes(1);expect(a.closeDocument).toHaveBeenCalledWith('candidate');expect(adopt).not.toHaveBeenCalled();expect(ocrDraftChanged(first)).toBe(true);
 });
 it('closes late results after cancel and never closes an existing source returned in error',async()=>{
  const abort=new AbortController();let resolve!:(value:typeof info)=>void;
  const a=adapter(vi.fn().mockImplementation(()=>new Promise(r=>{resolve=r;}))),adopt=vi.fn();
  const applying=applyOcrDrafts(a,[draft()],new Set(['source']),abort.signal,adopt);abort.abort();resolve({...info,id:'late'});
  await expect(applying).rejects.toMatchObject({name:'AbortError'});expect(a.closeDocument).toHaveBeenCalledTimes(1);expect(a.closeDocument).toHaveBeenCalledWith('late');expect(adopt).not.toHaveBeenCalled();
  const reused=adapter(vi.fn().mockResolvedValue(info));await expect(applyOcrDrafts(reused,[draft()],new Set(['source']),new AbortController().signal,adopt)).rejects.toThrow(/new source/i);
  expect(reused.closeDocument).not.toHaveBeenCalled();
 });
 it('cleans up when the tab rejects adoption and skips unchanged pages',async()=>{
  const a=adapter();await expect(applyOcrDrafts(a,[draft()],new Set(['source']),new AbortController().signal,()=>false)).rejects.toThrow(/changed/i);
  expect(a.closeDocument).toHaveBeenCalledTimes(1);expect(a.closeDocument).toHaveBeenCalledWith('candidate');
  const d=draft(),clean=createOcrDraft(d.target,d.page,d.page,[]);const untouched=adapter();
  expect(await applyOcrDrafts(untouched,[clean],new Set(['source']),new AbortController().signal,()=>true)).toBe(false);expect(untouched.applyOcr).not.toHaveBeenCalled();
 });
});
