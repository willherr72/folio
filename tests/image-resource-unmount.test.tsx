import {StrictMode} from "react";
import {act,cleanup,render} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {useWorkspaceImages} from "../src/editor/use-image-resources";
import {imageApi,retainImages,drainImageAcquisitions,flushImageReleases,imageResourceState} from "../src/editor/image-resources";
import {createSession,type Workspace} from "../src/editor/workspace";
import {createDemoAdapter} from "../src/editor/adapter";
function Harness({workspace}:{workspace:Workspace}){useWorkspaceImages(workspace);return null;}
afterEach(async()=>{cleanup();await retainImages(new Set());await drainImageAcquisitions();await flushImageReleases();vi.restoreAllMocks();});
it("does not acquire twice during StrictMode replay and releases an acquisition arriving after unmount",async()=>{
 const id="f".repeat(64);let resolve!:(value:any)=>void;
 const acquire=vi.spyOn(imageApi,"acquire").mockReturnValue(new Promise(done=>resolve=done));const release=vi.spyOn(imageApi,"release").mockResolvedValue();
 const session=createSession({id:"source",name:"Images",pages:[{width:200,height:300,overlays:[{type:"image",id:"photo",assetId:id,x:10,y:20,width:40,height:20,rotation:0}]}]},{...createDemoAdapter(),kind:"native"},100);
 const view=render(<StrictMode><Harness workspace={{activeId:session.id,tabs:[session]}}/></StrictMode>);
 await act(async()=>{});expect(acquire).toHaveBeenCalledTimes(1);view.unmount();await act(async()=>{});
 await act(async()=>{resolve({image:{id,width:4,height:2,mimeType:"image/png",hasAlpha:true,bitsPerChannel:8,byteLength:90},token:"late-token"});await drainImageAcquisitions();});
 expect(release).toHaveBeenCalledTimes(1);expect(release).toHaveBeenCalledWith("late-token");expect(imageResourceState(id)).toBeUndefined();
});
