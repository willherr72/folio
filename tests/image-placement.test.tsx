import {useState} from "react";
import {act,cleanup,renderHook} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {useImagePlacement} from "../src/editor/use-image-placement";
import {useWorkspaceImages} from "../src/editor/use-image-resources";
import {imageApi,retainImages,flushImageReleases,drainImageAcquisitions} from "../src/editor/image-resources";
import {createSession,type Workspace} from "../src/editor/workspace";
import {createDemoAdapter} from "../src/editor/adapter";
import {undo,redo} from "../src/editor/model";
const id="a".repeat(64),info={id,width:80,height:40,mimeType:"image/png",hasAlpha:true,bitsPerChannel:8,byteLength:100};
function workspace():Workspace{const tab=createSession({id:"source",name:"Images.pdf",pages:[{width:200,height:300}]},{...createDemoAdapter(),kind:"native"},100);return {tabs:[tab],activeId:tab.id};}
function setup(initial=workspace()){
 const placed=vi.fn();const view=renderHook(()=>{const [state,setWorkspace]=useState(initial);const [disabled,setDisabled]=useState(false);useWorkspaceImages(state);const placement=useImagePlacement({workspace:state,setWorkspace,disabled,onPlaced:placed});return {workspace:state,setWorkspace,placement,setDisabled};});return {...view,placed};
}
beforeEach(()=>{vi.spyOn(imageApi,"importImage").mockResolvedValue({image:info,token:"picked"});vi.spyOn(imageApi,"acquire").mockResolvedValue({image:info,token:"acquired"});vi.spyOn(imageApi,"release").mockResolvedValue();});
afterEach(async()=>{cleanup();await retainImages(new Set());await drainImageAcquisitions();await flushImageReleases();vi.restoreAllMocks();});
it("places once, retains its transferred lease through undo/redo and releases it after tab close",async()=>{
 const view=setup();await act(async()=>view.result.current.placement.begin());
 expect(view.result.current.placement.pending?.image.assetId).toBe(id);expect(view.result.current.workspace.tabs[0].history.past).toHaveLength(0);
 act(()=>{view.result.current.placement.place({x:199,y:299});view.result.current.placement.place();});
 const history=view.result.current.workspace.tabs[0].history;expect(history.past).toHaveLength(1);expect(history.present.pages[0].overlays).toHaveLength(1);expect(view.placed).toHaveBeenCalledTimes(1);expect(imageApi.acquire).not.toHaveBeenCalled();expect(imageApi.release).not.toHaveBeenCalled();
 act(()=>view.result.current.setWorkspace(value=>({...value,tabs:value.tabs.map(tab=>({...tab,history:undo(tab.history)}))})));expect(view.result.current.workspace.tabs[0].history.present.pages[0].overlays).toHaveLength(0);expect(imageApi.release).not.toHaveBeenCalled();
 act(()=>view.result.current.setWorkspace(value=>({...value,tabs:value.tabs.map(tab=>({...tab,history:redo(tab.history)}))})));expect(view.result.current.workspace.tabs[0].history.present.pages[0].overlays).toHaveLength(1);
 act(()=>view.result.current.setWorkspace({tabs:[],activeId:null}));await flushImageReleases();
 expect(vi.mocked(imageApi.release).mock.calls).toEqual([["picked"]]);
});
it("keeps cursor previews out of history and cancels them without an edit",async()=>{
 const view=setup();await act(async()=>view.result.current.placement.begin());act(()=>view.result.current.placement.move({x:20,y:30}));
 expect(view.result.current.placement.pending?.image.x).toBe(0);expect(view.result.current.workspace.tabs[0].history.present.pages[0].overlays).toHaveLength(0);
 act(()=>view.result.current.placement.cancel());await flushImageReleases();expect(imageApi.release).toHaveBeenCalledWith("picked");expect(view.placed).not.toHaveBeenCalled();
});
it.each(["source","tab","unmount"] as const)("releases a picker response arriving after %s changes",async change=>{
 let resolve!:(value:any)=>void;vi.mocked(imageApi.importImage).mockReturnValue(new Promise(done=>resolve=done));const view=setup();let work!:Promise<void>;
 act(()=>{work=view.result.current.placement.begin();});
 if(change==="unmount")view.unmount();else act(()=>view.result.current.setWorkspace(value=>change==="tab"?{...value,activeId:null}:{...value,tabs:value.tabs.map(tab=>({...tab,history:{...tab.history,present:{...tab.history.present,pages:tab.history.present.pages.map(page=>({...page,sourceId:"replacement"}))}}}))}));
 await act(async()=>{resolve({image:info,token:"late"});await work;});await flushImageReleases();expect(imageApi.release).toHaveBeenCalledWith("late");expect(view.placed).not.toHaveBeenCalled();
});
it("handles picker cancellation and failure without making a document edit",async()=>{
 const view=setup();vi.mocked(imageApi.importImage).mockResolvedValueOnce(null).mockRejectedValueOnce(new Error("Unsupported image profile"));
 await act(async()=>view.result.current.placement.begin());expect(view.result.current.placement.pending).toBeNull();expect(view.result.current.placement.error).toBeNull();
 await act(async()=>view.result.current.placement.begin());expect(view.result.current.placement.error).toContain("Unsupported image profile");expect(view.result.current.workspace.tabs[0].history.past).toHaveLength(0);
});
it("refuses disabled placement and cleans up if a queued source change wins the commit race",async()=>{
 const view=setup();act(()=>view.result.current.setDisabled(true));await act(async()=>view.result.current.placement.begin());expect(imageApi.importImage).not.toHaveBeenCalled();
 act(()=>view.result.current.setDisabled(false));await act(async()=>view.result.current.placement.begin());
 act(()=>{view.result.current.setWorkspace(value=>({...value,tabs:value.tabs.map(tab=>({...tab,history:{...tab.history,present:{...tab.history.present,pages:tab.history.present.pages.map(page=>({...page,sourceId:"queued-source"}))}}}))}));view.result.current.placement.place();});
 await flushImageReleases();expect(view.result.current.workspace.tabs[0].history.present.pages[0].overlays).toHaveLength(0);expect(view.placed).not.toHaveBeenCalled();expect(imageApi.release).toHaveBeenCalledWith("picked");
});
