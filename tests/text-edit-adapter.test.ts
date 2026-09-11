import {expect,it,vi} from "vitest";
import {nativeAdapter} from "../src/editor/adapter";
const ipc=vi.hoisted(()=>vi.fn());vi.mock("@tauri-apps/api/core",()=>({invoke:ipc}));
it("sends immutable source/run identity and expected original text to native editing commands",async()=>{
 ipc.mockResolvedValueOnce({runs:[]}).mockResolvedValueOnce({id:"derived",name:"Page.pdf",pages:[{width:300,height:400}]});
 await nativeAdapter.listTextRuns!("source",2);
 expect(ipc).toHaveBeenNthCalledWith(1,"list_text_runs",{sourceId:"source",pageIndex:2});
 const result=await nativeAdapter.replaceText!("source",2,7,"Before","After");
 expect(ipc).toHaveBeenNthCalledWith(2,"replace_text",{sourceId:"source",pageIndex:2,objectIndex:7,expectedText:"Before",replacement:"After"});expect(result.id).toBe("derived");
});
