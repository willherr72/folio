import {cleanup,render} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {ImageHandles} from "../src/components/ImageControls";
import {TextAreaHandles} from "../src/components/TextAreaControls";
import {defaultPageTextArea} from "../src/editor/text-area-geometry";
import {searchNavigationRect} from "../src/editor/search";
import type {PagePlan,Rotation} from "../src/editor/types";
const page:PagePlan={id:"p",sourceId:"s",pageIndex:0,width:300,height:400,rotation:0,overlays:[],crop:{x:100,y:100,width:150,height:150}};
afterEach(cleanup);
it("places a default text area inside a crop far from the source origin",()=>{
  const crop={x:20,y:250,width:200,height:100};const box=defaultPageTextArea({...page,crop});
  expect(box).not.toBeNull();expect(box!.x).toBeGreaterThanOrEqual(crop.x);expect(box!.y).toBeGreaterThanOrEqual(crop.y);
  expect(box!.x+box!.width).toBeLessThanOrEqual(crop.x+crop.width);expect(box!.y+box!.height).toBeLessThanOrEqual(crop.y+crop.height);
  expect(defaultPageTextArea({...page,crop:{x:0,y:0,width:1,height:1}})).toBeNull();
});
it("does not focus image handles whose padding alone intersects the crop",()=>{
  const image={id:"i",type:"image" as const,assetId:"asset",x:92,y:120,width:100,height:100,rotation:0 as const};
  const view=render(<svg><ImageHandles page={page} image={image} zoom={100} locked onStart={vi.fn()} onChange={vi.fn()}/></svg>);
  expect(view.queryAllByRole("button",{name:"Resize image top left"}).length).toBe(0);expect(view.getByRole("button",{name:"Resize image top right"})).toBeTruthy();
});
it("does not focus text-area handles whose padding alone intersects the crop",()=>{
  const overlay={id:"t",type:"text" as const,x:92,y:120,fontSize:12,text:"Text",color:"#000000",area:{version:1 as const,width:100,height:100,inset:4,lineSpacing:1.2,alignment:"left" as const}};
  const view=render(<svg><TextAreaHandles page={page} overlay={overlay} zoom={100} onStart={vi.fn()}/></svg>);
  expect(view.queryAllByRole("button",{name:"Resize text area top left"}).length).toBe(0);expect(view.getByRole("button",{name:"Resize text area top right"})).toBeTruthy();
});
it.each([0,90,180,270] as Rotation[])("navigates to the retained intersection of a multi-rectangle match at rotation %s",rotation=>{
  const p={...page,width:400,height:1200,rotation,crop:{x:100,y:100,width:200,height:1000}};
  const match={id:"m",pageId:"p",rects:[{x:20,y:20,width:40,height:12},{x:90,y:900,width:60,height:12}]};
  expect(searchNavigationRect(p,match)).toEqual({x:100,y:900,width:50,height:12});
  expect(match.rects[0].x).toBe(20);
});
