import {cleanup,fireEvent,render} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {ParagraphCanvasControls} from "../src/components/ParagraphCanvasControls";
import type {CanvasParagraphView} from "../src/editor/paragraph-canvas";
import type {PagePlan} from "../src/editor/types";
const page:PagePlan={id:"p",sourceId:"source",pageIndex:0,width:400,height:300,rotation:0,overlays:[]};
function view():CanvasParagraphView{return {pageId:"p",sessionKey:"session",bounds:{x:50,y:50,width:100,height:80},runs:[],selected:[],enabled:true,choosing:false,raster:{url:"page",loading:false},onChange:vi.fn(),onGestureActive:vi.fn(),onToggle:vi.fn()};}
const at=(x:number,y:number,pointerId=1)=>({clientX:x,clientY:y,pointerId,button:0,isPrimary:true});
function setup(value=view(),p=page){const v=render(<svg><ParagraphCanvasControls view={value} page={p} zoom={100}/></svg>);vi.spyOn(v.container.querySelector("svg")!,"getBoundingClientRect").mockReturnValue({left:0,top:0,width:400,height:300} as DOMRect);return {...v,value,root:v.container.querySelector('[data-paragraph-controls]')!};}
beforeEach(()=>{vi.stubGlobal("PointerEvent",class extends MouseEvent{pointerId:number;isPrimary:boolean;constructor(t:string,i:PointerEventInit){super(t,i);this.pointerId=i.pointerId??1;this.isPrimary=i.isPrimary??true;}});Object.defineProperty(SVGElement.prototype,"setPointerCapture",{configurable:true,value:vi.fn()});Object.defineProperty(SVGElement.prototype,"releasePointerCapture",{configurable:true,value:vi.fn()});});
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();});
it("commits only on release and retains handle grab offset",()=>{
 const v=setup();fireEvent.pointerDown(v.getByRole("button",{name:"Resize paragraph east"}),at(159,90));fireEvent.pointerMove(v.root,at(161,90));expect(v.value.onChange).not.toHaveBeenCalled();
 fireEvent.pointerMove(v.root,at(164,90));expect(v.value.onChange).not.toHaveBeenCalled();fireEvent.pointerUp(v.root,at(164,90));fireEvent.lostPointerCapture(v.root,at(164,90));
 expect(v.value.onChange).toHaveBeenCalledOnce();expect(v.value.onChange).toHaveBeenCalledWith({x:50,y:50,width:105,height:80});
});
it.each(["escape","cancel","lost","zoom","crop","disabled","unmount"])("cancels %s without committing",reason=>{
 const v=setup();fireEvent.pointerDown(v.getByRole("button",{name:"Move paragraph area"}),at(80,80));fireEvent.pointerMove(v.root,at(110,110));
 if(reason==="escape")fireEvent.keyDown(window,{key:"Escape"});else if(reason==="cancel")fireEvent.pointerCancel(v.root,at(110,110));else if(reason==="lost")fireEvent.lostPointerCapture(v.root,at(110,110));else if(reason==="unmount")v.unmount();else v.rerender(<svg><ParagraphCanvasControls view={reason==="disabled"?{...v.value,enabled:false}:v.value} page={reason==="crop"?{...page,crop:{x:0,y:0,width:200,height:200}}:page} zoom={reason==="zoom"?150:100}/></svg>);
 fireEvent.pointerUp(v.root,at(110,110));expect(v.value.onChange).not.toHaveBeenCalled();expect(v.value.onGestureActive).toHaveBeenLastCalledWith(false);
});
it("ignores short gestures and competing pointers",()=>{
 const v=setup(),body=v.getByRole("button",{name:"Move paragraph area"});fireEvent.pointerDown(body,at(80,80));fireEvent.pointerMove(v.root,at(130,130,2));fireEvent.pointerUp(v.root,at(130,130,2));fireEvent.pointerUp(v.root,at(81,81));expect(v.value.onChange).not.toHaveBeenCalled();
});
it("maps rotated keyboard movement and omits hidden handles",()=>{
 const v=setup(view(),{...page,rotation:90});fireEvent.keyDown(v.getByRole("button",{name:"Move paragraph area"}),{key:"ArrowRight",shiftKey:true});expect(v.value.onChange).toHaveBeenCalledWith({x:50,y:40,width:100,height:80});
 v.rerender(<svg><ParagraphCanvasControls view={v.value} page={{...page,crop:{x:80,y:70,width:20,height:20}}} zoom={100}/></svg>);expect(v.queryAllByRole("button",{name:/Resize paragraph/})).toHaveLength(0);
});

it("retains focused handles while native preview temporarily disables gestures",()=>{
 const v=setup(),handle=v.getByRole("button",{name:"Resize paragraph east"});handle.focus();expect(handle).toHaveFocus();
 v.rerender(<svg><ParagraphCanvasControls view={{...v.value,enabled:false}} page={page} zoom={100}/></svg>);expect(v.getByRole("button",{name:"Resize paragraph east"})).toBe(handle);expect(handle).toHaveFocus();fireEvent.keyDown(handle,{key:"ArrowRight"});expect(v.value.onChange).not.toHaveBeenCalled();
 v.rerender(<svg><ParagraphCanvasControls view={v.value} page={page} zoom={100}/></svg>);expect(handle).toHaveFocus();fireEvent.keyDown(handle,{key:"ArrowRight"});expect(v.value.onChange).toHaveBeenCalledWith({x:50,y:50,width:101,height:80});
});

it.each([
 [0,60,50,8,-6,200,160],[90,110,60,6,8,160,200],
 [180,140,110,-8,6,200,160],[270,50,140,-6,-8,160,200],
] as const)("maps pointer movement through rotation %s and an offset crop",(rotation,x,y,dx,dy,width,height)=>{
 const v=setup(view(),{...page,rotation,crop:{x:20,y:30,width:200,height:160}});
 vi.mocked(v.container.querySelector("svg")!.getBoundingClientRect).mockReturnValue({left:0,top:0,width,height} as DOMRect);
 fireEvent.pointerDown(v.getByRole("button",{name:"Move paragraph area"}),at(x,y));fireEvent.pointerMove(v.root,at(x+dx,y+dy));fireEvent.pointerUp(v.root,at(x+dx,y+dy));expect(v.value.onChange).toHaveBeenCalledWith({x:58,y:44,width:100,height:80});
});
