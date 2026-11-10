import { expect, it } from "vitest";
import { areaFromDrag, defaultTextArea, resizeArea, resizeAreaTo, moveArea, areaCorner } from "../src/editor/text-area-geometry";
import { rotateTextRect } from "../src/editor/text-overlay-geometry";
import type { Rotation, TextOverlay } from "../src/editor/types";

const text: TextOverlay = {type:"text",id:"area",x:250,y:250,text:"Keep\r\nthis text",fontId:"font",fontSize:18,color:"#123456",shaping:{version:1,direction:"ltr",ligatures:true},area:{version:1,width:140,height:100,inset:4,lineSpacing:1.2,alignment:"left"}};

it("creates rectangles in either drag direction, constrains them to the page and ignores clicks", () => {
 expect(areaFromDrag({x:200,y:160},{x:30,y:40},500,600)).toEqual({x:30,y:40,width:170,height:120});
 expect(areaFromDrag({x:-10,y:40},{x:800,y:900},500,600)).toEqual({x:0,y:40,width:500,height:560});
 expect(areaFromDrag({x:40,y:40},{x:45,y:50},500,600)).toBeNull();
 expect(areaFromDrag({x:NaN,y:0},{x:80,y:80},500,600)).toBeNull();
 expect(defaultTextArea(500,600)).toEqual({x:24,y:24,width:240,height:144});
 expect(defaultTextArea(25,25)).toBeNull();
});

it.each([0,90,180,270] as Rotation[])("resizes every corner at rotation %i without moving the opposite corner", rotation => {
 const original={...text,rotation};
 for (const corner of ["nw","ne","se","sw"] as const) {
  const opposite={nw:"se",ne:"sw",se:"nw",sw:"ne"} as const;
  const fixed=areaCorner(original,opposite[corner]);
  const requested=areaCorner({...original,area:{...original.area!,width:180,height:130}},corner);
  // Top/left corners also move: exercise all handles using a page-space delta.
  requested.x += corner.includes("w") ? -12 : 12;
  requested.y += corner.includes("n") ? -8 : 8;
  const resized=resizeArea(original,corner,requested,600,700);
  expect(areaCorner(resized,opposite[corner])).toEqual(fixed);
  expect(resized.text).toBe(original.text);expect(resized.fontSize).toBe(18);
  expect(original.area!.width).toBe(140);
 }
 const grown=resizeAreaTo(original,200,160,600,700);
 expect(grown.area).toMatchObject({width:200,height:160});
 expect({x:grown.x,y:grown.y}).toEqual({x:250,y:250});
});

it.each([0,90,180,270] as Rotation[])("keeps resized and moved areas within the page at rotation %i", rotation => {
 const original={...text,rotation};
 for(const corner of ["nw","ne","se","sw"] as const) for(const target of [{x:-1000,y:-1000},{x:2000,y:2000}]) {
  const resized=resizeArea(original,corner,target,600,700);
  const b=rotateTextRect(resized,{x:0,y:0,width:resized.area!.width,height:resized.area!.height});
  expect(b.x).toBeGreaterThanOrEqual(0);expect(b.y).toBeGreaterThanOrEqual(0);
  expect(b.x+b.width).toBeLessThanOrEqual(600);expect(b.y+b.height).toBeLessThanOrEqual(700);
  expect(resized.area!.width).toBeGreaterThanOrEqual(36);expect(resized.area!.height).toBeGreaterThanOrEqual(36);
 }
 const moved=moveArea(original,-1000,2000,600,700);
 const b=rotateTextRect(moved,{x:0,y:0,width:moved.area!.width,height:moved.area!.height});
 expect(b.x).toBe(0);expect(b.y+b.height).toBe(700);
 expect(resizeAreaTo(original,NaN,100,600,700)).toBe(original);
});

it("never shrinks inside the saved inset", () => {
 const padded={...text,area:{...text.area!,inset:30}};
 expect(resizeAreaTo(padded,1,1,600,700).area).toMatchObject({width:61,height:61});
});
