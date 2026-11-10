import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import {imageApi,retainImages,flushImageReleases,drainImageAcquisitions} from "../src/editor/image-resources";
vi.mock("@tauri-apps/api/window",()=>({getCurrentWindow:()=>({onCloseRequested:vi.fn().mockResolvedValue(()=>{})})}));
vi.mock("../src/editor/useWorkspaceRecovery",()=>({useWorkspaceRecovery:()=>({ready:true,checkpoint:vi.fn().mockResolvedValue(undefined),schedule:vi.fn()})}));
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:(p:any)=><div><output data-testid="images">{p.pages.flatMap((page:any)=>page.overlays).filter((o:any)=>o.type==="image").length}</output><output data-testid="pending">{p.pendingImagePlacement?.image.assetId??"none"}</output><button onClick={()=>p.onPlaceImage({x:200,y:200})}>Place on page</button></div>}));
vi.mock("../src/components/PageView",()=>({Thumbnail:()=>null,clearRenderCache:vi.fn()}));
const id="a".repeat(64),info={id,width:80,height:40,mimeType:"image/png",hasAlpha:true,bitsPerChannel:8,byteLength:100};
beforeEach(()=>{
 localStorage.clear();Object.defineProperty(window,"__TAURI_INTERNALS__",{value:{invoke:vi.fn().mockResolvedValue(0),transformCallback:()=>0},configurable:true});
 vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue({id:"source",name:"Source.pdf",pages:[{width:612,height:792}]});
 vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();
 vi.spyOn(imageApi,"importImage").mockResolvedValue({image:info,token:"picked"});vi.spyOn(imageApi,"release").mockResolvedValue();vi.spyOn(imageApi,"acquire").mockResolvedValue({image:info,token:"acquired"});
});
afterEach(async()=>{cleanup();await retainImages(new Set());await drainImageAcquisitions();await flushImageReleases();vi.restoreAllMocks();delete (window as any).__TAURI_INTERNALS__;});
async function open(){render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Source.pdf"});}
async function pick(){fireEvent.click(screen.getByRole("button",{name:"Insert image"}));await waitFor(()=>expect(screen.getByTestId("pending")).toHaveTextContent(id));}
it("places from the toolbar, selects image properties, and supports undo/redo/delete",async()=>{
 await open();await pick();expect(screen.getByTestId("images")).toHaveTextContent("0");
 fireEvent.click(screen.getByRole("button",{name:"Place on page"}));expect(screen.getByTestId("images")).toHaveTextContent("1");expect(screen.getByTestId("pending")).toHaveTextContent("none");expect(screen.getByRole("heading",{name:"Image"})).toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(screen.getByTestId("images")).toHaveTextContent("0");
 fireEvent.click(screen.getByRole("button",{name:"Redo"}));expect(screen.getByTestId("images")).toHaveTextContent("1");
 fireEvent.keyDown(window,{key:"Delete"});expect(screen.getByTestId("images")).toHaveTextContent("0");expect(imageApi.release).not.toHaveBeenCalled();
});
it("supports keyboard placement and Escape cancellation without changing the document",async()=>{
 await open();await pick();fireEvent.keyDown(window,{key:"Escape"});expect(screen.getByTestId("pending")).toHaveTextContent("none");expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();
 await flushImageReleases();expect(imageApi.release).toHaveBeenCalledWith("picked");
 vi.mocked(imageApi.importImage).mockResolvedValueOnce({image:info,token:"second"});await pick();fireEvent.keyDown(window,{key:"Enter"});expect(screen.getByTestId("images")).toHaveTextContent("1");
});
it("commits image size and rotation as separate undoable actions",async()=>{
 await open();await pick();fireEvent.click(screen.getByRole("button",{name:"Place on page"}));
 const width=()=>screen.getByRole("spinbutton",{name:"Image width (pt)"});expect(width()).toHaveValue(60);
 fireEvent.change(width(),{target:{value:"120"}});fireEvent.keyDown(width(),{key:"Enter"});expect(width()).toHaveValue(120);expect(screen.getByRole("spinbutton",{name:"Image height (pt)"})).toHaveValue(60);
 fireEvent.click(screen.getByRole("button",{name:"Rotate image clockwise"}));expect(width()).toHaveValue(60);
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(width()).toHaveValue(120);
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(width()).toHaveValue(60);
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(screen.getByTestId("images")).toHaveTextContent("0");expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();
});
it("leaves the document clean after picker cancellation or import failure",async()=>{
 await open();vi.mocked(imageApi.importImage).mockResolvedValueOnce(null).mockRejectedValueOnce(new Error("Image exceeds the pixel limit"));
 await act(async()=>fireEvent.click(screen.getByRole("button",{name:"Insert image"})));
 expect(screen.getByTestId("pending")).toHaveTextContent("none");expect(screen.getByRole("button",{name:"Select"})).toHaveClass("active");
 await act(async()=>fireEvent.click(screen.getByRole("button",{name:"Insert image"})));
 expect(screen.getByRole("alert")).toHaveTextContent("Image exceeds the pixel limit");expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();
});
it("disables image insertion for a reader without annotation permission",async()=>{
 vi.mocked(nativeAdapter.openPdf).mockResolvedValue({id:"source",name:"Source.pdf",pages:[{width:612,height:792}],protection:{documentId:"protected",owner:false,revision:4,permissions:{print:true,printHighQuality:true,copyContent:true,editContent:false,annotate:false,fillForms:false,assemble:false}}});
 await open();expect(screen.getByRole("button",{name:"Insert image"})).toBeDisabled();expect(imageApi.importImage).not.toHaveBeenCalled();
});
