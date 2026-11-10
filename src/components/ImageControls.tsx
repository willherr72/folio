import {rectVisibleOnPage} from "../editor/geometry";
import {useLayoutEffect,useRef,useState,type PointerEvent,type RefObject} from "react";
import {IMAGE_CORNERS,imageCorner,moveImage,resizeImage,type ImageCorner} from "../editor/image-geometry";
import {screenArrowDelta} from "../editor/geometry";
import type {EditorTool,ImageOverlay,InkPoint,PagePlan} from "../editor/types";
import "./image-controls.css";
type Gesture={pointer:number;overlay:ImageOverlay;start:InkPoint;corner?:ImageCorner};
interface Options {page:PagePlan;tool:EditorTool;zoom:number;selectedId:string|null;disabled:boolean;locked:boolean;svg:RefObject<SVGSVGElement|null>;onChange?(before:ImageOverlay,after:ImageOverlay):void}
export function useImageGesture(options:Options){
 const gesture=useRef<Gesture|null>(null),latest=useRef(options);latest.current=options;
 const [draft,setDraft]=useState<ImageOverlay|null>(null);
 const release=(pointer:number)=>{try{latest.current.svg.current?.releasePointerCapture(pointer);}catch{/* Capture may already be released. */}};
 const cancel=()=>{const active=gesture.current;gesture.current=null;setDraft(null);if(active)release(active.pointer);};
 useLayoutEffect(cancel,[options.page.id,options.page.sourceId,options.page.pageIndex,options.page.width,options.page.height,options.page.rotation,options.tool,options.zoom,options.disabled,options.locked]);
 useLayoutEffect(()=>{const active=gesture.current;if(active&&(options.selectedId!==active.overlay.id||options.page.overlays.find(o=>o.id===active.overlay.id)!==active.overlay))cancel();},[options.selectedId,options.page.overlays]);
 useLayoutEffect(()=>{const escape=(event:KeyboardEvent)=>{if(event.key==="Escape"&&gesture.current){event.preventDefault();cancel();}};window.addEventListener("keydown",escape);return()=>{window.removeEventListener("keydown",escape);const active=gesture.current;gesture.current=null;if(active)release(active.pointer);};},[]);
 const changed=(active:Gesture,point:InkPoint)=>active.corner?resizeImage(active.overlay,active.corner,point,options.page.width,options.page.height,options.locked):moveImage(active.overlay,active.overlay.x+point.x-active.start.x,active.overlay.y+point.y-active.start.y,options.page.width,options.page.height);
 return {draft,isActive:()=>!!gesture.current,
  begin(event:PointerEvent,overlay:ImageOverlay,point:InkPoint,corner?:ImageCorner){
   if(options.disabled||options.tool!=="select"||gesture.current||event.button!==0)return;
   event.preventDefault();event.stopPropagation();gesture.current={pointer:event.pointerId,overlay,start:point,corner};setDraft(overlay);
   try{options.svg.current?.setPointerCapture(event.pointerId);}catch{/* Synthetic events may not own a pointer. */}
  },
  move(event:PointerEvent,point:InkPoint){const active=gesture.current;if(!active)return false;if(active.pointer===event.pointerId&&!options.disabled)setDraft(changed(active,point));return true;},
  finish(event:PointerEvent,point:InkPoint|null,cancelled:boolean){
   const active=gesture.current;if(!active)return false;if(active.pointer!==event.pointerId)return true;cancel();
   if(!cancelled&&point&&!latest.current.disabled){const next=changed(active,point);if(next!==active.overlay)options.onChange?.(active.overlay,next);}return true;
  },
 };
}
const labels={nw:"top left",ne:"top right",se:"bottom right",sw:"bottom left"};
export function ImageHandles({image,page,zoom,locked,onStart,onChange}:{image:ImageOverlay;page:PagePlan;zoom:number;locked:boolean;onStart(event:PointerEvent,image:ImageOverlay,corner:ImageCorner):void;onChange(before:ImageOverlay,after:ImageOverlay):void}){
 const unit=100/zoom;
 return <g className="image-handles">{IMAGE_CORNERS.map((corner,index)=>{
  const point=imageCorner(image,corner),displayCorner=IMAGE_CORNERS[(index+page.rotation/90)%4];
  if(page.crop&&!rectVisibleOnPage({x:point.x-4*unit,y:point.y-4*unit,width:8*unit,height:8*unit},page))return null;
  return <g key={corner} data-image-corner={corner} role="button" tabIndex={0} aria-label={`Resize image ${labels[displayCorner]}`} style={{cursor:(displayCorner==="nw"||displayCorner==="se")?"nwse-resize":"nesw-resize"}}
   onPointerDown={event=>onStart(event,image,corner)} onKeyDown={event=>{
    const delta=screenArrowDelta(event.key,event.shiftKey,page.rotation);if(!delta||event.ctrlKey||event.metaKey||event.altKey)return;
    event.preventDefault();event.stopPropagation();const next=resizeImage(image,corner,{x:point.x+delta.x,y:point.y+delta.y},page.width,page.height,locked);if(next!==image)onChange(image,next);
   }}><title>Arrow keys resize · Shift for larger steps</title><rect className="image-handle-target" x={point.x-12*unit} y={point.y-12*unit} width={24*unit} height={24*unit}/><rect className="image-handle-mark" x={point.x-4*unit} y={point.y-4*unit} width={8*unit} height={8*unit}/></g>;
 })}</g>;
}
