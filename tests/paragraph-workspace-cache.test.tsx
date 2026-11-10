import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {ParagraphWorkspace} from "../src/components/ParagraphWorkspace";
import {clearRenderCache} from "../src/components/PageView";
import {createDemoAdapter,type FolioAdapter} from "../src/editor/adapter";
import type {CanvasParagraphView} from "../src/editor/paragraph-canvas";
import type {Overlay} from "../src/editor/types";
import type {ParagraphPreview} from "../src/editor/paragraph-transaction";

const source="workspace-cache-source";
const overlays:Overlay[]=[];
const revoke=()=>{const fn=vi.fn();Object.defineProperty(URL,"revokeObjectURL",{configurable:true,value:fn});return fn;};
function options(adapter:FolioAdapter,marks:Overlay[]=overlays){return {
 adapter,sourceId:source,pageIndex:0,pageWidth:800,pageHeight:900,overlays:marks,candidate:null as ParagraphPreview|null,
 onReady:vi.fn(),choosing:false,enabled:true,runs:[],selected:[],onToggle:vi.fn(),onChange:vi.fn(),onGestureActive:vi.fn(),
 canvas:{pageId:"cache-page",sessionKey:"session",zoom:100,onLocate:vi.fn(),onModalChange:vi.fn(),renderCanvas:(v:CanvasParagraphView)=><output data-testid="raster">{v.raster.url}</output>},
};}
const workspace=(props:ReturnType<typeof options>)=><ParagraphWorkspace {...props}><span>Controls</span></ParagraphWorkspace>;
afterEach(()=>{cleanup();clearRenderCache([source,source+"-other","workspace-cache-candidate"]);vi.restoreAllMocks();});

it("reuses an unchanged original across paragraph sessions until source invalidation",async()=>{
 const released=revoke(),adapter={...createDemoAdapter(),renderPage:vi.fn().mockResolvedValue("blob:shared-original")},props=options(adapter);
 const first=render(workspace(props));await screen.findByText("blob:shared-original");first.unmount();
 const second=render(workspace(props));await screen.findByText("blob:shared-original");
 expect(adapter.renderPage).toHaveBeenCalledTimes(1);expect(released).not.toHaveBeenCalled();
 second.unmount();clearRenderCache([source]);await waitFor(()=>expect(released.mock.calls).toEqual([["blob:shared-original"]]));
});

it("keeps a shared original alive until its last mounted owner leaves after invalidation",async()=>{
 const released=revoke(),adapter={...createDemoAdapter(),renderPage:vi.fn().mockResolvedValue("blob:two-owners")},props=options(adapter);
 const first=render(workspace(props)),second=render(workspace(props));await waitFor(()=>expect(screen.getAllByText("blob:two-owners")).toHaveLength(2));
 expect(adapter.renderPage).toHaveBeenCalledTimes(1);clearRenderCache([source]);first.unmount();
 expect(released).not.toHaveBeenCalled();expect(screen.getByText("blob:two-owners")).toBeVisible();
 second.unmount();await waitFor(()=>expect(released.mock.calls).toEqual([["blob:two-owners"]]));
});

it("releases a late original when its unmounted source was invalidated",async()=>{
 const released=revoke();let finish!:(url:string)=>void;
 const adapter={...createDemoAdapter(),renderPage:vi.fn(()=>new Promise<string>(resolve=>finish=resolve))};
 const view=render(workspace(options(adapter)));await waitFor(()=>expect(adapter.renderPage).toHaveBeenCalledTimes(1));
 view.unmount();clearRenderCache([source]);await act(async()=>finish("blob:late-original"));
 expect(released.mock.calls).toEqual([["blob:late-original"]]);
});

it("retries an original rendering failure without changing source or zoom",async()=>{
 revoke();const adapter={...createDemoAdapter(),renderPage:vi.fn().mockRejectedValueOnce(new Error("Temporary render failure")).mockResolvedValue("blob:retried")};
 render(workspace(options(adapter)));await screen.findByText(/Temporary render failure/);
 fireEvent.click(screen.getByRole("button",{name:"Retry preview"}));await screen.findByText("blob:retried");
 expect(adapter.renderPage).toHaveBeenCalledTimes(2);
});

it("renders annotated originals through composition on every session",async()=>{
 const released=revoke(),adapter={...createDemoAdapter(),renderPage:vi.fn(),renderPagePlan:vi.fn().mockResolvedValueOnce("blob:composed-one").mockResolvedValue("blob:composed-two")};
 const marks:Overlay[]=[{type:"comment",id:"note",x:10,y:20,text:"Saved note",color:"#000000"}],props=options(adapter,marks);
 const first=render(workspace(props));await screen.findByText("blob:composed-one");first.unmount();
 const second=render(workspace(props));await screen.findByText("blob:composed-two");second.unmount();
 expect(adapter.renderPage).not.toHaveBeenCalled();expect(adapter.renderPagePlan).toHaveBeenCalledTimes(2);
 expect(adapter.renderPagePlan.mock.calls.every(([plan])=>plan.overlays===marks)).toBe(true);
 expect(released.mock.calls.map(([url])=>url)).toEqual(["blob:composed-one","blob:composed-two"]);
});

it("uses the requested source and raster width after zoom or source changes",async()=>{
 revoke();const adapter={...createDemoAdapter(),renderPage:vi.fn(async(id:string,page:number,width:number)=>`${id}/${page}/${width}`)},props=options(adapter);
 const view=render(workspace(props));await screen.findByText(`${source}/0/800`);
 view.rerender(workspace({...props,canvas:{...props.canvas,zoom:200}}));await screen.findByText(`${source}/0/1600`);
 view.rerender(workspace({...props,sourceId:source+"-other"}));await screen.findByText(`${source}-other/0/800`);
 expect(adapter.renderPage).toHaveBeenCalledTimes(3);
});
