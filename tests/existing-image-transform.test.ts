import {expect,it} from "vitest";
import {imageMeasurements,imagePositionDraft,readImageMeasurements,imageTransform,resizeImageDraft} from "../src/editor/existing-image-transform";
import type {ExistingImage,PagePlan,Rotation} from "../src/editor/types";
const image:ExistingImage={objectPath:[2],selectionId:"a".repeat(64),corners:[{x:10,y:100},{x:110,y:100},{x:135,y:50},{x:35,y:50}],bounds:{x:10,y:50,width:125,height:50},pixelWidth:200,pixelHeight:100};
const page=(rotation:Rotation):PagePlan=>({id:"p",sourceId:"s",pageIndex:0,width:400,height:300,rotation,overlays:[]});
it("measures sheared image axes in displayed physical points, independent of workspace rotation",()=>{
 for(const rotation of [0,90,180,270] as const){const original=imageMeasurements(image,page(rotation));expect(original.width).toBe(100);expect(original.height).toBeCloseTo(Math.hypot(25,50));
  const transform=imageTransform(original,readImageMeasurements(imagePositionDraft(original),original),page(rotation));expect(transform).toEqual({deltaX:0,deltaY:0,scaleX:1,scaleY:1,rotationDegrees:0});
  const moved=imageTransform(original,{...original,centerX:original.centerX+10,width:200,rotation:original.rotation+30},page(rotation));
  expect([moved.deltaX,moved.deltaY]).toEqual(({0:[10,0],90:[0,-10],180:[-10,0],270:[0,10]})[rotation]);expect(moved.scaleX).toBe(2);expect(moved.scaleY).toBe(1);expect(moved.rotationDegrees).toBe(30);
 }
});
it("rejects empty, nonfinite and out-of-policy drafts and locks resized dimensions",()=>{
 const original=imageMeasurements(image,page(0)),draft=imagePositionDraft(original);
 for(const value of [""," ","NaN","Infinity"])expect(()=>readImageMeasurements({...draft,width:value},original)).toThrow();
 for(const value of [0,0.01,100001,Infinity,NaN])expect(()=>imageTransform(original,{...original,width:value},page(0))).toThrow();
 expect(()=>imageTransform(original,{...original,centerX:original.centerX+1000001},page(0))).toThrow();
 const locked=resizeImageDraft(draft,"width","200",original.width/original.height);expect(Number(locked.height)).toBeCloseTo(original.height*2);expect(locked.width).toBe("200");
 expect(resizeImageDraft(draft,"width","",original.width/original.height).height).toBe(draft.height);
});
