import {afterEach,expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {createDemoAdapter,nativeAdapter} from "../src/editor/adapter";
import {clearExistingTextCache,readExistingTextRuns} from "../src/editor/existing-text-cache";
import type {TextRuns} from "../src/editor/types";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
const run={objectIndex:4,text:"Original",sourceText:"Original",fontName:"Helvetica",fontSize:12,bounds:{x:2,y:3,width:40,height:12},supported:true};
const form={...run,objectPath:[1,2],text:"Nested"};
const deferred=()=>{let resolve!:(result:TextRuns)=>void;const promise=new Promise<TextRuns>(done=>resolve=done);return {promise,resolve};};
afterEach(()=>{clearExistingTextCache();vi.clearAllMocks();});

it("shares pending inspection but isolates adapter identity, page, source and selected reader",async()=>{
 const pending=deferred();const list=vi.fn().mockReturnValueOnce(pending.promise).mockResolvedValue({runs:[run]});const adapter={...createDemoAdapter(),listTextRuns:list};
 const a=readExistingTextRuns(adapter,"source",0),b=readExistingTextRuns(adapter,"source",0);await Promise.resolve();expect(list).toHaveBeenCalledTimes(1);
 pending.resolve({runs:[run]});await Promise.all([a,b]);await readExistingTextRuns(adapter,"source",0);expect(list).toHaveBeenCalledTimes(1);
 await readExistingTextRuns(adapter,"source",1);await readExistingTextRuns(adapter,"other",0);await readExistingTextRuns({...adapter},"source",0);expect(list).toHaveBeenCalledTimes(4);
 const changed=vi.fn().mockResolvedValue({runs:[{...run,text:"Changed reader"}]});adapter.listTextRuns=changed;
 expect((await readExistingTextRuns(adapter,"source",0)).runs[0].text).toBe("Changed reader");expect(changed).toHaveBeenCalledTimes(1);
});
it("uses complete native routes and never falls back on a native refusal",async()=>{
 const legacy=vi.fn().mockResolvedValue({runs:[run]});const native=vi.fn().mockRejectedValue(new Error("native refused"));
 const adapter={...createDemoAdapter(),listTextRuns:legacy,listOccurrenceTextRuns:native,replaceTextOccurrence:vi.fn(),listOccurrenceFormTextRuns:vi.fn().mockResolvedValue({runs:[form]}),replaceFormTextOccurrence:vi.fn()};
 const result=await readExistingTextRuns(adapter,"source",0);expect(result.reason).toBe("native refused");expect(result.runs[0]).toMatchObject({objectPath:[1,2],objectIndex:1});expect(legacy).not.toHaveBeenCalled();
 await readExistingTextRuns(adapter,"source",0);expect(native).toHaveBeenCalledTimes(2);
 await readExistingTextRuns({...adapter,replaceTextOccurrence:undefined},"source",0);expect(legacy).toHaveBeenCalledTimes(1);
});
it("retries reason-bearing results and synchronous partial failures",async()=>{
 const top=vi.fn().mockImplementationOnce(()=>{throw Error("temporarily busy");}).mockResolvedValue({runs:[run]});
 const forms=vi.fn().mockResolvedValueOnce({runs:[form]}).mockResolvedValueOnce({runs:[],reason:"retry forms"}).mockResolvedValue({runs:[form]});
 const adapter={...createDemoAdapter(),listTextRuns:top,listFormTextRuns:forms};
 expect((await readExistingTextRuns(adapter,"source",0)).reason).toBe("temporarily busy");
 expect((await readExistingTextRuns(adapter,"source",0)).runs).toHaveLength(2);expect(top).toHaveBeenCalledTimes(2);expect(forms).toHaveBeenCalledTimes(1);
 clearExistingTextCache();
 expect((await readExistingTextRuns(adapter,"source",0)).reason).toBe("retry forms");
 expect((await readExistingTextRuns(adapter,"source",0)).runs).toHaveLength(2);await readExistingTextRuns(adapter,"source",0);expect(top).toHaveBeenCalledTimes(3);expect(forms).toHaveBeenCalledTimes(3);
});
it("isolates both callers and later adapter mutation from retained text, bounds and occurrence paths",async()=>{
 const original={runs:[{...run,bounds:{...run.bounds}}]};const nested={runs:[{...form,bounds:{...form.bounds},objectPath:[1,2]}]};
 const adapter={...createDemoAdapter(),listTextRuns:vi.fn().mockResolvedValue(original),listFormTextRuns:vi.fn().mockResolvedValue(nested)};
 const first=await readExistingTextRuns(adapter,"source",0);first.runs[0].text="Caller changed";first.runs[1].objectPath![0]=999;first.runs[0].bounds.x=123;first.runs.pop();
 original.runs[0].text="Adapter changed";nested.runs[0].objectPath[0]=456;original.runs[0].bounds.x=789;
 const second=await readExistingTextRuns(adapter,"source",0);expect(second.runs).toHaveLength(2);expect(second.runs[0]).toMatchObject({text:"Original",bounds:{x:2}});expect(second.runs[1].objectPath).toEqual([1,2]);
});
it("reuses successful top-level discovery while retrying a tagged-form refusal",async()=>{
 const top=vi.fn().mockResolvedValue({runs:[run]});
 const forms=vi.fn().mockResolvedValueOnce({runs:[],reason:"Tagged source cannot use form editing"}).mockResolvedValueOnce({runs:[],reason:"Tagged source cannot use form editing"}).mockResolvedValue({runs:[form]});
 const adapter={...createDemoAdapter(),listTextRuns:top,listFormTextRuns:forms};
 expect((await readExistingTextRuns(adapter,"word",0)).reason).toContain("Tagged");
 expect((await readExistingTextRuns(adapter,"word",0)).reason).toContain("Tagged");
 expect(top).toHaveBeenCalledTimes(1);expect(forms).toHaveBeenCalledTimes(2);
 expect((await readExistingTextRuns(adapter,"word",0)).runs).toHaveLength(2);
 await readExistingTextRuns(adapter,"word",0);expect(top).toHaveBeenCalledTimes(1);expect(forms).toHaveBeenCalledTimes(3);
});
it.each(["top","forms"] as const)("snapshots %s when it settles while the sibling inspection is pending",async first=>{
 const topData={runs:[{...run,bounds:{...run.bounds}}]};
 const formData={runs:[{...form,bounds:{...form.bounds},objectPath:[1,2]}]};
 const topPending=deferred(),formsPending=deferred();
 const top=vi.fn().mockReturnValue(topPending.promise),forms=vi.fn().mockReturnValue(formsPending.promise);
 const adapter={...createDemoAdapter(),listTextRuns:top,listFormTextRuns:forms};
 const pending=readExistingTextRuns(adapter,"source",0);
 await Promise.resolve();
 if(first==="top")topPending.resolve(topData);else formsPending.resolve(formData);
 // Drain the completed provider's fulfillment chain without resolving its sibling.
 await new Promise(resolve=>setTimeout(resolve,0));
 if(first==="top"){
  topData.runs[0].text="Changed upstream";topData.runs[0].bounds.x=789;formsPending.resolve(formData);
 }else{
  formData.runs[0].text="Changed upstream";formData.runs[0].bounds.x=789;formData.runs[0].objectPath[0]=999;topPending.resolve(topData);
 }
 const initial=await pending,cached=await readExistingTextRuns(adapter,"source",0);
 for(const result of [initial,cached]){
  expect(result.runs[0]).toMatchObject({text:"Original",bounds:{x:2}});
  expect(result.runs[1]).toMatchObject({text:"Nested",bounds:{x:2},objectIndex:1,objectPath:[1,2]});
 }
 expect(top).toHaveBeenCalledTimes(1);expect(forms).toHaveBeenCalledTimes(1);
});
it("does not reinstall a late inspection after source release",async()=>{
 const pending=deferred(),list=vi.fn().mockReturnValueOnce(pending.promise).mockResolvedValue({runs:[{...run,text:"New source"}]});const adapter={...createDemoAdapter(),listTextRuns:list};
 const old=readExistingTextRuns(adapter,"source",0);await Promise.resolve();clearExistingTextCache(["source"]);
 expect((await readExistingTextRuns(adapter,"source",0)).runs[0].text).toBe("New source");pending.resolve({runs:[run]});await old;
 expect((await readExistingTextRuns(adapter,"source",0)).runs[0].text).toBe("New source");expect(list).toHaveBeenCalledTimes(2);
});
it("retains at most32 entries including pending inspections and does not resurrect evictions",async()=>{
 const pending:ReturnType<typeof deferred>[]=[];const list=vi.fn().mockImplementation(()=>{const request=deferred();pending.push(request);return request.promise;});const adapter={...createDemoAdapter(),listTextRuns:list};
 const requests=Array.from({length:33},(_,i)=>readExistingTextRuns(adapter,`source-${i}`,0));await Promise.resolve();expect(list).toHaveBeenCalledTimes(33);
 pending[0].resolve({runs:[run]});await requests[0];const again=readExistingTextRuns(adapter,"source-0",0);await Promise.resolve();expect(list).toHaveBeenCalledTimes(34);
 pending.forEach(p=>p.resolve({runs:[run]}));await Promise.all([...requests,again]);
});
it("evicts least recently used completed entries",async()=>{
 const list=vi.fn().mockResolvedValue({runs:[run]}),adapter={...createDemoAdapter(),listTextRuns:list};
 for(let i=0;i<32;i++)await readExistingTextRuns(adapter,`source-${i}`,0);
 await readExistingTextRuns(adapter,"source-0",0);await readExistingTextRuns(adapter,"source-32",0);await readExistingTextRuns(adapter,"source-0",0);expect(list).toHaveBeenCalledTimes(33);
 await readExistingTextRuns(adapter,"source-1",0);expect(list).toHaveBeenCalledTimes(34);
});
it("bounds aggregate retained payload and serves oversized results without retaining them",async()=>{
 const list=vi.fn().mockResolvedValue({runs:[{...run,text:"x".repeat(70_000)}]}),adapter={...createDemoAdapter(),listTextRuns:list};
 for(let i=0;i<20;i++)await readExistingTextRuns(adapter,`source-${i}`,0);
 await readExistingTextRuns(adapter,"source-19",0);expect(list).toHaveBeenCalledTimes(20);await readExistingTextRuns(adapter,"source-0",0);expect(list).toHaveBeenCalledTimes(21);
 list.mockResolvedValue({runs:[{...run,text:"x".repeat(1_100_000)}]});
 expect((await readExistingTextRuns(adapter,"oversized",0)).runs[0].text).toHaveLength(1_100_000);await readExistingTextRuns(adapter,"oversized",0);expect(list).toHaveBeenCalledTimes(23);
 await readExistingTextRuns(adapter,"source-19",0);expect(list).toHaveBeenCalledTimes(23);
});
it("invalidates native sources on close even when native cleanup rejects",async()=>{
 const ipc=vi.mocked(invoke);ipc.mockImplementation(async command=>{if(command==="close_document")throw Error("close failed");return {runs:command==="list_occurrence_text_runs"?[run]:[]};});
 await readExistingTextRuns(nativeAdapter,"source",0);await expect(nativeAdapter.closeDocument("source")).rejects.toThrow("close failed");await readExistingTextRuns(nativeAdapter,"source",0);
 expect(ipc.mock.calls.filter(([command])=>command==="list_occurrence_text_runs")).toHaveLength(2);
});
it("discards requests begun during native close and their late completions",async()=>{
 let finishClose!:()=>void;const closing=new Promise<void>(resolve=>finishClose=resolve),late=deferred();let listings=0;
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==="close_document")return closing;
  if(command==="list_occurrence_text_runs")return ++listings===2?late.promise:{runs:[{...run,text:`Generation ${listings}`}]};
  return {runs:[]};
 });
 await readExistingTextRuns(nativeAdapter,"source",0);const close=nativeAdapter.closeDocument("source"),during=readExistingTextRuns(nativeAdapter,"source",0);
 await Promise.resolve();expect(listings).toBe(2);finishClose();await close;
 expect((await readExistingTextRuns(nativeAdapter,"source",0)).runs[0].text).toBe("Generation 3");late.resolve({runs:[run]});await during;
 expect((await readExistingTextRuns(nativeAdapter,"source",0)).runs[0].text).toBe("Generation 3");expect(listings).toBe(3);
});
