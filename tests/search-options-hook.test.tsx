import {act,cleanup,renderHook,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {createDemoAdapter} from "../src/editor/adapter";
import {clearSearchTextCache,useDocumentSearch} from "../src/editor/search";
import type {PagePlan,PageText,PageSearchOptions,PageSearchResult} from "../src/editor/types";

const page:PagePlan={id:"options-page",sourceId:"options-hook",pageIndex:0,width:300,height:400,rotation:0,overlays:[]};
const text=(value:string):PageText=>({characters:Array.from(value,(text,index)=>({text,x:index*6,y:20,width:6,height:6}))});
const options={matchCase:false,wholeWords:false};
const found:PageSearchResult={matches:[[{x:10,y:20,width:12,height:6}]],hasText:true,truncated:false};
afterEach(()=>{cleanup();clearSearchTextCache([page.sourceId]);vi.restoreAllMocks();});

it("clears matches immediately on option change and filters source and overlay text using cached extraction",async()=>{
 const api={...createDemoAdapter(),getPageText:vi.fn().mockResolvedValue(text("foo Foo foobar"))};
 const pages=[{...page,overlays:[{id:"note",type:"text"as const,text:"Foo foo",x:10,y:100,fontSize:12,color:"#000000"}]}];
 const hook=renderHook(({flags})=>useDocumentSearch(api,pages,"foo",true,flags),{initialProps:{flags:options}});
 await waitFor(()=>expect(hook.result.current.matches).toHaveLength(5));
 hook.rerender({flags:{matchCase:true,wholeWords:true}});
 expect(hook.result.current.matches).toEqual([]);expect(hook.result.current.searching).toBe(true);
 await waitFor(()=>expect(hook.result.current.searching).toBe(false));
 expect(hook.result.current.matches).toHaveLength(2);
 expect(hook.result.current.matches.map(m=>m.rects[0].y)).toEqual([20,100]);
 expect(api.getPageText).toHaveBeenCalledTimes(1);
});

it("sends raw bounded protected queries and options, rejecting late results from old options",async()=>{
 let finishOld!:(value:PageSearchResult)=>void;
 const requests:{query:string;flags:PageSearchOptions|undefined}[]=[];
 const api={...createDemoAdapter(),getPageText:vi.fn(),searchPage:vi.fn((_id:string,_index:number,query:string,flags?:PageSearchOptions)=>{
  requests.push({query,flags});return requests.length===1?new Promise<PageSearchResult>(resolve=>{finishOld=resolve;}):Promise.resolve(found);
 })};
 const pages=[{...page,protection:{documentId:"origin",revision:1,owner:false,permissions:{copyContent:false,editContent:false,annotate:true,fillForms:true,assemble:false,print:true,printHighQuality:false}}}];
 const query="İ".repeat(512);
 const hook=renderHook(({flags})=>useDocumentSearch(api,pages,query,true,flags),{initialProps:{flags:options}});
 await waitFor(()=>expect(finishOld).toBeTypeOf("function"));
 hook.rerender({flags:{matchCase:true,wholeWords:true}});
 await waitFor(()=>expect(hook.result.current.searching).toBe(false));expect(hook.result.current.matches).toHaveLength(1);
 await act(async()=>finishOld({matches:[],hasText:false,truncated:false}));
 expect(hook.result.current.matches).toHaveLength(1);expect(hook.result.current.hasText).toBe(true);
 expect(requests).toEqual([{query,flags:options},{query,flags:{matchCase:true,wholeWords:true}}]);
 expect(api.getPageText).not.toHaveBeenCalled();
});

it("invalid queries clear old results without dispatching native work or throwing during render",async()=>{
 const api={...createDemoAdapter(),getPageText:vi.fn().mockResolvedValue(text("foo"))};
 const pages=[page];const hook=renderHook(({query})=>useDocumentSearch(api,pages,query,true,options),{initialProps:{query:"foo"}});
 await waitFor(()=>expect(hook.result.current.matches).toHaveLength(1));
 hook.rerender({query:"é".repeat(513)});
 expect(hook.result.current).toMatchObject({matches:[],searching:false});expect(hook.result.current.error).toMatch(/too long/i);
 hook.rerender({query:"\ud800"});expect(hook.result.current.error).toMatch(/Unicode/i);
 expect(api.getPageText).toHaveBeenCalledTimes(1);
});
