import {expect,it} from "vitest";
import {frameForMeasurements,frameMeasurements,imagePointerPoint,moveImageFrame,resizeImageFrame,rotateImageFrame,type ImageQuad} from "../src/editor/existing-image-canvas";
import {imageMeasurements,imageTransform} from "../src/editor/existing-image-transform";
import {pageVisibleRect,toDisplayPoint} from "../src/editor/geometry";
import type {ExistingImage,PagePlan,Rotation} from "../src/editor/types";
const frame:ImageQuad=[{x:160,y:170},{x:60,y:155},{x:85,y:105},{x:185,y:120}];
const image:ExistingImage={objectPath:[2],selectionId:"a".repeat(64),corners:frame,bounds:{x:60,y:105,width:125,height:65},pixelWidth:20,pixelHeight:10};
const page=(rotation:Rotation):PagePlan=>({id:"p",sourceId:"s",pageIndex:0,width:400,height:300,rotation,crop:{x:10,y:20,width:350,height:250},overlays:[]});
const near=(a:ImageQuad,b:ImageQuad)=>a.forEach((p,i)=>{expect(p.x).toBeCloseTo(b[i].x,8);expect(p.y).toBeCloseTo(b[i].y,8);});
it.each([0,90,180,270] as Rotation[])("maps cropped pointer geometry without clamping at rotation %i",rotation=>{
 const p=page(rotation),rect={left:120,top:70,width:700,height:500},visible=pageVisibleRect(p);
 for(const point of [{x:100,y:100},{x:-100,y:500}]){const d=toDisplayPoint(point,p.width,p.height,rotation);const x=rect.left+(d.x-visible.x)*rect.width/visible.width,y=rect.top+(d.y-visible.y)*rect.height/visible.height;
  expect(imagePointerPoint(x,y,rect,p)!.x).toBeCloseTo(point.x,8);expect(imagePointerPoint(x,y,rect,p)!.y).toBeCloseTo(point.y,8);
 }
 expect(imagePointerPoint(NaN,0,rect,p)).toBeNull();expect(imagePointerPoint(1,2,{...rect,width:0},p)).toBeNull();
 const initial=imageMeasurements(image,p);near(frameForMeasurements(image,p,initial),frame);
 const wanted={...initial,centerX:initial.centerX+12,centerY:initial.centerY-8,width:initial.width*.7,height:initial.height*1.2,rotation:initial.rotation+30};
 const measured=frameMeasurements(frameForMeasurements(image,p,wanted),p);expect(measured.centerX).toBeCloseTo(wanted.centerX,8);expect(measured.centerY).toBeCloseTo(wanted.centerY,8);expect(measured.width).toBeCloseTo(wanted.width,8);expect(measured.height).toBeCloseTo(wanted.height,8);
});
it("resizes every reflected/sheared corner independently with its opposite corner fixed",()=>{
 const u={x:frame[1].x-frame[0].x,y:frame[1].y-frame[0].y},v={x:frame[3].x-frame[0].x,y:frame[3].y-frame[0].y};const coords=[[0,0],[1,0],[1,1],[0,1]];
 for(const corner of [0,1,2,3] as const){const opposite=(corner+2)%4,fixed=frame[opposite],sx=coords[corner][0]-coords[opposite][0],sy=coords[corner][1]-coords[opposite][1];
  const target={x:fixed.x+sx*u.x*1.5+sy*v.x*.7,y:fixed.y+sx*u.y*1.5+sy*v.y*.7};const result=resizeImageFrame(frame,frame,corner,target,false);
  expect(result[opposite]).toEqual(fixed);expect(result[corner].x).toBeCloseTo(target.x,8);expect(result[corner].y).toBeCloseTo(target.y,8);
  const value=frameMeasurements(result,page(0)),before=frameMeasurements(frame,page(0));expect(value.width/before.width).toBeCloseTo(1.5,8);expect(value.height/before.height).toBeCloseTo(.7,8);
 }
});
it("locks aspect ratio, clamps crossings and aggregate scale, and limits movement",()=>{
 const fixed=frame[2],point={x:fixed.x+2*(frame[0].x-fixed.x),y:fixed.y+2*(frame[0].y-fixed.y)};
 const twice=resizeImageFrame(frame,frame,0,point,true);expect(twice[2]).toEqual(fixed);const original=frameMeasurements(frame,page(0));expect(frameMeasurements(twice,page(0)).width/original.width).toBeCloseTo(2);
 const crossed=resizeImageFrame(frame,twice,0,{x:fixed.x+1000,y:fixed.y-1000},true);const cross=frameMeasurements(crossed,page(0));expect(cross.width/original.width).toBeGreaterThanOrEqual(.001-1e-10);expect(cross.height/original.height).toBeGreaterThanOrEqual(.001-1e-10);
 const huge=resizeImageFrame(frame,twice,0,{x:1e10,y:1e10},false),large=frameMeasurements(huge,page(0));expect(large.width/original.width).toBeLessThanOrEqual(1000+1e-8);expect(large.height/original.height).toBeLessThanOrEqual(1000+1e-8);
 const moved=moveImageFrame(frame,frame,{x:10,y:-20});near(moved,frame.map(p=>({x:p.x+10,y:p.y-20})) as ImageQuad);near(moveImageFrame(frame,frame,{x:NaN,y:0}),frame);
 const outside=frameMeasurements(moveImageFrame(frame,moved,{x:1e10,y:-1e10}),page(0));expect(outside.centerX-original.centerX).toBeCloseTo(1e6);expect(outside.centerY-original.centerY).toBeCloseTo(-1e6);
});
it("rotates around the center, snaps displayed absolute angles and ignores center singularities",()=>{
 const p=page(90),initial=frameMeasurements(frame,p),center={x:(frame[0].x+frame[2].x)/2,y:(frame[0].y+frame[2].y)/2};
 const result=rotateImageFrame(frame,{x:center.x+100,y:center.y},{x:center.x,y:center.y+100},p,false),value=frameMeasurements(result,p);
 expect(value.centerX).toBeCloseTo(initial.centerX,8);expect(value.centerY).toBeCloseTo(initial.centerY,8);expect(value.width).toBeCloseTo(initial.width,8);
 const snap=frameMeasurements(rotateImageFrame(frame,{x:center.x+100,y:center.y},{x:center.x+100,y:center.y+30},p,true),p);expect(snap.rotation/15).toBeCloseTo(Math.round(snap.rotation/15),8);
 near(rotateImageFrame(frame,center,center,p,false),frame);
});

it.each([true,false])("accepts the exact controller round trip after crossing at minimum scale (locked=%s)",locked=>{
 const q:ImageQuad=[{x:20,y:100},{x:100,y:100},{x:100,y:20},{x:20,y:20}],p=page(0),initial=frameMeasurements(q,p);
 const next=resizeImageFrame(q,q,0,{x:110,y:10},locked),v=frameMeasurements(next,p);const t=imageTransform(initial,v,p);expect(t.scaleX).toBeGreaterThanOrEqual(.001);expect(t.scaleY).toBeGreaterThanOrEqual(.001);expect(t.scaleX).toBeCloseTo(.001,10);
 expect(()=>imageTransform(initial,{...initial,width:initial.width*.00099},p)).toThrow();
});
