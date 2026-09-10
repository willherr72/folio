import {cleanup,render,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {PageView,clearRenderCache} from "../src/components/PageView";
import type {FolioAdapter} from "../src/editor/adapter";
import type {PagePlan} from "../src/editor/types";
const sources=["tab-cache","cache-budget","pending-close"];
afterEach(()=>{cleanup();clearRenderCache(sources);vi.restoreAllMocks();});
function page(sourceId:string,pageIndex:number):PagePlan{return {id:sourceId+pageIndex,sourceId,pageIndex,width:1200,height:1600,rotation:0,overlays:[]};}
function adapter(renderPage:FolioAdapter["renderPage"]):FolioAdapter{return {kind:"native",renderPage,openPdf:async()=>null,exportPdf:async()=>null,closeDocument:async()=>{},engineStatus:async()=>""};}
function Surface({source,index,engine}:{source:string;index:number;engine:FolioAdapter}){return <PageView adapter={engine} page={page(source,index)} pageNumber={index+1} zoom={200} tool="select" selectedOverlayId={null} pendingSignature={null} onSelectOverlay={()=>{}} onAddText={()=>{}} onPlaceSignature={()=>{}} onMoveOverlay={()=>{}}/>;}
it("reuses a full-size render after leaving and returning to a tab",async()=>{
 const revoke=vi.fn();Object.defineProperty(URL,"revokeObjectURL",{configurable:true,value:revoke});
 const renderPage=vi.fn(async(_source:string,index:number)=>"blob:tab-"+index);
 const engine=adapter(renderPage);
 const view=render(<Surface key="a" source="tab-cache" index={0} engine={engine}/>);
 await waitFor(()=>expect(view.container.querySelector('image[href="blob:tab-0"]')).not.toBeNull());
 view.rerender(<Surface key="b" source="tab-cache" index={1} engine={engine}/>);
 await waitFor(()=>expect(view.container.querySelector('image[href="blob:tab-1"]')).not.toBeNull());
 view.rerender(<Surface key="a" source="tab-cache" index={0} engine={engine}/>);
 await waitFor(()=>expect(view.container.querySelector('image[href="blob:tab-0"]')).not.toBeNull());
 expect(renderPage).toHaveBeenCalledTimes(2);
 expect(revoke).not.toHaveBeenCalled();
});
it("evicts old large images under the memory budget while protecting mounted pages",async()=>{
 const revoke=vi.fn();Object.defineProperty(URL,"revokeObjectURL",{configurable:true,value:revoke});
 const engine=adapter(vi.fn(async(_source:string,index:number)=>"blob:budget-"+index));
 const surfaces=(index:number)=><><Surface key="anchor" source="cache-budget" index={0} engine={engine}/><Surface key={index} source="cache-budget" index={index} engine={engine}/></>;
 const view=render(surfaces(1));
 await waitFor(()=>expect(view.container.querySelectorAll("image")).toHaveLength(2));
 view.rerender(surfaces(2));
 await waitFor(()=>expect(view.container.querySelector('image[href="blob:budget-2"]')).not.toBeNull());
 expect(revoke).not.toHaveBeenCalledWith("blob:budget-1");
 view.rerender(surfaces(3));
 await waitFor(()=>expect(view.container.querySelector('image[href="blob:budget-3"]')).not.toBeNull());
 await waitFor(()=>expect(revoke).toHaveBeenCalledWith("blob:budget-1"));
 expect(revoke).not.toHaveBeenCalledWith("blob:budget-0");
 expect(revoke).not.toHaveBeenCalledWith("blob:budget-2");
});
it("releases a closed source even when its last native render finishes later",async()=>{
 const revoke=vi.fn();Object.defineProperty(URL,"revokeObjectURL",{configurable:true,value:revoke});
 let complete!:(url:string)=>void;
 const engine=adapter(()=>new Promise(resolve=>{complete=resolve;}));
 const view=render(<Surface source="pending-close" index={0} engine={engine}/>);
 clearRenderCache(["pending-close"]);view.unmount();complete("blob:closed-late");
 await waitFor(()=>expect(revoke).toHaveBeenCalledWith("blob:closed-late"));
 expect(revoke).toHaveBeenCalledTimes(1);
});
