import {describe,it,expect} from "vitest";
import {changeParagraphArea} from "../src/editor/paragraph-canvas";
import {screenArrowDelta} from "../src/editor/geometry";

describe("paragraph canvas bounds",()=>{
 const area={x:20,y:30,width:100,height:80};
 it("moves within the whole source page",()=>{
  expect(changeParagraphArea(area,"move",{x:-50,y:500},300,400)).toEqual({x:0,y:320,width:100,height:80});
 });
 it("keeps opposite edges fixed for every handle",()=>{
  for(const handle of ["n","ne","e","se","s","sw","w","nw"] as const){
   const result=changeParagraphArea(area,handle,{x:7,y:11},300,400);
   expect(result.x).toBe(handle.includes("w")?27:20);
   expect(result.y).toBe(handle.includes("n")?41:30);
   expect(result.x+result.width).toBe(handle.includes("e")?127:120);
   expect(result.y+result.height).toBe(handle.includes("s")?121:110);
  }
 });
 it("clamps resize to page and prevents opposite edge crossing",()=>{
  expect(changeParagraphArea(area,"nw",{x:-1000,y:-1000},300,400)).toEqual({x:0,y:0,width:120,height:110});
  const crossed=changeParagraphArea(area,"nw",{x:1000,y:1000},300,400);
  expect(crossed.width).toBeCloseTo(.01);expect(crossed.height).toBeCloseTo(.01);
  expect(crossed.x+crossed.width).toBe(120);expect(crossed.y+crossed.height).toBe(110);
 });
 it("ignores invalid deltas and preserves identity when unchanged",()=>{
  expect(changeParagraphArea(area,"move",{x:NaN,y:0},300,400)).toBe(area);
  expect(changeParagraphArea(area,"n",{x:10,y:0},300,400)).toBe(area);
 });
 it("maps screen arrows before constraining source edges",()=>{
  expect(changeParagraphArea(area,"e",screenArrowDelta("ArrowDown",false,90)!,300,400).width).toBe(101);
  expect(changeParagraphArea(area,"n",screenArrowDelta("ArrowRight",true,90)!,300,400).y).toBe(20);
 });
});
