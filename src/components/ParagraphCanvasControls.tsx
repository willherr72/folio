import {useLayoutEffect,useRef,useState,type PointerEvent} from "react";
import {screenArrowDelta} from "../editor/geometry";
import {imagePointerPoint} from "../editor/existing-image-canvas";
import {changeParagraphArea,type CanvasParagraphView,type ParagraphHandle} from "../editor/paragraph-canvas";
import {MAX_PARAGRAPH_PIECES} from "../editor/paragraph-preview";
import type {AnnotationRect,InkPoint,PagePlan} from "../editor/types";
import "./paragraph-workspace.css";

type Gesture={pointer:number;handle:ParagraphHandle;start:InkPoint;client:InkPoint;area:AnnotationRect;moved:boolean};
const handles=[['nw','northwest',0,0],['n','north',.5,0],['ne','northeast',1,0],['e','east',1,.5],['se','southeast',1,1],['s','south',.5,1],['sw','southwest',0,1],['w','west',0,.5]] as const;
export function ParagraphCanvasControls({view,page,zoom}:{view:CanvasParagraphView;page:PagePlan;zoom:number}){
 const root=useRef<SVGGElement>(null),gesture=useRef<Gesture|null>(null),latest=useRef({view,page});latest.current={view,page};
 const [draft,setDraft]=useState<AnnotationRect|null>(null);
 const release=(pointer:number)=>{try{root.current?.releasePointerCapture(pointer);}catch{/* Host may already have released it. */}};
 const cancel=()=>{const active=gesture.current;gesture.current=null;setDraft(null);if(active){release(active.pointer);latest.current.view.onGestureActive(false);}};
 const signature=JSON.stringify(view.bounds);
 useLayoutEffect(cancel,[view.sessionKey,view.enabled,view.choosing,signature,page.sourceId,page.pageIndex,page.width,page.height,page.rotation,page.crop?.x,page.crop?.y,page.crop?.width,page.crop?.height,zoom]);
 useLayoutEffect(()=>{
  const escape=(event:KeyboardEvent)=>{if(event.key==="Escape"&&gesture.current){event.preventDefault();event.stopImmediatePropagation();cancel();}};
  window.addEventListener("keydown",escape,true);
  return()=>{window.removeEventListener("keydown",escape,true);const active=gesture.current;gesture.current=null;if(active){release(active.pointer);latest.current.view.onGestureActive(false);}};
 },[]);
 const point=(event:PointerEvent)=>{const rect=root.current?.ownerSVGElement?.getBoundingClientRect();return rect?imagePointerPoint(event.clientX,event.clientY,rect,latest.current.page):null;};
 const change=(active:Gesture,event:PointerEvent)=>{
  const p=point(event);if(!p)return active.area;
  active.moved ||= Math.hypot(event.clientX-active.client.x,event.clientY-active.client.y)>=3;
  return active.moved?changeParagraphArea(active.area,active.handle,{x:p.x-active.start.x,y:p.y-active.start.y},latest.current.page.width,latest.current.page.height):active.area;
 };
 const begin=(event:PointerEvent,handle:ParagraphHandle)=>{
  if(!view.enabled||view.choosing||!view.bounds||gesture.current||event.button!==0||event.isPrimary===false)return;
  const p=point(event);if(!p)return;
  event.preventDefault();event.stopPropagation();
  try{root.current?.setPointerCapture(event.pointerId);}catch{return;}
  if(event.currentTarget instanceof SVGElement)event.currentTarget.focus({preventScroll:true});
  gesture.current={pointer:event.pointerId,handle,start:p,client:{x:event.clientX,y:event.clientY},area:view.bounds,moved:false};setDraft(view.bounds);view.onGestureActive(true);
 };
 const finish=(event:PointerEvent,cancelled=false)=>{
  const active=gesture.current;if(!active||event.pointerId!==active.pointer)return;
  event.preventDefault();event.stopPropagation();const next=cancelled?active.area:change(active,event);cancel();
  if(!cancelled&&latest.current.view.enabled&&next!==active.area)latest.current.view.onChange(next);
 };
 const keyboard=(event:React.KeyboardEvent,handle:ParagraphHandle)=>{
  if(!view.enabled||!view.bounds||gesture.current||event.ctrlKey||event.metaKey||event.altKey)return;
  const delta=screenArrowDelta(event.key,event.shiftKey,page.rotation);if(!delta)return;
  event.preventDefault();event.stopPropagation();const next=changeParagraphArea(view.bounds,handle,delta,page.width,page.height);if(next!==view.bounds)view.onChange(next);
 };
 const crop=page.crop??{x:0,y:0,width:page.width,height:page.height};
 const visible=(p:InkPoint)=>p.x>=Math.max(0,crop.x)&&p.y>=Math.max(0,crop.y)&&p.x<=Math.min(page.width,crop.x+crop.width)&&p.y<=Math.min(page.height,crop.y+crop.height);
 const clipped=(r:AnnotationRect)=>{const x=Math.max(0,crop.x,r.x),y=Math.max(0,crop.y,r.y),right=Math.min(page.width,crop.x+crop.width,r.x+r.width),bottom=Math.min(page.height,crop.y+crop.height,r.y+r.height);return right>x&&bottom>y?{x,y,width:right-x,height:bottom-y}:null;};
 const area=draft??view.bounds,body=area?clipped(area):null,unit=100/zoom;
 return <g ref={root} data-paragraph-controls="true" onPointerMove={event=>{const active=gesture.current;if(!active||active.pointer!==event.pointerId)return;event.preventDefault();event.stopPropagation();setDraft(change(active,event));}} onPointerUp={event=>finish(event)} onPointerCancel={event=>finish(event,true)} onLostPointerCapture={event=>finish(event,true)}>
  {view.choosing&&view.runs.map(run=>{const r=clipped(run.bounds);if(!r)return null;const pressed=view.selected.includes(run.objectIndex),disabled=!view.enabled||(!pressed&&view.selected.length>=MAX_PARAGRAPH_PIECES);return <rect key={run.objectIndex} {...r} className={`paragraph-piece-target${pressed?" selected":""}`} role="button" tabIndex={disabled?-1:0} aria-label={`Select text piece: ${run.text}`} aria-pressed={pressed} aria-disabled={disabled} onClick={event=>{event.stopPropagation();if(!disabled)view.onToggle(run.objectIndex);}} onKeyDown={event=>{if(event.key==="Enter"||event.key===" "){event.preventDefault();event.stopPropagation();if(!disabled)view.onToggle(run.objectIndex);}}}/>;})}
  {!view.choosing&&body&&<rect {...body} className="paragraph-canvas-area" role="button" tabIndex={view.enabled?0:-1} aria-disabled={!view.enabled||undefined} aria-label="Move paragraph area" onPointerDown={event=>begin(event,"move")} onKeyDown={event=>keyboard(event,"move")}><title>Drag or use arrows to move · Shift for larger steps · Escape cancels a drag</title></rect>}
  {!view.choosing&&area&&handles.map(([handle,name,dx,dy])=>{const p={x:area.x+area.width*dx,y:area.y+area.height*dy};return visible(p)?<g key={handle} className="paragraph-canvas-handle" role="button" tabIndex={view.enabled?0:-1} aria-disabled={!view.enabled||undefined} aria-label={`Resize paragraph ${name}`} onPointerDown={event=>begin(event,handle)} onKeyDown={event=>keyboard(event,handle)}><rect className="paragraph-handle-target" x={p.x-12*unit} y={p.y-12*unit} width={24*unit} height={24*unit}/><rect className="paragraph-handle-mark" x={p.x-4*unit} y={p.y-4*unit} width={8*unit} height={8*unit}/></g>:null;})}
 </g>;
}
