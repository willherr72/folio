import {expect,it,vi} from "vitest";
import {nativeAdapter} from "../src/editor/adapter";
const ipc=vi.hoisted(()=>vi.fn());vi.mock("@tauri-apps/api/core",()=>({invoke:ipc}));
it("omits an unchanged form font and forwards an explicit font with the full occurrence path",async()=>{
 ipc.mockClear();ipc.mockResolvedValue({id:"derived"});
 await nativeAdapter.replaceFormText!("source",2,[1,0,0],"Café old","Café edit");
 expect(ipc).toHaveBeenLastCalledWith("replace_form_text",{sourceId:"source",pageIndex:2,objectPath:[1,0,0],expectedText:"Café old",replacement:"Café edit"});
 await nativeAdapter.replaceFormText!("source",2,[1,0,0],"Café old","Cafè edit","font");
 expect(ipc).toHaveBeenLastCalledWith("replace_form_text",{sourceId:"source",pageIndex:2,objectPath:[1,0,0],expectedText:"Café old",replacement:"Cafè edit",fontId:"font"});ipc.mockClear();
});
it("sends explicit paragraph color alongside other formatting",async()=>{
 ipc.mockClear();ipc.mockResolvedValue({id:"preview"});const bounds={x:0,y:0,width:100,height:50};
 await nativeAdapter.previewTextParagraph!("source",0,[0],"Old","New",bounds,16,"right",1.5,"font","#1A80EF");
 expect(ipc).toHaveBeenLastCalledWith("preview_text_paragraph",{sourceId:"source",pageIndex:0,objectIndices:[0],expectedText:"Old",replacement:"New",bounds,fontSize:16,alignment:"right",lineSpacing:1.5,fontId:"font",color:"#1A80EF"});ipc.mockClear();
});
it("omits paragraph substitution by default and sends the explicit chosen font",async()=>{
 const bounds={x:0,y:0,width:100,height:50};ipc.mockResolvedValue({id:"preview"});
 await nativeAdapter.previewTextParagraph!("source",0,[0],"Old","New",bounds);
 expect(ipc).toHaveBeenLastCalledWith("preview_text_paragraph",{sourceId:"source",pageIndex:0,objectIndices:[0],expectedText:"Old",replacement:"New",bounds});
 await nativeAdapter.previewTextParagraph!("source",0,[0],"Old","New",bounds,16,"right",1.5,"font");
 expect(ipc).toHaveBeenLastCalledWith("preview_text_paragraph",{sourceId:"source",pageIndex:0,objectIndices:[0],expectedText:"Old",replacement:"New",bounds,fontSize:16,alignment:"right",lineSpacing:1.5,fontId:"font"});ipc.mockClear();
});
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
