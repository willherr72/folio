import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import {imageApi} from "../src/editor/image-resources";
import {clearSearchTextCache} from "../src/editor/search";
const size={width:300,height:400};
vi.mock("../src/editor/useWorkspaceRecovery",()=>({useWorkspaceRecovery:()=>({ready:true,checkpoint:vi.fn().mockResolvedValue(undefined)})}));
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:(props:any)=><div><output data-testid="plans">{JSON.stringify(props.pages)}</output><button onClick={()=>props.onEditImage(props.selectedPageId,{objectPath:[2],selectionId:"a".repeat(64),corners:[{x:0,y:0},{x:100,y:0},{x:100,y:100},{x:0,y:100}],bounds:{x:0,y:0,width:100,height:100},pixelWidth:20,pixelHeight:10})}>Choose existing image</button></div>}));
const saved={list:nativeAdapter.listExistingImages,preview:nativeAdapter.previewImageReplacementOccurrence,edit:nativeAdapter.previewImageEditOccurrence};
beforeEach(()=>{
 localStorage.clear();Object.defineProperty(window,"__TAURI_INTERNALS__",{value:{invoke:vi.fn().mockResolvedValue(0),transformCallback:()=>0},configurable:true});
 vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue({id:"source",name:"Pictures.pdf",pages:[size,size]});vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,AA");vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"renderPagePlan").mockResolvedValue("data:image/png;base64,AA");vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();
 nativeAdapter.listExistingImages=vi.fn().mockResolvedValue({images:[]});nativeAdapter.previewImageReplacementOccurrence=vi.fn().mockResolvedValue({document:{id:"candidate",name:"Edited.pdf",pages:[size,size,size]},pageIndices:[0,2,1]});
 nativeAdapter.previewImageEditOccurrence=vi.fn().mockResolvedValue({document:{id:"candidate",name:"Edited.pdf",pages:[size,size,size]},pageIndices:[0,2,1]});
 vi.spyOn(imageApi,"importImage").mockResolvedValue({image:{id:"b".repeat(64),width:80,height:40,mimeType:"image/png",hasAlpha:true,bitsPerChannel:8,byteLength:100},token:"app-image-lease"});vi.spyOn(imageApi,"release").mockResolvedValue();
});
afterEach(()=>{cleanup();clearSearchTextCache(["source","candidate"]);vi.restoreAllMocks();nativeAdapter.listExistingImages=saved.list;nativeAdapter.previewImageReplacementOccurrence=saved.preview;nativeAdapter.previewImageEditOccurrence=saved.edit;delete (window as any).__TAURI_INTERNALS__;});
const plans=()=>JSON.parse(screen.getByTestId("plans").textContent!);
it.each(["replace","transform","remove"])("edits a duplicated image with %s and one Undo while retaining page transforms",async(action)=>{
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Pictures.pdf"});fireEvent.click(screen.getByRole("button",{name:"Rotate"}));fireEvent.click(screen.getByRole("button",{name:"Duplicate"}));const before=plans();
 fireEvent.click(screen.getByRole("button",{name:"Edit images"}));fireEvent.click(screen.getByRole("button",{name:"Choose existing image"}));await screen.findByRole("region",{name:"Image editing"});expect(screen.getByRole("button",{name:"Zoom in"})).toBeEnabled();fireEvent.click(screen.getByRole("button",{name:"Zoom in"}));fireEvent.click(screen.getByRole("button",{name:"Next page"}));expect(screen.getByRole("button",{name:"Locate image"})).toBeEnabled();expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();
 if(action==="replace"){
 fireEvent.change(screen.getByLabelText("Image action"),{target:{value:"replace"}});
 fireEvent.click(screen.getByRole("button",{name:"Choose image…"}));await screen.findByText(/80 × 40/);fireEvent.click(screen.getByRole("button",{name:"Preview replacement"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Apply replacement"})).toBeEnabled());
 expect(nativeAdapter.previewImageReplacementOccurrence).toHaveBeenCalledWith("source",[0,0,1],1,[2],"a".repeat(64),"b".repeat(64),"fit");fireEvent.click(screen.getByRole("button",{name:"Apply replacement"}));await waitFor(()=>expect(screen.queryByRole("region",{name:"Image editing"})).not.toBeInTheDocument());
 }else{
  fireEvent.change(screen.getByLabelText("Image action"),{target:{value:action}});
  if(action==="transform")fireEvent.change(screen.getByLabelText("Width (pt)"),{target:{value:"150"}});
  fireEvent.click(screen.getByRole("button",{name:"Preview changes"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Apply changes"})).toBeEnabled());
  expect(nativeAdapter.previewImageEditOccurrence).toHaveBeenCalledWith("source",[0,0,1],1,[2],"a".repeat(64),action==="remove"?{kind:"remove"}:{kind:"adjust",crop:{left:0,top:0,right:1,bottom:1},transform:{deltaX:0,deltaY:0,scaleX:1.5,scaleY:1.5,rotationDegrees:0}});
  expect(imageApi.importImage).not.toHaveBeenCalled();fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));await waitFor(()=>expect(screen.queryByRole("region",{name:"Image editing"})).not.toBeInTheDocument());
 }
 await waitFor(()=>expect(screen.getByRole("button",{name:"Edit images"})).toHaveFocus());
 const after=plans();expect(after.map((p:any)=>[p.sourceId,p.pageIndex])).toEqual([["candidate",0],["candidate",2],["candidate",1]]);expect(after.map((p:any)=>[p.id,p.rotation,p.overlays])).toEqual(before.map((p:any)=>[p.id,p.rotation,p.overlays]));
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(plans()).toEqual(before);fireEvent.click(screen.getByRole("button",{name:"Redo"}));expect(plans()).toEqual(after);expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith("candidate");
});

it("cancels the canvas session with Escape from a reading control and restores tool focus",async()=>{
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Pictures.pdf"});fireEvent.click(screen.getByRole("button",{name:"Edit images"}));fireEvent.click(screen.getByRole("button",{name:"Choose existing image"}));await screen.findByRole("region",{name:"Image editing"});const zoom=screen.getByRole("button",{name:"Zoom in"});zoom.focus();fireEvent.click(zoom);fireEvent.keyDown(zoom,{key:"Escape"});await waitFor(()=>expect(screen.queryByRole("region",{name:"Image editing"})).toBeNull());expect(screen.getByRole("button",{name:"Edit images"})).toHaveFocus();expect(nativeAdapter.previewImageEditOccurrence).not.toHaveBeenCalled();
});
