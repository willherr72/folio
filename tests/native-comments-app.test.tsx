import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import type {DocumentViewportProps} from "../src/components/DocumentViewport";
import type {DocumentInfo,NativeComment} from "../src/editor/types";
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:({pages,nativeNotes,onMoveNativeComment}:DocumentViewportProps)=><><div data-testid="sources">{pages.map(p=>p.sourceId).join(",")}</div><button onClick={()=>{const page=pages[1],comment=nativeNotes?.get(page.sourceId)?.[0];if(comment)void onMoveNativeComment?.(page,comment,{x:80,y:100}).catch(()=>{});}}>Move native note</button></>}));
const note:NativeComment={pageIndex:1,annotationIndex:2,text:"Original sticky note",author:"Reviewer",subject:"Check",bounds:{x:30,y:40,width:18,height:18},reason:null};
const info:DocumentInfo={id:"source",name:"Comments.pdf",pages:[{width:300,height:400},{width:300,height:400,overlays:[{id:"overlay",type:"comment",x:80,y:90,text:"Overlay note",color:"#ffcc00"}]}]};
beforeEach(()=>{
 localStorage.clear();vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue(info);
 vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,");vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"renderPagePlan").mockResolvedValue("data:image/png;base64,");
 vi.spyOn(nativeAdapter,"exportPdf").mockResolvedValue("saved.pdf");vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"listNativeComments").mockImplementation(async id=>[{...note,text:id==="edited"?"Changed note":note.text}]);
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"editNativeComment").mockResolvedValue({...info,id:"edited"});
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"deleteNativeComment").mockResolvedValue({...info,id:"deleted"});
});
afterEach(()=>{cleanup();vi.restoreAllMocks();});
async function open(){const view=render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("button",{name:"Page 2: Original sticky note"});return view;}
it("lists native notes alongside overlays and edits shared source pages in one undo step",async()=>{
 await open();expect(screen.getByRole("button",{name:"Page 2: Overlay note"})).toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Page 2: Original sticky note"}));expect(await screen.findByRole("dialog",{name:"PDF comment"})).toHaveTextContent("Reviewer");
 fireEvent.change(screen.getByRole("textbox",{name:"Comment text"}),{target:{value:"Changed note"}});fireEvent.click(screen.getByRole("button",{name:"Apply comment"}));
 await waitFor(()=>expect(screen.queryByRole("dialog")).not.toBeInTheDocument());expect(screen.getByTestId("sources")).toHaveTextContent("edited,edited");
 expect(nativeAdapter.editNativeComment).toHaveBeenCalledWith("source",{pageIndex:1,annotationIndex:2,expectedText:note.text,text:"Changed note"});
 await screen.findByRole("button",{name:"Page 2: Changed note"});fireEvent.click(screen.getByRole("button",{name:"Undo"}));await screen.findByRole("button",{name:"Page 2: Original sticky note"});
 expect(screen.getByTestId("sources")).toHaveTextContent("source,source");fireEvent.click(screen.getByRole("button",{name:"Redo"}));fireEvent.click(screen.getByRole("button",{name:"Save a copy"}));
 await waitFor(()=>expect(nativeAdapter.exportPdf).toHaveBeenCalled());const pages=vi.mocked(nativeAdapter.exportPdf).mock.calls[0][0];expect(pages.map(p=>p.sourceId)).toEqual(["edited","edited"]);expect(pages[1].overlays[0].id).toBe("overlay");
});
it("keeps unsupported notes readable and preserves a failed edit draft",async()=>{
 vi.mocked(nativeAdapter.listNativeComments!).mockResolvedValue([{...note,reason:"This comment is locked."}]);await open();fireEvent.click(screen.getByRole("button",{name:"Page 2: Original sticky note"}));
 expect(await screen.findByRole("textbox",{name:"Comment text"})).toHaveAttribute("readonly");expect(screen.getByText("This comment is locked.")).toBeInTheDocument();expect(screen.getByRole("button",{name:"Apply comment"})).toBeDisabled();
});
it("shows native failure without changing the source and releases a late candidate on unmount",async()=>{
 vi.mocked(nativeAdapter.editNativeComment!).mockRejectedValueOnce(new Error("Cannot preserve comment"));const view=await open();fireEvent.click(screen.getByRole("button",{name:"Page 2: Original sticky note"}));
 fireEvent.change(screen.getByRole("textbox",{name:"Comment text"}),{target:{value:"Changed note"}});fireEvent.click(screen.getByRole("button",{name:"Apply comment"}));
 expect(await screen.findByRole("alert")).toHaveTextContent("Cannot preserve comment");expect(screen.getByRole("textbox",{name:"Comment text"})).toHaveValue("Changed note");expect(screen.getByTestId("sources")).toHaveTextContent("source,source");
 let finish!:(result:DocumentInfo)=>void;vi.mocked(nativeAdapter.editNativeComment!).mockReturnValue(new Promise(resolve=>{finish=resolve;}));fireEvent.click(screen.getByRole("button",{name:"Apply comment"}));view.unmount();await act(async()=>finish({...info,id:"late"}));await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("late"));
});
it("shows loading failures instead of silently claiming there are no comments",async()=>{
 vi.mocked(nativeAdapter.listNativeComments!).mockRejectedValue(new Error("Malformed comment metadata"));render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));expect(await screen.findByText(/Malformed comment metadata/)).toBeInTheDocument();
});
it("deletes all shared-source placements in one undo step and preserves overlay notes",async()=>{
 vi.mocked(nativeAdapter.listNativeComments!).mockImplementation(async id=>id==="deleted"?[]:[note]);await open();fireEvent.click(screen.getByRole("button",{name:"Page 2: Original sticky note"}));fireEvent.click(screen.getByRole("button",{name:"Delete comment"}));
 await waitFor(()=>expect(screen.queryByRole("dialog")).not.toBeInTheDocument());expect(nativeAdapter.deleteNativeComment).toHaveBeenCalledWith("source",{pageIndex:1,annotationIndex:2,expectedText:note.text});expect(screen.getByTestId("sources")).toHaveTextContent("deleted,deleted");expect(screen.getByRole("button",{name:"Page 2: Overlay note"})).toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));await screen.findByRole("button",{name:"Page 2: Original sticky note"});expect(screen.getByTestId("sources")).toHaveTextContent("source,source");fireEvent.click(screen.getByRole("button",{name:"Redo"}));await waitFor(()=>expect(screen.queryByRole("button",{name:"Page 2: Original sticky note"})).not.toBeInTheDocument());
});
it("keeps the original source after deletion failure and releases a late deletion result",async()=>{
 vi.mocked(nativeAdapter.deleteNativeComment!).mockRejectedValueOnce(new Error("Popup is shared"));const view=await open();fireEvent.click(screen.getByRole("button",{name:"Page 2: Original sticky note"}));fireEvent.click(screen.getByRole("button",{name:"Delete comment"}));expect(await screen.findByRole("alert")).toHaveTextContent("Popup is shared");expect(screen.getByTestId("sources")).toHaveTextContent("source,source");
 let finish!:(result:DocumentInfo)=>void;vi.mocked(nativeAdapter.deleteNativeComment!).mockReturnValue(new Promise(resolve=>{finish=resolve;}));fireEvent.click(screen.getByRole("button",{name:"Delete comment"}));view.unmount();await act(async()=>finish({...info,id:"late-deletion"}));await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("late-deletion"));
});

it("moves a note from the page without opening a dialog and undoes all shared pages together",async()=>{
 await open();fireEvent.click(screen.getByRole("button",{name:"Move native note"}));await waitFor(()=>expect(screen.getByTestId("sources")).toHaveTextContent("edited,edited"));
 expect(nativeAdapter.editNativeComment).toHaveBeenCalledWith("source",{pageIndex:1,annotationIndex:2,expectedText:note.text,text:note.text,position:{x:80,y:100}});expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(screen.getByTestId("sources")).toHaveTextContent("source,source");expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();
});
it("releases an unused late page-movement source when the app unmounts",async()=>{
 let finish!:(result:DocumentInfo)=>void;vi.mocked(nativeAdapter.editNativeComment!).mockReturnValue(new Promise(resolve=>{finish=resolve;}));const view=await open();fireEvent.click(screen.getByRole("button",{name:"Move native note"}));await waitFor(()=>expect(nativeAdapter.editNativeComment).toHaveBeenCalledOnce());view.unmount();await act(async()=>finish({...info,id:"late-move"}));await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("late-move"));
});
