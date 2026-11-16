import {cleanup,fireEvent,render,screen,waitFor,within} from '@testing-library/react';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {App} from '../src/App';
import {nativeAdapter} from '../src/editor/adapter';
import {clearSearchTextCache} from '../src/editor/search';
import type {PagePlan} from '../src/editor/types';

vi.mock('../src/editor/useWorkspaceRecovery',()=>({useWorkspaceRecovery:()=>({ready:true,checkpoint:vi.fn().mockResolvedValue(undefined)})}));
vi.mock('@tauri-apps/api/window',()=>({getCurrentWindow:()=>({onCloseRequested:vi.fn().mockResolvedValue(()=>{})})}));
vi.mock('../src/components/DocumentViewport',()=>({DocumentViewport:(props:any)=><output data-testid="plans">{JSON.stringify(props.pages)}</output>}));
const names=['beginWorkspaceReplacement','planWorkspaceReplacement','previewWorkspaceReplacement','applyWorkspaceReplacement','cancelWorkspaceReplacement','releaseWorkspaceReplacement'] as const;
const size={width:300,height:400};let pages:PagePlan[]=[];
let originals:Map<string,PropertyDescriptor|undefined>;
let result:any;
const api:Record<string,ReturnType<typeof vi.fn>>={};
beforeEach(()=>{
 localStorage.clear();Object.defineProperty(window,'__TAURI_INTERNALS__',{configurable:true,value:{invoke:vi.fn().mockResolvedValue(0),transformCallback:()=>0}});
 vi.spyOn(nativeAdapter,'openPdf').mockResolvedValueOnce({id:'replacement-source',name:'Original.pdf',pages:[size,size]}).mockResolvedValue({id:'other-source',name:'Other.pdf',pages:[size]});
 vi.spyOn(nativeAdapter,'renderPage').mockResolvedValue('data:image/png;base64,AA');vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,'renderPagePlan').mockResolvedValue('data:image/png;base64,AA');
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,'getPageText').mockResolvedValue({characters:Array.from('Alpha',(text,i)=>({text,x:30+i*6,y:40,width:6,height:12}))});
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,'searchPage').mockResolvedValue({matches:[[{x:30,y:40,width:30,height:12}]],hasText:true,truncated:false});vi.spyOn(nativeAdapter,'closeDocument').mockResolvedValue();
 originals=new Map(names.map(name=>[name,Object.getOwnPropertyDescriptor(nativeAdapter,name)]));
 api.beginWorkspaceReplacement=vi.fn(async(value:PagePlan[])=>{pages=structuredClone(value);return 'replacement-session';});
 api.planWorkspaceReplacement=vi.fn(async(_session:string,intent:any)=>({planId:'replacement-plan',query:intent.query,replacement:intent.replacement,options:intent.options,pages:pages.map(p=>({pageId:p.id,matches:[{matchIndex:0,overlayId:null,rects:[{x:30,y:40,width:30,height:12}],selected:true,blocker:null}]})),selectedCount:pages.length,blockedCount:0,affectedPageIds:pages.map(p=>p.id),batchBlocker:null,canApply:true}));
 api.previewWorkspaceReplacement=vi.fn(async()=>{
  let count=2;const seen=new Set<number>();const pageIndices=pages.map(p=>{if(seen.has(p.pageIndex))return count++;seen.add(p.pageIndex);return p.pageIndex;});
  result={sources:[{sourceId:'replacement-source',document:{id:'replacement-candidate',name:'Native title',pages:Array.from({length:count},()=>({...size}))},pageIndices}],overlays:[]};
  return {previewId:'replacement-preview',result,affectedPageIds:pages.map(p=>p.id),expiresInSeconds:900};
 });
 api.applyWorkspaceReplacement=vi.fn(async()=>structuredClone(result));api.cancelWorkspaceReplacement=vi.fn().mockResolvedValue(undefined);api.releaseWorkspaceReplacement=vi.fn().mockResolvedValue(undefined);
 Object.assign(nativeAdapter,api);
});
afterEach(()=>{
 cleanup();for(const name of names){const original=originals.get(name);if(original)Object.defineProperty(nativeAdapter,name,original);else delete(nativeAdapter as unknown as Record<string,unknown>)[name];}
 clearSearchTextCache(['replacement-source','replacement-candidate','other-source']);vi.restoreAllMocks();delete(window as any).__TAURI_INTERNALS__;
});
const currentPages=()=>JSON.parse(screen.getByTestId('plans').textContent!);
async function open(){render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole('button',{name:'Open a PDF'}));await screen.findByRole('tab',{name:'Original.pdf'});}
async function enter(){fireEvent.click(screen.getByRole('button',{name:'Find'}));fireEvent.change(screen.getByRole('searchbox'),{target:{value:'Alpha'}});fireEvent.click(screen.getByRole('button',{name:'Replace text'}));return screen.findByRole('dialog',{name:'Replace text'});}
async function preview(){const dialog=await enter();fireEvent.change(within(dialog).getByLabelText('Replace with'),{target:{value:'New'}});fireEvent.click(within(dialog).getByRole('button',{name:'Check matches'}));await waitFor(()=>expect(within(dialog).getByRole('button',{name:'Preview replacements'})).toBeEnabled());fireEvent.click(within(dialog).getByRole('button',{name:'Preview replacements'}));fireEvent.load(await within(dialog).findByAltText('Page before replacement'));fireEvent.load(await within(dialog).findByAltText('Page after replacement'));await waitFor(()=>expect(within(dialog).getByRole('button',{name:'Apply replacements'})).toBeEnabled());return dialog;}
it('opens workspace Replace from Find and commits duplicate-aware changes as one Undo action',async()=>{
 await open();fireEvent.click(screen.getByRole('button',{name:'Duplicate'}));const before=currentPages();const dialog=await preview();
 expect(api.beginWorkspaceReplacement).toHaveBeenCalledWith(before);expect(within(dialog).getByLabelText('Find text')).toHaveValue('Alpha');
 fireEvent.click(within(dialog).getByRole('button',{name:'Apply replacements'}));await waitFor(()=>expect(currentPages().every((p:any)=>p.sourceId==='replacement-candidate')).toBe(true));
 const after=currentPages();expect(after.map((p:any)=>p.id)).toEqual(before.map((p:any)=>p.id));expect(after.map((p:any)=>p.pageIndex)).toEqual([0,2,1]);
 fireEvent.click(screen.getByRole('button',{name:'Undo'}));expect(currentPages()).toEqual(before);fireEvent.click(screen.getByRole('button',{name:'Redo'}));expect(currentPages()).toEqual(after);
 expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith('replacement-source');expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith('replacement-candidate');
});
it('cancels a displayed preview without dirtying the document and releases native ownership',async()=>{
 await open();const before=currentPages();const dialog=await preview();fireEvent.click(within(dialog).getByRole('button',{name:'Cancel'}));
 await waitFor(()=>expect(screen.queryByRole('dialog',{name:'Replace text'})).not.toBeInTheDocument());expect(currentPages()).toEqual(before);expect(screen.queryByLabelText('Unsaved changes')).not.toBeInTheDocument();
 await waitFor(()=>expect(api.releaseWorkspaceReplacement).toHaveBeenCalledWith('replacement-session'));expect(api.applyWorkspaceReplacement).not.toHaveBeenCalled();
});
it('opens from Ctrl+H and invalidates a checked preview when the replacement changes',async()=>{
 await open();fireEvent.keyDown(window,{key:'h',ctrlKey:true});const dialog=await screen.findByRole('dialog',{name:'Replace text'});expect(within(dialog).getByLabelText('Find text')).toHaveFocus();
 fireEvent.click(within(dialog).getByRole('button',{name:'Cancel'}));const next=await preview();fireEvent.change(within(next).getByLabelText('Replace with'),{target:{value:'Another'}});
 expect(within(next).getByRole('button',{name:'Apply replacements'})).toBeDisabled();expect(api.applyWorkspaceReplacement).not.toHaveBeenCalled();
});
it('rejects a candidate alias without closing the current document',async()=>{
 const original=api.previewWorkspaceReplacement.getMockImplementation()!;
 api.previewWorkspaceReplacement.mockImplementationOnce(async(...args:any[])=>{const value=await original(...args);value.result.sources[0].document.id='replacement-source';return value;});
 await open();const before=currentPages();const dialog=await enter();fireEvent.change(within(dialog).getByLabelText('Replace with'),{target:{value:'New'}});fireEvent.click(within(dialog).getByRole('button',{name:'Check matches'}));
 await waitFor(()=>expect(within(dialog).getByRole('button',{name:'Preview replacements'})).toBeEnabled());fireEvent.click(within(dialog).getByRole('button',{name:'Preview replacements'}));
 await within(dialog).findByRole('alert');expect(within(dialog).getByRole('button',{name:'Apply replacements'})).toBeDisabled();expect(currentPages()).toEqual(before);
 fireEvent.click(within(dialog).getByRole('button',{name:'Cancel'}));expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith('replacement-source');
});
it('cancels an in-flight Apply and disposes only the known candidate when its reply arrives late',async()=>{
 let resolve!:(value:any)=>void;api.applyWorkspaceReplacement.mockImplementationOnce(()=>new Promise(done=>{resolve=done;}));
 await open();const before=currentPages();const dialog=await preview();fireEvent.click(within(dialog).getByRole('button',{name:'Apply replacements'}));
 await waitFor(()=>expect(api.applyWorkspaceReplacement).toHaveBeenCalledTimes(1));fireEvent.click(within(dialog).getByRole('button',{name:'Cancel'}));
 await waitFor(()=>expect(screen.queryByRole('dialog',{name:'Replace text'})).not.toBeInTheDocument());resolve(structuredClone(result));
 await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith('replacement-candidate'));expect(currentPages()).toEqual(before);expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith('replacement-source');
});
