import type {ImageOverlay,AnnotationRect,InkPoint,Rotation} from "./types";
export type ImageCorner="nw"|"ne"|"se"|"sw";
export const IMAGE_CORNERS:ImageCorner[]=["nw","ne","se","sw"];
const positive=(...values:number[])=>values.every(value=>Number.isFinite(value)&&value>0);
const clamp=(value:number,low:number,high:number)=>Math.max(low,Math.min(high,value));
export function imageBounds(image:ImageOverlay) {
  switch(image.rotation) {
    case 90:return {x:image.x-image.height,y:image.y,width:image.height,height:image.width};
    case 180:return {x:image.x-image.width,y:image.y-image.height,width:image.width,height:image.height};
    case 270:return {x:image.x,y:image.y-image.width,width:image.height,height:image.width};
    default:return {x:image.x,y:image.y,width:image.width,height:image.height};
  }
}
export function moveImage(image:ImageOverlay,x:number,y:number,pageWidth:number,pageHeight:number):ImageOverlay {
  if(!Number.isFinite(x)||!Number.isFinite(y)||!positive(pageWidth,pageHeight))return image;
  const box=imageBounds({...image,x,y});
  const left=Math.max(0,Math.min(pageWidth-box.width,box.x));
  const top=Math.max(0,Math.min(pageHeight-box.height,box.y));
  return unchanged(image,{...image,x:x+left-box.x,y:y+top-box.y});
}

function unchanged(before:ImageOverlay,after:ImageOverlay):ImageOverlay {
  return before.x===after.x&&before.y===after.y&&before.width===after.width&&before.height===after.height&&before.rotation===after.rotation?before:after;
}
/** Convert the visible axis-aligned frame back to the native rotated anchor. */
function fromBounds(image:ImageOverlay,box:AnnotationRect):ImageOverlay {
  const {x,y,width:w,height:h}=box;
  switch(image.rotation){
    case 90:return unchanged(image,{...image,x:x+w,y,width:h,height:w});
    case 180:return unchanged(image,{...image,x:x+w,y:y+h,width:w,height:h});
    case 270:return unchanged(image,{...image,x,y:y+h,width:h,height:w});
    default:return unchanged(image,{...image,x,y,width:w,height:h});
  }
}
export function imageCorner(image:ImageOverlay,corner:ImageCorner):InkPoint {
  const box=imageBounds(image);return {x:box.x+(corner.includes("e")?box.width:0),y:box.y+(corner.includes("s")?box.height:0)};
}
/** Displayed pixel dimensions already include EXIF orientation. Small images use
 * 96 pixels per inch; large images initially occupy at most 60% of either axis. */
export function fitImage(id:string,asset:{id:string;width:number;height:number},pageWidth:number,pageHeight:number,center:InkPoint={x:pageWidth/2,y:pageHeight/2}):ImageOverlay|null {
  if(!positive(asset.width,asset.height,pageWidth,pageHeight)||![center.x,center.y].every(Number.isFinite))return null;
  const scale=Math.min(.75,pageWidth*.6/asset.width,pageHeight*.6/asset.height),width=asset.width*scale,height=asset.height*scale;
  return {type:"image",id,assetId:asset.id,width,height,rotation:0,x:clamp(center.x-width/2,0,pageWidth-width),y:clamp(center.y-height/2,0,pageHeight-height)};
}
/** Corners refer to the visible frame in unrotated page coordinates. The opposite
 * corner is invariant; crossing it clamps the size instead of mirroring pixels. */
export function resizeImage(image:ImageOverlay,corner:ImageCorner,point:InkPoint,pageWidth:number,pageHeight:number,locked:boolean):ImageOverlay {
  if(!positive(image.width,image.height,pageWidth,pageHeight)||![point.x,point.y].every(Number.isFinite))return image;
  const box=imageBounds(image),west=corner.includes("w"),north=corner.includes("n");
  const fixed={x:west?box.x+box.width:box.x,y:north?box.y+box.height:box.y};
  const availableW=west?fixed.x:pageWidth-fixed.x,availableH=north?fixed.y:pageHeight-fixed.y;
  if(!positive(availableW,availableH))return image;
  const minimum=Math.min(.1,box.width,box.height),requestedW=(point.x-fixed.x)*(west?-1:1),requestedH=(point.y-fixed.y)*(north?-1:1);
  let width:number,height:number;
  if(locked){
    const sx=requestedW/box.width,sy=requestedH/box.height;
    const requested=Math.abs(sx-1)>=Math.abs(sy-1)?sx:sy;
    const maxScale=Math.min(availableW/box.width,availableH/box.height),minScale=Math.min(maxScale,Math.max(minimum/box.width,minimum/box.height));
    const scale=clamp(requested,minScale,maxScale);width=box.width*scale;height=box.height*scale;
  }else{width=clamp(requestedW,Math.min(minimum,availableW),availableW);height=clamp(requestedH,Math.min(minimum,availableH),availableH);}
  return fromBounds(image,{x:west?fixed.x-width:fixed.x,y:north?fixed.y-height:fixed.y,width,height});
}
/** Numeric dimensions describe the visible frame. Its top-left remains fixed. */
export function resizeImageTo(image:ImageOverlay,width:number,height:number,pageWidth:number,pageHeight:number,locked:boolean,axis:"width"|"height"):ImageOverlay {
  if(!positive(width,height,pageWidth,pageHeight))return image;
  const box=imageBounds(image);
  if(locked){if(axis==="width")height=width*box.height/box.width;else width=height*box.width/box.height;}
  return resizeImage(image,"se",{x:box.x+width,y:box.y+height},pageWidth,pageHeight,locked);
}
export function rotateImage(image:ImageOverlay,direction:1|-1,pageWidth:number,pageHeight:number):ImageOverlay {
  if(!positive(image.width,image.height,pageWidth,pageHeight))return image;
  const before=imageBounds(image),rotation=((image.rotation+direction*90+360)%360) as Rotation;
  const rotated={...image,rotation},box=imageBounds(rotated),scale=Math.min(1,pageWidth/box.width,pageHeight/box.height);
  const width=box.width*scale,height=box.height*scale;
  return fromBounds(rotated,{x:clamp(before.x+before.width/2-width/2,0,pageWidth-width),y:clamp(before.y+before.height/2-height/2,0,pageHeight-height),width,height});
}
