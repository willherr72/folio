import type { AnnotationRect, InkPoint, PagePlan, Rotation, TextOverlay } from "./types";
import {screenArrowDelta} from "./geometry";

export type AreaCorner = "nw" | "ne" | "se" | "sw";
export const AREA_CORNERS: AreaCorner[] = ["nw", "ne", "se", "sw"];
export const MIN_AREA_SIZE = 36;
const clamp = (value: number, low: number, high: number) => Math.max(low, Math.min(high, value));
function rotate(point: InkPoint, rotation: number): InkPoint {
  switch (rotation) {
    case 90: return {x:-point.y,y:point.x};
    case 180: return {x:-point.x,y:-point.y};
    case 270: return {x:point.y,y:-point.x};
    default: return point;
  }
}
export function areaFromDrag(start: InkPoint, end: InkPoint, pageWidth: number, pageHeight: number): AnnotationRect | null {
  if (![start.x,start.y,end.x,end.y,pageWidth,pageHeight].every(Number.isFinite)) return null;
  const x=clamp(Math.min(start.x,end.x),0,pageWidth), y=clamp(Math.min(start.y,end.y),0,pageHeight);
  const width=clamp(Math.max(start.x,end.x),0,pageWidth)-x, height=clamp(Math.max(start.y,end.y),0,pageHeight)-y;
  return width >= MIN_AREA_SIZE && height >= MIN_AREA_SIZE ? {x,y,width,height} : null;
}
export function defaultTextArea(pageWidth: number, pageHeight: number): AnnotationRect | null {
  const x=Math.max(0,Math.min(24,(pageWidth-MIN_AREA_SIZE)/2)), y=Math.max(0,Math.min(24,(pageHeight-MIN_AREA_SIZE)/2));
  return areaFromDrag({x,y},{x:x+Math.min(240,pageWidth-2*x),y:y+Math.min(144,pageHeight-2*y)},pageWidth,pageHeight);
}
/** New areas start inside the retained page; existing areas keep source coordinates. */
export function defaultPageTextArea(page: PagePlan): AnnotationRect | null {
  const bounds=page.crop??{x:0,y:0,width:page.width,height:page.height};
  const area=defaultTextArea(bounds.width,bounds.height);
  return area?{...area,x:area.x+bounds.x,y:area.y+bounds.y}:null;
}
export function areaCorner(overlay: TextOverlay, corner: AreaCorner): InkPoint {
  const vector=rotate({x:corner.includes("e") ? overlay.area!.width : 0,y:corner.includes("s") ? overlay.area!.height : 0},overlay.rotation??0);
  return {x:overlay.x+vector.x,y:overlay.y+vector.y};
}
/** Resize in the area's local axes; the opposite corner is invariant. */
export function resizeArea(overlay: TextOverlay, corner: AreaCorner, point: InkPoint, pageWidth: number, pageHeight: number): TextOverlay {
  if (!overlay.area || ![point.x,point.y,pageWidth,pageHeight].every(Number.isFinite)) return overlay;
  const inverse=(360-(overlay.rotation??0))%360;
  const local=(p:InkPoint)=>rotate({x:p.x-overlay.x,y:p.y-overlay.y},inverse);
  const corners=[{x:0,y:0},{x:pageWidth,y:0},{x:0,y:pageHeight},{x:pageWidth,y:pageHeight}].map(local);
  const page={left:Math.min(...corners.map(p=>p.x)),right:Math.max(...corners.map(p=>p.x)),top:Math.min(...corners.map(p=>p.y)),bottom:Math.max(...corners.map(p=>p.y))};
  const target=local(point), minimum=Math.max(MIN_AREA_SIZE,overlay.area.inset*2+1);
  let left=0,top=0,right=overlay.area.width,bottom=overlay.area.height;
  const xLimits=corner.includes("w") ? [page.left,right-minimum] : [minimum,page.right];
  const yLimits=corner.includes("n") ? [page.top,bottom-minimum] : [minimum,page.bottom];
  if(xLimits[0]>xLimits[1] || yLimits[0]>yLimits[1]) return overlay;
  if(corner.includes("w")) left=clamp(target.x,...xLimits as [number,number]);
  else right=clamp(target.x,...xLimits as [number,number]);
  if(corner.includes("n")) top=clamp(target.y,...yLimits as [number,number]);
  else bottom=clamp(target.y,...yLimits as [number,number]);
  const offset=rotate({x:left,y:top},overlay.rotation??0), width=right-left,height=bottom-top;
  if(left===0 && top===0 && width===overlay.area.width && height===overlay.area.height) return overlay;
  return {...overlay,x:overlay.x+offset.x,y:overlay.y+offset.y,area:{...overlay.area,width,height}};
}
export function resizeAreaTo(overlay: TextOverlay, width: number, height: number, pageWidth: number, pageHeight: number): TextOverlay {
  if(!overlay.area || ![width,height].every(Number.isFinite) || width<=0 || height<=0) return overlay;
  const vector=rotate({x:width,y:height},overlay.rotation??0);
  return resizeArea(overlay,"se",{x:overlay.x+vector.x,y:overlay.y+vector.y},pageWidth,pageHeight);
}
/** Keep the complete rotated rectangle on the page while moving it. */
export function moveArea(overlay: TextOverlay, x: number, y: number, pageWidth: number, pageHeight: number): TextOverlay {
  if(!overlay.area || ![x,y].every(Number.isFinite)) return overlay;
  const corners=AREA_CORNERS.map(corner=>areaCorner({...overlay,x:0,y:0},corner));
  const left=Math.min(...corners.map(p=>p.x)), right=Math.max(...corners.map(p=>p.x));
  const top=Math.min(...corners.map(p=>p.y)), bottom=Math.max(...corners.map(p=>p.y));
  return {...overlay,x:clamp(x,-left,Math.max(-left,pageWidth-right)),y:clamp(y,-top,Math.max(-top,pageHeight-bottom))};
}
/** Arrow-key deltas follow the screen even when the editor page is rotated. */
export function areaKeyboardDelta(key: string, shift: boolean, pageRotation: Rotation): InkPoint | null {
  return screenArrowDelta(key,shift,pageRotation);
}
