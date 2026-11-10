import {describe,it,expect} from "vitest";
import {cropFrame,fullFrameFromVisible,imagePointToUnit,cropCorner,cropMove,validateCrop,fullCrop} from "../src/editor/existing-image-crop";
import {validateExistingImages,visibleImagePolygon} from "../src/editor/existing-images";
import type {ExistingImage,PagePlan} from "../src/editor/types";
const q:ExistingImage["corners"]=[{x:160,y:30},{x:60,y:45},{x:85,y:95},{x:185,y:80}];
const crop={left:.2,top:.1,right:.8,bottom:.7};
const near=(a:typeof q,b:typeof q)=>a.forEach((p,i)=>{expect(p.x).toBeCloseTo(b[i].x,10);expect(p.y).toBeCloseTo(b[i].y,10);});
describe("reversible affine image crop",()=>{
 it("maps signed local edges and reconstructs the full frame",()=>{
  const visible=cropFrame(q,crop);
  near(visible,[{x:147.5,y:48},{x:87.5,y:57},{x:102.5,y:87},{x:162.5,y:78}]);
  near(fullFrameFromVisible(visible,crop),q);
  expect(imagePointToUnit(q,visible[0])).toEqual({x:.2,y:.7});
  for(const turn of [0,90,180,270]) {
   const rad=turn*Math.PI/180;
   const rotated=q.map(p=>({x:p.x*Math.cos(rad)-p.y*Math.sin(rad),y:p.x*Math.sin(rad)+p.y*Math.cos(rad)})) as typeof q;
   near(fullFrameFromVisible(cropFrame(rotated,crop),crop),rotated);
  }
 });
 it("clamps corners against their opposite corner and window against full bounds",()=>{
  const next=cropCorner(q,crop,0,q[2]);
  expect(next.right).toBe(crop.right);expect(next.top).toBe(crop.top);
  expect(next.right-next.left).toBeCloseTo(.001,12);
  expect(next.bottom-next.top).toBeCloseTo(.001,12);
  expect(()=>validateCrop(next)).not.toThrow();
  const move=cropMove(q,crop,{x:1e6,y:1e6});
  expect(move.right-move.left).toBeCloseTo(.6,12);
  expect(move.bottom-move.top).toBeCloseTo(.6,12);
  expect(()=>validateCrop(move)).not.toThrow();
  expect(cropMove(q,crop,{x:0,y:0})).toBe(crop);
 });
 it("rejects malformed crops without admitting a materially undersized crop",()=>{
  for(const bad of [{...crop,left:NaN},{...crop,right:1.001},{...crop,bottom:.1009},{...crop,left:.9},null])expect(()=>validateCrop(bad)).toThrow();
  expect(()=>validateCrop({...fullCrop,left:.999})).not.toThrow();
 });
 it("discovers only the visible crop and clones crop metadata",()=>{
  const image:ExistingImage={objectPath:[2],selectionId:"a".repeat(64),corners:[{x:0,y:100},{x:100,y:100},{x:100,y:0},{x:0,y:0}],bounds:{x:50,y:0,width:50,height:100},pixelWidth:10,pixelHeight:10,crop:{...fullCrop,left:.5}};
  const page={width:100,height:100,rotation:0,crop:{x:0,y:0,width:40,height:100}} as PagePlan;
  expect(visibleImagePolygon(image,page)).toEqual([]);
  const [copy]=validateExistingImages({images:[image]});
  expect(copy.crop).toEqual(image.crop);expect(copy.crop).not.toBe(image.crop);
  expect(()=>validateExistingImages({images:[{...image,crop:{...fullCrop,bottom:Infinity}}]})).toThrow();
 });
});
