import {expect,it} from "vitest";
import {imageBounds,moveImage,fitImage,resizeImage,resizeImageTo,rotateImage,imageCorner} from "../src/editor/image-geometry";
import {screenArrowDelta,toDisplayPoint} from "../src/editor/geometry";
import type {ImageOverlay,Rotation} from "../src/editor/types";
const image:ImageOverlay={type:"image",id:"photo",assetId:"a".repeat(64),x:100,y:100,width:80,height:40,rotation:0};
it.each([0,90,180,270] as Rotation[])("keeps every corner inside the page when moving a %i degree image",rotation=>{
 for(const [x,y] of [[-999,-999],[999,999],[-999,999],[999,-999],[100,100]]) {
  const moved=moveImage({...image,rotation},x,y,200,300),box=imageBounds(moved);
  expect(box.x).toBeGreaterThanOrEqual(0);expect(box.y).toBeGreaterThanOrEqual(0);
  expect(box.x+box.width).toBeLessThanOrEqual(200);expect(box.y+box.height).toBeLessThanOrEqual(300);
  expect(moved.width).toBe(80);expect(moved.height).toBe(40);expect(moved.rotation).toBe(rotation);
 }
});
it("ignores nonfinite pointer coordinates",()=>{
 expect(moveImage(image,NaN,20,200,300)).toBe(image);expect(moveImage(image,20,Infinity,200,300)).toBe(image);
});
const corners=["nw","ne","se","sw"] as const;
const opposite={nw:"se",ne:"sw",se:"nw",sw:"ne"} as const;
function within(value:ImageOverlay,w=200,h=300){const box=imageBounds(value);expect(box.x).toBeGreaterThanOrEqual(-1e-8);expect(box.y).toBeGreaterThanOrEqual(-1e-8);expect(box.x+box.width).toBeLessThanOrEqual(w+1e-8);expect(box.y+box.height).toBeLessThanOrEqual(h+1e-8);expect(box.width).toBeGreaterThan(0);expect(box.height).toBeGreaterThan(0);}
it("fits displayed pixel dimensions without enlarging small images and centers or clamps placement",()=>{
 const large=fitImage("new",{id:image.assetId,width:4000,height:2000},200,300)!;within(large);expect(large.width/large.height).toBe(2);expect(large.width).toBe(120);expect(imageBounds(large)).toMatchObject({x:40,y:120});
 const small=fitImage("new",{id:image.assetId,width:40,height:20},200,300,{x:200,y:300})!;within(small);expect(small.width).toBe(30);expect(small.height).toBe(15);
 expect(fitImage("new",{id:image.assetId,width:NaN,height:20},200,300)).toBeNull();expect(fitImage("new",{id:image.assetId,width:20,height:20},0,300)).toBeNull();
 const thin=fitImage("new",{id:image.assetId,width:32768,height:1},144,144)!;within(thin,144,144);expect(thin.width/thin.height).toBeCloseTo(32768);
});
it.each([0,90,180,270] as Rotation[])("resizes every corner with a fixed opposite corner and optional aspect lock at %i degrees",rotation=>{
 const start={...image,rotation},box=imageBounds(start);
 for(const corner of corners)for(const locked of [false,true])for(const point of [{x:-1000,y:-1000},{x:1000,y:1000},{x:45,y:160}]){
  const resized=resizeImage(start,corner,point,200,300,locked);within(resized);
  expect(imageCorner(resized,opposite[corner])).toEqual(imageCorner(start,opposite[corner]));
  if(locked)expect(resized.width/resized.height).toBeCloseTo(start.width/start.height,8);
 }
 for(const corner of corners)expect(resizeImage(start,corner,imageCorner(start,corner),200,300,true)).toBe(start);
 expect(resizeImageTo(start,box.width,box.height,200,300,false,"width")).toBe(start);
 const resized=resizeImageTo(start,500,500,200,300,true,"width");within(resized);expect(imageBounds(resized).x).toBeCloseTo(box.x);expect(imageBounds(resized).y).toBeCloseTo(box.y);expect(resized.width/resized.height).toBeCloseTo(2);
 expect(resizeImageTo(start,0,40,200,300,false,"width")).toBe(start);expect(resizeImageTo(start,Infinity,40,200,300,false,"width")).toBe(start);
});
it("free numeric sizing changes the requested dimensions, while locked height preserves proportions",()=>{
 const free=resizeImageTo(image,50,70,200,300,false,"width");expect(imageBounds(free)).toEqual({x:100,y:100,width:50,height:70});
 const locked=resizeImageTo(image,80,20,200,300,true,"height");expect(imageBounds(locked)).toEqual({x:100,y:100,width:40,height:20});
});
it.each([0,90,180,270] as Rotation[])("rotates around the visual center, fitting the page without changing aspect ratio from %i",rotation=>{
 const start={...image,x:150,y:150,rotation},before=imageBounds(start);const rotated=rotateImage(start,1,400,400),after=imageBounds(rotated);
 expect(after.x+after.width/2).toBe(before.x+before.width/2);expect(after.y+after.height/2).toBe(before.y+before.height/2);expect(rotated.rotation).toBe((rotation+90)%360);
 expect(rotateImage(rotated,-1,400,400)).toEqual(start);
 const wide={...image,x:0,y:100,width:200,height:40,rotation:0 as const};const fitted=rotateImage(wide,1,200,144);within(fitted,200,144);expect(fitted.width/fitted.height).toBeCloseTo(5);
});
it.each([0,90,180,270] as Rotation[])("keyboard deltas follow screen axes on a %i degree page",rotation=>{
 const point={x:80,y:90},before=toDisplayPoint(point,200,300,rotation);
 for(const [key,x,y] of [["ArrowLeft",-10,0],["ArrowRight",10,0],["ArrowUp",0,-10],["ArrowDown",0,10]] as const){
  const delta=screenArrowDelta(key,true,rotation)!,after=toDisplayPoint({x:point.x+delta.x,y:point.y+delta.y},200,300,rotation);expect({x:after.x-before.x,y:after.y-before.y}).toEqual({x,y});
 }
 expect(screenArrowDelta("Enter",false,rotation)).toBeNull();
});
