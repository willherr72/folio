import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import {clearSearchTextCache} from "../src/editor/search";
vi.mock("../src/editor/useWorkspaceRecovery",()=>({useWorkspaceRecovery:()=>({ready:true,checkpoint:vi.fn().mockResolvedValue(undefined)})}));
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:(p:any)=><output data-testid="page-sources">{p.pages.map((page:any)=>page.sourceId).join(",")}</output>}));
beforeEach(()=>{
 localStorage.clear();
 Object.defineProperty(window,"__TAURI_INTERNALS__",{value:{invoke:vi.fn().mockResolvedValue(0),transformCallback:()=>0},configurable:true});
 vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue({id:"original",name:"Source.pdf",pages:[{width:612,height:792},{width:612,height:792}]});
 vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,");
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"createBlankDocument").mockImplementation(async(width,height)=>({id:"blank",name:"Untitled.pdf",pages:[{width,height}]}));
 vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"startPdfSave").mockReturnValue({result:Promise.resolve("New.pdf"),cancel:vi.fn().mockResolvedValue(false)});
});
afterEach(()=>{cleanup();clearSearchTextCache(["original","blank"]);vi.restoreAllMocks();delete (window as any).__TAURI_INTERNALS__;});
it("creates an unsaved landscape A4 PDF, then saves it through the ordinary export",async()=>{
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"New PDF"}));
 fireEvent.change(screen.getByRole("combobox",{name:"Paper size"}),{target:{value:"a4"}});
 fireEvent.change(screen.getByRole("combobox",{name:"Orientation"}),{target:{value:"landscape"}});
 fireEvent.click(screen.getByRole("button",{name:"Create PDF"}));await screen.findByRole("tab",{name:"Untitled.pdf"});
 expect(nativeAdapter.createBlankDocument).toHaveBeenCalledWith(841.89,595.28);
 expect(screen.getByLabelText("Unsaved changes")).toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Save a copy"}));await waitFor(()=>expect(nativeAdapter.startPdfSave).toHaveBeenCalled());
 expect(vi.mocked(nativeAdapter.startPdfSave!).mock.calls[0][0][0]).toMatchObject({sourceId:"blank",width:841.89,height:595.28});
 await waitFor(()=>expect(screen.queryByLabelText("Unsaved changes")).not.toBeInTheDocument());
});
it.each([['before','blank,original,original'],['after','original,blank,original'],['end','original,original,blank']])("inserts %s as one undo step and retains the source for redo",async(position,expected)=>{
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Source.pdf"});
 fireEvent.click(screen.getByRole("button",{name:"Insert blank page…"}));
 fireEvent.change(screen.getByRole("combobox",{name:"Insert"}),{target:{value:position}});
 fireEvent.click(screen.getByRole("button",{name:"Insert page"}));await waitFor(()=>expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
 expect(screen.getByTestId("page-sources")).toHaveTextContent(expected);
 expect(screen.getByLabelText("Unsaved changes")).toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(screen.getByTestId("page-sources")).toHaveTextContent("original,original");
 expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith("blank");
 fireEvent.click(screen.getByRole("button",{name:"Redo"}));expect(screen.getByTestId("page-sources")).toHaveTextContent(expected);
 fireEvent.click(screen.getByRole("button",{name:"Close Source.pdf"}));fireEvent.click(screen.getByRole("button",{name:"Discard and close"}));
 await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("blank"));
});
it("keeps settings after failure and closes a late source after unmount",async()=>{
 vi.mocked(nativeAdapter.createBlankDocument!).mockRejectedValueOnce(new Error("Creation failed"));
 const view=render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"New PDF"}));
 fireEvent.change(screen.getByRole("combobox",{name:"Paper size"}),{target:{value:"legal"}});
 fireEvent.click(screen.getByRole("button",{name:"Create PDF"}));expect(await screen.findByRole("alert")).toHaveTextContent("Creation failed");
 expect(screen.getByRole("combobox",{name:"Paper size"})).toHaveValue("legal");
 let done!:(value:any)=>void;vi.mocked(nativeAdapter.createBlankDocument!).mockImplementation(()=>new Promise(resolve=>done=resolve));
 fireEvent.click(screen.getByRole("button",{name:"Create PDF"}));
 expect(screen.getByRole("button",{name:"Cancel"})).toBeDisabled();fireEvent.keyDown(screen.getByRole("dialog"),{key:"Escape"});expect(screen.getByRole("dialog")).toBeInTheDocument();
 view.unmount();await act(async()=>done({id:"late",name:"Untitled.pdf",pages:[{width:612,height:1008}]}));
 expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("late");
});

it("keeps the inserted page selected when Find already has a match",async()=>{
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"getPageText").mockImplementation(async source=>({characters:source==="original"?Array.from("Needle",(text,i)=>({text,x:i*6,y:30,width:6,height:12})):[]}));
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Source.pdf"});
 fireEvent.click(screen.getByRole("button",{name:"Find"}));fireEvent.change(screen.getByRole("searchbox"),{target:{value:"Needle"}});await screen.findByText("1 of 2");
 fireEvent.click(screen.getByRole("button",{name:"Insert blank page…"}));fireEvent.click(screen.getByRole("button",{name:"Insert page"}));
 await waitFor(()=>expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
 await act(async()=>{await new Promise(done=>setTimeout(done,250));});
 expect(screen.getByText("Page 2 of 3")).toBeInTheDocument();
});

it("explains desktop availability without invoking native creation in a browser",async()=>{
 delete (window as any).__TAURI_INTERNALS__;
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"New PDF"}));
 expect(await screen.findByRole("alert")).toHaveTextContent("Windows desktop app");
 expect(nativeAdapter.createBlankDocument).not.toHaveBeenCalled();expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
});
