import {cleanup,fireEvent,render} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {ExistingImageCropControls} from "../src/components/ExistingImageCropControls";
import {cropFrame} from "../src/editor/existing-image-crop";
import type {CanvasImageView} from "../src/editor/existing-image-canvas";
import type {ExistingImage,PagePlan} from "../src/editor/types";
const page:PagePlan={id:"p",sourceId:"s",pageIndex:0,width:400,height:300,rotation:0,overlays:[]};
const full:ExistingImage["corners"]=[{x:50,y:150},{x:150,y:150},{x:150,y:50},{x:50,y:50}];
const crop={left:.2,top:.2,right:.8,bottom:.8};
function setup(){const view:CanvasImageView={pageId:"p",sessionKey:"s:a",image:{objectPath:[2],selectionId:"a".repeat(64),corners:full,bounds:{x:50,y:50,width:100,height:100},pixelWidth:20,pixelHeight:10},frame:cropFrame(full,crop),fullFrame:full,crop,mode:"crop",locked:false,enabled:true,raster:{loading:false},onGestureActive:vi.fn(),onChange:vi.fn(),onCropChange:vi.fn()};const v=render(<svg><ExistingImageCropControls view={view} page={page} zoom={100}/></svg>);vi.spyOn(v.container.querySelector("svg")!,"getBoundingClientRect").mockReturnValue({left:0,top:0,width:400,height:300} as DOMRect);return {...v,view,root:v.container.querySelector('[data-image-crop-controls]')!};}
const at=(x:number,y:number,pointerId=1)=>({clientX:x,clientY:y,pointerId,button:0,isPrimary:true});
beforeEach(()=>{vi.stubGlobal("PointerEvent",class extends MouseEvent{pointerId:number;isPrimary:boolean;constructor(type:string,init:PointerEventInit){super(type,init);this.pointerId=init.pointerId??1;this.isPrimary=init.isPrimary??true;}});Object.defineProperty(SVGElement.prototype,"setPointerCapture",{configurable:true,value:vi.fn()});Object.defineProperty(SVGElement.prototype,"releasePointerCapture",{configurable:true,value:vi.fn()});});
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();});
it("retains corner grab offset and publishes only at release",()=>{
 const v=setup();fireEvent.pointerDown(v.getByRole("button",{name:"Crop image corner 1"}),at(75,130));fireEvent.pointerMove(v.root,at(85,120));expect(v.view.onCropChange).not.toHaveBeenCalled();fireEvent.pointerUp(v.root,at(85,120));fireEvent.lostPointerCapture(v.root,at(85,120));
 expect(v.view.onCropChange).toHaveBeenCalledOnce();const next=vi.mocked(v.view.onCropChange!).mock.calls[0][0];expect(next.left).toBeCloseTo(.3,12);expect(next.bottom).toBeCloseTo(.7,12);expect(next.right).toBe(.8);expect(next.top).toBe(.2);
});
it.each(["escape","cancel","lost","zoom","source","disabled","unmount"])("cancels %s without altering the crop",reason=>{
 const v=setup();fireEvent.pointerDown(v.getByRole("button",{name:"Move crop window"}),at(100,100));fireEvent.pointerMove(v.root,at(110,110));
 if(reason==="escape")fireEvent.keyDown(window,{key:"Escape"});else if(reason==="cancel")fireEvent.pointerCancel(v.root,at(110,110));else if(reason==="lost")fireEvent.lostPointerCapture(v.root,at(110,110));else if(reason==="unmount")v.unmount();else v.rerender(<svg><ExistingImageCropControls view={reason==="source"?{...v.view,sessionKey:"other"}:reason==="disabled"?{...v.view,enabled:false}:v.view} page={page} zoom={reason==="zoom"?150:100}/></svg>);
 fireEvent.pointerUp(v.root,at(110,110));expect(v.view.onCropChange).not.toHaveBeenCalled();expect(v.view.onGestureActive).toHaveBeenLastCalledWith(false);
});
it("ignores other pointers, moves in screen direction and hides page-excluded handles",()=>{
 const v=setup(),body=v.getByRole("button",{name:"Move crop window"});fireEvent.pointerDown(body,at(100,100));fireEvent.pointerUp(v.root,at(110,110,2));expect(v.view.onCropChange).not.toHaveBeenCalled();fireEvent.pointerCancel(v.root,at(100,100));
 fireEvent.keyDown(body,{key:"ArrowRight",shiftKey:true});expect(v.view.onCropChange).toHaveBeenCalledWith(expect.objectContaining({left:expect.closeTo(.3,10),right:expect.closeTo(.9,10)}));
 v.rerender(<svg><ExistingImageCropControls view={v.view} page={{...page,crop:{x:90,y:90,width:20,height:20}}} zoom={100}/></svg>);expect(v.queryAllByRole("button",{name:/Crop image corner/})).toHaveLength(0);
});
