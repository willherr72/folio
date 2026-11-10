import type {ExistingImage,ImageCrop,ImagePoint} from "./types";
type Quad=ExistingImage["corners"];
export const fullCrop:Readonly<ImageCrop>=Object.freeze({left:0,top:0,right:1,bottom:1});
const minimum=.001;
const clamp=(n:number,low:number,high:number)=>Math.max(low,Math.min(high,n));
export function validateCrop(value:unknown):asserts value is ImageCrop {
 const crop=value as ImageCrop|null;
 if(!crop||![crop.left,crop.top,crop.right,crop.bottom].every(v=>typeof v==="number"&&Number.isFinite(v)&&v>=0&&v<=1)
  ||crop.right-crop.left<minimum-8*Number.EPSILON||crop.bottom-crop.top<minimum-8*Number.EPSILON)throw new Error("Keep at least 0.1% of each image dimension within the full image.");
}
const at=(q:Quad,x:number,y:number):ImagePoint=>({x:q[0].x+x*(q[1].x-q[0].x)+y*(q[3].x-q[0].x),y:q[0].y+x*(q[1].y-q[0].y)+y*(q[3].y-q[0].y)});
export function cropFrame(full:Quad,crop:ImageCrop=fullCrop):Quad {
 validateCrop(crop);
 if(crop.left===0&&crop.top===0&&crop.right===1&&crop.bottom===1)return full;
 return [at(full,crop.left,1-crop.bottom),at(full,crop.right,1-crop.bottom),at(full,crop.right,1-crop.top),at(full,crop.left,1-crop.top)];
}
export function fullFrameFromVisible(visible:Quad,crop:ImageCrop):Quad {
 validateCrop(crop);
 if(crop.left===0&&crop.top===0&&crop.right===1&&crop.bottom===1)return visible;
 const w=crop.right-crop.left,h=crop.bottom-crop.top;
 return [[0,0],[1,0],[1,1],[0,1]].map(([x,y])=>at(visible,(x-crop.left)/w,(y-(1-crop.bottom))/h)) as Quad;
}
/** Image-local x goes right; y goes down, including reflected/sheared images. */
export function imagePointToUnit(full:Quad,point:ImagePoint):ImagePoint {
 const u={x:full[1].x-full[0].x,y:full[1].y-full[0].y},v={x:full[3].x-full[0].x,y:full[3].y-full[0].y};
 const dx=point.x-full[0].x,dy=point.y-full[0].y,det=u.x*v.y-u.y*v.x;
 if(!Number.isFinite(det)||Math.abs(det)<=Math.hypot(u.x,u.y)*Math.hypot(v.x,v.y)*1e-10)throw new Error("The image frame is too narrow to crop.");
 return {x:(dx*v.y-dy*v.x)/det,y:1-(u.x*dy-u.y*dx)/det};
}
export function cropCorner(full:Quad,crop:ImageCrop,corner:0|1|2|3,point:ImagePoint):ImageCrop {
 const p=imagePointToUnit(full,point);if(!Number.isFinite(p.x)||!Number.isFinite(p.y))return crop;
 const left=corner===0||corner===3,bottom=corner===0||corner===1;
 return {...crop,[left?"left":"right"]:clamp(p.x,left?0:crop.left+minimum,left?crop.right-minimum:1),[bottom?"bottom":"top"]:clamp(p.y,bottom?crop.top+minimum:0,bottom?1:crop.bottom-minimum)};
}
export function cropMove(full:Quad,crop:ImageCrop,delta:ImagePoint):ImageCrop {
 if((delta.x===0&&delta.y===0)||!Number.isFinite(delta.x)||!Number.isFinite(delta.y))return crop;
 const a=imagePointToUnit(full,full[0]),b=imagePointToUnit(full,{x:full[0].x+delta.x,y:full[0].y+delta.y});
 const dx=clamp(b.x-a.x,-crop.left,1-crop.right),dy=clamp(b.y-a.y,-crop.top,1-crop.bottom);
 return {left:crop.left+dx,right:crop.right+dx,top:crop.top+dy,bottom:crop.bottom+dy};
}
