import type {ReactNode} from "react";
import type {AnnotationRect,EditableTextRun,InkPoint} from "./types";
import {validParagraphBounds} from "./paragraph-preview";

export type ParagraphHandle="move"|"n"|"ne"|"e"|"se"|"s"|"sw"|"w"|"nw";
export interface CanvasParagraphView {
 pageId:string;sessionKey:string;bounds?:AnnotationRect;runs:EditableTextRun[];selected:number[];
 enabled:boolean;choosing:boolean;
 /** Raster is in unrotated source axes, with overlays already composited. */
 raster:{url?:string;error?:string;loading:boolean;retry?():void};
 onChange(bounds:AnnotationRect):void;onGestureActive(active:boolean):void;onToggle(index:number):void;
}
export interface ParagraphCanvasPresentation {
 pageId:string;sessionKey:string;zoom:number;
 renderCanvas(view:CanvasParagraphView):ReactNode;
 onLocate():void;onModalChange(open:boolean):void;
}
export function paragraphRasterWidth(pageWidth:number,zoom:number):number {
 return Math.max(600,Math.min(2400,Math.round(pageWidth*zoom/100*(globalThis.devicePixelRatio||1))));
}
export function changeParagraphArea(area:AnnotationRect,handle:ParagraphHandle,delta:InkPoint,pageWidth:number,pageHeight:number):AnnotationRect {
 if(!validParagraphBounds(area,pageWidth,pageHeight)||![delta.x,delta.y].every(Number.isFinite))return area;
 const clamp=(v:number,low:number,high:number)=>Math.max(low,Math.min(high,v));
 let {x,y,width,height}=area;
 if(handle==="move"){
  x=clamp(x+delta.x,0,pageWidth-width);y=clamp(y+delta.y,0,pageHeight-height);
 }else{
  let right=x+width,bottom=y+height;
  if(handle.includes("w"))x=clamp(x+delta.x,0,right-.01);
  if(handle.includes("e"))right=clamp(right+delta.x,x+.01,pageWidth);
  if(handle.includes("n"))y=clamp(y+delta.y,0,bottom-.01);
  if(handle.includes("s"))bottom=clamp(bottom+delta.y,y+.01,pageHeight);
  width=right-x;height=bottom-y;
 }
 return x===area.x&&y===area.y&&width===area.width&&height===area.height?area:{x,y,width,height};
}
