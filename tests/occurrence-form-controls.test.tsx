import {act,cleanup,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {createDemoAdapter,nativeAdapter} from "../src/editor/adapter";
import {ExistingTextLayer} from "../src/components/ExistingTextLayer";
import {supportsOccurrenceForm,supportsFormText} from "../src/editor/text-edit";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.clearAllMocks();});
const form={objectPath:[0,0,0],text:"Nested text",fontName:"Helvetica",fontSize:16,bounds:{x:20,y:50,width:80,height:20},supported:true,canSubstitute:true};
const page={id:"p",sourceId:"source",pageIndex:0,width:300,height:400,rotation:0 as const,overlays:[]};
it("sends source occurrences, nested path and optional font through the bridge",async()=>{
 const ipc=vi.mocked(invoke);ipc.mockResolvedValue({runs:[form]});
 expect(await nativeAdapter.listOccurrenceFormTextRuns!("source",2)).toEqual({runs:[form]});
 expect(ipc).toHaveBeenLastCalledWith("list_occurrence_form_text_runs",{sourceId:"source",pageIndex:2});
 const result={document:{id:"edited",name:"Edited.pdf",pages:[]},pageIndices:[0,2,1]};ipc.mockResolvedValue(result);
 for(const fontId of [undefined,"chosen-font"]){
  expect(await nativeAdapter.replaceFormTextOccurrence!("source",[0,0,1],1,[2,0,1],"Old","New",fontId)).toEqual(result);
  expect(ipc).toHaveBeenLastCalledWith("replace_form_text_occurrence",{sourceId:"source",occurrences:[0,0,1],selectedOccurrence:1,objectPath:[2,0,1],expectedText:"Old",replacement:"New",...(fontId?{fontId}:{})});
 }
});
it("requires a complete native pair and retains a complete legacy alternative",()=>{
 const base={...createDemoAdapter(),listFormTextRuns:vi.fn(),replaceFormText:vi.fn()};
 const native={...createDemoAdapter(),listOccurrenceFormTextRuns:vi.fn(),replaceFormTextOccurrence:vi.fn()};
 expect(supportsOccurrenceForm(base)).toBe(false);expect(supportsFormText(base)).toBe(true);expect(supportsFormText(native)).toBe(true);
 for(const key of ["listOccurrenceFormTextRuns","replaceFormTextOccurrence"]){const partial={...native,[key]:undefined};expect(supportsOccurrenceForm(partial)).toBe(false);expect(supportsFormText(partial)).toBe(false);expect(supportsFormText({...base,...partial})).toBe(true);}
});
it("lists native form paths and never retries legacy listing after a native refusal",async()=>{
 const adapter={...createDemoAdapter(),listOccurrenceFormTextRuns:vi.fn().mockResolvedValueOnce({runs:[form]}).mockRejectedValue(new Error("Native form refusal")),replaceFormTextOccurrence:vi.fn(),listFormTextRuns:vi.fn().mockResolvedValue({runs:[]}),replaceFormText:vi.fn()};
 const view=render(<svg><ExistingTextLayer adapter={adapter} page={page}/></svg>);
 expect(await screen.findByRole("button",{name:"Edit text: Nested text"})).toHaveAttribute("data-text-path","0.0.0");expect(adapter.listFormTextRuns).not.toHaveBeenCalled();
 view.rerender(<svg><ExistingTextLayer adapter={adapter} page={{...page,sourceId:"other"}}/></svg>);
 await screen.findByText("Native form refusal");expect(adapter.listFormTextRuns).not.toHaveBeenCalled();
});
it("drops delayed native listings when the page changes or the layer unmounts",async()=>{
 let finish!:(value:any)=>void;const adapter={...createDemoAdapter(),listOccurrenceFormTextRuns:vi.fn().mockImplementationOnce(()=>new Promise(done=>finish=done)).mockResolvedValue({runs:[{...form,text:"Current"}]}),replaceFormTextOccurrence:vi.fn()};
 const view=render(<svg><ExistingTextLayer adapter={adapter} page={page}/></svg>);
 await waitFor(()=>expect(adapter.listOccurrenceFormTextRuns).toHaveBeenCalled());
 view.rerender(<svg><ExistingTextLayer adapter={adapter} page={{...page,sourceId:"other"}}/></svg>);await screen.findByRole("button",{name:"Edit text: Current"});
 await act(async()=>finish({runs:[form]}));expect(screen.queryByRole("button",{name:"Edit text: Nested text"})).not.toBeInTheDocument();
 view.unmount();
});
it("uses legacy listing when the native pair is incomplete",async()=>{
 const adapter={...createDemoAdapter(),listOccurrenceFormTextRuns:vi.fn().mockRejectedValue(new Error("must not list")),listFormTextRuns:vi.fn().mockResolvedValue({runs:[form]}),replaceFormText:vi.fn()};
 render(<svg><ExistingTextLayer adapter={adapter} page={page}/></svg>);await screen.findByRole("button",{name:"Edit text: Nested text"});expect(adapter.listOccurrenceFormTextRuns).not.toHaveBeenCalled();expect(adapter.listFormTextRuns).toHaveBeenCalledWith("source",0);
});
