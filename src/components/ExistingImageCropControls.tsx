import {useLayoutEffect,useRef,useState,type PointerEvent} from "react";
import {screenArrowDelta} from "../editor/geometry";
import {visibleImagePolygon} from "../editor/existing-images";
import {imagePointerPoint,type CanvasImageView} from "../editor/existing-image-canvas";
import {cropFrame,cropCorner,cropMove,fullCrop} from "../editor/existing-image-crop";
import type {ImageCrop,ImagePoint,PagePlan} from "../editor/types";
type Kind="move"|0|1|2|3;
type Gesture={pointer:number;kind:Kind;start:ImagePoint;crop:ImageCrop;moved:boolean};
export function ExistingImageCropControls({view,page,zoom}:{view:CanvasImageView;page:PagePlan;zoom:number}){
 const root=useRef<SVGGElement>(null),gesture=useRef<Gesture|null>(null),latest=useRef({view,page,zoom});latest.current={view,page,zoom};
 const [draft,setDraft]=useState<ImageCrop|null>(null);
 const release=(pointer:number)=>{try{root.current?.releasePointerCapture(pointer);}catch{/* Host may have released capture. */}};
 const cancel=()=>{const active=gesture.current;gesture.current=null;setDraft(null);if(active){release(active.pointer);latest.current.view.onGestureActive(false);}};
 const signature=JSON.stringify([view.fullFrame,view.crop]);
 useLayoutEffect(cancel,[view.sessionKey,view.enabled,signature,page.sourceId,page.pageIndex,page.width,page.height,page.rotation,page.crop?.x,page.crop?.y,page.crop?.width,page.crop?.height,zoom]);
 useLayoutEffect(()=>{
  const escape=(event:KeyboardEvent)=>{if(event.key==="Escape"&&gesture.current){event.preventDefault();event.stopImmediatePropagation();cancel();}};
  window.addEventListener("keydown",escape,true);
  return()=>{window.removeEventListener("keydown",escape,true);const active=gesture.current;gesture.current=null;if(active){release(active.pointer);latest.current.view.onGestureActive(false);}};
 },[]);
 const point=(event:PointerEvent)=>{const rect=root.current?.ownerSVGElement?.getBoundingClientRect();return rect?imagePointerPoint(event.clientX,event.clientY,rect,latest.current.page):null;};
 const change=(active:Gesture,p:ImagePoint)=>{
  const {view,zoom}=latest.current,full=view.fullFrame??view.image.corners,delta={x:p.x-active.start.x,y:p.y-active.start.y};
  active.moved ||= Math.hypot(delta.x,delta.y)*zoom/100>=3;
  if(!active.moved)return active.crop;
  if(active.kind==="move")return cropMove(full,active.crop,delta);
  const corner=cropFrame(full,active.crop)[active.kind];
  return cropCorner(full,active.crop,active.kind,{x:corner.x+delta.x,y:corner.y+delta.y});
 };
 const begin=(event:PointerEvent,kind:Kind)=>{
  if(!latest.current.view.enabled||gesture.current||event.button!==0||event.isPrimary===false)return;
  const p=point(event);if(!p)return;event.preventDefault();event.stopPropagation();event.currentTarget instanceof SVGElement&&event.currentTarget.focus({preventScroll:true});
  const crop=latest.current.view.crop??fullCrop;
  gesture.current={pointer:event.pointerId,kind,start:p,crop,moved:false};setDraft(crop);latest.current.view.onGestureActive(true);
  try{root.current?.setPointerCapture(event.pointerId);}catch{/* Synthetic tests do not own pointers. */}
 };
 const finish=(event:PointerEvent,cancelled=false)=>{
  const active=gesture.current;if(!active||event.pointerId!==active.pointer)return;
  event.preventDefault();event.stopPropagation();const p=cancelled?null:point(event),next=p?change(active,p):active.crop;
  cancel();if(!cancelled&&p&&latest.current.view.enabled&&next!==active.crop)latest.current.view.onCropChange?.(next);
 };
 const keyboard=(event:React.KeyboardEvent,kind:Kind)=>{
  if(!view.enabled||gesture.current||event.ctrlKey||event.metaKey||event.altKey)return;
  const delta=screenArrowDelta(event.key,event.shiftKey,page.rotation);if(!delta)return;
  event.preventDefault();event.stopPropagation();const full=view.fullFrame??view.image.corners,crop=view.crop??fullCrop;
  const corner=kind==="move"?null:cropFrame(full,crop)[kind];
  view.onCropChange?.(kind==="move"?cropMove(full,crop,delta):cropCorner(full,crop,kind,{x:corner!.x+delta.x,y:corner!.y+delta.y}));
 };
 const full=view.fullFrame??view.image.corners,crop=draft??view.crop??fullCrop,q=cropFrame(full,crop),unit=100/zoom;
 const visible=visibleImagePolygon({...view.image,corners:q,crop:undefined},page),pageCrop=page.crop??{x:0,y:0,width:page.width,height:page.height};
 const handleVisible=(p:ImagePoint)=>p.x>=pageCrop.x&&p.x<=pageCrop.x+pageCrop.width&&p.y>=pageCrop.y&&p.y<=pageCrop.y+pageCrop.height&&p.x>=0&&p.y>=0&&p.x<=page.width&&p.y<=page.height;
 const path=(points:typeof full)=>`M${points.map(p=>`${p.x},${p.y}`).join(" L")} Z`;
 return <g ref={root} data-image-crop-controls="true" className="existing-image-controls"
  onPointerMove={event=>{const active=gesture.current;if(!active||event.pointerId!==active.pointer)return;event.preventDefault();event.stopPropagation();const p=point(event);if(p)setDraft(change(active,p));}}
  onPointerUp={event=>finish(event)} onPointerCancel={event=>finish(event,true)} onLostPointerCapture={event=>finish(event,true)}>
  <path className="existing-image-crop-shade" d={`${path(full)} ${path(q)}`} fillRule="evenodd"/>
  <polygon className="existing-image-full-frame" points={full.map(p=>`${p.x},${p.y}`).join(" ")}/>
  {visible.length>=3&&<polygon data-image-draft={draft?"true":undefined} className={`existing-image-frame${draft?" dragging":""}`} points={visible.map(p=>`${p.x},${p.y}`).join(" ")} role="button" tabIndex={view.enabled?0:-1} aria-disabled={!view.enabled||undefined} aria-label="Move crop window" onPointerDown={event=>begin(event,"move")} onKeyDown={event=>keyboard(event,"move")}><title>Slide the crop window · Arrows move · Shift for larger steps · Escape cancels</title></polygon>}
  {view.enabled&&([0,1,2,3] as const).filter(i=>handleVisible(q[i])).map(i=><g key={i} role="button" tabIndex={0} aria-label={`Crop image corner ${i+1}`} className="existing-image-corner" onPointerDown={event=>begin(event,i)} onKeyDown={event=>keyboard(event,i)}>
   <title>Drag or use arrows to crop · Opposite corner stays fixed</title><rect className="existing-image-handle-target" x={q[i].x-12*unit} y={q[i].y-12*unit} width={24*unit} height={24*unit}/><rect className="existing-image-handle-mark" x={q[i].x-4*unit} y={q[i].y-4*unit} width={8*unit} height={8*unit}/>
  </g>)}
 </g>;
}
