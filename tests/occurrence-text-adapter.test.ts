import {expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {nativeAdapter} from "../src/editor/adapter";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
it("sends source occurrences and expected text to the complete-source native transaction",async()=>{
 const ipc=vi.mocked(invoke);ipc.mockResolvedValue({runs:[]});
 await nativeAdapter.listOccurrenceTextRuns!("source",2);
 expect(ipc).toHaveBeenLastCalledWith("list_occurrence_text_runs",{sourceId:"source",pageIndex:2});
 const value={document:{id:"result",name:"Edited.pdf",pages:[]},pageIndices:[0,2,1]};ipc.mockResolvedValue(value);
 expect(await nativeAdapter.replaceTextOccurrence!("source",[0,0,1],1,7,"Old","New","font")).toEqual(value);
 expect(ipc).toHaveBeenLastCalledWith("replace_text_occurrence",{sourceId:"source",occurrences:[0,0,1],selectedOccurrence:1,objectIndex:7,expectedText:"Old",replacement:"New",fontId:"font"});
});
