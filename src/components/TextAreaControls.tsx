import {rectVisibleOnPage} from "../editor/geometry";
import { useEffect, useRef, useState, type PointerEvent, type RefObject } from "react";
import { AREA_CORNERS, areaCorner, areaFromDrag, areaKeyboardDelta, resizeArea, type AreaCorner } from "../editor/text-area-geometry";
import type { AnnotationRect, EditorTool, InkPoint, PagePlan, TextOverlay } from "../editor/types";
import "./text-area-controls.css";

type Gesture = {pointer:number;kind:"create";start:InkPoint} | {pointer:number;kind:"resize";overlay:TextOverlay;corner:AreaCorner};
interface Options {
  page:PagePlan;tool:EditorTool;disabled?:boolean;zoom:number;selectedId:string|null;
  svg:RefObject<SVGSVGElement|null>;
  onAdd?(rect:AnnotationRect):void;
  onResize?(id:string,rect:AnnotationRect):void;
}
const geometry=(overlay:TextOverlay):AnnotationRect=>({x:overlay.x,y:overlay.y,width:overlay.area!.width,height:overlay.area!.height});

/** Keep gestures local until pointerup, so cancellation never touches history/recovery. */
export function useTextAreaGesture(options:Options) {
  const gesture=useRef<Gesture|null>(null);
  const [draft,setDraft]=useState<{rect?:AnnotationRect;overlay?:TextOverlay}|null>(null);
  const latest=useRef(options);latest.current=options;
  const release=(pointer:number)=>{try{options.svg.current?.releasePointerCapture(pointer);}catch{/* Browser may already have released capture. */}};
  const cancel=()=>{const active=gesture.current;gesture.current=null;setDraft(null);if(active)release(active.pointer);};
  useEffect(cancel,[options.page.id,options.page.sourceId,options.page.pageIndex,options.page.width,options.page.height,options.page.rotation,options.tool,options.disabled,options.zoom]);
  useEffect(()=>{
    const active=gesture.current;
    if(active?.kind==="resize" && (options.selectedId!==active.overlay.id || options.page.overlays.find(o=>o.id===active.overlay.id)!==active.overlay)) cancel();
  },[options.selectedId,options.page.overlays]);
  useEffect(()=>{
    const escape=(event:KeyboardEvent)=>{if(event.key==="Escape" && gesture.current){event.preventDefault();cancel();}};
    window.addEventListener("keydown",escape);
    return ()=>{window.removeEventListener("keydown",escape);const active=gesture.current;gesture.current=null;if(active)release(active.pointer);};
  },[]);
  const begin=(event:PointerEvent,value:Gesture)=>{
    if(options.disabled || gesture.current || event.button!==0) return;
    event.preventDefault();gesture.current=value;
    try{options.svg.current?.setPointerCapture(event.pointerId);}catch{/* Synthetic events may have no native pointer. */}
    setDraft(value.kind==="resize"?{overlay:value.overlay}:{});
  };
  return {
    active:!!draft, isActive:()=>gesture.current!==null, draft,
    beginCreate:(event:PointerEvent,point:InkPoint)=>begin(event,{kind:"create",pointer:event.pointerId,start:point}),
    beginResize:(event:PointerEvent,overlay:TextOverlay,corner:AreaCorner)=>{
      event.stopPropagation();
      if(options.tool==="select") begin(event,{kind:"resize",pointer:event.pointerId,overlay,corner});
    },
    move:(event:PointerEvent,point:InkPoint)=>{
      const active=gesture.current;
      if(!active) return false;
      if(active.pointer===event.pointerId && !options.disabled) {
        if(active.kind==="create") setDraft({rect:{x:Math.min(active.start.x,point.x),y:Math.min(active.start.y,point.y),width:Math.abs(point.x-active.start.x),height:Math.abs(point.y-active.start.y)}});
        else setDraft({overlay:resizeArea(active.overlay,active.corner,point,options.page.width,options.page.height)});
      }
      return true;
    },
    finish:(event:PointerEvent,point:InkPoint|null,cancelled:boolean)=>{
      const active=gesture.current;
      if(!active) return false;
      if(active.pointer!==event.pointerId) return true;
      cancel();
      if(cancelled || !point || latest.current.disabled) return true;
      if(active.kind==="create") {
        const rect=areaFromDrag(active.start,point,options.page.width,options.page.height);
        if(rect) options.onAdd?.(rect);
      } else {
        const resized=resizeArea(active.overlay,active.corner,point,options.page.width,options.page.height);
        if(resized!==active.overlay) options.onResize?.(active.overlay.id,geometry(resized));
      }
      return true;
    },
  };
}

const labels={nw:"top left",ne:"top right",se:"bottom right",sw:"bottom left"};
export function TextAreaHandles({overlay,page,zoom,onStart,onResize}:{overlay:TextOverlay;page:PagePlan;zoom:number;onStart(event:PointerEvent,overlay:TextOverlay,corner:AreaCorner):void;onResize?(id:string,rect:AnnotationRect):void}) {
  const unit=100/zoom;
  return <g className="text-area-handles">{AREA_CORNERS.map(corner=>{
    const point=areaCorner(overlay,corner);
    if(page.crop&&!rectVisibleOnPage({x:point.x-4*unit,y:point.y-4*unit,width:8*unit,height:8*unit},page))return null;
    const diagonal=((overlay.rotation??0)+page.rotation)%180===90;
    return <g key={corner} role="button" tabIndex={0} aria-label={`Resize text area ${labels[corner]}`} style={{cursor:(corner==="nw"||corner==="se")!==diagonal?"nwse-resize":"nesw-resize"}}
      onPointerDown={event=>onStart(event,overlay,corner)} onKeyDown={event=>{
        const delta=areaKeyboardDelta(event.key,event.shiftKey,page.rotation);
        if(!delta || event.ctrlKey || event.metaKey || event.altKey) return;
        event.preventDefault();event.stopPropagation();
        const resized=resizeArea(overlay,corner,{x:point.x+delta.x,y:point.y+delta.y},page.width,page.height);
        if(resized!==overlay) onResize?.(overlay.id,geometry(resized));
      }}>
      <title>Arrow keys resize · Shift for larger steps</title>
      <rect className="area-handle-target" x={point.x-12*unit} y={point.y-12*unit} width={24*unit} height={24*unit}/>
      <rect className="area-handle-mark" x={point.x-4*unit} y={point.y-4*unit} width={8*unit} height={8*unit}/>
    </g>;
  })}</g>;
}
