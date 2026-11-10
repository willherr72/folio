import {afterEach,expect,it,vi} from "vitest";
import {cleanup,fireEvent,render,waitFor} from "@testing-library/react";
import {PageView,clearRenderCache} from "../src/components/PageView";
import {createDemoAdapter} from "../src/editor/adapter";
import {findPageMatches} from "../src/editor/search";
import type {PagePlan} from "../src/editor/types";

const page:PagePlan={id:"crop",sourceId:"crop-source",pageIndex:0,width:300,height:400,rotation:90,crop:{x:20,y:30,width:100,height:150},overlays:[
  {id:"hidden",type:"text",x:240,y:320,text:"Hidden",fontSize:12,color:"#000000"},
  {id:"visible",type:"text",x:40,y:70,text:"Visible",fontSize:12,color:"#000000"},
]};
const chars=[{text:"Hidden",x:240,y:320,width:35,height:12},{text:"Visible",x:40,y:70,width:35,height:12}];
function options(){return {adapter:{...createDemoAdapter(),getPageText:async()=>({characters:chars})},page,pageNumber:1,zoom:100,tool:"select" as const,selectedOverlayId:null,pendingSignature:null,onSelectOverlay:vi.fn(),onAddText:vi.fn(),onPlaceSignature:vi.fn(),onMoveOverlay:vi.fn()};}
afterEach(()=>{cleanup();clearRenderCache([page.sourceId]);window.getSelection()?.removeAllRanges();vi.restoreAllMocks();vi.unstubAllGlobals();});
it("shows only the retained frame and maps a click into full source coordinates",async()=>{
  vi.stubGlobal("PointerEvent",MouseEvent);const p=options();const view=render(<PageView {...p} tool="text"/>);
  const canvas=view.container.querySelector("svg")!;
  expect(canvas.getAttribute("viewBox")).toBe("220 20 150 100");
  vi.spyOn(canvas,"getBoundingClientRect").mockReturnValue({left:10,top:20,width:300,height:200} as DOMRect);
  fireEvent.pointerDown(canvas,{clientX:160,clientY:120,button:0});
  expect(p.onAddText).toHaveBeenCalledWith({x:70,y:105});
});
it("does not focus or copy objects hidden by the crop",async()=>{
  const view=render(<PageView {...options()}/>);
  await waitFor(()=>expect(view.container.querySelector('[data-pdf-character]')).not.toBeNull());
  expect(view.queryByRole("button",{name:"Text: Hidden"})).toBeNull();
  expect(view.getByRole("button",{name:"Text: Visible"})).toBeTruthy();
  const range=document.createRange();range.selectNodeContents(view.getByLabelText("Page 1 text"));window.getSelection()!.addRange(range);
  const setData=vi.fn();fireEvent.copy(document,{clipboardData:{setData}});
  expect(setData).toHaveBeenCalledWith("text/plain","Visible");
});
it("does not navigate to source or overlay matches entirely outside the crop",()=>{
  expect(findPageMatches(page,{characters:chars},"hidden")).toEqual([]);
  expect(findPageMatches(page,{characters:chars},"visible")).toHaveLength(2);
});
it("uses a full source raster for a clipped rotated canvas",async()=>{
  const p=options();const renderPagePlan=vi.fn(async(_page:PagePlan,_width:number)=>"data:image/png;base64,AA");
  const view=render(<PageView {...p} adapter={{...p.adapter,kind:"native",renderPagePlan}}/>);
  await waitFor(()=>expect(renderPagePlan).toHaveBeenCalled());
  expect(renderPagePlan.mock.calls[0][0]).toMatchObject({rotation:90});
  expect((renderPagePlan.mock.calls[0] as unknown[])[0]).not.toHaveProperty("crop",page.crop);
  await waitFor(()=>expect(view.container.querySelector("image")?.getAttribute("width")).toBe("400"));
});
