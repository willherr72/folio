import {act,cleanup,fireEvent,render,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {PageView,Thumbnail,clearRenderCache} from "../src/components/PageView";
import {StrictMode} from "react";
import {createDemoAdapter} from "../src/editor/adapter";
import type {PagePlan,Overlay} from "../src/editor/types";
const image={type:"image" as const,id:"photo",assetId:"a".repeat(64),x:40,y:50,width:80,height:40,rotation:90 as const};
const base:PagePlan={id:"image-page",sourceId:"image-preview-source",pageIndex:0,width:200,height:300,rotation:90,overlays:[image]};
const adapter=()=>({...createDemoAdapter(),kind:"native" as const,renderPage:vi.fn().mockResolvedValue("blob:source"),renderPagePlan:vi.fn().mockResolvedValue("blob:composed"),getPageText:vi.fn().mockResolvedValue({characters:[]})});
const props=(service=adapter(),page=base)=>({adapter:service,page,pageNumber:1,zoom:100,tool:"select" as const,selectedOverlayId:null,pendingSignature:null,onSelectOverlay:vi.fn(),onAddText:vi.fn(),onPlaceSignature:vi.fn(),onMoveOverlay:vi.fn()});
function deferred<T>(){let resolve!:(value:T)=>void;const promise=new Promise<T>(done=>resolve=done);return {promise,resolve};}
beforeEach(()=>{Object.defineProperty(URL,"revokeObjectURL",{configurable:true,value:vi.fn()});
 vi.stubGlobal("PointerEvent",class extends MouseEvent{pointerId:number;constructor(type:string,init:PointerEventInit={}){super(type,init);this.pointerId=init.pointerId??1;}});
 Object.defineProperty(SVGElement.prototype,"setPointerCapture",{configurable:true,value:vi.fn()});Object.defineProperty(SVGElement.prototype,"releasePointerCapture",{configurable:true,value:vi.fn()});
});
afterEach(()=>{cleanup();clearRenderCache([base.sourceId]);vi.restoreAllMocks();vi.unstubAllGlobals();});

it("previews a pending image on an empty page and places over existing overlay hit targets",async()=>{
 const service=adapter(),onPlaceImage=vi.fn(),onMoveImage=vi.fn();const page={...base,overlays:[{type:"text" as const,id:"text",x:20,y:20,text:"Existing overlay",fontSize:12,color:"#000000"}]};
 const options=props(service,page);const view=render(<PageView {...options} tool="image" pendingImage={image} onPlaceImage={onPlaceImage} onMoveImage={onMoveImage}/>);
 await waitFor(()=>expect(service.renderPagePlan).toHaveBeenCalledWith(expect.objectContaining({overlays:[...page.overlays,image]}),expect.any(Number)));
 const svg=view.container.querySelector("svg")!;vi.spyOn(svg,"getBoundingClientRect").mockReturnValue({left:0,top:0,width:300,height:200,right:300,bottom:200,x:0,y:0,toJSON(){}});
 fireEvent.pointerMove(svg,{clientX:80,clientY:60});expect(onMoveImage).toHaveBeenCalledWith({x:60,y:220});
 fireEvent.pointerDown(view.getByRole("button",{name:"Text: Existing overlay"}),{clientX:80,clientY:60,button:0});
 expect(onPlaceImage).toHaveBeenCalledWith({x:60,y:220});expect(options.onSelectOverlay).not.toHaveBeenCalled();expect(page.overlays).toHaveLength(1);
});
it("retains a pending image after preview failure and retries the same native composition",async()=>{
 const service=adapter();service.renderPagePlan.mockRejectedValueOnce(new Error("Temporary preview failure"));const onPlaceImage=vi.fn();
 const view=render(<PageView {...props(service,{...base,overlays:[]})} tool="image" pendingImage={image} onPlaceImage={onPlaceImage}/>);
 expect(await view.findByText("Temporary preview failure")).toBeInTheDocument();
 fireEvent.click(view.getByRole("button",{name:"Retry preview"}));
 await waitFor(()=>expect(view.container.querySelector('image[href="blob:composed"]')).not.toBeNull());
 expect(service.renderPagePlan).toHaveBeenCalledTimes(2);expect(service.renderPagePlan.mock.calls[1][0].overlays).toEqual([image]);expect(onPlaceImage).not.toHaveBeenCalled();
});

it("uses native page composition once and keeps SVG interaction without double-painting overlays",async()=>{
 const page:PagePlan={...base,overlays:[image,{type:"text",id:"text",x:20,y:20,text:"Overlay text",fontSize:12,color:"#000000"},{type:"ink",id:"ink",paths:[[{x:10,y:10},{x:20,y:20}]],strokeWidth:2,color:"#000000"},{type:"highlight",id:"mark",rects:[{x:30,y:40,width:50,height:20}],color:"#00ffff"}]};
 const options=props(adapter(),page);const view=render(<PageView {...options}/>);
 await waitFor(()=>expect(view.container.querySelector('image[href="blob:composed"]')).not.toBeNull());
 expect(options.adapter.renderPage).not.toHaveBeenCalled();
 expect(options.adapter.renderPagePlan).toHaveBeenCalledWith(expect.objectContaining({rotation:page.rotation,overlays:page.overlays}),expect.any(Number));
 expect(view.container.querySelectorAll("[data-overlay] text,[data-overlay] polyline,.highlight-mark")).toHaveLength(0);
 fireEvent.keyDown(view.getByRole("button",{name:"Image"}),{key:"Enter"});expect(options.onSelectOverlay).toHaveBeenCalledWith("photo");
});
it("composes thumbnails with the same image-aware plan",async()=>{
 const service=adapter();const view=render(<Thumbnail adapter={service} page={base}/>);
 await waitFor(()=>expect(view.container.querySelector('image[href="blob:composed"]')).not.toBeNull());
 expect(service.renderPagePlan).toHaveBeenCalledWith(expect.objectContaining({rotation:base.rotation}),240);
 expect(service.renderPage).not.toHaveBeenCalled();
});
it("keeps annotation hit targets above image content in the native painting order",async()=>{
 const page:PagePlan={...base,overlays:[{type:"highlight",id:"mark",rects:[{x:30,y:40,width:50,height:20}],color:"#00ffff"},image]};
 const view=render(<PageView {...props(adapter(),page)}/>);
 await waitFor(()=>expect(view.container.querySelector('image[href="blob:composed"]')).not.toBeNull());
 expect([...view.container.querySelectorAll("[data-overlay]")].map(node=>node.getAttribute("data-overlay"))).toEqual(["photo","mark"]);
});
it("coalesces changes while a render is running and disposes stale and unmounted URLs",async()=>{
 const first=deferred<string>(),last=deferred<string>();const service=adapter();service.renderPagePlan.mockReturnValueOnce(first.promise).mockReturnValueOnce(last.promise);
 const view=render(<PageView {...props(service)}/>);
 await waitFor(()=>expect(service.renderPagePlan).toHaveBeenCalledTimes(1));
 view.rerender(<PageView {...props(service,{...base,overlays:[{...image,x:60}]})}/>);
 view.rerender(<PageView {...props(service,{...base,overlays:[{...image,x:70}]})}/>);
 expect(service.renderPagePlan).toHaveBeenCalledTimes(1);
 await act(async()=>{first.resolve("blob:obsolete");});
 await waitFor(()=>expect(service.renderPagePlan).toHaveBeenCalledTimes(2));
 expect(service.renderPagePlan.mock.calls[1][0].overlays[0].x).toBe(70);
 expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:obsolete");
 expect(view.container.querySelector('image[href="blob:obsolete"]')).toBeNull();
 view.unmount();await act(async()=>{last.resolve("blob:unmounted");});
 expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:unmounted");
});
it("shows composition failure instead of silently omitting an image",async()=>{
 const service=adapter();service.renderPagePlan.mockRejectedValue(new Error("Image resource is unavailable"));
 const view=render(<PageView {...props(service)}/>);
 expect(await view.findByText("Image resource is unavailable")).toBeInTheDocument();expect(service.renderPage).not.toHaveBeenCalled();
});
it.each(["draw","signature"] as const)("composes transient %s strokes below annotation markup and removes them on cancellation",async tool=>{
 const service=adapter(),onDraw=vi.fn();const page:PagePlan={...base,rotation:0,overlays:[image,{type:"highlight",id:"mark",rects:[{x:30,y:40,width:100,height:50}],color:"#00ffff"}]};
 const view=render(<PageView {...props(service,page)} tool={tool} pendingSignature={tool==="signature"?[[{x:0,y:0},{x:40,y:20}]]:null} onDraw={onDraw}/>);
 const svg=view.container.querySelector("svg")!;vi.spyOn(svg,"getBoundingClientRect").mockReturnValue({left:0,top:0,width:200,height:300,right:200,bottom:300,x:0,y:0,toJSON(){}});
 await waitFor(()=>expect(service.renderPagePlan).toHaveBeenCalled());
 if(tool==="draw")fireEvent.pointerDown(svg,{clientX:40,clientY:50,button:0});
 fireEvent.pointerMove(svg,{clientX:80,clientY:70});
 await waitFor(()=>expect(service.renderPagePlan.mock.calls.at(-1)?.[0].overlays.some((overlay:Overlay)=>overlay.type==="ink")).toBe(true));
 expect(svg.querySelector(".draft-ink,.signature-preview")).toBeNull();expect(onDraw).not.toHaveBeenCalled();
 if(tool==="draw")fireEvent.pointerCancel(svg);else fireEvent.keyDown(window,{key:"Escape"});
 await waitFor(()=>expect(service.renderPagePlan.mock.calls.at(-1)?.[0].overlays.some((overlay:Overlay)=>overlay.type==="ink")).toBe(false));
 expect(page.overlays).toHaveLength(2);expect(onDraw).not.toHaveBeenCalled();
});
it("survives StrictMode replay and hides the old page immediately when its source changes",async()=>{
 const next=deferred<string>();const service=adapter();service.renderPagePlan.mockResolvedValueOnce("blob:first-page").mockReturnValueOnce(next.promise);
 const view=render(<StrictMode><PageView {...props(service)}/></StrictMode>);
 await waitFor(()=>expect(view.container.querySelector('image[href="blob:first-page"]')).not.toBeNull());
 view.rerender(<StrictMode><PageView {...props(service,{...base,sourceId:"second-source"})}/></StrictMode>);
 expect(view.container.querySelector('image[href="blob:first-page"]')).toBeNull();
 await act(async()=>next.resolve("blob:second-page"));
 expect(view.container.querySelector('image[href="blob:second-page"]')).not.toBeNull();
 expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:first-page");
});

it("places the shared canvas raster in the rotated cropped frame without duplicate rendering",async()=>{
 const service={...adapter(),listExistingImages:vi.fn(async()=>({images:[]}))},page={...base,crop:{x:20,y:30,width:100,height:150}};
 const selected={objectPath:[2],selectionId:"a",corners:[{x:30,y:50},{x:80,y:50},{x:80,y:90},{x:30,y:90}] as [import("../src/editor/types").ImagePoint,import("../src/editor/types").ImagePoint,import("../src/editor/types").ImagePoint,import("../src/editor/types").ImagePoint],bounds:{x:30,y:50,width:50,height:40},pixelWidth:20,pixelHeight:10};
 const canvas={pageId:page.id,sessionKey:"session",image:selected,frame:selected.corners,locked:true,enabled:true,raster:{url:"data:shared",loading:false,retry:vi.fn()},onChange:vi.fn(),onGestureActive:vi.fn()};
 const v=render(<PageView {...props(service,page)} tool="images" canvasImage={canvas} imageEditingActive/>);await waitFor(()=>expect(v.getByRole("button",{name:"Move selected image"})).toBeInTheDocument());
 expect(v.container.querySelector('svg')).toHaveAttribute('viewBox','120 20 150 100');const raster=v.container.querySelector('image[href="data:shared"]');expect(raster).toHaveAttribute('width','300');expect(raster).toHaveAttribute('height','200');expect(raster?.getAttribute('transform')).toBeTruthy();
 expect(service.renderPage).not.toHaveBeenCalled();expect(service.renderPagePlan).not.toHaveBeenCalled();expect(service.listExistingImages).not.toHaveBeenCalled();
 v.rerender(<PageView {...props(service,page)} tool="images" canvasImage={{...canvas,raster:{...canvas.raster,error:"Retry me",loading:false}}} imageEditingActive/>);fireEvent.click(v.getByRole("button",{name:"Retry preview"}));expect(canvas.raster.retry).toHaveBeenCalledOnce();
});

it("uses paragraph source-axis rasters with exactly one workspace rotation",()=>{
 const service=adapter(),page={...base,crop:{x:20,y:30,width:100,height:150}},canvas={pageId:page.id,sessionKey:"paragraph",bounds:{x:30,y:40,width:60,height:70},runs:[],selected:[],choosing:false,enabled:true,raster:{url:"data:paragraph",loading:false},onChange:vi.fn(),onGestureActive:vi.fn(),onToggle:vi.fn()};
 const v=render(<PageView {...props(service,page)} tool="edit" canvasParagraph={canvas} paragraphEditingActive interactionDisabled/>);
 const raster=v.container.querySelector('image[href="data:paragraph"]')!;
 expect(raster).toHaveAttribute("width","200");expect(raster).toHaveAttribute("height","300");expect(raster).not.toHaveAttribute("transform");expect(raster.parentElement).toHaveAttribute("transform","translate(300 0) rotate(90)");
 expect(service.renderPage).not.toHaveBeenCalled();expect(service.renderPagePlan).not.toHaveBeenCalled();expect(v.container.querySelectorAll("[data-overlay] image")).toHaveLength(0);expect(v.getByRole("button",{name:"Move paragraph area"})).toBeVisible();
});
