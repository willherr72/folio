import type {ExistingImage,ExistingImages,ImagePoint,PagePlan} from "./types";
import {cropFrame,validateCrop} from "./existing-image-crop";
const cross=(a:ImagePoint,b:ImagePoint,c:ImagePoint)=>(b.x-a.x)*(c.y-b.y)-(b.y-a.y)*(c.x-b.x);
export function validateExistingImages(value:ExistingImages):ExistingImage[]{
 if(!value||!Array.isArray(value.images)||value.images.length>5000)throw new Error("Invalid image discovery response.");
 const seen=new Set<string>();
 return value.images.map(image=>{
  const path=image?.objectPath,quad=image?.corners,b=image?.bounds;
  if(!image||!Array.isArray(path)||!path.length||path.length>13||path.some(i=>!Number.isSafeInteger(i)||i<0||i>=100000)
   ||typeof image.selectionId!=="string"||!/^[a-f0-9]{64}$/.test(image.selectionId)||!Array.isArray(quad)||quad.length!==4
   ||quad.some(p=>!p||!Number.isFinite(p.x)||!Number.isFinite(p.y))||!b||![b.x,b.y,b.width,b.height].every(Number.isFinite)||b.width<=0||b.height<=0
   ||![image.pixelWidth,image.pixelHeight].every(n=>Number.isSafeInteger(n)&&n>0&&n<=32768))throw new Error("Invalid image geometry or selection.");
  const signs=quad.map((a,i)=>cross(a,quad[(i+1)%4],quad[(i+2)%4]));
  if(!signs.every(n=>n>1e-10)&&!signs.every(n=>n< -1e-10))throw new Error("Invalid image quadrilateral.");
  const key=path.join(".");if(seen.has(key))throw new Error("Duplicate image selection.");seen.add(key);
  if(image.crop!==undefined)validateCrop(image.crop);
  return {...image,...(image.crop===undefined?{}:{crop:{...image.crop}}),objectPath:[...path],bounds:{...b},corners:quad.map(p=>({...p})) as ExistingImage["corners"]};
 });
}
/** Clip the actual affine frame; bounding-box intersections can be invisible. */
export function visibleImagePolygon(image:ExistingImage,page:PagePlan):ImagePoint[]{
 const r=page.crop??{x:0,y:0,width:page.width,height:page.height};const left=Math.max(0,r.x),top=Math.max(0,r.y),right=Math.min(page.width,r.x+r.width),bottom=Math.min(page.height,r.y+r.height);
 if(left>=right||top>=bottom)return [];
 let points:ImagePoint[]=cropFrame(image.corners,image.crop).map(p=>({...p}));
 for(const [axis,edge,keepGreater] of [["x",left,true],["x",right,false],["y",top,true],["y",bottom,false]] as const){
  const input=points;points=[];
  for(let i=0;i<input.length;i++){
   const a=input[i],b=input[(i+1)%input.length],insideA=keepGreater?a[axis]>=edge:a[axis]<=edge,insideB=keepGreater?b[axis]>=edge:b[axis]<=edge;
   if(insideA)points.push(a);
   if(insideA!==insideB){const t=(edge-a[axis])/(b[axis]-a[axis]);points.push({x:a.x+t*(b.x-a.x),y:a.y+t*(b.y-a.y)});}
  }
 }
 const area=points.reduce((sum,a,i)=>{const b=points[(i+1)%points.length];return sum+a.x*b.y-a.y*b.x;},0);
 return Math.abs(area)>1e-6?points:[];
}
