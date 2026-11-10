import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import {clearRenderCache} from "../src/components/PageView";
import {imageApi,retainImages,flushImageReleases,drainImageAcquisitions} from "../src/editor/image-resources";
vi.mock("../src/editor/useWorkspaceRecovery",()=>({useWorkspaceRecovery:()=>({ready:true,checkpoint:vi.fn().mockResolvedValue(undefined),schedule:vi.fn()})}));
vi.mock("@tauri-apps/api/window",()=>({getCurrentWindow:()=>({onCloseRequested:vi.fn().mockResolvedValue(()=>{})})}));
const info={id:"a".repeat(64),width:80,height:40,mimeType:"image/png",hasAlpha:true,bitsPerChannel:8,byteLength:100};
beforeEach(()=>{
 localStorage.clear();Object.defineProperty(window,"__TAURI_INTERNALS__",{value:{invoke:vi.fn().mockResolvedValue(0),transformCallback:()=>0},configurable:true});
 Object.defineProperty(URL,"revokeObjectURL",{configurable:true,value:vi.fn()});
 vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue({id:"keyboard-source",name:"Keyboard.pdf",pages:[{width:612,height:792}]});
 vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("blob:original");vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"renderPagePlan").mockResolvedValue("blob:composed");vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"getPageText").mockResolvedValue({characters:[]});
 vi.spyOn(imageApi,"importImage").mockResolvedValue({image:info,token:"picked"});vi.spyOn(imageApi,"release").mockResolvedValue();vi.spyOn(imageApi,"acquire").mockResolvedValue({image:info,token:"acquired"});
});
afterEach(async()=>{cleanup();clearRenderCache(["keyboard-source"]);await retainImages(new Set());await drainImageAcquisitions();await flushImageReleases();vi.restoreAllMocks();delete (window as any).__TAURI_INTERNALS__;});
async function pick(){const view=render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Keyboard.pdf"});fireEvent.click(screen.getByRole("button",{name:"Insert image"}));await screen.findByRole("button",{name:"Place image"});return view;}
it("leaves Enter on Cancel to activate its button without placing an image",async()=>{
 await pick();const cancel=screen.getByRole("button",{name:"Cancel image placement"});cancel.focus();
 expect(fireEvent.keyDown(cancel,{key:"Enter"})).toBe(true);expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();
 fireEvent.click(cancel);await flushImageReleases();expect(vi.mocked(imageApi.release).mock.calls).toEqual([["picked"]]);expect(screen.queryByRole("button",{name:"Place image"})).toBeNull();expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();
});
it("leaves Enter on Retry to retry the native preview and retain the pending image",async()=>{
 vi.mocked(nativeAdapter.renderPagePlan!).mockRejectedValueOnce(new Error("Temporary native failure"));await pick();const retry=await screen.findByRole("button",{name:"Retry preview"});retry.focus();
 expect(fireEvent.keyDown(retry,{key:"Enter"})).toBe(true);expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();
 fireEvent.click(retry);await waitFor(()=>expect(nativeAdapter.renderPagePlan).toHaveBeenCalledTimes(2));expect(screen.getByRole("button",{name:"Place image"})).toBeInTheDocument();expect(imageApi.release).not.toHaveBeenCalled();expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();
});
it.each(["toolbar","shortcut"])("cancels pending placement when Find opens by %s with no matching text",async method=>{
 const view=await pick();await waitFor(()=>expect(view.container.querySelector('.page-stage image[href="blob:composed"]')).not.toBeNull());
 if(method==="toolbar")fireEvent.click(screen.getByRole("button",{name:"Find"}));else fireEvent.keyDown(window,{key:"f",ctrlKey:true});
 fireEvent.change(screen.getByRole("searchbox"),{target:{value:"missing"}});await waitFor(()=>expect(screen.queryByRole("button",{name:"Place image"})).toBeNull());
 await flushImageReleases();expect(vi.mocked(imageApi.release).mock.calls).toEqual([["picked"]]);expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();expect(view.container.querySelector('.page-stage image[href="blob:composed"]')).toBeNull();
});
