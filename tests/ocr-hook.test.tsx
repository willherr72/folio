import {cleanup,act,renderHook,waitFor} from '@testing-library/react';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {useOcrWorkflow} from '../src/editor/ocr/useOcrWorkflow';
import {startOcrSession} from '../src/editor/ocr/session';
import {nativeAdapter,type FolioAdapter} from '../src/editor/adapter';
import {ocrTargets,editOcrWord} from '../src/editor/ocr/workflow';
import type {OcrPage,OcrPreparedPage,OcrSession} from '../src/editor/ocr/types';
vi.mock('../src/editor/ocr/session',()=>({startOcrSession:vi.fn()}));
const page:OcrPage={width:100,height:80,blocks:[{paragraphs:[{lines:[{words:[{text:'café',confidence:65,bounds:{x:5,y:5,width:30,height:10}}]}]}]}]};
function prepared(owned:OcrPage|null=null):OcrPreparedPage {
 const png=new Uint8Array(33);png.set([137,80,78,71,13,10,26,10]);const v=new DataView(png.buffer);v.setUint32(8,13);png.set([73,72,68,82],12);v.setUint32(16,100);v.setUint32(20,80);
 return {png,width:100,height:80,effectiveDpi:300,exclusions:[],owned};
}
const pages=[0,1].map(i=>({id:`p${i}`,sourceId:'source',pageIndex:i,width:200,height:160,rotation:0 as const,overlays:[]}));
const targets=ocrTargets(pages,pages);
function deferred<T>(){let resolve!:(value:T)=>void;const promise=new Promise<T>(r=>{resolve=r;});return {promise,resolve};}
let worker:OcrSession;
beforeEach(()=>{
 vi.clearAllMocks();let next=0;vi.stubGlobal('URL',class extends URL {static createObjectURL=vi.fn(()=>`blob:ocr-${++next}`);static revokeObjectURL=vi.fn();});
 worker={recognize:vi.fn().mockResolvedValue(page),dispose:vi.fn()};vi.mocked(startOcrSession).mockResolvedValue(worker);
});
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
function setup(overrides:Partial<FolioAdapter>={},adopt=vi.fn().mockReturnValue(true)){
 const adapter={...nativeAdapter,prepareOcrPage:vi.fn().mockResolvedValue(prepared()),applyOcr:vi.fn().mockResolvedValue({id:'candidate',name:'Scan',pages:[{width:200,height:160},{width:200,height:160}]}),closeDocument:vi.fn().mockResolvedValue(undefined),...overrides};
 const hook=renderHook(()=>useOcrWorkflow({adapter,knownSourceIds:()=>new Set(['source']),onAdopt:adopt}));return {...hook,adapter,adopt};
}
it('prepares pages sequentially using one worker and retains review drafts after disposing it',async()=>{
 const hook=setup();await act(async()=>hook.result.current.start(targets,['eng']));
 expect(hook.result.current.stage).toBe('review');expect(hook.result.current.drafts).toHaveLength(2);expect(hook.result.current.changed).toBe(2);
 expect(startOcrSession).toHaveBeenCalledTimes(1);expect(worker.recognize).toHaveBeenCalledTimes(2);expect(worker.dispose).toHaveBeenCalledTimes(1);
 expect(hook.result.current.active).toBe(0);expect(hook.result.current.preview).toMatch(/^blob:/);
 const url=hook.result.current.preview;hook.unmount();expect(URL.revokeObjectURL).toHaveBeenCalledWith(url);
});
it('opens verified existing OCR for correction without starting recognition',async()=>{
 const hook=setup({prepareOcrPage:vi.fn().mockResolvedValue(prepared(page))});await act(async()=>hook.result.current.start([targets[0]],['eng']));
 expect(startOcrSession).not.toHaveBeenCalled();expect(hook.result.current.changed).toBe(0);
 act(()=>hook.result.current.update(d=>editOcrWord(d,0,'corrected')));expect(hook.result.current.changed).toBe(1);
 await act(async()=>hook.result.current.apply());expect(hook.adopt).toHaveBeenCalledTimes(1);expect(hook.adapter.closeDocument).not.toHaveBeenCalled();
 expect(hook.adapter.applyOcr).toHaveBeenCalledWith('source',[expect.objectContaining({preserveOwnedOrientation:true})]);
});
it('uses the current orientation for fresh recognition of an existing layer',async()=>{
 const hook=setup({prepareOcrPage:vi.fn().mockResolvedValue(prepared(page))});await act(async()=>hook.result.current.start([targets[0]],['eng']));
 await act(async()=>hook.result.current.recognizeAgain(['eng']));act(()=>hook.result.current.update(d=>editOcrWord(d,0,'recognized again')));
 await act(async()=>hook.result.current.apply());expect(hook.adapter.applyOcr).toHaveBeenCalledWith('source',[expect.objectContaining({preserveOwnedOrientation:false})]);
});
it('cancels during native preparation before creating a worker or publishing a preview',async()=>{
 const pending=deferred<OcrPreparedPage>(),hook=setup({prepareOcrPage:vi.fn().mockReturnValue(pending.promise)});let running!:Promise<void>;
 act(()=>{running=hook.result.current.start([targets[0]],['eng']);});act(()=>hook.result.current.cancel());
 await act(async()=>{pending.resolve(prepared());await running;});expect(startOcrSession).not.toHaveBeenCalled();expect(URL.createObjectURL).not.toHaveBeenCalled();expect(hook.result.current.drafts).toEqual([]);
});
it('terminates a late startup worker and ignores recognized output after cancellation',async()=>{
 const pending=deferred<OcrSession>();vi.mocked(startOcrSession).mockReturnValue(pending.promise);const hook=setup();let running!:Promise<void>;
 act(()=>{running=hook.result.current.start([targets[0]],['eng']);});await waitFor(()=>expect(startOcrSession).toHaveBeenCalled());act(()=>hook.result.current.cancel());
 await act(async()=>{pending.resolve(worker);await running;});expect(worker.dispose).toHaveBeenCalled();expect(worker.recognize).not.toHaveBeenCalled();
 const result=deferred<OcrPage>();worker.recognize=vi.fn().mockReturnValue(result.promise);vi.mocked(startOcrSession).mockResolvedValue(worker);
 act(()=>{running=hook.result.current.start([targets[0]],['eng']);});await waitFor(()=>expect(worker.recognize).toHaveBeenCalled());act(()=>hook.result.current.cancel());
 await act(async()=>{result.resolve(page);await running;});expect(hook.result.current.drafts).toEqual([]);
});
it('closes a late native candidate after cancel without adopting or discarding the review draft',async()=>{
 const pending=deferred<{id:string;name:string;pages:{width:number;height:number}[]}>(),hook=setup({applyOcr:vi.fn().mockReturnValue(pending.promise)});
 await act(async()=>hook.result.current.start([targets[0]],['eng']));let running!:Promise<boolean>;
 act(()=>{running=hook.result.current.apply();});act(()=>hook.result.current.cancel());
 await act(async()=>{pending.resolve({id:'late',name:'Scan',pages:[{width:200,height:160}]});await running;});
 expect(hook.adapter.closeDocument).toHaveBeenCalledWith('late');expect(hook.adopt).not.toHaveBeenCalled();expect(hook.result.current.drafts).toHaveLength(1);
});
it('keeps corrections available for retry after native apply failure',async()=>{
 const hook=setup({applyOcr:vi.fn().mockRejectedValue(new Error('write failed'))});await act(async()=>hook.result.current.start([targets[0]],['eng']));
 act(()=>hook.result.current.update(d=>editOcrWord(d,0,'corrected')));await act(async()=>hook.result.current.apply());
 expect(hook.result.current.stage).toBe('review');expect(hook.result.current.error).toContain('write failed');expect(hook.result.current.changed).toBe(1);
});
it('ignores out-of-order preview replies and cleans up when unmounted during preparation',async()=>{
 const hook=setup();await act(async()=>hook.result.current.start(targets,['eng']));
 const older=deferred<OcrPreparedPage>(),newer=deferred<OcrPreparedPage>();vi.mocked(hook.adapter.prepareOcrPage!).mockReturnValueOnce(older.promise).mockReturnValueOnce(newer.promise);
 let one!:Promise<void>,two!:Promise<void>;act(()=>{one=hook.result.current.view(1);two=hook.result.current.view(0);});
 await act(async()=>{newer.resolve(prepared());await two;});const preview=hook.result.current.preview;
 await act(async()=>{older.resolve(prepared());await one;});expect(hook.result.current.active).toBe(0);expect(hook.result.current.preview).toBe(preview);
 const late=deferred<OcrPreparedPage>();vi.mocked(hook.adapter.prepareOcrPage!).mockReturnValueOnce(late.promise);act(()=>{one=hook.result.current.view(1);});hook.unmount();late.resolve(prepared());await one;
 expect(hook.adopt).not.toHaveBeenCalled();expect(URL.revokeObjectURL).toHaveBeenCalledWith(preview);
});
