import {fromDisplayPoint,pageVisibleRect} from "./geometry";
import {imageMeasurements,imageTransform,type ImageMeasurements} from "./existing-image-transform";
import type {ExistingImage,ImageCrop,ImagePoint,PagePlan} from "./types";

export type ImageQuad=ExistingImage["corners"];
export interface CanvasImageView {
 pageId:string;sessionKey:string;image:ExistingImage;frame:ImageQuad;locked:boolean;enabled:boolean;
 mode?:"position"|"crop";fullFrame?:ImageQuad;crop?:ImageCrop;onCropChange?(value:ImageCrop):void;
 raster:{url?:string;error?:string;loading:boolean;retry?():void};
 onGestureActive(active:boolean):void;
 onChange(value:ImageMeasurements):void;
}
const coordinates=[[0,0],[1,0],[1,1],[0,1]] as const;
const center=(q:ImageQuad)=>({x:(q[0].x+q[2].x)/2,y:(q[0].y+q[2].y)/2});
const finite=(p:ImagePoint)=>Number.isFinite(p.x)&&Number.isFinite(p.y);
const minus=(a:ImagePoint,b:ImagePoint)=>({x:a.x-b.x,y:a.y-b.y});
const length=(p:ImagePoint)=>Math.hypot(p.x,p.y);
const clamp=(n:number,low:number,high:number)=>Math.max(low,Math.min(high,n));
export const frameMeasurements=(frame:ImageQuad,page:PagePlan)=>imageMeasurements({corners:frame},page);

export function imagePointerPoint(x:number,y:number,rect:Pick<DOMRect,"left"|"top"|"width"|"height">,page:PagePlan):ImagePoint|null {
 if(![x,y,rect.left,rect.top,rect.width,rect.height].every(Number.isFinite)||rect.width<=0||rect.height<=0)return null;
 const visible=pageVisibleRect(page);
 const p=fromDisplayPoint({x:visible.x+(x-rect.left)*visible.width/rect.width,y:visible.y+(y-rect.top)*visible.height/rect.height},page.width,page.height,page.rotation);
 return finite(p)?p:null;
}
function rotate(point:ImagePoint,angle:number):ImagePoint {const r=angle*Math.PI/180,c=Math.cos(r),s=Math.sin(r);return {x:c*point.x-s*point.y,y:s*point.x+c*point.y};}
function admitted(original:ImageQuad,current:ImageQuad,next:ImageQuad):ImageQuad {
 const delta=minus(center(next),center(original));
 return next.every(finite)&&Math.abs(delta.x)<=1e6&&Math.abs(delta.y)<=1e6?next:current;
}
export function frameForMeasurements(image:ExistingImage,page:PagePlan,value:ImageMeasurements):ImageQuad {
 imageTransform(imageMeasurements(image,page),value,page);
 return reconstructImageFrame(image,page,value);
}
/** Reconstruct a signed basis; the caller admits the final transform against its immutable source. */
export function reconstructImageFrame(image:ExistingImage,page:PagePlan,value:ImageMeasurements):ImageQuad {
 const original=imageMeasurements(image,page),q=image.corners;
 if(!Object.values(value).every(Number.isFinite)||value.width<=0||value.height<=0||original.width<=0||original.height<=0||Math.abs(value.rotation)>360)throw new Error("Enter a positive image size and an angle from −360° to 360°.");
 const a=fromDisplayPoint({x:original.centerX,y:original.centerY},page.width,page.height,page.rotation),b=fromDisplayPoint({x:value.centerX,y:value.centerY},page.width,page.height,page.rotation);
 const t={deltaX:b.x-a.x,deltaY:b.y-a.y,scaleX:value.width/original.width,scaleY:value.height/original.height,rotationDegrees:((value.rotation-original.rotation+180)%360+360)%360-180};
 if(t.deltaX===0&&t.deltaY===0&&t.scaleX===1&&t.scaleY===1&&t.rotationDegrees===0)return q;
 const c=center(q),u=rotate(minus(q[1],q[0]),t.rotationDegrees),v=rotate(minus(q[3],q[0]),t.rotationDegrees);
 return coordinates.map(([x,y])=>({x:c.x+t.deltaX+(x-.5)*u.x*t.scaleX+(y-.5)*v.x*t.scaleY,y:c.y+t.deltaY+(x-.5)*u.y*t.scaleX+(y-.5)*v.y*t.scaleY})) as ImageQuad;
}
export function moveImageFrame(original:ImageQuad,current:ImageQuad,delta:ImagePoint):ImageQuad {
 if(!finite(delta)||(delta.x===0&&delta.y===0))return current;
 const prior=minus(center(current),center(original));
 const dx=clamp(prior.x+delta.x,-1e6,1e6)-prior.x,dy=clamp(prior.y+delta.y,-1e6,1e6)-prior.y;
 return current.map(p=>({x:p.x+dx,y:p.y+dy})) as ImageQuad;
}
export function resizeImageFrame(original:ImageQuad,current:ImageQuad,corner:0|1|2|3,point:ImagePoint,locked:boolean):ImageQuad {
 if(!finite(point)||(point.x===current[corner].x&&point.y===current[corner].y))return current;
 const opposite=(corner+2)%4,fixed=current[opposite],u=minus(current[1],current[0]),v=minus(current[3],current[0]);
 const sx=coordinates[corner][0]-coordinates[opposite][0],sy=coordinates[corner][1]-coordinates[opposite][1];
 const x={x:u.x*sx,y:u.y*sx},y={x:v.x*sy,y:v.y*sy},d=minus(point,fixed),det=x.x*y.y-x.y*y.x;
 if(!Number.isFinite(det)||Math.abs(det)<length(x)*length(y)*1e-10)return current;
 const unitX=length(minus(original[1],original[0]))/length(u),unitY=length(minus(original[3],original[0]))/length(v);
 let a:number,b:number;
 if(locked){const diagonal={x:x.x+y.x,y:x.y+y.y};const scale=(d.x*diagonal.x+d.y*diagonal.y)/(diagonal.x**2+diagonal.y**2);if(!Number.isFinite(scale))return current;a=b=clamp(scale,Math.max(.001*unitX,.001*unitY),Math.min(1000*unitX,1000*unitY));}
 else {a=(d.x*y.y-d.y*y.x)/det;b=(x.x*d.y-x.y*d.x)/det;if(!Number.isFinite(a)||!Number.isFinite(b))return current;a=clamp(a,.001*unitX,1000*unitX);b=clamp(b,.001*unitY,1000*unitY);}
 const next=coordinates.map(([cx,cy])=>({x:fixed.x+(cx-coordinates[opposite][0])*u.x*a+(cy-coordinates[opposite][1])*v.x*b,y:fixed.y+(cx-coordinates[opposite][0])*u.y*a+(cy-coordinates[opposite][1])*v.y*b})) as ImageQuad;
 return admitted(original,current,next);
}
export function rotateImageFrame(frame:ImageQuad,start:ImagePoint,point:ImagePoint,page:PagePlan,snap:boolean):ImageQuad {
 if(!finite(start)||!finite(point))return frame;
 const c=center(frame),a=minus(start,c),b=minus(point,c);
 if(length(a)<1e-6||length(b)<1e-6)return frame;
 let angle=(Math.atan2(b.y,b.x)-Math.atan2(a.y,a.x))*180/Math.PI;
 if(snap){const original=frameMeasurements(frame,page).rotation;angle=Math.round((original+angle)/15)*15-original;}
 if(angle===0)return frame;
 return frame.map(p=>{const d=rotate(minus(p,c),angle);return {x:c.x+d.x,y:c.y+d.y};}) as ImageQuad;
}
