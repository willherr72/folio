import {expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {createDemoAdapter,nativeAdapter} from "../src/editor/adapter";
import {supportsOccurrenceGroup,supportsTextGroup} from "../src/editor/text-edit";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
it("sends selected piece indices and page occurrences through the native bridge",async()=>{
 const ipc=vi.mocked(invoke),preview={objectIndices:[1,2],text:"Old words",fontName:"Courier",fontSize:12};ipc.mockResolvedValue(preview);
 expect(await nativeAdapter.inspectOccurrenceTextGroup!("source",3,[1,2])).toEqual(preview);
 expect(ipc).toHaveBeenLastCalledWith("inspect_occurrence_text_group",{sourceId:"source",pageIndex:3,objectIndices:[1,2]});
 const result={document:{id:"edited",name:"Edited.pdf",pages:[]},pageIndices:[0,2,1]};ipc.mockResolvedValue(result);
 expect(await nativeAdapter.replaceTextGroupOccurrence!("source",[0,0,1],1,[1,2],"Old words","New text")).toEqual(result);
 expect(ipc).toHaveBeenLastCalledWith("replace_text_group_occurrence",{sourceId:"source",occurrences:[0,0,1],selectedOccurrence:1,objectIndices:[1,2],expectedText:"Old words",replacement:"New text"});
});
it("requires a complete native or legacy capability set",()=>{
 const base={...createDemoAdapter(),listTextRuns:vi.fn(),inspectTextGroup:vi.fn(),replaceTextGroup:vi.fn()};
 const native={...base,listTextRuns:undefined,inspectTextGroup:undefined,replaceTextGroup:undefined,listOccurrenceTextRuns:vi.fn(),inspectOccurrenceTextGroup:vi.fn(),replaceTextGroupOccurrence:vi.fn()};
 expect(supportsTextGroup(base)).toBe(true);expect(supportsOccurrenceGroup(base)).toBe(false);expect(supportsTextGroup(native)).toBe(true);
 for(const missing of ["listOccurrenceTextRuns","inspectOccurrenceTextGroup","replaceTextGroupOccurrence"]){const partial={...native,[missing]:undefined};expect(supportsOccurrenceGroup(partial)).toBe(false);expect(supportsTextGroup(partial)).toBe(false);expect(supportsTextGroup({...base,...partial,listTextRuns:base.listTextRuns,inspectTextGroup:base.inspectTextGroup,replaceTextGroup:base.replaceTextGroup})).toBe(true);}
});
