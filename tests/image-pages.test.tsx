import {act,cleanup,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import {clearSearchTextCache} from "../src/editor/search";
vi.mock("../src/editor/useWorkspaceRecovery",()=>({useWorkspaceRecovery:()=>({ready:true,checkpoint:vi.fn().mockResolvedValue(undefined)})}));
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:(p:any)=><output data-testid="page-sources">{p.pages.map((page:any)=>`${page.sourceId}:${page.pageIndex}`).join(",")}</output>}));
beforeEach(()=>{
 localStorage.clear();Object.defineProperty(window,"__TAURI_INTERNALS__",{value:{invoke:vi.fn().mockResolvedValue(0),transformCallback:()=>0},configurable:true});
 vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue({id:"original",name:"Source.pdf",pages:[{width:612,height:792},{width:612,height:792}]});
 vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,");
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"pickImages").mockResolvedValue(["C:\\photos\\red.png","C:\\photos\\blue.jpg"]);
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"createImageDocument").mockResolvedValue({id:"images",name:"Images.pdf",pages:[{width:612,height:792},{width:792,height:612}]});
 vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"startPdfSave").mockReturnValue({result:Promise.resolve("New.pdf"),cancel:vi.fn().mockResolvedValue(false)});
});
afterEach(()=>{cleanup();clearSearchTextCache(["original","images"]);vi.restoreAllMocks();delete (window as any).__TAURI_INTERNALS__;});
async function addImages(){fireEvent.click(screen.getByRole("button",{name:"Add images…"}));await screen.findByText("red.png");}
it("keeps keyboard focus inside the dialog after reordering and removing images",async()=>{
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Images to PDF"}));await addImages();
 const up=screen.getByRole("button",{name:"Move image 2 up"});up.focus();fireEvent.click(up);
 expect(screen.getByRole("dialog").contains(document.activeElement)).toBe(true);
 const remove=screen.getByRole("button",{name:"Remove image 1"});remove.focus();fireEvent.click(remove);
 expect(screen.getByRole("dialog").contains(document.activeElement)).toBe(true);
 const last=screen.getByRole("button",{name:"Remove image 1"});last.focus();fireEvent.click(last);
 expect(screen.getByRole("button",{name:"Add images…"})).toHaveFocus();
 fireEvent.keyDown(document.activeElement!,{key:"Escape"});expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
});
it("orders, removes and adds images, preserving settings and canceled picker selection",async()=>{
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Images to PDF"}));
 expect(screen.getByRole("button",{name:"Create PDF"})).toBeDisabled();await addImages();
 fireEvent.click(screen.getByRole("button",{name:"Move image 2 up"}));
 expect(within(screen.getByRole("list",{name:"Image page order"})).getAllByRole("listitem")[0]).toHaveTextContent("blue.jpg");
 fireEvent.click(screen.getByRole("button",{name:"Remove image 2"}));
 vi.mocked(nativeAdapter.pickImages!).mockResolvedValueOnce(null);fireEvent.click(screen.getByRole("button",{name:"Add images…"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Create PDF"})).toBeEnabled());
 expect(screen.getByText("blue.jpg")).toBeInTheDocument();
 fireEvent.change(screen.getByRole("combobox",{name:"Paper size"}),{target:{value:"a4"}});
 fireEvent.change(screen.getByRole("combobox",{name:"Orientation"}),{target:{value:"landscape"}});
 fireEvent.change(screen.getByRole("combobox",{name:"Margins"}),{target:{value:"0"}});
 fireEvent.click(screen.getByRole("button",{name:"Create PDF"}));await screen.findByRole("tab",{name:"Images.pdf"});
 expect(nativeAdapter.createImageDocument).toHaveBeenCalledWith(["C:\\photos\\blue.jpg"],{width:841.89,height:595.28,margin:0,autoOrientation:false});
 expect(screen.getByLabelText("Unsaved changes")).toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Save a copy"}));await waitFor(()=>expect(nativeAdapter.startPdfSave).toHaveBeenCalled());
 expect(vi.mocked(nativeAdapter.startPdfSave!).mock.calls[0][0].map(p=>p.sourceId)).toEqual(["images","images"]);
});
it.each([["before","images:0,images:1,original:0,original:1",1],["after","original:0,images:0,images:1,original:1",2],["end","original:0,original:1,images:0,images:1",3]])("inserts %s in one undo step and retains image source for redo",async(position,expected,page)=>{
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Source.pdf"});
 fireEvent.click(screen.getByRole("button",{name:"Insert image pages…"}));await addImages();
 fireEvent.change(screen.getByRole("combobox",{name:"Insert"}),{target:{value:position}});fireEvent.click(screen.getByRole("button",{name:"Insert pages"}));
 await waitFor(()=>expect(screen.queryByRole("dialog")).not.toBeInTheDocument());expect(screen.getByTestId("page-sources")).toHaveTextContent(String(expected));expect(screen.getByText(`Page ${page} of 4`)).toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(screen.getByTestId("page-sources")).toHaveTextContent("original:0,original:1");expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith("images");
 fireEvent.click(screen.getByRole("button",{name:"Redo"}));expect(screen.getByTestId("page-sources")).toHaveTextContent(String(expected));
 fireEvent.click(screen.getByRole("button",{name:"Close Source.pdf"}));fireEvent.click(screen.getByRole("button",{name:"Discard and close"}));await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("images"));
});
it("retains failed inputs, prevents duplicate/pending dismissal and closes late generated sources",async()=>{
 const view=render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Images to PDF"}));
 vi.mocked(nativeAdapter.pickImages!).mockRejectedValueOnce(new Error("Picker failed"));fireEvent.click(screen.getByRole("button",{name:"Add images…"}));expect(await screen.findByRole("alert")).toHaveTextContent("Picker failed");await addImages();
 vi.mocked(nativeAdapter.createImageDocument!).mockRejectedValueOnce(new Error("Broken image"));fireEvent.click(screen.getByRole("button",{name:"Create PDF"}));expect(await screen.findByRole("alert")).toHaveTextContent("Broken image");expect(screen.getByText("red.png")).toBeInTheDocument();
 let done!:(value:any)=>void;vi.mocked(nativeAdapter.createImageDocument!).mockImplementation(()=>new Promise(resolve=>done=resolve));
 fireEvent.click(screen.getByRole("button",{name:"Create PDF"}));fireEvent.click(screen.getByRole("button",{name:"Create PDF"}));expect(nativeAdapter.createImageDocument).toHaveBeenCalledTimes(2);
 expect(screen.getByRole("button",{name:"Cancel"})).toBeDisabled();fireEvent.keyDown(screen.getByRole("dialog"),{key:"Escape"});expect(screen.getByRole("dialog")).toBeInTheDocument();
 view.unmount();await act(async()=>done({id:"late",name:"Images.pdf",pages:[{width:612,height:792}]}));expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("late");
});
it("keeps a pending picker modal open and rejects more than 32 files",async()=>{
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Images to PDF"}));
 let done!:(value:any)=>void;vi.mocked(nativeAdapter.pickImages!).mockImplementation(()=>new Promise(resolve=>done=resolve));fireEvent.click(screen.getByRole("button",{name:"Add images…"}));
 expect(screen.getByRole("button",{name:"Cancel"})).toBeDisabled();fireEvent.keyDown(screen.getByRole("dialog"),{key:"Escape"});expect(screen.getByRole("dialog")).toBeInTheDocument();
 await act(async()=>done(Array(33).fill("C:\\red.png")));expect(within(screen.getByRole("dialog")).getByRole("alert")).toHaveTextContent("32");expect(screen.getByRole("button",{name:"Create PDF"})).toBeDisabled();
});
it("ignores a native picker result after the image workflow unmounts",async()=>{
 let finish!:(paths:string[])=>void;vi.mocked(nativeAdapter.pickImages!).mockImplementation(()=>new Promise(resolve=>finish=resolve));
 const view=render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Images to PDF"}));
 fireEvent.click(screen.getByRole("button",{name:"Add images…"}));expect(nativeAdapter.pickImages).toHaveBeenCalledOnce();
 view.unmount();await act(async()=>finish(["C:\\photos\\late.png"]));
 expect(nativeAdapter.createImageDocument).not.toHaveBeenCalled();expect(nativeAdapter.closeDocument).not.toHaveBeenCalled();
});
it("freezes the image insertion destination against late Find results and retains the query",async()=>{
 const pending:((value:any)=>void)[]=[];
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"getPageText").mockImplementation(source=>source==="images"?Promise.resolve({characters:[]}):new Promise(resolve=>pending.push(resolve)));
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Source.pdf"});
 fireEvent.click(screen.getByRole("button",{name:"Find"}));fireEvent.change(screen.getByRole("searchbox"),{target:{value:"Needle"}});
 await waitFor(()=>expect(pending).toHaveLength(1));fireEvent.click(screen.getByRole("button",{name:"Insert image pages…"}));
 await act(async()=>pending[0]({characters:[]}));await waitFor(()=>expect(pending).toHaveLength(2));
 await act(async()=>pending[1]({characters:Array.from("Needle",(text,index)=>({text,x:index*6,y:30,width:6,height:12}))}));
 await screen.findByText("1 of 1");expect(screen.getByText("Page 1 of 2")).toBeInTheDocument();
 await addImages();fireEvent.click(screen.getByRole("button",{name:"Insert pages"}));await waitFor(()=>expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
 expect(screen.getByTestId("page-sources")).toHaveTextContent("original:0,images:0,images:1,original:1");
 expect(screen.getByText("Page 2 of 4")).toBeInTheDocument();expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Find"}));expect(screen.getByRole("searchbox")).toHaveValue("Needle");
});
