import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { PageView, clearRenderCache } from "../src/components/PageView";
import { createDemoAdapter } from "../src/editor/adapter";
import { toDisplayPoint, displayDimensions } from "../src/editor/geometry";
import { areaCorner } from "../src/editor/text-area-geometry";
import { clearShapedTextCache, shapedTextApi } from "../src/editor/shaped-text";
import { fontApi } from "../src/editor/custom-fonts";
import type { PagePlan, Rotation, TextOverlay } from "../src/editor/types";

const area: TextOverlay={type:"text",id:"area",x:250,y:250,text:"original\r\ntext",fontSize:18,fontId:"area-gesture-font",color:"#123456",shaping:{version:1,direction:"ltr",ligatures:true},area:{version:1,width:140,height:100,inset:4,lineSpacing:1.2,alignment:"left"}};
const page: PagePlan={id:"area-page",sourceId:"area-source",pageIndex:0,width:600,height:700,rotation:0,overlays:[]};
function props(overrides:Partial<React.ComponentProps<typeof PageView>>={}) {
 return {adapter:createDemoAdapter(),page,pageNumber:1,zoom:200,tool:"area" as const,selectedOverlayId:null,pendingSignature:null,onSelectOverlay:vi.fn(),onAddText:vi.fn(),onAddTextArea:vi.fn(),onResizeTextArea:vi.fn(),onPlaceSignature:vi.fn(),onMoveOverlay:vi.fn(),...overrides};
}
function position(svg: SVGSVGElement, value:PagePlan) {
 const size=displayDimensions(value.width,value.height,value.rotation);
 vi.spyOn(svg,"getBoundingClientRect").mockReturnValue({left:10,top:20,width:size.width*2,height:size.height*2} as DOMRect);
 return (p:{x:number;y:number}, pointerId=1)=>{const v=toDisplayPoint(p,value.width,value.height,value.rotation);return {clientX:10+v.x*2,clientY:20+v.y*2,pointerId,button:0};};
}
beforeEach(()=>{
 vi.stubGlobal("PointerEvent",class extends MouseEvent {pointerId:number;constructor(type:string,init:PointerEventInit={}){super(type,init);this.pointerId=init.pointerId??1;}});
 Object.defineProperty(SVGElement.prototype,"setPointerCapture",{configurable:true,value:vi.fn()});
 Object.defineProperty(SVGElement.prototype,"releasePointerCapture",{configurable:true,value:vi.fn()});
 vi.spyOn(fontApi,"release").mockResolvedValue();
 vi.spyOn(shapedTextApi,"prepare").mockImplementation(async overlay=>({preview:{version:1,fontId:overlay.fontId!,text:overlay.text,width:overlay.area!.width,height:overlay.area!.height,rotation:0,color:[0,0,0],outlines:[],glyphs:[]},origin:[0,overlay.area!.height-overlay.fontSize],bounds:{x:0,y:0,width:overlay.area!.width,height:overlay.area!.height},characters:[]}));
});
afterEach(()=>{cleanup();clearRenderCache([page.sourceId]);clearShapedTextCache();vi.restoreAllMocks();vi.unstubAllGlobals();});

it.each([0,90,180,270] as Rotation[])("draws a source-coordinate area at page rotation %i and 200%% zoom",rotation=>{
 const value={...page,rotation};const options=props({page:value});const view=render(<PageView {...options}/>);
 const svg=view.container.querySelector("svg")!;const at=position(svg,value);
 fireEvent.pointerDown(svg,at({x:180,y:160}));fireEvent.pointerMove(svg,at({x:40,y:70}));
 expect(options.onAddTextArea).not.toHaveBeenCalled();
 expect(svg.querySelector(".text-area-draft")).toHaveAttribute("width","140");
 fireEvent.pointerUp(svg,at({x:40,y:70}));
 expect(options.onAddTextArea).toHaveBeenCalledOnce();expect(options.onAddTextArea).toHaveBeenCalledWith({x:40,y:70,width:140,height:90});
 expect(svg.querySelector(".text-area-draft")).toBeNull();
});

it.each(["escape","cancel","lost capture","tool","disabled","source"])("cancels unfinished creation on %s without changing the document",reason=>{
 const options=props();const view=render(<PageView {...options}/>);const svg=view.container.querySelector("svg")!;const at=position(svg,page);
 fireEvent.pointerDown(svg,at({x:40,y:70}));fireEvent.pointerMove(svg,at({x:180,y:160}));
 if(reason==="escape")fireEvent.keyDown(window,{key:"Escape"});
 else if(reason==="cancel")fireEvent.pointerCancel(svg,at({x:180,y:160}));
 else if(reason==="lost capture")fireEvent.lostPointerCapture(svg,at({x:180,y:160}));
 else view.rerender(<PageView {...options} {...(reason==="tool"?{tool:"select"}:reason==="disabled"?{interactionDisabled:true}:{page:{...page,sourceId:"replacement"}})}/>);
 fireEvent.pointerUp(svg,at({x:180,y:160}));
 expect(options.onAddTextArea).not.toHaveBeenCalled();expect(svg.querySelector(".text-area-draft")).toBeNull();
});

it("ignores a second pointer and accidental clicks",()=>{
 const options=props();const view=render(<PageView {...options}/>);const svg=view.container.querySelector("svg")!;const at=position(svg,page);
 fireEvent.pointerDown(svg,at({x:40,y:70}));fireEvent.pointerMove(svg,at({x:400,y:500},2));fireEvent.pointerUp(svg,at({x:400,y:500},2));
 expect(options.onAddTextArea).not.toHaveBeenCalled();fireEvent.pointerUp(svg,at({x:42,y:72}));expect(options.onAddTextArea).not.toHaveBeenCalled();
});

it.each([0,90,180,270] as Rotation[])("commits one resize and preserves text at text/page rotation %i",rotation=>{
 const text={...area,rotation};const value={...page,rotation,overlays:[text]};const options=props({page:value,tool:"select",selectedOverlayId:"area"});
 const view=render(<PageView {...options}/>);const svg=view.container.querySelector("svg")!;const at=position(svg,value);
 const handle=view.getByRole("button",{name:"Resize text area bottom right"});
 fireEvent.pointerDown(handle,at(areaCorner(text,"se")));
 const end=areaCorner({...text,area:{...text.area!,width:200,height:140}},"se");
 fireEvent.pointerMove(svg,at(end));expect(options.onResizeTextArea).not.toHaveBeenCalled();
 fireEvent.pointerUp(svg,at(end));
 expect(options.onResizeTextArea).toHaveBeenCalledOnce();expect(options.onResizeTextArea).toHaveBeenCalledWith("area",{x:250,y:250,width:200,height:140});
 expect(options.onMoveOverlay).not.toHaveBeenCalled();expect(text.area!.width).toBe(140);
});

it("cancels a resize and keeps keyboard handles from moving the overlay",()=>{
 const value={...page,overlays:[area]};const options=props({page:value,tool:"select",selectedOverlayId:"area"});
 const view=render(<PageView {...options}/>);const svg=view.container.querySelector("svg")!;const at=position(svg,value);const handle=view.getByRole("button",{name:"Resize text area bottom right"});
 fireEvent.pointerDown(handle,at(areaCorner(area,"se")));fireEvent.pointerMove(svg,at({x:450,y:400}));
 fireEvent.keyDown(window,{key:"Escape"});fireEvent.pointerUp(svg,at({x:450,y:400}));expect(options.onResizeTextArea).not.toHaveBeenCalled();
 fireEvent.keyDown(handle,{key:"ArrowRight",shiftKey:true});
 expect(options.onResizeTextArea).toHaveBeenCalledOnce();expect(options.onResizeTextArea).toHaveBeenCalledWith("area",{x:250,y:250,width:150,height:100});
 expect(options.onMoveOverlay).not.toHaveBeenCalled();
 view.rerender(<PageView {...options} interactionDisabled/>);
 expect(view.queryByRole("button",{name:"Resize text area bottom right"})).toBeNull();
});

it.each(["move","resize"])("keeps %s exclusive when a second pointer starts the other gesture",first=>{
 const options=props({page:{...page,overlays:[area]},tool:"select",selectedOverlayId:"area"});
 const view=render(<PageView {...options}/>);const svg=view.container.querySelector("svg")!;const at=position(svg,page);
 const handle=view.getByRole("button",{name:"Resize text area bottom right"});const body=svg.querySelector("[data-overlay]")!;
 if(first==="move") {
   fireEvent.pointerDown(body,at({x:260,y:260},1));fireEvent.pointerDown(handle,at(areaCorner(area,"se"),2));
   fireEvent.pointerUp(svg,at({x:270,y:270},1));fireEvent.pointerUp(svg,at({x:450,y:400},2));
   expect(options.onMoveOverlay).toHaveBeenCalledTimes(2);
   expect(options.onMoveOverlay).toHaveBeenNthCalledWith(1,"area",expect.any(Number),expect.any(Number),"start");
   expect(options.onMoveOverlay).toHaveBeenNthCalledWith(2,"area",0,0,"end");
   expect(options.onResizeTextArea).not.toHaveBeenCalled();
 } else {
   fireEvent.pointerDown(handle,at(areaCorner(area,"se"),1));fireEvent.pointerDown(body,at({x:260,y:260},2));
   fireEvent.pointerMove(svg,at({x:450,y:400},1));fireEvent.keyDown(handle,{key:"ArrowRight"});
   expect(options.onResizeTextArea).not.toHaveBeenCalled();
   fireEvent.pointerUp(svg,at({x:450,y:400},1));fireEvent.pointerUp(svg,at({x:270,y:270},2));
   expect(options.onMoveOverlay).not.toHaveBeenCalled();expect(options.onResizeTextArea).toHaveBeenCalledOnce();
 }
});
