import {cleanup,fireEvent,render,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {PageView,clearRenderCache} from "../src/components/PageView";
import {createDemoAdapter} from "../src/editor/adapter";
import {displayDimensions,toDisplayPoint} from "../src/editor/geometry";
import {imageBounds,imageCorner} from "../src/editor/image-geometry";
import type {ImageOverlay,PagePlan,Rotation} from "../src/editor/types";
const image:ImageOverlay={type:"image",id:"photo",assetId:"a".repeat(64),x:250,y:250,width:100,height:60,rotation:0};
const page:PagePlan={id:"page",sourceId:"source",pageIndex:0,width:600,height:700,rotation:0,overlays:[image]};
function props(value=page){return {page:value,adapter:{...createDemoAdapter(),kind:"native" as const,renderPagePlan:vi.fn().mockResolvedValue("blob:preview")},pageNumber:1,zoom:200,tool:"select" as const,selectedOverlayId:image.id,pendingSignature:null,onSelectOverlay:vi.fn(),onAddText:vi.fn(),onPlaceSignature:vi.fn(),onMoveOverlay:vi.fn(),onTransformImage:vi.fn(),imageAspectLocked:true};}
function position(svg:SVGSVGElement,value:PagePlan){const size=displayDimensions(value.width,value.height,value.rotation);vi.spyOn(svg,"getBoundingClientRect").mockReturnValue({left:10,top:20,width:size.width*2,height:size.height*2} as DOMRect);return (point:{x:number;y:number},pointerId=1)=>{const p=toDisplayPoint(point,value.width,value.height,value.rotation);return {clientX:10+p.x*2,clientY:20+p.y*2,pointerId,button:0};};}
beforeEach(()=>{vi.stubGlobal("PointerEvent",class extends MouseEvent{pointerId:number;constructor(type:string,init:PointerEventInit={}){super(type,init);this.pointerId=init.pointerId??1;}});Object.defineProperty(SVGElement.prototype,"setPointerCapture",{configurable:true,value:vi.fn()});Object.defineProperty(SVGElement.prototype,"releasePointerCapture",{configurable:true,value:vi.fn()});Object.defineProperty(URL,"revokeObjectURL",{configurable:true,value:vi.fn()});});
afterEach(()=>{cleanup();clearRenderCache([page.sourceId]);vi.restoreAllMocks();vi.unstubAllGlobals();});
it.each([0,90,180,270] as Rotation[])("resizes a rotated image at page rotation %i with one commit and a native draft",async rotation=>{
 const original={...image,rotation},value={...page,rotation,overlays:[original]},options=props(value);const view=render(<PageView {...options}/>);const svg=view.container.querySelector("svg")!,at=position(svg,value);
 const handle=view.container.querySelector('[data-image-corner="se"]')!;const box=imageBounds(original),end={x:box.x+box.width*1.5,y:box.y+box.height*1.5};
 fireEvent.pointerDown(handle,at(imageCorner(original,"se")));fireEvent.pointerMove(svg,at(end));expect(options.onTransformImage).not.toHaveBeenCalled();
 await waitFor(()=>expect(options.adapter.renderPagePlan.mock.calls.at(-1)?.[0].overlays[0].width).toBe(150));
 fireEvent.pointerUp(svg,at(end));expect(options.onTransformImage).toHaveBeenCalledOnce();expect(options.onTransformImage).toHaveBeenCalledWith(original,expect.objectContaining({width:150,height:90,rotation}));expect(options.onMoveOverlay).not.toHaveBeenCalled();
});
it.each(["escape","cancel","lost capture","source","zoom","tool","disabled","selection"])("cancels an image resize on %s without committing",reason=>{
 const options=props(),view=render(<PageView {...options}/>),svg=view.container.querySelector("svg")!,at=position(svg,page);
 fireEvent.pointerDown(view.getByRole("button",{name:"Resize image bottom right"}),at(imageCorner(image,"se")));fireEvent.pointerMove(svg,at({x:400,y:340}));
 if(reason==="escape")fireEvent.keyDown(window,{key:"Escape"});else if(reason==="cancel")fireEvent.pointerCancel(svg,at({x:400,y:340}));else if(reason==="lost capture")fireEvent.lostPointerCapture(svg,at({x:400,y:340}));
 else view.rerender(<PageView {...options} {...(reason==="source"?{page:{...page,sourceId:"new"}}:reason==="zoom"?{zoom:100}:reason==="tool"?{tool:"draw"}:reason==="selection"?{selectedOverlayId:null}:{interactionDisabled:true})}/>);
 fireEvent.pointerUp(svg,at({x:400,y:340}));expect(options.onTransformImage).not.toHaveBeenCalled();
});
it("keeps moves local, ignores competing pointers, and cancels without changing history",()=>{
 const options=props(),view=render(<PageView {...options}/>),svg=view.container.querySelector("svg")!,at=position(svg,page),body=view.getByRole("button",{name:"Image"});
 fireEvent.pointerDown(body,at({x:260,y:260}));fireEvent.pointerDown(view.getByRole("button",{name:"Resize image bottom right"}),at(imageCorner(image,"se"),2));fireEvent.pointerMove(svg,at({x:400,y:340}));fireEvent.pointerUp(svg,at({x:400,y:340},2));
 expect(options.onTransformImage).not.toHaveBeenCalled();fireEvent.keyDown(window,{key:"Escape"});fireEvent.pointerUp(svg,at({x:400,y:340}));expect(options.onTransformImage).not.toHaveBeenCalled();expect(options.onMoveOverlay).not.toHaveBeenCalled();
 fireEvent.pointerDown(body,at({x:260,y:260}));fireEvent.pointerUp(svg,at({x:280,y:290}));expect(options.onTransformImage).toHaveBeenCalledWith(image,expect.objectContaining({x:270,y:280}));
});
it("supports screen-direction keyboard movement and free resizing with visible focus targets",()=>{
 const options=props({...page,rotation:90}),view=render(<PageView {...options} imageAspectLocked={false}/>);
 const body=view.getByRole("button",{name:"Image"});fireEvent.keyDown(body,{key:"ArrowRight",shiftKey:true});expect(options.onTransformImage).toHaveBeenLastCalledWith(image,expect.objectContaining({x:250,y:240}));
 const handle=view.getByRole("button",{name:"Resize image bottom left"});handle.focus();expect(handle).toHaveFocus();fireEvent.keyDown(handle,{key:"ArrowLeft",shiftKey:true});expect(options.onTransformImage).toHaveBeenLastCalledWith(image,expect.objectContaining({width:100,height:70}));
});
