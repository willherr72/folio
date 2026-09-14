import {expect,it,vi} from "vitest";
import {nativeAdapter} from "../src/editor/adapter";
const ipc=vi.hoisted(()=>vi.fn());vi.mock("@tauri-apps/api/core",()=>({invoke:ipc}));
it("sends explicit group membership and verified original to separate inspect and replace commands",async()=>{
 ipc.mockClear();ipc.mockResolvedValue({objectIndices:[2,3],text:"Hello world",fontName:"Helvetica",fontSize:12});
 await nativeAdapter.inspectTextGroup!("source",1,[2,3]);
 expect(ipc).toHaveBeenLastCalledWith("inspect_text_group",{sourceId:"source",pageIndex:1,objectIndices:[2,3]});
 await nativeAdapter.replaceTextGroup!("source",1,[2,3],"Hello world","Hi world");
 expect(ipc).toHaveBeenLastCalledWith("replace_text_group",{sourceId:"source",pageIndex:1,objectIndices:[2,3],expectedText:"Hello world",replacement:"Hi world"});
 ipc.mockClear();
});
it("sends immutable source/run identity and expected original text to native editing commands",async()=>{
 ipc.mockResolvedValueOnce({runs:[]}).mockResolvedValueOnce({id:"derived",name:"Page.pdf",pages:[{width:300,height:400}]});
 await nativeAdapter.listTextRuns!("source",2);
 expect(ipc).toHaveBeenNthCalledWith(1,"list_text_runs",{sourceId:"source",pageIndex:2});
 const result=await nativeAdapter.replaceText!("source",2,7,"Before","After");
 expect(ipc).toHaveBeenNthCalledWith(2,"replace_text",{sourceId:"source",pageIndex:2,objectIndex:7,expectedText:"Before",replacement:"After"});expect(result.id).toBe("derived");
});

it("sends a substitute only when the user explicitly chooses its font resource",async()=>{
 const fontId="a".repeat(64);ipc.mockClear();ipc.mockResolvedValue({id:"derived",name:"Page.pdf",pages:[{width:300,height:400}]});
 await nativeAdapter.replaceText!("source",2,7,"Before","café",fontId);
 expect(ipc).toHaveBeenCalledWith("replace_text",{sourceId:"source",pageIndex:2,objectIndex:7,expectedText:"Before",replacement:"café",fontId});
});
