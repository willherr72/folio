import {expect,it,vi} from "vitest";
import {nativeAdapter} from "../src/editor/adapter";
const ipc=vi.hoisted(()=>vi.fn());vi.mock("@tauri-apps/api/core",()=>({invoke:ipc}));
it("forwards occurrence order, selection and incoming page without workspace transforms",async()=>{
 const result={document:{id:"candidate"},pageIndices:[0,1,2]};ipc.mockResolvedValue(result);
 expect(await nativeAdapter.replacePageContent!("destination",[0,1,0],2,"incoming",4)).toBe(result);
 expect(ipc).toHaveBeenCalledWith("replace_page_content_occurrence",{destinationId:"destination",occurrences:[0,1,0],selectedOccurrence:2,incomingId:"incoming",incomingPage:4});
});
