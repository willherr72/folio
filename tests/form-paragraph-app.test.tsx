import {styledEditorFixture} from "./fixtures/styled-editor-data";
import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import {clearSearchTextCache} from "../src/editor/search";
const size={width:300,height:400};
const run={objectIndex:9,objectPath:[2,0,1],text:"one",fontName:"Helvetica",fontSize:18,bounds:{x:40,y:145,width:100,height:20},supported:true};
vi.mock("../src/editor/useWorkspaceRecovery",()=>({useWorkspaceRecovery:()=>({ready:true,checkpoint:vi.fn().mockResolvedValue(undefined)})}));
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:(props:any)=><div><output data-testid="plans">{JSON.stringify(props.pages)}</output><button onClick={()=>props.onEditText(props.selectedPageId,run)}>Choose text</button><button onClick={()=>{const page=props.pages.find((p:any)=>p.id!==props.selectedPageId);props.onSelectPage(page.id);props.onEditText(page.id,run);}}>Activate other page text</button></div>}));
const selection=styledEditorFixture();
const candidate=()=>({document:{id:"edited",name:"Edited.pdf",pages:[size,size,size]},pageIndices:[0,2,1]});
beforeEach(()=>{
 localStorage.clear();Object.defineProperty(window,"__TAURI_INTERNALS__",{value:{invoke:vi.fn().mockResolvedValue(0),transformCallback:()=>0},configurable:true});
 vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue({id:"source",name:"Shared.pdf",pages:[{...size,overlays:[{id:"note",type:"comment",x:20,y:25,text:"Keep note",color:"#ffff00"}]},size]});
 vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,AA");
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"renderPagePlan").mockResolvedValue("data:image/png;base64,AA");
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"listFormParagraphRuns").mockResolvedValue({anchorIndex:0,runs:[{...run,objectIndex:0,objectPath:undefined}]});
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"suggestFormStyledParagraph").mockResolvedValue(selection);
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"inspectFormStyledParagraph").mockResolvedValue(selection);
 vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"startPdfSave").mockReturnValue({result:Promise.resolve("Edited.pdf"),cancel:vi.fn().mockResolvedValue(false)});
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"previewFormStyledParagraphOccurrence").mockResolvedValue(candidate());
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"previewStyledParagraph").mockRejectedValue(new Error("legacy isolation must not run"));
});
afterEach(()=>{cleanup();clearSearchTextCache(["source","edited","borrowed"]);vi.restoreAllMocks();delete (window as any).__TAURI_INTERNALS__;});
const plans=()=>JSON.parse(screen.getByTestId("plans").textContent!);
async function open(){render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Shared.pdf"});fireEvent.click(screen.getByRole("button",{name:"Rotate"}));fireEvent.click(screen.getByRole("button",{name:"Duplicate"}));}
async function edit(){fireEvent.click(screen.getByRole("button",{name:"Choose text"}));fireEvent.click(await screen.findByRole("button",{name:"Edit paragraph…"}));await screen.findByRole("textbox",{name:"Paragraph text"});fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));}
async function apply(){await waitFor(()=>expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Apply paragraph"}));}

it("prepares the clicked form and remaps all linked pages in one undo step while preserving workspace state and saved sources",async()=>{
 await open();const before=plans();await edit();await apply();await waitFor(()=>expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
 expect(nativeAdapter.previewFormStyledParagraphOccurrence).toHaveBeenCalledWith("source",[0,0,1],1,[2,0,1],expect.objectContaining({objectIndices:selection.objectIndices}));expect(nativeAdapter.previewStyledParagraph).not.toHaveBeenCalled();
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
 const before=plans();vi.mocked(nativeAdapter.previewFormStyledParagraphOccurrence!).mockResolvedValue({...candidate(),document:{...candidate().document,id}});
 await edit();await screen.findByRole("alert");expect(plans()).toEqual(before);expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith(id);expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveTextContent("one two");
});
it("disposes an invalid mapping and keeps the draft and original document",async()=>{
 await open();const before=plans();vi.mocked(nativeAdapter.previewFormStyledParagraphOccurrence!).mockResolvedValue({...candidate(),pageIndices:[2,0,1]});
 await edit();await screen.findByRole("alert");expect(plans()).toEqual(before);await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("edited"));
});
it("rejects a result owned only by undo history",async()=>{
 await open();await edit();await apply();await waitFor(()=>expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));const before=plans();
 await edit();await screen.findByRole("alert");expect(plans()).toEqual(before);expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith("edited");
});
it("keeps the draft after native refusal without retrying through isolated editing",async()=>{
 await open();const before=plans();vi.mocked(nativeAdapter.previewFormStyledParagraphOccurrence!).mockRejectedValueOnce(new Error("The replacement would overlap neighboring text."));
 await edit();await screen.findByText("The replacement would overlap neighboring text.");expect(plans()).toEqual(before);
 expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveTextContent("one two");expect(nativeAdapter.previewStyledParagraph).not.toHaveBeenCalled();
});
it("releases a fresh result after unmount while keeping Apply controls blocked",async()=>{
 let resolve!:(value:ReturnType<typeof candidate>)=>void;vi.mocked(nativeAdapter.previewFormStyledParagraphOccurrence!).mockImplementation(()=>new Promise(done=>{resolve=done;}));
 await open();await edit();expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();expect(screen.getAllByRole("button",{name:"Undo"})[0]).toBeDisabled();
 cleanup();await act(async()=>resolve(candidate()));await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("edited"));
});

it("leaves the original text dialog and source intact when form preparation fails",async()=>{
 await open();const before=plans();vi.mocked(nativeAdapter.listFormParagraphRuns!).mockRejectedValue(new Error("Clipped form text cannot be edited as a paragraph."));
 fireEvent.click(screen.getByRole("button",{name:"Choose text"}));fireEvent.click(await screen.findByRole("button",{name:"Edit paragraph…"}));
 await screen.findByText("Clipped form text cannot be edited as a paragraph.");expect(plans()).toEqual(before);
 expect(screen.getByRole("button",{name:"Edit paragraph…"})).toBeEnabled();expect(screen.queryByRole("textbox",{name:"Paragraph text"})).not.toBeInTheDocument();
});
it("blocks workspace controls and ignores preparation after unmount",async()=>{
 let resolve!:(value:any)=>void;vi.mocked(nativeAdapter.listFormParagraphRuns!).mockImplementation(()=>new Promise(done=>{resolve=done;}));
 await open();fireEvent.click(screen.getByRole("button",{name:"Choose text"}));fireEvent.click(await screen.findByRole("button",{name:"Edit paragraph…"}));
 expect(screen.getAllByRole("button",{name:"Undo"})[0]).toBeDisabled();expect(screen.getByRole("button",{name:"Edit paragraph…"})).toBeDisabled();
 cleanup();await act(async()=>resolve({anchorIndex:0,runs:[{...run,objectIndex:0,objectPath:undefined}]}));
 expect(nativeAdapter.suggestFormStyledParagraph).not.toHaveBeenCalled();expect(nativeAdapter.previewFormStyledParagraphOccurrence).not.toHaveBeenCalled();
});
it("releases a cancelled form paragraph preview without changing the document",async()=>{
 await open();const before=plans();await edit();await waitFor(()=>expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled());
 fireEvent.click(screen.getByRole("button",{name:"Cancel"}));await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("edited"));expect(plans()).toEqual(before);
});


it("enters a nested paragraph directly with its bound form path and returns focus on cancel",async()=>{
 await open();const before=plans(),tool=screen.getByRole("button",{name:"Edit paragraphs"});fireEvent.click(tool);
 fireEvent.click(screen.getByRole("button",{name:"Choose text"}));await screen.findByRole("textbox",{name:"Paragraph text"});
 expect(screen.queryByRole("dialog",{name:"Edit existing text"})).toBeNull();
 expect(nativeAdapter.listFormParagraphRuns).toHaveBeenCalledWith("source",0,run.objectPath,run.text);
 fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled());
 expect(nativeAdapter.previewFormStyledParagraphOccurrence).toHaveBeenCalledWith("source",[0,0,1],1,run.objectPath,expect.anything());
 fireEvent.click(screen.getByRole("button",{name:"Cancel"}));await waitFor(()=>expect(tool).toHaveFocus());
 expect(plans()).toEqual(before);await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("edited"));
});
it("shows a direct nested preparation error without opening the single-text modal or changing pages",async()=>{
 await open();const before=plans();vi.mocked(nativeAdapter.listFormParagraphRuns!).mockRejectedValue(new Error("Source font could not be proved."));
 fireEvent.click(screen.getByRole("button",{name:"Edit paragraphs"}));fireEvent.click(screen.getByRole("button",{name:"Choose text"}));
 expect(await screen.findByRole("alert")).toHaveTextContent("Source font could not be proved.");
 expect(screen.queryByRole("dialog",{name:"Edit existing text"})).toBeNull();expect(screen.queryByRole("textbox",{name:"Paragraph text"})).toBeNull();
 expect(plans()).toEqual(before);expect(screen.getByRole("button",{name:"Edit text"})).toBeEnabled();
});


it("keeps direct nested preparation valid when the clicked page becomes selected",async()=>{
 await open();fireEvent.click(screen.getByRole("button",{name:"Edit paragraphs"}));
 fireEvent.click(screen.getByRole("button",{name:"Activate other page text"}));
 expect(await screen.findByRole("textbox",{name:"Paragraph text"})).toHaveTextContent("one two");
 expect(screen.queryByRole("dialog",{name:"Edit existing text"})).toBeNull();
});
