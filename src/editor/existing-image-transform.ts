import {fromDisplayPoint,toDisplayPoint} from "./geometry";
import type {ExistingImage,ImageTransform,PagePlan} from "./types";

export interface ImageMeasurements {centerX:number;centerY:number;width:number;height:number;rotation:number}
export type ImagePositionDraft = Record<keyof ImageMeasurements,string>;
const angle=(v:number)=>((v+180)%360+360)%360-180;
const display=(v:number)=>Number(v.toFixed(3)).toString();

export function imageMeasurements(image:Pick<ExistingImage,"corners">,page:PagePlan):ImageMeasurements {
 const p=image.corners.map(v=>toDisplayPoint(v,page.width,page.height,page.rotation));
 return {centerX:(p[0].x+p[2].x)/2,centerY:(p[0].y+p[2].y)/2,
  width:Math.hypot(p[1].x-p[0].x,p[1].y-p[0].y),height:Math.hypot(p[3].x-p[0].x,p[3].y-p[0].y),
  rotation:angle(Math.atan2(p[1].y-p[0].y,p[1].x-p[0].x)*180/Math.PI)};
}
export function imagePositionDraft(value:ImageMeasurements):ImagePositionDraft {
 return {centerX:display(value.centerX),centerY:display(value.centerY),width:display(value.width),height:display(value.height),rotation:display(value.rotation)};
}
export function readImageMeasurements(draft:ImagePositionDraft,original:ImageMeasurements):ImageMeasurements {
 const value={...original};
 for(const key of Object.keys(value) as (keyof ImageMeasurements)[]){
  if(!draft[key].trim()||!Number.isFinite(Number(draft[key])))throw new Error("Enter a valid number in every position field.");
  // Display rounded measurements without rounding the unchanged PDF geometry.
  value[key]=draft[key]===display(original[key])?original[key]:Number(draft[key]);
 }
 return value;
}
export function imageTransform(original:ImageMeasurements,draft:ImageMeasurements,page:PagePlan):ImageTransform {
 const a=fromDisplayPoint({x:original.centerX,y:original.centerY},page.width,page.height,page.rotation);
 const b=fromDisplayPoint({x:draft.centerX,y:draft.centerY},page.width,page.height,page.rotation);
 // Affine corners are subtracted and measured again after a canvas gesture.
 // Normalize only boundary-sized arithmetic error, in physical point units,
 // before sending the exact allowed endpoint to the native service.
 const epsilon=32*Number.EPSILON*Math.max(1,page.width,page.height,...Object.values(a).map(Math.abs),...Object.values(b).map(Math.abs),original.width,original.height,draft.width,draft.height);
 const endpoint=(v:number,low:number,high:number,tolerance:number)=>!Number.isFinite(v)||!Number.isFinite(tolerance)?v:v<low&&low-v<=tolerance?low:v>high&&v-high<=tolerance?high:v;
 const result={deltaX:endpoint(b.x-a.x,-1e6,1e6,epsilon),deltaY:endpoint(b.y-a.y,-1e6,1e6,epsilon),scaleX:endpoint(draft.width/original.width,.001,1000,epsilon/original.width),scaleY:endpoint(draft.height/original.height,.001,1000,epsilon/original.height),rotationDegrees:angle(draft.rotation-original.rotation)};
 if(draft.width<=0||draft.height<=0||original.width<=0||original.height<=0||Object.values(result).some(v=>!Number.isFinite(v))||Math.abs(result.deltaX)>1e6||Math.abs(result.deltaY)>1e6||result.scaleX<0.001||result.scaleX>1000||result.scaleY<0.001||result.scaleY>1000||Math.abs(draft.rotation)>360)throw new Error("Use a positive size between 0.1% and 1000× the original, an angle from −360° to 360°, and a move within 1,000,000 pt.");
 return result;
}
export function resizeImageDraft(draft:ImagePositionDraft,key:"width"|"height",text:string,ratio:number|null):ImagePositionDraft {
 const next={...draft,[key]:text},size=Number(text);
 if(ratio&&text.trim()&&Number.isFinite(size)&&size>0){if(key==="width")next.height=String(size/ratio);else next.width=String(size*ratio);}
 return next;
}
