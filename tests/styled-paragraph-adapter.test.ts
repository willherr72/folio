import {beforeEach,expect,it,vi} from "vitest";
import {nativeAdapter} from "../src/editor/adapter";
const ipc=vi.hoisted(()=>vi.fn());vi.mock("@tauri-apps/api/core",()=>({invoke:ipc}));
beforeEach(()=>{ipc.mockReset();ipc.mockResolvedValue({id:"derived"});});
it("forwards exact checked selection and anchor identity to mixed inspection",async()=>{
 await nativeAdapter.inspectStyledParagraph!("source",3,[2,4,5]);
 expect(ipc).toHaveBeenLastCalledWith("inspect_styled_paragraph",{sourceId:"source",pageIndex:3,objectIndices:[2,4,5]});
 await nativeAdapter.suggestStyledParagraph!("source",3,4,"Caf\u00e8 ");
 expect(ipc).toHaveBeenLastCalledWith("suggest_styled_paragraph",{sourceId:"source",pageIndex:3,objectIndex:4,expectedText:"Caf\u00e8 "});
});
it("carries attributed runs and explicit changes without inventing formatting defaults",async()=>{
 const request={objectIndices:[2,3],expectedText:"Before",runs:[{text:"a\n",styleId:0},{text:"b",styleId:2}],bounds:{x:10,y:20,width:100,height:60},alignment:"justify" as const,lineSpacing:1.4};
 expect(await nativeAdapter.previewStyledParagraph!("source",2,request)).toEqual({id:"derived"});
 expect(ipc).toHaveBeenLastCalledWith("preview_styled_paragraph",{sourceId:"source",pageIndex:2,request});
 const explicit={...request,styleChanges:[{styleId:2,sourceStyleId:1,fontSize:18,color:"#881122",fontId:"a".repeat(64)}]};
 await nativeAdapter.previewStyledParagraph!("source",2,explicit);
 expect(ipc).toHaveBeenLastCalledWith("preview_styled_paragraph",{sourceId:"source",pageIndex:2,request:explicit});
 await nativeAdapter.fitStyledParagraph!("source",2,explicit);
 expect(ipc).toHaveBeenLastCalledWith("fit_styled_paragraph",{sourceId:"source",pageIndex:2,request:explicit});
});
