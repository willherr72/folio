import {cleanup,fireEvent,render} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {ExistingImageControls} from "../src/components/ExistingImageControls";
import {frameMeasurements,type CanvasImageView} from "../src/editor/existing-image-canvas";
import type {ExistingImage,PagePlan} from "../src/editor/types";
const page:PagePlan={id:"p",sourceId:"source",pageIndex:0,width:400,height:300,rotation:0,overlays:[]};
const image:ExistingImage={objectPath:[2],selectionId:"a".repeat(64),corners:[{x:50,y:150},{x:150,y:150},{x:175,y:100},{x:75,y:100}],bounds:{x:50,y:100,width:125,height:50},pixelWidth:20,pixelHeight:10};
function view():CanvasImageView{return {pageId:"p",sessionKey:"source:a",image,frame:image.corners,locked:true,enabled:true,raster:{url:"image",loading:false},onGestureActive:vi.fn(),onChange:vi.fn()};}
function setup(value=view(),p=page){const v=render(<svg><ExistingImageControls view={value} page={p} zoom={100}/></svg>),svg=v.container.querySelector("svg")!;vi.spyOn(svg,"getBoundingClientRect").mockReturnValue({x:0,y:0,left:0,top:0,width:p.rotation%180?300:400,height:p.rotation%180?400:300,right:400,bottom:300,toJSON:()=>({})});return {...v,value,svg,root:v.container.querySelector('[data-existing-image-controls]')!};}
const at=(x:number,y:number,pointerId=1)=>({clientX:x,clientY:y,pointerId,button:0,isPrimary:true});
beforeEach(()=>{vi.stubGlobal("PointerEvent",class extends MouseEvent{pointerId:number;isPrimary:boolean;constructor(type:string,init:PointerEventInit){super(type,init);this.pointerId=init.pointerId??1;this.isPrimary=init.isPrimary??true;}});Object.defineProperty(SVGElement.prototype,"setPointerCapture",{configurable:true,value:vi.fn()});Object.defineProperty(SVGElement.prototype,"releasePointerCapture",{configurable:true,value:vi.fn()});});
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();});
it("moves a local frame while dragging, then completes exactly once",()=>{
 const v=setup();fireEvent.pointerDown(v.getByRole("button",{name:"Move selected image"}),at(100,125));fireEvent.pointerMove(v.root,at(120,135));expect(v.value.onChange).not.toHaveBeenCalled();expect(v.value.onGestureActive).toHaveBeenLastCalledWith(true);
 expect(v.container.querySelector('[data-image-draft]')?.getAttribute("points")).toContain("70,160");fireEvent.pointerUp(v.root,at(120,135));fireEvent.lostPointerCapture(v.root,at(120,135));
 expect(v.value.onChange).toHaveBeenCalledOnce();expect(v.value.onChange).toHaveBeenCalledWith(expect.objectContaining({centerX:132.5,centerY:135}));expect(v.value.onGestureActive).toHaveBeenLastCalledWith(false);
});
it.each(["escape","cancel","lost","zoom","crop","source","disabled","unmount"])("cancels on %s without publishing",reason=>{
 const v=setup();fireEvent.pointerDown(v.getByRole("button",{name:"Resize image corner 1"}),at(50,150));fireEvent.pointerMove(v.root,at(25,175));
 if(reason==="escape")fireEvent.keyDown(window,{key:"Escape"});else if(reason==="cancel")fireEvent.pointerCancel(v.root,at(25,175));else if(reason==="lost")fireEvent.lostPointerCapture(v.root,at(25,175));else if(reason==="unmount")v.unmount();else v.rerender(<svg><ExistingImageControls view={reason==="disabled"?{...v.value,enabled:false}:reason==="source"?{...v.value,sessionKey:"other"}:v.value} page={reason==="crop"?{...page,crop:{x:30,y:30,width:200,height:200}}:page} zoom={reason==="zoom"?150:100}/></svg>);
 fireEvent.pointerUp(v.root,at(25,175));expect(v.value.onChange).not.toHaveBeenCalled();expect(v.value.onGestureActive).toHaveBeenLastCalledWith(false);
});
it("ignores competing pointers and supports screen-direction keyboard movement and rotation",()=>{
 const v=setup(view(),{...page,rotation:90});const body=v.getByRole("button",{name:"Move selected image"});fireEvent.pointerDown(body,at(175,100));fireEvent.pointerMove(v.root,at(180,110,2));fireEvent.pointerUp(v.root,at(180,110,2));expect(v.value.onChange).not.toHaveBeenCalled();fireEvent.pointerCancel(v.root,at(175,100));
 fireEvent.keyDown(body,{key:"ArrowRight",shiftKey:true});const original=frameMeasurements(image.corners,{...page,rotation:90});expect(v.value.onChange).toHaveBeenLastCalledWith(expect.objectContaining({centerX:original.centerX+10,centerY:original.centerY}));
 fireEvent.keyDown(v.getByRole("button",{name:"Rotate selected image"}),{key:"ArrowRight",shiftKey:true});expect(v.value.onChange).toHaveBeenLastCalledWith(expect.objectContaining({rotation:105}));
 const count=vi.mocked(v.value.onChange).mock.calls.length;fireEvent.keyDown(body,{key:"ArrowLeft",ctrlKey:true});expect(v.value.onChange).toHaveBeenCalledTimes(count);
});
it("omits handles outside the visible crop and disables comparison targets",()=>{
 const v=setup(view(),{...page,crop:{x:80,y:110,width:50,height:20}});expect(v.queryAllByRole("button",{name:/Resize image corner/})).toHaveLength(0);expect(v.queryByRole("button",{name:"Rotate selected image"})).toBeNull();
 v.rerender(<svg><ExistingImageControls view={{...v.value,enabled:false}} page={page} zoom={100}/></svg>);expect(v.getByRole("button",{name:"Move selected image"})).toHaveAttribute("tabindex","-1");expect(v.queryAllByRole("button",{name:/Resize image corner/})).toHaveLength(0);
});

it("preserves the resize-handle grab offset instead of jumping to the pointer",()=>{
 const q:ExistingImage["corners"]=[{x:50,y:150},{x:150,y:150},{x:150,y:50},{x:50,y:50}],value=view();value.image={...image,corners:q};value.frame=q;value.locked=false;const v=setup(value);
 fireEvent.pointerDown(v.getByRole("button",{name:"Resize image corner 1"}),at(60,150));fireEvent.pointerMove(v.root,at(64,150));fireEvent.pointerUp(v.root,at(64,150));expect(value.onChange).toHaveBeenCalledWith(expect.objectContaining({centerX:102,width:96,height:100}));
});
