import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import {clearSearchTextCache} from "../src/editor/search";
const size={width:300,height:400};
const run={objectIndex:0,text:"Old content",fontName:"Helvetica",fontSize:18,bounds:{x:40,y:145,width:100,height:20},supported:true};
let selectedRun:typeof run & {sourceText?:string}=run;
vi.mock("../src/editor/useWorkspaceRecovery",()=>({useWorkspaceRecovery:()=>({ready:true,checkpoint:vi.fn().mockResolvedValue(undefined)})}));
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:(props:any)=><div><output data-testid="plans">{JSON.stringify(props.pages)}</output><button onClick={()=>props.onEditText(props.selectedPageId,selectedRun)}>Choose text</button></div>}));
const candidate=()=>({document:{id:"edited",name:"Edited.pdf",pages:[size,size,size]},pageIndices:[0,2,1]});
beforeEach(()=>{
 selectedRun=run;
 localStorage.clear();Object.defineProperty(window,"__TAURI_INTERNALS__",{value:{invoke:vi.fn().mockResolvedValue(0),transformCallback:()=>0},configurable:true});
 vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue({id:"source",name:"Shared.pdf",pages:[{...size,overlays:[{id:"note",type:"comment",x:20,y:25,text:"Keep note",color:"#ffff00"}]},size]});
 vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,AA");
 vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"startPdfSave").mockReturnValue({result:Promise.resolve("Edited.pdf"),cancel:vi.fn().mockResolvedValue(false)});
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"replaceTextOccurrence").mockResolvedValue(candidate());
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"replaceText").mockRejectedValue(new Error("legacy isolation must not run"));
});
afterEach(()=>{cleanup();clearSearchTextCache(["source","edited","borrowed"]);vi.restoreAllMocks();delete (window as any).__TAURI_INTERNALS__;});
const plans=()=>JSON.parse(screen.getByTestId("plans").textContent!);
async function open(){render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Shared.pdf"});fireEvent.click(screen.getByRole("button",{name:"Rotate"}));fireEvent.click(screen.getByRole("button",{name:"Duplicate"}));}
async function edit(){fireEvent.click(screen.getByRole("button",{name:"Choose text"}));fireEvent.change(await screen.findByRole("textbox",{name:"Replacement text"}),{target:{value:"New content"}});fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));}
it("edits exact source text while retaining native text as the selection identity",async()=>{
 selectedRun={...run,text:"Old content ",sourceText:"Old content"};
 await open();fireEvent.click(screen.getByRole("button",{name:"Choose text"}));
 const draft=await screen.findByRole("textbox",{name:"Replacement text"});
 expect(draft).toHaveValue("Old content");expect(screen.getByRole("button",{name:"Apply changes"})).toBeDisabled();
 fireEvent.change(draft,{target:{value:"New content"}});fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));
 await waitFor(()=>expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
 expect(nativeAdapter.replaceTextOccurrence).toHaveBeenCalledWith("source",[0,0,1],1,0,"Old content ","New content");
 expect(nativeAdapter.replaceText).not.toHaveBeenCalled();
});
it("remaps all linked pages in one undo step while preserving workspace state and saved sources",async()=>{
 await open();const before=plans();await edit();await waitFor(()=>expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
 expect(nativeAdapter.replaceTextOccurrence).toHaveBeenCalledWith("source",[0,0,1],1,0,"Old content","New content");expect(nativeAdapter.replaceText).not.toHaveBeenCalled();
 const after=plans();expect(after.map((p:any)=>[p.sourceId,p.pageIndex])).toEqual([["edited",0],["edited",2],["edited",1]]);
 expect(after.map((p:any)=>[p.id,p.rotation,p.overlays])).toEqual(before.map((p:any)=>[p.id,p.rotation,p.overlays]));
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(plans()).toEqual(before);
 fireEvent.click(screen.getByRole("button",{name:"Redo"}));expect(plans()).toEqual(after);
 fireEvent.click(screen.getByRole("button",{name:"Save a copy"}));await waitFor(()=>expect(nativeAdapter.startPdfSave).toHaveBeenCalled());expect(vi.mocked(nativeAdapter.startPdfSave!).mock.calls[0][0]).toEqual(after);
 expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith("source");expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith("edited");
});
it.each(["source","borrowed"])("rejects borrowed result %s without adopting or closing it",async id=>{
 await open();
 if(id==="borrowed"){
  vi.mocked(nativeAdapter.openPdf).mockResolvedValue({id:"borrowed",name:"Other.pdf",pages:[size]});fireEvent.click(screen.getByRole("button",{name:"Open PDF in new tab"}));await screen.findByRole("tab",{name:"Other.pdf"});fireEvent.click(screen.getByRole("tab",{name:"Shared.pdf"}));
 }
 const before=plans();vi.mocked(nativeAdapter.replaceTextOccurrence!).mockResolvedValue({...candidate(),document:{...candidate().document,id}});
 await edit();await screen.findByRole("alert");expect(plans()).toEqual(before);expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith(id);expect(screen.getByRole("textbox",{name:"Replacement text"})).toHaveValue("New content");
});
it("disposes an invalid mapping and keeps the draft and original document",async()=>{
 await open();const before=plans();vi.mocked(nativeAdapter.replaceTextOccurrence!).mockResolvedValue({...candidate(),pageIndices:[2,0,1]});
 await edit();await screen.findByRole("alert");expect(plans()).toEqual(before);await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("edited"));
});
it("rejects a result owned only by undo history",async()=>{
 await open();await edit();await waitFor(()=>expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));const before=plans();
 await edit();await screen.findByRole("alert");expect(plans()).toEqual(before);expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith("edited");
});
it("keeps the draft after native refusal without retrying through isolated editing",async()=>{
 await open();const before=plans();vi.mocked(nativeAdapter.replaceTextOccurrence!).mockRejectedValueOnce(new Error("The replacement would overlap neighboring text."));
 await edit();await screen.findByText("The replacement would overlap neighboring text.");expect(plans()).toEqual(before);
 expect(screen.getByRole("textbox",{name:"Replacement text"})).toHaveValue("New content");expect(nativeAdapter.replaceText).not.toHaveBeenCalled();
});
it("releases a fresh result after unmount while keeping Apply controls blocked",async()=>{
 let resolve!:(value:ReturnType<typeof candidate>)=>void;vi.mocked(nativeAdapter.replaceTextOccurrence!).mockImplementation(()=>new Promise(done=>{resolve=done;}));
 await open();await edit();expect(screen.getByRole("button",{name:"Cancel"})).toBeDisabled();expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();
 cleanup();await act(async()=>resolve(candidate()));await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("edited"));
});
