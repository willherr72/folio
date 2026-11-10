import {displayDimensions,fromDisplayPoint,pageVisibleRect} from "./geometry";
import {pageAllows} from "./protection";
import type {AnnotationRect,EditorDocument,PagePlan} from "./types";

export interface CropMargins {left:number;top:number;right:number;bottom:number}
export function validPageCrop(page:Pick<PagePlan,"width"|"height">,crop:AnnotationRect):boolean {
  return [crop.x,crop.y,crop.width,crop.height,page.width,page.height].every(Number.isFinite)
    &&crop.x>=0&&crop.y>=0&&crop.width>=1&&crop.height>=1
    &&crop.x+crop.width<=page.width&&crop.y+crop.height<=page.height;
}
export function cropMargins(page:PagePlan):CropMargins {
  const full=displayDimensions(page.width,page.height,page.rotation),box=pageVisibleRect(page);
  return {left:box.x,top:box.y,right:full.width-box.x-box.width,bottom:full.height-box.y-box.height};
}
export function cropFromMargins(page:PagePlan,margins:CropMargins):AnnotationRect|null {
  if(!Object.values(margins).every(value=>Number.isFinite(value)&&value>=0))return null;
  const full=displayDimensions(page.width,page.height,page.rotation);
  if(full.width-margins.left-margins.right<1||full.height-margins.top-margins.bottom<1)return null;
  const a=fromDisplayPoint({x:margins.left,y:margins.top},page.width,page.height,page.rotation);
  const b=fromDisplayPoint({x:full.width-margins.right,y:full.height-margins.bottom},page.width,page.height,page.rotation);
  const crop={x:Math.min(a.x,b.x),y:Math.min(a.y,b.y),width:Math.abs(a.x-b.x),height:Math.abs(a.y-b.y)};
  return validPageCrop(page,crop)?crop:null;
}
export function cropPage(document:EditorDocument,pageId:string,crop?:AnnotationRect):EditorDocument {
  const page=document.pages.find(p=>p.id===pageId);
  if(!page||!pageAllows(page,"editContent")||(crop&&!validPageCrop(page,crop)))return document;
  if(crop&&crop.x===0&&crop.y===0&&crop.width===page.width&&crop.height===page.height)crop=undefined;
  const before=page.crop;
  if((!before&&!crop)||(before&&crop&&before.x===crop.x&&before.y===crop.y&&before.width===crop.width&&before.height===crop.height))return document;
  const next={...page};if(crop)next.crop={...crop};else delete next.crop;
  return {...document,pages:document.pages.map(p=>p===page?next:p),selectedOverlayId:null};
}
