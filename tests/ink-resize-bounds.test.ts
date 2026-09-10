import {describe,expect,it} from "vitest";
import {resizeInk} from "../src/editor/geometry";
import type {InkOverlay} from "../src/editor/types";
describe("resizing ink dragged past the page edge",()=>{
 it("moves a negative anchor onto the page while retaining proportions",()=>{
  const ink:InkOverlay={type:"ink",id:"signature",paths:[[{x:-10,y:-5},{x:190,y:95}]],color:"#000000",strokeWidth:2};
  const resized=resizeInk(ink,200,150,100);
  expect(resized.paths).toEqual([[{x:0,y:0},{x:150,y:75}]]);
  expect(ink.paths[0][0]).toEqual({x:-10,y:-5});
 });
 it("corrects an off-page anchor even when no size change is needed",()=>{
  const ink:InkOverlay={type:"ink",id:"signature",paths:[[{x:-10,y:-5},{x:90,y:45}]],color:"#000000",strokeWidth:2};
  expect(resizeInk(ink,100,200,100).paths).toEqual([[{x:0,y:0},{x:100,y:50}]]);
 });
});
